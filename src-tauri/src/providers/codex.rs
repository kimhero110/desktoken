// Codex (ChatGPT) — GET https://chatgpt.com/backend-api/wham/usage
// Auth: OAuth access_token from Codex CLI (~/.codex/auth.json, "chatgpt" mode).
// Refresh: POST https://auth.openai.com/oauth/token (shared 6-step protocol).
// Response (tolerant-parsed):
// {
//   "plan_type": "plus",
//   "rate_limit": {
//     "primary_window":   { "used_percent": 5.0,  "limit_window_seconds": 18000,
//                           "reset_after_seconds": 1234, "reset_at": 1786000000 },
//     "secondary_window": { "used_percent": 12.0, "limit_window_seconds": 604800, ... }
//   }
// }
use super::{ProviderError, QuotaSnapshot, QuotaWindow};
use crate::fetch;
use crate::oauth;

pub const ID: &str = "codex";
pub const NAME: &str = "Codex";
const ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";

const CRED_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "tokens.access_token",
    refresh_path: "tokens.refresh_token",
    expires_path: None, // expiry comes from the access-token JWT exp claim
    expiry_unit: oauth::ExpiryUnit::Seconds,
};

fn cred_path() -> Result<std::path::PathBuf, ProviderError> {
    let home = std::env::var("USERPROFILE").map_err(|_| ProviderError::CredentialMissing)?;
    let p = std::path::Path::new(&home).join(".codex/auth.json");
    if p.exists() {
        Ok(p)
    } else {
        Err(ProviderError::CredentialMissing)
    }
}

/// Best-effort read of the account id for the chatgpt-account-id header.
fn account_id(path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    fetch::json_path(&v, "tokens.account_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

/// Codex CLI OAuth refresh (same client_id the official CLI uses).
async fn refresh_call(refresh_token: String) -> Result<oauth::RefreshResult, oauth::RefreshFailure> {
    const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
    const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
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
    let mut extra = vec![];
    if let Some(idt) = resp.get("id_token").and_then(|t| t.as_str()) {
        extra.push(("tokens.id_token".to_string(), idt.to_string()));
    }
    extra.push((
        "last_refresh".to_string(),
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    ));
    Ok(oauth::RefreshResult {
        access_token: access,
        refresh_token: refresh,
        expires_in_secs: expires_in,
        extra_writes: extra,
    })
}

/// Label a window by its length, not its position: on Pro Lite plans the
/// *primary* window is weekly (604800s) with no 5h window at all.
fn label_for(limit_window_seconds: Option<f64>) -> String {
    match limit_window_seconds.map(|s| s as i64) {
        Some(18000) => "5h".into(),
        Some(86400) => "今日".into(),
        Some(604800) => "周".into(),
        Some(s) if s >= 2_592_000 => "月".into(),
        Some(s) if s > 0 && s % 3600 == 0 && s / 3600 <= 99 => format!("{}h", s / 3600),
        _ => "窗口".into(),
    }
}

fn window_from(w: &serde_json::Value) -> Option<QuotaWindow> {
    let used = w.get("used_percent").and_then(fetch::as_f64)?;
    let resets_at = w
        .get("reset_at")
        .and_then(super::parse_reset)
        .or_else(|| {
            w.get("reset_after_seconds")
                .and_then(fetch::as_f64)
                .map(|s| super::now_secs() + s as i64)
        });
    Some(QuotaWindow {
        label: label_for(w.get("limit_window_seconds").and_then(fetch::as_f64)),
        used_percent: used,
        resets_at,
    })
}

pub fn parse(body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let rl = v.get("rate_limit").unwrap_or(&v);
    let mut windows: Vec<QuotaWindow> = vec![];
    if let Some(w) = rl.get("primary_window").and_then(window_from) {
        windows.push(w);
    }
    if let Some(w) = rl.get("secondary_window").and_then(window_from) {
        windows.push(w);
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    // display order: shortest window first (5h before weekly)
    let order = |w: &QuotaWindow| match w.label.as_str() {
        "5h" => 0,
        "今日" => 1,
        "周" => 2,
        "月" => 3,
        _ => 4,
    };
    windows.sort_by_key(order);
    let plan = v
        .get("plan_type")
        .and_then(|p| p.as_str())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => p.to_string(),
            }
        });
    Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "official"))
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let path = cred_path()?;
    let (token, _source) = oauth::resolve_oauth_token(&path, &CRED_SPEC, refresh_call).await?;
    let auth = format!("Bearer {}", token);
    let acct = account_id(&path).unwrap_or_default();
    let (status, body) = fetch::get_json(
        ENDPOINT,
        &[("Authorization", &auth), ("chatgpt-account-id", &acct)],
    )
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
        // Plus-style: 5h primary + weekly secondary
        let body = r#"{
          "plan_type": "plus",
          "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": { "used_percent": 5.0, "limit_window_seconds": 18000,
                                "reset_after_seconds": 3600, "reset_at": 1786000000 },
            "secondary_window": { "used_percent": 12.5, "limit_window_seconds": 604800,
                                  "reset_after_seconds": 400000, "reset_at": 1786600000 }
          },
          "credits": { "has_credits": false, "unlimited": false, "balance": null }
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[0].label, "5h");
        assert!((s.windows[0].used_percent - 5.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1786000000));
        assert_eq!(s.windows[1].label, "周");
        assert!((s.windows[1].used_percent - 12.5).abs() < 0.01);
        assert_eq!(s.plan.as_deref(), Some("Plus"));
    }

    #[test]
    fn parses_prolite_weekly_primary() {
        // real 2026-09 capture: Pro Lite has a *weekly* primary window and
        // secondary_window: null — the label must come from the window length.
        let body = r#"{
          "plan_type": "prolite",
          "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": { "used_percent": 9, "limit_window_seconds": 604800,
                                "reset_after_seconds": 404037, "reset_at": 1788758375 },
            "secondary_window": null
          },
          "additional_rate_limits": [
            { "limit_name": "GPT-5.3-Codex-Spark",
              "rate_limit": { "primary_window": { "used_percent": 0, "limit_window_seconds": 18000,
                                                  "reset_after_seconds": 18000, "reset_at": 1788372338 },
                              "secondary_window": { "used_percent": 0, "limit_window_seconds": 604800,
                                                    "reset_after_seconds": 604800, "reset_at": 1788959138 } } }
          ]
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].label, "周");
        assert!((s.windows[0].used_percent - 9.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1788758375));
        assert_eq!(s.plan.as_deref(), Some("Prolite"));
    }

    #[test]
    fn tolerates_missing_secondary_window() {
        let body = r#"{
          "plan_type": "free",
          "rate_limit": { "primary_window": { "used_percent": 0, "reset_after_seconds": 60 } }
        }"#;
        let s = parse(body).unwrap();
        assert_eq!(s.windows.len(), 1);
        assert!(s.windows[0].resets_at.unwrap() > super::super::now_secs());
    }

    #[test]
    fn tolerates_string_percent() {
        let body = r#"{ "rate_limit": { "primary_window": { "used_percent": "42.5", "reset_at": "2026-09-02T17:00:00Z" } } }"#;
        let s = parse(body).unwrap();
        assert!((s.windows[0].used_percent - 42.5).abs() < 0.01);
        assert!(s.windows[0].resets_at.is_some());
    }

    #[test]
    fn malformed_response_is_parse_error() {
        assert!(matches!(parse("not json"), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse(r#"{"foo": 1}"#), Err(ProviderError::ParseFailed)));
    }
}
