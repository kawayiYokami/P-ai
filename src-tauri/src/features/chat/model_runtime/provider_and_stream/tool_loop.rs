fn latest_assistant_reasoning_since_last_user(chat_history: &[RigMessage]) -> Option<String> {
    for msg in chat_history.iter().rev() {
        match msg {
            RigMessage::User { .. } => break,
            RigMessage::System { .. } => continue,
            RigMessage::Assistant { content, .. } => {
                let mut merged = String::new();
                for item in content.iter() {
                    if let AssistantContent::Reasoning(reasoning) = item {
                        let text = reasoning.display_text();
                        if !text.trim().is_empty() {
                            if !merged.is_empty() {
                                merged.push('\n');
                            }
                            merged.push_str(&text);
                        }
                    }
                }
                if !merged.trim().is_empty() {
                    return Some(merged);
                }
            }
        }
    }
    None
}

fn resolve_reasoning_for_tool_history(turn_reasoning: &str, chat_history: &[RigMessage]) -> String {
    let turn = turn_reasoning.trim();
    if !turn.is_empty() {
        return turn_reasoning.to_string();
    }
    if let Some(inherited) = latest_assistant_reasoning_since_last_user(chat_history) {
        return inherited;
    }
    " ".to_string()
}

#[derive(Debug, Clone)]
struct ToolLoopAutoCompactionContext {
    conversation_id: String,
    prompt_mode: PromptBuildMode,
    agent: AgentProfile,
    agents: Vec<AgentProfile>,
    departments: Vec<DepartmentConfig>,
    user_name: String,
    user_intro: String,
    response_style_id: String,
    ui_language: String,
    last_archive_summary: Option<String>,
    terminal_block: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
    enable_pdf_images: bool,
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
}

fn tool_loop_active_conversation_snapshot(
    state: &AppState,
    conversation_id: &str,
) -> Result<Option<Conversation>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let data = state_read_app_data_cached(state)?;
    let conversation = data
        .conversations
        .iter()
        .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
        .cloned();
    drop(guard);
    Ok(conversation)
}

fn build_tool_loop_prepared_for_continuation(
    state: &AppState,
    context: &ToolLoopAutoCompactionContext,
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
        context.terminal_block.clone(),
        context.chat_overrides.clone(),
        Some(state),
        Some(resolved_api),
        Some(context.enable_pdf_images),
    );
    append_tool_loop_transient_history_to_prepared(&mut prepared, transient_tool_history);
    Ok(Some((conversation, prepared)))
}

async fn maybe_apply_auto_compaction_before_tool_continue(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    protocol_family: ToolCallProtocolFamily,
    transient_tool_history: &[Value],
    current_prompt: &mut RigMessage,
    chat_history: &mut Vec<RigMessage>,
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

    let estimated_prompt_tokens =
        estimate_prepared_prompt_tokens(&prepared_before, selected_api, &context.agent);
    let context_window = u64::from(selected_api.context_window_tokens.max(1));
    let usage_ratio = estimated_prompt_tokens as f64 / context_window as f64;
    if usage_ratio < 0.82 {
        runtime_log_info(format!(
            "[聊天] 工具续调前上下文整理检查 跳过 conversation_id={} usage_ratio={:.4}",
            context.conversation_id, usage_ratio
        ));
        return Ok(false);
    }

    let _ = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
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
        "force_context_usage_82_before_tool_continue",
        "COMPACTION-BEFORE-TOOL-CONTINUE",
    )
    .await;

    let archive_result = match archive_res {
        Ok(result) => result,
        Err(err) => {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: String::new(),
                kind: Some("tool_status".to_string()),
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
            "[聊天] 工具续调前上下文整理 完成 conversation_id={} usage_ratio_before={:.4} estimated_prompt_tokens={}",
            context.conversation_id, usage_ratio, estimated_prompt_tokens
        ));
    }

    let Some((_compacted_source, prepared_after)) = build_tool_loop_prepared_for_continuation(
        state,
        context,
        resolved_api,
        transient_tool_history,
    )?
    else {
        return Err("自动整理完成后未找到当前会话，无法继续工具续调。".to_string());
    };
    let (next_prompt, next_history) = build_tool_loop_prompt(&prepared_after, protocol_family)?;
    *current_prompt = next_prompt;
    *chat_history = next_history;
    Ok(true)
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

