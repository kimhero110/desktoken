// QuotaBar — atomic file replacement that preserves the target's security.
//
// std::fs::write creates with default permissions, so replacing a file through
// tmp+rename silently widens it: a 0600 credential file comes back 0644, and a
// protected Windows DACL is replaced by the parent's inherited one.
use std::io::Write;
use std::path::Path;

/// Create `path` for writing, carrying `source`'s owner/group/DACL (Windows)
/// or permission bits (Unix). Security is applied AT CREATION, before any
/// bytes exist, so a rotated secret is never briefly world-readable.
/// Fails if `path` already exists. std::fs::Permissions only represents the
/// read-only attribute on Windows, hence the descriptor copy.
pub fn create_like(path: &Path, source: Option<&Path>) -> std::io::Result<std::fs::File> {
    #[cfg(windows)]
    if let Some(source) = source {
        use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
        use windows_sys::Win32::{Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE},
            Security::{GetFileSecurityW, SECURITY_ATTRIBUTES, OWNER_SECURITY_INFORMATION, GROUP_SECURITY_INFORMATION, DACL_SECURITY_INFORMATION},
            Storage::FileSystem::{CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL}};
        let wide = |p: &Path| -> std::io::Result<Vec<u16>> {
            let mut v: Vec<_> = p.as_os_str().encode_wide().collect();
            if v.contains(&0) { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,"path contains NUL")); }
            v.push(0); Ok(v)
        };
        let original = wide(source)?;
        let destination = wide(path)?;
        let info = OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let mut needed = 0;
        // The first call obtains the required size; usize storage provides descriptor alignment.
        unsafe { GetFileSecurityW(original.as_ptr(), info, std::ptr::null_mut(), 0, &mut needed); }
        if needed == 0 { return Err(std::io::Error::last_os_error()); }
        let mut descriptor = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
        let sd = descriptor.as_mut_ptr().cast();
        if unsafe { GetFileSecurityW(original.as_ptr(), info, sd, needed, &mut needed) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let attributes = SECURITY_ATTRIBUTES { nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd, bInheritHandle: 0 };
        let handle = unsafe { CreateFileW(destination.as_ptr(), GENERIC_WRITE, 0, &attributes,
            CREATE_NEW, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut()) };
        if handle == INVALID_HANDLE_VALUE { return Err(std::io::Error::last_os_error()); }
        return Ok(unsafe { std::fs::File::from_raw_handle(handle) });
    }
    #[cfg(not(windows))]
    let _ = source;
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)
}

/// Replace `path` with `bytes` atomically, preserving its existing security.
/// `attempts` / `base_delay_ms` bound the rename retry (on Windows the target
/// can be briefly held by another process; the delay doubles each attempt).
/// `attempts == 0` never renames — fault injection for tests.
pub fn replace(
    path: &Path,
    bytes: &[u8],
    attempts: u32,
    base_delay_ms: u64,
) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("quotabar-{}.tmp", std::process::id()));
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        let source = match std::fs::metadata(path) {
            Ok(_) => Some(path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let mut f = create_like(&tmp, source)?;
        created = true;
        if let Some(source) = source {
            f.set_permissions(std::fs::metadata(source)?.permissions())?;
        }
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        rename_with_retry(&tmp, path, attempts, base_delay_ms)
    })();
    // Only clean up a temp file we created ourselves: another writer may own
    // a file at this path.
    if result.is_err() && created {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn rename_with_retry(
    from: &Path,
    to: &Path,
    attempts: u32,
    base_delay_ms: u64,
) -> std::io::Result<()> {
    let mut delay = std::time::Duration::from_millis(base_delay_ms);
    let mut last: Option<std::io::Error> = None;
    for attempt in 0..attempts {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                if attempt + 1 < attempts {
                    std::thread::sleep(delay);
                    delay *= 2;
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("rename not attempted")))
}
