#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MessageStoreDirectorySnapshotWrite {
    manifest: MessageStoreManifest,
    message_count: usize,
    last_message_id: String,
}

pub(super) fn write_jsonl_snapshot_directory_shard(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    let existing_manifest = read_message_store_manifest(&paths.manifest_file)?;
    if existing_manifest
        .as_ref()
        .is_some_and(MessageStoreManifest::should_read_jsonl)
    {
        return write_jsonl_snapshot_directory_shard_incremental(paths, &normalized_conversation);
    }
    write_jsonl_snapshot_directory_shard_full(paths, &normalized_conversation)
}

pub(super) fn write_jsonl_snapshot_directory_shard_if_changed(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<bool, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    if let Some(existing) = read_ready_message_store_directory_conversation(paths)? {
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
    write_jsonl_snapshot_directory_shard(paths, &normalized_conversation)?;
    Ok(true)
}

fn normalize_conversation_media_refs_for_message_store(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Conversation {
    let mut next = conversation.clone();
    let downloads_subdir = conversation.id.trim().to_string();
    for message in &mut next.messages {
        for part in &mut message.parts {
            match part {
                MessagePart::Image {
                    mime,
                    bytes_base64,
                    ..
                }
                | MessagePart::Audio {
                    mime,
                    bytes_base64,
                    ..
                } => {
                    let trimmed = bytes_base64.trim();
                    if trimmed.is_empty()
                        || stored_binary_ref_from_marker(trimmed).is_some()
                        || trimmed.starts_with("http://")
                        || trimmed.starts_with("https://")
                    {
                        continue;
                    }
                    match externalize_stored_binary_base64_in_downloads_subdir(
                        &paths.data_path,
                        &downloads_subdir,
                        mime,
                        bytes_base64,
                    ) {
                        Ok(next_ref) => {
                            if next_ref != *bytes_base64 {
                                *bytes_base64 = next_ref;
                            }
                        }
                        Err(err) => {
                            eprintln!(
                                "[消息存储] 媒体外置化失败，保留原始内容: conversation_id={}，message_id={}，mime={}，bytes_len={}，error={}",
                                paths.conversation_id,
                                message.id,
                                mime,
                                bytes_base64.len(),
                                err
                            );
                        }
                    }
                }
                MessagePart::Text { .. } => {}
            }
        }
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
        return Err(format!(
            "增量写入会话块失败：block 数量减少，需要结构性重建，conversation_id={}，old_count={}，new_count={}",
            paths.conversation_id,
            old_block_count,
            new_block_count
        ));
    }
    for (idx, block) in source_blocks.iter().enumerate().take(old_block_count) {
        if old_block_ids
            .get(idx)
            .is_some_and(|old_block_id| Some(*old_block_id) == Some(block.block_id))
        {
            continue;
        }
        return Err(format!(
            "增量写入会话块失败：block 顺序不一致，需要结构性重建，conversation_id={}，index={}，old_block={:?}，new_block={}",
            paths.conversation_id,
            idx,
            old_block_ids.get(idx),
            block.block_file
        ));
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
                should_slim_conversation_block(!conversation.summary.trim().is_empty(), idx, new_block_count);
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
    cleanup_stale_conversation_block_files_by_names(paths, &expected_block_files)
}

fn cleanup_stale_conversation_block_files_by_names(
    paths: &MessageStorePaths,
    expected_block_files: &std::collections::HashSet<String>,
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

pub(super) fn write_jsonl_snapshot_truncated_directory_shard(
    paths: &MessageStorePaths,
    conversation: &Conversation,
    keep_count: usize,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    let normalized_conversation =
        normalize_conversation_media_refs_for_message_store(paths, conversation);
    let manifest = read_message_store_manifest(&paths.manifest_file)?.ok_or_else(|| {
        format!(
            "截断会话块失败：缺少 manifest，conversation_id={}",
            paths.conversation_id
        )
    })?;
    if !manifest.should_read_jsonl() {
        return Err(format!(
            "截断会话块失败：目录型会话未处于 ready JSONL 状态，conversation_id={}",
            paths.conversation_id
        ));
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    let old_index = (*read_message_store_index_file(&paths.index_file)?).clone();
    if keep_count > old_index.items.len() {
        return Err(format!(
            "截断会话块失败：保留数量超过当前消息数，conversation_id={}，keep_count={}，message_count={}",
            paths.conversation_id,
            keep_count,
            old_index.items.len()
        ));
    }
    if normalized_conversation.messages.len() != keep_count {
        return Err(format!(
            "截断会话块失败：会话消息数与保留数量不一致，conversation_id={}，conversation_messages={}，keep_count={}",
            paths.conversation_id,
            normalized_conversation.messages.len(),
            keep_count
        ));
    }

    let old_block_ids = ordered_message_store_index_block_ids(&old_index);
    let source_blocks = split_conversation_messages_into_blocks(&normalized_conversation);
    let new_block_count = source_blocks.len();
    let prefix_count = old_block_ids.len().min(new_block_count);
    for (idx, block) in source_blocks.iter().enumerate().take(prefix_count) {
        if old_block_ids
            .get(idx)
            .is_some_and(|old_block_id| Some(*old_block_id) == Some(block.block_id))
        {
            continue;
        }
        return Err(format!(
            "截断会话块失败：block 顺序不一致，conversation_id={}，index={}，old_block={:?}，new_block={}",
            paths.conversation_id,
            idx,
            old_block_ids.get(idx),
            block.block_file
        ));
    }

    fs::create_dir_all(&paths.blocks_dir).map_err(|err| {
        format!(
            "创建会话块目录失败，conversation_id={}，path={}，error={err}",
            paths.conversation_id,
            paths.blocks_dir.display()
        )
    })?;

    let building_manifest = MessageStoreManifest::jsonl_snapshot_building(&normalized_conversation);
    write_message_store_manifest_atomic(&paths.manifest_file, &building_manifest)?;

    let mut next_items = Vec::<MessageStoreIndexItem>::with_capacity(keep_count);
    let mut invalidated_block_paths = std::collections::HashSet::<PathBuf>::new();
    for (idx, block_refs) in source_blocks.iter().enumerate() {
        let is_last_kept_block = idx + 1 == new_block_count;
        if is_last_kept_block {
            let should_slim = should_slim_conversation_block(
                !normalized_conversation.summary.trim().is_empty(),
                idx,
                new_block_count,
            );
            let block = build_jsonl_snapshot_conversation_block(block_refs, should_slim)?;
            let block_path = paths.shard_dir.join(&block.block_file);
            write_jsonl_snapshot_atomic(&block_path, &block.content)?;
            invalidated_block_paths.insert(block_path);
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

    if next_items.len() != keep_count {
        return Err(format!(
            "截断会话块失败：索引消息数量不一致，conversation_id={}，expected={}，actual={}",
            paths.conversation_id,
            keep_count,
            next_items.len()
        ));
    }

    let expected_block_files = source_blocks
        .iter()
        .map(|block| block.block_file.clone())
        .collect::<std::collections::HashSet<_>>();
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
            invalidated_block_paths.insert(block_path);
        }
    }
    cleanup_stale_conversation_block_files_by_names(paths, &expected_block_files)?;
    forget_message_store_block_file_cache_paths(&invalidated_block_paths);

    if paths.messages_file.exists() {
        fs::remove_file(&paths.messages_file).map_err(|err| {
            format!(
                "清理旧单文件 JSONL 失败，conversation_id={}，path={}，error={err}",
                paths.conversation_id,
                paths.messages_file.display()
            )
        })?;
    }

    let next_index = MessageStoreIndexFile::new(MESSAGE_STORE_MANIFEST_VERSION, next_items);
    let total_bytes = message_store_index_total_bytes(paths, &next_index)?;
    let last_message_id = next_index
        .items
        .last()
        .map(|item| item.message_id.clone())
        .unwrap_or_default();
    let next_manifest = MessageStoreManifest::jsonl_snapshot_building(&normalized_conversation)
        .jsonl_snapshot_ready(total_bytes, manifest.messages_index_revision + 1);
    let meta = ConversationShardMeta::from_conversation(&normalized_conversation);

    write_conversation_shard_meta_atomic(&paths.meta_file, &meta)?;
    write_message_store_index_atomic(&paths.index_file, &next_index)?;
    write_message_store_manifest_atomic(&paths.manifest_file, &next_manifest)?;

    Ok(MessageStoreDirectorySnapshotWrite {
        manifest: next_manifest,
        message_count: next_index.items.len(),
        last_message_id,
    })
}

pub(super) fn write_conversation_directory_meta_shard(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
) -> Result<(), String> {
    let shard_meta = ConversationShardMeta::from_persist_meta(meta);
    write_conversation_shard_meta_atomic(&paths.meta_file, &shard_meta)
}

pub(super) fn write_jsonl_snapshot_messages_shard(
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
    cleanup_stale_conversation_block_files_by_names(paths, &expected_block_files)?;
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

pub(super) fn write_jsonl_snapshot_appended_message_shard(
    paths: &MessageStorePaths,
    meta: &ConversationPersistMeta,
    message: &ChatMessage,
) -> Result<MessageStoreDirectorySnapshotWrite, String> {
    write_jsonl_snapshot_appended_messages_shard(paths, &[(meta, message)], Some(meta))
}

pub(super) fn write_jsonl_snapshot_appended_messages_shard(
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
    let manifest = read_message_store_manifest(&paths.manifest_file)?
        .ok_or_else(|| format!("追加 JSONL 消息失败：缺少 manifest，conversation_id={}", paths.conversation_id))?;
    if !manifest.should_read_jsonl() {
        return Err(format!(
            "追加 JSONL 消息失败：目录型会话未处于 ready JSONL 状态，conversation_id={}",
            paths.conversation_id
        ));
    }
    let mut current = read_message_store_directory_conversation(paths)?;
    for (_, message) in entries {
        if current.messages.iter().any(|item| item.id == message.id) {
            return Err(format!(
                "追加 JSONL 消息失败：消息 ID 已存在，conversation_id={}，message_id={}",
                paths.conversation_id,
                message.id
            ));
        }
        current.messages.push((*message).clone());
    }
    write_conversation_directory_meta_shard(paths, final_meta)?;
    let snapshot = ConversationPersistMessagesSnapshot::from_conversation(&current);
    write_jsonl_snapshot_messages_shard(paths, &snapshot)
}

pub(super) fn write_jsonl_snapshot_truncated_messages_shard(
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
    let manifest = read_message_store_manifest(&paths.manifest_file)?.ok_or_else(|| {
        format!(
            "截断 JSONL 消息失败：缺少 manifest，conversation_id={}",
            paths.conversation_id
        )
    })?;
    if !manifest.should_read_jsonl() {
        return Err(format!(
            "截断 JSONL 消息失败：目录型会话未处于 ready JSONL 状态，conversation_id={}",
            paths.conversation_id
        ));
    }
    let mut current = read_message_store_directory_conversation(paths)?;
    if keep_count > current.messages.len() {
        return Err(format!(
            "截断 JSONL 消息失败：保留数量超过当前消息数，conversation_id={}，keep_count={}，message_count={}",
            paths.conversation_id,
            keep_count,
            current.messages.len()
        ));
    }
    current.messages.truncate(keep_count);
    write_conversation_directory_meta_shard(paths, meta)?;
    let snapshot = ConversationPersistMessagesSnapshot::from_conversation(&current);
    write_jsonl_snapshot_messages_shard(paths, &snapshot)
}

pub(super) fn write_jsonl_snapshot_replaced_message_shard(
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
    let manifest = read_message_store_manifest(&paths.manifest_file)?.ok_or_else(|| {
        format!(
            "替换 JSONL 消息失败：缺少 manifest，conversation_id={}",
            paths.conversation_id
        )
    })?;
    if !manifest.should_read_jsonl() {
        return Err(format!(
            "替换 JSONL 消息失败：目录型会话未处于 ready JSONL 状态，conversation_id={}",
            paths.conversation_id
        ));
    }
    let mut current = read_message_store_directory_conversation(paths)?;
    let Some(position) = current.messages.iter().position(|item| item.id == message.id) else {
        return Err(format!(
            "替换 JSONL 消息失败：消息不存在，conversation_id={}，message_id={}",
            paths.conversation_id,
            message.id
        ));
    };
    current.messages[position] = message.clone();
    write_conversation_directory_meta_shard(paths, meta)?;
    let snapshot = ConversationPersistMessagesSnapshot::from_conversation(&current);
    write_jsonl_snapshot_messages_shard(paths, &snapshot)
}

pub(super) fn write_jsonl_snapshot_spliced_messages_shard(
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
    let manifest = read_message_store_manifest(&paths.manifest_file)?.ok_or_else(|| {
        format!(
            "拼接 JSONL 消息失败：缺少 manifest，conversation_id={}",
            paths.conversation_id
        )
    })?;
    if !manifest.should_read_jsonl() {
        return Err(format!(
            "拼接 JSONL 消息失败：目录型会话未处于 ready JSONL 状态，conversation_id={}",
            paths.conversation_id
        ));
    }
    let mut current = read_message_store_directory_conversation(paths)?;
    let old_len = current.messages.len();
    if start_index > old_len || delete_count > old_len.saturating_sub(start_index) {
        return Err(format!(
            "拼接 JSONL 消息失败：范围越界，conversation_id={}，start_index={}，delete_count={}，message_count={}",
            paths.conversation_id,
            start_index,
            delete_count,
            old_len
        ));
    }
    for message in inserted_messages {
        let message_id = message.id.trim();
        if message_id.is_empty() {
            return Err(format!(
                "拼接 JSONL 消息失败：插入消息 ID 为空，conversation_id={}",
                paths.conversation_id
            ));
        }
        if current
            .messages
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx < start_index || *idx >= start_index + delete_count)
            .any(|(_, item)| item.id.trim() == message_id)
        {
            return Err(format!(
                "拼接 JSONL 消息失败：消息 ID 冲突，conversation_id={}，message_id={}",
                paths.conversation_id,
                message_id
            ));
        }
    }
    current.messages.splice(start_index..start_index + delete_count, inserted_messages.iter().cloned());
    write_conversation_directory_meta_shard(paths, meta)?;
    let snapshot = ConversationPersistMessagesSnapshot::from_conversation(&current);
    write_jsonl_snapshot_messages_shard(paths, &snapshot)
}

pub(super) fn should_write_jsonl_snapshot_directory_shard(
    paths: &MessageStorePaths,
) -> Result<bool, String> {
    Ok(read_message_store_manifest(&paths.manifest_file)?
        .map(|manifest| manifest.should_write_jsonl_snapshot())
        .unwrap_or(false))
}

pub(super) fn write_jsonl_snapshot_building_manifest(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<(), String> {
    write_message_store_manifest_atomic(
        &paths.manifest_file,
        &MessageStoreManifest::jsonl_snapshot_building(conversation),
    )
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
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-persist".to_string(),
            title: "persist".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            archived_at: None,
            messages,
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    #[test]
    fn message_store_persist_should_write_directory_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-persist-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
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
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        let loaded_before = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot before");
        conversation.title = "updated title".to_string();
        conversation.messages.push(test_message("m3"));
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        write_conversation_directory_meta_shard(&paths, &meta).expect("write meta only");
        let loaded_meta = read_conversation_shard_meta(&paths.meta_file).expect("read meta");
        let loaded_after = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot after");

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
        let data_path = root.join("app_data.json");
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
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1"), test_message("m2")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");

        let mut updated = conversation.clone();
        let appended = test_message("m3");
        updated.updated_at = appended.created_at.clone();
        updated.last_assistant_at = Some(appended.created_at.clone());
        updated.messages.push(appended.clone());
        let meta = ConversationPersistMeta::from_conversation(&updated);
        let write = write_jsonl_snapshot_appended_message_shard(&paths, &meta, &appended)
            .expect("append message");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot");

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
    fn message_store_persist_should_append_message_batch_with_one_file_copy() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-append-batch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![test_message("m1")]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");

        let appended_2 = test_message("m2");
        let appended_3 = test_message("m3");
        let mut meta_2 = ConversationPersistMeta::from_conversation(&conversation);
        meta_2.updated_at = appended_2.created_at.clone();
        meta_2.last_assistant_at = Some(appended_2.created_at.clone());
        let mut meta_3 = meta_2.clone();
        meta_3.updated_at = appended_3.created_at.clone();
        meta_3.last_assistant_at = Some(appended_3.created_at.clone());
        let write = write_jsonl_snapshot_appended_messages_shard(
            &paths,
            &[(&meta_2, &appended_2), (&meta_3, &appended_3)],
            Some(&meta_3),
        )
        .expect("append message batch");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read directory snapshot");

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
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        conversation.messages.truncate(2);
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        let write = write_jsonl_snapshot_truncated_messages_shard(&paths, &meta, 2)
            .expect("truncate messages");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read truncated directory");
        let index = read_message_store_index_file(&paths.index_file).expect("read index");
        let manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");

        assert_eq!(write.message_count, 2);
        assert_eq!(write.last_message_id, "m2");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2"]
        );
        assert_eq!(index.items.len(), 2);
        assert_eq!(manifest.source_message_count, 2);
        assert_eq!(manifest.last_message_id, "m2");
        assert_eq!(
            manifest.messages_jsonl_bytes,
            message_store_index_total_bytes(&paths, &index).expect("total bytes")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_replace_one_message_and_shift_offsets() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-replace-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        let mut updated_message = test_message("m2");
        updated_message.parts = vec![MessagePart::Text {
            text: "message m2 with much longer replacement content".to_string(),
        }];
        let mut updated = conversation.clone();
        if let Some(message) = updated.messages.iter_mut().find(|item| item.id == "m2") {
            *message = updated_message.clone();
        }
        let meta = ConversationPersistMeta::from_conversation(&updated);

        let write = write_jsonl_snapshot_replaced_message_shard(&paths, &meta, &updated_message)
            .expect("replace message");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read replaced directory");
        let index = read_message_store_index_file(&paths.index_file).expect("read index");
        let manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "m2", "m3"]
        );
        match loaded.messages[1].parts.first() {
            Some(MessagePart::Text { text }) => assert!(text.contains("longer replacement")),
            _ => panic!("expected replaced text message"),
        }
        assert_eq!(index.items.len(), 3);
        assert_eq!(
            manifest.messages_jsonl_bytes,
            message_store_index_total_bytes(&paths, &index).expect("total bytes")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_should_splice_messages_and_shift_offsets() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-splice-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-persist").expect("paths");
        let mut conversation = test_conversation(vec![
            test_message("m1"),
            test_message("m2"),
            test_message("m3"),
        ]);
        write_jsonl_snapshot_directory_shard(&paths, &conversation)
            .expect("write directory snapshot");
        let inserted = vec![test_message("r1")];
        conversation.messages.splice(1..2, inserted.clone());
        let meta = ConversationPersistMeta::from_conversation(&conversation);

        let write = write_jsonl_snapshot_spliced_messages_shard(
            &paths,
            &meta,
            1,
            1,
            &inserted,
        )
        .expect("splice messages");
        let loaded = read_message_store_directory_conversation(&paths)
            .expect("read spliced directory");
        let index = read_message_store_index_file(&paths.index_file).expect("read index");
        let manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");

        assert_eq!(write.message_count, 3);
        assert_eq!(write.last_message_id, "m3");
        assert_eq!(
            loaded.messages.iter().map(|message| message.id.as_str()).collect::<Vec<_>>(),
            vec!["m1", "r1", "m3"]
        );
        assert_eq!(index.items.len(), 3);
        assert_eq!(
            manifest.messages_jsonl_bytes,
            message_store_index_total_bytes(&paths, &index).expect("total bytes")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_persist_gate_should_only_allow_ready_jsonl_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-gate-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
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
}
