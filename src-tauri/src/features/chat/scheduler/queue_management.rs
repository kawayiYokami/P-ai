fn lock_conversation_runtime_slots(
    state: &AppState,
) -> Result<
    std::sync::MutexGuard<'_, std::collections::HashMap<String, ConversationRuntimeSlot>>,
    String,
> {
    match state.conversation_runtime_slots.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            runtime_log_info(format!("[聊天] 警告: conversation_runtime_slots 锁已 poison，正在继续恢复使用"));
            Ok(poisoned.into_inner())
        }
    }
}

fn lock_conversation_processing_claims(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, std::collections::HashSet<String>>, String> {
    match state.conversation_processing_claims.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            runtime_log_info(format!(
                "[聊天] 警告: conversation_processing_claims 锁已 poison，正在继续恢复使用"
            ));
            Ok(poisoned.into_inner())
        }
    }
}

fn conversation_slot_mut<'a>(
    slots: &'a mut std::collections::HashMap<String, ConversationRuntimeSlot>,
    conversation_id: &str,
) -> &'a mut ConversationRuntimeSlot {
    slots.entry(conversation_id.to_string()).or_insert_with(|| {
        let mut slot = ConversationRuntimeSlot::default();
        slot.last_activity_at = now_iso();
        slot
    })
}

fn conversation_running_slot_count(
    claims: &std::collections::HashSet<String>,
    conversation_id: &str,
) -> usize {
    usize::from(claims.contains(conversation_id))
}

