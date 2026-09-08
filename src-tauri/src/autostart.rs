//! Autostart via a Startup-folder shortcut (.lnk), created with Shell COM.
//!
//! Principles (native-cleanup + ownership review):
//! - Normal startup NEVER writes autostart state. The only mutation path is
//!   the user's explicit set_autostart toggle.
//! - We own exactly one artifact: `<Startup>\QuotaBar.lnk`. A foreign or
//!   user-modified shortcut with that name is never overwritten or deleted —
//!   that is a conflict, reported to the user.
//! - Legacy migration: versions ≤ 0.3.2 wrote `HKCU\...\Run -> "QuotaBar"`
//!   (unquoted exe path). An existing enabled Run value keeps working until the
//!   user toggles autostart; on the first successful toggle we create/remove
//!   the owned .lnk FIRST, and only then delete a matching legacy Run value.
//!   A Run value named QuotaBar that does not reference this executable is a
//!   conflict: error out, preserve all data.

#[cfg(target_os = "windows")]
const SHORTCUT_NAME: &str = "QuotaBar.lnk";
#[cfg(target_os = "windows")]
const LEGACY_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const LEGACY_VALUE_NAME: &str = "QuotaBar";

/// User-initiated toggle (Tauri command backend). No-op on non-Windows.
#[cfg(not(target_os = "windows"))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if enabled {
        enable(&exe)
    } else {
        disable(&exe)
    }
}

/// Per-user Startup folder via SHGetKnownFolderPath (respects folder
/// redirection; no shell invocation, no APPDATA string concatenation).
#[cfg(target_os = "windows")]
fn startup_dir() -> Result<std::path::PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Startup, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath};
    unsafe {
        // windows 0.61.3: (rfid, flags, htoken) -> Result<PWSTR>
        let ptr = SHGetKnownFolderPath(&FOLDERID_Startup, KNOWN_FOLDER_FLAG(0), None)
            .map_err(|e| format!("无法定位 Startup 文件夹：{e}"))?;
        if ptr.is_null() {
            return Err("无法定位 Startup 文件夹".into());
        }
        let mut len = 0usize;
        while *ptr.0.add(len) != 0 {
            len += 1;
        }
        let dir = std::path::PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(
            ptr.0, len,
        )));
        CoTaskMemFree(Some(ptr.0.cast()));
        Ok(dir)
    }
}

#[cfg(target_os = "windows")]
fn enable(exe: &std::path::Path) -> Result<(), String> {
    enable_in(&startup_dir()?, exe)?;
    clean_legacy_run(exe)
}

#[cfg(target_os = "windows")]
fn disable(exe: &std::path::Path) -> Result<(), String> {
    disable_in(&startup_dir()?, exe)?;
    clean_legacy_run(exe)
}

/// Enable against an explicit Startup directory (tests use a temp dir).
#[cfg(target_os = "windows")]
fn enable_in(startup: &std::path::Path, exe: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(startup).map_err(|e| e.to_string())?;
    write_shortcut(&startup.join(SHORTCUT_NAME), exe)
}

/// Disable against an explicit Startup directory. Deletes ONLY the .lnk we own
/// (target verified via COM); foreign/mismatched shortcuts are preserved and
/// reported as errors instead of silently claiming success.
#[cfg(target_os = "windows")]
fn disable_in(startup: &std::path::Path, exe: &std::path::Path) -> Result<(), String> {
    let link = startup.join(SHORTCUT_NAME);
    match std::fs::symlink_metadata(&link) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
        Ok(_) => match shortcut_target(&link) {
            Ok(Some(target)) if paths_equal(&target, exe) => {
                std::fs::remove_file(&link).map_err(|e| e.to_string())?;
            }
            Ok(_) => return Err(
                "Startup 中的 QuotaBar.lnk 不是本程序创建，未删除；未修改任何数据".into(),
            ),
            Err(e) => return Err(format!(
                "无法核对 Startup 快捷方式目标（{e}），未删除；未修改任何数据"
            )),
        },
    }
    Ok(())
}

