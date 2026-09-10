// QuotaBar — polling supervisor. Per-provider tokio task: immediate fetch on
// start (E7), then interval; panics/errors isolated per provider (failure
// isolation per PLAN.md §4). 429 → exponential backoff x2 capped at 8x period.
// E5 toasts (>=90% crossing with 85% hysteresis + reset moment) live here.
use crate::providers::{self, QuotaSnapshot};
use crate::settings;
use futures_util::FutureExt;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

fn period_for(base: &str, custom_minutes: Option<u64>) -> u64 {
    match base {
        "claude" => 600,
        "gemini" => 300,
        "custom" => custom_minutes.unwrap_or(5).clamp(1, 1440) * 60,
        _ => 120,
    }
}

fn instance_period(id: &str) -> u64 {
    let s = settings::load();
    if let Some(def) = s.custom_providers.iter().find(|d| d.id == id) {
        return period_for("custom", Some(def.poll_minutes));
    }
    period_for(id.split('#').next().unwrap_or(id), None)
}

/// Jitter cannot shorten the configured minimum or server Retry-After.
fn retry_delay(backoff: u64, period: u64, server: Option<u64>, jitter: i32) -> u64 {
    let varied = backoff.saturating_mul((100 + jitter.clamp(-20, 20)) as u64) / 100;
    varied.max(period).max(server.unwrap_or(0))
}

async fn isolate_fetch<
    F: std::future::Future<Output = Result<QuotaSnapshot, providers::ProviderError>>,
>(
    f: F,
) -> Result<QuotaSnapshot, providers::ProviderError> {
    std::panic::AssertUnwindSafe(f)
        .catch_unwind()
        .await
        .unwrap_or(Err(providers::ProviderError::Internal))
}

pub const EVENT: &str = "quota://snapshot";

/// User-triggered full refresh ("立即刷新" menu); all tasks wake on it.
static REFRESH: OnceLock<tokio::sync::Notify> = OnceLock::new();
/// Last snapshot per provider (toast transitions + E6 diagnostics).
static LAST: OnceLock<Mutex<HashMap<String, QuotaSnapshot>>> = OnceLock::new();
/// Tray handle for alert color swap.
static TRAY: OnceLock<tauri::tray::TrayIcon> = OnceLock::new();

fn notify_refreshes() -> &'static tokio::sync::Notify {
    REFRESH.get_or_init(tokio::sync::Notify::new)
}
fn last() -> &'static Mutex<HashMap<String, QuotaSnapshot>> {
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_tray(tray: tauri::tray::TrayIcon) {
    let _ = TRAY.set(tray);
}

pub fn refresh_now() {
    notify_refreshes().notify_waiters();
}

