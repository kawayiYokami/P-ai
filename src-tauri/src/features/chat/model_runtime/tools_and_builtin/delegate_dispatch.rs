fn delegate_resolve_context(
    app_state: &AppState,
    source_agent_id: &str,
    source_conversation_id: Option<&str>,
    target_agent_id: &str,
) -> Result<
    (
        AppConfig,
        Vec<AgentProfile>,
        AgentProfile,
        AgentProfile,
        String,
        Option<DelegateRuntimeThread>,
    ),
    String,
> {
    let resolved = conversation_service_v2().resolve_delegate_context(
        app_state,
        source_agent_id,
        source_conversation_id,
        target_agent_id,
    )?;
    Ok((
        resolved.config,
        resolved.agents,
        resolved.source_agent,
        resolved.target_agent,
        resolved.source_conversation_id,
        resolved.thread_context,
    ))
}

fn delegate_create_record(
    app_state: &AppState,
    kind: &str,
    root_conversation_id: &str,
    parent_delegate_id: Option<String>,
    source_agent_id: &str,
    target_agent_id: &str,
    title: &str,
    why: String,
    goal: String,
    todo: String,
    notify_assistant_when_done: bool,
    call_stack: Vec<String>,
) -> Result<DelegateEntry, String> {
    delegate_store_create_delegate(
        &app_state.data_path,
        &DelegateCreateInput {
            kind: kind.to_string(),
            conversation_id: root_conversation_id.to_string(),
            parent_delegate_id,
            source_agent_id: source_agent_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            title: title.to_string(),
            why,
            goal,
            todo,
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

const SAME_PERSONA_BACKGROUND_DELEGATE_REASON: &str =
    "你同时担任这个职位，只能使用 wait 等待结果";
const DELEGATE_THREAD_BACKGROUND_ONLY_REASON: &str = "委托线程中只能使用 wait 等待结果";

fn same_persona_background_delegate_block_reason(
    source_agent_id: &str,
    target_agent_id: &str,
) -> Option<&'static str> {
    let source_agent_id = source_agent_id.trim();
    let target_agent_id = target_agent_id.trim();
    if source_agent_id.is_empty() || target_agent_id.is_empty() {
        return None;
    }
    (source_agent_id == target_agent_id).then_some(SAME_PERSONA_BACKGROUND_DELEGATE_REASON)
}

#[cfg(test)]
mod delegate_dispatch_tests {
    use super::*;

    #[test]
    fn same_persona_background_delegate_block_reason_should_only_block_same_agent() {
        assert_eq!(
            same_persona_background_delegate_block_reason("agent-a", "agent-a"),
            Some(SAME_PERSONA_BACKGROUND_DELEGATE_REASON)
        );
        assert_eq!(
            same_persona_background_delegate_block_reason("agent-a", "agent-b"),
            None
        );
    }
}

#[derive(Debug, Clone)]
struct ValidatedDelegateArgs {
    mode: DelegateMode,
    target_agent_id: String,
    title: String,
    why: String,
    goal: String,
    todo: String,
}

fn delegate_title_from_goal(goal: &str) -> String {
    let compact = goal
        .trim()
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let title = compact.chars().take(32).collect::<String>();
    if title.trim().is_empty() {
        "未命名委托".to_string()
    } else {
        title
    }
}

fn validate_delegate_args(args: &DelegateToolArgs) -> Result<ValidatedDelegateArgs, String> {
    let mode = parse_delegate_mode(args.mode.as_deref())?;
    let target_agent_id = args.agent_id.trim().to_string();
    if target_agent_id.is_empty() {
        return Err("delegate.agent_id is required".to_string());
    }
    let why = delegate_arg_new_or_legacy(&args.why, &args.background);
    let goal = delegate_arg_new_or_legacy(&args.goal, &args.question);
    let todo = delegate_arg_new_or_legacy(&args.todo, &args.focus);
    if goal.trim().is_empty() {
        return Err("delegate.goal is required".to_string());
    }
    let title = delegate_title_from_goal(&goal);
    Ok(ValidatedDelegateArgs {
        mode,
        target_agent_id,
        title,
        why,
        goal,
        todo,
    })
}

fn check_and_push_call_stack(
    current_thread: Option<&DelegateRuntimeThread>,
    source_agent_id: &str,
    target_agent_id: &str,
) -> Result<Vec<String>, String> {
    let mut call_stack = current_thread
        .map(|thread| thread.call_stack.clone())
        .unwrap_or_else(|| vec![source_agent_id.to_string()]);
    let same_agent = source_agent_id == target_agent_id;
    if !same_agent && call_stack.iter().any(|item| item == target_agent_id) {
        return Err(format!(
            "目标人格已在当前调用链中，agentId={target_agent_id}"
        ));
    }
    if !same_agent {
        call_stack.push(target_agent_id.to_string());
    }
    Ok(call_stack)
}

#[derive(Debug, Clone)]
struct DelegatePreflight {
    config: AppConfig,
    source_agent: AgentProfile,
    target_agent: AgentProfile,
    root_conversation_id: String,
    current_thread: Option<DelegateRuntimeThread>,
}

fn common_delegate_preflight(
    app_state: &AppState,
    source_agent_id: &str,
    source_conversation_id: Option<&str>,
    target_agent_id: &str,
) -> Result<DelegatePreflight, String> {
    let (config, _agents, source_agent, target_agent, root_conversation_id, current_thread) =
        delegate_resolve_context(
            app_state,
            source_agent_id,
            source_conversation_id,
            target_agent_id,
        )?;
    Ok(DelegatePreflight {
        config,
        source_agent,
        target_agent,
        root_conversation_id,
        current_thread,
    })
}

fn validate_delegate_tool_direct_child_target(preflight: &DelegatePreflight) -> Result<(), String> {
    if preflight
        .source_agent
        .child_agent_ids
        .iter()
        .any(|id| id.trim() == preflight.target_agent.id)
    {
        return Ok(());
    }
    runtime_log_debug(format!(
        "[委托校验] 直接下级不匹配 source_agent_id={} source_agent_name={} target_agent_id={} target_agent_name={} source_child_agent_ids={:?}",
        preflight.source_agent.id,
        preflight.source_agent.name,
        preflight.target_agent.id,
        preflight.target_agent.name,
        preflight.source_agent.child_agent_ids
    ));
    Err(format!(
        "目标人格不是当前人格的直接下级，sourceAgentId={}，targetAgentId={}",
        preflight.source_agent.id, preflight.target_agent.id
    ))
}

fn delegate_target_chat_api_config_ids(
    config: &AppConfig,
    target_agent: &AgentProfile,
) -> Vec<String> {
    agent_effective_chat_api_config_ids(config, target_agent)
}

fn spawn_delegate_task(
    app_state: AppState,
    delegate: DelegateEntry,
    root_conversation_id: String,
    target_api_config_ids: Vec<String>,
    parent_chat_session_key: Option<String>,
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
            parent_chat_session_key,
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
                    }),
                    delegate_for_publish.notify_assistant_when_done,
                )
                .await
                {
                    runtime_log_error(format!(
                        "[委托线程] 投递委托完成消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        err
                    ));
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
                )
                .await
                {
                    runtime_log_error(format!(
                        "[委托线程] 投递委托失败消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
                        delegate_for_publish.delegate_id,
                        delegate_for_publish.target_agent_id,
                        root_conversation_id,
                        enqueue_err
                    ));
                }
            }
        }
    });
}

