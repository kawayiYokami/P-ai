#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
    tool_history_events: Vec<Value>,
    suppress_assistant_message: bool,
    trusted_input_tokens: Option<u64>,
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
            for (mime, bytes) in &hm.images {
                user_blocks.push(UserContent::image_base64(
                    bytes.clone(),
                    image_media_type_from_mime(mime),
                    Some(ImageDetail::Auto),
                ));
            }
            for (mime, bytes) in &hm.audios {
                user_blocks.push(UserContent::audio(
                    bytes.clone(),
                    audio_media_type_from_mime(mime),
                ));
            }
            chat_history.push(RigMessage::User {
                content: OneOrMany::many(user_blocks)
                    .map_err(|_| "Failed to build user history message".to_string())?,
            });
        } else if hm.role == "assistant" {
            let mut assistant_blocks = Vec::<AssistantContent>::new();
            if !hm.text.trim().is_empty() {
                assistant_blocks.push(AssistantContent::text(hm.text.clone()));
            }
            if let Some(tool_calls) = &hm.tool_calls {
                for raw in tool_calls {
                    let Some(id) = raw.get("id").and_then(Value::as_str).map(str::trim) else {
                        continue;
                    };
                    if id.is_empty() {
                        continue;
                    }
                    let Some(name) = raw
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                    else {
                        continue;
                    };
                    if name.is_empty() {
                        continue;
                    }
                    let arguments = raw
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .cloned()
                        .unwrap_or_else(|| Value::String("{}".to_string()));
                    // Normalize: stored arguments may be a JSON string (e.g. "{\"query\":\"...\"}").
                    // Gemini requires Value::Object (protobuf Struct); OpenAI's rig adapter
                    // also accepts Value::Object and stringifies it internally.
                    // Parse string→object so both providers work from the same history.
                    let arguments = match &arguments {
                        Value::String(s) => {
                            serde_json::from_str::<Value>(s).unwrap_or(arguments)
                        }
                        _ => arguments,
                    };
                    let call_id = raw
                        .get("call_id")
                        .and_then(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .or_else(|| Some(id.to_string()));
                    let tool_call = rig::message::ToolCall {
                        id: id.to_string(),
                        call_id,
                        function: rig::message::ToolFunction {
                            name: name.to_string(),
                            arguments,
                        },
                        signature: None,
                        additional_params: None,
                    };
                    assistant_blocks.push(AssistantContent::ToolCall(tool_call));
                }
            }
            if assistant_blocks.is_empty() {
                assistant_blocks.push(AssistantContent::text(String::new()));
            }
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(assistant_blocks)
                    .map_err(|_| "Failed to build assistant history message".to_string())?,
            });
        } else if hm.role == "tool" {
            let result_content = OneOrMany::one(ToolResultContent::text(hm.text.clone()));
            let tool_user_content = if let Some(tool_call_id) = hm
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
            {
                UserContent::tool_result_with_call_id(
                    tool_call_id.to_string(),
                    tool_call_id.to_string(),
                    result_content,
                )
            } else {
                UserContent::text(hm.text.clone())
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(tool_user_content),
            });
        }
    }
    Ok(chat_history)
}

#[cfg(test)]
mod prepared_history_to_rig_messages_tests {
    use super::*;

    #[test]
    fn should_replay_tool_result_message_into_rig_history() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "帮我看一下".to_string(),
                    user_time_text: Some("2026-03-01 11:00:00".to_string()),
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "xcap", "arguments": "{\"method\":\"capture_focused_window\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("thinking".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"method\":\"capture_focused_window\"}".to_string(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let chat_history = prepared_history_to_rig_messages(&prepared).expect("history built");
        let mut saw_tool_result = false;
        let mut saw_assistant_tool_call = false;

        for message in &chat_history {
            match message {
                RigMessage::Assistant { content, .. } => {
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            AssistantContent::ToolCall(call)
                                if call.id == "call_1"
                                    && call.function.name == "xcap"
                        )
                    }) {
                        saw_assistant_tool_call = true;
                    }
                }
                RigMessage::User { content } => {
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            UserContent::ToolResult(result)
                                if result.id == "call_1"
                                    && result.content.iter().any(|part| {
                                        matches!(
                                            part,
                                            ToolResultContent::Text(text)
                                                if text.text.contains("capture_focused_window")
                                        )
                                    })
                        )
                    }) {
                        saw_tool_result = true;
                    }
                }
            }
        }

        assert!(saw_assistant_tool_call, "assistant tool call should be replayed");
        assert!(saw_tool_result, "tool result should be replayed as rig UserContent::ToolResult");
    }
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
    for text_block in prepared_prompt_latest_user_text_blocks(prepared) {
        content_items.push(UserContent::text(text_block));
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
                .max_tokens(api_config.max_output_tokens as u64)
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
                .max_tokens(api_config.max_output_tokens as u64)
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
        .max_tokens(api_config.max_output_tokens as u64)
        .additional_params(gemini_safety_settings)
        .build();

    let mut content_items: Vec<UserContent> = Vec::new();
    for text_block in prepared_prompt_latest_user_text_blocks(&prepared) {
        content_items.push(UserContent::text(text_block));
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
    for text_block in prepared_prompt_latest_user_text_blocks(&prepared) {
        content_items.push(UserContent::text(text_block));
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
        .max_tokens(api_config.max_output_tokens as u64)
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
    tool_args: Option<&str>,
    message: &str,
) {
    let send_result = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        tool_name: Some(tool_name.to_string()),
        tool_status: Some(tool_status.to_string()),
        tool_args: tool_args.map(|v| v.to_string()),
        message: Some(message.to_string()),
    });
    eprintln!(
        "[TOOL-DEBUG] tool_status_event send={:?} name={} status={} message={}",
        send_result, tool_name, tool_status, message
    );
}

