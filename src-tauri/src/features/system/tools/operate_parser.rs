#[derive(Debug, Clone, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(
    description = "桌面脚本请求。只接收一个 script 字段；script 必须是多行字符串，一行一个动作。",
    example = operate_request_example()
)]
struct OperateRequest {
    #[schemars(
        description = "桌面脚本文本，一行一个动作。",
        example = operate_script_example()
    )]
    script: String,
}

fn operate_script_example() -> String {
    r#"mouse left click @0.50,0.10
wait 0.5
text "B站热榜"
key Enter
wait 1.0
screenshot"#
        .to_string()
}

fn operate_request_example() -> OperateRequest {
    OperateRequest {
        script: operate_script_example(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DesktopScriptStepKind {
    Mouse,
    Key,
    Text,
    Wait,
    Screenshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopScriptStepResult {
    line: usize,
    kind: DesktopScriptStepKind,
    summary: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatestScreenshotInfo {
    mode: String,
    width: u32,
    height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperateResponse {
    ok: bool,
    executed_count: usize,
    elapsed_ms: u64,
    steps: Vec<DesktopScriptStepResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_screenshot: Option<LatestScreenshotInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperateMouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone)]
struct NormalizedPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone)]
struct NormalizedRegion {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone)]
enum ScreenshotModeSpec {
    Desktop,
    FocusedWindow,
    Region(NormalizedRegion),
}

#[derive(Debug, Clone)]
enum DesktopScriptAction {
    MouseClick { line: usize, button: OperateMouseButton, target: NormalizedPoint, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration },
    MouseScroll { line: usize, direction: i32, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration },
    Key { line: usize, keys: Vec<String>, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration },
    Text { line: usize, text: String, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration },
    Wait { line: usize, duration: std::time::Duration },
    Screenshot { line: usize, mode: ScreenshotModeSpec, save_path: Option<String>, quality: f32 },
}

fn operate_invalid(message: impl Into<String>) -> DesktopToolError {
    DesktopToolError::invalid_params(message)
}

fn operate_line_error(line: usize, action: &str, message: impl Into<String>) -> DesktopToolError {
    operate_invalid(format!("第 {line} 行 {action} {}", message.into()))
}

fn tokenize_script_line(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::<String>::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                current.push(ch);
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if in_quotes {
        return Err("非法：双引号未闭合".to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

fn strip_quoted_value(token: &str) -> Option<String> {
    let trimmed = token.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return None;
    }
    Some(trimmed[1..trimmed.len() - 1].to_string())
}

fn parse_seconds_token(line: usize, action: &str, raw: &str, field: &str) -> DesktopToolResult<std::time::Duration> {
    let value = raw.parse::<f64>().map_err(|_| operate_line_error(line, action, format!("{field} 非法：必须是数字，当前为 `{raw}`")))?;
    if !value.is_finite() || value < 0.0 {
        return Err(operate_line_error(line, action, format!("{field} 非法：必须是 >= 0 的有限数字，当前为 `{raw}`")));
    }
    if value > 300.0 {
        return Err(operate_line_error(line, action, format!("{field} 非法：必须 <= 300 秒，当前为 `{raw}`")));
    }
    Ok(std::time::Duration::from_secs_f64(value))
}

fn parse_repeat_token(line: usize, action: &str, raw: &str) -> DesktopToolResult<u32> {
    let value = raw.parse::<u32>().map_err(|_| operate_line_error(line, action, format!("repeat 非法：必须是正整数，当前为 `{raw}`")))?;
    if value == 0 || value > 100 {
        return Err(operate_line_error(line, action, format!("repeat 非法：必须在 1~100 之间，当前为 `{raw}`")));
    }
    Ok(value)
}

fn parse_named_params(line: usize, action: &str, tokens: &[String], allowed: &[&str]) -> DesktopToolResult<std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::<String, String>::new();
    let allowed_set = allowed.iter().map(|item| item.to_string()).collect::<std::collections::HashSet<_>>();
    for token in tokens {
        let Some((raw_key, raw_value)) = token.split_once('=') else {
            return Err(operate_line_error(line, action, format!("非法参数 `{token}`：必须使用 key=value 形式")));
        };
        let key = raw_key.trim().to_ascii_lowercase();
        if !allowed_set.contains(&key) {
            return Err(operate_line_error(line, action, format!("不支持的参数 `{raw_key}`")));
        }
        if out.contains_key(&key) {
            return Err(operate_line_error(line, action, format!("参数 `{raw_key}` 重复出现")));
        }
        out.insert(key, raw_value.trim().to_string());
    }
    Ok(out)
}

fn parse_normalized_pair(line: usize, action: &str, raw: &str) -> DesktopToolResult<NormalizedPoint> {
    let value = raw.trim().strip_prefix('@').ok_or_else(|| operate_line_error(line, action, format!("坐标非法：必须使用 @x,y 形式，当前为 `{raw}`")))?;
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(operate_line_error(line, action, format!("坐标非法：必须使用 @x,y 形式，当前为 `{raw}`")));
    }
    let x = parts[0].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("坐标非法：x 必须是数字，当前为 `{}`", parts[0])))?;
    let y = parts[1].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("坐标非法：y 必须是数字，当前为 `{}`", parts[1])))?;
    if !(0.0..=1.0).contains(&x) {
        return Err(operate_line_error(line, action, format!("坐标非法：x 必须在 0.0~1.0 之间，当前为 {x}")));
    }
    if !(0.0..=1.0).contains(&y) {
        return Err(operate_line_error(line, action, format!("坐标非法：y 必须在 0.0~1.0 之间，当前为 {y}")));
    }
    Ok(NormalizedPoint { x, y })
}

