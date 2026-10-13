// ==================== Linux 平台实现 ====================
//
// 仅在 Linux 编译（platform/mod.rs 按 cfg 引入）。包含：
// - list windows：复用 xcap window_list()（EWMH _NET_CLIENT_LIST_STACKING 全量枚举，无同进程过滤）
// - activate window：xcb 原生发 _NET_ACTIVE_WINDOW ClientMessage（零外部依赖）+ 轮询 root 属性验证
// - 控件树：atspi（AT-SPI2 over D-Bus，与 zbus 同生态），异步 API 在独立线程 + 独立 tokio runtime 内 block_on
//
// 平台固有限制（在工具描述中提示）：
// - Wayland 原生窗口不支持 EWMH 激活与 xcap 窗口枚举（仅 XWayland 窗口可见），返回空/失败并在 summary 说明
// - 控件树依赖桌面环境 AT-SPI2 服务（GNOME/KDE 默认开启），缺失时返回空

use super::{MAX_ELEMENTS, UiElementInfo, WindowInfo};
use atspi::proxy::accessible::ObjectRefExt;
use std::future::Future;
use std::pin::Pin;

// ==================== 能力降级说明（H2） ====================

/// Wayland 会话下 xcap 只能看到 XWayland 窗口，原生窗口枚举会返回空、截图会失败，
/// 且底层错误不会说明原因。这里把限制讲清楚，让模型直接换路而不是反复试。
pub fn platform_capability_hint() -> Option<String> {
    if is_wayland_session() {
        Some("当前是 Wayland 会话：原生窗口无法枚举、截图可能失败（仅 XWayland 窗口可见）；请在 X11 会话下使用，或改用 key/text 直接操作当前前台".to_string())
    } else {
        None
    }
}

pub fn is_wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|value| value.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

// ==================== list windows（复用 xcap） ====================

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

// ==================== activate window（xcb _NET_ACTIVE_WINDOW） ====================

/// 激活窗口：xcb 向 root 发 `_NET_ACTIVE_WINDOW` ClientMessage → 轮询验证。
/// 返回 (窗口标题, 是否激活成功)。X11 窗口管理器支持 EWMH 即生效。
pub fn activate_window(window_id: usize) -> (String, bool) {
    let window = window_id as u32;
    let title = window_title_of(window);
    let (conn, screen_idx) = match xcb::Connection::connect(None) {
        Ok(v) => v,
        Err(_) => return (title, false),
    };
    let setup = conn.get_setup();
    let Some(screen) = setup.roots().nth(screen_idx as usize) else {
        return (title, false);
    };
    let root = screen.root();
    let net_active = match get_atom(&conn, "_NET_ACTIVE_WINDOW") {
        Ok(a) => a,
        Err(_) => return (title, false),
    };

    // EWMH 规范：_NET_ACTIVE_WINDOW 是发给 root 的 ClientMessage，
    // data32 = [source_indication(1=application), window, timestamp(0=CurrentTime), 0, 0]
    let event = xcb::x::ClientMessageEvent::new(
        xcb::XidNew::new(window),
        net_active,
        xcb::x::ClientMessageData::Data32([1, window, 0, 0, 0]),
    );
    let cookie = conn.send_request_checked(&xcb::x::SendEvent {
        propagate: false,
        destination: xcb::x::SendEventDest::Window(root),
        event_mask: xcb::x::EventMask::SUBSTRUCTURE_NOTIFY
            | xcb::x::EventMask::SUBSTRUCTURE_REDIRECT,
        event: &event,
    });
    if conn.check_request(cookie).is_err() || conn.flush().is_err() {
        return (title, false);
    }

    // 轮询 root 的 _NET_ACTIVE_WINDOW 属性验证（最多 1.5s）
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    while std::time::Instant::now() < deadline {
        if let Ok(reply) = get_window_property(&conn, root, net_active) {
            if reply.value::<u32>().first().copied() == Some(window) {
                return (title, true);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    (title, false)
}

/// 从 xcap 重新枚举找窗口标题（激活流程仅用于 summary 展示，失败返回空）。
fn window_title_of(window_id: u32) -> String {
    let Ok(windows) = xcap::Window::all() else {
        return String::new();
    };
    windows
        .iter()
        .find(|w| w.id().unwrap_or(0) == window_id)
        .and_then(|w| w.title().ok())
        .unwrap_or_default()
}

fn get_atom(conn: &xcb::Connection, name: &str) -> Result<xcb::x::Atom, xcb::Error> {
    let cookie = conn.send_request(&xcb::x::InternAtom {
        only_if_exists: false,
        name: name.as_bytes(),
    });
    Ok(conn.wait_for_reply(cookie)?.atom())
}

fn get_window_property(
    conn: &xcb::Connection,
    window: xcb::x::Window,
    property: xcb::x::Atom,
) -> Result<xcb::x::GetPropertyReply, xcb::Error> {
    let cookie = conn.send_request(&xcb::x::GetProperty {
        delete: false,
        window,
        property,
        r#type: xcb::x::ATOM_NONE,
        long_offset: 0,
        long_length: 4,
    });
    conn.wait_for_reply(cookie)
}

// ==================== 控件树（atspi） ====================

/// 递归深度上限（防异常深树爆栈/卡死）
const MAX_TREE_DEPTH: usize = 20;

/// 批量扫描指定窗口列表的可交互元素树。
/// windows 为 (xcap window id, 窗口标题) 列表；返回扁平元素列表（虚拟桌面物理像素坐标）。
pub fn collect_ui_tree_for_windows(
    windows: &[(usize, String)],
    _include_text: bool,
) -> Vec<UiElementInfo> {
    if windows.is_empty() {
        return Vec::new();
    }
    let windows = windows.to_vec();
    // atspi 是异步 API（zbus），而工具执行在 tokio async 上下文内以同步方式调用本函数；
    // 独立线程 + 独立 runtime 隔离，避免与调用方 runtime 冲突（不能在同一 runtime 内再 block_on）。
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return Vec::new(),
        };
        rt.block_on(async move {
            scan_atspi(&windows).await
        })
    })
    .join()
    .unwrap_or_default()
}

