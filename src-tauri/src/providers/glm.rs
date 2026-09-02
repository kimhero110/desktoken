// GLM Coding Plan (智谱) — GET open.bigmodel.cn/api/monitor/usage/quota/limit
// Auth: raw API key in Authorization header, NO Bearer prefix (per Zhipu's
// official zai-coding-plugins). Global users: api.z.ai host (auto-fallback).
// Response: data.limits[] classified by unit: TOKENS_LIMIT unit=3 → 5h,
// unit=6 → weekly; TIME_LIMIT unit=5 → monthly MCP (v1: shown as "月").
use super::{EpochSecs, ProviderError, QuotaSnapshot, QuotaWindow};
use crate::credentials;
use crate::fetch;

pub const ID: &str = "glm";
pub const NAME: &str = "GLM";
const ENDPOINT_CN: &str = "https://open.bigmodel.cn/api/monitor/usage/quota/limit";
const ENDPOINT_GLOBAL: &str = "https://api.z.ai/api/monitor/usage/quota/limit";

pub fn parse(body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    // Zhipu may return HTTP 200 with an error body: {"code": 401|1001..., "msg": ...}
    if let Some(code) = v.get("code").and_then(crate::fetch::as_f64) {
        if code != 200.0 {
            let msg = v
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_lowercase();
            return Err(if code == 401.0 || code == 1001.0 || msg.contains("token") || msg.contains("auth") || msg.contains("apikey") || msg.contains("api key") {
                ProviderError::AuthExpired
            } else if code == 429.0 || msg.contains("rate") {
                ProviderError::RateLimited { retry_after: None }
            } else {
                ProviderError::ParseFailed
            });
        }
    }
    parse_value(&v)
}

