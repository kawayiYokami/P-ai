pub(crate) fn register_chat_event_delta_channel(
    state: &AppState,
    event_id: &str,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(), String> {
    state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?
        .insert(event_id.to_string(), on_delta);
    Ok(())
}

pub(crate) fn register_chat_event_runtime(
    state: &AppState,
    event_id: &str,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
    sender: tokio::sync::oneshot::Sender<Result<SendChatResult, String>>,
) -> Result<(), String> {
    register_chat_event_delta_channel(state, event_id, on_delta)?;
    state
        .pending_chat_result_senders
        .lock()
        .map_err(|_| "Failed to lock pending chat result senders".to_string())?
        .insert(event_id.to_string(), sender);
    Ok(())
}

pub(crate) fn set_active_chat_view_stream_binding(
    state: &AppState,
    window_label: &str,
    binding_id: &str,
    conversation_id: Option<&str>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(), String> {
    let mut bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    let trimmed_window_label = window_label.trim();
    if trimmed_window_label.is_empty() {
        return Err("Missing window label when binding active chat stream".to_string());
    }
    let normalized_binding_id = normalize_active_chat_view_binding_id(binding_id);
    let binding_key = active_chat_view_binding_key(trimmed_window_label, &normalized_binding_id);
    let trimmed_conversation_id = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(conversation_id) = trimmed_conversation_id {
        bindings.insert(
            binding_key,
            ActiveChatViewBinding {
                window_label: trimmed_window_label.to_string(),
                binding_id: normalized_binding_id,
                conversation_id,
                delta_channel: on_delta,
            },
        );
    } else {
        bindings.remove(&binding_key);
    }
    Ok(())
}

fn normalize_active_chat_view_binding_id(binding_id: &str) -> String {
    let normalized = binding_id.trim();
    if normalized.is_empty() {
        "default".to_string()
    } else {
        normalized.to_string()
    }
}

fn active_chat_view_binding_key(window_label: &str, binding_id: &str) -> String {
    format!(
        "{}::{}",
        window_label.trim(),
        normalize_active_chat_view_binding_id(binding_id),
    )
}

fn collect_active_chat_view_delta_channels(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<(String, tauri::ipc::Channel<AssistantDeltaEvent>)>, String> {
    let bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    let conversation_id = conversation_id.trim();

    Ok(bindings
        .iter()
        .filter_map(|(window_label, binding)| {
            if binding.conversation_id != conversation_id {
                return None;
            }
            Some((window_label.clone(), binding.delta_channel.clone()))
        })
        .collect::<Vec<_>>())
}

fn prune_failed_active_chat_view_bindings(state: &AppState, binding_keys: &[String]) {
    if binding_keys.is_empty() {
        return;
    }
    if let Ok(mut bindings) = state.active_chat_view_bindings.lock() {
        for binding_key in binding_keys {
            bindings.remove(binding_key);
        }
    }
}

fn conversation_has_focused_chat_view(state: &AppState, conversation_id: &str) -> bool {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        return false;
    };
    let focused_window_labels = match state.active_chat_view_bindings.lock() {
        Ok(bindings) => bindings
            .values()
            .filter_map(|binding| {
                if binding.conversation_id.trim() != conversation_id.trim() {
                    return None;
                }
                Some(binding.window_label.clone())
            })
            .collect::<Vec<_>>(),
        Err(_) => return false,
    };
    if focused_window_labels.is_empty() {
        return false;
    }
    if focused_window_labels.iter().any(|window_label| {
        let Some(window) = app_handle.get_webview_window(window_label) else {
            return false;
        };
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        is_visible && is_focused
    }) {
        return true;
    }
    // VS Code 侧边栏通过 WebSocket 连接，不在 active_chat_view_bindings 中，
    // 但会注册到 detached_chat_windows；只要会话已打开就应跳过通知。
    detached_chat_window_for_conversation(conversation_id).is_some()
}

fn emit_assistant_delta_app_event(
    state: &AppState,
    conversation_id: &str,
    event: &AssistantDeltaEvent,
) {
    let conversation_title = if has_sidebar_conversation_subscriber() {
        assistant_delta_broadcast_conversation_title(state, conversation_id)
    } else {
        None
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "event": event,
        "conversationTitle": conversation_title,
    });
    // Web 桥接只靠 ide 广播收 tool_status，Tauri 窗口只靠 app_handle.emit；
    // 两条路必须独立，缺一条不能挡另一条。
    ide_chat_broadcast_notification("chat.assistantDelta", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(CHAT_ASSISTANT_DELTA_EVENT, payload);
    }
}

fn should_emit_assistant_delta_via_app_event_only(event: &AssistantDeltaEvent) -> bool {
    matches!(
        event.kind.as_deref(),
        Some("tool_status") | Some("context_usage_update")
    )
}

fn assistant_delta_broadcast_event(event: &AssistantDeltaEvent) -> AssistantDeltaEvent {
    let mut next = event.clone();
    next.delta.clear();
    next.stream_cache = None;
    next
}

/// 是否存在已映射会话的 sidebar 客户端（IDE 侧边栏 / 远程前端订阅者）。
/// 无订阅者时广播不附带会话标题，避免高频流式路径白付元数据查询与标题拼接开销。
fn has_sidebar_conversation_subscriber() -> bool {
    ide_context_chat_client_conversations()
        .lock()
        .map(|conversations| !conversations.is_empty())
        .unwrap_or(false)
}

/// 广播 chat.assistantDelta 时附加会话标题，供远程前端（手机壳层）通知对齐本地标题。
/// 标题来源与本地 live update 通知一致：会话 meta 标题 + 失败标记。
fn assistant_delta_broadcast_conversation_title(
    state: &AppState,
    conversation_id: &str,
) -> Option<String> {
    let meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(meta) => meta,
        Err(_) => return None,
    };
    if !conversation_meta_is_local_normal_chat_for_notification(&meta) {
        return None;
    }
    Some(notification_title_for_conversation_meta(
        &meta,
        local_chat_notification_settings(state, conversation_id).ui_language,
        false,
    ))
}

