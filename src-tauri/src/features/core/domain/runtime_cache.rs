#[derive(Debug, Clone, Default)]
struct CacheReadDetail {
    source: String,
    dirty_fast_path: bool,
    mtime_before_ms: u64,
    cache_lookup_ms: u64,
    disk_read_ms: u64,
    mtime_after_ms: u64,
    cache_write_ms: u64,
    total_ms: u64,
}

fn path_modified_time(path: &PathBuf) -> Option<std::time::SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn state_read_config_cached_with_detail(
    state: &AppState,
) -> Result<(AppConfig, CacheReadDetail), String> {
    let total_started = std::time::Instant::now();
    let mtime_started = std::time::Instant::now();
    let disk_mtime = path_modified_time(&state.config_path);
    let mtime_before_ms = mtime_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let cache_lookup_started = std::time::Instant::now();
    {
        let cached = state
            .cached_config
            .lock()
            .map_err(|_| "Failed to lock cached config".to_string())?;
        let cached_mtime = state
            .cached_config_mtime
            .lock()
            .map_err(|_| "Failed to lock cached config mtime".to_string())?;
        if let (Some(config), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                let cache_lookup_ms = cache_lookup_started
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64;
                let detail = CacheReadDetail {
                    source: "cache_hit".to_string(),
                    dirty_fast_path: false,
                    mtime_before_ms,
                    cache_lookup_ms,
                    disk_read_ms: 0,
                    mtime_after_ms: 0,
                    cache_write_ms: 0,
                    total_ms: total_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                };
                return Ok((config.clone(), detail));
            }
        }
    }
    let cache_lookup_ms = cache_lookup_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;

    let disk_read_started = std::time::Instant::now();
    let config = read_config(&state.config_path)?;
    let disk_read_ms = disk_read_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let mtime_after_started = std::time::Instant::now();
    let disk_mtime = path_modified_time(&state.config_path);
    let mtime_after_ms = mtime_after_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let cache_write_started = std::time::Instant::now();
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    let cache_write_ms = cache_write_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let detail = CacheReadDetail {
        source: "disk_read".to_string(),
        dirty_fast_path: false,
        mtime_before_ms,
        cache_lookup_ms,
        disk_read_ms,
        mtime_after_ms,
        cache_write_ms,
        total_ms: total_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64,
    };
    Ok((config, detail))
}

fn state_read_config_cached(state: &AppState) -> Result<AppConfig, String> {
    state_read_config_cached_with_detail(state).map(|(config, _detail)| config)
}

fn state_write_config_cached(state: &AppState, config: &AppConfig) -> Result<(), String> {
    write_config(&state.config_path, config)?;
    let disk_mtime = path_modified_time(&state.config_path);
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    clear_terminal_config_allowed_workspaces_cache_for_state(state);
    Ok(())
}

fn sync_cached_app_data_signature(state: &AppState) -> Result<(), String> {
    *state
        .cached_app_data_signature
        .lock()
        .map_err(|_| "Failed to lock cached app data signature".to_string())? =
        Some(app_data_cache_signature(&state.data_path));
    Ok(())
}

fn sort_chat_index_items(items: &mut Vec<ChatIndexConversationItem>) {
    items.sort_by(|a, b| {
        a.updated_at
            .cmp(&b.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn collect_chat_index_items_from_storage(
    data_path: &PathBuf,
) -> Result<Vec<ChatIndexConversationItem>, String> {
    Ok(message_store::chat_metadata_store_list_chat_index(data_path)?.unwrap_or_default())
}

fn sync_cached_app_data_agents(state: &AppState, agents: &[AgentProfile]) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        data.agents = agents.to_vec();
    }
    drop(cached);
    sync_cached_app_data_signature(state)
}

fn sanitize_runtime_cached_app_data(data: &mut AppData) {
    data.conversations.clear();
}

fn sync_cached_app_data_conversation(
    state: &AppState,
    _conversation: &Conversation,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        sanitize_runtime_cached_app_data(data);
    }
    drop(cached);
    sync_cached_app_data_signature(state)
}

fn sync_cached_app_data_conversation_metadata(
    state: &AppState,
    _conversation: &Conversation,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        sanitize_runtime_cached_app_data(data);
    }
    drop(cached);
    // 不在此处重算磁盘签名：本路径只改内存元数据（未落盘），磁盘文件未变，
    // 签名必然与之前相同；且调用方随后会 refresh_cached_app_data_dirty 置脏，
    // 读路径走 dirty_cache_hit 不比较签名。落盘完成后 persist worker 会重算。
    Ok(())
}

fn sync_cached_app_data_conversation_deleted(
    state: &AppState,
    _conversation_id: &str,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        sanitize_runtime_cached_app_data(data);
    }
    drop(cached);
    sync_cached_app_data_signature(state)
}

fn sync_cached_conversation_metadata(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let mut metadata = state
        .cached_conversation_metadata
        .lock()
        .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
    metadata.insert(
        conversation.id.clone(),
        message_store::ConversationShardMeta::from_conversation(conversation),
    );
    Ok(())
}

fn lock_cached_conversation_field_metadata_ids(
    state: &AppState,
) -> std::sync::MutexGuard<'_, std::collections::HashSet<String>> {
    match state.cached_conversation_field_metadata_ids.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_warn(
                "[会话元数据] 字段权威标记锁已中毒，恢复锁并继续".to_string(),
            );
            state.cached_conversation_field_metadata_ids.clear_poison();
            poisoned.into_inner()
        }
    }
}

