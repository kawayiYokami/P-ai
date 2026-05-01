use std::{
    fs::{self as std_fs, File as StdFile},
    io::{Read, Write},
    path::{Path as StdPath, PathBuf as StdPathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration as StdDuration,
};

use tauri_plugin_updater::UpdaterExt;
use walkdir::WalkDir;
use zip::ZipArchive;

const UPDATER_GITHUB_PROXY_PREFIX: &str = "https://gh-proxy.org/";
const UPDATER_GITHUB_EDGEONE_PROXY_PREFIX: &str = "https://edgeone.gh-proxy.org/";
const UPDATER_GITHUB_HK_PROXY_PREFIX: &str = "https://hk.gh-proxy.org/";
const UPDATER_GITHUB_RELEASE_API_ORIGIN: &str =
    "https://api.github.com/repos/kawayiYokami/P-ai/releases/latest";
const UPDATER_GITHUB_CHANGELOG_RAW_ORIGIN: &str =
    "https://raw.githubusercontent.com/kawayiYokami/P-ai/main/CHANGELOG.md";
const UPDATER_GITHUB_RELEASE_PAGE_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest";
const UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest.json";
const UPDATER_GITHUB_PORTABLE_MANIFEST_ORIGIN: &str =
    "https://github.com/kawayiYokami/P-ai/releases/latest/download/latest-portable.json";
const PORTABLE_UPDATE_EVENT_NAME: &str = "easy-call:update-status";
const PORTABLE_HELPER_FLAG: &str = "--portable-update-helper";
const PORTABLE_UPDATE_TARGET_SUFFIX: &str = "-portable";
const UPDATE_STAGE_CHECKING: &str = "checking";
const UPDATE_STAGE_DOWNLOADING: &str = "downloading";
const UPDATE_STAGE_VERIFYING: &str = "verifying";
const UPDATE_STAGE_PREPARING: &str = "preparing";
const UPDATE_STAGE_INSTALLING: &str = "installing";
const UPDATE_STAGE_REPLACING: &str = "replacing";
const UPDATE_STAGE_READY: &str = "ready";
const UPDATE_STAGE_COMPLETED: &str = "completed";
const UPDATE_STAGE_FAILED: &str = "failed";

static UPDATE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static PREPARED_GITHUB_UPDATE: Mutex<Option<PreparedGithubUpdate>> = Mutex::new(None);

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubUpdateInfo {
    current_version: String,
    latest_version: String,
    has_update: bool,
    release_url: String,
    update_source: String,
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

#[derive(Debug, Clone, Deserialize)]
struct GithubLatestReleasePayload {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
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

fn updater_proxy_url(origin: &str) -> String {
    format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}")
}

fn updater_release_api_fallback_urls() -> Vec<String> {
    vec![
        format!("{UPDATER_GITHUB_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
        format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{UPDATER_GITHUB_RELEASE_API_ORIGIN}"),
        UPDATER_GITHUB_RELEASE_API_ORIGIN.to_string(),
    ]
}

fn updater_changelog_api_fallback_urls() -> Vec<String> {
    vec![
        format!("{UPDATER_GITHUB_PROXY_PREFIX}{UPDATER_GITHUB_CHANGELOG_RAW_ORIGIN}"),
        format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{UPDATER_GITHUB_CHANGELOG_RAW_ORIGIN}"),
        UPDATER_GITHUB_CHANGELOG_RAW_ORIGIN.to_string(),
    ]
}

fn updater_manifest_fallback_urls(origin: &str) -> Vec<String> {
    vec![
        format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"),
        format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"),
        origin.to_string(),
    ]
}

fn updater_download_fallback_urls(origin: &str) -> Vec<String> {
    vec![
        format!("{UPDATER_GITHUB_PROXY_PREFIX}{origin}"),
        format!("{UPDATER_GITHUB_HK_PROXY_PREFIX}{origin}"),
        origin.to_string(),
    ]
}

fn strip_known_proxy_prefix(url: &str) -> &str {
    url.strip_prefix(UPDATER_GITHUB_PROXY_PREFIX)
        .or_else(|| url.strip_prefix(UPDATER_GITHUB_EDGEONE_PROXY_PREFIX))
        .or_else(|| url.strip_prefix(UPDATER_GITHUB_HK_PROXY_PREFIX))
        .unwrap_or(url)
}

fn clear_prepared_github_update() {
    if let Ok(mut guard) = PREPARED_GITHUB_UPDATE.lock() {
        *guard = None;
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
    #[cfg(not(target_os = "windows"))]
    {
        return "unsupported";
    }
}

fn current_portable_target() -> String {
    format!("{}{}", current_installer_target(), PORTABLE_UPDATE_TARGET_SUFFIX)
}

fn emit_update_progress(app: &AppHandle, payload: UpdateProgressPayload) {
    let _ = app.emit(PORTABLE_UPDATE_EVENT_NAME, payload);
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

async fn fetch_latest_release_payload() -> Result<GithubLatestReleasePayload, String> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(8))
        .build()
        .map_err(|err| format!("初始化更新检查客户端失败：{err}"))?;
    let mut last_error = String::new();
    for endpoint in updater_release_api_fallback_urls() {
        for attempt in 1..=3 {
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
            return response
                .json::<GithubLatestReleasePayload>()
                .await
                .map_err(|err| {
                    format!(
                        "解析 GitHub 更新响应失败（地址：{endpoint}，第 {attempt} 次）：{err}"
                    )
                });
        }
    }
    Err(last_error)
}

async fn fetch_remote_changelog_markdown() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(8))
        .build()
        .map_err(|err| format!("初始化更新日志客户端失败：{err}"))?;
    let mut last_error = String::new();
    for endpoint in updater_changelog_api_fallback_urls() {
        for attempt in 1..=3 {
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

fn extract_latest_changelog_section(markdown: &str) -> Option<String> {
    let normalized = markdown.replace("\r\n", "\n");
    let mut sections = Vec::<(String, Vec<String>)>::new();
    let mut current_title = String::new();
    let mut current_lines = Vec::<String>::new();
    for line in normalized.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            if !current_title.is_empty() {
                sections.push((current_title.clone(), current_lines.clone()));
            }
            current_title = title.trim().to_string();
            current_lines.clear();
            continue;
        }
        if !current_title.is_empty() {
            current_lines.push(line.to_string());
        }
    }
    if !current_title.is_empty() {
        sections.push((current_title, current_lines));
    }
    let (title, lines) = sections.into_iter().next()?;
    let mut body = lines.join("\n");
    body = body.trim().to_string();
    let mut result = String::new();
    result.push_str(title.trim());
    if !body.is_empty() {
        result.push_str("\n\n");
        result.push_str(&body);
    }
    Some(result)
}

async fn fetch_latest_changelog_notes() -> Result<String, String> {
    let markdown = fetch_remote_changelog_markdown().await?;
    extract_latest_changelog_section(&markdown)
        .ok_or_else(|| "CHANGELOG.md 中未找到可展示的最新节".to_string())
}

#[tauri::command]
async fn fetch_project_changelog_markdown() -> Result<String, String> {
    fetch_remote_changelog_markdown().await
}

#[tauri::command]
async fn check_github_update() -> Result<GithubUpdateInfo, String> {
    let runtime = detect_update_runtime_paths()?;
    let payload = fetch_latest_release_payload().await?;
    let latest_version = payload
        .tag_name
        .as_deref()
        .or(payload.name.as_deref())
        .map(normalize_release_version)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "GitHub Release 未返回有效版本号".to_string())?;
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release_notes = match fetch_latest_changelog_notes().await {
        Ok(notes) => notes,
        Err(err) => {
            eprintln!("[自动更新] 远程更新日志读取失败：{err}");
            payload.body.clone().unwrap_or_default()
        }
    };
    Ok(GithubUpdateInfo {
        current_version: current_version.clone(),
        latest_version: latest_version.clone(),
        has_update: is_newer_version(&current_version, &latest_version),
        release_url: updater_proxy_url(
            payload
                .html_url
                .as_deref()
                .unwrap_or(UPDATER_GITHUB_RELEASE_PAGE_ORIGIN),
        ),
        update_source: "github".to_string(),
        release_notes,
        published_at: payload.published_at,
        runtime_kind: runtime.runtime_kind.as_str().to_string(),
        can_force_update: true,
    })
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
    Ok(format!("{:x}", hasher.finalize()))
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
            .ok_or_else(|| format!("更新包中存在不安全路径：{}", entry.name()))?
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
fn spawn_detached_hidden(command: &mut Command) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, DETACHED_PROCESS};

    command.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    command.spawn().map_err(|err| format!("启动后台进程失败：{err}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_detached_hidden(command: &mut Command) -> Result<(), String> {
    command.spawn().map_err(|err| format!("启动后台进程失败：{err}"))?;
    Ok(())
}

async fn check_updater_with_manifest_fallbacks(
    app: &AppHandle,
    runtime_kind: UpdateRuntimeKind,
    target: Option<String>,
    manifest_origin: &str,
    force: bool,
    current_version: &str,
    checking_message: &str,
    check_failed_prefix: &str,
) -> Result<tauri_plugin_updater::Update, String> {
    emit_update_progress(
        app,
        build_update_progress(
            runtime_kind,
            UPDATE_STAGE_CHECKING,
            checking_message,
            Some(current_version.to_string()),
            None,
            None,
            None,
            None,
        ),
    );
    let mut last_error = String::new();
    for endpoint in updater_manifest_fallback_urls(manifest_origin) {
        for attempt in 1..=3 {
            let mut builder = app.updater_builder().pubkey(updater_public_key()?);
            if let Some(ref target) = target {
                builder = builder.target(target.clone());
            }
    #[cfg(target_os = "windows")]
            {
                // NSIS 自动更新如果不显式传入当前安装目录，安装器可能会回落到默认目录。
                // `/D=...` 需要作为最后一个 NSIS 参数传入，tauri-plugin-updater 会把额外 installer_args
                // 追加在内部参数之后，这里正好满足要求。
                if runtime_kind == UpdateRuntimeKind::Installer {
                    let runtime = detect_update_runtime_paths()?;
                    builder = builder.installer_arg(std::ffi::OsString::from(format!(
                        "/D={}",
                        runtime.exe_dir.display()
                    )));
                }
            }
            let mut builder = match reqwest::Url::parse(&endpoint) {
                Ok(url) => builder.endpoints(vec![url]),
                Err(err) => {
                    last_error = format!("解析更新端点失败（地址：{endpoint}，第 {attempt} 次）：{err}");
                    continue;
                }
            }
            .map_err(|err| format!("配置更新端点失败（地址：{endpoint}，第 {attempt} 次）：{err}"))?;
            if force {
                builder = builder.version_comparator(|current, update| update.version != current);
            }
            let updater = match builder.build() {
                Ok(updater) => updater,
                Err(err) => {
                    last_error =
                        format!("构建更新检查器失败（地址：{endpoint}，第 {attempt} 次）：{err}");
                    continue;
                }
            };
            let update = match updater.check().await {
                Ok(update) => update,
                Err(err) => {
                    last_error = format!(
                        "{check_failed_prefix}（地址：{endpoint}，第 {attempt} 次）：{err}"
                    );
                    continue;
                }
            };
            let Some(update) = update else {
                return Err("当前没有可安装的更新".to_string());
            };
            if !is_newer_version(current_version, &update.version) && !force {
                return Err("当前没有可安装的更新".to_string());
            }
            return Ok(update);
        }
    }
    Err(last_error)
}

async fn download_update_with_proxy_fallbacks<C, D>(
    update: &tauri_plugin_updater::Update,
    mut on_chunk: C,
    on_download_finish: D,
    download_failed_prefix: &str,
) -> Result<Vec<u8>, String>
where
    C: FnMut(usize, Option<u64>),
    D: FnOnce(),
{
    let origin_url = strip_known_proxy_prefix(update.download_url.as_str()).to_string();
    let mut on_download_finish = Some(on_download_finish);
    let mut last_error = String::new();
    for endpoint in updater_download_fallback_urls(&origin_url) {
        for attempt in 1..=3 {
            let mut retry_update = update.clone();
            retry_update.download_url = reqwest::Url::parse(&endpoint)
                .map_err(|err| format!("解析下载地址失败（地址：{endpoint}，第 {attempt} 次）：{err}"))?;
            match retry_update
                .download(
                    |chunk_length, content_length| {
                        on_chunk(chunk_length, content_length);
                    },
                    || {
                        if let Some(callback) = on_download_finish.take() {
                            callback();
                        }
                    },
                )
                .await
            {
                Ok(bytes) => return Ok(bytes),
                Err(err) => {
                    last_error = format!(
                        "{download_failed_prefix}（地址：{endpoint}，第 {attempt} 次）：{err}"
                    );
                }
            }
        }
    }
    Err(last_error)
}

async fn prepare_installer_update(app: &AppHandle, force: bool) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let runtime_kind = UpdateRuntimeKind::Installer;
    let update = check_updater_with_manifest_fallbacks(
        app,
        runtime_kind,
        None,
        UPDATER_GITHUB_INSTALLER_MANIFEST_ORIGIN,
        force,
        &current_version,
        "正在检查安装版更新",
        "检查安装版更新失败",
    )
    .await?;
    let target_version = update.version.clone();
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
            format!("正在下载安装版更新 {target_version}"),
            Some(current_version.clone()),
            Some(target_version.clone()),
            Some(0),
            None,
            None,
        ),
    );
    let bytes = download_update_with_proxy_fallbacks(
        &update,
            {
                let downloaded = downloaded.clone();
                move |chunk_length, content_length| {
                    let total = downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed)
                        + chunk_length as u64;
                    emit_update_progress(
                        app,
                        build_update_progress(
                            runtime_kind,
                            UPDATE_STAGE_DOWNLOADING,
                            format!("正在下载安装版更新 {download_progress_target_version}"),
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

async fn prepare_portable_update(app: &AppHandle, force: bool) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let runtime = detect_update_runtime_paths()?;
    let update = check_updater_with_manifest_fallbacks(
        app,
        runtime.runtime_kind,
        Some(current_portable_target()),
        UPDATER_GITHUB_PORTABLE_MANIFEST_ORIGIN,
        force,
        &current_version,
        "正在检查便携版更新",
        "检查便携版更新失败",
    )
    .await?;
    let target_version = update.version.clone();
    let download_progress_current_version = current_version.clone();
    let download_progress_target_version = target_version.clone();
    let verify_progress_current_version = current_version.clone();
    let verify_progress_target_version = target_version.clone();
    let temp_root = ensure_update_temp_dirs(&runtime)?;
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
            format!("正在下载便携版更新 {target_version}"),
            Some(current_version.clone()),
            Some(target_version.clone()),
            Some(0),
            None,
            None,
        ),
    );
    let bytes = download_update_with_proxy_fallbacks(
        &update,
            {
                let downloaded = downloaded.clone();
                move |chunk_length, content_length| {
                    let total = downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed)
                        + chunk_length as u64;
                    emit_update_progress(
                        app,
                        build_update_progress(
                            runtime.runtime_kind,
                            UPDATE_STAGE_DOWNLOADING,
                            format!("正在下载便携版更新 {download_progress_target_version}"),
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
    std_fs::write(&zip_path, &bytes).map_err(|err| {
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
    let extracted_files = extract_zip_to_dir(&zip_path, &staging_dir)?;
    let target_exe_name = runtime
        .exe_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("无法解析主程序文件名：{}", runtime.exe_path.display()))?
        .to_string();
    verify_staging_files(&staging_dir, &extracted_files, &target_exe_name)?;
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

#[tauri::command]
async fn start_github_update(app: AppHandle, force: bool) -> Result<(), String> {
    let _guard = UpdateInProgressGuard::acquire()?;
    clear_prepared_github_update();
    let runtime = detect_update_runtime_paths()?;
    let result = match runtime.runtime_kind {
        UpdateRuntimeKind::Installer => prepare_installer_update(&app, force).await,
        UpdateRuntimeKind::Portable => prepare_portable_update(&app, force).await,
    };
    if let Err(err) = &result {
        emit_update_progress(
            &app,
            build_update_progress(
                runtime.runtime_kind,
                UPDATE_STAGE_FAILED,
                format!("更新失败：{err}"),
                Some(env!("CARGO_PKG_VERSION").to_string()),
                None,
                None,
                None,
                Some(err.clone()),
            ),
        );
    }
    result
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
                    Some(prepared.current_version),
                    Some(prepared.target_version),
                    None,
                    None,
                    None,
                ),
            );
            app.restart()
        }
        PreparedGithubUpdate::Portable(prepared) => {
            let mut command = Command::new(&prepared.helper_copy_path);
            command
                .arg(PORTABLE_HELPER_FLAG)
                .arg(&prepared.plan_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if let Err(err) = spawn_detached_hidden(&mut command) {
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
                    Some(prepared.current_version),
                    Some(prepared.target_version),
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
    for rel in new_files {
        let target = target_dir.join(rel);
        remove_if_exists(&target)?;
    }
    for rel in replaced_files {
        let backup = backup_dir.join(rel);
        let target = target_dir.join(rel);
        copy_file_with_parent(&backup, &target)?;
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
            copy_file_with_parent(&from, &to)?;
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
        restore_backup_files(&target_dir, &backup_dir, &replaced_files, &new_files)?;
        append_helper_log(&log_path, "[自动更新] 便携版回滚完成");
        return Err(format!("便携版更新失败，已回滚旧版本：{err}"));
    }
    let mut relaunch = Command::new(&target_exe_path);
    relaunch.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    spawn_detached_hidden(&mut relaunch)?;
    append_helper_log(&log_path, "[自动更新] 新版本已启动，开始清理临时文件");
    let _ = remove_if_exists(&staging_dir);
    let _ = remove_if_exists(&zip_path);
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
    thread::sleep(StdDuration::from_millis(1800));
    let result = replace_from_staging(&plan);
    match &result {
        Ok(_) => append_helper_log(&log_path, "[自动更新] helper 执行完成"),
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
