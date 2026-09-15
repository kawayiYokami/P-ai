const STORAGE_CLEANUP_LEGACY_CONVERSATIONS: &str = "legacyConversations";
const STORAGE_CLEANUP_LEGACY_DELEGATE_CONVERSATIONS: &str = "legacyDelegateConversations";
const STORAGE_CLEANUP_ABNORMAL_CONVERSATIONS: &str = "abnormalConversations";
const STORAGE_CLEANUP_IMAGE_TEXT_CACHE: &str = "imageTextCache";

#[derive(Debug, Clone, Default)]
struct StorageSizeStats {
    bytes: u64,
    file_count: usize,
    directory_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageUsageOverview {
    root_path: String,
    total_bytes: u64,
    reclaimable_bytes: u64,
    items: Vec<StorageUsageItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewStatus {
    compute_state: String,
    freshness: String,
    generated_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewSnapshot<T> {
    status: OverviewStatus,
    data: Option<T>,
}

#[derive(Debug, Clone)]
struct OverviewCacheEntry<T> {
    computed_at: std::time::Instant,
    generated_at: String,
    data: T,
}

#[derive(Debug, Clone)]
struct OverviewRuntime<T> {
    cache: Option<OverviewCacheEntry<T>>,
    running: bool,
    last_error: Option<String>,
}

impl<T> Default for OverviewRuntime<T> {
    fn default() -> Self {
        Self {
            cache: None,
            running: false,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageOverviewTotals {
    conversation_count: usize,
    archived_conversation_count: usize,
    active_conversation_count: usize,
    delegate_conversation_count: usize,
    with_usage_conversation_count: usize,
    weighted_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageAggregateItem {
    key: String,
    label: String,
    conversation_count: usize,
    weighted_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageProviderModelAggregateItem {
    key: String,
    provider_key: String,
    provider_label: String,
    model_name: String,
    conversation_count: usize,
    weighted_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageConversationItem {
    conversation_id: String,
    title: String,
    summary_title: Option<String>,
    updated_at: String,
    archived_at: Option<String>,
    agent_id: String,
    agent_name: String,
    avatar_path: Option<String>,
    avatar_updated_at: Option<String>,
    api_config_id: String,
    api_config_name: String,
    model_name: String,
    conversation_kind: String,
    is_delegate: bool,
    is_system_notification_conversation: bool,
    message_count: usize,
    weighted_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageOverview {
    generated_at: String,
    totals: UsageOverviewTotals,
    conversations: Vec<UsageConversationItem>,
    by_provider_model: Vec<UsageProviderModelAggregateItem>,
    by_model: Vec<UsageAggregateItem>,
    by_api_config: Vec<UsageAggregateItem>,
    by_agent: Vec<UsageAggregateItem>,
    by_kind: Vec<UsageAggregateItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageUsageItem {
    id: String,
    target_path: String,
    bytes: u64,
    file_count: usize,
    directory_count: usize,
    cleanable_bytes: u64,
    cleanable_file_count: usize,
    cleanup_kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CleanupStorageLegacyItemsInput {
    cleanup_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenStorageUsageItemDirectoryInput {
    item_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageCleanupResult {
    deleted_file_count: usize,
    skipped_file_count: usize,
    freed_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageLegacyConversationScope {
    Normal,
    Delegate,
}

#[derive(Debug, Clone, Default)]
struct StorageLegacyCleanupScan {
    total_file_count: usize,
    cleanable_paths: Vec<PathBuf>,
    cleanable_bytes: u64,
}

#[derive(Debug, Clone)]
struct StorageAbnormalConversationCandidate {
    conversation_id: String,
    shard_dir: PathBuf,
    stats: StorageSizeStats,
}

#[derive(Debug, Clone, Default)]
struct StorageAbnormalConversationScan {
    candidates: Vec<StorageAbnormalConversationCandidate>,
    stats: StorageSizeStats,
}

fn storage_usage_item(
    id: &str,
    target_path: PathBuf,
    stats: StorageSizeStats,
    cleanup_kind: Option<&str>,
    cleanable_bytes: u64,
    cleanable_file_count: usize,
) -> StorageUsageItem {
    StorageUsageItem {
        id: id.to_string(),
        target_path: target_path.to_string_lossy().to_string(),
        bytes: stats.bytes,
        file_count: stats.file_count,
        directory_count: stats.directory_count,
        cleanable_bytes,
        cleanable_file_count,
        cleanup_kind: cleanup_kind.map(str::to_string),
    }
}

fn storage_add_path_stats(path: &PathBuf, stats: &mut StorageSizeStats) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(path).map_err(|err| {
        format!("读取存储路径元数据失败，path={}，error={err}", path.display())
    })?;
    if metadata.is_file() {
        stats.bytes = stats.bytes.saturating_add(metadata.len());
        stats.file_count += 1;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    stats.directory_count += 1;
    let entries = fs::read_dir(path).map_err(|err| {
        format!("读取存储目录失败，path={}，error={err}", path.display())
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!("读取存储目录项失败，path={}，error={err}", path.display())
        })?;
        storage_add_path_stats(&entry.path(), stats)?;
    }
    Ok(())
}

fn storage_stats_for_paths(paths: Vec<PathBuf>) -> Result<StorageSizeStats, String> {
    let mut stats = StorageSizeStats::default();
    for path in paths {
        storage_add_path_stats(&path, &mut stats)?;
    }
    Ok(stats)
}

fn storage_stats_for_directory_entries(
    dir: &PathBuf,
    filter: impl Fn(&PathBuf, &fs::FileType) -> bool,
) -> Result<StorageSizeStats, String> {
    let mut stats = StorageSizeStats::default();
    if !dir.exists() {
        return Ok(stats);
    }
    let entries = fs::read_dir(dir).map_err(|err| {
        format!("读取存储目录失败，path={}，error={err}", dir.display())
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!("读取存储目录项失败，path={}，error={err}", dir.display())
        })?;
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "读取存储目录项类型失败，path={}，error={err}",
                entry.path().display()
            )
        })?;
        let path = entry.path();
        if filter(&path, &file_type) {
            storage_add_path_stats(&path, &mut stats)?;
        }
    }
    Ok(stats)
}

fn storage_path_file_name_is(path: &PathBuf, name: &str) -> bool {
    path.file_name().and_then(|value| value.to_str()) == Some(name)
}

fn storage_path_extension_is(path: &PathBuf, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn storage_conversation_dir(data_path: &PathBuf, scope: StorageLegacyConversationScope) -> PathBuf {
    match scope {
        StorageLegacyConversationScope::Normal => app_layout_chat_conversations_dir(data_path),
        StorageLegacyConversationScope::Delegate => delegate_conversation_store_dir(data_path),
    }
}

fn storage_legacy_file_ready_to_cleanup(
    data_path: &PathBuf,
    conversation_id: &str,
    scope: StorageLegacyConversationScope,
) -> Result<bool, String> {
    match scope {
        StorageLegacyConversationScope::Normal => {
            let paths = message_store::message_store_paths(data_path, conversation_id)?;
            message_store::chat_store_read_status(&paths).map(|status| status.is_some())
        }
        StorageLegacyConversationScope::Delegate => {
            let paths = delegate_conversation_message_store_paths(data_path, conversation_id)?;
            delegate_conversation_store_read_meta(&paths, conversation_id)
                .map(|meta| meta.is_some())
        }
    }
}

fn storage_scan_legacy_cleanup_candidates(
    data_path: &PathBuf,
    scope: StorageLegacyConversationScope,
) -> Result<StorageLegacyCleanupScan, String> {
    let dir = storage_conversation_dir(data_path, scope);
    let mut scan = StorageLegacyCleanupScan::default();
    if !dir.exists() {
        return Ok(scan);
    }
    let entries = fs::read_dir(&dir).map_err(|err| {
        format!("读取旧会话目录失败，path={}，error={err}", dir.display())
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!("读取旧会话目录项失败，path={}，error={err}", dir.display())
        })?;
        let path = entry.path();
        if !path.is_file() || !storage_path_extension_is(&path, "json") {
            continue;
        }
        scan.total_file_count += 1;
        let conversation_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if conversation_id.is_empty() {
            continue;
        }
        match storage_legacy_file_ready_to_cleanup(data_path, &conversation_id, scope) {
            Ok(true) => {
                let bytes = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
                scan.cleanable_bytes = scan.cleanable_bytes.saturating_add(bytes);
                scan.cleanable_paths.push(path);
            }
            Ok(false) => {}
            Err(err) => {
                runtime_log_warn(format!(
                    "[存储] 跳过，任务=检查旧会话清理候选，conversation_id={}，reason={}",
                    conversation_id,
                    err
                ));
            }
        }
    }
    Ok(scan)
}

fn storage_legacy_conversation_stats(
    data_path: &PathBuf,
    scope: StorageLegacyConversationScope,
) -> Result<(StorageSizeStats, StorageLegacyCleanupScan), String> {
    let dir = storage_conversation_dir(data_path, scope);
    let stats = storage_stats_for_directory_entries(&dir, |path, file_type| {
        file_type.is_file() && storage_path_extension_is(path, "json")
    })?;
    let scan = storage_scan_legacy_cleanup_candidates(data_path, scope)?;
    Ok((stats, scan))
}

fn storage_current_conversation_store_stats(
    data_path: &PathBuf,
    scope: StorageLegacyConversationScope,
) -> Result<StorageSizeStats, String> {
    storage_current_conversation_store_stats_excluding_ids(
        data_path,
        scope,
        &std::collections::HashSet::new(),
    )
}

fn storage_current_conversation_store_stats_excluding_ids(
    data_path: &PathBuf,
    scope: StorageLegacyConversationScope,
    excluded_conversation_ids: &std::collections::HashSet<String>,
) -> Result<StorageSizeStats, String> {
    let dir = storage_conversation_dir(data_path, scope);
    storage_stats_for_directory_entries(&dir, |path, file_type| {
        if !file_type.is_dir() {
            return false;
        }
        let conversation_id = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        !conversation_id.is_empty() && !excluded_conversation_ids.contains(&conversation_id)
    })
}

fn storage_conversation_other_stats(
    data_path: &PathBuf,
    scope: StorageLegacyConversationScope,
) -> Result<StorageSizeStats, String> {
    let dir = storage_conversation_dir(data_path, scope);
    storage_stats_for_directory_entries(&dir, |path, file_type| {
        !file_type.is_dir() && !(file_type.is_file() && storage_path_extension_is(path, "json"))
    })
}

fn storage_abnormal_conversation_scan(
    state: &AppState,
) -> Result<StorageAbnormalConversationScan, String> {
    let active_bound_ids = state_service_list_remote_im_contacts(state, None)?
        .iter()
        .filter_map(|contact| contact.bound_conversation_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();
    let conversation_dir = app_layout_chat_conversations_dir(&state.data_path);
    let mut scan = StorageAbnormalConversationScan::default();
    if !conversation_dir.exists() {
        return Ok(scan);
    }
    let entries = fs::read_dir(&conversation_dir).map_err(|err| {
        format!(
            "读取异常会话目录失败，path={}，error={err}",
            conversation_dir.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            format!(
                "读取异常会话目录项失败，path={}，error={err}",
                conversation_dir.display()
            )
        })?;
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "读取异常会话目录项类型失败，path={}，error={err}",
                entry.path().display()
            )
        })?;
        if !file_type.is_dir() {
            continue;
        }
        let shard_dir = entry.path();
        let conversation_id = shard_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if conversation_id.is_empty() || active_bound_ids.contains(&conversation_id) {
            continue;
        }
        let conversation_meta = match read_conversation_meta_shard(&state.data_path, &conversation_id) {
            Ok(value) => value,
            Err(err) => {
                runtime_log_warn(format!(
                    "[存储] 跳过，任务=识别异常会话，conversation_id={}，reason=读取会话元数据失败，error={}",
                    conversation_id, err
                ));
                continue;
            }
        };
        if conversation_meta.conversation_kind().trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT {
            continue;
        }
        let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
        // 这里只判断普通会话 CURRENT（V3 SQLite）是否存在；不读取 V1/V2
        // manifest，也不尝试根据旧格式状态做运行期兼容或修复。没有 CURRENT
        // 时仅把目录作为清理候选，不解释其旧格式内容。
        if message_store::chat_store_read_status(&store_paths)?.is_some() {
            continue;
        }
        let stats = storage_stats_for_paths(vec![shard_dir.clone()])?;
        scan.stats.bytes = scan.stats.bytes.saturating_add(stats.bytes);
        scan.stats.file_count += stats.file_count;
        scan.stats.directory_count += stats.directory_count;
        scan.candidates.push(StorageAbnormalConversationCandidate {
            conversation_id,
            shard_dir,
            stats,
        });
    }
    Ok(scan)
}

fn storage_subtract_bytes(mut stats: StorageSizeStats, bytes: u64) -> StorageSizeStats {
    stats.bytes = stats.bytes.saturating_sub(bytes);
    stats
}

fn storage_image_text_cache_estimated_freed_bytes(state: &AppState) -> Result<u64, String> {
    let conn = state_db_open(&state.data_path)?;
    let bytes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(length(text) + length(hash) + length(model_api_id) + length(media_type) + length(description) + length(updated_at)), 0) FROM image_text_cache",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("统计 image_text_cache 占用失败，error={err}"))?;
    Ok(bytes.max(0) as u64)
}

fn storage_image_text_cache_stats(state: &AppState) -> Result<StorageSizeStats, String> {
    let entries = state_service_count_image_text_cache(state)?;
    let bytes = storage_image_text_cache_estimated_freed_bytes(state)?;
    Ok(StorageSizeStats {
        bytes,
        file_count: entries,
        directory_count: 0,
    })
}

fn storage_usage_target_path(state: &AppState, item_id: &str) -> Option<PathBuf> {
    let app_root = app_root_from_data_path(&state.data_path);
    let path = match item_id {
        "configuration" => state
            .config_path
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| app_root.clone()),
        "runtimeState" | "imageTextCache" => app_layout_state_dir(&state.data_path),
        "chatMetadata" => app_layout_chat_dir(&state.data_path),
        "conversationStores" | "legacyConversations" | "abnormalConversations" | "conversationOther" => {
            app_layout_chat_conversations_dir(&state.data_path)
        }
        "delegateRecords" => app_root.join("delegate"),
        "delegateConversationStores"
        | "legacyDelegateConversations"
        | "delegateConversationOther" => delegate_conversation_store_dir(&state.data_path),
        "memory" => app_root.join("memory"),
        "task" => app_root.join("task"),
        "media" => app_root.join("media"),
        "avatars" => app_root.join("avatars"),
        "exports" => app_root.join("exports"),
        "backups" => app_layout_backups_dir(&state.data_path),
        "workspace" => state.llm_workspace_path.clone(),
        "toolReview" => app_root.join("tool-review-reports"),
        "temp" => app_root.join("temp"),
        "other" => app_root,
        _ => return None,
    };
    Some(path)
}

fn storage_chat_metadata_stats(data_path: &PathBuf) -> Result<StorageSizeStats, String> {
    let chat_dir = app_layout_chat_dir(data_path);
    storage_stats_for_directory_entries(&chat_dir, |path, _file_type| {
        !storage_path_file_name_is(path, LAYOUT_DIR_CHAT_CONVERSATIONS)
    })
}

fn storage_root_other_stats(state: &AppState) -> Result<StorageSizeStats, String> {
    let app_root = app_root_from_data_path(&state.data_path);
    let config_file_name = state
        .config_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let data_file_name = state
        .data_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    storage_stats_for_directory_entries(&app_root, |path, _file_type| {
        let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        !matches!(
            name,
            LAYOUT_DIR_CONFIG
                | LAYOUT_DIR_STATE
                | LAYOUT_DIR_CHAT
                | LAYOUT_DIR_BACKUPS
                | DELEGATE_CONVERSATIONS_DIR_NAME
                | "delegate"
                | "memory"
                | "task"
                | "media"
                | "avatars"
                | "exports"
                | "llm-workspace"
                | "tool-review-reports"
                | "temp"
        ) && name != config_file_name
            && name != data_file_name
    })
}

fn build_storage_usage_overview(state: &AppState) -> Result<StorageUsageOverview, String> {
    let app_root = app_root_from_data_path(&state.data_path);
    let (legacy_conversation_stats, legacy_conversation_scan) =
        storage_legacy_conversation_stats(&state.data_path, StorageLegacyConversationScope::Normal)?;
    let (legacy_delegate_stats, legacy_delegate_scan) =
        storage_legacy_conversation_stats(&state.data_path, StorageLegacyConversationScope::Delegate)?;
    let abnormal_conversation_scan = storage_abnormal_conversation_scan(state)?;
    let image_text_cache_stats = storage_image_text_cache_stats(state)?;
    let runtime_state_stats = storage_subtract_bytes(
        storage_stats_for_paths(vec![app_layout_state_dir(&state.data_path)])?,
        image_text_cache_stats.bytes,
    );
    let abnormal_conversation_ids = abnormal_conversation_scan
        .candidates
        .iter()
        .map(|item| item.conversation_id.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut items = vec![
        storage_usage_item(
            "configuration",
            storage_usage_target_path(state, "configuration").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![state.config_path.clone(), app_layout_config_dir(&state.data_path)])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "runtimeState",
            storage_usage_target_path(state, "runtimeState").unwrap_or_else(|| app_root.clone()),
            runtime_state_stats,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "imageTextCache",
            storage_usage_target_path(state, "imageTextCache").unwrap_or_else(|| app_root.clone()),
            image_text_cache_stats.clone(),
            Some(STORAGE_CLEANUP_IMAGE_TEXT_CACHE),
            image_text_cache_stats.bytes,
            image_text_cache_stats.file_count,
        ),
        storage_usage_item(
            "chatMetadata",
            storage_usage_target_path(state, "chatMetadata").unwrap_or_else(|| app_root.clone()),
            storage_chat_metadata_stats(&state.data_path)?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "conversationStores",
            storage_usage_target_path(state, "conversationStores").unwrap_or_else(|| app_root.clone()),
            storage_current_conversation_store_stats_excluding_ids(
                &state.data_path,
                StorageLegacyConversationScope::Normal,
                &abnormal_conversation_ids,
            )?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "legacyConversations",
            storage_usage_target_path(state, "legacyConversations").unwrap_or_else(|| app_root.clone()),
            legacy_conversation_stats,
            Some(STORAGE_CLEANUP_LEGACY_CONVERSATIONS),
            legacy_conversation_scan.cleanable_bytes,
            legacy_conversation_scan.cleanable_paths.len(),
        ),
        storage_usage_item(
            "abnormalConversations",
            storage_usage_target_path(state, "abnormalConversations").unwrap_or_else(|| app_root.clone()),
            abnormal_conversation_scan.stats.clone(),
            Some(STORAGE_CLEANUP_ABNORMAL_CONVERSATIONS),
            abnormal_conversation_scan.stats.bytes,
            abnormal_conversation_scan.candidates.len(),
        ),
        storage_usage_item(
            "conversationOther",
            storage_usage_target_path(state, "conversationOther").unwrap_or_else(|| app_root.clone()),
            storage_conversation_other_stats(
                &state.data_path,
                StorageLegacyConversationScope::Normal,
            )?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "delegateRecords",
            storage_usage_target_path(state, "delegateRecords").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("delegate")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "delegateConversationStores",
            storage_usage_target_path(state, "delegateConversationStores").unwrap_or_else(|| app_root.clone()),
            storage_current_conversation_store_stats(
                &state.data_path,
                StorageLegacyConversationScope::Delegate,
            )?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "legacyDelegateConversations",
            storage_usage_target_path(state, "legacyDelegateConversations").unwrap_or_else(|| app_root.clone()),
            legacy_delegate_stats,
            Some(STORAGE_CLEANUP_LEGACY_DELEGATE_CONVERSATIONS),
            legacy_delegate_scan.cleanable_bytes,
            legacy_delegate_scan.cleanable_paths.len(),
        ),
        storage_usage_item(
            "delegateConversationOther",
            storage_usage_target_path(state, "delegateConversationOther").unwrap_or_else(|| app_root.clone()),
            storage_conversation_other_stats(
                &state.data_path,
                StorageLegacyConversationScope::Delegate,
            )?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "memory",
            storage_usage_target_path(state, "memory").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("memory")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "task",
            storage_usage_target_path(state, "task").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("task")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "media",
            storage_usage_target_path(state, "media").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("media")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "avatars",
            storage_usage_target_path(state, "avatars").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("avatars")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "exports",
            storage_usage_target_path(state, "exports").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("exports")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "backups",
            storage_usage_target_path(state, "backups").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_layout_backups_dir(&state.data_path)])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "workspace",
            storage_usage_target_path(state, "workspace").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![state.llm_workspace_path.clone()])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "toolReview",
            storage_usage_target_path(state, "toolReview").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("tool-review-reports")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "temp",
            storage_usage_target_path(state, "temp").unwrap_or_else(|| app_root.clone()),
            storage_stats_for_paths(vec![app_root.join("temp")])?,
            None,
            0,
            0,
        ),
        storage_usage_item(
            "other",
            storage_usage_target_path(state, "other").unwrap_or_else(|| app_root.clone()),
            storage_root_other_stats(state)?,
            None,
            0,
            0,
        ),
    ];
    items.sort_by(|left, right| right.bytes.cmp(&left.bytes).then_with(|| left.id.cmp(&right.id)));
    let total_bytes = items
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.bytes));
    let reclaimable_bytes = items
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.cleanable_bytes));
    Ok(StorageUsageOverview {
        root_path: app_root.to_string_lossy().to_string(),
        total_bytes,
        reclaimable_bytes,
        items,
    })
}

