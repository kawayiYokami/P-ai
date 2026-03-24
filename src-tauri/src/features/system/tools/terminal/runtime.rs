#[derive(Debug, Clone)]
struct TerminalShellProfile {
    kind: String,
    path: String,
    args_prefix: Vec<String>,
}

#[derive(Debug)]
struct TerminalLiveShellSession {
    session_id: String,
    shell_kind: String,
    shell_path: String,
    created_at: String,
    last_used_at: tokio::sync::Mutex<String>,
    child: tokio::sync::Mutex<tokio::process::Child>,
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    stdout: tokio::sync::Mutex<tokio::io::BufReader<tokio::process::ChildStdout>>,
    stderr: tokio::sync::Mutex<tokio::io::BufReader<tokio::process::ChildStderr>>,
    exec_lock: tokio::sync::Mutex<()>,
}

type TerminalLiveShellSessionHandle = std::sync::Arc<TerminalLiveShellSession>;

fn terminal_live_session_supported(shell: &TerminalShellProfile) -> bool {
    #[cfg(target_os = "windows")]
    {
        return matches!(shell.kind.as_str(), "powershell7" | "powershell5" | "git-bash");
    }
    #[cfg(target_os = "macos")]
    {
        return matches!(shell.kind.as_str(), "zsh" | "bash" | "sh");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return matches!(shell.kind.as_str(), "bash" | "zsh" | "sh");
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        false
    }
}

fn terminal_powershell_escape_literal(input: &str) -> String {
    input.replace('\'', "''")
}

fn terminal_bash_escape_literal(input: &str) -> String {
    input.replace('\'', "'\"'\"'")
}

#[cfg(target_os = "windows")]
fn terminal_strip_windows_verbatim_prefix(input: &str) -> String {
    let text = input.trim();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", rest);
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    text.to_string()
}

fn terminal_path_for_user(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        terminal_strip_windows_verbatim_prefix(&text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        text
    }
}

#[cfg(target_os = "windows")]
fn terminal_windows_path_to_bash(path: &Path) -> String {
    let normalized = terminal_strip_windows_verbatim_prefix(&path.to_string_lossy());
    let raw = normalized.replace('\\', "/");
    let bytes = raw.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = raw[3..].trim_start_matches('/');
        if rest.is_empty() {
            return format!("/{drive}");
        }
        return format!("/{drive}/{rest}");
    }
    raw
}

fn terminal_live_compose_command(shell: &TerminalShellProfile, cwd: &Path, command: &str, marker: &str) -> String {
    if matches!(shell.kind.as_str(), "powershell7" | "powershell5") {
        #[cfg(target_os = "windows")]
        let cwd_raw = terminal_strip_windows_verbatim_prefix(&cwd.to_string_lossy());
        #[cfg(not(target_os = "windows"))]
        let cwd_raw = cwd.to_string_lossy().to_string();
        let cwd_text = terminal_powershell_escape_literal(&cwd_raw);
        return format!(
            "$ErrorActionPreference='Continue'; try {{ [Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; chcp.com 65001 > $null; Set-Location -LiteralPath '{cwd_text}'; {command} }} catch {{ Write-Error $_; $global:LASTEXITCODE = 1 }}; $ecaExit = if ($null -eq $LASTEXITCODE) {{ 0 }} else {{ $LASTEXITCODE }}; Write-Output \"{marker}:$ecaExit\""
        );
    }
    if shell.kind == "git-bash" {
        #[cfg(target_os = "windows")]
        {
            let cwd_text = terminal_bash_escape_literal(&terminal_windows_path_to_bash(cwd));
            return format!(
                "chcp.com 65001 > /dev/null 2>&1; export LANG=en_US.UTF-8; cd '{cwd_text}' || exit 1; {command}; printf '%s:%s\\n' '{marker}' \"$?\""
            );
        }
    }
    format!("{command}\nprintf '%s:%s\\n' '{marker}' \"$?\"")
}

