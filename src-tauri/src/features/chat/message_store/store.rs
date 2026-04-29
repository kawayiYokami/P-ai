#[derive(Debug, Clone)]
pub(super) struct MessageStoreLimitPage {
    pub(super) messages: Vec<ChatMessage>,
    pub(super) has_more: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreStatus {
    pub(super) manifest_exists: bool,
    pub(super) legacy_shard_exists: bool,
    pub(super) directory_shard_exists: bool,
    pub(super) message_store_kind: String,
    pub(super) migration_state: String,
    pub(super) source_message_count: usize,
    pub(super) last_message_id: String,
    pub(super) messages_jsonl_bytes: u64,
    pub(super) ready_jsonl: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreIndexSummary {
    pub(super) message_count: usize,
    pub(super) visible_message_count: usize,
    pub(super) last_message_id: String,
    pub(super) last_message_at: Option<String>,
    pub(super) first_user_text_preview: Option<String>,
    pub(super) preview_items: Vec<MessageStoreIndexPreviewItem>,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreIndexPreviewItem {
    pub(super) message_id: String,
    pub(super) role: String,
    pub(super) speaker_agent_id: Option<String>,
    pub(super) created_at: Option<String>,
    pub(super) text_preview: String,
    pub(super) has_image: bool,
    pub(super) has_pdf: bool,
    pub(super) has_audio: bool,
    pub(super) has_attachment: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreChatSnapshot {
    pub(super) latest_user: Option<ChatMessage>,
    pub(super) latest_assistant: Option<ChatMessage>,
    pub(super) active_message_count: usize,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreCompactionSegment {
    pub(super) messages: Vec<ChatMessage>,
    pub(super) boundary_message_id: Option<String>,
    pub(super) previous_boundary_message_id: Option<String>,
    pub(super) has_previous_segment: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreBlockSummary {
    pub(super) block_id: u32,
    pub(super) message_count: usize,
    pub(super) first_message_id: String,
    pub(super) last_message_id: String,
    pub(super) first_created_at: Option<String>,
    pub(super) last_created_at: Option<String>,
    pub(super) is_latest: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreBlockPage {
    pub(super) blocks: Vec<MessageStoreBlockSummary>,
    pub(super) selected_block_id: u32,
    pub(super) messages: Vec<ChatMessage>,
    pub(super) has_prev_block: bool,
    pub(super) has_next_block: bool,
}

#[derive(Debug, Clone)]
pub(super) struct MessageStoreBranchSelection {
    pub(super) selected_messages: Vec<ChatMessage>,
    pub(super) first_selected_ordinal: usize,
    pub(super) latest_compaction_message: Option<ChatMessage>,
}

trait MessageStore {
    fn read_all_messages(&self) -> Result<Vec<ChatMessage>, String>;
    fn read_recent_messages(&self, limit: usize) -> Result<Vec<ChatMessage>, String>;
    fn read_message_by_id(&self, message_id: &str) -> Result<ChatMessage, String>;
    fn read_messages_before(&self, before_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String>;
    fn read_messages_after(&self, after_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String>;
    fn read_current_compaction_segment(&self) -> Result<MessageStoreCompactionSegment, String>;
    fn read_compaction_segment_before(&self, boundary_message_id: &str) -> Result<MessageStoreCompactionSegment, String>;
}

struct ConversationJsonMessageStore<'a> {
    conversation: &'a Conversation,
}

struct JsonlSnapshotMessageStore {
    messages_file: PathBuf,
    index_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct CachedMessageStoreBlockFile {
    modified_at: Option<std::time::SystemTime>,
    len: u64,
    messages_by_id: Arc<std::collections::HashMap<String, ChatMessage>>,
}

enum MessageStoreBackend<'a> {
    ConversationJson(ConversationJsonMessageStore<'a>),
    JsonlSnapshot(JsonlSnapshotMessageStore),
}

static MESSAGE_STORE_BLOCK_FILE_CACHE: OnceLock<
    Mutex<std::collections::HashMap<PathBuf, CachedMessageStoreBlockFile>>,
> = OnceLock::new();

fn message_store_block_file_cache(
) -> &'static Mutex<std::collections::HashMap<PathBuf, CachedMessageStoreBlockFile>> {
    MESSAGE_STORE_BLOCK_FILE_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn lock_message_store_block_file_cache(
) -> std::sync::MutexGuard<
    'static,
    std::collections::HashMap<PathBuf, CachedMessageStoreBlockFile>,
> {
    message_store_block_file_cache().lock().unwrap_or_else(|poison| {
        eprintln!(
            "[消息存储] 会话块缓存锁已污染，继续使用内部状态，error={:?}",
            poison
        );
        poison.into_inner()
    })
}

pub(super) fn retain_message_store_block_file_cache_paths(
    allowed_paths: &std::collections::HashSet<PathBuf>,
) {
    let mut cache = lock_message_store_block_file_cache();
    cache.retain(|path, _| allowed_paths.contains(path));
}

pub(super) fn forget_message_store_block_file_cache_paths(
    paths: &std::collections::HashSet<PathBuf>,
) {
    if paths.is_empty() {
        return;
    }
    let mut cache = lock_message_store_block_file_cache();
    for path in paths {
        cache.remove(path);
    }
}

impl<'a> ConversationJsonMessageStore<'a> {
    fn new(conversation: &'a Conversation) -> Self {
        Self { conversation }
    }

    fn messages(&self) -> &[ChatMessage] {
        &self.conversation.messages
    }
}

impl JsonlSnapshotMessageStore {
    fn new(messages_file: PathBuf) -> Self {
        Self {
            messages_file,
            index_file: None,
        }
    }

    fn with_index(messages_file: PathBuf, index_file: PathBuf) -> Self {
        Self {
            messages_file,
            index_file: Some(index_file),
        }
    }

    fn messages(&self) -> Result<Vec<ChatMessage>, String> {
        if let Some(index) = self.index()? {
            return read_jsonl_snapshot_messages_by_index_items(&self.messages_file, &index.items);
        }
        read_jsonl_snapshot_messages_file(&self.messages_file)
    }

    fn index(&self) -> Result<Option<Arc<MessageStoreIndexFile>>, String> {
        let Some(index_file) = self.index_file.as_ref() else {
            return Ok(None);
        };
        if !index_file.exists() {
            return Ok(None);
        }
        read_message_store_index_file(index_file).map(Some)
    }

    fn read_messages_after_all(&self, after_message_id: &str) -> Result<Vec<ChatMessage>, String> {
        if let Some(index) = self.index()? {
            return read_messages_after_all_from_index(&self.messages_file, &index, after_message_id);
        }
        let messages = self.messages()?;
        read_messages_after_all_from_slice(&messages, after_message_id)
    }

    fn read_recent_messages_page(&self, limit: usize) -> Result<MessageStoreLimitPage, String> {
        if let Some(index) = self.index()? {
            return read_recent_messages_page_from_index(&self.messages_file, &index, limit);
        }
        let messages = self.messages()?;
        read_recent_messages_page_from_slice(&messages, limit)
    }
}

impl MessageStore for ConversationJsonMessageStore<'_> {
    fn read_all_messages(&self) -> Result<Vec<ChatMessage>, String> {
        Ok(self.messages().to_vec())
    }

    fn read_recent_messages(&self, limit: usize) -> Result<Vec<ChatMessage>, String> {
        read_recent_messages_from_slice(self.messages(), limit)
    }

    fn read_message_by_id(&self, message_id: &str) -> Result<ChatMessage, String> {
        read_message_by_id_from_slice(self.messages(), message_id)
    }

    fn read_messages_before(&self, before_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        read_messages_before_from_slice(self.messages(), before_message_id, limit)
    }

    fn read_messages_after(&self, after_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        read_messages_after_from_slice(self.messages(), after_message_id, limit)
    }

    fn read_current_compaction_segment(&self) -> Result<MessageStoreCompactionSegment, String> {
        read_current_compaction_segment_from_slice(self.messages())
    }

    fn read_compaction_segment_before(&self, boundary_message_id: &str) -> Result<MessageStoreCompactionSegment, String> {
        read_compaction_segment_before_from_slice(self.messages(), boundary_message_id)
    }
}

impl MessageStore for JsonlSnapshotMessageStore {
    fn read_all_messages(&self) -> Result<Vec<ChatMessage>, String> {
        self.messages()
    }

    fn read_recent_messages(&self, limit: usize) -> Result<Vec<ChatMessage>, String> {
        if let Some(index) = self.index()? {
            return read_recent_messages_from_index(&self.messages_file, &index, limit);
        }
        let messages = self.messages()?;
        read_recent_messages_from_slice(&messages, limit)
    }

    fn read_message_by_id(&self, message_id: &str) -> Result<ChatMessage, String> {
        if let Some(index) = self.index()? {
            return read_message_by_id_from_index(&self.messages_file, &index, message_id);
        }
        let messages = self.messages()?;
        read_message_by_id_from_slice(&messages, message_id)
    }

    fn read_messages_before(&self, before_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        if let Some(index) = self.index()? {
            return read_messages_before_from_index(&self.messages_file, &index, before_message_id, limit);
        }
        let messages = self.messages()?;
        read_messages_before_from_slice(&messages, before_message_id, limit)
    }

    fn read_messages_after(&self, after_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        if let Some(index) = self.index()? {
            return read_messages_after_from_index(&self.messages_file, &index, after_message_id, limit);
        }
        let messages = self.messages()?;
        read_messages_after_from_slice(&messages, after_message_id, limit)
    }

    fn read_current_compaction_segment(&self) -> Result<MessageStoreCompactionSegment, String> {
        if let Some(index) = self.index()? {
            return read_current_compaction_segment_from_index(&self.messages_file, &index);
        }
        let messages = self.messages()?;
        read_current_compaction_segment_from_slice(&messages)
    }

    fn read_compaction_segment_before(&self, boundary_message_id: &str) -> Result<MessageStoreCompactionSegment, String> {
        if let Some(index) = self.index()? {
            return read_compaction_segment_before_from_index(&self.messages_file, &index, boundary_message_id);
        }
        let messages = self.messages()?;
        read_compaction_segment_before_from_slice(&messages, boundary_message_id)
    }
}

impl MessageStore for MessageStoreBackend<'_> {
    fn read_all_messages(&self) -> Result<Vec<ChatMessage>, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_all_messages(),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_all_messages(),
        }
    }

    fn read_recent_messages(&self, limit: usize) -> Result<Vec<ChatMessage>, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_recent_messages(limit),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_recent_messages(limit),
        }
    }

    fn read_message_by_id(&self, message_id: &str) -> Result<ChatMessage, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_message_by_id(message_id),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_message_by_id(message_id),
        }
    }

    fn read_messages_before(&self, before_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_messages_before(before_message_id, limit),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_messages_before(before_message_id, limit),
        }
    }

