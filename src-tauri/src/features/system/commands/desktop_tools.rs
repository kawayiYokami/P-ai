#[tauri::command]
async fn desktop_screenshot(input: ScreenshotRequest) -> Result<ScreenshotResponse, String> {
    run_screenshot_tool(input)
        .await
        .map_err(|err| to_tool_err_string(&err))
}

const NATIVE_NOTIFICATION_BODY_MAX_CHARS: usize = 180;
#[cfg(target_os = "windows")]
const NATIVE_NOTIFICATION_SOUND_DEFAULT: &str = "Default";

fn native_notification_text_excerpt(raw: &str, max_chars: usize) -> String {
    let normalized = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn send_native_notification(
    app: &AppHandle,
    title: &str,
    body: &str,
    play_sound: bool,
) -> Result<(), String> {
    use tauri_plugin_notification::{NotificationExt, PermissionState};

    let normalized_title = title.trim();
    let normalized_body = body.trim();
    if normalized_title.is_empty() {
        return Err("通知标题不能为空".to_string());
    }
    if normalized_body.is_empty() {
        return Err("通知正文不能为空".to_string());
    }

    let notifications = app.notification();
    let permission_before = notifications
        .permission_state()
        .map_err(|err| format!("读取通知权限失败：{err}"))?;
    let permission_after = match permission_before {
        PermissionState::Prompt | PermissionState::PromptWithRationale => notifications
            .request_permission()
            .map_err(|err| format!("请求通知权限失败：{err}"))?,
        state => state,
    };

    if permission_after == PermissionState::Denied {
        return Err("系统通知权限已被拒绝，请先在系统设置里允许通知。".to_string());
    }

    let mut builder = notifications
        .builder()
        .title(normalized_title)
        .body(normalized_body);

    #[cfg(target_os = "windows")]
    {
        if play_sound {
            builder = builder.sound(NATIVE_NOTIFICATION_SOUND_DEFAULT);
        }
    }

    builder
        .show()
        .map_err(|err| format!("发送原生通知失败：{err}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeNotificationDemoResult {
    permission_before: String,
    permission_after: String,
    title: String,
    body: String,
    sent_at: String,
}

#[tauri::command]
fn demo_send_native_notification(app: AppHandle) -> Result<NativeNotificationDemoResult, String> {
    use tauri_plugin_notification::NotificationExt;

    let sent_at = now_local_rfc3339();
    let title = "PAI Demo 原生通知".to_string();
    let body = format!("这是从 Demo 页发出的测试通知。时间：{sent_at}");
    let permission_before = app
        .notification()
        .permission_state()
        .map_err(|err| format!("读取通知权限失败：{err}"))?;
    send_native_notification(&app, &title, &body, true)?;
    let permission_after = app
        .notification()
        .permission_state()
        .map_err(|err| format!("读取通知权限失败：{err}"))?;

    runtime_log_info(format!(
        "[通知Demo] 完成，permission_before={}，permission_after={}，sent_at={}",
        permission_before,
        permission_after,
        sent_at
    ));

    Ok(NativeNotificationDemoResult {
        permission_before: permission_before.to_string(),
        permission_after: permission_after.to_string(),
        title,
        body,
        sent_at,
    })
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
                include_base64: true,
                max_pixels: None,
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
                include_base64: true,
                max_pixels: None,
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
                include_base64: true,
                max_pixels: None,
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
                include_base64: true,
                max_pixels: None,
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
    rg_installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostRuntimePrerequisiteInstallResult {
    kind: String,
    installed: bool,
    message: String,
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

fn host_runtime_prerequisite_installed(kind: &str) -> Result<bool, String> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "git" => {
            if command_exists_in_path("git") {
                return Ok(true);
            }
            #[cfg(target_os = "windows")]
            {
                return Ok([
                    r"C:\Program Files\Git\cmd\git.exe",
                    r"C:\Program Files\Git\bin\git.exe",
                    r"C:\Program Files (x86)\Git\cmd\git.exe",
                    r"C:\Program Files (x86)\Git\bin\git.exe",
                ]
                .iter()
                .any(|path| Path::new(path).is_file()));
            }
            #[cfg(not(target_os = "windows"))]
            {
                Ok(false)
            }
        }
        "node" => {
            if command_exists_in_path("node") {
                return Ok(true);
            }
            #[cfg(target_os = "windows")]
            {
                return Ok([
                    r"C:\Program Files\nodejs\node.exe",
                    r"C:\Program Files (x86)\nodejs\node.exe",
                ]
                .iter()
                .any(|path| Path::new(path).is_file()));
            }
            #[cfg(not(target_os = "windows"))]
            {
                Ok(false)
            }
        }
        "rg" | "ripgrep" => Ok(command_exists_in_path("rg")),
        other => Err(format!("不支持的运行时依赖：{other}")),
    }
}

#[tauri::command]
fn get_host_runtime_prerequisites() -> HostRuntimePrerequisites {
    HostRuntimePrerequisites {
        git_installed: host_runtime_prerequisite_installed("git").unwrap_or(false),
        node_installed: host_runtime_prerequisite_installed("node").unwrap_or(false),
        rg_installed: host_runtime_prerequisite_installed("rg").unwrap_or(false),
    }
}

#[cfg(target_os = "windows")]
fn winget_package_id_for_host_runtime(kind: &str) -> Result<&'static str, String> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "git" => Ok("Git.Git"),
        "node" => Ok("OpenJS.NodeJS.LTS"),
        "rg" | "ripgrep" => Ok("BurntSushi.ripgrep.MSVC"),
        other => Err(format!("不支持的运行时依赖：{other}")),
    }
}

#[cfg(target_os = "windows")]
fn run_winget_host_runtime_install(kind: &str, elevated: bool) -> Result<(), String> {
    let package_id = winget_package_id_for_host_runtime(kind)?;
    let winget_args = [
        "install",
        "--id",
        package_id,
        "-e",
        "--source",
        "winget",
        "--silent",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--disable-interactivity",
    ];

    let status = if elevated {
        let quoted_args = winget_args
            .iter()
            .map(|arg| format!("'{}'", arg.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "$p = Start-Process -FilePath 'winget' -Verb RunAs -Wait -PassThru -ArgumentList @({quoted_args}); if ($null -eq $p) {{ exit 1 }}; exit $p.ExitCode"
        );
        let mut command = std::process::Command::new("powershell");
        command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt as _;
            // PowerShell 是控制台程序，提权安装不能让 GUI 应用弹出控制台窗口。
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        command
            .status()
            .map_err(|err| format!("拉起管理员安装失败：{err}"))?
    } else {
        let mut command = std::process::Command::new("winget");
        command.args(winget_args);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt as _;
            // winget 是控制台程序，后台安装不能让 GUI 应用弹出控制台窗口。
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        command
            .status()
            .map_err(|err| format!("启动 winget 失败：{err}"))?
    };

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "winget 安装退出码：{}",
            status.code().map_or_else(|| "未知".to_string(), |code| code.to_string())
        ))
    }
}

#[cfg(target_os = "windows")]
fn install_host_runtime_prerequisite_sync(
    kind: String,
) -> Result<HostRuntimePrerequisiteInstallResult, String> {
    let normalized = kind.trim().to_ascii_lowercase();
    if host_runtime_prerequisite_installed(&normalized)? {
        return Ok(HostRuntimePrerequisiteInstallResult {
            kind: normalized,
            installed: true,
            message: "已经检测到依赖，无需安装。".to_string(),
        });
    }
    if !command_exists_in_path("winget") {
        return Err("当前系统未检测到 winget，已改为打开官方下载页。".to_string());
    }

    if let Err(first_err) = run_winget_host_runtime_install(&normalized, false) {
        runtime_log_error(format!("[依赖安装] 普通安装失败，准备请求管理员权限，kind={}，error={}", normalized, first_err));
        run_winget_host_runtime_install(&normalized, true)?;
    }

    if host_runtime_prerequisite_installed(&normalized)? {
        Ok(HostRuntimePrerequisiteInstallResult {
            kind: normalized,
            installed: true,
            message: "安装完成。".to_string(),
        })
    } else {
        Err("安装流程结束后仍未检测到依赖，请重启 PAI 或手动安装。".to_string())
    }
}

