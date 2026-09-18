use std::str::FromStr;

const MAIN_TRAY_ID: &str = "easy-call-tray";
const FILE_READER_WINDOW_LABEL: &str = "file-reader";
const NEAR_FULLSCREEN_RESTORE_RATIO: f64 = 0.92;
const WINDOW_LAYOUT_SAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

static DETACHED_CHAT_WINDOWS: OnceLock<Mutex<std::collections::HashMap<String, String>>> =
    OnceLock::new();

static CHAT_WINDOW_SIDE_EXPANSION: OnceLock<Mutex<ChatWindowSideExpansion>> = OnceLock::new();
static WINDOW_LAYOUT_STORE: OnceLock<Arc<Mutex<WindowLayoutStore>>> = OnceLock::new();
static WINDOW_LAYOUT_SAVE_SENDER: OnceLock<std::sync::mpsc::Sender<PersistedWindowLayouts>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ChatWindowSideExpansion {
    left_physical: u32,
    right_physical: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhysicalWindowRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct WindowLayoutStore {
    layouts: PersistedWindowLayouts,
}

fn chat_window_side_expansion() -> &'static Mutex<ChatWindowSideExpansion> {
    CHAT_WINDOW_SIDE_EXPANSION.get_or_init(|| Mutex::new(ChatWindowSideExpansion::default()))
}

fn read_chat_window_side_expansion() -> Result<ChatWindowSideExpansion, String> {
    chat_window_side_expansion()
        .lock()
        .map(|state| *state)
        .map_err(|err| format!("读取聊天窗口侧栏外扩状态失败：{err}"))
}

fn write_chat_window_side_expansion(
    update: impl FnOnce(&mut ChatWindowSideExpansion),
) -> Result<ChatWindowSideExpansion, String> {
    let mut state = chat_window_side_expansion()
        .lock()
        .map_err(|err| format!("更新聊天窗口侧栏外扩状态失败：{err}"))?;
    update(&mut state);
    Ok(*state)
}

fn calculate_chat_window_expand_target(
    window: PhysicalWindowRect,
    screen: PhysicalWindowRect,
    side: &str,
    requested_width: u32,
) -> Option<PhysicalWindowRect> {
    if requested_width == 0 {
        return None;
    }
    if side != "left" && side != "right" {
        return None;
    }
    if window.width.saturating_add(requested_width) > screen.width {
        return None;
    }
    Some(PhysicalWindowRect {
        x: if side == "left" {
            window.x.saturating_sub(requested_width as i32)
        } else {
            window.x
        },
        y: window.y,
        width: window.width.saturating_add(requested_width),
        height: window.height,
    })
}

fn calculate_chat_window_collapse_target(
    window: PhysicalWindowRect,
    side: &str,
    applied_width: u32,
) -> Option<PhysicalWindowRect> {
    if applied_width == 0 || window.width <= applied_width {
        return None;
    }
    Some(PhysicalWindowRect {
        x: if side == "left" {
            window.x.saturating_add(applied_width as i32)
        } else {
            window.x
        },
        y: window.y,
        width: window.width - applied_width,
        height: window.height,
    })
}

#[cfg(test)]
mod chat_window_side_expansion_tests {
    use super::*;

    fn rect(x: i32, width: u32) -> PhysicalWindowRect {
        PhysicalWindowRect {
            x,
            y: 40,
            width,
            height: 900,
        }
    }

    #[test]
    fn expands_left_when_full_width_fits() {
        let target = calculate_chat_window_expand_target(
            rect(500, 600),
            rect(0, 1920),
            "left",
            320,
        );
        assert_eq!(target, Some(rect(180, 920)));
    }

    #[test]
    fn keeps_current_layout_when_expanded_window_would_exceed_screen_width() {
        let target = calculate_chat_window_expand_target(
            rect(100, 1700),
            rect(0, 1920),
            "left",
            320,
        );
        assert_eq!(target, None);
    }

    #[test]
    fn allows_left_position_to_extend_past_screen_edge_when_total_width_fits() {
        let target = calculate_chat_window_expand_target(
            rect(100, 600),
            rect(0, 1920),
            "left",
            320,
        );
        assert_eq!(target, Some(rect(-220, 920)));
    }

    #[test]
    fn expands_right_without_moving_the_left_edge() {
        let target = calculate_chat_window_expand_target(
            rect(500, 600),
            rect(0, 1920),
            "right",
            320,
        );
        assert_eq!(target, Some(rect(500, 920)));
    }

    #[test]
    fn collapses_left_back_to_the_base_rect() {
        let target = calculate_chat_window_collapse_target(rect(180, 920), "left", 320);
        assert_eq!(target, Some(rect(500, 600)));
    }

