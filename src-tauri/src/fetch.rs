// DeskToken — generic HTTP fetch + jq-lite JSON path extraction.
// Used by the custom-provider engine and key verification. HTTP spec per PLAN.md:
// connect 5s / total 15s hard timeout; 1MB response cap.
use serde_json::Value;

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
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
    let client = http_client()?;
    let mut req = client.get(endpoint).header("Accept", "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
    let truncated: String = body.chars().take(1_000_000).collect();
    Ok((status, truncated))
}

/// POST form-encoded body; returns (status, body truncated to 1MB).
pub async fn post_form(endpoint: &str, form: &[(&str, &str)]) -> Result<(u16, String), String> {
    let client = http_client()?;
    let resp = client
        .post(endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("读取响应失败: {}", e))?;
    Ok((status, body.chars().take(1_000_000).collect()))
}

/// Verify a custom provider definition: fetch + try the window mappings.
pub async fn verify_custom(
    def: &crate::settings::CustomProvider,
    key: &str,
) -> Result<String, String> {
    let (status, body) =
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
        let used = json_path(&json, &w.used_path).and_then(as_f64);
        let limit = json_path(&json, &w.limit_path).and_then(as_f64);
        match (used, limit) {
            (Some(u), Some(l)) => lines.push(format!("✓ [{}] used={} limit={}", w.label, u, l)),
            _ => lines.push(format!("✗ [{}] 路径未取到数值", w.label)),
        }
    }
    if def.windows.is_empty() {
        lines.push("（未配置窗口映射，仅验证连通性）".into());
    }
    Ok(lines.join("\n"))
}
