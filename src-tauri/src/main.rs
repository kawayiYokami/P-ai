#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use directories::ProjectDirs;
use futures_util::{future::AbortHandle, future::join_all, future::BoxFuture, StreamExt};
use image::ImageFormat;
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rmcp::{schemars, ServiceExt};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, Position, State,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};
use uuid::Uuid;

macro_rules! eprintln {
    ($($arg:tt)*) => {{
        runtime_log_info(format!($($arg)*));
    }};
}

fn bytes_to_lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

// ==================== 核心领域模型 ====================
include!("features/core/domain.rs");
include!("features/core/time_semantics.rs");

// ==================== 配置与存储 ====================
include!("features/config/storage_and_stt.rs");
include!("features/config/app_data_layout.rs");
include!("features/config/agent_org_migration.rs");
include!("features/state/mod.rs");
include!("features/chat/message_store/mod.rs");
use easy_call_ai::pai_config_tool;

// ==================== 独立图像生成 ====================
include!("features/image_generation.rs");

// ==================== 对话核心 ====================
include!("features/chat/message_semantics.rs");
include!("features/chat/conversation.rs");
include!("features/chat/message_attachment_projection.rs");
include!("features/chat/prompt_manager.rs");
include!("features/chat/conversation_prompt_service.rs");
include!("features/chat/conversation_service/mod.rs");
include!("features/chat/model_runtime.rs");
include!("features/chat/scheduler.rs");
include!("features/remote_im/channel_store.rs");
include!("features/remote_im/markdown_filter.rs");
include!("features/remote_im/onebot_v11_ws.rs");
include!("features/remote_im/dingtalk_stream.rs");
include!("features/remote_im/weixin_oc.rs");
include!("features/remote_im.rs");
include!("features/remote_im/maintenance.rs");
include!("features/remote_im_adapters.rs");

// ==================== 系统窗口与命令 ====================
include!("features/system/windowing.rs");
// WebView2 进程失败监控只在 Windows 有宿主能力，整块在非 Windows 下不参与编译；
// test 下保留编译，以便校验其中的纯逻辑（失败种类映射与恢复分档）。
#[cfg(any(target_os = "windows", test))]
mod webview_health {
    include!("features/system/webview_health.rs");
}
include!("features/system/record_hotkey_probe.rs");
include!("features/system/windows_job.rs");
include!("features/system/execution.rs");
include!("features/system/local_port_service.rs");
include!("features/system/tools.rs");
include!("features/system/updater.rs");

// ==================== 记忆匹配 ====================
include!("features/memory/store.rs");
include!("features/memory/matcher.rs");
include!("features/memory/chat_history_search.rs");
include!("features/memory/deep_recall.rs");
include!("features/memory/providers.rs");

// ==================== MCP ====================
include!("features/mcp.rs");
include!("features/skill.rs");
include!("features/catalog.rs");
include!("features/goal.rs");
include!("features/task.rs");
include!("features/delegate.rs");
include!("features/monitor_events.rs");

include!("features/system/commands.rs");
include!("features/system/headless_cli.rs");

#[cfg(target_os = "windows")]
fn windows_set_process_app_user_model_id() {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
            REG_OPTION_NON_VOLATILE, REG_SZ,
        },
        UI::Shell::SetCurrentProcessExplicitAppUserModelID,
    };

    const AUMID: &str = "ai.easycall.app";
    const DISPLAY_NAME: &str = "PAI";

    // 1. 进程级 AUMID：让 toast 发送方身份与插件使用的 identifier 一致
    let aumid_wide: Vec<u16> = AUMID.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(aumid_wide.as_ptr()) };
    if result != 0 {
        runtime_log_warn(format!(
            "[通知] 设置进程 AppUserModelID 失败: 0x{:X}",
            result
        ));
    }

    // 2. HKCU 注册 AUMID（DisplayName/IconUri）：未打包应用的标准身份来源，
    //    无安装器快捷方式时通知中心也按品牌名+图标显示；幂等，每次启动重写
    let subkey: Vec<u16> = format!(r"Software\Classes\AppUserModelId\{AUMID}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key_handle: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key_handle,
            std::ptr::null_mut(),
        )
    };
    if status != ERROR_SUCCESS {
        runtime_log_warn(format!("[通知] 注册 AUMID 键失败: 0x{:X}", status));
        return;
    }
    let set_value = |name: &str, value: &str| {
        let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let value_wide: Vec<u16> = value.encode_utf16().collect();
        let status = unsafe {
            RegSetValueExW(
                key_handle,
                name_wide.as_ptr(),
                0,
                REG_SZ,
                value_wide.as_ptr() as *const u8,
                (value_wide.len() * 2) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            runtime_log_warn(format!("[通知] 写入 AUMID {name} 失败: 0x{:X}", status));
        }
    };
    set_value("DisplayName", DISPLAY_NAME);
    if let Ok(exe) = std::env::current_exe() {
        set_value("IconUri", &exe.to_string_lossy());
    }
    unsafe {
        RegCloseKey(key_handle);
    }
}

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

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_extend_process_path_from_login_shell() {
    use users::os::unix::UserExt;
    const PATH_BEGIN: &str = "__PAI_LOGIN_PATH_BEGIN__";
    const PATH_END: &str = "__PAI_LOGIN_PATH_END__";
    let shell = std::env::var_os("SHELL")
        .filter(|path| PathBuf::from(path).is_file())
        .or_else(|| {
            users::get_user_by_uid(users::get_current_uid())
                .map(|user| user.shell().as_os_str().to_os_string())
                .filter(|path| PathBuf::from(path).is_file())
        })
        .unwrap_or_else(|| "/bin/sh".into());
    let mut child = match std::process::Command::new(&shell)
        .args([
            "-ilc",
            "printf '__PAI_LOGIN_PATH_BEGIN__%s__PAI_LOGIN_PATH_END__' \"$PATH\"",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            runtime_log_warn(format!("[启动] 启动登录 shell 失败: err={err}"));
            return;
        }
    };
    let Some(stdout_pipe) = child.stdout.take() else {
        runtime_log_warn("[启动] 获取登录 shell stdout 失败，跳过环境同步".to_string());
        return;
    };
    // 读线程负责读 stdout 直到 EOF：shell 退出但 rc 启动的后台进程仍持有
    // stdout 写端时，阻塞只发生在子线程，不卡主线程（超时 kill 后由进程退出回收）。
    let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::BufReader::new(stdout_pipe), &mut bytes);
        let _ = stdout_tx.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut stdout = None;
    let status = loop {
        if stdout.is_none() {
            if let Ok(buf) = stdout_rx.try_recv() {
                stdout = Some(buf);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if stdout.is_some() => break status,
            Ok(_) => {}
            Err(err) => {
                runtime_log_warn(format!("[启动] 等待登录 shell 退出失败: err={err}"));
                return;
            }
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            runtime_log_warn("[启动] 读取登录 shell PATH 超时，跳过环境同步".to_string());
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stdout = stdout.unwrap_or_default();
    if !status.success() {
        runtime_log_warn(format!(
            "[启动] 读取登录 shell PATH 失败: exit_code={}",
            status.code().unwrap_or(-1)
        ));
        return;
    }
    let Some(path_start) = stdout.rfind(PATH_BEGIN).map(|index| index + PATH_BEGIN.len()) else {
        runtime_log_warn(format!("[启动] 登录 shell 未返回 PATH 标记，跳过环境同步"));
        return;
    };
    let Some(path_end) = stdout[path_start..]
        .find(PATH_END)
        .map(|index| path_start + index)
    else {
        runtime_log_warn(format!("[启动] 登录 shell PATH 标记不完整，跳过环境同步"));
        return;
    };
    let login_path = std::ffi::OsString::from(stdout[path_start..path_end].trim());
    if login_path.is_empty() {
        runtime_log_warn(format!("[启动] 登录 shell PATH 为空，跳过环境同步"));
        return;
    }

    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut merged = Vec::<PathBuf>::new();
    for path in std::env::split_paths(&login_path).chain(std::env::split_paths(&current_path)) {
        if !path.as_os_str().is_empty() && !merged.iter().any(|item| item == &path) {
            merged.push(path);
        }
    }
    let Ok(path) = std::env::join_paths(merged) else {
        runtime_log_warn(format!("[启动] 合并登录 shell PATH 失败，跳过环境同步"));
        return;
    };
    std::env::set_var("PATH", path);
    runtime_log_info(format!("[启动] 已同步登录 shell PATH"));
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn unix_extend_process_path_from_login_shell() {}

fn install_tauri_async_runtime() -> Result<tokio::runtime::Runtime, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .map_err(|err| format!("创建 Tauri 异步运行时失败: {err}"))?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 必须在 Tauri Builder 初始化前设置，否则会命中 tauri::async_runtime::set 的已初始化 panic。
        tauri::async_runtime::set(runtime.handle().clone());
    }))
    .map_err(|panic_payload| {
        let panic_text = panic_payload
            .downcast_ref::<&str>()
            .map(|value| (*value).to_string())
            .or_else(|| panic_payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "未知 panic".to_string());
        format!("设置 Tauri 异步运行时失败: {panic_text}")
    })?;
    Ok(runtime)
}

