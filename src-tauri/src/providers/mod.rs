// DeskToken — provider framework: shared types, tolerant parsing, sanitization.
// See PLAN.md §4. All provider responses pass through sanitize() before emit.
pub mod claude;
pub mod codex;
pub mod custom;
pub mod gemini;
pub mod glm;
pub mod kimi;

use serde::Serialize;

/// epoch seconds (UTC)
pub type EpochSecs = i64;

#[derive(Debug, Clone, Serialize)]
pub struct QuotaWindow {
    pub label: String, // controlled vocab: "5h" / "周" / "今日" / "月" / "Pro 日" ...
    pub used_percent: f64,
    pub resets_at: Option<EpochSecs>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuotaSnapshot {
    pub v: u32, // event contract version
    pub provider_id: String,
    pub provider_name: String,
    pub plan: Option<String>,
    pub windows: Vec<QuotaWindow>,
    pub source: String, // "official" | "header_estimate" | "manual_key"
    pub fetched_at: EpochSecs,
    pub error: Option<String>, // Some => row renders in its error/stale state
}

impl QuotaSnapshot {
    pub fn ok(
        provider_id: &str,
        provider_name: &str,
        plan: Option<String>,
        windows: Vec<QuotaWindow>,
        source: &str,
    ) -> Self {
        Self {
            v: 1,
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            plan,
            windows,
            source: source.into(),
            fetched_at: now_secs(),
            error: None,
        }
    }

    pub fn err(provider_id: &str, provider_name: &str, e: &ProviderError) -> Self {
        Self {
            v: 1,
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            plan: None,
            windows: vec![],
            source: "official".into(),
            fetched_at: now_secs(),
            error: Some(e.short_msg().into()),
        }
    }
}

#[derive(Debug)]
pub enum ProviderError {
    /// Retry-After hint from the platform; poller currently uses its own
    /// exponential backoff, so the field is informational (diagnostics/tooltips).
    RateLimited {
        #[allow(dead_code)]
        retry_after: Option<u64>,
    },
    AuthExpired,
    CredentialMissing,
    CredentialCorrupt { torn: bool },
    ParseFailed,
    Network,
    UnsupportedClient,
    /// Antigravity IDE (language server) not running — Gemini via IDE channel
    IdeNotRunning,
}

impl ProviderError {
    /// in-row short message, <= 28 chars (design spec: error text mapping table)
    pub fn short_msg(&self) -> &'static str {
        match self {
            ProviderError::RateLimited { .. } => "限流中，稍后自动重试",
            ProviderError::AuthExpired => "凭据失效，请重新登录",
            ProviderError::CredentialMissing => "未配置凭据",
            ProviderError::CredentialCorrupt { torn: true } => "读取冲突，自动恢复中",
            ProviderError::CredentialCorrupt { torn: false } => "凭据损坏，请重新登录",
            ProviderError::ParseFailed => "接口变更，请检查更新",
            ProviderError::Network => "网络无法连接",
            ProviderError::UnsupportedClient => "暂不支持，可在设置中停用",
            ProviderError::IdeNotRunning => "Antigravity 未运行",
        }
    }
}

pub fn now_secs() -> EpochSecs {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Fetch one provider instance (方案 B): id is "codex" / "codex#opencode" /
/// custom def id. Stamp the snapshot with the instance id + display name so
/// rows, toasts and history are per-account.
pub async fn fetch_instance(
    id: &str,
    name: &str,
) -> Result<QuotaSnapshot, ProviderError> {
    // custom monitors: id is the def id ("cp-...")
    let s = crate::settings::load();
    if let Some(def) = s.custom_providers.iter().find(|d| d.id == id) {
        let mut snap = custom::fetch_snapshot(def).await?;
        snap.provider_id = id.to_string();
        snap.provider_name = name.to_string();
        return Ok(snap);
    }
    let (base, _suffix) = id.split_once('#').unwrap_or((id, ""));
    let mut snap = match base {
        "kimi" => kimi::fetch_instance(id).await?,
        "glm" => glm::fetch_instance(id).await?,
        "codex" => codex::fetch_instance(id).await?,
        "claude" => claude::fetch_snapshot().await?,
        "gemini" => gemini::fetch_snapshot().await?,
        _ => return Err(ProviderError::CredentialMissing),
    };
    snap.provider_id = id.to_string();
    snap.provider_name = name.to_string();
    Ok(snap)
}

/// Parse reset time: epoch seconds, epoch milliseconds, or RFC3339 string.
pub fn parse_reset(v: &serde_json::Value) -> Option<EpochSecs> {
    match v {
        serde_json::Value::Number(n) => {
            let x = n.as_i64()?;
            // > 1e12 => milliseconds; else seconds
            Some(if x > 1_000_000_000_000 { x / 1000 } else { x })
        }
        serde_json::Value::String(s) => {
            let s = s.trim();
            if let Ok(x) = s.parse::<i64>() {
                return Some(if x > 1_000_000_000_000 { x / 1000 } else { x });
            }
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp())
        }
        _ => None,
    }
}

/// Centralized response sanitization (PLAN.md eng review #6): every snapshot
/// passes through here before reaching the frontend. No provider may bypass.
pub fn sanitize(mut s: QuotaSnapshot) -> QuotaSnapshot {
    s.windows.truncate(8);
    for w in &mut s.windows {
        if !w.used_percent.is_finite() {
            w.used_percent = 0.0;
        }
        w.used_percent = w.used_percent.clamp(0.0, 100.0);
        w.label = w.label.chars().take(8).collect();
    }
    if let Some(p) = &s.plan {
        s.plan = Some(p.chars().take(64).collect());
    }
    s.provider_name = s.provider_name.chars().take(64).collect();
    if let Some(e) = &s.error {
        s.error = Some(e.chars().take(120).collect());
    }
    s
}
