#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MessageStoreDirectorySnapshotWrite {
    manifest: MessageStoreManifest,
    message_count: usize,
    last_message_id: String,
}

pub(super) fn chat_store_write_snapshot(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    chat_metadata_store_write_snapshot(paths, &normalized_conversation)
}

pub(super) fn write_jsonl_snapshot_directory_shard(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    if let Some(existing_manifest) = read_message_store_manifest(&paths.manifest_file)?
        .filter(MessageStoreManifest::should_read_jsonl)
    {
        if let Some(reason) =
            jsonl_snapshot_directory_incremental_fallback_reason(paths, &existing_manifest)
        {
            runtime_log_error(format!(
                "[消息存储] ready 快照基线不可用于增量写入，回退全量重写：conversation_id={}，reason={}",
                paths.conversation_id, reason
            ));
        } else {
            return write_jsonl_snapshot_directory_shard_incremental(
                paths,
                &normalized_conversation,
            );
        }
    }
    write_jsonl_snapshot_directory_shard_full(paths, &normalized_conversation)
}

fn directory_store_write_if_changed(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<bool, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    match delegate_directory_store_read_conversation(paths) {
        Ok(Some(existing)) => {
            if serde_json::to_value(&existing)
                .map_err(|err| format!("序列化现有会话失败，conversation_id={}，error={err}", paths.conversation_id))?
                == serde_json::to_value(&normalized_conversation).map_err(|err| {
                    format!(
                        "序列化待写入会话失败，conversation_id={}，error={err}",
                        paths.conversation_id
                    )
                })?
            {
                return Ok(false);
            }
        }
        Ok(None) => {}
        Err(err) => {
            runtime_log_error(format!(
                "[消息存储] ready 快照读取失败，改为强制重写：conversation_id={}，error={}",
                paths.conversation_id, err
            ));
        }
    }
    write_jsonl_snapshot_directory_shard(paths, &normalized_conversation)?;
    Ok(true)
}

