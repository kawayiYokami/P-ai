fn migrate_app_data_inline_media_to_refs(data_path: &PathBuf, data: &mut AppData) -> bool {
    let mut changed = false;
    for conversation in &mut data.conversations {
        for message in &mut conversation.messages {
            changed |= externalize_message_parts_to_media_refs_lossy(&mut message.parts, data_path);
        }
    }
    for archive in &mut data.archived_conversations {
        for message in &mut archive.source_conversation.messages {
            changed |= externalize_message_parts_to_media_refs_lossy(&mut message.parts, data_path);
        }
    }
    changed
}

fn migrate_app_data_archives_into_conversations(
    data_path: &PathBuf,
    data: &mut AppData,
) -> Result<bool, String> {
    if data.archived_conversations.is_empty() {
        return Ok(false);
    }
    let backup_file = app_layout_backups_dir(data_path).join(format!(
        "app_data.pre_archive_merge.{}.json",
        now_utc().unix_timestamp()
    ));
    write_json_file_atomic(&backup_file, data, "pre-migration app_data backup")?;

    for archive in data.archived_conversations.clone() {
        let mut conv = archive.source_conversation;
        if conv.id.trim().is_empty() {
            conv.id = Uuid::new_v4().to_string();
        }
        if conv.archived_at.as_deref().unwrap_or("").trim().is_empty() {
            conv.archived_at = Some(archive.archived_at.clone());
        }
        if conv.status.trim() != "archived" {
            conv.status = "archived".to_string();
        }

        if let Some(existing_idx) = data.conversations.iter().position(|c| c.id == conv.id) {
            let should_replace = {
                let existing = &data.conversations[existing_idx];
                existing.summary.trim().is_empty() && !conv.summary.trim().is_empty()
            };
            if should_replace {
                data.conversations[existing_idx] = conv;
            }
        } else {
            data.conversations.push(conv);
        }
    }

    data.archived_conversations.clear();
    Ok(true)
}

fn migrate_agent_avatar_paths(data_path: &PathBuf, data: &mut AppData) -> bool {
    let root = app_root_from_data_path(data_path);
    let new_avatar_dir = root.join("avatars");
    let legacy_avatar_dir = root.join("config").join("avatars");
    let mut changed = false;

    for agent in &mut data.agents {
        let Some(path_raw) = agent.avatar_path.as_ref() else {
            continue;
        };
        if path_raw.trim().is_empty() {
            continue;
        }
        let old_path = PathBuf::from(path_raw);
        let file_name = old_path
            .file_name()
            .map(|v| v.to_owned())
            .or_else(|| {
                PathBuf::from(path_raw)
                    .components()
                    .last()
                    .map(|c| std::ffi::OsString::from(c.as_os_str()))
            });
        let Some(file_name) = file_name else {
            continue;
        };
        let new_path = new_avatar_dir.join(file_name);

        if new_path.exists() {
            let next = new_path.to_string_lossy().to_string();
            if next != *path_raw {
                agent.avatar_path = Some(next);
                changed = true;
            }
            continue;
        }

        let legacy_candidate = if old_path.exists() {
            old_path.clone()
        } else {
            legacy_avatar_dir.join(
                old_path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("")),
            )
        };
        if !legacy_candidate.exists() {
            continue;
        }
        let _ = fs::create_dir_all(&new_avatar_dir);
        if fs::rename(&legacy_candidate, &new_path).is_err() {
            if fs::copy(&legacy_candidate, &new_path).is_ok() {
                let _ = fs::remove_file(&legacy_candidate);
            }
        }
        if new_path.exists() {
            let next = new_path.to_string_lossy().to_string();
            if next != *path_raw {
                agent.avatar_path = Some(next);
                changed = true;
            }
        }
    }

    changed
}

const LEGACY_APP_DATA_SPLIT_DIR_NAME: &str = "app_data";
const LEGACY_APP_DATA_PROFILE_FILE_NAME: &str = "profile.json";
const LEGACY_APP_DATA_CONVERSATIONS_FILE_NAME: &str = "conversations.json";
const LEGACY_APP_DATA_IMAGE_CACHE_FILE_NAME: &str = "image_text_cache.json";