/// 获取队列状态
pub(crate) fn get_queue_snapshot(state: &AppState) -> Result<Vec<ChatQueueEventSummary>, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    let mut summaries = Vec::<ChatQueueEventSummary>::new();
    for slot in slots.values() {
        for event in &slot.pending_queue {
            let message_preview = event
                .messages
                .first()
                .and_then(|msg| {
                    msg.parts.iter().find_map(|part| match part {
                        MessagePart::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                })
                .unwrap_or_default();
            let preview = if message_preview.chars().count() > 50 {
                format!(
                    "{}...",
                    message_preview.chars().take(50).collect::<String>()
                )
            } else {
                message_preview.clone()
            };
            summaries.push(ChatQueueEventSummary {
                id: event.id.clone(),
                source: event.source.clone(),
                queue_mode: event.queue_mode.clone(),
                created_at: event.created_at.clone(),
                message_preview: preview,
                message_text: message_preview,
                conversation_id: event.conversation_id.clone(),
                sender_name: event
                    .messages
                    .first()
                    .and_then(|message| message.provider_meta.as_ref())
                    .and_then(|meta| meta.get("teamName"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            });
        }
    }
    summaries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(summaries)
}

pub(crate) fn emit_chat_queue_snapshot(state: &AppState) {
    let queue_events = get_queue_snapshot(state).unwrap_or_default();
    let session_state = get_main_session_state(state).unwrap_or(MainSessionState::Idle);
    let payload = ChatQueueSnapshotPush {
        queue_events,
        session_state,
    };
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(CHAT_QUEUE_SNAPSHOT_EVENT, &payload);
    }
    if let Ok(value) = serde_json::to_value(&payload) {
        ide_chat_broadcast_notification("chat.queueSnapshotUpdated", value);
    }
}

/// 将普通队列消息退回输入框
pub(crate) fn recall_queue_event(
    state: &AppState,
    event_id: &str,
) -> Result<Option<ChatPendingEvent>, String> {
    // 队列修改统一走 dequeue_lock -> queue_lock，保证进出队原子顺序一致。
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let mut removed = None;
    let mut remaining_queue_len = 0usize;
    let mut changed_conversation_id = None::<String>;
    for (conversation_id, slot) in slots.iter_mut() {
        if let Some(pos) = slot.pending_queue.iter().position(|e| e.id == event_id) {
            if slot.pending_queue[pos].queue_mode == ChatQueueMode::Guided {
                return Err("引导中的消息不能移出队列".to_string());
            }
            removed = slot.pending_queue.remove(pos);
            remaining_queue_len = slot.pending_queue.len();
            changed_conversation_id = Some(conversation_id.clone());
            break;
        }
    }
    drop(slots);
    drop(_dequeue_guard);
    if let Some(conversation_id) = changed_conversation_id.as_deref() {
        sync_active_dispatch(state, conversation_id);
    }
    if removed.is_some() {
        runtime_log_info(format!(
            "[聊天] 队列消息退回输入框 队列长度={}",
            remaining_queue_len
        ));
        emit_chat_queue_snapshot(state);
        complete_pending_chat_events_with_error(
            state,
            &[event_id.to_string()],
            "消息已退回输入框",
        )?;
    }
    Ok(removed)
}

pub(crate) fn mark_queue_event_guided_with_log(
    state: &AppState,
    event_id: &str,
    emit_log: bool,
) -> Result<Option<String>, String> {
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let mut updated_conversation_id = None::<String>;
    for slot in slots.values_mut() {
        if let Some(event) = slot.pending_queue.iter_mut().find(|item| item.id == event_id) {
            if !matches!(event.source, ChatEventSource::User | ChatEventSource::RemoteIm) {
                return Err("只有用户或远程联系人队列消息可以设置为引导".to_string());
            }
            if event.queue_mode == ChatQueueMode::Guided {
                updated_conversation_id = Some(event.conversation_id.clone());
                break;
            }
            event.queue_mode = ChatQueueMode::Guided;
            event.activate_assistant = true;
            let guided_event_source = match event.source {
                ChatEventSource::RemoteIm => "remote_im",
                _ => "user_message",
            };
            if let Some(runtime_context) = event.runtime_context.as_mut() {
                runtime_context.dispatch_reason = Some("guided_queue".to_string());
            } else {
                event.runtime_context = Some(runtime_context_new(guided_event_source, "guided_queue"));
            }
            updated_conversation_id = Some(event.conversation_id.clone());
            break;
        }
    }
    drop(slots);
    drop(_dequeue_guard);
    if let Some(conversation_id) = updated_conversation_id.as_deref() {
        sync_active_dispatch(state, conversation_id);
        if emit_log {
            runtime_log_info(format!(
                "[聊天] 开始 任务=mark_queue_event_guided"
            ));
        }
        emit_chat_queue_snapshot(state);
    }
    Ok(updated_conversation_id)
}

pub(crate) fn mark_queue_event_guided(
    state: &AppState,
    event_id: &str,
) -> Result<Option<String>, String> {
    mark_queue_event_guided_with_log(state, event_id, true)
}

pub(crate) fn clear_conversation_queue(
    state: &AppState,
    conversation_id: &str,
    error_message: &str,
) -> Result<usize, String> {
    let trimmed_conversation_id = conversation_id.trim();
    if trimmed_conversation_id.is_empty() {
        return Ok(0);
    }
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let Some(slot) = slots.get_mut(trimmed_conversation_id) else {
        return Ok(0);
    };
    let removed_events = slot.pending_queue.drain(..).collect::<Vec<_>>();
    slot.last_activity_at = now_iso();
    let removed_count = removed_events.len();
    let removed_event_ids = removed_events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    drop(slots);
    drop(_dequeue_guard);
    if removed_count > 0 {
        sync_active_dispatch(state, trimmed_conversation_id);
        runtime_log_info(format!(
            "[聊天] 清空会话队列 移除数={}",
            removed_count
        ));
        emit_chat_queue_snapshot(state);
        complete_pending_chat_events_with_error(state, &removed_event_ids, error_message)?;
    }
    Ok(removed_count)
}

fn claim_guided_queue_events_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<ChatPendingEvent>, String> {
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut claims = lock_conversation_processing_claims(state)?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let trimmed_conversation_id = conversation_id.trim();
    let slot = conversation_slot_mut(&mut slots, trimmed_conversation_id);
    let has_guided = slot
        .pending_queue
        .iter()
        .any(|event| event.queue_mode == ChatQueueMode::Guided);
    if slot.state != MainSessionState::Idle
        || !has_guided
        || claims.contains(trimmed_conversation_id)
        || claims.len() >= CHAT_CONCURRENCY_LIMIT
    {
        return Ok(Vec::new());
    }

    claims.insert(trimmed_conversation_id.to_string());
    slot.last_activity_at = now_iso();

    let mut guided_events = Vec::<ChatPendingEvent>::new();
    let mut remaining_queue = std::collections::VecDeque::<ChatPendingEvent>::new();
    while let Some(mut event) = slot.pending_queue.pop_front() {
        if event.queue_mode == ChatQueueMode::Guided {
            event.activate_assistant = true;
            if let Some(runtime_context) = event.runtime_context.as_mut() {
                runtime_context.dispatch_reason = Some("guided_queue".to_string());
            } else {
                event.runtime_context = Some(runtime_context_new("user_message", "guided_queue"));
            }
            guided_events.push(event);
        } else {
            remaining_queue.push_back(event);
        }
    }
    slot.pending_queue = remaining_queue;

    if guided_events.is_empty() {
        claims.remove(trimmed_conversation_id);
        if slot.pending_queue.is_empty() {
            slot.state = MainSessionState::Idle;
        }
    }
    // 认领即记：队列变了就同步一次，空了且有引导被取走就记空队列。
    let record_key = trimmed_conversation_id.to_string();
    drop(slots);
    drop(claims);
    drop(_dequeue_guard);
    sync_active_dispatch(state, &record_key);

    Ok(guided_events)
}

fn remove_queue_events_by_ids(
    state: &AppState,
    conversation_id: &str,
    event_ids: &[String],
) -> Result<usize, String> {
    if event_ids.is_empty() {
        return Ok(0);
    }
    let id_set = event_ids.iter().map(|item| item.as_str()).collect::<std::collections::HashSet<_>>();
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let Some(slot) = slots.get_mut(conversation_id) else {
        return Ok(0);
    };
    let before = slot.pending_queue.len();
    slot.pending_queue.retain(|event| !id_set.contains(event.id.as_str()));
    let removed = before.saturating_sub(slot.pending_queue.len());
    if removed > 0 {
        slot.last_activity_at = now_iso();
    }
    drop(slots);
    drop(_dequeue_guard);
    if removed > 0 {
        sync_active_dispatch(state, conversation_id);
    }
    Ok(removed)
}

fn conversation_has_guided_queue_events(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(conversation_id)
        .map(|slot| {
            slot.pending_queue
                .iter()
                .any(|event| event.queue_mode == ChatQueueMode::Guided)
        })
        .unwrap_or(false))
}

fn conversation_has_pending_queue_events(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(conversation_id.trim())
        .map(|slot| !slot.pending_queue.is_empty())
        .unwrap_or(false))
}

