fn ide_chat_load_config_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(load_config_inner(state)?)
}

fn ide_chat_get_department_default_draft_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let department_id = ide_chat_parse_param_field::<String>(params, "departmentId")?;
    ide_chat_serialize(get_department_default_draft_inner(state, &department_id)?)
}

fn ide_chat_load_app_bootstrap_snapshot_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(read_app_bootstrap_snapshot(state)?)
}

fn ide_chat_get_app_runtime_info_for_web_settings(state: &AppState) -> Result<Value, String> {
    let data_path = state.data_path.clone();
    let config_path = state.config_path.clone();
    let app_root = app_root_from_data_path(&data_path);
    let is_portable = detect_portable_runtime_root().is_some();
    ide_chat_serialize(AppRuntimeInfo {
        app_root: app_root.to_string_lossy().to_string(),
        data_path: data_path.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        is_portable,
    })
}

fn ide_chat_save_config_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    ide_context_runtime: &IdeContextRuntime,
    params: Value,
) -> Result<Value, String> {
    let config = ide_chat_parse_param_field::<AppConfig>(params, "config")?;
    ide_chat_serialize(save_config_inner(config, app, state, ide_context_runtime)?)
}

fn ide_chat_load_agents_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(load_agents_inner(state)?)
}

async fn ide_chat_stt_transcribe_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<SttTranscribeInput>(params)?;
    ide_chat_serialize(stt_transcribe_inner(input, state).await?)
}

async fn ide_chat_list_unarchived_conversations_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    let app_state = state.clone();
    let summaries = tokio::task::spawn_blocking(move || list_unarchived_conversations_blocking(&app_state))
        .await
        .map_err(|err| format!("读取未归档会话列表任务异常：{err}"))??;
    ide_chat_serialize(summaries)
}

fn ide_chat_remote_im_list_contact_conversations_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(conversation_service_v2().list_remote_im_contact_conversations(state)?)
}

fn ide_chat_list_delegate_conversations_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(list_delegate_conversations_inner(state)?)
}

async fn ide_chat_get_prompt_preview_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SessionSelector>(params.clone(), "input")?;
    let preview_mode = ide_chat_parse_optional_param_field::<String>(params, "previewMode")?;
    ide_chat_serialize(get_prompt_preview_inner(input, preview_mode, state).await?)
}

async fn ide_chat_get_system_prompt_preview_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SessionSelector>(params, "input")?;
    ide_chat_serialize(get_prompt_preview_inner(input, None, state).await.map(|preview| {
        SystemPromptPreview {
            system_prompt: preview.preamble,
        }
    })?)
}

fn ide_chat_save_agents_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SaveAgentsInput>(params, "input")?;
    ide_chat_serialize(save_agents_inner(input, app, state)?)
}

fn ide_chat_load_chat_settings_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(load_chat_settings_inner(state)?)
}

fn ide_chat_save_chat_settings_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ChatSettings>(params, "input")?;
    let patch = ChatSettingsPatch {
        assistant_department_agent_id: Some(input.assistant_department_agent_id),
        user_alias: Some(input.user_alias),
        response_style_id: Some(input.response_style_id),
        pdf_read_mode: Some(input.pdf_read_mode),
        background_voice_screenshot_keywords: Some(input.background_voice_screenshot_keywords),
        background_voice_screenshot_mode: Some(input.background_voice_screenshot_mode),
        instruction_presets: Some(input.instruction_presets),
    };
    ide_chat_serialize(patch_chat_settings_inner(patch, app, state)?)
}

fn ide_chat_patch_chat_settings_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ChatSettingsPatch>(params, "input")?;
    ide_chat_serialize(patch_chat_settings_inner(input, app, state)?)
}

fn ide_chat_patch_conversation_api_settings_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConversationApiSettingsPatch>(params, "input")?;
    ide_chat_serialize(patch_conversation_api_settings_inner(input, app, state)?)
}

fn ide_chat_save_conversation_api_settings_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConversationApiSettings>(params, "input")?;
    let patch = ConversationApiSettingsPatch {
        assistant_department_api_config_id: Some(input.assistant_department_api_config_id),
        vision_api_config_id: Some(input.vision_api_config_id),
        tool_review_api_config_id: Some(input.tool_review_api_config_id),
        stt_api_config_id: Some(input.stt_api_config_id),
        stt_auto_send: Some(input.stt_auto_send),
    };
    ide_chat_serialize(patch_conversation_api_settings_inner(patch, app, state)?)
}

async fn ide_chat_refresh_models_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RefreshModelsInput>(params, "input")?;
    ide_chat_serialize(refresh_models_inner(state, input).await?)
}

async fn ide_chat_quick_genai_chat_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<QuickGenaiChatInput>(params, "input")?;
    ide_chat_serialize(quick_genai_chat_inner(state, input).await?)
}

