async fn run_task_store_io<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| format!("task store worker join failed: {err}"))?
}

async fn builtin_task(app_state: &AppState, args: TaskToolArgsWire) -> Result<Value, String> {
    match args.action.trim() {
        "list" => {
            let data_path = app_state.data_path.clone();
            let tasks = run_task_store_io(move || task_store_list_tasks(&data_path)).await?;
            serde_json::to_value(tasks).map_err(|err| format!("Serialize task list failed: {err}"))
        }
        "get" => {
            let task_id = args.task_id.as_deref().map(str::trim).unwrap_or("");
            if task_id.is_empty() {
                return Err("task.taskId is required for action=get".to_string());
            }
            let task_id = task_id.to_string();
            let data_path = app_state.data_path.clone();
            let task = run_task_store_io(move || task_store_get_task(&data_path, &task_id)).await?;
            serde_json::to_value(task).map_err(|err| format!("Serialize task get failed: {err}"))
        }
        "create" => {
            let create_input = TaskCreateInput {
                title: args.title.unwrap_or_default(),
                cause: args.cause.unwrap_or_default(),
                goal: args.goal.unwrap_or_default(),
                flow: args.flow.unwrap_or_default(),
                todos: args.todos.unwrap_or_default(),
                status_summary: args.status_summary.unwrap_or_default(),
                trigger: args
                    .trigger
                    .ok_or_else(|| "task.trigger is required for action=create".to_string())?,
            };
            eprintln!(
                "[任务] 状态=开始 action=create title={} trigger={} todos_count={}",
                create_input.title.trim(),
                serde_json::to_string(&create_input.trigger)
                    .unwrap_or_else(|_| "<invalid trigger>".to_string()),
                create_input.todos.len()
            );
            let data_path = app_state.data_path.clone();
            let create_input_for_io = create_input.clone();
            let task = run_task_store_io(move || {
                task_store_create_task(&data_path, &create_input_for_io)
            })
            .await?;
            serde_json::to_value(task).map_err(|err| format!("Serialize task create failed: {err}"))
        }
        "update" => {
            let task_id = args
                .task_id
                .ok_or_else(|| "task.taskId is required for action=update".to_string())?;
            let update_input = TaskUpdateInput {
                task_id,
                title: args.title,
                cause: args.cause,
                goal: args.goal,
                flow: args.flow,
                todos: args.todos,
                status_summary: args.status_summary,
                stage_key: args.stage_key,
                append_note: args.append_note,
                trigger: args.trigger,
            };
            eprintln!(
                "[任务] 状态=开始 action=update task_id={} changed_fields=title:{},cause:{},goal:{},flow:{},todos:{},status_summary:{},stage_key:{},append_note:{},trigger:{}",
                update_input.task_id,
                update_input.title.is_some(),
                update_input.cause.is_some(),
                update_input.goal.is_some(),
                update_input.flow.is_some(),
                update_input.todos.is_some(),
                update_input.status_summary.is_some(),
                update_input.stage_key.is_some(),
                update_input.append_note.is_some(),
                update_input.trigger.is_some(),
            );
            let data_path = app_state.data_path.clone();
            let update_input_for_io = update_input.clone();
            let task = run_task_store_io(move || {
                task_store_update_task(&data_path, &update_input_for_io)
            })
            .await?;
            serde_json::to_value(task).map_err(|err| format!("Serialize task update failed: {err}"))
        }
        "complete" => {
            let task_id = args
                .task_id
                .ok_or_else(|| "task.taskId is required for action=complete".to_string())?;
            let completion_state = args
                .completion_state
                .ok_or_else(|| "task.completionState is required for action=complete".to_string())?;
            let complete_input = TaskCompleteInput {
                task_id,
                completion_state,
                completion_conclusion: args.completion_conclusion.unwrap_or_default(),
                status_summary: args.status_summary.unwrap_or_default(),
                append_note: args.append_note,
            };
            eprintln!(
                "[任务] 状态=开始 action=complete task_id={} completion_state={}",
                complete_input.task_id,
                complete_input.completion_state
            );
            let data_path = app_state.data_path.clone();
            let complete_input_for_io = complete_input.clone();
            let task = run_task_store_io(move || {
                task_store_complete_task(&data_path, &complete_input_for_io)
            })
            .await?;
            serde_json::to_value(task)
                .map_err(|err| format!("Serialize task complete failed: {err}"))
        }
        _ => Err("task.action must be one of: list, get, create, update, complete".to_string()),
    }
}
