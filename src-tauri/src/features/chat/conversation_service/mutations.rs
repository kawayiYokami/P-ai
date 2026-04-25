impl ConversationService {
    fn update_unarchived_conversation_by_id<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        updater: impl FnOnce(&mut Conversation) -> Result<T, String>,
    ) -> Result<T, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut conversation = state_read_conversation_cached(state, normalized_conversation_id)?;
        self.ensure_unarchived_foreground_conversation(&conversation, normalized_conversation_id)?;
        let result = updater(&mut conversation)?;
        state_schedule_conversation_persist(state, &conversation, false)?;
        drop(guard);
        Ok(result)
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
        let mut runtime = state_read_runtime_state_cached(state)?;
        let effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &agents,
            &runtime.assistant_department_agent_id,
            input.agent_id.as_deref().unwrap_or_default(),
        )?;
        let requested_conversation_id = input
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (mut target_conversation, created_new_conversation) =
            if let Some(conversation_id) = requested_conversation_id {
                let conversation = state_read_conversation_cached(state, conversation_id)
                    .ok()
                    .filter(|conversation| {
                        conversation.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(conversation)
                    })
                    .ok_or_else(|| {
                        format!("Requested conversation not found: {conversation_id}")
                    })?;
                (conversation, false)
            } else if let Some(conversation) = runtime
                .main_conversation_id
                .as_deref()
                .and_then(|conversation_id| {
                    state_read_conversation_cached(state, conversation_id.trim()).ok()
                })
                .filter(|conversation| {
                    conversation.summary.trim().is_empty()
                        && conversation_visible_in_foreground_lists(conversation)
                })
            {
                (conversation, false)
            } else if let Some(conversation) =
                read_latest_visible_foreground_conversation(state)?
            {
                (conversation, false)
            } else {
                let selected_api = resolve_selected_api_config(&app_config, None)
                    .ok_or_else(|| "No API config available".to_string())?;
                let department_id = assistant_department(&app_config)
                    .map(|department| department.id.clone())
                    .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
                let conversation = build_unarchived_conversation_record_from_runtime(
                    &state.data_path,
                    &agents,
                    &runtime.assistant_department_agent_id,
                    read_latest_archive_summary_from_chat_index(state)?,
                    &selected_api.id,
                    &effective_agent_id,
                    &department_id,
                    "",
                );
                (conversation, true)
            };
        let target_conversation_id = target_conversation.id.clone();
        ensure_unarchived_conversation_not_organizing(state, &target_conversation_id)?;
        clear_conversation_unread_count(&mut target_conversation);
        if created_new_conversation {
            state_schedule_conversation_persist(state, &target_conversation, true)?;
        } else {
            state_schedule_conversation_persist(state, &target_conversation, false)?;
        }
        if runtime.main_conversation_id.as_deref().map(str::trim)
            != Some(target_conversation_id.as_str())
        {
            runtime.main_conversation_id = Some(target_conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
        }
        let snapshot = build_foreground_conversation_snapshot_from_conversation(
            state,
            &target_conversation,
            DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT,
        )?;
        let unarchived_conversations =
            self.collect_unarchived_conversation_summaries_cached(state, &app_config)?;
        drop(guard);

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

    fn set_active_unarchived_conversation(
        &self,
        state: &AppState,
        input: &SetActiveUnarchivedConversationInput,
    ) -> Result<String, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

        let mut app_config = state_read_config_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let mut runtime = state_read_runtime_state_cached(state)?;
        let effective_agent_id = self.resolve_effective_agent_id_for_read(
            state,
            &mut app_config,
            &agents,
            &runtime.assistant_department_agent_id,
            input.agent_id.as_deref().unwrap_or_default(),
        )?;
        let requested_conversation_id = input
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (target_conversation, created_new_conversation) =
            if let Some(conversation_id) = requested_conversation_id {
                if let Some(conversation) = state_read_conversation_cached(state, conversation_id)
                    .ok()
                    .filter(|conversation| {
                        conversation.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(conversation)
                    })
                {
                    (conversation, false)
                } else if let Some(conversation) =
                    runtime.main_conversation_id.as_deref().and_then(|current_main| {
                        state_read_conversation_cached(state, current_main.trim()).ok()
                    }).filter(|conversation| {
                        conversation.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(conversation)
                    })
                {
                    (conversation, false)
                } else if let Some(conversation) =
                    read_latest_visible_foreground_conversation(state)?
                {
                    (conversation, false)
                } else {
                    let selected_api = resolve_selected_api_config(&app_config, None)
                        .ok_or_else(|| "No API config available".to_string())?;
                    let department_id = assistant_department(&app_config)
                        .map(|department| department.id.clone())
                        .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
                    let conversation = build_unarchived_conversation_record_from_runtime(
                        &state.data_path,
                        &agents,
                        &runtime.assistant_department_agent_id,
                        read_latest_archive_summary_from_chat_index(state)?,
                        &selected_api.id,
                        &effective_agent_id,
                        &department_id,
                        "",
                    );
                    (conversation, true)
                }
            } else if let Some(conversation) = runtime
                .main_conversation_id
                .as_deref()
                .and_then(|conversation_id| {
                    state_read_conversation_cached(state, conversation_id.trim()).ok()
                })
                .filter(|conversation| {
                    conversation.summary.trim().is_empty()
                        && conversation_visible_in_foreground_lists(conversation)
                })
            {
                (conversation, false)
            } else if let Some(conversation) = read_latest_visible_foreground_conversation(state)? {
                (conversation, false)
            } else {
                let selected_api = resolve_selected_api_config(&app_config, None)
                    .ok_or_else(|| "No API config available".to_string())?;
                let department_id = assistant_department(&app_config)
                    .map(|department| department.id.clone())
                    .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
                let conversation = build_unarchived_conversation_record_from_runtime(
                    &state.data_path,
                    &agents,
                    &runtime.assistant_department_agent_id,
                    read_latest_archive_summary_from_chat_index(state)?,
                    &selected_api.id,
                    &effective_agent_id,
                    &department_id,
                    "",
                );
                (conversation, true)
            };
        let conversation_id = target_conversation.id.clone();
        ensure_unarchived_conversation_not_organizing(state, &conversation_id)?;
        if created_new_conversation {
            state_schedule_conversation_persist(state, &target_conversation, true)?;
        }
        if runtime.main_conversation_id.as_deref().map(str::trim) != Some(conversation_id.as_str())
        {
            runtime.main_conversation_id = Some(conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
        }
        drop(guard);
        Ok(conversation_id)
    }

    fn toggle_unarchived_conversation_pin(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<ToggleUnarchivedConversationPinMutationResult, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId 不能为空".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

        let mut runtime = state_read_runtime_state_cached(state)?;
        let main_conversation_id = runtime
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if normalized_conversation_id == main_conversation_id {
            drop(guard);
            return Err("主会话始终置顶".to_string());
        }
        let conversation = match state_read_conversation_cached(state, normalized_conversation_id) {
            Ok(conversation) => conversation,
            Err(_) => {
            drop(guard);
            return Err("未找到可置顶的会话".to_string());
            }
        };
        if !conversation.summary.trim().is_empty()
            || !conversation_visible_in_foreground_lists(&conversation)
        {
            drop(guard);
            return Err("未找到可置顶的会话".to_string());
        }

        let visible_ids = state_read_chat_index_cached(state)?
            .conversations
            .iter()
            .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
            .filter(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .map(|conversation| conversation.id.trim().to_string())
            .filter(|conversation_id| !conversation_id.is_empty())
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::<String>::new();
        let previous_pinned = runtime
            .pinned_conversation_ids
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .filter(|item| *item != main_conversation_id)
            .filter(|item| visible_ids.contains(item))
            .filter(|item| seen.insert(item.clone()))
            .collect::<Vec<_>>();
        let mut next_pinned = previous_pinned.clone();
        if let Some(index) = next_pinned
            .iter()
            .position(|item| item.trim() == normalized_conversation_id)
        {
            next_pinned.remove(index);
        } else {
            next_pinned.insert(0, normalized_conversation_id.to_string());
        }
        runtime.pinned_conversation_ids = next_pinned.clone();
        state_write_runtime_state_cached(state, &runtime)?;
        drop(guard);

        let is_pinned = next_pinned
            .iter()
            .any(|item| item.trim() == normalized_conversation_id);
        let pin_index = next_pinned
            .iter()
            .position(|item| item.trim() == normalized_conversation_id);
        Ok(ToggleUnarchivedConversationPinMutationResult {
            conversation_id: normalized_conversation_id.to_string(),
            is_pinned,
            pin_index,
        })
    }


    fn delete_main_conversation_and_activate_latest(
        &self,
        state: &AppState,
        selected_api: &ApiConfig,
        source: &Conversation,
    ) -> Result<String, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut runtime = state_read_runtime_state_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let source_conversation = state_read_conversation_cached(state, &source.id)
            .map_err(|_| "活动对话已变化，请重试归档。".to_string())?;
        if source_conversation.summary.trim().is_empty() || conversation_is_delegate(&source_conversation) {
            drop(guard);
            return Err("活动对话已变化，请重试归档。".to_string());
        }
        state_schedule_conversation_delete(state, &source.id, true)?;
        let chat_index = state_read_chat_index_cached(state)?;
        let active_conversation_id = chat_index
            .conversations
            .iter()
            .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
            .find(|conversation| {
                conversation.id != source.id
                    && conversation.summary.trim().is_empty()
                    && !conversation_is_delegate(conversation)
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .map(|conversation| conversation.id.clone());
        let active_conversation_id = if let Some(active_conversation_id) = active_conversation_id {
            runtime.main_conversation_id = Some(active_conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
            active_conversation_id
        } else {
            let replacement = build_archive_replacement_conversation(
                state,
                &agents,
                &runtime.assistant_department_agent_id,
                selected_api,
                &source_conversation,
            )?;
            let replacement_id = replacement.id.clone();
            state_schedule_conversation_persist(state, &replacement, true)?;
            runtime.main_conversation_id = Some(replacement_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
            replacement_id
        };
        drop(guard);

        cleanup_pdf_session_memory_cache_for_conversation(&source.id);
        Ok(active_conversation_id)
    }

    fn rewind_conversation_from_message(
        &self,
        state: &AppState,
        input: &RewindConversationInput,
        requested_agent_id: &str,
        message_id: &str,
        started_at: &std::time::Instant,
    ) -> Result<RewindConversationMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| {
                format!(
                    "Failed to lock state mutex at {}:{} {}: {err}",
                    file!(),
                    line!(),
                    module_path!()
                )
            })?;

        let requested_conversation_id = trimmed_option(input.session.conversation_id.as_deref());
        let conversation_id = if let Some(conversation_id) = requested_conversation_id.as_deref() {
            let conversation = state_read_conversation_cached(state, conversation_id)
                .map_err(|_| {
                    "Target user message not found in active conversation.".to_string()
                })?;
            if conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(&conversation)
            {
                conversation.id
            } else {
                drop(guard);
                return Err("Target user message not found in active conversation.".to_string());
            }
        } else {
            self.resolve_latest_foreground_conversation_id(state, requested_agent_id)?
                .ok_or_else(|| "No conversation found for current agent.".to_string())?
        };
        let mut conversation = state_read_conversation_cached(state, &conversation_id)
            .map_err(|_| "Target user message not found in active conversation.".to_string())?;
        if !conversation.summary.trim().is_empty()
            || !conversation_visible_in_foreground_lists(&conversation)
        {
            drop(guard);
            return Err("Target user message not found in active conversation.".to_string());
        }
        let result = execute_rewind_conversation_mutation_on_conversation(
            state,
            &mut conversation,
            input,
            message_id,
            started_at,
        )?;
        if result.removed_count > 0 {
            let store_paths = message_store::message_store_paths(&state.data_path, &conversation.id)?;
            if message_store::should_write_jsonl_snapshot_directory_shard(&store_paths)? {
                let _write_guard = state
                    .app_data_persist_write_lock
                    .lock()
                    .map_err(|err| {
                        named_lock_error(
                            "app_data_persist_write_lock",
                            file!(),
                            line!(),
                            module_path!(),
                            &err,
                        )
                    })?;
                message_store::write_jsonl_snapshot_truncated_directory_shard(
                    &store_paths,
                    &conversation,
                    result.remaining_count,
                )?;
                state_mark_conversation_direct_persisted(state, &conversation, true)?;
            } else {
                state_schedule_conversation_persist(state, &conversation, false)?;
            }
        }
        drop(guard);

        Ok(result)
    }

    fn persist_stop_chat_partial_message(
        &self,
        state: &AppState,
        requested_conversation_id: Option<&str>,
        requested_department_id: Option<&str>,
        agent_id: &str,
        partial_assistant_text: &str,
        partial_reasoning_standard: &str,
        partial_reasoning_inline: &str,
        completed_tool_history: &[Value],
    ) -> Result<StopChatPersistResult, String> {
        let should_persist = !partial_assistant_text.is_empty()
            || !partial_reasoning_standard.is_empty()
            || !partial_reasoning_inline.is_empty()
            || !completed_tool_history.is_empty();
        if !should_persist {
            return Ok(StopChatPersistResult {
                persisted: false,
                conversation_id: None,
                assistant_message: None,
            });
        }

        let _guard = lock_conversation_with_metrics(state, "stop_chat_generation_persist_partial")?;
        let app_config = state_read_config_cached(state)?;
        let api_config_id =
            resolve_stop_chat_api_config_id(&app_config, requested_department_id, agent_id)?;
        if !app_config.api_configs.iter().any(|api| api.id == api_config_id) {
            return Err(format!("Selected API config '{api_config_id}' not found."));
        }
        let Some(mut target) =
            resolve_stop_chat_target(state, requested_conversation_id, agent_id)?
        else {
            return Ok(StopChatPersistResult {
                persisted: false,
                conversation_id: None,
                assistant_message: None,
            });
        };
        if let Some(result) = build_stop_chat_skip_result(target.conversation()) {
            return Ok(result);
        }

        let assistant_message = build_stop_chat_partial_assistant_message(
            agent_id,
            partial_assistant_text,
            partial_reasoning_standard,
            partial_reasoning_inline,
            completed_tool_history,
        );
        let conversation_id = apply_stop_chat_partial_message(target.conversation_mut(), &assistant_message);
        persist_stop_chat_target_update(self, state, target, &conversation_id)?;

        Ok(StopChatPersistResult {
            persisted: true,
            conversation_id: Some(conversation_id),
            assistant_message: Some(assistant_message),
        })
    }


    fn append_message_to_unarchived_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<(), String> {
        self.update_unarchived_conversation_by_id(state, conversation_id, |conversation| {
            conversation.messages.push(message.clone());
            conversation.updated_at = message.created_at.clone();
            conversation.last_assistant_at = Some(message.created_at.clone());
            self.increment_conversation_unread_count_if_background(state, conversation, 1);
            Ok(())
        })
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
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut conversation = match state_read_conversation_cached(state, normalized_conversation_id) {
            Ok(conversation) => conversation,
            Err(err) => {
                runtime_log_debug(format!(
                    "[会话已读] 读取会话失败，conversation_id={}，error={}",
                    normalized_conversation_id, err
                ));
                drop(guard);
                return Ok(MarkConversationReadResult {
                    conversation: None,
                });
            }
        };
        if !clear_conversation_unread_count(&mut conversation) {
            drop(guard);
            return Ok(MarkConversationReadResult {
                conversation: Some(conversation),
            });
        }
        state_schedule_conversation_persist(state, &conversation, false)?;
        let result_conversation = conversation.clone();
        drop(guard);
        Ok(MarkConversationReadResult {
            conversation: Some(result_conversation),
        })
    }

    fn rename_unarchived_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
        next_title: &str,
    ) -> Result<String, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let normalized_title = next_title.trim();
        if normalized_title.is_empty() {
            return Err("title is required.".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;

        let runtime = state_read_runtime_state_cached(state)?;
        let main_conversation_id = runtime
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if normalized_conversation_id == main_conversation_id {
            drop(guard);
            return Err("主会话暂不支持改名".to_string());
        }
        ensure_unarchived_conversation_not_organizing(state, normalized_conversation_id)?;

        let mut conversation = state_read_conversation_cached(state, normalized_conversation_id)?;
        self.ensure_unarchived_foreground_conversation(&conversation, normalized_conversation_id)
            .map_err(|_| "未找到可改名的会话".to_string())?;
        if conversation.title.trim() == normalized_title {
            drop(guard);
            return Ok(normalized_title.to_string());
        }

        conversation.title = normalized_title.to_string();
        state_schedule_conversation_persist(state, &conversation, false)?;
        drop(guard);
        Ok(normalized_title.to_string())
    }

    fn create_unarchived_conversation(
        &self,
        state: &AppState,
        input: &CreateUnarchivedConversationInput,
    ) -> Result<CreateUnarchivedConversationMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let app_config = state_read_config_cached(state)?;
        let mut runtime = state_read_runtime_state_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let requested_department_id = input
            .department_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let department = if let Some(department_id) = requested_department_id {
            department_by_id(&app_config, department_id)
                .ok_or_else(|| format!("Department '{department_id}' not found."))?
        } else {
            assistant_department(&app_config)
                .ok_or_else(|| "No assistant department configured.".to_string())?
        };
        let api_config_id = input
            .api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| department_primary_api_config_id(department));
        let agent_id = input
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                department
                    .agent_ids
                    .iter()
                    .find(|id| !id.trim().is_empty())
                    .cloned()
            })
            .unwrap_or_else(|| runtime.assistant_department_agent_id.clone());
        let conversation_title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let copy_source_conversation_id = trimmed_option(input.copy_source_conversation_id.as_deref());
        let conversation = if let Some(source_conversation_id) = copy_source_conversation_id.as_deref() {
            let source_conversation = state_read_conversation_cached(state, source_conversation_id)
                .ok()
                .filter(|conversation| {
                    conversation.summary.trim().is_empty()
                        && conversation_visible_in_foreground_lists(conversation)
                })
                .ok_or_else(|| "要复制的当前会话不存在或已归档".to_string())?;
            clone_foreground_conversation_for_copy(
                &source_conversation,
                &agent_id,
                &department.id,
                conversation_title,
            )
        } else {
            build_unarchived_conversation_record_from_runtime(
                &state.data_path,
                &agents,
                &runtime.assistant_department_agent_id,
                read_latest_archive_summary_from_chat_index(state)?,
                &api_config_id,
                &agent_id,
                &department.id,
                conversation_title,
            )
        };
        let conversation_id = conversation.id.clone();
        state_schedule_conversation_persist(state, &conversation, true)?;
        if runtime
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            runtime.main_conversation_id = Some(conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
        }
        let overview_payload = UnarchivedConversationOverviewUpdatedPayload {
            preferred_conversation_id: Some(conversation_id.clone()),
            unarchived_conversations: self.collect_unarchived_conversation_summaries_cached(
                state,
                &app_config,
            )?,
        };
        drop(guard);
        Ok(CreateUnarchivedConversationMutationResult {
            conversation_id,
            overview_payload,
        })
    }

    fn branch_unarchived_conversation_from_selection(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        selected_message_ids: &[String],
    ) -> Result<BranchUnarchivedConversationMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let app_config = state_read_config_cached(state)?;
        let mut runtime = state_read_runtime_state_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let source_conversation = state_read_conversation_cached(state, source_conversation_id)
            .ok()
            .filter(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let (selected_messages, first_selected_ordinal) =
            collect_selected_messages_for_branch(&source_conversation, selected_message_ids);
        if selected_messages.is_empty() {
            drop(guard);
            return Err("未找到可创建会话分支的已选消息".to_string());
        }
        let department = department_by_id(&app_config, source_conversation.department_id.trim())
            .cloned()
            .ok_or_else(|| "源会话所属部门不存在".to_string())?;
        let branched_title = build_branch_conversation_title(
            &source_conversation.title,
            first_selected_ordinal.max(1),
            runtime.main_conversation_id.as_deref().map(str::trim)
                == Some(source_conversation.id.as_str()),
        );
        let latest_compaction_message = latest_compaction_message_for_branch(&source_conversation);
        let conversation = build_branch_conversation_record_from_selection_runtime(
            &state.data_path,
            &agents,
            &runtime.assistant_department_agent_id,
            &source_conversation,
            &department,
            &branched_title,
            latest_compaction_message.as_ref(),
            &selected_messages,
        );
        let conversation_id = conversation.id.clone();
        state_schedule_conversation_persist(state, &conversation, true)?;
        if runtime
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            runtime.main_conversation_id = Some(conversation_id.clone());
            state_write_runtime_state_cached(state, &runtime)?;
        }
        let overview_payload = UnarchivedConversationOverviewUpdatedPayload {
            preferred_conversation_id: Some(conversation_id.clone()),
            unarchived_conversations: self.collect_unarchived_conversation_summaries_cached(
                state,
                &app_config,
            )?,
        };
        drop(guard);
        Ok(BranchUnarchivedConversationMutationResult {
            conversation_id,
            title: branched_title,
            selected_count: selected_messages.len(),
            has_compaction_seed: latest_compaction_message.is_some(),
            overview_payload,
        })
    }

    fn forward_unarchived_conversation_selection(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        target_conversation_id: &str,
        selected_message_ids: &[String],
    ) -> Result<ForwardUnarchivedConversationMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let target_runtime_state = {
            let runtime_slots = lock_conversation_runtime_slots(state)?;
            runtime_slots
                .get(target_conversation_id)
                .map(|slot| slot.state.clone())
                .unwrap_or(MainSessionState::Idle)
        };
        if target_runtime_state == MainSessionState::AssistantStreaming {
            drop(guard);
            return Err("目标会话正在流式输出中，暂时无法转发到会话".to_string());
        }
        if target_runtime_state == MainSessionState::OrganizingContext {
            drop(guard);
            return Err("目标会话正在整理上下文，暂时无法转发到会话".to_string());
        }
        let app_config = state_read_config_cached(state)?;
        let source_conversation = state_read_conversation_cached(state, source_conversation_id)
            .ok()
            .filter(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let (selected_messages, _) =
            collect_selected_messages_for_branch(&source_conversation, selected_message_ids);
        if selected_messages.is_empty() {
            drop(guard);
            return Err("未找到可转发到会话的已选消息".to_string());
        }

        let mut target_conversation = state_read_conversation_cached(state, target_conversation_id)
            .ok()
            .filter(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .ok_or_else(|| "目标会话不存在或已归档".to_string())?;
        let now = now_iso();
        target_conversation.messages.extend(
            selected_messages
                .iter()
                .map(clone_chat_message_for_copied_conversation),
        );
        target_conversation.updated_at = now.clone();
        target_conversation.status = "active".to_string();
        increment_conversation_unread_count(&mut target_conversation, selected_messages.len());
        if let Some(last_message) = target_conversation.messages.last() {
            if last_message.role.trim().eq_ignore_ascii_case("assistant") {
                target_conversation.last_assistant_at = Some(now.clone());
            } else if last_message.role.trim().eq_ignore_ascii_case("user") {
                target_conversation.last_user_at = Some(now.clone());
            }
        }

        state_schedule_conversation_persist(state, &target_conversation, true)?;
        let overview_payload = UnarchivedConversationOverviewUpdatedPayload {
            preferred_conversation_id: Some(target_conversation_id.to_string()),
            unarchived_conversations: self.collect_unarchived_conversation_summaries_cached(
                state,
                &app_config,
            )?,
        };
        drop(guard);
        Ok(ForwardUnarchivedConversationMutationResult {
            target_conversation_id: target_conversation_id.to_string(),
            forwarded_count: selected_messages.len(),
            overview_payload,
        })
    }

    fn delete_unarchived_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<DeleteUnarchivedConversationMutationResult, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let app_config = state_read_config_cached(state)?;
        let runtime = state_read_runtime_state_cached(state)?;
        let main_conversation_id = runtime
            .main_conversation_id
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if normalized_conversation_id == main_conversation_id {
            drop(guard);
            return Err("主会话暂不支持删除".to_string());
        }
        let conversation = state_read_conversation_cached(state, normalized_conversation_id)
            .map_err(|_| "Unarchived conversation not found.".to_string())?;
        if !conversation.summary.trim().is_empty()
            || !conversation_visible_in_foreground_lists(&conversation)
        {
            drop(guard);
            return Err("Unarchived conversation not found.".to_string());
        }
        let chat_index = state_read_chat_index_cached(state)?;
        let active_conversation_id = chat_index
            .conversations
            .iter()
            .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
            .find(|conversation| {
                conversation.id != normalized_conversation_id
                    && conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
                    && conversation.status.trim() == "active"
            })
            .map(|conversation| conversation.id.clone())
            .or_else(|| {
                chat_index
                    .conversations
                    .iter()
                    .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
                    .find(|conversation| {
                        conversation.id != normalized_conversation_id
                            && conversation.summary.trim().is_empty()
                            && conversation_visible_in_foreground_lists(conversation)
                    })
                    .map(|conversation| conversation.id)
            })
            .unwrap_or_default();
        mark_tasks_as_session_lost(&state.data_path, normalized_conversation_id);
        if active_conversation_id.trim().is_empty() {
            drop(guard);
            return Err("删除后未找到可用会话".to_string());
        }
        state_schedule_conversation_delete(state, normalized_conversation_id, true)?;
        let unarchived_conversations =
            self.collect_unarchived_conversation_summaries_cached(state, &app_config)?;
        drop(guard);
        Ok(DeleteUnarchivedConversationMutationResult {
            deleted_conversation_id: normalized_conversation_id.to_string(),
            active_conversation_id,
            overview_payload: UnarchivedConversationOverviewUpdatedPayload {
                preferred_conversation_id: unarchived_conversations
                    .first()
                    .map(|item| item.conversation_id.clone()),
                unarchived_conversations,
            },
        })
    }

}

