// Gemini (Google Code Assist) — two-step quota fetch:
//   POST /v1internal:loadCodeAssist     → project id (+ current tier)
//   POST /v1internal:retrieveUserQuota  → per-model daily buckets
// Auth: OAuth from Gemini CLI (~/.gemini/oauth_creds.json), refreshed via
// https://oauth2.googleapis.com/token with the public gemini-cli client creds
// through the shared 6-step protocol (oauth.rs).
// 403 → UnsupportedClient: personal tier was migrated to Antigravity
// (PLAN.md risk table — the UI tells the user to disable the row).
// Poll interval is clamped to >= 5 min (PLAN.md decision #12).
use super::{ProviderError, QuotaSnapshot, QuotaWindow};
use crate::fetch;
use crate::oauth;
use serde_json::Value;

pub const ID: &str = "gemini";
pub const NAME: &str = "Gemini";
const BASE: &str = "https://cloudcode-pa.googleapis.com/v1internal";

const CRED_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "access_token",
    refresh_path: "refresh_token",
    expires_path: Some("expiry_date"),
    expiry_unit: oauth::ExpiryUnit::Millis,
};

fn cred_path() -> Result<std::path::PathBuf, ProviderError> {
    let home = crate::credentials::home().ok_or(ProviderError::CredentialMissing)?;
    let p = home.join(".gemini/oauth_creds.json");
    if p.exists() {
        Ok(p)
    } else {
        Err(ProviderError::CredentialMissing)
    }
}

/// Google OAuth refresh with the public gemini-cli client credentials.
async fn refresh_call(refresh_token: String) -> Result<oauth::RefreshResult, oauth::RefreshFailure> {
    const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
    const CLIENT_ID: &str = "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
    const CLIENT_SECRET: &str = "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";
    let (status, body) = fetch::post_form(
        TOKEN_URL,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
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
    let resp: Value = serde_json::from_str(&body).map_err(|_| oauth::RefreshFailure::Parse)?;
    let access = resp
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or(oauth::RefreshFailure::Parse)?
        .to_string();
    let expires_in = resp
        .get("expires_in")
        .and_then(fetch::as_f64)
        .map(|s| s as i64);
    // Google does not rotate the refresh token here: None => keep the old one.
    Ok(oauth::RefreshResult {
        access_token: access,
        refresh_token: resp
            .get("refresh_token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        expires_in_secs: expires_in,
        extra_writes: vec![],
    })
}

/// loadCodeAssist response → (project id, plan tier).
pub fn parse_load(body: &str) -> Result<(String, Option<String>), ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let project = match v.get("cloudaicompanionProject") {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Object(o)) => o
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string()),
        _ => None,
    };
    let plan = fetch::json_path(&v, "currentTier.id")
        .and_then(|t| t.as_str())
        .map(|t| {
            // "free-tier" / "g1-pro-tier" / "legacy-tier" → short display name
            match t {
                "free-tier" => "Free".to_string(),
                "legacy-tier" => "Legacy".to_string(),
                s if s.contains("pro") => "Pro".to_string(),
                s => s.trim_end_matches("-tier").to_string(),
            }
        });
    let project = project.ok_or(ProviderError::ParseFailed)?;
    Ok((project, plan))
}

fn bucket_label(model_id: &str) -> String {
    let m = model_id.to_lowercase();
    if m.contains("pro") {
        "Pro 日".into()
    } else if m.contains("flash") {
        "Flash 日".into()
    } else {
        // unknown model family: last segment, clamped by sanitize() anyway
        model_id.rsplit('-').next().unwrap_or(model_id).to_string()
    }
}

/// retrieveUserQuota response → windows (per-model daily buckets).
/// Design spec: when there are >2 buckets, show only the worst one.
pub fn parse_quota(body: &str) -> Result<Vec<QuotaWindow>, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let buckets = v
        .get("buckets")
        .and_then(|b| b.as_array())
        .ok_or(ProviderError::ParseFailed)?;
    let mut windows: Vec<QuotaWindow> = vec![];
    for b in buckets {
        let remaining = b.get("remainingFraction").and_then(fetch::as_f64);
        let model = b.get("modelId").and_then(|m| m.as_str()).unwrap_or("");
        if let Some(r) = remaining {
            windows.push(QuotaWindow {
                label: bucket_label(model),
                used_percent: (1.0 - r) * 100.0,
                resets_at: b.get("resetTime").and_then(super::parse_reset),
            });
        }
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    // worst first; >2 buckets => keep only the worst (design spec §layout)
    windows.sort_by(|a, b| b.used_percent.partial_cmp(&a.used_percent).unwrap_or(std::cmp::Ordering::Equal));
    if windows.len() > 2 {
        windows.truncate(1);
    }
    Ok(windows)
}

