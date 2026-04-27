fn normalize_terminal_tool_session_id(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "default-session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_shell_workspace_level_text(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        SHELL_WORKSPACE_LEVEL_SYSTEM => SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
        SHELL_WORKSPACE_LEVEL_MAIN => SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        SHELL_WORKSPACE_LEVEL_SECONDARY => SHELL_WORKSPACE_LEVEL_SECONDARY.to_string(),
        _ => String::new(),
    }
}

fn normalize_shell_workspace_access_text(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        SHELL_WORKSPACE_ACCESS_APPROVAL => SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
        SHELL_WORKSPACE_ACCESS_FULL_ACCESS => SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        SHELL_WORKSPACE_ACCESS_READ_ONLY => SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
        _ => String::new(),
    }
}

fn shell_workspace_default_access_for_level(level: &str) -> String {
    match level {
        SHELL_WORKSPACE_LEVEL_SYSTEM => SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        SHELL_WORKSPACE_LEVEL_MAIN => SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
        _ => SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
    }
}

fn shell_workspace_level_rank(level: &str) -> i32 {
    match level {
        SHELL_WORKSPACE_LEVEL_SYSTEM => 0,
        SHELL_WORKSPACE_LEVEL_MAIN => 1,
        _ => 2,
    }
}

fn shell_workspace_display_name_fallback(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn shell_workspace_resolve_path_candidate(
    state: &AppState,
    workspace: &ShellWorkspaceConfig,
) -> Option<PathBuf> {
    let normalized = normalize_terminal_path_input_for_current_platform(workspace.path.trim());
    if normalized.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(&normalized);
    if candidate.is_absolute() {
        Some(candidate)
    } else {
        Some(state.llm_workspace_path.join(candidate))
    }
}

fn configured_system_workspace_root_from_shell_workspaces(
    shell_workspaces: &[ShellWorkspaceConfig],
    state: &AppState,
) -> PathBuf {
    for workspace in shell_workspaces {
        if normalize_shell_workspace_level_text(&workspace.level) != SHELL_WORKSPACE_LEVEL_SYSTEM {
            continue;
        }
        if let Some(path) = shell_workspace_resolve_path_candidate(state, workspace) {
            return path;
        }
    }
    state.llm_workspace_path.clone()
}

fn terminal_workspace_path_from_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> Option<PathBuf> {
    let raw = conversation
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let path = PathBuf::from(raw);
    let canonical = match path.canonicalize() {
        Ok(value) if value.is_dir() => value,
        _ => return None,
    };
    let target_key = normalize_terminal_path_for_compare(&canonical);
    let workspaces = terminal_allowed_workspaces_for_conversation_canonical(state, Some(conversation)).ok()?;
    for workspace in workspaces {
        if normalize_terminal_path_for_compare(&workspace.path) == target_key {
            return Some(canonical);
        }
    }
    None
}

fn terminal_session_conversation_id(session_id: &str) -> Option<String> {
    let normalized = normalize_terminal_tool_session_id(session_id);
    let (_, conversation_id) = normalized.split_once("::")?;
    let trimmed = conversation_id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn terminal_session_conversation(state: &AppState, session_id: &str) -> Result<Option<Conversation>, String> {
    let Some(conversation_id) = terminal_session_conversation_id(session_id) else {
        return Ok(None);
    };
    if let Some(conversation) = delegate_runtime_thread_conversation_get_any(state, &conversation_id)? {
        return Ok(Some(conversation));
    }
    conversation_service().try_read_persisted_conversation(state, &conversation_id)
}

fn normalize_terminal_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    let value = timeout_ms.unwrap_or(TERMINAL_DEFAULT_TIMEOUT_MS);
    value.clamp(1, TERMINAL_MAX_TIMEOUT_MS)
}

fn normalize_terminal_path_for_compare(path: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        let text = path.to_string_lossy().to_string();
        if let Some(stripped) = text.strip_prefix("\\\\?\\") {
            return stripped.to_ascii_lowercase();
        }
        return text.to_ascii_lowercase();
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_string_lossy().to_string()
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

fn configured_workspace_root_from_config(config: &AppConfig, state: &AppState) -> PathBuf {
    configured_system_workspace_root_from_shell_workspaces(&config.shell_workspaces, state)
}

fn configured_workspace_root_path(state: &AppState) -> Result<PathBuf, String> {
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    let _ = ensure_default_shell_workspace_in_config(&mut config, state);
    Ok(configured_workspace_root_from_config(&config, state))
}

fn configured_workspace_root_canonical(state: &AppState) -> Result<PathBuf, String> {
    let root = configured_workspace_root_path(state)?;
    root.canonicalize()
        .map_err(|err| format!("Resolve configured workspace failed: {err}"))
}

fn ensure_workspace_root_ready(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root)
        .map_err(|err| format!("Create workspace root failed ({}): {err}", root.display()))?;
    root.canonicalize()
        .map_err(|err| format!("Resolve workspace root failed ({}): {err}", root.display()))
}

fn terminal_workspace_canonical(state: &AppState) -> Result<PathBuf, String> {
    configured_workspace_root_canonical(state)
}

#[derive(Debug, Clone)]
struct TerminalWorkspaceResolved {
    id: String,
    name: String,
    level: String,
    access: String,
    built_in: bool,
    path: PathBuf,
}

fn terminal_paths_match(left: &Path, right: &Path) -> bool {
    if let (Ok(left_canonical), Ok(right_canonical)) = (left.canonicalize(), right.canonicalize()) {
        return normalize_terminal_path_for_compare(&left_canonical)
            == normalize_terminal_path_for_compare(&right_canonical);
    }
    normalize_terminal_path_for_compare(left) == normalize_terminal_path_for_compare(right)
}

fn legacy_default_shell_workspace_path() -> Option<PathBuf> {
    ProjectDirs::from("ai", "easycall", "easy-call-ai").map(|dirs| {
        dirs.config_dir()
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| dirs.config_dir().to_path_buf())
            .join("llm-workspace")
    })
}

