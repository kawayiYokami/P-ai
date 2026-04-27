const INTERNAL_MAX_TOOL_LOOP_ROUNDS: usize = 100;
const REPEATED_TOOL_CALL_BLOCK_THRESHOLD: usize = 10;

struct GenaiToolLoopRoundOutput {
    turn_text: String,
    turn_reasoning: String,
    turn_tool_calls: Vec<genai::chat::ToolCall>,
    trusted_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ToolRepeatGuard {
    last_tool_name: String,
    last_args_signature: String,
    same_call_streak: usize,
}

fn canonical_json_signature(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .map(canonical_json_signature)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{}]", parts)
        }
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let parts = keys
                .iter()
                .map(|key| {
                    let key_text =
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    let value_text = map
                        .get(key)
                        .map(canonical_json_signature)
                        .unwrap_or_else(|| "null".to_string());
                    format!("{key_text}:{value_text}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{parts}}}")
        }
    }
}

fn normalized_tool_args_signature(tool_args: &str) -> String {
    let trimmed = tool_args.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => canonical_json_signature(&value),
        Err(_) => trimmed.to_string(),
    }
}

fn register_tool_repeat_attempt(
    guard: &mut ToolRepeatGuard,
    tool_name: &str,
    tool_args: &str,
) -> usize {
    let next_signature = normalized_tool_args_signature(tool_args);
    if guard.last_tool_name == tool_name && guard.last_args_signature == next_signature {
        guard.same_call_streak = guard.same_call_streak.saturating_add(1);
    } else {
        guard.last_tool_name = tool_name.to_string();
        guard.last_args_signature = next_signature;
        guard.same_call_streak = 1;
    }
    guard.same_call_streak
}

#[derive(Debug, Clone)]
struct ToolLoopAutoCompactionContext {
    conversation_id: String,
    data_path: PathBuf,
    request_id: Option<String>,
    prompt_mode: PromptBuildMode,
    agent: AgentProfile,
    agents: Vec<AgentProfile>,
    departments: Vec<DepartmentConfig>,
    user_name: String,
    user_intro: String,
    response_style_id: String,
    ui_language: String,
    last_archive_summary: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
    enable_pdf_images: bool,
    trusted_prompt_usage: std::sync::Arc<std::sync::Mutex<Option<TrustedPromptUsage>>>,
}

