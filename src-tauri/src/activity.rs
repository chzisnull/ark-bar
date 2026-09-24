//! 「它在工作吗？」——按 Codenotch 的 activity 引擎移植（`Sources/Sessions/*ActivityMonitor.swift`）。
//!
//! 每个厂商只报它能诚实提供的东西，没有信号就不报（界面不显示状态），
//! 绝不为了凑出「工作中」而猜：
//!   - Antigravity：`brain/<会话>/.system_generated/logs/transcript.jsonl` 在跑动时被追加，
//!     且**每一步只在完成后才写入**（所以 status 永远是 DONE，没用）。回读尾部解析
//!     USER_INPUT / PLANNER_RESPONSE 判断回合是否结束，最近 45s 内写过才算在跑。
//!   - Codex：rollout 里的事件 `task_started` / `task_complete` 是真实状态；文件最近
//!     8s 内写过才算在跑。**没有事件时不做任何推断**——安静不等于回合结束。
//!   - Grok：`~/.grok/active_sessions.json` 列出的会话，其 `updates.jsonl` 最近 45s 内
//!     写过才算在跑。
//!   - 火山方舟 / Teamo 是纯 API 用量，本地没有回合概念，不产生活动条目。
//!
//! 每 2s 轮询一次（与上游一致），只在变化时广播 `activity` 事件。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const INTERVAL: Duration = Duration::from_secs(2);
/// 启动后先等窗口稳下来再扫第一遍：启动那几百毫秒正是 webview 冷启动最吃 I/O 的时候，
/// 别在这时候叠加一次文件系统扫描（background 的更新检查同样有初始延迟）。
const INITIAL_DELAY: Duration = Duration::from_secs(8);
/// Antigravity 回合中途可能长时间思考，窗口放宽（上游 staleAfter = 45s）
const AG_STALE_MS: u64 = 45_000;
/// 上游 `session()` 给 busy 的兜底：staleAfter + 15s
const AG_BUSY_EXPIRE_MS: u64 = AG_STALE_MS + 15_000;
/// 回合刚结束的宽限：9s 内仍算「已完成」，之后不再显示
const AG_SUCCESS_MS: u64 = 9_000;
/// Grok TUI 的 updates.jsonl 新鲜度窗口
const GROK_STALE_MS: u64 = 45_000;

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct Activity {
    /// "antigravity" | "codex" | "grok"
    pub provider: String,
    /// "busy" | "waiting" | "success"
    pub state: String,
    pub name: String,
    /// 界面上的短说明：工作中 / 等待回答 / 等待确认 / 已完成
    pub detail: String,
    /// 该状态开始的时刻（ms epoch）
    pub since: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn mtime_ms(p: &Path) -> Option<u64> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

fn home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// 文件尾部最多 `bytes` 字节，按 UTF-8 边界裁齐——只读尾巴，别把几百 MB 的
/// rollout 整个读进来（上游同样只读尾巴）。
fn tail(path: &Path, bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(bytes);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).to_string();
    if start == 0 {
        return Some(text);
    }
    // 起点可能落在一个字符中间，丢掉第一行（不完整的片段）
    match text.find('\n') {
        Some(i) => Some(text[i + 1..].to_string()),
        None => None,
    }
}

// ---------------------------------------------------------------- Antigravity

/// `~/.gemini/antigravity*/brain`——每个变体（antigravity、antigravity-ide、
/// antigravity-cli、antigravity-backup…）都有自己的 brain，只挑存在的那个会漏。
fn ag_transcript_roots() -> Vec<PathBuf> {
    let Some(gemini) = home().map(|h| h.join(".gemini")) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&gemini) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("antigravity"))
        .map(|e| e.path().join("brain"))
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// 所有安装里最新被写过的 transcript.jsonl。
fn ag_newest_transcript() -> Option<(PathBuf, u64)> {
    let mut newest: Option<(PathBuf, u64)> = None;
    for root in ag_transcript_roots() {
        let Ok(trajectories) = std::fs::read_dir(&root) else {
            continue;
        };
        for t in trajectories.flatten() {
            let transcript = t
                .path()
                .join(".system_generated")
                .join("logs")
                .join("transcript.jsonl");
            let Some(m) = mtime_ms(&transcript) else {
                continue;
            };
            if newest.as_ref().map(|(_, n)| m > *n).unwrap_or(true) {
                newest = Some((transcript, m));
            }
        }
    }
    newest
}

