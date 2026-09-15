// ==================== Git 面板仓库监视 ====================
// notify 递归监听 GitPanel 当前选中的仓库，让外部变化（编辑器保存、终端 git 操作、
// 外部工具）能自动刷新面板。架构复刻参考项目 gitron 的 watcher 裁剪版：
// - native watcher 优先（200ms 防抖），注册失败（含 Linux inotify 上限）回退 PollWatcher
// - 事件按路径分类：.git/HEAD→head、.git/index→workdir、其他 .git 内部→refs、非 .git→workdir
// - 100ms 合并窗口把一批事件折叠为最多三个布尔信号，一次 emit
// - refs 信号带指纹去重（HEAD + 全部 ref 的文件系统指纹），指纹未真变即丢弃，
//   避免 .git 内部噪音（index/index.lock/logs 等）触发无谓刷新；指纹不走 git 命令读缓存，
//   直接读文件元数据
// - 回调层 gitignore 过滤：第一版只解析仓库根 .gitignore，父目录命中即整棵子树丢弃
// 自适应降级状态机（用户策略）：status 单次耗时 >200ms → 停止监听进入降级；
// 降级中任意一次 status 调用（focus 刷新/手动刷新/操作收尾/tab 补载）<200ms → 恢复监听。
// 无专门探测定时器：计时与状态机收敛在 git_panel_status 命令内，恢复即重启监听。
//
// 注意：本文件由 include! 并入 main.rs 根作用域，不得顶层 use 与既有模块重名的类型，
// 一律使用完整路径。

// ---------- 常量 ----------

const GIT_PANEL_WATCH_DEBOUNCE_MS: u64 = 200;
const GIT_PANEL_WATCH_MERGE_WINDOW_MS: u64 = 100;
const GIT_PANEL_WATCH_POLL_INTERVAL_MS: u64 = 2000;
/// status 单次耗时降级阈值：超过则停监听，低于则恢复（用户策略）。
const GIT_PANEL_WATCH_DEGRADE_MS: u128 = 200;
const GIT_PANEL_WATCH_EVENT: &str = "easy-call:git-panel-changed";

// ---------- 信号与事件载荷 ----------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GitPanelWatchSignals {
    workdir_changed: bool,
    head_changed: bool,
    refs_changed: bool,
}

impl GitPanelWatchSignals {
    fn is_empty(&self) -> bool {
        !self.workdir_changed && !self.head_changed && !self.refs_changed
    }

