use std::{fs, io::Cursor, path::PathBuf, sync::Mutex};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use futures_util::StreamExt;
use image::ImageFormat;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rig::{
    completion::{
        message::{AudioMediaType, ImageDetail, ImageMediaType, UserContent},
        Message as RigMessage, Prompt,
    },
    prelude::CompletionClient,
    providers::openai,
    OneOrMany,
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, Position, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const APP_DATA_SCHEMA_VERSION: u32 = 1;
const ARCHIVE_IDLE_SECONDS: i64 = 30 * 60;
const MAX_MULTIMODAL_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiToolConfig {
    id: String,
    command: String,
    args: Vec<String>,
    #[serde(default)]
    values: Value,
}

fn default_false() -> bool {
    false
}

fn default_api_tools() -> Vec<ApiToolConfig> {
    vec![
        ApiToolConfig {
            id: "fetch".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@iflow-mcp/fetch".to_string()],
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "bing-search".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "bing-cn-mcp".to_string()],
            values: serde_json::json!({}),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiConfig {
    id: String,
    name: String,
    request_format: String,
    #[serde(default = "default_true")]
    enable_text: bool,
    #[serde(default = "default_true")]
    enable_image: bool,
    #[serde(default = "default_true")]
    enable_audio: bool,
    #[serde(default = "default_false")]
    enable_tools: bool,
    #[serde(default = "default_api_tools")]
    tools: Vec<ApiToolConfig>,
    base_url: String,
    api_key: String,
    model: String,
}

fn default_true() -> bool {
    true
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            id: "default-openai".to_string(),
            name: "Default OpenAI".to_string(),
            request_format: "openai".to_string(),
            enable_text: true,
            enable_image: true,
            enable_audio: true,
            enable_tools: false,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    hotkey: String,
    selected_api_config_id: String,
    #[serde(default)]
    chat_api_config_id: String,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    api_configs: Vec<ApiConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let api_config = ApiConfig::default();
        Self {
            hotkey: "Alt+·".to_string(),
            selected_api_config_id: api_config.id.clone(),
            chat_api_config_id: api_config.id.clone(),
            stt_api_config_id: None,
            vision_api_config_id: None,
            api_configs: vec![api_config],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugApiConfig {
    request_format: Option<String>,
    base_url: String,
    api_key: String,
    model: String,
    fixed_test_prompt: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinaryPart {
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatInputPayload {
    text: Option<String>,
    images: Option<Vec<BinaryPart>>,
    audios: Option<Vec<BinaryPart>>,
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendChatRequest {
    api_config_id: Option<String>,
    agent_id: String,
    payload: ChatInputPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendChatResult {
    conversation_id: String,
    latest_user_text: String,
    assistant_text: String,
    archived_before_send: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionSelector {
    api_config_id: Option<String>,
    agent_id: String,
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
struct RefreshModelsInput {
    base_url: String,
    api_key: String,
    request_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckToolsStatusInput {
    api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolLoadStatus {
    id: String,
    status: String,
    detail: String,
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
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIStreamDelta {
    content: Option<Value>,
    tool_calls: Option<Vec<OpenAIStreamToolCallDelta>>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIStreamToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<OpenAIStreamToolCallFnDelta>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIStreamToolCallFnDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIToolCall {
    id: String,
    function: OpenAIToolCallFunction,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssistantDeltaEvent {
    delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfile {
    id: String,
    name: String,
    system_prompt: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveAgentsInput {
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum MessagePart {
    Text {
        text: String,
    },
    Image {
        mime: String,
        bytes_base64: String,
        name: Option<String>,
        compressed: bool,
    },
    Audio {
        mime: String,
        bytes_base64: String,
        name: Option<String>,
        compressed: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    id: String,
    role: String,
    created_at: String,
    parts: Vec<MessagePart>,
    provider_meta: Option<Value>,
    tool_call: Option<Vec<Value>>,
    mcp_call: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Conversation {
    id: String,
    title: String,
    api_config_id: String,
    agent_id: String,
    created_at: String,
    updated_at: String,
    last_assistant_at: Option<String>,
    status: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationArchive {
    archive_id: String,
    archived_at: String,
    reason: String,
    source_conversation: Conversation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveSummary {
    archive_id: String,
    archived_at: String,
    title: String,
    message_count: usize,
    api_config_id: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppData {
    version: u32,
    agents: Vec<AgentProfile>,
    #[serde(default = "default_selected_agent_id")]
    selected_agent_id: String,
    #[serde(default = "default_user_alias")]
    user_alias: String,
    conversations: Vec<Conversation>,
    archived_conversations: Vec<ConversationArchive>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: APP_DATA_SCHEMA_VERSION,
            agents: vec![default_agent()],
            selected_agent_id: default_selected_agent_id(),
            user_alias: default_user_alias(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
        }
    }
}

fn default_selected_agent_id() -> String {
    "default-agent".to_string()
}

fn default_user_alias() -> String {
    "用户".to_string()
}

#[derive(Debug, Clone)]
struct ResolvedApiConfig {
    request_format: String,
    base_url: String,
    api_key: String,
    model: String,
    fixed_test_prompt: String,
}

#[derive(Debug, Clone)]
struct PreparedPrompt {
    preamble: String,
    latest_user_text: String,
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct McpExposedTool {
    openai_name: String,
    mcp_name: String,
    description: Option<String>,
    input_schema: Value,
}

struct AppState {
    config_path: PathBuf,
    data_path: PathBuf,
    state_lock: Mutex<()>,
}

impl AppState {
    fn new() -> Result<Self, String> {
        let project_dirs = ProjectDirs::from("ai", "easycall", "easy-call-ai")
            .ok_or_else(|| "Failed to resolve config directory".to_string())?;
        let config_dir = project_dirs.config_dir().to_path_buf();

        Ok(Self {
            config_path: config_dir.join("config.toml"),
            data_path: config_dir.join("app_data.json"),
            state_lock: Mutex::new(()),
        })
    }
}
fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn now_iso() -> String {
    now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn parse_iso(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).ok()
}

fn default_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
    id: "default-agent".to_string(),
    name: "助理".to_string(),
    system_prompt: "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string(),
    created_at: now.clone(),
    updated_at: now,
  }
}

fn ensure_default_agent(data: &mut AppData) {
    let old_prompt = "You are a concise and helpful assistant.";
    for agent in &mut data.agents {
        if agent.id == "default-agent" {
            if agent.name == "Default Agent" {
                agent.name = "助理".to_string();
            }
            if agent.system_prompt == old_prompt {
                agent.system_prompt = "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string();
            }
        }
    }
    if data.agents.is_empty() {
        data.agents.push(default_agent());
    }
    if data.selected_agent_id.trim().is_empty()
        || !data.agents.iter().any(|a| a.id == data.selected_agent_id)
    {
        data.selected_agent_id = default_selected_agent_id();
    }
    if data.user_alias.trim().is_empty() {
        data.user_alias = default_user_alias();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSettings {
    selected_agent_id: String,
    user_alias: String,
}

fn ensure_parent_dir(path: &PathBuf) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Config path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create config directory failed: {err}"))
}

fn read_config(path: &PathBuf) -> Result<AppConfig, String> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(path).map_err(|err| format!("Read config failed: {err}"))?;
    let mut parsed = toml::from_str::<AppConfig>(&content).unwrap_or_default();
    normalize_app_config(&mut parsed);
    Ok(parsed)
}

fn write_config(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let toml_str =
        toml::to_string_pretty(config).map_err(|err| format!("Serialize config failed: {err}"))?;
    fs::write(path, toml_str).map_err(|err| format!("Write config failed: {err}"))
}

fn normalize_api_tools(config: &mut AppConfig) {
    for api in &mut config.api_configs {
        if api.enable_tools && api.tools.is_empty() {
            api.tools = default_api_tools();
        }
    }
}

fn normalize_app_config(config: &mut AppConfig) {
    if config.api_configs.is_empty() {
        *config = AppConfig::default();
        return;
    }

    normalize_api_tools(config);

    if !config
        .api_configs
        .iter()
        .any(|a| a.id == config.selected_api_config_id)
    {
        config.selected_api_config_id = config.api_configs[0].id.clone();
    }

    let chat_valid = config
        .api_configs
        .iter()
        .any(|a| a.id == config.chat_api_config_id && a.enable_text);
    if !chat_valid {
        if let Some(api) = config.api_configs.iter().find(|a| a.enable_text) {
            config.chat_api_config_id = api.id.clone();
        } else {
            config.chat_api_config_id = config.selected_api_config_id.clone();
        }
    }

    config.stt_api_config_id = config
        .stt_api_config_id
        .as_deref()
        .filter(|id| {
            config
                .api_configs
                .iter()
                .any(|a| a.id == *id && a.enable_audio && a.request_format.trim() == "openai_tts")
        })
        .map(ToOwned::to_owned);

    config.vision_api_config_id = config
        .vision_api_config_id
        .as_deref()
        .filter(|id| {
            config
                .api_configs
                .iter()
                .any(|a| a.id == *id && a.enable_image)
        })
        .map(ToOwned::to_owned);
}

fn read_app_data(path: &PathBuf) -> Result<AppData, String> {
    if !path.exists() {
        return Ok(AppData::default());
    }

    let content = fs::read_to_string(path).map_err(|err| format!("Read app_data failed: {err}"))?;
    let mut parsed = serde_json::from_str::<AppData>(&content).unwrap_or_default();
    parsed.version = APP_DATA_SCHEMA_VERSION;
    ensure_default_agent(&mut parsed);
    Ok(parsed)
}

fn write_app_data(path: &PathBuf, data: &AppData) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let body = serde_json::to_string_pretty(data)
        .map_err(|err| format!("Serialize app_data failed: {err}"))?;
    fs::write(path, body).map_err(|err| format!("Write app_data failed: {err}"))
}

fn candidate_debug_config_paths() -> Vec<PathBuf> {
    vec![PathBuf::from(".debug").join("api-key.json")]
}

fn read_debug_api_config() -> Result<Option<DebugApiConfig>, String> {
    for path in candidate_debug_config_paths() {
        if !path.exists() {
            continue;
        }

        let content = fs::read_to_string(&path)
            .map_err(|err| format!("Read debug config failed ({}): {err}", path.display()))?;
        let parsed = serde_json::from_str::<DebugApiConfig>(&content)
            .map_err(|err| format!("Parse debug config failed ({}): {err}", path.display()))?;
        return Ok(Some(parsed));
    }
    Ok(None)
}

fn resolve_selected_api_config(
    app_config: &AppConfig,
    requested_id: Option<&str>,
) -> Option<ApiConfig> {
    if app_config.api_configs.is_empty() {
        return None;
    }

    let target_id = requested_id
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(app_config.chat_api_config_id.as_str());

    if let Some(found) = app_config.api_configs.iter().find(|p| p.id == target_id) {
        return Some(found.clone());
    }

    app_config.api_configs.first().cloned()
}

fn resolve_api_config(
    app_config: &AppConfig,
    requested_id: Option<&str>,
) -> Result<ResolvedApiConfig, String> {
    if let Some(debug_cfg) = read_debug_api_config()? {
        let enabled = debug_cfg.enabled.unwrap_or(true);
        let request_format_ok = debug_cfg
            .request_format
            .as_deref()
            .map(str::trim)
            .unwrap_or("openai")
            .eq_ignore_ascii_case("openai");

        if enabled && request_format_ok {
            if debug_cfg.api_key.trim().is_empty() {
                return Err(".debug/api-key.json exists but apiKey is empty.".to_string());
            }
            return Ok(ResolvedApiConfig {
                request_format: "openai".to_string(),
                base_url: debug_cfg.base_url.trim().to_string(),
                api_key: debug_cfg.api_key.trim().to_string(),
                model: debug_cfg.model.trim().to_string(),
                fixed_test_prompt: debug_cfg
                    .fixed_test_prompt
                    .unwrap_or_else(|| "EASY_CALL_AI_CACHE_TEST_V1".to_string()),
            });
        }
    }

    let selected = resolve_selected_api_config(app_config, requested_id).ok_or_else(|| {
        "No API config configured. Please add at least one API config.".to_string()
    })?;

    if selected.api_key.trim().is_empty() {
        return Err(
            "Selected API config API key is empty. Please fill it in settings.".to_string(),
        );
    }

    Ok(ResolvedApiConfig {
        request_format: selected.request_format.trim().to_string(),
        base_url: selected.base_url.trim().to_string(),
        api_key: selected.api_key.trim().to_string(),
        model: selected.model.trim().to_string(),
        fixed_test_prompt: "EASY_CALL_AI_CACHE_TEST_V1".to_string(),
    })
}

fn is_openai_style_request_format(request_format: &str) -> bool {
    matches!(request_format.trim(), "openai" | "deepseek/kimi")
}
fn ensure_active_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    if let Some((idx, _)) = data.conversations.iter().enumerate().find(|(_, c)| {
        c.status == "active" && c.api_config_id == api_config_id && c.agent_id == agent_id
    }) {
        return idx;
    }

    let now = now_iso();
    let conversation = Conversation {
        id: Uuid::new_v4().to_string(),
        title: format!("Chat {}", &now.chars().take(16).collect::<String>()),
        api_config_id: api_config_id.to_string(),
        agent_id: agent_id.to_string(),
        created_at: now.clone(),
        updated_at: now,
        last_assistant_at: None,
        status: "active".to_string(),
        messages: Vec::new(),
    };

    data.conversations.push(conversation);
    data.conversations.len() - 1
}

fn archive_if_idle(data: &mut AppData, api_config_id: &str, agent_id: &str) -> bool {
    let Some((idx, _)) = data.conversations.iter().enumerate().find(|(_, c)| {
        c.status == "active" && c.api_config_id == api_config_id && c.agent_id == agent_id
    }) else {
        return false;
    };

    let Some(last_assistant_at) = data.conversations[idx]
        .last_assistant_at
        .as_deref()
        .and_then(parse_iso)
    else {
        return false;
    };

    let now = now_utc();
    if now.unix_timestamp() - last_assistant_at.unix_timestamp() < ARCHIVE_IDLE_SECONDS {
        return false;
    }

    let mut source = data.conversations.remove(idx);
    source.status = "archived".to_string();
    source.updated_at = now_iso();
    data.archived_conversations.push(ConversationArchive {
        archive_id: Uuid::new_v4().to_string(),
        archived_at: now_iso(),
        reason: "idle_timeout_30m".to_string(),
        source_conversation: source,
    });

    true
}

fn compress_image_to_webp(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image =
        image::load_from_memory(bytes).map_err(|err| format!("Decode image failed: {err}"))?;
    let mut cursor = Cursor::new(Vec::<u8>::new());
    image
        .write_to(&mut cursor, ImageFormat::WebP)
        .map_err(|err| format!("Encode image to WebP failed: {err}"))?;
    Ok(cursor.into_inner())
}

fn build_user_parts(
    payload: &ChatInputPayload,
    api_config: &ApiConfig,
) -> Result<Vec<MessagePart>, String> {
    let mut parts = Vec::<MessagePart>::new();
    let mut total_binary = 0usize;

    if let Some(text) = payload
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !api_config.enable_text {
            return Err("Current API config has text disabled.".to_string());
        }
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }

    if let Some(images) = &payload.images {
        if !images.is_empty() && !api_config.enable_image {
            return Err("Current API config has image disabled.".to_string());
        }

        for image in images {
            let raw = B64
                .decode(image.bytes_base64.trim())
                .map_err(|err| format!("Decode image base64 failed: {err}"))?;
            let webp = compress_image_to_webp(&raw)?;
            total_binary += webp.len();
            parts.push(MessagePart::Image {
                mime: "image/webp".to_string(),
                bytes_base64: B64.encode(webp),
                name: None,
                compressed: true,
            });
        }
    }

    if let Some(audios) = &payload.audios {
        if !audios.is_empty() && !api_config.enable_audio {
            return Err("Current API config has audio disabled.".to_string());
        }

        for audio in audios {
            let raw = B64
                .decode(audio.bytes_base64.trim())
                .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
            total_binary += raw.len();
            parts.push(MessagePart::Audio {
                mime: audio.mime.trim().to_string(),
                bytes_base64: B64.encode(raw),
                name: None,
                compressed: false,
            });
        }
    }

    if total_binary > MAX_MULTIMODAL_BYTES {
        return Err(format!(
            "Multimodal payload exceeds 10MB limit ({} bytes).",
            total_binary
        ));
    }

    if parts.is_empty() {
        return Err("Request payload is empty. Provide text, image, or audio.".to_string());
    }

    Ok(parts)
}

fn render_message_for_context(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => chunks.push(text.clone()),
            MessagePart::Image { .. } => chunks.push("[image attached]".to_string()),
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    format!("{}: {}", message.role.to_uppercase(), chunks.join(" | "))
}
fn build_prompt(
    conversation: &Conversation,
    agent: &AgentProfile,
    user_alias: &str,
    current_time: &str,
) -> PreparedPrompt {
    let mut history_lines = Vec::<String>::new();
    for message in &conversation.messages {
        history_lines.push(render_message_for_context(message));
    }

    let preamble = format!(
    "[SYSTEM PROMPT]\n{}\n\n[ROLE MAPPING]\nAssistant name: {}\nUser name: {}\nRules:\n- You are the assistant named '{}'.\n- The human user is named '{}'.\n- Never treat yourself as the user.\n\n[TIME]\nCurrent UTC time: {}\n\n[CONVERSATION HISTORY]\n{}\n",
    agent.system_prompt,
    agent.name,
    user_alias,
    agent.name,
    user_alias,
    current_time,
    history_lines.join("\n")
  );

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();

    let mut latest_user_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        for part in msg.parts {
            match part {
                MessagePart::Text { text } => {
                    if !latest_user_text.is_empty() {
                        latest_user_text.push('\n');
                    }
                    latest_user_text.push_str(&text);
                }
                MessagePart::Image {
                    mime, bytes_base64, ..
                } => latest_images.push((mime, bytes_base64)),
                MessagePart::Audio {
                    mime, bytes_base64, ..
                } => latest_audios.push((mime, bytes_base64)),
            }
        }
    }

    PreparedPrompt {
        preamble,
        latest_user_text,
        latest_images,
        latest_audios,
    }
}

fn image_media_type_from_mime(mime: &str) -> Option<ImageMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some(ImageMediaType::JPEG),
        "image/png" => Some(ImageMediaType::PNG),
        "image/gif" => Some(ImageMediaType::GIF),
        "image/webp" => Some(ImageMediaType::WEBP),
        "image/heic" => Some(ImageMediaType::HEIC),
        "image/heif" => Some(ImageMediaType::HEIF),
        "image/svg+xml" => Some(ImageMediaType::SVG),
        _ => None,
    }
}

fn audio_media_type_from_mime(mime: &str) -> Option<AudioMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "audio/wav" | "audio/wave" => Some(AudioMediaType::WAV),
        "audio/mp3" | "audio/mpeg" => Some(AudioMediaType::MP3),
        "audio/aiff" => Some(AudioMediaType::AIFF),
        "audio/aac" => Some(AudioMediaType::AAC),
        "audio/ogg" => Some(AudioMediaType::OGG),
        "audio/flac" => Some(AudioMediaType::FLAC),
        _ => None,
    }
}

async fn call_model_openai_rig_style(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
) -> Result<String, String> {
    let mut content_items: Vec<UserContent> = vec![UserContent::text(prepared.preamble)];

    if !prepared.latest_user_text.trim().is_empty() {
        content_items.push(UserContent::text(prepared.latest_user_text));
    }

    for (mime, bytes) in prepared.latest_images {
        content_items.push(UserContent::image_base64(
            bytes,
            image_media_type_from_mime(&mime),
            Some(ImageDetail::Auto),
        ));
    }

    for (mime, bytes) in prepared.latest_audios {
        content_items.push(UserContent::audio(bytes, audio_media_type_from_mime(&mime)));
    }

    let prompt_content = OneOrMany::many(content_items)
        .map_err(|_| "Request payload is empty. Provide text, image, or audio.".to_string())?;

    let mut client_builder: openai::ClientBuilder =
        openai::Client::builder().api_key(&api_config.api_key);
    if !api_config.base_url.is_empty() {
        client_builder = client_builder.base_url(&api_config.base_url);
    }
    let client = client_builder
        .build()
        .map_err(|err| format!("Failed to create OpenAI client via rig: {err}"))?;

    let agent = client.completions_api().agent(model_name).build();
    let prompt_message = RigMessage::User {
        content: prompt_content,
    };

    agent
        .prompt(prompt_message)
        .await
        .map_err(|err| format!("rig prompt failed: {err}"))
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<invalid json>".to_string()),
    }
}

fn build_openai_tools_payload(tools: &[McpExposedTool]) -> Vec<Value> {
    tools
        .iter()
        .map(|tool| {
            serde_json::json!({
              "type": "function",
              "function": {
                "name": tool.openai_name,
                "description": tool.description.clone().unwrap_or_default(),
                "parameters": tool.input_schema,
              }
            })
        })
        .collect()
}

fn build_builtin_tools_payload(selected_api: &ApiConfig) -> Vec<McpExposedTool> {
    if !selected_api.enable_tools {
        return Vec::new();
    }
    selected_api
    .tools
    .iter()
    .filter_map(|tool| {
      if tool.id == "fetch" {
        return Some(McpExposedTool {
          openai_name: "fetch".to_string(),
          mcp_name: "fetch".to_string(),
          description: Some("Fetch webpage text.".to_string()),
          input_schema: serde_json::json!({
            "type": "object",
            "properties": {
              "url": { "type": "string", "description": "URL" },
              "max_length": { "type": "integer", "description": "Max chars", "default": 1800 }
            },
            "required": ["url"]
          }),
        });
      }
      if tool.id == "bing-search" {
        return Some(McpExposedTool {
          openai_name: "bing_search".to_string(),
          mcp_name: "bing-search".to_string(),
          description: Some("Search web with Bing.".to_string()),
          input_schema: serde_json::json!({
            "type": "object",
            "properties": {
              "query": { "type": "string", "description": "Query" },
              "num_results": { "type": "integer", "description": "Result count", "default": 5 }
            },
            "required": ["query"]
          }),
        });
      }
      None
    })
    .collect()
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn builtin_fetch(url: &str, max_length: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let resp = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .send()
        .await
        .map_err(|err| format!("Fetch url failed: {err}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("Fetch url failed with status {status}"));
    }
    let html = resp
        .text()
        .await
        .map_err(|err| format!("Read body failed: {err}"))?;
    let document = Html::parse_document(&html);
    let body_selector =
        Selector::parse("body").map_err(|err| format!("Parse selector failed: {err}"))?;
    let raw = document
        .select(&body_selector)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "));
    let cleaned = clean_text(&raw);
    let truncated = if cleaned.len() > max_length {
        format!("{}...", &cleaned[..max_length])
    } else {
        cleaned
    };
    Ok(serde_json::json!({
      "url": url,
      "content": truncated
    }))
}

async fn builtin_bing_search(query: &str, num_results: usize) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let mut last_error: Option<String> = None;
    for base in ["https://cn.bing.com", "https://www.bing.com"] {
        let url = format!("{base}/search?q={}", urlencoding::encode(query));
        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .send()
            .await;
        let Ok(resp) = resp else {
            last_error = Some("request failed".to_string());
            continue;
        };
        if !resp.status().is_success() {
            last_error = Some(format!("status {}", resp.status()));
            continue;
        }
        let html = resp
            .text()
            .await
            .map_err(|err| format!("Read search body failed: {err}"))?;
        let doc = Html::parse_document(&html);
        let item_sel =
            Selector::parse("li.b_algo").map_err(|err| format!("Parse selector failed: {err}"))?;
        let title_sel =
            Selector::parse("h2").map_err(|err| format!("Parse selector failed: {err}"))?;
        let a_sel =
            Selector::parse("h2 a").map_err(|err| format!("Parse selector failed: {err}"))?;
        let p_sel = Selector::parse("p").map_err(|err| format!("Parse selector failed: {err}"))?;
        let mut rows = Vec::new();
        for item in doc.select(&item_sel).take(num_results.max(1)) {
            let title = item
                .select(&title_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            let link = item
                .select(&a_sel)
                .next()
                .and_then(|n| n.value().attr("href"))
                .unwrap_or_default()
                .to_string();
            let snippet = item
                .select(&p_sel)
                .next()
                .map(|n| clean_text(&n.text().collect::<Vec<_>>().join(" ")))
                .unwrap_or_default();
            if !title.is_empty() && !link.is_empty() {
                rows.push(serde_json::json!({"title": title, "url": link, "snippet": snippet}));
            }
        }
        if !rows.is_empty() {
            return Ok(serde_json::json!({"query": query, "results": rows}));
        }
        last_error = Some("no results parsed".to_string());
    }
    Err(format!(
        "bing search failed: {}",
        last_error.unwrap_or_else(|| "unknown".to_string())
    ))
}

async fn execute_builtin_tool(name: &str, args: Value) -> Result<Value, String> {
    match name {
        "fetch" => {
            let url = args
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| "fetch.url is required".to_string())?;
            let max_length = args
                .get("max_length")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(1800);
            builtin_fetch(url, max_length).await
        }
        "bing-search" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| "bing_search.query is required".to_string())?;
            let num = args
                .get("num_results")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
                .unwrap_or(5);
            builtin_bing_search(query, num).await
        }
        _ => Err(format!("Unsupported builtin tool: {name}")),
    }
}

fn openai_headers(api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = format!("Bearer {}", api_key.trim());
    let auth_value = HeaderValue::from_str(&auth)
        .map_err(|err| format!("Build authorization header failed: {err}"))?;
    headers.insert(AUTHORIZATION, auth_value);
    Ok(headers)
}

fn candidate_openai_chat_urls(base_url: &str) -> Vec<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Vec::new();
    }
    let lower = base.to_ascii_lowercase();
    let mut urls = Vec::new();
    if lower.ends_with("/chat/completions") {
        urls.push(base.to_string());
    } else if lower.ends_with("/v1") {
        urls.push(format!("{base}/chat/completions"));
    } else {
        urls.push(format!("{base}/chat/completions"));
        urls.push(format!("{base}/v1/chat/completions"));
    }
    urls.sort();
    urls.dedup();
    urls
}

fn parse_stream_delta_text(content: &Option<Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|it| it.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// 通用 OpenAI SSE 流式请求：解析文本 delta（实时推送到 on_delta）和 tool_calls 积累。
/// 返回 (完整文本, 积累的 tool_calls)。
async fn openai_stream_request(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(String, Vec<OpenAIToolCall>), String> {
    openai_stream_request_with_sink(client, url, body, |delta| {
        let send_result = on_delta.send(AssistantDeltaEvent {
            delta: delta.to_string(),
        });
        eprintln!(
            "[STREAM-DEBUG] on_delta.send result: {:?}, delta_len={}",
            send_result,
            delta.len()
        );
    })
    .await
}

async fn openai_stream_request_with_sink<F>(
    client: &reqwest::Client,
    url: &str,
    body: Value,
    mut on_delta: F,
) -> Result<(String, Vec<OpenAIToolCall>), String>
where
    F: FnMut(&str),
{
    eprintln!("[STREAM-DEBUG] openai_stream_request called, url={}", url);
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|err| format!("OpenAI stream request failed: {err}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        return Err(format!(
            "OpenAI stream failed with status {status}: {}",
            raw.chars().take(300).collect::<String>()
        ));
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut output = String::new();

    // 积累 tool_calls：按 index 分组
    let mut tool_calls_map: std::collections::BTreeMap<usize, (String, String, String)> =
        std::collections::BTreeMap::new(); // index -> (id, name, arguments)

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|err| format!("Read stream chunk failed: {err}"))?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer.drain(..=pos);

            if !line.starts_with("data:") {
                continue;
            }
            let data = line["data:".len()..].trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                break;
            }

            let Ok(parsed) = serde_json::from_str::<OpenAIStreamChunk>(data) else {
                eprintln!(
                    "[STREAM-DEBUG] SSE parse failed: {}",
                    &data[..data.len().min(200)]
                );
                continue;
            };
            if parsed.choices.is_empty() {
                continue;
            }
            let choice = &parsed.choices[0];

            // 处理文本 delta
            let delta_text = parse_stream_delta_text(&choice.delta.content);
            if !delta_text.is_empty() {
                output.push_str(&delta_text);
                on_delta(&delta_text);
            }

            // 处理 tool_calls delta
            if let Some(tc_deltas) = &choice.delta.tool_calls {
                for tc_delta in tc_deltas {
                    let entry = tool_calls_map
                        .entry(tc_delta.index)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = &tc_delta.id {
                        entry.0 = id.clone();
                    }
                    if let Some(func) = &tc_delta.function {
                        if let Some(name) = &func.name {
                            entry.1.push_str(name);
                        }
                        if let Some(args) = &func.arguments {
                            entry.2.push_str(args);
                        }
                    }
                }
            }
        }
    }

    let tool_calls: Vec<OpenAIToolCall> = tool_calls_map
        .into_iter()
        .map(|(index, (id, name, arguments))| OpenAIToolCall {
            id: if id.trim().is_empty() {
                format!("tool_call_{index}")
            } else {
                id
            },
            function: OpenAIToolCallFunction { name, arguments },
        })
        .filter(|tc| !tc.function.name.trim().is_empty())
        .collect();

    eprintln!(
        "[STREAM-DEBUG] stream done: output_len={}, tool_calls_count={}",
        output.len(),
        tool_calls.len()
    );
    Ok((output, tool_calls))
}

async fn call_model_openai_stream_text(
    api_config: &ResolvedApiConfig,
    model_name: &str,
    prepared: &PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(openai_headers(&api_config.api_key)?)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let body = serde_json::json!({
      "model": model_name,
      "messages": [
        { "role": "system", "content": prepared.preamble },
        { "role": "user", "content": prepared.latest_user_text }
      ],
      "stream": true
    });

    let urls = candidate_openai_chat_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    let mut errors = Vec::new();
    for url in urls {
        match openai_stream_request(&client, &url, body.clone(), on_delta).await {
            Ok((text, _)) => return Ok(text),
            Err(err) => errors.push(format!("{url} -> {err}")),
        }
    }

    Err(format!(
        "OpenAI stream request failed for all candidate URLs: {}",
        errors.join(" || ")
    ))
}

async fn call_model_openai_with_tools(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    let exposed_tools = build_builtin_tools_payload(selected_api);
    if exposed_tools.is_empty() {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .default_headers(openai_headers(&api_config.api_key)?)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;
    let urls = candidate_openai_chat_urls(&api_config.base_url);
    if urls.is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    let mut messages = vec![
        serde_json::json!({"role":"system","content": prepared.preamble}),
        serde_json::json!({"role":"user","content": prepared.latest_user_text}),
    ];
    let mut full_assistant_text = String::new();
    let tools_payload = build_openai_tools_payload(&exposed_tools);

    for _ in 0..4 {
        let body = serde_json::json!({
          "model": model_name,
          "messages": messages,
          "tools": tools_payload,
          "tool_choice": "auto",
          "temperature": 0.2,
          "stream": true
        });

        let mut stream_result: Option<(String, Vec<OpenAIToolCall>)> = None;
        let mut stream_errors = Vec::new();
        for url in &urls {
            match openai_stream_request(&client, url, body.clone(), on_delta).await {
                Ok(ok) => {
                    stream_result = Some(ok);
                    break;
                }
                Err(err) => stream_errors.push(format!("{url} -> {err}")),
            }
        }
        let (text, tool_calls) = stream_result.ok_or_else(|| {
            format!(
                "OpenAI tool stream request failed for all candidate URLs: {}",
                stream_errors.join(" || ")
            )
        })?;
        if !text.is_empty() {
            if !full_assistant_text.trim().is_empty() {
                full_assistant_text.push_str("\n\n");
            }
            full_assistant_text.push_str(&text);
        }

        if tool_calls.is_empty() {
            return Ok(full_assistant_text);
        }

        // 有 tool_calls：积累到 messages 中，继续循环
        let assistant_tool_calls = tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                  "id": tc.id,
                  "type": "function",
                  "function": {
                    "name": tc.function.name,
                    "arguments": tc.function.arguments,
                  }
                })
            })
            .collect::<Vec<_>>();
        let content_value = if text.is_empty() {
            Value::Null
        } else {
            Value::String(text)
        };
        messages.push(serde_json::json!({
          "role":"assistant",
          "content": content_value,
          "tool_calls": assistant_tool_calls
        }));

        for tc in &tool_calls {
            let Some(exposed) = exposed_tools
                .iter()
                .find(|tool| tool.openai_name == tc.function.name)
            else {
                messages.push(serde_json::json!({
                  "role":"tool",
                  "tool_call_id": tc.id,
                  "content": format!("Tool '{}' is not registered.", tc.function.name)
                }));
                continue;
            };

            let args = if tc.function.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str::<Value>(&tc.function.arguments).map_err(|err| {
                    format!("Parse tool arguments failed ({}): {err}", tc.function.name)
                })?
            };

            let result_value = execute_builtin_tool(&exposed.mcp_name, args).await?;
            let tool_text = value_to_text(&result_value);
            messages.push(serde_json::json!({
              "role":"tool",
              "tool_call_id": tc.id,
              "content": tool_text
            }));
        }
    }

    Err("Tool call exceeded max iterations.".to_string())
}

async fn call_model_openai_style(
    api_config: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    model_name: &str,
    prepared: PreparedPrompt,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<String, String> {
    eprintln!(
        "[STREAM-DEBUG] call_model_openai_style: format={}, enable_tools={}, images={}, audios={}",
        selected_api.request_format,
        selected_api.enable_tools,
        prepared.latest_images.len(),
        prepared.latest_audios.len()
    );
    // 优先使用工具调用（如果启用）
    if selected_api.enable_tools
        && selected_api.request_format.trim() == "openai"
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_with_tools(
            api_config,
            selected_api,
            model_name,
            prepared,
            on_delta,
        )
        .await;
    }

    // 纯文本流式传输（无论工具是否启用，只要没有工具调用就走流式）
    if selected_api.request_format.trim() == "openai"
        && prepared.latest_images.is_empty()
        && prepared.latest_audios.is_empty()
    {
        return call_model_openai_stream_text(api_config, model_name, &prepared, on_delta).await;
    }

    // 回退到 rig（支持多模态）
    call_model_openai_rig_style(api_config, model_name, prepared).await
}

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    if label == "chat" {
        let _ = archive_selected_conversation_if_idle(app);
    }

    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;

    if let Ok(Some(monitor)) = window.current_monitor() {
        if let Ok(window_size) = window.outer_size() {
            let margin = 24_i32;
            let x = monitor.position().x + monitor.size().width as i32
                - window_size.width as i32
                - margin;
            let y = monitor.position().y + margin;
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
    }

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("easy-call:refresh", ());
    Ok(())
}

fn archive_selected_conversation_if_idle(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;
    let selected_agent_id = data.selected_agent_id.clone();
    let changed = archive_if_idle(&mut data, &api_config.id, &selected_agent_id);
    if changed {
        write_app_data(&state.data_path, &data)?;
    }

    drop(guard);
    Ok(())
}

fn toggle_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let visible = window
        .is_visible()
        .map_err(|err| format!("Check window visibility failed: {err}"))?;
    if visible {
        window
            .hide()
            .map_err(|err| format!("Hide window failed: {err}"))?;
        return Ok(());
    }
    show_window(app, label)
}

fn register_default_hotkey(app: &AppHandle) -> Result<(), String> {
    let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Backquote);
    app.global_shortcut()
        .register(shortcut)
        .map_err(|err| format!("Register hotkey failed: {err}"))
}

fn build_tray(app: &AppHandle) -> Result<(), String> {
    let config = MenuItem::with_id(app, "config", "配置", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let chat = MenuItem::with_id(app, "chat", "对话", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let archives = MenuItem::with_id(app, "archives", "归档", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;

    let menu = Menu::with_items(app, &[&config, &chat, &archives, &quit])
        .map_err(|err| format!("Create tray menu failed: {err}"))?;

    let mut tray = TrayIconBuilder::new().menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.tooltip("Easy Call AI")
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "config" {
                let _ = show_window(app, "main");
            } else if id == "chat" {
                let _ = show_window(app, "chat");
            } else if id == "archives" {
                let _ = show_window(app, "archives");
            } else if id == "quit" {
                app.exit(0);
            }
        })
        .build(app)
        .map_err(|err| format!("Build tray failed: {err}"))?;

    Ok(())
}

fn hide_on_close(app: &AppHandle) {
    for label in ["main", "chat", "archives"] {
        if let Some(window) = app.get_webview_window(label) {
            let cloned = window.clone();
            let _ = window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = cloned.hide();
                }
            });
        }
    }
}
#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut result = read_config(&state.config_path)?;
    normalize_app_config(&mut result);
    drop(guard);
    Ok(result)
}