fn is_assistant_delta_stream_channel_event(event: &AssistantDeltaEvent) -> bool {
    if is_visible_stream_progress_event(event) {
        return true;
    }
    matches!(
        event.kind.as_deref(),
        Some("round_completed") | Some("round_failed")
    )
}

fn emit_assistant_delta_to_open_sidebar(
    state: &AppState,
    conversation_id: &str,
    event: &AssistantDeltaEvent,
) -> bool {
    let conversation_title = if has_sidebar_conversation_subscriber() {
        assistant_delta_broadcast_conversation_title(state, conversation_id)
    } else {
        None
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id.trim(),
        "event": event,
        "conversationTitle": conversation_title,
    });
    ide_chat_emit_notification_to_sidebar_conversation(
        conversation_id,
        "chat.assistantDelta",
        payload,
    ) > 0
}

fn is_visible_stream_progress_event(event: &AssistantDeltaEvent) -> bool {
    if !event.delta.is_empty() {
        return true;
    }
    matches!(
        event.kind.as_deref(),
        Some("activity_reasoning_delta") | Some("assistant_tool_event") | Some("assistant_tool_result")
    )
}

fn stream_cache_has_visible_progress(cache: &ConversationStreamRuntimeCache) -> bool {
    !cache.assistant_text.trim().is_empty()
        || !cache.tool_status_text.trim().is_empty()
        || !cache.tool_status_state.trim().is_empty()
        || !cache.stream_blocks.is_empty()
}

fn stream_blocks_debug_counts(blocks: &[AssistantStreamBlock]) -> (usize, usize, usize, usize) {
    let reasoning_len = blocks
        .iter()
        .map(|block| block.reasoning.chars().count())
        .sum::<usize>();
    let text_len = blocks
        .iter()
        .map(|block| block.text.chars().count())
        .sum::<usize>();
    let tool_count = blocks
        .iter()
        .map(|block| block.tools.len())
        .sum::<usize>();
    (blocks.len(), reasoning_len, text_len, tool_count)
}

fn now_unix_ms() -> u64 {
    let millis = now_utc().unix_timestamp_nanos() / 1_000_000;
    if millis <= 0 {
        0
    } else {
        millis.min(i128::from(u64::MAX)) as u64
    }
}

fn stream_cache_current_block_mut(cache: &mut ConversationStreamRuntimeCache) -> &mut AssistantStreamBlock {
    if cache.stream_blocks.is_empty() {
        cache.stream_blocks.push(AssistantStreamBlock::default());
    }
    let index = cache.stream_blocks.len().saturating_sub(1);
    &mut cache.stream_blocks[index]
}

fn stream_block_has_inline_tool_marker(text: &str, tool_call_id: &str) -> bool {
    !tool_call_id.trim().is_empty() && text.contains(&format!("[toolcall:{}]", tool_call_id.trim()))
}

fn append_stream_text_block(cache: &mut ConversationStreamRuntimeCache, delta: &str) {
    if delta.is_empty() {
        return;
    }
    // 工具标记后的新正文：切新块，与前端 appendTextDeltaToStreamBlocks 一致。
    // 块间边界由前端拼接层按「前段含工具标记」注入占位符。
    let should_break = cache
        .stream_blocks
        .last()
        .is_some_and(|block| block.pending_text_break && !block.text.trim().is_empty());
    if should_break {
        cache.stream_blocks.push(AssistantStreamBlock::default());
    }
    let block = stream_cache_current_block_mut(cache);
    block.text.push_str(delta);
    block.pending_text_break = false;
}

fn append_stream_reasoning_block(cache: &mut ConversationStreamRuntimeCache, delta: &str) {
    if delta.is_empty() {
        return;
    }
    if cache
        .stream_blocks
        .last()
        .is_some_and(|block| !block.text.trim().is_empty() || !block.tools.is_empty())
    {
        cache.stream_blocks.push(AssistantStreamBlock::default());
    }
    let block = stream_cache_current_block_mut(cache);
    block.reasoning.push_str(delta);
}

