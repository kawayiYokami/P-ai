#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
    tool_history_events: Vec<Value>,
    suppress_assistant_message: bool,
    trusted_input_tokens: Option<u64>,
}

fn prepared_history_to_rig_messages(
    prepared: &PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
) -> Result<Vec<RigMessage>, String> {
    let mut chat_history = Vec::<RigMessage>::new();
    let mut tool_call_id_to_call_id = std::collections::HashMap::<String, String>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let base_user_text = if hm.text.trim().is_empty() {
                " ".to_string()
            } else {
                hm.text.clone()
            };
            let mut user_blocks = vec![UserContent::text(base_user_text)];
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
                for call in normalize_prompt_tool_calls(tool_calls) {
                    let Some(id) = call.invocation_id.as_deref().map(str::trim) else {
                        continue;
                    };
                    let Some(name) = call.tool_name.as_deref().map(str::trim) else {
                        continue;
                    };
                    if matches!(
                        tool_call_replay_capability(protocol_family, &call),
                        StructuredToolReplayCapability::Invalid | StructuredToolReplayCapability::TextOnly
                    ) {
                        continue;
                    }
                    let call_id = call
                        .provider_call_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned);
                    if let Some(call_id_value) = call_id.clone() {
                        tool_call_id_to_call_id.insert(id.to_string(), call_id_value);
                    }
                    let tool_call = rig::message::ToolCall {
                        id: id.to_string(),
                        call_id,
                        function: rig::message::ToolFunction {
                            name: name.to_string(),
                            arguments: call.arguments_value.clone(),
                        },
                        signature: None,
                        additional_params: None,
                    };
                    assistant_blocks.push(AssistantContent::ToolCall(tool_call));
                }
            }
            if assistant_blocks.is_empty() {
                assistant_blocks.push(AssistantContent::text(" ".to_string()));
            }
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(assistant_blocks)
                    .map_err(|_| "Failed to build assistant history message".to_string())?,
            });
        } else if hm.role == "tool" {
            let safe_tool_text = if hm.text.trim().is_empty() {
                " ".to_string()
            } else {
                hm.text.clone()
            };
            let result_content = OneOrMany::one(ToolResultContent::text(safe_tool_text.clone()));
            let tool_user_content = if let Some(tool_call_id) = hm
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
            {
                let provider_call_id = tool_call_id_to_call_id.get(tool_call_id).cloned();
                match tool_result_replay_capability(
                    protocol_family,
                    tool_call_id,
                    provider_call_id.as_deref(),
                ) {
                    StructuredToolReplayCapability::Structured => {
                        if let Some(call_id) = provider_call_id {
                            UserContent::tool_result_with_call_id(
                                tool_call_id.to_string(),
                                call_id,
                                result_content,
                            )
                        } else {
                            UserContent::tool_result(tool_call_id.to_string(), result_content)
                        }
                    }
                    StructuredToolReplayCapability::TextOnly
                    | StructuredToolReplayCapability::Invalid => UserContent::text(safe_tool_text),
                }
            } else {
                UserContent::text(safe_tool_text)
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
    fn should_replay_structured_tool_history_when_call_id_is_present() {
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
                        "id": "fc_1",
                        "call_id": "call_1",
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
                    tool_call_id: Some("fc_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let chat_history = prepared_history_to_rig_messages(
            &prepared,
            ToolCallProtocolFamily::OpenAiResponses,
        )
        .expect("history built");
        let mut saw_tool_result = false;
        let mut saw_assistant_tool_call = false;

        for message in &chat_history {
            match message {
                RigMessage::System { .. } => {}
                RigMessage::Assistant { content, .. } => {
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            AssistantContent::ToolCall(call)
                                if call.id == "fc_1"
                                    && call.call_id.as_deref() == Some("call_1")
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
                                if result.id == "fc_1"
                                    && result.call_id.as_deref() == Some("call_1")
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

    #[test]
    fn should_downgrade_legacy_tool_history_without_call_id_to_plain_text() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_legacy_1",
                        "type": "function",
                        "function": { "name": "xcap", "arguments": "{\"method\":\"capture_focused_window\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"method\":\"capture_focused_window\"}".to_string(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_legacy_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let chat_history = prepared_history_to_rig_messages(
            &prepared,
            ToolCallProtocolFamily::OpenAiResponses,
        )
        .expect("history built");
        assert!(chat_history.iter().any(|message| {
            matches!(
                message,
                RigMessage::Assistant { content, .. }
                    if content.iter().all(|item| matches!(item, AssistantContent::Text(_)))
            )
        }));
        assert!(chat_history.iter().any(|message| {
            matches!(
                message,
                RigMessage::User { content }
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            UserContent::Text(text)
                                if text.text.contains("capture_focused_window")
                        )
                    })
            )
        }));
    }

    #[test]
    fn should_keep_legacy_tool_history_structured_for_chat_like_protocols() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_legacy_1",
                        "type": "function",
                        "function": { "name": "xcap", "arguments": "{\"method\":\"capture_focused_window\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"method\":\"capture_focused_window\"}".to_string(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_legacy_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let chat_history = prepared_history_to_rig_messages(
            &prepared,
            ToolCallProtocolFamily::OpenAiChatLike,
        )
        .expect("history built");
        assert!(chat_history.iter().any(|message| {
            matches!(
                message,
                RigMessage::Assistant { content, .. }
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            AssistantContent::ToolCall(call)
                                if call.id == "call_legacy_1"
                                    && call.call_id.is_none()
                                    && call.function.name == "xcap"
                        )
                    })
            )
        }));
        assert!(chat_history.iter().any(|message| {
            matches!(
                message,
                RigMessage::User { content }
                    if content.iter().any(|item| {
                        matches!(
                            item,
                            UserContent::ToolResult(result)
                                if result.id == "call_legacy_1"
                                    && result.call_id.is_none()
                        )
                    })
            )
        }));
    }
}

#[derive(Debug, Clone, Copy)]
enum OpenAiRigApiKind {
    ChatCompletions,
    Responses,
}

fn build_openai_rig_prompt(
    prepared: &PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
) -> Result<(Vec<RigMessage>, RigMessage), String> {
    let chat_history = prepared_history_to_rig_messages(prepared, protocol_family)?;
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
    let protocol_family = match kind {
        OpenAiRigApiKind::ChatCompletions => ToolCallProtocolFamily::OpenAiChatLike,
        OpenAiRigApiKind::Responses => ToolCallProtocolFamily::OpenAiResponses,
    };
    let (chat_history, current_prompt) = build_openai_rig_prompt(&prepared, protocol_family)?;
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
            let agent_builder = client.completions_api().agent(model_name);
            let agent_builder = if prepared.preamble.trim().is_empty() {
                agent_builder
            } else {
                agent_builder.preamble(&prepared.preamble)
            };
            let agent = agent_builder
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
            let agent_builder = client.agent(model_name);
            let agent_builder = if prepared.preamble.trim().is_empty() {
                agent_builder
            } else {
                agent_builder.preamble(&prepared.preamble)
            };
            let agent = agent_builder
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

fn openai_non_stream_extract_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => {
            let mut blocks = Vec::<String>::new();
            for item in items {
                let Some(obj) = item.as_object() else {
                    continue;
                };
                let block_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
                if block_type != "text" {
                    continue;
                }
                let text = obj
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        obj.get("text")
                            .and_then(Value::as_object)
                            .and_then(|v| v.get("value"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .map(ToOwned::to_owned)
                    });
                if let Some(text) = text {
                    blocks.push(text);
                }
            }
            blocks.join("\n")
        }
        _ => String::new(),
    }
}

