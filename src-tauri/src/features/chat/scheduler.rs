// ==================== 群聊消息队列与主助理串行调度系统 ====================
//
// 这是当前项目最核心、也最容易被误改的业务边界之一。
//
// 我们实现的不是传统“用户发一句 -> 助理立刻回一句”的线性聊天，
// 而是一个面向未来跨进程协作的“单主助理消息流”：
//
// 1. 队列是入口层
//    所有新消息，无论来自用户、任务、委托、系统，未来甚至来自其他进程，
//    都必须先入队，不能直接写进正式历史。
//
// 2. 历史是唯一生效层
//    一条消息只有在批量出队、写入 conversation.messages 之后，才算正式生效。
//    因此消息的业务时间应以“刷入历史的时间”为准，而不是入队时间。
//
// 3. 激活信号决定是否开启下一轮主助理
//    批次写入历史后，只有当本批次存在 activate_assistant=true 的事件，
//    才允许开启新的主助理轮次；否则只更新历史，不启动流式。
//
// 4. 主助理永远只有一个前台轮次
//    当主助理正在流式，或者正在整理上下文时，新的消息只能继续排队，
//    绝不能插入当前轮次，也不能抢占当前流式显示。
//
// 这套设计保证了：
// - 无论同一时刻涌入多少消息、来自多少个来源，都能稳定收敛；
// - 批量消息总是先统一进入历史，再决定是否开启下一轮；
// - 前后端都能围绕“队列 -> 历史 -> 激活新轮次”的固定节奏工作。

// ==================== 数据结构定义 ====================

/// 主会话状态机
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MainSessionState {
    /// 空闲，可以出队
    Idle,
    /// 主助理正在流式输出
    AssistantStreaming,
    /// 正在整理上下文
    OrganizingContext,
}

/// 消息来源类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ChatEventSource {
    /// 用户发言
    User,
    /// 任务触发
    Task,
    /// 委托回报
    Delegate,
    /// 系统事件
    System,
    /// 远程 IM 渠道消息
    #[serde(rename = "remote_im")]
    RemoteIm,
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatSessionInfo {
    pub department_id: String,
    pub agent_id: String,
}

/// 待处理事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatPendingEvent {
    /// 事件唯一ID
    pub id: String,
    /// 目标会话ID
    pub conversation_id: String,
    /// 入队时间，仅用于排队观测，不代表正式生效时间
    pub created_at: String,
    /// 来源类型
    pub source: ChatEventSource,
    /// 要写入的消息集合
    pub messages: Vec<ChatMessage>,
    /// 是否在本批消息写入历史后激活主助理
    pub activate_assistant: bool,
    /// 会话信息
    pub session_info: ChatSessionInfo,
    /// 远程消息来源（仅 source=RemoteIm 时使用）
    #[serde(default)]
    pub sender_info: Option<RemoteImMessageSource>,
}

#[derive(Clone)]
struct QueuedChatActivation {
    event_id: String,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
    source: QueuedChatActivationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuedChatActivationSource {
    PendingEvent,
    ActiveViewBinding,
}

pub(crate) enum ChatEventIngress {
    Direct(ChatPendingEvent),
    Queued { event_id: String },
}

// ==================== 队列查询和管理 ====================

/// 队列事件摘要（用于前端显示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatQueueEventSummary {
    pub id: String,
    pub source: ChatEventSource,
    pub created_at: String,
    pub message_preview: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatQueueSnapshotPush {
    queue_events: Vec<ChatQueueEventSummary>,
    session_state: MainSessionState,
}

const CHAT_QUEUE_SNAPSHOT_EVENT: &str = "easy-call:chat-queue-snapshot";
const CHAT_HISTORY_FLUSHED_EVENT: &str = "easy-call:history-flushed";
const CHAT_ROUND_COMPLETED_EVENT: &str = "easy-call:round-completed";
const CHAT_ROUND_FAILED_EVENT: &str = "easy-call:round-failed";
const CHAT_ASSISTANT_DELTA_EVENT: &str = "easy-call:assistant-delta";
const CHAT_CONCURRENCY_LIMIT: usize = 8;

fn lock_conversation_runtime_slots(
    state: &AppState,
) -> Result<
    std::sync::MutexGuard<'_, std::collections::HashMap<String, ConversationRuntimeSlot>>,
    String,
> {
    match state.conversation_runtime_slots.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            eprintln!(
                "[聊天调度] 警告: conversation_runtime_slots 锁已 poison，正在继续恢复使用"
            );
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
            eprintln!(
                "[聊天调度] 警告: conversation_processing_claims 锁已 poison，正在继续恢复使用"
            );
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

fn total_queue_len(state: &AppState) -> Result<usize, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots.values().map(|slot| slot.pending_queue.len()).sum())
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
                        MessagePart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                })
                .unwrap_or_default();
            let preview = if message_preview.chars().count() > 50 {
                format!("{}...", message_preview.chars().take(50).collect::<String>())
            } else {
                message_preview
            };
            summaries.push(ChatQueueEventSummary {
                id: event.id.clone(),
                source: event.source.clone(),
                created_at: event.created_at.clone(),
                message_preview: preview,
                conversation_id: event.conversation_id.clone(),
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
        let _ = app_handle.emit(CHAT_QUEUE_SNAPSHOT_EVENT, payload);
    }
}

