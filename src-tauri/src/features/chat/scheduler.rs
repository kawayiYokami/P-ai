// ==================== 群聊消息调度与主助理串行系统 ====================
//
// 这是当前项目最核心、也最容易被误改的业务边界之一。
//
// 这里必须先分清两个概念：
//
// 1. 队列（queue）
//    语义是：等当前这一轮 LLM 调度完整结束后，再把下一批消息送进去。
//    它不会改变当前轮次的收口时机，只会决定“下一轮什么时候开始”。
//
// 2. 引导（guided）
//    语义是：当前这一轮只要完成一次工具执行，就允许立刻截断当前调度，
//    把引导消息插进去，然后重新发起一轮新的调度。
//    它不是普通队列换个标签，而是会改变“当前轮次什么时候被截断”。
//
// 因此，这里实现的不是传统“用户发一句 -> 助理立刻回一句”的线性聊天，
// 而是一个面向未来跨进程协作的“单主助理消息流”：
//
// 1. 未持久化事件先进入调度层
//    用户、任务、委托、系统事件和远程 IM 私聊都由调度器在领取后写入历史。
//    远程 IM 群聊是明确例外：它在入站阶段直接写入正式历史，再交给巡检。
//
// 2. 正式历史是唯一生效层
//    未持久化事件只有在批量写入消息仓库后才算正式生效；远程 IM 消息在入站
//    直接追加成功时已经生效，其 created_at 始终保留接收时间。
//
// 3. 主助理永远只有一个前台轮次
//    当主助理正在流式，或者正在整理上下文时，普通队列消息不能插入当前轮次。
//    只有引导才允许在“工具执行完成”这个切点提前截断并重启调度。
//
// 4. 调度器负责两种不同切点
//    - 队列：本轮完美结束 -> 下一轮开始
//    - 引导：一次工具执行完成 -> 立刻截断 -> 插入消息 -> 重启调度
//
// 这套设计保证了：
// - 无论同一时刻涌入多少消息、来自多少个来源，都能稳定收敛；
// - 正式历史和运行态切点分离，不把“已持久化”误当成“已完成调度”；
// - 前后端都围绕同一套“何时落历史、何时截断、何时重启”的语义工作。

// ==================== 数据结构定义 ====================

/// 主会话状态机
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChatQueueMode {
    Normal,
    Guided,
}

