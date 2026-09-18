// ==================== WebView2 进程健康监控 ====================
//
// WebView2 的进程失败在宿主侧没有预防 API：官方只提供 ProcessFailed 事件，
// 宿主拿到失败种类、原因与退出码后自行决定重载还是重建窗口。
// 事件回调运行在创建 WebView 的 UI 线程，回调内禁止调用 controller 或任何
// WebView2 方法，恢复动作一律派发到独立线程，在回调之外执行。

use super::*;

/// 需要挂载监控的常驻窗口；运行日志窗口与文件阅读窗口在各自创建处挂载。
const MONITORED_WINDOW_LABELS: [&str; 3] = ["main", "chat", "archives"];

/// 无响应事件每约 15 秒重复触发一次，且没有「已恢复」信号，累计到该次数才判定
/// 为一次需要恢复的 episode，避免第一次告警就把页面状态丢掉。
const UNRESPONSIVE_RECOVERY_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessFailureKind {
    BrowserProcessExited,
    RenderProcessExited,
    FrameRenderProcessExited,
    RenderProcessUnresponsive,
    UtilityProcessExited,
    SandboxHelperProcessExited,
    GpuProcessExited,
    PpapiPluginProcessExited,
    PpapiBrokerProcessExited,
    UnknownProcessExited,
}

