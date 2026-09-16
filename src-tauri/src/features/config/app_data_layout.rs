const LAYOUT_DIR_CONFIG: &str = "config";
const LAYOUT_DIR_STATE: &str = "state";
const LAYOUT_DIR_CHAT: &str = "chat";
const LAYOUT_DIR_CHAT_CONVERSATIONS: &str = "conversations";
const LAYOUT_DIR_BACKUPS: &str = "backups";
const LAYOUT_FILE_AGENTS: &str = "agents.json";
const LAYOUT_FILE_RUNTIME: &str = "runtime_state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    #[serde(default)]
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatIndexConversationItem {
    id: String,
    updated_at: String,
    status: String,
    #[serde(default)]
    archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ChatIndexFile {
    #[serde(default)]
    conversations: Vec<ChatIndexConversationItem>,
}

fn app_layout_config_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_CONFIG)
}

fn app_layout_state_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_STATE)
}

fn app_layout_chat_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_CHAT)
}

fn app_layout_chat_conversations_dir(path: &PathBuf) -> PathBuf {
    app_layout_chat_dir(path).join(LAYOUT_DIR_CHAT_CONVERSATIONS)
}

fn app_layout_backups_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_BACKUPS)
}

fn app_layout_agents_path(path: &PathBuf) -> PathBuf {
    app_layout_config_dir(path).join(LAYOUT_FILE_AGENTS)
}

fn app_layout_runtime_state_path(path: &PathBuf) -> PathBuf {
    app_layout_state_dir(path).join(LAYOUT_FILE_RUNTIME)
}

fn app_layout_chat_conversation_path(path: &PathBuf, conversation_id: &str) -> PathBuf {
    app_layout_chat_conversations_dir(path).join(format!("{conversation_id}.json"))
}

fn build_agents_file(agents: &[AgentProfile]) -> AgentsFile {
    AgentsFile {
        agents: agents.to_vec(),
    }
}

fn system_notification_conversation_shard_has_artifacts(path: &PathBuf) -> Result<bool, String> {
    let store_paths = message_store::message_store_paths(path, SYSTEM_NOTIFICATION_CONVERSATION_ID)?;
    message_store::chat_store_read_status(&store_paths).map(|status| status.is_some())
}

fn ensure_system_notification_conversation_shard(path: &PathBuf) -> Result<bool, String> {
    match read_conversation_shard(path, SYSTEM_NOTIFICATION_CONVERSATION_ID) {
        Ok(mut conversation) => {
            if normalize_system_notification_conversation(&mut conversation) {
                return write_conversation_shard(path, &conversation);
            }
            Ok(false)
        }
        Err(err) => {
            if system_notification_conversation_shard_has_artifacts(path)? {
                runtime_log_warn(format!(
                    "[系统通知会话] 跳过，任务=确保固定会话分片，原因=固定会话分片已存在但暂不可读，conversation_id={}，error={}",
                    SYSTEM_NOTIFICATION_CONVERSATION_ID,
                    err
                ));
                return Ok(false);
            }
            let conversation = build_system_notification_conversation_record();
            write_conversation_shard(path, &conversation)
        }
    }
}

fn build_chat_index_item(conversation: &Conversation) -> ChatIndexConversationItem {
    ChatIndexConversationItem {
        id: conversation.id.clone(),
        updated_at: conversation.updated_at.clone(),
        status: conversation.status.clone(),
        archived_at: conversation.archived_at.clone(),
    }
}