fn default_chat_queue_mode() -> ChatQueueMode {
    ChatQueueMode::Normal
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatSessionInfo {
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
    /// 队列模式
    #[serde(default = "default_chat_queue_mode")]
    pub queue_mode: ChatQueueMode,
    /// 要写入的消息集合
    pub messages: Vec<ChatMessage>,
    /// 是否在本批消息写入历史后激活主助理
    pub activate_assistant: bool,
    /// 若本批消息会激活主助理，则预先分配真实 assistant message id
    #[serde(default)]
    pub assistant_message_id: Option<String>,
    /// 会话信息
    pub session_info: ChatSessionInfo,
    /// 运行上下文（渐进接入）
    #[serde(default)]
    pub runtime_context: Option<RuntimeContext>,
    /// 远程消息来源（仅 source=RemoteIm 时使用）
    #[serde(default)]
    pub sender_info: Option<RemoteImMessageSource>,
}

#[derive(Clone)]
struct QueuedChatActivation {
    event_id: String,
    delta_channel: Option<tauri::ipc::Channel<AssistantDeltaEvent>>,
}

#[derive(Debug, Clone)]
struct ActivatedAssistantResult {
    result: SendChatResult,
    activation_id: String,
    request_id: String,
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
    pub queue_mode: ChatQueueMode,
    pub created_at: String,
    pub message_preview: String,
    pub message_text: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatQueueRecallResult {
    pub removed: bool,
    pub message_text: String,
    pub not_in_queue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatQueueMarkGuidedResult {
    pub updated: bool,
    pub not_in_queue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatQueueSnapshotPush {
    queue_events: Vec<ChatQueueEventSummary>,
    session_state: MainSessionState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationRuntimeSnapshot {
    pub conversation_id: String,
    pub runtime_state: MainSessionState,
    pub is_processing: bool,
    pub has_pending_queue: bool,
    pub pending_queue_count: usize,
    pub stream_cache: ConversationStreamRuntimeCacheSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationStreamRuntimeCacheSnapshot {
    pub activation_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub assistant_text: String,
    pub tool_status_text: String,
    pub tool_status_state: String,
    pub stream_blocks: Vec<AssistantStreamBlock>,
    pub started_at: String,
    pub started_at_ms: u64,
    pub updated_at: String,
    pub has_visible_progress: bool,
    pub persisted_assistant_message_id: String,
    pub context_usage_ratio: f64,
    pub context_usage_percent: u32,
    pub effective_prompt_tokens: u64,
    pub context_window_tokens: u32,
    #[serde(default)]
    pub scheduling_state: String,
}

const CHAT_QUEUE_SNAPSHOT_EVENT: &str = "easy-call:chat-queue-snapshot";
const CHAT_HISTORY_FLUSHED_EVENT: &str = "easy-call:history-flushed";
const CHAT_ROUND_STARTED_EVENT: &str = "easy-call:round-started";
const CHAT_ROUND_COMPLETED_EVENT: &str = "easy-call:round-completed";
const CHAT_ROUND_FAILED_EVENT: &str = "easy-call:round-failed";
const CHAT_ASSISTANT_DELTA_EVENT: &str = "easy-call:assistant-delta";
const CHAT_STREAM_REBIND_REQUIRED_EVENT: &str = "easy-call:stream-rebind-required";
const CHAT_REWIND_COMPLETED_EVENT: &str = "easy-call:chat-rewind-completed";
const CHAT_CONVERSATION_MESSAGE_APPENDED_EVENT: &str = "easy-call:conversation-message-appended";
const CHAT_CONVERSATION_OVERVIEW_UPDATED_EVENT: &str = "easy-call:conversation-overview-updated";
const CHAT_CONCURRENCY_LIMIT: usize = 8;
const GOAL_CONTINUE_DISPLAY_TEXT: &str = "继续完成目标";

include!("scheduler/queue_management.rs");
include!("scheduler/stream_runtime.rs");
pub(crate) fn trigger_chat_queue_processing(state: &AppState) {
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = process_chat_queue(&state_clone).await {
            runtime_log_error(format!("[聊天调度] process_chat_queue 失败: {}", err));
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
        runtime_log_error(format!("[聊天调度] process_chat_queue 失败: {}", err));
    }
    if is_chat_event_queued(state, event_id).unwrap_or(false) {
        emit_chat_queue_snapshot(state);
    }
}

pub(crate) async fn process_chat_event_after_ingress(state: &AppState, ingress: ChatEventIngress) {
    match ingress {
        ChatEventIngress::Direct(event) => {
            let conversation_id = event.conversation_id.clone();
            if let Err(err) =
                process_claimed_conversation_batch(state, &conversation_id, vec![event]).await
            {
                runtime_log_error(format!("[聊天调度] 处理直接事件失败: {}", err));
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

pub(crate) fn trigger_chat_event_after_ingress_with_delay(
    state: &AppState,
    ingress: ChatEventIngress,
    delay: std::time::Duration,
) {
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        process_chat_event_after_ingress(&state_clone, ingress).await;
    });
}

include!("scheduler/continuation_processing.rs");
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

pub(crate) fn get_conversation_runtime_state(
    state: &AppState,
    conversation_id: &str,
) -> Result<MainSessionState, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(conversation_id)
        .map(|slot| slot.state.clone())
        .unwrap_or(MainSessionState::Idle))
}

pub(crate) fn read_conversation_runtime_snapshot(
    state: &AppState,
    conversation_id: &str,
) -> Result<ConversationRuntimeSnapshot, String> {
    let cid = conversation_id.trim();
    let claims = lock_conversation_processing_claims(state)?;
    let is_processing = claims.contains(cid);
    drop(claims);
    let slots = lock_conversation_runtime_slots(state)?;
    let (runtime_state, pending_queue_count, stream_cache) = slots
        .get(cid)
        .map(|slot| {
            (
                slot.state.clone(),
                slot.pending_queue.len(),
                slot.stream_cache.clone(),
            )
        })
        .unwrap_or_else(|| {
            (
                MainSessionState::Idle,
                0,
                ConversationStreamRuntimeCache::default(),
            )
        });
    let stream_cache = conversation_stream_runtime_cache_snapshot(stream_cache);
    Ok(ConversationRuntimeSnapshot {
        conversation_id: cid.to_string(),
        runtime_state,
        is_processing,
        has_pending_queue: pending_queue_count > 0,
        pending_queue_count,
        stream_cache,
    })
}

/// 设置会话状态并记录日志
fn set_conversation_runtime_state(
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

    runtime_log_info(format!(
        "[聊天调度] 会话状态转换: conversation_id={}, {} -> {}",
        conversation_id, old_state_cn, new_state_cn
    ));

    emit_chat_queue_snapshot(state);
    Ok(())
}

pub(crate) fn set_conversation_runtime_state_and_emit(
    state: &AppState,
    conversation_id: &str,
    new_state: MainSessionState,
) -> Result<(), String> {
    set_conversation_runtime_state(state, conversation_id, new_state.clone())?;
    emit_conversation_runtime_state_updated_payload(
        state,
        &ConversationRuntimeStateUpdatedPayload {
            conversation_id: conversation_id.trim().to_string(),
            runtime_state: new_state,
        },
    );
    Ok(())
}

include!("scheduler/remote_im_processing.rs");
pub(crate) fn set_conversation_plan_mode_enabled(
    state: &AppState,
    conversation_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let normalized_conversation_id = conversation_id.trim();
    let mut slots = lock_conversation_runtime_slots(state)?;
    let slot = conversation_slot_mut(&mut slots, normalized_conversation_id);
    slot.plan_mode_enabled = enabled;
    slot.last_activity_at = now_iso();
    Ok(())
}

pub(crate) fn get_conversation_plan_mode_enabled(
    state: &AppState,
    conversation_id: &str,
) -> Result<bool, String> {
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(conversation_id.trim())
        .map(|slot| slot.plan_mode_enabled)
        .unwrap_or(false))
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
            let has_guided = slot
                .pending_queue
                .iter()
                .any(|event| event.queue_mode == ChatQueueMode::Guided);
            if slot.state != MainSessionState::Idle
                || slot.pending_queue.is_empty()
                || has_guided
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
        let Some(event) = slot.pending_queue.pop_front() else {
            continue;
        };
        slot.last_activity_at = now_iso();
        claims.insert(conversation_id.clone());
        claimed_batches.push((conversation_id, vec![event]));
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
                runtime_log_error(format!("[聊天调度] 处理会话失败 {}: {}", conversation_id, err));
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
    let event_ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let latest_user_text = latest_user_text_from_events(&events);
    let history_flush_time = now_iso();
    let oldest_queue_created_at = events
        .iter()
        .map(|event| event.created_at.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("");
    fn defer_image_parts_for_history_flushed(
        message: &ChatMessage,
        data_path: &PathBuf,
    ) -> (ChatMessage, usize, usize) {
        let mut deferred_message = message.clone();
        let mut deferred_image_count = 0usize;
        let mut deferred_base64_chars = 0usize;
        let mut next_parts = Vec::<MessagePart>::with_capacity(deferred_message.parts.len());

        for part in deferred_message.parts.drain(..) {
            match part {
                MessagePart::Image {
                    mime,
                    bytes_base64,
                    name,
                    ..
                } => {
                    let (attachment, warning) = legacy_binary_message_part_to_attachment(
                        data_path,
                        &mime,
                        &bytes_base64,
                        name.as_deref(),
                    );
                    if let Some(warning) = warning {
                        runtime_log_warn(format!("[附件迁移] 历史 flush 降级继续：{warning}"));
                    }
                    deferred_image_count += 1;
                    deferred_base64_chars += bytes_base64.len();
                    next_parts.push(attachment);
                }
                other => next_parts.push(other),
            }
        }

        if deferred_image_count > 0 {
            let mut provider_meta = deferred_message
                .provider_meta
                .take()
                .unwrap_or_else(|| serde_json::json!({}));
            if !provider_meta.is_object() {
                provider_meta = serde_json::json!({});
            }
            if let Some(obj) = provider_meta.as_object_mut() {
                obj.insert(
                    "historyFlushedImageDeferred".to_string(),
                    serde_json::json!(true),
                );
                obj.insert(
                    "historyFlushedDeferredImageCount".to_string(),
                    serde_json::json!(deferred_image_count),
                );
            }
            deferred_message.provider_meta = Some(provider_meta);
        }

        deferred_message.parts = next_parts;
        (
            deferred_message,
            deferred_image_count,
            deferred_base64_chars,
        )
    }

    // 1. 先写入所有消息到会话记录。
    //
    // 这里统一覆盖 created_at 为 history_flush_time，
    // 目的是把“正式进入历史的时间”作为消息的业务生效时间。
    // 入队时间只用于队列观察，不用于正式会话排序和轮次判断。
    let conversation_meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(conversation_meta)
            if conversation_meta.status.trim() != "archived"
                && conversation_meta
                    .archived_at
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none() => conversation_meta,
        _ => {
            complete_pending_chat_events_with_error(
                state,
                &event_ids,
                &format!("目标会话不存在，conversationId={conversation_id}"),
            )?;
            return Err(format!("目标会话不存在，conversationId={conversation_id}"));
        }
    };
    let scheduler_agents = state_read_agents_cached(state)?;
    let last_block = conversation_service_v2().get_conversation_last_block(state, conversation_id)?;
    let actual_message_count = last_block.messages.len();
    let actual_body_message_count = last_block
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant"
            )
        })
        .count();
    let is_empty_conversation = actual_message_count == 0;
    let should_seed_summary_context = is_empty_conversation
        && !conversation_meta.has_context_compaction_message
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT;
    if !is_empty_conversation
        && !conversation_meta.has_context_compaction_message
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT
    {
        runtime_log_warn(format!(
            "[上下文整理] 跳过补种初始摘要，任务=scheduler_history_flush，conversation_id={}，原因=non_empty_conversation_missing_compaction_marker，message_count={}，body_message_count={}，last_message_at={}",
            conversation_id,
            actual_message_count,
            actual_body_message_count,
            last_block
                .messages
                .last()
                .map(|message| message.created_at.as_str())
                .unwrap_or("")
        ));
    }
    let mut prepared_batches = Vec::<Vec<(ChatMessage, Vec<String>)>>::with_capacity(events.len());
    for event in &events {
        let mut prepared_messages =
            Vec::<(ChatMessage, Vec<String>)>::with_capacity(event.messages.len());
        for message in &event.messages {
            let mut persisted = message.clone();
            externalize_message_parts_to_media_refs(&mut persisted.parts, &state.data_path)?;
            persisted.created_at = history_flush_time.clone();
            let recall_payload = if persisted.role.trim() == "user"
                && !matches!(event.source, ChatEventSource::RemoteIm)
            {
                with_memory_lock(state, "scheduler_user_message_recall", || {
                    collect_recall_payload_for_user_message(
                        &state.data_path,
                        &scheduler_agents,
                        &event.session_info.agent_id,
                        &persisted,
                    )
                })?
            } else {
                UserMessageRecallPayload::default()
            };
            if !recall_payload.stored_ids.is_empty() {
                write_retrieved_memory_ids_into_provider_meta(
                    &mut persisted.provider_meta,
                    &recall_payload.stored_ids,
                );
            }
            prepared_messages.push((persisted, recall_payload.raw_ids));
        }
        prepared_batches.push(prepared_messages);
    }
    let persisted_recent_messages_before_flush = conversation_service_v2()
        .get_conversation_recent_messages(state, conversation_id, 7)
        .unwrap_or_default();
    let commit_result = conversation_service_v2().commit_scheduler_history_flush(
        state,
        conversation_id,
        &events,
        prepared_batches,
        &history_flush_time,
        should_seed_summary_context,
        actual_message_count > 0,
    )?;
    let persisted_batch_messages = commit_result.persisted_batch_messages;
    let event_activate_flags = commit_result.event_activate_flags;

    // 2. 判断是否需要激活主助理。
    // 这一步故意放在“写历史之后”，避免出现前端先开流式、
    // 但本批消息还没正式落入历史的时序错乱。
    let all_activated_remote_im_sources =
        collect_activated_remote_im_sources(&events, &event_activate_flags);
    let mut secretary_remote_im_sources = all_activated_remote_im_sources
        .iter()
        .filter(|source| source.remote_contact_type.trim().eq_ignore_ascii_case("group"))
        .cloned()
        .collect::<Vec<_>>();
    // 私聊直接进入绑定会话的主助理串行轮次；群聊才由秘书独立判断，并创建或续用远程应答委托。
    let activated_remote_im_sources = all_activated_remote_im_sources
        .into_iter()
        .filter(|source| !source.remote_contact_type.trim().eq_ignore_ascii_case("group"))
        .collect::<Vec<_>>();
    let mut should_activate = events
        .iter()
        .zip(event_activate_flags.iter().copied())
        .any(|(event, should_activate)| {
            should_activate && (!matches!(event.source, ChatEventSource::RemoteIm)
                || !remote_im_event_requires_reply_delegate(event))
        });
    let has_remote_group_messages = events.iter().any(remote_im_event_requires_reply_delegate);
    if has_remote_group_messages {
        process_persisted_remote_im_events_individually(
            state,
            conversation_id,
            &events,
            &event_activate_flags,
            &persisted_recent_messages_before_flush,
            &persisted_batch_messages,
            &scheduler_agents,
        )
        .await;
        // 远程应答委托不参与主轮次收尾。
        if !secretary_remote_im_sources.is_empty() {
            secretary_remote_im_sources.clear();
        }
    }
    let mut remote_im_skip_decision = None::<String>;
    let mut activating_session_info = events.first().map(|event| event.session_info.clone());
    if should_activate && !secretary_remote_im_sources.is_empty() {
        if let Some(contact) = remote_im_resolve_secretary_contact(state, &activated_remote_im_sources)? {
            match remote_im_resolve_contact_assistant_context(state, &contact) {
                Ok(resolved_assistant) => {
                    set_conversation_remote_im_assistant_context(
                        state,
                        conversation_id,
                        Some(resolved_assistant),
                    )?;
                }
                Err(err) => {
                    should_activate = false;
                    remote_im_skip_decision = Some("no_reply".to_string());
                    set_conversation_remote_im_assistant_context(state, conversation_id, None)?;
                    runtime_log_warn(format!(
                        "[远程联系人入场] 当前助理上下文非法，跳过本轮激活: conversation_id={}, contact_id={}, error={}",
                        conversation_id, contact.id, err
                    ));
                    remote_im_append_contact_log(
                        &contact,
                        "warn",
                        format!(
                            "[联系人入场] 跳过: contact={}, reason={}",
                            remote_im_contact_log_label(&contact),
                            err
                        ),
                    );
                    let follow_up_sources = remote_im_finalize_round_completion(
                        state,
                        &activated_remote_im_sources,
                        Some("no_reply"),
                        None,
                        None,
                        &history_flush_time,
                    )?;
                    if !follow_up_sources.is_empty() {
                        runtime_log_warn(format!(
                            "[远程联系人入场] 跳过激活后仍出现待办续跑，当前先跳过: conversation_id={}, source_count={}",
                            conversation_id,
                            follow_up_sources.len()
                        ));
                    }
                }
            }
            if should_activate {
                let current_assistant =
                    remote_im_secretary_current_assistant_context(state, conversation_id)?;
                activating_session_info = Some(ChatSessionInfo {
                    agent_id: current_assistant.agent_id.clone(),
                });
                let previous_history_messages = persisted_recent_messages_before_flush.as_slice();
                let secretary_recent_history = remote_im_collect_secretary_recent_messages(
                    previous_history_messages,
                    7,
                    &contact,
                    &scheduler_agents,
                    &current_assistant,
                );
                let secretary_new_batch_messages = remote_im_collect_secretary_recent_messages(
                    &persisted_batch_messages,
                    persisted_batch_messages.len(),
                    &contact,
                    &scheduler_agents,
                    &current_assistant,
                );
                let work_ledger = build_remote_im_assistant_work_ledger(
                    state,
                    &contact.id,
                    &conversation_id,
                )
                .unwrap_or_else(|err| {
                    runtime_log_warn(format!(
                        "[助理工作账本] 降级，任务=秘书批次判断，contact_id={}，error={}",
                        contact.id, err
                    ));
                    "（无）".to_string()
                });
                let active_remote_reply_delegate_ids =
                    remote_im_reply_delegate_active_ids_for_contact(state, &contact.id)?;
                let decision = match run_remote_im_secretary_decision(
                    state,
                    &contact,
                    &current_assistant,
                    &secretary_recent_history,
                    &secretary_new_batch_messages,
                    &work_ledger,
                    &active_remote_reply_delegate_ids,
                )
                .await
                {
                    Ok(result) => result,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[远程联系人秘书] 判断失败，降级为不回复: conversation_id={}, contact_id={}, error={}",
                            conversation_id, contact.id, err
                        ));
                        remote_im_append_contact_log(
                            &contact,
                            "warn",
                            format!(
                                "[联系人秘书] 智能判断失败: contact={}, strategy=smart_judge, fallback=no_reply, error={}",
                                remote_im_contact_log_label(&contact),
                                err
                            ),
                        );
                        RemoteImSecretaryDecision {
                            should_reply: false,
                            target_delegate_id: None,
                            reason: format!("秘书判断失败，已降级为不回复：{err}"),
                            model_name: String::new(),
                            emit_log: true,
                        }
                    }
                };
                if decision.emit_log {
                    runtime_log_warn(format!(
                        "[远程联系人秘书] 决策完成: conversation_id={}, contact_id={}, should_reply={}, model={}, reason={}",
                        conversation_id,
                        contact.id,
                        decision.should_reply,
                        if decision.model_name.trim().is_empty() {
                            "fallback"
                        } else {
                            decision.model_name.as_str()
                        },
                        decision.reason
                    ));
                    remote_im_append_contact_log(
                        &contact,
                        "info",
                        format!(
                            "[联系人秘书] 智能判断: contact={}, result={}, model={}, history_count={}, new_count={}, reason={}",
                            remote_im_contact_log_label(&contact),
                            if decision.should_reply { "回复" } else { "不回复" },
                            if decision.model_name.trim().is_empty() {
                                "fallback"
                            } else {
                                decision.model_name.as_str()
                            },
                            secretary_recent_history.len(),
                            secretary_new_batch_messages.len(),
                            decision.reason
                        ),
                    );
                }
                if !decision.should_reply {
                    should_activate = false;
                    remote_im_skip_decision = Some("no_reply".to_string());
                    let follow_up_sources = remote_im_finalize_round_completion(
                        state,
                        &activated_remote_im_sources,
                        Some("no_reply"),
                        None,
                        None,
                        &history_flush_time,
                    )?;
                    if !follow_up_sources.is_empty() {
                        runtime_log_warn(format!(
                            "[远程联系人秘书] 判定不回复后仍出现待办续跑，当前先跳过: conversation_id={}, source_count={}",
                            conversation_id,
                            follow_up_sources.len()
                        ));
                    }
                } else if let (Some(target_delegate_id), Some(guidance_message)) = (
                    decision.target_delegate_id.as_deref(),
                    persisted_batch_messages
                        .iter()
                        .rev()
                        .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
                        .cloned(),
                ) {
                    match remote_im_reply_delegate_enqueue_guidance(
                        state,
                        target_delegate_id,
                        guidance_message,
                        None,
                    ) {
                        Ok(()) => {
                            should_activate = false;
                            remote_im_skip_decision =
                                Some("remote_reply_delegate_guidance".to_string());
                            runtime_log_info(format!(
                                "[远程应答委托] 完成，任务=投递引导，conversation_id={}，contact_id={}，delegate_id={}",
                                conversation_id, contact.id, target_delegate_id
                            ));
                        }
                        Err(err) => runtime_log_warn(format!(
                            "[远程应答委托] 引导投递竞态降级，保留当前批次走新委托，conversation_id={}，contact_id={}，delegate_id={}，error={}",
                            conversation_id, contact.id, target_delegate_id, err
                        )),
                    }
                } else if let (Some(source), Some(trigger_message_id)) = (
                    activated_remote_im_sources.first().cloned(),
                    persisted_batch_messages
                        .iter()
                        .rev()
                        .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
                        .map(|message| message.id.clone()),
                ) {
                    let should_apply_dynamic_wake =
                        effective_remote_im_contact_response_strategy(&contact) == "smart_judge";
                    let patience_seconds = remote_im_channel_behavior_settings_for_contact(state, &contact)
                        .patience_seconds;
                    if should_apply_dynamic_wake {
                        if let Err(err) = remote_im_mark_contact_present_and_schedule_after_entry_compaction(
                            state,
                            &contact.id,
                            conversation_id,
                            &trigger_message_id,
                            patience_seconds,
                            "秘书决定通知远程应答委托",
                        ) {
                            runtime_log_warn(format!(
                                "[群聊秘书] 在场状态、压缩或计时刷新降级，conversation_id={}，contact_id={}，error={}",
                                conversation_id, contact.id, err
                            ));
                        }
                    } else {
                        remote_im_mark_contact_present_and_schedule(
                            state,
                            &contact.id,
                            patience_seconds,
                            "秘书决定通知远程应答委托",
                        )?;
                    }
                    // 未来的自己请停手：这里会把触发消息塞进远程应答委托，
                    // 属于后端生成链路。绝对不能读取 frontend_display_only，
                    // 否则工具历史会被展示投影污染后继续进模型/持久化流程。
                    let trigger_message = conversation_service_v2().get_raw_message_by_id(
                        state,
                        conversation_id,
                        &trigger_message_id,
                    )?;
                    match spawn_remote_im_reply_delegate(
                        state,
                        &contact.id,
                        conversation_id,
                        &trigger_message,
                        &ChatSessionInfo {
                            agent_id: current_assistant.agent_id.clone(),
                        },
                        source,
                        remote_im_channel_behavior_settings_for_contact(state, &contact)
                            .patience_seconds,
                        effective_remote_im_contact_response_strategy(&contact) == "smart_judge",
                        false,
                        None,
                    ) {
                        Ok(delegate_id) => {
                            should_activate = false;
                            remote_im_skip_decision = Some("remote_reply_delegate".to_string());
                            runtime_log_info(format!(
                                "[远程应答委托] 开始，delegate_id={}，conversation_id={}，contact_id={}，trigger_message_id={}",
                                delegate_id, conversation_id, contact.id, trigger_message_id
                            ));
                        }
                        Err(err) => {
                            should_activate = false;
                            remote_im_skip_decision = Some("remote_reply_delegate_start_failed".to_string());
                            runtime_log_error(format!(
                                "[远程应答委托] 失败，任务=创建，conversation_id={}，contact_id={}，error={}",
                                conversation_id, contact.id, err
                            ));
                        }
                    }
                }
            }
        } else {
            set_conversation_remote_im_assistant_context(state, conversation_id, None)?;
        }
    }
    let guided_event_ids = events
        .iter()
        .filter(|event| event.queue_mode == ChatQueueMode::Guided)
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let activating_runtime_context = events
        .iter()
        .zip(event_activate_flags.iter().copied())
        .rev()
        .find_map(|(event, should_activate)| {
            if should_activate {
                event.runtime_context.clone()
            } else {
                None
            }
        });
    let activating_assistant_message_id = events
        .iter()
        .zip(event_activate_flags.iter().copied())
        .rev()
        .find_map(|(event, should_activate)| {
            if should_activate {
                event.assistant_message_id.clone()
            } else {
                None
            }
        });

    let batch_message_count = events.iter().map(|e| e.messages.len()).sum::<usize>();
    let mut activations = take_queued_chat_activations(state, &event_ids)?;
    if activations.is_empty() {
        activations = collect_active_chat_view_activations(state, conversation_id)?;
    }
    let mut history_flushed_messages =
        Vec::<ChatMessage>::with_capacity(persisted_batch_messages.len());
    for message in &persisted_batch_messages {
        let (deferred_message, _, _) =
            defer_image_parts_for_history_flushed(message, &state.data_path);
        history_flushed_messages.push(project_message_for_frontend_display_only(deferred_message));
    }
    let history_flushed_payload = serde_json::json!({
        "conversationId": conversation_id,
        "messageCount": batch_message_count,
        "messages": history_flushed_messages,
        "activateAssistant": should_activate,
    });
    emit_history_flushed_event(state, &history_flushed_payload, conversation_id, &event_ids);

    // 3. 如果需要激活，调用主助理。
    if should_activate {
        if let Some(activating_session_info) = activating_session_info.as_ref() {
            // 同一批里可能有多个激活请求，但前台主助理轮次只能有一个。
            // 因此这里只保留最后一个激活请求作为实际流式绑定对象。
            let activation = activations.pop();
            let main_request_id = runtime_context_request_id_or_new(
                activating_runtime_context.as_ref(),
                activation
                    .as_ref()
                    .map(|item| format!("queue-{}", item.event_id))
                    .as_deref(),
                "queue",
            );
            match activate_main_assistant(
                state,
                activating_session_info,
                conversation_id,
                activation.clone(),
                activating_assistant_message_id.clone(),
                (!guided_event_ids.is_empty()).then_some(guided_event_ids.as_slice()),
                activating_runtime_context.clone(),
                activated_remote_im_sources.clone(),
                oldest_queue_created_at,
            )
            .await
            {
                Ok(activated) => {
                    let result = activated.result;
                    let mut follow_up_sources = match remote_im_finalize_round_completion(
                        state,
                        &activated_remote_im_sources,
                        result.remote_im_reply_decision.as_deref(),
                        result.remote_im_reply_target.as_ref(),
                        None,
                        &history_flush_time,
                    ) {
                        Ok(sources) => sources,
                        Err(finalize_err) => {
                            runtime_log_warn(format!(
                            "[聊天调度] 远程联系人轮次收尾失败（完成分支），conversation_id={}，error={}",
                            conversation_id, finalize_err
                            ));
                            Vec::new()
                        }
                    };
                    follow_up_sources = filter_remote_im_follow_up_sources_for_pending_queue(
                        state,
                        conversation_id,
                        follow_up_sources,
                    );
                    emit_round_completed_event(
                        state,
                        conversation_id,
                        &result,
                        Some(activated.activation_id.as_str()),
                        Some(activated.request_id.as_str()),
                    );
                    complete_pending_chat_events_with_result(state, &event_ids, result)?;
                    while !follow_up_sources.is_empty() {
                        let follow_up_started_at = now_iso();
                        let mut follow_up_context =
                            runtime_context_new("remote_im", "remote_im_followup");
                        follow_up_context.request_id = Some(format!(
                            "remote-im-follow-up-{}",
                            Uuid::new_v4()
                        ));
                        let follow_up_request_id = runtime_context_request_id_or_new(
                            Some(&follow_up_context),
                            None,
                            "queue",
                        );
                        runtime_log_info(format!(
                            "[远程联系人状态机] 待办续跑 开始: conversation_id={}, source_count={}",
                            conversation_id,
                            follow_up_sources.len()
                        ));
                        match activate_main_assistant(
                            state,
                            activating_session_info,
                            conversation_id,
                            None,
                            None,
                            None,
                            Some(follow_up_context),
                            follow_up_sources.clone(),
                            &follow_up_started_at,
                        )
                        .await
                        {
                            Ok(follow_up_activated) => {
                                let follow_up_result = follow_up_activated.result;
                                follow_up_sources = match remote_im_finalize_round_completion(
                                    state,
                                    &follow_up_sources,
                                    follow_up_result.remote_im_reply_decision.as_deref(),
                                    follow_up_result.remote_im_reply_target.as_ref(),
                                    None,
                                    &follow_up_started_at,
                                ) {
                                    Ok(sources) => sources,
                                    Err(finalize_err) => {
                                        runtime_log_warn(format!(
                                            "[聊天调度] 远程联系人待办续跑收尾失败，conversation_id={}，error={}",
                                            conversation_id, finalize_err
                                        ));
                                        Vec::new()
                                    }
                                };
                                follow_up_sources =
                                    filter_remote_im_follow_up_sources_for_pending_queue(
                                        state,
                                        conversation_id,
                                        follow_up_sources,
                                    );
                                emit_round_completed_event(
                                    state,
                                    conversation_id,
                                    &follow_up_result,
                                    Some(follow_up_activated.activation_id.as_str()),
                                    Some(follow_up_activated.request_id.as_str()),
                                );
                            }
                            Err(err) => {
                                emit_round_failed_event(
                                    state,
                                    conversation_id,
                                    &err,
                                    Some(follow_up_request_id.as_str()),
                                    Some(follow_up_request_id.as_str()),
                                );
                                if let Err(finalize_err) = remote_im_finalize_round_completion(
                                    state,
                                    &follow_up_sources,
                                    None,
                                    None,
                                    Some(&err),
                                    &follow_up_started_at,
                                ) {
                                    runtime_log_warn(format!(
                                        "[聊天调度] 远程联系人待办续跑收尾失败（失败分支），conversation_id={}，original_error={}，finalize_error={}",
                                        conversation_id, err, finalize_err
                                    ));
                                }
                                return Err(err);
                            }
                        }
                    }
                }
                Err(err) => {
                    if err == CHAT_ABORTED_BY_USER_ERROR {
                        complete_pending_chat_events_with_error(state, &event_ids, &err)?;
                        if let Err(finalize_err) = remote_im_finalize_round_completion(
                            state,
                            &activated_remote_im_sources,
                            None,
                            None,
                            None,
                            &history_flush_time,
                        ) {
                            runtime_log_warn(format!(
                                "[聊天调度] 远程联系人轮次收尾失败（停止分支），conversation_id={}，error={}",
                                conversation_id, finalize_err
                            ));
                        }
                        return Ok(());
                    }
                    emit_round_failed_event(
                        state,
                        conversation_id,
                        &err,
                        Some(main_request_id.as_str()),
                        Some(main_request_id.as_str()),
                    );
                    complete_pending_chat_events_with_error(state, &event_ids, &err)?;
                    if let Err(finalize_err) = remote_im_finalize_round_completion(
                        state,
                        &activated_remote_im_sources,
                        None,
                        None,
                        Some(&err),
                        &history_flush_time,
                    ) {
                        runtime_log_warn(format!(
                            "[聊天调度] 远程联系人轮次收尾失败（失败分支），conversation_id={}，original_error={}，finalize_error={}",
                            conversation_id, err, finalize_err
                        ));
                    }
                    return Err(err);
                }
            }
        }
    } else {
        set_conversation_remote_im_activation_sources(state, conversation_id, Vec::new())?;
        set_conversation_remote_im_assistant_context(state, conversation_id, None)?;
        if !guided_event_ids.is_empty() {
            let error_text = "引导消息未能触发助理回复";
            complete_pending_chat_events_with_error(state, &event_ids, error_text)?;
            return Err(error_text.to_string());
        }
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
                final_response_text: String::new(),
                archived_before_send: false,
                assistant_message: None,
                provider_prompt_tokens: None,
                estimated_prompt_tokens: None,
                effective_prompt_tokens: None,
                effective_prompt_source: None,
                context_window_tokens: None,
                max_output_tokens: None,
                context_usage_percent: None,
                remote_im_reply_decision: remote_im_skip_decision,
                remote_im_reply_target: None,
                usage: None,
                activation_request_id: None,
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
    assistant_message_id: Option<String>,
    guided_event_ids: Option<&[String]>,
    runtime_context: Option<RuntimeContext>,
    remote_im_activation_sources: Vec<RemoteImActivationSource>,
    oldest_queue_created_at: &str,
) -> Result<ActivatedAssistantResult, String> {
    let mut runtime_context = runtime_context.unwrap_or_default();
    if runtime_context.bound_remote_im_activation_source.is_none() {
        runtime_context.bound_remote_im_activation_source =
            resolve_bound_remote_im_activation_source(&remote_im_activation_sources);
    }
    let activation_trace_id = activation
        .as_ref()
        .map(|item| format!("queue-{}", item.event_id));
    let activation_delta_channel = activation
        .as_ref()
        .and_then(|item| item.delta_channel.clone());
    let trace_id = runtime_context_request_id_or_new(
        Some(&runtime_context),
        activation_trace_id.as_deref(),
        "queue",
    );
    if runtime_context.request_id.is_none() {
        runtime_context.request_id = Some(trace_id.clone());
    }
    if runtime_context.target_conversation_id.is_none() {
        runtime_context.target_conversation_id = Some(conversation_id.to_string());
    }
    if runtime_context.executor_agent_id.is_none() {
        runtime_context.executor_agent_id = Some(session_info.agent_id.clone());
    }
    let executor_agent_id = runtime_context
        .executor_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(session_info.agent_id.as_str())
        .to_string();
    let activation_id = trace_id.clone();
    let activation_reason = resolve_activation_reason(&runtime_context);
    let stream_started_at = now_iso();
    let stream_started_at_ms = now_unix_ms();
    let assistant_message_id = assistant_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if let Some(latest_message) = conversation_service_v2()
        .get_conversation_recent_messages(state, conversation_id, 1)?
        .pop()
    {
        if main_assistant_activation_should_reject_latest_message(
            &latest_message,
            executor_agent_id.as_str(),
        ) {
            return Err("当前最后一条消息来自助理自身，无需重复激活。".to_string());
        }
    }
    conversation_service_v2().bootstrap_streaming_assistant_message(
        state,
        &AssistantMessageBootstrapInput {
            conversation_id: conversation_id.to_string(),
            assistant_message_id: assistant_message_id.clone(),
            speaker_agent_id: executor_agent_id.clone(),
            created_at: Some(stream_started_at.clone()),
            provider_meta_patch: None,
            compaction_preserved_messages: None,
        },
    )?;
    reset_conversation_stream_runtime_cache(
        state,
        conversation_id,
        activation_id.as_str(),
        trace_id.as_str(),
        executor_agent_id.as_str(),
        assistant_message_id.as_str(),
        stream_started_at.as_str(),
        stream_started_at_ms,
    )?;
    emit_round_started_event(
        state,
        conversation_id,
        activation_id.as_str(),
        trace_id.as_str(),
        assistant_message_id.as_str(),
        activation_reason.as_str(),
        executor_agent_id.as_str(),
        stream_started_at.as_str(),
        stream_started_at_ms,
    );

    // 设置状态为 AssistantStreaming
    set_conversation_runtime_state_and_emit(
        state,
        conversation_id,
        MainSessionState::AssistantStreaming,
    )?;
    set_conversation_remote_im_activation_sources(
        state,
        conversation_id,
        remote_im_activation_sources.clone(),
    )?;
    if let Some(event_ids) = guided_event_ids.filter(|items| !items.is_empty()) {
        let removed = remove_queue_events_by_ids(state, conversation_id, event_ids)?;
        emit_chat_queue_snapshot(state);
        runtime_log_info(format!(
            "[引导投送] 完成，任务=remove_guided_queue_after_busy，conversation_id={}，event_count={}，removed_count={}",
            conversation_id,
            event_ids.len(),
            removed
        ));
    }

    // 对 WeixinOc 渠道启动 typing 状态（对方正在输入）
    let weixin_oc_typing_sources: Vec<(String, String, WeixinOcCredentials)> = {
        let config = state_read_config_cached(state);
        let config = match config {
            Ok(c) => c,
            Err(err) => {
                runtime_log_warn(format!("[聊天调度] 读取配置失败，跳过 typing 启动: error={}", err));
                AppConfig::default()
            }
        };
        remote_im_activation_sources
            .iter()
            .filter(|src| src.platform == RemoteImPlatform::WeixinOc)
            .filter_map(|src| {
                let channel = remote_im_channel_by_id(&config, &src.channel_id)?;
                let effective_channel =
                    remote_im_channel_with_effective_credentials(state, channel).ok()?;
                let credentials = WeixinOcCredentials::from_value(&effective_channel.credentials);
                if credentials.token.trim().is_empty() {
                    runtime_log_warn(format!(
                        "[聊天调度] 跳过个人微信 typing: 缺少有效 token, channel_id={}, remote_contact_id={}",
                        src.channel_id, src.remote_contact_id
                    ));
                    return None;
                }
                Some((
                    src.channel_id.clone(),
                    src.remote_contact_id.clone(),
                    credentials,
                ))
            })
            .collect()
    };
    for (ch_id, contact_id, credentials) in &weixin_oc_typing_sources {
        let ctx_token = weixin_oc_manager()
            .get_context_token(&ch_id, &contact_id)
            .await;
        weixin_oc_manager()
            .start_typing(&ch_id, credentials.clone(), &contact_id, ctx_token)
            .await;
    }

    // 构造 trigger_only 请求
    let request = SendChatRequest {
        trigger_only: true, // 不写入新消息，只触发助理回复
        session: Some(SessionSelector {
            api_config_id: None,
            agent_id: executor_agent_id.clone(),
            conversation_id: Some(conversation_id.to_string()),
        }),
        payload: ChatInputPayload {
            text: None,
            display_text: None,
            parts: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        },
        speaker_agent_id: None,
        trace_id: Some(trace_id.clone()),
        assistant_message_id: Some(assistant_message_id.clone()),
        oldest_queue_created_at: Some(oldest_queue_created_at.to_string()),
        remote_im_activation_sources,
        runtime_context: Some(runtime_context),
    };

    // 使用 emit 作为远程激活轮次的流式主通道，避免前端窗口重绑定造成 channel 失联。
    let state_for_delta = state.clone();
    let conversation_id_for_emit = conversation_id.to_string();
    let activation_delta_channel_for_emit = activation_delta_channel.clone();
    let stream_start_rebind_emitted = std::sync::Arc::new(std::sync::Mutex::new(false));
    let stream_start_rebind_emitted_for_channel = stream_start_rebind_emitted.clone();
    let active_channel: tauri::ipc::Channel<AssistantDeltaEvent> = tauri::ipc::Channel::new(
        move |body| {
            let parsed_event = match body {
                tauri::ipc::InvokeResponseBody::Json(json) => {
                    serde_json::from_str::<AssistantDeltaEvent>(&json).ok()
                }
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    serde_json::from_slice::<AssistantDeltaEvent>(&bytes).ok()
                }
            };
            if let Some(mut event) = parsed_event {
                match update_conversation_stream_runtime_cache(
                    &state_for_delta,
                    &conversation_id_for_emit,
                    &event,
                ) {
                    Ok(Some(snapshot)) => {
                        event.stream_cache = Some(snapshot);
                    }
                    Ok(None) => {}
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[聊天流式缓存] 更新失败，conversation_id={}，kind={}，error={}",
                            conversation_id_for_emit.trim(),
                            event.kind.as_deref().unwrap_or("delta"),
                            err
                        ));
                    }
                }
                let mut stream_start_rebind_guard =
                    stream_start_rebind_emitted_for_channel.lock().ok();
                if stream_start_rebind_guard
                    .as_ref()
                    .map(|flag| !**flag)
                    .unwrap_or(true)
                    && is_visible_stream_progress_event(&event)
                {
                    if let Some(flag) = stream_start_rebind_guard.as_mut() {
                        **flag = true;
                    }
                }
                if event.kind.as_deref() == Some("stream_rebind_required") {
                    emit_stream_rebind_required_event(
                        &state_for_delta,
                        &conversation_id_for_emit,
                        event.request_id.as_deref(),
                        event.phase_id.as_deref(),
                        event.reason.as_deref().unwrap_or("tool_start"),
                    );
                } else {
                    let active_view_delivered = dispatch_assistant_delta_to_active_view(
                        &state_for_delta,
                        &conversation_id_for_emit,
                        &event,
                    );
                    if should_use_activation_delta_fallback(
                        active_view_delivered,
                        activation_delta_channel_for_emit.is_some(),
                    ) {
                        if let Some(channel) = activation_delta_channel_for_emit.as_ref() {
                            if let Err(err) = channel.send(event.clone()) {
                                runtime_log_warn(format!(
                                    "[聊天流式订阅] 降级，任务=投递本次发送通道，conversation_id={}，kind={}，error={}",
                                    conversation_id_for_emit.trim(),
                                    event.kind.as_deref().unwrap_or("delta"),
                                    err
                                ));
                            }
                        }
                    }
                }
            }
            Ok(())
        },
    );

    // 调用 send_chat_message_inner
    let result = send_chat_message_inner(request, state, &active_channel).await;

    // WeixinOc 渠道：回复结束后停止 typing
    for (ch_id, contact_id, _) in &weixin_oc_typing_sources {
        weixin_oc_manager().stop_typing(ch_id, contact_id).await;
    }

    if let Err(err) =
        set_conversation_remote_im_activation_sources(state, conversation_id, Vec::new())
    {
        runtime_log_error(format!(
            "[聊天调度] 清理远程IM激活来源失败: conversation_id={}, error={}",
            conversation_id, err
        ));
    }
    if let Err(err) = set_conversation_remote_im_assistant_context(state, conversation_id, None) {
        runtime_log_error(format!(
            "[聊天调度] 清理远程IM当前助理失败: conversation_id={}, error={}",
            conversation_id, err
        ));
    }

    set_conversation_runtime_state_and_emit(state, conversation_id, MainSessionState::Idle)?;
    if let Err(err) = clear_conversation_stream_runtime_cache(state, conversation_id) {
        runtime_log_warn(format!(
            "[聊天流式缓存] 清理失败，conversation_id={}，error={}",
            conversation_id, err
        ));
    }

    // 后台会话活动标记：前台观看时不写 completed/failed，直接清标记
    let is_watched = state
        .active_chat_view_bindings
        .lock()
        .ok()
        .map(|bindings| {
            bindings.values().any(|b| b.conversation_id.trim() == conversation_id)
        })
        .unwrap_or(false)
        || detached_chat_window_for_conversation(conversation_id).is_some();
    if is_watched {
        clear_conversation_list_activity_mark(state, conversation_id);
    } else {
        match &result {
            Ok(_) => {
                set_conversation_list_activity_mark(
                    state,
                    conversation_id,
                    ConversationListActivityMark {
                        activity: "completed".to_string(),
                        failed_message: None,
                        completed_at: Some(now_iso()),
                    },
                );
            }
            Err(err) => {
                if err != CHAT_ABORTED_BY_USER_ERROR {
                    set_conversation_list_activity_mark(
                        state,
                        conversation_id,
                        ConversationListActivityMark {
                            activity: "failed".to_string(),
                            failed_message: Some(err.clone()),
                            completed_at: None,
                        },
                    );
                } else {
                    clear_conversation_list_activity_mark(state, conversation_id);
                }
            }
        }
    }

    // 活动标记只影响当前会话概览项，避免大列表全量广播。
    if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        conversation_id,
    ) {
        runtime_log_warn(format!(
            "[会话概览] 跳过，任务=活动标记更新后推送，conversation_id={}，error={}",
            conversation_id, err
        ));
    }

    result.map(|result| {
        // 压缩重开会为新一轮重新生成 request_id，并用它发轮次开始事件；
        // 收尾事件必须沿用同一个标识，否则前端按「标识必须相等」判定时会丢弃终态。
        let final_activation_id = result
            .activation_request_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| activation_id.clone());
        ActivatedAssistantResult {
            result,
            activation_id: final_activation_id.clone(),
            request_id: final_activation_id,
        }
    })
}