impl ProcessFailureKind {
    fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::BrowserProcessExited,
            1 => Self::RenderProcessExited,
            2 => Self::RenderProcessUnresponsive,
            3 => Self::FrameRenderProcessExited,
            4 => Self::UtilityProcessExited,
            5 => Self::SandboxHelperProcessExited,
            6 => Self::GpuProcessExited,
            7 => Self::PpapiPluginProcessExited,
            8 => Self::PpapiBrokerProcessExited,
            _ => Self::UnknownProcessExited,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::BrowserProcessExited => "浏览器进程退出",
            Self::RenderProcessExited => "渲染进程退出",
            Self::FrameRenderProcessExited => "子框架渲染进程退出",
            Self::RenderProcessUnresponsive => "渲染进程无响应",
            Self::UtilityProcessExited => "工具进程退出",
            Self::SandboxHelperProcessExited => "沙箱辅助进程退出",
            Self::GpuProcessExited => "GPU 进程退出",
            Self::PpapiPluginProcessExited => "PPAPI 插件进程退出",
            Self::PpapiBrokerProcessExited => "PPAPI 代理进程退出",
            Self::UnknownProcessExited => "未知进程退出",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::BrowserProcessExited => "BrowserProcessExited",
            Self::RenderProcessExited => "RenderProcessExited",
            Self::FrameRenderProcessExited => "FrameRenderProcessExited",
            Self::RenderProcessUnresponsive => "RenderProcessUnresponsive",
            Self::UtilityProcessExited => "UtilityProcessExited",
            Self::SandboxHelperProcessExited => "SandboxHelperProcessExited",
            Self::GpuProcessExited => "GpuProcessExited",
            Self::PpapiPluginProcessExited => "PpapiPluginProcessExited",
            Self::PpapiBrokerProcessExited => "PpapiBrokerProcessExited",
            Self::UnknownProcessExited => "UnknownProcessExited",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessFailureReason {
    Unexpected,
    Unresponsive,
    Terminated,
    Crashed,
    LaunchFailed,
    OutOfMemory,
    ProfileDeleted,
    Unknown,
}

impl ProcessFailureReason {
    fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Unexpected,
            1 => Self::Unresponsive,
            2 => Self::Terminated,
            3 => Self::Crashed,
            4 => Self::LaunchFailed,
            5 => Self::OutOfMemory,
            6 => Self::ProfileDeleted,
            _ => Self::Unknown,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Unexpected => "意外失败",
            Self::Unresponsive => "无响应",
            Self::Terminated => "被强制结束",
            Self::Crashed => "崩溃",
            Self::LaunchFailed => "启动失败",
            Self::OutOfMemory => "内存不足",
            Self::ProfileDeleted => "用户数据目录被删除",
            Self::Unknown => "未知",
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Unexpected => "Unexpected",
            Self::Unresponsive => "Unresponsive",
            Self::Terminated => "Terminated",
            Self::Crashed => "Crashed",
            Self::LaunchFailed => "LaunchFailed",
            Self::OutOfMemory => "OutOfMemory",
            Self::ProfileDeleted => "ProfileDeleted",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
struct ProcessFailureReport {
    window_label: String,
    kind: ProcessFailureKind,
    reason: ProcessFailureReason,
    exit_code: Option<i32>,
    process_description: String,
    failure_source_module: String,
    runtime_version: String,
}

impl ProcessFailureReport {
    fn describe(&self) -> String {
        let exit_code = match self.exit_code {
            Some(code) => code.to_string(),
            None => "未知".to_string(),
        };
        let trimmed = self.process_description.trim();
        let process = if trimmed.is_empty() { "未知" } else { trimmed };
        let trimmed = self.failure_source_module.trim();
        let source = if trimmed.is_empty() { "未知" } else { trimmed };
        format!(
            "[WebView2] 进程失败：window_label={}，kind={}（{}），reason={}（{}），exit_code={}，runtime_version={}，process={}，failure_source_module={}",
            self.window_label,
            self.kind.label(),
            self.kind.code(),
            self.reason.label(),
            self.reason.code(),
            exit_code,
            self.runtime_version,
            process,
            source,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessFailureRecovery {
    /// 只记录，不干预，交由 runtime 自行重启
    Record,
    /// 重载窗口内的 WebView：渲染进程死亡后的最小恢复动作
    Reload,
    /// 销毁并重建整个窗口：浏览器进程退出后旧 COM 引用全部失效，只有重建能恢复
    Rebuild,
}

impl ProcessFailureRecovery {
    fn label(self) -> &'static str {
        match self {
            Self::Record => "只记录",
            Self::Reload => "重载",
            Self::Rebuild => "重建窗口",
        }
    }
}

fn recovery_for_process_failure(
    kind: ProcessFailureKind,
    unresponsive_count: u32,
) -> ProcessFailureRecovery {
    match kind {
        ProcessFailureKind::RenderProcessUnresponsive => {
            if unresponsive_count >= UNRESPONSIVE_RECOVERY_THRESHOLD {
                ProcessFailureRecovery::Reload
            } else {
                ProcessFailureRecovery::Record
            }
        }
        ProcessFailureKind::RenderProcessExited | ProcessFailureKind::FrameRenderProcessExited => {
            ProcessFailureRecovery::Reload
        }
        ProcessFailureKind::BrowserProcessExited => ProcessFailureRecovery::Rebuild,
        _ => ProcessFailureRecovery::Record,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct WindowFailureState {
    unresponsive_count: u32,
    recovery_in_flight: bool,
}

/// 记录一次进程失败，返回本次应执行的恢复动作。
/// 无响应事件会高频重复触发，这里把连续的无响应归并为一次 episode：
/// 触发恢复后清零计数，并在恢复动作结束前拒绝重复触发。
fn note_process_failure(
    state: &mut WindowFailureState,
    kind: ProcessFailureKind,
) -> ProcessFailureRecovery {
    if kind == ProcessFailureKind::RenderProcessUnresponsive {
        state.unresponsive_count = state.unresponsive_count.saturating_add(1);
    } else {
        state.unresponsive_count = 0;
    }
    if state.recovery_in_flight {
        return ProcessFailureRecovery::Record;
    }
    let recovery = recovery_for_process_failure(kind, state.unresponsive_count);
    if recovery != ProcessFailureRecovery::Record {
        state.unresponsive_count = 0;
        state.recovery_in_flight = true;
    }
    recovery
}

static WEBVIEW_FAILURE_STATES: OnceLock<Mutex<std::collections::HashMap<String, WindowFailureState>>> =
    OnceLock::new();

fn webview_failure_states() -> &'static Mutex<std::collections::HashMap<String, WindowFailureState>> {
    WEBVIEW_FAILURE_STATES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn update_window_failure_state<R>(
    label: &str,
    update: impl FnOnce(&mut WindowFailureState) -> R,
) -> R {
    let mut states = webview_failure_states()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    update(states.entry(label.to_string()).or_default())
}

static WEBVIEW_ENVIRONMENT_FOOTPRINT_LOGGED: OnceLock<()> = OnceLock::new();

/// 为全部常驻窗口挂载进程失败监控。入口不阻塞：真正的注册发生在窗口所在的
/// UI 线程，失败只记日志，不影响启动。
pub fn attach_webview_process_failed_monitors(app: &AppHandle) {
    for label in MONITORED_WINDOW_LABELS {
        let Some(window) = app.get_webview_window(label) else {
            continue;
        };
        attach_webview_process_failed_monitor(&window, app);
    }
}

/// 为单个窗口挂载进程失败监控；非 Windows 平台没有对应宿主能力，静默跳过。
pub fn attach_webview_process_failed_monitor(window: &tauri::WebviewWindow, app: &AppHandle) {
    #[cfg(target_os = "windows")]
    attach_webview_process_failed_monitor_windows(window, app);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, app);
    }
}

/// 恢复动作派发到独立线程执行：事件回调运行在 UI 线程，直接在其中调用
/// WebView2 或窗口接口会重入当前 controller 调用。
#[cfg(target_os = "windows")]
fn dispatch_process_failure_recovery(
    app: &AppHandle,
    label: &str,
    recovery: ProcessFailureRecovery,
) {
    let app_handle = app.clone();
    let label = label.to_string();
    let thread_label = label.clone();
    if let Err(err) = std::thread::Builder::new()
        .name(format!("webview-recovery-{label}"))
        .spawn(move || {
            let label = thread_label;
            let started_at = std::time::Instant::now();
            runtime_log_info(format!(
                "[WebView2] 恢复开始：window_label={}，action={}",
                label,
                recovery.label()
            ));
            let result = match recovery {
                ProcessFailureRecovery::Record => Ok(()),
                ProcessFailureRecovery::Reload => reload_window(&app_handle, &label),
                ProcessFailureRecovery::Rebuild => rebuild_window(&app_handle, &label),
            };
            match &result {
                Ok(()) => runtime_log_info(format!(
                    "[WebView2] 恢复完成：window_label={}，action={}，elapsed_ms={}",
                    label,
                    recovery.label(),
                    started_at.elapsed().as_millis()
                )),
                Err(err) => runtime_log_error(format!(
                    "[WebView2] 恢复失败：window_label={}，action={}，elapsed_ms={}，error={}",
                    label,
                    recovery.label(),
                    started_at.elapsed().as_millis(),
                    err
                )),
            }
            update_window_failure_state(&label, |state| state.recovery_in_flight = false);
        })
    {
        runtime_log_error(format!(
            "[WebView2] 派发恢复动作失败：window_label={label}，error={err}"
        ));
        update_window_failure_state(&label, |state| state.recovery_in_flight = false);
    }
}

#[cfg(target_os = "windows")]
fn attach_webview_process_failed_monitor_windows(window: &tauri::WebviewWindow, app: &AppHandle) {
    use webview2_com::ProcessFailedEventHandler;

    let label = window.label().to_string();
    let app_handle = app.clone();
    let register_label = label.clone();
    let result = window.with_webview(move |platform| {
        let environment = platform.environment();
        let runtime_version = read_webview_runtime_version(&environment);
        runtime_log_info(format!(
            "[WebView2] 进程失败监控已挂载：window_label={}，runtime_version={}",
            register_label, runtime_version
        ));
        log_webview_environment_footprint(&environment);

        let controller = platform.controller();
        let core = match unsafe { controller.CoreWebView2() } {
            Ok(core) => core,
            Err(err) => {
                runtime_log_warn(format!(
                    "[WebView2] 读取 CoreWebView2 失败，跳过进程失败监控：window_label={}，error={}",
                    register_label, err
                ));
                return;
            }
        };

        let handler_label = register_label.clone();
        let handler_app = app_handle.clone();
        let handler_version = runtime_version.clone();
        let handler = ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
            let report = read_process_failure_report(&handler_label, &handler_version, args);
            runtime_log_warn(report.describe());
            let recovery = update_window_failure_state(&handler_label, |state| {
                note_process_failure(state, report.kind)
            });
            if recovery != ProcessFailureRecovery::Record {
                dispatch_process_failure_recovery(&handler_app, &handler_label, recovery);
            }
            Ok(())
        }));

        let mut token = 0i64;
        if let Err(err) = unsafe { core.add_ProcessFailed(&handler, &mut token) } {
            runtime_log_error(format!(
                "[WebView2] 注册进程失败监控失败：window_label={register_label}，error={err}"
            ));
        }
    });
    if let Err(err) = result {
        runtime_log_warn(format!(
            "[WebView2] 挂载进程失败监控失败：window_label={label}，error={err}"
        ));
    }
}

#[cfg(target_os = "windows")]
fn read_webview_runtime_version(
    environment: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
) -> String {
    use webview2_com::take_pwstr;
    use windows_core::PWSTR;

    let mut value = PWSTR::null();
    match unsafe { environment.BrowserVersionString(&mut value) } {
        Ok(()) => take_pwstr(value),
        Err(err) => {
            runtime_log_warn(format!("[WebView2] 读取 runtime 版本失败：error={err}"));
            "未知".to_string()
        }
    }
}

/// 崩溃转储目录只在真崩溃时才有内容（hang 不产生 dump），把路径记一次，
/// 用户下次报「卡住」时至少能指向具体目录。
#[cfg(target_os = "windows")]
fn log_webview_environment_footprint(
    environment: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment,
) {
    use webview2_com::take_pwstr;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment11;
    use windows_core::{Interface, PWSTR};

    if WEBVIEW_ENVIRONMENT_FOOTPRINT_LOGGED.set(()).is_err() {
        return;
    }
    let Ok(environment11) = environment.cast::<ICoreWebView2Environment11>() else {
        return;
    };
    let mut folder = PWSTR::null();
    match unsafe { environment11.FailureReportFolderPath(&mut folder) } {
        Ok(()) => runtime_log_info(format!(
            "[WebView2] 崩溃转储目录：{}",
            take_pwstr(folder)
        )),
        Err(err) => runtime_log_warn(format!("[WebView2] 读取崩溃转储目录失败：error={err}")),
    }
}

#[cfg(target_os = "windows")]
fn read_process_failure_report(
    window_label: &str,
    runtime_version: &str,
    args: Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ProcessFailedEventArgs>,
) -> ProcessFailureReport {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2ProcessFailedEventArgs2, ICoreWebView2ProcessFailedEventArgs3,
    };
    use webview2_com::take_pwstr;
    use windows_core::{Interface, PWSTR};

