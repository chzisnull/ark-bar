use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// tokens.json 是整文件读改写：SeatID 持久化与 provider token 保存
/// 并发时用这把锁串行化，避免互相覆盖丢数据
static TOKEN_LOCK: Mutex<()> = Mutex::new(());

/// 一次刷新里 get_token 会被反复调用（grok/codex/teamo/seat_id 等），
/// 每次都整读并反序列化 tokens.json 是纯浪费。缓存整份映射，写入路径
/// 同步刷新缓存，保证同进程内写后立即可读。
static TOKENS_CACHE: Mutex<Option<(Instant, HashMap<String, String>)>> = Mutex::new(None);
const TOKENS_CACHE_TTL: Duration = Duration::from_secs(5);

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

fn read_tokens_unlocked() -> HashMap<String, String> {
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

fn read_tokens_cached() -> HashMap<String, String> {
    if let Ok(guard) = TOKENS_CACHE.lock() {
        if let Some((cached_at, ref tokens)) = *guard {
            if cached_at.elapsed() < TOKENS_CACHE_TTL {
                return tokens.clone();
            }
        }
    }
    let tokens = read_tokens_unlocked();
    if let Ok(mut guard) = TOKENS_CACHE.lock() {
        *guard = Some((Instant::now(), tokens.clone()));
    }
    tokens
}

fn store_tokens_cache(tokens: &HashMap<String, String>) {
    if let Ok(mut guard) = TOKENS_CACHE.lock() {
        *guard = Some((Instant::now(), tokens.clone()));
    }
}

pub fn get_all_tokens() -> HashMap<String, String> {
    let _guard = TOKEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_tokens_cached()
}

pub fn get_token(provider: &str) -> Option<String> {
    get_all_tokens().get(provider).cloned()
}

pub fn save_token(provider: &str, token: &str) -> Result<(), String> {
    let _guard = TOKEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut tokens = read_tokens_unlocked();
    if token.trim().is_empty() {
        tokens.remove(provider);
    } else {
        tokens.insert(provider.to_string(), token.trim().to_string());
    }

    let p = get_config_file_path();
    let json_str = serde_json::to_string_pretty(&tokens)
        .map_err(|e| format!("序列化 tokens 失败: {}", e))?;
    fs::write(p, json_str).map_err(|e| format!("写入 tokens.json 失败: {}", e))?;
    store_tokens_cache(&tokens);
    Ok(())
}
