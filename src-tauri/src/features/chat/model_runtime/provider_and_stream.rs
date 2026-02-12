fn openai_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = format!("Bearer {}", api_key.trim());
    let auth_value = HeaderValue::from_str(&auth)
        .map_err(|err| format!("Build authorization header failed: {err}"))?;
    headers.insert(AUTHORIZATION, auth_value);
    Ok(headers)
}

fn candidate_openai_chat_urls(base_url: &str) -> Vec<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Vec::new();
    }
    let lower = base.to_ascii_lowercase();
    let mut urls = Vec::new();
    if lower.ends_with("/chat/completions") {
        urls.push(base.to_string());
    } else if lower.ends_with("/v1") {
        urls.push(format!("{base}/chat/completions"));
    } else {
        urls.push(format!("{base}/chat/completions"));
        urls.push(format!("{base}/v1/chat/completions"));
    }
    urls.sort();
    urls.dedup();
    urls
}

fn parse_stream_delta_text(content: &Option<Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|it| it.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn longest_suffix_prefix_len(text: &str, token: &str) -> usize {
    let max = std::cmp::min(text.len(), token.len().saturating_sub(1));
    for len in (1..=max).rev() {
        if text.ends_with(&token[..len]) {
            return len;
        }
    }
    0
}

fn extract_inline_reasoning_from_chunk(
    chunk: &str,
    inline_mode: &mut bool,
    carry: &mut String,
) -> String {
    let mut src = String::new();
    if !carry.is_empty() {
        src.push_str(carry);
        carry.clear();
    }
    src.push_str(chunk);

    let mut inline = String::new();
    let mut offset = 0usize;
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";

    while offset < src.len() {
        if *inline_mode {
            if let Some(pos) = src[offset..].find(CLOSE) {
                let end = offset + pos;
                inline.push_str(&src[offset..end]);
                offset = end + CLOSE.len();
                *inline_mode = false;
            } else {
                let rest = &src[offset..];
                let keep = longest_suffix_prefix_len(rest, CLOSE);
                if keep < rest.len() {
                    inline.push_str(&rest[..rest.len() - keep]);
                }
                if keep > 0 {
                    carry.push_str(&rest[rest.len() - keep..]);
                }
                break;
            }
        } else if let Some(pos) = src[offset..].find(OPEN) {
            let end = offset + pos;
            offset = end + OPEN.len();
            *inline_mode = true;
        } else {
            let rest = &src[offset..];
            let keep = longest_suffix_prefix_len(rest, OPEN);
            if keep > 0 {
                carry.push_str(&rest[rest.len() - keep..]);
            }
            break;
        }
    }

    inline
}

fn collect_reasoning_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            if !s.trim().is_empty() {
                out.push(s.to_string());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_reasoning_strings(item, out);
            }
        }
        Value::Object(map) => {
            for key in ["text", "reasoning", "content"] {
                if let Some(v) = map.get(key) {
                    collect_reasoning_strings(v, out);
                }
            }
        }
        _ => {}
    }
}

fn parse_stream_delta_reasoning(delta: &OpenAIStreamDelta) -> String {
    let mut parts = Vec::<String>::new();
    if let Some(content) = &delta.reasoning_content {
        if !content.trim().is_empty() {
            parts.push(content.clone());
        }
    }
    if let Some(details) = &delta.reasoning_details {
        collect_reasoning_strings(details, &mut parts);
    }
    parts.join("")
}

