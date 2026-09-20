use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAmount {
    pub value: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderUsageDataExtension {
    pub balance: Option<ProviderAmount>,
    pub today_cost: Option<ProviderAmount>,
    pub month_cost: Option<ProviderAmount>,
}

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
    pub token_summary: Option<ProviderTokenSummary>,
    pub extension: ProviderUsageDataExtension,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyTokenRecord {
    pub date: String,              // "YYYY-MM-DD"
    pub total_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    pub request_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderTokenSummary {
    /// 近 5 小时用量 (Token)
    pub session_5h_tokens: u64,
    /// 今日已用 (Token)
    pub today_tokens: u64,
    /// 本周已用 (Token, 周一 00:00 至今)
    pub this_week_tokens: u64,
    /// 本月已用 (Token, 当月 1 日 00:00 至今)
    pub this_month_tokens: u64,
    /// 累计总 Token (若支持)
    pub total_tokens: Option<u64>,

    /// 今日/当期详细构成
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_tokens: u64,
    /// 今日缓存命中率 (0.0% ~ 100.0%)
    pub cache_hit_rate: f64,
    /// 本月缓存命中 Token 汇总 (当月 1 日至今)
    pub this_month_cache_hit_tokens: u64,
    /// 本月缓存命中率 (0.0% ~ 100.0%)
    pub this_month_cache_hit_rate: f64,
    /// 该服务商是否支持上下文缓存命中统计
    pub supports_cache_stats: bool,
    /// API/Agent 请求次数
    pub request_count: u64,

    /// 历史每日明细 (最近 14~30 天，用于折线/柱状图)
    pub daily_history: Vec<DailyTokenRecord>,

    /// 数据来源类型: "api" (官方精准API) | "local_logs" (本地日志聚合) | "credits" (额度积分换算) | "estimated" (配额折算)
    pub data_source_type: String,
    /// 数据更新时间 (ISO 8601)
    pub updated_at: String,
}