    fn read_messages_after(&self, after_message_id: &str, limit: usize) -> Result<MessageStoreLimitPage, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_messages_after(after_message_id, limit),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_messages_after(after_message_id, limit),
        }
    }

    fn read_current_compaction_segment(&self) -> Result<MessageStoreCompactionSegment, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_current_compaction_segment(),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_current_compaction_segment(),
        }
    }

    fn read_compaction_segment_before(&self, boundary_message_id: &str) -> Result<MessageStoreCompactionSegment, String> {
        match self {
            MessageStoreBackend::ConversationJson(store) => store.read_compaction_segment_before(boundary_message_id),
            MessageStoreBackend::JsonlSnapshot(store) => store.read_compaction_segment_before(boundary_message_id),
        }
    }
}

fn message_store_backend_for_conversation<'a>(
    paths: &MessageStorePaths,
    conversation: &'a Conversation,
) -> Result<MessageStoreBackend<'a>, String> {
    let manifest = read_message_store_manifest(&paths.manifest_file)?;
    if let Some(item) = manifest.as_ref() {
        if matches!(
            (item.message_store_kind, item.migration_state),
            (MessageStoreKind::JsonlSnapshot, MessageStoreMigrationState::Ready)
        ) {
            validate_ready_message_store_snapshot_integrity(paths, item)?;
            return Ok(MessageStoreBackend::JsonlSnapshot(
                JsonlSnapshotMessageStore::with_index(
                    paths.messages_file.clone(),
                    paths.index_file.clone(),
                ),
            ));
        }
        if matches!(
            (item.message_store_kind, item.migration_state),
            (MessageStoreKind::JsonlEventLog, MessageStoreMigrationState::Ready)
        ) {
            return Err(format!(
                "消息存储暂不支持读取 JSONL 事件日志，conversation_id={}",
                conversation.id
            ));
        }
    }
    if let Some(reason) = manifest.and_then(|item| item.stale_jsonl_reason()) {
        eprintln!(
            "[消息存储] 跳过目录型消息 store，conversation_id={}，reason={}",
            conversation.id, reason
        );
    }
    Ok(MessageStoreBackend::ConversationJson(
        ConversationJsonMessageStore::new(conversation),
    ))
}

pub(super) fn read_ready_message_store_directory_conversation(
    paths: &MessageStorePaths,
) -> Result<Option<Conversation>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlSnapshot,
            MessageStoreMigrationState::Ready
        )
    ) {
        return read_message_store_directory_conversation_with_manifest(paths, manifest).map(Some);
    }
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlEventLog,
            MessageStoreMigrationState::Ready
        )
    ) {
        return Err(format!(
            "目录型会话暂不支持读取 JSONL 事件日志，path={}",
            paths.manifest_file.display()
        ));
    }
    Ok(None)
}

pub(super) fn read_ready_message_store_meta(
    paths: &MessageStorePaths,
) -> Result<Option<ConversationShardMeta>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlSnapshot,
            MessageStoreMigrationState::Ready
        )
    ) {
        let meta = read_conversation_shard_meta(&paths.meta_file)?;
        validate_conversation_shard_meta_id(paths, &meta)?;
        return Ok(Some(meta));
    }
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlEventLog,
            MessageStoreMigrationState::Ready
        )
    ) {
        return Err(format!(
            "目录型会话暂不支持读取 JSONL 事件日志元数据，path={}",
            paths.manifest_file.display()
        ));
    }
    Ok(None)
}

pub(super) fn read_ready_message_store_all_messages(
    paths: &MessageStorePaths,
) -> Result<Option<Vec<ChatMessage>>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_all_messages().map(Some)
}

pub(super) fn read_ready_message_store_recent_messages(
    paths: &MessageStorePaths,
    limit: usize,
) -> Result<Option<Vec<ChatMessage>>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_recent_messages(limit).map(Some)
}

pub(super) fn read_ready_message_store_recent_messages_page(
    paths: &MessageStorePaths,
    limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_recent_messages_page(limit).map(Some)
}

pub(super) fn read_ready_message_store_recent_messages_page_cached(
    paths: &MessageStorePaths,
    limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    let limit = normalized_message_limit(limit);
    let start = index.items.len().saturating_sub(limit);
    let messages =
        read_jsonl_snapshot_messages_by_index_items_cached(&paths.messages_file, &index.items[start..])?;
    Ok(Some(MessageStoreLimitPage {
        messages,
        has_more: start > 0,
    }))
}

pub(super) fn read_ready_message_store_recent_blocks_page(
    paths: &MessageStorePaths,
    block_limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    read_ready_message_store_recent_blocks_page_with_cache(paths, block_limit, false)
}

pub(super) fn read_ready_message_store_recent_blocks_page_cached(
    paths: &MessageStorePaths,
    block_limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    read_ready_message_store_recent_blocks_page_with_cache(paths, block_limit, true)
}

pub(super) fn read_ready_message_store_latest_block_paths(
    paths: &MessageStorePaths,
    block_limit: usize,
) -> Result<Option<Vec<PathBuf>>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    let mut block_ids = ordered_message_store_index_block_ids(&index);
    if block_ids.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let normalized_limit = block_limit.clamp(1, 8);
    let start = block_ids.len().saturating_sub(normalized_limit);
    block_ids = block_ids[start..].to_vec();
    let mut block_paths = Vec::<PathBuf>::with_capacity(block_ids.len());
    for block_id in block_ids {
        block_paths.push(jsonl_snapshot_index_item_path(
            &paths.messages_file,
            Some(block_id),
        )?);
    }
    Ok(Some(block_paths))
}

fn read_ready_message_store_recent_blocks_page_with_cache(
    paths: &MessageStorePaths,
    block_limit: usize,
    use_block_cache: bool,
) -> Result<Option<MessageStoreLimitPage>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    let block_ids = ordered_message_store_index_block_ids(&index);
    if block_ids.is_empty() {
        return Ok(Some(MessageStoreLimitPage {
            messages: Vec::new(),
            has_more: false,
        }));
    }
    let normalized_limit = block_limit.clamp(1, 8);
    let selected_block_ids = block_ids
        .iter()
        .rev()
        .take(normalized_limit)
        .copied()
        .collect::<Vec<_>>();
    let mut selected_block_ids = selected_block_ids;
    selected_block_ids.reverse();
    let selected_block_ids = selected_block_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let selected_items = index
        .items
        .iter()
        .filter(|item| selected_block_ids.contains(&item.block_id.unwrap_or(0)))
        .cloned()
        .collect::<Vec<_>>();
    let messages = if use_block_cache {
        read_jsonl_snapshot_messages_by_index_items_cached(&paths.messages_file, &selected_items)?
    } else {
        read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &selected_items)?
    };
    Ok(Some(MessageStoreLimitPage {
        messages,
        has_more: block_ids.len() > normalized_limit,
    }))
}

pub(super) fn read_ready_message_store_message_by_id(
    paths: &MessageStorePaths,
    message_id: &str,
) -> Result<Option<ChatMessage>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_message_by_id(message_id).map(Some)
}

pub(super) fn read_ready_message_store_messages_after_all(
    paths: &MessageStorePaths,
    after_message_id: &str,
) -> Result<Option<Vec<ChatMessage>>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_messages_after_all(after_message_id).map(Some)
}