/// transcript 尾部的回合状态（`AntigravityActivityMonitor.parseState` 的移植）。
///
/// 返回 (state, waiting_for)，state ∈ busy | waiting | idle。
fn ag_parse_state(tail_text: &str) -> (String, Option<String>) {
    // 从后往前找第一条有意义的记录：PLANNER_RESPONSE 里没有 tool_calls
    // 说明模型已经把话说完（回合结束），再往前遇到的就不是「正在进行」。
    let mut is_turn_over = false;

    for line in tail_text.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(kind) = v.get("type").and_then(|x| x.as_str()) else {
            continue;
        };

        match kind {
            "USER_INPUT" => {
                return if is_turn_over {
                    ("idle".into(), None)
                } else {
                    ("busy".into(), None)
                };
            }
            "PLANNER_RESPONSE" => {
                let calls = v.get("tool_calls").and_then(|x| x.as_array());
                match calls {
                    Some(calls) if !calls.is_empty() => {
                        for call in calls {
                            let name = call.get("name").and_then(|x| x.as_str()).unwrap_or("");
                            if name.contains("ask_question") {
                                return ("waiting".into(), Some("等待回答".into()));
                            }
                            let writes = name.contains("multi_replace_file_content")
                                || name.contains("write_to_file")
                                || name.contains("replace_file_content");
                            if writes {
                                // 上游写的是 arguments，Antigravity 自己的 transcript 是 args
                                let args = call
                                    .get("arguments")
                                    .or_else(|| call.get("args"))
                                    .and_then(|x| x.as_object());
                                let wants_feedback = args
                                    .and_then(|a| a.get("ArtifactMetadata"))
                                    .and_then(|m| m.get("RequestFeedback"))
                                    .and_then(|b| b.as_bool())
                                    .unwrap_or(false);
                                if wants_feedback {
                                    return ("waiting".into(), Some("等待确认".into()));
                                }
                            }
                        }
                        if !is_turn_over {
                            return ("busy".into(), None);
                        }
                    }
                    _ => is_turn_over = true,
                }
            }
            "EPHEMERAL_MESSAGE" | "CONVERSATION_HISTORY" | "KNOWLEDGE_ARTIFACTS" | "CHECKPOINT" => {}
            _ => {
                // 工具返回（GENERIC 等）：只要回合还没结束，就是在跑
                if !is_turn_over {
                    return ("busy".into(), None);
                }
            }
        }
    }

    ("idle".into(), None)
}

fn antigravity_activity(now: u64) -> Option<Activity> {
    let (path, modified) = ag_newest_transcript()?;
    let (mut state, mut waiting_for) = tail(&path, 64 * 1024)
        .map(|t| ag_parse_state(&t))
        .unwrap_or(("idle".into(), None));
    let age = now.saturating_sub(modified);

    // 只有最近写过的 transcript 才算数：更旧的是已经结束的回合，
    // 把它当成「正在进行」就是拿猜测当事实。
    match state.as_str() {
        "idle" => {
            if age <= AG_SUCCESS_MS {
                state = "success".into();
            } else {
                return None;
            }
        }
        "busy" => {
            if age > AG_BUSY_EXPIRE_MS {
                return None;
            }
        }
        // waiting 不过期：它就停在那里等你
        _ => {
            if waiting_for.is_none() {
                waiting_for = Some("等待输入".into());
            }
        }
    }

    let detail = match state.as_str() {
        "busy" => "工作中".to_string(),
        "waiting" => waiting_for.unwrap_or_else(|| "等待输入".into()),
        _ => "已完成".to_string(),
    };

    Some(Activity {
        provider: "antigravity".into(),
        state,
        name: "Antigravity".into(),
        detail,
        since: modified,
    })
}

// ---------------------------------------------------------------------- Codex

/// 最新被写过的 rollout（`~/.codex/sessions/**/rollout-*.jsonl`）。
fn codex_newest_rollout() -> Option<(PathBuf, u64)> {
    let root = home()?.join(".codex").join("sessions");
    let mut newest: Option<(PathBuf, u64)> = None;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let is_rollout = p
                .file_name()
                .map(|n| n.to_string_lossy().starts_with("rollout-"))
                .unwrap_or(false);
            if !is_rollout {
                continue;
            }
            let Some(m) = mtime_ms(&p) else { continue };
            if newest.as_ref().map(|(_, n)| m > *n).unwrap_or(true) {
                newest = Some((p, m));
            }
        }
    }
    newest
}

/// rollout 尾部最后一条有意义的记录给出的「现在进行到哪一步」
/// （`codex_last_step` 的移植：最后一步是什么类型，决定安静的容忍时长）。
#[derive(Clone, Copy, PartialEq, Debug)]
enum CodexStep {
    /// 工具正在跑（或在等你批准）
    Tool,
    /// 模型在想下一步
    Thinking,
    /// 助手消息：可能是最终答案，也可能是过程叙述
    AsstMsg,
    /// 回合已结束/被中断
    Aborted,
}