    #[test]
    fn chat_window_default_size_matches_tauri_config() {
        assert_eq!(default_window_size("chat"), (618, 1000));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PersistedWindowLayouts {
    #[serde(default)]
    windows: std::collections::HashMap<String, PersistedWindowLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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

fn window_layout_store() -> Result<Arc<Mutex<WindowLayoutStore>>, String> {
    WINDOW_LAYOUT_STORE
        .get()
        .cloned()
        .ok_or_else(|| "窗口布局内存缓存尚未初始化".to_string())
}

fn window_layouts_snapshot() -> Result<PersistedWindowLayouts, String> {
    let store = window_layout_store()?;
    store
        .lock()
        .map(|state| state.layouts.clone())
        .map_err(|err| format!("读取窗口布局内存缓存失败：{err}"))
}

fn enqueue_window_layout_save(layouts: PersistedWindowLayouts) {
    let Some(sender) = WINDOW_LAYOUT_SAVE_SENDER.get() else {
        runtime_log_warn("[窗口布局] 保存队列尚未初始化，跳过异步写盘".to_string());
        return;
    };
    if let Err(err) = sender.send(layouts) {
        runtime_log_warn(format!("[窗口布局] 写入异步保存队列失败：{err}"));
    }
}

fn run_window_layout_save_worker(
    state: AppState,
    receiver: std::sync::mpsc::Receiver<PersistedWindowLayouts>,
) {
    let mut pending = match receiver.recv() {
        Ok(layouts) => layouts,
        Err(_) => return,
    };
    let mut next_save_at = std::time::Instant::now() + WINDOW_LAYOUT_SAVE_INTERVAL;
    loop {
        let wait = next_save_at.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(wait) {
            Ok(layouts) => pending = layouts,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Err(err) = state_service_save_window_layouts(&state, &pending) {
                    runtime_log_warn(format!(
                        "[窗口布局] 异步写盘失败，将在下一轮重试：error={err}"
                    ));
                    next_save_at = std::time::Instant::now() + WINDOW_LAYOUT_SAVE_INTERVAL;
                    continue;
                }
                next_save_at = std::time::Instant::now() + WINDOW_LAYOUT_SAVE_INTERVAL;
                match receiver.try_recv() {
                    Ok(layouts) => pending = layouts,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        pending = match receiver.recv() {
                            Ok(layouts) => layouts,
                            Err(_) => return,
                        };
                        next_save_at = std::time::Instant::now() + WINDOW_LAYOUT_SAVE_INTERVAL;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn initialize_window_layout_store(app: &AppHandle) {
    if WINDOW_LAYOUT_STORE.get().is_some() {
        return;
    }
    let state = app.state::<AppState>();
    let (layouts, repair_needed) = match state_service_get_window_layouts(state.inner()) {
        Ok(layouts) => (layouts, false),
        Err(err) => {
            runtime_log_warn(format!(
                "[窗口布局] 读取布局失败，已使用新的内存布局：error={err}"
            ));
            (PersistedWindowLayouts::default(), true)
        }
    };
    let initial_layouts = layouts.clone();
    let (sender, receiver) = std::sync::mpsc::channel::<PersistedWindowLayouts>();
    let store = Arc::new(Mutex::new(WindowLayoutStore {
        layouts,
    }));
    if WINDOW_LAYOUT_STORE.set(store).is_err() {
        return;
    }
    if WINDOW_LAYOUT_SAVE_SENDER.set(sender).is_err() {
        return;
    }
    if repair_needed {
        enqueue_window_layout_save(initial_layouts);
    }
    let worker_state = state.inner().clone();
    std::thread::Builder::new()
        .name("window-layout-save".to_string())
        .spawn(move || run_window_layout_save_worker(worker_state, receiver))
        .ok();
}

fn upsert_window_layout<F>(label: &str, update: F) -> Result<(), String>
where
    F: FnOnce(&mut PersistedWindowLayout),
{
    let store = window_layout_store()?;
    let snapshot = {
        let mut state = store
            .lock()
            .map_err(|err| format!("更新窗口布局内存缓存失败：{err}"))?;
        let entry = state.layouts.windows.entry(label.to_string()).or_default();
        let previous = entry.clone();
        update(entry);
        if *entry == previous {
            return Ok(());
        }
        state.layouts.clone()
    };
    enqueue_window_layout_save(snapshot);
    Ok(())
}

fn default_window_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (900_u32, 900_u32),
        "chat" => (618_u32, 1000_u32),
        "archives" => (900_u32, 900_u32),
        FILE_READER_WINDOW_LABEL => (1040_u32, 760_u32),
        _ => (900_u32, 900_u32),
    }
}

fn minimum_window_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (900_u32, 600_u32),
        "chat" => (520_u32, 520_u32),
        "archives" => (560_u32, 560_u32),
        FILE_READER_WINDOW_LABEL => (720_u32, 520_u32),
        _ => (520_u32, 520_u32),
    }
}

fn restore_window_minimum_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (900_u32, 600_u32),
        _ => minimum_window_size(label),
    }
}

fn detached_chat_windows() -> &'static Mutex<std::collections::HashMap<String, String>> {
    DETACHED_CHAT_WINDOWS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn detached_chat_window_for_conversation(conversation_id: &str) -> Option<String> {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return None;
    }
    let guard = detached_chat_windows().lock().unwrap_or_else(|poison| {
        runtime_log_info(format!(
            "[独立聊天窗口] 会话到窗口映射锁已中毒，继续恢复读取：error={:?}",
            poison
        ));
        poison.into_inner()
    });
    guard.get(cid).cloned()
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

fn focus_file_reader_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(FILE_READER_WINDOW_LABEL)
        .ok_or_else(|| "文件阅读窗口不存在".to_string())?;
    let _ = window.unminimize();
    let _ = window.show();
    ensure_window_visible_after_show(app, FILE_READER_WINDOW_LABEL);
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

fn show_file_reader_window(app: &AppHandle) -> Result<String, String> {
    if app.get_webview_window(FILE_READER_WINDOW_LABEL).is_some() {
        focus_file_reader_window(app)?;
        return Ok(FILE_READER_WINDOW_LABEL.to_string());
    }

    schedule_file_reader_window_creation(app, String::new())?;
    Ok(FILE_READER_WINDOW_LABEL.to_string())
}

fn schedule_file_reader_window_creation(app: &AppHandle, path: String) -> Result<(), String> {
    let app_handle = app.clone();
    std::thread::Builder::new()
        .name("file-reader-window-create".to_string())
        .spawn(move || {
            let started_at = std::time::Instant::now();
            runtime_log_info(format!("[文件阅读窗口] 开始创建窗口：window_label={}", FILE_READER_WINDOW_LABEL));
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
            .title("PAI - 文件阅读")
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
                    runtime_log_error(format!(
                        "[文件阅读窗口] 创建失败：window_label={}，error={}",
                        FILE_READER_WINDOW_LABEL,
                        err
                    ));
                    return;
                }
            };

            if let Err(err) = apply_window_layout_before_show(&app_handle, FILE_READER_WINDOW_LABEL) {
                runtime_log_error(format!(
                    "[文件阅读窗口] 应用窗口布局失败：window_label={}，error={}",
                    FILE_READER_WINDOW_LABEL,
                    err
                ));
            }
            #[cfg(target_os = "windows")]
            webview_health::attach_webview_process_failed_monitor(&window, &app_handle);
            let _ = window.unminimize();
            let _ = window.show();
            ensure_window_visible_after_show(&app_handle, FILE_READER_WINDOW_LABEL);
            let _ = window.set_focus();
            runtime_log_info(format!(
                "[文件阅读窗口] 窗口已显示：window_label={}，elapsed_ms={}",
                FILE_READER_WINDOW_LABEL,
                started_at.elapsed().as_millis()
            ));
        })
        .map(|_| ())
        .map_err(|err| format!("调度创建文件阅读窗口失败：{err}"))
}