async fn run_sync_delegate_on_child_task(
    app_state: AppState,
    delegate: DelegateEntry,
    target_api_config_ids: Vec<String>,
    parent_chat_session_key: String,
) -> Result<SendChatResult, String> {
    // 同步委托仍需等待结果，但不要把子会话整条发送链路直接压在当前工具调用栈上。
    // 远程联系人路径会额外叠加一层上下文准备与 IM 规则处理，直接 await 容易把 tokio worker 栈顶爆。
    let abort_state = app_state.clone();
    let abort_delegate_id = delegate.delegate_id.clone();
    let (child_abort_handle, child_abort_registration) =
        futures_util::future::AbortHandle::new_pair();
    let mut child_abort_guard = SyncDelegateChildAbortGuard::new(child_abort_handle);
    let run_child = futures_util::future::Abortable::new(
        async move {
            delegate_run_thread_to_completion(
                app_state,
                delegate,
                target_api_config_ids,
                Some(parent_chat_session_key),
            )
            .await
        },
        child_abort_registration,
    );
    let join = tokio::spawn(run_child);
    let join_result = join.await;
    child_abort_guard.complete();
    match join_result {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => {
            let _ = abort_delegate_runtime_thread(
                &abort_state,
                &abort_delegate_id,
                "同步委托子任务被取消",
            );
            Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
        }
        Err(err) => {
            let _ = abort_delegate_runtime_thread(
                &abort_state,
                &abort_delegate_id,
                "同步委托子任务异常结束",
            );
            Err(format!("同步委托子任务异常结束: {err}"))
        }
    }
}

struct SyncDelegateChildAbortGuard {
    abort_handle: futures_util::future::AbortHandle,
    completed: bool,
}

impl SyncDelegateChildAbortGuard {
    fn new(abort_handle: futures_util::future::AbortHandle) -> Self {
        Self {
            abort_handle,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for SyncDelegateChildAbortGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.abort_handle.abort();
        }
    }
}

struct SyncDelegateAbortGuard {
    state: AppState,
    delegate_id: String,
    completed: bool,
}

