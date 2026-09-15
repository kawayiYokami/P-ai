const WEB_NATIVE_CAPABILITY_UNAVAILABLE: &str = "WEB_NATIVE_CAPABILITY_UNAVAILABLE";

fn ide_chat_web_native_only_method(method: &str) -> bool {
    matches!(
        method,
            | "list_file_reader_directory"
            | "list_file_reader_directory_open_targets"
            | "read_file_reader_file"
            | "read_file_reader_file_block"
            | "read_plan_file_content"
            | "open_file_reader_directory_target"
            | "open_file_reader_directory_shell"
            | "open_file_with_default_program"
            | "open_local_file_directory"
            | "open_file_in_vscode"
            | "save_local_file_as"
            | "open_workspace_file"
            | "open_storage_usage_item_directory"
            | "open_chat_shell_workspace_dir"
            | "mcp_open_workspace_dir"
            | "skill_open_workspace_dir"
            | "skill_open_item_dir"
            | "copy_local_chat_image_to_clipboard"
            | "save_local_chat_image_as"
            | "export_archive_to_file"
            | "archives.export"
            | "conversation.importShare"
            | "export_memories_to_path"
            | "export_agent_private_memories"
            | "write_base64_file_to_path"
            | "write_utf8_text_file_to_path"
            | "queue_local_file_attachment"
            | "attachment_transfer_begin"
            | "attachment_transfer_chunk"
            | "attachment_transfer_complete"
            | "attachment_transfer_abort"
            | "attachment_ingest_local_path"
            | "update_file_reader_watch_targets"
            | "migrate_shell_workspace_directory"
            | "desktop_screenshot"
            | "demo_send_native_notification"
            | "demo_restart_app"
            | "xcap"
            | "start_current_window_drag"
            | "toggle_current_window_maximize"
            | "hide_current_window"
            | "update_record_hotkey"
            | "update_record_background_wake"
            | "install_host_runtime_prerequisite"
            | "get_host_runtime_prerequisites"
            | "reset_chat_shell_workspace"
            | "get_default_chat_shell_workspace_path"
            | "open_external_url"
            | "show_main_window"
            | "show_chat_window"
            | "show_archives_window"
            | "open_runtime_logs_window"
            | "sync_tray_icon"
            | "get_github_update_state"
            | "check_github_update"
            | "start_github_update"
            | "cancel_github_update"
            | "apply_prepared_github_update"
            | "get_portable_pending_manual_replace"
            | "dismiss_portable_pending_manual_replace"
            | "retry_portable_pending_manual_replace"
            | "open_portable_pending_dir"
            | "bind_active_chat_view_stream"
            | "probe_active_chat_view_stream"
            | "unbind_active_chat_view_stream"
            | "clear_window_chat_view_stream_bindings_command"
            | "set_chat_window_active"
            | "open_file_reader_window_command"
            | "read_local_binary_file"
            | "set_chat_window_side_expanded"
            | "show_quick_setup_window"
            | "complete_quick_setup_and_open_chat"
            | "list_system_fonts"
            | "list_genai_chat_adapters"
    )
}

fn ide_chat_web_native_only_error(method: &str) -> String {
    format!(
        "{}: Web 端不支持本机能力：{}",
        WEB_NATIVE_CAPABILITY_UNAVAILABLE, method
    )
}

fn ide_chat_upsert_ide_context_command(
    state: &AppState,
    params: Value,
    ide_context_runtime: &IdeContextRuntime,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<UpsertIdeContextSnapshotInput>(params)?;
    let (client_id, updated_at) =
        upsert_ide_context_snapshot_internal(input, ide_context_runtime)?;
    emit_ide_context_updated(state, &client_id, &updated_at);
    Ok(serde_json::json!({
        "clientId": client_id,
        "updatedAt": updated_at,
    }))
}

