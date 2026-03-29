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

async fn builtin_fetch(url: &str, max_length: usize) -> Result<Value, String> {
    let normalized_url = url.trim();
    if normalized_url.is_empty() {
        return Ok(serde_json::json!({
          "ok": false,
          "url": "",
          "status": Value::Null,
          "error": "empty_url",
          "message": "Fetch url is empty.",
          "content": ""
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
              "content": ""
            }));
        }
    };

    let mut client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(12));
    if let Some(host) = validated.resolve_host.as_deref() {
        for addr in &validated.resolved_addrs {
            client_builder = client_builder.resolve(host, *addr);
        }
    }
    let client = match client_builder.build() {
        Ok(client) => client,
        Err(err) => {
            let build_err = format!("Build HTTP client failed: {err}");
            return Ok(serde_json::json!({
              "ok": false,
              "url": normalized_url,
              "status": Value::Null,
              "error": "build_http_client_failed",
              "message": build_err,
              "content": ""
            }));
        }
    };
    let resp = client
        .get(validated.url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
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
          "content": ""
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
          "content": fallback_content
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
      "content": truncated
    }))
}

// ========== bing search ==========

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        ('\u{4E00}'..='\u{9FFF}').contains(&ch)
            || ('\u{3400}'..='\u{4DBF}').contains(&ch)
            || ('\u{3040}'..='\u{30FF}').contains(&ch)
            || ('\u{AC00}'..='\u{D7AF}').contains(&ch)
    })
}

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

async fn builtin_bing_search(query: &str) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let limit = 10usize;
    let raw_query = query.trim();
    let mut last_error: Option<String> = None;
    let mut last_request_url: Option<String> = None;
    let prefer_cn = contains_cjk(raw_query);
    let bases = if prefer_cn {
        ["https://cn.bing.com", "https://www.bing.com"]
    } else {
        ["https://www.bing.com", "https://cn.bing.com"]
    };
    for base in bases {
        let item_sel =
            Selector::parse("li.b_algo").map_err(|err| format!("Parse selector failed: {err}"))?;
        let a_sel =
            Selector::parse("h2 a").map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_sel = Selector::parse("div.b_caption p")
            .map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_alt_sel = Selector::parse("div.b_caption div")
            .map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_fallback_sel =
            Selector::parse("p").map_err(|err| format!("Parse selector failed: {err}"))?;
        let url = format!("{base}/search?q={}", urlencoding::encode(raw_query));
        last_request_url = Some(url.clone());
        eprintln!(
            "[工具调试] websearch 请求地址，query={}，url={}",
            raw_query, url
        );
        let resp = client
            .get(&url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
            )
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
            .send()
            .await;
        let resp = match resp {
            Ok(resp) => {
                if !resp.status().is_success() {
                    last_error = Some(format!("status {}", resp.status()));
                    continue;
                }
                resp
            }
            Err(err) => {
                last_error = Some(format!("request failed: {err}"));
                continue;
            }
        };
        let html = resp
            .text()
            .await
            .map_err(|err| format!("Read search body failed: {err}"))?;
        let doc = Html::parse_document(&html);
        let mut seen = std::collections::HashSet::<String>::new();
        let mut rows = Vec::new();
        for item in doc.select(&item_sel) {
            let title_node = item
                .select(&a_sel)
                .next();
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
            return Ok(serde_json::json!({
                "query": query,
                "requestUrl": url,
                "engine": "bing",
                "results": rows
            }));
        }
        last_error = Some("no results parsed".to_string());
    }
    Err(format!(
        "bing search failed: {} (request_url={})",
        last_error.unwrap_or_else(|| "unknown".to_string()),
        last_request_url.unwrap_or_else(|| "<none>".to_string())
    ))
}