fn parse_normalized_region(line: usize, action: &str, raw: &str) -> DesktopToolResult<NormalizedRegion> {
    let value = raw.trim().strip_prefix('@').ok_or_else(|| operate_line_error(line, action, format!("region 非法：必须使用 @x,y,w,h 形式，当前为 `{raw}`")))?;
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(operate_line_error(line, action, format!("region 非法：必须使用 @x,y,w,h 形式，当前为 `{raw}`")));
    }
    let x = parts[0].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("region x 非法：必须是数字，当前为 `{}`", parts[0])))?;
    let y = parts[1].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("region y 非法：必须是数字，当前为 `{}`", parts[1])))?;
    let width = parts[2].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("region width 非法：必须是数字，当前为 `{}`", parts[2])))?;
    let height = parts[3].parse::<f64>().map_err(|_| operate_line_error(line, action, format!("region height 非法：必须是数字，当前为 `{}`", parts[3])))?;
    for (name, value) in [("x", x), ("y", y), ("width", width), ("height", height)] {
        if !(0.0..=1.0).contains(&value) {
            return Err(operate_line_error(line, action, format!("region {name} 非法：必须在 0.0~1.0 之间，当前为 {value}")));
        }
    }
    if width <= 0.0 || height <= 0.0 {
        return Err(operate_line_error(line, action, "region width/height 非法：必须大于 0".to_string()));
    }
    if x + width > 1.0 || y + height > 1.0 {
        return Err(operate_line_error(line, action, "region 非法：x+width 与 y+height 必须不超过 1.0".to_string()));
    }
    Ok(NormalizedRegion { x, y, width, height })
}

fn parse_mouse_button(line: usize, raw: &str) -> DesktopToolResult<OperateMouseButton> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "left" => Ok(OperateMouseButton::Left),
        "right" => Ok(OperateMouseButton::Right),
        "middle" => Ok(OperateMouseButton::Middle),
        "back" => Ok(OperateMouseButton::Back),
        "forward" => Ok(OperateMouseButton::Forward),
        other => Err(operate_line_error(line, "mouse", format!("按钮非法：不支持 `{other}`"))),
    }
}

fn parse_key_combo(raw: &str) -> Vec<String> {
    raw.split('+').map(str::trim).filter(|item| !item.is_empty()).map(ToOwned::to_owned).collect()
}

fn parse_absolute_save_path(line: usize, action: &str, raw: &str) -> DesktopToolResult<String> {
    let Some(path) = strip_quoted_value(raw) else {
        return Err(operate_line_error(line, action, "save 非法：必须使用双引号包裹绝对路径".to_string()));
    };
    if !std::path::Path::new(path.trim()).is_absolute() {
        return Err(operate_line_error(line, action, "save 非法：必须是绝对路径".to_string()));
    }
    Ok(path)
}

fn parse_mouse_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "mouse", "非法：至少需要按钮或滚动方向".to_string()));
    }
    let subject = tokens[1].trim().to_ascii_lowercase();
    if subject == "scroll_up" || subject == "scroll_down" {
        let params = parse_named_params(line_no, "mouse", &tokens[2..], &["repeat", "delay", "pre_delay"])?;
        let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "mouse", v)).transpose()?.unwrap_or(1);
        let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "mouse", v, "delay")).transpose()?.unwrap_or_default();
        let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
        let direction = if subject == "scroll_up" { 1 } else { -1 };
        return Ok(DesktopScriptAction::MouseScroll { line: line_no, direction, repeat, delay, pre_delay });
    }
    if tokens.len() < 4 {
        return Err(operate_line_error(line_no, "mouse", "非法：点击格式应为 `mouse <button> click @x,y`".to_string()));
    }
    let button = parse_mouse_button(line_no, &tokens[1])?;
    if tokens[2].trim().to_ascii_lowercase() != "click" {
        return Err(operate_line_error(line_no, "mouse", format!("非法：暂只支持 `click`，当前为 `{}`", tokens[2])));
    }
    let target = parse_normalized_pair(line_no, "mouse", &tokens[3])?;
    let params = parse_named_params(line_no, "mouse", &tokens[4..], &["repeat", "delay", "pre_delay", "press"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "mouse", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "mouse", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
    let press = params.get("press").map(|v| parse_seconds_token(line_no, "mouse", v, "press")).transpose()?.unwrap_or_default();
    Ok(DesktopScriptAction::MouseClick { line: line_no, button, target, repeat, delay, pre_delay, press })
}

fn parse_key_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "key", "非法：缺少按键组合".to_string()));
    }
    let keys = parse_key_combo(&tokens[1]);
    if keys.is_empty() {
        return Err(operate_line_error(line_no, "key", "非法：缺少按键组合".to_string()));
    }
    let params = parse_named_params(line_no, "key", &tokens[2..], &["repeat", "delay", "pre_delay", "press"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "key", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "key", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "key", v, "pre_delay")).transpose()?.unwrap_or_default();
    let press = params.get("press").map(|v| parse_seconds_token(line_no, "key", v, "press")).transpose()?.unwrap_or_default();
    Ok(DesktopScriptAction::Key { line: line_no, keys, repeat, delay, pre_delay, press })
}