/// 通用 OpenAI SSE 流式请求：解析文本 delta（实时推送到 on_delta）和 tool_calls 积累。
/// 返回 (完整文本, 标准思维链, 正文思维链, 积累的 tool_calls)。
async fn openai_stream_request(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(String, String, String, Vec<OpenAIToolCall>), String> {
    openai_stream_request_with_sink(client, url, body, |kind, delta| {
        let _ = on_delta.send(AssistantDeltaEvent {
            delta: delta.to_string(),
            kind: if kind == "text" {
                None
            } else {
                Some(kind.to_string())
            },
            tool_name: None,
            tool_status: None,
            message: None,
        });
    })
    .await
}

async fn openai_stream_request_with_sink<F>(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    mut on_event: F,
) -> Result<(String, String, String, Vec<OpenAIToolCall>), String>
where
    F: FnMut(&str, &str),
{
    eprintln!("[STREAM-DEBUG] openai_stream_request called, url={}", url);
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("OpenAI stream request failed: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        return Err(format!(
            "OpenAI stream failed with status {status}: {}",
            raw.chars().take(300).collect::<String>()
        ));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut output = String::new();
    let mut reasoning_standard_output = String::new();
    let mut reasoning_inline_output = String::new();
    let mut inline_mode = false;
    let mut inline_carry = String::new();

    // 积累 tool_calls：按 index 分组
    let mut tool_calls_map: std::collections::BTreeMap<usize, (String, String, String)> =
        std::collections::BTreeMap::new(); // index -> (id, name, arguments)

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|err| format!("Read stream chunk failed: {err}"))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer.drain(..=pos);

            if !line.starts_with("data:") {
                continue;
            }
            let data = line["data:".len()..].trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                break;
            }

            let Ok(parsed) = serde_json::from_str::<OpenAIStreamChunk>(data) else {
                eprintln!(
                    "[STREAM-DEBUG] SSE parse failed: {}",
                    &data[..data.len().min(200)]
                );
                continue;
            };
            if parsed.choices.is_empty() {
                continue;
            }
            let choice = &parsed.choices[0];

            // 处理标准思维链 delta（reasoning_content / reasoning_details）
            let reasoning_delta = parse_stream_delta_reasoning(&choice.delta);
            if !reasoning_delta.is_empty() {
                on_event("reasoning_standard", &reasoning_delta);
                reasoning_standard_output.push_str(&reasoning_delta);
            }

            // 处理文本 delta：保留原始文本（包含 <think> 标签），只额外提取正文思考事件
            let delta_text = parse_stream_delta_text(&choice.delta.content);
            if !delta_text.is_empty() {
                output.push_str(&delta_text);
                on_event("text", &delta_text);
                let inline = extract_inline_reasoning_from_chunk(
                    &delta_text,
                    &mut inline_mode,
                    &mut inline_carry,
                );
                if !inline.is_empty() {
                    on_event("reasoning_inline", &inline);
                    reasoning_inline_output.push_str(&inline);
                }
            }

            // 处理 tool_calls delta
            if let Some(tc_deltas) = &choice.delta.tool_calls {
                for tc_delta in tc_deltas {
                    let entry = tool_calls_map
                        .entry(tc_delta.index)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = &tc_delta.id {
                        entry.0 = id.clone();
                    }
                    if let Some(func) = &tc_delta.function {
                        if let Some(name) = &func.name {
                            entry.1.push_str(name);
                        }
                        if let Some(args) = &func.arguments {
                            entry.2.push_str(args);
                        }
                    }
                }
            }
        }
    }

    let tool_calls: Vec<OpenAIToolCall> = tool_calls_map
        .into_iter()
        .map(|(index, (id, name, arguments))| OpenAIToolCall {
            id: if id.trim().is_empty() {
                format!("tool_call_{index}")
            } else {
                id
            },
            function: OpenAIToolCallFunction { name, arguments },
        })
        .filter(|tc| !tc.function.name.trim().is_empty())
        .collect();

    Ok((
        output,
        reasoning_standard_output,
        reasoning_inline_output,
        tool_calls,
    ))
}

