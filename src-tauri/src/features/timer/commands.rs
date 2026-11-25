// ==================== 定时器命令层 ====================

#[tauri::command]
fn timer_list_timers(state: State<'_, AppState>) -> Result<Vec<TimerRecord>, String> {
    timer_list_timers_inner(state.inner())
}

fn timer_list_timers_inner(state: &AppState) -> Result<Vec<TimerRecord>, String> {
    let timers = timer_store_list(&state.data_path)?;
    Ok(timer_store_filter_unknown_conversations(&state.data_path, timers))
}

#[tauri::command]
fn timer_create_timer(
    input: TimerCreateInput,
    state: State<'_, AppState>,
) -> Result<TimerRecord, String> {
    timer_create_timer_inner(input, state.inner())
}

fn timer_create_timer_inner(
    input: TimerCreateInput,
    state: &AppState,
) -> Result<TimerRecord, String> {
    let timer = timer_store_create(state, &input)?;
    timer_scheduler_notify_changed(state);
    timer_publish_changed_event(state, "created", &timer.id, Some(&timer));
    Ok(timer)
}

#[tauri::command]
fn timer_update_timer(
    input: TimerUpdateInput,
    state: State<'_, AppState>,
) -> Result<TimerRecord, String> {
    timer_update_timer_inner(input, state.inner())
}

fn timer_update_timer_inner(
    input: TimerUpdateInput,
    state: &AppState,
) -> Result<TimerRecord, String> {
    let timer = timer_store_update(state, &input)?;
    timer_scheduler_notify_changed(state);
    timer_publish_changed_event(state, "updated", &timer.id, Some(&timer));
    Ok(timer)
}

#[tauri::command]
fn timer_delete_timer(
    input: TimerDeleteInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    timer_delete_timer_inner(input, state.inner())
}

fn timer_delete_timer_inner(input: TimerDeleteInput, state: &AppState) -> Result<(), String> {
    timer_store_delete(&state.data_path, &input.timer_id)?;
    timer_scheduler_notify_changed(state);
    timer_publish_changed_event(state, "deleted", input.timer_id.trim(), None);
    Ok(())
}