fn tool_enabled(
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    current_department: Option<&DepartmentConfig>,
    id: &str,
) -> bool {
    if matches!(id, "screenshot" | "wait") && !selected_api.enable_image {
        return false;
    }
    if tool_restricted_by_department(current_department, id).is_some() {
        return false;
    }
    selected_api.enable_tools
        && agent
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
            .find(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user && a.private_memory_enabled)
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

fn builtin_recall(app_state: &AppState, query: &str) -> Result<Value, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err("recall.query is required".to_string());
    }

    let memories = {
        let guard = app_state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut data = read_app_data(&app_state.data_path)?;
        ensure_default_agent(&mut data);
        let assistant_department_agent_id = data.assistant_department_agent_id.clone();
        let private_memory_enabled = data
            .agents
            .iter()
            .find(|a| a.id == assistant_department_agent_id)
            .map(|a| a.private_memory_enabled)
            .unwrap_or(false);
        let memories = memory_store_list_memories_visible_for_agent(
            &app_state.data_path,
            &assistant_department_agent_id,
            private_memory_enabled,
        )?;
        drop(guard);
        memories
    };

    let recall_hit_ids = memory_recall_hit_ids(&app_state.data_path, &memories, trimmed_query);
    let latest_recall_ids = memory_board_ids_from_current_hits(&recall_hit_ids, 7);
    let memory_board = build_memory_board_xml_from_recall_ids(&memories, &latest_recall_ids)
        .unwrap_or_default();

    Ok(serde_json::json!({
      "memoryBoard": memory_board,
      "count": latest_recall_ids.len()
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

async fn builtin_reload(app_state: &AppState) -> Result<Value, String> {
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

async fn builtin_organize_context(
    app_state: &AppState,
    api_config_id: &str,
    agent_id: &str,
) -> Result<Value, String> {
    let (selected_api, resolved_api, source, effective_agent_id) = {
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
        let mut app_config = read_config(&app_state.config_path)?;
        let mut data = read_app_data(&app_state.data_path)?;
        ensure_default_agent(&mut data);
        merge_private_organization_into_runtime_data(
            &app_state.data_path,
            &mut app_config,
            &mut data,
        )?;
        let selected_api = resolve_selected_api_config(&app_config, Some(api_config_id))
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let effective_agent_id = agent_id.trim().to_string();
        if effective_agent_id.is_empty() {
            return Err("缺少人格 ID，无法整理上下文。".to_string());
        }
        let source_idx = latest_active_conversation_index(&data, &selected_api.id, &effective_agent_id)
            .ok_or_else(|| "当前没有可整理的活动对话。".to_string())?;
        let source = data
            .conversations
            .get(source_idx)
            .cloned()
            .ok_or_else(|| "当前没有可整理的活动对话。".to_string())?;
        if source.messages.len() < 10 {
            return Ok(serde_json::json!({
                "ok": false,
                "shouldArchive": false,
                "message": "此时不应该压缩：当前对话少于 10 句。"
            }));
        }
        let usage_ratio = if source.last_context_usage_ratio.is_finite() {
            source.last_context_usage_ratio.max(0.0)
        } else {
            0.0
        };
        if usage_ratio < 0.10 {
            return Ok(serde_json::json!({
                "ok": false,
                "shouldArchive": false,
                "usageRatio": usage_ratio,
                "message": "此时不应该压缩：当前上下文占用不足 10%。"
            }));
        }
        drop(guard);
        (selected_api, resolved_api, source, effective_agent_id)
    };

    let result = run_archive_pipeline(
        app_state,
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "organize_context",
        "ORGANIZE-CONTEXT",
    )
    .await?;
    trigger_chat_queue_processing(app_state);
    serde_json::to_value(result)
        .map(|value| {
            let mut obj = serde_json::Map::new();
            obj.insert("ok".to_string(), Value::Bool(true));
            obj.insert("applied".to_string(), Value::Bool(true));
            obj.insert("result".to_string(), value);
            Value::Object(obj)
        })
        .map_err(|err| format!("Serialize organize context result failed: {err}"))
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
struct RecallToolArgs {
    query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DesktopWaitToolArgs {
    ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RefreshMcpAndSkillsToolArgs {}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OrganizeContextToolArgs {}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TerminalExecToolArgs {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteImSendToolArgs {
    channel_id: String,
    contact_id: String,
    text: String,
    #[serde(default)]
    file_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DelegateToolArgs {
    department_id: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    task_name: Option<String>,
    instruction: String,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    specific_goal: Option<String>,
    #[serde(default)]
    deliverable_requirement: Option<String>,
    #[serde(default = "default_true")]
    notify_assistant_when_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegateMode {
    Async,
    Sync,
}

fn parse_delegate_mode(raw: Option<&str>) -> Result<DelegateMode, String> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Err("delegate.mode is required".to_string()),
        Some("async") => Ok(DelegateMode::Async),
        Some("sync") => Ok(DelegateMode::Sync),
        Some(other) => Err(format!(
            "delegate.mode 必须是 `async` 或 `sync`，当前收到：{other}"
        )),
    }
}

fn debug_text_snippet(text: &str, max_chars: usize) -> String {
    let compact = text.trim().replace('\r', "").replace('\n', "\\n");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact.chars().take(max_chars).collect::<String>();
        format!("{head}...")
    }
}

fn debug_exec_result_summary(value: &Value) -> String {
    let Some(obj) = value.as_object() else {
        return debug_value_snippet(value, 320);
    };
    let ok = obj.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let approved = obj.get("approved").and_then(Value::as_bool);
    let timed_out = obj.get("timedOut").and_then(Value::as_bool).unwrap_or(false);
    let exit_code = obj.get("exitCode").and_then(Value::as_i64).unwrap_or(-1);
    let duration_ms = obj.get("durationMs").and_then(Value::as_u64).unwrap_or(0);
    let blocked_reason = obj
        .get("blockedReason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let command = obj.get("command").and_then(Value::as_str).unwrap_or_default();
    let stdout = obj.get("stdout").and_then(Value::as_str).unwrap_or_default();
    let stderr = obj.get("stderr").and_then(Value::as_str).unwrap_or_default();
    format!(
        "ok={}, approved={}, timedOut={}, exitCode={}, durationMs={}, blockedReason={}, command={}, stdout={}, stderr={}",
        ok,
        approved
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        timed_out,
        exit_code,
        duration_ms,
        if blocked_reason.is_empty() { "none" } else { blocked_reason },
        debug_text_snippet(command, 160),
        debug_text_snippet(stdout, 220),
        debug_text_snippet(stderr, 220),
    )
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TaskToolArgsWire {
    action: String,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    flow: Option<String>,
    #[serde(default)]
    todos: Option<Vec<String>>,
    #[serde(default)]
    status_summary: Option<String>,
    #[serde(default)]
    stage_key: Option<String>,
    #[serde(default)]
    append_note: Option<String>,
    #[serde(default)]
    completion_state: Option<String>,
    #[serde(default)]
    completion_conclusion: Option<String>,
    #[serde(default)]
    trigger: Option<TaskTriggerInput>,
}

fn builtin_task(app_state: &AppState, args: TaskToolArgsWire) -> Result<Value, String> {
    match args.action.trim() {
        "list" => serde_json::to_value(task_store_list_tasks(&app_state.data_path)?)
            .map_err(|err| format!("Serialize task list failed: {err}")),
        "get" => {
            let task_id = args.task_id.as_deref().map(str::trim).unwrap_or("");
            if task_id.is_empty() {
                return Err("task.taskId is required for action=get".to_string());
            }
            serde_json::to_value(task_store_get_task(&app_state.data_path, task_id)?)
                .map_err(|err| format!("Serialize task get failed: {err}"))
        }
        "create" => serde_json::to_value(task_store_create_task(&app_state.data_path, &TaskCreateInput {
            title: args.title.unwrap_or_default(),
            cause: args.cause.unwrap_or_default(),
            goal: args.goal.unwrap_or_default(),
            flow: args.flow.unwrap_or_default(),
            todos: args.todos.unwrap_or_default(),
            status_summary: args.status_summary.unwrap_or_default(),
            trigger: args.trigger.ok_or_else(|| "task.trigger is required for action=create".to_string())?,
        })?)
        .map_err(|err| format!("Serialize task create failed: {err}")),
        "update" => {
            let task_id = args.task_id.ok_or_else(|| "task.taskId is required for action=update".to_string())?;
            serde_json::to_value(task_store_update_task(&app_state.data_path, &TaskUpdateInput {
                task_id,
                title: args.title,
                cause: args.cause,
                goal: args.goal,
                flow: args.flow,
                todos: args.todos,
                status_summary: args.status_summary,
                stage_key: args.stage_key,
                append_note: args.append_note,
                trigger: args.trigger,
            })?)
            .map_err(|err| format!("Serialize task update failed: {err}"))
        }
        "complete" => {
            let task_id = args.task_id.ok_or_else(|| "task.taskId is required for action=complete".to_string())?;
            let completion_state = args
                .completion_state
                .ok_or_else(|| "task.completionState is required for action=complete".to_string())?;
            serde_json::to_value(task_store_complete_task(&app_state.data_path, &TaskCompleteInput {
                task_id,
                completion_state,
                completion_conclusion: args.completion_conclusion.unwrap_or_default(),
                status_summary: args.status_summary.unwrap_or_default(),
                append_note: args.append_note,
            })?)
            .map_err(|err| format!("Serialize task complete failed: {err}"))
        }
        _ => Err("task.action must be one of: list, get, create, update, complete".to_string()),
    }
}

fn delegate_parse_session_parts(session_id: &str) -> (String, String, Option<String>) {
    let mut parts = session_id.split("::");
    let api_config_id = parts.next().unwrap_or("").trim().to_string();
    let agent_id = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_AGENT_ID)
        .to_string();
    let conversation_id = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    (api_config_id, agent_id, conversation_id)
}

fn delegate_build_task_prompt_block(
    title: &str,
    instruction: &str,
    background: &str,
    specific_goal: &str,
    deliverable_requirement: &str,
) -> String {
    let mut lines = vec!["[DELEGATE TASK]".to_string(), format!("委托任务：{}", title.trim())];
    lines.push(format!("核心指令：{}", instruction.trim()));
    if !background.trim().is_empty() {
        lines.push(format!("背景：{}", background.trim()));
    }
    if !specific_goal.trim().is_empty() {
        lines.push(format!("具体目标：{}", specific_goal.trim()));
    }
    if !deliverable_requirement.trim().is_empty() {
        lines.push(format!("交付要求：{}", deliverable_requirement.trim()));
    }
    lines.join("\n")
}

fn delegate_build_trigger_provider_meta(
    delegate: &DelegateEntry,
    root_conversation_id: &str,
) -> Value {
    serde_json::json!({
        "messageKind": "delegate_trigger",
        "delegateId": delegate.delegate_id,
        "delegateKind": delegate.kind,
        "rootConversationId": root_conversation_id,
        "sourceDepartmentId": delegate.source_department_id,
        "targetDepartmentId": delegate.target_department_id,
        "sourceAgentId": delegate.source_agent_id,
        "targetAgentId": delegate.target_agent_id,
        "notifyAssistantWhenDone": delegate.notify_assistant_when_done,
        "callStack": delegate.call_stack,
    })
}

fn delegate_enqueue_result_message(
    app_state: &AppState,
    root_conversation_id: &str,
    speaker_agent_id: &str,
    text: &str,
    provider_meta: Value,
    notify_assistant: bool,
) -> Result<(), String> {
    // 获取会话信息
    let (department_id, agent_id) = {
        let guard = app_state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let config = read_config(&app_state.config_path)?;
        let assistant_agent_id = assistant_department_agent_id(&config)
            .ok_or_else(|| "未找到助理部门委任人".to_string())?;
        let department_id = department_for_agent_id(&config, &assistant_agent_id)
            .map(|item| item.id.clone())
            .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
        drop(guard);
        (department_id, assistant_agent_id)
    };

    // 构造委托结果消息
    let delegate_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(speaker_agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
    };

    // 创建事件并入队
    let event = ChatPendingEvent {
        id: Uuid::new_v4().to_string(),
        conversation_id: root_conversation_id.to_string(),
        created_at: now_iso(),
        source: ChatEventSource::Delegate,
        messages: vec![delegate_message],
        activate_assistant: notify_assistant,
        session_info: ChatSessionInfo {
            department_id,
            agent_id,
        },
        sender_info: None,
    };

    enqueue_chat_event(app_state, event)?;

    // 异步触发出队处理
    trigger_chat_queue_processing(app_state);

    Ok(())
}

const AGENT_WORK_EVENT_START: &str = "easy-call:agent-work-start";
const AGENT_WORK_EVENT_STOP: &str = "easy-call:agent-work-stop";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWorkSignalPayload {
    agent_id: String,
    delegate_id: String,
}

fn emit_agent_work_signal(
    app_state: &AppState,
    event_name: &str,
    agent_id: &str,
    delegate_id: &str,
) -> Result<(), String> {
    let app_handle = {
        let guard = app_state
            .app_handle
            .lock()
            .map_err(|_| "Failed to lock app handle".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "App handle is not ready".to_string())?
    };
    app_handle
        .emit(
            event_name,
            AgentWorkSignalPayload {
                agent_id: agent_id.to_string(),
                delegate_id: delegate_id.to_string(),
            },
        )
        .map_err(|err| format!("Emit agent work signal failed: {err}"))
}

async fn delegate_execute_agent_run(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    root_conversation_id: &str,
    delegate_conversation_id: &str,
) -> Result<SendChatResult, String> {
    let hidden_prompt = delegate_build_task_prompt_block(
        &delegate.title,
        &delegate.instruction,
        &delegate.background,
        &delegate.specific_goal,
        &delegate.deliverable_requirement,
    );
    let request = SendChatRequest {
        payload: ChatInputPayload {
            text: Some(hidden_prompt),
            display_text: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            provider_meta: Some(delegate_build_trigger_provider_meta(delegate, root_conversation_id)),
        },
        session: Some(SessionSelector {
            api_config_id: Some(target_api_config_id.to_string()),
            department_id: Some(delegate.target_department_id.clone()),
            agent_id: delegate.target_agent_id.clone(),
            conversation_id: Some(delegate_conversation_id.to_string()),
        }),
        speaker_agent_id: Some(delegate.source_agent_id.clone()),
        trace_id: Some(format!("delegate-{}", delegate.delegate_id)),
        oldest_queue_created_at: None,
        trigger_only: false,
    };
    let noop_channel = tauri::ipc::Channel::new(|_| Ok(()));
    send_chat_message_inner(request, app_state, &noop_channel).await
}

fn delegate_runtime_thread_update_api_config_id(
    app_state: &AppState,
    delegate_id: &str,
    _api_config_id: &str,
) -> Result<(), String> {
    delegate_runtime_thread_modify(app_state, delegate_id, |thread| {
        thread.conversation.updated_at = now_iso();
        Ok(())
    })
}

async fn delegate_run_thread_to_completion(
    app_state: AppState,
    delegate: DelegateEntry,
    target_api_config_ids: Vec<String>,
    parent_chat_session_key: Option<String>,
) -> Result<SendChatResult, String> {
    let primary_api_config_id = target_api_config_ids
        .first()
        .cloned()
        .ok_or_else(|| format!("部门没有可用模型，departmentId={}", delegate.target_department_id))?;
    let delegate_thread_id = delegate_runtime_thread_create(
        &app_state,
        &delegate,
        &primary_api_config_id,
        parent_chat_session_key,
    )?;
    if let Err(err) = emit_agent_work_signal(
        &app_state,
        AGENT_WORK_EVENT_START,
        &delegate.target_agent_id,
        &delegate.delegate_id,
    ) {
        eprintln!(
            "[委托线程] 推送开工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
            delegate_thread_id,
            delegate.delegate_id,
            delegate.target_agent_id,
            err
        );
    }
    let mut run_result = Err("未尝试任何候选模型".to_string());
    let mut errors = Vec::<String>::new();
    for api_config_id in target_api_config_ids {
        if let Err(err) =
            delegate_runtime_thread_update_api_config_id(&app_state, &delegate_thread_id, &api_config_id)
        {
            eprintln!(
                "[委托线程] 更新运行模型失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, api_config_id={}, error={}",
                delegate_thread_id,
                delegate.delegate_id,
                api_config_id,
                err
            );
        }
        match delegate_execute_agent_run(
            &app_state,
            &delegate,
            &api_config_id,
            &delegate.conversation_id,
            &delegate_thread_id,
        )
        .await
        {
            Ok(result) => {
                run_result = Ok(result);
                break;
            }
            Err(err) => {
                errors.push(format!("{api_config_id}: {err}"));
                run_result = Err(format!("部门所有候选模型均失败：{}", errors.join(" | ")));
            }
        }
    }
    match run_result {
        Ok(result) => {
            let completed_at = now_iso();
            if let Err(err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &completed_at)
            {
                eprintln!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    completed_at,
                    err
                );
            }
            if let Err(err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_COMPLETED,
            ) {
                eprintln!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    err
                );
            }
            if let Err(err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                eprintln!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    err
                );
            }
            Ok(result)
        }
        Err(err) => {
            let archived_at = now_iso();
            if let Err(remove_err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &archived_at)
            {
                eprintln!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    archived_at,
                    remove_err
                );
            }
            if let Err(status_err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            ) {
                eprintln!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    status_err
                );
            }
            if let Err(stop_err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                eprintln!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    stop_err
                );
            }
            Err(err)
        }
    }
}

fn delegate_resolve_context(
    app_state: &AppState,
    source_agent_id: &str,
    source_conversation_id: Option<&str>,
    target_department_id: &str,
) -> Result<
    (
        AppConfig,
        AppData,
        DepartmentConfig,
        DepartmentConfig,
        String,
        String,
        Option<DelegateRuntimeThread>,
    ),
    String,
> {
    let guard = app_state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut config = read_config(&app_state.config_path)?;
    let mut data = read_app_data(&app_state.data_path)?;
    let _ = ensure_default_agent(&mut data);
    merge_private_organization_into_runtime_data(&app_state.data_path, &mut config, &mut data)?;
    let source_department = department_for_agent_id(&config, source_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到发起部门，agentId={source_agent_id}"))?;
    let target_department = department_by_id(&config, target_department_id)
        .cloned()
        .ok_or_else(|| format!("目标部门不存在，departmentId={target_department_id}"))?;
    let target_agent_id = target_department
        .agent_ids
        .iter()
        .find(|id| !id.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("目标部门没有可用委任人，departmentId={target_department_id}"))?;
    if !data
        .agents
        .iter()
        .any(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
    {
        drop(guard);
        return Err(format!("目标委任人不存在，agentId={target_agent_id}"));
    }
    let thread_context = if let Some(conversation_id) = source_conversation_id {
        delegate_runtime_thread_get(app_state, conversation_id)?
    } else {
        None
    };
    let source_conversation_id = if let Some(thread) = thread_context.as_ref() {
        thread.root_conversation_id.clone()
    } else {
        let requested_conversation_id = source_conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "主代理缺少当前会话 ID，无法发起委托".to_string())?;
        data.conversations
            .iter()
            .find(|item| {
                item.id == requested_conversation_id
                    && item.summary.trim().is_empty()
                    && !conversation_is_delegate(item)
            })
            .map(|item| item.id.clone())
            .ok_or_else(|| format!("未找到指定主会话，conversationId={requested_conversation_id}"))?
    };
    drop(guard);
    Ok((
        config,
        data,
        source_department,
        target_department,
        target_agent_id,
        source_conversation_id,
        thread_context,
    ))
}

fn delegate_create_record(
    app_state: &AppState,
    root_conversation_id: &str,
    parent_delegate_id: Option<String>,
    source_department_id: &str,
    target_department_id: &str,
    source_agent_id: &str,
    target_agent_id: &str,
    title: &str,
    instruction: &str,
    background: String,
    specific_goal: String,
    deliverable_requirement: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
) -> Result<DelegateEntry, String> {
    delegate_store_create_delegate(
        &app_state.data_path,
        &DelegateCreateInput {
            kind: DELEGATE_TOOL_KIND_DELEGATE.to_string(),
            conversation_id: root_conversation_id.to_string(),
            parent_delegate_id,
            source_department_id: source_department_id.to_string(),
            target_department_id: target_department_id.to_string(),
            source_agent_id: source_agent_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            title: title.to_string(),
            instruction: instruction.to_string(),
            background,
            specific_goal,
            deliverable_requirement,
            notify_assistant_when_done,
            call_stack,
        },
    )
}

async fn builtin_delegate(
    app_state: &AppState,
    session_id: &str,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let mode = match parse_delegate_mode(args.mode.as_deref()) {
        Ok(value) => value,
        Err(err) => {
            return Ok(serde_json::json!({
                "status": "委托无法送达",
                "reason": err,
            }));
        }
    };
    if mode == DelegateMode::Sync {
        return delegate_execute_sync(app_state, session_id, args).await;
    }
    let (_, source_agent_id, source_conversation_id) = delegate_parse_session_parts(session_id);
    let target_department_id = args.department_id.trim();
    if target_department_id.is_empty() {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "delegate.department_id is required"
        }));
    }
    let instruction = args.instruction.trim();
    if instruction.is_empty() {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "delegate.instruction is required"
        }));
    }
    let (_config, data, source_department, target_department, target_agent_id, root_conversation_id, current_thread) =
        match delegate_resolve_context(
            app_state,
            &source_agent_id,
            source_conversation_id.as_deref(),
            target_department_id,
        ) {
            Ok(value) => value,
            Err(err) => return Ok(serde_json::json!({ "status": "委托无法送达", "reason": err })),
        };
    if current_thread.is_some() {
        eprintln!(
            "[INFO][TOOL] delegate blocked inside delegate thread: mode=async, session_id={}",
            session_id
        );
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "委托线程中禁止再次调用 delegate"
        }));
    }
    let is_main_assistant = source_department.id == ASSISTANT_DEPARTMENT_ID
        || source_department.is_built_in_assistant;
    if !is_main_assistant {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "delegate.mode=async 只能由主助理发起；需要当前线程等待结果时请使用 mode=sync"
        }));
    }
    if source_department.id == target_department.id {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "不能把委托发送给当前部门自己"
        }));
    }
    let mut call_stack = current_thread
        .as_ref()
        .map(|thread| thread.call_stack.clone())
        .unwrap_or_else(|| vec![source_department.id.clone()]);
    if call_stack.iter().any(|item| item == &target_department.id) {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": format!("目标部门已在当前调用链中，departmentId={}", target_department.id)
        }));
    }
    call_stack.push(target_department.id.clone());
    let title = args
        .task_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名委托")
        .to_string();
    let delegate = delegate_create_record(
        app_state,
        &root_conversation_id,
        None,
        &source_department.id,
        &target_department.id,
        &source_agent_id,
        &target_agent_id,
        &title,
        instruction,
        args.background.unwrap_or_default(),
        args.specific_goal.unwrap_or_default(),
        args.deliverable_requirement.unwrap_or_default(),
        args.notify_assistant_when_done,
        call_stack,
    )?;

    let target_name = data
        .agents
        .iter()
        .find(|agent| agent.id == target_agent_id)
        .map(|agent| agent.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| target_agent_id.clone());

    let app_state_for_run = app_state.clone();
    let app_state_for_publish = app_state.clone();
    let delegate_for_run = delegate.clone();
    let delegate_for_publish = delegate.clone();
    let target_api_config_ids = department_api_config_ids(&target_department);
    tokio::spawn(async move {
        let run_result = delegate_run_thread_to_completion(
            app_state_for_run,
            delegate_for_run,
            target_api_config_ids,
            None,
        )
        .await;
        match run_result {
            Ok(result) => {
                let text = if result.assistant_text.trim().is_empty() {
                    format!("《{}》已处理完成。", delegate_for_publish.title.trim())
                } else {
                    result.assistant_text.clone()
                };
                if let Err(err) = delegate_enqueue_result_message(
                    &app_state_for_publish,
                    &root_conversation_id,
                    &delegate_for_publish.target_agent_id,
                    &text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate_for_publish.delegate_id,
                        "delegateKind": delegate_for_publish.kind,
                        "resultStatus": "completed",
                        "speakerAgentId": delegate_for_publish.target_agent_id,
                        "sourceAgentId": delegate_for_publish.source_agent_id,
                        "targetAgentId": delegate_for_publish.target_agent_id,
                        "reasoningStandard": result.reasoning_standard,
                    }),
                    delegate_for_publish.notify_assistant_when_done,
                ) {
                    eprintln!(
                        "[委托线程] 投递委托完成消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        err
                    );
                }
            }
            Err(err) => {
                let fail_text = format!("《{}》执行失败：{}", delegate_for_publish.title.trim(), err);
                if let Err(enqueue_err) = delegate_enqueue_result_message(
                    &app_state_for_publish,
                    &root_conversation_id,
                    &delegate_for_publish.target_agent_id,
                    &fail_text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate_for_publish.delegate_id,
                        "delegateKind": delegate_for_publish.kind,
                        "resultStatus": "failed",
                        "speakerAgentId": delegate_for_publish.target_agent_id,
                        "sourceAgentId": delegate_for_publish.source_agent_id,
                        "targetAgentId": delegate_for_publish.target_agent_id,
                        "error": err,
                    }),
                    delegate_for_publish.notify_assistant_when_done,
                ) {
                    eprintln!(
                        "[委托线程] 投递委托失败消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        enqueue_err
                    );
                }
            }
        }
    });

    Ok(serde_json::json!({
        "status": "委托已送达",
        "delegate": delegate,
        "targetName": target_name
    }))
}