    fn merge(&mut self, other: GitPanelWatchSignals) {
        self.workdir_changed |= other.workdir_changed;
        self.head_changed |= other.head_changed;
        self.refs_changed |= other.refs_changed;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWatchEventPayload {
    workspace_path: String,
    workdir_changed: bool,
    head_changed: bool,
    refs_changed: bool,
}

/// 把变更路径分类为面板刷新信号。
/// 路径按组件边界匹配 ".git"：`my.gitrepo/x` 不算 git 目录，`.gitignore` 属于工作区文件。
/// 扫描路径中全部 ".git" 出现位置，取第一个通过组件边界检查的匹配
/// （如 `E:/dotfiles.git/.git/HEAD` 应命中后者，识别为 head_changed）。
fn git_panel_watch_classify_path(path: &str) -> GitPanelWatchSignals {
    let normalized = path.replace('\\', "/");
    let mut search_from = 0;
    let mut matched_after: Option<&str> = None;
    while let Some(pos) = normalized[search_from..].find(".git") {
        let abs_pos = search_from + pos;
        let before_ok = abs_pos == 0 || normalized.as_bytes()[abs_pos - 1] == b'/';
        let after = &normalized[abs_pos + 4..];
        let after_ok = after.is_empty() || after.starts_with('/');
        if before_ok && after_ok {
            matched_after = Some(after);
            break;
        }
        search_from = abs_pos + 4;
    }
    let Some(after) = matched_after else {
        return GitPanelWatchSignals { workdir_changed: true, ..Default::default() };
    };
    match after {
        // .git 目录自身变化或 HEAD 变化都意味着分支/检出状态可能变了
        "" | "/HEAD" => GitPanelWatchSignals { head_changed: true, ..Default::default() },
        // 暂存区索引变化等价于工作区状态变化（外部 git add/reset 等）
        "/index" => GitPanelWatchSignals { workdir_changed: true, ..Default::default() },
        // 其他 .git 内部（index.lock/logs/objects 等）不是工作区改动：按 refs 处理，
        // 由指纹去重挡掉没有真实引用变化的噪音。
        // 注意：本应用自己的 status 查询一律带 --no-optional-locks，不写索引，
        // 否则这条链会自我触发
        _ => GitPanelWatchSignals { refs_changed: true, ..Default::default() },
    }
}

// ---------- gitignore 回调层过滤 ----------

struct GitPanelIgnoreFilter {
    search: gix_ignore::Search,
    repo_root: std::path::PathBuf,
}

impl GitPanelIgnoreFilter {
    /// 第一版边界：只解析仓库根 .gitignore（嵌套 ignore 文件与 info/exclude 后续补）。
    fn load(repo_root: &std::path::Path) -> Self {
        let mut search = gix_ignore::Search::default();
        let ignore_file = repo_root.join(".gitignore");
        if let Ok(bytes) = std::fs::read(&ignore_file) {
            search.add_patterns_buffer(
                &bytes,
                std::path::PathBuf::from(".gitignore"),
                None,
                gix_ignore::search::Ignore::default(),
            );
        } else {
            runtime_log_debug(format!(
                "[Git面板监视] 仓库根无 .gitignore，跳过忽略规则解析 path={}",
                ignore_file.display()
            ));
        }
        Self {
            search,
            repo_root: repo_root.to_path_buf(),
        }
    }

    /// 逐级匹配相对路径的每一级祖先目录：任一级命中非 negation 规则即整棵子树丢弃。
    /// 与 git「被忽略目录内的文件无法 re-include」语义一致的第一版近似。
    fn is_ignored(&self, path: &std::path::Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.repo_root) else {
            return false;
        };
        let mut prefix = std::path::PathBuf::new();
        for component in relative.components() {
            prefix.push(component);
            let full = self.repo_root.join(&prefix);
            // 目录规则（target/）带 MUST_BE_DIR，必须明确 is_dir 才能命中；
            // 事件路径仍存在时按文件系统元数据判断，删除事件拿不到元数据按非目录
            let is_dir = std::fs::metadata(&full)
                .ok()
                .map(|metadata| metadata.is_dir());
            if self.matches_relative(&prefix, is_dir) {
                return true;
            }
        }
        false
    }

    fn matches_relative(&self, relative: &std::path::Path, is_dir: Option<bool>) -> bool {
        let normalized = relative.to_string_lossy().replace('\\', "/");
        // Windows 上 git 默认 core.ignorecase=true，路径大小写折叠匹配；其余平台敏感
        let case = if cfg!(windows) {
            gix_ignore::glob::pattern::Case::Fold
        } else {
            gix_ignore::glob::pattern::Case::Sensitive
        };
        let rel_bytes: &bstr::BStr = normalized.as_bytes().into();
        match self
            .search
            .pattern_matching_relative_path(rel_bytes, is_dir, case)
        {
            Some(m) => !m.pattern.is_negative(),
            None => false,
        }
    }
}

// ---------- watcher 持柄 ----------

/// watcher 持柄：inner 的 drop 会停止监视线程，必须与 state 同生命周期。
struct GitPanelWatcherHandle {
    /// 监视模式标签（原生/轮询），用于降级与启动日志
    mode: &'static str,
    #[allow(dead_code)] // 仅持柄：存活即监听，drop 即停止
    inner: Box<dyn std::any::Any + Send>,
}

struct GitPanelWatchState {
    repo_root: String,
    handle: Option<GitPanelWatcherHandle>,
    degraded: bool,
    /// 当前仓库的活跃实例数：多实例并存时，stop 只减计数，归零才真正停止监听。
    /// 全局单例同时只监听一个仓库，实例切仓库时由命令层重置为新仓库计数。
    ref_count: usize,
}

static GIT_PANEL_WATCH_STATE: std::sync::OnceLock<parking_lot::Mutex<Option<GitPanelWatchState>>> =
    std::sync::OnceLock::new();
/// 最近一次 refs 指纹：用于把「.git 内部噪音写入但引用未真变」的事件折叠为 workdir。
static GIT_PANEL_LAST_REFS_FINGERPRINT: parking_lot::Mutex<u64> = parking_lot::Mutex::new(0);

type GitPanelWatchTx = tokio::sync::mpsc::Sender<GitPanelWatchSignals>;

fn git_panel_watch_state_cell(
) -> &'static parking_lot::Mutex<Option<GitPanelWatchState>> {
    GIT_PANEL_WATCH_STATE.get_or_init(|| parking_lot::Mutex::new(None))
}

