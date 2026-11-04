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

    fn delegate_args(agent_id: &str) -> DelegateToolArgs {
        DelegateToolArgs {
            agent_id: agent_id.to_string(),
            mode: None,
            why: Some("背景".to_string()),
            goal: Some("目标".to_string()),
            todo: Some("边界".to_string()),
            background: None,
            question: None,
            focus: None,
        }
    }

    #[test]
    fn validate_delegate_args_should_default_empty_target_to_deputy() {
        let validated =
            validate_delegate_args(&delegate_args("  ")).expect("缺省目标人格应是合法调用");

        assert_eq!(validated.target_agent_id, DEPUTY_AGENT_ID);
        assert!(validated.target_agent_defaulted);
    }

    #[test]
    fn validate_delegate_args_should_keep_explicit_target() {
        let validated = validate_delegate_args(&delegate_args("审阅者")).expect("显式目标");

        assert_eq!(validated.target_agent_id, "审阅者");
        assert!(!validated.target_agent_defaulted);
    }

    #[test]
    fn validate_delegate_args_should_still_require_goal() {
        let mut args = delegate_args("副手");
        args.goal = None;

        assert_eq!(
            validate_delegate_args(&args).unwrap_err(),
            "delegate.goal is required"
        );
    }
}

#[derive(Debug, Clone)]
struct ValidatedDelegateArgs {
    mode: DelegateMode,
    target_agent_id: String,
    /// 目标是否由缺省规则补出（用户没写 agent_id）；缺省目标允许指向当前人格自己。
    target_agent_defaulted: bool,
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
    let requested_target = args.agent_id.trim().to_string();
    // 不填目标人格是合法调用：交给副手。
    let target_agent_defaulted = requested_target.is_empty();
    let target_agent_id = if target_agent_defaulted {
        DEPUTY_AGENT_ID.to_string()
    } else {
        requested_target
    };
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
        target_agent_defaulted,
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
    agents: Vec<AgentProfile>,
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
    let (config, agents, source_agent, target_agent, root_conversation_id, current_thread) =
        delegate_resolve_context(
            app_state,
            source_agent_id,
            source_conversation_id,
            target_agent_id,
        )?;
    Ok(DelegatePreflight {
        config,
        agents,
        source_agent,
        target_agent,
        root_conversation_id,
        current_thread,
    })
}

/// 目标必须是可委托的队友：人格库里除用户人格、系统播报人格、当前人格自己之外的全部人格。
/// 不看组织关系，队长、队员、无组织关系的人格一律放行。
/// 缺省补出的副手是唯一例外：当前人格本身可能就是副手，那时允许指向自己（后台模式仍由
/// `same_persona_background_delegate_block_reason` 拦截）。
fn validate_delegate_target_is_teammate(
    preflight: &DelegatePreflight,
    target_agent_defaulted: bool,
) -> Result<(), String> {
    let target_agent_id = preflight.target_agent.id.trim();
    let same_agent = preflight.source_agent.id.trim() == target_agent_id;
    if same_agent && target_agent_defaulted {
        return Ok(());
    }
    let is_teammate = available_delegate_teammates(&preflight.agents, &preflight.source_agent.id)
        .iter()
        .any(|agent| agent.id.trim() == target_agent_id);
    if is_teammate {
        return Ok(());
    }
    runtime_log_debug(format!(
        "[委托校验] 目标不在可委托名单 source_agent_id={} source_agent_name={} target_agent_id={} target_agent_name={}",
        preflight.source_agent.id,
        preflight.source_agent.name,
        preflight.target_agent.id,
        preflight.target_agent.name
    ));
    Err(format!(
        "目标人格不在可委托名单里，sourceAgentId={}，targetAgentId={}",
        preflight.source_agent.id, preflight.target_agent.id
    ))
}

fn delegate_target_chat_api_config_ids(
    config: &AppConfig,
    target_agent: &AgentProfile,
) -> Vec<String> {
    agent_effective_chat_api_config_ids(config, target_agent)
}

