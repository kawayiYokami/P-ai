#[derive(Debug, Clone, Deserialize, Serialize)]
struct FetchToolArgs {
    url: String,
    #[serde(default)]
    max_length: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BingSearchToolArgs {
    query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MemorySaveToolArgs {
    memory_type: String,
    judgment: String,
    reasoning: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RecallToolArgs {
    query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct EmptyToolArgs {}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WaitToolArgs {
    ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TerminalExecToolArgs {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ContactReplyToolArgs {
    text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ContactSendFilesToolArgs {
    file_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ContactNoReplyToolArgs {
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegateToolArgs {
    department_id: String,
    #[serde(default)]
    mode: Option<String>,
    background: String,
    question: String,
    focus: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegateMode {
    Async,
    Sync,
}

fn parse_delegate_mode(raw: Option<&str>) -> Result<DelegateMode, String> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(DelegateMode::Async),
        Some("async") => Ok(DelegateMode::Async),
        Some("sync") => Ok(DelegateMode::Sync),
        Some(other) => Err(format!(
            "delegate.mode 必须是 `async` 或 `sync`，当前收到：{other}"
        )),
    }
}

fn debug_text_snippet(text: &str, max_chars: usize) -> String {
    let compact = text.trim().replace('\r', "").replace('\n', "\\n");
    if compact.chars().count() <= max_chars {
        compact
    } else {
        let head = compact.chars().take(max_chars).collect::<String>();
        format!("{head}...")
    }
}

fn debug_exec_result_summary(value: &Value) -> String {
    let Some(obj) = value.as_object() else {
        return debug_value_snippet(value, 320);
    };
    let ok = obj.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let approved = obj.get("approved").and_then(Value::as_bool);
    let timed_out = obj.get("timedOut").and_then(Value::as_bool).unwrap_or(false);
    let exit_code = obj.get("exitCode").and_then(Value::as_i64).unwrap_or(-1);
    let duration_ms = obj.get("durationMs").and_then(Value::as_u64).unwrap_or(0);
    let blocked_reason = obj
        .get("blockedReason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let command = obj.get("command").and_then(Value::as_str).unwrap_or_default();
    let stdout = obj.get("stdout").and_then(Value::as_str).unwrap_or_default();
    let stderr = obj.get("stderr").and_then(Value::as_str).unwrap_or_default();
    format!(
        "ok={}, approved={}, timedOut={}, exitCode={}, durationMs={}, blockedReason={}, command={}, stdout={}, stderr={}",
        ok,
        approved
            .map(|v| v.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        timed_out,
        exit_code,
        duration_ms,
        if blocked_reason.is_empty() { "none" } else { blocked_reason },
        debug_text_snippet(command, 160),
        debug_text_snippet(stdout, 220),
        debug_text_snippet(stderr, 220),
    )
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TaskToolArgsWire {
    action: String,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    how: Option<String>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    flow: Option<String>,
    #[serde(default)]
    todos: Option<Vec<String>>,
    #[serde(default)]
    status_summary: Option<String>,
    #[serde(default)]
    stage_key: Option<String>,
    #[serde(default)]
    append_note: Option<String>,
    #[serde(default)]
    completion_state: Option<String>,
    #[serde(default)]
    completion_conclusion: Option<String>,
    #[serde(default)]
    trigger: Option<TaskTriggerInputLocal>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PlanToolArgs {
    action: String,
    #[serde(default)]
    context: String,
}
