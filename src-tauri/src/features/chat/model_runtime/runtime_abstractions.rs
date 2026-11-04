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

// ==================== 工具执行计数（跨重试边界） ====================

// 一次模型调度里，工具到底执行过多少次。
//
// 重试与切换候选模型都拿同一个 prompt 快照重跑整条工具循环，只要这一 attempt 已经
// 执行过工具，再跑一遍就会把同一批副作用（写文件、跑 shell、发消息、起委托）重复执行一次。
// 工具循环与重试循环之间隔着 inference_gateway / router，拿不到执行结果，因此由调用方
// 用 scope 挂一份计数，工具执行收口处递增，收口后把计数带回来做重试决策。
tokio::task_local! {
    static RUNTIME_TOOL_EXECUTION_COUNT: std::sync::Arc<std::sync::atomic::AtomicUsize>;
}

/// 为一个 dispatch 建立工具执行计数作用域；返回值额外给出本次作用域内执行了几次工具。
async fn scoped_runtime_tool_execution_count<F, T>(future: F) -> (T, usize)
where
    F: std::future::Future<Output = T>,
{
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handle = std::sync::Arc::clone(&counter);
    let output = RUNTIME_TOOL_EXECUTION_COUNT.scope(counter, future).await;
    (
        output,
        handle.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// 工具执行收口处调用：记录这一 attempt 已经产生过工具副作用。
///
/// 计数只对 scoped 作用域生效；不在作用域内的调用（单元测试、其它调用方）什么都不做。
fn note_runtime_tool_executed() {
    let _ = RUNTIME_TOOL_EXECUTION_COUNT
        .try_with(|counter| counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
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

// ==================== 工具回执的人话层 ====================
//
// 结果区是纯文本渲染，读不懂就是回执没写好——所以表型工具（名册、任务、后台、
// 看板、搜索、窗口、表情、会话）在这里就把数据排成一段能直接读的文本，
// 而不是把结构化字段丢给前端去拼。
//
// 只做排版，不补字段：需要的信息如果没在返回值里，那是上一步删过头了。

fn receipt_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn receipt_count(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn receipt_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `1 teammate` / `3 teammates`
fn receipt_plural(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn tool_receipt_text(tool_name: &str, value: &Value) -> Option<String> {
    match tool_name {
        "recall" => {
            if value_string(value, "reason").as_deref() == Some("memory_recall_disabled") {
                return Some("memory recall disabled".to_string());
            }
            let memories = value_string(value, "memories").unwrap_or_default();
            if memories.trim().is_empty() {
                return Some("no memories".to_string());
            }
            Some(format!(
                "recalled {} of {} (offset {}, limit {})\n{}",
                receipt_count(value, "count"),
                receipt_count(value, "total"),
                receipt_count(value, "offset"),
                receipt_count(value, "limit"),
                memories
            ))
        }
        "remember" => {
            if !receipt_bool(value, "saved") {
                return Some("memory save failed".to_string());
            }
            let id = value_string(value, "id").unwrap_or_default();
            let merged = receipt_array(value, "deletedSourceIds").len();
            let failed = receipt_array(value, "failedDeleteIds").len();
            let mut text = if merged > 0 {
                format!("merged {merged} memories into {id}")
            } else {
                format!("remembered {id}")
            };
            if failed > 0 {
                text.push_str(&format!("\n{failed} old memories failed to delete"));
            }
            Some(text)
        }
        "plan" => {
            let action = value_string(value, "action").unwrap_or_default();
            let path = value_string(value, "path").unwrap_or_default();
            let state = match action.as_str() {
                "present" if receipt_bool(value, "auto_approved") => {
                    "plan presented, auto approved"
                }
                "present" => "plan presented, awaiting confirmation",
                "complete" => "plan completed",
                _ => "plan updated",
            };
            Some(format!("{state}\n{path}"))
        }
        "team_info" => {
            let members = receipt_array(value, "members");
            let mut lines = Vec::<String>::new();
            if members.is_empty() {
                lines.push("no teammates".to_string());
            } else {
                lines.push(receipt_plural(members.len(), "teammate", "teammates"));
            }
            for member in members {
                let role = value_string(member, "role").unwrap_or_else(|| "unknown".to_string());
                let status = value_string(member, "status").unwrap_or_default();
                let state = value_string(member, "state").unwrap_or_default();
                let duty = value_string(member, "duty").unwrap_or_default();
                let branch = value_string(member, "workBranch").unwrap_or_default();
                let conversation = value_string(member, "conversationId").unwrap_or_default();
                let mut line = format!("- {role} [{status}]");
                if !state.is_empty() && !state.eq_ignore_ascii_case("idle") {
                    line.push_str(&format!(" {state}"));
                }
                if !duty.is_empty() {
                    line.push(' ');
                    line.push_str(&duty);
                }
                if !branch.is_empty() {
                    line.push_str(&format!(" (branch {branch})"));
                }
                if !conversation.is_empty() {
                    line.push_str(&format!("  conv:{conversation}"));
                }
                lines.push(line);
            }
            let closed = receipt_array(value, "closedMissingMembers").len();
            if closed > 0 {
                lines.push(format!(
                    "closed {} missing member(s) whose conversation is gone",
                    closed
                ));
            }
            Some(lines.join("\n"))
        }
        "team_read" => {
            let messages = receipt_array(value, "messages");
            if messages.is_empty() {
                return Some("no messages".to_string());
            }
            let mut header = receipt_plural(messages.len(), "message", "messages");
            if receipt_bool(value, "hasMore") {
                header.push_str(", more available");
            }
            let mut lines = vec![header];
            for message in messages {
                let title = value_string(message, "title").unwrap_or_default();
                let summary = value_string(message, "summary").unwrap_or_default();
                let text = if title.is_empty() {
                    summary
                } else if summary.is_empty() {
                    title
                } else {
                    format!("{title} — {summary}")
                };
                lines.push(format!("- {text}"));
            }
            for note in receipt_array(value, "notes") {
                if let Some(text) = note.as_str() {
                    lines.push(format!("({text})"));
                }
            }
            Some(lines.join("\n"))
        }
        "task" => {
            let items: Vec<Value> = match value {
                Value::Array(items) => items.clone(),
                Value::Object(_) => vec![value.clone()],
                _ => Vec::new(),
            };
            if items.is_empty() {
                return Some("no tasks".to_string());
            }
            let mut lines = vec![receipt_plural(items.len(), "task", "tasks")];
            for item in &items {
                // TaskEntry 按蛇形序列化；驼峰只是旧夹具的写法，兼容读一下。
                let id = value_string(item, "task_id")
                    .or_else(|| value_string(item, "taskId"))
                    .unwrap_or_else(|| "?".to_string());
                let state = value_string(item, "completion_state")
                    .or_else(|| value_string(item, "completionState"))
                    .unwrap_or_default();
                let goal = value_string(item, "goal").unwrap_or_default();
                let conclusion = value_string(item, "completion_conclusion")
                    .or_else(|| value_string(item, "completionConclusion"))
                    .unwrap_or_default();
                let next = item
                    .get("trigger")
                    .and_then(|trigger| {
                        value_string(trigger, "next_run_at")
                            .or_else(|| value_string(trigger, "nextRunAt"))
                    })
                    .unwrap_or_default();
                let mut line = format!("- {id} [{state}]");
                if !goal.is_empty() {
                    line.push(' ');
                    line.push_str(&goal);
                }
                if !next.is_empty() {
                    line.push_str(&format!("  next:{next}"));
                }
                if !conclusion.is_empty() {
                    line.push_str(&format!("  ({conclusion})"));
                }
                lines.push(line);
            }
            Some(lines.join("\n"))
        }
        "background" => {
            let items = receipt_array(value, "items");
            if !items.is_empty() {
                let running = items
                    .iter()
                    .filter(|item| {
                        value_string(item, "status")
                            .map(|status| status.eq_ignore_ascii_case("running"))
                            .unwrap_or(false)
                    })
                    .count();
                let mut lines = vec![if running > 0 {
                    format!(
                        "{running} of {} background tasks running",
                        items.len()
                    )
                } else {
                    receipt_plural(items.len(), "background task", "background tasks")
                }];
                for item in items {
                    let id = value_string(item, "id").unwrap_or_else(|| "?".to_string());
                    let kind = value_string(item, "kind").unwrap_or_default();
                    let status = value_string(item, "status").unwrap_or_default();
                    let title = value_string(item, "title").unwrap_or_default();
                    let started = value_string(item, "startedAt").unwrap_or_default();
                    let mut line = format!("- {id} {kind} [{status}]");
                    if !title.is_empty() {
                        line.push(' ');
                        line.push_str(&title);
                    }
                    if !started.is_empty() {
                        line.push_str(&format!("  since {started}"));
                    }
                    lines.push(line);
                }
                return Some(lines.join("\n"));
            }
            let id = value_string(value, "id").unwrap_or_default();
            let status = value_string(value, "status").unwrap_or_default();
            let detail = value_string(value, "detail")
                .or_else(|| value_string(value, "log"))
                .unwrap_or_default();
            if id.is_empty() && status.is_empty() {
                return None;
            }
            let mut text = format!("{id} {status}").trim().to_string();
            // shell kill 没杀成时 JSON 里带 alreadyEnded：正文必须说清是早就结束了，
            // 否则模型只看到编号加状态，分不清这次到底杀掉了没有。
            if receipt_bool(value, "alreadyEnded") {
                text.push_str(" (already ended)");
            }
            if !detail.is_empty() {
                text.push('\n');
                text.push_str(&detail);
            }
            Some(text)
        }
        "windows" => {
            let mut lines = Vec::<String>::new();
            let mut total = 0usize;
            // 同标题的两条只能靠进程区分，自身窗口还得标出来（截图截不到）。
            // window list 的 JSON 自带 processId，有就直接用；个别只回编号的场景才回查一次枚举。
            let self_pid = std::process::id();
            let mut pid_by_window: Option<std::collections::HashMap<usize, u32>> = None;
            for step in receipt_array(value, "steps") {
                for window in receipt_array(step, "windows") {
                    let id = window.get("windowId").and_then(Value::as_i64).unwrap_or(0);
                    let title = value_string(window, "title").unwrap_or_default();
                    let marker = if receipt_bool(window, "focused") {
                        " (focused)"
                    } else {
                        ""
                    };
                    let pid = window
                        .get("processId")
                        .or_else(|| window.get("process_id"))
                        .and_then(Value::as_u64)
                        .map(|pid| pid as u32)
                        .or_else(|| {
                            usize::try_from(id).ok().and_then(|window_id| {
                                pid_by_window
                                    .get_or_insert_with(|| {
                                        crate::platform::list_all_windows()
                                            .into_iter()
                                            .map(|info| (info.window_id, info.process_id))
                                            .collect()
                                    })
                                    .get(&window_id)
                                    .copied()
                            })
                        });
                    let pid_note = match pid {
                        Some(pid) if pid == self_pid => format!(" (pid {pid}, self)"),
                        Some(pid) => format!(" (pid {pid})"),
                        None => String::new(),
                    };
                    lines.push(format!("- {id} {title}{marker}{pid_note}"));
                    total += 1;
                }
                if step
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(|kind| kind.eq_ignore_ascii_case("activateWindow"))
                    .unwrap_or(false)
                {
                    let id = receipt_count(step, "windowId");
                    if receipt_bool(step, "ok") {
                        lines.push(format!("- activated window {id}"));
                    } else {
                        lines.push(format!("- failed to activate window {id}"));
                    }
                }
            }
            if lines.is_empty() {
                return Some("no windows".to_string());
            }
            let mut out = vec![receipt_plural(total, "window", "windows")];
            out.extend(lines);
            Some(out.join("\n"))
        }
        "websearch" => {
            let results = receipt_array(value, "results");
            if results.is_empty() {
                return Some("no results".to_string());
            }
            let mut lines = vec![receipt_plural(results.len(), "result", "results")];
            for result in results {
                let title = value_string(result, "title")
                    .or_else(|| value_string(result, "name"))
                    .unwrap_or_default();
                let url = value_string(result, "url").unwrap_or_default();
                let snippet = value_string(result, "snippet")
                    .or_else(|| value_string(result, "text"))
                    .unwrap_or_default();
                let head = if title.is_empty() {
                    url.clone()
                } else if url.is_empty() {
                    title.clone()
                } else {
                    format!("{title} — {url}")
                };
                lines.push(format!("- {head}"));
                if !snippet.is_empty() {
                    lines.push(format!("  {snippet}"));
                }
            }
            Some(lines.join("\n"))
        }
        "meme" => {
            let memes = receipt_array(value, "memes");
            if memes.is_empty() {
                return Some("no memes".to_string());
            }
            let mut lines = vec![receipt_plural(memes.len(), "meme", "memes")];
            for meme in memes {
                let emotion = value_string(meme, "emotion").unwrap_or_default();
                let path = value_string(meme, "path")
                    .or_else(|| value_string(meme, "relativePath"))
                    .unwrap_or_default();
                lines.push(format!("- {emotion}: {path}"));
            }
            Some(lines.join("\n"))
        }
        "get_session" => {
            let Value::Array(items) = value else {
                return None;
            };
            if items.is_empty() {
                return Some("no sessions".to_string());
            }
            let mut lines = vec![receipt_plural(items.len(), "session", "sessions")];
            for item in items {
                let title = value_string(item, "title").unwrap_or_default();
                let kind = value_string(item, "kind").unwrap_or_default();
                let id = value_string(item, "session_id")
                    .or_else(|| value_string(item, "sessionId"))
                    .unwrap_or_default();
                lines.push(format!("- {title} [{kind}] {id}"));
            }
            Some(lines.join("\n"))
        }
        "inform_session" => {
            let id = value_string(value, "session_id")
                .or_else(|| value_string(value, "sessionId"))
                .unwrap_or_default();
            let pushed = receipt_bool(value, "pushed_to_remote");
            Some(if pushed {
                format!("sent to {id} (delivered to remote)")
            } else {
                format!("sent to {id}")
            })
        }
        "team_recruit" => {
            let role = value_string(value, "role").unwrap_or_default();
            Some(format!("recruited {role} — not activated yet, use team_send"))
        }
        "image_generate" | "image_edit" | "image_to_video" => {
            value_string(value, "message").map(|message| message)
        }
        _ => None,
    }
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
    if let Some(text) = tool_receipt_text(tool_name, value) {
        return text;
    }
    match tool_name {
        "exec" => {
            if value.get("background").and_then(Value::as_bool) == Some(true) {
                // 后台态是一份结构，正文只给拍平后的可读行，不再整体序列化成 JSON。
                return tool_value_readable_text(value);
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
        "image_generate" | "image_edit" | "image_to_video" => value.get("message").and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_value_readable_text(value)),
        _ => tool_value_readable_text(value),
    }
}

fn provider_tool_result_from_value(tool_name: &str, mut value: Value) -> ProviderToolResult {
    let metadata = provider_tool_metadata_from_value(tool_name, &value);
    let images = match tool_name {
        "operate" | "read_media" => {
            let payload = value.get("data").unwrap_or(&value);
            extract_forward_images_from_value(payload)
        }
        // read 的 PDF 图片模式（readerKind=pdf_image_direct）把每页 base64 放在顶层 parts 数组，
        // 必须提取成 Image part 走图片通道，否则会被当作文本预算截断。
        // 文本/Office 模式的结果没有 parts，extract 返回空，行为不变。
        "read" => extract_forward_images_from_value(&value),
        _ => Vec::new(),
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
    // 这里列出的是会被从工具返回值顶层剥离的内部字段名。
    // 新增工具的返回值若用到这些词（尤其 "done"，太容易与业务语义撞上），
    // 改成 JSON 返回前必须先从本名单移除对应项，否则会被静默剥掉。
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
        // TaskEntry 按蛇形序列化，夹具照真实形状来。
        let result = provider_tool_result_from_value(
            "task",
            serde_json::json!({
                "task_id": "task-1",
                "completion_state": "completed",
                "goal": "第一项",
            }),
        );
        assert!(result.output.contains("task-1"));
        assert!(result.output.contains("completed"));
        assert!(!result.output.contains("{\""));
        assert!(!result.output.contains("task_id:"));
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
    fn operate_result_forwards_every_screenshot_part_not_base64_text() {
        let result = provider_tool_result_from_value(
            "operate",
            serde_json::json!({
                "ok": true,
                "executedCount": 3,
                "latestScreenshot": { "mode": "region", "width": 60, "height": 40, "bounds": { "x": 10, "y": 20, "width": 60, "height": 40 } },
                "screenshots": [
                    {
                        "type": "image",
                        "mimeType": "image/webp",
                        "data": "FIRST",
                        "width": 100,
                        "height": 80,
                        "line": 1,
                        "mode": "desktop",
                        "bounds": { "x": 0, "y": 0, "width": 100, "height": 80 }
                    },
                    {
                        "type": "image",
                        "mimeType": "image/webp",
                        "data": "SECOND",
                        "width": 60,
                        "height": 40,
                        "line": 3,
                        "mode": "region",
                        "bounds": { "x": 10, "y": 20, "width": 60, "height": 40 }
                    }
                ]
            }),
        );

        assert!(!result.output.contains("FIRST"));
        assert!(!result.output.contains("SECOND"));
        let images = result
            .parts
            .iter()
            .filter_map(|part| match part {
                ProviderToolResultPart::Image { data_base64, width, height, .. } => {
                    Some((data_base64.clone(), *width, *height))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            images,
            vec![
                ("FIRST".to_string(), 100, 80),
                ("SECOND".to_string(), 60, 40),
            ],
            "一次调用内的每张截图都应按执行顺序进入图片通道"
        );
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
    fn read_pdf_image_mode_forwards_image_parts_not_base64_text() {
        let result = provider_tool_result_from_value(
            "read",
            serde_json::json!({
                "ok": true,
                "path": "C:/tmp/doc.pdf",
                "detectedType": "pdf",
                "readerKind": "pdf_image_direct",
                "truncated": false,
                "parts": [
                    {"type": "image", "mimeType": "image/webp", "data": "AAAABBBB", "pageIndex": 0, "pageNumber": 1, "width": 100, "height": 80},
                    {"type": "image", "mimeType": "image/webp", "data": "CCCCDDDD", "pageIndex": 1, "pageNumber": 2, "width": 120, "height": 90}
                ],
                "response": {"ok": true, "returnedPageCount": 2, "totalPages": 5}
            }),
        );

        // Text + 2 Image part
        assert_eq!(result.parts.len(), 3);
        // base64 不再出现在文本投影里
        assert!(!result.output.contains("AAAABBBB"));
        assert!(!result.output.contains("CCCCDDDD"));
        match &result.parts[1] {
            ProviderToolResultPart::Image { mime, data_base64, width, height } => {
                assert_eq!(mime, "image/webp");
                assert_eq!(data_base64, "AAAABBBB");
                assert_eq!(*width, 100);
                assert_eq!(*height, 80);
            }
            other => panic!("expected image part, got {other:?}"),
        }
        match &result.parts[2] {
            ProviderToolResultPart::Image { mime, data_base64, width, height } => {
                assert_eq!(mime, "image/webp");
                assert_eq!(data_base64, "CCCCDDDD");
                assert_eq!(*width, 120);
                assert_eq!(*height, 90);
            }
            other => panic!("expected image part, got {other:?}"),
        }
    }

    #[test]
    fn read_text_result_keeps_no_image_part() {
        let result = provider_tool_result_from_value(
            "read",
            serde_json::json!({
                "ok": true,
                "path": "C:/tmp/a.rs",
                "detectedType": "text",
                "content": "fn main() {}"
            }),
        );

        assert_eq!(result.output, "fn main() {}");
        assert_eq!(result.parts.len(), 1);
    }

    #[test]
    fn exec_background_result_should_flatten_fields_instead_of_dumping_json() {
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

        assert!(result.output.contains("backgroundId: bg-shell-1"));
        assert!(result.output.contains("logPath: C:/workspace/logs/bg-shell-1.log"));
        assert!(result.output.contains("status: running"));
        assert!(
            !result.output.contains("{\"ok\":true"),
            "后台态正文不得是返回值的 JSON 序列化体：{}",
            result.output
        );
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
