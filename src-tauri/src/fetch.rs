// QuotaBar — generic HTTP fetch + jq-lite JSON path extraction.
// Used by the custom-provider engine and key verification. HTTP spec per PLAN.md:
// connect 5s / total 15s hard timeout; 1MB response cap; global client connection pool.
use serde_json::Value;
use std::sync::OnceLock;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn http_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("QuotaBar/0.1.0 (Windows NT; x64)")
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
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

/// GET endpoint with auth header; returns (status, body truncated to 1MB).
pub async fn get_with_auth(
    endpoint: &str,
    header: &str,
    prefix: &str,
    key: &str,
) -> Result<(u16, String), String> {
    get_json(endpoint, &[(header, &format!("{}{}", prefix, key))]).await
}

/// GET endpoint with arbitrary headers; returns (status, body truncated to 1MB).
pub async fn get_json(endpoint: &str, headers: &[(&str, &str)]) -> Result<(u16, String), String> {
    let client = http_client();
    let mut req = client.get(endpoint).header("Accept", "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let text = read_capped_body(resp, 1024 * 1024).await?;
    Ok((status, text))
}

/// POST form data; returns (status, body truncated to 1MB).
pub async fn post_form(endpoint: &str, form: &[(&str, &str)]) -> Result<(u16, String), String> {
    let client = http_client();
    let resp = client
        .post(endpoint)
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let text = read_capped_body(resp, 1024 * 1024).await?;
    Ok((status, text))
}

/// Read response body with a maximum byte limit (1MB default).
async fn read_capped_body(resp: reqwest::Response, max_bytes: usize) -> Result<String, String> {
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
    String::from_utf8(buf).map_err(|e| format!("UTF-8 解码失败: {}", e))
}
