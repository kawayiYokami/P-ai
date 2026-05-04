const SHELL_WORKSPACE_LEVEL_SYSTEM: &str = "system";
const SHELL_WORKSPACE_LEVEL_MAIN: &str = "main";
const SHELL_WORKSPACE_LEVEL_SECONDARY: &str = "secondary";

const SHELL_WORKSPACE_ACCESS_APPROVAL: &str = "approval";
const SHELL_WORKSPACE_ACCESS_FULL_ACCESS: &str = "full_access";
const SHELL_WORKSPACE_ACCESS_READ_ONLY: &str = "read_only";

fn default_shell_workspace_level() -> String {
    SHELL_WORKSPACE_LEVEL_SECONDARY.to_string()
}

fn default_shell_workspace_access() -> String {
    SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string()
}

const CODEX_AUTH_MODE_READ_LOCAL: &str = "read_local";
const CODEX_AUTH_MODE_MANAGED_OAUTH: &str = "managed_oauth";
const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

fn default_codex_auth_mode() -> String {
    CODEX_AUTH_MODE_READ_LOCAL.to_string()
}

fn normalize_codex_auth_mode(value: &str) -> String {
    match value.trim() {
        CODEX_AUTH_MODE_MANAGED_OAUTH => CODEX_AUTH_MODE_MANAGED_OAUTH.to_string(),
        _ => CODEX_AUTH_MODE_READ_LOCAL.to_string(),
    }
}

