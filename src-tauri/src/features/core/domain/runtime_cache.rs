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

fn conversation_index_repair_gate(
    state: &AppState,
    conversation_id: &str,
) -> Result<Arc<Mutex<()>>, String> {
    let mut gates = state
        .conversation_index_repair_gates
        .lock()
        .map_err(|err| format!("Failed to lock conversation index repair gates: {err}"))?;
    Ok(gates
        .entry(conversation_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

fn format_chat_index_item_log(item: Option<&ChatIndexConversationItem>) -> String {
    let Some(item) = item else {
        return "missing".to_string();
    };
    format!(
        "id={} updated_at={} status={} summary_len={} archived_at={}",
        item.id,
        item.updated_at,
        item.status,
        item.summary.chars().count(),
        item.archived_at.as_deref().unwrap_or("")
    )
}

fn chat_index_item_mismatch_fields(
    existing: &ChatIndexConversationItem,
    expected: &ChatIndexConversationItem,
) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if existing.updated_at != expected.updated_at {
        fields.push("updated_at");
    }
    if existing.status != expected.status {
        fields.push("status");
    }
    if existing.summary != expected.summary {
        fields.push("summary");
    }
    if existing.archived_at != expected.archived_at {
        fields.push("archived_at");
    }
    fields
}

fn ensure_conversation_chat_index_item_consistent(
    state: &AppState,
    conversation: &Conversation,
    trigger_reason: &str,
) -> Result<(), String> {
    let expected = build_chat_index_item(conversation);
    let chat_index = state_read_chat_index_cached(state)?;
    let initial_existing = chat_index
        .conversations
        .iter()
        .find(|item| item.id == conversation.id);
    if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
        if initial_existing.is_none() {
            return Ok(());
        }
        let before_log = format_chat_index_item_log(initial_existing);
        let started = std::time::Instant::now();
        eprintln!(
            "[会话索引自愈] 状态=开始，conversation_id={}，触发原因={}，修复原因=chat_index_stale_hidden_item，修复前={}",
            conversation.id, trigger_reason, before_log
        );

        let repair_gate = conversation_index_repair_gate(state, &conversation.id)?;
        let _repair_guard = repair_gate
            .lock()
            .map_err(|err| format!("Failed to lock conversation index repair gate: {err}"))?;

        let mut chat_index = state_read_chat_index_cached(state)?;
        let existing_after_lock = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .cloned();
        if existing_after_lock.is_none() {
            eprintln!(
                "[会话索引自愈] 状态=跳过，conversation_id={}，触发原因={}，修复原因=chat_index_stale_hidden_item，修复前={}，修复后={}，duration_ms={}",
                conversation.id,
                trigger_reason,
                before_log,
                format_chat_index_item_log(None),
                started.elapsed().as_millis()
            );
            return Ok(());
        }

        remove_chat_index_conversation(&mut chat_index, &conversation.id);
        state_write_chat_index_cached(state, &chat_index)?;
        eprintln!(
            "[会话索引自愈] 状态=完成，conversation_id={}，触发原因={}，修复原因=chat_index_stale_hidden_item，修复前={}，修复后={}，duration_ms={}",
            conversation.id,
            trigger_reason,
            before_log,
            format_chat_index_item_log(None),
            started.elapsed().as_millis()
        );
        return Ok(());
    }
    let initial_mismatch_fields = initial_existing
        .map(|existing| chat_index_item_mismatch_fields(existing, &expected))
        .unwrap_or_else(Vec::new);
    if initial_existing.is_some() && initial_mismatch_fields.is_empty() {
        return Ok(());
    }

    let repair_reason = if initial_existing.is_none() {
        "chat_index_missing_item".to_string()
    } else {
        format!(
            "chat_index_field_mismatch:{}",
            initial_mismatch_fields.join("|")
        )
    };
    let before_log = format_chat_index_item_log(initial_existing);
    let started = std::time::Instant::now();
    eprintln!(
        "[会话索引自愈] 状态=开始，conversation_id={}，触发原因={}，修复原因={}，修复前={}",
        conversation.id, trigger_reason, repair_reason, before_log
    );

    let repair_gate = conversation_index_repair_gate(state, &conversation.id)?;
    let _repair_guard = repair_gate
        .lock()
        .map_err(|err| format!("Failed to lock conversation index repair gate: {err}"))?;

    let mut chat_index = state_read_chat_index_cached(state)?;
    let existing_after_lock = chat_index
        .conversations
        .iter()
        .find(|item| item.id == conversation.id)
        .cloned();
    if existing_after_lock
        .as_ref()
        .map(|item| chat_index_item_mismatch_fields(item, &expected).is_empty())
        .unwrap_or(false)
    {
        eprintln!(
            "[会话索引自愈] 状态=跳过，conversation_id={}，触发原因={}，修复原因={}，修复前={}，修复后={}，duration_ms={}",
            conversation.id,
            trigger_reason,
            repair_reason,
            before_log,
            format_chat_index_item_log(existing_after_lock.as_ref()),
            started.elapsed().as_millis()
        );
        return Ok(());
    }

    upsert_chat_index_conversation(&mut chat_index, conversation);
    if let Err(err) = state_write_chat_index_cached(state, &chat_index) {
        eprintln!(
            "[会话索引自愈] 状态=失败，conversation_id={}，触发原因={}，修复原因={}，修复前={}，修复后={}，duration_ms={}，error={}",
            conversation.id,
            trigger_reason,
            repair_reason,
            before_log,
            format_chat_index_item_log(Some(&expected)),
            started.elapsed().as_millis(),
            err
        );
        return Err(format!(
            "修复会话索引项失败，conversation_id={}，trigger={}，reason={}，error={}",
            conversation.id, trigger_reason, repair_reason, err
        ));
    }

    eprintln!(
        "[会话索引自愈] 状态=完成，conversation_id={}，触发原因={}，修复原因={}，修复前={}，修复后={}，duration_ms={}",
        conversation.id,
        trigger_reason,
        repair_reason,
        before_log,
        format_chat_index_item_log(Some(&expected)),
        started.elapsed().as_millis()
    );
    Ok(())
}

fn sync_cached_app_data_agents(state: &AppState, agents: &[AgentProfile]) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        data.agents = agents.to_vec();
    }
    sync_cached_app_data_signature(state)
}