pub(super) fn read_message_store_status(
    paths: &MessageStorePaths,
    fallback_conversation: &Conversation,
) -> Result<MessageStoreStatus, String> {
    let manifest = read_message_store_manifest(&paths.manifest_file)?;
    let fallback_last_message_id = fallback_conversation
        .messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    let (
        message_store_kind,
        migration_state,
        source_message_count,
        last_message_id,
        messages_jsonl_bytes,
        ready_jsonl,
    ) = if let Some(item) = manifest.as_ref() {
        (
            item.store_kind_label().to_string(),
            item.migration_state_label().to_string(),
            item.source_message_count(),
            item.last_message_id().to_string(),
            item.messages_jsonl_bytes(),
            item.should_read_jsonl(),
        )
    } else {
        (
            "conversationJson".to_string(),
            "none".to_string(),
            fallback_conversation.messages.len(),
            fallback_last_message_id,
            0,
            false,
        )
    };
    Ok(MessageStoreStatus {
        manifest_exists: manifest.is_some(),
        legacy_shard_exists: paths.legacy_conversation_file.exists(),
        directory_shard_exists: paths.shard_dir.exists(),
        message_store_kind,
        migration_state,
        source_message_count,
        last_message_id,
        messages_jsonl_bytes,
        ready_jsonl,
    })
}

pub(super) fn read_ready_message_store_status(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreStatus>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    let meta = read_conversation_shard_meta(&paths.meta_file)
        .map_err(|err| format!("ready JSONL 会话缺少可读 meta.json，无法读取消息存储状态: {err}"))?;
    validate_conversation_shard_meta_id(paths, &meta)?;
    Ok(Some(MessageStoreStatus {
        manifest_exists: true,
        legacy_shard_exists: paths.legacy_conversation_file.exists(),
        directory_shard_exists: paths.shard_dir.exists(),
        message_store_kind: manifest.store_kind_label().to_string(),
        migration_state: manifest.migration_state_label().to_string(),
        source_message_count: manifest.source_message_count(),
        last_message_id: manifest.last_message_id().to_string(),
        messages_jsonl_bytes: manifest.messages_jsonl_bytes(),
        ready_jsonl: true,
    }))
}

pub(super) fn read_ready_message_store_index_summary(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreIndexSummary>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    let messages = read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &index.items)?;
    let last = messages.last();
    let visible_messages = messages
        .iter()
        .filter(|message| {
            !message_store_message_is_tool_review_report(message)
                && matches!(
                    message.role.trim().to_ascii_lowercase().as_str(),
                    "user" | "assistant" | "tool"
                )
        })
        .collect::<Vec<_>>();
    let first_user_text_preview = messages
        .iter()
        .find(|message| {
            message.role.trim().eq_ignore_ascii_case("user")
                && message
                    .speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    != Some(SYSTEM_PERSONA_ID)
                && !build_conversation_preview_text(message).trim().is_empty()
        })
        .map(|message| build_conversation_preview_text(message).trim().to_string());
    let preview_start = visible_messages.len().saturating_sub(2);
    let preview_items = visible_messages[preview_start..]
        .iter()
        .map(|message| MessageStoreIndexPreviewItem {
            message_id: message.id.clone(),
            role: message.role.clone(),
            speaker_agent_id: message.speaker_agent_id.clone(),
            created_at: Some(message.created_at.clone()).filter(|value| !value.trim().is_empty()),
            text_preview: build_conversation_preview_text(message),
            has_image: message_store_message_has_image(message),
            has_pdf: message_store_message_has_pdf(message),
            has_audio: message_store_message_has_audio(message),
            has_attachment: conversation_message_has_attachment(message),
        })
        .collect::<Vec<_>>();
    Ok(Some(MessageStoreIndexSummary {
        message_count: index.items.len(),
        visible_message_count: visible_messages.len(),
        last_message_id: last
            .map(|message| message.id.trim().to_string())
            .unwrap_or_default(),
        last_message_at: last.map(|message| message.created_at.clone()),
        first_user_text_preview,
        preview_items,
    }))
}

pub(super) fn read_ready_message_store_chat_snapshot(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreChatSnapshot>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    let messages = read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &index.items)?;
    let latest_user = messages
        .iter()
        .rev()
        .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
        .cloned();
    let latest_assistant = messages
        .iter()
        .rev()
        .find(|message| {
            message.role.trim().eq_ignore_ascii_case("assistant")
                && !message_store_message_is_tool_review_report(message)
        })
        .cloned();
    let active_message_count = messages
        .iter()
        .filter(|message| !message_store_message_is_tool_review_report(message))
        .count();
    Ok(Some(MessageStoreChatSnapshot {
        latest_user,
        latest_assistant,
        active_message_count,
    }))
}

pub(super) fn read_ready_message_store_branch_selection(
    paths: &MessageStorePaths,
    selected_message_ids: &[String],
) -> Result<Option<MessageStoreBranchSelection>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let selected_ids = selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let index = read_message_store_index_file(&paths.index_file)?;
    let mut selected_items = Vec::<MessageStoreIndexItem>::new();
    let mut visible_ordinal = 0usize;
    let mut first_selected_ordinal = 0usize;
    let mut latest_compaction_item: Option<MessageStoreIndexItem> = None;
    let boundaries = compaction_boundary_index_items(&index)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    for (item_idx, item) in index.items.iter().enumerate() {
        if boundaries.contains(&item_idx) {
            latest_compaction_item = Some(item.clone());
            continue;
        }
        visible_ordinal += 1;
        if selected_ids.contains(item.message_id.trim()) {
            if first_selected_ordinal == 0 {
                first_selected_ordinal = visible_ordinal;
            }
            selected_items.push(item.clone());
        }
    }
    let selected_messages =
        read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &selected_items)?;
    let latest_compaction_message = if let Some(item) = latest_compaction_item {
        let mut messages =
            read_jsonl_snapshot_messages_by_index_items(&paths.messages_file, &[item])?;
        messages.pop()
    } else {
        None
    };
    Ok(Some(MessageStoreBranchSelection {
        selected_messages,
        first_selected_ordinal,
        latest_compaction_message,
    }))
}

pub(super) fn read_message_store_manifest_status(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreStatus>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    Ok(Some(MessageStoreStatus {
        manifest_exists: true,
        legacy_shard_exists: paths.legacy_conversation_file.exists(),
        directory_shard_exists: paths.shard_dir.exists(),
        message_store_kind: manifest.store_kind_label().to_string(),
        migration_state: manifest.migration_state_label().to_string(),
        source_message_count: manifest.source_message_count(),
        last_message_id: manifest.last_message_id().to_string(),
        messages_jsonl_bytes: manifest.messages_jsonl_bytes(),
        ready_jsonl: manifest.should_read_jsonl(),
    }))
}

pub(super) fn read_ready_message_store_messages_before(
    paths: &MessageStorePaths,
    before_message_id: &str,
    limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_messages_before(before_message_id, limit).map(Some)
}

pub(super) fn read_ready_message_store_messages_after(
    paths: &MessageStorePaths,
    after_message_id: &str,
    limit: usize,
) -> Result<Option<MessageStoreLimitPage>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_messages_after(after_message_id, limit).map(Some)
}

pub(super) fn read_ready_message_store_current_compaction_segment(
    paths: &MessageStorePaths,
) -> Result<Option<MessageStoreCompactionSegment>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store.read_current_compaction_segment().map(Some)
}

pub(super) fn read_ready_message_store_compaction_segment_before(
    paths: &MessageStorePaths,
    boundary_message_id: &str,
) -> Result<Option<MessageStoreCompactionSegment>, String> {
    let Some(store) = ready_jsonl_snapshot_store(paths)? else {
        return Ok(None);
    };
    store
        .read_compaction_segment_before(boundary_message_id)
        .map(Some)
}

pub(super) fn read_ready_message_store_block_page(
    paths: &MessageStorePaths,
    requested_block_id: Option<u32>,
) -> Result<Option<MessageStoreBlockPage>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if !manifest.should_read_jsonl() {
        return Ok(None);
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    if !paths.index_file.exists() {
        return Ok(None);
    }
    let index = read_message_store_index_file(&paths.index_file)?;
    if index.items.is_empty() {
        return Ok(Some(MessageStoreBlockPage {
            blocks: Vec::new(),
            selected_block_id: requested_block_id.unwrap_or(0),
            messages: Vec::new(),
            has_prev_block: false,
            has_next_block: false,
        }));
    }
    let summaries = build_message_store_block_summaries(paths, &index)?;
    let selected_block_id = requested_block_id
        .filter(|block_id| summaries.iter().any(|item| item.block_id == *block_id))
        .or_else(|| summaries.last().map(|item| item.block_id))
        .unwrap_or(0);
    let selected_idx = summaries
        .iter()
        .position(|item| item.block_id == selected_block_id)
        .ok_or_else(|| {
            format!(
                "会话块不存在，conversation_id={}，block_id={selected_block_id}",
                paths.conversation_id
            )
        })?;
    let selected_items = index
        .items
        .iter()
        .filter(|item| item.block_id.unwrap_or(0) == selected_block_id)
        .cloned()
        .collect::<Vec<_>>();
    let messages =
        read_jsonl_snapshot_messages_by_index_items_cached(&paths.messages_file, &selected_items)?;
    let has_next_block = selected_idx + 1 < summaries.len();
    Ok(Some(MessageStoreBlockPage {
        blocks: summaries,
        selected_block_id,
        messages,
        has_prev_block: selected_idx > 0,
        has_next_block,
    }))
}

pub(super) fn read_message_store_current_compaction_segment_for_conversation(
    paths: &MessageStorePaths,
    conversation: &Conversation,
) -> Result<MessageStoreCompactionSegment, String> {
    message_store_backend_for_conversation(paths, conversation)?
        .read_current_compaction_segment()
}

