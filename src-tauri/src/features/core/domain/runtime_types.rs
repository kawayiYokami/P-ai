#[derive(Clone)]
struct CodexRuntimeAuth {
    provider_id: String,
    auth_mode: String,
    local_auth_path: String,
    access_token: String,
    refresh_token: Option<String>,
    account_id: Option<String>,
    email: Option<String>,
    expires_at_ms: Option<i64>,
}

impl std::fmt::Debug for CodexRuntimeAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexRuntimeAuth")
            .field("provider_id", &self.provider_id)
            .field("auth_mode", &self.auth_mode)
            .field("local_auth_path", &self.local_auth_path)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("account_id", &self.account_id.as_ref().map(|_| "<redacted>"))
            .field("email", &self.email.as_ref().map(|_| "<redacted>"))
            .field("expires_at_ms", &self.expires_at_ms.map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Debug, Clone)]
struct ResolvedApiConfig {
    login_provider: String,
    provider_id: Option<String>,
    provider_api_keys: Vec<String>,
    provider_key_cursor: usize,
    request_format: RequestFormat,
    allow_concurrent_requests: bool,
    max_concurrent_requests: Option<u32>,
    base_url: String,
    api_key: String,
    model: String,
    reasoning_effort: Option<String>,
    temperature: Option<f64>,
    max_output_tokens: Option<u32>,
    prompt_cache_key: Option<String>,
    extra_headers: Vec<(String, String)>,
    codex_auth: Option<CodexRuntimeAuth>,
    codex_custom_api_key: Option<String>,
    codex_originator: Option<String>,
    codex_residency_requirement: Option<String>,
}

struct ProviderRequestGate {
    limit: usize,
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
}

#[derive(Debug, Clone)]
struct PreparedBinaryPayload {
    mime: String,
    content: String,
    saved_path: Option<String>,
    label: String,
}