/// 对指定窗口扫描可交互元素树；窗口 id 为 0 时忽略。
pub fn collect_window_ui_elements(window_id: usize, include_text: bool) -> Vec<UiElementInfo> {
    if window_id == 0 {
        return Vec::new();
    }
    collect_ui_tree_for_windows(&[(window_id, String::new())], include_text)
}

async fn scan_atspi(
    windows: &[(usize, String)],
) -> Vec<UiElementInfo> {
    let conn = match atspi::AccessibilityConnection::new().await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let root = match conn.root_accessible_on_registry().await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let apps = match root.get_children().await {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    // xcap 窗口列表只枚举一次，供所有 AT-SPI 窗口的坐标兜底匹配复用
    let xcap_windows = xcap::Window::all().ok();
    let mut all: Vec<UiElementInfo> = Vec::new();
    'apps: for app_ref in apps {
        let app = match app_ref.into_accessible_proxy(conn.connection()).await {
            Ok(p) => p,
            Err(_) => continue,
        };
        let children = match app.get_children().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        for win_ref in children {
            let win = match win_ref.into_accessible_proxy(conn.connection()).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            // 窗口对象 → 输入窗口列表匹配（x11_window 属性优先，屏幕坐标兜底）
            let Some((matched_id, matched_title)) =
                match_window(&win, windows, conn.connection(), xcap_windows.as_deref()).await
            else {
                continue;
            };
            collect_elements(
                &win,
                conn.connection(),
                matched_id as u32,
                matched_title,
                &mut all,
                0,
            )
            .await;
            if all.len() >= MAX_ELEMENTS {
                break 'apps;
            }
        }
    }
    all.truncate(MAX_ELEMENTS);
    all
}

/// 把 AT-SPI 窗口对象映射到输入窗口列表。
/// 优先用 `x11_window` 属性（GTK/ATK 等提供）匹配 xcap window id；缺失时用屏幕坐标兜底匹配。
/// xcap 窗口列表由调用方枚举一次传入（坐标兜底复用，避免每个窗口重复全量枚举）。
async fn match_window<'a>(
    win: &atspi::proxy::accessible::AccessibleProxy<'_>,
    windows: &'a [(usize, String)],
    conn: &atspi::zbus::Connection,
    xcap_windows: Option<&[xcap::Window]>,
) -> Option<(usize, &'a str)> {
    if let Ok(attrs) = win.get_attributes().await {
        if let Some(raw) = attrs.get("x11_window") {
            if let Ok(id) = raw.parse::<u32>() {
                if let Some((wid, title)) = windows.iter().find(|(w, _)| *w == id as usize) {
                    return Some((*wid, title.as_str()));
                }
            }
        }
    }
    // 坐标兜底：AT-SPI 屏幕坐标与 xcap 窗口矩形中心点距离 < 10px 视为同一窗口
    let comp = match component_proxy(win, conn).await {
        Some(c) => c,
        None => return None,
    };
    let Ok((ax, ay, aw, ah)) = comp.get_extents(atspi::CoordType::Screen).await else {
        return None;
    };
    if aw <= 0 || ah <= 0 {
        return None;
    }
    let Some(all) = xcap_windows else {
        return None;
    };
    let (acx, acy) = (ax as f64 + aw as f64 / 2.0, ay as f64 + ah as f64 / 2.0);
    for (wid, title) in windows {
        let Some(xw) = all.iter().find(|w| w.id().unwrap_or(0) == *wid as u32) else {
            continue;
        };
        let (x, y, w, h) = (
            xw.x().unwrap_or(0) as f64,
            xw.y().unwrap_or(0) as f64,
            xw.width().unwrap_or(0) as f64,
            xw.height().unwrap_or(0) as f64,
        );
        if (acx - (x + w / 2.0)).abs() < 10.0 && (acy - (y + h / 2.0)).abs() < 10.0 {
            return Some((*wid, title.as_str()));
        }
    }
    None
}

