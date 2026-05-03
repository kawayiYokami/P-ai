#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    final_response_text: String,
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

fn provider_openai_chat_adapter_kind(
    api_config: &ResolvedApiConfig,
    model_name: &str,
) -> genai::adapter::AdapterKind {
    let base_url = api_config.base_url.to_ascii_lowercase();
    let model_name = model_name.to_ascii_lowercase();
    if base_url.contains("deepseek")
        || base_url.contains("moonshot")
        || model_name.contains("deepseek")
        || model_name.contains("kimi")
    {
        genai::adapter::AdapterKind::DeepSeek
    } else {
        genai::adapter::AdapterKind::OpenAI
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

fn genai_tool_call_id_for_history(call: &NormalizedToolCallRecord) -> Option<String> {
    call.provider_call_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn legacy_tool_call_text_for_history(call: &NormalizedToolCallRecord) -> Option<String> {
    let tool_name = call.tool_name.as_deref()?.trim();
    if tool_name.is_empty() {
        return None;
    }
    let args = call.arguments_text.trim();
    if args.is_empty() {
        Some(format!("工具调用: {tool_name}"))
    } else {
        Some(format!("工具调用: {tool_name}\n参数: {args}"))
    }
}

fn legacy_tool_result_text_for_history(tool_call_id: Option<&str>, text: &str) -> String {
    let trimmed = text.trim();
    let content = if trimmed.is_empty() { " " } else { text };
    match tool_call_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(call_id) => format!("工具结果 ({call_id}):\n{content}"),
        None => format!("工具结果:\n{content}"),
    }
}

fn prepared_history_to_genai_messages(
    prepared: &PreparedPrompt,
) -> Result<Vec<genai::chat::ChatMessage>, String> {
    let mut chat_history = Vec::<genai::chat::ChatMessage>::new();
    let mut tool_call_id_to_provider_call_id =
        std::collections::HashMap::<String, String>::new();
    let normalized_history_messages = normalized_prepared_history_messages(&prepared.history_messages);
    for hm in normalized_history_messages.iter() {
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
                    if let Some(provider_call_id) = call
                        .provider_call_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        tool_call_id_to_provider_call_id
                            .insert(invocation_id.to_string(), provider_call_id.to_string());
                    }
                    let Some(call_id) = genai_tool_call_id_for_history(&call) else {
                        if let Some(text) = legacy_tool_call_text_for_history(&call) {
                            assistant_parts.push(genai::chat::ContentPart::from_text(text));
                        }
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
            let assistant_reasoning_content = Some(hm.reasoning_content.clone().unwrap_or_default());
            if assistant_parts.is_empty() {
                assistant_parts.push(genai::chat::ContentPart::from_text(" "));
            }
            let assistant_message = genai::chat::ChatMessage::assistant(
                genai::chat::MessageContent::from_parts(assistant_parts),
            )
            .with_reasoning_content(assistant_reasoning_content);
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
            if let Some(provider_call_id) = tool_call_id_to_provider_call_id.get(tool_call_id).cloned() {
                chat_history.push(genai::chat::ChatMessage::from(
                    genai::chat::ToolResponse::new(provider_call_id, safe_tool_text),
                ));
            } else {
                chat_history.push(genai::chat::ChatMessage::user(
                    legacy_tool_result_text_for_history(Some(tool_call_id), &safe_tool_text),
                ));
            }
        }
    }
    Ok(chat_history)
}

fn build_genai_chat_request(prepared: &PreparedPrompt) -> Result<genai::chat::ChatRequest, String> {
    let history_messages = prepared_history_to_genai_messages(prepared)?;
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
) -> Result<genai::chat::ChatRequest, String> {
    build_genai_chat_request(prepared)
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
        genai::adapter::AdapterKind::OpenAI
        | genai::adapter::AdapterKind::OpenAIResp
        | genai::adapter::AdapterKind::DeepSeek => {
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
    if provider_genai_model_disables_reasoning(&api_config.model) {
        return None;
    }
    api_config
        .reasoning_effort
        .as_deref()
        .and_then(|value| value.parse::<genai::chat::ReasoningEffort>().ok())
}

fn provider_genai_model_disables_reasoning(model_name: &str) -> bool {
    let normalized = model_name.trim().to_ascii_lowercase();
    normalized.starts_with("gpt-5.3-codex-spark")
        || normalized.contains("-codex-spark")
}

fn build_provider_genai_service_target(
    api_config: &ResolvedApiConfig,
    adapter_kind: genai::adapter::AdapterKind,
    model_name: &str,
    request_api_key: String,
) -> genai::ServiceTarget {
    genai::ServiceTarget {
        endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
            adapter_kind,
            &api_config.base_url,
        )),
        auth: genai::resolver::AuthData::from_single(request_api_key),
        model: genai::ModelIden::new(adapter_kind, model_name),
    }
}