fn apply_tool_result_to_stream_blocks(
    cache: &mut ConversationStreamRuntimeCache,
    message: &str,
) {
    let Ok(event) = serde_json::from_str::<Value>(message) else {
        return;
    };
    let role = event
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if role != "tool" {
        return;
    }
    let tool_call_id = event
        .get("tool_call_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if tool_call_id.is_empty() {
        return;
    }
    let result_text = event
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    for block_index in 0..cache.stream_blocks.len() {
        let Some(tool_index) = cache.stream_blocks[block_index]
            .tools
            .iter()
            .find(|tool| tool.tool_call_id.trim() == tool_call_id)
            .map(|tool| {
                cache.stream_blocks[block_index]
                    .tools
                    .iter()
                    .position(|item| item.tool_call_id.trim() == tool.tool_call_id.trim())
                    .unwrap_or(0)
            }) else {
            continue;
        };
        cache.stream_blocks[block_index].tools[tool_index].result_text = result_text.clone();
        cache.stream_blocks[block_index].tools[tool_index].status = "done".to_string();

        let mut target_index = block_index;
        if cache.stream_blocks[target_index].text.trim().is_empty() {
            for index in (0..block_index).rev() {
                if cache.stream_blocks[index].text.trim().is_empty() {
                    continue;
                }
                target_index = index;
                break;
            }
        }
        let current_text = cache.stream_blocks[target_index].text.clone();
        if !stream_block_has_inline_tool_marker(&current_text, tool_call_id) {
            // 模型 delta 常以换行结尾；尾随空白会让渲染层 pre-wrap 段落把标记徽章顶到下一行
            cache.stream_blocks[target_index].text = if current_text.trim().is_empty() {
                format!("[toolcall:{}]", tool_call_id)
            } else {
                format!("{} [toolcall:{}]", current_text.trim_end(), tool_call_id)
            };
            cache.stream_blocks[target_index].pending_text_break = true;
        }
        return;
    }
}

