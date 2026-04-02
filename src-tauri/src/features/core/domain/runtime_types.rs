#[derive(Debug, Clone)]
struct ResolvedApiConfig {
    request_format: RequestFormat,
    base_url: String,
    api_key: String,
    model: String,
    temperature: Option<f64>,
    max_output_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
struct PreparedHistoryMessage {
    role: String,
    text: String,
    extra_text_blocks: Vec<String>,
    user_time_text: Option<String>,
    images: Vec<(String, String)>,
    audios: Vec<(String, String)>,
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
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct PendingAppDataPersist {
    seq: u64,
    data: AppData,
}

fn prepared_prompt_latest_user_text_blocks(prepared: &PreparedPrompt) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    for text in [
        prepared.latest_user_text.trim(),
        prepared.latest_user_meta_text.trim(),
        prepared.latest_user_extra_text.trim(),
    ] {
        if !text.is_empty() {
            blocks.push(text.to_string());
        }
    }
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