/// 从队列中移除指定事件
pub(crate) fn remove_from_queue(state: &AppState, event_id: &str) -> Result<Option<ChatPendingEvent>, String> {
    // 队列修改统一走 dequeue_lock -> queue_lock，保证进出队原子顺序一致。
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let mut removed = None;
    let mut remaining_queue_len = 0usize;
    for slot in slots.values_mut() {
        if let Some(pos) = slot.pending_queue.iter().position(|e| e.id == event_id) {
            removed = slot.pending_queue.remove(pos);
            remaining_queue_len = slot.pending_queue.len();
            break;
        }
    }
    drop(slots);
    if removed.is_some() {
        eprintln!(
            "[聊天调度] 从队列移除事件: id={}, queue_len={}",
            event_id, remaining_queue_len
        );
        emit_chat_queue_snapshot(state);
        complete_pending_chat_events_with_error(
            state,
            &[event_id.to_string()],
            "消息已从队列移除",
        )?;
    }
    Ok(removed)
}

// ==================== 队列管理函数 ====================

pub(crate) fn ingress_chat_event(
    state: &AppState,
    event: ChatPendingEvent,
) -> Result<ChatEventIngress, String> {
    // 原子区间：阻塞判定 +（可选）入队，在同一把流程锁内完成。
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut claims = lock_conversation_processing_claims(state)?;
    let mut slots = lock_conversation_runtime_slots(state)?;
    let running_count = claims.len();
    let slot = conversation_slot_mut(&mut slots, &event.conversation_id);
    let blocked = slot.state != MainSessionState::Idle
        || !slot.pending_queue.is_empty()
        || conversation_running_slot_count(&claims, &event.conversation_id) > 0
        || running_count >= CHAT_CONCURRENCY_LIMIT;
    slot.last_activity_at = now_iso();
    if blocked {
        let event_id = event.id.clone();
        slot.pending_queue.push_back(event);
        return Ok(ChatEventIngress::Queued { event_id });
    }
    claims.insert(event.conversation_id.clone());
    Ok(ChatEventIngress::Direct(event))
}

pub(crate) fn register_chat_event_runtime(
    state: &AppState,
    event_id: &str,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
    sender: tokio::sync::oneshot::Sender<Result<SendChatResult, String>>,
) -> Result<(), String> {
    state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?
        .insert(event_id.to_string(), on_delta);
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
    let trimmed_conversation_id = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(conversation_id) = trimmed_conversation_id {
        bindings.insert(
            trimmed_window_label.to_string(),
            ActiveChatViewBinding {
                conversation_id,
                on_delta,
            },
        );
    } else {
        bindings.remove(trimmed_window_label);
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn clear_active_chat_view_stream_binding(
    state: &AppState,
    window_label: &str,
) -> Result<(), String> {
    let mut bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    bindings.remove(window_label.trim());
    Ok(())
}

pub(crate) fn trigger_chat_queue_processing(state: &AppState) {
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = process_chat_queue(&state_clone).await {
            eprintln!("[聊天调度] process_chat_queue 失败: {}", err);
        }
    });
}

pub(crate) fn is_chat_event_queued(state: &AppState, event_id: &str) -> Result<bool, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .values()
        .any(|slot| slot.pending_queue.iter().any(|item| item.id == event_id)))
}

pub(crate) async fn process_chat_queue_for_event(state: &AppState, event_id: &str) {
    if let Err(err) = process_chat_queue(state).await {
        eprintln!("[聊天调度] process_chat_queue 失败: {}", err);
    }
    if is_chat_event_queued(state, event_id).unwrap_or(false) {
        emit_chat_queue_snapshot(state);
    }
}

pub(crate) async fn process_chat_event_after_ingress(
    state: &AppState,
    ingress: ChatEventIngress,
) {
    match ingress {
        ChatEventIngress::Direct(event) => {
            let conversation_id = event.conversation_id.clone();
            if let Err(err) =
                process_claimed_conversation_batch(state, &conversation_id, vec![event]).await
            {
                eprintln!("[聊天调度] 处理直接事件失败: {}", err);
            }
        }
        ChatEventIngress::Queued { event_id } => {
            process_chat_queue_for_event(state, &event_id).await;
        }
    }
}

pub(crate) fn trigger_chat_event_after_ingress(state: &AppState, ingress: ChatEventIngress) {
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        process_chat_event_after_ingress(&state_clone, ingress).await;
    });
}

