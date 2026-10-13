// ==================== macOS 平台实现 ====================
//
// 仅在 macOS 编译（platform/mod.rs 按 cfg 引入）。包含：
// - list windows：复用 xcap window_list()（CGWindowListCopyWindowInfo 全量枚举）
// - activate window：AXRaise（单窗口精确提升）+ NSRunningApplication（应用前台）+ 轮询验证
// - 控件树：AXUIElement 遍历（防环去重 + 角色白名单 + 屏幕物理像素坐标）
//
// 平台固有限制（在工具描述中提示）：
// - 控件树 / 激活需要「辅助功能」权限（AXIsProcessTrusted 检测，缺失时属性读取 kAXErrorAPIDisabled）
// - list windows 的标题需要「屏幕录制」权限（无权限时 kCGWindowName 为空）
//
// FFI 采用手写绑定（参考 lingxia-computer-use，MIT）：AXUIElement 是 CFType 而非 ObjC 类，
// objc2 生态无高层绑定；所需函数集固定（AX 核心 5 个 + CF 辅助 9 个）。
// libc / objc2-core-foundation / objc2-app-kit 为直接依赖（均已声明在 Cargo.toml macos target 段，
// 且原本就在 xcap 的传递依赖树中，不新增编译单元）。

use super::{MAX_ELEMENTS, UiElementInfo, WindowInfo};
use objc2_core_foundation::{CFRetained, CFString, CGPoint, CGSize};
use std::ffi::c_void;
use std::ptr;

// ==================== AX FFI（ApplicationServices 框架） ====================

/// AX 树遍历深度上限（防止异常深树卡死；与 linux MAX_TREE_DEPTH 对齐的量级）。
const MAX_TREE_DEPTH: usize = 32;

// AXValueType 原始值（CoreGraphics/AXValue.h）
const AX_VALUE_CGPOINT: u32 = 1;
const AX_VALUE_CGSIZE: u32 = 2;

// AXError 值（HIServices/AXError.h）：0 表示成功
const AX_SUCCESS: i32 = 0;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> *mut c_void;
    fn AXUIElementCopyAttributeValue(
        element: *mut c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn AXUIElementPerformAction(element: *mut c_void, action: *const c_void) -> i32;
    // Apple 头文件中这两者返回 Boolean（unsigned char），不能用 Rust bool 直接承接 FFI 值
    fn AXIsProcessTrusted() -> u8;
    fn AXValueGetValue(value: *const c_void, the_type: u32, value_ptr: *mut c_void) -> u8;
    // 私有但长期稳定：把 AX 窗口元素映射回 CGWindowID，
    // 是 CGWindowList（xcap）与 AX 视图之间唯一可靠桥（坐标匹配在窗口完全重叠时歧义）。
    fn _AXUIElementGetWindow(element: *mut c_void, out: *mut u32) -> i32;
}

// CoreFoundation 基础函数：objc2-core-foundation 未导出为自由函数，按需手写声明
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CFRelease(cf: *const c_void);
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(boolean: *const c_void) -> u8;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(the_array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(the_array: *const c_void, idx: isize) -> *const c_void;
    /// 判断两个 CF 对象是否等价（AX 元素按应用+元素标识比较，见 CFEqual 语义）。
    /// 返回 Boolean（unsigned char）。
    fn CFEqual(cf1: *const c_void, cf2: *const c_void) -> u8;
}

/// 属性名常量（AX 稳定字符串，随用随建 CFString）
const ATTR_ROLE: &str = "AXRole";
const ATTR_TITLE: &str = "AXTitle";
const ATTR_DESCRIPTION: &str = "AXDescription";
const ATTR_ROLE_DESCRIPTION: &str = "AXRoleDescription";
const ATTR_ENABLED: &str = "AXEnabled";
const ATTR_POSITION: &str = "AXPosition";
const ATTR_SIZE: &str = "AXSize";
const ATTR_WINDOWS: &str = "AXWindows";
const ATTR_CHILDREN: &str = "AXChildren";
const ATTR_FOCUSED: &str = "AXFocused";
const ACTION_RAISE: &str = "AXRaise";

