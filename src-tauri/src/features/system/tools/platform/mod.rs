// ==================== 平台抽象层：窗口枚举 / 激活 / 控件树 ====================
//
// 三平台统一数据契约与函数签名，按 cfg 分发到 windows / linux / macos 实现模块。
// Windows 实现由原 windows_tool.rs + ui_automation.rs 平移而来（不重写）；
// Linux 为 xcap 枚举 + xcb 激活 + atspi 控件树；macOS 为 xcap 枚举 + AXUIElement 激活/控件树（手写 FFI，零新增依赖）。
// 未支持平台（FreeBSD 等）走兜底空实现，与既有行为一致。

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

/// 控件树扫描的元素数量上限（三平台一致，防极端桌面卡死扫描）
pub const MAX_ELEMENTS: usize = 500;

// ==================== 数据契约 ====================

/// 单个窗口信息（list windows 返回项）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub window_id: usize,
    pub title: String,
    pub process_id: u32,
    /// 进程可执行名/应用名（G2 名字引用窗口的匹配依据之一，window list 原样返回供模型选用）。
    /// 查不到时为 None（此时只按标题匹配）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub minimized: bool,
    pub focused: bool,
}

/// 单个可交互元素（虚拟桌面物理像素坐标，与 operate mouse @x,y 同坐标系）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiElementInfo {
    pub window_id: u32,
    pub window_title: String,
    pub control_type: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// 该元素当前是否持有键盘焦点（激活窗口后用于确认焦点是否落在目标控件）
    pub focused: bool,
}

/// app 后台动作目标：按元素名寻址，或屏幕物理坐标定位
#[derive(Debug, Clone)]
pub enum AppTarget {
    /// 元素名寻址：ordinal 为执行前新鲜扫描的窗口内序号，报错文案按名字讲，不提编号
    NamedElement { ordinal: usize, control_type: String, name: String },
    Point { screen_x: i32, screen_y: i32 },
}

// ==================== app 后台动作（Windows 专用，其余平台明确报错） ====================

/// 后台点击结果：method 为实际投递方式；downgrade 说明本次是否退化为坐标投递（结果不保证）；
/// hit_control_type 为命中元素的控件类型（走坐标兜底时为 None），供上层判断跳转副作用。
#[derive(Debug, Clone)]
pub struct AppClickOutcome {
    pub method: &'static str,
    pub downgrade: Option<String>,
    pub hit_control_type: Option<&'static str>,
}

/// 后台点击：UIA InvokePattern 优先（dblclick=true 时跳过 Invoke 直接 PostMessage 双击序列），PostMessage 兜底。
#[cfg(target_os = "windows")]
pub fn app_click(window_id: usize, target: &AppTarget, repeat: u32, dblclick: bool) -> Result<AppClickOutcome, String> {
    windows::app_click(window_id, target, repeat, dblclick)
}

/// 后台写值：ValuePattern.SetValue 整体替换文本控件内容。
#[cfg(target_os = "windows")]
pub fn app_set_value(window_id: usize, target: &AppTarget, text: &str) -> Result<&'static str, String> {
    windows::app_set_value(window_id, target, text)
}

/// 后台读值：ValuePattern.CurrentValue 读回文本控件当前值。
#[cfg(target_os = "windows")]
pub fn app_get_value(window_id: usize, target: &AppTarget) -> Result<String, String> {
    windows::app_get_value(window_id, target)
}

/// 后台滚动：ScrollPattern 优先，滚轮消息兜底（垂直 WM_MOUSEWHEEL / 水平 WM_MOUSEHWHEEL）。
/// positive=true 表示 down（垂直）/ right（水平），与 enigo scroll 符号约定一致。返回实际使用的投递方式。
#[cfg(target_os = "windows")]
pub fn app_scroll(window_id: usize, target: &AppTarget, horizontal: bool, positive: bool, small: bool, repeat: u32) -> Result<&'static str, String> {
    windows::app_scroll(window_id, target, horizontal, positive, small, repeat)
}

/// 后台按键：向目标窗口的内部焦点控件投递键盘消息。
#[cfg(target_os = "windows")]
pub fn app_key(window_id: usize, keys: &[String], repeat: u32) -> Result<&'static str, String> {
    windows::app_key(window_id, keys, repeat)
}

