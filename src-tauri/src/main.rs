use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex},
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
            agents: vec![default_agent()],
            selected_agent_id: default_selected_agent_id(),
            user_alias: default_user_alias(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
            memories: Vec::new(),
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
            save_conversation_api_settings,
            get_chat_snapshot,
            get_active_conversation_messages,
            list_archives,
            list_memories,
            get_archive_messages,
            open_external_url,
            send_chat_message,
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