pub(super) fn read_message_store_compaction_segment_before_for_conversation(
    paths: &MessageStorePaths,
    conversation: &Conversation,
    boundary_message_id: &str,
) -> Result<MessageStoreCompactionSegment, String> {
    message_store_backend_for_conversation(paths, conversation)?
        .read_compaction_segment_before(boundary_message_id)
}

fn ready_jsonl_snapshot_store(
    paths: &MessageStorePaths,
) -> Result<Option<JsonlSnapshotMessageStore>, String> {
    let Some(manifest) = read_message_store_manifest(&paths.manifest_file)? else {
        return Ok(None);
    };
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlSnapshot,
            MessageStoreMigrationState::Ready
        )
    ) {
        validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
        return Ok(Some(JsonlSnapshotMessageStore::with_index(
            paths.messages_file.clone(),
            paths.index_file.clone(),
        )));
    }
    if matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (
            MessageStoreKind::JsonlEventLog,
            MessageStoreMigrationState::Ready
        )
    ) {
        return Err(format!(
            "目录型会话暂不支持读取 JSONL 事件日志，path={}",
            paths.manifest_file.display()
        ));
    }
    Ok(None)
}

pub(super) fn validate_ready_message_store_snapshot_integrity(
    paths: &MessageStorePaths,
    manifest: &MessageStoreManifest,
) -> Result<(), String> {
    if !manifest.should_read_jsonl() {
        return Ok(());
    }
    let actual_bytes = fs::metadata(&paths.messages_file)
        .map(|metadata| metadata.len())
        .or_else(|single_file_err| {
            let index = read_message_store_index_file(&paths.index_file)?;
            message_store_index_total_bytes(paths, &index).map_err(|err| {
                format!(
                    "ready JSONL 会话缺少可读 messages.jsonl 且会话块校验失败，path={}，single_file_error={}，block_error={}",
                    paths.messages_file.display(),
                    single_file_err,
                    err
                )
            })
        })?;
    if actual_bytes != manifest.messages_jsonl_bytes() {
        return Err(format!(
            "ready JSONL 会话消息文件大小不一致，conversation_id={}，manifest_bytes={}，actual_bytes={}，path={}",
            paths.conversation_id,
            manifest.messages_jsonl_bytes(),
            actual_bytes,
            paths.messages_file.display()
        ));
    }
    if paths.index_file.exists() {
        let index = read_message_store_index_file(&paths.index_file)?;
        if index.items.len() != manifest.source_message_count() {
            return Err(format!(
                "ready JSONL 会话索引数量不一致，conversation_id={}，manifest={}，actual={}，path={}",
                paths.conversation_id,
                manifest.source_message_count(),
                index.items.len(),
                paths.index_file.display()
            ));
        }
        let actual_last_message_id = index
            .items
            .last()
            .map(|item| item.message_id.trim())
            .unwrap_or_default();
        if actual_last_message_id != manifest.last_message_id().trim() {
            return Err(format!(
                "ready JSONL 会话索引最后消息不一致，conversation_id={}，manifest={}，actual={}，path={}",
                paths.conversation_id,
                manifest.last_message_id(),
                actual_last_message_id,
                paths.index_file.display()
            ));
        }
    }
    Ok(())
}

fn message_store_index_total_bytes(
    paths: &MessageStorePaths,
    index: &MessageStoreIndexFile,
) -> Result<u64, String> {
    let mut block_ids = std::collections::BTreeSet::<Option<u32>>::new();
    for item in &index.items {
        block_ids.insert(item.block_id);
    }
    let mut total = 0_u64;
    for block_id in block_ids {
        let path = jsonl_snapshot_index_item_path(&paths.messages_file, block_id)?;
        let len = fs::metadata(&path)
            .map_err(|err| {
                format!(
                    "读取会话块元数据失败，conversation_id={}，path={}，error={err}",
                    paths.conversation_id,
                    path.display()
                )
            })?
            .len();
        total = total.checked_add(len).ok_or_else(|| {
            format!(
                "统计会话块字节数失败：总字节数溢出，conversation_id={}，path={}",
                paths.conversation_id,
                path.display()
            )
        })?;
    }
    Ok(total)
}

fn read_message_store_directory_conversation(paths: &MessageStorePaths) -> Result<Conversation, String> {
    let manifest = read_message_store_manifest(&paths.manifest_file)?
        .ok_or_else(|| format!("目录型会话缺少 manifest，path={}", paths.manifest_file.display()))?;
    read_message_store_directory_conversation_with_manifest(paths, manifest)
}

fn read_message_store_directory_conversation_with_manifest(
    paths: &MessageStorePaths,
    manifest: MessageStoreManifest,
) -> Result<Conversation, String> {
    if !matches!(
        (manifest.message_store_kind, manifest.migration_state),
        (MessageStoreKind::JsonlSnapshot, MessageStoreMigrationState::Ready)
    ) {
        return Err(format!(
            "目录型会话 manifest 未处于可读取快照状态: kind={:?}, state={:?}",
            manifest.message_store_kind, manifest.migration_state
        ));
    }
    validate_ready_message_store_snapshot_integrity(paths, &manifest)?;
    let meta = read_conversation_shard_meta(&paths.meta_file)?;
    validate_conversation_shard_meta_id(paths, &meta)?;
    let messages = JsonlSnapshotMessageStore::with_index(
        paths.messages_file.clone(),
        paths.index_file.clone(),
    )
    .read_all_messages()?;
    if manifest.source_message_count != messages.len() {
        return Err(format!(
            "目录型会话消息数量不一致，conversation_id={}，manifest={}，actual={}",
            meta.id,
            manifest.source_message_count,
            messages.len()
        ));
    }
    let actual_last_message_id = messages
        .last()
        .map(|message| message.id.trim().to_string())
        .unwrap_or_default();
    if manifest.last_message_id.trim() != actual_last_message_id {
        return Err(format!(
            "目录型会话最后消息不一致，conversation_id={}，manifest={}，actual={}",
            meta.id, manifest.last_message_id, actual_last_message_id
        ));
    }
    Ok(meta.into_conversation(messages))
}

fn validate_conversation_shard_meta_id(
    paths: &MessageStorePaths,
    meta: &ConversationShardMeta,
) -> Result<(), String> {
    if meta.id.trim() != paths.conversation_id {
        return Err(format!(
            "目录型会话元数据 ID 不一致，expected={}，actual={}，path={}",
            paths.conversation_id,
            meta.id,
            paths.meta_file.display()
        ));
    }
    Ok(())
}

pub(super) fn delete_message_store_shard_artifacts(
    paths: &MessageStorePaths,
) -> Result<bool, String> {
    if paths.shard_dir.exists() {
        validate_message_store_shard_dir_for_delete(paths)?;
    }
    let mut changed = false;
    if paths.legacy_conversation_file.exists() {
        fs::remove_file(&paths.legacy_conversation_file).map_err(|err| {
            format!(
                "删除旧会话分片失败，path={}，error={err}",
                paths.legacy_conversation_file.display()
            )
        })?;
        changed = true;
    }
    if paths.shard_dir.exists() {
        fs::remove_dir_all(&paths.shard_dir).map_err(|err| {
            format!(
                "删除目录型会话分片失败，path={}，error={err}",
                paths.shard_dir.display()
            )
        })?;
        forget_message_store_index_cache(&paths.index_file);
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
pub(super) fn message_store_shard_write_signature(
    paths: &MessageStorePaths,
) -> Vec<(PathBuf, u64, Option<std::time::SystemTime>)> {
    let mut signatures = Vec::new();
    collect_message_store_shard_write_signatures(&paths.legacy_conversation_file, &mut signatures);
    collect_message_store_shard_write_signatures(&paths.shard_dir, &mut signatures);
    signatures.sort_by(|left, right| left.0.cmp(&right.0));
    signatures
}

#[cfg(test)]
fn collect_message_store_shard_write_signatures(
    path: &PathBuf,
    signatures: &mut Vec<(PathBuf, u64, Option<std::time::SystemTime>)>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_file() {
        signatures.push((path.clone(), metadata.len(), metadata.modified().ok()));
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_message_store_shard_write_signatures(&entry.path(), signatures);
    }
}

fn validate_message_store_shard_dir_for_delete(paths: &MessageStorePaths) -> Result<(), String> {
    let conversations_dir = paths
        .legacy_conversation_file
        .parent()
        .ok_or_else(|| "删除目录型会话分片失败：旧会话文件缺少父目录".to_string())?;
    let shard_parent = paths
        .shard_dir
        .parent()
        .ok_or_else(|| "删除目录型会话分片失败：目录型分片缺少父目录".to_string())?;
    if shard_parent != conversations_dir {
        return Err(format!(
            "删除目录型会话分片失败：分片目录不在 conversations 目录内，shard={}，conversations={}",
            paths.shard_dir.display(),
            conversations_dir.display()
        ));
    }
    if paths.shard_dir == conversations_dir {
        return Err(format!(
            "删除目录型会话分片失败：拒绝删除 conversations 根目录，path={}",
            paths.shard_dir.display()
        ));
    }
    if paths.shard_dir.file_name().is_none() {
        return Err(format!(
            "删除目录型会话分片失败：分片目录名为空，path={}",
            paths.shard_dir.display()
        ));
    }
    Ok(())
}

fn read_jsonl_snapshot_messages_file(path: &PathBuf) -> Result<Vec<ChatMessage>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取 JSONL 消息文件失败，path={}，error={err}", path.display()))?;
    read_jsonl_snapshot_messages_from_content(&raw)
}

fn read_jsonl_snapshot_messages_from_content(content: &str) -> Result<Vec<ChatMessage>, String> {
    let report = verify_jsonl_snapshot_content(content, usize::MAX, "")?;
    let mut messages = Vec::with_capacity(report.message_count);
    for (line_no, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let message = decode_jsonl_snapshot_message(line)
            .map_err(|err| format!("解析 JSONL 消息失败，line={}，error={err}", line_no + 1))?;
        messages.push(message);
    }
    Ok(messages)
}

fn find_index_item_position(index: &MessageStoreIndexFile, message_id: &str) -> Option<usize> {
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return None;
    }
    if let Some(position) = index.positions_by_message_id.get(message_id) {
        return Some(*position);
    }
    index
        .items
        .iter()
        .position(|item| item.message_id.trim() == message_id)
}

