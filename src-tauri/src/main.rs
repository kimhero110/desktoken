// QuotaBar — quota floating bar. Release builds are pure GUI: no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// M1 Lane A: window skeleton + spike A (NOACTIVATE / acrylic / drag / no-focus-steal)
//                 + tray + single-instance + position clamp (work-area aware)

/// UI language detection for the tray menu. On Windows, env LANG is usually
/// unset — ask the OS for the user's UI language instead (LANG_CHINESE = 0x04).
#[cfg(target_os = "windows")]
fn is_zh_locale() -> bool {
    let langid = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    langid & 0x3FF == 0x04
}

#[cfg(not(target_os = "windows"))]
fn is_zh_locale() -> bool {
    let lang = std::env::var("LANG").unwrap_or_default().to_lowercase();
    let sys_lang = std::env::var("LC_ALL").unwrap_or_default().to_lowercase();
    lang.starts_with("zh") || sys_lang.starts_with("zh")
}
mod settings;
mod credentials;
mod diagnostics;
mod fetch;
mod history;
mod oauth;
mod poller;
mod providers;
mod updater_check;

use settings::Settings;
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const WIN_H: f64 = 320.0;

// ---------------------------------------------------------------------------
// Spike A: WS_EX_NOACTIVATE via windows-sys FFI (Tauri does not expose this)
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn apply_noactivate(window: &WebviewWindow) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    match window.hwnd() {
        Ok(hwnd) => {
            let raw: HWND = hwnd.0 as HWND;
            unsafe {
                let before = GetWindowLongPtrW(raw, GWL_EXSTYLE);
                let after = before | WS_EX_NOACTIVATE as isize | WS_EX_TOOLWINDOW as isize;
                SetWindowLongPtrW(raw, GWL_EXSTYLE, after);
                SetWindowPos(
                    raw,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
                );
                let confirm = GetWindowLongPtrW(raw, GWL_EXSTYLE);
                rustlog(format!(
                    "noactivate: hwnd={:?} before=0x{:X} set=0x{:X} confirm=0x{:X}",
                    raw, before, after, confirm
                ));
            }
        }
        Err(e) => rustlog(format!("noactivate: hwnd() failed: {:?}", e)),
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_noactivate(_window: &WebviewWindow) {}

/// Round the OS window corners via DWM (Win11 only; silently ignored on
/// Server/Win10 — the bar draws square corners itself, so this is pure bonus).
/// NOTE: SetWindowRgn was tried and made the gray WORSE (acrylic fringe
/// artifact along the region edge) — do not re-add.
#[cfg(target_os = "windows")]
fn apply_rounded_corners(window: &WebviewWindow) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    if let Ok(hwnd) = window.hwnd() {
        let raw: HWND = hwnd.0 as HWND;
        let pref = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(
                raw,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &pref as *const _ as *const _,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_rounded_corners(_window: &WebviewWindow) {}

/// Show the bar and (re)apply NOACTIVATE. Used at startup and after ToS consent.
/// apply AFTER show: tao may rewrite exstyle when the window is first shown.
fn reveal_main(window: &WebviewWindow) {
    let _ = window.show();
    apply_noactivate(window);
    apply_rounded_corners(window);
}

// ---------------------------------------------------------------------------
// Position: logical-px persistence + work-area clamp (lazy, on startup)
// ---------------------------------------------------------------------------
fn clamp_position(window: &WebviewWindow, s: &Settings) -> tauri::Result<()> {
    let monitors = window.available_monitors()?;
    let primary = window
        .primary_monitor()?
        .or_else(|| monitors.first().cloned());
    let Some(primary) = primary else { return Ok(()) };

    let (mut x, mut y) = match (s.window_x, s.window_y) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            // default: primary top-right, 16px margin (logical px)
            let wa = primary.work_area();
            let sf = primary.scale_factor();
            let wa_r = wa.position.x as f64 + wa.size.width as f64 / sf;
            (wa_r - s.width - 16.0, wa.position.y as f64 + 16.0)
        }
    };

    // clamp: window must intersect some monitor's work area
    let in_any = monitors.iter().any(|m| {
        let sf = m.scale_factor();
        let wa = m.work_area();
        let (wx, wy) = (wa.position.x as f64 / sf, wa.position.y as f64 / sf);
        let (ww, wh) = (wa.size.width as f64 / sf, wa.size.height as f64 / sf);
        x + s.width > wx && x < wx + ww && y + 40.0 > wy && y < wy + wh
    });
    if !in_any {
        let sf = primary.scale_factor();
        let wa = primary.work_area();
        x = wa.position.x as f64 / sf + wa.size.width as f64 / sf - s.width - 16.0;
        y = wa.position.y as f64 / sf + 16.0;
    }
    window.set_position(tauri::LogicalPosition::new(x, y))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tray
// ---------------------------------------------------------------------------
fn tray_icon_image_colored(r: u8, g: u8, b: u8) -> tauri::image::Image<'static> {
    // 16x16 simple dot
    let mut rgba = vec![0u8; 16 * 16 * 4];
    for py in 0..16 {
        for px in 0..16 {
            let dx = px as f64 - 7.5;
            let dy = py as f64 - 7.5;
            if dx * dx + dy * dy <= 36.0 {
                let i = (py * 16 + px) * 4;
                rgba[i] = r;
                rgba[i + 1] = g;
                rgba[i + 2] = b;
                rgba[i + 3] = 0xFF;
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 16, 16)
}

pub(crate) fn tray_icon_image() -> tauri::image::Image<'static> {
    // the real QuotaBar logo, raw RGBA (icons/tray.rgba, 32x32, made by PIL)
    tauri::image::Image::new_owned(include_bytes!("../icons/tray.rgba").to_vec(), 32, 32)
}

pub(crate) fn tray_icon_image_alert() -> tauri::image::Image<'static> {
    tray_icon_image_colored(0xF8, 0x51, 0x49) // #F85149
}

// ---------------------------------------------------------------------------
// "立即刷新" cooldown (design: 30s, menu item greyed while cooling)
// ---------------------------------------------------------------------------
static LAST_MANUAL_REFRESH: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);