#[tauri::command]
async fn install_host_runtime_prerequisite(
    kind: String,
) -> Result<HostRuntimePrerequisiteInstallResult, String> {
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(move || install_host_runtime_prerequisite_sync(kind))
            .await
            .map_err(|err| format!("安装任务执行失败：{err}"))?
    }

    #[cfg(not(target_os = "windows"))]
    {
        let normalized = kind.trim().to_ascii_lowercase();
        Err(format!(
            "{normalized} 暂不支持一键安装，请使用系统包管理器或官方下载页安装。"
        ))
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
        match run_command_in_workspace(&state, &session_id, step, &cwd, 15_000, false).await {
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

#[tauri::command]
async fn list_file_reader_directory_open_targets(state: State<'_, AppState>) -> Result<Value, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let options = file_reader_directory_open_targets_for_ui(&app_state);
        Ok(serde_json::json!({
            "options": options,
        }))
    })
    .await
    .map_err(|err| format!("读取目录打开目标任务失败：{err}"))?
}

#[cfg(target_os = "windows")]
fn file_reader_target_icon_data_url(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    windows_extract_executable_icon_data_url(&PathBuf::from(trimmed)).ok()
}

#[cfg(not(target_os = "windows"))]
fn file_reader_target_icon_data_url(_path: &str) -> Option<String> {
    None
}

