// DeskToken — credentials: auto-discovery of official CLI credential files +
// manual API keys via OS keyring + opencode's own credential file
// (multi-account reality: the same platform often has several keys on one
// machine — CLI/desktop + opencode + manual). Read-only by default.
use serde::Serialize;

pub const KEYRING_SERVICE: &str = "quotabar";
pub const LEGACY_KEYRING_SERVICE: &str = "desktoken";

/// One runnable instance of a provider = base + credential source.
/// Rows on the bar are per-instance (方案 B: all accounts on screen at once).
#[derive(Debug, Clone, Serialize)]
pub struct InstanceDesc {
    /// unique id, e.g. "codex" / "codex#opencode" / "glm#opencode"
    pub id: String,
    /// display name on the bar, e.g. "Codex · opencode"
    pub name: String,
    /// base provider id ("codex", "glm", ...)
    pub base: String,
}

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

/// Built-in providers with CLI credential auto-discovery. This table is the
/// ONE place a credential path is written: detect(), discover_instances() and
/// each provider module used to repeat them, so adding a platform meant three
/// edits and the copies drifted.
/// (id, display name, supports manual key, candidate paths under $HOME)
const BUILTINS: &[(&str, &str, bool, &[&str])] = &[
    ("claude", "Claude", true, &[".claude/.credentials.json"]),
    ("codex", "Codex", false, &[".codex/auth.json"]),
    ("gemini", "Gemini", false, &[".gemini/oauth_creds.json"]),
    (
        "kimi",
        "Kimi",
        true,
        &[
            ".kimi-code/credentials/kimi-code.json",
            ".kimi/credentials/kimi-code.json",
        ],
    ),
    ("glm", "GLM 智谱", true, &[]),
];

/// First existing CLI credential file for a built-in provider.
pub fn cli_cred_path(id: &str) -> Option<std::path::PathBuf> {
    let home = home()?;
    BUILTINS
        .iter()
        .find(|(builtin, ..)| *builtin == id)?
        .3
        .iter()
        .map(|rel| home.join(rel))
        .find(|p| p.exists())
}

/// Antigravity IDE stores the same Google OAuth credential the gemini CLI
/// would, under this Windows Credential Manager target.
pub const ANTIGRAVITY_TARGET: &str = "gemini:antigravity";

/// Is this provider's credential reachable outside its own CLI file?
fn foreign_source(id: &str) -> Option<&'static str> {
    (id == "gemini" && read_foreign_cred(ANTIGRAVITY_TARGET).is_some())
        .then_some("Antigravity IDE（凭据管理器）")
}

fn instance(id: &str, name: &str) -> InstanceDesc {
    InstanceDesc { id: id.into(), name: name.into(), base: id.into() }
}