fn sync_cached_app_data_runtime(
    state: &AppState,
    runtime: &RuntimeStateFile,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        apply_runtime_state_to_app_data(data, runtime);
    }
    sync_cached_app_data_signature(state)
}

fn sync_cached_app_data_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        if let Some(existing) = data
            .conversations
            .iter_mut()
            .find(|item| item.id == conversation.id)
        {
            *existing = conversation.clone();
        } else {
            data.conversations.push(conversation.clone());
        }
    }
    sync_cached_app_data_signature(state)
}

fn sync_cached_app_data_conversation_deleted(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let mut cached = state
        .cached_app_data
        .lock()
        .map_err(|err| format!("Failed to lock cached app data: {err}"))?;
    if let Some(data) = cached.as_mut() {
        data.conversations
            .retain(|conversation| conversation.id != conversation_id);
    }
    sync_cached_app_data_signature(state)
}

fn state_mark_conversation_direct_persisted(
    state: &AppState,
    conversation: &Conversation,
    include_chat_index: bool,
) -> Result<(), String> {
    let disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.insert(conversation.id.clone(), conversation.clone());
    }
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
            slot.deleted_conversation_ids.remove(&conversation.id);
            slot.conversations.is_empty()
                && slot.deleted_conversation_ids.is_empty()
                && slot.chat_index.is_none()
        } else {
            false
        };
        if should_clear_slot {
            *pending = None;
        }
    }
    sync_cached_app_data_conversation(state, conversation)?;
    if include_chat_index {
        let mut chat_index = state_read_chat_index_cached(state)?;
        upsert_chat_index_conversation(&mut chat_index, conversation);
        let _ = write_chat_index_shard(&state.data_path, &chat_index)?;
        let chat_index_mtime = path_modified_time(&app_layout_chat_index_path(&state.data_path));
        *state
            .cached_chat_index
            .lock()
            .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(chat_index);
        *state
            .cached_chat_index_mtime
            .lock()
            .map_err(|_| "Failed to lock cached chat index mtime".to_string())? = chat_index_mtime;
        state
            .cached_chat_index_dirty
            .store(false, std::sync::atomic::Ordering::Release);
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn has_pending_app_data_persist(state: &AppState) -> bool {
    state
        .app_data_persist_pending
        .lock()
        .map(|pending| pending.is_some())
        .unwrap_or(true)
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
    let chat_index_dirty = state
        .cached_chat_index_dirty
        .load(std::sync::atomic::Ordering::Acquire);
    has_pending_slot || has_dirty_conversations || has_deleted_conversations || chat_index_dirty
}

fn refresh_cached_app_data_dirty(state: &AppState) {
    let dirty = has_pending_app_data_persist(state) || has_pending_conversation_persist(state);
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
        let cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        if let Some(conversation) = cached.get(conversation_id) {
            return Ok(conversation.clone());
        }
    }
    let disk_mtime = conversation_shard_modified_time(&state.data_path, conversation_id);
    {
        let cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        let cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        if let (Some(conversation), Some(cached_mtime), Some(disk_time)) = (
            cached.get(conversation_id),
            cached_mtimes.get(conversation_id),
            disk_mtime,
        )
        {
            if *cached_mtime == Some(disk_time) {
                ensure_conversation_chat_index_item_consistent(
                    state,
                    conversation,
                    "state_read_conversation_cached.cache_hit",
                )?;
                return Ok(conversation.clone());
            }
        }
    }
    let conversation = read_conversation_shard(&state.data_path, conversation_id)?;
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.insert(conversation_id.to_string(), conversation.clone());
    }
    {
        let mut cached_mtimes = state
            .cached_conversation_mtimes
            .lock()
            .map_err(|_| "Failed to lock cached conversation mtimes".to_string())?;
        cached_mtimes.insert(conversation_id.to_string(), disk_mtime);
    }
    ensure_conversation_chat_index_item_consistent(
        state,
        &conversation,
        "state_read_conversation_cached.disk_read",
    )?;
    Ok(conversation)
}

