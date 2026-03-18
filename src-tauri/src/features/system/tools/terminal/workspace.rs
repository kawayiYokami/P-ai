fn normalize_terminal_tool_session_id(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "default-session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn terminal_session_has_locked_root(state: &AppState, session_id: &str) -> bool {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let root_text = {
        let guard = match state.terminal_session_roots.lock() {
            Ok(v) => v,
            Err(_) => return false,
        };
        guard.get(&normalized).cloned()
    };
    let Some(root_text) = root_text else {
        return false;
    };
    PathBuf::from(root_text).is_dir()
}

fn normalize_terminal_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    let value = timeout_ms.unwrap_or(TERMINAL_DEFAULT_TIMEOUT_MS);
    value.clamp(1, TERMINAL_MAX_TIMEOUT_MS)
}

fn normalize_terminal_path_for_compare(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        text.to_ascii_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        text
    }
}

fn path_is_within(base: &Path, target: &Path) -> bool {
    let base_norm = normalize_terminal_path_for_compare(base);
    let target_norm = normalize_terminal_path_for_compare(target);
    let separator = std::path::MAIN_SEPARATOR.to_string();
    target_norm == base_norm
        || target_norm
            .strip_prefix(&(base_norm.clone() + &separator))
            .is_some()
}

fn resolve_terminal_path(base_dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Path is empty.".to_string());
    }
    let normalized = normalize_terminal_path_input_for_current_platform(trimmed);
    if normalized.is_empty() {
        return Err("Path is empty.".to_string());
    }
    let raw_path = PathBuf::from(&normalized);
    let joined = if raw_path.is_absolute() {
        raw_path
    } else {
        base_dir.join(raw_path)
    };

    let canonical = joined
        .canonicalize()
        .map_err(|_| format!("Path does not exist: {}", joined.to_string_lossy()))?;
    if !canonical.is_dir() {
        return Err(format!(
            "Path is not a directory: {}",
            canonical.to_string_lossy()
        ));
    }
    Ok(canonical)
}

fn terminal_workspace_canonical(state: &AppState) -> Result<PathBuf, String> {
    state
        .llm_workspace_path
        .canonicalize()
        .map_err(|err| format!("Resolve llm workspace failed: {err}"))
}

fn terminal_cwd_in_agent_default_workspace(state: &AppState, cwd: &Path) -> bool {
    let default_root = match terminal_workspace_canonical(state) {
        Ok(path) => path,
        Err(_) => return false,
    };
    let normalized_cwd = terminal_normalize_for_access_check(cwd);
    path_is_within(&default_root, &normalized_cwd)
}

#[derive(Debug, Clone)]
struct TerminalWorkspaceResolved {
    name: String,
    path: PathBuf,
}

fn ensure_default_shell_workspace_in_config(config: &mut AppConfig, state: &AppState) {
    let default_path = terminal_path_for_user(&state.llm_workspace_path);
    let default_canonical = PathBuf::from(&default_path).canonicalize().ok();
    for workspace in &mut config.shell_workspaces {
        let candidate = PathBuf::from(workspace.path.trim());
        let matches_default = if let (Some(default_dir), Ok(candidate_dir)) =
            (default_canonical.as_ref(), candidate.canonicalize())
        {
            normalize_terminal_path_for_compare(default_dir)
                == normalize_terminal_path_for_compare(&candidate_dir)
        } else {
            normalize_terminal_path_for_compare(&candidate)
                == normalize_terminal_path_for_compare(&PathBuf::from(&default_path))
        };
        if matches_default {
            workspace.built_in = true;
            if workspace.name.trim().is_empty() {
                workspace.name = "默认工作空间".to_string();
            }
            return;
        }
    }
    if config.shell_workspaces.iter().any(|w| w.built_in) {
        return;
    }
    config.shell_workspaces.insert(
        0,
        ShellWorkspaceConfig {
            name: "默认工作空间".to_string(),
            path: default_path,
            built_in: true,
        },
    );
}

fn terminal_allowed_workspaces_canonical(
    state: &AppState,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);
    ensure_default_shell_workspace_in_config(&mut config, state);
    let mut out = Vec::<TerminalWorkspaceResolved>::new();
    let mut seen_names = std::collections::HashSet::<String>::new();
    for raw in &config.shell_workspaces {
        let name = raw.name.trim();
        let path = raw.path.trim();
        if name.is_empty() || path.is_empty() {
            continue;
        }
        let canonical = match PathBuf::from(path).canonicalize() {
            Ok(v) if v.is_dir() => v,
            _ => continue,
        };
        let key = name.to_ascii_lowercase();
        if !seen_names.insert(key) {
            continue;
        }
        out.push(TerminalWorkspaceResolved {
            name: name.to_string(),
            path: canonical,
        });
    }
    if out.is_empty() {
        out.push(TerminalWorkspaceResolved {
            name: "默认工作空间".to_string(),
            path: terminal_workspace_canonical(state)?,
        });
    }
    Ok(out)
}

