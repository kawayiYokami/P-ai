use std::{
    ffi::OsString,
    fs::{self as std_fs, File as StdFile},
    io::{Read, Write},
    path::{Path as StdPath, PathBuf as StdPathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration as StdDuration, Instant as StdInstant},
};

use tauri_plugin_updater::UpdaterExt;
use walkdir::WalkDir;
use zip::ZipArchive;

const UPDATER_GITHUB_PROXY_PREFIX: &str = "https://gh-proxy.org/";
const UPDATER_GITHUB_EDGEONE_PROXY_PREFIX: &str = "https://edgeone.gh-proxy.org/";
const UPDATER_GITHUB_HK_PROXY_PREFIX: &str = "https://hk.gh-proxy.org/";
const UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES: &[&str] = &[
    "https://gh-proxy.com/",
    "https://ghproxy.net/",
    "https://ghproxy.homeboyc.cn/",
    "https://github.akams.cn/",
    UPDATER_GITHUB_PROXY_PREFIX,
    UPDATER_GITHUB_EDGEONE_PROXY_PREFIX,
    UPDATER_GITHUB_HK_PROXY_PREFIX,
];
const UPDATER_GITHUB_RELEASE_API_ORIGIN: &str =
    "https://api.github.com/repos/kawayiYokami/P-ai/releases/latest";
const UPDATER_GITHUB_CHANGELOG_LATEST_RAW_ORIGIN: &str =
    "https://raw.githubusercontent.com/kawayiYokami/P-ai/main/docs/changelog/latest.md";
const UPDATER_GITHUB_CHANGELOG_REMOTE_RAW_ORIGIN: &str =
    "https://raw.githubusercontent.com/kawayiYokami/P-ai/main/docs/changelog/remote.md";
const UPDATER_GITHUB_RELEASE_PAGE_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest";
#[cfg(target_os = "windows")]
const UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest.json";
#[cfg(target_os = "macos")]
const UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest-darwin.json";
#[cfg(target_os = "linux")]
const UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest-linux.json";
const UPDATER_GITHUB_PORTABLE_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest-portable.json";
const PORTABLE_UPDATE_EVENT_NAME: &str = "easy-call:update-status";
const PORTABLE_HELPER_FLAG: &str = "--portable-update-helper";
const PORTABLE_HELPER_FILE_PREFIX: &str = "portable-helper-";
const PORTABLE_PLAN_FILE_PREFIX: &str = "portable-plan-";
const PORTABLE_ZIP_FILE_PREFIX: &str = "p-ai-portable-";
const PORTABLE_STAGING_DIR_PREFIX: &str = "staging-";
const PORTABLE_UPDATE_TARGET_SUFFIX: &str = "-portable";
const PORTABLE_PENDING_FILE_NAME: &str = "portable-pending.json";
const PORTABLE_REPLACE_MAX_RETRIES: usize = 30;
const PORTABLE_REPLACE_RETRY_DELAY_MS: u64 = 400;
const PORTABLE_PARENT_WAIT_TIMEOUT_MS: u64 = 15000;
const UPDATE_STAGE_CHECKING: &str = "checking";
const UPDATE_STAGE_DOWNLOADING: &str = "downloading";
const UPDATE_STAGE_VERIFYING: &str = "verifying";
const UPDATE_STAGE_PREPARING: &str = "preparing";
const UPDATE_STAGE_INSTALLING: &str = "installing";
const UPDATE_STAGE_REPLACING: &str = "replacing";
const UPDATE_STAGE_READY: &str = "ready";
const UPDATE_STAGE_COMPLETED: &str = "completed";
const UPDATE_STAGE_CANCELLED: &str = "cancelled";
const UPDATE_STAGE_FAILED: &str = "failed";
const GITHUB_AUTO_UPDATE_COOLDOWN_HOURS: i64 = 8;
const GITHUB_AUTO_UPDATE_POLL_MINUTES: u64 = 10;
const GITHUB_AUTO_UPDATE_STARTUP_DELAY_SECONDS: u64 = 45;
const GITHUB_UPDATE_CHECK_TIMEOUT_SECONDS: u64 = 10;
const GITHUB_UPDATE_DOWNLOAD_TIMEOUT_SECONDS: u64 = 10 * 60;
const GITHUB_UPDATE_PROXY_CURSOR_FILE_NAME: &str = "github-update-proxy-cursor.json";
const CHANGELOG_FETCH_COOLDOWN_HOURS: i64 = 1;

static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static UPDATE_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
static PREPARED_GITHUB_UPDATE: Mutex<Option<PreparedGithubUpdate>> = Mutex::new(None);
static LAST_AUTO_UPDATE_CHECKED_AT: std::sync::Mutex<Option<OffsetDateTime>> =
    std::sync::Mutex::new(None);
static CHANGELOG_MARKDOWN_CACHE: std::sync::Mutex<Option<CachedChangelogMarkdown>> =
    std::sync::Mutex::new(None);
static GITHUB_UPDATE_STATE: std::sync::LazyLock<std::sync::Mutex<GithubUpdateState>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(GithubUpdateState::default()));
static GITHUB_AUTO_UPDATE_WORKER_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum UpdateRuntimeKind {
    Installer,
    Portable,
}

impl UpdateRuntimeKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Installer => "installer",
            Self::Portable => "portable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GithubUpdateMethod {
    Auto,
    Direct,
    Proxy,
}

