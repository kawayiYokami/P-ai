pub(super) const CONVERSATION_META_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationPersistMeta {
    meta_schema_version: u32,
    id: String,
    title: String,
    agent_id: String,
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
    user_profile_snapshot: String,
    shell_workspace_path: Option<String>,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    shell_autonomous_mode: bool,
    #[serde(default = "default_shell_work_mode")]
    shell_work_mode: String,
    #[serde(default)]
    shell_work_branch: String,
    archived_at: Option<String>,
    current_todos: Vec<ConversationTodoItem>,
    memory_recall_table: Vec<String>,
    plan_mode_enabled: bool,
    preferred_api_config_id: Option<String>,
    #[serde(default)]
    is_draft: bool,
    auto_push_remote_contact_id: Option<String>,
    cumulative_usage: ConversationCumulativeUsage,
    active_goal: Option<ConversationGoalState>,
    fast_request_turns: Vec<FastRequestTurn>,
    last_message_id: Option<String>,
    last_message_at: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    message_count: usize,
    body_message_count: usize,
    body_text_length: usize,
    has_assistant_reply: bool,
    has_context_compaction_message: bool,
    latest_summary_title: Option<String>,
    preview_messages: Vec<ConversationShardPreviewMessage>,
}

impl ConversationPersistMeta {
    pub(super) fn from_conversation(conversation: &Conversation) -> Self {
        ConversationShardMeta::from_conversation(conversation).to_persist_meta()
    }

    /// 基于 metadata 快照写局部 splice 时，不能用空 `Conversation.messages`
    /// 重算消息派生字段；只按实际替换的消息更新这些字段。
    pub(super) fn from_conversation_with_spliced_messages(
        conversation: &Conversation,
        source: &ConversationShardMeta,
        removed_messages: &[ChatMessage],
        inserted_messages: &[ChatMessage],
    ) -> Self {
        let mut meta = ConversationShardMeta::from_conversation(conversation);
        meta.preserve_message_derived_fields_from(source);
        meta.apply_spliced_messages(removed_messages, inserted_messages);
        if source
            .last_message_id
            .as_deref()
            .is_some_and(|tail_id| removed_messages.iter().any(|message| message.id == tail_id))
        {
            if let Some(last_inserted) = inserted_messages.last() {
                meta.last_message_id = Some(last_inserted.id.clone());
                meta.last_message_at = Some(last_inserted.created_at.clone());
            }
        }
        meta.to_persist_meta()
    }

    fn conversation_id(&self) -> &str {
        self.id.as_str()
    }

