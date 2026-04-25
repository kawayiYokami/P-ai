impl ConversationService {
    fn commit_scheduler_history_flush(
        &self,
        state: &AppState,
        conversation_id: &str,
        events: &[ChatPendingEvent],
        prepared_batches: Vec<Vec<(ChatMessage, Vec<String>)>>,
        history_flush_time: &str,
        should_seed_summary_context: bool,
        seeded_profile_snapshot: Option<&str>,
    ) -> Result<SchedulerHistoryFlushCommitResult, String> {
        let _guard = lock_conversation_with_metrics(state, "scheduler_commit")?;
        let mut conversation = match state_read_conversation_cached(state, conversation_id) {
            Ok(conversation) if conversation.summary.trim().is_empty() => conversation,
            _ => {
            let event_ids = events
                .iter()
                .map(|event| event.id.clone())
                .collect::<Vec<_>>();
            complete_pending_chat_events_with_error(
                state,
                &event_ids,
                &format!("目标会话不存在，conversationId={conversation_id}"),
            )?;
            return Err(format!("目标会话不存在，conversationId={conversation_id}"));
            }
        };
        let mut runtime = state_read_runtime_state_cached(state)?;
        let remote_im_runtime_before = serde_json::to_vec(&(
            runtime.remote_im_contacts.clone(),
            runtime.remote_im_contact_checkpoints.clone(),
        ))
        .ok();

        let last_archive_summary = scheduler_last_archive_summary_cached(state)?;
        let persisted_batch_messages = write_persisted_message_batch(
            self,
            state,
            &mut conversation,
            conversation_id,
            events,
            prepared_batches,
            history_flush_time,
            should_seed_summary_context,
            seeded_profile_snapshot,
            last_archive_summary.as_deref(),
        );
        let (event_activate_flags, _activated_contacts) = handle_remote_im_activations(
            state,
            &runtime.remote_im_contacts,
            &mut runtime.remote_im_contact_checkpoints,
            &mut conversation,
            events,
            history_flush_time,
        )?;
        conversation.updated_at = history_flush_time.to_string();
        persist_after_flush(
            self,
            state,
            &conversation,
            &runtime,
            remote_im_runtime_before,
        )?;
        Ok(SchedulerHistoryFlushCommitResult {
            persisted_batch_messages,
            event_activate_flags,
        })
    }
}

fn scheduler_last_archive_summary_cached(state: &AppState) -> Result<Option<String>, String> {
    let chat_index = state_read_chat_index_cached(state)?;
    Ok(chat_index
        .conversations
        .iter()
        .rev()
        .filter(|item| !item.summary.trim().is_empty())
        .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
        .find(|conversation| {
            !conversation_is_delegate(&conversation) && !conversation.summary.trim().is_empty()
        })
        .map(|conversation| conversation.summary))
}

fn write_persisted_message_batch(
    service: &ConversationService,
    state: &AppState,
    conversation: &mut Conversation,
    conversation_id: &str,
    events: &[ChatPendingEvent],
    prepared_batches: Vec<Vec<(ChatMessage, Vec<String>)>>,
    history_flush_time: &str,
    should_seed_summary_context: bool,
    seeded_profile_snapshot: Option<&str>,
    last_archive_summary: Option<&str>,
) -> Vec<ChatMessage> {
    let mut persisted_batch_messages = Vec::<ChatMessage>::new();
    let has_summary_context = conversation
        .messages
        .iter()
        .any(|message| is_context_compaction_message(message, message.role.trim()));
    if should_seed_summary_context
        && !has_summary_context
        && !conversation_is_delegate(conversation)
        && !conversation_is_remote_im_contact(conversation)
    {
        if conversation.user_profile_snapshot.trim().is_empty() {
            if let Some(snapshot) = seeded_profile_snapshot {
                conversation.user_profile_snapshot = snapshot.to_string();
            }
        }
        let summary_message = build_initial_summary_context_message(
            last_archive_summary,
            Some(conversation.user_profile_snapshot.as_str()),
            Some(&conversation.current_todos),
        );
        persisted_batch_messages.push(summary_message.clone());
        conversation.messages.insert(0, summary_message);
    }

    for (event, prepared_messages) in events.iter().zip(prepared_batches.into_iter()) {
        append_prepared_messages_to_conversation(
            service,
            state,
            conversation,
            conversation_id,
            event,
            prepared_messages,
            history_flush_time,
            &mut persisted_batch_messages,
        );
    }
    persisted_batch_messages
}

fn append_prepared_messages_to_conversation(
    service: &ConversationService,
    state: &AppState,
    conversation: &mut Conversation,
    conversation_id: &str,
    event: &ChatPendingEvent,
    prepared_messages: Vec<(ChatMessage, Vec<String>)>,
    history_flush_time: &str,
    persisted_batch_messages: &mut Vec<ChatMessage>,
) {
    for (persisted, recall_ids) in prepared_messages {
        if persisted.role.trim() == "user" && !recall_ids.is_empty() {
            for memory_id in &recall_ids {
                conversation.memory_recall_table.push(memory_id.clone());
            }
            eprintln!(
                "[记忆RAG][出队消息写入] conversation_id={} user_message_id={} agent_id={} retrieved_memory_ids={:?}",
                conversation_id,
                persisted.id,
                event.session_info.agent_id,
                persisted
                    .provider_meta
                    .as_ref()
                    .and_then(|meta| meta.get("retrieved_memory_ids"))
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_default()
            );
        }
        let persisted_for_event = persisted.clone();
        match persisted.role.trim() {
            "user" => conversation.last_user_at = Some(history_flush_time.to_string()),
            "assistant" => conversation.last_assistant_at = Some(history_flush_time.to_string()),
            _ => {}
        }
        conversation.messages.push(persisted);
        service.increment_conversation_unread_count_if_background(state, conversation, 1);
        persisted_batch_messages.push(persisted_for_event);
    }
}

fn handle_remote_im_activations(
    state: &AppState,
    contacts: &[RemoteImContact],
    checkpoints: &mut Vec<RemoteImContactCheckpoint>,
    conversation: &mut Conversation,
    events: &[ChatPendingEvent],
    history_flush_time: &str,
) -> Result<(Vec<bool>, std::collections::HashSet<String>), String> {
    let mut event_activate_flags = Vec::<bool>::with_capacity(events.len());
    let mut activated_contacts_in_batch = std::collections::HashSet::<String>::new();
    for event in events {
        let event_should_activate = if matches!(event.source, ChatEventSource::RemoteIm) {
            remote_im_handle_persisted_event_after_history_flush_runtime(
                state,
                contacts,
                checkpoints,
                conversation,
                event,
                history_flush_time,
                &mut activated_contacts_in_batch,
            )?
        } else {
            event.activate_assistant
        };
        event_activate_flags.push(event_should_activate);
    }
    Ok((event_activate_flags, activated_contacts_in_batch))
}

fn persist_after_flush(
    service: &ConversationService,
    state: &AppState,
    conversation: &Conversation,
    runtime: &RuntimeStateFile,
    remote_im_runtime_before: Option<Vec<u8>>,
) -> Result<(), String> {
    let remote_im_runtime_changed = remote_im_runtime_before
        != serde_json::to_vec(&(
            runtime.remote_im_contacts.clone(),
            runtime.remote_im_contact_checkpoints.clone(),
        ))
        .ok();
    service.persist_conversation_with_chat_index(state, conversation)?;
    if remote_im_runtime_changed {
        state_write_runtime_state_cached(state, runtime)?;
    }
    Ok(())
}
