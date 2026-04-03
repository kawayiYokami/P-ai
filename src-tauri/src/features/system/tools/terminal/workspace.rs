fn normalize_terminal_tool_session_id(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "default-session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn terminal_workspace_path_from_conversation(conversation: &Conversation) -> Option<PathBuf> {
    let raw = conversation
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let path = PathBuf::from(raw);
    match path.canonicalize() {
        Ok(canonical) if canonical.is_dir() => Some(canonical),
        _ => None,
    }
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
    let data = state_read_app_data_cached(state)?;
    Ok(data
        .conversations
        .iter()
        .find(|item| item.id == conversation_id)
        .cloned())
}

fn terminal_session_has_locked_root(state: &AppState, session_id: &str) -> bool {
    let default_root = match terminal_default_session_root_canonical(state) {
        Ok(path) => path,
        Err(_) => return false,
    };
    let session_root = match terminal_session_root_canonical(state, session_id) {
        Ok(path) => path,
        Err(_) => return false,
    };
    normalize_terminal_path_for_compare(&session_root)
        != normalize_terminal_path_for_compare(&default_root)
}

fn normalize_terminal_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    let value = timeout_ms.unwrap_or(TERMINAL_DEFAULT_TIMEOUT_MS);
    value.clamp(1, TERMINAL_MAX_TIMEOUT_MS)
}

fn normalize_terminal_path_for_compare(path: &Path) -> String {
    let mut text = path.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        if let Some(stripped) = text.strip_prefix("\\\\?\\") {
            text = stripped.to_string();
        }
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

fn configured_workspace_root_from_shell_workspaces(
    shell_workspaces: &[ShellWorkspaceConfig],
    state: &AppState,
) -> PathBuf {
    for workspace in shell_workspaces {
        let path = normalize_terminal_path_input_for_current_platform(workspace.path.trim());
        if workspace.name.trim().is_empty() || path.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(&path);
        if candidate.is_absolute() {
            return candidate;
        }
        return state.llm_workspace_path.join(candidate);
    }
    state.llm_workspace_path.clone()
}

fn configured_workspace_root_from_config(config: &AppConfig, state: &AppState) -> PathBuf {
    configured_workspace_root_from_shell_workspaces(&config.shell_workspaces, state)
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
    let configured_default_root = configured_workspace_root_from_config(config, state);
    let default_path = terminal_path_for_user(&configured_default_root);
    let default_path_buf = PathBuf::from(&default_path);
    let legacy_default_path = legacy_default_shell_workspace_path();

    let match_default_index = |workspace: &ShellWorkspaceConfig| {
        let candidate = PathBuf::from(workspace.path.trim());
        terminal_paths_match(&candidate, &default_path_buf)
    };

    let target_index = config
        .shell_workspaces
        .iter()
        .position(|workspace| workspace.built_in)
        .or_else(|| {
            config
                .shell_workspaces
                .iter()
                .position(match_default_index)
        });

    let Some(target_index) = target_index else {
        config.shell_workspaces.insert(
            0,
            ShellWorkspaceConfig {
                name: "默认工作空间".to_string(),
                path: default_path,
                built_in: true,
            },
        );
        return true;
    };

    let moved_position = target_index != 0;
    let cleared_other_built_ins = config
        .shell_workspaces
        .iter()
        .enumerate()
        .any(|(index, workspace)| index != target_index && workspace.built_in);
    let mut target = config.shell_workspaces.remove(target_index);
    let previous_name = target.name.trim().to_string();
    let previous_path = target.path.trim().to_string();
    let previous_built_in = target.built_in;
    if target.name.trim().is_empty() {
        target.name = "默认工作空间".to_string();
    }
    let previous_path_buf = PathBuf::from(&previous_path);
    if target.built_in
        && legacy_default_path
            .as_deref()
            .map(|legacy_path| terminal_paths_match(&previous_path_buf, legacy_path))
            .unwrap_or(false)
        && !terminal_paths_match(&previous_path_buf, &default_path_buf)
    {
        target.path = default_path.clone();
        runtime_log_info(format!(
            "[终端工作空间迁移] 内置工作空间路径已更新: '{}' -> '{}'",
            previous_path, target.path
        ));
    }
    target.built_in = true;
    for workspace in &mut config.shell_workspaces {
        workspace.built_in = false;
    }
    config.shell_workspaces.insert(0, target);
    let current = &config.shell_workspaces[0];
    moved_position
        || cleared_other_built_ins
        || previous_built_in != current.built_in
        || previous_name != current.name.trim()
        || !terminal_paths_match(&previous_path_buf, &PathBuf::from(current.path.trim()))
}

fn terminal_allowed_workspaces_canonical(
    state: &AppState,
) -> Result<Vec<TerminalWorkspaceResolved>, String> {
    let mut config = read_config(&state.config_path)?;
    normalize_app_config(&mut config);
    let _ = ensure_default_shell_workspace_in_config(&mut config, state);
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

    // Prompt 展示只需要稳定的工作区文本，不应在聊天热路径里重复做
    // read_config + canonicalize 路径求真。真正的执行边界校验仍由 exec 路径负责。
    let current_root_text = configured_workspace_root_path(state)
        .map(|path| terminal_path_for_user(&path))
        .unwrap_or_else(|_| terminal_path_for_user(&state.llm_workspace_path));

    let mut lines = Vec::<String>::new();
    lines.push(format!("当前工作路径: {}", current_root_text));
    lines.push("当前 exec 工具默认在当前工作路径执行命令。".to_string());
    lines.push("请不要在命令中使用绝对路径。".to_string());
    lines.push("请不要脱离当前工作空间执行任务。".to_string());
    Some(prompt_xml_block("shell workspace", lines.join("\n")))
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
    if let Some(conversation) = terminal_session_conversation(state, session_id)? {
        if let Some(path) = terminal_workspace_path_from_conversation(&conversation) {
            return Ok(path);
        }
        return Ok(default_root);
    }
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
            provider_streaming_disabled_keys: Arc::new(Mutex::new(HashSet::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(HashSet::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
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
            name: "派蒙的家".to_string(),
            path: custom_workspace_path.to_string_lossy().to_string(),
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
            name: "派蒙的家".to_string(),
            path: legacy_workspace_path.to_string_lossy().to_string(),
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
    fn terminal_session_root_prefers_conversation_workspace_path() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-terminal-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let custom_workspace_path = temp_root.join("custom-shell-root");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&custom_workspace_path).expect("create custom workspace");
        let state = build_test_state(llm_workspace_path);
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conv-1".to_string(),
            title: "Conversation".to_string(),
            agent_id: "agent-1".to_string(),
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
            shell_workspace_path: Some(custom_workspace_path.to_string_lossy().to_string()),
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
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
}