fn assistant_tool_event_calls(event: &Value) -> Vec<AssistantStreamToolBlock> {
    event
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let tool_call_id = call
                        .get("id")
                        .or_else(|| call.get("call_id"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    let function = call.get("function")?;
                    let name = function
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())?;
                    let args_text = match function.get("arguments") {
                        Some(Value::String(text)) => text.clone(),
                        Some(value) => value.to_string(),
                        None => String::new(),
                    };
                    Some(AssistantStreamToolBlock {
                        tool_call_id: tool_call_id.to_string(),
                        name: name.to_string(),
                        args_text: args_text.trim().to_string(),
                        result_text: String::new(),
                        status: "doing".to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn apply_assistant_tool_event_to_stream_blocks(
    cache: &mut ConversationStreamRuntimeCache,
    message: &str,
) {
    let Ok(event) = serde_json::from_str::<Value>(message) else {
        return;
    };
    let reasoning = event
        .get("reasoning_content")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let tools = assistant_tool_event_calls(&event);
    if reasoning.is_empty() && tools.is_empty() {
        return;
    }
    if (!reasoning.is_empty())
        && cache
            .stream_blocks
            .last()
            .is_some_and(|block| !block.text.trim().is_empty())
    {
        cache.stream_blocks.push(AssistantStreamBlock::default());
    }
    if !reasoning.is_empty() && cache.activity_reasoning_text.trim().is_empty() {
        cache.activity_reasoning_text.push_str(reasoning);
    }
    let target_index = {
        let block = stream_cache_current_block_mut(cache);
        if !reasoning.is_empty() && block.reasoning.trim().is_empty() {
            block.reasoning.push_str(reasoning);
        }
        cache.stream_blocks.len().saturating_sub(1)
    };
    for tool in tools {
        if let Some(existing) = cache
            .stream_blocks
            .iter_mut()
            .flat_map(|block| block.tools.iter_mut())
            .find(|existing| existing.tool_call_id.trim() == tool.tool_call_id)
        {
            existing.name = tool.name;
            if !tool.args_text.is_empty() {
                existing.args_text = tool.args_text;
            }
            existing.status = tool.status;
            continue;
        }
        if let Some(block) = cache.stream_blocks.get_mut(target_index) {
            block.tools.push(tool);
        }
    }
}

#[cfg(test)]
mod scheduler_stream_block_tests {
    use super::*;

    #[test]
    fn active_chat_view_binding_key_should_distinguish_views_in_same_window() {
        let main_key = active_chat_view_binding_key("chat", "view-main");
        let side_key = active_chat_view_binding_key("chat", "view-side");

        assert_ne!(main_key, side_key);
        assert_eq!(main_key, "chat::view-main");
        assert_eq!(side_key, "chat::view-side");
        assert_eq!(active_chat_view_binding_key("chat", ""), "chat::default");
    }

    fn assistant_delta_event_for_test(kind: Option<&str>, delta: &str) -> AssistantDeltaEvent {
        AssistantDeltaEvent {
            delta: delta.to_string(),
            kind: kind.map(ToOwned::to_owned),
            request_id: Some("request-1".to_string()),
            activation_id: Some("request-1".to_string()),
            phase_id: None,
            reason: None,
            tool_name: None,
            tool_call_id: None,
            tool_status: None,
            tool_args: None,
            message: Some("message".to_string()),
            stream_cache: None,
        }
    }

    fn stream_cache_snapshot_for_test() -> ConversationStreamRuntimeCacheSnapshot {
        ConversationStreamRuntimeCacheSnapshot {
            activation_id: "request-1".to_string(),
            request_id: "request-1".to_string(),
            agent_id: "agent-1".to_string(),
            assistant_text: "assistant text".to_string(),
            tool_status_text: "running".to_string(),
            tool_status_state: "running".to_string(),
            stream_blocks: Vec::new(),
            started_at: "2026-01-01T00:00:00Z".to_string(),
            started_at_ms: 1,
            updated_at: "2026-01-01T00:00:01Z".to_string(),
            has_visible_progress: true,
            persisted_assistant_message_id: "assistant-1".to_string(),
            context_usage_ratio: 0.0,
            context_usage_percent: 0,
            effective_prompt_tokens: 0,
            context_window_tokens: 0,
            scheduling_state: "idle".to_string(),
        }
    }

    #[test]
    fn stream_cache_snapshot_should_carry_context_usage_fields() {
        // 钉死：流式缓存快照必须携带上下文用量字段（ratio/percent/tokens），
        // 前端随每个 delta 事件的 stream_cache 拿到最新占用率；若丢失这些字段，
        // 工具执行期间的用量动态更新与切屏恢复都会失效。
        let mut cache = ConversationStreamRuntimeCache::default();
        cache.activation_id = "request-1".to_string();
        cache.persisted_assistant_message_id = "assistant-1".to_string();
        cache.assistant_text = "assistant text".to_string();
        cache.tool_status_text = "running".to_string();
        cache.tool_status_state = "running".to_string();
        cache.updated_at = "2026-01-01T00:00:01Z".to_string();
        cache.started_at = "2026-01-01T00:00:00Z".to_string();
        cache.started_at_ms = 1;
        cache.context_usage_ratio = 0.42;
        cache.context_usage_percent = 42;
        cache.effective_prompt_tokens = 4200;
        cache.context_window_tokens = 10_000;

        let snapshot = conversation_stream_runtime_cache_snapshot(cache);

        assert_eq!(snapshot.context_usage_ratio, 0.42);
        assert_eq!(snapshot.context_usage_percent, 42);
        assert_eq!(snapshot.effective_prompt_tokens, 4200);
        assert_eq!(snapshot.context_window_tokens, 10_000);
    }

    #[test]
    fn high_frequency_deltas_should_skip_stream_cache_snapshot() {
        // 钉死：高频逐 token 事件（正文 delta kind=None、思维链 delta
        // activity_reasoning_delta）不随事件下发 stream_cache——前端只要看到
        // streamCache 就走 reduceStreamSnapshot 全量覆盖，轻量快照的
        // assistantText 为空会把正文覆盖成空白，实测表现为正文/思维链完全不显示、
        // 只有工具事件带完整快照时刷一下。无 streamCache 时前端才走增量渲染。
        // 工具三兄弟（低频关键事件）必须保持完整快照做权威校正。
        assert!(should_use_slim_stream_cache_snapshot(None));
        assert!(should_use_slim_stream_cache_snapshot(Some("activity_reasoning_delta")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("assistant_tool_event")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("assistant_tool_result")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("tool_status")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("round_completed")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("round_failed")));
        assert!(!should_use_slim_stream_cache_snapshot(Some("context_usage_update")));
    }

    #[test]
    fn assistant_delta_broadcast_event_should_strip_high_frequency_payload() {
        let mut event = assistant_delta_event_for_test(Some("tool_status"), "token");
        event.tool_status = Some("running".to_string());
        event.stream_cache = Some(stream_cache_snapshot_for_test());

        let broadcast = assistant_delta_broadcast_event(&event);

        assert!(broadcast.delta.is_empty());
        assert!(broadcast.stream_cache.is_none());
        assert_eq!(broadcast.kind.as_deref(), Some("tool_status"));
        assert_eq!(broadcast.tool_status.as_deref(), Some("running"));
        assert_eq!(broadcast.message.as_deref(), Some("message"));
    }

    #[test]
    fn assistant_delta_stream_channel_event_should_only_match_stream_owned_events() {
        assert!(is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(None, "token")
        ));
        assert!(is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("activity_reasoning_delta"), "")
        ));
        assert!(is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("assistant_tool_event"), "")
        ));
        assert!(is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("assistant_tool_result"), "")
        ));
        assert!(is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("round_completed"), "")
        ));
        assert!(!is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("tool_status"), "")
        ));
        assert!(!is_assistant_delta_stream_channel_event(
            &assistant_delta_event_for_test(Some("context_usage_update"), "")
        ));
    }

    #[test]
    fn activation_channel_should_only_be_fallback_when_active_view_did_not_receive_event() {
        assert!(!should_use_activation_delta_fallback(true, true));
        assert!(should_use_activation_delta_fallback(false, true));
        assert!(!should_use_activation_delta_fallback(false, false));
    }

    #[test]
    fn assistant_tool_event_should_project_reasoning_and_tool_into_stream_block() {
        let mut cache = ConversationStreamRuntimeCache::default();
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先说明要使用等待工具。",
                "tool_calls": [{
                    "id": "call-wait",
                    "call_id": "call-wait",
                    "type": "function",
                    "function": {
                        "name": "operate",
                        "arguments": "{\"method\":\"wait3\"}"
                    }
                }]
            })
            .to_string(),
        );

        assert_eq!(cache.stream_blocks.len(), 1);
        assert_eq!(cache.stream_blocks[0].reasoning, "先说明要使用等待工具。");
        assert_eq!(cache.stream_blocks[0].tools.len(), 1);
        assert_eq!(cache.stream_blocks[0].tools[0].tool_call_id, "call-wait");
        assert_eq!(cache.stream_blocks[0].tools[0].name, "operate");
        assert_eq!(cache.stream_blocks[0].tools[0].status, "doing");
    }

    #[test]
    fn assistant_tool_result_should_complete_existing_stream_tool() {
        let mut cache = ConversationStreamRuntimeCache::default();
        append_stream_text_block(&mut cache, "先说明要等待。");
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-wait",
                    "type": "function",
                    "function": {
                        "name": "operate",
                        "arguments": "{\"method\":\"wait3\"}"
                    }
                }]
            })
            .to_string(),
        );
        apply_tool_result_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-wait",
                "content": "等待完成"
            })
            .to_string(),
        );

        assert_eq!(cache.stream_blocks.len(), 1);
        assert_eq!(cache.stream_blocks[0].tools.len(), 1);
        assert_eq!(cache.stream_blocks[0].tools[0].result_text, "等待完成");
        assert_eq!(cache.stream_blocks[0].tools[0].status, "done");
        assert_eq!(cache.stream_blocks[0].text, "先说明要等待。 [toolcall:call-wait]");
        assert!(cache.stream_blocks[0].pending_text_break);
    }

    #[test]
    fn assistant_tool_result_should_trim_trailing_whitespace_before_inline_marker() {
        let mut cache = ConversationStreamRuntimeCache::default();
        append_stream_text_block(&mut cache, "先说明要等待。\n");
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-wait",
                    "type": "function",
                    "function": {
                        "name": "operate",
                        "arguments": "{\"method\":\"wait3\"}"
                    }
                }]
            })
            .to_string(),
        );
        apply_tool_result_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-wait",
                "content": "等待完成"
            })
            .to_string(),
        );

        assert_eq!(cache.stream_blocks.len(), 1);
        assert_eq!(cache.stream_blocks[0].text, "先说明要等待。 [toolcall:call-wait]");
        assert!(cache.stream_blocks[0].pending_text_break);
    }

    #[test]
    fn multiple_tool_results_should_keep_multiple_inline_markers_and_following_text_break() {
        let mut cache = ConversationStreamRuntimeCache::default();
        append_stream_text_block(&mut cache, "先说明要并发读取。");
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-a",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"a.ts\"}"
                    }
                }, {
                    "id": "call-b",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"b.ts\"}"
                    }
                }]
            })
            .to_string(),
        );
        apply_tool_result_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-a",
                "content": "A 完成"
            })
            .to_string(),
        );
        apply_tool_result_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-b",
                "content": "B 完成"
            })
            .to_string(),
        );
        append_stream_text_block(&mut cache, "下面继续正文。");

        assert_eq!(cache.stream_blocks.len(), 2);
        assert_eq!(
            cache.stream_blocks[0].text,
            "先说明要并发读取。 [toolcall:call-a] [toolcall:call-b]"
        );
        assert!(cache.stream_blocks[0].pending_text_break);
        assert_eq!(cache.stream_blocks[1].text, "下面继续正文。");
        assert!(!cache.stream_blocks[1].pending_text_break);
    }

    #[test]
    fn tool_result_without_prior_text_should_still_render_marker_before_later_text() {
        let mut cache = ConversationStreamRuntimeCache::default();
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-first",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"a.ts\"}"
                    }
                }]
            })
            .to_string(),
        );
        apply_tool_result_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-first",
                "content": "A 完成"
            })
            .to_string(),
        );
        append_stream_text_block(&mut cache, "后面才开始正文。");

        assert_eq!(cache.stream_blocks.len(), 2);
        assert_eq!(cache.stream_blocks[0].text, "[toolcall:call-first]");
        assert!(cache.stream_blocks[0].pending_text_break);
        assert_eq!(cache.stream_blocks[1].text, "后面才开始正文。");
        assert!(!cache.stream_blocks[1].pending_text_break);
    }

    #[test]
    fn reasoning_after_tool_should_start_following_stream_block() {
        let mut cache = ConversationStreamRuntimeCache::default();
        append_stream_reasoning_block(&mut cache, "思维链1");
        apply_assistant_tool_event_to_stream_blocks(
            &mut cache,
            &serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-wait",
                    "type": "function",
                    "function": {
                        "name": "operate",
                        "arguments": "{\"method\":\"wait3\"}"
                    }
                }]
            })
            .to_string(),
        );
        append_stream_reasoning_block(&mut cache, "思维链2");

        assert_eq!(cache.stream_blocks.len(), 2);
        assert_eq!(cache.stream_blocks[0].reasoning, "思维链1");
        assert_eq!(cache.stream_blocks[0].tools.len(), 1);
        assert_eq!(cache.stream_blocks[1].reasoning, "思维链2");
        assert!(cache.stream_blocks[1].tools.is_empty());
    }
}