fn read_jsonl_snapshot_messages_by_index_items(
    path: &PathBuf,
    items: &[MessageStoreIndexItem],
) -> Result<Vec<ChatMessage>, String> {
    let mut messages = Vec::with_capacity(items.len());
    let mut current_file_path = PathBuf::new();
    let mut current_file: Option<fs::File> = None;
    for item in items {
        let item_path = jsonl_snapshot_index_item_path(path, item.block_id)?;
        if current_file_path != item_path {
            current_file = Some(fs::File::open(&item_path).map_err(|err| {
                format!(
                    "打开 JSONL 消息文件失败，path={}，message_id={}，error={err}",
                    item_path.display(),
                    item.message_id
                )
            })?);
            current_file_path = item_path;
        }
        let Some(file) = current_file.as_mut() else {
            return Err(format!("打开 JSONL 消息文件失败，message_id={}", item.message_id));
        };
        std::io::Seek::seek(file, std::io::SeekFrom::Start(item.offset)).map_err(|err| {
            format!(
                "定位 JSONL 消息失败，path={}，message_id={}，offset={}，error={err}",
                current_file_path.display(),
                item.message_id,
                item.offset
            )
        })?;
        let mut buffer = vec![0_u8; item.byte_len as usize];
        std::io::Read::read_exact(file, &mut buffer).map_err(|err| {
            format!(
                "读取 JSONL 消息失败，path={}，message_id={}，offset={}，byte_len={}，error={err}",
                current_file_path.display(),
                item.message_id,
                item.offset,
                item.byte_len
            )
        })?;
        let raw = String::from_utf8(buffer)
            .map_err(|err| format!("JSONL 消息不是 UTF-8，message_id={}，error={err}", item.message_id))?;
        let line = raw.trim_end_matches(['\r', '\n']);
        let message = decode_jsonl_snapshot_message(line)
            .map_err(|err| format!("解析 JSONL 消息失败，message_id={}，error={err}", item.message_id))?;
        if message.id.trim() != item.message_id.trim() {
            return Err(format!(
                "JSONL 索引与消息不一致，path={}，expected_message_id={}，actual_message_id={}，offset={}，byte_len={}",
                current_file_path.display(),
                item.message_id,
                message.id,
                item.offset,
                item.byte_len
            ));
        }
        messages.push(message);
    }
    Ok(messages)
}

fn read_jsonl_snapshot_messages_by_index_items_cached(
    path: &PathBuf,
    items: &[MessageStoreIndexItem],
) -> Result<Vec<ChatMessage>, String> {
    let mut messages = Vec::with_capacity(items.len());
    let mut current_file_path = PathBuf::new();
    let mut current_file: Option<fs::File> = None;
    let mut current_modified_at: Option<std::time::SystemTime> = None;
    let mut current_len = 0_u64;
    let mut current_messages_by_id = std::collections::HashMap::<String, ChatMessage>::new();
    let mut current_cache_dirty = false;

    let flush_current_cache = |
        current_file_path: &PathBuf,
        current_modified_at: &Option<std::time::SystemTime>,
        current_len: u64,
        current_messages_by_id: &std::collections::HashMap<String, ChatMessage>,
        current_cache_dirty: bool,
    | {
        if !current_cache_dirty || current_file_path.as_os_str().is_empty() {
            return;
        }
        lock_message_store_block_file_cache().insert(
            current_file_path.clone(),
            CachedMessageStoreBlockFile {
                modified_at: *current_modified_at,
                len: current_len,
                messages_by_id: Arc::new(current_messages_by_id.clone()),
            },
        );
    };

    for item in items {
        let item_path = jsonl_snapshot_index_item_path(path, item.block_id)?;
        if current_file_path != item_path {
            flush_current_cache(
                &current_file_path,
                &current_modified_at,
                current_len,
                &current_messages_by_id,
                current_cache_dirty,
            );
            current_cache_dirty = false;
            current_file = None;
            current_messages_by_id.clear();

            let metadata = fs::metadata(&item_path).map_err(|err| {
                format!(
                    "读取会话块元数据失败，path={}，message_id={}，error={err}",
                    item_path.display(),
                    item.message_id
                )
            })?;
            current_modified_at = metadata.modified().ok();
            current_len = metadata.len();
            {
                let cache = lock_message_store_block_file_cache();
                if let Some(cached) = cache.get(&item_path) {
                    if cached.modified_at == current_modified_at && cached.len == current_len {
                        current_messages_by_id = (*cached.messages_by_id).clone();
                    }
                }
            }
            current_file_path = item_path;
        }

        if let Some(message) = current_messages_by_id.get(item.message_id.trim()) {
            messages.push(message.clone());
            continue;
        }

        if current_file.is_none() {
            current_file = Some(fs::File::open(&current_file_path).map_err(|err| {
                format!(
                    "打开 JSONL 消息文件失败，path={}，message_id={}，error={err}",
                    current_file_path.display(),
                    item.message_id
                )
            })?);
        }
        let Some(file) = current_file.as_mut() else {
            return Err(format!("打开 JSONL 消息文件失败，message_id={}", item.message_id));
        };
        std::io::Seek::seek(file, std::io::SeekFrom::Start(item.offset)).map_err(|err| {
            format!(
                "定位 JSONL 消息失败，path={}，message_id={}，offset={}，error={err}",
                current_file_path.display(),
                item.message_id,
                item.offset
            )
        })?;
        let mut buffer = vec![0_u8; item.byte_len as usize];
        std::io::Read::read_exact(file, &mut buffer).map_err(|err| {
            format!(
                "读取 JSONL 消息失败，path={}，message_id={}，offset={}，byte_len={}，error={err}",
                current_file_path.display(),
                item.message_id,
                item.offset,
                item.byte_len
            )
        })?;
        let raw = String::from_utf8(buffer)
            .map_err(|err| format!("JSONL 消息不是 UTF-8，message_id={}，error={err}", item.message_id))?;
        let line = raw.trim_end_matches(['\r', '\n']);
        let message = decode_jsonl_snapshot_message(line)
            .map_err(|err| format!("解析 JSONL 消息失败，message_id={}，error={err}", item.message_id))?;
        if message.id.trim() != item.message_id.trim() {
            return Err(format!(
                "JSONL 索引与消息不一致，path={}，expected_message_id={}，actual_message_id={}，offset={}，byte_len={}",
                current_file_path.display(),
                item.message_id,
                message.id,
                item.offset,
                item.byte_len
            ));
        }
        current_messages_by_id.insert(message.id.clone(), message.clone());
        current_cache_dirty = true;
        messages.push(message);
    }

    flush_current_cache(
        &current_file_path,
        &current_modified_at,
        current_len,
        &current_messages_by_id,
        current_cache_dirty,
    );
    Ok(messages)
}

fn build_message_store_block_summaries(
    path: &MessageStorePaths,
    index: &MessageStoreIndexFile,
) -> Result<Vec<MessageStoreBlockSummary>, String> {
    let block_ids = ordered_message_store_index_block_ids(index);
    let latest_block_id = block_ids.last().copied().unwrap_or(0);
    let mut summaries = Vec::<MessageStoreBlockSummary>::with_capacity(block_ids.len());
    for block_id in block_ids {
        let block_items = index
            .items
            .iter()
            .filter(|item| item.block_id.unwrap_or(0) == block_id)
            .cloned()
            .collect::<Vec<_>>();
        if block_items.is_empty() {
            continue;
        }
        let first_message = read_jsonl_snapshot_messages_by_index_items(
            &path.messages_file,
            &block_items[0..1],
        )?
        .into_iter()
        .next();
        let last_message = read_jsonl_snapshot_messages_by_index_items(
            &path.messages_file,
            &block_items[(block_items.len() - 1)..],
        )?
        .into_iter()
        .next();
        summaries.push(MessageStoreBlockSummary {
            block_id,
            message_count: block_items.len(),
            first_message_id: block_items
                .first()
                .map(|item| item.message_id.clone())
                .unwrap_or_default(),
            last_message_id: block_items
                .last()
                .map(|item| item.message_id.clone())
                .unwrap_or_default(),
            first_created_at: first_message.map(|message| message.created_at),
            last_created_at: last_message.map(|message| message.created_at),
            is_latest: block_id == latest_block_id,
        });
    }
    Ok(summaries)
}

