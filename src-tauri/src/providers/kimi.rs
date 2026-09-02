// Kimi for Coding — GET https://api.kimi.com/coding/v1/usages
// Auth: Bearer <OAuth access_token from Kimi CLI> or <Console API key>.
// Response (tolerant-parsed, values may be strings):
// {
//   "usage":  { "limit":"100","used":"1","remaining":"99","resetTime":"...Z" },   // weekly
//   "limits": [ { "window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},         // 5h window
//                 "detail":{"limit":"100","used":"5","remaining":"95"} } ]
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
    let home = std::env::var("USERPROFILE").map_err(|_| ProviderError::CredentialMissing)?;
    for rel in [
        ".kimi-code/credentials/kimi-code.json",
        ".kimi/credentials/kimi-code.json",
    ] {
        let p = std::path::Path::new(&home).join(rel);
        if p.exists() {
            return oauth::resolve_oauth_token(&p, &CRED_SPEC, refresh_call).await;
        }
    }
    Err(ProviderError::CredentialMissing)
}

/// Kimi OAuth device-flow token endpoint.
async fn refresh_call(refresh_token: String) -> Result<oauth::RefreshResult, oauth::RefreshFailure> {
    const TOKEN_URL: &str = "https://auth.kimi.com/api/oauth/token";
    const CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
    let (status, body) = fetch::post_form(
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

pub fn parse_value(v: &serde_json::Value) -> Result<QuotaSnapshot, ProviderError> {
    let mut windows: Vec<QuotaWindow> = vec![];

    // weekly: usage.{used,limit,resetTime}
    if let Some(u) = v.get("usage") {
        let used = u.get("used").and_then(fetch::as_f64);
        let limit = u.get("limit").and_then(fetch::as_f64);
        if let (Some(used), Some(limit)) = (used, limit) {
            if limit > 0.0 {
                windows.push(QuotaWindow {
                    label: "周".into(),
                    used_percent: used / limit * 100.0,
                    resets_at: u.get("resetTime").and_then(parse_reset),
                });
            }
        }
    }

    // rolling windows: limits[] (300 minutes = 5h); detail.resetTime present on live data
    if let Some(arr) = v.get("limits").and_then(|l| l.as_array()) {
        for item in arr {
            let mins = item
                .get("window")
                .and_then(|w| w.get("duration"))
                .and_then(fetch::as_f64);
            let detail = item.get("detail");
            if let (Some(300.0), Some(d)) = (mins, detail) {
                let used = d.get("used").and_then(fetch::as_f64);
                let limit = d.get("limit").and_then(fetch::as_f64);
                if let (Some(used), Some(limit)) = (used, limit) {
                    if limit > 0.0 {
                        windows.push(QuotaWindow {
                            label: "5h".into(),
                            used_percent: used / limit * 100.0,
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
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str(),
                None => s.to_string(),
            }
        });
    Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "official"))
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let (token, _source) = resolve_token().await?;
    let (status, body) = fetch::get_with_auth(ENDPOINT, "Authorization", "Bearer ", &token)
        .await
        .map_err(|_| ProviderError::Network)?;
    match status {
        200..=299 => parse(&body),
        401 | 403 => Err(ProviderError::AuthExpired),
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
        assert!(matches!(parse(r#"{"foo": 1}"#), Err(ProviderError::ParseFailed)));
    }

    #[test]
    fn epoch_millis_reset_time() {
        let v = serde_json::json!(1786000000000i64);
        assert_eq!(parse_reset(&v), Some(1786000000));
        let v = serde_json::json!("2026-09-06T04:00:00Z");
        assert!(parse_reset(&v).unwrap() > 1_700_000_000);
    }
}
