/// 内存环形缓冲上限（前端日志窗口读的是它，与落盘文件无关）。
const RUNTIME_LOG_MAX_BYTES: usize = 10 * 1024 * 1024;
/// 落盘日志的单文件上限：与旧实现一致（5MB 触发轮转）。
const RUNTIME_LOG_FILE_MAX_BYTES: u64 = 5 * 1024 * 1024;
/// 保留的轮转文件个数（插件按个数保留，没有总量上限）。
///
/// 20 × 5MB = 100MB 归档上界，加上当前文件共约 105MB；旧实现是「保留 20 个 + 归档总量 25MB」。
/// 按裁定取 20：保留份数与旧实现一致，总量上界因此从 25MB 抬到 100MB —— 这个数字要在回执里写明。
const RUNTIME_LOG_FILE_KEEP: usize = 20;
/// 日志文件名（不含扩展名）：插件按 `<dir>/<name>.log` 落地，轮转文件为 `<name>_<date>.log`。
const RUNTIME_LOG_FILE_NAME: &str = "backend";
/// 日志目录名，相对应用根（便携版即 `data/logs`）。
const RUNTIME_LOG_DIR_NAME: &str = "logs";
/// 统一 target：便于按 target 过滤与排障。
const RUNTIME_LOG_TARGET: &str = "p_ai";
/// 启动分隔线：区分两次启动的判据，不许丢。
const RUNTIME_LOG_STARTUP_BANNER: &str = "========== 本次启动开始 ==========";
/// logger 就绪前缓存待落盘条目的上限，避免启动期异常刷屏撑爆内存。
const RUNTIME_LOG_PENDING_MAX: usize = 5000;
const DEFAULT_LLM_ROUND_LOG_CAPACITY: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmRoundLogHeader {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmRoundLogStage {
    stage: String,
    elapsed_ms: u64,
    since_prev_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmRoundLogEntry {
    id: String,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    scene: String,
    request_format: String,
    provider: String,
    model: String,
    base_url: String,
    headers: Vec<LlmRoundLogHeader>,
    tools: Option<Value>,
    response: Option<Value>,
    error: Option<String>,
    elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<Vec<LlmRoundLogStage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    round_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rounds: Option<Vec<LlmRoundLogEntry>>,
    success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeLogEntry {
    id: String,
    created_at: String,
    level: String,
    message: String,
    repeat: usize,
    /// 这条日志属于哪个联系人（按字段过滤，不写在正文里）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    contact_id: Option<String>,
    /// 业务角色：库房 / 前台 / 秘书 / 专家。窗口按它分组显示。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Debug, Default)]
struct RuntimeLogBuffer {
    entries: std::collections::VecDeque<RuntimeLogEntry>,
    total_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryConversationStat {
    conversation_id: String,
    title: String,
    message_count: usize,
    estimated_json_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryCacheStats {
    generated_at: String,
    pid: u32,
    cached_conversations: usize,
    cached_conversations_message_count: usize,
    cached_conversations_estimated_json_bytes: usize,
    cached_conversation_metadata: usize,
    cached_conversation_metadata_estimated_json_bytes: usize,
    cached_chat_index_conversation_count: usize,
    cached_chat_index_estimated_json_bytes: usize,
    cached_app_data_loaded: bool,
    cached_app_data_image_text_cache_entries: usize,
    cached_app_data_pdf_text_cache_entries: usize,
    cached_app_data_pdf_image_cache_entries: usize,
    cached_app_data_estimated_json_bytes: usize,
    cached_conversation_dirty_ids: usize,
    cached_deleted_conversation_ids: usize,
    inflight_chat_abort_handles: usize,
    inflight_completed_tool_sessions: usize,
    inflight_completed_tool_event_count: usize,
    terminal_live_sessions: usize,
    terminal_session_roots: usize,
    terminal_pending_approvals: usize,
    schedule_event_runs: usize,
    schedule_event_events: usize,
    schedule_event_estimated_json_bytes: usize,
    conversation_runtime_slots: usize,
    conversation_runtime_stream_block_count: usize,
    pending_chat_result_senders: usize,
    pending_chat_delta_channels: usize,
    active_chat_view_bindings: usize,
    conversation_list_activity_marks: usize,
    delegate_runtime_threads: usize,
    delegate_runtime_thread_message_count: usize,
    delegate_runtime_threads_estimated_json_bytes: usize,
    delegate_recent_threads: usize,
    delegate_recent_thread_message_count: usize,
    delegate_recent_threads_estimated_json_bytes: usize,
    remote_im_contact_runtime_states: usize,
    provider_streaming_disabled_keys: usize,
    provider_system_message_user_fallback_keys: usize,
    provider_request_gates: usize,
    message_store_block_cache_entries: usize,
    message_store_block_cache_message_count: usize,
    message_store_block_cache_estimated_json_bytes: usize,
    message_store_index_cache_entries: usize,
    message_store_index_cache_item_count: usize,
    message_store_index_cache_estimated_json_bytes: usize,
    prompt_final_cache_entries: usize,
    prompt_agent_cache_entries: usize,
    prompt_environment_cache_entries: usize,
    abstract_message_projection_cache_entries: usize,
    abstract_message_projection_message_count: usize,
    screenshot_artifact_cache_entries: usize,
    screenshot_artifact_image_count: usize,
    tool_schema_cache_count: usize,
    mcp_cached_clients: usize,
    mcp_runtime_states: usize,
    mcp_runtime_tool_count: usize,
    ide_context_chat_clients: usize,
    top_cached_conversations: Vec<MemoryConversationStat>,
    top_metadata_conversations: Vec<MemoryConversationStat>,
    top_delegate_runtime_threads: Vec<MemoryConversationStat>,
    notes: Vec<String>,
}

fn runtime_log_buffer() -> &'static Mutex<RuntimeLogBuffer> {
    static RUNTIME_LOGS: OnceLock<Mutex<RuntimeLogBuffer>> = OnceLock::new();
    RUNTIME_LOGS.get_or_init(|| Mutex::new(RuntimeLogBuffer::default()))
}

/// 日志目录：与旧实现同一口径，桌面与 CLI 共用。
///
/// 便携版取 exe 同级 `data/logs`；标准安装取 `ProjectDirs` 的 config 目录下 `logs`；
/// 再退到 exe 同级 `logs`，最后退到临时目录。
fn resolve_runtime_log_dir() -> Option<PathBuf> {
    let log_dir = detect_portable_runtime_root()
        .or_else(|| {
            ProjectDirs::from("ai", "easycall", "p-ai")
                .map(|dirs| dirs.config_dir().to_path_buf())
        })
        .or_else(current_exe_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join(RUNTIME_LOG_DIR_NAME);
    if fs::create_dir_all(&log_dir).is_err() {
        return None;
    }
    Some(log_dir)
}

fn runtime_log_pending() -> &'static Mutex<std::collections::VecDeque<(log::Level, String)>> {
    type Pending = Mutex<std::collections::VecDeque<(log::Level, String)>>;
    static RUNTIME_LOG_PENDING: OnceLock<Pending> = OnceLock::new();
    RUNTIME_LOG_PENDING.get_or_init(|| Mutex::new(std::collections::VecDeque::new()))
}

/// pending 当前条数：flush 的快速路径用它避开无谓的全局锁，由 push/drain 在锁内维护。
fn runtime_log_pending_count() -> &'static std::sync::atomic::AtomicUsize {
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    &COUNT
}

/// 启动期 pending 满后被丢弃的条数，flush 时随日志一起报出来。
fn runtime_log_pending_dropped() -> &'static std::sync::atomic::AtomicUsize {
    static DROPPED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    &DROPPED
}

fn runtime_log_logger_ready_flag() -> &'static std::sync::atomic::AtomicBool {
    static RUNTIME_LOG_LOGGER_READY: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    &RUNTIME_LOG_LOGGER_READY
}

/// logger 是否已经装好。
///
/// `log` 在没装 logger 时 `max_level` 是 `Off`，装好之后由 logger 自己设置；
/// 用这个判据而不是自己记「初始化函数被调用过」，桌面插件的 logger 在 app 构建期才安装，
/// 初始化函数先调用也说明不了什么。
fn runtime_log_logger_ready() -> bool {
    let flag = runtime_log_logger_ready_flag();
    if flag.load(std::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    let ready = log::max_level() != log::LevelFilter::Off;
    if ready {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    ready
}

fn runtime_log_level(level: &str) -> log::Level {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => log::Level::Error,
        "warn" | "warning" => log::Level::Warn,
        "debug" => log::Level::Debug,
        "trace" => log::Level::Trace,
        _ => log::Level::Info,
    }
}

/// 桌面插件的日志配置：`backend.log` 落在应用日志目录，同时回显到 stderr。
///
/// 不用插件的 `LogDir`：那会固定落到系统用户目录，破坏便携版「数据跟着 exe 走」的语义。
fn runtime_log_plugin_builder() -> tauri_plugin_log::Builder {
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .level(log::LevelFilter::Trace)
        // 只收本应用自己的记录：全局 logger 会把所有依赖库的 log 也接进来，
        // 那些噪音会把 5MB 的轮转窗口瞬间打满，排障时反而找不到自己要的那几行。
        .filter(|metadata: &log::Metadata| metadata.target() == RUNTIME_LOG_TARGET)
        .max_file_size(RUNTIME_LOG_FILE_MAX_BYTES as u128)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(
            RUNTIME_LOG_FILE_KEEP,
        ))
        .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Stderr,
        ));
    if let Some(dir) = resolve_runtime_log_dir() {
        let log_file = dir.join(format!("{RUNTIME_LOG_FILE_NAME}.log"));
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            Ok(probe) => {
                // 立刻释放句柄，插件自己会重新打开；探一次是为了文件不可用时只降级、不阻止启动。
                drop(probe);
                builder = builder.target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: dir,
                        file_name: Some(RUNTIME_LOG_FILE_NAME.to_string()),
                    },
                ));
            }
            Err(err) => {
                let message = format!(
                    "[日志] 日志文件不可写，本次只输出到 stderr path={} err={err}",
                    log_file.display()
                );
                runtime_log_write_stderr(&message);
                // release 没有控制台，stderr 那句看不见；再进内存缓冲让前端日志窗口能看到。
                runtime_log_warn(message);
            }
        }
    }
    builder
}

