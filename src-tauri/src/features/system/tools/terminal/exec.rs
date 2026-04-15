fn terminal_decode_live_line(bytes: &[u8]) -> String {
    terminal_decode_output_bytes(bytes)
}

async fn terminal_live_exec_command(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
    command: &str,
    timeout_ms: u64,
) -> Result<SandboxExecutionResult, String> {
    let session = terminal_live_session_for(state, session_id, cwd).await?;
    let runtime_shell = terminal_shell_for_state(state);
    let _session_guard = session.exec_lock.lock().await;
    let marker = format!("__ECA_DONE__{}", Uuid::new_v4());
    let wrapped = terminal_live_compose_command(&runtime_shell, cwd, command, &marker);
    {
        let mut stdin = session.stdin.lock().await;
        stdin
            .write_all(wrapped.as_bytes())
            .await
            .map_err(|err| format!("write live shell stdin failed: {err}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|err| format!("write live shell stdin failed: {err}"))?;
        stdin
            .flush()
            .await
            .map_err(|err| format!("flush live shell stdin failed: {err}"))?;
    }

    let started = std::time::Instant::now();
    let mut stdout_reader = session.stdout.lock().await;
    let mut stderr_reader = session.stderr.lock().await;
    let mut stdout_text = String::new();
    let mut stderr_text = String::new();
    let mut exit_code = 0i32;

    loop {
        let elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        if elapsed >= timeout_ms {
            let _ = terminal_live_close_session(state, session_id).await;
            return Err(format!("terminal_exec timed out after {}ms", timeout_ms));
        }
        let remain = timeout_ms.saturating_sub(elapsed).max(1);
        let mut out_line = Vec::<u8>::new();
        let mut err_line = Vec::<u8>::new();
        let selected = tokio::time::timeout(
            std::time::Duration::from_millis(remain),
            async {
                tokio::select! {
                    out = stdout_reader.read_until(b'\n', &mut out_line) => ("stdout", out.map(|n| n as i64), out_line),
                    err = stderr_reader.read_until(b'\n', &mut err_line) => ("stderr", err.map(|n| n as i64), err_line),
                }
            },
        )
        .await;
        let (stream, read_res, line) = match selected {
            Ok(value) => value,
            Err(_) => {
                let _ = terminal_live_close_session(state, session_id).await;
                return Err(format!("terminal_exec timed out after {}ms", timeout_ms));
            }
        };
        let n = read_res.map_err(|err| format!("read live shell output failed: {err}"))?;
        if n == 0 {
            let _ = terminal_live_close_session(state, session_id).await;
            return Err("live shell closed unexpectedly".to_string());
        }
        let line = terminal_decode_live_line(&line);
        let trimmed = line.trim_end_matches(['\r', '\n']);
        // Some commands (for example `cat`/`head` on files without trailing newline)
        // may print payload and marker in the same line. Detect marker anywhere in stdout.
        if stream == "stdout" && trimmed.contains(&marker) {
            if let Some(marker_pos) = trimmed.find(&marker) {
                let prefix = &trimmed[..marker_pos];
                if !prefix.is_empty() {
                    stdout_text.push_str(prefix);
                }
                let suffix = &trimmed[marker_pos + marker.len()..];
                let suffix = suffix.strip_prefix(':').unwrap_or(suffix).trim();
                exit_code = suffix.parse::<i32>().unwrap_or(0);
            }
            loop {
                let drain_elapsed = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                if drain_elapsed >= timeout_ms {
                    break;
                }
                let drain_remain = timeout_ms.saturating_sub(drain_elapsed).max(1).min(50);
                let mut drain_err_line = Vec::<u8>::new();
                let drained = tokio::time::timeout(
                    std::time::Duration::from_millis(drain_remain),
                    stderr_reader.read_until(b'\n', &mut drain_err_line),
                )
                .await;
                let drain_n = match drained {
                    Ok(result) => result.map_err(|err| format!("read live shell output failed: {err}"))?,
                    Err(_) => break,
                };
                if drain_n == 0 {
                    break;
                }
                stderr_text.push_str(&terminal_decode_live_line(&drain_err_line));
            }
            break;
        }
        if stream == "stdout" {
            stdout_text.push_str(&line);
        } else {
            stderr_text.push_str(&line);
        }
    }

    *session.last_used_at.lock().await = now_iso();

    Ok(SandboxExecutionResult {
        ok: exit_code == 0,
        exit_code,
        stdout: stdout_text.into_bytes(),
        stderr: stderr_text.into_bytes(),
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        shell_kind: session.shell_kind.clone(),
        shell_path: session.shell_path.clone(),
    })
}

fn terminal_is_approval_timeout_error(err: &str) -> bool {
    err.contains("terminal_approval_timeout")
}

fn terminal_approval_timeout_blocked_result(
    normalized_session: &str,
    session_root_text: &str,
    workspace_path_text: &str,
    cwd: &Path,
    cmd: &str,
) -> Value {
    serde_json::json!({
        "ok": false,
        "approved": false,
        "blockedReason": "approval_timeout_local_required",
        "message": "审核超时：当前本地并无管理员监守，非默认工作目录禁止高危操作。请在本机完成审批；如无法审批，请改用其他方式修改（例如先备份再新生成）。",
        "sessionId": normalized_session,
        "rootPath": session_root_text,
        "workspacePath": workspace_path_text,
        "cwd": terminal_path_for_user(cwd),
        "command": cmd,
    })
}

fn terminal_workspace_access_rank(access: &str) -> i32 {
    match access {
        SHELL_WORKSPACE_ACCESS_READ_ONLY => 3,
        SHELL_WORKSPACE_ACCESS_APPROVAL => 2,
        _ => 1,
    }
}

fn terminal_strictest_workspace_access(accesses: &[String]) -> String {
    accesses
        .iter()
        .max_by_key(|access| terminal_workspace_access_rank(access.as_str()))
        .cloned()
        .unwrap_or_else(|| SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string())
}

fn terminal_is_python_like_command(command: &str) -> bool {
    let tokens = terminal_tokenize(command);
    let Some(first) = tokens.first() else {
        return false;
    };
    let token = terminal_unquote_token(first);
    let exe = token
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(token.as_str())
        .to_ascii_lowercase();
    matches!(exe.as_str(), "python" | "python.exe" | "py" | "py.exe")
}

async fn builtin_shell_exec(
    state: &AppState,
    session_id: &str,
    action: &str,
    command: &str,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let action = action.trim().to_ascii_lowercase();
    let cmd = command.trim();
    let runtime_shell = terminal_shell_for_state(state);
    #[cfg(target_os = "windows")]
    if runtime_shell.kind == "missing-terminal-shell" {
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "missing_terminal_shell",
            "message": "No supported shell was detected on Windows. Install Git and use Git Bash: https://git-scm.com/downloads",
            "sessionId": normalize_terminal_tool_session_id(session_id),
            "command": cmd
        }));
    }
    let normalized_session = normalize_terminal_tool_session_id(session_id);
    if action == "list" {
        let sessions = terminal_live_list_sessions(state).await;
        return Ok(serde_json::json!({
            "ok": true,
            "action": "list",
            "sessions": sessions,
            "sessionCount": sessions.len(),
        }));
    }
    if action == "close" {
        let closed = terminal_live_close_session(state, &normalized_session).await?;
        return Ok(serde_json::json!({
            "ok": true,
            "action": "close",
            "sessionId": normalized_session,
            "closed": closed,
        }));
    }
    if action != "run" {
        return Err(format!("shell_exec.action must be run|list|close, got: {action}"));
    }
    if cmd.is_empty() {
        return Err("shell_exec.command is empty".to_string());
    }
    if let Some(reason) = terminal_command_block_reason(cmd) {
        return Err(format!("shell_exec blocked: {reason}"));
    }
    let allowed_project_roots = terminal_allowed_project_roots_for_session_canonical(state, &normalized_session)?
        .iter()
        .map(|v| terminal_path_for_user(v))
        .collect::<Vec<_>>();
    let session_root = terminal_session_root_canonical(state, &normalized_session)?;
    let session_root_text = terminal_path_for_user(&session_root);
    let workspace_path_text = configured_workspace_root_path(state)
        .map(|path| terminal_path_for_user(&path))
        .unwrap_or_else(|_| terminal_path_for_user(&state.llm_workspace_path));
    let cwd = match resolve_terminal_cwd(state, &normalized_session, None) {
        Ok(path) => path,
        Err(err) if err.contains("Call shell_switch_workspace first.") => {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "path_not_granted",
                "message": err,
                "sessionId": normalized_session,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "cwd": "",
                "command": cmd,
            }));
        }
        Err(err) => return Err(err),
    };
    let timeout_ms = normalize_terminal_timeout_ms(timeout_ms);
    let command_paths =
        terminal_collect_command_path_candidate_details(&cwd, cmd, &runtime_shell.kind);
    let mut unmatched_paths = Vec::<TerminalCommandPathCandidate>::new();
    let mut matched_accesses = Vec::<String>::new();
    for candidate in &command_paths {
        if let Some(workspace) = terminal_match_workspace_for_session_target(state, &normalized_session, &candidate.path)? {
            matched_accesses.push(workspace.access);
        } else {
            unmatched_paths.push(candidate.clone());
        }
    }
    let (write_target_paths, _) = terminal_collect_write_target_paths(&cwd, cmd);
    let mut matched_write_accesses = Vec::<String>::new();
    let mut unmatched_write_targets = Vec::<PathBuf>::new();
    for path in &write_target_paths {
        if let Some(workspace) = terminal_match_workspace_for_session_target(state, &normalized_session, path)? {
            matched_write_accesses.push(workspace.access);
        } else {
            unmatched_write_targets.push(path.clone());
        }
    }
    let cwd_access = terminal_match_workspace_for_session_target(state, &normalized_session, &cwd)?
        .map(|workspace| workspace.access)
        .unwrap_or_else(|| SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string());
    let effective_access = if matched_accesses.is_empty() {
        cwd_access.clone()
    } else {
        terminal_strictest_workspace_access(&matched_accesses)
    };
    let effective_write_access = if matched_write_accesses.is_empty() {
        cwd_access.clone()
    } else {
        terminal_strictest_workspace_access(&matched_write_accesses)
    };
    let relative_unmatched_paths = unmatched_paths
        .iter()
        .filter(|item| !item.is_absolute)
        .map(|item| terminal_path_for_user(&item.path))
        .collect::<Vec<_>>();
    if !relative_unmatched_paths.is_empty() {
        return Ok(serde_json::json!({
            "ok": false,
            "approved": false,
            "blockedReason": "relative_path_outside_workspace",
            "message": "相对路径不能脱离当前工作目录，请改用当前目录内相对路径或显式绝对路径。",
            "sessionId": normalized_session,
            "rootPath": session_root_text,
            "workspacePath": workspace_path_text,
            "allowedProjectRoots": allowed_project_roots,
            "cwd": terminal_path_for_user(&cwd),
            "command": cmd,
            "ungrantedPaths": relative_unmatched_paths,
        }));
    }

    let write_risk = classify_terminal_write_risk(&cwd, cmd);
    let is_write_command = !matches!(write_risk, TerminalWriteRisk::None);
    if terminal_is_python_like_command(cmd) {
        if effective_access != SHELL_WORKSPACE_ACCESS_FULL_ACCESS {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "python_requires_full_access",
                "message": "python/py 命令默认不走审批；当前目录不是完全访问，请改用 apply_patch 或明确的文件修改命令。",
                "sessionId": normalized_session,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
            }));
        }
    } else if is_write_command {
        let unmatched_write_paths = if matches!(write_risk, TerminalWriteRisk::Unknown)
            && unmatched_write_targets.is_empty()
        {
            unmatched_paths
                .iter()
                .map(|item| terminal_path_for_user(&item.path))
                .collect::<Vec<_>>()
        } else {
            unmatched_write_targets
                .iter()
                .map(|item| terminal_path_for_user(item))
                .collect::<Vec<_>>()
        };
        if !unmatched_write_paths.is_empty() {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "write_path_not_granted",
                "message": "写入类命令只能作用于已配置工作目录；未纳管绝对路径仅允许读取。",
                "sessionId": normalized_session,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
                "ungrantedPaths": unmatched_write_paths,
            }));
        }

        if effective_write_access == SHELL_WORKSPACE_ACCESS_READ_ONLY {
            return Ok(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "read_only_workspace",
                "message": "当前目录权限为只读，禁止执行写入类终端命令。",
                "sessionId": normalized_session,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
            }));
        }
    }

    match write_risk {
        TerminalWriteRisk::None => {}
        TerminalWriteRisk::NewOnly { count } => {
            runtime_log_debug(format!(
                "[TOOL-DEBUG] shell_exec write-risk=NewOnly new_path_count={} session={}",
                count, normalized_session
            ));
            if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                let message = format!(
                    "该命令将创建或改写文件，是否批准本次执行？\n会话: {normalized_session}\n工作目录: {}\n命令: {cmd}",
                    cwd.to_string_lossy()
                );
                let approved = match terminal_request_user_approval(
                    state,
                    "终端执行审批",
                    &message,
                    &normalized_session,
                    "new_write_risk",
                    Some(&cwd),
                    Some(cmd),
                    None,
                    None,
                    &[],
                )
                .await
                {
                    Ok(v) => v,
                    Err(err) if terminal_is_approval_timeout_error(&err) => {
                        return Ok(terminal_approval_timeout_blocked_result(
                            &normalized_session,
                            &session_root_text,
                            &workspace_path_text,
                            &cwd,
                            cmd,
                        ));
                    }
                    Err(err) => return Err(err),
                };
                if !approved {
                    return Ok(serde_json::json!({
                        "ok": false,
                        "approved": false,
                        "blockedReason": "user_denied_new_file_change",
                        "message": "用户拒绝了本次写入类终端命令。",
                        "sessionId": normalized_session,
                        "rootPath": session_root_text,
                        "workspacePath": workspace_path_text,
                        "cwd": terminal_path_for_user(&cwd),
                        "command": cmd,
                    }));
                }
            }
        }
        TerminalWriteRisk::Existing { paths } => {
            if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                let mut lines = vec![
                    "该命令将修改/删除已有文件，是否批准本次执行？".to_string(),
                    format!("会话: {normalized_session}"),
                    format!("工作目录: {}", cwd.to_string_lossy()),
                    format!("命令: {cmd}"),
                    "命中已有路径：".to_string(),
                ];
                for path in paths.iter().take(8) {
                    lines.push(format!("- {}", path.to_string_lossy()));
                }
                if paths.len() > 8 {
                    lines.push(format!("... 其余 {} 项已省略", paths.len() - 8));
                }
                let approved = match terminal_request_user_approval(
                    state,
                    "终端执行审批",
                    &lines.join("\n"),
                    &normalized_session,
                    "existing_write_risk",
                    Some(&cwd),
                    Some(cmd),
                    None,
                    None,
                    &paths,
                )
                .await
                {
                    Ok(v) => v,
                    Err(err) if terminal_is_approval_timeout_error(&err) => {
                        return Ok(terminal_approval_timeout_blocked_result(
                            &normalized_session,
                            &session_root_text,
                            &workspace_path_text,
                            &cwd,
                            cmd,
                        ));
                    }
                    Err(err) => return Err(err),
                };
                if !approved {
                    return Ok(serde_json::json!({
                        "ok": false,
                        "approved": false,
                        "blockedReason": "user_denied_existing_file_change",
                        "message": "用户拒绝了本次写入类终端命令。",
                        "sessionId": normalized_session,
                        "rootPath": session_root_text,
                        "workspacePath": workspace_path_text,
                        "cwd": terminal_path_for_user(&cwd),
                        "command": cmd,
                    }));
                }
            }
        }
        TerminalWriteRisk::Unknown => {
            if effective_write_access == SHELL_WORKSPACE_ACCESS_APPROVAL {
                return Ok(serde_json::json!({
                    "ok": false,
                    "approved": false,
                    "blockedReason": "approval_requires_explicit_write_command",
                    "message": "当前目录需要审批，但该命令无法明确识别具体写入目标，请改用 apply_patch 或更明确的文件修改命令。",
                    "sessionId": normalized_session,
                    "rootPath": session_root_text,
                    "workspacePath": workspace_path_text,
                    "cwd": terminal_path_for_user(&cwd),
                    "command": cmd,
                }));
            }
        }
    }

    let execution_result = if terminal_live_session_supported(&runtime_shell) {
        terminal_live_exec_command(state, &normalized_session, &cwd, cmd, timeout_ms).await
    } else {
        sandbox_execute_command(state, &normalized_session, cmd, &cwd, timeout_ms).await
    };
    let execution = match execution_result {
        Ok(execution) => execution,
        Err(err) if terminal_is_timeout_error(&err) => {
            return Ok(serde_json::json!({
                "ok": false,
                "shellKind": runtime_shell.kind,
                "shellPath": runtime_shell.path,
                "sessionId": normalized_session,
                "rootPath": session_root_text,
                "workspacePath": workspace_path_text,
                "allowedProjectRoots": allowed_project_roots,
                "cwd": terminal_path_for_user(&cwd),
                "command": cmd,
                "exitCode": -1,
                "stdout": "",
                "stderr": err,
                "durationMs": timeout_ms,
                "timedOut": true,
                "truncated": false,
                "stdoutTruncated": false,
                "stderrTruncated": false
            }));
        }
        Err(err) => return Err(err),
    };
    let (stdout, stdout_truncated) = truncate_terminal_output(&execution.stdout);
    let (stderr, stderr_truncated) = truncate_terminal_output(&execution.stderr);

    Ok(serde_json::json!({
        "ok": execution.ok,
        "shellKind": execution.shell_kind,
        "shellPath": execution.shell_path,
        "sessionId": normalized_session,
        "rootPath": session_root_text,
        "workspacePath": workspace_path_text,
        "allowedProjectRoots": allowed_project_roots,
        "cwd": terminal_path_for_user(&cwd),
        "command": cmd,
        "exitCode": execution.exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "durationMs": execution.duration_ms,
        "timedOut": false,
        "sessionManaged": terminal_live_session_supported(&runtime_shell),
        "truncated": stdout_truncated || stderr_truncated,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated
    }))
}

