// ==================== Git 面板 ====================
// 统一 git CLI 封装 + 白名单命令面。
// 所有命令只接收业务字段（路径、消息、hash 等），不接受任意 args；
// 参数安全在 Rust 侧硬编码约束，前端无法注入任意 git 参数。

// ---------- 输入结构 ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorkspaceInput {
    workspace_path: String,
}

/// 工作区 + 仓库根：工作树查询与「最近打开」记录共用同一组入参。
/// workspace_path 用于定位该工作区的历史分组，repo_root 用于定位仓库本身
/// （面板当前浏览目录与会话工作区不一定在同一仓库）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorkspaceRepoInput {
    workspace_path: String,
    repo_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelPathsInput {
    workspace_path: String,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCommitInput {
    workspace_path: String,
    message: String,
    amend: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelStashInput {
    workspace_path: String,
    message: String,
    staged: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelStashRefInput {
    workspace_path: String,
    stash_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelBranchInput {
    workspace_path: String,
    name: String,
    start_point: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCheckoutInput {
    workspace_path: String,
    reference: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelShowInput {
    workspace_path: String,
    hash: String,
    /// 为空时查看整个提交的 diff
    path: String,
    /// 上下文行数（-U 参数）；None 走 git 默认（3）
    context: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelLogInput {
    workspace_path: String,
    limit: Option<usize>,
    skip: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCommitFilesInput {
    workspace_path: String,
    hash: String,
}

// ---------- 输出结构 ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelDetectOutput {
    git_available: bool,
    repo_root: Option<String>,
    checked: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelStatusEntry {
    path: String,
    staged_status: String,
    unstaged_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelStatusOutput {
    repo_root: String,
    branch: String,
    entries: Vec<GitPanelStatusEntry>,
    /// 实际变更条目数超过展示上限时置 true（前端显示 1000+，不再全部加载）
    truncated: bool,
    /// 截断前暂存组实际数量（前端折叠条尾部显示）
    staged_total: usize,
    /// 截断前更改组实际数量（前端折叠条尾部显示）
    unstaged_total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelHeadStateOutput {
    /// 当前分支名；HEAD 游离时为空串
    branch: String,
    /// 变基进行中：HEAD 游离且收尾会重写历史，此时不允许切换分支
    rebase_in_progress: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelDiffOutput {
    diff: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCommitFileEntry {
    path: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCommitFilesOutput {
    entries: Vec<GitPanelCommitFileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelBranchEntry {
    name: String,
    is_current: bool,
    is_remote: bool,
    /// 分支尖端那次提交的日期（ISO 8601）；前端据此按「新的放前面」排序
    committer_date: String,
    /// 上游分支短名（如 origin/main）；空串表示没有配置上游
    upstream: String,
    /// 上游已被删除（track 报 gone）
    upstream_missing: bool,
    /// 本地领先上游的提交数
    ahead: u32,
    /// 本地落后上游的提交数
    behind: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelRemoteEntry {
    name: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelStashEntry {
    reference: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelLogEntry {
    hash: String,
    short_hash: String,
    author: String,
    date: String,
    message: String,
    parents: Vec<String>,
    refs: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelLogOutput {
    entries: Vec<GitPanelLogEntry>,
}

// ---------- 路径与参数校验 ----------

fn git_panel_validate_path(path: &str) -> Result<String, String> {
    let trimmed = path.trim().to_string();
    if trimmed.is_empty() {
        return Err("路径不能为空".to_string());
    }
    if trimmed.contains('\0') {
        return Err("路径包含非法字符".to_string());
    }
    if trimmed.starts_with('-') {
        return Err("路径不能以 - 开头".to_string());
    }
    Ok(trimmed)
}

fn git_panel_validate_paths(paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Err("路径列表不能为空".to_string());
    }
    paths.iter().map(|item| git_panel_validate_path(item)).collect()
}

fn git_panel_validate_reference(reference: &str) -> Result<String, String> {
    let trimmed = reference.trim().to_string();
    if trimmed.is_empty() {
        return Err("引用不能为空".to_string());
    }
    if trimmed.contains('\0') {
        return Err("引用包含非法字符".to_string());
    }
    if trimmed.starts_with('-') {
        return Err("引用不能以 - 开头".to_string());
    }
    Ok(trimmed)
}

fn git_panel_validate_message(message: &str) -> Result<String, String> {
    let trimmed = message.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交信息不能为空".to_string());
    }
    if trimmed.contains('\0') {
        return Err("提交信息包含非法字符".to_string());
    }
    Ok(trimmed)
}

fn git_panel_validate_hash(hash: &str) -> Result<String, String> {
    let trimmed = hash.trim().to_string();
    if trimmed.is_empty() {
        return Err("提交哈希不能为空".to_string());
    }
    if trimmed.contains('\0') || trimmed.contains(' ') {
        return Err("提交哈希格式非法".to_string());
    }
    Ok(trimmed)
}

fn git_panel_validate_branch_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        return Err("分支名不能为空".to_string());
    }
    if trimmed.contains('\0') || trimmed.contains(' ') || trimmed.starts_with('-') {
        return Err("分支名格式非法".to_string());
    }
    Ok(trimmed)
}

// ---------- git CLI 封装（执行逻辑见 git_executor.rs） ----------

/// 在 workspace_path 向上探测仓库根（git rev-parse --show-toplevel）。
async fn git_panel_resolve_root(workspace_path: &str) -> Result<String, String> {
    let normalized = git_panel_validate_path(workspace_path)?;
    match git_executor()
        .run_read(&normalized, &["rev-parse", "--show-toplevel"])
        .await
    {
        Ok(stdout) => {
            let root = stdout.trim().to_string();
            if root.is_empty() {
                return Err("Git 仓库探测失败：无法解析仓库根".to_string());
            }
            Ok(root)
        }
        Err(err) => {
            if err.contains("not a git repository") || err.contains("不是 git 仓库") {
                return Err("当前目录不是 Git 仓库".to_string());
            }
            Err(format!("Git 仓库探测失败：{err}"))
        }
    }
}

/// 获取当前分支名（detached HEAD 时返回空）。
async fn git_panel_current_branch(workdir: &str) -> String {
    git_executor().run_read(workdir, &["branch", "--show-current"])
        .await
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// git 目录下存在 rebase-merge / rebase-apply 即视为变基进行中。
/// 交互式变基与普通变基落 rebase-merge，apply 后端落 rebase-apply，任一存在都算。
fn git_panel_git_dir_is_rebasing(git_dir: &std::path::Path) -> bool {
    git_dir.join("rebase-merge").is_dir() || git_dir.join("rebase-apply").is_dir()
}

/// 当前工作树是否处于变基中。
/// 用 `--absolute-git-dir` 取工作树专属 git 目录，链接工作树里的变基现场同样能定位到。
async fn git_panel_rebase_in_progress(workdir: &str) -> bool {
    let Ok(git_dir) = git_executor()
        .run_read(workdir, &["rev-parse", "--absolute-git-dir"])
        .await
    else {
        return false;
    };
    let git_dir = git_dir.trim();
    if git_dir.is_empty() {
        return false;
    }
    git_panel_git_dir_is_rebasing(std::path::Path::new(git_dir))
}

fn git_panel_parse_status_entry(record: &str) -> Option<GitPanelStatusEntry> {
    // porcelain v1 -z 的记录格式：XY path；rename/copy 会附带第二条 old-path 记录，
    // 该记录开头不是合法的 XY 状态列（两字符 + 空格），直接跳过。
    let mut chars = record.chars();
    let staged = chars.next()?;
    let unstaged = chars.next()?;
    if chars.next()? != ' ' {
        return None;
    }
    let rest = chars.as_str();
    let path = rest.trim_start_matches(' ').to_string();
    if path.is_empty() {
        return None;
    }
    // rename/copy 时 rest 形如 "new -> old"，取 -> 前的目标路径
    let display_path = path.split(" -> ").next().unwrap_or(&path).to_string();
    Some(GitPanelStatusEntry {
        path: display_path,
        staged_status: staged.to_string(),
        unstaged_status: unstaged.to_string(),
    })
}

/// 从 porcelain v1 -z 记录中提取文件路径（忽略 rename 的旧路径记录）。
fn git_panel_parse_status_path(record: &str) -> Option<String> {
    let mut chars = record.chars();
    chars.next()?;
    chars.next()?;
    if chars.next()? != ' ' {
        return None;
    }
    let path = chars.as_str().trim_start_matches(' ').to_string();
    if path.is_empty() {
        return None;
    }
    let display_path = path.split(" -> ").next().unwrap_or(&path).to_string();
    Some(display_path)
}

// ---------- 命令：探测 ----------

#[tauri::command]
async fn git_panel_detect(input: GitPanelWorkspaceInput) -> Result<GitPanelDetectOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let version = git_executor().run_read(&workspace_path, &["--version"]).await;
    if version.is_err() {
        return Ok(GitPanelDetectOutput {
            git_available: false,
            repo_root: None,
            checked: false,
            error: Some("无法运行 git 命令".to_string()),
        });
    }
    match git_panel_resolve_root(&workspace_path).await {
        Ok(root) => Ok(GitPanelDetectOutput {
            git_available: true,
            repo_root: Some(root),
            checked: true,
            error: None,
        }),
        Err(err) => Ok(GitPanelDetectOutput {
            git_available: true,
            repo_root: None,
            checked: true,
            error: Some(err),
        }),
    }
}

// ---------- 命令：状态 ----------

/// 状态查询核心实现：tauri 命令与 IDE jsonrpc 分发共用。
async fn git_panel_status_inner(input: GitPanelWorkspaceInput) -> Result<GitPanelStatusOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let branch = git_panel_current_branch(&repo_root).await;
    // --no-optional-locks：只读查询不刷新索引，否则会写 .git/index.lock 触发面板自身的监视
    let stdout = git_executor().run_read(
        &repo_root,
        &["--no-optional-locks", "status", "--porcelain=v1", "-z", "-uall"],
    )
    .await?;
    let entries: Vec<GitPanelStatusEntry> = stdout
        .split('\0')
        .filter(|record| !record.is_empty())
        .filter_map(git_panel_parse_status_entry)
        .collect();
    // 截断前按前端分组规则统计两组实际数量（折叠条尾部显示用，可能同时属于两组）
    let staged_total = entries
        .iter()
        .filter(|entry| git_panel_entry_is_staged(entry))
        .count();
    let unstaged_total = entries
        .iter()
        .filter(|entry| git_panel_entry_is_unstaged(entry))
        .count();
    let (entries, truncated) = git_panel_truncate_status_entries(entries);
    Ok(GitPanelStatusOutput {
        repo_root,
        branch,
        entries,
        truncated,
        staged_total,
        unstaged_total,
    })
}

#[tauri::command]
async fn git_panel_status(app: tauri::AppHandle, input: GitPanelWorkspaceInput) -> Result<GitPanelStatusOutput, String> {
    let started = std::time::Instant::now();
    let result = git_panel_status_inner(input).await;
    // 自适应降级状态机收敛点：任何一次 status 调用（事件刷新/focus/手动/操作收尾/tab 补载）
    // 的耗时都驱动 watcher 降级或恢复（用户策略），无需专门探测定时器
    if let Ok(output) = &result {
        git_panel_watch_adapt_after_status(&output.repo_root, started.elapsed().as_millis(), app).await;
    }
    result
}

/// HEAD 状态：当前分支 + 是否变基中。
/// 供工作区卡片判断能否切换分支；比 status 轻，不扫工作区变更。
async fn git_panel_head_state_inner(
    input: GitPanelWorkspaceInput,
) -> Result<GitPanelHeadStateOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    Ok(GitPanelHeadStateOutput {
        branch: git_panel_current_branch(&repo_root).await,
        rebase_in_progress: git_panel_rebase_in_progress(&repo_root).await,
    })
}

#[tauri::command]
async fn git_panel_head_state(
    input: GitPanelWorkspaceInput,
) -> Result<GitPanelHeadStateOutput, String> {
    git_panel_head_state_inner(input).await
}

/// 前端暂存组过滤规则的 Rust 复刻：X 列非空且非 ?，且排除「未跟踪 + 已暂存」矛盾项。
fn git_panel_entry_is_staged(entry: &GitPanelStatusEntry) -> bool {
    let staged = entry.staged_status.trim();
    let unstaged = entry.unstaged_status.trim();
    staged != "" && staged != "?" && !(unstaged == "?" && staged == "?")
}

/// 前端更改组过滤规则的 Rust 复刻：未跟踪（??）或 Y 列非空，且不是「已暂存但未更改」。
fn git_panel_entry_is_unstaged(entry: &GitPanelStatusEntry) -> bool {
    let staged = entry.staged_status.trim();
    let unstaged = entry.unstaged_status.trim();
    if staged == "?" && unstaged == "?" {
        return true;
    }
    unstaged != "" && !(staged != "" && staged != "?" && unstaged == "")
}

/// 变更条目过多时只返回前 1000 条，避免大仓库全量渲染卡死前端；前端据此显示 1000+。
const GIT_PANEL_STATUS_MAX_ENTRIES: usize = 1000;

fn git_panel_truncate_status_entries(
    entries: Vec<GitPanelStatusEntry>,
) -> (Vec<GitPanelStatusEntry>, bool) {
    let truncated = entries.len() > GIT_PANEL_STATUS_MAX_ENTRIES;
    let entries: Vec<GitPanelStatusEntry> = entries
        .into_iter()
        .take(GIT_PANEL_STATUS_MAX_ENTRIES)
        .collect();
    (entries, truncated)
}

// ---------- 命令：diff ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelDiffInput {
    workspace_path: String,
    path: String,
    /// staged=true 表示暂存区 vs HEAD（git diff --cached）；false 表示工作区 vs 暂存区
    staged: bool,
    /// 提供 hash 时查看某次提交的该文件 diff（git show hash -- path）
    hash: String,
    /// 上下文行数（-U 参数）；None 走 git 默认（3）
    context: Option<usize>,
}

#[tauri::command]
async fn git_panel_diff(input: GitPanelDiffInput) -> Result<GitPanelDiffOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let path = git_panel_validate_path(&input.path)?;

    let context_arg = input.context.filter(|&n| n > 0).map(|n| format!("-U{n}"));
    let context_refs: Vec<&str> = context_arg.iter().map(String::as_str).collect();

    let diff = if !input.hash.trim().is_empty() {
        let hash = git_panel_validate_hash(&input.hash)?;
        let mut full: Vec<&str> = vec!["show", "--format=", "--no-ext-diff"];
        full.extend(context_refs.iter().copied());
        full.extend([hash.as_str(), "--", path.as_str()]);
        git_executor().run_read(&repo_root, &full).await?
    } else if input.staged {
        let mut full: Vec<&str> = vec!["diff", "--cached", "--no-ext-diff"];
        full.extend(context_refs.iter().copied());
        full.extend(["--", path.as_str()]);
        git_executor().run_read(&repo_root, &full).await?
    } else {
        let mut full: Vec<&str> = vec!["diff", "--no-ext-diff"];
        full.extend(context_refs.iter().copied());
        full.extend(["--", path.as_str()]);
        git_executor().run_read(&repo_root, &full).await?
    };
    Ok(GitPanelDiffOutput { diff })
}

// ---------- 命令：暂存 / 取消暂存 ----------

#[tauri::command]
async fn git_panel_stage(input: GitPanelPathsInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let paths = git_panel_validate_paths(&input.paths)?;
    let mut args: Vec<&str> = vec!["add", "--"];
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    args.extend(path_refs);
    git_executor().run_write(&repo_root, &args).await
}

#[tauri::command]
async fn git_panel_unstage(input: GitPanelPathsInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let paths = git_panel_validate_paths(&input.paths)?;
    let mut args: Vec<&str> = vec!["restore", "--staged", "--"];
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    args.extend(path_refs);
    git_executor().run_write(&repo_root, &args).await
}

// ---------- 命令：提交 ----------

#[tauri::command]
async fn git_panel_commit(input: GitPanelCommitInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let mut args: Vec<String> = vec!["commit".to_string()];
    if input.amend {
        args.push("--amend".to_string());
        let trimmed = input.message.trim();
        if trimmed.is_empty() {
            // 空消息 + amend = 只合并暂存内容到上一个 commit，保留原提交信息。
            args.push("--no-edit".to_string());
        } else {
            let message = git_panel_validate_message(&input.message)?;
            args.push("-m".to_string());
            args.push(message);
        }
    } else {
        let message = git_panel_validate_message(&input.message)?;
        args.push("-m".to_string());
        args.push(message);
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    git_executor().run_write(&repo_root, &arg_refs).await
}

// ---------- 命令：丢弃 ----------

#[tauri::command]
async fn git_panel_discard(input: GitPanelPathsInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let paths = git_panel_validate_paths(&input.paths)?;
    // restore 只对已跟踪文件生效；未跟踪文件（??）会报 pathspec 不匹配，需改用 clean 删除。
    // 先查已跟踪列表，再分流执行。
    let mut ls_args: Vec<&str> = vec!["ls-files", "--"];
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    ls_args.extend(path_refs.iter().copied());
    let tracked_stdout = git_executor().run_read(&repo_root, &ls_args).await?;
    let tracked: std::collections::HashSet<&str> = tracked_stdout.lines().collect();

    let mut restore_args: Vec<&str> = vec!["restore", "--staged", "--worktree", "--"];
    let mut clean_args: Vec<&str> = vec!["clean", "-f", "--"];
    let mut has_restore = false;
    let mut has_clean = false;
    for path in &paths {
        if tracked.contains(path.as_str()) {
            restore_args.push(path);
            has_restore = true;
        } else {
            clean_args.push(path);
            has_clean = true;
        }
    }

    let mut stdout_parts: Vec<String> = Vec::new();
    if has_restore {
        // restore 是写命令：走互斥 + 写后失效
        let out = git_executor().run_write(&repo_root, &restore_args).await?;
        if !out.stdout.trim().is_empty() {
            stdout_parts.push(out.stdout.trim().to_string());
        }
    }
    if has_clean {
        // clean 是写命令：走互斥 + 写后失效
        let out = git_executor().run_write(&repo_root, &clean_args).await?;
        if !out.stdout.trim().is_empty() {
            stdout_parts.push(out.stdout.trim().to_string());
        }
    }
    Ok(GitPanelRunOutput {
        stdout: stdout_parts.join("\n"),
        stderr: String::new(),
        exit_code: 0,
    })
}

// ---------- 命令：储藏 ----------

#[tauri::command]
async fn git_panel_stash_list(input: GitPanelWorkspaceInput) -> Result<Vec<GitPanelStashEntry>, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let stdout = git_executor().run_read(&repo_root, &["stash", "list"]).await?;
    let entries = stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let reference = trimmed.split(':').next().unwrap_or("").trim().to_string();
            let message = trimmed
                .splitn(2, ':')
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
            if reference.is_empty() {
                return None;
            }
            Some(GitPanelStashEntry { reference, message })
        })
        .collect();
    Ok(entries)
}

#[tauri::command]
async fn git_panel_stash_create(input: GitPanelStashInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let message = input.message.trim().to_string();
    let mut args: Vec<&str> = vec!["stash", "push"];
    if input.staged {
        args.push("--staged");
    }
    if !message.is_empty() {
        args.push("-m");
        args.push(&message);
    }
    git_executor().run_write(&repo_root, &args).await
}

#[tauri::command]
async fn git_panel_stash_apply(input: GitPanelStashRefInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.stash_ref)?;
    git_executor().run_write(&repo_root, &["stash", "apply", &reference]).await
}

#[tauri::command]
async fn git_panel_stash_pop(input: GitPanelStashRefInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.stash_ref)?;
    git_executor().run_write(&repo_root, &["stash", "pop", &reference]).await
}

#[tauri::command]
async fn git_panel_stash_drop(input: GitPanelStashRefInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.stash_ref)?;
    git_executor().run_write(&repo_root, &["stash", "drop", &reference]).await
}

#[tauri::command]
async fn git_panel_stash_files(input: GitPanelStashRefInput) -> Result<GitPanelCommitFilesOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.stash_ref)?;
    // --name-status 输出 "状态\t路径"；--no-renames 保证 rename 按 删+增 输出
    let stdout = git_executor().run_read(&repo_root, &["stash", "show", "--name-status", "--no-renames", &reference])
        .await?;
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(3, '\t');
        let Some(status) = parts.next() else { continue };
        let Some(path) = parts.next() else { continue };
        if status.is_empty() || path.is_empty() {
            continue;
        }
        entries.push(GitPanelCommitFileEntry {
            path: path.to_string(),
            status: status.chars().next().unwrap_or('?').to_string(),
        });
    }
    Ok(GitPanelCommitFilesOutput { entries })
}

// ---------- 命令：分支 ----------

/// 解析 `git for-each-ref` 输出（每条引用一行，字段以 US 即 \u{1f} 分隔）：
/// `%(HEAD) \u{1f} %(refname) \u{1f} %(committerdate:iso-strict) \u{1f} %(symref) \u{1f} %(upstream:short) \u{1f} %(upstream:track)`。
/// 符号引用（如 origin/HEAD）不是真实分支，跳过。
fn parse_branch_refs(stdout: &str) -> Vec<GitPanelBranchEntry> {
    stdout
        .lines()
        .filter_map(|line| {
            if line.is_empty() {
                return None;
            }
            let mut parts = line.split('\u{1f}');
            let head = parts.next().unwrap_or("");
            let full_name = parts.next().unwrap_or("");
            let committer_date = parts.next().unwrap_or("").to_string();
            let symref = parts.next().unwrap_or("");
            let upstream = parts.next().unwrap_or("").to_string();
            let track = parts.next().unwrap_or("");
            if full_name.is_empty() || !symref.is_empty() {
                return None;
            }
            // 引用全名 → 展示名：本地去掉 refs/heads/，远程去掉 refs/remotes/（保留 origin/ 前缀）
            let (is_remote, display_name) = if let Some(rest) = full_name.strip_prefix("refs/heads/") {
                (false, rest.to_string())
            } else if let Some(rest) = full_name.strip_prefix("refs/remotes/") {
                (true, rest.to_string())
            } else {
                return None;
            };
            if display_name.is_empty() {
                return None;
            }
            let (ahead, behind, upstream_missing) = parse_upstream_track(track);
            Some(GitPanelBranchEntry {
                is_current: head.starts_with('*'),
                name: display_name,
                is_remote,
                committer_date,
                upstream,
                upstream_missing,
                ahead,
                behind,
            })
        })
        .collect()
}

/// 解析 `%(upstream:track)` 输出，形如 `[ahead 3]`、`[behind 2]`、
/// `[ahead 1, behind 1]`、`[gone]`；无上游或完全同步时为空串。
/// 返回 (领先数, 落后数, 上游是否已删除)。
fn parse_upstream_track(track: &str) -> (u32, u32, bool) {
    if track.is_empty() {
        return (0, 0, false);
    }
    if track.contains("gone") {
        return (0, 0, true);
    }
    (
        parse_track_count(track, "ahead"),
        parse_track_count(track, "behind"),
        false,
    )
}

/// 从 `ahead 3` / `behind 2` 这类片段里取出数字；缺失或解析失败按 0 处理。
fn parse_track_count(track: &str, keyword: &str) -> u32 {
    let Some(idx) = track.find(keyword) else {
        return 0;
    };
    track[idx + keyword.len()..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[tauri::command]
async fn git_panel_branch_list(input: GitPanelWorkspaceInput) -> Result<Vec<GitPanelBranchEntry>, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    // 用 for-each-ref 一次拿到：是否为 HEAD、引用全名、末次提交日期、符号引用目标、
    // 上游短名与上游跟踪状态（领先/落后/已删除）。
    // 字段以 US（\u{1f}）分隔；引用名不含 US，天然安全。
    let stdout = git_executor()
        .run_read(
            &repo_root,
            &[
                "for-each-ref",
                "--format=%(HEAD)%1f%(refname)%1f%(committerdate:iso-strict)%1f%(symref)%1f%(upstream:short)%1f%(upstream:track)",
                "refs/heads",
                "refs/remotes",
            ],
        )
        .await?;
    Ok(parse_branch_refs(&stdout))
}

#[tauri::command]
async fn git_panel_branch_create(input: GitPanelBranchInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let name = git_panel_validate_branch_name(&input.name)?;
    let start_point = if input.start_point.trim().is_empty() {
        String::new()
    } else {
        git_panel_validate_reference(&input.start_point)?
    };
    let mut args: Vec<&str> = vec!["branch", &name];
    if !start_point.is_empty() {
        args.push(&start_point);
    }
    git_executor().run_write(&repo_root, &args).await
}

#[tauri::command]
async fn git_panel_branch_delete(input: GitPanelBranchInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let name = git_panel_validate_branch_name(&input.name)?;
    // -d 仅删除已合并分支；未合并时 git 会拒绝并提示，前端可再确认用 -D
    git_executor().run_write(&repo_root, &["branch", "-d", &name]).await
}

// ---------- 命令：签出 ----------

#[tauri::command]
async fn git_panel_checkout(input: GitPanelCheckoutInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.reference)?;
    // 与预检同一道闸：变基中切换分支会破坏变基现场，任何调用路径都不放行
    if git_panel_rebase_in_progress(&repo_root).await {
        return Err("变基进行中，无法切换分支，请先完成或中止本次变基".to_string());
    }
    git_executor().run_write(&repo_root, &["checkout", &reference]).await
}

/// 撤销最近一次提交（soft）：HEAD 回退一位，改动保留在暂存区。
/// 已推送校验：当前分支有 upstream 且 HEAD 已是 upstream 祖先（远端历史包含该提交）时拒绝执行，
/// 否则软退回会导致本地与远端分叉、必须强推才能同步。
#[tauri::command]
async fn git_panel_reset_soft(input: GitPanelWorkspaceInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;

    // 1. 当前分支是否有 upstream：rev-parse @{u} 失败即无 upstream（未推送过的分支直接允许）
    let upstream = git_executor()
        .run_read(&repo_root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .await
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    if let Some(upstream) = upstream {
        // 2. HEAD 是否已是 upstream 历史的一部分：is-ancestor 成功 = 已推送
        let is_ancestor = git_executor()
            .run_read(&repo_root, &["merge-base", "--is-ancestor", "HEAD", &upstream])
            .await
            .is_ok();
        if is_ancestor {
            return Err("已经在远端，无法软退回".to_string());
        }
    }

    git_executor().run_write(&repo_root, &["reset", "--soft", "HEAD~1"]).await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelCheckoutCheckOutput {
    /// 工作区未提交/未跟踪的文件路径
    dirty_paths: Vec<String>,
    /// 目标分支相对当前 HEAD 修改的文件路径
    changed_paths: Vec<String>,
    /// 交集：切换会覆盖或冲突的文件路径
    conflicting_paths: Vec<String>,
}

/// 切换分支预检：对比工作区未提交文件与目标分支改动，找出会冲突的文件
#[tauri::command]
async fn git_panel_checkout_check(input: GitPanelCheckoutInput) -> Result<GitPanelCheckoutCheckOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let reference = git_panel_validate_reference(&input.reference)?;

    // 变基进行中 HEAD 游离，且 rebase 收尾依赖当前工作区状态：此时切换分支会破坏变基现场。
    // 直接拒绝并给出可读原因，避免把游离 HEAD 的状态文本当成引用继续往下传给 git。
    if git_panel_rebase_in_progress(&repo_root).await {
        return Err("变基进行中，无法切换分支，请先完成或中止本次变基".to_string());
    }

    // 工作区未提交/未跟踪文件（含重命名等，-z 按 NUL 分隔）
    // --no-optional-locks：预检是只读查询，不刷新索引
    let status_stdout = git_executor().run_read(
        &repo_root,
        &["--no-optional-locks", "status", "--porcelain=v1", "-z", "-uall"],
    )
    .await?;
    let dirty_paths: Vec<String> = status_stdout
        .split('\0')
        .filter(|record| !record.is_empty())
        .filter_map(git_panel_parse_status_path)
        .collect();

    // 目标分支相对当前 HEAD 修改的文件
    let changed_stdout = git_executor().run_read(
        &repo_root,
        &["diff", "--name-only", "HEAD", &reference],
    )
    .await?;
    let changed_paths: Vec<String> = changed_stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let dirty_set: std::collections::HashSet<&String> = dirty_paths.iter().collect();
    let conflicting_paths: Vec<String> = changed_paths
        .iter()
        .filter(|path| dirty_set.contains(*path))
        .cloned()
        .collect();

    Ok(GitPanelCheckoutCheckOutput {
        dirty_paths,
        changed_paths,
        conflicting_paths,
    })
}

// ---------- 命令：远程 ----------

#[tauri::command]
async fn git_panel_remote_list(input: GitPanelWorkspaceInput) -> Result<Vec<GitPanelRemoteEntry>, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let stdout = git_executor().run_read(&repo_root, &["remote", "-v"]).await?;
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(name) = parts.next() else { continue };
        let Some(url) = parts.next() else { continue };
        if seen.insert(name.to_string()) {
            entries.push(GitPanelRemoteEntry {
                name: name.to_string(),
                url: url.to_string(),
            });
        }
    }
    Ok(entries)
}

// ---------- 命令：同步 ----------

#[tauri::command]
async fn git_panel_fetch(input: GitPanelWorkspaceInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    git_executor().run_network(&repo_root, &["fetch"]).await
}

#[tauri::command]
async fn git_panel_pull(input: GitPanelWorkspaceInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    git_executor().run_network(&repo_root, &["pull"]).await
}

#[tauri::command]
async fn git_panel_push(input: GitPanelWorkspaceInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    git_executor().run_network(&repo_root, &["push"]).await
}

#[tauri::command]
async fn git_panel_sync(input: GitPanelWorkspaceInput) -> Result<GitPanelRunOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let fetch_result = git_executor().run_network(&repo_root, &["fetch"]).await?;
    if fetch_result.exit_code != 0 {
        return Ok(fetch_result);
    }
    git_executor().run_network(&repo_root, &["pull"]).await
}