// ==================== AXUIElement 封装 ====================

/// 持有 +1 引用计数的 AXUIElementRef（Drop 时 CFRelease）。
struct AxEl(*mut c_void);

impl AxEl {
    /// 按 pid 创建应用级 accessibility 对象（成功即持有 +1；无权限时属性读取失败而非创建失败）。
    fn for_app(pid: libc::pid_t) -> Option<AxEl> {
        let raw = unsafe { AXUIElementCreateApplication(pid) };
        if raw.is_null() {
            None
        } else {
            Some(AxEl(raw))
        }
    }

    /// 复制属性为 owned CF 值（+1，CfValue 负责释放）。失败/缺属性返回 None。
    fn copy(&self, attr: &str) -> Option<CfValue> {
        let key = CFString::from_str(attr);
        let mut value: *const c_void = ptr::null();
        // SAFETY: AXUIElementCopyAttributeValue 按 CF 约定输出 +1 值，调用方负责释放
        let rc = unsafe {
            AXUIElementCopyAttributeValue(
                self.0,
                CFRetained::as_ptr(&key).as_ptr().cast::<c_void>(),
                &mut value,
            )
        };
        if rc == AX_SUCCESS && !value.is_null() {
            Some(CfValue(value))
        } else {
            None
        }
    }

    fn attr_string(&self, attr: &str) -> Option<String> {
        let v = self.copy(attr)?;
        unsafe {
            if CFGetTypeID(v.0) != CFStringGetTypeID() {
                return None;
            }
            Some((&*(v.0 as *const CFString)).to_string())
        }
    }

    fn attr_bool(&self, attr: &str) -> Option<bool> {
        let v = self.copy(attr)?;
        unsafe {
            if CFGetTypeID(v.0) != CFBooleanGetTypeID() {
                return None;
            }
            Some(CFBooleanGetValue(v.0) != 0)
        }
    }

    fn attr_point(&self, attr: &str) -> Option<CGPoint> {
        let v = self.copy(attr)?;
        let mut p = CGPoint { x: 0.0, y: 0.0 };
        // SAFETY: AXValueGetValue 按指定类型解包到调用方缓冲区
        let ok = unsafe { AXValueGetValue(v.0, AX_VALUE_CGPOINT, &mut p as *mut _ as *mut c_void) };
        (ok != 0).then_some(p)
    }

    fn attr_size(&self, attr: &str) -> Option<CGSize> {
        let v = self.copy(attr)?;
        let mut s = CGSize {
            width: 0.0,
            height: 0.0,
        };
        // SAFETY: 同上，AXValueGetValue 解包 CGSize
        let ok = unsafe { AXValueGetValue(v.0, AX_VALUE_CGSIZE, &mut s as *mut _ as *mut c_void) };
        (ok != 0).then_some(s)
    }

    /// 子 AX 元素（AXChildren）。返回的每个元素都 +1 持有。
    fn children(&self) -> Vec<AxEl> {
        self.array_elements(ATTR_CHILDREN)
    }

    /// 应用元素的窗口列表（AXWindows；mac 的窗口不挂在 AXChildren 下）。
    fn windows(&self) -> Vec<AxEl> {
        self.array_elements(ATTR_WINDOWS)
    }