#[tauri::command]
fn demo_restart_app(app: AppHandle) -> Result<(), String> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        graceful_restart_app(&app);
    });
    Ok(())
}

// Remote IM 命令包装
#[tauri::command]
async fn remote_im_get_channel_status(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<ChannelConnectionStatus, String> {
    remote_im_get_channel_status_inner(state.inner(), channel_id).await
}

#[tauri::command]
async fn remote_im_get_channel_logs(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChannelLogEntry>, String> {
    get_remote_im_channel_logs(state.inner(), channel_id).await
}

#[tauri::command]
async fn remote_im_get_contact_logs(
    input: RemoteImContactLogsInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChannelLogEntry>, String> {
    remote_im_get_contact_logs_inner(state.inner(), input).await
}

#[tauri::command]
async fn remote_im_restart_channel(
    channel_id: String,
    state: State<'_, AppState>,
) -> Result<ChannelConnectionStatus, String> {
    remote_im_restart_channel_inner(channel_id, state.inner()).await
}

#[tauri::command]
async fn frontend_ready_start_remote_im_services(app: AppHandle) -> Result<bool, String> {
    static BACKGROUND_SERVICES_START_REQUESTED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    let startup_state = app.state::<AppState>().inner().clone();
    require_message_store_migration_completed_for_runtime(
        &startup_state,
        "启动远程 IM 与后台服务",
    )?;
    if BACKGROUND_SERVICES_START_REQUESTED
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        runtime_log_warn(format!("[启动] 前端就绪后启动后台服务：已请求过，跳过重复启动"));
        return Ok(false);
    }

    refresh_conversation_meta_after_migration(startup_state.clone()).await;
    runtime_log_info(format!("[启动] 迁移门闩已完成，开始异步启动后台服务"));
    tauri::async_runtime::spawn(async move {
        start_background_services_after_frontend_ready(app, startup_state).await;
    });
    Ok(true)
}

async fn refresh_conversation_meta_after_migration(state: AppState) {
    let started = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        let chat_index = collect_chat_index_items_from_storage(&state.data_path)?;
        let mut refreshed = 0usize;
        for item in &chat_index {
            if refresh_conversation_meta_shard_if_needed(&state.data_path, item.id.as_str())? {
                refreshed += 1;
            }
        }
        Ok::<(usize, usize), String>((chat_index.len(), refreshed))
    })
    .await;
    match result {
        Ok(Ok((total, refreshed))) => runtime_log_info(format!(
            "[启动] 完成，任务=迁移后会话meta预热，conversation_count={}，refreshed={}，elapsed_ms={}",
            total,
            refreshed,
            started.elapsed().as_millis()
        )),
        Ok(Err(err)) => runtime_log_warn(format!(
            "[启动] 失败，任务=迁移后会话meta预热，error={}，elapsed_ms={}",
            err,
            started.elapsed().as_millis()
        )),
        Err(err) => runtime_log_warn(format!(
            "[启动] 失败，任务=迁移后会话meta预热，error={}，elapsed_ms={}",
            err,
            started.elapsed().as_millis()
        )),
    }
}

