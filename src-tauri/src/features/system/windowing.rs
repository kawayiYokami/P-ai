use std::str::FromStr;

const MAIN_TRAY_ID: &str = "easy-call-tray";
const WINDOW_LAYOUTS_FILE_NAME: &str = "window_layouts.json";
const DETACHED_CHAT_WINDOW_PREFIX: &str = "chat-detached-";
const FILE_READER_WINDOW_LABEL: &str = "file-reader";

static DETACHED_CHAT_WINDOWS: OnceLock<Mutex<std::collections::HashMap<String, String>>> =
    OnceLock::new();

static OFFSCREEN_LAYOUT_LOGGED_ONCE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedWindowLayouts {
    #[serde(default)]
    windows: std::collections::HashMap<String, PersistedWindowLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedWindowLayout {
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    maximized: bool,
}

fn window_layouts_path(data_path: &PathBuf) -> PathBuf {
    app_layout_state_dir(data_path).join(WINDOW_LAYOUTS_FILE_NAME)
}

fn load_window_layouts(data_path: &PathBuf) -> PersistedWindowLayouts {
    let path = window_layouts_path(data_path);
    if !path.exists() {
        return PersistedWindowLayouts::default();
    }
    read_json_file::<PersistedWindowLayouts>(&path, "window layouts").unwrap_or_default()
}

fn save_window_layouts(data_path: &PathBuf, layouts: &PersistedWindowLayouts) -> Result<(), String> {
    write_json_file_atomic(
        &window_layouts_path(data_path),
        layouts,
        "window layouts",
    )
}

fn upsert_window_layout<F>(app: &AppHandle, label: &str, update: F) -> Result<(), String>
where
    F: FnOnce(&mut PersistedWindowLayout),
{
    let state = app.state::<AppState>();
    let mut layouts = load_window_layouts(&state.data_path);
    let entry = layouts.windows.entry(label.to_string()).or_default();
    update(entry);
    save_window_layouts(&state.data_path, &layouts)
}

fn default_window_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (900_u32, 900_u32),
        "chat" => (900_u32, 900_u32),
        "archives" => (900_u32, 900_u32),
        "quick-setup" => (800_u32, 600_u32),
        FILE_READER_WINDOW_LABEL => (1040_u32, 760_u32),
        _ => (900_u32, 900_u32),
    }
}

fn minimum_window_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (520_u32, 520_u32),
        "chat" => (520_u32, 520_u32),
        "archives" => (560_u32, 560_u32),
        "quick-setup" => (800_u32, 600_u32),
        FILE_READER_WINDOW_LABEL => (720_u32, 520_u32),
        _ => (520_u32, 520_u32),
    }
}

fn is_fixed_window_size(label: &str) -> bool {
    matches!(label, "main" | "quick-setup")
}

fn detached_chat_windows() -> &'static Mutex<std::collections::HashMap<String, String>> {
    DETACHED_CHAT_WINDOWS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn is_detached_chat_window_label(label: &str) -> bool {
    label.trim().starts_with(DETACHED_CHAT_WINDOW_PREFIX)
}

fn detached_chat_window_for_conversation(conversation_id: &str) -> Option<String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return None;
    }
    let guard = detached_chat_windows().lock().unwrap_or_else(|poison| {
        eprintln!(
            "[独立聊天窗口] 会话到窗口映射锁已中毒，继续恢复读取：error={:?}",
            poison
        );
        poison.into_inner()
    });
    guard.get(cid).cloned()
}

fn detached_chat_conversation_for_window(label: &str) -> Option<String> {
    let window_label = label.trim();
    if window_label.is_empty() {
        return None;
    }
    let guard = detached_chat_windows().lock().unwrap_or_else(|poison| {
        eprintln!(
            "[独立聊天窗口] 窗口到会话映射锁已中毒，继续恢复读取：error={:?}",
            poison
        );
        poison.into_inner()
    });
    guard.iter().find_map(|(conversation_id, mapped_label)| {
        if mapped_label == window_label {
            Some(conversation_id.clone())
        } else {
            None
        }
    })
}

fn register_detached_chat_window(conversation_id: &str, label: &str) -> Result<(), String> {
    let cid = conversation_id.trim();
    let window_label = label.trim();
    if cid.is_empty() || window_label.is_empty() {
        return Err("conversationId 和 windowLabel 不能为空".to_string());
    }
    let mut guard = detached_chat_windows()
        .lock()
        .map_err(|err| format!("锁定独立聊天窗口映射失败：{err}"))?;
    guard.insert(cid.to_string(), window_label.to_string());
    Ok(())
}