fn chat_index_item_is_archived(item: &ChatIndexConversationItem) -> bool {
    if item.status.trim() == "archived" {
        return true;
    }
    item.archived_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

#[cfg(test)]
fn build_chat_index_file(conversations: &[Conversation]) -> ChatIndexFile {
    ChatIndexFile {
        conversations: conversations
            .iter()
            .map(build_chat_index_item)
            .collect::<Vec<_>>(),
    }
}

fn upsert_chat_index_conversation(index: &mut ChatIndexFile, conversation: &Conversation) {
    let next = build_chat_index_item(conversation);
    if let Some(existing) = index
        .conversations
        .iter_mut()
        .find(|item| item.id == conversation.id)
    {
        *existing = next;
    } else {
        index.conversations.push(next);
    }
}

#[allow(dead_code)]
fn remove_chat_index_conversation(index: &mut ChatIndexFile, conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    index.conversations.retain(|item| item.id != conversation_id);
}

fn read_agents_shard(path: &PathBuf) -> Result<Vec<AgentProfile>, String> {
    if app_layout_agents_path(path).exists() {
        Ok(read_json_file::<AgentsFile>(&app_layout_agents_path(path), "agents file")?.agents)
    } else {
        Ok(AppData::default().agents)
    }
}

fn write_agents_shard(path: &PathBuf, agents: &[AgentProfile]) -> Result<bool, String> {
    fs::create_dir_all(app_layout_config_dir(path))
        .map_err(|err| format!("Create config layout dir failed: {err}"))?;
    write_json_file_atomic_if_changed(
        &app_layout_agents_path(path),
        &build_agents_file(agents),
        "agents file",
    )
}

fn read_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<Conversation, String> {
    let mut conversation = read_conversation_shard_raw(path, conversation_id)?;
    normalize_conversation_runtime_volatile_fields(&mut conversation);
    Ok(conversation)
}

fn read_conversation_meta_shard(
    path: &PathBuf,
    conversation_id: &str,
) -> Result<message_store::ConversationShardMeta, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    match message_store::chat_store_read_meta(&store_paths) {
        Ok(Some(meta))
            if meta.schema_version() >= message_store::CONVERSATION_META_SCHEMA_VERSION =>
        {
            if meta.cumulative_usage().needs_legacy_total_tokens_backfill() {
                let repaired = meta.clone().normalized_legacy_usage_totals();
                write_conversation_meta_shard_from_meta(path, &repaired)?;
                return Ok(repaired);
            }
            return Ok(meta);
        }
        Ok(Some(_)) | Ok(None) | Err(_) => {}
    }
    Err(format!("Conversation '{conversation_id}' not found."))
}

fn refresh_conversation_meta_shard_if_needed(
    path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    if let Ok(Some(meta)) = message_store::chat_store_read_meta(&store_paths) {
        if meta.schema_version() >= message_store::CONVERSATION_META_SCHEMA_VERSION {
            return Ok(false);
        }
    }
    if message_store::chat_store_read_status(&store_paths)?.is_none() {
        return Ok(false);
    }
    let _ = read_conversation_meta_shard(path, conversation_id)?;
    Ok(true)
}

fn read_conversation_shard_raw(path: &PathBuf, conversation_id: &str) -> Result<Conversation, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    if let Some(conversation) =
        message_store::chat_store_read_conversation(&store_paths)?
    {
        if conversation
            .cumulative_usage
            .needs_legacy_total_tokens_backfill()
        {
            let mut repaired = conversation;
            repaired.cumulative_usage = repaired.cumulative_usage.clone().normalized_legacy_totals();
            let _ = write_conversation_shard(path, &repaired)?;
            return Ok(repaired);
        }
        return Ok(conversation);
    }
    Err(format!("Conversation '{conversation_id}' not found."))
}

fn write_conversation_shard(path: &PathBuf, conversation: &Conversation) -> Result<bool, String> {
    fs::create_dir_all(app_layout_chat_conversations_dir(path))
        .map_err(|err| format!("Create chat conversations dir failed: {err}"))?;
    let store_paths = message_store::message_store_paths(path, &conversation.id)?;
    if message_store::chat_store_read_meta(&store_paths)?.is_some() {
        // v3 的正文只能由 append/replace/truncate/splice 原子接口发布。
        // 后台 metadata 刷新不得整读或重建 locator 与 JSONL block。
        write_conversation_meta_shard_from_meta(
            path,
            &message_store::ConversationShardMeta::from_conversation(conversation),
        )?;
        return Ok(true);
    }
    message_store::chat_store_write_snapshot(&store_paths, conversation)?;
    Ok(true)
}