pub(super) fn delegate_directory_store_write_if_changed(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<bool, String> {
    directory_store_write_if_changed(paths, conversation)
}

fn normalize_conversation_media_refs_for_message_store(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Conversation {
    let mut next = conversation.clone();
    for message in &mut next.messages {
        canonicalize_message_parts_for_persistence(&mut message.parts, &paths.data_path);
        message.provider_meta = provider_meta_without_legacy_attachments(message.provider_meta.take());
    }
    next
}

fn write_jsonl_snapshot_directory_shard_full(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let expected_last_message_id = conversation
        .messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(conversation)?;
    if blocks.message_count != conversation.messages.len()
        || blocks.last_message_id != expected_last_message_id
    {
        return Err(format!(
            "写入会话块失败：构建结果不一致，conversation_id={}，expected_count={}，actual_count={}，expected_last={}，actual_last={}",
            paths.conversation_id,
            conversation.messages.len(),
            blocks.message_count,
            expected_last_message_id,
            blocks.last_message_id
        ));
    }
    let manifest = MessageStoreManifest::jsonl_snapshot_building(conversation)
        .jsonl_snapshot_ready(blocks.total_bytes, 1);
    let meta = ConversationShardMeta::from_conversation(conversation);

    write_conversation_shard_meta_atomic(&paths.meta_file, &meta)?;
    write_jsonl_snapshot_conversation_blocks(paths, &blocks)?;
    write_message_store_index_atomic(&paths.index_file, &blocks.index)?;
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)?;

    Ok(MessageStoreDirectorySnapshotWrite {
        manifest,
        message_count: blocks.message_count,
        last_message_id: blocks.last_message_id,
    })
}

fn jsonl_snapshot_directory_incremental_fallback_reason(
    paths: &MessageStorePaths,
    manifest: &MessageStoreManifest,
) -> Option<String> {
    if let Err(err) = validate_ready_message_store_snapshot_integrity(paths, manifest) {
        return Some(format!("ready 快照完整性校验失败：{err}"));
    }
    let old_index = match read_message_store_index_file(&paths.index_file) {
        Ok(index) => index,
        Err(err) => return Some(format!("读取旧索引失败：{err}")),
    };
    let old_block_ids = ordered_message_store_index_block_ids(&old_index);
    if old_block_ids.is_empty() {
        return Some("旧索引没有可复用 block 基线".to_string());
    }
    for block_id in old_block_ids {
        // delegate 目录快照保持明文 .jsonl（不随生产切 .jsonl.zstd）
        let block_path = paths
            .shard_dir
            .join(MESSAGE_STORE_BLOCKS_DIR_NAME)
            .join(format!("{block_id:06}.jsonl"));
        if !block_path.exists() {
            return Some(format!("旧 block 文件缺失：{}", block_path.display()));
        }
    }
    None
}

fn write_jsonl_snapshot_directory_shard_incremental(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let old_index = (*read_message_store_index_file(&paths.index_file)?).clone();
    let old_block_ids = ordered_message_store_index_block_ids(&old_index);
    if old_block_ids.is_empty() {
        return Err(format!(
            "增量写入会话块失败：ready 会话缺少旧 block 索引，conversation_id={}",
            paths.conversation_id
        ));
    }
    let source_blocks = split_conversation_messages_into_blocks(conversation);
    if source_blocks.is_empty() {
        return Err(format!(
            "增量写入会话块失败：ready 会话没有消息，conversation_id={}",
            paths.conversation_id
        ));
    }
    let old_block_count = old_block_ids.len();
    let new_block_count = source_blocks.len();
    if new_block_count < old_block_count {
        runtime_log_info(format!(
            "[消息存储] 增量写入命中 block 数量减少，回退全量重写：conversation_id={}，old_count={}，new_count={}",
            paths.conversation_id,
            old_block_count, new_block_count
        ));
        return write_jsonl_snapshot_directory_shard_full(paths, conversation);
    }
    for (idx, block) in source_blocks.iter().enumerate().take(old_block_count) {
        if old_block_ids
            .get(idx)
            .is_some_and(|old_block_id| Some(*old_block_id) == Some(block.block_id))
        {
            continue;
        }
        runtime_log_info(format!(
            "[消息存储] 增量写入命中 block 顺序不一致，回退全量重写：conversation_id={}，index={}，old_block={:?}，new_block={}",
            paths.conversation_id,
            idx,
            old_block_ids.get(idx),
            block.block_file
        ));
        return write_jsonl_snapshot_directory_shard_full(paths, conversation);
    }

    let mut rewrite_block_indices = std::collections::BTreeSet::<usize>::new();
    rewrite_block_indices.insert(new_block_count - 1);
    for idx in old_block_count..new_block_count {
        rewrite_block_indices.insert(idx);
    }
    if new_block_count > old_block_count {
        for idx in old_block_count.saturating_sub(2)..new_block_count.saturating_sub(2) {
            rewrite_block_indices.insert(idx);
        }
    }

    fs::create_dir_all(&paths.blocks_dir).map_err(|err| {
        format!(
            "创建会话块目录失败，conversation_id={}，path={}，error={err}",
            paths.conversation_id,
            paths.blocks_dir.display()
        )
    })?;
    let building_manifest = MessageStoreManifest::jsonl_snapshot_building(conversation);
    write_message_store_manifest_atomic(&paths.manifest_file, &building_manifest)?;

    let mut next_items = Vec::<MessageStoreIndexItem>::with_capacity(conversation.messages.len());
    for (idx, block_refs) in source_blocks.iter().enumerate() {
        if rewrite_block_indices.contains(&idx) {
            let should_slim =
                should_slim_conversation_block(conversation.status.trim() == "archived", idx, new_block_count);
            let block = build_jsonl_snapshot_conversation_block(block_refs, should_slim)?;
            let block_path = paths.shard_dir.join(&block.block_file);
            write_jsonl_snapshot_atomic(&block_path, &block.content)?;
            next_items.extend(block.index_items);
            continue;
        }
            next_items.extend(
                old_index
                    .items
                    .iter()
                    .filter(|item| item.block_id == Some(block_refs.block_id))
                    .cloned(),
            );
    }

    if next_items.len() != conversation.messages.len() {
        return Err(format!(
            "增量写入会话块失败：索引消息数量不一致，conversation_id={}，expected={}，actual={}",
            paths.conversation_id,
            conversation.messages.len(),
            next_items.len()
        ));
    }

    let next_index = MessageStoreIndexFile::new(MESSAGE_STORE_MANIFEST_VERSION, next_items);
    cleanup_stale_conversation_block_files(paths, &source_blocks)?;
    if paths.messages_file.exists() {
        fs::remove_file(&paths.messages_file).map_err(|err| {
            format!(
                "清理旧单文件 JSONL 失败，conversation_id={}，path={}，error={err}",
                paths.conversation_id,
                paths.messages_file.display()
            )
        })?;
    }

    let total_bytes = message_store_index_total_bytes(paths, &next_index)?;
    let last_message_id = next_index
        .items
        .last()
        .map(|item| item.message_id.clone())
        .unwrap_or_default();
    let manifest = MessageStoreManifest::jsonl_snapshot_building(conversation)
        .jsonl_snapshot_ready(total_bytes, 1);
    let meta = ConversationShardMeta::from_conversation(conversation);

    write_conversation_shard_meta_atomic(&paths.meta_file, &meta)?;
    write_message_store_index_atomic(&paths.index_file, &next_index)?;
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)?;

    Ok(MessageStoreDirectorySnapshotWrite {
        manifest,
        message_count: next_index.items.len(),
        last_message_id,
    })
}

fn ordered_message_store_index_block_ids(index: &MessageStoreIndexFile) -> Vec<u32> {
    let mut out = Vec::<u32>::new();
    for item in &index.items {
        let Some(block_id) = item.block_id else {
            continue;
        };
        if out.last().is_some_and(|last| *last == block_id) {
            continue;
        }
        out.push(block_id);
    }
    out
}