/// 解析委托目标人格的模型候选。
/// 目标人格配置「跟随会话」时，用发起方会话首选模型（无则回退全局专家）；
/// 否则按人格自身模型解析。
fn resolve_delegate_target_api_config_ids(
    app_state: &AppState,
    preflight: &DelegatePreflight,
    session_id: Option<&str>,
) -> Result<Vec<String>, String> {
    if !agent_follows_session_delegate_model(&preflight.target_agent) {
        return Ok(delegate_target_chat_api_config_ids(
            &preflight.config,
            &preflight.target_agent,
        ));
    }
    // 源会话的当前生效模型（自身 → 父链 → 主会话 → 专家 → 第一个可用模型）优先；
    // 解析不出来时再走全局兜底，终点是第一个可用聊天文本模型。
    let source_effective_model = session_id
        .and_then(delegate_session_conversation_id)
        .and_then(|conversation_id| {
            resolve_effective_conversation_model(
                app_state,
                &preflight.config,
                conversation_id.trim(),
            )
        });
    let resolved = resolve_follow_session_delegate_api_config(
        &preflight.config,
        source_effective_model.as_deref(),
    )
    .ok_or_else(|| "跟随会话模型解析失败：无可用模型".to_string())?;
    Ok(vec![resolved])
}

/// 指定模型覆盖：把用户在卡片上选的模型放到候选列表首位，其余保留作失败回退。
fn apply_delegate_model_override(
    config: &AppConfig,
    mut api_config_ids: Vec<String>,
    model_override: Option<&str>,
) -> Result<Vec<String>, String> {
    let Some(raw) = model_override.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(api_config_ids);
    };
    let resolved = resolve_chat_api_config_id(config, raw)
        .ok_or_else(|| format!("指定模型不可用或不支持文本对话，apiConfigId={raw}"))?;
    api_config_ids.retain(|id| id != &resolved);
    api_config_ids.insert(0, resolved);
    Ok(api_config_ids)
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
                // 后台委托完成以「系统人格引导消息入队」投递，与用户 @委托、代码审查保持一致：
                // 引导入队会把结果当作引导插话并激活主助理，而不是只往会话里平铺一条结果消息。
                if let Err(err) = conversation_service_v2().enqueue_delegate_completion_notification(
                    &app_state_for_publish,
                    &root_conversation_id,
                    &delegate_for_publish.target_agent_id,
                    &delegate_for_publish.title,
                    &text,
                    "delegate_tool_background_completion",
                ) {
                    runtime_log_error(format!(
                        "[委托线程] 投递委托完成引导消息失败: delegate_id={}, target_agent_id={}, root_conversation_id={}, error={}",
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

/// 同步委托的等待上限：这条路径此前没有任何超时，子任务不返回就无限等。
/// 刻意小于通用的 900 秒工具兜底（runtime_execution），因为这里能在超时后主动中止
/// 子任务并带上委托 id 记日志，比被外层统一掐断更有据可查。
const SYNC_DELEGATE_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// 等待同步委托子任务；None 表示已到点，中止子任务是调用方的事。
async fn await_sync_delegate_child<T>(
    join: tokio::task::JoinHandle<T>,
    timeout: std::time::Duration,
) -> Option<Result<T, tokio::task::JoinError>> {
    match tokio::time::timeout(timeout, join).await {
        Ok(join_result) => Some(join_result),
        Err(_) => None,
    }
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
    // 子任务永不 resolve 时，以前的裸 await 会把整轮调度挂在这里直到用户手点停止。
    // 超时后不 complete guard：Drop 里的 abort_handle 会掐掉子任务，避免它在报错之后
    // 继续跑完并把一份「成功」结果投递进根会话，与这里回报的失败互相矛盾。
    let join_result = match await_sync_delegate_child(join, SYNC_DELEGATE_JOIN_TIMEOUT).await {
        Some(join_result) => {
            child_abort_guard.complete();
            join_result
        }
        None => {
            let _ = abort_delegate_runtime_thread(
                &abort_state,
                &abort_delegate_id,
                "同步委托等待超时",
            );
            runtime_log_warn(format!(
                "[委托线程] 同步委托等待超时，已中止子任务: delegate_id={}, timeout_ms={}",
                abort_delegate_id,
                SYNC_DELEGATE_JOIN_TIMEOUT.as_millis()
            ));
            return Err(format!(
                "同步委托等待超时（{} 秒），已中止本次委托；委托可能未完成，请缩小任务范围或改用后台委托后重试。",
                SYNC_DELEGATE_JOIN_TIMEOUT.as_secs()
            ));
        }
    };
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
        Err(err) => {
            if validated.target_agent_defaulted {
                runtime_log_warn(format!(
                    "[工具][委托] 缺省目标解析失败，任务=落到副手，agent_id={}，error={err}",
                    DEPUTY_AGENT_ID
                ));
                return Ok(delegate_failed_result(
                    "副手人格不存在，请指定 agent_id 或恢复副手".to_string(),
                ));
            }
            return Ok(delegate_failed_result(err));
        }
    };
    if let Err(err) =
        validate_delegate_target_is_teammate(&preflight, validated.target_agent_defaulted)
    {
        return Ok(delegate_failed_result(err));
    }
    if validated.mode == DelegateMode::Wait {
        return delegate_execute_sync(
            app_state,
            session_id,
            Some(source_agent_id.as_str()),
            DELEGATE_TOOL_KIND_DELEGATE,
            args,
            None,
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
        true,
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
        resolve_delegate_target_api_config_ids(
            app_state,
            &preflight,
            Some(session_id),
        )?,
        Some(session_id.to_string()),
    );

    Ok(serde_json::json!({
        "ok": true,
        "status": "委托已送达",
        "delegate": delegate_tool_entry(delegate),
        "targetName": target_name
    }))
}

/// 委托回执里的 `delegate` 结构：剔掉委托请求正文。
///
/// `why` / `goal` / `todo` 是调用方刚写下的内容，原样搬回只是重复计费；
/// `assistantText` 是子代理产出，不属于入参回显，保持不动。
fn delegate_tool_entry(entry: DelegateEntry) -> Value {
    let mut value = serde_json::to_value(entry).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for key in ["why", "goal", "todo"] {
            map.remove(key);
        }
    }
    value
}

async fn delegate_execute_sync(
    app_state: &AppState,
    session_id: &str,
    source_agent_id: Option<&str>,
    kind: &str,
    args: DelegateToolArgs,
    model_override: Option<&str>,
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
        true,
        call_stack,
    )?;
    let mut abort_guard = SyncDelegateAbortGuard::new(app_state.clone(), delegate.delegate_id.clone());

    let sync_result = run_sync_delegate_on_child_task(
        app_state.clone(),
        delegate.clone(),
        apply_delegate_model_override(
            &preflight.config,
            resolve_delegate_target_api_config_ids(app_state, &preflight, Some(session_id))?,
            model_override,
        )?,
        session_id.to_string(),
    )
    .await;
    abort_guard.complete();
    match sync_result {
        Ok(run) => Ok(serde_json::json!({
            "ok": true,
            "status": "委托完成",
            "delegate": delegate_tool_entry(delegate),
            "conversationId": preflight.root_conversation_id,
            "assistantText": if run.final_response_text.trim().is_empty() { run.assistant_text } else { run.final_response_text },
            "targetAgentId": preflight.target_agent.id,
        })),
        Err(err) => Ok(serde_json::json!({
            "ok": false,
            "status": "委托无法送达",
            "delegate": delegate_tool_entry(delegate),
            "reason": err,
            "message": "委托工具执行失败"
        })),
    }
}
