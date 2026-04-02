#[derive(Debug, Clone)]
struct ResolvedApiConfig {
    request_format: RequestFormat,
    base_url: String,
    api_key: String,
    model: String,
    temperature: Option<f64>,
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
struct PreparedHistoryMessage {
    role: String,
    text: String,
    extra_text_blocks: Vec<String>,
    user_time_text: Option<String>,
    images: Vec<(String, String)>,
    audios: Vec<(String, String)>,
    tool_calls: Option<Vec<Value>>,
    tool_call_id: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedPrompt {
    preamble: String,
    history_messages: Vec<PreparedHistoryMessage>,
    latest_user_text: String,
    latest_user_meta_text: String,
    latest_user_extra_text: String,
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct PendingAppDataPersist {
    seq: u64,
    data: AppData,
}

fn prepared_prompt_latest_user_text_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    for text in [
        prepared.latest_user_text.trim(),
        prepared.latest_user_meta_text.trim(),
        prepared.latest_user_extra_text.trim(),
    ] {
        if !text.is_empty() {
            blocks.push(text.to_string());
        }
    }
    if blocks.is_empty() {
        // Guardrail: never send an empty latest user turn to model providers.
        blocks.push(" ".to_string());
    }
    blocks
}

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
    // 运行态内存缓存：减少热路径重复读盘（配置）
    cached_config: Arc<Mutex<Option<AppConfig>>>,
    cached_config_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    // 运行态内存缓存：减少热路径重复读盘（业务数据）
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
    // 主聊天会话级运行时
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
        } else {
            if legacy_exists {
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
            }
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

fn path_modified_time(path: &PathBuf) -> Option<std::time::SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn state_read_config_cached(state: &AppState) -> Result<AppConfig, String> {
    let disk_mtime = path_modified_time(&state.config_path);
    {
        let cached = state
            .cached_config
            .lock()
            .map_err(|_| "Failed to lock cached config".to_string())?;
        let cached_mtime = state
            .cached_config_mtime
            .lock()
            .map_err(|_| "Failed to lock cached config mtime".to_string())?;
        if let (Some(config), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(config.clone());
            }
        }
    }

    let config = read_config(&state.config_path)?;
    let disk_mtime = path_modified_time(&state.config_path);
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    Ok(config)
}

fn state_write_config_cached(state: &AppState, config: &AppConfig) -> Result<(), String> {
    write_config(&state.config_path, config)?;
    let disk_mtime = path_modified_time(&state.config_path);
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    Ok(())
}

fn state_read_app_data_cached(state: &AppState) -> Result<AppData, String> {
    if state
        .cached_app_data_dirty
        .load(std::sync::atomic::Ordering::Acquire)
    {
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        if let Some(data) = cached.as_ref() {
            return Ok(data.clone());
        }
    }
    let disk_mtime = path_modified_time(&state.data_path);
    {
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        let cached_mtime = state
            .cached_app_data_mtime
            .lock()
            .map_err(|_| "Failed to lock cached app data mtime".to_string())?;
        if let (Some(data), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(data.clone());
            }
        }
    }