async fn delegate_execute_sync(
    app_state: &AppState,
    session_id: &str,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let (_, source_agent_id, source_conversation_id) = delegate_parse_session_parts(session_id);
    let target_department_id = args.department_id.trim();
    if target_department_id.is_empty() {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "delegate.department_id is required"
        }));
    }
    let instruction = args.instruction.trim();
    if instruction.is_empty() {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "delegate.instruction is required"
        }));
    }
    let (_config, _data, source_department, target_department, target_agent_id, root_conversation_id, current_thread) =
        match delegate_resolve_context(
            app_state,
            &source_agent_id,
            source_conversation_id.as_deref(),
            target_department_id,
        ) {
            Ok(value) => value,
            Err(err) => return Ok(serde_json::json!({ "status": "委托无法送达", "reason": err })),
        };
    if current_thread.is_some() {
        eprintln!(
            "[INFO][TOOL] delegate blocked inside delegate thread: mode=sync, session_id={}",
            session_id
        );
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "委托线程中禁止再次调用 delegate"
        }));
    }
    if source_department.id == target_department.id {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": "不能把委托发送给当前部门自己"
        }));
    }
    let mut call_stack = current_thread
        .as_ref()
        .map(|thread| thread.call_stack.clone())
        .unwrap_or_else(|| vec![source_department.id.clone()]);
    if call_stack.iter().any(|item| item == &target_department.id) {
        return Ok(serde_json::json!({
            "status": "委托无法送达",
            "reason": format!("目标部门已在当前调用链中，departmentId={}", target_department.id)
        }));
    }
    call_stack.push(target_department.id.clone());
    let parent_delegate_id = current_thread.as_ref().map(|thread| thread.delegate_id.clone());
    let title = args
        .task_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名委托")
        .to_string();
    let delegate = delegate_create_record(
        app_state,
        &root_conversation_id,
        parent_delegate_id,
        &source_department.id,
        &target_department.id,
        &source_agent_id,
        &target_agent_id,
        &title,
        instruction,
        args.background.unwrap_or_default(),
        args.specific_goal.unwrap_or_default(),
        args.deliverable_requirement.unwrap_or_default(),
        false,
        call_stack,
    )?;
    match delegate_run_thread_to_completion(
        app_state.clone(),
        delegate.clone(),
        department_api_config_ids(&target_department),
        Some(session_id.to_string()),
    )
    .await
    {
        Ok(run) => Ok(serde_json::json!({
            "status": "委托完成",
            "delegate": delegate,
            "conversationId": root_conversation_id,
            "assistantText": run.assistant_text,
            "reasoningStandard": run.reasoning_standard,
            "targetAgentId": target_agent_id,
        })),
        Err(err) => Ok(serde_json::json!({
            "status": "委托无法送达",
            "delegate": delegate,
            "reason": err,
        })),
    }
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
    const NAME: &'static str = "websearch";
    type Error = ToolInvokeError;
    type Args = BingSearchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "websearch".to_string(),
            description: "Search the web.".to_string(),
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
            "[TOOL-DEBUG] execute_builtin_tool.start name=websearch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_bing_search(&args.query)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=websearch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=websearch err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRememberTool {
    app_state: AppState,
}

