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
    provider_id: Option<String>,
    provider_api_keys: Vec<String>,
    provider_key_cursor: usize,
    request_format: RequestFormat,
    base_url: String,
    api_key: String,
    model: String,
    reasoning_effort: Option<String>,
    temperature: Option<f64>,
    max_output_tokens: Option<u32>,
    extra_headers: Vec<(String, String)>,
    codex_auth: Option<CodexRuntimeAuth>,
}

#[derive(Debug, Clone)]
struct PreparedBinaryPayload {
    mime: String,
    content: String,
    saved_path: Option<String>,
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
    latest_images: Vec<PreparedBinaryPayload>,
    latest_audios: Vec<PreparedBinaryPayload>,
}

#[derive(Debug, Clone)]
struct PendingAppDataPersist {
    seq: u64,
    data: AppData,
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

fn prepared_prompt_prepend_latest_user_extra_block(
    prepared: &mut PreparedPrompt,
    block: impl AsRef<str>,
) {
    let trimmed = block.as_ref().trim();
    if trimmed.is_empty() {
        return;
    }
    let mut merged = vec![trimmed.to_string()];
    merged.extend(prepared_prompt_latest_user_extra_blocks(prepared));
    prepared_prompt_set_latest_user_extra_blocks(prepared, merged);
}

fn prepared_prompt_latest_user_text_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    for text in [
        prepared.latest_user_text.trim(),
        prepared.latest_user_meta_text.trim(),
    ] {
        if !text.is_empty() {
            blocks.push(text.to_string());
        }
    }
    blocks.extend(prepared_prompt_latest_user_extra_blocks(prepared));
    if blocks.is_empty() {
        blocks.push(" ".to_string());
    }
    blocks
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSettings {
    #[serde(alias = "selectedAgentId", alias = "selected_agent_id")]
    assistant_department_agent_id: String,
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
    assistant_department_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
}
