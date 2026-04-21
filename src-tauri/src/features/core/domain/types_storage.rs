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
    #[serde(default)]
    memory_no: Option<u64>,
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

impl MemoryEntry {
    fn display_id(&self) -> String {
        self.memory_no
            .map(|value| value.to_string())
            .unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptCommandPreset {
    id: String,
    name: String,
    prompt: String,
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
    instruction_presets: Vec<PromptCommandPreset>,
    #[serde(default)]
    main_conversation_id: Option<String>,
    #[serde(default)]
    pinned_conversation_ids: Vec<String>,
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
    #[serde(default)]
    remote_im_contact_checkpoints: Vec<RemoteImContactCheckpoint>,
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
            instruction_presets: Vec::new(),
            main_conversation_id: None,
            pinned_conversation_ids: Vec::new(),
            conversations: Vec::new(),
            archived_conversations: Vec::new(),
            image_text_cache: Vec::new(),
            pdf_text_cache: Vec::new(),
            pdf_image_cache: Vec::new(),
            remote_im_contacts: Vec::new(),
            remote_im_contact_checkpoints: Vec::new(),
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
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactCheckpoint {
    contact_id: String,
    #[serde(default)]
    latest_seen_message_id: Option<String>,
    #[serde(default)]
    last_boundary_message_id: Option<String>,
    #[serde(default)]
    last_boundary_covers_message_id: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

fn default_assistant_department_agent_id() -> String {
    DEFAULT_AGENT_ID.to_string()
}

fn default_remote_im_contact_activation_mode() -> String {
    "never".to_string()
}

fn default_remote_im_contact_patience_seconds() -> u64 {
    60
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DepartmentPermissionCategory {
    BuiltinTool,
    Skill,
    McpTool,
}

fn builtin_tool_is_fixed_system(tool_id: &str) -> bool {
    matches!(tool_id.trim(), "todo" | "remember" | "recall")
}

fn builtin_tool_is_local_conversation_fixed(tool_id: &str) -> bool {
    tool_id.trim() == "plan"
}

fn builtin_tool_is_contact_only_hidden(tool_id: &str) -> bool {
    matches!(
        tool_id.trim(),
        "contact_reply" | "contact_send_files" | "contact_no_reply"
    )
}

fn builtin_tool_is_department_controlled(tool_id: &str) -> bool {
    let trimmed = tool_id.trim();
    !trimmed.is_empty()
        && !builtin_tool_is_fixed_system(trimmed)
        && !builtin_tool_is_local_conversation_fixed(trimmed)
        && !builtin_tool_is_contact_only_hidden(trimmed)
}

fn builtin_tool_visible_in_department_permissions(tool_id: &str) -> bool {
    builtin_tool_is_department_controlled(tool_id)
}

fn normalize_department_permission_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "whitelist" => "whitelist".to_string(),
        _ => "blacklist".to_string(),
    }
}

fn normalize_department_permission_names(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn normalize_department_permission_control(
    raw: &DepartmentPermissionControl,
) -> DepartmentPermissionControl {
    DepartmentPermissionControl {
        enabled: raw.enabled,
        mode: normalize_department_permission_mode(&raw.mode),
        builtin_tool_names: normalize_department_permission_names(&raw.builtin_tool_names),
        skill_names: normalize_department_permission_names(&raw.skill_names),
        mcp_tool_names: normalize_department_permission_names(&raw.mcp_tool_names),
    }
}

fn department_permission_candidates<'a>(
    department: Option<&'a DepartmentConfig>,
    category: DepartmentPermissionCategory,
) -> Option<(&'a DepartmentPermissionControl, &'a [String])> {
    let department = department?;
    let control = &department.permission_control;
    if !control.enabled {
        return None;
    }
    let list = match category {
        DepartmentPermissionCategory::BuiltinTool => &control.builtin_tool_names,
        DepartmentPermissionCategory::Skill => &control.skill_names,
        DepartmentPermissionCategory::McpTool => &control.mcp_tool_names,
    };
    Some((control, list.as_slice()))
}

fn department_permission_allows_any_name(
    department: Option<&DepartmentConfig>,
    category: DepartmentPermissionCategory,
    candidate_names: &[&str],
) -> bool {
    let Some((control, list)) = department_permission_candidates(department, category) else {
        return true;
    };
    let matches = candidate_names.iter().any(|candidate| {
        let candidate = candidate.trim();
        !candidate.is_empty() && list.iter().any(|item| item == candidate)
    });
    if normalize_department_permission_mode(&control.mode) == "whitelist" {
        matches
    } else {
        !matches
    }
}

fn department_permission_mode_label(mode: &str) -> &'static str {
    if normalize_department_permission_mode(mode) == "whitelist" {
        "白名单"
    } else {
        "黑名单"
    }
}

fn department_permission_restricted_reason(
    department: Option<&DepartmentConfig>,
    category: DepartmentPermissionCategory,
    item_name: &str,
) -> Option<String> {
    let Some((control, _)) = department_permission_candidates(department, category) else {
        return None;
    };
    if department_permission_allows_any_name(department, category, &[item_name]) {
        return None;
    }
    let category_label = match category {
        DepartmentPermissionCategory::BuiltinTool => "工具",
        DepartmentPermissionCategory::Skill => "Skill",
        DepartmentPermissionCategory::McpTool => "MCP 工具",
    };
    Some(format!(
        "因为当前部门权限卡采用{}机制，{} `{}` 未被允许",
        department_permission_mode_label(&control.mode),
        category_label,
        item_name.trim()
    ))
}

fn tool_restricted_by_department(
    department: Option<&DepartmentConfig>,
    tool_id: &str,
) -> Option<String> {
    if !builtin_tool_is_department_controlled(tool_id) {
        return None;
    }
    let department = department?;
    let is_assistant = department.id == ASSISTANT_DEPARTMENT_ID || department.is_built_in_assistant;
    if !is_assistant
        && matches!(
            tool_id,
            "reload" | "organize_context" | "wait" | "screenshot" | "operate" | "task" | "delegate"
        )
    {
        let department_name = department.name.trim();
        let department_name = if department_name.is_empty() {
            "当前部门"
        } else {
            department_name
        };
        return Some(format!(
            "因为当前人格在 {department_name} 部门，本工具不被允许"
        ));
    }
    department_permission_restricted_reason(
        Some(department),
        DepartmentPermissionCategory::BuiltinTool,
        tool_id,
    )
}

fn tool_forced_by_department(
    department: Option<&DepartmentConfig>,
    tool_id: &str,
) -> bool {
    let _ = department;
    let _ = tool_id;
    false
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

#[cfg(test)]
mod types_storage_tests {
    use super::*;

    fn build_department_with_permission_control(
        mode: &str,
        builtin_tool_names: Vec<&str>,
        skill_names: Vec<&str>,
        mcp_tool_names: Vec<&str>,
    ) -> DepartmentConfig {
        let mut department = default_assistant_department("api-a");
        department.permission_control = DepartmentPermissionControl {
            enabled: true,
            mode: mode.to_string(),
            builtin_tool_names: builtin_tool_names.into_iter().map(|value| value.to_string()).collect(),
            skill_names: skill_names.into_iter().map(|value| value.to_string()).collect(),
            mcp_tool_names: mcp_tool_names.into_iter().map(|value| value.to_string()).collect(),
        };
        department
    }

    #[test]
    fn department_permission_allows_any_name_should_handle_whitelist_and_blacklist() {
        let whitelist = build_department_with_permission_control(
            "whitelist",
            vec!["fetch"],
            vec!["workspace-guide"],
            vec!["server-a::search"],
        );
        assert!(department_permission_allows_any_name(
            Some(&whitelist),
            DepartmentPermissionCategory::BuiltinTool,
            &["fetch"],
        ));
        assert!(!department_permission_allows_any_name(
            Some(&whitelist),
            DepartmentPermissionCategory::BuiltinTool,
            &["websearch"],
        ));
        assert!(department_permission_allows_any_name(
            Some(&whitelist),
            DepartmentPermissionCategory::McpTool,
            &["server-a::search", "search"],
        ));
        assert!(!department_permission_allows_any_name(
            Some(&whitelist),
            DepartmentPermissionCategory::Skill,
            &["mcp-setup"],
        ));

        let blacklist = build_department_with_permission_control(
            "blacklist",
            vec!["fetch"],
            vec!["workspace-guide"],
            vec!["server-a::search"],
        );
        assert!(!department_permission_allows_any_name(
            Some(&blacklist),
            DepartmentPermissionCategory::BuiltinTool,
            &["fetch"],
        ));
        assert!(department_permission_allows_any_name(
            Some(&blacklist),
            DepartmentPermissionCategory::BuiltinTool,
            &["websearch"],
        ));
        assert!(!department_permission_allows_any_name(
            Some(&blacklist),
            DepartmentPermissionCategory::McpTool,
            &["server-a::search", "search"],
        ));
    }
}
