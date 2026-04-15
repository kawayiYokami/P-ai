#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
    assistant_provider_meta: Option<Value>,
    tool_history_events: Vec<Value>,
    suppress_assistant_message: bool,
    trusted_input_tokens: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum OpenAiApiKind {
    ChatCompletions,
    Responses,
}

fn normalize_openai_genai_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://api.openai.com/v1/".to_string()
    } else {
        format!("{trimmed}/")
    }
}

fn genai_content_parts_from_text_and_binary(
    text_blocks: &[String],
    images: &[PreparedBinaryPayload],
    audios: &[PreparedBinaryPayload],
) -> Vec<genai::chat::ContentPart> {
    let mut parts = Vec::<genai::chat::ContentPart>::new();
    for text in text_blocks {
        parts.push(genai::chat::ContentPart::from_text(text.clone()));
    }
    for image in images {
        if is_remote_binary_url(&image.content) {
            parts.push(genai::chat::ContentPart::from_binary_url(
                image.mime.clone(),
                image.content.clone(),
                None,
            ));
        } else {
            parts.push(genai::chat::ContentPart::from_binary_base64(
                image.mime.clone(),
                image.content.clone(),
                None,
            ));
        }
    }
    for audio in audios {
        if is_remote_binary_url(&audio.content) {
            parts.push(genai::chat::ContentPart::from_binary_url(
                audio.mime.clone(),
                audio.content.clone(),
                None,
            ));
        } else {
            parts.push(genai::chat::ContentPart::from_binary_base64(
                audio.mime.clone(),
                audio.content.clone(),
                None,
            ));
        }
    }
    parts
}

fn genai_tool_call_id_for_history(
    protocol_family: ToolCallProtocolFamily,
    call: &NormalizedToolCallRecord,
) -> Option<String> {
    match protocol_family {
        ToolCallProtocolFamily::OpenAiResponses => call
            .provider_call_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        ToolCallProtocolFamily::OpenAiChatLike
        | ToolCallProtocolFamily::Gemini
        | ToolCallProtocolFamily::Anthropic => call
            .invocation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    }
}

fn prepared_history_to_genai_messages(
    prepared: &PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
) -> Result<Vec<genai::chat::ChatMessage>, String> {
    let mut chat_history = Vec::<genai::chat::ChatMessage>::new();
    let mut tool_call_id_to_provider_call_id =
        std::collections::HashMap::<String, String>::new();
    for hm in &prepared.history_messages {
        if hm.role == "user" {
            let base_user_text = if hm.text.trim().is_empty() {
                " ".to_string()
            } else {
                hm.text.clone()
            };
            let mut text_blocks = vec![base_user_text];
            for block in &hm.extra_text_blocks {
                if !block.trim().is_empty() {
                    text_blocks.push(block.clone());
                }
            }
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    text_blocks.push(time_text.clone());
                }
            }
            let parts =
                genai_content_parts_from_text_and_binary(&text_blocks, &hm.images, &hm.audios);
            chat_history.push(genai::chat::ChatMessage::user(
                genai::chat::MessageContent::from_parts(parts),
            ));
        } else if hm.role == "assistant" {
            let mut assistant_parts = Vec::<genai::chat::ContentPart>::new();
            if !hm.text.trim().is_empty() {
                assistant_parts.push(genai::chat::ContentPart::from_text(hm.text.clone()));
            }
            if let Some(tool_calls) = &hm.tool_calls {
                for call in normalize_prompt_tool_calls(tool_calls) {
                    let Some(invocation_id) = call
                        .invocation_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    let Some(tool_name) = call
                        .tool_name
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        continue;
                    };
                    if matches!(
                        tool_call_replay_capability(protocol_family, &call),
                        StructuredToolReplayCapability::Invalid
                            | StructuredToolReplayCapability::TextOnly
                    ) {
                        continue;
                    }
                    if let Some(provider_call_id) = call
                        .provider_call_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        tool_call_id_to_provider_call_id
                            .insert(invocation_id.to_string(), provider_call_id.to_string());
                    }
                    let Some(call_id) = genai_tool_call_id_for_history(protocol_family, &call) else {
                        continue;
                    };
                    assistant_parts.push(genai::chat::ContentPart::ToolCall(
                        genai::chat::ToolCall {
                            call_id,
                            fn_name: tool_name.to_string(),
                            fn_arguments: call.arguments_value.clone(),
                            thought_signatures: None,
                        },
                    ));
                }
            }
            if assistant_parts.is_empty() {
                assistant_parts.push(genai::chat::ContentPart::from_text(" "));
            }
            let assistant_message = genai::chat::ChatMessage::assistant(
                genai::chat::MessageContent::from_parts(assistant_parts),
            )
            .with_reasoning_content(hm.reasoning_content.clone());
            chat_history.push(assistant_message);
        } else if hm.role == "tool" {
            let safe_tool_text = if hm.text.trim().is_empty() {
                " ".to_string()
            } else {
                hm.text.clone()
            };
            let Some(tool_call_id) = hm
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                chat_history.push(genai::chat::ChatMessage::user(safe_tool_text));
                continue;
            };
            let provider_call_id = tool_call_id_to_provider_call_id.get(tool_call_id).cloned();
            match tool_result_replay_capability(
                protocol_family,
                tool_call_id,
                provider_call_id.as_deref(),
            ) {
                StructuredToolReplayCapability::Structured => {
                    let response_call_id = match protocol_family {
                        ToolCallProtocolFamily::OpenAiResponses => {
                            provider_call_id.unwrap_or_else(|| tool_call_id.to_string())
                        }
                        ToolCallProtocolFamily::OpenAiChatLike
                        | ToolCallProtocolFamily::Gemini
                        | ToolCallProtocolFamily::Anthropic => tool_call_id.to_string(),
                    };
                    chat_history.push(genai::chat::ChatMessage::from(
                        genai::chat::ToolResponse::new(response_call_id, safe_tool_text),
                    ));
                }
                StructuredToolReplayCapability::TextOnly
                | StructuredToolReplayCapability::Invalid => {
                    chat_history.push(genai::chat::ChatMessage::user(safe_tool_text));
                }
            }
        }
    }
    Ok(chat_history)
}