fn conversation_meta_needs_message_derived_repair(
    meta: &message_store::ConversationShardMeta,
) -> bool {
    if meta.message_count() > 0 && meta.preview_messages().is_empty() {
        return true;
    }
    meta.message_count() == 0
        && meta.body_message_count() == 0
        && !meta.has_assistant_reply()
        && meta.preview_messages().is_empty()
        && meta
            .latest_summary_title()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
}

fn repair_conversation_metadata_message_derived_fields_if_needed(
    state: &AppState,
    conversation_id: &str,
    meta: &message_store::ConversationShardMeta,
) -> Result<message_store::ConversationShardMeta, String> {
    if !conversation_meta_needs_message_derived_repair(meta) {
        return Ok(meta.clone());
    }
    let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    let ready_meta = match message_store::chat_store_read_meta(&store_paths) {
        Ok(Some(ready_meta)) => ready_meta,
        Ok(None) => return Ok(meta.clone()),
        Err(err) => {
            runtime_log_warn(format!(
                "[会话元数据] 消息派生字段修复读取失败，保留现有元数据继续，conversation_id={}，error={}",
                conversation_id, err
            ));
            return Ok(meta.clone());
        }
    };
    let should_repair =
        (meta.message_count() == 0 && ready_meta.message_count() > 0)
            || (meta.body_message_count() == 0 && ready_meta.body_message_count() > 0)
            || (!meta.has_assistant_reply() && ready_meta.has_assistant_reply())
            || (meta.preview_messages().is_empty() && !ready_meta.preview_messages().is_empty())
            || (meta
                .latest_summary_title()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
                && ready_meta
                    .latest_summary_title()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some());
    if !should_repair {
        return Ok(meta.clone());
    }
    {
        let mut cached = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        cached.insert(conversation_id.to_string(), ready_meta.clone());
    }
    Ok(ready_meta)
}

fn remove_cached_conversation_metadata(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mut metadata = state
        .cached_conversation_metadata
        .lock()
        .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
    metadata.remove(conversation_id);
    drop(metadata);
    lock_cached_conversation_field_metadata_ids(state).remove(conversation_id);
    Ok(())
}

fn apply_cached_conversation_metadata(
    state: &AppState,
    conversation: &mut Conversation,
) -> Result<(), String> {
    let metadata = state
        .cached_conversation_metadata
        .lock()
        .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
    if let Some(source) = metadata.get(&conversation.id) {
        source.apply_to_conversation(conversation);
    }
    Ok(())
}

fn state_read_conversation_metadata_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<message_store::ConversationShardMeta, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let deleted_fast_path = state
        .cached_deleted_conversation_ids
        .lock()
        .map(|deleted_ids| deleted_ids.contains(conversation_id))
        .unwrap_or(false);
    if deleted_fast_path {
        return Err(format!("Conversation not found: {}", conversation_id));
    }
    let dirty_fast_path = state
        .cached_conversation_dirty_ids
        .lock()
        .map(|dirty_ids| dirty_ids.contains(conversation_id))
        .unwrap_or(false);
    if dirty_fast_path {
        let cached_meta = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?
            .get(conversation_id)
            .cloned();
        if let Some(meta) = cached_meta {
            return repair_conversation_metadata_message_derived_fields_if_needed(
                state,
                conversation_id,
                &meta,
            );
        }
    }
    let disk_mtime = conversation_shard_modified_time(&state.data_path, conversation_id);
    let cached_hit = {
        let cached = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        let cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        if let (Some(meta), Some(cached_mtime), Some(disk_time)) = (
            cached.get(conversation_id),
            cached_mtimes.get(conversation_id),
            disk_mtime,
        ) {
            if *cached_mtime == Some(disk_time) {
                Some(meta.clone())
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(meta) = cached_hit {
        return repair_conversation_metadata_message_derived_fields_if_needed(
            state,
            conversation_id,
            &meta,
        );
    }
    let meta = match read_conversation_meta_shard(&state.data_path, conversation_id) {
        Ok(meta) => meta,
        Err(err) => {
            let pending_meta = state
                .conversation_persist_pending
                .lock()
                .map_err(|_| "Failed to lock pending conversation persist".to_string())?
                .as_ref()
                .and_then(|slot| slot.conversations.get(conversation_id))
                .map(message_store::ConversationShardMeta::from_conversation);
            let cached_meta = pending_meta.or_else(|| {
                state
                .cached_conversation_metadata
                .lock()
                    .ok()
                    .and_then(|cached| cached.get(conversation_id).cloned())
            });
            let Some(cached_meta) = cached_meta else {
                return Err(err);
            };
            runtime_log_warn(format!(
                "[会话元数据] 磁盘读取失败，使用最后缓存快照继续，conversation_id={}，error={}",
                conversation_id, err
            ));
            return repair_conversation_metadata_message_derived_fields_if_needed(
                state,
                conversation_id,
                &cached_meta,
            );
        }
    };
    {
        let mut cached = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        cached.insert(conversation_id.to_string(), meta.clone());
    }
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation_id.to_string(), disk_mtime);
    }
    repair_conversation_metadata_message_derived_fields_if_needed(state, conversation_id, &meta)
}