fn refresh_cooling_down() -> bool {
    LAST_MANUAL_REFRESH
        .lock()
        .ok()
        .and_then(|g| *g)
        .map(|t| t.elapsed() < std::time::Duration::from_secs(30))
        .unwrap_or(false)
}

fn trigger_refresh() -> bool {
    if refresh_cooling_down() {
        return false;
    }
    if let Ok(mut g) = LAST_MANUAL_REFRESH.lock() {
        *g = Some(std::time::Instant::now());
    }
    poller::refresh_now();
    true
}

fn build_app_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let refresh = MenuItemBuilder::with_id("refresh", if is_zh_locale() { "立即刷新" } else { "Refresh Now" })
        .enabled(!refresh_cooling_down())
        .build(app)?;
    let mini = CheckMenuItemBuilder::with_id("mini_mode", if is_zh_locale() { "迷你模式" } else { "Mini Mode" })
        .checked(settings::load().mini_mode)
        .build(app)?;
    let diag = MenuItemBuilder::with_id("diag", if is_zh_locale() { "复制诊断信息" } else { "Copy Diagnostics" }).build(app)?;
    let check_update = MenuItemBuilder::with_id("check_update", if is_zh_locale() { "检查更新" } else { "Check for Updates" }).build(app)?;
    let report = MenuItemBuilder::with_id("report", if is_zh_locale() { "在 GitHub 报告问题" } else { "Report Issue on GitHub" }).build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", if is_zh_locale() { "设置..." } else { "Settings..." }).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", if is_zh_locale() { "退出" } else { "Quit" }).build(app)?;
    MenuBuilder::new(app)
        .items(&[
            &refresh,
            &PredefinedMenuItem::separator(app)?,
            &mini,
            &PredefinedMenuItem::separator(app)?,
            &diag,
            &check_update,
            &report,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ])
        .build()
}

