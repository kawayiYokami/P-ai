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
    #[serde(default)]
    #[schemars(
        description = "本次桌面脚本工具调用的超时时间，单位毫秒；未指定时默认 300000ms。长时间 wait 或自动化脚本应显式传入足够大的值。"
    )]
    timeout_ms: Option<u64>,
    /// 单步重试预算（D4）：每个输入步骤失败时最多重试次数，0~3，默认 0（不重试）。
    /// 每次重试间隔 200ms；参数非法、超时中断不重试。
    #[serde(default)]
    #[schemars(description = "单步重试预算：每个输入步骤失败时最多重试次数（0~3，默认0）。瞬态失败（焦点竞争、加载慢）可设 1~3。")]
    retry: Option<u32>,
    /// 执行后焦点恢复（C4）：为 true 时脚本结束后把前台切回执行前的窗口，失败只记 warnings。
    #[serde(default)]
    #[schemars(description = "执行后焦点恢复：为 true 时脚本结束后把前台切回执行前的窗口（默认 false）。")]
    restore_focus: Option<bool>,
}

fn operate_script_example() -> String {
    r#"mouse left click @960,108
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
        timeout_ms: None,
        retry: None,
        restore_focus: None,
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
    App,
    Window,
    Clipboard,
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
    /// 该图对应的屏幕矩形（虚拟桌面物理像素）。图可能被 max_pixels 等比缩小，
    /// 模型按 width/height 与 bounds 的比例换算出屏幕坐标后再写进脚本。
    bounds: ScreenBounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree: Option<Vec<UiElementInfo>>,
}

/// 单张截图的图片负载。字段名与工具结果的 parts 约定保持一致（type/mimeType/data），
/// 转发层用同一套规则把它提取成图片 part；`data` 被提取后会从给模型的文本里移除。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperateScreenshotPart {
    #[serde(rename = "type")]
    kind: String,
    mime_type: String,
    data: String,
    width: u32,
    height: u32,
    /// 该图对应的脚本行号
    line: usize,
    mode: String,
    /// 该图对应的屏幕矩形（虚拟桌面物理像素）；图被 max_pixels 缩小过时按比例换算坐标
    bounds: ScreenBounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    saved_path: Option<String>,
}

/// window list 的紧凑窗口条目：只留模型真正用得上的字段（按 id / 名字引用窗口、判断是否最小化）。
/// 坐标、尺寸、进程号对 operate 动作没有用处，去掉后 22 个窗口不再一次吃掉两百行上下文。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowBrief {
    window_id: usize,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_name: Option<String>,
    focused: bool,
    minimized: bool,
}

