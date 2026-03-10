#[tauri::command]
fn task_list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskEntry>, String> {
    task_scheduler_refresh_current_tracked(&state.data_path)?;
    task_store_list_tasks(&state.data_path)
}

#[tauri::command]
fn task_get_task(input: TaskGetInput, state: State<'_, AppState>) -> Result<TaskEntry, String> {
    task_scheduler_refresh_current_tracked(&state.data_path)?;
    task_store_get_task(&state.data_path, input.task_id.trim())
}

#[tauri::command]
fn task_create_task(input: TaskCreateInput, state: State<'_, AppState>) -> Result<TaskEntry, String> {
    task_store_create_task(&state.data_path, &input)
}

#[tauri::command]
fn task_update_task(input: TaskUpdateInput, state: State<'_, AppState>) -> Result<TaskEntry, String> {
    task_store_update_task(&state.data_path, &input)
}

#[tauri::command]
fn task_complete_task(input: TaskCompleteInput, state: State<'_, AppState>) -> Result<TaskEntry, String> {
    task_store_complete_task(&state.data_path, &input)
}


#[tauri::command]
fn task_list_run_logs(
    input: Option<TaskRunLogListInput>,
    state: State<'_, AppState>,
) -> Result<Vec<TaskRunLogEntry>, String> {
    let payload = input.unwrap_or(TaskRunLogListInput {
        task_id: None,
        limit: Some(50),
    });
    task_store_list_run_logs(
        &state.data_path,
        payload.task_id.as_deref(),
        payload.limit.unwrap_or(50),
    )
}