enum StopChatConversationTarget {
    Runtime(Conversation),
    Persisted(Conversation),
}

impl StopChatConversationTarget {
    fn conversation(&self) -> Option<&Conversation> {
        match self {
            Self::Runtime(conversation) => Some(conversation),
            Self::Persisted(conversation) => Some(conversation),
        }
    }

    fn conversation_mut(&mut self) -> &mut Conversation {
        match self {
            Self::Runtime(conversation) => conversation,
            Self::Persisted(conversation) => conversation,
        }
    }
}

fn execute_rewind_conversation_mutation_on_conversation(
    state: &AppState,
    conversation: &mut Conversation,
    input: &RewindConversationInput,
    message_id: &str,
    started_at: &std::time::Instant,
) -> Result<RewindConversationMutationResult, String> {
    let remove_from = resolve_rewind_remove_from(conversation, message_id)?;
    let recalled_user_message = conversation.messages.get(remove_from).cloned();
    let removed_messages = conversation.messages[remove_from..].to_vec();
    let git_snapshot = recalled_user_message
        .as_ref()
        .and_then(|message| read_git_snapshot_record_from_provider_meta(message.provider_meta.as_ref()));
    maybe_undo_rewind_apply_patch(state, input, &removed_messages, message_id, started_at)?;
    let (removed_count, remaining_count, current_todo, current_todos) =
        persist_rewind_conversation_state(conversation, remove_from)?;
    Ok(RewindConversationMutationResult {
        conversation_id: conversation.id.clone(),
        removed_count,
        remaining_count,
        current_todo,
        current_todos,
        recalled_user_message,
        git_snapshot,
    })
}

