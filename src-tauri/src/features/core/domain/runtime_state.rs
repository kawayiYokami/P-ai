#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IdeContextReference {
    id: String,
    file_path: String,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    end_line: Option<u32>,
    content: String,
    #[serde(default)]
    language_id: Option<String>,
    source: String,
    // diff 编辑器里的文件来自某个提交/暂存版本，这里记下版本号；普通文件为空
    #[serde(default)]
    version: String,
    captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IdeContextSnapshot {
    client_id: String,
    editor: String,
    workspace_roots: Vec<String>,
    references: Vec<IdeContextReference>,
    updated_at: String,
}

/// 一个会话上「正在进行中的聊天轮次」的打断状态。
///
/// 表里**没有任何句柄**：轮次一旦跑起来就只认结束信号，自己停、自己收尾，
/// 谁都不许从外面把它掐掉。留下的身份信息只有一个用途——本轮收尾时认出
/// 「表里这条是我的」，从而只摘自己的记录，不把已被更晚一次调度接手的条目一起摘掉。
///
/// 两种状态：
/// - `Running`：轮次已宣布并在跑；
/// - `CancelRequested`：这一次提交还在启动途中（还没登记），停止落在它的请求号上；
///   登记时按请求号精确消费一次，被消费或本轮收尾即消失，绝不跨轮次存活。
#[derive(Debug)]
enum InflightChatAbortEntry {
    /// 轮次已开始，记下它属于哪个调度。
    ///
    /// 这条记录只用于「这个键上有没有在跑的轮次」与收尾认领，不承载任何中止手段。
    Running {
        /// 这一次调度的请求号（与前端发送时用的 traceId 一致）。
        request_id: String,
        /// 本轮正在写的助理消息 id。只用于排障：能说清停掉的是哪条消息。
        assistant_message_id: String,
        /// 本轮此前用过的请求号，最新一版在 `request_id` 上。
        ///
        /// 上下文压缩重启会把同一轮的请求号换成新的。收尾认领要按「用过的任一请求号」
        /// 才认得出自己——只比最初那个号会把重启过的这一轮认成别人的，条目便永远摘不掉。
        /// 只在一轮内累积，下一轮重新登记时清空。
        retired_request_ids: Vec<String>,
    },
    /// 这一次提交在轮次启动途中被停止：只认这个请求号，别的调度撞不上。
    CancelRequested {
        /// 被取消的那一次提交的请求号。
        request_id: String,
    },
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
    cached_config: Arc<Mutex<Option<AppConfig>>>,
    cached_config_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    cached_agents: Arc<Mutex<Option<Vec<AgentProfile>>>>,
    cached_agents_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    cached_chat_index: Arc<Mutex<Option<ChatIndexFile>>>,
    cached_conversation_metadata:
        Arc<Mutex<std::collections::HashMap<String, message_store::ConversationStoredMeta>>>,
    cached_conversation_field_metadata_ids:
        Arc<Mutex<std::collections::HashSet<String>>>,
    cached_conversation_mtimes:
        Arc<Mutex<std::collections::HashMap<String, Option<std::time::SystemTime>>>>,
    cached_app_data: Arc<Mutex<Option<AppData>>>,
    cached_app_data_signature: Arc<Mutex<Option<AppDataCacheSignature>>>,
    cached_app_data_dirty: Arc<std::sync::atomic::AtomicBool>,
    conversation_persist_pending: Arc<Mutex<Option<PendingConversationPersist>>>,
    conversation_persist_notify: Arc<tokio::sync::Notify>,
    conversation_persist_started: Arc<std::sync::atomic::AtomicBool>,
    conversation_persist_latest_seq: Arc<std::sync::atomic::AtomicU64>,
    cached_conversation_dirty_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    cached_deleted_conversation_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    app_data_persist_write_lock: Arc<Mutex<()>>,
    last_panic_snapshot: Arc<Mutex<Option<String>>>,
    inflight_chat_abort_handles:
        Arc<Mutex<std::collections::HashMap<String, InflightChatAbortEntry>>>,
    /// 已被要求停止的调度号（`request_id`）。
    ///
    /// 停止方把「要停的那一次调度」的号放进来，worker 在检查点问「我的号在不在里面」，
    /// 命中就自己收尾。新一轮的号天生不同，所以旧号留着也伤不到它——清理只是卫生，
    /// 不是正确性前置：漏清的那一条是死号，永远不会被任何新一轮匹配。
    stopped_request_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    /// 叫醒正在 `await` 的轮次：只拍一下，不承载任何信息（判据在 `stopped_request_ids`）。
    chat_stop_notify: Arc<tokio::sync::Notify>,
    inflight_completed_tool_history:
        Arc<Mutex<std::collections::HashMap<String, Vec<Value>>>>,
    terminal_session_roots: Arc<Mutex<std::collections::HashMap<String, String>>>,
    terminal_live_sessions: Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, TerminalLiveShellSessionHandle>>,
    >,
    terminal_background_shell_tasks: Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, TerminalBackgroundShellTaskHandle>>,
    >,
    terminal_pending_approvals:
        Arc<Mutex<std::collections::HashMap<String, PendingTerminalApprovalRequest>>>,
    schedule_events: Arc<Mutex<ScheduleEventStore>>,
    conversation_runtime_slots:
        Arc<Mutex<std::collections::HashMap<String, ConversationRuntimeSlot>>>,
    conversation_processing_claims: Arc<Mutex<std::collections::HashSet<String>>>,
    goal_continue_suppressed_conversation_ids:
        Arc<Mutex<std::collections::HashSet<String>>>,
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
    accepted_submit_trace_ids: Arc<Mutex<std::collections::VecDeque<String>>>,
    active_chat_view_bindings:
        Arc<Mutex<std::collections::HashMap<String, ActiveChatViewBinding>>>,
    conversation_list_activity_marks:
        Arc<Mutex<std::collections::HashMap<String, ConversationListActivityMark>>>,
    dequeue_lock: Arc<Mutex<()>>,
    timer_scheduler_notify: Arc<tokio::sync::Notify>,
    delegate_runtime_threads:
        Arc<Mutex<std::collections::HashMap<String, DelegateRuntimeThread>>>,
    delegate_recent_threads:
        Arc<Mutex<std::collections::VecDeque<DelegateRuntimeThread>>>,
    provider_streaming_disabled_keys: Arc<Mutex<std::collections::HashMap<String, i64>>>,
    provider_system_message_user_fallback_keys:
        Arc<Mutex<std::collections::HashSet<String>>>,
    provider_request_gates:
        Arc<tokio::sync::Mutex<std::collections::HashMap<String, Arc<ProviderRequestGate>>>>,
    remote_im_contact_runtime_states:
        Arc<Mutex<std::collections::HashMap<String, RemoteImContactRuntimeState>>>,
    remote_im_reply_delegate_runtimes:
        Arc<Mutex<std::collections::HashMap<String, RemoteImReplyDelegateRuntime>>>,
    remote_im_reply_delegate_semaphore: Arc<tokio::sync::Semaphore>,
    remote_im_channel_state_write_locks:
        Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>>,
    hidden_skill_snapshot_cache: Arc<Mutex<String>>,
    /// 按会话缓存的「项目专用 Skill」清单（来自会话绑定项目工作目录的 `.pai/skills/`）。
    /// 键为 conversation_id，值为该会话上次扫描结果；应用启动时不扫描，首次装配提示词时懒加载。
    conversation_project_skills_cache:
        Arc<Mutex<std::collections::HashMap<String, ConversationProjectSkillsCacheEntry>>>,
    preferred_release_source: Arc<Mutex<String>>,
    migration_preview_dirs: Arc<Mutex<std::collections::HashMap<String, String>>>,
    /// 当前活跃的委托线程 conversation_id 集合。
    /// 工具审批链路通过查表判断当前是否应跳过弹窗（有委托活跃 → 不弹窗，默认拒绝）。
    delegate_active_ids: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    /// 后端 setup 完成标记，前端在此标记为 true 之前不应发起数据加载。
    backend_ready: Arc<std::sync::atomic::AtomicBool>,
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

fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn portable_marker_path_from_exe_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join("PORTABLE")
}