/// 只看最后一条有意义的记录，其余（token_count 之类的记账行）跳过。
fn codex_last_step(tail_text: &str) -> Option<CodexStep> {
    for line in tail_text.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let payload = v.get("payload").cloned().unwrap_or(serde_json::Value::Null);
        let pt = payload.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let step = match kind {
            "turn_context" => Some(CodexStep::Thinking),
            "response_item" => match pt {
                "function_call" | "local_shell_call" | "custom_tool_call" | "web_search_call" => {
                    Some(CodexStep::Tool)
                }
                "function_call_output" | "custom_tool_call_output" | "reasoning" => {
                    Some(CodexStep::Thinking)
                }
                "message" => match payload.get("role").and_then(|x| x.as_str()).unwrap_or("") {
                    "assistant" => Some(CodexStep::AsstMsg),
                    "user" => Some(CodexStep::Thinking),
                    _ => None, // system/developer 说不出状态
                },
                _ => None,
            },
            "event_msg" => match pt {
                "turn_aborted" | "task_complete" => Some(CodexStep::Aborted),
                "task_started" | "item_started" | "exec_command_begin" => Some(CodexStep::Thinking),
                "user_message" => Some(CodexStep::Thinking),
                "agent_message" => Some(CodexStep::AsstMsg),
                "agent_reasoning" | "agent_reasoning_raw_content" => Some(CodexStep::Thinking),
                _ => None,
            },
            _ => None,
        };
        if let Some(st) = step {
            return Some(st);
        }
    }
    None
}

fn codex_activity(now: u64) -> Option<Activity> {
    let (path, modified) = codex_newest_rollout()?;
    let tail_text = tail(&path, 256 * 1024)?;
    // 安静多久还算在跑，按最后一步是什么来定：工具调用可能等很久（≤10 分钟），
    // 模型思考给 120s，助手消息只有 4s（多半已经答完了）。分不清「在想」还是
    // 「刚结束」时往短里报——宁可少报，也不编一个「工作中」。
    let quiet = now.saturating_sub(modified);
    let busy = match codex_last_step(&tail_text) {
        Some(CodexStep::Tool) => quiet <= 10 * 60_000,
        Some(CodexStep::Thinking) => quiet <= 120_000,
        Some(CodexStep::AsstMsg) => quiet <= 4_000,
        Some(CodexStep::Aborted) | None => false,
    };
    if !busy {
        return None;
    }
    Some(Activity {
        provider: "codex".into(),
        state: "busy".into(),
        name: "Codex".into(),
        detail: "工作中".into(),
        since: modified,
    })
}

// ----------------------------------------------------------------------- Grok

/// `~/.grok/sessions/<percent-encoded-cwd>/<session-id>/updates.jsonl`
fn grok_updates_paths() -> Vec<PathBuf> {
    let Some(root) = home().map(|h| h.join(".grok").join("sessions")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let Ok(folders) = std::fs::read_dir(&root) else {
        return out;
    };
    for folder in folders.flatten() {
        let Ok(sessions) = std::fs::read_dir(folder.path()) else {
            continue;
        };
        for s in sessions.flatten() {
            let updates = s.path().join("updates.jsonl");
            if updates.is_file() {
                out.push(updates);
            }
        }
    }
    out
}

/// updates.jsonl 尾部是否还留着未结束的回合（`turnIsOpen` 的移植）。
///
/// hook 是记账不是回合本身：Grok 在回合中途和 turn_completed 之后都会打 stop hook，
/// 所以跳过。被尾部窗口截断的片段也不算记录。什么都没有时按「还开着」处理——
/// 一个活着的 TUI 会话刚写过文件，就是还在跑。
fn grok_turn_is_open(tail_text: &str) -> bool {
    for line in tail_text.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(kind) = v
            .get("params")
            .and_then(|p| p.get("update"))
            .and_then(|u| u.get("sessionUpdate"))
            .and_then(|x| x.as_str())
        else {
            continue;
        };
        if kind == "hook_execution" {
            continue;
        }
        return kind != "turn_completed";
    }
    true
}

fn grok_activity(now: u64) -> Option<Activity> {
    let newest = grok_updates_paths()
        .into_iter()
        .filter_map(|p| mtime_ms(&p).map(|m| (p, m)))
        .max_by_key(|(_, m)| *m)?;
    let (path, modified) = newest;
    // 刚写过 = 这一轮可能还在跑；Grok 没有状态字段，这是它唯一诚实的信号
    if now.saturating_sub(modified) > GROK_STALE_MS {
        return None;
    }
    // 再核一次回合是否真的还开着：最后一条记录若是 turn_completed，就是已经答完了
    if let Some(text) = tail(&path, 64 * 1024) {
        if !grok_turn_is_open(&text) {
            return None;
        }
    }
    Some(Activity {
        provider: "grok".into(),
        state: "busy".into(),
        name: "Grok".into(),
        detail: "工作中".into(),
        since: modified,
    })
}