fn should_stop_after_remote_im_send(tool_name: &str, tool_result: &str) -> bool {
    if tool_name != "remote_im_send" {
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

fn remote_im_result_action(tool_result: &str) -> Option<String> {
    serde_json::from_str::<Value>(tool_result)
        .ok()
        .and_then(|value| value.get("action").and_then(Value::as_str).map(str::to_string))
}

fn tool_history_without_organize_context(events: &[Value]) -> Vec<Value> {
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
        filtered.push(event.clone());
    }
    filtered
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

async fn run_unified_tool_loop<M>(
    agent: rig::agent::Agent<M>,
    prepared: PreparedPrompt,
    protocol_family: ToolCallProtocolFamily,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    auto_compaction_context: Option<&ToolLoopAutoCompactionContext>,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    max_tool_iterations: usize,
    include_reasoning_before_tool_calls: bool,
    tool_abort_state: Option<&AppState>,
    chat_session_key: &str,
) -> Result<ModelReply, String>
where
    M: rig::completion::CompletionModel,
    <M as rig::completion::CompletionModel>::StreamingResponse: rig::completion::GetTokenUsage,
{
    let mut full_assistant_text = String::new();
    let mut full_reasoning_standard = String::new();
    let mut tool_history_events = Vec::<Value>::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let (mut current_prompt, mut chat_history) =
        build_tool_loop_prompt(&prepared, protocol_family)?;

    let max_rounds = std::cmp::max(1usize, max_tool_iterations);
    let mut auto_compaction_applied = false;
    for round_index in 0..max_rounds {
        if round_index > 0 && !auto_compaction_applied {
            auto_compaction_applied = maybe_apply_auto_compaction_before_tool_continue(
                tool_abort_state,
                auto_compaction_context,
                selected_api,
                resolved_api,
                on_delta,
                protocol_family,
                &tool_history_events,
                &mut current_prompt,
                &mut chat_history,
            )
            .await?;
        }
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
        let mut tool_results = Vec::<(String, String, Option<String>, String)>::new();
        let mut did_call_tool = false;
        let mut stop_after_remote_im_done_in_turn = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamedAssistantContent::Text(text)) => {
                    let _ = on_delta.send(AssistantDeltaEvent {
                        delta: text.text.clone(),
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        tool_args: None,
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
                    let tool_args = match &tool_args_value {
                        Value::String(raw) => raw.clone(),
                        other => other.to_string(),
                    };
                    send_tool_status_event(
                        on_delta,
                        &tool_name,
                        "running",
                        Some(tool_args.as_str()),
                        &format!("正在调用工具：{}", tool_name),
                    );
                    let tool_result = match call_tool_with_user_abort(
                        tool_abort_state,
                        chat_session_key,
                        agent.tool_server_handle.call_tool(&tool_name, &tool_args),
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
                            tool_failure_result_json(&tool_name, &err_text)
                        }
                    };
                    let tool_call_call_id = tool_call.call_id.clone();
                    let mut tc_json = serde_json::json!({
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "arguments": tool_args
                        }
                    });
                    if let Some(cid) = &tool_call_call_id {
                        if let Some(obj) = tc_json.as_object_mut() {
                            obj.insert("call_id".to_string(), Value::String(cid.clone()));
                        }
                    }
                    tool_history_events.push(serde_json::json!({
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [tc_json]
                    }));
                    let history_content = sanitize_tool_result_for_history(&tool_name, &tool_result);
                    tool_history_events.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": history_content
                    }));

                    tool_calls.push(AssistantContent::ToolCall(tool_call.clone()));
                    tool_results.push((tool_name, tool_call.id, tool_call.call_id, tool_result));
                    if let Some((last_tool_name, _, _, last_tool_result)) = tool_results.last() {
                        if should_stop_after_remote_im_send(last_tool_name, last_tool_result) {
                            eprintln!(
                                "[聊天] remote_im_send done=true，当前轮次立即停止后续工具调用 (session={})",
                                chat_session_key
                            );
                            stop_after_remote_im_done_in_turn = true;
                            break;
                        }
                    }
                }
                Ok(StreamedAssistantContent::Final(res)) => {
                    trusted_input_tokens = rig::completion::GetTokenUsage::token_usage(&res)
                        .map(|usage| usage.input_tokens.saturating_add(usage.cached_input_tokens))
                        .filter(|value| *value > 0);
                }
                Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                    let merged = reasoning.display_text();
                    if !merged.is_empty() {
                        if !turn_reasoning.is_empty() {
                            turn_reasoning.push('\n');
                        }
                        turn_reasoning.push_str(&merged);
                        if !full_reasoning_standard.is_empty() {
                            full_reasoning_standard.push('\n');
                        }
                        full_reasoning_standard.push_str(&merged);
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
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
                            tool_args: None,
                            message: None,
                        });
                    }
                }
                Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
                Err(err) => return Err(format!("rig streaming failed: {err}")),
            }
        }
        if stop_after_remote_im_done_in_turn {
            eprintln!(
                "[聊天] 结束当前工具轮次：remote_im_send 已返回 done (session={})",
                chat_session_key
            );
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
                suppress_assistant_message: false,
                trusted_input_tokens,
            });
        }

        if !tool_calls.is_empty() {
            let mut assistant_items = Vec::<AssistantContent>::new();
            if include_reasoning_before_tool_calls {
                let reasoning_for_history =
                    resolve_reasoning_for_tool_history(&turn_reasoning, &chat_history);
                assistant_items.push(AssistantContent::reasoning(reasoning_for_history));
            }
            assistant_items.extend(tool_calls);
            chat_history.push(RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(assistant_items)
                    .map_err(|_| "Failed to build assistant tool-call message".to_string())?,
            });
        }

        for (tool_name, tool_id, call_id, tool_result) in tool_results {
            if organize_context_succeeded(&tool_name, &tool_result) {
                return Ok(ModelReply {
                    assistant_text: String::new(),
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    tool_history_events: tool_history_without_organize_context(&tool_history_events),
                    suppress_assistant_message: true,
                    trusted_input_tokens: None,
                });
            }
            if should_stop_after_remote_im_send(&tool_name, &tool_result) {
                eprintln!(
                    "[聊天] remote_im_send done=true，立即退出工具循环 (session={})",
                    chat_session_key
                );
                let final_text = if full_assistant_text.trim().is_empty() {
                    match remote_im_result_action(&tool_result).as_deref() {
                        Some("no_reply") => "本轮决定不回复。".to_string(),
                        _ => "已发送完成。".to_string(),
                    }
                } else {
                    full_assistant_text.clone()
                };
                return Ok(ModelReply {
                    assistant_text: final_text,
                    reasoning_standard: full_reasoning_standard,
                    reasoning_inline: String::new(),
                    tool_history_events,
                    suppress_assistant_message: false,
                    trusted_input_tokens,
                });
            }
            let (tool_result_for_model, screenshot_forward) =
                enrich_screenshot_tool_result_with_cache(&tool_name, &tool_result);
            let result_content = OneOrMany::one(ToolResultContent::text(tool_result_for_model));
            let user_content = if let Some(call_id) = call_id {
                UserContent::tool_result_with_call_id(tool_id, call_id, result_content)
            } else {
                UserContent::tool_result(tool_id, result_content)
            };
            chat_history.push(RigMessage::User {
                content: OneOrMany::one(user_content),
            });
            if let Some((payload, artifact_id)) = screenshot_forward {
                let notice = screenshot_forward_notice(&payload);
                let cached = screenshot_artifact_cache_get(&artifact_id).unwrap_or(
                    ScreenshotArtifactEntry {
                        images: payload.images.clone(),
                        created_seq: 0,
                    },
                );
                let mut forwarded_items = vec![UserContent::text(notice)];
                forwarded_items.extend(cached.images.iter().map(|image| {
                    UserContent::image_base64(
                        image.base64.clone(),
                        image_media_type_from_mime(&image.mime),
                        Some(ImageDetail::Auto),
                    )
                }));
                let forwarded = OneOrMany::many(forwarded_items)
                .map_err(|_| "Failed to build screenshot forward user message".to_string())?;
                chat_history.push(RigMessage::User { content: forwarded });
                tool_history_events.push(serde_json::json!({
                    "role": "user",
                    "content": "[desktop screenshot forwarded as user image]",
                    "screenshotArtifactId": artifact_id,
                    "screenshotArtifactMaxRetained": SCREENSHOT_ARTIFACT_MAX_ITEMS,
                    "screenshotImageCount": cached.images.len()
                }));
            }
        }

        current_prompt = chat_history
            .pop()
            .ok_or_else(|| "Tool call turn ended with empty chat history".to_string())?;
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
mod tool_loop_tests {
    use super::*;

