use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use serde_json::Value;
use crate::env_resolver::execute_cmd;
use crate::provider_models::{
    ProviderAccountInfo, ProviderPlanGroup, ProviderQuotaPeriod, ProviderUsageData,
};

static CODEX_CACHE: Mutex<Option<(Instant, ProviderUsageData)>> = Mutex::new(None);

pub struct CodexAuthData {
    pub access_token: Option<String>,
    pub account_id: Option<String>,
    pub api_key: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
}

fn get_codex_home() -> Option<PathBuf> {
    if let Ok(h) = env::var("CODEX_HOME") {
        let p = PathBuf::from(h);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(home) = env::var("HOME") {
        let p = PathBuf::from(home).join(".codex");
        if p.exists() {
            return Some(p);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            let p = PathBuf::from(userprofile).join(".codex");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

pub fn get_codex_auth(custom_token: Option<&str>) -> CodexAuthData {
    if let Some(tok) = custom_token {
        let trimmed = tok.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with("sk-") {
                return CodexAuthData {
                    access_token: None,
                    account_id: None,
                    api_key: Some(trimmed.to_string()),
                    email: None,
                    plan_type: None,
                };
            } else {
                return CodexAuthData {
                    access_token: Some(trimmed.to_string()),
                    account_id: None,
                    api_key: None,
                    email: None,
                    plan_type: None,
                };
            }
        }
    }

    let mut auth_data = CodexAuthData {
        access_token: None,
        account_id: None,
        api_key: None,
        email: None,
        plan_type: None,
    };

    // 1. Read ~/.codex/auth.json
    if let Some(codex_dir) = get_codex_home() {
        let auth_file = codex_dir.join("auth.json");
        if auth_file.exists() {
            if let Ok(content) = std::fs::read_to_string(auth_file) {
                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                    if let Some(at) = val.get("access_token").and_then(|t| t.as_str()) {
                        auth_data.access_token = Some(at.to_string());
                    }
                    if let Some(acc) = val.get("chatgpt_account_id").and_then(|t| t.as_str()) {
                        auth_data.account_id = Some(acc.to_string());
                    }
                    if let Some(key) = val.get("OPENAI_API_KEY").and_then(|k| k.as_str()) {
                        auth_data.api_key = Some(key.to_string());
                    }
                    if let Some(em) = val.get("email").and_then(|e| e.as_str()) {
                        auth_data.email = Some(em.to_string());
                    }
                }
            }
        }
    }

    // 2. Fallback: check ~/.antigravity_cockpit/codex_accounts.json
    if auth_data.access_token.is_none() {
        if let Ok(home) = env::var("HOME") {
            let cp_file = PathBuf::from(home).join(".antigravity_cockpit").join("codex_accounts.json");
            if cp_file.exists() {
                if let Ok(content) = std::fs::read_to_string(cp_file) {
                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        if let Some(accs) = val.get("accounts").and_then(|a| a.as_array()) {
                            if let Some(first) = accs.first() {
                                auth_data.email = first.get("email").and_then(|e| e.as_str()).map(|s| s.to_string());
                                auth_data.plan_type = first.get("plan_type").and_then(|p| p.as_str()).map(|s| s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    auth_data
}

fn try_read_local_cached_usage() -> Option<ProviderUsageData> {
    let mut cache_path = None;
    if let Ok(home) = env::var("HOME") {
        cache_path = Some(PathBuf::from(home).join(".ping-island").join("cache").join("codex-usage.json"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            cache_path = Some(PathBuf::from(userprofile).join(".ping-island").join("cache").join("codex-usage.json"));
        }
    }

    if let Some(p) = cache_path {
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                    let plan_type = val.get("planType").and_then(|p| p.as_str()).unwrap_or("free");
                    let mut periods = Vec::new();
                    let mut primary_session_percent = None;

                    if let Some(windows) = val.get("windows").and_then(|w| w.as_array()) {
                        for w in windows {
                            let label = w.get("label").and_then(|l| l.as_str()).unwrap_or("window");
                            let used_pct = w.get("usedPercentage").and_then(|u| u.as_f64()).unwrap_or(0.0);
                            let left_pct = w.get("leftPercentage").and_then(|l| l.as_f64()).unwrap_or(100.0 - used_pct);
                            let resets_at = w.get("resetsAt").and_then(|r| r.as_i64()).map(|ts| {
                                format!("{}秒", ts)
                            });

                            if primary_session_percent.is_none() {
                                primary_session_percent = Some(used_pct);
                            }

                            periods.push(ProviderQuotaPeriod {
                                label: label.to_string(),
                                name: format!("周期限额 ({})", label),
                                used_percent: used_pct,
                                remaining_percent: left_pct,
                                reset_at: resets_at,
                                used: None,
                                total: None,
                                description: Some("本地已记录的配额快照".to_string()),
                            });
                        }
                    }

                    if !periods.is_empty() {
                        return Some(ProviderUsageData {
                            provider: "codex".to_string(),
                            provider_name: "OpenAI Codex".to_string(),
                            icon: "🤖".to_string(),
                            is_connected: true,
                            status_message: Some("读取本地配额快照".to_string()),
                            error_message: None,
                            account_info: Some(ProviderAccountInfo {
                                user_name: None,
                                email: None,
                                account_id: None,
                                plan_name: Some(format!("Codex {}", plan_type)),
                            }),
                            groups: vec![ProviderPlanGroup {
                                group_name: "Codex CLI".to_string(),
                                edition: Some(format!("Codex {}", plan_type)),
                                periods,
                            }],
                            primary_session_percent,
                            primary_reset_at: None,
                            console_url: Some("https://chatgpt.com".to_string()),
                        });
                    }
                }
            }
        }
    }
    None
}

pub fn get_codex_usage(custom_token: Option<&str>) -> ProviderUsageData {
    if custom_token.is_none() {
        if let Ok(guard) = CODEX_CACHE.lock() {
            if let Some((cached_at, ref data)) = *guard {
                if cached_at.elapsed() < Duration::from_secs(60) {
                    return data.clone();
                }
            }
        }
    }

    let auth = get_codex_auth(custom_token);

    // 1. If we have access_token, query https://chatgpt.com/backend-api/wham/usage
    if let Some(token) = &auth.access_token {
        let mut curl_args = vec![
            "-s",
            "--max-time",
            "6",
            "-H",
        ];
        let auth_header = format!("Authorization: Bearer {}", token);
        curl_args.push(&auth_header);

        let acc_header = auth.account_id.as_ref().map(|acc| format!("ChatGPT-Account-ID: {}", acc));
        if let Some(ref h) = acc_header {
            curl_args.push("-H");
            curl_args.push(h);
        }

        curl_args.push("-H");
        curl_args.push("User-Agent: codex-cli/0.1.0");
        curl_args.push("https://chatgpt.com/backend-api/wham/usage");

        if let Ok(output) = execute_cmd("curl", &curl_args) {
            if output.status.success() {
                let body = String::from_utf8_lossy(&output.stdout);
                if let Ok(json_val) = serde_json::from_str::<Value>(&body) {
                    if json_val.get("error").is_none() {
                        let plan_type = json_val.get("plan_type").and_then(|p| p.as_str()).unwrap_or("Plus");
                        let mut periods = Vec::new();
                        let mut primary_session_percent = None;
                        let mut primary_reset_at = None;

                        // 5-hour window
                        if let (Some(used), Some(remaining)) = (
                            json_val.get("quota5hUsed").and_then(|v| v.as_f64()),
                            json_val.get("quota5hRemaining").and_then(|v| v.as_f64()),
                        ) {
                            let total = used + remaining;
                            let used_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };
                            let rem_pct = if total > 0.0 { (remaining / total) * 100.0 } else { 100.0 };
                            let reset_at = json_val.get("quota5hReset").and_then(|r| r.as_str()).map(|s| s.to_string());

                            primary_session_percent = Some(used_pct);
                            primary_reset_at = reset_at.clone();

                            periods.push(ProviderQuotaPeriod {
                                label: "session".to_string(),
                                name: "5小时限额".to_string(),
                                used_percent: (used_pct * 10.0).round() / 10.0,
                                remaining_percent: (rem_pct * 10.0).round() / 10.0,
                                reset_at,
                                used: Some(used),
                                total: Some(total),
                                description: Some("5小时滚动消息/计算窗口".to_string()),
                            });
                        }

                        // Weekly window
                        if let (Some(used), Some(remaining)) = (
                            json_val.get("quota7dUsed").and_then(|v| v.as_f64()),
                            json_val.get("quota7dRemaining").and_then(|v| v.as_f64()),
                        ) {
                            let total = used + remaining;
                            let used_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };
                            let rem_pct = if total > 0.0 { (remaining / total) * 100.0 } else { 100.0 };
                            let reset_at = json_val.get("quota7dReset").and_then(|r| r.as_str()).map(|s| s.to_string());

                            periods.push(ProviderQuotaPeriod {
                                label: "weekly".to_string(),
                                name: "周度限额".to_string(),
                                used_percent: (used_pct * 10.0).round() / 10.0,
                                remaining_percent: (rem_pct * 10.0).round() / 10.0,
                                reset_at,
                                used: Some(used),
                                total: Some(total),
                                description: Some("7天用量池与重置周期".to_string()),
                            });
                        }

                        if !periods.is_empty() {
                            let result = ProviderUsageData {
                                provider: "codex".to_string(),
                                provider_name: "OpenAI Codex".to_string(),
                                icon: "🤖".to_string(),
                                is_connected: true,
                                status_message: Some("在线同步成功".to_string()),
                                error_message: None,
                                account_info: Some(ProviderAccountInfo {
                                    user_name: None,
                                    email: auth.email,
                                    account_id: auth.account_id,
                                    plan_name: Some(format!("Codex {}", plan_type)),
                                }),
                                groups: vec![ProviderPlanGroup {
                                    group_name: "Codex CLI / ChatGPT".to_string(),
                                    edition: Some(format!("{} 订阅", plan_type)),
                                    periods,
                                }],
                                primary_session_percent,
                                primary_reset_at,
                                console_url: Some("https://chatgpt.com".to_string()),
                            };
                            if custom_token.is_none() {
                                if let Ok(mut guard) = CODEX_CACHE.lock() {
                                    *guard = Some((Instant::now(), result.clone()));
                                }
                            }
                            return result;
                        }
                    }
                }
            }
        }
    }

    // 2. Try local cached usage
    if let Some(cached) = try_read_local_cached_usage() {
        if custom_token.is_none() {
            if let Ok(mut guard) = CODEX_CACHE.lock() {
                *guard = Some((Instant::now(), cached.clone()));
            }
        }
        return cached;
    }

    // 3. Fallback: return not connected with clear guidance
    let has_token = auth.access_token.is_some() || auth.api_key.is_some();
    ProviderUsageData {
        provider: "codex".to_string(),
        provider_name: "OpenAI Codex".to_string(),
        icon: "🤖".to_string(),
        is_connected: false,
        status_message: if has_token {
            Some("Codex 会话令牌已失效或未授权".to_string())
        } else {
            Some("未检测到 Codex 登录凭证".to_string())
        },
        error_message: Some("请在终端运行 codex login，或在设置中输入 ChatGPT Session Token / API Key".to_string()),
        account_info: Some(ProviderAccountInfo {
            user_name: None,
            email: auth.email,
            account_id: auth.account_id,
            plan_name: auth.plan_type.or(Some("OpenAI Codex".to_string())),
        }),
        groups: Vec::new(),
        primary_session_percent: None,
        primary_reset_at: None,
        console_url: Some("https://chatgpt.com".to_string()),
    }
}
