async fn call_model_openai_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<String, String> {
    let mut content_items: Vec<UserContent> = vec![UserContent::text(prepared.preamble)];

    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text));
    }

    for (mime, bytes) in prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes,
            image_media_type_from_mime(&mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in prepared.latest_audios {
        content_items.push(UserContent::audio(bytes, audio_media_type_from_mime(&mime)));
    }

    let prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;

    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;

    let agent = client.completions_api().agent(model_name).build();
    let prompt_message = RigMessage::User {
        content: prompt_content,
    };

    agent
        .prompt(prompt_message)
        .await
        .map_err(|err| format!("rig prompt failed: {err}"))
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<invalid json>".to_string()),
    }
}

fn build_openai_tools_payload(tools: &[McpExposedTool]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
              "type": "function",
              "function": {
                "name": tool.openai_name,
                "description": tool.description.clone().unwrap_or_default(),
                "parameters": tool.input_schema,
              }
            })
        })
        .collect()
}

fn build_builtin_tools_payload(selected_api: &ApiConfig) -> Vec<McpExposedTool> {
    if !selected_api.enable_tools {
        return Vec::new();
    }
    selected_api
    .tools
    .iter()
    .filter_map(|tool| {
      if tool.id == "fetch" {
        return Some(McpExposedTool {
          openai_name: "fetch".to_string(),
          mcp_name: "fetch".to_string(),
          description: Some("Fetch webpage text.".to_string()),
          input_schema: serde_json::json!({
            "type": "object",
            "properties": {
              "url": { "type": "string", "description": "URL" },
              "max_length": { "type": "integer", "description": "Max chars", "default": 1800 }
            },
            "required": ["url"]
          }),
        });
      }
      if tool.id == "bing-search" {
        return Some(McpExposedTool {
          openai_name: "bing_search".to_string(),
          mcp_name: "bing-search".to_string(),
          description: Some("Search web with Bing.".to_string()),
          input_schema: serde_json::json!({
            "type": "object",
            "properties": {
              "query": { "type": "string", "description": "Query" },
              "num_results": { "type": "integer", "description": "Result count", "default": 5 }
            },
            "required": ["query"]
          }),
        });
      }
      None
    })
    .collect()
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn builtin_fetch(url: &str, max_length: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .send()
        .await
        .map_err(|err| format!("Fetch url failed: {err}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Fetch url failed with status {status}"));
    }
    let html = resp
        .text()
        .await
        .map_err(|err| format!("Read body failed: {err}"))?;
    let document = Html::parse_document(&html);
    let body_selector =
        Selector::parse("body").map_err(|err| format!("Parse selector failed: {err}"))?;
    let raw = document
        .select(&body_selector)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "));
    let cleaned = clean_text(&raw);
    let truncated = if cleaned.len() > max_length {
        format!("{}...", &cleaned[..max_length])
    } else {
        cleaned
    };
    Ok(serde_json::json!({
      "url": url,
      "content": truncated
    }))
}

async fn builtin_bing_search(query: &str, num_results: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let mut last_error: Option<String> = None;
    for base in ["https://cn.bing.com", "https://www.bing.com"] {
        let url = format!("{base}/search?q={}", urlencoding::encode(query));
        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
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
        let item_sel =
            Selector::parse("li.b_algo").map_err(|err| format!("Parse selector failed: {err}"))?;
        let title_sel =
            Selector::parse("h2").map_err(|err| format!("Parse selector failed: {err}"))?;
        let a_sel =
            Selector::parse("h2 a").map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_sel = Selector::parse("p").map_err(|err| format!("Parse selector failed: {err}"))?;
        let mut rows = Vec::new();
        for item in doc.select(&item_sel).take(num_results.max(1)) {
            let title = item
                .select(&title_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            let link = item
                .select(&a_sel)
                .next()
                .and_then(|n| n.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            let snippet = item
                .select(&p_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            if !title.is_empty() && !link.is_empty() {
                rows.push(serde_json::json!({"title": title, "url": link, "snippet": snippet}));
            }
        }
        if !rows.is_empty() {
            return Ok(serde_json::json!({"query": query, "results": rows}));
        }
        last_error = Some("no results parsed".to_string());
    }
    Err(format!(
        "bing search failed: {}",
        last_error.unwrap_or_else(|| "unknown".to_string())
    ))
}

async fn execute_builtin_tool(name: &str, args: Value) -> Result<Value, String> {
    match name {
        "fetch" => {
            let url = args
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| "fetch.url is required".to_string())?;
            let max_length = args
                .get("max_length")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(1800);
            builtin_fetch(url, max_length).await
        }
        "bing-search" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| "bing_search.query is required".to_string())?;
            let num = args
                .get("num_results")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(5);
            builtin_bing_search(query, num).await
        }
        _ => Err(format!("Unsupported builtin tool: {name}")),
    }
}

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

