const SHELL_WORKSPACE_LEVEL_SYSTEM: &str = "system";
const SHELL_WORKSPACE_LEVEL_MAIN: &str = "main";
const SHELL_WORKSPACE_LEVEL_SECONDARY: &str = "secondary";
const SHELL_WORK_MODE_DIRECTORY: &str = "directory";
const SHELL_WORK_MODE_WORKTREE: &str = "worktree";
// 兼容旧值：旧 isolated / independent 统一迁移为 worktree
const SHELL_WORK_MODE_ISOLATED_WORKTREE: &str = "isolated_worktree";
const SHELL_WORK_MODE_INDEPENDENT_WORKTREE: &str = "independent_worktree";

const SHELL_WORKSPACE_ACCESS_APPROVAL: &str = "approval";
const SHELL_WORKSPACE_ACCESS_FULL_ACCESS: &str = "full_access";
const SHELL_WORKSPACE_ACCESS_READ_ONLY: &str = "read_only";

fn default_shell_workspace_level() -> String {
    SHELL_WORKSPACE_LEVEL_SECONDARY.to_string()
}

fn default_shell_workspace_access() -> String {
    SHELL_WORKSPACE_ACCESS_APPROVAL.to_string()
}

fn default_shell_work_mode() -> String {
    SHELL_WORK_MODE_DIRECTORY.to_string()
}

fn normalize_shell_workspace_access_text(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        SHELL_WORKSPACE_ACCESS_FULL_ACCESS => SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
        SHELL_WORKSPACE_ACCESS_READ_ONLY => SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
        _ => SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
    }
}

fn normalize_shell_work_mode_text(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        SHELL_WORK_MODE_WORKTREE | SHELL_WORK_MODE_ISOLATED_WORKTREE | SHELL_WORK_MODE_INDEPENDENT_WORKTREE => SHELL_WORK_MODE_WORKTREE.to_string(),
        _ => SHELL_WORK_MODE_DIRECTORY.to_string(),
    }
}