#[cfg(test)]
fn state_mark_conversation_direct_persisted(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    sync_cached_conversation_metadata(state, conversation)?;
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation.id.clone(), disk_mtime);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.remove(&conversation.id);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(&conversation.id);
    }
    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let should_clear_slot = if let Some(slot) = pending.as_mut() {
            slot.conversations.remove(&conversation.id);
            slot.metadata_conversation_ids.remove(&conversation.id);
            slot.deleted_conversation_ids.remove(&conversation.id);
            slot.conversations.is_empty()
                && slot.metadata_conversation_ids.is_empty()
                && slot.deleted_conversation_ids.is_empty()
        } else {
            false
        };
        if should_clear_slot {
            *pending = None;
        }
    }
    sync_cached_app_data_conversation(state, &conversation)?;
    state_upsert_chat_index_conversation_cached(state, &conversation)?;
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_mark_conversation_metadata_direct_persisted(
    state: &AppState,
    conversation_id: &str,
) -> Result<message_store::ConversationShardMeta, String> {
    let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    let meta = read_conversation_meta_shard(&state.data_path, conversation_id)?;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, conversation_id);
    {
        let mut cached = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        cached.insert(conversation_id.to_string(), meta.clone());
    }
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation_id.to_string(), disk_mtime);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.remove(conversation_id);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(conversation_id);
    }
    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let should_clear_slot = if let Some(slot) = pending.as_mut() {
            slot.conversations.remove(conversation_id);
            slot.metadata_conversation_ids.remove(conversation_id);
            slot.deleted_conversation_ids.remove(conversation_id);
            slot.conversations.is_empty()
                && slot.metadata_conversation_ids.is_empty()
                && slot.deleted_conversation_ids.is_empty()
        } else {
            false
        };
        if should_clear_slot {
            *pending = None;
        }
    }
    let metadata_conversation =
        conversation_service_v2().build_conversation_snapshot_from_meta(&meta, Vec::new());
    state_upsert_chat_index_conversation_cached(state, &metadata_conversation)?;
    refresh_cached_app_data_dirty(state);
    Ok(meta)
}

/// 用最终确定的元数据覆盖内存缓存（不改变 dirty/pending/seq 状态）。
/// 用于 replace 提交路径：统一派生规则重算摘要标题后，缓存必须与持久化输入一致。
fn state_override_conversation_metadata_cached(
    state: &AppState,
    conversation_id: &str,
    meta: &message_store::ConversationShardMeta,
) -> Result<(), String> {
    let mut metadata = state
        .cached_conversation_metadata
        .lock()
        .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
    metadata.insert(conversation_id.trim().to_string(), meta.clone());
    Ok(())
}

fn state_mark_conversation_metadata_cached_persisted_unlocked(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let disk_mtime = conversation_shard_modified_time(&state.data_path, conversation_id);
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation_id.to_string(), disk_mtime);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.remove(conversation_id);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(conversation_id);
    }
    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let should_clear_slot = if let Some(slot) = pending.as_mut() {
            slot.conversations.remove(conversation_id);
            slot.metadata_conversation_ids.remove(conversation_id);
            slot.deleted_conversation_ids.remove(conversation_id);
            slot.conversations.is_empty()
                && slot.metadata_conversation_ids.is_empty()
                && slot.deleted_conversation_ids.is_empty()
        } else {
            false
        };
        if should_clear_slot {
            *pending = None;
        }
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_update_conversation_metadata_cached<T>(
    state: &AppState,
    conversation_id: &str,
    updater: impl FnOnce(&mut Conversation) -> Result<T, String>,
) -> Result<(Conversation, T, u64), String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let mutation_gate = conversation_mutation_gate(&state.data_path, normalized_conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    state_update_conversation_metadata_cached_unlocked(state, normalized_conversation_id, updater)
}

fn state_update_conversation_metadata_cached_unlocked<T>(
    state: &AppState,
    normalized_conversation_id: &str,
    updater: impl FnOnce(&mut Conversation) -> Result<T, String>,
) -> Result<(Conversation, T, u64), String> {
    let conversation_meta =
        state_read_conversation_metadata_cached(state, normalized_conversation_id)?;
    let mut conversation = conversation_service_v2()
        .build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
    let result = updater(&mut conversation)?;
    let mut updated_meta = message_store::ConversationShardMeta::from_conversation(&conversation);
    updated_meta.preserve_message_derived_fields_from(&conversation_meta);
    {
        let mut metadata = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        metadata.insert(conversation.id.clone(), updated_meta.clone());
    }
    lock_cached_conversation_field_metadata_ids(state).insert(conversation.id.clone());
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation.id.clone(), disk_mtime);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(&conversation.id);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.insert(conversation.id.clone());
    }
    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            metadata_conversation_ids: std::collections::HashSet::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
        });
        slot.seq = seq;
        if let Some(pending_conversation) = slot.conversations.get_mut(&conversation.id) {
            updated_meta.apply_to_conversation(pending_conversation);
        } else {
            slot.metadata_conversation_ids.insert(conversation.id.clone());
        }
        slot.deleted_conversation_ids.remove(&conversation.id);
    }
    sync_cached_app_data_conversation_metadata(state, &conversation)?;
    state_upsert_chat_index_conversation_cached(state, &conversation)?;
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok((conversation, result, seq))
}

fn state_update_conversation_meta_cached_unlocked<T>(
    state: &AppState,
    normalized_conversation_id: &str,
    updater: impl FnOnce(&mut message_store::ConversationShardMeta) -> Result<T, String>,
) -> Result<(message_store::ConversationShardMeta, T, u64), String> {
    let mut conversation_meta =
        state_read_conversation_metadata_cached(state, normalized_conversation_id)?;
    let result = updater(&mut conversation_meta)?;
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, normalized_conversation_id);
    {
        let mut metadata = state
            .cached_conversation_metadata
            .lock()
            .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
        metadata.insert(normalized_conversation_id.to_string(), conversation_meta.clone());
    }
    lock_cached_conversation_field_metadata_ids(state)
        .insert(normalized_conversation_id.to_string());
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(normalized_conversation_id.to_string(), disk_mtime);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(normalized_conversation_id);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.insert(normalized_conversation_id.to_string());
    }
    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            metadata_conversation_ids: std::collections::HashSet::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
        });
        slot.seq = seq;
        if let Some(conversation) = slot.conversations.get_mut(normalized_conversation_id) {
            conversation_meta.apply_to_conversation(conversation);
            slot.metadata_conversation_ids
                .remove(normalized_conversation_id);
        } else {
            slot.metadata_conversation_ids
                .insert(normalized_conversation_id.to_string());
        }
        slot.deleted_conversation_ids
            .remove(normalized_conversation_id);
    }
    let metadata_conversation =
        conversation_service_v2().build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
    sync_cached_app_data_conversation_metadata(state, &metadata_conversation)?;
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok((conversation_meta, result, seq))
}