async fn ide_chat_handle_jsonrpc_request(
    request: IdeChatJsonRpcRequest,
    state: &AppState,
    app: &AppHandle,
    ide_context_runtime: &IdeContextRuntime,
    client_id: &str,
    opened_conversation_id: &mut Option<String>,
) -> Value {
    if request.jsonrpc.trim() != "2.0" {
        return ide_chat_jsonrpc_error(request.id, -32600, "jsonrpc must be 2.0");
    }
    if ide_chat_web_native_only_method(&request.method) {
        return ide_chat_jsonrpc_error(
            request.id,
            -32010,
            &ide_chat_web_native_only_error(&request.method),
        );
    }
    let sidebar_label = ide_chat_sidebar_window_label(client_id);
    let sidebar_viewer_id = chat_viewer_id_for_window_label(&sidebar_label)
        .unwrap_or_else(|| format!("web:{}", client_id.trim()));
    let result = match request.method.as_str() {
        "bridge.ping" => Ok(serde_json::json!({
            "ok": true,
            "ts": chrono::Utc::now().to_rfc3339(),
        })),
        "conversation.list" => ide_chat_conversation_list(state, &sidebar_viewer_id),
        "conversation.changedSince" => ide_chat_conversation_changed_since(state, request.params).await,
        "conversation.blockPage" => ide_chat_conversation_block_page(state, request.params).await,
        "conversation.fastRequestTurns" => ide_chat_conversation_fast_request_turns(state, request.params),
        "conversation.runtimeSnapshot" => ide_chat_conversation_runtime_snapshot(state, request.params),
        "conversation.resumeSubscription" => ide_chat_resume_sidebar_subscription(
            state,
            request.params,
            client_id,
            opened_conversation_id,
        ),
        "conversation.streamProbe" => ide_chat_stream_probe(request.params, client_id, opened_conversation_id),
        "conversation.freshnessSnapshot" => ide_chat_conversation_freshness_snapshot(state, request.params).await,
        "conversation.markRead" => ide_chat_mark_conversation_read(state, request.params).await,
        "conversation.messageById" => ide_chat_conversation_message_by_id_command(state, request.params).await,
        "conversation.messagesBefore" => ide_chat_conversation_messages_before_command(state, request.params).await,
        "conversation.compactionSegmentBefore" => ide_chat_conversation_compaction_segment_before_command(state, request.params).await,
        "conversation.messagesAfterAsync" =>
            ide_chat_parse_param_field::<RequestConversationMessagesAfterAsyncInput>(
                request.params,
                "input",
            )
            .and_then(|input| request_conversation_messages_after_async_inner(input, state))
            .and_then(ide_chat_serialize),
        "conversation.setActive" => ide_chat_set_active_conversation_command(state, request.params),
        "conversation.create" => ide_chat_create_conversation(state, request.params)
            .await
            .and_then(|result| {
                if let Some(conversation_id) = result
                    .get("conversationId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    ide_chat_register_sidebar_conversation(
                        state,
                        conversation_id,
                        &sidebar_label,
                        opened_conversation_id,
                    )?;
                }
                Ok(result)
            }),
        "conversation.openDraft" => ide_chat_open_draft_conversation(state, request.params)
            .await
            .and_then(|result| {
                if let Some(conversation_id) = result
                    .get("conversationId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    ide_chat_register_sidebar_conversation(
                        state,
                        conversation_id,
                        &sidebar_label,
                        opened_conversation_id,
                    )?;
                }
                Ok(result)
            }),
        "conversation.updateDraft" => ide_chat_update_draft_conversation(state, request.params).await,
        "conversation.createSide" => ide_chat_create_side_chat_conversation(state, request.params).await,
        "conversation.createOptions" => ide_chat_create_conversation_options(state),
        "workspace.permission" => ide_chat_workspace_permission(state, request.params),
        "workspace.permission.select" => ide_chat_select_workspace_permission(state, request.params),
        "workspace.ensureHostRoot" => ide_chat_parse_params::<IdeChatConversationInput>(request.params)
            .and_then(|input| {
                let workspace_path = input
                    .workspace_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| "workspacePath is required".to_string())?;
                ide_chat_ensure_sidebar_workspace(
                    state,
                    &input.conversation_id,
                    workspace_path,
                    input.workspace_name.as_deref(),
                )?;
                Ok(serde_json::json!({ "conversationId": input.conversation_id }))
            }),
        "workspace.layout.save" => ide_chat_workspace_layout_save(state, request.params),
        "workspace.list" => ide_chat_workspace_list(state, request.params),
        "workspace.directory.list" => ide_chat_workspace_directory_list(state, request.params).await,
        "workspace.gitRootCheck" => ide_chat_workspace_git_root_check(request.params).await,
        // 旧命令保留为兼容别名，但必须落到同一套工作区实现；不再将其视为
        // Web 不可用的 App 专属能力。
        "get_chat_shell_workspace" => ide_chat_parse_workspace_params::<ChatShellWorkspaceInput>(request.params)
            .and_then(|input| get_chat_shell_workspace_inner(input, state))
            .and_then(ide_chat_serialize),
        "update_chat_shell_workspace_layout" => ide_chat_parse_workspace_params::<SaveChatShellWorkspacesInput>(request.params)
            .and_then(|input| update_chat_shell_workspace_layout_inner(input, state))
            .and_then(ide_chat_serialize),
        "check_git_workspace_root" => ide_chat_workspace_git_root_check(request.params).await,
        // git 面板：Web 与 channel 双接口，与 tauri 命令共用同一套 git_panel 内部实现。
        // 路由全部收敛到 git_panel_dispatch，避免超长函数内堆积 30+ 个内联分支。
        git_panel_method if git_panel_method.starts_with("git_panel_") => {
            git_panel_dispatch(request.clone(), state, app).await
        }
        "fileReader.directory.list" => ide_chat_file_reader_directory_list(state, request.params).await,
        "fileReader.readFile" => ide_chat_file_reader_read(request.params).await,
        "fileReader.readRawFile" => ide_chat_file_reader_read_raw(request.params).await,
        "fileReader.readFileBlock" => ide_chat_file_reader_read_block(request.params).await,
        "conversation.delete" => ide_chat_delete_conversation(state, request.params).await,
        "conversation.batchArchive" => ide_chat_batch_archive_conversations(state, request.params).await,
        "conversation.rebindRecipient" => ide_chat_rebind_conversation_recipient(state, request.params).await,
        "conversation.rewind" => ide_chat_rewind_conversation(state, request.params).await,
        "conversation.branchFromMessage" => ide_chat_branch_conversation_from_message(state, request.params).await,
        "conversation.branchFromSelection" => ide_chat_branch_conversation(state, request.params).await,
        "conversation.branchFromCurrent" => {
            ide_chat_branch_from_current_command(state, request.params).await
        },
        "branch_unarchived_conversation_from_current" => {
            ide_chat_branch_from_current_command(state, request.params).await
        },
        "conversation.clearChatError" => {
            ide_chat_clear_chat_error_command(state, request.params).await
        },
        "clear_chat_error" => {
            ide_chat_clear_chat_error_command(state, request.params).await
        },
        "list_schedule_runs" => ide_chat_list_schedule_runs_command(state, request.params),
        "conversation.forwardSelection" => ide_chat_forward_selection_command(state, request.params).await,
        "conversation.forwardRemoteContact" => ide_chat_forward_remote_contact_command(state, request.params),
        "conversation.rename" => ide_chat_rename_conversation_command(state, request.params),
        "conversation.pin" => ide_chat_toggle_pin_command(state, request.params),
        "conversation.autoPush" => ide_chat_set_auto_push_command(state, request.params),
        "list_unarchived_conversations" => ide_chat_list_unarchived_conversations_for_web_settings(state).await,
        "conversation.overview.list" => ide_chat_list_unarchived_conversations_for_web_settings(state).await,
        "remote_im_list_contact_conversations" => {
            ide_chat_remote_im_list_contact_conversations_for_web_settings(state)
        }
        "remoteIm.conversations.list" => {
            ide_chat_remote_im_list_contact_conversations_for_web_settings(state)
        }
        "list_delegate_conversations" => ide_chat_list_delegate_conversations_for_web_settings(state),
        "delegate.conversations.list" => ide_chat_list_delegate_conversations_for_web_settings(state),
        "get_prompt_preview" => ide_chat_get_prompt_preview_for_web_settings(state, request.params).await,
        "prompt.preview" => ide_chat_get_prompt_preview_for_web_settings(state, request.params).await,
        "get_system_prompt_preview" => ide_chat_get_system_prompt_preview_for_web_settings(state, request.params).await,
        "prompt.systemPreview" => ide_chat_get_system_prompt_preview_for_web_settings(state, request.params).await,
        "get_conversation_section_orders" => (|| -> Result<Value, String> {
            ide_chat_serialize(get_conversation_section_orders_inner(state)?)
        })(),
        "conversation.sectionOrders.get" => (|| -> Result<Value, String> {
            ide_chat_serialize(get_conversation_section_orders_inner(state)?)
        })(),
        "save_conversation_section_order" => (|| -> Result<Value, String> {
            let input =
                ide_chat_parse_param_field::<SaveConversationSectionOrderInput>(request.params, "input")?;
            ide_chat_serialize(save_conversation_section_order_inner(input, state)?)
        })(),
        "conversation.sectionOrders.save" => (|| -> Result<Value, String> {
            let input =
                ide_chat_parse_param_field::<SaveConversationSectionOrderInput>(request.params, "input")?;
            ide_chat_serialize(save_conversation_section_order_inner(input, state)?)
        })(),
        "delegate.statuses" => ide_chat_delegate_statuses(state, request.params),
        "delegate.abort" => ide_chat_delegate_abort(state, request.params),
        "backgroundShell.list" => ide_chat_background_shell_list_command(state, request.params).await,
        "backgroundShell.terminate" => ide_chat_background_shell_terminate_command(state, request.params).await,
        "delegate.blockPage" => ide_chat_delegate_block_page(state, request.params),
        "delegate.submit" => ide_chat_submit_delegate(state, request.params).await,
        "delegate.delete" => ide_chat_delete_delegate_command(state, request.params),
        "conversation.deleteDelegate" => ide_chat_delete_delegate_command(state, request.params),
        "task.list" => ide_chat_task_list(state),
        "task.create" => ide_chat_task_create(state, request.params),
        "task.update" => ide_chat_task_update(state, request.params),
        "task.delete" => ide_chat_task_delete(state, request.params),
        "task.optimizeDraft" => ide_chat_task_optimize_draft(state, request.params).await,
        "task.dispatchNow" => ide_chat_task_dispatch_now(state, request.params).await,
        "goal.current" => ide_chat_goal_current(state, request.params),
        "goal.create" => ide_chat_goal_create(state, request.params),
        "goal.cancel" => ide_chat_goal_cancel(state, request.params),
        "conversation.compactPreview" => ide_chat_compact_preview(state, request.params),
        "conversation.compact" => ide_chat_compact_conversation(state, request.params).await,
        "conversation.preferredModel.set" => ide_chat_set_preferred_model_command(state, request.params),
        "model.list" => ide_chat_model_list(state, request.params),
        "model.select" => ide_chat_select_model(state, app, request.params),
        "ideContext.upsert" => ide_chat_upsert_ide_context_command(state, request.params, ide_context_runtime),
        "ideContext.query" => ide_chat_parse_params::<IdeContextWorkspaceQueryInput>(request.params)
            .and_then(|input| serde_json::to_value(query_ide_context_references_internal(input, ide_context_runtime)?)
                .map_err(|err| format!("serialize IDE context query result failed: {err}"))),
        "terminalApproval.resolve" => ide_chat_resolve_terminal_approval(state, request.params),
        "terminalApproval.approveForSession" => {
            ide_chat_approve_terminal_approval_for_session(state, request.params)
        }
        "terminalApproval.approveForWorkspace" => {
            ide_chat_approve_terminal_approval_for_workspace(state, request.params)
        }
        "terminalApproval.list" => ide_chat_list_pending_terminal_approvals(state),
        "conversation.planMode.set" => ide_chat_set_conversation_plan_mode(state, request.params),
        "conversation.plan.confirm" => ide_chat_confirm_plan(state, request.params).await,
        "conversation.plan.readFile" => ide_chat_read_plan_file(state, request.params),
        "conversation.rewindPreview" => ide_chat_preview_rewind_conversation(state, request.params).await,
        "conversation.archiveList" => ide_chat_list_archives_command(state),
        "conversation.archiveBlockPage" => ide_chat_archive_block_page_command(state, request.params).await,
        "conversation.deleteArchive" => ide_chat_delete_archive_command(state, request.params),
        "conversation.unarchive" => ide_chat_unarchive_command(state, request.params),
        "archives.list" => ide_chat_list_archives_command(state),
        "archives.blockPage" => ide_chat_archive_block_page_command(state, request.params).await,
        "archives.delete" => ide_chat_delete_archive_command(state, request.params),
        "archives.unarchive" => ide_chat_unarchive_command(state, request.params),
        "is_backend_ready" => Ok(serde_json::json!(state.backend_ready.load(std::sync::atomic::Ordering::Acquire))),
        "load_config" => ide_chat_load_config_for_web_settings(state),
        "load_app_bootstrap_snapshot" => ide_chat_load_app_bootstrap_snapshot_for_web_settings(state),
        "app.bootstrapSnapshot" => ide_chat_load_app_bootstrap_snapshot_for_web_settings(state),
        "get_app_runtime_info" => ide_chat_get_app_runtime_info_for_web_settings(state),
        "app.runtimeInfo" => ide_chat_get_app_runtime_info_for_web_settings(state),
        "messageStore.migration.check" => check_message_store_migration_inner(state)
            .and_then(ide_chat_serialize),
        "messageStore.migration.status" => {
            ide_chat_serialize(get_message_store_migration_runtime_status())
        }
        "messageStore.migration.run" => ide_chat_parse_workspace_params::<RunMessageStoreMigrationInput>(request.params)
            .and_then(|_| {
                spawn_message_store_migration_task(state.clone());
                ide_chat_serialize(message_store_migration_runtime_snapshot())
            }),
        "messageStore.migration.confirm" => ide_chat_serialize(confirm_message_store_migration_summary()),
        "save_config" => ide_chat_save_config_for_web_settings(state, app, ide_context_runtime, request.params),
        "load_agents" => ide_chat_load_agents_for_web_settings(state),
        "convert_private_agent_to_main" => {
            ide_chat_convert_private_agent_to_main_for_web_settings(state, app, request.params)
        }
        "save_agent_avatar" => ide_chat_save_agent_avatar_for_web_settings(state, request.params),
        "clear_agent_avatar" => ide_chat_clear_agent_avatar_for_web_settings(state, request.params),
        "set_agent_private_memory_enabled" => {
            ide_chat_set_agent_private_memory_enabled_for_web_settings(state, request.params)
        }
        "read_chat_image_data_url" => (|| -> Result<Value, String> {
            let input = ide_chat_parse_param_field::<ChatImageDataUrlInput>(request.params, "input")?;
            ide_chat_serialize(read_chat_image_data_url_inner(input, state)?)
        })(),
        "read_local_chat_image_thumbnail" => (|| async {
            let input = ide_chat_parse_param_field::<ReadLocalChatImageThumbnailInput>(request.params, "input")?;
            ide_chat_serialize(read_local_chat_image_thumbnail_inner(input, state).await?)
        })().await,
        "read_local_chat_image_original" => (|| async {
            let input = ide_chat_parse_param_field::<ReadLocalChatImageThumbnailInput>(request.params, "input")?;
            ide_chat_serialize(read_local_chat_image_original_inner(input, state).await?)
        })().await,
        "read_avatar_data_url" => (|| -> Result<Value, String> {
            let input = ide_chat_parse_param_field::<AvatarDataPathInput>(request.params, "input")?;
            ide_chat_serialize(read_avatar_data_url_inner(input, state)?)
        })(),
        "save_agents" => ide_chat_save_agents_for_web_settings(state, app, request.params),
        "load_chat_settings" => ide_chat_load_chat_settings_for_web_settings(state),
        "save_chat_settings" => ide_chat_save_chat_settings_for_web_settings(state, app, request.params),
        "patch_chat_settings" => ide_chat_patch_chat_settings_for_web_settings(state, app, request.params),
        "save_conversation_api_settings" => ide_chat_save_conversation_api_settings_for_web_settings(state, app, request.params),
        "patch_conversation_api_settings" => ide_chat_patch_conversation_api_settings_for_web_settings(state, app, request.params),
        "refresh_models" => ide_chat_refresh_models_for_web_settings(state, request.params).await,
        "quick_genai_chat" => ide_chat_quick_genai_chat_for_web_settings(state, request.params).await,
        "fetch_model_metadata" => ide_chat_fetch_model_metadata_for_web_settings(state, request.params).await,
        "resolve_model_adapter_kind" => ide_chat_resolve_model_adapter_kind_for_web_settings(request.params),
        "test_embedding_connection" => ide_chat_test_embedding_connection_for_web_settings(request.params).await,
        "test_rerank_connection" => ide_chat_test_rerank_connection_for_web_settings(request.params).await,
        "test_voice_connection" => ide_chat_test_voice_connection_for_web_settings(request.params).await,
        "test_memory_embedding_provider" => ide_chat_test_memory_embedding_provider_for_web_settings(state, request.params),
        "test_memory_rerank_provider" => ide_chat_test_memory_rerank_provider_for_web_settings(state, request.params),
        "get_image_text_cache_stats" => ide_chat_get_image_text_cache_stats_for_web_settings(state),
        "clear_image_text_cache" => ide_chat_clear_image_text_cache_for_web_settings(state),
        "check_tools_status" => ide_chat_check_tools_status_for_web_settings(state, request.params),
        "list_terminal_shell_candidates" => {
            ide_chat_list_terminal_shell_candidates_for_web_settings(state)
        }
        "list_tool_catalog" => ide_chat_list_tool_catalog_for_web_settings(state).await,
        "list_department_permission_catalog" => ide_chat_list_department_permission_catalog_for_web_settings(state).await,
        "get_app_version" => Ok(serde_json::json!(env!("CARGO_PKG_VERSION").to_string())),
        "stt_transcribe" => ide_chat_stt_transcribe_for_web_settings(state, request.params).await,
        "get_project_repository_url" => Ok(serde_json::json!(GITHUB_REPO_PAGE.to_string())),
        "fetch_project_changelog_markdown" => fetch_project_changelog_markdown().await.and_then(ide_chat_serialize),
        "get_web_access_info" => ide_chat_web_access_info_for_web_settings(app, state, ide_context_runtime).await,
        "transport.accessInfo" => ide_chat_web_access_info_for_web_settings(app, state, ide_context_runtime).await,
        "list_recent_runtime_logs" => list_recent_runtime_logs().and_then(ide_chat_serialize),
        "list_runtime_logs_since" => {
            let since_created_at = request
                .params
                .get("sinceCreatedAt")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            list_runtime_logs_since(since_created_at).and_then(ide_chat_serialize)
        }
        "append_runtime_log_probe" => {
            let message = request
                .params
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            append_runtime_log_probe(message).and_then(ide_chat_serialize)
        }
        "clear_recent_runtime_logs" => clear_recent_runtime_logs().and_then(ide_chat_serialize),
        "set_github_update_method" => ide_chat_set_github_update_method_for_web_settings(state, app, request.params),
        "get_department_default_draft" => {
            ide_chat_get_department_default_draft_for_web_settings(state, request.params)
        }
        "set_skipped_github_update_version" => {
            ide_chat_set_skipped_github_update_version_for_web_settings(state, app, request.params)
        },
        "get_storage_usage_overview" => {
            ide_chat_get_storage_usage_overview_for_web_settings(state).await
        }
        "refresh_storage_usage_overview" => {
            ide_chat_refresh_storage_usage_overview_for_web_settings(state).await
        }
        "cleanup_storage_legacy_items" => {
            ide_chat_cleanup_storage_legacy_items_for_web_settings(state, request.params)
        }
        "export_config_migration_package" | "configMigration.export" => {
            ide_chat_parse_param_field::<ExportConfigMigrationPackageInput>(
                request.params,
                "input",
            )
            .and_then(|input| export_config_migration_package_for_web(input, state)
                .map_err(migration_command_error_for_web))
            .and_then(ide_chat_serialize)
        }
        "preview_import_config_migration_package" | "configMigration.preview" => {
            ide_chat_parse_param_field::<PreviewImportConfigMigrationPackageInput>(
                request.params,
                "input",
            )
            .and_then(|input| preview_import_config_migration_package_for_web(input, state)
                .map_err(migration_command_error_for_web))
            .and_then(ide_chat_serialize)
        }
        "apply_import_config_migration_package" | "configMigration.apply" => {
            ide_chat_parse_param_field::<ApplyImportConfigMigrationPackageInput>(
                request.params,
                "input",
            )
            .and_then(|input| apply_import_config_migration_package_inner(input, app, state)
                .map_err(migration_command_error_for_web))
            .and_then(ide_chat_serialize)
        }
        "codex_get_auth_status" => {
            ide_chat_codex_get_auth_status_for_web_settings(request.params).await
        }
        "codex_start_oauth_login" => {
            ide_chat_codex_start_oauth_login_for_web_settings(request.params).await
        }
        "codex_get_rate_limits" => {
            ide_chat_codex_get_rate_limits_for_web_settings(request.params).await
        }
        "codex_consume_rate_limit_reset_credit" => {
            ide_chat_codex_consume_rate_limit_reset_credit_for_web_settings(request.params).await
        }
        "codex_logout" => ide_chat_codex_logout_for_web_settings(request.params),
        "codex_get_builtin_models" => ide_chat_serialize(codex_get_builtin_models()),
        "generate_image" => ide_chat_generate_image_for_web_settings(state, request.params).await,
        "list_memories" => ide_chat_list_memories_for_web_settings(state),
        "delete_memory" => ide_chat_delete_memory_for_web_settings(state, request.params),
        "search_memories_mixed" => ide_chat_search_memories_mixed_for_web_settings(state, request.params),
        "search_chat_history_slices" => ide_chat_search_chat_history_slices_for_web_settings(state, request.params),
        "get_memory_provider_bindings" => ide_chat_get_memory_provider_bindings_for_web_settings(state),
        "get_memory_embedding_sync_progress" => ide_chat_get_memory_embedding_sync_progress_for_web_settings(state),
        "save_memory_embedding_binding" => ide_chat_save_memory_embedding_binding_for_web_settings(state, request.params),
        "save_memory_rerank_binding" => ide_chat_save_memory_rerank_binding_for_web_settings(state, request.params),
        "get_agent_private_memory_count" => ide_chat_get_agent_private_memory_count_for_web_settings(state, request.params),
        "set_agent_memory_recall_mode" => ide_chat_set_agent_memory_recall_mode_for_web_settings(state, request.params),
        "disable_agent_private_memory" => ide_chat_disable_agent_private_memory_for_web_settings(state, request.params),
        "export_memories" => ide_chat_export_memories_for_web_settings(state, request.params),
        "preview_export_memories" => ide_chat_preview_export_memories_for_web_settings(state),
        "import_memories" => ide_chat_import_memories_for_web_settings(state, request.params),
        "preview_import_angel_memories" => ide_chat_preview_import_angel_memories_for_web_settings(request.params),
        "import_angel_memories" => ide_chat_import_angel_memories_for_web_settings(state, request.params),
        "task_list_tasks" => ide_chat_task_list_tasks_for_web_settings(state),
        "task_get_task" => ide_chat_task_get_task_for_web_settings(state, request.params),
        "task_create_task" => ide_chat_task_create_task_for_web_settings(state, request.params),
        "task_update_task" => ide_chat_task_update_task_for_web_settings(state, request.params),
        "task_complete_task" => ide_chat_task_complete_task_for_web_settings(state, request.params),
        "task_delete_task" => ide_chat_task_delete_task_for_web_settings(state, request.params),
        "task_list_run_logs" => ide_chat_task_list_run_logs_for_web_settings(state, request.params),
        "task_optimize_draft" => ide_chat_task_optimize_draft_for_web_settings(state, request.params).await,
        "mcp_list_servers" => ide_chat_mcp_list_servers_for_web_settings(state),
        "mcp_validate_definition" => ide_chat_mcp_validate_definition_for_web_settings(request.params),
        "mcp_fix_definition" => ide_chat_mcp_fix_definition_for_web_settings(state, request.params).await,
        "mcp_save_server" => ide_chat_mcp_save_server_for_web_settings(state, request.params),
        "mcp_remove_server" => ide_chat_mcp_remove_server_for_web_settings(state, request.params).await,
        "mcp_list_server_tools" => ide_chat_mcp_list_server_tools_for_web_settings(state, request.params).await,
        "mcp_list_server_tools_cached" => ide_chat_mcp_list_server_tools_cached_for_web_settings(state, request.params),
        "mcp_deploy_server" => ide_chat_mcp_deploy_server_for_web_settings(state, request.params).await,
        "mcp_undeploy_server" => ide_chat_mcp_undeploy_server_for_web_settings(state, request.params).await,
        "mcp_set_tool_enabled" => ide_chat_mcp_set_tool_enabled_for_web_settings(state, request.params),
        "mcp_list_skills" => ide_chat_mcp_list_skills_for_web_settings(state),
        "mcp_save_skill" => ide_chat_mcp_save_skill_for_web_settings(state, request.params),
        "mcp_read_skill_file" => ide_chat_mcp_read_skill_file_for_web_settings(state, request.params),
        "skill_open_item_dir" => ide_chat_skill_open_item_dir_for_web_settings(state, request.params),
        "catalog_list_sources" => ide_chat_catalog_list_sources_for_web_settings(request.params),
        "catalog_search" => ide_chat_catalog_search_for_web_settings(state, request.params).await,
        "catalog_install" => ide_chat_catalog_install_for_web_settings(state, request.params).await,
        "skill_set_enabled" => ide_chat_skill_set_enabled_for_web_settings(state, request.params).await,
        "skill_remove" => ide_chat_skill_remove_for_web_settings(state, request.params).await,
        "mcp_refresh_mcp_and_skills" => ide_chat_mcp_refresh_mcp_and_skills_for_web_settings(state).await,
        "get_usage_overview" => ide_chat_get_usage_overview_for_web_settings(state).await,
        "refresh_usage_overview" => ide_chat_refresh_usage_overview_for_web_settings(state).await,
        "get_usage_trail" => ide_chat_get_usage_trail_for_web_settings(state, request.params).await,
        "list_recent_llm_round_logs" => ide_chat_list_recent_llm_round_logs_for_web_settings(state),
        "get_recent_llm_round_log_section" => ide_chat_get_recent_llm_round_log_section_for_web_settings(state, request.params),
        "clear_recent_llm_round_logs" => ide_chat_clear_recent_llm_round_logs_for_web_settings(state),
        "remote_im_get_channel_status" => ide_chat_remote_im_get_channel_status_for_web_settings(state, request.params).await,
        "remote_im_restart_channel" => ide_chat_remote_im_restart_channel_for_web_settings(state, request.params).await,
        "remote_im_get_channel_logs" => ide_chat_remote_im_get_channel_logs_for_web_settings(state, request.params).await,
        "remote_im_get_contact_logs" => ide_chat_remote_im_get_contact_logs_for_web_settings(state, request.params).await,
        "remote_im_list_channels" => ide_chat_remote_im_list_channels_for_web_settings(state),
        "remote_im_list_contacts" => ide_chat_remote_im_list_contacts_for_web_settings(state),
        "remote_im_update_contact_allow_send" => ide_chat_remote_im_update_contact_allow_send_for_web_settings(state, request.params),
        "remote_im_update_contact_allow_send_files" => ide_chat_remote_im_update_contact_allow_send_files_for_web_settings(state, request.params),
        "remote_im_update_contact_blocked_message_prefixes" => ide_chat_remote_im_update_contact_blocked_message_prefixes_for_web_settings(state, request.params),
        "remote_im_update_contact_activation" => ide_chat_remote_im_update_contact_activation_for_web_settings(state, request.params),
        "remote_im_update_contact_department_binding" => ide_chat_remote_im_update_contact_department_binding_for_web_settings(state, request.params),
        "remote_im_update_contact_processing_mode" => ide_chat_remote_im_update_contact_processing_mode_for_web_settings(state, request.params),
        "remote_im_update_contact_workspace" => ide_chat_remote_im_update_contact_workspace_for_web_settings(state, request.params),
        "remote_im_delete_contact" => ide_chat_remote_im_delete_contact_for_web_settings(state, request.params),
        "remote_im_weixin_oc_start_login" => ide_chat_remote_im_weixin_oc_start_login_for_web_settings(state, request.params).await,
        "remote_im_weixin_oc_get_login_status" => ide_chat_remote_im_weixin_oc_get_login_status_for_web_settings(state, request.params).await,
        "remote_im_weixin_oc_sync_contacts" => ide_chat_remote_im_weixin_oc_sync_contacts_for_web_settings(state, request.params).await,
        "remote_im_weixin_oc_logout" => ide_chat_remote_im_weixin_oc_logout_for_web_settings(state, request.params).await,
        "remote_im_get_default_group_response_guidance" => {
            ide_chat_remote_im_default_group_response_guidance_for_web_settings()
        }
        "remote_im_patch_contact_settings" => {
            ide_chat_remote_im_patch_contact_settings_for_web_settings(state, request.params)
        }
        "remote_im_reconfigure_channel_behavior" => {
            ide_chat_remote_im_reconfigure_channel_behavior_for_web_settings(state, request.params)
        }
        "remoteIm.dashboard.subscribe" => {
            remote_im_subscribe_contact_dashboard_for_web(state, request.params, client_id)
        }
        "remoteIm.dashboard.sync" => remote_im_sync_contact_dashboard_for_web(state, request.params),
        "remoteIm.dashboard.unsubscribe" => {
            remote_im_unsubscribe_contact_dashboard_for_web(request.params, client_id)
        }
        "chat.queueAttachment" => ide_chat_queue_attachment(state, request.params).await,
        "chat.send" => ide_chat_send_message(state, request.params).await,
        "chat.stop" => ide_chat_stop_conversation(state, request.params),
        "chat.queueSnapshot" => ide_chat_queue_snapshot(state),
        "chat.sessionStateSnapshot" => ide_chat_session_state_snapshot(state),
        "chat.queueRecall" => ide_chat_recall_queue_event(state, request.params),
        "chat.queueMarkGuided" => ide_chat_mark_queue_event_guided(state, request.params),
        "queue_inline_file_attachment" => ide_chat_queue_inline_attachment(state, request.params).await,
        "attachment.transfer.begin" => ide_attachment_transfer_begin(state, client_id, request.params).await,
        "attachment.transfer.complete" => ide_attachment_transfer_complete(state, client_id, request.params).await,
        "attachment.transfer.abort" => ide_attachment_transfer_abort(client_id, request.params).await,
        "submit_chat_message" => ide_chat_submit_message_command(state, request.params).await,
        "stop_chat_message" => ide_chat_stop_message_command(state, request.params),
        "get_chat_queue_snapshot" => ide_chat_queue_snapshot(state),
        "get_main_session_state_snapshot" => ide_chat_session_state_snapshot(state),
        "recall_chat_queue_event" => ide_chat_recall_queue_event(state, request.params),
        "mark_chat_queue_event_guided" => ide_chat_mark_queue_event_guided(state, request.params),
        "get_conversation_fast_request_turns" => ide_chat_conversation_fast_request_turns_command(state, request.params),
        "get_conversation_runtime_snapshot" => ide_chat_conversation_runtime_snapshot(state, request.params),
        "conversation.foregroundLightSnapshot" => {
            ide_chat_conversation_light_snapshot_command(state, request.params).await
        }
        "get_foreground_conversation_light_snapshot" => {
            ide_chat_conversation_light_snapshot_command(state, request.params).await
        }
        "get_foreground_conversation_freshness_snapshot" => ide_chat_conversation_freshness_snapshot_command(state, request.params).await,
        "get_unarchived_conversation_block_page" => ide_chat_conversation_block_page_command(state, request.params).await,
        "get_unarchived_conversation_message_by_id" => ide_chat_conversation_message_by_id_command(state, request.params).await,
        "get_active_conversation_messages_before" => ide_chat_conversation_messages_before_command(state, request.params).await,
        "get_active_conversation_compaction_segment_before" => ide_chat_conversation_compaction_segment_before_command(state, request.params).await,
        "request_conversation_messages_after_async" =>
            ide_chat_parse_param_field::<RequestConversationMessagesAfterAsyncInput>(
                request.params,
                "input",
            )
            .and_then(|input| request_conversation_messages_after_async_inner(input, state))
            .and_then(ide_chat_serialize),
        "mark_conversation_read" => ide_chat_mark_conversation_read_command(state, request.params).await,
        "set_active_unarchived_conversation" => ide_chat_set_active_conversation_command(state, request.params),
        "rebind_unarchived_conversation_recipient" => ide_chat_rebind_conversation_command(state, request.params).await,
        "rewind_conversation_from_message" => ide_chat_rewind_conversation_command(state, request.params).await,
        "preview_rewind_conversation_from_message" => {
            ide_chat_preview_rewind_conversation(state, request.params).await
        }
        "set_conversation_plan_mode" => ide_chat_set_plan_mode_command(state, request.params),
        "set_conversation_preferred_model" => ide_chat_set_preferred_model_command(state, request.params),
        "confirm_plan_and_continue" => ide_chat_confirm_plan_command(state, request.params).await,
        "resolve_terminal_approval" => ide_chat_resolve_terminal_approval_command(state, request.params),
        "goal_get_current" => ide_chat_goal_current_command(state, request.params),
        "goal_create_goal" => ide_chat_goal_create_command(state, request.params),
        "goal_cancel_goal" => ide_chat_goal_cancel_command(state, request.params),
        "query_ide_context_references" => ide_chat_query_ide_context_command(request.params, ide_context_runtime),
        "list_archives" => ide_chat_list_archives_command(state),
        "get_archive_block_page" => ide_chat_archive_block_page_command(state, request.params).await,
        "delete_archive" => ide_chat_delete_archive_command(state, request.params),
        "unarchive_archive" => ide_chat_unarchive_command(state, request.params),
        "conversation.archive" => {
            ide_chat_archive_conversation_command(state, request.params).await
        }
        "archive_conversation" => {
            ide_chat_archive_conversation_command(state, request.params).await
        }
        "batch_archive_conversations" => ide_chat_batch_archive_command(state, request.params).await,
        "list_conversation_delegate_statuses" => ide_chat_delegate_statuses_command(state, request.params),
        "abort_delegate_conversation" => ide_chat_delegate_abort_command(state, request.params),
        "get_delegate_conversation_block_page" => ide_chat_delegate_block_page_command(state, request.params),
        "delete_delegate_conversation" => ide_chat_delete_delegate_command(state, request.params),
        "branch_unarchived_conversation_from_selection" => ide_chat_branch_selection_command(state, request.params).await,
        "create_conversation_branch_from_message" => ide_chat_branch_message_command(state, request.params).await,
        "submit_user_async_delegate" => ide_chat_submit_delegate_command(state, request.params).await,
        "delete_unarchived_conversation" => ide_chat_delete_unarchived_command(state, request.params).await,
        "create_side_chat_conversation" => {
            ide_chat_create_side_chat_conversation(state, request.params).await
        }
        "export_conversation_share_json" => ide_chat_export_conversation_share_command(state, request.params),
        "conversation.exportShare" => ide_chat_export_conversation_share_command(state, request.params),
        "import_archives_from_json" => ide_chat_import_archives_command(state, request.params),
        "conversation.importArchives" => ide_chat_import_archives_command(state, request.params),
        "import_agent_memories" => ide_chat_import_agent_memories_command(state, request.params),
        "remote_im_get_contact_conversation_block_page" => ide_chat_remote_im_block_page_command(state, request.params).await,
        "remoteIm.conversation.blockPage" => ide_chat_remote_im_block_page_command(state, request.params).await,
        "remote_im_clear_contact_conversation" => ide_chat_remote_im_clear_conversation_command(state, request.params),
        "remoteIm.conversation.clear" => ide_chat_remote_im_clear_conversation_command(state, request.params),
        "frontend_ready_start_remote_im_services" => ide_chat_frontend_ready_remote_im_command(app).await,
        "remoteIm.services.start" => ide_chat_frontend_ready_remote_im_command(app).await,
        "forward_unarchived_conversation_selection" => ide_chat_forward_selection_command(state, request.params).await,
        "forward_selection_to_remote_im_contact" => ide_chat_forward_remote_contact_command(state, request.params),
        "rename_unarchived_conversation" => ide_chat_rename_conversation_command(state, request.params),
        "toggle_unarchived_conversation_pin" => ide_chat_toggle_pin_command(state, request.params),
        "set_conversation_auto_push_remote_contact" => ide_chat_set_auto_push_command(state, request.params),
        "set_department_primary_api_config" => ide_chat_set_department_primary_api_command(state, app, request.params),
        "department.primaryApi.set" => ide_chat_set_department_primary_api_command(state, app, request.params),
        "set_ui_language" => ide_chat_set_ui_language_command(state, app, request.params),
        "app.language.set" => ide_chat_set_ui_language_command(state, app, request.params),
        "dump_memory_cache_stats" => ide_chat_dump_memory_cache_stats_command(state),
        "list_unarchived_conversations_changed_since" => ide_chat_conversation_changed_since_command(state, request.params).await,
        "search_memories_recall" => ide_chat_search_memories_recall_command(state, request.params),
        "toolReview.reports.list" => ide_chat_tool_review_reports(state, request.params),
        "toolReview.report.delete" => ide_chat_tool_review_delete_report(state, request.params),
        "toolReview.commitOptions.list" => ide_chat_tool_review_commit_options(state, request.params).await,
        "toolReview.code.submit" => ide_chat_tool_review_submit_code(state, request.params).await,
        "toolReview.batches.list" => ide_chat_tool_review_batches(state, request.params),
        "toolReview.item.detail" => ide_chat_tool_review_item_detail(state, request.params),
        "toolReview.item.review" => ide_chat_tool_review_item_review(state, request.params).await,
        "toolReview.batch.review" => ide_chat_tool_review_batch_review(state, request.params).await,
        "toolReview.item.decision" => ide_chat_tool_review_item_decision(state, request.params).await,
        _ => return ide_chat_jsonrpc_error(request.id, -32601, "method not found"),
    };
    match result {
        Ok(value) => ide_chat_jsonrpc_success(request.id, value),
        Err(err) => ide_chat_jsonrpc_error(request.id, -32000, err),
    }
}