const OVERVIEW_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(10 * 60);

fn overview_freshness<T>(runtime: &OverviewRuntime<T>) -> String {
    match runtime.cache.as_ref() {
        None => "never".to_string(),
        Some(entry) if entry.computed_at.elapsed() < OVERVIEW_CACHE_TTL => "fresh".to_string(),
        Some(_) => "expired".to_string(),
    }
}

fn overview_snapshot<T: Clone>(runtime: &OverviewRuntime<T>) -> OverviewSnapshot<T> {
    OverviewSnapshot {
        status: OverviewStatus {
            compute_state: if runtime.running { "running" } else { "idle" }.to_string(),
            freshness: overview_freshness(runtime),
            generated_at: runtime.cache.as_ref().map(|entry| entry.generated_at.clone()),
            last_error: runtime.last_error.clone(),
        },
        data: runtime.cache.as_ref().map(|entry| entry.data.clone()),
    }
}

fn storage_overview_runtime() -> &'static tokio::sync::Mutex<OverviewRuntime<StorageUsageOverview>> {
    static RUNTIME: std::sync::OnceLock<tokio::sync::Mutex<OverviewRuntime<StorageUsageOverview>>> =
        std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::sync::Mutex::new(OverviewRuntime::default()))
}

async fn start_storage_overview_refresh_if_needed(
    state: AppState,
    force: bool,
) -> OverviewSnapshot<StorageUsageOverview> {
    let mut runtime = storage_overview_runtime().lock().await;
    let freshness = overview_freshness(&runtime);
    let should_start = !runtime.running
        && (force || (freshness != "fresh" && runtime.last_error.is_none()));
    if !should_start {
        return overview_snapshot(&runtime);
    }

    runtime.running = true;
    runtime.last_error = None;
    let snapshot = overview_snapshot(&runtime);
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || build_storage_usage_overview(&state))
            .await
            .map_err(|err| format!("计算存储用量概览任务失败：{err}"))
            .and_then(|result| result);
        let mut runtime = storage_overview_runtime().lock().await;
        runtime.running = false;
        match result {
            Ok(data) => {
                runtime.cache = Some(OverviewCacheEntry {
                    computed_at: std::time::Instant::now(),
                    generated_at: now_iso(),
                    data,
                });
                runtime.last_error = None;
            }
            Err(err) => {
                runtime.last_error = Some(err);
            }
        }
    });
    snapshot
}