impl Tool for BuiltinRememberTool {
    const NAME: &'static str = "remember";
    type Error = ToolInvokeError;
    type Args = MemorySaveToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remember".to_string(),
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
            "[TOOL-DEBUG] execute_builtin_tool.start name=remember args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_memory_save(&self.app_state, args_json).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=remember result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=remember err={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRecallTool {
    app_state: AppState,
}

impl Tool for BuiltinRecallTool {
    const NAME: &'static str = "recall";
    type Error = ToolInvokeError;
    type Args = RecallToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall".to_string(),
            description: "按查询回忆相关记忆，并返回可直接注入提示词的记忆板。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "回忆查询文本" }
              },
              "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=recall args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_recall(&self.app_state, &args.query).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=recall result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=recall err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinDesktopWaitTool;

impl Tool for BuiltinDesktopWaitTool {
    const NAME: &'static str = "wait";
    type Error = ToolInvokeError;
    type Args = DesktopWaitToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "wait".to_string(),
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
            "[TOOL-DEBUG] execute_builtin_tool.start name=wait args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_desktop_wait(args.ms).await.map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=wait result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=wait err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRefreshMcpAndSkillsTool {
    app_state: AppState,
}

impl Tool for BuiltinRefreshMcpAndSkillsTool {
    const NAME: &'static str = "reload";
    type Error = ToolInvokeError;
    type Args = RefreshMcpAndSkillsToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "reload".to_string(),
            description: "Reload MCP and skill from workspace."
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {},
              "additionalProperties": false
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!("[TOOL-DEBUG] execute_builtin_tool.start name=reload");
        let result = builtin_reload(&self.app_state)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=reload result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.err name=reload err={err}"
            ),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinOrganizeContextTool {
    app_state: AppState,
    api_config_id: String,
    agent_id: String,
}