async fn process_claimed_conversation_batch(
    state: &AppState,
    conversation_id: &str,
    events: Vec<ChatPendingEvent>,
) -> Result<(), String> {
    let result = process_conversation_batch(state, conversation_id, events).await;
    if let Err(release_err) = release_conversation_processing_claim(state, conversation_id) {
        eprintln!(
            "[聊天调度] 释放会话处理声明失败: conversation_id={}, error={}",
            conversation_id, release_err
        );
    }
    emit_chat_queue_snapshot(state);
    trigger_chat_queue_processing(state);
    result
}

// ==================== 状态机管理函数 ====================

/// 获取当前状态
pub(crate) fn get_main_session_state(state: &AppState) -> Result<MainSessionState, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    if slots
        .values()
        .any(|slot| slot.state == MainSessionState::OrganizingContext)
    {
        return Ok(MainSessionState::OrganizingContext);
    }
    if slots
        .values()
        .any(|slot| slot.state == MainSessionState::AssistantStreaming)
    {
        return Ok(MainSessionState::AssistantStreaming);
    }
    Ok(MainSessionState::Idle)
}

/// 设置会话状态并记录日志
pub(crate) fn set_conversation_runtime_state(
    state: &AppState,
    conversation_id: &str,
    new_state: MainSessionState,
) -> Result<(), String> {
    let (old_state_cn, new_state_cn) = {
        let mut slots = lock_conversation_runtime_slots(state)?;
        let slot = conversation_slot_mut(&mut slots, conversation_id);
        let old_state = slot.state.clone();
        slot.state = new_state.clone();
        slot.last_activity_at = now_iso();

        let old_state_cn = match old_state {
            MainSessionState::Idle => "空闲",
            MainSessionState::AssistantStreaming => "助理流式输出",
            MainSessionState::OrganizingContext => "整理上下文",
        };
        let new_state_cn = match new_state {
            MainSessionState::Idle => "空闲",
            MainSessionState::AssistantStreaming => "助理流式输出",
            MainSessionState::OrganizingContext => "整理上下文",
        };
        (old_state_cn, new_state_cn)
    };

    eprintln!(
        "[聊天调度] 会话状态转换: conversation_id={}, {} -> {}",
        conversation_id, old_state_cn, new_state_cn
    );

    emit_chat_queue_snapshot(state);
    Ok(())
}

fn release_conversation_processing_claim(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mut claims = lock_conversation_processing_claims(state)?;
    claims.remove(conversation_id.trim());
    Ok(())
}

fn claim_queued_conversation_batches(
    state: &AppState,
) -> Result<Vec<(String, Vec<ChatPendingEvent>)>, String> {
    let _dequeue_guard = state
        .dequeue_lock
        .lock()
        .map_err(|_| "Failed to lock dequeue lock".to_string())?;
    let mut claims = lock_conversation_processing_claims(state)?;
    if claims.len() >= CHAT_CONCURRENCY_LIMIT {
        return Ok(Vec::new());
    }
    let available_slots = CHAT_CONCURRENCY_LIMIT.saturating_sub(claims.len());
    let mut slots = lock_conversation_runtime_slots(state)?;
    let mut eligible = slots
        .iter()
        .filter_map(|(conversation_id, slot)| {
            if slot.state != MainSessionState::Idle
                || slot.pending_queue.is_empty()
                || claims.contains(conversation_id)
            {
                return None;
            }
            let created_at = slot
                .pending_queue
                .front()
                .map(|event| event.created_at.clone())
                .unwrap_or_default();
            Some((conversation_id.clone(), created_at))
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));

    let mut claimed_batches = Vec::<(String, Vec<ChatPendingEvent>)>::new();
    for (conversation_id, _) in eligible.into_iter().take(available_slots) {
        let slot = conversation_slot_mut(&mut slots, &conversation_id);
        let batch = slot.pending_queue.drain(..).collect::<Vec<_>>();
        if batch.is_empty() {
            continue;
        }
        slot.last_activity_at = now_iso();
        claims.insert(conversation_id.clone());
        claimed_batches.push((conversation_id, batch));
    }
    Ok(claimed_batches)
}

// ==================== 出队调度器 ====================

/// 主出队处理函数
///
/// 语义上，它是在做“下一轮候选输入结算”：
/// 1. 把当前门口的所有消息先收进来；
/// 2. 按会话分别批处理；
/// 3. 每个会话先批量写正式历史；
/// 4. 再决定该会话是否需要开启新的主助理轮次。
pub(crate) async fn process_chat_queue(state: &AppState) -> Result<(), String> {
    let claimed_batches = claim_queued_conversation_batches(state)?;
    if claimed_batches.is_empty() {
        return Ok(());
    }
    emit_chat_queue_snapshot(state);
    for (conversation_id, events) in claimed_batches {
        let state_clone = state.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) =
                process_claimed_conversation_batch(&state_clone, &conversation_id, events).await
            {
                eprintln!(
                    "[聊天调度] 处理会话失败 {}: {}",
                    conversation_id, err
                );
            }
        });
    }
    Ok(())
}