/// 阶段 2 延迟初始化：在 backend_ready 之后异步执行，避免阻塞前端首屏渲染。
async fn run_deferred_setup(app_handle: AppHandle) {
    let app_state = app_handle.state::<AppState>();

    let log_step = |step: &str| {
        runtime_log_info(format!("[启动-延迟] 开始: {step}"));
    };

    log_step("注册快捷键");
    if let Err(err) = register_default_hotkey(&app_handle) {
        runtime_log_error(format!("[启动-延迟] 注册默认快捷键失败: {err}"));
    }
    log_step("挂载 WebView2 进程失败监控");
    #[cfg(target_os = "windows")]
    webview_health::attach_webview_process_failed_monitors(&app_handle);
    log_step("启动持久化服务");
    if let Err(err) = start_conversation_persist_worker(app_state.inner()) {
        runtime_log_error(format!("[启动-延迟] 启动会话后台持久化服务失败: {err}"));
    }
    // 工作区加载（铺 skills、填快照缓存、派发 MCP 探测）只依赖 AppState，与前端无耦合，
    // 因此归入后端阶段 2 启动序列，不再搭前端就绪回调的便车；无头入口走同一条加载。
    // 该函数不等待 MCP 探测（探测是内部 detached spawn），await 段只有同步文件与内存操作，
    // 所以内联 await 不会拖住阶段 2 后续步骤，也保留了「加载先于后续启动步骤」的串行顺序。
    log_step("加载工作区（skill/MCP）");
    match load_workspace(app_state.inner()).await {
        Ok(result) => log_workspace_load_result("[工作区加载]", &result),
        Err(err) => runtime_log_error(format!("[工作区加载] 状态=失败，error={err}")),
    }
    let recovery_state = app_state.inner().clone();
    tauri::async_runtime::spawn(async move {
        for attempt in 0..6u8 {
            match remote_im_recover_all_group_reply_delivery_markers(&recovery_state) {
                Ok((recovered, 0)) => {
                    if recovered > 0 {
                        runtime_log_info(format!(
                            "[群聊巡检] 启动恢复完成，recovered_count={}",
                            recovered
                        ));
                    }
                    break;
                }
                Ok((recovered, failed)) => runtime_log_warn(format!(
                    "[群聊巡检] 启动恢复部分失败，attempt={}，recovered_count={}，failed_count={}",
                    attempt.saturating_add(1),
                    recovered,
                    failed
                )),
                Err(err) => runtime_log_warn(format!(
                    "[群聊巡检] 启动恢复读取失败，attempt={}，error={}",
                    attempt.saturating_add(1),
                    err
                )),
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                5u64.saturating_mul(1u64 << attempt.min(3)),
            ))
            .await;
        }
    });
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let wayland_only =
            cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some();
        if wayland_only {
            runtime_log_info("[启动-延迟] Wayland 会话不支持按键监听，跳过录音热键探针".to_string());
        } else {
            log_step("启动录音热键探针");
            if let Err(err) = start_record_hotkey_probe(
                app_handle.clone(),
                app_state.config_path.clone(),
            ) {
                runtime_log_error(format!("[启动-延迟] 启动录音热键探针失败: {err}"));
            }
        }
    }
    log_step("配置自检");
    match state_read_config_cached(app_state.inner()) {
        Ok(mut config) => {
            if run_startup_self_checks(&mut config) {
                if let Err(err) = state_write_config_cached(app_state.inner(), &config) {
                    runtime_log_error(format!("[启动自检] 写入修复后的配置失败: {err}"));
                } else {
                    runtime_log_info(format!("[启动自检] 完成，已将副手模型从默认人格修正为副手"));
                }
            }
        }
        Err(err) => {
            runtime_log_error(format!("[启动自检] 读取配置失败: {err}"));
        }
    }
    log_step("初始化记忆存储");
    if let Err(err) = memory_store_open(&app_state.data_path) {
        runtime_log_error(format!("[启动-延迟] 初始化记忆存储失败: {err}"));
    }
    // 启动期记忆向量增量同步: 复用 memory_store_sync_provider_index 的差集逻辑
    // (补未向量化的记忆、清孤儿 chunk)。异步执行, 不阻塞启动; 失败由日志记录,
    // 因记忆增删也会触发增量, 下次启动会再次兜底。
    let sync_data_path = app_state.data_path.clone();
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            memory_sync_vectors_on_startup(&sync_data_path)
        })
        .await;
        match result {
            Ok(Ok(report)) => {
                runtime_log_info(format!(
                    "[记忆向量同步] 完成，任务=startup_sync，status={}，deleted={}，added={}，耗时毫秒={}",
                    report.status,
                    report.deleted,
                    report.added,
                    started.elapsed().as_millis(),
                ));
            }
            Ok(Err(err)) => {
                runtime_log_warn(format!(
                    "[记忆向量同步] 失败，任务=startup_sync，异常={}，耗时毫秒={}",
                    err,
                    started.elapsed().as_millis(),
                ));
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[记忆向量同步] 失败，任务=startup_sync，异常=join_error，详情={}，耗时毫秒={}",
                    err,
                    started.elapsed().as_millis(),
                ));
            }
        }
    });
    log_step("初始化任务存储");
    if let Err(err) = task_store_open(&app_state.data_path) {
        runtime_log_error(format!("[启动-延迟] 初始化任务存储失败: {err}"));
    }
    log_step("初始化委托存储");
    if let Err(err) = delegate_store_open(&app_state.data_path) {
        runtime_log_error(format!("[启动-延迟] 初始化委托存储失败：{err}"));
    } else {
        if let Err(err) = delegate_snapshot_cache_ensure_loaded(&app_state.data_path) {
            runtime_log_error(format!("[启动-延迟] 初始化委托快照失败：{err}"));
        }
        match delegate_store_interrupt_unfinished_remote_replies(&app_state.data_path) {
            Ok(delegate_ids) => {
                for delegate_id in delegate_ids {
                    if let Err(err) = delegate_runtime_thread_archive(app_state.inner(), &delegate_id, &now_iso()) {
                        runtime_log_error(format!("[启动-延迟] 归档中断远程委托失败，delegate_id={delegate_id}，error={err}"));
                    }
                }
            }
            Err(err) => runtime_log_error(format!("[启动-延迟] 收口未完成远程委托失败：{err}")),
        }
    }
    let ide_context_runtime = app_handle.state::<IdeContextRuntime>().inner().clone();
    start_web_access_server(
        app_handle.clone(),
        app_state.inner().clone(),
        ide_context_runtime,
    )
    .await;
    let _ = sync_default_tray_icon(&app_handle);
    if should_enable_devtools() {
        runtime_log_warn(format!("[启动-延迟] 检测到 devtools 开关已开启，但当前构建未启用 open_devtools API，跳过打开 devtools"));
    }
    runtime_log_info(format!("[启动-延迟] 阶段 2 初始化完成"));
}