impl From<&WindowInfo> for WindowBrief {
    fn from(window: &WindowInfo) -> Self {
        Self {
            window_id: window.window_id,
            title: window.title.clone(),
            process_name: window.process_name.clone(),
            focused: window.focused,
            minimized: window.minimized,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperateResponse {
    ok: bool,
    executed_count: usize,
    elapsed_ms: u64,
    steps: Vec<DesktopScriptStepResult>,
    /// 中途失败的位置与原因；成功时为 None。失败时 steps 仍包含已完成步骤（D1）
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<OperateFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_screenshot: Option<LatestScreenshotInfo>,
    /// 本次调用内截取的图片，按执行顺序排列；画面与上一次完全相同而被去重的那张不在此列。
    /// 字段名与工具结果的 parts 约定对齐（type/mimeType/data），转发层按图片通道一并回传。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    screenshots: Vec<OperateScreenshotPart>,
    /// window list 动作返回的可见窗口列表（最近一次），紧凑字段
    #[serde(skip_serializing_if = "Option::is_none")]
    windows: Option<Vec<WindowBrief>>,
    /// 不影响执行成功、但会改变坐标或输入可信度的环境提示（如 DPI 感知未生效）
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<String>>,
}

/// 脚本中途失败的位置与原因；失败时 steps 仍返回已完成步骤，模型可据此从失败行重试（D1）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperateFailure {
    line: usize,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    focus_failed: Option<FocusFailureInfo>,
}

/// 抢焦点失败时的结构化现场：目标状态、前后前台、恢复建议（C3）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FocusFailureInfo {
    target_window_id: u32,
    target_title: String,
    /// 窗口句柄是否仍然存在（存在但不可见即为被隐藏）
    alive: bool,
    /// 窗口是否出现在可见窗口枚举里
    visible: bool,
    minimized: bool,
    foreground_before: String,
    foreground_after: String,
    /// 建议动作：activate_window / unhide_app / open_application
    suggested_recovery: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperateMouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// 条件等待的默认超时（秒）：未显式给 timeout 时使用。
const DEFAULT_WAIT_UNTIL_TIMEOUT_SECS: f64 = 30.0;

/// 条件等待的目标条件（D2）
#[derive(Debug, Clone)]
enum WaitCondition {
    /// 出现标题包含指定子串的可见窗口
    Window { title: String },
    /// 目标窗口（未指定时为当前前台窗口）内出现名称包含指定子串的可交互元素
    Element { name: String, window_target: Option<ForegroundTarget> },
}

/// 前台动作的目标窗口声明：句柄或标题子串
#[derive(Debug, Clone)]
enum ForegroundTarget {
    WindowId(u32),
    Title(String),
}

/// 前台动作的焦点策略；仅在声明了 target 时生效
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPolicy {
    /// 不激活，只校验当前前台是否为目标窗口，不符即失败（声明 target 时的默认）
    Verify,
    /// 尝试激活，失败不中止，在结果中报告
    BestEffort,
    /// 必须激活成功，失败即中止并返回结构化焦点失败信息
    Strict,
}

/// 屏幕坐标：虚拟桌面物理像素。主屏左上角通常为 (0,0)，副屏在主屏左侧或上方时坐标为负。
#[derive(Debug, Clone, Copy)]
struct ScreenPoint {
    x: i32,
    y: i32,
}

/// 屏幕矩形：虚拟桌面物理像素。
#[derive(Debug, Clone, Copy)]
struct ScreenRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// 坐标合理范围上限：防止溢出与明显离谱的值（真实虚拟桌面远小于此）。
const MAX_SCREEN_COORDINATE: i32 = 100_000;

#[derive(Debug, Clone)]
enum ScreenshotModeSpec {
    Desktop,
    FocusedWindow,
    Region(ScreenRect),
    WindowId(u32),
    /// 按标题/进程名引用窗口（G2），执行时解析为句柄
    WindowName(String),
    Monitor(u32),
}

/// app 动作目标：元素名（新鲜扫描、唯一命中）或屏幕绝对像素坐标（@x,y，窗口内命中测试）。
/// el= 已退役：模型写 ref 序号时并不知道那是不是它想点的东西，写名字时它知道自己在说什么。
#[derive(Debug, Clone)]
enum AppScriptTarget {
    Name(String),
    Point(ScreenPoint),
}

#[derive(Debug, Clone)]
enum AppScriptAction {
    Click { target: AppScriptTarget, repeat: u32, dblclick: bool, pre_delay: std::time::Duration },
    SetValue { name: String, text: String, pre_delay: std::time::Duration, verify: bool },
    /// 后台滚动（A3）：horizontal=false 为垂直（positive=true 向下），true 为水平（positive=true 向右）。
    /// 符号与 enigo scroll 约定一致：垂直正=下，水平正=右。
    Scroll { target: AppScriptTarget, horizontal: bool, positive: bool, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration },
    Key { keys: Vec<String>, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration },
    GetValue { name: String },
}

/// 窗口变量名规则：ASCII 字母开头，后接字母/数字/下划线；大小写敏感；不得与动作关键字重名。
fn is_var_name(raw: &str) -> bool {
    let mut chars = raw.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 动作关键字：脚本行首出现这些词时按内置动作解析，不按变量动作解析。
fn is_script_keyword(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "mouse" | "app" | "key" | "text" | "wait" | "window" | "screenshot" | "clipboard"
    )
}

/// 老语法迁移提示（I2）：报错即给改写示例，模型可自我纠正。
fn app_legacy_hint() -> String {
    "app <windowId> / el= 已退役：先声明窗口变量再按元素名操作，例如：w = app \"记事本\"，然后 w click \"保存\"".to_string()
}

#[derive(Debug, Clone)]
enum ClipboardOp {
    Read,
    Write(String),
}

#[derive(Debug, Clone)]
enum DesktopScriptAction {
    MouseClick { line: usize, button: OperateMouseButton, target: ScreenPoint, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy, verify: bool },
    MouseDrag { line: usize, button: OperateMouseButton, from: ScreenPoint, to: ScreenPoint, duration: Option<std::time::Duration>, pre_delay: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy },
    MouseMove { line: usize, target: ScreenPoint, pre_delay: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy },
    MouseButtonState { line: usize, button: OperateMouseButton, pressed: bool, pre_delay: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy },
    /// 前台滚动（A3）：horizontal=false 为垂直（direction>0 向下），true 为水平（direction>0 向右）。
    /// 符号按 enigo 约定（引入于 116a8ed3b 的旧符号正负颠倒，本次一并修正）。
    MouseScroll { line: usize, horizontal: bool, direction: i32, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy },
    Key { line: usize, keys: Vec<String>, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, press: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy, verify: bool },
    Text { line: usize, text: String, repeat: u32, delay: std::time::Duration, pre_delay: std::time::Duration, window_target: Option<ForegroundTarget>, focus: FocusPolicy, verify: bool },
    Wait { line: usize, duration: std::time::Duration },
    WaitUntil { line: usize, condition: WaitCondition, timeout: std::time::Duration },
    WindowList { line: usize },
    WindowActivate { line: usize, target: ForegroundTarget },
    Screenshot { line: usize, mode: ScreenshotModeSpec, save_path: Option<String>, quality: f32, elements: bool, include_text: bool, max_pixels: Option<u64> },
    /// 窗口变量声明：`w = app "记事本"`，本次调用内有效，重复声明即覆盖
    AppDeclare { line: usize, var: String, name: String },
    /// 变量动作：`w click "保存"` / `w setvalue "用户名" = "admin"` / `w click @960,540`
    App { line: usize, var: String, action: AppScriptAction, post_delay: std::time::Duration },
    Clipboard { line: usize, op: ClipboardOp },
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

fn parse_bool_token(line: usize, action: &str, raw: &str) -> DesktopToolResult<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(operate_line_error(line, action, format!("布尔参数非法：必须是 true/1 或 false/0，当前为 `{raw}`"))),
    }
}

/// monitor 参数：显示器序号，0 起。省略时用主屏。
fn parse_monitor_token(line: usize, action: &str, raw: &str) -> DesktopToolResult<u32> {
    raw.trim()
        .parse::<u32>()
        .map_err(|_| operate_line_error(line, action, format!("monitor 非法：必须是非负整数，当前为 `{raw}`")))
}

/// target 参数：窗口句柄（十进制或 0x 十六进制）或带双引号的标题子串。
fn parse_foreground_target(line: usize, action: &str, raw: &str) -> DesktopToolResult<ForegroundTarget> {
    if let Some(title) = strip_quoted_value(raw) {
        if title.trim().is_empty() {
            return Err(operate_line_error(line, action, "target 非法：标题不能为空".to_string()));
        }
        return Ok(ForegroundTarget::Title(title));
    }
    parse_window_id_token(line, action, raw).map(ForegroundTarget::WindowId)
}

/// focus 参数：verify（默认，只校验不激活）/ best_effort（尝试激活，失败继续）/ strict（激活失败即中止）。
fn parse_focus_token(line: usize, action: &str, raw: &str) -> DesktopToolResult<FocusPolicy> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "verify" => Ok(FocusPolicy::Verify),
        "best_effort" => Ok(FocusPolicy::BestEffort),
        "strict" => Ok(FocusPolicy::Strict),
        other => Err(operate_line_error(line, action, format!("focus 非法：必须是 verify / best_effort / strict，当前为 `{other}`"))),
    }
}

