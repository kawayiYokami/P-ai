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
        api_config_ids: if api_config_id.is_empty() {
            Vec::new()
        } else {
            vec![api_config_id.clone()]
        },
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
        api_config_ids: if api_config_id.is_empty() {
            Vec::new()
        } else {
            vec![api_config_id.clone()]
        },
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
        api_config_ids: if api_config_id.is_empty() {
            Vec::new()
        } else {
            vec![api_config_id.clone()]
        },
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
    #[serde(default = "default_false")]
    custom_temperature_enabled: bool,
    #[serde(default = "default_context_window_tokens")]
    context_window_tokens: u32,
    #[serde(default = "default_max_output_tokens")]
    max_output_tokens: u32,
    #[serde(default = "default_false")]
    custom_max_output_tokens_enabled: bool,
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
            custom_temperature_enabled: false,
            context_window_tokens: default_context_window_tokens(),
            max_output_tokens: default_max_output_tokens(),
            custom_max_output_tokens_enabled: false,
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
