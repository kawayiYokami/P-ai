#[derive(Debug, Default)]
struct ConversationService;

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
    overview_payload: UnarchivedConversationOverviewUpdatedPayload,
}

struct ForwardUnarchivedConversationMutationResult {
    target_conversation_id: String,
    forwarded_count: usize,
    overview_payload: UnarchivedConversationOverviewUpdatedPayload,
}

struct DeleteUnarchivedConversationMutationResult {
    deleted_conversation_id: String,
    active_conversation_id: String,
    overview_payload: UnarchivedConversationOverviewUpdatedPayload,
}

struct ToggleUnarchivedConversationPinMutationResult {
    conversation_id: String,
    is_pinned: bool,
    pin_index: Option<usize>,
}

struct PromptPrepareConversationResolution {
    conversation_before: Conversation,
    last_archive_summary: Option<String>,
    is_remote_im_contact_conversation: bool,
    remote_im_contact_processing_mode: String,
    response_style_id: String,
    user_name: String,
    user_intro: String,
    enable_pdf_images: bool,
    is_runtime_conversation: bool,
}

struct SchedulerHistoryFlushCommitResult {
    persisted_batch_messages: Vec<ChatMessage>,
    event_activate_flags: Vec<bool>,
}

struct DelegateResultTargetConversationResolution {
    department_id: String,
    agent_id: String,
    target_conversation_id: String,
}

struct DelegateContextResolution {
    config: AppConfig,
    agents: Vec<AgentProfile>,
    source_department: DepartmentConfig,
    target_department: DepartmentConfig,
    target_agent_id: String,
    source_conversation_id: String,
    thread_context: Option<DelegateRuntimeThread>,
}

struct AssistantMemoryContext {
    owner_agent_id: Option<String>,
    assistant_department_agent_id: String,
    private_memory_enabled: bool,
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

fn conversation_service() -> &'static ConversationService {
    static SERVICE: OnceLock<ConversationService> = OnceLock::new();
    SERVICE.get_or_init(ConversationService::default)
}

