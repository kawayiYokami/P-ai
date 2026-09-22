#[derive(Debug, Clone)]
struct ModelReply {
    assistant_text: String,
    final_response_text: String,
    activity_reasoning_text: String,
    assistant_provider_meta: Option<Value>,
    tool_history_events: Vec<Value>,
    suppress_assistant_message: bool,
    trusted_input_tokens: Option<u64>,
    usage: Option<Value>,
    round_logs_recorded_internally: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum OpenAiApiKind {
    ChatCompletions,
    Responses,
}

fn genai_usage_to_log_value(usage: &genai::chat::Usage) -> Option<Value> {
    let prompt_details = usage.prompt_tokens_details.as_ref();
    let completion_details = usage.completion_tokens_details.as_ref();
    let cache_creation_details =
        prompt_details.and_then(|details| details.cache_creation_details.as_ref());
    if usage.prompt_tokens.is_none()
        && usage.completion_tokens.is_none()
        && usage.total_tokens.is_none()
        && prompt_details.and_then(|details| details.cached_tokens).is_none()
        && prompt_details.and_then(|details| details.cache_creation_tokens).is_none()
        && cache_creation_details.and_then(|details| details.ephemeral_5m_tokens).is_none()
        && cache_creation_details.and_then(|details| details.ephemeral_1h_tokens).is_none()
        && completion_details.and_then(|details| details.reasoning_tokens).is_none()
    {
        return None;
    }
    Some(serde_json::json!({
        "promptTokens": usage.prompt_tokens,
        "completionTokens": usage.completion_tokens,
        "totalTokens": usage.total_tokens,
        "cachedTokens": prompt_details.and_then(|details| details.cached_tokens),
        "cacheCreationTokens": prompt_details.and_then(|details| details.cache_creation_tokens),
        "cacheCreation5mTokens": cache_creation_details.and_then(|details| details.ephemeral_5m_tokens),
        "cacheCreation1hTokens": cache_creation_details.and_then(|details| details.ephemeral_1h_tokens),
        "reasoningTokens": completion_details.and_then(|details| details.reasoning_tokens),
    }))
}

fn genai_response_id_provider_meta(response_id: Option<&str>) -> Option<Value> {
    response_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            serde_json::json!({
                "genai": {
                    "responseId": value,
                }
            })
        })
}

fn merge_assistant_provider_meta_patch(target: &mut Option<Value>, patch: Option<Value>) {
    let Some(patch) = patch else {
        return;
    };
    if target.is_none() {
        *target = Some(patch);
        return;
    }
    let Some(current) = target.as_mut() else {
        return;
    };
    if !current.is_object() {
        let raw_provider_meta = std::mem::replace(current, serde_json::json!({}));
        *current = serde_json::json!({
            "_raw_provider_meta": raw_provider_meta,
        });
    }
    let Some(current_obj) = current.as_object_mut() else {
        return;
    };
    if let Some(patch_obj) = patch.as_object() {
        for (key, value) in patch_obj {
            current_obj.insert(key.clone(), value.clone());
        }
    }
}

fn add_provider_usage_delta_to_conversation(
    app_state: Option<&AppState>,
    conversation_id: Option<&str>,
    provider_key: Option<&str>,
    model_name: Option<&str>,
    usage: &Value,
) {
    let Some(app_state) = app_state else {
        return;
    };
    let Some(conversation_id) = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let result = match delegate_runtime_thread_conversation_get(app_state, conversation_id) {
        Ok(Some(mut conversation)) => {
            let changed = conversation_cumulative_usage_add_provider_usage(
                &mut conversation.cumulative_usage,
                provider_key,
                model_name,
                usage,
            );
            if changed {
                delegate_runtime_thread_conversation_update(
                    app_state,
                    conversation_id,
                    conversation.clone(),
                )
                .map(|_| {
                    usage_trail_record_conversation_delta(
                        app_state,
                        &conversation,
                        provider_key,
                        model_name,
                        usage,
                    );
                    true
                })
            } else {
                Ok(false)
            }
        }
        Ok(None) => conversation_service_v2()
            .add_conversation_cumulative_usage_delta(
                app_state,
                conversation_id,
                provider_key,
                model_name,
                usage,
            ),
        Err(err) => Err(err),
    };

    match result {
        Ok(true) => {}
        Ok(false) => {}
        Err(err) => runtime_log_warn(format!(
            "[聊天用量] 失败，任务=累加供应商用量，conversation_id={}，error={}",
            conversation_id, err
        )),
    }
}