fn handle_menu_event(app: &tauri::AppHandle, id: &str) {
    match id {
        "quit" => app.exit(0),
        "settings" => open_settings_window(app),
        "mini_mode" => {
            if let Some(w) = app.get_webview_window("main") {
                let mut s = settings::load();
                s.mini_mode = !s.mini_mode;
                let _ = settings::save(&s);
                set_mini_mode(&w, s.mini_mode);
            }
        }
        "refresh" => {
            trigger_refresh();
        }
        "diag" => {
            let text = diagnostics::collect(&poller::last_states());
            use tauri_plugin_clipboard_manager::ClipboardExt;
            let ok = app.clipboard().write_text(text).is_ok();
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("QuotaBar")
                .body(if ok { "诊断信息已复制（已脱敏）" } else { "复制失败" })
                .show();
        }
        "check_update" => {
            updater_check::maybe_check(app.clone(), true);
        }
        "report" => {
            use tauri_plugin_opener::OpenerExt;
            let _ = app.opener().open_url(
                "https://github.com/kimhero110/desktoken/issues/new/choose",
                None::<&str>,
            );
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Commands (frontend → backend)
// ---------------------------------------------------------------------------
#[tauri::command]
fn context_menu(window: WebviewWindow) {
    if let Ok(menu) = build_app_menu(&window.app_handle()) {
        let _ = window.popup_menu(&menu);
    }
}

#[tauri::command]
fn get_settings() -> Settings {
    settings::load()
}

// ---------------------------------------------------------------------------
// First-run ToS gate (PLAN.md: zero network requests before consent, including
// version checks). The bar stays hidden and the poller stays off until the
// user explicitly agrees in the ToS window.
// ---------------------------------------------------------------------------
fn open_tos_window(app: &tauri::AppHandle) {
    if app.get_webview_window("tos").is_some() {
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "tos", WebviewUrl::App("tos.html".into()))
        .title("DeskToken — 使用前请知悉")
        .inner_size(380.0, 300.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .decorations(true)
        .transparent(false)
        .center()
        .focused(true)
        .build();
}

#[tauri::command]
fn accept_tos(app: tauri::AppHandle) -> Result<(), String> {
    let mut s = settings::load();
    s.tos_accepted = true;
    settings::save(&s).map_err(|e| e.to_string())?;
    // E4 magical moment: auto-discovered providers are enabled by default
    // (empty enabled_providers = all), fetch immediately, notify 3s.
    let names: Vec<String> = credentials::detect()
        .into_iter()
        .filter(|c| c.status != "missing")
        .map(|c| c.name)
        .collect();
    if let Some(w) = app.get_webview_window("main") {
        reveal_main(&w);
    }
    poller::start(app.clone());
    updater_check::maybe_check(app.clone(), false);
    let _ = app.emit_to("main", "tos-accepted", names);
    if let Some(t) = app.get_webview_window("tos") {
        let _ = t.close();
    }
    Ok(())
}

#[tauri::command]
fn decline_tos(app: tauri::AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------------------
// Autostart (HKCU Run key) + misc commands
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn apply_autostart(enable: bool) -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("QuotaBar", &exe.to_string_lossy().to_string())
            .map_err(|e| e.to_string())?;
    } else {
        let _ = key.delete_value("QuotaBar");
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn apply_autostart(_enable: bool) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn set_autostart(enabled: bool) -> Result<(), String> {
    apply_autostart(enabled)?;
    let mut s = settings::load();
    s.autostart = enabled;
    settings::save(&s).map_err(|e| e.to_string())
}

#[tauri::command]
fn copy_diagnostics(app: tauri::AppHandle) -> Result<(), String> {
    let text = diagnostics::collect(&poller::last_states());
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Detail card "立即刷新" — same 30s cooldown as the menu item.
#[tauri::command]
fn refresh_now_cmd() -> bool {
    trigger_refresh()
}

/// Settings window "检查更新".
#[tauri::command]
fn check_update_cmd(app: tauri::AppHandle) {
    updater_check::maybe_check(app, true);
}

#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    // allowlist: our repo pages + official provider consoles (detail card links)
    const ALLOWED: &[&str] = &[
        "https://github.com/kimhero110/desktoken",
        "https://platform.moonshot.cn",
        "https://open.bigmodel.cn",
        "https://chatgpt.com",
        "https://claude.ai",
        "https://antigravity.google",
    ];
    if !ALLOWED.iter().any(|p| url.starts_with(p)) {
        return Err("不允许的链接".into());
    }
    app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string())
}

/// E8: 7-day usage history for the detail card sparklines.
/// Returns { label: [(ts, used_pct)] } oldest-first.
#[tauri::command]
fn get_history(provider_id: String) -> std::collections::BTreeMap<String, Vec<(i64, f64)>> {
    history::provider_history(&provider_id)
}

// ---------------------------------------------------------------------------
// Drag: manual cursor-poll thread. NOACTIVATE-proof (tao start_dragging relies
// on WM_NCLBUTTONDOWN which is unreliable with WS_EX_NOACTIVATE).
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
#[tauri::command]
fn begin_drag(window: WebviewWindow) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    const VK_LBUTTON: i32 = 0x01;
    std::thread::spawn(move || unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) == 0 {
            return;
        }
        let Ok(win_pos) = window.outer_position() else { return };
        let (cx0, cy0) = (pt.x, pt.y);
        let (wx0, wy0) = (win_pos.x, win_pos.y);
        loop {
            // high bit clear => button released
            if GetAsyncKeyState(VK_LBUTTON) as u16 & 0x8000 == 0 {
                break;
            }
            if GetCursorPos(&mut pt) == 0 {
                break;
            }
            let _ = window.set_position(tauri::PhysicalPosition::new(
                wx0 + (pt.x - cx0),
                wy0 + (pt.y - cy0),
            ));
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        // persist position at drag end
        if let Ok(pos) = window.outer_position() {
            let sf = window.scale_factor().unwrap_or(1.0);
            let mut s = settings::load();
            s.window_x = Some(pos.x as f64 / sf);
            s.window_y = Some(pos.y as f64 / sf);
            s.monitor_name = window
                .current_monitor()
                .ok()
                .flatten()
                .and_then(|m| m.name().cloned());
            let _ = settings::save(&s);
        }
    });
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn begin_drag(window: WebviewWindow) {
    let _ = window.start_dragging();
}

// ---------------------------------------------------------------------------
// Mini mode (degraded replacement for click-through after spike A: true
// click-through is not achievable with WebView2 child windows without breaking
// rendering — v1 exstyle/v2 subclass/v3+v4 disable all failed on real mouse).
// Mini mode (degraded replacement for click-through after spike A: true
// click-through is not achievable with WebView2 child windows without breaking
// rendering — v1 exstyle/v2 subclass/v3+v4 disable all failed on real mouse).
// Frontend owns sizing: .mini class + autosize(22).
// ---------------------------------------------------------------------------

fn set_mini_mode(window: &WebviewWindow, enable: bool) {
    // frontend owns height: it applies the .mini class then calls autosize
    rustlog(format!("set_mini_mode: emit mini-mode={}", enable));
    let _ = window.emit_to("main", "mini-mode", enable);
}

/// Fit window height to frontend content (kills the invisible dead zone below
/// the bar that still blocks clicks).
#[tauri::command]
fn jslog(msg: String) {
    rustlog(format!("js: {}", msg));
}

pub(crate) fn rustlog(msg: String) {
    // redact defensively (eng review: no tokens in logs)
    let safe = diagnostics::redact(&msg);
    let dir = settings::app_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("spike.log"))
        .map(|mut f| {
            use std::io::Write;
            let _ = writeln!(f, "{}", safe);
        });
}

#[tauri::command]
fn autosize(window: WebviewWindow, height: f64, width: Option<f64>) {
    let h = height.clamp(20.0, 800.0);
    let s = settings::load();
    let sf = window.scale_factor().unwrap_or(1.0);
    // PHYSICAL size: tao LogicalSize conversion misfires on mixed-DPI setups
    let w = width.unwrap_or(s.width).clamp(120.0, 400.0);
    let phys = tauri::PhysicalSize::new((w * sf).round() as u32, (h * sf).round() as u32);
    rustlog(format!("autosize: req={}x{:?} -> phys {:?}", height, width, phys));
    let _ = window.set_size(phys);
}

// ---------------------------------------------------------------------------
// Settings window (minimal, live-apply)
// ---------------------------------------------------------------------------
fn open_settings_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("DeskToken 设置")
        .inner_size(480.0, 560.0)
        .resizable(false)
        .maximizable(false)
        .always_on_top(false)
        .skip_taskbar(false)
        .decorations(true)
        .transparent(false)
        .center()
        .focused(true)
        .build();
}

#[tauri::command]
fn apply_appearance(
    app: tauri::AppHandle,
    opacity: f64,
    width: f64,
    mini_mode: bool,
) -> Result<(), String> {
    let mut s = settings::load();
    s.opacity = opacity.clamp(0.6, 1.0);
    s.width = width.clamp(240.0, 360.0);
    s.mini_mode = mini_mode;
    settings::save(&s).map_err(|e| e.to_string())?;
    if let Some(w) = app.get_webview_window("main") {
        if !mini_mode {
            let _ = w.set_size(tauri::PhysicalSize::new(
                (s.width * w.scale_factor().unwrap_or(1.0)).round() as u32,
                (WIN_H * w.scale_factor().unwrap_or(1.0)).round() as u32,
            ));
        }
        set_mini_mode(&w, mini_mode);
    }
    app.emit_to("main", "settings-changed", &s)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Provider credential management (settings window)
// ---------------------------------------------------------------------------
#[tauri::command]
fn detect_credentials() -> Vec<credentials::ProviderCredInfo> {
    credentials::detect()
}

#[tauri::command]
fn save_manual_key(provider_id: String, key: String) -> Result<(), String> {
    let k = credentials::normalize_key(&key);
    if k.is_empty() {
        return Err("key 为空".into());
    }
    credentials::keyring_set(&provider_id, &k)
}

#[tauri::command]
fn delete_manual_key(provider_id: String) -> Result<(), String> {
    credentials::keyring_delete(&provider_id)
}

#[tauri::command]
fn set_provider_enabled(provider_id: String, enabled: bool) -> Result<(), String> {
    let mut s = settings::load();
    if enabled && !s.enabled_providers.contains(&provider_id) {
        s.enabled_providers.push(provider_id);
    } else if !enabled {
        s.enabled_providers.retain(|p| p != &provider_id);
    }
    settings::save(&s).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_custom_providers() -> Vec<settings::CustomProvider> {
    settings::load().custom_providers
}

#[tauri::command]
fn save_custom_provider(def: settings::CustomProvider, key: Option<String>) -> Result<(), String> {
    if def.name.trim().is_empty() || def.endpoint.trim().is_empty() {
        return Err("名称与端点 URL 不能为空".into());
    }
    if let Some(k) = &key {
        let k = credentials::normalize_key(k);
        if !k.is_empty() {
            credentials::keyring_set(&format!("custom/{}", def.id), &k)?;
        }
    }
    let mut s = settings::load();
    s.custom_providers.retain(|p| p.id != def.id);
    s.custom_providers.push(def);
    settings::save(&s).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_custom_provider(id: String) -> Result<(), String> {
    let mut s = settings::load();
    s.custom_providers.retain(|p| p.id != id);
    settings::save(&s).map_err(|e| e.to_string())?;
    let _ = credentials::keyring_delete(&format!("custom/{}", id));
    Ok(())
}

/// Verify a manual key or custom provider by making one real request.
#[tauri::command]
async fn verify_provider(provider_id: String, custom_id: Option<String>) -> Result<String, String> {
    if let Some(cid) = custom_id {
        let s = settings::load();
        let def = s
            .custom_providers
            .iter()
            .find(|p| p.id == cid)
            .cloned()
            .ok_or("自定义监视不存在")?;
        let key = credentials::keyring_get(&format!("custom/{}", cid)).unwrap_or_default();
        return fetch::verify_custom(&def, &key).await;
    }
    let (endpoint, header, prefix) = credentials::manual_key_target(&provider_id)
        .ok_or("该平台不支持手动 key")?;
    let key = credentials::keyring_get(&provider_id).ok_or("尚未保存 key")?;
    let (status, body) = fetch::get_with_auth(endpoint, header, prefix, &key).await?;
    match status {
        200..=299 => Ok(format!("验证成功（HTTP {}）", status)),
        401 | 403 => Err(format!("HTTP {} — key 无效或已过期，去控制台重新生成", status)),
        429 => Err("HTTP 429 — 请求太频繁，30 秒后再试".into()),
        _ => Err(format!("HTTP {} — {}", status, body.chars().take(120).collect::<String>())),
    }
}

// ---------------------------------------------------------------------------
fn main() {
    // AUMID: makes Windows toasts attributable to QuotaBar (E5/M5).
    #[cfg(target_os = "windows")]
    unsafe {
        let wide: Vec<u16> = "com.quotabar.app\0".encode_utf16().collect();
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // second instance: show existing window, no focus steal
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            context_menu,
            get_settings,
            accept_tos,
            decline_tos,
            jslog,
            begin_drag,
            autosize,
            apply_appearance,
            detect_credentials,
            save_manual_key,
            delete_manual_key,
            set_provider_enabled,
            list_custom_providers,
            save_custom_provider,
            delete_custom_provider,
            verify_provider,
            set_autostart,
            copy_diagnostics,
            refresh_now_cmd,
            check_update_cmd,
            open_url,
            get_history,
            updater_check::skip_version,
            updater_check::current_version,
        ])
        .setup(|app| {
            let s = settings::load();

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("QuotaBar")
                .inner_size(s.width, WIN_H)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()?;

            clamp_position(&window, &s)?;
            // Spike A: acrylic (undocumented SetWindowCompositionAttribute under the hood).
            // Fallback per design tokens: if acrylic fails, raise opacity to 0.88.
            #[cfg(target_os = "windows")]
            if window_vibrancy::apply_acrylic(&window, Some((18, 18, 22, 184))).is_err() {
                let mut s2 = s.clone();
                s2.opacity = 0.88;
                let _ = settings::save(&s2);
            }
            #[cfg(target_os = "macos")]
            if window_vibrancy::apply_vibrancy(
                &window,
                window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground,
                None,
                None,
            )
            .is_err()
            {
                rustlog("macOS vibrancy failed; continuing without".into());
            }
            if s.mini_mode {
                // frontend owns height: emit after page load is racy, so just
                // mark state; index.html applies .mini + autosize on load
                let _ = window.emit_to("main", "mini-mode", true);
            }

            // tray (E2)
            let menu = build_app_menu(app.handle())?;
            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon_image())
                .tooltip("QuotaBar")
                .menu(&menu)
                // NOTE: no .on_menu_event here — Tauri dispatches menu events to
                // BOTH the tray handler and the global Builder::on_menu_event,
                // which would double-toggle. Global handler is the single entry.
                .build(app)?;
            poller::register_tray(tray);

            // sync autostart registry key with persisted setting
            let _ = apply_autostart(s.autostart);

            // First-run ToS gate: zero network before consent. Until the user
            // agrees, the bar hides and the poller stays off; the ToS window
            // is the only UI. Closing it without agreeing exits the app.
            if s.tos_accepted {
                reveal_main(&window);
                poller::start(app.handle().clone());
                // E1: version check only after consent (zero network before)
                updater_check::maybe_check(app.handle().clone(), false);
            } else {
                open_tos_window(app.handle());
            }
            Ok(())
        })
        .on_menu_event(|app, ev| handle_menu_event(app, ev.id().as_ref()))
        .on_window_event(|window, ev| {
            if let tauri::WindowEvent::Destroyed = ev {
                // closing the ToS window without consent = decline: exit,
                // still zero network requests made
                if window.label() == "tos" && !settings::load().tos_accepted {
                    window.app_handle().exit(0);
                    return;
                }
                // persist position only for the bar itself — the settings/tos
                // windows must not clobber it
                if window.label() != "main" {
                    return;
                }
                if let Ok(pos) = window.outer_position() {
                    let sf = window.scale_factor().unwrap_or(1.0);
                    let mut s = settings::load();
                    s.window_x = Some(pos.x as f64 / sf);
                    s.window_y = Some(pos.y as f64 / sf);
                    s.monitor_name = window
                        .current_monitor()
                        .ok()
                        .flatten()
                        .and_then(|m| m.name().cloned());
                    let _ = settings::save(&s);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running DeskToken");
}