const LAYOUT_DIR_CONFIG: &str = "config";
const LAYOUT_DIR_STATE: &str = "state";
const LAYOUT_DIR_CHAT: &str = "chat";
const LAYOUT_DIR_CHAT_CONVERSATIONS: &str = "conversations";
const LAYOUT_DIR_BACKUPS: &str = "backups";
const LAYOUT_FILE_AGENTS: &str = "agents.json";
const LAYOUT_FILE_RUNTIME: &str = "runtime_state.json";
const LAYOUT_FILE_CHAT_INDEX: &str = "index.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    #[serde(default)]
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStateFile {
    version: u32,
    #[serde(alias = "selectedAgentId", alias = "selected_agent_id")]
    assistant_department_agent_id: String,
    user_alias: String,
    response_style_id: String,
    #[serde(default = "default_pdf_read_mode")]
    pdf_read_mode: String,
    #[serde(default = "default_background_voice_screenshot_keywords")]
    background_voice_screenshot_keywords: String,
    #[serde(default = "default_background_voice_screenshot_mode")]
    background_voice_screenshot_mode: String,
    #[serde(default)]
    instruction_presets: Vec<PromptCommandPreset>,
    #[serde(default)]
    main_conversation_id: Option<String>,
    #[serde(default)]
    pinned_conversation_ids: Vec<String>,
    #[serde(default)]
    image_text_cache: Vec<ImageTextCacheEntry>,
    #[serde(default)]
    pdf_text_cache: Vec<PdfTextCacheEntry>,
    #[serde(default)]
    pdf_image_cache: Vec<PdfImageCacheEntry>,
    #[serde(default)]
    remote_im_contacts: Vec<RemoteImContact>,
    #[serde(default)]
    remote_im_contact_checkpoints: Vec<RemoteImContactCheckpoint>,
}