async fn call_model_openai_stream_text(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: &PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<ModelReply, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(openai_headers(&api_config.api_key)?)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let mut user_content = vec![serde_json::json!({
      "type": "text",
      "text": prepared.latest_user_text
    })];
    if !prepared.latest_user_system_text.trim().is_empty() {
        user_content.push(serde_json::json!({
          "type": "text",
          "text": prepared.latest_user_system_text
        }));
    }
    let mut messages = Vec::<Value>::new();
    messages.push(serde_json::json!({
      "role": "system",
      "content": prepared.preamble.clone()
    }));
    for hm in &prepared.history_messages {
        if hm.role == "assistant" && hm.tool_calls.is_some() {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("assistant".to_string()));
            if hm.text.trim().is_empty() {
                msg.insert("content".to_string(), Value::Null);
            } else {
                msg.insert("content".to_string(), Value::String(hm.text.clone()));
            }
            if let Some(reasoning) = &hm.reasoning_content {
                if !reasoning.trim().is_empty() {
                    msg.insert("reasoning_content".to_string(), Value::String(reasoning.clone()));
                }
            }
            if let Some(calls) = &hm.tool_calls {
                msg.insert("tool_calls".to_string(), Value::Array(calls.clone()));
            }
            messages.push(Value::Object(msg));
        } else if hm.role == "tool" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("tool".to_string()));
            msg.insert("content".to_string(), Value::String(hm.text.clone()));
            if let Some(call_id) = &hm.tool_call_id {
                msg.insert("tool_call_id".to_string(), Value::String(call_id.clone()));
            }
            messages.push(Value::Object(msg));
        } else {
            messages.push(serde_json::json!({
              "role": hm.role,
              "content": hm.text,
            }));
        }
    }
    messages.push(serde_json::json!({
      "role": "user",
      "content": user_content
    }));
    let body = serde_json::json!({
      "model": model_name,
      "messages": messages,
      "temperature": api_config.temperature,
      "stream": true
    });

    let urls = candidate_openai_chat_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    let mut errors = Vec::new();
    for url in urls {
        match openai_stream_request(&client, &url, body.clone(), on_delta).await {
            Ok((text, reasoning_standard, reasoning_inline, _)) => {
                return Ok(ModelReply {
                    assistant_text: text,
                    reasoning_standard,
                    reasoning_inline,
                    tool_history_events: Vec::new(),
                });
            }
            Err(err) => errors.push(format!("{url} -> {err}")),
        }
    }

    Err(format!(
        "OpenAI stream request failed for all candidate URLs: {}",
        errors.join(" || ")
    ))
}

fn deepseek_tool_schemas(selected_api: &ApiConfig) -> Vec<Value> {
    let mut tools = Vec::<Value>::new();
    if tool_enabled(selected_api, "fetch") {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "fetch",
                "description": "Fetch webpage text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "max_length": { "type": "integer", "default": 1800 }
                    },
                    "required": ["url"]
                }
            }
        }));
    }
    if tool_enabled(selected_api, "bing-search") {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "bing_search",
                "description": "Search web with Bing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "num_results": { "type": "integer", "default": 5 }
                    },
                    "required": ["query"]
                }
            }
        }));
    }
    if tool_enabled(selected_api, "memory-save") {
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "memory_save",
                "description": "保存与用户相关、长期有价值的记忆。禁止保存密码、密钥等敏感信息。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "content": { "type": "string" },
                        "keywords": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["content", "keywords"]
                }
            }
        }));
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": "memory_save_batch",
                "description": "批量保存与用户相关、长期有价值的记忆（单次最多 7 条）。禁止保存敏感信息。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "memories": {
                            "type": "array",
                            "maxItems": 7,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "content": { "type": "string" },
                                    "keywords": { "type": "array", "items": { "type": "string" } }
                                },
                                "required": ["content", "keywords"]
                            }
                        }
                    },
                    "required": ["memories"]
                }
            }
        }));
    }
    tools
}