/// 桌面路径不要调用：插件会自己装全局 logger，先装会冲突。
///
/// headless CLI 与 ACP 不构建 Tauri app，装不上插件，用插件 re-export 的 fern 自装一份，
/// 配置与桌面共用同一组常量：同一日志目录、同一 5MB 触发点、同一时区（本地）。
fn init_cli_file_logger() {
    static CLI_LOGGER_INSTALLED: OnceLock<()> = OnceLock::new();
    if CLI_LOGGER_INSTALLED.set(()).is_err() {
        return;
    }
    let mut dispatch = fern::Dispatch::new()
        .level(log::LevelFilter::Trace)
        // 与桌面插件同一口径：只写本应用自己的记录，不接依赖库的 log。
        .filter(|metadata: &log::Metadata| metadata.target() == RUNTIME_LOG_TARGET)
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} {:<5} {}",
                now_log_local_rfc3339(),
                record.level().as_str().to_uppercase(),
                message
            ))
        });
    if let Some(dir) = resolve_runtime_log_dir() {
        cleanup_stale_cli_log_files(&dir);
        // 按天分文件（backend-cli_2026-11-20.log）。文件名必须避开 backend_ 前缀：
        // 插件 remove_old_files 用 strip_prefix("backend") + strip_prefix("_") 认归档，
        // 用 backend_<日期>.log 会被它当成归档删掉；CLI 正持有时 Windows 上删除失败，
        // 那个 ? 冒到插件 setup，最终让 build 失败、窗口都出不来。
        // DateBased 没有字节上限，真正的上界是「一个自然日内这个进程写了多少」。
        dispatch = dispatch.chain(fern::DateBased::new(
            dir.join(format!("{RUNTIME_LOG_FILE_NAME}-cli_")),
            "%Y-%m-%d.log",
        ));
    }
    if !headless_cli_console_logging_suppressed() {
        dispatch = dispatch.chain(std::io::stderr());
    }
    if let Err(err) = dispatch.apply() {
        // 此后没有 logger：runtime_log_warn 只会进永不冲出的 pending，这里只能写 stderr。
        runtime_log_write_stderr(&format!(
            "[日志] 初始化 CLI 文件日志失败，将只保留内存缓冲: {err}"
        ));
    }
}

/// 清理 CLI 按天日志：当前命名 backend-cli_<日期>.log 按个数保留最近 KEEP 个，
/// 与桌面侧 keep=20 同口径；桌面归档名不碰、也不占保留名额。
/// 废弃命名（backend..<日期>.log / backend.<日期>.log / backend_<日期>.log）一律删除。
/// 其它文件不碰；失败只 warn，不影响 logger 初始化。
fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, byte)| i == 4 || i == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// v0.98.4 及更早的自研归档名：`backend.<unix_ts>.<pid>.log` 与
/// `backend.<unix_ts>.<pid>.idx.log`。这批孤儿文件在用户机器上真实存在，
/// 我们与插件两边的新清理都不认、只增不减，只能在这里删。
fn is_legacy_archive_name(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    // 末段之前必须全是纯数字段（时间戳、pid）。
    if !parts[..parts.len() - 1]
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let tail = parts[parts.len() - 1];
    !tail.is_empty() && (tail.bytes().all(|byte| byte.is_ascii_digit()) || tail == "idx")
}

fn cleanup_stale_cli_log_files(dir: &std::path::Path) {
    let mut dated: Vec<(String, PathBuf)> = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            runtime_log_warn(format!("[系统] 扫描日志目录失败 目录={} 错误={err}", dir.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        // 当前 CLI 命名，进保留名单。
        if let Some(date) = name
            .strip_prefix(&format!("{RUNTIME_LOG_FILE_NAME}-cli_"))
            .and_then(|rest| rest.strip_suffix(".log"))
        {
            if is_iso_date(date) {
                dated.push((date.to_string(), path));
            }
            continue;
        }

        // v0.98.4 及更早的自研归档：backend.<unix_ts>.<pid>[.idx].log，直接删除。
        if let Some(rest) = name
            .strip_prefix(&format!("{RUNTIME_LOG_FILE_NAME}."))
            .and_then(|rest| rest.strip_suffix(".log"))
        {
            if is_legacy_archive_name(rest) {
                if let Err(err) = fs::remove_file(&path) {
                    runtime_log_warn(format!("[系统] 清理旧日志失败 文件={} 错误={err}", path.display()));
                }
                continue;
            }
        }

        // 废弃命名：先剥前缀与 .log；双点名剩下的部分还带一个前导点，再剥一次。
        let legacy_date = name
            .strip_prefix(&format!("{RUNTIME_LOG_FILE_NAME}_"))
            .or_else(|| name.strip_prefix(&format!("{RUNTIME_LOG_FILE_NAME}.")))
            .and_then(|rest| rest.strip_suffix(".log"))
            .map(|rest| rest.strip_prefix('.').unwrap_or(rest));
        if let Some(date) = legacy_date {
            if is_iso_date(date) {
                if let Err(err) = fs::remove_file(&path) {
                    runtime_log_warn(format!("[系统] 清理旧日志失败 文件={} 错误={err}", path.display()));
                }
            }
        }
    }
    // 日期倒序，保留最新 KEEP 个，其余删；与桌面插件的 KeepSome(KEEP) 对齐。
    dated.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in dated.into_iter().skip(RUNTIME_LOG_FILE_KEEP) {
        if let Err(err) = fs::remove_file(&path) {
            runtime_log_warn(format!("[系统] 清理旧日志失败 文件={} 错误={err}", path.display()));
        }
    }
}

/// 启动分隔线：区分两次启动的判据，不许丢。
fn runtime_log_write_startup_banner() {
    runtime_log_info(RUNTIME_LOG_STARTUP_BANNER.to_string());
}

/// 无 Tauri app 的路径（headless CLI / ACP / 桌面进程里提前退出的分支）用这个：
/// 先装 CLI logger，再把 logger 就绪前积压的启动日志补写出去。
///
/// 逻辑：logger 已就绪时 `runtime_log_push` 会先把 pending 冲干净，所以上一条日志就够。
fn runtime_log_switch_to_cli_file_logger() {
    // logger 已就绪时只 flush：此时再 apply 会返回 Err，反而多打一条误报。
    if runtime_log_logger_ready() {
        runtime_log_flush_pending();
        return;
    }
    init_cli_file_logger();
    runtime_log_flush_pending();
}

/// 把 logger 就绪前缓存的条目补写到 logger。
///
/// 先整体取走再逐条写：写的过程中不持有 pending 锁，避免 logger 内部锁与它互相嵌套。
fn runtime_log_flush_pending() {
    if !runtime_log_logger_ready() {
        return;
    }
    // 快速路径：pending 为空时直接返回，不抢全局锁；计数由 push/drain 在锁内维护。
    if runtime_log_pending_count().load(std::sync::atomic::Ordering::Relaxed) == 0 {
        return;
    }
    let drained = match runtime_log_pending_drain() {
        Some(drained) => drained,
        None => return,
    };
    for (level, message) in drained {
        log::log!(target: RUNTIME_LOG_TARGET, level, "{}", message);
    }
    // 放在 drain 之后：先看到被缓存的日志，再看到「此前溢出丢弃 N 条」的合计。
    let dropped = runtime_log_pending_dropped().swap(0, std::sync::atomic::Ordering::Relaxed);
    if dropped > 0 {
        // logger 已 ready：这条 warn 会直接落盘并进内存缓冲；push 里再次调 flush 时
        // 计数已是 0，快速路径立即返回，不成环。
        runtime_log_warn(format!("[系统] 启动期溢出丢弃 {dropped} 条"));
    }
}

fn now_log_local_rfc3339() -> String {
    let now = now_utc();
    UtcOffset::current_local_offset()
        .ok()
        .map(|offset| now.to_offset(offset))
        .unwrap_or(now)
        .replace_nanosecond(0)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(now_utc_rfc3339)
}

/// stderr 直写：绕开 `runtime_log_*`，也绕开 `log` crate。
///
/// 这个编译单元里的 `eprintln!` 已被 main.rs 的宏重定义为 `runtime_log_info`，
/// 所以「必须出现在终端上、且不许等 logger 内部锁」的告警（panic、logger 安装失败）
/// 只能直接写 stderr，不能走 `eprintln!`。
fn runtime_log_write_stderr(text: &str) {
    let mut stderr = std::io::stderr();
    let _ = std::io::Write::write_all(&mut stderr, text.as_bytes());
    let _ = std::io::Write::write_all(&mut stderr, b"\n");
    let _ = std::io::Write::flush(&mut stderr);
}

fn install_runtime_log_panic_hook() {
    static RUNTIME_LOG_PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();
    if RUNTIME_LOG_PANIC_HOOK_INSTALLED.set(()).is_err() {
        return;
    }
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|value| (*value).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_string());
        let location = info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let thread_name = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let text = format!(
            "[panic] location={} thread={} payload={}",
            location.trim(),
            thread_name.trim(),
            payload.trim()
        );
        // 终端那行走 stderr 直写：不经过 runtime_log_*，也不经过 log crate，
        // 因此不会去等 logger 内部那把锁（这里的 `eprintln!` 已被宏重定义成 runtime_log_info，
        // 用它等于绕回 log::log!，规避是假的）。
        runtime_log_write_stderr(&text);
        // 落盘这一行是 best-effort：panic 若恰好发生在 logger 持锁区，`log::log!` 会等同一把锁。
        // 这条代价已知（旧实现用 try_lock 规避，是因为它自己持锁；logger 不给 try 接口）。
        log::log!(target: RUNTIME_LOG_TARGET, log::Level::Error, "{}", text);
        previous_hook(info);
    }));
}