/// After a successful shortcut operation: delete the legacy HKCU Run value
/// ONLY when it clearly belongs to this executable. Errors (including a
/// conflicting foreign value named QuotaBar) propagate to the caller so the
/// settings layer can report failure instead of silently keeping autostart
/// active in two places.
#[cfg(target_os = "windows")]
fn clean_legacy_run(exe: &std::path::Path) -> Result<(), String> {
    use winreg::enums::{KEY_READ, KEY_SET_VALUE};
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(LEGACY_RUN_KEY, KEY_READ | KEY_SET_VALUE) {
        Ok(key) => clean_legacy_run_in(&key, exe),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("打开旧版自启动 Run 键失败：{e}")),
    }
}

/// Decision core, factored for tests against a throwaway subkey.
#[cfg(target_os = "windows")]
fn clean_legacy_run_in(key: &winreg::RegKey, exe: &std::path::Path) -> Result<(), String> {
    match key.get_value::<String, _>(LEGACY_VALUE_NAME) {
        Ok(value) => {
            if legacy_value_is_ours(&value, exe) {
                key.delete_value(LEGACY_VALUE_NAME).map_err(|e| e.to_string())
            } else {
                Err(format!(
                    "注册表 Run 键中名为 QuotaBar 的值指向其他程序（{value}），请手动处理；未修改任何数据"
                ))
            }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(ref e) if e.kind() == std::io::ErrorKind::InvalidData => Err(
            "注册表 Run 键中名为 QuotaBar 的值不是字符串，可能被其他程序占用，请手动处理；未修改任何数据"
                .into(),
        ),
        Err(e) => Err(e.to_string()),
    }
}

/// The 0.3.x stable writer stored exactly `<exe>` (unquoted, no arguments).
/// Case-insensitive on the path; accepts the quoted form defensively.
#[cfg(target_os = "windows")]
fn legacy_value_is_ours(value: &str, exe: &std::path::Path) -> bool {
    let trimmed = value.trim();
    let exe = exe.to_string_lossy();
    trimmed.eq_ignore_ascii_case(&format!("\"{exe}\""))
        || trimmed.eq_ignore_ascii_case(exe.as_ref())
}

/// Case-insensitive path equality with a canonicalize fallback (different
/// spellings of the same file, e.g. 8.3 short names or subst drives).
#[cfg(target_os = "windows")]
fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca.eq_ignore_ascii_case(&cb),
        _ => false,
    }
}

#[cfg(target_os = "windows")]
trait EqIgnoreAsciiCasePath {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool;
}
#[cfg(target_os = "windows")]
impl EqIgnoreAsciiCasePath for std::path::Path {
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        let a = self.to_string_lossy().to_lowercase();
        let b = other.to_string_lossy().to_lowercase();
        a == b
    }
}

// ---------------------------------------------------------------------------
// Shell COM: .lnk write/read via IShellLinkW + IPersistFile (no scripting
// host, no WScript.Shell, no PowerShell).
// ---------------------------------------------------------------------------
#[cfg(target_os = "windows")]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
fn write_shortcut(link: &std::path::Path, exe: &std::path::Path) -> Result<(), String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    // Ownership guard: never overwrite an existing QuotaBar.lnk we cannot
    // prove points at this executable.
    if std::fs::symlink_metadata(link).is_ok() {
        match shortcut_target(link) {
            Ok(Some(target)) if paths_equal(&target, exe) => {} // ours: refresh in place
            Ok(_) => {
                return Err(
                    "Startup 中已存在名为 QuotaBar.lnk 但指向其他程序的快捷方式，未覆盖".into(),
                )
            }
            Err(e) => {
                return Err(format!(
                    "无法核对已有的 QuotaBar.lnk（{e}），未覆盖"
                ))
            }
        }
    }

    unsafe {
        // CoInitializeEx returns HRESULT; S_FALSE (already initialized on this
        // thread) is a success code and must be balanced by one CoUninitialize.
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| format!("COM 初始化失败：{e}"))?;
        let result = (|| -> Result<(), String> {
            let shell_link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("创建快捷方式对象失败：{e}"))?;
            let exe_w = wide(&exe.to_string_lossy());
            shell_link
                .SetPath(PCWSTR(exe_w.as_ptr()))
                .map_err(|e| format!("设置快捷方式目标失败：{e}"))?;
            shell_link
                .SetDescription(PCWSTR(wide("QuotaBar autostart").as_ptr()))
                .map_err(|e| format!("设置快捷方式描述失败：{e}"))?;
            let persist: IPersistFile = shell_link
                .cast()
                .map_err(|e| format!("快捷方式接口转换失败：{e}"))?;
            let link_w = wide(&link.to_string_lossy());
            persist
                .Save(PCWSTR(link_w.as_ptr()), true)
                .map_err(|e| format!("写入快捷方式失败：{e}"))
        })();
        CoUninitialize();
        result
    }
}

