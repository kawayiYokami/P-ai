const FETCH_BROWSER_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36";
const EXA_MCP_URL: &str = "https://mcp.exa.ai/mcp";
const EXA_ACCEPT_HEADER: &str = "application/json, text/event-stream";
const EXA_TOOL_NAME_WEB_FETCH: &str = "web_fetch_exa";

fn is_forbidden_fetch_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.octets()[0] == 0
        }
        std::net::IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }
            let seg0 = v6.segments()[0];
            if (seg0 & 0xfe00) == 0xfc00 {
                return true;
            }
            if (seg0 & 0xffc0) == 0xfe80 {
                return true;
            }
            if let Some(mapped) = v6.to_ipv4() {
                return is_forbidden_fetch_ip(std::net::IpAddr::V4(mapped));
            }
            false
        }
    }
}

struct ValidatedFetchTarget {
    url: reqwest::Url,
    resolve_host: Option<String>,
    resolved_addrs: Vec<std::net::SocketAddr>,
}

async fn validate_builtin_fetch_url(raw: &str) -> Result<ValidatedFetchTarget, String> {
    let parsed = reqwest::Url::parse(raw).map_err(|err| format!("Invalid fetch url: {err}"))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("Only http/https URLs are allowed.".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Fetch url must include a host.".to_string())?;
    let host_text = host.to_string();
    let host_lower = host_text.trim().to_ascii_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Err("Fetch url host is blocked: localhost.".to_string());
    }
    let port = parsed
        .port_or_known_default()
        .unwrap_or(if scheme == "https" { 443 } else { 80 });
    if let Ok(ip) = host_text.parse::<std::net::IpAddr>() {
        if is_forbidden_fetch_ip(ip) {
            return Err("Fetch url host resolves to a blocked local/private address.".to_string());
        }
        return Ok(ValidatedFetchTarget {
            url: parsed,
            resolve_host: None,
            resolved_addrs: vec![std::net::SocketAddr::new(ip, port)],
        });
    }
    let resolved = tokio::net::lookup_host((host_text.as_str(), port))
        .await
        .map_err(|err| format!("Resolve host failed: {err}"))?;
    let mut addrs = Vec::<std::net::SocketAddr>::new();
    for addr in resolved {
        if is_forbidden_fetch_ip(addr.ip()) {
            return Err(
                "Fetch url host resolves to a blocked loopback/link-local/private address."
                    .to_string(),
            );
        }
        if !addrs.contains(&addr) {
            addrs.push(addr);
        }
    }
    if addrs.is_empty() {
        return Err("Fetch url host has no resolved IP addresses.".to_string());
    }
    Ok(ValidatedFetchTarget {
        url: parsed,
        resolve_host: Some(host_text),
        resolved_addrs: addrs,
    })
}

fn build_fetch_client(
    state: &AppState,
    validated: &ValidatedFetchTarget,
) -> Result<reqwest::Client, String> {
    if validated.resolve_host.is_none() {
        return Ok(state.shared_http_client.clone());
    }
    let mut client_builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .connect_timeout(std::time::Duration::from_secs(8))
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::limited(10));
    if let Some(host) = validated.resolve_host.as_deref() {
        for addr in &validated.resolved_addrs {
            client_builder = client_builder.resolve(host, *addr);
        }
    }
    client_builder
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))
}

fn exa_result_texts(result: &Value) -> Vec<String> {
    result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = item.get("text").and_then(Value::as_str)?.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        })
        .collect()
}

fn exa_result_has_content(result: &Value) -> bool {
    !exa_result_texts(result).is_empty()
}

fn parse_exa_sse_result(body: &str) -> Result<Value, String> {
    let mut data_lines = Vec::<String>::new();
    for line in body.lines() {
        let trimmed = line.trim_end();
        if let Some(rest) = trimmed.strip_prefix("data:") {
            let payload = rest.trim();
            if !payload.is_empty() {
                data_lines.push(payload.to_string());
            }
        }
    }
    if data_lines.is_empty() {
        return Err("Exa SSE response missing data payload.".to_string());
    }
    let mut last_error: Option<String> = None;
    for payload in data_lines {
        let value: Value = match serde_json::from_str(&payload) {
            Ok(value) => value,
            Err(err) => {
                last_error = Some(format!("Parse Exa SSE JSON failed: {err}"));
                continue;
            }
        };
        if let Some(error) = value.get("error") {
            return Err(format!("Exa MCP returned error: {error}"));
        }
        if let Some(result) = value.get("result") {
            return Ok(result.clone());
        }
    }
    Err(last_error.unwrap_or_else(|| "Exa SSE response missing JSON-RPC result.".to_string()))
}