impl GithubUpdateMethod {
    fn from_raw(value: Option<String>) -> Self {
        match value.unwrap_or_default().trim() {
            "direct" => Self::Direct,
            "proxy" => Self::Proxy,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubUpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    release_url: String,
    update_source: String,
    access_mode: String,
    release_notes: String,
    published_at: Option<String>,
    runtime_kind: String,
    can_force_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgressPayload {
    stage: String,
    message: String,
    runtime_kind: String,
    current_version: Option<String>,
    target_version: Option<String>,
    downloaded_bytes: Option<u64>,
    content_length: Option<u64>,
    percent: Option<f64>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GithubUpdateState {
    stage: String,
    current_version: String,
    latest_version: String,
    runtime_kind: String,
    has_prepared_update: bool,
    has_visible_update: bool,
    release_notes: String,
    release_url: String,
    published_at: Option<String>,
    prepared_at: Option<String>,
    last_checked_at: Option<String>,
    last_error: Option<String>,
    skipped_version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubLatestReleasePayload {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
struct GithubUpdateProxyCursor {
    index: usize,
}

#[derive(Debug, Clone)]
struct GithubUpdateManifestCandidate {
    endpoint: String,
    display_name: &'static str,
    log_name: &'static str,
}

#[derive(Debug, Clone)]
struct UpdateRuntimePaths {
    exe_path: StdPathBuf,
    exe_dir: StdPathBuf,
    data_dir: StdPathBuf,
    runtime_kind: UpdateRuntimeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortableUpdatePlan {
    target_dir: String,
    target_exe_name: String,
    staging_dir: String,
    backup_root: String,
    temp_root: String,
    zip_path: String,
    log_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PortablePendingManualReplace {
    plan: PortableUpdatePlan,
    backup_dir: String,
    failure_reason: String,
    created_at: String,
}

struct PreparedInstallerUpdate {
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
    current_version: String,
    target_version: String,
}

struct PreparedPortableUpdate {
    runtime_kind: UpdateRuntimeKind,
    current_version: String,
    target_version: String,
    helper_copy_path: StdPathBuf,
    plan_path: StdPathBuf,
}

enum PreparedGithubUpdate {
    Installer(PreparedInstallerUpdate),
    Portable(PreparedPortableUpdate),
}

struct UpdateInProgressGuard;

impl UpdateInProgressGuard {
    fn acquire() -> Result<Self, String> {
        if UPDATE_IN_PROGRESS.swap(true, Ordering::SeqCst) {
            return Err("已有更新任务正在执行，请稍后再试".to_string());
        }
        Ok(Self)
    }
}

impl Drop for UpdateInProgressGuard {
    fn drop(&mut self) {
        UPDATE_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

fn updater_release_page_url(origin: &str) -> String {
    origin.to_string()
}

fn updater_release_api_fallback_urls(method: GithubUpdateMethod) -> Vec<String> {
    match method {
        GithubUpdateMethod::Auto => vec![
            format!("{UPDATER_GITHUB_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
            format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
            UPDATER_GITHUB_RELEASE_API_ORIGIN.to_string(),
        ],
        GithubUpdateMethod::Direct => vec![UPDATER_GITHUB_RELEASE_API_ORIGIN.to_string()],
        GithubUpdateMethod::Proxy => vec![
            format!("{UPDATER_GITHUB_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
            format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
        ],
    }
}

fn updater_changelog_api_fallback_urls(origin: &str, method: GithubUpdateMethod) -> Vec<String> {
    match method {
        GithubUpdateMethod::Auto => vec![
            format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"),
            format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"),
            origin.to_string(),
        ],
        GithubUpdateMethod::Direct => vec![origin.to_string()],
        GithubUpdateMethod::Proxy => vec![
            format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"),
            format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"),
        ],
    }
}

fn updater_manifest_fallbacks(
    origin: &str,
    method: GithubUpdateMethod,
) -> Vec<GithubUpdateManifestCandidate> {
    let proxy_a = GithubUpdateManifestCandidate {
        endpoint: format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"),
        display_name: "中转（A）",
        log_name: "proxy_a",
    };
    let proxy_b = GithubUpdateManifestCandidate {
        endpoint: format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"),
        display_name: "中转（B）",
        log_name: "proxy_b",
    };
    let direct = GithubUpdateManifestCandidate {
        endpoint: origin.to_string(),
        display_name: "直连",
        log_name: "direct",
    };
    match method {
        GithubUpdateMethod::Auto => vec![proxy_a, proxy_b, direct],
        GithubUpdateMethod::Direct => vec![direct],
        GithubUpdateMethod::Proxy => vec![proxy_a, proxy_b],
    }
}

fn github_update_download_route_name(method: GithubUpdateMethod, cursor: usize) -> &'static str {
    match method {
        GithubUpdateMethod::Direct => "直连",
        GithubUpdateMethod::Auto | GithubUpdateMethod::Proxy => match cursor
            % UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES.len()
        {
            0 => "中转（A）",
            1 => "中转（B）",
            2 => "中转（C）",
            3 => "中转（D）",
            4 => "中转（E）",
            5 => "中转（F）",
            _ => "中转（G）",
        },
    }
}

fn updater_download_endpoint(origin: &str, method: GithubUpdateMethod, cursor: usize) -> String {
    match method {
        GithubUpdateMethod::Direct => origin.to_string(),
        GithubUpdateMethod::Auto | GithubUpdateMethod::Proxy => {
            let prefix = UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES
                [cursor % UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES.len()];
            format!("{prefix}{origin}")
        }
    }
}

fn github_update_proxy_cursor_path(runtime: &UpdateRuntimePaths) -> StdPathBuf {
    runtime.data_dir.join(GITHUB_UPDATE_PROXY_CURSOR_FILE_NAME)
}

fn read_github_update_proxy_cursor(runtime: &UpdateRuntimePaths) -> usize {
    let path = github_update_proxy_cursor_path(runtime);
    let Ok(raw) = std_fs::read_to_string(&path) else {
        return 0;
    };
    match serde_json::from_str::<GithubUpdateProxyCursor>(&raw) {
        Ok(cursor) => cursor.index % UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES.len(),
        Err(err) => {
            runtime_log_warn(format!(
                "[自动更新] 读取代理游标失败，已回退首个代理：path={}，error={err}",
                path.display()
            ));
            0
        }
    }
}

fn next_github_update_proxy_cursor(current: usize) -> usize {
    (current + 1) % UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES.len()
}

fn advance_github_update_proxy_cursor(
    runtime: &UpdateRuntimePaths,
    current: usize,
) -> Result<usize, String> {
    let next = next_github_update_proxy_cursor(current);
    let path = github_update_proxy_cursor_path(runtime);
    let content = serde_json::to_vec_pretty(&GithubUpdateProxyCursor { index: next })
        .map_err(|err| format!("序列化更新代理游标失败：{err}"))?;
    if let Some(parent) = path.parent() {
        std_fs::create_dir_all(parent)
            .map_err(|err| format!("创建更新代理游标目录失败（{}）：{err}", parent.display()))?;
    }
    std_fs::write(&path, content)
        .map_err(|err| format!("写入更新代理游标失败（{}）：{err}", path.display()))?;
    runtime_log_warn(format!(
        "[自动更新] 下载代理失败，游标已切换：{} -> {}",
        current, next
    ));
    Ok(next)
}

fn finish_failed_update_download(
    runtime: &UpdateRuntimePaths,
    method: GithubUpdateMethod,
    cursor: usize,
    failure_detail: String,
) -> Result<String, String> {
    ensure_update_not_cancelled()?;
    let route_name = github_update_download_route_name(method, cursor);
    runtime_log_warn(format!(
        "[自动更新] 失败，任务=下载更新，阶段=downloading，线路={}，游标={cursor}，error={failure_detail}",
        route_name,
    ));
    if !matches!(method, GithubUpdateMethod::Auto | GithubUpdateMethod::Proxy) {
        return Ok("下载更新失败，请稍后重试或打开 Releases".to_string());
    }
    let next = advance_github_update_proxy_cursor(runtime, cursor).map_err(|err| {
        runtime_log_error(format!(
            "[自动更新] 失败，任务=切换下载线路，当前游标={cursor}，error={err}"
        ));
        "下载更新失败，切换下一条线路失败，请稍后重试".to_string()
    })?;
    runtime_log_info(format!(
        "[自动更新] 完成，任务=切换下载线路，当前游标={cursor}，下一游标={next}"
    ));
    Ok("下载更新失败，已切换到下一条线路，请重新发起更新".to_string())
}

fn strip_known_proxy_prefix(url: &str) -> &str {
    UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))
        .or_else(|| url.strip_prefix(UPDATER_GITHUB_PROXY_PREFIX))
        .or_else(|| url.strip_prefix(UPDATER_GITHUB_EDGEONE_PROXY_PREFIX))
        .or_else(|| url.strip_prefix(UPDATER_GITHUB_HK_PROXY_PREFIX))
        .unwrap_or(url)
}

fn clear_prepared_github_update() {
    if let Ok(mut guard) = PREPARED_GITHUB_UPDATE.lock() {
        *guard = None;
    }
}

fn reset_update_cancel_requested() {
    UPDATE_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

fn request_update_cancel() {
    UPDATE_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
}

fn is_update_cancel_requested() -> bool {
    UPDATE_CANCEL_REQUESTED.load(Ordering::SeqCst)
}

fn is_update_cancelled_error(message: &str) -> bool {
    message.contains("用户已取消更新")
}

fn ensure_update_not_cancelled() -> Result<(), String> {
    if is_update_cancel_requested() {
        return Err("用户已取消更新".to_string());
    }
    Ok(())
}

fn endpoint_access_mode(url: &str) -> &'static str {
    if UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
        || url.starts_with(UPDATER_GITHUB_PROXY_PREFIX)
        || url.starts_with(UPDATER_GITHUB_EDGEONE_PROXY_PREFIX)
        || url.starts_with(UPDATER_GITHUB_HK_PROXY_PREFIX)
    {
        "proxy"
    } else {
        "direct"
    }
}

fn store_prepared_github_update(update: PreparedGithubUpdate) -> Result<(), String> {
    let mut guard = PREPARED_GITHUB_UPDATE
        .lock()
        .map_err(|err| format!("锁定已准备更新状态失败：{err:?}"))?;
    *guard = Some(update);
    Ok(())
}

fn take_prepared_github_update() -> Result<PreparedGithubUpdate, String> {
    let mut guard = PREPARED_GITHUB_UPDATE
        .lock()
        .map_err(|err| format!("锁定已准备更新状态失败：{err:?}"))?;
    guard
        .take()
        .ok_or_else(|| "当前没有已下载完成的更新，请先检查并下载更新".to_string())
}

fn updater_public_key() -> Result<&'static str, String> {
    let key = option_env!("TAURI_UPDATER_PUBLIC_KEY")
        .map(str::trim)
        .unwrap_or_default();
    if key.is_empty() {
        return Err(
            "未配置更新公钥。请在构建时设置 TAURI_UPDATER_PUBLIC_KEY，再重新构建应用".to_string(),
        );
    }
    Ok(key)
}

#[cfg(target_os = "windows")]
fn shutdown_background_services_before_windows_updater_exit(app: AppHandle) {
    // tauri-plugin-updater 在 Windows 安装版会启动安装器后直接 std::process::exit(0)，
    // 必须挂到 on_before_exit，并保留默认 cleanup_before_exit，才能在硬退出前收干净运行态。
    runtime_log_info(format!("[自动更新] Windows 安装器退出前开始优雅停机后台服务"));
    let cleanup_app = app.clone();
    let handle = thread::spawn(move || {
        tauri::async_runtime::block_on(graceful_shutdown_background_services_with_timeout(&app))
    });
    match handle.join() {
        Ok(true) => {
            runtime_log_info(format!("[自动更新] Windows 安装器退出前优雅停机后台服务完成"));
        }
        Ok(false) => {
            runtime_log_error(format!("[自动更新] Windows 安装器退出前优雅停机后台服务超时"));
            show_background_shutdown_timeout_dialog(&cleanup_app);
        }
        Err(_) => {
            runtime_log_error(format!("[自动更新] Windows 安装器退出前优雅停机后台服务失败：停机线程异常退出"));
        }
    }
    cleanup_app.cleanup_before_exit();
}

fn detect_update_runtime_paths() -> Result<UpdateRuntimePaths, String> {
    let exe_path = std::env::current_exe()
        .map_err(|err| format!("获取当前可执行文件路径失败：{err}"))?;
    let exe_dir = exe_path
        .parent()
        .map(StdPath::to_path_buf)
        .ok_or_else(|| format!("无法解析可执行文件所在目录：{}", exe_path.display()))?;
    let runtime_kind = if portable_marker_path_from_exe_dir(&exe_dir).exists() {
        UpdateRuntimeKind::Portable
    } else {
        UpdateRuntimeKind::Installer
    };
    let data_dir = match runtime_kind {
        UpdateRuntimeKind::Portable => exe_dir.join("data"),
        UpdateRuntimeKind::Installer => resolve_standard_config_dir()?.0,
    };
    Ok(UpdateRuntimePaths {
        exe_path,
        exe_dir,
        data_dir,
        runtime_kind,
    })
}

fn current_installer_target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return "windows-x86_64";
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return "windows-aarch64";
    }
    #[cfg(all(target_os = "windows", target_arch = "x86"))]
    {
        return "windows-i686";
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return "linux-x86_64";
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return "linux-aarch64";
    }
    #[cfg(target_os = "macos")]
    {
        return "unsupported";
    }
}

fn current_portable_target() -> String {
    format!("{}{}", current_installer_target(), PORTABLE_UPDATE_TARGET_SUFFIX)
}

fn emit_update_progress(app: &AppHandle, payload: UpdateProgressPayload) {
    sync_update_state_from_progress(app, &payload);
    let _ = app.emit(PORTABLE_UPDATE_EVENT_NAME, payload.clone());
    match serde_json::to_value(payload) {
        Ok(value) => ide_chat_broadcast_notification(PORTABLE_UPDATE_EVENT_NAME, value),
        Err(err) => runtime_log_error(format!("[自动更新] 广播 Web 更新进度失败：{}", err)),
    }
}

fn build_update_progress(
    runtime_kind: UpdateRuntimeKind,
    stage: &str,
    message: impl Into<String>,
    current_version: Option<String>,
    target_version: Option<String>,
    downloaded_bytes: Option<u64>,
    content_length: Option<u64>,
    error: Option<String>,
) -> UpdateProgressPayload {
    let percent = match (downloaded_bytes, content_length) {
        (Some(done), Some(total)) if total > 0 => Some((done as f64 / total as f64) * 100.0),
        _ => None,
    };
    UpdateProgressPayload {
        stage: stage.to_string(),
        message: message.into(),
        runtime_kind: runtime_kind.as_str().to_string(),
        current_version,
        target_version,
        downloaded_bytes,
        content_length,
        percent,
        error,
    }
}

fn normalize_release_version(input: &str) -> String {
    input.trim().trim_start_matches(['v', 'V']).to_string()
}

async fn fetch_latest_release_payload(
    method: GithubUpdateMethod,
) -> Result<(GithubLatestReleasePayload, String), String> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(8))
        .build()
        .map_err(|err| format!("初始化更新检查客户端失败：{err}"))?;
    let mut last_error = String::new();
    for endpoint in updater_release_api_fallback_urls(method) {
        for attempt in 1..=3 {
            ensure_update_not_cancelled()?;
            let response = client
                .get(&endpoint)
                .header(
                    reqwest::header::USER_AGENT,
                    format!("p-ai/{}", env!("CARGO_PKG_VERSION")),
                )
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    last_error = format!(
                        "请求更新接口失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                    );
                    continue;
                }
            };
            if !response.status().is_success() {
                last_error = format!(
                    "GitHub 更新接口返回异常状态码：{}（地址：{endpoint}，第 {attempt} 次）",
                    response.status().as_u16()
                );
                continue;
            }
            let payload = response
                .json::<GithubLatestReleasePayload>()
                .await
                .map_err(|err| {
                    format!(
                        "解析 GitHub 更新响应失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                    )
                })?;
            return Ok((payload, endpoint_access_mode(&endpoint).to_string()));
        }
    }
    Err(last_error)
}

async fn fetch_remote_changelog_markdown(origin: &str, method: GithubUpdateMethod) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(8))
        .build()
        .map_err(|err| format!("初始化更新日志客户端失败：{err}"))?;
    let mut last_error = String::new();
    for endpoint in updater_changelog_api_fallback_urls(origin, method) {
        for attempt in 1..=3 {
            ensure_update_not_cancelled()?;
            let response = client
                .get(&endpoint)
                .header(
                    reqwest::header::USER_AGENT,
                    format!("p-ai/{}", env!("CARGO_PKG_VERSION")),
                )
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    last_error = format!(
                        "请求更新日志接口失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                    );
                    continue;
                }
            };
            if !response.status().is_success() {
                last_error = format!(
                    "GitHub 更新日志接口返回异常状态码：{}（地址：{endpoint}，第 {attempt} 次）",
                    response.status().as_u16()
                );
                continue;
            }
            let bytes = response
                .bytes()
                .await
                .map_err(|err| {
                    format!(
                        "解析 GitHub 更新日志响应失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                    )
                })?;
            return String::from_utf8(bytes.to_vec()).map_err(|err| {
                format!(
                    "解析 GitHub 更新日志文本失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                )
            });
        }
    }
    Err(last_error)
}

struct CachedChangelogMarkdown {
    fetched_at: OffsetDateTime,
    markdown: String,
}

#[tauri::command]
async fn fetch_project_changelog_markdown() -> Result<String, String> {
    // 1 小时内已成功拉取过则直接返回缓存，避免反复请求 GitHub
    {
        let guard = CHANGELOG_MARKDOWN_CACHE
            .lock()
            .map_err(|err| format!("读取更新日志缓存失败：{err}"))?;
        if let Some(cached) = guard.as_ref() {
            if (now_utc() - cached.fetched_at).whole_hours() < CHANGELOG_FETCH_COOLDOWN_HOURS {
                runtime_log_info(format!(
                    "[更新日志] 命中缓存（{} 小时内），跳过远程拉取",
                    CHANGELOG_FETCH_COOLDOWN_HOURS
                ));
                return Ok(cached.markdown.clone());
            }
        }
    }
    let markdown = fetch_remote_changelog_markdown(
        UPDATER_GITHUB_CHANGELOG_REMOTE_RAW_ORIGIN,
        GithubUpdateMethod::Auto,
    )
    .await?;
    let mut guard = CHANGELOG_MARKDOWN_CACHE
        .lock()
        .map_err(|err| format!("写入更新日志缓存失败：{err}"))?;
    *guard = Some(CachedChangelogMarkdown {
        fetched_at: now_utc(),
        markdown: markdown.clone(),
    });
    Ok(markdown)
}

fn github_auto_update_cooldown_active(
    last_checked_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> bool {
    let Some(last_checked_at) = last_checked_at else {
        return false;
    };
    (now - last_checked_at).whole_hours() < GITHUB_AUTO_UPDATE_COOLDOWN_HOURS
}

fn should_skip_auto_update_check() -> Result<bool, String> {
    let guard = LAST_AUTO_UPDATE_CHECKED_AT
        .lock()
        .map_err(|err| format!("读取自动更新检查状态失败：{err}"))?;
    Ok(github_auto_update_cooldown_active(*guard, now_utc()))
}

fn mark_auto_update_check_now() -> Result<(), String> {
    let mut guard = LAST_AUTO_UPDATE_CHECKED_AT
        .lock()
        .map_err(|err| format!("记录自动更新检查状态失败：{err}"))?;
    *guard = Some(now_utc());
    Ok(())
}

fn read_last_auto_update_checked_at_rfc3339() -> Option<String> {
    LAST_AUTO_UPDATE_CHECKED_AT
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .map(format_offset_datetime_to_local_rfc3339)
}

fn current_skipped_update_version(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    state_read_config_cached(state.inner())
        .map(|mut config| {
            normalize_app_config(&mut config);
            config.skipped_github_update_version.trim().to_string()
        })
        .unwrap_or_default()
}

fn update_github_update_state<F>(updater: F)
where
    F: FnOnce(&mut GithubUpdateState),
{
    if let Ok(mut state) = GITHUB_UPDATE_STATE.lock() {
        updater(&mut state);
    }
}

fn snapshot_github_update_state() -> GithubUpdateState {
    GITHUB_UPDATE_STATE
        .lock()
        .map(|state| state.clone())
        .unwrap_or_default()
}

fn refresh_update_visibility(state: &mut GithubUpdateState) {
    let latest_version = state.latest_version.trim();
    let skipped_version = state.skipped_version.trim();
    state.has_visible_update = state.has_prepared_update
        && !latest_version.is_empty()
        && (skipped_version.is_empty() || latest_version != skipped_version);
}

fn sync_update_state_from_check_result(app: &AppHandle, result: &GithubUpdateInfo) {
    let skipped_version = current_skipped_update_version(app);
    update_github_update_state(|state| {
        let previously_prepared = state.has_prepared_update
            && !state.latest_version.trim().is_empty()
            && state.latest_version.trim() == result.latest_version.trim();
        state.stage = if previously_prepared {
            UPDATE_STAGE_READY.to_string()
        } else {
            "idle".to_string()
        };
        state.current_version = result.current_version.clone();
        state.latest_version = result.latest_version.clone();
        state.runtime_kind = result.runtime_kind.clone();
        state.release_notes = result.release_notes.clone();
        state.release_url = result.release_url.clone();
        state.published_at = result.published_at.clone();
        state.last_checked_at = read_last_auto_update_checked_at_rfc3339();
        state.last_error = None;
        state.skipped_version = skipped_version;
        state.has_prepared_update = previously_prepared;
        if !previously_prepared {
            state.prepared_at = None;
        }
        refresh_update_visibility(state);
    });
}

fn sync_update_state_from_progress(app: &AppHandle, payload: &UpdateProgressPayload) {
    let skipped_version = current_skipped_update_version(app);
    update_github_update_state(|state| {
        state.stage = payload.stage.clone();
        state.current_version = payload.current_version.clone().unwrap_or_default();
        if let Some(target_version) = payload.target_version.clone() {
            state.latest_version = target_version;
        }
        state.runtime_kind = payload.runtime_kind.clone();
        state.last_checked_at = read_last_auto_update_checked_at_rfc3339();
        state.skipped_version = skipped_version;
        match payload.stage.as_str() {
            UPDATE_STAGE_READY => {
                state.has_prepared_update = true;
                state.prepared_at = Some(format_offset_datetime_to_local_rfc3339(now_utc()));
                state.last_error = None;
            }
            UPDATE_STAGE_FAILED => {
                state.has_prepared_update = false;
                state.prepared_at = None;
                state.last_error = payload.error.clone().or_else(|| Some(payload.message.clone()));
            }
            UPDATE_STAGE_CANCELLED => {
                state.has_prepared_update = false;
                state.prepared_at = None;
                state.last_error = None;
            }
            UPDATE_STAGE_COMPLETED => {
                state.has_prepared_update = false;
                state.has_visible_update = false;
                state.prepared_at = None;
                state.last_error = None;
            }
            _ => {
                state.last_error = None;
            }
        }
        if payload.stage != UPDATE_STAGE_COMPLETED {
            refresh_update_visibility(state);
        }
    });
}

fn sync_update_state_from_skip_version(_app: &AppHandle, version: &str) {
    update_github_update_state(|state| {
        state.skipped_version = version.trim().to_string();
        refresh_update_visibility(state);
    });
}

fn build_skipped_auto_update_result(runtime_kind: UpdateRuntimeKind) -> GithubUpdateInfo {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    GithubUpdateInfo {
        current_version: current_version.clone(),
        latest_version: current_version,
        has_update: false,
        release_url: String::new(),
        update_source: "cooldown".to_string(),
        access_mode: "direct".to_string(),
        release_notes: String::new(),
        published_at: None,
        runtime_kind: runtime_kind.as_str().to_string(),
        can_force_update: true,
    }
}

#[tauri::command]
fn get_github_update_state(app: AppHandle) -> Result<GithubUpdateState, String> {
    let runtime = detect_update_runtime_paths()?;
    let skipped_version = current_skipped_update_version(&app);
    update_github_update_state(|state| {
        if state.current_version.trim().is_empty() {
            state.current_version = env!("CARGO_PKG_VERSION").to_string();
        }
        if state.runtime_kind.trim().is_empty() {
            state.runtime_kind = runtime.runtime_kind.as_str().to_string();
        }
        state.skipped_version = skipped_version;
        if state.last_checked_at.is_none() {
            state.last_checked_at = read_last_auto_update_checked_at_rfc3339();
        }
        refresh_update_visibility(state);
    });
    Ok(snapshot_github_update_state())
}

#[tauri::command]
async fn check_github_update(
    app: AppHandle,
    update_method: Option<String>,
    respect_cooldown: Option<bool>,
) -> Result<GithubUpdateInfo, String> {
    let method = GithubUpdateMethod::from_raw(update_method);
    let runtime = detect_update_runtime_paths()?;
    if respect_cooldown.unwrap_or(false) {
        if should_skip_auto_update_check()? {
            runtime_log_warn(format!(
                "[自动更新] 自动检查仍处于 {} 小时冷却期，本次跳过远端检查",
                GITHUB_AUTO_UPDATE_COOLDOWN_HOURS
            ));
            let result = build_skipped_auto_update_result(runtime.runtime_kind);
            sync_update_state_from_check_result(&app, &result);
            return Ok(result);
        }
        mark_auto_update_check_now()?;
    }
    let (payload, access_mode) = fetch_latest_release_payload(method).await?;
    let latest_version = payload
        .tag_name
        .as_deref()
        .or(payload.name.as_deref())
        .map(normalize_release_version)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "GitHub Release 未返回有效版本号".to_string())?;
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release_notes = match fetch_remote_changelog_markdown(
        UPDATER_GITHUB_CHANGELOG_LATEST_RAW_ORIGIN,
        method,
    )
    .await {
        Ok(notes) => notes,
        Err(err) => {
            runtime_log_error(format!("[自动更新] 远程更新日志读取失败：{err}"));
            payload.body.clone().unwrap_or_default()
        }
    };
    let result = GithubUpdateInfo {
        current_version: current_version.clone(),
        latest_version: latest_version.clone(),
        has_update: is_newer_version(&current_version, &latest_version),
        release_url: updater_release_page_url(
            payload
                .html_url
                .as_deref()
                .unwrap_or(UPDATER_GITHUB_RELEASE_PAGE_ORIGIN),
        ),
        update_source: "github".to_string(),
        access_mode,
        release_notes,
        published_at: payload.published_at,
        runtime_kind: runtime.runtime_kind.as_str().to_string(),
        can_force_update: true,
    };
    sync_update_state_from_check_result(&app, &result);
    Ok(result)
}

fn copy_file_with_parent(src: &StdPath, dest: &StdPath) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std_fs::create_dir_all(parent).map_err(|err| {
            format!("创建目录失败（{}）：{err}", parent.display())
        })?;
    }
    std_fs::copy(src, dest).map_err(|err| {
        format!(
            "复制文件失败（{} -> {}）：{err}",
            src.display(),
            dest.display()
        )
    })?;
    Ok(())
}

fn copy_file_with_retry(src: &StdPath, dest: &StdPath, log_path: &StdPath) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 1..=PORTABLE_REPLACE_MAX_RETRIES {
        if let Some(parent) = dest.parent() {
            if let Err(err) = std_fs::create_dir_all(parent) {
                last_err = format!("创建目录失败（{}）：{err}", parent.display());
                append_helper_log(log_path, &format!("[自动更新] 重试 {attempt}/{} 失败：{last_err}", PORTABLE_REPLACE_MAX_RETRIES));
                thread::sleep(StdDuration::from_millis(PORTABLE_REPLACE_RETRY_DELAY_MS));
                continue;
            }
        }
        match std_fs::copy(src, dest) {
            Ok(_) => return Ok(()),
            Err(err) => {
                let is_access_denied = err.raw_os_error() == Some(5)
                    || err.kind() == std::io::ErrorKind::PermissionDenied;
                last_err = format!("复制文件失败（{} -> {}）：{err}", src.display(), dest.display());
                if is_access_denied {
                    append_helper_log(
                        log_path,
                        &format!(
                            "[自动更新] 文件被占用，重试 {attempt}/{}：{last_err}",
                            PORTABLE_REPLACE_MAX_RETRIES
                        ),
                    );
                } else {
                    append_helper_log(
                        log_path,
                        &format!("[自动更新] 重试 {attempt}/{} 失败：{last_err}", PORTABLE_REPLACE_MAX_RETRIES),
                    );
                }
                if attempt < PORTABLE_REPLACE_MAX_RETRIES {
                    thread::sleep(StdDuration::from_millis(PORTABLE_REPLACE_RETRY_DELAY_MS));
                }
            }
        }
    }
    Err(last_err)
}

