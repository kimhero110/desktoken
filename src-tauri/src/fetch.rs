// QuotaBar — generic HTTP fetch + jq-lite JSON path extraction.
// Used by the custom-provider engine and key verification. HTTP spec per PLAN.md:
// connect 5s / total 15s hard timeout; 1MB response cap; global client connection pool.
use serde_json::Value;
use std::sync::OnceLock;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// HTTP Retry-After accepts delta-seconds or an HTTP date (IMF-fixdate).
fn parse_retry_after(value: &str, now: std::time::SystemTime) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds);
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let now = now.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(date.timestamp().max(0) as u64).map(|t| t.saturating_sub(now))
}

/// Product UA carrying the real build version and host platform. The old
/// literal claimed 0.1.0 on Windows regardless of either.
fn default_user_agent() -> String {
    let os = if cfg!(windows) {
        "Windows NT"
    } else if cfg!(target_os = "macos") {
        "Macintosh"
    } else {
        std::env::consts::OS
    };
    format!(
        "QuotaBar/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        os,
        std::env::consts::ARCH
    )
}

/// Follow redirects only inside the origin the request was addressed to.
/// reqwest strips `Authorization` when a redirect crosses hosts, but not the
/// custom header names a user picks for their own monitor (`X-API-Key` and
/// friends) — those would otherwise be replayed to whatever host the endpoint
/// points at. Cross-origin hops stop and surface the 3xx to the caller.
fn same_origin_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let Some(origin) = attempt.previous().first() else {
            return attempt.stop();
        };
        if attempt.previous().len() >= 5 {
            return attempt.stop();
        }
        let next = attempt.url();
        let same = next.scheme() == origin.scheme()
            && next.host_str() == origin.host_str()
            && next.port_or_known_default() == origin.port_or_known_default();
        if same {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

pub fn http_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(default_user_agent())
            .redirect(same_origin_redirects())
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// A monitor endpoint carries the user's key on every poll, so it must not be
/// reachable over cleartext. https anywhere; http only against the loopback
/// interface, which keeps local mock servers (and these tests) usable.
pub fn check_endpoint(endpoint: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(endpoint.trim()).map_err(|_| "端点不是合法的 URL".to_string())?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host(url.host_str().unwrap_or("")) => Ok(()),
        "http" => Err("端点必须使用 https，否则 key 会以明文发送（本机 127.0.0.1 例外）".into()),
        other => Err(format!("不支持的协议: {}", other)),
    }
}

fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// jq-lite dotted path: "data.limits.0.percentage" (array index as numeric segment).
pub fn json_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        if let Ok(i) = seg.parse::<usize>() {
            cur = cur.get(i)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    Some(cur)
}

/// Coerce a JSON value that may be a string or number into f64 (tolerant parsing).
pub fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Resolve a number from a JSON path — or treat the "path" itself as a numeric
/// literal (balance-style APIs report no total, so users write their plan's
/// reference amount as the limit, e.g. `100`).
pub fn number_at(v: &Value, path: &str) -> Option<f64> {
    json_path(v, path)
        .and_then(as_f64)
        .or_else(|| path.trim().parse::<f64>().ok())
}

/// GET endpoint with auth header; returns (status, body truncated to 1MB, Retry-After seconds).
pub async fn get_with_auth(
    endpoint: &str,
    header: &str,
    prefix: &str,
    key: &str,
) -> Result<(u16, String, Option<u64>), String> {
    check_endpoint(endpoint)?;
    get_json(endpoint, &[(header, &format!("{}{}", prefix, key))]).await
}

/// GET endpoint with arbitrary headers; returns (status, body truncated to 1MB, Retry-After seconds).
pub async fn get_json(endpoint: &str, headers: &[(&str, &str)]) -> Result<(u16, String, Option<u64>), String> {
    get_json_via(http_client(), endpoint, headers).await
}

