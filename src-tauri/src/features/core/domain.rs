const APP_DATA_SCHEMA_VERSION: u32 = 1;
const ARCHIVE_IDLE_SECONDS: i64 = 30 * 60;
const MAX_MULTIMODAL_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_AGENT_ID: &str = "default-agent";
const USER_PERSONA_ID: &str = "user-persona";
const SYSTEM_PERSONA_ID: &str = "system-persona";
const ASSISTANT_DEPARTMENT_ID: &str = "assistant-department";
const DEPUTY_DEPARTMENT_ID: &str = "deputy-department";
const FRONT_DESK_DEPARTMENT_ID: &str = "front-desk-department";
const DELEGATE_TOOL_KIND_DELEGATE: &str = "delegate";
const CONVERSATION_KIND_CHAT: &str = "chat";
const CONVERSATION_KIND_DELEGATE: &str = "delegate";
const CONVERSATION_KIND_REMOTE_IM_CONTACT: &str = "remote_im_contact";
const DEFAULT_RESPONSE_STYLE_ID: &str = "concise";
const DEFAULT_PDF_READ_MODE: &str = "image";
const DEFAULT_BACKGROUND_VOICE_SCREENSHOT_MODE: &str = "focused_window";
const CHAT_ABORTED_BY_USER_ERROR: &str = "CHAT_ABORTED_BY_USER";

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
        serde_json::from_str(include_str!(
            "../../../../src/constants/response-styles.json"
        ))
        .unwrap_or_else(|_| {
            vec![ResponseStylePreset {
                id: DEFAULT_RESPONSE_STYLE_ID.to_string(),
                name: "简洁".to_string(),
                prompt: "- 用最少但足够的信息回答。".to_string(),
            }]
        })
    })
}

fn default_response_style_id() -> String {
    DEFAULT_RESPONSE_STYLE_ID.to_string()
}

fn default_pdf_read_mode() -> String {
    DEFAULT_PDF_READ_MODE.to_string()
}

fn normalize_pdf_read_mode(value: &str) -> String {
    match value.trim() {
        "text" => "text".to_string(),
        "image" => "image".to_string(),
        _ => default_pdf_read_mode(),
    }
}

fn default_background_voice_screenshot_keywords() -> String {
    "看看,这个,屏幕上,see,look,watch".to_string()
}

fn default_background_voice_screenshot_mode() -> String {
    DEFAULT_BACKGROUND_VOICE_SCREENSHOT_MODE.to_string()
}

fn normalize_background_voice_screenshot_mode(value: &str) -> String {
    match value.trim() {
        "desktop" => "desktop".to_string(),
        "focused_window" => "focused_window".to_string(),
        _ => default_background_voice_screenshot_mode(),
    }
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
        serde_json::from_str(include_str!(
            "../../../../src/constants/highest-instruction.json"
        ))
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
    let title = if title.is_empty() {
        "系统准则"
    } else {
        title
    };
    let mut out = String::new();
    for rule in &source.rules {
        let line = rule.trim();
        if !line.is_empty() {
            out.push_str(line);
            out.push('\n');
        }
    }
    prompt_xml_block(title, out)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum RequestFormat {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "openai_responses")]
    OpenAIResponses,
    #[serde(rename = "openai_tts")]
    OpenAITts,
    #[serde(rename = "openai_stt")]
    OpenAIStt,
    #[serde(rename = "openai_embedding")]
    OpenAIEmbedding,
    #[serde(rename = "openai_rerank")]
    OpenAIRerank,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "gemini_embedding")]
    GeminiEmbedding,
    #[serde(rename = "deepseek/kimi")]
    DeepSeekKimi,
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl RequestFormat {
    fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAI),
            "openai_responses" => Some(Self::OpenAIResponses),
            "openai_tts" => Some(Self::OpenAITts),
            "openai_stt" => Some(Self::OpenAIStt),
            "openai_embedding" => Some(Self::OpenAIEmbedding),
            "openai_rerank" => Some(Self::OpenAIRerank),
            "gemini" => Some(Self::Gemini),
            "gemini_embedding" => Some(Self::GeminiEmbedding),
            "deepseek/kimi" => Some(Self::DeepSeekKimi),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::OpenAIResponses => "openai_responses",
            Self::OpenAITts => "openai_tts",
            Self::OpenAIStt => "openai_stt",
            Self::OpenAIEmbedding => "openai_embedding",
            Self::OpenAIRerank => "openai_rerank",
            Self::Gemini => "gemini",
            Self::GeminiEmbedding => "gemini_embedding",
            Self::DeepSeekKimi => "deepseek/kimi",
            Self::Anthropic => "anthropic",
        }
    }

    fn is_openai_stt(self) -> bool {
        matches!(self, Self::OpenAIStt)
    }

    fn is_gemini(self) -> bool {
        matches!(self, Self::Gemini)
    }

    fn is_anthropic(self) -> bool {
        matches!(self, Self::Anthropic)
    }

    fn is_deepseek_kimi(self) -> bool {
        matches!(self, Self::DeepSeekKimi)
    }

    fn is_openai_style(self) -> bool {
        matches!(
            self,
            Self::OpenAI | Self::OpenAIResponses | Self::DeepSeekKimi
        )
    }

    fn is_chat_text(self) -> bool {
        matches!(
            self,
            Self::OpenAI
                | Self::OpenAIResponses
                | Self::DeepSeekKimi
                | Self::Gemini
                | Self::Anthropic
        )
    }
}