fn build_openai_responses_genai_request(
    prepared: &PreparedPrompt,
) -> Result<genai::chat::ChatRequest, String> {
    let history_messages =
        prepared_history_to_genai_messages(prepared, ToolCallProtocolFamily::OpenAiResponses)?;
    let latest_parts = genai_content_parts_from_text_and_binary(
        &prepared_prompt_latest_user_text_blocks(prepared),
        &prepared.latest_images,
        &prepared.latest_audios,
    );
    let mut request = genai::chat::ChatRequest::from_messages(history_messages).append_message(
        genai::chat::ChatMessage::user(genai::chat::MessageContent::from_parts(
            latest_parts,
        )),
    );
    if !prepared.preamble.trim().is_empty() {
        request = request.with_system(prepared.preamble.clone());
    }
    Ok(request)
}

fn build_openai_chat_genai_request(
    prepared: &PreparedPrompt,
) -> Result<genai::chat::ChatRequest, String> {
    let history_messages =
        prepared_history_to_genai_messages(prepared, ToolCallProtocolFamily::OpenAiChatLike)?;
    let latest_parts = genai_content_parts_from_text_and_binary(
        &prepared_prompt_latest_user_text_blocks(prepared),
        &prepared.latest_images,
        &prepared.latest_audios,
    );
    let mut request = genai::chat::ChatRequest::from_messages(history_messages).append_message(
        genai::chat::ChatMessage::user(genai::chat::MessageContent::from_parts(
            latest_parts,
        )),
    );
    if !prepared.preamble.trim().is_empty() {
        request = request.with_system(prepared.preamble.clone());
    }
    Ok(request)
}

fn build_gemini_genai_request(
    prepared: &PreparedPrompt,
) -> Result<genai::chat::ChatRequest, String> {
    let history_messages =
        prepared_history_to_genai_messages(prepared, ToolCallProtocolFamily::Gemini)?;
    let latest_parts = genai_content_parts_from_text_and_binary(
        &prepared_prompt_latest_user_text_blocks(prepared),
        &prepared.latest_images,
        &prepared.latest_audios,
    );
    let mut request = genai::chat::ChatRequest::from_messages(history_messages).append_message(
        genai::chat::ChatMessage::user(genai::chat::MessageContent::from_parts(
            latest_parts,
        )),
    );
    if !prepared.preamble.trim().is_empty() {
        request = request.with_system(prepared.preamble.clone());
    }
    Ok(request)
}

fn build_anthropic_genai_request(
    prepared: &PreparedPrompt,
) -> Result<genai::chat::ChatRequest, String> {
    let history_messages =
        prepared_history_to_genai_messages(prepared, ToolCallProtocolFamily::Anthropic)?;
    let latest_parts = genai_content_parts_from_text_and_binary(
        &prepared_prompt_latest_user_text_blocks(prepared),
        &prepared.latest_images,
        &prepared.latest_audios,
    );
    let mut request = genai::chat::ChatRequest::from_messages(history_messages).append_message(
        genai::chat::ChatMessage::user(genai::chat::MessageContent::from_parts(
            latest_parts,
        )),
    );
    if !prepared.preamble.trim().is_empty() {
        request = request.with_system(prepared.preamble.clone());
    }
    Ok(request)
}