fn tool_loop_transient_tool_history_message(events: &[Value]) -> Option<ChatMessage> {
    if events.is_empty() {
        return None;
    }
    Some(ChatMessage {
        id: "tool_loop_transient_tool_history".to_string(),
        role: "assistant".to_string(),
        created_at: String::new(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: String::new(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: None,
        tool_call: Some(events.to_vec()),
        mcp_call: None,
    })
}

fn append_tool_loop_transient_history_to_prepared(
    prepared: &mut PreparedPrompt,
    transient_tool_history: &[Value],
) {
    let Some(message) = tool_loop_transient_tool_history_message(transient_tool_history) else {
        return;
    };
    prepared.history_messages.extend(
        build_prepared_history_messages_from_tool_history(
            &message,
            MessageToolHistoryView::PromptReplay,
        ),
    );
    normalize_prepared_history_messages_in_place(prepared);
}

fn tool_loop_guided_close_reply(
    reasoning_standard: String,
    tool_history_events: Vec<Value>,
    trusted_input_tokens: Option<u64>,
) -> ModelReply {
    ModelReply {
        assistant_text: String::new(),
        final_response_text: String::new(),
        reasoning_standard,
        reasoning_inline: String::new(),
        assistant_provider_meta: Some(serde_json::json!({
            "dispatchCloseReason": "guided_queue_ready"
        })),
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
    }
}

fn send_text_delta_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let _ = on_delta.send(AssistantDeltaEvent {
        delta: text.to_string(),
        kind: None,
        request_id: None,
        activation_id: None,
        phase_id: None,
        reason: None,
        tool_name: None,
        tool_status: None,
        tool_args: None,
        message: None,
    });
}

fn tool_loop_active_conversation_snapshot(
    state: &AppState,
    conversation_id: &str,
) -> Result<Option<Conversation>, String> {
    conversation_service().try_read_unarchived_conversation(state, conversation_id)
}

fn build_tool_loop_prepared_for_continuation(
    state: &AppState,
    context: &ToolLoopAutoCompactionContext,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    transient_tool_history: &[Value],
) -> Result<Option<(Conversation, PreparedPrompt)>, String> {
    let Some(conversation) =
        tool_loop_active_conversation_snapshot(state, &context.conversation_id)?
    else {
        return Ok(None);
    };
    let mut prepared = build_prepared_prompt_for_mode(
        context.prompt_mode,
        &conversation,
        &context.agent,
        &context.agents,
        &context.departments,
        &context.user_name,
        &context.user_intro,
        &context.response_style_id,
        &context.ui_language,
        Some(&state.data_path),
        context.last_archive_summary.as_deref(),
        None,
        context.chat_overrides.clone(),
        Some(state),
        Some(selected_api),
        Some(resolved_api),
        Some(context.enable_pdf_images),
    );
    append_tool_loop_transient_history_to_prepared(&mut prepared, transient_tool_history);
    Ok(Some((conversation, prepared)))
}

fn organize_context_succeeded(tool_name: &str, tool_result: &str) -> bool {
    if tool_name != "organize_context" {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(tool_result) else {
        return false;
    };
    value
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && value
            .get("applied")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn should_stop_after_contact_tool(tool_name: &str, tool_result: &str) -> bool {
    if !matches!(
        tool_name,
        "remote_im_send" | "contact_reply" | "contact_send_files" | "contact_no_reply"
    ) {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(tool_result) else {
        return false;
    };
    if let Some(status) = value.get("status").and_then(Value::as_str) {
        let normalized = status.trim().to_ascii_lowercase();
        if normalized == "done" {
            return true;
        }
        if normalized == "continue" {
            return false;
        }
    }
    value
        .get("stop_tool_loop")
        .and_then(Value::as_bool)
        .or_else(|| value.get("done").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn contact_tool_should_run_last(tool_name: &str, tool_args: &str) -> bool {
    if tool_name == "contact_no_reply" {
        return true;
    }
    if tool_name != "remote_im_send" {
        return false;
    }
    let Ok(value) = serde_json::from_str::<Value>(tool_args) else {
        return false;
    };
    let Some(status) = value.get("status").and_then(Value::as_str) else {
        return false;
    };
    status.trim().eq_ignore_ascii_case("done")
}

fn reorder_turn_tool_calls_for_contact_tail(
    tool_calls: Vec<genai::chat::ToolCall>,
) -> Vec<genai::chat::ToolCall> {
    let mut normal = Vec::<genai::chat::ToolCall>::new();
    let mut tail_calls = Vec::<genai::chat::ToolCall>::new();
    for tool_call in tool_calls {
        let tool_args = match &tool_call.fn_arguments {
            Value::String(raw) => raw.as_str(),
            other => {
                let serialized = other.to_string();
                if contact_tool_should_run_last(&tool_call.fn_name, &serialized) {
                    tail_calls.push(tool_call);
                } else {
                    normal.push(tool_call);
                }
                continue;
            }
        };
        if contact_tool_should_run_last(&tool_call.fn_name, tool_args) {
            tail_calls.push(tool_call);
        } else {
            normal.push(tool_call);
        }
    }
    normal.extend(tail_calls);
    normal
}

fn remote_im_result_action(tool_result: &str) -> Option<String> {
    serde_json::from_str::<Value>(tool_result)
        .ok()
        .and_then(|value| value.get("action").and_then(Value::as_str).map(str::to_string))
}

fn json_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
    })
}

struct TerminalToolResultMessage {
    assistant_text: String,
    completion_text: Option<String>,
    provider_meta: Option<Value>,
}

fn terminal_task_complete_result(tool_name: &str, tool_args: &str, tool_result: &ProviderToolResult) -> Option<String> {
    if tool_name != "task" || tool_result.is_error {
        return None;
    }

    let args_value = serde_json::from_str::<Value>(tool_args).ok()?;
    let action = json_string_field(&args_value, &["action"])?;
    if !action.eq_ignore_ascii_case("complete") {
        return None;
    }

    let result_value = serde_json::from_str::<Value>(&tool_result.display_text).ok();
    let completion_conclusion = json_string_field(
        &args_value,
        &["completion_conclusion", "completionConclusion"],
    )
    .or_else(|| {
        result_value.as_ref().and_then(|value| {
            json_string_field(value, &["completionConclusion", "completion_conclusion"])
        })
    });
    let completion_state = json_string_field(
        &args_value,
        &["completion_state", "completionState"],
    )
    .or_else(|| {
        result_value
            .as_ref()
            .and_then(|value| json_string_field(value, &["completionState", "completion_state"]))
    })
    .unwrap_or_default();

    Some(completion_conclusion.unwrap_or_else(|| {
        if completion_state.eq_ignore_ascii_case("failed_completed") {
            "任务已按失败结束。".to_string()
        } else if completion_state.eq_ignore_ascii_case("completed") {
            "任务已完成。".to_string()
        } else {
            "任务已结束。".to_string()
        }
    }))
}

fn terminal_plan_result(
    tool_name: &str,
    tool_args: &str,
    tool_result: &ProviderToolResult,
) -> Option<TerminalToolResultMessage> {
    if tool_name != "plan" || tool_result.is_error {
        return None;
    }

    let args_value = serde_json::from_str::<Value>(tool_args).ok();
    let result_value = serde_json::from_str::<Value>(&tool_result.display_text).ok();
    let action = args_value
        .as_ref()
        .and_then(|value| json_string_field(value, &["action"]))
        .or_else(|| result_value.as_ref().and_then(|value| json_string_field(value, &["action"])))?;
    let normalized_action = action.to_ascii_lowercase();
    let context = args_value
        .as_ref()
        .and_then(|value| json_string_field(value, &["context"]))
        .or_else(|| result_value.as_ref().and_then(|value| json_string_field(value, &["context"])))
        .unwrap_or_else(|| "计划已完成。".to_string());

    let message_kind = match normalized_action.as_str() {
        "present" => "plan_present",
        "complete" => "plan_complete",
        _ => return None,
    };

    Some(TerminalToolResultMessage {
        assistant_text: String::new(),
        completion_text: if normalized_action == "complete" {
            Some(context.clone())
        } else {
            None
        },
        provider_meta: Some(serde_json::json!({
            "messageKind": message_kind,
            "planCard": {
                "action": action,
                "context": context,
            },
            "message_meta": {
                "kind": message_kind,
            }
        })),
    })
}

fn tool_history_without_organize_context(events: Vec<Value>) -> Vec<Value> {
    let mut filtered = Vec::<Value>::new();
    let mut skip_next_tool = false;
    for event in events {
        let role = event.get("role").and_then(Value::as_str).unwrap_or_default();
        if role == "assistant" {
            let has_organize_call = event
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|calls| {
                    calls.iter().any(|call| {
                        call.get("function")
                            .and_then(|func| func.get("name"))
                            .and_then(Value::as_str)
                            .map(|name| name == "organize_context")
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if has_organize_call {
                skip_next_tool = true;
                continue;
            }
        }
        if skip_next_tool && role == "tool" {
            skip_next_tool = false;
            continue;
        }
        skip_next_tool = false;
        filtered.push(event);
    }
    filtered
}

fn sync_completed_tool_history_cache(
    state: Option<&AppState>,
    chat_session_key: &str,
    events: &[Value],
) {
    let Some(state) = state else {
        return;
    };
    if let Err(err) = replace_inflight_completed_tool_history(state, chat_session_key, events) {
        eprintln!(
            "[聊天] 同步已完成工具历史缓存失败 (session={}): {}",
            chat_session_key, err
        );
    }
}

async fn call_tool_with_user_abort<F, T, E>(
    app_state: Option<&AppState>,
    chat_session_key: &str,
    future: F,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    if let Some(state) = app_state {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        register_inflight_tool_abort_handle(state, chat_session_key, abort_handle)?;
        let result = futures_util::future::Abortable::new(future, abort_registration).await;
        if let Err(err) = clear_inflight_tool_abort_handle(state, chat_session_key) {
            eprintln!(
                "[聊天] 清理进行中工具中断句柄失败 (session={}): {}",
                chat_session_key, err
            );
        }
        match result {
            Ok(inner) => inner.map_err(|err| err.to_string()),
            Err(_) => {
                eprintln!(
                    "[聊天] 用户中止工具调用 (session={})",
                    chat_session_key
                );
                Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
            }
        }
    } else {
        future.await.map_err(|err| err.to_string())
    }
}

async fn runtime_tool_definitions_for_genai(
    tools: &[Box<dyn RuntimeToolDyn>],
    protocol_family: ToolCallProtocolFamily,
) -> Result<Vec<genai::chat::Tool>, String> {
    let mut out = Vec::<genai::chat::Tool>::new();
    for tool in tools {
        let definition = tool.definition().await;
        let mut genai_tool = genai::chat::Tool::new(definition.name);
        if !definition.description.trim().is_empty() {
            genai_tool = genai_tool.with_description(definition.description);
        }
        let mut parameters = definition.parameters;
        if matches!(protocol_family, ToolCallProtocolFamily::Gemini) {
            gemini_to_openapi_schema(&mut parameters);
        }
        genai_tool = genai_tool.with_schema(parameters);
        out.push(genai_tool);
    }
    Ok(out)
}

async fn call_runtime_tool_by_name(
    tools: &[Box<dyn RuntimeToolDyn>],
    tool_name: &str,
    tool_args: &str,
) -> Result<ProviderToolResult, String> {
    let Some(tool) = tools.iter().find(|tool| tool.name() == tool_name) else {
        return Err(format!("未找到工具：{tool_name}"));
    };
    tool.call_json(tool_args.to_string()).await
}

fn runtime_tool_result_followup_message(
    tool_name: &str,
    tool_result: &ProviderToolResult,
) -> Option<genai::chat::ChatMessage> {
    let mut forwarded_parts = Vec::<genai::chat::ContentPart>::new();

    for part in &tool_result.parts {
        match part {
            ProviderToolResultPart::Text { .. } => {}
            ProviderToolResultPart::Image { mime, data_base64 } => {
                forwarded_parts.push(genai::chat::ContentPart::from_binary_base64(
                    mime.clone(),
                    data_base64.clone(),
                    None,
                ));
            }
            ProviderToolResultPart::Audio { mime, data_base64 } => {
                forwarded_parts.push(genai::chat::ContentPart::from_binary_base64(
                    mime.clone(),
                    data_base64.clone(),
                    None,
                ));
            }
            ProviderToolResultPart::Resource { mime, uri, text } => {
                let mut lines = vec![format!("工具 `{tool_name}` 返回了资源内容。")];
                if let Some(uri) = uri.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                    lines.push(format!("resource uri: {uri}"));
                }
                if let Some(mime) = mime
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    lines.push(format!("resource mime: {mime}"));
                }
                if !text.trim().is_empty() {
                    lines.push(text.clone());
                }
                forwarded_parts.push(genai::chat::ContentPart::from_text(lines.join("\n")));
            }
        }
    }

    if forwarded_parts.is_empty() {
        return None;
    }

    let mut parts = vec![genai::chat::ContentPart::from_text(format!(
        "工具 `{tool_name}` 返回了额外模态内容，以下内容已继续提供给模型。"
    ))];
    parts.extend(forwarded_parts);
    Some(genai::chat::ChatMessage::user(
        genai::chat::MessageContent::from_parts(parts),
    ))
}

fn build_genai_message_state(
    prepared: &PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
    force_tool_reasoning_content: bool,
    request_stage: &str,
) -> Result<(Option<String>, Vec<genai::chat::ChatMessage>), String> {
    let request = build_provider_genai_request(
        prepared,
        protocol_family,
        force_tool_reasoning_content,
        request_stage,
    )?;
    Ok((request.system, request.messages))
}

async fn maybe_apply_auto_compaction_before_tool_continue_genai(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    transient_tool_history: &[Value],
) -> Result<bool, String> {
    let Some(state) = state else {
        return Ok(false);
    };
    let Some(context) = context else {
        return Ok(false);
    };
    if transient_tool_history.is_empty() {
        return Ok(false);
    }

    let Some((source, prepared_before)) = build_tool_loop_prepared_for_continuation(
        state,
        context,
        selected_api,
        resolved_api,
        transient_tool_history,
    )?
    else {
        runtime_log_info(format!(
            "[聊天] 工具续调前上下文整理检查 跳过 conversation_id={} 原因=会话不存在或已归档",
            context.conversation_id
        ));
        return Ok(false);
    };

    let usage = conversation_prompt_service().consume_shared_trusted_prompt_usage_or_estimate(
        &context.trusted_prompt_usage,
        &prepared_before,
        selected_api,
        &context.agent,
    );
    let (decision, decision_source) = decide_archive_before_send_from_usage(
        &usage,
        source.last_user_at.as_deref(),
        archive_pipeline_has_assistant_reply(&source),
    );
    if !decision.should_archive {
        runtime_log_info(format!(
            "[聊天] 工具续调前上下文整理检查 跳过 conversation_id={} usage_ratio={:.4} source={} reason={}",
            context.conversation_id, decision.usage_ratio, decision_source, decision.reason
        ));
        return Ok(false);
    }

    let _ = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        request_id: None,
        activation_id: None,
        phase_id: None,
        reason: None,
        tool_name: Some("archive".to_string()),
        tool_status: Some("running".to_string()),
        tool_args: None,
        message: Some("上下文接近上限，正在整理后继续执行...".to_string()),
    });

    let archive_res = run_context_compaction_pipeline(
        state,
        selected_api,
        resolved_api,
        &source,
        &context.agent.id,
        &decision.reason,
        "COMPACTION-BEFORE-TOOL-CONTINUE",
    )
    .await;

    let archive_result = match archive_res {
        Ok(result) => result,
        Err(err) => {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: String::new(),
                kind: Some("tool_status".to_string()),
                request_id: None,
                activation_id: None,
                phase_id: None,
                reason: None,
                tool_name: Some("archive".to_string()),
                tool_status: Some("failed".to_string()),
                tool_args: None,
                message: Some(format!("自动整理失败：{err}")),
            });
            return Err(format!("自动整理失败：{err}"));
        }
    };

    if let Some(warning) = archive_result
        .warning
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_log_warn(format!(
            "[聊天] 工具续调前上下文整理 完成 conversation_id={} warning={}",
            context.conversation_id, warning
        ));
    } else {
        runtime_log_info(format!(
            "[聊天] 工具续调前上下文整理 完成 conversation_id={} usage_ratio_before={:.4} source={} reason={} forced={} effective_prompt_tokens={} estimated_prompt_tokens={}",
            context.conversation_id,
            usage.usage_ratio,
            decision_source,
            decision.reason,
            decision.forced,
            usage.effective_prompt_tokens,
            usage.estimated_prompt_tokens.unwrap_or(0)
        ));
    }

    let _ = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        request_id: None,
        activation_id: None,
        phase_id: None,
        reason: None,
        tool_name: Some("archive".to_string()),
        tool_status: Some("done".to_string()),
        tool_args: None,
        message: Some("整理完成，正在重新开始当前调度...".to_string()),
    });

    Err(CHAT_DISPATCH_RESTART_AFTER_COMPACTION.to_string())
}

