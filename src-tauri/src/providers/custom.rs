// Custom provider engine (open framework): user-defined endpoint + JSON path
// mappings. See PLAN.md §4 "自定义 provider（零代码）".
use super::{parse_reset, ProviderError, QuotaSnapshot, QuotaWindow};
use crate::credentials;
use crate::fetch;
use crate::settings::CustomProvider;

pub async fn fetch_snapshot(def: &CustomProvider) -> Result<QuotaSnapshot, ProviderError> {
    let key = credentials::keyring_get(&format!("custom/{}", def.id)).unwrap_or_default();
    let (status, body) =
        fetch::get_with_auth(&def.endpoint, &def.auth_header, &def.auth_prefix, &key)
            .await
            .map_err(|_| ProviderError::Network)?;
    match status {
        200..=299 => {}
        401 | 403 => return Err(ProviderError::AuthExpired),
        429 => return Err(ProviderError::RateLimited { retry_after: None }),
        _ => return Err(ProviderError::Network),
    }
    parse(&def, &body)
}

pub fn parse(def: &CustomProvider, body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let mut windows = vec![];
    for m in &def.windows {
        let used = fetch::json_path(&v, &m.used_path).and_then(fetch::as_f64);
        let limit = fetch::json_path(&v, &m.limit_path).and_then(fetch::as_f64);
        let resets_at = m
            .reset_path
            .as_deref()
            .and_then(|p| fetch::json_path(&v, p))
            .and_then(parse_reset);
        if let (Some(used), Some(limit)) = (used, limit) {
            if limit > 0.0 {
                windows.push(QuotaWindow {
                    label: m.label.clone(),
                    used_percent: used / limit * 100.0,
                    resets_at,
                });
            }
        }
    }
    if windows.is_empty() {
        return Err(ProviderError::ParseFailed);
    }
    Ok(QuotaSnapshot::ok(
        &def.id,
        &def.name,
        None,
        windows,
        "manual_key",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::WindowMapping;

    fn def() -> CustomProvider {
        CustomProvider {
            id: "cp-test".into(),
            name: "TestProv".into(),
            endpoint: "https://example.com/quota".into(),
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            poll_minutes: 5,
            windows: vec![WindowMapping {
                label: "周".into(),
                used_path: "data.usage.used".into(),
                limit_path: "data.usage.limit".into(),
                reset_path: Some("data.usage.resetTime".into()),
            }],
        }
    }

    #[test]
    fn parses_via_json_paths() {
        let body = r#"{ "data": { "usage": { "used": "40", "limit": "100", "resetTime": 1786291200000 } } }"#;
        let s = parse(&def(), body).unwrap();
        assert_eq!(s.provider_name, "TestProv");
        assert_eq!(s.windows.len(), 1);
        assert!((s.windows[0].used_percent - 40.0).abs() < 0.01);
        assert_eq!(s.windows[0].resets_at, Some(1786291200));
    }

    #[test]
    fn missing_paths_is_parse_error() {
        let body = r#"{ "data": {} }"#;
        assert!(matches!(parse(&def(), body), Err(ProviderError::ParseFailed)));
    }
}
