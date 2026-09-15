const INTERNAL_MAX_TOOL_LOOP_ROUNDS: usize = 10000;
const REPEATED_TOOL_CALL_BLOCK_THRESHOLD: usize = 3;

/// 统计请求构成的三类字符数（系统提示词 / 工具 schema / 正文消息），
/// 供前端词元账单按字符比例分配真实 usage 展示。
/// system/tools 属于固定成本：压缩不释放。
fn genai_request_context_chars(
    system_prompt: Option<&str>,
    tools: &[genai::chat::Tool],
    messages: &[genai::chat::ChatMessage],
) -> (usize, usize, usize) {
    let system_chars = system_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count())
        .unwrap_or(0);
    let tools_chars = if tools.is_empty() {
        0
    } else {
        serde_json::to_string(tools)
            .map(|value| value.chars().count())
            .unwrap_or(0)
    };
    let message_chars = messages
        .iter()
        .flat_map(|message| message.content.parts())
        .map(|part| match part {
            genai::chat::ContentPart::Text(text) => text.chars().count(),
            genai::chat::ContentPart::ToolCall(tool_call) => tool_call
                .fn_arguments
                .to_string()
                .chars()
                .count(),
            genai::chat::ContentPart::ToolResponse(tool_response) => {
                tool_response.content.chars().count()
            }
            _ => 0,
        })
        .sum();
    (system_chars, tools_chars, message_chars)
}

/// 按字符比例把真实 prompt_tokens 分配到三类，供前端词元账单展示。
fn genai_request_context_breakdown_patch(
    system_chars: usize,
    tools_chars: usize,
    message_chars: usize,
    prompt_tokens: u64,
) -> Option<Value> {
    let total_chars = system_chars + tools_chars + message_chars;
    if total_chars == 0 || prompt_tokens == 0 {
        return None;
    }
    let split = |chars: usize| -> u64 {
        if chars == 0 {
            return 0;
        }
        (u128::from(chars as u64) * u128::from(prompt_tokens) / u128::from(total_chars as u64)) as u64
    };
    let system_tokens = split(system_chars);
    let tools_tokens = split(tools_chars);
    let message_tokens = split(message_chars);
    Some(serde_json::json!({
        "contextBreakdown": {
            "systemTokens": system_tokens,
            "toolsTokens": tools_tokens,
            "messageTokens": message_tokens,
        }
    }))
}

struct GenaiToolLoopRoundOutput {
    turn_text: String,
    turn_reasoning: String,
    reasoning_delta_emitted: bool,
    turn_tool_calls: Vec<genai::chat::ToolCall>,
    trusted_input_tokens: Option<u64>,
    usage: Option<Value>,
    assistant_provider_meta: Option<Value>,
}

#[derive(Debug, Clone)]
struct PreparedToolCall {
    tool_call_id: String,
    tool_name: String,
    tool_args: String,
}

#[derive(Debug)]
struct ExecutedToolCall {
    tool_call_id: String,
    tool_name: String,
    tool_args: String,
    tool_result: ProviderToolResult,
}

#[derive(Debug)]
struct PreparedToolCallBatch {
    calls: Vec<PreparedToolCall>,
}

fn tool_result_history_event(
    tool_call_id: &str,
    content: String,
    metadata: &ProviderToolMetadata,
) -> Value {
    let mut event = serde_json::json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": content,
    });
    if let Ok(value) = serde_json::to_value(metadata) {
        if value.as_object().is_some_and(|object| !object.is_empty()) {
            if let Some(object) = event.as_object_mut() {
                object.insert("metadata".to_string(), value);
            }
        }
    }
    event
}

include!("tool_loop/repeat_guard.rs");
include!("tool_loop/tool_output_store.rs");

fn tool_loop_round_tool_calls_json(tool_calls: &[genai::chat::ToolCall]) -> Vec<Value> {
    tool_calls
        .iter()
        .map(|tool_call| {
            serde_json::json!({
                "id": tool_call.call_id.clone(),
                "call_id": tool_call.call_id.clone(),
                "type": "function",
                "function": {
                    "name": tool_call.fn_name.clone(),
                    "arguments": match &tool_call.fn_arguments {
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    }
                }
            })
        })
        .collect::<Vec<_>>()
}

fn tool_loop_assistant_message(
    turn_text: &str,
    turn_tool_calls: &[genai::chat::ToolCall],
    turn_reasoning: &str,
) -> genai::chat::ChatMessage {
    let mut assistant_parts = turn_tool_calls
        .first()
        .and_then(|tool_call| tool_call.thought_signatures.as_ref())
        .into_iter()
        .flatten()
        .cloned()
        .map(genai::chat::ContentPart::ThoughtSignature)
        .collect::<Vec<_>>();
    if !turn_text.is_empty() {
        assistant_parts.push(genai::chat::ContentPart::from_text(turn_text.to_string()));
    }
    assistant_parts.extend(
        turn_tool_calls
            .iter()
            .cloned()
            .map(genai::chat::ContentPart::ToolCall),
    );
    genai::chat::ChatMessage::assistant(genai::chat::MessageContent::from_parts(
        assistant_parts,
    ))
    .with_reasoning_content(Some(turn_reasoning.trim().to_string()))
}

fn tool_loop_round_response_value(
    turn_text: &str,
    turn_reasoning: &str,
    turn_tool_calls: &[genai::chat::ToolCall],
    usage: Option<&Value>,
) -> Value {
    let mut response = serde_json::json!({
        "assistantText": turn_text,
        "reasoningContent": turn_reasoning,
        "toolCalls": tool_loop_round_tool_calls_json(turn_tool_calls)
    });
    if let Some(usage) = usage {
        if let Some(map) = response.as_object_mut() {
            map.insert("usage".to_string(), usage.clone());
        }
    }
    response
}

// 调度事件：工具循环每一轮模型请求开始。逐轮打点，LogTab 才能按轮拆开。
fn push_tool_loop_round_start(
    state: Option<&AppState>,
    chat_session_key: &str,
    provider_name: &str,
    model_name: &str,
) {
    let Some(state) = state else {
        return;
    };
    let detail = serde_json::json!({
        "modelName": model_name,
        "providerName": provider_name,
    });
    let _ = schedule_event_push_to_latest_run(
        state,
        chat_session_key,
        "model_round_start",
        0,
        None,
        detail,
    );
}

// 调度事件：工具循环每一轮模型响应结束。只带该轮自己的正文/思考/工具，不做累计。
fn push_tool_loop_round_log(
    state: Option<&AppState>,
    chat_session_key: &str,
    model_name: &str,
    response: Value,
    elapsed_ms: u64,
) {
    let Some(state) = state else {
        return;
    };
    let assistant_text = response
        .get("assistantText")
        .and_then(Value::as_str)
        .unwrap_or("");
    let reasoning_text = response
        .get("reasoningContent")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tool_calls = response.get("toolCalls").and_then(Value::as_array);
    let tool_call_count = tool_calls.map(Vec::len).unwrap_or(0);
    let mut detail = serde_json::json!({
        "modelName": model_name,
        "elapsedMs": elapsed_ms,
        "assistantTextLength": assistant_text.chars().count(),
        "reasoningLength": reasoning_text.chars().count(),
        "toolCallCount": tool_call_count,
    });
    if let Some(obj) = detail.as_object_mut() {
        if !assistant_text.trim().is_empty() {
            obj.insert("textPreview".to_string(), serde_json::json!(assistant_text));
        }
        if !reasoning_text.trim().is_empty() {
            obj.insert("reasoningPreview".to_string(), serde_json::json!(reasoning_text));
        }
        if let Some(usage) = response.get("usage") {
            obj.insert("usage".to_string(), usage.clone());
        }
        if let Some(calls) = tool_calls {
            let mut names: Vec<String> = Vec::new();
            for call in calls {
                if let Some(name) = log_tool_call_name(call) {
                    push_unique_log_name(&mut names, name);
                }
            }
            if !names.is_empty() {
                obj.insert("toolCallNames".to_string(), log_tool_call_names_value(names));
            }
        }
    }
    let _ = schedule_event_push_to_latest_run(
        state,
        chat_session_key,
        "model_round_end",
        0,
        Some(true),
        detail,
    );
}

