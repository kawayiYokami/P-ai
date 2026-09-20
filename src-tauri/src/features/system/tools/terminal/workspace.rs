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

fn shell_workspace_default_access_for_level(level: &str) -> String {
    match level {
        SHELL_WORKSPACE_LEVEL_SYSTEM => SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        _ => SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
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

fn shell_workspace_display_name_from_input_or_path(raw_name: &str, path: &Path) -> String {
    let normalized_name = raw_name.trim();
    if !normalized_name.is_empty() {
        return normalized_name.to_string();
    }
    shell_workspace_display_name_fallback(path)
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
    delegate_session_conversation_id(&normalized)
}

fn terminal_session_conversation(state: &AppState, session_id: &str) -> Result<Option<Conversation>, String> {
    let Some(conversation_id) = terminal_session_conversation_id(session_id) else {
        return Ok(None);
    };
    if let Some(conversation) = delegate_runtime_thread_conversation_get_any(state, &conversation_id)? {
        return Ok(Some(conversation));
    }
    conversation_service_v2().try_get_conversation_snapshot_fast(state, &conversation_id)
}

/// 轻量读取：只取会话元数据（工作区/Shell 模式等），不整读消息正文。
/// 供会话工作区列表等 UI 轮询使用；需要消息内容的调用方仍走 terminal_session_conversation。
fn terminal_session_conversation_meta(state: &AppState, session_id: &str) -> Result<Option<Conversation>, String> {
    let Some(conversation_id) = terminal_session_conversation_id(session_id) else {
        return Ok(None);
    };
    if let Some(conversation) = delegate_runtime_thread_conversation_get_any(state, &conversation_id)? {
        return Ok(Some(conversation));
    }
    Ok(conversation_service_v2()
        .get_conversation_metadata_record(state, &conversation_id)
        .ok())
}

fn normalize_terminal_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    let value = timeout_ms.unwrap_or(TERMINAL_DEFAULT_TIMEOUT_MS);
    value.max(1)
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
        let text = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();
        #[cfg(target_os = "macos")]
        {
            for (canonical_prefix, user_prefix) in [
                ("/private/var", "/var"),
                ("/private/tmp", "/tmp"),
                ("/private/etc", "/etc"),
            ] {
                if text == canonical_prefix {
                    return user_prefix.to_string();
                }
                if let Some(suffix) = text.strip_prefix(&format!("{canonical_prefix}/")) {
                    return format!("{user_prefix}/{suffix}");
                }
            }
        }
        text
    }
}