fn portable_pending_path(temp_root: &StdPath) -> StdPathBuf {
    temp_root.join(PORTABLE_PENDING_FILE_NAME)
}

fn write_portable_pending(temp_root: &StdPath, plan: &PortableUpdatePlan, backup_dir: &StdPath, failure_reason: &str) -> Result<(), String> {
    let pending = PortablePendingManualReplace {
        plan: plan.clone(),
        backup_dir: backup_dir.to_string_lossy().to_string(),
        failure_reason: failure_reason.to_string(),
        created_at: now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string()),
    };
    let json = serde_json::to_vec_pretty(&pending).map_err(|err| format!("序列化 pending 失败：{err}"))?;
    let path = portable_pending_path(temp_root);
    if let Some(parent) = path.parent() {
        std_fs::create_dir_all(parent).map_err(|err| format!("创建 pending 目录失败（{}）：{err}", parent.display()))?;
    }
    std_fs::write(&path, json).map_err(|err| format!("写入 pending 失败（{}）：{err}", path.display()))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn wait_for_parent_process_exit(parent_pid: Option<u32>, log_path: &StdPath) {
    let Some(pid) = parent_pid else {
        thread::sleep(StdDuration::from_millis(1800));
        return;
    };
    if pid == 0 {
        thread::sleep(StdDuration::from_millis(1800));
        return;
    }
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
    const SYNCHRONIZE: u32 = 0x00100000;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle == std::ptr::null_mut() {
            append_helper_log(log_path, &format!("[自动更新] 无法打开父进程 {pid}，可能已退出，等待 2s 后替换"));
            thread::sleep(StdDuration::from_millis(2000));
            return;
        }
        append_helper_log(log_path, &format!("[自动更新] 等待主进程 {pid} 退出，最长 {}ms", PORTABLE_PARENT_WAIT_TIMEOUT_MS));
        let wait = WaitForSingleObject(handle, PORTABLE_PARENT_WAIT_TIMEOUT_MS as u32);
        CloseHandle(handle);
        if wait == WAIT_OBJECT_0 {
            append_helper_log(log_path, "[自动更新] 主进程已退出，开始替换");
        } else {
            append_helper_log(log_path, "[自动更新] 等待超时，主进程可能仍在退出中，尝试替换");
        }
    }
    thread::sleep(StdDuration::from_millis(400));
}