/// 解析前台动作的 target/focus 参数对；未声明 target 时返回 (None, Verify)，行为与历史脚本一致。
fn parse_foreground_params(
    line: usize,
    action: &str,
    params: &std::collections::HashMap<String, String>,
) -> DesktopToolResult<(Option<ForegroundTarget>, FocusPolicy)> {
    let window_target = params.get("target").map(|raw| parse_foreground_target(line, action, raw)).transpose()?;
    let focus = params.get("focus").map(|raw| parse_focus_token(line, action, raw)).transpose()?;
    match (&window_target, focus) {
        (None, Some(_)) => Err(operate_line_error(line, action, "focus 必须与 target 一起使用".to_string())),
        (_, Some(policy)) => Ok((window_target, policy)),
        (_, None) => Ok((window_target, FocusPolicy::Verify)),
    }
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

/// 坐标已改为虚拟桌面绝对像素，`monitor=` 不再参与坐标换算：出现在坐标类动作里直接报错并指路。
fn reject_monitor_param(line: usize, action: &str, tokens: &[String]) -> DesktopToolResult<()> {
    if tokens.iter().any(|token| token.trim().to_ascii_lowercase().starts_with("monitor=")) {
        return Err(operate_line_error(
            line,
            action,
            "monitor 参数已移除：坐标改为虚拟桌面绝对像素，跨显示器唯一，不再需要指定坐标基准；截图请用 `screenshot monitor=<id>`".to_string(),
        ));
    }
    Ok(())
}

/// 坐标解析：`@x,y` 为虚拟桌面物理像素（整数）。旧归一化写法（如 @0.5,0.5）直接报错，不静默取整。
fn parse_screen_point(line: usize, action: &str, raw: &str) -> DesktopToolResult<ScreenPoint> {
    let value = raw.trim().strip_prefix('@').ok_or_else(|| operate_line_error(line, action, format!("坐标非法：必须使用 @x,y 形式，当前为 `{raw}`")))?;
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(operate_line_error(line, action, format!("坐标非法：必须使用 @x,y 形式，当前为 `{raw}`")));
    }
    Ok(ScreenPoint {
        x: parse_pixel_axis(line, action, "x", parts[0])?,
        y: parse_pixel_axis(line, action, "y", parts[1])?,
    })
}

/// 单轴像素解析：必须是整数。小数按旧归一化写法给出迁移提示，避免静默取整后点到屏幕角落。
fn parse_pixel_axis(line: usize, action: &str, axis: &str, raw: &str) -> DesktopToolResult<i32> {
    let trimmed = raw.trim();
    let value = trimmed.parse::<i32>().map_err(|_| {
        if trimmed.parse::<f64>().is_ok() {
            operate_line_error(line, action, format!("坐标非法：{axis} 必须是整数像素，当前为 `{trimmed}`；坐标已改为虚拟桌面绝对像素，不再是 0~1 归一化"))
        } else {
            operate_line_error(line, action, format!("坐标非法：{axis} 必须是整数像素，当前为 `{trimmed}`"))
        }
    })?;
    if value.abs() > MAX_SCREEN_COORDINATE {
        return Err(operate_line_error(line, action, format!("坐标非法：{axis} 超出合理范围（绝对值不超过 {MAX_SCREEN_COORDINATE}），当前为 {value}")));
    }
    Ok(value)
}

/// 像素长度解析：必须为正整数。
fn parse_pixel_extent(line: usize, action: &str, field: &str, raw: &str) -> DesktopToolResult<u32> {
    let trimmed = raw.trim();
    let value = trimmed.parse::<u32>().map_err(|_| operate_line_error(line, action, format!("{field} 非法：必须是正整数像素，当前为 `{trimmed}`")))?;
    if value == 0 || value > MAX_SCREEN_COORDINATE as u32 {
        return Err(operate_line_error(line, action, format!("{field} 非法：必须在 1~{MAX_SCREEN_COORDINATE} 之间，当前为 {value}")));
    }
    Ok(value)
}