fn path_is_within(base: &Path, target: &Path) -> bool {
    let base_norm = normalize_terminal_path_for_compare(base);
    let target_norm = normalize_terminal_path_for_compare(target);
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let base_prefix = if base_norm.ends_with(&separator) {
        base_norm.clone()
    } else {
        format!("{base_norm}{separator}")
    };
    target_norm == base_norm
        || target_norm.strip_prefix(&base_prefix).is_some()
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

fn terminal_conversation_shell_autonomous_mode(conversation: Option<&Conversation>) -> bool {
    conversation
        .map(|value| value.shell_autonomous_mode)
        .unwrap_or(false)
}

fn terminal_session_shell_autonomous_mode(state: &AppState, session_id: &str) -> Result<bool, String> {
    let conversation = terminal_session_conversation(state, session_id)?;
    Ok(terminal_conversation_shell_autonomous_mode(conversation.as_ref()))
}

fn terminal_autonomous_workspace_for_target(target: &Path) -> TerminalWorkspaceResolved {
    let normalized = terminal_normalize_for_access_check(target);
    let path = if normalized.is_file() {
        normalized
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| normalized.clone())
    } else {
        normalized
    };
    TerminalWorkspaceResolved {
        id: "conversation-autonomous".to_string(),
        name: "给予本会话最大权限".to_string(),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        built_in: false,
        path,
    }
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
                    "[终端工作空间迁移] 助理空间路径已更新: '{}' -> '{}'",
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

fn assistant_workspace_as_conversation_main_workspace(
    state: &AppState,
    config: &AppConfig,
) -> ShellWorkspaceConfig {
    let system_workspace = config.shell_workspaces.iter().find(|workspace| {
        normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_SYSTEM
    });
    let candidate = system_workspace
        .and_then(|workspace| shell_workspace_resolve_path_candidate(state, workspace))
        .unwrap_or_else(|| state.llm_workspace_path.clone());
    let name = system_workspace
        .map(|workspace| workspace.name.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| shell_workspace_display_name_fallback(&candidate));
    ShellWorkspaceConfig {
        id: "assistant-main-workspace".to_string(),
        name,
        path: terminal_path_for_user(&candidate),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        built_in: false,
    }
}

fn normalize_conversation_shell_workspaces_or_assistant_default(
    state: &AppState,
    config: &AppConfig,
    raw_entries: &[ShellWorkspaceConfig],
) -> Vec<ShellWorkspaceConfig> {
    let normalized = normalize_conversation_shell_workspaces(state, raw_entries);
    if normalized.is_empty() {
        vec![assistant_workspace_as_conversation_main_workspace(state, config)]
    } else {
        normalized
    }
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
        workspace.name = shell_workspace_display_name_from_input_or_path(&workspace.name, &candidate);
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
        workspace.name = shell_workspace_display_name_from_input_or_path(&workspace.name, &candidate);
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

/// 从联系人配置中解析该联系人会话对应的工作区列表。
/// 通过 conversation.id 反查 bound_conversation_id == conversation.id 的联系人。
fn resolve_contact_workspaces_for_conversation(
    state: &AppState,
    conversation: &Conversation,
) -> Vec<ShellWorkspaceConfig> {
    let conversation_id = conversation.id.trim();
    if conversation_id.is_empty() {
        return Vec::new();
    }
    let Ok(contacts) = state_service_list_remote_im_contacts(state, None) else {
        return Vec::new();
    };
    contacts
        .iter()
        .find(|contact| {
            contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                == Some(conversation_id)
        })
        .map(|contact| contact.shell_workspaces.clone())
        .unwrap_or_default()
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
        .ok_or_else(|| "No assistant space available".to_string())?;

    // 判断是否为联系人会话，若是则使用联系人配置的工作区，系统目录降为 read_only
    let is_contact_conversation = conversation
        .map(|c| c.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT)
        .unwrap_or(false);

    let mut out = Vec::<TerminalWorkspaceResolved>::new();
    let mut seen_paths = std::collections::HashSet::<String>::new();

    if is_contact_conversation {
        // 联系人会话：系统目录强制 read_only
        let mut forced_system = system_workspace.clone();
        forced_system.access = SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string();
        out.push(forced_system);
        seen_paths.insert(normalize_terminal_path_for_compare(&out[0].path));

        // 从联系人配置中加载工作区
        if let Some(conversation) = conversation {
            let contact_workspaces = resolve_contact_workspaces_for_conversation(state, conversation);
            for raw in normalize_conversation_shell_workspaces(state, &contact_workspaces) {
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
                        format!("contact-{}", key)
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
    } else {
        // 普通会话：原有逻辑
        out.push(system_workspace);
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
        .ok_or_else(|| "No assistant space available".to_string())
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
    if terminal_conversation_shell_autonomous_mode(conversation) {
        return Ok(Some(terminal_autonomous_workspace_for_target(target)));
    }
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

fn terminal_worktree_write_rejection(
    state: &AppState,
    session_id: &str,
    targets: &[PathBuf],
) -> Result<Option<String>, String> {
    let Some(conversation) = terminal_session_conversation(state, session_id)? else {
        return Ok(None);
    };
    if conversation.shell_autonomous_mode {
        return Ok(None);
    }
    let mode = normalize_shell_work_mode_text(&conversation.shell_work_mode);
    if mode == SHELL_WORK_MODE_DIRECTORY {
        return Ok(None);
    }
    // 统一 worktree：仅允许写入 .pai（含专用工作树），阻断其他工作树
    let root = terminal_default_workspace_for_conversation_resolved(state, Some(&conversation))?.path;
    let pai_dir = terminal_normalize_for_access_check(&root.join(".pai"));
    let worktree_dir = terminal_normalize_for_access_check(&pai_dir.join(".worktree"));
    let dedicated_dir = terminal_normalize_for_access_check(
        &worktree_dir.join(conversation.id.clone()),
    );
    let dedicated_dir_legacy = terminal_normalize_for_access_check(
        &worktree_dir.join(conversation.id.chars().take(8).collect::<String>()),
    );
    for target in targets {
        let target = terminal_normalize_for_access_check(target);
        if !path_is_within(&pai_dir, &target) {
            return Ok(Some(format!(
                "当前工作模式为“工作树”，写入目标“{}”不在允许范围内。工作树模式只能写入“{}”；请改用该目录下的路径。",
                terminal_path_for_user(&target),
                terminal_path_for_user(&pai_dir),
            )));
        }
        if path_is_within(&worktree_dir, &target)
            && !path_is_within(&dedicated_dir, &target)
            && !path_is_within(&dedicated_dir_legacy, &target)
        {
            return Ok(Some(format!(
                "当前工作模式为“工作树”，写入目标“{}”属于其他工作树。当前会话只能在“{}”修改项目；计划、Skill 等工作记录仍可写入“{}”。",
                terminal_path_for_user(&target),
                terminal_path_for_user(&dedicated_dir),
                terminal_path_for_user(&pai_dir),
            )));
        }
    }
    Ok(None)
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
                        "exec" | "write" | "delete" | "update" | "move"
                    )
            });
    if !terminal_enabled {
        return None;
    }

    let default_workspace = conversation
        .map(|value| terminal_default_workspace_for_conversation_resolved(state, Some(value)))
        .unwrap_or_else(|| terminal_default_workspace_resolved(state))
        .ok();
    let include_assistant_space = conversation
        .map(|value| value.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE)
        .unwrap_or(true);
    let assistant_space = if include_assistant_space {
        terminal_system_workspace_resolved(state).ok()
    } else {
        None
    };
    let runtime_shell = terminal_shell_for_state(state);

    let shell_title = match runtime_shell.kind.as_str() {
        "powershell7" => "PowerShell 7",
        "powershell5" => "Windows PowerShell 5.1",
        "git-bash" => "Git Bash",
        "missing-terminal-shell" => "Unavailable",
        other => other,
    };
    let mut lines = Vec::<String>::new();
    lines.push(format!("当前操作系统: {}", std::env::consts::OS));
    lines.push(format!("当前 shell: {}", shell_title));
    if runtime_shell.kind == "powershell5" {
        lines.push("检测到当前仅有 Windows PowerShell 5.1（未安装 Git Bash 与 PowerShell 7），该环境兼容性差，不适合作为自动化执行 shell。请主动帮助用户安装 Git for Windows（自带 Git Bash）：优先执行 winget install --id Git.Git --source winget；若因网络问题无法下载 Git，则尝试安装 PowerShell 7：winget install --id Microsoft.PowerShell --source winget。安装完成后，提示用户重启 PAI 使新 shell 生效。".to_string());
    }
    lines.push("说明: 当前工作目录是用户任务的默认执行目录。".to_string());
    if terminal_conversation_shell_autonomous_mode(conversation) {
        lines.push("当前会话已开启“给予本会话最大权限”：终端与补丁工具可访问任意目录，并跳过目录权限、智能评估与人工审批。".to_string());
    }
    if let Some(default_workspace) = &default_workspace {
        lines.push(format!(
            "{}：当前工作目录（Session Working Directory）",
            terminal_path_for_user(&default_workspace.path)
        ));
        let work_mode = conversation
            .map(|value| normalize_shell_work_mode_text(&value.shell_work_mode))
            .unwrap_or_else(default_shell_work_mode);
        if work_mode == SHELL_WORK_MODE_WORKTREE {
            let root = terminal_path_for_user(&default_workspace.path);
            let session_id = conversation.map(|value| value.id.clone()).unwrap_or_default();
            let worktree = format!("{root}/.pai/.worktree/{session_id}");
            lines.push(format!(
                "用户希望在工作树中工作。项目修改只能发生在「{}」；本项目的工作记录仍维护在「{}/.pai/**」，包括 plan、skill 等所有 .pai 文件。",
                worktree, root
            ));
            lines.push(format!(
                "指定工作树不存在时，先检查 Git 状态后自行在该固定目录创建；不得改用其他工作树，也不得修改原始项目根目录的项目文件。默认 cwd 保持「{}」，需要项目修改时请显式进入或指定该工作树。",
                root
            ));
            lines.push("创建前检查仓库根工作区是否存在未提交改动；如果任务依赖这些改动，先询问用户，不得自行提交、暂存、stash 或复制。".to_string());
            lines.push("注意不要让 .pai/ 被 Git 追踪。不要自动删除工作树或分支，除非用户明确要求。".to_string());
        } else {
            lines.push("用户希望直接在当前工作目录中工作，请将其作为本次任务的默认读取、修改和命令执行根目录。".to_string());
        }
        lines.push(".pai/ 是 PAI 的项目级资产目录（通常被 Git 忽略），存放与项目本身无关、不应残留进代码库的资产，请主动按目录职责管理与维护：\n- .pai/skills/：项目专用 Skill\n- .pai/mcp/：项目级 MCP 预留位置\n- .pai/plan/{domain}/：按领域归类的计划文件\n- .pai/report/：审查、调查、验收报告\n- .pai/workflow/：项目固定工作流\n- .pai/reference-projects/：外部参考仓库（独立 Git 状态）\n- .pai/.worktree/：隔离开发工作树\n- .pai/temp/：可丢弃的临时产物".to_string());
    }
    let shell_block = prompt_xml_block("shell workspace", lines.join("\n"));
    let assistant_block = assistant_space.map(|workspace| {
        prompt_xml_block(
            "assistant space",
            format!(
                "说明: 助理空间是 PAI 的配置目录与助理个人长期目录，用于跨项目记忆和个人配置。\n{}：PAI 助理空间（Assistant Space）",
                terminal_path_for_user(&workspace.path)
            ),
        )
    });
    let mut blocks = vec![shell_block];
    if let Some(assistant_block) = assistant_block {
        if !assistant_block.trim().is_empty() {
            blocks.push(assistant_block);
        }
    }
    Some(blocks.join("\n"))
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
    let mut missing = Vec::<std::ffi::OsString>::new();
    let mut cursor = path;
    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            return missing.into_iter().rev().fold(canonical, |base, name| base.join(name));
        }
        let Some(name) = cursor.file_name() else { break };
        missing.push(name.to_os_string());
        let Some(parent) = cursor.parent() else { break };
        cursor = parent;
    }
    path.to_path_buf()
}

// ========== Worktree 自动创建（首轮发送时） ==========

async fn ensure_conversation_worktree_internal(
    git_root: &str,
    worktree_path: &std::path::Path,
    branch: &str,
) -> Result<(), String> {
    let worktree_path_owned = worktree_path.to_path_buf();
    let git_root_owned = git_root.to_string();
    if worktree_path.exists() {
        let worktree_str = worktree_path_owned.to_string_lossy().to_string();
        let registered = git_executor()
            .run_read(&git_root_owned, &["worktree", "list", "--porcelain"])
            .await
            .map(|stdout| {
                stdout
                    .lines()
                    .any(|line| line.trim().starts_with("worktree ") && line.contains(&worktree_str))
            })
            .unwrap_or(false);
        let git_file_valid = worktree_path_owned.join(".git").exists()
            || std::fs::read_to_string(worktree_path_owned.join(".git"))
                .map(|content| content.contains("gitdir:"))
                .unwrap_or(false);
        if registered || git_file_valid {
            return Ok(());
        }
        return Err(format!(
            "工作树路径已存在但未注册为当前仓库的工作树：{}，请手动清理后重试",
            worktree_str
        ));
    }
    if let Some(parent) = worktree_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("无法创建工作树父目录：{err}"))?;
    }
    let branch_trimmed = branch.trim();
    let validated_branch = if branch_trimmed.is_empty() {
        None
    } else {
        Some(git_panel_validate_branch_name(branch_trimmed)?)
    };
    // 优先尝试带分支的 worktree add，失败时回退为 HEAD
    let worktree_str = worktree_path.to_string_lossy().to_string();
    let args_sets: Vec<Vec<String>> = if let Some(validated) = validated_branch {
        vec![
            vec!["worktree".to_string(), "add".to_string(), "--force".to_string(), worktree_str.clone(), validated.clone()],
            vec!["worktree".to_string(), "add".to_string(), worktree_str.clone(), validated],
            vec!["worktree".to_string(), "add".to_string(), worktree_str.clone()],
        ]
    } else {
        // 无分支：直接以 HEAD 创建
        vec![
            vec!["worktree".to_string(), "add".to_string(), worktree_str.clone()],
            vec!["worktree".to_string(), "add".to_string(), worktree_str.clone(), "HEAD".to_string()],
        ]
    };
    let mut last_err = String::new();
    for args in args_sets {
        let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let run = git_executor().run_write(git_root, &args_ref);
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), run).await;
        match result {
            Ok(Ok(output)) if output.exit_code == 0 => {
                runtime_log_info(format!(
                    "[工作树] 完成，任务=创建工作树，git_root={}，worktree={}，branch={}，args={:?}",
                    git_root,
                    worktree_str,
                    branch_trimmed,
                    args
                ));
                return Ok(());
            }
            Ok(Ok(output)) => {
                last_err = format!("git worktree add 失败：{} {}", output.stderr.trim(), output.stdout.trim());
                runtime_log_warn(format!(
                    "[工作树] 失败，任务=创建工作树，git_root={}，worktree={}，branch={}，args={:?}，stderr={}，stdout={}",
                    git_root, worktree_str, branch_trimmed, args, output.stderr.trim(), output.stdout.trim()
                ));
                // 若是“已存在”相关错误，直接视为成功（可能已被并发创建）
                let combined = format!("{} {}", output.stderr, output.stdout).to_ascii_lowercase();
                if combined.contains("already exists") || combined.contains("already checked out") || combined.contains("已存在") {
                    if worktree_path.exists() {
                        return Ok(());
                    }
                }
            }
            Ok(Err(err)) => {
                last_err = format!("git worktree add 执行失败：{err}");
                runtime_log_warn(format!(
                    "[工作树] 失败，任务=创建工作树，git_root={}，worktree={}，branch={}，args={:?}，error={}",
                    git_root, worktree_str, branch_trimmed, args, err
                ));
            }
            Err(_) => {
                last_err = "git worktree add 超时（30s）".to_string();
                runtime_log_warn(format!(
                    "[工作树] 超时，任务=创建工作树，git_root={}，worktree={}，branch={}",
                    git_root, worktree_str, branch_trimmed
                ));
                break;
            }
        }
    }
    Err(last_err)
}