// ---------- 事件转发与消费 ----------

fn git_panel_watch_forward_events(
    events: Vec<notify_debouncer_mini::DebouncedEvent>,
    filter: &GitPanelIgnoreFilter,
    tx: &GitPanelWatchTx,
) {
    let mut merged = GitPanelWatchSignals::default();
    for event in events {
        let path_str = event.path.to_string_lossy().to_string();
        let signal = git_panel_watch_classify_path(&path_str);
        // 纯工作区路径走 gitignore 过滤；.git 内部路径保留给分类逻辑
        if signal.workdir_changed && !signal.head_changed && !signal.refs_changed
            && filter.is_ignored(&event.path)
        {
            continue;
        }
        merged.merge(signal);
    }
    if !merged.is_empty() {
        tx.try_send(merged).ok();
    }
}

async fn git_panel_watch_consumer(
    mut rx: tokio::sync::mpsc::Receiver<GitPanelWatchSignals>,
    repo_root: String,
    app: tauri::AppHandle,
) {
    loop {
        let Some(mut signals) = rx.recv().await else {
            break;
        };
        // 合并窗口：短暂等待后排空队列，把连续触发折叠为一轮刷新
        tokio::time::sleep(std::time::Duration::from_millis(GIT_PANEL_WATCH_MERGE_WINDOW_MS)).await;
        while let Ok(extra) = rx.try_recv() {
            signals.merge(extra);
        }
        if signals.head_changed || signals.refs_changed {
            let fingerprint = git_panel_watch_compute_refs_fingerprint(&repo_root);
            let mut last = GIT_PANEL_LAST_REFS_FINGERPRINT.lock();
            if *last == fingerprint {
                // 引用未真变：是 .git 内部噪音（index/index.lock/logs 等），丢弃不推送
                signals.head_changed = false;
                signals.refs_changed = false;
            } else {
                *last = fingerprint;
            }
        }
        if signals.is_empty() {
            continue;
        }
        // 事件 = 缓存可能过期信号：清掉该仓库读缓存（TTL 2s），否则前端收到事件后
        // loadStatus 会命中旧缓存，自动刷新失效
        git_executor().invalidate(&repo_root);
        runtime_log_debug(format!(
            "[Git面板监视] 推送变化信号 workspace={} workdir={} head={} refs={}",
            repo_root, signals.workdir_changed, signals.head_changed, signals.refs_changed
        ));
        let payload = GitPanelWatchEventPayload {
            workspace_path: repo_root.clone(),
            workdir_changed: signals.workdir_changed,
            head_changed: signals.head_changed,
            refs_changed: signals.refs_changed,
        };
        // 广播范围记录：原生事件覆盖全部窗口，桥接客户端逐个列举，排障时据此判断
        // 「后端发了」与「某个消费者没收到」之间的断点。
        let client_ids = ide_context_chat_client_ids();
        runtime_log_debug(format!(
            "[Git面板监视] 广播变化信号 workspace={} 原生事件={} 桥接客户端={} ids={:?}",
            repo_root,
            GIT_PANEL_WATCH_EVENT,
            client_ids.len(),
            client_ids
        ));
        let _ = app.emit(GIT_PANEL_WATCH_EVENT, payload.clone());
        // Web/VS Code 客户端经 WS bridge 收不到 Tauri 原生事件，必须主动广播
        // notification（method 与前端 onTransportNotification 的 canonical 名一致）。
        ide_chat_broadcast_notification(
            "gitPanel.watchChanged",
            serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
        );
    }
}

