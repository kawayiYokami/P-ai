fn remote_im_mark_contact_present(
    state: &AppState,
    contact_id: &str,
    reason: &str,
) -> Result<(), String> {
    let mut states = lock_remote_im_contact_runtime_states(state)?;
    let runtime = remote_im_contact_runtime_state_mut(&mut states, contact_id);
    runtime.presence_state = RemoteImPresenceState::Present;
    runtime.last_presence_at = Some(now_iso());
    runtime.consecutive_no_reply_count = 0;
    runtime_log_info(format!(
        "[远程联系人在场] 完成，contact_id={}，reason={}",
        contact_id, reason
    ));
    drop(states);
    remote_im_emit_contact_dashboard_snapshot(state, contact_id);
    Ok(())
}

fn remote_im_mark_contact_present_and_schedule(
    state: &AppState,
    contact_id: &str,
    patience_seconds: u64,
    reason: &str,
) -> Result<(), String> {
    remote_im_mark_contact_present(state, contact_id, reason)?;
    remote_im_schedule_presence_timeout(state, contact_id, patience_seconds)
}

fn remote_im_mark_contact_present_and_schedule_after_entry_compaction(
    state: &AppState,
    contact_id: &str,
    conversation_id: &str,
    trigger_message_id: &str,
    patience_seconds: u64,
    reason: &str,
) -> Result<bool, String> {
    remote_im_mark_contact_present(state, contact_id, reason)?;
    runtime_log_info(format!(
        "[远程唤醒压缩] 开始，任务=入场原子压缩，conversation_id={}，contact_id={}，trigger_message_id={}",
        conversation_id, contact_id, trigger_message_id
    ));
    let mut force_memory_prompt_snapshot = false;
    match conversation_service_v2().remote_im_apply_dynamic_wake_compaction(
        state,
        conversation_id,
        trigger_message_id,
        true,
    ) {
        Ok(RemoteImDynamicWakeCompactionOutcome::Applied) => {
            runtime_log_info(format!(
                "[远程唤醒压缩] 完成，任务=入场原子压缩，conversation_id={}，contact_id={}，trigger_message_id={}",
                conversation_id, contact_id, trigger_message_id
            ));
        }
        Ok(RemoteImDynamicWakeCompactionOutcome::SkippedLowFrequency {
            block_message_count,
        }) => {
            runtime_log_info(format!(
                "[远程唤醒压缩] 完成，任务=低频群跳过，conversation_id={}，contact_id={}，trigger_message_id={}，block_message_count={}",
                conversation_id, contact_id, trigger_message_id, block_message_count
            ));
        }
        Err(primary_err) => {
            runtime_log_error(format!(
                "[远程唤醒压缩] 失败，任务=历史摘要，conversation_id={}，contact_id={}，trigger_message_id={}，error={}",
                conversation_id, contact_id, trigger_message_id, primary_err
            ));
            force_memory_prompt_snapshot = true;
            if let Err(fallback_err) = conversation_service_v2()
                .remote_im_apply_dynamic_wake_compaction(
                    state,
                    conversation_id,
                    trigger_message_id,
                    false,
                )
            {
                runtime_log_error(format!(
                    "[远程唤醒压缩] 失败，任务=空摘要降级，conversation_id={}，contact_id={}，trigger_message_id={}，error={}",
                    conversation_id, contact_id, trigger_message_id, fallback_err
                ));
            } else {
                runtime_log_warn(format!(
                    "[远程唤醒压缩] 完成，任务=空摘要降级，conversation_id={}，contact_id={}，trigger_message_id={}",
                    conversation_id, contact_id, trigger_message_id
                ));
                force_memory_prompt_snapshot = false;
            }
        }
    }
    remote_im_schedule_presence_timeout(state, contact_id, patience_seconds)?;
    Ok(force_memory_prompt_snapshot)
}

