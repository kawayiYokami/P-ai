async fn run_task_store_io<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| format!("task store worker join failed: {err}"))?
}

fn task_tool_target_scope_from_conversation(
    app_state: &AppState,
    conversation_id: Option<&str>,
) -> Option<String> {
    let conversation_id = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if let Ok(conversation_meta) = conversation_service_v2().get_conversation_meta(app_state, conversation_id)
    {
        return Some(if conversation_meta.conversation_kind.trim()
            == CONVERSATION_KIND_REMOTE_IM_CONTACT
        {
            TASK_TARGET_SCOPE_CONTACT.to_string()
        } else {
            TASK_TARGET_SCOPE_DESKTOP.to_string()
        });
    }
    match state_service_list_remote_im_contacts(app_state, None) {
        Ok(contacts) => {
            if contacts.iter().any(|contact| {
                contact.bound_conversation_id.as_deref().map(str::trim) == Some(conversation_id)
            }) {
                Some(TASK_TARGET_SCOPE_CONTACT.to_string())
            } else {
                Some(TASK_TARGET_SCOPE_DESKTOP.to_string())
            }
        }
        Err(err) => {
            runtime_log_error(format!(
                "[任务] target_scope解析失败: conversation_id={}, error={}",
                conversation_id, err
            ));
            None
        }
    }
}

fn task_tool_goal_from_args(args: &TaskToolArgsWire) -> Option<String> {
    args.goal
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            args.title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn task_tool_why_from_args(args: &TaskToolArgsWire) -> Option<String> {
    args.why
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let mut parts = Vec::<String>::new();
            if let Some(cause) = args
                .cause
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parts.push(cause.to_string());
            }
            if let Some(title) = args
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let goal = task_tool_goal_from_args(args).unwrap_or_default();
                if title != goal {
                    parts.push(format!("原标题：{}", title));
                }
            }
            if let Some(flow) = args
                .flow
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parts.push(format!("原流程：{}", flow));
            }
            if let Some(stage_key) = args
                .stage_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parts.push(format!("原阶段：{}", stage_key));
            }
            if let Some(append_note) = args
                .append_note
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parts.push(format!("原补充：{}", append_note));
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        })
}

fn task_tool_todo_from_args(args: &TaskToolArgsWire) -> Option<String> {
    args.todo
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            args.how
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            let status_summary = args
                .status_summary
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let todo_list = args
                .todos
                .as_ref()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.trim().to_string())
                        .filter(|item| !item.is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if status_summary.is_none() && todo_list.is_empty() {
                return None;
            }
            let mut parts = Vec::<String>::new();
            if let Some(status_summary) = status_summary {
                parts.push(status_summary);
            }
            if !todo_list.is_empty() {
                let joined = todo_list.join("；");
                if parts.is_empty() {
                    parts.push(joined);
                } else {
                    let prefixed = format!("待办：{}", joined);
                    if !parts.iter().any(|part| part == &joined || part == &prefixed) {
                        parts.push(prefixed);
                    }
                }
            }
            Some(parts.join("\n"))
        })
}

async fn builtin_task(
    app_state: &AppState,
    session_id: &str,
    model_config_id: &str,
    executor_agent_id: &str,
    args: TaskToolArgsWire,
) -> Result<Value, String> {
    let bound_conversation_id = delegate_session_conversation_id(session_id);
    if let Some(conversation_id) = bound_conversation_id.as_deref() {
        let delegate_conversation = conversation_service_v2()
            .get_conversation_meta(app_state, conversation_id)
            .ok()
            .map(|conversation_meta| {
                conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE
            })
            .or_else(|| {
                delegate_runtime_thread_conversation_get(app_state, conversation_id)
                    .ok()
                    .flatten()
                    .map(|conversation| conversation_is_delegate(&conversation))
            })
            .unwrap_or(false);
        if delegate_conversation {
            return Err("委托线程中禁止使用 task 工具创建或管理任务，避免递归调度。".to_string());
        }
    }
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
            let mut runtime_context = runtime_context_new("task_tool", "task_create");
            runtime_context.request_id = Some(format!("task-create-{}", Uuid::new_v4()));
            runtime_context.origin_conversation_id = bound_conversation_id.clone();
            runtime_context.root_conversation_id = bound_conversation_id.clone();
            runtime_context.executor_agent_id = runtime_context_trimmed(Some(executor_agent_id));
            runtime_context.model_config_id = runtime_context_trimmed(Some(model_config_id));
            let target_scope = {
                let app_state = app_state.clone();
                let bound_conversation_id = bound_conversation_id.clone();
                run_task_store_io(move || {
                    Ok(task_tool_target_scope_from_conversation(
                        &app_state,
                        bound_conversation_id.as_deref(),
                    ))
                })
                .await?
            };
            let create_input = TaskCreateInput {
                goal: task_tool_goal_from_args(&args).unwrap_or_default(),
                conversation_id: bound_conversation_id,
                agent_id: runtime_context_trimmed(Some(executor_agent_id)),
                target_scope,
                why: task_tool_why_from_args(&args).unwrap_or_default(),
                todo: task_tool_todo_from_args(&args).unwrap_or_default(),
                trigger: args
                    .trigger
                    .ok_or_else(|| "task.trigger is required for action=create".to_string())?,
            };
            runtime_log_info(format!(
                "[任务] 状态=开始 action=create request_id={} goal={} origin_conversation_id={} trigger={} todo_present={}",
                runtime_context.request_id.as_deref().unwrap_or(""),
                create_input.goal.trim(),
                create_input.conversation_id.as_deref().unwrap_or(""),
                serde_json::to_string(&create_input.trigger)
                    .unwrap_or_else(|_| "<invalid trigger>".to_string()),
                !create_input.todo.trim().is_empty()
            ));
            let data_path = app_state.data_path.clone();
            let create_input_for_io = task_create_input_for_write(app_state, &create_input)?;
            let task = run_task_store_io(move || {
                task_store_create_task(&data_path, &create_input_for_io)
            })
            .await?;
            task_scheduler_notify_changed(app_state);
            serde_json::to_value(task).map_err(|err| format!("Serialize task create failed: {err}"))
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
            };
            runtime_log_info(format!(
                "[任务] 状态=开始 action=complete task_id={} completion_state={}",
                complete_input.task_id,
                complete_input.completion_state
            ));
            let data_path = app_state.data_path.clone();
            let complete_input_for_io = complete_input.clone();
            let task = run_task_store_io(move || {
                task_store_complete_task(&data_path, &complete_input_for_io)
            })
            .await?;
            task_scheduler_notify_changed(app_state);
            serde_json::to_value(task)
                .map_err(|err| format!("Serialize task complete failed: {err}"))
        }
        _ => Err("task.action must be one of: list, get, create, complete".to_string()),
    }
}