fn conversation_is_idle_for_goal_fallback(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    let claims = lock_conversation_processing_claims(state)?;
    if claims.contains(conversation_id) {
        return Ok(false);
    }
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(conversation_id)
        .map(|slot| slot.state == MainSessionState::Idle && slot.pending_queue.is_empty())
        .unwrap_or(true))
}

fn message_is_goal_continue(message: &ChatMessage) -> bool {
    let role = message.role.trim();
    let speaker_id = message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    let message_kind = message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("messageKind"))
        .and_then(Value::as_str)
        .map(str::trim);
    matches!(role, "assistant" | "system")
        && speaker_id == SYSTEM_PERSONA_ID
        && message_kind == Some("goal_continue")
}

fn event_is_goal_continue(event: &ChatPendingEvent) -> bool {
    event.messages.iter().any(message_is_goal_continue)
}

fn goal_continue_is_suppressed(state: &AppState, conversation_id: &str) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let ids = state
        .goal_continue_suppressed_conversation_ids
        .lock()
        .map_err(|_| "Failed to lock goal continue suppression ids".to_string())?;
    Ok(ids.contains(conversation_id))
}

pub(crate) fn mark_goal_continue_suppressed_by_user_interrupt(
    state: &AppState,
    conversation_id: &str,
    reason: &str,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(());
    }
    let mut ids = state
        .goal_continue_suppressed_conversation_ids
        .lock()
        .map_err(|_| "Failed to lock goal continue suppression ids".to_string())?;
    let inserted = ids.insert(conversation_id.to_string());
    if inserted {
        runtime_log_info(format!(
            "[聊天] 暂停 任务=用户中断 原因={}",
            reason.trim()
        ));
    }
    Ok(())
}

