const RUNTIME_LOG_MAX_BYTES: usize = 10 * 1024 * 1024;
const BACKEND_LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const BACKEND_LOG_MAX_FILES: usize = 20;
const BACKEND_LOG_ARCHIVE_MAX_BYTES: u64 = 25 * 1024 * 1024;
const BACKEND_LOG_FILE_NAME: &str = "backend.log";
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
    inflight_tool_abort_handles: usize,
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
    prompt_department_cache_entries: usize,
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

fn backend_log_write_lock() -> &'static Mutex<()> {
    static BACKEND_LOG_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    BACKEND_LOG_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

fn backend_log_path() -> &'static Option<PathBuf> {
    static BACKEND_LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    BACKEND_LOG_PATH.get_or_init(resolve_backend_log_path)
}

fn resolve_backend_log_path() -> Option<PathBuf> {
    let log_dir = detect_portable_runtime_root()
        .or_else(|| {
            ProjectDirs::from("ai", "easycall", "p-ai")
                .map(|dirs| dirs.config_dir().to_path_buf())
        })
        .or_else(|| current_exe_dir())
        .unwrap_or_else(std::env::temp_dir)
        .join("logs");
    if fs::create_dir_all(&log_dir).is_err() {
        return None;
    }
    Some(log_dir.join(BACKEND_LOG_FILE_NAME))
}

fn backend_log_archive_path(path: &PathBuf) -> PathBuf {
    let ts = now_utc().unix_timestamp();
    let pid = std::process::id();
    for index in 0..100_u32 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!(".{index}")
        };
        let candidate = path.with_file_name(format!("backend.{ts}.{pid}{suffix}.log"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!("backend.{ts}.{pid}.fallback.log"))
}

fn is_backend_log_archive(path: &PathBuf) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.starts_with("backend.") && name.ends_with(".log") && name != BACKEND_LOG_FILE_NAME
}

fn prune_backend_log_archives(path: &PathBuf) {
    let Some(dir) = path.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut archives = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(is_backend_log_archive)
        .filter_map(|archive_path| {
            let metadata = fs::metadata(&archive_path).ok()?;
            let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((archive_path, modified, metadata.len()))
        })
        .collect::<Vec<_>>();
    archives.sort_by(|a, b| b.1.cmp(&a.1));

    let mut kept_files = 0_usize;
    let mut kept_bytes = 0_u64;
    for (archive_path, _, bytes) in archives {
        let next_files = kept_files.saturating_add(1);
        let next_bytes = kept_bytes.saturating_add(bytes);
        if next_files > BACKEND_LOG_MAX_FILES || next_bytes > BACKEND_LOG_ARCHIVE_MAX_BYTES {
            let _ = fs::remove_file(archive_path);
        } else {
            kept_files = next_files;
            kept_bytes = next_bytes;
        }
    }
}

fn archive_backend_log(path: &PathBuf) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.len() == 0 {
        return;
    }
    let target = backend_log_archive_path(path);
    if fs::rename(path, &target).is_err() {
        if fs::copy(path, &target).is_ok() {
            let _ = fs::remove_file(path);
        }
    }
    prune_backend_log_archives(path);
}

fn rotate_backend_log_if_needed(path: &PathBuf, pending_bytes: u64) {
    let current_bytes = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if current_bytes.saturating_add(pending_bytes) <= BACKEND_LOG_MAX_BYTES {
        return;
    }
    archive_backend_log(path);
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

fn append_backend_log_line(level: &str, message: &str) {
    let Some(path) = backend_log_path().as_ref() else {
        return;
    };
    let Ok(_guard) = backend_log_write_lock().lock() else {
        return;
    };
    let line = format!(
        "{} {:<5} {}\n",
        now_log_local_rfc3339(),
        level.to_uppercase(),
        message
    );
    rotate_backend_log_if_needed(path, line.len() as u64);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = std::io::Write::write_all(&mut file, line.as_bytes());
    }
}