/// 处理单个会话的批次
///
/// 这里严格遵守“先历史，后激活”的顺序：
/// 1. 不论是否需要激活主助理，先把整批消息写入正式历史；
/// 2. 写入时统一刷新 created_at，确保消息生效时间以入历史为准；
/// 3. 然后再判断 should_activate：
///    - false：只更新历史，不开启流式；
///    - true：先通知前端历史已落地，再开启新的主助理轮次。
async fn process_conversation_batch(
    state: &AppState,
    conversation_id: &str,
    events: Vec<ChatPendingEvent>,
) -> Result<(), String> {
    let event_ids = events.iter().map(|event| event.id.clone()).collect::<Vec<_>>();
    let latest_user_text = latest_user_text_from_events(&events);
    let history_flush_time = now_iso();
    let oldest_queue_created_at = events
        .iter()
        .map(|event| event.created_at.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("");
    let mut persisted_batch_messages = Vec::<ChatMessage>::new();
    let mut event_activate_flags = Vec::<bool>::with_capacity(events.len());

    fn session_enable_image(app_config: &AppConfig, session: &ChatSessionInfo) -> bool {
        let api_id = department_by_id(app_config, &session.department_id)
            .map(department_primary_api_config_id)
            .or_else(|| {
                department_for_agent_id(app_config, &session.agent_id).map(department_primary_api_config_id)
            });
        resolve_selected_api_config(app_config, api_id.as_deref())
            .map(|api| api.enable_image)
            .unwrap_or(true)
    }

    fn resolve_image_text_from_cache_any(
        image_text_cache: &[ImageTextCacheEntry],
        mime: &str,
        bytes_base64: &str,
    ) -> Option<String> {
        let part = BinaryPart {
            mime: mime.to_string(),
            bytes_base64: bytes_base64.to_string(),
            saved_path: None,
        };
        let hash = compute_image_hash_hex(&part).ok()?;
        image_text_cache
            .iter()
            .rev()
            .find(|item| item.hash == hash && !item.text.trim().is_empty())
            .map(|item| item.text.trim().to_string())
    }

    fn normalize_user_message_for_image_support(
        message: &mut ChatMessage,
        enable_image: bool,
        image_text_cache: &[ImageTextCacheEntry],
    ) {
        if enable_image || message.role.trim() != "user" {
            return;
        }

        let mut next_parts = Vec::<MessagePart>::with_capacity(message.parts.len());
        let mut removed_image_index = 0usize;
        let mut appended_text_blocks = Vec::<String>::new();

        for part in message.parts.drain(..) {
            match part {
                MessagePart::Image { mime, bytes_base64, .. } => {
                    removed_image_index += 1;
                    if let Some(converted) =
                        resolve_image_text_from_cache_any(image_text_cache, &mime, &bytes_base64)
                    {
                        appended_text_blocks.push(format!(
                            "[图片{}]\n{}",
                            removed_image_index, converted
                        ));
                    } else {
                        appended_text_blocks.push(
                            "这里有一张图片，但当前模型不支持图片输入，所以已忽略。".to_string(),
                        );
                    }
                }
                other => next_parts.push(other),
            }
        }

        for text in appended_text_blocks {
            next_parts.push(MessagePart::Text { text });
        }
        message.parts = next_parts;
    }

    // 1. 先写入所有消息到会话记录。
    //
    // 这里统一覆盖 created_at 为 history_flush_time，
    // 目的是把“正式进入历史的时间”作为消息的业务生效时间。
    // 入队时间只用于队列观察，不用于正式会话排序和轮次判断。
    {
        let guard = state.state_lock.lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;

        let mut data = state_read_app_data_cached(state)?;
        let app_config = state_read_config_cached(state)?;
        let image_text_cache = data.image_text_cache.clone();
        let mut session_image_capability =
            std::collections::HashMap::<String, bool>::new();
        if let Some(conversation_idx) = data
            .conversations
            .iter()
            .position(|c| c.id == conversation_id && c.summary.trim().is_empty())
        {
            for event in &events {
                let event_should_activate = if matches!(event.source, ChatEventSource::RemoteIm) {
                    should_activate_remote_im_event(event, &mut data, &history_flush_time)
                } else {
                    event.activate_assistant
                };
                event_activate_flags.push(event_should_activate);
                let enable_image = *session_image_capability
                    .entry(event.session_info.department_id.clone())
                    .or_insert_with(|| session_enable_image(&app_config, &event.session_info));
                let conversation = &mut data.conversations[conversation_idx];
                for message in &event.messages {
                    let mut persisted = message.clone();
                    normalize_user_message_for_image_support(
                        &mut persisted,
                        enable_image,
                        &image_text_cache,
                    );
                    persisted.created_at = history_flush_time.clone();
                    persisted_batch_messages.push(persisted.clone());
                    conversation.messages.push(persisted.clone());
                    match persisted.role.trim() {
                        "user" => conversation.last_user_at = Some(history_flush_time.clone()),
                        "assistant" => {
                            conversation.last_assistant_at = Some(history_flush_time.clone())
                        }
                        _ => {}
                    }
                }
            }
            data.conversations[conversation_idx].updated_at = history_flush_time.clone();
            state_write_app_data_cached(state, &data)?;
        } else {
            drop(guard);
            complete_pending_chat_events_with_error(
                state,
                &event_ids,
                &format!("目标会话不存在，conversationId={conversation_id}"),
            )?;
            return Err(format!("目标会话不存在，conversationId={conversation_id}"));
        }
        drop(guard);
    }

    // 2. 判断是否需要激活主助理。
    // 这一步故意放在“写历史之后”，避免出现前端先开流式、
    // 但本批消息还没正式落入历史的时序错乱。
    let should_activate = event_activate_flags.into_iter().any(|v| v);
    let activate_status = if should_activate { "开始" } else { "跳过" };

    let batch_message_count = events.iter().map(|e| e.messages.len()).sum::<usize>();
    eprintln!(
        "[聊天调度] 批次写入完成: conversation_id={}, message_count={}, activate={}, oldest_queue_created_at={}",
        conversation_id,
        batch_message_count,
        activate_status,
        oldest_queue_created_at,
    );
    let mut activations = take_queued_chat_activations(state, &event_ids)?;
    if !activations.is_empty() {
        eprintln!(
            "[聊天调度] 使用请求绑定通道: conversation_id={}, binding_count={}",
            conversation_id,
            activations.len()
        );
    }
    if activations.is_empty() {
        activations = collect_active_chat_view_activations(state, conversation_id)?;
        if !activations.is_empty() {
            eprintln!(
                "[聊天调度] 使用当前聊天窗口绑定通道: conversation_id={}, binding_count={}",
                conversation_id,
                activations.len()
            );
        }
    }
    let history_flushed_payload = serde_json::json!({
        "conversationId": conversation_id,
        "messageCount": batch_message_count,
        "messages": persisted_batch_messages,
        "activateAssistant": should_activate,
    });
    emit_history_flushed_event(state, &history_flushed_payload, conversation_id, &event_ids);

    // 3. 如果需要激活，调用主助理。
    if should_activate {
        // 取第一个事件的会话信息
        if let Some(first_event) = events.first() {
            // 同一批里可能有多个激活请求，但前台主助理轮次只能有一个。
            // 因此这里只保留最后一个激活请求作为实际流式绑定对象。
            let activation = activations.pop();
            if let Some(active) = activation.as_ref() {
                eprintln!(
                    "[聊天调度] 批次流式绑定: conversation_id={}, event_id={}, oldest_queue_created_at={}",
                    conversation_id, active.event_id, oldest_queue_created_at
                );
            }
            match activate_main_assistant(
                state,
                &first_event.session_info,
                conversation_id,
                activation.clone(),
                oldest_queue_created_at,
            ).await {
                Ok(result) => {
                    if let Some(active) = activation.as_ref() {
                        send_round_completed_event(state, conversation_id, active, &result);
                    }
                    complete_pending_chat_events_with_result(state, &event_ids, result)?;
                }
                Err(err) => {
                    if let Some(active) = activation.as_ref() {
                        send_round_failed_event(state, conversation_id, active, &err);
                    }
                    complete_pending_chat_events_with_error(state, &event_ids, &err)?;
                    return Err(err);
                }
            }
        }
    } else {
        // 不激活时，本批消息依然已经是正式历史的一部分。
        // 这里只回传一个“已落地但未开启新轮次”的结果，前端应刷新历史，
        // 但不应启动新的主助理流式显示。
        complete_pending_chat_events_with_result(
            state,
            &event_ids,
            SendChatResult {
                conversation_id: conversation_id.to_string(),
                latest_user_text,
                assistant_text: String::new(),
                reasoning_standard: String::new(),
                reasoning_inline: String::new(),
                archived_before_send: false,
                assistant_message: None,
                provider_prompt_tokens: None,
                estimated_prompt_tokens: None,
                effective_prompt_tokens: None,
                effective_prompt_source: None,
                context_window_tokens: None,
                max_output_tokens: None,
                context_usage_percent: None,
            },
        )?;
    }

    Ok(())
}

/// 激活主助理
///
/// 注意：这里只负责启动“下一轮主助理”，不负责把新消息写进历史。
/// 新消息进入历史的动作已经在 process_conversation_batch 中完成。
async fn activate_main_assistant(
    state: &AppState,
    session_info: &ChatSessionInfo,
    conversation_id: &str,
    activation: Option<QueuedChatActivation>,
    oldest_queue_created_at: &str,
) -> Result<SendChatResult, String> {
    eprintln!(
        "[聊天调度] 开始: 激活主助理, conversation_id={}, department_id={}, agent_id={}, oldest_queue_created_at={}",
        conversation_id,
        session_info.department_id,
        session_info.agent_id,
        oldest_queue_created_at,
    );

    let trace_id = activation
        .as_ref()
        .map(|item| format!("queue-{}", item.event_id))
        .unwrap_or_else(|| format!("queue-{}", Uuid::new_v4()));

    // 设置状态为 AssistantStreaming
    set_conversation_runtime_state(
        state,
        conversation_id,
        MainSessionState::AssistantStreaming,
    )?;

    // 构造 trigger_only 请求
    let request = SendChatRequest {
        trigger_only: true,  // 不写入新消息，只触发助理回复
        session: Some(SessionSelector {
            api_config_id: None,
            department_id: Some(session_info.department_id.clone()),
            agent_id: session_info.agent_id.clone(),
            conversation_id: Some(conversation_id.to_string()),
        }),
        payload: ChatInputPayload {
            text: None,
            display_text: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            provider_meta: None,
        },
        speaker_agent_id: None,
        trace_id: Some(trace_id),
        oldest_queue_created_at: Some(oldest_queue_created_at.to_string()),
    };

    // 使用 emit 作为远程激活轮次的流式主通道，避免前端窗口重绑定造成 channel 失联。
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let conversation_id_for_emit = conversation_id.to_string();
    let active_channel: tauri::ipc::Channel<AssistantDeltaEvent> =
        tauri::ipc::Channel::new(move |body| {
            let parsed_event = match body {
                tauri::ipc::InvokeResponseBody::Json(json) => {
                    serde_json::from_str::<AssistantDeltaEvent>(&json).ok()
                }
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    serde_json::from_slice::<AssistantDeltaEvent>(&bytes).ok()
                }
            };
            if let (Some(app), Some(event)) = (app_handle.as_ref(), parsed_event) {
                let payload = serde_json::json!({
                    "conversationId": conversation_id_for_emit.clone(),
                    "event": event,
                });
                let _ = app.emit(CHAT_ASSISTANT_DELTA_EVENT, payload);
            }
            Ok(())
        });

    // 调用 send_chat_message_inner
    let result = send_chat_message_inner(request, state, &active_channel).await;

    // 回复完成，切换回 Idle
    set_conversation_runtime_state(state, conversation_id, MainSessionState::Idle)?;

    // 不在这里递归调用 process_chat_queue，避免 Send 问题
    // 改为在外部调用

    result
}

