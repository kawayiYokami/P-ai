#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ProviderToolDefinition {
    fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

#[allow(dead_code)]
trait RuntimeToolMetadata {
    fn provider_tool_definition(&self) -> ProviderToolDefinition;
}

type RuntimeToolCallFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ProviderToolResult, String>> + Send + 'a>,
>;
type RuntimeToolValueFuture<'a, E> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Value, E>> + Send + 'a>,
>;

trait RuntimeToolDyn: Send + Sync {
    fn name(&self) -> String;
    fn timeout_override(&self, _args_json: &str) -> Option<std::time::Duration> {
        None
    }
    fn is_mcp_tool(&self) -> bool {
        false
    }
    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_>;
}

trait RuntimeValueTool: RuntimeToolMetadata + Send + Sync {
    const NAME: &'static str;
    type Args: for<'de> Deserialize<'de> + Send;
    type Error: std::fmt::Display + Send;

    fn timeout_override(_args_json: &str) -> Option<std::time::Duration> {
        None
    }

    fn call_typed(&self, args: Self::Args) -> RuntimeToolValueFuture<'_, Self::Error>;
}

fn tool_value_scalar_text(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn push_tool_value_lines(value: &Value, indent: usize, lines: &mut Vec<String>) {
    let prefix = "  ".repeat(indent);
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                lines.push(format!("{prefix}(empty)"));
                return;
            }
            for item in items {
                if matches!(item, Value::Array(_) | Value::Object(_)) {
                    lines.push(format!("{prefix}-"));
                    push_tool_value_lines(item, indent + 1, lines);
                } else {
                    lines.push(format!("{prefix}- {}", tool_value_scalar_text(item)));
                }
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                lines.push(format!("{prefix}(empty)"));
                return;
            }
            for (key, item) in map {
                if item.is_null() || item.as_str().is_some_and(|text| text.is_empty()) {
                    continue;
                }
                if matches!(item, Value::Array(_) | Value::Object(_)) {
                    lines.push(format!("{prefix}{key}:"));
                    push_tool_value_lines(item, indent + 1, lines);
                } else {
                    let text = tool_value_scalar_text(item);
                    if text.contains('\n') {
                        lines.push(format!("{prefix}{key}:"));
                        lines.extend(text.lines().map(|line| format!("{}{}", "  ".repeat(indent + 1), line)));
                    } else {
                        lines.push(format!("{prefix}{key}: {text}"));
                    }
                }
            }
        }
        _ => lines.push(format!("{prefix}{}", tool_value_scalar_text(value))),
    }
}

fn tool_value_readable_text(value: &Value) -> String {
    if let Value::String(text) = value {
        return text.clone();
    }
    let mut lines = Vec::new();
    push_tool_value_lines(value, 0, &mut lines);
    lines.join("\n")
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty()).map(ToOwned::to_owned)
}

fn provider_tool_metadata_from_value(tool_name: &str, value: &Value) -> ProviderToolMetadata {
    let mut metadata = ProviderToolMetadata {
        backup_record_id: value_string(value, "backupRecordId"),
        ..ProviderToolMetadata::default()
    };
    match tool_name {
        "exec" => {
            metadata.exit_code = value.get("exitCode").and_then(Value::as_i64);
            metadata.wall_time_ms = value.get("durationMs").and_then(Value::as_u64);
            metadata.timed_out = value.get("timedOut").and_then(Value::as_bool).unwrap_or(false);
            metadata.truncated = value.get("truncated").and_then(Value::as_bool).unwrap_or(false);
            metadata.output_paths.extend(
                ["stdoutOutputPath", "stderrOutputPath"]
                    .into_iter()
                    .filter_map(|key| value_string(value, key)),
            );
        }
        "contact_send_files" => {
            let status = value_string(value, "status").unwrap_or_default();
            let stop = status.eq_ignore_ascii_case("done")
                || value.get("stop_tool_loop").and_then(Value::as_bool)
                    .or_else(|| value.get("done").and_then(Value::as_bool))
                    .unwrap_or(false);
            metadata.control = ProviderToolControl::Contact { stop };
        }
        "plan" => {
            metadata.control = ProviderToolControl::Plan {
                action: value_string(value, "action").unwrap_or_default(),
                path: value_string(value, "path").unwrap_or_default(),
                stop: value.get("should_stop_tool_loop").and_then(Value::as_bool)
                    .or_else(|| value.get("stop_tool_loop").and_then(Value::as_bool))
                    .unwrap_or(false),
            };
        }
        "task" => {
            metadata.control = ProviderToolControl::Task {
                completion_state: value_string(value, "completionState")
                    .or_else(|| value_string(value, "completion_state")),
                completion_conclusion: value_string(value, "completionConclusion")
                    .or_else(|| value_string(value, "completion_conclusion")),
            };
        }
        _ => {}
    }
    metadata
}

