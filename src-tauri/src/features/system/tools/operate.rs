fn validate_operate_request(input: &OperateRequest) -> DesktopToolResult<()> {
    if matches!(input.action, OperateAction::PasteText) {
        let content = input.text.as_deref().unwrap_or("").trim();
        if content.is_empty() {
            return Err(DesktopToolError::invalid_params(
                "text is required when action=paste_text",
            ));
        }
    }

    if matches!(
        input.action,
        OperateAction::KeyTap | OperateAction::KeyDown | OperateAction::KeyUp | OperateAction::Hotkey
    ) {
        let keys = input.keyboard.as_ref().map(|v| v.keys.len()).unwrap_or(0);
        if keys == 0 {
            return Err(DesktopToolError::invalid_params(
                "keyboard.keys is required for keyboard actions",
            ));
        }
    }

    if matches!(
        input.action,
        OperateAction::Click
            | OperateAction::DoubleClick
            | OperateAction::MouseDown
            | OperateAction::MouseUp
            | OperateAction::Drag
    ) && input.target.is_none()
    {
        return Err(DesktopToolError::invalid_params(
            "target is required for pointer actions",
        ));
    }

    if matches!(input.action, OperateAction::Drag)
        && input
            .mouse
            .as_ref()
            .and_then(|m| m.drag_to.as_ref())
            .is_none()
    {
        return Err(DesktopToolError::invalid_params(
            "mouse.drag_to is required when action=drag",
        ));
    }

    Ok(())
}

fn ensure_dpi_awareness_once() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let _ = ONCE.get_or_init(|| {
        let _ = enigo::set_dpi_awareness();
    });
}

fn map_mouse_button(kind: MouseButtonKind) -> enigo::Button {
    match kind {
        MouseButtonKind::Left => enigo::Button::Left,
        MouseButtonKind::Right => enigo::Button::Right,
        MouseButtonKind::Middle => enigo::Button::Middle,
    }
}

fn map_input_err(err: enigo::InputError, context: &str) -> DesktopToolError {
    DesktopToolError::internal_error(format!("{context}: {err}"))
}