#[derive(Debug, Clone)]
struct ToolLoopAutoCompactionContext {
    conversation_id: String,
    request_id: Option<String>,
    /// 不能从会话级流缓存回读：远程应答委托允许同会话并发。
    assistant_message_id: Option<String>,
    remote_im_reply_delegate_id: Option<String>,
    remote_im_auto_send_source: Option<RemoteImActivationSource>,
    prompt_mode: PromptBuildMode,
    agent: AgentProfile,
    agents: Vec<AgentProfile>,
    user_name: String,
    user_intro: String,
    response_style_id: String,
    ui_language: String,
    chat_overrides: Option<ChatPromptOverrides>,
    trusted_prompt_usage: std::sync::Arc<std::sync::Mutex<Option<TrustedPromptUsage>>>,
    /// 写入前闸门命中压缩时，把压缩保留消息交回外层调度。
    compaction_preserved_messages:
        std::sync::Arc<std::sync::Mutex<Option<CompactionPreservedMessages>>>,
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
                reasoning_content: None,
            }],
        extra_text_blocks: Vec::new(),
        provider_meta: None,
        tool_call: Some(events.to_vec()),
        mcp_call: None,
        meme_annotations: None,
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
    activity_reasoning_text: String,
    tool_history_events: Vec<Value>,
    trusted_input_tokens: Option<u64>,
) -> ModelReply {
    ModelReply {
        assistant_text: String::new(),
        final_response_text: String::new(),
        activity_reasoning_text,
        assistant_provider_meta: Some(serde_json::json!({
            "dispatchCloseReason": "guided_queue_ready"
        })),
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage: None,
        round_logs_recorded_internally: true,
    }
}

#[derive(Debug, Clone)]
enum DeferredToolLoopOutcome {
    PlanPresent(TerminalToolResultMessage),
}

fn deferred_tool_loop_outcome_from_result(
    tool_name: &str,
    tool_args: &str,
    tool_result: &ProviderToolResult,
) -> Option<DeferredToolLoopOutcome> {
    terminal_plan_present_result(tool_name, tool_args, tool_result)
        .map(DeferredToolLoopOutcome::PlanPresent)
}

fn finalize_deferred_tool_loop_outcome(
    outcome: DeferredToolLoopOutcome,
    full_activity_reasoning_text: String,
    tool_history_events: Vec<Value>,
    trusted_input_tokens: Option<u64>,
) -> ModelReply {
    match outcome {
        DeferredToolLoopOutcome::PlanPresent(plan_result) => {
            ModelReply {
                assistant_text: plan_result.assistant_text.clone(),
                final_response_text: plan_result.assistant_text,
                activity_reasoning_text: full_activity_reasoning_text,
                assistant_provider_meta: plan_result.provider_meta,
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
                usage: None,
                round_logs_recorded_internally: true,
            }
        }
    }
}

include!("tool_loop/tool_event_projection.rs");

fn tool_loop_active_conversation_snapshot(
    state: &AppState,
    conversation_id: &str,
) -> Result<Option<Conversation>, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(None);
    }
    match conversation_service_v2().get_conversation_prompt_context(state, conversation_id) {
        Ok(conversation) => Ok(Some(conversation)),
        Err(err) if err.contains("CONV_NOT_FOUND") || err.contains("not found") || err.contains("不存在") => Ok(None),
        Err(err) => Err(err),
    }
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
        &context.user_name,
        &context.user_intro,
        &context.response_style_id,
        &context.ui_language,
        Some(&state.data_path),
        None,
        None,
        context.chat_overrides.clone(),
        Some(state),
        Some(selected_api),
        Some(resolved_api),
    )?;
    append_tool_loop_transient_history_to_prepared(&mut prepared, transient_tool_history);
    Ok(Some((conversation, prepared)))
}

