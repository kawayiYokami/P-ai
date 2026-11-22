const REMOTE_IM_MAINTENANCE_INTERVAL_HOURS: i64 = 24;

fn remote_im_maintenance_running_keys() -> &'static Mutex<std::collections::HashSet<String>> {
    static KEYS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    KEYS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

fn remote_im_maintenance_key(state: &AppState) -> String {
    state.data_path.to_string_lossy().to_string()
}

fn remote_im_maintenance_last_started_at(data_path: &PathBuf) -> Result<Option<OffsetDateTime>, String> {
    let conn = delegate_store_open(data_path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS remote_im_maintenance_state (state_key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )
    .map_err(|err| format!("初始化远程维护状态失败：{err}"))?;
    let value = conn.query_row(
        "SELECT value FROM remote_im_maintenance_state WHERE state_key = 'last_started_at'",
        [],
        |row| row.get::<_, String>(0),
    ).ok();
    Ok(value.and_then(|raw| OffsetDateTime::parse(raw.trim(), &Rfc3339).ok()))
}

fn remote_im_maintenance_record_started_at(data_path: &PathBuf, started_at: &str) -> Result<(), String> {
    let conn = delegate_store_open(data_path)?;
    conn.execute(
        "INSERT INTO remote_im_maintenance_state (state_key, value) VALUES ('last_started_at', ?1)
         ON CONFLICT(state_key) DO UPDATE SET value=excluded.value",
        params![started_at],
    )
    .map_err(|err| format!("记录远程维护启动时间失败：{err}"))?;
    Ok(())
}

fn remote_im_maintenance_is_expired_at(value: &str, cutoff: OffsetDateTime) -> bool {
    OffsetDateTime::parse(value.trim(), &Rfc3339)
        .map(|at| at < cutoff)
        .unwrap_or(false)
}

fn remote_im_request_24h_maintenance(state: AppState) {
    let key = remote_im_maintenance_key(&state);
    let should_spawn = remote_im_maintenance_running_keys().lock()
        .map(|mut keys| keys.insert(key.clone()))
        .unwrap_or_else(|_| false);
    if !should_spawn {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let worker_state = state.clone();
        let join = tokio::task::spawn_blocking(move || remote_im_run_24h_maintenance(&worker_state)).await;
        match join {
            Ok(Err(err)) => runtime_log_warn(format!("[远程IM] 失败 任务=24小时维护 原因={err}")),
            Err(err) => runtime_log_warn(format!("[远程IM] 失败 任务=24小时维护 原因=任务组执行失败 详情={err}")),
            Ok(Ok(())) => {}
        }
        if let Ok(mut keys) = remote_im_maintenance_running_keys().lock() {
            keys.remove(&key);
        }
    });
}

fn remote_im_request_24h_maintenance_for_conversation(state: AppState, conversation_id: &str) {
    let is_remote_contact = conversation_service_v2()
        .get_conversation_meta(&state, conversation_id.trim())
        .map(|meta| meta.is_remote_im_contact)
        .unwrap_or(false);
    if is_remote_contact {
        remote_im_request_24h_maintenance(state);
    }
}

fn remote_im_run_24h_maintenance(state: &AppState) -> Result<(), String> {
    let now = OffsetDateTime::now_utc();
    if let Some(last_started) = remote_im_maintenance_last_started_at(&state.data_path)? {
        if now - last_started < time::Duration::hours(REMOTE_IM_MAINTENANCE_INTERVAL_HOURS) {
            return Ok(());
        }
    }
    let started_at = now_iso();
    remote_im_maintenance_record_started_at(&state.data_path, &started_at)?;
    let cutoff = now - time::Duration::hours(REMOTE_IM_MAINTENANCE_INTERVAL_HOURS);
    let mut failures = Vec::new();
    let (fast_removed, fast_errors) = conversation_service_v2()
        .prune_expired_remote_im_fast_request_turns(state, cutoff)?;
    failures.extend(fast_errors);

    let remote_roots = state_read_chat_index_cached(state)?
        .conversations.into_iter().filter_map(|item| conversation_service_v2()
            .get_conversation_meta(state, &item.id).ok()
            .filter(|meta| meta.is_remote_im_contact).map(|meta| meta.id.to_string()))
        .collect::<std::collections::HashSet<_>>();
    let snapshots = delegate_snapshot_cache_list(&state.data_path)?;
    let mut delegate_removed = 0usize;
    for snapshot in snapshots {
        if !remote_roots.contains(&snapshot.root_conversation_id)
            || !matches!(snapshot.status.as_str(), DELEGATE_STATUS_COMPLETED | DELEGATE_STATUS_FAILED) {
            continue;
        }
        let terminal_at = snapshot.completed_at.as_deref().unwrap_or(&snapshot.updated_at);
        let expired = remote_im_maintenance_is_expired_at(terminal_at, cutoff);
        if !expired { continue; }
        let is_active = delegate_runtime_thread_list(state)?
            .iter()
            .any(|thread| thread.delegate_id == snapshot.delegate_id);
        if is_active {
            failures.push(format!("delegate_id={}，仍在运行，已跳过", snapshot.delegate_id));
            continue;
        }
        match delegate_store_delete_terminal_delegate(&state.data_path, &snapshot.delegate_id) {
            Ok(true) => match delegate_runtime_thread_conversation_delete(state, &snapshot.delegate_id) {
                Ok(_) => delegate_removed = delegate_removed.saturating_add(1),
                Err(err) => failures.push(format!(
                    "delegate_id={}，数据库记录已删除，清理会话垃圾失败：{}",
                    snapshot.delegate_id, err
                )),
            },
            Ok(false) => failures.push(format!("delegate_id={}，终结状态复核未通过", snapshot.delegate_id)),
            Err(err) => failures.push(format!("delegate_id={}，删除数据库记录失败：{}", snapshot.delegate_id, err)),
        }
    }
    if failures.is_empty() {
        runtime_log_info(format!("[远程IM] 完成 任务=24小时维护 清理杂务={} 清理委托={}", fast_removed, delegate_removed));
    } else {
        runtime_log_warn(format!("[远程IM] 完成 任务=24小时维护 清理杂务={} 清理委托={} 失败={} 详情={}", fast_removed, delegate_removed, failures.len(), failures.join(" | ")));
    }
    Ok(())
}

#[cfg(test)]
mod remote_im_maintenance_tests {
    use super::*;

    #[test]
    fn maintenance_expiry_should_be_strictly_older_than_24_hours() {
        let cutoff = OffsetDateTime::parse("2026-07-19T12:00:00Z", &Rfc3339)
            .expect("parse cutoff");
        assert!(remote_im_maintenance_is_expired_at("2026-07-19T11:59:59Z", cutoff));
        assert!(!remote_im_maintenance_is_expired_at("2026-07-19T12:00:00Z", cutoff));
        assert!(!remote_im_maintenance_is_expired_at("invalid", cutoff));
    }
}