async fn terminal_live_create_session(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
) -> Result<TerminalLiveShellSessionHandle, String> {
    let shell = terminal_shell_for_state(state);
    if !terminal_live_session_supported(&shell) {
        return Err("live shell session is unsupported for current shell".to_string());
    }
    let mut command_builder = tokio::process::Command::new(&shell.path);
    #[cfg(target_os = "windows")]
    let process_cwd = std::path::PathBuf::from(terminal_strip_windows_verbatim_prefix(
        &cwd.to_string_lossy(),
    ));
    #[cfg(not(target_os = "windows"))]
    let process_cwd = cwd.to_path_buf();
    command_builder.current_dir(process_cwd);
    command_builder.stdin(std::process::Stdio::piped());
    command_builder.stdout(std::process::Stdio::piped());
    command_builder.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW, keep shell sessions headless on Windows.
        command_builder.creation_flags(0x08000000);
    }
    if matches!(shell.kind.as_str(), "powershell7" | "powershell5") {
        command_builder.arg("-NoLogo");
        command_builder.arg("-NoProfile");
        command_builder.arg("-ExecutionPolicy");
        command_builder.arg("Bypass");
        command_builder.arg("-Command");
        command_builder.arg("-");
    } else if shell.kind == "git-bash" {
        command_builder.arg("--noprofile");
        command_builder.arg("--norc");
        command_builder.env("LANG", "en_US.UTF-8");
        command_builder.env("LC_ALL", "en_US.UTF-8");
    } else if shell.kind == "bash" {
        command_builder.arg("--noprofile");
        command_builder.arg("--norc");
    } else if shell.kind == "zsh" {
        command_builder.arg("-f");
    } else {
        // For live sessions, avoid one-shot flags like -lc/-c.
        // Keep shell interactive and feed commands via stdin.
    }
    let mut child = command_builder
        .spawn()
        .map_err(|err| format!("spawn live shell failed: {err}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "capture live shell stdin failed".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "capture live shell stdout failed".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "capture live shell stderr failed".to_string())?;

    Ok(std::sync::Arc::new(TerminalLiveShellSession {
        session_id: session_id.to_string(),
        shell_kind: shell.kind.clone(),
        shell_path: shell.path.clone(),
        created_at: now_iso(),
        last_used_at: tokio::sync::Mutex::new(now_iso()),
        child: tokio::sync::Mutex::new(child),
        stdin: tokio::sync::Mutex::new(stdin),
        stdout: tokio::sync::Mutex::new(tokio::io::BufReader::new(stdout)),
        stderr: tokio::sync::Mutex::new(tokio::io::BufReader::new(stderr)),
        exec_lock: tokio::sync::Mutex::new(()),
    }))
}

async fn terminal_live_session_for(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
) -> Result<TerminalLiveShellSessionHandle, String> {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let runtime_shell = terminal_shell_for_state(state);
    {
        let mut sessions = state.terminal_live_sessions.lock().await;
        if let Some(existing) = sessions.get(&normalized).cloned() {
            let shell_changed = existing.shell_kind != runtime_shell.kind
                || existing.shell_path != runtime_shell.path;
            if !shell_changed {
                return Ok(existing);
            }
            sessions.remove(&normalized);
            drop(sessions);
            let mut child = existing.child.lock().await;
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
    let created = terminal_live_create_session(state, &normalized, cwd).await?;
    let mut sessions = state.terminal_live_sessions.lock().await;
    Ok(sessions
        .entry(normalized)
        .or_insert_with(|| created.clone())
        .clone())
}

async fn terminal_live_close_session(state: &AppState, session_id: &str) -> Result<bool, String> {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let removed = {
        let mut sessions = state.terminal_live_sessions.lock().await;
        sessions.remove(&normalized)
    };
    let Some(handle) = removed else {
        return Ok(false);
    };
    let mut child = handle.child.lock().await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(true)
}

async fn terminal_live_list_sessions(state: &AppState) -> Vec<Value> {
    let handles = {
        let sessions = state.terminal_live_sessions.lock().await;
        sessions.values().cloned().collect::<Vec<_>>()
    };
    let mut out = Vec::<Value>::new();
    for handle in handles {
        let last_used_at = handle.last_used_at.lock().await.clone();
        out.push(serde_json::json!({
            "sessionId": handle.session_id,
            "shellKind": handle.shell_kind,
            "shellPath": handle.shell_path,
            "createdAt": handle.created_at,
            "lastUsedAt": last_used_at
        }));
    }
    out
}

fn detect_terminal_shell_candidates() -> Vec<TerminalShellProfile> {
    #[cfg(target_os = "windows")]
    {
        fn with_args(kind: &str, path: String, args_prefix: &[&str]) -> TerminalShellProfile {
            TerminalShellProfile {
                kind: kind.to_string(),
                path,
                args_prefix: args_prefix.iter().map(|v| (*v).to_string()).collect(),
            }
        }

        fn first_existing_path(candidates: &[String]) -> Option<String> {
            candidates
                .iter()
                .find(|candidate| Path::new(candidate.as_str()).is_file())
                .cloned()
        }

        fn path_lookup_first(name: &str) -> Option<String> {
            let path_value = std::env::var_os("PATH")?;
            let name_path = Path::new(name);
            let has_ext = name_path.extension().is_some();
            let mut candidates = Vec::<String>::new();
            if has_ext {
                candidates.push(name.to_string());
            } else {
                candidates.push(name.to_string());
                if let Some(pathext) = std::env::var_os("PATHEXT") {
                    for ext in pathext.to_string_lossy().split(';') {
                        let trimmed = ext.trim();
                        if !trimmed.is_empty() {
                            candidates.push(format!("{name}{trimmed}"));
                        }
                    }
                } else {
                    candidates.push(format!("{name}.exe"));
                }
            }

            for dir in std::env::split_paths(&path_value) {
                for candidate in &candidates {
                    let full = dir.join(candidate);
                    if full.is_file() {
                        return Some(full.to_string_lossy().to_string());
                    }
                }
            }
            None
        }

        fn derive_bash_candidates_from_git(git_exe: &str) -> Vec<String> {
            let mut out = Vec::<String>::new();
            let git_path = PathBuf::from(git_exe);
            let Some(cmd_dir) = git_path.parent() else {
                return out;
            };
            let Some(git_root) = cmd_dir.parent() else {
                return out;
            };
            out.push(git_root.join("bin").join("bash.exe").to_string_lossy().to_string());
            out.push(
                git_root
                    .join("usr")
                    .join("bin")
                    .join("bash.exe")
                    .to_string_lossy()
                    .to_string(),
            );
            out
        }

        let mut out = Vec::<TerminalShellProfile>::new();
        let mut git_bash_candidates = vec![
            r"C:\Program Files\Git\bin\bash.exe".to_string(),
            r"C:\Program Files\Git\usr\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\bin\bash.exe".to_string(),
            r"C:\Program Files (x86)\Git\usr\bin\bash.exe".to_string(),
        ];
        if let Some(git_path) = path_lookup_first("git") {
            git_bash_candidates.extend(derive_bash_candidates_from_git(&git_path));
        }
        if let Some(path) = path_lookup_first("bash") {
            git_bash_candidates.push(path);
        }

        if let Some(path) = first_existing_path(&git_bash_candidates) {
            out.push(with_args("git-bash", path, &["-lc"]));
        }

        let mut pwsh7_candidates = vec![
            r"C:\Program Files\PowerShell\7\pwsh.exe".to_string(),
            r"C:\Program Files\PowerShell\7-preview\pwsh.exe".to_string(),
        ];
        if let Some(path) = path_lookup_first("pwsh.exe") {
            pwsh7_candidates.push(path);
        }
        if let Some(path) = first_existing_path(&pwsh7_candidates) {
            out.push(with_args("powershell7", path, &["-NoProfile", "-Command"]));
        }

        let mut powershell5_candidates = Vec::<String>::new();
        if let Ok(windir) = std::env::var("WINDIR") {
            powershell5_candidates.push(
                PathBuf::from(windir)
                    .join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        if let Some(path) = path_lookup_first("powershell.exe") {
            powershell5_candidates.push(path);
        }
        if let Some(path) = first_existing_path(&powershell5_candidates) {
            out.push(with_args("powershell5", path, &["-NoProfile", "-Command"]));
        }
        return out;
    }

    #[cfg(target_os = "macos")]
    {
        let mut out = Vec::<TerminalShellProfile>::new();
        let zsh = Path::new("/bin/zsh");
        if zsh.is_file() {
            out.push(TerminalShellProfile {
                kind: "zsh".to_string(),
                path: zsh.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        let bash = Path::new("/bin/bash");
        if bash.is_file() {
            out.push(TerminalShellProfile {
                kind: "bash".to_string(),
                path: bash.to_string_lossy().to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        if Path::new("/bin/sh").is_file() {
            out.push(TerminalShellProfile {
                kind: "sh".to_string(),
                path: "/bin/sh".to_string(),
                args_prefix: vec!["-lc".to_string()],
            });
        }
        return out;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut out = Vec::<TerminalShellProfile>::new();
        for candidate in ["/bin/bash", "/usr/bin/bash", "/bin/sh"] {
            if Path::new(candidate).is_file() {
                let kind = Path::new(candidate)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("sh")
                    .to_string();
                out.push(TerminalShellProfile {
                    kind,
                    path: candidate.to_string(),
                    args_prefix: vec!["-lc".to_string()],
                });
            }
        }
        return out;
    }

    #[allow(unreachable_code)]
    Vec::new()
}

fn terminal_shell_missing_profile() -> TerminalShellProfile {
    TerminalShellProfile {
        kind: "missing-terminal-shell".to_string(),
        path: String::new(),
        args_prefix: Vec::new(),
    }
}

fn detect_default_terminal_shell() -> TerminalShellProfile {
    detect_terminal_shell_candidates()
        .into_iter()
        .next()
        .unwrap_or_else(terminal_shell_missing_profile)
}

fn terminal_shell_from_candidates(
    candidates: &[TerminalShellProfile],
    preferred_kind: &str,
) -> TerminalShellProfile {
    let preferred = preferred_kind.trim().to_ascii_lowercase();
    if preferred != "auto" && !preferred.is_empty() {
        if let Some(hit) = candidates.iter().find(|item| item.kind == preferred) {
            return hit.clone();
        }
    }
    candidates
        .first()
        .cloned()
        .unwrap_or_else(terminal_shell_missing_profile)
}

fn terminal_shell_for_state(state: &AppState) -> TerminalShellProfile {
    let preferred = state_read_config_cached(state)
        .map(|cfg| cfg.terminal_shell_kind)
        .unwrap_or_else(|_| "auto".to_string());
    terminal_shell_from_candidates(&state.terminal_shell_candidates, &preferred)
}

fn terminal_shell_candidates_for_ui(
    state: &AppState,
) -> (String, TerminalShellProfile, Vec<Value>) {
    let preferred = state_read_config_cached(state)
        .map(|cfg| cfg.terminal_shell_kind)
        .unwrap_or_else(|_| "auto".to_string());
    let candidates = state.terminal_shell_candidates.clone();
    let current = terminal_shell_from_candidates(&candidates, &preferred);
    let mut items = Vec::<Value>::new();
    items.push(serde_json::json!({
        "kind": "auto",
        "label": "Auto",
        "available": true,
        "path": ""
    }));
    for item in &candidates {
        items.push(serde_json::json!({
            "kind": item.kind,
            "label": terminal_shell_runtime_label(item),
            "available": true,
            "path": item.path
        }));
    }
    (preferred, current, items)
}

fn terminal_shell_runtime_label(shell: &TerminalShellProfile) -> String {
    let title = match shell.kind.as_str() {
        "powershell7" => "PowerShell 7",
        "powershell5" => "Windows PowerShell 5.1",
        "git-bash" => "Git Bash",
        "missing-terminal-shell" => "Unavailable",
        other => other,
    };
    if shell.path.trim().is_empty() {
        return title.to_string();
    }
    format!("{title} ({})", shell.path.trim())
}

fn terminal_exec_tool_description(shell: &TerminalShellProfile) -> String {
    format!(
        "在当前 shell 工作区根目录中执行命令。运行时 shell：{}。",
        terminal_shell_runtime_label(shell)
    )
}
