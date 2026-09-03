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
    /// Multi-instance (B 方案): instance ids turned OFF individually
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
            disabled_instances: vec![],
            poll_intervals: Default::default(),
            skipped_version: None,
            custom_providers: vec![],
            update_checked_at: None,
            latest_version: None,
            toast_alerted: Default::default(),
            autostart: false,
        }
    }
}

/// Cross-platform app data dir: %APPDATA%\quotabar on Windows,
/// ~/Library/Application Support/quotabar on macOS, ~/.config/quotabar on Linux.
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    #[cfg(target_os = "macos")]
    let base = format!(
        "{}/Library/Application Support",
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_else(|_| ".".into())));
    PathBuf::from(base).join("quotabar")
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

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
                let _ = save(&s); // persist to the new location
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

/// Atomic write: temp file + rename, retry 6× with 100ms×2^n backoff
/// (Windows: target held by another process → ERROR_ACCESS_DENIED).
pub fn save(s: &Settings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(s)?;
    std::fs::write(&tmp, data)?;
    let mut delay = std::time::Duration::from_millis(100);
    let mut last_err = None;
    for _ in 0..6 {
        match std::fs::rename(&tmp, &path) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(delay);
                delay *= 2;
            }
        }
    }
    Err(last_err.unwrap())
}