// ---------- 命令：历史与提交图 ----------

#[tauri::command]
async fn git_panel_log(input: GitPanelLogInput) -> Result<GitPanelLogOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let limit = input.limit.unwrap_or(100).clamp(1, 500);
    let skip = input.skip.unwrap_or(0);

    let mut args: Vec<String> = vec![
        "log".to_string(),
        "-n".to_string(),
        limit.to_string(),
        // decorate 用 full：%D 输出 refs/heads/、refs/remotes/<remote>/、refs/tags/ 全名，
        // 前端据此区分本地分支、远程跟踪分支与 tag（short 形式无法区分同名本地/远程）
        "--decorate=full".to_string(),
        "--format=%H%x1f%h%x1f%an%x1f%aI%x1f%P%x1f%D%x1f%B%x1e".to_string(),
    ];
    if skip > 0 {
        args.push("--skip".to_string());
        args.push(skip.to_string());
    }
    let args_ref: Vec<&str> = args.iter().map(String::as_str).collect();
    let stdout = git_executor().run_read(&repo_root, &args_ref).await?;
    let entries = stdout
        .split('\u{1e}')
        .filter_map(|record| {
            let trimmed = record.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.splitn(7, '\u{1f}');
            let hash = parts.next().unwrap_or("").to_string();
            let short_hash = parts.next().unwrap_or("").to_string();
            let author = parts.next().unwrap_or("").to_string();
            let date = parts.next().unwrap_or("").to_string();
            let parents = parts
                .next()
                .unwrap_or("")
                .split_whitespace()
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect();
            let refs = parts.next().unwrap_or("").to_string();
            let message = parts.next().unwrap_or("").trim().to_string();
            if hash.is_empty() {
                return None;
            }
            Some(GitPanelLogEntry {
                hash,
                short_hash,
                author,
                date,
                message,
                parents,
                refs,
            })
        })
        .collect();
    Ok(GitPanelLogOutput { entries })
}

