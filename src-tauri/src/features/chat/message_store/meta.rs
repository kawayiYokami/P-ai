#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversationPersistMeta {
    id: String,
    title: String,
    agent_id: String,
    department_id: String,
    bound_conversation_id: Option<String>,
    parent_conversation_id: Option<String>,
    child_conversation_ids: Vec<String>,
    fork_message_cursor: Option<String>,
    unread_count: usize,
    conversation_kind: String,
    root_conversation_id: Option<String>,
    delegate_id: Option<String>,
    created_at: String,
    updated_at: String,
    last_user_at: Option<String>,
    last_assistant_at: Option<String>,
    status: String,
    summary: String,
    user_profile_snapshot: String,
    shell_workspace_path: Option<String>,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    archived_at: Option<String>,
    current_todos: Vec<ConversationTodoItem>,
    memory_recall_table: Vec<String>,
    plan_mode_enabled: bool,
}

impl ConversationPersistMeta {
    fn from_conversation(conversation: &Conversation) -> Self {
        ConversationShardMeta::from_conversation(conversation).to_persist_meta()
    }

    fn conversation_id(&self) -> &str {
        self.id.as_str()
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConversationPersistMessagesSnapshot {
    messages: Vec<ChatMessage>,
}

impl ConversationPersistMessagesSnapshot {
    fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            messages: conversation.messages.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationShardMeta {
    id: String,
    title: String,
    agent_id: String,
    #[serde(default)]
    department_id: String,
    #[serde(default)]
    bound_conversation_id: Option<String>,
    #[serde(default)]
    parent_conversation_id: Option<String>,
    #[serde(default)]
    child_conversation_ids: Vec<String>,
    #[serde(default)]
    fork_message_cursor: Option<String>,
    #[serde(default)]
    unread_count: usize,
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
    #[serde(default)]
    last_assistant_at: Option<String>,
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
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
}

impl ConversationShardMeta {
    pub(super) fn title(&self) -> &str {
        self.title.as_str()
    }

    pub(super) fn updated_at(&self) -> &str {
        self.updated_at.as_str()
    }

    pub(super) fn last_user_at(&self) -> Option<&str> {
        self.last_user_at.as_deref()
    }

    pub(super) fn last_assistant_at(&self) -> Option<&str> {
        self.last_assistant_at.as_deref()
    }

    fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            id: conversation.id.clone(),
            title: conversation.title.clone(),
            agent_id: conversation.agent_id.clone(),
            department_id: conversation.department_id.clone(),
            bound_conversation_id: conversation.bound_conversation_id.clone(),
            parent_conversation_id: conversation.parent_conversation_id.clone(),
            child_conversation_ids: conversation.child_conversation_ids.clone(),
            fork_message_cursor: conversation.fork_message_cursor.clone(),
            unread_count: conversation.unread_count,
            conversation_kind: conversation.conversation_kind.clone(),
            root_conversation_id: conversation.root_conversation_id.clone(),
            delegate_id: conversation.delegate_id.clone(),
            created_at: conversation.created_at.clone(),
            updated_at: conversation.updated_at.clone(),
            last_user_at: conversation.last_user_at.clone(),
            last_assistant_at: conversation.last_assistant_at.clone(),
            status: conversation.status.clone(),
            summary: conversation.summary.clone(),
            user_profile_snapshot: conversation.user_profile_snapshot.clone(),
            shell_workspace_path: conversation.shell_workspace_path.clone(),
            shell_workspaces: conversation.shell_workspaces.clone(),
            archived_at: conversation.archived_at.clone(),
            current_todos: conversation.current_todos.clone(),
            memory_recall_table: conversation.memory_recall_table.clone(),
            plan_mode_enabled: conversation.plan_mode_enabled,
        }
    }

    fn from_persist_meta(meta: &ConversationPersistMeta) -> Self {
        Self {
            id: meta.id.clone(),
            title: meta.title.clone(),
            agent_id: meta.agent_id.clone(),
            department_id: meta.department_id.clone(),
            bound_conversation_id: meta.bound_conversation_id.clone(),
            parent_conversation_id: meta.parent_conversation_id.clone(),
            child_conversation_ids: meta.child_conversation_ids.clone(),
            fork_message_cursor: meta.fork_message_cursor.clone(),
            unread_count: meta.unread_count,
            conversation_kind: meta.conversation_kind.clone(),
            root_conversation_id: meta.root_conversation_id.clone(),
            delegate_id: meta.delegate_id.clone(),
            created_at: meta.created_at.clone(),
            updated_at: meta.updated_at.clone(),
            last_user_at: meta.last_user_at.clone(),
            last_assistant_at: meta.last_assistant_at.clone(),
            status: meta.status.clone(),
            summary: meta.summary.clone(),
            user_profile_snapshot: meta.user_profile_snapshot.clone(),
            shell_workspace_path: meta.shell_workspace_path.clone(),
            shell_workspaces: meta.shell_workspaces.clone(),
            archived_at: meta.archived_at.clone(),
            current_todos: meta.current_todos.clone(),
            memory_recall_table: meta.memory_recall_table.clone(),
            plan_mode_enabled: meta.plan_mode_enabled,
        }
    }

    pub(super) fn to_persist_meta(&self) -> ConversationPersistMeta {
        ConversationPersistMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            agent_id: self.agent_id.clone(),
            department_id: self.department_id.clone(),
            bound_conversation_id: self.bound_conversation_id.clone(),
            parent_conversation_id: self.parent_conversation_id.clone(),
            child_conversation_ids: self.child_conversation_ids.clone(),
            fork_message_cursor: self.fork_message_cursor.clone(),
            unread_count: self.unread_count,
            conversation_kind: self.conversation_kind.clone(),
            root_conversation_id: self.root_conversation_id.clone(),
            delegate_id: self.delegate_id.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            last_user_at: self.last_user_at.clone(),
            last_assistant_at: self.last_assistant_at.clone(),
            status: self.status.clone(),
            summary: self.summary.clone(),
            user_profile_snapshot: self.user_profile_snapshot.clone(),
            shell_workspace_path: self.shell_workspace_path.clone(),
            shell_workspaces: self.shell_workspaces.clone(),
            archived_at: self.archived_at.clone(),
            current_todos: self.current_todos.clone(),
            memory_recall_table: self.memory_recall_table.clone(),
            plan_mode_enabled: self.plan_mode_enabled,
        }
    }