fn pick_candidate(target: &OperateTarget, require_button: bool) -> Option<&TargetCandidate> {
    let target_text = target.text.as_deref().unwrap_or("").trim();
    let mut candidates = target
        .candidates
        .iter()
        .filter(|c| !require_button || c.is_button.unwrap_or(false))
        .collect::<Vec<_>>();
    if !target_text.is_empty() {
        let lower = target_text.to_lowercase();
        candidates.retain(|c| c.text.to_lowercase().contains(&lower));
    }
    candidates.sort_by(|a, b| {
        b.confidence
            .unwrap_or(0.0)
            .partial_cmp(&a.confidence.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.into_iter().next()
}

fn resolve_target(target: &OperateTarget) -> DesktopToolResult<ResolvedTarget> {
    match target.target_type {
        OperateTargetType::Point => {
            let p = target
                .point
                .as_ref()
                .ok_or_else(|| DesktopToolError::invalid_params("target.point is required"))?;
            Ok(ResolvedTarget {
                x: p.x,
                y: p.y,
                source: "point".to_string(),
            })
        }
        OperateTargetType::Text => {
            if let Some(c) = pick_candidate(target, false) {
                return Ok(ResolvedTarget {
                    x: c.x,
                    y: c.y,
                    source: "text".to_string(),
                });
            }
            if let Some(p) = &target.point {
                return Ok(ResolvedTarget {
                    x: p.x,
                    y: p.y,
                    source: "text_fallback_point".to_string(),
                });
            }
            if target.candidates.len() > 1 {
                return Err(DesktopToolError::ambiguous_target(
                    "text target has multiple candidates and no unique match",
                ));
            }
            Err(DesktopToolError::target_not_found(
                "text target cannot be resolved",
            ))
        }
        OperateTargetType::Button => {
            if let Some(c) = pick_candidate(target, true) {
                return Ok(ResolvedTarget {
                    x: c.x,
                    y: c.y,
                    source: "button".to_string(),
                });
            }
            if let Some(p) = &target.point {
                return Ok(ResolvedTarget {
                    x: p.x,
                    y: p.y,
                    source: "button_fallback_point".to_string(),
                });
            }
            Err(DesktopToolError::target_not_found(
                "button target cannot be resolved",
            ))
        }
    }
}

fn parse_named_key(name: &str) -> Option<enigo::Key> {
    let normalized = name.trim().to_lowercase().replace(['_', ' '], "");
    match normalized.as_str() {
        "ctrl" | "control" => Some(enigo::Key::Control),
        "lctrl" => Some(enigo::Key::LControl),
        "rctrl" => Some(enigo::Key::RControl),
        "shift" => Some(enigo::Key::Shift),
        "lshift" => Some(enigo::Key::LShift),
        "rshift" => Some(enigo::Key::RShift),
        "alt" | "option" => Some(enigo::Key::Alt),
        "meta" | "win" | "windows" | "command" | "cmd" => Some(enigo::Key::Meta),
        "enter" | "return" => Some(enigo::Key::Return),
        "tab" => Some(enigo::Key::Tab),
        "esc" | "escape" => Some(enigo::Key::Escape),
        "space" => Some(enigo::Key::Space),
        "backspace" => Some(enigo::Key::Backspace),
        "delete" | "del" => Some(enigo::Key::Delete),
        "up" | "arrowup" => Some(enigo::Key::UpArrow),
        "down" | "arrowdown" => Some(enigo::Key::DownArrow),
        "left" | "arrowleft" => Some(enigo::Key::LeftArrow),
        "right" | "arrowright" => Some(enigo::Key::RightArrow),
        "home" => Some(enigo::Key::Home),
        "end" => Some(enigo::Key::End),
        "pageup" => Some(enigo::Key::PageUp),
        "pagedown" => Some(enigo::Key::PageDown),
        "f1" => Some(enigo::Key::F1),
        "f2" => Some(enigo::Key::F2),
        "f3" => Some(enigo::Key::F3),
        "f4" => Some(enigo::Key::F4),
        "f5" => Some(enigo::Key::F5),
        "f6" => Some(enigo::Key::F6),
        "f7" => Some(enigo::Key::F7),
        "f8" => Some(enigo::Key::F8),
        "f9" => Some(enigo::Key::F9),
        "f10" => Some(enigo::Key::F10),
        "f11" => Some(enigo::Key::F11),
        "f12" => Some(enigo::Key::F12),
        _ => None,
    }
}

fn parse_key(name: &str) -> DesktopToolResult<enigo::Key> {
    if let Some(k) = parse_named_key(name) {
        return Ok(k);
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(enigo::Key::Unicode(ch)),
        _ => Err(DesktopToolError::invalid_params(format!(
            "unsupported key: {name}"
        ))),
    }
}

fn require_keyboard_keys(input: &OperateRequest) -> DesktopToolResult<Vec<enigo::Key>> {
    let keys = input
        .keyboard
        .as_ref()
        .map(|v| v.keys.clone())
        .unwrap_or_default();
    if keys.is_empty() {
        return Err(DesktopToolError::invalid_params("keyboard.keys is required"));
    }
    keys.iter().map(|k| parse_key(k)).collect()
}

async fn sleep_ms(ms: u64) {
    if ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
    }
}

async fn run_operate_tool(input: OperateRequest) -> DesktopToolResult<OperateResponse> {
    validate_operate_request(&input)?;
    ensure_dpi_awareness_once();
    let started = std::time::Instant::now();

    let mut enigo = enigo::Enigo::new(&enigo::Settings::default())
        .map_err(|err| DesktopToolError::internal_error(format!("create enigo failed: {err}")))?;

    let mouse_cfg = input.mouse.clone().unwrap_or(MouseOptions {
        button: MouseButtonKind::Left,
        hold_ms: None,
        scroll_delta: None,
        drag_to: None,
    });
    let button = map_mouse_button(mouse_cfg.button);
    let hold_ms = mouse_cfg.hold_ms.unwrap_or(0);

    let resolved_target = match input.action {
        OperateAction::Click
        | OperateAction::DoubleClick
        | OperateAction::MouseDown
        | OperateAction::MouseUp
        | OperateAction::Drag => {
            let target = input
                .target
                .as_ref()
                .ok_or_else(|| DesktopToolError::invalid_params("target is required"))?;
            Some(resolve_target(target)?)
        }
        _ => None,
    };

    if let Some(t) = &resolved_target {
        enigo
            .move_mouse(t.x, t.y, enigo::Coordinate::Abs)
            .map_err(|err| map_input_err(err, "move mouse failed"))?;
    }

    match input.action {
        OperateAction::Click => {
            if hold_ms > 0 {
                enigo
                    .button(button, enigo::Direction::Press)
                    .map_err(|err| map_input_err(err, "mouse down failed"))?;
                sleep_ms(hold_ms).await;
                enigo
                    .button(button, enigo::Direction::Release)
                    .map_err(|err| map_input_err(err, "mouse up failed"))?;
            } else {
                enigo
                    .button(button, enigo::Direction::Click)
                    .map_err(|err| map_input_err(err, "mouse click failed"))?;
            }
        }
        OperateAction::DoubleClick => {
            enigo
                .button(button, enigo::Direction::Click)
                .map_err(|err| map_input_err(err, "mouse first click failed"))?;
            sleep_ms(60).await;
            enigo
                .button(button, enigo::Direction::Click)
                .map_err(|err| map_input_err(err, "mouse second click failed"))?;
        }
        OperateAction::MouseDown => {
            enigo
                .button(button, enigo::Direction::Press)
                .map_err(|err| map_input_err(err, "mouse down failed"))?;
        }
        OperateAction::MouseUp => {
            enigo
                .button(button, enigo::Direction::Release)
                .map_err(|err| map_input_err(err, "mouse up failed"))?;
        }
        OperateAction::Scroll => {
            let delta = mouse_cfg.scroll_delta.unwrap_or(120);
            let units = (delta / 120).clamp(-100, 100);
            enigo
                .scroll(units, enigo::Axis::Vertical)
                .map_err(|err| map_input_err(err, "mouse scroll failed"))?;
        }
        OperateAction::Drag => {
            let to = mouse_cfg
                .drag_to
                .as_ref()
                .ok_or_else(|| DesktopToolError::invalid_params("mouse.drag_to is required"))?;
            enigo
                .button(button, enigo::Direction::Press)
                .map_err(|err| map_input_err(err, "mouse down failed"))?;
            sleep_ms(hold_ms.max(30)).await;
            enigo
                .move_mouse(to.x, to.y, enigo::Coordinate::Abs)
                .map_err(|err| map_input_err(err, "drag move failed"))?;
            sleep_ms(30).await;
            enigo
                .button(button, enigo::Direction::Release)
                .map_err(|err| map_input_err(err, "mouse up failed"))?;
        }
        OperateAction::KeyTap => {
            for key in require_keyboard_keys(&input)? {
                enigo
                    .key(key, enigo::Direction::Click)
                    .map_err(|err| map_input_err(err, "key tap failed"))?;
            }
        }
        OperateAction::KeyDown => {
            for key in require_keyboard_keys(&input)? {
                enigo
                    .key(key, enigo::Direction::Press)
                    .map_err(|err| map_input_err(err, "key down failed"))?;
            }
        }
        OperateAction::KeyUp => {
            for key in require_keyboard_keys(&input)? {
                enigo
                    .key(key, enigo::Direction::Release)
                    .map_err(|err| map_input_err(err, "key up failed"))?;
            }
        }
        OperateAction::Hotkey => {
            let keys = require_keyboard_keys(&input)?;
            let hold = input
                .keyboard
                .as_ref()
                .and_then(|v| v.hold_ms)
                .unwrap_or(0);
            for key in &keys {
                enigo
                    .key(*key, enigo::Direction::Press)
                    .map_err(|err| map_input_err(err, "hotkey press failed"))?;
            }
            sleep_ms(hold).await;
            for key in keys.iter().rev() {
                enigo
                    .key(*key, enigo::Direction::Release)
                    .map_err(|err| map_input_err(err, "hotkey release failed"))?;
            }
        }
        OperateAction::PasteText => {
            let content = input.text.clone().unwrap_or_default();
            let mut clipboard = arboard::Clipboard::new().map_err(|err| {
                DesktopToolError::internal_error(format!("open clipboard failed: {err}"))
            })?;
            clipboard.set_text(content.clone()).map_err(|err| {
                DesktopToolError::internal_error(format!("set clipboard text failed: {err}"))
            })?;

            enigo
                .key(enigo::Key::Control, enigo::Direction::Press)
                .map_err(|err| map_input_err(err, "paste key press ctrl failed"))?;
            enigo
                .key(enigo::Key::Unicode('v'), enigo::Direction::Click)
                .map_err(|err| map_input_err(err, "paste key press v failed"))?;
            enigo
                .key(enigo::Key::Control, enigo::Direction::Release)
                .map_err(|err| map_input_err(err, "paste key release ctrl failed"))?;
        }
    }

    let post_wait_ms = input.post_delay_ms.unwrap_or(0);
    sleep_ms(post_wait_ms).await;

    let screenshot = if input.rescreenshot.unwrap_or(false) {
        Some(run_screenshot_tool(ScreenshotRequest {
            mode: ScreenshotMode::Desktop,
            monitor_id: None,
            region: None,
            save_path: None,
        })
        .await?)
    } else {
        None
    };

    Ok(OperateResponse {
        ok: true,
        action: input.action,
        resolved_target,
        elapsed_ms: started.elapsed().as_millis() as u64,
        post_wait_ms,
        screenshot,
    })
}
use enigo::{Keyboard, Mouse};