/// 通用 OpenAI SSE 流式请求：解析文本 delta（实时推送到 on_delta）和 tool_calls 积累。
/// 返回 (完整文本, 积累的 tool_calls)。
async fn openai_stream_request(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(String, Vec<OpenAIToolCall>), String> {
    openai_stream_request_with_sink(client, url, body, |delta| {
        let send_result = on_delta.send(AssistantDeltaEvent {
            delta: delta.to_string(),
        });
        eprintln!(
            "[STREAM-DEBUG] on_delta.send result: {:?}, delta_len={}",
            send_result,
            delta.len()
        );
    })
    .await
}

async fn openai_stream_request_with_sink<F>(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    mut on_delta: F,
) -> Result<(String, Vec<OpenAIToolCall>), String>
where
    F: FnMut(&str),
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

            // 处理文本 delta
            let delta_text = parse_stream_delta_text(&choice.delta.content);
            if !delta_text.is_empty() {
                output.push_str(&delta_text);
                on_delta(&delta_text);
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

    eprintln!(
        "[STREAM-DEBUG] stream done: output_len={}, tool_calls_count={}",
        output.len(),
        tool_calls.len()
    );
    Ok((output, tool_calls))
}

async fn call_model_openai_stream_text(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: &PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(openai_headers(&api_config.api_key)?)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let body = serde_json::json!({
      "model": model_name,
      "messages": [
        { "role": "system", "content": prepared.preamble },
        { "role": "user", "content": prepared.latest_user_text }
      ],
      "stream": true
    });

    let urls = candidate_openai_chat_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    let mut errors = Vec::new();
    for url in urls {
        match openai_stream_request(&client, &url, body.clone(), on_delta).await {
            Ok((text, _)) => return Ok(text),
            Err(err) => errors.push(format!("{url} -> {err}")),
        }
    }

    Err(format!(
        "OpenAI stream request failed for all candidate URLs: {}",
        errors.join(" || ")
    ))
}

async fn call_model_openai_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    let exposed_tools = build_builtin_tools_payload(selected_api);
    if exposed_tools.is_empty() {
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

    let mut messages = vec![
        serde_json::json!({"role":"system","content": prepared.preamble}),
        serde_json::json!({"role":"user","content": prepared.latest_user_text}),
    ];
    let mut full_assistant_text = String::new();
    let tools_payload = build_openai_tools_payload(&exposed_tools);

    for _ in 0..4 {
        let body = serde_json::json!({
          "model": model_name,
          "messages": messages,
          "tools": tools_payload,
          "tool_choice": "auto",
          "temperature": 0.2,
          "stream": true
        });

        let mut stream_result: Option<(String, Vec<OpenAIToolCall>)> = None;
        let mut stream_errors = Vec::new();
        for url in &urls {
            match openai_stream_request(&client, url, body.clone(), on_delta).await {
                Ok(ok) => {
                    stream_result = Some(ok);
                    break;
                }
                Err(err) => stream_errors.push(format!("{url} -> {err}")),
            }
        }
        let (text, tool_calls) = stream_result.ok_or_else(|| {
            format!(
                "OpenAI tool stream request failed for all candidate URLs: {}",
                stream_errors.join(" || ")
            )
        })?;
        if !text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&text);
        }

        if tool_calls.is_empty() {
            return Ok(full_assistant_text);
        }

        // 有 tool_calls：积累到 messages 中，继续循环
        let assistant_tool_calls = tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                  "id": tc.id,
                  "type": "function",
                  "function": {
                    "name": tc.function.name,
                    "arguments": tc.function.arguments,
                  }
                })
            })
            .collect::<Vec<_>>();
        let content_value = if text.is_empty() {
            Value::Null
        } else {
            Value::String(text)
        };
        messages.push(serde_json::json!({
          "role":"assistant",
          "content": content_value,
          "tool_calls": assistant_tool_calls
        }));

        for tc in &tool_calls {
            let Some(exposed) = exposed_tools
                .iter()
                .find(|tool| tool.openai_name == tc.function.name)
            else {
                messages.push(serde_json::json!({
                  "role":"tool",
                  "tool_call_id": tc.id,
                  "content": format!("Tool '{}' is not registered.", tc.function.name)
                }));
                continue;
            };

            let args = if tc.function.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str::<Value>(&tc.function.arguments).map_err(|err| {
                    format!("Parse tool arguments failed ({}): {err}", tc.function.name)
                })?
            };

            let result_value = execute_builtin_tool(&exposed.mcp_name, args).await?;
            let tool_text = value_to_text(&result_value);
            messages.push(serde_json::json!({
              "role":"tool",
              "tool_call_id": tc.id,
              "content": tool_text
            }));
        }
    }

    Err("Tool call exceeded max iterations.".to_string())
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    eprintln!(
        "[STREAM-DEBUG] call_model_openai_style: format={}, enable_tools={}, images={}, audios={}",
        selected_api.request_format,
        selected_api.enable_tools,
        prepared.latest_images.len(),
        prepared.latest_audios.len()
    );
    // 优先使用工具调用（如果启用）
    if selected_api.enable_tools
        && selected_api.request_format.trim() == "openai"
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_with_tools(
            api_config,
            selected_api,
            model_name,
            prepared,
            on_delta,
        )
        .await;
    }

    // 纯文本流式传输（无论工具是否启用，只要没有工具调用就走流式）
    if selected_api.request_format.trim() == "openai"
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    // 回退到 rig（支持多模态）
    call_model_openai_rig_style(api_config, model_name, prepared).await
}


