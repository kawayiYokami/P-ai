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
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;

    let mut app_config = read_config(&state.config_path)?;
    let api_config = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
        .ok_or_else(|| "No API config available".to_string())?;
    let mut resolved_api = resolve_api_config(&app_config, Some(&api_config.id))?;

    let mut data = read_app_data(&state.data_path)?;
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
        .and_then(|conversation_id| {
            data.conversations
                .iter()
                .find(|item| {
                    item.id == conversation_id
                        && item.summary.trim().is_empty()
                        && !conversation_is_delegate(item)
                })
                .cloned()
        })
        .or_else(|| {
            latest_active_conversation_index(&data, "", &effective_agent_id)
                .and_then(|idx| data.conversations.get(idx).cloned())
        })
        .unwrap_or_else(|| Conversation {
            id: "preview".to_string(),
            title: "Preview".to_string(),
            agent_id: effective_agent_id.clone(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
    let latest_user_message = conversation
        .messages
        .iter()
        .rev()
        .find(|message| prompt_role_for_message(message, &effective_agent_id).as_deref() == Some("user"));
    let latest_user_message_id = latest_user_message
        .map(|message| message.id.clone())
        .unwrap_or_default();
    let latest_user_retrieved_memory_ids = latest_user_message
        .and_then(|message| {
            message
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("retrieved_memory_ids").or_else(|| meta.get("recallMemoryIds")))
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
    let last_archive_summary = data
        .conversations
        .iter()
        .rev()
        .find(|c| !conversation_is_delegate(c) && !c.summary.trim().is_empty())
        .map(|c| c.summary.clone());
    let mut prepared = match preview_mode {
        PromptPreviewMode::Chat => {
            let mut system_preamble_blocks = vec![build_hidden_skill_snapshot_block(&state)];
            if let Some(workspace_agents_block) = build_workspace_agents_md_block(&conversation, &state) {
                system_preamble_blocks.push(workspace_agents_block);
            }
            build_prepared_prompt_for_mode(
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
                terminal_prompt_trusted_roots_block(&state, &api_config, Some(&conversation)),
                Some(ChatPromptOverrides {
                    system_preamble_blocks,
                    ..Default::default()
                }),
                Some(&*state),
                Some(&resolved_api),
                Some(data.pdf_read_mode == "image" && api_config.enable_image),
            )
        }
        PromptPreviewMode::Compaction | PromptPreviewMode::Archive => {
            let host_agent_id = choose_archive_host_agent_id(&data, &conversation, &effective_agent_id);
            let host_agent = data
                .agents
                .iter()
                .find(|item| item.id == host_agent_id)
                .cloned()
                .ok_or_else(|| "Selected agent not found.".to_string())?;
            build_prepared_prompt_for_mode(
                PromptBuildMode::SummaryContext,
                &conversation,
                &host_agent,
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
                    latest_user_text: Some(build_summary_context_requirement_block(
                        if preview_mode == PromptPreviewMode::Compaction {
                            SummaryContextScene::Compaction
                        } else {
                            SummaryContextScene::Archive
                        },
                    )),
                    latest_user_meta_text: Some(build_summary_context_memory_block(
                        &host_agent,
                        &user_name,
                        &build_user_profile_memory_board(&state.data_path, &host_agent)?
                            .unwrap_or_else(|| "（无）".to_string()),
                    )),
                    latest_user_extra_blocks: {
                        let mut blocks = vec![build_summary_context_json_contract_block(
                            if preview_mode == PromptPreviewMode::Compaction {
                                SummaryContextScene::Compaction
                            } else {
                                SummaryContextScene::Archive
                            },
                        )];
                        if let Some(todo_block) = build_summary_context_todo_block(&conversation) {
                            blocks.push(todo_block);
                        }
                        blocks
                    },
                    latest_images: Some(Vec::new()),
                    latest_audios: Some(Vec::new()),
                    ..Default::default()
                }),
                Some(&*state),
                Some(&resolved_api),
                Some(data.pdf_read_mode == "image" && api_config.enable_image),
            )
        }
    };
    drop(guard);

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

    let request_body_json = serde_json::to_string_pretty(&prepared_prompt_to_messages_json(&prepared))
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
        .find(|m| m.role == "user")
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

fn archived_conversations_from_data(data: &AppData) -> Vec<ConversationArchive> {
    let mut out = data
        .conversations
        .iter()
        .filter(|c| !c.summary.trim().is_empty())
        .map(conversation_to_archive)
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    out
}

fn archive_to_conversation(archive: ConversationArchive) -> Conversation {
    let mut conversation = archive.source_conversation;
    if conversation.id.trim().is_empty() {
        conversation.id = archive.archive_id;
    }
    if conversation.id.trim().is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    if conversation.archived_at.as_deref().unwrap_or("").trim().is_empty() {
        conversation.archived_at = Some(archive.archived_at);
    }
    conversation.status = "archived".to_string();
    conversation
}

#[tauri::command]
fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;

    let data = state_read_app_data_cached(&state)?;
    let app_config = read_config(&state.config_path)?;
    drop(guard);

    let mut summaries = data
        .conversations
        .iter()
        .filter(|c| !c.summary.trim().is_empty())
        .map(|archive| {
            let api_config_id = department_for_agent_id(&app_config, &archive.agent_id)
                .map(department_primary_api_config_id)
                .unwrap_or_default();
            ArchiveSummary {
                archive_id: archive.id.clone(),
                archived_at: archive
                    .archived_at
                    .clone()
                    .unwrap_or_else(|| archive.updated_at.clone()),
                title: archive_first_user_preview(archive, &app_config.ui_language),
                message_count: archive.messages.len(),
                api_config_id,
                agent_id: archive.agent_id.clone(),
            }
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    Ok(summaries)
}

#[tauri::command]
fn get_archive_messages(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;

    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == archive_id && !c.summary.trim().is_empty())
        .ok_or_else(|| "Archive not found".to_string())?;

    let mut messages = archive.messages.clone();
    materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
    Ok(messages)
}

#[tauri::command]
fn get_archive_summary(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;

    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == archive_id && !c.summary.trim().is_empty())
        .ok_or_else(|| "Archive not found".to_string())?;

    Ok(archive.summary.clone())
}

#[tauri::command]
fn delete_archive(archive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if archive_id.trim().is_empty() {
        return Err("archiveId is required".to_string());
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;

    let mut data = state_read_app_data_cached(&state)?;
    let before = data.conversations.len();
    data.conversations
        .retain(|c| !(c.id == archive_id && !c.summary.trim().is_empty()));

    if data.conversations.len() == before {
        drop(guard);
        return Err("Archive not found".to_string());
    }

    state_write_app_data_cached(&state, &data)?;
    drop(guard);
    Ok(())
}
