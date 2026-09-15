#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConversationServiceV2ErrorCode {
    ConversationNotFound,
    ConversationBusy,
    ConversationReadOnly,
    InvalidOverwriteSource,
    OverwriteForbidden,
    MessageNotFound,
    AnchorNotFound,
    ToolAppendClosed,
    FinalTextAlreadyCommitted,
    MessageNotWritable,
    IntegrityCheckFailed,
    StorageCorrupted,
}

impl ConversationServiceV2ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ConversationNotFound => "CONV_NOT_FOUND",
            Self::ConversationBusy => "CONV_BUSY",
            Self::ConversationReadOnly => "CONV_READ_ONLY",
            Self::InvalidOverwriteSource => "CONV_INVALID_OVERWRITE_SOURCE",
            Self::OverwriteForbidden => "CONV_OVERWRITE_FORBIDDEN",
            Self::MessageNotFound => "MSG_NOT_FOUND",
            Self::AnchorNotFound => "MSG_ANCHOR_NOT_FOUND",
            Self::ToolAppendClosed => "MSG_TOOL_APPEND_CLOSED",
            Self::FinalTextAlreadyCommitted => "MSG_FINAL_TEXT_ALREADY_COMMITTED",
            Self::MessageNotWritable => "MSG_NOT_WRITABLE",
            Self::IntegrityCheckFailed => "STORE_INTEGRITY_FAILED",
            Self::StorageCorrupted => "STORE_CORRUPTED",
        }
    }
}

#[derive(Debug, Clone)]
struct ConversationServiceV2Error {
    code: ConversationServiceV2ErrorCode,
    message: String,
}

