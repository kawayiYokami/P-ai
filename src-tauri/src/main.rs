use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use futures_util::StreamExt;
use image::ImageFormat;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rig::{
    completion::{
        message::{AudioMediaType, ImageDetail, ImageMediaType, UserContent},
        Message as RigMessage, Prompt, ToolDefinition,
    },
    message::{AssistantContent, ToolResultContent},
    prelude::CompletionClient,
    providers::openai,
    streaming::{StreamedAssistantContent, StreamingCompletion},
    tool::{Tool, ToolDyn},
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
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const APP_DATA_SCHEMA_VERSION: u32 = 1;
const ARCHIVE_IDLE_SECONDS: i64 = 30 * 60;
const MAX_MULTIMODAL_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_AGENT_ID: &str = "default-agent";
const USER_PERSONA_ID: &str = "user-persona";
const DEFAULT_RESPONSE_STYLE_ID: &str = "concise";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseStylePreset {
    id: String,
    name: String,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HighestInstruction {
    title: String,
    rules: Vec<String>,
}

fn built_in_response_styles() -> &'static Vec<ResponseStylePreset> {
    static STYLES: OnceLock<Vec<ResponseStylePreset>> = OnceLock::new();
    STYLES.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/constants/response-styles.json")).unwrap_or_else(
            |_| {
                vec![ResponseStylePreset {
                    id: DEFAULT_RESPONSE_STYLE_ID.to_string(),
                    name: "简洁".to_string(),
                    prompt: "- 用最少但足够的信息回答。".to_string(),
                }]
            },
        )
    })
}

fn default_response_style_id() -> String {
    DEFAULT_RESPONSE_STYLE_ID.to_string()
}

fn normalize_response_style_id(value: &str) -> String {
    let id = value.trim();
    if built_in_response_styles().iter().any(|s| s.id == id) {
        id.to_string()
    } else {
        default_response_style_id()
    }
}

fn response_style_preset(id: &str) -> ResponseStylePreset {
    built_in_response_styles()
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .or_else(|| built_in_response_styles().first().cloned())
        .unwrap_or(ResponseStylePreset {
            id: DEFAULT_RESPONSE_STYLE_ID.to_string(),
            name: "简洁".to_string(),
            prompt: "- 用最少但足够的信息回答。".to_string(),
        })
}

fn highest_instruction() -> &'static HighestInstruction {
    static INSTRUCTION: OnceLock<HighestInstruction> = OnceLock::new();
    INSTRUCTION.get_or_init(|| {
        serde_json::from_str(include_str!("../../src/constants/highest-instruction.json"))
            .unwrap_or_else(|_| HighestInstruction {
                title: "系统准则".to_string(),
                rules: vec![
                    "你必须基于客观事实回答问题，不编造数据、来源或结论。".to_string(),
                    "若信息不足或不确定，直接说明不确定，并给出可验证路径。".to_string(),
                    "优先给出可执行、可验证、与用户问题直接相关的结论。".to_string(),
                ],
            })
    })
}