fn remote_im_event_message_text(event: &ChatPendingEvent) -> String {
    event
        .messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.trim()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_contains_keyword(text: &str, keyword: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&keyword.to_ascii_lowercase())
}

struct RemoteImActivationContext<'a> {
    message_text: &'a str,
    now_ts: i64,
}

struct RemoteImActivationDecision {
    activate: bool,
    reason: String,
}

trait RemoteImActivationStrategy {
    fn decide(
        &self,
        contact: &RemoteImContact,
        ctx: &RemoteImActivationContext<'_>,
    ) -> RemoteImActivationDecision;
}

struct DefaultRemoteImActivationStrategy;

impl RemoteImActivationStrategy for DefaultRemoteImActivationStrategy {
    fn decide(
        &self,
        contact: &RemoteImContact,
        ctx: &RemoteImActivationContext<'_>,
    ) -> RemoteImActivationDecision {
        let mode = contact.activation_mode.trim().to_ascii_lowercase();
        if mode != "always" && mode != "keyword" {
            return RemoteImActivationDecision {
                activate: false,
                reason: format!("activationMode={}（不激活）", mode),
            };
        }

        if mode == "keyword" {
            let matched = contact
                .activation_keywords
                .iter()
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .any(|keyword| text_contains_keyword(ctx.message_text, keyword));
            if !matched {
                return RemoteImActivationDecision {
                    activate: false,
                    reason: "keyword 未命中，跳过激活".to_string(),
                };
            }
        }

        if contact.activation_cooldown_seconds > 0 {
            if let Some(last) = contact
                .last_activated_at
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            {
                let elapsed = ctx.now_ts.saturating_sub(last.timestamp()) as u64;
                if elapsed < contact.activation_cooldown_seconds {
                    let remaining = contact.activation_cooldown_seconds.saturating_sub(elapsed);
                    return RemoteImActivationDecision {
                        activate: false,
                        reason: format!("冷却中，remaining={}s", remaining),
                    };
                }
            }
        }

        RemoteImActivationDecision {
            activate: true,
            reason: "满足激活条件，触发主助理".to_string(),
        }
    }
}