    let started = std::time::Instant::now();
    let data = read_app_data(&state.data_path)?;
    let disk_mtime = path_modified_time(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
    state
        .cached_app_data_dirty
        .store(false, std::sync::atomic::Ordering::Release);
    eprintln!(
        "[应用数据耗时] 读取完成 source=disk_read conversations={} elapsed_ms={}",
        data.conversations.len(),
        started.elapsed().as_millis()
    );
    Ok(data)
}

fn state_write_app_data_cached(state: &AppState, data: &AppData) -> Result<(), String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    let _write_guard = state
        .app_data_persist_write_lock
        .lock()
        .map_err(|_| "Failed to lock app data persist write lock".to_string())?;
    write_app_data(&state.data_path, data)?;
    let disk_mtime = path_modified_time(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
    if let Ok(mut pending) = state.app_data_persist_pending.lock() {
        if pending
            .as_ref()
            .map(|item| item.seq <= seq)
            .unwrap_or(false)
        {
            *pending = None;
        }
    }
    let has_newer_pending = state
        .app_data_persist_latest_seq
        .load(std::sync::atomic::Ordering::Acquire)
        > seq;
    state
        .cached_app_data_dirty
        .store(has_newer_pending, std::sync::atomic::Ordering::Release);
    Ok(())
}

fn state_schedule_app_data_persist(state: &AppState, data: &AppData) -> Result<u64, String> {
    let seq = state
        .app_data_persist_latest_seq
        .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
        + 1;
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    state
        .cached_app_data_dirty
        .store(true, std::sync::atomic::Ordering::Release);
    {
        let mut pending = state
            .app_data_persist_pending
            .lock()
            .map_err(|_| "Failed to lock pending app data persist".to_string())?;
        *pending = Some(PendingAppDataPersist {
            seq,
            data: data.clone(),
        });
    }
    state.app_data_persist_notify.notify_one();
    Ok(seq)
}

fn start_app_data_persist_worker(state: &AppState) -> Result<(), String> {
    let started = state.app_data_persist_started.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    );
    if started.is_err() {
        return Ok(());
    }
    let state_clone = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            state_clone.app_data_persist_notify.notified().await;
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            loop {
                let Some(pending) = ({
                    let mut slot = match state_clone.app_data_persist_pending.lock() {
                        Ok(slot) => slot,
                        Err(_) => {
                            runtime_log_error(
                                "[后台持久化] 失败，任务=读取待写入队列，error=lock poisoned"
                                    .to_string(),
                            );
                            break;
                        }
                    };
                    slot.take()
                }) else {
                    break;
                };

                let latest_seq = state_clone
                    .app_data_persist_latest_seq
                    .load(std::sync::atomic::Ordering::Acquire);
                if pending.seq < latest_seq {
                    continue;
                }
                let data_path = state_clone.data_path.clone();
                let data_to_write = pending.data.clone();
                let write_lock = state_clone.app_data_persist_write_lock.clone();
                let write_result = tokio::task::spawn_blocking(move || {
                    let _write_guard = write_lock.lock().map_err(|err| {
                        named_lock_error(
                            "app_data_persist_write_lock",
                            file!(),
                            line!(),
                            module_path!(),
                            &err,
                        )
                    })?;
                    write_app_data(&data_path, &data_to_write)?;
                    Ok::<Option<std::time::SystemTime>, String>(path_modified_time(&data_path))
                })
                .await;
                match write_result {
                    Ok(Ok(disk_mtime)) => {
                        if let Ok(mut cached) = state_clone.cached_app_data.lock() {
                            *cached = Some(pending.data.clone());
                        }
                        if let Ok(mut cached_mtime) = state_clone.cached_app_data_mtime.lock() {
                            *cached_mtime = disk_mtime;
                        }
                        let still_latest = state_clone
                            .app_data_persist_latest_seq
                            .load(std::sync::atomic::Ordering::Acquire)
                            == pending.seq;
                        if still_latest {
                            state_clone.cached_app_data_dirty.store(
                                false,
                                std::sync::atomic::Ordering::Release,
                            );
                        }
                    }
                    Ok(Err(err)) => {
                        runtime_log_error(format!(
                            "[后台持久化] 失败，任务=写入应用数据，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                    Err(err) => {
                        runtime_log_error(format!(
                            "[后台持久化] 失败，任务=阻塞写入任务，seq={}，error={}",
                            pending.seq, err
                        ));
                    }
                }
            }
        }
    });
    Ok(())
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

static LAST_PANIC_SNAPSHOT_SLOT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn init_last_panic_snapshot_slot(slot: Arc<Mutex<Option<String>>>) {
    let _ = LAST_PANIC_SNAPSHOT_SLOT.set(slot);
}

fn last_panic_snapshot_text() -> String {
    LAST_PANIC_SNAPSHOT_SLOT
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|v| v.clone()))
        .unwrap_or_default()
}