fn map_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthExpired,
        403 => ProviderError::UnsupportedClient, // Antigravity migration
        429 => ProviderError::RateLimited { retry_after: None },
        _ => ProviderError::Network,
    }
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let path = cred_path()?;
    let (token, _source) = oauth::resolve_oauth_token(&path, &CRED_SPEC, refresh_call).await?;

    let load_body = serde_json::json!({
        "metadata": { "ideType": "IDE_UNSPECIFIED", "pluginType": "GEMINI" }
    });
    let (status, body) =
        fetch::post_json(&format!("{}:loadCodeAssist", BASE), &token, &load_body).await
            .map_err(|_| ProviderError::Network)?;
    if !(200..300).contains(&status) {
        return Err(map_status(status));
    }
    let (project, plan) = parse_load(&body)?;

    let quota_body = serde_json::json!({ "project": project });
    let (status, body) =
        fetch::post_json(&format!("{}:retrieveUserQuota", BASE), &token, &quota_body).await
            .map_err(|_| ProviderError::Network)?;
    if !(200..300).contains(&status) {
        return Err(map_status(status));
    }
    let windows = parse_quota(&body)?;
    Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "official"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_load_code_assist_string_project() {
        let body = r#"{
          "cloudaicompanionProject": "rising-fact-p41fc",
          "currentTier": { "id": "free-tier", "name": "Gemini Code Assist for individuals" }
        }"#;
        let (p, plan) = parse_load(body).unwrap();
        assert_eq!(p, "rising-fact-p41fc");
        assert_eq!(plan.as_deref(), Some("Free"));
    }

    #[test]
    fn parses_load_code_assist_object_project() {
        let body = r#"{ "cloudaicompanionProject": { "id": "proj-123" }, "currentTier": { "id": "g1-pro-tier" } }"#;
        let (p, plan) = parse_load(body).unwrap();
        assert_eq!(p, "proj-123");
        assert_eq!(plan.as_deref(), Some("Pro"));
    }

    #[test]
    fn parses_quota_buckets() {
        let body = r#"{
          "buckets": [
            { "modelId": "gemini-2.5-pro", "remainingFraction": 0.28, "resetTime": "2026-09-03T00:00:00Z" },
            { "modelId": "gemini-2.5-flash", "remainingFraction": 0.95, "resetTime": "2026-09-03T00:00:00Z" }
          ]
        }"#;
        let w = parse_quota(body).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].label, "Pro 日");
        assert!((w[0].used_percent - 72.0).abs() < 0.01);
        assert!(w[0].resets_at.is_some());
        assert_eq!(w[1].label, "Flash 日");
        assert!((w[1].used_percent - 5.0).abs() < 0.01);
    }

    #[test]
    fn more_than_two_buckets_keeps_only_worst() {
        let body = r#"{
          "buckets": [
            { "modelId": "gemini-2.5-flash", "remainingFraction": 0.9 },
            { "modelId": "gemini-2.5-pro", "remainingFraction": 0.1 },
            { "modelId": "gemini-2.5-flash-lite", "remainingFraction": 0.5 }
          ]
        }"#;
        let w = parse_quota(body).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].label, "Pro 日");
        assert!((w[0].used_percent - 90.0).abs() < 0.01);
    }

    #[test]
    fn tolerates_string_fraction_and_missing_reset() {
        let body = r#"{ "buckets": [ { "modelId": "gemini-2.5-pro", "remainingFraction": "0.5" } ] }"#;
        let w = parse_quota(body).unwrap();
        assert_eq!(w.len(), 1);
        assert!((w[0].used_percent - 50.0).abs() < 0.01);
        assert_eq!(w[0].resets_at, None);
    }

    #[test]
    fn malformed_is_parse_error() {
        assert!(matches!(parse_quota("not json"), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse_quota(r#"{"foo": 1}"#), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse_load(r#"{"foo": 1}"#), Err(ProviderError::ParseFailed)));
    }
}