fn ensure_default_shell_workspace_in_config(config: &mut AppConfig, state: &AppState) -> bool {
    let original_snapshot = serde_json::to_string(&config.shell_workspaces).unwrap_or_default();
    let default_path = terminal_path_for_user(&state.llm_workspace_path);
    let default_path_buf = PathBuf::from(&default_path);
    let legacy_default_path = legacy_default_shell_workspace_path();
    let mut prepared = Vec::<(ShellWorkspaceConfig, PathBuf)>::new();
    for raw in std::mem::take(&mut config.shell_workspaces) {
        let Some(candidate) = shell_workspace_resolve_path_candidate(state, &raw) else {
            continue;
        };
        let normalized_path = terminal_path_for_user(&candidate);

        let mut workspace = raw.clone();
        workspace.path = normalized_path;
        workspace.id = workspace.id.trim().to_string();
        workspace.name = workspace.name.trim().to_string();
        prepared.push((workspace, candidate));
    }

    let explicit_system_index = prepared.iter().position(|(workspace, _)| {
        normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_SYSTEM
    });
    let recovery_system_index = explicit_system_index.and_then(|system_idx| {
        let (system_workspace, system_candidate) = &prepared[system_idx];
        let system_matches_default = terminal_paths_match(system_candidate, &default_path_buf)
            || legacy_default_path
                .as_deref()
                .map(|path| terminal_paths_match(system_candidate, path))
                .unwrap_or(false);
        if !system_workspace.built_in || !system_matches_default {
            return None;
        }
        prepared.iter().enumerate().find_map(|(idx, (workspace, _))| {
            if idx == system_idx {
                return None;
            }
            if workspace.name.trim().is_empty() {
                return None;
            }
            Some(idx)
        })
    });
    let selected_system_index = recovery_system_index
        .or(explicit_system_index)
        .or_else(|| prepared.iter().position(|_| true));

    let mut system = selected_system_index
        .and_then(|idx| prepared.into_iter().nth(idx))
        .map(|(mut workspace, candidate)| {
            if workspace.built_in
                && legacy_default_path
                    .as_deref()
                    .map(|legacy_path| terminal_paths_match(&candidate, legacy_path))
                    .unwrap_or(false)
                && !terminal_paths_match(&candidate, &default_path_buf)
            {
                workspace.path = default_path.clone();
                runtime_log_info(format!(
                    "[终端工作空间迁移] 助理私人目录路径已更新: '{}' -> '{}'",
                    candidate.display(),
                    workspace.path
                ));
            }
            if workspace.name.is_empty() {
                workspace.name = shell_workspace_display_name_fallback(&candidate);
            }
            workspace
        })
        .unwrap_or_else(|| ShellWorkspaceConfig {
        id: "system-workspace".to_string(),
        name: shell_workspace_display_name_fallback(&state.llm_workspace_path),
        path: default_path.clone(),
        level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
        access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        built_in: true,
    });
    if system.id.trim().is_empty() {
        system.id = "system-workspace".to_string();
    }
    system.level = SHELL_WORKSPACE_LEVEL_SYSTEM.to_string();
    if normalize_shell_workspace_access_text(&system.access).is_empty() {
        system.access = SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string();
    }
    system.built_in = true;
    if system.name.trim().is_empty() {
        system.name = shell_workspace_display_name_fallback(
            &shell_workspace_resolve_path_candidate(state, &system)
                .unwrap_or_else(|| state.llm_workspace_path.clone()),
        );
    }
    system.path = terminal_path_for_user(
        &shell_workspace_resolve_path_candidate(state, &system)
            .unwrap_or_else(|| state.llm_workspace_path.clone()),
    );
    config.shell_workspaces = vec![system];
    let current_snapshot = serde_json::to_string(&config.shell_workspaces).unwrap_or_default();
    original_snapshot != current_snapshot
}