fn state_lock_error_with_panic(
    file: &str,
    line: u32,
    module_path: &str,
    err: &dyn std::fmt::Display,
) -> String {
    let panic_snapshot = last_panic_snapshot_text();
    if panic_snapshot.trim().is_empty() {
        return format!(
            "无法获取状态锁：{}（位置：{}:{} 模块：{}）",
            err, file, line, module_path
        );
    }
    format!(
        "无法获取状态锁：{}（位置：{}:{} 模块：{}；最近 panic：{}）",
        err, file, line, module_path, panic_snapshot
    )
}

fn named_lock_error(
    lock_name: &str,
    file: &str,
    line: u32,
    module_path: &str,
    err: &dyn std::fmt::Display,
) -> String {
    format!(
        "无法获取 {} 锁：{}（位置：{}:{} 模块：{}）",
        lock_name, err, file, line, module_path
    )
}

const CONVERSATION_LOCK_SLOW_WAIT_MS: u128 = 20;
const CONVERSATION_LOCK_SLOW_HOLD_MS: u128 = 20;

#[derive(Clone)]
struct ConversationLockOwnerSnapshot {
    task_name: String,
    acquired_at: std::time::Instant,
}

struct ConversationDomainLock {
    inner: Mutex<()>,
    owner: Mutex<Option<ConversationLockOwnerSnapshot>>,
}

impl ConversationDomainLock {
    fn new() -> Self {
        Self {
            inner: Mutex::new(()),
            owner: Mutex::new(None),
        }
    }

    #[track_caller]
    fn lock(&self) -> std::sync::LockResult<TimedConversationLockGuard<'_>> {
        let location = std::panic::Location::caller();
        let task_name = format!("{}:{}", location.file(), location.line());
        self.lock_named(&task_name)
    }

    fn lock_named(&self, task_name: &str) -> std::sync::LockResult<TimedConversationLockGuard<'_>> {
        let wait_started_at = std::time::Instant::now();
        let owner_before_wait = match self.inner.try_lock() {
            Ok(guard) => {
                return Ok(self.build_guard(guard, task_name.to_string()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                self.owner.lock().ok().and_then(|owner| owner.clone())
            }
            Err(std::sync::TryLockError::Poisoned(_)) => None,
        };
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                return Err(std::sync::PoisonError::new(
                    self.build_guard(poisoned.into_inner(), task_name.to_string()),
                ));
            }
        };
        let waited_ms = wait_started_at.elapsed().as_millis();
        if waited_ms >= CONVERSATION_LOCK_SLOW_WAIT_MS {
            if let Some(owner) = owner_before_wait {
                let owner_held_ms = owner.acquired_at.elapsed().as_millis();
                eprintln!(
                    "[会话锁] 等待完成: task={}, waited_ms={}, owner={}, owner_held_ms={}",
                    task_name, waited_ms, owner.task_name, owner_held_ms
                );
            } else {
                eprintln!(
                    "[会话锁] 等待完成: task={}, waited_ms={}",
                    task_name, waited_ms
                );
            }
        }
        Ok(self.build_guard(guard, task_name.to_string()))
    }

    fn build_guard<'a>(
        &'a self,
        guard: std::sync::MutexGuard<'a, ()>,
        task_name: String,
    ) -> TimedConversationLockGuard<'a> {
        let acquired_at = std::time::Instant::now();
        if let Ok(mut owner) = self.owner.lock() {
            *owner = Some(ConversationLockOwnerSnapshot {
                task_name: task_name.clone(),
                acquired_at,
            });
        }
        TimedConversationLockGuard {
            task_name,
            acquired_at,
            lock: self,
            _guard: guard,
        }
    }
}

struct TimedConversationLockGuard<'a> {
    task_name: String,
    acquired_at: std::time::Instant,
    lock: &'a ConversationDomainLock,
    _guard: std::sync::MutexGuard<'a, ()>,
}