    /// 从属性值为 CFArray 的集合中逐个取出元素并 +1 持有。
    fn array_elements(&self, attr: &str) -> Vec<AxEl> {
        let Some(v) = self.copy(attr) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            if CFGetTypeID(v.0) != CFArrayGetTypeID() {
                return out;
            }
            let count = CFArrayGetCount(v.0).max(0);
            for i in 0..count {
                let el = CFArrayGetValueAtIndex(v.0, i);
                if !el.is_null() {
                    CFRetain(el);
                    out.push(AxEl(el as *mut c_void));
                }
            }
        }
        out
    }

    /// 窗口元素的 CGWindowID（私有 API 桥；非窗口或失败返回 None）。
    fn window_id(&self) -> Option<u32> {
        let mut id: u32 = 0;
        // SAFETY: _AXUIElementGetWindow 输出 u32 到调用方缓冲区
        let rc = unsafe { _AXUIElementGetWindow(self.0, &mut id) };
        (rc == AX_SUCCESS && id != 0).then_some(id)
    }

    /// 对元素执行 AX 动作（如 AXRaise）。
    fn perform(&self, action: &str) -> bool {
        let a = CFString::from_str(action);
        // SAFETY: AXUIElementPerformAction 同步执行，动作名为临时 CFString
        unsafe {
            AXUIElementPerformAction(self.0, CFRetained::as_ptr(&a).as_ptr().cast::<c_void>())
                == AX_SUCCESS
        }
    }
}

impl Drop for AxEl {
    fn drop(&mut self) {
        // SAFETY: AxEl 始终持有 +1 引用
        unsafe { CFRelease(self.0 as *const c_void) };
    }
}

/// 持有 +1 引用计数的任意 CF 值（属性读取产物，Drop 时释放）。
struct CfValue(*const c_void);

impl Drop for CfValue {
    fn drop(&mut self) {
        // SAFETY: CfValue 由 AXUIElementCopyAttributeValue 输出，+1 归本对象
        unsafe { CFRelease(self.0) };
    }
}

// ==================== list windows（复用 xcap） ====================

/// 辅助功能权限是否已授予（E4/H3 空树提示用；macOS 的 AX 读取与窗口激活都依赖它）。
pub fn accessibility_permission_granted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn list_all_windows() -> Vec<WindowInfo> {
    let Ok(windows) = xcap::Window::all() else {
        return Vec::new();
    };
    windows
        .iter()
        .map(|w| {
            let app_name = w.app_name().unwrap_or_default();
            WindowInfo {
                window_id: w.id().unwrap_or(0) as usize,
                title: w.title().unwrap_or_default(),
                process_name: if app_name.is_empty() { None } else { Some(app_name) },
                process_id: w.pid().unwrap_or(0),
                x: w.x().unwrap_or(0),
                y: w.y().unwrap_or(0),
                width: w.width().unwrap_or(0) as i32,
                height: w.height().unwrap_or(0) as i32,
                minimized: w.is_minimized().unwrap_or(false),
                focused: w.is_focused().unwrap_or(false),
            }
        })
        .collect()
}

// ==================== activate window ====================