fn state_read_chat_index_cached(state: &AppState) -> Result<ChatIndexFile, String> {
    if state
        .cached_chat_index_dirty
        .load(std::sync::atomic::Ordering::Acquire)
    {
        let cached = state
            .cached_chat_index
            .lock()
            .map_err(|_| "Failed to lock cached chat index".to_string())?;
        if let Some(index) = cached.as_ref() {
            return Ok(index.clone());
        }
    }
    let disk_mtime = path_modified_time(&app_layout_chat_index_path(&state.data_path));
    {
        let cached = state
            .cached_chat_index
            .lock()
            .map_err(|_| "Failed to lock cached chat index".to_string())?;
        let cached_mtime = state
            .cached_chat_index_mtime
            .lock()
            .map_err(|_| "Failed to lock cached chat index mtime".to_string())?;
        if let (Some(index), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(index.clone());
            }
        }
    }
    let index = read_chat_index_shard(&state.data_path)?;
    *state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(index.clone());
    *state
        .cached_chat_index_mtime
        .lock()
        .map_err(|_| "Failed to lock cached chat index mtime".to_string())? = disk_mtime;
    Ok(index)
}

fn state_write_chat_index_cached(state: &AppState, index: &ChatIndexFile) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    let _ = write_chat_index_shard(&state.data_path, index)?;
    let disk_mtime = path_modified_time(&app_layout_chat_index_path(&state.data_path));
    *state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())? = Some(index.clone());
    *state
        .cached_chat_index_mtime
        .lock()
        .map_err(|_| "Failed to lock cached chat index mtime".to_string())? = disk_mtime;
    sync_cached_app_data_signature(state)?;
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

