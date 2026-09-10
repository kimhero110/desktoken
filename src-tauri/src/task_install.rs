//! Local integration edits: preserve unrelated settings; own only exact installed definitions.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::atomic_file::create_like;
use std::{io::{Read, Write}, path::Path};

#[derive(Serialize, Deserialize)]
struct Receipt { tool: String, definitions: Vec<String> }
#[derive(Serialize)]
pub struct InstallResult { pub changed: bool, pub path: String, pub backup: Option<String> }

fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(2_097_153).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if bytes.len() > 2_097_152 { return Err("配置文件超过 2 MiB，未修改".into()); }
            Ok(Some(bytes))
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn merge(raw: Option<&[u8]>, definitions: &[String], current: &str, install: bool) -> Result<Vec<u8>, String> {
    let mut doc: Value = match raw {
        Some(bytes) => serde_json::from_slice(bytes).map_err(|_| "配置 JSON 无效，未修改")?,
        None => serde_json::json!({}),
    };
    let root = doc.as_object_mut().ok_or("配置根节点必须是对象，未修改")?;
    let preserve_empty_hooks = root.get("hooks").and_then(Value::as_object).is_some_and(|h| h.is_empty());
    let hooks = root.entry("hooks").or_insert_with(|| serde_json::json!({})).as_object_mut().ok_or("hooks 必须是对象，未修改")?;
    let mut known = Vec::new();
    for definition in definitions {
        let v: Value = serde_json::from_str(definition).map_err(|_| "接入记录无效，未修改")?;
        let events = v["hooks"].as_object().ok_or("接入记录缺少 hooks")?;
        for (event, groups) in events {
            for group in groups.as_array().ok_or("接入记录无效")? {
                for h in group["hooks"].as_array().ok_or("接入记录无效")? { known.push((event.clone(), h.clone())); }
            }
        }
    }
    let mut removed_events = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let groups = groups.as_array_mut().ok_or("事件配置必须是数组，未修改")?;
        let mut emptied = Vec::new();
        for (index, group) in groups.iter_mut().enumerate() {
            if group.get("matcher").is_some_and(|m| !m.is_string()) { return Err("matcher 必须是字符串，未修改配置".into()); }
            let handlers = group.get_mut("hooks").and_then(Value::as_array_mut).ok_or("hook 配置结构无效，未修改")?;
            if handlers.iter().any(|h| known.iter().any(|(e, owned)| e == event && h["command"] == owned["command"])
                && !known.iter().any(|(e, owned)| e == event && h == owned)) {
                return Err("已安装的 QuotaBar hook 被手动修改，未自动覆盖或移除；请先核对该条目".into());
            }
            let before = handlers.len();
            handlers.retain(|h| !known.iter().any(|(e, owned)| e == event && h == owned));
            if before > 0 && handlers.is_empty() { emptied.push(index); }
        }
        for index in emptied.iter().rev() { groups.remove(*index); }
        if !emptied.is_empty() && groups.is_empty() { removed_events.push(event.clone()); }
    }
    for event in removed_events { hooks.remove(&event); }
    if install {
        let wanted: Value = serde_json::from_str(current).map_err(|e| e.to_string())?;
        for (event, groups) in wanted["hooks"].as_object().ok_or("生成配置无效")? {
            hooks.entry(event.clone()).or_insert_with(|| serde_json::json!([])).as_array_mut().ok_or("事件配置无效")?.extend(groups.as_array().ok_or("生成配置无效")?.iter().cloned());
        }
    }
    if hooks.is_empty() && !preserve_empty_hooks { root.remove("hooks"); }
    serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())
}