    fn apply_to_conversation(&self, target: &mut Conversation) -> Result<(), String> {
        let raw = serde_json::to_value(self)
            .map_err(|err| format!("序列化会话持久化 metadata 失败: {err}"))?;
        let meta = serde_json::from_value::<ConversationShardMeta>(raw)
            .map_err(|err| format!("解析会话持久化 metadata 失败: {err}"))?;
        meta.apply_to_conversation(target);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct ConversationPersistMessagesSnapshot {
    messages: Vec<ChatMessage>,
}

impl ConversationPersistMessagesSnapshot {
    pub(super) fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            messages: conversation.messages.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationShardPreviewMessage {
    pub(super) message_id: String,
    pub(super) role: String,
    #[serde(default)]
    pub(super) speaker_agent_id: Option<String>,
    #[serde(default)]
    pub(super) created_at: Option<String>,
    #[serde(default)]
    pub(super) text_preview: String,
    #[serde(default)]
    pub(super) has_image: bool,
    #[serde(default)]
    pub(super) has_pdf: bool,
    #[serde(default)]
    pub(super) has_audio: bool,
    #[serde(default)]
    pub(super) has_attachment: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct ConversationShardMeta {
    #[serde(default)]
    meta_schema_version: u32,
    id: String,
    title: String,
    agent_id: String,
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
    user_profile_snapshot: String,
    #[serde(default)]
    shell_workspace_path: Option<String>,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default)]
    shell_autonomous_mode: bool,
    #[serde(default = "default_shell_work_mode")]
    shell_work_mode: String,
    #[serde(default)]
    shell_work_branch: String,
    #[serde(default)]
    archived_at: Option<String>,
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
    #[serde(default)]
    preferred_api_config_id: Option<String>,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    auto_push_remote_contact_id: Option<String>,
    #[serde(default, alias = "usageSummary")]
    cumulative_usage: ConversationCumulativeUsage,
    #[serde(default)]
    active_goal: Option<ConversationGoalState>,
    #[serde(default)]
    fast_request_turns: Vec<FastRequestTurn>,
    #[serde(default)]
    last_message_id: Option<String>,
    #[serde(default)]
    last_message_at: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    message_count: usize,
    #[serde(default)]
    body_message_count: usize,
    #[serde(default)]
    body_text_length: usize,
    #[serde(default)]
    has_assistant_reply: bool,
    #[serde(default)]
    has_context_compaction_message: bool,
    #[serde(default)]
    latest_summary_title: Option<String>,
    #[serde(default)]
    preview_messages: Vec<ConversationShardPreviewMessage>,
}

impl ConversationShardMeta {
    fn preview_message_from_chat_message(
        message: &ChatMessage,
    ) -> Option<ConversationShardPreviewMessage> {
        if !matches!(
            message.role.trim().to_ascii_lowercase().as_str(),
            "user" | "assistant" | "tool"
        ) {
            return None;
        }
        let preview = ConversationShardPreviewMessage {
            message_id: message.id.clone(),
            role: message.role.clone(),
            speaker_agent_id: message.speaker_agent_id.clone(),
            created_at: Some(message.created_at.clone())
                .filter(|value| !value.trim().is_empty()),
            text_preview: super::build_conversation_preview_text(message),
            has_image: message.parts.iter().any(|part| {
                matches!(part, MessagePart::Image { mime, .. } if !mime.trim().eq_ignore_ascii_case("application/pdf"))
                    || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "image")
            }),
            has_pdf: message.parts.iter().any(|part| {
                matches!(part, MessagePart::Image { mime, .. } if mime.trim().eq_ignore_ascii_case("application/pdf"))
                    || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "pdf")
            }),
            has_audio: message
                .parts
                .iter()
                .any(|part| {
                    matches!(part, MessagePart::Audio { .. })
                        || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "audio")
                }),
            has_attachment: super::conversation_message_has_attachment(message),
        };
        if preview.text_preview.trim().is_empty()
            && !preview.has_image
            && !preview.has_pdf
            && !preview.has_audio
            && !preview.has_attachment
        {
            return None;
        }
        Some(preview)
    }

    fn body_text_length_for_message(message: &ChatMessage) -> usize {
        if !matches!(
            message.role.trim().to_ascii_lowercase().as_str(),
            "user" | "assistant"
        ) {
            return 0;
        }
        message
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.trim().chars().count()),
                _ => None,
            })
            .sum()
    }

    pub(super) fn schema_version(&self) -> u32 {
        self.meta_schema_version
    }

    pub(super) fn id(&self) -> &str {
        self.id.as_str()
    }

    pub(super) fn title(&self) -> &str {
        self.title.as_str()
    }

    pub(super) fn agent_id(&self) -> &str {
        self.agent_id.as_str()
    }

    pub(super) fn conversation_kind(&self) -> &str {
        self.conversation_kind.as_str()
    }

    pub(super) fn root_conversation_id(&self) -> Option<&str> {
        self.root_conversation_id.as_deref()
    }

    pub(super) fn delegate_id(&self) -> Option<&str> {
        self.delegate_id.as_deref()
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

    pub(super) fn archived_at(&self) -> Option<&str> {
        self.archived_at.as_deref()
    }

    pub(super) fn preferred_api_config_id(&self) -> Option<&str> {
        self.preferred_api_config_id.as_deref()
    }


    pub(super) fn auto_push_remote_contact_id(&self) -> Option<&str> {
        self.auto_push_remote_contact_id.as_deref()
    }

    pub(super) fn user_profile_snapshot(&self) -> &str {
        self.user_profile_snapshot.as_str()
    }

    pub(super) fn status(&self) -> &str {
        self.status.as_str()
    }

    pub(super) fn unread_count(&self) -> usize {
        self.unread_count
    }

    pub(super) fn root_conversation_id_text(&self) -> Option<&str> {
        self.root_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(super) fn message_count(&self) -> usize {
        self.message_count
    }

    pub(super) fn body_message_count(&self) -> usize {
        self.body_message_count
    }

    pub(super) fn body_text_length(&self) -> usize {
        self.body_text_length
    }

    pub(super) fn has_assistant_reply(&self) -> bool {
        self.has_assistant_reply
    }

    pub(super) fn has_context_compaction_message(&self) -> bool {
        self.has_context_compaction_message
    }

    pub(super) fn last_message_id(&self) -> Option<&str> {
        self.last_message_id.as_deref()
    }

    pub(super) fn latest_summary_title(&self) -> Option<&str> {
        self.latest_summary_title.as_deref()
    }

    pub(super) fn last_message_at(&self) -> Option<&str> {
        self.last_message_at.as_deref()
    }

    pub(super) fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub(super) fn created_at(&self) -> &str {
        self.created_at.as_str()
    }

    pub(super) fn current_todos(&self) -> &[ConversationTodoItem] {
        self.current_todos.as_slice()
    }

    pub(super) fn parent_conversation_id(&self) -> Option<&str> {
        self.parent_conversation_id.as_deref()
    }

    pub(super) fn child_conversation_ids(&self) -> &[String] {
        self.child_conversation_ids.as_slice()
    }

    pub(super) fn fork_message_cursor(&self) -> Option<&str> {
        self.fork_message_cursor.as_deref()
    }

    pub(super) fn shell_workspace_path(&self) -> Option<&str> {
        self.shell_workspace_path.as_deref()
    }

    pub(super) fn shell_workspaces(&self) -> &[ShellWorkspaceConfig] {
        self.shell_workspaces.as_slice()
    }

    pub(super) fn shell_autonomous_mode(&self) -> bool {
        self.shell_autonomous_mode
    }

    pub(super) fn shell_work_mode(&self) -> &str {
        self.shell_work_mode.as_str()
    }

    pub(super) fn shell_work_branch(&self) -> &str {
        self.shell_work_branch.as_str()
    }

    pub(super) fn plan_mode_enabled(&self) -> bool {
        self.plan_mode_enabled
    }

    pub(super) fn is_draft(&self) -> bool {
        self.is_draft
    }

    pub(super) fn active_goal(&self) -> Option<&ConversationGoalState> {
        self.active_goal.as_ref()
    }

    pub(super) fn fast_request_turns(&self) -> &[FastRequestTurn] {
        self.fast_request_turns.as_slice()
    }

    pub(super) fn push_fast_request_turn(&mut self, turn: FastRequestTurn) {
        self.fast_request_turns.push(turn);
    }

    pub(super) fn retain_fast_request_turns(
        &mut self,
        keep: impl FnMut(&FastRequestTurn) -> bool,
    ) {
        self.fast_request_turns.retain(keep);
    }

    pub(super) fn cumulative_usage(&self) -> &ConversationCumulativeUsage {
        &self.cumulative_usage
    }

    pub(super) fn normalized_legacy_usage_totals(mut self) -> Self {
        self.cumulative_usage = self.cumulative_usage.clone().normalized_legacy_totals();
        self
    }

    pub(super) fn preview_messages(&self) -> &[ConversationShardPreviewMessage] {
        self.preview_messages.as_slice()
    }

    pub(super) fn apply_to_conversation(&self, target: &mut Conversation) {
        target.title = self.title.clone();
        target.agent_id = self.agent_id.clone();
        target.bound_conversation_id = self.bound_conversation_id.clone();
        target.parent_conversation_id = self.parent_conversation_id.clone();
        target.child_conversation_ids = self.child_conversation_ids.clone();
        target.fork_message_cursor = self.fork_message_cursor.clone();
        target.unread_count = self.unread_count;
        target.conversation_kind = self.conversation_kind.clone();
        target.root_conversation_id = self.root_conversation_id.clone();
        target.delegate_id = self.delegate_id.clone();
        target.created_at = self.created_at.clone();
        target.updated_at = self.updated_at.clone();
        target.last_user_at = self.last_user_at.clone();
        target.last_assistant_at = self.last_assistant_at.clone();
        target.status = self.status.clone();
        target.user_profile_snapshot = self.user_profile_snapshot.clone();
        target.shell_workspace_path = self.shell_workspace_path.clone();
        target.shell_workspaces = self.shell_workspaces.clone();
        target.shell_autonomous_mode = self.shell_autonomous_mode;
        target.shell_work_mode = self.shell_work_mode.clone();
        target.shell_work_branch = self.shell_work_branch.clone();
        target.archived_at = self.archived_at.clone();
        target.current_todos = self.current_todos.clone();
        target.memory_recall_table = self.memory_recall_table.clone();
        target.plan_mode_enabled = self.plan_mode_enabled;
        target.preferred_api_config_id = self.preferred_api_config_id.clone();
        target.is_draft = self.is_draft;
        target.auto_push_remote_contact_id = self.auto_push_remote_contact_id.clone();
        target.cumulative_usage = self.cumulative_usage.clone();
        target.active_goal = self.active_goal.clone();
        target.fast_request_turns = self.fast_request_turns.clone();
        target.last_error = self.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_conversation(&mut self, source: &Conversation) {
        self.title = source.title.clone();
        self.agent_id = source.agent_id.clone();
        self.bound_conversation_id = source.bound_conversation_id.clone();
        self.parent_conversation_id = source.parent_conversation_id.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.conversation_kind = source.conversation_kind.clone();
        self.root_conversation_id = source.root_conversation_id.clone();
        self.delegate_id = source.delegate_id.clone();
        self.created_at = source.created_at.clone();
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.status = source.status.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.memory_recall_table = source.memory_recall_table.clone();
        self.plan_mode_enabled = source.plan_mode_enabled;
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.fast_request_turns = source.fast_request_turns.clone();
        self.last_error = source.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_meta(&mut self, source: &ConversationShardMeta) {
        self.title = source.title.clone();
        self.agent_id = source.agent_id.clone();
        self.bound_conversation_id = source.bound_conversation_id.clone();
        self.parent_conversation_id = source.parent_conversation_id.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.conversation_kind = source.conversation_kind.clone();
        self.root_conversation_id = source.root_conversation_id.clone();
        self.delegate_id = source.delegate_id.clone();
        self.created_at = source.created_at.clone();
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.status = source.status.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.memory_recall_table = source.memory_recall_table.clone();
        self.plan_mode_enabled = source.plan_mode_enabled;
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.last_error = source.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_meta_view(&mut self, source: &ConversationMetaView) {
        self.title = source.title.clone();
        self.agent_id = source.agent_id.clone();
        self.parent_conversation_id = source.parent_conversation_id.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.conversation_kind = source.conversation_kind.clone();
        self.root_conversation_id = source.root_conversation_id.clone();
        self.delegate_id = source.delegate_id.clone();
        self.created_at = source.created_at.clone();
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.status = source.status.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.plan_mode_enabled = source.plan_mode_enabled;
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.last_error = source.last_error.clone();
    }

    pub(super) fn preserve_message_derived_fields_from(&mut self, source: &ConversationShardMeta) {
        self.last_message_id = source.last_message_id.clone();
        self.last_message_at = source.last_message_at.clone();
        self.message_count = source.message_count;
        self.body_message_count = source.body_message_count;
        self.body_text_length = source.body_text_length;
        self.has_assistant_reply = source.has_assistant_reply;
        self.has_context_compaction_message = source.has_context_compaction_message;
        self.latest_summary_title = source.latest_summary_title.clone();
        self.preview_messages = source.preview_messages.clone();
    }

    pub(super) fn apply_appended_messages(&mut self, messages: &[ChatMessage]) {
        if messages.is_empty() {
            return;
        }
        self.message_count = self.message_count.saturating_add(messages.len());
        self.body_message_count = self
            .body_message_count
            .saturating_add(messages.iter().filter(|message| {
                matches!(
                    message.role.trim().to_ascii_lowercase().as_str(),
                    "user" | "assistant"
                )
            }).count());
        self.body_text_length = self.body_text_length.saturating_add(
            messages
                .iter()
                .flat_map(|message| message.parts.iter())
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim().chars().count()),
                    _ => None,
                })
                .sum::<usize>(),
        );
        if messages
            .iter()
            .any(|message| {
                message.role.trim().eq_ignore_ascii_case("assistant")
                    && Self::preview_message_from_chat_message(message).is_some()
            })
        {
            self.has_assistant_reply = true;
        }
        if messages.iter().any(|message| {
            super::is_context_compaction_message(message, message.role.trim())
        }) {
            self.has_context_compaction_message = true;
        }
        if let Some(last_title) = messages
            .iter()
            .rev()
            .find_map(super::summary_context_message_title)
        {
            self.latest_summary_title = Some(last_title);
        }
        if let Some(last_message) = messages.last() {
            self.last_message_id = Some(last_message.id.clone());
            self.last_message_at = Some(last_message.created_at.clone());
        }
        let mut preview_messages = self.preview_messages.clone();
        preview_messages.extend(
            messages
                .iter()
                .filter_map(Self::preview_message_from_chat_message),
        );
        if preview_messages.len() > 2 {
            let keep_from = preview_messages.len().saturating_sub(2);
            preview_messages = preview_messages[keep_from..].to_vec();
        }
        self.preview_messages = preview_messages;
    }

    /// 统一 replace 派生更新：按位置一一对应的前后消息对，更新计数以外的缓存派生字段。
    /// 当任何一对被替换消息的摘要标题状态发生变化（新增/删除/改写标题）时，
    /// 调用 `recompute_latest_summary_title` 取得替换后的最新摘要标题并写回；
    /// 该闭包由调用方按自身掌握的消息范围提供（全量会话重算或轻量摘要读取）。
    pub(super) fn apply_replaced_messages(
        &mut self,
        previous_messages: &[ChatMessage],
        updated_messages: &[ChatMessage],
        recompute_latest_summary_title: impl FnOnce() -> Result<Option<String>, String>,
    ) -> Result<(), String> {
        let pair_count = previous_messages.len().min(updated_messages.len());
        for index in 0..pair_count {
            self.apply_replaced_message(&previous_messages[index], &updated_messages[index]);
        }
        let summary_title_touched = previous_messages
            .iter()
            .zip(updated_messages.iter())
            .any(|(previous, updated)| {
                super::summary_context_message_title(previous)
                    != super::summary_context_message_title(updated)
            });
        if summary_title_touched {
            self.latest_summary_title = recompute_latest_summary_title()?;
        }
        Ok(())
    }

    pub(super) fn apply_replaced_message(
        &mut self,
        previous_message: &ChatMessage,
        updated_message: &ChatMessage,
    ) {
        let previous_body_text_length = Self::body_text_length_for_message(previous_message);
        let updated_body_text_length = Self::body_text_length_for_message(updated_message);
        self.body_text_length = self
            .body_text_length
            .saturating_sub(previous_body_text_length)
            .saturating_add(updated_body_text_length);
        if updated_message
            .role
            .trim()
            .eq_ignore_ascii_case("assistant")
        {
            self.has_assistant_reply = true;
        }

        let next_preview = Self::preview_message_from_chat_message(updated_message);
        if let Some(position) = self
            .preview_messages
            .iter()
            .position(|item| item.message_id.trim() == updated_message.id.trim())
        {
            if let Some(preview) = next_preview {
                self.preview_messages[position] = preview;
            } else {
                self.preview_messages.remove(position);
            }
        } else if self.last_message_id.as_deref().map(str::trim)
            == Some(updated_message.id.trim())
        {
            if let Some(preview) = next_preview {
                self.preview_messages.push(preview);
            }
        }
        if self.preview_messages.len() > 2 {
            let keep_from = self.preview_messages.len().saturating_sub(2);
            self.preview_messages = self.preview_messages[keep_from..].to_vec();
        }
    }

    fn apply_spliced_messages(
        &mut self,
        removed_messages: &[ChatMessage],
        inserted_messages: &[ChatMessage],
    ) {
        self.message_count = self.message_count.saturating_sub(removed_messages.len());
        self.body_message_count = self.body_message_count.saturating_sub(
            removed_messages
                .iter()
                .filter(|message| {
                    matches!(
                        message.role.trim().to_ascii_lowercase().as_str(),
                        "user" | "assistant"
                    )
                })
                .count(),
        );
        self.body_text_length = self.body_text_length.saturating_sub(
            removed_messages
                .iter()
                .flat_map(|message| message.parts.iter())
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim().chars().count()),
                    _ => None,
                })
                .sum::<usize>(),
        );
        self.message_count = self.message_count.saturating_add(inserted_messages.len());
        self.body_message_count = self.body_message_count.saturating_add(
            inserted_messages
                .iter()
                .filter(|message| {
                    matches!(
                        message.role.trim().to_ascii_lowercase().as_str(),
                        "user" | "assistant"
                    )
                })
                .count(),
        );
        self.body_text_length = self.body_text_length.saturating_add(
            inserted_messages
                .iter()
                .flat_map(|message| message.parts.iter())
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim().chars().count()),
                    _ => None,
                })
                .sum::<usize>(),
        );
        if inserted_messages.iter().any(|message| {
            super::is_context_compaction_message(message, message.role.trim())
        }) {
            self.has_context_compaction_message = true;
        }
        if let Some(last_title) = inserted_messages
            .iter()
            .rev()
            .find_map(super::summary_context_message_title)
        {
            self.latest_summary_title = Some(last_title);
        }
    }

    pub(super) fn apply_truncated_rewind_state(
        &mut self,
        keep_count: usize,
        current_todos: Vec<ConversationTodoItem>,
        updated_at: String,
        last_user_at: Option<String>,
        last_assistant_at: Option<String>,
        last_message_id: Option<String>,
        last_message_at: Option<String>,
        body_message_count: usize,
        body_text_length: usize,
        has_assistant_reply: bool,
        has_context_compaction_message: bool,
        latest_summary_title: Option<String>,
        preview_messages: Vec<ConversationShardPreviewMessage>,
    ) {
        self.current_todos = current_todos;
        self.updated_at = updated_at;
        self.last_user_at = last_user_at;
        self.last_assistant_at = last_assistant_at;
        self.last_message_id = last_message_id;
        self.last_message_at = last_message_at;
        self.message_count = keep_count;
        self.body_message_count = body_message_count;
        self.body_text_length = body_text_length;
        self.has_assistant_reply = has_assistant_reply;
        self.has_context_compaction_message = has_context_compaction_message;
        self.latest_summary_title = latest_summary_title;
        self.preview_messages = preview_messages;
    }

    pub(super) fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            meta_schema_version: CONVERSATION_META_SCHEMA_VERSION,
            id: conversation.id.clone(),
            title: conversation.title.clone(),
            agent_id: conversation.agent_id.clone(),
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
            user_profile_snapshot: conversation.user_profile_snapshot.clone(),
            shell_workspace_path: conversation.shell_workspace_path.clone(),
            shell_workspaces: conversation.shell_workspaces.clone(),
            shell_autonomous_mode: conversation.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&conversation.shell_work_mode),
            shell_work_branch: conversation.shell_work_branch.clone(),
            archived_at: conversation.archived_at.clone(),
            current_todos: conversation.current_todos.clone(),
            memory_recall_table: conversation.memory_recall_table.clone(),
            plan_mode_enabled: conversation.plan_mode_enabled,
            preferred_api_config_id: conversation.preferred_api_config_id.clone(),
            is_draft: conversation.is_draft,
            auto_push_remote_contact_id: conversation.auto_push_remote_contact_id.clone(),
            cumulative_usage: conversation.cumulative_usage.clone(),
            active_goal: conversation.active_goal.clone(),
            fast_request_turns: conversation.fast_request_turns.clone(),
            last_error: conversation.last_error.clone(),
            last_message_id: conversation.messages.last().map(|message| message.id.clone()),
            last_message_at: conversation.messages.last().map(|message| message.created_at.clone()),
            message_count: conversation.messages.len(),
            body_message_count: super::conversation_body_message_count(conversation),
            body_text_length: super::conversation_body_text_length(conversation),
            has_assistant_reply: conversation
                .messages
                .iter()
                .any(|message| message.role.trim().eq_ignore_ascii_case("assistant")),
            has_context_compaction_message: conversation
                .messages
                .iter()
                .any(|message| super::is_context_compaction_message(message, message.role.trim())),
            latest_summary_title: super::conversation_latest_summary_title(conversation),
            preview_messages: conversation
                .messages
                .iter()
                .filter(|message| {
                    matches!(
                        message.role.trim().to_ascii_lowercase().as_str(),
                        "user" | "assistant" | "tool"
                    )
                })
                .rev()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|message| ConversationShardPreviewMessage {
                    message_id: message.id.clone(),
                    role: message.role.clone(),
                    speaker_agent_id: message.speaker_agent_id.clone(),
                    created_at: Some(message.created_at.clone())
                        .filter(|value| !value.trim().is_empty()),
                    text_preview: super::build_conversation_preview_text(&message),
                    has_image: message.parts.iter().any(|part| {
                        matches!(part, MessagePart::Image { mime, .. } if !mime.trim().eq_ignore_ascii_case("application/pdf"))
                            || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "image")
                    }),
                    has_pdf: message.parts.iter().any(|part| {
                        matches!(part, MessagePart::Image { mime, .. } if mime.trim().eq_ignore_ascii_case("application/pdf"))
                            || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "pdf")
                    }),
                    has_audio: message
                        .parts
                        .iter()
                        .any(|part| {
                            matches!(part, MessagePart::Audio { .. })
                                || matches!(part, MessagePart::Attachment { mime, .. } if message_attachment_kind(mime) == "audio")
                        }),
                    has_attachment: super::conversation_message_has_attachment(&message),
                })
                .collect(),
        }
    }

    fn from_persist_meta(meta: &ConversationPersistMeta) -> Self {
        Self {
            meta_schema_version: if meta.meta_schema_version == 0 {
                CONVERSATION_META_SCHEMA_VERSION
            } else {
                meta.meta_schema_version
            },
            id: meta.id.clone(),
            title: meta.title.clone(),
            agent_id: meta.agent_id.clone(),
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
            user_profile_snapshot: meta.user_profile_snapshot.clone(),
            shell_workspace_path: meta.shell_workspace_path.clone(),
            shell_workspaces: meta.shell_workspaces.clone(),
            shell_autonomous_mode: meta.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&meta.shell_work_mode),
            shell_work_branch: meta.shell_work_branch.clone(),
            archived_at: meta.archived_at.clone(),
            current_todos: meta.current_todos.clone(),
            memory_recall_table: meta.memory_recall_table.clone(),
            plan_mode_enabled: meta.plan_mode_enabled,
            preferred_api_config_id: meta.preferred_api_config_id.clone(),
            is_draft: meta.is_draft,
            auto_push_remote_contact_id: meta.auto_push_remote_contact_id.clone(),
            cumulative_usage: meta.cumulative_usage.clone(),
            active_goal: meta.active_goal.clone(),
            fast_request_turns: meta.fast_request_turns.clone(),
            last_message_id: meta.last_message_id.clone(),
            last_message_at: meta.last_message_at.clone(),
            last_error: meta.last_error.clone(),
            message_count: meta.message_count,
            body_message_count: meta.body_message_count,
            body_text_length: meta.body_text_length,
            has_assistant_reply: meta.has_assistant_reply,
            has_context_compaction_message: meta.has_context_compaction_message,
            latest_summary_title: meta.latest_summary_title.clone(),
            preview_messages: meta.preview_messages.clone(),
        }
    }

    pub(super) fn to_persist_meta(&self) -> ConversationPersistMeta {
        ConversationPersistMeta {
            meta_schema_version: self.meta_schema_version,
            id: self.id.clone(),
            title: self.title.clone(),
            agent_id: self.agent_id.clone(),
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
            user_profile_snapshot: self.user_profile_snapshot.clone(),
            shell_workspace_path: self.shell_workspace_path.clone(),
            shell_workspaces: self.shell_workspaces.clone(),
            shell_autonomous_mode: self.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&self.shell_work_mode),
            shell_work_branch: self.shell_work_branch.clone(),
            archived_at: self.archived_at.clone(),
            current_todos: self.current_todos.clone(),
            memory_recall_table: self.memory_recall_table.clone(),
            plan_mode_enabled: self.plan_mode_enabled,
            preferred_api_config_id: self.preferred_api_config_id.clone(),
            is_draft: self.is_draft,
            auto_push_remote_contact_id: self.auto_push_remote_contact_id.clone(),
            cumulative_usage: self.cumulative_usage.clone(),
            active_goal: self.active_goal.clone(),
            fast_request_turns: self.fast_request_turns.clone(),
            last_message_id: self.last_message_id.clone(),
            last_message_at: self.last_message_at.clone(),
            last_error: self.last_error.clone(),
            message_count: self.message_count,
            body_message_count: self.body_message_count,
            body_text_length: self.body_text_length,
            has_assistant_reply: self.has_assistant_reply,
            has_context_compaction_message: self.has_context_compaction_message,
            latest_summary_title: self.latest_summary_title.clone(),
            preview_messages: self.preview_messages.clone(),
        }
    }

    fn into_conversation(self, messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: self.id,
            title: self.title,
            agent_id: self.agent_id,
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
            user_profile_snapshot: self.user_profile_snapshot,
            shell_workspace_path: self.shell_workspace_path,
            shell_workspaces: self.shell_workspaces,
            shell_autonomous_mode: self.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&self.shell_work_mode),
            shell_work_branch: self.shell_work_branch,
            archived_at: self.archived_at,
            messages,
            fast_request_turns: self.fast_request_turns,
            current_todos: self.current_todos,
            memory_recall_table: self.memory_recall_table,
            plan_mode_enabled: self.plan_mode_enabled,
            preferred_api_config_id: self.preferred_api_config_id,
            is_draft: self.is_draft,
            auto_push_remote_contact_id: self.auto_push_remote_contact_id,
            cumulative_usage: self.cumulative_usage,
            active_goal: self.active_goal,
            last_error: self.last_error,
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
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }
    }

    fn test_conversation() -> Conversation {
        Conversation {
            id: "conversation-meta".to_string(),
            title: "元数据会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
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
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![test_message("m1"), test_message("m2")],
            fast_request_turns: vec![FastRequestTurn {
                id: "fast-request-a".to_string(),
                kind: "title_generation".to_string(),
                request_text: "request".to_string(),
                response_text: "response".to_string(),
                success: true,
                error: None,
                model_name: Some("quick-model".to_string()),
                duration_ms: Some(12),
                created_at: "2026-04-24T00:00:50Z".to_string(),
            }],
            current_todos: vec![ConversationTodoItem {
                content: "todo".to_string(),
                status: "pending".to_string(),
            }],
            memory_recall_table: vec!["memory-a".to_string()],
            plan_mode_enabled: true,
            preferred_api_config_id: Some("api-c".to_string()),
            auto_push_remote_contact_id: Some("contact-a".to_string()),
            cumulative_usage: ConversationCumulativeUsage {
                input_tokens: 10,
                output_tokens: 20,
                cache_read_tokens: 30,
                cache_write_tokens: 40,
                ..ConversationCumulativeUsage::default()
            },
            active_goal: Some(ConversationGoalState {
                goal_id: "goal-a".to_string(),
                status: "active".to_string(),
                objective: "完成元数据测试".to_string(),
                started_at: "2026-04-24T00:00:10Z".to_string(),
                ended_at: None,
                usage_start: ConversationCumulativeUsage::default(),
                usage_end: None,
            }),
            last_error: Some("测试轮次失败".to_string()),
            is_draft: false,
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
        assert_eq!(restored.child_conversation_ids, conversation.child_conversation_ids);
        assert_eq!(restored.preferred_api_config_id, conversation.preferred_api_config_id);
        assert_eq!(
            restored.auto_push_remote_contact_id,
            conversation.auto_push_remote_contact_id
        );
        assert_eq!(restored.cumulative_usage, conversation.cumulative_usage);
        assert_eq!(restored.active_goal, conversation.active_goal);
        assert_eq!(restored.fast_request_turns, conversation.fast_request_turns);
        assert_eq!(persist_meta.fast_request_turns, conversation.fast_request_turns);
        assert_eq!(restored.last_error, conversation.last_error);
        assert_eq!(persist_meta.last_error, conversation.last_error);
        assert_eq!(meta.last_error(), Some("测试轮次失败"));
    }

    #[test]
    fn legacy_conversation_without_shell_work_mode_should_default_to_directory() {
        let conversation = test_conversation();
        let mut value = serde_json::to_value(&conversation).expect("serialize conversation");
        value
            .as_object_mut()
            .expect("conversation object")
            .remove("shellWorkMode");

        let restored: Conversation = serde_json::from_value(value).expect("deserialize legacy conversation");

        assert_eq!(restored.shell_work_mode, SHELL_WORK_MODE_DIRECTORY);
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