fn write_conversation_meta_shard_from_meta(
    path: &PathBuf,
    meta: &message_store::ConversationShardMeta,
) -> Result<(), String> {
    let paths = message_store::message_store_paths(path, meta.id())?;
    let mut meta_to_persist = meta.clone();
    if let Some(ready_meta) = message_store::chat_store_read_meta(&paths)? {
        meta_to_persist.preserve_message_derived_fields_from(&ready_meta);
    }
    let persist_meta = meta_to_persist.to_persist_meta();
    message_store::chat_store_write_meta(&paths, &persist_meta)
}

fn delete_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    message_store::delete_message_store_shard_artifacts(&store_paths)
}

fn read_json_file<T>(path: &PathBuf, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|err| format!("Read app_data failed: {err}"))?;
    serde_json::from_str::<T>(&content).map_err(|err| {
        runtime_log_error(format!("[配置] 解析{label}失败 ({}): {err}", path.display()));
        format!("Parse {label} failed ({}): {err}", path.display())
    })
}

fn file_metadata_signature(path: &PathBuf) -> (u64, Option<std::time::SystemTime>) {
    match fs::metadata(path) {
        Ok(metadata) => (metadata.len(), metadata.modified().ok()),
        Err(_) => (0, None),
    }
}

fn app_data_cache_signature(path: &PathBuf) -> AppDataCacheSignature {
    let agents_path = app_layout_agents_path(path);
    let runtime_path = app_layout_runtime_state_path(path);
    let (agents_len, agents_modified) = file_metadata_signature(&agents_path);
    let (runtime_len, runtime_modified) = file_metadata_signature(&runtime_path);
    // 普通运行时缓存只随 V3 current 数据库变化；不能用 V1 JSON 或 V2
    // manifest/meta/index/blocks 的文件状态驱动缓存，从而间接恢复旧格式。
    let chat_metadata_path = message_store::chat_metadata_store_db_path(path);
    let (chat_metadata_len, chat_metadata_modified) =
        file_metadata_signature(&chat_metadata_path);
    let conversations = ConversationDirCacheSignature {
        file_count: u64::from(chat_metadata_modified.is_some()),
        total_size: chat_metadata_len,
        latest_file_name: "chat_metadata.sqlite".to_string(),
        latest_modified: chat_metadata_modified,
    };

    AppDataCacheSignature {
        agents_len,
        agents_modified,
        runtime_len,
        runtime_modified,
        conversations,
    }
}

#[cfg(test)]
fn write_json_file_atomic<T>(path: &PathBuf, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize,
{
    ensure_parent_dir(path)?;
    let body = serde_json::to_vec_pretty(value).map_err(|err| format!("Serialize {label} failed: {err}"))?;
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid {label} file path"))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, body).map_err(|err| format!("Write temp {label} failed: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp, path) {
        fs::copy(&tmp, path).map_err(|copy_err| {
            format!(
                "Finalize {label} failed (rename: {rename_err}; copy: {copy_err})"
            )
        })?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

fn write_json_file_atomic_if_changed<T>(
    path: &PathBuf,
    value: &T,
    label: &str,
) -> Result<bool, String>
where
    T: Serialize,
{
    ensure_parent_dir(path)?;
    let body = serde_json::to_vec_pretty(value).map_err(|err| format!("Serialize {label} failed: {err}"))?;
    if let Ok(existing) = fs::read(path) {
        if existing == body {
            return Ok(false);
        }
    }
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid {label} file path"))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, body).map_err(|err| format!("Write temp {label} failed: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp, path) {
        fs::copy(&tmp, path).map_err(|copy_err| {
            format!(
                "Finalize {label} failed (rename: {rename_err}; copy: {copy_err})"
            )
        })?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(true)
}

#[cfg(test)]
fn read_layout_app_data(path: &PathBuf) -> Result<AppData, String> {
    let agents = if app_layout_agents_path(path).exists() {
        read_json_file::<AgentsFile>(&app_layout_agents_path(path), "agents file")?.agents
    } else {
        AppData::default().agents
    };

    let data_migration_version =
        state_db_get_kv(path, "data_migration_version")?
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(0);

    Ok(AppData {
        version: APP_DATA_SCHEMA_VERSION,
        data_migration_version,
        agents,
        user_alias: default_user_alias(),
        conversations: Vec::new(),
    })
}

// ========== 数据迁移 registry ==========
//
// v2+ 需要显式上下文，避免在只有 app_data 路径时猜测助理空间位置或名称。
struct DataMigrationContext<'a> {
    state: &'a AppState,
    config: &'a AppConfig,
}

#[derive(Debug, Default, Clone, Copy)]
struct DataMigrationStepStats {
    data_changed: bool,
    conversation_writes: usize,
}

struct DataMigrationStep {
    version: u32,
    name: &'static str,
    run: for<'a> fn(&DataMigrationContext<'a>) -> Result<DataMigrationStepStats, String>,
}

