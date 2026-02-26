#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
    tool_history_events: Vec<Value>,
}

fn prepared_history_to_rig_messages(prepared: &PreparedPrompt) -> Result<Vec<RigMessage>, String> {
    let mut chat_history = Vec::<RigMessage>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let mut user_blocks = vec![UserContent::text(hm.text.clone())];
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    user_blocks.push(UserContent::text(time_text.clone()));
                }
            }
            chat_history.push(RigMessage::User {
                content: OneOrMany::many(user_blocks)
                    .map_err(|_| "Failed to build user history message".to_string())?,
            });
        } else if hm.role == "assistant" {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(hm.text.clone())),
            });
        }
    }
    Ok(chat_history)
}

#[derive(Debug, Clone, Copy)]
enum OpenAiRigApiKind {
    ChatCompletions,
    Responses,
}

fn build_openai_rig_prompt(
    prepared: &PreparedPrompt,
) -> Result<(Vec<RigMessage>, RigMessage), String> {
    let chat_history = prepared_history_to_rig_messages(prepared)?;
    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text.clone()));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }

    for (mime, bytes) in &prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes.clone(),
            image_media_type_from_mime(mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in &prepared.latest_audios {
        content_items.push(UserContent::audio(bytes.clone(), audio_media_type_from_mime(mime)));
    }

    let current_prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let current_prompt = RigMessage::User {
        content: current_prompt_content,
    };
    Ok((chat_history, current_prompt))
}

async fn call_model_openai_rig_style_internal(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    kind: OpenAiRigApiKind,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    let (chat_history, current_prompt) = build_openai_rig_prompt(&prepared)?;
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;

    match kind {
        OpenAiRigApiKind::ChatCompletions => {
            let agent = client
                .completions_api()
                .agent(model_name)
                .preamble(&prepared.preamble)
                .temperature(api_config.temperature)
                .build();
            let mut stream = agent
                .stream_completion(current_prompt, chat_history)
                .await
                .map_err(|err| format!("rig stream completion build failed: {err}"))?
                .stream()
                .await
                .map_err(|err| format!("rig stream start failed: {err}"))?;
            collect_streaming_model_reply(&mut stream, None).await
        }
        OpenAiRigApiKind::Responses => {
            // IMPORTANT: do NOT call .completions_api() here; keep default Responses API.
            let agent = client
                .agent(model_name)
                .preamble(&prepared.preamble)
                .temperature(api_config.temperature)
                .build();
            let mut stream = agent
                .stream_completion(current_prompt, chat_history)
                .await
                .map_err(|err| format!("rig responses stream completion build failed: {err}"))?
                .stream()
                .await
                .map_err(|err| format!("rig responses stream start failed: {err}"))?;
            collect_streaming_model_reply(&mut stream, on_delta).await
        }
    }
}

async fn call_model_openai_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    call_model_openai_rig_style_internal(
        api_config,
        model_name,
        prepared,
        OpenAiRigApiKind::ChatCompletions,
        None,
    )
    .await
}

async fn call_model_openai_responses_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    call_model_openai_rig_style_internal(
        api_config,
        model_name,
        prepared,
        OpenAiRigApiKind::Responses,
        on_delta,
    )
    .await
}

fn normalize_gemini_rig_base_url(raw: &str) -> String {
    let mut base = raw.trim().trim_end_matches('/').to_string();
    for suffix in ["/v1beta/openai", "/v1beta", "/openai"] {
        if base.ends_with(suffix) {
            base = base.trim_end_matches(suffix).trim_end_matches('/').to_string();
            break;
        }
    }
    base
}

