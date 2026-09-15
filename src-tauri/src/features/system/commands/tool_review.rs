#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewConversationInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewCommitPageInput {
    conversation_id: String,
    page: usize,
    page_size: usize,
}

fn tool_review_delegate_background(scope: &str, target: Option<&str>) -> String {
    let mut lines = vec![format!("审查范围：{}", scope.trim())];
    if let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) {
        lines.push(format!("范围参数：{}", target));
    }
    lines.push("请你将以上选择内容视为审查目标，在当前工作区自行决定需要读取的只读 git 信息，再按 skill 输出 JSON。没有确认到真实缺陷时，findings 必须返回空数组。".to_string());
    lines.join("\n")
}

#[tauri::command]
async fn list_tool_review_commit_options(
    input: ToolReviewCommitPageInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewCommitOptionsOutput, String> {
    list_tool_review_commit_options_internal_command(input, state.inner()).await
}

async fn list_tool_review_commit_options_internal_command(
    input: ToolReviewCommitPageInput,
    state: &AppState,
) -> Result<ListToolReviewCommitOptionsOutput, String> {
    let conversation_id = input.conversation_id.trim();
    let page = input.page.max(1);
    let page_size = input.page_size.clamp(1, 100);
    runtime_log_info(format!(
        "[工具审查][commit列表] 开始 conversation_id={} page={} page_size={}",
        conversation_id, page, page_size
    ));
    if conversation_id.is_empty() {
        runtime_log_warn("[工具审查][commit列表] 跳过 conversation_id 为空".to_string());
        return Ok(ListToolReviewCommitOptionsOutput { total: 0, page, page_size, commits: Vec::new() });
    }
    let conversation = with_tool_review_conversation(state, conversation_id, |conversation| {
        Ok(conversation.clone())
    })
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][commit列表] 读取会话失败 conversation_id={} err={}",
            conversation_id, err
        ));
        err
    })?;
    let (total, commits) = tool_review_list_commit_options_internal(state, &conversation, page, page_size)
        .await
        .map_err(|err| {
            runtime_log_error(format!(
                "[工具审查][commit列表] 获取失败 conversation_id={} err={}",
                conversation_id, err
            ));
            err
        })?;
    runtime_log_info(format!(
        "[工具审查][commit列表] 完成 conversation_id={} total={} count={}",
        conversation_id,
        total,
        commits.len()
    ));
    Ok(ListToolReviewCommitOptionsOutput { total, page, page_size, commits })
}