/// region 解析：`@x,y,w,h` 为虚拟桌面物理像素矩形（整数）。
fn parse_screen_region(line: usize, action: &str, raw: &str) -> DesktopToolResult<ScreenRect> {
    let value = raw.trim().strip_prefix('@').ok_or_else(|| operate_line_error(line, action, format!("region 非法：必须使用 @x,y,w,h 形式，当前为 `{raw}`")))?;
    let parts = value.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(operate_line_error(line, action, format!("region 非法：必须使用 @x,y,w,h 形式，当前为 `{raw}`")));
    }
    Ok(ScreenRect {
        x: parse_pixel_axis(line, action, "region x", parts[0])?,
        y: parse_pixel_axis(line, action, "region y", parts[1])?,
        width: parse_pixel_extent(line, action, "region width", parts[2])?,
        height: parse_pixel_extent(line, action, "region height", parts[3])?,
    })
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
        return Err(operate_line_error(line_no, "mouse", "非法：至少需要按钮、滚动方向或 move".to_string()));
    }
    reject_monitor_param(line_no, "mouse", tokens)?;
    let subject = tokens[1].trim().to_ascii_lowercase();
    if subject == "scroll_up" || subject == "scroll_down" || subject == "scroll_left" || subject == "scroll_right" {
        let params = parse_named_params(line_no, "mouse", &tokens[2..], &["repeat", "delay", "pre_delay", "target", "focus"])?;
        let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "mouse", v)).transpose()?.unwrap_or(1);
        let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "mouse", v, "delay")).transpose()?.unwrap_or_default();
        let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
        let (window_target, focus) = parse_foreground_params(line_no, "mouse", &params)?;
        // 符号按 enigo 约定：垂直正=下、水平正=右（scroll_up 取负）。
        // 旧代码正负颠倒（引入于 116a8ed3b），本次随 A3 一并修正。
        let (horizontal, direction) = match subject.as_str() {
            "scroll_up" => (false, -1),
            "scroll_down" => (false, 1),
            "scroll_left" => (true, -1),
            _ => (true, 1),
        };
        return Ok(DesktopScriptAction::MouseScroll { line: line_no, horizontal, direction, repeat, delay, pre_delay, window_target, focus });
    }
    if subject == "move" {
        if tokens.len() < 3 {
            return Err(operate_line_error(line_no, "mouse", "非法：移动格式应为 `mouse move @x,y`".to_string()));
        }
        let target = parse_screen_point(line_no, "mouse", &tokens[2])?;
        let params = parse_named_params(line_no, "mouse", &tokens[3..], &["pre_delay", "target", "focus"])?;
        let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
        let (window_target, focus) = parse_foreground_params(line_no, "mouse", &params)?;
        return Ok(DesktopScriptAction::MouseMove { line: line_no, target, pre_delay, window_target, focus });
    }
    if tokens.len() < 3 {
        return Err(operate_line_error(line_no, "mouse", "非法：格式应为 `mouse <button> click @x,y` / `mouse <button> drag @x1,y1 @x2,y2` / `mouse <button> down|up`".to_string()));
    }
    let button = parse_mouse_button(line_no, &tokens[1])?;
    let verb = tokens[2].trim().to_ascii_lowercase();
    if verb == "down" || verb == "up" {
        let params = parse_named_params(line_no, "mouse", &tokens[3..], &["pre_delay", "target", "focus"])?;
        let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
        let (window_target, focus) = parse_foreground_params(line_no, "mouse", &params)?;
        return Ok(DesktopScriptAction::MouseButtonState { line: line_no, button, pressed: verb == "down", pre_delay, window_target, focus });
    }
    if tokens.len() < 4 {
        return Err(operate_line_error(line_no, "mouse", "非法：格式应为 `mouse <button> click @x,y` 或 `mouse <button> drag @x1,y1 @x2,y2`".to_string()));
    }
    if verb == "drag" {
        if tokens.len() < 5 {
            return Err(operate_line_error(line_no, "mouse", "非法：拖拽格式应为 `mouse <button> drag @x1,y1 @x2,y2 [duration=s]`".to_string()));
        }
        let from = parse_screen_point(line_no, "mouse", &tokens[3])?;
        let to = parse_screen_point(line_no, "mouse", &tokens[4])?;
        let params = parse_named_params(line_no, "mouse", &tokens[5..], &["duration", "pre_delay", "target", "focus"])?;
        let duration = params.get("duration").map(|v| parse_seconds_token(line_no, "mouse", v, "duration")).transpose()?;
        let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
        let (window_target, focus) = parse_foreground_params(line_no, "mouse", &params)?;
        return Ok(DesktopScriptAction::MouseDrag { line: line_no, button, from, to, duration, pre_delay, window_target, focus });
    }
    if verb != "click" {
        return Err(operate_line_error(line_no, "mouse", format!("非法：暂只支持 `click` / `drag`，当前为 `{}`", tokens[2])));
    }
    let target = parse_screen_point(line_no, "mouse", &tokens[3])?;
    let params = parse_named_params(line_no, "mouse", &tokens[4..], &["repeat", "delay", "pre_delay", "press", "target", "focus", "verify"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "mouse", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "mouse", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "mouse", v, "pre_delay")).transpose()?.unwrap_or_default();
    let press = params.get("press").map(|v| parse_seconds_token(line_no, "mouse", v, "press")).transpose()?.unwrap_or_default();
    let (window_target, focus) = parse_foreground_params(line_no, "mouse", &params)?;
    let verify = params.get("verify").map(|v| parse_bool_token(line_no, "mouse", v)).transpose()?.unwrap_or(false);
    Ok(DesktopScriptAction::MouseClick { line: line_no, button, target, repeat, delay, pre_delay, press, window_target, focus, verify })
}

/// windowId 解析：十进制或 0x 前缀十六进制（与 windows 工具的 id 口径一致）
fn parse_window_id_token(line: usize, action: &str, raw: &str) -> DesktopToolResult<u32> {
    let trimmed = raw.trim();
    let parsed = if let Some(hex) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<u32>()
    };
    parsed.map_err(|_| operate_line_error(line, action, format!("windowId 非法：必须是十进制或 0x 前缀十六进制，当前为 `{raw}`")))
}