fn data_migration_steps() -> Vec<DataMigrationStep> {
    vec![
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES,
            name: "v2_assistant_workspace_for_empty_shell_workspaces",
            run: migrate_empty_shell_workspaces_to_assistant_workspace,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V5_DEPARTMENTS_TO_AGENT_ORGANIZATION,
            name: "v5_departments_to_agent_organization",
            run: migrate_departments_into_agent_organization,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V6_AVATAR_PATH_RELATIVE,
            name: "v6_avatar_path_relative",
            run: migrate_avatar_paths_to_relative,
        },
    ]
}

/// V6 迁移步骤：把 `agents.json` 里的绝对头像路径归一为相对数据根的相对路径。
/// - 已是相对路径：跳过（幂等）。
/// - 绝对路径且当前 avatars 目录存在同名文件：改写为 `avatars/<文件名>`。
/// - 找不到同名文件：保留原值并告警，不做无声抹除。
fn migrate_avatar_paths_to_relative(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let mut stats = DataMigrationStepStats::default();
    let mut agents = read_agents_shard(&context.state.data_path)?;
    let avatars_dir = avatar_storage_dir(context.state)?;
    let mut changed = false;
    let mut rewritten = 0usize;
    let mut kept = 0usize;
    for agent in agents.iter_mut() {
        let Some(raw) = agent
            .avatar_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let candidate = PathBuf::from(raw);
        if !candidate.is_absolute() {
            continue;
        }
        let file_name = candidate
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !file_name.is_empty() && avatars_dir.join(file_name).is_file() {
            agent.avatar_path = Some(avatar_relative_path(file_name));
            changed = true;
            rewritten += 1;
        } else {
            kept += 1;
            runtime_log_warn(format!(
                "[应用数据迁移] 头像路径无法归一，任务=v6头像相对路径，人格={}，原值={}，原因=当前头像目录未找到同名文件",
                agent.id, raw
            ));
        }
    }
    if changed {
        write_agents_shard(&context.state.data_path, &agents)?;
        stats.data_changed = true;
    }
    runtime_log_info(format!(
        "[应用数据迁移] 头像路径迁移完成，任务=v6头像相对路径，改写={rewritten}，保留={kept}"
    ));
    Ok(stats)
}

fn conversation_shell_workspace_path_key(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        String::new()
    } else {
        normalize_terminal_path_for_compare(&PathBuf::from(path))
    }
}

fn legacy_shell_workspace_path_as_main_workspace(
    state: &AppState,
    path: &str,
) -> Option<ShellWorkspaceConfig> {
    let raw = ShellWorkspaceConfig {
        id: "legacy-main-workspace".to_string(),
        name: String::new(),
        path: path.trim().to_string(),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
        built_in: false,
    };
    let candidate = shell_workspace_resolve_path_candidate(state, &raw)?;
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    normalize_conversation_shell_workspaces(
        state,
        &[ShellWorkspaceConfig {
            path: terminal_path_for_user(&canonical),
            ..raw
        }],
    )
    .into_iter()
    .next()
}