fn resolve_rewind_remove_from(conversation: &Conversation, message_id: &str) -> Result<usize, String> {
    let remove_from = conversation
        .messages
        .iter()
        .position(|message| message.id == message_id && message.role == "user")
        .ok_or_else(|| "Target user message not found in active conversation.".to_string())?;
    runtime_log_info(format!(
        "[会话撤回] 命中目标，任务=rewind_conversation_from_message，conversation_id={}，remove_from={}，messages_total={}",
        conversation.id,
        remove_from,
        conversation.messages.len()
    ));
    Ok(remove_from)
}

fn maybe_undo_rewind_apply_patch(
    state: &AppState,
    input: &RewindConversationInput,
    removed_messages: &[ChatMessage],
    message_id: &str,
    started_at: &std::time::Instant,
) -> Result<(), String> {
    if !input.undo_apply_patch {
        return Ok(());
    }
    runtime_log_info(format!(
        "[会话撤回] 开始工具逆向，任务=rewind_conversation_from_message，removed_messages={}，message_id={}",
        removed_messages.len(),
        message_id
    ));
    let undone_patch_count = match try_undo_apply_patch_from_removed_messages(state, removed_messages) {
        Ok(value) => value,
        Err(err) => {
            let elapsed_ms = started_at.elapsed().as_millis();
            runtime_log_error(format!(
                "[会话撤回] 失败，任务=rewind_conversation_from_message，stage=undo_apply_patch，message_id={}，duration_ms={}，error={}",
                message_id, elapsed_ms, err
            ));
            return Err(err);
        }
    };
    runtime_log_info(format!(
        "[会话撤回] 工具逆向处理，任务=rewind_conversation_from_message，patches={}，message_id={}",
        undone_patch_count, message_id
    ));
    if undone_patch_count > 0 {
        eprintln!(
            "[会话撤回] 已执行 apply_patch 反向撤回: patches={}, message_id={}",
            undone_patch_count,
            message_id
        );
    }
    Ok(())
}