#[tauri::command]
fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<AppConfig, String> {
    if config.api_configs.is_empty() {
        return Err("At least one API config must be configured.".to_string());
    }
    let mut config = config;
    normalize_app_config(&mut config);

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    write_config(&state.config_path, &config)?;
    drop(guard);
    Ok(config)
}

#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn save_agents(
    input: SaveAgentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<AgentProfile>, String> {
    if input.agents.is_empty() {
        return Err("At least one agent is required.".to_string());
    }

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    data.agents = input.agents;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(data.agents)
}

#[tauri::command]
fn load_chat_settings(state: State<'_, AppState>) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ChatSettings {
        selected_agent_id: data.selected_agent_id,
        user_alias: data.user_alias,
    })
}

#[tauri::command]
fn save_chat_settings(
    input: ChatSettings,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data.agents.iter().any(|a| a.id == input.selected_agent_id) {
        return Err("Selected agent not found.".to_string());
    }
    data.selected_agent_id = input.selected_agent_id.clone();
    data.user_alias = if input.user_alias.trim().is_empty() {
        default_user_alias()
    } else {
        input.user_alias.trim().to_string()
    };
    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(input)
}

#[tauri::command]
fn get_chat_snapshot(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ChatSnapshot, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    if !data.agents.iter().any(|a| a.id == input.agent_id) {
        return Err("Selected agent not found.".to_string());
    }

    archive_if_idle(&mut data, &api_config.id, &input.agent_id);
    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &input.agent_id);
    let conversation = &data.conversations[idx];

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .cloned();
    let latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .cloned();

    write_app_data(&state.data_path, &data)?;
    drop(guard);

    Ok(ChatSnapshot {
        conversation_id: conversation.id.clone(),
        latest_user,
        latest_assistant,
        active_message_count: conversation.messages.len(),
    })
}