/// 解析仓库的实际 git 目录：普通仓库 `.git` 是目录直接返回；
/// worktree/submodule 的 `.git` 是 gitdir 文件（内容形如 `gitdir: <target>`），
/// 解析目标并按仓库根解析相对路径；解析失败返回 None。
fn git_panel_watch_resolve_git_dir(repo_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let target = text.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("gitdir:").map(str::trim)
    })?;
    // 路径含空格时 git 会用双引号包裹目标（C 风格转义，复杂场景暂不展开）
    let target = target
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(target);
    let target_path = std::path::PathBuf::from(target);
    if target_path.is_absolute() {
        Some(target_path)
    } else {
        Some(repo_root.join(target_path))
    }
}

/// refs 指纹：HEAD + packed-refs + refs/** 的 (相对路径, 大小, mtime 纳秒) 哈希。
/// 直接读文件元数据而非 git 命令，避开读缓存导致的旧值误判。
/// git 目录经 gitdir 解析（兼容 worktree/submodule 的 `.git` 文件形态）。
fn git_panel_watch_compute_refs_fingerprint(repo_root: &str) -> u64 {
    use std::hash::Hasher;

    fn entry_text(git_dir: &std::path::Path, file: &std::path::Path) -> Option<String> {
        let metadata = std::fs::metadata(file).ok()?;
        let modified = metadata.modified().ok()?;
        let nanos = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let relative = file.strip_prefix(git_dir).ok()?.to_string_lossy().replace('\\', "/");
        Some(format!("{relative}|{}|{nanos}", metadata.len()))
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut entries: Vec<String> = Vec::new();
    let Some(git_dir) = git_panel_watch_resolve_git_dir(std::path::Path::new(repo_root)) else {
        // git 目录不可解析（如 .git 文件损坏）：指纹退化为 0，等同无历史可比较
        return 0;
    };
    for name in ["HEAD", "packed-refs"] {
        let file = git_dir.join(name);
        if file.is_file() {
            if let Some(text) = entry_text(&git_dir, &file) {
                entries.push(text);
            }
        }
    }
    for entry in walkdir::WalkDir::new(git_dir.join("refs"))
        .max_depth(8)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Some(text) = entry_text(&git_dir, entry.path()) {
                entries.push(text);
            }
        }
    }
    entries.sort();
    for text in &entries {
        hasher.write(text.as_bytes());
    }
    hasher.finish()
}

// ---------- watcher 构建 ----------

fn git_panel_watch_try_native(
    repo_root: &std::path::Path,
    filter: std::sync::Arc<GitPanelIgnoreFilter>,
    tx: GitPanelWatchTx,
) -> Result<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>, String> {
    let mut debouncer = notify_debouncer_mini::new_debouncer(
        std::time::Duration::from_millis(GIT_PANEL_WATCH_DEBOUNCE_MS),
        move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            if let Ok(events) = events {
                git_panel_watch_forward_events(events, &filter, &tx);
            }
        },
    )
    .map_err(|err| format!("创建原生监视器失败:{err}"))?;
    debouncer
        .watcher()
        .watch(repo_root, notify::RecursiveMode::Recursive)
        .map_err(|err| format!("注册递归监听失败:{err}"))?;
    Ok(debouncer)
}

