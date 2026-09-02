// DeskToken — polling supervisor. Per-provider tokio task: immediate fetch on
// start (E7), then interval; panics/errors isolated per provider (failure
// isolation per PLAN.md §4). 429 → exponential backoff x2 capped at 8x period.
use crate::providers::{self, QuotaSnapshot};
use crate::settings;
use tauri::{AppHandle, Emitter};

pub const EVENT: &str = "quota://snapshot";

fn emit(app: &AppHandle, snap: QuotaSnapshot) {
    crate::rustlog(format!(
        "poller emit: {} error={:?} windows={}",
        snap.provider_id,
        snap.error,
        snap.windows.len()
    ));
    let _ = app.emit(EVENT, providers::sanitize(snap));
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
                    emit(&app, snap);
                }
                Err(e) => {
                    if let providers::ProviderError::RateLimited { .. } = e {
                        backoff = (backoff * 2).min(period_secs * 8);
                    }
                    emit(&app, QuotaSnapshot::err(&id, &name, &e));
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;
        }
    });
}

/// Start polling for all configured providers. Called once at startup.
/// ToS gate (M5): caller must ensure consent before any network request.
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