/// 变量动作目标解析：`@x,y` 屏幕绝对像素坐标，或元素名（带双引号；无空格单名可省略引号）。
/// el= 已退役，出现即报错并给改写示例。
fn parse_var_target(line: usize, var: &str, verb: &str, token: &str) -> DesktopToolResult<AppScriptTarget> {
    let trimmed = token.trim();
    if trimmed.starts_with('@') {
        return Ok(AppScriptTarget::Point(parse_screen_point(line, var, trimmed)?));
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("el=") {
        return Err(operate_line_error(line, var, format!("{verb} {}", app_legacy_hint())));
    }
    let name = strip_quoted_value(trimmed).unwrap_or_else(|| trimmed.to_string());
    if name.trim().is_empty() {
        return Err(operate_line_error(line, var, format!("{verb} 非法：元素名不能为空")));
    }
    Ok(AppScriptTarget::Name(name))
}

/// `w = app "记事本"`：声明本次调用内的窗口变量，重复声明即覆盖。
fn parse_var_declare_line(
    line_no: usize,
    tokens: &[String],
    declared: &mut std::collections::HashSet<String>,
) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 4 || tokens[1] != "=" || !tokens[2].trim().eq_ignore_ascii_case("app") {
        return Err(operate_line_error(line_no, "脚本", "非法：声明格式应为 `w = app \"窗口名\"`，例如：w = app \"记事本\""));
    }
    let var = tokens[0].trim().to_string();
    if !is_var_name(&var) {
        return Err(operate_line_error(line_no, "脚本", format!("变量名非法 `{var}`：须为字母开头、仅含字母/数字/下划线")));
    }
    if is_script_keyword(&var) {
        return Err(operate_line_error(line_no, "脚本", format!("变量名非法 `{var}`：不得与动作关键字重名")));
    }
    let raw = tokens[3].trim();
    let name = strip_quoted_value(raw).unwrap_or_else(|| raw.to_string());
    if name.trim().is_empty() {
        return Err(operate_line_error(line_no, &var, "窗口名不能为空，需给\"标题/进程名\"".to_string()));
    }
    reject_monitor_param(line_no, &var, &tokens[4..])?;
    if tokens.len() > 4 {
        return Err(operate_line_error(line_no, &var, format!("声明格式应为 `w = app \"窗口名\"`，不接受额外参数 `{}`", tokens[4])));
    }
    declared.insert(var.clone());
    Ok(DesktopScriptAction::AppDeclare { line: line_no, var, name })
}

fn parse_var_action_line(
    line_no: usize,
    tokens: &[String],
    declared: &std::collections::HashSet<String>,
) -> DesktopToolResult<DesktopScriptAction> {
    let var = tokens[0].trim().to_string();
    if !declared.contains(&var) {
        return Err(operate_line_error(line_no, &var, format!("变量 `{var}` 未声明：先用 `{var} = app \"窗口名\"` 声明，例如：{var} = app \"记事本\"")));
    }
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, &var, "非法：格式应为 `w click \"名\"|@x,y` / `w setvalue \"名\" = \"内容\"` / `w getvalue \"名\"` / `w scroll_up \"名\"|@x,y` / `w key <combo>`".to_string()));
    }
    let verb = tokens[1].trim().to_ascii_lowercase();
    reject_monitor_param(line_no, &var, &tokens[2..])?;
    // post_delay 是全部变量动作共享的收尾等待：动作完成后等 UI 响应（弹菜单/联想词）再返回，
    // 避免下一条动作拿到过期的元素树
    let parse_post_delay = |params: &std::collections::HashMap<String, String>| -> DesktopToolResult<std::time::Duration> {
        params
            .get("post_delay")
            .map(|v| parse_seconds_token(line_no, &var, v, "post_delay"))
            .transpose()
            .map(|d| d.unwrap_or_default())
    };
    match verb.as_str() {
        "click" => {
            if tokens.len() < 3 {
                return Err(operate_line_error(line_no, &var, "非法：格式应为 `w click \"名\"|@x,y [参数]`".to_string()));
            }
            let target = parse_var_target(line_no, &var, "click", &tokens[2])?;
            let params = parse_named_params(line_no, &var, &tokens[3..], &["repeat", "dblclick", "pre_delay", "post_delay"])?;
            let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, &var, v)).transpose()?.unwrap_or(1);
            let dblclick = params.get("dblclick").map(|v| parse_bool_token(line_no, &var, v)).transpose()?.unwrap_or(false);
            let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, &var, v, "pre_delay")).transpose()?.unwrap_or_default();
            let post_delay = parse_post_delay(&params)?;
            Ok(DesktopScriptAction::App { line: line_no, var, action: AppScriptAction::Click { target, repeat, dblclick, pre_delay }, post_delay })
        }
        "setvalue" => {
            if tokens.len() < 5 || tokens[3] != "=" {
                return Err(operate_line_error(line_no, &var, "非法：setvalue 格式应为 `w setvalue \"名\" = \"内容\"`".to_string()));
            }
            let name_token = tokens[2].trim();
            if name_token.starts_with('@') || name_token.to_ascii_lowercase().starts_with("el=") {
                return Err(operate_line_error(line_no, &var, format!("setvalue {}", app_legacy_hint())));
            }
            let name = strip_quoted_value(name_token).unwrap_or_else(|| name_token.to_string());
            if name.trim().is_empty() {
                return Err(operate_line_error(line_no, &var, "非法：元素名不能为空".to_string()));
            }
            let Some(text) = strip_quoted_value(&tokens[4]) else {
                return Err(operate_line_error(line_no, &var, "非法：必须使用双引号包裹文本内容".to_string()));
            };
            let text = text.replace("\\n", "\n");
            if text.is_empty() {
                return Err(operate_line_error(line_no, &var, "非法：文本内容不能为空".to_string()));
            }
            let params = parse_named_params(line_no, &var, &tokens[5..], &["pre_delay", "post_delay", "verify"])?;
            let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, &var, v, "pre_delay")).transpose()?.unwrap_or_default();
            let post_delay = parse_post_delay(&params)?;
            let verify = params.get("verify").map(|v| parse_bool_token(line_no, &var, v)).transpose()?.unwrap_or(false);
            Ok(DesktopScriptAction::App { line: line_no, var, action: AppScriptAction::SetValue { name, text, pre_delay, verify }, post_delay })
        }
        "getvalue" => {
            if tokens.len() != 3 {
                return Err(operate_line_error(line_no, &var, "非法：格式应为 `w getvalue \"名\"`".to_string()));
            }
            let name_token = tokens[2].trim();
            if name_token.starts_with('@') || name_token.to_ascii_lowercase().starts_with("el=") {
                return Err(operate_line_error(line_no, &var, format!("getvalue {}", app_legacy_hint())));
            }
            let name = strip_quoted_value(name_token).unwrap_or_else(|| name_token.to_string());
            if name.trim().is_empty() {
                return Err(operate_line_error(line_no, &var, "非法：元素名不能为空".to_string()));
            }
            Ok(DesktopScriptAction::App { line: line_no, var, action: AppScriptAction::GetValue { name }, post_delay: std::time::Duration::ZERO })
        }
        "scroll_up" | "scroll_down" | "scroll_left" | "scroll_right" => {
            if tokens.len() < 3 {
                return Err(operate_line_error(line_no, &var, format!("非法：格式应为 `w {verb} \"名\"|@x,y [参数]`")));
            }
            let target = parse_var_target(line_no, &var, &verb, &tokens[2])?;
            let params = parse_named_params(line_no, &var, &tokens[3..], &["repeat", "delay", "pre_delay", "post_delay"])?;
            let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, &var, v)).transpose()?.unwrap_or(1);
            let delay = params.get("delay").map(|v| parse_seconds_token(line_no, &var, v, "delay")).transpose()?.unwrap_or_default();
            let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, &var, v, "pre_delay")).transpose()?.unwrap_or_default();
            let post_delay = parse_post_delay(&params)?;
            // 符号与 enigo 约定一致：垂直正=下、水平正=右
            let (horizontal, positive) = match verb.as_str() {
                "scroll_up" => (false, false),
                "scroll_down" => (false, true),
                "scroll_left" => (true, false),
                _ => (true, true),
            };
            let action = AppScriptAction::Scroll { target, horizontal, positive, repeat, delay, pre_delay };
            Ok(DesktopScriptAction::App { line: line_no, var, action, post_delay })
        }
        "key" => {
            if tokens.len() < 3 {
                return Err(operate_line_error(line_no, &var, "非法：key 缺少按键组合，格式应为 `w key <combo>`".to_string()));
            }
            let keys = parse_key_combo(&tokens[2]);
            if keys.is_empty() {
                return Err(operate_line_error(line_no, &var, "非法：缺少按键组合".to_string()));
            }
            let params = parse_named_params(line_no, &var, &tokens[3..], &["repeat", "delay", "pre_delay", "post_delay"])?;
            let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, &var, v)).transpose()?.unwrap_or(1);
            let delay = params.get("delay").map(|v| parse_seconds_token(line_no, &var, v, "delay")).transpose()?.unwrap_or_default();
            let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, &var, v, "pre_delay")).transpose()?.unwrap_or_default();
            let post_delay = parse_post_delay(&params)?;
            Ok(DesktopScriptAction::App { line: line_no, var, action: AppScriptAction::Key { keys, repeat, delay, pre_delay }, post_delay })
        }
        other => Err(operate_line_error(line_no, &var, format!("非法：暂只支持 click / setvalue / getvalue / scroll_up / scroll_down / scroll_left / scroll_right / key，当前为 `{other}`"))),
    }
}

