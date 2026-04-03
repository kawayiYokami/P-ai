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


#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct XcapToolInput {
    method: String,
    #[serde(default)]
    args: Value,
}

fn xcap_arg_u32(args: &Value, key: &str) -> Result<u32, String> {
    let v = args
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| to_tool_err_string(&DesktopToolError::invalid_params(format!("{key} is required"))))?;
    u32::try_from(v).map_err(|_| {
        to_tool_err_string(&DesktopToolError::invalid_params(format!("{key} is out of range")))
    })
}

fn xcap_arg_i32(args: &Value, key: &str) -> Result<i32, String> {
    let v = args
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| to_tool_err_string(&DesktopToolError::invalid_params(format!("{key} is required"))))?;
    i32::try_from(v).map_err(|_| {
        to_tool_err_string(&DesktopToolError::invalid_params(format!("{key} is out of range")))
    })
}

fn xcap_optional_webp_quality(args: &Value) -> f32 {
    args.get("webpQuality")
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .unwrap_or(default_webp_quality())
}

fn xcap_optional_save_path(args: &Value) -> Option<String> {
    args.get("savePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

#[tauri::command]
async fn xcap(input: XcapToolInput) -> Result<Value, String> {
    let method = input.method.trim().to_string();
    let args = input.args;
    let out = match method.as_str() {
        "list_windows" => {
            let data = xcap_list_windows_infos().map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        "list_monitors" => {
            let data = xcap_list_monitors_infos().map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        "capture_focused_window" => {
            let req = ScreenshotRequest {
                mode: ScreenshotMode::Desktop,
                monitor_id: None,
                region: None,
                save_path: xcap_optional_save_path(&args),
                webp_quality: xcap_optional_webp_quality(&args),
            };
            let data = run_capture_window_tool(req, None)
                .map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        "capture_window" => {
            let window_id = xcap_arg_u32(&args, "windowId")?;
            let req = ScreenshotRequest {
                mode: ScreenshotMode::Desktop,
                monitor_id: None,
                region: None,
                save_path: xcap_optional_save_path(&args),
                webp_quality: xcap_optional_webp_quality(&args),
            };
            let data = run_capture_window_tool(req, Some(window_id))
                .map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        "capture_monitor" => {
            let monitor_id = xcap_arg_u32(&args, "monitorId")?;
            let req = ScreenshotRequest {
                mode: ScreenshotMode::Monitor,
                monitor_id: Some(monitor_id),
                region: None,
                save_path: xcap_optional_save_path(&args),
                webp_quality: xcap_optional_webp_quality(&args),
            };
            let data = run_screenshot_tool(req)
                .await
                .map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        "capture_region" => {
            let x = xcap_arg_i32(&args, "x")?;
            let y = xcap_arg_i32(&args, "y")?;
            let width = xcap_arg_u32(&args, "width")?;
            let height = xcap_arg_u32(&args, "height")?;
            let monitor_id = args
                .get("monitorId")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok());
            let req = ScreenshotRequest {
                mode: ScreenshotMode::Region,
                monitor_id,
                region: Some(ScreenBounds {
                    x,
                    y,
                    width,
                    height,
                }),
                save_path: xcap_optional_save_path(&args),
                webp_quality: xcap_optional_webp_quality(&args),
            };
            let data = run_screenshot_tool(req)
                .await
                .map_err(|err| to_tool_err_string(&err))?;
            serde_json::json!({
                "ok": true,
                "method": method,
                "data": data
            })
        }
        _ => {
            return Err(to_tool_err_string(&DesktopToolError::invalid_params(
                "unsupported xcap method",
            )));
        }
    };
    Ok(out)
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostRuntimePrerequisites {
    git_installed: bool,
    node_installed: bool,
}

fn command_exists_in_path(name: &str) -> bool {
    let raw = name.trim();
    if raw.is_empty() {
        return false;
    }
    let path_value = match std::env::var_os("PATH") {
        Some(value) => value,
        None => return false,
    };
    let name_path = Path::new(raw);
    let mut candidates = Vec::<String>::new();
    if name_path.extension().is_some() {
        candidates.push(raw.to_string());
    } else {
        candidates.push(raw.to_string());
        #[cfg(target_os = "windows")]
        {
            if let Some(pathext) = std::env::var_os("PATHEXT") {
                for ext in pathext.to_string_lossy().split(';') {
                    let trimmed = ext.trim();
                    if !trimmed.is_empty() {
                        candidates.push(format!("{raw}{trimmed}"));
                    }
                }
            } else {
                candidates.push(format!("{raw}.exe"));
            }
        }
    }

    for dir in std::env::split_paths(&path_value) {
        for candidate in &candidates {
            if dir.join(candidate).is_file() {
                return true;
            }
        }
    }
    false
}

#[tauri::command]
fn get_host_runtime_prerequisites() -> HostRuntimePrerequisites {
    HostRuntimePrerequisites {
        git_installed: command_exists_in_path("git"),
        node_installed: command_exists_in_path("node"),
    }
}

#[tauri::command]
async fn terminal_self_check(state: State<'_, AppState>) -> Result<Value, String> {
    let session_id = normalize_terminal_tool_session_id("ui-terminal-self-check");
    let runtime_shell = terminal_shell_for_state(&state);
    #[cfg(target_os = "windows")]
    if runtime_shell.kind == "missing-terminal-shell" {
        return Ok(serde_json::json!({
            "ok": false,
            "blockedReason": "missing_terminal_shell",
            "message": "No supported shell was detected on Windows. Install Git and use Git Bash: https://git-scm.com/downloads",
            "sessionId": session_id,
            "shellKind": runtime_shell.kind,
            "shellPath": runtime_shell.path,
            "steps": []
        }));
    }

    let root_path = terminal_session_root_canonical(&state, &session_id)?;
    let cwd = resolve_terminal_cwd(&state, &session_id, None)?;
    let allowed_project_roots = terminal_allowed_project_roots_canonical(&state)?
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let steps = if runtime_shell.kind.starts_with("powershell") {
        vec![
            "Get-Location",
            "$PSVersionTable.PSVersion.ToString()",
            "Get-Command git -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source",
            "Get-Command pwsh -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source",
        ]
    } else {
        vec![
            "pwd",
            "echo $0",
            "git --version",
            "bash --version",
            "command -v git",
            "command -v bash",
        ]
    };

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
        "shellKind": runtime_shell.kind,
        "shellPath": runtime_shell.path,
        "allowedProjectRoots": allowed_project_roots,
        "steps": results,
    }))
}

#[tauri::command]
fn list_terminal_shell_candidates(state: State<'_, AppState>) -> Result<Value, String> {
    let (preferred_kind, current, options) = terminal_shell_candidates_for_ui(&state);
    Ok(serde_json::json!({
        "preferredKind": preferred_kind,
        "currentKind": current.kind,
        "currentPath": current.path,
        "options": options,
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
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockChatShellWorkspaceInput {
    api_config_id: String,
    agent_id: String,
    conversation_id: Option<String>,
    workspace_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnlockChatShellWorkspaceInput {
    api_config_id: String,
    agent_id: String,
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellWorkspacePathInput {
    #[serde(default)]
    workspace_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrateWorkspaceDirectoryInput {
    old_path: String,
    new_path: String,
    task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceMigrationProgressPayload {
    task_id: String,
    stage: String,
    processed: usize,
    total: usize,
    current_path: Option<String>,
    message: String,
    done: bool,
    error: Option<String>,
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
    conversation_id: Option<&str>,
) -> Result<String, String> {
    let api_id = api_config_id.trim();
    let agent = agent_id.trim();
    if api_id.is_empty() {
        return Err("apiConfigId is required.".to_string());
    }
    if agent.is_empty() {
        return Err("agentId is required.".to_string());
    }

    let config = state_read_config_cached(state)?;
    if !config.api_configs.iter().any(|v| v.id == api_id) {
        return Err(format!("Selected API config '{api_id}' not found."));
    }
    let data = state_read_app_data_cached(state)?;
    if !data.agents.iter().any(|v| v.id == agent && !v.is_built_in_user) {
        return Err(format!("Selected agent '{agent}' not found."));
    }

    let session_id = inflight_chat_key(agent, conversation_id);
    Ok(normalize_terminal_tool_session_id(&session_id))
}

fn resolve_chat_workspace_conversation_id(
    state: &AppState,
    agent_id: &str,
    conversation_id: Option<&str>,
) -> Result<String, String> {
    if let Some(conversation_id) = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(conversation_id.to_string());
    }

    let data = state_read_app_data_cached(state)?;
    let agent_id = agent_id.trim();
    if let Some(conversation) = data
        .conversations
        .iter()
        .rev()
        .find(|item| {
            item.summary.trim().is_empty()
                && !conversation_is_delegate(item)
                && (agent_id.is_empty() || item.agent_id.trim() == agent_id)
        })
    {
        return Ok(conversation.id.clone());
    }
    Err("当前没有可用的活跃会话，需要提供 conversationId。".to_string())
}

fn update_conversation_shell_workspace_path(
    state: &AppState,
    conversation_id: &str,
    shell_workspace_path: Option<String>,
) -> Result<(), String> {
    if delegate_runtime_thread_conversation_get(state, conversation_id)?.is_some() {
        return delegate_runtime_thread_modify(state, conversation_id, move |thread| {
            thread.conversation.shell_workspace_path = shell_workspace_path;
            Ok(())
        });
    }

    let mut data = state_read_app_data_cached(state)?;
    let Some(conversation) = data
        .conversations
        .iter_mut()
        .find(|item| item.id == conversation_id)
    else {
        return Err(format!("指定会话不存在：{conversation_id}"));
    };
    conversation.shell_workspace_path = shell_workspace_path;
    let app_config = state_read_config_cached(state)?;
    let overview_payload = build_unarchived_conversation_overview_payload(state, &app_config, &data);
    state_write_app_data_cached(state, &data)?;
    emit_unarchived_conversation_overview_updated_payload(state, &overview_payload);
    Ok(())
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

fn open_shell_path_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in explorer failed: {err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in Finder failed: {err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|err| format!("Open in file manager failed: {err}"))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Open in file manager is not supported on this platform".to_string())
}

fn resolve_requested_shell_workspace_root(
    state: &AppState,
    requested: Option<&str>,
    create_if_missing: bool,
) -> Result<PathBuf, String> {
    let root = if let Some(raw) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        let normalized = normalize_terminal_path_input_for_current_platform(raw);
        if normalized.is_empty() {
            return Err("工作区路径不能为空".to_string());
        }
        let candidate = PathBuf::from(&normalized);
        if candidate.is_absolute() {
            candidate
        } else {
            state.llm_workspace_path.join(candidate)
        }
    } else {
        configured_workspace_root_path(state)?
    };

    if create_if_missing {
        return ensure_workspace_root_ready(&root);
    }
    let canonical = root
        .canonicalize()
        .map_err(|err| format!("解析工作区路径失败 ({}): {err}", root.display()))?;
    if !canonical.is_dir() {
        return Err(format!("工作区目录不存在：{}", canonical.display()));
    }
    Ok(canonical)
}

const WORKSPACE_MIGRATION_EVENT: &str = "easy-call:workspace-migration-progress";

fn emit_workspace_migration_progress(
    app: &AppHandle,
    payload: &WorkspaceMigrationProgressPayload,
) {
    let _ = app.emit(WORKSPACE_MIGRATION_EVENT, payload);
}

fn collect_workspace_entries(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(path)
        .map_err(|err| format!("读取目录失败 ({}): {err}", path.display()))?
    {
        let entry = entry.map_err(|err| format!("读取目录项失败 ({}): {err}", path.display()))?;
        let child = entry.path();
        out.push(child.clone());
        if child.is_dir() {
            collect_workspace_entries(&child, out)?;
        }
    }
    Ok(())
}

fn copy_workspace_entry_recursive(
    from: &Path,
    to: &Path,
    app: &AppHandle,
    task_id: &str,
    processed: &mut usize,
    total: usize,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(from)
        .map_err(|err| format!("读取路径信息失败 ({}): {err}", from.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("暂不支持迁移符号链接：{}", from.display()));
    }
    if metadata.is_dir() {
        if to.exists() && !to.is_dir() {
            return Err(format!("迁移失败，目标路径已存在同名文件：{}", to.display()));
        }
        fs::create_dir_all(to)
            .map_err(|err| format!("创建目录失败 ({}): {err}", to.display()))?;
        *processed += 1;
        emit_workspace_migration_progress(
            app,
            &WorkspaceMigrationProgressPayload {
                task_id: task_id.to_string(),
                stage: "copying".to_string(),
                processed: *processed,
                total,
                current_path: Some(to.to_string_lossy().to_string()),
                message: "正在复制目录".to_string(),
                done: false,
                error: None,
            },
        );
        for entry in fs::read_dir(from)
            .map_err(|err| format!("读取目录失败 ({}): {err}", from.display()))?
        {
            let entry = entry.map_err(|err| format!("读取目录项失败 ({}): {err}", from.display()))?;
            let child_from = entry.path();
            let child_to = to.join(entry.file_name());
            copy_workspace_entry_recursive(&child_from, &child_to, app, task_id, processed, total)?;
        }
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建目标目录失败 ({}): {err}", parent.display()))?;
    }
    if to.exists() {
        return Err(format!("迁移失败，目标路径已存在同名文件：{}", to.display()));
    }
    fs::copy(from, to).map_err(|err| {
        format!(
            "复制文件失败 ({} -> {}): {err}",
            from.display(),
            to.display()
        )
    })?;
    let target_meta = fs::metadata(to)
        .map_err(|err| format!("读取复制后文件失败 ({}): {err}", to.display()))?;
    if target_meta.len() != metadata.len() {
        return Err(format!("复制校验失败，文件大小不一致：{}", to.display()));
    }
    *processed += 1;
    emit_workspace_migration_progress(
        app,
        &WorkspaceMigrationProgressPayload {
            task_id: task_id.to_string(),
            stage: "copying".to_string(),
            processed: *processed,
            total,
            current_path: Some(to.to_string_lossy().to_string()),
            message: "正在复制文件".to_string(),
            done: false,
            error: None,
        },
    );
    Ok(())
}

fn remove_workspace_entry_recursive(
    path: &Path,
    app: &AppHandle,
    task_id: &str,
    processed: &mut usize,
    total: usize,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| format!("读取路径信息失败 ({}): {err}", path.display()))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|err| format!("读取目录失败 ({}): {err}", path.display()))?
        {
            let entry = entry.map_err(|err| format!("读取目录项失败 ({}): {err}", path.display()))?;
            remove_workspace_entry_recursive(&entry.path(), app, task_id, processed, total)?;
        }
        fs::remove_dir(path)
            .map_err(|err| format!("删除目录失败 ({}): {err}", path.display()))?;
    } else {
        fs::remove_file(path)
            .map_err(|err| format!("删除文件失败 ({}): {err}", path.display()))?;
    }
    *processed += 1;
    emit_workspace_migration_progress(
        app,
        &WorkspaceMigrationProgressPayload {
            task_id: task_id.to_string(),
            stage: "deleting".to_string(),
            processed: *processed,
            total,
            current_path: Some(path.to_string_lossy().to_string()),
            message: "正在清理旧目录".to_string(),
            done: false,
            error: None,
        },
    );
    Ok(())
}

#[tauri::command]
fn open_chat_shell_workspace_dir(
    input: Option<ShellWorkspacePathInput>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = resolve_requested_shell_workspace_root(
        &state,
        input.as_ref().and_then(|value| value.workspace_path.as_deref()),
        true,
    )?;
    open_shell_path_in_file_manager(&root)?;
    Ok(shell_workspace_display_path(&root))
}

fn shell_workspace_display_path(path: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        let raw = path.to_string_lossy();
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
        raw.to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_string_lossy().to_string()
    }
}

#[tauri::command]
fn reset_chat_shell_workspace(
    input: Option<ShellWorkspacePathInput>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let root = resolve_requested_shell_workspace_root(
        &state,
        input.as_ref().and_then(|value| value.workspace_path.as_deref()),
        true,
    )?;
    ensure_workspace_mcp_layout_at_root(&root)?;
    ensure_workspace_skills_layout_at_root(&root)?;
    ensure_workspace_private_organization_layout_at_root(&root)?;
    Ok(shell_workspace_display_path(&root))
}

#[tauri::command]
fn get_default_chat_shell_workspace_path(state: State<'_, AppState>) -> Result<String, String> {
    let root = terminal_default_session_root_canonical(&state)?;
    Ok(shell_workspace_display_path(&root))
}

#[tauri::command]
async fn migrate_shell_workspace_directory(
    input: MigrateWorkspaceDirectoryInput,
    app: AppHandle,
) -> Result<String, String> {
    let old_root = PathBuf::from(normalize_terminal_path_input_for_current_platform(&input.old_path));
    let new_root = PathBuf::from(normalize_terminal_path_input_for_current_platform(&input.new_path));
    if input.old_path.trim().is_empty() || input.new_path.trim().is_empty() {
        return Err("工作区迁移路径不能为空".to_string());
    }
    let task_id = input.task_id.trim().to_string();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let old_root = old_root
            .canonicalize()
            .map_err(|err| format!("解析旧工作区失败 ({}): {err}", old_root.display()))?;
        let new_root = ensure_workspace_root_ready(&new_root)?;
        emit_workspace_migration_progress(
            &app_handle,
            &WorkspaceMigrationProgressPayload {
                task_id: task_id.clone(),
                stage: "scanning".to_string(),
                processed: 0,
                total: 0,
                current_path: Some(old_root.to_string_lossy().to_string()),
                message: "正在扫描旧工作区".to_string(),
                done: false,
                error: None,
            },
        );
        let mut entries = Vec::<PathBuf>::new();
        collect_workspace_entries(&old_root, &mut entries)?;
        let total = entries.len();
        emit_workspace_migration_progress(
            &app_handle,
            &WorkspaceMigrationProgressPayload {
                task_id: task_id.clone(),
                stage: "copying".to_string(),
                processed: 0,
                total,
                current_path: Some(old_root.to_string_lossy().to_string()),
                message: "开始复制工作区内容".to_string(),
                done: false,
                error: None,
            },
        );
        let mut copy_processed = 0usize;
        for entry in fs::read_dir(&old_root)
            .map_err(|err| format!("读取旧工作区失败 ({}): {err}", old_root.display()))?
        {
            let entry = entry.map_err(|err| format!("读取旧工作区目录项失败 ({}): {err}", old_root.display()))?;
            let from = entry.path();
            let to = new_root.join(entry.file_name());
            copy_workspace_entry_recursive(&from, &to, &app_handle, &task_id, &mut copy_processed, total)?;
        }
        emit_workspace_migration_progress(
            &app_handle,
            &WorkspaceMigrationProgressPayload {
                task_id: task_id.clone(),
                stage: "deleting".to_string(),
                processed: 0,
                total,
                current_path: Some(old_root.to_string_lossy().to_string()),
                message: "开始清理旧工作区".to_string(),
                done: false,
                error: None,
            },
        );
        let mut delete_processed = 0usize;
        for entry in fs::read_dir(&old_root)
            .map_err(|err| format!("读取旧工作区失败 ({}): {err}", old_root.display()))?
        {
            let entry = entry.map_err(|err| format!("读取旧工作区目录项失败 ({}): {err}", old_root.display()))?;
            remove_workspace_entry_recursive(&entry.path(), &app_handle, &task_id, &mut delete_processed, total)?;
        }
        runtime_log_info(format!(
            "[工作区迁移] 完成: old='{}', new='{}', moved_entries={}",
            old_root.display(),
            new_root.display(),
            total
        ));
        emit_workspace_migration_progress(
            &app_handle,
            &WorkspaceMigrationProgressPayload {
                task_id: task_id.clone(),
                stage: "completed".to_string(),
                processed: total,
                total,
                current_path: Some(new_root.to_string_lossy().to_string()),
                message: "工作区迁移完成".to_string(),
                done: true,
                error: None,
            },
        );
        Ok(shell_workspace_display_path(&new_root))
    })
    .await
    .map_err(|err| format!("工作区迁移任务执行失败: {err}"))?
    .map_err(|err: String| {
        emit_workspace_migration_progress(
            &app,
            &WorkspaceMigrationProgressPayload {
                task_id: input.task_id.trim().to_string(),
                stage: "failed".to_string(),
                processed: 0,
                total: 0,
                current_path: None,
                message: "工作区迁移失败".to_string(),
                done: true,
                error: Some(err.clone()),
            },
        );
        err
    })
}

#[tauri::command]
fn get_chat_shell_workspace(
    input: ChatShellWorkspaceInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(
            &state,
            &input.api_config_id,
            &input.agent_id,
            input.conversation_id.as_deref(),
        )?;
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
        resolve_chat_tool_session_id(
            &state,
            &input.api_config_id,
            &input.agent_id,
            input.conversation_id.as_deref(),
        )?;
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
    let conversation_id = resolve_chat_workspace_conversation_id(
        &state,
        &input.agent_id,
        input.conversation_id.as_deref(),
    )?;
    update_conversation_shell_workspace_path(
        &state,
        &conversation_id,
        Some(target.to_string_lossy().to_string()),
    )?;
    {
        let mut roots = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        roots.remove(&session_id);
    }
    Ok(ChatShellWorkspaceOutput {
        session_id,
        workspace_name: resolve_workspace_display_name(&state, &target),
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
        resolve_chat_tool_session_id(
            &state,
            &input.api_config_id,
            &input.agent_id,
            input.conversation_id.as_deref(),
        )?;
    let conversation_id = resolve_chat_workspace_conversation_id(
        &state,
        &input.agent_id,
        input.conversation_id.as_deref(),
    )?;
    update_conversation_shell_workspace_path(&state, &conversation_id, None)?;
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

#[tauri::command]
fn open_local_file_directory(path: String) -> Result<(), String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let file_path = PathBuf::from(raw_path);

    if file_path.is_file() {
        #[cfg(target_os = "windows")]
        {
            let resolved_path = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
            let explorer_target = resolved_path.to_string_lossy().replace('/', "\\");
            std::process::Command::new("explorer")
                .args(["/select,", explorer_target.as_str()])
                .status()
                .map_err(|err| format!("Failed to open explorer: {err}"))?;
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .args(["-R", raw_path])
                .status()
                .map_err(|err| format!("Failed to open Finder: {err}"))?;
            return Ok(());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some(parent) = file_path.parent() {
                std::process::Command::new("xdg-open")
                    .arg(parent)
                    .status()
                    .map_err(|err| format!("Failed to open file manager: {err}"))?;
            } else {
                std::process::Command::new("xdg-open")
                    .arg(&file_path)
                    .status()
                    .map_err(|err| format!("Failed to open file manager: {err}"))?;
            }
            return Ok(());
        }
    }

    open_shell_path_in_file_manager(&file_path)?;
    Ok(())
}