async fn call_exa_mcp_tool(
    state: &AppState,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, String> {
    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments,
        }
    });
    let response = state
        .shared_http_client
        .post(EXA_MCP_URL)
        .header("Content-Type", "application/json")
        .header("Accept", EXA_ACCEPT_HEADER)
        .json(&request_body)
        .send()
        .await
        .map_err(|err| format!("Call Exa MCP failed: {err}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Read Exa MCP response failed: {err}"))?;
    if !status.is_success() {
        return Err(format!("Exa MCP returned status {status}: {}", truncate_by_chars(&clean_text(&body), 240)));
    }
    parse_exa_sse_result(&body)
}

fn map_exa_fetch_result(max_length: usize, result: &Value) -> Result<Value, String> {
    let texts = exa_result_texts(result);
    if texts.is_empty() {
        return Err("Exa fetch returned empty content.".to_string());
    }
    let combined = texts.join("\n\n");
    Ok(serde_json::json!({
      "status": 200,
      "content": truncate_by_chars(&combined, max_length)
    }))
}

async fn builtin_fetch_fallback(state: &AppState, url: &str, max_length: usize) -> Result<Value, String> {
    let normalized_url = url.trim();
    if normalized_url.is_empty() {
        return Ok(serde_json::json!({
          "ok": false,
          "url": "",
          "status": Value::Null,
          "error": "empty_url",
          "message": "Fetch url is empty.",
          "content": "",
          "provider": "builtin"
        }));
    }

    let validated = match validate_builtin_fetch_url(normalized_url).await {
        Ok(target) => target,
        Err(message) => {
            return Ok(serde_json::json!({
              "ok": false,
              "url": normalized_url,
              "status": Value::Null,
              "error": "invalid_url",
              "message": message,
              "content": "",
              "provider": "builtin"
            }));
        }
    };

    let client = match build_fetch_client(state, &validated) {
        Ok(client) => client,
        Err(err) => {
            return Ok(serde_json::json!({
              "ok": false,
              "url": normalized_url,
              "status": Value::Null,
              "error": "build_http_client_failed",
              "message": err,
              "content": "",
              "provider": "builtin"
            }));
        }
    };
    let resp = client
        .get(validated.url)
        .header("User-Agent", FETCH_BROWSER_UA)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        )
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Cache-Control", "no-cache")
        .header("Pragma", "no-cache")
        .header("Upgrade-Insecure-Requests", "1")
        .send()
        .await;
    let resp = match resp {
        Ok(resp) => resp,
        Err(err) => {
        return Ok(serde_json::json!({
          "ok": false,
          "url": normalized_url,
          "status": Value::Null,
          "error": "request_failed",
          "message": format!("Fetch url failed: {}", err),
          "content": "",
          "provider": "builtin"
        }));
        }
    };
    let status = resp.status();
    let html = resp
        .text()
        .await
        .unwrap_or_default();
    if !status.is_success() {
        let fallback_content = truncate_by_chars(&clean_text(&html), max_length);
        return Ok(serde_json::json!({
          "ok": false,
          "url": normalized_url,
          "status": status.as_u16(),
          "error": "http_status_not_success",
          "message": format!("Fetch url failed with status {status}"),
          "content": fallback_content,
          "provider": "builtin"
        }));
    }
    let extracted = rs_trafilatura::extract_with_options(
        &html,
        &rs_trafilatura::Options {
            url: Some(normalized_url.to_string()),
            output_markdown: false,
            include_tables: true,
            include_images: false,
            include_links: false,
            include_comments: false,
            favor_precision: true,
            favor_recall: false,
            deduplicate: true,
            ..rs_trafilatura::Options::default()
        },
    )
    .ok()
    .map(|result| result.content_text)
    .filter(|content| !content.trim().is_empty());
    let cleaned = match extracted {
        Some(content) => clean_text(&content),
        None => {
            let document = Html::parse_document(&html);
            let body_selector = Selector::parse("body");
            let raw = if let Ok(selector) = body_selector {
                document
                    .select(&selector)
                    .next()
                    .map(|n| n.text().collect::<Vec<_>>().join(" "))
                    .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "))
            } else {
                document.root_element().text().collect::<Vec<_>>().join(" ")
            };
            clean_text(&raw)
        }
    };
    let truncated = truncate_by_chars(&cleaned, max_length);
    Ok(serde_json::json!({
      "ok": true,
      "url": normalized_url,
      "status": status.as_u16(),
      "content": truncated,
      "provider": "builtin"
    }))
}

async fn builtin_fetch(state: &AppState, url: &str, max_length: usize) -> Result<Value, String> {
    match call_exa_mcp_tool(
        state,
        EXA_TOOL_NAME_WEB_FETCH,
        serde_json::json!({
            "urls": [url.trim()],
            "maxCharacters": max_length,
        }),
    )
    .await
    {
        Ok(result) if exa_result_has_content(&result) => map_exa_fetch_result(max_length, &result),
        Ok(_) | Err(_) => builtin_fetch_fallback(state, url, max_length).await,
    }
}

