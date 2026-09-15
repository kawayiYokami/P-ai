fn lock_remote_im_reply_delegate_runtimes(
    state: &AppState,
) -> Result<
    std::sync::MutexGuard<'_, std::collections::HashMap<String, RemoteImReplyDelegateRuntime>>,
    String,
> {
    match state.remote_im_reply_delegate_runtimes.lock() {
        Ok(runtimes) => Ok(runtimes),
        Err(poisoned) => {
            runtime_log_warn(
                "[远程应答委托] 运行时锁中毒，已恢复并继续处理当前业务".to_string(),
            );
            Ok(poisoned.into_inner())
        }
    }
}

fn remote_im_reply_delegate_register(
    state: &AppState,
    contact_id: &str,
    conversation_id: &str,
    trigger_message: &ChatMessage,
    session_info: &ChatSessionInfo,
    force_memory_prompt_snapshot: bool,
    dispatch_policy: Option<RemoteImGroupReplyDispatchPolicy>,
) -> Result<String, String> {
    let trigger_message_id = trigger_message.id.trim();
    if trigger_message_id.is_empty() {
        return Err("远程应答委托无法冻结启动快照：触发消息 ID 为空".to_string());
    }
    let mut prompt_snapshot_messages = if force_memory_prompt_snapshot {
        runtime_log_warn(format!(
            "[远程应答委托] 跳过，任务=读取启动 block，reason=dynamic_wake_persistence_failed，conversation_id={}，message_id={}",
            conversation_id, trigger_message_id
        ));
        vec![trigger_message.clone()]
    } else { match conversation_service_v2()
        .get_current_compaction_segment_messages_through(
            state,
            conversation_id,
            trigger_message_id,
        )
    {
        Ok(messages) => messages,
        Err(err) => {
            runtime_log_error(format!(
                "[远程应答委托] 失败，任务=读取启动 block，改用触发消息内存快照，conversation_id={}，message_id={}，error={}",
                conversation_id, trigger_message_id, err
            ));
            vec![trigger_message.clone()]
        }
    }};
    let trigger_position = prompt_snapshot_messages
        .iter()
        .position(|message| message.id == trigger_message_id);
    if let Some(trigger_position) = trigger_position {
        // 同批后续事件已经先落库时，快照仍必须止于本委托的触发消息。
        prompt_snapshot_messages.truncate(trigger_position.saturating_add(1));
    } else if prompt_snapshot_messages.len() != 1
        || prompt_snapshot_messages.first().map(|message| message.id.as_str())
            != Some(trigger_message_id)
    {
        runtime_log_error(format!(
            "[远程应答委托] 失败，任务=触发消息不在启动 block，改用触发消息内存快照，conversation_id={}，message_id={}",
            conversation_id, trigger_message_id
        ));
        prompt_snapshot_messages = vec![trigger_message.clone()];
    }
    let root_meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    let delegate = delegate_store_create_delegate(
        &state.data_path,
        &DelegateCreateInput {
            kind: "remote_im_reply".to_string(),
            conversation_id: conversation_id.to_string(),
            parent_delegate_id: None,
            source_agent_id: session_info.agent_id.clone(),
            target_agent_id: session_info.agent_id.clone(),
            title: format!("远程应答 · {}", contact_id),
            why: "远程联系人消息触发应答".to_string(),
            goal: "根据冻结上下文回复远程联系人".to_string(),
            todo: "生成并发送远程应答".to_string(),
            notify_assistant_when_done: false,
            call_stack: Vec::new(),
        },
    )?;
    let delegate_id = delegate.delegate_id.clone();
    if let Err(err) = delegate_runtime_thread_create(
        state,
        &delegate,
        root_meta.preferred_api_config_id.as_deref().unwrap_or_default(),
        None,
        None,
    ) {
        let _ = delegate_store_update_status(&state.data_path, &delegate_id, DELEGATE_STATUS_FAILED);
        return Err(format!("创建远程应答委托会话失败: {err}"));
    }
    let system_reminder = build_remote_im_reply_delegate_system_reminder(
        state,
        contact_id,
        conversation_id,
        trigger_message,
        &session_info.agent_id,
    );
    let snapshot_trigger_message = prompt_snapshot_messages
        .iter_mut()
        .find(|message| message.id == trigger_message_id)
        .ok_or_else(|| "远程应答委托无法注入系统提醒：冻结上文缺少触发消息".to_string())?;
    remote_im_reply_delegate_prepend_system_reminder(snapshot_trigger_message, system_reminder);
    if let Some(policy) = dispatch_policy {
        remote_im_reply_delegate_prepend_system_reminder(
            snapshot_trigger_message,
            build_remote_im_group_reply_length_reminder(policy.focus, policy.max_chars),
        );
    }
    let runtime = RemoteImReplyDelegateRuntime {
        delegate_id: delegate_id.clone(),
        contact_id: contact_id.to_string(),
        conversation_id: conversation_id.to_string(),
        trigger_message_id: trigger_message_id.to_string(),
        started_at: now_iso(),
        prompt_snapshot_messages,
        guidance_messages: std::collections::VecDeque::new(),
        consumed_guidance_messages: Vec::new(),
        cancelled: false,
        terminal: false,
        session_agent_id: session_info.agent_id.clone(),
        inspection_generation: dispatch_policy.map(|policy| policy.generation),
        group_reply_focus: dispatch_policy.map(|policy| policy.focus).unwrap_or(false),
        group_reply_max_chars: dispatch_policy.map(|policy| policy.max_chars),
    };
    lock_remote_im_reply_delegate_runtimes(state)?.insert(delegate_id.clone(), runtime);
    if let Err(err) = remote_im_reply_delegate_mirror_internal_messages(
        state,
        &delegate_id,
        "frozen_snapshot",
        &remote_im_reply_delegate_prompt_messages(state, &delegate_id)?,
    ) {
        runtime_log_warn(format!(
            "[远程应答委托] 镜像冻结快照失败，已降级继续，delegate_id={}，error={}",
            delegate_id, err
        ));
    }
    Ok(delegate_id)
}

