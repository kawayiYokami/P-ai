async fn call_model_deepseek_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    let (chat_history, current_prompt) = build_openai_rig_prompt(&prepared)?;
    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.trim().is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI-compatible client via rig: {err}"))?;

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
        .map_err(|err| format!("rig openai-compatible stream completion build failed: {err}"))?
        .stream()
        .await
        .map_err(|err| format!("rig openai-compatible stream start failed: {err}"))?;
    collect_streaming_model_reply(&mut stream, on_delta).await
}

fn deepseek_openai_api_base(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "https://api.deepseek.com/v1".to_string();
    }
    let base = trimmed.trim_end_matches('/');
    if base.ends_with("/v1") {
        base.to_string()
    } else {
        format!("{base}/v1")
    }
}

fn deepseek_messages_from_prepared(prepared: &PreparedPrompt) -> Vec<Value> {
    let mut messages = Vec::<Value>::new();
    if !prepared.preamble.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prepared.preamble
        }));
    }
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let mut content = Vec::<Value>::new();
            if !hm.text.trim().is_empty() {
                content.push(serde_json::json!({
                    "type": "text",
                    "text": hm.text.clone()
                }));
            }
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": time_text.clone()
                    }));
                }
            }
            messages.push(serde_json::json!({
                "role": "user",
                "content": content
            }));
        } else if hm.role == "assistant" && hm.tool_calls.is_some() {
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
                let normalized: Vec<Value> = calls
                    .iter()
                    .map(|call| {
                        let mut c = call.clone();
                        if let Some(func) = c.get_mut("function") {
                            if let Some(args) = func.get("arguments") {
                                if !args.is_string() {
                                    let s = args.to_string();
                                    if let Some(obj) = func.as_object_mut() {
                                        obj.insert(
                                            "arguments".to_string(),
                                            Value::String(s),
                                        );
                                    }
                                }
                            }
                        }
                        c
                    })
                    .collect();
                msg.insert("tool_calls".to_string(), Value::Array(normalized));
            }
            messages.push(Value::Object(msg));
        } else if hm.role == "assistant" {
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": hm.text
            }));
        } else if hm.role == "tool" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("tool".to_string()));
            msg.insert("content".to_string(), Value::String(hm.text.clone()));
            if let Some(call_id) = &hm.tool_call_id {
                if !call_id.trim().is_empty() {
                    msg.insert("tool_call_id".to_string(), Value::String(call_id.clone()));
                }
            }
            messages.push(Value::Object(msg));
        }
    }
    let latest_blocks = prepared_prompt_latest_user_text_blocks(prepared);
    if !latest_blocks.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": latest_blocks
                .into_iter()
                .map(|text| serde_json::json!({
                    "type": "text",
                    "text": text
                }))
                .collect::<Vec<_>>()
        }));
    }
    messages
}

fn latest_assistant_reasoning_since_last_user_openai(messages: &[Value]) -> Option<String> {
    for msg in messages.iter().rev() {
        let role = msg
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if role == "user" {
            break;
        }
        if role != "assistant" {
            continue;
        }
        let reasoning = msg
            .get("reasoning_content")
            .and_then(Value::as_str)
            .or_else(|| msg.get("reasoning").and_then(Value::as_str))
            .unwrap_or_default()
            .trim()
            .to_string();
        if !reasoning.is_empty() {
            return Some(reasoning);
        }
    }
    None
}

fn resolve_reasoning_for_deepseek_tool_assistant(
    current_reasoning: &str,
    messages: &[Value],
) -> String {
    let trimmed = current_reasoning.trim();
    if !trimmed.is_empty() {
        return current_reasoning.to_string();
    }
    if let Some(inherited) = latest_assistant_reasoning_since_last_user_openai(messages) {
        return inherited;
    }
    " ".to_string()
}

async fn deepseek_tool_definition_json(tool: &dyn ToolDyn) -> Value {
    let def = tool.definition(String::new()).await;
    serde_json::json!({
        "type": "function",
        "function": {
            "name": def.name,
            "description": def.description,
            "parameters": def.parameters
        }
    })
}

fn deepseek_tool_arguments_wire_value(args_text: &str) -> Value {
    Value::String(args_text.to_string())
}