include!("tool_loop/remote_im_tools.rs");
include!("tool_loop/tool_arg_clean.rs");
include!("tool_loop/tool_result_handling.rs");
include!("tool_loop/runtime_execution.rs");
include!("tool_loop/compaction.rs");
async fn run_genai_tool_loop(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    adapter_kind: genai::adapter::AdapterKind,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    _max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    // 本轮调度内桌面操作提醒最多一次：局部变量随调度结束销毁，无全局状态。
    let mut desktop_notice_sent = false;
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let options = build_provider_genai_chat_options(
        &api_config,
        adapter_kind,
        true,
        true,
        Some(chat_session_key),
    );

    let genai_tools = runtime_tool_definitions_for_genai(&tool_assembly.tool_definitions, adapter_kind).await?;
    let mut full_assistant_text = String::new();
    let mut full_activity_reasoning_text = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut pending_tool_group_result_persists =
        Vec::<tauri::async_runtime::JoinHandle<Result<(), String>>>::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let mut latest_usage = None::<Value>;
    let mut final_assistant_provider_meta_override = None::<Value>;
    let (system_prompt, mut messages) = build_genai_message_state(&prepared)?;
    let context_chars =
        genai_request_context_chars(system_prompt.as_deref(), &genai_tools, &messages);

    let mut auto_compaction_applied = false;
    let mut tool_repeat_guard = ToolRepeatGuard::default();
    for round_index in 0..INTERNAL_MAX_TOOL_LOOP_ROUNDS {
        let round_started_at = std::time::Instant::now();
        let mut emit_text_boundary_before_next_chunk = !full_assistant_text.trim().is_empty();
        if round_index > 0 && !auto_compaction_applied {
            auto_compaction_applied = maybe_apply_auto_compaction_before_tool_continue_genai(
                tool_abort_state,
                auto_compaction_context,
                selected_api,
                resolved_api,
                on_delta,
                &tool_history_events,
                &full_assistant_text,
                &full_activity_reasoning_text,
                chat_session_key,
                &mut pending_tool_group_result_persists,
            )
            .await?;
        }

        let mut stop_after_remote_im_done_in_turn = false;
        runtime_log_info(format!(
            "[聊天] 阶段=tool_loop.round_model_request.start，模式=stream，session={}，轮次={}，模型={}，api_id={}",
            chat_session_key,
            round_index + 1,
            model_name,
            selected_api.id,
        ));
        push_tool_loop_round_start(
            tool_abort_state,
            chat_session_key,
            selected_api.name.as_str(),
            model_name,
        );
        let round_output = async {
            let _provider_concurrency_guard = maybe_acquire_provider_concurrency_guard(
                tool_abort_state,
                &api_config,
                model_name,
            )
            .await?;
            let mut turn_text = String::new();
            let mut turn_reasoning = String::new();
            let mut reasoning_delta_emitted = false;
            let mut turn_tool_calls = Vec::<genai::chat::ToolCall>::new();
            let mut round_trusted_input_tokens = None;
            let mut round_usage = None;
            let mut round_assistant_provider_meta = None::<Value>;

            let mut stream = {
                let mut request = genai::chat::ChatRequest::from_messages(
                    sanitize_genai_messages_before_request(messages.clone(), "genai_tool_loop_stream"),
                );
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
                    .exec_chat_stream(model_spec.clone(), request, Some(&options))
                    .await
                    .map_err(|err| format!("GenAI 流式请求构建失败：{err}"))?
                    .stream
            };

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(genai::chat::ChatStreamEvent::Start) => {}
                    Ok(genai::chat::ChatStreamEvent::Heartbeat) => {}
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
                            full_activity_reasoning_text.push_str(&reasoning.content);
                            send_reasoning_delta_event(on_delta, &reasoning.content);
                            reasoning_delta_emitted = true;
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
                        round_usage = end.captured_usage.as_ref().and_then(genai_usage_to_log_value);
                        round_assistant_provider_meta =
                            genai_response_id_provider_meta(end.captured_response_id.as_deref());
                        if let Some(usage) = round_usage.as_ref() {
                            let usage_provider_key = usage_provider_key_from_api_config(&api_config);
                            add_provider_usage_delta_to_conversation(
                                tool_abort_state,
                                usage_conversation_id,
                                Some(usage_provider_key.as_str()),
                                Some(model_name),
                                usage,
                            );
                        }
                        if turn_text.is_empty() {
                            if let Some(captured_texts) = end
                                .captured_content
                                .as_ref()
                                .map(|content| content.texts())
                                .filter(|texts| !texts.is_empty())
                            {
                                let joined = join_model_text_blocks(captured_texts);
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
                                if full_activity_reasoning_text.is_empty() {
                                    full_activity_reasoning_text = captured_reasoning.to_string();
                                } else {
                                    full_activity_reasoning_text.push_str(captured_reasoning);
                                }
                                send_reasoning_delta_event(on_delta, captured_reasoning);
                                reasoning_delta_emitted = true;
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
                reasoning_delta_emitted,
                turn_tool_calls,
                trusted_input_tokens: round_trusted_input_tokens,
                usage: round_usage,
                assistant_provider_meta: round_assistant_provider_meta,
            })
        }
        .await;
        runtime_log_info(format!(
            "[聊天] 阶段=tool_loop.round_model_request.finish，模式=stream，session={}，轮次={}，模型={}，api_id={}，状态={}，耗时={}ms",
            chat_session_key,
            round_index + 1,
            model_name,
            selected_api.id,
            if round_output.is_ok() { "完成" } else { "失败" },
            round_started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        ));
        let round_output = round_output?;
        let GenaiToolLoopRoundOutput {
            turn_text,
            turn_reasoning,
            reasoning_delta_emitted,
            turn_tool_calls,
            trusted_input_tokens: round_trusted_input_tokens,
            usage: round_usage,
            assistant_provider_meta: round_assistant_provider_meta,
        } = round_output;
        let round_elapsed_ms = round_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        trusted_input_tokens = round_trusted_input_tokens;
        if let Some(usage) = round_usage.as_ref() {
            latest_usage = Some(usage.clone());
        }
        merge_assistant_provider_meta_patch(
            &mut final_assistant_provider_meta_override,
            round_assistant_provider_meta,
        );
        if let Some(prompt_tokens) = trusted_input_tokens {
            merge_assistant_provider_meta_patch(
                &mut final_assistant_provider_meta_override,
                genai_request_context_breakdown_patch(
                    context_chars.0,
                    context_chars.1,
                    context_chars.2,
                    prompt_tokens,
                ),
            );
        }
        push_tool_loop_round_log(
            tool_abort_state,
            chat_session_key,
            model_name,
            tool_loop_round_response_value(&turn_text, &turn_reasoning, &turn_tool_calls, round_usage.as_ref()),
            round_elapsed_ms,
        );

        if let Some(context) = auto_compaction_context {
            runtime_log_info(format!(
                "[聊天] 工具循环刷新缓存 conversation_id={} trusted_input_tokens={:?} context_window_tokens={}",
                context.conversation_id, round_trusted_input_tokens, selected_api.context_window_tokens,
            ));
            conversation_prompt_service().refresh_shared_trusted_prompt_usage(
                &context.trusted_prompt_usage,
                round_trusted_input_tokens,
                selected_api,
            );
        }

        let turn_tool_calls = reorder_turn_tool_calls_for_contact_tail(turn_tool_calls);
        let turn_tool_calls = clean_turn_tool_calls(turn_tool_calls);

        if turn_tool_calls.is_empty() {
            if !turn_text.is_empty() {
                if !full_assistant_text.trim().is_empty() {
                    full_assistant_text.push_str("\n\n");
                }
                full_assistant_text.push_str(&turn_text);
            }
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                final_response_text: turn_text,
                activity_reasoning_text: full_activity_reasoning_text,
                assistant_provider_meta: final_assistant_provider_meta_override.clone(),
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
                usage: latest_usage,
                round_logs_recorded_internally: true,
            });
        }

        let assistant_message =
            tool_loop_assistant_message(&turn_text, &turn_tool_calls, &turn_reasoning);
        messages.push(assistant_message);
        let mut deferred_outcome = None::<DeferredToolLoopOutcome>;
        let mut guided_close_requested = false;
        let assistant_tool_group_history_event = assistant_tool_group_history_event_value(
            &turn_text,
            &turn_tool_calls,
            &turn_reasoning,
            round_trusted_input_tokens,
            selected_api.context_window_tokens,
        );
        let assistant_tool_group_stream_event =
            assistant_tool_group_stream_event_value(&turn_text, &turn_tool_calls);
        if !reasoning_delta_emitted && !turn_reasoning.trim().is_empty() {
            send_reasoning_delta_event(on_delta, turn_reasoning.trim());
        }
        send_assistant_tool_event(on_delta, &assistant_tool_group_stream_event);
        tool_history_events.push(assistant_tool_group_history_event.clone());

        let prepared_turn_tool_calls = turn_tool_calls
            .into_iter()
            .map(prepared_tool_call_from_genai)
            .collect::<Vec<_>>();
        let mut round_completed_tool_result_events = Vec::<Value>::new();
        for batch in split_prepared_tool_calls_into_execution_batches(
            &tool_assembly.tools,
            &tool_assembly.tool_definitions,
            prepared_turn_tool_calls,
        ) {
            let mut executable_calls = Vec::<PreparedToolCall>::new();
            let mut repeat_block = None::<(PreparedToolCall, String)>;
            let mut batch_repeat_signatures = std::collections::HashSet::new();
            for call in batch.calls {
                // 模型即将操作电脑：本轮调度内首次调用 operate 时发一次系统通知（由调度器控制，无全局状态）。
                if !desktop_notice_sent && call.tool_name == "operate" {
                    desktop_notice_sent = true;
                    if let Some(state) = tool_abort_state {
                        if let Ok(args) = serde_json::from_str::<Value>(&call.tool_args) {
                            if let Some(script) = args.get("script").and_then(Value::as_str) {
                                notify_desktop_operation_started(state, script);
                            }
                        }
                    }
                }
                let repeat_streak = register_tool_repeat_attempt_once_per_batch(
                    &mut tool_repeat_guard,
                    &mut batch_repeat_signatures,
                    &call.tool_name,
                    &call.tool_args,
                );
                if repeat_streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD {
                    let err_text = repeated_tool_call_block_message(
                        &call.tool_name,
                        &call.tool_args,
                        repeat_streak,
                    );
                    runtime_log_info(format!(
                        "[聊天] 工具循环触发重复调用熔断: session={}, tool_name={}, streak={}, threshold={}, args={}",
                        chat_session_key, call.tool_name, repeat_streak, REPEATED_TOOL_CALL_BLOCK_THRESHOLD, call.tool_args
                    ));
                    repeat_block = Some((call, err_text));
                    break;
                }
                executable_calls.push(call);
            }

            let executed_tool_calls = execute_prepared_tool_call_group(
                tool_abort_state,
                chat_session_key,
                &tool_assembly.tools,
                on_delta,
                auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                executable_calls,
            )
            .await?;

            for executed_tool_call in executed_tool_calls {
            let ExecutedToolCall {
                tool_call_id,
                tool_name,
                tool_args,
                tool_result,
            } = executed_tool_call;
            let projection = project_provider_tool_result(tool_abort_state, &tool_name, &tool_result);
            let tool_result_event = tool_result_history_event(
                &tool_call_id,
                projection.text.clone(),
                &projection.metadata,
            );
            send_assistant_tool_result_event(on_delta, &tool_result_event);
            insert_before_trailing_user_history_events(
                &mut tool_history_events,
                tool_result_event.clone(),
            );
            // 判定前不写临时账本；仅内存累积本轮事件。
            round_completed_tool_result_events.push(tool_result_event);

            if tool_loop_should_close_for_guided_queue(tool_abort_state, auto_compaction_context) {
                runtime_log_info(format!(
                    "[引导投送] 工具轮次完成后闭合当前调度: session={}, tool_name={}",
                    chat_session_key, tool_name
                ));
                guided_close_requested = true;
            }

            if deferred_outcome.is_none() {
                deferred_outcome =
                    deferred_tool_loop_outcome_from_result(&tool_name, &tool_args, &tool_result);
            }
            if let Some(plan_state) =
                plan_tool_result_state(&tool_name, &tool_args, &tool_result)
            {
                if plan_state.action.eq_ignore_ascii_case("complete") {
                    final_assistant_provider_meta_override = Some(serde_json::json!({
                        "messageKind": "plan_complete",
                        "message_meta": {
                            "kind": "plan_complete",
                        }
                    }));
                }
            }
            if should_stop_after_contact_tool(&tool_name, &tool_result) {
                stop_after_remote_im_done_in_turn = true;
            }

            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result, &projection.text);
            insert_before_trailing_user_messages(
                &mut messages,
                genai::chat::ChatMessage::from(
                    genai::chat::ToolResponse::new(tool_call_id, tool_result_for_model),
                ),
            );
            if let Some(message) = runtime_tool_result_followup_message(
                &tool_name,
                &tool_result,
                screenshot_forward.is_none(),
            ) {
                messages.push(message);
            }
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload, &tool_name);
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
                // 判定前不写临时账本。
            }
            }

            if let Some((call, err_text)) = repeat_block {
                send_stream_rebind_required_event(
                    on_delta,
                    auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                    "tool_start",
                );
                send_tool_status_event(
                    on_delta,
                    &call.tool_name,
                    "running",
                    Some(call.tool_args.as_str()),
                    Some(call.tool_call_id.as_str()),
                    &format!("正在调用工具：{}", call.tool_name),
                );
                send_tool_status_event(
                    on_delta,
                    &call.tool_name,
                    "failed",
                    Some(call.tool_args.as_str()),
                    Some(call.tool_call_id.as_str()),
                    &err_text,
                );
                let history_content = err_text.clone();
                let tool_result_event = serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.tool_call_id,
                    "content": history_content
                });
                send_assistant_tool_result_event(on_delta, &tool_result_event);
                insert_before_trailing_user_history_events(
                    &mut tool_history_events,
                    tool_result_event.clone(),
                );
                sync_completed_tool_history_cache(
                    tool_abort_state,
                    chat_session_key,
                    &tool_history_events,
                );
                return Ok(repeated_tool_call_block_reply(
                    full_activity_reasoning_text,
                    tool_history_events,
                    trusted_input_tokens,
                    err_text,
                ));
            }
        }

        // 工具整轮执行完毕瞬间判定：判定前未写正式历史/临时账本。
        let round_history_start = tool_history_events
            .iter()
            .rposition(|event| {
                event
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|calls| !calls.is_empty())
            })
            .unwrap_or(0);
        let round_history_events = tool_history_events[round_history_start..].to_vec();
        if apply_compaction_preserved_gate_after_tool_round(
            tool_abort_state,
            auto_compaction_context,
            selected_api,
            resolved_api,
            on_delta,
            chat_session_key,
            &mut pending_tool_group_result_persists,
            trusted_input_tokens,
            &turn_text,
            &turn_reasoning,
            &assistant_tool_group_history_event,
            &round_history_events,
            &round_completed_tool_result_events,
        )
        .await?
        {
            return Err(CHAT_DISPATCH_RESTART_AFTER_COMPACTION.to_string());
        }
        // 判定为直写后，才同步临时账本（与旧语义一致）。
        sync_completed_tool_history_cache(
            tool_abort_state,
            chat_session_key,
            &tool_history_events,
        );

        if guided_close_requested {
            return Ok(tool_loop_guided_close_reply(
                full_activity_reasoning_text,
                tool_history_events,
                trusted_input_tokens,
            ));
        }

        if let Some(outcome) = deferred_outcome {
            return Ok(finalize_deferred_tool_loop_outcome(
                outcome,
                full_activity_reasoning_text,
                tool_history_events,
                trusted_input_tokens,
            ));
        }

        if stop_after_remote_im_done_in_turn {
            return Ok(finalize_remote_im_stop_model_reply(
                &full_assistant_text,
                full_activity_reasoning_text,
                final_assistant_provider_meta_override.clone(),
                tool_history_events,
                trusted_input_tokens,
                latest_usage,
            ));
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        None,
        None,
        "工具循环触发内部安全上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        final_response_text: String::new(),
        activity_reasoning_text: full_activity_reasoning_text,
        assistant_provider_meta: final_assistant_provider_meta_override,
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage: latest_usage,
        round_logs_recorded_internally: true,
    })
}

