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

fn sandbox_has_session_path_access(
    state: &AppState,
    session_id: &str,
    target: &std::path::Path,
) -> bool {
    let normalized_target = sandbox_normalize_path_for_compare(target);
    let guard = match state.terminal_path_grants.lock() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    let grants = match guard.get(session_id) {
        Some(grants) => grants,
        None => return false,
    };

    grants.iter().any(|granted| {
        let separator = std::path::MAIN_SEPARATOR.to_string();
        normalized_target == *granted
            || normalized_target
                .strip_prefix(&(granted.clone() + &separator))
                .is_some()
    })
}

fn sandbox_path_allowed(
    state: &AppState,
    session_id: &str,
    target: &std::path::Path,
) -> Result<bool, String> {
    let workspace = sandbox_workspace_canonical(state)?;
    if sandbox_path_is_within(&workspace, target) {
        return Ok(true);
    }
    Ok(sandbox_has_session_path_access(state, session_id, target))
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
        "Working directory is not allowed yet: {}. Call terminal_request_path_access first.",
        cwd.to_string_lossy()
    ))
}