fn terminal_allowed_project_roots_canonical(state: &AppState) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::<PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for ws in terminal_allowed_workspaces_canonical(state)? {
        let canonical = ws.path;
        let key = normalize_terminal_path_for_compare(&canonical);
        if seen.insert(key) {
            roots.push(canonical);
        }
    }

    if roots.is_empty() {
        roots.push(terminal_workspace_canonical(state)?);
    }

    Ok(roots)
}

fn terminal_prompt_trusted_roots_block(state: &AppState, selected_api: &ApiConfig) -> Option<String> {
    let terminal_enabled = selected_api.enable_tools
        && selected_api
            .tools
            .iter()
            .any(|tool| {
                tool.enabled
                    && matches!(
                        tool.id.as_str(),
                        "exec" | "apply_patch"
                    )
            });
    if !terminal_enabled {
        return None;
    }

    let workspaces = terminal_allowed_workspaces_canonical(state).ok()?;
    if workspaces.is_empty() {
        return None;
    }

    let mut lines = Vec::<String>::new();
    lines.push("[SHELL WORKSPACE 约束]".to_string());
    lines.push("你只能在允许的工作空间根目录内执行命令。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("禁止在命令中使用绝对路径。".to_string());
    lines.push("允许的工作空间根目录：".to_string());
    for (index, ws) in workspaces.iter().enumerate() {
        lines.push(format!("{}. {}", index + 1, ws.name));
    }
    lines.push(
        "切换请调用 shell_switch_workspace(workspaceName)；执行请调用 shell_exec(command)。"
            .to_string(),
    );
    Some(lines.join("\n"))
}

fn terminal_default_session_root_canonical(state: &AppState) -> Result<PathBuf, String> {
    let allowed = terminal_allowed_project_roots_canonical(state)?;
    allowed
        .into_iter()
        .next()
        .ok_or_else(|| "No terminal project root available".to_string())
}

fn terminal_session_root_canonical(state: &AppState, session_id: &str) -> Result<PathBuf, String> {
    let default_root = terminal_default_session_root_canonical(state)?;
    let root_text = {
        let guard = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        guard.get(session_id).cloned()
    };
    let Some(root_text) = root_text else {
        return Ok(default_root);
    };

    let root = PathBuf::from(root_text);
    match root.canonicalize() {
        Ok(path) if path.is_dir() => {
            Ok(path)
        }
        _ => Ok(default_root),
    }
}

fn ensure_terminal_workdir_allowed(
    state: &AppState,
    session_id: &str,
    cwd: &Path,
) -> Result<(), String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    if path_is_within(&session_root, cwd) {
        return Ok(());
    }
    Err(format!(
        "Working directory is outside current shell root: {}. Call shell_switch_workspace first.",
        cwd.to_string_lossy()
    ))
}

fn resolve_terminal_cwd(
    state: &AppState,
    session_id: &str,
    requested_cwd: Option<&str>,
) -> Result<PathBuf, String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    let resolved = if let Some(raw) = requested_cwd {
        if raw.trim().is_empty() {
            session_root.clone()
        } else {
            resolve_terminal_path(&session_root, raw)?
        }
    } else {
        session_root.clone()
    };
    ensure_terminal_workdir_allowed(state, session_id, &resolved)?;
    Ok(resolved)
}

fn terminal_path_allowed(state: &AppState, session_id: &str, target: &Path) -> Result<bool, String> {
    let session_root = terminal_session_root_canonical(state, session_id)?;
    if path_is_within(&session_root, target) {
        return Ok(true);
    }
    Ok(false)
}

fn terminal_should_parse_command_paths_for_boundary_check() -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Linux/macOS rely on OS sandbox backend for hard path boundary.
        return false;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        true
    }
}

fn terminal_normalize_for_access_check(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    if let Some(parent) = path.parent() {
        if let Ok(parent_canonical) = parent.canonicalize() {
            if let Some(name) = path.file_name() {
                return parent_canonical.join(name);
            }
            return parent_canonical;
        }
    }
    path.to_path_buf()
}
