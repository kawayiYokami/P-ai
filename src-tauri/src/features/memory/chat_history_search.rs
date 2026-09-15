const CHAT_HISTORY_SLICE_TARGET_CHARS: usize = 256;
const CHAT_HISTORY_LONG_MESSAGE_OVERLAP_CHARS: usize = 32;
const CHAT_HISTORY_INDEX_DIR_NAME: &str = "chat-history-tantivy";
const CHAT_HISTORY_INDEX_META_FILE_NAME: &str = "metadata.json";
const CHAT_HISTORY_INDEX_TMP_DIR_NAME: &str = "chat-history-tantivy.tmp";
const CHAT_HISTORY_FIELD_SLICE_IDX: &str = "slice_idx";
const CHAT_HISTORY_FIELD_CONTENT: &str = "content";
const CHAT_HISTORY_FIELD_OWNER_AGENT_ID: &str = "owner_agent_id";
/// 索引口径版本：切片归属口径变化时必须提升，强制废弃旧索引。
const CHAT_HISTORY_SIGNATURE_VERSION: &str = "v2-owner-agent-scope";

#[derive(Clone)]
struct ChatHistoryIndexFields {
    slice_idx: tantivy::schema::Field,
    content: tantivy::schema::Field,
    owner_agent_id: tantivy::schema::Field,
}