async fn execute_genai_non_stream_round(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    client: &genai::Client,
    model_spec: &genai::ModelSpec,
    request: genai::chat::ChatRequest,
    options: &genai::chat::ChatOptions,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    prefix_text_boundary: bool,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
) -> Result<GenaiToolLoopRoundOutput, String> {
    let response = client
        .exec_chat(model_spec.clone(), request, Some(options))
        .await
        .map_err(|err| format!("GenAI 非流式请求失败：{err}"))?;
    let response_texts = response.texts();
    let turn_text = join_model_text_blocks(response_texts);
    let turn_reasoning = response.reasoning_content.clone().unwrap_or_default();
    let turn_tool_calls = response.tool_calls().into_iter().cloned().collect::<Vec<_>>();
    let trusted_input_tokens = response
        .usage
        .prompt_tokens
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0);
    let usage = genai_usage_to_log_value(&response.usage);
    if let Some(usage) = usage.as_ref() {
        let usage_provider_key = usage_provider_key_from_api_config(&api_config);
        add_provider_usage_delta_to_conversation(
            app_state,
            usage_conversation_id,
            Some(usage_provider_key.as_str()),
            Some(model_name),
            usage,
        );
    }

    if !turn_reasoning.is_empty() {
        send_reasoning_delta_event(on_delta, &turn_reasoning);
    }
    if !turn_text.is_empty() {
        if prefix_text_boundary {
            send_text_delta_event(on_delta, "\n");
        }
        send_text_delta_event(on_delta, &turn_text);
    }

    Ok(GenaiToolLoopRoundOutput {
        turn_text,
        reasoning_delta_emitted: !turn_reasoning.is_empty(),
        turn_reasoning,
        turn_tool_calls,
        trusted_input_tokens,
        usage,
        assistant_provider_meta: genai_response_id_provider_meta(response.response_id.as_deref()),
    })
}

