fn latest_assistant_reasoning_since_last_user(chat_history: &[RigMessage]) -> Option<String> {
    for msg in chat_history.iter().rev() {
        match msg {
            RigMessage::User { .. } => break,
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
        .get("stopToolLoop")
        .and_then(Value::as_bool)
        .or_else(|| value.get("done").and_then(Value::as_bool))
        .unwrap_or(false)
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
                "[WARN][CHAT] clear inflight tool abort handle failed (session={}): {}",
                chat_session_key, err
            );
        }
        match result {
            Ok(inner) => inner.map_err(|err| err.to_string()),
            Err(_) => {
                eprintln!(
                    "[INFO][CHAT] tool call aborted by user (session={})",
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
    let (mut current_prompt, mut chat_history) = build_tool_loop_prompt(&prepared)?;

    let max_rounds = std::cmp::max(1usize, max_tool_iterations);
    for _ in 0..max_rounds {
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
                                    "[INFO][CHAT] stop requested; exiting tool loop immediately (session={})",
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
                    "[INFO][CHAT] remote_im_send done=true; stop tool loop immediately (session={})",
                    chat_session_key
                );
                let final_text = if full_assistant_text.trim().is_empty() {
                    "已发送完成。".to_string()
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
                        mime: payload.mime.clone(),
                        base64: payload.base64.clone(),
                        width: payload.width,
                        height: payload.height,
                        created_seq: 0,
                    },
                );
                let forwarded = OneOrMany::many(vec![
                    UserContent::text(notice),
                    UserContent::image_base64(
                        cached.base64,
                        image_media_type_from_mime(&cached.mime),
                        Some(ImageDetail::Auto),
                    ),
                ])
                .map_err(|_| "Failed to build screenshot forward user message".to_string())?;
                chat_history.push(RigMessage::User { content: forwarded });
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
    fn deepseek_reasoning_prefers_current_turn_reasoning() {
        let chat_history = vec![assistant_with_reasoning("old-reasoning")];
        let chosen = resolve_reasoning_for_tool_history("new-reasoning", &chat_history);
        assert_eq!(chosen, "new-reasoning");
    }

    #[test]
    fn deepseek_reasoning_inherits_assistant_reasoning_without_user_boundary() {
        let chat_history = vec![assistant_with_reasoning("r1"), assistant_with_tool_only()];
        let chosen = resolve_reasoning_for_tool_history("", &chat_history);
        assert_eq!(chosen, "r1");
    }

    #[test]
    fn deepseek_reasoning_must_not_cross_user_boundary() {
        let chat_history = vec![
            assistant_with_reasoning("r-before-user"),
            user_text("new question"),
            assistant_with_tool_only(),
        ];
        let chosen = resolve_reasoning_for_tool_history("", &chat_history);
        assert_eq!(chosen, " ");
    }

    #[test]
    fn deepseek_reasoning_fallback_is_space_when_none_available() {
        let chat_history = vec![user_text("first question"), assistant_with_tool_only()];
        let chosen = resolve_reasoning_for_tool_history("   ", &chat_history);
        assert_eq!(chosen, " ");
    }
}
