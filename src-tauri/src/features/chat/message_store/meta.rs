pub(super) const CONVERSATION_META_SCHEMA_VERSION: u32 = 1;

/// 快速请求记录在内存里保留的条数上限。它只在内存中存活，落盘前会被清空。
const FAST_REQUEST_TURNS_MEMORY_LIMIT: usize = 10;

// ==================== 骨：只住在 SQL 列里的基石字段 ====================

/// 会话元数据的「骨」：这些字段在 `conversation_metadata` 里有各自的列，
/// `metadata_json` 里不存它们。
///
/// `short_id` 与 `created_at` 只在「新建落库」与「V5 升级迁移」时写入一次；
/// 日常业务的 UPDATE 语句根本不写这两列，从物理层杜绝被涂抹。
/// 标题不在这里：它是高频变动、多种形态的业务载荷，留在 `metadata_json` 里由 serde 兼容。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ConversationBackbone {
    conversation_id: String,
    short_id: Option<u64>,
    created_at: String,
    updated_at: String,
    status: String,
    conversation_kind: String,
    agent_id: String,
    parent_conversation_id: Option<String>,
    root_conversation_id: Option<String>,
    delegate_id: Option<String>,
}

impl ConversationBackbone {
    pub(super) fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            conversation_id: conversation.id.clone(),
            short_id: conversation.short_id,
            created_at: conversation.created_at.clone(),
            updated_at: conversation.updated_at.clone(),
            status: conversation.status.clone(),
            conversation_kind: conversation.conversation_kind.clone(),
            agent_id: conversation.agent_id.clone(),
            parent_conversation_id: conversation.parent_conversation_id.clone(),
            root_conversation_id: conversation.root_conversation_id.clone(),
            delegate_id: conversation.delegate_id.clone(),
        }
    }

    pub(super) fn id(&self) -> &str {
        &self.conversation_id
    }

    pub(super) fn short_id(&self) -> Option<u64> {
        self.short_id
    }

    pub(super) fn created_at(&self) -> &str {
        &self.created_at
    }

    pub(super) fn updated_at(&self) -> &str {
        &self.updated_at
    }

    pub(super) fn status(&self) -> &str {
        &self.status
    }

    pub(super) fn conversation_kind(&self) -> &str {
        &self.conversation_kind
    }

    pub(super) fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub(super) fn parent_conversation_id(&self) -> Option<&str> {
        self.parent_conversation_id.as_deref()
    }

    pub(super) fn root_conversation_id(&self) -> Option<&str> {
        self.root_conversation_id.as_deref()
    }

    pub(super) fn delegate_id(&self) -> Option<&str> {
        self.delegate_id.as_deref()
    }

    pub(super) fn apply_to_conversation(&self, target: &mut Conversation) {
        target.short_id = self.short_id;
        target.created_at = self.created_at.clone();
        target.updated_at = self.updated_at.clone();
        target.status = self.status.clone();
        target.conversation_kind = self.conversation_kind.clone();
        target.agent_id = self.agent_id.clone();
        target.parent_conversation_id = self.parent_conversation_id.clone();
        target.root_conversation_id = self.root_conversation_id.clone();
        target.delegate_id = self.delegate_id.clone();
    }
}

// ==================== 骨肉合体：读取/写入时的一次性视图 ====================

/// 骨（SQL 列）+ 肉（metadata_json 载荷）。
///
/// 读库时先分别拿出两半再合体，写库时再拆开：列走列，JSON 走 JSON。
/// 通过 `Deref` 到载荷，载荷上的全部 getter 在合体视图上照样能用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversationStoredMeta {
    backbone: ConversationBackbone,
    payload: ConversationShardMeta,
    /// 问题清单 + 编号游标：`conversation_metadata.questions_json` 列在内存里的那一份。
    ///
    /// 它不参与 `metadata_json` 的序列化，也不进通用 meta 写入的 UPDATE。读写都走提问
    /// 链路：读会话元数据时连这一列一起取出，写入时整列覆盖回库。
    questions: ConversationQuestionsColumn,
}