fn resolve_stop_chat_api_config_id(
    app_config: &AppConfig,
    requested_department_id: Option<&str>,
    agent_id: &str,
) -> Result<String, String> {
    requested_department_id
        .and_then(|id| department_by_id(app_config, id))
        .map(department_primary_api_config_id)
        .or_else(|| department_for_agent_id(app_config, agent_id).map(department_primary_api_config_id))
        .or_else(|| resolve_selected_api_config(app_config, None).map(|api| api.id.clone()))
        .ok_or_else(|| "Missing available API config for stop request".to_string())
}

fn resolve_stop_chat_target(
    state: &AppState,
    requested_conversation_id: Option<&str>,
    agent_id: &str,
) -> Result<Option<StopChatConversationTarget>, String> {
    let runtime_requested = requested_conversation_id
        .filter(|conversation_id| {
            delegate_runtime_thread_conversation_get(state, conversation_id)
                .ok()
                .flatten()
                .is_some()
        })
        .map(ToOwned::to_owned);
    if let Some(conversation_id) = runtime_requested.as_deref() {
        let runtime_conversation = delegate_runtime_thread_conversation_get(state, conversation_id)?;
        return Ok(runtime_conversation.map(StopChatConversationTarget::Runtime));
    }
    let conversation_id = if let Some(conversation_id) = requested_conversation_id {
        Some(conversation_id.to_string())
    } else {
        conversation_service().resolve_latest_foreground_conversation_id(state, agent_id)?
    };
    Ok(conversation_id.and_then(|conversation_id| {
        state_read_conversation_cached(state, &conversation_id)
            .ok()
            .filter(|conversation| {
                conversation.summary.trim().is_empty()
                    && conversation_visible_in_foreground_lists(conversation)
            })
            .map(StopChatConversationTarget::Persisted)
    }))
}