// ---------- 命令：提交 diff ----------

#[tauri::command]
async fn git_panel_show(input: GitPanelShowInput) -> Result<GitPanelDiffOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let hash = git_panel_validate_hash(&input.hash)?;
    let path = input.path.trim().to_string();

    let context_arg = input.context.filter(|&n| n > 0).map(|n| format!("-U{n}"));
    let context_refs: Vec<&str> = context_arg.iter().map(String::as_str).collect();

    // stash 是合并提交：git show 只会输出 combined diff（diff --cc），可读性差；
    // 改走 diff 第一父提交（stash 创建时的 HEAD），得到标准 diff
    let is_stash = hash.starts_with("stash@{");
    let parent_ref = format!("{hash}^1");
    let diff = if path.is_empty() {
        if is_stash {
            let mut full: Vec<&str> = vec!["diff", &parent_ref, &hash];
            full.extend(context_refs.iter().copied());
            git_executor().run_read(&repo_root, &full).await?
        } else {
            let mut full: Vec<&str> = vec!["show", "--format=", "--no-ext-diff"];
            full.extend(context_refs.iter().copied());
            full.push(hash.as_str());
            git_executor().run_read(&repo_root, &full).await?
        }
    } else {
        let path = git_panel_validate_path(&path)?;
        if is_stash {
            let mut full: Vec<&str> = vec!["diff", &parent_ref, &hash];
            full.extend(context_refs.iter().copied());
            full.extend(["--", path.as_str()]);
            git_executor().run_read(&repo_root, &full).await?
        } else {
            let mut full: Vec<&str> = vec!["show", "--format=", "--no-ext-diff"];
            full.extend(context_refs.iter().copied());
            full.extend([hash.as_str(), "--", path.as_str()]);
            git_executor().run_read(&repo_root, &full).await?
        }
    };
    Ok(GitPanelDiffOutput { diff })
}