fn has_pending_conversation_persist(state: &AppState) -> bool {
    let has_pending_slot = state
        .conversation_persist_pending
        .lock()
        .map(|pending| pending.is_some())
        .unwrap_or(true);
    let has_dirty_conversations = state
        .cached_conversation_dirty_ids
        .lock()
        .map(|dirty_ids| !dirty_ids.is_empty())
        .unwrap_or(true);
    let has_deleted_conversations = state
        .cached_deleted_conversation_ids
        .lock()
        .map(|deleted_ids| !deleted_ids.is_empty())
        .unwrap_or(true);
    has_pending_slot || has_dirty_conversations || has_deleted_conversations
}

fn refresh_cached_app_data_dirty(state: &AppState) {
    let dirty = has_pending_conversation_persist(state);
    state
        .cached_app_data_dirty
        .store(dirty, std::sync::atomic::Ordering::Release);
}

fn conversation_shard_modified_time(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Option<std::time::SystemTime> {
    message_store::message_store_paths(data_path, conversation_id)
        .ok()
        .and_then(|paths| message_store::message_store_shard_modified_time(&paths))
}

fn state_read_conversation_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<Conversation, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let deleted_fast_path = state
        .cached_deleted_conversation_ids
        .lock()
        .map(|deleted_ids| deleted_ids.contains(conversation_id))
        .unwrap_or(false);
    if deleted_fast_path {
        return Err(format!("Conversation not found: {}", conversation_id));
    }
    let dirty_fast_path = state
        .cached_conversation_dirty_ids
        .lock()
        .map(|dirty_ids| dirty_ids.contains(conversation_id))
        .unwrap_or(false);
    if dirty_fast_path {
        let pending_conversation = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?
            .as_ref()
            .and_then(|slot| slot.conversations.get(conversation_id))
            .cloned();
        if let Some(conversation) = pending_conversation {
            sync_cached_conversation_metadata(state, &conversation)?;
            return Ok(conversation);
        }
    }
    let mut conversation = read_conversation_shard(&state.data_path, conversation_id)?;
    apply_cached_conversation_metadata(state, &mut conversation)?;
    sync_cached_conversation_metadata(state, &conversation)?;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, conversation_id);
    state
        .cached_conversation_mtimes
        .lock()
        .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?
        .insert(conversation_id.to_string(), disk_mtime);
    Ok(conversation)
}

fn state_read_chat_index_cached(state: &AppState) -> Result<ChatIndexFile, String> {
    {
        let cached = state
            .cached_chat_index
            .lock()
            .map_err(|_| "Failed to lock cached chat index".to_string())?;
        if let Some(index) = cached.as_ref() {
            return Ok(index.clone());
        }
    }
    let mut conversations = collect_chat_index_items_from_storage(&state.data_path)?;
    sort_chat_index_items(&mut conversations);
    let index = ChatIndexFile { conversations };
    *state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(index.clone());
    Ok(index)
}

fn state_upsert_chat_index_conversation_cached(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let mut index = state_read_chat_index_cached(state)?;
    upsert_chat_index_conversation(&mut index, conversation);
    sort_chat_index_items(&mut index.conversations);
    *state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(index);
    Ok(())
}

fn state_remove_chat_index_conversation_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mut index = state_read_chat_index_cached(state)?;
    remove_chat_index_conversation(&mut index, conversation_id);
    *state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(index);
    Ok(())
}

fn preserve_field_level_conversation_metadata(
    target: &mut Conversation,
    source: &Conversation,
) {
    target.title = source.title.clone();
    target.agent_id = source.agent_id.clone();
    target.bound_conversation_id = source.bound_conversation_id.clone();
    target.parent_conversation_id = source.parent_conversation_id.clone();
    target.child_conversation_ids = source.child_conversation_ids.clone();
    target.fork_message_cursor = source.fork_message_cursor.clone();
    target.conversation_kind = source.conversation_kind.clone();
    target.root_conversation_id = source.root_conversation_id.clone();
    target.delegate_id = source.delegate_id.clone();
    target.created_at = source.created_at.clone();
    target.shell_workspace_path = source.shell_workspace_path.clone();
    target.shell_workspaces = source.shell_workspaces.clone();
    target.shell_autonomous_mode = source.shell_autonomous_mode;
    target.shell_work_mode = source.shell_work_mode.clone();
    target.unread_count = source.unread_count;
    target.updated_at = source.updated_at.clone();
    target.last_user_at = source.last_user_at.clone();
    target.last_assistant_at = source.last_assistant_at.clone();
    target.status = source.status.clone();
    target.archived_at = source.archived_at.clone();
    target.current_todos = source.current_todos.clone();
    target.user_profile_snapshot = source.user_profile_snapshot.clone();
    target.memory_recall_table = source.memory_recall_table.clone();
    target.plan_mode_enabled = source.plan_mode_enabled;
    target.preferred_api_config_id = source.preferred_api_config_id.clone();
    target.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
    target.active_goal = source.active_goal.clone();
    target.fast_request_turns = source.fast_request_turns.clone();
}