/// 按元素名解析窗口内控件（新鲜扫描、唯一命中）：返回 (窗口内序号, 类型, 全名)。
/// 找不到或命中多个时返回带候选的中文说明，供模型自我纠正。
#[cfg(target_os = "windows")]
pub fn app_find_element_by_name(window_id: usize, name: &str) -> Result<(usize, String, String), String> {
    windows::app_find_element_by_name(window_id, name)
}

#[cfg(not(target_os = "windows"))]
pub fn app_find_element_by_name(_window_id: usize, _name: &str) -> Result<(usize, String, String), String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

/// 当前平台的能力降级说明：仅在平台会静默失败或只报底层错误时返回，
/// 供工具把「为什么没有结果」讲清楚，避免模型反复试错（H2）。
#[cfg(target_os = "linux")]
pub fn platform_capability_hint() -> Option<String> {
    linux::platform_capability_hint()
}

#[cfg(not(target_os = "linux"))]
pub fn platform_capability_hint() -> Option<String> {
    None
}

/// 控件树为空时的原因说明（E4/H3）：截图 elements=true 却扫不到元素时，
/// 把「为什么是空」讲清楚，避免模型反复试错。Windows 返回 None（沿用调用方原有文案）。
#[cfg(target_os = "macos")]
pub fn ui_tree_empty_hint() -> Option<String> {
    if macos::accessibility_permission_granted() {
        Some("macOS 控件树为空：目标窗口未暴露无障碍元素（部分自绘/Electron 窗口如此）；请改用坐标点击（mouse click @x,y）".to_string())
    } else {
        Some("macOS 辅助功能权限缺失：请在系统设置 → 隐私与安全性 → 辅助功能中允许本应用，然后重试；未授权前请改用 key/text 操作当前前台".to_string())
    }
}

#[cfg(target_os = "linux")]
pub fn ui_tree_empty_hint() -> Option<String> {
    if linux::is_wayland_session() {
        Some("当前是 Wayland 会话：原生窗口无法枚举、控件树为空（仅 XWayland 窗口可见）；请在 X11 会话下使用，或改用 key/text 直接操作当前前台".to_string())
    } else {
        Some("Linux 控件树为空：需要桌面环境的 AT-SPI2 无障碍服务（GNOME/KDE 默认开启）；缺失时请改用 key/text 操作当前前台".to_string())
    }
}

/// Windows 控件树为空时给模型的提示。
/// 这里刻意不再返回 None：空树有「窗口未暴露 UIA」和「协议层超时被截断」两种可能，
/// 两者的应对方式完全不同（前者换坐标点击、后者重试或缩小范围），只报「为空」会让模型
/// 按错误原因反复重试同一条命令。具体是哪种看日志里的 HRESULT，提示里两条都给。
#[cfg(target_os = "windows")]
pub fn ui_tree_empty_hint() -> Option<String> {
    Some("Windows 控件树为空：可能是目标窗口未暴露 UIA 元素（自绘/Electron/游戏窗口），也可能是目标窗口无响应导致扫描超时被截断；请先用 window activate 确认窗口可响应后重试一次，仍为空则改用坐标点击（mouse click @x,y）或用 window_id/region 缩小扫描范围".to_string())
}

/// 查询目标窗口线程当前内部焦点控件（type, name）；无内部焦点或查询失败返回 None。
#[cfg(target_os = "windows")]
pub fn app_focus_summary(window_id: usize) -> Result<Option<(String, String)>, String> {
    windows::app_focus_summary(window_id)
}