fn normalize_shell_work_branch_text(raw: &str) -> String {
    let mut value = raw.trim().to_string();
    loop {
        let trimmed = value.trim();
        if let Some(rest) = trimmed.strip_prefix('*') {
            value = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('+') {
            value = rest.trim().to_string();
            continue;
        }
        break;
    }
    value.trim().to_string()
}

fn shell_work_mode_requires_git_root(mode: &str) -> bool {
    normalize_shell_work_mode_text(mode) == SHELL_WORK_MODE_WORKTREE
}

#[cfg(test)]
mod shell_work_branch_tests {
    use super::*;

    #[test]
    fn normalize_shell_work_branch_text_should_strip_markers() {
        assert_eq!(
            normalize_shell_work_branch_text("+ feat/storage-event-sourcing"),
            "feat/storage-event-sourcing"
        );
        assert_eq!(
            normalize_shell_work_branch_text("* feat/storage-event-sourcing"),
            "feat/storage-event-sourcing"
        );
        assert_eq!(
            normalize_shell_work_branch_text("  +   feat/storage-event-sourcing  "),
            "feat/storage-event-sourcing"
        );
        assert_eq!(normalize_shell_work_branch_text(""), "");
        assert_eq!(
            normalize_shell_work_branch_text("feature/backend-tantivy"),
            "feature/backend-tantivy"
        );
    }
}

const CODEX_AUTH_MODE_READ_LOCAL: &str = "read_local";
const CODEX_AUTH_MODE_MANAGED_OAUTH: &str = "managed_oauth";
const CODEX_AUTH_MODE_CUSTOM_URL: &str = "custom_url";
const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const MODEL_ROLE_EXPERT_API_CONFIG_ID: &str = "role:expert";
const MODEL_ROLE_QUICK_API_CONFIG_ID: &str = "role:quick";

fn default_codex_auth_mode() -> String {
    CODEX_AUTH_MODE_READ_LOCAL.to_string()
}

fn normalize_codex_auth_mode(value: &str) -> String {
    match value.trim() {
        CODEX_AUTH_MODE_MANAGED_OAUTH => CODEX_AUTH_MODE_MANAGED_OAUTH.to_string(),
        CODEX_AUTH_MODE_CUSTOM_URL => CODEX_AUTH_MODE_CUSTOM_URL.to_string(),
        _ => CODEX_AUTH_MODE_READ_LOCAL.to_string(),
    }
}

fn default_codex_originator() -> String {
    "codex-tui".to_string()
}

fn default_codex_local_auth_path() -> String {
    "~/.codex/auth.json".to_string()
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

fn normalize_reasoning_effort(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "default"
        || normalized == "low"
        || normalized == "high"
        || normalized == "xhigh"
        || normalized == "none"
        || normalized == "minimal"
        || normalized == "max"
    {
        normalized
    } else {
        normalized
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

fn default_permission_mode() -> String {
    "blacklist".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPermissionControl {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_permission_mode")]
    mode: String,
    #[serde(default)]
    builtin_tool_names: Vec<String>,
    #[serde(default)]
    skill_names: Vec<String>,
    #[serde(default)]
    mcp_tool_names: Vec<String>,
}

impl Default for AgentPermissionControl {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_permission_mode(),
            builtin_tool_names: Vec::new(),
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        }
    }
}

fn whitelist_permission_control(
    builtin_tool_names: &[&str],
    skill_names: &[&str],
) -> AgentPermissionControl {
    AgentPermissionControl {
        enabled: true,
        mode: "whitelist".to_string(),
        builtin_tool_names: builtin_tool_names
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        skill_names: skill_names
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        mcp_tool_names: Vec::new(),
    }
}

fn explorer_permission_control() -> AgentPermissionControl {
    whitelist_permission_control(
        &["read", "read_media", "exec", "fetch", "websearch"],
        &[
            "assistant-space-guide",
            "agents-md-setup",
            "memory-generation",
        ],
    )
}

fn reviewer_permission_control() -> AgentPermissionControl {
    whitelist_permission_control(
        &["read", "read_media", "fetch", "websearch", "exec"],
        &["code-review", "memory-generation"],
    )
}

fn saddler_permission_control() -> AgentPermissionControl {
    whitelist_permission_control(
        &["read", "write", "update", "exec"],
        &[
            "agents-md-setup",
            "assistant-space-guide",
            "memory-generation",
        ],
    )
}

fn leader_permission_control() -> AgentPermissionControl {
    whitelist_permission_control(
        &["read", "read_media", "exec", "fetch", "websearch", "delegate"],
        &["memory-generation"],
    )
}

fn support_permission_control() -> AgentPermissionControl {
    whitelist_permission_control(
        &[
            "read",
            "read_media",
            "fetch",
            "websearch",
            "meme",
            "image_generate",
            "image_edit",
        ],
        &["news-analyst", "memory-generation"],
    )
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

/// 保存配置的结果：归一化后的配置本体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveConfigOutput {
    config: AppConfig,
}

fn is_model_role_api_config_id(api_config_id: &str) -> bool {
    matches!(
        api_config_id.trim(),
        MODEL_ROLE_EXPERT_API_CONFIG_ID | MODEL_ROLE_QUICK_API_CONFIG_ID
    )
}

fn resolve_model_role_api_config_id(app_config: &AppConfig, api_config_id: &str) -> Option<String> {
    let api_config_id = api_config_id.trim();
    match api_config_id {
        MODEL_ROLE_EXPERT_API_CONFIG_ID => {
            let expert_id = app_config.expert_api_config_id.trim();
            (!expert_id.is_empty()).then(|| expert_id.to_string())
        }
        MODEL_ROLE_QUICK_API_CONFIG_ID => app_config
            .tool_review_api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        _ if !api_config_id.is_empty() => Some(api_config_id.to_string()),
        _ => None,
    }
}

fn normalize_agent_child_ids(values: &[String], self_id: &str) -> Vec<String> {
    let self_id = self_id.trim();
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed == self_id {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn child_edge_path_exists(
    adjacency: &std::collections::BTreeMap<String, Vec<String>>,
    start_id: &str,
    target_id: &str,
    skip_edge: Option<(&str, &str)>,
) -> bool {
    let start_id = start_id.trim();
    let target_id = target_id.trim();
    if start_id.is_empty() || target_id.is_empty() {
        return false;
    }
    let mut stack = vec![start_id.to_string()];
    let mut seen = std::collections::HashSet::<String>::new();
    while let Some(current_id) = stack.pop() {
        if current_id == target_id {
            return true;
        }
        if !seen.insert(current_id.clone()) {
            continue;
        }
        let Some(children) = adjacency.get(&current_id) else {
            continue;
        };
        for child_id in children.iter().rev() {
            if let Some((skip_parent, skip_child)) = skip_edge {
                if current_id == skip_parent && child_id == skip_child {
                    continue;
                }
            }
            stack.push(child_id.clone());
        }
    }
    false
}

/// 去掉会成环的直接下级边，返回被移除的 `(父 id, 子 id)`。
fn remove_cyclic_child_edges(
    children_by_parent: &mut std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<(String, String)> {
    let parent_ids = children_by_parent.keys().cloned().collect::<Vec<_>>();
    let mut removed = Vec::<(String, String)>::new();

    for parent_id in parent_ids {
        let children = children_by_parent.get(&parent_id).cloned().unwrap_or_default();
        let mut retained = Vec::<String>::new();
        for child_id in children {
            if child_edge_path_exists(
                children_by_parent,
                &child_id,
                &parent_id,
                Some((&parent_id, &child_id)),
            ) {
                removed.push((parent_id.clone(), child_id));
            } else {
                retained.push(child_id);
            }
        }
        children_by_parent.insert(parent_id, retained);
    }

    removed
}

fn merge_api_config_ids(api_config_ids: &[String], api_config_id: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for api_id in api_config_ids {
        let api_id = api_id.trim().to_string();
        if api_id.is_empty() {
            continue;
        }
        let key = api_id.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(api_id);
        }
    }
    let legacy = api_config_id.trim().to_string();
    if !legacy.is_empty() {
        let key = legacy.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(legacy);
        }
    }
    out
}

fn agent_api_config_ids(agent: &AgentProfile) -> Vec<String> {
    let mut ids = merge_api_config_ids(&agent.api_config_ids, &agent.api_config_id);
    // 人格未显式指定模型时，回退到默认「专家」模型角色。
    // 这是与「内置部门默认指向 role:expert」等价的人格化翻译：内置人格（含主助理）本就不带模型字段，
    // 任务派发与委托目标解析改走人格后，必须保留同一默认模型，否则内置人格将解析不到可用模型。
    if ids.is_empty() {
        ids.push(MODEL_ROLE_EXPERT_API_CONFIG_ID.to_string());
    }
    ids
}

fn agent_primary_api_config_id(agent: &AgentProfile) -> String {
    agent_api_config_ids(agent)
        .into_iter()
        .next()
        .unwrap_or_else(|| agent.api_config_id.trim().to_string())
}

fn resolve_chat_api_config_id(
    app_config: &AppConfig,
    raw_api_config_id: &str,
) -> Option<String> {
    let resolved_id = resolve_model_role_api_config_id(app_config, raw_api_config_id)?;
    app_config
        .api_configs
        .iter()
        .any(|api| api.id == resolved_id && is_text_chat_api(api))
        .then_some(resolved_id)
}

fn agent_primary_chat_api_config_id(
    app_config: &AppConfig,
    agent: &AgentProfile,
) -> Option<String> {
    resolve_chat_api_config_id(app_config, &agent_primary_api_config_id(agent))
}

fn effective_chat_api_config_ids(
    app_config: &AppConfig,
    raw_ids: Vec<String>,
    failure_fallback_enabled: bool,
) -> Vec<String> {
    let raw_ids = if failure_fallback_enabled {
        raw_ids
    } else {
        raw_ids.into_iter().take(1).collect()
    };
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for raw_id in raw_ids {
        let Some(resolved_id) = resolve_chat_api_config_id(app_config, &raw_id) else {
            continue;
        };
        if seen.insert(resolved_id.clone()) {
            out.push(resolved_id);
        }
    }
    out
}

fn agent_effective_chat_api_config_ids(
    app_config: &AppConfig,
    agent: &AgentProfile,
) -> Vec<String> {
    effective_chat_api_config_ids(
        app_config,
        agent_api_config_ids(agent),
        agent_model_failure_fallback_enabled(agent),
    )
}

// 请求失败自动切换下一个模型的机制已禁用：候选模型恒只取第一个，不再降级。
#[allow(dead_code)]
fn agent_model_failure_fallback_enabled(_agent: &AgentProfile) -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiModelConfig {
    id: String,
    model: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    deprecated: bool,
    #[serde(default = "default_false")]
    enable_image: bool,
    #[serde(default = "default_false")]
    enable_audio: bool,
    #[serde(default = "default_false")]
    enable_video: bool,
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
            display_name: String::new(),
            deprecated: false,
            enable_image: false,
            enable_audio: false,
            enable_video: false,
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
    #[serde(default)]
    deprecated: bool,
    #[serde(default = "default_request_format")]
    request_format: RequestFormat,
    #[serde(default = "default_false")]
    allow_concurrent_requests: bool,
    #[serde(default)]
    max_concurrent_requests: Option<u32>,
    #[serde(default = "default_true")]
    enable_text: bool,
    #[serde(default = "default_false")]
    enable_image: bool,
    #[serde(default = "default_false")]
    enable_audio: bool,
    #[serde(default = "default_false")]
    enable_video: bool,
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
    codex_custom_url: Option<String>,
    #[serde(default)]
    codex_custom_api_key: Option<String>,
    #[serde(default = "default_codex_originator")]
    codex_originator: String,
    #[serde(default)]
    codex_residency_requirement: Option<String>,
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
            deprecated: false,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            max_concurrent_requests: None,
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_video: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            codex_custom_url: None,
            codex_custom_api_key: None,
            codex_originator: default_codex_originator(),
            codex_residency_requirement: None,
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
    #[serde(default)]
    max_concurrent_requests: Option<u32>,
    #[serde(default = "default_true")]
    enable_text: bool,
    #[serde(default = "default_false")]
    enable_image: bool,
    #[serde(default = "default_false")]
    enable_audio: bool,
    #[serde(default = "default_false")]
    enable_video: bool,
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
    #[serde(default)]
    codex_custom_url: Option<String>,
    #[serde(default)]
    codex_custom_api_key: Option<String>,
    #[serde(default = "default_codex_originator")]
    codex_originator: String,
    #[serde(default)]
    codex_residency_requirement: Option<String>,
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
    "CapsLock".to_string()
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
    false
}

fn default_message_notification_enabled() -> bool {
    true
}

fn default_message_notification_sound_enabled() -> bool {
    false
}

fn default_desktop_operation_notice_enabled() -> bool {
    true
}

fn default_desktop_operate_enabled() -> bool {
    false
}

fn default_ui_language() -> String {
    "zh-CN".to_string()
}

fn default_ui_font() -> String {
    "auto".to_string()
}

fn default_code_font() -> String {
    "auto".to_string()
}

fn default_ui_size_scale() -> u16 {
    100
}

fn default_web_access_port() -> u16 {
    8429
}

fn default_web_access_enabled() -> bool {
    true
}

fn default_web_access_password() -> String {
    String::new()
}

fn generate_web_access_password() -> String {
    let raw = Uuid::new_v4().simple().to_string().to_uppercase();
    format!("{}-{}", &raw[0..4], &raw[4..8])
}

fn normalize_web_access_password(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return generate_web_access_password();
    }
    trimmed.chars().take(64).collect::<String>()
}

fn normalize_web_access_port(value: u16) -> u16 {
    if (1024..=65535).contains(&value) {
        value
    } else {
        default_web_access_port()
    }
}

fn default_github_update_method() -> String {
    "auto".to_string()
}

fn normalize_github_update_method(value: &str) -> String {
    match value.trim() {
        "direct" | "proxy" => value.trim().to_string(),
        _ => default_github_update_method(),
    }
}

fn default_skipped_github_update_version() -> String {
    String::new()
}

fn normalize_skipped_github_update_version(value: &str) -> String {
    value.trim().to_string()
}

/** 字体配置归一化：空值回落 auto，超长截断；auto 或合法字体名原样保留。 */
fn normalize_ui_font(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return default_ui_font();
    }
    trimmed.chars().take(128).collect::<String>()
}

fn normalize_ui_size_scale(value: u16) -> u16 {
    value.clamp(75, 150)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UiSizeScaleValue {
    Scale(u16),
    LegacyPreset(String),
}

fn deserialize_ui_size_scale<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = UiSizeScaleValue::deserialize(deserializer)?;
    let scale = match value {
        UiSizeScaleValue::Scale(scale) => scale,
        UiSizeScaleValue::LegacyPreset(preset) => match preset.trim() {
            "small" => 75,
            "default" => 100,
            "large" => 125,
            "extraLarge" => 150,
            _ => default_ui_size_scale(),
        },
    };
    Ok(normalize_ui_size_scale(scale))
}

fn default_terminal_shell_kind() -> String {
    "auto".to_string()
}

fn default_simple_setup_mode() -> bool {
    false
}

fn default_api_temperature() -> f64 {
    1.0
}

fn default_context_window_tokens() -> u32 {
    128_000
}

fn default_codex_context_window_tokens() -> u32 {
    262_144
}

fn codex_context_window_tokens_for_model(model_id: &str) -> u32 {
    match model_id.trim().to_ascii_lowercase().as_str() {
        // gpt-6 系按 gpt-5 系同级处理
        "gpt-6-astra"
        | "gpt-5.6-sol"
        | "gpt-5.6-terra"
        | "gpt-5.6-luna"
        | "gpt-5.5"
        | "gpt-5.4"
        | "gpt-5.4-mini"
        | "gpt-5.3-codex" => 262_144,
        "gpt-5.3-codex-spark" => 131_072,
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
            max_concurrent_requests: None,
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_video: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            codex_custom_url: None,
            codex_custom_api_key: None,
            codex_originator: default_codex_originator(),
            codex_residency_requirement: None,
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
    #[serde(default = "default_remote_im_channel_receive_files")]
    receive_files: bool,
    #[serde(default)]
    streaming_send: bool,
    #[serde(default)]
    show_tool_calls: bool,
    #[serde(default)]
    filter_markdown: bool,
    #[serde(default)]
    allow_send_files: bool,
    #[serde(default)]
    behavior_settings: RemoteImChannelBehaviorSettings,
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
    #[serde(default = "default_code_font")]
    code_font: String,
    #[serde(
        default = "default_ui_size_scale",
        alias = "uiSizePreset",
        alias = "ui_size_preset",
        deserialize_with = "deserialize_ui_size_scale"
    )]
    ui_size_scale: u16,
    #[serde(default = "default_web_access_port")]
    web_access_port: u16,
    #[serde(default = "default_web_access_enabled")]
    web_access_enabled: bool,
    #[serde(default = "default_web_access_password")]
    web_access_password: String,
    #[serde(default = "default_github_update_method")]
    github_update_method: String,
    #[serde(default = "default_skipped_github_update_version")]
    skipped_github_update_version: String,
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
    #[serde(default = "default_message_notification_enabled")]
    message_notification_enabled: bool,
    #[serde(default = "default_message_notification_sound_enabled")]
    message_notification_sound_enabled: bool,
    #[serde(default = "default_desktop_operation_notice_enabled")]
    desktop_operation_notice_enabled: bool,
    #[serde(default = "default_desktop_operate_enabled")]
    desktop_operate_enabled: bool,
    selected_api_config_id: String,
    #[serde(default, alias = "chatApiConfigId")]
    expert_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    tool_review_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    image_generation_model_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
    #[serde(default = "default_terminal_shell_kind")]
    terminal_shell_kind: String,
    #[serde(default = "default_simple_setup_mode")]
    simple_setup_mode: bool,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default = "default_mcp_servers")]
    mcp_servers: Vec<McpServerConfig>,
    #[serde(default = "default_remote_im_channels")]
    remote_im_channels: Vec<RemoteImChannelConfig>,
    #[serde(default = "default_provider_non_stream_base_urls")]
    provider_non_stream_base_urls: Vec<String>,
    #[serde(default)]
    api_providers: Vec<ApiProviderConfig>,
    #[serde(default = "default_image_generation_providers")]
    image_providers: Vec<ImageGenerationProviderConfig>,
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
            code_font: default_code_font(),
            ui_size_scale: default_ui_size_scale(),
            web_access_port: default_web_access_port(),
            web_access_enabled: default_web_access_enabled(),
            web_access_password: default_web_access_password(),
            github_update_method: default_github_update_method(),
            skipped_github_update_version: default_skipped_github_update_version(),
            record_hotkey: default_record_hotkey(),
            record_background_wake_enabled: default_record_background_wake_enabled(),
            min_record_seconds: default_min_record_seconds(),
            max_record_seconds: default_max_record_seconds(),
            tool_max_iterations: default_tool_max_iterations(),
            llm_round_log_capacity: default_llm_round_log_capacity(),
            message_notification_enabled: default_message_notification_enabled(),
            message_notification_sound_enabled: default_message_notification_sound_enabled(),
            desktop_operation_notice_enabled: default_desktop_operation_notice_enabled(),
            desktop_operate_enabled: default_desktop_operate_enabled(),
            selected_api_config_id: api_config.id.clone(),
            expert_api_config_id: api_config.id.clone(),
            vision_api_config_id: None,
            tool_review_api_config_id: None,
            stt_api_config_id: None,
            image_generation_model_id: None,
            stt_auto_send: false,
            terminal_shell_kind: default_terminal_shell_kind(),
            simple_setup_mode: true,
            shell_workspaces: Vec::new(),
            mcp_servers: default_mcp_servers(),
            remote_im_channels: default_remote_im_channels(),
            provider_non_stream_base_urls: default_provider_non_stream_base_urls(),
            api_providers: default_api_providers(),
            image_providers: default_image_generation_providers(),
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

#[cfg(test)]
mod codex_context_window_tests {
    use super::*;

    #[test]
    fn codex_context_window_should_use_256k_except_for_128k_spark() {
        for model in [
            "gpt-6-astra",
            "gpt-5.6-sol",
            "gpt-5.6-terra",
            "gpt-5.6-luna",
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.3-codex",
            "gpt-5.3-codex-spark",
        ] {
            let expected = if model == "gpt-5.3-codex-spark" {
                131_072
            } else {
                262_144
            };
            assert_eq!(codex_context_window_tokens_for_model(model), expected, "model: {model}");
        }
    }
}
