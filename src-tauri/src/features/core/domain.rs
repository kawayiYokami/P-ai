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
        serde_json::from_str(include_str!("../../../../src/constants/response-styles.json")).unwrap_or_else(
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
        serde_json::from_str(include_str!("../../../../src/constants/highest-instruction.json"))
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
    #[serde(default = "default_true")]
    enabled: bool,
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
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "bing-search".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "bing-cn-mcp".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "memory-save".to_string(),
            command: "builtin".to_string(),
            args: vec!["memory-save".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "desktop-screenshot".to_string(),
            command: "builtin".to_string(),
            args: vec!["desktop-screenshot".to_string()],
            enabled: false,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "desktop-wait".to_string(),
            command: "builtin".to_string(),
            args: vec!["desktop-wait".to_string()],
            enabled: false,
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
    #[serde(default = "default_true")]
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

fn default_ui_language() -> String {
    "zh-CN".to_string()
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
            enable_tools: true,
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
    #[serde(default = "default_ui_language")]
    ui_language: String,
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
            ui_language: default_ui_language(),
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
struct GeminiNativeModelListItem {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiNativeModelListResponse {
    #[serde(default)]
    models: Vec<GeminiNativeModelListItem>,
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

