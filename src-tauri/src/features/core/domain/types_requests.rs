#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinaryPart {
    mime: String,
    bytes_base64: String,
    #[serde(default)]
    saved_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatInputPayload {
    text: Option<String>,
    #[serde(default)]
    display_text: Option<String>,
    #[serde(default)]
    parts: Option<Vec<ChatIngressPart>>,
    images: Option<Vec<BinaryPart>>,
    audios: Option<Vec<BinaryPart>>,
    #[serde(default)]
    attachments: Option<Vec<AttachmentMetaInput>>,
    model: Option<String>,
    #[serde(default)]
    extra_text_blocks: Option<Vec<String>>,
    #[serde(default)]
    mentions: Option<Vec<UserMentionTargetInput>>,
    #[serde(default)]
    provider_meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
enum ChatIngressPart {
    Text {
        text: String,
    },
    Attachment {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        bytes_base64: Option<String>,
        #[serde(default)]
        mime: String,
        #[serde(default)]
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserMentionTargetInput {
    agent_id: String,
    #[serde(default)]
    agent_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentMetaInput {
    file_name: String,
    #[serde(default, alias = "relativePath")]
    path: String,
    #[serde(default)]
    mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendChatRequest {
    payload: ChatInputPayload,
    #[serde(default)]
    session: Option<SessionSelector>,
    #[serde(default)]
    speaker_agent_id: Option<String>,
    #[serde(default)]
    trace_id: Option<String>,
    #[serde(default)]
    assistant_message_id: Option<String>,
    #[serde(default)]
    oldest_queue_created_at: Option<String>,
    #[serde(default)]
    remote_im_activation_sources: Vec<RemoteImActivationSource>,
    #[serde(default)]
    runtime_context: Option<RuntimeContext>,
    #[serde(default)]
    trigger_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopChatRequest {
    session: SessionSelector,
    #[serde(default)]
    partial_assistant_text: String,
    #[serde(default)]
    partial_stream_blocks: Vec<AssistantStreamBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImReplyTarget {
    channel_id: String,
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitChatResult {
    accepted: bool,
    duplicate: bool,
    event_id: String,
    conversation_id: String,
    trace_id: String,
    ingress: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendChatResult {
    conversation_id: String,
    latest_user_text: String,
    assistant_text: String,
    #[serde(default)]
    final_response_text: String,
    archived_before_send: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_prompt_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_window_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_usage_percent: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_im_reply_decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_im_reply_target: Option<RemoteImReplyTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Value>,
    /// 本轮调度实际使用的激活标识：压缩重开会换新标识，收尾事件必须与轮次开始事件一致。
    #[serde(skip_serializing_if = "Option::is_none")]
    activation_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopChatResult {
    aborted: bool,
    persisted: bool,
    conversation_id: Option<String>,
    #[serde(default)]
    assistant_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionSelector {
    api_config_id: Option<String>,
    agent_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RuntimeContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin_conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    executor_agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trusted_prompt_usage: Option<TrustedPromptUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bound_remote_im_activation_source: Option<RemoteImActivationSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote_im_reply_delegate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote_im_reply_trigger_message_id: Option<String>,
    /// 仅在进程内传递，不能序列化进任务或持久化状态。
    #[serde(skip)]
    remote_im_reply_prompt_snapshot_messages: Option<Vec<ChatMessage>>,
    /// 压缩保留消息：仅进程内传递；压缩完成后置为 ready，新调度 bootstrap 才能消费。
    #[serde(skip)]
    compaction_preserved_messages: Option<CompactionPreservedMessages>,
    #[serde(skip)]
    compaction_preserved_messages_ready: bool,
    #[serde(default)]
    remote_im_dynamic_boundary: bool,
    /// 远程应答委托多轮执行时，禁止 core_send 在每一轮结束后立即外发。
    #[serde(default)]
    remote_im_defer_auto_send: bool,
}

/// 压缩保留消息：一轮已完成但未写入旧段的 assistant 正文/思维链/工具事件。
#[derive(Debug, Clone, PartialEq)]
struct CompactionPreservedMessages {
    assistant_text: String,
    activity_reasoning_text: String,
    tool_history_events: Vec<Value>,
}

impl CompactionPreservedMessages {
    fn new(
        assistant_text: impl Into<String>,
        activity_reasoning_text: impl Into<String>,
        tool_history_events: Vec<Value>,
    ) -> Self {
        Self {
            assistant_text: assistant_text.into(),
            activity_reasoning_text: activity_reasoning_text.into(),
            tool_history_events,
        }
    }

    /// 复用现有 `estimated_tokens_for_text`，只估本组消息本身。
    fn token_usage(&self) -> u64 {
        let mut total = 0.0f64;
        total += estimated_tokens_for_text(self.assistant_text.trim());
        // 与 prepare 估算一致：reasoning 不计入 prompt 输入。
        for event in &self.tool_history_events {
            let role = event
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if role.eq_ignore_ascii_case("assistant") {
                if let Some(content) = event.get("content").and_then(Value::as_str) {
                    total += estimated_tokens_for_text(content);
                }
                if let Some(calls) = event.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        if let Some(function) = call.get("function") {
                            if let Some(name) = function.get("name").and_then(Value::as_str) {
                                total += estimated_tokens_for_text(name);
                            }
                            if let Some(arguments) =
                                function.get("arguments").and_then(Value::as_str)
                            {
                                total += estimated_tokens_for_text(arguments);
                            }
                        }
                    }
                }
            } else if role.eq_ignore_ascii_case("tool") {
                if let Some(content) = event.get("content").and_then(Value::as_str) {
                    total += estimated_tokens_for_text(content);
                }
            } else if let Some(content) = event.get("content").and_then(Value::as_str) {
                total += estimated_tokens_for_text(content);
            }
            total += 4.0;
        }
        total.ceil().max(0.0).min(u64::MAX as f64) as u64
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TrustedPromptUsage {
    effective_prompt_tokens: u64,
    context_usage_ratio: f64,
    #[serde(default)]
    estimated: bool,
}

fn runtime_context_trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn runtime_context_new(event_source: &str, dispatch_reason: &str) -> RuntimeContext {
    RuntimeContext {
        event_source: runtime_context_trimmed(Some(event_source)),
        dispatch_reason: runtime_context_trimmed(Some(dispatch_reason)),
        ..RuntimeContext::default()
    }
}

fn runtime_context_request_id_or_new(
    runtime_context: Option<&RuntimeContext>,
    trace_id: Option<&str>,
    prefix: &str,
) -> String {
    runtime_context
        .and_then(|value| value.request_id.as_deref())
        .or(trace_id)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{}-{}", prefix.trim(), Uuid::new_v4()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSnapshot {
    conversation_id: String,
    latest_user: Option<ChatMessage>,
    latest_assistant: Option<ChatMessage>,
    active_message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptPreview {
    preamble: String,
    latest_user_text: String,
    latest_images: usize,
    latest_audios: usize,
    request_body_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemPromptPreview {
    system_prompt: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshModelsInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
    #[serde(default)]
    provider_id: Option<String>,
    #[serde(default = "default_codex_auth_mode")]
    codex_auth_mode: String,
    #[serde(default = "default_codex_local_auth_path")]
    codex_local_auth_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuickGenaiChatInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
    model: String,
    prompt: String,
    #[serde(default)]
    provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchModelMetadataInput {
    request_format: RequestFormat,
    model: String,
    #[serde(default)]
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchModelMetadataOutput {
    found: bool,
    fuzzy_match: bool,
    provider_name: Option<String>,
    provider_api: Option<String>,
    matched_model_id: Option<String>,
    context_window_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
    enable_image: Option<bool>,
    enable_tools: Option<bool>,
    enable_audio: Option<bool>,
    enable_video: Option<bool>,
    reasoning: Option<bool>,
    #[serde(default)]
    reasoning_effort_options: Vec<String>,
    documentation_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestEmbeddingConnectionInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
    model: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestEmbeddingConnectionResult {
    vector_dim: usize,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestRerankConnectionInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
    model: String,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    documents: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestRerankConnectionResult {
    result_count: usize,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestVoiceConnectionInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestVoiceConnectionResult {
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckToolsStatusInput {
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolLoadStatus {
    id: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionCatalogItem {
    name: String,
    description: String,
    #[serde(default)]
    group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionCatalog {
    builtin_tools: Vec<PermissionCatalogItem>,
    skills: Vec<PermissionCatalogItem>,
    mcp_tools: Vec<PermissionCatalogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendToolFunctionDefinition {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendToolDefinition {
    #[serde(rename = "type")]
    kind: String,
    function: FrontendToolFunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageTextCacheStats {
    entries: usize,
    total_chars: usize,
    latest_updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIModelListItem {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIModelListResponse {
    data: Vec<OpenAIModelListItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiNativeModelListItem {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiNativeModelListResponse {
    #[serde(default)]
    models: Vec<GeminiNativeModelListItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicModelListItem {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AnthropicModelListResponse {
    #[serde(default)]
    data: Vec<AnthropicModelListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssistantDeltaEvent {
    delta: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default, skip_deserializing)]
    stream_cache: Option<ConversationStreamRuntimeCacheSnapshot>,
}

fn round_completed_delta_event(
    conversation_id: &str,
    request_id: Option<&str>,
    assistant_text: &str,
    assistant_message: Option<&ChatMessage>,
) -> AssistantDeltaEvent {
    let normalized_request_id = request_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let message = serde_json::json!({
        "conversationId": conversation_id.trim(),
        "activationId": normalized_request_id,
        "requestId": normalized_request_id,
        "assistantText": assistant_text,
        "archivedBeforeSend": false,
        "assistantMessage": assistant_message,
    })
    .to_string();
    AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("round_completed".to_string()),
        request_id: normalized_request_id.clone(),
        activation_id: normalized_request_id,
        phase_id: None,
        reason: Some("context_compaction_boundary".to_string()),
        tool_name: None,
        tool_call_id: None,
        tool_status: None,
        tool_args: None,
        message: Some(message),
        stream_cache: None,
    }
}

#[derive(Clone)]
struct ActiveChatViewBinding {
    window_label: String,
    binding_id: String,
    conversation_id: String,
    delta_channel: tauri::ipc::Channel<AssistantDeltaEvent>,
}

#[derive(Debug, Clone)]
struct ConversationListActivityMark {
    activity: String,
    failed_message: Option<String>,
    completed_at: Option<String>,
}

#[cfg(test)]
mod compaction_preserved_messages_tests {
    use super::*;

    #[test]
    fn compaction_preserved_messages_token_usage_should_be_stable() {
        let group = CompactionPreservedMessages::new(
            "hello",
            "think",
            vec![
                serde_json::json!({
                    "role":"assistant",
                    "content": null,
                    "tool_calls":[{"id":"c1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"a\"}"}}]
                }),
                serde_json::json!({"role":"tool","tool_call_id":"c1","content":"body"}),
            ],
        );
        let a = group.token_usage();
        let b = group.token_usage();
        assert_eq!(a, b);
        assert!(a > 0);
    }
}