fn git_panel_watch_try_poll(
    repo_root: &std::path::Path,
    filter: std::sync::Arc<GitPanelIgnoreFilter>,
    tx: GitPanelWatchTx,
) -> Result<notify_debouncer_mini::Debouncer<notify::PollWatcher>, String> {
    let config = notify_debouncer_mini::Config::default()
        .with_timeout(std::time::Duration::from_millis(GIT_PANEL_WATCH_DEBOUNCE_MS))
        .with_notify_config(
            notify::Config::default()
                .with_poll_interval(std::time::Duration::from_millis(GIT_PANEL_WATCH_POLL_INTERVAL_MS)),
        );
    let mut debouncer = notify_debouncer_mini::new_debouncer_opt::<_, notify::PollWatcher>(
        config,
        move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            if let Ok(events) = events {
                git_panel_watch_forward_events(events, &filter, &tx);
            }
        },
    )
    .map_err(|err| format!("创建轮询监视器失败:{err}"))?;
    debouncer
        .watcher()
        .watch(repo_root, notify::RecursiveMode::Recursive)
        .map_err(|err| format!("注册轮询监听失败:{err}"))?;
    Ok(debouncer)
}

// ---------- 启停与命令 ----------

async fn git_panel_watch_start_internal(app: tauri::AppHandle, repo_root: String) -> Result<(), String> {
    let cell = git_panel_watch_state_cell();
    {
        let guard = cell.lock();
        if let Some(existing) = guard.as_ref() {
            if existing.repo_root == repo_root && existing.handle.is_some() && !existing.degraded {
                return Ok(());
            }
        }
    }
    // 重建（降级恢复/切仓库）时保留旧实例的引用计数；命令层切仓库后另行重置
    let previous_ref_count = cell.lock().as_ref().map(|s| s.ref_count).unwrap_or(0);

    let filter = std::sync::Arc::new(GitPanelIgnoreFilter::load(std::path::Path::new(&repo_root)));
    let (tx, rx) = tokio::sync::mpsc::channel::<GitPanelWatchSignals>(64);
    let root_path = std::path::PathBuf::from(&repo_root);

    let handle = match git_panel_watch_try_native(&root_path, filter.clone(), tx.clone()) {
        Ok(handle) => {
            runtime_log_info(format!(
                "[Git面板监视] 开始监听仓库（原生 watcher，防抖 {}ms）workspace={repo_root}",
                GIT_PANEL_WATCH_DEBOUNCE_MS
            ));
            GitPanelWatcherHandle {
                mode: "原生",
                inner: Box::new(handle),
            }
        }
        Err(native_error) => {
            runtime_log_warn(format!(
                "[Git面板监视] 原生 watcher 失败({native_error})，回退轮询模式（间隔 {}ms）",
                GIT_PANEL_WATCH_POLL_INTERVAL_MS
            ));
            let handle = git_panel_watch_try_poll(&root_path, filter, tx.clone())?;
            GitPanelWatcherHandle {
                mode: "轮询",
                inner: Box::new(handle),
            }
        }
    };

    let consumer_repo = repo_root.clone();
    tokio::spawn(async move {
        git_panel_watch_consumer(rx, consumer_repo, app).await;
    });

    *cell.lock() = Some(GitPanelWatchState {
        repo_root,
        handle: Some(handle),
        degraded: false,
        ref_count: previous_ref_count,
    });
    *GIT_PANEL_LAST_REFS_FINGERPRINT.lock() = 0;
    Ok(())
}