pub fn activate_window(window_id: usize) -> (String, bool) {
    // 辅助功能权限缺失时 AX 操作必然失败，直接返回明确失败
    if unsafe { AXIsProcessTrusted() } == 0 {
        return (String::new(), false);
    }
    let Ok(windows) = xcap::Window::all() else {
        return (String::new(), false);
    };
    let Some(win) = windows.iter().find(|w| w.id().unwrap_or(0) == window_id as u32) else {
        return (String::new(), false);
    };
    let title = win.title().unwrap_or_else(|_| window_id.to_string());
    let pid = win.pid().unwrap_or(0) as libc::pid_t;
    if pid <= 0 {
        return (title, false);
    }

    // 1) AXRaise 精确提升目标窗口（只动这一个窗口）
    let mut raised = false;
    if let Some(app) = AxEl::for_app(pid) {
        for w in app.windows() {
            if w.window_id() == Some(window_id as u32) {
                raised = w.perform(ACTION_RAISE);
                break;
            }
        }
    }

    // 2) NSRunningApplication 把应用拉到前台（AXRaise 只提升窗口到应用内前部）
    //    ActivateIgnoringOtherApps 在 macOS 14+ 已废弃无效果（Apple 官方行为变更），
    //    但 macOS 13 及更早是强制抢占前台的唯一途径，两个 flag 叠加覆盖新旧系统。
    let activated = unsafe {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .map(|app| {
                #[allow(deprecated)]
                app.activateWithOptions(
                    NSApplicationActivationOptions::ActivateAllWindows
                        | NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                )
            })
            .unwrap_or(false)
    };

    // 3) 轮询 isActive 验证（1.5s 上限）
    let mut ok = false;
    if activated || raised {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        while std::time::Instant::now() < deadline {
            let active = unsafe {
                use objc2_app_kit::NSRunningApplication;
                NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
                    .is_some_and(|app| app.isActive())
            };
            if active {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    (title, ok)
}

// ==================== 控件树 ====================

/// 批量扫描指定窗口列表的可交互元素树。
/// windows 为 (xcap window_id, 窗口标题) 列表；返回扁平元素列表（屏幕物理像素坐标）。
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    _include_text: bool,
) -> Vec<UiElementInfo> {
    if windows.is_empty() {
        return Vec::new();
    }
    // 控件树读取需要辅助功能权限；缺失时直接返回空（summary 由 operate 侧提示）
    if unsafe { AXIsProcessTrusted() } == 0 {
        return Vec::new();
    }
    let Ok(all) = xcap::Window::all() else {
        return Vec::new();
    };
    let mut out: Vec<UiElementInfo> = Vec::new();
    for (wid, title) in windows {
        if out.len() >= MAX_ELEMENTS {
            break;
        }
        // 窗口 → pid → 应用 AX 对象 → 匹配 AX 窗口
        let Some(pid) = all
            .iter()
            .find(|w| w.id().unwrap_or(0) == *wid as u32)
            .and_then(|w| w.pid().ok())
        else {
            continue;
        };
        let Some(app) = AxEl::for_app(pid as libc::pid_t) else {
            continue;
        };
        // _AXUIElementGetWindow 精确匹配优先，坐标兜底（与 linux match_window 同构）
        let mut matched: Option<AxEl> = None;
        for w in app.windows() {
            if w.window_id() == Some(*wid as u32) {
                matched = Some(w);
                break;
            }
        }
        if matched.is_none() {
            for w in app.windows() {
                if let (Some(p), Some(s)) = (w.attr_point(ATTR_POSITION), w.attr_size(ATTR_SIZE)) {
                    let Some(xw) = all.iter().find(|xw| xw.id().unwrap_or(0) == *wid as u32) else {
                        continue;
                    };
                    let (x, y, ww, wh) = (
                        xw.x().unwrap_or(0) as f64,
                        xw.y().unwrap_or(0) as f64,
                        xw.width().unwrap_or(0) as f64,
                        xw.height().unwrap_or(0) as f64,
                    );
                    // 中心点距离 < 10 视为同一窗口
                    let (acx, acy) = (p.x + s.width / 2.0, p.y + s.height / 2.0);
                    if (acx - (x + ww / 2.0)).abs() < 10.0 && (acy - (y + wh / 2.0)).abs() < 10.0 {
                        matched = Some(w);
                        break;
                    }
                }
            }
        }
        if let Some(root) = matched {
            scan_elements(&root, *wid as u32, title, &mut out, 0, &mut Vec::new());
        }
    }
    out.truncate(MAX_ELEMENTS);
    out
}

/// 对指定窗口扫描可交互元素树；窗口 id 为 0 时忽略。
pub fn collect_window_ui_elements(window_id: usize, include_text: bool) -> Vec<UiElementInfo> {
    if window_id == 0 {
        return Vec::new();
    }
    collect_ui_tree_for_windows(&[(window_id, String::new())], include_text)
}

/// 递归收集窗口内可交互元素。
/// visited 按 CFEqual 等价性去重——AX 树是图不是树，同一元素可挂多个父，防死循环。
/// 遍历期间所有进入 visited 的元素都被调用方的 AxEl 持有 +1 引用，地址不会被释放复用。
fn scan_elements(
    node: &AxEl,
    window_id: u32,
    window_title: &str,
    out: &mut Vec<UiElementInfo>,
    depth: usize,
    visited: &mut Vec<*const c_void>,
) {
    if depth > MAX_TREE_DEPTH || out.len() >= MAX_ELEMENTS {
        return;
    }
    // AX 树防环：同一 AX 元素（CFEqual 等价）只访问一次。
    // 用 CFEqual 而非裸指针比较：同一元素可能由不同地址的 CF 对象承载（AX 返回包装对象时），
    // 指针比较会漏判导致重复遍历。
    if visited.iter().any(|&prev| unsafe { CFEqual(prev, node.0) != 0 }) {
        return;
    }
    visited.push(node.0);
    let role = node.attr_string(ATTR_ROLE);
    if let Some(role) = &role {
        if is_interactive_role(role) {
            let enabled = node.attr_bool(ATTR_ENABLED).unwrap_or(true);
            if enabled {
                if let (Some(p), Some(s)) = (
                    node.attr_point(ATTR_POSITION),
                    node.attr_size(ATTR_SIZE),
                ) {
                    if s.width > 0.0 && s.height > 0.0 {
                        let name = node
                            .attr_string(ATTR_TITLE)
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                node.attr_string(ATTR_DESCRIPTION).filter(|s| !s.is_empty())
                            })
                            .or_else(|| {
                                node.attr_string(ATTR_ROLE_DESCRIPTION)
                                    .filter(|s| !s.is_empty())
                            })
                            .unwrap_or_default();
                        out.push(UiElementInfo {
                            window_id,
                            window_title: window_title.to_string(),
                            control_type: role_name(role).to_string(),
                            name,
                            x: p.x.round() as i32,
                            y: p.y.round() as i32,
                            width: s.width.round() as i32,
                            height: s.height.round() as i32,
                            focused: node.attr_bool(ATTR_FOCUSED).unwrap_or(false),
                        });
                    }
                }
            }
        }
    }
    let children = node.children();
    for child in children {
        if out.len() >= MAX_ELEMENTS {
            return;
        }
        scan_elements(&child, window_id, window_title, out, depth + 1, visited);
    }
}

