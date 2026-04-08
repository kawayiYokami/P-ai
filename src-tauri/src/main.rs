#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use futures_util::{future::AbortHandle, StreamExt};
use image::ImageFormat;
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rmcp::{schemars, ServiceExt};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, Position, State,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};
use uuid::Uuid;

macro_rules! eprintln {
    ($($arg:tt)*) => {{
        runtime_log_info(format!($($arg)*));
    }};
}

// ==================== 核心领域模型 ====================
include!("features/core/domain.rs");
include!("features/core/time_semantics.rs");

// ==================== 配置与存储 ====================
include!("features/config/storage_and_stt.rs");
include!("features/config/app_data_layout.rs");

// ==================== 对话核心 ====================
include!("features/chat/message_semantics.rs");
include!("features/chat/conversation.rs");
include!("features/chat/model_runtime.rs");
include!("features/chat/scheduler.rs");
include!("features/remote_im/onebot_v11_ws.rs");
include!("features/remote_im/dingtalk_stream.rs");
include!("features/remote_im/weixin_oc.rs");
include!("features/remote_im.rs");
include!("features/remote_im_adapters.rs");

// ==================== 系统窗口与命令 ====================
include!("features/system/windowing.rs");
include!("features/system/record_hotkey_probe.rs");
include!("features/system/sandbox.rs");
include!("features/system/tools.rs");
include!("features/system/updater.rs");

// ==================== 记忆匹配 ====================
include!("features/memory/store.rs");
include!("features/memory/matcher.rs");
include!("features/memory/providers.rs");

// ==================== MCP ====================
include!("features/mcp.rs");
include!("features/skill.rs");
include!("features/task.rs");
include!("features/delegate.rs");

include!("features/system/commands.rs");