#[tauri::command]
fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;

    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    archive_if_idle(&mut data, &api_config.id, &input.agent_id);
    let idx = ensure_active_conversation_index(&mut data, &api_config.id, &input.agent_id);
    let messages = data.conversations[idx].messages.clone();

    write_app_data(&state.data_path, &data)?;
    drop(guard);
    Ok(messages)
}

#[tauri::command]
fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let mut summaries = data
        .archived_conversations
        .iter()
        .map(|archive| ArchiveSummary {
            archive_id: archive.archive_id.clone(),
            archived_at: archive.archived_at.clone(),
            title: archive.source_conversation.title.clone(),
            message_count: archive.source_conversation.messages.len(),
            api_config_id: archive.source_conversation.api_config_id.clone(),
            agent_id: archive.source_conversation.agent_id.clone(),
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    Ok(summaries)
}

#[tauri::command]
fn get_archive_messages(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let data = read_app_data(&state.data_path)?;
    drop(guard);

    let archive = data
        .archived_conversations
        .iter()
        .find(|a| a.archive_id == archive_id)
        .ok_or_else(|| "Archive not found".to_string())?;

    Ok(archive.source_conversation.messages.clone())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("Only http/https URLs are allowed.".to_string());
    }
    webbrowser::open(trimmed).map_err(|err| format!("Open browser failed: {err}"))?;
    Ok(())
}