#[allow(dead_code)]
fn state_write_conversation_with_chat_index_cached(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    state_write_conversation_cached(state, conversation)?;
    let mut chat_index = state_read_chat_index_cached(state)?;
    upsert_chat_index_conversation(&mut chat_index, conversation);
    state_write_chat_index_cached(state, &chat_index)?;
    Ok(())
}

#[allow(dead_code)]
fn state_write_conversation_cached(
    state: &AppState,
    conversation: &Conversation,
) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    let _ = write_conversation_shard(&state.data_path, conversation)?;
    let disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.insert(conversation.id.clone(), conversation.clone());
    }
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
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

#[allow(dead_code)]
fn state_delete_conversation_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    let _ = delete_conversation_shard(&state.data_path, conversation_id)?;
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.remove(conversation_id);
    }
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
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
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
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
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
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_read_runtime_state_cached(state: &AppState) -> Result<RuntimeStateFile, String> {
    let disk_mtime = path_modified_time(&app_layout_runtime_state_path(&state.data_path));
    {
        let cached = state
            .cached_runtime_state
            .lock()
            .map_err(|_| "Failed to lock cached runtime state".to_string())?;
        let cached_mtime = state
            .cached_runtime_state_mtime
            .lock()
            .map_err(|_| "Failed to lock cached runtime state mtime".to_string())?;
        if let (Some(runtime), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(runtime.clone());
            }
        }
    }
    let runtime = read_runtime_state_shard(&state.data_path)?;
    *state
        .cached_runtime_state
        .lock()
        .map_err(|_| "Failed to lock cached runtime state".to_string())? = Some(runtime.clone());
    *state
        .cached_runtime_state_mtime
        .lock()
        .map_err(|_| "Failed to lock cached runtime state mtime".to_string())? = disk_mtime;
    Ok(runtime)
}

fn state_write_runtime_state_cached(
    state: &AppState,
    runtime: &RuntimeStateFile,
) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    let _ = write_runtime_state_shard(&state.data_path, runtime)?;
    let disk_mtime = path_modified_time(&app_layout_runtime_state_path(&state.data_path));
    *state
        .cached_runtime_state
        .lock()
        .map_err(|_| "Failed to lock cached runtime state".to_string())? = Some(runtime.clone());
    *state
        .cached_runtime_state_mtime
        .lock()
        .map_err(|_| "Failed to lock cached runtime state mtime".to_string())? = disk_mtime;
    sync_cached_app_data_runtime(state, runtime)?;
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    refresh_cached_app_data_dirty(state);
    Ok(())
}