fn unregister_detached_chat_window_by_label(label: &str) -> Option<String> {
    let window_label = label.trim();
    if window_label.is_empty() {
        return None;
    }
    let mut guard = detached_chat_windows().lock().ok()?;
    let conversation_id = guard
        .iter()
        .find_map(|(conversation_id, mapped_label)| {
            if mapped_label == window_label {
                Some(conversation_id.clone())
            } else {
                None
            }
        })?;
    guard.remove(&conversation_id);
    Some(conversation_id)
}

fn focus_detached_chat_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label.trim())
        .ok_or_else(|| format!("独立聊天窗口不存在：{}", label.trim()))?;
    let _ = window.unminimize();
    let _ = window.show();
    window
        .set_focus()
        .map_err(|err| format!("聚焦独立聊天窗口失败：{err}"))
}

fn open_detached_chat_window(
    app: &AppHandle,
    conversation_id: &str,
    title: Option<&str>,
) -> Result<String, String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }

    if let Some(existing_label) = detached_chat_window_for_conversation(cid) {
        if app.get_webview_window(&existing_label).is_some() {
            focus_detached_chat_window(app, &existing_label)?;
            return Ok(existing_label);
        }
        let _ = unregister_detached_chat_window_by_label(&existing_label);
    }

    let label = format!("{}{}", DETACHED_CHAT_WINDOW_PREFIX, Uuid::new_v4());
    let window_title = title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("π师傅 - {value}"))
        .unwrap_or_else(|| "π师傅 - 独立聊天窗口".to_string());
    register_detached_chat_window(cid, &label)?;
    schedule_detached_chat_window_creation(app, cid.to_string(), label.clone(), window_title)?;
    Ok(label)
}

fn focus_file_reader_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(FILE_READER_WINDOW_LABEL)
        .ok_or_else(|| "文件阅读窗口不存在".to_string())?;
    let _ = window.unminimize();
    let _ = window.show();
    window
        .set_focus()
        .map_err(|err| format!("聚焦文件阅读窗口失败：{err}"))
}

fn emit_file_reader_open_path(app: &AppHandle, path: &str) -> Result<(), String> {
    app.emit_to(
        FILE_READER_WINDOW_LABEL,
        "file-reader-open-path",
        serde_json::json!({ "path": path }),
    )
    .map_err(|err| format!("投递文件阅读请求失败：{err}"))
}

fn open_file_reader_window(app: &AppHandle, path: String) -> Result<String, String> {
    let normalized_path = path.trim().to_string();
    if normalized_path.is_empty() {
        return Err("path 不能为空".to_string());
    }

    if app.get_webview_window(FILE_READER_WINDOW_LABEL).is_some() {
        focus_file_reader_window(app)?;
        emit_file_reader_open_path(app, &normalized_path)?;
        return Ok(FILE_READER_WINDOW_LABEL.to_string());
    }

    schedule_file_reader_window_creation(app, normalized_path)?;
    Ok(FILE_READER_WINDOW_LABEL.to_string())
}

fn schedule_file_reader_window_creation(app: &AppHandle, path: String) -> Result<(), String> {
    let app_handle = app.clone();
    std::thread::Builder::new()
        .name("file-reader-window-create".to_string())
        .spawn(move || {
            let started_at = std::time::Instant::now();
            eprintln!("[文件阅读窗口] 开始创建窗口：window_label={}", FILE_READER_WINDOW_LABEL);
            if app_handle.get_webview_window(FILE_READER_WINDOW_LABEL).is_some() {
                let _ = focus_file_reader_window(&app_handle);
                let _ = emit_file_reader_open_path(&app_handle, &path);
                return;
            }

            let encoded_path = urlencoding::encode(&path);
            let url = format!("file-reader.html?path={encoded_path}");
            let window = match tauri::WebviewWindowBuilder::new(
                &app_handle,
                FILE_READER_WINDOW_LABEL,
                tauri::WebviewUrl::App(url.into()),
            )
            .title("π师傅 - 文件阅读")
            .inner_size(1040.0, 760.0)
            .min_inner_size(720.0, 520.0)
            .resizable(true)
            .decorations(false)
            .shadow(true)
            .visible(false)
            .build()
            {
                Ok(window) => window,
                Err(err) => {
                    eprintln!(
                        "[文件阅读窗口] 创建失败：window_label={}，error={}",
                        FILE_READER_WINDOW_LABEL,
                        err
                    );
                    return;
                }
            };

            if let Err(err) = apply_window_layout_before_show(&app_handle, FILE_READER_WINDOW_LABEL) {
                eprintln!(
                    "[文件阅读窗口] 应用窗口布局失败：window_label={}，error={}",
                    FILE_READER_WINDOW_LABEL,
                    err
                );
            }
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
            eprintln!(
                "[文件阅读窗口] 窗口已显示：window_label={}，elapsed_ms={}",
                FILE_READER_WINDOW_LABEL,
                started_at.elapsed().as_millis()
            );
        })
        .map(|_| ())
        .map_err(|err| format!("调度创建文件阅读窗口失败：{err}"))
}