#[allow(dead_code)]
fn state_write_conversation_cached(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error(
            "conversation_mutation_gate",
            file!(),
            line!(),
            module_path!(),
            &err,
        )
    })?;
    let _ = write_conversation_shard(&state.data_path, conversation)?;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    sync_cached_conversation_metadata(state, conversation)?;
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation.id.clone(), disk_mtime);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(&conversation.id);
    }
    sync_cached_app_data_conversation(state, conversation)?;
    state_upsert_chat_index_conversation_cached(state, conversation)?;
    refresh_cached_app_data_dirty(state);
    Ok(())
}

#[allow(dead_code)]
fn state_delete_conversation_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error(
            "conversation_mutation_gate",
            file!(),
            line!(),
            module_path!(),
            &err,
        )
    })?;
    let _ = delete_conversation_shard(&state.data_path, conversation_id)?;
    remove_cached_conversation_metadata(state, conversation_id)?;
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.remove(conversation_id);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.insert(conversation_id.to_string());
    }
    sync_cached_app_data_conversation_deleted(state, conversation_id)?;
    state_remove_chat_index_conversation_cached(state, conversation_id)?;
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_read_agents_cached(state: &AppState) -> Result<Vec<AgentProfile>, String> {
    let disk_mtime = path_modified_time(&app_layout_agents_path(&state.data_path));
    {
        let cached = state
            .cached_agents
            .lock()
            .map_err(|_| "Failed to lock cached agents".to_string())?;
        let cached_mtime = state
            .cached_agents_mtime
            .lock()
            .map_err(|_| "Failed to lock cached agents mtime".to_string())?;
        if let (Some(agents), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(agents.clone());
            }
        }
    }
    let agents = read_agents_shard(&state.data_path)?;
    *state
        .cached_agents
        .lock()
        .map_err(|_| "Failed to lock cached agents".to_string())? = Some(agents.clone());
    *state
        .cached_agents_mtime
        .lock()
        .map_err(|_| "Failed to lock cached agents mtime".to_string())? = disk_mtime;
    Ok(agents)
}

fn state_write_agents_cached(state: &AppState, agents: &[AgentProfile]) -> Result<(), String> {
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    let _ = write_agents_shard(&state.data_path, agents)?;
    let disk_mtime = path_modified_time(&app_layout_agents_path(&state.data_path));
    *state
        .cached_agents
        .lock()
        .map_err(|_| "Failed to lock cached agents".to_string())? = Some(agents.to_vec());
    *state
        .cached_agents_mtime
        .lock()
        .map_err(|_| "Failed to lock cached agents mtime".to_string())? = disk_mtime;
    sync_cached_app_data_agents(state, agents)?;
    refresh_cached_app_data_dirty(state);
    Ok(())
}

#[cfg(test)]
fn state_read_app_data_cached_with_detail(
    state: &AppState,
) -> Result<(AppData, CacheReadDetail), String> {
    let (data, detail) = ensure_app_data_cache_ready_inner(state, true)?;
    let data = data.ok_or_else(|| "Cached app data is unexpectedly missing".to_string())?;
    Ok((data, detail))
}

#[cfg(test)]
fn state_read_app_data_cached(state: &AppState) -> Result<AppData, String> {
    state_read_app_data_cached_with_detail(state).map(|(data, _detail)| data)
}

#[cfg(test)]
fn ensure_app_data_cache_ready_inner(
    state: &AppState,
    return_data: bool,
) -> Result<(Option<AppData>, CacheReadDetail), String> {
    let total_started = std::time::Instant::now();
    let dirty_fast_path = state
        .cached_app_data_dirty
        .load(std::sync::atomic::Ordering::Acquire);
    if dirty_fast_path {
        let cache_lookup_started = std::time::Instant::now();
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        if let Some(data) = cached.as_ref() {
            return Ok((
                return_data.then(|| data.clone()),
                CacheReadDetail {
                    source: "dirty_cache_hit".to_string(),
                    dirty_fast_path: true,
                    mtime_before_ms: 0,
                    cache_lookup_ms: cache_lookup_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                    disk_read_ms: 0,
                    mtime_after_ms: 0,
                    cache_write_ms: 0,
                    total_ms: total_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                },
            ));
        }
    }

    let mtime_started = std::time::Instant::now();
    let disk_signature = app_data_cache_signature(&state.data_path);
    let mtime_before_ms = mtime_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let cache_lookup_started = std::time::Instant::now();
    {
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        let cached_signature = state
            .cached_app_data_signature
            .lock()
            .map_err(|_| "Failed to lock cached app data signature".to_string())?;
        if let (Some(_data), Some(signature)) = (cached.as_ref(), cached_signature.as_ref()) {
            if *signature == disk_signature {
                return Ok((
                    if return_data {
                        cached.as_ref().cloned()
                    } else {
                        None
                    },
                    CacheReadDetail {
                        source: "cache_hit".to_string(),
                        dirty_fast_path,
                        mtime_before_ms,
                        cache_lookup_ms: cache_lookup_started
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u64::MAX)) as u64,
                        disk_read_ms: 0,
                        mtime_after_ms: 0,
                        cache_write_ms: 0,
                        total_ms: total_started
                            .elapsed()
                            .as_millis()
                            .min(u128::from(u64::MAX)) as u64,
                    },
                ));
            }
        }
    }
    let cache_lookup_ms = cache_lookup_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;

    let disk_read_started = std::time::Instant::now();
    let mut data = read_layout_app_data(&state.data_path)?;
    for conversation in data.conversations.iter_mut() {
        normalize_conversation_runtime_volatile_fields(conversation);
    }
    sanitize_runtime_cached_app_data(&mut data);
    let disk_read_ms = disk_read_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let mtime_after_started = std::time::Instant::now();
    let disk_signature = app_data_cache_signature(&state.data_path);
    let mtime_after_ms = mtime_after_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let cache_write_started = std::time::Instant::now();
    let conversation_count = data.conversations.len();
    let data_for_return = return_data.then(|| data.clone());
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data);
    *state
        .cached_app_data_signature
        .lock()
        .map_err(|_| "Failed to lock cached app data signature".to_string())? =
        Some(disk_signature);
    state
        .cached_app_data_dirty
        .store(false, std::sync::atomic::Ordering::Release);
    let cache_write_ms = cache_write_started
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    runtime_log_debug(format!(
        "[应用数据耗时] 完成 conversations={} elapsed_ms={}",
        conversation_count,
        disk_read_started.elapsed().as_millis()
    ));
    Ok((
        data_for_return,
        CacheReadDetail {
            source: "disk_read".to_string(),
            dirty_fast_path,
            mtime_before_ms,
            cache_lookup_ms,
            disk_read_ms,
            mtime_after_ms,
            cache_write_ms,
            total_ms: total_started
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        },
    ))
}

