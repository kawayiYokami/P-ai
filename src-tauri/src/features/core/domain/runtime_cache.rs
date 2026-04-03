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
    Ok(())
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
    let disk_mtime = path_modified_time(&state.data_path);
    let mtime_before_ms = mtime_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let cache_lookup_started = std::time::Instant::now();
    {
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        let cached_mtime = state
            .cached_app_data_mtime
            .lock()
            .map_err(|_| "Failed to lock cached app data mtime".to_string())?;
        if let (Some(_data), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
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
    let disk_mtime = path_modified_time(&state.data_path);
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
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
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

fn state_write_app_data_cached(state: &AppState, data: &AppData) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    write_app_data(&state.data_path, data)?;
    let disk_mtime = path_modified_time(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
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
                    write_app_data(&data_path, &data_to_write)?;
                    Ok::<Option<std::time::SystemTime>, String>(path_modified_time(&data_path))
                })
                .await;
                match write_result {
                    Ok(Ok(disk_mtime)) => {
                        if let Ok(mut cached) = state_clone.cached_app_data.lock() {
                            *cached = Some(pending.data.clone());
                        }
                        if let Ok(mut cached_mtime) = state_clone.cached_app_data_mtime.lock() {
                            *cached_mtime = disk_mtime;
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