fn is_ascii_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

/// 把日志消息里的标准 UUID（8-4-4-4-12）截断为前 8 位短 id，降低日志噪音。
/// 非 UUID 文本原样保留；只匹配完整 36 字符 UUID 形态，避免误伤
/// `delegate-xxx`、`persona-xxx`、`api-provider-xxx::api-model-xxx` 等带前缀 id。
fn shorten_uuids_in_log(message: &str) -> String {
    let bytes = message.as_bytes();
    let mut out = String::with_capacity(message.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some(36) = uuid_full_len_at(bytes, i) {
            out.push_str(&message[i..i + 8]);
            i += 36;
        } else {
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&message[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

/// 从 start 开始是否是一个完整 UUID（8-4-4-4-12，36 字符）。
/// 前置字符为字母数字或连字符时视为长 id 的一部分，不匹配。
fn uuid_full_len_at(bytes: &[u8], start: usize) -> Option<usize> {
    if start + 36 > bytes.len() {
        return None;
    }
    if start > 0 {
        let prev = bytes[start - 1];
        if prev.is_ascii_alphanumeric() || prev == b'-' {
            return None;
        }
    }
    let segments = [8usize, 4, 4, 4, 12];
    let mut pos = start;
    for (index, segment) in segments.iter().enumerate() {
        for _ in 0..*segment {
            if !is_ascii_hex_digit(bytes[pos]) {
                return None;
            }
            pos += 1;
        }
        if index < segments.len() - 1 {
            if bytes[pos] != b'-' {
                return None;
            }
            pos += 1;
        }
    }
    Some(36)
}

fn utf8_char_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else if first >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

fn normalize_runtime_log(level: &str, message: String) -> (String, String) {
    let mut current_level = level.to_string();
    let mut text = message.trim().to_string();
    let mappings = [
        ("[ERROR]", "error"),
        ("[WARN]", "warn"),
        ("[WARNING]", "warn"),
        ("[INFO]", "info"),
        ("[DEBUG]", "debug"),
        ("[TRACE]", "trace"),
    ];
    loop {
        let mut matched = false;
        for (prefix, mapped_level) in mappings {
            if let Some(rest) = text.strip_prefix(prefix) {
                current_level = mapped_level.to_string();
                text = rest.trim_start().to_string();
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }
    (current_level, text)
}

/// 入队并同步计数：满时丢最旧，计数与 deque 长度在锁内一起更新。
/// 入队的可测内核：满时丢最旧并累加 dropped，计数与 deque 长度同步。
fn pending_push_apply(
    pending: &mut std::collections::VecDeque<(log::Level, String)>,
    count: &std::sync::atomic::AtomicUsize,
    dropped: &std::sync::atomic::AtomicUsize,
    level: log::Level,
    message: String,
) {
    if pending.len() >= RUNTIME_LOG_PENDING_MAX {
        pending.pop_front();
        dropped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    pending.push_back((level, message));
    count.store(pending.len(), std::sync::atomic::Ordering::Relaxed);
}

/// 清空的可测内核：计数与 take 同步。
fn pending_drain_apply(
    pending: &mut std::collections::VecDeque<(log::Level, String)>,
    count: &std::sync::atomic::AtomicUsize,
) -> std::collections::VecDeque<(log::Level, String)> {
    count.store(0, std::sync::atomic::Ordering::Relaxed);
    std::mem::take(pending)
}

fn runtime_log_pending_push(level: log::Level, message: String) {
    let Ok(mut pending) = runtime_log_pending().lock() else {
        return;
    };
    pending_push_apply(
        &mut pending,
        runtime_log_pending_count(),
        runtime_log_pending_dropped(),
        level,
        message,
    );
}

/// 整体取走并清零计数：计数在锁内与 take 同步，供 flush 与单测共用。
fn runtime_log_pending_drain() -> Option<std::collections::VecDeque<(log::Level, String)>> {
    let mut pending = runtime_log_pending().lock().ok()?;
    Some(pending_drain_apply(&mut pending, runtime_log_pending_count()))
}

fn runtime_log_push(level: &str, message: String) {
    runtime_log_push_roles(level, message, None, None);
}

/// 带联系人归属与角色的日志：两者都走条目字段，不进正文。
pub(crate) fn runtime_log_push_roles(
    level: &str,
    message: String,
    contact_id: Option<String>,
    role: Option<String>,
) {
    let message = shorten_uuids_in_log(&message);
    let (normalized_level, normalized_message) = normalize_runtime_log(level, message);
    // 落盘与终端输出：logger 就绪前只排队，等插件（桌面）或 fern（CLI）装好再补写。
    // 终端落点由 logger 的 target 决定（桌面 Stderr，CLI 仅在不抑制控制台时才挂 stderr）。
    let log_level = runtime_log_level(&normalized_level);
    if runtime_log_logger_ready() {
        runtime_log_flush_pending();
        log::log!(
            target: RUNTIME_LOG_TARGET,
            log_level,
            "{}",
            normalized_message
        );
    } else {
        runtime_log_pending_push(log_level, normalized_message.clone());
    }
    let created_at = now_log_local_rfc3339();
    let Ok(mut buf) = runtime_log_buffer().lock() else {
        return;
    };
    if let Some(last) = buf.entries.back_mut() {
        if last.level == normalized_level
            && last.message == normalized_message
            && last.contact_id == contact_id
            && last.role == role
        {
            last.repeat = last.repeat.saturating_add(1);
            last.created_at = created_at;
            return;
        }
    }
    let entry = RuntimeLogEntry {
        id: Uuid::new_v4().to_string(),
        created_at,
        level: normalized_level,
        message: normalized_message,
        repeat: 1,
        contact_id,
        role,
    };
    let entry_bytes = entry.created_at.len() + entry.level.len() + entry.message.len();
    buf.total_bytes = buf.total_bytes.saturating_add(entry_bytes);
    buf.entries.push_back(entry);
    while buf.total_bytes > RUNTIME_LOG_MAX_BYTES {
        let Some(old) = buf.entries.pop_front() else {
            break;
        };
        let old_bytes = old.created_at.len() + old.level.len() + old.message.len();
        buf.total_bytes = buf.total_bytes.saturating_sub(old_bytes);
    }
}

fn runtime_log_info(message: String) {
    runtime_log_push("info", message);
}

fn runtime_log_warn(message: String) {
    runtime_log_push("warn", message);
}

fn runtime_log_error(message: String) {
    runtime_log_push("error", message);
}

fn runtime_log_debug(message: String) {
    runtime_log_push("debug", message);
}

fn mask_secret_keep_edges(value: &str) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= 4 {
        return "****".to_string();
    }
    let head = chars.iter().take(2).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}***{tail}")
}

fn masked_auth_headers(api_key: &str) -> Vec<LlmRoundLogHeader> {
    let masked = mask_secret_keep_edges(api_key);
    vec![
        LlmRoundLogHeader {
            name: "authorization".to_string(),
            value: format!("Bearer {masked}"),
        },
        LlmRoundLogHeader {
            name: "x-api-key".to_string(),
            value: masked,
        },
    ]
}

fn openai_input_audio_format_from_mime(mime: &str) -> String {
    let normalized = mime.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "audio/wav" | "audio/wave" | "audio/x-wav" => "wav".to_string(),
        "audio/mp3" | "audio/mpeg" => "mp3".to_string(),
        _ => normalized
            .split('/')
            .nth(1)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("wav")
            .to_string(),
    }
}

fn normalize_user_content(content: &Value) -> Value {
    let Value::Array(items) = content else {
        return content.clone();
    };
    if items.is_empty() {
        return Value::String(String::new());
    }
    let mut texts = Vec::<String>::new();
    for item in items {
        let Value::Object(obj) = item else {
            return content.clone();
        };
        if obj.get("type").and_then(Value::as_str) != Some("text") {
            return content.clone();
        }
        texts.push(
            obj.get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
    }
    if texts.len() == 1 {
        return Value::String(texts.remove(0));
    }
    content.clone()
}

fn normalize_prepared_prompt_messages(messages: &mut [Value]) {
    for msg in messages.iter_mut() {
        let Value::Object(obj) = msg else {
            continue;
        };
        if obj.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let Some(content) = obj.get("content").cloned() else {
            continue;
        };
        obj.insert("content".to_string(), normalize_user_content(&content));
    }
}

fn prepared_prompt_latest_user_text_blocks_for_json(prepared: &PreparedPrompt) -> Vec<String> {
    prepared_prompt_latest_user_text_blocks(prepared)
}

fn prepared_prompt_to_messages_json(prepared: &PreparedPrompt) -> Vec<Value> {
    let mut messages = Vec::<Value>::new();
    if !prepared.preamble.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prepared.preamble
        }));
    }

    let normalized_history_messages = normalized_prepared_history_messages(&prepared.history_messages);
    for hm in &normalized_history_messages {
        if hm.role == "assistant" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("assistant".to_string()));
            if hm.text.trim().is_empty() {
                msg.insert("content".to_string(), Value::Null);
            } else {
                msg.insert("content".to_string(), Value::String(hm.text.clone()));
            }
            if let Some(reasoning) = &hm.reasoning_content {
                msg.insert("reasoning_content".to_string(), Value::String(reasoning.clone()));
            }
            if let Some(calls) = &hm.tool_calls {
                msg.insert(
                    "tool_calls".to_string(),
                    Value::Array(
                        normalize_prompt_tool_calls(calls)
                            .iter()
                            .filter_map(normalized_tool_call_to_history_value)
                            .collect(),
                    ),
                );
            }
            messages.push(Value::Object(msg));
            continue;
        }

        if hm.role == "tool" {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("tool".to_string()));
            msg.insert("content".to_string(), Value::String(hm.text.clone()));
            if let Some(call_id) = &hm.tool_call_id {
                msg.insert("tool_call_id".to_string(), Value::String(call_id.clone()));
            }
            messages.push(Value::Object(msg));
            continue;
        }

        if hm.role == "user" {
            // 历史消息冻结：与真实请求同一入口 prepared_history_user_text_blocks。
            let mut content = Vec::<Value>::new();
            for block in prepared_history_user_text_blocks(hm) {
                content.push(serde_json::json!({
                    "type": "text",
                    "text": block,
                }));
            }
            for image in &hm.images {
                if image.mime.trim().eq_ignore_ascii_case("application/pdf") {
                    content.push(serde_json::json!({
                        "type": "file",
                        "mime": image.mime,
                        "bytesBase64": image.content
                    }));
                } else {
                    let image_url = if is_remote_binary_url(&image.content) {
                        image.content.clone()
                    } else {
                        format!("data:{};base64,{}", image.mime, image.content)
                    };
                    content.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": image_url,
                            "detail": "auto"
                        }
                    }));
                }
            }
            for audio in &hm.audios {
                content.push(serde_json::json!({
                    "type": "input_audio",
                    "input_audio": {
                        "data": audio.content.clone(),
                        "format": openai_input_audio_format_from_mime(&audio.mime)
                    }
                }));
            }
            // 空消息（无文本块且无媒体）不进请求体，日志预览与生产一致
            if content.is_empty() {
                continue;
            }
            messages.push(serde_json::json!({
                "role": "user",
                "content": content,
            }));
            continue;
        }

        messages.push(serde_json::json!({
            "role": hm.role,
            "content": hm.text
        }));
    }

    let mut latest_user_content = Vec::<Value>::new();
    for text_block in prepared_prompt_latest_user_text_blocks_for_json(prepared) {
        latest_user_content.push(serde_json::json!({
            "type": "text",
            "text": text_block
        }));
    }
    for image in &prepared.latest_images {
        if image.mime.trim().eq_ignore_ascii_case("application/pdf") {
            latest_user_content.push(serde_json::json!({
                "type": "file",
                "mime": image.mime,
                "bytesBase64": image.content
            }));
        } else {
            let image_url = if is_remote_binary_url(&image.content) {
                image.content.clone()
            } else {
                format!("data:{};base64,{}", image.mime, image.content)
            };
            latest_user_content.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": image_url,
                    "detail": "auto"
                }
            }));
        }
    }
    for audio in &prepared.latest_audios {
        latest_user_content.push(serde_json::json!({
            "type": "input_audio",
            "input_audio": {
                "data": audio.content,
                "format": openai_input_audio_format_from_mime(&audio.mime)
            }
        }));
    }
    if !latest_user_content.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": latest_user_content
        }));
    }
    normalize_prepared_prompt_messages(&mut messages);
    messages
}