fn remote_im_reply_delegate_group_policy(
    state: &AppState,
    delegate_id: &str,
) -> Option<(String, RemoteImGroupReplyDispatchPolicy)> {
    lock_remote_im_reply_delegate_runtimes(state)
        .ok()
        .and_then(|runtimes| {
            let runtime = runtimes.get(delegate_id)?;
            Some((
                runtime.contact_id.clone(),
                RemoteImGroupReplyDispatchPolicy {
                    generation: runtime.inspection_generation?,
                    focus: runtime.group_reply_focus,
                    max_chars: runtime.group_reply_max_chars?,
                },
            ))
        })
}

fn remote_im_reply_delegate_prepend_system_reminder(
    trigger_message: &mut ChatMessage,
    system_reminder: String,
) {
    trigger_message.extra_text_blocks.insert(0, system_reminder);
}

fn build_remote_im_reply_delegate_system_reminder(
    state: &AppState,
    contact_id: &str,
    conversation_id: &str,
    trigger_message: &ChatMessage,
    agent_id: &str,
) -> String {
    const PROFILE_MEMORY_LIMIT: usize = 12;
    let profile = remote_im_message_canonical_user_id(trigger_message)
        .and_then(|user_id| {
            let agents = state_read_agents_cached(state).ok()?;
            let agent = agents.iter().find(|agent| agent.id == agent_id)?;
            match build_transient_user_profile_snapshot_block_for_user(
                &state.data_path,
                agent,
                &user_id,
                "",
                PROFILE_MEMORY_LIMIT,
            ) {
                Ok(block) => block,
                Err(err) => {
                    runtime_log_error(format!(
                        "[用户画像] 失败，任务=冻结远程应答系统提醒，contact_id={}，user_id={}，error={}",
                        contact_id, user_id, err
                    ));
                    None
                }
            }
        })
        .unwrap_or_else(|| "（暂无）".to_string());
    let processing_messages = lock_remote_im_reply_delegate_runtimes(state)
        .map(|runtimes| {
            runtimes
                .values()
                .filter(|runtime| {
                    runtime.conversation_id == conversation_id
                        && !runtime.cancelled
                        && !runtime.terminal
                })
                .filter_map(|runtime| {
                    let trigger_message = runtime
                        .prompt_snapshot_messages
                        .iter()
                        .find(|message| message.id == runtime.trigger_message_id)?;
                    let text = Some(render_message_content_for_model(trigger_message))
                        .map(|text| remote_im_secretary_truncate_text(&text, 100))
                        .filter(|text| !text.trim().is_empty())?;
                    let speaker = remote_im_reply_delegate_processing_message_speaker(
                        trigger_message,
                        contact_id,
                    );
                    Some(format!("- [{speaker}]：{text}"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut reminder = format!("[系统提醒]\n当前联系人画像：\n{profile}");
    if let Some(processing_block) =
        build_remote_im_reply_delegate_processing_reminder(&processing_messages)
    {
        reminder.push_str("\n\n");
        reminder.push_str(&processing_block);
    }
    reminder
}

fn remote_im_reply_delegate_processing_message_speaker(
    message: &ChatMessage,
    fallback: &str,
) -> String {
    remote_im_origin_from_message(message)
        .and_then(|origin| {
            remote_im_origin_string(origin, "sender_name")
                .or_else(|| remote_im_origin_string(origin, "contact_name"))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn build_remote_im_reply_delegate_processing_reminder(
    processing_messages: &[String],
) -> Option<String> {
    if processing_messages.is_empty() {
        return None;
    }
    Some(format!(
        "[以下消息已经在委托处理中，请不要重复处理]\n\n正在委托子代理处理消息：\n{}\n\n[以上消息及其之前的消息，以及与该消息相关的话题和焦点，都不要再次回应。请你假装你正在忙于工作，忙里偷闲回答，而不是暴露内部机制。]\n\n如果用户继续询问这件事，只需简短回复仍在处理中，不要重新回答这件事。\n如果用户提出完全无关的新问题，可以正常回答。\n如果无法判断是否完全无关，一律认为有关。\n请你只关注相关被处理消息之后的内容，之前的内容仅作参考，绝对禁止回应。\n\n对用户必须始终表现为同一个正在忙于处理事务的人。",
        processing_messages.join("\n")
    ))
}

fn build_remote_im_assistant_work_ledger(
    state: &AppState,
    contact_id: &str,
    conversation_id: &str,
) -> Result<String, String> {
    let mut runtimes = lock_remote_im_reply_delegate_runtimes(state)?
        .values()
        .filter(|runtime| {
            runtime.contact_id == contact_id
                && runtime.conversation_id == conversation_id
                && !runtime.cancelled
                && !runtime.terminal
        })
        .cloned()
        .collect::<Vec<_>>();
    runtimes.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    let lines = runtimes
        .into_iter()
        .map(|runtime| {
            let task = runtime
                .prompt_snapshot_messages
                .iter()
                .find(|message| message.id == runtime.trigger_message_id)
                .map(render_message_content_for_model)
                .map(|text| remote_im_secretary_truncate_text(&text, 100))
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| "远程联系人消息触发应答".to_string());
            format!(
                "- [运行中] 委托 ID：{}；任务：\"{}\"；开始：{}",
                runtime.delegate_id, task, runtime.started_at
            )
        })
        .collect::<Vec<_>>();
    Ok(if lines.is_empty() {
        "（无）".to_string()
    } else {
        lines.join("\n")
    })
}

fn remote_im_reply_delegate_prompt_messages(
    state: &AppState,
    delegate_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
    let runtime = runtimes
        .get(delegate_id)
        .ok_or_else(|| format!("远程应答委托不存在，delegate_id={delegate_id}"))?;
    if runtime.cancelled || runtime.terminal {
        return Err(format!("远程应答委托已结束，delegate_id={delegate_id}"));
    }
    let mut messages = runtime.prompt_snapshot_messages.clone();
    messages.extend(runtime.consumed_guidance_messages.iter().cloned());
    Ok(messages)
}

fn remote_im_reply_delegate_is_active(state: &AppState, delegate_id: &str) -> bool {
    lock_remote_im_reply_delegate_runtimes(state)
        .ok()
        .and_then(|runtimes| runtimes.get(delegate_id).cloned())
        .map(|runtime| !runtime.cancelled && !runtime.terminal)
        .unwrap_or(false)
}

fn remote_im_reply_delegate_enqueue_guidance(
    state: &AppState,
    delegate_id: &str,
    mut message: ChatMessage,
    policy: Option<RemoteImGroupReplyDispatchPolicy>,
) -> Result<(), String> {
    let runtime_snapshot = {
        let runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
        let runtime = runtimes
            .get(delegate_id)
            .ok_or_else(|| format!("远程应答委托不存在，delegate_id={delegate_id}"))?;
        if runtime.cancelled || runtime.terminal {
            return Err(format!("远程应答委托已结束，delegate_id={delegate_id}"));
        }
        (
            runtime.contact_id.clone(),
            runtime.conversation_id.clone(),
            runtime.session_agent_id.clone(),
        )
    };
    let system_reminder = build_remote_im_reply_delegate_system_reminder(
        state,
        &runtime_snapshot.0,
        &runtime_snapshot.1,
        &message,
        &runtime_snapshot.2,
    );
    remote_im_reply_delegate_prepend_system_reminder(&mut message, system_reminder);
    {
        let mut runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
        let runtime = runtimes
            .get_mut(delegate_id)
            .ok_or_else(|| format!("远程应答委托不存在，delegate_id={delegate_id}"))?;
        if runtime.cancelled || runtime.terminal {
            return Err(format!("远程应答委托已结束，delegate_id={delegate_id}"));
        }
        if let Some(policy) = policy {
            runtime.inspection_generation = Some(policy.generation);
            runtime.group_reply_focus = policy.focus;
            runtime.group_reply_max_chars = Some(policy.max_chars);
        }
        if let Some(max_chars) = runtime.group_reply_max_chars {
            remote_im_reply_delegate_prepend_system_reminder(
                &mut message,
                build_remote_im_group_reply_length_reminder(
                    runtime.group_reply_focus,
                    max_chars,
                ),
            );
        }
        runtime.guidance_messages.push_back(message.clone());
    }
    if let Err(err) = remote_im_reply_delegate_mirror_internal_messages(
        state,
        delegate_id,
        "guidance",
        &[message],
    ) {
        runtime_log_warn(format!(
            "[远程应答委托] 失败，任务=镜像秘书引导，delegate_id={}，error={}",
            delegate_id, err
        ));
    }
    Ok(())
}

/// 在同一把锁内消费引导，或在确认队列为空时注销委托。
/// 这样秘书不会在“最后一次读空”和“删除运行态”之间塞入一条永远不会被消费的消息。
fn remote_im_reply_delegate_take_guidance_or_finish(
    state: &AppState,
    delegate_id: &str,
) -> Result<RemoteImReplyDelegateNext, String> {
    let mut runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
    let completed_runtime = {
        let runtime = runtimes
            .get_mut(delegate_id)
            .ok_or_else(|| format!("远程应答委托不存在，delegate_id={delegate_id}"))?;
        if runtime.cancelled || runtime.terminal {
            return Ok(RemoteImReplyDelegateNext::Ended);
        }
        if runtime.guidance_messages.is_empty() {
            runtime.terminal = true;
            Some(runtime.clone())
        } else {
            let messages = runtime.guidance_messages.drain(..).collect::<Vec<_>>();
            runtime.consumed_guidance_messages.extend(messages.iter().cloned());
            return Ok(RemoteImReplyDelegateNext::Guidance(messages));
        }
    };
    if let Some(runtime) = completed_runtime {
        runtimes.remove(delegate_id);
        Ok(RemoteImReplyDelegateNext::Completed(runtime))
    } else {
        Ok(RemoteImReplyDelegateNext::Ended)
    }
}

enum RemoteImReplyDelegateNext {
    Guidance(Vec<ChatMessage>),
    Completed(RemoteImReplyDelegateRuntime),
    Ended,
}

fn remote_im_reply_delegate_active_ids_for_contact(
    state: &AppState,
    contact_id: &str,
) -> Result<Vec<String>, String> {
    let runtimes = lock_remote_im_reply_delegate_runtimes(state)?
        .values()
        .filter(|runtime| runtime.contact_id == contact_id && !runtime.cancelled && !runtime.terminal)
        .cloned()
        .collect::<Vec<_>>();
    for runtime in &runtimes {
        runtime_log_debug(format!(
            "[远程应答委托] 活跃快照，delegate_id={}，conversation_id={}，trigger_message_id={}，started_at={}",
            runtime.delegate_id,
            runtime.conversation_id,
            runtime.trigger_message_id,
            runtime.started_at
        ));
    }
    Ok(runtimes
        .into_iter()
        .map(|runtime| runtime.delegate_id)
        .collect())
}

fn abort_remote_im_reply_delegates_for_contact(
    state: &AppState,
    contact_id: &str,
    reason: &str,
) -> Result<usize, String> {
    let delegate_ids = match remote_im_reply_delegate_active_ids_for_contact(state, contact_id) {
        Ok(delegate_ids) => delegate_ids,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程应答委托] 闭嘴中止快照读取降级，contact_id={}，error={}",
                contact_id, err
            ));
            return Ok(0);
        }
    };
    let mut aborted = 0usize;
    for delegate_id in delegate_ids {
        match abort_remote_im_reply_delegate(state, &delegate_id, reason) {
            Ok(true) => aborted += 1,
            Ok(false) => {}
            Err(err) => {
                // runtime 可能已从活跃表移除，但归档/状态回写失败；不能因此中断其余委托的中止。
                runtime_log_warn(format!(
                    "[远程应答委托] 降级，任务=闭嘴中止，delegate_id={}，error={}",
                    delegate_id, err
                ));
                if !remote_im_reply_delegate_is_active(state, &delegate_id) {
                    aborted += 1;
                }
            }
        }
    }
    Ok(aborted)
}

fn remote_im_reply_delegate_finish(
    state: &AppState,
    delegate_id: &str,
    status: &str,
    reason: &str,
) -> Result<bool, String> {
    let runtime = {
        let mut runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
        let Some(runtime) = runtimes.get_mut(delegate_id) else {
            return Ok(false);
        };
        if runtime.terminal {
            return Ok(false);
        }
        runtime.terminal = true;
        let runtime = runtime.clone();
        runtimes.remove(delegate_id);
        runtime
    };
    remote_im_reply_delegate_finalize(state, runtime, status, reason)?;
    Ok(true)
}

fn remote_im_reply_delegate_finalize(
    state: &AppState,
    runtime: RemoteImReplyDelegateRuntime,
    status: &str,
    reason: &str,
) -> Result<(), String> {
    let archived_at = now_iso();
    if let Err(err) = remote_im_reply_delegate_mirror_message(
        state,
        &runtime.delegate_id,
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "system".to_string(),
            created_at: archived_at.clone(),
            speaker_agent_id: Some(runtime.session_agent_id.clone()),
            parts: Vec::new(),
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "remote_im_work_ledger_terminal_reason": reason,
                "remote_im_work_ledger_terminal_status": status,
            })),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        },
        Some("remote_im_work_ledger_terminal"),
    ) {
        runtime_log_warn(format!(
            "[助理工作账本] 降级，任务=记录应答终态，delegate_id={}，error={}",
            runtime.delegate_id, err
        ));
    }
    if let Err(err) = delegate_runtime_thread_archive(state, &runtime.delegate_id, &archived_at) {
        runtime_log_warn(format!(
            "[远程应答委托] 线程归档降级，delegate_id={}，error={}",
            runtime.delegate_id, err
        ));
    }
    if let Err(err) = delegate_store_update_status(&state.data_path, &runtime.delegate_id, status) {
        runtime_log_warn(format!(
            "[远程应答委托] 状态落盘降级，delegate_id={}，status={}，error={}",
            runtime.delegate_id, status, err
        ));
    }
    remote_im_request_24h_maintenance_for_conversation(state.clone(), &runtime.conversation_id);
    emit_conversation_delegate_status_updated(
        state,
        &runtime.conversation_id,
        &runtime.delegate_id,
        status,
    );
    runtime_log_info(format!(
        "[远程应答委托] 完成，任务=终结，delegate_id={}，status={}，reason={}",
        runtime.delegate_id, status, reason
    ));
    if let Err(err) = remote_im_reschedule_presence_timeout_after_delegate(
        state,
        &runtime.contact_id,
    ) {
        runtime_log_warn(format!(
            "[远程联系人在场] 降级，任务=应答委托结束后重新计时，contact_id={}，error={}",
            runtime.contact_id, err
        ));
    }
    Ok(())
}