fn strip_model_namespace<'a>(model_name: &'a str) -> &'a str {
    for sep in ['/', ':'] {
        if let Some((_, suffix)) = model_name.split_once(sep) {
            let suffix = suffix.trim();
            if !suffix.is_empty() {
                return suffix;
            }
        }
    }
    model_name
}

fn resolve_model_adapter_for_auto(model_name: &str) -> genai::adapter::AdapterKind {
    let stripped = strip_model_namespace(model_name);
    match genai::adapter::AdapterKind::from_model(stripped) {
        // Ollama 在本应用里走 OpenAI-compatible 协议，由 base_url 指向 Ollama 服务。
        Ok(genai::adapter::AdapterKind::Ollama) => genai::adapter::AdapterKind::OpenAI,
        Ok(kind) => kind,
        Err(_) => genai::adapter::AdapterKind::OpenAI,
    }
}

fn resolve_provider_genai_adapter_kind(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    fallback_adapter_kind: genai::adapter::AdapterKind,
) -> genai::adapter::AdapterKind {
    if api_config.request_format.is_auto() {
        resolve_model_adapter_for_auto(model_name)
    } else {
        api_config
            .request_format
            .genai_adapter_kind()
            .unwrap_or(fallback_adapter_kind)
    }
}

fn build_provider_genai_client_and_model_spec_from_target(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    request_api_key: String,
    service_target: genai::ServiceTarget,
) -> (genai::Client, genai::ModelSpec) {
    let adapter_kind = api_config
        .request_format
        .genai_adapter_kind()
        .or_else(|| api_config.request_format.is_auto().then(|| resolve_model_adapter_for_auto(model_name)));
    if let Some(adapter_kind) = adapter_kind {
        let target = genai::ServiceTarget {
            endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
                adapter_kind,
                &api_config.base_url,
            )),
            auth: genai::resolver::AuthData::from_single(request_api_key),
            model: genai::ModelIden::new(adapter_kind, model_name.to_string()),
        };
        (
            genai::Client::builder().build(),
            genai::ModelSpec::from_target(target),
        )
    } else {
        (
            genai::Client::builder().build(),
            genai::ModelSpec::from_target(service_target),
        )
    }
}

fn build_provider_genai_chat_options(
    api_config: &ResolvedApiConfig,
    capture_reasoning_content: bool,
    capture_tool_calls: bool,
) -> genai::chat::ChatOptions {
    let capture_reasoning_content =
        capture_reasoning_content && !provider_genai_model_disables_reasoning(&api_config.model);
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(capture_reasoning_content)
        .with_extra_headers(provider_genai_headers(api_config));
    if capture_tool_calls {
        options = options.with_capture_tool_calls(true);
    }
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(api_config) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    if let Some(prompt_cache_key) = api_config
        .prompt_cache_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        options = options.with_prompt_cache_key(prompt_cache_key);
    }
    options
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
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_serial_guard =
        maybe_acquire_provider_serial_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let adapter_kind = match kind {
        OpenAiApiKind::ChatCompletions => resolve_provider_genai_adapter_kind(
            &api_config,
            model_name,
            provider_openai_chat_adapter_kind(&api_config, model_name),
        ),
        OpenAiApiKind::Responses => resolve_provider_genai_adapter_kind(
            &api_config,
            model_name,
            genai::adapter::AdapterKind::OpenAIResp,
        ),
    };
    let request = build_provider_genai_request(&prepared)?;
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let options = build_provider_genai_chat_options(&api_config, true, false);

    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    );
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai openai stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, on_delta).await
}

async fn call_model_openai_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    call_model_openai_stream_internal(
        api_config,
        model_name,
        prepared,
        OpenAiApiKind::ChatCompletions,
        None,
        app_state,
    )
    .await
}