impl Tool for BuiltinOrganizeContextTool {
    const NAME: &'static str = "organize_context";
    type Error = ToolInvokeError;
    type Args = OrganizeContextToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "organize_context".to_string(),
            description: "只有当话题已经偏离很远、无关信息可能干扰最新话题时才使用。若当前对话太短或上下文占用太低，就不应该压缩。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {},
              "additionalProperties": false
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!("[TOOL-DEBUG] execute_builtin_tool.start name=organize_context");
        let result = builtin_organize_context(&self.app_state, &self.api_config_id, &self.agent_id)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=organize_context result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.err name=organize_context err={err}"
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
    const NAME: &'static str = "exec";
    type Error = ToolInvokeError;
    type Args = TerminalExecToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "exec".to_string(),
            description: terminal_exec_tool_description(&terminal_shell_for_state(&self.app_state)),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["run", "list", "close"], "default": "run" },
                "session_id": { "type": "string", "description": "Optional shell session id; defaults to current chat session id." },
                "command": { "type": "string", "description": "Shell command to execute when action=run" },
                "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 120000, "default": 20000 }
              },
              "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=exec args={}",
            debug_value_snippet(&args_json, 240)
        );
        let resolved_session_id = args
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&self.session_id);
        let resolved_action = args
            .action
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("run");
        let resolved_command = args.command.as_deref().map(str::trim).unwrap_or("");
        if resolved_action == "run" && resolved_command.is_empty() {
            return Err(ToolInvokeError::from(
                "shell_exec.command is required when action=run".to_string(),
            ));
        }
        let result = builtin_shell_exec(
            &self.app_state,
            resolved_session_id,
            resolved_action,
            resolved_command,
            args.timeout_ms,
        )
        .await
        .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => {
                eprintln!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=exec result={}",
                    debug_value_snippet(v, 240)
                );
                eprintln!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=exec summary={}",
                    debug_exec_result_summary(v)
                );
            }
            Err(err) => {
                eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=exec err={err}")
            }
        }
        result
    }
}




