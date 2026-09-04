// QuotaBar — E1 minimal version check (PLAN.md eng review #11).
// ToS-gated (caller ensures consent), GitHub API + releases-page redirect
// checked concurrently, first success wins; 5s hard timeout, silent failure;
// result cached in settings for 24h. Skipped versions are not re-reported.
use crate::settings;

const CURRENT: &str = env!("CARGO_PKG_VERSION");
const API: &str = "https://api.github.com/repos/kimhero110/desktoken/releases/latest";
const LATEST_PAGE: &str = "https://github.com/kimhero110/desktoken/releases/latest";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct UpdateInfo {
    pub version: String, // "0.2.0" (no v prefix)
    pub url: String,
}

/// semver-ish compare: true if `candidate` > `current` ("0.1.10" > "0.1.9").
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.trim_start_matches('v')
            .split('-') // skip pre-release suffixes
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    }
    parts(candidate) > parts(current)
}

async fn fetch_via_api() -> Option<UpdateInfo> {
    let client = crate::fetch::http_client();
    let resp = client
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = v.get("tag_name").and_then(|t| t.as_str())?;
    // skip pre-releases (PLAN: 跳预发布)
    if v.get("prerelease").and_then(|p| p.as_bool()).unwrap_or(false) {
        return None;
    }
    Some(UpdateInfo {
        version: tag.trim_start_matches('v').to_string(),
        url: v
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or(LATEST_PAGE)
            .to_string(),
    })
}

/// Fallback path: the releases/latest page 302s to the newest tag
/// (works even when api.github.com is rate-limited).
async fn fetch_via_redirect() -> Option<UpdateInfo> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;
    let resp = client.get(LATEST_PAGE).send().await.ok()?;
    if resp.status().as_u16() != 302 {
        return None;
    }
    let loc = resp.headers().get("location")?.to_str().ok()?.to_string();
    let tag = loc.rsplit('/').next()?.trim_start_matches('v');
    if tag.is_empty() || !tag.chars().next()?.is_ascii_digit() {
        return None;
    }
    Some(UpdateInfo {
        version: tag.to_string(),
        url: loc,
    })
}

/// Concurrent double-check; first success wins. Silent None on total failure.
pub async fn check_now() -> Option<UpdateInfo> {
    tokio::select! {
        a = fetch_via_api() => a,
        b = fetch_via_redirect() => b,
    }
}

/// Startup/periodic entry: respect the 24h cache and the skip list, emit
/// "update-available" to the bar when a newer version exists. Never blocks,
/// never errors out loud (silent failure per spec).
pub fn maybe_check(app: tauri::AppHandle, force: bool) {
    tauri::async_runtime::spawn(async move {
        let now = crate::providers::now_secs();
        let s = settings::load();
        let fresh_cache = s
            .update_checked_at
            .map(|t| now - t < 24 * 3600)
            .unwrap_or(false);
        let info: Option<UpdateInfo> = if fresh_cache && !force {
            s.latest_version.clone().map(|v| UpdateInfo {
                url: format!("https://github.com/kimhero110/desktoken/releases/tag/v{}", v),
                version: v,
            })
        } else {
            // network OUTSIDE the settings lock
            let r = check_now().await;
            settings::edit(|s| {
                s.update_checked_at = Some(now);
                if let Some(ref r) = r {
                    s.latest_version = Some(r.version.clone());
                }
            });
            r
        };
        if let Some(i) = info {
            let skipped = s.skipped_version.as_deref() == Some(i.version.as_str());
            if is_newer(&i.version, CURRENT) && !skipped {
                use tauri::Emitter;
                let _ = app.emit("update-available", i);
            }
        }
    });
}

#[tauri::command]
pub fn skip_version(version: String) {
    crate::settings::edit(|s| s.skipped_version = Some(version));
}

#[tauri::command]
pub fn current_version() -> String {
    CURRENT.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }
}