fn monitor_logical_size(monitor: &tauri::Monitor) -> tauri::LogicalSize<f64> {
    monitor
        .size()
        .to_logical::<f64>(monitor.scale_factor().max(0.1))
}

fn default_window_size_for_monitor(label: &str, monitor: &tauri::Monitor) -> (u32, u32) {
    let fallback = default_window_size(label);
    if matches!(label, "chat") {
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
    if let Ok(Some(monitor)) = window.primary_monitor() {
        return Some(monitor);
    }
    if let Some(monitor) = window
        .available_monitors()
        .ok()
        .and_then(|mut monitors| monitors.drain(..).next())
    {
        return Some(monitor);
    }
    window.current_monitor().ok().flatten()
}

fn resolved_window_size_for_monitor(
    label: &str,
    monitor: &tauri::Monitor,
    width: Option<u32>,
    height: Option<u32>,
) -> (u32, u32) {
    let (default_width, default_height) = default_window_size_for_monitor(label, monitor);
    let (min_width, min_height) = minimum_window_size(label);
    let (restore_min_width, restore_min_height) = restore_window_minimum_size(label);
    let monitor_logical = monitor_logical_size(monitor);
    let max_width = monitor_logical.width.max(1.0).round() as u32;
    let max_height = monitor_logical.height.max(1.0).round() as u32;
    let target_width = width.unwrap_or(default_width);
    let target_height = height.unwrap_or(default_height);
    (
        target_width
            .max(restore_min_width.min(max_width))
            .max(min_width.min(max_width))
            .min(max_width),
        target_height
            .max(restore_min_height.min(max_height))
            .max(min_height.min(max_height))
            .min(max_height),
    )
}

fn window_size_is_near_fullscreen(width: u32, height: u32, monitor: &tauri::Monitor) -> bool {
    let monitor_logical = monitor_logical_size(monitor);
    if !monitor_logical.width.is_finite() || !monitor_logical.height.is_finite() {
        return false;
    }
    if monitor_logical.width <= 1.0 || monitor_logical.height <= 1.0 {
        return false;
    }
    let width_ratio = width as f64 / monitor_logical.width;
    let height_ratio = height as f64 / monitor_logical.height;
    width_ratio >= NEAR_FULLSCREEN_RESTORE_RATIO && height_ratio >= NEAR_FULLSCREEN_RESTORE_RATIO
}

fn saved_window_layout_is_near_fullscreen(
    label: &str,
    monitor: &tauri::Monitor,
) -> bool {
    let Ok(layouts) = window_layouts_snapshot() else {
        return false;
    };
    let Some(saved) = layouts.windows.get(label) else {
        return false;
    };
    let (Some(width), Some(height)) = (saved.width, saved.height) else {
        return false;
    };
    window_size_is_near_fullscreen(width, height, monitor)
}

fn webview_window_inner_size_logical(
    window: &tauri::WebviewWindow,
) -> Result<(u32, u32), String> {
    let inner_size = window
        .inner_size()
        .map_err(|err| format!("Read window inner size failed: {err}"))?;
    let scale_factor = window
        .scale_factor()
        .map_err(|err| format!("Read window scale factor failed: {err}"))?;
    let inner_size_logical = inner_size.to_logical::<f64>(scale_factor.max(0.1));
    Ok((
        inner_size_logical.width.round().max(1.0) as u32,
        inner_size_logical.height.round().max(1.0) as u32,
    ))
}

fn apply_physical_window_rect(
    window: &tauri::WebviewWindow,
    current: PhysicalWindowRect,
    target: PhysicalWindowRect,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
        };

        let hwnd = window
            .hwnd()
            .map_err(|err| format!("读取聊天窗口句柄失败：{err}"))?;
        let outer_size = window
            .outer_size()
            .map_err(|err| format!("读取聊天窗口外框尺寸失败：{err}"))?;
        let width_delta = target.width as i64 - current.width as i64;
        let height_delta = target.height as i64 - current.height as i64;
        let target_outer_width = (outer_size.width as i64 + width_delta).max(1) as i32;
        let target_outer_height = (outer_size.height as i64 + height_delta).max(1) as i32;
        let ok = unsafe {
            SetWindowPos(
                hwnd.0,
                std::ptr::null_mut(),
                target.x,
                target.y,
                target_outer_width,
                target_outer_height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        };
        if ok == 0 {
            return Err("原子调整聊天窗口位置和尺寸失败".to_string());
        }
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    let position_changed = current.x != target.x || current.y != target.y;
    #[cfg(not(target_os = "windows"))]
    if position_changed {
        window
            .set_position(Position::Physical(PhysicalPosition::new(target.x, target.y)))
            .map_err(|err| format!("调整聊天窗口位置失败：{err}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    if current.width != target.width || current.height != target.height {
        if let Err(err) = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            target.width,
            target.height,
        ))) {
            if position_changed {
                let _ = window.set_position(Position::Physical(PhysicalPosition::new(
                    current.x,
                    current.y,
                )));
            }
            return Err(format!("调整聊天窗口尺寸失败：{err}"));
        }
    }
    #[cfg(not(target_os = "windows"))]
    Ok(())
}