fn usage_provider_key_from_api_config(api_config: &ResolvedApiConfig) -> String {
    api_config
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| api_config.request_format.as_str().to_string())
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
    let content = if trimmed.is_empty() { "(no output)" } else { text };
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
            let mut text_blocks = Vec::<String>::new();
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    text_blocks.push(time_text.clone());
                }
            }
            if !hm.text.trim().is_empty() {
                text_blocks.push(hm.text.clone());
            }
            for block in &hm.extra_text_blocks {
                if !block.trim().is_empty() {
                    text_blocks.push(block.clone());
                }
            }
            // 空消息（无文本块且无媒体）不进请求体
            if text_blocks.is_empty() && hm.images.is_empty() && hm.audios.is_empty() {
                continue;
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
                    // 非法或非 object 参数：拒绝结构化回放（模型端 .items() 会崩/400），
                    // 转成普通文本并保留原始参数；不注册 invocation_id，其工具结果也会走文本兜底，避免孤立 ToolResult。
                    if !call.arguments_value.is_object() {
                        if let Some(text) = legacy_tool_call_text_for_history(&call) {
                            assistant_parts.push(genai::chat::ContentPart::from_text(text));
                        }
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
            let assistant_message = genai::chat::ChatMessage::assistant(
                genai::chat::MessageContent::from_parts(assistant_parts),
            )
            .with_reasoning_content(assistant_reasoning_content);
            chat_history.push(assistant_message);
        } else if hm.role == "tool" {
            let safe_tool_text = if hm.text.trim().is_empty() {
                "(no output)".to_string()
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

fn genai_assistant_message_has_provider_content(message: &genai::chat::ChatMessage) -> bool {
    if !matches!(message.role, genai::chat::ChatRole::Assistant) {
        return true;
    }
    if message
        .content
        .texts()
        .into_iter()
        .any(|text| !text.trim().is_empty())
    {
        return true;
    }
    if !message.content.binaries().is_empty() {
        return true;
    }
    if message
        .content
        .reasoning_contents()
        .into_iter()
        .any(|text| !text.trim().is_empty())
    {
        return true;
    }
    message
        .content
        .tool_calls()
        .into_iter()
        .any(|call| !call.call_id.trim().is_empty() && !call.fn_name.trim().is_empty())
}

fn sanitize_genai_messages_before_request(
    messages: Vec<genai::chat::ChatMessage>,
    scene: &str,
) -> Vec<genai::chat::ChatMessage> {
    let mut dropped = 0usize;
    let sanitized = messages
        .into_iter()
        .filter(|message| {
            let keep = genai_assistant_message_has_provider_content(message);
            if !keep {
                dropped += 1;
            }
            keep
        })
        .collect::<Vec<_>>();
    if dropped > 0 {
        runtime_log_warn(format!(
            "[聊天] 请求前过滤空 assistant 消息: scene={}, dropped_count={}",
            scene, dropped
        ));
    }
    sanitized
}

fn build_genai_chat_request(prepared: &PreparedPrompt) -> Result<genai::chat::ChatRequest, String> {
    let history_messages = prepared_history_to_genai_messages(prepared)?;
    let latest_parts = genai_content_parts_from_text_and_binary(
        &prepared_prompt_latest_user_text_blocks(prepared),
        &prepared.latest_images,
        &prepared.latest_audios,
    );
    let mut request = genai::chat::ChatRequest::from_messages(
        sanitize_genai_messages_before_request(history_messages, "prepared_history"),
    );
    // 最新消息全空（无文本块且无媒体）时不追加空 user 消息
    if !latest_parts.is_empty() {
        request = request.append_message(genai::chat::ChatMessage::user(
            genai::chat::MessageContent::from_parts(latest_parts),
        ));
    }
    let system = prepared.preamble.trim();
    if !system.is_empty() {
        request = request.with_system(system.to_string());
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

fn normalize_minimax_genai_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.minimax.io/anthropic/v1/".to_string();
    }
    let without_messages = trimmed.trim_end_matches("/messages").trim_end_matches('/');
    let with_version = if without_messages.ends_with("/v1") {
        without_messages.to_string()
    } else {
        format!("{without_messages}/v1")
    };
    format!("{with_version}/")
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
        genai::adapter::AdapterKind::MiniMax => normalize_minimax_genai_base_url(raw),
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

/// OpenCode 端点会话头回退值：无会话上下文的调用（标题生成、摘要等）使用全局稳定标识
const OPENCODE_SESSION_FALLBACK_ID: &str = "pai-standalone";

/// 判断 base_url 是否指向 OpenCode 站点（x-opencode-session 会话头注入条件）。
/// 只认 host、不钉路径形态，端点路径变化不受影响。
fn is_opencode_ai_endpoint(base_url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    host == "opencode.ai" || host.ends_with(".opencode.ai")
}

fn provider_genai_headers(api_config: &ResolvedApiConfig) -> genai::Headers {
    match api_config.request_format {
        RequestFormat::Codex => {
            let mut headers = app_identity_genai_headers();
            headers.merge(api_config.extra_headers.clone());
            headers
        }
        _ => {
            let mut headers = app_identity_genai_headers();
            headers.merge(api_config.extra_headers.clone());
            headers
        }
    }
}

fn provider_genai_reasoning_effort(
    api_config: &ResolvedApiConfig,
    adapter_kind: genai::adapter::AdapterKind,
) -> Option<genai::chat::ReasoningEffort> {
    if provider_genai_model_disables_reasoning(&api_config.model) {
        return None;
    }
    // DeepSeek 已由 genai 的 managed_body_thinking 管理（Zero 会生成 thinking.type=disabled），
    // 直接透传 reasoning_effort；其余需要手动关思维链的通道（moonshot/doubao 等）none 时压成 None。
    if provider_genai_reasoning_disabled_raw(api_config)
        && provider_genai_requires_manual_thinking_disabled(api_config, adapter_kind)
    {
        return None;
    }
    api_config
        .reasoning_effort
        .as_deref()
        .and_then(|value| value.parse::<genai::chat::ReasoningEffort>().ok())
}

fn provider_genai_reasoning_disabled_raw(api_config: &ResolvedApiConfig) -> bool {
    matches!(
        api_config.reasoning_effort.as_deref().map(str::trim),
        Some(value) if value.eq_ignore_ascii_case("none")
    )
}

fn provider_genai_reasoning_explicitly_disabled(api_config: &ResolvedApiConfig) -> bool {
    provider_genai_reasoning_disabled_raw(api_config)
}

/// 需要手动注入 thinking.type=disabled 的通道：非 DeepSeek（genai 已管理）且命中
/// 已知需关闭思维链的供应商域名/模型（moonshot/doubao/ark/volc/kimi 等）。
/// Responses 协议家族不走该通道：`thinking` 是 Chat Completions 字段，
/// /responses 端点不识别；none 由 genai 转成 reasoning.effort=none 透传。
fn provider_genai_requires_manual_thinking_disabled(
    api_config: &ResolvedApiConfig,
    adapter_kind: genai::adapter::AdapterKind,
) -> bool {
    if !provider_genai_reasoning_disabled_raw(api_config) {
        return false;
    }
    if api_config.request_format.is_openai_responses_family() {
        return false;
    }
    if adapter_kind == genai::adapter::AdapterKind::DeepSeek {
        return false;
    }
    let base_url = api_config.base_url.trim().to_ascii_lowercase();
    let model_name = api_config.model.trim().to_ascii_lowercase();
    base_url.contains("deepseek")
        || base_url.contains("moonshot")
        || base_url.contains("doubao")
        || base_url.contains("ark")
        || base_url.contains("volc")
        || model_name.contains("deepseek")
        || model_name.contains("kimi")
        || model_name.contains("doubao")
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

fn resolve_provider_genai_adapter_kind(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    fallback_adapter_kind: genai::adapter::AdapterKind,
) -> genai::adapter::AdapterKind {
    resolve_model_protocol(
        api_config.request_format,
        &api_config.base_url,
        model_name,
        fallback_adapter_kind,
    )
    .adapter_kind
}

fn build_provider_genai_client_and_model_spec_from_target(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    request_api_key: String,
    service_target: genai::ServiceTarget,
) -> Result<(genai::Client, genai::ModelSpec), String> {
    let adapter_kind = (api_config.request_format.is_genai_chat()
        || api_config.request_format.is_auto())
        .then(|| resolve_provider_genai_adapter_kind(
            api_config,
            model_name,
            service_target.model.adapter_kind,
        ));
    if let Some(adapter_kind) = adapter_kind {
        let target = genai::ServiceTarget {
            endpoint: genai::resolver::Endpoint::from_owned(normalize_provider_genai_base_url(
                adapter_kind,
                &api_config.base_url,
            )),
            auth: genai::resolver::AuthData::from_single(request_api_key),
            model: genai::ModelIden::new(adapter_kind, model_name.to_string()),
        };
        let client = genai::Client::builder()
            .with_adapter_kind(adapter_kind)
            .build()
            .map_err(|err| format!("构建 genai 客户端失败: {err}"))?;
        Ok((
            client,
            genai::ModelSpec::from_target(target),
        ))
    } else {
        let client = genai::Client::builder()
            .build()
            .map_err(|err| format!("构建 genai 客户端失败: {err}"))?;
        Ok((client, genai::ModelSpec::from_target(service_target)))
    }
}

fn build_provider_genai_chat_options(
    api_config: &ResolvedApiConfig,
    adapter_kind: genai::adapter::AdapterKind,
    capture_reasoning_content: bool,
    capture_tool_calls: bool,
    opencode_session_id: Option<&str>,
) -> genai::chat::ChatOptions {
    let capture_reasoning_content = capture_reasoning_content
        && !provider_genai_model_disables_reasoning(&api_config.model)
        && !provider_genai_reasoning_explicitly_disabled(api_config);
    let mut headers = provider_genai_headers(api_config);
    if is_opencode_ai_endpoint(&api_config.base_url) {
        // OpenCode 要求每对话稳定的会话标识用于路由与提示词缓存；无会话上下文时回退全局稳定值
        let session_id = opencode_session_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(OPENCODE_SESSION_FALLBACK_ID);
        headers.merge(vec![(
            "x-opencode-session".to_string(),
            session_id.to_string(),
        )]);
    }
    let mut options = genai::chat::ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_reasoning_content(capture_reasoning_content)
        .with_extra_headers(headers);
    if capture_tool_calls {
        options = options.with_capture_tool_calls(true);
    }
    if let Some(reasoning_effort) = provider_genai_reasoning_effort(api_config, adapter_kind) {
        options = options.with_reasoning_effort(reasoning_effort);
    }
    if provider_genai_requires_manual_thinking_disabled(api_config, adapter_kind) {
        options = options.with_extra_body(serde_json::json!({
            "thinking": {
                "type": "disabled",
            }
        }));
    }
    if api_config.request_format == RequestFormat::Codex {
        options = options.with_extra_body(serde_json::json!({
            "text": {
                "verbosity": "low",
            }
        }));
    }
    if let Some(temperature) = api_config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_output_tokens) = api_config.max_output_tokens {
        options = options.with_max_tokens(max_output_tokens);
    }
    if api_config.request_format.is_openai_responses_family() {
        if let Some(prompt_cache_key) = api_config
            .prompt_cache_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            options = options.with_prompt_cache_key(prompt_cache_key);
        }
    }
    options
}

async fn resolve_request_api_config(
    api_config: &ResolvedApiConfig,
) -> Result<ResolvedApiConfig, String> {
    // 如果是自定义URL模式，直接使用自定义API Key
    if api_config.request_format == RequestFormat::Codex {
        if let Some(custom_api_key) = &api_config.codex_custom_api_key {
            if !custom_api_key.trim().is_empty() {
                let mut next = api_config.clone();
                next.api_key = custom_api_key.clone();
                return Ok(next);
            }
        }
    }
    
    let Some(codex_auth) = &api_config.codex_auth else {
        return Ok(api_config.clone());
    };
    let fresh_auth = ensure_codex_runtime_auth_fresh(codex_auth).await?;
    let mut next = api_config.clone();
    next.api_key = fresh_auth.access_token.clone();
    if let Some(account_id) = fresh_auth
        .account_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        next.extra_headers
            .retain(|(key, _)| !key.eq_ignore_ascii_case("ChatGPT-Account-Id"));
        next.extra_headers
            .push(("ChatGPT-Account-Id".to_string(), account_id.to_string()));
    }
    next.codex_auth = Some(fresh_auth.clone());
    Ok(next)
}

async fn call_model_genai_stream_internal(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    kind: OpenAiApiKind,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
    tool_definitions: Option<&[ProviderToolDefinition]>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_concurrency_guard =
        maybe_acquire_provider_concurrency_guard(app_state, &api_config, model_name).await?;
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
    let mut request = build_provider_genai_request(&prepared)?;
    if let Some(definitions) = tool_definitions {
        if !definitions.is_empty() {
            let genai_tools =
                runtime_tool_definitions_for_genai(definitions, adapter_kind).await?;
            request = request.with_tools(genai_tools);
        }
    }
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let options = build_provider_genai_chat_options(&api_config, adapter_kind, true, false, None);

    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai openai stream build failed: {err}"))?
        .stream;
    let usage_provider_key = usage_provider_key_from_api_config(&api_config);
    collect_streaming_model_reply_genai(
        &mut stream,
        on_delta,
        app_state,
        usage_conversation_id,
        Some(usage_provider_key.as_str()),
        Some(model_name),
    )
    .await
}

async fn call_model_genai_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    call_model_genai_stream_internal(
        api_config,
        model_name,
        prepared,
        OpenAiApiKind::ChatCompletions,
        None,
        app_state,
        usage_conversation_id,
        None,
    )
    .await
}

async fn call_model_genai_stream_with_tools(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_definitions: Vec<ProviderToolDefinition>,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    call_model_genai_stream_internal(
        api_config,
        model_name,
        prepared,
        OpenAiApiKind::ChatCompletions,
        None,
        app_state,
        usage_conversation_id,
        Some(&tool_definitions),
    )
    .await
}

async fn call_model_genai_non_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    call_model_genai_non_stream_with_definitions(
        api_config,
        model_name,
        prepared,
        Vec::new(),
        app_state,
        usage_conversation_id,
    )
    .await
}