fn portable_data_root_from_exe_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join("data")
}

fn detect_portable_runtime_root() -> Option<PathBuf> {
    let exe_dir = current_exe_dir()?;
    if portable_marker_path_from_exe_dir(&exe_dir).exists() {
        Some(portable_data_root_from_exe_dir(&exe_dir))
    } else {
        None
    }
}

fn resolve_standard_config_dirs() -> Result<(PathBuf, PathBuf), String> {
    let legacy_project_dirs = ProjectDirs::from("ai", "easycall", "easy-call-ai")
        .ok_or_else(|| "Failed to resolve legacy config directory".to_string())?;
    let next_project_dirs = ProjectDirs::from("ai", "easycall", "p-ai")
        .ok_or_else(|| "Failed to resolve new config directory".to_string())?;
    Ok((
        legacy_project_dirs.config_dir().to_path_buf(),
        next_project_dirs.config_dir().to_path_buf(),
    ))
}

fn resolve_standard_config_dir() -> Result<(PathBuf, PathBuf), String> {
    let (legacy_config_dir, next_config_dir) = resolve_standard_config_dirs()?;
    let legacy_exists = legacy_config_dir.exists();
    let next_exists = next_config_dir.exists();
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
    Ok((config_dir, legacy_config_dir))
}

impl AppState {
    fn new() -> Result<Self, String> {
        let (config_dir, _legacy_config_dir, app_root, legacy_app_root) =
            if let Some(portable_root) = detect_portable_runtime_root() {
                let config_dir = portable_root.join("config");
                fs::create_dir_all(&config_dir).map_err(|err| {
                    format!(
                        "Create portable config directory failed ({}): {err}",
                        config_dir.display()
                    )
                })?;
                (
                    config_dir,
                    portable_root.join("legacy-unused"),
                    portable_root.clone(),
                    portable_root,
                )
            } else {
                let (config_dir, legacy_config_dir) = resolve_standard_config_dir()?;
                let app_root = config_dir.clone();
                let legacy_app_root = legacy_config_dir
                    .parent()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| legacy_config_dir.clone());
                (config_dir, legacy_config_dir, app_root, legacy_app_root)
            };
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
            data_path: config_dir.join("config_mark"),
            llm_workspace_path,
            shared_http_client,
            terminal_shell,
            terminal_shell_candidates,
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
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            stopped_request_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            chat_stop_notify: Arc::new(tokio::sync::Notify::new()),
            inflight_completed_tool_history:
                Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            timer_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            conversation_project_skills_cache: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

/// 头像在数据根下的相对路径形态：`avatars/agent-<id>.webp`。
/// 存相对路径而非绝对路径，数据根整体搬迁（便携/导入导出）后依然可解析。
fn avatar_relative_path(file_name: &str) -> String {
    format!("avatars/{file_name}")
}

/// 头像在当前数据根下的绝对文件路径解析见 `avatar_path_to_absolute`。

/// 把存储的 `avatar_path` 解析成绝对路径。
/// - 相对路径（新形态）：拼当前数据根
/// - 绝对路径（历史数据）：原样返回
fn avatar_path_to_absolute(data_path: &PathBuf, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw.trim());
    if candidate.is_absolute() {
        candidate
    } else {
        app_root_from_data_path(data_path).join(candidate)
    }
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