async fn call_model_openai_non_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_serial_guard =
        maybe_acquire_provider_serial_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let adapter_kind = resolve_provider_genai_adapter_kind(
        &api_config,
        model_name,
        provider_openai_chat_adapter_kind(&api_config, model_name),
    );
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let request = build_genai_chat_request(&prepared)?;
    let options = build_provider_genai_chat_options(&api_config, true, false);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    );
    let response = client
        .exec_chat(model_spec, request, Some(&options))
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
        assistant_text: assistant_text.clone(),
        final_response_text: assistant_text,
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
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_serial_guard =
        maybe_acquire_provider_serial_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let service_target = build_provider_genai_service_target(
        &api_config,
        resolve_provider_genai_adapter_kind(
            &api_config,
            model_name,
            genai::adapter::AdapterKind::OpenAIResp,
        ),
        model_name,
        request_api_key.clone(),
    );
    let request = build_genai_chat_request(&prepared)?;
    let options = build_provider_genai_chat_options(&api_config, true, false);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    );
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai responses stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, on_delta).await
}

async fn call_model_gemini(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let request = build_genai_chat_request(&prepared)?;
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_serial_guard =
        maybe_acquire_provider_serial_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let service_target = build_provider_genai_service_target(
        &api_config,
        resolve_provider_genai_adapter_kind(
            &api_config,
            model_name,
            genai::adapter::AdapterKind::Gemini,
        ),
        model_name,
        request_api_key.clone(),
    );
    let options = build_provider_genai_chat_options(&api_config, true, false);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    );
    let response = client
        .exec_chat(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai gemini non-stream failed: {err}"))?;
    let assistant_text = response.content.into_texts().join("\n");
    Ok(ModelReply {
        assistant_text: assistant_text.clone(),
        final_response_text: assistant_text,
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
    app_state: Option<&AppState>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_serial_guard =
        maybe_acquire_provider_serial_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let service_target = build_provider_genai_service_target(
        &api_config,
        resolve_provider_genai_adapter_kind(
            &api_config,
            model_name,
            genai::adapter::AdapterKind::Anthropic,
        ),
        model_name,
        request_api_key.clone(),
    );
    let request = build_genai_chat_request(&prepared)?;
    let options = build_provider_genai_chat_options(&api_config, true, false);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    );
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai anthropic stream build failed: {err}"))?
        .stream;
    collect_streaming_model_reply_genai(&mut stream, None).await
}

#[cfg(test)]
mod openai_responses_genai_request_tests {
    use super::*;

    #[test]
    fn build_genai_chat_request_should_keep_system_at_top_level() {
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

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert_eq!(request.system.as_deref(), Some("你是系统提示"));
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::User
        ));
    }

    #[test]
    fn build_provider_genai_chat_options_should_use_prompt_cache_key() {
        let api_config = ResolvedApiConfig {
            provider_id: None,
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            reasoning_effort: None,
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-1".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
        };

        let options = build_provider_genai_chat_options(&api_config, false, false);

        assert_eq!(options.prompt_cache_key.as_deref(), Some("conversation-1"));
    }

    #[test]
    fn build_provider_genai_chat_options_should_use_prompt_cache_key_for_codex() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-codex".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
        };

        let options = build_provider_genai_chat_options(&api_config, true, true);

        assert_eq!(options.prompt_cache_key.as_deref(), Some("conversation-codex"));
    }

    #[test]
    fn build_provider_genai_chat_options_should_disable_reasoning_for_codex_spark() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-codex".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
        };

        let options = build_provider_genai_chat_options(&api_config, true, true);

        assert_eq!(options.capture_reasoning_content, Some(false));
        assert!(options.reasoning_effort.is_none());
    }

    #[test]
    fn build_genai_chat_request_should_backfill_empty_reasoning_content() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "第一条没有思维链".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "收到".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "第二条保留思维链".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: Some("已有思维链".to_string()),
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert_eq!(request.messages[0].content.reasoning_contents(), vec![""]);
        assert_eq!(
            request.messages[2].content.reasoning_contents(),
            vec!["已有思维链"]
        );
    }

    #[test]
    fn build_genai_chat_request_should_downgrade_legacy_tool_history_without_provider_call_id() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "local_call_1",
                        "type": "function",
                        "function": {
                            "name": "lookup",
                            "arguments": "{\"q\":\"天气\"}"
                        }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "晴天".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("local_call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::Assistant
        ));
        assert_eq!(
            request.messages[0].content.texts(),
            vec!["工具调用: lookup\n参数: {\"q\":\"天气\"}"]
        );
        assert!(request.messages[0].content.tool_calls().is_empty());
        assert!(matches!(
            request.messages[1].role,
            genai::chat::ChatRole::User
        ));
        assert_eq!(
            request.messages[1].content.texts(),
            vec!["工具结果 (local_call_1):\n晴天"]
        );
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