impl std::ops::Deref for ConversationStoredMeta {
    type Target = ConversationShardMeta;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

/// 合体视图的序列化只输出肉：骨只住 SQL 列，任何 JSON 都不该带它。
impl serde::Serialize for ConversationStoredMeta {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.payload.serialize(serializer)
    }
}

impl std::ops::DerefMut for ConversationStoredMeta {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.payload
    }
}

impl ConversationStoredMeta {
    pub(super) fn new(
        backbone: ConversationBackbone,
        payload: ConversationShardMeta,
    ) -> Self {
        Self {
            backbone,
            payload,
            questions: ConversationQuestionsColumn::default(),
        }
    }

    /// 从库里读出的那份：问题列与骨肉同一条 SELECT 拿出，原样带进来。
    pub(super) fn new_with_questions(
        backbone: ConversationBackbone,
        payload: ConversationShardMeta,
        questions: ConversationQuestionsColumn,
    ) -> Self {
        Self {
            backbone,
            payload,
            questions,
        }
    }

    pub(super) fn questions(&self) -> &ConversationQuestionsColumn {
        &self.questions
    }

    pub(super) fn set_questions(&mut self, questions: ConversationQuestionsColumn) {
        self.questions = questions;
    }

    pub(super) fn questions_mut(&mut self) -> &mut ConversationQuestionsColumn {
        &mut self.questions
    }

    pub(super) fn from_conversation(conversation: &Conversation) -> Self {
        Self {
            backbone: ConversationBackbone::from_conversation(conversation),
            payload: ConversationShardMeta::from_conversation(conversation),
            questions: ConversationQuestionsColumn::default(),
        }
    }

    /// 局部 splice：骨取自会话本体，肉沿用基于快照的派生字段规则。
    pub(super) fn from_conversation_with_spliced_messages(
        conversation: &Conversation,
        source: &ConversationShardMeta,
        removed_messages: &[ChatMessage],
        inserted_messages: &[ChatMessage],
    ) -> Self {
        Self {
            backbone: ConversationBackbone::from_conversation(conversation),
            payload: ConversationShardMeta::from_conversation_with_spliced_messages(
                conversation,
                source,
                removed_messages,
                inserted_messages,
            ),
            questions: ConversationQuestionsColumn::default(),
        }
    }

    pub(super) fn into_parts(self) -> (ConversationBackbone, ConversationShardMeta) {
        (self.backbone, self.payload)
    }

    /// 归一化历史用量总量：只动肉，骨原样保留。
    pub(super) fn normalized_legacy_usage_totals(mut self) -> Self {
        self.payload = self.payload.normalized_legacy_usage_totals();
        self
    }

    pub(super) fn backbone(&self) -> &ConversationBackbone {
        &self.backbone
    }

    pub(super) fn payload(&self) -> &ConversationShardMeta {
        &self.payload
    }

    pub(super) fn updated_at(&self) -> &str {
        self.backbone.updated_at()
    }

    pub(super) fn into_payload(self) -> ConversationShardMeta {
        self.payload
    }

    pub(super) fn id(&self) -> &str {
        self.backbone.id()
    }

    /// 与老的持久化结构同名：日志与错误信息里用得一贯。
    pub(super) fn conversation_id(&self) -> &str {
        self.backbone.id()
    }

    pub(super) fn short_id(&self) -> Option<u64> {
        self.backbone.short_id()
    }

    pub(super) fn agent_id(&self) -> &str {
        self.backbone.agent_id()
    }

    pub(super) fn status(&self) -> &str {
        self.backbone.status()
    }

    pub(super) fn conversation_kind(&self) -> &str {
        self.backbone.conversation_kind()
    }

    pub(super) fn created_at(&self) -> &str {
        self.backbone.created_at()
    }

    pub(super) fn parent_conversation_id(&self) -> Option<&str> {
        self.backbone.parent_conversation_id()
    }

    pub(super) fn root_conversation_id(&self) -> Option<&str> {
        self.backbone.root_conversation_id()
    }