#[tauri::command]
async fn git_panel_watch_start(
    app: tauri::AppHandle,
    input: GitPanelWorkspaceInput,
) -> Result<(), String> {
    let repo_root = git_panel_resolve_root(&input.workspace_path).await?;
    let cell = git_panel_watch_state_cell();
    // 同仓库且监听正常：只增加引用计数，不重建 watcher
    {
        let mut guard = cell.lock();
        if let Some(state) = guard.as_mut() {
            if state.repo_root == repo_root && state.handle.is_some() && !state.degraded {
                state.ref_count += 1;
                runtime_log_debug(format!(
                    "[Git面板监视] 同仓库新实例启动，引用计数 {} workspace={repo_root}",
                    state.ref_count
                ));
                return Ok(());
            }
        }
    }
    git_panel_watch_start_internal(app, repo_root.clone()).await?;
    // 新建或切换仓库：全局单例只监听一个仓库，计数重置为单实例
    let mut guard = cell.lock();
    if let Some(state) = guard.as_mut() {
        if state.repo_root == repo_root {
            state.ref_count = 1;
        }
    }
    Ok(())
}

#[tauri::command]
async fn git_panel_watch_stop(input: GitPanelWorkspaceInput) -> Result<(), String> {
    // 解析失败（仓库已不存在等）时静默返回：没有可停的监听对象
    let Ok(repo_root) = git_panel_resolve_root(&input.workspace_path).await else {
        return Ok(());
    };
    let cell = git_panel_watch_state_cell();
    let mut guard = cell.lock();
    let Some(state) = guard.as_ref() else {
        return Ok(());
    };
    // 仓库不匹配：不是本实例的监听，保护其他并存实例的全局监听状态
    if state.repo_root != repo_root {
        return Ok(());
    }
    if state.ref_count <= 1 {
        guard.take();
        runtime_log_info(format!(
            "[Git面板监视] 停止监听仓库（最后一个实例退出）workspace={repo_root}"
        ));
    } else if let Some(state) = guard.as_mut() {
        state.ref_count -= 1;
        runtime_log_debug(format!(
            "[Git面板监视] 实例退出，引用计数 {}，继续监听 workspace={repo_root}",
            state.ref_count
        ));
    }
    Ok(())
}

// ---------- 自适应降级状态机 ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitPanelWatchTimingAction {
    None,
    EnterDegraded,
    ExitDegraded,
}

/// 纯决策函数：耗时超阈值且未降级 → 进入降级；未超阈值且已降级 → 恢复。
/// 边界语义：恰好等于阈值视为正常区间（严格大于才降级）。
fn git_panel_watch_decide_timing_action(
    elapsed_ms: u128,
    degraded: bool,
) -> GitPanelWatchTimingAction {
    if elapsed_ms > GIT_PANEL_WATCH_DEGRADE_MS {
        if degraded {
            GitPanelWatchTimingAction::None
        } else {
            GitPanelWatchTimingAction::EnterDegraded
        }
    } else if degraded {
        GitPanelWatchTimingAction::ExitDegraded
    } else {
        GitPanelWatchTimingAction::None
    }
}

/// git_panel_status 收尾钩子：按本次耗时驱动降级状态机。
/// 仅对当前正在监听的仓库生效；恢复即以同一仓库重启监听（复用 start 的幂等检查）。
async fn git_panel_watch_adapt_after_status(
    repo_root: &str,
    elapsed_ms: u128,
    app: tauri::AppHandle,
) {
    let action = {
        let cell = git_panel_watch_state_cell();
        let guard = cell.lock();
        let Some(state) = guard.as_ref() else {
            return;
        };
        // 只对正在监视的仓库做状态机；其他仓库的 status 调用不干扰当前监听
        if state.repo_root != repo_root {
            return;
        }
        git_panel_watch_decide_timing_action(elapsed_ms, state.degraded)
    };
    match action {
        GitPanelWatchTimingAction::None => {}
        GitPanelWatchTimingAction::EnterDegraded => {
            let cell = git_panel_watch_state_cell();
            let mut guard = cell.lock();
            if let Some(state) = guard.as_mut() {
                if state.repo_root == repo_root {
                    if let Some(handle) = state.handle.take() {
                        let handle_mode = handle.mode;
                        state.degraded = true;
                        runtime_log_warn(format!(
                            "[Git面板监视] status 耗时 {elapsed_ms}ms 超过阈值 {}ms，暂停自动刷新（大仓库降级，等待下一次快速 status 恢复；原监视模式：{handle_mode}）workspace={repo_root}",
                            GIT_PANEL_WATCH_DEGRADE_MS,
                        ));
                    }
                }
            }
        }
        GitPanelWatchTimingAction::ExitDegraded => {
            runtime_log_info(format!(
                "[Git面板监视] status 耗时 {elapsed_ms}ms 已回落到阈值 {}ms 内，恢复自动刷新 workspace={repo_root}",
                GIT_PANEL_WATCH_DEGRADE_MS
            ));
            let _ = git_panel_watch_start_internal(app, repo_root.to_string()).await;
        }
    }
}

