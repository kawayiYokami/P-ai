const LLM_ROUND_LOG_CAPACITY: usize = 10;
const RUNTIME_LOG_MAX_BYTES: usize = 10 * 1024 * 1024;

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
    request: Value,
    response: Option<Value>,
    error: Option<String>,
    elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<Vec<LlmRoundLogStage>>,
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

fn runtime_log_buffer() -> &'static Mutex<RuntimeLogBuffer> {
    static RUNTIME_LOGS: OnceLock<Mutex<RuntimeLogBuffer>> = OnceLock::new();
    RUNTIME_LOGS.get_or_init(|| Mutex::new(RuntimeLogBuffer::default()))
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
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("{message}\n").as_bytes());
    let (normalized_level, normalized_message) = normalize_runtime_log(level, message);
    let Ok(mut buf) = runtime_log_buffer().lock() else {
        return;
    };
    if let Some(last) = buf.entries.back_mut() {
        if last.level == normalized_level && last.message == normalized_message {
            last.repeat = last.repeat.saturating_add(1);
            last.created_at = now_iso();
            return;
        }
    }
    let entry = RuntimeLogEntry {
        id: Uuid::new_v4().to_string(),
        created_at: now_iso(),
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

fn prepared_prompt_to_equivalent_request_json(
    prepared: &PreparedPrompt,
    model_name: &str,
    temperature: f64,
) -> Value {
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

    fn normalize_messages(messages: &mut [Value]) {
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

    let mut messages = Vec::<Value>::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": prepared.preamble
    }));

    for hm in &prepared.history_messages {
        if hm.role == "assistant" && hm.tool_calls.is_some() {
            let mut msg = serde_json::Map::new();
            msg.insert("role".to_string(), Value::String("assistant".to_string()));
            if hm.text.trim().is_empty() {
                msg.insert("content".to_string(), Value::Null);
            } else {
                msg.insert("content".to_string(), Value::String(hm.text.clone()));
            }
            if let Some(reasoning) = &hm.reasoning_content {
                if !reasoning.trim().is_empty() {
                    msg.insert("reasoning_content".to_string(), Value::String(reasoning.clone()));
                }
            }
            if let Some(calls) = &hm.tool_calls {
                msg.insert("tool_calls".to_string(), Value::Array(calls.clone()));
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
            let mut content = vec![serde_json::json!({
                "type": "text",
                "text": hm.text,
            })];
            if let Some(time_text) = &hm.user_time_text {
                if !time_text.trim().is_empty() {
                    content.push(serde_json::json!({
                        "type": "text",
                        "text": time_text,
                    }));
                }
            }
            for (mime, bytes_base64) in &hm.images {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    content.push(serde_json::json!({
                        "type": "file",
                        "mime": mime,
                        "bytesBase64": bytes_base64
                    }));
                } else {
                    content.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", mime, bytes_base64),
                            "detail": "auto"
                        }
                    }));
                }
            }
            for (mime, bytes_base64) in &hm.audios {
                content.push(serde_json::json!({
                    "type": "input_audio",
                    "input_audio": {
                        "data": bytes_base64,
                        "format": openai_input_audio_format_from_mime(mime)
                    }
                }));
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
    for text_block in prepared_prompt_latest_user_text_blocks(prepared) {
        latest_user_content.push(serde_json::json!({
            "type": "text",
            "text": text_block
        }));
    }
    for (mime, bytes_base64) in &prepared.latest_images {
        if mime.trim().eq_ignore_ascii_case("application/pdf") {
            latest_user_content.push(serde_json::json!({
                "type": "file",
                "mime": mime,
                "bytesBase64": bytes_base64
            }));
        } else {
            latest_user_content.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", mime, bytes_base64),
                    "detail": "auto"
                }
            }));
        }
    }
    for (mime, bytes_base64) in &prepared.latest_audios {
        latest_user_content.push(serde_json::json!({
            "type": "input_audio",
            "input_audio": {
                "data": bytes_base64,
                "format": openai_input_audio_format_from_mime(mime)
            }
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": latest_user_content
    }));
    normalize_messages(&mut messages);

    serde_json::json!({
        "model": model_name,
        "temperature": temperature,
        "stream": true,
        "messages": messages
    })
}

fn model_reply_to_log_value(reply: &ModelReply) -> Value {
    serde_json::json!({
        "assistantText": reply.assistant_text,
        "reasoningStandard": reply.reasoning_standard,
        "reasoningInline": reply.reasoning_inline,
        "toolHistoryEvents": reply.tool_history_events
    })
}

fn push_llm_round_log(
    state: Option<&AppState>,
    trace_id: Option<String>,
    scene: &str,
    request_format: RequestFormat,
    provider_name: &str,
    model_name: &str,
    base_url: &str,
    headers: Vec<LlmRoundLogHeader>,
    tools: Option<Value>,
    request: Value,
    response: Option<Value>,
    error: Option<String>,
    elapsed_ms: u64,
    timeline: Option<Vec<LlmRoundLogStage>>,
) {
    let Some(app_state) = state else {
        return;
    };
    let success = error.is_none();
    let Ok(mut logs) = app_state.llm_round_logs.lock() else {
        return;
    };
    logs.push_back(LlmRoundLogEntry {
        id: Uuid::new_v4().to_string(),
        created_at: now_iso(),
        trace_id,
        scene: scene.to_string(),
        request_format: request_format.as_str().to_string(),
        provider: provider_name.to_string(),
        model: model_name.to_string(),
        base_url: base_url.to_string(),
        headers,
        tools,
        request,
        response,
        error: error.filter(|v| !v.trim().is_empty()),
        elapsed_ms,
        timeline,
        success,
    });
    while logs.len() > LLM_ROUND_LOG_CAPACITY {
        let _ = logs.pop_front();
    }
}

fn latest_chat_round_headers_and_tools(
    state: &AppState,
    request_format: RequestFormat,
    provider_name: &str,
    model_name: &str,
    base_url: &str,
) -> (Vec<LlmRoundLogHeader>, Option<Value>) {
    let Ok(logs) = state.llm_round_logs.lock() else {
        return (Vec::new(), None);
    };
    let Some(entry) = logs.iter().rev().find(|entry| {
        entry.scene == "chat"
            && entry.request_format == request_format.as_str()
            && entry.provider == provider_name
            && entry.model == model_name
            && entry.base_url == base_url
    }) else {
        return (Vec::new(), None);
    };
    (entry.headers.clone(), entry.tools.clone())
}

#[tauri::command]
fn list_recent_llm_round_logs(state: State<'_, AppState>) -> Result<Vec<LlmRoundLogEntry>, String> {
    let logs = state
        .llm_round_logs
        .lock()
        .map_err(|_| "Failed to lock llm round logs".to_string())?;
    Ok(logs.iter().cloned().collect::<Vec<_>>())
}

#[tauri::command]
fn clear_recent_llm_round_logs(state: State<'_, AppState>) -> Result<bool, String> {
    let mut logs = state
        .llm_round_logs
        .lock()
        .map_err(|_| "Failed to lock llm round logs".to_string())?;
    logs.clear();
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