async fn call_model_genai_non_stream_with_definitions(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_definitions: Vec<ProviderToolDefinition>,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_concurrency_guard =
        maybe_acquire_provider_concurrency_guard(app_state, &api_config, model_name).await?;
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
    let mut request = build_genai_chat_request(&prepared)?;
    if !tool_definitions.is_empty() {
        let genai_tools = runtime_tool_definitions_for_genai(&tool_definitions, adapter_kind).await?;
        request = request.with_tools(genai_tools);
    }
    let options = build_provider_genai_chat_options(&api_config, adapter_kind, true, false, None);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let response = client
        .exec_chat(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai openai non-stream failed: {err}"))?;
    let usage = genai_usage_to_log_value(&response.usage);
    let usage_provider_key = usage_provider_key_from_api_config(&api_config);
    if let Some(usage) = usage.as_ref() {
        add_provider_usage_delta_to_conversation(
            app_state,
            usage_conversation_id,
            Some(usage_provider_key.as_str()),
            Some(model_name),
            usage,
        );
    }
    let response_texts = response.content.into_texts();
    let assistant_text = join_model_text_blocks(response_texts.iter().map(String::as_str));
    let activity_reasoning_text = response.reasoning_content.unwrap_or_default();
    let assistant_provider_meta = genai_response_id_provider_meta(response.response_id.as_deref());
    let trusted_input_tokens = response
        .usage
        .prompt_tokens
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0);
    Ok(ModelReply {
        assistant_text: assistant_text.clone(),
        final_response_text: assistant_text,
        activity_reasoning_text,
        assistant_provider_meta,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage,
        round_logs_recorded_internally: false,
    })
}