fn schedule_detached_chat_window_creation(
    app: &AppHandle,
    conversation_id: String,
    label: String,
    window_title: String,
) -> Result<(), String> {
    let app_handle = app.clone();
    let timeout_app_handle = app.clone();
    let timeout_conversation_id = conversation_id.clone();
    let timeout_window_label = label.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(12));
        let still_registered =
            detached_chat_window_for_conversation(&timeout_conversation_id).as_deref()
                == Some(timeout_window_label.as_str());
        if still_registered && timeout_app_handle.get_webview_window(&timeout_window_label).is_none() {
            let _ = unregister_detached_chat_window_by_label(&timeout_window_label);
            let state = timeout_app_handle.state::<AppState>();
            let _ = emit_unarchived_conversation_overview_updated_from_state(&state);
            eprintln!(
                "[独立聊天窗口] 创建超时：conversation_id={}，window_label={}，timeout_ms=12000",
                timeout_conversation_id,
                timeout_window_label
            );
        }
    });

    let window_app_handle = app_handle.clone();
    let window_conversation_id = conversation_id.clone();
    let window_label = label.clone();
    std::thread::Builder::new()
        .name("detached-chat-window-create".to_string())
        .spawn(move || {
        let started_at = std::time::Instant::now();
        eprintln!(
            "[独立聊天窗口] 开始创建窗口：conversation_id={}，window_label={}",
            window_conversation_id,
            window_label
        );
        let app_handle = window_app_handle;
        if app_handle.get_webview_window(&window_label).is_some() {
            let _ = focus_detached_chat_window(&app_handle, &window_label);
            return;
        }
        let url = format!("chat.html?detachedConversationId={window_conversation_id}");
        let window = match tauri::WebviewWindowBuilder::new(
            &app_handle,
            window_label.clone(),
            tauri::WebviewUrl::App(url.into()),
        )
        .title(window_title)
        .inner_size(618.0, 1000.0)
        .min_inner_size(520.0, 520.0)
        .resizable(true)
        .decorations(false)
        .shadow(true)
        .visible(false)
        .build()
        {
            Ok(window) => window,
            Err(err) => {
                let _ = unregister_detached_chat_window_by_label(&window_label);
                let state = app_handle.state::<AppState>();
                let _ = emit_unarchived_conversation_overview_updated_from_state(&state);
                eprintln!(
                    "[独立聊天窗口] 创建失败：conversation_id={}，window_label={}，error={}",
                    window_conversation_id,
                    window_label,
                    err
                );
                return;
            }
        };
        eprintln!(
            "[独立聊天窗口] 窗口对象已创建：conversation_id={}，window_label={}，elapsed_ms={}",
            window_conversation_id,
            window_label,
            started_at.elapsed().as_millis()
        );

        let event_app_handle = app_handle.clone();
        let event_window_label = window_label.clone();
        let _ = window.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
                if let Some(conversation_id) =
                    unregister_detached_chat_window_by_label(&event_window_label)
                {
                    eprintln!(
                        "[独立聊天窗口] 释放会话占用：conversation_id={}，window_label={}",
                        conversation_id,
                        event_window_label
                    );
                    let state = event_app_handle.state::<AppState>();
                    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(&state) {
                        eprintln!("[独立聊天窗口] 刷新会话概览失败：error={}", err);
                    }
                }
            }
            _ => {}
        });

        if let Err(err) = apply_window_layout_before_show(&app_handle, &window_label) {
            eprintln!(
                "[独立聊天窗口] 应用窗口布局失败：window_label={}，error={}",
                window_label,
                err
            );
        }
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        eprintln!(
            "[独立聊天窗口] 窗口已显示：conversation_id={}，window_label={}，elapsed_ms={}",
            window_conversation_id,
            window_label,
            started_at.elapsed().as_millis()
        );
    })
    .map(|_| ())
    .map_err(|err| {
        let _ = unregister_detached_chat_window_by_label(&label);
        format!("调度创建独立聊天窗口失败：{err}")
    })
}