async fn start_background_services_after_frontend_ready(
    app_handle: AppHandle,
    startup_state: AppState,
) {
    start_task_scheduler(startup_state.clone());
    tauri::async_runtime::spawn({
        let probe_state = startup_state.clone();
        async move {
            probe_release_source_once(&probe_state).await;
        }
    });
    // 工作区加载已移到后端阶段 2（run_deferred_setup），此处只负责远程 IM 等
    // 确实需要等前端就绪的服务，不再顺带加载 skill/MCP。
    start_remote_im_services_after_frontend_ready(app_handle.clone()).await;
}

async fn start_remote_im_services_after_frontend_ready(app_handle: AppHandle) {
    let app_state = app_handle.state::<AppState>();
    let config = match state_read_config_cached(&app_state) {
        Ok(mut config) => {
            if let Err(err) =
                remote_im_migrate_channel_private_states(app_state.inner(), &mut config)
                    .and_then(|changed| {
                        if changed {
                            state_write_config_cached(app_state.inner(), &config)?;
                        }
                        Ok(())
                    })
            {
                runtime_log_error(format!("[启动] 远程 IM 私有状态迁移失败，继续按现有配置启动: {}", err));
            }
            config
        }
        Err(err) => {
            runtime_log_warn(format!("[启动] 前端就绪后读取远程 IM 配置失败，跳过启动: {:?}", err));
            return;
        }
    };
    let event_state = app_handle.state::<AppState>().inner().clone();

    let napcat_channels: Vec<_> = config
        .remote_im_channels
        .iter()
        .filter(|ch| ch.enabled && ch.platform == RemoteImPlatform::OnebotV11)
        .cloned()
        .collect();
    let mut started_napcat_channels = Vec::new();
    for channel in &napcat_channels {
        onebot_v11_ws_manager()
            .add_log(&channel.id, "info", "[启动] 前端就绪，准备启动 OneBot v11 WS 服务")
            .await;
        let effective_channel =
            match remote_im_channel_with_effective_credentials(&event_state, channel) {
                Ok(value) => value,
                Err(err) => {
                    onebot_v11_ws_manager()
                        .add_log(
                            &channel.id,
                            "error",
                            "[启动] OneBot v11 渠道私有状态读取失败，详情请查看后端诊断日志",
                        )
                        .await;
                    runtime_log_error(format!(
                        "[启动] OneBot v11 渠道私有状态读取失败: channel_id={}, error={}",
                        channel.id, err
                    ));
                    continue;
                }
            };
        match onebot_v11_ws_server_start(effective_channel).await {
            Ok(()) => {
                started_napcat_channels.push(channel.clone());
            }
            Err(err) => {
                onebot_v11_ws_manager()
                    .add_log(
                        &channel.id,
                        "error",
                        &format!("[启动] OneBot v11 WS 服务启动失败: {}", err),
                    )
                    .await;
                runtime_log_error(format!(
                    "[启动] 前端就绪后启动 OneBot v11 WS 服务失败: channel_id={}, error={}",
                    channel.id, err
                ));
            }
        }
    }
    for channel in started_napcat_channels {
        let channel_id = channel.id.clone();
        let state_clone = event_state.clone();
        if let Err(err) = onebot_v11_ws_manager()
            .start_event_consumer(channel_id.clone(), state_clone)
            .await
        {
            onebot_v11_ws_manager()
                .add_log(
                    &channel_id,
                    "error",
                    &format!("[启动] OneBot v11 事件消费器启动失败: {}", err),
                )
                .await;
            runtime_log_error(format!(
                "[启动] 前端就绪后启动 OneBot v11 事件消费器失败: channel_id={}, error={}",
                channel_id, err
            ));
        }
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
            manager
                .add_log(&channel_id, "info", "[启动] 前端就绪，准备启动钉钉 Stream 渠道")
                .await;
            let effective_channel =
                match remote_im_channel_with_effective_credentials(&state_clone, &channel) {
                    Ok(value) => value,
                    Err(err) => {
                        manager
                            .add_log(
                                &channel_id,
                                "error",
                                "[启动] 钉钉渠道私有状态读取失败，详情请查看后端诊断日志",
                            )
                            .await;
                        runtime_log_error(format!(
                            "[启动] 钉钉渠道私有状态读取失败: channel_id={}, error={}",
                            channel_id, err
                        ));
                        return;
                    }
                };
            if let Err(err) = manager.start_channel(effective_channel, state_clone).await {
                manager
                    .add_log(
                        &channel_id,
                        "error",
                        &format!("[启动] 钉钉 Stream 渠道启动失败: {}", err),
                    )
                    .await;
                runtime_log_error(format!(
                    "[启动] 前端就绪后启动钉钉 Stream 渠道失败: channel_id={}, error={}",
                    channel_id, err
                ));
            }
        });
    }

    let weixin_channels: Vec<_> = config
        .remote_im_channels
        .iter()
        .filter(|ch| ch.enabled && ch.platform == RemoteImPlatform::WeixinOc)
        .cloned()
        .collect();
    for channel in weixin_channels {
        let state_clone = event_state.clone();
        tauri::async_runtime::spawn(async move {
            weixin_oc_manager()
                .add_log(&channel.id, "info", "[启动] 前端就绪，准备启动个人微信渠道")
                .await;
            if let Err(err) = weixin_oc_manager()
                .reconcile_channel_runtime(&channel, state_clone)
                .await
            {
                weixin_oc_manager()
                    .add_log(
                        &channel.id,
                        "error",
                        "[启动] 个人微信渠道启动失败，详情请查看后端诊断日志",
                    )
                    .await;
                runtime_log_error(format!(
                    "[启动] 前端就绪后启动个人微信渠道失败: channel_id={}, error={}",
                    channel.id, err
                ));
            }
        });
    }
}

const APP_SHUTDOWN_STATE_IDLE: u8 = 0;
const APP_SHUTDOWN_STATE_RUNNING: u8 = 1;
const APP_SHUTDOWN_STATE_DONE: u8 = 2;
const BACKGROUND_SHUTDOWN_TIMEOUT_SECS: u64 = 60;

static APP_SHUTDOWN_STATE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(APP_SHUTDOWN_STATE_IDLE);

