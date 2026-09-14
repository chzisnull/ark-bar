use std::sync::Mutex;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::env_resolver::{execute_cmd, invalidate_arkcli_cache};
use crate::provider_models::{
    ProviderAccountInfo, ProviderPlanGroup, ProviderQuotaPeriod, ProviderUsageData,
};
use crate::usage_cache::UsageCache;

static VOLC_CACHE: UsageCache = UsageCache::new();
static ENV_CACHE: Mutex<Option<(Instant, EnvironmentStatus)>> = Mutex::new(None);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnvironmentStatus {
    pub has_node: bool,
    pub node_version: Option<String>,
    pub has_npm: bool,
    pub npm_version: Option<String>,
    pub has_arkcli: bool,
    pub arkcli_version: Option<String>,
    pub logged_in: bool,
    pub user_name: Option<String>,
    pub account_id: Option<String>,
    pub active_profile: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub download_url: Option<String>,
}

#[tauri::command]
pub fn check_environment() -> EnvironmentStatus {
    if let Ok(guard) = ENV_CACHE.lock() {
        if let Some((cached_at, ref status)) = *guard {
            if cached_at.elapsed() < Duration::from_secs(45) {
                return status.clone();
            }
        }
    }

    let mut status = EnvironmentStatus {
        has_node: false,
        node_version: None,
        has_npm: false,
        npm_version: None,
        has_arkcli: false,
        arkcli_version: None,
        logged_in: false,
        user_name: None,
        account_id: None,
        active_profile: None,
        error_message: None,
    };

    // 1. Check Node
    if let Ok(output) = execute_cmd("node", &["-v"]) {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status.has_node = true;
            status.node_version = Some(ver);
        }
    }

    // 2. Check npm
    if let Ok(output) = execute_cmd("npm", &["-v"]) {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status.has_npm = true;
            status.npm_version = Some(ver);
        }
    }

    // 3. Check arkcli
    if let Ok(output) = execute_cmd("arkcli", &["-v"]) {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status.has_arkcli = true;
            status.arkcli_version = Some(ver);
        }
    } else if let Ok(output) = execute_cmd("arkcli", &["--version"]) {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            status.has_arkcli = true;
            status.arkcli_version = Some(ver);
        }
    }

    // 4. If arkcli exists, check auth status
    if status.has_arkcli {
        if let Ok(output) = execute_cmd("arkcli", &["auth", "status", "--format", "json"]) {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            if let Ok(json_val) = serde_json::from_str::<Value>(&stdout_str) {
                if let Some(logged_in) = json_val.get("logged_in").and_then(|v| v.as_bool()) {
                    status.logged_in = logged_in;
                }
                if let Some(identity) = json_val.get("volc_sso").and_then(|v| v.get("identity")) {
                    if let Some(name) = identity.get("name").and_then(|v| v.as_str()) {
                        status.user_name = Some(name.to_string());
                    }
                    if let Some(acc) = identity.get("account_id").and_then(|v| v.as_str()) {
                        status.account_id = Some(acc.to_string());
                    }
                }
                if let Some(prof) = json_val.get("active_profile").and_then(|v| v.get("name")).and_then(|v| v.as_str()) {
                    status.active_profile = Some(prof.to_string());
                }
            } else {
                status.error_message = Some("解析 arkcli auth 状态返回失败".to_string());
            }
        }
    }

    if let Ok(mut guard) = ENV_CACHE.lock() {
        *guard = Some((Instant::now(), status.clone()));
    }

    status
}

#[tauri::command]
pub fn install_arkcli() -> CommandResult {
    match execute_cmd("npm", &["install", "-g", "@volcengine/ark-cli@latest"]) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                invalidate_arkcli_cache();
                if let Ok(mut guard) = ENV_CACHE.lock() {
                    *guard = None;
                }
                CommandResult {
                    success: true,
                    message: "arkcli 安装成功".to_string(),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            } else {
                CommandResult {
                    success: false,
                    message: "arkcli 安装失败，请检查网络或 npm 权限".to_string(),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            }
        }
        Err(e) => CommandResult {
            success: false,
            message: format!("无法执行 npm 命令: {}", e),
            details: None,
        },
    }
}