fn jsonl_snapshot_index_item_path(base_messages_file: &PathBuf, block_id: Option<u32>) -> Result<PathBuf, String> {
    let Some(block_id) = block_id else {
        return Ok(base_messages_file.clone());
    };
    let Some(shard_dir) = base_messages_file.parent() else {
        return Err(format!(
            "会话块路径解析失败：messages 文件缺少父目录，path={}",
            base_messages_file.display()
        ));
    };
    Ok(shard_dir
        .join(MESSAGE_STORE_BLOCKS_DIR_NAME)
        .join(format!("{block_id:06}.jsonl")))
}

fn message_store_message_is_tool_review_report(message: &ChatMessage) -> bool {
    matches!(
        message_store_provider_meta_kind(message).as_deref(),
        Some("tool_review_report")
    )
}

fn message_store_message_has_image(message: &ChatMessage) -> bool {
    message.parts.iter().any(|part| {
        matches!(part, MessagePart::Image { mime, .. } if !mime.trim().eq_ignore_ascii_case("application/pdf"))
    })
}

fn message_store_message_has_pdf(message: &ChatMessage) -> bool {
    message.parts.iter().any(|part| {
        matches!(part, MessagePart::Image { mime, .. } if mime.trim().eq_ignore_ascii_case("application/pdf"))
    })
}

fn message_store_message_has_audio(message: &ChatMessage) -> bool {
    message
        .parts
        .iter()
        .any(|part| matches!(part, MessagePart::Audio { .. }))
}

fn read_messages_before_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    before_message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let before_idx = find_index_item_position(index, before_message_id)
        .ok_or_else(|| format!("Message not found: {}", before_message_id.trim()))?;
    let limit = normalized_message_limit(limit);
    let start = before_idx.saturating_sub(limit);
    Ok(MessageStoreLimitPage {
        messages: read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..before_idx])?,
        has_more: start > 0,
    })
}

fn read_messages_after_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    after_message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let after_idx = find_index_item_position(index, after_message_id)
        .ok_or_else(|| format!("Message not found: {}", after_message_id.trim()))?;
    let limit = normalized_message_limit(limit);
    let start = after_idx.saturating_add(1);
    let end = (start + limit).min(index.items.len());
    Ok(MessageStoreLimitPage {
        messages: read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..end])?,
        has_more: end < index.items.len(),
    })
}

fn read_messages_after_all_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    after_message_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let after_idx = find_index_item_position(index, after_message_id)
        .ok_or_else(|| format!("Message not found: {}", after_message_id.trim()))?;
    let start = after_idx.saturating_add(1);
    read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..])
}

fn compaction_boundary_index_items(index: &MessageStoreIndexFile) -> Vec<usize> {
    if !index.positions_by_message_id.is_empty() || index.items.is_empty() {
        return index.compaction_boundary_positions.clone();
    }
    index
        .items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if item.compaction_kind.is_some() {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

fn build_indexed_compaction_segment(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    start: usize,
    end: usize,
    previous_boundary_index: Option<usize>,
) -> Result<MessageStoreCompactionSegment, String> {
    let boundary_message_id = index
        .items
        .get(start)
        .filter(|_| is_index_compaction_boundary_position(index, start))
        .map(|item| item.message_id.trim().to_string());
    let previous_boundary_message_id = previous_boundary_index
        .and_then(|idx| index.items.get(idx))
        .map(|item| item.message_id.trim().to_string());
    Ok(MessageStoreCompactionSegment {
        messages: read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..end])?,
        boundary_message_id,
        previous_boundary_message_id,
        has_previous_segment: start > 0,
    })
}

fn is_index_compaction_boundary_position(index: &MessageStoreIndexFile, idx: usize) -> bool {
    index.compaction_boundary_positions.contains(&idx)
        || index
            .items
            .get(idx)
            .is_some_and(|item| item.compaction_kind.is_some())
}

fn read_current_compaction_segment_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
) -> Result<MessageStoreCompactionSegment, String> {
    if index.items.is_empty() {
        return Ok(MessageStoreCompactionSegment {
            messages: Vec::new(),
            boundary_message_id: None,
            previous_boundary_message_id: None,
            has_previous_segment: false,
        });
    }
    let boundaries = compaction_boundary_index_items(index);
    let start = boundaries.last().copied().unwrap_or(0);
    let previous_boundary_index = boundaries.iter().rev().nth(1).copied();
    build_indexed_compaction_segment(path, index, start, index.items.len(), previous_boundary_index)
}

fn read_compaction_segment_before_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    boundary_message_id: &str,
) -> Result<MessageStoreCompactionSegment, String> {
    let boundary_idx = find_index_item_position(index, boundary_message_id)
        .ok_or_else(|| format!("Compaction boundary not found: {}", boundary_message_id.trim()))?;
    let boundaries = compaction_boundary_index_items(index);
    let Some(boundary_pos) = boundaries.iter().position(|idx| *idx == boundary_idx) else {
        return Err(format!("Compaction boundary not indexed: {}", boundary_message_id.trim()));
    };
    let start = if boundary_pos == 0 {
        0
    } else {
        boundaries[boundary_pos - 1]
    };
    let previous_boundary_index = if boundary_pos >= 2 {
        Some(boundaries[boundary_pos - 2])
    } else {
        None
    };
    build_indexed_compaction_segment(path, index, start, boundary_idx, previous_boundary_index)
}

fn normalized_message_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

fn find_message_index(messages: &[ChatMessage], message_id: &str) -> Option<usize> {
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return None;
    }
    messages
        .iter()
        .position(|message| message.id.trim() == message_id)
}

fn read_recent_messages_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    limit: usize,
) -> Result<Vec<ChatMessage>, String> {
    let limit = normalized_message_limit(limit);
    let start = index.items.len().saturating_sub(limit);
    read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..])
}

fn read_recent_messages_page_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let limit = normalized_message_limit(limit);
    let start = index.items.len().saturating_sub(limit);
    let messages = read_jsonl_snapshot_messages_by_index_items(path, &index.items[start..])?;
    Ok(MessageStoreLimitPage {
        messages,
        has_more: start > 0,
    })
}

fn read_message_by_id_from_index(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
    message_id: &str,
) -> Result<ChatMessage, String> {
    let idx = find_index_item_position(index, message_id)
        .ok_or_else(|| format!("Message not found: {}", message_id.trim()))?;
    let mut messages = read_jsonl_snapshot_messages_by_index_items(path, &index.items[idx..=idx])?;
    messages
        .pop()
        .ok_or_else(|| format!("Message not found: {}", message_id.trim()))
}

fn read_recent_messages_from_slice(
    messages: &[ChatMessage],
    limit: usize,
) -> Result<Vec<ChatMessage>, String> {
    let limit = normalized_message_limit(limit);
    let start = messages.len().saturating_sub(limit);
    Ok(messages[start..].to_vec())
}

fn read_recent_messages_page_from_slice(
    messages: &[ChatMessage],
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let limit = normalized_message_limit(limit);
    let start = messages.len().saturating_sub(limit);
    Ok(MessageStoreLimitPage {
        messages: messages[start..].to_vec(),
        has_more: start > 0,
    })
}

fn read_message_by_id_from_slice(
    messages: &[ChatMessage],
    message_id: &str,
) -> Result<ChatMessage, String> {
    let message_id = message_id.trim();
    if message_id.is_empty() {
        return Err("messageId is required.".to_string());
    }
    messages
        .iter()
        .find(|item| item.id.trim() == message_id)
        .cloned()
        .ok_or_else(|| format!("Message not found: {message_id}"))
}

fn read_messages_before_from_slice(
    messages: &[ChatMessage],
    before_message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let before_idx = find_message_index(messages, before_message_id)
        .ok_or_else(|| format!("Message not found: {}", before_message_id.trim()))?;
    let limit = normalized_message_limit(limit);
    let start = before_idx.saturating_sub(limit);
    Ok(MessageStoreLimitPage {
        messages: messages[start..before_idx].to_vec(),
        has_more: start > 0,
    })
}

fn read_messages_after_from_slice(
    messages: &[ChatMessage],
    after_message_id: &str,
    limit: usize,
) -> Result<MessageStoreLimitPage, String> {
    let after_idx = find_message_index(messages, after_message_id)
        .ok_or_else(|| format!("Message not found: {}", after_message_id.trim()))?;
    let limit = normalized_message_limit(limit);
    let start = after_idx.saturating_add(1);
    let end = (start + limit).min(messages.len());
    Ok(MessageStoreLimitPage {
        messages: messages[start..end].to_vec(),
        has_more: end < messages.len(),
    })
}

fn read_messages_after_all_from_slice(
    messages: &[ChatMessage],
    after_message_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let after_idx = find_message_index(messages, after_message_id)
        .ok_or_else(|| format!("Message not found: {}", after_message_id.trim()))?;
    Ok(messages[(after_idx + 1)..].to_vec())
}

fn compaction_boundary_indexes(messages: &[ChatMessage]) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter_map(|(idx, message)| {
            if message_store_compaction_kind(message).is_some() {
                Some(idx)
            } else {
                None
            }
        })
        .collect()
}