async fn run_genai_tool_loop_non_stream(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    tool_assembly: RuntimeToolAssembly,
    adapter_kind: genai::adapter::AdapterKind,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    _max_tool_iterations: usize,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
    usage_conversation_id: Option<&str>,
) -> Result<ModelReply, String> {
    // 本轮调度内桌面操作提醒最多一次：局部变量随调度结束销毁，无全局状态。
    let mut desktop_notice_sent = false;
    let api_config = resolve_request_api_config(api_config).await?;
    let request_api_key = consume_api_key_for_request(&api_config);
    let service_target = build_provider_genai_service_target(
        &api_config,
        adapter_kind,
        model_name,
        request_api_key.clone(),
    );
    let (client, model_spec) = build_provider_genai_client_and_model_spec_from_target(
        &api_config,
        model_name,
        request_api_key,
        service_target,
    )?;
    let options = build_provider_genai_chat_options(
        &api_config,
        adapter_kind,
        true,
        true,
        Some(chat_session_key),
    );

    let genai_tools = runtime_tool_definitions_for_genai(&tool_assembly.tool_definitions, adapter_kind).await?;
    let mut full_assistant_text = String::new();
    let mut full_activity_reasoning_text = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut pending_tool_group_result_persists =
        Vec::<tauri::async_runtime::JoinHandle<Result<(), String>>>::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let mut latest_usage = None::<Value>;
    let mut final_assistant_provider_meta_override = None::<Value>;
    let (system_prompt, mut messages) = build_genai_message_state(&prepared)?;
    let context_chars =
        genai_request_context_chars(system_prompt.as_deref(), &genai_tools, &messages);

    let mut auto_compaction_applied = false;
    let mut tool_repeat_guard = ToolRepeatGuard::default();
    for round_index in 0..INTERNAL_MAX_TOOL_LOOP_ROUNDS {
        let round_started_at = std::time::Instant::now();
        if round_index > 0 && !auto_compaction_applied {
            auto_compaction_applied = maybe_apply_auto_compaction_before_tool_continue_genai(
                tool_abort_state,
                auto_compaction_context,
                selected_api,
                resolved_api,
                on_delta,
                &tool_history_events,
                &full_assistant_text,
                &full_activity_reasoning_text,
                chat_session_key,
                &mut pending_tool_group_result_persists,
            )
            .await?;
        }

        let mut stop_after_remote_im_done_in_turn = false;
        runtime_log_info(format!(
            "[聊天] 阶段=tool_loop.round_model_request.start，模式=non_stream，session={}，轮次={}，模型={}，api_id={}",
            chat_session_key,
            round_index + 1,
            model_name,
            selected_api.id,
        ));
        push_tool_loop_round_start(
            tool_abort_state,
            chat_session_key,
            selected_api.name.as_str(),
            model_name,
        );
        let round = {
            let mut request = genai::chat::ChatRequest::from_messages(
                sanitize_genai_messages_before_request(messages.clone(), "genai_tool_loop_non_stream"),
            );
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
            let _provider_concurrency_guard = maybe_acquire_provider_concurrency_guard(
                tool_abort_state,
                &api_config,
                model_name,
            )
            .await?;
            execute_genai_non_stream_round(
                &api_config,
                model_name,
                &client,
                &model_spec,
                request,
                &options,
                on_delta,
                !full_assistant_text.trim().is_empty(),
                tool_abort_state,
                usage_conversation_id,
            )
            .await
        };
        runtime_log_info(format!(
            "[聊天] 阶段=tool_loop.round_model_request.finish，模式=non_stream，session={}，轮次={}，模型={}，api_id={}，状态={}，耗时={}ms",
            chat_session_key,
            round_index + 1,
            model_name,
            selected_api.id,
            if round.is_ok() { "完成" } else { "失败" },
            round_started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        ));
        let round = round?;
        let turn_text = round.turn_text;
        let turn_reasoning = round.turn_reasoning;
        let reasoning_delta_emitted = round.reasoning_delta_emitted;
        let raw_turn_tool_calls = round.turn_tool_calls;
        let round_trusted_input_tokens = round.trusted_input_tokens;
        let round_usage = round.usage;
        let round_assistant_provider_meta = round.assistant_provider_meta;
        let round_elapsed_ms = round_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Some(value) = round_trusted_input_tokens {
            trusted_input_tokens = Some(value);
        }
        if let Some(usage) = round_usage.as_ref() {
            latest_usage = Some(usage.clone());
        }
        merge_assistant_provider_meta_patch(
            &mut final_assistant_provider_meta_override,
            round_assistant_provider_meta,
        );
        if let Some(prompt_tokens) = trusted_input_tokens {
            merge_assistant_provider_meta_patch(
                &mut final_assistant_provider_meta_override,
                genai_request_context_breakdown_patch(
                    context_chars.0,
                    context_chars.1,
                    context_chars.2,
                    prompt_tokens,
                ),
            );
        }
        push_tool_loop_round_log(
            tool_abort_state,
            chat_session_key,
            model_name,
            tool_loop_round_response_value(&turn_text, &turn_reasoning, &raw_turn_tool_calls, round_usage.as_ref()),
            round_elapsed_ms,
        );
        if let Some(context) = auto_compaction_context {
            runtime_log_info(format!(
                "[聊天] 工具循环刷新缓存 conversation_id={} trusted_input_tokens={:?} context_window_tokens={}",
                context.conversation_id, round_trusted_input_tokens, selected_api.context_window_tokens,
            ));
            conversation_prompt_service().refresh_shared_trusted_prompt_usage(
                &context.trusted_prompt_usage,
                round_trusted_input_tokens,
                selected_api,
            );
        }
        let turn_tool_calls = reorder_turn_tool_calls_for_contact_tail(raw_turn_tool_calls);
        let turn_tool_calls = clean_turn_tool_calls(turn_tool_calls);
        if !turn_reasoning.is_empty() {
            full_activity_reasoning_text.push_str(&turn_reasoning);
        }

        if turn_tool_calls.is_empty() {
            if !turn_text.is_empty() {
                if !full_assistant_text.trim().is_empty() {
                    full_assistant_text.push_str("\n\n");
                }
                full_assistant_text.push_str(&turn_text);
            }
            return Ok(ModelReply {
                assistant_text: full_assistant_text,
                final_response_text: turn_text,
                activity_reasoning_text: full_activity_reasoning_text,
                assistant_provider_meta: final_assistant_provider_meta_override.clone(),
                tool_history_events,
                suppress_assistant_message: false,
                trusted_input_tokens,
                usage: latest_usage,
                round_logs_recorded_internally: true,
            });
        }

        let mut assistant_parts = Vec::<genai::chat::ContentPart>::new();
        if !turn_text.is_empty() {
            assistant_parts.push(genai::chat::ContentPart::from_text(turn_text.clone()));
        }
        for tool_call in &turn_tool_calls {
            assistant_parts.push(genai::chat::ContentPart::ToolCall(tool_call.clone()));
        }
        let mut assistant_message = genai::chat::ChatMessage::assistant(
            genai::chat::MessageContent::from_parts(assistant_parts),
        );
        assistant_message =
            assistant_message.with_reasoning_content(Some(turn_reasoning.trim().to_string()));
        messages.push(assistant_message);
        let mut deferred_outcome = None::<DeferredToolLoopOutcome>;
        let mut guided_close_requested = false;
        let assistant_tool_group_history_event = assistant_tool_group_history_event_value(
            &turn_text,
            &turn_tool_calls,
            &turn_reasoning,
            round_trusted_input_tokens,
            selected_api.context_window_tokens,
        );
        let assistant_tool_group_stream_event =
            assistant_tool_group_stream_event_value(&turn_text, &turn_tool_calls);
        if !reasoning_delta_emitted && !turn_reasoning.trim().is_empty() {
            send_reasoning_delta_event(on_delta, turn_reasoning.trim());
        }
        send_assistant_tool_event(on_delta, &assistant_tool_group_stream_event);
        tool_history_events.push(assistant_tool_group_history_event.clone());

        let prepared_turn_tool_calls = turn_tool_calls
            .into_iter()
            .map(prepared_tool_call_from_genai)
            .collect::<Vec<_>>();
        let mut round_completed_tool_result_events = Vec::<Value>::new();
        for batch in split_prepared_tool_calls_into_execution_batches(
            &tool_assembly.tools,
            &tool_assembly.tool_definitions,
            prepared_turn_tool_calls,
        ) {
            let mut executable_calls = Vec::<PreparedToolCall>::new();
            let mut repeat_block = None::<(PreparedToolCall, String)>;
            let mut batch_repeat_signatures = std::collections::HashSet::new();
            for call in batch.calls {
                // 模型即将操作电脑：本轮调度内首次调用 operate 时发一次系统通知（由调度器控制，无全局状态）。
                if !desktop_notice_sent && call.tool_name == "operate" {
                    desktop_notice_sent = true;
                    if let Some(state) = tool_abort_state {
                        if let Ok(args) = serde_json::from_str::<Value>(&call.tool_args) {
                            if let Some(script) = args.get("script").and_then(Value::as_str) {
                                notify_desktop_operation_started(state, script);
                            }
                        }
                    }
                }
                let repeat_streak = register_tool_repeat_attempt_once_per_batch(
                    &mut tool_repeat_guard,
                    &mut batch_repeat_signatures,
                    &call.tool_name,
                    &call.tool_args,
                );
                if repeat_streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD {
                    let err_text = repeated_tool_call_block_message(
                        &call.tool_name,
                        &call.tool_args,
                        repeat_streak,
                    );
                    runtime_log_info(format!(
                        "[聊天] 工具循环触发重复调用熔断: session={}, tool_name={}, streak={}, threshold={}, args={}",
                        chat_session_key, call.tool_name, repeat_streak, REPEATED_TOOL_CALL_BLOCK_THRESHOLD, call.tool_args
                    ));
                    repeat_block = Some((call, err_text));
                    break;
                }
                executable_calls.push(call);
            }

            let executed_tool_calls = execute_prepared_tool_call_group(
                tool_abort_state,
                chat_session_key,
                &tool_assembly.tools,
                on_delta,
                auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                executable_calls,
            )
            .await?;

            for executed_tool_call in executed_tool_calls {
            let ExecutedToolCall {
                tool_call_id,
                tool_name,
                tool_args,
                tool_result,
            } = executed_tool_call;
            let projection = project_provider_tool_result(tool_abort_state, &tool_name, &tool_result);
            let tool_result_event = tool_result_history_event(
                &tool_call_id,
                projection.text.clone(),
                &projection.metadata,
            );
            send_assistant_tool_result_event(on_delta, &tool_result_event);
            insert_before_trailing_user_history_events(
                &mut tool_history_events,
                tool_result_event.clone(),
            );
            // 判定前不写临时账本；仅内存累积本轮事件。
            round_completed_tool_result_events.push(tool_result_event);

            if tool_loop_should_close_for_guided_queue(tool_abort_state, auto_compaction_context) {
                runtime_log_info(format!(
                    "[引导投送] 工具轮次完成后闭合当前非流式调度: session={}, tool_name={}",
                    chat_session_key, tool_name
                ));
                guided_close_requested = true;
            }

            if deferred_outcome.is_none() {
                deferred_outcome =
                    deferred_tool_loop_outcome_from_result(&tool_name, &tool_args, &tool_result);
            }
            if let Some(plan_state) =
                plan_tool_result_state(&tool_name, &tool_args, &tool_result)
            {
                if plan_state.action.eq_ignore_ascii_case("complete") {
                    final_assistant_provider_meta_override = Some(serde_json::json!({
                        "messageKind": "plan_complete",
                        "message_meta": {
                            "kind": "plan_complete",
                        }
                    }));
                }
            }
            if should_stop_after_contact_tool(&tool_name, &tool_result) {
                stop_after_remote_im_done_in_turn = true;
            }

            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result, &projection.text);
            insert_before_trailing_user_messages(
                &mut messages,
                genai::chat::ChatMessage::from(
                    genai::chat::ToolResponse::new(tool_call_id, tool_result_for_model),
                ),
            );
            if let Some(message) = runtime_tool_result_followup_message(
                &tool_name,
                &tool_result,
                screenshot_forward.is_none(),
            ) {
                messages.push(message);
            }
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload, &tool_name);
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
                // 判定前不写临时账本。
            }
            }

            if let Some((call, err_text)) = repeat_block {
                send_stream_rebind_required_event(
                    on_delta,
                    auto_compaction_context.and_then(|ctx| ctx.request_id.as_deref()),
                    "tool_start",
                );
                send_tool_status_event(
                    on_delta,
                    &call.tool_name,
                    "running",
                    Some(call.tool_args.as_str()),
                    Some(call.tool_call_id.as_str()),
                    &format!("正在调用工具：{}", call.tool_name),
                );
                send_tool_status_event(
                    on_delta,
                    &call.tool_name,
                    "failed",
                    Some(call.tool_args.as_str()),
                    Some(call.tool_call_id.as_str()),
                    &err_text,
                );
                let history_content = err_text.clone();
                let tool_result_event = serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call.tool_call_id,
                    "content": history_content
                });
                send_assistant_tool_result_event(on_delta, &tool_result_event);
                insert_before_trailing_user_history_events(
                    &mut tool_history_events,
                    tool_result_event.clone(),
                );
                sync_completed_tool_history_cache(
                    tool_abort_state,
                    chat_session_key,
                    &tool_history_events,
                );
                return Ok(repeated_tool_call_block_reply(
                    full_activity_reasoning_text,
                    tool_history_events,
                    trusted_input_tokens,
                    err_text,
                ));
            }
        }

        // 工具整轮执行完毕瞬间判定：判定前未写正式历史/临时账本。
        let round_history_start = tool_history_events
            .iter()
            .rposition(|event| {
                event
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .is_some_and(|calls| !calls.is_empty())
            })
            .unwrap_or(0);
        let round_history_events = tool_history_events[round_history_start..].to_vec();
        if apply_compaction_preserved_gate_after_tool_round(
            tool_abort_state,
            auto_compaction_context,
            selected_api,
            resolved_api,
            on_delta,
            chat_session_key,
            &mut pending_tool_group_result_persists,
            trusted_input_tokens,
            &turn_text,
            &turn_reasoning,
            &assistant_tool_group_history_event,
            &round_history_events,
            &round_completed_tool_result_events,
        )
        .await?
        {
            return Err(CHAT_DISPATCH_RESTART_AFTER_COMPACTION.to_string());
        }
        // 判定为直写后，才同步临时账本（与旧语义一致）。
        sync_completed_tool_history_cache(
            tool_abort_state,
            chat_session_key,
            &tool_history_events,
        );

        if guided_close_requested {
            return Ok(tool_loop_guided_close_reply(
                full_activity_reasoning_text,
                tool_history_events,
                trusted_input_tokens,
            ));
        }

        if let Some(outcome) = deferred_outcome {
            return Ok(finalize_deferred_tool_loop_outcome(
                outcome,
                full_activity_reasoning_text,
                tool_history_events,
                trusted_input_tokens,
            ));
        }

        if stop_after_remote_im_done_in_turn {
            return Ok(finalize_remote_im_stop_model_reply(
                &full_assistant_text,
                full_activity_reasoning_text,
                final_assistant_provider_meta_override.clone(),
                tool_history_events,
                trusted_input_tokens,
                latest_usage,
            ));
        }
    }

    send_tool_status_event(
        on_delta,
        "tools",
        "failed",
        None,
        None,
        "工具循环触发内部安全上限，停止继续调用并立刻汇报。",
    );
    Ok(ModelReply {
        assistant_text: full_assistant_text,
        final_response_text: String::new(),
        activity_reasoning_text: full_activity_reasoning_text,
        assistant_provider_meta: final_assistant_provider_meta_override,
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage: latest_usage,
        round_logs_recorded_internally: true,
    })
}