fn remote_im_contact_is_away(state: &AppState, contact_id: &str) -> Result<bool, String> {
    Ok(lock_remote_im_contact_runtime_states(state)?
        .get(contact_id)
        .map(|runtime| runtime.presence_state == RemoteImPresenceState::Away)
        .unwrap_or(true))
}

fn remote_im_schedule_presence_timeout(
    state: &AppState,
    contact_id: &str,
    patience_seconds: u64,
) -> Result<(), String> {
    let expected_presence_at = lock_remote_im_contact_runtime_states(state)?
        .get(contact_id)
        .and_then(|runtime| runtime.last_presence_at.clone())
        .unwrap_or_else(now_iso);
    let state_clone = state.clone();
    let contact_id = contact_id.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(patience_seconds)).await;
        match remote_im_reply_delegate_active_ids_for_contact(&state_clone, &contact_id) {
            Ok(active_delegate_ids) if !active_delegate_ids.is_empty() => {
                runtime_log_info(format!(
                    "[远程联系人在场] 跳过，任务=耐心超时离场，contact_id={}，原因=仍有活跃应答委托，active_delegate_count={}",
                    contact_id,
                    active_delegate_ids.len()
                ));
                return;
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程联系人在场] 降级，任务=耐心超时离场，contact_id={}，原因=读取活跃应答委托失败，继续执行原离场判断，error={}",
                    contact_id, err
                ));
            }
            _ => {}
        }
        let departed = {
            let Ok(mut states) = lock_remote_im_contact_runtime_states(&state_clone) else {
                return;
            };
            let Some(runtime) = states.get_mut(&contact_id) else {
                return;
            };
            if runtime.presence_state == RemoteImPresenceState::Present
                && runtime.last_presence_at.as_deref() == Some(expected_presence_at.as_str())
            {
                runtime.presence_state = RemoteImPresenceState::Away;
                true
            } else {
                false
            }
        };
        if departed {
            runtime_log_error(format!(
                "[远程联系人在场] 完成，任务=耐心超时离场，contact_id={}，patience_seconds={}",
                contact_id, patience_seconds
            ));
            remote_im_emit_contact_dashboard_snapshot(&state_clone, &contact_id);
            if let Err(err) = spawn_remote_im_departure_reflection_delegate(
                &state_clone,
                &contact_id,
            ) {
                runtime_log_warn(format!(
                    "[群聊离场反思] 跳过，contact_id={}，error={}",
                    contact_id, err
                ));
            }
        }
    });
    Ok(())
}

fn remote_im_departure_reflection_context(
    state: &AppState,
    contact_id: &str,
) -> Result<(RemoteImContact, Conversation, RemoteImConversationAssistantContext), String> {
    let contact = state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("远程联系人不存在：{contact_id}"))?;
    if contact.remote_contact_type.trim() != "group" {
        return Err("仅群聊联系人需要离场反思".to_string());
    }
    let conversation_id = contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "群聊联系人未绑定会话".to_string())?;
    let conversation = conversation_service_v2()
        .get_conversation_prompt_context(state, conversation_id)?;
    if conversation.messages.is_empty() {
        return Err("离场时已有上下文为空".to_string());
    }
    let assistant = remote_im_resolve_contact_assistant_context(state, &contact)?;
    Ok((contact, conversation, assistant))
}