async fn tool_review_list_commit_options_internal(
    state: &AppState,
    conversation: &Conversation,
    page: usize,
    page_size: usize,
) -> Result<(usize, Vec<ToolReviewCommitOption>), String> {
    let workspace_path = terminal_default_workspace_for_conversation_resolved(
        state,
        Some(conversation),
    )
    .map(|workspace| workspace.path)
    .map_err(|err| format!("当前会话缺少可用主工作区，无法读取 commit 列表：{}", err))?;
    let workspace_text = workspace_path.to_string_lossy().to_string();
    let total_command = "git rev-list --count HEAD";
    runtime_log_info(format!(
        "[工具审查][commit列表] 执行 git conversation_id={} cwd={} command={}",
        conversation.id,
        workspace_text,
        total_command
    ));
    let total_output = tool_review_exec_git_readonly(
        state,
        &conversation.id,
        &workspace_path,
        total_command,
        120_000,
    )
    .await
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][commit列表] git失败 conversation_id={} cwd={} command={} err={}",
            conversation.id,
            workspace_text,
            total_command,
            err
        ));
        err
    })?;
    let total = total_output.trim().parse::<usize>().map_err(|err| {
        format!("无法解析 commit 总数：{}", err)
    })?;
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let command = format!("git log --skip {} -n {} --pretty=format:%H%x1f%h%x1f%s%x1f%cI", offset, page_size);
    runtime_log_info(format!(
        "[工具审查][commit列表] 执行 git conversation_id={} cwd={} command={}",
        conversation.id,
        workspace_text,
        command
    ));
    let output = tool_review_exec_git_readonly(
        state,
        &conversation.id,
        &workspace_path,
        &command,
        120_000,
    )
    .await
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][commit列表] git失败 conversation_id={} cwd={} command={} err={}",
            conversation.id,
            workspace_text,
            command,
            err
        ));
        err
    })?;
    runtime_log_info(format!(
        "[工具审查][commit列表] git完成 conversation_id={} cwd={} stdout_lines={}",
        conversation.id,
        workspace_text,
        output.lines().count()
    ));
    Ok((
        total,
        output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\u{1f}');
                let hash = parts.next()?.trim();
                let short_hash = parts.next()?.trim();
                let subject = parts.next()?.trim();
                let author_time = parts.next()?.trim();
                if hash.is_empty() || short_hash.is_empty() || subject.is_empty() {
                    return None;
                }
                Some(ToolReviewCommitOption {
                    hash: hash.to_string(),
                    short_hash: short_hash.to_string(),
                    subject: subject.to_string(),
                    author_time: author_time.to_string(),
                })
            })
            .collect(),
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewCommitOption {
    hash: String,
    short_hash: String,
    subject: String,
    author_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListToolReviewCommitOptionsOutput {
    total: usize,
    page: usize,
    page_size: usize,
    commits: Vec<ToolReviewCommitOption>,
}

fn tool_review_command_for_item(item: &ToolReviewCollectedItem) -> Option<String> {
    item.result_value
        .as_ref()
        .and_then(|value| tool_review_json_string_field(value, "command"))
        .or_else(|| tool_review_json_string_field(&item.args_value, "command"))
        .or_else(|| {
            let trimmed = item.args_text.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        })
        .map(ToOwned::to_owned)
}

fn tool_review_extract_patch_paths_from_text(input: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for line in input.lines() {
        let value = line
            .strip_prefix("*** Add File: ")
            .or_else(|| line.strip_prefix("*** Delete File: "))
            .or_else(|| line.strip_prefix("*** Update File: "))
            .or_else(|| line.strip_prefix("*** Move to: "))
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(path) = value {
            out.push(path.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn tool_review_is_item_denied(item: &ToolReviewCollectedItem) -> bool {
    let Some(result) = &item.result_value else {
        return false;
    };
    if let Some(approved) = result.get("approved").and_then(Value::as_bool) {
        if !approved {
            return true;
        }
    }
    if let Some(blocked) = result.get("blockedReason").and_then(Value::as_str) {
        let lower = blocked.to_ascii_lowercase();
        if lower.contains("denied") || lower == "rejected" || lower.contains("refused") {
            return true;
        }
    }
    false
}

fn tool_review_is_item_successful(item: &ToolReviewCollectedItem) -> bool {
    let Some(result) = &item.result_value else {
        let text = item.result_text.trim();
        if text.is_empty() {
            return false;
        }
        return !text.starts_with("Error:") && !text.starts_with("error:");
    };
    if tool_review_is_item_denied(item) {
        return false;
    }
    if let Some(ok) = result.get("ok").and_then(Value::as_bool) {
        if !ok {
            return false;
        }
    }
    if let Some(blocked) = result.get("blockedReason").and_then(Value::as_str) {
        if !blocked.is_empty() {
            return false;
        }
    }
    if result.get("error").is_some() {
        return false;
    }
    if let Some(exit_code) = result
        .get("exitCode")
        .or_else(|| result.get("exit_code"))
        .and_then(Value::as_i64)
    {
        if exit_code != 0 {
            return false;
        }
    }
    true
}

fn tool_review_patch_paths_for_item(item: &ToolReviewCollectedItem) -> Vec<String> {
    let mut out = Vec::<String>::new();
    if let Some(changed) = item
        .result_value
        .as_ref()
        .and_then(|value| value.get("changed"))
        .and_then(Value::as_array)
    {
        for entry in changed {
            for key in ["path", "from", "to"] {
                if let Some(path) = tool_review_json_string_field(entry, key) {
                    out.push(path.to_string());
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(operations) = item.args_value.get("operations").and_then(Value::as_array) {
            for operation in operations {
                for key in ["path", "to"] {
                    if let Some(path) = tool_review_json_string_field(operation, key) {
                        out.push(path.to_string());
                    }
                }
            }
        }
    }
    if out.is_empty() {
        for key in ["path", "file", "target", "from", "to"] {
            if let Some(path) = tool_review_json_string_field(&item.args_value, key) {
                out.push(path.to_string());
            }
        }
    }
    if out.is_empty() {
        let (_, preview_text) = tool_review_preview_for_item(item);
        out.extend(tool_review_extract_patch_paths_from_text(&preview_text));
    }
    out.sort();
    out.dedup();
    out
}

fn tool_review_patch_operation_for_item(item: &ToolReviewCollectedItem) -> Option<String> {
    let mut operations = Vec::<String>::new();
    if let Some(changed) = item
        .result_value
        .as_ref()
        .and_then(|value| value.get("changed"))
        .and_then(Value::as_array)
    {
        for entry in changed {
            if let Some(op) = tool_review_json_string_field(entry, "op") {
                let normalized = match op {
                    "add" => "add",
                    "delete" => "delete",
                    "update" | "update_move" => "update",
                    _ => "update",
                };
                operations.push(normalized.to_string());
            }
        }
    }
    if operations.is_empty() {
        if let Some(raw_operations) = item.args_value.get("operations").and_then(Value::as_array) {
            for operation in raw_operations {
                if let Some(action) = tool_review_json_string_field(operation, "action") {
                    let normalized = match action {
                        "add" => "add",
                        "delete" => "delete",
                        "move" => "update",
                        "update" => "update",
                        _ => "update",
                    };
                    operations.push(normalized.to_string());
                }
            }
        }
    }
    if operations.is_empty() {
        let (_, preview_text) = tool_review_preview_for_item(item);
        for line in preview_text.lines() {
            let operation = if line.starts_with("*** Add File: ") {
                Some("add")
            } else if line.starts_with("*** Delete File: ") {
                Some("delete")
            } else if line.starts_with("*** Update File: ") {
                Some("update")
            } else {
                None
            };
            if let Some(operation) = operation {
                operations.push(operation.to_string());
            }
        }
    }
    if operations.is_empty() {
        match item.tool_name.as_str() {
            "write" => operations.push("add".to_string()),
            "delete" => operations.push("delete".to_string()),
            "update" => operations.push("update".to_string()),
            "move" => operations.push("update".to_string()),
            _ => {}
        }
    }
    operations.sort();
    operations.dedup();
    match operations.as_slice() {
        [] => None,
        [single] => Some(single.clone()),
        _ => Some("mixed".to_string()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewCallInput {
    conversation_id: String,
    call_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewBatchActionInput {
    conversation_id: String,
    batch_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewCodeReviewInput {
    conversation_id: String,
    scope: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteToolReviewReportInput {
    conversation_id: String,
    report_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewStoredReview {
    kind: String,
    allow: bool,
    review_opinion: String,
    model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewItemSummary {
    call_id: String,
    tool_name: String,
    order_index: usize,
    has_review: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_opinion: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    affected_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch_operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at: Option<String>,
    #[serde(default)]
    added_lines: usize,
    #[serde(default)]
    deleted_lines: usize,
    #[serde(default = "tool_review_default_true")]
    is_success: bool,
    #[serde(default)]
    is_denied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocked_reason: Option<String>,
}

fn tool_review_default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewReportRecord {
    id: String,
    conversation_id: String,
    #[serde(default)]
    title: String,
    status: String,
    scope: String,
    target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
    workspace_path: String,
    created_at: String,
    updated_at: String,
    report_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delegate_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewBatchSummary {
    batch_key: String,
    user_message_id: String,
    user_message_text: String,
    item_count: usize,
    unreviewed_count: usize,
    changed_files: usize,
    added_lines: usize,
    deleted_lines: usize,
    items: Vec<ToolReviewItemSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListToolReviewBatchesOutput {
    batches: Vec<ToolReviewBatchSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_batch_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewItemDetail {
    batch_key: String,
    call_id: String,
    message_id: String,
    tool_name: String,
    order_index: usize,
    has_review: bool,
    preview_kind: String,
    preview_text: String,
    result_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<ToolReviewStoredReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewSegment {
    path: String,
    action: String,
    diff_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewBatchDetailsOutput {
    batch_key: String,
    segments: Vec<ToolReviewSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunToolReviewBatchOutput {
    batch_key: String,
    reviewed_call_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitToolReviewCodeOutput {
    report: ToolReviewReportRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListToolReviewReportsOutput {
    reports: Vec<ToolReviewReportRecord>,
}

#[derive(Debug, Clone)]
struct ToolReviewCollectedItem {
    batch_key: String,
    call_id: String,
    message_id: String,
    finished_at: Option<String>,
    tool_name: String,
    order_index: usize,
    args_value: Value,
    args_text: String,
    result_text: String,
    result_value: Option<Value>,
    review_value: Option<Value>,
}

#[derive(Debug, Clone)]
struct ToolReviewCollectedBatch {
    batch_key: String,
    user_message_id: String,
    user_message_text: String,
    items: Vec<ToolReviewCollectedItem>,
}

fn tool_review_user_message_text(message: &ChatMessage) -> String {
    let text = message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.trim()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        "（空白用户消息）".to_string()
    } else {
        text
    }
}

fn tool_review_json_string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().map(str::trim).filter(|item| !item.is_empty())
}

fn tool_review_value_to_stored_review(raw: &Value) -> Option<ToolReviewStoredReview> {
    let object = raw.as_object()?;
    Some(ToolReviewStoredReview {
        kind: object
            .get("kind")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("decision")
            .to_string(),
        allow: object.get("allow").and_then(Value::as_bool).unwrap_or(false),
        review_opinion: object
            .get("reviewOpinion")
            .or_else(|| object.get("review_opinion"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        model_name: object
            .get("modelName")
            .or_else(|| object.get("model_name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
        raw_content: object
            .get("rawContent")
            .or_else(|| object.get("raw_content"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

fn tool_review_normalize_review_value(raw: Option<Value>) -> Option<Value> {
    let value = raw?;
    match &value {
        Value::Null => None,
        Value::Object(object) if object.is_empty() => None,
        _ => Some(value),
    }
}

#[tauri::command]
fn delete_tool_review_report(
    input: DeleteToolReviewReportInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_tool_review_report_internal(input, state.inner())
}

fn delete_tool_review_report_internal(
    input: DeleteToolReviewReportInput,
    state: &AppState,
) -> Result<(), String> {
    let conversation_id = input.conversation_id.trim();
    let report_id = input.report_id.trim();
    if conversation_id.is_empty() || report_id.is_empty() {
        return Err("conversationId 和 reportId 不能为空。".to_string());
    }
    // 先读取报告，若有 delegate_id 则打断对应委托
    let reports = tool_review_read_report_records(&state.data_path, conversation_id)?;
    if let Some(report) = reports.iter().find(|r| r.id.trim() == report_id) {
        if let Some(ref delegate_id) = report.delegate_id {
            let did = delegate_id.trim();
            if !did.is_empty() {
                let _ = abort_delegate_runtime_thread(state, did, "用户删除审查报告时连带打断");
            }
        }
    }
    with_tool_review_conversation(state, conversation_id, |_conversation| {
        tool_review_delete_report_record(&state.data_path, conversation_id, report_id)
    })?;
    emit_tool_review_reports_updated(state, conversation_id, report_id, "deleted");
    Ok(())
}

fn tool_review_patch_operations_summary(args: &Value) -> Option<Vec<Value>> {
    let operations = args.get("operations")?.as_array()?;
    let mut out = Vec::<Value>::new();
    for operation in operations {
        let action = tool_review_json_string_field(operation, "action").unwrap_or("");
        let path = tool_review_json_string_field(operation, "path").unwrap_or("");
        let to = tool_review_json_string_field(operation, "to").unwrap_or("");
        let old_string = tool_review_json_string_field(operation, "old_string")
            .or_else(|| tool_review_json_string_field(operation, "oldString"))
            .unwrap_or("");
        let new_string = tool_review_json_string_field(operation, "new_string")
            .or_else(|| tool_review_json_string_field(operation, "newString"))
            .unwrap_or("");
        let content = tool_review_json_string_field(operation, "content").unwrap_or("");
        let replace_all = operation
            .get("replace_all")
            .or_else(|| operation.get("replaceAll"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        out.push(serde_json::json!({
            "action": action,
            "path": path,
            "to": to,
            "replace_all": replace_all,
            "old_preview": apply_patch_preview_text(old_string, 800),
            "new_preview": apply_patch_preview_text(new_string, 800),
            "content_preview": apply_patch_preview_text(content, 800),
        }));
    }
    Some(out)
}

fn tool_review_prefixed_preview_lines(prefix: &str, text: &str) -> Vec<String> {
    apply_patch_preview_text(text, 4_000)
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect()
}

fn tool_review_patch_line_ranges_from_changed_entry(entry: &Value) -> Vec<(usize, usize)> {
    entry
        .get("lineRanges")
        .and_then(Value::as_array)
        .map(|ranges| {
            ranges
                .iter()
                .filter_map(|range| {
                    let start = range.get("start")?.as_u64()? as usize;
                    let end = range.get("end")?.as_u64()? as usize;
                    Some((start, end))
                })
                .collect::<Vec<_>>()
        })
        .filter(|ranges| !ranges.is_empty())
        .or_else(|| {
            let start = entry.get("lineStart")?.as_u64()? as usize;
            let end = entry
                .get("lineEnd")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
                .unwrap_or(start);
            Some(vec![(start, end)])
        })
        .unwrap_or_default()
}

fn tool_review_patch_line_header(changed_entry: Option<&Value>) -> Option<String> {
    let ranges = changed_entry
        .map(tool_review_patch_line_ranges_from_changed_entry)
        .unwrap_or_default();
    if ranges.is_empty() {
        return None;
    }
    let formatted = ranges
        .into_iter()
        .map(apply_patch_format_line_range)
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("@@ {formatted} @@"))
}

fn tool_review_patch_operations_preview(args: &Value, changed_entries: Option<&[Value]>) -> Option<String> {
    let operations = args.get("operations")?.as_array()?;
    let mut lines = Vec::<String>::new();
    for (index, operation) in operations.iter().enumerate() {
        let action = tool_review_json_string_field(operation, "action").unwrap_or("");
        let old_string = tool_review_json_string_field(operation, "old_string")
            .or_else(|| tool_review_json_string_field(operation, "oldString"))
            .unwrap_or("");
        let new_string = tool_review_json_string_field(operation, "new_string")
            .or_else(|| tool_review_json_string_field(operation, "newString"))
            .unwrap_or("");
        let content = tool_review_json_string_field(operation, "content").unwrap_or("");
        match action {
            "add" => {
                lines.extend(tool_review_prefixed_preview_lines("+", content));
            }
            "delete" => {}
            "move" => {}
            _ => {
                if let Some(header) = tool_review_patch_line_header(changed_entries.and_then(|entries| entries.get(index))) {
                    lines.push(header);
                }
                lines.extend(tool_review_prefixed_preview_lines("-", old_string));
                lines.extend(tool_review_prefixed_preview_lines("+", new_string));
            }
        }
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn tool_review_single_edit_preview(tool_name: &str, args: &Value, changed_entry: Option<&Value>) -> Option<String> {
    let _path = tool_review_json_string_field(args, "path")?;
    let mut lines = Vec::<String>::new();
    match tool_name {
        "write" => {
            let content = tool_review_json_string_field(args, "content").unwrap_or("");
            let overwrite = args
                .get("overwrite")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if overwrite && changed_entry.is_some() {
                if let Some(header) = tool_review_patch_line_header(changed_entry) {
                    lines.push(header);
                }
            }
            if overwrite {
                lines.extend(tool_review_prefixed_preview_lines("+", content));
            } else {
                lines.extend(tool_review_prefixed_preview_lines("+", content));
            }
        }
        "delete" => {}
        "move" => {}
        "update" => {
            let old_string = tool_review_json_string_field(args, "old_string")
                .or_else(|| tool_review_json_string_field(args, "oldString"))
                .unwrap_or("");
            let new_string = tool_review_json_string_field(args, "new_string")
                .or_else(|| tool_review_json_string_field(args, "newString"))
                .unwrap_or("");
            if let Some(header) = tool_review_patch_line_header(changed_entry) {
                lines.push(header);
            }
            lines.extend(tool_review_prefixed_preview_lines("-", old_string));
            lines.extend(tool_review_prefixed_preview_lines("+", new_string));
        }
        _ => return None,
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn tool_review_preview_for_item(item: &ToolReviewCollectedItem) -> (String, String) {
    match item.tool_name.as_str() {
        "apply_patch" | "write" | "delete" | "update" | "move" => {
            let changed_entries = item
                .result_value
                .as_ref()
                .and_then(|value| value.get("changed"))
                .and_then(Value::as_array);
            if let Some(preview) = tool_review_patch_operations_preview(&item.args_value, changed_entries.map(|items| items.as_slice())) {
                return ("patch".to_string(), preview);
            }
            if let Some(preview) = tool_review_single_edit_preview(
                &item.tool_name,
                &item.args_value,
                changed_entries.and_then(|entries| entries.first()),
            ) {
                return ("patch".to_string(), preview);
            }
            let preview = tool_review_json_string_field(&item.args_value, "input")
                .or_else(|| tool_review_json_string_field(&item.args_value, "patch"))
                .unwrap_or(item.args_text.trim());
            ("patch".to_string(), preview.to_string())
        }
        _ => {
            let preview = item
                .result_value
                .as_ref()
                .and_then(|value| tool_review_json_string_field(value, "command"))
                .or_else(|| tool_review_json_string_field(&item.args_value, "command"))
                .unwrap_or(item.args_text.trim());
            ("command".to_string(), preview.to_string())
        }
    }
}

fn tool_review_git_hunk_header(
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
) -> String {
    format!("@@ -{},{} +{},{} @@", old_start, old_count, new_start, new_count)
}

fn tool_review_prefixed_git_lines(prefix: &str, text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect()
}

fn tool_review_segment_update_lines(
    old_start: usize,
    old_string: &str,
    new_string: &str,
) -> Vec<String> {
    let old_count = old_string.lines().count();
    let new_count = new_string.lines().count();
    let mut lines = vec![tool_review_git_hunk_header(old_start, old_count, old_start, new_count)];
    lines.extend(tool_review_prefixed_git_lines("-", old_string));
    lines.extend(tool_review_prefixed_git_lines("+", new_string));
    lines
}

fn tool_review_segments_for_apply_patch(item: &ToolReviewCollectedItem) -> Vec<ToolReviewSegment> {
    let mut segments = Vec::<ToolReviewSegment>::new();
    let Some(operations) = item.args_value.get("operations").and_then(Value::as_array) else {
        return segments;
    };
    let changed_entries = item
        .result_value
        .as_ref()
        .and_then(|value| value.get("changed"))
        .and_then(Value::as_array);
    for (index, operation) in operations.iter().enumerate() {
        let action = tool_review_json_string_field(operation, "action").unwrap_or("").to_string();
        let path = tool_review_json_string_field(operation, "path")
            .or_else(|| tool_review_json_string_field(operation, "to"))
            .unwrap_or("")
            .to_string();
        let old_string = tool_review_json_string_field(operation, "old_string")
            .or_else(|| tool_review_json_string_field(operation, "oldString"))
            .unwrap_or("");
        let new_string = tool_review_json_string_field(operation, "new_string")
            .or_else(|| tool_review_json_string_field(operation, "newString"))
            .unwrap_or("");
        let content = tool_review_json_string_field(operation, "content").unwrap_or("");
        let changed_entry = changed_entries.and_then(|entries| entries.get(index));
        match action.as_str() {
            "add" => {
                let new_count = content.lines().count();
                let mut lines = vec![tool_review_git_hunk_header(0, 0, 1, new_count)];
                lines.extend(tool_review_prefixed_git_lines("+", content));
                segments.push(ToolReviewSegment {
                    path,
                    action: "add".to_string(),
                    diff_lines: lines,
                });
            }
            "delete" => {
                segments.push(ToolReviewSegment {
                    path,
                    action: "delete".to_string(),
                    diff_lines: Vec::new(),
                });
            }
            "move" => {
                segments.push(ToolReviewSegment {
                    path,
                    action: "move".to_string(),
                    diff_lines: Vec::new(),
                });
            }
            _ => {
                let ranges = changed_entry
                    .map(tool_review_patch_line_ranges_from_changed_entry)
                    .unwrap_or_default();
                if ranges.is_empty() {
                    let mut lines = tool_review_segment_update_lines(1, old_string, new_string);
                    if old_string.is_empty() && !new_string.is_empty() {
                        lines = vec![tool_review_git_hunk_header(0, 0, 1, new_string.lines().count())];
                        lines.extend(tool_review_prefixed_git_lines("+", new_string));
                    }
                    segments.push(ToolReviewSegment {
                        path,
                        action: "update".to_string(),
                        diff_lines: lines,
                    });
                } else {
                    for (start, _end) in ranges {
                        let lines = tool_review_segment_update_lines(start, old_string, new_string);
                        segments.push(ToolReviewSegment {
                            path: path.clone(),
                            action: "update".to_string(),
                            diff_lines: lines,
                        });
                    }
                }
            }
        }
    }
    segments
}

fn tool_review_segments_for_single_edit(item: &ToolReviewCollectedItem) -> Vec<ToolReviewSegment> {
    let tool_name = item.tool_name.as_str();
    let path = tool_review_json_string_field(&item.args_value, "path")
        .or_else(|| tool_review_json_string_field(&item.args_value, "to"))
        .unwrap_or("")
        .to_string();
    let changed_entry = item
        .result_value
        .as_ref()
        .and_then(|value| value.get("changed"))
        .and_then(Value::as_array)
        .and_then(|entries| entries.first());
    match tool_name {
        "write" => {
            let content = tool_review_json_string_field(&item.args_value, "content").unwrap_or("");
            let new_count = content.lines().count();
            let mut lines = vec![tool_review_git_hunk_header(0, 0, 1, new_count)];
            lines.extend(tool_review_prefixed_git_lines("+", content));
            let entry_action = changed_entry
                .and_then(|entry| tool_review_json_string_field(entry, "op"))
                .unwrap_or("add")
                .to_string();
            vec![ToolReviewSegment {
                path,
                action: if entry_action == "update" { "update".to_string() } else { "add".to_string() },
                diff_lines: lines,
            }]
        }
        "update" => {
            let old_string = tool_review_json_string_field(&item.args_value, "old_string")
                .or_else(|| tool_review_json_string_field(&item.args_value, "oldString"))
                .unwrap_or("");
            let new_string = tool_review_json_string_field(&item.args_value, "new_string")
                .or_else(|| tool_review_json_string_field(&item.args_value, "newString"))
                .unwrap_or("");
            let ranges = changed_entry
                .map(tool_review_patch_line_ranges_from_changed_entry)
                .unwrap_or_default();
            if ranges.is_empty() {
                let lines = tool_review_segment_update_lines(1, old_string, new_string);
                vec![ToolReviewSegment {
                    path,
                    action: "update".to_string(),
                    diff_lines: lines,
                }]
            } else {
                ranges
                    .into_iter()
                    .map(|(start, _end)| ToolReviewSegment {
                        path: path.clone(),
                        action: "update".to_string(),
                        diff_lines: tool_review_segment_update_lines(start, old_string, new_string),
                    })
                    .collect()
            }
        }
        "delete" => {
            vec![ToolReviewSegment {
                path,
                action: "delete".to_string(),
                diff_lines: Vec::new(),
            }]
        }
        "move" => {
            vec![ToolReviewSegment {
                path,
                action: "move".to_string(),
                diff_lines: Vec::new(),
            }]
        }
        _ => Vec::new(),
    }
}

fn tool_review_segments_for_item(item: &ToolReviewCollectedItem) -> Vec<ToolReviewSegment> {
    match item.tool_name.as_str() {
        "apply_patch" => tool_review_segments_for_apply_patch(item),
        "write" | "update" | "delete" | "move" => tool_review_segments_for_single_edit(item),
        _ => Vec::new(),
    }
}

fn collect_tool_review_batches_internal(conversation: &Conversation) -> Vec<ToolReviewCollectedBatch> {
    let mut current_batch_key = None::<String>;
    let mut current_user_message_id = None::<String>;
    let mut order_index = 0usize;
    let mut batches = Vec::<ToolReviewCollectedBatch>::new();
    let mut batch_index_by_key = std::collections::HashMap::<String, usize>::new();
    let mut pending_calls = std::collections::HashMap::<String, (usize, usize)>::new();

    for message in &conversation.messages {
        if message.role.trim().eq_ignore_ascii_case("user") {
            let batch_key = message.id.trim().to_string();
            current_user_message_id = Some(batch_key.clone());
            current_batch_key = Some(batch_key.clone());
            if !batch_index_by_key.contains_key(&batch_key) {
                let next_index = batches.len();
                batch_index_by_key.insert(batch_key.clone(), next_index);
                batches.push(ToolReviewCollectedBatch {
                    batch_key: batch_key.clone(),
                    user_message_id: message.id.clone(),
                    user_message_text: tool_review_user_message_text(message),
                    items: Vec::new(),
                });
            }
            continue;
        }

        let Some(batch_key) = current_batch_key.clone() else {
            continue;
        };
        let Some(_) = current_user_message_id.clone() else {
            continue;
        };
        let Some(batch_idx) = batch_index_by_key.get(&batch_key).copied() else {
            continue;
        };
        for event in normalize_message_tool_history_events(message, MessageToolHistoryView::Display) {
            if event.role == "assistant" {
                for call in event.tool_calls {
                    let tool_name = call.tool_name.unwrap_or_default();
                    if tool_name != "shell_exec"
                        && tool_name != "apply_patch"
                        && tool_name != "write"
                        && tool_name != "delete"
                        && tool_name != "update"
                        && tool_name != "move"
                    {
                        continue;
                    }
                    let call_id = call
                        .invocation_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or_default()
                        .to_string();
                    if call_id.is_empty() {
                        continue;
                    }
                    order_index += 1;
                    let item_idx = batches[batch_idx].items.len();
                    batches[batch_idx].items.push(ToolReviewCollectedItem {
                        batch_key: batch_key.clone(),
                        call_id: call_id.clone(),
                        message_id: String::new(),
                        finished_at: None,
                        tool_name,
                        order_index,
                        args_value: call.arguments_value,
                        args_text: call.arguments_text,
                        result_text: String::new(),
                        result_value: None,
                        review_value: None,
                    });
                    pending_calls.insert(call_id, (batch_idx, item_idx));
                }
                continue;
            }
            if event.role != "tool" {
                continue;
            }
            let Some(call_id) = event
                .tool_call_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            let Some((pending_batch_idx, pending_item_idx)) = pending_calls.get(&call_id).copied() else {
                continue;
            };
            let item = &mut batches[pending_batch_idx].items[pending_item_idx];
            item.message_id = message.id.clone();
            item.finished_at = Some(message.created_at.clone());
            item.result_text = event.text.trim().to_string();
            item.result_value = serde_json::from_str::<Value>(event.text.trim()).ok();
            item.review_value = tool_review_normalize_review_value(
                item.result_value
                    .as_ref()
                    .and_then(|value| value.get("toolReview"))
                    .cloned(),
            );
        }
    }

    batches
        .into_iter()
        .filter(|batch| !batch.items.is_empty())
        .collect()
}

fn tool_review_find_batch_by_index(
    conversation: &Conversation,
    batch_index: usize,
) -> Result<(usize, ToolReviewCollectedBatch), String> {
    let batches = collect_tool_review_batches_internal(conversation);
    let total = batches.len();
    if total == 0 {
        return Err("当前会话没有可审查的工具批次。".to_string());
    }
    if batch_index >= total {
        return Err(format!("批次索引超出范围：batch_index={} total={}", batch_index, total));
    }
    let display_number = total - batch_index;
    let batch = batches
        .get(batch_index)
        .cloned()
        .ok_or_else(|| format!("未找到批次：batch_index={}", batch_index))?;
    Ok((display_number, batch))
}

fn tool_review_batch_summary_from_collected(batch: &ToolReviewCollectedBatch) -> ToolReviewBatchSummary {
    let mut added_lines_total = 0usize;
    let mut deleted_lines_total = 0usize;
    let mut changed_paths = std::collections::BTreeSet::<String>::new();
    let items = batch
        .items
        .iter()
        .map(|item| {
            let is_success = tool_review_is_item_successful(item);
            let is_denied = tool_review_is_item_denied(item);
            let affected_paths = if matches!(item.tool_name.as_str(), "apply_patch" | "write" | "delete" | "update" | "move") {
                tool_review_patch_paths_for_item(item)
            } else {
                Vec::new()
            };
            let (added_lines, deleted_lines) = tool_review_diff_stats_for_item(item);
            if is_success {
                added_lines_total += added_lines;
                deleted_lines_total += deleted_lines;
                changed_paths.extend(affected_paths.iter().cloned());
            }
            ToolReviewItemSummary {
                call_id: item.call_id.clone(),
                tool_name: item.tool_name.clone(),
                order_index: item.order_index,
                has_review: item.review_value.is_some(),
                review_opinion: item
                    .review_value
                    .as_ref()
                    .and_then(tool_review_value_to_stored_review)
                    .map(|review| review.review_opinion)
                    .filter(|value| !value.trim().is_empty()),
                affected_paths,
                patch_operation: if matches!(item.tool_name.as_str(), "apply_patch" | "write" | "delete" | "update" | "move") {
                    tool_review_patch_operation_for_item(item)
                } else {
                    None
                },
                command: if item.tool_name == "shell_exec" {
                    tool_review_command_for_item(item)
                } else {
                    None
                },
                finished_at: item.finished_at.clone(),
                added_lines,
                deleted_lines,
                is_success,
                is_denied,
                blocked_reason: item
                    .result_value
                    .as_ref()
                    .and_then(|value| value.get("blockedReason"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .or_else(|| {
                        if is_denied {
                            Some("user_denied".to_string())
                        } else {
                            None
                        }
                    }),
            }
        })
        .collect();
    ToolReviewBatchSummary {
        batch_key: batch.batch_key.clone(),
        user_message_id: batch.user_message_id.clone(),
        user_message_text: batch.user_message_text.clone(),
        item_count: batch.items.len(),
        unreviewed_count: batch
            .items
            .iter()
            .filter(|item| item.review_value.is_none())
            .count(),
        changed_files: changed_paths.len(),
        added_lines: added_lines_total,
        deleted_lines: deleted_lines_total,
        items,
    }
}

fn tool_review_diff_stats_for_item(item: &ToolReviewCollectedItem) -> (usize, usize) {
    if !tool_review_is_item_successful(item) {
        return (0, 0);
    }
    if !matches!(item.tool_name.as_str(), "apply_patch" | "write" | "delete" | "update" | "move") {
        return (0, 0);
    }
    let (_, preview_text) = tool_review_preview_for_item(item);
    tool_review_diff_line_counts_from_text(&preview_text)
}

fn tool_review_diff_line_counts_from_text(text: &str) -> (usize, usize) {
    let mut added = 0usize;
    let mut deleted = 0usize;
    for line in text.lines() {
        // 跳过补丁段头、git hunk 头与 unified diff 文件头，只统计正负内容行
        if line.starts_with("***") || line.starts_with("@@") || line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            deleted += 1;
        }
    }
    (added, deleted)
}

#[cfg(test)]
mod tool_review_diff_tests {
    use super::*;

    #[test]
    fn diff_line_counts_skip_headers_and_count_content() {
        let text = "*** Begin Patch\n*** Add File: src/a.rs\n+fn a() {}\n+fn b() {}\n*** Update File: src/b.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n context\n--- not a diff header content\n+++ also content\n*** End Patch";
        let (added, deleted) = tool_review_diff_line_counts_from_text(text);
        // "+++"/"---" 开头的内容行按 unified diff 规则视为文件头跳过；此处仅验证段头/hunk 头不计数
        assert_eq!((added, deleted), (3, 1), "added={added} deleted={deleted}");
    }

    #[test]
    fn diff_line_counts_ignore_non_patch_tools_text() {
        let item = ToolReviewCollectedItem {
            batch_key: "b".to_string(),
            call_id: "c".to_string(),
            message_id: "m".to_string(),
            finished_at: None,
            tool_name: "shell_exec".to_string(),
            order_index: 0,
            args_value: serde_json::json!({}),
            args_text: String::new(),
            result_text: String::new(),
            result_value: None,
            review_value: None,
        };
        assert_eq!(tool_review_diff_stats_for_item(&item), (0, 0));
    }
}

fn tool_review_item_detail_from_collected(item: &ToolReviewCollectedItem) -> ToolReviewItemDetail {
    let (preview_kind, preview_text) = tool_review_preview_for_item(item);
    ToolReviewItemDetail {
        batch_key: item.batch_key.clone(),
        call_id: item.call_id.clone(),
        message_id: item.message_id.clone(),
        tool_name: item.tool_name.clone(),
        order_index: item.order_index,
        has_review: item.review_value.is_some(),
        preview_kind,
        preview_text,
        result_text: item.result_text.clone(),
        review: item
            .review_value
            .as_ref()
            .and_then(tool_review_value_to_stored_review),
    }
}

fn tool_review_find_item(conversation: &Conversation, call_id: &str) -> Result<ToolReviewCollectedItem, String> {
    collect_tool_review_batches_internal(conversation)
        .into_iter()
        .flat_map(|batch| batch.items.into_iter())
        .find(|item| item.call_id == call_id)
        .ok_or_else(|| format!("Tool review item not found: {call_id}"))
}

fn tool_review_updated_result_content(result_text: &str, review: &Value) -> String {
    let mut object = match serde_json::from_str::<Value>(result_text.trim()) {
        Ok(Value::Object(map)) => map,
        Ok(other) => {
            let mut map = serde_json::Map::new();
            map.insert("rawResult".to_string(), other);
            map
        }
        Err(_) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "rawResult".to_string(),
                Value::String(result_text.trim().to_string()),
            );
            map
        }
    };
    object.insert("toolReview".to_string(), review.clone());
    serde_json::to_string_pretty(&Value::Object(object))
        .unwrap_or_else(|_| serde_json::json!({ "toolReview": review }).to_string())
}

fn tool_review_write_call_review(
    conversation: &mut Conversation,
    call_id: &str,
    review: &Value,
) -> Result<(), String> {
    for message in conversation.messages.iter_mut() {
        let Some(events) = message.tool_call.as_mut() else {
            continue;
        };
        for event in events.iter_mut() {
            let Some(object) = event.as_object_mut() else {
                continue;
            };
            let tool_call_id = object
                .get("tool_call_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            if tool_call_id != call_id {
                continue;
            }
            let content = object
                .get("content")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            object.insert(
                "content".to_string(),
                Value::String(tool_review_updated_result_content(content, review)),
            );
            return Ok(());
        }
    }
    Err(format!("Tool result event not found for call_id={call_id}"))
}

fn tool_review_build_context(item: &ToolReviewCollectedItem) -> Value {
    match item.tool_name.as_str() {
        "apply_patch" | "write" | "delete" | "update" | "move" => {
            let (_, preview_text) = tool_review_preview_for_item(item);
            serde_json::json!({
                "patch_preview": preview_text,
                "operations": tool_review_patch_operations_summary(&item.args_value).unwrap_or_default(),
                "result": item.result_value.clone().unwrap_or_else(|| Value::String(item.result_text.clone())),
            })
        }
        _ => serde_json::json!({
            "command": item
                .result_value
                .as_ref()
                .and_then(|value| tool_review_json_string_field(value, "command"))
                .or_else(|| tool_review_json_string_field(&item.args_value, "command"))
                .unwrap_or(item.args_text.trim()),
            "cwd": item
                .result_value
                .as_ref()
                .and_then(|value| tool_review_json_string_field(value, "cwd"))
                .unwrap_or(""),
            "result": item.result_value.clone().unwrap_or_else(|| Value::String(item.result_text.clone())),
        }),
    }
}

async fn tool_review_run_for_call_internal(
    state: &AppState,
    conversation_id: &str,
    call_id: &str,
) -> Result<ToolReviewItemDetail, String> {
    let review_api_config_id = current_tool_review_api_config_id(state)?
        .ok_or_else(|| "未配置工具评估模型。".to_string())?;
    let conversation =
        with_tool_review_conversation(state, conversation_id, |conversation| Ok(conversation.clone()))?;

    let item = tool_review_find_item(&conversation, call_id)?;
    let context = tool_review_build_context(&item);
    let timeout_opinion = "快速模型请求超时，劳烦亲自审查";
    let review_value = match run_tool_smart_review(
        state,
        &review_api_config_id,
        &item.tool_name,
        "Tool safety review",
        context,
        Some(FastRequestRecordTarget {
            conversation_id: conversation_id.trim().to_string(),
            kind: "tool_review",
        }),
    )
    .await
    {
        Ok(result) => match result {
            TerminalSmartReviewOutcome::Decision(review) => serde_json::json!({
                "kind": "decision",
                "allow": review.allow,
                "reviewOpinion": review.review_opinion,
                "modelName": review.model_name,
            }),
            TerminalSmartReviewOutcome::RawJson { raw_json, model_name } => serde_json::json!({
                "kind": "raw_json",
                "allow": false,
                "reviewOpinion": "当前工具评估模型返回了不符合约定的结果，请直接查看原始返回内容。",
                "modelName": model_name,
                "rawContent": raw_json,
            }),
        },
        Err(error) => {
            let is_timeout = error.contains("timed out") || error.contains("timeout");
            serde_json::json!({
                "kind": if is_timeout { "timeout" } else { "error" },
                "allow": false,
                "reviewOpinion": if is_timeout { timeout_opinion.to_string() } else { error.clone() },
                "error": error,
            })
        }
    };

    let call_id_for_mutation = call_id.to_string();
    conversation_service_v2()
        .update_unarchived_conversation_by_id(
            state,
            conversation_id,
            move |conversation| {
                tool_review_write_call_review(
                    conversation,
                    &call_id_for_mutation,
                    &review_value,
                )?;
                let refreshed = tool_review_find_item(conversation, &call_id_for_mutation)?;
                Ok(tool_review_item_detail_from_collected(&refreshed))
            },
        )
        .await
}

fn tool_review_parse_scope(raw: &str) -> Result<&'static str, String> {
    match raw.trim() {
        "uncommitted" => Ok("uncommitted"),
        "main" => Ok("main"),
        "commit" => Ok("commit"),
        "custom" => Ok("custom"),
        other => Err(format!("不支持的代码审查范围：{other}")),
    }
}

fn tool_review_find_skill_by_name(
    state: &AppState,
    skill_name: &str,
) -> Result<SkillSummaryItem, String> {
    let (skills, _errors) = load_workspace_skill_summaries_with_errors(state)?;
    skills
        .into_iter()
        .find(|item| item.name.trim() == skill_name)
        .ok_or_else(|| format!("未找到 skill：{skill_name}"))
}

async fn tool_review_exec_git_readonly(
    state: &AppState,
    conversation_id: &str,
    cwd: &Path,
    command: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    let session_id = format!("tool-review-code::{}", conversation_id.trim());
    // 前置解析合成会话的根目录：确认该会话能解析出有效主工作区，且 cwd 位于其中。
    // 合成会话解析失败时底层会回退默认根，可能误拦截或误放行；这里显式校验并给出可诊断错误。
    // root 解析、cwd 归一化都是同步 canonicalize，统一放 spawn_blocking 避免阻塞 async 线程；
    // 归一化后的 canonical 路径传给执行层，校验与执行之间不再存在 symlink 替换窗口。
    let state_for_blocking = state.clone();
    let session_id_for_blocking = session_id.clone();
    let cwd_for_blocking = cwd.to_path_buf();
    let normalized_cwd = tokio::task::spawn_blocking(move || {
        let root = terminal_session_root_canonical(&state_for_blocking, &session_id_for_blocking)
            .map_err(|err| format!("解析 tool-review 会话根目录失败：{err}"))?;
        let normalized = normalize_target_for_access_check(&cwd_for_blocking);
        if !exec_path_is_within(&root, &normalized) {
            return Err(format!(
                "tool-review 工作区不在会话根目录内：cwd={} root={}",
                cwd_for_blocking.to_string_lossy(),
                root.to_string_lossy()
            ));
        }
        Ok(normalized)
    })
    .await
    .map_err(|err| format!("tool-review 工作区校验任务执行失败：{err}"))??;
    let execution =
        run_command_in_workspace(state, &session_id, command, &normalized_cwd, timeout_ms, false)
            .await?;
    let stdout = terminal_decode_output_bytes(&execution.stdout);
    let stderr = terminal_decode_output_bytes(&execution.stderr);
    if !execution.ok {
        let detail = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
        return Err(format!("git 命令失败：{}", detail));
    }
    Ok(stdout)
}

fn tool_review_reports_root(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path).join("tool-review-reports")
}

fn tool_review_validate_conversation_id(conversation_id: &str) -> Result<String, String> {
    let normalized = conversation_id.trim();
    if normalized.is_empty() {
        return Err("会话 ID 为空，无法定位结果记录存储。".to_string());
    }
    if normalized.contains('/') || normalized.contains('\\') || normalized.contains("..") {
        return Err(format!("非法会话 ID：{}", normalized));
    }
    Ok(normalized.to_string())
}

fn tool_review_reports_file_path(data_path: &PathBuf, conversation_id: &str) -> Result<PathBuf, String> {
    let normalized = tool_review_validate_conversation_id(conversation_id)?;
    Ok(tool_review_reports_root(data_path)
        .join(normalized)
        .join("reports.jsonl"))
}

fn tool_review_write_text_atomic(path: &PathBuf, body: &str, label: &str) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid {label} file path"))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, body.as_bytes()).map_err(|err| format!("Write temp {label} failed: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp, path) {
        fs::copy(&tmp, path).map_err(|copy_err| {
            format!("Finalize {label} failed (rename: {rename_err}; copy: {copy_err})")
        })?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

fn emit_tool_review_reports_updated(state: &AppState, conversation_id: &str, report_id: &str, status: &str) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "reportId": report_id,
        "status": status,
    });
    runtime_log_info(format!(
        "[工具审查][事件] 推送 reports-updated conversation_id={} report_id={} status={}",
        conversation_id, report_id, status
    ));
    let _ = app_handle.emit("easy-call:tool-review-reports-updated", payload);
}

fn tool_review_read_report_records(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Vec<ToolReviewReportRecord>, String> {
    let path = tool_review_reports_file_path(data_path, conversation_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("读取结果记录文件失败，path={}，error={err}", path.display()))?;
    let mut out = Vec::<ToolReviewReportRecord>::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<ToolReviewReportRecord>(trimmed).map_err(|err| {
            format!(
                "解析结果记录失败，path={}，line={}，error={err}",
                path.display(),
                index + 1
            )
        })?;
        out.push(record);
    }
    Ok(out)
}

fn tool_review_write_report_records(
    data_path: &PathBuf,
    conversation_id: &str,
    records: &[ToolReviewReportRecord],
) -> Result<(), String> {
    let path = tool_review_reports_file_path(data_path, conversation_id)?;
    let mut body = String::new();
    for record in records {
        body.push_str(
            &serde_json::to_string(record)
                .map_err(|err| format!("序列化结果记录失败：{err}"))?,
        );
        body.push('\n');
    }
    tool_review_write_text_atomic(&path, &body, "tool review reports jsonl")
}

fn tool_review_list_reports_newest_first(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<Vec<ToolReviewReportRecord>, String> {
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    records.reverse();
    Ok(records)
}

fn tool_review_is_legacy_batch_scope(scope: &str) -> bool {
    scope.trim().eq_ignore_ascii_case("batch")
}

fn tool_review_prune_legacy_batch_report_records(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let path = tool_review_reports_file_path(data_path, conversation_id)?;
    if !path.exists() {
        return Ok(false);
    }
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    let before_len = records.len();
    records.retain(|item| !tool_review_is_legacy_batch_scope(&item.scope));
    if records.len() == before_len {
        return Ok(false);
    }
    if records.is_empty() {
        fs::remove_file(&path)
            .map_err(|err| format!("删除旧结果记录文件失败，path={}，error={err}", path.display()))?;
        return Ok(true);
    }
    tool_review_write_report_records(data_path, conversation_id, &records)?;
    Ok(true)
}

fn tool_review_create_pending_report(
    data_path: &PathBuf,
    conversation_id: &str,
    scope: &str,
    target: &str,
    agent_id: Option<&str>,
    workspace_path: &str,
) -> Result<ToolReviewReportRecord, String> {
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    let now = now_iso();
    let record = ToolReviewReportRecord {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.trim().to_string(),
        title: String::new(),
        status: "pending".to_string(),
        scope: scope.trim().to_string(),
        target: target.trim().to_string(),
        agent_id: agent_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        workspace_path: workspace_path.trim().to_string(),
        created_at: now.clone(),
        updated_at: now,
        report_text: String::new(),
        error_text: None,
        delegate_id: None,
    };
    records.push(record.clone());
    tool_review_write_report_records(data_path, conversation_id, &records)?;
    Ok(record)
}

fn tool_review_update_report_record(
    data_path: &PathBuf,
    conversation_id: &str,
    report_id: &str,
    status: &str,
    title: Option<&str>,
    report_text: Option<&str>,
    error_text: Option<&str>,
    delegate_id: Option<&str>,
) -> Result<ToolReviewReportRecord, String> {
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    let target_id = report_id.trim();
    let position = records
        .iter()
        .position(|item| item.id.trim() == target_id)
        .ok_or_else(|| format!("未找到结果记录：{}", target_id))?;
    let updated_at = now_iso();
    {
        let item = &mut records[position];
        item.status = status.trim().to_string();
        item.updated_at = updated_at;
        if let Some(value) = title.map(str::trim).filter(|value| !value.is_empty()) {
            item.title = value.chars().take(20).collect::<String>();
        }
        if let Some(text) = report_text {
            item.report_text = text.trim().to_string();
        }
        item.error_text = error_text
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if let Some(did) = delegate_id.map(str::trim).filter(|v| !v.is_empty()) {
            item.delegate_id = Some(did.to_string());
        }
    }
    let updated = records[position].clone();
    tool_review_write_report_records(data_path, conversation_id, &records)?;
    Ok(updated)
}

fn tool_review_delete_report_record(
    data_path: &PathBuf,
    conversation_id: &str,
    report_id: &str,
) -> Result<(), String> {
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    let target_id = report_id.trim();
    let before_len = records.len();
    records.retain(|item| item.id.trim() != target_id);
    if records.len() == before_len {
        return Err(format!("未找到结果记录：{}", target_id));
    }
    tool_review_write_report_records(data_path, conversation_id, &records)
}

fn tool_review_scope_instruction(scope: &str) -> &'static str {
    match scope {
        "uncommitted" => "请审查当前工作区未提交改动。",
        "main" => "请审查当前工作区相对主分支的改动。",
        "commit" => "请审查指定 commit 的改动。",
        "custom" => "请审查指定自定义范围的改动。",
        _ => "请审查当前代码改动。",
    }
}

fn tool_review_builtin_json_protocol() -> &'static str {
    r#"内置审查 Markdown 输出协议：
你必须只返回 Markdown 正文，不要输出 JSON，不要包 markdown 代码块，不要解释协议本身。

请严格使用以下结构：

# 审查结论

- 标题：10 到 20 个中文字符，概括本次审查对象
- 整体判定：patch is correct 或 patch is incorrect
- 整体置信度：0 到 1 之间的小数

## 发现的问题

没有确认到真实缺陷时，只输出下面这一行：

- 无

确认到真实缺陷时，按下面格式逐条列出，不要合并不同问题：

### 1. 一句话标题
- 优先级：1 / 2 / 3
- 置信度：0 到 1 之间的小数
- 位置：E:/project/src/foo.ts:10
- 说明：说明问题成因、触发条件、影响，引用文件和行号

### 2. 第二个独立缺陷标题
- 优先级：2
- 置信度：0.90
- 位置：E:/project/src/bar.ts:30
- 说明：如果发现多个互不依赖的真实缺陷，继续追加；不要因为示例数量而合并或截断

## 判定说明

用 1 到 3 句说明整体判断。

规则：
- 只有确认真实缺陷时才允许列入“发现的问题”；证据不足、无法判断、只是建议或担忧时，不得当作缺陷输出。
- 没有确认到真实缺陷时，“发现的问题”必须写 `- 无`。
- 发现多个独立真实缺陷时必须逐条列出，不要只输出第一条，也不要把不同问题合并成一条。
- “整体判定”只能是 `patch is correct` 或 `patch is incorrect`。
- “位置”必须落在当前 diff 范围内。
- 除以上结构外不要输出多余章节。"#
}

fn tool_review_render_delegate_instruction(
    scope: &str,
    target: Option<&str>,
    workspace_path: &str,
    skill: &SkillSummaryItem,
) -> String {
    let target_text = target
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\n\n范围参数：{}", value))
        .unwrap_or_default();
    format!(
        "{}\n\n当前工作区：{}{}\n\n请严格遵守以下 code-review skill 内容：\n\n{}\n\n{}",
        tool_review_scope_instruction(scope),
        workspace_path.trim(),
        target_text,
        skill.content.trim(),
        tool_review_builtin_json_protocol(),
    )
}

fn tool_review_extract_json_object(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let starts = trimmed
        .char_indices()
        .filter_map(|(index, ch)| (ch == '{').then_some(index))
        .collect::<Vec<_>>();
    for start in starts.into_iter().rev() {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (offset, ch) in trimmed[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        let end = start + offset + ch.len_utf8();
                        return trimmed.get(start..end);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn tool_review_title_from_json_value(value: &Value) -> String {
    value
        .get("review_title")
        .or_else(|| value.get("reviewTitle"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(20).collect::<String>())
        .unwrap_or_default()
}

fn tool_review_title_from_json_text(raw: &str) -> String {
    tool_review_extract_json_object(raw)
        .and_then(|json_text| serde_json::from_str::<Value>(json_text).ok())
        .map(|value| tool_review_title_from_json_value(&value))
        .unwrap_or_default()
}

fn tool_review_title_from_markdown_text(raw: &str) -> String {
    raw.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("- 标题：")
                .or_else(|| trimmed.strip_prefix("标题："))
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| text.chars().take(20).collect::<String>())
        })
        .unwrap_or_default()
}

fn tool_review_title_from_report_text(raw: &str) -> String {
    let json_title = tool_review_title_from_json_text(raw);
    if !json_title.trim().is_empty() {
        return json_title;
    }
    tool_review_title_from_markdown_text(raw)
}

fn with_tool_review_conversation<T>(
    state: &AppState,
    conversation_id: &str,
    reader: impl FnOnce(&Conversation) -> Result<T, String>,
) -> Result<T, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId 不能为空。".to_string());
    }
    conversation_service_v2().with_unarchived_conversation_by_id_fast(
        state,
        normalized_conversation_id,
        reader,
    )
}

#[tauri::command]
async fn list_tool_review_reports(
    input: ToolReviewConversationInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewReportsOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        list_tool_review_reports_internal(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取工具评审报告列表任务异常：{err}"))?
}

fn list_tool_review_reports_internal(
    input: ToolReviewConversationInput,
    state: &AppState,
) -> Result<ListToolReviewReportsOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(ListToolReviewReportsOutput { reports: Vec::new() });
    }
    let _ = tool_review_prune_legacy_batch_report_records(&state.data_path, conversation_id)?;
    Ok(ListToolReviewReportsOutput {
        reports: tool_review_list_reports_newest_first(&state.data_path, conversation_id)?,
    })
}

#[tauri::command]
async fn list_tool_review_batches(
    input: ToolReviewConversationInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewBatchesOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conversation_id = input.conversation_id.trim();
        if conversation_id.is_empty() {
            return Ok(ListToolReviewBatchesOutput {
                batches: Vec::new(),
                current_batch_key: None,
            });
        }
        let (batches, current_batch_key) =
            with_tool_review_conversation(&app_state, conversation_id, |conversation| {
                let batches = collect_tool_review_batches_internal(conversation);
                let current_batch_key = conversation
                    .messages
                    .iter()
                    .rev()
                    .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
                    .map(|message| message.id.clone());
                Ok((batches, current_batch_key))
            })?;
        Ok(ListToolReviewBatchesOutput {
            current_batch_key,
            batches: batches
                .iter()
                .map(tool_review_batch_summary_from_collected)
                .collect(),
        })
    })
    .await
    .map_err(|err| format!("读取工具评审批次列表任务异常：{err}"))?
}

#[tauri::command]
async fn get_tool_review_item_detail(
    input: ToolReviewCallInput,
    state: State<'_, AppState>,
) -> Result<ToolReviewItemDetail, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conversation_id = input.conversation_id.trim();
        let call_id = input.call_id.trim();
        if conversation_id.is_empty() || call_id.is_empty() {
            return Err("conversationId 和 callId 不能为空。".to_string());
        }
        with_tool_review_conversation(&app_state, conversation_id, |conversation| {
            let item = tool_review_find_item(conversation, call_id)?;
            Ok(tool_review_item_detail_from_collected(&item))
        })
    })
    .await
    .map_err(|err| format!("读取工具评审条目详情任务异常：{err}"))?
}

#[tauri::command]
async fn get_tool_review_batch_details(
    input: ToolReviewBatchActionInput,
    state: State<'_, AppState>,
) -> Result<ToolReviewBatchDetailsOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conversation_id = input.conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId 不能为空。".to_string());
        }
        with_tool_review_conversation(&app_state, conversation_id, |conversation| {
            let (_display_number, batch) = tool_review_find_batch_by_index(conversation, input.batch_index)?;
            let mut segments = Vec::<ToolReviewSegment>::new();
            for item in batch.items.iter() {
                if !tool_review_is_item_successful(item) {
                    continue;
                }
                if matches!(
                    item.tool_name.as_str(),
                    "apply_patch" | "write" | "delete" | "update" | "move"
                ) {
                    segments.extend(tool_review_segments_for_item(item));
                }
            }
            Ok(ToolReviewBatchDetailsOutput {
                batch_key: batch.batch_key,
                segments,
            })
        })
    })
    .await
    .map_err(|err| format!("读取工具评审批次详情任务异常：{err}"))?
}

#[tauri::command]
async fn run_tool_review_for_call(
    input: ToolReviewCallInput,
    state: State<'_, AppState>,
) -> Result<ToolReviewItemDetail, String> {
    let conversation_id = input.conversation_id.trim();
    let call_id = input.call_id.trim();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    tool_review_run_for_call_internal(state.inner(), conversation_id, call_id).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewSetUserDecisionInput {
    conversation_id: String,
    call_id: String,
    allow: bool,
    #[serde(default)]
    opinion: String,
}

#[tauri::command]
async fn set_tool_review_item_user_decision(
    input: ToolReviewSetUserDecisionInput,
    state: State<'_, AppState>,
) -> Result<ToolReviewItemDetail, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    let call_id = input.call_id.trim().to_string();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    let opinion = input.opinion.trim().to_string();
    let user_decision_review = serde_json::json!({
        "kind": "user_decision",
        "allow": input.allow,
        "reviewOpinion": if opinion.is_empty() {
            if input.allow { "用户已批准本次工具执行" } else { "用户已否决本次工具执行" }
        } else {
            opinion.as_str()
        },
        "userOpinion": opinion,
    });
    conversation_service_v2()
        .update_unarchived_conversation_by_id(
            state.inner(),
            &conversation_id,
            move |conversation| {
                tool_review_write_call_review(conversation, &call_id, &user_decision_review)?;
                let refreshed = tool_review_find_item(conversation, &call_id)?;
                Ok(tool_review_item_detail_from_collected(&refreshed))
            },
        )
        .await
}

async fn tool_review_run_missing_reviews_for_batch(
    state: &AppState,
    conversation_id: &str,
    batch: &ToolReviewCollectedBatch,
) -> Result<Vec<String>, String> {
    let mut reviewed_call_ids = Vec::<String>::new();
    for item in batch.items.iter().filter(|item| item.review_value.is_none()) {
        tool_review_run_for_call_internal(state, conversation_id, &item.call_id).await?;
        reviewed_call_ids.push(item.call_id.clone());
    }
    Ok(reviewed_call_ids)
}

#[tauri::command]
async fn run_tool_review_for_batch(
    input: ToolReviewBatchActionInput,
    state: State<'_, AppState>,
) -> Result<RunToolReviewBatchOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空。".to_string());
    }
    let conversation = with_tool_review_conversation(state.inner(), conversation_id, |conversation| {
        Ok(conversation.clone())
    })?;
    let (_batch_number, batch) = tool_review_find_batch_by_index(&conversation, input.batch_index)?;
    let reviewed_call_ids =
        tool_review_run_missing_reviews_for_batch(state.inner(), conversation_id, &batch).await?;
    Ok(RunToolReviewBatchOutput {
        batch_key: batch.batch_key,
        reviewed_call_ids,
    })
}

#[tauri::command]
async fn submit_tool_review_code(
    input: ToolReviewCodeReviewInput,
    state: State<'_, AppState>,
) -> Result<SubmitToolReviewCodeOutput, String> {
    submit_tool_review_code_internal(input, state.inner()).await
}

async fn submit_tool_review_code_internal(
    input: ToolReviewCodeReviewInput,
    state: &AppState,
) -> Result<SubmitToolReviewCodeOutput, String> {
    let conversation_id = input.conversation_id.trim();
    runtime_log_info(format!(
        "[工具审查][后端] 收到 submit_tool_review_code conversation_id={} scope={} target={}",
        conversation_id,
        input.scope.trim(),
        input.target.as_deref().unwrap_or("").trim()
    ));
    if conversation_id.is_empty() {
        runtime_log_error("[工具审查][后端] submit_tool_review_code 失败 conversationId 为空".to_string());
        return Err("conversationId 不能为空。".to_string());
    }
    let scope = tool_review_parse_scope(&input.scope).map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][后端] 解析审查范围失败 conversation_id={} raw_scope={} err={}",
            conversation_id,
            input.scope.trim(),
            err
        ));
        err
    })?;
    runtime_log_info(format!(
        "[工具审查][后端] 审查范围解析完成 conversation_id={} scope={}",
        conversation_id, scope
    ));
    let target = input
        .target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let app_state = state.clone();
    let conversation = with_tool_review_conversation(&app_state, conversation_id, |conversation| {
        Ok(conversation.clone())
    })
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][后端] 读取会话失败 conversation_id={} err={}",
            conversation_id, err
        ));
        err
    })?;
    runtime_log_info(format!(
        "[工具审查][后端] 会话读取完成 conversation_id={} message_count={}",
        conversation_id,
        conversation.messages.len()
    ));

    let workspace_path = terminal_default_workspace_for_conversation_resolved(
        &app_state,
        Some(&conversation),
    )
    .map(|workspace| workspace.path)
    .map_err(|err| {
        let detail = format!("当前会话缺少可用主工作区，无法发起代码审查：{}", err);
        runtime_log_error(format!(
            "[工具审查][后端] 解析工作区失败 conversation_id={} err={}",
            conversation_id, detail
        ));
        detail
    })?;
    let workspace_text = workspace_path.to_string_lossy().to_string();
    runtime_log_info(format!(
        "[工具审查][后端] 工作区解析完成 conversation_id={} workspace_path={}",
        conversation_id, workspace_text
    ));
    let target_text = target.unwrap_or_default().to_string();
    let source_agent_id = if conversation.agent_id.trim().is_empty() {
        DEFAULT_AGENT_ID.to_string()
    } else {
        conversation.agent_id.trim().to_string()
    };
    let requested_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let runtime_snapshot = load_runtime_organization_snapshot(&app_state)?;
    let target_agent_id = if let Some(agent_id) = requested_agent_id {
        runtime_snapshot
            .agents
            .iter()
            .find(|agent| agent.id == agent_id && !agent.is_built_in_user)
            .map(|agent| agent.id.clone())
            .ok_or_else(|| format!("代码审查目标人格不存在或不可用，agentId={agent_id}"))?
    } else {
        REVIEWER_AGENT_ID.to_string()
    };
    let pending_report = tool_review_create_pending_report(
        &app_state.data_path,
        conversation_id,
        scope,
        &target_text,
        Some(&target_agent_id),
        &workspace_text,
    )
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][后端] 创建代码审查记录失败 conversation_id={} scope={} target={} err={}",
            conversation_id, scope, target_text, err
        ));
        err
    })?;
    runtime_log_info(format!(
        "[工具审查][后端] 已创建代码审查记录 conversation_id={} scope={} report_id={} target={}",
        conversation_id, scope, pending_report.id, target_text
    ));
    emit_tool_review_reports_updated(&app_state, conversation_id, &pending_report.id, "pending");

    let conversation_id_owned = conversation_id.to_string();
    let report_id = pending_report.id.clone();
    let scope_owned = scope.to_string();
    let target_owned = if target_text.trim().is_empty() { None } else { Some(target_text.clone()) };
    let source_agent_id_owned = source_agent_id.clone();
    let target_agent_id_owned = target_agent_id.clone();
    tauri::async_runtime::spawn(async move {
        runtime_log_info(format!(
            "[工具审查][后端] 开始代码审查子任务 conversation_id={} scope={} report_id={} target={}",
            conversation_id_owned,
            scope_owned,
            report_id,
            target_owned.as_deref().unwrap_or("")
        ));
        let skill = match tool_review_find_skill_by_name(&app_state, "code-review") {
            Ok(skill) => skill,
            Err(err) => {
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                    None,
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 读取 code-review skill 失败 conversation_id={} scope={} report_id={} err={}",
                    conversation_id_owned, scope_owned, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let instruction = tool_review_render_delegate_instruction(
            &scope_owned,
            target_owned.as_deref(),
            &workspace_text,
            &skill,
        );
        let delegate_args = DelegateToolArgs {
            agent_id: target_agent_id_owned.clone(),
            mode: Some("wait".to_string()),
            why: Some(tool_review_delegate_background(&scope_owned, target_owned.as_deref())),
            goal: Some(instruction),
            todo: Some("输出符合协议的代码审查 JSON；仅返回纯 JSON，不要包 markdown。".to_string()),
            background: None,
            question: None,
            focus: None,
        };
        let session_id = format!("{}::{}", source_agent_id_owned, conversation_id_owned);
        runtime_log_info(format!(
            "[工具审查][后端] 发起代码审查委托 conversation_id={} scope={} report_id={} session_id={} source_agent_id={} target_agent_id={}",
            conversation_id_owned,
            scope_owned,
            report_id,
            session_id,
            source_agent_id_owned,
            target_agent_id_owned
        ));
        let delegate_result = match delegate_execute_sync(
            &app_state,
            &session_id,
            Some(source_agent_id_owned.as_str()),
            DELEGATE_TOOL_KIND_DELEGATE,
            delegate_args,
        )
        .await
        {
            Ok(result) => result,
            Err(err) => {
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                    None,
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 代码审查委托失败 conversation_id={} scope={} report_id={} err={}",
                    conversation_id_owned, scope_owned, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let result_delegate_id = delegate_result
            .get("delegate")
            .and_then(|d| d.get("delegateId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned);
        let ok = delegate_result.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if !ok {
            let reason = delegate_result
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("代码审查委托失败")
                .to_string();
            let _ = tool_review_update_report_record(
                &app_state.data_path,
                &conversation_id_owned,
                &report_id,
                "failed",
                None,
                None,
                Some(&reason),
                result_delegate_id.as_deref(),
            );
            runtime_log_error(format!(
                "[工具审查][后端] 代码审查委托返回失败 conversation_id={} scope={} report_id={} reason={}",
                conversation_id_owned, scope_owned, report_id, reason
            ));
            emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
            return;
        }
        let assistant_text = match delegate_result
            .get("assistantText")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(text) => text.to_string(),
            None => {
                let err = "下级人格未返回代码审查结果。".to_string();
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                    None,
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 代码审查结果缺失 conversation_id={} scope={} report_id={}",
                    conversation_id_owned, scope_owned, report_id
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let report_text = assistant_text.trim().to_string();
        let report_title = tool_review_title_from_report_text(&report_text);
        match tool_review_update_report_record(
            &app_state.data_path,
            &conversation_id_owned,
            &report_id,
            "success",
            Some(&report_title),
            Some(&report_text),
            None,
            result_delegate_id.as_deref(),
        ) {
            Ok(_) => {
                runtime_log_info(format!(
                    "[工具审查][后端] 代码审查完成 conversation_id={} scope={} report_id={}",
                    conversation_id_owned, scope_owned, report_id
                ));
                if let Err(err) = conversation_service_v2().enqueue_delegate_completion_notification(
                    &app_state,
                    &conversation_id_owned,
                    &target_agent_id_owned,
                    &report_title,
                    &report_text,
                    "tool_review_delegate_completion",
                ) {
                    runtime_log_error(format!(
                        "[工具审查][后端] 投递完成系统通知失败 conversation_id={} scope={} report_id={} err={}",
                        conversation_id_owned, scope_owned, report_id, err
                    ));
                }
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "success");
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[工具审查][后端] 代码审查结果落盘失败 conversation_id={} scope={} report_id={} err={}",
                    conversation_id_owned, scope_owned, report_id, err
                ));
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                    None,
                );
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
            }
        }
    });

    Ok(SubmitToolReviewCodeOutput { report: pending_report })
}

#[cfg(test)]
mod tool_review_tests {
    use super::{
        tool_review_build_context, tool_review_diff_stats_for_item, tool_review_is_item_denied,
        tool_review_is_item_successful, tool_review_patch_paths_for_item,
        tool_review_preview_for_item, tool_review_prune_legacy_batch_report_records,
        tool_review_segments_for_item, ToolReviewCollectedItem, ToolReviewReportRecord,
    };
    use crate::app_root_from_data_path;
    use std::{env, fs};
    use uuid::Uuid;

    #[test]
    fn tool_review_apply_patch_context_should_include_operation_content() {
        let args_value = serde_json::json!({
            "operations": [
                {
                    "action": "update",
                    "path": "E:/github/easy_call_ai/src/main.rs",
                    "old_string": "let value = ;",
                    "new_string": "let value = 1;",
                    "replace_all": false
                }
            ]
        });
        let item = ToolReviewCollectedItem {
            batch_key: "batch-1".to_string(),
            call_id: "call-1".to_string(),
            message_id: "message-1".to_string(),
            finished_at: None,
            tool_name: "apply_patch".to_string(),
            order_index: 0,
            args_text: args_value.to_string(),
            args_value,
            result_text: "{}".to_string(),
            result_value: Some(serde_json::json!({ "ok": true })),
            review_value: None,
        };

        let (_, preview_text) = tool_review_preview_for_item(&item);
        let context = tool_review_build_context(&item);

        assert!(preview_text.contains("-let value = ;"));
        assert!(preview_text.contains("+let value = 1;"));
        assert_eq!(context["operations"][0]["action"], "update");
        assert_eq!(context["operations"][0]["old_preview"], "let value = ;");
        assert_eq!(context["operations"][0]["new_preview"], "let value = 1;");
    }

    #[test]
    fn tool_review_apply_patch_preview_should_include_persisted_line_header() {
        let args_value = serde_json::json!({
            "operations": [
                {
                    "action": "update",
                    "path": "E:/github/easy_call_ai/src/main.rs",
                    "old_string": "let value = ;",
                    "new_string": "let value = 1;",
                    "replace_all": false
                }
            ]
        });
        let item = ToolReviewCollectedItem {
            batch_key: "batch-1".to_string(),
            call_id: "call-1".to_string(),
            message_id: "message-1".to_string(),
            finished_at: None,
            tool_name: "apply_patch".to_string(),
            order_index: 0,
            args_text: args_value.to_string(),
            args_value,
            result_text: "{}".to_string(),
            result_value: Some(serde_json::json!({
                "ok": true,
                "changed": [
                    {
                        "op": "update",
                        "path": "E:/github/easy_call_ai/src/main.rs",
                        "lineStart": 42,
                        "lineEnd": 42,
                        "lineRanges": [{ "start": 42, "end": 42 }]
                    }
                ]
            })),
            review_value: None,
        };

        let (_, preview_text) = tool_review_preview_for_item(&item);

        assert!(preview_text.contains("@@ line 42 @@"));
    }

    #[test]
    fn tool_review_write_preview_should_show_update_when_overwrite_true() {
        let item = ToolReviewCollectedItem {
            batch_key: "batch-1".to_string(),
            call_id: "call-1".to_string(),
            message_id: "message-1".to_string(),
            finished_at: None,
            tool_name: "write".to_string(),
            order_index: 0,
            args_text: serde_json::json!({
                "path": "E:/github/easy_call_ai/src/main.rs",
                "content": "fn main() {}\n",
                "overwrite": true
            })
            .to_string(),
            args_value: serde_json::json!({
                "path": "E:/github/easy_call_ai/src/main.rs",
                "content": "fn main() {}\n",
                "overwrite": true
            }),
            result_text: "{}".to_string(),
            result_value: Some(serde_json::json!({ "ok": true })),
            review_value: None,
        };

        let (_, preview_text) = tool_review_preview_for_item(&item);

        assert!(preview_text.contains("+fn main() {}"));
    }

    #[test]
    fn tool_review_report_record_should_tolerate_legacy_department_id() {
        let record = serde_json::from_str::<ToolReviewReportRecord>(
            r#"{
                "id":"report-1",
                "conversationId":"conversation-1",
                "title":"Report",
                "status":"success",
                "scope":"commit",
                "target":"HEAD",
                "departmentId":"legacy-department",
                "workspacePath":"E:/workspace",
                "createdAt":"2026-05-05T00:00:00.000Z",
                "updatedAt":"2026-05-05T00:00:00.000Z",
                "reportText":"ok"
            }"#,
        )
        .expect("legacy report record with departmentId should deserialize");

        assert_eq!(record.id, "report-1");
        assert_eq!(record.agent_id, None);
    }

    #[test]
    fn tool_review_prune_legacy_batch_report_records_should_remove_batch_scope_records() {
        let root = env::temp_dir().join(format!("easy-call-ai-tool-review-{}", Uuid::new_v4()));
        let data_path = root.join("config_mark");
        let conversation_id = "conversation-1";
        let reports_dir = app_root_from_data_path(&data_path)
            .join("tool-review-reports")
            .join(conversation_id);
        fs::create_dir_all(&reports_dir).expect("create reports dir");
        fs::write(
            reports_dir.join("reports.jsonl"),
            concat!(
                "{\"id\":\"r1\",\"conversationId\":\"conversation-1\",\"title\":\"\",\"status\":\"failed\",\"scope\":\"batch\",\"target\":\"第 1 批\",\"workspacePath\":\"E:/workspace\",\"createdAt\":\"2026-05-05T00:00:00.000Z\",\"updatedAt\":\"2026-05-05T00:00:00.000Z\",\"reportText\":\"old\"}\n",
                "{\"id\":\"r2\",\"conversationId\":\"conversation-1\",\"title\":\"\",\"status\":\"success\",\"scope\":\"commit\",\"target\":\"HEAD\",\"workspacePath\":\"E:/workspace\",\"createdAt\":\"2026-05-05T00:00:00.000Z\",\"updatedAt\":\"2026-05-05T00:00:00.000Z\",\"reportText\":\"new\"}\n"
            ),
        )
        .expect("write reports");

        let changed = tool_review_prune_legacy_batch_report_records(&data_path, conversation_id)
            .expect("prune legacy batch reports");
        let records = super::tool_review_read_report_records(&data_path, conversation_id)
            .expect("read cleaned reports");

        assert!(changed);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].scope, "commit");
        let _ = fs::remove_dir_all(root);
    }

    fn test_segment_item(
        tool_name: &str,
        args_value: serde_json::Value,
        result_value: Option<serde_json::Value>,
    ) -> ToolReviewCollectedItem {
        ToolReviewCollectedItem {
            batch_key: "batch-1".to_string(),
            call_id: "call-1".to_string(),
            message_id: "message-1".to_string(),
            finished_at: None,
            tool_name: tool_name.to_string(),
            order_index: 0,
            args_text: args_value.to_string(),
            args_value,
            result_text: result_value
                .as_ref()
                .map(serde_json::Value::to_string)
                .unwrap_or_default(),
            result_value,
            review_value: None,
        }
    }

    #[test]
    fn tool_review_segments_apply_patch_update_single_hunk() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [{
                    "action": "update",
                    "path": "src/main.rs",
                    "old_string": "let a = 1;\nlet b = 2;",
                    "new_string": "let a = 10;\nlet b = 20;",
                    "replace_all": false
                }]
            }),
            Some(serde_json::json!({
                "changed": [{
                    "op": "update",
                    "path": "src/main.rs",
                    "lineRanges": [{ "start": 5, "end": 6 }]
                }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].path, "src/main.rs");
        assert_eq!(segments[0].action, "update");
        assert_eq!(segments[0].diff_lines[0], "@@ -5,2 +5,2 @@");
        assert_eq!(segments[0].diff_lines[1], "-let a = 1;");
        assert_eq!(segments[0].diff_lines[2], "-let b = 2;");
        assert_eq!(segments[0].diff_lines[3], "+let a = 10;");
        assert_eq!(segments[0].diff_lines[4], "+let b = 20;");
    }

    #[test]
    fn tool_review_segments_apply_patch_replace_all_splits_each_match() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [{
                    "action": "update",
                    "path": "src/a.ts",
                    "old_string": "旧行",
                    "new_string": "新行",
                    "replace_all": true
                }]
            }),
            Some(serde_json::json!({
                "changed": [{
                    "op": "update",
                    "path": "src/a.ts",
                    "lineRanges": [
                        { "start": 10, "end": 10 },
                        { "start": 40, "end": 40 },
                        { "start": 70, "end": 70 }
                    ]
                }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].diff_lines[0], "@@ -10,1 +10,1 @@");
        assert_eq!(segments[1].diff_lines[0], "@@ -40,1 +40,1 @@");
        assert_eq!(segments[2].diff_lines[0], "@@ -70,1 +70,1 @@");
    }

    #[test]
    fn tool_review_segments_apply_patch_add_delete_move() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [
                    { "action": "add", "path": "src/new.ts", "content": "export const x = 1;\nexport const y = 2;" },
                    { "action": "delete", "path": "src/old.ts" },
                    { "action": "move", "path": "src/a.ts", "to": "src/b.ts" }
                ]
            }),
            Some(serde_json::json!({
                "changed": [
                    { "op": "add", "path": "src/new.ts" },
                    { "op": "delete", "path": "src/old.ts" },
                    { "op": "move", "from": "src/a.ts", "to": "src/b.ts" }
                ]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].action, "add");
        assert_eq!(segments[0].diff_lines[0], "@@ -0,0 +1,2 @@");
        assert_eq!(segments[0].diff_lines[1], "+export const x = 1;");
        assert_eq!(segments[0].diff_lines[2], "+export const y = 2;");
        assert_eq!(segments[1].action, "delete");
        assert!(segments[1].diff_lines.is_empty());
        assert_eq!(segments[2].action, "move");
        assert!(segments[2].diff_lines.is_empty());
    }

    #[test]
    fn tool_review_segments_update_tool_replace_all() {
        let item = test_segment_item(
            "update",
            serde_json::json!({
                "path": "src/lib.ts",
                "old_string": "name",
                "new_string": "title",
                "replace_all": true
            }),
            Some(serde_json::json!({
                "changed": [{
                    "op": "update",
                    "path": "src/lib.ts",
                    "lineRanges": [
                        { "start": 3, "end": 3 },
                        { "start": 8, "end": 8 }
                    ]
                }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].path, "src/lib.ts");
        assert_eq!(segments[0].diff_lines[0], "@@ -3,1 +3,1 @@");
        assert_eq!(segments[1].diff_lines[0], "@@ -8,1 +8,1 @@");
    }

    #[test]
    fn tool_review_segments_write_add_file() {
        let item = test_segment_item(
            "write",
            serde_json::json!({
                "path": "src/hello.ts",
                "content": "export function hello() {\n  return 1;\n}\n",
                "overwrite": false
            }),
            Some(serde_json::json!({
                "changed": [{ "op": "add", "path": "src/hello.ts" }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].action, "add");
        assert_eq!(segments[0].diff_lines[0], "@@ -0,0 +1,3 @@");
        assert_eq!(segments[0].diff_lines[1], "+export function hello() {");
        assert_eq!(segments[0].diff_lines[2], "+  return 1;");
        assert_eq!(segments[0].diff_lines[3], "+}");
    }

    #[test]
    fn tool_review_segments_write_overwrite_is_update_action() {
        let item = test_segment_item(
            "write",
            serde_json::json!({
                "path": "src/hello.ts",
                "content": "export const v = 2;\n",
                "overwrite": true
            }),
            Some(serde_json::json!({
                "changed": [{ "op": "update", "path": "src/hello.ts" }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].action, "update");
        assert_eq!(segments[0].diff_lines[0], "@@ -0,0 +1,1 @@");
        assert_eq!(segments[0].diff_lines[1], "+export const v = 2;");
    }

    #[test]
    fn tool_review_segments_missing_line_ranges_falls_back_to_line_1() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [{
                    "action": "update",
                    "path": "src/main.rs",
                    "old_string": "旧",
                    "new_string": "新",
                    "replace_all": false
                }]
            }),
            Some(serde_json::json!({
                "changed": [{ "op": "update", "path": "src/main.rs" }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].diff_lines[0], "@@ -1,1 +1,1 @@");
    }

    #[test]
    fn tool_review_segments_empty_old_new_strings_produce_header_only() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [{
                    "action": "update",
                    "path": "src/main.rs",
                    "old_string": "a\nb",
                    "new_string": "",
                    "replace_all": false
                }]
            }),
            Some(serde_json::json!({
                "changed": [{ "op": "update", "path": "src/main.rs", "lineRanges": [{ "start": 20, "end": 21 }] }]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].diff_lines[0], "@@ -20,2 +20,0 @@");
        assert_eq!(segments[0].diff_lines.len(), 3);
    }

    #[test]
    fn tool_review_segments_shell_tool_returns_empty() {
        let item = test_segment_item(
            "shell_exec",
            serde_json::json!({ "command": "ls" }),
            Some(serde_json::json!({ "command": "ls", "exitCode": 0 })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert!(segments.is_empty());
    }

    #[test]
    fn tool_review_segments_multi_operation_apply_patch_preserves_order() {
        let item = test_segment_item(
            "apply_patch",
            serde_json::json!({
                "operations": [
                    { "action": "update", "path": "src/a.ts", "old_string": "1", "new_string": "2", "replace_all": false },
                    { "action": "update", "path": "src/b.ts", "old_string": "3", "new_string": "4", "replace_all": true },
                    { "action": "add", "path": "src/c.ts", "content": "5" }
                ]
            }),
            Some(serde_json::json!({
                "changed": [
                    { "op": "update", "path": "src/a.ts", "lineRanges": [{ "start": 1, "end": 1 }] },
                    { "op": "update", "path": "src/b.ts", "lineRanges": [{ "start": 2, "end": 2 }, { "start": 9, "end": 9 }] },
                    { "op": "add", "path": "src/c.ts" }
                ]
            })),
        );

        let segments = tool_review_segments_for_item(&item);

        assert_eq!(segments.len(), 4);
        assert_eq!(segments[0].path, "src/a.ts");
        assert_eq!(segments[0].diff_lines[0], "@@ -1,1 +1,1 @@");
        assert_eq!(segments[1].path, "src/b.ts");
        assert_eq!(segments[1].diff_lines[0], "@@ -2,1 +2,1 @@");
        assert_eq!(segments[2].path, "src/b.ts");
        assert_eq!(segments[2].diff_lines[0], "@@ -9,1 +9,1 @@");
        assert_eq!(segments[3].path, "src/c.ts");
        assert_eq!(segments[3].action, "add");
    }

    #[test]
    fn tool_review_denied_tool_should_not_have_diff_stats_or_paths() {
        let item = test_segment_item(
            "write",
            serde_json::json!({
                "path": "cli2api/permission-test.md",
                "content": "# 权限测试\n此文件用于验证当前工作目录的写入权限。\n创建时间: 2026-09-09"
            }),
            Some(serde_json::json!({
                "ok": false,
                "approved": false,
                "blockedReason": "user_denied_apply_patch",
                "message": "用户拒绝了本次变更应用。"
            })),
        );

        assert!(!tool_review_is_item_successful(&item));
        assert!(tool_review_is_item_denied(&item));
        assert_eq!(tool_review_diff_stats_for_item(&item), (0, 0));
        assert_eq!(tool_review_patch_paths_for_item(&item), vec!["cli2api/permission-test.md"]);
    }

    #[test]
    fn tool_review_approved_false_without_blocked_reason_should_be_denied() {
        let item = test_segment_item(
            "write",
            serde_json::json!({
                "path": "cli2api/test.md",
                "content": "test"
            }),
            Some(serde_json::json!({
                "approved": false
            })),
        );

        assert!(tool_review_is_item_denied(&item));
        assert!(!tool_review_is_item_successful(&item));
    }

    #[test]
    fn tool_review_nonzero_exit_code_should_be_failed() {
        let item = test_segment_item(
            "exec",
            serde_json::json!({ "command": "false" }),
            Some(serde_json::json!({
                "command": "false",
                "exitCode": 1,
                "stdout": "",
                "stderr": "error"
            })),
        );

        assert!(!tool_review_is_item_successful(&item));
        assert!(!tool_review_is_item_denied(&item));
    }
}