fn build_provider_genai_request(
    prepared: &PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
) -> Result<genai::chat::ChatRequest, String> {
    match protocol_family {
        ToolCallProtocolFamily::OpenAiResponses => build_openai_responses_genai_request(prepared),
        ToolCallProtocolFamily::OpenAiChatLike => build_openai_chat_genai_request(prepared),
        ToolCallProtocolFamily::Gemini => build_gemini_genai_request(prepared),
        ToolCallProtocolFamily::Anthropic => build_anthropic_genai_request(prepared),
    }
}

fn normalize_gemini_genai_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://generativelanguage.googleapis.com/v1beta/".to_string();
    }

    let without_openai = trimmed.trim_end_matches("/openai").trim_end_matches('/');
    let with_version = if without_openai.ends_with("/v1beta") {
        without_openai.to_string()
    } else {
        format!("{without_openai}/v1beta")
    };
    format!("{with_version}/")
}

fn normalize_anthropic_genai_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://api.anthropic.com/v1/".to_string()
    } else {
        let with_version = if trimmed.ends_with("/v1") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/v1")
        };
        format!("{with_version}/")
    }
}

fn normalize_provider_genai_base_url(
    adapter_kind: genai::adapter::AdapterKind,
    raw: &str,
) -> String {
    match adapter_kind {
        genai::adapter::AdapterKind::OpenAI | genai::adapter::AdapterKind::OpenAIResp => {
            normalize_openai_genai_base_url(raw)
        }
        genai::adapter::AdapterKind::Gemini => normalize_gemini_genai_base_url(raw),
        genai::adapter::AdapterKind::Anthropic => normalize_anthropic_genai_base_url(raw),
        _ => {
            let trimmed = raw.trim().trim_end_matches('/');
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{trimmed}/")
            }
        }
    }
}

fn provider_genai_headers(api_config: &ResolvedApiConfig) -> genai::Headers {
    let mut headers = app_identity_genai_headers();
    headers.merge(api_config.extra_headers.clone());
    headers
}

fn provider_genai_reasoning_effort(
    api_config: &ResolvedApiConfig,
) -> Option<genai::chat::ReasoningEffort> {
    api_config
        .reasoning_effort
        .as_deref()
        .and_then(|value| value.parse::<genai::chat::ReasoningEffort>().ok())
}

async fn resolve_request_api_config(
    api_config: &ResolvedApiConfig,
) -> Result<ResolvedApiConfig, String> {
    let Some(codex_auth) = &api_config.codex_auth else {
        return Ok(api_config.clone());
    };
    let fresh_auth = ensure_codex_runtime_auth_fresh(codex_auth).await?;
    let mut next = api_config.clone();
    next.api_key = fresh_auth.access_token.clone();
    next.codex_auth = Some(fresh_auth.clone());
    next.extra_headers.retain(|(key, _)| key != "ChatGPT-Account-Id");
    if let Some(account_id) = fresh_auth.account_id.as_deref().filter(|value| !value.is_empty()) {
        next.extra_headers
            .push(("ChatGPT-Account-Id".to_string(), account_id.to_string()));
    }
    Ok(next)
}

async fn call_model_openai_stream_internal(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    kind: OpenAiApiKind,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
    let adapter_kind = match kind {
        OpenAiApiKind::ChatCompletions => genai::adapter::AdapterKind::OpenAI,
        OpenAiApiKind::Responses => genai::adapter::AdapterKind::OpenAIResp,
    };
    let protocol_family = match kind {
        OpenAiApiKind::ChatCompletions => ToolCallProtocolFamily::OpenAiChatLike,
        OpenAiApiKind::Responses => ToolCallProtocolFamily::OpenAiResponses,
    };
    let request = build_provider_genai_request(&prepared, protocol_family)?;
    let service_target = genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
            adapter_kind,
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(adapter_kind, model_name),
    };
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_extra_headers(provider_genai_headers(&api_config));
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(&api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }

    let mut stream = client
        .exec_chat_stream(service_target, request, Some(&options))
        .await
        .map_err(|err| format!("genai openai stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, on_delta).await
}

async fn call_model_openai_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    call_model_openai_stream_internal(
        api_config,
        model_name,
        prepared,
        OpenAiApiKind::ChatCompletions,
        None,
    )
    .await
}