fn should_activate_remote_im_event(
    event: &ChatPendingEvent,
    data: &mut AppData,
    now: &str,
) -> bool {
    let Some(sender) = event.sender_info.as_ref() else {
        return false;
    };
    let Some(contact) = data
        .remote_im_contacts
        .iter_mut()
        .find(|item| {
            item.channel_id == sender.channel_id
                && item.remote_contact_type == sender.remote_contact_type
                && item.remote_contact_id == sender.remote_contact_id
        })
    else {
        return false;
    };

    let now_ts = match chrono::DateTime::parse_from_rfc3339(now) {
        Ok(dt) => dt.timestamp(),
        Err(err) => {
            eprintln!(
                "[远程IM][激活判定] 解析当前时间失败，使用系统当前时间兜底: now={}, error={}",
                now, err
            );
            chrono::Utc::now().timestamp()
        }
    };
    let message_text = remote_im_event_message_text(event);
    let strategy = DefaultRemoteImActivationStrategy;
    let decision = strategy.decide(
        contact,
        &RemoteImActivationContext {
            message_text: &message_text,
            now_ts,
        },
    );
    if decision.activate {
        contact.last_activated_at = Some(now.to_string());
    }
    log_remote_im_activation_decision(
        &sender.channel_id,
        &contact.remote_contact_id,
        if decision.activate { "activate" } else { "skip" },
        &decision.reason,
    );
    decision.activate
}

