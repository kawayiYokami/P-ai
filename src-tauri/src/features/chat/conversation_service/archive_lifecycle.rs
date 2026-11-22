const REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT: usize = 14;
/// 压缩收尾清附件时，最近这么多秒内改动过的文件一律不删。
/// 入站附件先落盘、消息后入库，窗口期内文件还没进任何 block，删了就是丢附件。
const REMOTE_IM_ATTACHMENT_PRUNE_MIN_AGE_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
enum RemoteImDynamicWakeCompactionOutcome {
    Applied,
    SkippedLowFrequency { block_message_count: usize },
}

fn remote_im_wake_compaction_should_skip_for_low_frequency(block_message_count: usize) -> bool {
    block_message_count < REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
}

/// 压缩收尾：清掉本会话附件目录里不再被新 block 引用的文件。
///
/// 只碰 `chat/conversations/<id>/downloads/`，旧 `media/` 与 `llm-workspace/downloads/`
/// 一个文件都不动。任何一步失败只记 warn——压缩本身已经成功，清附件只是收尾。
fn remote_im_prune_conversation_attachments_after_compaction(
    state: &AppState,
    conversation_id: &str,
    store_paths: &message_store::MessageStorePaths,
) {
    let dir = match conversation_downloads_dir_from_data_path(&state.data_path, conversation_id) {
        Ok(dir) => dir,
        Err(err) => {
            runtime_log_info(format!(
                "[聊天] 跳过 任务=清理附件 原因=会话附件目录不可用 错误={}",
                err
            ));
            return;
        }
    };
    if !dir.is_dir() {
        return;
    }
    // splice 之后新 block 就是末尾那一块。
    let block_id = match message_store::chat_store_read_block_page(store_paths, None) {
        Ok(Some(page)) => page.selected_block_id,
        Ok(None) => return,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=清理附件 原因=读取末尾 block 失败 错误={}",
                err
            ));
            return;
        }
    };
    // 扫新 block 全部消息引用的附件。比对用文件名，不做字符串包含判断。
    let mut referenced_names = std::collections::HashSet::<String>::new();
    let mut scanned_message_count = 0usize;
    let mut before_message_id: Option<String> = None;
    loop {
        let page = match message_store::chat_store_read_block_messages_before(
            store_paths,
            Some(block_id),
            before_message_id.as_deref(),
            PRESERVED_DIALOGUE_READ_PAGE_SIZE,
        ) {
            Ok(Some(page)) => page,
            Ok(None) => break,
            Err(err) => {
                runtime_log_warn(format!(
                    "[聊天] 跳过 任务=清理附件 原因=读取新 block 消息失败 错误={}",
                    err
                ));
                return;
            }
        };
        let Some(oldest) = page.messages.first() else {
            break;
        };
        before_message_id = Some(oldest.id.clone());
        scanned_message_count = scanned_message_count.saturating_add(page.messages.len());
        for message in &page.messages {
            for part in &message.parts {
                if let MessagePart::Attachment { path, .. } = part {
                    if let Some(name) = std::path::Path::new(path.trim()).file_name() {
                        if let Some(name) = name.to_str() {
                            referenced_names.insert(name.to_string());
                        }
                    }
                }
            }
        }
        if !page.has_more {
            break;
        }
    }
    // 一条消息都没扫到，说明 block 行在、消息却读不出来——这时清单为空是异常，
    // 不能当成「全都没被引用」去删。消息都在但都没附件是正常的，继续删。
    if scanned_message_count == 0 {
        runtime_log_warn(format!(
            "[聊天] 跳过 任务=清理附件 原因=新 block 读不到任何消息，保留全部附件"
        ));
        return;
    }
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=清理附件 原因=读取会话附件目录失败 错误={}",
                err
            ));
            return;
        }
    };
    let mut removed_count = 0usize;
    let mut skipped_recent_count = 0usize;
    let mut skipped_unknown_age_count = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if referenced_names.contains(&file_name) {
            continue;
        }
        // 入站附件先落盘、消息后入库：刚写下的文件可能还没进任何 block，
        // 当成孤儿删掉就是丢附件。最近一分钟内动过的文件一律不删。
        let modified_age_seconds = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
            .map(|age| age.as_secs());
        match modified_age_seconds {
            Some(age) if age >= REMOTE_IM_ATTACHMENT_PRUNE_MIN_AGE_SECONDS => {}
            Some(_) => {
                skipped_recent_count = skipped_recent_count.saturating_add(1);
                continue;
            }
            // 时间戳读不出来时同样不删——删除不可逆，这里一律保守。
            None => {
                skipped_unknown_age_count = skipped_unknown_age_count.saturating_add(1);
                continue;
            }
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed_count = removed_count.saturating_add(1),
            Err(err) => runtime_log_warn(format!(
                "[聊天] 降级 任务=清理附件 文件={} 原因={}",
                file_name, err
            )),
        }
    }
    runtime_log_info(format!(
        "[聊天] 完成 任务=清理附件 引用={} 移除={} 跳过近期={} 跳过时长未知={}",
        referenced_names.len(),
        removed_count,
        skipped_recent_count,
        skipped_unknown_age_count
    ));
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
                        "[聊天] 状态=失败 任务=list_archives 原因={}",
                        err
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
        with_conversation_mutation(state, normalized_archive_id, "unarchive_archive", || {
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
            "[聊天] 完成 任务=取消归档"
        ));
        Ok(())
    }

    fn resolve_archive_request_conversation_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<(ApiConfig, ResolvedApiConfig, Conversation, String), String> {
        self.resolve_request_conversation_by_id(
            state,
            conversation_id,
            "resolve_archive_request_conversation_by_id",
        )
    }

    fn resolve_request_conversation_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        trace_task: &str,
    ) -> Result<(ApiConfig, ResolvedApiConfig, Conversation, String), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required".to_string());
        }
        let unavailable_reason = "当前没有可归档的活动对话。";
        let guard = lock_conversation_with_metrics(state, trace_task)?;
        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        let app_config = &runtime_snapshot.config;
        let source_meta = self
            .get_conversation_meta(state, normalized_conversation_id)
            .map_err(|_| unavailable_reason.to_string())?;
        let kind_allowed = self.conversation_meta_is_local_normal_chat_meta_view(&source_meta);
        if !kind_allowed && source_meta.status.trim() != "archived" {
            drop(guard);
            return Err(unavailable_reason.to_string());
        }
        let effective_agent_id = source_meta.agent_id.trim();
        let effective_agent_id = if effective_agent_id.is_empty() {
            runtime_log_warn(format!(
                "[聊天] 跳过人格校验 任务={} 原因=会话未绑定人格，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
                trace_task
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
                "[聊天] 跳过人格校验 任务={} 原因=会话绑定人格不存在或不可用，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
                trace_task
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
                    "[聊天] apply_patch 备份清理完成 清理={}",
                    cleaned
                ));
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[聊天] apply_patch 备份清理失败 原因={}",
                    err
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
        boundary_message_id: Option<&str>,
        include_history: bool,
    ) -> Result<RemoteImDynamicWakeCompactionOutcome, String> {
        let conversation_id = conversation_id.trim();
        let trigger_message_id = trigger_message_id.trim();
        if conversation_id.is_empty() || trigger_message_id.is_empty() {
            return Err("远程唤醒压缩失败：缺少会话或触发消息 ID".to_string());
        }
        let outcome = with_conversation_mutation(
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
        // 压缩边界严格止步于已处理游标：游标之后是前台还没看过的新鲜消息，一条都不许压进摘要。
        // 游标为空（首次入场）或游标消息已被更早的压缩吃掉时，退回触发消息，不让整理因此失败。
        let boundary_message_id = match boundary_message_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(cursor)
                if message_store::chat_store_read_message_by_id(&store_paths, cursor)?.is_some() =>
            {
                cursor
            }
            Some(cursor) => {
                runtime_log_info(format!(
                    "[聊天] 压缩降级 任务=压缩边界 原因=游标消息已不存在 处理=退回触发消息 游标={cursor}"
                ));
                trigger_message_id
            }
            None => trigger_message_id,
        };
        let boundary = message_store::chat_store_read_message_by_id(&store_paths, boundary_message_id)?
            .ok_or_else(|| {
                format!("远程唤醒压缩失败：压缩边界消息不存在，message_id={boundary_message_id}")
            })?;
        let boundary_index =
            message_store::chat_store_read_message_sequence(&store_paths, boundary_message_id)?
                .ok_or_else(|| {
                    format!(
                        "远程唤醒压缩失败：压缩边界消息缺少序号，message_id={boundary_message_id}"
                    )
                })?;
        if include_history {
            match message_store::chat_store_read_block_message_count(
                &store_paths,
                trigger_message_id,
            ) {
                Ok(Some(block_message_count)) => {
                    runtime_log_info(format!(
                        "[聊天] 跳过 任务=低频群入场 当前块消息数={} 最小块消息数={}",
                        block_message_count,
                        REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
                    ));
                    if remote_im_wake_compaction_should_skip_for_low_frequency(block_message_count) {
                        runtime_log_info(format!(
                            "[聊天] 跳过 任务=低频群入场 块消息数={} 最小块消息数={}",
                            block_message_count,
                            REMOTE_IM_WAKE_COMPACTION_MIN_BLOCK_MESSAGE_COUNT
                        ));
                        return Ok(RemoteImDynamicWakeCompactionOutcome::SkippedLowFrequency {
                            block_message_count,
                        });
                    }
                }
                Ok(None) => runtime_log_warn(format!(
                    "[聊天] 降级 任务=入场 block 计数 原因=消息存储未就绪，继续压缩路径"
                )),
                Err(err) => runtime_log_warn(format!(
                    "[聊天] 降级 任务=入场 block 计数 原因={}，继续压缩路径",
                    err
                )),
            }
        }
        // 第二段：游标之前最后 26 KiB 的原文。压缩后它跟着第三段一起进新 block，
        // 旧 block 只剩第一段。计费口径是整条消息 render_prompt_message_text 的字节数，
        // 不做单条截断、不加说话人前缀；这一条与原 PreservedDialogueAccumulator
        // 的行计费口径不同，两者不要混用。
        // 读到更早的摘要就停：一个 block 只许有一个摘要头，旧摘要不能被搬进新 block。
        let mut keep_messages: Vec<ChatMessage> = Vec::new();
        let mut keep_dialogue_bytes = 0usize;
        if include_history {
            let budget_bytes = ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET.limit();
            let mut requested_block_id: Option<u32> = None;
            let mut before_message_id: Option<String> = Some(boundary_message_id.to_string());
            'read_keep: loop {
                let page = message_store::chat_store_read_block_messages_before(
                    &store_paths,
                    requested_block_id,
                    before_message_id.as_deref(),
                    PRESERVED_DIALOGUE_READ_PAGE_SIZE,
                )?
                .ok_or_else(|| {
                    format!(
                        "远程唤醒压缩失败：消息存储未就绪，conversation_id={conversation_id}"
                    )
                })?;
                // 第一页之后锁死在同一个 block：第二段不许跨到上一个 block，
                // 否则 splice 会重写两个块，「旧 block 只剩第一段」就不成立了。
                requested_block_id = Some(page.selected_block_id);
                let Some(oldest) = page.messages.first() else {
                    break;
                };
                before_message_id = Some(oldest.id.clone());
                // 页内是时间正序，从新往旧累加。
                for message in page.messages.iter().rev() {
                    if is_context_compaction_message(message, message.role.trim()) {
                        break 'read_keep;
                    }
                    let message_bytes = render_prompt_message_text(message).len();
                    if keep_dialogue_bytes.saturating_add(message_bytes) > budget_bytes {
                        break 'read_keep;
                    }
                    keep_dialogue_bytes = keep_dialogue_bytes.saturating_add(message_bytes);
                    keep_messages.push(message.clone());
                }
                if !page.has_more {
                    break;
                }
            }
            keep_messages.reverse();
        }
        let keep_start_sequence = match keep_messages.first() {
            Some(first) => message_store::chat_store_read_message_sequence(&store_paths, &first.id)?
                .ok_or_else(|| {
                    format!(
                        "远程唤醒压缩失败：第二段首条缺少序号，message_id={}",
                        first.id
                    )
                })?,
            None => boundary_index,
        };
        // 摘要正文只放元信息：压缩后摘要就是新 block 的开头，读者要知道这是哪个联系人/群。
        let contact_summary = state_service_list_remote_im_contacts(state, None)
            .unwrap_or_default()
            .into_iter()
            .find(|contact| {
                contact.bound_conversation_id.as_deref().map(str::trim) == Some(conversation_id)
            })
            .map(|contact| {
                format!(
                    "当前联系人：{}（{}，{}）",
                    contact.remote_contact_name.trim(),
                    contact.remote_contact_type.trim(),
                    contact.remote_contact_id.trim()
                )
            })
            .unwrap_or_default();
        let background_shell_lines =
            terminal_background_shell_running_summary_lines(state, conversation_id);
        let teammate_names = available_delegate_teammate_names(
            &state_read_agents_cached(state)?,
            &conversation_meta.agent_id,
        );
        let summary = build_compaction_message(
            &contact_summary,
            Some("远程唤醒上下文"),
            if include_history {
                "remote_im_wake_dynamic"
            } else {
                "remote_im_wake_empty_fallback"
            },
            None,
            &background_shell_lines,
            &teammate_names,
        );
        let mut persisted_conversation = self.build_conversation_record_from_meta_view(&conversation_meta);
        persisted_conversation.updated_at = now_iso();
        let cached_metadata = state_read_conversation_metadata_cached(state, conversation_id)?;
        // 三段式：第二段原位换成「摘要 + 第二段」，第三段（边界及之后）原封不动，
        // 旧 block 只剩第一段。第二段为空时退回原来的「边界原位换成摘要 + 边界」。
        let (splice_start, splice_delete, removed_messages, inserted_messages) =
            if keep_messages.is_empty() {
                (
                    boundary_index,
                    1usize,
                    vec![boundary.clone()],
                    vec![summary.clone(), boundary.clone()],
                )
            } else {
                let delete_count = boundary_index.saturating_sub(keep_start_sequence);
                if delete_count != keep_messages.len() {
                    return Err(format!(
                        "远程唤醒压缩失败：第二段与边界不连续，conversation_id={}，keep_start_sequence={}，keep_count={}，boundary_index={}",
                        conversation_id,
                        keep_start_sequence,
                        keep_messages.len(),
                        boundary_index
                    ));
                }
                let mut inserted = Vec::with_capacity(keep_messages.len() + 1);
                inserted.push(summary.clone());
                // 第二段进新 block 前瘦身：工具、内联 base64、extraTextBlocks 去掉，
                // providerMeta 与思维链原样带。removed 仍传原始消息（它们才是被替换掉的那批）。
                inserted.extend(
                    keep_messages
                        .iter()
                        .map(message_store::slim_older_conversation_block_message),
                );
                (
                    keep_start_sequence,
                    delete_count,
                    keep_messages.clone(),
                    inserted,
                )
            };
        let persist_meta = message_store::ConversationPersistMeta::from_conversation_with_spliced_messages(
            &persisted_conversation,
            &cached_metadata,
            &removed_messages,
            &inserted_messages,
        );
        message_store::chat_store_splice_messages(
            &store_paths,
            &persist_meta,
            splice_start,
            splice_delete,
            &inserted_messages,
        )?;
        state_mark_conversation_metadata_direct_persisted(state, conversation_id)?;
        let next_messages = message_store::chat_store_read_messages_after(
            &store_paths,
            &summary.id,
            1,
        )?
        .map(|page| page.messages)
        .unwrap_or_default();
        // 摘要后面第一条：第二段非空时是它自己的首条，否则是边界消息。
        let expected_first_after_summary = keep_messages
            .first()
            .map(|message| message.id.as_str())
            .unwrap_or(boundary_message_id);
        if next_messages.first().map(|message| message.id.as_str())
            != Some(expected_first_after_summary)
        {
            return Err(format!(
                "远程唤醒压缩写入校验失败：摘要后的第一条消息顺序错误，conversation_id={conversation_id}，expected={expected_first_after_summary}"
            ));
        }
        runtime_log_info(format!(
            "[聊天] 完成 任务=写入摘要 含历史={} 保留消息数={} 保留字节={}",
            include_history,
            keep_messages.len(),
            keep_dialogue_bytes
        ));
        Ok(RemoteImDynamicWakeCompactionOutcome::Applied)
            },
        )?;
        // 附件清理放在会话锁外：分页读新 block + 目录遍历 + 逐文件删除都很慢，
        // 不该挡着同会话的其他写操作。压缩本身已经成功，这里失败只记 warn。
        if matches!(outcome, RemoteImDynamicWakeCompactionOutcome::Applied) {
            match message_store::message_store_paths(&state.data_path, conversation_id) {
                Ok(store_paths) => {
                    remote_im_prune_conversation_attachments_after_compaction(
                        state,
                        conversation_id,
                        &store_paths,
                    );
                }
                Err(err) => runtime_log_warn(format!(
                    "[聊天] 跳过 任务=清理附件 原因=消息存储路径不可用 错误={}",
                    err
                )),
            }
        }
        Ok(outcome)
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
                // 压缩后清理 MCP 临时名单：压界丢掉拿取历史，留着就是无来源常驻。
                metadata_conversation.loaded_mcp_tools = Vec::new();
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
                "[聊天] 完成 任务=即时标记归档 原状态={} 原因={} 归档时间={}",
                previous_status,
                archive_reason,
                archived_at
            ));
            // 会话归档后按会话清空截图目录（已归档的不重复清理）。
            match clear_operate_screenshots_temp(&state.data_path, &conversation_id) {
                Ok((file_count, dir_count)) => {
                    runtime_log_info(format!(
                        "[工具] 完成 任务=clear_temp_on_archive 截图文件数={} 子目录数={}",
                        file_count, dir_count
                    ));
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[工具] 失败 任务=clear_temp_on_archive 原因={}",
                        err
                    ));
                }
            }
        }
        Ok(mutation_result)
    }

}