fn log_text_len(text: &str) -> usize {
    text.chars().count()
}

fn push_unique_log_name(names: &mut Vec<String>, raw: &str) {
    let name = raw.trim();
    if !name.is_empty() && !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn log_tool_call_name(call: &Value) -> Option<&str> {
    call.get("function")
        .and_then(|function| function.get("name"))
        .and_then(Value::as_str)
        .or_else(|| call.get("function_name").and_then(Value::as_str))
        .or_else(|| call.get("name").and_then(Value::as_str))
}

fn log_tool_call_names_value(names: Vec<String>) -> Value {
    Value::Array(names.into_iter().map(Value::String).collect())
}

fn tool_calls_summary_from_value(value: Option<&Value>) -> (usize, Vec<String>) {
    let Some(calls) = value.and_then(Value::as_array) else {
        return (0, Vec::new());
    };
    let mut names = Vec::<String>::new();
    for call in calls {
        if let Some(name) = log_tool_call_name(call) {
            push_unique_log_name(&mut names, name);
        }
    }
    (calls.len(), names)
}

fn tool_history_summary_from_events(events: &[Value]) -> (usize, Vec<String>) {
    let mut count = 0usize;
    let mut names = Vec::<String>::new();
    for event in events {
        let Some(calls) = event.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        count = count.saturating_add(calls.len());
        for call in calls {
            if let Some(name) = log_tool_call_name(call) {
                push_unique_log_name(&mut names, name);
            }
        }
    }
    (count, names)
}

fn tool_history_summary_from_value(value: Option<&Value>) -> (usize, Vec<String>) {
    let Some(events) = value.and_then(Value::as_array) else {
        return (0, Vec::new());
    };
    tool_history_summary_from_events(events)
}

fn model_reply_to_log_value(reply: &ModelReply) -> Value {
    let mut value = serde_json::json!({
        "assistantText": reply.assistant_text,
        "activityReasoningText": reply.activity_reasoning_text,
        "toolHistoryEvents": reply.tool_history_events
    });
    if let Some(usage) = reply.usage.as_ref() {
        if let Some(map) = value.as_object_mut() {
            map.insert("usage".to_string(), usage.clone());
        }
    }
    value
}

fn compact_log_tools_value(tools: &Value) -> Option<Value> {
    let items = tools.as_array()?;
    let mut names = Vec::<String>::new();
    for item in items {
        if let Some(name) = item
            .as_str()
            .or_else(|| item.get("name").and_then(Value::as_str))
        {
            push_unique_log_name(&mut names, name);
        }
    }
    if names.is_empty() {
        None
    } else {
        Some(Value::Array(
            names
                .into_iter()
                .map(|name| serde_json::json!({ "name": name }))
                .collect(),
        ))
    }
}

fn compact_log_response_value(response: &Value) -> Value {
    let Some(source) = response.as_object() else {
        return response.clone();
    };
    let mut compact = serde_json::Map::<String, Value>::new();
    for key in [
        "conversationId",
        "assistantTextLength",
        "activityReasoningTextLength",
        "reasoningContentLength",
        "reasoningTextLength",
        "toolCallCount",
        "toolCallNames",
        "usage",
        "roundUsage",
    ] {
        if let Some(value) = source.get(key) {
            compact.insert(key.to_string(), value.clone());
        }
    }
    if !compact.contains_key("assistantTextLength") {
        if let Some(text) = source.get("assistantText").and_then(Value::as_str) {
            compact.insert(
                "assistantTextLength".to_string(),
                serde_json::json!(log_text_len(text)),
            );
        }
    }
    if !compact.contains_key("reasoningContentLength")
        && !compact.contains_key("activityReasoningTextLength")
    {
        if let Some(text) = source
            .get("reasoningContent")
            .or_else(|| source.get("activityReasoningText"))
            .and_then(Value::as_str)
        {
            compact.insert(
                "reasoningContentLength".to_string(),
                serde_json::json!(log_text_len(text)),
            );
        }
    }
    if !compact.contains_key("toolCallCount") || !compact.contains_key("toolCallNames") {
        let (direct_count, direct_names) = tool_calls_summary_from_value(source.get("toolCalls"));
        let (history_count, history_names) =
            tool_history_summary_from_value(source.get("toolHistoryEvents"));
        let count = direct_count.saturating_add(history_count);
        let mut names = direct_names;
        for name in history_names {
            push_unique_log_name(&mut names, &name);
        }
        if !compact.contains_key("toolCallCount") {
            compact.insert("toolCallCount".to_string(), serde_json::json!(count));
        }
        if !compact.contains_key("toolCallNames") {
            compact.insert("toolCallNames".to_string(), log_tool_call_names_value(names));
        }
    }
    Value::Object(compact)
}

fn compact_llm_round_log_entry_for_ui(entry: &LlmRoundLogEntry) -> LlmRoundLogEntry {
    LlmRoundLogEntry {
        id: entry.id.clone(),
        created_at: entry.created_at.clone(),
        trace_id: entry.trace_id.clone(),
        scene: entry.scene.clone(),
        request_format: entry.request_format.clone(),
        provider: entry.provider.clone(),
        model: entry.model.clone(),
        base_url: entry.base_url.clone(),
        headers: entry.headers.clone(),
        tools: entry.tools.as_ref().and_then(compact_log_tools_value),
        response: entry.response.as_ref().map(compact_log_response_value),
        error: entry.error.clone(),
        elapsed_ms: entry.elapsed_ms,
        timeline: entry.timeline.clone(),
        round_count: entry.round_count,
        tool_call_count: entry.tool_call_count,
        rounds: entry.rounds.as_ref().map(|rounds| {
            rounds
                .iter()
                .map(compact_llm_round_log_entry_for_ui)
                .collect()
        }),
        success: entry.success,
    }
}

#[allow(dead_code)]
#[derive(Default)]
struct LlmRoundUsageTotals {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
    cached_tokens: i64,
    cache_creation_tokens: i64,
    cache_creation_5m_tokens: i64,
    cache_creation_1h_tokens: i64,
    reasoning_tokens: i64,
    round_count: usize,
}

#[allow(dead_code)]
fn usage_value_i64(usage: &Value, key: &str) -> i64 {
    usage
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .unwrap_or(0)
}

#[allow(dead_code)]
fn aggregate_round_usage(rounds: &[LlmRoundLogEntry]) -> Option<Value> {
    let mut totals = LlmRoundUsageTotals::default();
    for round in rounds {
        let Some(usage) = round.response.as_ref().and_then(|response| response.get("usage")) else {
            continue;
        };
        totals.round_count += 1;
        totals.prompt_tokens += usage_value_i64(usage, "promptTokens");
        totals.completion_tokens += usage_value_i64(usage, "completionTokens");
        totals.total_tokens += usage_value_i64(usage, "totalTokens");
        totals.cached_tokens += usage_value_i64(usage, "cachedTokens");
        totals.cache_creation_tokens += usage_value_i64(usage, "cacheCreationTokens");
        totals.cache_creation_5m_tokens += usage_value_i64(usage, "cacheCreation5mTokens");
        totals.cache_creation_1h_tokens += usage_value_i64(usage, "cacheCreation1hTokens");
        totals.reasoning_tokens += usage_value_i64(usage, "reasoningTokens");
    }
    if totals.round_count == 0 {
        return None;
    }
    let total_tokens = if totals.total_tokens > 0 {
        totals.total_tokens
    } else {
        totals.prompt_tokens + totals.completion_tokens
    };
    Some(serde_json::json!({
        "roundCount": totals.round_count,
        "promptTokens": totals.prompt_tokens,
        "completionTokens": totals.completion_tokens,
        "totalTokens": total_tokens,
        "cachedTokens": totals.cached_tokens,
        "cacheCreationTokens": totals.cache_creation_tokens,
        "cacheCreation5mTokens": totals.cache_creation_5m_tokens,
        "cacheCreation1hTokens": totals.cache_creation_1h_tokens,
        "reasoningTokens": totals.reasoning_tokens,
    }))
}

fn normalize_llm_round_log_capacity(value: u32) -> usize {
    match value {
        1 => 1,
        3 => 3,
        10 => 10,
        0 => DEFAULT_LLM_ROUND_LOG_CAPACITY,
        value if value < 3 => 1,
        value if value < 10 => 3,
        _ => 10,
    }
}

fn llm_round_log_capacity_for_state(state: &AppState) -> usize {
    state_read_config_cached(state)
        .map(|config| normalize_llm_round_log_capacity(config.llm_round_log_capacity))
        .unwrap_or(DEFAULT_LLM_ROUND_LOG_CAPACITY)
}


#[tauri::command]
fn list_recent_llm_round_logs(state: State<'_, AppState>) -> Result<Vec<LlmRoundLogEntry>, String> {
    list_recent_llm_round_logs_inner(state.inner())
}

fn list_recent_llm_round_logs_inner(state: &AppState) -> Result<Vec<LlmRoundLogEntry>, String> {
    // 已全量切换至调度事件聚合，真实数据来自 schedule_events
    let capacity = llm_round_log_capacity_for_state(state);
    Ok(schedule_event_collect_llm_entries(state, capacity))
}

fn find_llm_round_log_entry_by_id<'a>(
    entry: &'a LlmRoundLogEntry,
    id: &str,
) -> Option<&'a LlmRoundLogEntry> {
    if entry.id == id {
        return Some(entry);
    }
    entry.rounds.as_ref().and_then(|rounds| {
        rounds
            .iter()
            .find_map(|round| find_llm_round_log_entry_by_id(round, id))
    })
}