impl Drop for TimedConversationLockGuard<'_> {
    fn drop(&mut self) {
        let held_ms = self.acquired_at.elapsed().as_millis();
        if let Ok(mut owner) = self.lock.owner.lock() {
            owner.take();
        }
        if held_ms >= CONVERSATION_LOCK_SLOW_HOLD_MS {
            eprintln!(
                "[会话锁] 持有完成: task={}, held_ms={}",
                self.task_name, held_ms
            );
        }
    }
}

fn lock_conversation_with_metrics<'a>(
    state: &'a AppState,
    task_name: &str,
) -> Result<TimedConversationLockGuard<'a>, String> {
    state
        .conversation_lock
        .lock_named(task_name)
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))
}

fn format_message_time_rfc3339_local(raw: &str) -> String {
    format_utc_storage_time_to_local_rfc3339(raw)
}

fn format_message_time_text(raw: &str) -> String {
    format_utc_storage_time_to_local_text(raw)
}

fn default_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: DEFAULT_AGENT_ID.to_string(),
        name: "助理".to_string(),
        system_prompt: "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_user_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: USER_PERSONA_ID.to_string(),
        name: "用户".to_string(),
        system_prompt: "我是...".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: true,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_system_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: SYSTEM_PERSONA_ID.to_string(),
        name: "凯瑟琳".to_string(),
        system_prompt: "我是系统人格，负责代表任务中心与系统调度向当前助手传达信息。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: true,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn normalize_agent_tools(agent: &mut AgentProfile) -> bool {
    let defaults = default_agent_tools();
    let mut next = Vec::<ApiToolConfig>::new();
    for default in defaults {
        if let Some(found) = agent.tools.iter().find(|tool| tool.id == default.id) {
            next.push(ApiToolConfig {
                id: default.id.clone(),
                command: if found.command.trim().is_empty() {
                    default.command.clone()
                } else {
                    found.command.clone()
                },
                args: if found.args.is_empty() {
                    default.args.clone()
                } else {
                    found.args.clone()
                },
                enabled: found.enabled,
                values: found.values.clone(),
            });
        } else {
            next.push(default);
        }
    }
    let changed = agent.tools.len() != next.len()
        || agent.tools.iter().zip(next.iter()).any(|(left, right)| {
            left.id != right.id
                || left.enabled != right.enabled
                || left.command != right.command
                || left.args != right.args
                || left.values != right.values
        });
    if changed {
        agent.tools = next;
    }
    changed
}

