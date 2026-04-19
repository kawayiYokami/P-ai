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

fn state_read_conversation_cached(
    state: &AppState,
    conversation_id: &str,
) -> Result<Conversation, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let conversation_path = app_layout_chat_conversation_path(&state.data_path, conversation_id);
    let disk_mtime = path_modified_time(&conversation_path);
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
    Ok(conversation)
}

fn state_read_chat_index_cached(state: &AppState) -> Result<ChatIndexFile, String> {
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
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
    Ok(())
}

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
    let disk_mtime = path_modified_time(&app_layout_chat_conversation_path(
        &state.data_path,
        &conversation.id,
    ));
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
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
    Ok(())
}

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
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
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
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
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
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
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

fn state_read_app_data_cached_with_detail(
    state: &AppState,
) -> Result<(AppData, CacheReadDetail), String> {
    let (data, detail) = ensure_app_data_cache_ready_inner(state, true)?;
    let data = data.ok_or_else(|| "Cached app data is unexpectedly missing".to_string())?;
    Ok((data, detail))
}

fn state_read_app_data_cached(state: &AppState) -> Result<AppData, String> {
    state_read_app_data_cached_with_detail(state).map(|(data, _detail)| data)
}

fn ensure_app_data_cache_ready_with_detail(state: &AppState) -> Result<CacheReadDetail, String> {
    let (_data, detail) = ensure_app_data_cache_ready_inner(state, false)?;
    Ok(detail)
}

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

fn with_app_data_cached_ref<T>(
    state: &AppState,
    f: impl FnOnce(&AppData, &CacheReadDetail) -> Result<T, String>,
) -> Result<T, String> {
    let detail = ensure_app_data_cache_ready_with_detail(state)?;
    let cached = state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())?;
    let data = cached
        .as_ref()
        .ok_or_else(|| "Cached app data is unexpectedly missing".to_string())?;
    f(data, &detail)
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
    state.app_data_persist_notify.notify_one();
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
                            state_clone.cached_app_data_dirty.store(
                                false,
                                std::sync::atomic::Ordering::Release,
                            );
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
