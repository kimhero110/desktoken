// Custom provider engine (open framework): user-defined endpoint + JSON path
// mappings. See PLAN.md §4 "自定义 provider（零代码）".
use super::{parse_reset, ProviderError, QuotaSnapshot, QuotaWindow};
use crate::credentials;
use crate::fetch;
use crate::settings::CustomProvider;

pub async fn fetch_snapshot(def: &CustomProvider) -> Result<QuotaSnapshot, ProviderError> {
    let key = credentials::keyring_get(&format!("custom/{}", def.id)).unwrap_or_default();
    let (status, body, retry_after) =
        fetch::get_with_auth(&def.endpoint, &def.auth_header, &def.auth_prefix, &key)
            .await
            .map_err(|_| ProviderError::Network)?;
    match status {
        200..=299 => {}
        401 | 403 => return Err(ProviderError::AuthExpired),
        429 => return Err(ProviderError::RateLimited { retry_after }),
        _ => return Err(ProviderError::Network),
    }
    parse(&def, &body)
}

pub fn parse(def: &CustomProvider, body: &str) -> Result<QuotaSnapshot, ProviderError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::ParseFailed)?;
    let mut windows = vec![];
    for m in &def.windows {
        let used = fetch::number_at(&v, &m.used_path);
        let limit = fetch::number_at(&v, &m.limit_path);
        let resets_at = m
            .reset_path
            .as_deref()
            .and_then(|p| fetch::json_path(&v, p))
            .and_then(parse_reset);
        if let (Some(used), Some(limit)) = (used, limit) {
            if limit > 0.0 {
                let mut pct = used / limit * 100.0;
                if m.invert {
                    // low-watermark: remaining/ratio inverted so a draining
                    // balance reads as a draining quota (bar red, toasts fire)
                    pct = 100.0 - pct;
                }
                windows.push(QuotaWindow {
                    label: m.label.clone(),
                    used_percent: pct,
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
                invert: false,
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

    #[test]
    fn literal_limit_for_balance_apis() {
        // balance endpoints report no total: user writes their reference
        // recharge amount as a numeric-literal limit
        let mut d = def();
        d.windows[0].used_path = "data.available_balance".into();
        d.windows[0].limit_path = "100".into();
        d.windows[0].reset_path = None;
        let body = r#"{ "data": { "available_balance": 49.59 } }"#;
        let s = parse(&d, body).unwrap();
        assert!((s.windows[0].used_percent - 49.59).abs() < 0.01);
    }

    #[test]
    fn invert_low_watermark_mode() {
        // Moonshot 余额 template: draining balance reads as draining quota —
        // $49.59 left of a $100 reference → 50.41% "used"
        let mut d = def();
        d.windows[0].label = "余额消耗".into();
        d.windows[0].used_path = "data.available_balance".into();
        d.windows[0].limit_path = "100".into();
        d.windows[0].reset_path = None;
        d.windows[0].invert = true;
        let body = r#"{ "code":0, "data": { "available_balance": 49.59, "voucher_balance": 46.59, "cash_balance": 3.0 } }"#;
        let s = parse(&d, body).unwrap();
        assert!((s.windows[0].used_percent - 50.41).abs() < 0.01);
        // nearly empty balance → >90% → red + toast territory
        let body2 = r#"{ "data": { "available_balance": 5.0 } }"#;
        let s2 = parse(&d, body2).unwrap();
        assert!(s2.windows[0].used_percent >= 90.0);
    }

    // ---- HTTP contract tests (wiremock): endpoint injected via def.endpoint ----

    mod http {
        use super::*;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        fn mock_def(server: &MockServer) -> CustomProvider {
            CustomProvider {
                endpoint: format!("{}/quota", server.uri()),
                ..def()
            }
        }

        #[tokio::test]
        async fn happy_path_over_http() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/quota"))
                .respond_with(ResponseTemplate::new(200).set_body_string(
                    r#"{ "data": { "usage": { "used": 40, "limit": 100, "resetTime": 1786291200000 } } }"#,
                ))
                .expect(1)
                .mount(&server)
                .await;
            // keyring miss in test env → empty key; mock does not gate on it
            let s = fetch_snapshot(&mock_def(&server)).await.unwrap();
            assert_eq!(s.windows.len(), 1);
            assert!((s.windows[0].used_percent - 40.0).abs() < 0.01);
            assert_eq!(s.windows[0].resets_at, Some(1786291200));
        }

        #[tokio::test]
        async fn http_429_maps_to_rate_limited() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(429).append_header("retry-after", "15"))
                .mount(&server)
                .await;
            let r = fetch_snapshot(&mock_def(&server)).await;
            assert!(matches!(r, Err(ProviderError::RateLimited { .. })));
        }

        #[tokio::test]
        async fn http_401_maps_to_auth_expired() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&server)
                .await;
            let r = fetch_snapshot(&mock_def(&server)).await;
            assert!(matches!(r, Err(ProviderError::AuthExpired)));
        }

        #[tokio::test]
        async fn http_500_maps_to_network() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&server)
                .await;
            let r = fetch_snapshot(&mock_def(&server)).await;
            assert!(matches!(r, Err(ProviderError::Network)));
        }
    }
}
