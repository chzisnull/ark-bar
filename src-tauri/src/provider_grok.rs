use std::env;
use std::path::PathBuf;
use std::time::Duration;
use serde_json::Value;
use crate::env_resolver::execute_cmd;
use crate::provider_models::{
    ProviderAccountInfo, ProviderPlanGroup, ProviderQuotaPeriod, ProviderUsageData,
};
use crate::usage_cache::UsageCache;

static GROK_CACHE: UsageCache = UsageCache::new();

pub struct GrokAuthData {
    pub token: String,
    pub email: Option<String>,
    pub user_name: Option<String>,
    pub user_id: Option<String>,
}

pub fn get_grok_auth(custom_token: Option<&str>) -> Option<GrokAuthData> {
    if let Some(tok) = custom_token {
        let trimmed = tok.trim();
        if !trimmed.is_empty() {
            return Some(GrokAuthData {
                token: trimmed.to_string(),
                email: None,
                user_name: None,
                user_id: None,
            });
        }
    }

    // Check ~/.grok/auth.json
    let mut auth_path = None;
    if let Ok(home) = env::var("HOME") {
        auth_path = Some(PathBuf::from(home).join(".grok").join("auth.json"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            auth_path = Some(PathBuf::from(userprofile).join(".grok").join("auth.json"));
        }
    }

    if let Some(p) = auth_path {
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(val) = serde_json::from_str::<Value>(&content) {
                    if let Some(obj) = val.as_object() {
                        for (_k, entry) in obj {
                            if let Some(key) = entry.get("key").and_then(|k| k.as_str()) {
                                if !key.is_empty() {
                                    let email = entry.get("email").and_then(|e| e.as_str()).map(|s| s.to_string());
                                    let first = entry.get("first_name").and_then(|f| f.as_str()).unwrap_or("");
                                    let last = entry.get("last_name").and_then(|l| l.as_str()).unwrap_or("");
                                    let user_name = if !first.is_empty() || !last.is_empty() {
                                        Some(format!("{} {}", first, last).trim().to_string())
                                    } else {
                                        None
                                    };
                                    let user_id = entry.get("user_id").and_then(|u| u.as_str()).map(|s| s.to_string());

                                    return Some(GrokAuthData {
                                        token: key.to_string(),
                                        email,
                                        user_name,
                                        user_id,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn get_grok_usage(custom_token: Option<&str>) -> ProviderUsageData {
    if custom_token.is_none() {
        if let Some(fresh) = GROK_CACHE.get_fresh(Duration::from_secs(90)) {
            return fresh;
        }
        if let Some(stale) = GROK_CACHE.get_stale(Duration::from_secs(15 * 60)) {
            if GROK_CACHE.begin_refresh() {
                std::thread::spawn(|| {
                    let data = fetch_grok_usage_uncached(None);
                    GROK_CACHE.set(data);
                });
            }
            return stale;
        }
    }

    let data = fetch_grok_usage_uncached(custom_token);
    if custom_token.is_none() {
        GROK_CACHE.set(data.clone());
    }
    data
}

pub fn peek_grok_usage() -> Option<ProviderUsageData> {
    GROK_CACHE.peek()
}

fn fetch_grok_usage_uncached(custom_token: Option<&str>) -> ProviderUsageData {
    let auth = match get_grok_auth(custom_token) {
        Some(a) => a,
        None => {
            return ProviderUsageData {
                provider: "grok".to_string(),
                provider_name: "xAI Grok".to_string(),
                icon: "⚡".to_string(),
                is_connected: false,
                status_message: Some("未找到 Grok 凭证".to_string()),
                error_message: Some("请在终端运行 grok login 或在设置中填入 Grok 访问令牌".to_string()),
                account_info: None,
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://grok.com".to_string()),
            };
        }
    };

    let url = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
    let auth_header = format!("Authorization: Bearer {}", auth.token);

    let output = match execute_cmd(
        "curl",
        &[
            "-s",
            "--max-time",
            "6",
            "-H",
            &auth_header,
            "-H",
            "X-XAI-Token-Auth: xai-grok-cli",
            "-H",
            "User-Agent: grok-cli/1.0.25",
            url,
        ],
    ) {
        Ok(out) => out,
        Err(e) => {
            return ProviderUsageData {
                provider: "grok".to_string(),
                provider_name: "xAI Grok".to_string(),
                icon: "⚡".to_string(),
                is_connected: false,
                status_message: Some("网络请求失败".to_string()),
                error_message: Some(format!("执行 curl 失败: {}", e)),
                account_info: Some(ProviderAccountInfo {
                    user_name: auth.user_name,
                    email: auth.email,
                    account_id: auth.user_id,
                    plan_name: Some("SuperGrok".to_string()),
                }),
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://grok.com".to_string()),
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return ProviderUsageData {
            provider: "grok".to_string(),
            provider_name: "xAI Grok".to_string(),
            icon: "⚡".to_string(),
            is_connected: false,
            status_message: Some("请求 Grok 服务失败".to_string()),
            error_message: Some(format!("curl 退出状态异常: {}", stderr.trim())),
            account_info: Some(ProviderAccountInfo {
                user_name: auth.user_name,
                email: auth.email,
                account_id: auth.user_id,
                plan_name: Some("SuperGrok".to_string()),
            }),
            groups: Vec::new(),
            primary_session_percent: None,
            primary_reset_at: None,
            console_url: Some("https://grok.com".to_string()),
        };
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let json_val: Value = match serde_json::from_str(&stdout_str) {
        Ok(v) => v,
        Err(e) => {
            return ProviderUsageData {
                provider: "grok".to_string(),
                provider_name: "xAI Grok".to_string(),
                icon: "⚡".to_string(),
                is_connected: false,
                status_message: Some("解析 Grok 响应失败".to_string()),
                error_message: Some(format!("JSON 解析失败: {}, 返回: {}", e, stdout_str.chars().take(120).collect::<String>())),
                account_info: Some(ProviderAccountInfo {
                    user_name: auth.user_name,
                    email: auth.email,
                    account_id: auth.user_id,
                    plan_name: Some("SuperGrok".to_string()),
                }),
                groups: Vec::new(),
                primary_session_percent: None,
                primary_reset_at: None,
                console_url: Some("https://grok.com".to_string()),
            };
        }
    };

    // Check if error response from server
    if let Some(err_msg) = json_val.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
        return ProviderUsageData {
            provider: "grok".to_string(),
            provider_name: "xAI Grok".to_string(),
            icon: "⚡".to_string(),
            is_connected: false,
            status_message: Some("Grok 认证失败或令牌失效".to_string()),
            error_message: Some(err_msg.to_string()),
            account_info: Some(ProviderAccountInfo {
                user_name: auth.user_name,
                email: auth.email,
                account_id: auth.user_id,
                plan_name: Some("SuperGrok".to_string()),
            }),
            groups: Vec::new(),
            primary_session_percent: None,
            primary_reset_at: None,
            console_url: Some("https://grok.com".to_string()),
        };
    }

    let config = json_val.get("config").unwrap_or(&json_val);
    let credit_usage_percent = config
        .get("creditUsagePercent")
        .and_then(|p| p.as_f64())
        .unwrap_or(0.0);

    let reset_at = config
        .get("currentPeriod")
        .and_then(|c| c.get("end"))
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            config
                .get("billingPeriodEnd")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string())
        });

    let mut periods: Vec<ProviderQuotaPeriod> = Vec::new();
    let used_percent = credit_usage_percent.clamp(0.0, 100.0);
    let remaining_percent = (100.0 - used_percent).clamp(0.0, 100.0);

    periods.push(ProviderQuotaPeriod {
        label: "weekly".to_string(),
        name: "当前周期使用配额".to_string(),
        used_percent: (used_percent * 10.0).round() / 10.0,
        remaining_percent: (remaining_percent * 10.0).round() / 10.0,
        reset_at: reset_at.clone(),
        used: None,
        total: None,
        description: Some("每周动态配额池，按模型消耗比例折算".to_string()),
    });

    if let Some(products) = config.get("productUsage").and_then(|p| p.as_array()) {
        for prod in products {
            let p_name = prod.get("product").and_then(|p| p.as_str()).unwrap_or("Grok");
            let p_usage = prod.get("usagePercent").and_then(|u| u.as_f64()).unwrap_or(0.0);
            if p_name != "GrokBuild" {
                periods.push(ProviderQuotaPeriod {
                    label: p_name.to_lowercase(),
                    name: format!("{} 消耗比例", p_name),
                    used_percent: (p_usage * 10.0).round() / 10.0,
                    remaining_percent: ((100.0 - p_usage).clamp(0.0, 100.0) * 10.0).round() / 10.0,
                    reset_at: reset_at.clone(),
                    used: None,
                    total: None,
                    description: None,
                });
            }
        }
    }

    let groups = vec![ProviderPlanGroup {
        group_name: "GrokBuild".to_string(),
        edition: Some("SuperGrok 订阅".to_string()),
        periods,
    }];

    let result = ProviderUsageData {
        provider: "grok".to_string(),
        provider_name: "xAI Grok".to_string(),
        icon: "⚡".to_string(),
        is_connected: true,
        status_message: Some("在线同步成功".to_string()),
        error_message: None,
        account_info: Some(ProviderAccountInfo {
            user_name: auth.user_name,
            email: auth.email,
            account_id: auth.user_id,
            plan_name: Some("SuperGrok".to_string()),
        }),
        groups,
        primary_session_percent: Some(used_percent),
        primary_reset_at: reset_at,
        console_url: Some("https://grok.com".to_string()),
    };

    result
}