fn state_read_agents_runtime_snapshot(state: &AppState) -> Result<AppData, String> {
    let agents = state_read_agents_cached(state)?;
    let runtime = state_read_runtime_state_cached(state)?;
    let mut data = AppData::default();
    data.agents = agents;
    apply_runtime_state_to_app_data(&mut data, &runtime);
    Ok(data)
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
    let data = read_app_data(&state.data_path)?;
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
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    #[allow(deprecated)]
    write_app_data(&state.data_path, data)?;
    let disk_signature = app_data_cache_signature(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_signature
        .lock()
        .map_err(|_| "Failed to lock cached app data signature".to_string())? =
        Some(disk_signature);
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
fn state_schedule_app_data_persist(state: &AppState, data: &AppData) -> Result<u64, String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    state
        .cached_app_data_dirty
        .store(true, std::sync::atomic::Ordering::Release);
    {
        let mut pending = state
            .app_data_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending app data persist".to_string())?;
        *pending = Some(PendingAppDataPersist {
            seq,
            data: data.clone(),
        });
    }
    refresh_cached_app_data_dirty(state);
    state.app_data_persist_notify.notify_one();
    Ok(seq)
}

fn state_schedule_conversation_persist(
    state: &AppState,
    conversation: &Conversation,
    include_chat_index: bool,
) -> Result<u64, String> {
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let conversation_disk_mtime = conversation_shard_modified_time(&state.data_path, &conversation.id);
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.insert(conversation.id.clone(), conversation.clone());
    }
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
    sync_cached_app_data_conversation(state, conversation)?;

    let mut pending_chat_index = None;
    if include_chat_index {
        let mut chat_index = state_read_chat_index_cached(state)?;
        upsert_chat_index_conversation(&mut chat_index, conversation);
        let disk_mtime = path_modified_time(&app_layout_chat_index_path(&state.data_path));
        *state
            .cached_chat_index
            .lock()
            .map_err(|_| "Failed to lock cached chat index".to_string())? =
            Some(chat_index.clone());
        *state
            .cached_chat_index_mtime
            .lock()
            .map_err(|_| "Failed to lock cached chat index mtime".to_string())? = disk_mtime;
        state
            .cached_chat_index_dirty
            .store(true, std::sync::atomic::Ordering::Release);
        pending_chat_index = Some(chat_index);
    }

    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
            chat_index: None,
        });
        slot.seq = seq;
        slot.conversations
            .insert(conversation.id.clone(), conversation.clone());
        slot.deleted_conversation_ids.remove(&conversation.id);
        if pending_chat_index.is_some() {
            slot.chat_index = pending_chat_index;
        }
    }
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok(seq)
}

fn state_schedule_conversation_delete(
    state: &AppState,
    conversation_id: &str,
    include_chat_index: bool,
) -> Result<u64, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let seq = state
        .conversation_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    {
        let mut cached = state
            .cached_conversations
            .lock()
            .map_err(|_| "Failed to lock cached conversations".to_string())?;
        cached.remove(normalized_conversation_id);
    }
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

    let mut pending_chat_index = None;
    if include_chat_index {
        let chat_index_snapshot = {
            let cached = state
                .cached_app_data
                .lock()
                .map_err(|_| "Failed to lock cached app data".to_string())?;
            cached
                .as_ref()
                .map(|data| build_chat_index_file(&data.conversations))
        };
        if let Some(chat_index) = chat_index_snapshot {
            let disk_mtime = path_modified_time(&app_layout_chat_index_path(&state.data_path));
            *state
                .cached_chat_index
                .lock()
                .map_err(|_| "Failed to lock cached chat index".to_string())? =
                Some(chat_index.clone());
            *state
                .cached_chat_index_mtime
                .lock()
                .map_err(|_| "Failed to lock cached chat index mtime".to_string())? = disk_mtime;
            state
                .cached_chat_index_dirty
                .store(true, std::sync::atomic::Ordering::Release);
            pending_chat_index = Some(chat_index);
        }
    }

    {
        let mut pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending conversation persist".to_string())?;
        let slot = pending.get_or_insert_with(|| PendingConversationPersist {
            seq,
            conversations: std::collections::HashMap::new(),
            deleted_conversation_ids: std::collections::HashSet::new(),
            chat_index: None,
        });
        slot.seq = seq;
        slot.conversations.remove(normalized_conversation_id);
        slot.deleted_conversation_ids
            .insert(normalized_conversation_id.to_string());
        if pending_chat_index.is_some() {
            slot.chat_index = pending_chat_index;
        }
    }
    refresh_cached_app_data_dirty(state);
    state.conversation_persist_notify.notify_one();
    Ok(seq)
}

