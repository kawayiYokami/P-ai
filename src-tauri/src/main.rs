#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use futures_util::StreamExt;
use image::ImageFormat;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rmcp::{ServiceExt, schemars};
use rig::{
    completion::{
        message::{AudioMediaType, ImageDetail, ImageMediaType, UserContent},
        Message as RigMessage, Prompt, ToolDefinition,
    },
    message::{AssistantContent, ToolResultContent},
    prelude::CompletionClient,
    providers::{anthropic, gemini, openai},
    streaming::{StreamedAssistantContent, StreamingCompletion},
    tool::{Tool, ToolDyn},
    OneOrMany,
};
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
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;


// ==================== 核心领域模型 ====================
include!("features/core/domain.rs");

// ==================== 配置与存储 ====================
include!("features/config/storage_and_stt.rs");

// ==================== 对话核心 ====================
include!("features/chat/conversation.rs");
include!("features/chat/model_runtime.rs");

// ==================== 系统窗口与命令 ====================
include!("features/system/windowing.rs");
include!("features/system/tools.rs");

// ==================== 记忆匹配 ====================
include!("features/memory/matcher.rs");

include!("features/system/commands.rs");

fn main() {
    if std::env::args().any(|arg| arg == MCP_SCREENSHOT_SERVER_FLAG) {
        if let Err(err) = run_desktop_screenshot_mcp_server() {
            eprintln!("{err}");
        }
        return;
    }

    let state = match AppState::new() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("Failed to initialize application state: {err}");
            return;
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_window(app, "chat");
                    }
                })
                .build(),
        )
        .manage(state)
        .setup(|app| {
            let app_handle = app.handle().clone();
            if let Err(err) = register_default_hotkey(&app_handle) {
                eprintln!("[BOOT] register_default_hotkey failed: {err}");
            }
            if let Err(err) = build_tray(&app_handle) {
                eprintln!("[BOOT] build_tray failed: {err}");
            }
            let app_state = app_handle.state::<AppState>();
            let guard = app_state
                .state_lock
                .lock()
                .map_err(|_| "Failed to lock state mutex".to_string())?;
            let mut data = read_app_data(&app_state.data_path).unwrap_or_default();
            let changed = ensure_default_agent(&mut data);
            if changed {
                let _ = write_app_data(&app_state.data_path, &data);
            }
            let avatar_path = data
                .agents
                .iter()
                .find(|a| a.id == data.selected_agent_id)
                .and_then(|a| a.avatar_path.clone());
            drop(guard);
            let _ = sync_tray_icon_from_avatar_path(&app_handle, avatar_path.as_deref());
            hide_on_close(&app_handle);
            if let Err(err) = show_window(&app_handle, "main") {
                eprintln!("[BOOT] show_window(main) failed: {err}");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            load_agents,
            save_agents,
            load_chat_settings,
            save_chat_settings,
            save_agent_avatar,
            clear_agent_avatar,
            read_avatar_data_url,
            sync_tray_icon,
            save_conversation_api_settings,
            get_chat_snapshot,
            get_active_conversation_messages,
            get_prompt_preview,
            get_system_prompt_preview,
            list_archives,
            list_memories,
            export_memories,
            export_memories_to_file,
            export_memories_to_path,
            import_memories,
            get_archive_messages,
            delete_archive,
            export_archive_to_file,
            open_external_url,
            send_chat_message,
            force_archive_current,
            refresh_models,
            check_tools_status,
            get_image_text_cache_stats,
            clear_image_text_cache,
            send_debug_probe,
            desktop_screenshot,
            desktop_wait
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("error while running tauri application: {err}");
        });
}

#[cfg(test)]
mod tests {
    include!("features/tests.rs");
}
