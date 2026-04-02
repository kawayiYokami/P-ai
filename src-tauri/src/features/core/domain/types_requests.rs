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
    images: Option<Vec<BinaryPart>>,
    audios: Option<Vec<BinaryPart>>,
    #[serde(default)]
    attachments: Option<Vec<AttachmentMetaInput>>,
    model: Option<String>,
    #[serde(default)]
    extra_text_blocks: Option<Vec<String>>,
    #[serde(default)]
    provider_meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentMetaInput {
    file_name: String,
    relative_path: String,
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
    partial_reasoning_standard: String,
    #[serde(default)]
    partial_reasoning_inline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendChatResult {
    conversation_id: String,
    latest_user_text: String,
    assistant_text: String,
    reasoning_standard: String,
    reasoning_inline: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopChatResult {
    aborted: bool,
    persisted: bool,
    conversation_id: Option<String>,
    #[serde(default)]
    assistant_text: String,
    #[serde(default)]
    reasoning_standard: String,
    #[serde(default)]
    reasoning_inline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_message: Option<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionSelector {
    api_config_id: Option<String>,
    #[serde(default)]
    department_id: Option<String>,
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
    executor_department_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_reason: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshModelsInput {
    base_url: String,
    api_key: String,
    request_format: RequestFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchModelMetadataInput {
    request_format: RequestFormat,
    model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchModelMetadataOutput {
    found: bool,
    matched_model_id: Option<String>,
    context_window_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
    enable_image: Option<bool>,
    enable_tools: Option<bool>,
    enable_audio: Option<bool>,
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
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_args: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Clone)]
struct ActiveChatViewBinding {
    conversation_id: String,
}
