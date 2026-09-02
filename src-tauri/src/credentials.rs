// DeskToken — credentials: auto-discovery of official CLI credential files +
// manual API keys via OS keyring. Read-only by default (see PLAN.md §3/§5).
use serde::Serialize;

pub const KEYRING_SERVICE: &str = "quotabar";
pub const LEGACY_KEYRING_SERVICE: &str = "desktoken";

#[derive(Debug, Clone, Serialize)]
pub struct ProviderCredInfo {
    pub id: String,
    pub name: String,
    /// "auto" = found CLI credential file (path in detail)
    /// "manual" = manual key stored in keyring
    /// "missing" = neither
    pub status: String,
    pub detail: String,
    pub supports_manual_key: bool,
}

pub fn home() -> Option<std::path::PathBuf> {
    std::env::var("USERPROFILE")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(dirs_home_fallback)
}

#[cfg(target_family = "unix")]
fn dirs_home_fallback() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}
#[cfg(not(target_family = "unix"))]
fn dirs_home_fallback() -> Option<std::path::PathBuf> {
    None
}

pub fn keyring_get(account: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, account)
        .ok()
        .and_then(|e| e.get_password().ok())
        .or_else(|| {
            keyring::Entry::new(LEGACY_KEYRING_SERVICE, account)
                .ok()
                .and_then(|e| e.get_password().ok())
        })
}

/// Read a credential owned by another application, by its exact Windows
/// Credential Manager target name (e.g. Antigravity's "gemini:antigravity").
/// READ-ONLY: we never write to foreign credentials (PLAN.md 凭据只读优先).
/// Raw CredReadW: foreign blobs may be UTF-8 bytes (Antigravity) rather than
/// the UTF-16 the keyring crate assumes — decode UTF-8 first, UTF-16 fallback.
#[cfg(target_os = "windows")]
pub fn read_foreign_cred(target: &str) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::Credentials::{CredFree, CredReadW, CRED_TYPE_GENERIC};
    let wide: Vec<u16> = std::ffi::OsStr::new(target)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let mut pcred: *mut windows_sys::Win32::Security::Credentials::CREDENTIALW =
            std::ptr::null_mut();
        if CredReadW(wide.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pcred) == 0 {
            return None;
        }
        let c = &*pcred;
        let blob = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
        let text = String::from_utf8(blob.to_vec()).ok().or_else(|| {
            let u16s: Vec<u16> = blob
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            String::from_utf16(&u16s).ok()
        });
        CredFree(pcred as *mut _);
        text.map(|s| s.trim_end_matches('\0').to_string())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn read_foreign_cred(_target: &str) -> Option<String> {
    None
}

pub fn keyring_set(account: &str, secret: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, account)
        .map_err(|e| e.to_string())?
        .set_password(secret)
        .map_err(|e| e.to_string())
}

pub fn keyring_delete(account: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, account)
        .map_err(|e| e.to_string())?
        .delete_credential()
        .map_err(|e| e.to_string())
}

/// Normalize a pasted key: trim whitespace, strip a leading "Bearer " prefix.
pub fn normalize_key(raw: &str) -> String {
    let t = raw.trim();
    t.strip_prefix("Bearer ").unwrap_or(t).trim().to_string()
}

/// Built-in providers with CLI credential auto-discovery.
/// (id, display name, supports manual key, candidate credential file paths)
fn builtin_specs() -> Vec<(&'static str, &'static str, bool, Vec<&'static str>)> {
    vec![
        ("claude", "Claude", true, vec![".claude/.credentials.json"]),
        ("codex", "Codex", false, vec![".codex/auth.json"]),
        ("gemini", "Gemini", false, vec![".gemini/oauth_creds.json"]),
        (
            "kimi",
            "Kimi",
            true,
            vec![
                ".kimi-code/credentials/kimi-code.json",
                ".kimi/credentials/kimi-code.json",
            ],
        ),
        ("glm", "GLM 智谱", true, vec![]),
    ]
}

pub fn detect() -> Vec<ProviderCredInfo> {
    let h = home();
    builtin_specs()
        .into_iter()
        .map(|(id, name, supports_key, paths)| {
            // manual key takes precedence display-wise
            if supports_key && keyring_get(id).is_some() {
                return ProviderCredInfo {
                    id: id.into(),
                    name: name.into(),
                    status: "manual".into(),
                    detail: "已保存手动 API key（凭据管理器）".into(),
                    supports_manual_key: supports_key,
                };
            }
            let found = paths.iter().find_map(|p| {
                h.as_ref().map(|hh| hh.join(p)).and_then(|full| {
                    if full.exists() {
                        Some(full.display().to_string())
                    } else {
                        None
                    }
                })
            });
            // Gemini fallback: Antigravity IDE stores the same Google OAuth
            // credential in Windows Credential Manager ("gemini:antigravity").
            let found = found.or_else(|| {
                if id == "gemini" && read_foreign_cred("gemini:antigravity").is_some() {
                    Some("Antigravity IDE（凭据管理器）".to_string())
                } else {
                    None
                }
            });
            match found {
                Some(p) => ProviderCredInfo {
                    id: id.into(),
                    name: name.into(),
                    status: "auto".into(),
                    detail: format!("已检测到 CLI 凭据: {}", p),
                    supports_manual_key: supports_key,
                },
                None => ProviderCredInfo {
                    id: id.into(),
                    name: name.into(),
                    status: "missing".into(),
                    detail: if supports_key {
                        "未检测到凭据，可手动添加 API key".into()
                    } else {
                        "未检测到 CLI 凭据（需安装并登录官方 CLI）".into()
                    },
                    supports_manual_key: supports_key,
                },
            }
        })
        .collect()
}

/// Verify targets for manual-key providers: endpoint, header name, prefix.
pub fn manual_key_target(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match id {
        // Kimi Code console API key — Bearer
        ("kimi") => Some((
            "https://api.kimi.com/coding/v1/usages",
            "Authorization",
            "Bearer ",
        )),
        // GLM Coding Plan — raw key, no Bearer prefix (per Zhipu official plugin)
        ("glm") => Some((
            "https://open.bigmodel.cn/api/monitor/usage/quota/limit",
            "Authorization",
            "",
        )),
        _ => None,
    }
}