#[tauri::command]
pub fn login_volc_sso() -> CommandResult {
    // Run arkcli auth login volc-sso
    match execute_cmd("arkcli", &["auth", "login", "volc-sso"]) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                CommandResult {
                    success: true,
                    message: "登录授权成功".to_string(),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            } else {
                CommandResult {
                    success: false,
                    message: "登录失败或已超时".to_string(),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            }
        }
        Err(e) => CommandResult {
            success: false,
            message: format!("无法执行 arkcli 登录: {}", e),
            details: None,
        },
    }
}

#[tauri::command]
pub fn get_sso_auth_url() -> Result<String, String> {
    let output = execute_cmd("arkcli", &["auth", "login", "volc-sso", "--no-browser"])
        .map_err(|e| format!("执行 arkcli 出错: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    // 1. Try to find authorize_url in JSON
    for line in combined.lines() {
        if let Ok(json_val) = serde_json::from_str::<Value>(line) {
            if let Some(url) = json_val.get("authorize_url").and_then(|u| u.as_str()) {
                return Ok(url.to_string());
            }
        }
    }

    // 2. Search for https://signin.volcengine.com/authorize/oauth/authorize
    for line in combined.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("https://signin.volcengine.com/authorize/oauth/authorize") {
            return Ok(trimmed.to_string());
        }
    }

    Err(format!("未能解析授权链接，原始输出: {}", combined))
}

#[tauri::command]
pub fn login_with_code(code: String) -> CommandResult {
    let clean_code = code.trim();
    if clean_code.is_empty() {
        return CommandResult {
            success: false,
            message: "授权码不能为空".to_string(),
            details: None,
        };
    }

    match execute_cmd("arkcli", &["auth", "login", "--no-browser", "--code", clean_code]) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                CommandResult {
                    success: true,
                    message: "授权码登录成功！".to_string(),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            } else {
                CommandResult {
                    success: false,
                    message: format!("授权码验证失败: {}", stderr.trim()),
                    details: Some(format!("{}\n{}", stdout, stderr)),
                }
            }
        }
        Err(e) => CommandResult {
            success: false,
            message: format!("无法执行验证命令: {}", e),
            details: None,
        },
    }
}

#[tauri::command]
pub fn get_usage_plan() -> Result<Value, String> {
    match execute_cmd("arkcli", &["usage", "plan", "--format", "json"]) {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str::<Value>(&stdout)
                    .map_err(|e| format!("解析 usage plan JSON 失败: {}, 原始输出: {}", e, stdout))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("执行 usage plan 失败 (exit {}): {}", output.status.code().unwrap_or(-1), stderr))
            }
        }
        Err(e) => Err(format!("无法运行 arkcli: {}", e)),
    }
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

pub fn unix_to_iso8601(ts_sec: i64) -> String {
    let days = ts_sec / 86400;
    let rem_sec = ts_sec % 86400;
    let (year, month, day) = days_to_ymd(days);
    let hour = rem_sec / 3600;
    let minute = (rem_sec % 3600) / 60;
    let second = rem_sec % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hour, minute, second)
}

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct VolcSeatMilestones {
    pub seat_id: Option<String>,
    pub short_term_reset: Option<String>,
    pub weekly_reset: Option<String>,
    pub monthly_reset: Option<String>,
    pub short_term_usage: Option<f64>,
    pub weekly_usage: Option<f64>,
    pub monthly_usage: Option<f64>,
}

static SEAT_CACHE: Mutex<Option<(Instant, VolcSeatMilestones)>> = Mutex::new(None);