/// 老 `app` 写法已退役：统一报错并给改写示例，不再接受任何参数。
fn parse_app_line(line_no: usize, _tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    Err(operate_line_error(line_no, "app", app_legacy_hint()))
}

fn parse_key_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "key", "非法：缺少按键组合".to_string()));
    }
    let keys = parse_key_combo(&tokens[1]);
    if keys.is_empty() {
        return Err(operate_line_error(line_no, "key", "非法：缺少按键组合".to_string()));
    }
    let params = parse_named_params(line_no, "key", &tokens[2..], &["repeat", "delay", "pre_delay", "press", "target", "focus", "verify"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "key", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "key", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "key", v, "pre_delay")).transpose()?.unwrap_or_default();
    let press = params.get("press").map(|v| parse_seconds_token(line_no, "key", v, "press")).transpose()?.unwrap_or_default();
    let (window_target, focus) = parse_foreground_params(line_no, "key", &params)?;
    let verify = params.get("verify").map(|v| parse_bool_token(line_no, "key", v)).transpose()?.unwrap_or(false);
    Ok(DesktopScriptAction::Key { line: line_no, keys, repeat, delay, pre_delay, press, window_target, focus, verify })
}

fn parse_text_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "text", "非法：缺少文本内容".to_string()));
    }
    let Some(text) = strip_quoted_value(&tokens[1]) else {
        return Err(operate_line_error(line_no, "text", "非法：必须使用双引号包裹文本内容".to_string()));
    };
    // 字面 `\n`（反斜杠+n）解码为真实换行，支持脚本单行书写多行文本
    let text = text.replace("\\n", "\n");
    if text.is_empty() {
        return Err(operate_line_error(line_no, "text", "非法：文本内容不能为空".to_string()));
    }
    let params = parse_named_params(line_no, "text", &tokens[2..], &["repeat", "delay", "pre_delay", "target", "focus", "verify"])?;
    let repeat = params.get("repeat").map(|v| parse_repeat_token(line_no, "text", v)).transpose()?.unwrap_or(1);
    let delay = params.get("delay").map(|v| parse_seconds_token(line_no, "text", v, "delay")).transpose()?.unwrap_or_default();
    let pre_delay = params.get("pre_delay").map(|v| parse_seconds_token(line_no, "text", v, "pre_delay")).transpose()?.unwrap_or_default();
    let (window_target, focus) = parse_foreground_params(line_no, "text", &params)?;
    let verify = params.get("verify").map(|v| parse_bool_token(line_no, "text", v)).transpose()?.unwrap_or(false);
    Ok(DesktopScriptAction::Text { line: line_no, text, repeat, delay, pre_delay, window_target, focus, verify })
}