    fn assistant_with_reasoning(text: &str) -> RigMessage {
        RigMessage::Assistant {
            id: None,
            content: OneOrMany::many(vec![AssistantContent::reasoning(text.to_string())])
                .expect("assistant content"),
        }
    }

    fn assistant_with_tool_only() -> RigMessage {
        RigMessage::Assistant {
            id: None,
            content: OneOrMany::one(AssistantContent::tool_call(
                "call_1".to_string(),
                "noop".to_string(),
                serde_json::json!({}),
            )),
        }
    }

    fn user_text(text: &str) -> RigMessage {
        RigMessage::User {
            content: OneOrMany::one(UserContent::text(text.to_string())),
        }
    }

    #[test]
    fn tool_history_reasoning_prefers_current_turn_reasoning() {
        let chat_history = vec![assistant_with_reasoning("old-reasoning")];
        let chosen = resolve_reasoning_for_tool_history("new-reasoning", &chat_history);
        assert_eq!(chosen, "new-reasoning");
    }

    #[test]
    fn tool_history_reasoning_inherits_assistant_reasoning_without_user_boundary() {
        let chat_history = vec![assistant_with_reasoning("r1"), assistant_with_tool_only()];
        let chosen = resolve_reasoning_for_tool_history("", &chat_history);
        assert_eq!(chosen, "r1");
    }

    #[test]
    fn tool_history_reasoning_must_not_cross_user_boundary() {
        let chat_history = vec![
            assistant_with_reasoning("r-before-user"),
            user_text("new question"),
            assistant_with_tool_only(),
        ];
        let chosen = resolve_reasoning_for_tool_history("", &chat_history);
        assert_eq!(chosen, " ");
    }

    #[test]
    fn tool_history_reasoning_fallback_is_space_when_none_available() {
        let chat_history = vec![user_text("first question"), assistant_with_tool_only()];
        let chosen = resolve_reasoning_for_tool_history("   ", &chat_history);
        assert_eq!(chosen, " ");
    }

    #[test]
    fn remote_im_send_should_stop_on_snake_case_stop_tool_loop() {
        let tool_result = serde_json::json!({
            "ok": true,
            "action": "send",
            "stop_tool_loop": true
        })
        .to_string();

        assert!(should_stop_after_remote_im_send("remote_im_send", &tool_result));
    }

    #[test]
    fn append_tool_loop_transient_history_to_prepared_should_expand_tool_events() {
        let mut prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
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
