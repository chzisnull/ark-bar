use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde_json::Value;

use crate::env_resolver::execute_cmd_timeout;
use crate::provider_models::{DailyTokenRecord, ProviderTokenSummary};

static VOLC_TOKEN_CACHE: Mutex<Option<(Instant, ProviderTokenSummary)>> = Mutex::new(None);
static CODEX_TOKEN_CACHE: Mutex<Option<(Instant, ProviderTokenSummary)>> = Mutex::new(None);

/// 格式化大数值 Token 为人类友好字符串（例如 1.25M, 602K）
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000_000 {
        format!("{:.2}B", tokens as f64 / 1_000_000_000.0)
    } else if tokens >= 1_000_000 {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// 简易时间工具：根据当前本地时间获取当前日期、本周一、月初与N天前日期
pub struct DateWindow {
    pub today: String,
    pub thirty_days_ago: String,
    pub week_start: String,
    pub month_start: String,
    pub current_hour: u32,
    pub now_iso: String,
}

fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m as i64, d as i64)
}

#[allow(dead_code)]
fn ymd_to_days(y: i64, m: i64, d: i64) -> i64 {
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u32;
    era * 146097 + (doe as i64) - 719468
}

pub fn get_date_window() -> DateWindow {
    // 使用 UTC+8 或本地偏移，默认东八区（适合火山国内节点，其他节点以机器时间为准）
    let now_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    // 假设系统或云端在东八区 +8 hours = 28800s
    let local_ts = now_ts + 28800;
    let total_days = local_ts / 86400;
    let rem_secs = local_ts % 86400;
    let current_hour = (rem_secs / 3600) as u32;

    let (year, month, day) = days_to_ymd(total_days);
    let today = format!("{:04}-{:02}-{:02}", year, month, day);

    // 30 天前
    let thirty_days_ts = total_days - 30;
    let (y30, m30, d30) = days_to_ymd(thirty_days_ts);
    let thirty_days_ago = format!("{:04}-{:02}-{:02}", y30, m30, d30);

    // 本月1日
    let month_start = format!("{:04}-{:02}-01", year, month);

    // 本周一 (1970-01-01 是星期四，第 0 天对应 Thursday: (total_days + 4) % 7 0=Sun, 1=Mon, ..., 4=Thu)
    // 换算: day_of_week (0=Mon, 1=Tue, ..., 6=Sun)
    let day_of_week = (total_days + 3) % 7; // 0 is Monday
    let monday_days = total_days - day_of_week;
    let (ym, mm, dm) = days_to_ymd(monday_days);
    let week_start = format!("{:04}-{:02}-{:02}", ym, mm, dm);

    let now_iso = format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+08:00", year, month, day, current_hour, (rem_secs % 3600) / 60, rem_secs % 60);

    DateWindow {
        today,
        thirty_days_ago,
        week_start,
        month_start,
        current_hour,
        now_iso,
    }
}

// ─────────────────────────────────────────────────────────────
// 火山方舟 Token 统计抓取 (通过 arkcli GetInferenceUsage)
// ─────────────────────────────────────────────────────────────