fn spawn_remote_im_departure_reflection_delegate(
    state: &AppState,
    contact_id: &str,
) -> Result<String, String> {
    let (contact, mut context, assistant) =
        remote_im_departure_reflection_context(state, contact_id)?;
    let work_ledger = build_remote_im_assistant_work_ledger(state, contact_id, &context.id)
        .unwrap_or_else(|err| {
            runtime_log_warn(format!(
                "[助理工作账本] 降级，任务=离场反思，contact_id={}，error={}",
                contact_id, err
            ));
            "（无）".to_string()
        });
    context.messages.push(ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "system".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(assistant.agent_id.clone()),
        parts: vec![MessagePart::Text {
            text: format!("[系统提醒]\n助理工作账本：\n{work_ledger}"),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "remote_im_assistant_work_ledger": true,
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    });
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let agent = runtime_agent_by_id(&runtime_snapshot, &assistant.agent_id)
        .ok_or_else(|| format!("负责人格不存在：{}", assistant.agent_id))?;
    let api_config_id = agent_primary_chat_api_config_id(&runtime_snapshot.config, agent)
        .ok_or_else(|| format!("负责人格没有可用模型：{}", assistant.agent_id))?;
    let delegate = delegate_store_create_delegate(
        &state.data_path,
        &remote_im_departure_reflection_delegate_input(&contact, &context, &assistant),
    )?;
    if let Err(err) = delegate_runtime_thread_create(state, &delegate, &api_config_id, None, None) {
        let _ = delegate_store_update_status(
            &state.data_path,
            &delegate.delegate_id,
            DELEGATE_STATUS_FAILED,
        );
        return Err(err);
    }
    if let Err(err) = remote_im_reply_delegate_mirror_internal_messages(
        state,
        &delegate.delegate_id,
        "departure_reflection_context",
        &context.messages,
    ) {
        let _ = finalize_remote_im_departure_reflection_delegate(
            state,
            &delegate,
            DELEGATE_STATUS_FAILED,
            "写入离场反思上下文失败",
        );
        return Err(err);
    }

    let state_clone = state.clone();
    let delegate_for_task = delegate.clone();
    let contact_name = remote_im_secretary_contact_display_name(&contact);
    let (abort_handle, abort_registration) = futures_util::future::AbortHandle::new_pair();
    let thread = delegate_runtime_thread_get(state, &delegate.delegate_id)?
        .ok_or_else(|| "离场反思委托线程创建后丢失".to_string())?;
    let chat_key = delegate_thread_chat_key(&thread);
    state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "无法获取离场反思取消句柄锁".to_string())?
        .insert(chat_key.clone(), abort_handle);
    tauri::async_runtime::spawn(async move {
        let delegate_id = delegate_for_task.delegate_id.clone();
        let result = futures_util::future::Abortable::new(
            run_remote_im_departure_reflection(
                &state_clone,
                &delegate_for_task,
                &context,
                &contact_name,
                &api_config_id,
            ),
            abort_registration,
        )
        .await;
        if let Ok(mut handles) = state_clone.inflight_chat_abort_handles.lock() {
            handles.remove(&chat_key);
        }
        match result {
            Ok(Ok(())) => {
                let _ = finalize_remote_im_departure_reflection_delegate(
                    &state_clone,
                    &delegate_for_task,
                    DELEGATE_STATUS_COMPLETED,
                    "完成",
                );
            }
            Ok(Err(err)) => {
                runtime_log_error(format!(
                    "[群聊离场反思] 失败，delegate_id={}，error={}",
                    delegate_id, err
                ));
                let _ = finalize_remote_im_departure_reflection_delegate(
                    &state_clone,
                    &delegate_for_task,
                    DELEGATE_STATUS_FAILED,
                    &err,
                );
            }
            Err(_) => {
                runtime_log_info(format!(
                    "[群聊离场反思] 打断，delegate_id={}",
                    delegate_id
                ));
            }
        }
    });
    Ok(delegate.delegate_id)
}

fn remote_im_departure_reflection_delegate_input(
    contact: &RemoteImContact,
    context: &Conversation,
    assistant: &RemoteImConversationAssistantContext,
) -> DelegateCreateInput {
    DelegateCreateInput {
        kind: "remote_im_departure_reflection".to_string(),
        conversation_id: context.id.clone(),
        parent_delegate_id: None,
        source_agent_id: assistant.agent_id.clone(),
        target_agent_id: assistant.agent_id.clone(),
        title: format!(
            "群聊离场反思 · {}",
            remote_im_secretary_contact_display_name(contact)
        ),
        why: "远程群聊联系人完成本次在场过程并离场".to_string(),
        goal: "根据离场时已有上下文整理可复用记忆".to_string(),
        todo: "输出归档反思格式 JSON，并只应用记忆结果".to_string(),
        notify_assistant_when_done: false,
        call_stack: Vec::new(),
    }
}