fn build_stop_chat_skip_result(conversation: Option<&Conversation>) -> Option<StopChatPersistResult> {
    let conversation = conversation?;
    if conversation
        .messages
        .last()
        .map(|message| message.role == "assistant")
        .unwrap_or(false)
    {
        return Some(StopChatPersistResult {
            persisted: false,
            conversation_id: Some(conversation.id.clone()),
            assistant_message: conversation.messages.last().cloned(),
        });
    }
    None
}

fn build_stop_chat_partial_assistant_message(
    agent_id: &str,
    partial_assistant_text: &str,
    partial_reasoning_standard: &str,
    partial_reasoning_inline: &str,
    completed_tool_history: &[Value],
) -> ChatMessage {
    let provider_meta = if partial_reasoning_standard.is_empty() && partial_reasoning_inline.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "reasoningStandard": partial_reasoning_standard,
            "reasoningInline": partial_reasoning_inline
        }))
    };
    let now = now_iso();
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now,
        speaker_agent_id: Some(agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: partial_assistant_text.to_string(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta,
        tool_call: if completed_tool_history.is_empty() {
            None
        } else {
            Some(completed_tool_history.to_vec())
        },
        mcp_call: None,
    }
}

fn apply_stop_chat_partial_message(
    conversation: &mut Conversation,
    assistant_message: &ChatMessage,
) -> String {
    conversation.messages.push(assistant_message.clone());
    conversation.updated_at = assistant_message.created_at.clone();
    conversation.last_assistant_at = Some(assistant_message.created_at.clone());
    conversation.id.clone()
}