// ========== bing search ==========

fn decode_b64_relaxed(input: &str) -> Option<String> {
    let mut candidates = Vec::new();
    candidates.push(input.trim().to_string());
    candidates.push(input.trim().replace('-', "+").replace('_', "/"));
    for mut candidate in candidates {
        let rem = candidate.len() % 4;
        if rem != 0 {
            candidate.push_str(&"=".repeat(4 - rem));
        }
        if let Ok(bytes) = B64.decode(candidate.as_bytes()) {
            if let Ok(text) = String::from_utf8(bytes) {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }
    None
}

fn normalize_bing_result_url(raw: &str) -> String {
    let input = raw.trim();
    if input.is_empty() {
        return String::new();
    }
    let Ok(parsed) = reqwest::Url::parse(input) else {
        return input.to_string();
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    if !host.ends_with("bing.com") || !path.starts_with("/ck/") {
        return input.to_string();
    }

    for (k, v) in parsed.query_pairs() {
        let key = k.as_ref();
        let value = v.as_ref().trim();
        if value.is_empty() {
            continue;
        }
        if key == "url" && (value.starts_with("http://") || value.starts_with("https://")) {
            return value.to_string();
        }
        if key == "u" {
            let decoded_url = urlencoding::decode(value)
                .map(|x| x.into_owned())
                .unwrap_or_else(|_| value.to_string());
            if decoded_url.starts_with("http://") || decoded_url.starts_with("https://") {
                return decoded_url;
            }
            let b64_payload = decoded_url.strip_prefix("a1").unwrap_or(decoded_url.as_str());
            if let Some(text) = decode_b64_relaxed(b64_payload) {
                if text.starts_with("http://") || text.starts_with("https://") {
                    return text;
                }
            }
        }
    }
    input.to_string()
}

fn canonical_url_key(raw: &str) -> String {
    let normalized = normalize_bing_result_url(raw);
    if normalized.is_empty() {
        return String::new();
    }
    let mut key = normalized.trim().trim_end_matches('/').to_ascii_lowercase();
    if let Some(stripped) = key.strip_prefix("https://") {
        key = stripped.to_string();
    } else if let Some(stripped) = key.strip_prefix("http://") {
        key = stripped.to_string();
    }
    key
}

// ========== bing 会话 cookie ==========

const BING_HOME_URL: &str = "https://www.bing.com/";
/// 浏览器 UA 会拿到 JS 渲染的壳页（正文里没有 li.b_algo），经典服务端渲染结果页只有非浏览器 UA 才拿得到。
const BING_SEARCH_UA: &str = "Mozilla/5.0";
/// Bing 按会话 cookie 判定市场；没有 cookie 时按出口 IP 判，非中国 IP 会被判到别的市场。
/// cookie 是续期不是重建：到期时带着旧 cookie 去访问首页、把 Set-Cookie 合并回表里，SID 才能一直续下去。
const BING_COOKIE_TTL_SECONDS: u64 = 24 * 3600;
/// cookie 刷新与搜索请求都要能超时退出，别让一次卡死的请求拖住整条兜底。
const BING_REQUEST_TIMEOUT_SECONDS: u64 = 15;

/// 一条 bing cookie：值 + 可选过期时刻（来自 Set-Cookie 的 Max-Age 或 Expires）。
#[derive(Clone, Debug, PartialEq, Eq)]
struct BingCookie {
    value: String,
    expires_at: Option<std::time::SystemTime>,
}

/// 进程级 cookie 表：name → cookie，外加最近一次刷新时刻。
#[derive(Debug)]
struct BingCookieJar {
    cookies: std::collections::BTreeMap<String, BingCookie>,
    refreshed_at: std::time::Instant,
}

static BING_COOKIE_JAR: once_cell::sync::Lazy<std::sync::Mutex<Option<BingCookieJar>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

/// 公历日期 → 1970-01-01 起的天数（Howard Hinnant 的 days_from_civil）。
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 {
        month as i64 - 3
    } else {
        month as i64 + 9
    };
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn cookie_month_index(raw: &str) -> Option<u32> {
    let index = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ]
    .iter()
    .position(|month| month.eq_ignore_ascii_case(raw))?;
    Some(index as u32 + 1)
}

/// 解析 Set-Cookie 里的日期，形如 `Wed, 05 Nov 2026 00:00:00 GMT`。
fn parse_cookie_datetime(raw: &str) -> Option<std::time::SystemTime> {
    let parts = raw.split_whitespace().collect::<Vec<_>>();
    let mut index = 0usize;
    if parts.first().is_some_and(|first| first.ends_with(',')) {
        index = 1;
    }
    let day: u32 = parts.get(index)?.parse().ok()?;
    let month = cookie_month_index(parts.get(index + 1)?)?;
    let year: i64 = parts.get(index + 2)?.parse().ok()?;
    let mut clock = parts.get(index + 3)?.split(':');
    let hour: u64 = clock.next()?.parse().ok()?;
    let minute: u64 = clock.next()?.parse().ok()?;
    let second: u64 = clock.next().unwrap_or("0").parse().ok()?;
    let seconds =
        days_from_civil(year, month, day) * 86400 + (hour * 3600 + minute * 60 + second) as i64;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds.max(0) as u64))
}