fn provider_tool_output_from_value(tool_name: &str, value: &Value) -> String {
    match tool_name {
        "exec" => {
            if value.get("background").and_then(Value::as_bool) == Some(true) {
                return serde_json::to_string(value).unwrap_or_else(|_| tool_value_readable_text(value));
            }
            if let Some(aggregated_output) = value
                .get("aggregatedOutput")
                .and_then(Value::as_str)
            {
                return aggregated_output.to_string();
            }
            let stdout = value.get("stdout").and_then(Value::as_str).unwrap_or_default();
            let stderr = value.get("stderr").and_then(Value::as_str).unwrap_or_default();
            match (stdout.is_empty(), stderr.is_empty()) {
                (false, true) => stdout.to_string(),
                (true, false) => stderr.to_string(),
                (false, false) => format!("{stdout}\n{stderr}"),
                (true, true) => {
                    let mut text = value
                        .get("message")
                        .and_then(Value::as_str)
                        .or_else(|| value.get("blockedReason").and_then(Value::as_str))
                        .unwrap_or("(no output)")
                        .to_string();
                    if let Some(hint) = value.get("commitmentHint").and_then(Value::as_str) {
                        text.push_str("\n\n");
                        text.push_str(hint);
                    }
                    text
                }
            }
        }
        "fetch" => value.get("content").and_then(Value::as_str).filter(|text| !text.is_empty())
            .or_else(|| value.get("message").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_value_readable_text(value)),
        "config" => value.get("result")
            .map(tool_value_readable_text)
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| tool_value_readable_text(value)),
        "read" | "read_media" => value.get("content").and_then(Value::as_str)
            .or_else(|| value.get("text").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_value_readable_text(value)),
        "image_generate" | "image_edit" => value.get("message").and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_value_readable_text(value)),
        _ => tool_value_readable_text(value),
    }
}

fn provider_tool_result_from_value(tool_name: &str, mut value: Value) -> ProviderToolResult {
    let metadata = provider_tool_metadata_from_value(tool_name, &value);
    let images = if tool_name == "operate" || tool_name == "read_media" {
        let payload = value.get("data").unwrap_or(&value);
        extract_forward_images_from_value(payload)
    } else {
        Vec::new()
    };
    if !images.is_empty() {
        remove_inline_media_from_tool_value(&mut value);
    }
    remove_top_level_internal_tool_fields(&mut value);
    let output = provider_tool_output_from_value(tool_name, &value);
    let mut parts = vec![ProviderToolResultPart::Text { text: output.clone() }];
    parts.extend(images.into_iter().map(|image| ProviderToolResultPart::Image {
        mime: image.mime,
        data_base64: image.base64,
        width: image.width,
        height: image.height,
    }));
    ProviderToolResult {
        output,
        metadata,
        parts,
        is_error: false,
    }
}