// ---------- 测试 ----------

#[cfg(test)]
mod git_panel_watch_tests {
    use super::*;

    #[test]
    fn classify_git_internal_paths_by_category() {
        let head = git_panel_watch_classify_path("E:/repo/.git/HEAD");
        assert!(head.head_changed && !head.workdir_changed && !head.refs_changed);

        // index 变化等价于暂存区变化（外部 git add/reset 等），仍是工作区信号
        let index = git_panel_watch_classify_path("E:/repo/.git/index");
        assert!(index.workdir_changed && !index.head_changed && !index.refs_changed);

        // index.lock 是 git 写索引的中间文件，不代表工作区改动：归 refs，由指纹去重挡掉
        let index_lock = git_panel_watch_classify_path("E:/repo/.git/index.lock");
        assert!(index_lock.refs_changed && !index_lock.head_changed && !index_lock.workdir_changed);

        let refs = git_panel_watch_classify_path("E:/repo/.git/refs/heads/main");
        assert!(refs.refs_changed && !refs.workdir_changed);

        // logs/COMMIT_EDITMSG 等其他 .git 内部路径保守归 refs
        let logs = git_panel_watch_classify_path("E:/repo/.git/logs/HEAD");
        assert!(logs.refs_changed);

        let commit_editmsg = git_panel_watch_classify_path("E:/repo/.git/COMMIT_EDITMSG");
        assert!(commit_editmsg.refs_changed);

        // Windows 反斜杠同样可分类
        let backslash = git_panel_watch_classify_path("E:\\repo\\.git\\HEAD");
        assert!(backslash.head_changed);
    }

    #[test]
    fn classify_workspace_paths_as_workdir() {
        for path in [
            "E:/repo/src/main.rs",
            "E:/repo/.gitignore",
            "E:/repo/.github/workflows/ci.yml",
            "E:/my.gitrepo/file.txt",
            "E:/repo/sub/.gitconfig-note.txt",
        ] {
            let signal = git_panel_watch_classify_path(path);
            assert!(
                signal.workdir_changed && !signal.head_changed && !signal.refs_changed,
                "{path} 应归类为工作区变化"
            );
        }
    }

    #[test]
    fn classify_skips_git_in_repo_name_and_uses_real_git_dir() {
        // 仓库名含 ".git"（如 dotfiles.git）：应跳过仓库名内的假匹配，
        // 命中真正的 .git 目录后按既有规则分类
        let head = git_panel_watch_classify_path("E:/dotfiles.git/.git/HEAD");
        assert!(head.head_changed && !head.workdir_changed && !head.refs_changed);

        let refs = git_panel_watch_classify_path("E:/dotfiles.git/.git/refs/heads/main");
        assert!(refs.refs_changed && !refs.workdir_changed && !refs.head_changed);

        let index = git_panel_watch_classify_path("E:/dotfiles.git/.git/index");
        assert!(index.workdir_changed && !index.head_changed && !index.refs_changed);

        // 仓库名含 .git 但无真实 git 目录：仍按工作区处理
        let workdir = git_panel_watch_classify_path("E:/dotfiles.git/src/main.rs");
        assert!(workdir.workdir_changed && !workdir.head_changed && !workdir.refs_changed);
    }

