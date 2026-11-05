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

// ========== 结果相关性（对全部搜索引擎通用） ==========

/// 乱码或长句查询下，引擎照样会回一屏无关内容；这里做最后一道「搜到了但没用」的判定。
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

fn result_matches_query_keywords(keywords: &[String], row: &Value) -> bool {
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
fn filter_relevant_search_results(query: &str, rows: Vec<Value>) -> Vec<Value> {
    let keywords = search_query_keywords(query);
    if keywords.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| result_matches_query_keywords(&keywords, row))
        .collect()
}

#[cfg(test)]
mod search_relevance_tests {
    use super::*;
    use serde_json::json;

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
        let kept = filter_relevant_search_results("原神 纳西妲", rows);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0]["title"], json!("纳西妲 - 米游社"));
    }

    #[test]
    fn relevance_should_drop_everything_when_nothing_matches() {
        let rows = vec![
            json!({"title": "日本酒店预订", "url": "https://example.jp/hotel", "snippet": "东京 旅馆"}),
            json!({"title": "小红书 - 你的生活兴趣社区", "url": "https://www.xiaohongshu.com/", "snippet": "发现精彩"}),
        ];
        assert!(filter_relevant_search_results("zzqxvvbnmkljhgf", rows).is_empty());
    }

    #[test]
    fn relevance_should_keep_everything_when_query_has_no_keywords() {
        let rows = vec![json!({"title": "任意", "url": "https://example.com/", "snippet": ""})];
        assert_eq!(filter_relevant_search_results("", rows.clone()).len(), 1);
        assert_eq!(filter_relevant_search_results("!!! ???", rows).len(), 1);
    }

    #[test]
    fn relevance_should_match_url_case_insensitively() {
        let rows = vec![json!({"title": "Docs", "url": "https://Rust-Lang.org/", "snippet": ""})];
        assert_eq!(filter_relevant_search_results("rust-lang", rows).len(), 1);
    }
}
