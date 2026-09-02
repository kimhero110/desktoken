// QuotaBar — E6 diagnostics: unified redaction + copy-to-clipboard.
// Redaction lives here (not in providers' goodwill — eng review #13):
// Bearer/ya29./sk- token patterns, JSON credential field values, known
// keyring secrets (literal replacement), and %USERPROFILE% → ~.
use crate::credentials;

/// Redact secrets from an arbitrary string (log line or diagnostics bundle).
pub fn redact(input: &str) -> String {
    let mut out = input.to_string();

    // 1) literal secrets we hold (manual keys in keyring)
    let mut known: Vec<String> = vec![];
    for acct in ["glm", "kimi"] {
        if let Some(k) = credentials::keyring_get(acct) {
            if !k.is_empty() {
                known.push(k);
            }
        }
    }
    if let Ok(s) = std::panic::catch_unwind(|| crate::settings::load()) {
        for cp in s.custom_providers {
            if let Some(k) = credentials::keyring_get(&format!("custom/{}", cp.id)) {
                if !k.is_empty() {
                    known.push(k);
                }
            }
        }
    }
    for k in known {
        out = out.replace(&k, mask(&k).as_str());
    }

    // 2) pattern-based: OAuth/GitHub/API token shapes + JSON credential values
    let patterns = [
        (r#"Bearer\s+[A-Za-z0-9._\-]+"#, "Bearer ***"),
        (r#"ya29\.[A-Za-z0-9._\-]+"#, "ya29.***"),
        (r#"sk-[A-Za-z0-9._\-]{6,}"#, "sk-***"),
        (
            r#""(access_token|refresh_token|id_token|api_key|sessionKey)"\s*:\s*"[^"]*""#,
            r#""$1":"***""#,
        ),
    ];
    for (re, rep) in patterns {
        if let Ok(r) = regex::Regex::new(re) {
            out = r.replace_all(&out, rep).to_string();
        }
    }

    // 3) home dir → ~
    if let Some(h) = credentials::home() {
        let hs = h.to_string_lossy().to_string();
        out = out.replace(&hs, "~").replace(&hs.replace('\\', "\\\\"), "~");
    }
    out
}

fn mask(k: &str) -> String {
    // show first/last 3 chars only (design: key 只显示前后各 3 字符)
    if k.len() <= 8 {
        return "***".into();
    }
    format!("{}...{}", &k[..3], &k[k.len() - 3..])
}

/// Build the diagnostics bundle: versions, provider states, redacted log tail.
pub fn collect(provider_states: &[(String, String, Option<String>)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "QuotaBar v{} | Windows | built profile: {}\n",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) { "debug" } else { "release" }
    ));
    out.push_str("--- providers ---\n");
    for (id, name, err) in provider_states {
        out.push_str(&format!("{} ({}): {}\n", name, id, err.as_deref().unwrap_or("ok")));
    }
    out.push_str("--- log tail (redacted) ---\n");
    let log = std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into()))
        .join("quotabar")
        .join("spike.log");
    if let Ok(raw) = std::fs::read_to_string(&log) {
        let tail: String = raw
            .lines()
            .rev()
            .take(80)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&redact(&tail));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_token_patterns() {
        let s = r#"header: Bearer abc.def.ghi and google ya29.a0AfH6SMBxyz plus sk-1234567890abcdef {"access_token":"TOPSECRET123"}"#;
        let r = redact(s);
        assert!(!r.contains("abc.def.ghi"));
        assert!(!r.contains("ya29.a0AfH6SMBxyz"));
        assert!(!r.contains("sk-1234567890abcdef"));
        assert!(!r.contains("TOPSECRET123"));
        assert!(r.contains("***"));
    }

    #[test]
    fn leak_assertion_with_realistic_log() {
        // construct a log line containing a plausible real token; nothing may survive
        let token = "ya29.a0AfH6SMBfAKEfakeFAKE123456789";
        let line = format!("poller emit with Authorization: Bearer {} done", token);
        let r = redact(&line);
        assert!(!r.contains(token), "token leaked in diagnostics output");
    }
}
