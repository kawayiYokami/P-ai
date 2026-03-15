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
    /// 正在压缩/整理上下文
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
    pub api_config_id: String,
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

fn lock_chat_pending_queue(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, std::collections::VecDeque<ChatPendingEvent>>, String> {
    match state.chat_pending_queue.lock() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            eprintln!("[聊天调度] 警告: chat_pending_queue 锁已 poison，正在继续恢复使用");
            Ok(poisoned.into_inner())
        }
    }
}

/// 获取队列状态
pub(crate) fn get_queue_snapshot(state: &AppState) -> Result<Vec<ChatQueueEventSummary>, String> {
    let queue = lock_chat_pending_queue(state)?;

    let summaries = queue
        .iter()
        .map(|event| {
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

            ChatQueueEventSummary {
                id: event.id.clone(),
                source: event.source.clone(),
                created_at: event.created_at.clone(),
                message_preview: preview,
                conversation_id: event.conversation_id.clone(),
            }
        })
        .collect();

    Ok(summaries)
}

/// 从队列中移除指定事件
pub(crate) fn remove_from_queue(state: &AppState, event_id: &str) -> Result<Option<ChatPendingEvent>, String> {
    let mut queue = lock_chat_pending_queue(state)?;

    if let Some(pos) = queue.iter().position(|e| e.id == event_id) {
        let removed = queue.remove(pos);
        eprintln!(
            "[聊天调度] 从队列移除事件: id={}, source={:?}, queue_len={}",
            event_id, removed.as_ref().map(|e| &e.source), queue.len()
        );
        drop(queue);
        complete_pending_chat_events_with_error(state, &[event_id.to_string()], "消息已从队列移除")?;
        Ok(removed)
    } else {
        Ok(None)
    }
}

// ==================== 队列管理函数 ====================

