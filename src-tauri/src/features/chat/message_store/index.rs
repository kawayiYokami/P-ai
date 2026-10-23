#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct MessageStoreIndexItem {
    message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    block_id: Option<u32>,
    offset: u64,
    byte_len: u64,
    #[serde(default, skip_serializing)]
    compaction_kind: Option<String>,
    #[serde(default, skip_serializing)]
    role: String,
    #[serde(default, skip_serializing)]
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MessageStoreIndexFile {
    version: u32,
    #[serde(default)]
    items: Vec<MessageStoreIndexItem>,
    #[serde(skip)]
    positions_by_message_id: std::collections::HashMap<String, usize>,
    #[serde(skip)]
    compaction_boundary_positions: Vec<usize>,
}

impl MessageStoreIndexFile {
    fn new(version: u32, items: Vec<MessageStoreIndexItem>) -> Self {
        Self {
            version,
            items,
            positions_by_message_id: std::collections::HashMap::new(),
            compaction_boundary_positions: Vec::new(),
        }
        .with_position_lookup()
    }

    fn with_position_lookup(mut self) -> Self {
        self.rebuild_position_lookup();
        self
    }

    fn persistent_view(&self) -> Self {
        let mut next = self.clone();
        for item in &mut next.items {
            item.compaction_kind = None;
            item.role.clear();
            item.created_at.clear();
        }
        next.positions_by_message_id.clear();
        next.compaction_boundary_positions.clear();
        next.with_position_lookup()
    }

    fn rebuild_position_lookup(&mut self) {
        self.positions_by_message_id.clear();
        self.compaction_boundary_positions.clear();
        let has_block_ids = self
            .items
            .iter()
            .any(|item| item.block_id.is_some());
        let mut previous_block_file = String::new();
        for (idx, item) in self.items.iter().enumerate() {
            let message_id = item.message_id.trim();
            if !message_id.is_empty() {
                self.positions_by_message_id
                    .insert(message_id.to_string(), idx);
            }
            if has_block_ids {
                let block_file = message_store_index_item_block_key(item);
                if idx > 0 && block_file != previous_block_file {
                    self.compaction_boundary_positions.push(idx);
                }
                previous_block_file = block_file;
                continue;
            }
            if item.compaction_kind.is_some() {
                self.compaction_boundary_positions.push(idx);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct CachedMessageStoreIndexFile {
    modified_at: Option<std::time::SystemTime>,
    len: u64,
    index: Arc<MessageStoreIndexFile>,
}

static MESSAGE_STORE_INDEX_CACHE: OnceLock<
    Mutex<std::collections::HashMap<PathBuf, CachedMessageStoreIndexFile>>,
> = OnceLock::new();

fn message_store_index_cache(
) -> &'static Mutex<std::collections::HashMap<PathBuf, CachedMessageStoreIndexFile>> {
    MESSAGE_STORE_INDEX_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn lock_message_store_index_cache(
) -> std::sync::MutexGuard<
    'static,
    std::collections::HashMap<PathBuf, CachedMessageStoreIndexFile>,
> {
    message_store_index_cache().lock().unwrap_or_else(|poison| {
        runtime_log_info(format!(
            "[消息存储] 消息索引缓存锁已污染，继续使用内部状态，error={:?}",
            poison
        ));
        poison.into_inner()
    })
}

fn write_message_store_index_atomic(path: &PathBuf, index: &MessageStoreIndexFile) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(index)
        .map_err(|err| format!("序列化消息索引失败: {err}"))?;
    write_message_store_text_atomic(path, "json.tmp", &raw, "消息索引")?;
    if validate_message_store_index_file(path, index).is_ok() {
        remember_message_store_index_cache(path, index);
    } else {
        forget_message_store_index_cache(path);
    }
    Ok(())
}

fn read_message_store_index_file(path: &PathBuf) -> Result<Arc<MessageStoreIndexFile>, String> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("读取消息索引元数据失败，path={}，error={err}", path.display()))?;
    let modified_at = metadata.modified().ok();
    let len = metadata.len();
    {
        let cache = lock_message_store_index_cache();
        if let Some(cached) = cache.get(path) {
            if cached.modified_at == modified_at && cached.len == len {
                return Ok(Arc::clone(&cached.index));
            }
        }
    }
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取消息索引失败，path={}，error={err}", path.display()))?;
    let index = serde_json::from_str::<MessageStoreIndexFile>(&raw)
        .map_err(|err| format!("解析消息索引失败，path={}，error={err}", path.display()))?;
    validate_message_store_index_file(path, &index)?;
    let index = Arc::new(index.with_position_lookup());
    lock_message_store_index_cache().insert(
        path.clone(),
        CachedMessageStoreIndexFile {
            modified_at,
            len,
            index: Arc::clone(&index),
        },
    );
    Ok(index)
}

fn remember_message_store_index_cache(path: &PathBuf, index: &MessageStoreIndexFile) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    lock_message_store_index_cache().insert(
        path.clone(),
        CachedMessageStoreIndexFile {
            modified_at: metadata.modified().ok(),
            len: metadata.len(),
                index: Arc::new(index.clone().with_position_lookup()),
        },
    );
}