// ------------------------------------------------------------------- 汇总/广播

fn read_all(now: u64) -> Vec<Activity> {
    let mut out = Vec::new();
    if let Some(a) = antigravity_activity(now) {
        out.push(a);
    }
    if let Some(a) = codex_activity(now) {
        out.push(a);
    }
    if let Some(a) = grok_activity(now) {
        out.push(a);
    }
    out
}

/// 每 2s 探测一次，只在变化时广播。
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // 先睡再扫：原来循环体先执行，启动瞬间就叠加一次文件系统扫描
        std::thread::sleep(INITIAL_DELAY);
        let mut last: Vec<Activity> = Vec::new();
        loop {
            let found = read_all(now_ms());
            if found != last {
                // 状态变化记一行（上限 200 条，免得常年运行把日志写满）
                static LOGGED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                if LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 200 {
                    crate::applog::log(&format!(
                        "activity: {}",
                        found
                            .iter()
                            .map(|a| format!("{}={}", a.provider, a.state))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }
                last = found.clone();
                let _ = app.emit("activity", &found);
            }
            std::thread::sleep(INTERVAL);
        }
    });
}

/// 前端挂载时先拿一份当前状态，避免等下一次变化才显示。
#[tauri::command]
pub fn get_activity() -> Vec<Activity> {
    read_all(now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ag_turn_over_reads_idle() {
        let t = r#"{"type":"PLANNER_RESPONSE","tool_calls":[{"name":"run_command","args":{}}]}
{"type":"GENERIC","content":"ok"}
{"type":"PLANNER_RESPONSE","tool_calls":[]}"#;
        assert_eq!(ag_parse_state(t).0, "idle");
    }

    #[test]
    fn ag_tool_call_reads_busy() {
        let t = r#"{"type":"USER_INPUT","content":"do it"}
{"type":"PLANNER_RESPONSE","tool_calls":[{"name":"run_command","args":{"CommandLine":"ls"}}]}"#;
        assert_eq!(ag_parse_state(t).0, "busy");
    }

    #[test]
    fn ag_ask_question_reads_waiting() {
        let t = r#"{"type":"PLANNER_RESPONSE","tool_calls":[{"name":"ask_question","args":{}}]}"#;
        assert_eq!(ag_parse_state(t).0, "waiting");
    }

    #[test]
    fn ag_write_needing_feedback_reads_waiting() {
        let t = r#"{"type":"PLANNER_RESPONSE","tool_calls":[{"name":"write_to_file","args":{"ArtifactMetadata":{"RequestFeedback":true}}}]}"#;
        assert_eq!(ag_parse_state(t).0, "waiting");
    }

    #[test]
    fn grok_open_turn_only() {
        let running = r#"{"params":{"update":{"sessionUpdate":"agent_message_chunk"}}}"#;
        let done = r#"{"params":{"update":{"sessionUpdate":"agent_message_chunk"}}}
{"params":{"update":{"sessionUpdate":"turn_completed"}}}"#;
        // hook 是记账，跳过它继续往前找
        let hook_after_done = r#"{"params":{"update":{"sessionUpdate":"turn_completed"}}}
{"params":{"update":{"sessionUpdate":"hook_execution"}}}"#;
        assert!(grok_turn_is_open(running));
        assert!(!grok_turn_is_open(done));
        assert!(!grok_turn_is_open(hook_after_done));
        // 截断的尾部：什么记录都没有时按「还开着」处理
        assert!(grok_turn_is_open(""));
    }

    #[test]
    fn codex_last_step_classifies() {
        // rollout 按时间追加，最后一行才是最新的（解析器从后往前找）
        let tool = r#"{"type":"response_item","payload":{"type":"function_call"}}"#;
        let thinking = r#"{"type":"response_item","payload":{"type":"reasoning"}}"#;
        let asst = r#"{"type":"response_item","payload":{"type":"message","role":"assistant"}}"#;
        let done = r#"{"type":"event_msg","payload":{"type":"task_started"}}
{"type":"event_msg","payload":{"type":"task_complete"}}"#;
        assert_eq!(codex_last_step(tool), Some(CodexStep::Tool));
        assert_eq!(codex_last_step(thinking), Some(CodexStep::Thinking));
        assert_eq!(codex_last_step(asst), Some(CodexStep::AsstMsg));
        assert_eq!(codex_last_step(done), Some(CodexStep::Aborted));
        // 记账行说不出状态，继续往前找
        let bookkeeping_then_tool = r#"{"type":"response_item","payload":{"type":"function_call"}}
{"type":"event_msg","payload":{"type":"token_count"}}"#;
        assert_eq!(codex_last_step(bookkeeping_then_tool), Some(CodexStep::Tool));
        assert_eq!(codex_last_step(""), None);
    }
}