include!("scheduler/round_events.rs");
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
        .values()
        .map(|binding| {
            format!(
                "{}#{}=>{}",
                binding.window_label, binding.binding_id, binding.conversation_id,
            )
        })
        .collect::<Vec<_>>();
    runtime_log_debug(format!(
        "[聊天调度] 绑定快照: conversation_id={}, bindings={:?}",
        conversation_id, binding_snapshot
    ));
    let exact = bindings
        .iter()
        .filter_map(|(window_label, binding)| {
            if binding.conversation_id != conversation_id {
                return None;
            }
            Some(QueuedChatActivation {
                event_id: format!("active-view:{window_label}"),
                delta_channel: None,
            })
        })
        .collect::<Vec<_>>();
    if !exact.is_empty() {
        runtime_log_info(format!(
            "[聊天调度] 绑定筛选命中(exact): conversation_id={}, hit={}",
            conversation_id,
            exact.len()
        ));
        return Ok(exact);
    }

    runtime_log_info(format!(
        "[聊天调度] 绑定筛选未命中: conversation_id={}, bindings_count={}",
        conversation_id,
        bindings.len()
    ));
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
        if let Some(delta_channel) = channels.remove(event_id) {
            activations.push(QueuedChatActivation {
                event_id: event_id.clone(),
                delta_channel: Some(delta_channel),
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
