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
#[serde(deny_unknown_fields)]
struct MemorySaveToolArgs {
    action: String,
    #[serde(default, rename = "sourceMemoryIds")]
    source_memory_ids: Vec<String>,
    memory: MemorySaveToolMemoryArgs,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MemorySaveToolMemoryArgs {
    #[serde(rename = "memoryType")]
    memory_type: String,
    judgment: String,
    #[serde(default)]
    reasoning: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RecallToolArgs {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BackgroundToolArgs {
    action: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TerminalExecToolArgs {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    commitment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigToolArgs {
    command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReadMediaToolArgs {
    #[serde(alias = "absolute_path", alias = "absolutePath")]
    path: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ContactSendFilesToolArgs {
    file_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct GetSessionToolArgs {
    #[serde(default)]
    keyword: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InformSessionToolArgs {
    session_id: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamInfoToolArgs {
    /// active = 当前在队的队员；history = 含已请离的历史名册。
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamRecruitToolArgs {
    /// 队员的人格：填人格名，也兼容人格 id。同一支队伍里一个角色只能有一名队员。
    role: String,
    #[serde(default)]
    duty: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamSendToolArgs {
    /// 队员的角色（人格名，也兼容人格 id）。
    role: String,
    /// 邮件标题，必填。它就是正文第一行（`# 标题`），同时单列进 meta 供 `team_read` 回报。
    title: String,
    content: String,
    /// 这条线的状态（六态 key）。队长发件时设置，随邮件一起落进队员会话。
    #[serde(default)]
    status: Option<String>,
    /// guided = 引导（对方执行完一个工具就插入并重启）；queue = 排队等对方这一轮收尾。
    #[serde(default)]
    mode: Option<String>,
    /// 可选。仅在同一角色有多名在队队员（历史数据）时用来精确定位。
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamReportToolArgs {
    /// 邮件标题，必填。它就是正文第一行（`# 标题`），同时单列进 meta 供 `team_read` 回报。
    title: String,
    content: String,
    /// 汇报时顺手更新这条线的状态（六态 key）。不填就只汇报、不动状态。
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    mode: Option<String>,
}

/// 队长查队员最近的发言。`role` 与 `conversation_id` 二选一。
#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamReadToolArgs {
    /// 队员的角色（人格名，也兼容人格 id）。
    #[serde(default)]
    role: Option<String>,
    /// 可选。仅在同一角色有多名在队队员（历史数据）时用来精确定位。
    #[serde(default)]
    conversation_id: Option<String>,
    /// 想读几条有正文的消息，默认 1，上限 10；超上限按上限返回并说明。
    #[serde(default)]
    limit: Option<usize>,
}

/// 队长把自身模型推给队员。
#[derive(Debug, Clone, Deserialize, Serialize)]
struct TeamShareModelToolArgs {
    /// 队员的角色（人格名，也兼容人格 id）。
    role: String,
    /// 可选。要推给这名队员的模型；不给就用队长当前生效模型。
    #[serde(default)]
    model_api_config_id: Option<String>,
    /// 可选。仅在同一角色有多名在队队员（历史数据）时用来精确定位。
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegateToolArgs {
    agent_id: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    todo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    focus: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeepRecallToolArgs {
    query: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeepRecallSearchToolArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeepRecallContextToolArgs {
    conversation_index: usize,
    start_index: usize,
    end_index: usize,
}

fn delegate_arg_new_or_legacy(new_value: &Option<String>, legacy_value: &Option<String>) -> String {
    new_value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            legacy_value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default()
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegateMode {
    Background,
    Wait,
}

fn parse_delegate_mode(raw: Option<&str>) -> Result<DelegateMode, String> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(DelegateMode::Wait),
        Some("background") => Ok(DelegateMode::Background),
        Some("wait") => Ok(DelegateMode::Wait),
        Some(other) => Err(format!(
            "delegate.mode 必须是 `wait` 或 `background`，当前收到：{other}"
        )),
    }
}

#[cfg(test)]
mod tool_arg_types_tests {
    use super::*;

    #[test]
    fn parse_delegate_mode_should_default_to_wait() {
        assert_eq!(parse_delegate_mode(None).expect("default mode"), DelegateMode::Wait);
        assert_eq!(parse_delegate_mode(Some("")).expect("empty mode"), DelegateMode::Wait);
    }

    #[test]
    fn parse_delegate_mode_should_reject_legacy_values() {
        assert!(parse_delegate_mode(Some("sync")).is_err());
        assert!(parse_delegate_mode(Some("async")).is_err());
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
    todo: Option<String>,
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
#[serde(deny_unknown_fields)]
struct PlanToolArgs {
    action: String,
    path: String,
}
