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

fn resolve_chat_prompt_preview_api_config(
    app_config: &AppConfig,
    agent: &AgentProfile,
    conversation: &Conversation,
    requested_api_config_id: Option<&str>,
) -> Result<ApiConfig, String> {
    let preferred_api_config_id = if conversation_is_remote_im_contact(conversation) {
        agent_primary_chat_api_config_id(app_config, agent)
    } else {
        conversation
            .preferred_api_config_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    resolve_selected_api_config(
        app_config,
        preferred_api_config_id
            .as_deref()
            .or_else(|| requested_api_config_id.map(str::trim).filter(|value| !value.is_empty())),
    )
    .ok_or_else(|| "No API config available".to_string())
}

#[tauri::command]
async fn get_prompt_preview(
    input: SessionSelector,
    preview_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<PromptPreview, String> {
    get_prompt_preview_inner(input, preview_mode, state.inner()).await
}

async fn get_prompt_preview_inner(
    input: SessionSelector,
    preview_mode: Option<String>,
    state: &AppState,
) -> Result<PromptPreview, String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let app_config = runtime_snapshot.config;
    let agents = runtime_snapshot.agents;
    let response_style_id = state_service_get_response_style_id(state)?;
    let preview_mode = parse_prompt_preview_mode(preview_mode.as_deref());
    let requested_conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let mut conversation = match conversation_service_v2()
        .get_conversation_snapshot(state, requested_conversation_id)
    {
        Ok(conversation) => conversation,
        Err(_) => delegate_runtime_thread_conversation_get_any(
            state,
            requested_conversation_id,
        )?
        .ok_or_else(|| format!("指定会话不存在或不可用：{requested_conversation_id}"))?,
    };
    if conversation_is_archived(&conversation) {
        return Err(format!("指定会话不存在或不可用：{requested_conversation_id}"));
    }
    let agent =
        resolve_conversation_bound_agent(&conversation, &agents)?
            .clone();
    let api_config = match preview_mode {
        PromptPreviewMode::Chat => resolve_chat_prompt_preview_api_config(
            &app_config,
            &agent,
            &conversation,
            input.api_config_id.as_deref(),
        )?,
        PromptPreviewMode::Compaction | PromptPreviewMode::Archive => {
            resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
                .ok_or_else(|| "No API config available".to_string())?
        }
    };
    let mut resolved_api = resolve_api_config(&app_config, Some(&api_config.id))?;
    let latest_user_message = conversation.messages.iter().rev().find(|message| {
        prompt_role_for_message(message, &agent.id).as_deref() == Some("user")
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
    runtime_log_info(format!(
        "[请求体预览][当前消息提取] mode={:?} requested_conversation_id={:?} selected_conversation_id={} agent_id={} latest_user_message_id={} latest_user_retrieved_memory_ids={:?}",
        preview_mode,
        input.conversation_id,
        conversation.id,
        agent.id,
        latest_user_message_id,
        latest_user_retrieved_memory_ids
    ));

    let user_name = agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID || a.is_built_in_user)
        .map(|a| a.name.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_user_alias);
    let user_intro = agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID || a.is_built_in_user)
        .map(|a| a.system_prompt.trim().to_string())
        .unwrap_or_default();
    let mut prepared = match preview_mode {
        PromptPreviewMode::Chat => build_prepared_prompt_for_mode(
            if conversation_is_delegate(&conversation) {
                PromptBuildMode::Delegate
            } else {
                PromptBuildMode::Chat
            },
            &conversation,
            &agent,
            &agents,
            &user_name,
            &user_intro,
            &response_style_id,
            &app_config.ui_language,
            Some(&state.data_path),
            None,
            None,
            Some(ChatPromptOverrides::default()),
            Some(state),
            Some(&api_config),
            Some(&resolved_api),
        )?,
        PromptPreviewMode::Compaction | PromptPreviewMode::Archive => {
            let owner_agent_id =
                resolve_archive_owner_agent_id(&app_config, &agents, &conversation)?;
            let owner_agent = agents
                .iter()
                .find(|item| item.id == owner_agent_id)
                .cloned()
                .ok_or_else(|| "Selected agent not found.".to_string())?;
            build_prepared_prompt_for_mode(
                PromptBuildMode::Chat,
                &conversation,
                &owner_agent,
                &agents,
                &user_name,
                &user_intro,
                &response_style_id,
                &app_config.ui_language,
                Some(&state.data_path),
                None,
                None,
                Some(ChatPromptOverrides {
                    latest_user_intent: Some(LatestUserPayloadIntent::SummaryContext {
                        scene: if preview_mode == PromptPreviewMode::Compaction {
                            SummaryContextScene::Compaction
                        } else {
                            SummaryContextScene::Archive
                        },
                        user_alias: user_name.clone(),
                    }),
                    latest_images: Some(Vec::new()),
                    latest_audios: Some(Vec::new()),
                    ..Default::default()
                }),
                Some(state),
                Some(&api_config),
                Some(&resolved_api),
            )?
        }
    };
    let model_name = if api_config.model.trim().is_empty() {
        resolved_api.model.clone()
    } else {
        api_config.model.trim().to_string()
    };
    maybe_prepare_aliyun_multimodal_urls_for_candidate(
        state,
        &api_config,
        &mut resolved_api,
        &model_name,
        &mut prepared,
        &mut conversation,
        false,
        false,
    )
    .await?;
    let _ = apply_prompt_image_fallbacks_to_prepared(
        state,
        &conversation.id,
        &app_config,
        &api_config,
        &mut prepared,
    )
    .await?;
    let _ = replace_disabled_multimodal_with_text(
        &mut prepared,
        api_config.enable_image,
        api_config.enable_audio,
    );

    let request_body_json =
        serde_json::to_string_pretty(&prepared_prompt_to_messages_json(&prepared))
            .map_err(|err| format!("序列化请求预览失败：{err}"))?;
    runtime_log_info(format!(
        "[请求体预览] 完成: mode={:?} conversation_id={} latest_user_text_len={} latest_images={} latest_audios={} request_has_memory_board={} request_len={}",
        preview_mode,
        conversation.id,
        prepared.latest_user_text.len(),
        prepared.latest_images.len(),
        prepared.latest_audios.len(),
        request_body_json.contains("<memory_context>"),
        request_body_json.len()
    ));
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
    let preview = get_prompt_preview_inner(input, None, state.inner()).await?;
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

fn conversation_to_archive(conversation: &Conversation) -> ConversationArchive {
    let mut source_conversation = conversation.clone();
    source_conversation.fast_request_turns.clear();
    ConversationArchive {
        archive_id: conversation.id.clone(),
        archived_at: conversation
            .archived_at
            .clone()
            .unwrap_or_else(|| conversation.updated_at.clone()),
        reason: "conversation_summary".to_string(),
        source_conversation,
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
    conversation.fast_request_turns.clear();
    conversation
}

#[tauri::command]
async fn list_archives(state: State<'_, AppState>) -> Result<Vec<ArchiveSummary>, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || list_archives_inner(&app_state))
        .await
        .map_err(|err| format!("读取归档列表任务异常：{err}"))?
}

fn list_archives_inner(state: &AppState) -> Result<Vec<ArchiveSummary>, String> {
    conversation_service_v2().list_archives(state)
}

#[tauri::command]
async fn get_archive_messages(
    archive_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_archive_messages(&app_state, &archive_id)
    })
    .await
    .map_err(|err| format!("读取归档消息任务异常：{err}"))?
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
async fn get_archive_block_page(
    input: GetArchiveBlockPageInput,
    state: State<'_, AppState>,
) -> Result<ArchiveBlockPageOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || get_archive_block_page_inner(input, &app_state))
        .await
        .map_err(|err| format!("读取归档块分页任务异常：{err}"))?
}