    pub(super) fn root_conversation_id_text(&self) -> Option<&str> {
        self.backbone
            .root_conversation_id()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(super) fn delegate_id(&self) -> Option<&str> {
        self.backbone.delegate_id()
    }

    /// 先铺肉、再铺骨：骨是列权威，任何残留的肉字段都不许盖掉它。
    /// 问题来自独立列，放在最后铺，谁都不会把它盖掉。
    pub(super) fn apply_to_conversation(&self, target: &mut Conversation) {
        self.payload.apply_to_conversation(target);
        self.backbone.apply_to_conversation(target);
        target.questions = self.questions.items.clone();
        target.question_next_number = self.questions.next_number;
    }

    pub(super) fn into_conversation(self, messages: Vec<ChatMessage>) -> Conversation {
        let questions = self.questions.clone();
        let (backbone, payload) = self.into_parts();
        let mut conversation = payload.into_conversation(messages);
        backbone.apply_to_conversation(&mut conversation);
        conversation.questions = questions.items;
        conversation.question_next_number = questions.next_number;
        conversation
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
    /// 标题留在 JSON 里：高频变动、多种形态（重命名 / 摘要标题 / 草稿），不进 SQL 列。
    title: String,
    #[serde(default)]
    bound_conversation_id: Option<String>,
    #[serde(default)]
    child_conversation_ids: Vec<String>,
    #[serde(default)]
    fork_message_cursor: Option<String>,
    #[serde(default)]
    unread_count: usize,
    updated_at: String,
    #[serde(default)]
    last_user_at: Option<String>,
    #[serde(default)]
    last_assistant_at: Option<String>,
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
    shell_worktree_path: String,
    #[serde(default)]
    shell_recorded_branch: String,
    #[serde(default)]
    archived_at: Option<String>,
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
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
    /// 会话级队伍状态；联系人会话摘要需要读取名册，故对本 crate 开放。
    #[serde(default)]
    pub(crate) team: TeamState,
    /// 本会话自己的待答问题与编号游标**不在这里**：它们住在
    /// `conversation_metadata.questions_json` 独立列，只由提问写入路径维护，
    /// 因此 `metadata_json` 里永远不会出现这两个键。
    /// 队长会话的议题看板与编号游标；队员会话恒为空。
    issues: Vec<KanbanIssueEntry>,
    issue_next_number: u32,
    #[serde(default)]
    loaded_mcp_tools: Vec<String>,
    /// 快速请求记录只在内存里存活：落盘不序列化，读回恒为空。
    #[serde(skip)]
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

// ==================== 落盘文本读取 ====================

/// 读文件那一层做的归一化：缺附属记录的四个键时，补成「没有记录」再解析。
///
/// 结构体本身要求这四个键必须存在，缺了算数据错误，不靠字段属性兜底；
/// 但读的时候不能因为缺键就丢掉整条会话，所以补键只发生在这一层。
pub(super) fn conversation_json_fill_missing_attachments(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    for (key, empty) in [
        ("issues", serde_json::Value::Array(Vec::new())),
        ("issueNextNumber", serde_json::Value::from(0)),
    ] {
        if !object.contains_key(key) {
            object.insert(key.to_string(), empty);
        }
    }
}

/// 从落盘文本读出会话元数据：先把缺席的附属记录键补上，再转成结构体。
pub(super) fn conversation_shard_meta_from_stored_json(
    raw: &str,
) -> Result<ConversationShardMeta, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(raw)?;
    conversation_json_fill_missing_attachments(&mut value);
    serde_json::from_value(value)
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

    pub(super) fn unread_count(&self) -> usize {
        self.unread_count
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

    pub(super) fn current_todos(&self) -> &[ConversationTodoItem] {
        self.current_todos.as_slice()
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

    pub(super) fn shell_worktree_path(&self) -> &str {
        self.shell_worktree_path.as_str()
    }

    pub(super) fn shell_recorded_branch(&self) -> &str {
        self.shell_recorded_branch.as_str()
    }

    pub(super) fn is_draft(&self) -> bool {
        self.is_draft
    }

    pub(super) fn active_goal(&self) -> Option<&ConversationGoalState> {
        self.active_goal.as_ref()
    }

    pub(super) fn team(&self) -> &TeamState {
        &self.team
    }

    pub(super) fn issues(&self) -> &[KanbanIssueEntry] {
        self.issues.as_slice()
    }

    pub(super) fn issue_next_number(&self) -> u32 {
        self.issue_next_number
    }

    pub(super) fn fast_request_turns(&self) -> &[FastRequestTurn] {
        self.fast_request_turns.as_slice()
    }

    pub(super) fn push_fast_request_turn(&mut self, turn: FastRequestTurn) {
        self.fast_request_turns.push(turn);
        // 快速请求只在内存里留最近若干条：落盘时该字段恒为空（见 `ConversationShardMeta::from_conversation`），
        // 这里顺手裁掉更早的，避免活着的会话在内存里无限增长。
        let overflow = self
            .fast_request_turns
            .len()
            .saturating_sub(FAST_REQUEST_TURNS_MEMORY_LIMIT);
        if overflow > 0 {
            self.fast_request_turns.drain(..overflow);
        }
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

    pub(super) fn loaded_mcp_tools(&self) -> &[String] {
        self.loaded_mcp_tools.as_slice()
    }

    pub(super) fn normalized_legacy_usage_totals(mut self) -> Self {
        self.cumulative_usage = self.cumulative_usage.clone().normalized_legacy_totals();
        self
    }

    pub(super) fn preview_messages(&self) -> &[ConversationShardPreviewMessage] {
        self.preview_messages.as_slice()
    }

    pub(super) fn apply_to_conversation(&self, target: &mut Conversation) {
        // 骨字段（status / created_at / agent_id / kind / 拓扑 / short_id）不在这里，
        // 由 ConversationBackbone 铺；这里只铺 metadata_json 里的肉（含标题）。
        target.title = self.title.clone();
        target.bound_conversation_id = self.bound_conversation_id.clone();
        target.child_conversation_ids = self.child_conversation_ids.clone();
        target.fork_message_cursor = self.fork_message_cursor.clone();
        target.unread_count = self.unread_count;
        target.updated_at = self.updated_at.clone();
        target.last_user_at = self.last_user_at.clone();
        target.last_assistant_at = self.last_assistant_at.clone();
        target.user_profile_snapshot = self.user_profile_snapshot.clone();
        target.shell_workspace_path = self.shell_workspace_path.clone();
        target.shell_workspaces = self.shell_workspaces.clone();
        target.shell_autonomous_mode = self.shell_autonomous_mode;
        target.shell_work_mode = self.shell_work_mode.clone();
        target.shell_work_branch = self.shell_work_branch.clone();
        target.shell_worktree_path = self.shell_worktree_path.clone();
        target.shell_recorded_branch = self.shell_recorded_branch.clone();
        target.archived_at = self.archived_at.clone();
        target.current_todos = self.current_todos.clone();
        target.memory_recall_table = self.memory_recall_table.clone();
        target.preferred_api_config_id = self.preferred_api_config_id.clone();
        target.is_draft = self.is_draft;
        target.auto_push_remote_contact_id = self.auto_push_remote_contact_id.clone();
        target.cumulative_usage = self.cumulative_usage.clone();
        target.active_goal = self.active_goal.clone();
        target.team = self.team.clone();
        target.issues = self.issues.clone();
        target.issue_next_number = self.issue_next_number;
        target.loaded_mcp_tools = self.loaded_mcp_tools.clone();
        target.fast_request_turns = self.fast_request_turns.clone();
        target.last_error = self.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_conversation(&mut self, source: &Conversation) {
        self.title = source.title.clone();
        self.bound_conversation_id = source.bound_conversation_id.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.shell_worktree_path = source.shell_worktree_path.clone();
        self.shell_recorded_branch = source.shell_recorded_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.memory_recall_table = source.memory_recall_table.clone();
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.team = source.team.clone();
        self.issues = source.issues.clone();
        self.issue_next_number = source.issue_next_number;
        self.loaded_mcp_tools = source.loaded_mcp_tools.clone();
        self.fast_request_turns = source.fast_request_turns.clone();
        self.last_error = source.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_meta(&mut self, source: &ConversationShardMeta) {
        self.title = source.title.clone();
        self.bound_conversation_id = source.bound_conversation_id.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.shell_worktree_path = source.shell_worktree_path.clone();
        self.shell_recorded_branch = source.shell_recorded_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.memory_recall_table = source.memory_recall_table.clone();
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.team = source.team.clone();
        self.issues = source.issues.clone();
        self.issue_next_number = source.issue_next_number;
        self.loaded_mcp_tools = source.loaded_mcp_tools.clone();
        self.last_error = source.last_error.clone();
    }

    pub(super) fn apply_metadata_fields_from_meta_view(&mut self, source: &ConversationMetaView) {
        self.title = source.title.clone();
        self.child_conversation_ids = source.child_conversation_ids.clone();
        self.fork_message_cursor = source.fork_message_cursor.clone();
        self.unread_count = source.unread_count;
        self.updated_at = source.updated_at.clone();
        self.last_user_at = source.last_user_at.clone();
        self.last_assistant_at = source.last_assistant_at.clone();
        self.user_profile_snapshot = source.user_profile_snapshot.clone();
        self.shell_workspace_path = source.shell_workspace_path.clone();
        self.shell_workspaces = source.shell_workspaces.clone();
        self.shell_autonomous_mode = source.shell_autonomous_mode;
        self.shell_work_mode = source.shell_work_mode.clone();
        self.shell_work_branch = source.shell_work_branch.clone();
        self.shell_worktree_path = source.shell_worktree_path.clone();
        self.shell_recorded_branch = source.shell_recorded_branch.clone();
        self.archived_at = source.archived_at.clone();
        self.current_todos = source.current_todos.clone();
        self.preferred_api_config_id = source.preferred_api_config_id.clone();
        self.is_draft = source.is_draft;
        self.auto_push_remote_contact_id = source.auto_push_remote_contact_id.clone();
        self.cumulative_usage = source.cumulative_usage.clone();
        self.active_goal = source.active_goal.clone();
        self.team = source.team.clone();
        self.issues = source.issues.clone();
        self.issue_next_number = source.issue_next_number;
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
            bound_conversation_id: conversation.bound_conversation_id.clone(),
            child_conversation_ids: conversation.child_conversation_ids.clone(),
            fork_message_cursor: conversation.fork_message_cursor.clone(),
            unread_count: conversation.unread_count,
            updated_at: conversation.updated_at.clone(),
            last_user_at: conversation.last_user_at.clone(),
            last_assistant_at: conversation.last_assistant_at.clone(),
            user_profile_snapshot: conversation.user_profile_snapshot.clone(),
            shell_workspace_path: conversation.shell_workspace_path.clone(),
            shell_workspaces: conversation.shell_workspaces.clone(),
            shell_autonomous_mode: conversation.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&conversation.shell_work_mode),
            shell_work_branch: conversation.shell_work_branch.clone(),
            shell_worktree_path: conversation.shell_worktree_path.clone(),
            shell_recorded_branch: conversation.shell_recorded_branch.clone(),
            archived_at: conversation.archived_at.clone(),
            current_todos: conversation.current_todos.clone(),
            memory_recall_table: conversation.memory_recall_table.clone(),
            preferred_api_config_id: conversation.preferred_api_config_id.clone(),
            is_draft: conversation.is_draft,
            auto_push_remote_contact_id: conversation.auto_push_remote_contact_id.clone(),
            cumulative_usage: conversation.cumulative_usage.clone(),
            active_goal: conversation.active_goal.clone(),
            team: conversation.team.clone(),
            issues: conversation.issues.clone(),
            issue_next_number: conversation.issue_next_number,
            loaded_mcp_tools: conversation.loaded_mcp_tools.clone(),
            // 快速请求只在内存里存活，落盘结构里恒为空。
            fast_request_turns: Vec::new(),
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

    /// 基于 metadata 快照写局部 splice：消息派生字段不从空消息列表重算，只按实际替换的消息更新。
    pub(super) fn from_conversation_with_spliced_messages(
        conversation: &Conversation,
        source: &ConversationShardMeta,
        removed_messages: &[ChatMessage],
        inserted_messages: &[ChatMessage],
    ) -> Self {
        let mut meta = Self::from_conversation(conversation);
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
        meta
    }

    fn into_conversation(self, messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            // 骨字段这里只给占位：真正的值由 ConversationStoredMeta 用 SQL 列覆盖。
            short_id: None,
            id: self.id,
            title: self.title,
            agent_id: String::new(),
            bound_conversation_id: self.bound_conversation_id,
            parent_conversation_id: None,
            child_conversation_ids: self.child_conversation_ids,
            fork_message_cursor: self.fork_message_cursor,
            unread_count: self.unread_count,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: String::new(),
            updated_at: self.updated_at,
            last_user_at: self.last_user_at,
            last_assistant_at: self.last_assistant_at,
            status: String::new(),
            user_profile_snapshot: self.user_profile_snapshot,
            shell_workspace_path: self.shell_workspace_path,
            shell_workspaces: self.shell_workspaces,
            shell_autonomous_mode: self.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&self.shell_work_mode),
            shell_work_branch: self.shell_work_branch,
            shell_worktree_path: self.shell_worktree_path,
            shell_recorded_branch: self.shell_recorded_branch,
            archived_at: self.archived_at,
            messages,
            // 快速请求不落盘：盘上的旧记录一律不认，只保留本进程内存里新产生的那些。
            fast_request_turns: Vec::new(),
            current_todos: self.current_todos,
            memory_recall_table: self.memory_recall_table,
            preferred_api_config_id: self.preferred_api_config_id,
            is_draft: self.is_draft,
            auto_push_remote_contact_id: self.auto_push_remote_contact_id,
            cumulative_usage: self.cumulative_usage,
            active_goal: self.active_goal,
            team: self.team,
            // 问题清单住在独立列：这里只填占位，由
            // `ConversationStoredMeta::into_conversation` 用列值覆盖。
            questions: Vec::new(),
            question_next_number: 0,
            issues: self.issues,
            issue_next_number: self.issue_next_number,
            loaded_mcp_tools: self.loaded_mcp_tools,
            last_error: self.last_error,
        }
    }
}

/// 旧 V2 明文格式：骨与肉合排在同一份 JSON 里。
///
/// 只有 V1→V2 播种与 V2→V3 导入这两条历史链路用它。SQLite 生产路径是
/// 「骨进列、肉进 `metadata_json`」，与这里无关；这里合排只是为了不丢老数据。
fn conversation_stored_meta_to_legacy_json(
    meta: &ConversationStoredMeta,
) -> Result<String, String> {
    let mut value = serde_json::to_value(meta.payload()).map_err(|err| {
        format!(
            "序列化会话元数据失败，conversation_id={}，error={err}",
            meta.id()
        )
    })?;
    let backbone = meta.backbone();
    if let Some(object) = value.as_object_mut() {
        if let Some(short_id) = backbone.short_id() {
            object.insert("shortId".to_string(), serde_json::json!(short_id));
        }
        object.insert(
            "createdAt".to_string(),
            serde_json::json!(backbone.created_at()),
        );
        object.insert("status".to_string(), serde_json::json!(backbone.status()));
        object.insert(
            "conversationKind".to_string(),
            serde_json::json!(backbone.conversation_kind()),
        );
        object.insert("agentId".to_string(), serde_json::json!(backbone.agent_id()));
        object.insert(
            "parentConversationId".to_string(),
            serde_json::json!(backbone.parent_conversation_id()),
        );
        object.insert(
            "rootConversationId".to_string(),
            serde_json::json!(backbone.root_conversation_id()),
        );
        object.insert(
            "delegateId".to_string(),
            serde_json::json!(backbone.delegate_id()),
        );
    }
    serde_json::to_string_pretty(&value).map_err(|err| {
        format!(
            "序列化会话元数据失败，conversation_id={}，error={err}",
            meta.id()
        )
    })
}

/// 从旧 V2 明文里读回骨与肉。老版本写出的 JSON 带骨键，取不到就给安全默认值。
pub(super) fn conversation_stored_meta_from_legacy_json(
    raw: &str,
) -> Result<ConversationStoredMeta, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|err| format!("解析会话元数据失败: {err}"))?;
    let payload = conversation_shard_meta_from_stored_json(raw)
        .map_err(|err| format!("解析会话元数据失败: {err}"))?;
    let text = |key: &str| -> Option<String> {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .filter(|item| !item.trim().is_empty())
    };
    let backbone = ConversationBackbone {
        conversation_id: payload.id.clone(),
        short_id: value.get("shortId").and_then(serde_json::Value::as_u64),
        created_at: text("createdAt").unwrap_or_default(),
        updated_at: text("updatedAt").unwrap_or_else(|| payload.updated_at.clone()),
        status: text("status").unwrap_or_else(|| "active".to_string()),
        conversation_kind: text("conversationKind")
            .unwrap_or_else(|| CONVERSATION_KIND_CHAT.to_string()),
        agent_id: text("agentId").unwrap_or_default(),
        parent_conversation_id: text("parentConversationId"),
        root_conversation_id: text("rootConversationId"),
        delegate_id: text("delegateId"),
    };
    Ok(ConversationStoredMeta::new_with_questions(
        backbone,
        payload,
        // 老明文里若还留着 questions 键，反序列化时已被忽略：不迁移，按空开始。
        ConversationQuestionsColumn::default(),
    ))
}

fn write_conversation_shard_meta_atomic(
    path: &PathBuf,
    meta: &ConversationStoredMeta,
) -> Result<(), String> {
    let raw = conversation_stored_meta_to_legacy_json(meta)?;
    write_message_store_text_atomic(path, "json.tmp", &raw, "会话元数据")
}

fn read_conversation_shard_meta(path: &PathBuf) -> Result<ConversationShardMeta, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取会话元数据失败，path={}，error={err}", path.display()))?;
    conversation_shard_meta_from_stored_json(&raw)
        .map_err(|err| format!("解析会话元数据失败，path={}，error={err}", path.display()))
}

/// 读旧 V2 明文 meta 的完整视图（骨 + 肉）。骨从 JSON 键里取，取不到给安全默认值。
pub(super) fn read_conversation_stored_meta(
    path: &PathBuf,
) -> Result<ConversationStoredMeta, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取会话元数据失败，path={}，error={err}", path.display()))?;
    conversation_stored_meta_from_legacy_json(&raw)
        .map_err(|err| format!("解析会话元数据失败，path={}，error={err}", path.display()))
}

#[cfg(test)]
mod message_store_meta_tests {
    use super::*;

