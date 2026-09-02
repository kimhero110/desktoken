// Gemini — two credential paths, two quota channels:
//
// A) gemini-cli (~/.gemini/oauth_creds.json): classic Code Assist flow —
//    POST cloudcode-pa.googleapis.com/v1internal:loadCodeAssist → project,
//    then :retrieveUserQuota → per-model daily buckets.
// B) Antigravity IDE (Windows Credential Manager target "gemini:antigravity"):
//    the Code Assist quota API refuses consumer accounts (free-tier
//    UNSUPPORTED_CLIENT, retrieveUserQuota 403 SUBSCRIPTION_REQUIRED), but
//    Antigravity's own channel works: POST
//    daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels with
//    header "User-Agent: antigravity/*" (the endpoint gates on it) — each
//    model entry carries quotaInfo{remainingFraction, resetTime}. All
//    gemini-* models share one daily bucket; we report the worst.
//
// OAuth refresh: gemini-cli public client creds, shared 6-step protocol.
// Antigravity credentials are READ-ONLY (PLAN.md 凭据只读优先): when its
// access token is near expiry we let the IDE refresh it (it does, hourly) —
// we never write the foreign entry; a failed self-refresh maps to AuthExpired
// and the next poll picks up the IDE's fresh token.
// Poll interval clamped to >= 5 min (PLAN.md decision #12).
use super::{ProviderError, QuotaSnapshot, QuotaWindow};
use crate::fetch;
use crate::oauth;
use serde_json::Value;

pub const ID: &str = "gemini";
pub const NAME: &str = "Gemini";
const BASE_CLASSIC: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const BASE_DAILY: &str = "https://daily-cloudcode-pa.googleapis.com/v1internal";
const ANTIGRAVITY_UA: &str = "antigravity/1.0";

const CLI_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "access_token",
    refresh_path: "refresh_token",
    expires_path: Some("expiry_date"),
    expiry_unit: oauth::ExpiryUnit::Millis,
};

const ANTIGRAVITY_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "token.access_token",
    refresh_path: "token.refresh_token",
    expires_path: Some("token.expiry"), // RFC3339 string
    expiry_unit: oauth::ExpiryUnit::Seconds,
};

const ANTIGRAVITY_TARGET: &str = "gemini:antigravity";

enum CredSource {
    CliFile,
    Antigravity,
}

/// gemini-cli credential file (preferred); Antigravity keyring as fallback.
async fn resolve_token() -> Result<(String, CredSource), ProviderError> {
    let home = crate::credentials::home().ok_or(ProviderError::CredentialMissing)?;
    let p = home.join(".gemini/oauth_creds.json");
    if p.exists() {
        let (t, _) = oauth::resolve_oauth_token(&p, &CLI_SPEC, refresh_call).await?;
        return Ok((t, CredSource::CliFile));
    }
    if crate::credentials::read_foreign_cred(ANTIGRAVITY_TARGET).is_some() {
        let (t, _) =
            oauth::resolve_oauth_keyring(ANTIGRAVITY_TARGET, &ANTIGRAVITY_SPEC, refresh_call)
                .await?;
        return Ok((t, CredSource::Antigravity));
    }
    Err(ProviderError::CredentialMissing)
}

/// Google OAuth refresh with the public gemini-cli client credentials.
/// Note: Antigravity's refresh_token is issued to its own OAuth client, so a
/// self-refresh with these creds can fail with invalid_grant; by design the
/// next poll then re-reads the IDE-refreshed credential (see module docs).
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

// ---------------------------------------------------------------------------
// A) gemini-cli classic flow
// ---------------------------------------------------------------------------

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
        .map(|t| match t {
            "free-tier" => "Free".to_string(),
            "legacy-tier" => "Legacy".to_string(),
            s if s.contains("pro") => "Pro".to_string(),
            s => s.trim_end_matches("-tier").to_string(),
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
    windows.sort_by(|a, b| b.used_percent.partial_cmp(&a.used_percent).unwrap_or(std::cmp::Ordering::Equal));
    if windows.len() > 2 {
        windows.truncate(1);
    }
    Ok(windows)
}

// ---------------------------------------------------------------------------
// B) Antigravity flow: quota rides on fetchAvailableModels
// ---------------------------------------------------------------------------

/// fetchAvailableModels response → one window per shared gemini-* bucket
/// (in practice they share a single rolling bucket; we report the worst).
/// The bucket's resetTime is rolling-window semantics: the server leaves it
/// in the past while the fraction keeps decaying (observed 2026-09-02), so a
/// stale resetTime is dropped — showing "等待刷新…" forever is worse than "—".
pub fn parse_models(body: &str) -> Result<Vec<QuotaWindow>, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let models = v
        .get("models")
        .and_then(|m| m.as_object())
        .ok_or(ProviderError::ParseFailed)?;
    let now = super::now_secs();
    let mut worst: Option<QuotaWindow> = None;
    for (key, m) in models.iter() {
        if !key.starts_with("gemini-") {
            continue;
        }
        let q = match m.get("quotaInfo") {
            Some(q) => q,
            None => continue,
        };
        let Some(remaining) = q.get("remainingFraction").and_then(fetch::as_f64) else {
            continue;
        };
        let used = (1.0 - remaining) * 100.0;
        let resets_at = q
            .get("resetTime")
            .and_then(super::parse_reset)
            .filter(|t| *t > now); // stale rolling-window markers carry no info
        let w = QuotaWindow {
            label: "滚动".into(),
            used_percent: used,
            resets_at,
        };
        worst = Some(match worst {
            None => w,
            Some(cur) if w.used_percent > cur.used_percent => w,
            Some(cur) => cur,
        });
    }
    match worst {
        Some(w) => Ok(vec![w]),
        None => Err(ProviderError::ParseFailed),
    }
}