fn highest_instruction_markdown() -> String {
    let source = highest_instruction();
    let title = source.title.trim();
    let title = if title.is_empty() { "系统准则" } else { title };
    let mut out = format!("# {}\n", title);
    for rule in &source.rules {
        let line = rule.trim();
        if !line.is_empty() {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

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
        ApiToolConfig {
            id: "memory-save".to_string(),
            command: "builtin".to_string(),
            args: vec!["memory-save".to_string()],
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
    #[serde(default = "default_false")]
    enable_image: bool,
    #[serde(default = "default_false")]
    enable_audio: bool,
    #[serde(default = "default_false")]
    enable_tools: bool,
    #[serde(default = "default_api_tools")]
    tools: Vec<ApiToolConfig>,
    base_url: String,
    api_key: String,
    model: String,
    #[serde(default = "default_api_temperature")]
    temperature: f64,
    #[serde(default = "default_context_window_tokens")]
    context_window_tokens: u32,
}

fn default_true() -> bool {
    true
}

fn default_record_hotkey() -> String {
    "Alt".to_string()
}

fn default_min_record_seconds() -> u32 {
    1
}

fn default_max_record_seconds() -> u32 {
    60
}

fn default_tool_max_iterations() -> u32 {
    10
}

fn default_api_temperature() -> f64 {
    1.0
}

fn default_context_window_tokens() -> u32 {
    128_000
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            id: "default-openai".to_string(),
            name: "Default OpenAI".to_string(),
            request_format: "openai".to_string(),
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_tools: false,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            temperature: default_api_temperature(),
            context_window_tokens: default_context_window_tokens(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    hotkey: String,
    #[serde(default = "default_record_hotkey")]
    record_hotkey: String,
    #[serde(default = "default_min_record_seconds")]
    min_record_seconds: u32,
    #[serde(default = "default_max_record_seconds")]
    max_record_seconds: u32,
    #[serde(default = "default_tool_max_iterations")]
    tool_max_iterations: u32,
    selected_api_config_id: String,
    #[serde(default)]
    chat_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    api_configs: Vec<ApiConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let api_config = ApiConfig::default();
        Self {
            hotkey: "Alt+·".to_string(),
            record_hotkey: default_record_hotkey(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            tool_max_iterations: default_tool_max_iterations(),
            selected_api_config_id: api_config.id.clone(),
            chat_api_config_id: api_config.id.clone(),
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
    temperature: Option<f64>,
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
    reasoning_standard: String,
    reasoning_inline: String,
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
    reasoning_content: Option<String>,
    reasoning_details: Option<Value>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfile {
    id: String,
    name: String,
    system_prompt: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    avatar_path: Option<String>,
    #[serde(default)]
    avatar_updated_at: Option<String>,
    #[serde(default)]
    is_built_in_user: bool,
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
    #[serde(default)]
    extra_text_blocks: Vec<String>,
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
    #[serde(default)]
    last_user_at: Option<String>,
    last_assistant_at: Option<String>,
    #[serde(default)]
    last_context_usage_ratio: f64,
    status: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationArchive {
    archive_id: String,
    archived_at: String,
    reason: String,
    #[serde(default)]
    summary: String,
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
struct ImageTextCacheEntry {
    hash: String,
    vision_api_id: String,
    text: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEntry {
    id: String,
    content: String,
    keywords: Vec<String>,
    created_at: String,
    updated_at: String,
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
    #[serde(default = "default_response_style_id")]
    response_style_id: String,
    conversations: Vec<Conversation>,
    archived_conversations: Vec<ConversationArchive>,
    #[serde(default)]
    image_text_cache: Vec<ImageTextCacheEntry>,
    #[serde(default)]
    memories: Vec<MemoryEntry>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: APP_DATA_SCHEMA_VERSION,
            agents: vec![default_agent(), default_user_persona()],
            selected_agent_id: default_selected_agent_id(),
            user_alias: default_user_alias(),
            response_style_id: default_response_style_id(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
            memories: Vec::new(),
        }
    }
}

fn default_selected_agent_id() -> String {
    DEFAULT_AGENT_ID.to_string()
}

fn default_user_alias() -> String {
    "用户".to_string()
}

fn user_persona_name(data: &AppData) -> String {
    data.agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID || a.is_built_in_user)
        .map(|a| a.name.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_user_alias)
}

fn user_persona_intro(data: &AppData) -> String {
    data.agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID || a.is_built_in_user)
        .map(|a| a.system_prompt.trim().to_string())
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
struct ResolvedApiConfig {
    request_format: String,
    base_url: String,
    api_key: String,
    model: String,
    temperature: f64,
    fixed_test_prompt: String,
}

#[derive(Debug, Clone)]
struct PreparedHistoryMessage {
    role: String,
    text: String,
    tool_calls: Option<Vec<Value>>,
    tool_call_id: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedPrompt {
    preamble: String,
    history_messages: Vec<PreparedHistoryMessage>,
    latest_user_text: String,
    latest_user_system_text: String,
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct AppState {
    config_path: PathBuf,
    data_path: PathBuf,
    state_lock: Arc<Mutex<()>>,
}

impl AppState {
    fn new() -> Result<Self, String> {
        let project_dirs = ProjectDirs::from("ai", "easycall", "easy-call-ai")
            .ok_or_else(|| "Failed to resolve config directory".to_string())?;
        let config_dir = project_dirs.config_dir().to_path_buf();

        Ok(Self {
            config_path: config_dir.join("config.toml"),
            data_path: config_dir.join("app_data.json"),
            state_lock: Arc::new(Mutex::new(())),
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
        id: DEFAULT_AGENT_ID.to_string(),
        name: "助理".to_string(),
        system_prompt: "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
    }
}

fn default_user_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: USER_PERSONA_ID.to_string(),
        name: "用户".to_string(),
        system_prompt: "我是...".to_string(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: true,
    }
}

fn ensure_default_agent(data: &mut AppData) -> bool {
    let mut changed = false;
    let old_prompt = "You are a concise and helpful assistant.";
    let mut has_assistant = false;
    let mut has_user_persona = false;
    for agent in &mut data.agents {
        if agent.id == DEFAULT_AGENT_ID {
            has_assistant = true;
            if agent.is_built_in_user {
                agent.is_built_in_user = false;
                changed = true;
            }
            if agent.name == "Default Agent" {
                agent.name = "助理".to_string();
                changed = true;
            }
            if agent.system_prompt == old_prompt {
                agent.system_prompt = "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string();
                changed = true;
            }
        } else if agent.id == USER_PERSONA_ID {
            has_user_persona = true;
            if !agent.is_built_in_user {
                agent.is_built_in_user = true;
                changed = true;
            }
        } else if !agent.is_built_in_user {
            has_assistant = true;
        }
    }
    if !has_assistant {
        data.agents.push(default_agent());
        changed = true;
    }
    if !has_user_persona {
        data.agents.push(default_user_persona());
        changed = true;
    }
    if data.selected_agent_id.trim().is_empty()
        || !data
            .agents
            .iter()
            .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
    {
        data.selected_agent_id = default_selected_agent_id();
        changed = true;
    }
    let desired_alias = user_persona_name(data);
    if data.user_alias != desired_alias {
        data.user_alias = desired_alias;
        changed = true;
    }
    let desired_style = normalize_response_style_id(&data.response_style_id);
    if data.response_style_id != desired_style {
        data.response_style_id = desired_style;
        changed = true;
    }
    changed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatSettings {
    selected_agent_id: String,
    user_alias: String,
    #[serde(default = "default_response_style_id")]
    response_style_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationApiSettings {
    chat_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
}

include!("app/storage_and_stt.rs");

include!("app/conversation.rs");

include!("app/model_runtime.rs");

include!("app/windowing.rs");

include!("app/commands.rs");

fn main() {
    let state = match AppState::new() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("Failed to initialize application state: {err}");
            return;
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            save_agent_avatar,
            clear_agent_avatar,
            read_avatar_data_url,
            save_conversation_api_settings,
            get_chat_snapshot,
            get_active_conversation_messages,
            get_prompt_preview,
            get_system_prompt_preview,
            list_archives,
            list_memories,
            export_memories,
            export_memories_to_file,
            export_memories_to_path,
            import_memories,
            get_archive_messages,
            open_external_url,
            send_chat_message,
            force_archive_current,
            refresh_models,
            check_tools_status,
            get_image_text_cache_stats,
            clear_image_text_cache,
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
    fn image_text_cache_upsert_and_find_should_work() {
        let mut data = AppData::default();
        upsert_image_text_cache(&mut data, "h1", "vision-a", "text-a");
        assert_eq!(
            find_image_text_cache(&data, "h1", "vision-a"),
            Some("text-a".to_string())
        );

        upsert_image_text_cache(&mut data, "h1", "vision-a", "text-b");
        assert_eq!(
            find_image_text_cache(&data, "h1", "vision-a"),
            Some("text-b".to_string())
        );
        assert_eq!(find_image_text_cache(&data, "h1", "vision-b"), None);
    }

    #[test]
    fn compute_image_hash_hex_should_be_stable() {
        let png_1x1_red = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO9WfXkAAAAASUVORK5CYII=";
        let part = BinaryPart {
            mime: "image/png".to_string(),
            bytes_base64: png_1x1_red.to_string(),
        };
        let h1 = compute_image_hash_hex(&part).expect("hash1");
        let h2 = compute_image_hash_hex(&part).expect("hash2");
        assert_eq!(h1, h2);
        assert!(!h1.is_empty());
    }

    #[test]
    fn normalize_app_config_should_fix_invalid_record_and_stt_fields() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            record_hotkey: "".to_string(),
            min_record_seconds: 0,
            max_record_seconds: 0,
            tool_max_iterations: 0,
            selected_api_config_id: "a1".to_string(),
            chat_api_config_id: "a1".to_string(),
            vision_api_config_id: None,
            api_configs: vec![
                ApiConfig {
                    id: "a1".to_string(),
                    name: "chat".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: true,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
                ApiConfig {
                    id: "a2".to_string(),
                    name: "bad-stt".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: false,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
            ],
        };
        normalize_app_config(&mut cfg);
        assert_eq!(cfg.record_hotkey, "Alt");
        assert_eq!(cfg.min_record_seconds, 1);
        assert!(cfg.max_record_seconds >= cfg.min_record_seconds);
        assert_eq!(cfg.tool_max_iterations, 1);
    }

    #[test]
    fn normalize_app_config_should_not_bind_chat_api_to_selected_api() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            record_hotkey: "Alt".to_string(),
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            selected_api_config_id: "edit-b".to_string(),
            chat_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            api_configs: vec![
                ApiConfig {
                    id: "chat-a".to_string(),
                    name: "chat-a".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: true,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
                ApiConfig {
                    id: "edit-b".to_string(),
                    name: "edit-b".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
            ],
        };
        normalize_app_config(&mut cfg);
        assert_eq!(cfg.selected_api_config_id, "edit-b".to_string());
        assert_eq!(cfg.chat_api_config_id, "chat-a".to_string());
    }

    #[test]
    fn normalize_app_config_should_disable_audio_capability_globally() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            record_hotkey: "Alt".to_string(),
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            selected_api_config_id: "tts-a".to_string(),
            chat_api_config_id: "tts-a".to_string(),
            vision_api_config_id: Some("tts-a".to_string()),
            api_configs: vec![ApiConfig {
                id: "tts-a".to_string(),
                name: "tts-a".to_string(),
                request_format: "openai_tts".to_string(),
                enable_text: true,
                enable_image: false,
                enable_audio: true,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.siliconflow.cn/v1/audio/transcriptions".to_string(),
                api_key: "k".to_string(),
                model: "m".to_string(),
                temperature: 1.0,
                context_window_tokens: 128_000,
            }],
        };
        normalize_app_config(&mut cfg);
        let api = &cfg.api_configs[0];
        assert!(api.enable_text);
        assert!(!api.enable_image);
        assert!(!api.enable_audio);
        assert!(api.enable_tools);
        assert_eq!(cfg.vision_api_config_id, None);
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
        let (full_text, reasoning_standard, reasoning_inline, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |kind, delta| {
                    if kind == "text" {
                        deltas.push(delta.to_string());
                    }
                },
            ))
            .expect("stream request should parse");

        sse_mock.assert();
        assert_eq!(deltas, vec!["你".to_string(), "好".to_string()]);
        assert_eq!(full_text, "你好".to_string());
        assert!(reasoning_standard.is_empty());
        assert!(reasoning_inline.is_empty());
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
        let (_full_text, _reasoning_standard, _reasoning_inline, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |_kind, _delta| {},
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

    fn test_text_message(role: &str, text: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_active_conversation_with_messages(
        messages: Vec<ChatMessage>,
        last_user_at: Option<String>,
    ) -> Conversation {
        let now = now_iso();
        Conversation {
            id: Uuid::new_v4().to_string(),
            title: "t".to_string(),
            api_config_id: "api".to_string(),
            agent_id: "agent".to_string(),
            created_at: now.clone(),
            updated_at: now,
            last_user_at,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            status: "active".to_string(),
            messages,
        }
    }

    #[test]
    fn build_prompt_should_include_structured_tool_history_messages() {
        let now = now_iso();
        let mut assistant_with_tool = test_text_message("assistant", "我去查一下", &now);
        assistant_with_tool.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "bing_search",
                        "arguments": "{\"query\":\"rust\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"results\":[{\"title\":\"Rust\"}]}"
            }),
        ]);

        let messages = vec![
            test_text_message("user", "帮我查 Rust", &now),
            assistant_with_tool,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));
        let agent = default_agent();

        let prepared = build_prompt(&conv, &agent, "用户", "我是...", DEFAULT_RESPONSE_STYLE_ID);

        assert!(
            prepared
                .history_messages
                .iter()
                .any(|m| m.role == "assistant" && m.tool_calls.is_some())
        );
        assert!(
            prepared.history_messages.iter().any(|m| {
                m.role == "tool"
                    && m.tool_call_id.as_deref() == Some("call_1")
                    && m.text.contains("\"results\"")
            })
        );
    }

    #[test]
    fn request_preview_should_keep_structured_tool_history_messages() {
        let api = ApiConfig {
            id: "api-a".to_string(),
            name: "api-a".to_string(),
            request_format: "openai".to_string(),
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://example.com/v1".to_string(),
            api_key: "k".to_string(),
            model: "gpt-x".to_string(),
            temperature: 0.7,
            context_window_tokens: 128_000,
        };
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "bing_search", "arguments": "{\"query\":\"rust\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"results\":[{\"title\":\"Rust\"}]}".to_string(),
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_system_text: "<time_context><utc>2026-02-11T17:30:45Z</utc></time_context>"
                .to_string(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let preview = build_request_preview_value(
            &api,
            &prepared,
            vec![
                serde_json::json!({"type":"text","text":"继续"}),
                serde_json::json!({"type":"text","text":prepared.latest_user_system_text}),
            ],
        );
        let messages = preview
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages array");
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("assistant")
                && m.get("tool_calls").and_then(Value::as_array).is_some()
        }));
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("tool")
                && m.get("tool_call_id").and_then(Value::as_str) == Some("call_1")
        }));
    }

    #[test]
    fn archive_decision_should_force_when_usage_reaches_82pct() {
        let now = now_iso();
        let huge = "中".repeat(2000);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &huge, &now)],
            Some(now),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(d.should_archive);
        assert!(d.forced);
        assert!(d.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_archive_after_30m_and_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let text = "中".repeat(600);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &text, &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio >= 0.30);
    }

    #[test]
    fn archive_decision_should_not_archive_when_usage_below_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "hello", &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio < 0.30);
    }
}
