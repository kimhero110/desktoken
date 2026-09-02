// DeskToken — shared OAuth credential protocol (PLAN.md §3 "6 步并发协议").
//
// Steps: re-read the credential file every round → refresh only when the
// token expires within 5 min → per-path single-flight mutex + re-check inside
// the lock → refresh via the provider's token endpoint → compare-before-write
// (if the file changed while we refreshed, the official CLI won: adopt its
// token, drop ours) → atomic tmp+rename write-back with retries. If the
// write-back ultimately fails the fresh token is still used in memory
// (PendingWriteback lite) — we never lose a working token over a file lock.
//
// The HTTP refresh call itself is injected by each provider so tests can stub
// it and simulate adversarial CLI behavior (spike B).
use crate::fetch;
use crate::providers::{now_secs, EpochSecs, ProviderError};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Refresh when the token expires within this window.
const SKEW_SECS: i64 = 300;
/// Torn-read tolerance: silent retries on JSON parse failure (~150ms apart).
const TORN_RETRIES: u32 = 3;
/// Write-back rename retries (production): 100ms * 2^n, ~6.3s total.
const RENAME_ATTEMPTS: u32 = 6;
const RENAME_BASE_DELAY_MS: u64 = 100;

/// How the credential file stores the expiry timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpiryUnit {
    Seconds,
    Millis,
}

/// Static description of one provider's OAuth credential file layout.
/// Paths are jq-lite dotted segments (see fetch::json_path).
pub struct OAuthFileSpec {
    pub access_path: &'static str,
    pub refresh_path: &'static str,
    /// File-side expiry field; None => derive expiry from the access-token JWT.
    pub expires_path: Option<&'static str>,
    pub expiry_unit: ExpiryUnit,
}

/// Successful refresh payload from a provider token endpoint.
pub struct RefreshResult {
    pub access_token: String,
    /// New refresh token; None => keep the old one.
    pub refresh_token: Option<String>,
    pub expires_in_secs: Option<i64>,
    /// Extra fields the provider wants updated on write-back
    /// (jq-lite path, string value), e.g. Codex "last_refresh".
    pub extra_writes: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub enum RefreshFailure {
    /// 400/401 from the token endpoint (invalid_grant etc.).
    InvalidGrant,
    Network,
    Parse,
}

/// Read + parse the credential file with torn-read tolerance: both open
/// failures (Windows MoveFileEx REPLACE_EXISTING has a brief delete+rename
/// window where the path vanishes) and JSON parse failures (CLI mid-write)
/// are retried silently; only persistent failure reports an error.
fn read_doc(path: &Path) -> Result<(String, Value), ProviderError> {
    let mut last_read_failed = false;
    for attempt in 0..TORN_RETRIES {
        match std::fs::read_to_string(path) {
            Ok(raw) => {
                last_read_failed = false;
                match serde_json::from_str::<Value>(&raw) {
                    Ok(v) => return Ok((raw, v)),
                    Err(_) => {}
                }
            }
            Err(_) => last_read_failed = true,
        }
        if attempt + 1 < TORN_RETRIES {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    if last_read_failed {
        Err(ProviderError::CredentialMissing)
    } else {
        Err(ProviderError::CredentialCorrupt { torn: true })
    }
}

fn json_path_str<'a>(doc: &'a Value, path: &str) -> Option<&'a str> {
    fetch::json_path(doc, path).and_then(|v| v.as_str())
}

/// Decode the `exp` claim from a JWT without verifying the signature
/// (we only need the expiry the issuer put there).
pub fn jwt_exp(token: &str) -> Option<EpochSecs> {
    use base64::Engine;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp").and_then(|e| e.as_i64())
}

/// (access_token, refresh_token, expiry epoch secs) from a credential doc.
fn extract(doc: &Value, spec: &OAuthFileSpec) -> (Option<String>, Option<String>, Option<EpochSecs>) {
    let access = json_path_str(doc, spec.access_path).map(|s| s.to_string());
    let refresh = json_path_str(doc, spec.refresh_path).map(|s| s.to_string());
    let expires = match spec.expires_path {
        Some(p) => fetch::json_path(doc, p)
            .and_then(fetch::as_f64)
            .map(|x| match spec.expiry_unit {
                ExpiryUnit::Seconds => x as i64,
                ExpiryUnit::Millis => (x / 1000.0) as i64,
            }),
        None => access.as_deref().and_then(jwt_exp),
    };
    (access, refresh, expires)
}

fn is_fresh(expires: Option<EpochSecs>) -> bool {
    expires.map(|e| e > now_secs() + SKEW_SECS).unwrap_or(false)
}

fn set_path(doc: &mut Value, path: &str, val: Value) {
    let segs: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return;
    }
    let mut cur = doc;
    for (i, seg) in segs.iter().enumerate() {
        if i == segs.len() - 1 {
            if let Some(obj) = cur.as_object_mut() {
                obj.insert(seg.to_string(), val);
            }
            return;
        }
        if cur.get(seg).is_none() {
            cur[*seg] = Value::Object(serde_json::Map::new());
        }
        match cur.get_mut(seg) {
            Some(next) => cur = next,
            None => return,
        }
    }
}