/// git 面板路由分发：与 tauri 命令共用同一套 git_panel 内部实现。
/// 主分发函数只保留一个前缀守卫分支，具体分支收敛到这里，避免超长函数。
async fn git_panel_dispatch(
    request: IdeChatJsonRpcRequest,
    state: &AppState,
    app: &AppHandle,
) -> Result<Value, String> {
    match request.method.as_str() {
        "git_panel_repos" => {
            let refresh = request.params.get("refresh").and_then(Value::as_bool).unwrap_or(false);
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_repos_inner(input, refresh, state).await?)
        }
        "git_panel_watch_start" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_watch_start(app.clone(), input).await?)
        }
        "git_panel_watch_stop" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_watch_stop(input).await?)
        }
        "git_panel_detect" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_detect(input).await?)
        }
        "git_panel_discover" => {
            let refresh = request.params.get("refresh").and_then(Value::as_bool).unwrap_or(false);
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_discover_inner(input, refresh, state).await?)
        }
        "git_panel_status" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_status_inner(input).await?)
        }
        "git_panel_diff" => {
            let input = ide_chat_parse_param_field::<GitPanelDiffInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_diff(input).await?)
        }
        "git_panel_stage" => {
            let input = ide_chat_parse_param_field::<GitPanelPathsInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stage(input).await?)
        }
        "git_panel_unstage" => {
            let input = ide_chat_parse_param_field::<GitPanelPathsInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_unstage(input).await?)
        }
        "git_panel_commit" => {
            let input = ide_chat_parse_param_field::<GitPanelCommitInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_commit(input).await?)
        }
        "git_panel_commit_files" => {
            let input = ide_chat_parse_param_field::<GitPanelCommitFilesInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_commit_files(input).await?)
        }
        "git_panel_discard" => {
            let input = ide_chat_parse_param_field::<GitPanelPathsInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_discard(input).await?)
        }
        "git_panel_stash_list" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_list(input).await?)
        }
        "git_panel_stash_files" => {
            let input = ide_chat_parse_param_field::<GitPanelStashRefInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_files(input).await?)
        }
        "git_panel_stash_create" => {
            let input = ide_chat_parse_param_field::<GitPanelStashInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_create(input).await?)
        }
        "git_panel_stash_apply" => {
            let input = ide_chat_parse_param_field::<GitPanelStashRefInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_apply(input).await?)
        }
        "git_panel_stash_pop" => {
            let input = ide_chat_parse_param_field::<GitPanelStashRefInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_pop(input).await?)
        }
        "git_panel_stash_drop" => {
            let input = ide_chat_parse_param_field::<GitPanelStashRefInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_stash_drop(input).await?)
        }
        "git_panel_branch_list" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_branch_list(input).await?)
        }
        "git_panel_branch_create" => {
            let input = ide_chat_parse_param_field::<GitPanelBranchInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_branch_create(input).await?)
        }
        "git_panel_branch_delete" => {
            let input = ide_chat_parse_param_field::<GitPanelBranchInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_branch_delete(input).await?)
        }
        "git_panel_checkout" => {
            let input = ide_chat_parse_param_field::<GitPanelCheckoutInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_checkout(input).await?)
        }
        "git_panel_checkout_check" => {
            let input = ide_chat_parse_param_field::<GitPanelCheckoutInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_checkout_check(input).await?)
        }
        "git_panel_remote_list" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_remote_list(input).await?)
        }
        "git_panel_fetch" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_fetch(input).await?)
        }
        "git_panel_pull" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_pull(input).await?)
        }
        "git_panel_push" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_push(input).await?)
        }
        "git_panel_sync" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_sync(input).await?)
        }
        "git_panel_log" => {
            let input = ide_chat_parse_param_field::<GitPanelLogInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_log(input).await?)
        }
        "git_panel_show" => {
            let input = ide_chat_parse_param_field::<GitPanelShowInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_show(input).await?)
        }
        "git_panel_reset_soft" => {
            let input = ide_chat_parse_param_field::<GitPanelWorkspaceInput>(request.params, "input")?;
            ide_chat_serialize(git_panel_reset_soft(input).await?)
        }
        _ => Err(format!("未知的 git 面板方法：{}", request.method)),
    }
}