pub(crate) fn clear_goal_continue_suppression(
    state: &AppState,
    conversation_id: &str,
    reason: &str,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(());
    }
    let mut ids = state
        .goal_continue_suppressed_conversation_ids
        .lock()
        .map_err(|_| "Failed to lock goal continue suppression ids".to_string())?;
    let removed = ids.remove(conversation_id);
    if removed {
        runtime_log_info(format!(
            "[聊天] 恢复 任务=清除用户中断暂停 原因={}",
            reason.trim()
        ));
    }
    Ok(())
}

fn tool_loop_should_close_for_guided_queue(
    state: Option<&AppState>,
    conversation_id: Option<&str>,
) -> bool {
    // 注意：这里判断的不是“当前轮次是否已经完整结束”，
    // 而是“在一次工具执行完成之后，是否存在待插入的引导消息”。
    // 一旦为 true，当前调度应在这个工具切点收口，并把后续回复让位给引导重启的新轮次。
    // 会话 id 单独传：动态边界下压缩上下文是 None，引导投送不能跟着一起失效。
    let Some(state) = state else {
        return false;
    };
    let Some(conversation_id) = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    conversation_has_guided_queue_events(state, conversation_id).unwrap_or(false)
}

// ==================== 队列管理函数 ====================

/// 调度运行态以内存为准：队列在内存里改，写盘由运行态自己按快照补上。
///
/// 调用方只管改内存、报一个会话 id，不拼 JSON、不碰表、不等磁盘。判据是
/// 「正在跑」或「还有排队」——两者都不成立才删行。只在进调度与队列变动时调用；
/// 执行过程（流式、工具、整理上下文）一律不碰。
fn sync_active_dispatch(state: &AppState, conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    let running = match lock_conversation_processing_claims(state) {
        Ok(claims) => claims.contains(conversation_id),
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天调度] 同步调度运行态失败 会话={conversation_id} 错误={err}"
            ));
            return;
        }
    };
    let snapshot = match lock_conversation_runtime_slots(state) {
        Ok(slots) => slots.get(conversation_id).map(|slot| {
            (
                slot.pending_queue.is_empty(),
                serde_json::to_string(&slot.pending_queue).ok(),
            )
        }),
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天调度] 同步调度运行态失败 会话={conversation_id} 错误={err}"
            ));
            return;
        }
    };
    match snapshot {
        Some((empty, Some(json))) if running || !empty => {
            schedule_active_dispatch_persist(state, conversation_id, Some(json))
        }
        _ => schedule_active_dispatch_persist(state, conversation_id, None),
    }
}

pub(crate) fn ingress_chat_event(
    state: &AppState,
    event: ChatPendingEvent,
) -> Result<ChatEventIngress, String> {
    if !event_is_goal_continue(&event) {
        clear_goal_continue_suppression(
            state,
            &event.conversation_id,
            "new_non_goal_event",
        )?;
    }
    // 原子区间：阻塞判定 +（可选）入队，在同一把流程锁内完成。
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut claims = lock_conversation_processing_claims(state)?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let running_count = claims.len();
    let blocked = {
        let slot = conversation_slot_mut(&mut slots, &event.conversation_id);
        let blocked = slot.state != MainSessionState::Idle
            || !slot.pending_queue.is_empty()
            || conversation_running_slot_count(&claims, &event.conversation_id) > 0
            || running_count >= CHAT_CONCURRENCY_LIMIT;
        slot.last_activity_at = now_iso();
        blocked
    };
    if blocked {
        let event_id = event.id.clone();
        let conversation_id = event.conversation_id.clone();
        {
            let slot = conversation_slot_mut(&mut slots, &conversation_id);
            slot.pending_queue.push_back(event);
        }
        drop(slots);
        drop(claims);
        drop(_dequeue_guard);
        sync_active_dispatch(state, &conversation_id);
        return Ok(ChatEventIngress::Queued { event_id });
    }
    let conversation_id = event.conversation_id.clone();
    claims.insert(conversation_id.clone());
    drop(slots);
    drop(claims);
    drop(_dequeue_guard);
    // 直接开跑：这一轮已经在跑，队列通常是空的，把这一刻的现场记一行。
    sync_active_dispatch(state, &conversation_id);
    Ok(ChatEventIngress::Direct(event))
}
