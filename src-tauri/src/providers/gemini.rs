// Gemini — two channels:
//
// A) gemini-cli (~/.gemini/oauth_creds.json): classic Code Assist flow —
//    POST cloudcode-pa.googleapis.com/v1internal:loadCodeAssist → project,
//    then :retrieveUserQuota → per-model daily buckets.
// B) Antigravity IDE: the Code Assist quota API refuses consumer accounts
//    (free-tier UNSUPPORTED_CLIENT, retrieveUserQuota 403), and the quotaInfo
//    on fetchAvailableModels is a DIFFERENT number from the IDE's own panel.
//    The panel's real source is the language server's local ConnectRPC:
//      POST https://127.0.0.1:<port>/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary
//      header x-codeium-csrf-token (from the LS process command line)
//    Returns groups → buckets {window: "weekly"|"5h", remainingFraction,
//    resetTime} — exactly what the IDE panel shows.
//    Discovery: the LS listens on random loopback ports; we resolve port +
//    CSRF token from the running process (PowerShell CIM + TCP table), once
//    per poll (5 min cadence — cheap and self-healing across IDE restarts).
//    IDE not running → IdeNotRunning row (honest, instead of a wrong number).
//
// Poll interval clamped to >= 5 min (PLAN.md decision #12).
use super::{ProviderError, QuotaSnapshot, QuotaWindow};
use crate::fetch;
use crate::oauth;
use serde_json::Value;

pub const ID: &str = "gemini";
pub const NAME: &str = "Gemini";
const BASE_CLASSIC: &str = "https://cloudcode-pa.googleapis.com/v1internal";

const CLI_SPEC: oauth::OAuthFileSpec = oauth::OAuthFileSpec {
    access_path: "access_token",
    refresh_path: "refresh_token",
    expires_path: Some("expiry_date"),
    expiry_unit: oauth::ExpiryUnit::Millis,
};

const ANTIGRAVITY_TARGET: &str = "gemini:antigravity";
const LS_RPC: &str = "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";