fn log_response_text_field(response: Option<&Value>, keys: &[&str]) -> String {
    let Some(response) = response else {
        return String::new();
    };
    keys.iter()
        .find_map(|key| response.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn log_response_usage_section(response: Option<&Value>) -> Option<Value> {
    let response = response?;
    let mut usage = serde_json::Map::<String, Value>::new();
    if let Some(value) = response.get("usage") {
        usage.insert("usage".to_string(), value.clone());
    }
    if let Some(value) = response.get("roundUsage") {
        usage.insert("roundUsage".to_string(), value.clone());
    }
    if usage.is_empty() {
        None
    } else {
        Some(Value::Object(usage))
    }
}

fn llm_round_log_section_value(entry: &LlmRoundLogEntry, section: &str) -> Option<Value> {
    let response = entry.response.as_ref();
    match section.trim() {
        "answer" => {
            let assistant_text = log_response_text_field(response, &["assistantText"]);
            let reasoning_text =
                log_response_text_field(response, &["reasoningContent", "activityReasoningText"]);
            Some(serde_json::json!({
                "assistantText": assistant_text,
                "activityReasoningText": reasoning_text,
                "assistantTextLength": response
                    .and_then(|value| value.get("assistantTextLength"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(0)),
                "reasoningTextLength": response
                    .and_then(|value| {
                        value.get("reasoningContentLength")
                            .or_else(|| value.get("activityReasoningTextLength"))
                            .or_else(|| value.get("reasoningTextLength"))
                    })
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!(0)),
            }))
        }
        "usage" => log_response_usage_section(response),
        "raw_response" => response.cloned(),
        "tools" => {
            let mut tools = serde_json::Map::<String, Value>::new();
            let effective_tools = entry.tools.as_ref().or_else(|| {
                entry.rounds.as_ref().and_then(|rounds| {
                    rounds.iter().find_map(|r| r.tools.as_ref())
                })
            });
            if let Some(value) = effective_tools {
                if let Some(compact) = compact_log_tools_value(value) {
                    tools.insert("availableTools".to_string(), compact);
                }
            }
            if let Some(value) = response.and_then(|value| value.get("toolCallNames")) {
                tools.insert("toolCallNames".to_string(), value.clone());
            } else if let Some(response) = response {
                let (direct_count, direct_names) =
                    tool_calls_summary_from_value(response.get("toolCalls"));
                let (history_count, history_names) =
                    tool_history_summary_from_value(response.get("toolHistoryEvents"));
                let mut names = direct_names;
                for name in history_names {
                    push_unique_log_name(&mut names, &name);
                }
                let count = direct_count.saturating_add(history_count);
                tools.insert("toolCallNames".to_string(), log_tool_call_names_value(names));
                tools.insert("toolCallCount".to_string(), serde_json::json!(count));
            }
            if tools.is_empty() {
                None
            } else {
                Some(Value::Object(tools))
            }
        }
        _ => None,
    }
}

#[tauri::command]
fn get_recent_llm_round_log_section(
    state: State<'_, AppState>,
    id: String,
    section: String,
) -> Result<Option<Value>, String> {
    get_recent_llm_round_log_section_inner(state.inner(), id, section)
}

fn get_recent_llm_round_log_section_inner(
    state: &AppState,
    id: String,
    section: String,
) -> Result<Option<Value>, String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Ok(None);
    }
    if let Some(entry) = schedule_event_find_entry_by_id(state, &id) {
        if let Some(value) = llm_round_log_section_value(&entry, &section) {
            return Ok(Some(value));
        }
        // 回退到 rounds 内查找
        if let Some(rounds) = entry.rounds.as_ref() {
            for round in rounds {
                if let Some(found) = find_llm_round_log_entry_by_id(round, &id).and_then(|found| llm_round_log_section_value(found, &section)) {
                    return Ok(Some(found));
                }
            }
        }
        // 若仍未命中，直接返回 entry 的 section
        return Ok(llm_round_log_section_value(&entry, &section));
    }
    Ok(None)
}

#[tauri::command]
fn clear_recent_llm_round_logs(state: State<'_, AppState>) -> Result<bool, String> {
    clear_recent_llm_round_logs_inner(state.inner())
}

fn clear_recent_llm_round_logs_inner(state: &AppState) -> Result<bool, String> {
    // 已切换至事件系统：清空调度事件即等价于清空 LogTab
    let _ = schedule_event_clear_all(state)?;
    Ok(true)
}

