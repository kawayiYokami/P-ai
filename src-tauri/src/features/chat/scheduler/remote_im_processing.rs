fn remote_im_activation_source_key(source: &RemoteImActivationSource) -> String {
    format!(
        "{}::{}::{}",
        source.channel_id.trim(),
        source.remote_contact_type.trim(),
        source.remote_contact_id.trim()
    )
}

fn remote_im_activation_source_from_sender(
    sender: &RemoteImMessageSource,
) -> RemoteImActivationSource {
    RemoteImActivationSource {
        channel_id: sender.channel_id.trim().to_string(),
        platform: sender.platform.clone(),
        remote_contact_type: sender.remote_contact_type.trim().to_string(),
        remote_contact_id: sender.remote_contact_id.trim().to_string(),
        remote_contact_name: sender.remote_contact_name.trim().to_string(),
    }
}

pub(crate) fn resolve_bound_remote_im_activation_source(
    sources: &[RemoteImActivationSource],
) -> Option<RemoteImActivationSource> {
    if sources.len() == 1 {
        return sources.first().cloned();
    }
    None
}

pub(crate) fn set_conversation_remote_im_activation_sources(
    state: &AppState,
    conversation_id: &str,
    sources: Vec<RemoteImActivationSource>,
) -> Result<(), String> {
    let mut slots = lock_conversation_runtime_slots(state)?;
    let slot = conversation_slot_mut(&mut slots, conversation_id);
    slot.active_remote_im_activation_sources = sources;
    slot.last_activity_at = now_iso();
    Ok(())
}

pub(crate) fn get_conversation_remote_im_activation_sources(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<RemoteImActivationSource>, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Ok(Vec::new());
    }
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(normalized_conversation_id)
        .map(|slot| slot.active_remote_im_activation_sources.clone())
        .unwrap_or_default())
}

pub(crate) fn set_conversation_remote_im_assistant_context(
    state: &AppState,
    conversation_id: &str,
    context: Option<RemoteImConversationAssistantContext>,
) -> Result<(), String> {
    let normalized_conversation_id = conversation_id.trim();
    let mut slots = lock_conversation_runtime_slots(state)?;
    let slot = conversation_slot_mut(&mut slots, normalized_conversation_id);
    slot.active_remote_im_assistant_context = context;
    slot.last_activity_at = now_iso();
    Ok(())
}

pub(crate) fn get_conversation_remote_im_assistant_context(
    state: &AppState,
    conversation_id: &str,
) -> Result<Option<RemoteImConversationAssistantContext>, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Ok(None);
    }
    let slots = lock_conversation_runtime_slots(state)?;
    Ok(slots
        .get(normalized_conversation_id)
        .and_then(|slot| slot.active_remote_im_assistant_context.clone()))
}

fn collect_activated_remote_im_sources(
    events: &[ChatPendingEvent],
    event_activate_flags: &[bool],
) -> Vec<RemoteImActivationSource> {
    let mut activated_remote_im_sources = Vec::<RemoteImActivationSource>::new();
    let mut activated_remote_im_source_keys = std::collections::HashSet::<String>::new();
    for (event, should_activate) in events.iter().zip(event_activate_flags.iter().copied()) {
        if !should_activate || !matches!(event.source, ChatEventSource::RemoteIm) {
            continue;
        }
        let Some(sender) = event.sender_info.as_ref() else {
            continue;
        };
        let source = remote_im_activation_source_from_sender(sender);
        let source_key = remote_im_activation_source_key(&source);
        if activated_remote_im_source_keys.insert(source_key) {
            activated_remote_im_sources.push(source);
        }
    }
    activated_remote_im_sources
}

fn remote_im_event_requires_reply_delegate(event: &ChatPendingEvent) -> bool {
    matches!(event.source, ChatEventSource::RemoteIm)
        && event
            .sender_info
            .as_ref()
            .map(|sender| sender.remote_contact_type.trim().eq_ignore_ascii_case("group"))
        .unwrap_or(false)
}

fn remote_im_event_should_observe_after_persistence(
    event: &ChatPendingEvent,
    should_activate: bool,
) -> bool {
    should_activate && remote_im_event_requires_reply_delegate(event)
}

