// QuotaBar — polling supervisor. Per-provider tokio task: immediate fetch on
// start (E7), then interval; panics/errors isolated per provider (failure
// isolation per PLAN.md §4). 429 → exponential backoff x2 capped at 8x period.
// E5 toasts (>=90% crossing with 85% hysteresis + reset moment) live here.
use crate::providers::{self, QuotaSnapshot};
use crate::settings;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

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
                .map(|s| (s.provider_id.clone(), s.provider_name.clone(), s.error.clone()))
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
    let mut s = settings::load();
    let mut dirty = false;
    let now = providers::now_secs();

    for w in &snap.windows {
        let key = format!("{}/{}", snap.provider_id, w.label);

        // hysteresis re-arm: usage fell below 85%
        if w.used_percent < 85.0 && s.toast_alerted.remove(&key).is_some() {
            dirty = true;
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
                dirty = true;
            }
        }
        // reset moment: previous cycle's resets_at passed, new cycle began
        if let Some(p) = &prev {
            if let Some(pw) = p.windows.iter().find(|pw| pw.label == w.label) {
                if let (Some(pt), Some(nt)) = (pw.resets_at, w.resets_at) {
                    if pt <= now && nt > pt && w.used_percent < pw.used_percent {
                        toast(app, &format!("{} {} 已重置，放开用", snap.provider_name, w.label));
                    }
                }
            }
        }
    }
    if dirty {
        let _ = settings::save(&s);
    }
}

async fn run_kimi() -> Result<QuotaSnapshot, providers::ProviderError> {
    providers::kimi::fetch_snapshot().await
}
async fn run_glm() -> Result<QuotaSnapshot, providers::ProviderError> {
    providers::glm::fetch_snapshot().await
}
async fn run_codex() -> Result<QuotaSnapshot, providers::ProviderError> {
    providers::codex::fetch_snapshot().await
}
async fn run_claude() -> Result<QuotaSnapshot, providers::ProviderError> {
    providers::claude::fetch_snapshot().await
}
async fn run_gemini() -> Result<QuotaSnapshot, providers::ProviderError> {
    providers::gemini::fetch_snapshot().await
}

fn spawn_provider<F, Fut>(
    app: AppHandle,
    id: String,
    name: String,
    period_secs: u64,
    fetch: F,
) where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<QuotaSnapshot, providers::ProviderError>> + Send,
{
    tauri::async_runtime::spawn(async move {
        let mut backoff = period_secs;
        loop {
            // failure isolation: a panic inside this task kills only this
            // provider's task; other providers keep polling.
            match fetch().await {
                Ok(snap) => {
                    backoff = period_secs;
                    let prev = last()
                        .lock()
                        .ok()
                        .and_then(|m| m.get(&id).cloned());
                    evaluate_alerts(&app, prev, &snap);
                    if let Ok(mut m) = last().lock() {
                        m.insert(id.clone(), snap.clone());
                    }
                    emit(&app, snap);
                }
                Err(e) => {
                    if let providers::ProviderError::RateLimited { .. } = e {
                        backoff = (backoff * 2).min(period_secs * 8);
                    }
                    if let Ok(mut m) = last().lock() {
                        let mut es = QuotaSnapshot::err(&id, &name, &e);
                        es.fetched_at = providers::now_secs();
                        m.insert(id.clone(), es);
                    }
                    emit(&app, QuotaSnapshot::err(&id, &name, &e));
                }
            }
            // sleep for the backoff period, waking early on "立即刷新"
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(backoff)) => {}
                _ = notify_refreshes().notified() => {}
            }
        }
    });
}

/// Start polling for all configured providers. Called once at startup.
/// ToS gate: caller must ensure consent before any network request.
pub fn start(app: AppHandle) {
    let s = settings::load();

    let enabled = |id: &str| {
        s.enabled_providers.is_empty() || s.enabled_providers.iter().any(|p| p == id)
    };

    if enabled("kimi") {
        spawn_provider(app.clone(), "kimi".into(), "Kimi".into(), 120, || run_kimi());
    }
    if enabled("glm") {
        spawn_provider(app.clone(), "glm".into(), "GLM".into(), 120, || run_glm());
    }
    if enabled("codex") {
        spawn_provider(app.clone(), "codex".into(), "Codex".into(), 120, || run_codex());
    }
    if enabled("claude") {
        // Claude polling is clamped to >= 10 min (PLAN.md ToS decision);
        // 429 backoff doubles from this floor (max 8x).
        spawn_provider(app.clone(), "claude".into(), "Claude".into(), 600, || run_claude());
    }
    if enabled("gemini") {
        // Gemini polling is clamped to >= 5 min (PLAN.md decision #12).
        spawn_provider(app.clone(), "gemini".into(), "Gemini".into(), 300, || run_gemini());
    }
    for def in s.custom_providers {
        let period = def.poll_minutes.max(1) * 60;
        spawn_provider(
            app.clone(),
            def.id.clone(),
            def.name.clone(),
            period,
            move || {
                let d = def.clone();
                async move { providers::custom::fetch_snapshot(&d).await }
            },
        );
    }
}