fn parse_window_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "window", "非法：格式应为 `window list` 或 `window activate <windowId|\"标题\">`".to_string()));
    }
    match tokens[1].trim().to_ascii_lowercase().as_str() {
        "list" => {
            if tokens.len() != 2 {
                return Err(operate_line_error(line_no, "window", format!("非法参数 `{}`：window list 不接受参数", tokens[2])));
            }
            Ok(DesktopScriptAction::WindowList { line: line_no })
        }
        "activate" | "focus" => {
            if tokens.len() != 3 {
                return Err(operate_line_error(line_no, "window", "非法：格式应为 `window activate <windowId|\"标题\">`".to_string()));
            }
            Ok(DesktopScriptAction::WindowActivate { line: line_no, target: parse_foreground_target(line_no, "window", &tokens[2])? })
        }
        other => Err(operate_line_error(line_no, "window", format!("未知子动作：{other}。可用：list、activate"))),
    }
}

fn parse_wait_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() >= 2 && tokens[1].trim().eq_ignore_ascii_case("until") {
        return parse_wait_until_line(line_no, tokens);
    }
    if tokens.len() != 2 {
        return Err(operate_line_error(line_no, "wait", "非法：格式应为 `wait <seconds>` 或 `wait until window|element \"值\" [timeout=s]`".to_string()));
    }
    let duration = parse_seconds_token(line_no, "wait", &tokens[1], "seconds")?;
    Ok(DesktopScriptAction::Wait { line: line_no, duration })
}

/// `wait until window "标题"` / `wait until element "名称" [target=...]` [timeout=s]
fn parse_wait_until_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 4 {
        return Err(operate_line_error(line_no, "wait", "非法：格式应为 `wait until window \"标题\"` 或 `wait until element \"名称\"`".to_string()));
    }
    let kind = tokens[2].trim().to_ascii_lowercase();
    let Some(needle) = strip_quoted_value(&tokens[3]) else {
        return Err(operate_line_error(line_no, "wait", "非法：条件值必须用双引号包裹".to_string()));
    };
    if needle.trim().is_empty() {
        return Err(operate_line_error(line_no, "wait", "非法：条件值不能为空".to_string()));
    }
    let params = parse_named_params(line_no, "wait", &tokens[4..], &["timeout", "target"])?;
    let timeout = params
        .get("timeout")
        .map(|v| parse_seconds_token(line_no, "wait", v, "timeout"))
        .transpose()?
        .unwrap_or_else(|| std::time::Duration::from_secs_f64(DEFAULT_WAIT_UNTIL_TIMEOUT_SECS));
    let condition = match kind.as_str() {
        "window" => {
            if params.contains_key("target") {
                return Err(operate_line_error(line_no, "wait", "非法：wait until window 不接受 target".to_string()));
            }
            WaitCondition::Window { title: needle }
        }
        "element" => {
            let window_target = params.get("target").map(|raw| parse_foreground_target(line_no, "wait", raw)).transpose()?;
            WaitCondition::Element { name: needle, window_target }
        }
        other => return Err(operate_line_error(line_no, "wait", format!("未知条件：{other}。可用：window、element"))),
    };
    Ok(DesktopScriptAction::WaitUntil { line: line_no, condition, timeout })
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
                    return Err(operate_line_error(line_no, "screenshot", "非法：focused_window/window_id/window 与 region 不能同时出现".to_string()));
                }
                mode = ScreenshotModeSpec::FocusedWindow;
            }
            other => return Err(operate_line_error(line_no, "screenshot", format!("非法参数 `{other}`"))),
        }
    }
    let params = parse_named_params(line_no, "screenshot", &named_tokens, &["region", "save", "quality", "elements", "text", "window_id", "window", "monitor", "max_pixels"])?;
    if let Some(raw) = params.get("window_id") {
        if !matches!(mode, ScreenshotModeSpec::Desktop) {
            return Err(operate_line_error(line_no, "screenshot", "非法：window_id 与 focused_window/region/window 不能同时出现".to_string()));
        }
        mode = ScreenshotModeSpec::WindowId(parse_window_id_token(line_no, "screenshot", raw)?);
    }
    if let Some(raw) = params.get("window") {
        if !matches!(mode, ScreenshotModeSpec::Desktop) {
            return Err(operate_line_error(line_no, "screenshot", "非法：window 与 focused_window/region/window_id 不能同时出现".to_string()));
        }
        let name = strip_quoted_value(raw).unwrap_or_else(|| raw.trim().to_string());
        if name.trim().is_empty() {
            return Err(operate_line_error(line_no, "screenshot", "非法：window 不能为空，需给\"标题/进程名\"".to_string()));
        }
        mode = ScreenshotModeSpec::WindowName(name);
    }
    if let Some(raw) = params.get("region") {
        if !matches!(mode, ScreenshotModeSpec::Desktop) {
            return Err(operate_line_error(line_no, "screenshot", "非法：focused_window/window_id/window 与 region 不能同时出现".to_string()));
        }
        mode = ScreenshotModeSpec::Region(parse_screen_region(line_no, "screenshot", raw)?);
    }
    if let Some(raw) = params.get("monitor") {
        if !matches!(mode, ScreenshotModeSpec::Desktop) {
            return Err(operate_line_error(line_no, "screenshot", "非法：monitor 与 focused_window/window_id/window/region 不能同时出现".to_string()));
        }
        mode = ScreenshotModeSpec::Monitor(parse_monitor_token(line_no, "screenshot", raw)?);
    }
    let save_path = params.get("save").map(|v| parse_absolute_save_path(line_no, "screenshot", v)).transpose()?;
    let quality = params.get("quality").map(|v| {
        let parsed = v.parse::<f32>().map_err(|_| operate_line_error(line_no, "screenshot", format!("quality 非法：必须是数字，当前为 `{v}`")))?;
        if !(1.0..=100.0).contains(&parsed) {
            return Err(operate_line_error(line_no, "screenshot", format!("quality 非法：必须在 1~100 之间，当前为 `{v}`")));
        }
        Ok(parsed)
    }).transpose()?.unwrap_or(75.0);
    let elements = params.get("elements").map(|v| {
        match v.to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(operate_line_error(line_no, "screenshot", format!("elements 非法：必须是 true 或 false，当前为 `{v}`"))),
        }
    }).transpose()?.unwrap_or(false);
    let include_text = params.get("text").map(|v| {
        match v.to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(operate_line_error(line_no, "screenshot", format!("text 非法：必须是 true 或 false，当前为 `{v}`"))),
        }
    }).transpose()?.unwrap_or(false);
    if include_text && !elements {
        return Err(operate_line_error(line_no, "screenshot", "非法：text=true 需要同时 elements=true，否则没有元素树可以附加文本标签".to_string()));
    }
    let max_pixels = params.get("max_pixels").map(|v| {
        let parsed = v.parse::<u64>().map_err(|_| operate_line_error(line_no, "screenshot", format!("max_pixels 非法：必须是正整数，当前为 `{v}`")))?;
        if !(40_000..=100_000_000).contains(&parsed) {
            return Err(operate_line_error(line_no, "screenshot", format!("max_pixels 非法：必须在 40000~100000000 之间，当前为 `{v}`")));
        }
        Ok(parsed)
    }).transpose()?;
    Ok(DesktopScriptAction::Screenshot { line: line_no, mode, save_path, quality, elements, include_text, max_pixels })
}