/// 远程消息已经先统一落库，但不能把同一批的多条消息合并成一次秘书判断。
/// 每条事件只看它之前已落库的轻量历史和本事件自身，避免较晚消息倒灌到较早
/// 消息的判断里，也让秘书可以把后续消息准确投递给刚刚启动的委托。
async fn process_persisted_remote_im_events_individually_now(
    state: &AppState,
    conversation_id: &str,
    events: &[ChatPendingEvent],
    event_activate_flags: &[bool],
    persisted_recent_messages_before_flush: &[ChatMessage],
    persisted_batch_messages: &[ChatMessage],
    scheduler_agents: &[AgentProfile],
    must_reply_override: bool,
    inspection: Option<&RemoteImReplyDebounceReady>,
) -> Result<RemoteImReplyDispatchOutcome, String> {
    let mut outcome = RemoteImReplyDispatchOutcome::NoReply;
    for (event, should_consult_secretary) in events.iter().zip(event_activate_flags.iter().copied()) {
        if !should_consult_secretary || !remote_im_event_requires_reply_delegate(event) {
            continue;
        }
        let Some(sender) = event.sender_info.as_ref() else {
            continue;
        };
        let source = remote_im_activation_source_from_sender(sender);
        let Some(contact) = remote_im_resolve_secretary_contact(state, std::slice::from_ref(&source))? else {
            runtime_log_warn(format!(
                "[远程联系人秘书] 跳过，任务=按事件解析联系人，conversation_id={}，event_id={}",
                conversation_id, event.id
            ));
            continue;
        };
        let current_assistant = match remote_im_resolve_contact_assistant_context(state, &contact) {
            Ok(value) => value,
            Err(err) => {
                runtime_log_error(format!(
                    "[远程联系人秘书] 失败，任务=解析助理上下文，conversation_id={}，contact_id={}，event_id={}，error={}",
                    conversation_id, contact.id, event.id, err
                ));
                continue;
            }
        };

        let event_message_ids = event
            .messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let event_message_indexes = persisted_batch_messages
            .iter()
            .enumerate()
            .filter_map(|(index, message)| event_message_ids.contains(message.id.as_str()).then_some(index))
            .collect::<Vec<_>>();
        let Some(first_event_message_index) = event_message_indexes.first().copied() else {
            runtime_log_error(format!(
                "[远程联系人秘书] 失败，任务=定位已落库事件消息，conversation_id={}，contact_id={}，event_id={}",
                conversation_id, contact.id, event.id
            ));
            continue;
        };
        let event_messages = event_message_indexes
            .iter()
            .filter_map(|index| persisted_batch_messages.get(*index).cloned())
            .collect::<Vec<_>>();
        let Some(trigger_message) = event_messages
            .iter()
            .rev()
            .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
            .cloned()
        else {
            runtime_log_warn(format!(
                "[远程联系人秘书] 跳过，任务=事件没有用户消息，conversation_id={}，contact_id={}，event_id={}",
                conversation_id, contact.id, event.id
            ));
            continue;
        };

        let mut history_before_event = persisted_recent_messages_before_flush.to_vec();
        history_before_event.extend_from_slice(&persisted_batch_messages[..first_event_message_index]);
        let secretary_recent_history = remote_im_collect_secretary_recent_messages(
            &history_before_event,
            7,
            &contact,
            scheduler_agents,
            &current_assistant,
        );
        let secretary_current_messages = remote_im_collect_secretary_recent_messages(
            &event_messages,
            event_messages.len(),
            &contact,
            scheduler_agents,
            &current_assistant,
        );
        let work_ledger = build_remote_im_assistant_work_ledger(state, &contact.id, conversation_id)
            .unwrap_or_else(|err| {
                runtime_log_warn(format!(
                    "[助理工作账本] 降级，任务=秘书单事件判断，contact_id={}，error={}",
                    contact.id, err
                ));
                "（无）".to_string()
            });
        let active_delegate_ids =
            remote_im_reply_delegate_active_ids_for_contact(state, &contact.id)?;
        let decision = if must_reply_override {
            RemoteImSecretaryDecision {
                should_reply: true,
                target_delegate_id: active_delegate_ids.first().cloned(),
                reason: "明确点名助理，跳过秘书判断".to_string(),
                model_name: String::new(),
                emit_log: true,
            }
        } else {
            match run_remote_im_secretary_decision(
                state,
                &contact,
                &current_assistant,
                &secretary_recent_history,
                &secretary_current_messages,
                &work_ledger,
                &active_delegate_ids,
            )
            .await
            {
                Ok(mut value) => {
                    if must_reply_override {
                        value.should_reply = true;
                    }
                    value
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[远程联系人秘书] 失败，任务=单事件判断降级为不回复，conversation_id={}，contact_id={}，event_id={}，error={}",
                        conversation_id, contact.id, event.id, err
                    ));
                    RemoteImSecretaryDecision {
                        should_reply: false,
                        target_delegate_id: None,
                        reason: format!("秘书判断失败，已降级为不回复：{err}"),
                        model_name: String::new(),
                        emit_log: true,
                    }
                }
            }
        };
        if decision.emit_log {
            runtime_log_warn(format!(
                "[远程联系人秘书] 完成，任务=单事件判断，conversation_id={}，contact_id={}，event_id={}，should_reply={}，model={}，reason={}",
                conversation_id,
                contact.id,
                event.id,
                decision.should_reply,
                if decision.model_name.trim().is_empty() { "fallback" } else { decision.model_name.as_str() },
                decision.reason
            ));
        }
        if let Some(inspection) = inspection {
            if !remote_im_group_reply_generation_is_current(
                state,
                &inspection.contact_id,
                inspection.generation,
            ) {
                runtime_log_warn(format!(
                    "[群聊巡检] 异步结果过期，contact_id={}，generation={}，stage=secretary_finished",
                    inspection.contact_id, inspection.generation
                ));
                return Ok(RemoteImReplyDispatchOutcome::NoReply);
            }
        }
        if !decision.should_reply {
            continue;
        }
        if remote_im_contact_is_muted(state, &contact.id)? {
            clear_remote_im_debounces_for_contact(state, &contact.id)?;
            runtime_log_info(format!(
                "[远程联系人防抖] 跳过，任务=发起应答，contact_id={}，reason=联系人处于闭嘴状态",
                contact.id
            ));
            continue;
        }

        if let Some(target_delegate_id) = decision.target_delegate_id.as_deref() {
            let policy = inspection.map(|entry| RemoteImGroupReplyDispatchPolicy {
                generation: entry.generation,
                focus: entry.focus,
                max_chars: entry.max_chars,
            });
            match remote_im_reply_delegate_enqueue_guidance(
                state,
                target_delegate_id,
                trigger_message.clone(),
                policy,
            ) {
                Ok(()) => {
                    outcome = RemoteImReplyDispatchOutcome::DispatchStarted;
                    runtime_log_info(format!(
                        "[远程应答委托] 完成，任务=投递单事件引导，conversation_id={}，contact_id={}，delegate_id={}，event_id={}",
                        conversation_id, contact.id, target_delegate_id, event.id
                    ));
                    continue;
                }
                Err(err) => runtime_log_warn(format!(
                    "[远程应答委托] 跳过，任务=目标委托已结束，改为新建委托，conversation_id={}，contact_id={}，delegate_id={}，event_id={}，error={}",
                    conversation_id, contact.id, target_delegate_id, event.id, err
                )),
            }
        }

        let should_apply_dynamic_wake = effective_remote_im_contact_response_strategy(&contact)
            == "smart_judge";
        let patience_seconds = remote_im_channel_behavior_settings_for_contact(state, &contact)
            .patience_seconds;
        let mut force_memory_prompt_snapshot = false;
        if should_apply_dynamic_wake {
            if let Err(err) = remote_im_mark_contact_present_and_schedule_after_entry_compaction(
                state,
                &contact.id,
                conversation_id,
                &trigger_message.id,
                patience_seconds,
                "巡检决定通知远程应答委托",
            ) {
                force_memory_prompt_snapshot = true;
                runtime_log_warn(format!(
                    "[群聊巡检] 在场状态、压缩或计时刷新降级，contact_id={}，error={}",
                    contact.id, err
                ));
            }
        } else if let Err(err) = remote_im_mark_contact_present_and_schedule(
            state,
            &contact.id,
            patience_seconds,
            "巡检决定通知远程应答委托",
        ) {
            runtime_log_warn(format!(
                "[群聊巡检] 在场状态或计时刷新降级，contact_id={}，error={}",
                contact.id, err
            ));
        }
        let dispatch_policy = inspection.map(|entry| RemoteImGroupReplyDispatchPolicy {
            generation: entry.generation,
            focus: entry.focus,
            max_chars: entry.max_chars,
        });
        match spawn_remote_im_reply_delegate(
            state,
            &contact.id,
            conversation_id,
            &trigger_message,
            &ChatSessionInfo {
                agent_id: current_assistant.agent_id.clone(),
            },
            source,
            patience_seconds,
            effective_remote_im_contact_response_strategy(&contact) == "smart_judge",
            force_memory_prompt_snapshot,
            dispatch_policy,
        ) {
            Ok(delegate_id) => {
                outcome = RemoteImReplyDispatchOutcome::DispatchStarted;
                runtime_log_info(format!(
                    "[远程应答委托] 开始，delegate_id={}，conversation_id={}，contact_id={}，trigger_message_id={}，event_id={}",
                    delegate_id, conversation_id, contact.id, trigger_message.id, event.id
                ));
            }
            Err(err) => return Err(format!(
                "创建远程应答委托失败，conversation_id={}，contact_id={}，event_id={}，error={}",
                conversation_id, contact.id, event.id, err
            )),
        }
    }
    Ok(outcome)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImReplyDispatchOutcome {
    NoReply,
    DispatchStarted,
}