fn normalize_conversation_shell_workspaces(
    state: &AppState,
    raw_entries: &[ShellWorkspaceConfig],
) -> Vec<ShellWorkspaceConfig> {
    let mut prepared = Vec::<(ShellWorkspaceConfig, PathBuf, String)>::new();
    for raw in raw_entries {
        let Some(candidate) = shell_workspace_resolve_path_candidate(state, raw) else {
            continue;
        };
        let normalized_path = terminal_path_for_user(&candidate);
        let path_key = normalize_terminal_path_for_compare(&PathBuf::from(&normalized_path));
        let mut workspace = raw.clone();
        workspace.path = normalized_path;
        workspace.id = workspace.id.trim().to_string();
        workspace.name = workspace.name.trim().to_string();
        workspace.level = if normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_MAIN {
            SHELL_WORKSPACE_LEVEL_MAIN.to_string()
        } else {
            SHELL_WORKSPACE_LEVEL_SECONDARY.to_string()
        };
        let access = normalize_shell_workspace_access_text(&workspace.access);
        workspace.access = if access.is_empty() {
            shell_workspace_default_access_for_level(&workspace.level)
        } else {
            access
        };
        workspace.built_in = false;
        prepared.push((workspace, candidate, path_key));
    }

    let mut rebuilt = Vec::<ShellWorkspaceConfig>::new();
    let mut seen_paths = std::collections::HashSet::<String>::new();
    for (mut workspace, candidate, path_key) in prepared {
        if !seen_paths.insert(path_key) {
            continue;
        }
        if workspace.name.is_empty() {
            workspace.name = shell_workspace_display_name_fallback(&candidate);
        }
        rebuilt.push(workspace);
    }
    if !rebuilt.is_empty()
        && !rebuilt
            .iter()
            .any(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_MAIN)
    {
        if let Some(first) = rebuilt.first_mut() {
            first.level = SHELL_WORKSPACE_LEVEL_MAIN.to_string();
            first.access = SHELL_WORKSPACE_ACCESS_APPROVAL.to_string();
        }
    }
    rebuilt.sort_by(|left, right| {
        shell_workspace_level_rank(&left.level)
            .cmp(&shell_workspace_level_rank(&right.level))
            .then_with(|| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()))
    });
    rebuilt
}