fn reset_conversation_stream_runtime_cache(
    state: &AppState,
    conversation_id: &str,
    activation_id: &str,
    request_id: &str,
    agent_id: &str,
    assistant_message_id: &str,
    started_at: &str,
    started_at_ms: u64,
) -> Result<(), String> {
    let mut slots = lock_conversation_runtime_slots(state)?;
    let slot = conversation_slot_mut(&mut slots, conversation_id);
    slot.stream_cache = ConversationStreamRuntimeCache {
        activation_id: activation_id.trim().to_string(),
        request_id: request_id.trim().to_string(),
        agent_id: agent_id.trim().to_string(),
        persisted_assistant_message_id: assistant_message_id.trim().to_string(),
        started_at: started_at.trim().to_string(),
        started_at_ms,
        updated_at: started_at.trim().to_string(),
        scheduling_state: "preparing_context".to_string(),
        ..ConversationStreamRuntimeCache::default()
    };
    Ok(())
}

fn set_stream_cache_persisted_assistant_message_id(
    state: &AppState,
    conversation_id: &str,
    assistant_message_id: &str,
) {
    let mut slots = match lock_conversation_runtime_slots(state) {
        Ok(slots) => slots,
        Err(err) => {
            runtime_log_error(format!("[聊天流式缓存] 更新 persisted_assistant_message_id 失败，锁错误: {err}"));
            return;
        }
    };
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return;
    }
    let slot = conversation_slot_mut(&mut slots, cid);
    slot.stream_cache.persisted_assistant_message_id = assistant_message_id.trim().to_string();
}