/// `clipboard read` / `clipboard write "内容"`（A4）：剪贴板当数据通道用。
/// read 只读不改；write 会覆盖当前剪贴板内容（显式写入，意图明确才用）。
fn parse_clipboard_line(line_no: usize, tokens: &[String]) -> DesktopToolResult<DesktopScriptAction> {
    if tokens.len() < 2 {
        return Err(operate_line_error(line_no, "clipboard", "非法：格式应为 `clipboard read` 或 `clipboard write \"内容\"`".to_string()));
    }
    match tokens[1].trim().to_ascii_lowercase().as_str() {
        "read" => {
            if tokens.len() != 2 {
                return Err(operate_line_error(line_no, "clipboard", "非法：clipboard read 不接受参数".to_string()));
            }
            Ok(DesktopScriptAction::Clipboard { line: line_no, op: ClipboardOp::Read })
        }
        "write" => {
            if tokens.len() != 3 {
                return Err(operate_line_error(line_no, "clipboard", "非法：格式应为 `clipboard write \"内容\"`".to_string()));
            }
            let Some(text) = strip_quoted_value(&tokens[2]) else {
                return Err(operate_line_error(line_no, "clipboard", "非法：必须使用双引号包裹写入内容".to_string()));
            };
            let text = text.replace("\\n", "\n");
            if text.is_empty() {
                return Err(operate_line_error(line_no, "clipboard", "非法：写入内容不能为空".to_string()));
            }
            Ok(DesktopScriptAction::Clipboard { line: line_no, op: ClipboardOp::Write(text) })
        }
        other => Err(operate_line_error(line_no, "clipboard", format!("未知子动作：{other}。可用：read、write"))),
    }
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
        "app" => parse_app_line(line_no, &tokens).map(Some),
        "key" => parse_key_line(line_no, &tokens).map(Some),
        "text" => parse_text_line(line_no, &tokens).map(Some),
        "wait" => parse_wait_line(line_no, &tokens).map(Some),
        "window" => parse_window_line(line_no, &tokens).map(Some),
        "screenshot" => parse_screenshot_line(line_no, &tokens).map(Some),
        "clipboard" => parse_clipboard_line(line_no, &tokens).map(Some),
        other => Err(operate_line_error(line_no, "脚本", format!("未知动作：{other}。可用动作：mouse、key、text、wait、window、screenshot、clipboard；后台操作先声明窗口变量（w = app \"窗口名\"），再用变量动作（w click \"名\"）"))),
    }
}

/// 是否为变量声明行：`w = app ...`（tokens[1] 为孤立 `=` 即可判定，形状由声明解析函数严格校验）
fn is_declare_line(tokens: &[String]) -> bool {
    tokens.len() >= 2 && tokens[1] == "="
}

fn parse_script(request: &OperateRequest) -> DesktopToolResult<Vec<DesktopScriptAction>> {
    let trimmed = request.script.trim();
    if trimmed.is_empty() {
        return Err(operate_invalid("script 不能为空"));
    }
    // 引号感知拆行：双引号内的换行属于字符串内容，引号外的换行才是动作边界。
    // 不能直接用 lines() 裸拆，否则 text "第一行\n第二行" 会被拦腰切开、报引号未闭合。
    let mut lines = Vec::<String>::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in request.script.chars() {
        match ch {
            '"' => {
                current.push(ch);
                in_quotes = !in_quotes;
            }
            '\n' if !in_quotes => {
                lines.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    let mut actions = Vec::<DesktopScriptAction>::new();
    // 脚本内窗口变量表：声明行登记，使用行校验先声明后使用；脚本结束即丢弃，不跨调用
    let mut declared = std::collections::HashSet::<String>::new();
    for (idx, raw_line) in lines.iter().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = tokenize_script_line(trimmed).map_err(|err| operate_line_error(idx + 1, "脚本", err))?;
        if tokens.is_empty() {
            continue;
        }
        let action = if is_declare_line(&tokens) {
            parse_var_declare_line(idx + 1, &tokens, &mut declared)?
        } else if is_script_keyword(&tokens[0]) || !is_var_name(tokens[0].trim()) {
            match parse_script_line(idx + 1, raw_line)? {
                Some(action) => action,
                None => continue,
            }
        } else {
            parse_var_action_line(idx + 1, &tokens, &declared)?
        };
        actions.push(action);
    }
    if actions.is_empty() {
        return Err(operate_invalid("script 不能为空"));
    }
    Ok(actions)
}