/// gemini-cli credential file path, if present.
fn cli_cred_path() -> Option<std::path::PathBuf> {
    let p = crate::credentials::home()?.join(".gemini/oauth_creds.json");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn antigravity_installed() -> bool {
    crate::credentials::read_foreign_cred(ANTIGRAVITY_TARGET).is_some()
}

// ---------------------------------------------------------------------------
// B) Antigravity local language-server channel
// ---------------------------------------------------------------------------

struct LsEndpoint {
    ports: Vec<u16>,
    csrf: String,
}

/// Discover the running Antigravity language server: CSRF token from its
/// command line, loopback listen ports from the TCP table.
#[cfg(target_os = "windows")]
fn discover_ls() -> Option<LsEndpoint> {
    const SCRIPT: &str = r#"
$p = Get-CimInstance Win32_Process -Filter "Name='language_server.exe'" |
     Where-Object { $_.CommandLine -match '--override_ide_name antigravity' } |
     Select-Object -First 1
if (-not $p) { exit 1 }
$ports = (Get-NetTCPConnection -OwningProcess $p.ProcessId -State Listen -ErrorAction SilentlyContinue |
          Where-Object { $_.LocalAddress -eq '127.0.0.1' } |
          Select-Object -ExpandProperty LocalPort) -join ','
$csrf = ([regex]::Match($p.CommandLine, '--csrf_token ([0-9a-fA-F-]+)')).Groups[1].Value
"$ports|$csrf"
"#;
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (ports, csrf) = text.split_once('|')?;
    let ports: Vec<u16> = ports.split(',').filter_map(|p| p.parse().ok()).collect();
    if ports.is_empty() || csrf.is_empty() {
        return None;
    }
    Some(LsEndpoint {
        ports,
        csrf: csrf.to_string(),
    })
}

#[cfg(not(target_os = "windows"))]
fn discover_ls() -> Option<LsEndpoint> {
    // macOS/Linux: find the LS via ps, ports via lsof
    let out = std::process::Command::new("ps")
        .args(["-eo", "pid=,command="])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.contains("language_server") || !line.contains("--override_ide_name antigravity") {
            continue;
        }
        let pid: u32 = line.trim().split_whitespace().next()?.parse().ok()?;
        let csrf = line
            .split("--csrf_token ")
            .nth(1)?
            .split_whitespace()
            .next()?
            .to_string();
        let lsof = std::process::Command::new("lsof")
            .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let ltext = String::from_utf8_lossy(&lsof.stdout);
        let mut ports = vec![];
        for l in ltext.lines() {
            if let Some(idx) = l.find("127.0.0.1:") {
                let digits: String = l[idx + 10..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(p) = digits.parse::<u16>() {
                    ports.push(p);
                }
            }
        }
        if ports.is_empty() || csrf.is_empty() {
            continue;
        }
        return Some(LsEndpoint { ports, csrf });
    }
    None
}

/// The LS serves HTTPS with a self-signed cert on loopback — a dedicated
/// client scoped to this one purpose (never used for internet traffic).
fn ls_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

async fn fetch_via_ls() -> Result<Vec<QuotaWindow>, ProviderError> {
    let ep = tokio::task::spawn_blocking(discover_ls)
        .await
        .map_err(|_| ProviderError::IdeNotRunning)?
        .ok_or(ProviderError::IdeNotRunning)?;
    let client = ls_client();
    for port in &ep.ports {
        let url = format!("https://127.0.0.1:{}{}", port, LS_RPC);
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-codeium-csrf-token", &ep.csrf)
            .body("{}")
            .send()
            .await;
        let Ok(resp) = resp else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let body = resp.text().await.map_err(|_| ProviderError::Network)?;
        return parse_ls_quota(&body);
    }
    Err(ProviderError::IdeNotRunning)
}

/// RetrieveUserQuotaSummary response → Gemini group windows (5h + weekly).
pub fn parse_ls_quota(body: &str) -> Result<Vec<QuotaWindow>, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let groups = fetch::json_path(&v, "response.groups")
        .and_then(|g| g.as_array())
        .ok_or(ProviderError::ParseFailed)?;
    let gemini = groups
        .iter()
        .find(|g| {
            g.get("displayName")
                .and_then(|d| d.as_str())
                .map(|d| d.contains("Gemini"))
                .unwrap_or(false)
        })
        .ok_or(ProviderError::ParseFailed)?;
    let buckets = gemini
        .get("buckets")
        .and_then(|b| b.as_array())
        .ok_or(ProviderError::ParseFailed)?;
    let mut windows: Vec<QuotaWindow> = vec![];
    for b in buckets {
        let label = match b.get("window").and_then(|w| w.as_str()) {
            Some("5h") => "5h",
            Some("weekly") => "周",
            _ => continue,
        };
        let Some(remaining) = b.get("remainingFraction").and_then(fetch::as_f64) else {
            continue;
        };
        windows.push(QuotaWindow {
            label: label.into(),
            used_percent: (1.0 - remaining) * 100.0,
            resets_at: b.get("resetTime").and_then(super::parse_reset),
        });
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    // display order: 5h first, then weekly (matches other providers)
    windows.sort_by_key(|w| if w.label == "5h" { 0 } else { 1 });
    Ok(windows)
}

// ---------------------------------------------------------------------------
// A) gemini-cli classic Code Assist flow
// ---------------------------------------------------------------------------

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

fn map_status(status: u16) -> ProviderError {
    match status {
        401 => ProviderError::AuthExpired,
        403 => ProviderError::UnsupportedClient, // consumer tier removed
        429 => ProviderError::RateLimited { retry_after: None },
        _ => ProviderError::Network,
    }
}

pub async fn fetch_snapshot() -> Result<QuotaSnapshot, ProviderError> {
    // gemini-cli credential file → classic Code Assist flow. Otherwise the
    // Antigravity LS channel (works whenever the IDE is running; no keyring
    // read needed — the LS holds the auth).
    if cli_cred_path().is_none() {
        return match fetch_via_ls().await {
            Ok(windows) => Ok(QuotaSnapshot::ok(ID, NAME, Some("Antigravity".into()), windows, "official")),
            Err(e) => {
                if antigravity_installed() {
                    Err(ProviderError::IdeNotRunning)
                } else {
                    Err(e)
                }
            }
        };
    }
    let path = cli_cred_path().unwrap();
    let (token, _source) = oauth::resolve_oauth_token(&path, &CLI_SPEC, refresh_call).await?;

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

    // ---- Antigravity LS channel ----

    #[test]
    fn parses_ls_quota_summary() {
        // real 2026-09-03 capture from RetrieveUserQuotaSummary
        let body = r#"{"response":{"groups":[
          {"displayName":"Gemini Models","buckets":[
            {"bucketId":"gemini-weekly","window":"weekly","remainingFraction":0.5305139,"resetTime":"2026-09-06T07:40:17Z"},
            {"bucketId":"gemini-5h","window":"5h","remainingFraction":0.9217109,"resetTime":"2026-09-03T07:46:37Z"}]},
          {"displayName":"Claude and GPT models","buckets":[
            {"bucketId":"3p-weekly","window":"weekly","remainingFraction":1,"resetTime":"2026-09-10T02:54:21Z"},
            {"bucketId":"3p-5h","window":"5h","remainingFraction":1,"resetTime":"2026-09-03T07:54:21Z"}]}
        ]}}"#;
        let w = parse_ls_quota(body).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].label, "5h");
        assert!((w[0].used_percent - 7.83).abs() < 0.1);
        assert_eq!(w[1].label, "周");
        assert!((w[1].used_percent - 46.95).abs() < 0.1);
        assert!(w[0].resets_at.is_some() && w[1].resets_at.is_some());
    }

    #[test]
    fn ls_quota_without_gemini_group_is_parse_error() {
        let body = r#"{"response":{"groups":[{"displayName":"Claude and GPT models","buckets":[]}]}}"#;
        assert!(matches!(parse_ls_quota(body), Err(ProviderError::ParseFailed)));
        assert!(matches!(parse_ls_quota("not json"), Err(ProviderError::ParseFailed)));
    }
}