fn clear_conversation_stream_runtime_cache(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mut slots = lock_conversation_runtime_slots(state)?;
    if let Some(slot) = slots.get_mut(conversation_id.trim()) {
        let mut cleared = ConversationStreamRuntimeCache::default();
        cleared.scheduling_state = "idle".to_string();
        slot.stream_cache = cleared;
    }
    Ok(())
}

fn conversation_stream_runtime_cache_snapshot(
    stream_cache: ConversationStreamRuntimeCache,
) -> ConversationStreamRuntimeCacheSnapshot {
    let has_visible_progress = stream_cache_has_visible_progress(&stream_cache);
    ConversationStreamRuntimeCacheSnapshot {
        activation_id: stream_cache.activation_id,
        request_id: stream_cache.request_id,
        agent_id: stream_cache.agent_id,
        assistant_text: stream_cache.assistant_text,
        tool_status_text: stream_cache.tool_status_text,
        tool_status_state: stream_cache.tool_status_state,
        stream_blocks: stream_cache.stream_blocks,
        started_at: stream_cache.started_at,
        started_at_ms: stream_cache.started_at_ms,
        updated_at: stream_cache.updated_at,
        has_visible_progress,
        persisted_assistant_message_id: stream_cache.persisted_assistant_message_id,
        context_usage_ratio: stream_cache.context_usage_ratio,
        context_usage_percent: stream_cache.context_usage_percent,
        effective_prompt_tokens: stream_cache.effective_prompt_tokens,
        context_window_tokens: stream_cache.context_window_tokens,
        scheduling_state: stream_cache.scheduling_state.clone(),
    }
}

/// 判断该事件是否应随事件下发 stream_cache 快照。
/// 高频逐 token 事件（正文 delta / 思维链 delta）不挂快照：前端对无
/// streamCache 的 delta 走增量渲染逐字累积，挂了快照反而会触发前端全量
/// 覆盖路径（只要 streamCache 存在就走 reduceStreamSnapshot），轻量快照
/// 的 assistantText 为空会把正文覆盖成空白。低频关键事件（工具三兄弟）
/// 仍带完整快照做权威校正。
fn should_use_slim_stream_cache_snapshot(kind: Option<&str>) -> bool {
    matches!(kind, None | Some("activity_reasoning_delta"))
}

fn set_stream_cache_context_usage(
    state: &AppState,
    conversation_id: &str,
    effective_prompt_tokens: u64,
    context_window_tokens: u32,
) {
    if effective_prompt_tokens == 0 || context_window_tokens == 0 {
        return;
    }
    let mut slots = match lock_conversation_runtime_slots(state) {
        Ok(slots) => slots,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天流式缓存] 更新上下文用量失败，锁错误: {err}"
            ));
            return;
        }
    };
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return;
    }
    let slot = conversation_slot_mut(&mut slots, cid);
    let ratio = effective_prompt_tokens as f64 / f64::from(context_window_tokens);
    slot.stream_cache.effective_prompt_tokens = effective_prompt_tokens;
    slot.stream_cache.context_window_tokens = context_window_tokens;
    slot.stream_cache.context_usage_ratio = ratio;
    slot.stream_cache.context_usage_percent = ratio
        .mul_add(100.0, 0.0)
        .round()
        .clamp(0.0, 100.0) as u32;
}