/// Read a .lnk's target path. Ok(None) = no/empty target; Err = unreadable.
#[cfg(target_os = "windows")]
fn shortcut_target(link: &std::path::Path) -> Result<Option<std::path::PathBuf>, String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| format!("COM 初始化失败：{e}"))?;
        let result = (|| -> Result<Option<std::path::PathBuf>, String> {
            let shell_link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("创建快捷方式对象失败：{e}"))?;
            let persist: IPersistFile = shell_link
                .cast()
                .map_err(|e| format!("快捷方式接口转换失败：{e}"))?;
            let link_w = wide(&link.to_string_lossy());
            persist
                .Load(PCWSTR(link_w.as_ptr()), STGM_READ)
                .map_err(|e| format!("读取快捷方式失败：{e}"))?;
            let mut buf = [0u16; 1024];
            shell_link
                .GetPath(&mut buf, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)
                .map_err(|e| format!("读取快捷方式目标失败：{e}"))?;
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            if len == 0 {
                return Ok(None);
            }
            Ok(Some(std::path::PathBuf::from(String::from_utf16_lossy(
                &buf[..len],
            ))))
        })();
        CoUninitialize();
        result
    }
}

// ---------------------------------------------------------------------------
// Tests: temp dirs and throwaway registry subkeys only — the real Startup
// folder and the real Run key are never touched.
// ---------------------------------------------------------------------------
#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;

    #[test]
    fn legacy_value_matching_is_strict() {
        let exe = std::path::Path::new(r"C:\Program Files\QuotaBar\quotabar.exe");
        assert!(legacy_value_is_ours(
            "\"C:\\Program Files\\QuotaBar\\quotabar.exe\"",
            exe
        ));
        // case-insensitive path
        assert!(legacy_value_is_ours(
            "\"c:\\program files\\quotabar\\QUOTABAR.EXE\"",
            exe
        ));
        // unquoted 0.3.x stable form
        assert!(legacy_value_is_ours(
            "C:\\Program Files\\QuotaBar\\quotabar.exe",
            exe
        ));
        // foreign values are never ours
        assert!(!legacy_value_is_ours("\"C:\\Other\\tool.exe\"", exe));
        assert!(!legacy_value_is_ours(
            "\"C:\\Program Files\\QuotaBar\\quotabar.exe\" --arg",
            exe
        ));
        assert!(!legacy_value_is_ours("", exe));
    }

    /// Throwaway HKCU subkey (PID + counter: parallel tests never share).
    struct TempKeyGuard(String);
    impl Drop for TempKeyGuard {
        fn drop(&mut self) {
            let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let _ = hkcu.delete_subkey_all(&self.0);
        }
    }
    static TEMP_KEY_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    fn temp_key() -> (TempKeyGuard, winreg::RegKey) {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let n = TEMP_KEY_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let full_path = format!(
            r"Software\QuotaBarAutostartLegacyTest_{}_{}",
            std::process::id(),
            n
        );
        let (key, _) = hkcu.create_subkey(&full_path).expect("create temp subkey");
        (TempKeyGuard(full_path), key)
    }

    #[test]
    fn legacy_cleanup_removes_only_matching_value() {
        let exe = std::path::Path::new(r"C:\Program Files\QuotaBar\quotabar.exe");
        // absent value: success, no writes
        let (guard, key) = temp_key();
        clean_legacy_run_in(&key, exe).unwrap();
        // matching value + unrelated sibling: only ours is deleted
        key.set_value(
            LEGACY_VALUE_NAME,
            &"\"C:\\Program Files\\QuotaBar\\quotabar.exe\"",
        )
        .unwrap();
        key.set_value("OtherApp", &"keep").unwrap();
        clean_legacy_run_in(&key, exe).unwrap();
        assert!(key.get_value::<String, _>(LEGACY_VALUE_NAME).is_err());
        assert_eq!(key.get_value::<String, _>("OtherApp").unwrap(), "keep");
        drop(guard);
    }

    #[test]
    fn legacy_cleanup_conflicting_value_is_clear_error_and_preserved() {
        let exe = std::path::Path::new(r"C:\Program Files\QuotaBar\quotabar.exe");
        let (guard, key) = temp_key();
        key.set_value(LEGACY_VALUE_NAME, &"\"C:\\somewhere else.exe\"").unwrap();
        let err = clean_legacy_run_in(&key, exe).unwrap_err();
        assert!(err.contains("请手动处理"), "clear conflict message: {err}");
        // data preserved untouched
        assert_eq!(
            key.get_value::<String, _>(LEGACY_VALUE_NAME).unwrap(),
            "\"C:\\somewhere else.exe\""
        );
        drop(guard);
    }

    #[test]
    fn shortcut_roundtrip_and_ownership_semantics() {
        let dir = crate::settings::app_data_dir().join("autostart-lnk");
        std::fs::create_dir_all(&dir).unwrap();
        let startup = dir.join("Startup 拷 贝"); // spaces + Unicode
        let exe = std::env::current_exe().unwrap();
        // enable: .lnk created and points back at our exe
        enable_in(&startup, &exe).unwrap();
        let link = startup.join(SHORTCUT_NAME);
        assert!(link.is_file());
        assert!(paths_equal(&shortcut_target(&link).unwrap().unwrap(), &exe));
        // enable is idempotent (owned link: overwrite in place allowed)
        enable_in(&startup, &exe).unwrap();
        assert!(link.is_file());
        // disable: removes the owned .lnk
        disable_in(&startup, &exe).unwrap();
        assert!(!link.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn disable_preserves_foreign_shortcut() {
        let dir = crate::settings::app_data_dir().join("autostart-foreign-lnk");
        std::fs::create_dir_all(&dir).unwrap();
        let startup = dir.join("Startup");
        std::fs::create_dir_all(&startup).unwrap();
        let foreign = dir.join("foreign-app.exe");
        std::fs::write(&foreign, b"stub").unwrap();
        write_shortcut(&startup.join(SHORTCUT_NAME), &foreign).unwrap();
        let mine = std::env::current_exe().unwrap();
        let err = disable_in(&startup, &mine).unwrap_err();
        assert!(err.contains("未删除"), "clear conflict message: {err}");
        assert!(
            startup.join(SHORTCUT_NAME).is_file(),
            "foreign QuotaBar-named shortcut must survive"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn enable_refuses_to_overwrite_foreign_shortcut() {
        let dir = crate::settings::app_data_dir().join("autostart-foreign-overwrite");
        std::fs::create_dir_all(&dir).unwrap();
        let startup = dir.join("Startup");
        std::fs::create_dir_all(&startup).unwrap();
        let foreign = dir.join("foreign-app.exe");
        std::fs::write(&foreign, b"stub").unwrap();
        write_shortcut(&startup.join(SHORTCUT_NAME), &foreign).unwrap();
        let mine = std::env::current_exe().unwrap();
        let err = enable_in(&startup, &mine).unwrap_err();
        assert!(err.contains("未覆盖"), "clear conflict message: {err}");
        // the foreign shortcut still points at the foreign exe
        let target = shortcut_target(&startup.join(SHORTCUT_NAME)).unwrap().unwrap();
        assert!(paths_equal(&target, &foreign));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn disable_without_shortcut_is_success() {
        let dir = crate::settings::app_data_dir().join("autostart-empty");
        std::fs::create_dir_all(&dir).unwrap();
        disable_in(&dir.join("Startup"), std::path::Path::new(r"C:\x\q.exe")).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