fn file_reader_directory_open_targets_for_ui(state: &AppState) -> Vec<Value> {
    let (_, _, shell_options) = terminal_shell_candidates_for_ui(state);
    let mut items = Vec::<Value>::new();
    for item in shell_options {
        let kind = item.get("kind").and_then(Value::as_str).unwrap_or("").trim().to_string();
        if kind == "auto" || kind.is_empty() {
            continue;
        }
        let path = item.get("path").and_then(Value::as_str).unwrap_or("").trim().to_string();
        items.push(serde_json::json!({
            "kind": format!("shell:{kind}"),
            "label": item.get("label").and_then(Value::as_str).unwrap_or("Shell"),
            "type": "shell",
            "iconDataUrl": file_reader_target_icon_data_url(&path),
        }));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(wsl_exe) = first_existing_wsl_exe() {
            items.push(serde_json::json!({
                "kind": "shell:wsl",
                "label": "WSL",
                "type": "shell",
                "iconDataUrl": file_reader_target_icon_data_url(&wsl_exe.to_string_lossy()),
            }));
        }

        if let Some(vscode_exe) = first_existing_vscode_exe() {
            items.push(serde_json::json!({
                "kind": "vscode",
                "label": "VS Code",
                "type": "vscode",
                "iconDataUrl": file_reader_target_icon_data_url(&vscode_exe.to_string_lossy()),
            }));
        } else {
            items.push(serde_json::json!({
                "kind": "vscode",
                "label": "VS Code",
                "type": "vscode",
                "iconDataUrl": serde_json::Value::Null,
            }));
        }

        if let Some(explorer_exe) = first_existing_windows_explorer_exe() {
            items.push(serde_json::json!({
                "kind": "explorer",
                "label": "资源管理器",
                "type": "explorer",
                "iconDataUrl": file_reader_target_icon_data_url(&explorer_exe.to_string_lossy()),
            }));
        } else {
            items.push(serde_json::json!({
                "kind": "explorer",
                "label": "资源管理器",
                "type": "explorer",
                "iconDataUrl": serde_json::Value::Null,
            }));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        items.push(serde_json::json!({
            "kind": "vscode",
            "label": "VS Code",
            "type": "vscode",
            "iconDataUrl": serde_json::Value::Null,
        }));
        items.push(serde_json::json!({
            "kind": "explorer",
            "label": "资源管理器",
            "type": "explorer",
            "iconDataUrl": serde_json::Value::Null,
        }));
    }

    let mut deduped = Vec::<Value>::new();
    let mut seen = std::collections::BTreeSet::<String>::new();
    for item in items {
        let kind = item.get("kind").and_then(Value::as_str).unwrap_or("").trim().to_string();
        if kind.is_empty() || !seen.insert(kind) {
            continue;
        }
        deduped.push(item);
    }
    deduped
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveTerminalApprovalInput {
    request_id: String,
    approved: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalApprovalRequestIdInput {
    request_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatShellWorkspaceInput {
    #[serde(default)]
    api_config_id: String,
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveChatShellWorkspacesInput {
    #[serde(default)]
    api_config_id: String,
    #[serde(default)]
    agent_id: String,
    conversation_id: Option<String>,
    #[serde(default)]
    workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default)]
    autonomous_mode: Option<bool>,
    #[serde(default)]
    shell_work_mode: Option<String>,
    #[serde(default)]
    shell_work_branch: Option<String>,
    /// 会话记录的工作分支；传空串表示清空（换工作区后重新记录），不传表示不动
    #[serde(default)]
    shell_recorded_branch: Option<String>,
    /// 会话绑定的工作树路径；传空串表示清空（换分支后交给后端重新推导），不传表示不动
    #[serde(default)]
    shell_worktree_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellWorkspacePathInput {
    #[serde(default)]
    workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitWorkspaceRootCheckOutput {
    is_git_root: bool,
    checked: bool,
    error: Option<String>,
}

// 仅在用户打开工作目录面板时由前端调用；进程存活期间同一路径绝不重复探测。
static GIT_WORKSPACE_ROOT_CHECK_CACHE: OnceLock<
    Mutex<std::collections::HashMap<String, GitWorkspaceRootCheckOutput>>,
> = OnceLock::new();

fn git_workspace_root_check_cache(
) -> &'static Mutex<std::collections::HashMap<String, GitWorkspaceRootCheckOutput>> {
    GIT_WORKSPACE_ROOT_CHECK_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn git_workspace_root_check_cache_lock(
) -> std::sync::MutexGuard<'static, std::collections::HashMap<String, GitWorkspaceRootCheckOutput>> {
    match git_workspace_root_check_cache().lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFileReaderDirectoryTargetInput {
    path: String,
    target_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFileReaderDirectoryShellInput {
    path: String,
    #[serde(default)]
    preferred_kind: Option<String>,
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
    workspaces: Vec<ShellWorkspaceConfig>,
    autonomous_mode: bool,
    shell_work_mode: String,
    shell_work_branch: String,
    shell_recorded_branch: String,
    worktree_path: String,
    worktree_exists: bool,
}

fn resolve_chat_tool_session_id(
    state: &AppState,
    api_config_id: &str,
    agent_id: &str,
    conversation_id: Option<&str>,
) -> Result<String, String> {
    let api_id = api_config_id.trim();
    let agent = agent_id.trim();
    if let Some(conversation_id) = conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let conversation_agent_id = match conversation_service_v2()
            .get_conversation_meta(state, conversation_id)
        {
            Ok(meta) => meta.agent_id,
            Err(primary_error) => match delegate_runtime_thread_conversation_get_any(
                state,
                conversation_id,
            )? {
                Some(conversation) => conversation.agent_id,
                None => return Err(primary_error),
            },
        };
        let conversation_agent_id = conversation_agent_id.trim().to_string();
        if conversation_agent_id.is_empty() {
            return Err(format!("指定会话缺少 Agent 标识：{conversation_id}"));
        }
        return Ok(normalize_terminal_tool_session_id(&inflight_chat_key(
            &conversation_agent_id,
            Some(conversation_id),
        )));
    }
    if api_id.is_empty() {
        return Err("apiConfigId is required.".to_string());
    }
    if agent.is_empty() {
        return Err("agentId is required.".to_string());
    }

    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let config = runtime_snapshot.config;
    let resolved_api_id = resolve_model_role_api_config_id(&config, api_id)
        .ok_or_else(|| format!("Model role '{api_id}' is not configured."))?;
    if !config.api_configs.iter().any(|v| v.id == resolved_api_id) {
        return Err(format!("Selected API config '{api_id}' not found."));
    }
    let agents = runtime_snapshot.agents;
    if !agents.iter().any(|v| v.id == agent && !v.is_built_in_user) {
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
    conversation_service_v2()
        .resolve_latest_foreground_conversation_id(state, agent_id)
        .and_then(|value| {
            value.ok_or_else(|| "当前没有可用的活跃会话，需要提供 conversationId。".to_string())
        })
}

fn apply_conversation_chat_workspace_changes(
    state: &AppState,
    conversation_id: &str,
    shell_workspace_path: Option<Option<String>>,
    shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    shell_autonomous_mode: Option<bool>,
    shell_work_mode: Option<String>,
    shell_work_branch: Option<String>,
    shell_recorded_branch: Option<String>,
    shell_worktree_path: Option<String>,
) -> Result<Conversation, String> {
    if delegate_runtime_thread_conversation_get(state, conversation_id)?.is_some() {
        let next_path = shell_workspace_path.clone();
        let next_workspaces = shell_workspaces.clone();
        let next_branch = shell_work_branch.clone();
        let next_recorded_branch = shell_recorded_branch.clone();
        let next_worktree_path = shell_worktree_path.clone();
        delegate_runtime_thread_modify(state, conversation_id, move |thread| {
            let original_path = thread.conversation.shell_workspace_path.clone();
            let original_workspaces = thread.conversation.shell_workspaces.clone();
            let original_autonomous_mode = thread.conversation.shell_autonomous_mode;
            let original_work_mode = thread.conversation.shell_work_mode.clone();
            let original_branch = thread.conversation.shell_work_branch.clone();
            let original_recorded_branch = thread.conversation.shell_recorded_branch.clone();
            let original_worktree_path = thread.conversation.shell_worktree_path.clone();
            if let Some(value) = next_path.clone() {
                thread.conversation.shell_workspace_path = value;
            }
            if let Some(value) = next_workspaces.clone() {
                thread.conversation.shell_workspaces = value;
            }
            if let Some(value) = shell_autonomous_mode {
                thread.conversation.shell_autonomous_mode = value;
            }
            if let Some(value) = shell_work_mode.clone() {
                thread.conversation.shell_work_mode = normalize_shell_work_mode_text(&value);
            }
            if let Some(value) = next_branch.clone() {
                thread.conversation.shell_work_branch = normalize_shell_work_branch_text(&value);
            }
            if let Some(value) = next_recorded_branch.clone() {
                thread.conversation.shell_recorded_branch =
                    normalize_shell_work_branch_text(&value);
            }
            if let Some(value) = next_worktree_path.clone() {
                thread.conversation.shell_worktree_path = value.trim().to_string();
            }
            if thread.conversation.shell_workspace_path.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some()
                && terminal_workspace_path_from_conversation(state, &thread.conversation).is_none()
            {
                thread.conversation.shell_workspace_path = None;
            }
            if thread.conversation.shell_workspace_path == original_path
                && thread.conversation.shell_workspaces == original_workspaces
                && thread.conversation.shell_autonomous_mode == original_autonomous_mode
                && thread.conversation.shell_work_mode == original_work_mode
                && thread.conversation.shell_work_branch == original_branch
                && thread.conversation.shell_recorded_branch == original_recorded_branch
                && thread.conversation.shell_worktree_path == original_worktree_path
            {
                return Ok(());
            }
            mark_prompt_cache_rebuild_for_system_environment_by_conversation(
                state,
                conversation_id,
            );
            Ok(())
        })?;
        return delegate_runtime_thread_conversation_get_any(state, conversation_id)?
            .ok_or_else(|| format!("指定会话不存在：{conversation_id}"));
    }

    let updated = conversation_service_v2().update_shell_workspace(
        state,
        conversation_id,
        shell_workspace_path,
        shell_workspaces,
        shell_autonomous_mode,
        shell_work_mode,
        shell_work_branch,
        shell_recorded_branch,
        shell_worktree_path,
    )?;
    mark_prompt_cache_rebuild_for_system_environment_by_conversation(state, conversation_id);
    Ok(updated)
}

fn workspace_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|v| v.to_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn resolve_workspace_display_name_for_conversation(
    state: &AppState,
    conversation: Option<&Conversation>,
    root: &Path,
) -> String {
    let root_key = normalize_terminal_path_for_compare(root);
    if let Ok(workspaces) = terminal_allowed_workspaces_for_conversation_canonical(state, conversation) {
        for ws in workspaces {
            if normalize_terminal_path_for_compare(&ws.path) == root_key {
                return ws.name;
            }
        }
    }
    workspace_name_from_path(root)
}

fn build_chat_shell_workspace_list(
    state: &AppState,
    conversation: Option<&Conversation>,
) -> Vec<ShellWorkspaceConfig> {
    terminal_allowed_workspaces_for_conversation_canonical(state, conversation)
        .unwrap_or_default()
        .into_iter()
        .map(|workspace| ShellWorkspaceConfig {
            id: workspace.id,
            name: workspace.name,
            path: workspace.path.to_string_lossy().to_string(),
            level: workspace.level,
            access: workspace.access,
            built_in: workspace.built_in,
        })
        .collect()
}

fn resolve_chat_shell_worktree_info(
    conversation: Option<&Conversation>,
    root: &Path,
) -> (String, bool) {
    let Some(conv) = conversation else {
        return (String::new(), false);
    };
    if normalize_shell_work_mode_text(&conv.shell_work_mode) != SHELL_WORK_MODE_WORKTREE {
        return (String::new(), false);
    }
    let git_root = root.to_string_lossy().to_string();
    if git_root.trim().is_empty() {
        return (String::new(), false);
    }
    // 会话字段优先（须落在当前仓库的 .pai/.worktree/ 下）
    if let Some(recorded) = conversation_recorded_worktree_path(conv, &git_root) {
        return (
            recorded.to_string_lossy().to_string(),
            conversation_worktree_dir_is_valid(&recorded),
        );
    }
    // 字段为空：在 .pai/.worktree/ 下推导已有目录（新规则通配 → 全量 id → 8 位）
    if let Some(existing) = resolve_existing_conversation_worktree_dir(&git_root, &conv.id) {
        return (existing.to_string_lossy().to_string(), true);
    }
    // 未创建时仍返回将要使用的路径供前端作意图展示，但标记不存在
    let planned = PathBuf::from(root)
        .join(".pai")
        .join(".worktree")
        .join(conversation_worktree_dir_name(&conv.id));
    (planned.to_string_lossy().to_string(), false)
}

fn build_chat_shell_workspace_output(
    state: &AppState,
    session_id: String,
    conversation: Option<&Conversation>,
    root: PathBuf,
) -> ChatShellWorkspaceOutput {
    let (worktree_path, worktree_exists) = resolve_chat_shell_worktree_info(conversation, &root);
    ChatShellWorkspaceOutput {
        session_id,
        workspace_name: resolve_workspace_display_name_for_conversation(state, conversation, &root),
        root_path: root.to_string_lossy().to_string(),
        workspaces: build_chat_shell_workspace_list(state, conversation),
        autonomous_mode: conversation.map(|value| value.shell_autonomous_mode).unwrap_or(false),
        shell_work_mode: conversation
            .map(|value| normalize_shell_work_mode_text(&value.shell_work_mode))
            .unwrap_or_else(default_shell_work_mode),
        shell_work_branch: conversation
            .map(|value| normalize_shell_work_branch_text(&value.shell_work_branch))
            .unwrap_or_default(),
        shell_recorded_branch: conversation
            .map(|value| normalize_shell_work_branch_text(&value.shell_recorded_branch))
            .unwrap_or_default(),
        worktree_path,
        worktree_exists,
    }
}

fn open_shell_path_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let explorer_path = resolved.to_string_lossy().replace('/', "\\");
        std::process::Command::new("explorer")
            .arg(explorer_path.as_str())
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

#[cfg(target_os = "windows")]
fn first_existing_vscode_exe() -> Option<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Programs").join("Microsoft VS Code").join("Code.exe"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Microsoft VS Code").join("Code.exe"));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(program_files_x86).join("Microsoft VS Code").join("Code.exe"));
    }
    if let Some(path_value) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_value) {
            candidates.push(dir.join("Code.exe"));
            candidates.push(dir.join("..").join("Code.exe"));
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn first_existing_wsl_exe() -> Option<PathBuf> {
    let mut candidates = Vec::<PathBuf>::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        candidates.push(PathBuf::from(&windir).join("System32").join("wsl.exe"));
        candidates.push(PathBuf::from(&windir).join("Sysnative").join("wsl.exe"));
    }
    if let Some(path_value) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_value) {
            candidates.push(dir.join("wsl.exe"));
        }
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn first_existing_windows_explorer_exe() -> Option<PathBuf> {
    if let Ok(windir) = std::env::var("WINDIR") {
        let candidate = PathBuf::from(windir).join("explorer.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_path_to_wsl_directory(path: &Path) -> Option<String> {
    let normalized = terminal_strip_windows_verbatim_prefix(&path.to_string_lossy()).replace('\\', "/");
    if normalized.starts_with("//") {
        return None;
    }
    let bytes = normalized.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' {
        return None;
    }
    let drive = normalized[..1].to_ascii_lowercase();
    let tail = normalized[2..].trim_start_matches('/');
    if tail.is_empty() {
        return Some(format!("/mnt/{drive}"));
    }
    Some(format!("/mnt/{drive}/{tail}"))
}

#[cfg(target_os = "windows")]
fn windows_os_str_to_wide_null(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn windows_extract_executable_icon_data_url(path: &Path) -> Result<String, String> {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
    };
    use windows_sys::Win32::UI::Shell::ExtractIconExW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let wide_path = windows_os_str_to_wide_null(path);
    let mut large_icon = [std::ptr::null_mut(); 1];
    let mut icon_info = ICONINFO {
        fIcon: 0,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: std::ptr::null_mut(),
        hbmColor: std::ptr::null_mut(),
    };
    let mut screen_dc = std::ptr::null_mut();
    let mut memory_dc = std::ptr::null_mut();

    let result = unsafe {
        let extracted = ExtractIconExW(wide_path.as_ptr(), 0, large_icon.as_mut_ptr(), std::ptr::null_mut(), 1);
        if extracted == 0 || large_icon[0].is_null() {
            return Err(format!("提取图标失败：{}", path.display()));
        }
        let icon = large_icon[0];
        let inner = (|| -> Result<String, String> {
            if GetIconInfo(icon, &mut icon_info) == 0 {
                return Err(format!("读取图标信息失败：{}", path.display()));
            }
            let bitmap_handle = if !icon_info.hbmColor.is_null() { icon_info.hbmColor } else { icon_info.hbmMask };
            if bitmap_handle.is_null() {
                return Err(format!("图标位图不存在：{}", path.display()));
            }

            let mut bitmap = std::mem::zeroed::<BITMAP>();
            if GetObjectW(
                bitmap_handle,
                std::mem::size_of::<BITMAP>() as i32,
                &mut bitmap as *mut _ as *mut std::ffi::c_void,
            ) == 0
            {
                return Err(format!("读取图标位图失败：{}", path.display()));
            }

            let width = bitmap.bmWidth.unsigned_abs();
            let mut height = bitmap.bmHeight.unsigned_abs();
            if icon_info.hbmColor.is_null() && height > 1 {
                height /= 2;
            }
            if width == 0 || height == 0 {
                return Err(format!("图标尺寸无效：{}", path.display()));
            }

            screen_dc = GetDC(std::ptr::null_mut());
            if screen_dc.is_null() {
                return Err("获取屏幕设备上下文失败。".to_string());
            }
            memory_dc = CreateCompatibleDC(screen_dc);
            if memory_dc.is_null() {
                return Err("创建内存设备上下文失败。".to_string());
            }

            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [std::mem::zeroed()],
            };
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            let copied = GetDIBits(
                memory_dc,
                bitmap_handle,
                0,
                height,
                pixels.as_mut_ptr() as *mut std::ffi::c_void,
                &mut bitmap_info,
                DIB_RGB_COLORS,
            );
            if copied == 0 {
                return Err(format!("读取图标像素失败：{}", path.display()));
            }

            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                if icon_info.hbmColor.is_null() {
                    pixel[3] = 255;
                }
            }

            let image = image::RgbaImage::from_raw(width, height, pixels)
                .ok_or_else(|| format!("组装图标像素失败：{}", path.display()))?;
            let mut encoded = Vec::<u8>::new();
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
                .map_err(|err| format!("编码图标 PNG 失败：{err}"))?;
            Ok(format!("data:image/png;base64,{}", B64.encode(encoded)))
        })();

        if !memory_dc.is_null() {
            DeleteDC(memory_dc);
        }
        if !screen_dc.is_null() {
            ReleaseDC(std::ptr::null_mut(), screen_dc);
        }
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask);
        }
        DestroyIcon(icon);
        inner
    };

    result
}

