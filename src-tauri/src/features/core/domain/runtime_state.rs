#[derive(Clone)]
struct AppState {
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    config_path: PathBuf,
    data_path: PathBuf,
    llm_workspace_path: PathBuf,
    shared_http_client: reqwest::Client,
    terminal_shell: TerminalShellProfile,
    terminal_shell_candidates: Vec<TerminalShellProfile>,
    conversation_lock: Arc<ConversationDomainLock>,
    memory_lock: Arc<Mutex<()>>,
    cached_config: Arc<Mutex<Option<AppConfig>>>,
    cached_config_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    cached_app_data: Arc<Mutex<Option<AppData>>>,
    cached_app_data_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    cached_app_data_dirty: Arc<std::sync::atomic::AtomicBool>,
    app_data_persist_pending: Arc<Mutex<Option<PendingAppDataPersist>>>,
    app_data_persist_notify: Arc<tokio::sync::Notify>,
    app_data_persist_started: Arc<std::sync::atomic::AtomicBool>,
    app_data_persist_latest_seq: Arc<std::sync::atomic::AtomicU64>,
    app_data_persist_write_lock: Arc<Mutex<()>>,
    last_panic_snapshot: Arc<Mutex<Option<String>>>,
    inflight_chat_abort_handles: Arc<Mutex<std::collections::HashMap<String, AbortHandle>>>,
    inflight_tool_abort_handles: Arc<Mutex<std::collections::HashMap<String, AbortHandle>>>,
    terminal_session_roots: Arc<Mutex<std::collections::HashMap<String, String>>>,
    terminal_live_sessions: Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, TerminalLiveShellSessionHandle>>,
    >,
    terminal_pending_approvals:
        Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    llm_round_logs: Arc<Mutex<std::collections::VecDeque<LlmRoundLogEntry>>>,
    conversation_runtime_slots:
        Arc<Mutex<std::collections::HashMap<String, ConversationRuntimeSlot>>>,
    conversation_processing_claims: Arc<Mutex<std::collections::HashSet<String>>>,
    pending_chat_result_senders: Arc<
        Mutex<
            std::collections::HashMap<
                String,
                tokio::sync::oneshot::Sender<Result<SendChatResult, String>>,
            >,
        >,
    >,
    pending_chat_delta_channels:
        Arc<Mutex<std::collections::HashMap<String, tauri::ipc::Channel<AssistantDeltaEvent>>>>,
    active_chat_view_bindings:
        Arc<Mutex<std::collections::HashMap<String, ActiveChatViewBinding>>>,
    dequeue_lock: Arc<Mutex<()>>,
    delegate_runtime_threads:
        Arc<Mutex<std::collections::HashMap<String, DelegateRuntimeThread>>>,
    delegate_recent_threads:
        Arc<Mutex<std::collections::VecDeque<DelegateRuntimeThread>>>,
    provider_streaming_disabled_keys: Arc<Mutex<std::collections::HashSet<String>>>,
    provider_system_message_user_fallback_keys:
        Arc<Mutex<std::collections::HashSet<String>>>,
    preferred_release_source: Arc<Mutex<String>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config_path", &self.config_path)
            .field("data_path", &self.data_path)
            .field("llm_workspace_path", &self.llm_workspace_path)
            .field("terminal_shell", &self.terminal_shell)
            .field("terminal_shell_candidates", &self.terminal_shell_candidates)
            .finish_non_exhaustive()
    }
}

impl AppState {
    fn new() -> Result<Self, String> {
        let legacy_project_dirs = ProjectDirs::from("ai", "easycall", "easy-call-ai")
            .ok_or_else(|| "Failed to resolve legacy config directory".to_string())?;
        let next_project_dirs = ProjectDirs::from("ai", "easycall", "p-ai")
            .ok_or_else(|| "Failed to resolve new config directory".to_string())?;
        let legacy_config_dir = legacy_project_dirs.config_dir().to_path_buf();
        let next_config_dir = next_project_dirs.config_dir().to_path_buf();
        let legacy_exists = legacy_config_dir.exists();
        let next_exists = next_config_dir.exists();
        let mut _migrated_legacy_to_new = false;

        let config_dir = if next_exists {
            next_config_dir.clone()
        } else if legacy_exists {
            if let Some(parent) = next_config_dir.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "Create new config parent directory failed ({}): {err}",
                        parent.display()
                    )
                })?;
            }
            fs::rename(&legacy_config_dir, &next_config_dir).map_err(|err| {
                format!(
                    "Migrate legacy config directory failed ({} -> {}): {err}",
                    legacy_config_dir.display(),
                    next_config_dir.display()
                )
            })?;
            _migrated_legacy_to_new = true;
            next_config_dir.clone()
        } else {
            fs::create_dir_all(&next_config_dir).map_err(|err| {
                format!(
                    "Create new config directory failed ({}): {err}",
                    next_config_dir.display()
                )
            })?;
            next_config_dir.clone()
        };
        fs::create_dir_all(&config_dir)
            .map_err(|err| format!("Create config directory failed: {err}"))?;
        let app_root = config_dir
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| config_dir.clone());
        let legacy_app_root = legacy_config_dir
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| legacy_config_dir.clone());
        for dir_name in ["avatars", "media", "exports"] {
            let legacy = config_dir.join(dir_name);
            let target = app_root.join(dir_name);
            if legacy.exists() && !target.exists() {
                fs::rename(&legacy, &target).map_err(|err| {
                    format!(
                        "Migrate legacy {dir_name} dir failed ({} -> {}): {err}",
                        legacy.display(),
                        target.display()
                    )
                })?;
            }
        }
        let llm_workspace_path = app_root.join("llm-workspace");
        for legacy_llm_workspace_path in [
            legacy_app_root.join("llm-workspace"),
            config_dir.join("llm-workspace"),
        ] {
            if legacy_llm_workspace_path.exists() && !llm_workspace_path.exists() {
                fs::rename(&legacy_llm_workspace_path, &llm_workspace_path).map_err(|err| {
                    format!(
                        "Migrate llm workspace failed ({} -> {}): {err}",
                        legacy_llm_workspace_path.display(),
                        llm_workspace_path.display()
                    )
                })?;
                break;
            }
        }
        fs::create_dir_all(&llm_workspace_path)
            .map_err(|err| format!("Create llm workspace failed: {err}"))?;
        let terminal_shell_candidates = detect_terminal_shell_candidates();
        let terminal_shell = detect_default_terminal_shell();
        let shared_http_client = reqwest::Client::builder()
            .user_agent(app_http_user_agent())
            .default_headers(app_identity_headers())
            .timeout(std::time::Duration::from_secs(12))
            .connect_timeout(std::time::Duration::from_secs(8))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|err| format!("Build shared HTTP client failed: {err}"))?;

        Ok(Self {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: config_dir.join("app_config.toml"),
            data_path: config_dir.join("app_data.json"),
            llm_workspace_path,
            shared_http_client,
            terminal_shell,
            terminal_shell_candidates,
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
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashSet::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
        })
    }
}

fn app_root_from_data_path(data_path: &PathBuf) -> PathBuf {
    let parent = data_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    let is_config_dir = parent
        .file_name()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("config"))
        .unwrap_or(false);
    if is_config_dir {
        if let Some(root) = parent.parent() {
            return root.to_path_buf();
        }
    }
    parent
}

fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn now_iso() -> String {
    now_utc_rfc3339()
}

fn parse_iso(value: &str) -> Option<OffsetDateTime> {
    parse_rfc3339_time(value)
}