fn remote_im_reschedule_presence_timeout_after_delegate(
    state: &AppState,
    contact_id: &str,
) -> Result<(), String> {
    if remote_im_contact_is_away(state, contact_id)? {
        return Ok(());
    }
    let contact = state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("远程联系人不存在：{contact_id}"))?;
    remote_im_schedule_presence_timeout(
        state,
        contact_id,
        remote_im_channel_behavior_settings_for_contact(state, &contact).patience_seconds,
    )
}

fn abort_remote_im_reply_delegate(
    state: &AppState,
    delegate_id: &str,
    reason: &str,
) -> Result<bool, String> {
    let runtime = {
        let mut runtimes = lock_remote_im_reply_delegate_runtimes(state)?;
        let Some(runtime) = runtimes.get_mut(delegate_id) else {
            return Ok(false);
        };
        if runtime.terminal {
            return Ok(false);
        }
        runtime.cancelled = true;
        runtime.terminal = true;
        let runtime = runtime.clone();
        runtimes.remove(delegate_id);
        runtime
    };
    let chat_key = format!("remote-im-reply-delegate::{delegate_id}");
    let aborted_chat = match state.inflight_chat_abort_handles.lock() {
        Ok(mut inflight) => {
            if let Some(handle) = inflight.remove(&chat_key) {
                handle.abort();
                true
            } else {
                false
            }
        }
        Err(poisoned) => {
            runtime_log_warn(format!(
                "[远程应答委托] 聊天取消句柄锁中毒，已恢复，delegate_id={}",
                delegate_id
            ));
            let mut inflight = poisoned.into_inner();
            if let Some(handle) = inflight.remove(&chat_key) {
                handle.abort();
                true
            } else {
                false
            }
        }
    };
    let tool_key = format!(
        "{}::{}::remote_reply_delegate:{}",
        runtime.session_agent_id, runtime.conversation_id, delegate_id
    );
    let aborted_tool = match abort_inflight_tool_abort_handle(state, &tool_key) {
        Ok(aborted) => aborted,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程应答委托] 工具取消降级，delegate_id={}，error={}",
                delegate_id, err
            ));
            false
        }
    };
    if let Err(err) = remote_im_reply_delegate_finalize(
        state,
        runtime,
        DELEGATE_STATUS_FAILED,
        reason,
    ) {
        runtime_log_warn(format!(
            "[远程应答委托] 终态归档降级，delegate_id={}，error={}",
            delegate_id, err
        ));
    }
    runtime_log_info(format!(
        "[远程应答委托] 完成，任务=取消，delegate_id={}，aborted_chat={}，aborted_tool={}，reason={}",
        delegate_id, aborted_chat, aborted_tool, reason
    ));
    Ok(true)
}