async fn call_model_gemini_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let chat_history = prepared_history_to_rig_messages(&prepared)?;
    let mut client_builder = gemini::Client::builder().api_key(&api_config.api_key);
    let normalized_base = normalize_gemini_rig_base_url(&api_config.base_url);
    if !normalized_base.is_empty() {
        client_builder = client_builder.base_url(&normalized_base);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Gemini client via rig: {err}"))?;

    let gemini_safety_settings = serde_json::json!({
        "safetySettings": [
            {
                "category": "HARM_CATEGORY_HARASSMENT",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_HATE_SPEECH",
                "threshold": "BLOCK_NONE"
            },
            {
                "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                "threshold": "BLOCK_NONE"
            }
        ]
    });

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .additional_params(gemini_safety_settings)
        .build();

    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text.clone()));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }

    for (mime, bytes) in &prepared.latest_images {
        if mime.trim().eq_ignore_ascii_case("application/pdf") {
            content_items.push(UserContent::document(bytes.clone(), Some(DocumentMediaType::PDF)));
        } else {
            content_items.push(UserContent::image_base64(
                bytes.clone(),
                image_media_type_from_mime(mime),
                Some(ImageDetail::Auto),
            ));
        }
    }

    for (mime, bytes) in &prepared.latest_audios {
        content_items.push(UserContent::audio(bytes.clone(), audio_media_type_from_mime(mime)));
    }

    let current_prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let current_prompt = RigMessage::User {
        content: current_prompt_content,
    };
    let mut stream = agent
        .stream_completion(current_prompt, chat_history)
        .await
        .map_err(|err| format!("rig stream completion build failed: {err}"))?
        .stream()
        .await
        .map_err(|err| format!("rig stream start failed: {err}"))?;
    collect_streaming_model_reply(&mut stream, None).await
}

async fn call_model_anthropic_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let chat_history = prepared_history_to_rig_messages(&prepared)?;
    let mut content_items: Vec<UserContent> = Vec::new();
    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text.clone()));
    }
    if !prepared.latest_user_time_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_time_text.clone()));
    }
    if !prepared.latest_user_system_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }

    for (mime, bytes) in &prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes.clone(),
            image_media_type_from_mime(mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in &prepared.latest_audios {
        content_items.push(UserContent::audio(bytes.clone(), audio_media_type_from_mime(mime)));
    }

    let current_prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let current_prompt = RigMessage::User {
        content: current_prompt_content,
    };

    let mut client_builder: anthropic::ClientBuilder =
        anthropic::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create Anthropic client via rig: {err}"))?;

    let agent = client
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .build();
    let mut stream = agent
        .stream_completion(current_prompt, chat_history)
        .await
        .map_err(|err| format!("rig stream completion build failed: {err}"))?
        .stream()
        .await
        .map_err(|err| format!("rig stream start failed: {err}"))?;
    collect_streaming_model_reply(&mut stream, None).await
}

fn debug_value_snippet(value: &Value, max_chars: usize) -> String {
    let raw = serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string());
    if raw.chars().count() <= max_chars {
        raw
    } else {
        let head = raw.chars().take(max_chars).collect::<String>();
        format!("{head}...")
    }
}

fn send_tool_status_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    tool_name: &str,
    tool_status: &str,
    message: &str,
) {
    let send_result = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        tool_name: Some(tool_name.to_string()),
        tool_status: Some(tool_status.to_string()),
        message: Some(message.to_string()),
    });
    eprintln!(
        "[TOOL-DEBUG] tool_status_event send={:?} name={} status={} message={}",
        send_result, tool_name, tool_status, message
    );
}

fn tool_enabled(selected_api: &ApiConfig, id: &str) -> bool {
    if matches!(id, "desktop-screenshot" | "desktop-wait") && !selected_api.enable_image {
        return false;
    }
    selected_api.enable_tools
        && selected_api
            .tools
            .iter()
            .any(|tool| tool.id == id && tool.enabled)
}

#[derive(Debug)]
struct ToolInvokeError(String);

impl std::fmt::Display for ToolInvokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolInvokeError {}

impl From<String> for ToolInvokeError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_image_unsupported_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unknown variant `image_url`")
        || lower.contains("expected `text`")
        || lower.contains("does not support image")
        || lower.contains("image input")
}

