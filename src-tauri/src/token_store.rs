use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn get_config_file_path() -> PathBuf {
    let mut dir = None;
    if let Ok(home) = env::var("HOME") {
        dir = Some(PathBuf::from(home).join(".ark-bar"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = env::var("USERPROFILE") {
            dir = Some(PathBuf::from(userprofile).join(".ark-bar"));
        }
    }

    let dir_path = dir.unwrap_or_else(|| PathBuf::from(".ark-bar"));
    if !dir_path.exists() {
        let _ = fs::create_dir_all(&dir_path);
    }
    dir_path.join("tokens.json")
}

pub fn get_all_tokens() -> HashMap<String, String> {
    let p = get_config_file_path();
    if p.exists() {
        if let Ok(content) = fs::read_to_string(p) {
            if let Ok(val) = serde_json::from_str::<HashMap<String, String>>(&content) {
                return val;
            }
        }
    }
    HashMap::new()
}

pub fn get_token(provider: &str) -> Option<String> {
    get_all_tokens().get(provider).cloned()
}

pub fn save_token(provider: &str, token: &str) -> Result<(), String> {
    let mut tokens = get_all_tokens();
    if token.trim().is_empty() {
        tokens.remove(provider);
    } else {
        tokens.insert(provider.to_string(), token.trim().to_string());
    }

    let p = get_config_file_path();
    let json_str = serde_json::to_string_pretty(&tokens)
        .map_err(|e| format!("序列化 tokens 失败: {}", e))?;
    fs::write(p, json_str).map_err(|e| format!("写入 tokens.json 失败: {}", e))
}