#[tauri::command]
fn list_recent_runtime_logs() -> Result<Vec<RuntimeLogEntry>, String> {
    let logs = runtime_log_buffer()
        .lock()
        .map_err(|_| "Failed to lock runtime logs".to_string())?;
    Ok(logs.entries.iter().cloned().collect::<Vec<_>>())
}

/// 某个联系人的全部日志：按条目上的 contact_id 字段过滤，不靠正文里出现 id。
pub(crate) fn runtime_log_entries_for_contact(
    contact_id: &str,
) -> Vec<(String, String, String, Option<String>)> {
    let contact_id = contact_id.trim();
    if contact_id.is_empty() {
        return Vec::new();
    }
    let Ok(logs) = runtime_log_buffer().lock() else {
        return Vec::new();
    };
    logs.entries
        .iter()
        .filter(|entry| entry.contact_id.as_deref() == Some(contact_id))
        .map(|entry| {
            (
                entry.created_at.clone(),
                entry.level.clone(),
                entry.message.clone(),
                entry.role.clone(),
            )
        })
        .collect()
}

/// 唯一日志里带某个标识的条目（渠道日志窗用）。
///
/// 存储时 UUID 已被缩成前 8 位，这里对查询串做同样处理，否则完整 id 永远匹配不上。
pub(crate) fn runtime_log_entries_containing(needle: &str) -> Vec<(String, String, String)> {
    let needle = shorten_uuids_in_log(needle.trim());
    if needle.is_empty() {
        return Vec::new();
    }
    let Ok(logs) = runtime_log_buffer().lock() else {
        return Vec::new();
    };
    logs.entries
        .iter()
        .filter(|entry| entry.message.contains(needle.as_str()))
        .map(|entry| {
            (
                entry.created_at.clone(),
                entry.level.clone(),
                entry.message.clone(),
            )
        })
        .collect()
}

#[tauri::command]
fn list_runtime_logs_since(since_created_at: Option<String>) -> Result<Vec<RuntimeLogEntry>, String> {
    let logs = runtime_log_buffer()
        .lock()
        .map_err(|_| "Failed to lock runtime logs".to_string())?;
    let anchor = since_created_at
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("");
    if anchor.is_empty() {
        return Ok(logs.entries.iter().cloned().collect());
    }
    Ok(logs
        .entries
        .iter()
        .filter(|entry| entry.created_at.as_str() > anchor)
        .cloned()
        .collect())
}

#[tauri::command]
fn clear_recent_runtime_logs() -> Result<bool, String> {
    let mut logs = runtime_log_buffer()
        .lock()
        .map_err(|_| "Failed to lock runtime logs".to_string())?;
    logs.entries.clear();
    logs.total_bytes = 0;
    Ok(true)
}

#[tauri::command]
fn append_runtime_log_probe(message: Option<String>, level: Option<String>) -> Result<bool, String> {
    let msg = message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("运行日志窗口已打开");
    // 探针按调用方声明的级别落盘：排障链路需要和同链路的后端日志落在同一级别，
    // 否则用户得在日志窗口的级别过滤器之间来回切。
    match level.as_deref().map(str::trim) {
        Some("debug") => runtime_log_debug(format!("[系统] {}", msg)),
        Some("warn") => runtime_log_warn(format!("[系统] {}", msg)),
        Some("error") => runtime_log_error(format!("[系统] {}", msg)),
        _ => runtime_log_info(format!("[系统] {}", msg)),
    }
    Ok(true)
}

fn estimate_json_bytes<T: Serialize>(value: &T) -> usize {
    serde_json::to_vec(value).map(|raw| raw.len()).unwrap_or(0)
}

fn build_memory_conversation_stats<'a, I>(items: I, limit: usize) -> Vec<MemoryConversationStat>
where
    I: IntoIterator<Item = &'a Conversation>,
{
    let mut stats = items
        .into_iter()
        .map(|conversation| MemoryConversationStat {
            conversation_id: conversation.id.clone(),
            title: conversation.title.clone(),
            message_count: conversation.messages.len(),
            estimated_json_bytes: estimate_json_bytes(conversation),
        })
        .collect::<Vec<_>>();
    stats.sort_by(|a, b| {
        b.estimated_json_bytes
            .cmp(&a.estimated_json_bytes)
            .then_with(|| b.message_count.cmp(&a.message_count))
    });
    stats.truncate(limit);
    stats
}

fn build_memory_conversation_meta_stats<'a, I>(items: I, limit: usize) -> Vec<MemoryConversationStat>
where
    I: IntoIterator<Item = &'a message_store::ConversationStoredMeta>,
{
    let mut stats = items
        .into_iter()
        .map(|conversation| MemoryConversationStat {
            conversation_id: conversation.id().to_string(),
            title: conversation.title().to_string(),
            message_count: conversation.message_count(),
            estimated_json_bytes: estimate_json_bytes(conversation),
        })
        .collect::<Vec<_>>();
    stats.sort_by(|a, b| {
        b.estimated_json_bytes
            .cmp(&a.estimated_json_bytes)
            .then_with(|| b.message_count.cmp(&a.message_count))
    });
    stats.truncate(limit);
    stats
}

#[tauri::command]
fn dump_memory_cache_stats(state: State<'_, AppState>) -> Result<MemoryCacheStats, String> {
    dump_memory_cache_stats_inner(state.inner())
}