#[cfg(not(target_os = "windows"))]
fn wait_for_parent_process_exit(parent_pid: Option<u32>, log_path: &StdPath) {
    let _ = parent_pid;
    append_helper_log(log_path, "[自动更新] 非 Windows 平台，等待 1.8s 后替换");
    thread::sleep(StdDuration::from_millis(1800));
}

fn compute_file_sha256(path: &StdPath) -> Result<String, String> {
    let mut file = StdFile::open(path)
        .map_err(|err| format!("打开文件失败（{}）：{err}", path.display()))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("读取文件失败（{}）：{err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(bytes_to_lower_hex(hasher.finalize()))
}

fn extract_zip_to_dir(
    zip_path: &StdPath,
    output_dir: &StdPath,
) -> Result<Vec<StdPathBuf>, String> {
    if output_dir.exists() {
        std_fs::remove_dir_all(output_dir).map_err(|err| {
            format!("清理 staging 目录失败（{}）：{err}", output_dir.display())
        })?;
    }
    std_fs::create_dir_all(output_dir).map_err(|err| {
        format!("创建 staging 目录失败（{}）：{err}", output_dir.display())
    })?;
    let file = StdFile::open(zip_path)
        .map_err(|err| format!("打开更新压缩包失败（{}）：{err}", zip_path.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|err| format!("解析 ZIP 更新包失败：{err}"))?;
    let mut files = Vec::new();
    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|err| format!("读取 ZIP 条目失败：{err}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| {
                let display = entry
                    .name()
                    .map(|name| name.into_owned())
                    .unwrap_or_else(|_| "<非 UTF-8 文件名>".to_string());
                format!("更新包中存在不安全路径：{display}")
            })?
            .to_path_buf();
        let out_path = output_dir.join(&enclosed);
        if entry.is_dir() {
            std_fs::create_dir_all(&out_path).map_err(|err| {
                format!("创建解压目录失败（{}）：{err}", out_path.display())
            })?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std_fs::create_dir_all(parent).map_err(|err| {
                format!("创建解压父目录失败（{}）：{err}", parent.display())
            })?;
        }
        let mut output = StdFile::create(&out_path).map_err(|err| {
            format!("创建解压文件失败（{}）：{err}", out_path.display())
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|err| {
            format!("写入解压文件失败（{}）：{err}", out_path.display())
        })?;
        files.push(enclosed);
    }
    if files.is_empty() {
        return Err("更新压缩包为空，无法继续".to_string());
    }
    Ok(files)
}

fn verify_staging_files(
    staging_dir: &StdPath,
    relative_files: &[StdPathBuf],
    target_exe_name: &str,
) -> Result<(), String> {
    let has_target_exe = relative_files.iter().any(|rel| {
        rel.file_name()
            .and_then(|v| v.to_str())
            .map(|name| name.eq_ignore_ascii_case(target_exe_name))
            .unwrap_or(false)
    });
    if !has_target_exe {
        return Err(format!("更新包缺少主程序文件：{target_exe_name}"));
    }
    for rel in relative_files {
        let full = staging_dir.join(rel);
        if !full.exists() {
            return Err(format!("staging 文件缺失：{}", full.display()));
        }
    }
    Ok(())
}

fn updater_temp_root(runtime: &UpdateRuntimePaths) -> StdPathBuf {
    runtime.data_dir.join("temp").join("updater")
}

fn ensure_update_temp_dirs(runtime: &UpdateRuntimePaths) -> Result<StdPathBuf, String> {
    let root = updater_temp_root(runtime);
    std_fs::create_dir_all(&root)
        .map_err(|err| format!("创建更新临时目录失败（{}）：{err}", root.display()))?;
    Ok(root)
}

fn should_cleanup_portable_temp_entry(path: &StdPath) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if path.is_dir() {
        return name.starts_with(PORTABLE_STAGING_DIR_PREFIX);
    }
    (name.starts_with(PORTABLE_HELPER_FILE_PREFIX) && name.ends_with(".exe"))
        || (name.starts_with(PORTABLE_PLAN_FILE_PREFIX) && name.ends_with(".json"))
        || (name.starts_with(PORTABLE_ZIP_FILE_PREFIX) && name.ends_with(".zip"))
}

fn cleanup_portable_update_temp_artifacts(temp_root: &StdPath) {
    // 若存在待手动替换的 pending，则保留其指向的 staging/zip，避免下次启动被清掉现场
    let pending_paths = read_pending_protected_paths(temp_root);
    let Ok(entries) = std_fs::read_dir(temp_root) else {
        return;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if !should_cleanup_portable_temp_entry(&path) {
            continue;
        }
        if pending_paths.iter().any(|p| p == &path) {
            continue;
        }
        if let Err(err) = remove_if_exists(&path) {
            runtime_log_error(format!(
                "[自动更新] 清理便携版更新临时文件失败: path={}，error={}",
                path.display(),
                err
            ));
        }
    }
}

fn read_pending_protected_paths(temp_root: &StdPath) -> Vec<StdPathBuf> {
    let pending_path = portable_pending_path(temp_root);
    let Ok(raw) = std_fs::read(&pending_path) else {
        return vec![];
    };
    let Ok(pending) = serde_json::from_slice::<PortablePendingManualReplace>(&raw) else {
        return vec![];
    };
    let mut out = vec![];
    out.push(StdPathBuf::from(&pending.plan.staging_dir));
    out.push(StdPathBuf::from(&pending.plan.zip_path));
    out.push(pending_path);
    out
}

fn cleanup_portable_update_temp_artifacts_for_current_runtime() -> Result<(), String> {
    let runtime = detect_update_runtime_paths()?;
    if runtime.runtime_kind != UpdateRuntimeKind::Portable {
        return Ok(());
    }
    cleanup_portable_update_temp_artifacts(&updater_temp_root(&runtime));
    Ok(())
}

