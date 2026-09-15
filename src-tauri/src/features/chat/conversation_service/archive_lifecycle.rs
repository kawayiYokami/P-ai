const REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT: usize = 14;

#[derive(Debug, Clone)]
enum RemoteImDynamicWakeCompactionOutcome {
    Applied,
    SkippedLowFrequency { block_message_count: usize },
}

fn remote_im_wake_compaction_should_skip_for_low_frequency(block_message_count: usize) -> bool {
    block_message_count < REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
}

impl ConversationServiceV2 {
    fn list_archives(
        &self,
        state: &AppState,
    ) -> Result<Vec<ArchiveSummary>, String> {
        self.list_archives_with_resolvers(
            state,
            |archive_id| self.get_conversation_meta(state, archive_id),
            |archive_id| {
                let store_paths =
                    message_store::message_store_paths(&state.data_path, archive_id).ok()?;
                message_store::chat_store_read_index_summary(&store_paths)
                    .ok()
                    .flatten()
                    .and_then(|summary| summary.first_user_text_preview)
                    .filter(|value| !value.trim().is_empty())
            },
        )
    }

    fn list_archives_with_resolvers<LoadMeta, ResolveTitle>(
        &self,
        state: &AppState,
        load_meta: LoadMeta,
        resolve_title: ResolveTitle,
    ) -> Result<Vec<ArchiveSummary>, String>
    where
        LoadMeta: Fn(&str) -> Result<ConversationMetaView, String>,
        ResolveTitle: Fn(&str) -> Option<String>,
    {
        // 元数据缓存校验和标题兜底都可能触发磁盘访问，不能包在全局会话锁内。
        let candidate_index = state_read_chat_index_cached(state)?;
        let archive_metas = candidate_index
            .conversations
            .iter()
            .filter(|item| chat_index_item_is_archived(item))
            .filter_map(|item| match load_meta(item.id.as_str()) {
                Ok(conversation_meta) => Some(conversation_meta),
                Err(err) => {
                    runtime_log_error(format!(
                        "[会话索引读取] 状态=失败，任务=list_archives，conversation_id={}，error={}",
                        item.id, err
                    ));
                    None
                }
            })
            .collect::<Vec<_>>();

        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        // 锁外预读可能与并发归档交错；锁内只复核当前仍归档的 ID，避免返回已取消归档项。
        let current_archived_ids = {
            let guard = lock_conversation_with_metrics(state, "list_archives_with_resolvers")?;
            let current_index = state_read_chat_index_cached(state)?;
            let archived_ids = current_index
                .conversations
                .iter()
                .filter(|item| chat_index_item_is_archived(item))
                .map(|item| item.id.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<std::collections::HashSet<_>>();
            drop(guard);
            archived_ids
        };
        let mut summaries = archive_metas
            .into_iter()
            .filter(|archive_meta| current_archived_ids.contains(archive_meta.id.trim()))
            .filter(|archive_meta| archive_meta.status.trim() == "archived")
            .map(|archive_meta| {
                let api_config_id = agent_by_id(&runtime_snapshot.agents, archive_meta.agent_id.as_str())
                    .map(agent_primary_api_config_id)
                    .unwrap_or_default();
                let title = archive_meta.title.trim().to_string();
                ArchiveSummary {
                    archive_id: archive_meta.id.to_string(),
                    archived_at: archive_meta
                        .archived_at
                        .clone()
                        .unwrap_or_else(|| archive_meta.updated_at.to_string()),
                    title,
                    message_count: archive_meta.message_count,
                    api_config_id,
                    agent_id: archive_meta.agent_id.to_string(),
                }
            })
            .collect::<Vec<_>>();
        for summary in &mut summaries {
            if summary.title.is_empty() {
                summary.title = resolve_title(&summary.archive_id)
                    .unwrap_or_else(|| "无内容".to_string());
            }
        }
        summaries.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
        Ok(summaries)
    }

    fn get_archive_messages(
        &self,
        state: &AppState,
        archive_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_archive_id = archive_id.trim();
        if normalized_archive_id.is_empty() {
            return Err("archiveId is required".to_string());
        }
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_archive_id)?;
        if let Some(mut messages) =
            message_store::chat_store_read_all_messages(&store_paths)?
        {
            materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
            return Ok(messages);
        }
        Err(format!(
            "归档消息仓库不可读，archive_id={normalized_archive_id}；请先完成消息存储迁移"
        ))
    }