/// 用 VS Code 打开一个路径（文件或目录）：平台差异只在这里处理一次。
/// 目录版与文件版都复用它，避免「同一套启动逻辑写两遍」再次漏掉平台 cfg。
fn launch_in_vscode(target: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let normalized = terminal_path_for_user(target);
        let vscode_exe = first_existing_vscode_exe()
            .ok_or_else(|| "未检测到 VS Code 可执行文件。".to_string())?;
        std::process::Command::new(vscode_exe)
            .arg(normalized.as_str())
            .spawn()
            .map_err(|err| format!("打开 VS Code 失败：{err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Visual Studio Code"])
            .arg(target)
            .spawn()
            .map_err(|err| format!("打开 VS Code 失败：{err}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("code")
            .arg(target)
            .spawn()
            .map_err(|err| format!("打开 VS Code 失败：{err}"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("当前平台不支持打开 VS Code。".to_string())
}

fn open_directory_in_vscode(path: &Path) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|err| format!("解析目录失败 ({}): {err}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("不是目录：{}", canonical.display()));
    }
    launch_in_vscode(&canonical)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenFileInVscodeInput {
    path: String,
}

/// 在 VS Code 中打开文件。
/// 与目录版共用 `launch_in_vscode`，这里只多一层「必须是文件」的校验。
#[tauri::command]
fn open_file_in_vscode(input: OpenFileInVscodeInput) -> Result<(), String> {
    let raw_path = input.path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let file_path = PathBuf::from(raw_path);
    if !file_path.is_file() {
        return Err(format!("不是文件：{raw_path}"));
    }
    let canonical = file_path
        .canonicalize()
        .map_err(|err| format!("解析文件路径失败（{raw_path}）：{err}"))?;
    launch_in_vscode(&canonical)
}

/// 通用「另存为」：系统保存对话框选目标，再把文件复制过去。
/// 图片版另有 `save_local_chat_image_as`，本命令面向任意本地文件。
#[tauri::command]
async fn save_local_file_as(path: String, app: AppHandle) -> Result<Value, String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let source_path = PathBuf::from(raw_path);
    if !source_path.is_file() {
        return Err(format!("文件不存在：{raw_path}"));
    }
    let file_name = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_string();
    let (dialog_tx, dialog_rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(file_name)
        .save_file(move |file| {
            let _ = dialog_tx.send(file);
        });
    let dest = dialog_rx
        .await
        .map_err(|err| format!("等待保存对话框结果失败：{err}"))?;
    let dest_path = match dest.and_then(|fp| fp.as_path().map(ToOwned::to_owned)) {
        Some(value) => value,
        // 用户取消不是失败：前端只需静默收尾，不要弹错误提示。
        None => return Ok(serde_json::json!({ "ok": false, "cancelled": true })),
    };
    // 保存到原路径时不需要复制。
    if dest_path == source_path || dest_path.canonicalize().ok() == source_path.canonicalize().ok() {
        return Ok(serde_json::json!({ "ok": true }));
    }
    tokio::task::spawn_blocking(move || std::fs::copy(&source_path, &dest_path))
        .await
        .map_err(|err| format!("复制文件任务异常：{err}"))?
        .map_err(|err| format!("复制文件失败：{err}"))?;
    Ok(serde_json::json!({ "ok": true }))
}

fn open_shell_terminal_at_path(state: &AppState, path: &Path, preferred_kind: Option<&str>) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|err| format!("解析目录失败 ({}): {err}", path.display()))?;
    if !canonical.is_dir() {
        return Err(format!("不是目录：{}", canonical.display()));
    }

    #[cfg(target_os = "windows")]
    {
        if matches!(preferred_kind.map(str::trim), Some("wsl")) {
            let wsl_exe = first_existing_wsl_exe()
                .ok_or_else(|| "未检测到 WSL 可执行文件。".to_string())?;
            let wsl_cwd = windows_path_to_wsl_directory(&canonical)
                .ok_or_else(|| format!("当前目录无法转换为 WSL 路径：{}", canonical.display()))?;
            let wsl_path = terminal_powershell_escape_literal(&wsl_exe.to_string_lossy());
            let wsl_cwd_text = terminal_powershell_escape_literal(&wsl_cwd);
            let script = format!(
                "Start-Process -FilePath '{wsl_path}' -ArgumentList @('--cd','{wsl_cwd_text}') -Verb Open -PassThru | Out-Null"
            );
            let mut command = std::process::Command::new("powershell");
            command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
            terminal_apply_windows_utf8_env(&mut command);
            // powershell 只是启动器，自身不能弹多余控制台；WSL 窗口由 Start-Process 正常弹出。
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt as _;
                command.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            command
                .spawn()
                .map_err(|err| format!("打开 WSL 失败：{err}"))?;
            return Ok(());
        }
    }

    let shell = preferred_kind
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| terminal_shell_from_candidates(&state.terminal_shell_candidates, value))
        .unwrap_or_else(|| terminal_shell_for_state(state));
    if shell.kind == "missing-terminal-shell" || shell.path.trim().is_empty() {
        return Err("未检测到可用 Shell，请先在设置中配置终端 Shell。".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let cwd = terminal_strip_windows_verbatim_prefix(&canonical.to_string_lossy());
        let title = terminal_powershell_escape_literal("PAI Shell");
        let shell_path = terminal_powershell_escape_literal(&shell.path);
        let cwd_text = terminal_powershell_escape_literal(&cwd);
        let args = if shell.kind == "git-bash" {
            format!("@('--login','-i')")
        } else if matches!(shell.kind.as_str(), "powershell7" | "powershell5") {
            format!("@('-NoLogo','-NoExit','-Command','Set-Location -LiteralPath ''{cwd_text}''')")
        } else {
            "@()".to_string()
        };
        let script = format!(
            "Start-Process -FilePath '{shell_path}' -WorkingDirectory '{cwd_text}' -WindowStyle Normal -ArgumentList {args} -Verb Open -PassThru | Out-Null; $host.UI.RawUI.WindowTitle = '{title}'"
        );
        let mut command = std::process::Command::new("powershell");
        command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
        terminal_apply_windows_utf8_env(&mut command);
        // powershell 只是启动器，自身不能弹多余控制台；Shell 窗口由 Start-Process 正常弹出。
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        command
            .spawn()
            .map_err(|err| format!("打开 Shell 失败：{err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        let cwd = canonical.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("tell application \"Terminal\" to do script \"cd \\\"{cwd}\\\" && exec \\\"{}\\\"\"", shell.path.replace('"', "\\\""));
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|err| format!("打开 Shell 失败：{err}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xfce4-terminal", "xterm"] {
            let mut command = std::process::Command::new(terminal);
            command.current_dir(&canonical);
            if terminal == "xterm" {
                command.args(["-e", &shell.path]);
            } else {
                command.args(["--", &shell.path]);
            }
            if command.spawn().is_ok() {
                return Ok(());
            }
        }
        return Err("未检测到可用图形终端。".to_string());
    }

    #[allow(unreachable_code)]
    Err("当前平台不支持打开 Shell。".to_string())
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
async fn check_git_workspace_root(
    input: ShellWorkspacePathInput,
) -> Result<GitWorkspaceRootCheckOutput, String> {
    let raw_path = input
        .workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "workspacePath is required".to_string())?;
    let workspace_path = PathBuf::from(raw_path);
    let canonical_workspace = workspace_path
        .canonicalize()
        .map_err(|err| format!("无法读取目录：{err}"))?;
    let cache_key = normalize_terminal_path_for_compare(&canonical_workspace);
    if let Some(cached) = git_workspace_root_check_cache_lock().get(&cache_key).cloned() {
        return Ok(cached);
    }
    let mut command = tokio::process::Command::new("git");
    command
        .current_dir(&canonical_workspace)
        .args(["rev-parse", "--show-toplevel"]);
    #[cfg(target_os = "windows")]
    {
        // 避免 Git 进程在 GUI 应用中创建短暂可见的控制台窗口。
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let result = match tokio::time::timeout(std::time::Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) if !output.status.success() => GitWorkspaceRootCheckOutput {
            is_git_root: false,
            checked: true,
            error: None,
        },
        Ok(Ok(output)) => {
            let reported_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let canonical_root = PathBuf::from(reported_root)
                .canonicalize()
                .map_err(|err| format!("无法解析 Git 根目录：{err}"))?;
            GitWorkspaceRootCheckOutput {
                is_git_root: normalize_terminal_path_for_compare(&canonical_root) == cache_key,
                checked: true,
                error: None,
            }
        }
        Ok(Err(err)) => GitWorkspaceRootCheckOutput {
            is_git_root: false,
            checked: false,
            error: Some(format!("无法运行 Git：{err}")),
        },
        Err(_) => GitWorkspaceRootCheckOutput {
            is_git_root: false,
            checked: false,
            error: Some("Git 仓库检测超时".to_string()),
        },
    };
    git_workspace_root_check_cache_lock().insert(cache_key, result.clone());
    Ok(result)
}

#[tauri::command]
fn get_chat_shell_workspace(
    input: ChatShellWorkspaceInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    get_chat_shell_workspace_inner(input, &state)
}

fn get_chat_shell_workspace_inner(
    input: ChatShellWorkspaceInput,
    state: &AppState,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(
            state,
            &input.api_config_id,
            &input.agent_id,
            input.conversation_id.as_deref(),
        )?;
    let conversation = terminal_session_conversation_meta(state, &session_id)?;
    let root = terminal_session_root_canonical(state, &session_id)?;
    Ok(build_chat_shell_workspace_output(
        state,
        session_id,
        conversation.as_ref(),
        root,
    ))
}

#[tauri::command]
fn update_chat_shell_workspace_layout(
    input: SaveChatShellWorkspacesInput,
    state: State<'_, AppState>,
) -> Result<ChatShellWorkspaceOutput, String> {
    update_chat_shell_workspace_layout_inner(input, &state)
}

fn update_chat_shell_workspace_layout_inner(
    input: SaveChatShellWorkspacesInput,
    state: &AppState,
) -> Result<ChatShellWorkspaceOutput, String> {
    let session_id =
        resolve_chat_tool_session_id(
            state,
            &input.api_config_id,
            &input.agent_id,
            input.conversation_id.as_deref(),
        )?;
    let conversation_id = resolve_chat_workspace_conversation_id(
        state,
        &input.agent_id,
        input.conversation_id.as_deref(),
    )?;
    let normalized_workspaces = normalize_conversation_shell_workspaces(state, &input.workspaces);
    let normalized_branch = input
        .shell_work_branch
        .as_deref()
        .map(|value| normalize_shell_work_branch_text(value))
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    // 空串是有效输入：换工作区后要把记录清空，交给下一次状态回填重新记录
    let normalized_recorded_branch = input
        .shell_recorded_branch
        .as_deref()
        .map(|value| normalize_shell_work_branch_text(value));
    // 空串是有效输入：换分支后要清空工作树绑定，交给后端重新推导
    let normalized_worktree_path = input
        .shell_worktree_path
        .as_deref()
        .map(|value| value.trim().to_string());
    // 选中了工作树路径时，探测该路径的真实当前分支写入 shell_recorded_branch——
    // 系统提示词读 meta 展示「在哪个工作树、什么分支」，不在注入时再探测。
    // 探测失败（非 git / git 不可用）时保留前端传入值，不阻断保存。
    let normalized_recorded_branch = match normalized_worktree_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(worktree_path) => {
            let probed = std::process::Command::new("git")
                .args(["branch", "--show-current"])
                .current_dir(worktree_path)
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_default();
            if !probed.is_empty() {
                Some(normalize_shell_work_branch_text(&probed))
            } else {
                normalized_recorded_branch
            }
        }
        None => normalized_recorded_branch,
    };
    let updated = apply_conversation_chat_workspace_changes(
        state,
        &conversation_id,
        Some(None),
        Some(normalized_workspaces),
        input.autonomous_mode,
        input.shell_work_mode,
        normalized_branch,
        normalized_recorded_branch,
        normalized_worktree_path,
    )?;
    {
        let mut roots = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        roots.remove(&session_id);
    }
    let root = terminal_session_root_canonical(state, &session_id)?;
    Ok(build_chat_shell_workspace_output(
        state,
        session_id,
        Some(&updated),
        root,
    ))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordConversationWorkspaceBranchInput {
    conversation_id: String,
    /// 要记录的会话工作分支；空串表示清空记录
    #[serde(default)]
    branch: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordConversationWorkspaceBranchOutput {
    conversation_id: String,
    shell_recorded_branch: String,
}

/// 复写会话记录的「本会话工作分支」。只改会话元数据，不碰仓库。
fn record_conversation_workspace_branch_inner(
    input: RecordConversationWorkspaceBranchInput,
    state: &AppState,
) -> Result<RecordConversationWorkspaceBranchOutput, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("指定会话不存在：".to_string());
    }
    let branch = normalize_shell_work_branch_text(&input.branch);
    let updated = apply_conversation_chat_workspace_changes(
        state,
        &conversation_id,
        None,
        None,
        None,
        None,
        None,
        Some(branch.clone()),
        None,
    )?;
    Ok(RecordConversationWorkspaceBranchOutput {
        conversation_id: updated.id,
        shell_recorded_branch: normalize_shell_work_branch_text(&updated.shell_recorded_branch),
    })
}

#[tauri::command]
fn resolve_terminal_approval(
    input: ResolveTerminalApprovalInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = resolve_terminal_approval_request(&state, &input.request_id, input.approved, input.reason.as_deref())?;
    Ok(())
}

#[tauri::command]
fn approve_terminal_approval_for_session(
    input: TerminalApprovalRequestIdInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = approve_terminal_approval_for_session_request(&state, &input.request_id)?;
    Ok(())
}

#[tauri::command]
fn approve_terminal_approval_for_workspace(
    input: TerminalApprovalRequestIdInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = approve_terminal_approval_for_workspace_request(&state, &input.request_id)?;
    Ok(())
}

#[tauri::command]
fn open_file_reader_window_command(app: AppHandle, path: String) -> Result<String, String> {
    open_file_reader_window(&app, path)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderFilePayload {
    path: String,
    name: String,
    extension: String,
    kind: String,
    content: String,
    force_plain: bool,
    virtualized: bool,
    total_lines: usize,
    block_line_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderFileBlockPayload {
    path: String,
    start_line: usize,
    end_line: usize,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderDirectoryEntry {
    path: String,
    name: String,
    is_directory: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderDirectoryPayload {
    path: String,
    name: String,
    entries: Vec<FileReaderDirectoryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderWatchTargetInput {
    path: String,
    kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderWatchTargetsInput {
    session_id: String,
    targets: Vec<FileReaderWatchTargetInput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReaderWatchEventPayload {
    session_id: String,
    path: String,
    kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileReaderWatchFingerprint {
    exists: bool,
    is_dir: bool,
    len: u64,
    modified_ms: u128,
    directory_signature: String,
}

#[derive(Debug, Clone)]
struct FileReaderWatchTrackedTarget {
    path: String,
    kind: String,
    fingerprint: FileReaderWatchFingerprint,
}

static FILE_READER_WATCH_TARGETS: OnceLock<
    Mutex<std::collections::HashMap<String, Vec<FileReaderWatchTrackedTarget>>>,
> = OnceLock::new();
static FILE_READER_WATCH_THREAD_STARTED: OnceLock<()> = OnceLock::new();

const FILE_READER_PLAIN_TEXT_THRESHOLD: u64 = 2 * 1024 * 1024;
const FILE_READER_LINE_TRUNCATE_CHARS: usize = 10000;
const FILE_READER_BLOCK_LINE_COUNT: usize = 120;
const FILE_READER_READ_BURST_WINDOW_MS: u64 = 100;
const FILE_READER_WATCH_POLL_MS: u64 = 700;
const FILE_READER_WATCH_MAX_TARGETS: usize = 160;
const FILE_READER_WATCH_DIRECTORY_SIGNATURE_LIMIT: usize = 100;

#[derive(Debug, Clone)]
struct FileReaderReadTrace {
    at: std::time::Instant,
    window_label: String,
    path: String,
}

static FILE_READER_READ_TRACES: OnceLock<Mutex<Vec<FileReaderReadTrace>>> = OnceLock::new();

fn file_reader_file_kind(extension: &str) -> &'static str {
    match extension {
        "md" | "markdown" | "mdx" => "markdown",
        _ => "code",
    }
}

fn file_reader_extension_is_unsupported(extension: &str) -> bool {
    // 注意：不要把 "ts" 放进黑名单——TypeScript 源码也是 .ts；
    // MPEG-TS 请用更具体的 mts/m2ts。
    matches!(
        extension,
        "3g2" | "3gp" | "7z" | "a" | "aac" | "accdb" | "aif" | "aiff" | "amr" | "ape"
            | "apk" | "app" | "arrow" | "asf" | "avi" | "avif" | "bin" | "bmp" | "br"
            | "bz2" | "caf" | "class" | "com" | "dat" | "db" | "db3" | "dbf" | "deb"
            | "dll" | "dmg" | "doc" | "docx" | "duckdb" | "dylib" | "eot" | "exe"
            | "feather" | "flac" | "flv" | "gif" | "gz" | "heic" | "heif" | "ico" | "img"
            | "ipa" | "iso" | "jar" | "jpg" | "jpeg" | "lib" | "m2ts" | "m4a" | "mdb"
            | "mid" | "midi" | "mkv" | "mov" | "mp3" | "mp4" | "mpeg" | "mpg" | "msi"
            | "mts" | "o" | "obj" | "odp" | "ods" | "odt" | "oga" | "ogg" | "opus"
            | "orc" | "otf" | "pak" | "parquet" | "pdf" | "pfx" | "pkg" | "png" | "ppt"
            | "pptx" | "psd" | "pyc" | "pyo" | "rar" | "raw" | "rm" | "rmvb" | "rpm"
            | "rtf" | "scr" | "so" | "sqlite" | "sqlite3" | "svg" | "sys" | "tar" | "tgz"
            | "tif" | "tiff" | "ttf" | "vob" | "war" | "wasm" | "wav" | "weba"
            | "webm" | "webp" | "wma" | "wmv" | "woff" | "woff2" | "xls" | "xlsx" | "xz"
            | "zip" | "zst"
    )
}

fn file_reader_watch_targets_store(
) -> &'static Mutex<std::collections::HashMap<String, Vec<FileReaderWatchTrackedTarget>>> {
    FILE_READER_WATCH_TARGETS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn file_reader_watch_modified_ms(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn file_reader_watch_fingerprint(path: &str) -> FileReaderWatchFingerprint {
    let path_buf = PathBuf::from(path);
    let metadata = match fs::metadata(&path_buf) {
        Ok(value) => value,
        Err(_) => {
            return FileReaderWatchFingerprint {
                exists: false,
                is_dir: false,
                len: 0,
                modified_ms: 0,
                directory_signature: String::new(),
            };
        }
    };
    let is_dir = metadata.is_dir();
    let directory_signature = if is_dir {
        file_reader_watch_directory_signature(&path_buf)
    } else {
        String::new()
    };
    FileReaderWatchFingerprint {
        exists: true,
        is_dir,
        len: metadata.len(),
        modified_ms: file_reader_watch_modified_ms(&metadata),
        directory_signature,
    }
}

fn is_file_reader_watch_git_internal_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_lowercase();
    normalized.ends_with("/.git") || normalized.contains("/.git/")
}

fn file_reader_watch_directory_signature(path: &PathBuf) -> String {
    let read_dir = match fs::read_dir(path) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    let mut entries = Vec::new();
    for entry in read_dir.take(FILE_READER_WATCH_DIRECTORY_SIGNATURE_LIMIT) {
        let Ok(entry) = entry else { continue };
        let file_name = entry.file_name();
        let file_name_lossy = file_name.to_string_lossy();
        if file_name_lossy.eq_ignore_ascii_case(".git") {
            continue;
        }
        let entry_path = entry.path();
        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        entries.push(format!(
            "{}|{}|{}|{}",
            file_name_lossy,
            if metadata.is_dir() { "d" } else { "f" },
            metadata.len(),
            file_reader_watch_modified_ms(&metadata)
        ));
        if entry_path.as_os_str().is_empty() {
            continue;
        }
    }
    entries.sort();
    entries.join("\n")
}

fn file_reader_watch_first_directory_diff(old_sig: &str, new_sig: &str) -> Option<String> {
    if old_sig == new_sig {
        return None;
    }
    let old_lines: Vec<&str> = if old_sig.is_empty() {
        Vec::new()
    } else {
        old_sig.split('\n').collect()
    };
    let new_lines: Vec<&str> = if new_sig.is_empty() {
        Vec::new()
    } else {
        new_sig.split('\n').collect()
    };
    let old_set: std::collections::HashSet<&str> = old_lines.iter().copied().collect();
    for line in &new_lines {
        if !old_set.contains(line) {
            return Some(format!("新增/变更条目={}", line));
        }
    }
    let new_set: std::collections::HashSet<&str> = new_lines.iter().copied().collect();
    for line in &old_lines {
        if !new_set.contains(line) {
            return Some(format!("删除/变更条目={}", line));
        }
    }
    Some(format!(
        "签名长度 {}->{}",
        old_sig.len(),
        new_sig.len()
    ))
}

fn start_file_reader_watch_polling(app: AppHandle) {
    let _ = FILE_READER_WATCH_THREAD_STARTED.get_or_init(|| {
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(FILE_READER_WATCH_POLL_MS));
            let (events, first_changed_log) = {
                let store = file_reader_watch_targets_store();
                let mut targets_by_session =
                    store.lock().unwrap_or_else(|poison| poison.into_inner());
                let mut events = Vec::new();
                let mut first_changed_log: Option<String> = None;
                for (session_id, targets) in targets_by_session.iter_mut() {
                    for target in targets.iter_mut() {
                        let next = file_reader_watch_fingerprint(&target.path);
                        if next == target.fingerprint {
                            continue;
                        }
                        if first_changed_log.is_none() {
                            let detail = if target.fingerprint.is_dir || next.is_dir {
                                let diff = file_reader_watch_first_directory_diff(
                                    &target.fingerprint.directory_signature,
                                    &next.directory_signature,
                                );
                                format!(
                                    "session={} path={} kind={} exists {}->{} is_dir {}->{} len {}->{} modified_ms {}->{} signature_len {}->{} {}",
                                    session_id,
                                    target.path,
                                    target.kind,
                                    target.fingerprint.exists,
                                    next.exists,
                                    target.fingerprint.is_dir,
                                    next.is_dir,
                                    target.fingerprint.len,
                                    next.len,
                                    target.fingerprint.modified_ms,
                                    next.modified_ms,
                                    target.fingerprint.directory_signature.len(),
                                    next.directory_signature.len(),
                                    diff.unwrap_or_else(|| "无条目差异".to_string())
                                )
                            } else {
                                format!(
                                    "session={} path={} kind={} exists {}->{} len {}->{} modified_ms {}->{}",
                                    session_id,
                                    target.path,
                                    target.kind,
                                    target.fingerprint.exists,
                                    next.exists,
                                    target.fingerprint.len,
                                    next.len,
                                    target.fingerprint.modified_ms,
                                    next.modified_ms
                                )
                            };
                            first_changed_log = Some(detail);
                        }
                        target.fingerprint = next;
                        events.push(FileReaderWatchEventPayload {
                            session_id: session_id.clone(),
                            path: target.path.clone(),
                            kind: target.kind.clone(),
                        });
                    }
                }
                (events, first_changed_log)
            };
            if let Some(detail) = first_changed_log {
                runtime_log_debug(format!(
                    "[文件管理器] 检测到变更 {} 本次批量数={}",
                    detail,
                    events.len()
                ));
            }
            for payload in events {
                let _ = app.emit("easy-call:file-reader-watch-changed", payload);
            }
        });
    });
}

#[tauri::command]
fn update_file_reader_watch_targets(
    app: AppHandle,
    input: FileReaderWatchTargetsInput,
) -> Result<(), String> {
    let session_id = input.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err("sessionId is required".to_string());
    }
    start_file_reader_watch_polling(app);
    let mut seen = std::collections::HashSet::new();
    let mut targets = Vec::new();
    for target in input.targets.into_iter().take(FILE_READER_WATCH_MAX_TARGETS) {
        let raw_path = target.path.trim();
        if raw_path.is_empty() {
            continue;
        }
        let normalized_path = PathBuf::from(raw_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(raw_path))
            .to_string_lossy()
            .replace('\\', "/");
        if is_file_reader_watch_git_internal_path(&normalized_path) {
            continue;
        }
        if !seen.insert(normalized_path.clone()) {
            continue;
        }
        let kind = match target.kind.trim() {
            "directory" => "directory",
            _ => "file",
        }
        .to_string();
        targets.push(FileReaderWatchTrackedTarget {
            fingerprint: file_reader_watch_fingerprint(&normalized_path),
            path: normalized_path,
            kind,
        });
    }
    let store = file_reader_watch_targets_store();
    let mut targets_by_session = store.lock().map_err(|_| "文件阅读器监听状态锁定失败".to_string())?;
    if targets.is_empty() {
        targets_by_session.remove(&session_id);
    } else {
        targets_by_session.insert(session_id, targets);
    }
    Ok(())
}

fn log_file_reader_read_burst(window_label: &str, path: &str) {
    let now = std::time::Instant::now();
    let traces = FILE_READER_READ_TRACES.get_or_init(|| Mutex::new(Vec::new()));
    let mut traces = traces.lock().unwrap_or_else(|poison| poison.into_inner());
    traces.retain(|item| {
        now.saturating_duration_since(item.at).as_millis()
            <= u128::from(FILE_READER_READ_BURST_WINDOW_MS)
    });
    traces.push(FileReaderReadTrace {
        at: now,
        window_label: window_label.to_string(),
        path: path.to_string(),
    });
    if traces.len() <= 2 {
        return;
    }

    let recent = traces
        .iter()
        .map(|item| {
            format!(
                "{{窗口={}, 距今={}ms, 路径={}}}",
                item.window_label,
                now.saturating_duration_since(item.at).as_millis(),
                item.path
            )
        })
        .collect::<Vec<_>>()
        .join("；");
    runtime_log_debug(format!(
        "[文件阅读窗口] 高频读取，任务=read_file_reader_file，窗口={}，100ms内次数={}，当前路径={}，最近读取=[{}]",
        window_label,
        traces.len(),
        path,
        recent
    ));
}

fn truncate_single_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() > max_chars {
        format!("{}...", line.chars().take(max_chars).collect::<String>())
    } else {
        line.to_string()
    }
}

fn file_reader_decode_text_content(path: &PathBuf) -> Result<String, String> {
    decode_text_file_from_path(path)
        .map(|decoded| decoded.text)
        .map_err(|_| "此文件不是可预览的文本文件".to_string())
}

fn file_reader_decode_text_rope(path: &PathBuf) -> Result<ropey::Rope, String> {
    let content = file_reader_decode_text_content(path)?;
    Ok(ropey::Rope::from_str(&content))
}

fn file_reader_rope_line_count(rope: &ropey::Rope) -> usize {
    rope.len_lines().max(1)
}

fn file_reader_rope_lines_to_string(
    rope: &ropey::Rope,
    start_line: usize,
    line_count: usize,
    truncate_chars: Option<usize>,
) -> String {
    let total_lines = file_reader_rope_line_count(rope);
    if total_lines == 0 {
        return String::new();
    }
    let start_index = start_line.saturating_sub(1).min(total_lines.saturating_sub(1));
    let end_index_exclusive = (start_index + line_count.max(1)).min(total_lines);
    let mut output = String::new();
    for index in start_index..end_index_exclusive {
        let mut line = rope.line(index).to_string();
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        let normalized_line = match truncate_chars {
            Some(limit) => truncate_single_line(&line, limit),
            None => line,
        };
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&normalized_line);
    }
    output
}

#[tauri::command]
async fn read_file_reader_file(window: tauri::Window, path: String) -> Result<FileReaderFilePayload, String> {
    let window_label = window.label().to_string();
    tokio::task::spawn_blocking(move || {
        read_file_reader_file_inner(path, Some(&window_label))
    })
    .await
    .map_err(|err| format!("读取文件任务失败：{err}"))?
}

fn read_file_reader_file_inner(path: String, window_label: Option<&str>) -> Result<FileReaderFilePayload, String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    if let Some(label) = window_label {
        log_file_reader_read_burst(label, raw_path);
    }
    let file_path = PathBuf::from(raw_path);
    if !file_path.exists() {
        return Err(format!("文件不存在：{raw_path}"));
    }
    if !file_path.is_file() {
        return Err(format!("目标不是文件：{raw_path}"));
    }
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(raw_path)
        .to_string();
    let file_key = if extension.is_empty() {
        name.trim().to_ascii_lowercase()
    } else {
        extension
    };
    if file_reader_extension_is_unsupported(&file_key) {
        return Err("此类文件不适合在文件阅读器中直接读取".to_string());
    }
    let metadata = fs::metadata(&file_path).map_err(|err| format!("读取文件信息失败：{err}"))?;
    let file_size = metadata.len();
    let force_plain = file_size > FILE_READER_PLAIN_TEXT_THRESHOLD;
    let resolved_path = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
    let kind = file_reader_file_kind(&file_key).to_string();
    let rope = file_reader_decode_text_rope(&file_path)?;
    let total_lines = file_reader_rope_line_count(&rope);
    let virtualized = kind == "code";
    let content = if kind == "markdown" {
        rope.to_string()
    } else if virtualized {
        String::new()
    } else if force_plain {
        file_reader_rope_lines_to_string(&rope, 1, total_lines, Some(FILE_READER_LINE_TRUNCATE_CHARS))
    } else {
        rope.to_string()
    };
    Ok(FileReaderFilePayload {
        path: resolved_path.to_string_lossy().replace('\\', "/"),
        name,
        extension: file_key.clone(),
        kind,
        content,
        force_plain,
        virtualized,
        total_lines,
        block_line_count: if virtualized { FILE_READER_BLOCK_LINE_COUNT } else { 0 },
    })
}

#[tauri::command]
async fn read_file_reader_file_block(path: String, start_line: usize, line_count: usize) -> Result<FileReaderFileBlockPayload, String> {
    tokio::task::spawn_blocking(move || {
        read_file_reader_file_block_inner(path, start_line, line_count)
    })
    .await
    .map_err(|err| format!("读取文件块任务失败：{err}"))?
}

fn read_file_reader_file_block_inner(path: String, start_line: usize, line_count: usize) -> Result<FileReaderFileBlockPayload, String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let file_path = PathBuf::from(raw_path);
    if !file_path.exists() {
        return Err(format!("文件不存在：{raw_path}"));
    }
    if !file_path.is_file() {
        return Err(format!("目标不是文件：{raw_path}"));
    }
    let extension = file_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(raw_path)
        .to_string();
    let file_key = if extension.is_empty() {
        name.trim().to_ascii_lowercase()
    } else {
        extension
    };
    if file_reader_extension_is_unsupported(&file_key) {
        return Err("此类文件不适合在文件阅读器中直接读取".to_string());
    }
    let metadata = fs::metadata(&file_path).map_err(|err| format!("读取文件信息失败：{err}"))?;
    let force_plain = metadata.len() > FILE_READER_PLAIN_TEXT_THRESHOLD;
    let requested_start_line = start_line.max(1);
    let requested_line_count = line_count.max(1);
    let resolved_path = file_path.canonicalize().unwrap_or_else(|_| file_path.clone());
    let rope = file_reader_decode_text_rope(&file_path)?;
    let total_lines = file_reader_rope_line_count(&rope);
    if total_lines == 0 {
        return Ok(FileReaderFileBlockPayload {
            path: resolved_path.to_string_lossy().replace('\\', "/"),
            start_line: 1,
            end_line: 1,
            content: String::new(),
        });
    }
    let start_index = requested_start_line.saturating_sub(1).min(total_lines.saturating_sub(1));
    let end_index_exclusive = (start_index + requested_line_count).min(total_lines);
    let block_content = file_reader_rope_lines_to_string(
        &rope,
        start_index + 1,
        end_index_exclusive.saturating_sub(start_index),
        if force_plain { Some(FILE_READER_LINE_TRUNCATE_CHARS) } else { None },
    );
    Ok(FileReaderFileBlockPayload {
        path: resolved_path.to_string_lossy().replace('\\', "/"),
        start_line: start_index + 1,
        end_line: end_index_exclusive.max(start_index + 1),
        content: block_content,
    })
}

#[tauri::command]
async fn list_file_reader_directory(path: String, state: State<'_, AppState>) -> Result<FileReaderDirectoryPayload, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || list_file_reader_directory_inner_with_state(path, app_state))
        .await
        .map_err(|err| format!("读取目录任务失败：{err}"))?
}

#[allow(dead_code)]
fn list_file_reader_directory_inner(path: String) -> Result<FileReaderDirectoryPayload, String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let directory_path = PathBuf::from(raw_path);
    if !directory_path.exists() {
        return Err(format!("目录不存在：{raw_path}"));
    }
    if !directory_path.is_dir() {
        return Err(format!("目标不是目录：{raw_path}"));
    }
    let resolved_path = directory_path
        .canonicalize()
        .unwrap_or_else(|_| directory_path.clone());
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&directory_path).map_err(|err| format!("读取目录失败：{err}"))?;
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("读取目录项失败：{err}"))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("读取目录项类型失败：{err}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.trim().is_empty() {
            continue;
        }
        let resolved_entry_path = entry_path.canonicalize().unwrap_or(entry_path);
        entries.push(FileReaderDirectoryEntry {
            path: terminal_path_for_user(&resolved_entry_path).replace('\\', "/"),
            name,
            is_directory: file_type.is_dir(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });

    let name = resolved_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_else(|| raw_path.trim_end_matches(['/', '\\']))
        .to_string();
    Ok(FileReaderDirectoryPayload {
        path: terminal_path_for_user(&resolved_path).replace('\\', "/"),
        name,
        entries,
    })
}