#[derive(Debug, Clone)]
struct TerminalConfigAllowedWorkspacesCacheEntry {
    signature: String,
    workspaces: Vec<TerminalWorkspaceResolved>,
}

fn terminal_config_allowed_workspaces_cache(
) -> &'static std::sync::Mutex<
    std::collections::HashMap<String, TerminalConfigAllowedWorkspacesCacheEntry>,
> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<String, TerminalConfigAllowedWorkspacesCacheEntry>,
        >,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn terminal_config_allowed_workspaces_cache_scope_key(state: &AppState) -> String {
    normalize_terminal_path_for_compare(&state.config_path)
}

fn clear_terminal_config_allowed_workspaces_cache_for_state(state: &AppState) {
    let scope_key = terminal_config_allowed_workspaces_cache_scope_key(state);
    let mut cache = terminal_workspace_cache_lock_recover(
        "terminal_config_allowed_workspaces",
        terminal_config_allowed_workspaces_cache(),
    );
    cache.remove(&scope_key);
}

fn terminal_shell_workspaces_cache_signature(
    state: &AppState,
    shell_workspaces: &[ShellWorkspaceConfig],
) -> String {
    let mut parts = vec![format!(
        "llm_workspace={}",
        normalize_terminal_path_for_compare(&state.llm_workspace_path)
    )];
    for workspace in shell_workspaces {
        parts.push(format!(
            "id={}|name={}|level={}|access={}|path={}|built_in={}",
            workspace.id.trim(),
            workspace.name.trim(),
            workspace.level.trim(),
            workspace.access.trim(),
            workspace.path.trim(),
            workspace.built_in
        ));
    }
    parts.join("||")
}

fn terminal_workspace_cache_lock_recover<'a, T>(
    label: &str,
    mutex: &'a std::sync::Mutex<T>,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(err) => {
            runtime_log_info(format!(
                "[终端工作区] 警告: {} 锁已 poison，继续恢复使用 error={:?}",
                label, err
            ));
            err.into_inner()
        }
    }
}

fn terminal_config_allowed_workspaces_canonical(
    state: &AppState,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    let _ = ensure_default_shell_workspace_in_config(&mut config, state);
    let cache_scope_key = terminal_config_allowed_workspaces_cache_scope_key(state);
    let cache_signature = terminal_shell_workspaces_cache_signature(state, &config.shell_workspaces);
    {
        let cache = terminal_workspace_cache_lock_recover(
            "terminal_config_allowed_workspaces",
            terminal_config_allowed_workspaces_cache(),
        );
        if let Some(entry) = cache.get(&cache_scope_key) {
            if entry.signature == cache_signature {
                return Ok(entry.workspaces.clone());
            }
        }
    }
    let mut out = Vec::<TerminalWorkspaceResolved>::new();
    let mut seen_paths = std::collections::HashSet::<String>::new();
    for raw in &config.shell_workspaces {
        let path = raw.path.trim();
        if path.is_empty() {
            continue;
        }
        let canonical = match PathBuf::from(path).canonicalize() {
            Ok(v) if v.is_dir() => v,
            _ => continue,
        };
        let key = normalize_terminal_path_for_compare(&canonical);
        if !seen_paths.insert(key.clone()) {
            continue;
        }
        let mut name = raw.name.trim().to_string();
        if name.is_empty() {
            name = shell_workspace_display_name_fallback(&canonical);
        }
        out.push(TerminalWorkspaceResolved {
            id: if raw.id.trim().is_empty() {
                format!("config-{}", key)
            } else {
                raw.id.trim().to_string()
            },
            name,
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
            path: canonical,
        });
    }
    if out.is_empty() {
        let fallback_path = terminal_workspace_canonical(state)?;
        out.push(TerminalWorkspaceResolved {
            id: "system-workspace".to_string(),
            name: shell_workspace_display_name_fallback(&fallback_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
            path: fallback_path,
        });
    }
    out.sort_by(|left, right| {
        shell_workspace_level_rank(&left.level)
            .cmp(&shell_workspace_level_rank(&right.level))
            .then_with(|| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()))
    });
    {
        let mut cache = terminal_workspace_cache_lock_recover(
            "terminal_config_allowed_workspaces",
            terminal_config_allowed_workspaces_cache(),
        );
        cache.insert(
            cache_scope_key,
            TerminalConfigAllowedWorkspacesCacheEntry {
                signature: cache_signature,
                workspaces: out.clone(),
            },
        );
    }
    Ok(out)
}