// ---------- 命令：提交文件列表 ----------

#[tauri::command]
async fn git_panel_commit_files(input: GitPanelCommitFilesInput) -> Result<GitPanelCommitFilesOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_resolve_root(&workspace_path).await?;
    let hash = git_panel_validate_hash(&input.hash)?;
    // --name-status 输出 "状态\t路径"；--no-renames 保证 rename 也按 删+增 输出，避免旧路径行干扰
    let stdout = git_executor().run_read(
        &repo_root,
        &["show", "--format=", "--name-status", "--no-renames", &hash],
    )
    .await?;
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(3, '\t');
        let Some(status) = parts.next() else { continue };
        let Some(path) = parts.next() else { continue };
        if status.is_empty() || path.is_empty() {
            continue;
        }
        entries.push(GitPanelCommitFileEntry {
            path: path.to_string(),
            status: status.chars().next().unwrap_or('?').to_string(),
        });
    }
    Ok(GitPanelCommitFilesOutput { entries })
}

// ---------- 命令：仓库列表（多仓库切换） ----------

/// 主动刷新时的扫描深度：相对 workspace_path 的目录层级数（workspace 自身为 0）。
const GIT_REPO_SCAN_MAX_DEPTH: usize = 3;

/// 默认扫描深度：只扫工作区直接子目录（对齐 VSCode 的 repositoryScanMaxDepth=1 远虑），
/// 深层仓库靠「打开过即记住」的历史列表覆盖，主动刷新才扩到 3 层。
const GIT_REPO_SCAN_DEFAULT_DEPTH: usize = 1;

/// 历史上限：最近打开过的仓库最多保留 50 个。
const GIT_REPO_HISTORY_LIMIT: usize = 50;

/// 扫描时跳过的构建/依赖目录。
const GIT_REPO_SCAN_SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", "build"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelRepoEntry {
    path: String,
    name: String,
}

/// 单个工作树条目：path 为工作树根，branch 为已签出分支（分离头指针时为空）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorktreeEntry {
    path: String,
    name: String,
    branch: String,
    /// 主工作树（仓库根本身）；linked worktree 为 false
    is_main: bool,
    /// 分离头指针
    detached: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorktreesOutput {
    repo_root: String,
    worktrees: Vec<GitPanelWorktreeEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelReposOutput {
    repos: Vec<GitPanelRepoEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelDiscoverOutput {
    git_available: bool,
    current_repo_root: Option<String>,
    repos: Vec<GitPanelRepoEntry>,
    default_repo_root: Option<String>,
    checked: bool,
    error: Option<String>,
}

/// 扫描结果缓存：workspace_path → 仓库列表；展开时只读缓存，点刷新（refresh=true）才重扫。
static GIT_REPO_SCAN_CACHE: OnceLock<Mutex<HashMap<String, Vec<GitPanelRepoEntry>>>> = OnceLock::new();

fn git_panel_repo_cache() -> &'static Mutex<HashMap<String, Vec<GitPanelRepoEntry>>> {
    GIT_REPO_SCAN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 仓库根路径比较：统一分隔符（Windows 下再忽略大小写），避免
/// rev-parse 输出的反斜杠路径与前端传入的正斜杠路径被误判为不同仓库。
fn git_panel_repo_path_eq(a: &str, b: &str) -> bool {
    git_panel_normalize_repo_path(a) == git_panel_normalize_repo_path(b)
}

/// 路径归一化：剥离 Windows verbatim 前缀（\\?\）、统一分隔符为 /（Windows 下再转小写），
/// 用于比较与分组键。前端传入的工作区路径常带 \\?\ 前缀，而 git rev-parse 输出不带，
/// 不剥离会导致同一仓库被误判为两个。
fn git_panel_normalize_repo_path(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        git_panel_strip_verbatim_prefix(path).replace('\\', "/").to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.replace('\\', "/")
    }
}

/// 剥离 Windows verbatim 前缀（\\?\），保留其余路径原样；无前缀时原样返回。
fn git_panel_strip_verbatim_prefix(path: &str) -> &str {
    #[cfg(target_os = "windows")]
    {
        path.strip_prefix("\\\\?\\").unwrap_or(path)
    }
    #[cfg(not(target_os = "windows"))]
    {
        path
    }
}

/// 取路径最后一段作为仓库名。
fn git_panel_repo_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string())
}

/// 从 workspace_path 向下递归扫描 .git（目录或文件都算），每个命中的父目录即仓库根。
/// 不进入 .git 内部；跳过忽略名单目录；深度不超过 max_depth。
fn git_panel_scan_repos_inner(workspace_path: &str, max_depth: usize) -> Vec<GitPanelRepoEntry> {
    let mut repos = Vec::new();
    let mut stack = vec![(workspace_path.to_string(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" {
                repos.push(GitPanelRepoEntry {
                    path: dir.clone(),
                    name: git_panel_repo_name(&dir),
                });
                continue;
            }
            if GIT_REPO_SCAN_SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if depth + 1 > max_depth {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push((entry.path().to_string_lossy().to_string(), depth + 1));
            }
        }
    }
    repos
}

// ---------- 打开过的仓库历史（state.sqlite 持久化，按工作区分组） ----------

/// 读取历史：workspace_path(归一化) → 最近打开过的仓库列表。
/// 读取失败时返回空 Map。
async fn git_panel_read_repo_history(state: &AppState) -> HashMap<String, Vec<String>> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        state_service_get_git_repo_history(&state).unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