/// get_json against an explicit client — tests inject short-timeout clients
/// here so timeout behavior can be exercised without waiting out the 15s
/// production budget.
async fn get_json_via(
    client: &reqwest::Client,
    endpoint: &str,
    headers: &[(&str, &str)],
) -> Result<(u16, String, Option<u64>), String> {
    let mut req = client.get(endpoint).header("Accept", "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|h| parse_retry_after(h, std::time::SystemTime::now()));
    let text = read_capped_body(resp, 1024 * 1024).await?;
    Ok((status, text, retry_after))
}

/// POST form data; returns (status, body truncated to 1MB, Retry-After seconds).
pub async fn post_form(endpoint: &str, form: &[(&str, &str)]) -> Result<(u16, String, Option<u64>), String> {
    post_form_via(http_client(), endpoint, form).await
}

async fn post_form_via(
    client: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<(u16, String, Option<u64>), String> {
    let resp = client
        .post(endpoint)
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|h| parse_retry_after(h, std::time::SystemTime::now()));
    let text = read_capped_body(resp, 1024 * 1024).await?;
    Ok((status, text, retry_after))
}

/// POST JSON with Bearer auth and an optional User-Agent override
/// (Antigravity's daily-cloudcode-pa endpoint gates on it).
pub async fn post_json_ua(
    endpoint: &str,
    token: &str,
    user_agent: Option<&str>,
    body: &Value,
) -> Result<(u16, String, Option<u64>), String> {
    let client = http_client();
    let mut req = client
        .post(endpoint)
        .bearer_auth(token)
        .header("Accept", "application/json");
    if let Some(ua) = user_agent {
        req = req.header("User-Agent", ua);
    }
    let resp = req
        .json(body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|h| parse_retry_after(h, std::time::SystemTime::now()));
    let text = read_capped_body(resp, 1024 * 1024).await?;
    Ok((status, text, retry_after))
}

/// Read response body with a maximum byte limit (1MB default).
async fn read_capped_body(resp: reqwest::Response, max_bytes: usize) -> Result<String, String> {
    use futures_util::TryStreamExt;
    use tokio::io::AsyncReadExt;
    let mut stream = tokio_util::io::StreamReader::new(
        resp.bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
    );
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|e| format!("读取响应失败: {}", e))?;
        if n == 0 {
            break;
        }
        let take = n.min(max_bytes.saturating_sub(buf.len()));
        buf.extend_from_slice(&chunk[..take]);
        if buf.len() >= max_bytes {
            break;
        }
    }
    // lossy: a byte-boundary cut can split a multi-byte char; truncated JSON
    // fails tolerant parsing downstream either way
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Verify a custom provider definition: fetch + try the window mappings.
pub async fn verify_custom(
    def: &crate::settings::CustomProvider,
    key: &str,
) -> Result<String, String> {
    let (status, body, _) =
        get_with_auth(&def.endpoint, &def.auth_header, &def.auth_prefix, key).await?;
    if status == 401 || status == 403 {
        return Err(format!("HTTP {} — key 无效或无权限", status));
    }
    if status == 429 {
        return Err("HTTP 429 — 被限流，稍后再试".into());
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {}", status));
    }
    let json: Value =
        serde_json::from_str(&body).map_err(|e| format!("响应不是合法 JSON: {}", e))?;
    let mut lines = vec![];
    for w in &def.windows {
        let used = number_at(&json, &w.used_path);
        let limit = number_at(&json, &w.limit_path);
        match (used, limit) {
            (Some(u), Some(l)) => lines.push(format!(
                "✓ [{}] used={} limit={}{}",
                w.label,
                u,
                l,
                if w.invert { "（低水位）" } else { "" }
            )),
            _ => lines.push(format!("✗ [{}] 路径未取到数值", w.label)),
        }
    }
    if def.windows.is_empty() {
        lines.push("（未配置窗口映射，仅验证连通性）".into());
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn user_agent_carries_the_real_build_version() {
        let ua = default_user_agent();
        assert!(ua.contains(env!("CARGO_PKG_VERSION")), "stale UA: {ua}");
        assert!(!ua.contains("0.1.0 (Windows NT; x64)"), "hardcoded UA came back: {ua}");
    }

    #[test]
    fn cleartext_endpoints_are_refused_outside_loopback() {
        assert!(check_endpoint("https://api.example.com/quota").is_ok());
        assert!(check_endpoint("http://127.0.0.1:8080/quota").is_ok());
        assert!(check_endpoint("http://localhost:8080/quota").is_ok());
        assert!(check_endpoint("http://[::1]:8080/quota").is_ok());
        assert!(check_endpoint("http://api.example.com/quota").is_err());
        assert!(check_endpoint("http://10.0.0.5/quota").is_err());
        assert!(check_endpoint("file:///etc/passwd").is_err());
        assert!(check_endpoint("not a url").is_err());
    }

    #[test]
    fn retry_after_seconds_dates_and_invalid_values() {
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1445412420);
        assert_eq!(parse_retry_after("120", now), Some(120));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT", now), Some(60));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:26:00 GMT", now), Some(0));
        assert_eq!(parse_retry_after("invalid", now), None);
        assert_eq!(parse_retry_after("-1", now), None);
    }

    /// Short-timeout client so timeout tests don't burn the 15s production budget.
    fn fast_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(500))
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn auth_header_is_sent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/quota"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/quota", server.uri());
        let (status, _, _) = get_with_auth(&url, "Authorization", "Bearer ", "test-key")
            .await
            .unwrap();
        assert_eq!(status, 200);
        // expect(1) verified on drop
    }

    #[tokio::test]
    async fn status_passes_through_including_429() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/quota"))
            .respond_with(
                ResponseTemplate::new(429)
                    .append_header("retry-after", "30")
                    .set_body_string("{\"error\":\"slow down\"}"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/quota", server.uri());
        let (status, body, retry_after) = get_json(&url, &[]).await.unwrap();
        assert_eq!(status, 429);
        assert_eq!(retry_after, Some(30));
        assert!(body.contains("slow down"));
    }

    #[tokio::test]
    async fn body_capped_at_1mb() {
        let server = MockServer::start().await;
        let big = vec![b'x'; 2 * 1024 * 1024]; // 2MB > 1MB cap
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(big))
            .mount(&server)
            .await;
        let url = format!("{}/big", server.uri());
        let (status, body, _) = get_json(&url, &[]).await.unwrap();
        assert_eq!(status, 200);
        assert_eq!(body.len(), 1024 * 1024);
    }

    #[tokio::test]
    async fn slow_server_maps_to_network_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_secs(30))
                    .set_body_string("{}"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/slow", server.uri());
        let err = get_json_via(&fast_client(), &url, &[]).await.unwrap_err();
        assert!(err.contains("网络错误"), "unexpected error: {}", err);
    }

    #[tokio::test]
    async fn verify_custom_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/quota"))
            .and(header("x-api-key", "k-1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{ "data": { "usage": { "used": 40, "limit": 100 } } }"#,
            ))
            .mount(&server)
            .await;
        let def = crate::settings::CustomProvider {
            id: "cp-t".into(),
            name: "T".into(),
            endpoint: format!("{}/quota", server.uri()),
            auth_header: "x-api-key".into(),
            auth_prefix: "".into(),
            poll_minutes: 5,
            windows: vec![crate::settings::WindowMapping {
                label: "周".into(),
                used_path: "data.usage.used".into(),
                limit_path: "data.usage.limit".into(),
                reset_path: None,
                invert: false,
            }],
        };
        let report = verify_custom(&def, "k-1").await.unwrap();
        assert!(report.contains("✓ [周] used=40 limit=100"), "{}", report);
    }

    #[tokio::test]
    async fn verify_custom_429_message() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let def = crate::settings::CustomProvider {
            id: "cp-t".into(),
            name: "T".into(),
            endpoint: server.uri(),
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
            poll_minutes: 5,
            windows: vec![],
        };
        let err = verify_custom(&def, "k").await.unwrap_err();
        assert!(err.contains("429"), "{}", err);
    }
}