fn terminal_allowed_workspaces_for_conversation_canonical(
    state: &AppState,
    conversation: Option<&Conversation>,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    let config_workspaces = terminal_config_allowed_workspaces_canonical(state)?;
    let system_workspace = config_workspaces
        .iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_SYSTEM)
        .cloned()
        .or_else(|| config_workspaces.first().cloned())
        .ok_or_else(|| "No assistant private workspace available".to_string())?;

    let mut out = vec![system_workspace];
    let mut seen_paths = std::collections::HashSet::<String>::new();
    seen_paths.insert(normalize_terminal_path_for_compare(&out[0].path));

    if let Some(conversation) = conversation {
        for raw in normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces) {
            let canonical = match PathBuf::from(raw.path.trim()).canonicalize() {
                Ok(value) if value.is_dir() => value,
                _ => continue,
            };
            let key = normalize_terminal_path_for_compare(&canonical);
            if !seen_paths.insert(key.clone()) {
                continue;
            }
            let mut name = raw.name.trim().to_string();
            if name.is_empty() {
                name = shell_workspace_display_name_fallback(&canonical);
            }
            out.push(TerminalWorkspaceResolved {
                id: if raw.id.trim().is_empty() {
                    format!("conversation-{}", key)
                } else {
                    raw.id.trim().to_string()
                },
                name,
                level: raw.level.trim().to_string(),
                access: raw.access.trim().to_string(),
                built_in: false,
                path: canonical,
            });
        }
    }

    out.sort_by(|left, right| {
        shell_workspace_level_rank(&left.level)
            .cmp(&shell_workspace_level_rank(&right.level))
            .then_with(|| left.name.to_ascii_lowercase().cmp(&right.name.to_ascii_lowercase()))
    });
    Ok(out)
}

fn terminal_allowed_workspaces_canonical(
    state: &AppState,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    terminal_config_allowed_workspaces_canonical(state)
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

fn terminal_allowed_project_roots_for_session_canonical(
    state: &AppState,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let conversation = terminal_session_conversation(state, session_id)?;
    let mut roots = Vec::<PathBuf>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for ws in terminal_allowed_workspaces_for_conversation_canonical(state, conversation.as_ref())? {
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

fn terminal_system_workspace_resolved(state: &AppState) -> Result<TerminalWorkspaceResolved, String> {
    terminal_config_allowed_workspaces_canonical(state)?
        .into_iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_SYSTEM)
        .ok_or_else(|| "No assistant private workspace available".to_string())
}

fn terminal_default_workspace_resolved(state: &AppState) -> Result<TerminalWorkspaceResolved, String> {
    let workspaces = terminal_allowed_workspaces_canonical(state)?;
    if let Some(workspace) = workspaces
        .iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_MAIN)
    {
        return Ok(workspace.clone());
    }
    workspaces
        .into_iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_SYSTEM)
        .ok_or_else(|| "No default workspace available".to_string())
}

