use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use serde_json::Value;
use crate::env_resolver::execute_cmd_timeout;
use crate::provider_models::{
    ProviderAccountInfo, ProviderPlanGroup, ProviderQuotaPeriod, ProviderUsageData,
};
use crate::usage_cache::UsageCache;

static AGY_CACHE: UsageCache = UsageCache::new();
static AGY_CMD: Mutex<Option<String>> = Mutex::new(None);

fn resolve_agy_command() -> String {
    if let Ok(guard) = AGY_CMD.lock() {
        if let Some(ref cmd) = *guard {
            return cmd.clone();
        }
    }

    let resolved = {
        if let Ok(home) = env::var("HOME") {
            let p = PathBuf::from(&home).join(".local/bin/agy");
            if p.exists() {
                p.to_string_lossy().to_string()
            } else {
                "agy".to_string()
            }
        } else {
            "agy".to_string()
        }
    };

    #[cfg(target_os = "windows")]
    let resolved = {
        if let Ok(localappdata) = env::var("LOCALAPPDATA") {
            let p = PathBuf::from(localappdata).join("Programs").join("agy").join("agy.exe");
            if p.exists() {
                p.to_string_lossy().to_string()
            } else {
                resolved
            }
        } else {
            resolved
        }
    };

    if let Ok(mut guard) = AGY_CMD.lock() {
        *guard = Some(resolved.clone());
    }
    resolved
}