fn state_write_conversation_shell_workspace_metadata_direct(
    state: &AppState,
    conversation_id: &str,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    let conversation_meta = state_read_conversation_metadata_cached(state, conversation_id)?;
    let mut conversation =
        conversation_service_v2().build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
    let original_path = conversation.shell_workspace_path.clone();
    let original_workspaces = conversation.shell_workspaces.clone();
    conversation.shell_workspace_path = None;
    conversation.shell_workspaces = shell_workspaces;
    if conversation.shell_workspace_path == original_path
        && conversation.shell_workspaces == original_workspaces
    {
        return Ok(false);
    }
    let mut updated_meta = message_store::ConversationShardMeta::from_conversation(&conversation);
    updated_meta.preserve_message_derived_fields_from(&conversation_meta);
    write_conversation_meta_shard_from_meta(&state.data_path, &updated_meta)?;
    let _ = state_mark_conversation_metadata_direct_persisted(state, conversation_id)?;
    Ok(true)
}

fn shell_workspaces_for_empty_conversation_workspace_migration(
    state: &AppState,
    config: &AppConfig,
    conversation: &Conversation,
) -> Option<Vec<ShellWorkspaceConfig>> {
    let normalized = normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
    if !normalized.is_empty() {
        return None;
    }
    if let Some(legacy_workspace) = conversation
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|path| legacy_shell_workspace_path_as_main_workspace(state, path))
    {
        return Some(vec![legacy_workspace]);
    }
    Some(vec![assistant_workspace_as_conversation_main_workspace(
        state, config,
    )])
}

fn migrate_empty_shell_workspaces_to_assistant_workspace(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let chat_index = collect_chat_index_items_from_storage(&context.state.data_path)?;
    let mut stats = DataMigrationStepStats::default();
    for item in chat_index {
        let conversation_id = item.id.trim();
        if conversation_id.is_empty() {
            continue;
        }
        let conversation_meta =
            match state_read_conversation_metadata_cached(context.state, conversation_id) {
                Ok(meta) => meta,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[应用数据迁移] 跳过，任务=v2补齐会话工作区，conversation_id={}，error={}",
                        conversation_id, err
                    ));
                    continue;
                }
            };
        let conversation = conversation_service_v2()
            .build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
        if !conversation_visible_in_foreground_lists(&conversation)
            || !conversation_is_local_normal_chat(&conversation)
            || !conversation_is_unarchived(&conversation)
        {
            continue;
        }
        let Some(shell_workspaces) = shell_workspaces_for_empty_conversation_workspace_migration(
            context.state,
            context.config,
            &conversation,
        ) else {
            continue;
        };
        if state_write_conversation_shell_workspace_metadata_direct(
            context.state,
            conversation_id,
            shell_workspaces,
        )? {
            stats.data_changed = true;
            stats.conversation_writes += 1;
        }
    }
    Ok(stats)
}

fn run_app_data_migrations_with_state(
    state: &AppState,
    config: &AppConfig,
) -> Result<bool, String> {
    require_message_store_migration_completed_for_runtime(state, "执行应用数据迁移")?;
    let mut migration_version = state_service_get_data_migration_version(state)?;
    if migration_version >= DATA_MIGRATION_CURRENT_VERSION {
        return Ok(false);
    }
    let migration_version_before = migration_version;
    let mut any_data_changed = false;
    for step in data_migration_steps() {
        if migration_version >= step.version {
            continue;
        }
        let started = std::time::Instant::now();
        let stats = (step.run)(&DataMigrationContext { state, config })?;
        migration_version = step.version;
        any_data_changed |= stats.data_changed;
        runtime_log_info(format!(
            "[应用数据迁移] 完成，任务={}，migration_version_before={}，migration_version_after={}，data_changed={}，conversation_writes={}，duration_ms={}",
            step.name,
            migration_version_before,
            migration_version,
            stats.data_changed,
            stats.conversation_writes,
            started.elapsed().as_millis()
        ));
    }
    if migration_version < DATA_MIGRATION_CURRENT_VERSION {
        migration_version = DATA_MIGRATION_CURRENT_VERSION;
    }
    state_service_set_data_migration_version(state, migration_version)?;
    Ok(any_data_changed || migration_version_before != migration_version)
}