fn terminal_default_workspace_for_conversation_resolved(
    state: &AppState,
    conversation: Option<&Conversation>,
) -> Result<TerminalWorkspaceResolved, String> {
    let workspaces = terminal_allowed_workspaces_for_conversation_canonical(state, conversation)?;
    if let Some(workspace) = workspaces
        .iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_MAIN)
    {
        return Ok(workspace.clone());
    }
    workspaces
        .into_iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_SYSTEM)
        .ok_or_else(|| "No default workspace available".to_string())
}

fn terminal_match_workspace_for_target_in_conversation(
    state: &AppState,
    conversation: Option<&Conversation>,
    target: &Path,
) -> Result<Option<TerminalWorkspaceResolved>, String> {
    let normalized = terminal_normalize_for_access_check(target);
    let mut best_match: Option<TerminalWorkspaceResolved> = None;
    let mut best_len = 0usize;
    for workspace in terminal_allowed_workspaces_for_conversation_canonical(state, conversation)? {
        if !path_is_within(&workspace.path, &normalized) {
            continue;
        }
        let current_len = normalize_terminal_path_for_compare(&workspace.path).len();
        if current_len >= best_len {
            best_len = current_len;
            best_match = Some(workspace);
        }
    }
    Ok(best_match)
}

fn terminal_match_workspace_for_session_target(
    state: &AppState,
    session_id: &str,
    target: &Path,
) -> Result<Option<TerminalWorkspaceResolved>, String> {
    let conversation = terminal_session_conversation(state, session_id)?;
    terminal_match_workspace_for_target_in_conversation(state, conversation.as_ref(), target)
}

fn terminal_prompt_trusted_roots_block(
    state: &AppState,
    selected_api: &ApiConfig,
    conversation: Option<&Conversation>,
) -> Option<String> {
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

    let workspaces = conversation
        .map(|value| terminal_allowed_workspaces_for_conversation_canonical(state, Some(value)))
        .unwrap_or_else(|| terminal_allowed_workspaces_canonical(state))
        .ok()
        .unwrap_or_default();
    let system_workspace = workspaces
        .iter()
        .find(|workspace| workspace.level == SHELL_WORKSPACE_LEVEL_SYSTEM)
        .cloned()
        .or_else(|| terminal_system_workspace_resolved(state).ok());
    let default_workspace = conversation
        .map(|value| terminal_default_workspace_for_conversation_resolved(state, Some(value)))
        .unwrap_or_else(|| terminal_default_workspace_resolved(state))
        .ok();
    let runtime_shell = terminal_shell_for_state(state);

    let mut lines = Vec::<String>::new();
    lines.push(format!("当前操作系统: {}", std::env::consts::OS));
    lines.push(format!("当前 shell: {}", terminal_shell_runtime_label(&runtime_shell)));
    if let Some(system) = &system_workspace {
        lines.push(format!(
            "助理私人目录: {} [{} / {}] {}",
            system.name,
            system.level,
            system.access,
            terminal_path_for_user(&system.path)
        ));
    }
    if let Some(default_workspace) = &default_workspace {
        lines.push(format!(
            "Shell 默认启动/执行目录: {} [{} / {}] {}",
            default_workspace.name,
            default_workspace.level,
            default_workspace.access,
            terminal_path_for_user(&default_workspace.path)
        ));
    }
    if !workspaces.is_empty() {
        lines.push("当前允许的工作目录：".to_string());
        for workspace in &workspaces {
            lines.push(format!(
                "- {} [{} / {}] {}",
                workspace.name,
                workspace.level,
                workspace.access,
                terminal_path_for_user(&workspace.path)
            ));
        }
    }
    lines.push("显式绝对路径可用于读取；若绝对路径未命中任何已配置工作目录，则禁止写入。".to_string());
    lines.push("审批只用于 apply_patch 与明确写文件的终端命令；python/py 只有 full_access 才允许。".to_string());
    Some(prompt_xml_block("shell workspace", lines.join("\n")))
}

