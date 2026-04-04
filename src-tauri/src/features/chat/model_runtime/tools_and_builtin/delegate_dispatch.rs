fn delegate_resolve_context(
    app_state: &AppState,
    source_agent_id: &str,
    source_conversation_id: Option<&str>,
    target_department_id: &str,
) -> Result<
    (
        AppConfig,
        AppData,
        DepartmentConfig,
        DepartmentConfig,
        String,
        String,
        Option<DelegateRuntimeThread>,
    ),
    String,
> {
    let guard = app_state
        .conversation_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut config = read_config(&app_state.config_path)?;
    let mut data = read_app_data(&app_state.data_path)?;
    merge_private_organization_into_runtime_data(&app_state.data_path, &mut config, &mut data)?;
    let source_department = department_for_agent_id(&config, source_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到发起部门，agentId={source_agent_id}"))?;
    let target_department = department_by_id(&config, target_department_id)
        .cloned()
        .ok_or_else(|| format!("目标部门不存在，departmentId={target_department_id}"))?;
    let target_agent_id = target_department
        .agent_ids
        .iter()
        .find(|id| !id.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("目标部门没有可用委任人，departmentId={target_department_id}"))?;
    if !data
        .agents
        .iter()
        .any(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
    {
        drop(guard);
        return Err(format!("目标委任人不存在，agentId={target_agent_id}"));
    }
    let thread_context = if let Some(conversation_id) = source_conversation_id {
        delegate_runtime_thread_get(app_state, conversation_id)?
    } else {
        None
    };
    let source_conversation_id = if let Some(thread) = thread_context.as_ref() {
        thread.root_conversation_id.clone()
    } else {
        let requested_conversation_id = source_conversation_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "主代理缺少当前会话 ID，无法发起委托".to_string())?;
        data.conversations
            .iter()
            .find(|item| {
                item.id == requested_conversation_id
                    && item.summary.trim().is_empty()
                    && !conversation_is_delegate(item)
            })
            .map(|item| item.id.clone())
            .ok_or_else(|| format!("未找到指定主会话，conversationId={requested_conversation_id}"))?
    };
    drop(guard);
    Ok((
        config,
        data,
        source_department,
        target_department,
        target_agent_id,
        source_conversation_id,
        thread_context,
    ))
}

fn delegate_create_record(
    app_state: &AppState,
    root_conversation_id: &str,
    parent_delegate_id: Option<String>,
    source_department_id: &str,
    target_department_id: &str,
    source_agent_id: &str,
    target_agent_id: &str,
    title: &str,
    instruction: &str,
    background: String,
    specific_goal: String,
    deliverable_requirement: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
) -> Result<DelegateEntry, String> {
    delegate_store_create_delegate(
        &app_state.data_path,
        &DelegateCreateInput {
            kind: DELEGATE_TOOL_KIND_DELEGATE.to_string(),
            conversation_id: root_conversation_id.to_string(),
            parent_delegate_id,
            source_department_id: source_department_id.to_string(),
            target_department_id: target_department_id.to_string(),
            source_agent_id: source_agent_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            title: title.to_string(),
            instruction: instruction.to_string(),
            background,
            specific_goal,
            deliverable_requirement,
            notify_assistant_when_done,
            call_stack,
        },
    )
}

fn delegate_failed_result(reason: impl Into<String>) -> Value {
    let reason = reason.into();
    serde_json::json!({
        "ok": false,
        "status": "委托无法送达",
        "reason": reason,
        "message": "委托工具执行失败"
    })
}

#[derive(Debug, Clone)]
struct ValidatedDelegateArgs {
    mode: DelegateMode,
    target_department_id: String,
    instruction: String,
    title: String,
    background: String,
    specific_goal: String,
    deliverable_requirement: String,
    notify_assistant_when_done: bool,
}

fn validate_delegate_args(args: &DelegateToolArgs) -> Result<ValidatedDelegateArgs, String> {
    let mode = parse_delegate_mode(args.mode.as_deref())?;
    let target_department_id = args.department_id.trim().to_string();
    if target_department_id.is_empty() {
        return Err("delegate.department_id is required".to_string());
    }
    let instruction = args.instruction.trim().to_string();
    if instruction.is_empty() {
        return Err("delegate.instruction is required".to_string());
    }
    let title = args
        .task_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("未命名委托")
        .to_string();
    Ok(ValidatedDelegateArgs {
        mode,
        target_department_id,
        instruction,
        title,
        background: args.background.clone().unwrap_or_default(),
        specific_goal: args.specific_goal.clone().unwrap_or_default(),
        deliverable_requirement: args.deliverable_requirement.clone().unwrap_or_default(),
        notify_assistant_when_done: args.notify_assistant_when_done,
    })
}

fn check_and_push_call_stack(
    current_thread: Option<&DelegateRuntimeThread>,
    source_department_id: &str,
    target_department_id: &str,
) -> Result<Vec<String>, String> {
    if source_department_id == target_department_id {
        return Err("不能把委托发送给当前部门自己".to_string());
    }
    let mut call_stack = current_thread
        .map(|thread| thread.call_stack.clone())
        .unwrap_or_else(|| vec![source_department_id.to_string()]);
    if call_stack.iter().any(|item| item == target_department_id) {
        return Err(format!(
            "目标部门已在当前调用链中，departmentId={target_department_id}"
        ));
    }
    call_stack.push(target_department_id.to_string());
    Ok(call_stack)
}

#[derive(Debug, Clone)]
struct DelegatePreflight {
    config: AppConfig,
    data: AppData,
    source_department: DepartmentConfig,
    target_department: DepartmentConfig,
    target_agent_id: String,
    root_conversation_id: String,
    current_thread: Option<DelegateRuntimeThread>,
}

fn common_delegate_preflight(
    app_state: &AppState,
    source_agent_id: &str,
    source_conversation_id: Option<&str>,
    target_department_id: &str,
) -> Result<DelegatePreflight, String> {
    let (config, data, source_department, target_department, target_agent_id, root_conversation_id, current_thread) =
        delegate_resolve_context(
            app_state,
            source_agent_id,
            source_conversation_id,
            target_department_id,
        )?;
    Ok(DelegatePreflight {
        config,
        data,
        source_department,
        target_department,
        target_agent_id,
        root_conversation_id,
        current_thread,
    })
}

fn delegate_target_chat_api_config_ids(
    config: &AppConfig,
    target_department: &DepartmentConfig,
) -> Vec<String> {
    let valid_text_chat_api_ids = config
        .api_configs
        .iter()
        .filter(|api| api.enable_text && api.request_format.is_chat_text())
        .map(|api| api.id.clone())
        .collect::<std::collections::HashSet<_>>();
    department_api_config_ids(target_department)
        .into_iter()
        .filter(|id| valid_text_chat_api_ids.contains(id))
        .collect::<Vec<_>>()
}

fn spawn_delegate_task(
    app_state: AppState,
    delegate: DelegateEntry,
    root_conversation_id: String,
    target_api_config_ids: Vec<String>,
) {
    let app_state_for_run = app_state.clone();
    let app_state_for_publish = app_state;
    let delegate_for_run = delegate.clone();
    let delegate_for_publish = delegate;
    tokio::spawn(async move {
        let run_result = delegate_run_thread_to_completion(
            app_state_for_run,
            delegate_for_run,
            target_api_config_ids,
            None,
        )
        .await;
        match run_result {
            Ok(result) => {
                let text = if result.assistant_text.trim().is_empty() {
                    format!("《{}》已处理完成。", delegate_for_publish.title.trim())
                } else {
                    result.assistant_text.clone()
                };
                if let Err(err) = delegate_enqueue_result_message(
                    &app_state_for_publish,
                    &root_conversation_id,
                    &delegate_for_publish.target_agent_id,
                    &text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate_for_publish.delegate_id,
                        "delegateKind": delegate_for_publish.kind,
                        "resultStatus": "completed",
                        "speakerAgentId": delegate_for_publish.target_agent_id,
                        "sourceAgentId": delegate_for_publish.source_agent_id,
                        "targetAgentId": delegate_for_publish.target_agent_id,
                        "reasoningStandard": result.reasoning_standard,
                    }),
                    delegate_for_publish.notify_assistant_when_done,
                ) {
                    eprintln!(
                        "[委托线程] 投递委托完成消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        err
                    );
                }
            }
            Err(err) => {
                let fail_text = format!("《{}》执行失败：{}", delegate_for_publish.title.trim(), err);
                if let Err(enqueue_err) = delegate_enqueue_result_message(
                    &app_state_for_publish,
                    &root_conversation_id,
                    &delegate_for_publish.target_agent_id,
                    &fail_text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate_for_publish.delegate_id,
                        "delegateKind": delegate_for_publish.kind,
                        "resultStatus": "failed",
                        "speakerAgentId": delegate_for_publish.target_agent_id,
                        "sourceAgentId": delegate_for_publish.source_agent_id,
                        "targetAgentId": delegate_for_publish.target_agent_id,
                        "error": err,
                    }),
                    delegate_for_publish.notify_assistant_when_done,
                ) {
                    eprintln!(
                        "[委托线程] 投递委托失败消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        enqueue_err
                    );
                }
            }
        }
    });
}