struct CachedChatHistoryIndex {
    signature: String,
    slices: Vec<ChatHistorySlice>,
    stats: ChatHistorySearchStats,
    index: Index,
    reader: tantivy::IndexReader,
    fields: ChatHistoryIndexFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistoryPersistedIndexMetadata {
    signature: String,
    slices: Vec<ChatHistorySlice>,
    stats: ChatHistorySearchStats,
}

fn chat_history_index_cache() -> &'static std::sync::Mutex<Option<CachedChatHistoryIndex>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<CachedChatHistoryIndex>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistorySearchInput {
    /// 可选的归属人格筛选：给值只返回该人格归属的会话，不给则全库检索。
    #[serde(default)]
    agent_id: Option<String>,
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistorySearchResult {
    hits: Vec<ChatHistorySearchHit>,
    stats: ChatHistorySearchStats,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistorySearchHit {
    slice: ChatHistorySlice,
    bm25_score: f64,
    bm25_normalized_score: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistorySearchStats {
    total_slices: usize,
    index_storage_bytes: u64,
    cached_slice_bytes: u64,
    indexed_conversations: usize,
    skipped_delegate_conversations: usize,
    skipped_live_blocks: usize,
    /// 归属人格为空的孤儿会话：不进索引，需先指定接管人格。
    #[serde(default)]
    skipped_orphan_conversations: usize,
    local_conversation_slices: usize,
    archive_slices: usize,
    contact_slices: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatHistorySlice {
    id: String,
    source_kind: String,
    source_id: String,
    source_title: String,
    segment_id: String,
    slice_index: usize,
    content: String,
    speakers: Vec<String>,
    /// 会话归属人格：检索按它筛选，孤儿会话（空）不入索引。
    owner_agent_id: String,
    time_start: String,
    time_end: String,
    message_start_id: String,
    message_end_id: String,
}

#[derive(Debug, Clone)]
struct ChatHistoryRenderedMessage {
    message_id: String,
    speaker_name: String,
    created_at: String,
    rendered: String,
}

#[derive(Debug, Clone)]
struct ChatHistorySegment {
    source_kind: String,
    source_id: String,
    source_title: String,
    segment_id: String,
    messages: Vec<ChatMessage>,
}

fn chat_history_index_signature(
    chat_index: &ChatIndexFile,
    agents: &[AgentProfile],
    user_alias: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CHAT_HISTORY_SIGNATURE_VERSION.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(user_alias.as_bytes());
    hasher.update(b"\x1f");
    for agent in agents {
        hasher.update(agent.id.as_bytes());
        hasher.update(b"\x1d");
        hasher.update(agent.name.as_bytes());
        hasher.update(b"\x1d");
        hasher.update(agent.updated_at.as_bytes());
        hasher.update(b"\x1e");
    }
    for item in &chat_index.conversations {
        hasher.update(item.id.as_bytes());
        hasher.update(b"\x1d");
        hasher.update(item.updated_at.as_bytes());
        hasher.update(b"\x1d");
        hasher.update(item.status.as_bytes());
        hasher.update(b"\x1d");
        hasher.update(item.archived_at.as_deref().unwrap_or("").as_bytes());
        hasher.update(b"\x1e");
    }
    bytes_to_lower_hex(hasher.finalize())
}

fn chat_history_source_kind(conversation: &Conversation) -> Option<&'static str> {
    if conversation_is_delegate(conversation) {
        return None;
    }
    if conversation_is_archived(conversation) {
        return Some("archive");
    }
    if conversation_is_remote_im_contact(conversation) {
        return Some("contact");
    }
    Some("localConversation")
}

fn chat_history_source_kind_from_meta(
    conversation_meta: &ConversationMetaView,
) -> Option<&'static str> {
    let kind = conversation_meta.conversation_kind.trim();
    if kind == CONVERSATION_KIND_DELEGATE {
        return None;
    }
    if conversation_meta.status.trim() == "archived"
        || conversation_meta
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    {
        return Some("archive");
    }
    if kind == CONVERSATION_KIND_REMOTE_IM_CONTACT {
        return Some("contact");
    }
    Some("localConversation")
}

fn chat_history_message_text(message: &ChatMessage) -> String {
    let mut parts = Vec::<String>::new();
    for part in &message.parts {
        if let MessagePart::Text { text, .. } = part {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
    }
    for block in &message.extra_text_blocks {
        let trimmed = block.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
    parts.join("\n")
}

fn chat_history_speaker_name(
    message: &ChatMessage,
    agents: &[AgentProfile],
    user_alias: &str,
) -> String {
    if let Some(origin) = remote_im_origin_from_message(message) {
        if let Some(sender) = remote_im_origin_string(origin, "sender_name") {
            if !sender.trim().is_empty() {
                return sender.trim().to_string();
            }
        }
        if let Some(contact) = remote_im_origin_string(origin, "contact_name") {
            if !contact.trim().is_empty() {
                return contact.trim().to_string();
            }
        }
    }

    let speaker_id = message.speaker_agent_id.as_deref().unwrap_or("").trim();
    if !speaker_id.is_empty() {
        if let Some(agent) = agents.iter().find(|agent| agent.id == speaker_id) {
            return agent.name.trim().to_string();
        }
        return speaker_id.to_string();
    }

    match message.role.trim() {
        "user" => {
            let alias = user_alias.trim();
            if alias.is_empty() {
                "用户".to_string()
            } else {
                alias.to_string()
            }
        }
        "assistant" => "助手".to_string(),
        "system" => "系统".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "未知".to_string(),
    }
}

fn chat_history_render_message(
    message: &ChatMessage,
    agents: &[AgentProfile],
    user_alias: &str,
) -> Option<ChatHistoryRenderedMessage> {
    let text = chat_history_message_text(message);
    if text.trim().is_empty() {
        return None;
    }
    let speaker_name = chat_history_speaker_name(message, agents, user_alias);
    Some(ChatHistoryRenderedMessage {
        message_id: message.id.clone(),
        created_at: message.created_at.clone(),
        rendered: format!("[{}]: {}", speaker_name, text.trim()),
        speaker_name,
    })
}

fn chat_history_unique_push(out: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if value.trim().is_empty() || out.iter().any(|item| item == &value) {
        return;
    }
    out.push(value);
}

fn chat_history_split_long_rendered_message(
    message: &ChatHistoryRenderedMessage,
) -> Vec<String> {
    let chars = message.rendered.chars().collect::<Vec<_>>();
    if chars.len() <= CHAT_HISTORY_SLICE_TARGET_CHARS {
        return vec![message.rendered.clone()];
    }
    let mut out = Vec::<String>::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + CHAT_HISTORY_SLICE_TARGET_CHARS).min(chars.len());
        out.push(chars[start..end].iter().collect::<String>());
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(CHAT_HISTORY_LONG_MESSAGE_OVERLAP_CHARS);
    }
    out
}

fn chat_history_build_slice(
    segment: &ChatHistorySegment,
    slice_index: usize,
    content: String,
    messages: &[ChatHistoryRenderedMessage],
    owner_agent_id: &str,
) -> ChatHistorySlice {
    let mut speakers = Vec::<String>::new();
    for message in messages {
        chat_history_unique_push(&mut speakers, message.speaker_name.clone());
    }
    let time_start = messages
        .first()
        .map(|message| message.created_at.clone())
        .unwrap_or_default();
    let time_end = messages
        .last()
        .map(|message| message.created_at.clone())
        .unwrap_or_default();
    let message_start_id = messages
        .first()
        .map(|message| message.message_id.clone())
        .unwrap_or_default();
    let message_end_id = messages
        .last()
        .map(|message| message.message_id.clone())
        .unwrap_or_default();
    ChatHistorySlice {
        id: format!("{}:{}:{}", segment.source_id, segment.segment_id, slice_index),
        source_kind: segment.source_kind.clone(),
        source_id: segment.source_id.clone(),
        source_title: segment.source_title.clone(),
        segment_id: segment.segment_id.clone(),
        slice_index,
        content,
        speakers,
        owner_agent_id: owner_agent_id.to_string(),
        time_start,
        time_end,
        message_start_id,
        message_end_id,
    }
}

fn chat_history_slices_from_segment(
    segment: &ChatHistorySegment,
    agents: &[AgentProfile],
    user_alias: &str,
    owner_agent_id: &str,
) -> Vec<ChatHistorySlice> {
    let rendered = segment
        .messages
        .iter()
        .filter_map(|message| chat_history_render_message(message, agents, user_alias))
        .collect::<Vec<_>>();
    if rendered.is_empty() || owner_agent_id.trim().is_empty() {
        return Vec::new();
    }

    let mut slices = Vec::<ChatHistorySlice>::new();
    let mut current_lines = Vec::<String>::new();
    let mut current_messages = Vec::<ChatHistoryRenderedMessage>::new();

    let flush = |
        slices: &mut Vec<ChatHistorySlice>,
        current_lines: &mut Vec<String>,
        current_messages: &mut Vec<ChatHistoryRenderedMessage>,
    | {
        if current_lines.is_empty() {
            return;
        }
        let content = current_lines.join("\n");
        let slice = chat_history_build_slice(
            segment,
            slices.len(),
            content,
            current_messages,
            owner_agent_id,
        );
        slices.push(slice);
        current_lines.clear();
        current_messages.clear();
    };

    for message in rendered {
        if message.rendered.chars().count() > CHAT_HISTORY_SLICE_TARGET_CHARS {
            flush(&mut slices, &mut current_lines, &mut current_messages);
            for part in chat_history_split_long_rendered_message(&message) {
                slices.push(chat_history_build_slice(
                    segment,
                    slices.len(),
                    part,
                    &[message.clone()],
                    owner_agent_id,
                ));
            }
            continue;
        }

        let next_len = current_lines.join("\n").chars().count()
            + if current_lines.is_empty() { 0 } else { 1 }
            + message.rendered.chars().count();
        if !current_lines.is_empty() && next_len > CHAT_HISTORY_SLICE_TARGET_CHARS {
            flush(&mut slices, &mut current_lines, &mut current_messages);
        }
        current_lines.push(message.rendered.clone());
        current_messages.push(message);
    }
    flush(&mut slices, &mut current_lines, &mut current_messages);
    slices
}

fn chat_history_segments_from_snapshot(conversation: &Conversation) -> Vec<ChatHistorySegment> {
    conversation
        .messages
        .iter()
        .filter(|message| is_context_compaction_message(message, message.role.as_str()))
        .cloned()
        .enumerate()
        .map(|(idx, message)| ChatHistorySegment {
            source_kind: chat_history_source_kind(conversation)
                .unwrap_or("localConversation")
                .to_string(),
            source_id: conversation.id.clone(),
            source_title: conversation.title.clone(),
            segment_id: format!("snapshot-{idx}"),
            messages: vec![message],
        })
        .collect()
}

fn chat_history_segments_from_message_store(
    data_path: &PathBuf,
    conversation: &Conversation,
) -> Result<(Vec<ChatHistorySegment>, usize), String> {
    let Some(source_kind) = chat_history_source_kind(conversation) else {
        return Ok((Vec::new(), 0));
    };
    let paths = message_store::message_store_paths(data_path, &conversation.id)?;
    let Some(page) = message_store::chat_store_read_block_page(&paths, None)? else {
        return Ok((chat_history_segments_from_snapshot(conversation), 0));
    };
    if page.blocks.is_empty() {
        return Ok((chat_history_segments_from_snapshot(conversation), 0));
    }

    let mut segments = Vec::<ChatHistorySegment>::new();
    let mut skipped_live_blocks = 0usize;
    for block in page.blocks {
        if !conversation_is_archived(conversation) && block.is_latest {
            skipped_live_blocks += 1;
            continue;
        }
        let Some(block_page) =
            message_store::chat_store_read_block_page(&paths, Some(block.block_id))?
        else {
            continue;
        };
        if block_page.messages.is_empty() {
            continue;
        }
        segments.push(ChatHistorySegment {
            source_kind: source_kind.to_string(),
            source_id: conversation.id.clone(),
            source_title: conversation.title.clone(),
            segment_id: format!("block-{}", block.block_id),
            messages: block_page.messages,
        });
    }

    if segments.is_empty() {
        Ok((chat_history_segments_from_snapshot(conversation), skipped_live_blocks))
    } else {
        Ok((segments, skipped_live_blocks))
    }
}

fn chat_history_user_persona_name(agents: &[AgentProfile]) -> String {
    agents
        .iter()
        .find(|agent| agent.id == USER_PERSONA_ID || agent.is_built_in_user)
        .map(|agent| agent.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn chat_history_collect_slices_for_state(
    state: &AppState,
) -> Result<(String, Vec<ChatHistorySlice>, ChatHistorySearchStats), String> {
    let started_stats = ChatHistorySearchStats::default();
    let mut stats = started_stats;
    let agents = state_read_agents_cached(state)?;
    let chat_index = state_read_chat_index_cached(state)?;
    let user_alias = chat_history_user_persona_name(&agents);
    let signature = chat_history_index_signature(&chat_index, &agents, &user_alias);
    let mut slices = Vec::<ChatHistorySlice>::new();

    for item in chat_index.conversations {
        let conversation_meta = match conversation_service_v2().get_conversation_meta(state, &item.id) {
            Ok(conversation_meta) => conversation_meta,
            Err(_) => continue,
        };
        let Some(_) = chat_history_source_kind_from_meta(&conversation_meta) else {
            stats.skipped_delegate_conversations += 1;
            continue;
        };
        // 归属人格即会话记忆的归属者；孤儿会话（归属为空）不入索引。
        let owner_agent_id = conversation_meta.agent_id.trim().to_string();
        if owner_agent_id.is_empty() {
            stats.skipped_orphan_conversations += 1;
            continue;
        }
        let conversation = Conversation {
            id: conversation_meta.id.clone(),
            title: conversation_meta.title.clone(),
            agent_id: conversation_meta.agent_id.clone(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: conversation_meta.unread_count,
            conversation_kind: conversation_meta.conversation_kind.clone(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: conversation_meta.created_at.clone(),
            updated_at: conversation_meta.updated_at.clone(),
            last_user_at: None,
            last_assistant_at: None,
            status: conversation_meta.status.clone(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: conversation_meta.shell_workspace_path.clone(),
            shell_workspaces: conversation_meta.shell_workspaces.clone(),
            shell_autonomous_mode: conversation_meta.shell_autonomous_mode,
            shell_work_mode: normalize_shell_work_mode_text(&conversation_meta.shell_work_mode),
            shell_work_branch: String::new(),
            archived_at: conversation_meta.archived_at.clone(),
            messages: Vec::new(),
            fast_request_turns: conversation_meta.fast_request_turns.clone(),
            current_todos: conversation_meta.current_todos.clone(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: conversation_meta.preferred_api_config_id.clone(),
            is_draft: conversation_meta.is_draft,
            auto_push_remote_contact_id: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            active_goal: conversation_meta.active_goal.clone(),
            last_error: conversation_meta.last_error.clone(),
        };
        let (segments, skipped_live_blocks) =
            chat_history_segments_from_message_store(&state.data_path, &conversation)?;
        stats.skipped_live_blocks += skipped_live_blocks;
        if segments.is_empty() {
            continue;
        }
        stats.indexed_conversations += 1;
        for segment in segments {
            let before = slices.len();
            let built =
                chat_history_slices_from_segment(&segment, &agents, &user_alias, &owner_agent_id);
            if built.is_empty() {
                continue;
            }
            slices.extend(built);
            let added = slices.len().saturating_sub(before);
            match segment.source_kind.as_str() {
                "archive" => stats.archive_slices += added,
                "contact" => stats.contact_slices += added,
                _ => stats.local_conversation_slices += added,
            }
        }
    }

    stats.total_slices = slices.len();
    Ok((signature, slices, stats))
}

fn chat_history_index_root(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("indexes")
        .join(CHAT_HISTORY_INDEX_DIR_NAME)
}

fn chat_history_index_tmp_root(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("indexes")
        .join(CHAT_HISTORY_INDEX_TMP_DIR_NAME)
}

fn chat_history_index_meta_path(index_root: &PathBuf) -> PathBuf {
    index_root.join(CHAT_HISTORY_INDEX_META_FILE_NAME)
}

fn chat_history_directory_size(path: &PathBuf) -> u64 {
    let Ok(metadata) = std::fs::metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| chat_history_directory_size(&entry.path()))
        .sum()
}

fn chat_history_string_bytes(value: &str) -> u64 {
    value.as_bytes().len() as u64
}

fn chat_history_vec_string_bytes(values: &[String]) -> u64 {
    values
        .iter()
        .map(|value| chat_history_string_bytes(value))
        .sum()
}

fn chat_history_estimate_slice_cache_bytes(slices: &[ChatHistorySlice]) -> u64 {
    slices
        .iter()
        .map(|slice| {
            chat_history_string_bytes(&slice.id)
                + chat_history_string_bytes(&slice.source_kind)
                + chat_history_string_bytes(&slice.source_id)
                + chat_history_string_bytes(&slice.source_title)
                + chat_history_string_bytes(&slice.segment_id)
                + chat_history_string_bytes(&slice.content)
                + chat_history_vec_string_bytes(&slice.speakers)
                + chat_history_string_bytes(&slice.owner_agent_id)
                + chat_history_string_bytes(&slice.time_start)
                + chat_history_string_bytes(&slice.time_end)
                + chat_history_string_bytes(&slice.message_start_id)
                + chat_history_string_bytes(&slice.message_end_id)
        })
        .sum()
}

fn chat_history_build_schema() -> (Schema, ChatHistoryIndexFields) {
    let mut schema_builder = Schema::builder();
    let slice_idx_field = schema_builder.add_u64_field(CHAT_HISTORY_FIELD_SLICE_IDX, FAST | STORED);
    let indexing = TextFieldIndexing::default()
        .set_tokenizer("chat_ws")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(indexing)
        .set_stored();
    let content_field = schema_builder.add_text_field(CHAT_HISTORY_FIELD_CONTENT, text_options);
    let owner_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("raw")
            .set_index_option(IndexRecordOption::Basic),
    );
    let owner_field =
        schema_builder.add_text_field(CHAT_HISTORY_FIELD_OWNER_AGENT_ID, owner_options);
    let schema = schema_builder.build();
    (
        schema,
        ChatHistoryIndexFields {
            slice_idx: slice_idx_field,
            content: content_field,
            owner_agent_id: owner_field,
        },
    )
}

fn chat_history_fields_from_schema(schema: &Schema) -> Result<ChatHistoryIndexFields, String> {
    let slice_idx = schema
        .get_field(CHAT_HISTORY_FIELD_SLICE_IDX)
        .map_err(|err| format!("Read chat history slice_idx field failed: {err}"))?;
    let content = schema
        .get_field(CHAT_HISTORY_FIELD_CONTENT)
        .map_err(|err| format!("Read chat history content field failed: {err}"))?;
    let owner_field = schema
        .get_field(CHAT_HISTORY_FIELD_OWNER_AGENT_ID)
        .map_err(|err| format!("Read chat history owner_agent_id field failed: {err}"))?;
    Ok(ChatHistoryIndexFields {
        slice_idx,
        content,
        owner_agent_id: owner_field,
    })
}

fn chat_history_register_tokenizers(index: &Index) {
    index
        .tokenizers()
        .register("chat_ws", TextAnalyzer::from(SimpleTokenizer::default()));
}

fn chat_history_write_index_documents(
    index: &Index,
    fields: &ChatHistoryIndexFields,
    slices: &[ChatHistorySlice],
) -> Result<(), String> {
    let mut writer = index
        .writer(20_000_000)
        .map_err(|err| format!("Create chat history tantivy writer failed: {err}"))?;
    for (idx, slice) in slices.iter().enumerate() {
        let tokenized = memory_tokenize_terms(&slice.content, false).join(" ");
        if tokenized.trim().is_empty() {
            continue;
        }
        let mut document = doc!(fields.slice_idx => idx as u64, fields.content => tokenized);
        if !slice.owner_agent_id.trim().is_empty() {
            document.add_text(fields.owner_agent_id, &slice.owner_agent_id);
        }
        writer
            .add_document(document)
            .map_err(|err| format!("Add chat history tantivy document failed: {err}"))?;
    }
    writer
        .commit()
        .map_err(|err| format!("Commit chat history tantivy index failed: {err}"))?;
    Ok(())
}

#[cfg(test)]
fn chat_history_build_tantivy_index(
    slices: &[ChatHistorySlice],
) -> Result<(Index, tantivy::IndexReader, ChatHistoryIndexFields), String> {
    let (schema, fields) = chat_history_build_schema();
    let index = Index::create_in_ram(schema);
    chat_history_register_tokenizers(&index);
    chat_history_write_index_documents(&index, &fields, slices)?;
    let reader = index
        .reader()
        .map_err(|err| format!("Open chat history tantivy reader failed: {err}"))?;
    Ok((index, reader, fields))
}

fn chat_history_write_persisted_metadata(
    index_root: &PathBuf,
    metadata: &ChatHistoryPersistedIndexMetadata,
) -> Result<(), String> {
    let meta_path = chat_history_index_meta_path(index_root);
    let body = serde_json::to_string(metadata)
        .map_err(|err| format!("Serialize chat history index metadata failed: {err}"))?;
    std::fs::write(&meta_path, body)
        .map_err(|err| format!("Write chat history index metadata failed: {err}"))
}

fn chat_history_read_persisted_metadata(
    index_root: &PathBuf,
) -> Result<ChatHistoryPersistedIndexMetadata, String> {
    let meta_path = chat_history_index_meta_path(index_root);
    let body = std::fs::read_to_string(&meta_path)
        .map_err(|err| format!("Read chat history index metadata failed: {err}"))?;
    serde_json::from_str(&body)
        .map_err(|err| format!("Parse chat history index metadata failed: {err}"))
}

fn chat_history_open_persisted_index(
    index_root: &PathBuf,
    expected_signature: &str,
) -> Result<CachedChatHistoryIndex, String> {
    let metadata = chat_history_read_persisted_metadata(index_root)?;
    if metadata.signature != expected_signature {
        return Err("Chat history index signature mismatch".to_string());
    }
    let index = Index::open_in_dir(index_root)
        .map_err(|err| format!("Open chat history persisted index failed: {err}"))?;
    chat_history_register_tokenizers(&index);
    let fields = chat_history_fields_from_schema(&index.schema())?;
    let reader = index
        .reader()
        .map_err(|err| format!("Open chat history persisted reader failed: {err}"))?;
    Ok(CachedChatHistoryIndex {
        signature: metadata.signature,
        slices: metadata.slices,
        stats: metadata.stats,
        index,
        reader,
        fields,
    })
}

fn chat_history_rebuild_persisted_index(
    state: &AppState,
    signature: String,
    slices: Vec<ChatHistorySlice>,
    stats: ChatHistorySearchStats,
) -> Result<CachedChatHistoryIndex, String> {
    let index_root = chat_history_index_root(&state.data_path);
    let tmp_root = chat_history_index_tmp_root(&state.data_path);
    if tmp_root.exists() {
        std::fs::remove_dir_all(&tmp_root)
            .map_err(|err| format!("Remove stale chat history temp index failed: {err}"))?;
    }
    let Some(parent) = tmp_root.parent() else {
        return Err("Resolve chat history index parent failed".to_string());
    };
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("Create chat history index parent failed: {err}"))?;
    std::fs::create_dir_all(&tmp_root)
        .map_err(|err| format!("Create chat history temp index failed: {err}"))?;

    let (schema, fields) = chat_history_build_schema();
    let index = Index::create_in_dir(&tmp_root, schema)
        .map_err(|err| format!("Create chat history persisted index failed: {err}"))?;
    chat_history_register_tokenizers(&index);
    chat_history_write_index_documents(&index, &fields, &slices)?;
    chat_history_write_persisted_metadata(
        &tmp_root,
        &ChatHistoryPersistedIndexMetadata {
            signature: signature.clone(),
            slices,
            stats,
        },
    )?;
    drop(index);

    if index_root.exists() {
        std::fs::remove_dir_all(&index_root)
            .map_err(|err| format!("Remove old chat history index failed: {err}"))?;
    }
    std::fs::rename(&tmp_root, &index_root).or_else(|_| {
        std::fs::create_dir_all(&index_root)?;
        for entry in std::fs::read_dir(&tmp_root)? {
            let entry = entry?;
            let target = index_root.join(entry.file_name());
            if entry.path().is_dir() {
                std::fs::rename(entry.path(), target)?;
            } else {
                std::fs::copy(entry.path(), target)?;
            }
        }
        std::fs::remove_dir_all(&tmp_root)
    }).map_err(|err| format!("Install chat history index failed: {err}"))?;

    chat_history_open_persisted_index(&index_root, &signature)
}

fn chat_history_cached_index_for_state(state: &AppState) -> Result<(), String> {
    let chat_index = state_read_chat_index_cached(state)?;
    let agents = state_read_agents_cached(state)?;
    let user_alias = chat_history_user_persona_name(&agents);
    let expected_signature = chat_history_index_signature(&chat_index, &agents, &user_alias);
    {
        let mut cache = chat_history_index_cache()
            .lock()
            .map_err(|err| format!("Lock chat history index cache failed: {err}"))?;
        if cache
            .as_ref()
            .map(|cached| cached.signature == expected_signature)
            .unwrap_or(false)
        {
            return Ok(());
        }
        if cache.is_some() {
            *cache = None;
        }
    }

    let index_root = chat_history_index_root(&state.data_path);
    if let Ok(persisted) = chat_history_open_persisted_index(&index_root, &expected_signature) {
        let mut cache = chat_history_index_cache()
            .lock()
            .map_err(|err| format!("Lock chat history index cache failed: {err}"))?;
        *cache = Some(persisted);
        return Ok(());
    }

    let (signature, slices, stats) = chat_history_collect_slices_for_state(state)?;
    let rebuilt = chat_history_rebuild_persisted_index(state, signature, slices, stats)?;
    let mut cache = chat_history_index_cache()
        .lock()
        .map_err(|err| format!("Lock chat history index cache failed: {err}"))?;
    *cache = Some(rebuilt);
    Ok(())
}

fn chat_history_tantivy_search(
    cached: &CachedChatHistoryIndex,
    owner_agent_id: Option<&str>,
    query_text: &str,
    limit: usize,
) -> Result<Vec<(usize, f64, f64)>, String> {
    if cached.slices.is_empty() || query_text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let searcher = cached.reader.searcher();
    let qp = QueryParser::for_index(&cached.index, vec![cached.fields.content]);
    let terms = memory_tokenize_terms(query_text, true);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let query = memory_build_any_terms_query("content", &terms);
    let content_query = qp
        .parse_query(&query)
        .map_err(|err| format!("Parse chat history query failed: {err}"))?;
    let mut clauses: Vec<(tantivy::query::Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
    if let Some(owner) = owner_agent_id.map(str::trim).filter(|value| !value.is_empty()) {
        let owner_term = tantivy::Term::from_field_text(cached.fields.owner_agent_id, owner);
        clauses.push((
            tantivy::query::Occur::Must,
            Box::new(tantivy::query::TermQuery::new(
                owner_term,
                IndexRecordOption::Basic,
            )),
        ));
    }
    clauses.push((tantivy::query::Occur::Must, content_query));
    let parsed = tantivy::query::BooleanQuery::new(clauses);
    let hits = searcher
        .search(&parsed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|err| format!("Search chat history bm25 failed: {err}"))?;
    let max_score = hits
        .iter()
        .map(|(score, _)| *score as f64)
        .fold(0.0f64, f64::max);
    let mut out = Vec::<(usize, f64, f64)>::new();
    for (score, addr) in hits {
        let doc: tantivy::schema::TantivyDocument = searcher
            .doc(addr)
            .map_err(|err| format!("Read chat history hit document failed: {err}"))?;
        let idx = doc
            .get_first(cached.fields.slice_idx)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .ok_or_else(|| "Read chat history slice_idx failed".to_string())?;
        let raw = score as f64;
        let normalized = if max_score > 0.0 {
            (raw / max_score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push((idx, raw, normalized));
    }
    Ok(out)
}

fn chat_history_search_slices(
    state: &AppState,
    input: &ChatHistorySearchInput,
) -> Result<ChatHistorySearchResult, String> {
    let started = std::time::Instant::now();
    let owner_filter = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let query = input.query.trim();
    let limit = input.limit.unwrap_or(20).clamp(1, 100);
    chat_history_cached_index_for_state(state)?;
    let cache = chat_history_index_cache()
        .lock()
        .map_err(|err| format!("Lock chat history index cache failed: {err}"))?;
    let cached = cache
        .as_ref()
        .ok_or_else(|| "Chat history index cache is empty".to_string())?;
    let mut stats = cached.stats.clone();
    stats.index_storage_bytes = chat_history_directory_size(&chat_history_index_root(&state.data_path));
    stats.cached_slice_bytes = chat_history_estimate_slice_cache_bytes(&cached.slices);

    let hits = if query.is_empty() {
        cached
            .slices
            .iter()
            .filter(|slice| match owner_filter {
                Some(owner) => slice.owner_agent_id.as_str() == owner,
                None => true,
            })
            .take(limit)
            .cloned()
            .map(|slice| ChatHistorySearchHit {
                slice,
                bm25_score: 0.0,
                bm25_normalized_score: 0.0,
            })
            .collect::<Vec<_>>()
    } else {
        chat_history_tantivy_search(cached, owner_filter, query, limit)?
            .into_iter()
            .filter_map(|(idx, raw, normalized)| {
                cached.slices.get(idx).cloned().map(|slice| ChatHistorySearchHit {
                    slice,
                    bm25_score: raw,
                    bm25_normalized_score: normalized,
                })
            })
            .collect::<Vec<_>>()
    };

    Ok(ChatHistorySearchResult {
        hits,
        stats,
        elapsed_ms: started.elapsed().as_millis(),
    })
}