fn schedule_remote_im_persisted_event_observe_retry(
    state: &AppState,
    event: ChatPendingEvent,
    attempt: u8,
) {
    const MAX_ATTEMPTS: u8 = 6;
    if attempt >= MAX_ATTEMPTS {
        runtime_log_warn(format!(
            "[群聊巡检] 联系人解析连续失败，消息已正常落库并等待后续入站恢复，event_id={}，attempts={}",
            event.id, attempt
        ));
        return;
    }
    let delay_seconds = 5u64.saturating_mul(1u64 << attempt.min(3));
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
        let Some(sender) = event.sender_info.as_ref() else {
            return;
        };
        let source = remote_im_activation_source_from_sender(sender);
        match remote_im_resolve_secretary_contact(&state, std::slice::from_ref(&source)) {
            Ok(Some(contact)) => observe_remote_im_persisted_event(&state, &contact, &event),
            Ok(None) => runtime_log_warn(format!(
                "[群聊巡检] 联系人已不存在或不再允许巡检，消息保留但停止重试，event_id={}",
                event.id
            )),
            Err(err) => {
                runtime_log_warn(format!(
                    "[群聊巡检] 联系人解析重试失败，批次仍保留，event_id={}，attempt={}，error={}",
                    event.id,
                    attempt.saturating_add(1),
                    err
                ));
                schedule_remote_im_persisted_event_observe_retry(
                    &state,
                    event,
                    attempt.saturating_add(1),
                );
            }
        }
    });
}