struct ParsedSetCookie {
    name: String,
    value: String,
    expires_at: Option<std::time::SystemTime>,
}

fn parse_set_cookie(raw: &str, now: std::time::SystemTime) -> Option<ParsedSetCookie> {
    let mut segments = raw.split(';');
    let (name, value) = segments.next()?.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut expires_at = None;
    for attribute in segments {
        let Some((key, attribute_value)) = attribute.trim().split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let attribute_value = attribute_value.trim();
        match key.as_str() {
            "max-age" => {
                if let Ok(seconds) = attribute_value.parse::<i64>() {
                    expires_at = Some(if seconds <= 0 {
                        now
                    } else {
                        now + std::time::Duration::from_secs(seconds as u64)
                    });
                }
            }
            "expires" => {
                if let Some(deadline) = parse_cookie_datetime(attribute_value) {
                    expires_at = Some(deadline);
                }
            }
            _ => {}
        }
    }
    Some(ParsedSetCookie {
        name: name.to_string(),
        value: value.trim().to_string(),
        expires_at,
    })
}

/// 合并 Set-Cookie：同名覆盖，空值或已过期删除，响应里没出现的键保留。
fn merge_bing_set_cookies(
    cookies: &mut std::collections::BTreeMap<String, BingCookie>,
    set_cookies: &[String],
    now: std::time::SystemTime,
) {
    for raw in set_cookies {
        let Some(parsed) = parse_set_cookie(raw, now) else {
            continue;
        };
        let expired = parsed.expires_at.is_some_and(|at| at <= now);
        if parsed.value.is_empty() || expired {
            cookies.remove(&parsed.name);
            continue;
        }
        cookies.insert(
            parsed.name,
            BingCookie {
                value: parsed.value,
                expires_at: parsed.expires_at,
            },
        );
    }
}

fn prune_expired_bing_cookies(
    cookies: &mut std::collections::BTreeMap<String, BingCookie>,
    now: std::time::SystemTime,
) {
    cookies.retain(|_, cookie| cookie.expires_at.is_none_or(|at| at > now));
}

