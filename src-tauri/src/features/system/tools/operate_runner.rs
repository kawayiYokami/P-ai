async fn run_operate_tool(input: OperateRequest) -> DesktopToolResult<OperateResponse> {
    let started = std::time::Instant::now();
    ensure_dpi_awareness_once();
    let actions = parse_script(&input)?;
    let total_actions = actions.len();
    runtime_log_info(format!(
        "[桌面脚本] 开始，任务=run_operate_tool，total_actions={}，timestamp={}",
        total_actions,
        now_iso()
    ));
    let mut enigo = enigo::Enigo::new(&enigo::Settings::default())
        .map_err(|err| DesktopToolError::internal_error(format!("创建 Enigo 失败：{err}")))?;
    let mut steps = Vec::<DesktopScriptStepResult>::new();
    let mut latest_screenshot: Option<LatestScreenshotInfo> = None;
    let mut image_mime = None;
    let mut image_base64 = None;
    let mut width = None;
    let mut height = None;

    for action in actions {
        match action {
            DesktopScriptAction::MouseClick { line, button, target, repeat, delay, pre_delay, press } => {
                execute_mouse_click(&mut enigo, button, &target, repeat, delay, pre_delay, press).await?;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse click completed, repeat={repeat}"),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseClick，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::MouseScroll { line, direction, repeat, delay, pre_delay } => {
                execute_mouse_scroll(&mut enigo, direction, repeat, delay, pre_delay).await?;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Mouse,
                    summary: format!("mouse scroll completed, repeat={repeat}"),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=MouseScroll，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Key { line, keys, repeat, delay, pre_delay, press } => {
                execute_key_action(&mut enigo, &keys, line, repeat, delay, pre_delay, press).await?;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Key,
                    summary: format!("key action completed, combo={}, repeat={repeat}", keys.join("+")),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Key，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Text { line, text, repeat, delay, pre_delay } => {
                execute_text_action(&mut enigo, &text, repeat, delay, pre_delay).await?;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Text,
                    summary: format!("text input completed, chars={}, repeat={repeat}", text.chars().count()),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Text，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Wait { line, duration } => {
                sleep_duration(duration).await;
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Wait,
                    summary: format!("wait completed, seconds={:.3}", duration.as_secs_f64()),
                    ok: true,
                    saved_path: None,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Wait，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
            DesktopScriptAction::Screenshot { line, mode, save_path, quality } => {
                let (result, mode_name) = execute_screenshot_action(&mode, save_path, quality).await?;
                latest_screenshot = Some(LatestScreenshotInfo {
                    mode: mode_name.clone(),
                    width: result.width,
                    height: result.height,
                    saved_path: result.path.clone(),
                });
                image_mime = Some(result.image_mime.clone());
                image_base64 = Some(result.image_base64.clone());
                width = Some(result.width);
                height = Some(result.height);
                let step = DesktopScriptStepResult {
                    line,
                    kind: DesktopScriptStepKind::Screenshot,
                    summary: format!("screenshot completed, mode={mode_name}"),
                    ok: true,
                    saved_path: result.path,
                };
                runtime_log_info(format!(
                    "[桌面脚本] 步骤完成，任务=run_operate_tool，line={}，kind=Screenshot，summary={}",
                    line, step.summary
                ));
                steps.push(step);
            }
        }
    }

    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    runtime_log_info(format!(
        "[桌面脚本] 完成，任务=run_operate_tool，executed_count={}，elapsed_ms={}，latest_screenshot={}，image_mime={}，has_image_base64={}，width={}，height={}",
        steps.len(),
        elapsed_ms,
        latest_screenshot
            .as_ref()
            .map(|shot| format!(
                "mode={},width={},height={},saved_path={}",
                shot.mode,
                shot.width,
                shot.height,
                shot.saved_path.as_deref().unwrap_or("-")
            ))
            .unwrap_or_else(|| "none".to_string()),
        image_mime.as_deref().unwrap_or("-"),
        image_base64.as_ref().map(|_| true).unwrap_or(false),
        width
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        height
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));

    Ok(OperateResponse {
        ok: true,
        executed_count: steps.len(),
        elapsed_ms,
        steps,
        latest_screenshot,
        image_mime,
        image_base64,
        width,
        height,
    })
}

#[cfg(test)]
mod operate_tool_tests {
    use super::*;

    fn parse_single(script: &str) -> DesktopScriptAction {
        parse_script(&OperateRequest { script: script.to_string() })
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn parse_mouse_click_script() {
        match parse_single("mouse left click @0.50,0.10 repeat=2 delay=0.1") {
            DesktopScriptAction::MouseClick { repeat, .. } => assert_eq!(repeat, 2),
            _ => panic!("expected mouse click"),
        }
    }

    #[test]
    fn parse_key_script() {
        match parse_single("key Control+L") {
            DesktopScriptAction::Key { keys, .. } => assert_eq!(keys, vec!["Control".to_string(), "L".to_string()]),
            _ => panic!("expected key action"),
        }
    }

    #[test]
    fn parse_text_requires_quotes() {
        let err = parse_script(&OperateRequest { script: "text hello".to_string() }).unwrap_err();
        assert!(err.message.contains("第 1 行 text"));
        assert!(err.message.contains("双引号"));
    }

    #[test]
    fn screenshot_save_requires_absolute_path() {
        let err = parse_script(&OperateRequest { script: r#"screenshot save="tmp/shot.webp""#.to_string() }).unwrap_err();
        assert!(err.message.contains("第 1 行 screenshot"));
        assert!(err.message.contains("绝对路径"));
    }

    #[test]
    fn mouse_coordinates_must_be_normalized() {
        let err = parse_script(&OperateRequest { script: "mouse left click @1.2,0.5".to_string() }).unwrap_err();
        assert!(err.message.contains("第 1 行 mouse"));
        assert!(err.message.contains("0.0~1.0"));
    }

    #[test]
    fn screenshot_region_should_parse() {
        match parse_single("screenshot region=@0.10,0.10,0.80,0.60") {
            DesktopScriptAction::Screenshot { mode: ScreenshotModeSpec::Region(_), .. } => {}
            _ => panic!("expected screenshot region"),
        }
    }
}