fn log_remote_im_activation_decision(
    channel_id: &str,
    remote_contact_id: &str,
    action: &str,
    reason: &str,
) {
    eprintln!(
        "[远程IM][激活判定] channel_id={}, contact_id={}, action={}, reason={}",
        channel_id, remote_contact_id, action, reason
    );
    let channel_id_owned = channel_id.to_string();
    let message = format!(
        "[激活判定] contactId={}, action={}, reason={}",
        remote_contact_id, action, reason
    );
    let manager = napcat_ws_manager();
    tauri::async_runtime::spawn(async move {
        manager
            .add_log(&channel_id_owned, "info", &message)
            .await;
    });
}

fn latest_user_text_from_events(events: &[ChatPendingEvent]) -> String {
    events
        .iter()
        .flat_map(|event| event.messages.iter())
        .rev()
        .find_map(|message| {
            if message.role.trim() != "user" {
                return None;
            }
            message.parts.iter().find_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

fn emit_history_flushed_event(
    state: &AppState,
    payload: &serde_json::Value,
    conversation_id: &str,
    event_ids: &[String],
) {
    eprintln!(
        "[聊天推送] 准备发送 history_flushed: conversation_id={}, event_ids={:?}",
        conversation_id, event_ids
    );
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        eprintln!(
            "[聊天调度] history_flushed emit 失败: app_handle unavailable, conversation_id={}, event_ids={:?}",
            conversation_id, event_ids
        );
        return;
    };
    match app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        Ok(_) => {
            eprintln!(
                "[聊天调度] history_flushed 已通过 emit 发送: conversation_id={}, event_ids={:?}",
                conversation_id, event_ids
            );
        }
        Err(err) => {
            eprintln!(
                "[聊天调度] history_flushed emit 失败: conversation_id={}, event_ids={:?}, error={}",
                conversation_id, event_ids, err
            );
        }
    }
}

fn send_round_completed_event(
    state: &AppState,
    conversation_id: &str,
    activation: &QueuedChatActivation,
    result: &SendChatResult,
) {
    if activation.source != QueuedChatActivationSource::ActiveViewBinding {
        eprintln!(
            "[聊天推送] 跳过 round_completed 通道发送: conversation_id={}, reason=activation_source_not_active_view",
            conversation_id
        );
        emit_round_completed_event(state, conversation_id, result);
        return;
    }
    eprintln!(
        "[聊天推送] 通过绑定通道发送 round_completed: conversation_id={}, event_id={}",
        conversation_id, activation.event_id
    );
    let payload_json = serde_json::to_string(result).unwrap_or_else(|_| {
        serde_json::json!({
            "conversationId": result.conversation_id,
            "assistantText": result.assistant_text,
            "reasoningStandard": result.reasoning_standard,
            "reasoningInline": result.reasoning_inline,
            "archivedBeforeSend": result.archived_before_send,
            "assistantMessage": result.assistant_message,
        })
        .to_string()
    });
    let _ = activation.on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("round_completed".to_string()),
        tool_name: None,
        tool_status: None,
        tool_args: None,
        message: Some(payload_json),
    });
    emit_round_completed_event(state, conversation_id, result);
}

fn send_round_failed_event(
    state: &AppState,
    conversation_id: &str,
    activation: &QueuedChatActivation,
    error_text: &str,
) {
    if activation.source != QueuedChatActivationSource::ActiveViewBinding {
        eprintln!(
            "[聊天推送] 跳过 round_failed 通道发送: conversation_id={}, reason=activation_source_not_active_view",
            conversation_id
        );
        emit_round_failed_event(state, conversation_id, error_text);
        return;
    }
    eprintln!(
        "[聊天推送] 通过绑定通道发送 round_failed: conversation_id={}, event_id={}",
        conversation_id, activation.event_id
    );
    let payload_json = serde_json::json!({
        "error": error_text,
    })
    .to_string();
    let _ = activation.on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("round_failed".to_string()),
        tool_name: None,
        tool_status: None,
        tool_args: None,
        message: Some(payload_json),
    });
    emit_round_failed_event(state, conversation_id, error_text);
}