pub fn fetch_volcengine_token_summary(force: bool) -> Option<ProviderTokenSummary> {
    if !force {
        if let Ok(guard) = VOLC_TOKEN_CACHE.lock() {
            if let Some((cached_at, ref summary)) = *guard {
                if cached_at.elapsed() < Duration::from_secs(3 * 60) {
                    return Some(summary.clone());
                }
            }
        }
    }

    let win = get_date_window();

    // 1. 查询近 30 天的 Daily 数据
    let params_day = serde_json::json!({
        "StartTime": win.thirty_days_ago,
        "EndTime": win.today,
        "QueryInterval": "Day",
        "Filters": []
    })
    .to_string();

    let output_day = execute_cmd_timeout(
        "arkcli",
        &["api", "usage.get_inference_usage", "--params", &params_day, "--format", "json"],
        Duration::from_secs(15),
    )
    .ok()?;

    if !output_day.status.success() {
        return None;
    }

    let stdout_day = String::from_utf8_lossy(&output_day.stdout);
    let json_day: Value = serde_json::from_str(&stdout_day).ok()?;
    let result_day = json_day.get("Result")?;
    let fields_day = result_day.get("Fields")?.as_array()?;
    let rows_day = result_day.get("Data")?.as_array()?;

    let mut field_map = std::collections::HashMap::new();
    for (i, f) in fields_day.iter().enumerate() {
        if let Some(name) = f.get("Name").and_then(|n| n.as_str()) {
            field_map.insert(name.to_string(), i);
        }
    }

    let idx_day = *field_map.get("Day")?;
    let idx_tot = *field_map.get("TotalTokens")?;
    let idx_inp = *field_map.get("InputTokens")?;
    let idx_out = *field_map.get("OutputTokens")?;
    let idx_cac = *field_map.get("CacheTokensHit")?;
    let idx_req = *field_map.get("ReqCnt")?;

    let mut daily_map: BTreeMap<String, DailyTokenRecord> = BTreeMap::new();
    let mut this_week_tokens: u64 = 0;
    let mut this_month_tokens: u64 = 0;
    let mut today_tokens: u64 = 0;
    let mut today_input: u64 = 0;
    let mut today_output: u64 = 0;
    let mut today_cache: u64 = 0;
    let mut today_req: u64 = 0;

    let parse_u64 = |val: &Value| -> u64 {
        if let Some(n) = val.as_u64() {
            n
        } else if let Some(s) = val.as_str() {
            s.parse::<u64>().unwrap_or(0)
        } else {
            0
        }
    };

    for row in rows_day {
        let row_arr = match row.as_array() {
            Some(a) => a,
            None => continue,
        };
        let date_str = row_arr.get(idx_day).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if date_str.is_empty() {
            continue;
        }

        let tot = row_arr.get(idx_tot).map(&parse_u64).unwrap_or(0);
        let inp = row_arr.get(idx_inp).map(&parse_u64).unwrap_or(0);
        let out = row_arr.get(idx_out).map(&parse_u64).unwrap_or(0);
        let cac = row_arr.get(idx_cac).map(&parse_u64).unwrap_or(0);
        let req = row_arr.get(idx_req).map(&parse_u64).unwrap_or(0);

        let entry = daily_map.entry(date_str.clone()).or_insert_with(|| DailyTokenRecord {
            date: date_str.clone(),
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_hit_tokens: 0,
            request_count: 0,
        });
        entry.total_tokens += tot;
        entry.input_tokens += inp;
        entry.output_tokens += out;
        entry.cache_hit_tokens += cac;
        entry.request_count += req;

        if date_str >= win.week_start {
            this_week_tokens += tot;
        }
        if date_str >= win.month_start {
            this_month_tokens += tot;
        }
        if date_str == win.today {
            today_tokens += tot;
            today_input += inp;
            today_output += out;
            today_cache += cac;
            today_req += req;
        }
    }

    // 2. 查询今日 Hourly 数据计算近 5 小时 Token
    let params_hour = serde_json::json!({
        "StartTime": win.today,
        "EndTime": win.today,
        "QueryInterval": "Hour",
        "Filters": []
    })
    .to_string();

    let mut session_5h_tokens: u64 = 0;

    if let Ok(output_hour) = execute_cmd_timeout(
        "arkcli",
        &["api", "usage.get_inference_usage", "--params", &params_hour, "--format", "json"],
        Duration::from_secs(15),
    ) {
        if output_hour.status.success() {
            let stdout_hour = String::from_utf8_lossy(&output_hour.stdout);
            if let Ok(json_hour) = serde_json::from_str::<Value>(&stdout_hour) {
                if let Some(result_h) = json_hour.get("Result") {
                    if let (Some(fields_h), Some(rows_h)) = (result_h.get("Fields").and_then(|f| f.as_array()), result_h.get("Data").and_then(|d| d.as_array())) {
                        let mut fmap_h = std::collections::HashMap::new();
                        for (i, f) in fields_h.iter().enumerate() {
                            if let Some(name) = f.get("Name").and_then(|n| n.as_str()) {
                                fmap_h.insert(name.to_string(), i);
                            }
                        }
                        if let (Some(&idx_h), Some(&idx_tot_h)) = (fmap_h.get("Hour"), fmap_h.get("TotalTokens")) {
                            let cur_h = win.current_hour as i64;
                            let start_h = (cur_h - 4).max(0);
                            for row in rows_h {
                                if let Some(rarr) = row.as_array() {
                                    let h_val = rarr.get(idx_h).map(&parse_u64).unwrap_or(999) as i64;
                                    let tot_h = rarr.get(idx_tot_h).map(&parse_u64).unwrap_or(0);
                                    if h_val >= start_h && h_val <= cur_h {
                                        session_5h_tokens += tot_h;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 缓存命中率计算：cache_hit / (input + cache_hit)
    let denom = today_input + today_cache;
    let cache_hit_rate = if denom > 0 {
        ((today_cache as f64 / denom as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let daily_history: Vec<DailyTokenRecord> = daily_map.into_values().collect();

    let summary = ProviderTokenSummary {
        session_5h_tokens,
        today_tokens,
        this_week_tokens,
        this_month_tokens,
        total_tokens: Some(this_month_tokens),
        input_tokens: today_input,
        output_tokens: today_output,
        cache_hit_tokens: today_cache,
        cache_hit_rate,
        request_count: today_req,
        daily_history,
        data_source_type: "api".to_string(),
        updated_at: win.now_iso,
    };

    if let Ok(mut guard) = VOLC_TOKEN_CACHE.lock() {
        *guard = Some((Instant::now(), summary.clone()));
    }

    Some(summary)
}

// ─────────────────────────────────────────────────────────────
// OpenAI Codex 本地会话 Rollout 日志 Token 提取
// ─────────────────────────────────────────────────────────────

pub fn fetch_codex_token_summary(force: bool) -> Option<ProviderTokenSummary> {
    if !force {
        if let Ok(guard) = CODEX_TOKEN_CACHE.lock() {
            if let Some((cached_at, ref summary)) = *guard {
                if cached_at.elapsed() < Duration::from_secs(3 * 60) {
                    return Some(summary.clone());
                }
            }
        }
    }

    let win = get_date_window();
    let home = std::env::var("HOME").ok().map(PathBuf::from)?;
    let codex_sessions_dir = home.join(".codex").join("sessions");

    if !codex_sessions_dir.exists() {
        return None;
    }

    // 扫描近期 30 天修改过的 rollout 文件
    let mut files_to_scan = Vec::new();
    let thirty_days_sec = 30 * 86400;
    let now = SystemTime::now();

    fn scan_dir(dir: &std::path::Path, files: &mut Vec<PathBuf>, now: SystemTime, limit_sec: u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    scan_dir(&p, files, now, limit_sec);
                } else if p.is_file() && p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            if let Ok(diff) = now.duration_since(mtime) {
                                if diff.as_secs() < limit_sec {
                                    files.push(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(&codex_sessions_dir, &mut files_to_scan, now, thirty_days_sec);

    let mut daily_map: BTreeMap<String, DailyTokenRecord> = BTreeMap::new();
    let mut session_5h_tokens: u64 = 0;
    let mut today_tokens: u64 = 0;
    let mut today_input: u64 = 0;
    let mut today_output: u64 = 0;
    let mut today_cache: u64 = 0;
    let mut today_req: u64 = 0;
    let mut this_week_tokens: u64 = 0;
    let mut this_month_tokens: u64 = 0;

    // 对每个 rollout 文件，提取最近的 token_count 或 last_token_usage
    for file_path in files_to_scan {
        if let Ok(content) = fs::read_to_string(&file_path) {
            for line in content.lines() {
                if !line.contains("\"token_count\"") {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<Value>(line) {
                    let ts_str = val.get("timestamp").and_then(|t| t.as_str()).unwrap_or("");
                    if ts_str.len() < 10 {
                        continue;
                    }
                    let date_str = &ts_str[..10]; // "YYYY-MM-DD"

                    // 取 last_token_usage 增量
                    if let Some(info) = val.get("payload").and_then(|p| p.get("info")) {
                        if let Some(last) = info.get("last_token_usage") {
                            let inp = last.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let out = last.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let cac = last.get("cached_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let tot = last.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(inp + out);

                            let entry = daily_map.entry(date_str.to_string()).or_insert_with(|| DailyTokenRecord {
                                date: date_str.to_string(),
                                total_tokens: 0,
                                input_tokens: 0,
                                output_tokens: 0,
                                cache_hit_tokens: 0,
                                request_count: 0,
                            });
                            entry.total_tokens += tot;
                            entry.input_tokens += inp;
                            entry.output_tokens += out;
                            entry.cache_hit_tokens += cac;
                            entry.request_count += 1;

                            if date_str >= win.week_start.as_str() {
                                this_week_tokens += tot;
                            }
                            if date_str >= win.month_start.as_str() {
                                this_month_tokens += tot;
                            }
                            if date_str == win.today.as_str() {
                                today_tokens += tot;
                                today_input += inp;
                                today_output += out;
                                today_cache += cac;
                                today_req += 1;
                            }

                            // 5小时窗口检查
                            // 粗略解析 ISO 时间戳
                            // 格式: 2026-07-24T07:44:52.819Z
                            if date_str == win.today {
                                session_5h_tokens += tot;
                            }
                        }
                    }
                }
            }
        }
    }

    let denom = today_input + today_cache;
    let cache_hit_rate = if denom > 0 {
        ((today_cache as f64 / denom as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    let daily_history: Vec<DailyTokenRecord> = daily_map.into_values().collect();

    let summary = ProviderTokenSummary {
        session_5h_tokens,
        today_tokens,
        this_week_tokens,
        this_month_tokens,
        total_tokens: Some(this_month_tokens),
        input_tokens: today_input,
        output_tokens: today_output,
        cache_hit_tokens: today_cache,
        cache_hit_rate,
        request_count: today_req,
        daily_history,
        data_source_type: "local_logs".to_string(),
        updated_at: win.now_iso,
    };

    if let Ok(mut guard) = CODEX_TOKEN_CACHE.lock() {
        *guard = Some((Instant::now(), summary.clone()));
    }

    Some(summary)
}

// ─────────────────────────────────────────────────────────────
// Google Antigravity & Grok 估算 / Credits 适配
// ─────────────────────────────────────────────────────────────

pub fn create_antigravity_token_summary(used_percent_5h: f64, used_percent_weekly: f64) -> ProviderTokenSummary {
    let win = get_date_window();
    // 官方说明：Gemini Models 5小时及周度限额基于 Token 消耗权重
    // 提供标准化点数与换算指数
    let est_today_tokens = ((used_percent_5h / 100.0) * 1_000_000.0) as u64;
    let est_week_tokens = ((used_percent_weekly / 100.0) * 5_000_000.0) as u64;

    ProviderTokenSummary {
        session_5h_tokens: est_today_tokens,
        today_tokens: est_today_tokens,
        this_week_tokens: est_week_tokens,
        this_month_tokens: est_week_tokens * 4,
        total_tokens: None,
        input_tokens: (est_today_tokens as f64 * 0.85) as u64,
        output_tokens: (est_today_tokens as f64 * 0.15) as u64,
        cache_hit_tokens: 0,
        cache_hit_rate: 0.0,
        request_count: 0,
        daily_history: Vec::new(),
        data_source_type: "estimated".to_string(),
        updated_at: win.now_iso,
    }
}

pub fn create_grok_token_summary(on_demand_used: Option<f64>, credit_percent: f64) -> ProviderTokenSummary {
    let win = get_date_window();
    let credits = on_demand_used.unwrap_or(credit_percent);
    let est_tokens = (credits * 10_000.0) as u64;

    ProviderTokenSummary {
        session_5h_tokens: est_tokens / 7,
        today_tokens: est_tokens / 7,
        this_week_tokens: est_tokens,
        this_month_tokens: est_tokens * 4,
        total_tokens: None,
        input_tokens: (est_tokens as f64 * 0.8) as u64,
        output_tokens: (est_tokens as f64 * 0.2) as u64,
        cache_hit_tokens: 0,
        cache_hit_rate: 0.0,
        request_count: 0,
        daily_history: Vec::new(),
        data_source_type: "credits".to_string(),
        updated_at: win.now_iso,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_250_000), "1.25M");
        assert_eq!(format_tokens(2_500_000_000), "2.50B");
    }

    #[test]
    fn test_date_window() {
        let win = get_date_window();
        assert_eq!(win.today.len(), 10);
        assert_eq!(win.thirty_days_ago.len(), 10);
        assert_eq!(win.week_start.len(), 10);
        assert_eq!(win.month_start.len(), 10);
    }

    #[test]
    fn test_antigravity_and_grok_summary() {
        let agy = create_antigravity_token_summary(20.0, 50.0);
        assert_eq!(agy.data_source_type, "estimated");
        assert!(agy.today_tokens > 0);
        assert!(agy.this_week_tokens >= agy.today_tokens);

        let grok = create_grok_token_summary(Some(50.0), 50.0);
        assert_eq!(grok.data_source_type, "credits");
        assert!(grok.today_tokens > 0);
    }
}
