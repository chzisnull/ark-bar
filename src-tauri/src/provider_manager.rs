use std::thread;
use crate::ark_cli::{get_volcengine_usage, peek_volcengine_usage};
use crate::provider_antigravity::{get_antigravity_usage, peek_antigravity_usage};
use crate::provider_codex::{get_codex_usage, peek_codex_usage};
use crate::provider_grok::{get_grok_usage, peek_grok_usage};
use crate::provider_models::ProviderUsageData;
use crate::token_store::{get_token, save_token};

#[tauri::command]
pub fn get_unified_usage(provider: String, custom_token: Option<String>) -> Result<ProviderUsageData, String> {
    let token = custom_token.or_else(|| get_token(&provider));
    match provider.as_str() {
        "volcengine" => Ok(get_volcengine_usage()),
        "antigravity" => Ok(get_antigravity_usage()),
        "grok" => Ok(get_grok_usage(token.as_deref())),
        "codex" => Ok(get_codex_usage(token.as_deref())),
        unknown => Err(format!("未知的服务商: {}", unknown)),
    }
}

#[tauri::command]
pub fn peek_cached_usage(provider: String) -> Option<ProviderUsageData> {
    match provider.as_str() {
        "volcengine" => peek_volcengine_usage(),
        "antigravity" => peek_antigravity_usage(),
        "grok" => peek_grok_usage(),
        "codex" => peek_codex_usage(),
        _ => None,
    }
}

#[tauri::command]
pub fn get_all_providers_usage() -> Vec<ProviderUsageData> {
    let grok_token = get_token("grok");
    let codex_token = get_token("codex");

    // Fetch all 4 providers concurrently in parallel threads to minimize latency!
    let h_volc = thread::spawn(get_volcengine_usage);
    let h_agy = thread::spawn(get_antigravity_usage);
    let h_grok = thread::spawn(move || get_grok_usage(grok_token.as_deref()));
    let h_codex = thread::spawn(move || get_codex_usage(codex_token.as_deref()));

    let volc = h_volc.join().unwrap_or_else(|_| get_volcengine_usage());
    let agy = h_agy.join().unwrap_or_else(|_| get_antigravity_usage());
    let grok = h_grok.join().unwrap_or_else(|_| get_grok_usage(None));
    let codex = h_codex.join().unwrap_or_else(|_| get_codex_usage(None));

    vec![volc, agy, grok, codex]
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
        assert_eq!(providers.len(), 4);
        assert_eq!(providers[0].provider, "volcengine");
        assert_eq!(providers[1].provider, "antigravity");
        assert_eq!(providers[2].provider, "grok");
        assert_eq!(providers[3].provider, "codex");
    }

    #[test]
    fn test_token_save_read() {
        let _ = save_token("test_provider", "test_token_123");
        assert_eq!(get_token("test_provider"), Some("test_token_123".to_string()));
        let _ = save_token("test_provider", "");
        assert_eq!(get_token("test_provider"), None);
    }
}