fn show_background_shutdown_timeout_dialog(app: &AppHandle) {
    let message = "自动关闭失败，请手动关闭应用重启";
    runtime_log_info(format!("[退出] {message}"));
    app.dialog()
        .message(message)
        .title("自动关闭失败")
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

fn show_background_shutdown_timeout_dialog_then_exit(app: &AppHandle, code: i32) {
    let message = "自动关闭失败，请手动关闭应用重启";
    runtime_log_info(format!("[退出] {message}"));
    let app_handle = app.clone();
    app.dialog()
        .message(message)
        .title("自动关闭失败")
        .kind(MessageDialogKind::Error)
        .show(move |_| {
            app_handle.exit(code);
        });
}

async fn graceful_shutdown_background_services(app: &AppHandle) {
    let shutdown_started = APP_SHUTDOWN_STATE.compare_exchange(
        APP_SHUTDOWN_STATE_IDLE,
        APP_SHUTDOWN_STATE_RUNNING,
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
    );
    if shutdown_started.is_err() {
        while APP_SHUTDOWN_STATE.load(std::sync::atomic::Ordering::SeqCst)
            == APP_SHUTDOWN_STATE_RUNNING
        {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        runtime_log_info(format!("[退出] 已等待进行中的后台服务优雅停机完成"));
        return;
    }

    let state = app.state::<AppState>().inner().clone();
    let started_at = std::time::Instant::now();
    runtime_log_info(format!("[退出] 开始优雅停机后台服务"));

    let config = match state_read_config_cached(&state) {
        Ok(config) => Some(config),
        Err(err) => {
            runtime_log_warn(format!("[退出] 读取配置失败，继续按兜底方式停机: {err}"));
            None
        }
    };

    if let Some(config) = config.as_ref() {
        let mut shutdown_futures = Vec::<BoxFuture<'static, ()>>::new();
        for channel in &config.remote_im_channels {
            let channel_id = channel.id.trim().to_string();
            if channel_id.is_empty() {
                continue;
            }
            match channel.platform {
                RemoteImPlatform::OnebotV11 => shutdown_futures.push(Box::pin(async move {
                    let stop_started = std::time::Instant::now();
                    match onebot_v11_ws_manager().stop_channel(&channel_id).await {
                        Ok(()) => runtime_log_info(format!(
                            "[退出] OneBot 渠道已停止: channel_id={}，duration_ms={}",
                            channel_id,
                            stop_started.elapsed().as_millis()
                        )),
                        Err(err) => runtime_log_error(format!(
                            "[退出] OneBot 渠道停止异常，底层已执行强制清理: channel_id={}，duration_ms={}，error={}",
                            channel_id,
                            stop_started.elapsed().as_millis(),
                            err
                        )),
                    }
                })),
                RemoteImPlatform::Dingtalk => shutdown_futures.push(Box::pin(async move {
                    let stop_started = std::time::Instant::now();
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        dingtalk_stream_manager().stop_channel(&channel_id),
                    )
                    .await
                    {
                        Ok(()) => runtime_log_info(format!(
                            "[退出] 钉钉渠道已停止: channel_id={}，duration_ms={}",
                            channel_id,
                            stop_started.elapsed().as_millis()
                        )),
                        Err(_) => runtime_log_error(format!(
                            "[退出] 钉钉渠道停止超时: channel_id={}，timeout_ms=10000",
                            channel_id
                        )),
                    }
                })),
                RemoteImPlatform::WeixinOc => shutdown_futures.push(Box::pin(async move {
                    let stop_started = std::time::Instant::now();
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        weixin_oc_manager().stop_channel(&channel_id),
                    )
                    .await
                    {
                        Ok(()) => runtime_log_info(format!(
                            "[退出] 个人微信渠道已停止: channel_id={}，duration_ms={}",
                            channel_id,
                            stop_started.elapsed().as_millis()
                        )),
                        Err(_) => runtime_log_error(format!(
                            "[退出] 个人微信渠道停止超时: channel_id={}，timeout_ms=10000",
                            channel_id
                        )),
                    }
                })),
                RemoteImPlatform::Feishu => {}
            }
        }
        join_all(shutdown_futures).await;
    }

    // 兜底广播关闭，确保仍持有 shutdown receiver 的旧渠道尽快退出 accept 循环。
    onebot_v11_ws_manager().shutdown().await;

    let ide_bridge_started_at = std::time::Instant::now();
    shutdown_web_access_server().await;
    runtime_log_info(format!(
        "[退出] Web 访问服务已停止: duration_ms={}",
        ide_bridge_started_at.elapsed().as_millis()
    ));

    match load_workspace_mcp_servers(&state) {
        Ok(servers) => {
            let shutdown_futures = servers
                .into_iter()
                .filter_map(|server| {
                    let server_id = server.id.trim().to_string();
                    if server_id.is_empty() {
                        return None;
                    }
                    Some(Box::pin(async move {
                        let disconnect_started = std::time::Instant::now();
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            mcp_disconnect_cached_client(&server_id),
                        )
                        .await
                        {
                            Ok(()) => runtime_log_info(format!(
                                "[退出] MCP 已断开: server_id={}，duration_ms={}",
                                server_id,
                                disconnect_started.elapsed().as_millis()
                            )),
                            Err(_) => runtime_log_error(format!(
                                "[退出] MCP 断开超时: server_id={}，timeout_ms=10000",
                                server_id
                            )),
                        }
                    }) as BoxFuture<'static, ()>)
                })
                .collect::<Vec<BoxFuture<'static, ()>>>();
            join_all(shutdown_futures).await;
        }
        Err(err) => {
            runtime_log_warn(format!("[退出] 读取 MCP 工作区失败，跳过逐个断开: {err}"));
        }
    }

    // 退出前同步排空持久化队列：后台 worker 采用 120ms 去抖，
    // 落在该窗口内的会话/应用数据写入若不在此 flush，会随进程退出丢失。
    let persist_flush_started_at = std::time::Instant::now();
    match flush_pending_persists_blocking(&state) {
        Ok(wrote) => runtime_log_info(format!(
            "[退出] 持久化队列已排空: wrote={}，duration_ms={}",
            wrote,
            persist_flush_started_at.elapsed().as_millis()
        )),
        Err(err) => runtime_log_error(format!(
            "[退出] 持久化队列排空失败，可能丢失最后一次写入: duration_ms={}，error={}",
            persist_flush_started_at.elapsed().as_millis(),
            err
        )),
    }

    runtime_log_info(format!(
        "[退出] 后台服务优雅停机完成: duration_ms={}",
        started_at.elapsed().as_millis()
    ));
    APP_SHUTDOWN_STATE.store(APP_SHUTDOWN_STATE_DONE, std::sync::atomic::Ordering::SeqCst);
}