    let mut report = ProcessFailureReport {
        window_label: window_label.to_string(),
        kind: ProcessFailureKind::UnknownProcessExited,
        reason: ProcessFailureReason::Unknown,
        exit_code: None,
        process_description: String::new(),
        failure_source_module: String::new(),
        runtime_version: runtime_version.to_string(),
    };

    let Some(args) = args else {
        return report;
    };

    let mut raw_kind = Default::default();
    if unsafe { args.ProcessFailedKind(&mut raw_kind) }.is_ok() {
        report.kind = ProcessFailureKind::from_raw(raw_kind.0);
    }

    if let Ok(args2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
        let mut raw_reason = Default::default();
        if unsafe { args2.Reason(&mut raw_reason) }.is_ok() {
            report.reason = ProcessFailureReason::from_raw(raw_reason.0);
        }
        let mut raw_exit_code = 0i32;
        if unsafe { args2.ExitCode(&mut raw_exit_code) }.is_ok() {
            report.exit_code = Some(raw_exit_code);
        }
        let mut description = PWSTR::null();
        if unsafe { args2.ProcessDescription(&mut description) }.is_ok() {
            report.process_description = take_pwstr(description);
        }
    }

    if let Ok(args3) = args.cast::<ICoreWebView2ProcessFailedEventArgs3>() {
        let mut source_module = PWSTR::null();
        if unsafe { args3.FailureSourceModulePath(&mut source_module) }.is_ok() {
            report.failure_source_module = take_pwstr(source_module);
        }
    }

