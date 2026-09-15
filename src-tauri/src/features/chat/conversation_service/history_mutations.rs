impl ConversationServiceV2 {
    fn delete_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<DeleteUnarchivedConversationMutationResult, String> {
        struct DeleteConversationPreparation {
            child_conversation_ids: Vec<String>,
            active_conversation_id: String,
            should_create_system_notification: bool,
            should_set_main_to_system_notification: bool,
            parent_conversation_id: Option<String>,
        }

        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let preparation = with_conversation_mutation(
            state,
            normalized_conversation_id,
            "delete_conversation",
            || {
                let main_conversation_id = state_service_get_main_conversation_id(state)?
                    .map(|value| value.trim().to_string())
                    .unwrap_or_default();
                if normalized_conversation_id == main_conversation_id {
                    return Err("系统通知会话暂不支持删除".to_string());
                }
                let conversation = self.get_conversation_meta(state, normalized_conversation_id).ok();
                let child_conversation_ids = conversation
                    .as_ref()
                    .map(|item| item.child_conversation_ids.clone())
                    .unwrap_or_default();
                if conversation
                    .as_ref()
                    .map(|conversation| self.conversation_meta_is_system_notification_meta_view(conversation))
                    .unwrap_or(false)
                {
                    return Err("系统通知会话暂不支持删除".to_string());
                }
                let chat_index = state_read_chat_index_cached(state)?;
                let active_conversation_id = chat_index
                    .conversations
                    .iter()
                    .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
                    .find(|conversation_meta| {
                        conversation_meta.id != normalized_conversation_id
                            && self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                            && conversation_meta.visible_in_foreground_lists
                            && conversation_meta.status.trim() == "active"
                    })
                    .map(|conversation_meta| conversation_meta.id.to_string())
                    .or_else(|| {
                        chat_index
                            .conversations
                            .iter()
                            .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
                            .find(|conversation_meta| {
                                conversation_meta.id != normalized_conversation_id
                                    && self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                                    && conversation_meta.visible_in_foreground_lists
                            })
                            .map(|conversation_meta| conversation_meta.id.to_string())
                    })
                    .unwrap_or_default();
                let system_notification_exists = if active_conversation_id.trim().is_empty() {
                    self.get_conversation_meta(state, SYSTEM_NOTIFICATION_CONVERSATION_ID)
                        .ok()
                        .filter(|conversation_meta| {
                            self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                                && conversation_meta.visible_in_foreground_lists
                                && self.conversation_meta_is_system_notification_meta_view(conversation_meta)
                        })
                        .is_some()
                } else {
                    true
                };
                let should_set_main_to_system_notification = active_conversation_id.trim().is_empty()
                    && main_conversation_id.trim() != SYSTEM_NOTIFICATION_CONVERSATION_ID;
                let parent_conversation_id = conversation
                    .as_ref()
                    .and_then(|item| item.parent_conversation_id.clone())
                    .filter(|id| !id.trim().is_empty());

                Ok(DeleteConversationPreparation {
                    child_conversation_ids,
                    active_conversation_id: if active_conversation_id.trim().is_empty() {
                        SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string()
                    } else {
                        active_conversation_id
                    },
                    should_create_system_notification: !system_notification_exists,
                    should_set_main_to_system_notification,
                    parent_conversation_id,
                })
            },
        )?;

        mark_tasks_as_session_lost(&state.data_path, normalized_conversation_id);
        if preparation.should_create_system_notification {
            let system_notification = build_system_notification_conversation_record();
            state_schedule_conversation_persist(state, &system_notification)?;
        }
        if preparation.should_set_main_to_system_notification {
            state_service_set_main_conversation_id(
                state,
                Some(SYSTEM_NOTIFICATION_CONVERSATION_ID),
            )?;
        }
        if let Ok(cleanup_conversation) =
            read_conversation_for_backup_cleanup(state, normalized_conversation_id)
        {
            match cleanup_backup_records_from_messages(&state.data_path, &cleanup_conversation.messages) {
                Ok(cleaned) if cleaned > 0 => {
                    runtime_log_info(format!(
                        "[会话删除] apply_patch 备份清理完成: conversation={}, cleaned={}",
                        normalized_conversation_id, cleaned
                    ));
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[会话删除] apply_patch 备份清理失败: conversation={}, error={}",
                        normalized_conversation_id, err
                    ));
                }
                _ => {}
            }
        }
        state_schedule_conversation_delete(state, normalized_conversation_id)?;
        for child_conversation_id in preparation.child_conversation_ids {
            if child_conversation_id.trim().is_empty() {
                continue;
            }
            if let Err(err) = state_schedule_conversation_delete(state, &child_conversation_id) {
                runtime_log_warn(format!(
                    "[会话删除] 跳过，任务=级联删除追问会话，parent_conversation_id={}，conversation_id={}，error={}",
                    normalized_conversation_id, child_conversation_id, err
                ));
            }
            clear_conversation_list_activity_mark(state, &child_conversation_id);
        }
        if let Some(parent_conversation_id) = preparation.parent_conversation_id {
            if let Err(err) = state_update_conversation_metadata_cached(
                state,
                &parent_conversation_id,
                |parent| {
                    parent
                        .child_conversation_ids
                        .retain(|id| id != normalized_conversation_id);
                    Ok(())
                },
            ) {
                runtime_log_warn(format!(
                    "[会话删除] 跳过，任务=移除父会话子关系，conversation_id={}，parent_conversation_id={}，error={}",
                    normalized_conversation_id, parent_conversation_id, err
                ));
            }
        }
        clear_conversation_list_activity_mark(state, normalized_conversation_id);
        // 删除不重建全量概览：命令层注册 watermark 删除语义，前端差量同步收敛。
        Ok(DeleteUnarchivedConversationMutationResult {
            deleted_conversation_id: normalized_conversation_id.to_string(),
            active_conversation_id: preparation.active_conversation_id,
        })
    }

    fn rewind_conversation(
        &self,
        state: &AppState,
        input: &RewindConversationInput,
        message_id: &str,
        started_at: &std::time::Instant,
    ) -> Result<RewindConversationMutationResult, String> {
        let requested_conversation_id = trimmed_option(input.session.conversation_id.as_deref());
        let Some(requested_conversation_id) = requested_conversation_id.as_deref() else {
            return Err("conversationId is required.".to_string());
        };
        with_conversation_mutation(
            state,
            requested_conversation_id,
            "rewind_conversation",
            || {
                let conversation_meta = self
                    .get_conversation_meta(state, requested_conversation_id)
                    .map_err(|_| "Target message not found in active conversation.".to_string())?;
                let conversation_id = if self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
                    && (conversation_meta.visible_in_foreground_lists
                        || conversation_meta.is_remote_im_contact
                        || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT)
                {
                    conversation_meta.id.to_string()
                } else {
                    return Err("Target message not found in active conversation.".to_string());
                };
                let runtime_state = get_conversation_runtime_state(state, &conversation_id)?;
                if runtime_state != MainSessionState::Idle {
                    let runtime_state_text = match runtime_state {
                        MainSessionState::Idle => "空闲",
                        MainSessionState::AssistantStreaming => "助理流式输出",
                        MainSessionState::OrganizingContext => "整理上下文",
                    };
                    runtime_log_error(format!(
                        "[会话撤回] 失败，任务=rewind_conversation_from_message，conversation_id={}，原因=会话运行中，runtime_state={}",
                        conversation_id, runtime_state_text
                    ));
                    return Err("当前会话正在运行或整理上下文，完成后再撤回。".to_string());
                }
                let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
                ensure_chat_store_conversation_readable(state, &conversation_id, &store_paths)?;
                let rewind_state =
                    read_chat_store_rewind_state_meta_view(state, &store_paths, &conversation_meta, message_id)?;
                if is_context_compaction_message(
                    &rewind_state.recalled_user_message,
                    rewind_state.recalled_user_message.role.trim(),
                ) && Self::is_first_context_compaction_message_in_store(&store_paths, message_id)?
                {
                    return Err("不能撤回会话的第一条摘要消息。".to_string());
                }
                let git_snapshot = read_git_snapshot_record_from_provider_meta(
                    rewind_state.recalled_user_message.provider_meta.as_ref(),
                );
                maybe_undo_rewind_apply_patch(
                    state,
                    input,
                    &rewind_state.removed_messages,
                    message_id,
                    started_at,
                )?;
                let updated_at = now_iso();
                let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
                    state,
                    &conversation_id,
                    |cached| {
                        cached.apply_truncated_rewind_state(
                            rewind_state.keep_count,
                            rewind_state.remaining_todos.clone(),
                            updated_at.clone(),
                            rewind_state.remaining_last_user_at.clone(),
                            rewind_state.remaining_last_assistant_at.clone(),
                            rewind_state.remaining_last_message_id.clone(),
                            rewind_state.remaining_last_message_at.clone(),
                            rewind_state.remaining_body_message_count,
                            rewind_state.remaining_body_text_length,
                            rewind_state.remaining_last_assistant_at.is_some(),
                            rewind_state.remaining_has_context_compaction_message,
                            rewind_state.remaining_latest_summary_title.clone(),
                            rewind_state.remaining_preview_messages.clone(),
                        );
                        Ok(())
                    },
                )?;
                let metadata_conversation =
                    self.build_conversation_snapshot_from_meta(&updated_meta, Vec::new());
                state_upsert_chat_index_conversation_cached(state, &metadata_conversation)?;
                let current_todo = conversation_current_todo_text_from_items(&rewind_state.remaining_todos);
                message_store::chat_store_truncate_messages_from_meta(
                    &store_paths,
                    &updated_meta,
                    rewind_state.keep_count,
                )?;
                self.mark_conversation_metadata_cached_persisted(state, &conversation_id)?;
                emit_rewind_completed_event(
                    state,
                    &conversation_id,
                    message_id,
                    rewind_state.remaining_last_message_id.clone(),
                    rewind_state.removed_messages.len(),
                    rewind_state.keep_count,
                );
                Ok(RewindConversationMutationResult {
                    conversation_id,
                    removed_count: rewind_state.removed_messages.len(),
                    remaining_count: rewind_state.keep_count,
                    current_todo,
                    current_todos: rewind_state.remaining_todos,
                    recalled_user_message: Some(rewind_state.recalled_user_message),
                    git_snapshot,
                })
            },
        )
    }

    fn is_first_context_compaction_message_in_store(
        store_paths: &message_store::MessageStorePaths,
        message_id: &str,
    ) -> Result<bool, String> {
        let mut before_message_id = message_id.trim().to_string();
        while !before_message_id.is_empty() {
            let Some(page) = message_store::chat_store_read_messages_before(
                store_paths,
                &before_message_id,
                4,
            )?
            else {
                break;
            };
            if page.messages.is_empty() {
                break;
            }
            if page
                .messages
                .iter()
                .any(|message| is_context_compaction_message(message, message.role.trim()))
            {
                return Ok(false);
            }
            if !page.has_more {
                break;
            }
            before_message_id = page
                .messages
                .first()
                .map(|message| message.id.trim().to_string())
                .unwrap_or_default();
        }
        Ok(true)
    }

    fn preview_rewind_conversation(
        &self,
        state: &AppState,
        input: &RewindConversationInput,
        message_id: &str,
    ) -> Result<RewindConversationPreviewResult, String> {
        let guard = lock_conversation_with_metrics(state, "preview_rewind_conversation")?;

        let requested_conversation_id = trimmed_option(input.session.conversation_id.as_deref());
        let Some(requested_conversation_id) = requested_conversation_id.as_deref() else {
            drop(guard);
            return Err("conversationId is required.".to_string());
        };
        let conversation_meta = self
            .get_conversation_meta(state, requested_conversation_id)
            .map_err(|_| "Target message not found in active conversation.".to_string())?;        let conversation_id = if self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
            && (conversation_meta.visible_in_foreground_lists
                || conversation_meta.is_remote_im_contact)
        {
            conversation_meta.id.to_string()
        } else {
            drop(guard);
            return Err("Target message not found in active conversation.".to_string());        };
        let runtime_state = get_conversation_runtime_state(state, &conversation_id)?;
        if runtime_state != MainSessionState::Idle {
            drop(guard);
            return Ok(RewindConversationPreviewResult {
                conversation_id,
                can_undo_patch: false,
                hint: "当前会话正在运行或整理上下文，完成后再撤回。".to_string(),
            });
        }
        drop(guard);
        let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
        ensure_chat_store_conversation_readable(state, &conversation_id, &store_paths)?;
        let rewind_state =
            read_chat_store_rewind_state_meta_view(state, &store_paths, &conversation_meta, message_id)?;
        let backup_record_ids = collect_backup_record_ids_from_messages(&rewind_state.removed_messages);
        let existing_backup_count = backup_record_ids
            .iter()
            .filter(|record_id| apply_patch_record_path(&state.data_path, record_id).exists())
            .count();
        runtime_log_debug(format!(
            "[会话撤回] 预览诊断，任务=preview_rewind_conversation，conversation_id={}，message_id={}，removed_messages={}，backup_record_ids={}，existing_backup_count={}，missing_backup_count={}",
            conversation_id,
            message_id,
            rewind_state.removed_messages.len(),
            backup_record_ids.len(),
            existing_backup_count,
            backup_record_ids.len().saturating_sub(existing_backup_count)
        ));

        if existing_backup_count > 0 {
            return Ok(RewindConversationPreviewResult {
                conversation_id,
                can_undo_patch: true,
                hint: String::new(),
            });
        }
        let hint = if backup_record_ids.is_empty() {
            "该范围内没有检测到可撤回的工具修改。"
        } else {
            "检测到工具修改记录，但对应备份已不存在，无法撤回文件修改。"
        };
        Ok(RewindConversationPreviewResult {
            conversation_id,
            can_undo_patch: false,
            hint: hint.to_string(),
        })
    }

    fn branch_conversation_from_selection(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        selected_message_ids: &[String],
    ) -> Result<BranchUnarchivedConversationMutationResult, String> {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        let agents = runtime_snapshot.agents.clone();
        let source_conversation_meta = self
            .get_conversation_meta(state, source_conversation_id)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_local_conversation_runtime_meta_view(conversation_meta)
            })
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let selection =
            read_branch_selection_or_pending_conversation(state, source_conversation_id, selected_message_ids)?;
        let selected_messages = selection.selected_messages;
        let first_selected_ordinal = selection.first_selected_ordinal;
        if selected_messages.is_empty() {
            drop(guard);
            return Err("未找到可创建会话分支的已选消息".to_string());
        }
        let branch_summary_title = build_branch_conversation_summary_title(
            &source_conversation_meta.title,
            source_conversation_meta.latest_summary_title.as_deref(),
            Some(first_selected_ordinal.max(1)),
            main_conversation_id_downgraded(state).as_deref().map(str::trim)
                == Some(source_conversation_meta.id.as_str()),
        );
        let latest_compaction_message = selection.latest_compaction_message;
        let conversation = build_branch_conversation_record_from_selection_runtime_meta_view(
            &state.data_path,
            &agents,
            &source_conversation_meta,
            &branch_summary_title,
            latest_compaction_message.as_ref(),
            &selected_messages,
        )?;
        let conversation_id = conversation.id.clone();
        drop(guard);
        state_schedule_conversation_persist(state, &conversation)?;
        Ok(BranchUnarchivedConversationMutationResult {
            conversation_id,
            title: branch_summary_title,
            selected_count: selected_messages.len(),
            has_compaction_seed: latest_compaction_message.is_some(),
        })
    }

    async fn forward_conversation_selection(
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
        let _source_conversation = self
            .get_conversation_meta(state, source_conversation_id)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_local_conversation_runtime_meta_view(conversation_meta)
            })
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let selection =
            read_branch_selection_or_pending_conversation(state, source_conversation_id, selected_message_ids)?;
        let selected_messages = selection.selected_messages;
        if selected_messages.is_empty() {
            drop(guard);
            return Err("未找到可转发到会话的已选消息".to_string());
        }
        let _target_conversation = self
            .get_conversation_meta(state, target_conversation_id)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_local_conversation_runtime_meta_view(conversation_meta)
            })
            .ok_or_else(|| "目标会话不存在或已归档".to_string())?;
        drop(guard);
        let copied_messages = selected_messages
            .iter()
            .map(clone_chat_message_for_copied_conversation)
            .collect::<Vec<_>>();
        conversation_service_v2()
            .append_messages(state, target_conversation_id, &copied_messages)
            .await?;
        // 转发不重建全量概览：目标会话变更由命令层单项事件推送。
        Ok(ForwardUnarchivedConversationMutationResult {
            target_conversation_id: target_conversation_id.to_string(),
            forwarded_count: selected_messages.len(),
        })
    }

    fn forward_selection_to_remote_im_contact(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        target_conversation_id: &str,
        remote_contact_id: &str,
        selected_message_ids: &[String],
    ) -> Result<ForwardSelectionToRemoteImContactMutationResult, String> {
        let normalized_remote_contact_id = remote_contact_id.trim();
        if normalized_remote_contact_id.is_empty() {
            return Err("remoteContactId 不能为空".to_string());
        }
        let app_config = state_read_config_cached(state)?;
        let _source_conversation = self
            .get_conversation_meta(state, source_conversation_id)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_local_conversation_runtime_meta_view(conversation_meta)
            })
            .ok_or_else(|| "源会话不存在或已归档".to_string())?;
        let selection =
            read_branch_selection_or_pending_conversation(state, source_conversation_id, selected_message_ids)?;
        let selected_messages = selection.selected_messages;
        if selected_messages.is_empty() {
            return Err("未找到可推送到远程联系人的已选消息".to_string());
        }

        let _target_conversation = self
            .get_conversation_meta(state, target_conversation_id)
            .ok()
            .filter(|conversation_meta| conversation_meta.is_remote_im_contact)
            .ok_or_else(|| "目标远程联系人会话不存在".to_string())?;
        let contact = state_service_get_remote_im_contact(state, normalized_remote_contact_id)?
            .ok_or_else(|| "目标远程联系人不存在".to_string())?;
        if contact.bound_conversation_id.as_deref().map(str::trim) != Some(target_conversation_id) {
            return Err("远程联系人与目标会话不匹配".to_string());
        }
        if !contact.allow_send {
            return Err("当前联系人不允许发送消息".to_string());
        }
        let channel = remote_im_channel_by_id(&app_config, &contact.channel_id)
            .cloned()
            .ok_or_else(|| format!("远程 IM 渠道不存在: {}", contact.channel_id))?;
        if !channel.enabled {
            return Err(format!("远程 IM 渠道未启用: {}", contact.channel_id));
        }

        let notification_message = self.build_forward_selection_notification_message(
            state,
            source_conversation_id,
            &selected_messages,
        )?;
        let notification_body = notification_message
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.trim().to_string()),
                _ => None,
            })
            .filter(|text: &String| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        enqueue_session_notification_dispatch(
            state,
            target_conversation_id,
            &notification_body,
            &notification_message,
            "forward_selection_to_remote_im_contact",
        )?;
        // 转发不重建全量概览：目标会话变更由命令层单项事件推送。
        Ok(ForwardSelectionToRemoteImContactMutationResult {
            target_conversation_id: target_conversation_id.to_string(),
            remote_contact_id: normalized_remote_contact_id.to_string(),
            forwarded_count: selected_messages.len(),
        })
    }

}

// ========== 会话撤回广播 ==========

fn emit_rewind_completed_event(
    state: &AppState,
    conversation_id: &str,
    target_message_id: &str,
    remaining_last_message_id: Option<String>,
    removed_count: usize,
    remaining_count: usize,
) {
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "targetMessageId": target_message_id,
        "remainingLastMessageId": remaining_last_message_id,
        "removedCount": removed_count,
        "remainingCount": remaining_count,
    });
    ide_chat_broadcast_notification("chat.rewindCompleted", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!(
            "[会话撤回] 广播 rewind_completed 跳过: app_handle unavailable, conversation_id={}",
            conversation_id
        ));
        return;
    };
    if let Err(err) = app_handle.emit(CHAT_REWIND_COMPLETED_EVENT, payload) {
        runtime_log_error(format!(
            "[会话撤回] 广播 rewind_completed 失败: conversation_id={}, error={}",
            conversation_id, err
        ));
    }
}
