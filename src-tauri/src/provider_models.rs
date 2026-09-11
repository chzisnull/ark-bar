use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderQuotaPeriod {
    pub label: String,            // "session" | "weekly" | "monthly"
    pub name: String,             // Display label: "近5小时用量", "Gemini 5小时限额", etc.
    pub used_percent: f64,        // 0.0 - 100.0
    pub remaining_percent: f64,   // 0.0 - 100.0
    pub reset_at: Option<String>, // ISO 8601 timestamp or date string
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPlanGroup {
    pub group_name: String,       // e.g. "Coding Plan", "Gemini Models", "GrokBuild", "Codex CLI"
    pub edition: Option<String>,  // e.g. "企业套餐", "Pro", "SuperGrok", "Plus"
    pub periods: Vec<ProviderQuotaPeriod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccountInfo {
    pub user_name: Option<String>,
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageData {
    pub provider: String,         // "volcengine" | "antigravity" | "grok" | "codex"
    pub provider_name: String,    // "火山方舟" | "Antigravity" | "xAI Grok" | "OpenAI Codex"
    pub icon: String,             // "🌋" | "🌐" | "⚡" | "🤖"
    pub is_connected: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub account_info: Option<ProviderAccountInfo>,
    pub groups: Vec<ProviderPlanGroup>,
    pub primary_session_percent: Option<f64>,
    pub primary_reset_at: Option<String>,
    pub console_url: Option<String>,
}