fn monitor_logical_size(monitor: &tauri::Monitor) -> tauri::LogicalSize<f64> {
    monitor
        .size()
        .to_logical::<f64>(monitor.scale_factor().max(0.1))
}

fn default_window_size_for_monitor(label: &str, monitor: &tauri::Monitor) -> (u32, u32) {
    let fallback = default_window_size(label);
    if matches!(label, "quick-setup") {
        return fallback;
    }
    let logical = monitor_logical_size(monitor);
    let min_side = logical.width.min(logical.height);
    if !min_side.is_finite() || min_side <= 1.0 {
        return fallback;
    }
    let target = (min_side * 0.8).round().max(1.0) as u32;
    (target, target)
}

fn logical_to_physical_px(value: u32, scale_factor: f64) -> i32 {
    ((value as f64) * scale_factor.max(0.1)).round() as i32
}

fn preferred_window_monitor(window: &tauri::WebviewWindow) -> Option<tauri::Monitor> {
    if let Ok(Some(monitor)) = window.current_monitor() {
        return Some(monitor);
    }
    if let Ok(Some(monitor)) = window.primary_monitor() {
        return Some(monitor);
    }
    window
        .available_monitors()
        .ok()
        .and_then(|mut monitors| monitors.drain(..).next())
}

fn resolved_window_size_for_monitor(
    label: &str,
    monitor: &tauri::Monitor,
    width: Option<u32>,
    height: Option<u32>,
) -> (u32, u32) {
    let (default_width, default_height) = default_window_size_for_monitor(label, monitor);
    let (min_width, min_height) = minimum_window_size(label);
    let monitor_logical = monitor_logical_size(monitor);
    let max_width = monitor_logical.width.max(1.0).round() as u32;
    let max_height = monitor_logical.height.max(1.0).round() as u32;
    let target_width = if is_fixed_window_size(label) {
        default_width
    } else {
        width.unwrap_or(default_width)
    };
    let target_height = if is_fixed_window_size(label) {
        default_height
    } else {
        height.unwrap_or(default_height)
    };
    (
        target_width
            .max(min_width.min(max_width))
            .min(max_width),
        target_height
            .max(min_height.min(max_height))
            .min(max_height),
    )
}

fn window_rect_is_visible_on_any_monitor(
    monitors: &[tauri::Monitor],
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> bool {
    let right = x.saturating_add(width as i32);
    let bottom = y.saturating_add(height as i32);
    monitors.iter().any(|monitor| {
        let monitor_x = monitor.position().x;
        let monitor_y = monitor.position().y;
        let monitor_right = monitor_x.saturating_add(monitor.size().width as i32);
        let monitor_bottom = monitor_y.saturating_add(monitor.size().height as i32);
        let visible_width = (right.min(monitor_right) - x.max(monitor_x)).max(0);
        let visible_height = (bottom.min(monitor_bottom) - y.max(monitor_y)).max(0);
        visible_width >= 80 && visible_height >= 80
    })
}

fn position_window_on_monitor(
    window: &tauri::WebviewWindow,
    label: &str,
    monitor: &tauri::Monitor,
    width: Option<u32>,
    height: Option<u32>,
) {
    let (resolved_width, resolved_height) =
        resolved_window_size_for_monitor(label, monitor, width, height);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        resolved_width as f64,
        resolved_height as f64,
    )));
    let margin = 24_i32;
    let resolved_width_physical = logical_to_physical_px(resolved_width, monitor.scale_factor());
    let x = monitor.position().x + monitor.size().width as i32 - resolved_width_physical - margin;
    let y = monitor.position().y + margin;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

