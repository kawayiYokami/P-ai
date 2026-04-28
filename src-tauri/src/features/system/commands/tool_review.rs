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
    let conversation_id = input.conversation_id.trim();
    let page = input.page.max(1);
    let page_size = input.page_size.clamp(1, 100);
    runtime_log_info(format!(
        "[工具审查][commit列表] 开始 conversation_id={} page={} page_size={}",
        conversation_id, page, page_size
    ));
    if conversation_id.is_empty() {
        runtime_log_info("[工具审查][commit列表] 跳过 conversation_id 为空".to_string());
        return Ok(ListToolReviewCommitOptionsOutput { total: 0, page, page_size, commits: Vec::new() });
    }
    let conversation = with_tool_review_conversation(state.inner(), conversation_id, |conversation| {
        Ok(conversation.clone())
    })
    .map_err(|err| {
        runtime_log_error(format!(
            "[工具审查][commit列表] 读取会话失败 conversation_id={} err={}",
            conversation_id, err
        ));
        err
    })?;
    let (total, commits) = tool_review_list_commit_options_internal(state.inner(), &conversation, page, page_size)
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
    department_id: Option<String>,
    #[serde(default)]
    api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteToolReviewReportInput {
    conversation_id: String,
    report_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitToolReviewBatchInput {
    conversation_id: String,
    batch_number: usize,
    #[serde(default)]
    department_id: Option<String>,
    #[serde(default)]
    api_config_id: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    affected_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    patch_operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
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
    workspace_path: String,
    created_at: String,
    updated_at: String,
    report_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewBatchSummary {
    batch_key: String,
    user_message_id: String,
    user_message_text: String,
    item_count: usize,
    unreviewed_count: usize,
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
struct RunToolReviewBatchOutput {
    batch_key: String,
    reviewed_call_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitToolReviewBatchOutput {
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
            MessagePart::Text { text } => Some(text.trim()),
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

#[tauri::command]
fn delete_tool_review_report(
    input: DeleteToolReviewReportInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conversation_id = input.conversation_id.trim();
    let report_id = input.report_id.trim();
    if conversation_id.is_empty() || report_id.is_empty() {
        return Err("conversationId 和 reportId 不能为空。".to_string());
    }
    with_tool_review_conversation(state.inner(), conversation_id, |_conversation| {
        tool_review_delete_report_record(&state.data_path, conversation_id, report_id)
    })?;
    emit_tool_review_reports_updated(state.inner(), conversation_id, report_id, "deleted");
    Ok(())
}

fn tool_review_preview_for_item(item: &ToolReviewCollectedItem) -> (String, String) {
    match item.tool_name.as_str() {
        "apply_patch" => {
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

fn tool_review_report_from_message(message: &ChatMessage) -> Option<ToolReviewReportRecord> {
    if !is_tool_review_report_message(message) {
        return None;
    }
    let meta = message.provider_meta.as_ref()?;
    let batch_key = meta
        .get("batchKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let report_text = meta
        .get("reportText")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| render_message_content_for_model(message).trim().to_string());
    Some(ToolReviewReportRecord {
        id: message.id.clone(),
        conversation_id: String::new(),
        title: String::new(),
        status: "success".to_string(),
        scope: "batch".to_string(),
        target: batch_key.clone(),
        workspace_path: String::new(),
        created_at: message.created_at.clone(),
        updated_at: message.created_at.clone(),
        report_text,
        error_text: None,
    })
}

fn collect_tool_review_batches_internal(conversation: &Conversation) -> Vec<ToolReviewCollectedBatch> {
    let mut current_batch_key = None::<String>;
    let mut current_user_message_id = None::<String>;
    let mut order_index = 0usize;
    let mut batches = Vec::<ToolReviewCollectedBatch>::new();
    let mut batch_index_by_key = std::collections::HashMap::<String, usize>::new();
    let mut pending_calls = std::collections::HashMap::<String, (usize, usize)>::new();

    for message in &conversation.messages {
        if is_tool_review_report_message(message) {
            continue;
        }
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
                    if tool_name != "shell_exec" && tool_name != "apply_patch" {
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
            item.result_text = event.text.trim().to_string();
            item.result_value = serde_json::from_str::<Value>(event.text.trim()).ok();
            item.review_value = item
                .result_value
                .as_ref()
                .and_then(|value| value.get("toolReview"))
                .cloned();
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
        items: batch
            .items
            .iter()
            .map(|item| ToolReviewItemSummary {
                call_id: item.call_id.clone(),
                tool_name: item.tool_name.clone(),
                order_index: item.order_index,
                has_review: item.review_value.is_some(),
                affected_paths: if item.tool_name == "apply_patch" {
                    tool_review_patch_paths_for_item(item)
                } else {
                    Vec::new()
                },
                patch_operation: if item.tool_name == "apply_patch" {
                    tool_review_patch_operation_for_item(item)
                } else {
                    None
                },
                command: if item.tool_name == "shell_exec" {
                    tool_review_command_for_item(item)
                } else {
                    None
                },
            })
            .collect(),
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
        "apply_patch" => {
            let (_, preview_text) = tool_review_preview_for_item(item);
            serde_json::json!({
                "patch_preview": preview_text,
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
        .ok_or_else(|| "未配置工具审查模型。".to_string())?;
    let conversation =
        with_tool_review_conversation(state, conversation_id, |conversation| Ok(conversation.clone()))?;

    let item = tool_review_find_item(&conversation, call_id)?;
    let context = tool_review_build_context(&item);
    let review_value = match run_tool_smart_review(
        state,
        &review_api_config_id,
        &item.tool_name,
        "Tool safety review",
        context,
    )
    .await?
    {
        TerminalSmartReviewOutcome::Decision(review) => serde_json::json!({
            "kind": "decision",
            "allow": review.allow,
            "reviewOpinion": review.review_opinion,
            "modelName": review.model_name,
        }),
        TerminalSmartReviewOutcome::RawJson { raw_json, model_name } => serde_json::json!({
            "kind": "raw_json",
            "allow": false,
            "reviewOpinion": "当前工具审查模型返回了不符合约定的结果，请直接查看原始返回内容。",
            "modelName": model_name,
            "rawContent": raw_json,
        }),
    };

    conversation_service().update_unarchived_conversation_by_id(
        state,
        conversation_id,
        |conversation| {
            tool_review_write_call_review(conversation, call_id, &review_value)?;
            let refreshed = tool_review_find_item(conversation, call_id)?;
            Ok(tool_review_item_detail_from_collected(&refreshed))
        },
    )
}

fn tool_review_last_related_message_index(
    conversation: &Conversation,
    batch: &ToolReviewCollectedBatch,
) -> Option<usize> {
    let user_message_id = batch.user_message_id.trim();
    let related_call_ids = batch
        .items
        .iter()
        .map(|item| item.call_id.trim())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut last_related_index = conversation
        .messages
        .iter()
        .position(|message| message.id.trim() == user_message_id);

    for (index, message) in conversation.messages.iter().enumerate() {
        if message.id.trim() == user_message_id {
            last_related_index = Some(index);
            continue;
        }

        let mut matched = false;
        for event in normalize_message_tool_history_events(message, MessageToolHistoryView::Display) {
            if event.role == "assistant" {
                matched = event.tool_calls.iter().any(|call| {
                    call.invocation_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_some_and(|call_id| related_call_ids.contains(call_id))
                });
            } else if event.role == "tool" {
                matched = event
                    .tool_call_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some_and(|call_id| related_call_ids.contains(call_id));
            }

            if matched {
                last_related_index = Some(index);
                break;
            }
        }
    }

    last_related_index
}

fn tool_review_collect_recent_context(
    conversation: &Conversation,
    batch: &ToolReviewCollectedBatch,
    max_chars: usize,
) -> String {
    let anchor_exclusive = tool_review_last_related_message_index(conversation, batch)
        .map(|index| index.saturating_add(1))
        .unwrap_or(conversation.messages.len());
    let mut blocks = Vec::<String>::new();
    let mut found_summary = false;

    for message in conversation.messages[..anchor_exclusive].iter().rev() {
        if is_tool_review_report_message(message) {
            continue;
        }
        let role = message.role.trim().to_ascii_lowercase();
        if !matches!(role.as_str(), "user" | "assistant") {
            continue;
        }
        let is_summary = role == "user" && is_context_compaction_message(message, "user");
        let text = if role == "user" {
            render_prompt_user_text_only(message)
        } else {
            render_message_content_for_model(message)
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let block = format!(
            "[{}] {}\n{}",
            if role == "user" { "用户" } else { "助手" },
            message.created_at.trim(),
            text
        );

        if is_summary {
            blocks.push(block);
            found_summary = true;
            break;
        }
        blocks.push(block);
    }

    if found_summary {
        blocks.reverse();
        return blocks.join("\n\n");
    }

    let joined = blocks.into_iter().rev().collect::<Vec<_>>().join("\n\n");
    let total_chars = joined.chars().count();
    if total_chars <= max_chars {
        return joined;
    }
    joined
        .chars()
        .rev()
        .take(max_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
}

fn tool_review_latest_unfinished_plan(conversation: &Conversation) -> String {
    for message in conversation.messages.iter().rev() {
        let meta = message.provider_meta.as_ref();
        let action = meta
            .and_then(|item| item.get("planCard"))
            .and_then(Value::as_object)
            .and_then(|item| item.get("action"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let kind = provider_meta_message_kind(message).unwrap_or_default();
        if kind == "plan_complete" || action.eq_ignore_ascii_case("complete") {
            return String::new();
        }
        let context = meta
            .and_then(|item| item.get("planCard"))
            .and_then(Value::as_object)
            .and_then(|item| item.get("context"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if (kind == "plan_present" || action.eq_ignore_ascii_case("present")) && !context.is_empty() {
            return context.to_string();
        }
    }
    String::new()
}

fn tool_review_report_system_prompt(ui_language: &str) -> String {
    let language = match ui_language.trim() {
        "en-US" => "English",
        "zh-TW" => "繁體中文",
        _ => "简体中文",
    };
    format!(
        "Please review the current batch of changes and write the final review report in {language}.\n\
Generate a professional, well-structured review report.\n\
This is not a chat reply. It is a formal review report. Do not ask for the user's opinion.\n\
\n\
The report must include these sections:\n\
1. Overall change summary\n\
2. Quality score (0-100)\n\
3. Deviation from the plan\n\
4. Issues found\n\
5. Possible optimization suggestions\n\
\n\
Check for:\n\
1. Bugs: Logic errors, off-by-one, null handling, race conditions\n\
2. Security: Injection risks, auth issues, data exposure\n\
3. Performance: N+1 queries, unnecessary loops, memory leaks\n\
4. Maintainability: Naming, complexity, duplication\n\
5. Edge cases: What inputs would break this?\n\
\n\
For each issue:\n\
- Severity: Critical / High / Medium / Low\n\
- Line number or section\n\
- What's wrong\n\
- How to fix it (with example code if possible)\n\
\n\
Be harsh. I'd rather fix issues now than in production. Suggest improvements and explain your reasoning for each suggestion.\n\
\n\
For the quality score, give a single integer from 0 to 100 and briefly explain the score.\n\
For deviation from the plan, explicitly compare the current changes against the provided latest unfinished plan. If there is no plan, say that explicitly.\n\
For issues found and possible optimization suggestions, keep them separate.\n\
\n\
If you do not find any issue that needs to be fixed, say that explicitly and then list residual risks, assumptions, or verification gaps."
    )
}

fn tool_review_submission_user_prompt(
    batch: &ToolReviewCollectedBatch,
    recent_context: &str,
    plan_text: &str,
) -> String {
    let mut tool_blocks = Vec::<String>::new();
    for item in &batch.items {
        let (_, preview_text) = tool_review_preview_for_item(item);
        let review = item
            .review_value
            .as_ref()
            .and_then(tool_review_value_to_stored_review)
            .map(|value| {
                if let Some(raw_content) = value.raw_content.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
                    format!("{}\n原始审查返回：{}", value.review_opinion, raw_content)
                } else {
                    value.review_opinion
                }
            })
            .unwrap_or_else(|| "尚未生成评估意见。".to_string());
        let result_text = item.result_text.trim();
        let result_preview = if result_text.chars().count() > 800 {
            format!("{}...", result_text.chars().take(800).collect::<String>())
        } else {
            result_text.to_string()
        };
        tool_blocks.push(format!(
            "## 工具 {}：{}\n- 调用ID：{}\n- 预览：\n{}\n- 工具审查：{}\n- 工具结果摘录：\n{}",
            item.order_index,
            item.tool_name,
            item.call_id,
            preview_text,
            review,
            result_preview
        ));
    }
    format!(
        "# 最近对话摘录\n{}\n\n# 最新未完成计划\n{}\n\n# 当前批次工具改动\n{}",
        if recent_context.trim().is_empty() { "无" } else { recent_context },
        if plan_text.trim().is_empty() { "无" } else { plan_text },
        tool_blocks.join("\n\n"),
    )
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
    let runtime_shell = terminal_shell_for_state(state);
    let session_id = format!("tool-review-code::{}", conversation_id.trim());
    let execution = if terminal_live_session_supported(&runtime_shell) {
        terminal_live_exec_command(state, &session_id, cwd, command, timeout_ms).await
    } else {
        sandbox_execute_command(state, &session_id, command, cwd, timeout_ms).await
    }?;
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
        return Err("会话 ID 为空，无法定位审查报告存储。".to_string());
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
        .map_err(|err| format!("读取审查报告文件失败，path={}，error={err}", path.display()))?;
    let mut out = Vec::<ToolReviewReportRecord>::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record = serde_json::from_str::<ToolReviewReportRecord>(trimmed).map_err(|err| {
            format!(
                "解析审查报告记录失败，path={}，line={}，error={err}",
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
                .map_err(|err| format!("序列化审查报告记录失败：{err}"))?,
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

fn tool_review_batch_number_target(batch_number: usize) -> String {
    format!("第 {} 批", batch_number)
}

fn tool_review_find_batch_by_number(
    conversation: &Conversation,
    batch_number: usize,
) -> Result<(usize, ToolReviewCollectedBatch), String> {
    let batches = collect_tool_review_batches_internal(conversation);
    let total = batches.len();
    if total == 0 {
        return Err("当前会话没有可审查的工具批次。".to_string());
    }
    if batch_number == 0 || batch_number > total {
        return Err(format!("批次序号超出范围：batch_number={} total={}", batch_number, total));
    }
    let batch_index = total - batch_number;
    let batch = batches
        .get(batch_index)
        .cloned()
        .ok_or_else(|| format!("未找到批次：batch_number={}", batch_number))?;
    Ok((batch_number, batch))
}

fn tool_review_create_pending_report(
    data_path: &PathBuf,
    conversation_id: &str,
    scope: &str,
    target: &str,
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
        workspace_path: workspace_path.trim().to_string(),
        created_at: now.clone(),
        updated_at: now,
        report_text: String::new(),
        error_text: None,
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
) -> Result<ToolReviewReportRecord, String> {
    let mut records = tool_review_read_report_records(data_path, conversation_id)?;
    let target_id = report_id.trim();
    let position = records
        .iter()
        .position(|item| item.id.trim() == target_id)
        .ok_or_else(|| format!("未找到审查报告记录：{}", target_id))?;
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
        return Err(format!("未找到审查报告记录：{}", target_id));
    }
    tool_review_write_report_records(data_path, conversation_id, &records)
}

fn tool_review_migrate_legacy_reports_after_message_store(
    data_path: &PathBuf,
    conversation: &mut Conversation,
) -> Result<bool, String> {
    let conversation_id = conversation.id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let legacy_reports = conversation
        .messages
        .iter()
        .filter_map(tool_review_report_from_message)
        .collect::<Vec<_>>();
    if legacy_reports.is_empty() {
        return Ok(false);
    }
    let mut existing = tool_review_read_report_records(data_path, conversation_id)?;
    let mut changed = false;
    for legacy in legacy_reports {
        let duplicate = existing.iter().any(|item| {
            item.status == "success"
                && item.scope == legacy.scope
                && item.target == legacy.target
                && item.report_text.trim() == legacy.report_text.trim()
        });
        if duplicate {
            continue;
        }
        existing.push(ToolReviewReportRecord {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            title: String::new(),
            status: "success".to_string(),
            scope: legacy.scope,
            target: legacy.target,
            workspace_path: legacy.workspace_path,
            created_at: legacy.created_at,
            updated_at: legacy.updated_at,
            report_text: legacy.report_text,
            error_text: None,
        });
        changed = true;
    }
    if !changed {
        return Ok(false);
    }
    tool_review_write_report_records(data_path, conversation_id, &existing)?;
    let verified = tool_review_read_report_records(data_path, conversation_id)?;
    if verified.len() < existing.len() {
        return Err(format!(
            "审查报告迁移校验失败，conversation_id={}，expected={}，actual={}",
            conversation_id,
            existing.len(),
            verified.len()
        ));
    }
    let before_len = conversation.messages.len();
    conversation
        .messages
        .retain(|message| !is_tool_review_report_message(message));
    Ok(conversation.messages.len() != before_len)
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
    r#"内置审查 JSON 输出协议：
你必须只返回纯 JSON，不要包 markdown 代码块，不要输出协议字段以外的字段。

JSON 结构：
{
  "review_title": "10 到 20 个中文字符，描述本次审查对象",
  "findings": [
    {
      "title": "一句话标题，80 字以内",
      "body": "说明问题成因、触发条件、影响，引用文件和行号",
      "confidence_score": 0.95,
      "priority": 1,
      "code_location": {
        "absolute_file_path": "E:/project/src/foo.ts",
        "line_range": { "start": 10, "end": 15 }
      }
    },
    {
      "title": "第二个独立缺陷标题，80 字以内",
      "body": "如果发现多个互不依赖的真实缺陷，继续追加 finding；不要因为示例数量而合并或截断",
      "confidence_score": 0.9,
      "priority": 2,
      "code_location": {
        "absolute_file_path": "E:/project/src/bar.ts",
        "line_range": { "start": 30, "end": 34 }
      }
    }
  ],
  "overall_correctness": "patch is correct",
  "overall_explanation": "1 到 3 句整体判断",
  "overall_confidence_score": 0.9
}

规则：
- `findings` 可以为空数组，也可以包含多条。
- 只有确认真实缺陷时才允许输出 finding；证据不足、无法判断、只是建议或担忧时，不得输出 finding。
- 没有确认到真实缺陷时，`findings` 必须是空数组 `[]`，并在 `overall_explanation` 简要说明未发现可确认缺陷或证据不足。
- 发现多个独立真实缺陷时必须逐条列出，不要只输出第一条，也不要把不同问题合并成一条。
- `review_title` 必填，用 10 到 20 个中文字符概括本次审查对象，供报告列表展示。
- `overall_correctness` 只能是 `patch is correct` 或 `patch is incorrect`。
- `code_location` 必须落在当前 diff 范围内。
- `confidence_score` 和 `overall_confidence_score` 取值 0 到 1。
- 除协议字段外不要输出多余字段。"#
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

fn with_tool_review_conversation<T>(
    state: &AppState,
    conversation_id: &str,
    reader: impl FnOnce(&Conversation) -> Result<T, String>,
) -> Result<T, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId 不能为空。".to_string());
    }
    conversation_service().with_unarchived_conversation_by_id_fast(
        state,
        normalized_conversation_id,
        reader,
    )
}

#[tauri::command]
fn list_tool_review_reports(
    input: ToolReviewConversationInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewReportsOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(ListToolReviewReportsOutput { reports: Vec::new() });
    }
    with_tool_review_conversation(state.inner(), conversation_id, |_conversation| {
        Ok(ListToolReviewReportsOutput {
            reports: tool_review_list_reports_newest_first(&state.data_path, conversation_id)?,
        })
    })
}

#[tauri::command]
fn list_tool_review_batches(
    input: ToolReviewConversationInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewBatchesOutput, String> {
    let total_started_at = std::time::Instant::now();
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(ListToolReviewBatchesOutput {
            batches: Vec::new(),
            current_batch_key: None,
        });
    }
    let read_started_at = std::time::Instant::now();
    let (batches, current_batch_key, message_count, collect_elapsed_ms, current_key_elapsed_ms) =
        with_tool_review_conversation(state.inner(), conversation_id, |conversation| {
            let collect_started_at = std::time::Instant::now();
            let batches = collect_tool_review_batches_internal(conversation);
            let collect_elapsed_ms = collect_started_at.elapsed().as_millis();
            let current_key_started_at = std::time::Instant::now();
            let current_batch_key = conversation
                .messages
                .iter()
                .rev()
                .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
                .map(|message| message.id.clone());
            let current_key_elapsed_ms = current_key_started_at.elapsed().as_millis();
            Ok((
                batches,
                current_batch_key,
                conversation.messages.len(),
                collect_elapsed_ms,
                current_key_elapsed_ms,
            ))
        })?;
    let read_elapsed_ms = read_started_at.elapsed().as_millis();
    runtime_log_debug(format!(
        "[工具审查] 批次读取 完成 total_ms={} lock_wait_ms={} read_ms={} collect_ms={} current_batch_ms={} conversation_id={} batch_count={} message_count={}",
        total_started_at.elapsed().as_millis(),
        0,
        read_elapsed_ms,
        collect_elapsed_ms,
        current_key_elapsed_ms,
        conversation_id,
        batches.len(),
        message_count
    ));
    Ok(ListToolReviewBatchesOutput {
        current_batch_key,
        batches: batches
            .iter()
            .map(tool_review_batch_summary_from_collected)
            .collect(),
    })
}

#[tauri::command]
fn get_tool_review_item_detail(
    input: ToolReviewCallInput,
    state: State<'_, AppState>,
) -> Result<ToolReviewItemDetail, String> {
    let conversation_id = input.conversation_id.trim();
    let call_id = input.call_id.trim();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    with_tool_review_conversation(state.inner(), conversation_id, |conversation| {
        let item = tool_review_find_item(conversation, call_id)?;
        Ok(tool_review_item_detail_from_collected(&item))
    })
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
    let mut reviewed_call_ids = Vec::<String>::new();
    for item in batch.items.iter().filter(|item| item.review_value.is_none()) {
        tool_review_run_for_call_internal(state.inner(), conversation_id, &item.call_id).await?;
        reviewed_call_ids.push(item.call_id.clone());
    }
    Ok(RunToolReviewBatchOutput {
        batch_key: batch.batch_key,
        reviewed_call_ids,
    })
}

#[tauri::command]
async fn submit_tool_review_batch(
    input: SubmitToolReviewBatchInput,
    state: State<'_, AppState>,
) -> Result<SubmitToolReviewBatchOutput, String> {
    let conversation_id = input.conversation_id.trim();
    let batch_number = input.batch_number;
    let requested_department_id = input
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    runtime_log_info(format!(
        "[工具审查][后端] 收到 submit_tool_review_batch conversation_id={} batch_number={} department_id={}",
        conversation_id,
        batch_number,
        requested_department_id.as_deref().unwrap_or("")
    ));
    if conversation_id.is_empty() || batch_number == 0 {
        return Err("conversationId 和 batchNumber 不能为空。".to_string());
    }

    let app_state = state.inner().clone();
    let conversation = with_tool_review_conversation(&app_state, conversation_id, |conversation| {
        Ok(conversation.clone())
    })?;
    let (resolved_batch_number, batch_to_review) = tool_review_find_batch_by_number(&conversation, batch_number)?;
    for item in batch_to_review.items.iter().filter(|item| item.review_value.is_none()) {
        tool_review_run_for_call_internal(&app_state, conversation_id, &item.call_id).await?;
    }

    let conversation = with_tool_review_conversation(&app_state, conversation_id, |conversation| {
        Ok(conversation.clone())
    })?;
    let (_, batch) = tool_review_find_batch_by_number(&conversation, resolved_batch_number)?;
    let workspace_path = terminal_default_workspace_for_conversation_resolved(
        &app_state,
        Some(&conversation),
    )
    .map(|workspace| workspace.path)
    .map_err(|err| format!("当前会话缺少可用主工作区，无法生成批次审查报告：{}", err))?;
    let workspace_text = workspace_path.to_string_lossy().to_string();
    let report_target = tool_review_batch_number_target(resolved_batch_number);
    let pending_report = tool_review_create_pending_report(
        &app_state.data_path,
        conversation_id,
        "batch",
        &report_target,
        &workspace_text,
    )?;
    runtime_log_info(format!(
        "[工具审查][后端] 已创建批次审查记录 conversation_id={} batch_number={} report_id={} target={}",
        conversation_id, resolved_batch_number, pending_report.id, report_target
    ));
    emit_tool_review_reports_updated(&app_state, conversation_id, &pending_report.id, "pending");

    let conversation_id_owned = conversation_id.to_string();
    let report_id = pending_report.id.clone();
    let requested_department_id_owned = requested_department_id.clone();
    tauri::async_runtime::spawn(async move {
        runtime_log_info(format!(
            "[工具审查][后端] 开始批次审查子任务 conversation_id={} batch_number={} report_id={}",
            conversation_id_owned, resolved_batch_number, report_id
        ));
        let app_config = match state_read_config_cached(&app_state) {
            Ok(config) => config,
            Err(err) => {
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 读取配置失败 conversation_id={} batch_number={} report_id={} err={}",
                    conversation_id_owned, resolved_batch_number, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let requested_api_config_id = input.api_config_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
        let department_api_config_id = requested_department_id_owned.as_deref().and_then(|department_id| {
            app_config
                .departments
                .iter()
                .find(|department| department.id.trim() == department_id)
                .and_then(|department| {
                    department
                        .api_config_ids
                        .iter()
                        .map(|id| id.trim())
                        .find(|id| !id.is_empty())
                        .or_else(|| {
                            let id = department.api_config_id.trim();
                            if id.is_empty() { None } else { Some(id) }
                        })
                })
        });
        let selected_api = match resolve_selected_api_config(&app_config, requested_api_config_id)
            .or_else(|| resolve_selected_api_config(&app_config, department_api_config_id))
            .or_else(|| resolve_selected_api_config(&app_config, None))
        {
            Some(api) => api,
            None => {
                let err = "当前会话模型不可用。".to_string();
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 选择模型失败 conversation_id={} batch_number={} report_id={} err={}",
                    conversation_id_owned, resolved_batch_number, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let resolved_api = match resolve_api_config(&app_config, Some(&selected_api.id)) {
            Ok(api) => api,
            Err(err) => {
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 解析模型配置失败 conversation_id={} batch_number={} report_id={} err={}",
                    conversation_id_owned, resolved_batch_number, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let prepared = conversation_prompt_service().build_tool_review_submission_prepared_prompt(
            &app_config.ui_language,
            &batch,
            &tool_review_collect_recent_context(&conversation, &batch, 20_000),
            &tool_review_latest_unfinished_plan(&conversation),
        );
        let review_submit_execution = invoke_model_with_policy(
            &resolved_api,
            &selected_api.model,
            prepared,
            CallPolicy {
                scene: "Tool review submit",
                timeout_secs: Some(600),
                json_only: false,
            },
            Some(&app_state),
        )
        .await;
        push_model_call_log_parts(Some(&app_state), &review_submit_execution);
        let reply = match review_submit_execution.result {
            Ok(reply) => reply,
            Err(err) => {
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 批次审查失败 conversation_id={} batch_number={} report_id={} err={}",
                    conversation_id_owned, resolved_batch_number, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
        let report_text = reply.assistant_text.trim().to_string();
        if report_text.is_empty() {
            let err = "最终提交审查未返回内容。".to_string();
            let _ = tool_review_update_report_record(
                &app_state.data_path,
                &conversation_id_owned,
                &report_id,
                "failed",
                None,
                None,
                Some(&err),
            );
            runtime_log_error(format!(
                "[工具审查][后端] 批次审查返回空内容 conversation_id={} batch_number={} report_id={}",
                conversation_id_owned, resolved_batch_number, report_id
            ));
            emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
            return;
        }
        match tool_review_update_report_record(
            &app_state.data_path,
            &conversation_id_owned,
            &report_id,
            "success",
            None,
            Some(&report_text),
            None,
        ) {
            Ok(_) => {
                runtime_log_info(format!(
                    "[工具审查][后端] 批次审查完成 conversation_id={} batch_number={} report_id={}",
                    conversation_id_owned, resolved_batch_number, report_id
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "success");
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[工具审查][后端] 批次审查结果落盘失败 conversation_id={} batch_number={} report_id={} err={}",
                    conversation_id_owned, resolved_batch_number, report_id, err
                ));
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
                );
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
            }
        }
    });

    Ok(SubmitToolReviewBatchOutput { report: pending_report })
}

#[tauri::command]
async fn submit_tool_review_code(
    input: ToolReviewCodeReviewInput,
    state: State<'_, AppState>,
) -> Result<SubmitToolReviewBatchOutput, String> {
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

    let app_state = state.inner().clone();
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
    let requested_department_id = input
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let target_department_id = if let Some(department_id) = requested_department_id {
        department_id.to_string()
    } else if conversation.department_id.trim().is_empty() {
        ASSISTANT_DEPARTMENT_ID.to_string()
    } else {
        conversation.department_id.trim().to_string()
    };
    let pending_report = tool_review_create_pending_report(
        &app_state.data_path,
        conversation_id,
        scope,
        &target_text,
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
    let target_department_id_owned = target_department_id.clone();
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
            department_id: target_department_id_owned.clone(),
            mode: Some("sync".to_string()),
            task_name: Some("代码审查".to_string()),
            instruction,
            background: Some(tool_review_delegate_background(&scope_owned, target_owned.as_deref())),
            specific_goal: Some("输出符合协议的代码审查 JSON".to_string()),
            deliverable_requirement: Some("仅返回纯 JSON，不要包 markdown。".to_string()),
            notify_assistant_when_done: false,
        };
        let session_id = format!("{}::{}", source_agent_id_owned, conversation_id_owned);
        runtime_log_info(format!(
            "[工具审查][后端] 发起代码审查委托 conversation_id={} scope={} report_id={} session_id={} source_agent_id={} target_department_id={}",
            conversation_id_owned,
            scope_owned,
            report_id,
            session_id,
            source_agent_id_owned,
            target_department_id_owned
        ));
        let delegate_result = match builtin_delegate(&app_state, &session_id, delegate_args).await {
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
                );
                runtime_log_error(format!(
                    "[工具审查][后端] 代码审查委托失败 conversation_id={} scope={} report_id={} err={}",
                    conversation_id_owned, scope_owned, report_id, err
                ));
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
                return;
            }
        };
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
                let err = "副手未返回代码审查结果。".to_string();
                let _ = tool_review_update_report_record(
                    &app_state.data_path,
                    &conversation_id_owned,
                    &report_id,
                    "failed",
                    None,
                    None,
                    Some(&err),
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
        let report_title = tool_review_title_from_json_text(&report_text);
        match tool_review_update_report_record(
            &app_state.data_path,
            &conversation_id_owned,
            &report_id,
            "success",
            Some(&report_title),
            Some(&report_text),
            None,
        ) {
            Ok(_) => {
                runtime_log_info(format!(
                    "[工具审查][后端] 代码审查完成 conversation_id={} scope={} report_id={}",
                    conversation_id_owned, scope_owned, report_id
                ));
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
                );
                emit_tool_review_reports_updated(&app_state, &conversation_id_owned, &report_id, "failed");
            }
        }
    });

    Ok(SubmitToolReviewBatchOutput { report: pending_report })
}