pub fn query_volc_seat_usage(known_seat_id: Option<&str>) -> Option<VolcSeatMilestones> {
    query_volc_seat_usage_cached(known_seat_id, false)
}

fn query_volc_seat_usage_cached(known_seat_id: Option<&str>, force: bool) -> Option<VolcSeatMilestones> {
    if !force {
        if let Ok(guard) = SEAT_CACHE.lock() {
            if let Some((cached_at, ref data)) = *guard {
                if cached_at.elapsed() < Duration::from_secs(2 * 60) {
                    return Some(data.clone());
                }
            }
        }
    }

    // 1. Identify SeatID: use known_seat_id or query usage.get_seat_info
    let seat_id = if let Some(sid) = known_seat_id.filter(|s| !s.trim().is_empty()) {
        sid.to_string()
    } else {
        match execute_cmd("arkcli", &["api", "usage.get_seat_info", "--params", "{\"ProjectName\":\"default\"}", "--format", "json"]) {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                serde_json::from_str::<Value>(&stdout)
                    .ok()
                    .and_then(|v| v.get("Result").and_then(|r| r.get("SeatID")).and_then(|s| s.as_str()).map(|s| s.to_string()))
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    };

    // 2. Query usage.get_seat_info_usage with SeatID and ProjectName
    let params = if !seat_id.is_empty() {
        format!("{{\"SeatID\":\"{}\",\"ProjectName\":\"default\"}}", seat_id)
    } else {
        "{\"ProjectName\":\"default\"}".to_string()
    };

    let output = execute_cmd("arkcli", &["api", "usage.get_seat_info_usage", "--params", &params, "--format", "json"]).ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_val = serde_json::from_str::<Value>(&stdout).ok()?;
    let result = json_val.get("Result")?;

    let short_term_reset = result.get("ShortTermResetMilestone")
        .and_then(|v| v.as_i64())
        .filter(|&ts| ts > 0)
        .map(unix_to_iso8601);

    let weekly_reset = result.get("WeeklyResetMilestone")
        .and_then(|v| v.as_i64())
        .filter(|&ts| ts > 0)
        .map(unix_to_iso8601);

    let monthly_reset = result.get("MonthlyResetMilestone")
        .and_then(|v| v.as_i64())
        .filter(|&ts| ts > 0)
        .map(unix_to_iso8601);

    let short_term_usage = result.get("ShortTermUsage").and_then(|v| v.as_f64());
    let weekly_usage = result.get("WeeklyUsage").and_then(|v| v.as_f64());
    let monthly_usage = result.get("MonthlyUsage").and_then(|v| v.as_f64());

    let milestones = VolcSeatMilestones {
        seat_id: if !seat_id.is_empty() { Some(seat_id) } else { None },
        short_term_reset,
        weekly_reset,
        monthly_reset,
        short_term_usage,
        weekly_usage,
        monthly_usage,
    };

    if let Ok(mut guard) = SEAT_CACHE.lock() {
        *guard = Some((Instant::now(), milestones.clone()));
    }

    Some(milestones)
}

pub fn get_volcengine_usage() -> ProviderUsageData {
    get_volcengine_usage_forced(false)
}

pub fn get_volcengine_usage_forced(force: bool) -> ProviderUsageData {
    VOLC_CACHE.get_or_refresh(force, move || fetch_volcengine_usage_uncached(force))
}

pub fn peek_volcengine_usage() -> Option<ProviderUsageData> {
    VOLC_CACHE.peek()
}

fn fetch_volcengine_usage_uncached(force: bool) -> ProviderUsageData {
    match get_usage_plan() {
        Ok(json_val) => {
            let mut groups = Vec::new();
            let mut primary_session_percent = None;
            let mut primary_reset_at = None;

            let known_seat = json_val.get("items")
                .and_then(|i| i.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("seat_id"))
                .and_then(|s| s.as_str());

            // Session/weekly reset timestamps only exist on the seat API.
            // Wait for them here so the UI is not missing countdown badges.
            let seat_milestones = query_volc_seat_usage_cached(known_seat, force);

            if let Some(items) = json_val.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    let product = item.get("product").and_then(|p| p.as_str()).unwrap_or("Coding Plan").to_string();
                    let edition = item.get("edition").and_then(|e| e.as_str()).unwrap_or("Enterprise").to_string();
                    let mut periods = Vec::new();

                    if let Some(pers) = item.get("periods").and_then(|p| p.as_array()) {
                        for p in pers {
                            let label = p.get("label").and_then(|l| l.as_str()).unwrap_or("").to_lowercase();
                            let mut percent = p.get("percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let mut reset_at = p.get("reset_at").and_then(|r| r.as_str()).map(|s| s.to_string());
                            let used = p.get("used").and_then(|u| u.as_f64());
                            let total = p.get("total").and_then(|t| t.as_f64());

                            let name = match label.as_str() {
                                "session" => {
                                    if let Some(ref m) = seat_milestones {
                                        if let Some(rt) = &m.short_term_reset {
                                            reset_at = Some(rt.clone());
                                        }
                                        if let Some(u) = m.short_term_usage {
                                            percent = u;
                                        }
                                    }
                                    "近5小时用量".to_string()
                                }
                                "weekly" => {
                                    if let Some(ref m) = seat_milestones {
                                        if let Some(rt) = &m.weekly_reset {
                                            reset_at = Some(rt.clone());
                                        }
                                        if let Some(u) = m.weekly_usage {
                                            percent = u;
                                        }
                                    }
                                    "近一周用量".to_string()
                                }
                                "monthly" => {
                                    if let Some(ref m) = seat_milestones {
                                        if reset_at.is_none() {
                                            reset_at = m.monthly_reset.clone();
                                        }
                                        if let Some(u) = m.monthly_usage {
                                            percent = u;
                                        }
                                    }
                                    "近一月用量".to_string()
                                }
                                other => other.to_string(),
                            };

                            if label == "session" && primary_session_percent.is_none() {
                                primary_session_percent = Some(percent);
                                primary_reset_at = reset_at.clone();
                            }

                            periods.push(ProviderQuotaPeriod {
                                label,
                                name,
                                used_percent: (percent * 10.0).round() / 10.0,
                                remaining_percent: ((100.0 - percent).clamp(0.0, 100.0) * 10.0).round() / 10.0,
                                reset_at,
                                used,
                                total,
                                description: None,
                            });
                        }
                    }

                    groups.push(ProviderPlanGroup {
                        group_name: product,
                        edition: Some(format!("{} 套餐", edition)),
                        periods,
                    });
                }
            }

            let viewer = json_val.get("viewer");
            let user_name = viewer.and_then(|v| v.get("user_name")).and_then(|u| u.as_str()).map(|s| s.to_string());
            let account_id = viewer.and_then(|v| v.get("account_id")).and_then(|a| a.as_str()).map(|s| s.to_string());

            let result = ProviderUsageData {
                provider: "volcengine".to_string(),
                provider_name: "火山方舟".to_string(),
                icon: "🌋".to_string(),
                is_connected: true,
                status_message: Some("在线同步成功".to_string()),
                error_message: None,
                account_info: Some(ProviderAccountInfo {
                    user_name,
                    email: None,
                    account_id,
                    plan_name: Some("Coding Plan".to_string()),
                }),
                groups,
                primary_session_percent,
                primary_reset_at,
                console_url: Some("https://console.volcengine.com/ark/region:cn-beijing/subscription/coding-plan-enterprise".to_string()),
            };

            result
        }
        Err(e) => {
            // Fallback via get_seat_info_usage directly if usage plan fails
            if let Some(milestones) = query_volc_seat_usage(None) {
                let mut periods = Vec::new();
                let short_used = milestones.short_term_usage.unwrap_or(0.0);
                let week_used = milestones.weekly_usage.unwrap_or(0.0);
                let month_used = milestones.monthly_usage.unwrap_or(0.0);

                periods.push(ProviderQuotaPeriod {
                    label: "session".to_string(),
                    name: "近5小时用量".to_string(),
                    used_percent: (short_used * 10.0).round() / 10.0,
                    remaining_percent: ((100.0 - short_used).clamp(0.0, 100.0) * 10.0).round() / 10.0,
                    reset_at: milestones.short_term_reset.clone(),
                    used: None,
                    total: None,
                    description: None,
                });

                periods.push(ProviderQuotaPeriod {
                    label: "weekly".to_string(),
                    name: "近一周用量".to_string(),
                    used_percent: (week_used * 10.0).round() / 10.0,
                    remaining_percent: ((100.0 - week_used).clamp(0.0, 100.0) * 10.0).round() / 10.0,
                    reset_at: milestones.weekly_reset.clone(),
                    used: None,
                    total: None,
                    description: None,
                });

                periods.push(ProviderQuotaPeriod {
                    label: "monthly".to_string(),
                    name: "近一月用量".to_string(),
                    used_percent: (month_used * 10.0).round() / 10.0,
                    remaining_percent: ((100.0 - month_used).clamp(0.0, 100.0) * 10.0).round() / 10.0,
                    reset_at: milestones.monthly_reset.clone(),
                    used: None,
                    total: None,
                    description: None,
                });

                let result = ProviderUsageData {
                    provider: "volcengine".to_string(),
                    provider_name: "火山方舟".to_string(),
                    icon: "🌋".to_string(),
                    is_connected: true,
                    status_message: Some("在线同步成功 (席位直连)".to_string()),
                    error_message: None,
                    account_info: Some(ProviderAccountInfo {
                        user_name: None,
                        email: None,
                        account_id: None,
                        plan_name: Some("Coding Plan".to_string()),
                    }),
                    groups: vec![ProviderPlanGroup {
                        group_name: "Coding Plan".to_string(),
                        edition: Some("席位配额".to_string()),
                        periods,
                    }],
                    primary_session_percent: Some(short_used),
                    primary_reset_at: milestones.short_term_reset,
                    console_url: Some("https://console.volcengine.com/ark/region:cn-beijing/subscription/coding-plan-enterprise".to_string()),
                };

                return result;
            }

            // Slow diagnostic path: only check environment when usage plan fails
            let env_status = check_environment();
            let status_msg = if !env_status.has_arkcli {
                "未检测到 arkcli 命令行工具".to_string()
            } else if !env_status.logged_in {
                "未登录火山方舟或登录已过期".to_string()
            } else {
                "获取火山方舟配额失败".to_string()
            };

            ProviderUsageData {
                provider: "volcengine".to_string(),
                provider_name: "火山方舟".to_string(),
                icon: "🌋".to_string(),
                is_connected: false,
                status_message: Some(status_msg),
                error_message: Some(e),
                account_info: Some(ProviderAccountInfo {
                    user_name: env_status.user_name,
                    email: None,
                    account_id: env_status.account_id,
                    plan_name: Some("Coding Plan".to_string()),
                }),
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://console.volcengine.com/ark/region:cn-beijing/subscription/coding-plan-enterprise".to_string()),
            }
        }
    }
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
            .collect()
    };
    let l_parts = parse_parts(latest);
    let c_parts = parse_parts(current);
    for (l, c) in l_parts.iter().zip(c_parts.iter()) {
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }
    l_parts.len() > c_parts.len()
}

