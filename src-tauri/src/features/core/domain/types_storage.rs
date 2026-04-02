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
