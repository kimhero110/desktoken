// Kimi for Coding — GET https://api.kimi.com/coding/v1/usages
// Auth: Bearer <OAuth access_token from Kimi CLI> or <Console API key>.
// Response (tolerant-parsed, values may be strings; `used` may be absent —
// then usage is derived from limit-remaining, see docs/kimi-quota-fix.md):
// {
//   "usage":  { "limit":"100","used":"1","remaining":"99","resetTime":"...Z" },   // weekly
//   "limits": [ { "window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},         // 5h window
//                 "detail":{"limit":"100","remaining":"100","resetTime":"...Z"} } ]
// }
use super::{parse_reset, ProviderError, QuotaSnapshot, QuotaWindow};
use crate::credentials;
use crate::fetch;
use crate::oauth;

pub const ID: &str = "kimi";
pub const NAME: &str = "Kimi";
const ENDPOINT: &str = "https://api.kimi.com/coding/v1/usages";

const CRED_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "access_token",
    refresh_path: "refresh_token",
    expires_path: Some("expires_at"),
    expiry_unit: oauth::ExpiryUnit::Seconds,
};

/// Credential: manual key (keyring) preferred, else Kimi CLI OAuth file
/// (refresh + write-back handled by the shared 6-step protocol, oauth.rs).
pub async fn resolve_token() -> Result<(String, &'static str), ProviderError> {
    if let Some(k) = credentials::keyring_get(ID) {
        return Ok((k, "manual_key"));
    }
    // credentials::cli_cred_path, not USERPROFILE: that env var is unset on
    // macOS, so the CLI credential was never found there.
    let p = credentials::cli_cred_path(ID).ok_or(ProviderError::CredentialMissing)?;
    oauth::resolve_oauth_token(&p, &CRED_SPEC, refresh_call).await
}

/// Kimi OAuth device-flow token endpoint.
async fn refresh_call(
    refresh_token: String,
) -> Result<oauth::RefreshResult, oauth::RefreshFailure> {
    const TOKEN_URL: &str = "https://auth.kimi.com/api/oauth/token";
    const CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
    let (status, body, _retry_after) = fetch::post_form(
        TOKEN_URL,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("client_id", CLIENT_ID),
        ],
    )
    .await
    .map_err(|_| oauth::RefreshFailure::Network)?;
    if !(200..300).contains(&status) {
        return Err(if status == 400 || status == 401 {
            oauth::RefreshFailure::InvalidGrant
        } else {
            oauth::RefreshFailure::Network
        });
    }
    let resp: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| oauth::RefreshFailure::Parse)?;
    let access = resp
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or(oauth::RefreshFailure::Parse)?
        .to_string();
    let refresh = resp
        .get("refresh_token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string());
    let expires_in = resp
        .get("expires_in")
        .and_then(crate::fetch::as_f64)
        .map(|s| s as i64)
        .or_else(|| {
            resp.get("expires_at")
                .and_then(crate::fetch::as_f64)
                .map(|x| x as i64 - super::now_secs())
        });
    Ok(oauth::RefreshResult {
        access_token: access,
        refresh_token: refresh,
        expires_in_secs: expires_in,
        extra_writes: vec![],
    })
}

pub fn parse(body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    parse_value(&v)
}

/// Shared quota percentage: prefer explicit valid finite `used`; when `used`
/// is absent/null derive `limit - remaining`. Requires finite positive limit;
/// fallback additionally requires finite non-negative remaining within limit.
/// Malformed explicit `used` never silently becomes zero — window is dropped.
fn used_percent(detail: &serde_json::Value) -> Option<f64> {
    let limit = detail.get("limit").and_then(fetch::as_f64)?;
    if !limit.is_finite() || limit <= 0.0 {
        return None;
    }
    let used_field = detail.get("used");
    let explicit = match used_field {
        None | Some(serde_json::Value::Null) => None,
        Some(raw) => Some(fetch::as_f64(raw)),
    };
    match explicit {
        Some(Some(used)) if used.is_finite() && used >= 0.0 => Some(used / limit * 100.0),
        Some(_) => None, // malformed explicit used: do not fabricate or fall back
        None => {
            let remaining = detail.get("remaining").and_then(fetch::as_f64)?;
            if !remaining.is_finite() || remaining < 0.0 || remaining > limit {
                return None;
            }
            Some((limit - remaining) / limit * 100.0)
        }
    }
}

/// 5h window recognition: 300 TIME_UNIT_MINUTE (legacy: missing unit = minutes),
/// 5 TIME_UNIT_HOUR, or 18000 TIME_UNIT_SECOND. Unknown/wrong unit → not 5h.
fn is_5h_window(window: &serde_json::Value) -> bool {
    let Some(duration) = window.get("duration").and_then(fetch::as_f64) else {
        return false;
    };
    let unit = match window.get("timeUnit") {
        // legacy fixtures: absent unit = minutes; present null/number/non-string rejects
        None => "TIME_UNIT_MINUTE",
        Some(serde_json::Value::String(s)) => s.as_str(),
        Some(_) => return false,
    };
    let minutes = match unit {
        "TIME_UNIT_MINUTE" => duration,
        "TIME_UNIT_HOUR" => duration * 60.0,
        "TIME_UNIT_SECOND" => duration / 60.0,
        _ => return false,
    };
    minutes.is_finite() && minutes == 300.0
}