pub(crate) fn list_file_reader_directory_inner_with_state(path: String, state: AppState) -> Result<FileReaderDirectoryPayload, String> {
    let mut raw_path = path.trim().to_string();
    if raw_path.is_empty() {
        // 空目录不允许：回退到助理空间（统一体验）
        if let Ok(root) = terminal_default_session_root_canonical(&state) {
            raw_path = shell_workspace_display_path(&root);
        } else {
            // 兜底：尝试列举盘符（Windows）或根目录
            #[cfg(target_os = "windows")]
            {
                return Ok(list_windows_drives_payload());
            }
            #[cfg(not(target_os = "windows"))]
            {
                raw_path = "/".to_string();
                if raw_path.is_empty() {
                    return Err("path is required".to_string());
                }
            }
        }
    }
    let directory_path = PathBuf::from(raw_path.clone());
    if !directory_path.exists() {
        return Err(format!("目录不存在：{raw_path}"));
    }
    if !directory_path.is_dir() {
        return Err(format!("目标不是目录：{raw_path}"));
    }
    let resolved_path = directory_path
        .canonicalize()
        .unwrap_or_else(|_| directory_path.clone());
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&directory_path).map_err(|err| format!("读取目录失败：{err}"))?;
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("读取目录项失败：{err}"))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("读取目录项类型失败：{err}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.trim().is_empty() {
            continue;
        }
        let resolved_entry_path = entry_path.canonicalize().unwrap_or(entry_path);
        entries.push(FileReaderDirectoryEntry {
            path: terminal_path_for_user(&resolved_entry_path).replace('\\', "/"),
            name,
            is_directory: file_type.is_dir(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });

    let name = resolved_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_else(|| raw_path.trim_end_matches(['/', '\\']))
        .to_string();
    Ok(FileReaderDirectoryPayload {
        path: terminal_path_for_user(&resolved_path).replace('\\', "/"),
        name,
        entries,
    })
}

