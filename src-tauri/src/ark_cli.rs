use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::env_resolver::execute_cmd;

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
}

#[tauri::command]
pub fn check_environment() -> EnvironmentStatus {
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

    status
}

#[tauri::command]
pub fn install_arkcli() -> CommandResult {
    match execute_cmd("npm", &["install", "-g", "@volcengine/ark-cli@latest"]) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
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

#[tauri::command]
pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let repo = "chzisnull/ark-bar";
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    // Use curl which is always available on macOS / Linux
    let output = execute_cmd("curl", &["-s", "--max-time", "3", "-H", "User-Agent: ark-bar-app", &url])
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !output.status.success() {
        return Err("GitHub API 响应失败".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if let Ok(json_val) = serde_json::from_str::<Value>(&body) {
        if let Some(tag_name) = json_val.get("tag_name").and_then(|v| v.as_str()) {
            let latest_version = tag_name.trim_start_matches('v').to_string();
            let release_url = json_val.get("html_url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://github.com/chzisnull/ark-bar/releases")
                .to_string();
            let release_notes = json_val.get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let has_update = latest_version != current_version && !latest_version.is_empty();

            return Ok(UpdateInfo {
                has_update,
                current_version,
                latest_version,
                release_url,
                release_notes,
            });
        }
    }

    Ok(UpdateInfo {
        has_update: false,
        current_version: current_version.clone(),
        latest_version: current_version,
        release_url: format!("https://github.com/{}/releases", repo),
        release_notes: "当前已是最新版本".to_string(),
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
}