async fn call_model_deepseek_with_tools(
    api_config: &ResolvedApiConfig,
    _selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let api_base = deepseek_openai_api_base(&api_config.base_url);
    let config = async_openai::config::OpenAIConfig::new()
        .with_api_key(&api_config.api_key)
        .with_api_base(api_base);
    let client = async_openai::Client::with_config(config);

    let mut messages = deepseek_messages_from_prepared(&prepared);
    if messages.is_empty() {
        return Err("Request payload is empty. Provide text, image, or audio.".to_string());
    }
    let mut tool_defs = Vec::<Value>::new();
    let mut tool_map = std::collections::HashMap::<String, usize>::new();
    for (idx, tool) in tool_assembly.tools.iter().enumerate() {
        tool_defs.push(deepseek_tool_definition_json(tool.as_ref()).await);
        tool_map.insert(tool.name(), idx);
    }

    let max_rounds = std::cmp::max(1usize, max_tool_iterations);
    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut trusted_input_tokens: Option<u64> = None;

    for _ in 0..max_rounds {
        let req = serde_json::json!({
            "model": model_name,
            "messages": messages,
            "tools": tool_defs,
            "temperature": api_config.temperature,
            "stream": true,
            "stream_options": {
                "include_usage": true
            }
        });
        let mut stream: std::pin::Pin<
            Box<
                dyn futures_util::Stream<
                        Item = Result<Value, async_openai::error::OpenAIError>,
                    > + Send,
            >,
        > = client
            .chat()
            .create_stream_byot(req)
            .await
            .map_err(|err| format!("async-openai deepseek tool loop failed: {err}"))?;
        let mut turn_text = String::new();
        let mut turn_reasoning = String::new();
        let mut tool_call_chunks =
            std::collections::BTreeMap::<usize, (String, String, String)>::new();

        while let Some(item) = stream.next().await {
            let chunk = item.map_err(|err| format!("async-openai deepseek stream failed: {err}"))?;
            if let Some(usage) = chunk.get("usage").and_then(Value::as_object) {
                let prompt_tokens = usage
                    .get("prompt_tokens")
                    .and_then(Value::as_u64)
                    .filter(|value| *value > 0);
                if prompt_tokens.is_some() {
                    trusted_input_tokens = prompt_tokens;
                }
            }
            let Some(choice0) = chunk
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
            else {
                continue;
            };
            let delta = choice0.get("delta").cloned().unwrap_or(Value::Null);
            if let Some(text_piece) = delta.get("content").and_then(Value::as_str) {
                if !text_piece.is_empty() {
                    turn_text.push_str(text_piece);
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text_piece.to_string(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        tool_args: None,
                        message: None,
                    });
                }
            }
            let reasoning_piece = delta
                .get("reasoning_content")
                .and_then(Value::as_str)
                .or_else(|| delta.get("reasoning").and_then(Value::as_str))
                .unwrap_or_default();
            if !reasoning_piece.is_empty() {
                turn_reasoning.push_str(reasoning_piece);
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: reasoning_piece.to_string(),
                    kind: Some("reasoning_standard".to_string()),
                    tool_name: None,
                    tool_status: None,
                    tool_args: None,
                    message: None,
                });
            }
            if let Some(delta_tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for tc in delta_tool_calls {
                    let idx = tc
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize;
                    let entry = tool_call_chunks
                        .entry(idx)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = tc.get("id").and_then(Value::as_str) {
                        if !id.is_empty() {
                            entry.0 = id.to_string();
                        }
                    }
                    if let Some(name) = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                    {
                        if !name.is_empty() {
                            if entry.1.is_empty() {
                                entry.1 = name.to_string();
                            } else {
                                entry.1.push_str(name);
                            }
                        }
                    }
                    if let Some(args_piece) = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(Value::as_str)
                    {
                        if !args_piece.is_empty() {
                            entry.2.push_str(args_piece);
                        }
                    }
                }
            }
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }
        if !turn_reasoning.is_empty() {
            if !full_reasoning_standard.is_empty() {
                full_reasoning_standard.push('\n');
            }
            full_reasoning_standard.push_str(&turn_reasoning);
        }

        let mut tool_calls = Vec::<Value>::new();
        for (_, (id, name, args_text)) in tool_call_chunks {
            let args_value = deepseek_tool_arguments_wire_value(&args_text);
            tool_calls.push(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": args_value
                }
            }));
        }
        if tool_calls.is_empty() {
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": turn_text,
                "reasoning_content": turn_reasoning
            }));
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }

        let reasoning_for_tool_assistant =
            resolve_reasoning_for_deepseek_tool_assistant(&turn_reasoning, &messages);
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": turn_text,
            "tool_calls": tool_calls,
            "reasoning_content": reasoning_for_tool_assistant
        }));

        for tool_call in tool_calls {
            let tool_call_id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let tool_name = tool_call
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let args_value = tool_call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .cloned()
                .unwrap_or_else(|| Value::String("{}".to_string()));
            let args_str = match &args_value {
                Value::String(raw) => raw.clone(),
                other => other.to_string(),
            };

            send_tool_status_event(
                on_delta,
                &tool_name,
                "running",
                Some(args_str.as_str()),
                &format!("正在调用工具：{}", tool_name),
            );
            let tool_result = if let Some(idx) = tool_map.get(&tool_name) {
                match call_tool_with_user_abort(
                    tool_abort_state,
                    chat_session_key,
                    tool_assembly.tools[*idx].call(args_str.clone()),
                )
                .await
                {
                    Ok(output) => {
                        send_tool_status_event(
                            on_delta,
                            &tool_name,
                            "done",
                            None,
                            &format!("工具调用完成：{}", tool_name),
                        );
                        output
                    }
                    Err(err) => {
                        if err == CHAT_ABORTED_BY_USER_ERROR {
                            eprintln!(
                                "[INFO][CHAT] stop requested during deepseek tool call; aborting turn (session={})",
                                chat_session_key
                            );
                            return Err(err);
                        }
                        let err_text = err.to_string();
                        send_tool_status_event(
                            on_delta,
                            &tool_name,
                            "failed",
                            None,
                            &format!("工具调用失败：{} ({})", tool_name, err_text),
                        );
                        serde_json::json!({
                            "ok": false,
                            "tool": tool_name,
                            "error": err_text
                        })
                        .to_string()
                    }
                }
            } else {
                let err_text = format!("Tool not found in runtime assembly: {tool_name}");
                send_tool_status_event(
                    on_delta,
                    &tool_name,
                    "failed",
                    None,
                    &format!("工具调用失败：{} ({})", tool_name, err_text),
                );
                serde_json::json!({
                    "ok": false,
                    "tool": tool_name,
                    "error": err_text
                })
                .to_string()
            };

            tool_history_events.push(serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [
                    {
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "arguments": args_str
                        }
                    }
                ]
            }));
            let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result);
            tool_history_events.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": history_content
            }));

            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result);
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": tool_result_for_model
            }));

            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        mime: payload.mime.clone(),
                        base64: payload.base64.clone(),
                        width: payload.width,
                        height: payload.height,
                        created_seq: 0,
                    },
                );
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": [
                        {"type": "text", "text": notice},
                        {"type": "image_url", "image_url": {"url": format!("data:{};base64,{}", cached.mime, cached.base64)}}
                    ]
                }));
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotWidth": cached.width,
                    "screenshotHeight": cached.height
                }));
            }
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        None,
        "工具调用达到上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}

#[cfg(test)]
mod deepseek_tool_call_tests {
    use super::*;

    #[test]
    fn deepseek_tool_call_arguments_are_serialized_as_string() {
        let args = r#"{"query":"test"}"#;
        let wire = deepseek_tool_arguments_wire_value(args);
        assert!(wire.is_string());
        assert_eq!(wire.as_str().unwrap_or_default(), args);
    }

    #[test]
    fn deepseek_messages_include_tool_history_events() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    user_time_text: None,
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "xcap", "arguments": "{\"method\":\"list_windows\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("thinking".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true}".to_string(),
                    user_time_text: None,
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "next".to_string(),
            latest_user_time_text: String::new(),
            latest_user_system_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = deepseek_messages_from_prepared(&prepared);
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("assistant")
                && m.get("tool_calls").and_then(Value::as_array).is_some()
        }));
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("tool")
                && m.get("tool_call_id").and_then(Value::as_str) == Some("call_1")
        }));
    }
}
