use enigo::{Keyboard, Mouse};

fn ensure_dpi_awareness_once() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let _ = ONCE.get_or_init(|| {
        let _ = enigo::set_dpi_awareness();
    });
}

fn map_mouse_button(button: OperateMouseButton) -> enigo::Button {
    match button {
        OperateMouseButton::Left => enigo::Button::Left,
        OperateMouseButton::Right => enigo::Button::Right,
        OperateMouseButton::Middle => enigo::Button::Middle,
        OperateMouseButton::Back => enigo::Button::Back,
        OperateMouseButton::Forward => enigo::Button::Forward,
    }
}

fn map_input_err(err: enigo::InputError, context: &str) -> DesktopToolError {
    DesktopToolError::internal_error(format!("{context}: {err}"))
}

fn parse_named_key(name: &str) -> Option<enigo::Key> {
    let normalized = name.trim().to_lowercase().replace(['_', ' ', '-'], "");
    match normalized.as_str() {
        "ctrl" | "control" => Some(enigo::Key::Control),
        "lctrl" | "leftcontrol" => Some(enigo::Key::LControl),
        "rctrl" | "rightcontrol" => Some(enigo::Key::RControl),
        "shift" => Some(enigo::Key::Shift),
        "lshift" | "leftshift" => Some(enigo::Key::LShift),
        "rshift" | "rightshift" => Some(enigo::Key::RShift),
        "alt" | "option" => Some(enigo::Key::Alt),
        "meta" | "win" | "windows" | "command" | "cmd" => Some(enigo::Key::Meta),
        "enter" | "return" => Some(enigo::Key::Return),
        "tab" => Some(enigo::Key::Tab),
        "esc" | "escape" => Some(enigo::Key::Escape),
        "space" | "spacebar" => Some(enigo::Key::Space),
        "backspace" => Some(enigo::Key::Backspace),
        "delete" | "del" => Some(enigo::Key::Delete),
        "insert" => Some(enigo::Key::Insert),
        "up" | "arrowup" => Some(enigo::Key::UpArrow),
        "down" | "arrowdown" => Some(enigo::Key::DownArrow),
        "left" | "arrowleft" => Some(enigo::Key::LeftArrow),
        "right" | "arrowright" => Some(enigo::Key::RightArrow),
        "home" => Some(enigo::Key::Home),
        "end" => Some(enigo::Key::End),
        "pageup" => Some(enigo::Key::PageUp),
        "pagedown" => Some(enigo::Key::PageDown),
        "capslock" => Some(enigo::Key::CapsLock),
        "printscreen" => Some(enigo::Key::PrintScr),
        "pause" => Some(enigo::Key::Pause),
        "numlock" => Some(enigo::Key::Numlock),
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

fn parse_key(name: &str, line: usize) -> DesktopToolResult<enigo::Key> {
    if let Some(key) = parse_named_key(name) {
        return Ok(key);
    }
    let trimmed = name.trim();
    let mut chars = trimmed.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Ok(enigo::Key::Unicode(ch)),
        _ => Err(operate_line_error(line, "key", format!("非法：不支持的按键 `{name}`"))),
    }
}

fn primary_monitor_bounds() -> DesktopToolResult<ScreenBounds> {
    let monitors = monitor_list()?;
    let monitor = resolve_primary_monitor(&monitors);
    let x = monitor.x().unwrap_or(0);
    let y = monitor.y().unwrap_or(0);
    let width = monitor.width().map_err(|err| DesktopToolError::internal_error(format!("read monitor width failed: {err}")))?;
    let height = monitor.height().map_err(|err| DesktopToolError::internal_error(format!("read monitor height failed: {err}")))?;
    Ok(ScreenBounds { x, y, width, height })
}

fn normalized_point_to_screen(point: &NormalizedPoint, bounds: &ScreenBounds) -> (i32, i32) {
    let max_x = bounds.width.saturating_sub(1) as f64;
    let max_y = bounds.height.saturating_sub(1) as f64;
    (bounds.x + (point.x * max_x).round() as i32, bounds.y + (point.y * max_y).round() as i32)
}

fn normalized_region_to_screen(region: &NormalizedRegion, bounds: &ScreenBounds) -> ScreenBounds {
    let width_f = bounds.width as f64;
    let height_f = bounds.height as f64;
    ScreenBounds {
        x: bounds.x + (region.x * width_f).round() as i32,
        y: bounds.y + (region.y * height_f).round() as i32,
        width: (region.width * width_f).round().max(1.0) as u32,
        height: (region.height * height_f).round().max(1.0) as u32,
    }
}

async fn sleep_duration(duration: std::time::Duration) {
    if !duration.is_zero() {
        tokio::time::sleep(duration).await;
    }
}

async fn execute_mouse_click(enigo: &mut enigo::Enigo, button: OperateMouseButton, target: &NormalizedPoint, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let bounds = primary_monitor_bounds()?;
    let (x, y) = normalized_point_to_screen(target, &bounds);
    enigo.move_mouse(x, y, enigo::Coordinate::Abs).map_err(|err| map_input_err(err, "move mouse failed"))?;
    let mapped = map_mouse_button(button);
    for idx in 0..repeat {
        if press.is_zero() {
            enigo.button(mapped, enigo::Direction::Click).map_err(|err| map_input_err(err, "mouse click failed"))?;
        } else {
            enigo.button(mapped, enigo::Direction::Press).map_err(|err| map_input_err(err, "mouse down failed"))?;
            sleep_duration(press).await;
            enigo.button(mapped, enigo::Direction::Release).map_err(|err| map_input_err(err, "mouse up failed"))?;
        }
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_mouse_scroll(enigo: &mut enigo::Enigo, direction: i32, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    for idx in 0..repeat {
        enigo.scroll(direction, enigo::Axis::Vertical).map_err(|err| map_input_err(err, "mouse scroll failed"))?;
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_key_action(enigo: &mut enigo::Enigo, keys: &[String], line: usize, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    let parsed = keys.iter().map(|key| parse_key(key, line)).collect::<DesktopToolResult<Vec<_>>>()?;
    for idx in 0..repeat {
        if parsed.len() == 1 && press.is_zero() {
            enigo.key(parsed[0], enigo::Direction::Click).map_err(|err| map_input_err(err, "key tap failed"))?;
        } else {
            for key in &parsed {
                enigo.key(*key, enigo::Direction::Press).map_err(|err| map_input_err(err, "key press failed"))?;
            }
            sleep_duration(press).await;
            for key in parsed.iter().rev() {
                enigo.key(*key, enigo::Direction::Release).map_err(|err| map_input_err(err, "key release failed"))?;
            }
        }
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_text_action(enigo: &mut enigo::Enigo, text: &str, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration) -> DesktopToolResult<()> {
    sleep_duration(pre_delay).await;
    for idx in 0..repeat {
        enigo.text(text).map_err(|err| map_input_err(err, "text input failed"))?;
        if idx + 1 < repeat {
            sleep_duration(delay).await;
        }
    }
    Ok(())
}

async fn execute_screenshot_action(mode: &ScreenshotModeSpec, save_path: Option<String>, quality: f32) -> DesktopToolResult<(ScreenshotResponse, String)> {
    let request = ScreenshotRequest {
        mode: match mode {
            ScreenshotModeSpec::Desktop | ScreenshotModeSpec::FocusedWindow => ScreenshotMode::Desktop,
            ScreenshotModeSpec::Region(_) => ScreenshotMode::Region,
        },
        monitor_id: None,
        region: match mode {
            ScreenshotModeSpec::Region(region) => {
                let bounds = primary_monitor_bounds()?;
                Some(normalized_region_to_screen(region, &bounds))
            }
            _ => None,
        },
        save_path,
        webp_quality: quality,
    };
    let result = match mode {
        ScreenshotModeSpec::Desktop | ScreenshotModeSpec::Region(_) => run_screenshot_tool(request).await?,
        ScreenshotModeSpec::FocusedWindow => run_capture_window_tool(request, None)?,
    };
    let mode_name = match mode {
        ScreenshotModeSpec::Desktop => "desktop",
        ScreenshotModeSpec::FocusedWindow => "focused_window",
        ScreenshotModeSpec::Region(_) => "region",
    }
    .to_string();
    Ok((result, mode_name))
}

#[cfg(test)]
mod operate_actions_tests {
    use super::*;

    #[test]
    fn normalized_region_should_include_screen_offsets() {
        let region = NormalizedRegion {
            x: 0.25,
            y: 0.5,
            width: 0.4,
            height: 0.25,
        };
        let bounds = ScreenBounds {
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };

        let screen = normalized_region_to_screen(&region, &bounds);

        assert_eq!(screen.x, 300);
        assert_eq!(screen.y, 500);
        assert_eq!(screen.width, 320);
        assert_eq!(screen.height, 150);
    }
}