    report
}

#[cfg(test)]
mod webview_health_tests {
    use super::*;

    fn report(kind: ProcessFailureKind, reason: ProcessFailureReason) -> ProcessFailureReport {
        ProcessFailureReport {
            window_label: "chat".to_string(),
            kind,
            reason,
            exit_code: Some(259),
            process_description: "browser".to_string(),
            failure_source_module: String::new(),
            runtime_version: "131.0.2903.86".to_string(),
        }
    }

    #[test]
    fn maps_process_failed_kind_raw_values() {
        assert_eq!(
            ProcessFailureKind::from_raw(0),
            ProcessFailureKind::BrowserProcessExited
        );
        assert_eq!(
            ProcessFailureKind::from_raw(1),
            ProcessFailureKind::RenderProcessExited
        );
        assert_eq!(
            ProcessFailureKind::from_raw(2),
            ProcessFailureKind::RenderProcessUnresponsive
        );
        assert_eq!(
            ProcessFailureKind::from_raw(6),
            ProcessFailureKind::GpuProcessExited
        );
        assert_eq!(
            ProcessFailureKind::from_raw(42),
            ProcessFailureKind::UnknownProcessExited
        );
    }

    #[test]
    fn maps_process_failed_reason_raw_values() {
        assert_eq!(ProcessFailureReason::from_raw(0), ProcessFailureReason::Unexpected);
        assert_eq!(
            ProcessFailureReason::from_raw(1),
            ProcessFailureReason::Unresponsive
        );
        assert_eq!(ProcessFailureReason::from_raw(3), ProcessFailureReason::Crashed);
        assert_eq!(
            ProcessFailureReason::from_raw(5),
            ProcessFailureReason::OutOfMemory
        );
        assert_eq!(ProcessFailureReason::from_raw(9), ProcessFailureReason::Unknown);
    }