    fn get_archive_block_page(
        &self,
        state: &AppState,
        archive_id: &str,
        block_id: Option<u32>,
    ) -> Result<ConversationBlockPageResult, String> {
        let normalized_archive_id = archive_id.trim();
        if normalized_archive_id.is_empty() {
            return Err("archiveId is required".to_string());
        }
        let store_paths = message_store::message_store_paths(&state.data_path, normalized_archive_id)?;
        if let Some(page) = message_store::chat_store_read_block_page(&store_paths, block_id)? {
            let mut messages = page.messages;
            materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
            return Ok(ConversationBlockPageResult {
                blocks: page
                    .blocks
                    .into_iter()
                    .map(|item| ConversationBlockSummaryResult {
                        block_id: item.block_id,
                        message_count: item.message_count,
                        first_message_id: item.first_message_id,
                        last_message_id: item.last_message_id,
                        first_created_at: item.first_created_at,
                        last_created_at: item.last_created_at,
                        is_latest: item.is_latest,
                    })
                    .collect(),
                selected_block_id: page.selected_block_id,
                messages,
                has_prev_block: page.has_prev_block,
                has_next_block: page.has_next_block,
            });
        }

        Err(format!(
            "归档块分页不可读，archive_id={normalized_archive_id}；请先完成消息存储迁移"
        ))
    }

    fn delete_archive(
        &self,
        state: &AppState,
        archive_id: &str,
    ) -> Result<(), String> {
        let normalized_archive_id = archive_id.trim();
        if normalized_archive_id.is_empty() {
            return Err("archiveId is required".to_string());
        }
        with_conversation_mutation(state, normalized_archive_id, "delete_archive", || {
            let conversation_meta = self
                .get_conversation_meta(state, normalized_archive_id)
                .map_err(|_| "Archive not found".to_string())?;
            if conversation_meta.status.trim() != "archived" {
                return Err("Archive not found".to_string());
            }
            state_schedule_conversation_delete(state, normalized_archive_id)?;
            Ok(())
        })
    }

    fn unarchive_archive(
        &self,
        state: &AppState,
        archive_id: &str,
    ) -> Result<(), String> {
        let normalized_archive_id = archive_id.trim();
        if normalized_archive_id.is_empty() {
            return Err("archiveId is required".to_string());
        }
        let conversation_id = with_conversation_mutation(state, normalized_archive_id, "unarchive_archive", || {
            let conversation_meta = self
                .get_conversation_meta(state, normalized_archive_id)
                .map_err(|_| "Archive not found".to_string())?;
            if conversation_meta.status.trim() != "archived"
                || !conversation_meta.visible_in_foreground_lists
                || conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_CHAT
            {
                return Err("该归档会话无法恢复为普通会话".to_string());
            }

            let now = now_iso();
            let (conversation, (), _) = state_update_conversation_metadata_cached_unlocked(
                state,
                normalized_archive_id,
                |conversation| {
                    conversation.status = "active".to_string();
                    conversation.archived_at = None;
                    conversation.updated_at = now.clone();
                    Ok(())
                },
            )?;
            Ok(conversation.id)
        })?;
        runtime_log_info(format!(
            "[归档] 完成，任务=取消归档，conversation_id={}",
            conversation_id
        ));
        Ok(())
    }

