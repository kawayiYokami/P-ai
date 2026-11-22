fn should_stop_after_contact_tool(tool_name: &str, tool_result: &ProviderToolResult) -> bool {
    if tool_name != "contact_send_files" {
        return false;
    }
    matches!(
        tool_result.metadata.control,
        ProviderToolControl::Contact { stop: true }
    )
}

fn contact_tool_should_run_last(tool_name: &str, tool_args: &str) -> bool {
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

fn finalize_remote_im_stop_model_reply(
    full_assistant_text: &str,
    full_activity_reasoning_text: String,
    final_assistant_provider_meta_override: Option<Value>,
    tool_history_events: Vec<Value>,
    trusted_input_tokens: Option<u64>,
    latest_usage: Option<Value>,
) -> ModelReply {
    let final_text = if full_assistant_text.trim().is_empty() {
        "已发送完成。".to_string()
    } else {
        full_assistant_text.to_string()
    };

    ModelReply {
        assistant_text: final_text.clone(),
        final_response_text: final_text,
        activity_reasoning_text: full_activity_reasoning_text,
        assistant_provider_meta: final_assistant_provider_meta_override,
        tool_history_events,
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage: latest_usage,
        round_logs_recorded_internally: true,
    }
}

fn tool_loop_assistant_tool_event_text(event: &Value) -> String {
    match event.get("content") {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string().trim().to_string(),
    }
}

fn tool_loop_first_assistant_tool_call_id(event: &Value) -> Option<String> {
    event
        .get("tool_calls")
        .and_then(Value::as_array)
        .and_then(|calls| calls.first())
        .and_then(|call| call.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn tool_loop_tool_result_call_id(event: &Value) -> Option<String> {
    event
        .get("tool_call_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn tool_loop_is_first_tool_result_in_group(
    assistant_tool_call_event: &Value,
    tool_result_event: &Value,
) -> bool {
    let Some(first_call_id) = tool_loop_first_assistant_tool_call_id(assistant_tool_call_event) else {
        return false;
    };
    tool_loop_tool_result_call_id(tool_result_event)
        .map(|call_id| call_id == first_call_id)
        .unwrap_or(false)
}

fn maybe_spawn_remote_im_round_intermediate_send(
    state: &AppState,
    context: &ToolLoopAutoCompactionContext,
    turn_text: &str,
) {
    // 只服务远程应答委托：私聊自动推送由工具持久化路径负责，这里不能重复发。
    if context.remote_im_reply_delegate_id.is_none() {
        return;
    }
    let Some(activation_source) = context.remote_im_auto_send_source.clone() else {
        return;
    };
    let text = turn_text.trim();
    if text.is_empty() {
        return;
    }
    // 逐轮外发：本轮正文一产出就先发出去，发完再执行这一轮的工具调用。
    // 发送是 fire-and-forget，失败只记日志，调度不等结果。
    spawn_remote_im_auto_send_intermediate_text(
        state.clone(),
        activation_source,
        text.to_string(),
    );
}

fn maybe_spawn_remote_im_tool_persist_auto_send(
    state: &AppState,
    context: &ToolLoopAutoCompactionContext,
    assistant_message_id: &str,
    assistant_tool_call_event: &Value,
    tool_result_event: &Value,
) {
    if context.remote_im_reply_delegate_id.is_some() {
        runtime_log_debug(format!(
            "[远程IM][工具持久化自动发送] 跳过，conversation_id={}，reason=group_reply_delegate_buffers_until_final",
            context.conversation_id
        ));
        return;
    }
    let Some(activation_source) = context.remote_im_auto_send_source.clone() else {
        return;
    };
    if !tool_loop_is_first_tool_result_in_group(assistant_tool_call_event, tool_result_event) {
        runtime_log_debug(format!(
            "[远程IM][工具持久化自动发送] 跳过，任务=tool_persist_auto_send，conversation_id={}，assistant_message_id={}，contact_id={}，reason=not_first_tool_result",
            context.conversation_id,
            assistant_message_id,
            activation_source.remote_contact_id
        ));
        return;
    }
    let assistant_text = tool_loop_assistant_tool_event_text(assistant_tool_call_event);
    if assistant_text.is_empty() {
        runtime_log_warn(format!(
            "[远程IM][工具持久化自动发送] 跳过，任务=tool_persist_auto_send，conversation_id={}，assistant_message_id={}，contact_id={}，reason=empty_text",
            context.conversation_id,
            assistant_message_id,
            activation_source.remote_contact_id
        ));
        return;
    }
    spawn_remote_im_auto_send_contact_assistant_reply(
        state.clone(),
        activation_source,
        context.conversation_id.clone(),
        assistant_text,
        None,
        Some(assistant_message_id.to_string()),
    );
}