impl SyncDelegateAbortGuard {
    fn new(state: AppState, delegate_id: String) -> Self {
        Self {
            state,
            delegate_id,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for SyncDelegateAbortGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let _ = abort_delegate_runtime_thread(
            &self.state,
            &self.delegate_id,
            "同步委托等待层被取消",
        );
    }
}

fn resolve_delegate_call_stack(
    current_thread: Option<&DelegateRuntimeThread>,
    source_agent: &AgentProfile,
    target_agent: &AgentProfile,
) -> Result<Vec<String>, String> {
    check_and_push_call_stack(current_thread, &source_agent.id, &target_agent.id)
}

async fn builtin_delegate(
    app_state: &AppState,
    session_id: &str,
    source_agent_id: Option<&str>,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let validated = match validate_delegate_args(&args) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };
    let session_agent_id = delegate_session_agent_id(session_id);
    let source_conversation_id = delegate_session_conversation_id(session_id);
    let source_agent_id = source_agent_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(session_agent_id);
    let preflight = match common_delegate_preflight(
        app_state,
        &source_agent_id,
        source_conversation_id.as_deref(),
        &validated.target_agent_id,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };
    if let Err(err) = validate_delegate_tool_direct_child_target(&preflight) {
        return Ok(delegate_failed_result(err));
    }
    if validated.mode == DelegateMode::Wait {
        return delegate_execute_sync(
            app_state,
            session_id,
            Some(source_agent_id.as_str()),
            DELEGATE_TOOL_KIND_DELEGATE,
            args,
        )
        .await;
    }

    if preflight.current_thread.is_some() {
        runtime_log_info(format!(
            "[工具][委托] 委托线程内禁止再次调用 delegate：mode=background, session_id={}",
            session_id
        ));
        return Ok(delegate_failed_result(DELEGATE_THREAD_BACKGROUND_ONLY_REASON));
    }
    if let Some(reason) = same_persona_background_delegate_block_reason(
        &source_agent_id,
        &preflight.target_agent.id,
    ) {
        return Ok(delegate_failed_result(reason));
    }
    let call_stack = match resolve_delegate_call_stack(
        preflight.current_thread.as_ref(),
        &preflight.source_agent,
        &preflight.target_agent,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    let delegate = delegate_create_record(
        app_state,
        DELEGATE_TOOL_KIND_DELEGATE,
        &preflight.root_conversation_id,
        None,
        &source_agent_id,
        &preflight.target_agent.id,
        &validated.title,
        validated.why,
        validated.goal,
        validated.todo,
        false,
        call_stack,
    )?;

    let target_name = preflight
        .target_agent
        .name
        .trim()
        .to_string();
    let target_name = if target_name.is_empty() {
        preflight.target_agent.id.clone()
    } else {
        target_name
    };

    spawn_delegate_task(
        app_state.clone(),
        delegate.clone(),
        delegate.conversation_id.clone(),
        delegate_target_chat_api_config_ids(&preflight.config, &preflight.target_agent),
        Some(session_id.to_string()),
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
    source_agent_id: Option<&str>,
    kind: &str,
    args: DelegateToolArgs,
) -> Result<Value, String> {
    let validated = match validate_delegate_args(&args) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };
    let session_agent_id = delegate_session_agent_id(session_id);
    let source_conversation_id = delegate_session_conversation_id(session_id);
    let source_agent_id = source_agent_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or(session_agent_id);
    let preflight = match common_delegate_preflight(
        app_state,
        &source_agent_id,
        source_conversation_id.as_deref(),
        &validated.target_agent_id,
    ) {
        Ok(value) => value,
        Err(err) => return Ok(delegate_failed_result(err)),
    };

    let call_stack = match resolve_delegate_call_stack(
        preflight.current_thread.as_ref(),
        &preflight.source_agent,
        &preflight.target_agent,
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
        kind,
        &preflight.root_conversation_id,
        parent_delegate_id,
        &source_agent_id,
        &preflight.target_agent.id,
        &validated.title,
        validated.why,
        validated.goal,
        validated.todo,
        false,
        call_stack,
    )?;
    let mut abort_guard = SyncDelegateAbortGuard::new(app_state.clone(), delegate.delegate_id.clone());

    let sync_result = run_sync_delegate_on_child_task(
        app_state.clone(),
        delegate.clone(),
        delegate_target_chat_api_config_ids(&preflight.config, &preflight.target_agent),
        session_id.to_string(),
    )
    .await;
    abort_guard.complete();
    match sync_result {
        Ok(run) => Ok(serde_json::json!({
            "ok": true,
            "status": "委托完成",
            "delegate": delegate,
            "conversationId": preflight.root_conversation_id,
            "assistantText": if run.final_response_text.trim().is_empty() { run.assistant_text } else { run.final_response_text },
            "targetAgentId": preflight.target_agent.id,
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
