use chrono::{Datelike, Local};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::env_resolver::execute_cmd;
use crate::provider_models::{
    ProviderAccountInfo, ProviderAmount, ProviderUsageData, ProviderUsageDataExtension,
};
use crate::usage_cache::UsageCache;

static TEAMO_CACHE: UsageCache = UsageCache::new();

const TEAMO_BASE_URL: &str = "https://teamorouter.cn";

#[derive(Debug, Default)]
struct TeamoUsageTotals {
    input_tokens: u64,
    output_tokens: u64,
    #[allow(dead_code)]
    cached_write_tokens: u64,
    cached_read_tokens: u64,
    total_tokens: u64,
    request_count: u64,
}

pub fn get_teamo_usage(custom_token: Option<&str>) -> ProviderUsageData {
    get_teamo_usage_forced(custom_token, false)
}

pub fn get_teamo_usage_forced(custom_token: Option<&str>, force: bool) -> ProviderUsageData {
    let token = custom_token.map(str::to_string);
    TEAMO_CACHE.get_or_refresh(force, move || {
        fetch_teamo_usage_uncached(token.as_deref())
    })
}

pub fn peek_teamo_usage() -> Option<ProviderUsageData> {
    TEAMO_CACHE.peek()
}

fn curl_get(path: &str, token: &str) -> Result<Value, String> {
    let url = format!("{}{}", TEAMO_BASE_URL, path);
    let output = execute_cmd(
        "curl",
        &[
            "-sS",
            "--max-time",
            "8",
            "-H",
            &format!("Authorization: Bearer {}", token),
            &url,
        ],
    )
    .map_err(|e| format!("执行 curl 失败: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "curl 退出码 {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    let json: Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("JSON 解析失败: {}, 返回: {}", e, stdout.chars().take(200).collect::<String>()))?;

    if let Some(message) = json.pointer("/error/message").and_then(Value::as_str) {
        let code = json
            .pointer("/error/code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        return Err(format!("TeamoRouter API {}: {}", code, message));
    }
    Ok(json)
}

fn parse_decimal(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// TeamoRouter 的金额字段有两种返回形态：
/// - 简版：`"85.32"` / `85.32`
/// - 现网版：`{"value":"4.67380993","currency":"USD"}`
/// 只认字符串会让余额与今日费用恒为 `None`，前端两张卡片一直显示 `--`。
fn parse_amount(value: Option<&Value>) -> Option<ProviderAmount> {
    let value = value?;
    match value {
        Value::Object(map) => {
            let amount = parse_decimal(map.get("value")?)?;
            let currency = map
                .get("currency")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|currency| !currency.is_empty())
                .unwrap_or("USD")
                .to_string();
            Some(ProviderAmount {
                value: amount,
                currency,
            })
        }
        _ => Some(ProviderAmount {
            value: parse_decimal(value)?,
            currency: "USD".to_string(),
        }),
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn range_usage(token: &str, start: u64, end: u64) -> TeamoUsageTotals {
    let json = match curl_get(&format!("/v1/usage?start_time={}&end_time={}", start, end), token) {
        Ok(value) => value,
        Err(_) => return TeamoUsageTotals::default(),
    };
    let usage = json.pointer("/usage").cloned().unwrap_or_default();
    let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
    let output = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
    let cached_write = usage.get("cached_write_tokens").and_then(Value::as_u64).unwrap_or(0);
    let cached_read = usage.get("cached_read_tokens").and_then(Value::as_u64).unwrap_or(0);
    let total = usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(input + output + cached_write + cached_read);

    TeamoUsageTotals {
        input_tokens: input,
        output_tokens: output,
        cached_write_tokens: cached_write,
        cached_read_tokens: cached_read,
        total_tokens: total,
        request_count: json.get("requests").and_then(Value::as_u64).unwrap_or(0),
    }
}

fn local_range_start(kind: &str) -> u64 {
    let now = Local::now();
    let date = now.date_naive();
    let start_date = match kind {
        "week" => date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64),
        "month" => date.with_day(1).unwrap_or(date),
        _ => date,
    };
    let start_time = if kind == "session" {
        now - chrono::Duration::hours(5)
    } else {
        start_date
            .and_hms_opt(0, 0, 0)
            .unwrap_or_else(|| now.naive_local())
            .and_local_timezone(Local)
            .single()
            .unwrap_or(now)
    };
    start_time.timestamp().max(0) as u64
}

fn today_range() -> (u64, u64) {
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    let start = midnight.and_local_timezone(Local).single().map(|t| t.timestamp().max(0) as u64);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    (start.unwrap_or(now), now)
}

fn fetch_teamo_usage_uncached(custom_token: Option<&str>) -> ProviderUsageData {
    let mut data = ProviderUsageData {
        provider: "teamo".to_string(),
        provider_name: "TeamoRouter".to_string(),
        icon: "🛰️".to_string(),
        is_connected: false,
        status_message: None,
        error_message: None,
        account_info: Some(ProviderAccountInfo {
            user_name: None,
            email: None,
            account_id: None,
            plan_name: Some("按量付费".to_string()),
        }),
        groups: Vec::new(),
        primary_session_percent: None,
        primary_reset_at: None,
        console_url: Some("https://teamorouter.com/dashboard".to_string()),
        token_summary: None,
        extension: ProviderUsageDataExtension::default(),
    };

    let token = custom_token.map(str::trim).filter(|value| !value.is_empty());
    let Some(token) = token else {
        data.status_message = Some("未配置 TeamoRouter API Key".to_string());
        data.error_message = Some("请在设置中填入 sk-teamo- 开头的 API Key".to_string());
        return data;
    };

    let balance_json = match curl_get("/v1/billing/balance", token) {
        Ok(value) => value,
        Err(error) => {
            data.status_message = Some("TeamoRouter 同步失败".to_string());
            data.error_message = Some(error);
            return data;
        }
    };

    let (start, end) = today_range();
    let costs_path = format!("/v1/billing/costs?start_time={}&end_time={}", start, end);
    let costs_json = curl_get(&costs_path, token).ok();
    let today_usage = range_usage(token, start, end);
    let session_usage = range_usage(token, local_range_start("session"), now_epoch());
    let week_usage = range_usage(token, local_range_start("week"), now_epoch());
    let month_usage = range_usage(token, local_range_start("month"), now_epoch());
    let month_cost_json = curl_get(
        &format!(
            "/v1/billing/costs?start_time={}&end_time={}",
            local_range_start("month"),
            now_epoch()
        ),
        token,
    )
    .ok();

    if let Some(error) = costs_json
        .as_ref()
        .and_then(|value| value.pointer("/error/message"))
        .and_then(Value::as_str)
    {
        data.status_message = Some("今日费用查询失败".to_string());
        data.error_message = Some(error.to_string());
    }

    let balance = parse_amount(balance_json.pointer("/balance"));
    let today_cost = costs_json.as_ref().and_then(|value| parse_amount(value.get("total_amount")));
    let today_requests = costs_json.as_ref().and_then(|value| value.get("requests")).and_then(Value::as_u64).unwrap_or(0);

    let month_cost = month_cost_json
        .as_ref()
        .and_then(|value| parse_amount(value.get("total_amount")));
    data.token_summary = Some(crate::provider_models::ProviderTokenSummary {
        session_5h_tokens: session_usage.total_tokens,
        today_tokens: today_usage.total_tokens,
        this_week_tokens: week_usage.total_tokens,
        this_month_tokens: month_usage.total_tokens,
        total_tokens: None,
        input_tokens: today_usage.input_tokens,
        output_tokens: today_usage.output_tokens,
        cache_hit_tokens: today_usage.cached_read_tokens,
        cache_hit_rate: if today_usage.total_tokens > 0 {
            ((today_usage.cached_read_tokens as f64 * 100.0 / today_usage.total_tokens as f64) * 10.0).round() / 10.0
        } else {
            0.0
        },
        this_month_cache_hit_tokens: month_usage.cached_read_tokens,
        this_month_cache_hit_rate: if month_usage.total_tokens > 0 {
            ((month_usage.cached_read_tokens as f64 * 100.0 / month_usage.total_tokens as f64) * 10.0).round() / 10.0
        } else {
            0.0
        },
        supports_cache_stats: true,
        request_count: today_usage.request_count,
        daily_history: Vec::new(),
        data_source_type: "api".to_string(),
        updated_at: Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });

    data.extension.balance = balance.clone();
    data.extension.today_cost = today_cost.clone();
    data.extension.month_cost = month_cost;
    data.status_message = Some(format!(
        "余额 {} · 今日费用 {} · {} 次请求",
        balance.as_ref().map(|value| format!("${:.2}", value.value)).unwrap_or_else(|| "--".to_string()),
        today_cost.as_ref().map(|value| format!("${:.4}", value.value)).unwrap_or_else(|| "--".to_string()),
        today_requests
    ));
    data.is_connected = true;
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_amount_parses_decimal_strings() {
        let amount = parse_amount(Some(&serde_json::json!("85.320000")));
        assert_eq!(amount.unwrap().value, 85.32);
    }

    #[test]
    fn parse_amount_parses_wrapped_objects() {
        let amount = parse_amount(Some(&serde_json::json!({
            "value": "4.67380993",
            "currency": "USD"
        })))
        .expect("现网对象形态必须能解析");
        assert_eq!(amount.value, 4.67380993);
        assert_eq!(amount.currency, "USD");
    }

    #[test]
    fn parse_amount_parses_numbers_and_defaults_currency() {
        let amount = parse_amount(Some(&serde_json::json!(0.32599507))).unwrap();
        assert_eq!(amount.value, 0.32599507);
        assert_eq!(amount.currency, "USD");

        let wrapped = parse_amount(Some(&serde_json::json!({ "value": 12.5 }))).unwrap();
        assert_eq!(wrapped.currency, "USD");
    }

    #[test]
    fn parse_amount_rejects_unparsable_payloads() {
        assert!(parse_amount(None).is_none());
        assert!(parse_amount(Some(&serde_json::json!({ "currency": "USD" }))).is_none());
        assert!(parse_amount(Some(&serde_json::json!("not-a-number"))).is_none());
    }

    #[test]
    fn missing_token_is_disconnected() {
        let data = fetch_teamo_usage_uncached(None);
        assert!(!data.is_connected);
    }

    #[test]
    fn configured_token_connects_when_env_present() {
        let Ok(token) = std::env::var("TEAMO_TEST_TOKEN") else {
            return;
        };
        let data = fetch_teamo_usage_uncached(Some(&token));
        assert!(data.is_connected, "error: {:?}", data.error_message);
        // 余额与今日费用必须真正落到 extension 上，否则前端两张卡片只会显示 `--`。
        assert!(
            data.extension.balance.is_some(),
            "余额解析为空，status: {:?}",
            data.status_message
        );
        assert!(
            data.extension.today_cost.is_some(),
            "今日费用解析为空，status: {:?}",
            data.status_message
        );
    }
}