fn start_app_data_persist_worker(state: &AppState) -> Result<(), String> {
    let started = state.app_data_persist_started.compare_exchange(
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
            state_clone.app_data_persist_notify.notified().await;
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            loop {
                let Some(pending) = ({
                    let mut slot = match state_clone.app_data_persist_pending.lock() {
                        Ok(slot) => slot,
                        Err(_) => {
                            runtime_log_error(
                                "[后台持久化] 失败，任务=读取待写入队列，error=lock poisoned"
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
                    .app_data_persist_latest_seq
                    .load(std::sync::atomic::Ordering::Acquire);
                if pending.seq < latest_seq {
                    continue;
                }
                let data_path = state_clone.data_path.clone();
                let data_to_write = pending.data.clone();
                let write_lock = state_clone.app_data_persist_write_lock.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    let _write_guard = write_lock.lock().map_err(|err| {
                        named_lock_error(
                            "app_data_persist_write_lock",
                            file!(),
                            line!(),
                            module_path!(),
                            &err,
                        )
                    })?;
                    #[allow(deprecated)]
                    write_app_data(&data_path, &data_to_write)?;
                    Ok::<(), String>(())
                })
                .await;
                match write_result {
                    Ok(Ok(())) => {
                        if let Ok(mut cached) = state_clone.cached_app_data.lock() {
                            *cached = Some(pending.data.clone());
                        }
                        if let Ok(mut cached_signature) =
                            state_clone.cached_app_data_signature.lock()
                        {
                            *cached_signature =
                                Some(app_data_cache_signature(&state_clone.data_path));
                        }
                        let still_latest = state_clone
                            .app_data_persist_latest_seq
                            .load(std::sync::atomic::Ordering::Acquire)
                            == pending.seq;
                        if still_latest {
                            refresh_cached_app_data_dirty(&state_clone);
                        }
                    }
                    Ok(Err(err)) => {
                        runtime_log_error(format!(
                            "[后台持久化] 失败，任务=写入应用数据，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                    Err(err) => {
                        runtime_log_error(format!(
                            "[后台持久化] 失败，任务=阻塞写入任务，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                }
            }
        }
    });
    Ok(())
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
                let write_lock = state_clone.app_data_persist_write_lock.clone();
                let pending_for_write = pending.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    let _write_guard = write_lock.lock().map_err(|err| {
                        named_lock_error(
                            "app_data_persist_write_lock",
                            file!(),
                            line!(),
                            module_path!(),
                            &err,
                        )
                    })?;
                    for conversation_id in &pending_for_write.deleted_conversation_ids {
                        delete_conversation_shard(&data_path, conversation_id)?;
                    }
                    for conversation in pending_for_write.conversations.values() {
                        write_conversation_shard(&data_path, conversation)?;
                    }
                    let conversation_mtimes = pending_for_write
                        .conversations
                        .keys()
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
                    let chat_index_mtime = if let Some(chat_index) = pending_for_write.chat_index.as_ref() {
                        write_chat_index_shard(&data_path, chat_index)?;
                        path_modified_time(&app_layout_chat_index_path(&data_path))
                    } else {
                        None
                    };
                    Ok::<(Vec<(String, Option<std::time::SystemTime>)>, Vec<String>, Option<std::time::SystemTime>), String>((
                        conversation_mtimes,
                        deleted_ids,
                        chat_index_mtime,
                    ))
                })
                .await;

                match write_result {
                    Ok(Ok((conversation_mtimes, deleted_ids, chat_index_mtime))) => {
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
                        if pending.chat_index.is_some() {
                            if let Some(disk_mtime) = chat_index_mtime {
                                if let Ok(mut cached_chat_index_mtime) =
                                    state_clone.cached_chat_index_mtime.lock()
                                {
                                    *cached_chat_index_mtime = Some(disk_mtime);
                                }
                            }
                            let chat_index_still_pending = state_clone
                                .conversation_persist_pending
                                .lock()
                                .ok()
                                .and_then(|slot| slot.as_ref().map(|item| item.chat_index.is_some()))
                                .unwrap_or(false);
                            if !chat_index_still_pending {
                                state_clone.cached_chat_index_dirty.store(
                                    false,
                                    std::sync::atomic::Ordering::Release,
                                );
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