/// 入队函数：添加事件到队列
pub(crate) fn enqueue_chat_event(
    state: &AppState,
    event: ChatPendingEvent,
) -> Result<(), String> {
    let started_at = std::time::Instant::now();
    let mut queue = lock_chat_pending_queue(state)?;

    let queue_len_before = queue.len();
    queue.push_back(event.clone());
    let elapsed_ms = started_at.elapsed().as_millis();

    eprintln!(
        "[聊天调度] 完成: 事件入队, id={}, source={:?}, activate={}, queue_len={}->{}, elapsed_ms={}",
        event.id, event.source, event.activate_assistant, queue_len_before, queue.len(), elapsed_ms
    );

    Ok(())
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

/// 批量出队：一次性取走当前所有待处理事件
///
/// 注意：这里故意不是“一条一条弹出”。
/// 我们需要把同一时刻已经排到门口的消息视为同一批候选输入，
/// 之后再按 conversation_id 分组，统一决定这批消息如何进入正式历史。
fn dequeue_batch(
    state: &AppState,
) -> Result<Vec<ChatPendingEvent>, String> {
    let mut queue = lock_chat_pending_queue(state)?;

    let batch: Vec<ChatPendingEvent> = queue.drain(..).collect();
    Ok(batch)
}

// ==================== 状态机管理函数 ====================

/// 获取当前状态
pub(crate) fn get_main_session_state(state: &AppState) -> Result<MainSessionState, String> {
    let state_guard = state
        .main_session_state
        .lock()
        .map_err(|_| "Failed to lock main session state".to_string())?;
    Ok(state_guard.clone())
}

/// 设置状态并记录日志
pub(crate) fn set_main_session_state(
    state: &AppState,
    new_state: MainSessionState,
) -> Result<(), String> {
    let mut state_guard = state
        .main_session_state
        .lock()
        .map_err(|_| "Failed to lock main session state".to_string())?;

    let old_state = state_guard.clone();
    *state_guard = new_state.clone();

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

    eprintln!(
        "[聊天调度] 状态转换: {} -> {}",
        old_state_cn, new_state_cn
    );

    // 可选：发送事件到前端
    // if let Some(app_handle) = state.app_handle.lock().ok().and_then(|g| g.as_ref()) {
    //     let _ = app_handle.emit("main-session-state-changed", &new_state);
    // }

    Ok(())
}

/// 检查是否可以出队
///
/// 只有主助理完全空闲时，才允许把下一批消息刷入历史。
/// 这样可以保证：
/// 1. 当前轮次流式期间，新消息只会排队，不会打断当前轮次；
/// 2. 整理上下文期间，也不会出现“半整理、半对话”的混乱状态；
/// 3. 每一轮开始前，输入集都是稳定且封闭的一批消息。
pub(crate) fn can_dequeue(state: &AppState) -> Result<bool, String> {
    let current_state = get_main_session_state(state)?;
    let queue = lock_chat_pending_queue(state)?;

    let can = current_state == MainSessionState::Idle && !queue.is_empty();

    if !can && !queue.is_empty() {
        let state_cn = match current_state {
            MainSessionState::Idle => "空闲",
            MainSessionState::AssistantStreaming => "助理流式输出",
            MainSessionState::OrganizingContext => "整理上下文",
        };
        eprintln!(
            "[聊天调度] 跳过: 出队被阻塞, state={}, queue_len={}, 原因=主助理未空闲或仍在整理上下文",
            state_cn, queue.len()
        );
    }

    Ok(can)
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
    loop {
        // 获取出队锁并批量获取事件，然后立即释放锁
        let batch = {
            let _dequeue_guard = state
                .dequeue_lock
                .lock()
                .map_err(|_| "Failed to lock dequeue lock".to_string())?;

            // 检查是否可以出队
            if !can_dequeue(state)? {
                return Ok(());
            }

            // 批量获取所有待处理事件
            dequeue_batch(state)?
        }; // _dequeue_guard 在这里被释放

        if batch.is_empty() {
            return Ok(());
        }

        eprintln!(
            "[CHAT-SCHEDULER] Processing batch: size={}, events={:?}",
            batch.len(),
            batch.iter().map(|e| (&e.id, &e.source)).collect::<Vec<_>>()
        );

        // 按会话分组，并保持首次出现顺序稳定。
        // 这样不同会话互不串扰，而同一会话在同一批里收到的消息，
        // 又能以稳定顺序一起进入该会话的下一轮输入集。
        let mut grouped: std::collections::HashMap<String, Vec<ChatPendingEvent>> = std::collections::HashMap::new();
        let mut conversation_order = Vec::<String>::new();
        for event in batch {
            let conversation_id = event.conversation_id.clone();
            if !grouped.contains_key(&conversation_id) {
                conversation_order.push(conversation_id.clone());
            }
            grouped
                .entry(conversation_id)
                .or_insert_with(Vec::new)
                .push(event);
        }

        // 逐会话处理
        for conversation_id in conversation_order {
            let Some(events) = grouped.remove(&conversation_id) else {
                continue;
            };
            if let Err(err) = process_conversation_batch(state, &conversation_id, events).await {
                eprintln!("[CHAT-SCHEDULER] Error processing conversation {}: {}", conversation_id, err);
            }
        }

        // 处理完一批后，检查是否还有新事件，如果有则继续循环
        // 如果没有或状态不是 Idle，则退出
    }
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
        resolve_selected_api_config(app_config, Some(session.api_config_id.as_str()))
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
                    .entry(event.session_info.api_config_id.clone())
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
    let history_flushed_message = serde_json::json!({
        "conversationId": conversation_id,
        "messageCount": batch_message_count,
        "messages": persisted_batch_messages,
        "activateAssistant": should_activate,
    })
    .to_string();
    for active in &activations {
        send_history_flushed_event(active, &history_flushed_message);
    }

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
                        send_round_completed_event(active, &result);
                    }
                    complete_pending_chat_events_with_result(state, &event_ids, result)?;
                }
                Err(err) => {
                    if let Some(active) = activation.as_ref() {
                        send_round_failed_event(active, &err);
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
        "[聊天调度] 开始: 激活主助理, conversation_id={}, api_config_id={}, agent_id={}, oldest_queue_created_at={}",
        conversation_id,
        session_info.api_config_id,
        session_info.agent_id,
        oldest_queue_created_at,
    );

    let trace_id = activation
        .as_ref()
        .map(|item| format!("queue-{}", item.event_id))
        .unwrap_or_else(|| format!("queue-{}", Uuid::new_v4()));

    // 设置状态为 AssistantStreaming
    set_main_session_state(state, MainSessionState::AssistantStreaming)?;

    // 构造 trigger_only 请求
    let request = SendChatRequest {
        trigger_only: true,  // 不写入新消息，只触发助理回复
        session: Some(SessionSelector {
            api_config_id: Some(session_info.api_config_id.clone()),
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

    // 创建一个空的 delta channel
    let noop_channel = tauri::ipc::Channel::new(|_event| {
        // 不处理，实际流式输出由前端的 channel 处理
        Ok(())
    });
    let active_channel = activation
        .as_ref()
        .map(|item| item.on_delta.clone())
        .unwrap_or(noop_channel);

    // 调用 send_chat_message_inner
    let result = send_chat_message_inner(request, state, &active_channel).await;

    // 回复完成，切换回 Idle
    set_main_session_state(state, MainSessionState::Idle)?;

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

    let mode = contact.activation_mode.trim().to_ascii_lowercase();
    if mode != "always" && mode != "keyword" {
        log_remote_im_activation_decision(
            &sender.channel_id,
            &contact.remote_contact_id,
            "skip",
            &format!("activationMode={}（不激活）", mode),
        );
        return false;
    }

    if mode == "keyword" {
        let text = remote_im_event_message_text(event);
        let matched = contact
            .activation_keywords
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .any(|keyword| text_contains_keyword(&text, keyword));
        if !matched {
            log_remote_im_activation_decision(
                &sender.channel_id,
                &contact.remote_contact_id,
                "skip",
                "keyword 未命中，跳过激活",
            );
            return false;
        }
    }

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
    if contact.activation_cooldown_seconds > 0 {
        if let Some(last) = contact
            .last_activated_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        {
            let elapsed = now_ts.saturating_sub(last.timestamp()) as u64;
            if elapsed < contact.activation_cooldown_seconds {
                let remaining = contact.activation_cooldown_seconds.saturating_sub(elapsed);
                log_remote_im_activation_decision(
                    &sender.channel_id,
                    &contact.remote_contact_id,
                    "skip",
                    &format!("冷却中，remaining={}s", remaining),
                );
                return false;
            }
        }
    }

    contact.last_activated_at = Some(now.to_string());
    log_remote_im_activation_decision(
        &sender.channel_id,
        &contact.remote_contact_id,
        "activate",
        "满足激活条件，触发主助理",
    );
    true
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

fn send_history_flushed_event(
    activation: &QueuedChatActivation,
    payload_json: &str,
) {
    let _ = activation.on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("history_flushed".to_string()),
        tool_name: None,
        tool_status: None,
        tool_args: None,
        message: Some(payload_json.to_string()),
    });
}

fn send_round_completed_event(
    activation: &QueuedChatActivation,
    result: &SendChatResult,
) {
    if activation.source != QueuedChatActivationSource::ActiveViewBinding {
        return;
    }
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
}

fn send_round_failed_event(
    activation: &QueuedChatActivation,
    error_text: &str,
) {
    if activation.source != QueuedChatActivationSource::ActiveViewBinding {
        return;
    }
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
}

fn collect_active_chat_view_activations(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<QueuedChatActivation>, String> {
    let bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?;
    Ok(bindings
        .iter()
        .filter_map(|(window_label, binding)| {
            if binding.conversation_id != conversation_id.trim() {
                return None;
            }
            Some(QueuedChatActivation {
                event_id: format!("active-view:{window_label}"),
                on_delta: binding.on_delta.clone(),
                source: QueuedChatActivationSource::ActiveViewBinding,
            })
        })
        .collect())
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