// ==================== AppData 全量兼容入口（测试专用） ====================
//
// AppData 聚合读写需要长期保留：
// - 启动聚合视图
// - 迁移/兼容逻辑
// - 测试构造
//
// 但 runtime_cache 里的这两个 state helper 已经退化为测试专用：
// - 生产代码禁止再依赖它们
// - 业务热路径必须优先走分片 API
//
// 推荐分片入口：
// - conversation:<id>
// - chat_index
// - runtime_state
// - agents
//
// 如果未来生产代码尝试重新使用它们，会直接在编译期暴露。

#[cfg(test)]
#[allow(dead_code)]
fn state_write_app_data_cached(state: &AppState, data: &AppData) -> Result<(), String> {
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    write_agents_shard(&state.data_path, &data.agents)?;
    if data.data_migration_version > 0 {
        state_db_upsert_kv(
            &state.data_path,
            "data_migration_version",
            &data.data_migration_version.to_string(),
        )?;
    }
    for conv in &data.conversations {
        write_conversation_shard(&state.data_path, conv)?;
    }
    ensure_system_notification_conversation_shard(&state.data_path)?;
    let disk_signature = app_data_cache_signature(&state.data_path);
    let mut cached_data = data.clone();
    sanitize_runtime_cached_app_data(&mut cached_data);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(cached_data);
    *state
        .cached_app_data_signature
        .lock()
        .map_err(|_| "Failed to lock cached app data signature".to_string())? =
        Some(disk_signature);
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_schedule_conversation_persist(
    state: &AppState,
    conversation: &Conversation,
) -> Result<u64, String> {
    let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let pending_conversation = {
        let pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        pending
            .as_ref()
            .and_then(|slot| slot.conversations.get(&conversation.id).cloned())
    };
    let mut conversation_for_cache = conversation.clone();
    let has_field_metadata_authority =
        lock_cached_conversation_field_metadata_ids(state).contains(&conversation.id);
    if has_field_metadata_authority {
        // 字段级 metadata API 是人格、路由、工作区、Todo 等字段的权威写入面。
        // 标记独立于 pending 批次保存，避免 worker take/落盘后旧完整快照再次回滚这些字段。
        // messages 仍来自传入快照；完整快照若要修改 metadata，应改走字段级 API。
        apply_cached_conversation_metadata(state, &mut conversation_for_cache)?;
    }
    if let Some(current) = pending_conversation {
        conversation_for_cache
            .cumulative_usage
            .keep_at_least(&current.cumulative_usage);
    } else if let Ok(current_meta) = state_read_conversation_metadata_cached(state, &conversation.id) {
        conversation_for_cache
            .cumulative_usage
            .keep_at_least(current_meta.cumulative_usage());
    }
    let conversation_disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation.id.clone(), conversation_disk_mtime);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.insert(conversation.id.clone());
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.remove(&conversation.id);
    }
    sync_cached_conversation_metadata(state, &conversation_for_cache)?;
    sync_cached_app_data_conversation(state, &conversation_for_cache)?;
    state_upsert_chat_index_conversation_cached(state, &conversation_for_cache)?;

    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            metadata_conversation_ids: std::collections::HashSet::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
        });
        slot.seq = seq;
        slot.conversations
            .insert(conversation.id.clone(), conversation_for_cache.clone());
        slot.metadata_conversation_ids.remove(&conversation.id);
        slot.deleted_conversation_ids.remove(&conversation.id);
    }
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok(seq)
}

fn state_schedule_conversation_delete(
    state: &AppState,
    conversation_id: &str,
) -> Result<u64, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let mutation_gate = conversation_mutation_gate(&state.data_path, normalized_conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    remove_cached_conversation_metadata(state, normalized_conversation_id)?;
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.remove(normalized_conversation_id);
    }
    {
        let mut dirty_ids = state
            .cached_conversation_dirty_ids
            .lock()
            .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?;
        dirty_ids.remove(normalized_conversation_id);
    }
    {
        let mut deleted_ids = state
            .cached_deleted_conversation_ids
            .lock()
            .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?;
        deleted_ids.insert(normalized_conversation_id.to_string());
    }
    sync_cached_app_data_conversation_deleted(state, normalized_conversation_id)?;
    state_remove_chat_index_conversation_cached(state, normalized_conversation_id)?;

    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            metadata_conversation_ids: std::collections::HashSet::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
        });
        slot.seq = seq;
        slot.conversations.remove(normalized_conversation_id);
        slot.metadata_conversation_ids
            .remove(normalized_conversation_id);
        slot.deleted_conversation_ids
            .insert(normalized_conversation_id.to_string());
    }
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok(seq)
}