fn dump_memory_cache_stats_inner(state: &AppState) -> Result<MemoryCacheStats, String> {
    let cached_conversations_count = 0;
    let cached_conversations_message_count = 0;
    let cached_conversations_estimated_json_bytes = 0;
    let top_cached_conversations = Vec::new();

    let cached_conversation_metadata = state
        .cached_conversation_metadata
        .lock()
        .map_err(|_| "Failed to lock cached conversation metadata".to_string())?;
    let cached_conversation_metadata_count = cached_conversation_metadata.len();
    let cached_conversation_metadata_estimated_json_bytes = cached_conversation_metadata
        .values()
        .map(estimate_json_bytes)
        .sum::<usize>();
    let top_metadata_conversations =
        build_memory_conversation_meta_stats(cached_conversation_metadata.values(), 5);

    let cached_chat_index = state
        .cached_chat_index
        .lock()
        .map_err(|_| "Failed to lock cached chat index".to_string())?;
    let cached_chat_index_conversation_count = cached_chat_index
        .as_ref()
        .map(|item| item.conversations.len())
        .unwrap_or(0);
    let cached_chat_index_estimated_json_bytes = cached_chat_index
        .as_ref()
        .map(estimate_json_bytes)
        .unwrap_or(0);

    let cached_app_data = state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())?;
    let cached_app_data_loaded = cached_app_data.is_some();
    let cached_app_data_image_text_cache_entries = match state_service_count_image_text_cache(state) {
        Ok(count) => count,
        Err(err) => {
            runtime_log_warn(format!("[系统] image_text_cache 计数失败，按 0 展示 错误={err}"));
            0
        }
    };
    let cached_app_data_pdf_text_cache_entries = match state_service_count_pdf_text_cache(state) {
        Ok(count) => count,
        Err(err) => {
            runtime_log_warn(format!("[系统] pdf_text_cache 计数失败，按 0 展示 错误={err}"));
            0
        }
    };
    let cached_app_data_pdf_image_cache_entries = match state_service_count_pdf_image_cache(state) {
        Ok(count) => count,
        Err(err) => {
            runtime_log_warn(format!("[系统] pdf_image_cache 计数失败，按 0 展示 错误={err}"));
            0
        }
    };
    let cached_app_data_estimated_json_bytes = cached_app_data
        .as_ref()
        .map(estimate_json_bytes)
        .unwrap_or(0);

    let cached_conversation_dirty_ids = state
        .cached_conversation_dirty_ids
        .lock()
        .map_err(|_| "Failed to lock cached conversation dirty ids".to_string())?
        .len();
    let cached_deleted_conversation_ids = state
        .cached_deleted_conversation_ids
        .lock()
        .map_err(|_| "Failed to lock cached deleted conversation ids".to_string())?
        .len();
    let inflight_chat_abort_handles = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?
        .len();

    let inflight_completed_tool_history = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    let inflight_completed_tool_sessions = inflight_completed_tool_history.len();
    let inflight_completed_tool_event_count = inflight_completed_tool_history
        .values()
        .map(Vec::len)
        .sum::<usize>();

    let terminal_session_roots = state
        .terminal_session_roots
        .lock()
        .map_err(|_| "Failed to lock terminal session roots".to_string())?
        .len();
    let terminal_pending_approvals = state
        .terminal_pending_approvals
        .lock()
        .map_err(|_| "Failed to lock terminal pending approvals".to_string())?
        .len();
    let terminal_live_sessions = state.terminal_live_sessions.blocking_lock().len();

    let schedule_events = state
        .schedule_events
        .lock()
        .map_err(|_| "Failed to lock schedule events".to_string())?;
    let schedule_event_runs = schedule_events
        .runs_by_conversation
        .values()
        .map(|deque: &std::collections::VecDeque<ScheduleRun>| deque.len())
        .sum::<usize>();
    let schedule_event_events = schedule_events
        .runs_by_conversation
        .values()
        .flat_map(|deque: &std::collections::VecDeque<ScheduleRun>| deque.iter())
        .map(|run| run.events.len())
        .sum::<usize>();
    let schedule_event_estimated_json_bytes =
        schedule_event_estimated_json_bytes(&schedule_events);

    let conversation_runtime_slots = state
        .conversation_runtime_slots
        .lock()
        .map_err(|_| "Failed to lock conversation runtime slots".to_string())?;
    let conversation_runtime_slots_count = conversation_runtime_slots.len();
    let conversation_runtime_stream_block_count = conversation_runtime_slots
        .values()
        .map(|item| item.stream_cache.stream_blocks.len())
        .sum::<usize>();

    let pending_chat_result_senders = state
        .pending_chat_result_senders
        .lock()
        .map_err(|_| "Failed to lock pending chat result senders".to_string())?
        .len();
    let pending_chat_delta_channels = state
        .pending_chat_delta_channels
        .lock()
        .map_err(|_| "Failed to lock pending chat delta channels".to_string())?
        .len();
    let active_chat_view_bindings = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?
        .len();
    let conversation_list_activity_marks = state
        .conversation_list_activity_marks
        .lock()
        .map_err(|_| "Failed to lock conversation list activity marks".to_string())?
        .len();

    let delegate_runtime_threads = state
        .delegate_runtime_threads
        .lock()
        .map_err(|_| "Failed to lock delegate runtime threads".to_string())?;
    let delegate_runtime_threads_count = delegate_runtime_threads.len();
    let delegate_runtime_thread_message_count = delegate_runtime_threads
        .values()
        .map(|item| item.conversation.messages.len())
        .sum::<usize>();
    let delegate_runtime_threads_estimated_json_bytes =
        delegate_runtime_threads.values().map(|item| estimate_json_bytes(&item.conversation)).sum();
    let top_delegate_runtime_threads = build_memory_conversation_stats(
        delegate_runtime_threads.values().map(|item| &item.conversation),
        5,
    );

    let delegate_recent_threads = state
        .delegate_recent_threads
        .lock()
        .map_err(|_| "Failed to lock delegate recent threads".to_string())?;
    let delegate_recent_threads_count = delegate_recent_threads.len();
    let delegate_recent_thread_message_count = delegate_recent_threads
        .iter()
        .map(|item| item.conversation.messages.len())
        .sum::<usize>();
    let delegate_recent_threads_estimated_json_bytes = delegate_recent_threads
        .iter()
        .map(|item| estimate_json_bytes(&item.conversation))
        .sum::<usize>();

    let remote_im_contact_runtime_states = state
        .remote_im_contact_runtime_states
        .lock()
        .map_err(|_| "Failed to lock remote im contact runtime states".to_string())?
        .len();
    let provider_streaming_disabled_keys = state
        .provider_streaming_disabled_keys
        .lock()
        .map_err(|_| "Failed to lock provider streaming disabled keys".to_string())?
        .len();
    let provider_system_message_user_fallback_keys = state
        .provider_system_message_user_fallback_keys
        .lock()
        .map_err(|_| "Failed to lock provider system message fallback keys".to_string())?
        .len();
    let provider_request_gates = state.provider_request_gates.blocking_lock().len();

    let (
        message_store_block_cache_entries,
        message_store_block_cache_message_count,
        message_store_block_cache_estimated_json_bytes,
    ) = message_store::message_store_block_file_cache_stats();

    let (
        message_store_index_cache_entries,
        message_store_index_cache_item_count,
        message_store_index_cache_estimated_json_bytes,
    ) = message_store::message_store_index_cache_stats();

    let prompt_final_cache_entries = system_prompt_text_cache()
        .lock()
        .map_err(|_| "Failed to lock final prompt cache".to_string())?
        .len();
    let prompt_agent_cache_entries = agent_system_prompt_cache()
        .lock()
        .map_err(|_| "Failed to lock agent prompt cache".to_string())?
        .len();
    let prompt_environment_cache_entries = conversation_environment_prompt_cache()
        .lock()
        .map_err(|_| "Failed to lock environment prompt cache".to_string())?
        .len();

    let abstract_message_projection_cache = abstract_message_projection_cache()
        .lock()
        .map_err(|_| "Failed to lock abstract message projection cache".to_string())?;
    let abstract_message_projection_cache_entries = abstract_message_projection_cache.len();
    let abstract_message_projection_message_count = abstract_message_projection_cache
        .values()
        .map(|item| item.messages.len())
        .sum::<usize>();

    let screenshot_artifact_cache = screenshot_artifact_cache()
        .lock()
        .map_err(|_| "Failed to lock screenshot artifact cache".to_string())?;
    let screenshot_artifact_cache_entries = screenshot_artifact_cache.len();
    let screenshot_artifact_image_count = screenshot_artifact_cache
        .values()
        .map(|item| item.images.len())
        .sum::<usize>();

    let tool_schema_cache_count = tool_schema_cache_store()
        .lock()
        .map_err(|_| "Failed to lock tool schema cache".to_string())?
        .as_ref()
        .map(Vec::len)
        .unwrap_or(0);

    let mcp_cached_clients = mcp_client_cache().blocking_lock().len();
    let mcp_runtime_state_store = mcp_runtime_state_store()
        .lock()
        .map_err(|_| "Failed to lock mcp runtime state store".to_string())?;
    let mcp_runtime_states = mcp_runtime_state_store.len();
    let mcp_runtime_tool_count = mcp_runtime_state_store
        .values()
        .map(|item| item.tools.len())
        .sum::<usize>();

    let ide_context_chat_clients = IDE_CONTEXT_CHAT_CLIENTS
        .get()
        .and_then(|clients| clients.lock().ok().map(|guard| guard.len()))
        .unwrap_or(0);

    let mut notes = Vec::<String>::new();
    if cached_conversations_estimated_json_bytes > 0 {
        notes.push("cached_conversations 仍存在非零数据，这违反当前运行时边界。".to_string());
    }
    if delegate_runtime_threads_count > 0 || delegate_recent_threads_count > 0 {
        notes.push("delegate_runtime_threads / delegate_recent_threads 也各自带整份 Conversation。".to_string());
    }

    Ok(MemoryCacheStats {
        generated_at: now_utc_rfc3339(),
        pid: std::process::id(),
        cached_conversations: cached_conversations_count,
        cached_conversations_message_count,
        cached_conversations_estimated_json_bytes,
        cached_conversation_metadata: cached_conversation_metadata_count,
        cached_conversation_metadata_estimated_json_bytes,
        cached_chat_index_conversation_count,
        cached_chat_index_estimated_json_bytes,
        cached_app_data_loaded,
        cached_app_data_image_text_cache_entries,
        cached_app_data_pdf_text_cache_entries,
        cached_app_data_pdf_image_cache_entries,
        cached_app_data_estimated_json_bytes,
        cached_conversation_dirty_ids,
        cached_deleted_conversation_ids,
        inflight_chat_abort_handles,
        inflight_completed_tool_sessions,
        inflight_completed_tool_event_count,
        terminal_live_sessions,
        terminal_session_roots,
        terminal_pending_approvals,
        schedule_event_runs,
        schedule_event_events,
        schedule_event_estimated_json_bytes,
        conversation_runtime_slots: conversation_runtime_slots_count,
        conversation_runtime_stream_block_count,
        pending_chat_result_senders,
        pending_chat_delta_channels,
        active_chat_view_bindings,
        conversation_list_activity_marks,
        delegate_runtime_threads: delegate_runtime_threads_count,
        delegate_runtime_thread_message_count,
        delegate_runtime_threads_estimated_json_bytes,
        delegate_recent_threads: delegate_recent_threads_count,
        delegate_recent_thread_message_count,
        delegate_recent_threads_estimated_json_bytes,
        remote_im_contact_runtime_states,
        provider_streaming_disabled_keys,
        provider_system_message_user_fallback_keys,
        provider_request_gates,
        message_store_block_cache_entries,
        message_store_block_cache_message_count,
        message_store_block_cache_estimated_json_bytes,
        message_store_index_cache_entries,
        message_store_index_cache_item_count,
        message_store_index_cache_estimated_json_bytes,
        prompt_final_cache_entries,
        prompt_agent_cache_entries,
        prompt_environment_cache_entries,
        abstract_message_projection_cache_entries,
        abstract_message_projection_message_count,
        screenshot_artifact_cache_entries,
        screenshot_artifact_image_count,
        tool_schema_cache_count,
        mcp_cached_clients,
        mcp_runtime_states,
        mcp_runtime_tool_count,
        ide_context_chat_clients,
        top_cached_conversations,
        top_metadata_conversations,
        top_delegate_runtime_threads,
        notes,
    })
}

#[cfg(test)]
mod debug_log_tests {
    use super::*;

    #[test]
    fn shorten_uuids_in_log_keeps_only_first_8_chars() {
        let message = "conversation_id=8fdd1d0e-4423-4fd5-8a23-61f22364d606，message_id=01234567-89ab-cdef-0123-456789abcdef";
        let shortened = shorten_uuids_in_log(message);
        assert_eq!(shortened, "conversation_id=8fdd1d0e，message_id=01234567");
    }

    #[test]
    fn shorten_uuids_in_log_keeps_non_uuid_ids_untouched() {
        let message = "task_id=task-1776001910939，agent_id=persona-1781792413924，api=api-provider-1784421152299::api-model-1786365798704";
        let shortened = shorten_uuids_in_log(message);
        assert_eq!(shortened, message);
    }