fn default_codex_local_auth_path() -> String {
    "~/.codex/auth.json".to_string()
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

fn normalize_reasoning_effort(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => "low".to_string(),
        "high" => "high".to_string(),
        "xhigh" => "xhigh".to_string(),
        "none" => "none".to_string(),
        "minimal" => "minimal".to_string(),
        _ => "medium".to_string(),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellWorkspaceConfig {
    #[serde(default)]
    id: String,
    name: String,
    path: String,
    #[serde(default = "default_shell_workspace_level", alias = "role")]
    level: String,
    #[serde(default = "default_shell_workspace_access")]
    access: String,
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

fn default_department_permission_mode() -> String {
    "blacklist".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepartmentPermissionControl {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_department_permission_mode")]
    mode: String,
    #[serde(default)]
    builtin_tool_names: Vec<String>,
    #[serde(default)]
    skill_names: Vec<String>,
    #[serde(default)]
    mcp_tool_names: Vec<String>,
}

impl Default for DepartmentPermissionControl {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_department_permission_mode(),
            builtin_tool_names: Vec::new(),
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default)]
    is_deputy: bool,
    #[serde(default = "default_main_source")]
    source: String,
    #[serde(default = "default_global_scope")]
    scope: String,
    #[serde(default)]
    permission_control: DepartmentPermissionControl,
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
        is_deputy: false,
        source: default_main_source(),
        scope: default_global_scope(),
        permission_control: DepartmentPermissionControl::default(),
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
        agent_ids: vec![DEPUTY_AGENT_ID.to_string()],
        created_at: now.clone(),
        updated_at: now,
        order_index: 2,
        is_built_in_assistant: false,
        is_deputy: true,
        source: default_main_source(),
        scope: default_global_scope(),
        permission_control: DepartmentPermissionControl::default(),
    }
}

fn default_remote_customer_service_department(api_config_id: &str) -> DepartmentConfig {
    let now = now_iso();
    let api_config_id = api_config_id.trim().to_string();
    DepartmentConfig {
        id: REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string(),
        name: "远程客服".to_string(),
        summary: REMOTE_CUSTOMER_SERVICE_DEPARTMENT_SUMMARY.to_string(),
        guide: REMOTE_CUSTOMER_SERVICE_DEPARTMENT_GUIDE.to_string(),
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
        is_deputy: false,
        source: default_main_source(),
        scope: default_global_scope(),
        permission_control: DepartmentPermissionControl::default(),
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
        REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID => 2,
        _ => 3,
    }
}

fn default_departments(api_config_id: &str) -> Vec<DepartmentConfig> {
    vec![
        default_assistant_department(api_config_id),
        default_deputy_department(api_config_id),
        default_remote_customer_service_department(api_config_id),
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
struct ApiModelConfig {
    id: String,
    model: String,
    #[serde(default = "default_false")]
    enable_image: bool,
    #[serde(default = "default_true")]
    enable_tools: bool,
    #[serde(default = "default_reasoning_effort")]
    reasoning_effort: String,
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
}

impl Default for ApiModelConfig {
    fn default() -> Self {
        Self {
            id: "default-model".to_string(),
            model: "gpt-4o-mini".to_string(),
            enable_image: false,
            enable_tools: true,
            reasoning_effort: default_reasoning_effort(),
            temperature: default_api_temperature(),
            custom_temperature_enabled: false,
            context_window_tokens: default_context_window_tokens(),
            max_output_tokens: default_max_output_tokens(),
            custom_max_output_tokens_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProviderConfig {
    id: String,
    name: String,
    #[serde(default = "default_request_format")]
    request_format: RequestFormat,
    #[serde(default = "default_false")]
    allow_concurrent_requests: bool,
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
    #[serde(default = "default_codex_auth_mode")]
    codex_auth_mode: String,
    #[serde(default = "default_codex_local_auth_path")]
    codex_local_auth_path: String,
    #[serde(default)]
    api_keys: Vec<String>,
    #[serde(default)]
    key_cursor: u32,
    #[serde(default)]
    cached_model_options: Vec<String>,
    #[serde(default)]
    models: Vec<ApiModelConfig>,
    #[serde(default = "default_failure_retry_count")]
    failure_retry_count: u32,
}

impl Default for ApiProviderConfig {
    fn default() -> Self {
        Self {
            id: "default-provider-openai".to_string(),
            name: "Default OpenAI".to_string(),
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            api_keys: Vec::new(),
            key_cursor: 0,
            cached_model_options: vec!["gpt-4o-mini".to_string()],
            models: vec![ApiModelConfig::default()],
            failure_retry_count: default_failure_retry_count(),
        }
    }
}

fn default_api_providers() -> Vec<ApiProviderConfig> {
    vec![ApiProviderConfig::default()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiConfig {
    id: String,
    name: String,
    #[serde(default = "default_request_format")]
    request_format: RequestFormat,
    #[serde(default = "default_false")]
    allow_concurrent_requests: bool,
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
    #[serde(default = "default_codex_auth_mode")]
    codex_auth_mode: String,
    #[serde(default = "default_codex_local_auth_path")]
    codex_local_auth_path: String,
    model: String,
    #[serde(default = "default_reasoning_effort")]
    reasoning_effort: String,
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

fn default_llm_round_log_capacity() -> u32 {
    3
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

fn default_webview_zoom_percent() -> u32 {
    100
}

fn normalize_webview_zoom_percent(value: u32) -> u32 {
    const OPTIONS: [u32; 6] = [80, 90, 100, 110, 120, 150];
    OPTIONS
        .into_iter()
        .min_by_key(|option| option.abs_diff(value))
        .unwrap_or(default_webview_zoom_percent())
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

fn default_codex_context_window_tokens() -> u32 {
    272_000
}

fn codex_context_window_tokens_for_model(model_id: &str) -> u32 {
    match model_id.trim().to_ascii_lowercase().as_str() {
        "gpt-5.5" => 265_000,
        "gpt-5.4" | "gpt-5.4-mini" | "gpt-5.3-codex" | "gpt-5.2" => 272_000,
        "gpt-5.3-codex-spark" => 125_000,
        _ => default_codex_context_window_tokens(),
    }
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
            allow_concurrent_requests: false,
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            model: "gpt-4o-mini".to_string(),
            reasoning_effort: default_reasoning_effort(),
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
    #[serde(default = "default_webview_zoom_percent")]
    webview_zoom_percent: u32,
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
    #[serde(default = "default_llm_round_log_capacity")]
    llm_round_log_capacity: u32,
    selected_api_config_id: String,
    #[serde(default, alias = "chatApiConfigId")]
    assistant_department_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    tool_review_api_config_id: Option<String>,
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
    #[serde(default)]
    api_providers: Vec<ApiProviderConfig>,
    #[serde(default)]
    api_configs: Vec<ApiConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let api_config = ApiConfig::default();
        Self {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            webview_zoom_percent: default_webview_zoom_percent(),
            record_hotkey: default_record_hotkey(),
            record_background_wake_enabled: default_record_background_wake_enabled(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            tool_max_iterations: default_tool_max_iterations(),
            llm_round_log_capacity: default_llm_round_log_capacity(),
            selected_api_config_id: api_config.id.clone(),
            assistant_department_api_config_id: api_config.id.clone(),
            vision_api_config_id: None,
            tool_review_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: default_mcp_servers(),
            remote_im_channels: default_remote_im_channels(),
            departments: default_departments(&api_config.id),
            provider_non_stream_base_urls: default_provider_non_stream_base_urls(),
            api_providers: default_api_providers(),
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