fn resolve_delegate_call_stack(
    current_thread: Option<&DelegateRuntimeThread>,
    source_department: &DepartmentConfig,
    target_department: &DepartmentConfig,
) -> Result<Vec<String>, String> {
    check_and_push_call_stack(
        current_thread,
        &source_department.id,
        &target_department.id,
    )
}

async fn builtin_delegate(
    app_state: &AppState,
    session_id: &str,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let validated = match validate_delegate_args(&args) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };
    if validated.mode == DelegateMode::Sync {
        return delegate_execute_sync(app_state, session_id, args).await;
    }

    let (_, source_agent_id, source_conversation_id) = delegate_parse_session_parts(session_id);
    let preflight = match common_delegate_preflight(
        app_state,
        &source_agent_id,
        source_conversation_id.as_deref(),
        &validated.target_department_id,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    if preflight.current_thread.is_some() {
        eprintln!(
            "[工具][委托] 委托线程内禁止再次调用 delegate：mode=async, session_id={}",
            session_id
        );
        return Ok(delegate_failed_result("委托线程中禁止再次调用 delegate"));
    }
    if preflight.source_department.id != ASSISTANT_DEPARTMENT_ID
        && !preflight.source_department.is_built_in_assistant
    {
        return Ok(delegate_failed_result(
            "delegate.mode=async 只能由主助理发起；需要当前线程等待结果时请使用 mode=sync",
        ));
    }

    let call_stack = match resolve_delegate_call_stack(
        preflight.current_thread.as_ref(),
        &preflight.source_department,
        &preflight.target_department,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    let delegate = delegate_create_record(
        app_state,
        &preflight.root_conversation_id,
        None,
        &preflight.source_department.id,
        &preflight.target_department.id,
        &source_agent_id,
        &preflight.target_agent_id,
        &validated.title,
        &validated.instruction,
        validated.background,
        validated.specific_goal,
        validated.deliverable_requirement,
        validated.notify_assistant_when_done,
        call_stack,
    )?;

    let target_name = preflight
        .data
        .agents
        .iter()
        .find(|agent| agent.id == preflight.target_agent_id)
        .map(|agent| agent.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| preflight.target_agent_id.clone());

    spawn_delegate_task(
        app_state.clone(),
        delegate.clone(),
        delegate.conversation_id.clone(),
        delegate_target_chat_api_config_ids(&preflight.config, &preflight.target_department),
    );

    Ok(serde_json::json!({
        "ok": true,
        "status": "委托已送达",
        "delegate": delegate,
        "targetName": target_name
    }))
}

async fn delegate_execute_sync(
    app_state: &AppState,
    session_id: &str,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let validated = match validate_delegate_args(&args) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };
    let (_, source_agent_id, source_conversation_id) = delegate_parse_session_parts(session_id);
    let preflight = match common_delegate_preflight(
        app_state,
        &source_agent_id,
        source_conversation_id.as_deref(),
        &validated.target_department_id,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    if preflight.current_thread.is_some() {
        eprintln!(
            "[工具][委托] 委托线程内禁止再次调用 delegate：mode=sync, session_id={}",
            session_id
        );
        return Ok(delegate_failed_result("委托线程中禁止再次调用 delegate"));
    }

    let call_stack = match resolve_delegate_call_stack(
        preflight.current_thread.as_ref(),
        &preflight.source_department,
        &preflight.target_department,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    let parent_delegate_id = preflight
        .current_thread
        .as_ref()
        .map(|thread| thread.delegate_id.clone());
    let delegate = delegate_create_record(
        app_state,
        &preflight.root_conversation_id,
        parent_delegate_id,
        &preflight.source_department.id,
        &preflight.target_department.id,
        &source_agent_id,
        &preflight.target_agent_id,
        &validated.title,
        &validated.instruction,
        validated.background,
        validated.specific_goal,
        validated.deliverable_requirement,
        false,
        call_stack,
    )?;

    match delegate_run_thread_to_completion(
        app_state.clone(),
        delegate.clone(),
        delegate_target_chat_api_config_ids(&preflight.config, &preflight.target_department),
        Some(session_id.to_string()),
    )
    .await
    {
        Ok(run) => Ok(serde_json::json!({
            "ok": true,
            "status": "委托完成",
            "delegate": delegate,
            "conversationId": preflight.root_conversation_id,
            "assistantText": run.assistant_text,
            "reasoningStandard": run.reasoning_standard,
            "targetAgentId": preflight.target_agent_id,
        })),
        Err(err) => Ok(serde_json::json!({
            "ok": false,
            "status": "委托无法送达",
            "delegate": delegate,
            "reason": err,
            "message": "委托工具执行失败"
        })),
    }
}