async fn graceful_shutdown_background_services_with_timeout(app: &AppHandle) -> bool {
    match tokio::time::timeout(
        std::time::Duration::from_secs(BACKGROUND_SHUTDOWN_TIMEOUT_SECS),
        graceful_shutdown_background_services(app),
    )
    .await
    {
        Ok(()) => true,
        Err(_) => {
            APP_SHUTDOWN_STATE.store(
                APP_SHUTDOWN_STATE_IDLE,
                std::sync::atomic::Ordering::SeqCst,
            );
            runtime_log_error(format!("[退出] 后台服务自动关闭超过 {} 秒", BACKGROUND_SHUTDOWN_TIMEOUT_SECS));
            false
        }
    }
}

fn graceful_shutdown_background_services_blocking(app: &AppHandle) -> bool {
    tauri::async_runtime::block_on(graceful_shutdown_background_services_with_timeout(app))
}

fn graceful_exit_app(app: &AppHandle, code: i32) {
    if graceful_shutdown_background_services_blocking(app) {
        app.exit(code);
    } else {
        show_background_shutdown_timeout_dialog_then_exit(app, code);
    }
}

fn graceful_restart_app(app: &AppHandle) {
    if graceful_shutdown_background_services_blocking(app) {
        app.restart();
    } else {
        show_background_shutdown_timeout_dialog(app);
    }
}