async fn process_persisted_remote_im_events_individually(
    state: &AppState,
    _conversation_id: &str,
    events: &[ChatPendingEvent],
    event_activate_flags: &[bool],
    _persisted_recent_messages_before_flush: &[ChatMessage],
    _persisted_batch_messages: &[ChatMessage],
    _scheduler_agents: &[AgentProfile],
) {
    for (event, should_activate) in events.iter().zip(event_activate_flags.iter().copied()) {
        if !remote_im_event_should_observe_after_persistence(event, should_activate) {
            continue;
        }
        let Some(sender) = event.sender_info.as_ref() else {
            continue;
        };
        let source = remote_im_activation_source_from_sender(sender);
        let contact = match remote_im_resolve_secretary_contact(state, std::slice::from_ref(&source)) {
            Ok(Some(contact)) => contact,
            Ok(None) => continue,
            Err(err) => {
                runtime_log_warn(format!(
                    "[群聊巡检] 入站降级，event_id={}，reason=联系人解析失败，error={}",
                    event.id, err
                ));
                schedule_remote_im_persisted_event_observe_retry(state, event.clone(), 0);
                continue;
            }
        };
        observe_remote_im_persisted_event(state, &contact, event);
    }
}

async fn process_remote_im_reply_debounce(
    state: &AppState,
    entry: RemoteImReplyDebounceReady,
) -> Result<RemoteImReplyDispatchOutcome, String> {
    let sender = entry
        .event
        .sender_info
        .as_ref()
        .ok_or_else(|| "防抖消息缺少远程联系人来源".to_string())?;
    let source = remote_im_activation_source_from_sender(sender);
    let contact = remote_im_resolve_secretary_contact(state, std::slice::from_ref(&source))?
        .ok_or_else(|| "防抖消息无法解析远程联系人".to_string())?;
    if remote_im_contact_is_muted(state, &contact.id)? {
        clear_remote_im_debounces_for_contact(state, &contact.id)?;
        runtime_log_info(format!(
            "[远程联系人防抖] 跳过，任务=定时触发，contact_id={}，reason=联系人处于闭嘴状态",
            contact.id
        ));
        return Ok(RemoteImReplyDispatchOutcome::NoReply);
    }
    let conversation_id = entry.event.conversation_id.clone();
    let active_delegate_ids =
        remote_im_reply_delegate_active_ids_for_contact(state, &contact.id)?;
    let mention_only_read = entry.path == RemoteImReplyInspectionPath::Mention
        && active_delegate_ids.is_empty();
    let (history, batch) = {
        let state_for_blocking = state.clone();
        let conversation_id_for_blocking = conversation_id.clone();
        let start_message_id = entry.start_message_id.clone();
        let end_message_id = entry.end_message_id.clone();
        tokio::task::spawn_blocking(move || {
            if mention_only_read {
                // 未来的自己请停手：这里的 batch 会继续交给秘书/远程应答调度，
                // 属于后端生成链路。绝对不能读取 frontend_display_only，
                // 否则工具历史会被展示投影污染后继续进模型/持久化流程。
                let message = conversation_service_v2().get_raw_message_by_id(
                    &state_for_blocking,
                    &conversation_id_for_blocking,
                    &end_message_id,
                )?;
                Ok::<_, String>((Vec::new(), vec![message]))
            } else {
                read_remote_im_debounce_secretary_messages(
                    &state_for_blocking,
                    &conversation_id_for_blocking,
                    &start_message_id,
                    &end_message_id,
                )
            }
        })
        .await
        .map_err(|err| format!("防抖消息读取任务失败：{err}"))?
    }?;
    let mut event = entry.event.clone();
    event.messages = batch.clone();
    let agents = state_read_agents_cached(state).unwrap_or_default();
    process_persisted_remote_im_events_individually_now(
        state,
        &conversation_id,
        &[event],
        &[true],
        &history,
        &batch,
        &agents,
        entry.path == RemoteImReplyInspectionPath::Mention,
        Some(&entry),
    )
    .await
}