/// Per-path async mutex: refresh single-flight. One map entry per credential
/// file, so Kimi refreshing never blocks Codex refreshing.
fn path_mutex(path: &Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    use std::sync::{Arc, Mutex, OnceLock};
    static MAP: OnceLock<Mutex<std::collections::HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let map = MAP.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    map.lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// Atomic write-back: write sibling tmp file, then rename over the original
/// with bounded retries (Windows file-lock tolerance). Failure here is NOT
/// fatal to the caller — the token still works in memory.
fn write_back(path: &Path, doc: &Value, attempts: u32, base_delay_ms: u64) -> Result<(), ()> {
    let body = serde_json::to_string_pretty(doc).map_err(|_| ())?;
    let tmp = path.with_extension("desktoken-tmp");
    std::fs::write(&tmp, body).map_err(|_| ())?;
    let mut delay = std::time::Duration::from_millis(base_delay_ms);
    for attempt in 0..attempts {
        match std::fs::rename(&tmp, path) {
            Ok(_) => return Ok(()),
            Err(_) => {
                if attempt + 1 < attempts {
                    std::thread::sleep(delay);
                    delay *= 2;
                }
            }
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Err(())
}

/// Resolve a usable access token for an OAuth-file credential provider.
/// Returns (access_token, source). See module docs for the protocol.
pub async fn resolve_oauth_token<F, Fut>(
    path: &Path,
    spec: &OAuthFileSpec,
    do_refresh: F,
) -> Result<(String, &'static str), ProviderError>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<RefreshResult, RefreshFailure>>,
{
    resolve_oauth_token_with(path, spec, do_refresh, RENAME_ATTEMPTS, RENAME_BASE_DELAY_MS).await
}

/// Inner implementation with test-tunable write-back retry policy.
async fn resolve_oauth_token_with<F, Fut>(
    path: &Path,
    spec: &OAuthFileSpec,
    do_refresh: F,
    rename_attempts: u32,
    rename_delay_ms: u64,
) -> Result<(String, &'static str), ProviderError>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<RefreshResult, RefreshFailure>>,
{
    // Step 1: read every round (never cache credentials across polls).
    let (_raw, doc) = read_doc(path)?;
    let (access, refresh, expires) = extract(&doc, spec);
    if let (Some(t), true) = (&access, is_fresh(expires)) {
        return Ok((t.clone(), "official"));
    }
    let Some(rt) = refresh else {
        // No refresh token: try the access token anyway (unknown expiry);
        // a 401 from the usage endpoint maps to AuthExpired upstream.
        return match access {
            Some(t) => Ok((t, "official")),
            None => Err(ProviderError::CredentialMissing),
        };
    };

    // Step 2: single-flight per credential file.
    let mutex = path_mutex(path);
    let _guard = mutex.lock().await;

    // Step 3: re-read inside the lock — another task (or the CLI itself) may
    // have refreshed while we waited.
    let (raw2, doc2) = read_doc(path)?;
    let (a2, r2, e2) = extract(&doc2, spec);
    if let (Some(t), true) = (&a2, is_fresh(e2)) {
        return Ok((t.clone(), "official"));
    }
    let rt = r2.unwrap_or(rt);

    // Step 4: refresh.
    let rr = match do_refresh(rt).await {
        Ok(rr) => rr,
        Err(RefreshFailure::InvalidGrant) => {
            // The CLI may have rotated the pair concurrently: re-read once
            // before declaring the credential dead.
            if let Ok((_, doc3)) = read_doc(path) {
                let (a3, _, e3) = extract(&doc3, spec);
                if let (Some(t), true) = (&a3, is_fresh(e3)) {
                    return Ok((t.clone(), "official"));
                }
            }
            return Err(ProviderError::AuthExpired);
        }
        Err(RefreshFailure::Network) => return Err(ProviderError::Network),
        Err(RefreshFailure::Parse) => return Err(ProviderError::ParseFailed),
    };

    // Step 5: merge into the latest doc we read.
    let mut doc = doc2;
    set_path(&mut doc, spec.access_path, Value::String(rr.access_token.clone()));
    if let Some(nrt) = &rr.refresh_token {
        set_path(&mut doc, spec.refresh_path, Value::String(nrt.clone()));
    }
    if let (Some(p), Some(secs)) = (spec.expires_path, rr.expires_in_secs) {
        let v = match spec.expiry_unit {
            ExpiryUnit::Seconds => now_secs() + secs,
            ExpiryUnit::Millis => (now_secs() + secs) * 1000,
        };
        set_path(&mut doc, p, Value::Number(v.into()));
    }
    for (p, val) in &rr.extra_writes {
        set_path(&mut doc, p, Value::String(val.clone()));
    }

    // Step 6: compare-before-write — if the file changed since our in-lock
    // read, the CLI refreshed concurrently; adopt its token, drop ours.
    if let Ok(cur) = std::fs::read_to_string(path) {
        if cur != raw2 {
            if let Ok(cd) = serde_json::from_str::<Value>(&cur) {
                let (ca, _, _) = extract(&cd, spec);
                if let Some(t) = ca {
                    return Ok((t, "official"));
                }
            }
        }
    }

    // Write-back failure is non-fatal: use the fresh token in memory
    // (PendingWriteback lite; PLAN.md eng review #3).
    let _ = write_back(path, &doc, rename_attempts, rename_delay_ms);
    Ok((rr.access_token, "official"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    pub(crate) const SPEC: OAuthFileSpec = OAuthFileSpec {
        access_path: "access_token",
        refresh_path: "refresh_token",
        expires_path: Some("expires_at"),
        expiry_unit: ExpiryUnit::Seconds,
    };

    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "desktoken-oauth-test-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_cred(path: &Path, access: &str, refresh: &str, expires: i64) {
        let doc = serde_json::json!({
            "access_token": access,
            "refresh_token": refresh,
            "expires_at": expires,
        });
        std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }

    fn ok_result(access: &str) -> RefreshResult {
        RefreshResult {
            access_token: access.into(),
            refresh_token: Some(format!("{}-rt", access)),
            expires_in_secs: Some(3600),
            extra_writes: vec![],
        }
    }

    #[tokio::test]
    async fn fresh_token_skips_refresh() {
        let dir = tempdir("fresh");
        let p = dir.join("cred.json");
        write_cred(&p, "good-token", "rt", now_secs() + 3600);
        let (t, _) = resolve_oauth_token(&p, &SPEC, |_| async {
            panic!("refresh must not be called for a fresh token");
        })
        .await
        .unwrap();
        assert_eq!(t, "good-token");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn expired_refreshes_and_writes_back() {
        let dir = tempdir("expired");
        let p = dir.join("cred.json");
        write_cred(&p, "old-token", "old-rt", now_secs() - 10);
        let (t, _) = resolve_oauth_token(&p, &SPEC, |rt| async move {
            assert_eq!(rt, "old-rt");
            Ok(ok_result("new-token"))
        })
        .await
        .unwrap();
        assert_eq!(t, "new-token");
        let raw = std::fs::read_to_string(&p).unwrap();
        let doc: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(doc["access_token"], "new-token");
        assert_eq!(doc["refresh_token"], "new-token-rt");
        assert!(doc["expires_at"].as_i64().unwrap() > now_secs() + 3000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spike B-1: N concurrent resolves on one expired file must collapse to
    /// a single refresh call (single-flight), all getting the same token.
    #[tokio::test]
    async fn concurrent_resolves_single_flight() {
        let dir = tempdir("singleflight");
        let p = dir.join("cred.json");
        write_cred(&p, "old", "rt", now_secs() - 10);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        for _ in 0..8 {
            let p = p.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                resolve_oauth_token(&p, &SPEC, move |_| {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        Ok(ok_result("shared-token"))
                    }
                })
                .await
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap().unwrap().0, "shared-token");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "refresh must be single-flight");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spike B-2: the CLI refreshes (rewrites the file) while our refresh is
    /// in flight. compare-before-write must adopt the CLI's token and leave
    /// the file alone.
    #[tokio::test]
    async fn cli_wins_concurrent_refresh() {
        let dir = tempdir("cliwins");
        let p = dir.join("cred.json");
        write_cred(&p, "old", "rt", now_secs() - 10);
        let p2 = p.clone();
        let (t, _) = resolve_oauth_token(&p, &SPEC, move |_| {
            let p2 = p2.clone();
            async move {
                // simulate the official CLI refreshing right now
                write_cred(&p2, "cli-token", "cli-rt", now_secs() + 3600);
                Ok(ok_result("our-token"))
            }
        })
        .await
        .unwrap();
        assert_eq!(t, "cli-token", "must adopt the CLI's concurrent refresh");
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("cli-token"), "file must keep the CLI's content");
        assert!(!raw.contains("our-token"), "our pair must be dropped");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spike B-3: invalid_grant — the CLI rotated first. Re-read must find
    /// the fresh token instead of reporting AuthExpired.
    #[tokio::test]
    async fn invalid_grant_rereads_file() {
        let dir = tempdir("invalidgrant");
        let p = dir.join("cred.json");
        write_cred(&p, "old", "rt", now_secs() - 10);
        let p2 = p.clone();
        let (t, _) = resolve_oauth_token(&p, &SPEC, move |_| {
            let p2 = p2.clone();
            async move {
                write_cred(&p2, "rotated", "rotated-rt", now_secs() + 3600);
                Err(RefreshFailure::InvalidGrant)
            }
        })
        .await
        .unwrap();
        assert_eq!(t, "rotated");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn invalid_grant_without_rotation_is_auth_expired() {
        let dir = tempdir("invalidgrant2");
        let p = dir.join("cred.json");
        write_cred(&p, "old", "rt", now_secs() - 10);
        let err = resolve_oauth_token(&p, &SPEC, |_| async {
            Err(RefreshFailure::InvalidGrant)
        })
        .await
        .unwrap_err();
        assert!(matches!(err, ProviderError::AuthExpired));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spike B-4 (rename fault injection): the target path becomes a
    /// directory mid-refresh, so rename always fails. The fresh token must
    /// still be returned (PendingWriteback lite).
    #[tokio::test]
    async fn rename_failure_still_returns_token() {
        let dir = tempdir("renamefail");
        let p = dir.join("cred.json");
        write_cred(&p, "old", "rt", now_secs() - 10);
        let p2 = p.clone();
        let (t, _) = resolve_oauth_token_with(
            &p,
            &SPEC,
            move |_| {
                let p2 = p2.clone();
                async move {
                    // sabotage: replace the file with a same-named directory
                    let _ = std::fs::remove_file(&p2);
                    std::fs::create_dir(&p2).unwrap();
                    Ok(ok_result("mem-token"))
                }
            },
            2,
            1,
        )
        .await
        .unwrap();
        assert_eq!(t, "mem-token");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spike B-5: CLI rewrite storm — a background thread rapidly rewrites
    /// the credential file while resolves loop. No panics, every resolve
    /// yields *some* token, and the file stays valid JSON throughout.
    #[tokio::test]
    async fn cli_rewrite_storm() {
        let dir = tempdir("storm");
        let p = dir.join("cred.json");
        write_cred(&p, "seed", "seed-rt", now_secs() + 3600);
        let stop = Arc::new(AtomicUsize::new(0));
        let written = Arc::new(AtomicUsize::new(0));
        let p_bg = p.clone();
        let stop_bg = stop.clone();
        let written_bg = written.clone();
        let storm = std::thread::spawn(move || {
            let mut i = 0u64;
            while stop_bg.load(Ordering::SeqCst) == 0 {
                i += 1;
                // faithful CLI simulation: atomic tmp+rename (what the real
                // Kimi/Codex/Claude CLIs do), never a torn direct write
                let tmp = p_bg.with_extension("storm-tmp");
                let doc = serde_json::json!({
                    "access_token": format!("cli-{}", i),
                    "refresh_token": "rt",
                    "expires_at": now_secs() + 3600,
                });
                std::fs::write(&tmp, serde_json::to_string(&doc).unwrap()).unwrap();
                let _ = std::fs::rename(&tmp, &p_bg);
                written_bg.store(i as usize, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });
        for _ in 0..20 {
            let (t, _) = resolve_oauth_token(&p, &SPEC, |_| async {
                Ok(ok_result("ours"))
            })
            .await
            .expect("resolve must not fail during a CLI rewrite storm");
            assert!(!t.is_empty());
            // the file must never be left unreadable beyond the torn-read window
            assert!(read_doc(&p).is_ok());
        }
        // bounded wait: the storm must actually have overlapped with us
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while written.load(Ordering::SeqCst) <= 10 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        stop.store(1, Ordering::SeqCst);
        storm.join().unwrap();
        assert!(written.load(Ordering::SeqCst) > 10, "storm must actually have written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_reports_torn() {
        let dir = tempdir("corrupt");
        let p = dir.join("cred.json");
        std::fs::write(&p, "{ not json").unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt
            .block_on(resolve_oauth_token(&p, &SPEC, |_| async {
                Ok(ok_result("x"))
            }))
            .unwrap_err();
        assert!(matches!(err, ProviderError::CredentialCorrupt { torn: true }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jwt_exp_decodes_payload() {
        // payload {"exp":1786000000} base64url
        let token = "e30.eyJleHAiOjE3ODYwMDAwMDB9.e30";
        assert_eq!(jwt_exp(token), Some(1786000000));
        assert_eq!(jwt_exp("not-a-jwt"), None);
    }

    #[test]
    fn extract_honours_expiry_unit() {
        let doc = serde_json::json!({"a": "tok", "r": "rt", "e": 1786000000000i64});
        let spec = OAuthFileSpec {
            access_path: "a",
            refresh_path: "r",
            expires_path: Some("e"),
            expiry_unit: ExpiryUnit::Millis,
        };
        let (_, _, exp) = extract(&doc, &spec);
        assert_eq!(exp, Some(1786000000));
    }

    #[test]
    fn extract_falls_back_to_jwt_exp() {
        let doc = serde_json::json!({"a": "e30.eyJleHAiOjE3ODYwMDAwMDB9.e30", "r": "rt"});
        let spec = OAuthFileSpec {
            access_path: "a",
            refresh_path: "r",
            expires_path: None,
            expiry_unit: ExpiryUnit::Seconds,
        };
        let (_, _, exp) = extract(&doc, &spec);
        assert_eq!(exp, Some(1786000000));
    }

    #[test]
    fn set_path_creates_missing_parents() {
        let mut doc = serde_json::json!({});
        set_path(&mut doc, "tokens.access_token", Value::String("x".into()));
        assert_eq!(doc["tokens"]["access_token"], "x");
    }
}