fn remove_top_level_internal_tool_fields(value: &mut Value) {
    let Value::Object(map) = value else {
        return;
    };
    const INTERNAL_FIELDS: &[&str] = &[
        "ok",
        "approved",
        "durationMs",
        "elapsed_ms",
        "timedOut",
        "truncated",
        "stdoutTruncated",
        "stderrTruncated",
        "stdoutOutputPath",
        "stderrOutputPath",
        "backupRecordId",
        "stop_tool_loop",
        "should_stop_tool_loop",
        "done",
    ];
    for key in INTERNAL_FIELDS {
        map.remove(*key);
    }
}

fn parse_runtime_tool_args<T>(args_json: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    if args_json.trim().is_empty() {
        serde_json::from_str::<T>("{}")
    } else {
        serde_json::from_str::<T>(args_json)
    }
    .map_err(|err| format!("Parse tool args failed: {err}"))
}

impl<T> RuntimeToolDyn for T
where
    T: RuntimeValueTool,
{
    fn name(&self) -> String {
        T::NAME.to_string()
    }

    fn timeout_override(&self, args_json: &str) -> Option<std::time::Duration> {
        T::timeout_override(args_json)
    }

    fn is_mcp_tool(&self) -> bool {
        false
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        Box::pin(async move {
            let args = parse_runtime_tool_args::<T::Args>(&args_json)?;
            let output_value = self.call_typed(args).await.map_err(|err| err.to_string())?;
            Ok(provider_tool_result_from_value(T::NAME, output_value))
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProviderToolCallRequest {
    invocation_id: String,
    provider_call_id: Option<String>,
    tool_name: String,
    arguments: Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum ProviderToolResultPart {
    Text {
        text: String,
    },
    Image {
        mime: String,
        data_base64: String,
        width: u32,
        height: u32,
    },
    Resource {
        mime: Option<String>,
        uri: Option<String>,
        text: String,
    },
    Audio {
        mime: String,
        data_base64: String,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProviderToolResult {
    output: String,
    metadata: ProviderToolMetadata,
    parts: Vec<ProviderToolResultPart>,
    is_error: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct ProviderToolMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wall_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "bool_is_false")]
    timed_out: bool,
    #[serde(skip_serializing_if = "bool_is_false")]
    truncated: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_output_lines: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup_record_id: Option<String>,
    #[serde(skip_serializing_if = "provider_tool_control_is_none")]
    control: ProviderToolControl,
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

fn provider_tool_control_is_none(control: &ProviderToolControl) -> bool {
    matches!(control, ProviderToolControl::None)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
enum ProviderToolControl {
    #[default]
    None,
    Contact { stop: bool },
    Plan { action: String, path: String, stop: bool },
    Task { completion_state: Option<String>, completion_conclusion: Option<String> },
}

#[allow(dead_code)]
impl ProviderToolResult {
    fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            output: text.clone(),
            metadata: ProviderToolMetadata::default(),
            parts: vec![ProviderToolResultPart::Text { text }],
            is_error: false,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            output: text.clone(),
            metadata: ProviderToolMetadata::default(),
            parts: vec![ProviderToolResultPart::Text { text }],
            is_error: true,
        }
    }
}

#[cfg(test)]
mod runtime_tool_result_tests {
    use super::*;

    #[test]
    fn structured_tool_result_is_readable_text_not_json() {
        let result = provider_tool_result_from_value(
            "task",
            serde_json::json!({
                "ok": true,
                "taskId": "task-1",
                "completionState": "completed",
                "items": [{"title": "第一项", "done": true}]
            }),
        );
        assert!(result.output.contains("taskId: task-1"));
        assert!(result.output.contains("completionState: completed"));
        assert!(!result.output.contains("{\""));
        assert!(!result.output.contains("ok:"));
        assert!(result.output.contains("done: true"));
        assert!(matches!(
            result.metadata.control,
            ProviderToolControl::Task { .. }
        ));
    }

    #[test]
    fn read_media_direct_image_result_forwards_image_part_not_base64_text() {
        let result = provider_tool_result_from_value(
            "read_media",
            serde_json::json!({
                "ok": true,
                "mediaType": "image",
                "path": "C:/tmp/a.png",
                "text": "已直接返回原图（未生成文字描述），请查看图片内容。",
                "imageMime": "image/png",
                "imageBase64": "AAAABBBB",
                "directImage": true
            }),
        );

        assert_eq!(result.output, "已直接返回原图（未生成文字描述），请查看图片内容。");
        assert!(!result.output.contains("AAAABBBB"));
        assert_eq!(result.parts.len(), 2);
        match &result.parts[1] {
            ProviderToolResultPart::Image { mime, data_base64, .. } => {
                assert_eq!(mime, "image/png");
                assert_eq!(data_base64, "AAAABBBB");
            }
            other => panic!("expected image part, got {other:?}"),
        }
    }

    #[test]
    fn read_media_described_result_keeps_no_image_part() {
        let result = provider_tool_result_from_value(
            "read_media",
            serde_json::json!({
                "ok": true,
                "mediaType": "audio",
                "text": "音频转写：你好",
                "cached": false
            }),
        );

        assert_eq!(result.output, "音频转写：你好");
        assert_eq!(result.parts.len(), 1);
    }

    #[test]
    fn exec_background_result_keeps_full_json_for_projection() {
        let value = serde_json::json!({
            "ok": true,
            "background": true,
            "backgroundId": "bg-shell-1",
            "id": "bg-shell-1",
            "status": "running",
            "command": "sleep 10",
            "description": "等待",
            "cwd": "C:/workspace",
            "mode": "background",
            "logPath": "C:/workspace/logs/bg-shell-1.log"
        });
        let result = provider_tool_result_from_value("exec", value);

        assert!(result.output.contains("\"backgroundId\":\"bg-shell-1\""));
        assert!(result.output.contains("\"logPath\""));
        assert!(result.output.contains("\"status\":\"running\""));
    }

    #[test]
    fn exec_result_keeps_raw_output_and_external_metadata() {
        let result = provider_tool_result_from_value(
            "exec",
            serde_json::json!({
                "ok": true,
                "exitCode": 0,
                "stdout": "first\nsecond \"quoted\"",
                "stderr": "stderr",
                "aggregatedOutput": "first\nsecond \"quoted\"stderr",
                "durationMs": 420,
                "timedOut": false
            }),
        );
        assert_eq!(result.output, "first\nsecond \"quoted\"stderr");
        assert_eq!(result.metadata.exit_code, Some(0));
        assert_eq!(result.metadata.wall_time_ms, Some(420));
        assert!(!result.metadata.timed_out);
    }

    #[test]
    fn exec_blocked_result_includes_commitment_hint_for_ai() {
        let result = provider_tool_result_from_value(
            "exec",
            serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "local_rule_blocked",
                "message": "命令包含 git reset，本地规则已直接拦截。它会改动仓库历史或工作区状态，不进入 AI 评估。 你正在执行高度危险的指令。请向用户确认是否真的要执行，并向用户说明危险性；得到用户明确许可后，重新调用 exec 并在 commitment 参数中填入承诺文案。",
                "commitmentHint": "用户已经充分理解本命令的危险性并且批准本次执行",
                "toolReview": { "kind": "local_rule", "allow": false },
                "command": "git reset --hard HEAD~1"
            }),
        );
        assert!(result.output.contains("本地规则已直接拦截"));
        assert!(result.output.contains("commitment"));
        assert!(
            result
                .output
                .contains("用户已经充分理解本命令的危险性并且批准本次执行"),
            "被拦截时 AI 必须能看到承诺文案，否则无法回填 commitment 参数：{}",
            result.output
        );
        assert!(!result.output.contains("{\""));
    }
}