async fn run_remote_im_departure_reflection(
    state: &AppState,
    delegate: &DelegateEntry,
    context: &Conversation,
    contact_name: &str,
    api_config_id: &str,
) -> Result<(), String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let selected_api = runtime_snapshot
        .config
        .api_configs
        .iter()
        .find(|api| api.id == api_config_id)
        .cloned()
        .ok_or_else(|| format!("离场反思模型不存在：{api_config_id}"))?;
    let resolved_api = resolve_api_config(&runtime_snapshot.config, Some(api_config_id))?;
    let agent = runtime_snapshot
        .agents
        .iter()
        .find(|agent| agent.id == delegate.target_agent_id)
        .cloned()
        .ok_or_else(|| format!("离场反思人格不存在：{}", delegate.target_agent_id))?;
    let memories = memory_store_list_memories_visible_for_agent(
        &state.data_path,
        &agent.id,
        agent.private_memory_enabled,
    )?;
    let (draft, warning) = summarize_archive_summary_with_fallback(
        state,
        &resolved_api,
        &selected_api,
        &agent,
        contact_name,
        context,
        &memories,
    )
    .await;
    if let Some(warning) = warning {
        return Err(warning);
    }
    if delegate_runtime_thread_get(state, &delegate.delegate_id)?.is_none() {
        return Err("离场反思委托已被打断".to_string());
    }
    let recall_ids = archive_pipeline_dedup_recall_table(&context.memory_recall_table);
    let report = apply_summary_context_result(&state.data_path, &agent, &recall_ids, &draft)?;
    let normalized_json = serde_json::to_string_pretty(&serde_json::json!({
        "usefulMemoryIds": &draft.useful_memory_ids,
        "memoryActions": &draft.memory_actions,
    }))
        .map_err(|err| format!("序列化离场反思结果失败：{err}"))?;
    remote_im_reply_delegate_mirror_message(
        state,
        &delegate.delegate_id,
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some(agent.id.clone()),
            parts: vec![MessagePart::Text {
                text: normalized_json,
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "remote_im_departure_reflection": true,
                "merged_memories": report.merged_memories,
                "merged_groups": report.merged_groups,
            })),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        },
        Some("departure_reflection_result"),
    )?;
    runtime_log_info(format!(
        "[群聊离场反思] 完成，delegate_id={}，conversation_id={}，memory_actions={}，useful_memory_ids={}，merged_memories={}",
        delegate.delegate_id,
        context.id,
        draft.memory_actions.len(),
        draft.useful_memory_ids.len(),
        report.merged_memories
    ));
    Ok(())
}

fn finalize_remote_im_departure_reflection_delegate(
    state: &AppState,
    delegate: &DelegateEntry,
    status: &str,
    reason: &str,
) -> Result<(), String> {
    if delegate_runtime_thread_get(state, &delegate.delegate_id)?.is_none() {
        return Ok(());
    }
    delegate_runtime_thread_archive(state, &delegate.delegate_id, &now_iso())?;
    delegate_store_update_status(&state.data_path, &delegate.delegate_id, status)?;
    emit_conversation_delegate_status_updated(
        state,
        &delegate.conversation_id,
        &delegate.delegate_id,
        status,
    );
    runtime_log_info(format!(
        "[群聊离场反思] {}，delegate_id={}，reason={}",
        if status == DELEGATE_STATUS_COMPLETED { "完成" } else { "失败" },
        delegate.delegate_id,
        reason
    ));
    Ok(())
}