    fn resolve_archive_request_conversation_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<(ApiConfig, ResolvedApiConfig, Conversation, String), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required".to_string());
        }
        let guard = lock_conversation_with_metrics(state, "resolve_archive_request_conversation_by_id")?;
        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        let app_config = &runtime_snapshot.config;
        let source_meta = self
            .get_conversation_meta(state, normalized_conversation_id)
            .map_err(|_| "当前没有可归档的活动对话。".to_string())?;
        if !self.conversation_meta_is_local_normal_chat_meta_view(&source_meta)
            && source_meta.status.trim() != "archived"
        {
            drop(guard);
            return Err("当前没有可归档的活动对话。".to_string());
        }
        let effective_agent_id = source_meta.agent_id.trim();
        let effective_agent_id = if effective_agent_id.is_empty() {
            runtime_log_warn(format!(
                "[归档] 跳过人格校验，任务=resolve_archive_request_conversation_by_id，conversation_id={}，原因=会话未绑定人格，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
                source_meta.id
            ));
            String::new()
        } else if runtime_snapshot
            .agents
            .iter()
            .any(|agent| agent.id == effective_agent_id && !agent.is_built_in_user)
        {
            effective_agent_id.to_string()
        } else {
            runtime_log_warn(format!(
                "[归档] 跳过人格校验，任务=resolve_archive_request_conversation_by_id，conversation_id={}，agent_id={}，原因=会话绑定人格不存在或不可用，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
                source_meta.id, effective_agent_id
            ));
            effective_agent_id.to_string()
        };
        let preferred_api_id = source_meta
            .preferred_api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|api_id| resolve_chat_api_config_id(app_config, api_id));
        let selected_api_id = preferred_api_id.or_else(|| {
            agent_by_id(&runtime_snapshot.agents, &effective_agent_id)
                .and_then(|agent| agent_primary_chat_api_config_id(app_config, agent))
        });
        let selected_api = resolve_selected_api_config(app_config, selected_api_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(app_config, Some(selected_api.id.as_str()))?;
        let source = self.get_conversation_snapshot(state, &source_meta.id)?;
        drop(guard);
        Ok((selected_api, resolved_api, source, effective_agent_id))
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
        let mut main_conversation_id = state_service_get_main_conversation_id(state)?;
        let assistant_agent_id = state_service_get_assistant_agent_id(state)?;
        let agents = state_read_agents_cached(state)?;
        let source_conversation = read_conversation_for_backup_cleanup(state, &source.id)
            .map_err(|_| "活动对话已变化，请重试归档。".to_string())?;
        if !conversation_is_archived(&source_conversation) || conversation_is_delegate(&source_conversation) {
            drop(guard);
            return Err("活动对话已变化，请重试归档。".to_string());
        }
        match cleanup_backup_records_from_messages(&state.data_path, &source_conversation.messages) {
            Ok(cleaned) if cleaned > 0 => {
                runtime_log_info(format!(
                    "[会话删除] apply_patch 备份清理完成: conversation={}, cleaned={}",
                    source.id, cleaned
                ));
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[会话删除] apply_patch 备份清理失败: conversation={}, error={}",
                    source.id, err
                ));
            }
            _ => {}
        }
        drop(guard);
        state_schedule_conversation_delete(state, &source.id)?;
        let system_notification_exists = self
            .get_conversation_meta(state, SYSTEM_NOTIFICATION_CONVERSATION_ID)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                    && conversation_meta.visible_in_foreground_lists
                    && self.conversation_meta_is_system_notification_meta_view(conversation_meta)
            })
            .is_some();
        if !system_notification_exists {
            let system_notification = build_system_notification_conversation_record();
            state_schedule_conversation_persist(state, &system_notification)?;
        }
        if main_conversation_id.as_deref().map(str::trim)
            != Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        {
            main_conversation_id = Some(SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string());
            state_service_set_main_conversation_id(state, main_conversation_id.as_deref())?;
        }
        let chat_index = state_read_chat_index_cached(state)?;
        let active_conversation_id = chat_index
            .conversations
            .iter()
            .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
            .find(|conversation_meta| {
                conversation_meta.id != source.id
                    && !conversation_meta.is_delegate
                    && self.conversation_meta_is_local_normal_chat_meta_view(conversation_meta)
            })
            .map(|conversation_meta| conversation_meta.id.to_string());
        let active_conversation_id = if let Some(active_conversation_id) = active_conversation_id {
            active_conversation_id
        } else {
            let replacement = build_archive_replacement_conversation(
                state,
                &agents,
                &assistant_agent_id,
                selected_api,
                &source_conversation,
            )?;
            let replacement_id = replacement.id.clone();
            state_schedule_conversation_persist(state, &replacement)?;
            replacement_id
        };
        cleanup_pdf_session_memory_cache_for_conversation(&source.id);
        Ok(active_conversation_id)
    }

    fn remote_im_apply_dynamic_wake_compaction(
        &self,
        state: &AppState,
        conversation_id: &str,
        trigger_message_id: &str,
        include_history: bool,
    ) -> Result<RemoteImDynamicWakeCompactionOutcome, String> {
        let conversation_id = conversation_id.trim();
        let trigger_message_id = trigger_message_id.trim();
        if conversation_id.is_empty() || trigger_message_id.is_empty() {
            return Err("远程唤醒压缩失败：缺少会话或触发消息 ID".to_string());
        }
        with_conversation_mutation(
            state,
            conversation_id,
            "remote_im_apply_dynamic_wake_compaction",
            || {
        let conversation_meta = self.get_conversation_meta(state, conversation_id)?;
        if !conversation_meta.is_remote_im_contact {
            return Err(format!(
                "远程唤醒压缩失败：目标不是远程联系人会话，conversation_id={conversation_id}"
            ));
        }
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        ensure_chat_store_conversation_readable(state, conversation_id, &store_paths)?;
        let trigger = message_store::chat_store_read_message_by_id(
            &store_paths,
            trigger_message_id,
        )?
        .ok_or_else(|| format!("远程唤醒压缩失败：触发消息不存在，message_id={trigger_message_id}"))?;
        let trigger_index = message_store::chat_store_read_message_sequence(
            &store_paths,
            trigger_message_id,
        )?
        .ok_or_else(|| format!("远程唤醒压缩失败：触发消息缺少序号，message_id={trigger_message_id}"))?;
        if include_history {
            match message_store::chat_store_read_block_message_count(
                &store_paths,
                trigger_message_id,
            ) {
                Ok(Some(block_message_count)) => {
                    runtime_log_info(format!(
                        "[远程唤醒压缩] 跳过，任务=低频群入场，conversation_id={}，trigger_message_id={}，current_block_message_count={}，minimum_block_message_count={}",
                        conversation_id,
                        trigger_message_id,
                        block_message_count,
                        REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
                    ));
                    if remote_im_wake_compaction_should_skip_for_low_frequency(block_message_count) {
                        runtime_log_info(format!(
                            "[远程唤醒压缩] 跳过，任务=低频群入场，conversation_id={}，trigger_message_id={}，block_message_count={}，minimum_block_message_count={}",
                            conversation_id,
                            trigger_message_id,
                            block_message_count,
                            REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
                        ));
                        return Ok(RemoteImDynamicWakeCompactionOutcome::SkippedLowFrequency {
                            block_message_count,
                        });
                    }
                }
                Ok(None) => runtime_log_warn(format!(
                    "[远程唤醒压缩] 降级，任务=入场 block 计数，conversation_id={}，trigger_message_id={}，reason=消息存储未就绪，继续压缩路径",
                    conversation_id, trigger_message_id
                )),
                Err(err) => runtime_log_warn(format!(
                    "[远程唤醒压缩] 降级，任务=入场 block 计数，conversation_id={}，trigger_message_id={}，error={}，继续压缩路径",
                    conversation_id, trigger_message_id, err
                )),
            }
        }
        let assistant_name = state_read_agents_cached(state)?
            .into_iter()
            .find(|agent| agent.id.trim() == conversation_meta.agent_id.trim())
            .map(|agent| agent.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "助手".to_string());
        let preserved_dialogue = if include_history {
            // 触发消息作为结束锚点但不纳入保留对话；触发后新消息留在当前 block。
            self.read_block_preserved_dialogue(
                state,
                conversation_id,
                None,
                Some(trigger_message_id),
                "远程联系人",
                &assistant_name,
                ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET,
            )?
        } else {
            String::new()
        };
        let background_shell_lines =
            terminal_background_shell_running_summary_lines(state, conversation_id);
        let summary = build_compaction_message(
            "",
            Some("远程唤醒上下文"),
            if include_history {
                "remote_im_wake_dynamic"
            } else {
                "remote_im_wake_empty_fallback"
            },
            (!preserved_dialogue.trim().is_empty()).then_some(preserved_dialogue.as_str()),
            &background_shell_lines,
        );
        let mut persisted_conversation = self.build_conversation_record_from_meta_view(&conversation_meta);
        persisted_conversation.updated_at = now_iso();
        let cached_metadata = state_read_conversation_metadata_cached(state, conversation_id)?;
        let persist_meta = message_store::ConversationPersistMeta::from_conversation_with_spliced_messages(
            &persisted_conversation,
            &cached_metadata,
            std::slice::from_ref(&trigger),
            &[summary.clone(), trigger.clone()],
        );
        message_store::chat_store_splice_messages(
            &store_paths,
            &persist_meta,
            trigger_index,
            1,
            &[summary.clone(), trigger.clone()],
        )?;
        state_mark_conversation_metadata_direct_persisted(state, conversation_id)?;
        let next_messages = message_store::chat_store_read_messages_after(
            &store_paths,
            &summary.id,
            1,
        )?
        .map(|page| page.messages)
        .unwrap_or_default();
        if next_messages.first().map(|message| message.id.as_str()) != Some(trigger_message_id) {
            return Err(format!(
                "远程唤醒压缩写入校验失败：摘要和触发消息顺序错误，conversation_id={conversation_id}"
            ));
        }
        runtime_log_info(format!(
            "[远程唤醒压缩] 完成，任务=写入摘要，conversation_id={}，summary_message_id={}，trigger_message_id={}，include_history={}",
            conversation_id,
            summary.id,
            trigger_message_id,
            include_history
        ));
        Ok(RemoteImDynamicWakeCompactionOutcome::Applied)
            },
        )
    }

    fn persist_compaction_message(
        &self,
        state: &AppState,
        source: &Conversation,
        compression_message: &ChatMessage,
        refreshed_user_profile_snapshot: Option<String>,
    ) -> Result<CompactionMessagePersistResult, String> {
        let (store_paths, compression_message_id, previous_latest_block_id, active_conversation_id) =
            with_conversation_mutation(state, &source.id, "persist_compaction_message", || {
        let source_meta = self
            .get_conversation_meta(state, &source.id)
            .map_err(|_| "活动对话已变化，请重试上下文整理。".to_string())?;
        if !self.conversation_meta_is_unarchived_meta_view(&source_meta) {
            return Err("活动对话已变化，请重试上下文整理。".to_string());
        }
        let store_paths = message_store::message_store_paths(&state.data_path, &source.id)?;
        ensure_chat_store_conversation_readable(state, &source.id, &store_paths)?;
        let previous_latest_block_id = message_store::chat_store_read_block_page(
            &store_paths,
            None,
        )?
        .map(|page| page.selected_block_id);
        let compression_message_id = compression_message.id.clone();
        let now = now_iso();
        let (conversation_meta, (), _) = state_update_conversation_meta_cached_unlocked(
            state,
            &source.id,
            |cached| {
                let mut metadata_conversation =
                    self.build_conversation_snapshot_from_meta(cached, Vec::new());
                metadata_conversation.user_profile_snapshot =
                    refreshed_user_profile_snapshot.clone().unwrap_or_default();
                metadata_conversation.updated_at = now.clone();
                metadata_conversation.last_user_at = Some(now.clone());
                cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                cached.apply_appended_messages(std::slice::from_ref(compression_message));
                Ok(())
            },
        )?;
        let metadata_conversation =
            self.build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
        state_upsert_chat_index_conversation_cached(state, &metadata_conversation)?;
        let active_conversation_id = Some(metadata_conversation.id.clone());
        let mut ready_meta = message_store::chat_store_read_meta(&store_paths)?
            .ok_or_else(|| {
                format!(
                    "写入上下文整理消息失败：缺少 ready 消息元数据，conversation_id={}",
                    metadata_conversation.id
                )
            })?;
        ready_meta.apply_metadata_fields_from_meta(&conversation_meta);
        ready_meta.apply_appended_messages(std::slice::from_ref(compression_message));
        message_store::chat_store_append_messages_from_meta(
            &store_paths,
            &ready_meta,
            std::slice::from_ref(compression_message),
        )?;
        // v3 保留远程联系人的完整 JSONL 历史；压缩消息只作为新消息追加。
        // 旧的“仅保留最后 block”策略会删掉正文并触发整会话 snapshot 重写。
        Ok((
            store_paths,
            compression_message_id,
            previous_latest_block_id,
            active_conversation_id,
        ))
            })?;

        let persisted = message_store::chat_store_read_message_by_id(
            &store_paths,
            &compression_message_id,
        )?
        .is_some();
        if !persisted {
            return Err(
                "上下文整理消息写入校验失败：已执行整理但未找到落盘消息，请重试。".to_string(),
            );
        }
        let latest_block = message_store::chat_store_read_block_page(&store_paths, None)?
            .ok_or_else(|| {
                format!(
                    "上下文整理消息写入校验失败：缺少最新块，conversation_id={}",
                    source.id
                )
            })?;
        if previous_latest_block_id.is_some()
            && Some(latest_block.selected_block_id) == previous_latest_block_id
        {
            return Err(format!(
                "上下文整理消息写入校验失败：未创建新的摘要块，conversation_id={}",
                source.id
            ));
        }
        let first_message_id = latest_block
            .blocks
            .iter()
            .find(|block| block.block_id == latest_block.selected_block_id)
            .map(|block| block.first_message_id.as_str())
            .unwrap_or_default();
        if first_message_id.trim() != compression_message_id {
            return Err(format!(
                "上下文整理消息写入校验失败：摘要消息不是新块首条消息，conversation_id={}",
                source.id
            ));
        }

        Ok(CompactionMessagePersistResult {
            active_conversation_id,
            compression_message_id,
        })
    }

    fn import_archives(
        &self,
        state: &AppState,
        incoming_archives: &mut Vec<ConversationArchive>,
    ) -> Result<ImportArchivesMutationResult, String> {
        let guard = lock_conversation_with_metrics(state, "import_archives")?;
        let chat_index = state_read_chat_index_cached(state)?;
        let existing_archive_ids = chat_index
            .conversations
            .iter()
            .filter(|item| chat_index_item_is_archived(item))
            .map(|item| item.id.clone())
            .collect::<std::collections::HashSet<_>>();

        let mut imported_count = 0usize;
        let mut replaced_count = 0usize;
        let mut skipped_count = 0usize;
        let mut selected_archive_id: Option<String> = None;
        let mut seen_conversation_ids = std::collections::HashSet::<String>::new();

        for archive in incoming_archives.iter_mut() {
            normalize_archive_for_import(archive, &state.data_path);
        }

        for archive in incoming_archives.drain(..) {
            let archive_id = archive.archive_id.clone();
            let conversation = archive_to_conversation(archive);
            let conversation_id = conversation.id.clone();
            if !seen_conversation_ids.insert(conversation_id.clone()) {
                skipped_count += 1;
                continue;
            }
            self.import_conversation_snapshot(
                state,
                &format!("archive_import_{}", archive_id),
                "archive_import",
                "archive_json_import",
                &conversation,
            )?;
            if existing_archive_ids.contains(&conversation_id) {
                replaced_count += 1;
            } else {
                imported_count += 1;
            }
            if selected_archive_id.is_none() {
                selected_archive_id = Some(archive_id);
            }
        }
        drop(guard);
        let total_count = state_read_chat_index_cached(state)?
            .conversations
            .iter()
            .filter(|item| chat_index_item_is_archived(item))
            .count();

        Ok(ImportArchivesMutationResult {
            imported_count,
            replaced_count,
            skipped_count,
            total_count,
            selected_archive_id,
        })
    }
    fn archive_conversation(
        &self,
        state: &AppState,
        selected_api: &ApiConfig,
        source: &Conversation,
        archive_reason: &str,
    ) -> Result<InstantArchiveConversationMutationResult, String> {
        let (mutation_result, archive_log) =
            with_conversation_mutation(state, &source.id, "archive_conversation", || {
                let source_conversation_meta = self
                    .get_conversation_meta(state, &source.id)
                    .map_err(|err| format!("当前没有可归档的活动对话：{}", err))?;
                let source_conversation =
                    self.build_conversation_record_from_meta_view(&source_conversation_meta);
                let already_archived = source_conversation_meta.status.trim() == "archived";
                if !already_archived
                    && !self.conversation_meta_is_local_normal_chat_meta_view(&source_conversation_meta)
                {
                    return Err("当前没有可归档的活动对话。".to_string());
                }

                let assistant_agent_id = state_service_get_assistant_agent_id(state)?;
                let runtime_snapshot = load_runtime_organization_snapshot(state)?;
                let agents = runtime_snapshot.agents;
                let chat_index = state_read_chat_index_cached(state)?;
                let active_conversation_id = if let Some(conversation_id) = chat_index
                    .conversations
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, item)| {
                        let conversation_meta = self.get_conversation_meta(state, item.id.as_str()).ok()?;
                        Some((idx, conversation_meta))
                    })
                    .filter(|(_, conversation_meta)| {
                        conversation_meta.id != source.id
                            && self.conversation_meta_is_local_normal_chat_meta_view(conversation_meta)
                    })
                    .max_by(|(idx_a, a), (idx_b, b)| {
                        let a_updated = a.updated_at.trim();
                        let b_updated = b.updated_at.trim();
                        let a_created = a.created_at.trim();
                        let b_created = b.created_at.trim();
                        a_updated
                            .cmp(b_updated)
                            .then_with(|| a_created.cmp(b_created))
                            .then_with(|| idx_a.cmp(idx_b))
                    })
                    .map(|(_, conversation_meta)| conversation_meta.id.to_string())
                {
                    conversation_id
                } else {
                    let conversation = build_archive_replacement_conversation(
                        state,
                        &agents,
                        &assistant_agent_id,
                        selected_api,
                        source,
                    )?;
                    let conversation_id = conversation.id.clone();
                    state_schedule_conversation_persist(state, &conversation)?;
                    conversation_id
                };

                let archive_log = if !already_archived {
                    let previous_status = source_conversation.status.clone();
                    let now = now_iso();
                    let (conversation, (), _) = state_update_conversation_metadata_cached_unlocked(
                        state,
                        &source.id,
                        |conversation| {
                            conversation.status = "archived".to_string();
                            conversation.fast_request_turns.clear();
                            conversation.archived_at = Some(now.clone());
                            conversation.updated_at = now.clone();
                            Ok(())
                        },
                    )?;
                    Some((
                        conversation.id,
                        previous_status,
                        conversation.archived_at.unwrap_or_default(),
                    ))
                } else {
                    None
                };
                // 归档不重建全量概览：命令层注册 watermark 删除语义，前端差量同步收敛。
                Ok((
                    InstantArchiveConversationMutationResult {
                        active_conversation_id,
                        already_archived,
                    },
                    archive_log,
                ))
            })?;
        if let Some((conversation_id, previous_status, archived_at)) = archive_log {
            runtime_log_info(format!(
                "[归档] 完成，任务=即时标记归档，conversation_id={}，previous_status={}，reason={}，archived_at={}",
                conversation_id,
                previous_status,
                archive_reason,
                archived_at
            ));
            // 会话归档后按会话清空截图目录（已归档的不重复清理）。
            match clear_operate_screenshots_temp(&state.data_path, &conversation_id) {
                Ok((file_count, dir_count)) => {
                    runtime_log_info(format!(
                        "[operate截图缓存] 完成，任务=clear_temp_on_archive，conversation_id={}，截图文件数={}，子目录数={}",
                        conversation_id, file_count, dir_count
                    ));
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[operate截图缓存] 失败，任务=clear_temp_on_archive，conversation_id={}，error={}",
                        conversation_id, err
                    ));
                }
            }
        }
        Ok(mutation_result)
    }

}