async fn call_model_openai_non_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
    let service_target = genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_openai_genai_base_url(
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(genai::adapter::AdapterKind::OpenAI, model_name),
    };
    let request = build_openai_chat_genai_request(&prepared)?;
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_extra_headers(provider_genai_headers(&api_config));
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(&api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    let response = client
        .exec_chat(service_target, request, Some(&options))
        .await
        .map_err(|err| format!("genai openai non-stream failed: {err}"))?;
    let assistant_text = response.content.into_texts().join("\n");
    let reasoning_standard = response.reasoning_content.unwrap_or_default();
    let trusted_input_tokens = response
        .usage
        .prompt_tokens
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0);
    Ok(ModelReply {
        assistant_text,
        reasoning_standard,
        reasoning_inline: String::new(),
        assistant_provider_meta: None,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}

async fn call_model_openai_responses(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
    let service_target = genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_openai_genai_base_url(
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(genai::adapter::AdapterKind::OpenAIResp, model_name),
    };
    let request = build_openai_responses_genai_request(&prepared)?;
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_extra_headers(provider_genai_headers(&api_config));
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(&api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    let mut stream = client
        .exec_chat_stream(service_target, request, Some(&options))
        .await
        .map_err(|err| format!("genai responses stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, on_delta).await
}

async fn call_model_gemini(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let request = build_gemini_genai_request(&prepared)?;
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
    let service_target = genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
            genai::adapter::AdapterKind::Gemini,
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(genai::adapter::AdapterKind::Gemini, model_name),
    };
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_extra_headers(provider_genai_headers(&api_config));
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(&api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    let response = client
        .exec_chat(service_target, request, Some(&options))
        .await
        .map_err(|err| format!("genai gemini non-stream failed: {err}"))?;
    Ok(ModelReply {
        assistant_text: response.content.into_texts().join("\n"),
        reasoning_standard: response.reasoning_content.unwrap_or_default(),
        reasoning_inline: String::new(),
        assistant_provider_meta: None,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens: response
            .usage
            .prompt_tokens
            .and_then(|value| u64::try_from(value).ok())
            .filter(|value| *value > 0),
    })
}

async fn call_model_anthropic(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
    let service_target = genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
            genai::adapter::AdapterKind::Anthropic,
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(genai::adapter::AdapterKind::Anthropic, model_name),
    };
    let request = build_anthropic_genai_request(&prepared)?;
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(true)
        .with_extra_headers(provider_genai_headers(&api_config));
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(&api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    let mut stream = client
        .exec_chat_stream(service_target, request, Some(&options))
        .await
        .map_err(|err| format!("genai anthropic stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, None).await
}

#[cfg(test)]
mod openai_responses_genai_request_tests {
    use super::*;

    #[test]
    fn build_openai_responses_genai_request_should_keep_system_at_top_level() {
        let prepared = PreparedPrompt {
            preamble: "你是系统提示".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "写一个快速排序".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_openai_responses_genai_request(&prepared)
            .expect("build_openai_responses_genai_request should succeed");

        assert_eq!(request.system.as_deref(), Some("你是系统提示"));
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::User
        ));
    }

    #[test]
    fn build_gemini_genai_request_should_keep_system_at_top_level() {
        let prepared = PreparedPrompt {
            preamble: "你是 Gemini 系统提示".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "分析这段代码".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request =
            build_gemini_genai_request(&prepared).expect("build_gemini_genai_request should succeed");

        assert_eq!(request.system.as_deref(), Some("你是 Gemini 系统提示"));
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::User
        ));
    }

    #[test]
    fn build_anthropic_genai_request_should_keep_system_at_top_level() {
        let prepared = PreparedPrompt {
            preamble: "你是 Claude 系统提示".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "总结这段日志".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_anthropic_genai_request(&prepared)
            .expect("build_anthropic_genai_request should succeed");

        assert_eq!(request.system.as_deref(), Some("你是 Claude 系统提示"));
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::User
        ));
    }

    #[test]
    fn normalize_anthropic_genai_base_url_should_append_v1_for_custom_endpoint() {
        assert_eq!(
            normalize_anthropic_genai_base_url("https://ark.cn-beijing.volces.com/api/coding"),
            "https://ark.cn-beijing.volces.com/api/coding/v1/"
        );
        assert_eq!(
            normalize_anthropic_genai_base_url("https://open.bigmodel.cn/api/anthropic/v1"),
            "https://open.bigmodel.cn/api/anthropic/v1/"
        );
    }
}