fn current_physical_window_rect(
    window: &tauri::WebviewWindow,
) -> Result<PhysicalWindowRect, String> {
    let position = window
        .outer_position()
        .map_err(|err| format!("读取聊天窗口位置失败：{err}"))?;
    let size = window
        .inner_size()
        .map_err(|err| format!("读取聊天窗口内容尺寸失败：{err}"))?;
    Ok(PhysicalWindowRect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

fn current_monitor_bounds(window: &tauri::WebviewWindow) -> Result<PhysicalWindowRect, String> {
    let monitor = window
        .current_monitor()
        .map_err(|err| format!("读取聊天窗口所在显示器失败：{err}"))?
        .ok_or_else(|| "未找到聊天窗口所在显示器".to_string())?;
    Ok(PhysicalWindowRect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    })
}

#[tauri::command]
fn set_chat_window_side_expanded(
    app: AppHandle,
    window: tauri::Window,
    side: String,
    expanded: bool,
    width_physical: u32,
) -> Result<bool, String> {
    if window.label() != "chat" || (side != "left" && side != "right") {
        return Ok(false);
    }
    let webview = app
        .get_webview_window(window.label())
        .ok_or_else(|| "未找到聊天窗口".to_string())?;
    let maximized = webview
        .is_maximized()
        .map_err(|err| format!("读取聊天窗口最大化状态失败：{err}"))?;
    let previous_state = read_chat_window_side_expansion()?;
    let previous_width = if side == "left" {
        previous_state.left_physical
    } else {
        previous_state.right_physical
    };

    if expanded {
        if maximized || previous_width > 0 {
            return Ok(previous_width > 0);
        }
        let current = current_physical_window_rect(&webview)?;
        let screen_bounds = current_monitor_bounds(&webview)?;
        let requested_width = width_physical.max(1);
        let Some(target) = calculate_chat_window_expand_target(
            current,
            screen_bounds,
            &side,
            requested_width,
        ) else {
            return Ok(false);
        };
        write_chat_window_side_expansion(|state| {
            if side == "left" {
                state.left_physical = requested_width;
            } else {
                state.right_physical = requested_width;
            }
        })?;
        if let Err(err) = apply_physical_window_rect(&webview, current, target) {
            let _ = write_chat_window_side_expansion(|state| *state = previous_state);
            return Err(err);
        }
        persist_window_layout_snapshot_with_reason(&app, "chat", "side_panel_expanded")?;
        return Ok(true);
    }

    if previous_width == 0 {
        return Ok(false);
    }
    write_chat_window_side_expansion(|state| {
        if side == "left" {
            state.left_physical = 0;
        } else {
            state.right_physical = 0;
        }
    })?;
    if maximized {
        return Ok(false);
    }
    let current = current_physical_window_rect(&webview)?;
    let Some(target) = calculate_chat_window_collapse_target(current, &side, previous_width) else {
        let _ = write_chat_window_side_expansion(|state| *state = previous_state);
        return Err("聊天窗口侧栏外扩尺寸无效，无法收回".to_string());
    };
    if let Err(err) = apply_physical_window_rect(&webview, current, target) {
        let _ = write_chat_window_side_expansion(|state| *state = previous_state);
        return Err(err);
    }
    persist_window_layout_snapshot_with_reason(&app, "chat", "side_panel_collapsed")?;
    Ok(true)
}

fn current_window_size_is_near_fullscreen(
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
) -> bool {
    webview_window_inner_size_logical(window)
        .map(|(width, height)| window_size_is_near_fullscreen(width, height, monitor))
        .unwrap_or(false)
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

fn restore_window_to_default_drag_size(
    window: &tauri::WebviewWindow,
    label: &str,
    monitor: &tauri::Monitor,
) -> Result<(), String> {
    let outer_position = window
        .outer_position()
        .map_err(|err| format!("Read window outer position failed: {err}"))?;
    let outer_size = window
        .outer_size()
        .map_err(|err| format!("Read window outer size failed: {err}"))?;
    let cursor_position = window
        .cursor_position()
        .map_err(|err| format!("Read cursor position failed: {err}"))?;
    let (resolved_width, resolved_height) =
        resolved_window_size_for_monitor(label, monitor, None, None);
    let resolved_width_physical = logical_to_physical_px(resolved_width, monitor.scale_factor());
    let cursor_offset_x = (cursor_position.x - outer_position.x as f64)
        .clamp(0.0, outer_size.width.max(1) as f64);
    let cursor_anchor_ratio = if outer_size.width > 0 {
        (cursor_offset_x / outer_size.width as f64).clamp(0.15, 0.85)
    } else {
        0.5
    };
    let cursor_offset_y = (cursor_position.y - outer_position.y as f64).clamp(12.0, 48.0);
    let monitor_left = monitor.position().x;
    let monitor_top = monitor.position().y;
    let monitor_right = monitor_left.saturating_add(monitor.size().width as i32);
    let max_x = monitor_right.saturating_sub(resolved_width_physical);
    let target_x =
        (cursor_position.x.round() as i32) - (resolved_width_physical as f64 * cursor_anchor_ratio).round() as i32;
    let clamped_x = target_x.clamp(monitor_left, max_x.max(monitor_left));
    let target_y = (cursor_position.y.round() as i32) - cursor_offset_y.round() as i32;
    let clamped_y = target_y.max(monitor_top);

    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        resolved_width as f64,
        resolved_height as f64,
    )));
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(
        clamped_x, clamped_y,
    )));
    Ok(())
}