#[tauri::command]
pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let repo = "chzisnull/ark-bar";
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    // 1. Try GitHub Releases API first (gets release notes & tag)
    if let Ok(output) = execute_cmd("curl", &["-s", "--max-time", "3", "-H", "User-Agent: ark-bar-app", &api_url]) {
        if output.status.success() {
            let body = String::from_utf8_lossy(&output.stdout);
            if let Ok(json_val) = serde_json::from_str::<Value>(&body) {
                if let Some(tag_name) = json_val.get("tag_name").and_then(|v| v.as_str()) {
                    let latest_version = tag_name.trim_start_matches('v').to_string();
                    let release_url = json_val.get("html_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&format!("https://github.com/{}/releases", repo))
                        .to_string();
                    let release_notes = json_val.get("body")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let has_update = is_newer_version(&latest_version, &current_version);
                    let asset_file = crate::updater::get_platform_asset_filename(&latest_version);
                    let mut download_url = Some(format!(
                        "https://github.com/{}/releases/download/v{}/{}",
                        repo, latest_version, asset_file
                    ));

                    if let Some(assets) = json_val.get("assets").and_then(|a| a.as_array()) {
                        for asset in assets {
                            if let Some(name) = asset.get("name").and_then(|n| n.as_str()) {
                                if name == asset_file {
                                    if let Some(url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                                        download_url = Some(url.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    return Ok(UpdateInfo {
                        has_update,
                        current_version,
                        latest_version,
                        release_url,
                        release_notes,
                        download_url,
                    });
                }
            }
        }
    }

    // 2. Fallback: GitHub Releases redirect (bypasses 60 req/hr API limit)
    let redirect_url = format!("https://github.com/{}/releases/latest", repo);
    if let Ok(output) = execute_cmd("curl", &["-sI", "--max-time", "3", "-A", "ark-bar-app", &redirect_url]) {
        let headers = String::from_utf8_lossy(&output.stdout);
        for line in headers.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("location:") {
                let loc = line["location:".len()..].trim();
                if let Some(tag) = loc.split("/tag/").nth(1) {
                    let latest_version = tag.trim_start_matches('v').to_string();
                    let has_update = is_newer_version(&latest_version, &current_version);
                    let asset_file = crate::updater::get_platform_asset_filename(&latest_version);
                    let download_url = Some(format!(
                        "https://github.com/{}/releases/download/v{}/{}",
                        repo, latest_version, asset_file
                    ));
                    return Ok(UpdateInfo {
                        has_update,
                        current_version,
                        latest_version,
                        release_url: loc.to_string(),
                        release_notes: "发现新版本，支持应用内一键在线极速更新。".to_string(),
                        download_url,
                    });
                }
            }
        }
    }

    Ok(UpdateInfo {
        has_update: false,
        current_version: current_version.clone(),
        latest_version: current_version,
        release_url: format!("https://github.com/{}/releases", repo),
        release_notes: "当前已是最新版本".to_string(),
        download_url: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usage_plan_json() {
        let sample_json = r#"{
            "viewer": {
                "user_name": "test_user",
                "account_id": "123456"
            },
            "items": [
                {
                    "product": "coding-plan-team",
                    "edition": "team",
                    "seat_id": "seat-test-123",
                    "subscribed": true,
                    "periods": [
                        { "label": "session", "percent": 25.5 },
                        { "label": "weekly", "percent": 40.0 },
                        { "label": "monthly", "percent": 15.0, "reset_at": "2026-10-01T00:00:00+08:00" }
                    ]
                }
            ]
        }"#;

        let parsed: Result<Value, _> = serde_json::from_str(sample_json);
        assert!(parsed.is_ok());
        let val = parsed.unwrap();
        assert_eq!(val["items"][0]["seat_id"], "seat-test-123");
        assert_eq!(val["items"][0]["periods"][1]["percent"], 40.0);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("0.2.0", "0.1.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.1"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }

    #[test]
    fn test_unix_to_iso8601() {
        assert_eq!(unix_to_iso8601(1789137461), "2026-09-11T14:37:41Z");
        assert_eq!(unix_to_iso8601(1789315200), "2026-09-13T16:00:00Z");
        assert_eq!(unix_to_iso8601(1791302399), "2026-10-06T15:59:59Z");
    }
}
