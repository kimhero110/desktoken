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
    pub poll_intervals: std::collections::HashMap<String, u64>,
    #[serde(default)]
    pub skipped_version: Option<String>,
    #[serde(default)]
    pub custom_providers: Vec<CustomProvider>,
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
            poll_intervals: Default::default(),
            skipped_version: None,
            custom_providers: vec![],
        }
    }
}

pub fn settings_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("desktoken").join("settings.json")
}

/// Load settings; corrupt file → backup + defaults (per Error Registry).
pub fn load() -> Settings {
    let path = settings_path();
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