fn assistant_workspace_label_sync_target_keys(
    state: &AppState,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> std::collections::HashSet<String> {
    let mut previous = previous_config.clone();
    let mut next = next_config.clone();
    let _ = ensure_default_shell_workspace_in_config(&mut previous, state);
    let _ = ensure_default_shell_workspace_in_config(&mut next, state);
    [
        assistant_workspace_as_conversation_main_workspace(state, &previous),
        assistant_workspace_as_conversation_main_workspace(state, &next),
    ]
    .into_iter()
    .map(|workspace| conversation_shell_workspace_path_key(&workspace.path))
    .filter(|key| !key.is_empty())
    .collect()
}

fn sync_assistant_workspace_label_for_unarchived_conversations(
    state: &AppState,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> Result<usize, String> {
    let target_keys =
        assistant_workspace_label_sync_target_keys(state, previous_config, next_config);
    if target_keys.is_empty() {
        return Ok(0);
    }
    let assistant_workspace =
        assistant_workspace_as_conversation_main_workspace(state, next_config);
    let chat_index = collect_chat_index_items_from_storage(&state.data_path)?;
    let mut changed = 0usize;
    for item in chat_index {
        let conversation_id = item.id.trim();
        if conversation_id.is_empty() {
            continue;
        }
        let conversation_meta = match state_read_conversation_metadata_cached(state, conversation_id)
        {
            Ok(meta) => meta,
            Err(err) => {
                runtime_log_warn(format!(
                    "[终端工作空间] 跳过，任务=同步助理空间会话标签，conversation_id={}，error={}",
                    conversation_id, err
                ));
                continue;
            }
        };
        let conversation = conversation_service_v2()
            .build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
        if !conversation_visible_in_foreground_lists(&conversation)
            || !conversation_is_local_normal_chat(&conversation)
            || !conversation_is_unarchived(&conversation)
        {
            continue;
        }
        let mut workspaces =
            normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
        if workspaces.is_empty() {
            workspaces = vec![assistant_workspace.clone()];
        } else {
            let main_index = workspaces
                .iter()
                .position(|workspace| {
                    normalize_shell_workspace_level_text(&workspace.level)
                        == SHELL_WORKSPACE_LEVEL_MAIN
                })
                .unwrap_or(0);
            let key = conversation_shell_workspace_path_key(&workspaces[main_index].path);
            if !target_keys.contains(&key) {
                continue;
            }
            let mut synced = workspaces[main_index].clone();
            synced.name = assistant_workspace.name.clone();
            synced.path = assistant_workspace.path.clone();
            synced.level = SHELL_WORKSPACE_LEVEL_MAIN.to_string();
            synced.built_in = false;
            if normalize_shell_workspace_access_text(&synced.access).is_empty() {
                synced.access = assistant_workspace.access.clone();
            }
            workspaces[main_index] = synced;
        }
        if state_write_conversation_shell_workspace_metadata_direct(
            state,
            conversation_id,
            workspaces,
        )? {
            changed += 1;
        }
    }
    if changed > 0 {
        runtime_log_info(format!(
            "[终端工作空间] 完成，任务=同步助理空间会话标签，conversation_count={}",
            changed
        ));
    }
    Ok(changed)
}

fn normalize_conversation_runtime_volatile_fields(conversation: &mut Conversation) {
    let _ = fill_missing_conversation_message_speaker_agent_ids(conversation);
    let _ = cleanup_legacy_summary_context_messages(conversation);
}

/// 测试专用：从聚合 AppData 结构 seed 分片布局（agents.json + conversation shards + 系统通知会话）。
/// 替代已删除的兼容层全量写入器，供测试构造使用。
#[cfg(test)]
fn seed_app_data_shards(path: &PathBuf, data: &AppData) -> Result<(), String> {
    write_agents_shard(path, &data.agents)?;
    if data.data_migration_version > 0 {
        state_db_upsert_kv(
            path,
            "data_migration_version",
            &data.data_migration_version.to_string(),
        )?;
    }
    for conv in &data.conversations {
        write_conversation_shard(path, conv)?;
    }
    ensure_system_notification_conversation_shard(path)?;
    Ok(())
}