fn read_remote_im_debounce_secretary_messages(
    state: &AppState,
    conversation_id: &str,
    start_message_id: &str,
    end_message_id: &str,
) -> Result<(Vec<ChatMessage>, Vec<ChatMessage>), String> {
    const HISTORY_READ_LIMIT: usize = 50;
    const RANGE_PAGE_SIZE: usize = 100;
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    ensure_chat_store_conversation_readable(state, conversation_id, &paths)?;
    let start = message_store::chat_store_read_message_by_id(&paths, start_message_id)?
        .ok_or_else(|| format!("防抖起点消息不存在：{start_message_id}"))?;
    let history = message_store::chat_store_read_messages_before(
        &paths,
        start_message_id,
        HISTORY_READ_LIMIT,
    )?
    .map(|page| page.messages)
    .unwrap_or_default();
    let mut batch = vec![start];
    if start_message_id == end_message_id {
        return Ok((history, batch));
    }
    let mut after_message_id = start_message_id.to_string();
    loop {
        let page = message_store::chat_store_read_messages_after(
            &paths,
            &after_message_id,
            RANGE_PAGE_SIZE,
        )?
        .ok_or_else(|| format!("防抖范围读取失败：after={after_message_id}"))?;
        if page.messages.is_empty() {
            return Err(format!("防抖终点消息不存在：{end_message_id}"));
        }
        let has_more = page.has_more;
        let mut reached_end = false;
        for message in page.messages {
            after_message_id = message.id.clone();
            reached_end = message.id == end_message_id;
            batch.push(message);
            if reached_end {
                break;
            }
        }
        if reached_end {
            return Ok((history, batch));
        }
        if !has_more {
            return Err(format!("防抖终点消息不存在：{end_message_id}"));
        }
    }
}

fn remote_im_source_has_pending_queue_event(
    state: &AppState,
    conversation_id: &str,
    source: &RemoteImActivationSource,
) -> bool {
    let Ok(slots) = lock_conversation_runtime_slots(state) else {
        return false;
    };
    let Some(slot) = slots.get(conversation_id.trim()) else {
        return false;
    };
    let source_key = remote_im_activation_source_key(source);
    slot.pending_queue.iter().any(|event| {
        matches!(event.source, ChatEventSource::RemoteIm)
            && event
                .sender_info
                .as_ref()
                .map(|sender| {
                    remote_im_activation_source_key(&remote_im_activation_source_from_sender(sender))
                        == source_key
                })
                .unwrap_or(false)
    })
}

fn filter_remote_im_follow_up_sources_for_pending_queue(
    state: &AppState,
    conversation_id: &str,
    sources: Vec<RemoteImActivationSource>,
) -> Vec<RemoteImActivationSource> {
    sources
        .into_iter()
        .filter(|source| {
            let has_pending_queue =
                remote_im_source_has_pending_queue_event(state, conversation_id, source);
            if has_pending_queue {
                runtime_log_warn(format!(
                    "[远程联系人状态机] 待办续跑跳过: conversation_id={}，remote_contact_id={}，reason=等待队列消息先写入历史",
                    conversation_id,
                    source.remote_contact_id
                ));
            }
            !has_pending_queue
        })
        .collect()
}
