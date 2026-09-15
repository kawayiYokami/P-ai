fn goal_active_goal_from_conversation(
    conversation: &Conversation,
) -> Option<ConversationGoalState> {
    conversation
        .active_goal
        .as_ref()
        .filter(|goal| conversation_goal_is_active(goal))
        .cloned()
}

struct ConversationTodosUpdateResult {
    current_todo: Option<String>,
}

struct CreateUnarchivedConversationMutationResult {
    conversation_id: String,
    overview_payload: UnarchivedConversationOverviewUpdatedPayload,
}

struct BranchUnarchivedConversationMutationResult {
    conversation_id: String,
    title: String,
    selected_count: usize,
    has_compaction_seed: bool,
}

struct ForwardUnarchivedConversationMutationResult {
    target_conversation_id: String,
    forwarded_count: usize,
}

struct ForwardSelectionToRemoteImContactMutationResult {
    target_conversation_id: String,
    remote_contact_id: String,
    forwarded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolSessionTargetSummary {
    session_id: String,
    kind: String,
    title: String,
    persona_name: Option<String>,
    remote_contact_id: Option<String>,
    remote_contact_name: Option<String>,
    channel_name: Option<String>,
    updated_at: String,
}

struct InformSessionMutationResult {
    target_conversation_id: String,
    target_kind: String,
    remote_contact_id: Option<String>,
    pushed_to_remote: bool,
    message: ChatMessage,
}

struct DeleteUnarchivedConversationMutationResult {
    deleted_conversation_id: String,
    active_conversation_id: String,
}

struct ToggleUnarchivedConversationPinMutationResult {
    conversation_id: String,
    is_pinned: bool,
    pin_index: Option<usize>,
}

struct PromptPrepareConversationResolution {
    conversation_before: Conversation,
    is_remote_im_contact_conversation: bool,
    remote_im_contact_processing_mode: String,
    response_style_id: String,
    user_name: String,
    user_intro: String,
    is_runtime_conversation: bool,
}

struct SchedulerHistoryFlushCommitResult {
    persisted_batch_messages: Vec<ChatMessage>,
    event_activate_flags: Vec<bool>,
}

struct DelegateResultTargetConversationResolution {
    agent_id: String,
    target_conversation_id: String,
}

struct DelegateContextResolution {
    config: AppConfig,
    agents: Vec<AgentProfile>,
    source_agent: AgentProfile,
    target_agent: AgentProfile,
    source_conversation_id: String,
    thread_context: Option<DelegateRuntimeThread>,
}

struct SwitchActiveConversationSnapshotMutationResult {
    snapshot: ForegroundConversationSnapshotCore,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

struct MarkConversationReadResult {
    conversation: Option<Conversation>,
}

struct RewindConversationMutationResult {
    conversation_id: String,
    removed_count: usize,
    remaining_count: usize,
    current_todo: Option<String>,
    current_todos: Vec<ConversationTodoItem>,
    recalled_user_message: Option<ChatMessage>,
    git_snapshot: Option<git_ghost_snapshot::UserMessageGitGhostSnapshotRecord>,
}

struct RewindConversationPreviewResult {
    conversation_id: String,
    can_undo_patch: bool,
    hint: String,
}

struct StopChatPersistResult {
    persisted: bool,
    conversation_id: Option<String>,
    assistant_message: Option<ChatMessage>,
}

struct ImportArchivesMutationResult {
    imported_count: usize,
    replaced_count: usize,
    skipped_count: usize,
    total_count: usize,
    selected_archive_id: Option<String>,
}

struct ConversationBlockSummaryResult {
    block_id: u32,
    message_count: usize,
    first_message_id: String,
    last_message_id: String,
    first_created_at: Option<String>,
    last_created_at: Option<String>,
    is_latest: bool,
}

struct ConversationBlockPageResult {
    blocks: Vec<ConversationBlockSummaryResult>,
    selected_block_id: u32,
    messages: Vec<ChatMessage>,
    has_prev_block: bool,
    has_next_block: bool,
}

struct CompactionMessagePersistResult {
    active_conversation_id: Option<String>,
    compression_message_id: String,
}

struct ListUnarchivedConversationsMutationResult {
    summaries: Vec<UnarchivedConversationSummary>,
}

struct InstantArchiveConversationMutationResult {
    active_conversation_id: String,
    already_archived: bool,
}