#[derive(Debug, Clone)]
struct PreparedHistoryMessage {
    role: String,
    text: String,
    extra_text_blocks: Vec<String>,
    user_time_text: Option<String>,
    images: Vec<PreparedBinaryPayload>,
    audios: Vec<PreparedBinaryPayload>,
    tool_calls: Option<Vec<Value>>,
    tool_call_id: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedPrompt {
    preamble: String,
    history_messages: Vec<PreparedHistoryMessage>,
    latest_user_text: String,
    latest_user_meta_text: String,
    latest_user_extra_text: String,
    latest_user_extra_blocks: Vec<String>,
    latest_user_trailing_blocks: Vec<String>,
    latest_images: Vec<PreparedBinaryPayload>,
    latest_audios: Vec<PreparedBinaryPayload>,
}

#[derive(Debug, Clone)]
struct PendingConversationPersist {
    seq: u64,
    conversations: std::collections::HashMap<String, Conversation>,
    metadata_conversation_ids: std::collections::HashSet<String>,
    deleted_conversation_ids: std::collections::HashSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ConversationDirCacheSignature {
    file_count: u64,
    total_size: u64,
    latest_file_name: String,
    latest_modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AppDataCacheSignature {
    agents_len: u64,
    agents_modified: Option<std::time::SystemTime>,
    runtime_len: u64,
    runtime_modified: Option<std::time::SystemTime>,
    conversations: ConversationDirCacheSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImPresenceState {
    Away,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImWorkState {
    Idle,
    Busy,
}

#[derive(Debug, Clone)]
struct RemoteImReplyDelegateRuntime {
    delegate_id: String,
    contact_id: String,
    conversation_id: String,
    trigger_message_id: String,
    started_at: String,
    /// 委托启动瞬间冻结的在场 block 快照；之后绝不从全局当前 block 重读。
    prompt_snapshot_messages: Vec<ChatMessage>,
    guidance_messages: std::collections::VecDeque<ChatMessage>,
    /// 已消费的引导会累积到后续委托轮次的私有提示词中，但不写回初始快照。
    consumed_guidance_messages: Vec<ChatMessage>,
    /// 取消后禁止排队任务或已返回的模型结果继续写入联系人会话。
    cancelled: bool,
    /// 已开始终结，不再接收秘书引导。
    terminal: bool,
    session_agent_id: String,
    inspection_generation: Option<u64>,
    group_reply_focus: bool,
    group_reply_max_chars: Option<u32>,
}

#[derive(Debug, Clone)]
struct RemoteImContactRuntimeState {
    presence_state: RemoteImPresenceState,
    last_presence_at: Option<String>,
    // 旧的 Busy / has_pending 仅为兼容旧调度收尾路径保留；远程应答委托不使用它们排队。
    work_state: RemoteImWorkState,
    has_pending: bool,
    last_success_reply_at: Option<String>,
    mute_until: Option<String>,
    consecutive_no_reply_count: u32,
}

impl Default for RemoteImContactRuntimeState {
    fn default() -> Self {
        Self {
            presence_state: RemoteImPresenceState::Away,
            last_presence_at: None,
            work_state: RemoteImWorkState::Idle,
            has_pending: false,
            last_success_reply_at: None,
            mute_until: None,
            consecutive_no_reply_count: 0,
        }
    }
}

fn normalize_prepared_prompt_extra_blocks(blocks: &[String]) -> Vec<String> {
    blocks
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn prepared_prompt_latest_user_extra_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    let normalized = normalize_prepared_prompt_extra_blocks(&prepared.latest_user_extra_blocks);
    if !normalized.is_empty() {
        return normalized;
    }
    let fallback = prepared.latest_user_extra_text.trim();
    if fallback.is_empty() {
        Vec::new()
    } else {
        vec![fallback.to_string()]
    }
}

fn prepared_prompt_set_latest_user_extra_blocks(
    prepared: &mut PreparedPrompt,
    blocks: Vec<String>,
) {
    let normalized = normalize_prepared_prompt_extra_blocks(&blocks);
    prepared.latest_user_extra_text = normalized.join("\n\n");
    prepared.latest_user_extra_blocks = normalized;
}

fn prepared_prompt_append_latest_user_extra_blocks(
    prepared: &mut PreparedPrompt,
    blocks: &[String],
) {
    let mut merged = prepared_prompt_latest_user_extra_blocks(prepared);
    merged.extend(normalize_prepared_prompt_extra_blocks(blocks));
    prepared_prompt_set_latest_user_extra_blocks(prepared, merged);
}

fn prepared_prompt_append_latest_user_extra_block(
    prepared: &mut PreparedPrompt,
    block: impl AsRef<str>,
) {
    let trimmed = block.as_ref().trim();
    if trimmed.is_empty() {
        return;
    }
    prepared_prompt_append_latest_user_extra_blocks(prepared, &[trimmed.to_string()]);
}

fn prepared_prompt_latest_user_trailing_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    normalize_prepared_prompt_extra_blocks(&prepared.latest_user_trailing_blocks)
}

fn prepared_prompt_set_latest_user_trailing_blocks(
    prepared: &mut PreparedPrompt,
    blocks: Vec<String>,
) {
    prepared.latest_user_trailing_blocks = normalize_prepared_prompt_extra_blocks(&blocks);
}

fn prepared_prompt_append_latest_user_trailing_blocks(
    prepared: &mut PreparedPrompt,
    blocks: &[String],
) {
    let mut merged = prepared_prompt_latest_user_trailing_blocks(prepared);
    merged.extend(normalize_prepared_prompt_extra_blocks(blocks));
    prepared_prompt_set_latest_user_trailing_blocks(prepared, merged);
}

// 前置提醒统一收进一个包裹块，整块排在用户正文之前。
fn build_system_reminder_block(blocks: &[String]) -> Option<String> {
    let normalized = normalize_prepared_prompt_extra_blocks(blocks);
    if normalized.is_empty() {
        return None;
    }
    Some(format!(
        "<system-reminder>\n{}\n</system-reminder>",
        normalized.join("\n")
    ))
}

fn prepared_prompt_latest_user_text_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    if let Some(reminder) =
        build_system_reminder_block(&prepared_prompt_latest_user_extra_blocks(prepared))
    {
        blocks.push(reminder);
    }
    for text in [
        prepared.latest_user_meta_text.trim(),
        prepared.latest_user_text.trim(),
    ] {
        if !text.is_empty() {
            blocks.push(text.to_string());
        }
    }
    blocks.extend(prepared_prompt_latest_user_trailing_blocks(prepared));
    blocks
}

// 历史消息唯一拼装入口：时间、正文、附加块按存量直排。真实请求、预览、快速文本共用此函数，不允许分叉。
fn prepared_history_user_text_blocks(message: &PreparedHistoryMessage) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    if let Some(time_text) = &message.user_time_text {
        if !time_text.trim().is_empty() {
            blocks.push(time_text.clone());
        }
    }
    if !message.text.trim().is_empty() {
        blocks.push(message.text.clone());
    }
    for block in &message.extra_text_blocks {
        if !block.trim().is_empty() {
            blocks.push(block.clone());
        }
    }
    blocks
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSettings {
    #[serde(alias = "selectedAgentId", alias = "selected_agent_id")]
    assistant_agent_id: String,
    user_alias: String,
    #[serde(default = "default_response_style_id")]
    response_style_id: String,
    #[serde(default = "default_pdf_read_mode")]
    pdf_read_mode: String,
    #[serde(default = "default_background_voice_screenshot_keywords")]
    background_voice_screenshot_keywords: String,
    #[serde(default = "default_background_voice_screenshot_mode")]
    background_voice_screenshot_mode: String,
    #[serde(default)]
    instruction_presets: Vec<PromptCommandPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppBootstrapSnapshot {
    config: AppConfig,
    agents: Vec<AgentProfile>,
    chat_settings: ChatSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationApiSettings {
    #[serde(alias = "chatApiConfigId", alias = "chat_api_config_id")]
    expert_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    tool_review_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
}