fn write_portable_plan(plan_path: &StdPath, plan: &PortableUpdatePlan) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(plan)
        .map_err(|err| format!("序列化便携版更新计划失败：{err}"))?;
    if let Some(parent) = plan_path.parent() {
        std_fs::create_dir_all(parent).map_err(|err| {
            format!("创建更新计划目录失败（{}）：{err}", parent.display())
        })?;
    }
    std_fs::write(plan_path, json).map_err(|err| {
        format!("写入便携版更新计划失败（{}）：{err}", plan_path.display())
    })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_command_line_arg(arg: &std::ffi::OsStr) -> String {
    use std::os::windows::ffi::OsStrExt;

    let raw = String::from_utf16_lossy(&arg.encode_wide().collect::<Vec<u16>>());
    if raw.is_empty() {
        return "\"\"".to_string();
    }
    if !raw.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return raw;
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for ch in raw.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                if backslashes > 0 {
                    quoted.push_str(&"\\".repeat(backslashes));
                    backslashes = 0;
                }
                quoted.push(ch);
            }
        }
    }
    if backslashes > 0 {
        quoted.push_str(&"\\".repeat(backslashes * 2));
    }
    quoted.push('"');
    quoted
}

#[cfg(target_os = "windows")]
fn spawn_detached_hidden(exe: &StdPath, args: &[OsString]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, CREATE_NO_WINDOW, DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW,
    };

    let mut command_line = windows_command_line_arg(exe.as_os_str());
    for arg in args {
        command_line.push(' ');
        command_line.push_str(&windows_command_line_arg(arg.as_os_str()));
    }

    let mut exe_wide = exe.as_os_str().encode_wide().collect::<Vec<u16>>();
    exe_wide.push(0);
    let mut command_line_wide = command_line.encode_utf16().collect::<Vec<u16>>();
    command_line_wide.push(0);

    let mut startup_info: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let success = unsafe {
        CreateProcessW(
            exe_wide.as_ptr(),
            command_line_wide.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            DETACHED_PROCESS | CREATE_NO_WINDOW,
            std::ptr::null(),
            std::ptr::null(),
            &startup_info,
            &mut process_info,
        )
    };

    if success == 0 {
        return Err(format!(
            "启动后台进程失败：CreateProcessW 失败，error={}",
            std::io::Error::last_os_error()
        ));
    }

    unsafe {
        CloseHandle(process_info.hProcess);
        CloseHandle(process_info.hThread);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_detached_hidden(exe: &StdPath, args: &[OsString]) -> Result<(), String> {
    let mut command = std::process::Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command.spawn().map_err(|err| format!("启动后台进程失败：{err}"))?;
    Ok(())
}

fn emit_update_checking_progress(
    app: &AppHandle,
    runtime_kind: UpdateRuntimeKind,
    current_version: &str,
    message: String,
    target_version: Option<String>,
) {
    emit_update_progress(
        app,
        build_update_progress(
            runtime_kind,
            UPDATE_STAGE_CHECKING,
            message,
            Some(current_version.to_string()),
            target_version,
            None,
            None,
            None,
        ),
    );
}

fn build_manifest_updater(
    app: &AppHandle,
    runtime_kind: UpdateRuntimeKind,
    target: Option<&str>,
    endpoint: &str,
    force: bool,
) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder().pubkey(updater_public_key()?);
    if let Some(target) = target {
        builder = builder.target(target.to_string());
    }
    #[cfg(target_os = "windows")]
    {
        // NSIS 自动更新如果不显式传入当前安装目录，安装器可能会回落到默认目录。
        // `/D=...` 需要作为最后一个 NSIS 参数传入，tauri-plugin-updater 会把额外 installer_args
        // 追加在内部参数之后，这里正好满足要求。
        if runtime_kind == UpdateRuntimeKind::Installer {
            let runtime = detect_update_runtime_paths()?;
            let app_before_exit = app.clone();
            builder = builder.on_before_exit(move || {
                shutdown_background_services_before_windows_updater_exit(app_before_exit.clone());
            });
            builder = builder.installer_arg(std::ffi::OsString::from(format!(
                "/D={}",
                runtime.exe_dir.display()
            )));
        }
    }
    if force {
        builder = builder.version_comparator(|_, _| true);
    }
    let endpoint = reqwest::Url::parse(endpoint).map_err(|err| format!("解析更新端点失败：{err}"))?;
    builder
        .endpoints(vec![endpoint])
        .map_err(|err| format!("配置更新端点失败：{err}"))?
        .build()
        .map_err(|err| format!("构建更新检查器失败：{err}"))
}

async fn run_manifest_update_check(
    updater: tauri_plugin_updater::Updater,
    check_failed_prefix: &str,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    match tokio::time::timeout(
        StdDuration::from_secs(GITHUB_UPDATE_CHECK_TIMEOUT_SECONDS),
        updater.check(),
    )
    .await
    {
        Ok(Ok(update)) => Ok(update),
        Ok(Err(err)) => Err(format!("{check_failed_prefix}：{err}")),
        Err(_) => Err(format!(
            "{check_failed_prefix}：检查超时（{} 秒）",
            GITHUB_UPDATE_CHECK_TIMEOUT_SECONDS
        )),
    }
}

async fn check_updater_with_manifest_fallbacks(
    app: &AppHandle,
    runtime_kind: UpdateRuntimeKind,
    target: Option<String>,
    manifest_origin: &str,
    method: GithubUpdateMethod,
    force: bool,
    current_version: &str,
    checking_message: &str,
    check_failed_prefix: &str,
) -> Result<tauri_plugin_updater::Update, String> {
    emit_update_checking_progress(
        app,
        runtime_kind,
        current_version,
        checking_message.to_string(),
        None,
    );
    let candidates = updater_manifest_fallbacks(manifest_origin, method);
    let candidate_count = candidates.len();
    for (index, candidate) in candidates.into_iter().enumerate() {
        ensure_update_not_cancelled()?;
        emit_update_checking_progress(
            app,
            runtime_kind,
            current_version,
            format!("正在尝试{}", candidate.display_name),
            None,
        );
        runtime_log_info(format!(
            "[自动更新] 开始，任务=检查更新清单，阶段=checking，运行类型={}，线路={}，force={force}",
            runtime_kind.as_str(),
            candidate.log_name,
        ));
        let started_at = StdInstant::now();
        let updater = build_manifest_updater(
            app,
            runtime_kind,
            target.as_deref(),
            &candidate.endpoint,
            force,
        );
        let update_result = match updater {
            Ok(updater) => run_manifest_update_check(updater, check_failed_prefix).await,
            Err(err) => Err(err),
        };
        let update = match update_result {
            Ok(Some(update)) if force || is_newer_version(current_version, &update.version) => update,
            Ok(_) => {
                runtime_log_info(format!(
                    "[自动更新] 完成，任务=检查更新清单，阶段=checking，运行类型={}，线路={}，结果=当前已是最新版本，耗时={}ms",
                    runtime_kind.as_str(),
                    candidate.log_name,
                    started_at.elapsed().as_millis(),
                ));
                return Err("当前没有可安装的更新".to_string());
            }
            Err(error) => {
                runtime_log_warn(format!(
                    "[自动更新] 失败，任务=检查更新清单，阶段=checking，运行类型={}，线路={}，耗时={}ms，error={error}",
                    runtime_kind.as_str(),
                    candidate.log_name,
                    started_at.elapsed().as_millis(),
                ));
                let message = if index + 1 < candidate_count {
                    format!("{}无法连接，正在换下一条线路", candidate.display_name)
                } else {
                    format!("{}无法连接", candidate.display_name)
                };
                emit_update_checking_progress(app, runtime_kind, current_version, message, None);
                continue;
            }
        };
        runtime_log_info(format!(
            "[自动更新] 完成，任务=检查更新清单，阶段=checking，运行类型={}，线路={}，结果=发现更新，target_version={}，耗时={}ms",
            runtime_kind.as_str(),
            candidate.log_name,
            update.version,
            started_at.elapsed().as_millis(),
        ));
        emit_update_checking_progress(
            app,
            runtime_kind,
            current_version,
            format!("正在使用{}更新", candidate.display_name),
            Some(update.version.clone()),
        );
        return Ok(update);
    }
    Err(format!(
        "{check_failed_prefix}：所有线路均无法连接，请稍后重试或打开 Releases"
    ))
}

async fn download_update_with_proxy_cursor<C, D>(
    runtime: &UpdateRuntimePaths,
    update: &tauri_plugin_updater::Update,
    method: GithubUpdateMethod,
    mut on_chunk: C,
    on_download_finish: D,
    download_failed_prefix: &str,
) -> Result<Vec<u8>, String>
where
    C: FnMut(usize, Option<u64>),
    D: FnOnce(),
{
    use futures_util::StreamExt as _;

    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(
            GITHUB_UPDATE_DOWNLOAD_TIMEOUT_SECONDS,
        ))
        .build()
        .map_err(|err| format!("初始化更新下载客户端失败：{err}"))?;
    let origin_url = strip_known_proxy_prefix(update.download_url.as_str()).to_string();
    let cursor = read_github_update_proxy_cursor(runtime);
    let route_name = github_update_download_route_name(method, cursor);
    let endpoint = updater_download_endpoint(&origin_url, method, cursor);
    let started_at = StdInstant::now();
    runtime_log_info(format!(
        "[自动更新] 开始，任务=下载更新，阶段=downloading，线路={}，游标={cursor}",
        route_name,
    ));
    ensure_update_not_cancelled()?;
    let response = match client
        .get(&endpoint)
        .header(
            reqwest::header::USER_AGENT,
            format!("p-ai/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            return Err(finish_failed_update_download(
                runtime,
                method,
                cursor,
                format!("{download_failed_prefix}（地址：{endpoint}）：{err}"),
            )?);
        }
    };
    if !response.status().is_success() {
        return Err(finish_failed_update_download(
            runtime,
            method,
            cursor,
            format!(
                "{download_failed_prefix}（地址：{endpoint}）：HTTP {}",
                response.status().as_u16()
            ),
        )?);
    }
    let content_length = response.content_length();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        ensure_update_not_cancelled()?;
        match chunk {
            Ok(chunk) => {
                on_chunk(chunk.len(), content_length);
                bytes.extend_from_slice(&chunk);
            }
            Err(err) => {
                return Err(finish_failed_update_download(
                    runtime,
                    method,
                    cursor,
                    format!("{download_failed_prefix}（地址：{endpoint}）：{err}"),
                )?);
            }
        }
    }
    ensure_update_not_cancelled()?;
    if let Some(expected) = content_length {
        let actual = bytes.len() as u64;
        if actual != expected {
            return Err(finish_failed_update_download(
                runtime,
                method,
                cursor,
                format!(
                    "{download_failed_prefix}（地址：{endpoint}）：下载大小不完整，期望 {expected} 字节，实际 {actual} 字节"
                ),
            )?);
        }
    }
    on_download_finish();
    runtime_log_info(format!(
        "[自动更新] 完成，任务=下载更新，阶段=downloading，线路={}，字节数={}，耗时={}ms",
        route_name,
        bytes.len(),
        started_at.elapsed().as_millis(),
    ));
    Ok(bytes)
}