fn forget_message_store_index_cache(path: &PathBuf) {
    lock_message_store_index_cache().remove(path);
}

pub(super) fn message_store_index_cache_stats() -> (usize, usize, usize) {
    let cache = lock_message_store_index_cache();
    let entry_count = cache.len();
    let item_count = cache.values().map(|item| item.index.items.len()).sum::<usize>();
    let estimated_json_bytes = cache
        .values()
        .map(|item| serde_json::to_vec(&*item.index).map(|raw| raw.len()).unwrap_or(0))
        .sum::<usize>();
    (entry_count, item_count, estimated_json_bytes)
}

fn validate_message_store_index_file(
    path: &PathBuf,
    index: &MessageStoreIndexFile,
) -> Result<(), String> {
    if index.version != MESSAGE_STORE_MANIFEST_VERSION {
        return Err(format!(
            "消息索引版本不支持，path={}，expected={}，actual={}",
            path.display(),
            MESSAGE_STORE_MANIFEST_VERSION,
            index.version
        ));
    }
    let mut seen_ids = std::collections::HashSet::<String>::new();
    let mut previous_end_by_block = std::collections::HashMap::<String, u64>::new();
    for item in &index.items {
        let message_id = item.message_id.trim();
        if message_id.is_empty() {
            return Err(format!(
                "消息索引包含空消息 ID，path={}，offset={}",
                path.display(),
                item.offset
            ));
        }
        if !seen_ids.insert(message_id.to_string()) {
            return Err(format!(
                "消息索引包含重复消息 ID，path={}，message_id={}",
                path.display(),
                message_id
            ));
        }
        if item.byte_len == 0 {
            return Err(format!(
                "消息索引 byte_len 不能为 0，path={}，message_id={}",
                path.display(),
                message_id
            ));
        }
        let block_key = message_store_index_item_block_key(item);
        let previous_end = *previous_end_by_block.get(&block_key).unwrap_or(&0);
        if item.offset < previous_end {
            return Err(format!(
                "消息索引 offset 重叠，path={}，block={}，message_id={}，offset={}，previous_end={}",
                path.display(),
                block_key,
                message_id,
                item.offset,
                previous_end
            ));
        }
        let next_end = item.offset.checked_add(item.byte_len).ok_or_else(|| {
            format!(
                "消息索引 offset 溢出，path={}，message_id={}，offset={}，byte_len={}",
                path.display(),
                message_id,
                item.offset,
                item.byte_len
            )
        })?;
        previous_end_by_block.insert(block_key, next_end);
    }
    Ok(())
}

fn message_store_provider_meta_kind(message: &ChatMessage) -> Option<String> {
    message
        .provider_meta
        .as_ref()?
        .get("message_meta")
        .or_else(|| message.provider_meta.as_ref()?.get("messageMeta"))?
        .get("kind")?
        .as_str()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn message_store_compaction_kind(message: &ChatMessage) -> Option<String> {
    match message_store_provider_meta_kind(message).as_deref() {
        Some("context_compaction") => Some("context_compaction".to_string()),
        Some("summary_context_seed") => Some("summary_context_seed".to_string()),
        _ => None,
    }
}

fn message_store_index_item_for_message(
    message: &ChatMessage,
    offset: u64,
    byte_len: u64,
) -> MessageStoreIndexItem {
    message_store_index_item_for_message_in_block(message, None, offset, byte_len)
}

fn message_store_index_item_for_message_in_block(
    message: &ChatMessage,
    block_id: Option<u32>,
    offset: u64,
    byte_len: u64,
) -> MessageStoreIndexItem {
    MessageStoreIndexItem {
        message_id: message.id.trim().to_string(),
        block_id,
        offset,
        byte_len,
        compaction_kind: message_store_compaction_kind(message),
        role: message.role.trim().to_string(),
        created_at: message.created_at.trim().to_string(),
    }
}

fn message_store_index_item_block_key(item: &MessageStoreIndexItem) -> String {
    item.block_id
        .map(|block_id| block_id.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[test]
fn message_store_index_persistent_view_should_ignore_runtime_only_locator_fields() {
    let persisted = MessageStoreIndexFile::new(
        MESSAGE_STORE_MANIFEST_VERSION,
        vec![MessageStoreIndexItem {
            message_id: "message-1".to_string(),
            block_id: Some(0),
            offset: 0,
            byte_len: 42,
            compaction_kind: Some("context_compaction".to_string()),
            role: "assistant".to_string(),
            created_at: "2026-07-10T00:00:00Z".to_string(),
        }],
    );
    let restored: MessageStoreIndexFile = serde_json::from_str(
        &serde_json::to_string(&persisted).expect("serialize index"),
    )
    .expect("deserialize index");

    assert_eq!(persisted.persistent_view().items, restored.persistent_view().items);
}
