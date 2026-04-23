impl ConversationService {
    fn resolve_effective_agent_id_for_read(
        &self,
        state: &AppState,
        app_config: &mut AppConfig,
        data: &AppData,
        requested_agent_id: &str,
    ) -> Result<String, String> {
        let mut runtime_agents = data.agents.clone();
        merge_private_organization_into_runtime(&state.data_path, app_config, &mut runtime_agents)?;
        let requested_agent_id = requested_agent_id.trim();
        if !requested_agent_id.is_empty() {
            if runtime_agents
                .iter()
                .any(|agent| agent.id == requested_agent_id && !agent.is_built_in_user)
            {
                return Ok(requested_agent_id.to_string());
            }
            return Err(format!("Selected agent '{requested_agent_id}' not found."));
        }
        if runtime_agents
            .iter()
            .any(|agent| agent.id == data.assistant_department_agent_id && !agent.is_built_in_user)
        {
            return Ok(data.assistant_department_agent_id.clone());
        }
        runtime_agents
            .iter()
            .find(|agent| !agent.is_built_in_user)
            .map(|agent| agent.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())
    }

    fn read_foreground_snapshot(
        &self,
        state: &AppState,
        conversation_id: Option<&str>,
        agent_id: Option<&str>,
        recent_limit: usize,
    ) -> Result<ForegroundConversationSnapshotCore, String> {
        let direct_conversation_id = conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let mut snapshot = if let Some(conversation_id) = direct_conversation_id {
            self.with_unarchived_conversation_by_id_fast(state, &conversation_id, |conversation| {
                ensure_unarchived_conversation_not_organizing(state, &conversation.id)?;
                build_foreground_conversation_snapshot_from_conversation(
                    state,
                    conversation,
                    recent_limit,
                )
            })?
        } else {
            let guard = state.conversation_lock.lock().map_err(|err| {
                format!(
                    "Failed to lock state mutex at {}:{} {}: {err}",
                    file!(),
                    line!(),
                    module_path!()
                )
            })?;

            let app_config = state_read_config_cached(state)?;
            let mut data = state_read_app_data_cached(state)?;
            let target_idx = resolve_foreground_snapshot_target_index(
                &SwitchActiveConversationSnapshotInput {
                    conversation_id: conversation_id.map(ToOwned::to_owned),
                    agent_id: agent_id.map(ToOwned::to_owned),
                },
                &app_config,
                &mut data,
            )?;
            let target_conversation_id = data
                .conversations
                .get(target_idx)
                .map(|item| item.id.clone())
                .ok_or_else(|| "Unarchived conversation index out of bounds.".to_string())?;
            ensure_unarchived_conversation_not_organizing(state, &target_conversation_id)?;
            let snapshot = build_foreground_conversation_snapshot_core(
                state,
                &data,
                target_idx,
                recent_limit,
            )?;
            drop(guard);
            snapshot
        };

        materialize_chat_message_parts_from_media_refs(&mut snapshot.messages, &state.data_path);
        Ok(snapshot)
    }

    fn read_chat_snapshot(
        &self,
        state: &AppState,
        input: &SessionSelector,
    ) -> Result<ChatSnapshot, String> {
        let guard = state.conversation_lock.lock().map_err(|err| {
            format!(
                "Failed to lock state mutex at {}:{} {}: {err}",
                file!(),
                line!(),
                module_path!()
            )
        })?;

        let mut app_config = state_read_config_cached(state)?;
        let mut data = state_read_app_data_cached(state)?;
        let before_conversation_count = data.conversations.len();
        let before_main_conversation_id = data.main_conversation_id.clone();
        let effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &data,
            &input.agent_id,
        )?;

        let idx = if let Some(existing_idx) =
            latest_active_conversation_index(&data, "", &effective_agent_id)
        {
            existing_idx
        } else {
            let api_config = resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config available".to_string())?;
            ensure_active_conversation_index(&mut data, &api_config.id, &effective_agent_id)
        };
        let conversation = data
            .conversations
            .get(idx)
            .ok_or_else(|| "Selected conversation index out of bounds.".to_string())?;

        let mut latest_user = conversation
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .cloned();
        let mut latest_assistant = conversation
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "assistant" && !is_tool_review_report_message(message))
            .cloned();

        let changed = data.conversations.len() != before_conversation_count
            || data.main_conversation_id != before_main_conversation_id;
        if changed {
            persist_selected_conversations_and_runtime(
                state,
                &data,
                &foreground_conversation_ids(&data),
                "get_chat_snapshot",
            )?;
        }
        drop(guard);

        if let Some(message) = latest_user.as_mut() {
            materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
        }
        if let Some(message) = latest_assistant.as_mut() {
            materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
        }

        Ok(ChatSnapshot {
            conversation_id: conversation.id.clone(),
            latest_user,
            latest_assistant,
            active_message_count: conversation
                .messages
                .iter()
                .filter(|message| !is_tool_review_report_message(message))
                .count(),
        })
    }

    fn refresh_unarchived_conversation_overview_payload(
        &self,
        state: &AppState,
    ) -> Result<UnarchivedConversationOverviewUpdatedPayload, String> {
        let guard = state.conversation_lock.lock().map_err(|err| {
            format!(
                "Failed to lock state mutex at {}:{} {}: {err}",
                file!(),
                line!(),
                module_path!()
            )
        })?;
        let data = state_read_app_data_cached(state)?;
        let app_config = state_read_config_cached(state)?;
        let payload = build_unarchived_conversation_overview_payload(state, &app_config, &data);
        drop(guard);
        Ok(payload)
    }

    fn list_unarchived_conversation_summaries(
        &self,
        state: &AppState,
    ) -> Result<ListUnarchivedConversationsMutationResult, String> {
        let guard = state.conversation_lock.lock().map_err(|err| {
            format!(
                "Failed to lock state mutex at {}:{} {}: {err}",
                file!(),
                line!(),
                module_path!()
            )
        })?;
        let data = state_read_app_data_cached(state)?;
        let app_config = state_read_config_cached(state)?;
        let summaries = collect_unarchived_conversation_summaries(state, &app_config, &data);
        drop(guard);
        Ok(ListUnarchivedConversationsMutationResult { summaries })
    }

    fn read_active_conversation_messages(
        &self,
        state: &AppState,
        session: &SessionSelector,
    ) -> Result<Vec<ChatMessage>, String> {
        let guard = state.conversation_lock.lock().map_err(|err| {
            format!(
                "Failed to lock state mutex at {}:{} {}: {err}",
                file!(),
                line!(),
                module_path!()
            )
        })?;
        let mut app_config = state_read_config_cached(state)?;
        let mut data = state_read_app_data_cached(state)?;
        let _normalized_changed = normalize_single_active_main_conversation(&mut data);
        let _department_changed =
            normalize_foreground_conversation_departments(&app_config, &mut data);
        let effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &data,
            &session.agent_id,
        )?;
        let requested_conversation_id = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let idx = resolve_unarchived_conversation_index_with_fallback(
            &mut data,
            &app_config,
            &effective_agent_id,
            requested_conversation_id,
        )?;
        let mut messages = data.conversations[idx].messages.clone();
        drop(guard);
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(messages)
    }

    fn read_unarchived_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let mut messages =
            self.with_unarchived_conversation_by_id(state, conversation_id, |conversation| {
                Ok(conversation.messages.clone())
            })?;
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(messages)
    }

    fn read_recent_unarchived_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_limit = limit.clamp(1, 50);
        let mut messages =
            self.with_unarchived_conversation_by_id(state, conversation_id, |conversation| {
                let total = conversation.messages.len();
                let start = total.saturating_sub(normalized_limit);
                Ok(conversation.messages[start..].to_vec())
            })?;
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(messages)
    }

    fn read_message_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<ChatMessage, String> {
        let normalized_message_id = message_id.trim();
        if normalized_message_id.is_empty() {
            return Err("messageId is required.".to_string());
        }
        let mut message =
            self.with_unarchived_conversation_by_id(state, conversation_id, |conversation| {
                conversation
                    .messages
                    .iter()
                    .find(|item| item.id.trim() == normalized_message_id)
                    .cloned()
                    .ok_or_else(|| format!("Message not found: {normalized_message_id}"))
            })?;
        materialize_chat_message_parts_from_media_refs(
            std::slice::from_mut(&mut message),
            &state.data_path,
        );
        Ok(message)
    }

    fn read_messages_before(
        &self,
        state: &AppState,
        session: &SessionSelector,
        before_message_id: &str,
        limit: usize,
    ) -> Result<(Vec<ChatMessage>, bool), String> {
        let normalized_before_message_id = before_message_id.trim();
        if normalized_before_message_id.is_empty() {
            return Err("beforeMessageId is required.".to_string());
        }
        let normalized_limit = limit.clamp(1, 100);
        let direct_conversation_id = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let (mut page, has_more) = if let Some(conversation_id) = direct_conversation_id {
            self.with_unarchived_conversation_by_id_fast(state, &conversation_id, |conversation| {
                clone_messages_before_page(
                    &conversation.messages,
                    normalized_before_message_id,
                    normalized_limit,
                )
            })?
        } else {
            let guard = state.conversation_lock.lock().map_err(|err| {
                format!(
                    "Failed to lock state mutex at {}:{} {}: {err}",
                    file!(),
                    line!(),
                    module_path!()
                )
            })?;

            let mut app_config = state_read_config_cached(state)?;
            let mut data = state_read_app_data_cached(state)?;
            let _normalized_changed = normalize_single_active_main_conversation(&mut data);
            let _department_changed =
                normalize_foreground_conversation_departments(&app_config, &mut data);
            let effective_agent_id = self.resolve_effective_agent_id_for_read(
                state,
                &mut app_config,
                &data,
                &session.agent_id,
            )?;

            let idx = resolve_unarchived_conversation_index_with_fallback(
                &mut data,
                &app_config,
                &effective_agent_id,
                None,
            )?;
            let result = clone_messages_before_page(
                &data.conversations[idx].messages,
                normalized_before_message_id,
                normalized_limit,
            )?;
            drop(guard);
            result
        };

        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok((page, has_more))
    }

    fn read_messages_after(
        &self,
        state: &AppState,
        session: &SessionSelector,
        after_message_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_after_message_id = after_message_id.trim();
        if normalized_after_message_id.is_empty() {
            return Err("afterMessageId is required.".to_string());
        }
        let normalized_limit = limit.clamp(1, 100);
        let direct_conversation_id = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);

        let mut page = if let Some(conversation_id) = direct_conversation_id {
            self.with_unarchived_conversation_by_id_fast(state, &conversation_id, |conversation| {
                clone_messages_after_page(
                    &conversation.messages,
                    normalized_after_message_id,
                    normalized_limit,
                )
            })?
        } else {
            let guard = state.conversation_lock.lock().map_err(|err| {
                format!(
                    "Failed to lock state mutex at {}:{} {}: {err}",
                    file!(),
                    line!(),
                    module_path!()
                )
            })?;

            let mut app_config = state_read_config_cached(state)?;
            let mut data = state_read_app_data_cached(state)?;
            let _normalized_changed = normalize_single_active_main_conversation(&mut data);
            let _department_changed =
                normalize_foreground_conversation_departments(&app_config, &mut data);
            let effective_agent_id = self.resolve_effective_agent_id_for_read(
                state,
                &mut app_config,
                &data,
                &session.agent_id,
            )?;

            let idx = resolve_unarchived_conversation_index_with_fallback(
                &mut data,
                &app_config,
                &effective_agent_id,
                None,
            )?;
            let page = clone_messages_after_page(
                &data.conversations[idx].messages,
                normalized_after_message_id,
                normalized_limit,
            )?;
            drop(guard);
            page
        };

        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok(page)
    }

    fn read_messages_after_with_fallback(
        &self,
        state: &AppState,
        conversation_id: &str,
        after_message_id: Option<&str>,
        fallback_limit: usize,
    ) -> Result<(Vec<ChatMessage>, Option<String>), String> {
        let trimmed_after = after_message_id
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let (mut page, fallback_mode) =
            self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                let messages = &conversation.messages;
                let page_result = if let Some(after_id) = trimmed_after {
                    if let Some(after_idx) = messages.iter().position(|item| item.id == after_id) {
                        (messages[(after_idx + 1)..].to_vec(), None)
                    } else {
                        let start = messages.len().saturating_sub(fallback_limit);
                        (messages[start..].to_vec(), Some("recent_limit".to_string()))
                    }
                } else {
                    let start = messages.len().saturating_sub(fallback_limit);
                    (messages[start..].to_vec(), Some("recent_limit".to_string()))
                };
                Ok(page_result)
            })?;
        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok((page, fallback_mode))
    }
}