/// The lock serializes this installer's processes. Recheck detects edits from other programs.
pub fn change(path: &Path, tool: &str, current: &str, install: bool) -> Result<InstallResult, String> {
    let parent = path.parent().ok_or("无效配置路径")?;
    if !install && !path.exists() { return Ok(InstallResult { changed:false,path:path.to_string_lossy().into(),backup:None }); }
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let lock = path.with_extension("quotabar-lock");
    if std::fs::symlink_metadata(&lock).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("接入锁文件是符号链接，未修改配置".into());
    }
    let guard = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&lock).map_err(|e| format!("无法打开接入锁：{e}"))?;
    guard.try_lock().map_err(|e| format!("无法取得接入锁，可能有安装器正在修改配置：{e}"))?;
    let result = change_locked(path, tool, current, install);
    // Keep the file: unlinking after unlock allows concurrent processes to lock different inodes.
    // The OS releases the actual lock even when the installer is forcibly terminated.
    drop(guard);
    result
}
fn change_locked(path: &Path, tool: &str, current: &str, install: bool) -> Result<InstallResult, String> {
    let receipt_path = path.with_extension("quotabar-receipt.json");
    for p in [path, receipt_path.as_path()] {
        if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) { return Err("配置或接入记录是符号链接，请手动合并以保留链接".into()); }
    }
    let original = read(path)?;
    let mut definitions = match read(&receipt_path)? {
        Some(bytes) => {
            let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|_| "接入记录损坏，未修改配置")?;
            if receipt.tool != tool { return Err("接入记录工具不匹配，未修改".into()); }
            receipt.definitions
        },
        None => Vec::new(),
    };
    if !definitions.iter().any(|v| v == current) { definitions.push(current.into()); }
    let next = if tool == "opencode" {
        if let Some(bytes) = &original {
            if !definitions.iter().any(|v| v.replace("\r\n","\n").as_bytes() == String::from_utf8_lossy(bytes).replace("\r\n","\n").as_bytes()) {
                return Err("同名 OpenCode 插件已有自定义内容，未覆盖；请先手动核对".into());
            }
        }
        if install { Some(current.as_bytes().to_vec()) } else { None }
    } else { Some(merge(original.as_deref(), &definitions, current, install)?) };
    let same = if tool == "opencode" { original == next } else {
        original.as_deref().and_then(|v| serde_json::from_slice::<Value>(v).ok()) == next.as_deref().and_then(|v| serde_json::from_slice::<Value>(v).ok())
    };
    let mut report = InstallResult { changed:!same, path:path.to_string_lossy().into(), backup:None };
    if same && !install { return Ok(report); }
    // Keep receipts for crash recovery, before committing the config. No credentials in receipts.
    if definitions.len() > 32 { return Err("接入历史超过上限，请手动核对后清理接入记录".into()); }
    crate::atomic_file::replace(&receipt_path, &serde_json::to_vec(&Receipt {tool:tool.into(),definitions}).map_err(|e| e.to_string())?, 1, 0).map_err(|e| e.to_string())?;
    if same { return Ok(report); }
    if let Some(bytes) = &original {
        let backup = path.with_extension(format!("quotabar-backup-{}-{}.bak", chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(), std::process::id()));
        let mut f = create_like(&backup, Some(path)).map_err(|e| e.to_string())?;
        f.set_permissions(std::fs::metadata(path).map_err(|e| e.to_string())?.permissions()).map_err(|e| e.to_string())?;
        f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
        report.backup = Some(backup.to_string_lossy().into());
    }
    if read(path)? != original { return Err("配置被其他程序更新，本次未写入；请重试".into()); }
    match next { Some(bytes) => crate::atomic_file::replace(path, &bytes, 1, 0).map_err(|e| e.to_string())?, None => std::fs::remove_file(path).map_err(|e| e.to_string())? }
    Ok(report)
}