fn get_active_google_email() -> Option<String> {
    let mut path = None;
    if let Ok(home) = env::var("HOME") {
        path = Some(PathBuf::from(home).join(".gemini").join("google_accounts.json"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            path = Some(PathBuf::from(userprofile).join(".gemini").join("google_accounts.json"));
        }
    }

    if let Some(p) = path {
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                    if let Some(active) = val.get("active").and_then(|v| v.as_str()) {
                        return Some(active.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn get_antigravity_usage() -> ProviderUsageData {
    get_antigravity_usage_forced(false)
}

pub fn get_antigravity_usage_forced(force: bool) -> ProviderUsageData {
    AGY_CACHE.get_or_refresh(force, fetch_antigravity_usage_uncached)
}

pub fn peek_antigravity_usage() -> Option<ProviderUsageData> {
    AGY_CACHE.peek()
}

fn fetch_antigravity_usage_uncached() -> ProviderUsageData {
    let cmd = resolve_agy_command();
    let email = get_active_google_email();

    let output = match execute_cmd_timeout(
        &cmd,
        &["-p", "/usage", "--output-format", "json"],
        Duration::from_secs(45),
    ) {
        Ok(out) => out,
        Err(e) => {
            if let Some(cached) = AGY_CACHE.peek().filter(|d| d.is_connected) {
                return cached;
            }
            return ProviderUsageData {
                provider: "antigravity".to_string(),
                provider_name: "Google Antigravity".to_string(),
                icon: "🌐".to_string(),
                is_connected: false,
                status_message: Some("未检测到 agy 命令行工具".to_string()),
                error_message: Some(format!("执行 agy 失败: {}", e)),
                account_info: email.map(|em| ProviderAccountInfo {
                    user_name: None,
                    email: Some(em),
                    account_id: None,
                    plan_name: Some("Google Antigravity".to_string()),
                }),
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://antigravity.google".to_string()),
            };
        }
    };

    if !output.status.success() {
        if let Some(cached) = AGY_CACHE.peek().filter(|d| d.is_connected) {
            return cached;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return ProviderUsageData {
            provider: "antigravity".to_string(),
            provider_name: "Google Antigravity".to_string(),
            icon: "🌐".to_string(),
            is_connected: false,
            status_message: Some("agy 查询用量失败".to_string()),
            error_message: Some(format!("agy 退出码异常: {}", stderr.trim())),
            account_info: email.map(|em| ProviderAccountInfo {
                user_name: None,
                email: Some(em),
                account_id: None,
                plan_name: Some("Google Antigravity".to_string()),
            }),
            groups: Vec::new(),
            primary_session_percent: None,
            primary_reset_at: None,
            console_url: Some("https://antigravity.google".to_string()),
        };
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let json_val: Value = match serde_json::from_str(&stdout_str) {
        Ok(v) => v,
        Err(e) => {
            if let Some(cached) = AGY_CACHE.peek().filter(|d| d.is_connected) {
                return cached;
            }
            return ProviderUsageData {
                provider: "antigravity".to_string(),
                provider_name: "Google Antigravity".to_string(),
                icon: "🌐".to_string(),
                is_connected: false,
                status_message: Some("解析 agy JSON 失败".to_string()),
                error_message: Some(format!("JSON 解析错误: {}", e)),
                account_info: email.map(|em| ProviderAccountInfo {
                    user_name: None,
                    email: Some(em),
                    account_id: None,
                    plan_name: Some("Google Antigravity".to_string()),
                }),
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://antigravity.google".to_string()),
            };
        }
    };

    let mut groups: Vec<ProviderPlanGroup> = Vec::new();
    let mut primary_session_percent: Option<f64> = None;
    let mut primary_reset_at: Option<String> = None;

    if let Some(cmd_groups) = json_val
        .get("command")
        .and_then(|c| c.get("data"))
        .and_then(|d| d.get("groups"))
        .and_then(|g| g.as_array())
    {
        for g in cmd_groups {
            let group_name = g.get("name").and_then(|n| n.as_str()).unwrap_or("Model Group").to_string();
            let mut periods: Vec<ProviderQuotaPeriod> = Vec::new();

            if let Some(buckets) = g.get("buckets").and_then(|b| b.as_array()) {
                for b in buckets {
                    let bucket_id = b.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    let b_name = b.get("name").and_then(|n| n.as_str()).unwrap_or(bucket_id);
                    let window = b.get("window").and_then(|w| w.as_str()).unwrap_or("");
                    let remaining_fraction = b.get("remaining_fraction").and_then(|r| r.as_f64()).unwrap_or(1.0);
                    let reset_time = b.get("reset_time").and_then(|rt| rt.as_str()).map(|s| s.to_string());

                    let remaining_percent = (remaining_fraction * 100.0).clamp(0.0, 100.0);
                    let used_percent = ((1.0 - remaining_fraction) * 100.0).clamp(0.0, 100.0);

                    let (label, display_name) = if window == "5h" || bucket_id.contains("5h") {
                        ("session".to_string(), format!("{} 5小时限额", group_name.replace(" Models", "").replace(" models", "")))
                    } else if window == "weekly" || bucket_id.contains("weekly") {
                        ("weekly".to_string(), format!("{} 周度限额", group_name.replace(" Models", "").replace(" models", "")))
                    } else {
                        (window.to_string(), b_name.to_string())
                    };

                    // Record primary session metric for tray display
                    if bucket_id == "gemini-5h" || (label == "session" && primary_session_percent.is_none()) {
                        primary_session_percent = Some(used_percent);
                        primary_reset_at = reset_time.clone();
                    }

                    periods.push(ProviderQuotaPeriod {
                        label,
                        name: display_name,
                        used_percent: (used_percent * 10.0).round() / 10.0,
                        remaining_percent: (remaining_percent * 10.0).round() / 10.0,
                        reset_at: reset_time,
                        used: None,
                        total: None,
                        description: b.get("description").and_then(|d| d.as_str()).map(|s| s.to_string()),
                    });
                }
            }

            groups.push(ProviderPlanGroup {
                group_name: group_name.clone(),
                edition: Some("Antigravity 订阅".to_string()),
                periods,
            });
        }
    }

    let result = ProviderUsageData {
        provider: "antigravity".to_string(),
        provider_name: "Google Antigravity".to_string(),
        icon: "🌐".to_string(),
        is_connected: !groups.is_empty(),
        status_message: Some("在线同步成功".to_string()),
        error_message: None,
        account_info: Some(ProviderAccountInfo {
            user_name: None,
            email,
            account_id: None,
            plan_name: Some("Google Antigravity".to_string()),
        }),
        groups,
        primary_session_percent,
        primary_reset_at,
        console_url: Some("https://antigravity.google".to_string()),
    };

    result
}