async fn call_model_openai_responses(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_concurrency_guard =
        maybe_acquire_provider_concurrency_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let adapter_kind = resolve_provider_genai_adapter_kind(
        &api_config,
        model_name,
        genai::adapter::AdapterKind::OpenAIResp,
    );
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let request = build_genai_chat_request(&prepared)?;
    let options = build_provider_genai_chat_options(&api_config, adapter_kind, true, false, None);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai responses stream build failed: {err}"))?
        .stream;
    let usage_provider_key = usage_provider_key_from_api_config(&api_config);
    collect_streaming_model_reply_genai(
        &mut stream,
        on_delta,
        app_state,
        usage_conversation_id,
        Some(usage_provider_key.as_str()),
        Some(model_name),
    )
    .await
}

async fn call_model_gemini(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    let request = build_genai_chat_request(&prepared)?;
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_concurrency_guard =
        maybe_acquire_provider_concurrency_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let adapter_kind = resolve_provider_genai_adapter_kind(
        &api_config,
        model_name,
        genai::adapter::AdapterKind::Gemini,
    );
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let options = build_provider_genai_chat_options(&api_config, adapter_kind, true, false, None);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let response = client
        .exec_chat(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai gemini non-stream failed: {err}"))?;
    let usage = genai_usage_to_log_value(&response.usage);
    let usage_provider_key = usage_provider_key_from_api_config(&api_config);
    if let Some(usage) = usage.as_ref() {
        add_provider_usage_delta_to_conversation(
            app_state,
            usage_conversation_id,
            Some(usage_provider_key.as_str()),
            Some(model_name),
            usage,
        );
    }
    let response_texts = response.content.into_texts();
    let assistant_text = join_model_text_blocks(response_texts.iter().map(String::as_str));
    let assistant_provider_meta = genai_response_id_provider_meta(response.response_id.as_deref());
    Ok(ModelReply {
        assistant_text: assistant_text.clone(),
        final_response_text: assistant_text,
        activity_reasoning_text: response.reasoning_content.unwrap_or_default(),
        assistant_provider_meta,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens: response
            .usage
            .prompt_tokens
            .and_then(|value| u64::try_from(value).ok())
            .filter(|value| *value > 0),
        usage,
        round_logs_recorded_internally: false,
    })
}

async fn call_model_anthropic(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let _provider_concurrency_guard =
        maybe_acquire_provider_concurrency_guard(app_state, &api_config, model_name).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let adapter_kind = resolve_provider_genai_adapter_kind(
        &api_config,
        model_name,
        genai::adapter::AdapterKind::Anthropic,
    );
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let request = build_genai_chat_request(&prepared)?;
    let options = build_provider_genai_chat_options(&api_config, adapter_kind, true, false, None);
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let mut stream = client
        .exec_chat_stream(model_spec, request, Some(&options))
        .await
        .map_err(|err| format!("genai anthropic stream build failed: {err}"))?
        .stream;
    let usage_provider_key = usage_provider_key_from_api_config(&api_config);
    collect_streaming_model_reply_genai(
        &mut stream,
        None,
        app_state,
        usage_conversation_id,
        Some(usage_provider_key.as_str()),
        Some(model_name),
    )
    .await
}

#[cfg(test)]
mod openai_responses_genai_request_tests {
    use super::*;

    fn prepared_prompt_from_aggregated_assistant(message: &ChatMessage) -> PreparedPrompt {
        let mut history_messages = vec![PreparedHistoryMessage {
            role: "user".to_string(),
            text: "查一下 PowerShell 版本".to_string(),
            extra_text_blocks: Vec::new(),
            user_time_text: None,
            images: Vec::new(),
            audios: Vec::new(),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }];
        history_messages.extend(build_prepared_history_messages_from_tool_history(
            message,
            MessageToolHistoryView::PromptReplay,
        ));
        history_messages.push(PreparedHistoryMessage {
            role: "assistant".to_string(),
            text: render_prompt_message_text(message),
            extra_text_blocks: Vec::new(),
            user_time_text: None,
            images: Vec::new(),
            audios: Vec::new(),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: Some("我已经拿到工具结果，现在直接回答用户终端版本。".to_string()),
        });
        PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages,
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        }
    }

    fn canonical_preview_assistant_and_tool_messages(messages: &[Value]) -> Vec<Value> {
        let mut invocation_to_provider = std::collections::HashMap::<String, Value>::new();
        let mut canonical = Vec::<Value>::new();
        for message in messages {
            let Some(role) = message.get("role").and_then(Value::as_str) else {
                continue;
            };
            match role {
                "assistant" => {
                    let tool_calls = message
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|call| {
                            let provider_tool_call_id = call
                                .get("call_id")
                                .or_else(|| call.get("id"))
                                .cloned()
                                .unwrap_or(Value::Null);
                            if let Some(invocation_id) =
                                call.get("id").and_then(Value::as_str).map(ToOwned::to_owned)
                            {
                                invocation_to_provider
                                    .insert(invocation_id, provider_tool_call_id.clone());
                            }
                            serde_json::json!({
                                "tool_call_id": provider_tool_call_id,
                                "function_name": call
                                    .get("function")
                                    .and_then(|func| func.get("name"))
                                    .cloned()
                                    .unwrap_or(Value::Null),
                                "arguments": normalize_tool_call_arguments(
                                    call.get("function").and_then(|func| func.get("arguments"))
                                ).0,
                            })
                        })
                        .collect::<Vec<_>>();
                    canonical.push(serde_json::json!({
                        "role": "assistant",
                        "content": message.get("content").cloned().unwrap_or(Value::Null),
                        "reasoning_content": message.get("reasoning_content").cloned().unwrap_or(Value::Null),
                        "tool_calls": Value::Array(tool_calls),
                    }));
                }
                "tool" => {
                    let raw_tool_call_id = message.get("tool_call_id").and_then(Value::as_str);
                    let tool_call_id = raw_tool_call_id
                        .and_then(|value| invocation_to_provider.get(value).cloned())
                        .or_else(|| message.get("tool_call_id").cloned())
                        .unwrap_or(Value::Null);
                    canonical.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": message.get("content").cloned().unwrap_or(Value::Null),
                    }));
                }
                _ => {}
            }
        }
        canonical
    }

    fn canonical_provider_assistant_and_tool_messages(
        request: &genai::chat::ChatRequest,
    ) -> Vec<Value> {
        request
            .messages
            .iter()
            .filter_map(|message| match message.role {
                genai::chat::ChatRole::Assistant => Some(serde_json::json!({
                    "role": "assistant",
                    "content": message
                        .content
                        .texts()
                        .first()
                        .map(|text| Value::String((*text).to_string()))
                        .unwrap_or(Value::Null),
                    "reasoning_content": message
                        .content
                        .reasoning_contents()
                        .first()
                        .map(|text| Value::String((*text).to_string()))
                        .unwrap_or(Value::Null),
                    "tool_calls": Value::Array(
                        message
                            .content
                            .tool_calls()
                            .into_iter()
                            .map(|call| {
                                serde_json::json!({
                                    "tool_call_id": call.call_id,
                                    "function_name": call.fn_name,
                                    "arguments": call.fn_arguments,
                                })
                            })
                            .collect()
                    ),
                })),
                genai::chat::ChatRole::Tool => Some(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": message
                        .content
                        .tool_responses()
                        .first()
                        .map(|response| Value::String(response.call_id.clone()))
                        .unwrap_or(Value::Null),
                    "content": message
                        .content
                        .tool_responses()
                        .first()
                        .map(|response| Value::String(response.content.clone()))
                        .unwrap_or(Value::Null),
                })),
                _ => None,
            })
            .collect()
    }

    fn prepared_prompt_with_single_image_path_and_base64() -> PreparedPrompt {
        PreparedPrompt {
            preamble: String::new(),
            history_messages: Vec::new(),
            latest_user_text: "这是什么".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: "[附件#1]\npath: {Assistant Space}/downloads/image.png".to_string(),
            latest_user_extra_blocks: vec!["[附件#1]\npath: {Assistant Space}/downloads/image.png".to_string()],
            latest_images: vec![PreparedBinaryPayload {
                label: "图片#1".to_string(),
                mime: "image/png".to_string(),
                content: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO9Wl1QAAAAASUVORK5CYII=".to_string(),
                saved_path: Some("downloads/image.png".to_string()),
            }],
            latest_audios: Vec::new(),
        }
    }

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
    fn build_genai_chat_request_should_skip_empty_latest_user_when_no_media() {
        let prepared = PreparedPrompt {
            preamble: "你是系统提示".to_string(),
            history_messages: vec![PreparedHistoryMessage {
                role: "user".to_string(),
                text: "上一轮问题".to_string(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert_eq!(request.messages.len(), 1);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::User
        ));
    }

    #[test]
    fn build_genai_chat_request_should_skip_empty_history_user_when_no_media() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: String::new(),
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
                    text: "这是结论".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
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

        // 空 user 历史消息被跳过，只保留 assistant 与 latest user
        assert_eq!(request.messages.len(), 2);
        assert!(matches!(
            request.messages[0].role,
            genai::chat::ChatRole::Assistant
        ));
        assert!(matches!(
            request.messages[1].role,
            genai::chat::ChatRole::User
        ));
    }

    fn history_pp_with_tool_calls(
        tool_calls: Vec<serde_json::Value>,
        tool_result_text: &str,
    ) -> PreparedPrompt {
        PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "调用 write 写文件".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(tool_calls),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: tool_result_text.to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("inv_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        }
    }

    fn history_tool_call(invocation_id: &str, call_id: &str, name: &str, arguments: &str) -> serde_json::Value {
        serde_json::json!({
            "id": invocation_id,
            "call_id": call_id,
            "type": "function",
            "function": { "name": name, "arguments": arguments }
        })
    }

    #[test]
    fn history_replay_should_render_invalid_args_as_text_pair_not_structured() {
        // 截断 JSON 对象文本（与真实故障 4de438c6 同类结构：content 未闭合）
        let truncated = "{\"path\": \"a.md\", \"content\": \"# 未闭合";
        let prepared = history_pp_with_tool_calls(
            vec![history_tool_call("inv_1", "call_1", "write", truncated)],
            "结果文本",
        );
        let messages = prepared_history_to_genai_messages(&prepared).expect("ok");
        assert_eq!(messages.len(), 2, "非法调用+结果应为两段文本，无结构化 call/result");

        // assistant：转文本，无 ToolCall
        assert!(matches!(messages[0].role, genai::chat::ChatRole::Assistant));
        assert!(
            !messages[0].content.parts().iter().any(|p| matches!(p, genai::chat::ContentPart::ToolCall(_))),
            "非法参数不得产生结构化 ToolCall"
        );
        let texts = messages[0]
            .content
            .parts()
            .iter()
            .filter_map(|p| match p {
                genai::chat::ContentPart::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            texts.iter().any(|t| t.contains("write") && t.contains(truncated)),
            "应保留原始参数与失败事实，实际={:?}",
            texts
        );

        // tool 结果：因 invocation 未注册，走文本兜底 user 消息，而非孤立 ToolResponse
        assert!(matches!(messages[1].role, genai::chat::ChatRole::User));
        assert!(
            !messages[1].content.parts().iter().any(|p| matches!(p, genai::chat::ContentPart::ToolResponse(_))),
            "不得产生孤立 ToolResponse"
        );
        assert!(
            messages[1]
                .content
                .parts()
                .iter()
                .filter_map(|p| match p {
                    genai::chat::ContentPart::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .any(|t| t.contains("结果文本"))
        );
    }

    #[test]
    fn history_replay_should_keep_valid_and_mixed_calls_semantics() {
        let truncated = "{\"path\": \"b.md\", \"content\": \"# 未闭合";
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "写两个文件".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![
                        // 合法 double-escaped 对象：规范成 object ToolCall
                        serde_json::json!({
                            "id": "inv_valid",
                            "call_id": "call_valid",
                            "type": "function",
                            "function": { "name": "write", "arguments": "{\"path\":\"a.md\",\"content\":\"# 标题\"}" }
                        }),
                        // 非法截断：转文本
                        serde_json::json!({
                            "id": "inv_bad",
                            "call_id": "call_bad",
                            "type": "function",
                            "function": { "name": "write", "arguments": truncated }
                        }),
                    ]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "有效结果".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("inv_valid".to_string()),
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "坏结果".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("inv_bad".to_string()),
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
        let messages = prepared_history_to_genai_messages(&prepared).expect("ok");

        // assistant：1 个合法 ToolCall（object 参数）+ 1 段非法文本
        assert!(matches!(messages[0].role, genai::chat::ChatRole::Assistant));
        let valid_calls = messages[0]
            .content
            .parts()
            .iter()
            .filter_map(|part| match part {
                genai::chat::ContentPart::ToolCall(call)
                    if call.fn_arguments.is_object() =>
                {
                    Some(call)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(valid_calls.len(), 1, "合法调用应保留为 object ToolCall");
        assert_eq!(valid_calls[0].fn_arguments["path"], "a.md");
        assert!(
            messages[0]
                .content
                .parts()
                .iter()
                .filter_map(|p| match p {
                    genai::chat::ContentPart::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .any(|t| t.contains(truncated)),
            "非法调用应转为文本"
        );

        // 合法结果 → ToolResponse；非法结果 → 文本兜底 user 消息，无孤立 ToolResponse
        let tool_responses = messages
            .iter()
            .flat_map(|m| m.content.parts().iter())
            .filter_map(|part| match part {
                genai::chat::ContentPart::ToolResponse(r) => Some(r.content.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            tool_responses.iter().any(|t| t.contains("有效结果")),
            "合法结果应保留为 ToolResponse，实际={:?}",
            tool_responses
        );
        let user_texts = messages
            .iter()
            .filter(|m| matches!(m.role, genai::chat::ChatRole::User))
            .flat_map(|m| m.content.parts().iter())
            .filter_map(|p| match p {
                genai::chat::ContentPart::Text(t) => Some(t.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            user_texts.iter().any(|t| t.contains("坏结果")),
            "非法结果应转文本 user 消息，实际={:?}",
            user_texts
        );
        assert!(
            !tool_responses.iter().any(|t| t.contains("坏结果")),
            "非法结果不得产生孤立 ToolResponse"
        );
    }

    fn session_header_test_fixture() -> ResolvedApiConfig {
        ResolvedApiConfig {
            provider_id: None,
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            reasoning_effort: None,
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        }
    }

    fn header_value_of(headers: &genai::Headers, name: &str) -> Option<String> {
        for (k, v) in headers {
            if k.as_str() == name {
                return Some(v.clone());
            }
        }
        None
    }

    #[test]
    fn opencode_endpoint_should_inject_session_header() {
        let mut api_config = session_header_test_fixture();
        api_config.base_url = "https://opencode.ai/zen/go/v1".to_string();

        // 有会话标识：注入会话值
        let options =
            build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenCodeGo, true, true, Some("conv-123"));
        let headers = options.extra_headers.as_ref().expect("headers should exist");
        assert_eq!(header_value_of(headers, "x-opencode-session").as_deref(), Some("conv-123"));

        // 无会话标识：回退全局稳定值
        let options =
            build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenCodeGo, true, true, None);
        let headers = options.extra_headers.as_ref().expect("headers should exist");
        assert_eq!(
            header_value_of(headers, "x-opencode-session").as_deref(),
            Some(OPENCODE_SESSION_FALLBACK_ID)
        );
    }

    #[test]
    fn non_opencode_endpoint_should_not_inject_session_header() {
        let api_config = session_header_test_fixture();

        let options =
            build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAI, true, true, Some("conv-123"));
        let headers = options.extra_headers.as_ref().expect("headers should exist");
        assert!(header_value_of(headers, "x-opencode-session").is_none());
    }

    #[test]
    fn opencode_endpoint_detection_should_be_host_based() {
        assert!(is_opencode_ai_endpoint("https://opencode.ai/zen/go/v1"));
        assert!(is_opencode_ai_endpoint("https://opencode.ai/zen/go"));
        assert!(is_opencode_ai_endpoint("https://opencode.ai/future-path/v9"));
        assert!(is_opencode_ai_endpoint("https://api.opencode.ai/v1"));
        assert!(!is_opencode_ai_endpoint("https://openai.com/v1"));
        assert!(!is_opencode_ai_endpoint("https://api.openai.com/v1"));
        assert!(!is_opencode_ai_endpoint("not a url"));
    }

    #[test]
    fn build_provider_genai_chat_options_should_skip_prompt_cache_key_for_openai_compatible() {
        let api_config = ResolvedApiConfig {
            provider_id: None,
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            reasoning_effort: None,
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-1".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAI, false, false, None);

        assert_eq!(options.prompt_cache_key, None);
        assert_eq!(options.cache_control, None);
    }

    #[test]
    fn build_provider_genai_chat_options_should_use_prompt_cache_key_for_openai_responses() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("responses-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAIResponses,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-responses".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAIResp, true, true, None);

        assert_eq!(
            options.prompt_cache_key.as_deref(),
            Some("conversation-responses")
        );
        assert_eq!(options.cache_control, None);
    }

    #[test]
    fn build_provider_genai_chat_options_should_use_prompt_cache_key_for_codex() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-codex".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAIResp, true, true, None);

        assert_eq!(options.prompt_cache_key.as_deref(), Some("conversation-codex"));
        assert_eq!(options.cache_control, None);
    }

    #[test]
    fn build_provider_genai_chat_options_should_set_low_verbosity_for_codex() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAIResp, true, true, None);

        assert_eq!(
            options.extra_body,
            Some(serde_json::json!({
                "text": {
                    "verbosity": "low",
                }
            }))
        );
    }

    #[test]
    fn genai_response_id_provider_meta_should_use_nested_genai_key() {
        let meta = genai_response_id_provider_meta(Some(" resp_123 "))
            .expect("response id provider meta should be created");

        assert_eq!(
            meta.get("genai")
                .and_then(|value| value.get("responseId"))
                .and_then(Value::as_str),
            Some("resp_123")
        );
    }

    #[test]
    fn build_provider_genai_chat_options_should_disable_reasoning_for_codex_spark() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("codex-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::Codex,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5.3-codex-spark".to_string(),
            reasoning_effort: Some("high".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: Some("conversation-codex".to_string()),
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAIResp, true, true, None);

        assert_eq!(options.capture_reasoning_content, Some(false));
        assert!(options.reasoning_effort.is_none());
    }

    #[test]
    fn build_provider_genai_chat_options_should_disable_reasoning_capture_for_deepseek_none() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("deepseek-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::DeepSeek,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.deepseek.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "deepseek-chat".to_string(),
            reasoning_effort: Some("none".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::DeepSeek, true, true, None);

        assert_eq!(options.capture_reasoning_content, Some(false));
        // DeepSeek 已由 genai managed_body_thinking 管理：none 透传为 Zero，
        // 由 genai 生成 thinking.type=disabled，本项目不再手动注入。
        assert!(matches!(
            options.reasoning_effort,
            Some(genai::chat::ReasoningEffort::Zero)
        ));
        assert_eq!(options.extra_body, None);
    }

    #[test]
    fn build_provider_genai_chat_options_should_pass_zero_effort_for_deepseek_responses_none() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("deepseek-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAIResponses,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.deepseek.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "deepseek-v4-pro".to_string(),
            reasoning_effort: Some("none".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(
            &api_config,
            genai::adapter::AdapterKind::OpenAIResp,
            true,
            true,
            None,
        );

        // Responses 协议：none 透传为 Zero，由 genai 生成 reasoning.effort=none；
        // 不注入 Chat Completions 的 thinking.type=disabled。
        assert!(matches!(
            options.reasoning_effort,
            Some(genai::chat::ReasoningEffort::Zero)
        ));
        assert_eq!(options.extra_body, None);
    }

    #[test]
    fn build_provider_genai_chat_options_should_set_thinking_disabled_for_moonshot_none() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("moonshot-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.moonshot.cn/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "kimi-k2.5".to_string(),
            reasoning_effort: Some("none".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAI, true, true, None);

        assert!(options.reasoning_effort.is_none());
        assert_eq!(
            options.extra_body,
            Some(serde_json::json!({
                "thinking": {
                    "type": "disabled",
                }
            }))
        );
    }

    #[test]
    fn build_provider_genai_chat_options_should_set_thinking_disabled_for_doubao_none() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("doubao-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://ark.cn-beijing.volces.com/api/v3".to_string(),
            api_key: "test-key".to_string(),
            model: "doubao-seed-1-6-thinking".to_string(),
            reasoning_effort: Some("none".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAI, true, true, None);

        assert!(options.reasoning_effort.is_none());
        assert_eq!(
            options.extra_body,
            Some(serde_json::json!({
                "thinking": {
                    "type": "disabled",
                }
            }))
        );
    }

    #[test]
    fn build_provider_genai_chat_options_should_not_set_thinking_disabled_for_generic_openai_none() {
        let api_config = ResolvedApiConfig {
            provider_id: Some("openai-provider".to_string()),
            provider_api_keys: Vec::new(),
            provider_key_cursor: 0,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-5".to_string(),
            reasoning_effort: Some("none".to_string()),
            temperature: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            extra_headers: Vec::new(),
            codex_auth: None,
            codex_custom_api_key: None,
        };

        let options = build_provider_genai_chat_options(&api_config, genai::adapter::AdapterKind::OpenAI, true, true, None);

        assert_eq!(options.extra_body, None);
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
    fn build_genai_chat_request_should_filter_empty_assistant_history() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: String::new(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, genai::chat::ChatRole::User);
        assert_eq!(request.messages[0].content.texts(), vec!["继续"]);
    }

    #[test]
    fn build_genai_chat_request_should_keep_reasoning_only_assistant_history() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: String::new(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: Some("只保留推理上下文".to_string()),
            }],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, genai::chat::ChatRole::Assistant);
        assert_eq!(
            request.messages[0].content.reasoning_contents(),
            vec!["只保留推理上下文"]
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
    fn build_genai_chat_request_should_align_preview_and_provider_request_for_shared_tool_fixture() {
        let assistant = ChatMessage {
            id: "assistant-tool-fixture".to_string(),
            role: "assistant".to_string(),
            created_at: "2026-05-08T12:00:00Z".to_string(),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: "终端版本是 PowerShell 7.5.4。".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(vec![
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
                    "tool_calls": [{
                        "id": "call_1",
                        "call_id": "provider_call_1",
                        "type": "function",
                        "function": {
                            "name": "exec",
                            "arguments": "{\"command\":\"pwsh --version\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "PowerShell 7.5.4"
                }),
            ]),
            mcp_call: None,
        meme_annotations: None,
        };
        let prepared = prepared_prompt_from_aggregated_assistant(&assistant);

        let preview_messages = prepared_prompt_to_messages_json(&prepared);
        let provider_request = build_provider_genai_request(&prepared)
            .expect("build_provider_genai_request should succeed");
        let preview_canonical =
            canonical_preview_assistant_and_tool_messages(&preview_messages);
        let provider_canonical =
            canonical_provider_assistant_and_tool_messages(&provider_request);

        assert_eq!(preview_canonical, provider_canonical);
        assert_eq!(
            preview_canonical
                .last()
                .and_then(|message| message.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("我已经拿到工具结果，现在直接回答用户终端版本。")
        );
        assert_eq!(
            preview_canonical
                .first()
                .and_then(|message| message.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("先调用终端工具查看 PowerShell 版本。")
        );
    }

    #[test]
    fn build_provider_genai_request_should_keep_openai_image_payload_as_standard_base64() {
        let prepared = prepared_prompt_with_single_image_path_and_base64();

        let provider_request = build_provider_genai_request(&prepared)
            .expect("build_provider_genai_request should succeed");
        let binaries = provider_request.messages[0].content.binaries();

        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0].content_type, "image/png");
        assert_eq!(binaries[0].name, None);
        assert!(matches!(
            &binaries[0].source,
            genai::chat::BinarySource::Base64(value)
                if value.as_ref() == "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO9Wl1QAAAAASUVORK5CYII="
        ));
    }

    #[test]
    fn build_provider_genai_request_should_keep_gemini_image_payload_as_standard_base64() {
        let prepared = prepared_prompt_with_single_image_path_and_base64();
        let adapter_kind = resolve_provider_genai_adapter_kind(
            &ResolvedApiConfig {
                provider_id: Some("gemini-provider".to_string()),
                provider_api_keys: Vec::new(),
                provider_key_cursor: 0,
                request_format: RequestFormat::Gemini,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
                api_key: "test-key".to_string(),
                model: "gemini-2.5-flash".to_string(),
                reasoning_effort: None,
                temperature: None,
                max_output_tokens: None,
                prompt_cache_key: None,
                extra_headers: Vec::new(),
                codex_auth: None,
                codex_custom_api_key: None,
            },
            "gemini-2.5-flash",
            genai::adapter::AdapterKind::Gemini,
        );
        let provider_request = build_provider_genai_request(&prepared)
            .expect("build_provider_genai_request should succeed");
        let binaries = provider_request.messages[0].content.binaries();

        assert_eq!(adapter_kind, genai::adapter::AdapterKind::Gemini);
        assert_eq!(binaries.len(), 1);
        assert!(matches!(&binaries[0].source, genai::chat::BinarySource::Base64(_)));
    }

    #[test]
    fn build_provider_genai_request_should_keep_anthropic_image_payload_as_standard_base64() {
        let prepared = prepared_prompt_with_single_image_path_and_base64();
        let adapter_kind = resolve_provider_genai_adapter_kind(
            &ResolvedApiConfig {
                provider_id: Some("anthropic-provider".to_string()),
                provider_api_keys: Vec::new(),
                provider_key_cursor: 0,
                request_format: RequestFormat::Anthropic,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key: "test-key".to_string(),
                model: "claude-3-7-sonnet".to_string(),
                reasoning_effort: None,
                temperature: None,
                max_output_tokens: None,
                prompt_cache_key: None,
                extra_headers: Vec::new(),
                codex_auth: None,
                codex_custom_api_key: None,
            },
            "claude-3-7-sonnet",
            genai::adapter::AdapterKind::Anthropic,
        );
        let provider_request = build_provider_genai_request(&prepared)
            .expect("build_provider_genai_request should succeed");
        let binaries = provider_request.messages[0].content.binaries();

        assert_eq!(adapter_kind, genai::adapter::AdapterKind::Anthropic);
        assert_eq!(binaries.len(), 1);
        assert!(matches!(&binaries[0].source, genai::chat::BinarySource::Base64(_)));
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

    #[test]
    fn normalize_minimax_genai_base_url_should_preserve_prefix_and_append_v1() {
        assert_eq!(
            normalize_minimax_genai_base_url(""),
            "https://api.minimax.io/anthropic/v1/"
        );
        assert_eq!(
            normalize_minimax_genai_base_url("https://api.minimax.io"),
            "https://api.minimax.io/v1/"
        );
        assert_eq!(
            normalize_minimax_genai_base_url("https://api.minimaxi.com/anthropic"),
            "https://api.minimaxi.com/anthropic/v1/"
        );
        assert_eq!(
            normalize_minimax_genai_base_url("https://example.com/custom/prefix"),
            "https://example.com/custom/prefix/v1/"
        );
        assert_eq!(
            normalize_minimax_genai_base_url("https://api.minimax.io/anthropic/v1/messages"),
            "https://api.minimax.io/anthropic/v1/"
        );
    }
}