    fn into_conversation(self, messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: self.id,
            title: self.title,
            agent_id: self.agent_id,
            department_id: self.department_id,
            bound_conversation_id: self.bound_conversation_id,
            parent_conversation_id: self.parent_conversation_id,
            child_conversation_ids: self.child_conversation_ids,
            fork_message_cursor: self.fork_message_cursor,
            unread_count: self.unread_count,
            conversation_kind: self.conversation_kind,
            root_conversation_id: self.root_conversation_id,
            delegate_id: self.delegate_id,
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_user_at: self.last_user_at,
            last_assistant_at: self.last_assistant_at,
            status: self.status,
            summary: self.summary,
            user_profile_snapshot: self.user_profile_snapshot,
            shell_workspace_path: self.shell_workspace_path,
            shell_workspaces: self.shell_workspaces,
            archived_at: self.archived_at,
            messages,
            current_todos: self.current_todos,
            memory_recall_table: self.memory_recall_table,
            plan_mode_enabled: self.plan_mode_enabled,
        }
    }
}

fn write_conversation_shard_meta_atomic(
    path: &PathBuf,
    meta: &ConversationShardMeta,
) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta).map_err(|err| {
        format!(
            "序列化会话元数据失败，conversation_id={}，error={err}",
            meta.id
        )
    })?;
    write_message_store_text_atomic(path, "json.tmp", &raw, "会话元数据")
}

fn read_conversation_shard_meta(path: &PathBuf) -> Result<ConversationShardMeta, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取会话元数据失败，path={}，error={err}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("解析会话元数据失败，path={}，error={err}", path.display()))
}

#[cfg(test)]
mod message_store_meta_tests {
    use super::*;

    fn test_message(id: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "user".to_string(),
            created_at: "2026-04-24T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: format!("message {id}"),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_conversation() -> Conversation {
        Conversation {
            id: "conversation-meta".to_string(),
            title: "元数据会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            bound_conversation_id: Some("bound-a".to_string()),
            parent_conversation_id: Some("parent-a".to_string()),
            child_conversation_ids: vec!["child-a".to_string()],
            fork_message_cursor: Some("m1".to_string()),
            unread_count: 0,
            conversation_kind: "branch".to_string(),
            root_conversation_id: Some("root-a".to_string()),
            delegate_id: Some("delegate-a".to_string()),
            created_at: "2026-04-24T00:00:00Z".to_string(),
            updated_at: "2026-04-24T00:01:00Z".to_string(),
            last_user_at: Some("2026-04-24T00:00:30Z".to_string()),
            last_assistant_at: Some("2026-04-24T00:00:40Z".to_string()),
            status: "active".to_string(),
            summary: "summary".to_string(),
            user_profile_snapshot: "profile".to_string(),
            shell_workspace_path: Some("E:/workspace".to_string()),
            shell_workspaces: vec![ShellWorkspaceConfig {
                id: "workspace-a".to_string(),
                name: "workspace".to_string(),
                path: "E:/workspace".to_string(),
                level: "medium".to_string(),
                access: "workspace-write".to_string(),
                built_in: false,
            }],
            archived_at: None,
            messages: vec![test_message("m1"), test_message("m2")],
            current_todos: vec![ConversationTodoItem {
                content: "todo".to_string(),
                status: "pending".to_string(),
            }],
            memory_recall_table: vec!["memory-a".to_string()],
            plan_mode_enabled: true,
        }
    }

    #[test]
    fn message_store_meta_should_round_trip_without_messages() {
        let conversation = test_conversation();
        let meta = ConversationShardMeta::from_conversation(&conversation);
        let restored = meta.clone().into_conversation(conversation.messages.clone());
        let persist_meta = meta.to_persist_meta();

        assert_eq!(meta.id, conversation.id);
        assert_eq!(meta.title, conversation.title);
        assert_eq!(meta.current_todos, conversation.current_todos);
        assert_eq!(persist_meta, ConversationPersistMeta::from_conversation(&conversation));
        assert_eq!(restored.messages.len(), 2);
        assert_eq!(restored.id, conversation.id);
    }

    #[test]
    fn message_store_meta_file_should_not_contain_messages_array() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-meta-{}",
            Uuid::new_v4()
        ));
        let meta_path = root.join("chat").join("conversations").join("conversation-meta").join("meta.json");
        let conversation = test_conversation();
        let meta = ConversationShardMeta::from_conversation(&conversation);

        write_conversation_shard_meta_atomic(&meta_path, &meta).expect("write meta");
        let raw = fs::read_to_string(&meta_path).expect("read raw meta");
        let loaded = read_conversation_shard_meta(&meta_path).expect("read meta");

        assert!(!raw.contains("\"messages\""));
        assert_eq!(loaded, meta);
        let _ = fs::remove_dir_all(root);
    }
}