fn update_conversation_stream_runtime_cache(
    state: &AppState,
    conversation_id: &str,
    event: &AssistantDeltaEvent,
) -> Result<Option<ConversationStreamRuntimeCacheSnapshot>, String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return Ok(None);
    }
    if event.kind.as_deref() == Some("round_completed")
        || event.kind.as_deref() == Some("round_failed")
        || event.kind.as_deref() == Some("history_flushed")
        || event.kind.as_deref() == Some("stream_rebind_required")
    {
        return Ok(None);
    }
    let has_progress = is_visible_stream_progress_event(event)
        || event.kind.as_deref() == Some("tool_status");
    if !has_progress {
        return Ok(None);
    }

    let mut slots = lock_conversation_runtime_slots(state)?;
    let slot = conversation_slot_mut(&mut slots, cid);
    let cache = &mut slot.stream_cache;
    if let Some(value) = event
        .activation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cache.activation_id = value.to_string();
    }
    if let Some(value) = event
        .request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cache.request_id = value.to_string();
    }
    let previous_scheduling_state = cache.scheduling_state.clone();
    match event.kind.as_deref() {
        Some("tool_status") => {
            // tool_status 是调度层信号，服务头像右侧/运行态提示；气泡与持久化只读取 stream_blocks。
            let tool_status = event.tool_status.as_deref().unwrap_or("").trim();
            cache.tool_status_text = event.message.clone().unwrap_or_default();
            cache.tool_status_state = match tool_status {
                "running" | "done" | "failed" => tool_status.to_string(),
                _ => String::new(),
            };
            // 7步闭环调度枚举：tool_status 文案决定 preparing/waiting/executing
            let msg = cache.tool_status_text.trim().to_string();
            if tool_status == "running" {
                if msg.contains("正在调用工具") {
                    cache.scheduling_state = "executing_tool".to_string();
                } else if msg.contains("正在进入模型请求") || msg.contains("等待回应") || msg.contains("等待响应") {
                    cache.scheduling_state = "waiting_response".to_string();
                } else if msg.contains("正在准备调度") || msg.contains("正在处理附件") || msg.contains("上下文") {
                    cache.scheduling_state = "preparing_context".to_string();
                } else if cache.scheduling_state.is_empty() || cache.scheduling_state == "idle" {
                    cache.scheduling_state = "preparing_context".to_string();
                }
            } else if tool_status == "done" || tool_status == "failed" {
                // 工具执行结束仍保留 executing_tool，直到下一轮覆盖或 round_completed 清理为 idle
                if cache.scheduling_state != "executing_tool" && msg.contains("工具") {
                    cache.scheduling_state = "executing_tool".to_string();
                }
            }
        }
        Some("activity_reasoning_delta") => {
            cache.activity_reasoning_text.push_str(&event.delta);
            append_stream_reasoning_block(cache, &event.delta);
            cache.scheduling_state = "streaming_reasoning".to_string();
        }
        Some("assistant_tool_event") => {
            apply_assistant_tool_event_to_stream_blocks(
                cache,
                event.message.as_deref().unwrap_or_default(),
            );
            cache.scheduling_state = "streaming_tool".to_string();
        }
        Some("assistant_tool_result") => {
            apply_tool_result_to_stream_blocks(
                cache,
                event.message.as_deref().unwrap_or_default(),
            );
            // 工具结果到达仍处于执行阶段，直到下一轮准备上下文覆盖
            cache.scheduling_state = "executing_tool".to_string();
        }
        _ => {
            cache.assistant_text.push_str(&event.delta);
            append_stream_text_block(cache, &event.delta);
            if !event.delta.trim().is_empty() {
                cache.scheduling_state = "streaming_text".to_string();
            }
        }
    }
    cache.updated_at = now_iso();
    // 高频逐 token 事件（正文 delta / 思维链 delta）不随事件下发 stream_cache：
    // 前端只要看到 streamCache 就走 reduceStreamSnapshot 全量覆盖路径，轻量
    // 快照的 assistantText 为空会把正文覆盖成空白；无 streamCache 时前端才走
    // 增量渲染逐字累积。后端缓存照常全量更新，恢复查询仍拿完整快照。
    // 低频关键事件（工具三兄弟）仍下发完整快照做权威校正。
    // 但调度状态首次切换（waiting -> streaming_*）必须立即推送，否则前端上面一行会卡在 waiting。
    let scheduling_changed = previous_scheduling_state != cache.scheduling_state;
    if should_use_slim_stream_cache_snapshot(event.kind.as_deref()) && !scheduling_changed {
        return Ok(None);
    }
    let snapshot = conversation_stream_runtime_cache_snapshot(cache.clone());
    Ok(Some(snapshot))
}

