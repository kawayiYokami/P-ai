fn persist_failed_chat_completed_tool_history(
    state: &AppState,
    requested_conversation_id: Option<&str>,
    agent_id: &str,
    chat_key: &str,
    error: &str,
) -> Result<bool, String> {
    let completed_tool_history = inflight_completed_tool_history(state, chat_key)?;
    if completed_tool_history.is_empty() {
        return Ok(false);
    }
    let persist_result = conversation_service_v2().persist_stop_chat_partial_message(
        state,
        requested_conversation_id,
        agent_id,
        "",
        "",
        "",
        &completed_tool_history,
        None,
    )?;
    runtime_log_error(format!(
        "[聊天] 失败前工具历史落盘检查 完成 session={} persisted={} conversation_id={} tool_event_count={} error={}",
        chat_key,
        persist_result.persisted,
        persist_result.conversation_id.as_deref().unwrap_or(""),
        completed_tool_history.len(),
        error
    ));
    Ok(persist_result.persisted)
}

fn tool_call_ids_from_history(events: &[Value]) -> std::collections::HashSet<String> {
    events
        .iter()
        .filter_map(|event| event.get("tool_calls").and_then(Value::as_array))
        .flat_map(|calls| calls.iter())
        .filter_map(|call| call.get("id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn stream_blocks_to_tool_history_events(blocks: &[AssistantStreamBlock]) -> Vec<Value> {
    let mut events = Vec::<Value>::new();
    for block in blocks {
        let tools = block
            .tools
            .iter()
            .filter(|tool| !tool.tool_call_id.trim().is_empty() && !tool.name.trim().is_empty())
            .collect::<Vec<_>>();
        if tools.is_empty() {
            continue;
        }
        let mut assistant_event = serde_json::Map::new();
        assistant_event.insert("role".to_string(), Value::String("assistant".to_string()));
        if block.text.trim().is_empty() {
            assistant_event.insert("content".to_string(), Value::Null);
        } else {
            assistant_event.insert("content".to_string(), Value::String(block.text.clone()));
        }
        if !block.reasoning.trim().is_empty() {
            assistant_event.insert(
                "reasoning_content".to_string(),
                Value::String(block.reasoning.trim().to_string()),
            );
        }
        assistant_event.insert(
            "tool_calls".to_string(),
            Value::Array(
                tools
                    .iter()
                    .map(|tool| {
                        serde_json::json!({
                            "id": tool.tool_call_id.trim(),
                            "type": "function",
                            "function": {
                                "name": tool.name.trim(),
                                "arguments": if tool.args_text.trim().is_empty() { "{}" } else { tool.args_text.trim() },
                            }
                        })
                    })
                    .collect(),
            ),
        );
        events.push(Value::Object(assistant_event));
        for tool in tools {
            let result_text = tool.result_text.trim();
            if result_text.is_empty() || tool.status.trim() == "doing" {
                continue;
            }
            events.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tool.tool_call_id.trim(),
                "content": result_text,
            }));
        }
    }
    events
}

fn merge_stream_block_tool_history(
    completed_tool_history: &[Value],
    partial_stream_blocks: &[AssistantStreamBlock],
) -> Vec<Value> {
    let mut merged = completed_tool_history.to_vec();
    let existing_ids = tool_call_ids_from_history(&merged);
    for event in stream_blocks_to_tool_history_events(partial_stream_blocks) {
        if event
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| role.eq_ignore_ascii_case("assistant"))
        {
            let has_new_tool = event
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|calls| {
                    calls.iter().any(|call| {
                        call.get("id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|id| !id.is_empty())
                            .is_some_and(|id| !existing_ids.contains(id))
                    })
                })
                .unwrap_or(false);
            if !has_new_tool {
                continue;
            }
        }
        merged.push(event);
    }
    merged
}

fn persist_aborted_chat_partial_result(
    state: &AppState,
    requested_conversation_id: Option<&str>,
    agent_id: &str,
    chat_key: &str,
) -> Result<Option<SendChatResult>, String> {
    let Some(conversation_id) = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let runtime_snapshot = read_conversation_runtime_snapshot(state, conversation_id)?;
    let stream_cache = runtime_snapshot.stream_cache;
    let assistant_text_from_blocks = assistant_text_from_stream_blocks(&stream_cache.stream_blocks);
    let assistant_text = if assistant_text_from_blocks.trim().is_empty() {
        stream_cache.assistant_text.trim().to_string()
    } else {
        assistant_text_from_blocks.trim().to_string()
    };
    let reasoning_text = reasoning_text_from_stream_blocks(&stream_cache.stream_blocks);
    let completed_tool_history = inflight_completed_tool_history(state, chat_key)?;
    let partial_tool_history =
        merge_stream_block_tool_history(&completed_tool_history, &stream_cache.stream_blocks);
    let abort_assistant_message_id = {
        let value = stream_cache.persisted_assistant_message_id.trim();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    };
    let persist_result = conversation_service_v2().persist_stop_chat_partial_message(
        state,
        Some(conversation_id),
        agent_id,
        &assistant_text,
        &reasoning_text,
        "",
        &partial_tool_history,
        abort_assistant_message_id,
    )?;
    if !persist_result.persisted {
        return Ok(None);
    }

    Ok(Some(SendChatResult {
        conversation_id: persist_result
            .conversation_id
            .unwrap_or_else(|| conversation_id.to_string()),
        latest_user_text: String::new(),
        assistant_text,
        final_response_text: String::new(),
        archived_before_send: false,
        assistant_message: persist_result.assistant_message,
        provider_prompt_tokens: None,
        estimated_prompt_tokens: None,
        effective_prompt_tokens: None,
        effective_prompt_source: None,
        context_window_tokens: None,
        max_output_tokens: None,
        context_usage_percent: None,
        remote_im_reply_decision: None,
        remote_im_reply_target: None,
        usage: None,
        activation_request_id: {
            let value = stream_cache.request_id.trim();
            (!value.is_empty()).then(|| value.to_string())
        },
    }))
}