fn ensure_window_visible_after_show(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    if monitors.is_empty() {
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    if window_rect_is_visible_on_any_monitor(
        &monitors,
        position.x,
        position.y,
        size.width,
        size.height,
    ) {
        return;
    }
    let Some(monitor) = preferred_window_monitor(&window) else {
        return;
    };
    position_window_on_monitor(&window, label, &monitor, None, None);
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
    let layouts = window_layouts_snapshot()?;
    let saved = layouts.windows.get(label);
    let fallback_monitor = preferred_window_monitor(&window);

    if let Some(saved) = saved {
        if let Some(monitor) = fallback_monitor.as_ref() {
            let preferred_width = saved.width;
            let preferred_height = saved.height;
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
                if !monitors.is_empty()
                    && window_rect_is_visible_on_any_monitor(
                        &monitors,
                        x,
                        y,
                        resolved_width_physical,
                        resolved_height_physical,
                    )
                {
                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
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
                position_window_on_monitor(
                    &window,
                    label,
                    monitor,
                    Some(resolved_width),
                    Some(resolved_height),
                );
            }
        } else {
            if let (Some(width), Some(height)) = (saved.width, saved.height) {
                let (restore_min_width, restore_min_height) = restore_window_minimum_size(label);
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    width.max(restore_min_width) as f64,
                    height.max(restore_min_height) as f64,
                )));
            } else {
                let (width, height) = default_window_size(label);
                let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
                    width as f64,
                    height as f64,
                )));
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