fn persist_stop_chat_target_update(
    service: &ConversationService,
    state: &AppState,
    target: StopChatConversationTarget,
    conversation_id: &str,
) -> Result<(), String> {
    match target {
        StopChatConversationTarget::Runtime(conversation) => {
            delegate_runtime_thread_conversation_update(state, conversation_id, conversation)
        }
        StopChatConversationTarget::Persisted(conversation) => {
            service.persist_conversation_with_chat_index(state, &conversation)
        }
    }
}

fn read_latest_archive_summary_from_chat_index(state: &AppState) -> Result<Option<String>, String> {
    let chat_index = state_read_chat_index_cached(state)?;
    Ok(chat_index
        .conversations
        .iter()
        .rev()
        .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
        .find(|conversation| {
            !conversation_is_delegate(conversation) && !conversation.summary.trim().is_empty()
        })
        .map(|conversation| conversation.summary))
}

fn build_unarchived_conversation_record_from_runtime(
    data_path: &PathBuf,
    agents: &[AgentProfile],
    assistant_department_agent_id: &str,
    last_archive_summary: Option<String>,
    api_config_id: &str,
    agent_id: &str,
    department_id: &str,
    title: &str,
) -> Conversation {
    let mut conversation = build_conversation_record(
        api_config_id,
        agent_id,
        department_id,
        title,
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    let snapshot_agent_id = if agent_id.trim().is_empty() {
        assistant_department_agent_id.trim().to_string()
    } else {
        agent_id.trim().to_string()
    };
    let user_profile_snapshot = agents
        .iter()
        .find(|item| item.id == snapshot_agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_error(format!(
                    "[用户画像] 失败，任务=build_unarchived_conversation_record_from_runtime，agent_id={}，error={}",
                    agent.id, err
                ));
                None
            }
        });
    if let Some(snapshot) = user_profile_snapshot.clone() {
        conversation.user_profile_snapshot = snapshot;
    }
    let summary_message = build_initial_summary_context_message(
        last_archive_summary.as_deref(),
        user_profile_snapshot.as_deref(),
        Some(&conversation.current_todos),
    );
    conversation.last_user_at = Some(summary_message.created_at.clone());
    conversation.updated_at = summary_message.created_at.clone();
    conversation.messages.push(summary_message);
    conversation
}