    #[test]
    fn records_unresponsive_until_threshold_then_reloads() {
        let mut state = WindowFailureState::default();
        for _ in 0..UNRESPONSIVE_RECOVERY_THRESHOLD - 1 {
            assert_eq!(
                note_process_failure(&mut state, ProcessFailureKind::RenderProcessUnresponsive),
                ProcessFailureRecovery::Record
            );
        }
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessUnresponsive),
            ProcessFailureRecovery::Reload
        );
        assert_eq!(state.unresponsive_count, 0);
    }

    #[test]
    fn reloads_immediately_when_render_process_exited() {
        let mut state = WindowFailureState::default();
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessExited),
            ProcessFailureRecovery::Reload
        );
        let mut state = WindowFailureState::default();
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::FrameRenderProcessExited),
            ProcessFailureRecovery::Reload
        );
    }

    #[test]
    fn rebuilds_window_when_browser_process_exited() {
        let mut state = WindowFailureState::default();
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::BrowserProcessExited),
            ProcessFailureRecovery::Rebuild
        );
    }

    #[test]
    fn leaves_auxiliary_processes_alone() {
        for kind in [
            ProcessFailureKind::GpuProcessExited,
            ProcessFailureKind::UtilityProcessExited,
            ProcessFailureKind::SandboxHelperProcessExited,
            ProcessFailureKind::PpapiPluginProcessExited,
            ProcessFailureKind::PpapiBrokerProcessExited,
            ProcessFailureKind::UnknownProcessExited,
        ] {
            let mut state = WindowFailureState::default();
            assert_eq!(
                note_process_failure(&mut state, kind),
                ProcessFailureRecovery::Record,
                "{kind:?} 不应触发恢复"
            );
            assert_eq!(recovery_for_process_failure(kind, 99), ProcessFailureRecovery::Record);
        }
    }

    #[test]
    fn ignores_failures_while_recovery_is_in_flight() {
        let mut state = WindowFailureState::default();
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessExited),
            ProcessFailureRecovery::Reload
        );
        assert!(state.recovery_in_flight);
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessExited),
            ProcessFailureRecovery::Record
        );
        state.recovery_in_flight = false;
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessExited),
            ProcessFailureRecovery::Reload
        );
    }

    #[test]
    fn resets_unresponsive_episode_on_other_failures() {
        let mut state = WindowFailureState::default();
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessUnresponsive),
            ProcessFailureRecovery::Record
        );
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::GpuProcessExited),
            ProcessFailureRecovery::Record
        );
        assert_eq!(state.unresponsive_count, 0);
        assert_eq!(
            note_process_failure(&mut state, ProcessFailureKind::RenderProcessUnresponsive),
            ProcessFailureRecovery::Record
        );
    }

    #[test]
    fn describes_failure_with_diagnosable_fields() {
        let line = report(
            ProcessFailureKind::RenderProcessUnresponsive,
            ProcessFailureReason::Unresponsive,
        )
        .describe();
        assert!(line.contains("window_label=chat"), "{line}");
        assert!(line.contains("渲染进程无响应"), "{line}");
        assert!(line.contains("RenderProcessUnresponsive"), "{line}");
        assert!(line.contains("Unresponsive"), "{line}");
        assert!(line.contains("exit_code=259"), "{line}");
        assert!(line.contains("runtime_version=131.0.2903.86"), "{line}");
        assert!(line.contains("failure_source_module=未知"), "{line}");
    }

    #[test]
    fn falls_back_to_unknown_placeholders() {
        let mut failure = report(
            ProcessFailureKind::UnknownProcessExited,
            ProcessFailureReason::Unknown,
        );
        failure.exit_code = None;
        failure.process_description = "  ".to_string();
        failure.failure_source_module = String::new();
        let line = failure.describe();
        assert!(line.contains("exit_code=未知"), "{line}");
        assert!(line.contains("failure_source_module=未知"), "{line}");
    }
}