fn parse_text_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "text", "非法：缺少文本内容".to_string()));
    }
    let Some(text) = strip_quoted_value(&tokens[1]) else {
        return Err(operate_line_error(line_no, "text", "非法：必须使用双引号包裹文本内容".to_string()));
    };
    if text.is_empty() {
        return Err(operate_line_error(line_no, "text", "非法：文本内容不能为空".to_string()));
    }
    let params = parse_named_params(line_no, "text", &tokens[2..], &["repeat", "delay", "pre_delay"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "text", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "text", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "text", v, "pre_delay")).transpose()?.unwrap_or_default();
    Ok(DesktopScriptAction::Text { line: line_no, text, repeat, delay, pre_delay })
}

fn parse_wait_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() != 2 {
        return Err(operate_line_error(line_no, "wait", "非法：格式应为 `wait <seconds>`".to_string()));
    }
    let duration = parse_seconds_token(line_no, "wait", &tokens[1], "seconds")?;
    Ok(DesktopScriptAction::Wait { line: line_no, duration })
}

fn parse_screenshot_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    let mut mode = ScreenshotModeSpec::Desktop;
    let mut named_tokens = Vec::<String>::new();
    for token in &tokens[1..] {
        if token.contains('=') {
            named_tokens.push(token.clone());
            continue;
        }
        match token.trim().to_ascii_lowercase().as_str() {
            "focused_window" => {
                if !matches!(mode, ScreenshotModeSpec::Desktop) {
                    return Err(operate_line_error(line_no, "screenshot", "非法：focused_window 与 region 不能同时出现".to_string()));
                }
                mode = ScreenshotModeSpec::FocusedWindow;
            }
            other => return Err(operate_line_error(line_no, "screenshot", format!("非法参数 `{other}`"))),
        }
    }
    let params = parse_named_params(line_no, "screenshot", &named_tokens, &["region", "save", "quality"])?;
    if let Some(raw) = params.get("region") {
        if !matches!(mode, ScreenshotModeSpec::Desktop) {
            return Err(operate_line_error(line_no, "screenshot", "非法：focused_window 与 region 不能同时出现".to_string()));
        }
        mode = ScreenshotModeSpec::Region(parse_normalized_region(line_no, "screenshot", raw)?);
    }
    let save_path = params.get("save").map(|v| parse_absolute_save_path(line_no, "screenshot", v)).transpose()?;
    let quality = params.get("quality").map(|v| {
        let parsed = v.parse::<f32>().map_err(|_| operate_line_error(line_no, "screenshot", format!("quality 非法：必须是数字，当前为 `{v}`")))?;
        if !(1.0..=100.0).contains(&parsed) {
            return Err(operate_line_error(line_no, "screenshot", format!("quality 非法：必须在 1~100 之间，当前为 `{v}`")));
        }
        Ok(parsed)
    }).transpose()?.unwrap_or(75.0);
    Ok(DesktopScriptAction::Screenshot { line: line_no, mode, save_path, quality })
}

fn parse_script_line(line_no: usize, raw_line: &str) -> DesktopToolResult<Option<DesktopScriptAction>> {
    let trimmed = raw_line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let tokens = tokenize_script_line(trimmed).map_err(|err| operate_line_error(line_no, "脚本", err))?;
    if tokens.is_empty() {
        return Ok(None);
    }
    match tokens[0].trim().to_ascii_lowercase().as_str() {
        "mouse" => parse_mouse_line(line_no, &tokens).map(Some),
        "key" => parse_key_line(line_no, &tokens).map(Some),
        "text" => parse_text_line(line_no, &tokens).map(Some),
        "wait" => parse_wait_line(line_no, &tokens).map(Some),
        "screenshot" => parse_screenshot_line(line_no, &tokens).map(Some),
        other => Err(operate_line_error(line_no, "脚本", format!("未知动作：{other}。可用动作：mouse、key、text、wait、screenshot"))),
    }
}

fn parse_script(request: &OperateRequest) -> DesktopToolResult<Vec<DesktopScriptAction>> {
    let trimmed = request.script.trim();
    if trimmed.is_empty() {
        return Err(operate_invalid("script 不能为空"));
    }
    let mut actions = Vec::<DesktopScriptAction>::new();
    for (idx, raw_line) in request.script.lines().enumerate() {
        if let Some(action) = parse_script_line(idx + 1, raw_line)? {
            actions.push(action);
        }
    }
    if actions.is_empty() {
        return Err(operate_invalid("script 不能为空"));
    }
    Ok(actions)
}