fn list_windows_drives_payload() -> FileReaderDirectoryPayload {
    let mut entries = Vec::new();
    #[cfg(target_os = "windows")]
    {
        for drive in b'A'..=b'Z' {
            let drive_path = format!("{}:/", drive as char);
            let exists = PathBuf::from(&drive_path).exists();
            if exists {
                entries.push(FileReaderDirectoryEntry {
                    path: drive_path.clone(),
                    name: drive_path.clone(),
                    is_directory: true,
                });
            }
        }
    }
    FileReaderDirectoryPayload {
        path: String::new(),
        name: String::new(),
        entries,
    }
}

#[tauri::command]
fn open_file_reader_directory_target(input: OpenFileReaderDirectoryTargetInput, state: State<'_, AppState>) -> Result<(), String> {
    let raw_path = input.path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let target_kind = input.target_kind.trim();
    let path = PathBuf::from(raw_path);
    if let Some(shell_kind) = target_kind.strip_prefix("shell:") {
        return open_shell_terminal_at_path(&state, &path, Some(shell_kind));
    }
    match target_kind {
        "vscode" => open_directory_in_vscode(&path),
        "explorer" | "file-manager" => open_local_file_directory(raw_path.to_string()),
        _ => Err(format!("Unsupported open target: {target_kind}")),
    }
}

#[tauri::command]
fn open_file_reader_directory_shell(input: OpenFileReaderDirectoryShellInput, state: State<'_, AppState>) -> Result<(), String> {
    let raw_path = input.path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    open_shell_terminal_at_path(&state, &PathBuf::from(raw_path), input.preferred_kind.as_deref())
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

#[tauri::command]
fn open_file_with_default_program(path: String) -> Result<(), String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("path is required".to_string());
    }
    let file_path = PathBuf::from(raw_path);
    if !file_path.exists() {
        return Err(format!("File not found: {raw_path}"));
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", raw_path]);
        // cmd 只是启动器，自身不能弹多余控制台；文件由默认程序正常打开。
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        command
            .status()
            .map_err(|err| format!("Failed to open file: {err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(raw_path)
            .status()
            .map_err(|err| format!("Failed to open file: {err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(raw_path)
            .status()
            .map_err(|err| format!("Failed to open file: {err}"))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Open file is not supported on this platform".to_string())
}
