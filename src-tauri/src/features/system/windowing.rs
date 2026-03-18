use std::str::FromStr;

const MAIN_TRAY_ID: &str = "easy-call-tray";
const WINDOW_LAYOUTS_FILE_NAME: &str = "window_layouts.json";

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
        "chat" => (618_u32, 1000_u32),
        "archives" => (900_u32, 900_u32),
        _ => (900_u32, 900_u32),
    }
}

fn minimum_window_size(label: &str) -> (u32, u32) {
    match label {
        "main" => (900_u32, 900_u32),
        "chat" => (560_u32, 760_u32),
        "archives" => (820_u32, 720_u32),
        _ => (820_u32, 720_u32),
    }
}

fn is_fixed_window_size(label: &str) -> bool {
    matches!(label, "main")
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
    let (default_width, default_height) = default_window_size(label);
    let (min_width, min_height) = minimum_window_size(label);
    let max_width = monitor.size().width.max(1);
    let max_height = monitor.size().height.max(1);
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
    let x = monitor.position().x + monitor.size().width as i32 - resolved_width as i32 - margin;
    let y = monitor.position().y + margin;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

fn apply_window_layout_before_show(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let state = app.state::<AppState>();
    let layouts = load_window_layouts(&state.data_path);
    let saved = layouts.windows.get(label);
    let fallback_monitor = preferred_window_monitor(&window);

    if let Some(saved) = saved {
        if let Some(monitor) = fallback_monitor.as_ref() {
            let (resolved_width, resolved_height) =
                resolved_window_size_for_monitor(label, monitor, saved.width, saved.height);
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
                        resolved_width,
                        resolved_height,
                    )
                {
                    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                } else {
                    eprintln!(
                        "[INFO][窗口] 检测到离屏窗口布局，已重置到可见区域: label={}, saved_x={}, saved_y={}, width={}, height={}",
                        label.trim(),
                        x,
                        y,
                        resolved_width,
                        resolved_height
                    );
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
    let outer_pos = window
        .outer_position()
        .map_err(|err| format!("Read window outer position failed: {err}"))?;
    let maximized = window
        .is_maximized()
        .map_err(|err| format!("Read window maximized state failed: {err}"))?;

    upsert_window_layout(app, label, |entry| {
        entry.width = Some(outer_size.width);
        entry.height = Some(outer_size.height);
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

fn sync_tray_icon_from_avatar_path(app: &AppHandle, avatar_path: Option<&str>) -> Result<(), String> {
    let tray = app
        .tray_by_id(MAIN_TRAY_ID)
        .ok_or_else(|| "Tray icon not found".to_string())?;

    let image = avatar_path
        .and_then(|p| {
            let bytes = fs::read(p).ok()?;
            let dyn_img = image::load_from_memory(&bytes).ok()?;
            let resized = dyn_img
                .resize_to_fill(32, 32, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            let (w, h) = resized.dimensions();
            Some(tauri::image::Image::new_owned(resized.into_raw(), w, h))
        })
        .or_else(|| app.default_window_icon().cloned());

    tray
        .set_icon(image)
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

fn build_tray(app: &AppHandle) -> Result<(), String> {
    let config = MenuItem::with_id(app, "config", "配置", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let chat = MenuItem::with_id(app, "chat", "对话", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let archives = MenuItem::with_id(app, "archives", "归档", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|err| format!("Create tray menu item failed: {err}"))?;

    let menu = Menu::with_items(app, &[&config, &chat, &archives, &quit])
        .map_err(|err| format!("Create tray menu failed: {err}"))?;

    let mut tray = TrayIconBuilder::with_id(MAIN_TRAY_ID).menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.tooltip("P-ai")
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "config" {
                let _ = show_window(app, "main");
            } else if id == "chat" {
                let _ = show_window(app, "chat");
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
    for label in ["main", "chat", "archives"] {
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