#[cfg(not(target_os = "windows"))]
pub fn app_click(_window_id: usize, _target: &AppTarget, _repeat: u32, _dblclick: bool) -> Result<AppClickOutcome, String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn app_set_value(_window_id: usize, _target: &AppTarget, _text: &str) -> Result<&'static str, String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn app_get_value(_window_id: usize, _target: &AppTarget) -> Result<String, String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn app_scroll(_window_id: usize, _target: &AppTarget, _horizontal: bool, _positive: bool, _small: bool, _repeat: u32) -> Result<&'static str, String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn app_key(_window_id: usize, _keys: &[String], _repeat: u32) -> Result<&'static str, String> {
    Err("app 后台操作当前仅在 Windows 平台可用；其他平台请改用前台动作（mouse/key/text）并先 window activate 把目标窗口切到前台".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn app_focus_summary(_window_id: usize) -> Result<Option<(String, String)>, String> {
    Ok(None)
}

// ==================== 平台函数入口（按 cfg 分发） ====================

/// 全量枚举顶层可见窗口（含当前进程窗口）。返回按 z-order 排序的窗口列表。
#[cfg(target_os = "windows")]
pub fn list_all_windows() -> Vec<WindowInfo> {
    windows::list_all_windows()
}

/// 激活窗口到前台。返回 (窗口标题, 是否激活成功)。
#[cfg(target_os = "windows")]
pub fn activate_window(window_id: usize) -> (String, bool) {
    windows::activate_window(window_id)
}

/// 窗口句柄是否仍然存在（含被隐藏的窗口）。
#[cfg(target_os = "windows")]
pub fn window_is_alive(window_id: usize) -> bool {
    windows::window_is_alive(window_id)
}

#[cfg(not(target_os = "windows"))]
pub fn window_is_alive(window_id: usize) -> bool {
    list_all_windows().iter().any(|w| w.window_id == window_id)
}

/// 批量扫描指定窗口列表的可交互元素树（一次 COM/automation 实例复用）。
/// windows 为 (平台窗口 id, 窗口标题) 列表；返回扁平元素列表（虚拟桌面物理像素坐标）。
#[cfg(target_os = "windows")]
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    include_text: bool,
) -> Vec<UiElementInfo> {
    windows::collect_ui_tree_for_windows(windows, include_text)
}

/// 对指定窗口扫描可交互元素树；窗口 id 为 0 时忽略。
#[cfg(target_os = "windows")]
pub fn collect_window_ui_elements(window_id: usize, include_text: bool) -> Vec<UiElementInfo> {
    windows::collect_window_ui_elements(window_id, include_text)
}

/// 绝对鼠标移动，坐标为虚拟桌面物理像素（Windows 用 SendInput + MOUSEEVENTF_VIRTUALDESK）。
#[cfg(target_os = "windows")]
pub fn mouse_move_absolute(x: i32, y: i32) -> Result<(), String> {
    windows::mouse_move_absolute(x, y)
}

#[cfg(target_os = "linux")]
pub fn list_all_windows() -> Vec<WindowInfo> {
    linux::list_all_windows()
}

#[cfg(target_os = "linux")]
pub fn activate_window(window_id: usize) -> (String, bool) {
    linux::activate_window(window_id)
}

#[cfg(target_os = "linux")]
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    include_text: bool,
) -> Vec<UiElementInfo> {
    linux::collect_ui_tree_for_windows(windows, include_text)
}

#[cfg(target_os = "linux")]
pub fn collect_window_ui_elements(window_id: usize, include_text: bool) -> Vec<UiElementInfo> {
    linux::collect_window_ui_elements(window_id, include_text)
}

#[cfg(target_os = "macos")]
pub fn list_all_windows() -> Vec<WindowInfo> {
    macos::list_all_windows()
}

#[cfg(target_os = "macos")]
pub fn activate_window(window_id: usize) -> (String, bool) {
    macos::activate_window(window_id)
}

#[cfg(target_os = "macos")]
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    include_text: bool,
) -> Vec<UiElementInfo> {
    macos::collect_ui_tree_for_windows(windows, include_text)
}

#[cfg(target_os = "macos")]
pub fn collect_window_ui_elements(window_id: usize, include_text: bool) -> Vec<UiElementInfo> {
    macos::collect_window_ui_elements(window_id, include_text)
}

// ==================== 未支持平台兜底（FreeBSD 等） ====================

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn list_all_windows() -> Vec<WindowInfo> {
    Vec::new()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn activate_window(_window_id: usize) -> (String, bool) {
    (String::new(), false)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn collect_ui_tree_for_windows(
    _windows: &[(usize, String)],
    _include_text: bool,
) -> Vec<UiElementInfo> {
    Vec::new()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn collect_window_ui_elements(_window_id: usize, _include_text: bool) -> Vec<UiElementInfo> {
    Vec::new()
}