#[cfg(test)]
mod web_native_capability_tests {
    use super::*;

    fn collect_frontend_source_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_frontend_source_files(&path, out);
            } else if matches!(path.extension().and_then(|value| value.to_str()), Some("ts" | "vue"))
                && !path.file_name().and_then(|value| value.to_str()).is_some_and(|name| {
                    name.ends_with(".spec.ts") || name.ends_with(".test.ts")
                })
            {
                out.push(path);
            }
        }
    }

    fn quoted_value_at(source: &str, quote_index: usize) -> Option<(String, usize)> {
        let quote = *source.as_bytes().get(quote_index)?;
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        let bytes = source.as_bytes();
        let mut index = quote_index + 1;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                index = index.saturating_add(2);
                continue;
            }
            if bytes[index] == quote {
                return Some((source[quote_index + 1..index].to_string(), index + 1));
            }
            index += 1;
        }
        None
    }

    fn static_invoke_tauri_methods(source: &str) -> Vec<String> {
        let mut methods = Vec::new();
        let mut offset = 0usize;
        while let Some(relative) = source[offset..].find("invokeTauri") {
            let start = offset + relative + "invokeTauri".len();
            let Some(open_relative) = source[start..].find('(') else {
                break;
            };
            let mut index = start + open_relative + 1;
            while source.as_bytes().get(index).is_some_and(u8::is_ascii_whitespace) {
                index += 1;
            }
            if let Some((method, end)) = quoted_value_at(source, index) {
                methods.push(method);
                offset = end;
            } else {
                offset = index.saturating_add(1);
            }
        }
        methods
    }

    fn web_covered_methods(source: &str) -> std::collections::HashSet<String> {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let dispatch_start = production
            .find("async fn ide_chat_handle_jsonrpc_request")
            .unwrap_or(production.len());
        let mut covered = std::collections::HashSet::new();
        let mut index = 0usize;
        while index < production.len() {
            let Some(relative) = production[index..].find(['\'', '"']) else {
                break;
            };
            let quote_index = index + relative;
            let Some((value, end)) = quoted_value_at(production, quote_index) else {
                break;
            };
            let is_native_declaration = quote_index < dispatch_start;
            let is_dispatch_arm = production[end..].trim_start().starts_with("=>");
            if is_native_declaration || is_dispatch_arm {
                covered.insert(value);
            }
            index = end;
        }
        covered
    }

    #[test]
    fn every_static_frontend_tauri_command_should_have_web_behavior() {
        let frontend_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
        let mut files = Vec::new();
        collect_frontend_source_files(&frontend_root, &mut files);
        let mut invoked = std::collections::BTreeSet::new();
        for path in files {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            invoked.extend(static_invoke_tauri_methods(&source));
        }
        let covered = web_covered_methods(include_str!("jsonrpc_dispatch.rs"));
        let missing = invoked
            .into_iter()
            .filter(|method| !covered.contains(method))
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "frontend invokeTauri commands must be handled or explicitly rejected on Web: {missing:?}"
        );
    }

    #[test]
    fn local_file_and_window_methods_should_be_explicitly_native_only() {
        for method in [
            "read_file_reader_file",
            "open_storage_usage_item_directory",
            "mcp_open_workspace_dir",
            "migrate_shell_workspace_directory",
            "desktop_screenshot",
            "demo_send_native_notification",
            "demo_restart_app",
            "show_main_window",
            "sync_tray_icon",
            "get_github_update_state",
            "check_github_update",
            "start_github_update",
            "cancel_github_update",
            "apply_prepared_github_update",
            "export_memories_to_path",
            "export_agent_private_memories",
            "bind_active_chat_view_stream",
            "probe_active_chat_view_stream",
            "unbind_active_chat_view_stream",
            "set_chat_window_active",
        ] {
            assert!(
                ide_chat_web_native_only_method(method),
                "method should be native-only: {method}"
            );
            assert!(
                ide_chat_web_native_only_error(method)
                    .starts_with("WEB_NATIVE_CAPABILITY_UNAVAILABLE:"),
                "method should use stable error code: {method}"
            );
        }
    }

    #[test]
    fn portable_business_methods_should_not_be_marked_native_only() {
        for method in [
            "conversation.list",
            "conversation.resumeSubscription",
            "conversation.streamProbe",
            "workspace.list",
            "check_git_workspace_root",
            "get_chat_shell_workspace",
            "update_chat_shell_workspace_layout",
            "workspace.directory.list",
            "fileReader.directory.list",
            "fileReader.readFile",
            "fileReader.readFileBlock",
            "read_local_chat_image_thumbnail",
            "read_local_chat_image_original",
            "git_panel_reset_soft",
            "conversation.plan.readFile",
            "conversation.rewindPreview",
            "conversation.archive",
            "conversation.compact",
            "conversation.foregroundLightSnapshot",
            "chat.send",
            "remote_im_list_contacts",
            "task.list",
            "mcp_list_servers",
            "set_github_update_method",
            "set_skipped_github_update_version",
            "list_recent_llm_round_logs",
            "get_usage_overview",
            "refresh_usage_overview",
            "get_usage_trail",
            "queue_inline_file_attachment",
            "attachment.transfer.begin",
            "attachment.transfer.complete",
            "attachment.transfer.abort",
            "submit_chat_message",
            "stop_chat_message",
            "get_chat_queue_snapshot",
            "get_main_session_state_snapshot",
            "recall_chat_queue_event",
            "mark_chat_queue_event_guided",
            "get_conversation_fast_request_turns",
            "get_conversation_runtime_snapshot",
            "get_foreground_conversation_light_snapshot",
            "get_foreground_conversation_freshness_snapshot",
            "get_unarchived_conversation_block_page",
            "get_unarchived_conversation_message_by_id",
            "get_active_conversation_messages_before",
            "get_active_conversation_compaction_segment_before",
            "conversation.compactionSegmentBefore",
            "request_conversation_messages_after_async",
            "mark_conversation_read",
            "set_active_unarchived_conversation",
            "rebind_unarchived_conversation_recipient",
            "rewind_conversation_from_message",
            "set_conversation_plan_mode",
            "set_conversation_preferred_model",
            "confirm_plan_and_continue",
            "resolve_terminal_approval",
            "goal_get_current",
            "goal_create_goal",
            "goal_cancel_goal",
            "query_ide_context_references",
            "list_archives",
            "get_archive_block_page",
            "delete_archive",
            "unarchive_archive",
            "archive_conversation",
            "batch_archive_conversations",
            "list_conversation_delegate_statuses",
            "abort_delegate_conversation",
            "get_delegate_conversation_block_page",
            "delete_delegate_conversation",
            "branch_unarchived_conversation_from_selection",
            "create_conversation_branch_from_message",
            "submit_user_async_delegate",
            "delete_unarchived_conversation",
            "read_chat_image_data_url",
            "read_avatar_data_url",
            "messageStore.migration.check",
            "messageStore.migration.run",
            "stt_transcribe",
            "get_storage_usage_overview",
            "refresh_storage_usage_overview",
            "cleanup_storage_legacy_items",
            "configMigration.export",
            "configMigration.preview",
            "configMigration.apply",
            "export_config_migration_package",
            "preview_import_config_migration_package",
            "apply_import_config_migration_package",
            "codex_get_auth_status",
            "codex_start_oauth_login",
            "codex_get_rate_limits",
            "codex_consume_rate_limit_reset_credit",
            "codex_logout",
            "codex_get_builtin_models",
            "save_agent_avatar",
            "clear_agent_avatar",
            "generate_image",
            "check_tools_status",
            "list_terminal_shell_candidates",
            "preview_rewind_conversation_from_message",
            "convert_private_agent_to_main",
            "set_agent_private_memory_enabled",
            "remote_im_get_default_group_response_guidance",
            "remote_im_patch_contact_settings",
            "remote_im_reconfigure_channel_behavior",
            "create_side_chat_conversation",
            "git_panel_discover",
            "git_panel_status",
            "git_panel_diff",
            "git_panel_stage",
            "git_panel_commit",
            "git_panel_log",
            "git_panel_show",
        ] {
            assert!(
                !ide_chat_web_native_only_method(method),
                "portable method should remain available: {method}"
            );
        }
    }

    #[test]
    fn chat_send_stop_and_rewind_should_use_canonical_tauri_request_shapes() {
        let send = serde_json::json!({
            "payload": {
                "text": "hello",
                "images": [],
                "attachments": [],
                "extraTextBlocks": ["context"]
            },
            "session": {
                "apiConfigId": null,
                "departmentId": "department-1",
                "agentId": "agent-1",
                "conversationId": "conversation-1"
            }
        });
        let stop = serde_json::json!({
            "session": {
                "apiConfigId": null,
                "departmentId": "department-1",
                "agentId": "agent-1",
                "conversationId": "conversation-1"
            },
            "partialAssistantText": "visible text",
            "partialStreamBlocks": []
        });
        let rewind = serde_json::json!({
            "session": {
                "apiConfigId": null,
                "departmentId": "department-1",
                "agentId": "agent-1",
                "conversationId": "conversation-1"
            },
            "messageId": "message-1",
            "undoApplyPatch": true
        });

        assert!(serde_json::from_value::<SendChatRequest>(send).is_ok());
        assert!(serde_json::from_value::<StopChatRequest>(stop).is_ok());
        assert!(serde_json::from_value::<RewindConversationInput>(rewind).is_ok());
        assert!(serde_json::from_value::<SendChatRequest>(serde_json::json!({
            "conversationId": "conversation-1",
            "text": "legacy"
        }))
        .is_err());
        assert!(serde_json::from_value::<RewindConversationInput>(serde_json::json!({
            "conversationId": "conversation-1",
            "messageId": "message-1"
        }))
        .is_err());
    }
}