async fn execute_builtin_tool_call(
    selected_api: &ApiConfig,
    app_state: Option<&AppState>,
    tool_name: &str,
    args_json: &Value,
) -> Result<Value, String> {
    match tool_name {
        "fetch" if tool_enabled(selected_api, "fetch") => {
            let args: FetchToolArgs = serde_json::from_value(args_json.clone())
                .map_err(|err| format!("Parse fetch args failed: {err}"))?;
            builtin_fetch(&args.url, args.max_length.unwrap_or(1800)).await
        }
        "bing_search" | "bing-search" if tool_enabled(selected_api, "bing-search") => {
            let args: BingSearchToolArgs = serde_json::from_value(args_json.clone())
                .map_err(|err| format!("Parse bing_search args failed: {err}"))?;
            builtin_bing_search(&args.query, args.num_results.unwrap_or(5)).await
        }
        "memory_save" | "memory-save" if tool_enabled(selected_api, "memory-save") => {
            let state = app_state.ok_or_else(|| "memory_save requires app state".to_string())?;
            builtin_memory_save(state, args_json.clone())
        }
        "memory_save_batch" | "memory-save-batch" if tool_enabled(selected_api, "memory-save") => {
            let state =
                app_state.ok_or_else(|| "memory_save_batch requires app state".to_string())?;
            builtin_memory_save_batch(state, args_json.clone())
        }
        _ => Err(format!("Unsupported or disabled tool: {tool_name}")),
    }
}

async fn call_model_deepseek_with_tools_http(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
) -> Result<ModelReply, String> {
    let tools = deepseek_tool_schemas(selected_api);
    if tools.is_empty() {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(openai_headers(&api_config.api_key)?)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let urls = candidate_openai_chat_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut full_reasoning_inline = String::new();

    let mut first_user_content = vec![serde_json::json!({
      "type": "text",
      "text": prepared.latest_user_text
    })];
    if !prepared.latest_user_system_text.trim().is_empty() {
        first_user_content.push(serde_json::json!({
          "type": "text",
          "text": prepared.latest_user_system_text
        }));
    }
    let mut messages = Vec::<Value>::new();
    messages.push(serde_json::json!({ "role": "system", "content": prepared.preamble }));
    for hm in &prepared.history_messages {
        if hm.role == "assistant" && hm.tool_calls.is_some() {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("assistant".to_string()));
            if hm.text.trim().is_empty() {
                msg.insert("content".to_string(), Value::Null);
            } else {
                msg.insert("content".to_string(), Value::String(hm.text.clone()));
            }
            if let Some(reasoning) = &hm.reasoning_content {
                if !reasoning.trim().is_empty() {
                    msg.insert("reasoning_content".to_string(), Value::String(reasoning.clone()));
                }
            }
            if let Some(calls) = &hm.tool_calls {
                msg.insert("tool_calls".to_string(), Value::Array(calls.clone()));
            }
            messages.push(Value::Object(msg));
        } else if hm.role == "tool" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("tool".to_string()));
            msg.insert("content".to_string(), Value::String(hm.text.clone()));
            if let Some(call_id) = &hm.tool_call_id {
                msg.insert("tool_call_id".to_string(), Value::String(call_id.clone()));
            }
            messages.push(Value::Object(msg));
        } else {
            messages.push(serde_json::json!({
                "role": hm.role,
                "content": hm.text,
            }));
        }
    }
    messages.push(serde_json::json!({ "role": "user", "content": first_user_content }));

    for _ in 0..max_tool_iterations {
        let body = serde_json::json!({
            "model": model_name,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "temperature": api_config.temperature,
            "stream": true
        });

        let mut errors = Vec::new();
        let mut turn_result: Option<(String, String, String, Vec<OpenAIToolCall>)> = None;
        for url in &urls {
            match openai_stream_request(&client, url, body.clone(), on_delta).await {
                Ok(v) => {
                    turn_result = Some(v);
                    break;
                }
                Err(err) => errors.push(format!("{url} -> {err}")),
            }
        }
        let (turn_text, reasoning_standard, reasoning_inline, tool_calls) = turn_result.ok_or_else(|| {
            format!(
                "DeepSeek stream request failed for all candidate URLs: {}",
                errors.join(" || ")
            )
        })?;

        if !turn_text.trim().is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }
        if !reasoning_standard.trim().is_empty() {
            full_reasoning_standard.push_str(&reasoning_standard);
        }
        if !reasoning_inline.trim().is_empty() {
            full_reasoning_inline.push_str(&reasoning_inline);
        }

        if tool_calls.is_empty() {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: full_reasoning_inline,
                tool_history_events,
            });
        }

        let tool_calls_payload = tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function.name,
                        "arguments": tc.function.arguments
                    }
                })
            })
            .collect::<Vec<_>>();
        let assistant_tool_event = if selected_api.request_format.trim() == "deepseek/kimi" {
            serde_json::json!({
                "role": "assistant",
                "content": if turn_text.is_empty() { Value::Null } else { Value::String(turn_text.clone()) },
                "reasoning_content": reasoning_standard,
                "tool_calls": tool_calls_payload
            })
        } else {
            serde_json::json!({
                "role": "assistant",
                "content": if turn_text.is_empty() { Value::Null } else { Value::String(turn_text.clone()) },
                "tool_calls": tool_calls_payload
            })
        };
        messages.push(assistant_tool_event.clone());
        tool_history_events.push(assistant_tool_event);

        for tc in tool_calls {
            let tool_name = tc.function.name.clone();
            send_tool_status_event(
                on_delta,
                &tool_name,
                "running",
                &format!("正在调用工具：{tool_name}"),
            );
            let args_json: Value = serde_json::from_str(&tc.function.arguments)
                .map_err(|err| format!("Parse tool arguments failed: {err}"))?;
            let tool_result = execute_builtin_tool_call(selected_api, app_state, &tool_name, &args_json)
                .await
                .map_err(|err| {
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "failed",
                        &format!("工具调用失败：{tool_name} ({err})"),
                    );
                    err
                })?;
            send_tool_status_event(
                on_delta,
                &tool_name,
                "done",
                &format!("工具调用完成：{tool_name}"),
            );
            let tool_event = serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": tool_result.to_string()
            });
            messages.push(tool_event.clone());
            tool_history_events.push(tool_event);
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: full_reasoning_inline,
        tool_history_events,
    })
}