/// 从 AccessibleProxy 建 ComponentProxy（同一 destination/path 换接口）。
async fn component_proxy<'c>(
    proxy: &'c atspi::proxy::accessible::AccessibleProxy<'_>,
    conn: &'c atspi::zbus::Connection,
) -> Option<atspi::proxy::component::ComponentProxy<'c>> {
    let inner = proxy.inner();
    atspi::proxy::component::ComponentProxy::builder(conn)
        .destination(inner.destination().clone())
        .ok()?
        .path(inner.path().clone())
        .ok()?
        .build()
        .await
        .ok()
}

/// 递归收集窗口内可交互元素（role 白名单 + Enabled/Showing + 屏幕物理像素坐标）。
/// 直接递归的 async fn 会产生无限大小 future 类型（E0733），改为返回装箱 Future 的普通函数，
/// 递归调用通过 Box::pin 执行，消除编译错误。
#[allow(clippy::too_many_arguments)]
fn collect_elements<'a>(
    node: &'a atspi::proxy::accessible::AccessibleProxy<'a>,
    conn: &'a atspi::zbus::Connection,
    window_id: u32,
    window_title: &'a str,
    out: &'a mut Vec<UiElementInfo>,
    depth: usize,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async move {
        if depth > MAX_TREE_DEPTH || out.len() >= MAX_ELEMENTS {
            return;
        }
        let role = match node.get_role().await {
            Ok(r) => r,
            Err(_) => atspi::Role::Unknown,
        };
        if is_interactive_role(&role) {
            let state = match node.get_state().await {
                Ok(s) => s,
                Err(_) => atspi::StateSet::empty(),
            };
            // Text 角色需 Editable 状态才可交互（与 UIA Edit 语义对齐）
            let text_editable_ok =
                role != atspi::Role::Text || state.contains(atspi::State::Editable);
            if text_editable_ok
                && state.contains(atspi::State::Enabled)
                && state.contains(atspi::State::Showing)
            {
                if let Some(comp) = component_proxy(node, conn).await {
                    if let Ok((ex, ey, ew, eh)) = comp.get_extents(atspi::CoordType::Screen).await {
                        if ew > 0 && eh > 0 {
                            let name = node.name().await.unwrap_or_default();
                            out.push(UiElementInfo {
                                window_id,
                                window_title: window_title.to_string(),
                                control_type: role_name(&role).to_string(),
                                name,
                                x: ex,
                                y: ey,
                                width: ew,
                                height: eh,
                                focused: state.contains(atspi::State::Focused),
                            });
                        }
                    }
                }
            }
        }
        let children = match node.get_children().await {
            Ok(v) => v,
            Err(_) => return,
        };
        for child in children {
            if out.len() >= MAX_ELEMENTS {
                return;
            }
            let Ok(cp) = child.into_accessible_proxy(conn).await else {
                continue;
            };
            collect_elements(
                &cp,
                conn,
                window_id,
                window_title,
                out,
                depth + 1,
            )
            .await;
        }
    })
}

/// 可交互角色白名单（与 Windows UIA 白名单语义对齐）。
/// Tree/TreeTable 是容器角色不算可交互控件；树节点用 TreeItem。
fn is_interactive_role(role: &atspi::Role) -> bool {
    matches!(
        role,
        atspi::Role::Button
            | atspi::Role::ToggleButton
            | atspi::Role::Text
            | atspi::Role::PasswordText
            | atspi::Role::CheckBox
            | atspi::Role::ComboBox
            | atspi::Role::ListItem
            | atspi::Role::MenuItem
            | atspi::Role::CheckMenuItem
            | atspi::Role::RadioMenuItem
            | atspi::Role::RadioButton
            | atspi::Role::Slider
            | atspi::Role::PageTab
            | atspi::Role::TreeItem
    )
}