/// 可交互角色白名单（AXRole 字符串 → 与 Windows UIA / Linux 语义对齐）。
/// 覆盖 macOS 常见可交互角色；容器角色（AXGroup/AXTabGroup/AXTree/AXWindow 等）不算。
fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "AXButton"
            | "AXMenuButton"
            | "AXDisclosureTriangle"
            | "AXTextField"
            | "AXTextArea"
            | "AXSearchField"
            | "AXCheckBox"
            | "AXComboBox"
            | "AXPopUpButton"
            | "AXRadioButton"
            | "AXMenuItem"
            | "AXMenuBarItem"
            | "AXSlider"
            | "AXStepper"
            | "AXLink"
            | "AXColorWell"
            | "AXRow"
            | "AXCell"
    )
}

/// AXRole → 可读名称（与 Windows UIA 的 control_type 命名对齐，模型侧不感知平台差异）。
fn role_name(role: &str) -> &'static str {
    match role {
        "AXButton" | "AXMenuButton" | "AXDisclosureTriangle" => "Button",
        "AXTextField" | "AXTextArea" | "AXSearchField" => "Edit",
        "AXCheckBox" => "CheckBox",
        "AXComboBox" | "AXPopUpButton" => "ComboBox",
        "AXRadioButton" => "RadioButton",
        "AXMenuItem" | "AXMenuBarItem" => "MenuItem",
        "AXSlider" | "AXStepper" => "Slider",
        "AXLink" => "Hyperlink",
        "AXColorWell" => "ColorWell",
        "AXRow" | "AXCell" => "ListItem",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod macos_platform_tests {
    use super::*;
    // eprintln! 宏在 main.rs:29 被重定义为 runtime_log_info（crate root）
    use crate::runtime_log_info;

    #[test]
    fn interactive_role_whitelist_should_match_expected_roles() {
        assert!(is_interactive_role("AXButton"));
        assert!(is_interactive_role("AXMenuButton"));
        assert!(is_interactive_role("AXDisclosureTriangle"));
        assert!(is_interactive_role("AXTextField"));
        assert!(is_interactive_role("AXTextArea"));
        assert!(is_interactive_role("AXSearchField"));
        assert!(is_interactive_role("AXCheckBox"));
        assert!(is_interactive_role("AXRadioButton"));
        assert!(is_interactive_role("AXSlider"));
        assert!(is_interactive_role("AXStepper"));
        assert!(is_interactive_role("AXLink"));
        assert!(is_interactive_role("AXColorWell"));
        assert!(!is_interactive_role("AXWindow"));
        assert!(!is_interactive_role("AXStaticText"));
        assert!(!is_interactive_role("AXGroup"));
        assert!(!is_interactive_role("AXTabGroup"));
        assert!(!is_interactive_role("AXTree"));
    }

    #[test]
    fn role_name_should_map_known_roles() {
        assert_eq!(role_name("AXButton"), "Button");
        assert_eq!(role_name("AXMenuButton"), "Button");
        assert_eq!(role_name("AXDisclosureTriangle"), "Button");
        assert_eq!(role_name("AXTextField"), "Edit");
        assert_eq!(role_name("AXTextArea"), "Edit");
        assert_eq!(role_name("AXSearchField"), "Edit");
        assert_eq!(role_name("AXCheckBox"), "CheckBox");
        assert_eq!(role_name("AXPopUpButton"), "ComboBox");
        assert_eq!(role_name("AXMenuItem"), "MenuItem");
        assert_eq!(role_name("AXMenuBarItem"), "MenuItem");
        assert_eq!(role_name("AXSlider"), "Slider");
        assert_eq!(role_name("AXStepper"), "Slider");
        assert_eq!(role_name("AXLink"), "Hyperlink");
        assert_eq!(role_name("AXColorWell"), "ColorWell");
        assert_eq!(role_name("AXRow"), "ListItem");
        assert_eq!(role_name("AXUnknown"), "Unknown");
    }

    #[test]
    fn zero_inputs_should_return_empty() {
        assert!(collect_window_ui_elements(0, 0.0, 0.0, 1920.0, 1080.0, false).is_empty());
        assert!(collect_ui_tree_for_windows(&[], 0.0, 0.0, 1920.0, 1080.0, false).is_empty());
    }

    /// 真实桌面冒烟测试：枚举窗口 + 激活前台 + 控件树扫描。
    /// 依赖真实 macOS 桌面 + 辅助功能权限，默认忽略，手动 `--ignored` 跑。
    #[test]
    #[ignore = "需要真实 macOS 桌面（辅助功能 + 屏幕录制权限）"]
    fn real_desktop_should_list_activate_and_scan() {
        let windows = list_all_windows();
        assert!(!windows.is_empty(), "macOS 桌面应有可见窗口");
        eprintln!("[probe] total windows: {}", windows.len());
        for w in windows.iter().take(5) {
            eprintln!(
                "[probe] id={} title={:?} pid={} minimized={} focused={} rect=({},{},{},{})",
                w.window_id, w.title, w.process_id, w.minimized, w.focused, w.x, w.y, w.width, w.height
            );
        }
        // 激活前台窗口：应成功
        if let Some(fg) = windows.iter().find(|w| w.focused) {
            let (title, activated) = activate_window(fg.window_id);
            eprintln!(
                "[probe] activate focused window {}: title={:?} activated={}",
                fg.window_id, title, activated
            );
            assert!(activated, "激活前台窗口应成功");
        }
        // 控件树扫描（权限缺失/应用不暴露时为空）
        let targets: Vec<(usize, String)> = windows
            .iter()
            .take(3)
            .map(|w| (w.window_id, w.title.clone()))
            .collect();
        let elems = collect_ui_tree_for_windows(&targets, 0.0, 0.0, 1920.0, 1080.0, false);
        eprintln!(
            "[probe] scanned {} elements from {} windows",
            elems.len(),
            targets.len()
        );
        for e in elems.iter().take(5) {
            eprintln!("  {}({}) at {}x{}", e.control_type, e.name, e.x, e.y);
        }
    }
}