async fn ide_chat_fetch_model_metadata_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<FetchModelMetadataInput>(params, "input")?;
    ide_chat_serialize(fetch_model_metadata_inner(state, input).await?)
}

async fn ide_chat_test_embedding_connection_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TestEmbeddingConnectionInput>(params, "input")?;
    ide_chat_serialize(test_embedding_connection_inner(input).await?)
}

async fn ide_chat_test_rerank_connection_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TestRerankConnectionInput>(params, "input")?;
    ide_chat_serialize(test_rerank_connection_inner(input).await?)
}

async fn ide_chat_test_voice_connection_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TestVoiceConnectionInput>(params, "input")?;
    ide_chat_serialize(test_voice_connection_inner(input).await?)
}

fn ide_chat_resolve_model_adapter_kind_for_web_settings(params: Value) -> Result<Value, String> {
    let (model_name, base_url, request_format) = match params {
        Value::Object(mut map) => {
            let model_name = map
                .remove("modelName")
                .or_else(|| map.remove("model_name"))
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_default();
            let base_url = map
                .remove("baseUrl")
                .or_else(|| map.remove("base_url"))
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_default();
            let request_format = map
                .remove("requestFormat")
                .or_else(|| map.remove("request_format"))
                .and_then(|value| value.as_str().and_then(RequestFormat::from_str))
                .unwrap_or(RequestFormat::Auto);
            (model_name, base_url, request_format)
        }
        _ => (String::new(), String::new(), RequestFormat::Auto),
    };
    ide_chat_serialize(resolve_model_adapter_kind_label(
        request_format,
        &base_url,
        &model_name,
    ))
}

fn ide_chat_get_image_text_cache_stats_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(get_image_text_cache_stats_inner(state)?)
}

fn ide_chat_clear_image_text_cache_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(clear_image_text_cache_inner(state)?)
}

async fn ide_chat_list_tool_catalog_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(list_tool_catalog_inner(state).await?)
}

async fn ide_chat_list_department_permission_catalog_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(list_department_permission_catalog_inner(state).await?)
}

async fn ide_chat_web_access_info_for_web_settings(
    app: &AppHandle,
    state: &AppState,
    ide_context_runtime: &IdeContextRuntime,
) -> Result<Value, String> {
    ide_chat_serialize(get_web_access_info_inner(app, state, ide_context_runtime, false).await?)
}

include!("memory_methods.rs");

fn ide_chat_task_list_tasks_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(task_list_tasks_inner(state)?)
}

fn ide_chat_task_get_task_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskGetInput>(params, "input")?;
    ide_chat_serialize(task_get_task_inner(input, state)?)
}

fn ide_chat_task_create_task_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskCreateInput>(params, "input")?;
    ide_chat_serialize(task_create_task_inner(input, state)?)
}

fn ide_chat_task_update_task_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskUpdateInput>(params, "input")?;
    ide_chat_serialize(task_update_task_inner(input, state)?)
}

fn ide_chat_task_complete_task_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskCompleteInput>(params, "input")?;
    ide_chat_serialize(task_complete_task_inner(input, state)?)
}

fn ide_chat_task_delete_task_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskDeleteInput>(params, "input")?;
    ide_chat_serialize(task_delete_task_inner(input, state)?)
}

fn ide_chat_task_list_run_logs_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskRunLogListInput>(params, "input")?;
    ide_chat_serialize(task_list_run_logs_inner(Some(input), state)?)
}

async fn ide_chat_task_optimize_draft_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<TaskOptimizeDraftInput>(params, "input")?;
    ide_chat_serialize(task_optimize_draft_internal(input, state).await?)
}

include!("mcp_methods.rs");

include!("catalog_methods.rs");

async fn ide_chat_get_usage_overview_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(start_usage_overview_refresh_if_needed(state.clone(), false).await)
}

async fn ide_chat_refresh_usage_overview_for_web_settings(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(start_usage_overview_refresh_if_needed(state.clone(), true).await)
}

async fn ide_chat_get_usage_trail_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let (view, year) = match params {
        Value::Object(map) => {
            let view = map
                .get("view")
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_else(|| "today".to_string());
            let year = map
                .get("year")
                .and_then(|value| value.as_str().map(ToOwned::to_owned));
            (view, year)
        }
        _ => ("today".to_string(), None),
    };
    let query = UsageTrailWallQuery { view, year };
    let state = state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || build_usage_trail_wall(&state, &query))
        .await
        .map_err(|err| format!("计算足迹墙失败：{err}"))?
        .map_err(|err| err.to_string())?;
    ide_chat_serialize(result)
}

fn ide_chat_list_recent_llm_round_logs_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(list_recent_llm_round_logs_inner(state)?)
}