impl ConversationServiceV2Error {
    fn new(code: ConversationServiceV2ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn into_string(self) -> String {
        format!("{}: {}", self.code.as_str(), self.message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistReadyMessageMode {
    Replace,
    AppendLineToGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConversationOverwriteSource {
    Import,
    ExportSync,
}

impl ConversationOverwriteSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::ExportSync => "export_sync",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationOverwriteAudit {
    job_id: String,
    source: ConversationOverwriteSource,
    operator: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMetaView {
    id: String,
    title: String,
    latest_summary_title: Option<String>,
    status: String,
    conversation_kind: String,
    visible_in_foreground_lists: bool,
    is_remote_im_contact: bool,
    is_delegate: bool,
    agent_id: String,
    delegate_id: Option<String>,
    root_conversation_id: Option<String>,
    unread_count: usize,
    updated_at: String,
    created_at: String,
    archived_at: Option<String>,
    last_user_at: Option<String>,
    last_assistant_at: Option<String>,
    message_count: usize,
    body_message_count: usize,
    body_text_length: usize,
    has_assistant_reply: bool,
    has_context_compaction_message: bool,
    last_message_at: Option<String>,
    parent_conversation_id: Option<String>,
    child_conversation_ids: Vec<String>,
    fork_message_cursor: Option<String>,
    user_profile_snapshot: String,
    preferred_api_config_id: Option<String>,
    is_draft: bool,
    auto_push_remote_contact_id: Option<String>,
    cumulative_usage: ConversationCumulativeUsage,
    plan_mode_enabled: bool,
    shell_workspace_path: Option<String>,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    shell_autonomous_mode: bool,
    shell_work_mode: String,
    #[serde(default)]
    shell_work_branch: String,
    current_todos: Vec<ConversationTodoItem>,
    active_goal: Option<ConversationGoalState>,
    fast_request_turns: Vec<FastRequestTurn>,
    last_message_id: Option<String>,
    last_error: Option<String>,
    preview_messages: Vec<ConversationMetaPreviewMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMetaPreviewMessage {
    message_id: String,
    role: String,
    speaker_agent_id: Option<String>,
    created_at: Option<String>,
    text_preview: String,
    has_image: bool,
    has_pdf: bool,
    has_audio: bool,
    has_attachment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessagePageView {
    messages: Vec<ChatMessage>,
    has_more: bool,
    has_more_before: bool,
    has_more_after: bool,
    first_message_id: Option<String>,
    last_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageToolAppendInput {
    conversation_id: String,
    assistant_message_id: String,
    assistant_tool_event: Value,
    tool_result_event: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageToolAppendResult {
    conversation_id: String,
    assistant_message_id: String,
    tool_event_count: usize,
    tool_append_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageFinalTextAppendInput {
    conversation_id: String,
    assistant_message_id: String,
    final_text: String,
    reasoning_text: Option<String>,
    provider_meta_patch: Option<Value>,
    meme_annotations: Option<Vec<MemeAnnotation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageFinalTextAppendResult {
    conversation_id: String,
    assistant_message_id: String,
    final_text_committed: bool,
    tool_append_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageBootstrapInput {
    conversation_id: String,
    assistant_message_id: String,
    speaker_agent_id: String,
    created_at: Option<String>,
    provider_meta_patch: Option<Value>,
    /// 调度启动时若携带压缩保留消息，bootstrap 只恢复工具历史；final 正文保持空。
    #[serde(skip)]
    compaction_preserved_messages: Option<CompactionPreservedMessages>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageBootstrapResult {
    conversation_id: String,
    assistant_message_id: String,
    created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageProviderMetaPatchInput {
    conversation_id: String,
    assistant_message_id: String,
    provider_meta_patch: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantMessageProviderMetaPatchResult {
    conversation_id: String,
    assistant_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserMessageAppendInput {
    conversation_id: String,
    message: ChatMessage,
    #[serde(default)]
    memory_recall_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageProviderMetaPatchItem {
    message_id: String,
    provider_meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageProviderMetaBatchPatchInput {
    conversation_id: String,
    items: Vec<MessageProviderMetaPatchItem>,
}

impl ConversationMetaView {
    fn from_meta(meta: &message_store::ConversationShardMeta) -> Self {
        let preview_messages = meta
            .preview_messages()
            .iter()
            .map(|item| ConversationMetaPreviewMessage {
                message_id: item.message_id.clone(),
                role: item.role.clone(),
                speaker_agent_id: item.speaker_agent_id.clone(),
                created_at: item.created_at.clone(),
                text_preview: item.text_preview.clone(),
                has_image: item.has_image,
                has_pdf: item.has_pdf,
                has_audio: item.has_audio,
                has_attachment: item.has_attachment,
            })
            .collect::<Vec<_>>();
        let last_message_id = meta.last_message_id().map(ToOwned::to_owned);
        Self {
            id: meta.id().to_string(),
            title: meta.title().to_string(),
            latest_summary_title: meta.latest_summary_title().map(ToOwned::to_owned),
            status: meta.status().to_string(),
            conversation_kind: meta.conversation_kind().to_string(),
            visible_in_foreground_lists: conversation_service_v2()
                .conversation_meta_visible_in_foreground_lists(meta),
            is_remote_im_contact: conversation_service_v2()
                .conversation_meta_is_remote_im_contact(meta),
            is_delegate: conversation_service_v2().conversation_meta_is_delegate(meta),
            agent_id: meta.agent_id().to_string(),
            delegate_id: meta.delegate_id().map(ToOwned::to_owned),
            root_conversation_id: meta.root_conversation_id_text().map(ToOwned::to_owned),
            unread_count: meta.unread_count(),
            updated_at: meta.updated_at().to_string(),
            created_at: meta.created_at().to_string(),
            archived_at: meta.archived_at().map(ToOwned::to_owned),
            last_user_at: meta.last_user_at().map(ToOwned::to_owned),
            last_assistant_at: meta.last_assistant_at().map(ToOwned::to_owned),
            message_count: meta.message_count(),
            body_message_count: meta.body_message_count(),
            body_text_length: meta.body_text_length(),
            has_assistant_reply: meta.has_assistant_reply(),
            has_context_compaction_message: meta.has_context_compaction_message(),
            last_message_at: meta.last_message_at().map(ToOwned::to_owned),
            parent_conversation_id: meta.parent_conversation_id().map(ToOwned::to_owned),
            child_conversation_ids: meta.child_conversation_ids().to_vec(),
            fork_message_cursor: meta.fork_message_cursor().map(ToOwned::to_owned),
            user_profile_snapshot: meta.user_profile_snapshot().to_string(),
            preferred_api_config_id: meta.preferred_api_config_id().map(ToOwned::to_owned),
            is_draft: meta.is_draft(),
            auto_push_remote_contact_id: meta.auto_push_remote_contact_id().map(ToOwned::to_owned),
            cumulative_usage: meta.cumulative_usage().clone(),
            plan_mode_enabled: meta.plan_mode_enabled(),
            shell_workspace_path: meta.shell_workspace_path().map(ToOwned::to_owned),
            shell_workspaces: meta.shell_workspaces().to_vec(),
            shell_autonomous_mode: meta.shell_autonomous_mode(),
            shell_work_mode: normalize_shell_work_mode_text(meta.shell_work_mode()),
            shell_work_branch: meta.shell_work_branch().to_string(),
            current_todos: meta.current_todos().to_vec(),
            active_goal: meta.active_goal().cloned(),
            fast_request_turns: meta.fast_request_turns().to_vec(),
            last_message_id,
            last_error: meta.last_error().map(ToOwned::to_owned),
            preview_messages,
        }
    }
}

const FRONTEND_MESSAGE_DISPLAY_TOOL_RESULT_PLACEHOLDER_TEXT: &str = "工具已执行，结果已省略。";

// 前端消息展示专用：这里会把 tool result 正文替换成占位文案。
// 禁止任何写路径、撤回路径、持久化路径复用本函数或其返回值，
// 否则会把原始 tool metadata（如 backup_record_id）污染掉。
fn project_tool_history_event_for_frontend_message_display_only(event: &Value) -> Value {
    let Some(object) = event.as_object() else {
        return event.clone();
    };
    let role = object
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if role != "tool" {
        return event.clone();
    }
    let tool_call_id = object
        .get("tool_call_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let mut sanitized = object.clone();
    sanitized.insert(
        "content".to_string(),
        Value::String(FRONTEND_MESSAGE_DISPLAY_TOOL_RESULT_PLACEHOLDER_TEXT.to_string()),
    );
    sanitized.insert("contentOmitted".to_string(), Value::Bool(true));
    if !tool_call_id.is_empty() {
        sanitized.insert(
            "tool_call_id".to_string(),
            Value::String(tool_call_id.to_string()),
        );
    }
    Value::Object(sanitized)
}

// 前端消息展示专用：只用于返回给前端渲染层的消息投影。
// 禁止把本函数返回的消息再写回仓库；需要写回时必须使用原始消息读取函数。
fn project_message_for_frontend_display_only(mut message: ChatMessage) -> ChatMessage {
    if let Some(events) = message.tool_call.take() {
        let projected = events
            .into_iter()
            .map(|event| project_tool_history_event_for_frontend_message_display_only(&event))
            .collect::<Vec<_>>();
        message.tool_call = if projected.is_empty() {
            None
        } else {
            Some(projected)
        };
    }
    message
}

// 前端消息展示专用：批量消息投影，语义同 `project_message_for_frontend_display_only`。
fn project_messages_for_frontend_display_only(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    messages
        .into_iter()
        .map(project_message_for_frontend_display_only)
        .collect()
}

fn assistant_message_has_final_text(message: &ChatMessage) -> bool {
    message.parts.iter().any(|part| match part {
        MessagePart::Text { text, .. } => !text.trim().is_empty(),
        _ => false,
    }) || message
        .extra_text_blocks
        .iter()
        .any(|block| !block.trim().is_empty())
}

fn build_message_page_view_v2(
    messages: Vec<ChatMessage>,
    has_more_before: bool,
    has_more_after: bool,
) -> ConversationMessagePageView {
    let first_message_id = messages
        .first()
        .map(|message| message.id.trim().to_string())
        .filter(|value| !value.is_empty());
    let last_message_id = messages
        .last()
        .map(|message| message.id.trim().to_string())
        .filter(|value| !value.is_empty());
    ConversationMessagePageView {
        has_more: has_more_before || has_more_after,
        has_more_before,
        has_more_after,
        first_message_id,
        last_message_id,
        messages,
    }
}

fn dedup_memory_recall_ids_v2(ids: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    ids.iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

fn assistant_message_tool_append_closed(message: &ChatMessage) -> bool {
    assistant_message_has_final_text(message)
        || message
            .provider_meta
            .as_ref()
            .and_then(|meta| meta.get("streamFinalCommitted"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn merge_optional_text_block_v2(current: &mut Option<String>, next: Option<String>) {
    let Some(next) = next else {
        return;
    };
    match current {
        Some(current) if !current.is_empty() && !next.is_empty() => {
            current.push_str("\n\n");
            current.push_str(&next);
        }
        Some(current) if current.is_empty() => *current = next,
        Some(_) => {}
        _ => *current = Some(next),
    }
}

fn merge_provider_meta_patch_v2(target: &mut Option<Value>, patch: Option<Value>) {
    let Some(patch) = patch else {
        return;
    };
    let Some(patch_obj) = patch.as_object() else {
        return;
    };
    if patch_obj.is_empty() {
        return;
    }
    let mut current = target.take().unwrap_or_else(|| serde_json::json!({}));
    if !current.is_object() {
        current = serde_json::json!({
            "_raw_provider_meta": current,
        });
    }
    if let Some(current_obj) = current.as_object_mut() {
        for (key, value) in patch_obj {
            current_obj.insert(key.clone(), value.clone());
        }
    }
    *target = Some(current);
}

/// 用量聚合：meta 尚无真实用量时，取最后一个自带真实用量的工具调用事件派生展示字段。
/// 工具轮的用量随工具调用事件落盘（出生点挂载），此处只兜底、不覆盖最终 call 写入的真实用量。
fn merge_last_tool_call_usage_into_provider_meta(
    target: &mut Option<Value>,
    tool_call: &Option<Vec<Value>>,
) {
    if target
        .as_ref()
        .and_then(|meta| meta.get("providerPromptTokens"))
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0)
    {
        return;
    }
    let Some(events) = tool_call.as_deref() else {
        return;
    };
    let Some((prompt_tokens, Some(context_window))) = resolve_tool_call_usage(events) else {
        // 事件缺 contextWindowTokens：占用率无法正确计算，跳过（不派生出错误的占用率）
        return;
    };
    let usage_ratio = prompt_tokens as f64 / context_window as f64;
    let usage_percent = (usage_ratio * 100.0).round().clamp(0.0, 100.0) as u32;
    let mut current = target.take().unwrap_or_else(|| serde_json::json!({}));
    if !current.is_object() {
        current = serde_json::json!({
            "_raw_provider_meta": current,
        });
    }
    if let Some(current_obj) = current.as_object_mut() {
        current_obj.insert("providerPromptTokens".to_string(), serde_json::json!(prompt_tokens));
        current_obj.insert("effectivePromptTokens".to_string(), serde_json::json!(prompt_tokens));
        current_obj.insert(
            "effectivePromptSource".to_string(),
            serde_json::json!("provider_tool_round"),
        );
        current_obj.insert("contextUsageRatio".to_string(), serde_json::json!(usage_ratio));
        current_obj.insert(
            "contextUsagePercent".to_string(),
            serde_json::json!(usage_percent),
        );
        current_obj.insert(
            "contextWindowTokens".to_string(),
            serde_json::json!(context_window),
        );
    }
    *target = Some(current);
}

fn mark_stream_final_committed_v2(target: &mut Option<Value>) {
    let mut current = target.take().unwrap_or_else(|| serde_json::json!({}));
    if !current.is_object() {
        current = serde_json::json!({
            "_raw_provider_meta": current,
        });
    }
    if let Some(current_obj) = current.as_object_mut() {
        current_obj.insert("streamFinalCommitted".to_string(), Value::Bool(true));
    }
    *target = Some(current);
}

fn tool_call_ids_from_assistant_tool_event_v2(event: &Value) -> Vec<String> {
    event
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("id")
                .or_else(|| item.get("call_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn tool_history_contains_assistant_tool_group_v2(
    events: &[Value],
    group_call_ids: &[String],
) -> bool {
    if group_call_ids.is_empty() {
        return false;
    }
    events.iter().any(|event| {
        event
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| role.trim().eq_ignore_ascii_case("assistant"))
            && event
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|call| {
                    call.get("id")
                        .or_else(|| call.get("call_id"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .is_some_and(|id| group_call_ids.iter().any(|candidate| candidate == id))
                })
    })
}

fn tool_history_contains_tool_result_id_v2(events: &[Value], tool_call_id: &str) -> bool {
    events.iter().any(|event| {
        event
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| role.trim().eq_ignore_ascii_case("tool"))
            && event
                .get("tool_call_id")
                .and_then(Value::as_str)
                .map(str::trim)
                == Some(tool_call_id)
    })
}

fn validate_tool_group_result_append_v2(
    assistant_tool_call_event: &Value,
    tool_result_event: &Value,
) -> Result<String, String> {
    let assistant_role = assistant_tool_call_event
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if assistant_role != "assistant" {
        return Err("追加工具结果失败：第一条事件必须是 assistant tool_call".to_string());
    }
    let group_call_ids = tool_call_ids_from_assistant_tool_event_v2(assistant_tool_call_event);
    if group_call_ids.is_empty() {
        return Err("追加工具结果失败：assistant 事件缺少 tool_calls".to_string());
    }
    let tool_role = tool_result_event
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if tool_role != "tool" {
        return Err("追加工具结果失败：第二条事件必须是 tool result".to_string());
    }
    let result_call_id = tool_result_event
        .get("tool_call_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "追加工具结果失败：tool result 缺少 tool_call_id".to_string())?;
    if !group_call_ids.iter().any(|tool_call_id| tool_call_id == result_call_id) {
        return Err(format!(
            "追加工具结果失败：tool_call_id 不在工具组内，group_tool_call_ids={}，result_tool_call_id={}",
            group_call_ids.join(","),
            result_call_id
        ));
    }
    Ok(result_call_id.to_string())
}

#[derive(Debug, Default)]
struct ConversationServiceV2;

#[derive(Debug, Default, Clone)]
struct ConversationExternalMetadataPatch {
    title: Option<String>,
    unread_count: Option<usize>,
    preferred_api_config_id: Option<Option<String>>,
    auto_push_remote_contact_id: Option<Option<String>>,
    current_todos: Option<Vec<ConversationTodoItem>>,
    plan_mode_enabled: Option<bool>,
    shell_workspace_path: Option<Option<String>>,
    shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    shell_autonomous_mode: Option<bool>,
    shell_work_mode: Option<String>,
    shell_work_branch: Option<String>,
    lifecycle_status: Option<String>,
    lifecycle_archived_at: Option<Option<String>>,
    lifecycle_updated_at: Option<String>,
    routing_agent_id: Option<String>,
    routing_root_conversation_id: Option<Option<String>>,
    routing_conversation_kind: Option<String>,
    is_draft: Option<bool>,
}

fn conversation_service_v2() -> &'static ConversationServiceV2 {
    static SERVICE: OnceLock<ConversationServiceV2> = OnceLock::new();
    SERVICE.get_or_init(ConversationServiceV2::default)
}

#[cfg(test)]
fn conversation_service() -> &'static ConversationServiceV2 {
    conversation_service_v2()
}

fn publish_pending_new_conversation_if_needed(
    state: &AppState,
    conversation_id: &str,
    paths: &message_store::MessageStorePaths,
) -> Result<(), String> {
    if message_store::chat_store_read_status(paths)?.is_some() {
        return Ok(());
    }
    let conversation = {
        let pending = state
            .conversation_persist_pending
            .lock()
            .map_err(|_| "读取待持久化新会话失败".to_string())?;
        pending
            .as_ref()
            .and_then(|slot| slot.conversations.get(conversation_id).cloned())
    };
    let Some(conversation) = conversation else {
        return Ok(());
    };
    // 这里只发布当前进程刚创建、尚在 pending 队列中的新会话。新建接口本身
    // 仍只入 pending；若紧随创建发生首条写入/首读，则同步发布到 V4 current
    // store 并 flush，保证消息只追加到当前生产存储。
    message_store::chat_store_write_snapshot(paths, &conversation)?;
    state_mark_conversation_metadata_direct_persisted(state, conversation_id)?;
    flush_pending_persists_blocking(state)?;
    Ok(())
}

impl ConversationServiceV2 {
    fn ensure_appendable_ready_message_store(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<message_store::ConversationShardMeta, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        publish_pending_new_conversation_if_needed(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        message_store::chat_store_read_meta(&store_paths)?
            .ok_or_else(|| {
                format!(
                    "会话消息仓库不可追加：缺少 ready 消息元数据，conversation_id={normalized_conversation_id}"
                )
            })
    }

    fn fill_summary_preview_messages_fallback(
        &self,
        state: &AppState,
        conversation_meta: &ConversationMetaView,
    ) -> ConversationMetaView {
        if !conversation_meta.preview_messages.is_empty() {
            return conversation_meta.clone();
        }
        let fallback_messages = match self.get_recent_messages_for_frontend_display_only(
            state,
            &conversation_meta.id,
            2,
        ) {
            Ok(messages) => messages,
            Err(_) => return conversation_meta.clone(),
        };
        if fallback_messages.is_empty() {
            return conversation_meta.clone();
        }
        let mut hydrated = conversation_meta.clone();
        hydrated.preview_messages = build_preview_messages_from_chat_messages(&fallback_messages, 2)
            .into_iter()
            .map(|message| ConversationMetaPreviewMessage {
                message_id: message.message_id,
                role: message.role,
                speaker_agent_id: message.speaker_agent_id,
                created_at: message.created_at,
                text_preview: message.text_preview,
                has_image: message.has_image,
                has_pdf: message.has_pdf,
                has_audio: message.has_audio,
                has_attachment: message.has_attachment,
            })
            .collect();
        if hydrated.message_count == 0 {
            hydrated.message_count = fallback_messages.len();
        }
        if hydrated.body_message_count == 0 {
            hydrated.body_message_count = fallback_messages
                .iter()
                .filter(|message| {
                    matches!(
                        message.role.trim().to_ascii_lowercase().as_str(),
                        "user" | "assistant"
                    )
                })
                .count();
        }
        if hydrated.body_text_length == 0 {
            hydrated.body_text_length = fallback_messages
                .iter()
                .flat_map(|message| message.parts.iter())
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim().chars().count()),
                    _ => None,
                })
                .sum();
        }
        if !hydrated.has_assistant_reply {
            hydrated.has_assistant_reply = fallback_messages
                .iter()
                .any(|message| message.role.trim().eq_ignore_ascii_case("assistant"));
        }
        hydrated
    }

    fn validate_overwrite_audit(
        &self,
        audit: &ConversationOverwriteAudit,
    ) -> Result<(), String> {
        if audit.job_id.trim().is_empty() {
            return Err("overwrite audit jobId is required.".to_string());
        }
        if audit.operator.trim().is_empty() {
            return Err("overwrite audit operator is required.".to_string());
        }
        if audit.reason.trim().is_empty() {
            return Err("overwrite audit reason is required.".to_string());
        }
        Ok(())
    }

    // 危险：这是遗留的整会话覆写入口，会直接按传入 snapshot 全量重写消息仓库。
    // 除导入/迁移这类“向空会话灌入外部快照”的过渡场景外，项目内一律禁止调用。
    // 普通聊天、补写、恢复、自愈、同步都必须使用原子增量接口，不能复用这个方法。
    // 后续目标：彻底移除此能力，连导入也改为按消息/按 block 增量写入。
    fn apply_privileged_snapshot_overwrite(
        &self,
        state: &AppState,
        audit: &ConversationOverwriteAudit,
        snapshot: &Conversation,
    ) -> Result<(), String> {
        self.validate_overwrite_audit(audit)?;
        let snapshot_id = snapshot.id.trim();
        if snapshot_id.is_empty() {
            return Err("overwrite snapshot conversation.id is required.".to_string());
        }
        with_conversation_mutation(state, snapshot_id, "apply_privileged_snapshot_overwrite", || {
            self.apply_privileged_snapshot_overwrite_inner(state, audit, snapshot)
        })
    }

    fn apply_privileged_snapshot_overwrite_inner(
        &self,
        state: &AppState,
        audit: &ConversationOverwriteAudit,
        snapshot: &Conversation,
    ) -> Result<(), String> {
        self.validate_overwrite_audit(audit)?;
        if snapshot.id.trim().is_empty() {
            return Err("overwrite snapshot conversation.id is required.".to_string());
        }
        runtime_log_info(format!(
            "[会话V2] 开始，任务=特批覆写会话，conversation_id={}，source={}，job_id={}，operator={}，reason={}，message_count={}",
            snapshot.id,
            audit.source.as_str(),
            audit.job_id,
            audit.operator,
            audit.reason,
            snapshot.messages.len()
        ));
        let store_paths = message_store::message_store_paths(&state.data_path, &snapshot.id)?;
        message_store::chat_store_write_snapshot(&store_paths, snapshot)?;
        state_mark_conversation_metadata_direct_persisted(state, &snapshot.id)?;
        runtime_log_info(format!(
            "[会话V2] 完成，任务=特批覆写会话，conversation_id={}，source={}，job_id={}，operator={}，message_count={}",
            snapshot.id,
            audit.source.as_str(),
            audit.job_id,
            audit.operator,
            snapshot.messages.len()
        ));
        Ok(())
    }

    fn read_current_writable_assistant_message(
        &self,
        state: &AppState,
        conversation_id: &str,
        assistant_message_id: &str,
    ) -> Result<ChatMessage, String> {
        let target_message =
            self.get_raw_message_by_id(state, conversation_id, assistant_message_id)?;
        if target_message.role.trim() != "assistant" {
            return Err(
                ConversationServiceV2Error::new(
                    ConversationServiceV2ErrorCode::MessageNotWritable,
                    format!(
                        "目标消息不是 assistant，conversationId={}，assistantMessageId={}",
                        conversation_id, assistant_message_id
                    ),
                )
                .into_string(),
            );
        }
        Ok(target_message)
    }

    fn get_raw_recent_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_limit = limit.clamp(1, 50);
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let mut messages = if let Some(page) =
            message_store::chat_store_read_recent_messages_page_cached(
                &store_paths,
                normalized_limit,
            )?
        {
            page.messages
        } else {
            let conversation = state_read_conversation_cached(state, conversation_id)?;
            self.ensure_unarchived_conversation(&conversation, conversation_id)?;
            let total = conversation.messages.len();
            let start = total.saturating_sub(normalized_limit);
            conversation.messages[start..].to_vec()
        };
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(messages)
    }

    fn get_raw_message_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<ChatMessage, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let normalized_message_id = message_id.trim();
        if normalized_message_id.is_empty() {
            return Err("messageId is required.".to_string());
        }
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        let mut message =
            message_store::chat_store_read_message_by_id(&store_paths, normalized_message_id)?
                .ok_or_else(|| format!("Message not found: {normalized_message_id}"))?;
        materialize_chat_message_parts_from_media_refs(
            std::slice::from_mut(&mut message),
            &state.data_path,
        );
        Ok(message)
    }

    fn conversation_has_active_chat_view(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> bool {
        let target = conversation_id.trim();
        if target.is_empty() {
            return false;
        }
        state
            .active_chat_view_bindings
            .lock()
            .map(|bindings| {
                bindings.values().any(|binding| {
                    let bound = binding.conversation_id.trim();
                    !bound.is_empty() && bound != "*" && bound == target
                })
            })
            .unwrap_or(false)
    }

    fn mark_conversation_metadata_cached_persisted(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<(), String> {
        state_mark_conversation_metadata_cached_persisted_unlocked(state, conversation_id)
    }

    fn increment_conversation_unread_count_if_background(
        &self,
        state: &AppState,
        conversation: &mut Conversation,
        count: usize,
        mutation_gate_already_held: bool,
    ) {
        if count == 0 {
            return;
        }
        if self.conversation_has_active_chat_view(state, &conversation.id) {
            clear_conversation_unread_count(conversation);
        } else {
            increment_conversation_unread_count(conversation, count);
        }
        let updater = |cached: &mut Conversation| {
                cached.unread_count = conversation.unread_count;
                cached.updated_at = conversation.updated_at.clone();
                cached.last_user_at = conversation.last_user_at.clone();
                cached.last_assistant_at = conversation.last_assistant_at.clone();
                Ok(())
            };
        let update_result = if mutation_gate_already_held {
            state_update_conversation_metadata_cached_unlocked(
                state,
                &conversation.id,
                updater,
            )
        } else {
            state_update_conversation_metadata_cached(state, &conversation.id, updater)
        };
        if let Err(err) = update_result {
            runtime_log_warn(format!(
                "[会话未读] 警告，任务=同步未读数metadata缓存，会话ID={}，unread_count={}，error={}",
                conversation.id, conversation.unread_count, err
            ));
        }
    }

    fn persist_replaced_ready_message_locked(
        &self,
        state: &AppState,
        conversation_id: &str,
        updated_message: &ChatMessage,
    ) -> Result<(), String> {
        self.persist_ready_message_locked_inner(
            state,
            conversation_id,
            updated_message,
            PersistReadyMessageMode::Replace,
        )
    }

    fn persist_appended_ready_message_locked(
        &self,
        state: &AppState,
        conversation_id: &str,
        updated_message: &ChatMessage,
    ) -> Result<(), String> {
        self.persist_ready_message_locked_inner(
            state,
            conversation_id,
            updated_message,
            PersistReadyMessageMode::AppendLineToGroup,
        )
    }

    fn persist_ready_message_locked_inner(
        &self,
        state: &AppState,
        conversation_id: &str,
        updated_message: &ChatMessage,
        mode: PersistReadyMessageMode,
    ) -> Result<(), String> {
        let previous_message = self.get_raw_message_by_id(
            state,
            conversation_id,
            updated_message.id.trim(),
        )?;
        let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let mut ready_meta = message_store::chat_store_read_meta(&paths)?
            .ok_or_else(|| {
                ConversationServiceV2Error::new(
                    ConversationServiceV2ErrorCode::StorageCorrupted,
                    format!(
                        "ready 消息元数据缺失，conversationId={}，assistantMessageId={}",
                        conversation_id,
                        updated_message.id.trim()
                    ),
                )
                .into_string()
            })?;
        let updated_at = updated_message.created_at.clone();
        let last_assistant_at = Some(updated_at.clone());
        let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
            state,
            conversation_id,
            |cached| {
                let mut metadata_conversation =
                    self.build_conversation_snapshot_from_meta(cached, Vec::new());
                metadata_conversation.updated_at = updated_at.clone();
                metadata_conversation.last_assistant_at = last_assistant_at.clone();
                cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                cached.apply_replaced_messages(
                    std::slice::from_ref(&previous_message),
                    std::slice::from_ref(updated_message),
                    || {
                        message_store::chat_store_recompute_latest_summary_title_after_replace(
                            &paths,
                            std::slice::from_ref(updated_message),
                        )
                    },
                )?;
                Ok(())
            },
        )?;
        ready_meta.apply_metadata_fields_from_meta(&updated_meta);
        ready_meta.preserve_message_derived_fields_from(&updated_meta);
        match mode {
            PersistReadyMessageMode::Replace => {
                message_store::chat_store_replace_message(
                    &paths,
                    &ready_meta.to_persist_meta(),
                    updated_message,
                )?;
            }
            PersistReadyMessageMode::AppendLineToGroup => {
                message_store::chat_store_append_line_to_group(
                    &paths,
                    &ready_meta.to_persist_meta(),
                    &previous_message,
                    updated_message,
                )?;
            }
        }
        self.mark_conversation_metadata_cached_persisted(state, conversation_id)?;
        Ok(())
    }

    fn resolve_latest_foreground_conversation_id(
        &self,
        state: &AppState,
        agent_id: &str,
    ) -> Result<Option<String>, String> {
        let normalized_agent_id = agent_id.trim();
        let chat_index = state_read_chat_index_cached(state)?;
        Ok(chat_index
            .conversations
            .iter()
            .rev()
            .find_map(|item| {
                let conversation_meta = self.get_conversation_meta(state, &item.id).ok()?;
                if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return None;
                }
                if !conversation_meta.visible_in_foreground_lists
                    || !self.conversation_meta_is_local_normal_chat_meta_view(&conversation_meta)
                {
                    return None;
                }
                if !normalized_agent_id.is_empty()
                    && conversation_meta.agent_id.trim() != normalized_agent_id
                {
                    return None;
                }
                Some(conversation_meta.id.to_string())
            }))
    }

    fn with_unarchived_conversation_by_id_fast<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        reader: impl FnOnce(&Conversation) -> Result<T, String>,
    ) -> Result<T, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let conversation = self.read_persisted_conversation(state, normalized_conversation_id)
            .map_err(|err| {
                format!(
                    "Unarchived conversation not found: {normalized_conversation_id}: {err}"
                )
            })?;
        self.ensure_unarchived_conversation(&conversation, normalized_conversation_id)?;
        let result = reader(&conversation)?;
        Ok(result)
    }

    fn apply_external_metadata_patch(
        &self,
        state: &AppState,
        conversation_id: &str,
        task_name: &str,
        patch: ConversationExternalMetadataPatch,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let (conversation, (), _) = with_conversation_mutation(
            state,
            normalized_conversation_id,
            task_name,
            || {
                state_update_conversation_metadata_cached_unlocked(
                    state,
                    normalized_conversation_id,
                    |conversation| {
                        if let Some(value) = patch.title {
                            conversation.title = value;
                        }
                        if let Some(value) = patch.unread_count {
                            conversation.unread_count = value;
                        }
                        if let Some(value) = patch.preferred_api_config_id {
                            conversation.preferred_api_config_id = value;
                        }
                        if let Some(value) = patch.auto_push_remote_contact_id {
                            conversation.auto_push_remote_contact_id = value;
                        }
                        if let Some(value) = patch.current_todos {
                            conversation.current_todos = value;
                        }
                        if let Some(value) = patch.plan_mode_enabled {
                            conversation.plan_mode_enabled = value;
                        }
                        if let Some(value) = patch.shell_workspace_path {
                            conversation.shell_workspace_path = value;
                        }
                        if let Some(value) = patch.shell_workspaces {
                            conversation.shell_workspaces = value;
                        }
                        if let Some(value) = patch.shell_autonomous_mode {
                            conversation.shell_autonomous_mode = value;
                        }
                        if let Some(value) = patch.shell_work_mode {
                            conversation.shell_work_mode = normalize_shell_work_mode_text(&value);
                        }
                        if let Some(value) = patch.shell_work_branch {
                            conversation.shell_work_branch =
                                normalize_shell_work_branch_text(&value);
                        }
                        if let Some(value) = patch.lifecycle_status {
                            conversation.status = value;
                        }
                        if let Some(value) = patch.lifecycle_archived_at {
                            conversation.archived_at = value;
                        }
                        if let Some(value) = patch.lifecycle_updated_at {
                            conversation.updated_at = value;
                        }
                        if let Some(value) = patch.routing_agent_id {
                            conversation.agent_id = value;
                        }
                        if let Some(value) = patch.routing_root_conversation_id {
                            conversation.root_conversation_id = value;
                        }
                        if let Some(value) = patch.routing_conversation_kind {
                            conversation.conversation_kind = value;
                        }
                        if let Some(value) = patch.is_draft {
                            conversation.is_draft = value;
                        }
                        if conversation
                            .shell_workspace_path
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .is_some()
                            && terminal_workspace_path_from_conversation(state, conversation).is_none()
                        {
                            conversation.shell_workspace_path = None;
                        }
                        Ok(())
                    },
                )
            },
        )?;
        Ok(conversation)
    }

    fn get_conversation_meta(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<ConversationMetaView, String> {
        let meta = match state_read_conversation_metadata_cached(state, conversation_id) {
            Ok(meta) => meta,
            // 仅使用本次运行中已经进入 pending/current runtime cache 的完整快照兜住
            // 尚未落盘的状态；这里不会读取或解释 V1/V2 文件。
            Err(meta_err) => match state_read_conversation_cached(state, conversation_id) {
                Ok(conversation) => {
                    runtime_log_warn(format!(
                        "[会话元数据] 消息仓库轻量读取失败，使用当前运行时缓存继续，conversation_id={}，error={}",
                        conversation_id.trim(), meta_err
                    ));
                    message_store::ConversationShardMeta::from_conversation(&conversation)
                }
                Err(_) => {
                    return Err(ConversationServiceV2Error::new(
                        ConversationServiceV2ErrorCode::ConversationNotFound,
                        format!("conversationId={}", conversation_id.trim()),
                    )
                    .into_string())
                }
            },
        };
        Ok(ConversationMetaView::from_meta(&meta))
    }

    fn get_conversation_metadata_record(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        let conversation_meta = self.get_conversation_meta(state, conversation_id)?;
        Ok(self.build_conversation_record_from_meta_view(&conversation_meta))
    }

    fn ensure_system_notification_conversation(
        &self,
        state: &AppState,
    ) -> Result<(), String> {
        let exists = self
            .get_conversation_meta(state, SYSTEM_NOTIFICATION_CONVERSATION_ID)
            .ok()
            .filter(|conversation_meta| {
                self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                    && self.conversation_meta_is_system_notification_meta_view(conversation_meta)
            })
            .is_some();
        if exists {
            return Ok(());
        }
        let conversation = build_system_notification_conversation_record();
        state_schedule_conversation_persist(state, &conversation)?;
        Ok(())
    }

    fn conversation_meta_is_delegate(
        &self,
        conversation_meta: &message_store::ConversationShardMeta,
    ) -> bool {
        conversation_meta.conversation_kind().trim() == CONVERSATION_KIND_DELEGATE
    }

    fn conversation_meta_is_system_notification_meta_view(
        &self,
        conversation_meta: &ConversationMetaView,
    ) -> bool {
        conversation_meta.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID
            || conversation_meta.conversation_kind.trim()
                == CONVERSATION_KIND_SYSTEM_NOTIFICATION
    }

    fn conversation_meta_is_unarchived_meta_view(
        &self,
        conversation_meta: &ConversationMetaView,
    ) -> bool {
        conversation_meta.status.trim() != "archived"
            && conversation_meta
                .archived_at
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
    }

    fn conversation_meta_visible_in_foreground_lists(
        &self,
        conversation_meta: &message_store::ConversationShardMeta,
    ) -> bool {
        !self.conversation_meta_is_delegate(conversation_meta)
            && !self.conversation_meta_is_remote_im_contact(conversation_meta)
            && conversation_meta.conversation_kind().trim() != CONVERSATION_KIND_SIDE_CHAT
    }

    fn conversation_meta_is_remote_im_contact(
        &self,
        conversation_meta: &message_store::ConversationShardMeta,
    ) -> bool {
        conversation_meta.conversation_kind().trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
    }

    fn conversation_meta_is_local_normal_chat_meta_view(
        &self,
        conversation_meta: &ConversationMetaView,
    ) -> bool {
        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
            && conversation_meta.visible_in_foreground_lists
            && conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_CHAT
            && conversation_meta.conversation_kind.trim()
                != CONVERSATION_KIND_SYSTEM_NOTIFICATION
    }

    fn conversation_meta_is_local_conversation_runtime_meta_view(
        &self,
        conversation_meta: &ConversationMetaView,
    ) -> bool {
        // 这是运行时能力判断，不等于“是否出现在主会话列表”；side_chat 必须走普通消息操作。
        self.conversation_meta_is_unarchived_meta_view(conversation_meta)
            && matches!(
                conversation_meta.conversation_kind.trim(),
                CONVERSATION_KIND_CHAT | CONVERSATION_KIND_SIDE_CHAT
            )
    }

    fn build_conversation_snapshot_from_meta(
        &self,
        conversation_meta: &message_store::ConversationShardMeta,
        messages: Vec<ChatMessage>,
    ) -> Conversation {
        let mut conversation = build_conversation_record("", "", "", "", None, None);
        conversation.id = conversation_meta.id().to_string();
        conversation_meta.apply_to_conversation(&mut conversation);
        conversation.messages = messages;
        conversation
    }

    fn build_conversation_record_from_meta_view(
        &self,
        conversation_meta: &ConversationMetaView,
    ) -> Conversation {
        let mut conversation = build_conversation_record("", "", "", "", None, None);
        conversation.id = conversation_meta.id.clone();
        conversation.title = conversation_meta.title.clone();
        conversation.agent_id = conversation_meta.agent_id.clone();
        conversation.unread_count = conversation_meta.unread_count;
        conversation.parent_conversation_id = conversation_meta.parent_conversation_id.clone();
        conversation.child_conversation_ids = conversation_meta.child_conversation_ids.clone();
        conversation.conversation_kind = conversation_meta.conversation_kind.clone();
        conversation.root_conversation_id = conversation_meta.root_conversation_id.clone();
        conversation.delegate_id = conversation_meta.delegate_id.clone();
        conversation.created_at = conversation_meta.created_at.clone();
        conversation.updated_at = conversation_meta.updated_at.clone();
        conversation.last_user_at = conversation_meta.last_user_at.clone();
        conversation.last_assistant_at = conversation_meta.last_assistant_at.clone();
        conversation.status = conversation_meta.status.clone();
        conversation.user_profile_snapshot = conversation_meta.user_profile_snapshot.clone();
        conversation.shell_workspace_path = conversation_meta.shell_workspace_path.clone();
        conversation.shell_workspaces = conversation_meta.shell_workspaces.clone();
        conversation.shell_autonomous_mode = conversation_meta.shell_autonomous_mode;
        conversation.shell_work_mode = normalize_shell_work_mode_text(&conversation_meta.shell_work_mode);
        conversation.archived_at = conversation_meta.archived_at.clone();
        conversation.current_todos = conversation_meta.current_todos.clone();
        conversation.plan_mode_enabled = conversation_meta.plan_mode_enabled;
        conversation.preferred_api_config_id =
            conversation_meta.preferred_api_config_id.clone();
        conversation.auto_push_remote_contact_id =
            conversation_meta.auto_push_remote_contact_id.clone();
        conversation.cumulative_usage = conversation_meta.cumulative_usage.clone();
        conversation.active_goal = conversation_meta.active_goal.clone();
        conversation.fast_request_turns = conversation_meta.fast_request_turns.clone();
        conversation
    }

    fn set_conversation_unread_count_metadata(
        &self,
        state: &AppState,
        conversation_id: &str,
        unread_count: usize,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_set_conversation_unread_count_metadata",
            ConversationExternalMetadataPatch {
                unread_count: Some(unread_count),
                ..Default::default()
            },
        )
    }

    async fn append_message(
        &self,
        state: &AppState,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<(), String> {
        let state_for_mutation = state.clone();
        let conversation_id_for_mutation = conversation_id.to_string();
        let message_for_mutation = message.clone();
        with_conversation_mutation_async(
            state.clone(),
            conversation_id_for_mutation.clone(),
            "append_message".to_string(),
            move || {
                conversation_service_v2().append_message_locked(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                    &message_for_mutation,
                )
            },
        )
        .await?;
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=单消息写入后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
        Ok(())
    }

    fn append_message_locked(
        &self,
        state: &AppState,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<(), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let conversation_meta =
            self.get_conversation_meta(state, normalized_conversation_id)?;
        if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
            return Err(format!(
                "Unarchived conversation not found: {}",
                normalized_conversation_id
            ));
        }
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        runtime_log_info(format!(
            "[会话消息写入] 节点，任务=元数据读取完成，conversation_id={}",
            normalized_conversation_id
        ));
        let updated_at = message.created_at.clone();
        let last_user_at = if message.role.trim() == "user" {
            Some(message.created_at.clone())
        } else {
            conversation_meta.last_user_at.clone()
        };
        let last_assistant_at = if message.role.trim() == "assistant" {
            Some(message.created_at.clone())
        } else {
            conversation_meta.last_assistant_at.clone()
        };
        let unread_count = if self.conversation_has_active_chat_view(state, normalized_conversation_id)
            || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
        {
            0
        } else {
            conversation_meta.unread_count.saturating_add(1)
        };
        let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
            state,
            normalized_conversation_id,
            |cached| {
                let mut metadata_conversation =
                    self.build_conversation_snapshot_from_meta(cached, Vec::new());
                metadata_conversation.unread_count = unread_count;
                metadata_conversation.updated_at = updated_at.clone();
                metadata_conversation.last_user_at = last_user_at.clone();
                metadata_conversation.last_assistant_at = last_assistant_at.clone();
                cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                cached.apply_appended_messages(std::slice::from_ref(message));
                Ok(())
            },
        )?;
        runtime_log_info(format!(
            "[会话消息写入] 节点，任务=元数据缓存更新完成，conversation_id={}",
            normalized_conversation_id
        ));
        let metadata_conversation =
            self.build_conversation_snapshot_from_meta(&updated_meta, Vec::new());
        state_upsert_chat_index_conversation_cached(state, &metadata_conversation)?;
        let mut ready_meta = self
            .ensure_appendable_ready_message_store(state, normalized_conversation_id)?;
        ready_meta.apply_metadata_fields_from_meta(&updated_meta);
        ready_meta.apply_appended_messages(std::slice::from_ref(message));
        message_store::chat_store_append_messages_from_meta(
            &store_paths,
            &ready_meta,
            std::slice::from_ref(message),
        )?;
        runtime_log_info(format!(
            "[会话消息写入] 节点，任务=存储写入完成，conversation_id={}",
            normalized_conversation_id
        ));
        self.mark_conversation_metadata_cached_persisted(state, normalized_conversation_id)?;
        Ok(())
    }

    async fn append_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
        messages: &[ChatMessage],
    ) -> Result<(), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if messages.is_empty() {
            return Ok(());
        }
        let service = conversation_service_v2();
        let state_for_mutation = state.clone();
        let conversation_id_for_mutation = normalized_conversation_id.to_string();
        let messages_for_mutation = messages.to_vec();
        with_conversation_mutation_async(
            state.clone(),
            conversation_id_for_mutation.clone(),
            "append_messages".to_string(),
            move || {
                let conversation_meta = service
                    .get_conversation_meta(&state_for_mutation, &conversation_id_for_mutation)?;
                if !service.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Err(format!(
                        "Unarchived conversation not found: {}",
                        conversation_id_for_mutation
                    ));
                }
                let store_paths = message_store::message_store_paths(
                    &state_for_mutation.data_path,
                    &conversation_id_for_mutation,
                )?;
                let last_message = messages_for_mutation
                    .last()
                    .ok_or_else(|| "messages is empty".to_string())?;
                let updated_at = last_message.created_at.clone();
                let last_user_at = if last_message.role.trim() == "user" {
                    Some(last_message.created_at.clone())
                } else {
                    conversation_meta.last_user_at.clone()
                };
                let last_assistant_at = if last_message.role.trim() == "assistant" {
                    Some(last_message.created_at.clone())
                } else {
                    conversation_meta.last_assistant_at.clone()
                };
                let unread_count = if service.conversation_has_active_chat_view(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                ) || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
                {
                    0
                } else {
                    conversation_meta.unread_count.saturating_add(messages_for_mutation.len())
                };
                let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                    |cached| {
                        let mut metadata_conversation =
                            service.build_conversation_snapshot_from_meta(cached, Vec::new());
                        metadata_conversation.unread_count = unread_count;
                        metadata_conversation.updated_at = updated_at.clone();
                        metadata_conversation.last_user_at = last_user_at.clone();
                        metadata_conversation.last_assistant_at = last_assistant_at.clone();
                        cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                        cached.apply_appended_messages(&messages_for_mutation);
                        Ok(())
                    },
                )?;
                let metadata_conversation =
                    service.build_conversation_snapshot_from_meta(&updated_meta, Vec::new());
                state_upsert_chat_index_conversation_cached(&state_for_mutation, &metadata_conversation)?;
                let mut ready_meta = service.ensure_appendable_ready_message_store(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                ready_meta.apply_metadata_fields_from_meta(&updated_meta);
                ready_meta.apply_appended_messages(&messages_for_mutation);
                message_store::chat_store_append_messages_from_meta(
                    &store_paths,
                    &ready_meta,
                    &messages_for_mutation,
                )?;
                service.mark_conversation_metadata_cached_persisted(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                Ok(())
            },
        )
        .await?;
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            normalized_conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=批量消息写入后推送单会话，conversation_id={}，error={}",
                normalized_conversation_id, err
            ));
        }
        Ok(())
    }

    fn build_forward_selection_notification_message(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        selected_messages: &[ChatMessage],
    ) -> Result<ChatMessage, String> {
        let content = selected_messages_notification_content(selected_messages);
        let body = build_session_notification_body(state, source_conversation_id, &content)?;
        Ok(build_session_notification_message(&body))
    }

    async fn append_user_message(
        &self,
        state: &AppState,
        input: &UserMessageAppendInput,
    ) -> Result<(), String> {
        let conversation_id = input.conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if input.message.role.trim() != "user" {
            return Err("append_user_message 只允许 user message".to_string());
        }
        let memory_recall_ids = dedup_memory_recall_ids_v2(&input.memory_recall_ids);
        let service = conversation_service_v2();
        let state_for_mutation = state.clone();
        let conversation_id_for_mutation = conversation_id.to_string();
        let input_for_mutation = input.clone();
        let (was_draft, promoted_conversation) = with_conversation_mutation_async(
            state.clone(),
            conversation_id_for_mutation.clone(),
            "append_user_message".to_string(),
            move || {
                let conversation_meta =
                    service.get_conversation_meta(&state_for_mutation, &conversation_id_for_mutation)?;
                if !service.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Err(format!(
                        "Unarchived conversation not found: {conversation_id_for_mutation}"
                    ));
                }
                let was_draft = conversation_meta.is_draft;
                let store_paths =
                    message_store::message_store_paths(&state_for_mutation.data_path, &conversation_id_for_mutation)?;
                let updated_at = input_for_mutation.message.created_at.clone();
                let unread_count = if service
                    .conversation_has_active_chat_view(&state_for_mutation, &conversation_id_for_mutation)
                    || conversation_meta
                        .conversation_kind
                        .trim()
                        == CONVERSATION_KIND_REMOTE_IM_CONTACT
                {
                    0
                } else {
                    conversation_meta.unread_count.saturating_add(1)
                };
                let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                    |cached| {
                        let mut metadata_conversation =
                            service.build_conversation_snapshot_from_meta(cached, Vec::new());
                        // 任何用户消息写入都立刻转正：草稿收到第一条用户消息即清除草稿标记，
                        // 写回存储后 append 路径后续 emit 的 overview 水位线携带 isDraft=false。
                        if cached.is_draft() {
                            metadata_conversation.is_draft = false;
                        }
                        metadata_conversation.unread_count = unread_count;
                        metadata_conversation.updated_at = updated_at.clone();
                        metadata_conversation.last_user_at = Some(updated_at.clone());
                        if !memory_recall_ids.is_empty() {
                            for memory_id in &memory_recall_ids {
                                if !metadata_conversation
                                    .memory_recall_table
                                    .iter()
                                    .any(|item| item == memory_id)
                                {
                                    metadata_conversation.memory_recall_table.push(memory_id.clone());
                                }
                            }
                        }
                        cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                        cached.apply_appended_messages(std::slice::from_ref(
                            &input_for_mutation.message,
                        ));
                        Ok(())
                    },
                )?;
                let metadata_conversation =
                    service.build_conversation_snapshot_from_meta(&updated_meta, Vec::new());
                state_upsert_chat_index_conversation_cached(
                    &state_for_mutation,
                    &metadata_conversation,
                )?;
                let mut ready_meta =
                    service.ensure_appendable_ready_message_store(&state_for_mutation, &conversation_id_for_mutation)?;
                ready_meta.apply_metadata_fields_from_meta(&updated_meta);
                ready_meta.apply_appended_messages(std::slice::from_ref(
                    &input_for_mutation.message,
                ));
                message_store::chat_store_append_messages_from_meta(
                    &store_paths,
                    &ready_meta,
                    std::slice::from_ref(&input_for_mutation.message),
                )?;
                service.mark_conversation_metadata_cached_persisted(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                Ok((was_draft, metadata_conversation))
            },
        )
        .await?;
        // 草稿转正后立即创建下一个备用草稿：继承刚转正会话的人格/模型/workspace。
        // 创建失败不阻断消息发送，下次打开草稿入口时按单例查询兜底重建。
        if was_draft {
            match create_next_draft_conversation_inherited(state, &promoted_conversation) {
                Ok(new_draft_id) => runtime_log_info(format!(
                    "[会话草稿] 完成，任务=首条用户消息转正，conversation_id={}，new_draft_conversation_id={}",
                    conversation_id, new_draft_id
                )),
                Err(err) => runtime_log_warn(format!(
                    "[会话草稿] 创建备用草稿失败，等待下次打开时重建，conversation_id={}，error={err}",
                    conversation_id
                )),
            }
        }
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=用户消息写入后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
        Ok(())
    }

    /// 远程入站专用追加：只负责把远程消息正式写入会话历史。
    async fn append_remote_im_user_message(
        &self,
        state: &AppState,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<ChatMessage, String> {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("远程入站追加缺少 conversation_id".to_string());
        }
        if message.role.trim() != "user" {
            return Err("远程入站追加只允许 user message".to_string());
        }
        let service = conversation_service_v2();
        let state_for_mutation = state.clone();
        let conversation_id_for_mutation = conversation_id.to_string();
        let message_for_mutation = message.clone();
        with_conversation_mutation_async(
            state.clone(),
            conversation_id_for_mutation.clone(),
            "append_remote_im_user_message".to_string(),
            move || {
                let conversation_meta =
                    service.get_conversation_meta(&state_for_mutation, &conversation_id_for_mutation)?;
                if !service.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Err(format!(
                        "远程入站目标会话不存在：{conversation_id_for_mutation}"
                    ));
                }
                let store_paths = message_store::message_store_paths(
                    &state_for_mutation.data_path,
                    &conversation_id_for_mutation,
                )?;
                let updated_at = message_for_mutation.created_at.clone();
                let unread_count = if service.conversation_has_active_chat_view(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                ) || conversation_meta
                    .conversation_kind
                    .trim()
                    == CONVERSATION_KIND_REMOTE_IM_CONTACT
                {
                    0
                } else {
                    conversation_meta.unread_count.saturating_add(1)
                };
                let (updated_meta, (), _) = state_update_conversation_meta_cached_unlocked(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                    |cached| {
                        let mut metadata_conversation =
                            service.build_conversation_snapshot_from_meta(cached, Vec::new());
                        metadata_conversation.unread_count = unread_count;
                        metadata_conversation.updated_at = updated_at.clone();
                        metadata_conversation.last_user_at = Some(updated_at.clone());
                        cached.apply_metadata_fields_from_conversation(&metadata_conversation);
                        cached.apply_appended_messages(std::slice::from_ref(
                            &message_for_mutation,
                        ));
                        Ok(())
                    },
                )?;
                let metadata_conversation =
                    service.build_conversation_snapshot_from_meta(&updated_meta, Vec::new());
                state_upsert_chat_index_conversation_cached(
                    &state_for_mutation,
                    &metadata_conversation,
                )?;
                let mut ready_meta = service.ensure_appendable_ready_message_store(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                ready_meta.apply_metadata_fields_from_meta(&updated_meta);
                ready_meta.apply_appended_messages(std::slice::from_ref(&message_for_mutation));
                message_store::chat_store_append_messages_from_meta(
                    &store_paths,
                    &ready_meta,
                    std::slice::from_ref(&message_for_mutation),
                )?;
                service.mark_conversation_metadata_cached_persisted(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                Ok(())
            },
        )
        .await?;
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=远程入站直接写入后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
        emit_conversation_message_appended_event(state, conversation_id, message);
        Ok(message.clone())
    }

    fn increment_unread_count_if_background(
        &self,
        state: &AppState,
        conversation: &mut Conversation,
        count: usize,
    ) {
        self.increment_conversation_unread_count_if_background(
            state,
            conversation,
            count,
            false,
        )
    }

    fn enqueue_delegate_completion_notification(
        &self,
        state: &AppState,
        root_conversation_id: &str,
        target_agent_id: &str,
        delegate_title: &str,
        content: &str,
        action: &str,
    ) -> Result<(), String> {
        let resolved_target =
            self.resolve_delegate_result_target_conversation(state, root_conversation_id)?;
        let body = build_delegate_completion_notification_body(
            state,
            target_agent_id,
            delegate_title,
            content,
        )?;
        let message = build_session_notification_message(&body);
        enqueue_session_notification_dispatch(
            state,
            &resolved_target.target_conversation_id,
            &body,
            &message,
            action,
        )
    }

    fn enqueue_auto_push_remote_contact_message(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        remote_contact_id: &str,
        content: &str,
    ) -> Result<(), String> {
        let normalized_source_conversation_id = source_conversation_id.trim();
        let normalized_remote_contact_id = remote_contact_id.trim();
        if normalized_source_conversation_id.is_empty() || normalized_remote_contact_id.is_empty() {
            return Ok(());
        }
        let source_conversation_meta = self
            .get_conversation_meta(state, normalized_source_conversation_id)?;
        if !self.conversation_meta_is_local_normal_chat_meta_view(&source_conversation_meta)
            || !source_conversation_meta.visible_in_foreground_lists
            || self.conversation_meta_is_system_notification_meta_view(&source_conversation_meta)
        {
            runtime_log_warn(format!(
                "[自动推送] 跳过，任务=解析推送源会话，source_conversation_id={}，remote_contact_id={}，reason=source_conversation_not_eligible",
                normalized_source_conversation_id,
                normalized_remote_contact_id
            ));
            return Ok(());
        }
        runtime_log_info(format!(
            "[自动推送] 开始，任务=解析远程联系人通知目标，source_conversation_id={}，remote_contact_id={}",
            normalized_source_conversation_id,
            normalized_remote_contact_id
        ));
        let target_conversation_id =
            self.resolve_remote_im_contact_conversation_id_for_notification(
                state,
                normalized_remote_contact_id,
            )?;
        let body =
            build_session_notification_body(state, normalized_source_conversation_id, content)?;
        let message = build_session_notification_message(&body);
        runtime_log_info(format!(
            "[自动推送] 开始，任务=通知转发入队，source_conversation_id={}，target_conversation_id={}，remote_contact_id={}，message_id={}",
            normalized_source_conversation_id,
            target_conversation_id,
            normalized_remote_contact_id,
            message.id
        ));
        enqueue_session_notification_dispatch(
            state,
            &target_conversation_id,
            &body,
            &message,
            "auto_push_session",
        )?;
        runtime_log_info(format!(
            "[自动推送] 完成，任务=通知转发入队，source_conversation_id={}，target_conversation_id={}，remote_contact_id={}，message_id={}",
            normalized_source_conversation_id,
            target_conversation_id,
            normalized_remote_contact_id,
            message.id
        ));
        Ok(())
    }

    fn import_conversation_snapshot(
        &self,
        state: &AppState,
        job_id: &str,
        operator: &str,
        reason: &str,
        snapshot: &Conversation,
    ) -> Result<(), String> {
        self.apply_privileged_snapshot_overwrite(
            state,
            &ConversationOverwriteAudit {
                job_id: job_id.trim().to_string(),
                source: ConversationOverwriteSource::Import,
                operator: operator.trim().to_string(),
                reason: reason.trim().to_string(),
            },
            snapshot,
        )
    }

    #[cfg(test)]
    fn sync_replace_conversation_snapshot(
        &self,
        state: &AppState,
        job_id: &str,
        operator: &str,
        reason: &str,
        snapshot: &Conversation,
    ) -> Result<(), String> {
        self.apply_privileged_snapshot_overwrite(
            state,
            &ConversationOverwriteAudit {
                job_id: job_id.trim().to_string(),
                source: ConversationOverwriteSource::ExportSync,
                operator: operator.trim().to_string(),
                reason: reason.trim().to_string(),
            },
            snapshot,
        )
    }

    #[cfg(test)]
    fn set_conversation_preferred_api_config_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        preferred_api_config_id: Option<String>,
    ) -> Result<Conversation, String> {
        self.set_preferred_api_config_id(state, conversation_id, preferred_api_config_id)
    }

    #[cfg(test)]
    fn set_conversation_auto_push_remote_contact_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        auto_push_remote_contact_id: Option<String>,
    ) -> Result<Conversation, String> {
        self.set_auto_push_remote_contact_id(state, conversation_id, auto_push_remote_contact_id)
    }

    #[cfg(test)]
    fn read_foreground_snapshot(
        &self,
        state: &AppState,
        conversation_id: Option<&str>,
        agent_id: Option<&str>,
        recent_limit: usize,
    ) -> Result<ForegroundConversationSnapshotCore, String> {
        self.get_foreground_snapshot(state, conversation_id, agent_id, recent_limit)
    }

    #[cfg(test)]
    fn set_conversation_title_metadata(
        &self,
        state: &AppState,
        conversation_id: &str,
        next_title: &str,
    ) -> Result<Conversation, String> {
        self.set_title(state, conversation_id, next_title)
    }

    #[cfg(test)]
    fn set_conversation_shell_workspace_metadata(
        &self,
        state: &AppState,
        conversation_id: &str,
        shell_workspace_path: Option<Option<String>>,
        shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
        shell_autonomous_mode: Option<bool>,
        shell_work_mode: Option<String>,
    ) -> Result<Conversation, String> {
        self.set_shell_workspace(
            state,
            conversation_id,
            shell_workspace_path,
            shell_workspaces,
            shell_autonomous_mode,
            shell_work_mode,
            None,
        )
    }

    #[cfg(test)]
    fn set_conversation_lifecycle_metadata(
        &self,
        state: &AppState,
        conversation_id: &str,
        status: Option<&str>,
        archived_at: Option<Option<String>>,
        updated_at: Option<String>,
    ) -> Result<Conversation, String> {
        self.apply_external_metadata_patch(
            state,
            conversation_id,
            "conversation_v2_test_set_lifecycle_metadata",
            ConversationExternalMetadataPatch {
                lifecycle_status: status.map(|value| value.trim().to_string()),
                lifecycle_archived_at: archived_at,
                lifecycle_updated_at: updated_at,
                ..Default::default()
            },
        )
    }

    #[cfg(test)]
    fn set_conversation_current_todos_metadata(
        &self,
        state: &AppState,
        conversation_id: &str,
        current_todos: Vec<ConversationTodoItem>,
    ) -> Result<Conversation, String> {
        self.set_current_todos(state, conversation_id, current_todos)
    }

    #[cfg(test)]
    async fn append_message_to_unarchived_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> Result<(), String> {
        self.append_message(state, conversation_id, message).await
    }

    #[cfg(test)]
    fn read_message_by_id(
        &self,
        state: &AppState,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<ChatMessage, String> {
        self.get_message_by_id_for_frontend_display_only(state, conversation_id, message_id)
    }

    #[cfg(test)]
    fn read_archive_block_page(
        &self,
        state: &AppState,
        archive_id: &str,
        block_id: Option<u32>,
    ) -> Result<ConversationBlockPageResult, String> {
        self.get_archive_block_page(state, archive_id, block_id)
    }

    #[cfg(test)]
    fn read_unarchived_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        self.get_all_messages(state, conversation_id)
    }

    #[cfg(test)]
    fn rewind_conversation_from_message(
        &self,
        state: &AppState,
        input: &RewindConversationInput,
        message_id: &str,
        started_at: &std::time::Instant,
    ) -> Result<RewindConversationMutationResult, String> {
        self.rewind_conversation(state, input, message_id, started_at)
    }

}