#[tauri::command]
async fn send_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    let (
        resolved_api,
        selected_api,
        model_name,
        prepared_prompt,
        conversation_id,
        latest_user_text,
        archived_before_send,
    ) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;

        let app_config = read_config(&state.config_path)?;
        let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(api_config.id.as_str()))?;

        if !is_openai_style_request_format(&resolved_api.request_format) {
            return Err(format!(
                "Request format '{}' is not implemented in chat router yet.",
                resolved_api.request_format
            ));
        }

        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
        let agent = data
            .agents
            .iter()
            .find(|a| a.id == input.agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;

        let archived_before_send = archive_if_idle(&mut data, &api_config.id, &input.agent_id);
        let idx = ensure_active_conversation_index(&mut data, &api_config.id, &input.agent_id);

        let user_parts = build_user_parts(&input.payload, &api_config)?;
        let latest_user_text = user_parts
            .iter()
            .map(|part| match part {
                MessagePart::Text { text } => text.clone(),
                MessagePart::Image { .. } => "[image]".to_string(),
                MessagePart::Audio { .. } => "[audio]".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n");

        let now = now_iso();
        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            parts: user_parts,
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        };

        data.conversations[idx].messages.push(user_message);
        data.conversations[idx].updated_at = now;

        let conversation = data.conversations[idx].clone();
        let prepared = build_prompt(&conversation, &agent, &data.user_alias, &now_iso());

        let model_name = input
            .payload
            .model
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| resolved_api.model.clone());
        let conversation_id = conversation.id.clone();

        write_app_data(&state.data_path, &data)?;
        drop(guard);

        (
            resolved_api,
            api_config,
            model_name,
            prepared,
            conversation_id,
            latest_user_text,
            archived_before_send,
        )
    };

    let assistant_text = call_model_openai_style(
        &resolved_api,
        &selected_api,
        &model_name,
        prepared_prompt,
        &on_delta,
    )
    .await?;

    {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;

        let mut data = read_app_data(&state.data_path)?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id && c.status == "active")
        {
            let now = now_iso();
            conversation.messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                parts: vec![MessagePart::Text {
                    text: assistant_text.clone(),
                }],
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            });
            conversation.updated_at = now.clone();
            conversation.last_assistant_at = Some(now);
            write_app_data(&state.data_path, &data)?;
        }
        drop(guard);
    }

    Ok(SendChatResult {
        conversation_id,
        latest_user_text,
        assistant_text,
        archived_before_send,
    })
}