fn dispatch_assistant_delta_to_active_view(
    state: &AppState,
    conversation_id: &str,
    event: &AssistantDeltaEvent,
) -> bool {
    if should_emit_assistant_delta_via_app_event_only(event) {
        let broadcast_event = assistant_delta_broadcast_event(event);
        emit_assistant_delta_app_event(state, conversation_id, &broadcast_event);
        // App event / IDE 广播不能证明本次发送请求的临时 Channel 已收到事件。
        // 返回 false，让调用方在存在 activation Channel 时继续做请求级兜底。
        return false;
    }

    if !is_assistant_delta_stream_channel_event(event) {
        return false;
    }

    let targets =
        collect_active_chat_view_delta_channels(state, conversation_id).unwrap_or_default();
    let ide_sidebar_delivered = emit_assistant_delta_to_open_sidebar(
        state,
        conversation_id,
        event,
    );
    if targets.is_empty() && !ide_sidebar_delivered {
        if matches!(
            event.kind.as_deref(),
            Some("assistant_tool_event") | Some("assistant_tool_result") | Some("tool_status")
        ) {
            runtime_log_debug(format!(
                "[聊天流式订阅] 跳过，conversation_id={} kind={} reason=无订阅者",
                conversation_id.trim(),
                event.kind.as_deref().unwrap_or("delta"),
            ));
        }
        return false;
    }
    let target_count = targets.len();
    let mut delivered = false;
    let mut failed_labels = Vec::<String>::new();
    for (window_label, channel) in targets {
        match channel.send(event.clone()) {
            Ok(_) => {
                delivered = true;
            }
            Err(_) => {
                failed_labels.push(window_label);
            }
        }
    }
    prune_failed_active_chat_view_bindings(state, &failed_labels);
    if !delivered || !failed_labels.is_empty() {
        // 只在投递失败时记录；成功路径静默，避免流式高频事件刷日志写盘。
        let stream_cache_blocks = event
            .stream_cache
            .as_ref()
            .map(|cache| cache.stream_blocks.as_slice())
            .unwrap_or(&[]);
        let (block_count, reasoning_len, text_len, tool_count) =
            stream_blocks_debug_counts(stream_cache_blocks);
        runtime_log_error(format!(
            "[聊天流式块][后端发送失败] conversation_id={} kind={} channel_targets={} delivered={} failed={} has_stream_cache={} block_count={} reasoning_len={} text_len={} tool_count={}",
            conversation_id.trim(),
            event.kind.as_deref().unwrap_or("delta"),
            target_count,
            delivered || ide_sidebar_delivered,
            failed_labels.len(),
            event.stream_cache.is_some(),
            block_count,
            reasoning_len,
            text_len,
            tool_count,
        ));
    }
    // IDE 侧边栏与桌面对话视图是独立消费者。侧边栏投递成功不能阻止
    // activation Channel 为尚未建立长期绑定的桌面视图兜底。
    delivered
}

fn should_use_activation_delta_fallback(
    active_view_delivered: bool,
    has_activation_channel: bool,
) -> bool {
    !active_view_delivered && has_activation_channel
}

fn emit_stream_rebind_required_event(
    state: &AppState,
    conversation_id: &str,
    request_id: Option<&str>,
    phase_id: Option<&str>,
    reason: &str,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id.trim(),
        "requestId": request_id.map(str::trim).filter(|value| !value.is_empty()),
        "phaseId": phase_id.map(str::trim).filter(|value| !value.is_empty()),
        "reason": reason.trim(),
    });
    let _ = app_handle.emit(CHAT_STREAM_REBIND_REQUIRED_EVENT, &payload);
    ide_chat_broadcast_notification("chat.streamRebindRequired", payload);
}

#[allow(dead_code)]
pub(crate) fn clear_active_chat_view_stream_binding(
    state: &AppState,
    window_label: &str,
    binding_id: &str,
) -> Result<(), String> {
    let mut bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    bindings.remove(&active_chat_view_binding_key(window_label, binding_id));
    Ok(())
}

/// 清空指定窗口的全部活动聊天流绑定。
///
/// 前端页面重载（HMR / 手动刷新 / 崩溃重建）后，旧 bindingId 的 channel 在
/// JS 侧已失效，但 Rust 侧仍持有注册；Tauri 的 Channel::send 在 callback
/// 不存在时仍返回 Ok，导致僵尸注册无法通过 send 失败自动清理，流式期间会
/// 反复向失效 channel 投递并刷 `Couldn't find callback id` 警告。
/// 前端在每次窗口启动/重载后调用本命令，先清掉本窗口残留绑定，再重新绑定新 channel。
pub(crate) fn clear_window_chat_view_stream_bindings(
    state: &AppState,
    window_label: &str,
) -> Result<(), String> {
    let mut bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    let window_label = window_label.trim();
    if window_label.is_empty() {
        return Ok(());
    }
    let removed = bindings
        .keys()
        .filter(|key| key.starts_with(&format!("{}::", window_label)))
        .cloned()
        .collect::<Vec<_>>();
    for key in &removed {
        bindings.remove(key);
    }
    if !removed.is_empty() {
        runtime_log_debug(format!(
            "[聊天流式订阅] 窗口重载后清理残留绑定: window={}, removed={}",
            window_label,
            removed.len()
        ));
    }
    Ok(())
}