fn terminal_default_session_root_canonical(state: &AppState) -> Result<PathBuf, String> {
    Ok(terminal_default_workspace_resolved(state)?.path)
}

fn terminal_session_root_canonical(state: &AppState, session_id: &str) -> Result<PathBuf, String> {
    if let Some(conversation) = terminal_session_conversation(state, session_id)? {
        return Ok(terminal_default_workspace_for_conversation_resolved(state, Some(&conversation))?.path);
    }
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

#[cfg(test)]
mod terminal_workspace_tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn build_test_state(llm_workspace_path: PathBuf) -> AppState {
        let terminal_shell = detect_default_terminal_shell();
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: llm_workspace_path.join("app_config.toml"),
            data_path: llm_workspace_path.join("app_data.json"),
            llm_workspace_path,
            shared_http_client: reqwest::Client::new(),
            terminal_shell: terminal_shell.clone(),
            terminal_shell_candidates: vec![terminal_shell],
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_runtime_state: Arc::new(Mutex::new(None)),
            cached_runtime_state_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_chat_index_mtime: Arc::new(Mutex::new(None)),
            cached_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_chat_index_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            conversation_index_repair_gates: Arc::new(Mutex::new(HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
            migration_preview_dirs: Arc::new(Mutex::new(HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    #[test]
    fn ensure_default_shell_workspace_preserves_custom_built_in_path() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("outer-space");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path);
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "派蒙的家".to_string(),
            path: custom_workspace_path.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];

        let changed = ensure_default_shell_workspace_in_config(&mut config, &state);

        assert_eq!(config.shell_workspaces.len(), 1);
        assert_eq!(config.shell_workspaces[0].name, "派蒙的家".to_string());
        assert_eq!(
            config.shell_workspaces[0].path,
            terminal_path_for_user(&custom_workspace_path)
        );
        assert!(config.shell_workspaces[0].built_in);
        assert_eq!(config.shell_workspaces[0].level, SHELL_WORKSPACE_LEVEL_SYSTEM);
        assert_eq!(config.shell_workspaces[0].access, SHELL_WORKSPACE_ACCESS_FULL_ACCESS);
        assert!(!changed);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn ensure_default_shell_workspace_migrates_legacy_builtin_path_only() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let legacy_workspace_path = legacy_default_shell_workspace_path()
            .expect("legacy default workspace path");
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "派蒙的家".to_string(),
            path: legacy_workspace_path.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];

        let changed = ensure_default_shell_workspace_in_config(&mut config, &state);

        assert_eq!(config.shell_workspaces.len(), 1);
        assert_eq!(config.shell_workspaces[0].name, "派蒙的家".to_string());
        assert_eq!(
            config.shell_workspaces[0].path,
            terminal_path_for_user(&llm_workspace_path)
        );
        assert!(config.shell_workspaces[0].built_in);
        assert!(changed);

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn ensure_default_shell_workspace_prefers_user_workspace_over_auto_injected_default_system() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let user_workspace_path = temp_root.join("paimonhome");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&user_workspace_path).expect("create user workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![
            ShellWorkspaceConfig {
                id: "system-workspace".to_string(),
                name: "llm-workspace".to_string(),
                path: terminal_path_for_user(&llm_workspace_path),
                level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
                access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
                built_in: true,
            },
            ShellWorkspaceConfig {
                id: "secondary-workspace-1".to_string(),
                name: "派蒙的家".to_string(),
                path: terminal_path_for_user(&user_workspace_path),
                level: SHELL_WORKSPACE_LEVEL_SECONDARY.to_string(),
                access: SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
                built_in: false,
            },
        ];

        let changed = ensure_default_shell_workspace_in_config(&mut config, &state);

        assert!(changed);
        assert_eq!(config.shell_workspaces.len(), 1);
        assert_eq!(config.shell_workspaces[0].level, SHELL_WORKSPACE_LEVEL_SYSTEM);
        assert_eq!(
            config.shell_workspaces[0].path,
            terminal_path_for_user(&user_workspace_path)
        );
        assert_eq!(config.shell_workspaces[0].name, "派蒙的家".to_string());

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_session_root_prefers_conversation_main_workspace() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("custom-shell-root");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(&state, &config).expect("write config");
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conv-1".to_string(),
            title: "Conversation".to_string(),
            agent_id: "agent-1".to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: Some(custom_workspace_path.to_string_lossy().to_string()),
            shell_workspaces: vec![ShellWorkspaceConfig {
                id: "main-workspace-1".to_string(),
                name: "项目主目录".to_string(),
                path: terminal_path_for_user(&custom_workspace_path),
                level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
                built_in: false,
            }],
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        let session_id = normalize_terminal_tool_session_id(&inflight_chat_key(
            "agent-1",
            Some("conv-1"),
        ));
        let resolved = terminal_session_root_canonical(&state, &session_id).expect("resolve root");

        assert_eq!(
            normalize_terminal_path_for_compare(&resolved),
            normalize_terminal_path_for_compare(&custom_workspace_path)
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_session_root_should_ignore_stale_workspace_override() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let main_workspace_path = temp_root.join("main-root");
        let stale_locked_path = temp_root.join("stale-root");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&main_workspace_path).expect("create main workspace");
        std::fs::create_dir_all(&stale_locked_path).expect("create stale workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(&state, &config).expect("write config");
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conv-1".to_string(),
            title: "Conversation".to_string(),
            agent_id: "agent-1".to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: Some(stale_locked_path.to_string_lossy().to_string()),
            shell_workspaces: vec![ShellWorkspaceConfig {
                id: "main-workspace-1".to_string(),
                name: "项目主目录".to_string(),
                path: terminal_path_for_user(&main_workspace_path),
                level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
                built_in: false,
            }],
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        let session_id = normalize_terminal_tool_session_id(&inflight_chat_key(
            "agent-1",
            Some("conv-1"),
        ));
        let resolved = terminal_session_root_canonical(&state, &session_id).expect("resolve root");

        assert_eq!(
            normalize_terminal_path_for_compare(&resolved),
            normalize_terminal_path_for_compare(&main_workspace_path)
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_prompt_trusted_roots_block_should_use_configured_runtime_shell() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let mut state = build_test_state(llm_workspace_path.clone());
        state.terminal_shell_candidates = vec![
            TerminalShellProfile {
                kind: "git-bash".to_string(),
                path: r"C:\Program Files\Git\bin\bash.exe".to_string(),
                args_prefix: vec!["-lc".to_string()],
            },
            TerminalShellProfile {
                kind: "powershell7".to_string(),
                path: r"C:\Program Files\PowerShell\7\pwsh.exe".to_string(),
                args_prefix: vec!["-NoProfile".to_string(), "-Command".to_string()],
            },
        ];
        state.terminal_shell = state.terminal_shell_candidates[0].clone();
        let mut config = AppConfig::default();
        config.terminal_shell_kind = "powershell7".to_string();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(&state, &config).expect("write config");
        let mut api = ApiConfig::default();
        api.enable_tools = true;
        api.tools = vec![ApiToolConfig {
            id: "exec".to_string(),
            command: String::new(),
            args: Vec::new(),
            enabled: true,
            values: Value::Null,
        }];

        let block = terminal_prompt_trusted_roots_block(&state, &api, None).expect("terminal block");

        assert!(block.contains("PowerShell 7"));
        assert!(!block.contains("Git Bash"));

        let _ = std::fs::remove_dir_all(temp_root);
    }
}
