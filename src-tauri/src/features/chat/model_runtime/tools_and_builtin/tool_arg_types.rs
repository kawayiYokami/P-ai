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
struct CommandToolArgs {
    command: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TerminalExecToolArgs {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RemoteImSendToolArgs {
    #[serde(
        default = "remote_im_action_default",
        deserialize_with = "deserialize_remote_im_action"
    )]
    action: String,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    contact_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(
        default = "remote_im_send_status_default",
        deserialize_with = "deserialize_remote_im_send_status"
    )]
    status: String,
    #[serde(default)]
    file_paths: Option<Vec<String>>,
}

fn remote_im_action_default() -> String {
    "send".to_string()
}

fn deserialize_remote_im_action<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct RemoteImActionVisitor;

    impl<'de> serde::de::Visitor<'de> for RemoteImActionVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("action string: list or send")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "list" | "send" => Ok(normalized),
                _ => Err(E::custom(format!(
                    "action 非法：`{value}`。请返回正确动作：list 或 send"
                ))),
            }
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(remote_im_action_default())
        }
    }

    deserializer.deserialize_any(RemoteImActionVisitor)
}

fn remote_im_send_status_default() -> String {
    "done".to_string()
}

fn deserialize_remote_im_send_status<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct RemoteImStatusVisitor;

    impl<'de> serde::de::Visitor<'de> for RemoteImStatusVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("status string: continue or done")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value { "done" } else { "continue" }.to_string())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value == 0 { "continue" } else { "done" }.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value == 0 { "continue" } else { "done" }.to_string())
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "done" => Ok("done".to_string()),
                "continue" => Ok("continue".to_string()),
                "true" | "1" | "yes" | "y" | "on" => Ok("done".to_string()),
                "false" | "0" | "no" | "n" | "off" => Ok("continue".to_string()),
                _ => Err(E::custom(format!(
                    "status 非法：`{value}`。请返回正确状态：continue 或 done"
                ))),
            }
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(remote_im_send_status_default())
        }
    }

    deserializer.deserialize_any(RemoteImStatusVisitor)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DelegateToolArgs {
    department_id: String,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    task_name: Option<String>,
    instruction: String,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    specific_goal: Option<String>,
    #[serde(default)]
    deliverable_requirement: Option<String>,
    #[serde(default = "default_true")]
    notify_assistant_when_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelegateMode {
    Async,
    Sync,
}

fn parse_delegate_mode(raw: Option<&str>) -> Result<DelegateMode, String> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Err("delegate.mode is required".to_string()),
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
    title: Option<String>,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    goal: Option<String>,
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
    trigger: Option<TaskTriggerInput>,
}
