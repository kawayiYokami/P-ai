impl ConversationServiceV2 {
    fn commit_scheduler_history_flush(
        &self,
        state: &AppState,
        conversation_id: &str,
        events: &[ChatPendingEvent],
        prepared_batches: Vec<Vec<(ChatMessage, Vec<String>)>>,
        history_flush_time: &str,
        should_seed_summary_context: bool,
        has_existing_messages: bool,
    ) -> Result<SchedulerHistoryFlushCommitResult, String> {
        enum SchedulerHistoryFlushOutcome {
            MissingTarget(String),
            Committed(SchedulerHistoryFlushCommitResult),
        }
        let mut promoted: Option<Conversation> = None;
        let outcome = with_conversation_mutation(
            state,
            conversation_id,
            "commit_scheduler_history_flush",
            || {
                let conversation_meta = match self.get_conversation_meta(state, conversation_id) {
                    Ok(conversation_meta)
                        if self.conversation_meta_is_unarchived_meta_view(&conversation_meta) =>
                    {
                        conversation_meta
                    }
                    _ => {
                        return Ok(SchedulerHistoryFlushOutcome::MissingTarget(format!(
                            "目标会话不存在，conversationId={conversation_id}"
                        )));
                    }
                };
                let was_draft = conversation_meta.is_draft;
                let mut conversation = self.build_conversation_record_from_meta_view(&conversation_meta);
                let remote_im_contacts = state_service_list_remote_im_contacts(state, None)?;
                let mut remote_im_checkpoints = state_service_list_remote_im_contact_checkpoints(state)?;
                let remote_im_runtime_before =
                    serde_json::to_vec(&remote_im_checkpoints).ok();

                let persisted_batch_messages = self.write_scheduler_persisted_message_batch_v2(
                    conversation_id,
                    events,
                    prepared_batches,
                    history_flush_time,
                    should_seed_summary_context,
                    has_existing_messages,
                    conversation_meta.has_context_compaction_message,
                    state,
                    &mut conversation,
                );
                let (event_activate_flags, _activated_contacts) =
                    self.handle_scheduler_remote_im_activations_v2(
                        state,
                        &remote_im_contacts,
                        &mut remote_im_checkpoints,
                        &mut conversation,
                        events,
                        history_flush_time,
                    )?;
                conversation.updated_at = history_flush_time.to_string();
                let (metadata_conversation, (), _) = state_update_conversation_meta_cached_unlocked(
                    state,
                    &conversation.id,
                    |cached| {
                        let mut metadata_snapshot =
                            self.build_conversation_snapshot_from_meta(cached, Vec::new());
                        metadata_snapshot.user_profile_snapshot = conversation.user_profile_snapshot.clone();
                        metadata_snapshot.memory_recall_table = conversation.memory_recall_table.clone();
                        metadata_snapshot.unread_count = conversation.unread_count;
                        metadata_snapshot.updated_at = conversation.updated_at.clone();
                        metadata_snapshot.last_user_at = conversation.last_user_at.clone();
                        metadata_snapshot.last_assistant_at = conversation.last_assistant_at.clone();
                        // 草稿转正：scheduler 批量落盘写入真实消息即清除草稿标记，
                        // 写回存储后后续 emit 的 overview 水位线携带 isDraft=false。
                        // 压缩消息走 persist_compaction_message 独立路径，不会进入本函数。
                        if was_draft {
                            metadata_snapshot.is_draft = false;
                        }
                        cached.apply_metadata_fields_from_conversation(&metadata_snapshot);
                        cached.apply_appended_messages(&persisted_batch_messages);
                        Ok(())
                    },
                )?;
                let metadata_snapshot =
                    self.build_conversation_snapshot_from_meta(&metadata_conversation, Vec::new());
                state_upsert_chat_index_conversation_cached(state, &metadata_snapshot)?;
                self.persist_scheduler_flush_appended_messages_v2(
                    state,
                    &metadata_conversation,
                    &persisted_batch_messages,
                    &remote_im_checkpoints,
                    remote_im_runtime_before,
                )?;
                if was_draft && !metadata_snapshot.is_draft {
                    promoted = Some(metadata_snapshot);
                }
                Ok(SchedulerHistoryFlushOutcome::Committed(
                    SchedulerHistoryFlushCommitResult {
                        persisted_batch_messages,
                        event_activate_flags,
                    },
                ))
            },
        )?;
        // 草稿转正后立即创建下一个备用草稿：继承刚转正会话的人格/模型/workspace。
        // 创建失败不阻断消息发送，下次打开草稿入口时按单例查询兜底重建。
        if let Some(promoted_conversation) = promoted {
            match create_next_draft_conversation_inherited(state, &promoted_conversation) {
                Ok(new_draft_id) => runtime_log_info(format!(
                    "[会话草稿] 完成，任务=scheduler 落盘转正，conversation_id={}，new_draft_conversation_id={}",
                    conversation_id, new_draft_id
                )),
                Err(err) => runtime_log_warn(format!(
                    "[会话草稿] 创建备用草稿失败，等待下次打开时重建，conversation_id={}，error={err}",
                    conversation_id
                )),
            }
        }
        // 水位线：会话状态已变更（消息落盘/草稿转正），在 service 层统一收尾处
        // 调用水位线方法推送，调用方无需也不允许手动推送。
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=scheduler 落盘后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
        match outcome {
            SchedulerHistoryFlushOutcome::MissingTarget(error) => {
                let event_ids = events
                    .iter()
                    .map(|event| event.id.clone())
                    .collect::<Vec<_>>();
                complete_pending_chat_events_with_error(state, &event_ids, &error)?;
                Err(error)
            }
            SchedulerHistoryFlushOutcome::Committed(result) => Ok(result),
        }
    }

    fn write_scheduler_persisted_message_batch_v2(
        &self,
        conversation_id: &str,
        events: &[ChatPendingEvent],
        prepared_batches: Vec<Vec<(ChatMessage, Vec<String>)>>,
        history_flush_time: &str,
        should_seed_summary_context: bool,
        has_existing_messages: bool,
        has_summary_context: bool,
        state: &AppState,
        conversation: &mut Conversation,
    ) -> Vec<ChatMessage> {
        let mut persisted_batch_messages = Vec::<ChatMessage>::new();
        if should_seed_summary_context
            && !has_existing_messages
            && conversation.messages.is_empty()
            && !has_summary_context
            && !conversation_is_delegate(conversation)
        {
            let summary_message = build_initial_summary_context_message(
                Some(&conversation.current_todos),
                None,
            );
            persisted_batch_messages.push(summary_message.clone());
            conversation.messages.push(summary_message);
        }

        for (event, prepared_messages) in events.iter().zip(prepared_batches.into_iter()) {
            self.append_scheduler_prepared_messages_to_conversation_v2(
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

    fn append_scheduler_prepared_messages_to_conversation_v2(
        &self,
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
                runtime_log_debug(format!(
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
                ));
            }
            let persisted_for_event = persisted.clone();
            match persisted.role.trim() {
                "user" => conversation.last_user_at = Some(history_flush_time.to_string()),
                "assistant" => {
                    conversation.last_assistant_at = Some(history_flush_time.to_string())
                }
                _ => {}
            }
            conversation.messages.push(persisted);
            self.increment_conversation_unread_count_if_background(
                state,
                conversation,
                1,
                true,
            );
            persisted_batch_messages.push(persisted_for_event);
        }
    }

    fn handle_scheduler_remote_im_activations_v2(
        &self,
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

    fn persist_scheduler_flush_appended_messages_v2(
        &self,
        state: &AppState,
        conversation_meta: &message_store::ConversationShardMeta,
        appended_messages: &[ChatMessage],
        checkpoints: &[RemoteImContactCheckpoint],
        remote_im_runtime_before: Option<Vec<u8>>,
    ) -> Result<(), String> {
        let remote_im_runtime_changed =
            remote_im_runtime_before != serde_json::to_vec(checkpoints).ok();
        let paths = message_store::message_store_paths(&state.data_path, conversation_meta.id())?;
        let mut ready_meta = message_store::chat_store_read_meta(&paths)?
            .ok_or_else(|| {
                format!(
                    "历史回灌落盘失败：缺少 ready 消息元数据，conversation_id={}",
                    conversation_meta.id()
                )
            })?;
        ready_meta.apply_metadata_fields_from_meta(conversation_meta);
        ready_meta.apply_appended_messages(appended_messages);
        message_store::chat_store_append_messages_from_meta(
            &paths,
            &ready_meta,
            appended_messages,
        )?;
        self.mark_conversation_metadata_cached_persisted(state, conversation_meta.id())?;
        if remote_im_runtime_changed {
            // 只回写本批次实际变更的 checkpoint，避免用旧快照覆盖并发更新的其他联系人
            let before_checkpoints: std::collections::HashMap<String, RemoteImContactCheckpoint> =
                remote_im_runtime_before
                    .as_deref()
                    .and_then(|bytes| {
                        serde_json::from_slice::<Vec<RemoteImContactCheckpoint>>(bytes).ok()
                    })
                    .map(|list| {
                        list.into_iter()
                            .map(|checkpoint| (checkpoint.contact_id.clone(), checkpoint))
                            .collect()
                    })
                    .unwrap_or_default();
            for checkpoint in checkpoints {
                let unchanged = before_checkpoints
                    .get(&checkpoint.contact_id)
                    .map(|before| {
                        serde_json::to_vec(before).ok() == serde_json::to_vec(checkpoint).ok()
                    })
                    .unwrap_or(false);
                if !unchanged {
                    state_service_set_remote_im_contact_checkpoint(state, checkpoint)?;
                }
            }
        }
        Ok(())
    }

}