fn init_backend_file_logging() {
    let Some(path) = backend_log_path().as_ref() else {
        return;
    };
    if let Ok(_guard) = backend_log_write_lock().lock() {
        archive_backend_log(path);
    }
    append_backend_log_line("info", "========== 本次启动开始 ==========");
}

fn install_backend_file_panic_hook() {
    static BACKEND_FILE_PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();
    if BACKEND_FILE_PANIC_HOOK_INSTALLED.set(()).is_err() {
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
        append_backend_log_line(
            "error",
            &format!(
                "[panic] location={} thread={} payload={}",
                location.trim(),
                thread_name.trim(),
                payload.trim()
            ),
        );
        previous_hook(info);
    }));
}

fn is_ascii_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

/// 把日志消息里的标准 UUID（8-4-4-4-12）截断为前 8 位短 id，降低日志噪音。
/// 非 UUID 文本原样保留；只匹配完整 36 字符 UUID 形态，避免误伤
/// `department-xxx`、`persona-xxx`、`api-provider-xxx::api-model-xxx` 等带前缀 id。
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

fn runtime_log_push(level: &str, message: String) {
    let message = shorten_uuids_in_log(&message);
    if !headless_cli_console_logging_suppressed() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("{message}\n").as_bytes());
    }
    let (normalized_level, normalized_message) = normalize_runtime_log(level, message);
    append_backend_log_line(&normalized_level, &normalized_message);
    let created_at = now_log_local_rfc3339();
    let Ok(mut buf) = runtime_log_buffer().lock() else {
        return;
    };
    if let Some(last) = buf.entries.back_mut() {
        if last.level == normalized_level && last.message == normalized_message {
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
            let mut content = Vec::<Value>::new();
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": time_text,
                    }));
                }
            }
            if !hm.text.trim().is_empty() {
                content.push(serde_json::json!({
                    "type": "text",
                    "text": hm.text,
                }));
            }
            for block in &hm.extra_text_blocks {
                if block.trim().is_empty() {
                    continue;
                }
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

fn latest_chat_round_headers_and_tools(
    _state: &AppState,
    _chat_session_key: Option<&str>,
    _request_format: RequestFormat,
    _provider_name: &str,
    _model_name: &str,
    _base_url: &str,
) -> (Vec<LlmRoundLogHeader>, Option<Value>) {
    // 已切换至调度事件：旧 pipeline 缓冲不再使用，直接返回空由调用方回退至 masked_auth
    (Vec::new(), None)
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
            if let Some(value) = entry.tools.as_ref() {
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
fn append_runtime_log_probe(message: Option<String>) -> Result<bool, String> {
    let msg = message
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("运行日志窗口已打开");
    runtime_log_info(format!("[运行日志] {}", msg));
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
    I: IntoIterator<Item = &'a message_store::ConversationShardMeta>,
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
            runtime_log_warn(format!("[内存统计] image_text_cache 计数失败，按 0 展示：error={err}"));
            0
        }
    };
    let cached_app_data_pdf_text_cache_entries = match state_service_count_pdf_text_cache(state) {
        Ok(count) => count,
        Err(err) => {
            runtime_log_warn(format!("[内存统计] pdf_text_cache 计数失败，按 0 展示：error={err}"));
            0
        }
    };
    let cached_app_data_pdf_image_cache_entries = match state_service_count_pdf_image_cache(state) {
        Ok(count) => count,
        Err(err) => {
            runtime_log_warn(format!("[内存统计] pdf_image_cache 计数失败，按 0 展示：error={err}"));
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
    let inflight_tool_abort_handles = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?
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
    let prompt_department_cache_entries = department_system_prompt_cache()
        .lock()
        .map_err(|_| "Failed to lock department prompt cache".to_string())?
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
        inflight_tool_abort_handles,
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
        prompt_department_cache_entries,
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
        let message = "department_id=department-1776001910939，agent_id=persona-1781792413924，api=api-provider-1784421152299::api-model-1786365798704";
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
}