#[derive(Debug, Clone)]
struct BuiltinTaskTool {
    app_state: AppState,
}

impl Tool for BuiltinTaskTool {
    const NAME: &'static str = "task";
    type Error = ToolInvokeError;
    type Args = TaskToolArgsWire;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "task".to_string(),
            description: "Manage the persistent task board. Use action=list|get|create|update|complete. Trigger rules: if trigger.run_at is omitted, the task becomes active immediately; if trigger.run_at is set, it runs once at that local time; if trigger.every_minutes is also set, it repeats every N minutes starting from trigger.run_at and must also include trigger.end_at as the stop time. When writing trigger.run_at or trigger.end_at, copy the local RFC3339 time format shown in the hidden task/current-time hints, including timezone offset and second precision; do not use milliseconds.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "get", "create", "update", "complete"] },
                "task_id": { "type": "string" },
                "title": { "type": "string" },
                "cause": { "type": "string" },
                "goal": { "type": "string" },
                "flow": { "type": "string" },
                "todos": { "type": "array", "items": { "type": "string" } },
                "status_summary": { "type": "string" },
                "stage_key": { "type": "string" },
                "append_note": { "type": "string" },
                "completion_state": { "type": "string", "enum": ["completed", "failed_completed"] },
                "completion_conclusion": { "type": "string" },
                "trigger": {
                  "type": "object",
                  "properties": {
                    "run_at": { "type": "string", "description": "Optional. Copy the same local RFC3339 format shown in the task/current-time hints, for example 2026-03-10T09:30:00+08:00. Include timezone offset, keep second precision, and do not include milliseconds. If omitted, the task becomes active immediately." },
                    "every_minutes": { "type": "integer", "minimum": 1, "description": "Optional. If set, repeat every N minutes starting from trigger.run_at. Requires trigger.run_at and trigger.end_at." },
                    "end_at": { "type": "string", "description": "Optional unless trigger.every_minutes is set. Defines when a repeating task stops. Must be later than trigger.run_at. Use the same local RFC3339 format shown in the task/current-time hints." }
                  }
                }
              },
              "required": ["action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=task args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_task(&self.app_state, args).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=task result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=task err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinDelegateTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinDelegateTool {
    const NAME: &'static str = "delegate";
    type Error = ToolInvokeError;
    type Args = DelegateToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "delegate".to_string(),
            description: "向某个部门发起委托。mode=async 仅主助理可用，表示后台处理并立即返回送达结果；mode=sync 表示当前线程等待下级完成后再继续。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "department_id": { "type": "string", "description": "目标部门 ID" },
                "mode": { "type": "string", "enum": ["async", "sync"], "description": "委托模式。async=仅主助理可用，后台处理并立即返回；sync=当前线程等待结果再继续。" },
                "task_name": { "type": "string", "description": "委托标题" },
                "instruction": { "type": "string", "description": "明确委托内容" },
                "background": { "type": "string", "description": "当前已知背景" },
                "specific_goal": { "type": "string", "description": "具体目标" },
                "deliverable_requirement": { "type": "string", "description": "交付要求" },
                "notify_assistant_when_done": { "type": "boolean", "description": "仅在 mode=async 时生效；完成后是否额外提醒主助理。mode=sync 会忽略此字段。", "default": true }
              },
              "required": ["department_id", "mode", "instruction"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=delegate args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_delegate(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=delegate result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=delegate err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRemoteImSendTool {
    app_state: AppState,
}

impl Tool for BuiltinRemoteImSendTool {
    const NAME: &'static str = "remote_im_send";
    type Error = ToolInvokeError;
    type Args = RemoteImSendToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remote_im_send".to_string(),
            description: "向远程IM联系人发送消息。当你在 [远程IM] 标记的消息中收到来自QQ等平台的消息时，你的普通文字回复对方无法看到，必须调用此工具才能将回复送达对方。channelId 和 contactId 在 [远程IM] 来源标记中已提供。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "channel_id": { "type": "string", "description": "远程IM渠道 ID（channelId）" },
                "contact_id": { "type": "string", "description": "远程联系人 ID（contactId，即QQ号或群号）" },
                "text": { "type": "string", "description": "要发送的文本内容" },
                "file_paths": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "（预留）附件文件路径列表，当前版本暂不支持"
                }
              },
              "required": ["channel_id", "contact_id", "text"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=remote_im_send args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_remote_im_send(&self.app_state, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=remote_im_send result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=remote_im_send err={err}"),
        }
        result
    }
}

