// Claude — GET https://api.anthropic.com/api/oauth/usage
// Auth: OAuth accessToken from Claude Code CLI (~/.claude/.credentials.json),
// refreshed via POST https://console.anthropic.com/v1/oauth/token and written
// back through the shared 6-step protocol (oauth.rs).
// Poll interval is clamped to >= 10 min (PLAN.md ToS decision).
// Response (tolerant-parsed; utilization may be a 0..1 fraction or 0..100):
// {
//   "five_hour": { "utilization": 0.05, "resets_at": "2026-09-02T14:00:00Z" },
//   "seven_day": { "utilization": 0.33, "resets_at": "2026-09-08T00:00:00Z" }
// }
use super::{ProviderError, QuotaSnapshot, QuotaWindow};
use crate::fetch;
use crate::oauth;

pub const ID: &str = "claude";
pub const NAME: &str = "Claude";
const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";

const CRED_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "claudeAiOauth.accessToken",
    refresh_path: "claudeAiOauth.refreshToken",
    expires_path: Some("claudeAiOauth.expiresAt"),
    expiry_unit: oauth::ExpiryUnit::Millis,
};

fn cred_path() -> Result<std::path::PathBuf, ProviderError> {
    let home = std::env::var("USERPROFILE").map_err(|_| ProviderError::CredentialMissing)?;
    let p = std::path::Path::new(&home).join(".claude/.credentials.json");
    if p.exists() {
        Ok(p)
    } else {
        Err(ProviderError::CredentialMissing)
    }
}

/// Plan tier from the credential file (usage endpoint does not return it).
fn plan_from_credentials(path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let t = fetch::json_path(&v, "claudeAiOauth.subscriptionType")?.as_str()?;
    let mut c = t.chars();
    Some(match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => t.to_string(),
    })
}

/// Claude Code OAuth refresh (same client_id the official CLI uses).
async fn refresh_call(refresh_token: String) -> Result<oauth::RefreshResult, oauth::RefreshFailure> {
    const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
    const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
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
        .and_then(fetch::as_f64)
        .map(|s| s as i64);
    Ok(oauth::RefreshResult {
        access_token: access,
        refresh_token: refresh,
        expires_in_secs: expires_in,
        extra_writes: vec![],
    })
}

fn window_from(w: &serde_json::Value, label: &str) -> Option<QuotaWindow> {
    let raw = w.get("utilization").and_then(fetch::as_f64)?;
    // observed as a 0..1 fraction; tolerate 0..100 percent style too
    let used_percent = if raw <= 1.0 { raw * 100.0 } else { raw };
    Some(QuotaWindow {
        label: label.into(),
        used_percent,
        resets_at: w.get("resets_at").and_then(super::parse_reset),
    })
}

pub fn parse(body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let mut windows: Vec<QuotaWindow> = vec![];
    if let Some(w) = v.get("five_hour").and_then(|w| window_from(w, "5h")) {
        windows.push(w);
    }
    if let Some(w) = v.get("seven_day").and_then(|w| window_from(w, "周")) {
        windows.push(w);
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    Ok(QuotaSnapshot::ok(ID, NAME, None, windows, "official"))
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let path = cred_path()?;
    let (token, _source) = oauth::resolve_oauth_token(&path, &CRED_SPEC, refresh_call).await?;
    let auth = format!("Bearer {}", token);
    let (status, body) = fetch::get_json(
        ENDPOINT,
        &[
            ("Authorization", &auth),
            ("anthropic-beta", "oauth-2025-04-20"),
        ],
    )
    .await
    .map_err(|_| ProviderError::Network)?;
    match status {
        200..=299 => {
            let mut snap = parse(&body)?;
            snap.plan = plan_from_credentials(&path);
            Ok(snap)
        }
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
          "five_hour": { "utilization": 0.24, "resets_at": "2026-09-02T17:00:00Z" },
          "seven_day": { "utilization": 0.14, "resets_at": "2026-09-08T04:00:00Z" },
          "seven_day_opus": { "utilization": 0.0, "resets_at": "2026-09-08T04:00:00Z" }
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 24.0).abs() < 0.01);
        assert!(s.windows[0].resets_at.unwrap() > 1_700_000_000);
        assert_eq!(s.windows[1].label, "周");
        assert!((s.windows[1].used_percent - 14.0).abs() < 0.01);
    }

    #[test]
    fn tolerates_percent_style_utilization() {
        let body = r#"{ "five_hour": { "utilization": 42.5, "resets_at": 1786000000 } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert!((s.windows[0].used_percent - 42.5).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1786000000));
    }

    #[test]
    fn tolerates_missing_seven_day() {
        let body = r#"{ "five_hour": { "utilization": 0.0 } }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].resets_at, None);
    }

    #[test]
    fn malformed_response_is_parse_error() {
        assert!(matches!(parse("not json"), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse(r#"{"foo": 1}"#), Err(ProviderError::ParseFailed)));
    }
}