    /// 老分片缺附属记录键时整条会话要照常读出来；问题相关键已经不在落盘结构里。
    #[test]
    fn missing_attachment_keys_should_read_as_empty() {
        let conversation = test_conversation();
        let meta = ConversationShardMeta::from_conversation(&conversation);
        let mut value = serde_json::to_value(&meta).expect("serialize meta");
        let object = value.as_object_mut().expect("落盘元数据必须是 JSON 对象");
        for key in ["questions", "questionNextNumber"] {
            assert!(
                !object.contains_key(key),
                "问题不再进 metadata_json：{key}"
            );
        }
        for key in ["issues", "issueNextNumber"] {
            assert!(
                object.remove(key).is_some(),
                "议题清单仍应在落盘结构里：{key}"
            );
        }
        // 落盘结构里没有这两个键，是结构决定的；磁盘上的实况断言放在 chat 的落盘守卫测试里
        // （直读 `conversation_metadata.metadata_json` 原文）。这里补一条真有行为含义的：
        // 老明文里若还残留 questions 键，必须照常读出来并按空开始——不迁移、不报错。
        let mut legacy = value.clone();
        if let Some(object) = legacy.as_object_mut() {
            object.insert(
                "questions".to_string(),
                serde_json::json!([{ "number": 1, "title": "旧题" }]),
            );
            object.insert("questionNextNumber".to_string(), serde_json::json!(9));
        }
        let legacy_raw = serde_json::to_string(&legacy).expect("to text");
        let restored_legacy = conversation_shard_meta_from_stored_json(&legacy_raw)
            .expect("老明文带 questions 键也要读得出来");
        assert_eq!(
            restored_legacy.id(),
            conversation.id,
            "带 legacy questions 键的老行不能被跳过"
        );

        let raw = serde_json::to_string(&value).expect("to text");
        let restored = conversation_shard_meta_from_stored_json(&raw).expect("缺键也要读得出来");
        assert_eq!(
            restored.id(),
            conversation.id,
            "会话本身要读得出来，不许跳过"
        );
        assert!(restored.issues().is_empty(), "议题清单要读成空");
        assert_eq!(restored.issue_next_number(), 0, "议题编号游标要读成 0");
    }

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
            short_id: None,
            loaded_mcp_tools: Vec::new(),
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
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
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
            team: TeamState::default(),
            questions: Vec::new(),
            question_next_number: 0,
            issues: Vec::new(),
            issue_next_number: 0,
        }
    }

    #[test]
    fn message_store_meta_should_round_trip_without_messages() {
        let conversation = test_conversation();
        let meta = ConversationShardMeta::from_conversation(&conversation);
        let restored = meta.clone().into_conversation(conversation.messages.clone());
        let round_tripped: ConversationShardMeta = serde_json::from_str(
            &serde_json::to_string(&meta).expect("serialize meta"),
        )
        .expect("deserialize meta");

        assert_eq!(meta.id, conversation.id);
        assert_eq!(meta.title, conversation.title);
        assert_eq!(meta.current_todos, conversation.current_todos);
        assert_eq!(round_tripped, meta, "metadata_json 往返必须逐字段一致");
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
        assert!(
            restored.fast_request_turns.is_empty(),
            "快速请求不落盘，从落盘结构读回时不应带出记录"
        );
        assert!(
            meta.fast_request_turns.is_empty(),
            "快速请求不落盘，落盘结构里应为空"
        );
        assert_eq!(restored.last_error, conversation.last_error);
        assert_eq!(meta.last_error, conversation.last_error);
        assert_eq!(meta.last_error(), Some("测试轮次失败"));
    }

    /// 快速请求记录只活在内存：序列化时整键不出现，读回恒为空。
    #[test]
    fn fast_request_turns_should_never_reach_persisted_json() {
        let conversation = test_conversation();
        let mut meta = ConversationShardMeta::from_conversation(&conversation);
        meta.push_fast_request_turn(FastRequestTurn {
            id: "fast-request-mem".to_string(),
            kind: "title_generation".to_string(),
            request_text: "request".to_string(),
            response_text: "response".to_string(),
            success: true,
            error: None,
            model_name: Some("quick-model".to_string()),
            duration_ms: Some(8),
            created_at: "2026-04-24T00:00:30Z".to_string(),
        });

        let raw = serde_json::to_string(&meta).expect("serialize meta");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("parse meta json");
        assert!(
            value.get("fastRequestTurns").is_none(),
            "快速请求不落盘，JSON 里不该出现 fastRequestTurns：{raw}"
        );
        assert!(
            !meta.fast_request_turns().is_empty(),
            "内存里仍要保留这份过程记录"
        );

        let restored: ConversationShardMeta = serde_json::from_str(&raw).expect("deserialize meta");
        assert!(
            restored.fast_request_turns().is_empty(),
            "读回时快速请求必须为空"
        );
    }

    #[test]
    fn conversation_shell_recorded_branch_should_round_trip() {        let mut conversation = test_conversation();
        conversation.shell_recorded_branch = "feature/branch-guard".to_string();

        let meta = ConversationShardMeta::from_conversation(&conversation);
        assert_eq!(meta.shell_recorded_branch(), "feature/branch-guard");

        let restored = meta.into_conversation(conversation.messages.clone());
        assert_eq!(restored.shell_recorded_branch, "feature/branch-guard");
    }

    #[test]
    fn legacy_conversation_without_shell_recorded_branch_should_default_to_empty() {
        let conversation = test_conversation();
        let mut value = serde_json::to_value(&conversation).expect("serialize conversation");
        value
            .as_object_mut()
            .expect("conversation object")
            .remove("shellRecordedBranch");

        let restored: Conversation = serde_json::from_value(value).expect("deserialize legacy conversation");

        assert_eq!(restored.shell_recorded_branch, "");
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
        let meta = ConversationStoredMeta::from_conversation(&conversation);

        write_conversation_shard_meta_atomic(&meta_path, &meta).expect("write meta");
        let raw = fs::read_to_string(&meta_path).expect("read raw meta");
        let loaded = read_conversation_shard_meta(&meta_path).expect("read meta");

        assert!(!raw.contains("\"messages\""));
        assert_eq!(loaded, meta.payload().clone());
        let _ = fs::remove_dir_all(root);
    }

}