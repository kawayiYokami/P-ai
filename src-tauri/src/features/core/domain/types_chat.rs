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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationTodoItem {
    content: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Conversation {
    id: String,
    title: String,
    agent_id: String,
    #[serde(default)]
    department_id: String,
    #[serde(default)]
    last_read_message_id: String,
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
    user_profile_snapshot: String,
    #[serde(default)]
    shell_workspace_path: Option<String>,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default)]
    archived_at: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
}

#[derive(Debug, Clone)]
struct ConversationRuntimeSlot {
    state: MainSessionState,
    pending_queue: std::collections::VecDeque<ChatPendingEvent>,
    active_remote_im_activation_sources: Vec<RemoteImActivationSource>,
    plan_mode_enabled: bool,
    last_activity_at: String,
}

impl Default for ConversationRuntimeSlot {
    fn default() -> Self {
        Self {
            state: MainSessionState::Idle,
            pending_queue: std::collections::VecDeque::new(),
            active_remote_im_activation_sources: Vec::new(),
            plan_mode_enabled: false,
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
