#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewConversationInput {
    conversation_id: String,
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
    batch_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitToolReviewBatchInput {
    conversation_id: String,
    batch_key: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewReportSummary {
    batch_key: String,
    reviewed_tool_call_ids: Vec<String>,
    report_text: String,
    source_tool_count: usize,
    generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolReviewBatchSummary {
    batch_key: String,
    user_message_id: String,
    item_count: usize,
    unreviewed_count: usize,
    items: Vec<ToolReviewItemSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<ToolReviewReportSummary>,
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
    batch_key: String,
    report: ToolReviewReportSummary,
}

#[derive(Debug, Clone)]
struct ToolReviewCollectedItem {
    batch_key: String,
    call_id: String,
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
    items: Vec<ToolReviewCollectedItem>,
    report: Option<ToolReviewReportSummary>,
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

fn tool_review_report_from_message(message: &ChatMessage) -> Option<ToolReviewReportSummary> {
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
    Some(ToolReviewReportSummary {
        batch_key,
        reviewed_tool_call_ids: meta
            .get("reviewedToolCallIds")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        report_text,
        source_tool_count: meta
            .get("sourceToolCount")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        generated_at: meta
            .get("generatedAt")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string(),
    })
}

fn collect_tool_review_batches_internal(conversation: &Conversation) -> Vec<ToolReviewCollectedBatch> {
    let mut current_batch_key = None::<String>;
    let mut current_user_message_id = None::<String>;
    let mut order_index = 0usize;
    let mut batches = Vec::<ToolReviewCollectedBatch>::new();
    let mut batch_index_by_key = std::collections::HashMap::<String, usize>::new();
    let mut pending_calls = std::collections::HashMap::<String, (usize, usize)>::new();
    let mut reports = std::collections::HashMap::<String, ToolReviewReportSummary>::new();

    for message in &conversation.messages {
        if let Some(report) = tool_review_report_from_message(message) {
            reports.insert(report.batch_key.clone(), report);
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
                    items: Vec::new(),
                    report: None,
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
            item.result_text = event.text.trim().to_string();
            item.result_value = serde_json::from_str::<Value>(event.text.trim()).ok();
            item.review_value = item
                .result_value
                .as_ref()
                .and_then(|value| value.get("toolReview"))
                .cloned();
        }
    }

    for batch in batches.iter_mut() {
        batch.report = reports.get(&batch.batch_key).cloned();
    }

    batches
        .into_iter()
        .filter(|batch| !batch.items.is_empty())
        .collect()
}

fn tool_review_batch_summary_from_collected(batch: &ToolReviewCollectedBatch) -> ToolReviewBatchSummary {
    ToolReviewBatchSummary {
        batch_key: batch.batch_key.clone(),
        user_message_id: batch.user_message_id.clone(),
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
            })
            .collect(),
        report: batch.report.clone(),
    }
}

fn tool_review_item_detail_from_collected(item: &ToolReviewCollectedItem) -> ToolReviewItemDetail {
    let (preview_kind, preview_text) = tool_review_preview_for_item(item);
    ToolReviewItemDetail {
        batch_key: item.batch_key.clone(),
        call_id: item.call_id.clone(),
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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(state, conversation_id)?;
    drop(guard);

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

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut conversation = state_read_conversation_cached(state, conversation_id)?;
    tool_review_write_call_review(&mut conversation, call_id, &review_value)?;
    state_write_conversation_cached(state, &conversation)?;
    drop(guard);

    let refreshed = tool_review_find_item(&conversation, call_id)?;
    Ok(tool_review_item_detail_from_collected(&refreshed))
}

fn tool_review_collect_recent_context(conversation: &Conversation, max_chars: usize) -> String {
    let blocks = conversation
        .messages
        .iter()
        .filter(|message| {
            if is_tool_review_report_message(message) {
                return false;
            }
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant"
            )
        })
        .filter_map(|message| {
            let role = message.role.trim().to_ascii_lowercase();
            if role == "user" && is_context_compaction_message(message, "user") {
                return None;
            }
            let text = if role == "user" {
                render_prompt_user_text_only(message)
            } else {
                render_message_content_for_model(message)
            };
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            Some(format!(
                "[{}] {}\n{}",
                if role == "user" { "用户" } else { "助手" },
                message.created_at.trim(),
                text
            ))
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let total_chars = blocks.chars().count();
    if total_chars <= max_chars {
        return blocks;
    }
    blocks
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
            .unwrap_or_else(|| "尚未生成工具审查意见。".to_string());
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

fn tool_review_report_insert_index(
    conversation: &Conversation,
    batch: &ToolReviewCollectedBatch,
) -> usize {
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
        .map(|index| index.saturating_add(1))
        .unwrap_or(conversation.messages.len())
}

fn tool_review_upsert_report_message(
    conversation: &mut Conversation,
    batch: &ToolReviewCollectedBatch,
    report_text: &str,
) -> ToolReviewReportSummary {
    let generated_at = now_iso();
    let reviewed_tool_call_ids = batch
        .items
        .iter()
        .map(|item| item.call_id.clone())
        .collect::<Vec<_>>();
    let report = ToolReviewReportSummary {
        batch_key: batch.batch_key.clone(),
        reviewed_tool_call_ids: reviewed_tool_call_ids.clone(),
        report_text: report_text.trim().to_string(),
        source_tool_count: batch.items.len(),
        generated_at: generated_at.clone(),
    };
    let provider_meta = serde_json::json!({
        "messageKind": "tool_review_report",
        "message_meta": {
            "kind": "tool_review_report",
        },
        "batchKey": report.batch_key,
        "reviewedToolCallIds": report.reviewed_tool_call_ids,
        "reportText": report.report_text,
        "sourceToolCount": report.source_tool_count,
        "generatedAt": report.generated_at,
    });
      let mut matching_indexes = conversation
          .messages
          .iter()
          .enumerate()
          .filter_map(|(index, message)| {
              let matches_batch = is_tool_review_report_message(message)
                  && message
                      .provider_meta
                      .as_ref()
                      .and_then(|meta| meta.get("batchKey"))
                      .and_then(Value::as_str)
                      .map(str::trim)
                      == Some(batch.batch_key.as_str());
              if matches_batch { Some(index) } else { None }
          })
          .collect::<Vec<_>>();
      if let Some(primary_index) = matching_indexes.first().copied() {
          for duplicate_index in matching_indexes.drain(1..).rev() {
              conversation.messages.remove(duplicate_index);
          }
          if let Some(existing) = conversation.messages.get_mut(primary_index) {
              existing.created_at = generated_at.clone();
              existing.parts = vec![MessagePart::Text {
                  text: report_text.trim().to_string(),
              }];
              existing.provider_meta = Some(provider_meta);
              return report;
          }
      }
      let insert_index = tool_review_report_insert_index(conversation, batch)
          .min(conversation.messages.len());
      conversation.messages.insert(insert_index, ChatMessage {
          id: Uuid::new_v4().to_string(),
          role: "assistant".to_string(),
          created_at: generated_at.clone(),
          speaker_agent_id: Some(conversation.agent_id.clone()),
          parts: vec![MessagePart::Text {
            text: report_text.trim().to_string(),
        }],
          extra_text_blocks: Vec::new(),
          provider_meta: Some(provider_meta),
          tool_call: None,
          mcp_call: None,
      });
    report
}

#[tauri::command]
fn list_tool_review_batches(
    input: ToolReviewConversationInput,
    state: State<'_, AppState>,
) -> Result<ListToolReviewBatchesOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(ListToolReviewBatchesOutput {
            batches: Vec::new(),
            current_batch_key: None,
        });
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    drop(guard);
    let batches = collect_tool_review_batches_internal(&conversation);
    let current_batch_key = conversation
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
        .map(|message| message.id.clone());
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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    drop(guard);
    let item = tool_review_find_item(&conversation, call_id)?;
    Ok(tool_review_item_detail_from_collected(&item))
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
    let batch_key = input.batch_key.trim();
    if conversation_id.is_empty() || batch_key.is_empty() {
        return Err("conversationId 和 batchKey 不能为空。".to_string());
    }
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    drop(guard);
    let batch = collect_tool_review_batches_internal(&conversation)
        .into_iter()
        .find(|item| item.batch_key == batch_key)
        .ok_or_else(|| format!("Tool review batch not found: {batch_key}"))?;
    let mut reviewed_call_ids = Vec::<String>::new();
    for item in batch.items.iter().filter(|item| item.review_value.is_none()) {
        tool_review_run_for_call_internal(state.inner(), conversation_id, &item.call_id).await?;
        reviewed_call_ids.push(item.call_id.clone());
    }
    Ok(RunToolReviewBatchOutput {
        batch_key: batch_key.to_string(),
        reviewed_call_ids,
    })
}

#[tauri::command]
async fn submit_tool_review_batch(
    input: SubmitToolReviewBatchInput,
    state: State<'_, AppState>,
) -> Result<SubmitToolReviewBatchOutput, String> {
    let conversation_id = input.conversation_id.trim();
    let batch_key = input.batch_key.trim();
    if conversation_id.is_empty() || batch_key.is_empty() {
        return Err("conversationId 和 batchKey 不能为空。".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    drop(guard);
    let batch_to_review = collect_tool_review_batches_internal(&conversation)
        .into_iter()
        .find(|item| item.batch_key == batch_key)
        .ok_or_else(|| format!("Tool review batch not found: {batch_key}"))?;
    for item in batch_to_review.items.iter().filter(|item| item.review_value.is_none()) {
        tool_review_run_for_call_internal(state.inner(), conversation_id, &item.call_id).await?;
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let conversation = state_read_conversation_cached(&state, conversation_id)?;
    drop(guard);
    let batch = collect_tool_review_batches_internal(&conversation)
        .into_iter()
        .find(|item| item.batch_key == batch_key)
        .ok_or_else(|| format!("Tool review batch not found: {batch_key}"))?;

    let app_config = state_read_config_cached(&state)?;
    let requested_api_config_id = input
        .api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let selected_api = resolve_selected_api_config(&app_config, requested_api_config_id)
        .or_else(|| resolve_selected_api_config(&app_config, None))
        .ok_or_else(|| "当前会话模型不可用。".to_string())?;
    let resolved_api = resolve_api_config(&app_config, Some(&selected_api.id))?;
    let prepared = PreparedPrompt {
        preamble: tool_review_report_system_prompt(&app_config.ui_language),
        history_messages: Vec::new(),
        latest_user_text: tool_review_submission_user_prompt(
            &batch,
            &tool_review_collect_recent_context(&conversation, 20_000),
            &tool_review_latest_unfinished_plan(&conversation),
        ),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };
    let reply = invoke_model_with_policy(
        &resolved_api,
        &selected_api.model,
        prepared,
        CallPolicy {
            scene: "Tool review submit",
            timeout_secs: Some(600),
            json_only: false,
        },
        Some(state.inner()),
    )
    .await?;
    let report_text = reply.assistant_text.trim().to_string();
    if report_text.is_empty() {
        return Err("最终提交审查未返回内容。".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut conversation = state_read_conversation_cached(&state, conversation_id)?;
    let refreshed_batch = collect_tool_review_batches_internal(&conversation)
        .into_iter()
        .find(|item| item.batch_key == batch_key)
        .ok_or_else(|| format!("Tool review batch not found: {batch_key}"))?;
    let report = tool_review_upsert_report_message(&mut conversation, &refreshed_batch, &report_text);
    state_write_conversation_cached(&state, &conversation)?;
    drop(guard);

    Ok(SubmitToolReviewBatchOutput {
        batch_key: batch_key.to_string(),
        report,
    })
}