fn remote_im_reply_delegate_mirror_message(
    state: &AppState,
    delegate_id: &str,
    mut message: ChatMessage,
    internal_kind: Option<&str>,
) -> Result<(), String> {
    if let Some(kind) = internal_kind {
        let mut meta = message.provider_meta.take().unwrap_or_else(|| serde_json::json!({}));
        if !meta.is_object() {
            meta = serde_json::json!({});
        }
        if let Some(object) = meta.as_object_mut() {
            object.insert("remote_im_delegate_internal".to_string(), serde_json::json!(true));
            object.insert("remote_im_delegate_internal_kind".to_string(), serde_json::json!(kind));
        }
        message.provider_meta = Some(meta);
    }
    delegate_runtime_thread_conversation_append_if_absent(state, delegate_id, message).map(|_| ())
}

fn remote_im_reply_delegate_mirror_internal_messages(
    state: &AppState,
    delegate_id: &str,
    kind: &str,
    messages: &[ChatMessage],
) -> Result<(), String> {
    for message in messages {
        let mut mirrored = message.clone();
        mirrored.id = format!("remote-im-internal-{}-{}-{}", delegate_id, kind, message.id);
        remote_im_reply_delegate_mirror_message(state, delegate_id, mirrored, Some(kind))?;
    }
    Ok(())
}