#[tauri::command]
async fn get_storage_usage_overview(
    state: State<'_, AppState>,
) -> Result<OverviewSnapshot<StorageUsageOverview>, String> {
    Ok(start_storage_overview_refresh_if_needed(state.inner().clone(), false).await)
}

#[tauri::command]
async fn refresh_storage_usage_overview(
    state: State<'_, AppState>,
) -> Result<OverviewSnapshot<StorageUsageOverview>, String> {
    Ok(start_storage_overview_refresh_if_needed(state.inner().clone(), true).await)
}

fn usage_resolve_api_config_id(
    state: &AppState,
    conversation: &Conversation,
    config: &AppConfig,
) -> String {
    let preferred = conversation
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(value) = preferred {
        return value;
    }
    // 会话未固化模型时回退到负责人格的主模型（旧口径是所属部门的主模型）。
    let agent_id = conversation.agent_id.trim();
    if agent_id.is_empty() {
        return String::new();
    }
    let Ok(agents) = state_read_agents_cached(state) else {
        return String::new();
    };
    agents
        .iter()
        .find(|agent| agent.id.trim() == agent_id)
        .map(|agent| {
            resolve_chat_api_config_id(config, &agent_primary_api_config_id(agent))
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn usage_kind_key_and_label(conversation: &Conversation) -> (String, String) {
    if conversation_is_system_notification(conversation) {
        return ("system_notification".to_string(), "系统通知".to_string());
    }
    if conversation
        .delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return ("delegate".to_string(), "委托".to_string());
    }
    let kind = conversation.conversation_kind.trim();
    if kind == "remote_im_contact" {
        return ("remote_im_contact".to_string(), "远程联系人".to_string());
    }
    if conversation.archived_at.is_some() {
        return ("archived".to_string(), "已归档".to_string());
    }
    ("normal".to_string(), "普通".to_string())
}

fn usage_aggregate_push(
    map: &mut std::collections::HashMap<String, UsageAggregateItem>,
    key: String,
    label: String,
    item: &UsageConversationItem,
) {
    let entry = map.entry(key.clone()).or_insert_with(|| UsageAggregateItem {
        key,
        label,
        ..UsageAggregateItem::default()
    });
    entry.conversation_count = entry.conversation_count.saturating_add(1);
    entry.weighted_tokens = entry.weighted_tokens.saturating_add(item.weighted_tokens);
    entry.input_tokens = entry.input_tokens.saturating_add(item.input_tokens);
    entry.output_tokens = entry.output_tokens.saturating_add(item.output_tokens);
    entry.total_tokens = entry.total_tokens.saturating_add(item.total_tokens);
    entry.cache_read_tokens = entry.cache_read_tokens.saturating_add(item.cache_read_tokens);
    entry.cache_write_tokens = entry.cache_write_tokens.saturating_add(item.cache_write_tokens);
    entry.reasoning_tokens = entry.reasoning_tokens.saturating_add(item.reasoning_tokens);
}

fn usage_sort_aggregate_items(items: std::collections::HashMap<String, UsageAggregateItem>) -> Vec<UsageAggregateItem> {
    let mut out = items.into_values().collect::<Vec<_>>();
    out.sort_by(|left, right| {
        right
            .weighted_tokens
            .cmp(&left.weighted_tokens)
            .then_with(|| right.output_tokens.cmp(&left.output_tokens))
            .then_with(|| right.conversation_count.cmp(&left.conversation_count))
            .then_with(|| left.label.cmp(&right.label))
    });
    out
}

fn usage_provider_model_compound_key(provider_key: &str, model_name: &str) -> String {
    format!("{provider_key}::{model_name}")
}

fn usage_provider_model_sort_aggregate_items(
    items: std::collections::HashMap<String, UsageProviderModelAggregateItem>,
) -> Vec<UsageProviderModelAggregateItem> {
    let mut out = items.into_values().collect::<Vec<_>>();
    out.sort_by(|left, right| {
        right
            .weighted_tokens
            .cmp(&left.weighted_tokens)
            .then_with(|| right.output_tokens.cmp(&left.output_tokens))
            .then_with(|| right.conversation_count.cmp(&left.conversation_count))
            .then_with(|| left.provider_label.cmp(&right.provider_label))
            .then_with(|| left.model_name.cmp(&right.model_name))
    });
    out
}

fn usage_provider_model_aggregate_push(
    map: &mut std::collections::HashMap<String, UsageProviderModelAggregateItem>,
    provider_key: String,
    provider_label: String,
    model_name: String,
    usage: &ConversationUsageBucket,
) {
    if usage.is_empty() {
        return;
    }
    let key = usage_provider_model_compound_key(&provider_key, &model_name);
    let entry = map.entry(key.clone()).or_insert_with(|| UsageProviderModelAggregateItem {
        key,
        provider_key,
        provider_label,
        model_name,
        conversation_count: 0,
        weighted_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        total_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
    });
    entry.conversation_count = entry.conversation_count.saturating_add(1);
    entry.input_tokens = entry.input_tokens.saturating_add(usage.input_tokens);
    entry.output_tokens = entry.output_tokens.saturating_add(usage.output_tokens);
    entry.total_tokens = entry.total_tokens.saturating_add(usage.total_tokens);
    entry.cache_read_tokens = entry.cache_read_tokens.saturating_add(usage.cache_read_tokens);
    entry.cache_write_tokens = entry.cache_write_tokens.saturating_add(usage.cache_write_tokens);
    entry.reasoning_tokens = entry.reasoning_tokens.saturating_add(usage.reasoning_tokens);
    entry.weighted_tokens = entry
        .weighted_tokens
        .saturating_add(conversation_cumulative_usage_weighted_tokens(
            &ConversationCumulativeUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                total_tokens: usage.total_tokens,
                cache_read_tokens: usage.cache_read_tokens,
                cache_write_tokens: usage.cache_write_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                ..ConversationCumulativeUsage::default()
            },
        ));
}

fn usage_provider_label_from_provider_key(
    provider_key: &str,
    config: &AppConfig,
) -> String {
    let normalized_provider_key = provider_key.trim();
    if normalized_provider_key.is_empty() {
        return "未识别供应商".to_string();
    }
    config
        .api_providers
        .iter()
        .find(|item| item.id.trim() == normalized_provider_key)
        .map(|item| item.name.clone())
        .unwrap_or_else(|| normalized_provider_key.to_string())
}

/// 用量台账统一记账入口：把一次 LLM 调用按小时桶 UPSERT 累加进 usage_trail，
/// 同时快照会话维度与 provider_label。由 add_conversation_cumulative_usage_delta
/// 与委托线程 usage 落盘路径调用；失败只告警，不阻塞主流程。
fn usage_trail_record_conversation_delta(
    state: &AppState,
    conversation: &Conversation,
    provider_key: Option<&str>,
    model_name: Option<&str>,
    usage: &Value,
) {
    let Some(provider_key) = provider_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(config) = state_read_config_cached(state) else {
        return;
    };
    let tokens = message_store::usage_trail_token_delta_from_usage_value(usage);
    if tokens.is_empty() {
        return;
    }
    let delta = message_store::UsageTrailDelta {
        conversation_id: conversation.id.clone(),
        agent_id: conversation.agent_id.clone(),
        conversation_kind: usage_kind_key_and_label(conversation).0,
        api_config_id: usage_resolve_api_config_id(state, conversation, &config),
        provider_key: provider_key.to_string(),
        provider_label: usage_provider_label_from_provider_key(provider_key, &config),
        model_name: model_name.unwrap_or("").trim().to_string(),
        tokens,
    };
    let bucket = message_store::usage_trail_hour_bucket(now_utc());
    if let Err(err) =
        message_store::chat_metadata_store_usage_trail_upsert_delta(&state.data_path, &bucket, &delta)
    {
        runtime_log_warn(format!(
            "[用量台账] 写入失败，conversation_id={}，error={}",
            conversation.id, err
        ));
    }
}

fn build_usage_overview(state: &AppState) -> Result<UsageOverview, String> {
    let config = state_read_config_cached(state)?;
    let agents = state_read_agents_cached(state)?;
    let rows = message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)?;
    let mut api_config_name_map = std::collections::HashMap::<String, String>::new();
    for item in &config.api_configs {
        api_config_name_map.insert(item.id.clone(), item.name.clone());
    }
    let mut agent_name_map = std::collections::HashMap::<String, String>::new();
    let mut agent_avatar_path_map = std::collections::HashMap::<String, String>::new();
    let mut agent_avatar_updated_at_map = std::collections::HashMap::<String, String>::new();
    for agent in &agents {
        agent_name_map.insert(agent.id.clone(), agent.name.clone());
        if let Some(path) = agent.avatar_path.as_ref().map(|value| value.trim()).filter(|value| !value.is_empty()) {
            agent_avatar_path_map.insert(agent.id.clone(), path.to_string());
        }
        if let Some(updated_at) = agent
            .avatar_updated_at
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            agent_avatar_updated_at_map.insert(agent.id.clone(), updated_at.to_string());
        }
    }
    let mut totals = UsageOverviewTotals::default();

    let mut conversations = Vec::<UsageConversationItem>::new();
    let mut by_provider_model =
        std::collections::HashMap::<String, UsageProviderModelAggregateItem>::new();
    let mut by_model = std::collections::HashMap::<String, UsageAggregateItem>::new();
    let mut by_api_config = std::collections::HashMap::<String, UsageAggregateItem>::new();
    let mut by_agent = std::collections::HashMap::<String, UsageAggregateItem>::new();
    let mut by_kind = std::collections::HashMap::<String, UsageAggregateItem>::new();

    let mut per_conversation =
        std::collections::BTreeMap::<String, Vec<&message_store::UsageTrailRow>>::new();
    for row in &rows {
        per_conversation
            .entry(row.conversation_id.clone())
            .or_default()
            .push(row);
    }

    for (conversation_id, conv_rows) in per_conversation {
        let representative = conv_rows
            .iter()
            .max_by_key(|row| row.tokens.total_tokens)
            .expect("conv_rows is non-empty");
        let mut tokens = message_store::UsageTrailTokenDelta::default();
        let mut provider_model = std::collections::BTreeMap::<
            (String, String),
            (String, message_store::UsageTrailTokenDelta),
        >::new();
        for row in &conv_rows {
            tokens.saturating_add_assign(&row.tokens);
            let entry = provider_model
                .entry((row.provider_key.clone(), row.model_name.clone()))
                .or_insert_with(|| (row.provider_label.clone(), message_store::UsageTrailTokenDelta::default()));
            entry.1.saturating_add_assign(&row.tokens);
        }
        let weighted = usage_trail_weighted_tokens(&tokens);
        let meta = conversation_service_v2()
            .get_conversation_meta(state, &conversation_id)
            .ok();
        let agent_id = representative.agent_id.clone();
        let agent_name = agent_name_map.get(&agent_id).cloned().unwrap_or_else(|| {
            if agent_id.trim().is_empty() {
                "未绑定人格".to_string()
            } else {
                agent_id.clone()
            }
        });
        let api_config_id = representative.api_config_id.clone();
        let api_config_name = api_config_name_map
            .get(&api_config_id)
            .cloned()
            .unwrap_or_else(|| {
                if api_config_id.is_empty() {
                    "未绑定配置".to_string()
                } else {
                    api_config_id.clone()
                }
            });
        let model_name = if representative.model_name.trim().is_empty() {
            "unknown".to_string()
        } else {
            representative.model_name.clone()
        };
        let snapshot_kind = representative.conversation_kind.clone();
        let is_delegate = snapshot_kind == "delegate";
        let is_system_notification_conversation = snapshot_kind == "system_notification";
        // 与 usage_kind_key_and_label 一致：delegate / system_notification 优先于归档，
        // 归档只覆盖普通/远程联系人等常规 kind，避免委托会话归档后统计口径漂移
        let is_archived = meta
            .as_ref()
            .and_then(|item| item.archived_at.clone())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let kind_key = if is_delegate || is_system_notification_conversation {
            snapshot_kind
        } else if is_archived {
            "archived".to_string()
        } else {
            snapshot_kind
        };
        let kind_label = usage_kind_label_from_key(&kind_key);

        totals.conversation_count = totals.conversation_count.saturating_add(1);
        if kind_key == "archived" {
            totals.archived_conversation_count = totals.archived_conversation_count.saturating_add(1);
        } else {
            totals.active_conversation_count = totals.active_conversation_count.saturating_add(1);
        }
        if is_delegate {
            totals.delegate_conversation_count = totals.delegate_conversation_count.saturating_add(1);
        }
        totals.with_usage_conversation_count = totals.with_usage_conversation_count.saturating_add(1);
        totals.weighted_tokens = totals.weighted_tokens.saturating_add(weighted);
        totals.input_tokens = totals.input_tokens.saturating_add(tokens.input_tokens);
        totals.output_tokens = totals.output_tokens.saturating_add(tokens.output_tokens);
        totals.total_tokens = totals.total_tokens.saturating_add(tokens.total_tokens);
        totals.cache_read_tokens = totals.cache_read_tokens.saturating_add(tokens.cache_read_tokens);
        totals.cache_write_tokens = totals.cache_write_tokens.saturating_add(tokens.cache_write_tokens);
        totals.reasoning_tokens = totals.reasoning_tokens.saturating_add(tokens.reasoning_tokens);

        let usage_item = UsageConversationItem {
            conversation_id: conversation_id.clone(),
            title: meta
                .as_ref()
                .map(|item| item.title.clone())
                .unwrap_or_else(|| "已删除会话".to_string()),
            summary_title: meta.as_ref().and_then(|item| item.latest_summary_title.clone()),
            updated_at: meta.as_ref().map(|item| item.updated_at.clone()).unwrap_or_default(),
            archived_at: meta.as_ref().and_then(|item| item.archived_at.clone()),
            agent_id: agent_id.clone(),
            agent_name: agent_name.clone(),
            avatar_path: agent_avatar_path_map.get(&agent_id).cloned(),
            avatar_updated_at: agent_avatar_updated_at_map.get(&agent_id).cloned(),
            api_config_id: api_config_id.clone(),
            api_config_name: api_config_name.clone(),
            model_name: model_name.clone(),
            conversation_kind: kind_key.clone(),
            is_delegate,
            is_system_notification_conversation,
            message_count: meta.as_ref().map(|item| item.message_count).unwrap_or(0),
            weighted_tokens: weighted,
            input_tokens: tokens.input_tokens,
            output_tokens: tokens.output_tokens,
            total_tokens: tokens.total_tokens,
            cache_read_tokens: tokens.cache_read_tokens,
            cache_write_tokens: tokens.cache_write_tokens,
            reasoning_tokens: tokens.reasoning_tokens,
        };
        for ((provider_key, provider_model_name), (provider_label, provider_tokens)) in provider_model {
            let display_model_name = if provider_model_name.trim().is_empty() {
                "unknown".to_string()
            } else {
                provider_model_name.clone()
            };
            usage_provider_model_aggregate_push(
                &mut by_provider_model,
                provider_key,
                provider_label,
                display_model_name,
                &ConversationUsageBucket {
                    input_tokens: provider_tokens.input_tokens,
                    output_tokens: provider_tokens.output_tokens,
                    total_tokens: provider_tokens.total_tokens,
                    cache_read_tokens: provider_tokens.cache_read_tokens,
                    cache_write_tokens: provider_tokens.cache_write_tokens,
                    reasoning_tokens: provider_tokens.reasoning_tokens,
                },
            );
        }
        usage_aggregate_push(
            &mut by_model,
            if model_name == "unknown" {
                "unknown_model".to_string()
            } else {
                model_name.clone()
            },
            model_name,
            &usage_item,
        );
        usage_aggregate_push(
            &mut by_api_config,
            if api_config_id.trim().is_empty() {
                "unbound_api_config".to_string()
            } else {
                api_config_id.clone()
            },
            api_config_name,
            &usage_item,
        );
        usage_aggregate_push(&mut by_agent, agent_id, agent_name, &usage_item);
        usage_aggregate_push(&mut by_kind, kind_key, kind_label, &usage_item);
        conversations.push(usage_item);
    }

    conversations.sort_by(|left, right| {
        right
            .weighted_tokens
            .cmp(&left.weighted_tokens)
            .then_with(|| right.output_tokens.cmp(&left.output_tokens))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.conversation_id.cmp(&right.conversation_id))
    });

    Ok(UsageOverview {
        generated_at: now_iso(),
        totals,
        conversations,
        by_provider_model: usage_provider_model_sort_aggregate_items(by_provider_model),
        by_model: usage_sort_aggregate_items(by_model),
        by_api_config: usage_sort_aggregate_items(by_api_config),
        by_agent: usage_sort_aggregate_items(by_agent),
        by_kind: usage_sort_aggregate_items(by_kind),
    })
}