/// 退出/重启前同步排空待持久化队列，确保 120ms 去抖窗口内尚未落盘的写入不丢失。
///
/// 设计要点：
/// - 在持有 `app_data_persist_write_lock` 期间串行写盘，与两个后台 worker 互斥；
/// - 复用后台 worker 的“dirty 集合复核”策略，跳过已被直写路径落盘的会话，避免覆盖更新版本；
/// - 同步阻塞执行（退出链路本就 `block_on`），不依赖 tokio 调度，规避运行时已开始关闭的竞态。
///
/// 返回是否实际写出了内容（用于日志），错误不向上传播为致命，调用方记录即可。
fn flush_pending_persists_blocking(state: &AppState) -> Result<bool, String> {
    // 只用 app-data gate 原子地取走队列；本地 chat 的文件 I/O 必须再按会话 gate 协调，
    // 不能因为退出 flush 把不同会话串行化。
    let pending_conversations = {
        let _write_guard = state.app_data_persist_write_lock.lock().map_err(|err| {
            named_lock_error(
                "app_data_persist_write_lock",
                file!(),
                line!(),
                module_path!(),
                &err,
            )
        })?;
        state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?
            .take()
    };
    let mut wrote_anything = false;

    if let Some(pending) = pending_conversations {
        for conversation_id in &pending.deleted_conversation_ids {
            let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
            let _guard = mutation_gate.lock().map_err(|err| {
                named_lock_error(
                    "conversation_mutation_gate",
                    file!(),
                    line!(),
                    module_path!(),
                    &err,
                )
            })?;
            delete_conversation_shard(&state.data_path, conversation_id)?;
            if let Ok(mut deleted_ids) = state.cached_deleted_conversation_ids.lock() {
                deleted_ids.remove(conversation_id);
            }
            wrote_anything = true;
        }
        for (conversation_id, conversation) in pending.conversations.iter() {
            let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
            let _guard = mutation_gate.lock().map_err(|err| {
                named_lock_error(
                    "conversation_mutation_gate",
                    file!(),
                    line!(),
                    module_path!(),
                    &err,
                )
            })?;
            let skip_directly_persisted = {
                let dirty = state.cached_conversation_dirty_ids.lock().map_err(|err| {
                    named_lock_error(
                        "cached_conversation_dirty_ids",
                        file!(),
                        line!(),
                        module_path!(),
                        &err,
                    )
                })?;
                !dirty.contains(conversation_id)
            };
            if !skip_directly_persisted {
                write_conversation_shard(&state.data_path, conversation)?;
                state.cached_conversation_dirty_ids
                    .lock()
                    .map_err(|err| {
                        named_lock_error(
                            "cached_conversation_dirty_ids",
                            file!(),
                            line!(),
                            module_path!(),
                            &err,
                        )
                    })?
                    .remove(conversation_id);
                wrote_anything = true;
            }
        }
        for conversation_id in &pending.metadata_conversation_ids {
            if pending.conversations.contains_key(conversation_id)
                || pending.deleted_conversation_ids.contains(conversation_id)
            {
                continue;
            }
            let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
            let _guard = mutation_gate.lock().map_err(|err| {
                named_lock_error(
                    "conversation_mutation_gate",
                    file!(),
                    line!(),
                    module_path!(),
                    &err,
                )
            })?;
            let skip_directly_persisted = {
                let dirty = state.cached_conversation_dirty_ids.lock().map_err(|err| {
                    named_lock_error(
                        "cached_conversation_dirty_ids",
                        file!(),
                        line!(),
                        module_path!(),
                        &err,
                    )
                })?;
                !dirty.contains(conversation_id)
            };
            if skip_directly_persisted {
                continue;
            }
            let Some(conversation_meta) = ({
                let cached = state
                    .cached_conversation_metadata
                    .lock()
                    .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
                cached.get(conversation_id).cloned()
            }) else {
                continue;
            };
            write_conversation_meta_shard_from_meta(&state.data_path, &conversation_meta)?;
            state.cached_conversation_dirty_ids
                .lock()
                .map_err(|err| {
                    named_lock_error(
                        "cached_conversation_dirty_ids",
                        file!(),
                        line!(),
                        module_path!(),
                        &err,
                    )
                })?
                .remove(conversation_id);
            wrote_anything = true;
        }
    }

    refresh_cached_app_data_dirty(state);
    Ok(wrote_anything)
}