fn get_archive_block_page_inner(
    input: GetArchiveBlockPageInput,
    state: &AppState,
) -> Result<ArchiveBlockPageOutput, String> {
    let archive_id = input.archive_id.trim();
    if archive_id.is_empty() {
        return Err("archiveId 是必填项".to_string());
    }
    let page = conversation_service_v2().get_archive_block_page(
        state,
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
fn delete_archive(archive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_archive_inner(state.inner(), &archive_id)
}

fn delete_archive_inner(state: &AppState, archive_id: &str) -> Result<(), String> {
    conversation_service_v2().delete_archive(state, archive_id)
}

#[tauri::command]
fn unarchive_archive(archive_id: String, state: State<'_, AppState>) -> Result<(), String> {
    unarchive_archive_inner(state.inner(), &archive_id)
}

fn unarchive_archive_inner(state: &AppState, archive_id: &str) -> Result<(), String> {
    conversation_service_v2().unarchive_archive(state, archive_id)?;
    flush_pending_persists_blocking(state)?;
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        runtime_log_warn(format!(
            "[归档] 失败，任务=取消归档后刷新会话概览，archive_id={}，error={}",
            archive_id, err
        ));
    }
    Ok(())
}

#[cfg(test)]
mod fast_request_archive_tests {
    use super::*;

    fn test_fast_request_turn() -> FastRequestTurn {
        FastRequestTurn {
            id: "fast-request-a".to_string(),
            kind: "remote_im".to_string(),
            request_text: "request".to_string(),
            response_text: "response".to_string(),
            success: true,
            error: None,
            model_name: Some("quick-model".to_string()),
            duration_ms: Some(42),
            created_at: "2026-06-28T00:00:00Z".to_string(),
        }
    }

    fn test_conversation_with_fast_request() -> Conversation {
        let mut conversation = build_conversation_record(
            "",
            DEFAULT_AGENT_ID,
            "测试会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.fast_request_turns.push(test_fast_request_turn());
        conversation
    }

    #[test]
    fn conversation_to_archive_should_clear_fast_request_turns() {
        let conversation = test_conversation_with_fast_request();
        let archive = conversation_to_archive(&conversation);

        assert_eq!(conversation.fast_request_turns.len(), 1);
        assert!(archive.source_conversation.fast_request_turns.is_empty());
    }

    #[test]
    fn archive_to_conversation_should_clear_fast_request_turns() {
        let conversation = test_conversation_with_fast_request();
        let archive = ConversationArchive {
            archive_id: conversation.id.clone(),
            archived_at: now_iso(),
            reason: "test".to_string(),
            source_conversation: conversation,
        };
        let restored = archive_to_conversation(archive);

        assert_eq!(restored.status, "archived");
        assert!(restored.fast_request_turns.is_empty());
    }
}