async fn git_panel_write_repo_history(state: &AppState, history: &HashMap<String, Vec<String>>) {
    let state = state.clone();
    let history = history.clone();
    if let Err(err) = tokio::task::spawn_blocking(move || {
        state_service_save_git_repo_history(&state, &history)
    })
    .await
    {
        runtime_log_warn(format!("[git面板] 仓库历史写盘失败：error={err}"));
    }
}

/// 把仓库根记入历史：按当前工作区精确分组（O(1) 定位，无需遍历匹配），
/// 组内最近在前、去重、封顶 50。返回该组历史是否发生变化。
fn git_panel_history_push(
    history: &mut HashMap<String, Vec<String>>,
    workspace_path: &str,
    repo_root: &str,
) -> bool {
    let key = git_panel_normalize_repo_path(workspace_path);
    let group = history.entry(key).or_default();
    if let Some(index) = group
        .iter()
        .position(|item| git_panel_repo_path_eq(item, repo_root))
    {
        if index == 0 {
            return false;
        }
        let item = group.remove(index);
        group.insert(0, item);
        return true;
    }
    group.insert(0, repo_root.to_string());
    if group.len() > GIT_REPO_HISTORY_LIMIT {
        group.truncate(GIT_REPO_HISTORY_LIMIT);
    }
    true
}

/// 历史序号：命中返回下标（越近越小），未命中返回 usize::MAX，排在所有命中之后。
/// 用于让「最近打开」的顺序不被路径序抹平。
fn git_panel_history_rank(history: &[String], path: &str) -> usize {
    history
        .iter()
        .position(|item| git_panel_repo_path_eq(item, path))
        .unwrap_or(usize::MAX)
}

/// 当前仓库进入历史；写盘失败静默忽略，不影响 git 命令本身。
async fn git_panel_history_remember(state: &AppState, workspace_path: &str, repo_root: &str) {
    let mut history = git_panel_read_repo_history(state).await;
    if git_panel_history_push(&mut history, workspace_path, repo_root) {
        git_panel_write_repo_history(state, &history).await;
    }
}