async fn prepare_installer_update(
    app: &AppHandle,
    runtime: &UpdateRuntimePaths,
    force: bool,
    method: GithubUpdateMethod,
) -> Result<(), String> {
    ensure_update_not_cancelled()?;
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let runtime_kind = runtime.runtime_kind;
    let update = check_updater_with_manifest_fallbacks(
        app,
        runtime_kind,
        None,
        UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN,
        method,
        force,
        &current_version,
        "正在检查是否有更新",
        "检查安装版更新失败",
    )
    .await?;
    let target_version = update.version.clone();
    let download_route_name =
        github_update_download_route_name(method, read_github_update_proxy_cursor(runtime)).to_string();
    let download_progress_route_name = download_route_name.clone();
    let download_progress_current_version = current_version.clone();
    let download_progress_target_version = target_version.clone();
    let install_progress_current_version = current_version.clone();
    let install_progress_target_version = target_version.clone();
    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    emit_update_progress(
        app,
        build_update_progress(
            runtime_kind,
            UPDATE_STAGE_DOWNLOADING,
            format!("正在使用{download_route_name}更新，正在下载安装包 {target_version}"),
            Some(current_version.clone()),
            Some(target_version.clone()),
            Some(0),
            None,
            None,
        ),
    );
    let bytes = download_update_with_proxy_cursor(
        runtime,
        &update,
        method,
        {
            let downloaded = downloaded.clone();
            move |chunk_length, content_length| {
                let total =
                    downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed) + chunk_length as u64;
                emit_update_progress(
                    app,
                    build_update_progress(
                        runtime_kind,
                        UPDATE_STAGE_DOWNLOADING,
                        format!(
                            "正在使用{}更新，正在下载安装包 {}",
                            download_progress_route_name,
                            download_progress_target_version
                        ),
                        Some(download_progress_current_version.clone()),
                        Some(download_progress_target_version.clone()),
                        Some(total),
                        content_length,
                        None,
                    ),
                );
            }
        },
        {
            let downloaded = downloaded.clone();
            move || {
                let total = downloaded.load(Ordering::Relaxed);
                emit_update_progress(
                    app,
                    build_update_progress(
                        runtime_kind,
                        UPDATE_STAGE_INSTALLING,
                        format!("安装包下载完成，正在安装 {install_progress_target_version}"),
                        Some(install_progress_current_version.clone()),
                        Some(install_progress_target_version.clone()),
                        Some(total),
                        None,
                        None,
                    ),
                );
            }
        },
        "下载安装版更新失败",
    )
    .await
    .map_err(|err| format!("下载安装版更新失败：{err}"))?;
    ensure_update_not_cancelled()?;
    store_prepared_github_update(PreparedGithubUpdate::Installer(PreparedInstallerUpdate {
        update,
        bytes,
        current_version: current_version.clone(),
        target_version: target_version.clone(),
    }))?;
    emit_update_progress(
        app,
        build_update_progress(
            runtime_kind,
            UPDATE_STAGE_READY,
            format!("安装版更新 {target_version} 已下载完成，点击“更新并重启”开始安装"),
            Some(current_version),
            Some(target_version),
            None,
            None,
            None,
        ),
    );
    Ok(())
}

async fn prepare_portable_update(
    app: &AppHandle,
    runtime: &UpdateRuntimePaths,
    force: bool,
    method: GithubUpdateMethod,
) -> Result<(), String> {
    ensure_update_not_cancelled()?;
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let update = check_updater_with_manifest_fallbacks(
        app,
        runtime.runtime_kind,
        Some(current_portable_target()),
        UPDATER_GITHUB_PORTABLE_MANIFEST_ORIGIN,
        method,
        force,
        &current_version,
        "正在检查是否有更新",
        "检查便携版更新失败",
    )
    .await?;
    let target_version = update.version.clone();
    let download_route_name =
        github_update_download_route_name(method, read_github_update_proxy_cursor(runtime)).to_string();
    let download_progress_route_name = download_route_name.clone();
    let download_progress_current_version = current_version.clone();
    let download_progress_target_version = target_version.clone();
    let verify_progress_current_version = current_version.clone();
    let verify_progress_target_version = target_version.clone();
    let temp_root = ensure_update_temp_dirs(&runtime)?;
    cleanup_portable_update_temp_artifacts(&temp_root);
    let zip_path = temp_root.join(format!("p-ai-portable-{}.zip", target_version));
    let staging_dir = temp_root.join(format!("staging-{}", target_version));
    let helper_copy_path = temp_root.join(format!("portable-helper-{}.exe", Uuid::new_v4()));
    let backup_root = temp_root.join("backups");
    let plan_path = temp_root.join(format!("portable-plan-{}.json", Uuid::new_v4()));
    let log_path = temp_root.join("portable-update.log");
    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    emit_update_progress(
        app,
        build_update_progress(
            runtime.runtime_kind,
            UPDATE_STAGE_DOWNLOADING,
            format!("正在使用{download_route_name}更新，正在下载便携版更新 {target_version}"),
            Some(current_version.clone()),
            Some(target_version.clone()),
            Some(0),
            None,
            None,
        ),
    );
    let bytes = download_update_with_proxy_cursor(
        runtime,
        &update,
        method,
        {
            let downloaded = downloaded.clone();
            move |chunk_length, content_length| {
                let total =
                    downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed) + chunk_length as u64;
                emit_update_progress(
                    app,
                    build_update_progress(
                        runtime.runtime_kind,
                        UPDATE_STAGE_DOWNLOADING,
                        format!(
                            "正在使用{}更新，正在下载便携版更新 {}",
                            download_progress_route_name,
                            download_progress_target_version
                        ),
                        Some(download_progress_current_version.clone()),
                        Some(download_progress_target_version.clone()),
                        Some(total),
                        content_length,
                        None,
                    ),
                );
            }
        },
        {
            let downloaded = downloaded.clone();
            move || {
                let total = downloaded.load(Ordering::Relaxed);
                emit_update_progress(
                    app,
                    build_update_progress(
                        runtime.runtime_kind,
                        UPDATE_STAGE_VERIFYING,
                        format!("便携版更新 {verify_progress_target_version} 下载完成，正在校验"),
                        Some(verify_progress_current_version.clone()),
                        Some(verify_progress_target_version.clone()),
                        Some(total),
                        None,
                        None,
                    ),
                );
            }
        },
        "下载便携版更新失败",
    )
    .await
    .map_err(|err| format!("下载便携版更新失败：{err}"))?;
    ensure_update_not_cancelled()?;
    // 写入更新包（可达数十 MB）与解压是同步 IO + CPU 密集操作，移到 blocking 线程池执行。
    let zip_path_for_write = zip_path.clone();
    tokio::task::spawn_blocking(move || {
        std_fs::write(&zip_path_for_write, &bytes)
    })
    .await
    .map_err(|err| format!("写入便携版更新包任务失败：{err}"))?
    .map_err(|err| {
        format!("写入便携版更新包失败（{}）：{err}", zip_path.display())
    })?;
    emit_update_progress(
        app,
        build_update_progress(
            runtime.runtime_kind,
            UPDATE_STAGE_PREPARING,
            "正在准备便携版 staging 目录",
            Some(current_version.clone()),
            Some(target_version.clone()),
            None,
            None,
            None,
        ),
    );
    ensure_update_not_cancelled()?;
    let zip_path_for_extract = zip_path.clone();
    let staging_dir_for_extract = staging_dir.clone();
    let extracted_files = tokio::task::spawn_blocking(move || {
        extract_zip_to_dir(&zip_path_for_extract, &staging_dir_for_extract)
    })
    .await
    .map_err(|err| format!("解压便携版更新包任务失败：{err}"))??;
    let target_exe_name = runtime
        .exe_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("无法解析主程序文件名：{}", runtime.exe_path.display()))?
        .to_string();
    verify_staging_files(&staging_dir, &extracted_files, &target_exe_name)?;
    ensure_update_not_cancelled()?;
    copy_file_with_parent(&runtime.exe_path, &helper_copy_path)?;
    let helper_hash = compute_file_sha256(&helper_copy_path)?;
    let current_hash = compute_file_sha256(&runtime.exe_path)?;
    if helper_hash != current_hash {
        return Err("临时 helper 校验失败，已中止便携版更新".to_string());
    }
    let plan = PortableUpdatePlan {
        target_dir: runtime.exe_dir.to_string_lossy().to_string(),
        target_exe_name,
        staging_dir: staging_dir.to_string_lossy().to_string(),
        backup_root: backup_root.to_string_lossy().to_string(),
        temp_root: temp_root.to_string_lossy().to_string(),
        zip_path: zip_path.to_string_lossy().to_string(),
        log_path: log_path.to_string_lossy().to_string(),
        parent_pid: Some(std::process::id()),
        target_version: Some(target_version.clone()),
    };
    write_portable_plan(&plan_path, &plan)?;
    store_prepared_github_update(PreparedGithubUpdate::Portable(PreparedPortableUpdate {
        runtime_kind: runtime.runtime_kind,
        current_version: current_version.clone(),
        target_version: target_version.clone(),
        helper_copy_path,
        plan_path,
    }))?;
    emit_update_progress(
        app,
        build_update_progress(
            runtime.runtime_kind,
            UPDATE_STAGE_READY,
            format!("便携版更新 {target_version} 已下载完成，点击“更新并重启”开始替换"),
            Some(current_version),
            Some(target_version),
            None,
            None,
            None,
        ),
    );
    Ok(())
}

async fn run_auto_update_cycle(app: AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = app;
        return;
    }
    if UPDATE_IN_PROGRESS.load(Ordering::SeqCst) {
        return;
    }
    let state = snapshot_github_update_state();
    if state.has_prepared_update && state.has_visible_update {
        return;
    }
    let result = check_github_update(app.clone(), None, Some(true)).await;
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            runtime_log_warn(format!("[自动更新] 自动检查失败：error={err}"));
            update_github_update_state(|state| {
                state.last_error = Some(err.clone());
                state.last_checked_at = read_last_auto_update_checked_at_rfc3339();
            });
            return;
        }
    };
    let skipped_version = current_skipped_update_version(&app);
    if !result.has_update {
        return;
    }
    if !skipped_version.is_empty() && skipped_version == result.latest_version {
        runtime_log_info(format!(
            "[自动更新] 跳过，latest_version={}，reason=版本已被用户跳过",
            result.latest_version
        ));
        return;
    }
    if state.has_prepared_update && state.latest_version == result.latest_version {
        return;
    }
    if let Err(err) = start_github_update(app.clone(), false, None).await {
        runtime_log_warn(format!(
            "[自动更新] 静默准备失败：latest_version={}，error={err}",
            result.latest_version
        ));
    }
}