/// AT-SPI 角色转可读名称（与 Windows UIA 的 control_type 命名对齐，模型侧不感知平台差异）。
fn role_name(role: &atspi::Role) -> &'static str {
    match role {
        atspi::Role::Button | atspi::Role::ToggleButton => "Button",
        atspi::Role::Text | atspi::Role::PasswordText => "Edit",
        atspi::Role::CheckBox => "CheckBox",
        atspi::Role::ComboBox => "ComboBox",
        atspi::Role::ListItem => "ListItem",
        atspi::Role::MenuItem | atspi::Role::CheckMenuItem | atspi::Role::RadioMenuItem => {
            "MenuItem"
        }
        atspi::Role::RadioButton => "RadioButton",
        atspi::Role::Slider => "Slider",
        atspi::Role::PageTab => "TabItem",
        atspi::Role::TreeItem => "TreeItem",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod linux_platform_tests {
    use super::*;
    // eprintln! 宏在 main.rs:29 被重定义为 runtime_log_info（crate root）
    use crate::runtime_log_info;

    #[test]
    fn interactive_role_whitelist_should_match_expected_roles() {
        assert!(is_interactive_role(&atspi::Role::Button));
        assert!(is_interactive_role(&atspi::Role::ToggleButton));
        assert!(is_interactive_role(&atspi::Role::Text));
        assert!(is_interactive_role(&atspi::Role::PasswordText));
        assert!(is_interactive_role(&atspi::Role::CheckBox));
        assert!(is_interactive_role(&atspi::Role::ComboBox));
        assert!(is_interactive_role(&atspi::Role::RadioButton));
        assert!(is_interactive_role(&atspi::Role::TreeItem));
        assert!(!is_interactive_role(&atspi::Role::Label));
        assert!(!is_interactive_role(&atspi::Role::Panel));
        assert!(!is_interactive_role(&atspi::Role::Frame));
        assert!(!is_interactive_role(&atspi::Role::Tree));
        assert!(!is_interactive_role(&atspi::Role::TreeTable));
    }

    #[test]
    fn role_name_should_map_known_roles() {
        assert_eq!(role_name(&atspi::Role::Button), "Button");
        assert_eq!(role_name(&atspi::Role::ToggleButton), "Button");
        assert_eq!(role_name(&atspi::Role::Text), "Edit");
        assert_eq!(role_name(&atspi::Role::PasswordText), "Edit");
        assert_eq!(role_name(&atspi::Role::PageTab), "TabItem");
        assert_eq!(role_name(&atspi::Role::TreeItem), "TreeItem");
        assert_eq!(role_name(&atspi::Role::Tree), "Unknown");
        assert_eq!(role_name(&atspi::Role::Unknown), "Unknown");
    }

    #[test]
    fn zero_inputs_should_return_empty() {
        assert!(collect_window_ui_elements(0, false).is_empty());
    }

    /// 真实桌面冒烟测试：枚举窗口 + 激活前台 + 控件树扫描。
    /// 依赖真实 Linux X11 桌面 + AT-SPI2 服务，默认忽略，手动 `--ignored` 跑。
    #[test]
    #[ignore = "需要真实 Linux 桌面（X11 + AT-SPI2）"]
    fn real_desktop_should_list_activate_and_scan() {
        let windows = list_all_windows();
        assert!(!windows.is_empty(), "X11 桌面应有可见窗口");
        eprintln!("[probe] total windows: {}", windows.len());
        for w in windows.iter().take(5) {
            eprintln!(
                "[probe] id=0x{:x} title={:?} pid={} minimized={} focused={} rect=({},{},{},{})",
                w.window_id, w.title, w.process_id, w.minimized, w.focused, w.x, w.y, w.width, w.height
            );
        }
        // 激活前台窗口：应成功
        if let Some(fg) = windows.iter().find(|w| w.focused) {
            let (title, activated) = activate_window(fg.window_id);
            eprintln!(
                "[probe] activate focused window 0x{:x}: title={:?} activated={}",
                fg.window_id, title, activated
            );
            assert!(activated, "激活前台窗口应成功");
        }
        // 控件树扫描（不强制非空，AT-SPI 服务缺失/应用不暴露时为空）
        let targets: Vec<(usize, String)> = windows
            .iter()
            .take(3)
            .map(|w| (w.window_id, w.title.clone()))
            .collect();
        let elems = collect_ui_tree_for_windows(&targets, false);
        eprintln!("[probe] scanned {} elements from {} windows", elems.len(), targets.len());
        for e in elems.iter().take(5) {
            eprintln!("  {}({}) at {}x{}", e.control_type, e.name, e.x, e.y);
        }
    }
}