async fn call_model_openai_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
) -> Result<ModelReply, String> {
    if selected_api.request_format.trim() == "deepseek/kimi" {
        return call_model_deepseek_with_tools_http(
            api_config,
            selected_api,
            model_name,
            prepared,
            app_state,
            on_delta,
            max_tool_iterations,
        )
        .await;
    }

    let has_fetch = tool_enabled(selected_api, "fetch");
    let has_bing = tool_enabled(selected_api, "bing-search");
    let has_memory = tool_enabled(selected_api, "memory-save");
    if !has_fetch && !has_bing && !has_memory {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;

    let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
    if has_fetch {
        tools.push(Box::new(BuiltinFetchTool));
    }
    if has_bing {
        tools.push(Box::new(BuiltinBingSearchTool));
    }
    if has_memory {
        let state = app_state
            .ok_or_else(|| "memory_save requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveTool { app_state: state }));
        let state = app_state
            .ok_or_else(|| "memory_save_batch requires app state".to_string())?
            .clone();
        tools.push(Box::new(BuiltinMemorySaveBatchTool { app_state: state }));
    }

    let agent = client
        .clone()
        .completions_api()
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .tools(tools)
        .build();

    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut prompt_blocks = vec![UserContent::text(prepared.latest_user_text.clone())];
    if !prepared.latest_user_system_text.trim().is_empty() {
        prompt_blocks.push(UserContent::text(prepared.latest_user_system_text.clone()));
    }
    let current_prompt_content = OneOrMany::many(prompt_blocks)
    .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;
    let mut current_prompt: RigMessage = RigMessage::User {
        content: current_prompt_content,
    };
    let mut chat_history = Vec::<RigMessage>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(UserContent::text(hm.text.clone())),
            });
        } else if hm.role == "assistant" {
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(hm.text.clone())),
            });
        }
    }

    for _ in 0..max_tool_iterations {
        let mut stream = agent
            .stream_completion(current_prompt.clone(), chat_history.clone())
            .await
            .map_err(|err| format!("rig stream completion build failed: {err}"))?
            .stream()
            .await
            .map_err(|err| format!("rig stream start failed: {err}"))?;

        chat_history.push(current_prompt.clone());

        let mut turn_text = String::new();
        let mut turn_reasoning = String::new();
        let mut tool_calls = Vec::<AssistantContent>::new();
        let mut tool_results = Vec::<(String, Option<String>, String)>::new();
        let mut did_call_tool = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamedAssistantContent::Text(text)) => {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                    turn_text.push_str(&text.text);
                }
                Ok(StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id: _,
                }) => {
                    did_call_tool = true;
                    let tool_call_id = tool_call.id.clone();
                    let tool_name = tool_call.function.name.clone();
                    let tool_args_value = tool_call.function.arguments.clone();
                    eprintln!(
                        "[TOOL-DEBUG] tool_call id={} name={} args={}",
                        tool_call_id,
                        tool_name,
                        tool_args_value
                    );
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "running",
                        &format!("正在调用工具：{}", tool_name),
                    );
                    let tool_args = match &tool_args_value {
                        // Some providers return arguments as JSON string payload.
                        // Passing `to_string()` here would double-encode into "\"{...}\"".
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    };
                    let tool_result = agent
                        .tool_server_handle
                        .call_tool(&tool_name, &tool_args)
                        .await
                        .map_err(|err| {
                            send_tool_status_event(
                                on_delta,
                                &tool_name,
                                "failed",
                                &format!("工具调用失败：{} ({err})", tool_name),
                            );
                            format!("Tool call '{}' failed: {err}", tool_name)
                        })?;
                    eprintln!(
                        "[TOOL-DEBUG] tool_result id={} name={} content={}",
                        tool_call_id,
                        tool_name,
                        tool_result.chars().take(240).collect::<String>()
                    );
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "done",
                        &format!("工具调用完成：{}", tool_name),
                    );
                    let assistant_tool_event = serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [
                            {
                                "id": tool_call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": tool_args_value
                                }
                            }
                        ]
                    });
                    tool_history_events.push(assistant_tool_event);
                    let tool_event = serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": tool_result.clone()
                    });
                    tool_history_events.push(tool_event);

                    tool_calls.push(AssistantContent::ToolCall(tool_call.clone()));
                    tool_results.push((tool_call.id, tool_call.call_id, tool_result));
                }
                Ok(StreamedAssistantContent::Final(_)) => {}
                Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                    let merged = reasoning.reasoning.join("\n");
                    if !merged.is_empty() {
                        if !turn_reasoning.is_empty() {
                            turn_reasoning.push('\n');
                        }
                        turn_reasoning.push_str(&merged);
                        full_reasoning_standard.push_str(&merged);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                    if !reasoning.is_empty() {
                        turn_reasoning.push_str(&reasoning);
                        full_reasoning_standard.push_str(&reasoning);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: reasoning,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
                Err(err) => return Err(format!("rig streaming failed: {err}")),
            }
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        if !did_call_tool {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                tool_history_events,
            });
        }

        if !tool_calls.is_empty() {
            let mut assistant_items = Vec::<AssistantContent>::new();
            if selected_api.request_format.trim() == "deepseek/kimi" {
                // DeepSeek thinking mode + tool calls requires reasoning_content in assistant message.
                assistant_items.push(AssistantContent::reasoning(turn_reasoning.clone()));
            }
            assistant_items.extend(tool_calls);
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(assistant_items)
                    .map_err(|_| "Failed to build assistant tool-call message".to_string())?,
            });
        }

        for (tool_id, call_id, tool_result) in tool_results {
            let result_content = OneOrMany::one(ToolResultContent::text(tool_result));
            let user_content = if let Some(call_id) = call_id {
                UserContent::tool_result_with_call_id(tool_id, call_id, result_content)
            } else {
                UserContent::tool_result(tool_id, result_content)
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(user_content),
            });
        }

        current_prompt = chat_history
            .pop()
            .ok_or_else(|| "Tool call turn ended with empty chat history".to_string())?;
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    let final_instruction = "工具调用次数达到上限，必须立刻汇报。禁止继续调用任何工具。请基于已有信息直接给出结论，并明确不确定性。";
    current_prompt = final_instruction.into();
    let final_agent = client
        .completions_api()
        .agent(model_name)
        .preamble(&prepared.preamble)
        .temperature(api_config.temperature)
        .build();
    let mut final_stream = final_agent
        .stream_completion(current_prompt.clone(), chat_history.clone())
        .await
        .map_err(|err| format!("rig final stream build failed: {err}"))?
        .stream()
        .await
        .map_err(|err| format!("rig final stream start failed: {err}"))?;
    let mut final_text = String::new();
    while let Some(chunk) = final_stream.next().await {
        match chunk {
            Ok(StreamedAssistantContent::Text(text)) => {
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: text.text.clone(),
                    kind: None,
                    tool_name: None,
                    tool_status: None,
                    message: None,
                });
                final_text.push_str(&text.text);
            }
            Ok(StreamedAssistantContent::Final(_)) => {}
            Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                let merged = reasoning.reasoning.join("\n");
                if !merged.is_empty() {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: merged,
                        kind: Some("reasoning_standard".to_string()),
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                }
            }
            Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                if !reasoning.is_empty() {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: reasoning,
                        kind: Some("reasoning_standard".to_string()),
                        tool_name: None,
                        tool_status: None,
                        message: None,
                    });
                }
            }
            Ok(StreamedAssistantContent::ToolCall { .. }) => {}
            Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
            Err(err) => return Err(format!("rig final streaming failed: {err}")),
        }
    }
    if !final_text.trim().is_empty() {
        if !full_assistant_text.trim().is_empty() {
            full_assistant_text.push_str("\n\n");
        }
        full_assistant_text.push_str(&final_text);
    }
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events,
    })
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
) -> Result<ModelReply, String> {
    eprintln!(
        "[STREAM-DEBUG] call_model_openai_style: format={}, enable_tools={}, images={}, audios={}",
        selected_api.request_format,
        selected_api.enable_tools,
        prepared.latest_images.len(),
        prepared.latest_audios.len()
    );
    // 优先使用工具调用（如果启用）
    if selected_api.enable_tools
        && is_openai_style_request_format(&selected_api.request_format)
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_with_tools(
            api_config,
            selected_api,
            model_name,
            prepared,
            app_state,
            on_delta,
            max_tool_iterations,
        )
        .await;
    }

    // 纯文本流式传输（无论工具是否启用，只要没有工具调用就走流式）
    if is_openai_style_request_format(&selected_api.request_format)
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    // 回退到 rig（支持多模态）
    let original = prepared.clone();
    let rig_result = call_model_openai_rig_style(api_config, model_name, prepared).await;
    match rig_result {
        Ok(reply) => Ok(reply),
        Err(err)
            if !original.latest_images.is_empty() && is_image_unsupported_error(&err) =>
        {
            eprintln!(
                "[CHAT] Model rejected image input, fallback to text-only request. error={}",
                err
            );
            let mut fallback = original;
            fallback.latest_images.clear();
            fallback.latest_audios.clear();
            call_model_openai_stream_text(api_config, model_name, &fallback, on_delta).await
        }
        Err(err) => Err(err),
    }
}

async fn describe_image_with_vision_api(
    vision_resolved: &ResolvedApiConfig,
    vision_api: &ApiConfig,
    image: &BinaryPart,
) -> Result<String, String> {
    if !is_openai_style_request_format(&vision_resolved.request_format) {
        return Err(format!(
            "Vision request format '{}' is not implemented yet.",
            vision_resolved.request_format
        ));
    }

    let mime = image.mime.trim();
    let prepared = PreparedPrompt {
        preamble: "[SYSTEM PROMPT]\n你是图像理解助手。请读取图片中的关键信息并输出简洁中文描述，保留有价值的文本、数字、UI元素与上下文。".to_string(),
        history_messages: Vec::new(),
        latest_user_text: "请识别这张图片并给出可用于后续对话的文本描述。".to_string(),
        latest_user_system_text: String::new(),
        latest_images: vec![(
            if mime.is_empty() {
                "image/png".to_string()
            } else {
                mime.to_string()
            },
            image.bytes_base64.clone(),
        )],
        latest_audios: Vec::new(),
    };

    let reply = call_model_openai_rig_style(vision_resolved, &vision_api.model, prepared).await?;
    Ok(reply.assistant_text)
}