fn bing_cookie_header(cookies: &std::collections::BTreeMap<String, BingCookie>) -> String {
    cookies
        .iter()
        .map(|(name, cookie)| format!("{name}={}", cookie.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 当前 cookie 表拼成的请求头；键按名字排序，输出稳定。
fn current_bing_cookie_header() -> String {
    let guard = BING_COOKIE_JAR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(jar) = guard.as_ref() else {
        return String::new();
    };
    let mut cookies = jar.cookies.clone();
    prune_expired_bing_cookies(&mut cookies, std::time::SystemTime::now());
    bing_cookie_header(&cookies)
}

fn bing_cookie_jar_is_stale(jar: Option<&BingCookieJar>, now: std::time::Instant) -> bool {
    match jar {
        None => true,
        Some(jar) => now.duration_since(jar.refreshed_at).as_secs() >= BING_COOKIE_TTL_SECONDS,
    }
}

fn bing_cookie_should_refresh(now: std::time::Instant) -> bool {
    let guard = BING_COOKIE_JAR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    bing_cookie_jar_is_stale(guard.as_ref(), now)
}

/// 把一批 Set-Cookie 合并进表并返回当前请求头；`refreshed` 表示这次是刷新首页拿到的。
fn apply_bing_response_cookies(set_cookies: &[String], refreshed: bool) -> String {
    let mut guard = BING_COOKIE_JAR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now_system = std::time::SystemTime::now();
    if guard.is_none() {
        // 还一次都没拿到过 cookie：只有这次真拿到了才建表，空表不缓存，下次继续试。
        let mut cookies = std::collections::BTreeMap::new();
        merge_bing_set_cookies(&mut cookies, set_cookies, now_system);
        prune_expired_bing_cookies(&mut cookies, now_system);
        if cookies.is_empty() {
            return String::new();
        }
        let header = bing_cookie_header(&cookies);
        *guard = Some(BingCookieJar {
            cookies,
            refreshed_at: std::time::Instant::now(),
        });
        return header;
    }
    if let Some(jar) = guard.as_mut() {
        merge_bing_set_cookies(&mut jar.cookies, set_cookies, now_system);
        prune_expired_bing_cookies(&mut jar.cookies, now_system);
        if refreshed {
            jar.refreshed_at = std::time::Instant::now();
        }
        return bing_cookie_header(&jar.cookies);
    }
    String::new()
}

fn collect_set_cookie_headers(headers: &reqwest::header::HeaderMap) -> Vec<String> {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(str::to_string)
        .collect()
}

/// 带着当前 cookie 去访问首页续期；失败时不写表，返回当前（可能为空的）请求头。
async fn refresh_bing_cookies(client: &reqwest::Client) -> String {
    let existing = current_bing_cookie_header();
    let mut request = client
        .get(BING_HOME_URL)
        .header("User-Agent", BING_SEARCH_UA)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");
    if !existing.is_empty() {
        request = request.header("Cookie", &existing);
    }
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(BING_REQUEST_TIMEOUT_SECONDS),
        request.send(),
    )
    .await;
    let response = match response {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => {
            runtime_log_warn(format!("[搜索引擎] 刷新 Bing 会话 cookie 失败，沿用现有 cookie：{err}"));
            return existing;
        }
        Err(_) => {
            runtime_log_warn("[搜索引擎] 刷新 Bing 会话 cookie 超时，沿用现有 cookie".to_string());
            return existing;
        }
    };
    if !response.status().is_success() {
        runtime_log_warn(format!(
            "[搜索引擎] 刷新 Bing 会话 cookie 返回状态 {}，沿用现有 cookie",
            response.status()
        ));
        return existing;
    }
    let set_cookies = collect_set_cookie_headers(response.headers());
    let header = apply_bing_response_cookies(&set_cookies, true);
    if header.is_empty() {
        runtime_log_warn("[搜索引擎] Bing 首页没有下发可用 cookie，本次不带 cookie".to_string());
    }
    header
}

async fn ensure_bing_cookies(client: &reqwest::Client) -> String {
    if bing_cookie_should_refresh(std::time::Instant::now()) {
        return refresh_bing_cookies(client).await;
    }
    current_bing_cookie_header()
}

fn bing_request(client: &reqwest::Client, cookie: &str) -> reqwest::RequestBuilder {
    let request = client
        .get(BING_HOME_URL)
        .header("User-Agent", BING_SEARCH_UA)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");
    if cookie.is_empty() {
        request
    } else {
        request.header("Cookie", cookie)
    }
}

// ========== bing 市场判定 ==========

/// 取出 `<html lang="...">` 的值。
fn html_lang_value(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<html")?;
    let end = lower[start..].find('>').map(|offset| start + offset)?;
    let head = &lower[start..end];
    let lang_at = head.find("lang=")?;
    let rest = &head[lang_at + 5..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let body = &rest[1..];
        let close = body.find(quote)?;
        return Some(body[..close].to_string());
    }
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == '>')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// 市场是不是中文：lang 读不到时无法判断，按通过处理，不要因为读不到属性就反复重试。
fn bing_market_is_chinese(html: &str) -> bool {
    match html_lang_value(html) {
        Some(lang) => lang.to_ascii_lowercase().starts_with("zh"),
        None => true,
    }
}

// ========== bing 结果相关性 ==========

/// 检索词里没有实义的虚词与口语连接词。
const SEARCH_QUERY_STOPWORDS: &[&str] = &[
    "的", "了", "是", "在", "和", "与", "及", "或", "也", "就", "都", "还", "又", "把", "被", "吗",
    "呢", "吧", "啊", "什么", "怎么", "怎样", "如何", "为什么", "哪个", "哪些", "是否", "以及",
    "还有", "这个", "那个", "一个", "可以", "请问", "一下", "里面", "然后", "到底",
];

/// 按空白与标点切词；连续字母数字或汉字算一个词。
fn split_query_tokens(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn push_unique_keyword(keywords: &mut Vec<String>, candidate: String) {
    if candidate.is_empty() {
        return;
    }
    if !keywords.iter().any(|existing| existing == &candidate) {
        keywords.push(candidate);
    }
}

/// 从 query 提取判断相关性的词元：长度 1 的字与虚词丢掉，中文串额外切成连续二字、整串也算候选。
fn search_query_keywords(query: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    for token in split_query_tokens(query) {
        let lower = token.to_lowercase();
        let chars = lower.chars().collect::<Vec<_>>();
        let is_stopword = SEARCH_QUERY_STOPWORDS.contains(&lower.as_str());
        if chars.len() >= 2 && !is_stopword {
            push_unique_keyword(&mut keywords, lower.clone());
        }
        // is_cjk_char 是 crate 级公共辅助（pdf_text_service.rs 里的同名函数），这里直接复用。
        if chars.iter().any(|ch| is_cjk_char(*ch)) {
            for window in chars.windows(2) {
                let gram = window.iter().collect::<String>();
                if SEARCH_QUERY_STOPWORDS.contains(&gram.as_str()) {
                    continue;
                }
                push_unique_keyword(&mut keywords, gram);
            }
        }
    }
    keywords
}

fn bing_result_matches_keywords(keywords: &[String], row: &Value) -> bool {
    let mut haystack = String::new();
    for field in ["title", "snippet", "url"] {
        if let Some(text) = row.get(field).and_then(Value::as_str) {
            haystack.push_str(text);
            haystack.push(' ');
        }
    }
    let haystack = haystack.to_lowercase();
    keywords
        .iter()
        .any(|keyword| haystack.contains(keyword.as_str()))
}

/// 丢掉与查询毫无交集的结果；提取不出词元时无法判断，原样保留。
fn filter_relevant_bing_results(query: &str, rows: Vec<Value>) -> Vec<Value> {
    let keywords = search_query_keywords(query);
    if keywords.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| bing_result_matches_keywords(&keywords, row))
        .collect()
}

// ========== bing 兜底入口 ==========

async fn fetch_bing_search_html(
    client: &reqwest::Client,
    url: &str,
    cookie: &str,
) -> Result<String, String> {
    let mut builder = client
        .get(url)
        .header("User-Agent", BING_SEARCH_UA)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8");
    if !cookie.is_empty() {
        builder = builder.header("Cookie", cookie);
    }
    let timeout = std::time::Duration::from_secs(BING_REQUEST_TIMEOUT_SECONDS);
    let response = match tokio::time::timeout(timeout, builder.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => return Err(format!("request failed: {err}")),
        Err(_) => return Err("request timeout".to_string()),
    };
    if !response.status().is_success() {
        return Err(format!("status {}", response.status()));
    }
    // 搜索响应也会带 Set-Cookie（Bing 会顺带更新 SRCHUSR、_EDGE_S），合并回表里。
    let set_cookies = collect_set_cookie_headers(response.headers());
    if !set_cookies.is_empty() {
        apply_bing_response_cookies(&set_cookies, false);
    }
    match tokio::time::timeout(timeout, response.text()).await {
        Ok(Ok(html)) => Ok(html),
        Ok(Err(err)) => Err(format!("Read search body failed: {err}")),
        Err(_) => Err("Read search body timeout".to_string()),
    }
}

async fn builtin_bing_search_fallback(state: &AppState, query: &str) -> Result<Value, String> {
    let client = state.shared_http_client.clone();
    let limit = 10usize;
    let raw_query = query.trim();
    // 只走 www.bing.com：市场由会话 cookie 决定，cn.bing.com 不再使用。
    let url = format!(
        "https://www.bing.com/search?q={}",
        urlencoding::encode(raw_query)
    );
    runtime_log_info(format!(
        "[工具调试] websearch 请求地址，query={}，url={}",
        raw_query, url
    ));

    let mut cookie = ensure_bing_cookies(&client).await;
    let mut html = fetch_bing_search_html(&client, &url, &cookie).await?;
    if !bing_market_is_chinese(&html) {
        // 市场掉了（cookie 失效或轮换）：带当前 cookie 再刷一次首页，只重试一次。
        runtime_log_warn(format!(
            "[搜索引擎] Bing 返回非中文市场页面，刷新 cookie 后重试一次：query={raw_query}"
        ));
        cookie = refresh_bing_cookies(&client).await;
        html = fetch_bing_search_html(&client, &url, &cookie).await?;
        if !bing_market_is_chinese(&html) {
            runtime_log_warn(format!(
                "[搜索引擎] Bing 重试后仍是非中文市场页面，放弃这次抓取：query={raw_query}"
            ));
            return Ok(serde_json::json!({"results": [], "marketMismatch": true}));
        }
    }

    let item_sel =
        Selector::parse("li.b_algo").map_err(|err| format!("Parse selector failed: {err}"))?;
    let a_sel = Selector::parse("h2 a").map_err(|err| format!("Parse selector failed: {err}"))?;
    let p_sel =
        Selector::parse("div.b_caption p").map_err(|err| format!("Parse selector failed: {err}"))?;
    let p_alt_sel =
        Selector::parse("div.b_caption div").map_err(|err| format!("Parse selector failed: {err}"))?;
    let p_fallback_sel =
        Selector::parse("p").map_err(|err| format!("Parse selector failed: {err}"))?;
    let doc = Html::parse_document(&html);
    let mut seen = std::collections::HashSet::<String>::new();
    let mut rows = Vec::new();
    for item in doc.select(&item_sel) {
        let title_node = item.select(&a_sel).next();
        let title = title_node
            .as_ref()
            .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();
        let raw_link = title_node
            .as_ref()
            .and_then(|n| n.value().attr("href"))
            .unwrap_or_default();
        let link = normalize_bing_result_url(raw_link);
        let snippet = item
            .select(&p_sel)
            .next()
            .or_else(|| item.select(&p_alt_sel).next())
            .or_else(|| item.select(&p_fallback_sel).next())
            .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
            .unwrap_or_default();
        let key = canonical_url_key(&link);
        if !title.is_empty() && !link.is_empty() && !key.is_empty() && seen.insert(key) {
            rows.push(serde_json::json!({"title": title, "url": link, "snippet": snippet}));
            if rows.len() >= limit {
                break;
            }
        }
    }
    if !rows.is_empty() {
        return Ok(serde_json::json!({ "results": rows }));
    }
    Err(format!("bing search failed: no results parsed (request_url={url})"))
}

#[cfg(test)]
mod bing_search_tests {
    use super::*;
    use serde_json::json;

    fn instant() -> std::time::Instant {
        std::time::Instant::now()
    }

    fn jar(cookies: &[(&str, &str)], refreshed_at: std::time::Instant) -> BingCookieJar {
        BingCookieJar {
            cookies: cookies
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        BingCookie {
                            value: value.to_string(),
                            expires_at: None,
                        },
                    )
                })
                .collect(),
            refreshed_at,
        }
    }

    // ========== cookie 表合并 ==========

    #[test]
    fn cookie_header_should_join_name_value_pairs() {
        let mut cookies = std::collections::BTreeMap::new();
        cookies.insert(
            "MUID".to_string(),
            BingCookie {
                value: "3A1B".to_string(),
                expires_at: None,
            },
        );
        cookies.insert(
            "SRCHHPGUSR".to_string(),
            BingCookie {
                value: "SRCHLANG=zh".to_string(),
                expires_at: None,
            },
        );
        assert_eq!(
            bing_cookie_header(&cookies),
            "MUID=3A1B; SRCHHPGUSR=SRCHLANG=zh"
        );
    }

    #[test]
    fn merge_should_overwrite_same_name_and_keep_untouched_keys() {
        let now = std::time::SystemTime::now();
        let mut cookies = std::collections::BTreeMap::new();
        cookies.insert(
            "MUID".to_string(),
            BingCookie {
                value: "old".to_string(),
                expires_at: None,
            },
        );
        cookies.insert(
            "SRCHUSR".to_string(),
            BingCookie {
                value: "keep".to_string(),
                expires_at: None,
            },
        );
        merge_bing_set_cookies(
            &mut cookies,
            &[
                "MUID=new; path=/; domain=.bing.com".to_string(),
                "_EDGE_S=t=1; path=/".to_string(),
            ],
            now,
        );
        assert_eq!(cookies.get("MUID").map(|c| c.value.as_str()), Some("new"));
        assert_eq!(cookies.get("SRCHUSR").map(|c| c.value.as_str()), Some("keep"));
        assert_eq!(cookies.get("_EDGE_S").map(|c| c.value.as_str()), Some("t=1"));
    }

    #[test]
    fn merge_should_remove_cookie_with_empty_value() {
        let now = std::time::SystemTime::now();
        let mut cookies = std::collections::BTreeMap::new();
        cookies.insert(
            "MUID".to_string(),
            BingCookie {
                value: "old".to_string(),
                expires_at: None,
            },
        );
        merge_bing_set_cookies(&mut cookies, &["MUID=; path=/".to_string()], now);
        assert!(cookies.is_empty());
    }

    #[test]
    fn merge_should_remove_cookie_with_past_expiry() {
        let now = std::time::SystemTime::now();
        let mut cookies = std::collections::BTreeMap::new();
        for name in ["MUID", "SRCHUSR"] {
            cookies.insert(
                name.to_string(),
                BingCookie {
                    value: "old".to_string(),
                    expires_at: None,
                },
            );
        }
        merge_bing_set_cookies(
            &mut cookies,
            &[
                "MUID=; expires=Thu, 01 Jan 1970 00:00:00 GMT".to_string(),
                "SRCHUSR=dead; max-age=0".to_string(),
            ],
            now,
        );
        assert!(cookies.is_empty(), "过期与 max-age=0 都要删掉");
    }

    #[test]
    fn prune_should_drop_cookies_past_their_expiry() {
        let now = std::time::SystemTime::now();
        let mut cookies = std::collections::BTreeMap::new();
        cookies.insert(
            "ALIVE".to_string(),
            BingCookie {
                value: "1".to_string(),
                expires_at: Some(now + std::time::Duration::from_secs(60)),
            },
        );
        cookies.insert(
            "DEAD".to_string(),
            BingCookie {
                value: "2".to_string(),
                expires_at: Some(now - std::time::Duration::from_secs(1)),
            },
        );
        prune_expired_bing_cookies(&mut cookies, now);
        assert!(cookies.contains_key("ALIVE"));
        assert!(!cookies.contains_key("DEAD"));
    }

    #[test]
    fn cookie_datetime_should_parse_rfc1123() {
        let parsed = parse_cookie_datetime("Wed, 05 Nov 2026 00:00:00 GMT").expect("应能解析");
        let seconds = parsed
            .duration_since(std::time::UNIX_EPOCH)
            .expect("晚于纪元")
            .as_secs();
        // 2026-11-05T00:00:00Z
        assert_eq!(seconds, 1_793_836_800);
        assert!(parse_cookie_datetime("not a date").is_none());
    }

    // ========== cookie 缓存与续期 ==========

    #[test]
    fn jar_should_be_fresh_before_ttl_and_stale_after() {
        let now = instant();
        let fresh = jar(&[("MUID", "1")], now);
        assert!(!bing_cookie_jar_is_stale(Some(&fresh), now));
        let old = jar(
            &[("MUID", "1")],
            now - std::time::Duration::from_secs(BING_COOKIE_TTL_SECONDS + 1),
        );
        assert!(bing_cookie_jar_is_stale(Some(&old), now), "超过 24 小时要续期");
        assert!(bing_cookie_jar_is_stale(None, now), "从没拿过就是要取");
    }

    #[test]
    fn ttl_should_be_twenty_four_hours() {
        assert_eq!(BING_COOKIE_TTL_SECONDS, 86_400);
    }

    #[test]
    fn request_should_still_build_without_cookie() {
        let client = reqwest::Client::new();
        let url = "https://www.bing.com/search?q=x";
        let without = client
            .get(url)
            .header("User-Agent", BING_SEARCH_UA)
            .build()
            .expect("无 cookie 也要能建请求");
        assert!(without.headers().get(reqwest::header::COOKIE).is_none());
        let with = bing_request(&client, "MUID=1; _EDGE_S=t=1")
            .build()
            .expect("带 cookie 建请求");
        assert_eq!(
            with.headers()
                .get(reqwest::header::COOKIE)
                .and_then(|value| value.to_str().ok()),
            Some("MUID=1; _EDGE_S=t=1")
        );
    }

    // ========== 市场判定 ==========

    #[test]
    fn market_check_should_read_html_lang() {
        assert!(bing_market_is_chinese(r#"<html lang="zh-CN"><body>x</body></html>"#));
        assert!(!bing_market_is_chinese(r#"<html lang="ja"><body>x</body></html>"#));
        assert!(!bing_market_is_chinese(r#"<html lang="en"><body>x</body></html>"#));
        assert!(bing_market_is_chinese("<html><body>x</body></html>"), "读不到 lang 不该反复重试");
        assert_eq!(
            html_lang_value(r#"<html lang="zh" data-x="1">"#),
            Some("zh".to_string())
        );
    }

    // ========== 相关性 ==========

    #[test]
    fn keywords_should_split_cjk_into_bigrams() {
        let keywords = search_query_keywords("原神 纳西妲");
        for expected in ["原神", "纳西", "西妲", "纳西妲"] {
            assert!(
                keywords.contains(&expected.to_string()),
                "缺少词元 {expected}: {keywords:?}"
            );
        }
    }

    #[test]
    fn keywords_should_drop_single_chars_and_stopwords() {
        let keywords = search_query_keywords("这个 角色 的 技能 是什么");
        for dropped in ["的", "这个", "什么"] {
            assert!(
                !keywords.contains(&dropped.to_string()),
                "{dropped} 不该进词元表: {keywords:?}"
            );
        }
        for kept in ["角色", "技能"] {
            assert!(keywords.contains(&kept.to_string()), "缺少词元 {kept}");
        }
    }

    #[test]
    fn relevance_should_keep_rows_mentioning_query() {
        let rows = vec![
            json!({"title": "纳西妲 - 米游社", "url": "https://www.miyoushe.com/x", "snippet": "须弥的草神"}),
            json!({"title": "日本酒店预订", "url": "https://example.jp/hotel", "snippet": "东京 旅馆"}),
        ];
        let kept = filter_relevant_bing_results("原神 纳西妲", rows);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0]["title"], json!("纳西妲 - 米游社"));
    }

    #[test]
    fn relevance_should_drop_everything_when_nothing_matches() {
        let rows = vec![
            json!({"title": "日本酒店预订", "url": "https://example.jp/hotel", "snippet": "东京 旅馆"}),
            json!({"title": "小红书 - 你的生活兴趣社区", "url": "https://www.xiaohongshu.com/", "snippet": "发现精彩"}),
        ];
        assert!(filter_relevant_bing_results("zzqxvvbnmkljhgf", rows).is_empty());
    }

    #[test]
    fn relevance_should_keep_everything_when_query_has_no_keywords() {
        let rows = vec![json!({"title": "任意", "url": "https://example.com/", "snippet": ""})];
        assert_eq!(filter_relevant_bing_results("", rows.clone()).len(), 1);
        assert_eq!(filter_relevant_bing_results("!!! ???", rows).len(), 1);
    }

    #[test]
    fn relevance_should_match_url_case_insensitively() {
        let rows = vec![json!({"title": "Docs", "url": "https://Rust-Lang.org/", "snippet": ""})];
        assert_eq!(filter_relevant_bing_results("rust-lang", rows).len(), 1);
    }
}