fn cleanup_stale_conversation_block_files(
    paths: &MessageStorePaths,
    source_blocks: &[ConversationBlockMessageRefs<'_>],
) -> Result<(), String> {
    let expected_block_files = source_blocks
        .iter()
        .map(|block| block.block_file.clone())
        .collect::<std::collections::HashSet<_>>();
    cleanup_stale_conversation_block_files_by_names(paths, &expected_block_files, None)
}

fn cleanup_stale_conversation_block_files_by_names(
    paths: &MessageStorePaths,
    expected_block_files: &std::collections::HashSet<String>,
    managed_block_files: Option<&std::collections::HashSet<String>>,
) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(&paths.blocks_dir) {
        for entry in entries.flatten() {
            let block_path = entry.path();
            if !block_path.is_file() {
                continue;
            }
            let block_name = entry.file_name().to_string_lossy().to_string();
            let block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_name}");
            if expected_block_files.contains(&block_file) {
                continue;
            }
            if let Some(managed_block_files) = managed_block_files {
                // V3 与旧 V2 可能共用 blocks 目录。只有明确属于当前 V3
                // operation 的文件才允许被退休；未被 SQLite current
                // 记录的旧文件保持原样，避免普通新建/更新误删旧数据。
                if !managed_block_files.contains(&block_file) {
                    continue;
                }
            }
            fs::remove_file(&block_path).map_err(|err| {
                format!(
                    "清理过期会话块失败，conversation_id={}，path={}，error={err}",
                    paths.conversation_id,
                    block_path.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(super) fn chat_store_write_meta(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
) -> Result<(), String> {
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    chat_metadata_store_write_meta_only(paths, &shard_meta)
}

#[cfg(test)]
fn write_jsonl_snapshot_messages_shard(
    paths: &MessageStorePaths,
    snapshot: &ConversationPersistMessagesSnapshot,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let expected_last_message_id = snapshot
        .messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    let blocks = build_jsonl_snapshot_conversation_blocks(&snapshot.messages)?;
    if blocks.message_count != snapshot.messages.len()
        || blocks.last_message_id != expected_last_message_id
    {
        return Err(format!(
            "写入会话块失败：消息快照构建结果不一致，conversation_id={}，expected_count={}，actual_count={}，expected_last={}，actual_last={}",
            paths.conversation_id,
            snapshot.messages.len(),
            blocks.message_count,
            expected_last_message_id,
            blocks.last_message_id
        ));
    }
    let manifest = MessageStoreManifest::jsonl_snapshot_ready_for_messages(
        blocks.message_count,
        blocks.last_message_id.clone(),
        blocks.total_bytes,
        1,
    );

    write_jsonl_snapshot_conversation_blocks(paths, &blocks)?;
    write_message_store_index_atomic(&paths.index_file, &blocks.index)?;
    write_message_store_manifest_atomic(&paths.manifest_file, &manifest)?;

    Ok(MessageStoreDirectorySnapshotWrite {
        manifest,
        message_count: blocks.message_count,
        last_message_id: blocks.last_message_id,
    })
}

fn write_jsonl_snapshot_conversation_blocks(
    paths: &MessageStorePaths,
    blocks: &JsonlSnapshotConversationBlocks,
) -> Result<(), String> {
    fs::create_dir_all(&paths.blocks_dir).map_err(|err| {
        format!(
            "创建会话块目录失败，conversation_id={}，path={}，error={err}",
            paths.conversation_id,
            paths.blocks_dir.display()
        )
    })?;
    for block in &blocks.blocks {
        let block_path = paths.shard_dir.join(&block.block_file);
        write_jsonl_snapshot_atomic(&block_path, &block.content)?;
    }
    let expected_block_files = blocks
        .blocks
        .iter()
        .map(|block| block.block_file.clone())
        .collect::<std::collections::HashSet<_>>();
    cleanup_stale_conversation_block_files_by_names(paths, &expected_block_files, None)?;
    if paths.messages_file.exists() {
        fs::remove_file(&paths.messages_file).map_err(|err| {
            format!(
                "清理旧单文件 JSONL 失败，conversation_id={}，path={}，error={err}",
                paths.conversation_id,
                paths.messages_file.display()
            )
        })?;
    }
    Ok(())
}

pub(super) fn chat_store_append_message(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    message: &ChatMessage,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    chat_store_append_message_entries(paths, &[(meta, message)], Some(meta))
}

pub(super) fn chat_store_append_messages_from_meta(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
    messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let persist_meta = meta.to_persist_meta();
    let entries = messages
        .iter()
        .map(|message| (&persist_meta, message))
        .collect::<Vec<_>>();
    chat_store_append_message_entries(paths, &entries, Some(&persist_meta))
}

fn chat_store_append_message_entries(
    paths: &MessageStorePaths,
    entries: &[(&ConversationPersistMeta, &ChatMessage)],
    final_meta: Option<&ConversationPersistMeta>,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if entries.is_empty() {
        return Err(format!(
            "追加 JSONL 消息失败：追加列表为空，conversation_id={}",
            paths.conversation_id
        ));
    }
    for (meta, _) in entries {
        if meta.conversation_id() != paths.conversation_id {
            return Err(format!(
                "追加 JSONL 消息失败：meta 会话 ID 不一致，expected={}，actual={}",
                paths.conversation_id,
                meta.conversation_id()
            ));
        }
    }
    if let Some(meta) = final_meta {
        if meta.conversation_id() != paths.conversation_id {
            return Err(format!(
                "追加 JSONL 消息失败：final meta 会话 ID 不一致，expected={}，actual={}",
                paths.conversation_id,
                meta.conversation_id()
            ));
        }
    }
    let final_meta = final_meta
        .or_else(|| entries.last().map(|(meta, _)| *meta))
        .ok_or_else(|| "追加 JSONL 消息失败：缺少最终 meta".to_string())?;
    chat_metadata_store_append_messages(
        paths,
        final_meta,
        &entries
            .iter()
            .map(|(_, message)| (*message).clone())
            .collect::<Vec<_>>(),
    )
}

struct AppendedMessageBlockPlan {
    continue_last_block: bool,
    groups: Vec<Vec<ChatMessage>>,
}

fn appended_message_starts_new_block(
    next_message: &ChatMessage,
) -> bool {
    message_store_compaction_kind(next_message).is_some()
}

fn plan_appended_message_blocks(
    last_existing_message: Option<&ChatMessage>,
    entries: &[(&ConversationPersistMeta, &ChatMessage)],
) -> AppendedMessageBlockPlan {
    let mut groups = Vec::<Vec<ChatMessage>>::new();
    let mut current = Vec::<ChatMessage>::new();
    let mut previous = last_existing_message.cloned();
    let mut continue_last_block = false;
    for (idx, (_, message)) in entries.iter().enumerate() {
        let start_new_block = previous
            .as_ref()
            .map(|_| appended_message_starts_new_block(message))
            .unwrap_or(false);
        if idx == 0 {
            continue_last_block = previous.is_some() && !start_new_block;
        }
        if start_new_block && !current.is_empty() {
            groups.push(current);
            current = Vec::new();
        }
        current.push((*message).clone());
        previous = Some((*message).clone());
    }
    if !current.is_empty() {
        groups.push(current);
    }
    AppendedMessageBlockPlan {
        continue_last_block,
        groups,
    }
}

pub(super) fn chat_store_truncate_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    keep_count: usize,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    chat_store_truncate_messages_with_persist_meta(paths, meta, keep_count)
}

pub(super) fn chat_store_truncate_messages_from_meta(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
    keep_count: usize,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let persist_meta = meta.to_persist_meta();
    chat_store_truncate_messages_with_persist_meta(paths, &persist_meta, keep_count)
}

fn chat_store_truncate_messages_with_persist_meta(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    keep_count: usize,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if meta.conversation_id() != paths.conversation_id {
        return Err(format!(
            "截断 JSONL 消息失败：meta 会话 ID 不一致，expected={}，actual={}",
            paths.conversation_id,
            meta.conversation_id()
        ));
    }
    chat_metadata_store_truncate_messages(paths, meta, keep_count)
}

pub(super) fn chat_store_replace_message(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    message: &ChatMessage,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if meta.conversation_id() != paths.conversation_id {
        return Err(format!(
            "替换 JSONL 消息失败：meta 会话 ID 不一致，expected={}，actual={}",
            paths.conversation_id,
            meta.conversation_id()
        ));
    }
    chat_metadata_store_replace_message(paths, meta, message)
}

/// 组内追加子行（D14：工具事件/正文累积不再走 replace）：
/// 对比 previous_message 与 updated_message 的差异，追加增量子行到目标组。
///
/// 适用场景与新增行判定：
/// - 工具事件（D14 工具累积）：updated 比 previous 多出若干工具事件（元素，不配对）
///   → 每个新元素追加一行工具行；若 provider_meta 同时变化，正文行尚未写、
///   provider_meta 无处承载 → 回退 replace（D14 场景①）。运行时已保证工具追加不带
///   meta 变化（用量统一在 final text 落盘写入），本守卫仅防御其它来源的 meta 变化。
/// - 正文累积（final text / meme / provider_meta 回填）：工具行数不变、正文内容变化
///   → 追加正文行；若工具行也变了（罕见），回退 replace。
///
/// 物理约束：目标组必须是块内最后一个 locator（帧追加只能到文件尾）。不满足时回退
/// 整块重建（replace），由调用方决定是否替换语义。
pub(super) fn chat_store_append_line_to_group(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    previous_message: &ChatMessage,
    updated_message: &ChatMessage,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if meta.conversation_id() != paths.conversation_id {
        return Err(format!(
            "组内追加子行失败：meta 会话 ID 不一致，expected={}，actual={}",
            paths.conversation_id,
            meta.conversation_id()
        ));
    }
    if previous_message.id.trim() != updated_message.id.trim() {
        return Err(format!(
            "组内追加子行失败：前后消息 ID 不一致，previous={}，updated={}",
            previous_message.id.trim(),
            updated_message.id.trim()
        ));
    }
    let locator = chat_metadata_store_read_locator_by_id(paths, updated_message.id.trim())?
        .ok_or_else(|| {
            format!(
                "组内追加子行失败：消息不存在，conversation_id={}，message_id={}",
                paths.conversation_id,
                updated_message.id.trim()
            )
        })?;
    let block_id = locator.item.block_id.ok_or_else(|| {
        format!(
            "组内追加子行失败：locator 缺少 block id，message_id={}",
            updated_message.id.trim()
        )
    })?;
    // 目标组必须是块内最后一个 locator（帧追加只能到文件尾）
    let block_rows = chat_metadata_store_read_locators_for_block(paths, block_id)?;
    let is_block_tail = block_rows
        .last()
        .map(|row| row.sequence == locator.sequence)
        .unwrap_or(false);
    if !is_block_tail {
        // 目标组不在块尾：物理帧追加无法表达，回退整块重建（replace 语义由调用方决定）
        return chat_store_replace_message(paths, meta, updated_message);
    }

    // 判定增量：工具事件（工具行增加） vs 正文累积（正文行变化）
    let previous_tools = previous_message.tool_call.as_deref().unwrap_or_default();
    let updated_tools = updated_message.tool_call.as_deref().unwrap_or_default();
    let tool_diff = updated_tools.len() as i64 - previous_tools.len() as i64;
    if tool_diff < 0 {
        // 工具减少：形态变化，回退整块重建
        return chat_store_replace_message(paths, meta, updated_message);
    }
    // 正文字段（含 provider_meta patch）任何变化都意味着正文行需要重写；
    // 工具增量分支只允许「纯工具行追加」，正文变化一律回退 replace。
    let body_content_changed = previous_message.parts != updated_message.parts
        || previous_message.extra_text_blocks != updated_message.extra_text_blocks
        || previous_message.provider_meta != updated_message.provider_meta
        || previous_message.meme_annotations != updated_message.meme_annotations
        || previous_message.mcp_call != updated_message.mcp_call;
    if tool_diff > 0 {
        // 工具事件增量：只允许纯工具行追加；任何正文字段变化都回退 replace
        if body_content_changed {
            return chat_store_replace_message(paths, meta, updated_message);
        }
        // previous 无工具行：目标组是单行 message 组（或纯正文组），物理追加工具行会
        // 落到 message 行后面，assemble 遇 kind=message 行短路无视后续行 → 回退整块重写
        // 为工具行形态（split 对正文全空的开放组不写正文行，重写后组是纯工具行）。
        if previous_tools.is_empty() {
            return chat_store_replace_message(paths, meta, updated_message);
        }
        let mut new_lines = Vec::<String>::with_capacity(tool_diff as usize);
        for event in &updated_tools[previous_tools.len()..] {
            new_lines.push(encode_group_tool_line(
                &updated_message.id,
                &updated_message.role,
                &updated_message.created_at,
                &updated_message.speaker_agent_id,
                event,
            )?);
        }
        return chat_metadata_store_append_line_to_group_physical(
            paths,
            &ConversationShardMeta::from_persist_meta(meta),
            block_id,
            &locator,
            &new_lines,
        );
    }

    // 工具行数不变：正文累积（final text / meme / provider_meta 回填）
    if !body_content_changed {
        return Err(format!(
            "组内追加子行失败：前后消息无差异，message_id={}",
            updated_message.id.trim()
        ));
    }
    // 目标组必须已有正文行语义：普通消息（无工具）是单行组，追加正文 = 整行替换
    if previous_tools.is_empty() {
        return chat_store_replace_message(paths, meta, updated_message);
    }
    let new_lines = vec![encode_group_assistant_line(
        &updated_message.id,
        &updated_message.role,
        &updated_message.created_at,
        &updated_message.speaker_agent_id,
        &updated_message.parts,
        &updated_message.extra_text_blocks,
        &updated_message.provider_meta,
        &updated_message.meme_annotations,
        &updated_message.mcp_call,
    )?];
    chat_metadata_store_append_line_to_group_physical(
        paths,
        &ConversationShardMeta::from_persist_meta(meta),
        block_id,
        &locator,
        &new_lines,
    )
}

pub(super) fn chat_store_replace_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if meta.conversation_id() != paths.conversation_id {
        return Err(format!(
            "批量替换 JSONL 消息失败：meta 会话 ID 不一致，expected={}，actual={}",
            paths.conversation_id,
            meta.conversation_id()
        ));
    }
    chat_metadata_store_replace_messages(paths, meta, messages)
}

pub(super) fn chat_store_splice_messages(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    start_index: usize,
    delete_count: usize,
    inserted_messages: &[ChatMessage],
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    if meta.conversation_id() != paths.conversation_id {
        return Err(format!(
            "拼接 JSONL 消息失败：meta 会话 ID 不一致，expected={}，actual={}",
            paths.conversation_id,
            meta.conversation_id()
        ));
    }
    chat_metadata_store_splice_messages(paths, meta, start_index, delete_count, inserted_messages)
}

#[cfg(test)]
pub(super) fn should_write_jsonl_snapshot_directory_shard(
    paths: &MessageStorePaths,
) -> Result<bool, String> {
    Ok(read_message_store_manifest(&paths.manifest_file)?
        .map(|manifest| manifest.should_write_jsonl_snapshot())
        .unwrap_or(false))
}

#[cfg(test)]
mod message_store_persist_tests {
    use super::*;

    fn test_message(id: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "user".to_string(),
            created_at: "2026-04-24T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("message {id}"),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-persist".to_string(),
            title: "persist".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-24T00:00:00Z".to_string(),
            updated_at: "2026-04-24T00:00:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    #[test]
    fn message_store_persist_should_write_directory_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-persist-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);

        let write = write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot");

        assert_eq!(write.message_count, 2);
        assert_eq!(write.last_message_id, "m2");
        assert!(write.manifest.should_read_jsonl());
        assert_eq!(loaded.messages.len(), 2);
        assert!(paths.meta_file.exists());
        assert!(!paths.messages_file.exists());
        assert!(paths.blocks_dir.exists());
        assert!(paths.index_file.exists());
        assert!(paths.manifest_file.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_update_meta_without_touching_messages() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-meta-only-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");
        let loaded_before = chat_store_read_conversation(&paths)
            .expect("read ready snapshot before")
            .expect("ready snapshot exists");
        conversation.title = "updated title".to_string();
        conversation.messages.push(test_message("m3"));
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        chat_store_write_meta(&paths, &meta).expect("write meta only");
        let loaded_meta = chat_store_read_meta(&paths)
            .expect("read meta")
            .expect("ready meta exists");
        let loaded_after = chat_store_read_conversation(&paths)
            .expect("read ready snapshot after")
            .expect("ready snapshot exists");

        assert_eq!(loaded_meta.title, "updated title");
        assert_eq!(loaded_after.messages.len(), loaded_before.messages.len());
        assert_eq!(
            loaded_after
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            loaded_before
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(!loaded_after.messages.iter().any(|message| message.id == "m3"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_update_messages_without_touching_meta() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-messages-only-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        let meta_before = fs::read_to_string(&paths.meta_file).expect("read meta before");
        let updated = test_conversation(vec![test_message("m1"), test_message("m2")]);
        let snapshot = ConversationPersistMessagesSnapshot::from_conversation(&updated);

        let write = write_jsonl_snapshot_messages_shard(&paths, &snapshot)
            .expect("write messages only");
        let meta_after = fs::read_to_string(&paths.meta_file).expect("read meta after");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot");

        assert_eq!(write.message_count, 2);
        assert_eq!(write.last_message_id, "m2");
        assert_eq!(meta_after, meta_before);
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_append_message_without_decoding_existing_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");

        let mut updated = conversation.clone();
        let appended = test_message("m3");
        updated.updated_at = appended.created_at.clone();
        updated.last_assistant_at = Some(appended.created_at.clone());
        updated.messages.push(appended.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);
        let write = chat_store_append_message(&paths, &meta, &appended)
            .expect("append message");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );
        assert_eq!(loaded.updated_at, appended.created_at);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_append_first_message_to_empty_ready_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-empty-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(Vec::new());
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write empty ready snapshot");

        let appended = test_message("m1");
        let mut updated = conversation.clone();
        updated.updated_at = appended.created_at.clone();
        updated.last_assistant_at = Some(appended.created_at.clone());
        updated.messages.push(appended.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);

        let write = chat_store_append_message(&paths, &meta, &appended)
            .expect("append first message");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");

        assert_eq!(write.message_count, 1);
        assert_eq!(write.last_message_id, "m1");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1"]
        );
        assert_eq!(loaded.updated_at, appended.created_at);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_append_message_batch_with_one_file_copy() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-batch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");

        let appended_2 = test_message("m2");
        let appended_3 = test_message("m3");
        let mut meta_2 = ConversationPersistMeta::from_conversation(&conversation);
        meta_2.updated_at = appended_2.created_at.clone();
        meta_2.last_assistant_at = Some(appended_2.created_at.clone());
        let mut meta_3 = meta_2.clone();
        meta_3.updated_at = appended_3.created_at.clone();
        meta_3.last_assistant_at = Some(appended_3.created_at.clone());
        let write = chat_store_append_message_entries(
            &paths,
            &[(&meta_2, &appended_2), (&meta_3, &appended_3)],
            Some(&meta_3),
        )
        .expect("append message batch");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );
        assert_eq!(loaded.updated_at, appended_3.created_at);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_truncate_messages_by_index_prefix() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-truncate-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");
        conversation.messages.truncate(2);
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        let write = chat_store_truncate_messages(&paths, &meta, 2)
            .expect("truncate messages");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read truncated ready conversation")
            .expect("ready conversation exists");
        let status = chat_metadata_store_status(&paths)
            .expect("read status")
            .expect("status exists");

        assert_eq!(write.message_count, 2);
        assert_eq!(write.last_message_id, "m2");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        assert_eq!(status.message_count, 2);
        assert_eq!(status.last_message_id, "m2");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_replace_one_message_and_shift_offsets() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-replace-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");
        let mut updated_message = test_message("m2");
        updated_message.parts = vec![MessagePart::Text {
            text: "message m2 with much longer replacement content".to_string(),
                reasoning_content: None,
            }];
        let mut updated = conversation.clone();
        if let Some(message) = updated.messages.iter_mut().find(|item| item.id == "m2") {
            *message = updated_message.clone();
        }
        let meta = ConversationPersistMeta::from_conversation(&updated);

        let write = chat_store_replace_message(&paths, &meta, &updated_message)
            .expect("replace message");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read replaced ready conversation")
            .expect("ready conversation exists");
        let status = chat_metadata_store_status(&paths)
            .expect("read status")
            .expect("status exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );
        match loaded.messages[1].parts.first() {
            Some(MessagePart::Text { text, .. }) => assert!(text.contains("longer replacement")),
            _ => panic!("expected replaced text message"),
        }
        assert_eq!(status.message_count, 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_splice_messages_and_shift_offsets() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-splice-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        chat_store_write_snapshot(&paths, &conversation)
            .expect("write ready snapshot");
        let inserted = vec![test_message("r1")];
        conversation.messages.splice(1..2, inserted.clone());
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        let write = chat_store_splice_messages(
            &paths,
            &meta,
            1,
            1,
            &inserted,
        )
        .expect("splice messages");
        let loaded = chat_store_read_conversation(&paths)
            .expect("read spliced ready conversation")
            .expect("ready conversation exists");
        let status = chat_metadata_store_status(&paths)
            .expect("read status")
            .expect("status exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "r1", "m3"]
        );
        assert_eq!(status.message_count, 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_gate_should_only_allow_ready_jsonl_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-gate-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);

        assert!(!should_write_jsonl_snapshot_directory_shard(&paths).expect("missing manifest"));
        write_message_store_manifest_atomic(
            &paths.manifest_file,
            &MessageStoreManifest::jsonl_snapshot_building(&conversation),
        )
        .expect("write building manifest");
        assert!(!should_write_jsonl_snapshot_directory_shard(&paths).expect("building manifest"));
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write ready snapshot");
        assert!(should_write_jsonl_snapshot_directory_shard(&paths).expect("ready manifest"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_recover_from_ready_snapshot_missing_block_files() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-recover-missing-blocks-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write initial directory snapshot");
        fs::remove_dir_all(&paths.blocks_dir).expect("remove blocks dir");

        let mut updated = conversation.clone();
        updated.messages.push(test_message("m3"));
        let write = write_jsonl_snapshot_directory_shard(&paths, &updated)
            .expect("recover from missing block files");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read recovered conversation");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert!(paths.blocks_dir.join("000000.jsonl").exists());
        assert_eq!(
            loaded
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_if_changed_should_rewrite_when_ready_snapshot_is_stale() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-if-changed-stale-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write initial directory snapshot");

        let mut stale_manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");
        stale_manifest.source_message_count = 0;
        write_message_store_manifest_atomic(&paths.manifest_file, &stale_manifest)
            .expect("write stale manifest");

        let mut updated = conversation.clone();
        updated.messages.push(test_message("m2"));
        let changed = directory_store_write_if_changed(&paths, &updated)
            .expect("rewrite stale ready snapshot");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read recovered conversation");
        let healed_manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read healed manifest")
            .expect("healed manifest exists");

        assert!(changed);
        assert_eq!(
            loaded
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        assert_eq!(healed_manifest.source_message_count, 2);
        assert_eq!(healed_manifest.last_message_id, "m2");
        let _ = fs::remove_dir_all(root);
    }

    fn test_tool_message(
        id: &str,
        tool_pairs: Vec<(Value, Value)>,
        text: &str,
    ) -> ChatMessage {
        let mut message = ChatMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            created_at: "2026-04-24T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(tool_pairs.into_iter().flat_map(|(call, result)| vec![call, result]).collect()),
            mcp_call: None,
            meme_annotations: None,
        };
        if text.is_empty() {
            message.parts.clear();
        }
        message
    }

    fn tool_call_event(id: &str) -> Value {
        serde_json::json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": id,
                "type": "function",
                "function": { "name": "search", "arguments": "{}" }
            }]
        })
    }

    fn tool_result_event(id: &str) -> Value {
        serde_json::json!({
            "role": "tool",
            "tool_call_id": id,
            "content": "{\"ok\":true}"
        })
    }

    #[test]
    fn message_store_append_line_to_group_should_accumulate_tool_events_without_rebuild() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-line-tool-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        chat_store_write_snapshot(&paths, &conversation).expect("write ready snapshot");

        // 追加一条带 1 对工具的 assistant 消息（未闭合组：只有工具行，无正文）
        let tool_message = test_tool_message(
            "a1",
            vec![(tool_call_event("call-1"), tool_result_event("call-1"))],
            "",
        );
        let mut updated = conversation.clone();
        updated.messages.push(tool_message.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);
        chat_store_append_message(&paths, &meta, &tool_message)
            .expect("append tool message");

        // 工具事件累积：追加第 2 对工具，走组内追加子行
        let mut with_second_tool = tool_message.clone();
        with_second_tool.tool_call = Some(vec![
            tool_call_event("call-1"),
            tool_result_event("call-1"),
            tool_call_event("call-2"),
            tool_result_event("call-2"),
        ]);
        let mut updated2 = updated.clone();
        updated2.messages.pop();
        updated2.messages.push(with_second_tool.clone());
        let meta2 = ConversationPersistMeta::from_conversation(&updated2);
        chat_store_append_line_to_group(&paths, &meta2, &tool_message, &with_second_tool)
            .expect("append tool line to group");

        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");
        let a1 = loaded.messages.iter().find(|message| message.id == "a1").expect("a1 exists");
        assert_eq!(a1.tool_call.as_ref().expect("tool_call").len(), 4);
        assert!(a1.parts.is_empty(), "未闭合组无正文");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_append_line_to_group_should_append_final_text_to_tool_group() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-line-text-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        chat_store_write_snapshot(&paths, &conversation).expect("write ready snapshot");

        let tool_message = test_tool_message(
            "a1",
            vec![(tool_call_event("call-1"), tool_result_event("call-1"))],
            "",
        );
        let mut updated = conversation.clone();
        updated.messages.push(tool_message.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);
        chat_store_append_message(&paths, &meta, &tool_message)
            .expect("append tool message");

        // 正文累积：final text 写回，追加正文行承载 provider_meta
        let mut with_final = tool_message.clone();
        with_final.parts = vec![MessagePart::Text {
            text: "final answer".to_string(),
            reasoning_content: Some("reason".to_string()),
        }];
        with_final.provider_meta = Some(serde_json::json!({ "model": "gpt-4" }));
        let mut updated2 = updated.clone();
        updated2.messages.pop();
        updated2.messages.push(with_final.clone());
        let meta2 = ConversationPersistMeta::from_conversation(&updated2);
        chat_store_append_line_to_group(&paths, &meta2, &tool_message, &with_final)
            .expect("append final text line to group");

        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");
        let a1 = loaded.messages.iter().find(|message| message.id == "a1").expect("a1 exists");
        assert_eq!(a1.tool_call.as_ref().expect("tool_call").len(), 2);
        assert_eq!(
            a1.parts.iter().find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            }),
            Some("final answer")
        );
        assert_eq!(a1.provider_meta, Some(serde_json::json!({ "model": "gpt-4" })));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_append_line_to_group_should_fallback_replace_when_meta_patch() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-line-meta-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        chat_store_write_snapshot(&paths, &conversation).expect("write ready snapshot");

        let tool_message = test_tool_message(
            "a1",
            vec![(tool_call_event("call-1"), tool_result_event("call-1"))],
            "",
        );
        let mut updated = conversation.clone();
        updated.messages.push(tool_message.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);
        chat_store_append_message(&paths, &meta, &tool_message)
            .expect("append tool message");

        // 工具事件 + provider_meta patch：正文行未写，provider_meta 无处承载 → 回退 replace
        let mut with_meta_patch = tool_message.clone();
        with_meta_patch.tool_call = Some(vec![
            tool_call_event("call-1"),
            tool_result_event("call-1"),
            tool_call_event("call-2"),
            tool_result_event("call-2"),
        ]);
        with_meta_patch.provider_meta = Some(serde_json::json!({ "model": "gpt-4" }));
        let mut updated2 = updated.clone();
        updated2.messages.pop();
        updated2.messages.push(with_meta_patch.clone());
        let meta2 = ConversationPersistMeta::from_conversation(&updated2);
        chat_store_append_line_to_group(&paths, &meta2, &tool_message, &with_meta_patch)
            .expect("fallback replace on provider_meta patch");

        let loaded = chat_store_read_conversation(&paths)
            .expect("read ready snapshot")
            .expect("ready snapshot exists");
        let a1 = loaded.messages.iter().find(|message| message.id == "a1").expect("a1 exists");
        assert_eq!(a1.tool_call.as_ref().expect("tool_call").len(), 4);
        assert_eq!(a1.provider_meta, Some(serde_json::json!({ "model": "gpt-4" })));
        let _ = fs::remove_dir_all(root);
    }
}