/// 收集仓库列表：扫描（缓存/重扫）+ 当前仓库根 extra + 历史合并，去重后按路径排序。
/// 供 git_panel_repos 与 git_panel_discover 共用，保证两者列表语义一致。
async fn git_panel_collect_repos(
    workspace_path: &str,
    refresh: bool,
    state: &AppState,
) -> Result<Vec<GitPanelRepoEntry>, String> {
    // 扫描：默认深度 1 走缓存（懒加载只扫一次）；主动刷新扩到深度 3，直接重扫不写缓存
    let scanned = if refresh {
        tokio::task::spawn_blocking({
            let workspace_path = workspace_path.to_string();
            move || git_panel_scan_repos_inner(&workspace_path, GIT_REPO_SCAN_MAX_DEPTH)
        })
        .await
        .map_err(|err| format!("仓库扫描失败：{err}"))?
    } else {
        let cached = {
            let cache = git_panel_repo_cache()
                .lock()
                .map_err(|_| "仓库列表缓存读取失败".to_string())?;
            cache.get(workspace_path).cloned()
        };
        match cached {
            Some(repos) => repos,
            None => {
                let scanned = tokio::task::spawn_blocking({
                    let workspace_path = workspace_path.to_string();
                    move || git_panel_scan_repos_inner(&workspace_path, GIT_REPO_SCAN_DEFAULT_DEPTH)
                })
                .await
                .map_err(|err| format!("仓库扫描失败：{err}"))?;
                git_panel_repo_cache()
                    .lock()
                    .map_err(|_| "仓库列表缓存写入失败".to_string())?
                    .insert(workspace_path.to_string(), scanned.clone());
                scanned
            }
        }
    };
    // 当前仓库根（可能在工作区上方，扫描不到）：记录进历史，并恒保留在列表中
    let mut extra = Vec::new();
    if let Ok(root) = git_panel_resolve_root(workspace_path).await {
        git_panel_history_remember(state, workspace_path, &root).await;
        let name = git_panel_repo_name(&root);
        extra.push(GitPanelRepoEntry { path: root, name });
    }
    // 合并：当前工作区的历史（最近在前，过滤已删除目录）∪ 扫描结果 ∪ 当前仓库根，去重
    let mut repos: Vec<GitPanelRepoEntry> = Vec::new();
    let history_key = git_panel_normalize_repo_path(workspace_path);
    let history_order = git_panel_read_repo_history(state)
        .await
        .get(&history_key)
        .cloned()
        .unwrap_or_default();
    for path in &history_order {
        if !Path::new(path).is_dir() {
            continue;
        }
        if !repos.iter().any(|repo| git_panel_repo_path_eq(&repo.path, path)) {
            repos.push(GitPanelRepoEntry {
                path: path.clone(),
                name: git_panel_repo_name(path),
            });
        }
    }
    for repo in scanned {
        if !repos.iter().any(|item| git_panel_repo_path_eq(&item.path, &repo.path)) {
            repos.push(repo);
        }
    }
    for repo in extra {
        if !repos.iter().any(|item| git_panel_repo_path_eq(&item.path, &repo.path)) {
            repos.push(repo);
        }
    }
    // 返回前统一剥离 verbatim 前缀，前端拿到的都是普通路径（切换后直接可作 git 工作目录）
    for repo in &mut repos {
        repo.path = git_panel_strip_verbatim_prefix(&repo.path).to_string();
    }
    // 排序：最近打开过的在前（越近越靠前），其余按路径；顺序信息来自历史，不能再用路径序覆盖
    repos.sort_by(|a, b| {
        git_panel_history_rank(&history_order, &a.path)
            .cmp(&git_panel_history_rank(&history_order, &b.path))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(repos)
}

/// 推荐默认打开的仓库：向上命中→当前仓库；否则→历史最近命中；否则→仅 1 个仓库时它自己；否则→null。
async fn git_panel_default_repo_root(
    state: &AppState,
    workspace_path: &str,
    current_repo_root: &Option<String>,
    repos: &[GitPanelRepoEntry],
) -> Option<String> {
    if let Some(root) = current_repo_root {
        return Some(git_panel_strip_verbatim_prefix(root).to_string());
    }
    let history_key = git_panel_normalize_repo_path(workspace_path);
    if let Some(history) = git_panel_read_repo_history(state).await.get(&history_key) {
        for path in history {
            if Path::new(path).is_dir() {
                return Some(git_panel_strip_verbatim_prefix(path).to_string());
            }
        }
    }
    if repos.len() == 1 {
        return Some(repos[0].path.clone());
    }
    None
}

// ---------- 命令：当前仓库的工作树列表 ----------

/// 主工作树判定：主工作树的 `.git` 是目录，linked worktree 的 `.git` 是指向
/// 共享 gitdir 的文件。不依赖 `git worktree list` 的输出顺序。
fn git_panel_is_main_worktree(path: &str) -> bool {
    Path::new(path).join(".git").is_dir()
}

/// 组装单个工作树条目：名称取路径最后一段，分支剥掉 `refs/heads/` 前缀。
fn git_panel_build_worktree_entry(path: String, branch: &str, detached: bool) -> GitPanelWorktreeEntry {
    let is_main = git_panel_is_main_worktree(&path);
    GitPanelWorktreeEntry {
        name: git_panel_repo_name(&path),
        path,
        branch: branch.to_string(),
        is_main,
        detached,
    }
}

/// 解析 `git worktree list --porcelain`：每组以空行分隔，组内 `worktree <path>` 起头，
/// `branch refs/heads/<name>` 给出分支，`detached` 表示分离头指针；`HEAD`/`bare`/`prunable` 忽略。
fn parse_worktree_list(stdout: &str) -> Vec<GitPanelWorktreeEntry> {
    let mut entries: Vec<GitPanelWorktreeEntry> = Vec::new();
    let mut path: Option<String> = None;
    let mut branch = String::new();
    let mut detached = false;

    for raw_line in stdout.lines() {
        let line = raw_line.trim_end();
        if let Some(rest) = line.strip_prefix("worktree ") {
            // 上一组未以空行收尾时兜底落盘（porcelain 正常会有空行）
            if let Some(previous) = path.take() {
                entries.push(git_panel_build_worktree_entry(previous, &branch, detached));
            }
            branch.clear();
            detached = false;
            path = Some(rest.trim().to_string());
            continue;
        }
        if line.is_empty() {
            if let Some(current) = path.take() {
                entries.push(git_panel_build_worktree_entry(current, &branch, detached));
            }
            branch.clear();
            detached = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("branch ") {
            let full = rest.trim();
            branch = full.strip_prefix("refs/heads/").unwrap_or(full).to_string();
            continue;
        }
        if line == "detached" {
            detached = true;
        }
    }
    if let Some(current) = path.take() {
        entries.push(git_panel_build_worktree_entry(current, &branch, detached));
    }
    entries
}

/// 排序：最近打开过的在前（越近越靠前），其余保持「主工作树在前 + 路径序」。
fn git_panel_sort_worktrees(worktrees: &mut [GitPanelWorktreeEntry], history: &[String]) {
    worktrees.sort_by(|a, b| {
        git_panel_history_rank(history, &a.path)
            .cmp(&git_panel_history_rank(history, &b.path))
            .then_with(|| b.is_main.cmp(&a.is_main))
            .then_with(|| a.path.cmp(&b.path))
    });
}

/// 当前仓库的工作树列表：最近打开过的在前，其余主工作树置顶、按路径排序。
/// 仓库根为空、不可访问或 git 执行失败时返回空列表，由面板降级回普通仓库列表。
async fn git_panel_worktrees_inner(
    input: GitPanelWorkspaceRepoInput,
    state: &AppState,
) -> Result<GitPanelWorktreesOutput, String> {
    let repo_root = input.repo_root.trim().to_string();
    if repo_root.is_empty() {
        return Ok(GitPanelWorktreesOutput {
            repo_root,
            worktrees: Vec::new(),
        });
    }
    let repo_root = git_panel_validate_path(&repo_root)?;
    let stdout = match git_executor()
        .run_read(&repo_root, &["worktree", "list", "--porcelain"])
        .await
    {
        Ok(stdout) => stdout,
        Err(err) => {
            runtime_log_warn(format!(
                "[git面板] 工作树列表读取失败，repo_root={repo_root}，error={err}"
            ));
            return Ok(GitPanelWorktreesOutput {
                repo_root,
                worktrees: Vec::new(),
            });
        }
    };
    let mut worktrees = parse_worktree_list(&stdout);
    for item in &mut worktrees {
        item.path = git_panel_strip_verbatim_prefix(&item.path).to_string();
    }
    let history_key = git_panel_normalize_repo_path(input.workspace_path.trim());
    let history_order = git_panel_read_repo_history(state)
        .await
        .get(&history_key)
        .cloned()
        .unwrap_or_default();
    git_panel_sort_worktrees(&mut worktrees, &history_order);
    Ok(GitPanelWorktreesOutput { repo_root, worktrees })
}

#[tauri::command]
async fn git_panel_worktrees(
    input: GitPanelWorkspaceRepoInput,
    state: State<'_, AppState>,
) -> Result<GitPanelWorktreesOutput, String> {
    git_panel_worktrees_inner(input, &state).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorktreeAddInput {
    /// 仓库根（主工作树路径）
    repo_root: String,
    /// 新工作树的目标目录
    path: String,
    /// 新分支名（checkout_existing=false 时使用）
    #[serde(default)]
    branch: String,
    /// 基分支/基点（checkout_existing=false 时作为 -b 的起点；true 时作为检出目标）
    base_branch: String,
    /// true：直接检出已有分支（分支名=base_branch，不再派生新分支）
    #[serde(default)]
    checkout_existing: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitPanelWorktreeAddOutput {
    path: String,
    branch: String,
}

/// 新建工作树：子弹窗确认后调用。
/// checkout_existing=false → `git worktree add -b <branch> <path> <base>`（从基分支派生新分支）
/// checkout_existing=true  → `git worktree add <path> <base>`（直接检出已有分支，
/// 该分支不能被其他工作树占用，前端已禁用，后端再兜底校验一次）。
async fn git_panel_worktree_add_inner(
    input: GitPanelWorktreeAddInput,
) -> Result<GitPanelWorktreeAddOutput, String> {
    let repo_root = git_panel_validate_path(&input.repo_root)?;
    let path = git_panel_validate_path(&input.path)?;
    let base = git_panel_validate_reference(&input.base_branch)?;
    if input.checkout_existing {
        // 兜底：目标分支不能被其他工作树占用
        let stdout = git_executor()
            .run_read(&repo_root, &["worktree", "list", "--porcelain"])
            .await
            .map_err(|err| format!("读取工作树列表失败：{err}"))?;
        let occupied = parse_worktree_list(&stdout)
            .iter()
            .any(|entry| entry.branch.trim() == base.trim());
        if occupied {
            return Err(format!("分支 {base} 已被其他工作树检出，不能直接检出"));
        }
        let output = git_executor()
            .run_write(&repo_root, &["worktree", "add", &path, &base])
            .await
            .map_err(|err| format!("git worktree add 执行失败：{err}"))?;
        if output.exit_code != 0 {
            return Err(format!(
                "git worktree add 失败：{} {}",
                output.stderr.trim(),
                output.stdout.trim()
            ));
        }
        return Ok(GitPanelWorktreeAddOutput { path, branch: base });
    }
    let branch = git_panel_validate_reference(&input.branch)?;
    let output = git_executor()
        .run_write(&repo_root, &["worktree", "add", "-b", &branch, &path, &base])
        .await
        .map_err(|err| format!("git worktree add 执行失败：{err}"))?;
    if output.exit_code != 0 {
        return Err(format!(
            "git worktree add 失败：{} {}",
            output.stderr.trim(),
            output.stdout.trim()
        ));
    }
    Ok(GitPanelWorktreeAddOutput { path, branch })
}

#[tauri::command]
async fn git_panel_worktree_add(
    input: GitPanelWorktreeAddInput,
) -> Result<GitPanelWorktreeAddOutput, String> {
    git_panel_worktree_add_inner(input).await
}

/// 记录一次打开：前端切换仓库/工作树后调用，供仓库栏按「最近打开」排序。
#[tauri::command]
async fn git_panel_remember_repo(
    input: GitPanelWorkspaceRepoInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repo_root = git_panel_validate_path(&input.repo_root)?;
    git_panel_history_remember(&state, &workspace_path, &repo_root).await;
    Ok(())
}

/// 仓库列表：扫描 + 当前仓库根 extra + 历史合并，按路径排序。
/// 供 tauri 命令与 Web dispatcher 共用（channel + Web 双接口）。
async fn git_panel_repos_inner(
    input: GitPanelWorkspaceInput,
    refresh: bool,
    state: &AppState,
) -> Result<GitPanelReposOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let repos = git_panel_collect_repos(&workspace_path, refresh, state).await?;
    Ok(GitPanelReposOutput { repos })
}

#[tauri::command]
async fn git_panel_repos(
    input: GitPanelWorkspaceInput,
    refresh: bool,
    state: State<'_, AppState>,
) -> Result<GitPanelReposOutput, String> {
    git_panel_repos_inner(input, refresh, &state).await
}

/// 一次探查：git 可用性 + 向上探测当前仓库根 + 向下扫描仓库列表 + 推荐默认仓库。
/// 替代前端 loadDetect + loadRepos 两次并发调用，从根上消除「先刷出子仓库再被
/// 向上探测失败覆盖」的时序竞态。
/// 供 tauri 命令与 Web dispatcher 共用（channel + Web 双接口）。
async fn git_panel_discover_inner(
    input: GitPanelWorkspaceInput,
    refresh: bool,
    state: &AppState,
) -> Result<GitPanelDiscoverOutput, String> {
    let workspace_path = git_panel_validate_path(&input.workspace_path)?;
    let version = git_executor().run_read(&workspace_path, &["--version"]).await;
    if version.is_err() {
        return Ok(GitPanelDiscoverOutput {
            git_available: false,
            current_repo_root: None,
            repos: Vec::new(),
            default_repo_root: None,
            checked: false,
            error: Some("无法运行 git 命令".to_string()),
        });
    }
    // 向上探测：当前目录是否在仓库内（失败仅表示不在仓库内，不是错误）
    let current_repo_root = git_panel_resolve_root(&workspace_path).await.ok();
    let repos = git_panel_collect_repos(&workspace_path, refresh, state).await?;
    let default_repo_root =
        git_panel_default_repo_root(state, &workspace_path, &current_repo_root, &repos).await;
    Ok(GitPanelDiscoverOutput {
        git_available: true,
        current_repo_root,
        repos,
        default_repo_root,
        checked: true,
        error: None,
    })
}

#[tauri::command]
async fn git_panel_discover(
    input: GitPanelWorkspaceInput,
    refresh: bool,
    state: State<'_, AppState>,
) -> Result<GitPanelDiscoverOutput, String> {
    git_panel_discover_inner(input, refresh, &state).await
}

#[cfg(test)]
mod git_panel_rebase_tests {
    use super::*;

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("pai-git-rebase-{tag}-{nanos}"))
    }

    #[test]
    fn detects_rebase_from_merge_or_apply_dir() {
        let dir = unique_dir("detect");
        std::fs::create_dir_all(&dir).unwrap();

        // 两个标记目录都不存在：不是变基
        assert!(!git_panel_git_dir_is_rebasing(&dir));

        // rebase-merge：普通与交互式变基
        std::fs::create_dir_all(dir.join("rebase-merge")).unwrap();
        assert!(git_panel_git_dir_is_rebasing(&dir));
        std::fs::remove_dir_all(dir.join("rebase-merge")).unwrap();

        // rebase-apply：apply 后端变基
        std::fs::create_dir_all(dir.join("rebase-apply")).unwrap();
        assert!(git_panel_git_dir_is_rebasing(&dir));
        std::fs::remove_dir_all(dir.join("rebase-apply")).unwrap();

        // 同名普通文件不算：git 只会创建目录
        std::fs::write(dir.join("rebase-apply"), b"").unwrap();
        assert!(!git_panel_git_dir_is_rebasing(&dir));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod git_panel_branch_refs_tests {
    use super::*;

    /// for-each-ref 每行形如：`HEAD标记 \u{1f} 引用全名 \u{1f} 日期 \u{1f} 符号引用目标 \u{1f} 上游短名 \u{1f} 上游跟踪`
    fn line(head: &str, refname: &str, date: &str, symref: &str, upstream: &str, track: &str) -> String {
        format!("{head}\u{1f}{refname}\u{1f}{date}\u{1f}{symref}\u{1f}{upstream}\u{1f}{track}")
    }

    #[test]
    fn parses_local_and_remote_with_current_flag() {
        let raw = [
            line("*", "refs/heads/main", "2026-09-19T10:00:00+08:00", "", "origin/main", "[ahead 3]"),
            line(" ", "refs/heads/feature/login", "2026-09-10T10:00:00+08:00", "", "", ""),
            line(" ", "refs/remotes/origin/main", "2026-09-18T10:00:00+08:00", "", "", ""),
        ]
        .join("\n");

        let entries = parse_branch_refs(&raw);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].name, "main");
        assert!(entries[0].is_current);
        assert!(!entries[0].is_remote);
        assert_eq!(entries[0].committer_date, "2026-09-19T10:00:00+08:00");
        assert_eq!(entries[0].upstream, "origin/main");
        assert_eq!(entries[0].ahead, 3);
        assert_eq!(entries[0].behind, 0);
        assert!(!entries[0].upstream_missing);

        assert_eq!(entries[1].name, "feature/login");
        assert!(!entries[1].is_current);
        assert!(!entries[1].is_remote);
        assert_eq!(entries[1].upstream, "");

        // 远程保留 origin/ 前缀，不误判为当前分支
        assert_eq!(entries[2].name, "origin/main");
        assert!(entries[2].is_remote);
        assert!(!entries[2].is_current);
    }

    #[test]
    fn skips_symbolic_refs_like_origin_head() {
        let raw = [
            line(" ", "refs/remotes/origin/HEAD", "2026-09-18T10:00:00+08:00", "refs/remotes/origin/main", "", ""),
            line(" ", "refs/remotes/origin/main", "2026-09-18T10:00:00+08:00", "", "", ""),
        ]
        .join("\n");

        let entries = parse_branch_refs(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "origin/main");
    }

    #[test]
    fn skips_blank_lines_and_unrelated_refs() {
        let raw = [
            String::new(),
            line(" ", "refs/tags/v1", "2026-01-01T10:00:00+08:00", "", "", ""),
            line(" ", "refs/heads/dev", "2026-02-01T10:00:00+08:00", "", "", ""),
        ]
        .join("\n");

        let entries = parse_branch_refs(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "dev");
    }

    #[test]
    fn keeps_branch_with_empty_date() {
        // 空仓库/异常情况下 committerdate 可能为空，仍应保留分支、日期留空由前端排到最后
        let raw = line(" ", "refs/heads/empty", "", "", "", "");
        let entries = parse_branch_refs(&raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "empty");
        assert_eq!(entries[0].committer_date, "");
    }

    #[test]
    fn parses_upstream_track_states() {
        let raw = [
            line(" ", "refs/heads/a", "2026-09-01T00:00:00+08:00", "", "origin/a", "[ahead 3]"),
            line(" ", "refs/heads/b", "2026-09-01T00:00:00+08:00", "", "origin/b", "[behind 2]"),
            line(" ", "refs/heads/c", "2026-09-01T00:00:00+08:00", "", "origin/c", "[ahead 1, behind 1]"),
            line(" ", "refs/heads/d", "2026-09-01T00:00:00+08:00", "", "origin/d", "[gone]"),
            line(" ", "refs/heads/e", "2026-09-01T00:00:00+08:00", "", "origin/e", ""),
            line(" ", "refs/heads/f", "2026-09-01T00:00:00+08:00", "", "", ""),
        ]
        .join("\n");

        let entries = parse_branch_refs(&raw);
        assert_eq!(entries.len(), 6);

        assert_eq!((entries[0].ahead, entries[0].behind, entries[0].upstream_missing), (3, 0, false));
        assert_eq!((entries[1].ahead, entries[1].behind, entries[1].upstream_missing), (0, 2, false));
        assert_eq!((entries[2].ahead, entries[2].behind, entries[2].upstream_missing), (1, 1, false));
        // [gone]：上游记录还在但远程分支已删
        assert_eq!(entries[3].upstream, "origin/d");
        assert_eq!((entries[3].ahead, entries[3].behind, entries[3].upstream_missing), (0, 0, true));
        // 上游存在且完全同步：track 为空
        assert_eq!((entries[4].ahead, entries[4].behind, entries[4].upstream_missing), (0, 0, false));
        assert_eq!(entries[4].upstream, "origin/e");
        // 没配上游：upstream 与 track 都为空
        assert_eq!(entries[5].upstream, "");
    }
}

#[cfg(test)]
mod git_panel_repos_tests {
    use super::*;

    /// 构造临时目录树（用完由 guard 清理）：
    /// - root/.git                     → root 仓库（深度 0）
    /// - root/sub1/.git                → sub1 仓库（深度 1）
    /// - root/sub1/inner/.git          → inner 仓库（深度 2）
    /// - root/node_modules/dep/.git    → 忽略名单内，不命中
    /// - root/sub2/a/b/.git            → 深度 3 仓库（边界内）
    /// - root/sub2/a/b/deeper/.git     → 深度 4，超限不命中
    /// - root/wt/.git（文件）           → worktree 模拟，命中
    fn make_fixture() -> (TempDirGuard, String) {
        let base = std::env::temp_dir().join(format!(
            "git-panel-repos-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        for rel in [
            ".git",
            "sub1/.git",
            "sub1/inner/.git",
            "node_modules/dep/.git",
            "sub2/a/b/.git",
            "sub2/a/b/deeper/.git",
        ] {
            std::fs::create_dir_all(base.join(rel)).expect("创建 fixture 失败");
        }
        std::fs::create_dir_all(base.join("wt")).expect("创建 fixture 失败");
        std::fs::write(base.join("wt/.git"), "gitdir: ../.git/worktrees/wt")
            .expect("创建 worktree 模拟失败");
        let root = base.to_string_lossy().to_string();
        (TempDirGuard(base), root)
    }

    struct TempDirGuard(std::path::PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scans_nested_repos_within_depth_and_skips_ignored() {
        let (_guard, root) = make_fixture();
        let repos = git_panel_scan_repos_inner(&root, GIT_REPO_SCAN_MAX_DEPTH);
        let paths: Vec<&str> = repos.iter().map(|repo| repo.path.as_str()).collect();
        let sub1 = Path::new(&root).join("sub1").to_string_lossy().into_owned();
        let inner = Path::new(&root).join("sub1").join("inner").to_string_lossy().into_owned();
        let sub3 = Path::new(&root).join("sub2").join("a").join("b").to_string_lossy().into_owned();
        let ignored = Path::new(&root).join("node_modules").join("dep").to_string_lossy().into_owned();
        let deep = Path::new(&root).join("sub2").join("a").join("b").join("deeper").to_string_lossy().into_owned();
        let wt = Path::new(&root).join("wt").to_string_lossy().into_owned();
        assert!(paths.contains(&root.as_str()), "深度 0 仓库应命中，实际 {paths:?}");
        assert!(paths.contains(&sub1.as_str()), "深度 1 仓库应命中");
        assert!(paths.contains(&inner.as_str()), "深度 2 仓库应命中");
        assert!(paths.contains(&sub3.as_str()), "深度 3 仓库应命中（边界内）");
        assert!(!paths.contains(&ignored.as_str()), "忽略名单目录内的仓库不应命中");
        assert!(!paths.contains(&deep.as_str()), "超过深度上限的仓库不应命中");
        assert!(paths.contains(&wt.as_str()), ".git 文件（worktree）也应命中");
    }

    #[test]
    fn repo_name_takes_last_segment() {
        assert_eq!(git_panel_repo_name("E:\\github\\easy_call_ai"), "easy_call_ai");
        assert_eq!(git_panel_repo_name("/home/user/project"), "project");
    }

    #[test]
    fn default_depth_one_only_finds_direct_children() {
        let (_guard, root) = make_fixture();
        let repos = git_panel_scan_repos_inner(&root, GIT_REPO_SCAN_DEFAULT_DEPTH);
        let paths: Vec<&str> = repos.iter().map(|repo| repo.path.as_str()).collect();
        let sub1 = Path::new(&root).join("sub1").to_string_lossy().into_owned();
        let inner = Path::new(&root).join("sub1").join("inner").to_string_lossy().into_owned();
        assert!(paths.contains(&root.as_str()), "深度 0 仓库应命中");
        assert!(paths.contains(&sub1.as_str()), "深度 1 仓库应命中");
        assert!(!paths.contains(&inner.as_str()), "默认深度 1 时深度 2 仓库不应命中");
    }

    #[tokio::test]
    async fn default_repo_root_prefers_current_then_history_then_single() {
        let base = std::env::temp_dir().join(format!(
            "git-panel-discover-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        for rel in [".git", "sub1/.git", "sub2/.git", "history_only/.git"] {
            std::fs::create_dir_all(base.join(rel)).expect("创建 fixture 失败");
        }
        let _guard = TempDirGuard(base.clone());
        let root = base.to_string_lossy().into_owned();
        let sub1 = base.join("sub1").to_string_lossy().into_owned();
        let sub2 = base.join("sub2").to_string_lossy().into_owned();
        let history_only = base.join("history_only").to_string_lossy().into_owned();

        let state = AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: base.join("app_config.toml"),
            data_path: base.join("config_mark"),
            llm_workspace_path: base.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let repos = vec![
            GitPanelRepoEntry { path: sub1.clone(), name: "sub1".to_string() },
            GitPanelRepoEntry { path: sub2.clone(), name: "sub2".to_string() },
        ];
        // 1. 向上命中 → 当前仓库优先（即使有历史）
        let current = Some(sub1.clone());
        assert_eq!(
            git_panel_default_repo_root(&state, &root, &current, &repos).await,
            Some(sub1.clone())
        );
        // 2. 无当前仓库，历史最近命中（含不在扫描列表里的目录）→ 历史优先
        let mut history = std::collections::HashMap::new();
        let history_key = git_panel_normalize_repo_path(&root);
        history.insert(history_key, vec![history_only.clone(), sub2.clone()]);
        git_panel_write_repo_history(&state, &history).await;
        assert_eq!(
            git_panel_default_repo_root(&state, &root, &None, &repos).await,
            Some(history_only.clone())
        );
        // 3. 无当前仓库、无历史，仅 1 个仓库 → 它自己
        git_panel_write_repo_history(&state, &std::collections::HashMap::new()).await;
        let single = vec![GitPanelRepoEntry { path: sub2.clone(), name: "sub2".to_string() }];
        assert_eq!(git_panel_default_repo_root(&state, &root, &None, &single).await, Some(sub2));
        // 4. 无当前仓库、无历史、多仓库 → None（不瞎猜）
        assert_eq!(git_panel_default_repo_root(&state, &root, &None, &repos).await, None);
    }

    #[test]
    fn history_push_dedupes_reorders_and_caps() {
        let mut history: HashMap<String, Vec<String>> = HashMap::new();
        for i in 0..GIT_REPO_HISTORY_LIMIT {
            assert!(git_panel_history_push(
                &mut history,
                "E:/github/easy_call_ai",
                &format!("E:/github/easy_call_ai/repo/{i:02}")
            ));
        }
        let group = history.get("e:/github/easy_call_ai").expect("分组键应存在");
        assert_eq!(group.len(), GIT_REPO_HISTORY_LIMIT);
        // 已存在（不在最前）：移到最前，不新增
        assert!(git_panel_history_push(
            &mut history,
            "E:/github/easy_call_ai",
            "E:/github/easy_call_ai/repo/30"
        ));
        let group = history.get("e:/github/easy_call_ai").unwrap();
        assert_eq!(group.len(), GIT_REPO_HISTORY_LIMIT);
        assert_eq!(group[0], "E:/github/easy_call_ai/repo/30");
        // 已在最前：无变化
        assert!(!git_panel_history_push(
            &mut history,
            "E:/github/easy_call_ai",
            "E:/github/easy_call_ai/repo/30"
        ));
        // 超限：新仓库插入头部，最旧的被丢弃
        assert!(git_panel_history_push(
            &mut history,
            "E:/github/easy_call_ai",
            "E:/github/easy_call_ai/repo/new"
        ));
        let group = history.get("e:/github/easy_call_ai").unwrap();
        assert_eq!(group.len(), GIT_REPO_HISTORY_LIMIT);
        assert_eq!(group[0], "E:/github/easy_call_ai/repo/new");
        assert!(!group.contains(&"E:/github/easy_call_ai/repo/00".to_string()), "最旧的应被丢弃");
    }

    #[test]
    fn history_groups_are_isolated_by_workspace() {
        let mut history: HashMap<String, Vec<String>> = HashMap::new();
        git_panel_history_push(&mut history, "E:/github/easy_call_ai", "E:/github/easy_call_ai/sub/repo");
        git_panel_history_push(&mut history, "E:\\github\\other_project", "E:\\github\\other_project\\deep\\repo");

        let first = history.get("e:/github/easy_call_ai").expect("第一个工作区分组应存在");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0], "E:/github/easy_call_ai/sub/repo");

        let second = history.get("e:/github/other_project").expect("第二个工作区分组应存在");
        assert_eq!(second.len(), 1);
        assert_eq!(second[0], "E:\\github\\other_project\\deep\\repo");

        // 分隔符/大小写不同的同一工作区应命中同一分组
        git_panel_history_push(&mut history, "E:/GITHUB/EASY_CALL_AI", "E:/github/easy_call_ai/another");
        let first = history.get("e:/github/easy_call_ai").unwrap();
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn repo_path_eq_ignores_case_separators_and_verbatim_prefix() {
        #[cfg(target_os = "windows")]
        {
            assert!(git_panel_repo_path_eq("E:\\GitHub\\Demo", "e:/github/demo"));
            assert!(git_panel_repo_path_eq("\\\\?\\E:\\github\\story", "E:/github/story"));
            assert!(git_panel_repo_path_eq(
                "\\\\?\\E:\\github\\story\\AnimeGameData",
                "e:/github/story/AnimeGameData"
            ));
        }
        #[cfg(not(target_os = "windows"))]
        assert!(!git_panel_repo_path_eq("/A/B", "/a/b"));
    }

    #[test]
    fn status_entries_truncated_at_max_with_flag() {
        fn entry(path: &str) -> GitPanelStatusEntry {
            GitPanelStatusEntry {
                path: path.to_string(),
                staged_status: " ".to_string(),
                unstaged_status: "M".to_string(),
            }
        }
        // 未超过上限：不截断，保留全部
        let small: Vec<GitPanelStatusEntry> = (0..GIT_PANEL_STATUS_MAX_ENTRIES)
            .map(|i| entry(&format!("file-{i}.txt")))
            .collect();
        let (kept, truncated) = git_panel_truncate_status_entries(small.clone());
        assert_eq!(kept.len(), small.len());
        assert!(!truncated);

        // 超过上限：截断到 1000 且标记 truncated
        let large: Vec<GitPanelStatusEntry> = (0..GIT_PANEL_STATUS_MAX_ENTRIES + 1)
            .map(|i| entry(&format!("file-{i}.txt")))
            .collect();
        let (kept, truncated) = git_panel_truncate_status_entries(large);
        assert_eq!(kept.len(), GIT_PANEL_STATUS_MAX_ENTRIES);
        assert!(truncated);

        // 空列表：不截断
        let (kept, truncated) = git_panel_truncate_status_entries(Vec::new());
        assert!(kept.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn status_group_totals_match_frontend_filter_rules() {
        fn entry(path: &str, staged: &str, unstaged: &str) -> GitPanelStatusEntry {
            GitPanelStatusEntry {
                path: path.to_string(),
                staged_status: staged.to_string(),
                unstaged_status: unstaged.to_string(),
            }
        }
        // ?? 未跟踪：只进更改组
        let untracked = entry("new.txt", "?", "?");
        // M 在 X 列：只进暂存组
        let staged_only = entry("a.txt", "M", " ");
        // M 在 Y 列：只进更改组
        let unstaged_only = entry("b.txt", " ", "M");
        // 两侧都有 M：同时进两组
        let both = entry("c.txt", "M", "M");
        let entries = vec![untracked, staged_only, unstaged_only, both];

        let staged_total = entries.iter().filter(|e| git_panel_entry_is_staged(e)).count();
        let unstaged_total = entries.iter().filter(|e| git_panel_entry_is_unstaged(e)).count();
        assert_eq!(staged_total, 2, "暂存组应含 staged_only 与 both");
        assert_eq!(unstaged_total, 3, "更改组应含 untracked、unstaged_only 与 both");
    }
}

#[cfg(test)]
mod git_panel_worktrees_tests {
    use super::*;

    /// 典型 porcelain 输出：主工作树 + 命名工作树 + 分离头指针工作树。
    const SAMPLE: &str = "worktree E:/repo/main\nHEAD 1111111111111111111111111111111111111111\nbranch refs/heads/main\n\nworktree E:/repo/.pai/.worktree/session-a\nHEAD 2222222222222222222222222222222222222222\nbranch refs/heads/feat/relay-access\n\nworktree E:/repo/.pai/.worktree/detached-wt\nHEAD 3333333333333333333333333333333333333333\ndetached\n\n";

    fn temp_base(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "git-panel-worktree-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn parses_worktree_entries_with_branch_and_detached() {
        let entries = parse_worktree_list(SAMPLE);
        assert_eq!(entries.len(), 3, "三个工作树都应收录");
        assert_eq!(entries[0].path, "E:/repo/main");
        assert_eq!(entries[0].name, "main");
        assert_eq!(entries[0].branch, "main", "分支名应剥掉 refs/heads/ 前缀");
        assert_eq!(
            entries[1].branch, "feat/relay-access",
            "带斜杠的分支名应完整保留"
        );
        assert!(entries[2].detached, "detached 组应标记分离头指针");
        assert_eq!(entries[2].branch, "", "分离头指针没有分支名");
    }

    #[test]
    fn detects_main_worktree_by_git_directory() {
        let base = temp_base("main");
        let main = base.join("main");
        let linked = base.join("linked");
        std::fs::create_dir_all(main.join(".git")).expect("创建主工作树 .git 目录失败");
        std::fs::create_dir_all(&linked).expect("创建 linked 工作树目录失败");
        std::fs::write(linked.join(".git"), "gitdir: ../main/.git/worktrees/linked")
            .expect("写入 linked 工作树 .git 文件失败");

        assert!(
            git_panel_is_main_worktree(&main.to_string_lossy()),
            "主工作树的 .git 是目录"
        );
        assert!(
            !git_panel_is_main_worktree(&linked.to_string_lossy()),
            "linked 工作树的 .git 是文件"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sorts_recently_opened_first_then_main_then_path() {
        fn entry(path: &str, is_main: bool) -> GitPanelWorktreeEntry {
            GitPanelWorktreeEntry {
                path: path.to_string(),
                name: git_panel_repo_name(path),
                branch: String::new(),
                is_main,
                detached: false,
            }
        }
        fn paths(worktrees: &[GitPanelWorktreeEntry]) -> Vec<&str> {
            worktrees.iter().map(|item| item.path.as_str()).collect()
        }
        let mut worktrees = vec![
            entry("E:/repo/.pai/.worktree/b", false),
            entry("E:/repo/main", true),
            entry("E:/repo/.pai/.worktree/a", false),
        ];
        // 没有历史：主工作树置顶，其余按路径排序
        git_panel_sort_worktrees(&mut worktrees, &[]);
        assert_eq!(
            paths(&worktrees),
            vec![
                "E:/repo/main",
                "E:/repo/.pai/.worktree/a",
                "E:/repo/.pai/.worktree/b"
            ],
            "无历史时主工作树置顶，其余按路径"
        );

        // 有历史：最近打开过的排在前面，历史内部按「越近越靠前」
        let history = vec![
            "E:/repo/.pai/.worktree/b".to_string(),
            "E:/repo/.pai/.worktree/a".to_string(),
        ];
        git_panel_sort_worktrees(&mut worktrees, &history);
        assert_eq!(
            paths(&worktrees),
            vec![
                "E:/repo/.pai/.worktree/b",
                "E:/repo/.pai/.worktree/a",
                "E:/repo/main"
            ],
            "历史顺序应压过主工作树置顶与路径序"
        );
    }

    #[test]
    fn history_rank_matches_paths_case_and_slash_insensitively() {
        let history = vec!["E:\\repo\\sub".to_string()];
        assert_eq!(git_panel_history_rank(&history, "e:/repo/sub"), 0);
        assert_eq!(
            git_panel_history_rank(&history, "E:/repo/other"),
            usize::MAX,
            "未打开过的路径排在所有命中之后"
        );
    }
}