fn start_github_auto_update_worker(app: AppHandle) {
    if GITHUB_AUTO_UPDATE_WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(
            GITHUB_AUTO_UPDATE_STARTUP_DELAY_SECONDS,
        ))
        .await;
        loop {
            run_auto_update_cycle(app.clone()).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(
                GITHUB_AUTO_UPDATE_POLL_MINUTES * 60,
            ))
            .await;
        }
    });
}

#[tauri::command]
async fn start_github_update(
    app: AppHandle,
    force: bool,
    update_method: Option<String>,
) -> Result<(), String> {
    let _guard = UpdateInProgressGuard::acquire()?;
    reset_update_cancel_requested();
    clear_prepared_github_update();
    let method = GithubUpdateMethod::from_raw(update_method);
    let runtime = detect_update_runtime_paths()?;
    let result = match runtime.runtime_kind {
        UpdateRuntimeKind::Installer => {
            prepare_installer_update(&app, &runtime, force, method).await
        }
        UpdateRuntimeKind::Portable => {
            prepare_portable_update(&app, &runtime, force, method).await
        }
    };
    if let Err(err) = &result {
        let cancelled = is_update_cancelled_error(err);
        emit_update_progress(
            &app,
            build_update_progress(
                runtime.runtime_kind,
                if cancelled {
                    UPDATE_STAGE_CANCELLED
                } else {
                    UPDATE_STAGE_FAILED
                },
                if cancelled {
                    "已取消更新".to_string()
                } else {
                    format!("更新失败：{err}")
                },
                Some(env!("CARGO_PKG_VERSION").to_string()),
                None,
                None,
                None,
                if cancelled { None } else { Some(err.clone()) },
            ),
        );
    }
    result
}

#[tauri::command]
async fn cancel_github_update() -> Result<(), String> {
    if !UPDATE_IN_PROGRESS.load(Ordering::SeqCst) {
        return Err("当前没有正在执行的更新任务".to_string());
    }
    request_update_cancel();
    Ok(())
}

#[tauri::command]
async fn apply_prepared_github_update(app: AppHandle) -> Result<(), String> {
    let _guard = UpdateInProgressGuard::acquire()?;
    let prepared = take_prepared_github_update()?;
    match prepared {
        PreparedGithubUpdate::Installer(prepared) => {
            emit_update_progress(
                &app,
                build_update_progress(
                    UpdateRuntimeKind::Installer,
                    UPDATE_STAGE_INSTALLING,
                    format!("正在安装更新 {}", prepared.target_version),
                    Some(prepared.current_version.clone()),
                    Some(prepared.target_version.clone()),
                    None,
                    None,
                    None,
                ),
            );
            if let Err(err) = prepared.update.install(&prepared.bytes) {
                let message = format!("安装安装版更新失败：{err}");
                emit_update_progress(
                    &app,
                    build_update_progress(
                        UpdateRuntimeKind::Installer,
                        UPDATE_STAGE_FAILED,
                        format!("更新失败：{message}"),
                        Some(prepared.current_version),
                        Some(prepared.target_version),
                        None,
                        None,
                        Some(message.clone()),
                    ),
                );
                return Err(message);
            }
            emit_update_progress(
                &app,
                build_update_progress(
                    UpdateRuntimeKind::Installer,
                    UPDATE_STAGE_COMPLETED,
                    format!("安装版更新 {} 已安装，准备重启", prepared.target_version),
                    Some(prepared.current_version.clone()),
                    Some(prepared.target_version.clone()),
                    None,
                    None,
                    None,
                ),
            );
            if !graceful_shutdown_background_services_with_timeout(&app).await {
                let message = "自动关闭失败，请手动关闭应用重启".to_string();
                show_background_shutdown_timeout_dialog(&app);
                emit_update_progress(
                    &app,
                    build_update_progress(
                        UpdateRuntimeKind::Installer,
                        UPDATE_STAGE_FAILED,
                        format!("更新失败：{message}"),
                        Some(prepared.current_version),
                        Some(prepared.target_version),
                        None,
                        None,
                        Some(message.clone()),
                    ),
                );
                return Err(message);
            }
            app.restart()
        }
        PreparedGithubUpdate::Portable(prepared) => {
            if !graceful_shutdown_background_services_with_timeout(&app).await {
                let message = "自动关闭失败，请手动关闭应用重启".to_string();
                show_background_shutdown_timeout_dialog(&app);
                emit_update_progress(
                    &app,
                    build_update_progress(
                        prepared.runtime_kind,
                        UPDATE_STAGE_FAILED,
                        format!("更新失败：{message}"),
                        Some(prepared.current_version),
                        Some(prepared.target_version),
                        None,
                        None,
                        Some(message.clone()),
                    ),
                );
                return Err(message);
            }
            let helper_args = vec![
                OsString::from(PORTABLE_HELPER_FLAG),
                prepared.plan_path.as_os_str().to_os_string(),
            ];
            if let Err(err) = spawn_detached_hidden(&prepared.helper_copy_path, &helper_args) {
                let message = format!("启动便携版更新助手失败：{err}");
                emit_update_progress(
                    &app,
                    build_update_progress(
                        prepared.runtime_kind,
                        UPDATE_STAGE_FAILED,
                        format!("更新失败：{message}"),
                        Some(prepared.current_version),
                        Some(prepared.target_version),
                        None,
                        None,
                        Some(message.clone()),
                    ),
                );
                return Err(message);
            }
            emit_update_progress(
                &app,
                build_update_progress(
                    prepared.runtime_kind,
                    UPDATE_STAGE_REPLACING,
                    format!("便携版更新 {} 已准备完成，程序即将退出并完成替换", prepared.target_version),
                    Some(prepared.current_version.clone()),
                    Some(prepared.target_version.clone()),
                    None,
                    None,
                    None,
                ),
            );
            app.exit(0);
            Ok(())
        }
    }
}

#[tauri::command]
async fn get_portable_pending_manual_replace() -> Result<Option<PortablePendingManualReplace>, String> {
    let runtime = detect_update_runtime_paths()?;
    if runtime.runtime_kind != UpdateRuntimeKind::Portable {
        return Ok(None);
    }
    let temp_root = updater_temp_root(&runtime);
    let pending_path = portable_pending_path(&temp_root);
    if !pending_path.exists() {
        return Ok(None);
    }
    let raw = std_fs::read(&pending_path).map_err(|err| format!("读取 pending 失败（{}）：{err}", pending_path.display()))?;
    let pending = serde_json::from_slice::<PortablePendingManualReplace>(&raw).map_err(|err| format!("解析 pending 失败：{err}"))?;
    Ok(Some(pending))
}

#[tauri::command]
async fn dismiss_portable_pending_manual_replace() -> Result<(), String> {
    let runtime = detect_update_runtime_paths()?;
    if runtime.runtime_kind != UpdateRuntimeKind::Portable {
        // 非便携版无 pending，直接视为已关闭，保持与 get/retry 一致的守卫
        return Ok(());
    }
    let temp_root = updater_temp_root(&runtime);
    let pending_path = portable_pending_path(&temp_root);
    let _ = remove_if_exists(&pending_path);
    Ok(())
}

#[tauri::command]
async fn retry_portable_pending_manual_replace(app: AppHandle) -> Result<(), String> {
    let runtime = detect_update_runtime_paths()?;
    if runtime.runtime_kind != UpdateRuntimeKind::Portable {
        return Err("当前不是便携版，无需重试".to_string());
    }
    let temp_root = updater_temp_root(&runtime);
    let pending_path = portable_pending_path(&temp_root);
    if !pending_path.exists() {
        return Err("没有待重试的更新任务".to_string());
    }
    let raw = std_fs::read(&pending_path).map_err(|err| format!("读取 pending 失败：{err}"))?;
    let pending = serde_json::from_slice::<PortablePendingManualReplace>(&raw).map_err(|err| format!("解析 pending 失败：{err}"))?;
    // 重试前先优雅关闭后台服务，释放句柄以提升替换成功率（与首次 apply 路径一致）
    if !graceful_shutdown_background_services_with_timeout(&app).await {
        let message = "自动关闭失败，请手动关闭应用重启".to_string();
        show_background_shutdown_timeout_dialog(&app);
        emit_update_progress(
            &app,
            build_update_progress(
                runtime.runtime_kind,
                UPDATE_STAGE_FAILED,
                format!("重试失败：{message}"),
                Some(env!("CARGO_PKG_VERSION").to_string()),
                pending.plan.target_version.clone(),
                None,
                None,
                Some(message.clone()),
            ),
        );
        return Err(message);
    }
    // 复用原 staging 重试替换（不重新下载），成功后由 helper 清理 pending
    let plan_path = temp_root.join(format!("{}retry.json", PORTABLE_PLAN_FILE_PREFIX));
    let mut plan = pending.plan.clone();
    plan.parent_pid = Some(std::process::id());
    write_portable_plan(&plan_path, &plan)?;
    let helper_copy = temp_root.join(format!("portable-helper-retry-{}.exe", Uuid::new_v4()));
    copy_file_with_parent(&runtime.exe_path, &helper_copy)?;
    let helper_args = vec![OsString::from(PORTABLE_HELPER_FLAG), plan_path.as_os_str().to_os_string()];
    spawn_detached_hidden(&helper_copy, &helper_args)?;
    emit_update_progress(
        &app,
        build_update_progress(
            runtime.runtime_kind,
            UPDATE_STAGE_REPLACING,
            format!("正在重试便携版替换，目标 {}", pending.plan.target_version.clone().unwrap_or_default()),
            Some(env!("CARGO_PKG_VERSION").to_string()),
            pending.plan.target_version.clone(),
            None,
            None,
            None,
        ),
    );
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn open_portable_pending_dir(kind: String) -> Result<(), String> {
    let runtime = detect_update_runtime_paths()?;
    let temp_root = updater_temp_root(&runtime);
    let pending_path = portable_pending_path(&temp_root);
    let raw = std_fs::read(&pending_path).map_err(|err| format!("读取 pending 失败：{err}"))?;
    let pending = serde_json::from_slice::<PortablePendingManualReplace>(&raw).map_err(|err| format!("解析 pending 失败：{err}"))?;
    // 只提供一个目录入口：打开更新暂存目录，便于用户手动替换
    let _ = pending;
    let target: StdPathBuf = temp_root.to_path_buf();
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&target).spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&target).spawn();
    }
    let _ = kind;
    Ok(())
}

fn append_helper_log(log_path: &StdPath, line: &str) {
    if let Some(parent) = log_path.parent() {
        let _ = std_fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = writeln!(file, "{}", line);
    }
}

