#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptPreviewMode {
    Chat,
    Compaction,
    Archive,
}

fn parse_prompt_preview_mode(raw: Option<&str>) -> PromptPreviewMode {
    match raw.unwrap_or("").trim() {
        "compaction" => PromptPreviewMode::Compaction,
        "archive" => PromptPreviewMode::Archive,
        _ => PromptPreviewMode::Chat,
    }
}

#[tauri::command]
async fn get_prompt_preview(
    input: SessionSelector,
    preview_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<PromptPreview, String> {
    let mut app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;
    let mut resolved_api = resolve_api_config(&app_config, Some(&api_config.id))?;

    let mut data = state_read_agents_runtime_snapshot(&state)?;
    merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut data)?;
    let requested_agent_id = input.agent_id.trim();
    let effective_agent_id = if !requested_agent_id.is_empty()
        && data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
    {
        requested_agent_id.to_string()
    } else if data
        .agents
        .iter()
        .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
    {
        data.assistant_department_agent_id.clone()
    } else {
        data.agents
            .iter()
            .find(|a| !a.is_built_in_user)
            .map(|a| a.id.clone())
            .ok_or_else(|| "Selected agent not found.".to_string())?
    };

    let agent = data
        .agents
        .iter()
        .find(|a| a.id == effective_agent_id)
        .cloned()
        .ok_or_else(|| "Selected agent not found.".to_string())?;
    let preview_mode = parse_prompt_preview_mode(preview_mode.as_deref());

    let mut conversation = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|conversation_id| state_read_conversation_cached(&state, conversation_id).ok())
        .filter(|conversation| {
            conversation.summary.trim().is_empty() && !conversation_is_delegate(conversation)
        })
        .or_else(|| {
            conversation_service()
                .resolve_latest_foreground_conversation_id(&state, &effective_agent_id)
                .ok()
                .flatten()
                .and_then(|conversation_id| state_read_conversation_cached(&state, &conversation_id).ok())
        })
        .unwrap_or_else(|| Conversation {
            id: "preview".to_string(),
            title: "Preview".to_string(),
            agent_id: effective_agent_id.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
    let latest_user_message = conversation.messages.iter().rev().find(|message| {
        prompt_role_for_message(message, &effective_agent_id).as_deref() == Some("user")
    });
    let latest_user_message_id = latest_user_message
        .map(|message| message.id.clone())
        .unwrap_or_default();
    let latest_user_retrieved_memory_ids = latest_user_message
        .and_then(|message| {
            message
                .provider_meta
                .as_ref()
                .and_then(|meta| {
                    meta.get("retrieved_memory_ids")
                        .or_else(|| meta.get("recallMemoryIds"))
                })
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();
    eprintln!(
        "[请求体预览][当前消息提取] mode={:?} requested_conversation_id={:?} selected_conversation_id={} agent_id={} latest_user_message_id={} latest_user_retrieved_memory_ids={:?}",
        preview_mode,
        input.conversation_id,
        conversation.id,
        effective_agent_id,
        latest_user_message_id,
        latest_user_retrieved_memory_ids
    );

    let user_name = user_persona_name(&data);
    let user_intro = user_persona_intro(&data);
    let last_archive_summary = state_read_chat_index_cached(&state)?
        .conversations
        .iter()
        .rev()
        .filter_map(|item| state_read_conversation_cached(&state, item.id.as_str()).ok())
        .find(|c| !conversation_is_delegate(c) && !c.summary.trim().is_empty())
        .map(|c| c.summary.clone());
    let mut prepared = match preview_mode {
        PromptPreviewMode::Chat => build_prepared_prompt_for_mode(
            PromptBuildMode::Chat,
            &conversation,
            &agent,
            &data.agents,
            &app_config.departments,
            &user_name,
            &user_intro,
            &data.response_style_id,
            &app_config.ui_language,
            Some(&state.data_path),
            last_archive_summary.as_deref(),
            None,
            Some(ChatPromptOverrides::default()),
            Some(&*state),
            Some(&api_config),
            Some(&resolved_api),
            Some(data.pdf_read_mode == "image" && api_config.enable_image),
        ),
        PromptPreviewMode::Compaction | PromptPreviewMode::Archive => {
            let owner_agent_id =
                resolve_archive_owner_agent_id(&app_config, &data.agents, &conversation)?;
            let owner_agent = data
                .agents
                .iter()
                .find(|item| item.id == owner_agent_id)
                .cloned()
                .ok_or_else(|| "Selected agent not found.".to_string())?;
            build_prepared_prompt_for_mode(
                PromptBuildMode::SummaryContext,
                &conversation,
                &owner_agent,
                &data.agents,
                &app_config.departments,
                &user_name,
                &user_intro,
                &data.response_style_id,
                &app_config.ui_language,
                Some(&state.data_path),
                last_archive_summary.as_deref(),
                None,
                Some(ChatPromptOverrides {
                    latest_user_intent: Some(LatestUserPayloadIntent::SummaryContext {
                        scene: if preview_mode == PromptPreviewMode::Compaction {
                            SummaryContextScene::Compaction
                        } else {
                            SummaryContextScene::Archive
                        },
                        user_alias: user_name.clone(),
                        current_user_profile: build_user_profile_memory_board(
                            &state.data_path,
                            &owner_agent,
                        )?
                        .unwrap_or_else(|| "（无）".to_string()),
                        include_todo_block: build_summary_context_todo_block(&conversation)
                            .is_some(),
                    }),
                    latest_images: Some(Vec::new()),
                    latest_audios: Some(Vec::new()),
                    ..Default::default()
                }),
                Some(&*state),
                Some(&api_config),
                Some(&resolved_api),
                Some(data.pdf_read_mode == "image" && api_config.enable_image),
            )
        }
    };
    let model_name = if api_config.model.trim().is_empty() {
        resolved_api.model.clone()
    } else {
        api_config.model.trim().to_string()
    };
    maybe_prepare_aliyun_multimodal_urls_for_candidate(
        state.inner(),
        &api_config,
        &mut resolved_api,
        &model_name,
        &mut prepared,
        &mut conversation,
        false,
        false,
    )
    .await?;

    let request_body_json =
        serde_json::to_string_pretty(&prepared_prompt_to_messages_json(&prepared))
            .map_err(|err| format!("序列化请求预览失败：{err}"))?;
    eprintln!(
        "[请求体预览] 完成: mode={:?} conversation_id={} latest_user_text_len={} latest_images={} latest_audios={} request_has_memory_board={} request_len={}",
        preview_mode,
        conversation.id,
        prepared.latest_user_text.len(),
        prepared.latest_images.len(),
        prepared.latest_audios.len(),
        request_body_json.contains("<memory_context>"),
        request_body_json.len()
    );
    Ok(PromptPreview {
        preamble: prepared.preamble,
        latest_user_text: prepared.latest_user_text,
        latest_images: prepared.latest_images.len(),
        latest_audios: prepared.latest_audios.len(),
        request_body_json,
    })
}

#[tauri::command]
async fn get_system_prompt_preview(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<SystemPromptPreview, String> {
    let preview = get_prompt_preview(input, None, state).await?;
    Ok(SystemPromptPreview {
        system_prompt: preview.preamble,
    })
}

fn archive_time_label(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return "unknown-time".to_string();
    }
    let mut normalized = s.replace('T', " ");
    if normalized.ends_with('Z') {
        normalized.pop();
    }
    if normalized.chars().count() >= 16 {
        normalized.chars().take(16).collect::<String>()
    } else {
        normalized
    }
}

fn archive_no_content_label(ui_language: &str) -> String {
    match ui_language.trim() {
        "en-US" => "No content".to_string(),
        "zh-TW" => "無內容".to_string(),
        _ => "无内容".to_string(),
    }
}

fn archive_first_user_preview(conversation: &Conversation, ui_language: &str) -> String {
    let text = conversation
        .messages
        .iter()
        .find(|m| {
            m.role == "user"
                && m.speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    != Some(SYSTEM_PERSONA_ID)
        })
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    MessagePart::Text { text } => Some(text.trim()),
                    _ => None,
                })
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let compact = clean_text(text.trim());
    if compact.is_empty() {
        archive_no_content_label(ui_language)
    } else {
        compact.chars().take(10).collect::<String>()
    }
}

fn conversation_to_archive(conversation: &Conversation) -> ConversationArchive {
    ConversationArchive {
        archive_id: conversation.id.clone(),
        archived_at: conversation
            .archived_at
            .clone()
            .unwrap_or_else(|| conversation.updated_at.clone()),
        reason: "conversation_summary".to_string(),
        source_conversation: conversation.clone(),
    }
}

fn archive_to_conversation(archive: ConversationArchive) -> Conversation {
    let mut conversation = archive.source_conversation;
    if conversation.id.trim().is_empty() {
        conversation.id = archive.archive_id;
    }
    if conversation.id.trim().is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    if conversation
        .archived_at
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        conversation.archived_at = Some(archive.archived_at);
    }
    conversation.status = "archived".to_string();
    conversation
}