async fn fetch_models_openai(input: &RefreshModelsInput) -> Result<Vec<String>, String> {
    let base = input.base_url.trim().trim_end_matches('/');

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = format!("Bearer {}", input.api_key.trim());
    let auth_value = HeaderValue::from_str(&auth)
        .map_err(|err| format!("Build authorization header failed: {err}"))?;
    headers.insert(AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .default_headers(headers)
        .build()
        .map_err(|err| format!("Build HTTP client failed: {err}"))?;

    let mut candidate_urls = vec![format!("{base}/models")];
    if !base.to_ascii_lowercase().contains("/v1") {
        candidate_urls.push(format!("{base}/v1/models"));
    }
    candidate_urls.sort();
    candidate_urls.dedup();

    let mut errors = Vec::new();
    for url in candidate_urls {
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|err| format!("Fetch model list failed ({url}): {err}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let raw = resp.text().await.unwrap_or_default();
            let snippet = raw.chars().take(300).collect::<String>();
            errors.push(format!("{url} -> {status} | {snippet}"));
            continue;
        }

        let body = resp
            .json::<OpenAIModelListResponse>()
            .await
            .map_err(|err| format!("Parse model list failed ({url}): {err}"))?;

        let mut models = body
            .data
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        return Ok(models);
    }

    if errors.is_empty() {
        Err("Fetch model list failed: no candidate URL attempted".to_string())
    } else {
        Err(format!(
            "Fetch model list failed. Tried: {}",
            errors.join(" || ")
        ))
    }
}

#[tauri::command]
async fn refresh_models(input: RefreshModelsInput) -> Result<Vec<String>, String> {
    if input.api_key.trim().is_empty() {
        return Err("API key is empty.".to_string());
    }
    if input.base_url.trim().is_empty() {
        return Err("Base URL is empty.".to_string());
    }

    match input.request_format.trim() {
        "openai" | "deepseek/kimi" => fetch_models_openai(&input).await,
        "openai_tts" => Err(
            "Request format 'openai_tts' is for audio transcriptions and does not support model list refresh."
                .to_string(),
        ),
        other => Err(format!(
            "Request format '{other}' model refresh is not implemented yet."
        )),
    }
}

#[tauri::command]
fn check_tools_status(
    input: CheckToolsStatusInput,
    state: State<'_, AppState>,
) -> Result<Vec<ToolLoadStatus>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut config = read_config(&state.config_path)?;
    normalize_api_tools(&mut config);
    drop(guard);

    let selected = resolve_selected_api_config(&config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config configured. Please add one.".to_string())?;

    if !selected.enable_tools {
        return Ok(selected
            .tools
            .iter()
            .map(|tool| ToolLoadStatus {
                id: tool.id.clone(),
                status: "disabled".to_string(),
                detail: "此 API 配置未启用工具调用。".to_string(),
            })
            .collect());
    }

    let mut statuses = Vec::new();
    for tool in selected.tools {
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "bing-search" => ("loaded".to_string(), "内置 Bing 爬虫搜索可用".to_string()),
            other => ("failed".to_string(), format!("未支持的内置工具: {other}")),
        };
        statuses.push(ToolLoadStatus {
            id: tool.id,
            status,
            detail,
        });
    }
    Ok(statuses)
}

