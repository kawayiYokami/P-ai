fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
    if label == "chat" {
        let _ = archive_selected_conversation_if_idle(app);
    }

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

fn archive_selected_conversation_if_idle(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;

    let app_config = read_config(&state.config_path)?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);

    let api_config = resolve_selected_api_config(&app_config, None)
        .ok_or_else(|| "No API config available".to_string())?;
    let selected_agent_id = data.selected_agent_id.clone();
    let changed = archive_if_idle(&mut data, &api_config.id, &selected_agent_id);
    if changed {
        write_app_data(&state.data_path, &data)?;
    }

    drop(guard);
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

fn register_default_hotkey(app: &AppHandle) -> Result<(), String> {
    let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Backquote);
    app.global_shortcut()
        .register(shortcut)
        .map_err(|err| format!("Register hotkey failed: {err}"))
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
