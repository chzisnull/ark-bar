use std::thread;
use crate::ark_cli::{get_volcengine_usage, get_volcengine_usage_forced, peek_volcengine_usage};
use crate::provider_antigravity::{get_antigravity_usage, get_antigravity_usage_forced, peek_antigravity_usage};
use crate::provider_codex::{get_codex_usage, get_codex_usage_forced, peek_codex_usage};
use crate::provider_grok::{get_grok_usage, get_grok_usage_forced, peek_grok_usage};
use crate::provider_teamo::{get_teamo_usage, get_teamo_usage_forced, peek_teamo_usage};
use crate::provider_models::ProviderUsageData;
use crate::token_store::{get_token, save_token};

#[tauri::command]
pub async fn get_unified_usage(
    provider: String,
    custom_token: Option<String>,
    force: Option<bool>,
) -> Result<ProviderUsageData, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let force = force.unwrap_or(false);
        let token = custom_token.or_else(|| get_token(&provider));
        match provider.as_str() {
            "volcengine" => Ok(get_volcengine_usage_forced(force)),
            "antigravity" => Ok(get_antigravity_usage_forced(force)),
            "grok" => Ok(get_grok_usage_forced(token.as_deref(), force)),
            "codex" => Ok(get_codex_usage_forced(token.as_deref(), force)),
            "teamo" => Ok(get_teamo_usage_forced(token.as_deref(), force)),
            unknown => Err(format!("未知的服务商: {}", unknown)),
        }
    })
    .await
    .map_err(|e| format!("用量查询任务失败: {}", e))?
}

#[tauri::command]
pub fn peek_cached_usage(provider: String) -> Option<ProviderUsageData> {
    match provider.as_str() {
        "volcengine" => peek_volcengine_usage(),
        "antigravity" => peek_antigravity_usage(),
        "grok" => peek_grok_usage(),
        "codex" => peek_codex_usage(),
        "teamo" => peek_teamo_usage(),
        _ => None,
    }
}

#[tauri::command]
pub fn get_all_providers_usage() -> Vec<ProviderUsageData> {
    let grok_token = get_token("grok");
    let codex_token = get_token("codex");
    let teamo_token = get_token("teamo");

    // Fetch all 4 providers concurrently in parallel threads to minimize latency!
    let h_volc = thread::spawn(get_volcengine_usage);
    let h_agy = thread::spawn(get_antigravity_usage);
    let h_grok = thread::spawn(move || get_grok_usage(grok_token.as_deref()));
    let h_codex = thread::spawn(move || get_codex_usage(codex_token.as_deref()));
    let h_teamo = thread::spawn(move || get_teamo_usage(teamo_token.as_deref()));

    let volc = h_volc.join().unwrap_or_else(|_| get_volcengine_usage());
    let agy = h_agy.join().unwrap_or_else(|_| get_antigravity_usage());
    let grok = h_grok.join().unwrap_or_else(|_| get_grok_usage(None));
    let codex = h_codex.join().unwrap_or_else(|_| get_codex_usage(None));
    let teamo = h_teamo.join().unwrap_or_else(|_| get_teamo_usage(None));

    vec![volc, agy, grok, codex, teamo]
}

#[tauri::command]
pub fn set_provider_token(provider: String, token: String) -> Result<(), String> {
    save_token(&provider, &token)
}

#[tauri::command]
pub fn read_provider_token(provider: String) -> Option<String> {
    get_token(&provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_providers_list() {
        let providers = get_all_providers_usage();
        assert_eq!(providers.len(), 5);
        assert_eq!(providers[0].provider, "volcengine");
        assert_eq!(providers[1].provider, "antigravity");
        assert_eq!(providers[2].provider, "grok");
        assert_eq!(providers[3].provider, "codex");
        assert_eq!(providers[4].provider, "teamo");
    }

    #[test]
    fn test_token_save_read() {
        let _ = save_token("test_provider", "test_token_123");
        assert_eq!(get_token("test_provider"), Some("test_token_123".to_string()));
        let _ = save_token("test_provider", "");
        assert_eq!(get_token("test_provider"), None);
    }
}
