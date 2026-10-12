// ==================== 开机自启 ====================
// 桌面宿主能力，由 tauri-plugin-autostart 承载：
// Windows 写 HKCU\...\Run（不需要管理员）、macOS 用 LaunchAgent、Linux 写 autostart desktop entry。
// 注册状态以系统实际状态为准，不落配置；Web / VS Code 宿主没有该能力，前端按能力显隐。

use tauri_plugin_autostart::ManagerExt;

/// 开机自启拉起时附加的命令行标记：命中则本次启动静默驻留托盘，不显示窗口。
const AUTOSTART_LAUNCH_FLAG: &str = "--autostart";

/// 读取当前是否已注册开机自启。
#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|err| format!("读取开机自启状态失败：{err}"))
}

/// 开启 / 关闭开机自启，返回落定后的实际状态。
#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|err| format!("开启开机自启失败：{err}"))?;
    } else {
        manager
            .disable()
            .map_err(|err| format!("关闭开机自启失败：{err}"))?;
    }
    manager
        .is_enabled()
        .map_err(|err| format!("读取开机自启状态失败：{err}"))
}

/// 启动期自愈：已开启自启时按当前可执行文件路径重写一次注册项。
/// 便携版被挪到别的目录后，注册项里的旧路径会指空，这里把它刷新成当前路径。
fn refresh_autostart_target(app: &AppHandle) {
    let manager = app.autolaunch();
    match manager.is_enabled() {
        Ok(false) => {}
        Ok(true) => match manager.enable() {
            Ok(()) => runtime_log_info("[开机自启] 已开启，按当前可执行文件路径刷新注册项".to_string()),
            Err(err) => runtime_log_warn(format!("[开机自启] 刷新注册项失败：{err}")),
        },
        Err(err) => runtime_log_warn(format!("[开机自启] 读取状态失败：{err}")),
    }
}