/// E6 diagnostics: (id, name, error) per provider seen this session.
pub fn last_states() -> Vec<(String, String, Option<String>)> {
    last()
        .lock()
        .map(|m| {
            m.values()
                .map(|s| {
                    (
                        s.provider_id.clone(),
                        s.provider_name.clone(),
                        s.error.clone(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn emit(app: &AppHandle, snap: QuotaSnapshot) {
    crate::rustlog(format!(
        "poller emit: {} error={:?} windows={}",
        snap.provider_id,
        snap.error,
        snap.windows.len()
    ));
    let _ = app.emit(EVENT, providers::sanitize(snap));
}

fn toast(app: &AppHandle, body: &str) {
    crate::rustlog(format!("toast: {}", body));
    let _ = app
        .notification()
        .builder()
        .title("QuotaBar")
        .body(body)
        .show();
    // border flash + tray icon turns red for 3s (design spec E5)
    let _ = app.emit("alert-flash", ());
    if let Some(tray) = TRAY.get() {
        let _ = tray.set_icon(Some(crate::tray_icon_image_alert()));
        let tray2 = tray.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let _ = tray2.set_icon(Some(crate::tray_icon_image()));
        });
    }
}

/// E5 alert state machine: >=90% crossing toast (re-armed when usage drops
/// <85%; one toast per reset cycle, persisted) + reset-moment toast.
fn evaluate_alerts(app: &AppHandle, prev: Option<QuotaSnapshot>, snap: &QuotaSnapshot) {
    if snap.error.is_some() {
        return;
    }
    let now = providers::now_secs();

    settings::edit(|s| {
        for w in &snap.windows {
            let key = format!("{}/{}", snap.provider_id, w.label);

            // hysteresis re-arm: usage fell below 85%
            if w.used_percent < 85.0 {
                s.toast_alerted.remove(&key);
            }
            // >=90% crossing: one toast per reset cycle
            if w.used_percent >= 90.0 {
                let cycle = w.resets_at.unwrap_or(0);
                if s.toast_alerted.get(&key) != Some(&cycle) {
                    toast(
                        app,
                        &format!(
                            "{} {} 窗口已用 {}%（{}）",
                            snap.provider_name,
                            w.label,
                            w.used_percent.round() as i64,
                            "接近用尽"
                        ),
                    );
                    s.toast_alerted.insert(key, cycle);
                }
            }
            // reset moment: previous cycle's resets_at passed, new cycle began
            if let Some(p) = &prev {
                if let Some(pw) = p.windows.iter().find(|pw| pw.label == w.label) {
                    if let (Some(pt), Some(nt)) = (pw.resets_at, w.resets_at) {
                        if pt <= now && nt > pt && w.used_percent < pw.used_percent {
                            toast(
                                app,
                                &format!("{} {} 已重置，放开用", snap.provider_name, w.label),
                            );
                        }
                    }
                }
            }
        }
    });
}

fn spawn_provider<F, Fut>(
    app: AppHandle,
    id: String,
    name: String,
    fetch: F,
) -> tauri::async_runtime::JoinHandle<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<QuotaSnapshot, providers::ProviderError>> + Send,
{
    tauri::async_runtime::spawn(async move {
        let mut backoff = instance_period(&id);
        loop {
            let period_secs = instance_period(&id);
            backoff = backoff.max(period_secs).min(period_secs * 8);
            let mut limited = false;
            let mut server_delay = None;
            // Include the call itself inside the unwind boundary, then retry normally.
            match isolate_fetch(async { fetch().await }).await {
                Ok(snap) => {
                    let mut snap = providers::sanitize(snap);
                    snap.stale_after_secs = period_secs.saturating_add(60).max(600);
                    backoff = period_secs;
                    let prev = last().lock().ok().and_then(|m| m.get(&id).cloned());
                    evaluate_alerts(&app, prev, &snap);
                    crate::history::record(&snap); // E8: 7-day local history
                    if let Ok(mut m) = last().lock() {
                        m.insert(id.clone(), snap.clone());
                    }
                    emit(&app, snap);
                }
                Err(e) => {
                    if let providers::ProviderError::RateLimited { retry_after } = &e {
                        backoff = (backoff * 2).min(period_secs * 8);
                        limited = true;
                        server_delay = *retry_after;
                    }
                    if let Ok(mut m) = last().lock() {
                        let mut es = QuotaSnapshot::err(&id, &name, &e);
                        es.fetched_at = providers::now_secs();
                        m.insert(id.clone(), es);
                    }
                    emit(&app, QuotaSnapshot::err(&id, &name, &e));
                }
            }
            if limited {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos();
                let mut remaining =
                    retry_delay(backoff, period_secs, server_delay, (nanos % 41) as i32 - 20);
                // Ignore manual refresh during cooldown. Chunk huge server hints
                // to avoid Instant overflow; the task remains abortable.
                while remaining > 0 {
                    let chunk = remaining.min(86400);
                    tokio::time::sleep(std::time::Duration::from_secs(chunk)).await;
                    remaining -= chunk;
                }
            } else {
                tokio::select! {
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(backoff)) => {}
                    _ = notify_refreshes().notified() => {}
                }
            }
        }
    })
}

/// Running poll tasks per instance id — abortable so settings changes take
/// effect immediately (hot reload) instead of requiring an app restart.
static TASKS: OnceLock<
    Mutex<std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
> = OnceLock::new();

fn tasks() -> &'static Mutex<std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>>
{
    TASKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Desired instance set: discovered builtin instances + custom providers,
/// filtered by base-level and instance-level settings.
fn desired() -> Vec<crate::credentials::InstanceDesc> {
    let s = settings::load();
    let enabled_base = |base: &str| {
        if s.providers_configured {
            s.enabled_providers.iter().any(|p| p == base)
        } else {
            s.enabled_providers.is_empty() || s.enabled_providers.iter().any(|p| p == base)
        }
    };
    let mut out: Vec<crate::credentials::InstanceDesc> = crate::credentials::discover_instances()
        .into_iter()
        .filter(|i| enabled_base(&i.base) && !s.disabled_instances.contains(&i.id))
        .collect();
    for def in &s.custom_providers {
        out.push(crate::credentials::InstanceDesc {
            id: def.id.clone(),
            name: def.name.clone(),
            base: "custom".into(),
        });
    }
    out
}

/// (Re)compute the polling set: abort tasks whose instance is gone, spawn new
/// ones, tell the frontend what's live now. Called at startup (ToS-gated) and
/// on every provider/instance/custom settings change.
pub fn sync(app: AppHandle) {
    let want = desired();

    // abort removed
    {
        let mut m = match tasks().lock() {
            Ok(m) => m,
            Err(e) => e.into_inner(),
        };
        let want_ids: Vec<&String> = want.iter().map(|i| &i.id).collect();
        let removed: Vec<String> = m
            .keys()
            .filter(|k| !want_ids.contains(k))
            .cloned()
            .collect();
        for id in &removed {
            if let Some(h) = m.remove(id) {
                h.abort();
            }
            // clear row state so the bar drops it immediately
            if let Ok(mut lm) = last().lock() {
                lm.remove(id);
            }
            use tauri::Emitter;
            let _ = app.emit("provider-removed", id.clone());
            crate::rustlog(format!("poller: removed {}", id));
        }
    }

    // spawn new
    // tell the frontend the live set (loading placeholders for new rows)
    let live: Vec<serde_json::Value> = want
        .iter()
        .map(|i| serde_json::json!({ "id": i.id, "name": i.name }))
        .collect();

    let mut spawned: Vec<String> = vec![];
    for inst in want {
        let mut m = match tasks().lock() {
            Ok(m) => m,
            Err(e) => e.into_inner(),
        };
        if m.contains_key(&inst.id) {
            continue;
        }
        let id2 = inst.id.clone();
        let name2 = inst.name.clone();
        let handle = spawn_provider(app.clone(), inst.id.clone(), inst.name.clone(), move || {
            let id = id2.clone();
            let name = name2.clone();
            async move { providers::fetch_instance(&id, &name).await }
        });
        m.insert(inst.id.clone(), handle);
        spawned.push(inst.id.clone());
    }

    if !spawned.is_empty() {
        crate::rustlog(format!("poller: spawned {:?}", spawned));
    }

    let _ = app.emit("providers-init", live);
    refresh_now(); // existing tasks re-read settings; 429 cooldowns stay intact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_period_and_minimums() {
        assert_eq!(period_for("custom", Some(17)), 1020);
        assert_eq!(period_for("custom", Some(0)), 60);
        assert_eq!(period_for("custom", Some(u64::MAX)), 86400);
        assert_eq!(period_for("claude", None), 600);
    }

    #[test]
    fn server_cooldown_is_not_capped_by_local_backoff() {
        assert_eq!(retry_delay(960, 120, Some(3600), -20), 3600);
        assert_eq!(retry_delay(120, 120, None, -20), 120);
        assert_eq!(retry_delay(240, 120, None, 20), 288);
    }

    #[tokio::test]
    async fn panic_becomes_error_and_next_fetch_can_run() {
        let result = isolate_fetch(async { panic!("fixture panic") }).await;
        assert!(matches!(result, Err(providers::ProviderError::Internal)));
        assert!(isolate_fetch(async {
            Ok(QuotaSnapshot::ok("test", "Test", None, vec![], "official"))
        })
        .await
        .is_ok());
    }
}