fn build_compaction_segment(
    messages: &[ChatMessage],
    start: usize,
    end: usize,
    previous_boundary_index: Option<usize>,
) -> MessageStoreCompactionSegment {
    let boundary_message_id = messages
        .get(start)
        .filter(|message| message_store_compaction_kind(message).is_some())
        .map(|message| message.id.trim().to_string());
    let previous_boundary_message_id = previous_boundary_index
        .and_then(|idx| messages.get(idx))
        .map(|message| message.id.trim().to_string());
    MessageStoreCompactionSegment {
        messages: messages[start..end].to_vec(),
        boundary_message_id,
        previous_boundary_message_id,
        has_previous_segment: start > 0,
    }
}

fn read_current_compaction_segment_from_slice(messages: &[ChatMessage]) -> Result<MessageStoreCompactionSegment, String> {
    if messages.is_empty() {
        return Ok(MessageStoreCompactionSegment {
            messages: Vec::new(),
            boundary_message_id: None,
            previous_boundary_message_id: None,
            has_previous_segment: false,
        });
    }
    let boundaries = compaction_boundary_indexes(messages);
    let start = boundaries.last().copied().unwrap_or(0);
    let previous_boundary_index = boundaries
        .iter()
        .rev()
        .nth(1)
        .copied();
    Ok(build_compaction_segment(
        messages,
        start,
        messages.len(),
        previous_boundary_index,
    ))
}

fn read_compaction_segment_before_from_slice(
    messages: &[ChatMessage],
    boundary_message_id: &str,
) -> Result<MessageStoreCompactionSegment, String> {
    let boundary_idx = find_message_index(messages, boundary_message_id)
        .ok_or_else(|| format!("Compaction boundary not found: {}", boundary_message_id.trim()))?;
    if message_store_compaction_kind(&messages[boundary_idx]).is_none() {
        return Err(format!(
            "Message is not a compaction boundary: {}",
            boundary_message_id.trim()
        ));
    }
    let boundaries = compaction_boundary_indexes(messages);
    let Some(boundary_pos) = boundaries.iter().position(|idx| *idx == boundary_idx) else {
        return Err(format!("Compaction boundary not indexed: {}", boundary_message_id.trim()));
    };
    let start = if boundary_pos == 0 {
        0
    } else {
        boundaries[boundary_pos - 1]
    };
    let previous_boundary_index = if boundary_pos >= 2 {
        Some(boundaries[boundary_pos - 2])
    } else {
        None
    };
    Ok(build_compaction_segment(
        messages,
        start,
        boundary_idx,
        previous_boundary_index,
    ))
}

#[cfg(test)]
mod message_store_reader_tests {
    use super::*;