async fn call_model_openai_non_stream_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let mut request =
        prepared_prompt_to_equivalent_request_json(&prepared, model_name, api_config.temperature);
    let request_obj = request
        .as_object_mut()
        .ok_or_else(|| "Invalid request payload".to_string())?;
    request_obj.insert("stream".to_string(), Value::Bool(false));
    let base_url = if api_config.base_url.trim().is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        api_config.base_url.trim().trim_end_matches('/').to_string()
    };
    let endpoint = format!("{base_url}/chat/completions");
    let response = reqwest::Client::new()
        .post(&endpoint)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", api_config.api_key))
        .json(&request)
        .send()
        .await
        .map_err(|err| format!("openai non-stream request failed: {err}"))?;
    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|err| format!("openai non-stream read body failed: {err}"))?;
    if !status.is_success() {
        let snippet = response_text.chars().take(600).collect::<String>();
        return Err(format!(
            "openai non-stream request failed: status={} body={}",
            status.as_u16(),
            snippet
        ));
    }
    let payload: Value = serde_json::from_str(&response_text)
        .map_err(|err| format!("openai non-stream parse failed: {err}"))?;
    let first_choice = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "openai non-stream response missing choices[0]".to_string())?;
    let message = first_choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| "openai non-stream response missing choices[0].message".to_string())?;
    let assistant_text = message
        .get("content")
        .map(openai_non_stream_extract_text)
        .unwrap_or_default();
    let reasoning_standard = message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .map(openai_non_stream_extract_text)
        .unwrap_or_default();
    let trusted_input_tokens = payload
        .get("usage")
        .and_then(Value::as_object)
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0);
    Ok(ModelReply {
        assistant_text,
        reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
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
    let chat_history =
        prepared_history_to_rig_messages(&prepared, ToolCallProtocolFamily::Gemini)?;
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
    let chat_history =
        prepared_history_to_rig_messages(&prepared, ToolCallProtocolFamily::Anthropic)?;
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