    #[test]
    fn shorten_uuids_in_log_keeps_plain_text_and_cjk() {
        let message = "开始，任务=新建未归档会话，耗时=24ms";
        assert_eq!(shorten_uuids_in_log(message), message);
    }

    #[test]
    fn shorten_uuids_in_log_keeps_uuid_without_separator_context() {
        // 连字符前是字母数字（如长 id 尾部）不应误截
        let message = "abc8fdd1d0e-4423-4fd5-8a23-61f22364d606";
        let shortened = shorten_uuids_in_log(message);
        assert_eq!(shortened, message);
    }

    #[test]
    fn test_llm_round_log_section_tools_available() {
        let entry = LlmRoundLogEntry {
            id: "test-run".to_string(),
            created_at: "2026-09-25T00:00:00Z".to_string(),
            trace_id: None,
            scene: "chat".to_string(),
            request_format: "openai".to_string(),
            provider: "test".to_string(),
            model: "test-model".to_string(),
            base_url: "http://localhost".to_string(),
            headers: Vec::new(),
            tools: Some(serde_json::json!([
                {"name": "tavily_search"},
                {"name": "websearch"}
            ])),
            response: Some(serde_json::json!({
                "toolCallNames": ["websearch"]
            })),
            error: None,
            elapsed_ms: 100,
            timeline: None,
            round_count: Some(1),
            tool_call_count: Some(1),
            rounds: None,
            success: true,
        };
        let section = llm_round_log_section_value(&entry, "tools").expect("tools section should exist");
        let tools_map = section.as_object().expect("should be map");
        let available = tools_map.get("availableTools").expect("availableTools must exist").as_array().expect("array");
        assert_eq!(available.len(), 2);
        let called = tools_map.get("toolCallNames").expect("toolCallNames must exist").as_array().expect("array");
        assert_eq!(called.len(), 1);
    }
}

#[cfg(test)]
mod runtime_log_pipeline_tests {
    use super::*;

    /// 级别归一化必须保留：前缀决定级别，正文去掉前缀。
    #[test]
    fn normalize_runtime_log_should_lift_prefixed_level() {
        assert_eq!(
            normalize_runtime_log("info", "[ERROR] 出错了".to_string()),
            ("error".to_string(), "出错了".to_string())
        );
        assert_eq!(
            normalize_runtime_log("debug", "[WARN] 慢".to_string()),
            ("warn".to_string(), "慢".to_string())
        );
        // 无前缀时保留传入级别与正文（这里是「换前换后级别与正文一致」的自查基线）。
        assert_eq!(
            normalize_runtime_log("info", "普通消息".to_string()),
            ("info".to_string(), "普通消息".to_string())
        );
    }

    /// 前端探针必须仍能进内存缓冲（logger 未就绪时也不许丢），四个读取命令照旧可用。
    #[test]
    fn append_runtime_log_probe_should_land_in_buffer_with_declared_level() {
        if let Ok(mut logs) = runtime_log_buffer().lock() {
            logs.entries.clear();
            logs.total_bytes = 0;
        }
        append_runtime_log_probe(
            Some("日志窗口探针".to_string()),
            Some("warn".to_string()),
        )
        .expect("probe");
        let logs = list_recent_runtime_logs().expect("list recent runtime logs");
        assert!(logs
            .iter()
            .any(|entry| entry.level == "warn" && entry.message.contains("日志窗口探针")));
    }

    /// 废弃命名（真实双点 / 单点 / date-only）一律删除；活动文件与桌面归档不碰。
    #[test]
    fn cleanup_stale_cli_log_files_should_drop_legacy_names_and_keep_active_and_archives() {
        let dir = std::env::temp_dir().join(format!("pai-log-cleanup-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");

        let legacy = [
            format!("{RUNTIME_LOG_FILE_NAME}..2026-11-19.log"),
            format!("{RUNTIME_LOG_FILE_NAME}.2026-11-19.log"),
            format!("{RUNTIME_LOG_FILE_NAME}_2026-11-19.log"),
        ];
        let keep = [
            format!("{RUNTIME_LOG_FILE_NAME}.log"),
            format!("{RUNTIME_LOG_FILE_NAME}_2026-11-20_14-07-33.log"),
            "other.log".to_string(),
        ];
        for name in legacy.iter().chain(keep.iter()) {
            fs::write(dir.join(name), "x").expect("write fixture");
        }

        cleanup_stale_cli_log_files(&dir);
        for name in &legacy {
            assert!(!dir.join(name).exists(), "废弃命名 {name} 应删除");
        }
        for name in &keep {
            assert!(dir.join(name).exists(), "{name} 不应被删");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// CLI 清理：超过 KEEP 个时删最旧、留最新 KEEP 个；桌面归档不删也不占名额。
    #[test]
    fn cleanup_stale_cli_log_files_should_keep_newest_k_and_drop_older() {
        let now = time::OffsetDateTime::now_utc();
        let dir = std::env::temp_dir().join(format!("pai-log-cleanup-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");

        // 按日期从新到旧造 KEEP+5 个，前 KEEP 个应保留、其余应删除。
        let mut paths = Vec::new();
        for i in 0..(RUNTIME_LOG_FILE_KEEP + 5) as i64 {
            let date = (now.date() - time::Duration::days(i)).to_string();
            let path = dir.join(format!("{RUNTIME_LOG_FILE_NAME}-cli_{date}.log"));
            fs::write(&path, "x").expect("write fixture");
            paths.push(path);
        }
        let archive = dir.join(format!("{RUNTIME_LOG_FILE_NAME}_2026-11-20_14-07-33.log"));
        fs::write(&archive, "x").expect("write archive fixture");

        cleanup_stale_cli_log_files(&dir);
        for (i, path) in paths.iter().enumerate() {
            assert_eq!(
                path.exists(),
                i < RUNTIME_LOG_FILE_KEEP,
                "第 {i} 新的文件保留状态不符"
            );
        }
        assert!(archive.exists(), "桌面归档不应被删，也不应占用保留名额");
        let _ = fs::remove_dir_all(&dir);
    }

    /// 不足 KEEP 个时一个都不删。
    #[test]
    fn cleanup_stale_cli_log_files_should_keep_all_when_under_limit() {
        let now = time::OffsetDateTime::now_utc();
        let dir = std::env::temp_dir().join(format!("pai-log-cleanup-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");

        let mut paths = Vec::new();
        for i in 0..3i64 {
            let date = (now.date() - time::Duration::days(i)).to_string();
            let path = dir.join(format!("{RUNTIME_LOG_FILE_NAME}-cli_{date}.log"));
            fs::write(&path, "x").expect("write fixture");
            paths.push(path);
        }

        cleanup_stale_cli_log_files(&dir);
        for path in &paths {
            assert!(path.exists(), "不足 KEEP 个时不应删除任何文件");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// v0.98.4 及更早的自研归档（含 .idx 分片）必须删掉。
    #[test]
    fn cleanup_stale_cli_log_files_should_drop_v0984_archives() {
        let dir = std::env::temp_dir().join(format!("pai-log-cleanup-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let archives = [
            format!("{RUNTIME_LOG_FILE_NAME}.1763000000.12345.log"),
            format!("{RUNTIME_LOG_FILE_NAME}.1763000000.12345.idx.log"),
        ];
        for name in &archives {
            fs::write(dir.join(name), "x").expect("write fixture");
        }

        cleanup_stale_cli_log_files(&dir);
        for name in &archives {
            assert!(!dir.join(name).exists(), "v0.98.4 归档 {name} 应删除");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// 日期校验：非法月日不能通过，否则占保留名额或被误删。
    #[test]
    fn is_iso_date_should_reject_invalid_month_and_day() {
        assert!(is_iso_date("2026-11-20"));
        assert!(!is_iso_date("2026-aa-bb"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date("2026-00-01"));
        assert!(!is_iso_date("2026-11-00"));
        assert!(!is_iso_date("2026-11-32"));
    }

    /// 计数与长度一致、满时丢最旧并留痕：用局部内核测，避开全局 pending 的并发干扰。
    #[test]
    fn pending_push_apply_should_track_count_and_drop_oldest() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let mut queue = std::collections::VecDeque::new();
        let count = AtomicUsize::new(0);
        let dropped = AtomicUsize::new(0);

        for i in 0..RUNTIME_LOG_PENDING_MAX {
            pending_push_apply(
                &mut queue,
                &count,
                &dropped,
                log::Level::Info,
                format!("msg-{i}"),
            );
        }
        assert_eq!(queue.len(), RUNTIME_LOG_PENDING_MAX);
        assert_eq!(count.load(Ordering::Relaxed), RUNTIME_LOG_PENDING_MAX);
        assert_eq!(dropped.load(Ordering::Relaxed), 0);

        pending_push_apply(
            &mut queue,
            &count,
            &dropped,
            log::Level::Info,
            "newest".to_string(),
        );
        assert_eq!(queue.len(), RUNTIME_LOG_PENDING_MAX);
        assert_eq!(count.load(Ordering::Relaxed), RUNTIME_LOG_PENDING_MAX);
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        assert_eq!(queue.front().expect("oldest").1, "msg-1");
        assert_eq!(queue.back().expect("newest").1, "newest");

        let drained = pending_drain_apply(&mut queue, &count);
        assert_eq!(drained.len(), RUNTIME_LOG_PENDING_MAX);
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }
}
