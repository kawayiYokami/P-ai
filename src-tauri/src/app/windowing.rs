use std::str::FromStr;

fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;

    if let Ok(Some(monitor)) = window.current_monitor() {
        if let Ok(window_size) = window.outer_size() {
            let margin = 24_i32;
            let x = monitor.position().x + monitor.size().width as i32
                - window_size.width as i32
                - margin;
            let y = monitor.position().y + margin;
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
    }

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("easy-call:refresh", ());
    Ok(())
}

fn toggle_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| format!("Window '{label}' not found"))?;
    let visible = window
        .is_visible()
        .map_err(|err| format!("Check window visibility failed: {err}"))?;
    if visible {
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
    let shortcut = parse_hotkey(&config.hotkey)?;

    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    manager
        .register(shortcut)
        .map_err(|err| format!("Register hotkey failed: {err}"))
}

fn register_hotkey_from_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let shortcut = parse_hotkey(&config.hotkey)?;
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    manager
        .register(shortcut)
        .map_err(|err| format!("Register hotkey failed: {err}"))
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

    let mut tray = TrayIconBuilder::new().menu(&menu);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.tooltip("Easy Call AI")
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