    fn test_message(id: &str, role: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-04-24T00:00:0{}Z", id.len()),
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

    fn test_compaction_message(id: &str, kind: &str) -> ChatMessage {
        let mut message = test_message(id, "assistant");
        message.provider_meta = Some(serde_json::json!({
            "messageMeta": {
                "kind": kind
            }
        }));
        message
    }

    fn test_tool_review_report_message(id: &str) -> ChatMessage {
        let mut message = test_message(id, "assistant");
        message.provider_meta = Some(serde_json::json!({
            "messageKind": "tool_review_report",
            "messageMeta": {
                "kind": "tool_review_report"
            }
        }));
        message
    }

    fn write_test_messages(messages: &[ChatMessage]) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-reader-{}",
            Uuid::new_v4()
        ));
        let messages_file = root.join("messages.jsonl");
        let content = encode_jsonl_snapshot_messages(messages).expect("encode messages");
        write_jsonl_snapshot_atomic(&messages_file, &content).expect("write messages");
        (root, messages_file)
    }

    fn write_test_messages_with_index(messages: &[ChatMessage]) -> (PathBuf, PathBuf, PathBuf) {
        let (root, messages_file) = write_test_messages(messages);
        let index_file = root.join("messages.idx.json");
        let index = rebuild_jsonl_snapshot_index_from_file(&messages_file).expect("rebuild index");
        write_message_store_index_atomic(&index_file, &index).expect("write index");
        (root, messages_file, index_file)
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-reader".to_string(),
            title: "reader".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages,
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    #[test]
    fn message_store_jsonl_reader_should_match_before_after_limit_semantics() {
        let messages = vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
        ];
        let (root, messages_file) = write_test_messages(&messages);
        let store = JsonlSnapshotMessageStore::new(messages_file);

        let before = store.read_messages_before("m4", 2).expect("before page");
        let after = store.read_messages_after("m1", 2).expect("after page");

        assert_eq!(
            before.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m2", "m3"]
        );
        assert!(before.has_more);
        assert_eq!(
            after.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m2", "m3"]
        );
        assert!(after.has_more);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_reader_should_match_compaction_segment_semantics() {
        let messages = vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
            test_compaction_message("c2", "summary_context_seed"),
            test_message("m3", "assistant"),
        ];
        let (root, messages_file) = write_test_messages(&messages);
        let store = JsonlSnapshotMessageStore::new(messages_file);

        let current = store.read_current_compaction_segment().expect("current segment");
        let previous = store
            .read_compaction_segment_before("c2")
            .expect("previous segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c2", "m3"]
        );
        assert_eq!(current.boundary_message_id.as_deref(), Some("c2"));
        assert_eq!(current.previous_boundary_message_id.as_deref(), Some("c1"));
        assert_eq!(
            previous.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "m2"]
        );
        assert_eq!(previous.boundary_message_id.as_deref(), Some("c1"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_only_decode_requested_page() {
        let messages = vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
            test_message("m5", "user"),
        ];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let before = store.read_messages_before("m5", 2).expect("indexed before page");
        let after = store.read_messages_after("m2", 2).expect("indexed after page");

        assert_eq!(
            before.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m3", "m4"]
        );
        assert!(before.has_more);
        assert_eq!(
            after.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m3", "m4"]
        );
        assert!(after.has_more);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_read_recent_and_single_message() {
        let messages = vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
        ];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let recent = store.read_recent_messages(2).expect("recent messages");
        let message = store.read_message_by_id("m2").expect("message by id");
        let after_all = store.read_messages_after_all("m2").expect("after all");

        assert_eq!(
            recent.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m3", "m4"]
        );
        assert_eq!(message.id, "m2");
        assert_eq!(
            after_all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m3", "m4"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_read_recent_page_with_has_more() {
        let messages = vec![
            test_message("m1", "user"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_message("m4", "assistant"),
        ];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let page = store
            .read_recent_messages_page(2)
            .expect("recent messages page");

        assert_eq!(
            page.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["m3", "m4"]
        );
        assert!(page.has_more);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_page_compaction_segments() {
        let messages = vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
            test_compaction_message("c2", "summary_context_seed"),
            test_message("m3", "assistant"),
        ];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let current = store.read_current_compaction_segment().expect("indexed current segment");
        let previous = store
            .read_compaction_segment_before("c2")
            .expect("indexed previous segment");

        assert_eq!(
            current.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c2", "m3"]
        );
        assert_eq!(current.previous_boundary_message_id.as_deref(), Some("c1"));
        assert_eq!(
            previous.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["c1", "m2"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_reject_unsupported_index_version() {
        let messages = vec![test_message("m1", "user"), test_message("m2", "assistant")];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let mut index = (*read_message_store_index_file(&index_file).expect("read index")).clone();
        index.version = MESSAGE_STORE_MANIFEST_VERSION + 1;
        write_message_store_index_atomic(&index_file, &index).expect("write future index");
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let err = store
            .read_recent_messages(1)
            .expect_err("future index version should fail");

        assert!(err.contains("消息索引版本不支持"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_reject_stale_message_id() {
        let messages = vec![test_message("m1", "user"), test_message("m2", "assistant")];
        let (root, messages_file, index_file) = write_test_messages_with_index(&messages);
        let mut index = (*read_message_store_index_file(&index_file).expect("read index")).clone();
        index.items[1].message_id = "wrong-m2".to_string();
        write_message_store_index_atomic(&index_file, &index).expect("write stale index");
        let store = JsonlSnapshotMessageStore::with_index(messages_file, index_file);

        let err = store
            .read_message_by_id("wrong-m2")
            .expect_err("stale message id should fail");

        assert!(err.contains("JSONL 索引与消息不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_jsonl_indexed_reader_should_reject_invalid_index_shape() {
        let messages = vec![test_message("m1", "user"), test_message("m2", "assistant")];
        let (root, _messages_file, index_file) = write_test_messages_with_index(&messages);
        let mut index = (*read_message_store_index_file(&index_file).expect("read index")).clone();
        index.items[1].message_id = index.items[0].message_id.clone();
        write_message_store_index_atomic(&index_file, &index).expect("write duplicate index");

        let err = read_message_store_index_file(&index_file)
            .expect_err("duplicate index message id should fail");

        assert!(err.contains("消息索引包含重复消息 ID"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_backend_should_not_read_stale_jsonl_without_ready_manifest() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-backend-stale-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("old1", "user")]);
        let stale_messages = vec![test_message("jsonl1", "assistant")];
        let stale_content = encode_jsonl_snapshot_messages(&stale_messages).expect("encode stale");
        write_jsonl_snapshot_atomic(&paths.messages_file, &stale_content).expect("write stale");
        let building = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        write_message_store_manifest_atomic(&paths.manifest_file, &building)
            .expect("write manifest");

        let store =
            message_store_backend_for_conversation(&paths, &conversation).expect("select store");
        let messages = store.read_all_messages().expect("read messages");

        assert_eq!(
            messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["old1"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_backend_should_read_jsonl_only_when_manifest_ready() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-backend-ready-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("old1", "user")]);
        let jsonl_conversation = test_conversation(vec![
            test_message("jsonl1", "assistant"),
            test_message("jsonl2", "user"),
        ]);
        run_jsonl_snapshot_migration(&paths, &jsonl_conversation, false).expect("run migration");

        let store =
            message_store_backend_for_conversation(&paths, &conversation).expect("select store");
        let messages = store.read_all_messages().expect("read messages");

        assert_eq!(
            messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["jsonl1", "jsonl2"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_chat_snapshot_should_seek_latest_messages_from_index() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-chat-snapshot-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![
            test_message("u1", "user"),
            test_message("a1", "assistant"),
            test_tool_review_report_message("review1"),
            test_message("u2", "user"),
        ]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");

        let snapshot = read_ready_message_store_chat_snapshot(&paths)
            .expect("read chat snapshot")
            .expect("ready snapshot");

        assert_eq!(snapshot.latest_user.as_ref().map(|m| m.id.as_str()), Some("u2"));
        assert_eq!(
            snapshot.latest_assistant.as_ref().map(|m| m.id.as_str()),
            Some("a1")
        );
        assert_eq!(snapshot.active_message_count, 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_branch_selection_should_keep_branch_semantics_from_index() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-branch-selection-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![
            test_message("m1", "user"),
            test_compaction_message("c1", "context_compaction"),
            test_message("m2", "assistant"),
            test_message("m3", "user"),
            test_compaction_message("c2", "summary_context_seed"),
            test_message("m4", "assistant"),
        ]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let selected_ids = vec!["m3".to_string(), "m2".to_string()];

        let selection = read_ready_message_store_branch_selection(&paths, &selected_ids)
            .expect("read branch selection")
            .expect("ready branch selection");

        assert_eq!(
            selection
                .selected_messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["m2", "m3"]
        );
        assert_eq!(selection.first_selected_ordinal, 2);
        assert_eq!(
            selection
                .latest_compaction_message
                .as_ref()
                .map(|message| message.id.as_str()),
            Some("c2")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_directory_conversation_should_assemble_meta_and_messages() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-directory-read-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![
            test_message("jsonl1", "assistant"),
            test_message("jsonl2", "user"),
        ]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");

        let loaded = read_message_store_directory_conversation(&paths).expect("read directory");

        assert_eq!(loaded.id, conversation.id);
        assert_eq!(loaded.title, conversation.title);
        assert_eq!(
            loaded
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["jsonl1", "jsonl2"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_meta_should_not_decode_messages_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-meta-ready-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![
            test_message("jsonl1", "assistant"),
            test_message("jsonl2", "user"),
        ]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        fs::write(&paths.messages_file, "{broken jsonl").expect("break messages jsonl");

        let meta = read_ready_message_store_meta(&paths)
            .expect("read ready meta")
            .expect("ready meta should exist");

        assert_eq!(meta.id, conversation.id);
        assert_eq!(meta.title, conversation.title);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_status_should_not_decode_messages_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-status-ready-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![
            test_message("jsonl1", "assistant"),
            test_message("jsonl2", "user"),
        ]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let block_path = paths.blocks_dir.join("000000.jsonl");
        let original_content = fs::read_to_string(&block_path).expect("read messages");
        fs::write(&block_path, "x".repeat(original_content.len()))
            .expect("break messages jsonl without changing size");

        let status = read_ready_message_store_status(&paths)
            .expect("read ready status")
            .expect("ready status should exist");

        assert!(status.ready_jsonl);
        assert_eq!(status.source_message_count, 2);
        assert_eq!(status.last_message_id, "jsonl2");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_status_should_reject_mismatched_jsonl_size() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-status-size-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("jsonl1", "assistant")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        fs::write(&paths.messages_file, "").expect("truncate messages");

        let err = read_ready_message_store_status(&paths)
            .expect_err("mismatched message file size should fail status");

        assert!(err.contains("消息文件大小不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_status_should_not_mark_event_log_ready_as_supported() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-event-log-status-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("jsonl1", "assistant")]);
        let mut manifest = MessageStoreManifest::jsonl_snapshot_building(&conversation);
        manifest.message_store_kind = MessageStoreKind::JsonlEventLog;
        manifest.migration_state = MessageStoreMigrationState::Ready;
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write event log manifest");

        let ready_status = read_ready_message_store_status(&paths).expect("read ready status");
        let manifest_status = read_message_store_manifest_status(&paths)
            .expect("read manifest status")
            .expect("manifest status should exist");

        assert!(ready_status.is_none());
        assert_eq!(manifest_status.message_store_kind, "jsonlEventLog");
        assert_eq!(manifest_status.migration_state, "ready");
        assert!(!manifest_status.ready_jsonl);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_meta_should_reject_mismatched_meta_id() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-meta-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("jsonl1", "assistant")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let mut wrong_conversation = conversation.clone();
        wrong_conversation.id = "wrong-conversation".to_string();
        let wrong_meta = ConversationShardMeta::from_conversation(&wrong_conversation);
        write_conversation_shard_meta_atomic(&paths.meta_file, &wrong_meta)
            .expect("write wrong meta");

        let meta_err =
            read_ready_message_store_meta(&paths).expect_err("mismatched meta should fail");
        let status_err =
            read_ready_message_store_status(&paths).expect_err("mismatched status should fail");
        let directory_err = read_message_store_directory_conversation(&paths)
            .expect_err("mismatched directory should fail");

        assert!(meta_err.contains("元数据 ID 不一致"));
        assert!(status_err.contains("元数据 ID 不一致"));
        assert!(directory_err.contains("元数据 ID 不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_directory_conversation_should_reject_stale_manifest() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-directory-stale-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let mut manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");
        manifest.last_message_id = "wrong-last-id".to_string();
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write stale manifest");

        let err = read_message_store_directory_conversation(&paths)
            .expect_err("stale manifest should fail");

        assert!(err.contains("最后消息不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_reader_should_reject_mismatched_jsonl_size() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-size-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        fs::write(&paths.messages_file, "").expect("truncate messages");

        let err = read_ready_message_store_recent_messages(&paths, 1)
            .expect_err("mismatched message file size should fail");

        assert!(err.contains("消息文件大小不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_backend_should_reject_mismatched_jsonl_size() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-backend-size-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        fs::write(&paths.messages_file, "").expect("truncate messages");

        let err = match message_store_backend_for_conversation(&paths, &conversation) {
            Ok(_) => panic!("mismatched message file size should fail"),
            Err(err) => err,
        };

        assert!(err.contains("消息文件大小不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_reader_should_reject_index_manifest_count_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-index-count-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let mut manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");
        manifest.source_message_count = 2;
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write stale manifest");

        let err = read_ready_message_store_recent_messages(&paths, 1)
            .expect_err("mismatched index count should fail");

        assert!(err.contains("索引数量不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_ready_reader_should_reject_index_manifest_last_id_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-index-last-id-mismatch-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");
        let mut manifest = read_message_store_manifest(&paths.manifest_file)
            .expect("read manifest")
            .expect("manifest exists");
        manifest.last_message_id = "wrong-last-id".to_string();
        write_message_store_manifest_atomic(&paths.manifest_file, &manifest)
            .expect("write stale manifest");

        let err = read_ready_message_store_recent_messages(&paths, 1)
            .expect_err("mismatched index last id should fail");

        assert!(err.contains("索引最后消息不一致"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_delete_should_remove_legacy_file_and_directory_shard() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-delete-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        write_conversation_shard(&data_path, &conversation).expect("write legacy");
        run_jsonl_snapshot_migration(&paths, &conversation, false).expect("run migration");

        let changed = delete_message_store_shard_artifacts(&paths).expect("delete artifacts");

        assert!(changed);
        assert!(!paths.legacy_conversation_file.exists());
        assert!(!paths.shard_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn message_store_delete_should_validate_directory_before_removing_legacy_file() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-message-store-delete-guard-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("app_data.json");
        let paths = message_store_paths(&data_path, "conversation-reader").expect("paths");
        let conversation = test_conversation(vec![test_message("m1", "user")]);
        write_conversation_shard(&data_path, &conversation).expect("write legacy");
        let outside_dir = root.join("outside-shard");
        fs::create_dir_all(&outside_dir).expect("create outside dir");
        let bad_paths = MessageStorePaths {
            data_path: paths.data_path.clone(),
            conversation_id: paths.conversation_id.clone(),
            legacy_conversation_file: paths.legacy_conversation_file.clone(),
            shard_dir: outside_dir.clone(),
            manifest_file: outside_dir.join("manifest.json"),
            meta_file: outside_dir.join("meta.json"),
            messages_file: outside_dir.join("messages.jsonl"),
            active_plans_file: outside_dir.join("active_plans.jsonl"),
            index_file: outside_dir.join("messages.idx.json"),
            blocks_dir: outside_dir.join("blocks"),
            blobs_dir: outside_dir.join("blobs"),
        };

        let err = delete_message_store_shard_artifacts(&bad_paths)
            .expect_err("unsafe shard dir should fail before deletion");

        assert!(err.contains("分片目录不在 conversations 目录内"));
        assert!(!paths.legacy_conversation_file.exists());
        assert!(outside_dir.exists());
        let _ = fs::remove_dir_all(root);
    }
}
