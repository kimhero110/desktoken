// DeskToken — settings persistence (atomic write + Windows rename retry per eng review §3)
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMapping {
    pub label: String,
    pub used_path: String,
    pub limit_path: String,
    #[serde(default)]
    pub reset_path: Option<String>,
    /// Low-watermark mode (余额类接口): percent is inverted so the bar/toasts
    /// treat "balance running out" as "quota running out".
    #[serde(default)]
    pub invert: bool,
}

/// Custom provider definition (open framework): one GET → JSON quota, user-configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProvider {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub auth_header: String,
    pub auth_prefix: String,
    pub poll_minutes: u64,
    #[serde(default)]
    pub windows: Vec<WindowMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub window_x: Option<f64>,
    pub window_y: Option<f64>,
    pub monitor_name: Option<String>,
    pub opacity: f64,
    pub width: f64,
    /// Mini mode (replaces true click-through; see PLAN.md: Win32/WebView2
    /// click-through spike failed across 5 approaches, degraded by decision).
    #[serde(alias = "click_through")]
    pub mini_mode: bool,
    pub tos_accepted: bool,
    pub enabled_providers: Vec<String>,
    /// true once the user has explicitly toggled any provider: from then on an
    /// EMPTY enabled_providers means "nothing on" instead of the first-run
    /// default "everything on" (the empty-list trap).
    #[serde(default)]
    pub providers_configured: bool,
    /// Multi-instance (方案 B): instance ids turned OFF individually
    /// (e.g. "codex#opencode"). enabled_providers stays base-level.
    #[serde(default)]
    pub disabled_instances: Vec<String>,
    pub poll_intervals: std::collections::HashMap<String, u64>,
    #[serde(default)]
    pub skipped_version: Option<String>,
    #[serde(default)]
    pub custom_providers: Vec<CustomProvider>,
    /// E1: last version-check epoch + latest seen version (24h cache)
    #[serde(default)]
    pub update_checked_at: Option<i64>,
    #[serde(default)]
    pub latest_version: Option<String>,
    /// E5 toast dedup: "provider/label" -> resets_at of the alerted cycle
    #[serde(default)]
    pub toast_alerted: std::collections::HashMap<String, i64>,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub task_notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window_x: None,
            window_y: None,
            monitor_name: None,
            opacity: 0.72,
            width: 280.0,
            mini_mode: false,
            tos_accepted: false,
            enabled_providers: vec![],
            providers_configured: false,
            disabled_instances: vec![],
            poll_intervals: Default::default(),
            skipped_version: None,
            custom_providers: vec![],
            update_checked_at: None,
            latest_version: None,
            toast_alerted: Default::default(),
            autostart: false,
            task_notifications: false,
        }
    }
}

/// Cross-platform app data dir: %APPDATA%\quotabar on Windows,
/// ~/Library/Application Support/quotabar on macOS, ~/.config/quotabar on Linux.
#[cfg(test)]
pub fn app_data_dir() -> PathBuf {
    // Tests must never read/write the real user's settings or usage history.
    std::env::temp_dir().join(format!("quotabar-tests-{}", std::process::id()))
}

#[cfg(not(test))]
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    #[cfg(target_os = "macos")]
    let base = format!(
        "{}/Library/Application Support",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        format!(
            "{}/.config",
            std::env::var("HOME").unwrap_or_else(|_| ".".into())
        )
    });
    PathBuf::from(base).join("quotabar")
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

#[cfg(test)]
fn legacy_settings_path() -> PathBuf {
    app_data_dir().join("legacy-settings.json")
}

#[cfg(not(test))]
fn legacy_settings_path() -> PathBuf {
    // DeskToken-era dir (Windows); non-Windows never had one
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base).join("desktoken").join("settings.json")
    }
    #[cfg(not(target_os = "windows"))]
    {
        app_data_dir().join("settings.json") // == new path: legacy check is a no-op
    }
}

/// Load settings; corrupt file → backup + defaults (per Error Registry).
/// One-time migration: DeskToken-era settings dir → QuotaBar.
pub fn load() -> Settings {
    let path = settings_path();
    if !path.exists() {
        let legacy = legacy_settings_path();
        if let Ok(raw) = std::fs::read_to_string(&legacy) {
            if let Ok(s) = serde_json::from_str::<Settings>(&raw) {
                // save_locked, not save: load() is called from inside edit(),
                // which already holds WRITE_LOCK, and the mutex is not
                // reentrant. Two concurrent first-run migrations would write
                // identical bytes, so skipping the lock here is safe.
                let _ = save_locked(&s); // persist to the new location
                return s;
            }
        }
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    match serde_json::from_str::<Settings>(&raw) {
        Ok(s) => s,
        Err(_) => {
            let bak = path.with_extension("json.bak");
            let _ = std::fs::copy(&path, &bak);
            Settings::default()
        }
    }
}

/// All writes serialize through this lock. Concurrent writers (poller toast
/// dedup vs UI toggles) used to last-write-win and lose each other's edits, so
/// edit()/try_edit() are the only way in: a bare load-modify-save pair from a
/// command raced the poller.
static WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Atomic write: temp file + rename, retry 6× with 100ms×2^n backoff
/// (Windows: target held by another process → ERROR_ACCESS_DENIED).
/// Does NOT take WRITE_LOCK — callers hold it, and the mutex is not reentrant.
fn save_locked(s: &Settings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic_file::replace(&path, &serde_json::to_vec_pretty(s)?, 6, 100)
}

/// Read-modify-write under the write lock: the only safe way to edit settings
/// from concurrent contexts (poller tasks, UI commands).
pub fn edit<R>(f: impl FnOnce(&mut Settings) -> R) -> R {
    // serialize read→edit→save as one unit
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load();
    let r = f(&mut s);
    let _ = save_locked(&s);
    r
}

/// Fallible settings edit for controls that must report persistence failures.
pub fn try_edit(f: impl FnOnce(&mut Settings)) -> std::io::Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load();
    f(&mut s);
    save_locked(&s)
}

#[cfg(test)]
mod tests {
    /// Regression: ba6de99 had edit() take the non-reentrant WRITE_LOCK and
    /// then call a save() that took it again, so every successful provider
    /// poll froze before emit and the bar never updated. Fail fast instead of
    /// hanging the suite if this ever comes back.
    #[test]
    fn edit_roundtrip_no_deadlock() {
        let h = std::thread::spawn(|| super::edit(|s| s.opacity));
        let start = std::time::Instant::now();
        while !h.is_finished() {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(10),
                "settings::edit deadlocked — WRITE_LOCK reentrancy regression"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let opacity = h.join().unwrap();
        assert!(opacity > 0.0);
    }
}
