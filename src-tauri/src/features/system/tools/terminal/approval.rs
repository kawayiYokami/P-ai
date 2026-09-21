#[derive(Debug)]
struct TerminalApprovalDecision {
    approved: bool,
    reason: Option<String>,
}

#[derive(Debug)]
struct PendingTerminalApprovalRequest {
    sender: tokio::sync::oneshot::Sender<TerminalApprovalDecision>,
    session_id: String,
    workspace_path: Option<String>,
    payload: TerminalApprovalRequestPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalApprovalRequestPayload {
    request_id: String,
    title: String,
    message: String,
    approval_kind: String,
    session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    call_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    existing_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    target_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_opinion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_model_name: Option<String>,
    #[serde(default)]
    can_remember_workspace: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

fn approval_workspace_memory_target(
    state: &AppState,
    session_id: &str,
    requested_path: Option<&Path>,
    existing_paths: &[PathBuf],
    target_paths: &[PathBuf],
    cwd: Option<&Path>,
) -> Option<(String, String)> {
    let conversation = terminal_session_conversation(state, session_id).ok().flatten()?;
    let configured = normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
    if configured.is_empty() {
        return None;
    }

    let candidates = requested_path
        .into_iter()
        .map(PathBuf::from)
        .chain(target_paths.iter().cloned())
        .chain(existing_paths.iter().cloned())
        .chain(cwd.into_iter().map(PathBuf::from))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    for workspace in configured {
        if normalize_shell_workspace_access_text(&workspace.access)
            != SHELL_WORKSPACE_ACCESS_APPROVAL
        {
            continue;
        }
        let canonical = match PathBuf::from(workspace.path.trim()).canonicalize() {
            Ok(value) if value.is_dir() => value,
            _ => continue,
        };
        if candidates.iter().any(|candidate| path_is_within(&canonical, candidate)) {
            let display_path = terminal_path_for_user(&canonical);
            let display_name = workspace.name.trim().to_string();
            return Some((display_name, display_path));
        }
    }
    None
}

fn remember_terminal_workspace_without_approval(
    state: &AppState,
    session_id: &str,
    workspace_path: &str,
) -> Result<(), String> {
    let normalized_workspace_path =
        normalize_terminal_path_input_for_current_platform(workspace_path.trim());
    if normalized_workspace_path.is_empty() {
        return Err("workspacePath is empty.".to_string());
    }
    let Some(conversation_id) = terminal_session_conversation_id(session_id) else {
        return Err("当前审批不属于可持久化会话。".to_string());
    };
    let conversation = terminal_session_conversation(state, session_id)?
        .ok_or_else(|| "当前审批不属于可持久化会话。".to_string())?;
    let mut workspaces =
        normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
    let target_key = normalize_terminal_path_for_compare(&PathBuf::from(&normalized_workspace_path));
    let mut changed = false;
    for workspace in &mut workspaces {
        let workspace_key =
            normalize_terminal_path_for_compare(&PathBuf::from(workspace.path.trim()));
        if workspace_key != target_key {
            continue;
        }
        if normalize_shell_workspace_access_text(&workspace.access)
            != SHELL_WORKSPACE_ACCESS_FULL_ACCESS
        {
            workspace.access = SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string();
            changed = true;
        }
    }
    if !changed {
        return Err("当前审批不属于可记忆的会话工作区。".to_string());
    }
    let _ = apply_conversation_chat_workspace_changes(
        state,
        &conversation_id,
        None,
        Some(workspaces),
        None,
        None,
        None,
        None,
        None,
    )?;
    Ok(())
}

fn remember_terminal_conversation_autonomous_mode(
    state: &AppState,
    session_id: &str,
) -> Result<(), String> {
    let Some(conversation_id) = terminal_session_conversation_id(session_id) else {
        return Err("当前审批不属于可持久化会话。".to_string());
    };
    let conversation = terminal_session_conversation(state, session_id)?
        .ok_or_else(|| "当前审批不属于可持久化会话。".to_string())?;
    if conversation.shell_autonomous_mode {
        return Ok(());
    }
    let _ = apply_conversation_chat_workspace_changes(
        state,
        &conversation_id,
        None,
        None,
        Some(true),
        None,
        None,
        None,
        None,
    )?;
    Ok(())
}

async fn terminal_request_user_approval(
    state: &AppState,
    title: &str,
    message: &str,
    session_id: &str,
    approval_kind: &str,
    tool_name: Option<&str>,
    summary: Option<&str>,
    call_preview: Option<&str>,
    cwd: Option<&Path>,
    command: Option<&str>,
    requested_path: Option<&Path>,
    reason: Option<&str>,
    existing_paths: &[PathBuf],
    target_paths: &[PathBuf],
    review_opinion: Option<&str>,
    review_model_name: Option<&str>,
    description: Option<&str>,
) -> Result<TerminalApprovalDecision, String> {
    let request_id = Uuid::new_v4().to_string();
    let app_handle = {
        let guard = state
            .app_handle
            .lock()
            .map_err(|_| "Failed to lock app handle".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "App handle is not ready".to_string())?
    };

    let workspace_memory_target = approval_workspace_memory_target(
        state,
        session_id,
        requested_path,
        existing_paths,
        target_paths,
        cwd,
    );
    let payload = TerminalApprovalRequestPayload {
        request_id: request_id.clone(),
        title: title.to_string(),
        message: message.to_string(),
        approval_kind: approval_kind.to_string(),
        session_id: session_id.to_string(),
        tool_name: tool_name
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        summary: summary
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        call_preview: call_preview
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        cwd: cwd.map(terminal_path_for_user),
        command: command
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        requested_path: requested_path.map(terminal_path_for_user),
        reason: reason
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        existing_paths: existing_paths
            .iter()
            .take(32)
            .map(|path| terminal_path_for_user(path))
            .collect(),
        target_paths: target_paths
            .iter()
            .take(32)
            .map(|path| terminal_path_for_user(path))
            .collect(),
        review_opinion: review_opinion
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        review_model_name: review_model_name
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
        can_remember_workspace: workspace_memory_target.is_some(),
        workspace_name: workspace_memory_target.as_ref().map(|(name, _)| name.clone()),
        workspace_path: workspace_memory_target.map(|(_, path)| path),
        description: description
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToString::to_string),
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<TerminalApprovalDecision>();
    {
        let mut pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        pending.insert(
            request_id.clone(),
            PendingTerminalApprovalRequest {
                sender: tx,
                session_id: normalize_terminal_tool_session_id(session_id),
                workspace_path: payload.workspace_path.clone(),
                payload: payload.clone(),
            },
        );
    }

    if let Err(err) = app_handle.emit("easy-call:terminal-approval-request", &payload) {
        if let Ok(mut pending) = state.terminal_pending_approvals.lock() {
            pending.remove(&request_id);
        }
        return Err(format!("Emit terminal approval request failed: {err}"));
    }
    if let Ok(value) = serde_json::to_value(&payload) {
        ide_chat_broadcast_notification("terminalApproval.requested", value);
    }

    // 同时发系统通知，用户不在聊天窗口时也能被提醒去审批；失败不阻断审批流程
    // 通知只提示有人格请求许可，不暴露命令/路径等具体内容
    let ui_language = state_read_config_cached(state)
        .map(|config| config.ui_language)
        .unwrap_or_else(|_| "zh-CN".to_string());
    let notify_title = terminal_localized_text(
        &ui_language,
        "人格请求工具执行许可",
        "人格請求工具執行許可",
        "Agent requests tool execution permission",
    );
    let notify_body = terminal_localized_text(
        &ui_language,
        "PAI 正在等待你的审批，请打开应用查看并决定是否允许。",
        "PAI 正在等待你的審批，請開啟應用程式檢視並決定是否允許。",
        "PAI is waiting for your approval. Open the app to review and decide.",
    );
    if let Err(err) = send_native_notification(&app_handle, &notify_title, &notify_body, true) {
        runtime_log_warn(format!("[审批通知] 发送失败: {err}"));
    }

    let wait_result = rx.await;

    if let Ok(mut pending) = state.terminal_pending_approvals.lock() {
        pending.remove(&request_id);
    }

    match wait_result {
        Ok(decision) => Ok(decision),
        Err(_) => Err("Terminal approval channel closed unexpectedly.".to_string()),
    }
}

fn normalize_terminal_approval_reason(reason: Option<&str>) -> Option<String> {
    let text = reason.map(str::trim).filter(|v| !v.is_empty()).map(|v| v.chars().take(500).collect::<String>())?;
    Some(text)
}

fn format_terminal_denied_message(base: &str, decision: &TerminalApprovalDecision) -> String {
    if let Some(reason) = decision.reason.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        format!("{base} 拒绝原因：{reason}")
    } else {
        base.to_string()
    }
}

fn resolve_terminal_approval_request(
    state: &AppState,
    request_id: &str,
    approved: bool,
    reason: Option<&str>,
) -> Result<bool, String> {
    let trimmed = request_id.trim();
    if trimmed.is_empty() {
        return Err("requestId is empty.".to_string());
    }

    let sender = {
        let mut pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        pending.remove(trimmed)
    };

    let Some(pending_request) = sender else {
        runtime_log_debug(format!(
            "[工具调试] 未找到终端审批请求: {}",
            trimmed
        ));
        return Ok(false);
    };

    let normalized_reason = normalize_terminal_approval_reason(reason);
    if pending_request.sender.send(TerminalApprovalDecision { approved, reason: normalized_reason }).is_err() {
        runtime_log_debug(format!(
            "[工具调试] 终端审批接收端已关闭: {}",
            trimmed
        ));
        return Ok(false);
    }
    Ok(true)
}

fn approve_terminal_approval_for_session_request(
    state: &AppState,
    request_id: &str,
) -> Result<bool, String> {
    let trimmed = request_id.trim();
    if trimmed.is_empty() {
        return Err("requestId is empty.".to_string());
    }
    let session_id = {
        let pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        pending
            .get(trimmed)
            .map(|item| item.session_id.clone())
            .ok_or_else(|| "terminal approval request not found".to_string())?
    };
    remember_terminal_conversation_autonomous_mode(state, &session_id)?;
    resolve_terminal_approval_request(state, trimmed, true, None)
}

fn approve_terminal_approval_for_workspace_request(
    state: &AppState,
    request_id: &str,
) -> Result<bool, String> {
    let trimmed = request_id.trim();
    if trimmed.is_empty() {
        return Err("requestId is empty.".to_string());
    }
    let (session_id, workspace_path) = {
        let pending = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "Failed to lock terminal pending approvals".to_string())?;
        let item = pending
            .get(trimmed)
            .ok_or_else(|| "terminal approval request not found".to_string())?;
        (
            item.session_id.clone(),
            item.workspace_path
                .clone()
                .ok_or_else(|| "当前审批不属于可记忆的会话工作区。".to_string())?,
        )
    };
    remember_terminal_workspace_without_approval(state, &session_id, &workspace_path)?;
    resolve_terminal_approval_request(state, trimmed, true, None)
}

fn list_pending_terminal_approval_requests(state: &AppState) -> Vec<TerminalApprovalRequestPayload> {
    let Ok(pending) = state.terminal_pending_approvals.lock() else {
        return vec![];
    };
    pending.values().map(|item| item.payload.clone()).collect()
}

#[tauri::command]
fn list_pending_terminal_approvals(state: State<'_, AppState>) -> Result<Vec<TerminalApprovalRequestPayload>, String> {
    Ok(list_pending_terminal_approval_requests(&state))
}

fn ide_chat_list_pending_terminal_approvals(state: &AppState) -> Result<Value, String> {
    let items = list_pending_terminal_approval_requests(state);
    ide_chat_serialize(items)
}
