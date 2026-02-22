fn sandbox_normalize_path_for_compare(path: &std::path::Path) -> String {
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

fn sandbox_path_is_within(base: &std::path::Path, target: &std::path::Path) -> bool {
    let base_norm = sandbox_normalize_path_for_compare(base);
    let target_norm = sandbox_normalize_path_for_compare(target);
    let separator = std::path::MAIN_SEPARATOR.to_string();
    target_norm == base_norm
        || target_norm
            .strip_prefix(&(base_norm.clone() + &separator))
            .is_some()
}

fn sandbox_sanitize_normalized_path(path: &std::path::Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()))
            .join(path)
    };

    let mut normalized = PathBuf::new();
    let mut normal_depth = 0usize;
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
            }
            std::path::Component::RootDir => {
                normalized.push(component.as_os_str());
                normal_depth = 0;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if normal_depth > 0 {
                    normalized.pop();
                    normal_depth = normal_depth.saturating_sub(1);
                }
            }
            std::path::Component::Normal(seg) => {
                normalized.push(seg);
                normal_depth = normal_depth.saturating_add(1);
            }
        }
    }
    normalized
}

fn sandbox_normalize_target_for_access_check(path: &std::path::Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let sanitized = sandbox_sanitize_normalized_path(path);
    if let Some(parent) = sanitized.parent() {
        if let Ok(parent_canonical) = parent.canonicalize() {
            if let Some(name) = sanitized.file_name() {
                return parent_canonical.join(name);
            }
            return parent_canonical;
        }
    }
    sanitized
}

fn sandbox_workspace_canonical(state: &AppState) -> Result<PathBuf, String> {
    state
        .llm_workspace_path
        .canonicalize()
        .map_err(|err| format!("Resolve llm workspace failed: {err}"))
}

fn sandbox_allowed_project_roots_canonical(state: &AppState) -> Result<Vec<PathBuf>, String> {
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);
    let mut roots = Vec::<PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for ws in &config.shell_workspaces {
        let trimmed = ws.path.trim();
        if ws.name.trim().is_empty() || trimmed.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(trimmed);
        let canonical = match candidate.canonicalize() {
            Ok(path) if path.is_dir() => path,
            _ => continue,
        };
        let key = sandbox_normalize_path_for_compare(&canonical);
        if seen.insert(key) {
            roots.push(canonical);
        }
    }

    if roots.is_empty() {
        roots.push(sandbox_workspace_canonical(state)?);
    }
    Ok(roots)
}

fn sandbox_default_session_root_canonical(state: &AppState) -> Result<PathBuf, String> {
    let allowed = sandbox_allowed_project_roots_canonical(state)?;
    allowed
        .into_iter()
        .next()
        .ok_or_else(|| "No sandbox root available".to_string())
}

fn sandbox_session_root_canonical(
    state: &AppState,
    session_id: &str,
 ) -> Result<PathBuf, String> {
    let default_root = sandbox_default_session_root_canonical(state)?;
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

fn sandbox_path_allowed(
    state: &AppState,
    session_id: &str,
    target: &std::path::Path,
) -> Result<bool, String> {
    let root = sandbox_session_root_canonical(state, session_id)?;
    let target = sandbox_normalize_target_for_access_check(target);
    if sandbox_path_is_within(&root, &target) {
        return Ok(true);
    }
    Ok(false)
}

fn sandbox_assert_cwd_allowed(
    state: &AppState,
    session_id: &str,
    cwd: &std::path::Path,
) -> Result<(), String> {
    if sandbox_path_allowed(state, session_id, cwd)? {
        return Ok(());
    }
    Err(format!(
        "Working directory is outside current shell root: {}. Call shell_switch_workspace first.",
        cwd.to_string_lossy()
    ))
}
