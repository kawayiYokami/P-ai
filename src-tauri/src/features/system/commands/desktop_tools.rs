#[tauri::command]
async fn desktop_screenshot(input: ScreenshotRequest) -> Result<ScreenshotResponse, String> {
    run_screenshot_tool(input)
        .await
        .map_err(|err| to_tool_err_string(&err))
}

#[tauri::command]
async fn desktop_wait(input: WaitRequest) -> Result<WaitResponse, String> {
    run_wait_tool(input)
        .await
        .map_err(|err| to_tool_err_string(&err))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSelfCheckStepResult {
    name: String,
    ok: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

#[tauri::command]
async fn terminal_self_check(state: State<'_, AppState>) -> Result<Value, String> {
    let session_id = normalize_terminal_tool_session_id("ui-terminal-self-check");
    #[cfg(target_os = "windows")]
    if state.terminal_shell.kind == "missing-git-bash" {
        return Ok(serde_json::json!({
            "ok": false,
            "blockedReason": "missing_git_bash",
            "message": "Git Bash is required for terminal tools on Windows.",
            "sessionId": session_id,
            "shellKind": state.terminal_shell.kind,
            "shellPath": state.terminal_shell.path,
            "steps": []
        }));
    }

    let root_path = terminal_session_root_canonical(&state, &session_id)?;
    let cwd = resolve_terminal_cwd(&state, &session_id, None)?;
    let allowed_project_roots = terminal_allowed_project_roots_canonical(&state)?
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let steps = vec![
        "pwd",
        "echo $0",
        "git --version",
        "bash --version",
        "command -v git",
        "command -v bash",
    ];

    let mut results = Vec::<TerminalSelfCheckStepResult>::new();
    for step in steps {
        match sandbox_execute_command(&state, &session_id, step, &cwd, 15_000).await {
            Ok(execution) => {
                let (stdout, _) = truncate_terminal_output(&execution.stdout);
                let (stderr, _) = truncate_terminal_output(&execution.stderr);
                results.push(TerminalSelfCheckStepResult {
                    name: step.to_string(),
                    ok: execution.ok,
                    exit_code: execution.exit_code,
                    stdout,
                    stderr,
                    duration_ms: execution.duration_ms,
                });
            }
            Err(err) => {
                results.push(TerminalSelfCheckStepResult {
                    name: step.to_string(),
                    ok: false,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: err,
                    duration_ms: 0,
                });
                break;
            }
        }
    }

    let ok = results.iter().all(|item| item.ok);
    Ok(serde_json::json!({
        "ok": ok,
        "sessionId": session_id,
        "rootPath": root_path.to_string_lossy().to_string(),
        "cwd": cwd.to_string_lossy().to_string(),
        "shellKind": state.terminal_shell.kind,
        "shellPath": state.terminal_shell.path,
        "allowedProjectRoots": allowed_project_roots,
        "steps": results,
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveTerminalApprovalInput {
    request_id: String,
    approved: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatShellWorkspaceInput {
    api_config_id: String,
    agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockChatShellWorkspaceInput {
    api_config_id: String,
    agent_id: String,
    workspace_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnlockChatShellWorkspaceInput {
    api_config_id: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatShellWorkspaceOutput {
    session_id: String,
    workspace_name: String,
    root_path: String,
    locked: bool,
}

fn resolve_chat_tool_session_id(
    state: &AppState,
    api_config_id: &str,
    agent_id: &str,
) -> Result<String, String> {
    let api_id = api_config_id.trim();
    let agent = agent_id.trim();
    if api_id.is_empty() {
        return Err("apiConfigId is required.".to_string());
    }
    if agent.is_empty() {
        return Err("agentId is required.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let config = read_config(&state.config_path)?;
    if !config.api_configs.iter().any(|v| v.id == api_id) {
        drop(guard);
        return Err(format!("Selected API config '{api_id}' not found."));
    }
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data.agents.iter().any(|v| v.id == agent && !v.is_built_in_user) {
        drop(guard);
        return Err(format!("Selected agent '{agent}' not found."));
    }
    drop(guard);

    let session_id = inflight_chat_key(api_id, agent);
    Ok(normalize_terminal_tool_session_id(&session_id))
}

fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn resolve_workspace_display_name(state: &AppState, root: &Path) -> String {
    let root_key = normalize_terminal_path_for_compare(root);
    if let Ok(workspaces) = terminal_allowed_workspaces_canonical(state) {
        for ws in workspaces {
            if normalize_terminal_path_for_compare(&ws.path) == root_key {
                return ws.name;
            }
        }
    }
    workspace_name_from_path(root)
}

#[tauri::command]
fn get_chat_shell_workspace(
    input: ChatShellWorkspaceInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(&state, &input.api_config_id, &input.agent_id)?;
    let root = terminal_session_root_canonical(&state, &session_id)?;
    let default_root = terminal_default_session_root_canonical(&state)?;
    let locked = normalize_terminal_path_for_compare(&root)
        != normalize_terminal_path_for_compare(&default_root);
    Ok(ChatShellWorkspaceOutput {
        session_id,
        workspace_name: resolve_workspace_display_name(&state, &root),
        root_path: root.to_string_lossy().to_string(),
        locked,
    })
}

#[tauri::command]
fn lock_chat_shell_workspace(
    input: LockChatShellWorkspaceInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(&state, &input.api_config_id, &input.agent_id)?;
    let target_text = input.workspace_path.trim();
    if target_text.is_empty() {
        return Err("workspacePath is required.".to_string());
    }
    let target = PathBuf::from(target_text)
        .canonicalize()
        .map_err(|err| format!("Resolve workspace path failed: {err}"))?;
    if !target.is_dir() {
        return Err("workspacePath must be a directory.".to_string());
    }
    {
        let mut roots = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        roots.insert(session_id.clone(), target.to_string_lossy().to_string());
    }
    Ok(ChatShellWorkspaceOutput {
        session_id,
        workspace_name: workspace_name_from_path(&target),
        root_path: target.to_string_lossy().to_string(),
        locked: true,
    })
}

#[tauri::command]
fn unlock_chat_shell_workspace(
    input: UnlockChatShellWorkspaceInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(&state, &input.api_config_id, &input.agent_id)?;
    {
        let mut roots = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        roots.remove(&session_id);
    }
    let root = terminal_session_root_canonical(&state, &session_id)?;
    Ok(ChatShellWorkspaceOutput {
        session_id,
        workspace_name: resolve_workspace_display_name(&state, &root),
        root_path: root.to_string_lossy().to_string(),
        locked: false,
    })
}

#[tauri::command]
fn resolve_terminal_approval(
    input: ResolveTerminalApprovalInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = resolve_terminal_approval_request(&state, &input.request_id, input.approved)?;
    Ok(())
}