impl std::fmt::Display for RequestFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for RequestFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::from_str(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!("unsupported request format '{}'", raw.trim()))
        })
    }
}

fn default_request_format() -> RequestFormat {
    RequestFormat::OpenAI
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
            id: "websearch".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "bing-cn-mcp".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "remember".to_string(),
            command: "builtin".to_string(),
            args: vec!["remember".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "recall".to_string(),
            command: "builtin".to_string(),
            args: vec!["recall".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "operate".to_string(),
            command: "builtin".to_string(),
            args: vec!["operate".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "exec".to_string(),
            command: "builtin".to_string(),
            args: vec!["exec".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "read_file".to_string(),
            command: "builtin".to_string(),
            args: vec!["read_file".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "apply_patch".to_string(),
            command: "builtin".to_string(),
            args: vec!["apply_patch".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "command".to_string(),
            command: "builtin".to_string(),
            args: vec!["command".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "task".to_string(),
            command: "builtin".to_string(),
            args: vec!["task".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "delegate".to_string(),
            command: "builtin".to_string(),
            args: vec!["delegate".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        },
        ApiToolConfig {
            id: "remote_im_send".to_string(),
            command: "builtin".to_string(),
            args: vec!["remote_im_send".to_string()],
            enabled: false,
            values: serde_json::json!({}),
        },
    ]
}

fn default_agent_tools() -> Vec<ApiToolConfig> {
    default_api_tools()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellWorkspaceConfig {
    name: String,
    path: String,
    #[serde(default)]
    built_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolPolicy {
    tool_name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpCachedTool {
    tool_name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpServerConfig {
    id: String,
    name: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    definition_json: String,
    #[serde(default)]
    tool_policies: Vec<McpToolPolicy>,
    #[serde(default)]
    cached_tools: Vec<McpCachedTool>,
    #[serde(default)]
    last_status: String,
    #[serde(default)]
    last_error: String,
    #[serde(default)]
    updated_at: String,
}

fn default_mcp_servers() -> Vec<McpServerConfig> {
    Vec::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepartmentConfig {
    id: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    guide: String,
    #[serde(default)]
    api_config_ids: Vec<String>,
    #[serde(default)]
    api_config_id: String,
    #[serde(default)]
    agent_ids: Vec<String>,
    created_at: String,
    updated_at: String,
    order_index: i64,
    #[serde(default)]
    is_built_in_assistant: bool,
    #[serde(default = "default_main_source")]
    source: String,
    #[serde(default = "default_global_scope")]
    scope: String,
}

fn default_main_source() -> String {
    "main_config".to_string()
}

fn default_private_workspace_source() -> String {
    "private_workspace".to_string()
}

fn default_global_scope() -> String {
    "global".to_string()
}

fn default_assistant_private_scope() -> String {
    "assistant_private".to_string()
}

fn default_assistant_department(api_config_id: &str) -> DepartmentConfig {
    let now = now_iso();
    let api_config_id = api_config_id.trim().to_string();
    DepartmentConfig {
        id: ASSISTANT_DEPARTMENT_ID.to_string(),
        name: "助理部门".to_string(),
        summary: "负责直接与用户对话，承接主会话与统筹调度。".to_string(),
        guide: "你是助理部门，负责作为主负责人理解用户需求、决定是否需要委派、汇总结果并继续推进主对话。".to_string(),
        api_config_ids: if api_config_id.is_empty() { Vec::new() } else { vec![api_config_id.clone()] },
        api_config_id,
        agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
        created_at: now.clone(),
        updated_at: now,
        order_index: 1,
        is_built_in_assistant: true,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_deputy_department(api_config_id: &str) -> DepartmentConfig {
    let now = now_iso();
    let api_config_id = api_config_id.trim().to_string();
    DepartmentConfig {
        id: DEPUTY_DEPARTMENT_ID.to_string(),
        name: "副手".to_string(),
        summary: "负责快速执行上级派发的明确任务，强调最小行动与严格边界。".to_string(),
        guide: "你是副手部门。你的核心原则是严格不越权、不擅自扩展需求、不多想。收到上级派发的任务后，用最少的工具调用、最快的速度完成明确目标；若信息不足或任务超出指令边界，就直接说明缺口并等待主部门继续决策。".to_string(),
        api_config_ids: if api_config_id.is_empty() { Vec::new() } else { vec![api_config_id.clone()] },
        api_config_id,
        agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
        created_at: now.clone(),
        updated_at: now,
        order_index: 2,
        is_built_in_assistant: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_front_desk_department(api_config_id: &str) -> DepartmentConfig {
    let now = now_iso();
    let api_config_id = api_config_id.trim().to_string();
    DepartmentConfig {
        id: FRONT_DESK_DEPARTMENT_ID.to_string(),
        name: "前台".to_string(),
        summary: "负责承接远程 IM 消息，简短友好应答，并把复杂任务转交主部门。".to_string(),
        guide: "你是前台部门，专门负责承接各个远程 IM 联系人的消息。说话必须简短、友好、有耐心，优先直接回答简单问题；遇到复杂任务、涉及多步骤分析、需要明显调度或你无法稳妥处理的需求时，应明确告知将转交主部门处理，不要自己展开复杂推理。".to_string(),
        api_config_ids: if api_config_id.is_empty() { Vec::new() } else { vec![api_config_id.clone()] },
        api_config_id,
        agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
        created_at: now.clone(),
        updated_at: now,
        order_index: 3,
        is_built_in_assistant: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_assistant_department_name(ui_language: &str) -> String {
    match ui_language.trim() {
        "en-US" => "Assistant Department".to_string(),
        "zh-TW" => "助理部門".to_string(),
        _ => "助理部门".to_string(),
    }
}

fn built_in_department_rank(id: &str) -> i32 {
    match id.trim() {
        ASSISTANT_DEPARTMENT_ID => 0,
        DEPUTY_DEPARTMENT_ID => 1,
        FRONT_DESK_DEPARTMENT_ID => 2,
        _ => 3,
    }
}

fn default_departments(api_config_id: &str) -> Vec<DepartmentConfig> {
    vec![
        default_assistant_department(api_config_id),
        default_deputy_department(api_config_id),
        default_front_desk_department(api_config_id),
    ]
}

fn department_api_config_ids(department: &DepartmentConfig) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for api_id in &department.api_config_ids {
        let api_id = api_id.trim().to_string();
        if api_id.is_empty() {
            continue;
        }
        let key = api_id.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(api_id);
        }
    }
    let legacy = department.api_config_id.trim().to_string();
    if !legacy.is_empty() {
        let key = legacy.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(legacy);
        }
    }
    out
}

fn department_primary_api_config_id(department: &DepartmentConfig) -> String {
    department_api_config_ids(department)
        .into_iter()
        .next()
        .unwrap_or_else(|| department.api_config_id.trim().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiConfig {
    id: String,
    name: String,
    #[serde(default = "default_request_format")]
    request_format: RequestFormat,
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
    #[serde(default = "default_max_output_tokens")]
    max_output_tokens: u32,
    #[serde(default = "default_failure_retry_count")]
    failure_retry_count: u32,
}

fn default_true() -> bool {
    true
}

fn default_record_hotkey() -> String {
    #[cfg(target_os = "macos")]
    {
        return "Option+Space".to_string();
    }
    #[cfg(not(target_os = "macos"))]
    {
        "Alt".to_string()
    }
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

fn default_failure_retry_count() -> u32 {
    0
}

fn default_provider_non_stream_base_urls() -> Vec<String> {
    Vec::new()
}

fn default_record_background_wake_enabled() -> bool {
    true
}

fn default_ui_language() -> String {
    "zh-CN".to_string()
}

fn default_ui_font() -> String {
    "auto".to_string()
}

fn default_terminal_shell_kind() -> String {
    "auto".to_string()
}

fn default_api_temperature() -> f64 {
    1.0
}

fn default_context_window_tokens() -> u32 {
    128_000
}

fn default_max_output_tokens() -> u32 {
    4_096
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            id: "default-openai".to_string(),
            name: "Default OpenAI".to_string(),
            request_format: RequestFormat::OpenAI,
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
            max_output_tokens: default_max_output_tokens(),
            failure_retry_count: default_failure_retry_count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RemoteImPlatform {
    Feishu,
    Dingtalk,
    #[serde(rename = "onebot_v11", alias = "napcat")]
    OnebotV11,
    #[serde(rename = "weixin_oc")]
    WeixinOc,
}

impl<'de> serde::Deserialize<'de> for RemoteImPlatform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        let normalized = raw.trim().to_ascii_lowercase();
        let platform = match normalized.as_str() {
            "feishu" => Self::Feishu,
            "dingtalk" => Self::Dingtalk,
            "onebot_v11" | "napcat" => Self::OnebotV11,
            "weixin_oc" => Self::WeixinOc,
            _ => {
                runtime_log_warn(format!(
                    "[RemoteImPlatform反序列化] 收到未知平台值: '{}' (规范化后: '{}'), 回退到OnebotV11",
                    raw, normalized
                ));
                Self::OnebotV11
            }
        };
        Ok(platform)
    }
}

fn default_remote_im_channel_activate_assistant() -> bool {
    true
}

fn default_remote_im_channel_receive_files() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImChannelConfig {
    id: String,
    name: String,
    platform: RemoteImPlatform,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    credentials: Value,
    #[serde(default = "default_remote_im_channel_activate_assistant")]
    activate_assistant: bool,
    #[serde(default = "default_remote_im_channel_receive_files")]
    receive_files: bool,
    #[serde(default)]
    streaming_send: bool,
    #[serde(default)]
    show_tool_calls: bool,
    #[serde(default)]
    allow_send_files: bool,
}

fn default_remote_im_channels() -> Vec<RemoteImChannelConfig> {
    Vec::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    hotkey: String,
    #[serde(default = "default_ui_language")]
    ui_language: String,
    #[serde(default = "default_ui_font")]
    ui_font: String,
    #[serde(default = "default_record_hotkey")]
    record_hotkey: String,
    #[serde(default = "default_record_background_wake_enabled")]
    record_background_wake_enabled: bool,
    #[serde(default = "default_min_record_seconds")]
    min_record_seconds: u32,
    #[serde(default = "default_max_record_seconds")]
    max_record_seconds: u32,
    #[serde(default = "default_tool_max_iterations")]
    tool_max_iterations: u32,
    selected_api_config_id: String,
    #[serde(default, alias = "chatApiConfigId")]
    assistant_department_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
    #[serde(default = "default_terminal_shell_kind")]
    terminal_shell_kind: String,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default = "default_mcp_servers")]
    mcp_servers: Vec<McpServerConfig>,
    #[serde(default = "default_remote_im_channels")]
    remote_im_channels: Vec<RemoteImChannelConfig>,
    #[serde(default)]
    departments: Vec<DepartmentConfig>,
    #[serde(default = "default_provider_non_stream_base_urls")]
    provider_non_stream_base_urls: Vec<String>,
    api_configs: Vec<ApiConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let api_config = ApiConfig::default();
        Self {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            record_hotkey: default_record_hotkey(),
            record_background_wake_enabled: default_record_background_wake_enabled(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            tool_max_iterations: default_tool_max_iterations(),
            selected_api_config_id: api_config.id.clone(),
            assistant_department_api_config_id: api_config.id.clone(),
            vision_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: default_mcp_servers(),
            remote_im_channels: default_remote_im_channels(),
            departments: default_departments(&api_config.id),
            provider_non_stream_base_urls: default_provider_non_stream_base_urls(),
            api_configs: vec![api_config],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugApiConfig {
    request_format: Option<RequestFormat>,
    base_url: String,
    api_key: String,
    model: String,
    temperature: Option<f64>,
    enabled: Option<bool>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfile {
    id: String,
    name: String,
    system_prompt: String,
    #[serde(default = "default_agent_tools")]
    tools: Vec<ApiToolConfig>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    avatar_path: Option<String>,
    #[serde(default)]
    avatar_updated_at: Option<String>,
    #[serde(default)]
    is_built_in_user: bool,
    #[serde(default)]
    is_built_in_system: bool,
    #[serde(default)]
    private_memory_enabled: bool,
    #[serde(default = "default_main_source")]
    source: String,
    #[serde(default = "default_global_scope")]
    scope: String,
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
    #[serde(default)]
    speaker_agent_id: Option<String>,
    parts: Vec<MessagePart>,
    #[serde(default)]
    extra_text_blocks: Vec<String>,
    provider_meta: Option<Value>,
    tool_call: Option<Vec<Value>>,
    mcp_call: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImMessageSource {
    channel_id: String,
    platform: RemoteImPlatform,
    im_name: String,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: String,
    sender_id: String,
    sender_name: String,
    #[serde(default)]
    sender_avatar_url: Option<String>,
    #[serde(default)]
    platform_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RemoteImActivationSource {
    channel_id: String,
    platform: RemoteImPlatform,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Conversation {
    id: String,
    title: String,
    agent_id: String,
    #[serde(default)]
    conversation_kind: String,
    #[serde(default)]
    root_conversation_id: Option<String>,
    #[serde(default)]
    delegate_id: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    last_user_at: Option<String>,
    last_assistant_at: Option<String>,
    #[serde(default)]
    last_context_usage_ratio: f64,
    #[serde(default)]
    last_effective_prompt_tokens: u64,
    status: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    archived_at: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
}

#[derive(Debug, Clone)]
struct ConversationRuntimeSlot {
    state: MainSessionState,
    pending_queue: std::collections::VecDeque<ChatPendingEvent>,
    active_remote_im_activation_sources: Vec<RemoteImActivationSource>,
    last_activity_at: String,
}

impl Default for ConversationRuntimeSlot {
    fn default() -> Self {
        Self {
            state: MainSessionState::Idle,
            pending_queue: std::collections::VecDeque::new(),
            active_remote_im_activation_sources: Vec::new(),
            last_activity_at: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct DelegateRuntimeThread {
    delegate_id: String,
    root_conversation_id: String,
    target_agent_id: String,
    title: String,
    call_stack: Vec<String>,
    parent_chat_session_key: Option<String>,
    archived_at: Option<String>,
    conversation: Conversation,
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
pub struct PdfTextCacheEntry {
    pub file_hash: String,
    pub file_path: String,
    pub file_name: String,
    pub extracted_text: String,
    pub total_pages: u32,
    pub extracted_pages: u32,
    pub is_truncated: bool,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfImageCacheEntry {
    pub file_hash: String,
    pub file_path: String,
    pub file_name: String,
    pub total_pages: u32,
    pub rendered_pages: u32,
    pub dpi: u32,
    pub images: Vec<PdfRenderedImage>,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfRenderedImage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub bytes_base64: String,
    pub mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEntry {
    id: String,
    #[serde(default, alias = "memoryType")]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, alias = "keywords")]
    tags: Vec<String>,
    #[serde(default)]
    owner_agent_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppData {
    version: u32,
    agents: Vec<AgentProfile>,
    #[serde(
        default = "default_assistant_department_agent_id",
        alias = "selectedAgentId",
        alias = "selected_agent_id"
    )]
    assistant_department_agent_id: String,
    #[serde(default = "default_user_alias")]
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
    main_conversation_id: Option<String>,
    conversations: Vec<Conversation>,
    #[serde(default, skip_serializing)]
    archived_conversations: Vec<ConversationArchive>,
    #[serde(default)]
    image_text_cache: Vec<ImageTextCacheEntry>,
    #[serde(default)]
    pdf_text_cache: Vec<PdfTextCacheEntry>,
    #[serde(default)]
    pdf_image_cache: Vec<PdfImageCacheEntry>,
    #[serde(default)]
    remote_im_contacts: Vec<RemoteImContact>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: APP_DATA_SCHEMA_VERSION,
            agents: vec![
                default_agent(),
                default_user_persona(),
                default_system_persona(),
            ],
            assistant_department_agent_id: default_assistant_department_agent_id(),
            user_alias: default_user_alias(),
            response_style_id: default_response_style_id(),
            pdf_read_mode: default_pdf_read_mode(),
            background_voice_screenshot_keywords: default_background_voice_screenshot_keywords(),
            background_voice_screenshot_mode: default_background_voice_screenshot_mode(),
            main_conversation_id: None,
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
            pdf_text_cache: Vec::new(),
            pdf_image_cache: Vec::new(),
            remote_im_contacts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContact {
    id: String,
    channel_id: String,
    platform: RemoteImPlatform,
    remote_contact_type: String,
    remote_contact_id: String,
    #[serde(default)]
    remote_contact_name: String,
    #[serde(default)]
    remark_name: String,
    #[serde(default)]
    allow_send: bool,
    #[serde(default)]
    allow_send_files: bool,
    #[serde(default)]
    allow_receive: bool,
    #[serde(default = "default_remote_im_contact_activation_mode")]
    activation_mode: String,
    #[serde(default)]
    activation_keywords: Vec<String>,
    #[serde(default)]
    activation_cooldown_seconds: u64,
    #[serde(default = "default_remote_im_contact_route_mode")]
    route_mode: String,
    #[serde(default)]
    bound_department_id: Option<String>,
    #[serde(default)]
    bound_conversation_id: Option<String>,
    #[serde(default = "default_remote_im_contact_processing_mode")]
    processing_mode: String,
    #[serde(default)]
    last_activated_at: Option<String>,
    #[serde(default)]
    last_message_at: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook_expired_time: Option<i64>,
}

fn default_assistant_department_agent_id() -> String {
    DEFAULT_AGENT_ID.to_string()
}

fn default_remote_im_contact_activation_mode() -> String {
    "never".to_string()
}

fn default_remote_im_contact_route_mode() -> String {
    "main_session".to_string()
}

fn default_remote_im_contact_processing_mode() -> String {
    "continuous".to_string()
}

fn default_user_alias() -> String {
    "用户".to_string()
}

fn assistant_department(config: &AppConfig) -> Option<&DepartmentConfig> {
    config
        .departments
        .iter()
        .find(|item| item.id == ASSISTANT_DEPARTMENT_ID || item.is_built_in_assistant)
}

fn assistant_department_mut(config: &mut AppConfig) -> Option<&mut DepartmentConfig> {
    config
        .departments
        .iter_mut()
        .find(|item| item.id == ASSISTANT_DEPARTMENT_ID || item.is_built_in_assistant)
}

fn assistant_department_agent_id(config: &AppConfig) -> Option<String> {
    assistant_department(config).and_then(|dept| {
        dept.agent_ids
            .iter()
            .find(|id| !id.trim().is_empty())
            .cloned()
    })
}

fn department_by_id<'a>(
    config: &'a AppConfig,
    department_id: &str,
) -> Option<&'a DepartmentConfig> {
    let trimmed = department_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    config.departments.iter().find(|item| item.id == trimmed)
}

fn department_for_agent_id<'a>(
    config: &'a AppConfig,
    agent_id: &str,
) -> Option<&'a DepartmentConfig> {
    let trimmed = agent_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    config
        .departments
        .iter()
        .find(|item| item.agent_ids.iter().any(|id| id.trim() == trimmed))
        .or_else(|| {
            if trimmed == DEFAULT_AGENT_ID {
                assistant_department(config)
            } else {
                None
            }
        })
}

fn tool_restricted_by_department(
    department: Option<&DepartmentConfig>,
    tool_id: &str,
) -> Option<String> {
    let department = department?;
    let is_assistant = department.id == ASSISTANT_DEPARTMENT_ID || department.is_built_in_assistant;
    if is_assistant {
        return None;
    }
    if !matches!(
        tool_id,
        "command" | "screenshot" | "operate" | "task" | "delegate" | "remote_im_send"
    ) {
        return None;
    }
    let department_name = department.name.trim();
    let department_name = if department_name.is_empty() {
        "当前部门"
    } else {
        department_name
    };
    Some(format!(
        "因为当前人格在 {department_name} 部门，本工具不被允许"
    ))
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
    request_format: RequestFormat,
    base_url: String,
    api_key: String,
    model: String,
    temperature: f64,
    max_output_tokens: u32,
}

#[derive(Debug, Clone)]
struct PreparedHistoryMessage {
    role: String,
    text: String,
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
        // Guardrail: never send an empty latest user turn to model providers.
        blocks.push(" ".to_string());
    }
    blocks
}

#[derive(Clone)]
struct AppState {
    app_handle: Arc<Mutex<Option<AppHandle>>>,
    config_path: PathBuf,
    data_path: PathBuf,
    llm_workspace_path: PathBuf,
    terminal_shell: TerminalShellProfile,
    terminal_shell_candidates: Vec<TerminalShellProfile>,
    state_lock: Arc<Mutex<()>>,
    // 运行态内存缓存：减少热路径重复读盘（配置）
    cached_config: Arc<Mutex<Option<AppConfig>>>,
    cached_config_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    // 运行态内存缓存：减少热路径重复读盘（业务数据）
    cached_app_data: Arc<Mutex<Option<AppData>>>,
    cached_app_data_mtime: Arc<Mutex<Option<std::time::SystemTime>>>,
    last_panic_snapshot: Arc<Mutex<Option<String>>>,
    inflight_chat_abort_handles: Arc<Mutex<std::collections::HashMap<String, AbortHandle>>>,
    inflight_tool_abort_handles: Arc<Mutex<std::collections::HashMap<String, AbortHandle>>>,
    terminal_session_roots: Arc<Mutex<std::collections::HashMap<String, String>>>,
    terminal_live_sessions: Arc<
        tokio::sync::Mutex<std::collections::HashMap<String, TerminalLiveShellSessionHandle>>,
    >,
    terminal_pending_approvals:
        Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    llm_round_logs: Arc<Mutex<std::collections::VecDeque<LlmRoundLogEntry>>>,
    // 主聊天会话级运行时
    conversation_runtime_slots:
        Arc<Mutex<std::collections::HashMap<String, ConversationRuntimeSlot>>>,
    conversation_processing_claims: Arc<Mutex<std::collections::HashSet<String>>>,
    pending_chat_result_senders: Arc<
        Mutex<
            std::collections::HashMap<
                String,
                tokio::sync::oneshot::Sender<Result<SendChatResult, String>>,
            >,
        >,
    >,
    pending_chat_delta_channels:
        Arc<Mutex<std::collections::HashMap<String, tauri::ipc::Channel<AssistantDeltaEvent>>>>,
    active_chat_view_bindings:
        Arc<Mutex<std::collections::HashMap<String, ActiveChatViewBinding>>>,
    dequeue_lock: Arc<Mutex<()>>,
    delegate_runtime_threads:
        Arc<Mutex<std::collections::HashMap<String, DelegateRuntimeThread>>>,
    delegate_recent_threads:
        Arc<Mutex<std::collections::VecDeque<DelegateRuntimeThread>>>,
    provider_streaming_disabled_keys: Arc<Mutex<std::collections::HashSet<String>>>,
    provider_system_message_user_fallback_keys:
        Arc<Mutex<std::collections::HashSet<String>>>,
    preferred_release_source: Arc<Mutex<String>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config_path", &self.config_path)
            .field("data_path", &self.data_path)
            .field("llm_workspace_path", &self.llm_workspace_path)
            .field("terminal_shell", &self.terminal_shell)
            .field("terminal_shell_candidates", &self.terminal_shell_candidates)
            .finish_non_exhaustive()
    }
}

impl AppState {
    fn new() -> Result<Self, String> {
        let legacy_project_dirs = ProjectDirs::from("ai", "easycall", "easy-call-ai")
            .ok_or_else(|| "Failed to resolve legacy config directory".to_string())?;
        let next_project_dirs = ProjectDirs::from("ai", "easycall", "p-ai")
            .ok_or_else(|| "Failed to resolve new config directory".to_string())?;
        let legacy_config_dir = legacy_project_dirs.config_dir().to_path_buf();
        let next_config_dir = next_project_dirs.config_dir().to_path_buf();
        let legacy_exists = legacy_config_dir.exists();
        let next_exists = next_config_dir.exists();
        let mut migrated_legacy_to_new = false;

        let config_dir = if next_exists {
            next_config_dir.clone()
        } else {
            if legacy_exists {
                if let Some(parent) = next_config_dir.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        format!(
                            "Create new config parent directory failed ({}): {err}",
                            parent.display()
                        )
                    })?;
                }
                fs::rename(&legacy_config_dir, &next_config_dir).map_err(|err| {
                    format!(
                        "Migrate legacy config directory failed ({} -> {}): {err}",
                        legacy_config_dir.display(),
                        next_config_dir.display()
                    )
                })?;
                migrated_legacy_to_new = true;
                next_config_dir.clone()
            } else {
                fs::create_dir_all(&next_config_dir).map_err(|err| {
                    format!(
                        "Create new config directory failed ({}): {err}",
                        next_config_dir.display()
                    )
                })?;
                next_config_dir.clone()
            }
        };
        eprintln!(
            "[配置路径] 固定使用 p-ai: selected={}, legacy_exists={}, next_exists_before={}, migrated_legacy_to_new={}",
            config_dir.display(),
            legacy_exists,
            next_exists,
            migrated_legacy_to_new
        );
        fs::create_dir_all(&config_dir)
            .map_err(|err| format!("Create config directory failed: {err}"))?;
        let app_root = config_dir
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| config_dir.clone());
        let legacy_app_root = legacy_config_dir
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| legacy_config_dir.clone());
        for dir_name in ["avatars", "media", "exports"] {
            let legacy = config_dir.join(dir_name);
            let target = app_root.join(dir_name);
            if legacy.exists() && !target.exists() {
                fs::rename(&legacy, &target).map_err(|err| {
                    format!(
                        "Migrate legacy {dir_name} dir failed ({} -> {}): {err}",
                        legacy.display(),
                        target.display()
                    )
                })?;
            }
        }
        let llm_workspace_path = app_root.join("llm-workspace");
        for legacy_llm_workspace_path in [
            legacy_app_root.join("llm-workspace"),
            config_dir.join("llm-workspace"),
        ] {
            if legacy_llm_workspace_path.exists() && !llm_workspace_path.exists() {
                fs::rename(&legacy_llm_workspace_path, &llm_workspace_path).map_err(|err| {
                    format!(
                        "Migrate llm workspace failed ({} -> {}): {err}",
                        legacy_llm_workspace_path.display(),
                        llm_workspace_path.display()
                    )
                })?;
                break;
            }
        }
        fs::create_dir_all(&llm_workspace_path)
            .map_err(|err| format!("Create llm workspace failed: {err}"))?;
        let terminal_shell_candidates = detect_terminal_shell_candidates();
        let terminal_shell = detect_default_terminal_shell();

        Ok(Self {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: config_dir.join("app_config.toml"),
            data_path: config_dir.join("app_data.json"),
            llm_workspace_path,
            terminal_shell,
            terminal_shell_candidates,
            state_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_mtime: Arc::new(Mutex::new(None)),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashSet::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
        })
    }
}

fn path_modified_time(path: &PathBuf) -> Option<std::time::SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

fn state_read_config_cached(state: &AppState) -> Result<AppConfig, String> {
    let disk_mtime = path_modified_time(&state.config_path);
    {
        let cached = state
            .cached_config
            .lock()
            .map_err(|_| "Failed to lock cached config".to_string())?;
        let cached_mtime = state
            .cached_config_mtime
            .lock()
            .map_err(|_| "Failed to lock cached config mtime".to_string())?;
        if let (Some(config), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(config.clone());
            }
        }
    }

    let config = read_config(&state.config_path)?;
    let disk_mtime = path_modified_time(&state.config_path);
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    Ok(config)
}

fn state_write_config_cached(state: &AppState, config: &AppConfig) -> Result<(), String> {
    write_config(&state.config_path, config)?;
    let disk_mtime = path_modified_time(&state.config_path);
    *state
        .cached_config
        .lock()
        .map_err(|_| "Failed to lock cached config".to_string())? = Some(config.clone());
    *state
        .cached_config_mtime
        .lock()
        .map_err(|_| "Failed to lock cached config mtime".to_string())? = disk_mtime;
    Ok(())
}

fn state_read_app_data_cached(state: &AppState) -> Result<AppData, String> {
    let disk_mtime = path_modified_time(&state.data_path);
    {
        let cached = state
            .cached_app_data
            .lock()
            .map_err(|_| "Failed to lock cached app data".to_string())?;
        let cached_mtime = state
            .cached_app_data_mtime
            .lock()
            .map_err(|_| "Failed to lock cached app data mtime".to_string())?;
        if let (Some(data), Some(cached_time), Some(disk_time)) =
            (cached.as_ref(), *cached_mtime, disk_mtime)
        {
            if cached_time == disk_time {
                return Ok(data.clone());
            }
        }
    }

    let data = read_app_data(&state.data_path)?;
    let disk_mtime = path_modified_time(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
    Ok(data)
}

fn state_write_app_data_cached(state: &AppState, data: &AppData) -> Result<(), String> {
    write_app_data(&state.data_path, data)?;
    let disk_mtime = path_modified_time(&state.data_path);
    *state
        .cached_app_data
        .lock()
        .map_err(|_| "Failed to lock cached app data".to_string())? = Some(data.clone());
    *state
        .cached_app_data_mtime
        .lock()
        .map_err(|_| "Failed to lock cached app data mtime".to_string())? = disk_mtime;
    Ok(())
}

fn app_root_from_data_path(data_path: &PathBuf) -> PathBuf {
    let parent = data_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    let is_config_dir = parent
        .file_name()
        .and_then(|v| v.to_str())
        .map(|v| v.eq_ignore_ascii_case("config"))
        .unwrap_or(false);
    if is_config_dir {
        if let Some(root) = parent.parent() {
            return root.to_path_buf();
        }
    }
    parent
}
fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn now_iso() -> String {
    now_utc_rfc3339()
}

fn parse_iso(value: &str) -> Option<OffsetDateTime> {
    parse_rfc3339_time(value)
}

static LAST_PANIC_SNAPSHOT_SLOT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn init_last_panic_snapshot_slot(slot: Arc<Mutex<Option<String>>>) {
    let _ = LAST_PANIC_SNAPSHOT_SLOT.set(slot);
}

fn last_panic_snapshot_text() -> String {
    LAST_PANIC_SNAPSHOT_SLOT
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|v| v.clone()))
        .unwrap_or_default()
}

fn state_lock_error_with_panic(
    file: &str,
    line: u32,
    module_path: &str,
    err: &dyn std::fmt::Display,
) -> String {
    let panic_snapshot = last_panic_snapshot_text();
    if panic_snapshot.trim().is_empty() {
        return format!(
            "Failed to lock state mutex at {}:{} {}: {err}",
            file, line, module_path
        );
    }
    format!(
        "Failed to lock state mutex at {}:{} {}: {err}; last panic: {}",
        file, line, module_path, panic_snapshot
    )
}

fn format_message_time_rfc3339_local(raw: &str) -> String {
    format_utc_storage_time_to_local_rfc3339(raw)
}

fn format_message_time_text(raw: &str) -> String {
    format_utc_storage_time_to_local_text(raw)
}

fn default_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: DEFAULT_AGENT_ID.to_string(),
        name: "助理".to_string(),
        system_prompt: "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_user_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: USER_PERSONA_ID.to_string(),
        name: "用户".to_string(),
        system_prompt: "我是...".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: true,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_system_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: SYSTEM_PERSONA_ID.to_string(),
        name: "凯瑟琳".to_string(),
        system_prompt: "我是系统人格，负责代表任务中心与系统调度向当前助手传达信息。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: true,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn normalize_agent_tools(agent: &mut AgentProfile) -> bool {
    let defaults = default_agent_tools();
    let mut next = Vec::<ApiToolConfig>::new();
    for default in defaults {
        if let Some(found) = agent.tools.iter().find(|tool| tool.id == default.id) {
            next.push(ApiToolConfig {
                id: default.id.clone(),
                command: if found.command.trim().is_empty() {
                    default.command.clone()
                } else {
                    found.command.clone()
                },
                args: if found.args.is_empty() {
                    default.args.clone()
                } else {
                    found.args.clone()
                },
                enabled: found.enabled,
                values: found.values.clone(),
            });
        } else {
            next.push(default);
        }
    }
    let changed = agent.tools.len() != next.len()
        || agent.tools.iter().zip(next.iter()).any(|(left, right)| {
            left.id != right.id
                || left.enabled != right.enabled
                || left.command != right.command
                || left.args != right.args
                || left.values != right.values
        });
    if changed {
        agent.tools = next;
    }
    changed
}

fn ensure_default_agent(data: &mut AppData) -> bool {
    let mut changed = false;
    let old_prompt = "You are a concise and helpful assistant.";
    let mut has_assistant = false;
    let mut has_user_persona = false;
    let mut has_system_persona = false;
    for agent in &mut data.agents {
        if normalize_agent_tools(agent) {
            changed = true;
        }
        if agent.source.trim().is_empty() {
            agent.source = default_main_source();
            changed = true;
        }
        if agent.scope.trim().is_empty() {
            agent.scope = default_global_scope();
            changed = true;
        }
        if agent.id == DEFAULT_AGENT_ID {
            has_assistant = true;
            if agent.is_built_in_user {
                agent.is_built_in_user = false;
                changed = true;
            }
            if agent.is_built_in_system {
                agent.is_built_in_system = false;
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
            if agent.is_built_in_system {
                agent.is_built_in_system = false;
                changed = true;
            }
        } else if agent.id == SYSTEM_PERSONA_ID {
            has_system_persona = true;
            if !agent.is_built_in_system {
                agent.is_built_in_system = true;
                changed = true;
            }
        } else if !agent.is_built_in_user && !agent.is_built_in_system {
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
    if !has_system_persona {
        data.agents.push(default_system_persona());
        changed = true;
    }
    if data.assistant_department_agent_id.trim().is_empty()
        || !data.agents.iter().any(|a| {
            a.id == data.assistant_department_agent_id
                && !a.is_built_in_user
                && !a.is_built_in_system
        })
    {
        data.assistant_department_agent_id = default_assistant_department_agent_id();
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
    let desired_pdf_read_mode = normalize_pdf_read_mode(&data.pdf_read_mode);
    if data.pdf_read_mode != desired_pdf_read_mode {
        data.pdf_read_mode = desired_pdf_read_mode;
        changed = true;
    }
    let desired_screenshot_mode =
        normalize_background_voice_screenshot_mode(&data.background_voice_screenshot_mode);
    if data.background_voice_screenshot_mode != desired_screenshot_mode {
        data.background_voice_screenshot_mode = desired_screenshot_mode;
        changed = true;
    }
    if data.background_voice_screenshot_keywords.trim().is_empty() {
        data.background_voice_screenshot_keywords = default_background_voice_screenshot_keywords();
        changed = true;
    }
    changed
}

fn fill_missing_message_speaker_agent_ids(data: &mut AppData) -> bool {
    fn provider_meta_speaker_agent_id(message: &ChatMessage) -> Option<String> {
        let meta = message.provider_meta.as_ref()?;
        let object = meta.as_object()?;
        for key in [
            "speakerAgentId",
            "speaker_agent_id",
            "targetAgentId",
            "target_agent_id",
            "agentId",
            "agent_id",
            "sourceAgentId",
            "source_agent_id",
        ] {
            let value = object
                .get(key)
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    }

    let mut changed = false;
    for conversation in &mut data.conversations {
        let host_agent_id = conversation.agent_id.trim().to_string();
        if host_agent_id.is_empty() {
            continue;
        }
        for message in &mut conversation.messages {
            let current = message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                .unwrap_or("");
            if current.is_empty() {
                message.speaker_agent_id =
                    Some(provider_meta_speaker_agent_id(message).unwrap_or_else(|| {
                        if message.role == "user" {
                            USER_PERSONA_ID.to_string()
                        } else {
                            host_agent_id.clone()
                        }
                    }));
                changed = true;
            }
        }
    }
    changed
}

fn fill_missing_conversation_metadata(data: &mut AppData) -> bool {
    let mut changed = false;
    for conversation in &mut data.conversations {
        if conversation.conversation_kind.trim().is_empty() {
            conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    for archive in &mut data.archived_conversations {
        if archive
            .source_conversation
            .conversation_kind
            .trim()
            .is_empty()
        {
            archive.source_conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    changed
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