fn branch_conversation_settings_agent_id_runtime(
    assistant_department_agent_id: &str,
    department: &DepartmentConfig,
    requested_agent_id: &str,
) -> String {
    let normalized_requested_agent_id = requested_agent_id.trim();
    if !normalized_requested_agent_id.is_empty()
        && department
            .agent_ids
            .iter()
            .any(|item| item.trim() == normalized_requested_agent_id)
    {
        return normalized_requested_agent_id.to_string();
    }
    department
        .agent_ids
        .iter()
        .find(|item| !item.trim().is_empty())
        .map(|item| item.trim().to_string())
        .unwrap_or_else(|| assistant_department_agent_id.trim().to_string())
}

fn build_branch_conversation_record_from_selection_runtime(
    data_path: &PathBuf,
    agents: &[AgentProfile],
    assistant_department_agent_id: &str,
    source: &Conversation,
    department: &DepartmentConfig,
    title: &str,
    latest_compaction_message: Option<&ChatMessage>,
    selected_messages: &[ChatMessage],
) -> Conversation {
    let agent_id = branch_conversation_settings_agent_id_runtime(
        assistant_department_agent_id,
        department,
        &source.agent_id,
    );
    let mut conversation = build_conversation_record(
        &department_primary_api_config_id(department),
        &agent_id,
        &department.id,
        title,
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    conversation.parent_conversation_id = Some(source.id.clone());
    conversation.plan_mode_enabled = source.plan_mode_enabled;
    conversation.shell_workspace_path = source.shell_workspace_path.clone();
    conversation.shell_workspaces = source.shell_workspaces.clone();
    conversation.current_todos = source.current_todos.clone();
    let user_profile_snapshot = agents
        .iter()
        .find(|item| item.id == agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_warn(format!(
                    "[会话分支] 跳过，任务=构建用户画像快照，agent_id={}，error={}",
                    agent.id, err
                ));
                None
            }
        })
        .or_else(|| {
            let snapshot = source.user_profile_snapshot.trim();
            if snapshot.is_empty() {
                None
            } else {
                Some(snapshot.to_string())
            }
        });
    if let Some(snapshot) = user_profile_snapshot.clone() {
        conversation.user_profile_snapshot = snapshot;
    }
    if let Some(message) = latest_compaction_message {
        conversation
            .messages
            .push(clone_chat_message_for_copied_conversation(message));
    } else {
        conversation.messages.push(build_initial_summary_context_message(
            None,
            user_profile_snapshot.as_deref(),
            Some(&conversation.current_todos),
        ));
    }
    conversation.messages.extend(
        selected_messages
            .iter()
            .map(clone_chat_message_for_copied_conversation),
    );
    if let Some(last_message) = conversation.messages.last() {
        conversation.unread_count = 0;
        conversation.updated_at = last_message.created_at.clone();
        conversation.last_user_at = Some(last_message.created_at.clone());
    }
    conversation
}

fn read_latest_visible_foreground_conversation(
    state: &AppState,
) -> Result<Option<Conversation>, String> {
    let chat_index = state_read_chat_index_cached(state)?;
    Ok(chat_index
        .conversations
        .iter()
        .filter_map(|item| state_read_conversation_cached(state, item.id.as_str()).ok())
        .filter(|conversation| {
            conversation.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(conversation)
        })
        .max_by(|a, b| {
            a.updated_at
                .trim()
                .cmp(b.updated_at.trim())
                .then_with(|| a.created_at.trim().cmp(b.created_at.trim()))
                .then_with(|| a.id.cmp(&b.id))
        }))
}