#[tauri::command]
async fn send_debug_probe(state: State<'_, AppState>) -> Result<String, String> {
    let app_config = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let cfg = read_config(&state.config_path)?;
        drop(guard);
        cfg
    };

    let api_config = resolve_api_config(&app_config, None)?;
    if !is_openai_style_request_format(&api_config.request_format) {
        return Err(format!(
            "Request format '{}' is not implemented in probe router yet.",
            api_config.request_format
        ));
    }

    let prepared = PreparedPrompt {
        preamble: format!("[TIME]\nCurrent UTC time: {}", now_iso()),
        latest_user_text: api_config.fixed_test_prompt.clone(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    };

    call_model_openai_rig_style(&api_config, &api_config.model, prepared).await
}

fn main() {
    let state = match AppState::new() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("Failed to initialize application state: {err}");
            return;
        }
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_window(app, "chat");
                    }
                })
                .build(),
        )
        .manage(state)
        .setup(|app| {
            let app_handle = app.handle().clone();
            register_default_hotkey(&app_handle)?;
            build_tray(&app_handle)?;
            hide_on_close(&app_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            load_agents,
            save_agents,
            load_chat_settings,
            save_chat_settings,
            get_chat_snapshot,
            get_active_conversation_messages,
            list_archives,
            get_archive_messages,
            open_external_url,
            send_chat_message,
            refresh_models,
            check_tools_status,
            send_debug_probe
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("error while running tauri application: {err}");
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{
        Method::{GET, POST},
        MockServer,
    };

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
    }

    #[test]
    fn candidate_openai_chat_urls_should_handle_common_forms() {
        assert_eq!(
            candidate_openai_chat_urls("https://api.openai.com/v1"),
            vec!["https://api.openai.com/v1/chat/completions".to_string()]
        );
        assert_eq!(
            candidate_openai_chat_urls("https://gateway.example.com/chat/completions"),
            vec!["https://gateway.example.com/chat/completions".to_string()]
        );
        assert_eq!(
            candidate_openai_chat_urls("https://gateway.example.com"),
            vec![
                "https://gateway.example.com/chat/completions".to_string(),
                "https://gateway.example.com/v1/chat/completions".to_string()
            ]
        );
        assert!(candidate_openai_chat_urls("  ").is_empty());
    }

    #[test]
    fn fetch_models_openai_should_read_models_from_base_url() {
        let server = MockServer::start();
        let model_mock = server.mock(|when, then| {
            when.method(GET).path("/models");
            then.status(200).json_body(serde_json::json!({
              "data": [
                { "id": "gpt-4o-mini" },
                { "id": "gpt-4.1-mini" }
              ]
            }));
        });

        let input = RefreshModelsInput {
            base_url: server.base_url(),
            api_key: "test-key".to_string(),
            request_format: "openai".to_string(),
        };

        let rt = test_runtime();
        let models = rt
            .block_on(fetch_models_openai(&input))
            .expect("fetch models from mock");

        model_mock.assert();
        assert_eq!(
            models,
            vec!["gpt-4.1-mini".to_string(), "gpt-4o-mini".to_string()]
        );
    }

    #[test]
    fn fetch_models_openai_should_fallback_to_v1_models() {
        let server = MockServer::start();
        let base_404_mock = server.mock(|when, then| {
            when.method(GET).path("/models");
            then.status(404).body("not found");
        });
        let v1_ok_mock = server.mock(|when, then| {
            when.method(GET).path("/v1/models");
            then.status(200).json_body(serde_json::json!({
              "data": [{ "id": "moonshot-v1-8k" }]
            }));
        });

        let input = RefreshModelsInput {
            base_url: server.base_url(),
            api_key: "test-key".to_string(),
            request_format: "openai".to_string(),
        };

        let rt = test_runtime();
        let models = rt
            .block_on(fetch_models_openai(&input))
            .expect("fallback /v1/models should succeed");

        base_404_mock.assert();
        v1_ok_mock.assert();
        assert_eq!(models, vec!["moonshot-v1-8k".to_string()]);
    }

    #[test]
    fn openai_stream_request_with_sink_should_emit_incremental_deltas() {
        let server = MockServer::start();
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n",
            "\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n",
            "\n",
            "data: [DONE]\n",
            "\n"
        );
        let sse_mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(sse_body);
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("build reqwest client");
        let body = serde_json::json!({
          "model": "gpt-4o-mini",
          "messages": [{ "role": "user", "content": "hello" }],
          "stream": true
        });

        let mut deltas = Vec::<String>::new();
        let rt = test_runtime();
        let (full_text, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |delta| deltas.push(delta.to_string()),
            ))
            .expect("stream request should parse");

        sse_mock.assert();
        assert_eq!(deltas, vec!["你".to_string(), "好".to_string()]);
        assert_eq!(full_text, "你好".to_string());
        assert!(tool_calls.is_empty());
    }

    #[test]
    fn openai_stream_request_with_sink_should_assemble_tool_calls_from_fragments() {
        let server = MockServer::start();
        let sse_body = concat!(
      "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"bing_\",\"arguments\":\"{\\\"query\\\":\\\"\"}}]}}]}\n",
      "\n",
      "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"search\",\"arguments\":\"rust\\\"}\"}}]}}]}\n",
      "\n",
      "data: [DONE]\n",
      "\n"
    );
        let sse_mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(sse_body);
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("build reqwest client");
        let body = serde_json::json!({
          "model": "gpt-4o-mini",
          "messages": [{ "role": "user", "content": "hello" }],
          "stream": true
        });

        let rt = test_runtime();
        let (_full_text, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |_delta| {},
            ))
            .expect("stream tool call should parse");

        sse_mock.assert();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1".to_string());
        assert_eq!(tool_calls[0].function.name, "bing_search".to_string());
        assert_eq!(
            tool_calls[0].function.arguments,
            "{\"query\":\"rust\"}".to_string()
        );
    }
}