#[tauri::command]
fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    conversation_service().list_archives(state.inner())
}

#[tauri::command]
fn get_archive_messages(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    conversation_service().read_archive_messages(state.inner(), &archive_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveBlockSummaryOutput {
    block_id: u32,
    message_count: usize,
    first_message_id: String,
    last_message_id: String,
    #[serde(default)]
    first_created_at: Option<String>,
    #[serde(default)]
    last_created_at: Option<String>,
    is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetArchiveBlockPageInput {
    archive_id: String,
    #[serde(default)]
    block_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveBlockPageOutput {
    blocks: Vec<ArchiveBlockSummaryOutput>,
    selected_block_id: u32,
    messages: Vec<ChatMessage>,
    has_prev_block: bool,
    has_next_block: bool,
}

#[tauri::command]
fn get_archive_block_page(
    input: GetArchiveBlockPageInput,
    state: State<'_, AppState>,
) -> Result<ArchiveBlockPageOutput, String> {
    let archive_id = input.archive_id.trim();
    if archive_id.is_empty() {
        return Err("archiveId 是必填项".to_string());
    }
    let page = conversation_service().read_archive_block_page(
        state.inner(),
        archive_id,
        input.block_id,
    )?;
    Ok(ArchiveBlockPageOutput {
        blocks: page
            .blocks
            .into_iter()
            .map(|item| ArchiveBlockSummaryOutput {
                block_id: item.block_id,
                message_count: item.message_count,
                first_message_id: item.first_message_id,
                last_message_id: item.last_message_id,
                first_created_at: item.first_created_at,
                last_created_at: item.last_created_at,
                is_latest: item.is_latest,
            })
            .collect(),
        selected_block_id: page.selected_block_id,
        messages: page.messages,
        has_prev_block: page.has_prev_block,
        has_next_block: page.has_next_block,
    })
}

#[tauri::command]
fn get_archive_summary(archive_id: String, state: State<'_, AppState>) -> Result<String, String> {
    conversation_service().read_archive_summary(state.inner(), &archive_id)
}

#[tauri::command]
fn delete_archive(archive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    conversation_service().delete_archive(state.inner(), &archive_id)
}