async fn builtin_remote_im_send(
    state: &AppState,
    args: RemoteImSendToolArgs,
) -> Result<Value, String> {
    let channel_id = args.channel_id.trim().to_string();
    let contact_id_input = args.contact_id.trim().to_string();
    let text = args.text.trim().to_string();
    if channel_id.is_empty() {
        return Err("channel_id 不能为空".to_string());
    }
    if contact_id_input.is_empty() {
        return Err("contact_id 不能为空".to_string());
    }
    if text.is_empty() {
        return Err("text 不能为空".to_string());
    }

    let config = state_read_config_cached(state)?;
    let channel = remote_im_channel_by_id(&config, &channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {channel_id}"))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {channel_id}"));
    }

    let data = state_read_app_data_cached(state)?;
    let contact = data
        .remote_im_contacts
        .iter()
        .find(|c| c.channel_id == channel_id && c.remote_contact_id == contact_id_input)
        .ok_or_else(|| format!("未找到匹配的远程联系人: channel_id={channel_id}, contact_id={contact_id_input}"))?
        .clone();

    if !contact.allow_send {
        return Err(format!(
            "用户已禁止向该联系人发送消息: channel_id={}, contact_id={}",
            channel_id, contact_id_input
        ));
    }

    let mut content = Vec::<Value>::new();
    content.push(serde_json::json!({
        "type": "text",
        "text": text
    }));

    let payload = serde_json::json!({
        "channelId": contact.channel_id,
        "contactId": contact.id,
        "platform": channel.platform,
        "remoteContactType": contact.remote_contact_type,
        "remoteContactId": contact.remote_contact_id,
        "content": content,
    });

    let platform_message_id = remote_im_send_via_sdk(&channel, &contact, &payload).await?;

    Ok(serde_json::json!({
        "ok": true,
        "channelId": channel_id,
        "contactId": contact_id_input,
        "contactName": contact.remote_contact_name,
        "contactType": contact.remote_contact_type,
        "platformMessageId": platform_message_id
    }))
}