fn should_enable_devtools() -> bool {
    if !cfg!(debug_assertions) {
        return false;
    }
    matches!(
        std::env::var("EASYCALL_DEVTOOLS")
            .ok()
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

// Remote IM 命令包装
#[tauri::command]
async fn remote_im_get_channel_status(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<ChannelConnectionStatus, String> {
    let config = state_read_config_cached(&state).map_err(|e| format!("{e:?}"))?;
    if let Some(channel) = config
        .remote_im_channels
        .iter()
        .find(|ch| ch.id == channel_id)
    {
        return match channel.platform {
            RemoteImPlatform::OnebotV11 => get_channel_connection_status(channel_id).await,
            RemoteImPlatform::Dingtalk => Ok(dingtalk_stream_manager()
                .get_channel_status(&channel.id)
                .await),
            RemoteImPlatform::Feishu => Ok(ChannelConnectionStatus {
                channel_id: channel.id.clone(),
                connected: false,
                peer_addr: None,
                connected_at: None,
                listen_addr: String::new(),
                status_text: None,
                last_error: None,
                account_id: None,
                base_url: None,
                login_session_key: None,
                qrcode_url: None,
            }),
            RemoteImPlatform::WeixinOc => Ok(weixin_oc_manager().build_status(&channel.id).await),
        };
    }
    get_channel_connection_status(channel_id).await
}

#[tauri::command]
async fn remote_im_get_channel_logs(channel_id: String) -> Result<Vec<ChannelLogEntry>, String> {
    get_channel_logs(channel_id).await
}

#[tauri::command]
async fn remote_im_restart_channel(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<ChannelConnectionStatus, String> {
    eprintln!("[远程IM] 重启渠道: {}", channel_id);
    onebot_v11_ws_manager()
        .add_log(&channel_id, "info", "[远程IM] 收到渠道重启请求")
        .await;
    let config = state_read_config_cached(&state).map_err(|e| format!("{e:?}"))?;
    let channel = config
        .remote_im_channels
        .iter()
        .find(|ch| ch.id == channel_id)
        .ok_or_else(|| format!("渠道 {} 未找到", channel_id))?
        .clone();
    onebot_v11_ws_manager()
        .add_log(
            &channel_id,
            "info",
            &format!(
                "[远程IM] 当前渠道配置: enabled={}, platform={:?}",
                channel.enabled, channel.platform
            ),
        )
        .await;

    let manager = onebot_v11_ws_manager();
    manager
        .reconcile_channel_runtime(&channel)
        .await
        .map_err(|err| format!("重启渠道失败: {}", err))?;
    eprintln!(
        "[远程IM] 渠道 {} 已按配置收敛: enabled={}, platform={:?}",
        channel_id, channel.enabled, channel.platform
    );

    if channel.enabled && channel.platform == RemoteImPlatform::OnebotV11 {
        // 重启事件消费循环
        let state_clone = state.inner().clone();
        let cid = channel_id.clone();
        tauri::async_runtime::spawn(async move {
            napcat_start_event_consumer(cid, state_clone).await;
        });
    } else if channel.enabled && channel.platform == RemoteImPlatform::Dingtalk {
        let state_clone = state.inner().clone();
        let manager = dingtalk_stream_manager();
        let channel_clone = channel.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = manager
                .reconcile_channel_runtime(&channel_clone, state_clone)
                .await
            {
                eprintln!(
                    "[远程IM] 钉钉渠道收敛失败: channel_id={}, platform={:?}, error={}",
                    channel_clone.id, channel_clone.platform, err
                );
            }
        });
    } else if channel.platform == RemoteImPlatform::WeixinOc {
        let state_clone = state.inner().clone();
        weixin_oc_manager()
            .reconcile_channel_runtime(&channel, state_clone)
            .await?;
    }

    if channel.platform == RemoteImPlatform::Dingtalk {
        Ok(dingtalk_stream_manager()
            .get_channel_status(&channel_id)
            .await)
    } else if channel.platform == RemoteImPlatform::WeixinOc {
        Ok(weixin_oc_manager().build_status(&channel_id).await)
    } else {
        Ok(manager.get_connection_status(&channel_id).await)
    }
}

fn main() {
    if std::env::args().any(|arg| arg == MCP_SCREENSHOT_SERVER_FLAG) {
        if let Err(err) = run_desktop_screenshot_mcp_server() {
            eprintln!("{err}");
        }
        return;
    }
    if std::env::args().any(|arg| arg == MCP_READ_FILE_SERVER_FLAG) {
        if let Err(err) = run_read_file_mcp_server() {
            eprintln!("{err}");
        }
        return;
    }
    if std::env::args().any(|arg| arg == MCP_OPERATE_SERVER_FLAG) {
        if let Err(err) = run_operate_mcp_server() {
            eprintln!("{err}");
        }
        return;
    }
    match maybe_run_portable_update_helper_from_args() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            eprintln!("[自动更新] 便携版 helper 启动失败: {err}");
            return;
        }
    }

    let state = match AppState::new() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("[启动] 初始化应用状态失败: {err}");
            return;
        }
    };
    init_last_panic_snapshot_slot(state.last_panic_snapshot.clone());
    {
        let panic_slot = state.last_panic_snapshot.clone();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic payload".to_string()
            };
            let location = info
                .location()
                .map(|loc| format!("{}:{}", loc.file(), loc.line()))
                .unwrap_or_else(|| "unknown location".to_string());
            let thread_name = std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string();
            let snapshot = format!(
                "{} thread={} payload={}",
                location.trim(),
                thread_name.trim(),
                payload.trim()
            );
            if let Ok(mut slot) = panic_slot.lock() {
                *slot = Some(snapshot);
            }
            previous_hook(info);
        }));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let chat_is_visible = app
                .get_webview_window("chat")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            let target = if chat_is_visible { "chat" } else { "main" };
            if let Err(err) = show_window(app, target) {
                eprintln!("[单实例] 激活已有实例失败: target={}, error={}", target, err);
            } else {
                eprintln!("[单实例] 已拦截重复启动并激活现有实例: target={}", target);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut_probe(app, shortcut, event.state());
                })
                .build(),
        )
        .manage(state)
        .setup(|app| {
            let app_handle = app.handle().clone();
            match app_handle.state::<AppState>().app_handle.lock() {
                Ok(mut handle_slot) => {
                    *handle_slot = Some(app_handle.clone());
                }
                Err(e) => {
                    eprintln!("[启动] 写入应用句柄槽位失败: {e}");
                }
            }
            if let Err(err) = register_default_hotkey(&app_handle) {
                eprintln!("[启动] 注册默认快捷键失败: {err}");
            }
            if let Err(err) = start_app_data_persist_worker(app_handle.state::<AppState>().inner()) {
                eprintln!("[启动] 启动后台持久化服务失败: {err}");
            }
            if let Err(err) = start_record_hotkey_probe(
                app_handle.clone(),
                app_handle.state::<AppState>().config_path.clone(),
            ) {
                eprintln!("[启动] 启动录音热键探针失败: {err}");
            }
            if let Err(err) = build_tray(&app_handle) {
                eprintln!("[启动] 构建托盘失败: {err}");
            }
            let app_state = app_handle.state::<AppState>();
            if let Err(err) = warm_hidden_skill_snapshot_cache(app_state.inner()) {
                eprintln!("[启动] 预热技能快照缓存失败: {err}");
            }
            let guard = app_state
                .conversation_lock
                .lock()
                .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
            let data = match state_read_app_data_cached(&app_state) {
                Ok(data) => data,
                Err(err) => {
                    eprintln!("[启动] 读取应用数据失败（main::setup）: {err}");
                    AppData::default()
                }
            };
            if let Err(err) = memory_store_open(&app_state.data_path) {
                eprintln!("[启动] 初始化记忆存储失败: {err}");
            }
            if let Err(err) = task_store_open(&app_state.data_path) {
                eprintln!("[启动] 初始化任务存储失败: {err}");
            }
            if let Err(err) = delegate_store_open(&app_state.data_path) {
                eprintln!("[启动] 初始化委托存储失败：{err}");
            }
            let avatar_path = data
                .agents
                .iter()
                .find(|a| a.id == data.assistant_department_agent_id)
                .and_then(|a| a.avatar_path.clone());
            drop(guard);
            let _ = sync_tray_icon_from_avatar_path(&app_handle, avatar_path.as_deref());
            attach_window_layout_persistence(&app_handle);
            hide_on_close(&app_handle);
            if let Err(err) = show_window(&app_handle, "main") {
                eprintln!("[启动] 显示主窗口失败: {err}");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            if should_enable_devtools() {
                eprintln!("[启动] 检测到 devtools 开关已开启，但当前构建未启用 open_devtools API，跳过打开 devtools");
            }
            let startup_state = app_handle.state::<AppState>().inner().clone();
            start_task_scheduler(startup_state.clone());
            tauri::async_runtime::spawn({
                let probe_state = startup_state.clone();
                async move {
                    probe_release_source_once(&probe_state).await;
                }
            });

            // 启动 OneBot v11 WebSocket 服务
            {
                let config = match state_read_config_cached(&app_handle.state::<AppState>()) {
                    Ok(config) => config,
                    Err(err) => {
                        eprintln!("[启动] 读取应用状态配置失败，使用默认配置: {:?}", err);
                        AppConfig::default()
                    }
                };
                let napcat_channels: Vec<_> = config
                    .remote_im_channels
                    .iter()
                    .filter(|ch| ch.enabled && ch.platform == RemoteImPlatform::OnebotV11)
                    .collect();
                for channel in &napcat_channels {
                    if let Err(err) = onebot_v11_ws_server_start((*channel).clone(), app_handle.clone()) {
                        eprintln!("[启动] 启动 OneBot v11 WS 服务失败，渠道 {}: {}", channel.id, err);
                    }
                }

                // 启动事件消费循环
                let event_state = app_handle.state::<AppState>().inner().clone();
                for channel in &napcat_channels {
                    let channel_id = channel.id.clone();
                    let state_clone = event_state.clone();
                    tauri::async_runtime::spawn(async move {
                        napcat_start_event_consumer(channel_id, state_clone).await;
                    });
                }

                let dingtalk_channels: Vec<_> = config
                    .remote_im_channels
                    .iter()
                    .filter(|ch| ch.enabled && ch.platform == RemoteImPlatform::Dingtalk)
                    .cloned()
                    .collect();
                for channel in dingtalk_channels {
                    let state_clone = event_state.clone();
                    let manager = dingtalk_stream_manager();
                    tauri::async_runtime::spawn(async move {
                        let channel_id = channel.id.clone();
                        if let Err(err) = manager.start_channel(channel, state_clone).await {
                            eprintln!(
                                "[启动] 启动钉钉 Stream 渠道失败: channel_id={}, error={}",
                                channel_id,
                                err
                            );
                        }
                    });
                }

                let weixin_channels: Vec<_> = config
                    .remote_im_channels
                    .iter()
                    .filter(|ch| ch.platform == RemoteImPlatform::WeixinOc)
                    .cloned()
                    .collect();
                for channel in weixin_channels {
                    let state_clone = event_state.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(err) = weixin_oc_manager()
                            .reconcile_channel_runtime(&channel, state_clone)
                            .await
                        {
                            eprintln!(
                                "[启动] 启动个人微信渠道失败: channel_id={}, error={}",
                                channel.id,
                                err
                            );
                        }
                    });
                }
            }

            tauri::async_runtime::spawn(async move {
                match mcp_redeploy_all_from_policy(&startup_state).await {
                    Ok(errors) => {
                        if !errors.is_empty() {
                            eprintln!(
                                "[启动] MCP 自动重部署完成，发生 {} 个错误",
                                errors.len()
                            );
                            for item in errors {
                                eprintln!("[启动] MCP 自动重部署异常: {} | {}", item.item, item.error);
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("[启动] MCP 自动重部署失败: {err}");
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            show_chat_window,
            show_archives_window,
            set_chat_window_active,
            check_github_update,
            start_github_update,
            get_app_version,
            get_project_repository_url,
            load_config,
            load_app_bootstrap_snapshot,
            list_system_fonts,
            save_config,
            load_agents,
            save_agents,
            get_agent_private_memory_count,
            set_agent_private_memory_enabled,
            export_agent_private_memories,
            disable_agent_private_memory,
            import_agent_memories,
            load_chat_settings,
            save_chat_settings,
            save_agent_avatar,
            clear_agent_avatar,
            read_avatar_data_url,
            read_chat_image_data_url,
            sync_tray_icon,
            save_conversation_api_settings,
            get_chat_snapshot,
            list_unarchived_conversations,
            set_active_unarchived_conversation,
            switch_active_conversation_snapshot,
            create_unarchived_conversation,
            rename_unarchived_conversation,
            mark_conversation_read,
            get_unarchived_conversation_messages,
            get_unarchived_conversation_recent_messages,
            list_delegate_conversations,
            get_delegate_conversation_messages,
            delete_unarchived_conversation,
            get_active_conversation_messages,
            get_active_conversation_messages_before,
            get_active_conversation_messages_after,
            request_conversation_messages_after_async,
            rewind_conversation_from_message,
            get_prompt_preview,
            get_system_prompt_preview,
            list_archives,
            list_memories,
            delete_memory,
            export_memories,
            export_memories_to_file,
            export_memories_to_path,
            import_memories,
            search_memories_mixed,
            sync_memory_embedding_provider,
            test_memory_embedding_provider,
            test_memory_rerank_provider,
            get_memory_provider_bindings,
            get_memory_embedding_sync_progress,
            save_memory_embedding_binding,
            save_memory_rerank_binding,
            memory_rebuild_indexes,
            memory_health_check,
            memory_backup_db,
            memory_restore_db,
            get_archive_messages,
            get_archive_summary,
            delete_archive,
            export_archive_to_file,
            import_archives_from_json,
            open_external_url,
            open_workspace_file,
            send_chat_message,
            bind_active_chat_view_stream,
             stop_chat_message,
             get_chat_queue_snapshot,
             remove_chat_queue_event,
             interrupt_conversation_runtime,
             get_main_session_state_snapshot,
            read_local_binary_file,
            queue_local_file_attachment,
            queue_inline_file_attachment,
            stt_transcribe,
            force_archive_current,
            force_compact_current,
            preview_force_archive_current,
            preview_force_compact_current,
            refresh_models,
            fetch_model_metadata,
            check_tools_status,
            list_tool_catalog,
            get_image_text_cache_stats,
            clear_image_text_cache,
            list_recent_llm_round_logs,
            clear_recent_llm_round_logs,
            list_recent_runtime_logs,
            clear_recent_runtime_logs,
            append_runtime_log_probe,
            remote_im_list_channels,
            remote_im_list_contacts,
            remote_im_update_contact_allow_send,
            remote_im_update_contact_allow_send_files,
            remote_im_update_contact_allow_receive,
            remote_im_update_contact_activation,
            remote_im_update_contact_remark,
            remote_im_update_contact_route_mode,
            remote_im_update_contact_department_binding,
            remote_im_update_contact_processing_mode,
            remote_im_delete_contact,
            remote_im_clear_contact_conversation,
            remote_im_enqueue_message,
            remote_im_list_contact_conversations,
            remote_im_get_contact_conversation_messages,
            remote_im_get_channel_status,
            remote_im_get_channel_logs,
            remote_im_restart_channel,
            remote_im_weixin_oc_start_login,
            remote_im_weixin_oc_get_login_status,
            remote_im_weixin_oc_sync_contacts,
            remote_im_weixin_oc_logout,
            desktop_screenshot,
            xcap,
            desktop_wait,
            get_host_runtime_prerequisites,
            terminal_self_check,
            list_terminal_shell_candidates,
            open_chat_shell_workspace_dir,
            reset_chat_shell_workspace,
            migrate_shell_workspace_directory,
            get_default_chat_shell_workspace_path,
            get_chat_shell_workspace,
            task_list_tasks,
            task_get_task,
            task_create_task,
            task_dispatch_task_now,
            task_update_task,
            task_complete_task,
            task_delete_task,
            task_list_run_logs,
            lock_chat_shell_workspace,
            unlock_chat_shell_workspace,
            resolve_terminal_approval,
            open_local_file_directory,
            mcp_list_servers,
            mcp_validate_definition,
            mcp_save_server,
            mcp_remove_server,
            mcp_deploy_server,
            mcp_undeploy_server,
            mcp_list_server_tools,
            mcp_list_server_tools_cached,
            mcp_set_tool_enabled,
            commands::mcp_refresh_mcp_and_skills,
            commands::mcp_list_skills,
            mcp_open_workspace_dir,
            commands::skill_open_workspace_dir
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("[启动] 运行 Tauri 应用失败: {err}");
        });
}

#[cfg(test)]
mod tests {
    include!("features/tests.rs");
}