fn usage_trail_weighted_tokens(tokens: &message_store::UsageTrailTokenDelta) -> u64 {
    conversation_cumulative_usage_weighted_tokens(&ConversationCumulativeUsage {
        input_tokens: tokens.input_tokens,
        output_tokens: tokens.output_tokens,
        total_tokens: tokens.total_tokens,
        cache_read_tokens: tokens.cache_read_tokens,
        cache_write_tokens: tokens.cache_write_tokens,
        reasoning_tokens: tokens.reasoning_tokens,
        ..ConversationCumulativeUsage::default()
    })
}

fn usage_kind_label_from_key(key: &str) -> String {
    match key {
        "system_notification" => "系统通知".to_string(),
        "delegate" => "委托".to_string(),
        "remote_im_contact" => "远程联系人".to_string(),
        "archived" => "已归档".to_string(),
        "normal" => "普通".to_string(),
        other => other.to_string(),
    }
}

fn usage_overview_runtime() -> &'static tokio::sync::Mutex<OverviewRuntime<UsageOverview>> {
    static RUNTIME: std::sync::OnceLock<tokio::sync::Mutex<OverviewRuntime<UsageOverview>>> =
        std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::sync::Mutex::new(OverviewRuntime::default()))
}

async fn start_usage_overview_refresh_if_needed(
    state: AppState,
    force: bool,
) -> OverviewSnapshot<UsageOverview> {
    let mut runtime = usage_overview_runtime().lock().await;
    let freshness = overview_freshness(&runtime);
    let should_start = !runtime.running
        && (force || (freshness != "fresh" && runtime.last_error.is_none()));
    if !should_start {
        return overview_snapshot(&runtime);
    }

    runtime.running = true;
    runtime.last_error = None;
    let snapshot = overview_snapshot(&runtime);
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || build_usage_overview(&state))
            .await
            .map_err(|err| format!("计算用量概览任务失败：{err}"))
            .and_then(|result| result);
        let mut runtime = usage_overview_runtime().lock().await;
        runtime.running = false;
        match result {
            Ok(data) => {
                runtime.cache = Some(OverviewCacheEntry {
                    computed_at: std::time::Instant::now(),
                    generated_at: data.generated_at.clone(),
                    data,
                });
                runtime.last_error = None;
            }
            Err(err) => {
                runtime.last_error = Some(err);
            }
        }
    });
    snapshot
}

#[tauri::command]
async fn get_usage_overview(
    state: State<'_, AppState>,
) -> Result<OverviewSnapshot<UsageOverview>, String> {
    Ok(start_usage_overview_refresh_if_needed(state.inner().clone(), false).await)
}