fn spawn_remote_im_reply_delegate(
    state: &AppState,
    contact_id: &str,
    conversation_id: &str,
    trigger_message: &ChatMessage,
    session_info: &ChatSessionInfo,
    source: RemoteImActivationSource,
    patience_seconds: u64,
    dynamic_boundary: bool,
    force_memory_prompt_snapshot: bool,
    dispatch_policy: Option<RemoteImGroupReplyDispatchPolicy>,
) -> Result<String, String> {
    let delegate_id = remote_im_reply_delegate_register(
        state,
        contact_id,
        conversation_id,
        trigger_message,
        session_info,
        force_memory_prompt_snapshot,
        dispatch_policy,
    )?;
    let state_clone = state.clone();
    let delegate_id_for_task = delegate_id.clone();
    let conversation_id = conversation_id.to_string();
    let trigger_message_id = trigger_message.id.clone();
    let session_info = session_info.clone();
    let contact_id_for_task = contact_id.to_string();
    tauri::async_runtime::spawn(async move {
        let permit = match state_clone
            .remote_im_reply_delegate_semaphore
            .clone()
            .acquire_owned()
            .await
        {
            Ok(value) => value,
            Err(_) => {
                runtime_log_error(format!(
                    "[远程应答委托] 失败，任务=获取并发槽，delegate_id={}",
                    delegate_id_for_task
                ));
                let _ = remote_im_reply_delegate_finish(
                    &state_clone,
                    &delegate_id_for_task,
                    DELEGATE_STATUS_FAILED,
                    "获取远程应答并发槽失败",
                );
                return;
            }
        };
        if !remote_im_reply_delegate_is_active(&state_clone, &delegate_id_for_task) {
            drop(permit);
            return;
        }
        let channel: tauri::ipc::Channel<AssistantDeltaEvent> =
            tauri::ipc::Channel::new(|_| Ok(()));
        let mut terminal_status = DELEGATE_STATUS_COMPLETED;
        let mut terminal_reason = "远程应答完成";
        loop {
            let prompt_snapshot_messages = match remote_im_reply_delegate_prompt_messages(
                &state_clone,
                &delegate_id_for_task,
            ) {
                Ok(messages) => messages,
                Err(err) => {
                    runtime_log_error(format!(
                        "[远程应答委托] 失败，任务=读取私有提示词快照，delegate_id={}，error={}",
                        delegate_id_for_task, err
                    ));
                    terminal_status = DELEGATE_STATUS_FAILED;
                    terminal_reason = "读取远程应答上下文失败";
                    break;
                }
            };
            let iteration_group_dispatch =
                remote_im_reply_delegate_group_policy(&state_clone, &delegate_id_for_task);
            let request = SendChatRequest {
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
                session: Some(SessionSelector {
                    api_config_id: None,
                    agent_id: session_info.agent_id.clone(),
                    conversation_id: Some(conversation_id.clone()),
                }),
                speaker_agent_id: None,
                trace_id: Some(format!("remote-reply-{}", delegate_id_for_task)),
                assistant_message_id: Some(Uuid::new_v4().to_string()),
                oldest_queue_created_at: None,
                remote_im_activation_sources: vec![source.clone()],
                runtime_context: Some(RuntimeContext {
                    event_source: Some("remote_im_reply_delegate".to_string()),
                    dispatch_reason: Some("remote_im_reply_delegate".to_string()),
                    bound_remote_im_activation_source: Some(source.clone()),
                    remote_im_reply_delegate_id: Some(delegate_id_for_task.clone()),
                    remote_im_reply_trigger_message_id: Some(trigger_message_id.to_string()),
                    remote_im_reply_prompt_snapshot_messages: Some(prompt_snapshot_messages),
                    remote_im_dynamic_boundary: dynamic_boundary,
                    remote_im_defer_auto_send: true,
                    ..RuntimeContext::default()
                }),
                trigger_only: true,
            };
            let send_result = match send_chat_message_inner(request, &state_clone, &channel).await {
                Ok(result) => {
                    let _ = remote_im_mark_contact_present_and_schedule(
                        &state_clone,
                        &contact_id_for_task,
                        patience_seconds,
                        "远程应答委托已产生模型回答",
                    );
                    runtime_log_info(format!(
                        "[远程应答委托] 完成一轮，delegate_id={}，conversation_id={}",
                        delegate_id_for_task, conversation_id
                    ));
                    result
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[远程应答委托] 失败，delegate_id={}，conversation_id={}，error={}",
                        delegate_id_for_task, conversation_id, err
                    ));
                    terminal_status = DELEGATE_STATUS_FAILED;
                    terminal_reason = "远程应答模型执行失败";
                    break;
                }
            };
            match remote_im_reply_delegate_take_guidance_or_finish(&state_clone, &delegate_id_for_task) {
                Ok(RemoteImReplyDelegateNext::Ended) => break,
                Ok(RemoteImReplyDelegateNext::Completed(runtime)) => {
                    let assistant_message_id = send_result
                        .assistant_message
                        .as_ref()
                        .map(|message| message.id.clone());
                    spawn_remote_im_auto_send_contact_assistant_reply(
                        state_clone.clone(),
                        source.clone(),
                        conversation_id.clone(),
                        send_result.final_response_text.clone(),
                        send_result.assistant_message,
                        assistant_message_id,
                        iteration_group_dispatch,
                    );
                    if let Err(err) = remote_im_reply_delegate_finalize(
                        &state_clone,
                        runtime,
                        DELEGATE_STATUS_COMPLETED,
                        "远程应答完成",
                    ) {
                        runtime_log_warn(format!(
                            "[远程应答委托] 失败，任务=终结，delegate_id={}，error={}",
                            delegate_id_for_task, err
                        ));
                    }
                    break;
                }
                Ok(RemoteImReplyDelegateNext::Guidance(messages)) => runtime_log_info(format!(
                    "[远程应答委托] 继续，任务=消费引导，delegate_id={}，message_count={}",
                    delegate_id_for_task,
                    messages.len()
                )),
                Err(err) => {
                    runtime_log_warn(format!(
                        "[远程应答委托] 跳过，任务=读取引导，delegate_id={}，error={}",
                        delegate_id_for_task, err
                    ));
                    terminal_status = DELEGATE_STATUS_FAILED;
                    terminal_reason = "读取远程应答引导失败";
                    break;
                }
            }
        }
        drop(permit);
        if let Err(err) = remote_im_reply_delegate_finish(
            &state_clone,
            &delegate_id_for_task,
            terminal_status,
            terminal_reason,
        ) {
            runtime_log_warn(format!(
                "[远程应答委托] 失败，任务=终结，delegate_id={}，error={}",
                delegate_id_for_task, err
            ));
        }
        if terminal_status == DELEGATE_STATUS_FAILED {
            if let Some(policy) = dispatch_policy {
                let state = state_clone.clone();
                let contact_id = contact_id_for_task.clone();
                let reason = terminal_reason.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    remote_im_group_reply_retry_after_dispatch_failure(
                        &state,
                        &contact_id,
                        policy.generation,
                        &reason,
                    )
                })
                .await;
            }
        }
    });
    Ok(delegate_id)
}