pub(crate) async fn ensure_conversation_worktree(_state: &AppState, conversation: &Conversation) -> Result<(), String> {
    let mode = normalize_shell_work_mode_text(&conversation.shell_work_mode);
    if mode != SHELL_WORK_MODE_WORKTREE {
        return Ok(());
    }
    let main_ws = conversation
        .shell_workspaces
        .iter()
        .find(|w| w.level == SHELL_WORKSPACE_LEVEL_MAIN)
        .or_else(|| conversation.shell_workspaces.first());
    let Some(ws) = main_ws else {
        return Err("工作树模式需要主目录，当前会话未配置工作区".to_string());
    };
    let ws_path = ws.path.trim();
    if ws_path.is_empty() {
        return Err("工作树模式的主目录路径为空".to_string());
    }
    // 解析 Git 根
    let git_root = git_panel_resolve_root(ws_path).await.map_err(|err| {
        format!("工作树 Git 探测失败：{err}")
    })?;
    let worktree_path = std::path::PathBuf::from(&git_root).join(".pai").join(".worktree").join(conversation.id.clone());
    let legacy_path = std::path::PathBuf::from(&git_root).join(".pai").join(".worktree").join(conversation.id.chars().take(8).collect::<String>());
    if worktree_path.exists() || legacy_path.exists() {
        let check_path = if worktree_path.exists() { &worktree_path } else { &legacy_path };
        let check_str = check_path.to_string_lossy().to_string();
        let registered = git_executor()
            .run_read(&git_root, &["worktree", "list", "--porcelain"])
            .await
            .map(|stdout| {
                stdout
                    .lines()
                    .any(|line| line.trim().starts_with("worktree ") && line.contains(&check_str))
            })
            .unwrap_or(false);
        let git_file_valid = check_path.join(".git").exists()
            || std::fs::read_to_string(check_path.join(".git"))
                .map(|content| content.contains("gitdir:"))
                .unwrap_or(false);
        if registered || git_file_valid {
            if !worktree_path.exists() && legacy_path.exists() {
                runtime_log_info(format!(
                    "[工作树] 跳过，任务=创建工作树，原因=legacy已存在，git_root={}，legacy={}，worktree={}",
                    git_root,
                    legacy_path.display(),
                    worktree_path.display()
                ));
            }
            return Ok(());
        }
        if worktree_path.exists() {
            return Err(format!(
                "工作树路径已存在但未注册为当前仓库的工作树：{}，请手动清理后重试",
                check_str
            ));
        }
        // legacy 存在但未注册，继续走新建流程
    }
    let branch = normalize_shell_work_branch_text(&conversation.shell_work_branch);
    let _branch_for_log = if branch.is_empty() {
        // 尝试取当前分支作为兜底分支名
        let cur = normalize_shell_work_branch_text(&git_panel_current_branch(&git_root).await);
        let cur_trim = cur.trim().to_string();
        if cur_trim.is_empty() { branch.clone() } else { cur_trim }
    } else {
        branch.clone()
    };
    // 若 branch 为空，仍尝试以 HEAD 创建
    let effective_branch = if branch.trim().is_empty() {
        normalize_shell_work_branch_text(&git_panel_current_branch(&git_root).await)
    } else {
        branch.clone()
    };
    runtime_log_info(format!(
        "[工作树] 开始，任务=创建工作树，conversation_id={}，git_root={}，worktree={}，branch={}",
        conversation.id,
        git_root,
        worktree_path.display(),
        effective_branch
    ));
    ensure_conversation_worktree_internal(&git_root, &worktree_path, &effective_branch).await?;
    Ok(())
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
            data_path: llm_workspace_path.join("config_mark"),
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
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(HashMap::new())),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(HashMap::new())),
            migration_preview_dirs: Arc::new(Mutex::new(HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn build_workspace_test_conversation(conversation_id: &str) -> Conversation {
        let mut conversation = build_conversation_record(
            "api-1",
            DEFAULT_AGENT_ID,
            conversation_id,
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.id = conversation_id.to_string();
        conversation
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn path_is_within_should_allow_descendants_of_drive_root() {
        assert!(path_is_within(
            Path::new(r"D:\"),
            Path::new(r"D:\projects\demo")
        ));
        assert!(path_is_within(Path::new(r"D:\"), Path::new(r"D:\")));
        assert!(!path_is_within(
            Path::new(r"D:\"),
            Path::new(r"E:\projects\demo")
        ));
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
    fn normalize_conversation_shell_workspaces_should_keep_input_label() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("project-alpha");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path);

        let normalized = normalize_conversation_shell_workspaces(
            &state,
            &[ShellWorkspaceConfig {
                id: "workspace-1".to_string(),
                name: "前端乱传的标题".to_string(),
                path: custom_workspace_path.to_string_lossy().to_string(),
                level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
                built_in: false,
            }],
        );

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].name, "前端乱传的标题".to_string());

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn data_migration_should_fill_empty_unarchived_conversation_shell_workspaces() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("custom-root");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "天空岛".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(&state, &config).expect("write config");

        let empty_conversation = build_workspace_test_conversation("conv-empty-workspace");
        write_conversation_shard(&state.data_path, &empty_conversation)
            .expect("write empty conversation");
        let mut custom_conversation = build_workspace_test_conversation("conv-custom-workspace");
        custom_conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "custom-main".to_string(),
            name: "Custom".to_string(),
            path: terminal_path_for_user(&custom_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        write_conversation_shard(&state.data_path, &custom_conversation)
            .expect("write custom conversation");
        let mut archived_conversation = build_workspace_test_conversation("conv-archived-workspace");
        archived_conversation.status = "archived".to_string();
        archived_conversation.archived_at = Some(now_iso());
        write_conversation_shard(&state.data_path, &archived_conversation)
            .expect("write archived conversation");
        state_service_set_message_store_migration_version(
            &state,
            MESSAGE_STORE_MIGRATION_CURRENT_VERSION,
        )
        .expect("mark message store migration complete");

        let changed =
            run_app_data_migrations_with_state(&state, &config).expect("run migrations");

        assert!(changed);
        let migrated = read_conversation_shard(&state.data_path, "conv-empty-workspace")
            .expect("read migrated conversation");
        assert_eq!(migrated.shell_workspace_path, None);
        assert_eq!(migrated.shell_workspaces.len(), 1);
        assert_eq!(migrated.shell_workspaces[0].name, "天空岛");
        assert_eq!(
            migrated.shell_workspaces[0].path,
            terminal_path_for_user(&llm_workspace_path)
        );
        assert_eq!(
            migrated.shell_workspaces[0].level,
            SHELL_WORKSPACE_LEVEL_MAIN
        );
        assert_eq!(
            migrated.shell_workspaces[0].access,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS
        );
        let custom = read_conversation_shard(&state.data_path, "conv-custom-workspace")
            .expect("read custom conversation");
        assert_eq!(
            custom.shell_workspaces[0].path,
            terminal_path_for_user(&custom_workspace_path)
        );
        let archived = read_conversation_shard(&state.data_path, "conv-archived-workspace")
            .expect("read archived conversation");
        assert!(archived.shell_workspaces.is_empty());
        assert_eq!(
            state_service_get_data_migration_version(&state)
                .expect("read migration version"),
            DATA_MIGRATION_CURRENT_VERSION
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn assistant_workspace_label_sync_should_update_unarchived_matching_conversations() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("custom-root");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path.clone());
        let mut previous_config = AppConfig::default();
        previous_config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "旧助理空间".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        let mut next_config = previous_config.clone();
        next_config.shell_workspaces[0].name = "新助理空间".to_string();

        let mut assistant_conversation =
            build_workspace_test_conversation("conv-assistant-workspace");
        assistant_conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "assistant-main".to_string(),
            name: "旧助理空间".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        write_conversation_shard(&state.data_path, &assistant_conversation)
            .expect("write assistant conversation");
        let mut custom_conversation = build_workspace_test_conversation("conv-custom-workspace");
        custom_conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "custom-main".to_string(),
            name: "Custom".to_string(),
            path: terminal_path_for_user(&custom_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        write_conversation_shard(&state.data_path, &custom_conversation)
            .expect("write custom conversation");

        let changed = sync_assistant_workspace_label_for_unarchived_conversations(
            &state,
            &previous_config,
            &next_config,
        )
        .expect("sync assistant labels");

        assert_eq!(changed, 1);
        let assistant = read_conversation_shard(&state.data_path, "conv-assistant-workspace")
            .expect("read assistant conversation");
        assert_eq!(assistant.shell_workspaces[0].name, "新助理空间");
        assert_eq!(
            assistant.shell_workspaces[0].path,
            terminal_path_for_user(&llm_workspace_path)
        );
        assert_eq!(
            assistant.shell_workspaces[0].access,
            SHELL_WORKSPACE_ACCESS_APPROVAL
        );
        let custom = read_conversation_shard(&state.data_path, "conv-custom-workspace")
            .expect("read custom conversation");
        assert_eq!(custom.shell_workspaces[0].name, "Custom");

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
        let user_workspace_path = temp_root.join("demo-home");
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
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
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
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
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
    fn terminal_session_conversation_id_should_parse_remote_reply_delegate_session() {
        let session_id =
            "agent-a::conversation-sub::remote_reply_delegate:delegate-a".to_string();

        let conversation_id = terminal_session_conversation_id(&session_id);

        assert_eq!(conversation_id.as_deref(), Some("conversation-sub"));
    }

    #[test]
    fn terminal_match_workspace_should_grant_any_path_when_conversation_autonomous() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let outside_path = temp_root.join("outside-root").join("write-target.txt");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(outside_path.parent().expect("outside parent")).expect("create outside root");
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
        let mut conversation = build_conversation_record(
            "api-1",
            "agent-1",
            "Conversation",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.shell_autonomous_mode = true;

        let workspace = terminal_match_workspace_for_target_in_conversation(
            &state,
            Some(&conversation),
            &outside_path,
        )
        .expect("match workspace")
        .expect("autonomous workspace");

        assert_eq!(workspace.access, SHELL_WORKSPACE_ACCESS_FULL_ACCESS);
        assert_eq!(workspace.name, "给予本会话最大权限");

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
        assert!(!block.contains("请主动帮助用户安装 PowerShell 7"));
        assert!(block.contains("当前工作目录是用户任务的默认执行目录"));
        assert!(block.contains("助理空间是 PAI 的配置目录与助理个人长期目录"));
        assert!(block.contains("</shell workspace>\n<assistant space>"));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_prompt_trusted_roots_block_should_guide_powershell5_install() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-powershell5-prompt-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let mut state = build_test_state(llm_workspace_path.clone());
        state.terminal_shell_candidates = vec![TerminalShellProfile {
            kind: "powershell5".to_string(),
            path: r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".to_string(),
            args_prefix: vec!["-NoProfile".to_string(), "-Command".to_string()],
        }];
        state.terminal_shell = state.terminal_shell_candidates[0].clone();
        let mut config = AppConfig::default();
        config.terminal_shell_kind = "auto".to_string();
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

        assert!(block.contains("当前 shell: Windows PowerShell 5.1"));
        assert!(block.contains("请主动帮助用户安装 Git for Windows"));
        assert!(block.contains("winget install --id Git.Git --source winget"));
        assert!(block.contains("winget install --id Microsoft.PowerShell --source winget"));
        assert!(block.contains("提示用户重启 PAI"));
        assert!(!block.contains("当前 shell: PowerShell 7"));
        assert!(!block.contains("当前 shell: Git Bash"));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_prompt_trusted_roots_block_should_describe_isolated_worktree_mode() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-worktree-prompt-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).expect("create workspace");
        let state = build_test_state(temp_root.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "workspace-main".to_string(),
            name: "项目".to_string(),
            path: temp_root.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        state_write_config_cached(&state, &config).expect("write config");
        let mut conversation = build_workspace_test_conversation("worktree-prompt");
        conversation.shell_workspaces = config.shell_workspaces.clone();
        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        let mut api = ApiConfig::default();
        api.enable_tools = true;
        api.tools = vec![ApiToolConfig {
            id: "exec".to_string(),
            command: String::new(),
            args: Vec::new(),
            enabled: true,
            values: Value::Null,
        }];

        let block = terminal_prompt_trusted_roots_block(&state, &api, Some(&conversation))
            .expect("terminal block");

        assert!(block.contains("在工作树中工作"));
        assert!(block.contains(".pai/.worktree/"));
        assert!(block.contains("不要让 .pai/ 被 Git 追踪"));
        assert!(block.contains("不得自行提交、暂存、stash 或复制"));
        assert!(!block.contains("用户希望直接在当前工作目录中工作"));
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_prompt_trusted_roots_block_should_describe_independent_worktree_mode() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-independent-worktree-prompt-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_root).expect("create workspace");
        let state = build_test_state(temp_root.clone());
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "workspace-main".to_string(),
            name: "项目".to_string(),
            path: temp_root.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        state_write_config_cached(&state, &config).expect("write config");
        let mut conversation = build_workspace_test_conversation("a1b2c3d4-independent-worktree");
        conversation.shell_workspaces = config.shell_workspaces.clone();
        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        let mut api = ApiConfig::default();
        api.enable_tools = true;
        api.tools = vec![ApiToolConfig {
            id: "exec".to_string(),
            command: String::new(),
            args: Vec::new(),
            enabled: true,
            values: Value::Null,
        }];

        let block = terminal_prompt_trusted_roots_block(&state, &api, Some(&conversation))
            .expect("terminal block");

        assert!(block.contains("工作树"));
        assert!(block.contains(".pai/.worktree/a1b2c3d4"));
        assert!(block.contains("不得改用其他工作树"));
        assert!(block.contains("工作记录仍维护"));
        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn terminal_worktree_write_rejection_should_enforce_mode_boundaries() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-worktree-write-boundary-test-{}",
            uuid::Uuid::new_v4()
        ));
        let root = temp_root.join("project");
        let pai_dir = root.join(".pai");
        let dedicated_dir = pai_dir.join(".worktree").join("a1b2c3d4");
        let other_worktree_dir = pai_dir.join(".worktree").join("other-session");
        std::fs::create_dir_all(pai_dir.join("plan")).expect("create plan directory");
        std::fs::create_dir_all(&dedicated_dir).expect("create dedicated worktree directory");
        std::fs::create_dir_all(&other_worktree_dir).expect("create other worktree directory");
        let state = build_test_state(root.clone());
        let workspace = ShellWorkspaceConfig {
            id: "workspace-main".to_string(),
            name: "项目".to_string(),
            path: root.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        };
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![workspace.clone()];
        state_write_config_cached(&state, &config).expect("write config");
        let mut conversation = build_workspace_test_conversation("a1b2c3d4-conversation");
        conversation.shell_workspaces = vec![workspace];
        let session_id = normalize_terminal_tool_session_id(&inflight_chat_key(
            DEFAULT_AGENT_ID,
            Some(&conversation.id),
        ));
        let mut data = AppData::default();
        data.conversations.push(conversation.clone());

        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        data.conversations[0] = conversation.clone();
        state_write_app_data_cached(&state, &data).expect("write isolated conversation");
        let root_source_rejection = terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[root.join("src").join("main.rs")],
        )
        .expect("check isolated root source");
        assert!(root_source_rejection
            .as_deref()
            .is_some_and(|reason| reason.contains("允许范围")));
        assert!(terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[pai_dir.join("plan").join("record.md")],
        )
        .expect("check isolated plan")
        .is_none());

        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        data.conversations[0] = conversation.clone();
        state_write_app_data_cached(&state, &data).expect("write independent conversation");
        assert!(terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[dedicated_dir.join("src").join("main.rs")],
        )
        .expect("check dedicated worktree")
        .is_none());
        let other_worktree_rejection = terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[other_worktree_dir.join("src").join("main.rs")],
        )
        .expect("check other worktree");
        assert!(other_worktree_rejection
            .as_deref()
            .is_some_and(|reason| reason.contains("属于其他工作树")));

        conversation.shell_work_mode = SHELL_WORK_MODE_DIRECTORY.to_string();
        data.conversations[0] = conversation.clone();
        state_write_app_data_cached(&state, &data).expect("write directory conversation");
        assert!(terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[root.join("src").join("main.rs")],
        )
        .expect("check directory mode")
        .is_none());

        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        conversation.shell_autonomous_mode = true;
        data.conversations[0] = conversation;
        state_write_app_data_cached(&state, &data).expect("write autonomous conversation");
        assert!(terminal_worktree_write_rejection(
            &state,
            &session_id,
            &[root.join("src").join("main.rs")],
        )
        .expect("check autonomous mode")
        .is_none());
        let _ = std::fs::remove_dir_all(temp_root);
    }
}