async fn run_genai_tool_loop(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    protocol_family: ToolCallProtocolFamily,
    adapter_kind: genai::adapter::AdapterKind,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    _max_tool_iterations: usize,
    include_reasoning_before_tool_calls: bool,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
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
        .with_capture_reasoning_content(include_reasoning_before_tool_calls)
        .with_capture_tool_calls(true)
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

    let genai_tools = runtime_tool_definitions_for_genai(&tool_assembly.tools, protocol_family).await?;
    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let (system_prompt, mut messages) = build_genai_message_state(
        &prepared,
        protocol_family,
        matches!(adapter_kind, genai::adapter::AdapterKind::DeepSeek),
        "run_genai_tool_loop.stream",
    )?;

    let mut auto_compaction_applied = false;
    let mut tool_repeat_guard = ToolRepeatGuard::default();
    for round_index in 0..INTERNAL_MAX_TOOL_LOOP_ROUNDS {
        let mut emit_text_boundary_before_next_chunk = !full_assistant_text.trim().is_empty();
        if round_index > 0 && !auto_compaction_applied {
            auto_compaction_applied = maybe_apply_auto_compaction_before_tool_continue_genai(
                tool_abort_state,
                auto_compaction_context,
                selected_api,
                resolved_api,
                on_delta,
                &tool_history_events,
            )
            .await?;
        }

        let mut stop_after_remote_im_done_in_turn = false;
        let round_output = async {
            let _provider_serial_guard = maybe_acquire_provider_serial_guard(
                tool_abort_state,
                &api_config,
                model_name,
            )
            .await?;
            let mut turn_text = String::new();
            let mut turn_reasoning = String::new();
            let mut turn_tool_calls = Vec::<genai::chat::ToolCall>::new();
            let mut round_trusted_input_tokens = None;

            let mut stream = {
                let mut request = genai::chat::ChatRequest::from_messages(messages.clone());
                if let Some(system) = system_prompt
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    request = request.with_system(system.to_string());
                }
                if !genai_tools.is_empty() {
                    request = request.with_tools(genai_tools.clone());
                }
                client
                    .exec_chat_stream(service_target.clone(), request, Some(&options))
                    .await
                    .map_err(|err| format!("GenAI 流式请求构建失败：{err}"))?
                    .stream
            };

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(genai::chat::ChatStreamEvent::Start) => {}
                    Ok(genai::chat::ChatStreamEvent::Chunk(text)) => {
                        if emit_text_boundary_before_next_chunk && !text.content.is_empty() {
                            send_text_delta_event(on_delta, "\n");
                            emit_text_boundary_before_next_chunk = false;
                        }
                        send_text_delta_event(on_delta, &text.content);
                        turn_text.push_str(&text.content);
                    }
                    Ok(genai::chat::ChatStreamEvent::ReasoningChunk(reasoning)) => {
                        if !reasoning.content.is_empty() {
                            turn_reasoning.push_str(&reasoning.content);
                            full_reasoning_standard.push_str(&reasoning.content);
                            let _ = on_delta.send(AssistantDeltaEvent {
                                delta: reasoning.content,
                                kind: Some("reasoning_standard".to_string()),
                                request_id: None,
                                activation_id: None,
                                phase_id: None,
                                reason: None,
                                tool_name: None,
                                tool_status: None,
                                tool_args: None,
                                message: None,
                            });
                        }
                    }
                    Ok(genai::chat::ChatStreamEvent::ThoughtSignatureChunk(_)) => {}
                    Ok(genai::chat::ChatStreamEvent::ToolCallChunk(_)) => {}
                    Ok(genai::chat::ChatStreamEvent::End(end)) => {
                        round_trusted_input_tokens = end
                            .captured_usage
                            .as_ref()
                            .and_then(|usage| usage.prompt_tokens)
                            .and_then(|value| u64::try_from(value).ok())
                            .filter(|value| *value > 0);
                        if turn_text.is_empty() {
                            if let Some(captured_texts) = end
                                .captured_content
                                .as_ref()
                                .map(|content| content.texts())
                                .filter(|texts| !texts.is_empty())
                            {
                                let joined = captured_texts.join("\n");
                                turn_text = joined.clone();
                                if emit_text_boundary_before_next_chunk && !joined.is_empty() {
                                    send_text_delta_event(on_delta, "\n");
                                    emit_text_boundary_before_next_chunk = false;
                                }
                                send_text_delta_event(on_delta, &joined);
                            }
                        }
                        if turn_reasoning.is_empty() {
                            if let Some(captured_reasoning) = end
                                .captured_reasoning_content
                                .as_deref()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                            {
                                turn_reasoning = captured_reasoning.to_string();
                                if full_reasoning_standard.is_empty() {
                                    full_reasoning_standard = captured_reasoning.to_string();
                                }
                            }
                        }
                        if let Some(captured_content) = end.captured_content.as_ref() {
                            turn_tool_calls = captured_content
                                .tool_calls()
                                .into_iter()
                                .cloned()
                                .collect::<Vec<_>>();
                        }
                    }
                    Err(err) => return Err(format!("GenAI 流式处理失败：{err}")),
                }
            }

            Ok::<GenaiToolLoopRoundOutput, String>(GenaiToolLoopRoundOutput {
                turn_text,
                turn_reasoning,
                turn_tool_calls,
                trusted_input_tokens: round_trusted_input_tokens,
            })
        }
        .await?;
        let GenaiToolLoopRoundOutput {
            turn_text,
            turn_reasoning,
            turn_tool_calls,
            trusted_input_tokens: round_trusted_input_tokens,
        } = round_output;
        trusted_input_tokens = round_trusted_input_tokens;

        if let Some(context) = auto_compaction_context {
            conversation_prompt_service().refresh_shared_trusted_prompt_usage(
                &context.trusted_prompt_usage,
                trusted_input_tokens,
                selected_api,
            );
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        let turn_tool_calls = reorder_turn_tool_calls_for_contact_tail(turn_tool_calls);

        if turn_tool_calls.is_empty() {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                final_response_text: turn_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                assistant_provider_meta: None,
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }

        let mut assistant_parts = Vec::<genai::chat::ContentPart>::new();
        for tool_call in &turn_tool_calls {
            assistant_parts.push(genai::chat::ContentPart::ToolCall(tool_call.clone()));
        }
        let mut assistant_message = genai::chat::ChatMessage::assistant(
            genai::chat::MessageContent::from_parts(assistant_parts),
        );
        if include_reasoning_before_tool_calls {
            assistant_message =
                assistant_message.with_reasoning_content(Some(turn_reasoning.trim().to_string()));
        }
        messages.push(assistant_message);

        for tool_call in turn_tool_calls {
            let genai::chat::ToolCall {
                call_id: tool_call_id,
                fn_name: tool_name,
                fn_arguments,
                ..
            } = tool_call;
            let tool_args = match fn_arguments {
                Value::String(raw) => raw,
                other => other.to_string(),
            };
            let repeat_streak =
                register_tool_repeat_attempt(&mut tool_repeat_guard, &tool_name, &tool_args);
            send_stream_rebind_required_event(
                on_delta,
                auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                "tool_start",
            );
            send_tool_status_event(
                on_delta,
                &tool_name,
                "running",
                Some(tool_args.as_str()),
                &format!("正在调用工具：{}", tool_name),
            );

            let tool_result = if repeat_streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD {
                let err_text = format!(
                    "工具调用已被系统阻止：相同工具与相同参数已连续调用 {} 次，请调整参数或停止调用。",
                    REPEATED_TOOL_CALL_BLOCK_THRESHOLD
                );
                runtime_log_info(format!(
                    "[聊天] 工具循环触发重复调用熔断: session={}, tool_name={}, streak={}, args={}",
                    chat_session_key, tool_name, repeat_streak, tool_args
                ));
                send_tool_status_event(
                    on_delta,
                    &tool_name,
                    "failed",
                    Some(tool_args.as_str()),
                    &err_text,
                );
                ProviderToolResult::error(tool_failure_result_json(&tool_name, &err_text))
            } else {
                match call_tool_with_user_abort(
                    tool_abort_state,
                    chat_session_key,
                    call_runtime_tool_by_name(&tool_assembly.tools, &tool_name, &tool_args),
                )
                .await
                {
                    Ok(output) => {
                        let status_message = if output.is_error {
                            format!("工具返回错误结果：{}", tool_name)
                        } else {
                            format!("工具调用完成：{}", tool_name)
                        };
                        send_tool_status_event(
                            on_delta,
                            &tool_name,
                            if output.is_error { "failed" } else { "done" },
                            None,
                            &status_message,
                        );
                        output
                    }
                    Err(err) => {
                        if err == CHAT_ABORTED_BY_USER_ERROR {
                            eprintln!(
                                "[聊天] 收到停止请求，立即退出工具循环 (session={})",
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
                        ProviderToolResult::error(tool_failure_result_json(&tool_name, &err_text))
                    }
                }
            };
            let tool_result_text = tool_result.display_text.as_str();

            let tc_json = serde_json::json!({
                "id": tool_call_id,
                "call_id": tool_call_id,
                "type": "function",
                "function": {
                    "name": tool_name,
                    "arguments": tool_args
                }
            });
            let mut assistant_tool_event = serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [tc_json]
            });
            if include_reasoning_before_tool_calls {
                if let Some(object) = assistant_tool_event.as_object_mut() {
                    object.insert(
                        "reasoning_content".to_string(),
                        Value::String(turn_reasoning.trim().to_string()),
                    );
                }
            }
            tool_history_events.push(assistant_tool_event);
            let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result_text);
            tool_history_events.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": history_content
            }));
            sync_completed_tool_history_cache(
                tool_abort_state,
                chat_session_key,
                &tool_history_events,
            );

            if tool_loop_should_close_for_guided_queue(tool_abort_state, auto_compaction_context) {
                runtime_log_info(format!(
                    "[引导投送] 工具轮次完成后闭合当前调度: session={}, tool_name={}",
                    chat_session_key, tool_name
                ));
                return Ok(tool_loop_guided_close_reply(
                    full_reasoning_standard,
                    tool_history_events,
                    trusted_input_tokens,
                ));
            }

            if organize_context_succeeded(&tool_name, &tool_result_text) {
                return Ok(ModelReply {
                    assistant_text: String::new(),
                    final_response_text: String::new(),
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: None,
                    tool_history_events: tool_history_without_organize_context(tool_history_events),
                    suppress_assistant_message: true,
                    trusted_input_tokens: None,
                });
            }
            if let Some(final_text) =
                terminal_task_complete_result(&tool_name, &tool_args, &tool_result)
            {
                return Ok(ModelReply {
                    assistant_text: final_text.clone(),
                    final_response_text: final_text,
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: None,
                    tool_history_events,
                    suppress_assistant_message: false,
                    trusted_input_tokens,
                });
            }
            if let Some(plan_result) = terminal_plan_result(&tool_name, &tool_args, &tool_result) {
                if plan_result
                    .provider_meta
                    .as_ref()
                    .and_then(|meta| meta.get("messageKind"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == Some("plan_complete")
                {
                    if let Some(context) = auto_compaction_context {
                        if let Err(err) = message_store::active_plan_complete_latest_in_progress(
                            &context.data_path,
                            &context.conversation_id,
                            plan_result.completion_text.as_deref(),
                        ) {
                            runtime_log_warn(format!(
                                "[计划] 完成状态写入失败 conversation_id={} error={}",
                                context.conversation_id, err
                            ));
                        }
                    }
                }
                return Ok(ModelReply {
                    assistant_text: plan_result.assistant_text.clone(),
                    final_response_text: plan_result.assistant_text,
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: plan_result.provider_meta,
                    tool_history_events,
                    suppress_assistant_message: false,
                    trusted_input_tokens,
                });
            }
            if should_stop_after_contact_tool(&tool_name, &tool_result_text) {
                stop_after_remote_im_done_in_turn = true;
            }

            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result_text);
            messages.push(genai::chat::ChatMessage::from(
                genai::chat::ToolResponse::new(tool_call_id, tool_result_for_model),
            ));
            if let Some(message) = runtime_tool_result_followup_message(&tool_name, &tool_result) {
                messages.push(message);
            }
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        images: payload.images.clone(),
                        created_seq: 0,
                    },
                );
                let mut forwarded_parts =
                    vec![genai::chat::ContentPart::from_text(notice)];
                forwarded_parts.extend(cached.images.iter().map(|image| {
                    genai::chat::ContentPart::from_binary_base64(
                        image.mime.clone(),
                        image.base64.clone(),
                        None,
                    )
                }));
                messages.push(genai::chat::ChatMessage::user(
                    genai::chat::MessageContent::from_parts(forwarded_parts),
                ));
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotImageCount": cached.images.len()
                }));
                sync_completed_tool_history_cache(
                    tool_abort_state,
                    chat_session_key,
                    &tool_history_events,
                );
            }
            if stop_after_remote_im_done_in_turn {
                break;
            }
        }

        if stop_after_remote_im_done_in_turn {
            let final_text = if full_assistant_text.trim().is_empty() {
                match tool_history_events
                    .iter()
                    .rev()
                    .find_map(|event| {
                        event.get("role")
                            .and_then(Value::as_str)
                            .filter(|role| *role == "tool")?;
                        event.get("content")
                            .and_then(Value::as_str)
                            .and_then(remote_im_result_action)
                    })
                    .as_deref()
                {
                    Some("no_reply") => "本轮决定不回复。".to_string(),
                    _ => "已发送完成。".to_string(),
                }
            } else {
                full_assistant_text.clone()
            };
            return Ok(ModelReply {
                assistant_text: final_text.clone(),
                final_response_text: final_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                assistant_provider_meta: None,
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        None,
        "工具循环触发内部安全上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        final_response_text: String::new(),
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        assistant_provider_meta: None,
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}

async fn execute_genai_non_stream_round(
    client: &genai::Client,
    service_target: &genai::ServiceTarget,
    request: genai::chat::ChatRequest,
    options: &genai::chat::ChatOptions,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    prefix_text_boundary: bool,
) -> Result<GenaiToolLoopRoundOutput, String> {
    let response = client
        .exec_chat(service_target.clone(), request, Some(options))
        .await
        .map_err(|err| format!("GenAI 非流式请求失败：{err}"))?;
    let turn_text = response
        .texts()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let turn_reasoning = response.reasoning_content.clone().unwrap_or_default();
    let turn_tool_calls = response.tool_calls().into_iter().cloned().collect::<Vec<_>>();
    let trusted_input_tokens = response
        .usage
        .prompt_tokens
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0);

    if !turn_reasoning.is_empty() {
        let _ = on_delta.send(AssistantDeltaEvent {
            delta: turn_reasoning.clone(),
            kind: Some("reasoning_standard".to_string()),
            request_id: None,
            activation_id: None,
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            message: None,
        });
    }
    if !turn_text.is_empty() {
        if prefix_text_boundary {
            send_text_delta_event(on_delta, "\n");
        }
        send_text_delta_event(on_delta, &turn_text);
    }

    Ok(GenaiToolLoopRoundOutput {
        turn_text,
        turn_reasoning,
        turn_tool_calls,
        trusted_input_tokens,
    })
}

async fn run_genai_tool_loop_non_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    protocol_family: ToolCallProtocolFamily,
    adapter_kind: genai::adapter::AdapterKind,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    _max_tool_iterations: usize,
    include_reasoning_before_tool_calls: bool,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String> {
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let client = genai::Client::builder().build();
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
        .with_capture_reasoning_content(include_reasoning_before_tool_calls)
        .with_capture_tool_calls(true)
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

    let genai_tools = runtime_tool_definitions_for_genai(&tool_assembly.tools, protocol_family).await?;
    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let (system_prompt, mut messages) = build_genai_message_state(
        &prepared,
        protocol_family,
        matches!(adapter_kind, genai::adapter::AdapterKind::DeepSeek),
        "run_genai_tool_loop_non_stream",
    )?;

    let mut auto_compaction_applied = false;
    let mut tool_repeat_guard = ToolRepeatGuard::default();
    for round_index in 0..INTERNAL_MAX_TOOL_LOOP_ROUNDS {
        if round_index > 0 && !auto_compaction_applied {
            auto_compaction_applied = maybe_apply_auto_compaction_before_tool_continue_genai(
                tool_abort_state,
                auto_compaction_context,
                selected_api,
                resolved_api,
                on_delta,
                &tool_history_events,
            )
            .await?;
        }

        let mut stop_after_remote_im_done_in_turn = false;
        let round = {
            let mut request = genai::chat::ChatRequest::from_messages(messages.clone());
            if let Some(system) = system_prompt
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                request = request.with_system(system.to_string());
            }
            if !genai_tools.is_empty() {
                request = request.with_tools(genai_tools.clone());
            }
            let _provider_serial_guard = maybe_acquire_provider_serial_guard(
                tool_abort_state,
                &api_config,
                model_name,
            )
            .await?;
            execute_genai_non_stream_round(
                &client,
                &service_target,
                request,
                &options,
                on_delta,
                !full_assistant_text.trim().is_empty(),
            )
            .await?
        };
        let turn_text = round.turn_text;
        let turn_reasoning = round.turn_reasoning;
        let turn_tool_calls = reorder_turn_tool_calls_for_contact_tail(round.turn_tool_calls);
        if let Some(value) = round.trusted_input_tokens {
            trusted_input_tokens = Some(value);
        }
        if !turn_reasoning.is_empty() {
            full_reasoning_standard.push_str(&turn_reasoning);
        }

        if !turn_text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&turn_text);
        }

        if turn_tool_calls.is_empty() {
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                final_response_text: turn_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                assistant_provider_meta: None,
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }

        let mut assistant_parts = Vec::<genai::chat::ContentPart>::new();
        for tool_call in &turn_tool_calls {
            assistant_parts.push(genai::chat::ContentPart::ToolCall(tool_call.clone()));
        }
        let mut assistant_message = genai::chat::ChatMessage::assistant(
            genai::chat::MessageContent::from_parts(assistant_parts),
        );
        if include_reasoning_before_tool_calls {
            assistant_message =
                assistant_message.with_reasoning_content(Some(turn_reasoning.trim().to_string()));
        }
        messages.push(assistant_message);

        for tool_call in turn_tool_calls {
            let genai::chat::ToolCall {
                call_id: tool_call_id,
                fn_name: tool_name,
                fn_arguments,
                ..
            } = tool_call;
            let tool_args = match fn_arguments {
                Value::String(raw) => raw,
                other => other.to_string(),
            };
            let repeat_streak =
                register_tool_repeat_attempt(&mut tool_repeat_guard, &tool_name, &tool_args);
            send_stream_rebind_required_event(
                on_delta,
                auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                "tool_start",
            );
            send_tool_status_event(
                on_delta,
                &tool_name,
                "running",
                Some(tool_args.as_str()),
                &format!("正在调用工具：{}", tool_name),
            );

            let tool_result = if repeat_streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD {
                let err_text = format!(
                    "工具调用已被系统阻止：相同工具与相同参数已连续调用 {} 次，请调整参数或停止调用。",
                    REPEATED_TOOL_CALL_BLOCK_THRESHOLD
                );
                runtime_log_info(format!(
                    "[聊天] 工具循环触发重复调用熔断: session={}, tool_name={}, streak={}, args={}",
                    chat_session_key, tool_name, repeat_streak, tool_args
                ));
                send_tool_status_event(
                    on_delta,
                    &tool_name,
                    "failed",
                    Some(tool_args.as_str()),
                    &err_text,
                );
                ProviderToolResult::error(tool_failure_result_json(&tool_name, &err_text))
            } else {
                match call_tool_with_user_abort(
                    tool_abort_state,
                    chat_session_key,
                    call_runtime_tool_by_name(&tool_assembly.tools, &tool_name, &tool_args),
                )
                .await
                {
                    Ok(output) => {
                        let status_message = if output.is_error {
                            format!("工具返回错误结果：{}", tool_name)
                        } else {
                            format!("工具调用完成：{}", tool_name)
                        };
                        send_tool_status_event(
                            on_delta,
                            &tool_name,
                            if output.is_error { "failed" } else { "done" },
                            None,
                            &status_message,
                        );
                        output
                    }
                    Err(err) => {
                        if err == CHAT_ABORTED_BY_USER_ERROR {
                            eprintln!(
                                "[聊天] 收到停止请求，立即退出工具循环 (session={})",
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
                        ProviderToolResult::error(tool_failure_result_json(&tool_name, &err_text))
                    }
                }
            };
            let tool_result_text = tool_result.display_text.as_str();
            let tc_json = serde_json::json!({
                "id": tool_call_id,
                "call_id": tool_call_id,
                "type": "function",
                "function": {
                    "name": tool_name,
                    "arguments": tool_args
                }
            });
            let mut assistant_tool_event = serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [tc_json]
            });
            if include_reasoning_before_tool_calls {
                if let Some(object) = assistant_tool_event.as_object_mut() {
                    object.insert(
                        "reasoning_content".to_string(),
                        Value::String(turn_reasoning.trim().to_string()),
                    );
                }
            }
            tool_history_events.push(assistant_tool_event);
            let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result_text);
            tool_history_events.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": history_content
            }));

            if tool_loop_should_close_for_guided_queue(tool_abort_state, auto_compaction_context) {
                runtime_log_info(format!(
                    "[引导投送] 工具轮次完成后闭合当前非流式调度: session={}, tool_name={}",
                    chat_session_key, tool_name
                ));
                return Ok(tool_loop_guided_close_reply(
                    full_reasoning_standard,
                    tool_history_events,
                    trusted_input_tokens,
                ));
            }

            if organize_context_succeeded(&tool_name, &tool_result_text) {
                return Ok(ModelReply {
                    assistant_text: String::new(),
                    final_response_text: String::new(),
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: None,
                    tool_history_events: tool_history_without_organize_context(tool_history_events),
                    suppress_assistant_message: true,
                    trusted_input_tokens: None,
                });
            }
            if let Some(final_text) =
                terminal_task_complete_result(&tool_name, &tool_args, &tool_result)
            {
                return Ok(ModelReply {
                    assistant_text: final_text.clone(),
                    final_response_text: final_text,
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: None,
                    tool_history_events,
                    suppress_assistant_message: false,
                    trusted_input_tokens,
                });
            }
            if let Some(plan_result) = terminal_plan_result(&tool_name, &tool_args, &tool_result) {
                if plan_result
                    .provider_meta
                    .as_ref()
                    .and_then(|meta| meta.get("messageKind"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    == Some("plan_complete")
                {
                    if let Some(context) = auto_compaction_context {
                        if let Err(err) = message_store::active_plan_complete_latest_in_progress(
                            &context.data_path,
                            &context.conversation_id,
                            plan_result.completion_text.as_deref(),
                        ) {
                            runtime_log_warn(format!(
                                "[计划] 完成状态写入失败 conversation_id={} error={}",
                                context.conversation_id, err
                            ));
                        }
                    }
                }
                return Ok(ModelReply {
                    assistant_text: plan_result.assistant_text.clone(),
                    final_response_text: plan_result.assistant_text,
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    assistant_provider_meta: plan_result.provider_meta,
                    tool_history_events,
                    suppress_assistant_message: false,
                    trusted_input_tokens,
                });
            }
            if should_stop_after_contact_tool(&tool_name, &tool_result_text) {
                stop_after_remote_im_done_in_turn = true;
            }

            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result_text);
            messages.push(genai::chat::ChatMessage::from(
                genai::chat::ToolResponse::new(tool_call_id, tool_result_for_model),
            ));
            if let Some(message) = runtime_tool_result_followup_message(&tool_name, &tool_result) {
                messages.push(message);
            }
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        images: payload.images.clone(),
                        created_seq: 0,
                    },
                );
                let mut forwarded_parts =
                    vec![genai::chat::ContentPart::from_text(notice)];
                forwarded_parts.extend(cached.images.iter().map(|image| {
                    genai::chat::ContentPart::from_binary_base64(
                        image.mime.clone(),
                        image.base64.clone(),
                        None,
                    )
                }));
                messages.push(genai::chat::ChatMessage::user(
                    genai::chat::MessageContent::from_parts(forwarded_parts),
                ));
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotImageCount": cached.images.len()
                }));
            }
            if stop_after_remote_im_done_in_turn {
                break;
            }
        }

        if stop_after_remote_im_done_in_turn {
            let final_text = if full_assistant_text.trim().is_empty() {
                match tool_history_events
                    .iter()
                    .rev()
                    .find_map(|event| {
                        event.get("role")
                            .and_then(Value::as_str)
                            .filter(|role| *role == "tool")?;
                        event.get("content")
                            .and_then(Value::as_str)
                            .and_then(remote_im_result_action)
                    })
                    .as_deref()
                {
                    Some("no_reply") => "本轮决定不回复。".to_string(),
                    _ => "已发送完成。".to_string(),
                }
            } else {
                full_assistant_text.clone()
            };
            return Ok(ModelReply {
                assistant_text: final_text.clone(),
                final_response_text: final_text,
                reasoning_standard: full_reasoning_standard,
                reasoning_inline: String::new(),
                assistant_provider_meta: None,
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        None,
        "工具循环触发内部安全上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        final_response_text: String::new(),
        reasoning_standard: full_reasoning_standard,
        reasoning_inline: String::new(),
        assistant_provider_meta: None,
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}

#[cfg(test)]
mod tool_loop_tests {
    use super::*;

    #[test]
    fn contact_no_reply_should_stop_on_snake_case_stop_tool_loop() {
        let tool_result = serde_json::json!({
            "ok": true,
            "action": "no_reply",
            "stop_tool_loop": true
        })
        .to_string();

        assert!(should_stop_after_contact_tool("contact_no_reply", &tool_result));
    }

    #[test]
    fn reorder_turn_tool_calls_should_move_contact_no_reply_to_tail() {
        let tool_calls = vec![
            genai::chat::ToolCall {
                call_id: "call-1".to_string(),
                fn_name: "contact_no_reply".to_string(),
                fn_arguments: serde_json::json!({
                    "reason": "不需要回复"
                }),
                thought_signatures: None,
            },
            genai::chat::ToolCall {
                call_id: "call-2".to_string(),
                fn_name: "fetch".to_string(),
                fn_arguments: serde_json::json!({
                    "url": "https://example.com"
                }),
                thought_signatures: None,
            },
            genai::chat::ToolCall {
                call_id: "call-3".to_string(),
                fn_name: "contact_reply".to_string(),
                fn_arguments: serde_json::json!({
                    "text": "收到"
                }),
                thought_signatures: None,
            },
        ];

        let reordered = reorder_turn_tool_calls_for_contact_tail(tool_calls);
        let names = reordered
            .iter()
            .map(|item| format!("{}:{}", item.fn_name, item.call_id))
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "fetch:call-2".to_string(),
                "contact_reply:call-3".to_string(),
                "contact_no_reply:call-1".to_string(),
            ]
        );
    }

    #[test]
    fn terminal_task_complete_result_prefers_completion_conclusion_from_args() {
        let tool_result = ProviderToolResult::text(
            serde_json::json!({
                "taskId": "task-1",
                "completionState": "completed",
                "completionConclusion": "工具结果里的结论"
            })
            .to_string(),
        );

        let final_text = terminal_task_complete_result(
            "task",
            r#"{"action":"complete","task_id":"task-1","completion_state":"completed","completion_conclusion":"用户应直接看到这句"}"#,
            &tool_result,
        );

        assert_eq!(final_text.as_deref(), Some("用户应直接看到这句"));
    }

    #[test]
    fn terminal_task_complete_result_can_fall_back_to_tool_result_json() {
        let tool_result = ProviderToolResult::text(
            serde_json::json!({
                "taskId": "task-1",
                "completionState": "failed_completed",
                "completionConclusion": "因为缺少权限，任务已按失败结束"
            })
            .to_string(),
        );

        let final_text = terminal_task_complete_result(
            "task",
            r#"{"action":"complete","task_id":"task-1","completion_state":"failed_completed"}"#,
            &tool_result,
        );

        assert_eq!(
            final_text.as_deref(),
            Some("因为缺少权限，任务已按失败结束")
        );
    }

    #[test]
    fn terminal_task_complete_result_ignores_non_complete_actions() {
        let tool_result = ProviderToolResult::text(
            serde_json::json!({
                "taskId": "task-1",
                "completionState": "completed",
                "completionConclusion": "不会被使用"
            })
            .to_string(),
        );

        let final_text = terminal_task_complete_result(
            "task",
            r#"{"action":"update","task_id":"task-1"}"#,
            &tool_result,
        );

        assert_eq!(final_text, None);
    }

    #[test]
    fn rejected_exec_result_should_remain_a_tool_result_instead_of_ending_the_round() {
        let tool_result = ProviderToolResult::text(
            serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "absolute_path_not_granted",
                "message": "写入类命令只能作用于已配置工作目录；未纳管绝对路径仅允许读取。"
            })
            .to_string(),
        );

        assert_eq!(
            terminal_task_complete_result(
                "exec",
                r#"{"command":"echo hi > E:\\outside.txt"}"#,
                &tool_result,
            ),
            None
        );
        assert!(
            terminal_plan_result(
                "exec",
                r#"{"command":"echo hi > E:\\outside.txt"}"#,
                &tool_result,
            )
            .is_none()
        );

        let history_content = sanitize_tool_result_for_history("exec", &tool_result.display_text);
        assert!(history_content.contains("\"approved\":false"));
        assert!(history_content.contains("absolute_path_not_granted"));
        assert!(history_content.contains("已配置工作目录"));
        assert!(!history_content.contains("本轮调度已终止"));
    }

    #[test]
    fn guided_close_reply_should_be_visible_model_reply_with_tool_history() {
        let reply = tool_loop_guided_close_reply(
            String::new(),
            vec![serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            })],
            Some(12),
        );

        assert!(model_reply_has_visible_content(&reply));
        assert!(reply.assistant_text.is_empty());
        assert_eq!(reply.tool_history_events.len(), 1);
        assert_eq!(reply.trusted_input_tokens, Some(12));
        assert!(reply.assistant_provider_meta.is_some());
    }

    #[test]
    fn normalized_tool_args_signature_should_ignore_json_key_order() {
        let left = normalized_tool_args_signature(r#"{"b":2,"a":1}"#);
        let right = normalized_tool_args_signature(r#"{"a":1,"b":2}"#);

        assert_eq!(left, right);
    }

    #[test]
    fn tool_repeat_guard_should_block_after_ten_identical_calls() {
        let mut guard = ToolRepeatGuard::default();
        let mut streak = 0usize;
        for _ in 0..11 {
            streak = register_tool_repeat_attempt(&mut guard, "read_file", r#"{"path":"a.txt"}"#);
        }

        assert_eq!(streak, 11);
        assert!(streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD);
    }

    #[test]
    fn tool_repeat_guard_should_reset_when_args_change() {
        let mut guard = ToolRepeatGuard::default();
        for _ in 0..4 {
            let _ = register_tool_repeat_attempt(&mut guard, "read_file", r#"{"path":"a.txt"}"#);
        }

        let streak = register_tool_repeat_attempt(&mut guard, "read_file", r#"{"path":"b.txt"}"#);

        assert_eq!(streak, 1);
    }

    #[test]
    fn tool_repeat_guard_should_reset_when_tool_changes() {
        let mut guard = ToolRepeatGuard::default();
        for _ in 0..4 {
            let _ = register_tool_repeat_attempt(&mut guard, "read_file", r#"{"path":"a.txt"}"#);
        }

        let streak = register_tool_repeat_attempt(&mut guard, "exec", r#"{"command":"dir"}"#);

        assert_eq!(streak, 1);
    }

    #[test]
    fn append_tool_loop_transient_history_to_prepared_should_expand_tool_events() {
        let mut prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "xcap",
                        "arguments": "{\"method\":\"list_windows\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
        ];

        append_tool_loop_transient_history_to_prepared(&mut prepared, &events);

        assert_eq!(prepared.history_messages.len(), 2);
        assert_eq!(prepared.history_messages[0].role, "assistant");
        assert!(prepared.history_messages[0].tool_calls.is_some());
        assert_eq!(prepared.history_messages[1].role, "tool");
        assert_eq!(
            prepared.history_messages[1].tool_call_id.as_deref(),
            Some("call_1")
        );
    }
}
