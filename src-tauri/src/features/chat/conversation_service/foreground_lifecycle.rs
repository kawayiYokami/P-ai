impl ConversationServiceV2 {
    fn create_remote_im_contact_conversation(
        &self,
        state: &AppState,
        title: &str,
        agent_id: &str,
        root_conversation_id: &str,
    ) -> Result<Conversation, String> {
        let normalized_title = title.trim();
        let normalized_agent_id = agent_id.trim();
        let normalized_root_conversation_id = root_conversation_id.trim();
        if normalized_title.is_empty() {
            return Err("title is required.".to_string());
        }
        if normalized_agent_id.is_empty() {
            return Err("agentId is required.".to_string());
        }
        if normalized_root_conversation_id.is_empty() {
            return Err("rootConversationId is required.".to_string());
        }
        let conversation = {
            let _guard = lock_conversation_with_metrics(
                state,
                "conversation_v2_create_remote_im_contact_conversation",
            )?;
            let mut conversation = build_conversation_record(
                "",
                normalized_agent_id,
                normalized_title,
                CONVERSATION_KIND_REMOTE_IM_CONTACT,
                Some(normalized_root_conversation_id.to_string()),
                None,
            );
            conversation.status = "inactive".to_string();
            let summary_message =
                build_initial_summary_context_message(Some(&conversation.current_todos), None);
            conversation.last_user_at = Some(summary_message.created_at.clone());
            conversation.updated_at = summary_message.created_at.clone();
            conversation.messages.push(summary_message);
            conversation
        };
        state_schedule_conversation_persist(state, &conversation)?;
        Ok(conversation)
    }

    fn create_conversation(
        &self,
        state: &AppState,
        input: &CreateUnarchivedConversationInput,
    ) -> Result<CreateUnarchivedConversationMutationResult, String> {
        create_unarchived_conversation_shared(state, input)
    }

    fn switch_active_conversation_snapshot(
        &self,
        state: &AppState,
        input: &SwitchActiveConversationSnapshotInput,
    ) -> Result<SwitchActiveConversationSnapshotMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut app_config = state_read_config_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let assistant_agent_id = assistant_agent_id_downgraded(state);
        let (main_conversation_id, main_conversation_id_readable) =
            match state_service_get_main_conversation_id(state) {
                Ok(value) => (value, true),
                Err(err) => {
                    runtime_log_warn(format!(
                        "[会话切换] 读取主会话 ID 失败，按无主会话降级继续：error={err}"
                    ));
                    (None, false)
                }
            };
        let _effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &agents,
            &assistant_agent_id,
            input.agent_id.as_deref().unwrap_or_default(),
        )?;
        let requested_conversation_id = input
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (target_conversation_meta, target_conversation, created_new_conversation) =
            if let Some(conversation_id) = requested_conversation_id {
                let conversation_meta = self.get_conversation_meta(state, conversation_id)
                    .ok()
                    .filter(|conversation_meta| {
                        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                            && (conversation_meta.visible_in_foreground_lists
                                || conversation_meta.conversation_kind.trim()
                                    == CONVERSATION_KIND_SIDE_CHAT)
                    })
                    .ok_or_else(|| {
                        format!("Requested conversation not found: {conversation_id}")
                    })?;
                (Some(conversation_meta), None, false)
            } else if let Some(conversation_meta) = main_conversation_id
                .as_deref()
                .and_then(|conversation_id| {
                    self.get_conversation_meta(state, conversation_id.trim()).ok()
                })
                .filter(|conversation_meta| {
                    self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                        && conversation_meta.visible_in_foreground_lists
                })
            {
                (Some(conversation_meta), None, false)
            } else if main_conversation_id.as_deref().map(str::trim)
                == Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
            {
                let conversation = build_system_notification_conversation_record();
                (None, Some(conversation), true)
            } else if let Some(conversation_meta) =
                read_latest_visible_foreground_conversation_metadata(state)?
            {
                (Some(conversation_meta), None, false)
            } else {
                let conversation = build_system_notification_conversation_record();
                (None, Some(conversation), true)
            };
        let target_conversation_id = target_conversation_meta
            .as_ref()
            .map(|conversation_meta| conversation_meta.id.to_string())
            .or_else(|| target_conversation.as_ref().map(|conversation| conversation.id.clone()))
            .ok_or_else(|| "Requested conversation not found.".to_string())?;
        let unread_changed = target_conversation_meta
            .as_ref()
            .map(|conversation_meta| conversation_meta.unread_count > 0)
            .unwrap_or(false);
        drop(guard);
        clear_conversation_list_activity_mark(state, &target_conversation_id);
        if unread_changed && !created_new_conversation {
            state_update_conversation_metadata_cached(
                state,
                &target_conversation_id,
                |conversation| {
                    conversation.unread_count = 0;
                    Ok(())
                },
            )?;
        }
        if created_new_conversation {
            let conversation = target_conversation
                .as_ref()
                .ok_or_else(|| "Requested conversation not found.".to_string())?;
            state_schedule_conversation_persist(state, conversation)?;
        }
        if target_conversation_meta
            .as_ref()
            .map(|conversation_meta| self.conversation_meta_is_system_notification_meta_view(conversation_meta))
            .or_else(|| {
                target_conversation
                    .as_ref()
                    .map(conversation_is_system_notification)
            })
            .unwrap_or(false)
            && main_conversation_id_readable
            && main_conversation_id.as_deref().map(str::trim)
                != Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        {
            state_service_set_main_conversation_id(state, Some(SYSTEM_NOTIFICATION_CONVERSATION_ID))?;
        }
        let snapshot = if let Some(conversation_meta) = target_conversation_meta.as_ref() {
            build_foreground_conversation_snapshot_from_meta_view(
                state,
                conversation_meta,
                DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT,
            )?
        } else {
            build_foreground_conversation_snapshot_from_conversation(
                state,
                target_conversation
                    .as_ref()
                    .ok_or_else(|| "Requested conversation not found.".to_string())?,
                DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT,
            )?
        };
        // 切换会话不改变任何列表数据：不再全量重建未归档会话概览。
        let unarchived_conversations = Vec::new();
        let mut materialized_snapshot = snapshot;
        materialize_chat_message_parts_from_media_refs(
            &mut materialized_snapshot.messages,
            &state.data_path,
        );
        Ok(SwitchActiveConversationSnapshotMutationResult {
            snapshot: materialized_snapshot,
            unarchived_conversations,
        })
    }

    fn get_foreground_conversation_meta_for_fast_path(
        &self,
        state: &AppState,
        conversation_id: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Option<ConversationMetaView>, String> {
        if let Some(conversation_id) = conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let conversation_meta = self.get_conversation_meta(state, conversation_id)?;
            if self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
                && (conversation_meta.visible_in_foreground_lists
                    || conversation_meta.is_remote_im_contact
                    || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT)
            {
                return Ok(Some(conversation_meta));
            }
            return Err(format!(
                "Conversation not available for chat view: {}",
                conversation_id
            ));
        }

        if let Some(main_conversation_id) = main_conversation_id_downgraded(state)
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        {
            let conversation_meta = self.get_conversation_meta(state, &main_conversation_id)?;
            if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                return Err(format!(
                    "Unarchived conversation not found: {}",
                    main_conversation_id
                ));
            }
            return Ok(Some(conversation_meta));
        }

        let mut app_config = state_read_config_cached(state)?;
        let assistant_agent_id = assistant_agent_id_downgraded(state);
        let agents = state_read_agents_cached(state)?;
        let effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &agents,
            &assistant_agent_id,
            agent_id.unwrap_or_default(),
        )?;
        if let Some(target_conversation_id) =
            self.resolve_latest_foreground_conversation_id(state, &effective_agent_id)?
        {
            return self
                .get_conversation_meta(state, &target_conversation_id)
                .map(Some);
        }
        Ok(None)
    }

    fn get_foreground_snapshot(
        &self,
        state: &AppState,
        conversation_id: Option<&str>,
        agent_id: Option<&str>,
        recent_limit: usize,
    ) -> Result<ForegroundConversationSnapshotCore, String> {
        let mut snapshot = if let Some(conversation_meta) =
            self.get_foreground_conversation_meta_for_fast_path(state, conversation_id, agent_id)?
        {
            build_foreground_conversation_snapshot_from_meta_view(
                state,
                &conversation_meta,
                recent_limit,
            )?
        } else {
            ForegroundConversationSnapshotCore {
                conversation_id: String::new(),
                messages: Vec::new(),
                last_message_id: None,
                has_more_history: false,
                runtime_state: None,
                current_todo: None,
                current_todos: Vec::new(),
                preferred_api_config_id: None,
                active_goal: None,
            }
        };

        materialize_chat_message_parts_from_media_refs(&mut snapshot.messages, &state.data_path);
        snapshot.messages = project_messages_for_frontend_display_only(snapshot.messages);
        Ok(snapshot)
    }

    fn mark_conversation_read(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<MarkConversationReadResult, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(MarkConversationReadResult {
                conversation: None,
            });
        }
        let conversation_meta = with_conversation_mutation(
            state,
            normalized_conversation_id,
            "mark_conversation_read",
            || {
                match self.get_conversation_meta(state, normalized_conversation_id) {
                    Ok(conversation_meta) => Ok(Some(conversation_meta)),
                    Err(err) => {
                        runtime_log_debug(format!(
                            "[会话已读] 读取会话失败，conversation_id={}，error={}",
                            normalized_conversation_id, err
                        ));
                        Ok(None)
                    }
                }
            },
        )?;
        let Some(conversation_meta) = conversation_meta else {
            return Ok(MarkConversationReadResult {
                conversation: None,
            });
        };
        if conversation_meta.unread_count == 0 {
            return Ok(MarkConversationReadResult {
                conversation: Some(self.build_conversation_record_from_meta_view(
                    &conversation_meta,
                )),
            });
        }
        let result_conversation =
            self.set_conversation_unread_count_metadata(state, normalized_conversation_id, 0)?;
        Ok(MarkConversationReadResult {
            conversation: Some(result_conversation),
        })
    }

}