fn apply_window_layout_before_show(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let (min_width, min_height) = minimum_window_size(label);
    let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(
        min_width as f64,
        min_height as f64,
    ))));
    let state = app.state::<AppState>();
    let layouts = load_window_layouts(&state.data_path);
    let saved = layouts.windows.get(label);
    let fallback_monitor = preferred_window_monitor(&window);

    if matches!(label, "quick-setup") {
        if let Some(monitor) = fallback_monitor.as_ref() {
            position_window_on_monitor(&window, label, monitor, None, None);
        } else {
            let (width, height) = default_window_size(label);
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                width as f64,
                height as f64,
            )));
        }
        return Ok(());
    }

    if let Some(saved) = saved {
        if let Some(monitor) = fallback_monitor.as_ref() {
            let use_default_size_on_startup = matches!(label, "chat" | "archives");
            let preferred_width = if use_default_size_on_startup {
                None
            } else {
                saved.width
            };
            let preferred_height = if use_default_size_on_startup {
                None
            } else {
                saved.height
            };
            let (resolved_width, resolved_height) =
                resolved_window_size_for_monitor(label, monitor, preferred_width, preferred_height);
            let resolved_width_physical =
                logical_to_physical_px(resolved_width, monitor.scale_factor()) as u32;
            let resolved_height_physical =
                logical_to_physical_px(resolved_height, monitor.scale_factor()) as u32;
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                resolved_width as f64,
                resolved_height as f64,
            )));
            if let (Some(x), Some(y)) = (saved.x, saved.y) {
                let monitors = window.available_monitors().unwrap_or_default();
                if monitors.is_empty()
                    || window_rect_is_visible_on_any_monitor(
                        &monitors,
                        x,
                        y,
                        resolved_width_physical,
                        resolved_height_physical,
                    )
                {
                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                } else {
                    if !OFFSCREEN_LAYOUT_LOGGED_ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        eprintln!(
                            "[窗口] 检测到离屏窗口布局，已重置到可见区域: label={}, saved_x={}, saved_y={}, width={}, height={}",
                            label.trim(),
                            x,
                            y,
                            resolved_width,
                            resolved_height
                        );
                    }
                    position_window_on_monitor(
                        &window,
                        label,
                        monitor,
                        Some(resolved_width),
                        Some(resolved_height),
                    );
                }
            } else {
                position_window_on_monitor(
                    &window,
                    label,
                    monitor,
                    Some(resolved_width),
                    Some(resolved_height),
                );
            }
        } else {
            if !matches!(label, "chat" | "archives") {
                if let (Some(width), Some(height)) = (saved.width, saved.height) {
                    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                        width as f64,
                        height as f64,
                    )));
                }
            } else {
                let (width, height) = default_window_size(label);
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    width as f64,
                    height as f64,
                )));
            }
            if let (Some(x), Some(y)) = (saved.x, saved.y) {
                let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            }
        }
        if saved.maximized {
            let _ = window.maximize();
        }
        return Ok(());
    }

    if let Some(monitor) = fallback_monitor.as_ref() {
        position_window_on_monitor(&window, label, monitor, None, None);
    }
    Ok(())
}

fn persist_window_layout_snapshot(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let outer_size = window
        .outer_size()
        .map_err(|err| format!("Read window outer size failed: {err}"))?;
    let scale_factor = window
        .scale_factor()
        .map_err(|err| format!("Read window scale factor failed: {err}"))?;
    let outer_size_logical = outer_size.to_logical::<f64>(scale_factor.max(0.1));
    let outer_pos = window
        .outer_position()
        .map_err(|err| format!("Read window outer position failed: {err}"))?;
    let maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;

    upsert_window_layout(app, label, |entry| {
        entry.width = Some(outer_size_logical.width.round().max(1.0) as u32);
        entry.height = Some(outer_size_logical.height.round().max(1.0) as u32);
        entry.x = Some(outer_pos.x);
        entry.y = Some(outer_pos.y);
        entry.maximized = maximized;
    })
}

fn attach_window_layout_persistence(app: &AppHandle) {
    for label in ["main", "chat", "archives"] {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        let app_handle = app.clone();
        let label = label.to_string();
        let _ = window.on_window_event(move |event| match event {
            tauri::WindowEvent::Resized(_)
            | tauri::WindowEvent::Moved(_)
            | tauri::WindowEvent::CloseRequested { .. }
            | tauri::WindowEvent::Destroyed => {
                if let Err(err) = persist_window_layout_snapshot(&app_handle, &label) {
                    eprintln!(
                        "[窗口] 持久化窗口布局失败: label={}, error={}",
                        label.trim(),
                        err
                    );
                }
            }
            _ => {}
        });
    }
}