pub fn parse_value(v: &serde_json::Value) -> Result<QuotaSnapshot, ProviderError> {
    let mut windows: Vec<QuotaWindow> = vec![];

    // weekly: usage.{used|remaining,limit,resetTime}
    if let Some(u) = v.get("usage") {
        if let Some(pct) = used_percent(u) {
            windows.push(QuotaWindow {
                label: "周".into(),
                used_percent: pct,
                resets_at: u.get("resetTime").and_then(parse_reset),
            });
        }
    }

    // rolling windows: limits[] (5h); detail.resetTime present on live data
    if let Some(arr) = v.get("limits").and_then(|l| l.as_array()) {
        for item in arr {
            let window = item.get("window");
            let detail = item.get("detail");
            if let (Some(w), Some(d)) = (window, detail) {
                if is_5h_window(w) {
                    if let Some(pct) = used_percent(d) {
                        windows.push(QuotaWindow {
                            label: "5h".into(),
                            used_percent: pct,
                            resets_at: d.get("resetTime").and_then(parse_reset),
                        });
                    }
                }
            }
        }
    }

    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    // display order: 5h first, then weekly
    windows.sort_by_key(|w| if w.label == "5h" { 0 } else { 1 });
    // plan tier: user.membership.level, e.g. "LEVEL_ADVANCED" → "Advanced"
    let plan = v
        .get("user")
        .and_then(|u| u.get("membership"))
        .and_then(|m| m.get("level"))
        .and_then(|l| l.as_str())
        .map(|l| {
            let s = l.strip_prefix("LEVEL_").unwrap_or(l);
            let mut c = s.chars();
            match c.next() {
                Some(f) => {
                    f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str()
                }
                None => s.to_string(),
            }
        });
    Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "official"))
}

async fn fetch_with_bearer(token: &str) -> Result<QuotaSnapshot, ProviderError> {
    let (status, body, retry_after) =
        fetch::get_with_auth(ENDPOINT, "Authorization", "Bearer ", token)
            .await
            .map_err(|_| ProviderError::Network)?;
    match status {
        200..=299 => parse(&body),
        401 | 403 => Err(ProviderError::AuthExpired),
        429 => Err(ProviderError::RateLimited { retry_after }),
        _ => Err(ProviderError::Network),
    }
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let (token, _source) = resolve_token().await?;
    fetch_with_bearer(&token).await
}