fn truncate_by_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

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
    let client = client_builder.build();
    let Ok(client) = client else {
        let build_err = client
            .err()
            .map(|err| format!("Build HTTP client failed: {err}"))
            .unwrap_or_else(|| "Build HTTP client failed: unknown error".to_string());
        return Ok(serde_json::json!({
          "ok": false,
          "url": normalized_url,
          "status": Value::Null,
          "error": "build_http_client_failed",
          "message": build_err,
          "content": ""
        }));
    };
    let resp = client
        .get(validated.url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .send()
        .await;
    let Ok(resp) = resp else {
        return Ok(serde_json::json!({
          "ok": false,
          "url": normalized_url,
          "status": Value::Null,
          "error": "request_failed",
          "message": format!("Fetch url failed: {}", resp.err().unwrap()),
          "content": ""
        }));
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
    let cleaned = clean_text(&raw);
    let truncated = truncate_by_chars(&cleaned, max_length);
    Ok(serde_json::json!({
      "ok": true,
      "url": normalized_url,
      "status": status.as_u16(),
      "content": truncated
    }))
}

async fn builtin_bing_search(query: &str) -> Result<Value, String> {
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
            if key == "url" {
                if value.starts_with("http://") || value.starts_with("https://") {
                    return value.to_string();
                }
            }
            if key == "u" {
                let decoded_url = urlencoding::decode(value)
                    .map(|x| x.into_owned())
                    .unwrap_or_else(|_| value.to_string());
                if decoded_url.starts_with("http://") || decoded_url.starts_with("https://") {
                    return decoded_url;
                }
                // Bing often stores target as base64 with an `a1` prefix.
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
            "[TOOL-DEBUG] bing-search request_url query={} url={}",
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
        let Ok(resp) = resp else {
            last_error = Some("request failed".to_string());
            continue;
        };
        if !resp.status().is_success() {
            last_error = Some(format!("status {}", resp.status()));
            continue;
        }
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

fn normalize_memory_keywords(raw: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for item in raw {
        let v = item.trim().to_lowercase();
        if v.chars().count() < 2 {
            continue;
        }
        if !out.iter().any(|x| x == &v) {
            out.push(v);
        }
        if out.len() >= 12 {
            break;
        }
    }
    out
}

fn memory_contains_sensitive(content: &str, keywords: &[String]) -> bool {
    let mut full = content.to_lowercase();
    if !keywords.is_empty() {
        full.push('\n');
        full.push_str(&keywords.join(" ").to_lowercase());
    }
    let danger_tokens = [
        "password",
        "passwd",
        "api key",
        "apikey",
        "token",
        "secret",
        "private key",
        "sk-",
        "ssh-rsa",
        "验证码",
        "密码",
        "密钥",
        "身份证",
        "银行卡",
        "cvv",
    ];
    danger_tokens.iter().any(|token| full.contains(token))
}

#[derive(Debug, Clone)]
struct MemorySaveDraft {
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MemorySaveUpsertItemResult {
    saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn parse_memory_save_draft(
    memory_type_raw: &str,
    judgment: &str,
    reasoning: &str,
    tags_raw: Vec<String>,
) -> Result<MemorySaveDraft, String> {
    let judgment = judgment.trim();
    if judgment.is_empty() {
        return Err("memory_save.judgment is required".to_string());
    }
    let memory_type = memory_store_normalize_memory_type(memory_type_raw)?;
    let tags = normalize_memory_keywords(&tags_raw);
    if tags.is_empty() {
        return Err("memory_save.tags must contain at least one valid tag".to_string());
    }
    Ok(MemorySaveDraft {
        memory_type,
        judgment: judgment.to_string(),
        reasoning: clean_text(reasoning),
        tags,
    })
}

fn upsert_memories(
    app_state: &AppState,
    drafts: &[MemorySaveDraft],
) -> Result<(Vec<MemorySaveUpsertItemResult>, usize), String> {
    let owner_agent_id = {
        let guard = app_state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut data = read_app_data(&app_state.data_path)?;
        ensure_default_agent(&mut data);
        drop(guard);
        data.agents
            .iter()
            .find(|a| a.id == data.selected_agent_id && !a.is_built_in_user && a.private_memory_enabled)
            .map(|a| a.id.clone())
    };
    let inputs = drafts
        .iter()
        .map(|d| MemoryDraftInput {
            memory_type: d.memory_type.clone(),
            judgment: d.judgment.clone(),
            reasoning: d.reasoning.clone(),
            tags: d.tags.clone(),
            owner_agent_id: owner_agent_id.clone(),
        })
        .collect::<Vec<_>>();
    let (results, total_memories) = memory_store_upsert_drafts(&app_state.data_path, &inputs)?;
    for (draft, result) in drafts.iter().zip(results.iter()) {
        if let Some(id) = result.id.as_ref() {
            eprintln!(
                "[TOOL-DEBUG] memory-save saved. id={}, type={}, tags={}, judgment_len={}",
                id,
                draft.memory_type,
                draft.tags.join(","),
                draft.judgment.chars().count()
            );
        }
    }
    Ok((results, total_memories))
}

fn builtin_memory_save(app_state: &AppState, args: Value) -> Result<Value, String> {
    let memory_type = args
        .get("memory_type")
        .or_else(|| args.get("memoryType"))
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_save.memoryType is required".to_string())?;
    let judgment = args
        .get("judgment")
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_save.judgment is required".to_string())?;
    let reasoning = args
        .get("reasoning")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tags_raw = args
        .get("tags")
        .and_then(Value::as_array)
        .ok_or_else(|| "memory_save.tags is required".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let draft = parse_memory_save_draft(memory_type, judgment, reasoning, tags_raw)?;
    let (results, total_memories) = upsert_memories(app_state, &[draft])?;
    let first = results
        .into_iter()
        .next()
        .ok_or_else(|| "memory_save failed to produce result".to_string())?;
    Ok(serde_json::json!({
      "saved": first.saved,
      "id": first.id,
      "tags": first.tags,
      "updatedAt": first.updated_at,
      "reason": first.reason,
      "totalMemories": total_memories
    }))
}

async fn builtin_desktop_wait(ms: u64) -> Result<Value, String> {
    let res = run_wait_tool(WaitRequest {
        mode: WaitMode::Sleep,
        ms,
    })
    .await
    .map_err(|err| to_tool_err_string(&err))?;
    serde_json::to_value(res).map_err(|err| format!("serialize desktop wait result failed: {err}"))
}

async fn builtin_refresh_mcp_and_skills(app_state: &AppState) -> Result<Value, String> {
    let mut result = {
        let guard = app_state
            .state_lock
            .lock()
            .map_err(|err| {
                format!(
                    "Failed to lock state mutex at {}:{}:{}: {}",
                    file!(),
                    line!(),
                    module_path!(),
                    err
                )
            })?;
        let result = refresh_workspace_mcp_and_skills(app_state)?;
        drop(guard);
        result
    };
    match mcp_redeploy_all_from_policy(app_state).await {
        Ok(deploy_errors) => {
            if !deploy_errors.is_empty() {
                result.mcp_failed.extend(deploy_errors);
            }
        }
        Err(err) => {
            result.mcp_failed.push(WorkspaceLoadError {
                item: "mcp_redeploy_all_from_policy".to_string(),
                error: err,
            });
        }
    }
    serde_json::to_value(result).map_err(|err| format!("Serialize refresh result failed: {err}"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FetchToolArgs {
    url: String,
    #[serde(default)]
    max_length: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BingSearchToolArgs {
    query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemorySaveToolArgs {
    memory_type: String,
    judgment: String,
    reasoning: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DesktopWaitToolArgs {
    ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RefreshMcpAndSkillsToolArgs {}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TerminalExecToolArgs {
    command: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ShellSwitchWorkspaceToolArgs {
    workspace_name: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct BuiltinFetchTool;

impl Tool for BuiltinFetchTool {
    const NAME: &'static str = "fetch";
    type Error = ToolInvokeError;
    type Args = FetchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "fetch".to_string(),
            description: "Fetch webpage text.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "url": { "type": "string", "description": "URL" },
                "max_length": { "type": "integer", "description": "Max chars", "default": 1800 }
              },
              "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=fetch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_fetch(&args.url, args.max_length.unwrap_or(1800))
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=fetch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=fetch err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinBingSearchTool;

impl Tool for BuiltinBingSearchTool {
    const NAME: &'static str = "bing_search";
    type Error = ToolInvokeError;
    type Args = BingSearchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "bing_search".to_string(),
            description: "Search web with Bing.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "Query" }
              },
              "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=bing-search args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_bing_search(&args.query)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=bing-search result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=bing-search err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinMemorySaveTool {
    app_state: AppState,
}

impl Tool for BuiltinMemorySaveTool {
    const NAME: &'static str = "memory_save";
    type Error = ToolInvokeError;
    type Args = MemorySaveToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "memory_save".to_string(),
            description: "保存与用户相关、长期有价值的记忆。禁止保存密码、密钥等敏感信息。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "memory_type": { "type": "string", "enum": ["knowledge", "skill", "emotion", "event"], "description": "记忆类型（不支持 task）" },
                "judgment": { "type": "string", "description": "记忆论断，单条可检索判断句" },
                "reasoning": { "type": "string", "description": "理由，可为空" },
                "tags": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "标签列表，用于后续命中提示板"
                }
              },
              "required": ["memory_type", "judgment", "tags"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::json!({
            "memoryType": args.memory_type,
            "judgment": args.judgment,
            "reasoning": args.reasoning.unwrap_or_default(),
            "tags": args.tags,
        });
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=memory-save args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_memory_save(&self.app_state, args_json).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=memory-save result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=memory-save err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDesktopWaitTool;

impl Tool for BuiltinDesktopWaitTool {
    const NAME: &'static str = "desktop_wait";
    type Error = ToolInvokeError;
    type Args = DesktopWaitToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "desktop_wait".to_string(),
            description: "Wait for specified milliseconds.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "ms": { "type": "integer", "minimum": 1, "maximum": 120000, "description": "wait milliseconds" }
              },
              "required": ["ms"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=desktop-wait args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_desktop_wait(args.ms).await.map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=desktop-wait result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=desktop-wait err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRefreshMcpAndSkillsTool {
    app_state: AppState,
}

impl Tool for BuiltinRefreshMcpAndSkillsTool {
    const NAME: &'static str = "refresh_mcp_and_skills";
    type Error = ToolInvokeError;
    type Args = RefreshMcpAndSkillsToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "refresh_mcp_and_skills".to_string(),
            description: "Reload MCP and SKILL from llm-workspace and return latest skill summary."
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {},
              "additionalProperties": false
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!("[TOOL-DEBUG] execute_builtin_tool.start name=refresh-mcp-and-skills");
        let result = builtin_refresh_mcp_and_skills(&self.app_state)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=refresh-mcp-and-skills result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.err name=refresh-mcp-and-skills err={err}"
            ),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinTerminalExecTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinTerminalExecTool {
    const NAME: &'static str = "shell_exec";
    type Error = ToolInvokeError;
    type Args = TerminalExecToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "shell_exec".to_string(),
            description: "Execute a shell command inside current shell workspace root."
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "command": { "type": "string", "description": "Shell command to execute" },
                "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 120000, "default": 20000 }
              },
              "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=shell-exec args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_shell_exec(
            &self.app_state,
            &self.session_id,
            &args.command,
            args.timeout_ms,
        )
        .await
        .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=shell-exec result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=shell-exec err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinShellSwitchWorkspaceTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinShellSwitchWorkspaceTool {
    const NAME: &'static str = "shell_switch_workspace";
    type Error = ToolInvokeError;
    type Args = ShellSwitchWorkspaceToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "shell_switch_workspace".to_string(),
            description:
                "Switch shell workspace root by workspaceName."
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "workspace_name": { "type": "string", "description": "Configured workspace name" },
                "reason": { "type": "string", "description": "Why this path is needed" }
              },
              "required": ["workspace_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=shell-switch-workspace args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_shell_switch_workspace(
            &self.app_state,
            &self.session_id,
            &args.workspace_name,
            args.reason.as_deref(),
        )
        .await
        .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=shell-switch-workspace result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.err name=shell-switch-workspace err={err}"
            ),
        }
        result
    }
}