fn ensure_default_agent(data: &mut AppData) -> bool {
    let mut changed = false;
    let old_prompt = "You are a concise and helpful assistant.";
    let mut has_assistant = false;
    let mut has_user_persona = false;
    let mut has_system_persona = false;
    for agent in &mut data.agents {
        if normalize_agent_tools(agent) {
            changed = true;
        }
        if agent.source.trim().is_empty() {
            agent.source = default_main_source();
            changed = true;
        }
        if agent.scope.trim().is_empty() {
            agent.scope = default_global_scope();
            changed = true;
        }
        if agent.id == DEFAULT_AGENT_ID {
            has_assistant = true;
            if agent.is_built_in_user {
                agent.is_built_in_user = false;
                changed = true;
            }
            if agent.is_built_in_system {
                agent.is_built_in_system = false;
                changed = true;
            }
            if agent.name == "Default Agent" {
                agent.name = "助理".to_string();
                changed = true;
            }
            if agent.system_prompt == old_prompt {
                agent.system_prompt = "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string();
                changed = true;
            }
        } else if agent.id == USER_PERSONA_ID {
            has_user_persona = true;
            if !agent.is_built_in_user {
                agent.is_built_in_user = true;
                changed = true;
            }
            if agent.is_built_in_system {
                agent.is_built_in_system = false;
                changed = true;
            }
        } else if agent.id == SYSTEM_PERSONA_ID {
            has_system_persona = true;
            if !agent.is_built_in_system {
                agent.is_built_in_system = true;
                changed = true;
            }
        } else if !agent.is_built_in_user && !agent.is_built_in_system {
            has_assistant = true;
        }
    }
    if !has_assistant {
        data.agents.push(default_agent());
        changed = true;
    }
    if !has_user_persona {
        data.agents.push(default_user_persona());
        changed = true;
    }
    if !has_system_persona {
        data.agents.push(default_system_persona());
        changed = true;
    }
    if data.assistant_department_agent_id.trim().is_empty()
        || !data.agents.iter().any(|a| {
            a.id == data.assistant_department_agent_id
                && !a.is_built_in_user
                && !a.is_built_in_system
        })
    {
        data.assistant_department_agent_id = default_assistant_department_agent_id();
        changed = true;
    }
    let desired_alias = user_persona_name(data);
    if data.user_alias != desired_alias {
        data.user_alias = desired_alias;
        changed = true;
    }
    let desired_style = normalize_response_style_id(&data.response_style_id);
    if data.response_style_id != desired_style {
        data.response_style_id = desired_style;
        changed = true;
    }
    let desired_pdf_read_mode = normalize_pdf_read_mode(&data.pdf_read_mode);
    if data.pdf_read_mode != desired_pdf_read_mode {
        data.pdf_read_mode = desired_pdf_read_mode;
        changed = true;
    }
    let desired_screenshot_mode =
        normalize_background_voice_screenshot_mode(&data.background_voice_screenshot_mode);
    if data.background_voice_screenshot_mode != desired_screenshot_mode {
        data.background_voice_screenshot_mode = desired_screenshot_mode;
        changed = true;
    }
    if data.background_voice_screenshot_keywords.trim().is_empty() {
        data.background_voice_screenshot_keywords = default_background_voice_screenshot_keywords();
        changed = true;
    }
    changed
}

fn fill_missing_message_speaker_agent_ids(data: &mut AppData) -> bool {
    fn provider_meta_speaker_agent_id(message: &ChatMessage) -> Option<String> {
        let meta = message.provider_meta.as_ref()?;
        let object = meta.as_object()?;
        for key in [
            "speakerAgentId",
            "speaker_agent_id",
            "targetAgentId",
            "target_agent_id",
            "agentId",
            "agent_id",
            "sourceAgentId",
            "source_agent_id",
        ] {
            let value = object
                .get(key)
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    }

    let mut changed = false;
    for conversation in &mut data.conversations {
        let host_agent_id = conversation.agent_id.trim().to_string();
        if host_agent_id.is_empty() {
            continue;
        }
        for message in &mut conversation.messages {
            let current = message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                .unwrap_or("");
            if current.is_empty() {
                message.speaker_agent_id =
                    Some(provider_meta_speaker_agent_id(message).unwrap_or_else(|| {
                        if message.role == "user" {
                            USER_PERSONA_ID.to_string()
                        } else {
                            host_agent_id.clone()
                        }
                    }));
                changed = true;
            }
        }
    }
    changed
}

fn fill_missing_conversation_metadata(data: &mut AppData) -> bool {
    let mut changed = false;
    for conversation in &mut data.conversations {
        if conversation.conversation_kind.trim().is_empty() {
            conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    for archive in &mut data.archived_conversations {
        if archive
            .source_conversation
            .conversation_kind
            .trim()
            .is_empty()
        {
            archive.source_conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    changed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSettings {
    #[serde(alias = "selectedAgentId", alias = "selected_agent_id")]
    assistant_department_agent_id: String,
    user_alias: String,
    #[serde(default = "default_response_style_id")]
    response_style_id: String,
    #[serde(default = "default_pdf_read_mode")]
    pdf_read_mode: String,
    #[serde(default = "default_background_voice_screenshot_keywords")]
    background_voice_screenshot_keywords: String,
    #[serde(default = "default_background_voice_screenshot_mode")]
    background_voice_screenshot_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationApiSettings {
    #[serde(alias = "chatApiConfigId", alias = "chat_api_config_id")]
    assistant_department_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
}