    #[test]
    fn resolve_git_dir_follows_gitdir_file_targets() {
        let temp = std::env::temp_dir().join(format!("pai-git-watch-test-{}", std::process::id()));
        let repo = temp.join("repo");
        let real_git = temp.join("real-git");
        std::fs::create_dir_all(&real_git).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        // worktree/submodule 形态：.git 是文件，gitdir 目标为相对路径（基于仓库根解析）
        std::fs::write(repo.join(".git"), "gitdir: ../real-git\n").unwrap();
        let resolved = git_panel_watch_resolve_git_dir(&repo).unwrap();
        assert_eq!(
            std::fs::canonicalize(&resolved).unwrap(),
            std::fs::canonicalize(&real_git).unwrap()
        );
        // 绝对目标同样支持
        std::fs::write(
            repo.join(".git"),
            format!("gitdir: {}\n", real_git.display()),
        )
        .unwrap();
        let resolved_abs = git_panel_watch_resolve_git_dir(&repo).unwrap();
        assert_eq!(resolved_abs, real_git);
        // 普通仓库形态：.git 是目录，直接返回
        let normal = temp.join("normal");
        std::fs::create_dir_all(normal.join(".git")).unwrap();
        let resolved_normal = git_panel_watch_resolve_git_dir(&normal).unwrap();
        assert_eq!(resolved_normal, normal.join(".git"));
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn timing_action_follows_degrade_thresholds() {
        // 正常监听中：慢 → 降级
        assert_eq!(
            git_panel_watch_decide_timing_action(500, false),
            GitPanelWatchTimingAction::EnterDegraded
        );
        // 边界：严格大于阈值才降级
        assert_eq!(
            git_panel_watch_decide_timing_action(201, false),
            GitPanelWatchTimingAction::EnterDegraded
        );
        // 已降级：快 → 恢复；阈值处也视为快速
        assert_eq!(
            git_panel_watch_decide_timing_action(150, true),
            GitPanelWatchTimingAction::ExitDegraded
        );
        assert_eq!(
            git_panel_watch_decide_timing_action(200, true),
            GitPanelWatchTimingAction::ExitDegraded
        );
        // 已降级仍慢：保持现状
        assert_eq!(
            git_panel_watch_decide_timing_action(900, true),
            GitPanelWatchTimingAction::None
        );
        // 正常监听且不慢：不动
        assert_eq!(
            git_panel_watch_decide_timing_action(80, false),
            GitPanelWatchTimingAction::None
        );
    }

    #[test]
    fn ignore_filter_matches_root_gitignore_rules() {
        let base = std::env::temp_dir().join(format!(
            "pai-git-watch-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let repo = base.join("repo");
        std::fs::create_dir_all(repo.join("src")).expect("创建测试目录结构");
        std::fs::create_dir_all(repo.join("target/debug")).expect("创建测试目录结构");
        std::fs::create_dir_all(repo.join("node_modules/pkg")).expect("创建测试目录结构");
        std::fs::write(repo.join(".gitignore"), "target/\n*.log\n!keep.log\nnode_modules\n")
            .expect("写入测试 .gitignore");

        let filter = GitPanelIgnoreFilter::load(&repo);
        let ignored_paths = [
            "target/debug/a.o",
            "build.log",
            "sub/dir/deep.log",
            "node_modules/pkg/index.js",
            "node_modules",
        ];
        for relative in ignored_paths {
            assert!(
                filter.is_ignored(&repo.join(relative)),
                "{relative} 应被忽略规则命中"
            );
        }
        let kept_paths = ["src/main.rs", "keep.log", ".gitignore"];
        for relative in kept_paths {
            assert!(
                !filter.is_ignored(&repo.join(relative)),
                "{relative} 不应被忽略"
            );
        }
        // 仓库外路径一律不过滤
        assert!(!filter.is_ignored(&base.join("outside/file.txt")));

        let _ = std::fs::remove_dir_all(base);
    }
}