impl Default for RuntimeStateFile {
    fn default() -> Self {
        Self {
            version: APP_DATA_SCHEMA_VERSION,
            assistant_department_agent_id: default_assistant_department_agent_id(),
            user_alias: default_user_alias(),
            response_style_id: default_response_style_id(),
            pdf_read_mode: default_pdf_read_mode(),
            background_voice_screenshot_keywords: default_background_voice_screenshot_keywords(),
            background_voice_screenshot_mode: default_background_voice_screenshot_mode(),
            instruction_presets: Vec::new(),
            main_conversation_id: None,
            pinned_conversation_ids: Vec::new(),
            image_text_cache: Vec::new(),
            pdf_text_cache: Vec::new(),
            pdf_image_cache: Vec::new(),
            remote_im_contacts: Vec::new(),
            remote_im_contact_checkpoints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatIndexConversationItem {
    id: String,
    updated_at: String,
    status: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ChatIndexFile {
    #[serde(default)]
    conversations: Vec<ChatIndexConversationItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AppDataWriteStats {
    agents_written: bool,
    runtime_written: bool,
    chat_index_written: bool,
    conversation_writes: usize,
    conversation_deletes: usize,
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

fn app_layout_chat_index_path(path: &PathBuf) -> PathBuf {
    app_layout_chat_dir(path).join(LAYOUT_FILE_CHAT_INDEX)
}

fn app_layout_chat_conversation_path(path: &PathBuf, conversation_id: &str) -> PathBuf {
    app_layout_chat_conversations_dir(path).join(format!("{conversation_id}.json"))
}

fn build_agents_file(agents: &[AgentProfile]) -> AgentsFile {
    AgentsFile {
        agents: agents.to_vec(),
    }
}

fn build_runtime_state_file(data: &AppData) -> RuntimeStateFile {
    RuntimeStateFile {
        version: APP_DATA_SCHEMA_VERSION,
        assistant_department_agent_id: data.assistant_department_agent_id.clone(),
        user_alias: data.user_alias.clone(),
        response_style_id: data.response_style_id.clone(),
        pdf_read_mode: data.pdf_read_mode.clone(),
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords.clone(),
        background_voice_screenshot_mode: data.background_voice_screenshot_mode.clone(),
        instruction_presets: data.instruction_presets.clone(),
        main_conversation_id: data.main_conversation_id.clone(),
        pinned_conversation_ids: data.pinned_conversation_ids.clone(),
        image_text_cache: data.image_text_cache.clone(),
        pdf_text_cache: data.pdf_text_cache.clone(),
        pdf_image_cache: data.pdf_image_cache.clone(),
        remote_im_contacts: data.remote_im_contacts.clone(),
        remote_im_contact_checkpoints: data.remote_im_contact_checkpoints.clone(),
    }
}

fn build_chat_index_item(conversation: &Conversation) -> ChatIndexConversationItem {
    ChatIndexConversationItem {
        id: conversation.id.clone(),
        updated_at: conversation.updated_at.clone(),
        status: conversation.status.clone(),
        summary: conversation.summary.clone(),
        archived_at: conversation.archived_at.clone(),
    }
}

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

#[cfg(test)]
fn remove_chat_index_conversation(index: &mut ChatIndexFile, conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    index.conversations.retain(|item| item.id != conversation_id);
}

fn apply_runtime_state_to_app_data(data: &mut AppData, runtime: &RuntimeStateFile) {
    data.version = runtime.version;
    data.assistant_department_agent_id = runtime.assistant_department_agent_id.clone();
    data.user_alias = runtime.user_alias.clone();
    data.response_style_id = runtime.response_style_id.clone();
    data.pdf_read_mode = runtime.pdf_read_mode.clone();
    data.background_voice_screenshot_keywords =
        runtime.background_voice_screenshot_keywords.clone();
    data.background_voice_screenshot_mode = runtime.background_voice_screenshot_mode.clone();
    data.instruction_presets = runtime.instruction_presets.clone();
    data.main_conversation_id = runtime.main_conversation_id.clone();
    data.pinned_conversation_ids = runtime.pinned_conversation_ids.clone();
    data.image_text_cache = runtime.image_text_cache.clone();
    data.pdf_text_cache = runtime.pdf_text_cache.clone();
    data.pdf_image_cache = runtime.pdf_image_cache.clone();
    data.remote_im_contacts = runtime.remote_im_contacts.clone();
    data.remote_im_contact_checkpoints = runtime.remote_im_contact_checkpoints.clone();
}

fn read_agents_shard(path: &PathBuf) -> Result<Vec<AgentProfile>, String> {
    if !app_layout_exists(path) && path.exists() {
        return Ok(read_app_data(path)?.agents);
    }
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

fn read_runtime_state_shard(path: &PathBuf) -> Result<RuntimeStateFile, String> {
    if !app_layout_exists(path) && path.exists() {
        let data = read_app_data(path)?;
        return Ok(build_runtime_state_file(&data));
    }
    if app_layout_runtime_state_path(path).exists() {
        read_json_file::<RuntimeStateFile>(&app_layout_runtime_state_path(path), "runtime state file")
    } else {
        Ok(RuntimeStateFile::default())
    }
}

fn write_runtime_state_shard(path: &PathBuf, runtime: &RuntimeStateFile) -> Result<bool, String> {
    fs::create_dir_all(app_layout_state_dir(path))
        .map_err(|err| format!("Create state layout dir failed: {err}"))?;
    write_json_file_atomic_if_changed(
        &app_layout_runtime_state_path(path),
        runtime,
        "runtime state file",
    )
}

fn read_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<Conversation, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    if let Some(conversation) =
        message_store::read_ready_message_store_directory_conversation(&store_paths)?
    {
        return Ok(conversation);
    }
    if let Some(status) = message_store::read_message_store_manifest_status(&store_paths)? {
        return Err(format!(
            "会话消息仓库未处于可读取状态，conversation_id={}，kind={}，state={}",
            conversation_id, status.message_store_kind, status.migration_state
        ));
    }
    let conversation_path = app_layout_chat_conversation_path(path, conversation_id);
    if conversation_path.exists() {
        return read_json_file::<Conversation>(&conversation_path, "conversation file");
    }
    if !app_layout_exists(path) && path.exists() {
        let data = read_app_data(path)?;
        if let Some(conversation) = data
            .conversations
            .into_iter()
            .find(|item| item.id.trim() == conversation_id)
        {
            return Ok(conversation);
        }
    }
    Err(format!("Conversation '{conversation_id}' not found."))
}

fn write_conversation_shard(path: &PathBuf, conversation: &Conversation) -> Result<bool, String> {
    fs::create_dir_all(app_layout_chat_conversations_dir(path))
        .map_err(|err| format!("Create chat conversations dir failed: {err}"))?;
    let store_paths = message_store::message_store_paths(path, &conversation.id)?;
    if message_store::should_write_jsonl_snapshot_directory_shard(&store_paths)? {
        message_store::write_jsonl_snapshot_directory_shard(&store_paths, conversation)?;
        return Ok(true);
    }
    write_json_file_atomic_if_changed(
        &app_layout_chat_conversation_path(path, &conversation.id),
        conversation,
        "conversation file",
    )
}

fn delete_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    message_store::delete_message_store_shard_artifacts(&store_paths)
}

fn read_chat_index_shard(path: &PathBuf) -> Result<ChatIndexFile, String> {
    if !app_layout_exists(path) && path.exists() {
        let data = read_app_data(path)?;
        return Ok(build_chat_index_file(&data.conversations));
    }
    if app_layout_chat_index_path(path).exists() {
        read_json_file::<ChatIndexFile>(&app_layout_chat_index_path(path), "chat index file")
    } else {
        Ok(ChatIndexFile::default())
    }
}

fn write_chat_index_shard(path: &PathBuf, index: &ChatIndexFile) -> Result<bool, String> {
    fs::create_dir_all(app_layout_chat_dir(path))
        .map_err(|err| format!("Create chat layout dir failed: {err}"))?;
    write_json_file_atomic_if_changed(&app_layout_chat_index_path(path), index, "chat index file")
}

fn app_layout_exists(path: &PathBuf) -> bool {
    app_layout_agents_path(path).exists()
        || app_layout_runtime_state_path(path).exists()
        || app_layout_chat_index_path(path).exists()
        || app_layout_chat_conversations_dir(path).exists()
}

fn legacy_app_data_split_dir(path: &PathBuf) -> PathBuf {
    let parent = path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join(LEGACY_APP_DATA_SPLIT_DIR_NAME)
}

fn legacy_app_data_split_profile_path(path: &PathBuf) -> PathBuf {
    legacy_app_data_split_dir(path).join(LEGACY_APP_DATA_PROFILE_FILE_NAME)
}

fn legacy_app_data_split_conversations_path(path: &PathBuf) -> PathBuf {
    legacy_app_data_split_dir(path).join(LEGACY_APP_DATA_CONVERSATIONS_FILE_NAME)
}

fn legacy_app_data_split_image_cache_path(path: &PathBuf) -> PathBuf {
    legacy_app_data_split_dir(path).join(LEGACY_APP_DATA_IMAGE_CACHE_FILE_NAME)
}

fn legacy_app_data_split_exists(path: &PathBuf) -> bool {
    legacy_app_data_split_profile_path(path).exists()
        || legacy_app_data_split_conversations_path(path).exists()
        || legacy_app_data_split_image_cache_path(path).exists()
}

fn read_json_file<T>(path: &PathBuf, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|err| format!("Read app_data failed: {err}"))?;
    serde_json::from_str::<T>(&content).map_err(|err| {
        eprintln!("[CONFIG] Parse {label} failed ({}): {err}", path.display());
        format!("Parse {label} failed ({}): {err}", path.display())
    })
}

fn file_metadata_signature(path: &PathBuf) -> (u64, Option<std::time::SystemTime>) {
    match fs::metadata(path) {
        Ok(metadata) => (metadata.len(), metadata.modified().ok()),
        Err(_) => (0, None),
    }
}

fn update_conversation_cache_signature_for_file(
    conversations: &mut ConversationDirCacheSignature,
    file_path: &PathBuf,
    file_name: String,
) {
    let Ok(metadata) = fs::metadata(file_path) else {
        return;
    };
    if !metadata.is_file() {
        return;
    }
    conversations.file_count += 1;
    conversations.total_size = conversations.total_size.saturating_add(metadata.len());
    let modified = metadata.modified().ok();
    let should_replace_latest = match (
        conversations.latest_modified,
        modified,
        conversations.latest_file_name.as_str(),
    ) {
        (None, Some(_), _) => true,
        (None, None, current_name) => file_name.as_str() > current_name,
        (Some(current), Some(next), current_name) => {
            next > current || (next == current && file_name.as_str() > current_name)
        }
        (Some(_), None, _) => false,
    };
    if should_replace_latest {
        conversations.latest_modified = modified;
        conversations.latest_file_name = file_name;
    }
}

fn app_data_cache_signature(path: &PathBuf) -> AppDataCacheSignature {
    let agents_path = app_layout_agents_path(path);
    let runtime_path = app_layout_runtime_state_path(path);
    let chat_index_path = app_layout_chat_index_path(path);
    let (agents_len, agents_modified) = file_metadata_signature(&agents_path);
    let (runtime_len, runtime_modified) = file_metadata_signature(&runtime_path);
    let (chat_index_len, chat_index_modified) = file_metadata_signature(&chat_index_path);

    let mut conversations = ConversationDirCacheSignature::default();
    let conversations_dir = app_layout_chat_conversations_dir(path);
    if let Ok(entries) = fs::read_dir(conversations_dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if entry_path.extension().and_then(|value| value.to_str()) == Some("json") {
                update_conversation_cache_signature_for_file(
                    &mut conversations,
                    &entry_path,
                    file_name,
                );
                continue;
            }
            if !entry_path.is_dir() {
                continue;
            }
            for shard_file_name in [
                message_store::MESSAGE_STORE_MANIFEST_FILE_NAME,
                message_store::MESSAGE_STORE_META_FILE_NAME,
                message_store::MESSAGE_STORE_MESSAGES_FILE_NAME,
                message_store::MESSAGE_STORE_INDEX_FILE_NAME,
            ] {
                update_conversation_cache_signature_for_file(
                    &mut conversations,
                    &entry_path.join(shard_file_name),
                    format!("{file_name}/{shard_file_name}"),
                );
            }
            let blocks_dir = entry_path.join(message_store::MESSAGE_STORE_BLOCKS_DIR_NAME);
            if let Ok(block_entries) = fs::read_dir(blocks_dir) {
                for block_entry in block_entries.flatten() {
                    let block_path = block_entry.path();
                    if !block_path.is_file() {
                        continue;
                    }
                    let block_file_name = block_entry.file_name().to_string_lossy().to_string();
                    update_conversation_cache_signature_for_file(
                        &mut conversations,
                        &block_path,
                        format!("{file_name}/{}/{}", message_store::MESSAGE_STORE_BLOCKS_DIR_NAME, block_file_name),
                    );
                }
            }
        }
    }

    AppDataCacheSignature {
        agents_len,
        agents_modified,
        runtime_len,
        runtime_modified,
        chat_index_len,
        chat_index_modified,
        conversations,
    }
}

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

fn read_legacy_app_data(path: &PathBuf) -> Result<AppData, String> {
    if !path.exists() {
        return Ok(AppData::default());
    }
    let mut parsed = read_json_file::<AppData>(path, "legacy app_data")?;
    parsed.version = APP_DATA_SCHEMA_VERSION;
    Ok(parsed)
}

fn read_legacy_split_app_data(path: &PathBuf) -> Result<AppData, String> {
    let defaults = AppData::default();
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LegacyProfile {
        version: u32,
        agents: Vec<AgentProfile>,
        #[serde(alias = "selectedAgentId", alias = "selected_agent_id")]
        assistant_department_agent_id: String,
        user_alias: String,
        response_style_id: String,
        #[serde(default = "default_pdf_read_mode")]
        pdf_read_mode: String,
        #[serde(default = "default_background_voice_screenshot_keywords")]
        background_voice_screenshot_keywords: String,
        #[serde(default = "default_background_voice_screenshot_mode")]
        background_voice_screenshot_mode: String,
        #[serde(default)]
        instruction_presets: Vec<PromptCommandPreset>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct LegacyConversations {
        #[serde(default)]
        conversations: Vec<Conversation>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct LegacyImageCache {
        #[serde(default)]
        image_text_cache: Vec<ImageTextCacheEntry>,
    }

    let profile_path = legacy_app_data_split_profile_path(path);
    let conversations_path = legacy_app_data_split_conversations_path(path);
    let image_cache_path = legacy_app_data_split_image_cache_path(path);

    let profile = if profile_path.exists() {
        read_json_file::<LegacyProfile>(&profile_path, "legacy app_data profile")?
    } else {
        LegacyProfile {
            version: defaults.version,
            agents: defaults.agents.clone(),
            assistant_department_agent_id: defaults.assistant_department_agent_id.clone(),
            user_alias: defaults.user_alias.clone(),
            response_style_id: defaults.response_style_id.clone(),
            pdf_read_mode: defaults.pdf_read_mode.clone(),
            background_voice_screenshot_keywords: defaults
                .background_voice_screenshot_keywords
                .clone(),
            background_voice_screenshot_mode: defaults
                .background_voice_screenshot_mode
                .clone(),
            instruction_presets: defaults.instruction_presets.clone(),
        }
    };
    let conversations = if conversations_path.exists() {
        read_json_file::<LegacyConversations>(&conversations_path, "legacy app_data conversations")?
    } else {
        LegacyConversations::default()
    };
    let image_cache = if image_cache_path.exists() {
        read_json_file::<LegacyImageCache>(&image_cache_path, "legacy app_data image cache")?
    } else {
        LegacyImageCache::default()
    };

    Ok(AppData {
        version: profile.version,
        agents: profile.agents,
        assistant_department_agent_id: profile.assistant_department_agent_id,
        user_alias: profile.user_alias,
        response_style_id: profile.response_style_id,
        pdf_read_mode: profile.pdf_read_mode,
        background_voice_screenshot_keywords: profile.background_voice_screenshot_keywords,
        background_voice_screenshot_mode: profile.background_voice_screenshot_mode,
        instruction_presets: profile.instruction_presets,
        main_conversation_id: None,
        pinned_conversation_ids: Vec::new(),
        conversations: conversations.conversations,
        archived_conversations: Vec::new(),
        image_text_cache: image_cache.image_text_cache,
        remote_im_contacts: Vec::new(),
        remote_im_contact_checkpoints: Vec::new(),
        pdf_text_cache: Vec::new(),
        pdf_image_cache: Vec::new(),
    })
}

fn read_layout_app_data(path: &PathBuf) -> Result<AppData, String> {
    let agents = if app_layout_agents_path(path).exists() {
        read_json_file::<AgentsFile>(&app_layout_agents_path(path), "agents file")?.agents
    } else {
        AppData::default().agents
    };

    let runtime = if app_layout_runtime_state_path(path).exists() {
        read_json_file::<RuntimeStateFile>(&app_layout_runtime_state_path(path), "runtime state file")?
    } else {
        RuntimeStateFile::default()
    };

    let mut conversations = Vec::<Conversation>::new();
    let index = if app_layout_chat_index_path(path).exists() {
        read_json_file::<ChatIndexFile>(&app_layout_chat_index_path(path), "chat index file")?
    } else {
        ChatIndexFile::default()
    };
    for item in &index.conversations {
        if let Ok(conv) = read_conversation_shard(path, &item.id) {
            conversations.push(conv);
        }
    }
    if conversations.is_empty() {
        let conv_dir = app_layout_chat_conversations_dir(path);
        if conv_dir.exists() {
            if let Ok(entries) = fs::read_dir(&conv_dir) {
                let mut seen_ids = std::collections::HashSet::<String>::new();
                for entry in entries.flatten() {
                    let p = entry.path();
                    let conversation_id = if p.extension().and_then(|v| v.to_str()) == Some("json")
                    {
                        p.file_stem()
                            .and_then(|v| v.to_str())
                            .unwrap_or_default()
                            .to_string()
                    } else if p.is_dir() {
                        p.file_name()
                            .and_then(|v| v.to_str())
                            .unwrap_or_default()
                            .to_string()
                    } else {
                        continue;
                    };
                    if conversation_id.trim().is_empty() || !seen_ids.insert(conversation_id.clone()) {
                        continue;
                    }
                    if let Ok(conv) = read_conversation_shard(path, &conversation_id) {
                        conversations.push(conv);
                    }
                }
            }
        }
    }

    Ok(AppData {
        version: runtime.version,
        agents,
        assistant_department_agent_id: runtime.assistant_department_agent_id,
        user_alias: runtime.user_alias,
        response_style_id: runtime.response_style_id,
        pdf_read_mode: runtime.pdf_read_mode,
        background_voice_screenshot_keywords: runtime.background_voice_screenshot_keywords,
        background_voice_screenshot_mode: runtime.background_voice_screenshot_mode,
        instruction_presets: runtime.instruction_presets,
        main_conversation_id: runtime.main_conversation_id,
        pinned_conversation_ids: runtime.pinned_conversation_ids,
        conversations,
        archived_conversations: Vec::new(),
        image_text_cache: runtime.image_text_cache,
        remote_im_contacts: runtime.remote_im_contacts,
        remote_im_contact_checkpoints: runtime.remote_im_contact_checkpoints,
        pdf_text_cache: runtime.pdf_text_cache,
        pdf_image_cache: runtime.pdf_image_cache,
    })
}

fn read_app_data(path: &PathBuf) -> Result<AppData, String> {
    let mut parsed = if app_layout_exists(path) {
        read_layout_app_data(path)?
    } else if legacy_app_data_split_exists(path) {
        read_legacy_split_app_data(path)?
    } else {
        read_legacy_app_data(path)?
    };
    parsed.version = APP_DATA_SCHEMA_VERSION;
    let builtin_agents_filled = ensure_required_builtin_agents(&mut parsed);
    let conversation_metadata_filled = fill_missing_conversation_metadata(&mut parsed);
    let message_speaker_filled = fill_missing_message_speaker_agent_ids(&mut parsed);
    let avatar_paths_migrated = migrate_agent_avatar_paths(path, &mut parsed);
    let merged_archives = migrate_app_data_archives_into_conversations(path, &mut parsed)?;
    let migrated = migrate_app_data_inline_media_to_refs(path, &mut parsed);
    let main_conversation_marker_changed = normalize_main_conversation_marker(&mut parsed, "");
    let mut tool_review_reports_migrated = false;
    for conversation in parsed.conversations.iter_mut() {
        let store_paths = message_store::message_store_paths(path, &conversation.id)?;
        let _ = message_store::resume_jsonl_snapshot_migration(&store_paths, conversation)?;
        if tool_review_migrate_legacy_reports_after_message_store(path, conversation)? {
            tool_review_reports_migrated = true;
        }
    }
    if conversation_metadata_filled
        || builtin_agents_filled
        || message_speaker_filled
        || avatar_paths_migrated
        || merged_archives
        || migrated
        || tool_review_reports_migrated
        || main_conversation_marker_changed
        || !app_layout_exists(path)
    {
        #[allow(deprecated)]
        write_app_data(path, &parsed)?;
    }
    Ok(parsed)
}

// AppData 聚合写入需要保留，作为兼容/迁移/全量导入导出入口。
// 但业务热路径禁止直接依赖它，应该优先走分片写入：
// agents / runtime_state / chat_index / conversation:<id>
fn write_app_data_with_stats(path: &PathBuf, data: &AppData) -> Result<AppDataWriteStats, String> {
    let agents = build_agents_file(&data.agents);
    let runtime = build_runtime_state_file(data);
    let index = ChatIndexFile {
        conversations: data
            .conversations
            .iter()
            .map(|c| ChatIndexConversationItem {
                id: c.id.clone(),
                updated_at: c.updated_at.clone(),
                status: c.status.clone(),
                summary: c.summary.clone(),
                archived_at: c.archived_at.clone(),
            })
            .collect::<Vec<_>>(),
    };

    fs::create_dir_all(app_layout_config_dir(path))
        .map_err(|err| format!("Create config layout dir failed: {err}"))?;
    fs::create_dir_all(app_layout_state_dir(path))
        .map_err(|err| format!("Create state layout dir failed: {err}"))?;
    fs::create_dir_all(app_layout_chat_dir(path))
        .map_err(|err| format!("Create chat layout dir failed: {err}"))?;
    fs::create_dir_all(app_layout_chat_conversations_dir(path))
        .map_err(|err| format!("Create chat conversations dir failed: {err}"))?;
    fs::create_dir_all(app_layout_backups_dir(path))
        .map_err(|err| format!("Create backups dir failed: {err}"))?;

    let mut stats = AppDataWriteStats::default();

    stats.agents_written = write_json_file_atomic_if_changed(
        &app_layout_agents_path(path),
        &agents,
        "agents file",
    )?;
    stats.runtime_written = write_json_file_atomic_if_changed(
        &app_layout_runtime_state_path(path),
        &runtime,
        "runtime state file",
    )?;
    stats.chat_index_written = write_json_file_atomic_if_changed(
        &app_layout_chat_index_path(path),
        &index,
        "chat index file",
    )?;

    let mut expected_ids = std::collections::HashSet::<String>::new();
    for conv in &data.conversations {
        expected_ids.insert(conv.id.clone());
        let conv_path = app_layout_chat_conversation_path(path, &conv.id);
        if write_json_file_atomic_if_changed(&conv_path, conv, "conversation file")? {
            stats.conversation_writes += 1;
        }
    }
    if let Ok(entries) = fs::read_dir(app_layout_chat_conversations_dir(path)) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let stem = p
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or_default()
                .to_string();
            if !expected_ids.contains(&stem) {
                if fs::remove_file(p).is_ok() {
                    stats.conversation_deletes += 1;
                }
            }
        }
    }
    Ok(stats)
}

/// Compatibility-only full AppData writer.
///
/// New production code must not add new call sites to this function. Prefer shard APIs:
/// `write_agents_shard`, `write_runtime_state_shard`, `write_chat_index_shard`,
/// `write_conversation_shard`, and their cached state wrappers.
///
/// Migration timeline:
/// - New code: forbidden immediately.
/// - Existing compatibility / migration / import-export flows: temporarily allowed.
/// - After compatibility-only callers are fully isolated, reevaluate final removal.
#[deprecated(
    note = "兼容层专用的全量 AppData 写入器；新代码禁止调用，请改用 agents/runtime_state/chat_index/conversation 分片写入 API。"
)]
fn write_app_data(path: &PathBuf, data: &AppData) -> Result<(), String> {
    let started = std::time::Instant::now();
    let stats = write_app_data_with_stats(path, data)?;
    runtime_log_debug(format!(
        "[应用数据写入] 任务=应用数据写入，状态=完成，触发=兼容层全量写入，agents_written={}，runtime_written={}，chat_index_written={}，conversation_writes={}，conversation_deletes={}，duration_ms={}",
        stats.agents_written,
        stats.runtime_written,
        stats.chat_index_written,
        stats.conversation_writes,
        stats.conversation_deletes,
        started.elapsed().as_millis()
    ));
    Ok(())
}