/// 保存布局前校验可见性：若 x/y 不在任何显示器可见范围内，改写为主屏内兜底坐标，
/// 避免离屏坐标（如副屏拔除后的残留 -32000,-32000）被持久化。
/// 可见性校验与居中均使用待持久化尺寸对应的物理尺寸（width_physical/height_physical），
/// 与 apply_window_layout_before_show 恢复路径的尺寸转换规则保持一致；
/// 不能内部读 window.outer_size()——它包含 expansion，与待持久化的基础布局不一致。
fn fallback_visible_position(
    window: &tauri::WebviewWindow,
    x: i32,
    y: i32,
    width_physical: u32,
    height_physical: u32,
) -> (i32, i32) {
    let Ok(monitors) = window.available_monitors() else {
        return (x, y);
    };
    if monitors.is_empty() {
        return (x, y);
    }
    if window_rect_is_visible_on_any_monitor(
        &monitors,
        x,
        y,
        width_physical,
        height_physical,
    ) {
        return (x, y);
    }
    let Some(monitor) = preferred_window_monitor(window) else {
        return (x, y);
    };
    let monitor_x = monitor.position().x;
    let monitor_y = monitor.position().y;
    let center_x = monitor_x + (monitor.size().width as i32 - width_physical as i32) / 2;
    let center_y = monitor_y + (monitor.size().height as i32 - height_physical as i32) / 2;
    (center_x.max(monitor_x), center_y.max(monitor_y))
}

fn persist_window_layout_snapshot_with_reason(
    app: &AppHandle,
    label: &str,
    _reason: &str,
) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;
    let size_and_position = if maximized {
        None
    } else {
        let (mut width, height) = webview_window_inner_size_logical(&window)?;
        let outer_pos = window
            .outer_position()
            .map_err(|err| format!("Read window outer position failed: {err}"))?;
        let scale_factor = window
            .scale_factor()
            .map_err(|err| format!("Read window scale factor failed: {err}"))?
            .max(0.1);
        let mut x = outer_pos.x;
        if label == "chat" {
            let expansion = read_chat_window_side_expansion()?;
            let expanded_logical_width = (((expansion.left_physical as u64
                + expansion.right_physical as u64) as f64)
                / scale_factor)
                .round()
                .max(0.0) as u32;
            width = width.saturating_sub(expanded_logical_width).max(1);
            x = x.saturating_add(expansion.left_physical as i32);
        }
        // 用待持久化的 width/height（已扣除 expansion）转物理尺寸做可见性校验，
        // 与 apply_window_layout_before_show 恢复路径的尺寸口径一致。
        let width_physical = logical_to_physical_px(width, scale_factor) as u32;
        let height_physical = logical_to_physical_px(height, scale_factor) as u32;
        let (x, y) =
            fallback_visible_position(&window, x, outer_pos.y, width_physical, height_physical);
        Some((width, height, x, y))
    };

    upsert_window_layout(label, |entry| {
        if let Some((width, height, x, y)) = size_and_position {
            entry.width = Some(width);
            entry.height = Some(height);
            entry.x = Some(x);
            entry.y = Some(y);
        }
        entry.maximized = maximized;
    })
}

/// 为单个窗口注册布局持久化监听（大小/位置/关闭/销毁时写快照）
fn attach_window_layout_persistence_for(
    window: &tauri::WebviewWindow,
    app: &AppHandle,
    label: &str,
) {
    let app_handle = app.clone();
    let label = label.to_string();
    let _ = window.on_window_event(move |event| match event {
        tauri::WindowEvent::Resized(_) => {
            if let Err(err) = persist_window_layout_snapshot_with_reason(&app_handle, &label, "resized")
            {
                runtime_log_error(format!(
                    "[窗口] 持久化窗口布局失败: label={}, error={}",
                    label.trim(),
                    err
                ));
            }
        }
        tauri::WindowEvent::Moved(_) => {
            if let Err(err) = persist_window_layout_snapshot_with_reason(&app_handle, &label, "moved")
            {
                runtime_log_error(format!(
                    "[窗口] 持久化窗口布局失败: label={}, error={}",
                    label.trim(),
                    err
                ));
            }
        }
        tauri::WindowEvent::CloseRequested { .. } => {
            if let Err(err) = persist_window_layout_snapshot_with_reason(
                &app_handle,
                &label,
                "close_requested",
            ) {
                runtime_log_error(format!(
                    "[窗口] 持久化窗口布局失败: label={}, error={}",
                    label.trim(),
                    err
                ));
            }
        }
        tauri::WindowEvent::Destroyed => {
            if let Err(err) =
                persist_window_layout_snapshot_with_reason(&app_handle, &label, "destroyed")
            {
                runtime_log_error(format!(
                    "[窗口] 持久化窗口布局失败: label={}, error={}",
                    label.trim(),
                    err
                ));
            }
        }
        _ => {}
    });
}