/// Multi-instance entry (方案 B): "kimi" (CLI/manual) or "kimi#opencode".
pub async fn fetch_instance(inst: &str) -> Result<QuotaSnapshot, ProviderError> {
    match inst {
        "kimi#opencode" => match crate::credentials::opencode_cred("kimi-for-coding") {
            Some(crate::credentials::OpencodeCred::ApiKey(k)) => {
                let mut s = fetch_with_bearer(&k).await?;
                s.source = "manual_key".into();
                Ok(s)
            }
            _ => Err(ProviderError::CredentialMissing),
        },
        _ => fetch_snapshot().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_response() {
        let body = r#"{
          "usage": { "limit": "100", "used": "21", "remaining": "79", "resetTime": "2026-09-06T04:00:00Z" },
          "limits": [ { "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                        "detail": { "limit": "100", "used": "5", "remaining": "95" } } ]
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 5.0).abs() < 0.01);
        assert_eq!(s.windows[1].label, "周");
        assert!((s.windows[1].used_percent - 21.0).abs() < 0.01);
        assert!(s.windows[1].resets_at.unwrap() > 1_700_000_000);
    }

    #[test]
    fn tolerates_numeric_instead_of_strings() {
        let body = r#"{ "usage": { "limit": 200, "used": 50, "resetTime": 1786000000 } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert!((s.windows[0].used_percent - 25.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1786000000));
    }

    #[test]
    fn tolerates_missing_limits_array() {
        let body = r#"{ "usage": { "limit": "100", "used": "0", "remaining": "100" } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
    }

    #[test]
    fn malformed_response_is_parse_error() {
        assert!(matches!(parse("not json"), Err(ProviderError::ParseFailed)));
        assert!(matches!(
            parse(r#"{"foo": 1}"#),
            Err(ProviderError::ParseFailed)
        ));
    }

    #[test]
    fn epoch_millis_reset_time() {
        let v = serde_json::json!(1786000000000i64);
        assert_eq!(parse_reset(&v), Some(1786000000));
        let v = serde_json::json!("2026-09-06T04:00:00Z");
        assert!(parse_reset(&v).unwrap() > 1_700_000_000);
    }

    // --- quota-fix regressions: official endpoint omits `used` (measured 2026-09-08) ---

    /// Real sanitized live fixture: limits[0] has no `used` field; 5h must show
    /// 0% (limit==remaining) and keep resetTime.
    #[test]
    fn real_missing_used_fixture_yields_5h_zero_percent_with_reset() {
        let body = r#"{
          "limits": [ { "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                        "detail": { "limit": "100", "remaining": "100",
                                    "resetTime": "2026-09-08T08:04:00.891299Z" } } ]
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 0.0).abs() < 0.01);
        let reset = s.windows[0].resets_at.expect("reset retained");
        assert!(reset > 1_700_000_000);
    }

    /// Remaining-only fallback with partial consumption: 100 limit / 55 remaining → 45%.
    #[test]
    fn remaining_only_derives_partial_percentage() {
        let body = r#"{
          "limits": [ { "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                        "detail": { "limit": "100", "remaining": "55" } } ]
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 45.0).abs() < 0.01);
    }

    /// Weekly `usage` object also falls back to limit-remaining when `used` absent.
    #[test]
    fn weekly_usage_falls_back_to_remaining() {
        let body = r#"{ "usage": { "limit": "200", "remaining": "50", "resetTime": "2026-09-06T04:00:00Z" } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "周");
        assert!((s.windows[0].used_percent - 75.0).abs() < 0.01);
    }

    /// Explicit `used` wins over an inconsistent `remaining`.
    #[test]
    fn explicit_used_takes_precedence() {
        let body = r#"{
          "usage": { "limit": "100", "used": "10", "remaining": "70" },
          "limits": [ { "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                        "detail": { "limit": "100", "used": "20", "remaining": "70" } } ]
        }"#;
        let s = parse(body).unwrap();
        assert!((s.windows[0].used_percent - 20.0).abs() < 0.01); // 5h
        assert!((s.windows[1].used_percent - 10.0).abs() < 0.01); // weekly
    }

    /// Missing both `used` and `remaining`, malformed `used`, remaining out of
    /// range, or non-positive limit → window ignored (never fabricated zero).
    #[test]
    fn missing_both_or_invalid_fields_are_ignored() {
        let no_fields =
            serde_json::json!({ "usage": { "limit": "100", "resetTime": "2026-09-06T04:00:00Z" } });
        assert!(matches!(
            parse_value(&no_fields),
            Err(ProviderError::ParseFailed)
        ));

        let bad_used = serde_json::json!({ "usage": { "limit": "100", "used": "NaN" } });
        assert!(matches!(
            parse_value(&bad_used),
            Err(ProviderError::ParseFailed)
        ));

        let bad_remaining = serde_json::json!({ "usage": { "limit": "100", "remaining": "150" } });
        assert!(matches!(
            parse_value(&bad_remaining),
            Err(ProviderError::ParseFailed)
        ));

        let neg_remaining = serde_json::json!({ "usage": { "limit": "100", "remaining": "-1" } });
        assert!(matches!(
            parse_value(&neg_remaining),
            Err(ProviderError::ParseFailed)
        ));

        let zero_limit = serde_json::json!({ "usage": { "limit": "0", "remaining": "0" } });
        assert!(matches!(
            parse_value(&zero_limit),
            Err(ProviderError::ParseFailed)
        ));

        let mixed = serde_json::json!({
          "usage": { "limit": "100", "used": "1" },
          "limits": [ { "window": { "duration": 300, "timeUnit": "TIME_UNIT_MINUTE" },
                        "detail": { "limit": "100" } } ]
        });
        let s = parse_value(&mixed).unwrap();
        assert_eq!(s.windows.len(), 1); // only weekly survives
        assert_eq!(s.windows[0].label, "周");
    }

    /// 5 TIME_UNIT_HOUR and 18000 TIME_UNIT_SECOND are 5h; 300 HOURS is not;
    /// unknown, null, or non-string unit is not; legacy missing unit = minutes.
    #[test]
    fn window_unit_normalization() {
        let mk = |duration: i64, unit: Option<serde_json::Value>| {
            let mut window = serde_json::Map::new();
            window.insert("duration".into(), serde_json::json!(duration));
            if let Some(u) = unit {
                window.insert("timeUnit".into(), u);
            }
            serde_json::json!({
              "limits": [ { "window": window,
                            "detail": { "limit": "100", "remaining": "50" } } ]
            })
        };
        assert!(parse_value(&mk(5, Some(serde_json::json!("TIME_UNIT_HOUR")))).is_ok());
        let s = parse_value(&mk(18000, Some(serde_json::json!("TIME_UNIT_SECOND")))).unwrap();
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 50.0).abs() < 0.01);
        // legacy fixture: missing unit → minutes
        assert!(parse_value(&mk(300, None)).is_ok());
        // wrong / unknown / non-string units
        let rejects = [
            mk(300, Some(serde_json::json!("TIME_UNIT_HOUR"))),
            mk(300, Some(serde_json::json!("TIME_UNIT_WEEK"))),
            mk(300, Some(serde_json::Value::Null)),
            mk(300, Some(serde_json::json!(1))),
        ];
        for v in &rejects {
            assert!(
                matches!(parse_value(v), Err(ProviderError::ParseFailed)),
                "rejected: {v}"
            );
        }
    }
}