fn map_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthExpired,
        403 => ProviderError::UnsupportedClient, // Antigravity migration etc.
        429 => ProviderError::RateLimited { retry_after: None },
        _ => ProviderError::Network,
    }
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    let (token, source) = resolve_token().await?;

    match source {
        CredSource::Antigravity => {
            let (status, body) = fetch::post_json_ua(
                &format!("{}:fetchAvailableModels", BASE_DAILY),
                &token,
                Some(ANTIGRAVITY_UA),
                &serde_json::json!({}),
            )
            .await
            .map_err(|_| ProviderError::Network)?;
            if !(200..300).contains(&status) {
                return Err(map_status(status));
            }
            let windows = parse_models(&body)?;
            Ok(QuotaSnapshot::ok(ID, NAME, Some("Antigravity".into()), windows, "official"))
        }
        CredSource::CliFile => {
            let load_body = serde_json::json!({
                "metadata": { "ideType": "IDE_UNSPECIFIED", "pluginType": "GEMINI" }
            });
            let (status, body) = fetch::post_json_ua(
                &format!("{}:loadCodeAssist", BASE_CLASSIC),
                &token,
                None,
                &load_body,
            )
            .await
            .map_err(|_| ProviderError::Network)?;
            if !(200..300).contains(&status) {
                return Err(map_status(status));
            }
            let (project, plan) = parse_load(&body)?;

            let quota_body = serde_json::json!({ "project": project });
            let (status, body) = fetch::post_json_ua(
                &format!("{}:retrieveUserQuota", BASE_CLASSIC),
                &token,
                None,
                &quota_body,
            )
            .await
            .map_err(|_| ProviderError::Network)?;
            if !(200..300).contains(&status) {
                return Err(map_status(status));
            }
            let windows = parse_quota(&body)?;
            Ok(QuotaSnapshot::ok(ID, NAME, plan, windows, "official"))
        }
    }
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

    // ---- Antigravity fetchAvailableModels channel ----

    #[test]
    fn parses_antigravity_models_shared_bucket() {
        // real 2026-09 capture shape: all gemini-* share one bucket
        let body = r#"{
          "models": {
            "chat_20706": { "isInternal": true, "quotaInfo": { "remainingFraction": 1 } },
            "gemini-2.5-pro": { "displayName": "Gemini 2.5 Pro",
              "quotaInfo": { "remainingFraction": 0.9084101, "resetTime": "2099-09-02T20:43:09Z" } },
            "gemini-3.6-flash-medium": { "displayName": "Gemini 3.6 Flash (Medium)",
              "quotaInfo": { "remainingFraction": 0.9084101, "resetTime": "2099-09-02T20:43:09Z" } },
            "claude-opus-4-6-thinking": { "displayName": "Claude Opus 4.6 (Thinking)",
              "quotaInfo": { "remainingFraction": 1, "resetTime": "2099-09-02T22:05:03Z" } }
          }
        }"#;
        let w = parse_models(body).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].label, "滚动");
        assert!((w[0].used_percent - 9.16).abs() < 0.1);
        assert!(w[0].resets_at.is_some(), "future resetTime survives");
    }

    #[test]
    fn antigravity_stale_reset_time_is_dropped() {
        // rolling-window marker left in the past (observed live 2026-09-02):
        // must not produce an eternal "等待刷新…" countdown
        let body = r#"{
          "models": {
            "gemini-a": { "quotaInfo": { "remainingFraction": 0.5, "resetTime": "2020-01-01T00:00:00Z" } }
          }
        }"#;
        let w = parse_models(body).unwrap();
        assert_eq!(w[0].resets_at, None);
    }

    #[test]
    fn antigravity_models_worst_wins() {
        let body = r#"{
          "models": {
            "gemini-a": { "quotaInfo": { "remainingFraction": 0.9 } },
            "gemini-b": { "quotaInfo": { "remainingFraction": 0.2, "resetTime": 4000000000 } }
          }
        }"#;
        let w = parse_models(body).unwrap();
        assert!((w[0].used_percent - 80.0).abs() < 0.01);
        assert_eq!(w[0].resets_at, Some(4000000000));
    }

    #[test]
    fn antigravity_models_missing_quota_is_parse_error() {
        assert!(matches!(parse_models(r#"{"models": {"chat_1": {}}}"#), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse_models("not json"), Err(ProviderError::ParseFailed)));
    }
}