fn remove_if_exists(path: &StdPath) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        std_fs::remove_dir_all(path)
            .map_err(|err| format!("删除目录失败（{}）：{err}", path.display()))
    } else {
        std_fs::remove_file(path)
            .map_err(|err| format!("删除文件失败（{}）：{err}", path.display()))
    }
}

fn prune_old_backup_dirs(backup_root: &StdPath) {
    let Ok(entries) = std_fs::read_dir(backup_root) else {
        return;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, stale) in dirs.into_iter().skip(2) {
        let _ = std_fs::remove_dir_all(stale);
    }
}

fn collect_relative_files(root: &StdPath) -> Result<Vec<StdPathBuf>, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|err| format!("遍历目录失败（{}）：{err}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|err| format!("解析相对路径失败：{err}"))?
            .to_path_buf();
        files.push(rel);
    }
    files.sort();
    Ok(files)
}

fn restore_backup_files(
    target_dir: &StdPath,
    backup_dir: &StdPath,
    replaced_files: &[StdPathBuf],
    new_files: &[StdPathBuf],
) -> Result<(), String> {
    let mut first_err: Option<String> = None;
    for rel in new_files {
        let target = target_dir.join(rel);
        if let Err(err) = remove_if_exists(&target) {
            if first_err.is_none() {
                first_err = Some(err);
            }
        }
    }
    for rel in replaced_files {
        let backup = backup_dir.join(rel);
        let target = target_dir.join(rel);
        // 回滚也要带重试，避免再次撞锁导致静默失败
        let temp_root = target_dir.join("data").join("temp").join("updater");
        let log_path = temp_root.join("portable-update.log");
        if let Err(err) = copy_file_with_retry(&backup, &target, &log_path) {
            if first_err.is_none() {
                first_err = Some(err);
            }
        }
    }
    if let Some(err) = first_err {
        return Err(format!("回滚部分失败：{err}"));
    }
    Ok(())
}

fn replace_from_staging(plan: &PortableUpdatePlan) -> Result<(), String> {
    let target_dir = StdPathBuf::from(&plan.target_dir);
    let target_exe_path = target_dir.join(&plan.target_exe_name);
    let staging_dir = StdPathBuf::from(&plan.staging_dir);
    let backup_root = StdPathBuf::from(&plan.backup_root);
    let log_path = StdPathBuf::from(&plan.log_path);
    let zip_path = StdPathBuf::from(&plan.zip_path);
    append_helper_log(&log_path, "[自动更新] helper 开始执行便携版替换");
    if !staging_dir.exists() {
        return Err(format!("staging 目录不存在：{}", staging_dir.display()));
    }
    if !target_dir.exists() {
        return Err(format!("目标目录不存在：{}", target_dir.display()));
    }
    let staging_files = collect_relative_files(&staging_dir)?;
    if staging_files.is_empty() {
        return Err("staging 目录为空，无法替换".to_string());
    }
    let has_target_exe = staging_files.iter().any(|rel| {
        rel.file_name()
            .and_then(|v| v.to_str())
            .map(|name| name.eq_ignore_ascii_case(&plan.target_exe_name))
            .unwrap_or(false)
    });
    if !has_target_exe {
        return Err(format!("staging 中缺少主程序：{}", plan.target_exe_name));
    }
    std_fs::create_dir_all(&backup_root).map_err(|err| {
        format!("创建备份根目录失败（{}）：{err}", backup_root.display())
    })?;
    let backup_dir = backup_root.join(
        now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "backup".to_string())
            .replace(':', "-"),
    );
    std_fs::create_dir_all(&backup_dir).map_err(|err| {
        format!("创建备份目录失败（{}）：{err}", backup_dir.display())
    })?;
    let mut replaced_files = Vec::new();
    let mut new_files = Vec::new();
    for rel in &staging_files {
        let target = target_dir.join(rel);
        if target.exists() {
            let backup = backup_dir.join(rel);
            copy_file_with_parent(&target, &backup)?;
            replaced_files.push(rel.clone());
        } else {
            new_files.push(rel.clone());
        }
    }
    let replace_result = (|| -> Result<(), String> {
        for rel in &staging_files {
            let from = staging_dir.join(rel);
            let to = target_dir.join(rel);
            copy_file_with_retry(&from, &to, &log_path)?;
        }
        for rel in &staging_files {
            let from_hash = compute_file_sha256(&staging_dir.join(rel))?;
            let to_hash = compute_file_sha256(&target_dir.join(rel))?;
            if from_hash != to_hash {
                return Err(format!("落地校验失败：{}", rel.display()));
            }
        }
        if !target_exe_path.exists() {
            return Err(format!("替换后主程序不存在：{}", target_exe_path.display()));
        }
        Ok(())
    })();
    if let Err(err) = replace_result {
        append_helper_log(&log_path, &format!("[自动更新] 便携版替换失败，开始回滚：{err}"));
        match restore_backup_files(&target_dir, &backup_dir, &replaced_files, &new_files) {
            Ok(_) => append_helper_log(&log_path, "[自动更新] 便携版回滚完成"),
            Err(rollback_err) => append_helper_log(&log_path, &format!("[自动更新] 便携版回滚失败：{rollback_err}，旧备份保留在 {}", backup_dir.display())),
        }
        // 落盘 pending，供下次启动弹窗指引手动替换
        let temp_root = StdPathBuf::from(&plan.temp_root);
        let _ = write_portable_pending(&temp_root, plan, &backup_dir, &err);
        return Err(format!("便携版更新失败，已回滚旧版本：{err}"));
    }
    spawn_detached_hidden(&target_exe_path, &[])?;
    append_helper_log(&log_path, "[自动更新] 新版本已启动，开始清理临时文件");
    let _ = remove_if_exists(&staging_dir);
    let _ = remove_if_exists(&zip_path);
    // 成功则清理 pending（若存在）
    let temp_root = StdPathBuf::from(&plan.temp_root);
    let _ = remove_if_exists(&portable_pending_path(&temp_root));
    prune_old_backup_dirs(&backup_root);
    Ok(())
}

fn run_portable_update_helper(plan_path: &str) -> Result<(), String> {
    let plan_path = StdPathBuf::from(plan_path);
    let raw = std_fs::read(&plan_path).map_err(|err| {
        format!("读取便携版更新计划失败（{}）：{err}", plan_path.display())
    })?;
    let plan: PortableUpdatePlan = serde_json::from_slice(&raw)
        .map_err(|err| format!("解析便携版更新计划失败：{err}"))?;
    let log_path = StdPathBuf::from(&plan.log_path);
    append_helper_log(&log_path, "[自动更新] helper 已启动，等待主程序退出");
    wait_for_parent_process_exit(plan.parent_pid, &log_path);
    let result = replace_from_staging(&plan);
    match &result {
        Ok(_) => {
            let _ = remove_if_exists(&plan_path);
            append_helper_log(&log_path, "[自动更新] helper 执行完成");
        }
        Err(err) => append_helper_log(&log_path, &format!("[自动更新] helper 执行失败：{err}")),
    }
    result
}

fn maybe_run_portable_update_helper_from_args() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().collect();
    let Some(idx) = args.iter().position(|arg| arg == PORTABLE_HELPER_FLAG) else {
        return Ok(false);
    };
    let plan_path = args
        .get(idx + 1)
        .map(String::as_str)
        .ok_or_else(|| "便携版更新 helper 缺少计划文件参数".to_string())?;
    run_portable_update_helper(plan_path)?;
    Ok(true)
}

#[cfg(test)]
mod updater_release_page_tests {
    use super::{
        github_update_download_route_name, next_github_update_proxy_cursor,
        updater_download_endpoint, updater_manifest_fallbacks, updater_release_page_url,
        GithubUpdateMethod, UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES,
        UPDATER_GITHUB_HK_PROXY_PREFIX, UPDATER_GITHUB_PROXY_PREFIX,
    };

    #[test]
    fn manifest_candidates_keep_proxy_then_direct_order() {
        let origin = "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest.json";
        let candidates = updater_manifest_fallbacks(origin, GithubUpdateMethod::Auto);

        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].display_name, "中转（A）");
        assert_eq!(candidates[0].endpoint, format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"));
        assert_eq!(candidates[1].display_name, "中转（B）");
        assert_eq!(candidates[1].endpoint, format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"));
        assert_eq!(candidates[2].display_name, "直连");
        assert_eq!(candidates[2].endpoint, origin);
    }

    #[test]
    fn manifest_candidates_respect_selected_method() {
        let origin = "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest.json";

        assert_eq!(
            updater_manifest_fallbacks(origin, GithubUpdateMethod::Proxy)
                .iter()
                .map(|candidate| candidate.display_name)
                .collect::<Vec<_>>(),
            vec!["中转（A）", "中转（B）"]
        );
        assert_eq!(
            updater_manifest_fallbacks(origin, GithubUpdateMethod::Direct)
                .iter()
                .map(|candidate| candidate.display_name)
                .collect::<Vec<_>>(),
            vec!["直连"]
        );
    }

    #[test]
    fn download_route_name_is_human_readable() {
        assert_eq!(github_update_download_route_name(GithubUpdateMethod::Direct, 0), "直连");
        assert_eq!(github_update_download_route_name(GithubUpdateMethod::Proxy, 0), "中转（A）");
        assert_eq!(github_update_download_route_name(GithubUpdateMethod::Proxy, 1), "中转（B）");
    }

    #[test]
    fn release_page_url_keeps_original_github_page() {
        let origin = "https://github.com/kawayiYokami/P-ai/releases/latest";

        assert_eq!(updater_release_page_url(origin), origin);
    }

    #[test]
    fn download_endpoint_uses_only_current_proxy_cursor() {
        let origin = "https://github.com/kawayiYokami/P-ai/releases/download/v0.41.0/P-ai.zip";

        assert_eq!(
            updater_download_endpoint(origin, GithubUpdateMethod::Proxy, 2),
            format!("{}{}", UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES[2], origin)
        );
        assert_eq!(
            updater_download_endpoint(origin, GithubUpdateMethod::Proxy, 3),
            format!("{}{}", UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES[3], origin)
        );
    }

    #[test]
    fn download_proxy_cursor_wraps_after_last_proxy() {
        let last = UPDATER_GITHUB_DOWNLOAD_PROXY_PREFIXES.len() - 1;

        assert_eq!(next_github_update_proxy_cursor(last), 0);
    }

    #[test]
    fn direct_download_endpoint_does_not_use_proxy_cursor() {
        let origin = "https://github.com/kawayiYokami/P-ai/releases/download/v0.41.0/P-ai.zip";

        assert_eq!(
            updater_download_endpoint(origin, GithubUpdateMethod::Direct, 3),
            origin
        );
    }
}