pub fn parse_value(v: &serde_json::Value) -> Result<QuotaSnapshot, ProviderError> {
    let data = v.get("data").ok_or(ProviderError::ParseFailed)?;
    let plan = data
        .get("planName")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());
    let limits = data
        .get("limits")
        .and_then(|l| l.as_array())
        .ok_or(ProviderError::ParseFailed)?;

    let mut five_hour: Option<QuotaWindow> = None;
    let mut weekly: Option<QuotaWindow> = None;
    let mut monthly: Option<QuotaWindow> = None;

    for item in limits {
        let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let unit = item.get("unit").and_then(fetch::as_f64).unwrap_or(0.0) as i64;
        let pct = item.get("percentage").and_then(fetch::as_f64);
        let reset: Option<EpochSecs> = item.get("nextResetTime").and_then(super::parse_reset);
        // classify by unit, not type: newer credit-based plans report the 5h/weekly
        // windows as CREDIT_LIMIT (unit 3/6), older token-based plans as TOKENS_LIMIT
        let is_quota = ty == "TOKENS_LIMIT" || ty == "CREDIT_LIMIT";
        match (is_quota, unit) {
            (true, 3) => {
                five_hour = pct.map(|p| QuotaWindow {
                    label: "5h".into(),
                    used_percent: p,
                    resets_at: reset,
                })
            }
            (true, 6) => {
                weekly = pct.map(|p| QuotaWindow {
                    label: "周".into(),
                    used_percent: p,
                    resets_at: reset,
                })
            }
            (_, _) if ty == "TIME_LIMIT" && unit == 5 => {
                monthly = pct.map(|p| QuotaWindow {
                    label: "月".into(),
                    used_percent: p,
                    resets_at: reset,
                })
            }
            _ => {} // unknown entries ignored
        }
    }

    let mut windows = vec![];
    if let Some(w) = five_hour {
        windows.push(w);
    }
    if let Some(w) = weekly {
        windows.push(w);
    }
    if let Some(w) = monthly {
        windows.push(w);
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "manual_key"))
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let key = credentials::keyring_get(ID).ok_or(ProviderError::CredentialMissing)?;
    // CN first, global fallback on 401/403 (wrong-region key)
    let (status, body) = fetch::get_with_auth(ENDPOINT_CN, "Authorization", "", &key)
        .await
        .map_err(|_| ProviderError::Network)?;
    let try_parse = |b: &str| {
        let r = parse(b);
        if r.is_err() {
            crate::rustlog(format!(
                "glm parse failed, body preview: {}",
                b.chars().take(300).collect::<String>()
            ));
        }
        r
    };
    match status {
        200..=299 => return try_parse(&body),
        401 | 403 => {
            let (status2, body2) = fetch::get_with_auth(ENDPOINT_GLOBAL, "Authorization", "", &key)
                .await
                .map_err(|_| ProviderError::Network)?;
            return match status2 {
                200..=299 => try_parse(&body2),
                401 | 403 => Err(ProviderError::AuthExpired),
                429 => Err(ProviderError::RateLimited { retry_after: None }),
                _ => Err(ProviderError::Network),
            };
        }
        429 => Err(ProviderError::RateLimited { retry_after: None }),
        _ => Err(ProviderError::Network),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_response() {
        let body = r#"{
          "code": 200, "msg": "success", "success": true,
          "data": {
            "planName": "Pro",
            "limits": [
              { "type": "TOKENS_LIMIT", "unit": 3, "number": 5, "percentage": 25, "nextResetTime": 1785816000000 },
              { "type": "TOKENS_LIMIT", "unit": 6, "number": 1, "percentage": 9,  "nextResetTime": 1786291200000 },
              { "type": "TIME_LIMIT",   "unit": 5, "number": 1, "usage": 1000, "currentValue": 224, "remaining": 776, "percentage": 22.4 }
            ]
          }
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.plan.as_deref(), Some("Pro"));
        assert_eq!(s.windows.len(), 3);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 25.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1785816000));
        assert_eq!(s.windows[1].label, "周");
        assert_eq!(s.windows[2].label, "月");
        assert!((s.windows[2].used_percent - 22.4).abs() < 0.01);
    }

    #[test]
    fn old_plan_without_weekly() {
        let body = r#"{ "code":200, "data": { "limits": [
            { "type":"TOKENS_LIMIT", "unit":3, "percentage": 12 }
        ] } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "5h");
        assert!(s.windows[0].resets_at.is_none());
    }

    #[test]
    fn credit_limit_plan_classified_by_unit() {
        // live-captured: credit-based subscription reports 5h/weekly as CREDIT_LIMIT
        let body = r#"{ "code":200, "msg":"操作成功", "data": { "limits": [
            { "type":"CREDIT_LIMIT", "unit":3, "number":5, "usage":28000, "currentValue":1638, "remaining":26361, "percentage":5, "nextResetTime":1788345820016 },
            { "type":"CREDIT_LIMIT", "unit":6, "number":1, "usage":140000, "currentValue":4494, "remaining":135505, "percentage":3 }
        ] } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 5.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1788345820));
        assert_eq!(s.windows[1].label, "周");
    }

    #[test]
    fn ignores_credit_limit_entry() {
        let body = r#"{ "code":200, "data": { "limits": [
            { "type":"CREDIT_LIMIT", "unit":9, "percentage": 55 },
            { "type":"TOKENS_LIMIT", "unit":6, "percentage": 30 }
        ] } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "周");
    }

    #[test]
    fn http200_with_error_body_is_auth_error() {
        let body = r#"{ "code": 401, "msg": "无效的ApiKey" }"#;
        assert!(matches!(parse(body), Err(ProviderError::AuthExpired)));
        let body2 = r#"{ "code": 429, "msg": "rate limit" }"#;
        assert!(matches!(parse(body2), Err(ProviderError::RateLimited { .. })));
    }

    #[test]
    fn malformed_is_parse_error() {
        assert!(matches!(parse("{}"), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse("<html>"), Err(ProviderError::ParseFailed)));
    }
}