fn attach_window_layout_persistence(app: &AppHandle) {
    for label in ["main", "chat", "archives"] {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        attach_window_layout_persistence_for(&window, app, label);
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
    let chat_side_expanded = label == "chat"
        && read_chat_window_side_expansion()
            .map(|state| state.left_physical > 0 || state.right_physical > 0)
            .unwrap_or(false);
    if !chat_side_expanded {
        apply_window_layout_before_show(app, label)?;
    }
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;

    let _ = window.unminimize();
    let _ = window.show();
    ensure_window_visible_after_show(app, label);
    let _ = window.set_focus();
    Ok(())
}

/// 重载窗口内的 WebView：渲染进程死亡或持续无响应后的最小恢复动作。
#[cfg(target_os = "windows")]
fn reload_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    window
        .reload()
        .map_err(|err| format!("Reload window '{label}' failed: {err}"))
}

/// 统一窗口重建入口：销毁后按 tauri.conf.json 的声明重建，并重新挂载布局
/// 持久化、关闭语义与 WebView2 进程失败监控。浏览器进程退出后旧 COM 引用
/// 全部失效，只有重建能恢复；重建会丢失页面内状态，因此排在重载之后使用。
/// 运行日志、文件阅读这类动态创建的窗口不在配置中，退化为重载。
#[cfg(target_os = "windows")]
fn rebuild_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == label)
        .cloned()
    else {
        runtime_log_warn(format!(
            "[WebView2] 窗口未在配置中声明，无法重建，退化为重载：window_label={label}"
        ));
        return reload_window(app, label);
    };

    if let Some(existing) = app.get_webview_window(label) {
        existing
            .destroy()
            .map_err(|err| format!("销毁窗口失败：label={label}，error={err}"))?;
    }

    let window = tauri::WebviewWindowBuilder::from_config(app, &config)
        .map_err(|err| format!("构建窗口失败：label={label}，error={err}"))?
        .build()
        .map_err(|err| format!("重建窗口失败：label={label}，error={err}"))?;

    attach_window_layout_persistence_for(&window, app, label);
    install_hide_on_close(&window, app);
    webview_health::attach_webview_process_failed_monitor(&window, app);

    apply_window_layout_before_show(app, label)?;
    let _ = window.unminimize();
    let _ = window.show();
    ensure_window_visible_after_show(app, label);
    let _ = window.set_focus();
    Ok(())
}

fn toggle_window_maximize_with_default_restore(
    app: &AppHandle,
    label: &str,
) -> Result<bool, String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let was_maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;
    if !was_maximized {
        window
            .maximize()
            .map_err(|err| format!("Maximize window failed: {err}"))?;
        let maximized = window
            .is_maximized()
            .map_err(|err| format!("Read window maximized state failed: {err}"))?;
        return Ok(maximized);
    }

    let restore_monitor = preferred_window_monitor(&window);
    let saved_layout_near_fullscreen = restore_monitor
        .as_ref()
        .map(|monitor| saved_window_layout_is_near_fullscreen(label, monitor))
        .unwrap_or(false);
    window
        .unmaximize()
        .map_err(|err| format!("Restore window failed: {err}"))?;
    let restored_near_fullscreen = restore_monitor
        .as_ref()
        .map(|monitor| current_window_size_is_near_fullscreen(&window, monitor))
        .unwrap_or(false);
    if saved_layout_near_fullscreen || restored_near_fullscreen {
        if let Some(monitor) = restore_monitor.as_ref() {
            position_window_on_monitor(&window, label, monitor, None, None);
            let _ = persist_window_layout_snapshot_with_reason(
                app,
                label,
                "restore_near_fullscreen_to_default",
            );
        }
    }
    let maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;
    Ok(maximized)
}

fn start_window_drag_with_default_restore(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let was_maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;
    let restore_monitor = preferred_window_monitor(&window);
    let should_restore_default_size = if was_maximized {
        true
    } else {
        restore_monitor
            .as_ref()
            .map(|monitor| current_window_size_is_near_fullscreen(&window, monitor))
            .unwrap_or(false)
    };

    if should_restore_default_size {
        if was_maximized {
            window
                .unmaximize()
                .map_err(|err| format!("Restore window failed: {err}"))?;
        }
        if let Some(monitor) = restore_monitor.as_ref() {
            restore_window_to_default_drag_size(&window, label, monitor)?;
            let _ =
                persist_window_layout_snapshot_with_reason(app, label, "drag_restore_to_default");
        }
    }

    window
        .start_dragging()
        .map_err(|err| format!("Start dragging window failed: {err}"))
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
            runtime_log_error(format!("[托盘] 读取对话入口配置失败: {err}"));
            "main"
        }
    };
    show_window(app, target)
}

fn run_tray_action(app: &AppHandle, action: &str) -> Result<(), String> {
    match action {
        "config" => show_window(app, "main"),
        "chat" => show_chat_entry_window(app),
        "file-reader" => {
            show_file_reader_window(app)?;
            Ok(())
        }
        "archives" => show_window(app, "archives"),
        "runtime-logs" => show_runtime_logs_window(app),
        other => Err(format!("未知托盘动作：{other}")),
    }
}