fn emit_round_completed_event(
    state: &AppState,
    conversation_id: &str,
    result: &SendChatResult,
) {
    eprintln!(
        "[聊天推送] 准备 emit round_completed: conversation_id={}, has_assistant_message={}",
        conversation_id,
        result.assistant_message.is_some()
    );
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        eprintln!(
            "[聊天推送] emit round_completed 失败: app_handle unavailable, conversation_id={}",
            conversation_id
        );
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "assistantText": result.assistant_text,
        "reasoningStandard": result.reasoning_standard,
        "reasoningInline": result.reasoning_inline,
        "archivedBeforeSend": result.archived_before_send,
        "assistantMessage": result.assistant_message,
    });
    match app_handle.emit(CHAT_ROUND_COMPLETED_EVENT, payload) {
        Ok(_) => eprintln!(
            "[聊天推送] emit round_completed 成功: conversation_id={}",
            conversation_id
        ),
        Err(err) => eprintln!(
            "[聊天推送] emit round_completed 失败: conversation_id={}, error={}",
            conversation_id, err
        ),
    }
}

fn emit_round_failed_event(
    state: &AppState,
    conversation_id: &str,
    error_text: &str,
) {
    eprintln!(
        "[聊天推送] 准备 emit round_failed: conversation_id={}, error_len={}",
        conversation_id,
        error_text.len()
    );
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        eprintln!(
            "[聊天推送] emit round_failed 失败: app_handle unavailable, conversation_id={}",
            conversation_id
        );
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "error": error_text,
    });
    match app_handle.emit(CHAT_ROUND_FAILED_EVENT, payload) {
        Ok(_) => eprintln!(
            "[聊天推送] emit round_failed 成功: conversation_id={}",
            conversation_id
        ),
        Err(err) => eprintln!(
            "[聊天推送] emit round_failed 失败: conversation_id={}, error={}",
            conversation_id, err
        ),
    }
}

fn collect_active_chat_view_activations(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<QueuedChatActivation>, String> {
    let bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    let conversation_id = conversation_id.trim();
    let binding_snapshot = bindings
        .iter()
        .map(|(window_label, binding)| format!("{}=>{}", window_label, binding.conversation_id))
        .collect::<Vec<_>>();
    eprintln!(
        "[聊天调度] 绑定快照: conversation_id={}, bindings={:?}",
        conversation_id, binding_snapshot
    );
    let exact = bindings
        .iter()
        .filter_map(|(window_label, binding)| {
            if binding.conversation_id != conversation_id {
                return None;
            }
            Some(QueuedChatActivation {
                event_id: format!("active-view:{window_label}"),
                on_delta: binding.on_delta.clone(),
                source: QueuedChatActivationSource::ActiveViewBinding,
            })
        })
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        eprintln!(
            "[聊天调度] 绑定筛选命中(exact): conversation_id={}, hit={}",
            conversation_id,
            exact.len()
        );
        return Ok(exact);
    }

    let wildcard = bindings
        .iter()
        .filter_map(|(window_label, binding)| {
            if binding.conversation_id != "*" {
                return None;
            }
            Some(QueuedChatActivation {
                event_id: format!("active-view:{window_label}"),
                on_delta: binding.on_delta.clone(),
                source: QueuedChatActivationSource::ActiveViewBinding,
            })
        })
        .collect::<Vec<_>>();
    if !wildcard.is_empty() {
        eprintln!(
            "[聊天调度] 使用通配前端绑定: conversation_id={}, binding_count={}",
            conversation_id,
            wildcard.len()
        );
        return Ok(wildcard);
    }

    eprintln!(
        "[聊天调度] 绑定筛选未命中: conversation_id={}, bindings_count={}",
        conversation_id,
        bindings.len()
    );
    Ok(Vec::new())
}

fn take_queued_chat_activations(
    state: &AppState,
    event_ids: &[String],
) -> Result<Vec<QueuedChatActivation>, String> {
    let mut channels = state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?;
    let mut activations = Vec::<QueuedChatActivation>::new();
    for event_id in event_ids {
        if let Some(on_delta) = channels.remove(event_id) {
            activations.push(QueuedChatActivation {
                event_id: event_id.clone(),
                on_delta,
                source: QueuedChatActivationSource::PendingEvent,
            });
        }
    }
    Ok(activations)
}

fn complete_pending_chat_events_with_result(
    state: &AppState,
    event_ids: &[String],
    result: SendChatResult,
) -> Result<(), String> {
    let mut channels = state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?;
    let mut senders = state
        .pending_chat_result_senders
        .lock()
        .map_err(|_| "Failed to lock pending chat result senders".to_string())?;
    for event_id in event_ids {
        channels.remove(event_id);
        if let Some(sender) = senders.remove(event_id) {
            let _ = sender.send(Ok(result.clone()));
        }
    }
    Ok(())
}

fn complete_pending_chat_events_with_error(
    state: &AppState,
    event_ids: &[String],
    error: &str,
) -> Result<(), String> {
    let mut channels = state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?;
    let mut senders = state
        .pending_chat_result_senders
        .lock()
        .map_err(|_| "Failed to lock pending chat result senders".to_string())?;
    for event_id in event_ids {
        channels.remove(event_id);
        if let Some(sender) = senders.remove(event_id) {
            let _ = sender.send(Err(error.to_string()));
        }
    }
    Ok(())
}