fn ide_chat_get_recent_llm_round_log_section_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let (id, section) = match params {
        Value::Object(mut map) => {
            let id = map
                .remove("id")
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_default();
            let section = map
                .remove("section")
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .unwrap_or_default();
            (id, section)
        }
        _ => (String::new(), String::new()),
    };
    ide_chat_serialize(get_recent_llm_round_log_section_inner(state, id, section)?)
}

fn ide_chat_clear_recent_llm_round_logs_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(clear_recent_llm_round_logs_inner(state)?)
}

fn ide_chat_set_github_update_method_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let update_method = match params {
        Value::Object(mut map) => map
            .remove("updateMethod")
            .or_else(|| map.remove("update_method"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default(),
        _ => String::new(),
    };
    ide_chat_serialize(set_github_update_method_inner(update_method, app, state)?)
}

fn ide_chat_set_skipped_github_update_version_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let version = match params {
        Value::Object(mut map) => map
            .remove("version")
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .unwrap_or_default(),
        _ => String::new(),
    };
    ide_chat_serialize(set_skipped_github_update_version_inner(version, app, state)?)
}

fn ide_chat_set_agent_private_memory_enabled_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetAgentPrivateMemoryEnabledInput>(params, "input")?;
    ide_chat_serialize(set_agent_private_memory_enabled_inner(input, state)?)
}

fn ide_chat_save_agent_avatar_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SaveAgentAvatarInput>(params, "input")?;
    ide_chat_serialize(save_agent_avatar_inner(input, state)?)
}

fn ide_chat_clear_agent_avatar_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ClearAgentAvatarInput>(params, "input")?;
    clear_agent_avatar_inner(input, state)?;
    Ok(Value::Null)
}

fn ide_chat_convert_private_agent_to_main_for_web_settings(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConvertPrivateAgentToMainInput>(params, "input")?;
    ide_chat_serialize(convert_private_agent_to_main_inner(input, app, state)?)
}

fn ide_chat_check_tools_status_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CheckToolsStatusInput>(params, "input")?;
    ide_chat_serialize(check_tools_status_inner(input, state)?)
}

fn ide_chat_list_terminal_shell_candidates_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    let (preferred_kind, current, options) = terminal_shell_candidates_for_ui(state);
    Ok(serde_json::json!({
        "preferredKind": preferred_kind,
        "currentKind": current.kind,
        "currentPath": current.path,
        "options": options,
    }))
}

async fn ide_chat_generate_image_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let request = ide_chat_parse_param_field::<ImageGenerationRequest>(params, "request")?;
    ide_chat_serialize(generate_images(state, request).await?)
}

async fn ide_chat_get_storage_usage_overview_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(start_storage_overview_refresh_if_needed(state.clone(), false).await)
}

async fn ide_chat_refresh_storage_usage_overview_for_web_settings(
    state: &AppState,
) -> Result<Value, String> {
    ide_chat_serialize(start_storage_overview_refresh_if_needed(state.clone(), true).await)
}

fn ide_chat_cleanup_storage_legacy_items_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CleanupStorageLegacyItemsInput>(params, "input")?;
    ide_chat_serialize(cleanup_storage_legacy_items_inner(state, input)?)
}

async fn ide_chat_codex_get_auth_status_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CodexAuthStatusInput>(params, "input")?;
    ide_chat_serialize(codex_get_auth_status(input).await?)
}

async fn ide_chat_codex_start_oauth_login_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CodexStartOAuthLoginInput>(params, "input")?;
    ide_chat_serialize(codex_start_oauth_login(input).await?)
}

async fn ide_chat_codex_get_rate_limits_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CodexGetRateLimitsInput>(params, "input")?;
    ide_chat_serialize(codex_get_rate_limits(input).await?)
}

async fn ide_chat_codex_consume_rate_limit_reset_credit_for_web_settings(
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CodexGetRateLimitsInput>(params, "input")?;
    ide_chat_serialize(codex_consume_rate_limit_reset_credit(input).await?)
}

fn ide_chat_codex_logout_for_web_settings(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CodexLogoutInput>(params, "input")?;
    ide_chat_serialize(codex_logout(input)?)
}

fn ide_chat_remote_im_default_group_response_guidance_for_web_settings() -> Result<Value, String> {
    ide_chat_serialize(default_remote_im_contact_response_guidance())
}

fn ide_chat_remote_im_patch_contact_settings_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactSettingsPatchInput>(params, "input")?;
    ide_chat_serialize(remote_im_patch_contact_settings_inner(state, input)?)
}

fn ide_chat_remote_im_reconfigure_channel_behavior_for_web_settings(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let channel_id = params
        .get("channelId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    ide_chat_serialize(remote_im_reconfigure_channel_behavior_inner(state, channel_id))
}

include!("remote_im_methods.rs");