#[cfg(test)]
mod tool_loop_tests {
    use super::*;

    struct TestRuntimeTool {
        name: &'static str,
        mcp: bool,
    }

    impl RuntimeToolDyn for TestRuntimeTool {
        fn name(&self) -> String {
            self.name.to_string()
        }

        fn is_mcp_tool(&self) -> bool {
            self.mcp
        }

        fn call_json(&self, _args_json: String) -> RuntimeToolCallFuture<'_> {
            Box::pin(async { Ok(ProviderToolResult::text("ok")) })
        }
    }

    fn test_tool(name: &'static str, mcp: bool) -> Box<dyn RuntimeToolDyn> {
        Box::new(TestRuntimeTool { name, mcp })
    }

    struct TimeoutReadMediaTool;

    impl RuntimeToolDyn for TimeoutReadMediaTool {
        fn name(&self) -> String {
            READ_MEDIA_TOOL_NAME.to_string()
        }

        fn is_mcp_tool(&self) -> bool {
            false
        }

        fn timeout_override(&self, _args_json: &str) -> Option<std::time::Duration> {
            Some(std::time::Duration::from_millis(1))
        }

        fn call_json(&self, _args_json: String) -> RuntimeToolCallFuture<'_> {
            Box::pin(std::future::pending())
        }
    }

    struct InnerTimeoutReadMediaTool;

    impl RuntimeToolDyn for InnerTimeoutReadMediaTool {
        fn name(&self) -> String {
            READ_MEDIA_TOOL_NAME.to_string()
        }

        fn is_mcp_tool(&self) -> bool {
            false
        }

        fn timeout_override(&self, _args_json: &str) -> Option<std::time::Duration> {
            Some(std::time::Duration::from_secs(1))
        }

        fn call_json(&self, _args_json: String) -> RuntimeToolCallFuture<'_> {
            Box::pin(async { Err("解析超时".to_string()) })
        }
    }

    #[tokio::test]
    async fn read_media_timeout_should_return_normal_tool_result() {
        let tools: Vec<Box<dyn RuntimeToolDyn>> = vec![Box::new(TimeoutReadMediaTool)];
        let result = call_runtime_tool_by_name(&tools, READ_MEDIA_TOOL_NAME, "{}")
            .await
            .expect("read_media timeout should be a tool result");

        assert_eq!(result.output, "解析超时");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn read_media_inner_timeout_should_return_normal_tool_result() {
        let tools: Vec<Box<dyn RuntimeToolDyn>> = vec![Box::new(InnerTimeoutReadMediaTool)];
        let result = call_runtime_tool_by_name(&tools, READ_MEDIA_TOOL_NAME, "{}")
            .await
            .expect("read_media inner timeout should be a tool result");

        assert_eq!(result.output, "解析超时");
        assert!(!result.is_error);
    }

    #[test]
    fn read_media_timeout_should_follow_media_type() {
        assert_eq!(
            read_media_tool_timeout_override(r#"{"path":"C:\\tmp\\image.png"}"#),
            std::time::Duration::from_secs(READ_MEDIA_IMAGE_TOOL_TIMEOUT_SECS)
        );
        assert_eq!(
            read_media_tool_timeout_override(r#"{"path":"C:\\tmp\\audio.mp3"}"#),
            std::time::Duration::from_secs(READ_MEDIA_AUDIO_TOOL_TIMEOUT_SECS)
        );
        assert_eq!(
            read_media_tool_timeout_override(r#"{"path":"C:\\tmp\\video.mp4"}"#),
            std::time::Duration::from_secs(READ_MEDIA_VIDEO_TOOL_TIMEOUT_SECS)
        );
    }

    fn estimate_latest_tool_result_content_tokens(events: &[Value]) -> u64 {
        let latest_tool_call_ids = events
            .iter()
            .rposition(|event| event.get("role").and_then(Value::as_str) == Some("assistant"))
            .and_then(|index| events.get(index))
            .and_then(|event| event.get("tool_calls"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("id").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();
        if latest_tool_call_ids.is_empty() {
            return 0;
        }
        let text = events
            .iter()
            .filter(|event| {
                event.get("role").and_then(Value::as_str) == Some("tool")
                    && event
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| latest_tool_call_ids.contains(id))
            })
            .filter_map(|event| event.get("content").and_then(Value::as_str))
            .collect::<String>();
        estimated_tokens_for_text(&text).ceil() as u64
    }

    #[test]
    fn stateful_builtin_tools_should_be_serial_tools() {
        let tools = vec![
            test_tool("exec", false),
            test_tool("config", false),
            test_tool("shell_exec", false),
            test_tool("write", false),
            test_tool("delete", false),
            test_tool("update", false),
            test_tool("move", false),
            test_tool("todo", false),
            test_tool("task", false),
            test_tool("remember", false),
            test_tool("plan", false),
            test_tool("image_generate", false),
            test_tool("image_edit", false),
            test_tool("remote_im_send", false),
            test_tool("contact_send_files", false),
            test_tool("read", false),
            test_tool("fetch", false),
            test_tool("websearch", false),
            test_tool("recall", false),
        ];
        let definitions = Vec::<ProviderToolDefinition>::new();

        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "exec"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "config"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "shell_exec"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "write"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "delete"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "update"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "move"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "todo"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "task"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "remember"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "plan"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "image_generate"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "image_edit"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "remote_im_send"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "contact_send_files"));
        assert!(!runtime_tool_call_requires_serial_execution(&tools, &definitions, "read"));
        assert!(!runtime_tool_call_requires_serial_execution(&tools, &definitions, "fetch"));
        assert!(!runtime_tool_call_requires_serial_execution(&tools, &definitions, "websearch"));
        assert!(!runtime_tool_call_requires_serial_execution(&tools, &definitions, "recall"));
    }

    fn prepared_test_call(id: &str, name: &str) -> PreparedToolCall {
        PreparedToolCall {
            tool_call_id: id.to_string(),
            tool_name: name.to_string(),
            tool_args: "{}".to_string(),
        }
    }

    #[test]
    fn serial_tool_should_split_parallel_batches() {
        let tools = vec![
            test_tool("read", false),
            test_tool("fetch", false),
            test_tool("exec", false),
            test_tool("todo", false),
            test_tool("mcp_lookup", true),
            test_tool("recall", false),
        ];
        let definitions = vec![ProviderToolDefinition::new(
            "mcp_lookup",
            "Search external data without changing state.",
            serde_json::json!({"type":"object"}),
        )];

        let batches = split_prepared_tool_calls_into_execution_batches(
            &tools,
            &definitions,
            vec![
                prepared_test_call("1", "read"),
                prepared_test_call("2", "fetch"),
                prepared_test_call("3", "exec"),
                prepared_test_call("4", "todo"),
                prepared_test_call("5", "mcp_lookup"),
                prepared_test_call("6", "recall"),
            ],
        );
        let grouped_ids = batches
            .into_iter()
            .map(|batch| {
                batch
                    .calls
                    .into_iter()
                    .map(|call| call.tool_call_id)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            grouped_ids,
            vec![
                vec!["1".to_string(), "2".to_string()],
                vec!["3".to_string()],
                vec!["4".to_string()],
                vec!["5".to_string(), "6".to_string()],
            ]
        );
    }

    #[test]
    fn mcp_tools_with_mutating_file_or_shell_semantics_should_be_serial() {
        let tools = vec![
            test_tool("workspace_edit", true),
            test_tool("repo_lookup", true),
            test_tool("profile_lookup", true),
        ];
        let definitions = vec![
            ProviderToolDefinition::new(
                "workspace_edit",
                "Edit files in the workspace.",
                serde_json::json!({"type":"object"}),
            ),
            ProviderToolDefinition::new(
                "repo_lookup",
                "Search repository metadata.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string" }
                    }
                }),
            ),
            ProviderToolDefinition::new(
                "profile_lookup",
                "Search profile settings without changing state.",
                serde_json::json!({"type":"object"}),
            ),
        ];

        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "workspace_edit"));
        assert!(runtime_tool_call_requires_serial_execution(&tools, &definitions, "repo_lookup"));
        assert!(!runtime_tool_call_requires_serial_execution(&tools, &definitions, "profile_lookup"));
    }

    #[test]
    fn task_complete_should_remain_tool_result_instead_of_ending_the_round() {
        let tool_result = provider_tool_result_from_value("task", serde_json::json!({
                "taskId": "task-1",
                "completionState": "completed",
                "completionConclusion": "任务完成结论"
            }));

        let deferred = deferred_tool_loop_outcome_from_result(
            "task",
            r#"{"action":"complete","task_id":"task-1","completion_state":"completed","completion_conclusion":"任务完成结论"}"#,
            &tool_result,
        );
        let projection = project_provider_tool_result(None, "task", &tool_result);

        assert!(deferred.is_none());
        assert!(projection.text.contains("任务完成结论"));
        assert!(matches!(
            &projection.metadata.control,
            ProviderToolControl::Task {
                completion_state: Some(state),
                completion_conclusion: Some(conclusion),
            } if state == "completed" && conclusion == "任务完成结论"
        ));
    }

    #[test]
    fn plan_complete_should_not_become_terminal_outcome() {
        let tool_result = provider_tool_result_from_value("plan", serde_json::json!({
                "action": "complete",
                "path": "E:\\demo\\.pai\\plan\\plan.md",
                "should_stop_tool_loop": false,
                "active_plan_completed": true
            }));

        let deferred = deferred_tool_loop_outcome_from_result(
            "plan",
            r#"{"action":"complete","path":"E:\\demo\\.pai\\plan\\plan.md"}"#,
            &tool_result,
        );

        assert!(deferred.is_none());
    }

    #[test]
    fn auto_approved_plan_present_should_not_become_terminal_outcome() {
        let tool_result = provider_tool_result_from_value("plan", serde_json::json!({
                "action": "present",
                "path": "E:\\demo\\.pai\\plan\\plan.md",
                "should_stop_tool_loop": false,
                "auto_approved": true
            }));

        let deferred = deferred_tool_loop_outcome_from_result(
            "plan",
            r#"{"action":"present","path":"E:\\demo\\.pai\\plan\\plan.md"}"#,
            &tool_result,
        );

        assert!(deferred.is_none());
    }

    #[test]
    fn rejected_exec_result_should_remain_a_tool_result_instead_of_ending_the_round() {
        let tool_result = provider_tool_result_from_value("exec", serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "absolute_path_not_granted",
                "message": "写入类命令只能作用于已配置工作目录；未纳管绝对路径仅允许读取。"
            }));

        assert!(terminal_plan_present_result(
            "exec",
            r#"{"command":"echo hi > E:\\outside.txt"}"#,
            &tool_result,
        )
        .is_none());

        let history_content = project_provider_tool_result(None, "exec", &tool_result).text;
        assert!(history_content.contains("Exit code: -1"));
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

        assert_eq!(
            model_reply_content_state(&reply),
            ModelReplyContentState::Visible
        );
        assert!(reply.assistant_text.is_empty());
        assert_eq!(reply.tool_history_events.len(), 1);
        assert_eq!(reply.trusted_input_tokens, Some(12));
        assert!(reply.assistant_provider_meta.is_some());
    }

    #[test]
    fn finalize_remote_im_stop_model_reply_should_keep_send_completion_text() {
        let reply = finalize_remote_im_stop_model_reply(
            "",
            String::new(),
            None,
            vec![serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true,\"action\":\"send\"}"
            })],
            None,
            None,
        );

        assert!(!reply.suppress_assistant_message);
        assert_eq!(reply.assistant_text, "已发送完成。");
        assert_eq!(reply.final_response_text, "已发送完成。");
    }

    #[test]
    fn tool_loop_round_response_value_should_keep_reasoning_content() {
        let response = tool_loop_round_response_value("准备调用工具", "先读取目标文件确认结构", &[], None);

        assert_eq!(response["assistantText"].as_str(), Some("准备调用工具"));
        assert_eq!(
            response["reasoningContent"].as_str(),
            Some("先读取目标文件确认结构")
        );
        assert!(response["toolCalls"].as_array().is_some_and(|items| items.is_empty()));
    }

    #[test]
    fn tool_loop_assistant_message_should_preserve_gemini_thought_signatures() {
        let tool_calls = vec![genai::chat::ToolCall {
            call_id: "call-read-media".to_string(),
            fn_name: "read_media".to_string(),
            fn_arguments: serde_json::json!({"path": "image.png"}),
            thought_signatures: Some(vec!["gemini-signature".to_string()]),
        }];

        let message = tool_loop_assistant_message("我来读取图片", &tool_calls, "先检查图片内容");
        let parts = message.content.parts();

        assert!(matches!(
            parts.first(),
            Some(genai::chat::ContentPart::ThoughtSignature(signature)) if signature == "gemini-signature"
        ));
        assert!(matches!(
            parts.get(1),
            Some(genai::chat::ContentPart::Text(text)) if text == "我来读取图片"
        ));
        assert!(matches!(
            parts.get(2),
            Some(genai::chat::ContentPart::ToolCall(tool_call)) if tool_call.fn_name == "read_media"
        ));
    }

    #[test]
    fn assistant_tool_group_history_event_value_should_keep_reasoning_once_for_multiple_tools() {
        let tool_calls = vec![
            genai::chat::ToolCall {
                call_id: "call-a".to_string(),
                fn_name: "read".to_string(),
                fn_arguments: serde_json::json!({"path": "a.rs"}),
                thought_signatures: None,
            },
            genai::chat::ToolCall {
                call_id: "call-b".to_string(),
                fn_name: "read".to_string(),
                fn_arguments: serde_json::json!({"path": "b.rs"}),
                thought_signatures: None,
            },
        ];

        let event = assistant_tool_group_history_event_value(
            "三个并发 shell 跑完后我再汇总。",
            &tool_calls,
            "先同时读取两个文件",
            None,
            0,
        );

        assert_eq!(event["role"].as_str(), Some("assistant"));
        assert_eq!(
            event["content"].as_str(),
            Some("三个并发 shell 跑完后我再汇总。")
        );
        assert_eq!(event["reasoning_content"].as_str(), Some("先同时读取两个文件"));
        assert_eq!(event["tool_calls"].as_array().map(Vec::len), Some(2));
        assert_eq!(event["tool_calls"][0]["id"].as_str(), Some("call-a"));
        assert_eq!(event["tool_calls"][1]["id"].as_str(), Some("call-b"));
        assert!(event.get("usage").is_none(), "无真实用量时不挂 usage");
    }

    #[test]
    fn assistant_tool_group_history_event_value_should_carry_real_usage() {
        let tool_calls = vec![genai::chat::ToolCall {
            call_id: "call-a".to_string(),
            fn_name: "read".to_string(),
            fn_arguments: serde_json::json!({"path": "a.rs"}),
            thought_signatures: None,
        }];
        let event = assistant_tool_group_history_event_value(
            "",
            &tool_calls,
            "",
            Some(4200),
            128000,
        );
        assert_eq!(event["usage"]["promptTokens"].as_u64(), Some(4200));
        assert_eq!(event["usage"]["contextWindowTokens"].as_u64(), Some(128000));
    }

    #[test]
    fn assistant_tool_group_history_event_value_should_normalize_zero_context_window() {
        let tool_calls = vec![genai::chat::ToolCall {
            call_id: "call-a".to_string(),
            fn_name: "read".to_string(),
            fn_arguments: serde_json::json!({"path": "a.rs"}),
            thought_signatures: None,
        }];
        let event = assistant_tool_group_history_event_value(
            "",
            &tool_calls,
            "",
            Some(4200),
            0,
        );
        assert_eq!(event["usage"]["promptTokens"].as_u64(), Some(4200));
        assert_eq!(
            event["usage"]["contextWindowTokens"].as_u64(),
            Some(1),
            "context_window=0 时归一化为 1，与 meta 写入口径一致"
        );
    }

    #[test]
    fn latest_tool_result_estimate_should_only_count_current_tool_group() {
        let events = vec![
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"id": "call-a"}]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-a",
                "content": "上一轮工具结果应该已经进入模型用量"
            }),
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"id": "call-b"}]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-b",
                "content": "当前工具结果"
            }),
        ];

        let latest = estimate_latest_tool_result_content_tokens(&events);
        let all_results =
            estimated_tokens_for_text("上一轮工具结果应该已经进入模型用量当前工具结果")
                .ceil() as u64;

        assert!(latest > 0);
        assert!(latest < all_results);
    }

    #[test]
    fn assistant_tool_group_stream_event_value_should_not_include_reasoning_content() {
        let tool_calls = vec![genai::chat::ToolCall {
            call_id: "call-a".to_string(),
            fn_name: "read".to_string(),
            fn_arguments: serde_json::json!({"path": "a.rs"}),
            thought_signatures: None,
        }];

        let event = assistant_tool_group_stream_event_value(
            "三个并发 shell 跑完后我再汇总。",
            &tool_calls,
        );

        assert_eq!(event["role"].as_str(), Some("assistant"));
        assert!(event.get("reasoning_content").is_none());
        assert_eq!(event["tool_calls"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn insert_before_trailing_user_history_events_should_keep_tools_before_sidecars() {
        let mut events = vec![
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"id": "call-a"}, {"id": "call-b"}]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-a",
                "content": "工具 A 结果"
            }),
            serde_json::json!({
                "role": "user",
                "content": "[desktop screenshot forwarded as user image]"
            }),
        ];

        insert_before_trailing_user_history_events(
            &mut events,
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-b",
                "content": "工具 B 结果"
            }),
        );

        assert_eq!(events[0]["role"].as_str(), Some("assistant"));
        assert_eq!(events[1]["tool_call_id"].as_str(), Some("call-a"));
        assert_eq!(events[2]["tool_call_id"].as_str(), Some("call-b"));
        assert_eq!(events[3]["role"].as_str(), Some("user"));
    }

    #[test]
    fn insert_before_trailing_user_messages_should_keep_tool_responses_before_sidecars() {
        let mut messages = vec![
            genai::chat::ChatMessage::assistant("assistant"),
            genai::chat::ChatMessage::from(genai::chat::ToolResponse::new(
                "call-a",
                "工具 A 结果",
            )),
            genai::chat::ChatMessage::user("sidecar"),
        ];

        insert_before_trailing_user_messages(
            &mut messages,
            genai::chat::ChatMessage::from(genai::chat::ToolResponse::new(
                "call-b",
                "工具 B 结果",
            )),
        );

        assert!(matches!(messages[0].role, genai::chat::ChatRole::Assistant));
        assert!(matches!(messages[1].role, genai::chat::ChatRole::Tool));
        assert!(matches!(messages[2].role, genai::chat::ChatRole::Tool));
        assert!(matches!(messages[3].role, genai::chat::ChatRole::User));
    }

    #[test]
    fn normalized_tool_args_signature_should_ignore_json_key_order() {
        let left = normalized_tool_args_signature(r#"{"b":2,"a":1}"#);
        let right = normalized_tool_args_signature(r#"{"a":1,"b":2}"#);

        assert_eq!(left, right);
    }

    #[test]
    fn tool_repeat_guard_should_block_after_three_identical_calls() {
        let mut guard = ToolRepeatGuard::default();
        let mut streak = 0usize;
        for _ in 0..4 {
            streak = register_tool_repeat_attempt(&mut guard, "read_file", r#"{"path":"a.txt"}"#);
        }

        assert_eq!(streak, 4);
        assert!(streak > REPEATED_TOOL_CALL_BLOCK_THRESHOLD);
    }

    #[test]
    fn tool_repeat_guard_should_count_identical_calls_once_per_batch() {
        let mut guard = ToolRepeatGuard::default();
        let mut batch_signatures = std::collections::HashSet::new();
        let mut streak = 0usize;
        for index in 0..4 {
            streak = register_tool_repeat_attempt_once_per_batch(
                &mut guard,
                &mut batch_signatures,
                "delegate",
                r#"{"agent_id":"deputy-agent","mode":"sync"}"#,
            );
            assert!(
                streak <= REPEATED_TOOL_CALL_BLOCK_THRESHOLD,
                "同批第 {} 个相同委托不应触发重复调用熔断",
                index + 1
            );
        }

        assert_eq!(streak, 1);
        assert_eq!(guard.same_call_streak, 1);
    }

    #[test]
    fn empty_tool_args_should_use_short_repeat_block_threshold() {
        assert!(tool_args_effectively_empty(""));
        assert!(tool_args_effectively_empty("{}"));
        assert!(tool_args_effectively_empty("[]"));
        assert!(tool_args_effectively_empty("null"));
        assert!(tool_args_effectively_empty("\"\""));
        assert!(!tool_args_effectively_empty(r#"{"query":"abc"}"#));

        let mut guard = ToolRepeatGuard::default();
        let first = register_tool_repeat_attempt(&mut guard, "akasha_search", "{}");
        let second = register_tool_repeat_attempt(&mut guard, "akasha_search", "{}");

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert!(second <= REPEATED_TOOL_CALL_BLOCK_THRESHOLD);
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
                "reasoning_content": "先看当前窗口列表",
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
        assert_eq!(
            prepared.history_messages[0].reasoning_content.as_deref(),
            Some("先看当前窗口列表")
        );
        assert_eq!(prepared.history_messages[1].role, "tool");
        assert_eq!(
            prepared.history_messages[1].tool_call_id.as_deref(),
            Some("call_1")
        );
    }

    #[test]
    fn append_tool_loop_transient_history_to_prepared_should_keep_reasoning_when_continuing_request(
    ) {
        let mut prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![PreparedHistoryMessage {
                role: "user".to_string(),
                text: "继续".to_string(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: "再继续".to_string(),
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
                "reasoning_content": "第1轮先看窗口",
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
                "content": "{\"ok\":true,\"windows\":3}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第2轮再截图确认",
                "tool_calls": [{
                    "id": "call_2",
                    "type": "function",
                    "function": {
                        "name": "xcap",
                        "arguments": "{\"method\":\"capture_active\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_2",
                "content": "{\"ok\":true,\"image\":\"cached\"}"
            }),
        ];

        append_tool_loop_transient_history_to_prepared(&mut prepared, &events);
        let request = build_genai_chat_request(&prepared)
            .expect("build_genai_chat_request should succeed");

        let assistant_reasonings = request
            .messages
            .iter()
            .filter(|message| matches!(message.role, genai::chat::ChatRole::Assistant))
            .flat_map(|message| message.content.reasoning_contents().into_iter())
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            assistant_reasonings,
            vec!["第1轮先看窗口".to_string(), "第2轮再截图确认".to_string()]
        );
    }
}
