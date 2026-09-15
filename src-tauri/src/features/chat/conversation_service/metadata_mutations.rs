impl ConversationServiceV2 {
    fn rename_conversation(
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
        let should_update = with_conversation_mutation(
            state,
            normalized_conversation_id,
            "rename_conversation",
            || {
                ensure_unarchived_conversation_not_organizing(state, normalized_conversation_id)?;

                let conversation_meta =
                    self.get_conversation_meta(state, normalized_conversation_id)?;
                if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Err("未找到可改名的会话".to_string());
                }
                if self.conversation_meta_is_system_notification_meta_view(&conversation_meta) {
                    return Err("系统通知会话不支持改名".to_string());
                }
                Ok(conversation_meta.title.trim() != normalized_title)
            },
        )?;
        if should_update {
            self.set_title(state, normalized_conversation_id, normalized_title)?;
        }
        Ok(normalized_title.to_string())
    }

    async fn update_latest_summary_title(
        &self,
        state: &AppState,
        conversation_id: &str,
        next_title: &str,
    ) -> Result<bool, String> {
        let next_title_for_mutation = next_title.to_string();
        self.update_unarchived_conversation_by_id(state, conversation_id, move |conversation| {
            Ok(conversation_update_latest_summary_title(
                conversation,
                Some(next_title_for_mutation.as_str()),
            ))
        })
        .await
    }

    async fn update_latest_summary_title_with_source(
        &self,
        state: &AppState,
        conversation_id: &str,
        next_title: &str,
        title_source: &str,
    ) -> Result<bool, String> {
        let next_title_for_mutation = next_title.to_string();
        let title_source_for_mutation = title_source.to_string();
        self.update_unarchived_conversation_by_id(state, conversation_id, move |conversation| {
            Ok(conversation_update_latest_summary_title_with_source(
                conversation,
                Some(next_title_for_mutation.as_str()),
                Some(title_source_for_mutation.as_str()),
            ))
        })
        .await
    }

    fn toggle_conversation_pin(
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

        let main_conversation_id = state_service_get_main_conversation_id(state)?
            .map(|id| id.trim().to_string())
            .unwrap_or_default();
        if normalized_conversation_id == main_conversation_id {
            drop(guard);
            return Err("系统通知会话始终置顶".to_string());
        }
        let conversation = match self.get_conversation_meta(state, normalized_conversation_id) {
            Ok(conversation_meta) => conversation_meta,
            Err(_) => {
                drop(guard);
                return Err("未找到可置顶的会话".to_string());
            }
        };
        if self.conversation_meta_is_system_notification_meta_view(&conversation) {
            drop(guard);
            return Err("系统通知会话始终置顶".to_string());
        }
        if !self.conversation_meta_is_local_normal_chat_meta_view(&conversation) {
            drop(guard);
            return Err("未找到可置顶的会话".to_string());
        }

        let mut seen = std::collections::HashSet::<String>::new();
        let previous_pinned = state_service_get_pinned_conversation_ids(state)?
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .filter(|item| *item != main_conversation_id)
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
        state_service_set_pinned_conversation_ids(state, &next_pinned)?;
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

    fn set_preferred_api_config_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        preferred_api_config_id: Option<String>,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_preferred_api_config_id",
            ConversationExternalMetadataPatch {
                preferred_api_config_id: Some(preferred_api_config_id),
                ..Default::default()
            },
        )
    }

    fn set_auto_push_remote_contact_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        auto_push_remote_contact_id: Option<String>,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_auto_push_remote_contact_id",
            ConversationExternalMetadataPatch {
                auto_push_remote_contact_id: Some(auto_push_remote_contact_id),
                ..Default::default()
            },
        )
    }

    fn set_title(
        &self,
        state: &AppState,
        conversation_id: &str,
        next_title: &str,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_title",
            ConversationExternalMetadataPatch {
                title: Some(next_title.trim().to_string()),
                ..Default::default()
            },
        )
    }

    fn refresh_unarchived_conversation_overview(
        &self,
        state: &AppState,
    ) -> Result<UnarchivedConversationOverviewUpdatedPayload, String> {
        let unarchived_conversations =
            self.collect_unarchived_conversation_summaries_cached(state)?;
        Ok(UnarchivedConversationOverviewUpdatedPayload {
            preferred_conversation_id: unarchived_conversations
                .first()
                .map(|item| item.conversation_id.clone()),
            unarchived_conversations,
        })
    }

    fn list_unarchived_conversation_summaries(
        &self,
        state: &AppState,
    ) -> Result<ListUnarchivedConversationsMutationResult, String> {
        let summaries = self.collect_unarchived_conversation_summaries_cached(state)?;
        Ok(ListUnarchivedConversationsMutationResult { summaries })
    }

    fn set_active_conversation(
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
                if let Some(conversation_meta) = self.get_conversation_meta(state, conversation_id)
                    .ok()
                    .filter(|conversation_meta| {
                        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                            && conversation_meta.visible_in_foreground_lists
                    })
                {
                    (Some(conversation_meta), None, false)
                } else {
                    return Err(format!(
                        "Requested conversation not found: {conversation_id}"
                    ));
                }
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
        let conversation_id = target_conversation_meta
            .as_ref()
            .map(|conversation_meta| conversation_meta.id.to_string())
            .or_else(|| target_conversation.as_ref().map(|conversation| conversation.id.clone()))
            .ok_or_else(|| "Requested conversation not found.".to_string())?;
        drop(guard);
        clear_conversation_list_activity_mark(state, &conversation_id);
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
        Ok(conversation_id)
    }

    fn update_conversation_todos(
        &self,
        state: &AppState,
        conversation_id: &str,
        stored_todos: &[ConversationTodoItem],
    ) -> Result<Option<ConversationTodosUpdateResult>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(None);
        }
        let should_update = with_conversation_mutation(
            state,
            normalized_conversation_id,
            "update_conversation_todos",
            || {
                let conversation_meta = match self.get_conversation_meta(state, normalized_conversation_id) {
                    Ok(conversation) => conversation,
                    Err(err) => {
                        runtime_log_debug(format!(
                            "[Todo] 读取会话失败，函数=update_conversation_todos，conversation_id={}，error={}",
                            normalized_conversation_id, err
                        ));
                        return Ok(false);
                    }
                };
                if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Ok(false);
                }
                Ok(conversation_meta.current_todos != stored_todos)
            },
        )?;
        if !should_update {
            return Ok(None);
        }
        let updated = self.set_current_todos(
            state,
            normalized_conversation_id,
            stored_todos.to_vec(),
        )?;
        let current_todo = conversation_current_todo_text_from_items(&updated.current_todos);
        Ok(Some(ConversationTodosUpdateResult { current_todo }))
    }

    fn read_unarchived_conversation_summary(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<UnarchivedConversationSummary>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(None);
        }
        let guard = lock_conversation_with_metrics(state, "read_unarchived_conversation_summary")?;
        let main_conversation_id = state_service_get_main_conversation_id(state)?
            .map(|id| id.trim().to_string())
            .unwrap_or_default();
        let chat_index = state_read_chat_index_cached(state)?;
        let visible_ids = chat_index
            .conversations
            .iter()
            .filter(|item| !chat_index_item_is_archived(item))
            .map(|item| item.id.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<std::collections::HashSet<_>>();
        let conversation_meta = match self.get_conversation_meta(state, normalized_conversation_id) {
                Ok(conversation_meta) => conversation_meta,
                Err(err) => {
                    drop(guard);
                    runtime_log_error(format!(
                        "[会话索引读取] 状态=失败，任务=read_unarchived_conversation_summary，conversation_id={}，error={}",
                        normalized_conversation_id, err
                    ));
                    return Ok(None);
                }
            };
        let is_side_chat = conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT;
        if (!visible_ids.contains(normalized_conversation_id) && !is_side_chat)
            || !self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
            || (!conversation_meta.visible_in_foreground_lists && !is_side_chat)
        {
            drop(guard);
            return Ok(None);
        }
        let mut seen_pins = std::collections::HashSet::<String>::new();
        let pinned_conversation_ids = pinned_conversation_ids_downgraded(state)
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .filter(|item| *item != main_conversation_id)
            .filter(|item| visible_ids.contains(item))
            .filter(|item| seen_pins.insert(item.clone()))
            .collect::<Vec<_>>();
        let summary = build_unarchived_conversation_summary_from_meta_view(
            state,
            &main_conversation_id,
            &pinned_conversation_ids,
            &conversation_meta,
            Some(DESKTOP_CHAT_VIEWER_ID),
        );
        drop(guard);
        Ok(Some(summary))
    }

    fn set_current_todos(
        &self,
        state: &AppState,
        conversation_id: &str,
        current_todos: Vec<ConversationTodoItem>,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_current_todos",
            ConversationExternalMetadataPatch {
                current_todos: Some(current_todos),
                ..Default::default()
            },
        )
    }

    fn set_shell_workspace(
        &self,
        state: &AppState,
        conversation_id: &str,
        shell_workspace_path: Option<Option<String>>,
        shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
        shell_autonomous_mode: Option<bool>,
        shell_work_mode: Option<String>,
        shell_work_branch: Option<String>,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_shell_workspace",
            ConversationExternalMetadataPatch {
                shell_workspace_path,
                shell_workspaces,
                shell_autonomous_mode,
                shell_work_mode,
                shell_work_branch,
                ..Default::default()
            },
        )
    }

    fn update_shell_workspace(
        &self,
        state: &AppState,
        conversation_id: &str,
        shell_workspace_path: Option<Option<String>>,
        shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
        shell_autonomous_mode: Option<bool>,
        shell_work_mode: Option<String>,
        shell_work_branch: Option<String>,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("指定会话不存在：".to_string());
        }
        let conversation = self
            .read_persisted_conversation(state, normalized_conversation_id)
            .map_err(|_| format!("指定会话不存在：{normalized_conversation_id}"))?;
        let original_path = conversation.shell_workspace_path.clone();
        let original_workspaces = conversation.shell_workspaces.clone();
        let original_autonomous_mode = conversation.shell_autonomous_mode;
        let original_work_mode = conversation.shell_work_mode.clone();
        let original_branch = conversation.shell_work_branch.clone();
        let updated = self.set_shell_workspace(
            state,
            normalized_conversation_id,
            shell_workspace_path,
            shell_workspaces,
            shell_autonomous_mode,
            shell_work_mode,
            shell_work_branch,
        )?;
        if updated.shell_workspace_path == original_path
            && updated.shell_workspaces == original_workspaces
            && updated.shell_autonomous_mode == original_autonomous_mode
            && updated.shell_work_mode == original_work_mode
            && updated.shell_work_branch == original_branch
        {
            return Ok(updated);
        }
        Ok(updated)
    }

    fn add_conversation_cumulative_usage_delta(
        &self,
        state: &AppState,
        conversation_id: &str,
        provider_key: Option<&str>,
        model_name: Option<&str>,
        usage: &Value,
    ) -> Result<bool, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(false);
        }
        let mut probe = ConversationCumulativeUsage::default();
        if !conversation_cumulative_usage_add_provider_usage(
            &mut probe,
            provider_key,
            model_name,
            usage,
        ) {
            return Ok(false);
        }
        let (conversation, changed) = with_conversation_mutation(
            state,
            normalized_conversation_id,
            "add_conversation_cumulative_usage_delta",
            || {
                let (conversation, changed, _) = state_update_conversation_metadata_cached_unlocked(
                    state,
                    normalized_conversation_id,
                    |conversation| {
                        Ok(conversation_cumulative_usage_add_provider_usage(
                            &mut conversation.cumulative_usage,
                            provider_key,
                            model_name,
                            usage,
                        ))
                    },
                )?;
                Ok((conversation, changed))
            },
        )?;
        if changed {
            emit_provider_context_usage_update_from_conversation(state, &conversation, usage);
            usage_trail_record_conversation_delta(state, &conversation, provider_key, model_name, usage);
        }
        Ok(changed)
    }

}