#[cfg(test)]
mod terminal_exec_tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tokio::time::{timeout, Duration};

    fn build_test_state(shell: TerminalShellProfile, llm_workspace_path: PathBuf) -> AppState {
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: llm_workspace_path.join("app_config.toml"),
            data_path: llm_workspace_path.join("app_data.json"),
            llm_workspace_path,
            shared_http_client: reqwest::Client::new(),
            terminal_shell: shell.clone(),
            terminal_shell_candidates: vec![shell],
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_mtime: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(HashSet::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
        }
    }

    fn shell_candidate_by_kind(kind: &str) -> Option<TerminalShellProfile> {
        detect_terminal_shell_candidates()
            .into_iter()
            .find(|item| item.kind == kind)
    }

    fn configure_test_workspaces(
        state: &AppState,
        _main_access: &str,
        _secondary_access: &str,
    ) -> Result<(PathBuf, PathBuf, PathBuf), String> {
        let system_root = state.llm_workspace_path.clone();
        let main_root = system_root.join("main-workspace");
        let secondary_root = system_root.join("secondary-workspace");
        fs::create_dir_all(&system_root).map_err(|err| format!("create system root failed: {err}"))?;
        fs::create_dir_all(&main_root).map_err(|err| format!("create main root failed: {err}"))?;
        fs::create_dir_all(&secondary_root).map_err(|err| format!("create secondary root failed: {err}"))?;
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&system_root),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(state, &config).map_err(|err| format!("write config failed: {err}"))?;
        Ok((system_root, main_root, secondary_root))
    }

    fn configure_test_conversation_workspaces(
        state: &AppState,
        conversation_id: &str,
        agent_id: &str,
        locked_root: Option<&Path>,
        main_root: &Path,
        main_access: &str,
        secondary_root: &Path,
        secondary_access: &str,
    ) -> Result<String, String> {
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: conversation_id.to_string(),
            title: "Terminal Test Conversation".to_string(),
            agent_id: agent_id.to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: locked_root.map(terminal_path_for_user),
            shell_workspaces: vec![
                ShellWorkspaceConfig {
                    id: "main-workspace-1".to_string(),
                    name: "主要工作目录".to_string(),
                    path: terminal_path_for_user(main_root),
                    level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                    access: main_access.to_string(),
                    built_in: false,
                },
                ShellWorkspaceConfig {
                    id: "secondary-workspace-1".to_string(),
                    name: "次要工作目录".to_string(),
                    path: terminal_path_for_user(secondary_root),
                    level: SHELL_WORKSPACE_LEVEL_SECONDARY.to_string(),
                    access: secondary_access.to_string(),
                    built_in: false,
                },
            ],
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(state, &data)
            .map_err(|err| format!("write app data failed: {err}"))?;
        Ok(normalize_terminal_tool_session_id(&inflight_chat_key(
            agent_id,
            Some(conversation_id),
        )))
    }

    async fn verify_default_workspace_skip_for_shell(kind: &str) -> Result<(), String> {
        let Some(shell) = shell_candidate_by_kind(kind) else {
            eprintln!("[TEST] skip shell kind={kind}: not available on this machine");
            return Ok(());
        };

        let root = std::env::temp_dir().join(format!("eca-terminal-skip-{}-{}", kind, Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        let existing_file = root.join("existing.txt");
        fs::write(&existing_file, "before\n").map_err(|err| format!("seed file failed: {err}"))?;

        let state = build_test_state(shell, root.clone());
        let started = std::time::Instant::now();
        let run = builtin_shell_exec(
            &state,
            "test-session",
            "run",
            "echo changed > ./existing.txt",
            Some(8_000),
        );
        let result = timeout(Duration::from_secs(15), run)
            .await
            .map_err(|_| "builtin_shell_exec timed out (likely waiting for approval)".to_string())??;

        let elapsed = started.elapsed();
        let approvals_left = state
            .terminal_pending_approvals
            .lock()
            .map_err(|_| "lock terminal_pending_approvals failed".to_string())?
            .len();
        if approvals_left != 0 {
            return Err(format!("unexpected pending approvals count: {approvals_left}"));
        }
        if elapsed > Duration::from_secs(15) {
            return Err(format!("execution took too long: {elapsed:?}"));
        }
        let ok = result.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            return Err(format!("shell_exec returned non-ok: {}", result));
        }

        let content = fs::read_to_string(&existing_file)
            .map_err(|err| format!("read updated file failed: {err}"))?;
        if !content.contains("changed") {
            return Err(format!("existing file not updated as expected, content={content:?}"));
        }

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[tokio::test]
    async fn default_workspace_skip_approval_for_powershell() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        if let Err(err) = verify_default_workspace_skip_for_shell(powershell_kind).await {
            panic!("powershell default-workspace skip check failed: {err}");
        }
    }

    #[tokio::test]
    async fn default_workspace_skip_approval_for_git_bash() {
        if let Err(err) = verify_default_workspace_skip_for_shell("git-bash").await {
            panic!("git-bash default-workspace skip check failed: {err}");
        }
    }

    #[test]
    fn approval_timeout_should_map_to_local_required_block() {
        let err = "terminal_approval_timeout: 审核超时（60000ms）";
        assert!(terminal_is_approval_timeout_error(err));
        let result = terminal_approval_timeout_blocked_result(
            "s1",
            "r1",
            "w1",
            std::path::Path::new("."),
            "echo 1 > a.txt",
        );
        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("approval_timeout_local_required")
        );
        assert_eq!(result.get("approved").and_then(Value::as_bool), Some(false));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_read_should_be_allowed() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-read-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");

        let result = builtin_shell_exec(
            &state,
            "read-outside-session",
            "run",
            "Get-Content C:\\Windows\\win.ini | Select-Object -First 1",
            Some(8_000),
        )
        .await
        .expect("run read command");

        assert_eq!(result.get("blockedReason").and_then(Value::as_str), None);
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(true));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_write_should_be_blocked() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-write-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let outside_path = std::env::temp_dir().join(format!("eca-unmanaged-write-{}.txt", Uuid::new_v4()));
        let command = format!(
            "Set-Content -Path '{}' -Value 'hi'",
            outside_path.to_string_lossy()
        );

        let result = builtin_shell_exec(&state, "write-outside-session", "run", &command, Some(8_000))
            .await
            .expect("run write command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("write_path_not_granted")
        );
        assert!(!outside_path.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn approval_workspace_should_reject_python_command() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-python-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_, main_root, secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-python-approval",
            "agent-python-approval",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            &secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "python -c \"print('hello')\"",
            Some(8_000),
        )
        .await
        .expect("run python command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("python_requires_full_access")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn read_only_workspace_should_block_write_command() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-readonly-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_, main_root, secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-read-only",
            "agent-read-only",
            Some(&secondary_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_APPROVAL,
            &secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "Set-Content -Path .\\note.txt -Value 'hi'",
            Some(8_000),
        )
        .await
        .expect("run readonly command");

        assert_eq!(
            result.get("blockedReason").and_then(Value::as_str),
            Some("read_only_workspace")
        );
        assert!(!secondary_root.join("note.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn unmatched_absolute_read_should_not_block_granted_write_target() {
        let powershell_kind = if shell_candidate_by_kind("powershell7").is_some() {
            "powershell7"
        } else {
            "powershell5"
        };
        let Some(shell) = shell_candidate_by_kind(powershell_kind) else {
            return;
        };
        let root = std::env::temp_dir().join(format!("eca-terminal-mixed-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create root");
        let state = build_test_state(shell, root.clone());
        let (_system_root, main_root, _secondary_root) = configure_test_workspaces(
            &state,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure workspaces");
        let session_id = configure_test_conversation_workspaces(
            &state,
            "conv-mixed-read-write",
            "agent-mixed-read-write",
            Some(&main_root),
            &main_root,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS,
            &_secondary_root,
            SHELL_WORKSPACE_ACCESS_READ_ONLY,
        )
        .expect("configure conversation workspaces");

        let result = builtin_shell_exec(
            &state,
            &session_id,
            "run",
            "Get-Content C:\\Windows\\win.ini | Select-Object -First 1 | Set-Content -Path '.\\note.txt'",
            Some(8_000),
        )
        .await
        .expect("run mixed read/write command");

        assert_eq!(result.get("blockedReason").and_then(Value::as_str), None);
        assert_eq!(result.get("ok").and_then(Value::as_bool), Some(true));
        assert!(main_root.join("note.txt").exists(), "expected note.txt to be created");
        let _ = fs::remove_dir_all(&root);
    }
}
