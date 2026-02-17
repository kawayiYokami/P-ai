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

fn sandbox_workspace_canonical(state: &AppState) -> Result<PathBuf, String> {
    state
        .llm_workspace_path
        .canonicalize()
        .map_err(|err| format!("Resolve llm workspace failed: {err}"))
}

fn sandbox_session_root_canonical(
    state: &AppState,
    session_id: &str,
 ) -> Result<PathBuf, String> {
    let workspace = sandbox_workspace_canonical(state)?;
    let root_text = {
        let guard = state
            .terminal_session_roots
            .lock()
            .map_err(|_| "Failed to lock terminal session roots".to_string())?;
        guard.get(session_id).cloned()
    };
    let Some(root_text) = root_text else {
        return Ok(workspace);
    };

    let root = PathBuf::from(root_text);
    match root.canonicalize() {
        Ok(path) if path.is_dir() => Ok(path),
        _ => Ok(workspace),
    }
}

fn sandbox_path_allowed(
    state: &AppState,
    session_id: &str,
    target: &std::path::Path,
) -> Result<bool, String> {
    let root = sandbox_session_root_canonical(state, session_id)?;
    if sandbox_path_is_within(&root, target) {
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
        "Working directory is outside current terminal root: {}. Call terminal_request_path_access first.",
        cwd.to_string_lossy()
    ))
}