fn sync_default_tray_icon(app: &AppHandle) -> Result<(), String> {
    let tray = app
        .tray_by_id(MAIN_TRAY_ID)
        .ok_or_else(|| "Tray icon not found".to_string())?;

    tray
        .set_icon(app.default_window_icon().cloned())
        .map_err(|err| format!("Set tray icon failed: {err}"))
}

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    apply_window_layout_before_show(app, label)?;
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

fn toggle_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let visible = window
        .is_visible()
        .map_err(|err| format!("Check window visibility failed: {err}"))?;
    let focused = window
        .is_focused()
        .map_err(|err| format!("Check window focus failed: {err}"))?;
    if visible && focused {
        window
            .hide()
            .map_err(|err| format!("Hide window failed: {err}"))?;
        return Ok(());
    }
    show_window(app, label)
}

fn normalize_hotkey_for_parser(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.is_empty() {
        return "Alt+Backquote".to_string();
    }
    text = text.replace('·', "`");
    text = text.replace('＋', "+");
    text
}

fn parse_hotkey(raw: &str) -> Result<Shortcut, String> {
    let normalized = normalize_hotkey_for_parser(raw);
    Shortcut::from_str(&normalized)
        .or_else(|_| Shortcut::from_str("Alt+Backquote"))
        .map_err(|err| format!("Parse hotkey failed: {err}"))
}

fn register_default_hotkey(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = read_config(&state.config_path).unwrap_or_default();
    register_hotkeys_from_config(app, &config)
}

fn register_hotkey_from_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    register_hotkeys_from_config(app, config)
}

fn register_hotkeys_from_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let summon_shortcut = parse_hotkey(&config.hotkey)?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|err| format!("Unregister hotkeys failed: {err}"))?;
    manager
        .register(summon_shortcut)
        .map_err(|err| format!("Register summon hotkey failed: {err}"))
}

fn default_hotkey_label() -> String {
    "Alt+·".to_string()
}

fn normalize_hotkey_label(value: &str) -> String {
    let raw = value.trim();
    if raw.is_empty() {
        return default_hotkey_label();
    }
    let normalized = raw.replace('＋', "+").replace('`', "·");
    let upper = normalized.to_uppercase();
    if upper.contains("BACKQUOTE") {
        return normalized
            .replace("Backquote", "·")
            .replace("BACKQUOTE", "·")
            .replace("backquote", "·");
    }
    normalized
}

fn ensure_hotkey_config_normalized(config: &mut AppConfig) {
    config.hotkey = normalize_hotkey_label(&config.hotkey);
    if config.hotkey.trim().is_empty() {
        config.hotkey = default_hotkey_label();
    }
}

fn show_chat_entry_window(app: &AppHandle) -> Result<(), String> {
    let target = match state_read_config_cached(app.state::<AppState>().inner()) {
        Ok(mut config) => {
            normalize_app_config(&mut config);
            startup_window_label_for_config(&config)
        }
        Err(err) => {
            eprintln!("[托盘] 读取对话入口配置失败: {err}");
            "quick-setup"
        }
    };
    show_window(app, target)
}

fn build_tray(app: &AppHandle) -> Result<(), String> {
    let config = MenuItem::with_id(app, "config", "配置", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let chat = MenuItem::with_id(app, "chat", "对话", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let quick_setup = MenuItem::with_id(app, "quick-setup", "快速设置", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let archives = MenuItem::with_id(app, "archives", "归档", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;

    let menu = Menu::with_items(app, &[&config, &chat, &quick_setup, &archives, &quit])
        .map_err(|err| format!("Create tray menu failed: {err}"))?;

    let mut tray = TrayIconBuilder::with_id(MAIN_TRAY_ID).menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.tooltip("P-ai")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_chat_entry_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "config" {
                let _ = show_window(app, "main");
            } else if id == "chat" {
                let _ = show_chat_entry_window(app);
            } else if id == "quick-setup" {
                let _ = show_window(app, "quick-setup");
            } else if id == "archives" {
                let _ = show_window(app, "archives");
            } else if id == "quit" {
                app.exit(0);
            }
        })
        .build(app)
        .map_err(|err| format!("Build tray failed: {err}"))?;

    Ok(())
}

fn hide_on_close(app: &AppHandle) {
    for label in ["main", "chat", "archives", "quick-setup"] {
        if let Some(window) = app.get_webview_window(label) {
            let cloned = window.clone();
            let _ = window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = cloned.hide();
                }
            });
        }
    }
}