fn dispatch_tray_action(app: &AppHandle, source: &'static str, action: &'static str) {
    let app_handle = app.clone();
    let thread_name = format!("tray-action-{action}");
    if let Err(err) = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            runtime_log_info(format!("[托盘] 收到动作：source={}，action={}", source, action));
            match run_tray_action(&app_handle, action) {
                Ok(()) => {
                    runtime_log_info(format!("[托盘] 动作完成：source={}，action={}", source, action));
                }
                Err(err) => {
                    runtime_log_error(format!(
                        "[托盘] 动作失败：source={}，action={}，error={}",
                        source, action, err
                    ));
                }
            }
        })
    {
        runtime_log_error(format!(
            "[托盘] 调度动作失败：source={}，action={}，error={}",
            source, action, err
        ));
    }
}

// ==================== 运行日志窗口 ====================

const RUNTIME_LOGS_WINDOW_LABEL: &str = "runtime-logs";

fn show_runtime_logs_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(RUNTIME_LOGS_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        ensure_window_visible_after_show(app, RUNTIME_LOGS_WINDOW_LABEL);
        let _ = window.set_focus();
        return Ok(());
    }
    let app_handle = app.clone();
    std::thread::Builder::new()
        .name("runtime-logs-window-create".to_string())
        .spawn(move || {
            if app_handle.get_webview_window(RUNTIME_LOGS_WINDOW_LABEL).is_some() {
                return;
            }
            let window = match tauri::WebviewWindowBuilder::new(
                &app_handle,
                RUNTIME_LOGS_WINDOW_LABEL,
                tauri::WebviewUrl::App("runtime-logs.html".into()),
            )
            .title("PAI - 运行日志")
            .inner_size(900.0, 600.0)
            .min_inner_size(600.0, 400.0)
            .resizable(true)
            .decorations(false)
            .shadow(true)
            .visible(false)
            .build()
            {
                Ok(w) => w,
                Err(_) => return,
            };
            let _ = apply_window_layout_before_show(&app_handle, RUNTIME_LOGS_WINDOW_LABEL);
            #[cfg(target_os = "windows")]
            webview_health::attach_webview_process_failed_monitor(&window, &app_handle);
            let _ = window.unminimize();
            let _ = window.show();
            ensure_window_visible_after_show(&app_handle, RUNTIME_LOGS_WINDOW_LABEL);
            let _ = window.set_focus();
            let cloned = window.clone();
            let _ = window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = cloned.hide();
                }
            });
        })
        .map_err(|err| format!("调度创建运行日志窗口失败：{err}"))?;
    Ok(())
}

fn build_tray(app: &AppHandle) -> Result<(), String> {
    let config = MenuItem::with_id(app, "config", "配置", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let chat = MenuItem::with_id(app, "chat", "对话", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let file_reader = MenuItem::with_id(app, "file-reader", "文件浏览器", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let archives = MenuItem::with_id(app, "archives", "归档", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let runtime_logs = MenuItem::with_id(app, "runtime-logs", "运行日志", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;

    let menu = Menu::with_items(app, &[&config, &chat, &file_reader, &archives, &runtime_logs, &quit])
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
                dispatch_tray_action(tray.app_handle(), "left_click", "chat");
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "config" {
                dispatch_tray_action(app, "menu", "config");
            } else if id == "chat" {
                dispatch_tray_action(app, "menu", "chat");
            } else if id == "file-reader" {
                dispatch_tray_action(app, "menu", "file-reader");
            } else if id == "archives" {
                dispatch_tray_action(app, "menu", "archives");
            } else if id == "runtime-logs" {
                dispatch_tray_action(app, "menu", "runtime-logs");
            } else if id == "quit" {
                runtime_log_info(format!("[托盘] 收到动作：source=menu，action=quit"));
                graceful_exit_app(app, 0);
            }
        })
        .build(app)
        .map_err(|err| format!("Build tray failed: {err}"))?;

    Ok(())
}

/// 为单个窗口注册关闭语义（平台相关）：
/// Windows 关闭即隐藏；Linux/macOS 无托盘环境关闭即优雅退出，避免无法恢复的死锁。
fn install_hide_on_close(window: &tauri::WebviewWindow, app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        let cloned = window.clone();
        let _ = window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = cloned.hide();
            }
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Linux/macOS 部分桌面环境不显示托盘，隐藏后可能无法恢复窗口；
        // 关闭任一主窗口直接优雅退出，避免无退出途径的死锁。
        let label = window.label().to_string();
        let app_clone = app.clone();
        let _ = window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                runtime_log_info(format!("[窗口] {label} 关闭请求，非 Windows 平台直接退出应用"));
                graceful_exit_app(&app_clone, 0);
            }
        });
    }
}

fn hide_on_close(app: &AppHandle) {
    for label in ["main", "chat", "archives"] {
        if let Some(window) = app.get_webview_window(label) {
            install_hide_on_close(&window, app);
        }
    }
}