fn start_conversation_persist_worker(state: &AppState) -> Result<(), String> {
    let started = state.conversation_persist_started.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    );
    if started.is_err() {
        return Ok(());
    }
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            state_clone.conversation_persist_notify.notified().await;
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            loop {
                let Some(pending) = ({
                    let mut slot = match state_clone.conversation_persist_pending.lock() {
                        Ok(slot) => slot,
                        Err(_) => {
                            runtime_log_error(
                                "[会话后台持久化] 失败，任务=读取待写入队列，error=lock poisoned"
                                    .to_string(),
                            );
                            break;
                        }
                    };
                    slot.take()
                }) else {
                    break;
                };

                let latest_seq = state_clone
                    .conversation_persist_latest_seq
                    .load(std::sync::atomic::Ordering::Acquire);
                if pending.seq < latest_seq {
                    continue;
                }

                let data_path = state_clone.data_path.clone();
                let dirty_ids_for_write = state_clone.cached_conversation_dirty_ids.clone();
                let cached_conversation_metadata_for_write =
                    state_clone.cached_conversation_metadata.clone();
                let pending_for_write = pending.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    for conversation_id in &pending_for_write.deleted_conversation_ids {
                        let mutation_gate = conversation_mutation_gate(&data_path, conversation_id)?;
                        let _guard = mutation_gate.lock().map_err(|err| {
                            named_lock_error(
                                "conversation_mutation_gate",
                                file!(),
                                line!(),
                                module_path!(),
                                &err,
                            )
                        })?;
                        delete_conversation_shard(&data_path, conversation_id)?;
                    }
                    for (conversation_id, conversation) in pending_for_write.conversations.iter() {
                        let mutation_gate = conversation_mutation_gate(&data_path, conversation_id)?;
                        let _guard = mutation_gate.lock().map_err(|err| {
                            named_lock_error(
                                "conversation_mutation_gate",
                                file!(),
                                line!(),
                                module_path!(),
                                &err,
                            )
                        })?;
                        // 直写路径会在同一会话 gate 内移除 dirty 标记。若这里已被移除，
                        // 说明磁盘已有更新版本，不能再用后台快照覆盖。
                        let skip_directly_persisted = {
                            let dirty = dirty_ids_for_write.lock().map_err(|err| {
                                named_lock_error(
                                    "cached_conversation_dirty_ids",
                                    file!(),
                                    line!(),
                                    module_path!(),
                                    &err,
                                )
                            })?;
                            !dirty.contains(conversation_id)
                        };
                        if !skip_directly_persisted {
                            write_conversation_shard(&data_path, conversation)?;
                        }
                    }
                    for conversation_id in &pending_for_write.metadata_conversation_ids {
                        if pending_for_write.conversations.contains_key(conversation_id)
                            || pending_for_write.deleted_conversation_ids.contains(conversation_id)
                        {
                            continue;
                        }
                        let mutation_gate = conversation_mutation_gate(&data_path, conversation_id)?;
                        let _guard = mutation_gate.lock().map_err(|err| {
                            named_lock_error(
                                "conversation_mutation_gate",
                                file!(),
                                line!(),
                                module_path!(),
                                &err,
                            )
                        })?;
                        let Some(conversation_meta) = ({
                            let cached = cached_conversation_metadata_for_write.lock().map_err(|err| {
                                named_lock_error(
                                    "cached_conversation_metadata",
                                    file!(),
                                    line!(),
                                    module_path!(),
                                    &err,
                                )
                            })?;
                            cached.get(conversation_id).cloned()
                        }) else {
                            continue;
                        };
                        write_conversation_meta_shard_from_meta(&data_path, &conversation_meta)?;
                    }
                    let conversation_mtimes = pending_for_write
                        .conversations
                        .keys()
                        .chain(pending_for_write.metadata_conversation_ids.iter())
                        .map(|conversation_id| {
                            (
                                conversation_id.clone(),
                                conversation_shard_modified_time(&data_path, conversation_id),
                            )
                        })
                        .collect::<Vec<_>>();
                    let deleted_ids = pending_for_write
                        .deleted_conversation_ids
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>();
                    Ok::<(Vec<(String, Option<std::time::SystemTime>)>, Vec<String>), String>((
                        conversation_mtimes,
                        deleted_ids,
                    ))
                })
                .await;

                match write_result {
                    Ok(Ok((conversation_mtimes, deleted_ids))) => {
                        if let Ok(mut cached_mtimes) = state_clone.cached_conversation_mtimes.lock() {
                            for (conversation_id, disk_mtime) in &conversation_mtimes {
                                cached_mtimes.insert(conversation_id.clone(), *disk_mtime);
                            }
                            for conversation_id in &deleted_ids {
                                cached_mtimes.remove(conversation_id);
                            }
                        }
                        let pending_ids = state_clone
                            .conversation_persist_pending
                            .lock()
                            .ok()
                            .and_then(|slot| {
                                slot.as_ref().map(|item| {
                                    item.conversations
                                        .keys()
                                        .chain(item.metadata_conversation_ids.iter())
                                        .cloned()
                                        .collect::<std::collections::HashSet<_>>()
                                })
                            })
                            .unwrap_or_default();
                        if let Ok(mut dirty_ids) = state_clone.cached_conversation_dirty_ids.lock() {
                            for conversation_id in pending.conversations.keys() {
                                if !pending_ids.contains(conversation_id) {
                                    dirty_ids.remove(conversation_id);
                                }
                            }
                            for conversation_id in &pending.metadata_conversation_ids {
                                if !pending_ids.contains(conversation_id) {
                                    dirty_ids.remove(conversation_id);
                                }
                            }
                        }
                        let pending_deleted_ids = state_clone
                            .conversation_persist_pending
                            .lock()
                            .ok()
                            .and_then(|slot| {
                                slot.as_ref().map(|item| {
                                    item.deleted_conversation_ids
                                        .iter()
                                        .cloned()
                                        .collect::<std::collections::HashSet<_>>()
                                })
                            })
                            .unwrap_or_default();
                        if let Ok(mut deleted_conversation_ids) =
                            state_clone.cached_deleted_conversation_ids.lock()
                        {
                            for conversation_id in &deleted_ids {
                                if !pending_deleted_ids.contains(conversation_id) {
                                    deleted_conversation_ids.remove(conversation_id);
                                }
                            }
                        }
                        refresh_cached_app_data_dirty(&state_clone);
                    }
                    Ok(Err(err)) => {
                        runtime_log_error(format!(
                            "[会话后台持久化] 失败，任务=写入会话分片，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                    Err(err) => {
                        runtime_log_error(format!(
                            "[会话后台持久化] 失败，任务=阻塞写入任务，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                }
            }
        }
    });
    Ok(())
}