#[tauri::command]
async fn refresh_usage_overview(
    state: State<'_, AppState>,
) -> Result<OverviewSnapshot<UsageOverview>, String> {
    Ok(start_usage_overview_refresh_if_needed(state.inner().clone(), true).await)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallQuery {
    view: String,
    year: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallView {
    generated_at: String,
    view: String,
    totals: UsageTrailWallTotals,
    epoch_totals: Option<UsageTrailWallTotals>,
    hourly: Vec<UsageTrailWallHour>,
    peak_hour: Option<u8>,
    year: String,
    years: Vec<String>,
    calendar: Vec<UsageTrailWallDay>,
    top_conversation_label: Option<String>,
    top_conversation_percent: Option<u64>,
    active_period_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallTotals {
    conversation_count: usize,
    weighted_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallHour {
    hour: u8,
    total_tokens: u64,
    conversation_count: usize,
    models: Vec<UsageTrailWallModel>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallModel {
    model: String,
    tokens: u64,
    provider_label: String,
    reasoning_effort: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct UsageTrailWallDay {
    date: String,
    total_tokens: u64,
    conversation_count: usize,
}

/// 足迹墙 today 起点：凌晨 4 点分界（当前时刻往前推 4 小时所在分界日的 04:00）。
/// 0:00-3:59 的使用属于前一个分界日，因此分界日由 now-4h 的日期决定。
fn usage_trail_wall_business_day(now_local: OffsetDateTime) -> String {
    let shifted = now_local - time::Duration::hours(4);
    format!(
        "{:04}-{:02}-{:02}",
        shifted.year(),
        shifted.month() as u8,
        shifted.day()
    )
}

fn usage_trail_wall_totals_from_rows(rows: &[message_store::UsageTrailRow]) -> UsageTrailWallTotals {
    let mut totals = UsageTrailWallTotals::default();
    let mut seen = std::collections::HashSet::<String>::new();
    for row in rows {
        seen.insert(row.conversation_id.clone());
        totals.weighted_tokens = totals
            .weighted_tokens
            .saturating_add(usage_trail_weighted_tokens(&row.tokens));
        totals.input_tokens = totals.input_tokens.saturating_add(row.tokens.input_tokens);
        totals.output_tokens = totals.output_tokens.saturating_add(row.tokens.output_tokens);
        totals.total_tokens = totals.total_tokens.saturating_add(row.tokens.total_tokens);
        totals.cache_read_tokens = totals
            .cache_read_tokens
            .saturating_add(row.tokens.cache_read_tokens);
        totals.cache_write_tokens = totals
            .cache_write_tokens
            .saturating_add(row.tokens.cache_write_tokens);
        totals.reasoning_tokens = totals
            .reasoning_tokens
            .saturating_add(row.tokens.reasoning_tokens);
    }
    totals.conversation_count = seen.len();
    totals
}

/// 从小时桶字符串解析小时（bucket 格式 YYYY-MM-DDTHH:00:00）。
fn usage_trail_wall_hour_from_bucket(bucket: &str) -> Option<u8> {
    bucket.get(11..13)?.parse::<u8>().ok()
}

/// 今天视图：24 小时格 + 峰值小时（weighted token 最大）+ 每格按模型拆分。
fn usage_trail_wall_hourly(
    rows: &[message_store::UsageTrailRow],
    config: &AppConfig,
) -> (Vec<UsageTrailWallHour>, Option<u8>) {
    let mut effort_by_config = std::collections::HashMap::<String, String>::new();
    for item in &config.api_configs {
        effort_by_config.insert(item.id.clone(), item.reasoning_effort.clone());
    }
    let mut by_hour = std::collections::BTreeMap::<
        u8,
        (
            u64,
            u64,
            std::collections::HashSet<String>,
            std::collections::BTreeMap<String, (u64, String, String, String)>,
        ),
    >::new();
    for row in rows {
        let Some(hour) = usage_trail_wall_hour_from_bucket(&row.bucket) else {
            continue;
        };
        let entry = by_hour
            .entry(hour)
            .or_insert_with(|| (0, 0, std::collections::HashSet::new(), std::collections::BTreeMap::new()));
        entry.0 = entry
            .0
            .saturating_add(row.tokens.total_tokens);
        entry.1 = entry.1.saturating_add(row.tokens.total_tokens);
        entry.2.insert(row.conversation_id.clone());
        // 聚合键含 provider_key，同名模型在不同供应商下分开统计，避免 label 串用
        let model_key = format!("{}::{}", row.provider_key, row.model_name);
        let model_entry = entry.3.entry(model_key).or_insert_with(|| {
            let effort = effort_by_config
                .get(&row.api_config_id)
                .cloned()
                .unwrap_or_default();
            (0, row.provider_label.clone(), effort, row.model_name.clone())
        });
        model_entry.0 = model_entry
            .0
            .saturating_add(row.tokens.total_tokens);
    }
    let mut out = Vec::<UsageTrailWallHour>::with_capacity(24);
    let mut peak_hour: Option<u8> = None;
    let mut peak_weighted = 0_u64;
    for hour in 0..24_u8 {
        let entry = by_hour.get(&hour);
        let weighted = entry.map(|item| item.0).unwrap_or(0);
        if weighted > peak_weighted {
            peak_weighted = weighted;
            peak_hour = Some(hour);
        }
        let models = entry
            .map(|item| {
                item.3
                    .iter()
                    .map(|(model_key, (tokens, provider_label, reasoning_effort, _model_name))| {
                        UsageTrailWallModel {
                            model: model_key.clone(),
                            tokens: *tokens,
                            provider_label: provider_label.clone(),
                            reasoning_effort: reasoning_effort.clone(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.push(UsageTrailWallHour {
            hour,
            total_tokens: entry.map(|item| item.1).unwrap_or(0),
            conversation_count: entry.map(|item| item.2.len()).unwrap_or(0),
            models,
        });
    }
    (out, peak_hour)
}

/// 历史视图：给定年份的全年日历（1/1~12/31，含无数据日补 0）。
fn usage_trail_wall_calendar(
    year: i32,
    rows: &[message_store::UsageTrailRow],
) -> Vec<UsageTrailWallDay> {
    let mut by_day = std::collections::BTreeMap::<String, (u64, std::collections::HashSet<String>)>::new();
    for row in rows {
        if row.bucket.len() < 10 || row.bucket[..4] != format!("{:04}", year) {
            continue;
        }
        let day = row.bucket[..10].to_string();
        let entry = by_day
            .entry(day)
            .or_insert_with(|| (0, std::collections::HashSet::new()));
        entry.0 = entry.0.saturating_add(row.tokens.total_tokens);
        entry.1.insert(row.conversation_id.clone());
    }
    let jan1 = time::Date::from_calendar_date(year, time::Month::January, 1)
        .unwrap_or_else(|_| time::Date::from_calendar_date(2026, time::Month::January, 1).unwrap());
    let days_in_year = time::Date::from_calendar_date(year, time::Month::December, 31)
        .ok()
        .and_then(|dec31| {
            let start = time::Date::from_calendar_date(year, time::Month::January, 1).ok()?;
            Some((dec31 - start).whole_days() + 1)
        })
        .unwrap_or(365);
    let mut out = Vec::<UsageTrailWallDay>::with_capacity(days_in_year as usize);
    for offset in 0..days_in_year {
        let date = jan1.saturating_add(time::Duration::days(offset));
        let key = format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            date.month() as u8,
            date.day()
        );
        let entry = by_day.get(&key);
        out.push(UsageTrailWallDay {
            date: key,
            total_tokens: entry.map(|item| item.0).unwrap_or(0),
            conversation_count: entry.map(|item| item.1.len()).unwrap_or(0),
        });
    }
    out
}

/// 历史视图可用年份（小时桶数据年份，升序）。
fn usage_trail_wall_years(rows: &[message_store::UsageTrailRow]) -> Vec<String> {
    let mut years = rows
        .iter()
        .filter_map(|row| {
            if row.bucket.len() >= 4 && row.bucket[..4].parse::<i32>().is_ok() {
                Some(row.bucket[..4].to_string())
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    years.reverse();
    years
}

/// 历史视图 top 会话：按 weighted token 聚合取最大，label 走 title → summary → 未命名 → 已删除 兜底链。
fn usage_trail_wall_top_conversation(
    state: &AppState,
    rows: &[message_store::UsageTrailRow],
) -> Result<Option<(String, u64)>, String> {
    let mut by_conversation = std::collections::BTreeMap::<String, u64>::new();
    let mut total_weighted = 0_u64;
    for row in rows {
        let weighted = usage_trail_weighted_tokens(&row.tokens);
        total_weighted = total_weighted.saturating_add(weighted);
        let entry = by_conversation
            .entry(row.conversation_id.clone())
            .or_insert(0);
        *entry = entry.saturating_add(weighted);
    }
    if total_weighted == 0 {
        return Ok(None);
    }
    let Some((conversation_id, weighted)) = by_conversation
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1))
    else {
        return Ok(None);
    };
    let label = conversation_service_v2()
        .get_conversation_meta(state, &conversation_id)
        .map(|meta| {
            let title = meta.title.trim();
            if !title.is_empty() {
                title.to_string()
            } else if let Some(summary) = meta
                .latest_summary_title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                summary.to_string()
            } else {
                "未命名会话".to_string()
            }
        })
        .unwrap_or_else(|_| "已删除会话".to_string());
    let percent = if total_weighted > 0 {
        (weighted as u128 * 100 / total_weighted as u128) as u64
    } else {
        0
    };
    Ok(Some((label, percent)))
}

/// 历史视图最活跃时段：凌晨 0-5 / 上午 6-11 / 下午 12-17 / 晚上 18-23，按 weighted token 取最大。
fn usage_trail_wall_active_period(rows: &[message_store::UsageTrailRow]) -> Option<String> {
    let mut buckets = [0_u64; 4];
    for row in rows {
        let Some(hour) = usage_trail_wall_hour_from_bucket(&row.bucket) else {
            continue;
        };
        let index = match hour {
            0..=5 => 0,
            6..=11 => 1,
            12..=17 => 2,
            _ => 3,
        };
        buckets[index] = buckets[index].saturating_add(usage_trail_weighted_tokens(&row.tokens));
    }
    let max_index = buckets
        .iter()
        .enumerate()
        .max_by_key(|(_, value)| **value)
        .map(|(index, _)| index)?;
    if buckets[max_index] == 0 {
        return None;
    }
    Some(match max_index {
        0 => "dawn".to_string(),
        1 => "morning".to_string(),
        2 => "afternoon".to_string(),
        _ => "night".to_string(),
    })
}

fn build_usage_trail_wall(
    state: &AppState,
    query: &UsageTrailWallQuery,
) -> Result<UsageTrailWallView, String> {
    let config = state_read_config_cached(state)?;
    let view = query.view.trim().to_string();
    let now_local = to_local_datetime(now_utc());
    let rows = message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)?;
    let (window_rows, epoch_rows): (Vec<_>, Vec<_>) = rows
        .into_iter()
        .partition(|row| row.bucket != message_store::USAGE_TRAIL_EPOCH_BUCKET);
    let epoch_totals = usage_trail_wall_totals_from_rows(&epoch_rows);
    let years = usage_trail_wall_years(&window_rows);
    let default_year = years
        .first()
        .cloned()
        .unwrap_or_else(|| format!("{:04}", now_local.year()));
    if view == "today" {
        let business_day = usage_trail_wall_business_day(now_local);
        let today_rows = window_rows
            .iter()
            .filter(|row| row.bucket.starts_with(&business_day))
            .cloned()
            .collect::<Vec<_>>();
        let totals = usage_trail_wall_totals_from_rows(&today_rows);
        let (hourly, peak_hour) = usage_trail_wall_hourly(&today_rows, &config);
        return Ok(UsageTrailWallView {
            generated_at: now_iso(),
            view: "today".to_string(),
            totals,
            epoch_totals: Some(epoch_totals),
            hourly,
            peak_hour,
            year: default_year,
            years,
            calendar: Vec::new(),
            top_conversation_label: None,
            top_conversation_percent: None,
            active_period_label: None,
        });
    }
    let year = query
        .year
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| default_year.clone());
    let year_rows = window_rows
        .iter()
        .filter(|row| row.bucket.len() >= 4 && row.bucket[..4] == year)
        .cloned()
        .collect::<Vec<_>>();
    let totals = usage_trail_wall_totals_from_rows(&year_rows);
    let parsed_year = year
        .parse::<i32>()
        .unwrap_or_else(|_| now_local.year());
    let calendar = usage_trail_wall_calendar(parsed_year, &year_rows);
    let top_conversation = usage_trail_wall_top_conversation(state, &year_rows)?;
    let active_period_label = usage_trail_wall_active_period(&year_rows);
    Ok(UsageTrailWallView {
        generated_at: now_iso(),
        view: "history".to_string(),
        totals,
        epoch_totals: Some(epoch_totals),
        hourly: Vec::new(),
        peak_hour: None,
        year,
        years,
        calendar,
        top_conversation_label: top_conversation.as_ref().map(|item| item.0.clone()),
        top_conversation_percent: top_conversation.as_ref().map(|item| item.1),
        active_period_label,
    })
}

#[tauri::command]
async fn get_usage_trail(
    state: State<'_, AppState>,
    input: UsageTrailWallQuery,
) -> Result<UsageTrailWallView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || build_usage_trail_wall(&state, &input))
        .await
        .map_err(|err| format!("计算足迹墙失败：{err}"))
        .and_then(|result| result)
}

fn storage_existing_directory_for_open(path: &PathBuf) -> Result<PathBuf, String> {
    if path.exists() {
        if path.is_dir() {
            return Ok(path.clone());
        }
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }
    let mut current = path.as_path();
    while let Some(parent) = current.parent() {
        if parent.exists() && parent.is_dir() {
            return Ok(parent.to_path_buf());
        }
        current = parent;
    }
    Err(format!("目标目录不存在，path={}", path.display()))
}

#[tauri::command]
fn open_storage_usage_item_directory(
    state: State<'_, AppState>,
    input: OpenStorageUsageItemDirectoryInput,
) -> Result<(), String> {
    let item_id = input.item_id.trim();
    let target = storage_usage_target_path(&state, item_id)
        .ok_or_else(|| format!("未知存储分类：{item_id}"))?;
    let app_root = app_root_from_data_path(&state.data_path);
    let open_dir = storage_existing_directory_for_open(&target)?;
    let canonical_root = app_root.canonicalize().unwrap_or(app_root);
    let canonical_open_dir = open_dir.canonicalize().unwrap_or(open_dir.clone());
    if !canonical_open_dir.starts_with(&canonical_root) {
        return Err(format!(
            "拒绝打开应用私有目录之外的路径，path={}",
            canonical_open_dir.display()
        ));
    }
    open_shell_path_in_file_manager(&canonical_open_dir)
}

fn cleanup_storage_legacy_scope(
    state: &AppState,
    scope: StorageLegacyConversationScope,
) -> Result<StorageCleanupResult, String> {
    let scan = storage_scan_legacy_cleanup_candidates(&state.data_path, scope)?;
    let skipped_file_count = scan
        .total_file_count
        .saturating_sub(scan.cleanable_paths.len());
    let expected_dir = storage_conversation_dir(&state.data_path, scope);
    let mut deleted_file_count = 0;
    let mut freed_bytes = 0_u64;
    for path in scan.cleanable_paths {
        if path.parent() != Some(expected_dir.as_path()) {
            return Err(format!(
                "拒绝清理旧会话文件：候选路径不在预期目录内，path={}，expected_dir={}",
                path.display(),
                expected_dir.display()
            ));
        }
        let bytes = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
        fs::remove_file(&path).map_err(|err| {
            format!("删除旧会话文件失败，path={}，error={err}", path.display())
        })?;
        deleted_file_count += 1;
        freed_bytes = freed_bytes.saturating_add(bytes);
    }
    if deleted_file_count > 0 {
        refresh_message_store_migration_caches(state)?;
    }
    Ok(StorageCleanupResult {
        deleted_file_count,
        skipped_file_count,
        freed_bytes,
    })
}

fn cleanup_storage_abnormal_conversations(state: &AppState) -> Result<StorageCleanupResult, String> {
    let scan = storage_abnormal_conversation_scan(state)?;
    let expected_dir = app_layout_chat_conversations_dir(&state.data_path);
    let mut deleted_file_count = 0;
    let mut freed_bytes = 0_u64;
    for candidate in scan.candidates {
        if candidate.shard_dir.parent() != Some(expected_dir.as_path()) {
            return Err(format!(
                "拒绝清理异常会话目录：候选路径不在预期目录内，path={}，expected_dir={}",
                candidate.shard_dir.display(),
                expected_dir.display()
            ));
        }
        state_delete_conversation_cached(state, &candidate.conversation_id)?;
        deleted_file_count += 1;
        freed_bytes = freed_bytes.saturating_add(candidate.stats.bytes);
    }
    Ok(StorageCleanupResult {
        deleted_file_count,
        skipped_file_count: 0,
        freed_bytes,
    })
}

fn cleanup_storage_image_text_cache(state: &AppState) -> Result<StorageCleanupResult, String> {
    let deleted_file_count = state_service_count_image_text_cache(state)?;
    let freed_bytes = storage_image_text_cache_estimated_freed_bytes(state)?;
    state_service_clear_image_text_cache(state)?;
    Ok(StorageCleanupResult {
        deleted_file_count,
        skipped_file_count: 0,
        freed_bytes,
    })
}

#[tauri::command]
fn cleanup_storage_legacy_items(
    state: State<'_, AppState>,
    input: CleanupStorageLegacyItemsInput,
) -> Result<StorageCleanupResult, String> {
    cleanup_storage_legacy_items_inner(state.inner(), input)
}

fn cleanup_storage_legacy_items_inner(
    state: &AppState,
    input: CleanupStorageLegacyItemsInput,
) -> Result<StorageCleanupResult, String> {
    let cleanup_kind = input.cleanup_kind.trim();
    let (scope, label) = match cleanup_kind {
        STORAGE_CLEANUP_LEGACY_CONVERSATIONS => (
            StorageLegacyConversationScope::Normal,
            "旧普通会话 JSON",
        ),
        STORAGE_CLEANUP_LEGACY_DELEGATE_CONVERSATIONS => (
            StorageLegacyConversationScope::Delegate,
            "旧委托会话 JSON",
        ),
        STORAGE_CLEANUP_ABNORMAL_CONVERSATIONS => {
            let _migration_guard = lock_message_store_migration();
            runtime_log_error(format!(
                "[存储] 开始，任务=清理异常会话目录，cleanup_kind={}",
                cleanup_kind
            ));
            let started_at = std::time::Instant::now();
            let result = cleanup_storage_abnormal_conversations(state);
            match &result {
                Ok(report) => runtime_log_warn(format!(
                    "[存储] 完成，任务=清理异常会话目录，cleanup_kind={}，删除文件数={}，跳过文件数={}，释放字节={}，耗时毫秒={}",
                    cleanup_kind,
                    report.deleted_file_count,
                    report.skipped_file_count,
                    report.freed_bytes,
                    started_at.elapsed().as_millis()
                )),
                Err(err) => runtime_log_error(format!(
                    "[存储] 失败，任务=清理异常会话目录，cleanup_kind={}，error={}，耗时毫秒={}",
                    cleanup_kind,
                    err,
                    started_at.elapsed().as_millis()
                )),
            }
            return result;
        }
        STORAGE_CLEANUP_IMAGE_TEXT_CACHE => {
            runtime_log_info(format!(
                "[存储] 开始，任务=清理多媒体解析缓存，cleanup_kind={}",
                cleanup_kind
            ));
            let started_at = std::time::Instant::now();
            let result = cleanup_storage_image_text_cache(state);
            match &result {
                Ok(report) => runtime_log_warn(format!(
                    "[存储] 完成，任务=清理多媒体解析缓存，cleanup_kind={}，删除文件数={}，跳过文件数={}，释放字节={}，耗时毫秒={}",
                    cleanup_kind,
                    report.deleted_file_count,
                    report.skipped_file_count,
                    report.freed_bytes,
                    started_at.elapsed().as_millis()
                )),
                Err(err) => runtime_log_error(format!(
                    "[存储] 失败，任务=清理多媒体解析缓存，cleanup_kind={}，error={}，耗时毫秒={}",
                    cleanup_kind,
                    err,
                    started_at.elapsed().as_millis()
                )),
            }
            return result;
        }
        _ => return Err(format!("未知存储清理类型：{cleanup_kind}")),
    };
    let _migration_guard = lock_message_store_migration();
    runtime_log_info(format!(
        "[存储] 开始，任务=清理{}，cleanup_kind={}",
        label,
        cleanup_kind
    ));
    let started_at = std::time::Instant::now();
    let result = cleanup_storage_legacy_scope(state, scope);
    match &result {
        Ok(report) => runtime_log_warn(format!(
            "[存储] 完成，任务=清理{}，cleanup_kind={}，删除文件数={}，跳过文件数={}，释放字节={}，耗时毫秒={}",
            label,
            cleanup_kind,
            report.deleted_file_count,
            report.skipped_file_count,
            report.freed_bytes,
            started_at.elapsed().as_millis()
        )),
        Err(err) => runtime_log_error(format!(
            "[存储] 失败，任务=清理{}，cleanup_kind={}，error={}，耗时毫秒={}",
            label,
            cleanup_kind,
            err,
            started_at.elapsed().as_millis()
        )),
    }
    result
}

#[cfg(test)]
mod storage_usage_tests {
    use super::*;
    use serde_json::json;

    fn storage_usage_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-storage-usage-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(std::collections::HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn storage_usage_test_remote_contact(bound_conversation_id: Option<&str>) -> RemoteImContact {
        RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: bound_conversation_id.map(str::to_string),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: Some(now_iso()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        }
    }

    fn storage_usage_test_message(id: &str, role: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: "2026-06-08T00:00:00Z".to_string(),
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

    fn storage_usage_test_conversation(conversation_id: &str, kind: &str) -> Conversation {
        Conversation {
            id: conversation_id.to_string(),
            title: "测试会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: kind.to_string(),
            root_conversation_id: (kind == CONVERSATION_KIND_DELEGATE)
                .then_some("root-conversation".to_string()),
            delegate_id: (kind == CONVERSATION_KIND_DELEGATE)
                .then_some(conversation_id.to_string()),
            created_at: "2026-06-08T00:00:00Z".to_string(),
            updated_at: "2026-06-08T00:00:01Z".to_string(),
            last_user_at: Some("2026-06-08T00:00:00Z".to_string()),
            last_assistant_at: Some("2026-06-08T00:00:01Z".to_string()),
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![
                storage_usage_test_message("m1", "user"),
                storage_usage_test_message("m2", "assistant"),
            ],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    fn storage_usage_test_delegate_legacy_path(data_path: &PathBuf, conversation_id: &str) -> PathBuf {
        delegate_conversation_store_dir(data_path).join(format!("{conversation_id}.json"))
    }

    #[test]
    fn storage_image_text_cache_estimated_freed_bytes_should_track_runtime_payload() {
        let state = storage_usage_test_state();
        assert_eq!(
            storage_image_text_cache_estimated_freed_bytes(&state).expect("empty cache bytes"),
            0
        );

        state_service_upsert_image_text_cache(
            &state,
            "hash-a",
            "vision-a",
            "image",
            "截图解析",
            "这是一段多媒体解析结果。".repeat(8).as_str(),
        )
        .expect("upsert image text cache");

        assert!(
            storage_image_text_cache_estimated_freed_bytes(&state).expect("cache bytes") > 0
        );
    }

    #[test]
    fn storage_cleanup_candidates_require_ready_normal_message_store() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-storage-cleanup-normal-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let ready = storage_usage_test_conversation("ready-conversation", "");
        let legacy_only = storage_usage_test_conversation("legacy-only", "");
        let ready_paths =
            message_store::message_store_paths(&data_path, &ready.id).expect("ready paths");
        message_store::chat_store_write_snapshot(&ready_paths, &ready)
            .expect("write ready message store");
        write_json_file_atomic(
            &app_layout_chat_conversation_path(&data_path, &ready.id),
            &ready,
            "legacy ready conversation",
        )
        .expect("write ready legacy");
        write_json_file_atomic(
            &app_layout_chat_conversation_path(&data_path, &legacy_only.id),
            &legacy_only,
            "legacy only conversation",
        )
        .expect("write legacy only");

        let scan = storage_scan_legacy_cleanup_candidates(
            &data_path,
            StorageLegacyConversationScope::Normal,
        )
        .expect("scan cleanup candidates");

        assert_eq!(scan.total_file_count, 2);
        assert_eq!(scan.cleanable_paths.len(), 1);
        assert_eq!(
            scan.cleanable_paths[0],
            app_layout_chat_conversation_path(&data_path, &ready.id)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn storage_cleanup_candidates_require_ready_delegate_message_store() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-storage-cleanup-delegate-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let entry = delegate_store_create_delegate(
            &data_path,
            &DelegateCreateInput {
                kind: "delegate".to_string(),
                conversation_id: "root-conversation".to_string(),
                parent_delegate_id: None,
                source_agent_id: "source-agent".to_string(),
                target_agent_id: DEFAULT_AGENT_ID.to_string(),
                title: "清理测试委托".to_string(),
                why: "验证存储清理".to_string(),
                goal: "写入委托快照".to_string(),
                todo: "完成测试".to_string(),
                notify_assistant_when_done: false,
                call_stack: Vec::new(),
            },
        )
        .expect("create delegate record");
        let mut ready = storage_usage_test_conversation(&entry.delegate_id, CONVERSATION_KIND_DELEGATE);
        ready.delegate_id = Some(entry.delegate_id.clone());
        ready.root_conversation_id = Some(entry.conversation_id.clone());
        let legacy_only =
            storage_usage_test_conversation("delegate-legacy", CONVERSATION_KIND_DELEGATE);
        delegate_conversation_store_write(&data_path, &ready).expect("write ready delegate store");
        let ready_legacy_path = storage_usage_test_delegate_legacy_path(&data_path, &ready.id);
        let legacy_only_path = storage_usage_test_delegate_legacy_path(&data_path, &legacy_only.id);
        write_json_file_atomic(&ready_legacy_path, &ready, "legacy ready delegate")
            .expect("write ready legacy delegate");
        write_json_file_atomic(&legacy_only_path, &legacy_only, "legacy only delegate")
            .expect("write legacy only delegate");

        let scan = storage_scan_legacy_cleanup_candidates(
            &data_path,
            StorageLegacyConversationScope::Delegate,
        )
        .expect("scan delegate cleanup candidates");

        assert_eq!(scan.total_file_count, 2);
        assert_eq!(scan.cleanable_paths.len(), 1);
        assert_eq!(scan.cleanable_paths[0], ready_legacy_path);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn storage_abnormal_conversation_scan_should_ignore_legacy_manifest_state() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-storage-abnormal-conversations-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let mut stale =
            storage_usage_test_conversation("remote-stale", CONVERSATION_KIND_REMOTE_IM_CONTACT);
        stale.root_conversation_id = Some("remote_im_contact:channel-a:group:remote-a".to_string());
        let mut active =
            storage_usage_test_conversation("remote-active", CONVERSATION_KIND_REMOTE_IM_CONTACT);
        active.root_conversation_id = Some("remote_im_contact:channel-a:group:remote-a".to_string());

        let stale_paths =
            message_store::message_store_paths(&data_path, &stale.id).expect("stale paths");
        let active_paths =
            message_store::message_store_paths(&data_path, &active.id).expect("active paths");
        message_store::migration_v1_to_v2_conversation(&stale_paths, &stale, false)
            .expect("write stale shard");
        message_store::migration_v1_to_v2_conversation(&active_paths, &active, false)
            .expect("write active shard");
        let building_manifest = message_store::MessageStoreManifest::jsonl_snapshot_building(&stale);
        let stale_manifest_path = app_layout_chat_conversations_dir(&data_path)
            .join(&stale.id)
            .join(message_store::MESSAGE_STORE_MANIFEST_FILE_NAME);
        message_store::write_message_store_manifest_atomic(&stale_manifest_path, &building_manifest)
            .expect("downgrade stale manifest to building");

        let state = storage_usage_test_state();
        let state = AppState {
            data_path: data_path.clone(),
            ..state
        };
        state_service_upsert_remote_im_contact(
            &state,
            &storage_usage_test_remote_contact(Some(&active.id)),
        )
        .expect("upsert remote im contact");

        let scan =
            storage_abnormal_conversation_scan(&state).expect("scan abnormal conversations");

        assert_eq!(scan.candidates.len(), 0);
        let raw_manifest = fs::read_to_string(
            app_layout_chat_conversations_dir(&data_path)
                .join(&stale.id)
                .join(message_store::MESSAGE_STORE_MANIFEST_FILE_NAME),
        )
            .expect("legacy manifest remains untouched");
        assert!(raw_manifest.contains("building"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn usage_overview_should_resolve_model_name_from_usage_trail() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            api_providers: vec![ApiProviderConfig {
                id: "provider-a".to_string(),
                name: "主供应商".to_string(),
                deprecated: true,
                models: vec![ApiModelConfig {
                    id: "model-a".to_string(),
                    model: "real-model-v1".to_string(),
                    deprecated: true,
                    ..ApiModelConfig::default()
                }],
                ..ApiProviderConfig::default()
            }],
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let mut conversation = storage_usage_test_conversation("usage-trail-model", "");
        conversation.preferred_api_config_id = Some("provider-a::model-a".to_string());
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        conversation_service_v2()
            .add_conversation_cumulative_usage_delta(
                &state,
                &conversation.id,
                Some("provider-a"),
                Some("real-model-v1"),
                &json!({"promptTokens": 11, "completionTokens": 7, "totalTokens": 18}),
            )
            .expect("add usage delta");

        let rows = message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)
            .expect("query usage trail");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_label, "主供应商", "provider_label 应写时快照");
        assert_eq!(rows[0].model_name, "real-model-v1");

        let overview = build_usage_overview(&state).expect("build usage overview");
        let conversation_item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("conversation item");
        assert_eq!(conversation_item.model_name, "real-model-v1");
        assert_eq!(conversation_item.total_tokens, 18);
        assert_eq!(conversation_item.input_tokens, 11);
        assert_eq!(conversation_item.output_tokens, 7);
        assert!(
            overview.by_provider_model.iter().any(|item| {
                item.model_name == "real-model-v1"
                    && item.total_tokens == 18
                    && item.input_tokens == 11
                    && item.output_tokens == 7
            }),
            "usage overview should expose provider/model breakdown from usage trail"
        );
    }

    #[test]
    fn usage_overview_should_resolve_unknown_model_when_model_missing() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let mut conversation = storage_usage_test_conversation("usage-trail-unknown", "");
        conversation.preferred_api_config_id = Some("missing-provider::missing-model".to_string());
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        conversation_service_v2()
            .add_conversation_cumulative_usage_delta(
                &state,
                &conversation.id,
                Some("missing-provider"),
                None,
                &json!({"promptTokens": 3, "completionTokens": 2, "totalTokens": 5}),
            )
            .expect("add usage delta");

        let overview = build_usage_overview(&state).expect("build usage overview");
        let conversation_item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("conversation item");
        assert_eq!(conversation_item.model_name, "unknown");
        assert!(
            overview.by_provider_model.iter().any(|item| {
                item.model_name == "unknown"
                    && item.total_tokens == 5
                    && item.input_tokens == 3
                    && item.output_tokens == 2
            }),
            "usage overview should degrade missing model name to unknown"
        );
    }

    #[test]
    fn usage_overview_should_prefer_highest_usage_model_name() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let mut conversation = storage_usage_test_conversation("multi-model-usage", "");
        conversation.preferred_api_config_id = Some("provider-a::model-a".to_string());
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        conversation_service_v2()
            .add_conversation_cumulative_usage_delta(
                &state,
                &conversation.id,
                Some("provider-a"),
                Some("small-model"),
                &json!({"promptTokens": 2, "completionTokens": 1, "totalTokens": 3}),
            )
            .expect("add small model usage");
        conversation_service_v2()
            .add_conversation_cumulative_usage_delta(
                &state,
                &conversation.id,
                Some("provider-a"),
                Some("big-model"),
                &json!({"promptTokens": 13, "completionTokens": 11, "totalTokens": 24}),
            )
            .expect("add big model usage");

        let overview = build_usage_overview(&state).expect("build usage overview");
        let conversation_item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("conversation item");
        assert_eq!(conversation_item.model_name, "big-model");
        assert_eq!(conversation_item.total_tokens, 27);
        assert_eq!(conversation_item.input_tokens, 15);
        assert_eq!(conversation_item.output_tokens, 12);
    }

    #[test]
    fn usage_overview_should_include_delegate_usage_from_usage_trail() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let delta = message_store::UsageTrailDelta {
            conversation_id: "delegate-usage-trail".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            conversation_kind: "delegate".to_string(),
            api_config_id: "provider-a::model-a".to_string(),
            provider_key: "provider-a".to_string(),
            provider_label: "主供应商".to_string(),
            model_name: "delegate-model".to_string(),
            tokens: message_store::UsageTrailTokenDelta {
                input_tokens: 21,
                output_tokens: 8,
                total_tokens: 29,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
            },
        };
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &delta,
        )
        .expect("upsert delegate usage trail");

        let overview = build_usage_overview(&state).expect("build usage overview");
        let conversation_item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == "delegate-usage-trail")
            .expect("delegate conversation item");
        assert_eq!(conversation_item.model_name, "delegate-model");
        assert_eq!(conversation_item.weighted_tokens, 16);
        assert_eq!(conversation_item.input_tokens, 21);
        assert_eq!(conversation_item.output_tokens, 8);
        assert!(conversation_item.is_delegate);
        assert!(
            overview.by_provider_model.iter().any(|item| {
                item.model_name == "delegate-model"
                    && item.input_tokens == 21
                    && item.output_tokens == 8
            }),
            "delegate usage should contribute provider/model breakdown from usage trail"
        );
    }

    #[test]
    fn usage_overview_should_keep_delegate_kind_priority_over_archived() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 写一个 delegate 会话并归档，kind 口径应保持 delegate（与 usage_kind_key_and_label 一致）
        let mut conversation = storage_usage_test_conversation("delegate-archived", CONVERSATION_KIND_DELEGATE);
        conversation.title = "已归档委托".to_string();
        conversation.archived_at = Some(now_iso());
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &message_store::UsageTrailDelta {
                conversation_id: conversation.id.clone(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "delegate".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "delegate-model".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: 5,
                    output_tokens: 2,
                    total_tokens: 7,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            },
        )
        .expect("upsert archived delegate usage");

        let overview = build_usage_overview(&state).expect("build usage overview");
        let item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("archived delegate item");
        assert_eq!(item.conversation_kind, "delegate", "委托会话归档后仍应归 delegate");
        assert!(item.is_delegate);
        assert_eq!(item.title, "已归档委托");
        assert_eq!(overview.totals.delegate_conversation_count, 1);
        assert_eq!(overview.totals.archived_conversation_count, 0, "委托不计入 archived");
        assert!(
            overview.by_kind.iter().any(|item| item.key == "delegate"),
            "by_kind 应包含 delegate 分组"
        );
    }

    #[test]
    fn usage_trail_wall_today_should_aggregate_hourly_and_peak() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 会话存在但 title 为空，足迹墙 top 会话 label 应走 summary/未命名兜底而不是空串
        let mut conversation = storage_usage_test_conversation("empty-title-conv", "");
        conversation.title = String::new();
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &message_store::UsageTrailDelta {
                conversation_id: conversation.id.clone(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "model-a".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: 3,
                    output_tokens: 1,
                    total_tokens: 4,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            },
        )
        .expect("upsert empty title usage");
        // 同一小时另一个模型，验证 hourly.models 按模型拆分
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &message_store::UsageTrailDelta {
                conversation_id: conversation.id.clone(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-b".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "model-b".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: 0,
                    output_tokens: 3,
                    total_tokens: 3,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            },
        )
        .expect("upsert second model usage");

        let view = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "today".to_string(),
                year: None,
            },
        )
        .expect("build wall");
        assert_eq!(view.view, "today");
        assert_eq!(view.hourly.len(), 24, "today 应固定 24 个整点格");
        assert_eq!(view.totals.conversation_count, 1);
        assert_eq!(view.totals.total_tokens, 7);
        let now_local = to_local_datetime(now_utc());
        let current_hour = now_local.hour();
        assert_eq!(
            view.hourly[current_hour as usize].total_tokens, 7,
            "当前小时格应聚合两个模型的 total"
        );
        assert_eq!(view.peak_hour, Some(current_hour as u8), "唯一有量小时应为峰值");
        let hour_models = &view.hourly[current_hour as usize].models;
        assert_eq!(hour_models.len(), 2, "两个模型应各自一条");
        let model_a = hour_models.iter().find(|m| m.model == "provider-a::model-a").expect("model-a 存在");
        let model_b = hour_models.iter().find(|m| m.model == "provider-a::model-b").expect("model-b 存在");
        assert_eq!(model_a.tokens, 4, "model-a total_tokens = 4");
        assert_eq!(model_b.tokens, 3, "model-b total_tokens = 3");
        assert_eq!(model_a.provider_label, "主供应商", "provider_label 应快照进模型条目");
        assert_eq!(model_b.provider_label, "主供应商", "provider_label 应快照进模型条目");
    }

    #[test]
    fn usage_trail_wall_should_split_same_model_name_across_providers() {
        let state = storage_usage_test_state();
        let bucket = message_store::usage_trail_hour_bucket(now_utc());
        let make_delta = |conversation_id: &str, provider_key: &str, provider_label: &str, input: u64| {
            message_store::UsageTrailDelta {
                conversation_id: conversation_id.to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: format!("{provider_key}::flash-model"),
                provider_key: provider_key.to_string(),
                provider_label: provider_label.to_string(),
                model_name: "flash-model".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: input,
                    output_tokens: 0,
                    total_tokens: input,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            }
        };
        // 同一小时、同一模型名，两个不同供应商各记一笔（不同会话）
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &bucket,
            &make_delta("conv-x", "provider-x", "供应商X", 100),
        )
        .expect("upsert provider-x");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &bucket,
            &make_delta("conv-y", "provider-y", "供应商Y", 200),
        )
        .expect("upsert provider-y");

        let view = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "today".to_string(),
                year: None,
            },
        )
        .expect("build wall");
        let now_local = to_local_datetime(now_utc());
        let current_hour = now_local.hour();
        let hour_models = &view.hourly[current_hour as usize].models;
        assert_eq!(hour_models.len(), 2, "同模型名不同供应商应各自一条");
        let model_x = hour_models
            .iter()
            .find(|m| m.model == "provider-x::flash-model")
            .expect("provider-x 条目存在");
        let model_y = hour_models
            .iter()
            .find(|m| m.model == "provider-y::flash-model")
            .expect("provider-y 条目存在");
        assert_eq!(model_x.tokens, 100, "provider-x tokens 独立");
        assert_eq!(model_y.tokens, 200, "provider-y tokens 独立");
        assert_eq!(model_x.provider_label, "供应商X", "provider-x label 不被串用");
        assert_eq!(model_y.provider_label, "供应商Y", "provider-y label 不被串用");
    }

    #[test]
    fn usage_trail_should_accumulate_same_hour_same_model_same_conversation() {
        let state = storage_usage_test_state();
        let bucket = message_store::usage_trail_hour_bucket(now_utc());
        let make_delta = |input: u64| message_store::UsageTrailDelta {
            conversation_id: "conv-acc".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            conversation_kind: "normal".to_string(),
            api_config_id: "provider-a::model-a".to_string(),
            provider_key: "provider-a".to_string(),
            provider_label: "主供应商".to_string(),
            model_name: "model-a".to_string(),
            tokens: message_store::UsageTrailTokenDelta {
                input_tokens: input,
                output_tokens: input,
                total_tokens: input * 2,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
            },
        };
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &bucket,
            &make_delta(10),
        )
        .expect("first upsert");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &bucket,
            &make_delta(20),
        )
        .expect("second upsert");

        let rows = message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)
            .expect("query usage trail");
        assert_eq!(rows.len(), 1, "同一小时同一会话同一模型应累加同一行");
        assert_eq!(rows[0].tokens.input_tokens, 30);
        assert_eq!(rows[0].tokens.total_tokens, 60);
    }

    #[test]
    fn usage_trail_should_split_rows_across_model_and_conversation() {
        let state = storage_usage_test_state();
        let bucket = message_store::usage_trail_hour_bucket(now_utc());
        let make_delta = |conversation_id: &str, model_name: &str, input: u64| {
            message_store::UsageTrailDelta {
                conversation_id: conversation_id.to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: model_name.to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: input,
                    output_tokens: 0,
                    total_tokens: input,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            }
        };
        for (conversation_id, model_name, input) in [
            ("conv-a", "model-a", 10),
            ("conv-a", "model-b", 5),
            ("conv-b", "model-a", 7),
        ] {
            message_store::chat_metadata_store_usage_trail_upsert_delta(
                &state.data_path,
                &bucket,
                &make_delta(conversation_id, model_name, input),
            )
            .expect("upsert usage trail");
        }

        let rows = message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)
            .expect("query usage trail");
        assert_eq!(rows.len(), 3, "跨模型/跨会话应分属不同行");
    }

    #[test]
    fn usage_trail_wall_should_filter_today_vs_history_year() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        let today_bucket = message_store::usage_trail_hour_bucket(now_utc());
        let make_delta = |conversation_id: &str, input: u64| message_store::UsageTrailDelta {
            conversation_id: conversation_id.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            conversation_kind: "normal".to_string(),
            api_config_id: "provider-a::model-a".to_string(),
            provider_key: "provider-a".to_string(),
            provider_label: "主供应商".to_string(),
            model_name: "model-a".to_string(),
            tokens: message_store::UsageTrailTokenDelta {
                input_tokens: input,
                output_tokens: 0,
                total_tokens: input,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
            },
        };
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &today_bucket,
            &make_delta("wall-conv-a", 10),
        )
        .expect("upsert today");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            "2020-01-01T10:00:00",
            &message_store::UsageTrailDelta {
                conversation_id: "wall-conv-a".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "model-a".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: 10,
                    output_tokens: 10,
                    total_tokens: 20,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            },
        )
        .expect("upsert historical");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            message_store::USAGE_TRAIL_EPOCH_BUCKET,
            &make_delta("wall-conv-b", 30),
        )
        .expect("upsert epoch");

        let view_today = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "today".to_string(),
                year: None,
            },
        )
        .expect("build today wall");
        assert_eq!(view_today.totals.conversation_count, 1, "today 只含今天会话");
        assert_eq!(view_today.totals.total_tokens, 10);
        assert_eq!(view_today.hourly.len(), 24, "today 固定 24 个整点格");
        let epoch_totals = view_today.epoch_totals.expect("epoch totals");
        assert_eq!(epoch_totals.total_tokens, 30, "epoch 历史累计独立返回");
        assert_eq!(view_today.top_conversation_label, None, "today 不计算 top 会话");

        let view_history = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "history".to_string(),
                year: Some("2020".to_string()),
            },
        )
        .expect("build history wall");
        assert_eq!(view_history.view, "history");
        assert_eq!(view_history.year, "2020");
        assert_eq!(view_history.totals.conversation_count, 1);
        assert_eq!(view_history.totals.total_tokens, 20);
        assert_eq!(view_history.calendar.len(), 366, "2020 为闰年应铺 366 天");
        let jan1 = view_history
            .calendar
            .iter()
            .find(|day| day.date == "2020-01-01")
            .expect("jan 1 day");
        assert_eq!(jan1.total_tokens, 20);
        assert_eq!(
            view_history.top_conversation_label.as_deref(),
            Some("已删除会话"),
            "会话不存在应降级为占位"
        );
        assert_eq!(
            view_history.years.contains(&"2020".to_string()),
            true,
            "年份列表应含历史年份"
        );
        assert_eq!(
            view_history.years.first().map(String::as_str),
            Some("2026"),
            "年份列表降序，最新年份在前"
        );
    }

    #[test]
    fn usage_overview_should_keep_deleted_conversation_totals() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 只写台账，不写会话缓存（模拟会话已删除）
        let delta = message_store::UsageTrailDelta {
            conversation_id: "deleted-conv".to_string(),
            agent_id: "agent-deleted".to_string(),
            conversation_kind: "normal".to_string(),
            api_config_id: "provider-a::model-a".to_string(),
            provider_key: "provider-a".to_string(),
            provider_label: "主供应商".to_string(),
            model_name: "model-a".to_string(),
            tokens: message_store::UsageTrailTokenDelta {
                input_tokens: 10,
                output_tokens: 5,
                total_tokens: 15,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
            },
        };
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &delta,
        )
        .expect("upsert deleted conversation usage");

        let overview = build_usage_overview(&state).expect("build usage overview");
        assert_eq!(overview.totals.conversation_count, 1);
        assert_eq!(overview.totals.total_tokens, 15, "会话删除后 totals 仍计入");
        let item = overview
            .conversations
            .iter()
            .find(|item| item.conversation_id == "deleted-conv")
            .expect("deleted conversation item");
        assert_eq!(item.title, "已删除会话", "明细应降级为已删除会话占位");
        assert_eq!(item.total_tokens, 15);
    }

    #[test]
    fn usage_trail_migration_should_be_idempotent() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            api_providers: vec![ApiProviderConfig {
                id: "provider-a".to_string(),
                name: "主供应商".to_string(),
                deprecated: true,
                models: vec![ApiModelConfig {
                    id: "model-a".to_string(),
                    model: "real-model-v1".to_string(),
                    deprecated: true,
                    ..ApiModelConfig::default()
                }],
                ..ApiProviderConfig::default()
            }],
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 先完成 v3 消息仓库迁移，使台账迁移满足 ready 前置
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("run v3 migration");

        // 写入带旧账本（by_provider_model）的会话元数据
        let mut conversation = storage_usage_test_conversation("migrate-legacy", "");
        conversation.preferred_api_config_id = Some("provider-a::model-a".to_string());
        conversation.cumulative_usage = ConversationCumulativeUsage {
            input_tokens: 11,
            output_tokens: 7,
            total_tokens: 18,
            by_provider_model: std::collections::BTreeMap::from([(
                "provider-a".to_string(),
                std::collections::BTreeMap::from([(
                    "real-model-v1".to_string(),
                    ConversationUsageBucket {
                        input_tokens: 11,
                        output_tokens: 7,
                        total_tokens: 18,
                        ..ConversationUsageBucket::default()
                    },
                )]),
            )]),
            ..ConversationCumulativeUsage::default()
        };
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        // 第一次迁移：写入 epoch 桶
        message_store::chat_metadata_store_run_usage_trail_migration(&state.data_path, &config, &[])
            .expect("first migration");
        let rows_after_first =
            message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)
                .expect("query after first");
        assert_eq!(rows_after_first.len(), 1, "首次迁移应写入一条 epoch 行");
        assert_eq!(rows_after_first[0].tokens.total_tokens, 18);
        assert_eq!(rows_after_first[0].tokens.input_tokens, 11);
        assert_eq!(rows_after_first[0].tokens.output_tokens, 7);

        // 第二次迁移：completed 标记已写入，不应重复累加
        message_store::chat_metadata_store_run_usage_trail_migration(&state.data_path, &config, &[])
            .expect("second migration");
        let rows_after_second =
            message_store::chat_metadata_store_usage_trail_query(&state.data_path, None)
                .expect("query after second");
        assert_eq!(rows_after_second.len(), 1, "重复迁移不应新增行");
        assert_eq!(
            rows_after_second[0].tokens.total_tokens, 18,
            "重复迁移不应再次累加 epoch 行"
        );
        assert_eq!(rows_after_second[0].tokens.input_tokens, 11);
        assert_eq!(rows_after_second[0].tokens.output_tokens, 7);
    }

    #[test]
    fn usage_trail_wall_history_top_conversation_label_should_fallback_chain() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 会话存在但 title 为空：top 会话 label 应降级为未命名会话，而不是已删除会话
        let mut conversation = storage_usage_test_conversation("wall-top-conv", "");
        conversation.title = String::new();
        state_write_conversation_cached(&state, &conversation).expect("write conversation");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            "2021-03-05T09:00:00",
            &message_store::UsageTrailDelta {
                conversation_id: conversation.id.clone(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "model-a".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: 5,
                    output_tokens: 5,
                    total_tokens: 10,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            },
        )
        .expect("upsert history");

        let view = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "history".to_string(),
                year: Some("2021".to_string()),
            },
        )
        .expect("build history wall");
        assert_eq!(
            view.top_conversation_label.as_deref(),
            Some("未命名会话"),
            "title 为空且无 summary 应降级为未命名会话"
        );
        assert_eq!(view.top_conversation_percent, Some(100));
        assert_eq!(
            view.active_period_label.as_deref(),
            Some("morning"),
            "09:00 应归为上午时段"
        );
    }

    #[test]
    fn usage_trail_wall_should_keep_epoch_totals_separate_from_today() {
        let state = storage_usage_test_state();
        let config = AppConfig {
            ..AppConfig::default()
        };
        state_write_config_cached(&state, &config).expect("write config");

        // 同一会话在小时桶与 epoch 桶都出现，today 总量只计小时桶，epoch 独立返回
        let make_delta = |_bucket: &str, conversation_id: &str, input: u64| {
            message_store::UsageTrailDelta {
                conversation_id: conversation_id.to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_kind: "normal".to_string(),
                api_config_id: "provider-a::model-a".to_string(),
                provider_key: "provider-a".to_string(),
                provider_label: "主供应商".to_string(),
                model_name: "model-a".to_string(),
                tokens: message_store::UsageTrailTokenDelta {
                    input_tokens: input,
                    output_tokens: 0,
                    total_tokens: input,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    reasoning_tokens: 0,
                },
            }
        };
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            &message_store::usage_trail_hour_bucket(now_utc()),
            &make_delta("", "shared-conv", 10),
        )
        .expect("upsert today");
        message_store::chat_metadata_store_usage_trail_upsert_delta(
            &state.data_path,
            message_store::USAGE_TRAIL_EPOCH_BUCKET,
            &make_delta("", "shared-conv", 20),
        )
        .expect("upsert epoch");

        let view = build_usage_trail_wall(
            &state,
            &UsageTrailWallQuery {
                view: "today".to_string(),
                year: None,
            },
        )
        .expect("build today wall");
        assert_eq!(
            view.totals.conversation_count, 1,
            "today 只统计小时桶会话"
        );
        assert_eq!(view.totals.total_tokens, 10, "today 不含 epoch 桶");
        let epoch_totals = view.epoch_totals.expect("epoch totals");
        assert_eq!(epoch_totals.total_tokens, 20, "epoch 历史累计独立返回");
    }

}