#[tauri::command]
pub fn set_task_integration(tool: String, installed: bool) -> Result<InstallResult, String> {
    let path = crate::task_integration::default_path(&tool)?;
    let current = crate::task_integration::task_integration_config(tool.clone())?;
    change(&path, &tool, &current.content, installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Native Windows security APIs only — the test must not shell out to
    // PowerShell (native-cleanup goal: zero runtime PowerShell invocation).
    #[cfg(windows)]
    fn acl_sddl(path: &Path, protect: bool) -> String {
        if protect {
            protect_with_guests_read_deny(path);
        }
        // CreateFile may clear the historical AUTO_INHERITED marker. Compare
        // the owner, group, protection flag and all actual ACEs, including
        // their inheritance flags.
        sddl_of(path).replace("D:PAI", "D:P").replace("D:AI", "D:")
    }
    /// Read a file's SDDL (owner+group+DACL) via GetFileSecurityW +
    /// ConvertSecurityDescriptorToStringSecurityDescriptorW. Panics on failure
    /// (fixture setup must never silently pass).
    #[cfg(windows)]
    fn sddl_of(path: &Path) -> String {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
        use windows_sys::Win32::Security::{
            GetFileSecurityW, DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION,
            OWNER_SECURITY_INFORMATION,
        };
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let info =
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let mut needed = 0;
        unsafe { GetFileSecurityW(wide.as_ptr(), info, std::ptr::null_mut(), 0, &mut needed) };
        assert!(needed > 0, "GetFileSecurityW size probe failed");
        let mut buf = vec![0u8; needed as usize];
        assert_ne!(
            unsafe { GetFileSecurityW(wide.as_ptr(), info, buf.as_mut_ptr().cast(), needed, &mut needed) },
            0,
            "GetFileSecurityW read failed"
        );
        let mut out = std::ptr::null_mut();
        let mut len = 0u32;
        assert_ne!(
            unsafe {
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    buf.as_mut_ptr().cast(),
                    1, // SDDL_REVISION_1
                    info,
                    &mut out,
                    &mut len,
                )
            },
            0,
            "SDDL conversion failed"
        );
        let text = unsafe {
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(out, len as usize))
                .trim_end_matches('\u{0}')
                .to_string();
            LocalFree(out.cast());
            s
        };
        assert!(text.contains("D:"), "SDDL missing DACL: {text}");
        text
    }
    /// Native equivalent of the old PowerShell fixture: protect the DACL from
    /// inheritance (preserving the existing ACEs verbatim) and prepend a Deny
    /// ReadData ACE for Guests (S-1-5-32-546), applied via SetFileSecurityW.
    #[cfg(windows)]
    fn protect_with_guests_read_deny(path: &Path) {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
        use windows_sys::Win32::Security::{
            SetFileSecurityW, DACL_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION,
            OWNER_SECURITY_INFORMATION,
        };
        let current = sddl_of(path);
        let (prefix, aces) = match current.find('(') {
            Some(i) => (&current[..i], &current[i..]),
            None => panic!("fixture SDDL has no ACE list: {current}"),
        };
        let dacl = prefix.find("D:").unwrap_or_else(|| panic!("fixture SDDL missing DACL marker: {current}"));
        // D:P = protected; deny ACE first (canonical order); inherited ACEs copied verbatim.
        let next = format!("{}D:P(D;;FR;;;S-1-5-32-546){aces}", &prefix[..dacl]);
        let sddl_wide: Vec<u16> = next.encode_utf16().chain(Some(0)).collect();
        let mut sd = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl_wide.as_ptr(),
                    1, // SDDL_REVISION_1
                    &mut sd,
                    std::ptr::null_mut(),
                )
            },
            0,
            "SDDL parse failed: {next}"
        );
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let info =
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let applied = unsafe { SetFileSecurityW(wide.as_ptr(), info, sd.cast()) };
        unsafe { LocalFree(sd.cast()) };
        assert_ne!(applied, 0, "SetFileSecurityW failed");
    }
    #[cfg(windows)]
    #[test]
    fn installation_and_backups_preserve_protected_windows_acl() {
        check_windows_acl(true);
    }
    #[cfg(windows)]
    #[test]
    fn installation_and_backups_preserve_inherited_windows_acl() {
        check_windows_acl(false);
    }
    #[cfg(windows)]
    fn check_windows_acl(protect: bool) {
        let dir=crate::settings::app_data_dir().join(if protect { "protected-acl" } else { "inherited-acl" });
        std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");
        std::fs::write(&path,br#"{"env":{"fixture":"keep"}}"#).unwrap();
        let expected=acl_sddl(&path,protect);
        assert_eq!(expected.contains("D:P"),protect,"fixture inheritance mismatch");
        for (command,install) in [("receiver-v1",true),("receiver-v2",true),("receiver-v2",false)] {
            let report=change(&path,"claude",&own(command),install).unwrap();
            assert!(report.changed);
            assert_eq!(acl_sddl(&path,false),expected,"configuration ACL changed");
            assert_eq!(acl_sddl(Path::new(report.backup.as_ref().unwrap()),false),expected,"backup ACL changed");
        }
    }
    #[cfg(windows)]
    #[test]
    fn missing_security_source_must_not_create_copy_with_default_acl() {
        let dir=crate::settings::app_data_dir().join("missing-acl-source");
        std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("copy.json");
        assert!(create_like(&path,Some(&dir.join("missing.json"))).is_err());
        assert!(!path.exists());
    }
    #[test]
    fn stale_lock_file_must_not_block_installation() {
        let dir=crate::settings::app_data_dir().join("stale-install-lock");
        std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");
        std::fs::write(path.with_extension("quotabar-lock"), b"").unwrap();
        assert!(change(&path,"claude",&own("receiver"),true).is_ok());
    }
    #[test]
    fn active_lock_blocks_changes_and_release_allows_retry() {
        let dir=crate::settings::app_data_dir().join("active-install-lock");
        std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");
        let lock=std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path.with_extension("quotabar-lock")).unwrap();
        lock.try_lock().unwrap();
        assert!(change(&path,"claude",&own("receiver"),true).is_err());
        assert!(!path.exists());
        assert!(!path.with_extension("quotabar-receipt.json").exists());
        drop(lock);
        assert!(change(&path,"claude",&own("receiver"),true).unwrap().changed);
    }
    #[test]
    fn lock_holder_process() {
        let Some(path)=std::env::var_os("QUOTABAR_TEST_LOCK_PATH") else { return; };
        let guard=std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&path).unwrap();
        guard.try_lock().unwrap();
        std::fs::write(Path::new(&path).with_extension("ready"), b"ready").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
    #[test]
    fn killed_installer_releases_lock_without_manual_cleanup() {
        let dir=crate::settings::app_data_dir().join("killed-install-lock");
        std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");
        let lock=path.with_extension("quotabar-lock");
        let ready=lock.with_extension("ready");
        let mut child=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","task_install::tests::lock_holder_process"])
            .env("QUOTABAR_TEST_LOCK_PATH", &lock)
            .stdout(std::process::Stdio::null()).spawn().unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
        while !ready.exists() && std::time::Instant::now()<deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let acquired=ready.exists();
        let blocked=acquired && change(&path,"claude",&own("receiver"),true).is_err();
        let _=child.kill();
        child.wait().unwrap();
        assert!(acquired,"child did not acquire lock");
        assert!(blocked,"active child failed to exclude installer");
        assert!(lock.exists());
        assert!(change(&path,"claude",&own("receiver"),true).unwrap().changed);
    }
    #[test]
    fn atomic_failure_must_not_delete_preexisting_temp_file() {
        let dir=crate::settings::app_data_dir().join("atomic-collision");std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("config.json");
        let tmp=path.with_extension(format!("quotabar-{}.tmp",std::process::id()));
        std::fs::write(&tmp,b"another writer").unwrap();
        assert!(crate::atomic_file::replace(&path,b"replacement",1,0).is_err());
        assert_eq!(std::fs::read(&tmp).unwrap(),b"another writer");
        std::fs::remove_file(tmp).unwrap();
    }
    #[test]
    fn uninstall_without_owned_hooks_preserves_empty_hooks_object() {
        let raw=br#"{"hooks":{},"user":true}"#;
        let result=merge(Some(raw),&[],&own("receiver"),false).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&result).unwrap(),serde_json::from_slice::<Value>(raw).unwrap());
    }
    fn own(cmd: &str) -> String { serde_json::json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":cmd}]}]}}).to_string() }
    #[test]
    fn upgrade_remove_preserves_other_hooks_and_settings() {
        let old=own("old receiver"); let new=own("new receiver");
        let base=serde_json::json!({"env":{"SECRET":"keep"},"hooks":{"Stop":[{"matcher":"*","hooks":[{"type":"command","command":"user hook"}]}]}}).to_string();
        let first=merge(Some(base.as_bytes()), &[old.clone()],&old,true).unwrap();
        let updated=merge(Some(&first), &[old,new.clone()],&new,true).unwrap();
        let repeated=merge(Some(&updated), &[new.clone()],&new,true).unwrap();
        assert_eq!(updated,repeated);
        let removed=merge(Some(&updated), &[new.clone()],&new,false).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&removed).unwrap(),serde_json::from_str::<Value>(&base).unwrap());
    }
    #[test]
    fn invalid_shapes_are_rejected() {
        for raw in ["{", "[]", r#"{"hooks":[]}"#, r#"{"hooks":{"Stop":{}}}"#, r#"{"hooks":{"Stop":[{"matcher":42,"hooks":[]}]}}"#] { assert!(merge(Some(raw.as_bytes()), &[], &own("x"),true).is_err()); }
    }
    #[test]
    fn roundtrip_preserves_unrelated_handlers_across_many_shapes() {
        let current=own("receiver");
        for n in 0..128 {
            let groups:Vec<Value>=(0..n%7).map(|i| serde_json::json!({"matcher":format!("tool{i}"),"hooks":[{"type":"command","command":format!("user{i}"),"timeout":n+1}]})).collect();
            let base=serde_json::json!({"permissions":{"allow":[format!("read{n}")]},"hooks":{"OtherEvent":groups,"Stop":[{"hooks":[{"type":"prompt","prompt":"keep this"}]}]}});
            let raw=base.to_string();
            let installed=merge(Some(raw.as_bytes()),&[current.clone()],&current,true).unwrap();
            let removed=merge(Some(&installed),&[current.clone()],&current,false).unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&removed).unwrap(),base,"case {n}");
        }
    }
    #[test]
    fn disk_backups_idempotency_and_plugin_conflict() {
        let dir=crate::settings::app_data_dir().join("install-test");std::fs::create_dir_all(&dir).unwrap();
        let path=dir.join("settings.json");std::fs::write(&path,b"{\"user\":true}").unwrap();
        let a=change(&path,"claude",&own("receiver"),true).unwrap();
        assert_eq!(std::fs::read(a.backup.unwrap()).unwrap(),b"{\"user\":true}");
        assert!(!change(&path,"claude",&own("receiver"),true).unwrap().changed);
        change(&path,"claude",&own("receiver"),false).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap(),serde_json::json!({"user":true}));
        let plugin=dir.join("plugin.js");std::fs::write(&plugin,"user code").unwrap();
        assert!(change(&plugin,"opencode","our code",true).is_err());
        assert_eq!(std::fs::read_to_string(plugin).unwrap(),"user code");
    }
    #[test]
    fn edited_owned_handler_is_not_deleted() {
        let original=own("receiver");
        let mut edited:Value=serde_json::from_str(&original).unwrap();
        edited["hooks"]["Stop"][0]["hooks"][0]["timeout"]=serde_json::json!(42);
        let raw=edited.to_string();
        assert!(merge(Some(raw.as_bytes()), &[original.clone()],&original,false).is_err());
    }
}