pub fn detect() -> Vec<ProviderCredInfo> {
    BUILTINS
        .iter()
        .map(|&(id, name, supports_key, _)| {
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
            let found = cli_cred_path(id)
                .map(|p| p.display().to_string())
                .or_else(|| foreign_source(id).map(String::from));
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
        "kimi" => Some((
            "https://api.kimi.com/coding/v1/usages",
            "Authorization",
            "Bearer ",
        )),
        // GLM Coding Plan — raw key, no Bearer prefix (per Zhipu official plugin)
        "glm" => Some((
            "https://open.bigmodel.cn/api/monitor/usage/quota/limit",
            "Authorization",
            "",
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// opencode's own credential file — the second-account reality (方案 B)
// ---------------------------------------------------------------------------

/// A parsed entry from opencode's auth.json.
#[derive(Debug, Clone, PartialEq)]
pub enum OpencodeCred {
    /// ChatGPT OAuth (opencode "openai" entry): read-only for us — opencode
    /// owns the refresh; we use the access token while fresh.
    ChatGptOauth {
        access: String,
        expires_ms: i64,
        account_id: String,
    },
    /// Plain API key (kimi-for-coding / zhipuai-coding-plan / ...)
    ApiKey(String),
}

fn opencode_auth_path() -> Option<std::path::PathBuf> {
    let p = home()?.join(".local/share/opencode/auth.json");
    p.exists().then_some(p)
}

/// Pure parser: opencode auth.json → (provider-key, cred) pairs.
pub fn parse_opencode_auth(raw: &str) -> Vec<(String, OpencodeCred)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return vec![];
    };
    let Some(obj) = v.as_object() else {
        return vec![];
    };
    let mut out = vec![];
    for (k, e) in obj {
        let cred = if let Some(acc) = e.get("access").and_then(|x| x.as_str()) {
            let exp = e
                .get("expires")
                .and_then(crate::fetch::as_f64)
                .map(|x| x as i64)
                .unwrap_or(0);
            let acct = e
                .get("accountId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            Some(OpencodeCred::ChatGptOauth {
                access: acc.to_string(),
                expires_ms: exp,
                account_id: acct,
            })
        } else {
            e.get("key")
                .and_then(|x| x.as_str())
                .map(|s| OpencodeCred::ApiKey(s.to_string()))
        };
        if let Some(c) = cred {
            out.push((k.clone(), c));
        }
    }
    out
}

/// Best-effort read of the Codex CLI's account id (dedup vs opencode's).
fn codex_cli_account_id() -> Option<String> {
    let raw = std::fs::read_to_string(cli_cred_path("codex")?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    crate::fetch::json_path(&v, "tokens.account_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

/// Discover every runnable provider instance on this machine.
/// Order: primary (CLI/manual) first, opencode accounts after.
pub fn discover_instances() -> Vec<InstanceDesc> {
    let mut out: Vec<InstanceDesc> = vec![];
    if cli_cred_path("codex").is_some() {
        out.push(instance("codex", "Codex"));
    }
    if cli_cred_path("claude").is_some() {
        out.push(instance("claude", "Claude"));
    }
    if cli_cred_path("gemini").is_some() || foreign_source("gemini").is_some() {
        out.push(instance("gemini", "Gemini"));
    }
    if cli_cred_path("kimi").is_some() || keyring_get("kimi").is_some() {
        out.push(instance("kimi", "Kimi"));
    }
    if keyring_get("glm").is_some() {
        out.push(instance("glm", "GLM"));
    }

    // opencode accounts (dedup against the primary credential of the platform)
    if let Some(p) = opencode_auth_path() {
        if let Ok(raw) = std::fs::read_to_string(&p) {
            for (key, cred) in parse_opencode_auth(&raw) {
                match (key.as_str(), &cred) {
                    ("openai", OpencodeCred::ChatGptOauth { account_id, .. }) => {
                        let dup = codex_cli_account_id()
                            .map(|a| !a.is_empty() && a == *account_id)
                            .unwrap_or(false);
                        if !dup && !out.iter().any(|i| i.id == "codex#opencode") {
                            out.push(InstanceDesc {
                                id: "codex#opencode".into(),
                                name: "Codex · opencode".into(),
                                base: "codex".into(),
                            });
                        }
                    }
                    ("kimi-for-coding", OpencodeCred::ApiKey(k)) => {
                        let dup = keyring_get("kimi").map(|m| m == *k).unwrap_or(false);
                        if !dup {
                            out.push(InstanceDesc {
                                id: "kimi#opencode".into(),
                                name: "Kimi · opencode".into(),
                                base: "kimi".into(),
                            });
                        }
                    }
                    ("zhipuai-coding-plan", OpencodeCred::ApiKey(k)) => {
                        let dup = keyring_get("glm").map(|m| m == *k).unwrap_or(false);
                        if !dup {
                            out.push(InstanceDesc {
                                id: "glm#opencode".into(),
                                name: "GLM · opencode".into(),
                                base: "glm".into(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

/// Instance credential lookup used by providers.
pub fn opencode_cred(key: &str) -> Option<OpencodeCred> {
    let p = opencode_auth_path()?;
    let raw = std::fs::read_to_string(p).ok()?;
    parse_opencode_auth(&raw)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_opencode_auth_shapes() {
        let raw = r#"{
          "kimi-for-coding": { "type": "api", "key": "sk-kimi-xxx" },
          "zhipuai-coding-plan": { "type": "api", "key": "zpu-yyy" },
          "openai": { "type": "oauth", "refresh": "r", "access": "a",
                      "expires": 1786000000000, "accountId": "acc-1" },
          "deepseek": { "type": "api", "key": "sk-ds" }
        }"#;
        let entries = parse_opencode_auth(raw);
        assert_eq!(entries.len(), 4);
        assert!(entries
            .iter()
            .any(|(k, c)| k == "openai"
                && matches!(c, OpencodeCred::ChatGptOauth { account_id, expires_ms, .. }
                    if account_id == "acc-1" && *expires_ms == 1786000000000)));
        assert!(entries
            .iter()
            .any(|(k, c)| k == "kimi-for-coding" && matches!(c, OpencodeCred::ApiKey(s) if s == "sk-kimi-xxx")));
    }

    #[test]
    fn opencode_auth_garbage_is_empty() {
        assert!(parse_opencode_auth("not json").is_empty());
        assert!(parse_opencode_auth("[]").is_empty());
    }
}