fn main() {
    // 用 args_os 兜住非 UTF-8 参数：std::env::args 在迭代时会 panic，会绕开退出码约定。
    let argv: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if headless_cli_invocation(&argv) {
        let parsed = match parse_headless_cli_args(&argv) {
            Ok(parsed) => parsed,
            Err(err) => std::process::exit(headless_cli_fail(&err)),
        };
        headless_cli_enter_mode(parsed.minimal_tools);
        std::process::exit(run_headless_cli(parsed));
    }

    #[cfg(target_os = "windows")]
    windows_set_process_app_user_model_id();
    init_backend_file_logging();
    install_backend_file_panic_hook();
    unix_extend_process_path_from_login_shell();

    if argv.iter().any(|arg| arg.as_str() == MCP_SCREENSHOT_SERVER_FLAG) {
        if let Err(err) = run_desktop_screenshot_mcp_server() {
            runtime_log_info(format!("{err}"));
        }
        return;
    }
    match maybe_run_portable_update_helper_from_args() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            runtime_log_error(format!("[自动更新] 便携版 helper 启动失败: {err}"));
            return;
        }
    }

    let _tauri_runtime = match install_tauri_async_runtime() {
        Ok(runtime) => runtime,
        Err(err) => {
            runtime_log_error(format!("[启动] 初始化 Tokio 运行时失败: {err}"));
            return;
        }
    };

    let state = match AppState::new() {
        Ok(state) => state,
        Err(err) => {
            runtime_log_error(format!("[启动] 初始化应用状态失败: {err}"));
            return;
        }
    };
    if let Err(err) = cleanup_portable_update_temp_artifacts_for_current_runtime() {
        runtime_log_error(format!("[自动更新] 清理便携版更新临时文件失败: {err}"));
    }
    if let Err(err) = run_state_migration_if_needed(&state) {
        runtime_log_error(format!("[启动] 执行 state 迁移失败: {err}"));
    }
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

    let builder = tauri::Builder::default();
    let builder = if cfg!(debug_assertions) {
        runtime_log_warn(format!("[单实例] 当前为调试构建，跳过单实例拦截"));
        builder
    } else {
        builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let chat_is_visible = app
                .get_webview_window("chat")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            let target = if chat_is_visible {
                "chat"
            } else {
                match state_read_config_cached(app.state::<AppState>().inner()) {
                    Ok(mut config) => {
                        normalize_app_config(&mut config);
                        startup_window_label_for_config(&config)
                    }
                    Err(err) => {
                        runtime_log_error(format!("[单实例] 读取启动窗口配置失败: {err}"));
                        "main"
                    }
                }
            };
            if let Err(err) = show_window(app, target) {
                runtime_log_error(format!(
                    "[单实例] 激活已有实例失败: target={}, error={}",
                    target, err
                ));
            } else {
                runtime_log_info(format!("[单实例] 已拦截重复启动并激活现有实例: target={}", target));
            }
        }))
    };

    let ide_context_runtime = IdeContextRuntime::new();

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut_probe(app, shortcut, event.state());
                })
                .build(),
        )
        .manage(state)
        .manage(ide_context_runtime)
        .setup(|app| {
            let app_handle = app.handle().clone();
            match app_handle.state::<AppState>().app_handle.lock() {
                Ok(mut handle_slot) => {
                    *handle_slot = Some(app_handle.clone());
                }
                Err(e) => {
                    runtime_log_error(format!("[启动] 写入应用句柄槽位失败: {e}"));
                }
            }
            if let Err(err) = build_tray(&app_handle) {
                runtime_log_error(format!("[启动] 构建托盘失败: {err}"));
            } else {
                runtime_log_info(format!("[启动] 构建托盘完成"));
            }

            // ========== 阶段 1：最小启动，尽快让前端可见 ==========
            let app_state = app_handle.state::<AppState>();
            // 旧配置键只在迁移模块内可见，且任何一次配置落盘都会让它消失；
            // 这里必须早于启动窗口判定与后续所有配置写。
            if let Err(err) = agent_org_salvage_legacy_expert_model_key(app_state.inner()) {
                runtime_log_warn(format!("[启动] 抢救旧专家模型配置键失败: {err}"));
            }
            initialize_window_layout_store(&app_handle);
            attach_window_layout_persistence(&app_handle);
            hide_on_close(&app_handle);
            let startup_window_label = match state_read_config_cached(app_state.inner()) {
                Ok(mut config) => {
                    normalize_app_config(&mut config);
                    startup_window_label_for_config(&config)
                }
                Err(err) => {
                    runtime_log_error(format!("[启动] 读取启动窗口配置失败: {err}"));
                    "main"
                }
            };
            if let Err(err) = show_window(&app_handle, startup_window_label) {
                runtime_log_error(format!("[启动] 显示启动窗口失败: target={startup_window_label}, error={err}"));
                if let Some(window) = app_handle.get_webview_window(startup_window_label) {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            // 消息存储迁移：版本不足时先标记等待（前端轮询可见），再后台自动开跑
            let migration_state = app_handle.state::<AppState>().inner().clone();
            match prepare_message_store_migration_runtime(&migration_state) {
                Ok(true) => {
                    runtime_log_info(format!("[启动] 检测到消息存储待迁移，后台自动开始"));
                    spawn_message_store_migration_task(migration_state);
                }
                Ok(false) => {}
                Err(err) => {
                    runtime_log_error(format!("[启动] 消息存储迁移版本预判失败：{err}"));
                    message_store_migration_runtime_mark_failed(err);
                }
            }
            app_handle
                .state::<AppState>()
                .backend_ready
                .store(true, std::sync::atomic::Ordering::Release);
            runtime_log_info(format!("[启动] 后端已就绪（阶段 1 完成）"));

            start_github_auto_update_worker(app_handle.clone());

            // ========== 阶段 2：异步完成剩余初始化 ==========
            let deferred_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                run_deferred_setup(deferred_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            show_main_window,
            show_chat_window,
            show_archives_window,
            open_runtime_logs_window,
            hide_current_window,
            toggle_current_window_maximize,
            start_current_window_drag,
            set_chat_window_side_expanded,
            set_chat_window_active,
            get_github_update_state,
            check_github_update,
            start_github_update,
            cancel_github_update,
            apply_prepared_github_update,
            get_portable_pending_manual_replace,
            dismiss_portable_pending_manual_replace,
            retry_portable_pending_manual_replace,
            open_portable_pending_dir,
            fetch_project_changelog_markdown,
            get_app_version,
            get_app_runtime_info,
            get_project_repository_url,
            set_github_update_method,
            set_skipped_github_update_version,
            set_ui_language,
            load_config,
            generate_image,
            check_message_store_migration,
            run_message_store_migration,
            get_message_store_migration_runtime_status,
            confirm_message_store_migration_summary,
            get_storage_usage_overview,
            refresh_storage_usage_overview,
            get_usage_overview,
            refresh_usage_overview,
            get_usage_trail,
            open_storage_usage_item_directory,
            cleanup_storage_legacy_items,
            load_app_bootstrap_snapshot,
            is_backend_ready,
            list_system_fonts,
            list_genai_chat_adapters,
            update_record_hotkey,
            update_record_background_wake,
            save_config,
            load_agents,
            save_agents,
            convert_private_agent_to_main,
            get_agent_private_memory_count,
            set_agent_private_memory_enabled,
            set_agent_memory_recall_mode,
            export_agent_private_memories,
            disable_agent_private_memory,
            import_agent_memories,
            load_chat_settings,
            save_chat_settings,
            patch_chat_settings,
            save_agent_avatar,
            clear_agent_avatar,
            read_avatar_data_url,
            read_chat_image_data_url,
            sync_tray_icon,
            save_conversation_api_settings,
            patch_conversation_api_settings,
            set_agent_primary_api_config,
            get_chat_snapshot,
            list_unarchived_conversations,
            list_unarchived_conversations_changed_since,
            get_conversation_section_orders,
            set_active_unarchived_conversation,
            switch_active_conversation_snapshot,
            get_foreground_conversation_light_snapshot,
            get_foreground_conversation_freshness_snapshot,
            list_transport_conversations,
            list_conversation_create_options,
            get_conversation_workspace_permission,
            select_conversation_workspace_permission,
            save_conversation_workspace_layout,
            list_conversation_workspaces,
            mark_conversation_read,
            set_conversation_plan_mode,
            set_conversation_preferred_model,
            set_conversation_auto_push_remote_contact,
            create_unarchived_conversation,
            open_draft_conversation,
            update_draft_conversation,
            clear_chat_error,
            create_side_chat_conversation,
            create_conversation_branch_from_message,
            branch_unarchived_conversation_from_current,
            export_conversation_share_json,
            import_conversation_share_from_file,
            branch_unarchived_conversation_from_selection,
            forward_unarchived_conversation_selection,
            forward_selection_to_remote_im_contact,
            rename_unarchived_conversation,
            rebind_unarchived_conversation_recipient,
            toggle_unarchived_conversation_pin,
            save_conversation_section_order,
            get_unarchived_conversation_messages,
            get_unarchived_conversation_block_page,
            get_unarchived_conversation_recent_block_messages,
            get_unarchived_conversation_recent_messages,
            get_unarchived_conversation_message_by_id,
            list_delegate_conversations,
            list_conversation_delegate_statuses,
            list_conversation_background_shell_tasks,
            terminate_conversation_background_shell_task,
            abort_delegate_conversation,
            get_delegate_conversation_messages,
            get_delegate_conversation_block_page,
            delete_delegate_conversation,
            delete_unarchived_conversation,
            get_active_conversation_messages,
            get_active_conversation_messages_before,
            get_active_conversation_messages_after,
            get_active_conversation_compaction_segment_before,
            list_tool_review_batches,
            list_tool_review_reports,
            delete_tool_review_report,
            list_tool_review_commit_options,
            get_tool_review_item_detail,
            get_tool_review_batch_details,
            run_tool_review_for_call,
            set_tool_review_item_user_decision,
            run_tool_review_for_batch,
            submit_tool_review_code,
            request_conversation_messages_after_async,
            preview_rewind_conversation_from_message,
            rewind_conversation_from_message,
            get_prompt_preview,
            get_system_prompt_preview,
            list_archives,
            list_memories,
            delete_memory,
            export_memories,
            preview_export_memories,
            export_memories_to_file,
            export_memories_to_path,
            import_memories,
            preview_import_angel_memories,
            import_angel_memories,
            write_utf8_text_file_to_path,
            write_base64_file_to_path,
            search_memories_mixed,
            search_memories_recall,
            search_chat_history_slices,
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
            get_archive_block_page,
            delete_archive,
            unarchive_archive,
            export_archive_to_file,
            import_archives_from_json,
            open_external_url,
            open_workspace_file,
            read_plan_file_content,
            confirm_plan_and_continue,
            submit_chat_message,
            send_chat_message,
            send_user_mention_message,
            submit_user_async_delegate,
            bind_active_chat_view_stream,
            unbind_active_chat_view_stream,
            clear_window_chat_view_stream_bindings_command,
            probe_active_chat_view_stream,
             stop_chat_message,
             get_chat_queue_snapshot,
             get_conversation_fast_request_turns,
             recall_chat_queue_event,
             mark_chat_queue_event_guided,
             interrupt_conversation_runtime,
             get_main_session_state_snapshot,
             get_conversation_runtime_snapshot,
            read_local_binary_file,
            queue_local_file_attachment,
            queue_inline_file_attachment,
            attachment_transfer_begin,
            attachment_transfer_chunk,
            attachment_transfer_complete,
            attachment_transfer_abort,
            attachment_ingest_local_path,
            read_local_chat_image_thumbnail,
            read_local_chat_image_original,
            copy_local_chat_image_to_clipboard,
            save_local_chat_image_as,
            stt_transcribe,
            archive_conversation,
            batch_archive_conversations,
            compact_conversation,
            refresh_models,
            codex_get_builtin_models,
            catalog_list_sources,
            catalog_search,
            catalog_install,
            commands::skill_set_enabled,
            commands::skill_remove,
            quick_genai_chat,
            test_embedding_connection,
            test_rerank_connection,
            test_voice_connection,
            resolve_model_adapter_kind,
            fetch_model_metadata,
            export_config_migration_package,
            preview_import_config_migration_package,
            apply_import_config_migration_package,
            codex_get_auth_status,
            codex_get_rate_limits,
            codex_consume_rate_limit_reset_credit,
            codex_start_oauth_login,
            codex_logout,
            check_tools_status,
            list_tool_catalog,
            list_permission_catalog,
            get_image_text_cache_stats,
            clear_image_text_cache,
            list_recent_llm_round_logs,
            get_recent_llm_round_log_section,
            clear_recent_llm_round_logs,
            list_schedule_runs,
            list_recent_runtime_logs,
            list_runtime_logs_since,
            clear_recent_runtime_logs,
            append_runtime_log_probe,
            dump_memory_cache_stats,
            remote_im_list_channels,
            remote_im_list_contacts,
            remote_im_subscribe_contact_dashboard,
            remote_im_sync_contact_dashboard,
            remote_im_unsubscribe_contact_dashboard,
            remote_im_get_default_group_response_guidance,
            remote_im_update_contact_allow_send,
            remote_im_update_contact_allow_send_files,
            remote_im_update_contact_allow_receive,
            remote_im_update_contact_blocked_message_prefixes,
            remote_im_update_contact_behavior,
            remote_im_reconfigure_channel_behavior,
            remote_im_patch_contact_settings,
            remote_im_update_contact_activation,
            remote_im_update_contact_remark,
            remote_im_update_contact_route_mode,
            remote_im_update_contact_agent_binding,
            remote_im_update_contact_processing_mode,
            remote_im_update_contact_workspace,
            remote_im_delete_contact,
            remote_im_clear_contact_conversation,
            remote_im_enqueue_message,
            remote_im_list_contact_conversations,
            remote_im_get_contact_conversation_messages,
            remote_im_get_contact_conversation_block_page,
            remote_im_get_channel_status,
            remote_im_get_channel_logs,
            remote_im_get_contact_logs,
            remote_im_restart_channel,
            frontend_ready_start_remote_im_services,
            remote_im_weixin_oc_start_login,
            remote_im_weixin_oc_get_login_status,
            remote_im_weixin_oc_sync_contacts,
            remote_im_weixin_oc_logout,
            desktop_screenshot,
            xcap,
            demo_send_native_notification,
            demo_restart_app,
            get_host_runtime_prerequisites,
            install_host_runtime_prerequisite,
            terminal_self_check,
            list_terminal_shell_candidates,
            open_chat_shell_workspace_dir,
            reset_chat_shell_workspace,
            migrate_shell_workspace_directory,
            get_default_chat_shell_workspace_path,
            get_chat_shell_workspace,
            check_git_workspace_root,
            update_chat_shell_workspace_layout,
            upsert_ide_context_snapshot,
            query_ide_context_references,
            get_web_access_info,
            task_list_tasks,
            task_get_task,
            task_optimize_draft,
            task_create_task,
            task_dispatch_task_now,
            task_update_task,
            task_complete_task,
            task_delete_task,
            task_list_run_logs,
            goal_get_current,
            goal_create_goal,
            goal_cancel_goal,
            resolve_terminal_approval,
            approve_terminal_approval_for_session,
            approve_terminal_approval_for_workspace,
            list_pending_terminal_approvals,
            open_file_reader_window_command,
            read_file_reader_file,
            read_file_reader_file_block,
            list_file_reader_directory,
            list_file_reader_directory_open_targets,
            update_file_reader_watch_targets,
            open_file_reader_directory_target,
            open_file_reader_directory_shell,
            open_local_file_directory,
            open_file_with_default_program,
            open_file_in_vscode,
            save_local_file_as,
            git_panel_repos,
            git_panel_detect,
            git_panel_watch_start,
            git_panel_watch_stop,
            git_panel_discover,
            git_panel_status,
            git_panel_diff,
            git_panel_stage,
            git_panel_unstage,
            git_panel_commit,
            git_panel_discard,
            git_panel_stash_list,
            git_panel_stash_create,
            git_panel_stash_apply,
            git_panel_stash_pop,
            git_panel_stash_drop,
            git_panel_stash_files,
            git_panel_branch_list,
            git_panel_branch_create,
            git_panel_branch_delete,
            git_panel_checkout,
            git_panel_checkout_check,
            git_panel_reset_soft,
            git_panel_remote_list,
            git_panel_fetch,
            git_panel_pull,
            git_panel_push,
            git_panel_sync,
            git_panel_log,
            git_panel_show,
            git_panel_commit_files,
            mcp_list_servers,
            mcp_validate_definition,
            mcp_save_server,
            mcp_fix_definition,
            mcp_remove_server,
            mcp_deploy_server,
            mcp_undeploy_server,
            mcp_list_server_tools,
            mcp_list_server_tools_cached,
            mcp_set_tool_enabled,
            commands::mcp_refresh_mcp_and_skills,
            commands::mcp_list_skills,
            mcp_open_workspace_dir,
            commands::skill_open_workspace_dir,
            commands::skill_open_item_dir,
            commands::mcp_save_skill,
            commands::mcp_read_skill_file
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|err| {
            runtime_log_error(format!("[启动] 运行 Tauri 应用失败: {err}"));
        });
}

#[cfg(test)]
mod tests {
    include!("features/tests.rs");
}
