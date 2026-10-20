// ==================== Git 命令执行器 ====================
// 统一 git CLI 执行层：读命令 TTL 缓存 + singleflight 合并，写命令全局互斥 + 写后失效。
// 通过 GitCommandRunner 注入真实/测试实现，缓存与并发行为可被单元测试覆盖。
//
// 设计要点：
// - run_read：同一 (workdir, args) 执行中时，后来的请求等待同一个结果，不重复执行；
//   完成后按 TTL 缓存，未过期直接返回。
// - run_write：本地写命令（add/commit/checkout/stash/reset/discard/worktree）经
//   local_write_lock 串行；撞上外部 Git 进程（如 VS Code）的 index.lock / refs 锁时
//   指数退避重试，成功后 invalidate 该仓库读缓存。
// - run_network：纯网络命令（fetch/push）只经 network_lock，不阻塞本地写。
// - run_pull：pull 会改 worktree 与 index，同时持 network_lock 与 local_write_lock。
// - 失败结果短 TTL（500ms），避免多端反复打同一失败命令叠加风暴。
//
// 注意：本文件由 include! 并入 main.rs 根作用域，不得顶层 use 与既有模块重名的
// 类型（HashMap/Arc/Mutex/Duration 等），一律使用完整路径。

// ---------- 输出类型 ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPanelRunOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ---------- 锁冲突判定与重试参数 ----------

/// 撞锁后的额外重试次数上限（首次执行之外最多再试 3 次）。
const GIT_LOCK_MAX_RETRIES: u32 = 3;
/// 退避基准间隔，按 2^n 递增：150ms → 300ms → 600ms（累计约 1.05s）。
const GIT_LOCK_BASE_DELAY_MS: u64 = 150;

/// 判定一次 git 失败是否属于「锁被外部 Git 进程占用」。
/// 外部进程（如 VS Code 的 Git 扩展）短暂持有 `.git/index.lock` 或 refs 锁时，
/// git 以非零退出码返回下列文案之一；等待锁释放后重试即可成功，不该直接抛给用户。
fn git_lock_conflict(output: &GitPanelRunOutput) -> bool {
    if output.exit_code == 0 {
        return false;
    }
    let text = format!("{}\n{}", output.stdout, output.stderr);
    text.contains("index.lock")
        || text.contains("Another git process seems to be running")
        || text.contains("cannot lock ref")
        || (text.contains("Unable to create") && text.contains("File exists"))
}

// ---------- Runner 抽象（测试注入点） ----------

pub trait GitCommandRunner: Send + Sync {
    fn run(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GitPanelRunOutput, String>> + Send + '_>,
    >;
}

/// 生产实现：真实 git 子进程。
pub struct RealGitRunner;

impl GitCommandRunner for RealGitRunner {
    fn run(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<GitPanelRunOutput, String>> + Send + '_>,
    > {
        let workdir = workdir.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Box::pin(async move {
            let mut command = tokio::process::Command::new("git");
            command
                .current_dir(&workdir)
                .args(&args)
                .env("GIT_TERMINAL_PROMPT", "0")
                // future 被取消（超时等）时终止子进程，避免残留。
                .kill_on_drop(true);
            #[cfg(target_os = "windows")]
            {
                // 避免 Git 进程在 GUI 应用中创建短暂可见的控制台窗口。
                command.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            let output = command
                .output()
                .await
                .map_err(|err| format!("无法运行 git：{err}"))?;
            Ok(GitPanelRunOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
            })
        })
    }
}

// ---------- 读缓存状态 ----------

/// 缓存 key：仓库目录 + 完整命令参数（参数安全由调用方保证，均为硬编码或校验过的值）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CmdKey {
    workdir: String,
    args: Vec<String>,
}

enum CommandSlot {
    /// 执行中：等待者 subscribe 到 watch 上，执行者完成时 send 结果并唤醒。
    /// id 是该执行者的唯一标识，用于 Drop/重试清理时校验槽位归属，避免误删新执行者。
    Inflight {
        tx: tokio::sync::watch::Sender<Option<Result<String, String>>>,
        id: u64,
    },
    /// 已完成：结果 + 写入时间戳 + 写入时的仓库世代号。
    Done {
        at: std::time::Instant,
        /// 写入时的仓库世代号；世代号变化（写命令完成）后该缓存视为失效。
        version: u64,
        result: Result<String, String>,
    },
}

/// 缓存状态：命令槽 + 各仓库世代号（写命令完成后 +1，用于拦截旧读结果回写）。
struct CacheState {
    slots: std::collections::HashMap<CmdKey, CommandSlot>,
    versions: std::collections::HashMap<String, u64>,
    /// 执行者 id 分配器，保证每个 Inflight 槽位有唯一归属标识。
    next_executor_id: u64,
}

/// 执行者守卫：正常完成时槽位已替换为 Done，Drop 不动；被取消/panic 时槽位仍是
/// 自己的 Inflight（按 id 校验归属），Drop 时删除，避免悬挂槽位让后续请求永久失败。
/// 只删除 id 匹配的槽位：期间若有新执行者（如 invalidate 清槽后被重建）占用同一
/// key，不会误删。
struct ExecutorGuard<'a> {
    state: &'a std::sync::Mutex<CacheState>,
    key: CmdKey,
    executor_id: u64,
}

impl Drop for ExecutorGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(CommandSlot::Inflight { id, .. }) = state.slots.get(&self.key) {
                if *id == self.executor_id {
                    state.slots.remove(&self.key);
                }
            }
        }
    }
}

// ---------- 执行器 ----------

pub struct GitExecutor {
    runner: std::sync::Arc<dyn GitCommandRunner>,
    state: std::sync::Mutex<CacheState>,
    ttl: std::time::Duration,
    fail_ttl: std::time::Duration,
    network_timeout: std::time::Duration,
    /// 本地写锁：add/commit/checkout/stash/reset/discard/worktree 等会改 index 或
    /// worktree 的命令互斥，防止自身并发制造 index.lock 冲突。
    local_write_lock: tokio::sync::Mutex<()>,
    /// 网络锁：fetch/push/pull 互斥。与本地写锁分离，让慢速网络操作不再阻塞本地写。
    network_lock: tokio::sync::Mutex<()>,
}

impl GitExecutor {
    pub fn new(
        runner: std::sync::Arc<dyn GitCommandRunner>,
        ttl: std::time::Duration,
        fail_ttl: std::time::Duration,
    ) -> Self {
        Self {
            runner,
            state: std::sync::Mutex::new(CacheState {
                slots: std::collections::HashMap::new(),
                versions: std::collections::HashMap::new(),
                next_executor_id: 0,
            }),
            ttl,
            fail_ttl,
            network_timeout: std::time::Duration::from_secs(60),
            local_write_lock: tokio::sync::Mutex::new(()),
            network_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// 读命令：TTL 缓存 + singleflight。成功（退出码 0）返回 stdout，失败返回可读错误。
    pub async fn run_read(&self, workdir: &str, args: &[&str]) -> Result<String, String> {
        let key = CmdKey {
            workdir: workdir.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        };

        // 等待执行者被取消后允许重新抢占的次数上限，防止异常循环。
        const MAX_WAIT_RETRIES: u32 = 3;
        let mut wait_retries = 0u32;

        loop {
            // 快速路径：命中未过期且世代号未变的 Done 直接返回。
            {
                let state = self.state.lock().unwrap();
                let version = state.versions.get(workdir).copied().unwrap_or(0);
                if let Some(CommandSlot::Done { at, version: v, result }) = state.slots.get(&key) {
                    if *v == version && at.elapsed() < self.ttl_for(result) {
                        return result.clone();
                    }
                }
            }

            // 慢路径：注册为执行者，或等待已有执行者。
            let (tx, _rx) = tokio::sync::watch::channel(None);
            let mut existing_rx: Option<tokio::sync::watch::Receiver<Option<Result<String, String>>>> = None;
            // 等待者所属执行者 id：重试清理时只删这个执行者的悬挂槽位。
            let mut waited_id: Option<u64> = None;
            let mut my_id: Option<u64> = None;
            let started_version;
            {
                let mut state = self.state.lock().unwrap();
                started_version = state.versions.get(workdir).copied().unwrap_or(0);
                // 提前分配执行者 id（等待分支也消耗一个编号，无副作用，只用于归属校验）。
                let id = state.next_executor_id;
                state.next_executor_id += 1;
                match state.slots.entry(key.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut e) => match e.get() {
                        CommandSlot::Done { at, version, result } => {
                            if *version == started_version && at.elapsed() < self.ttl_for(result) {
                                return result.clone();
                            }
                            // 过期或世代号已变：替换为 Inflight，自己执行。
                            e.insert(CommandSlot::Inflight { tx: tx.clone(), id });
                            my_id = Some(id);
                        }
                        CommandSlot::Inflight { tx: existing, id: existing_id } => {
                            // 已有执行者：记录其通知通道与其 id，锁外等待结果。
                            existing_rx = Some(existing.subscribe());
                            waited_id = Some(*existing_id);
                        }
                    },
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(CommandSlot::Inflight { tx: tx.clone(), id });
                        my_id = Some(id);
                    }
                }
            }
            // 锁已释放：等待已有执行者完成（singleflight 合并）。
            if let Some(mut rx) = existing_rx {
                match Self::wait_inflight(&mut rx).await {
                    Ok(result) => return Ok(result),
                    Err(_) => {
                        // 执行者被取消（channel closed）：仅当槽位仍是该执行者
                        // （id 匹配）时删除悬挂槽位；若已被新执行者重建则不动，
                        // 重新进入循环继续等待/抢占。
                        wait_retries += 1;
                        if wait_retries > MAX_WAIT_RETRIES {
                            return Err("git 命令执行被取消，多次抢占失败".to_string());
                        }
                        if let Some(waited) = waited_id {
                            let mut state = self.state.lock().unwrap();
                            if let Some(CommandSlot::Inflight { id, .. }) = state.slots.get(&key) {
                                if *id == waited {
                                    state.slots.remove(&key);
                                }
                            }
                        }
                        continue;
                    }
                }
            }

            // 自己是执行者：先创建守卫再执行，被取消/panic 时守卫 Drop 清理悬挂槽位。
            let guard = ExecutorGuard {
                state: &self.state,
                key: key.clone(),
                executor_id: my_id.unwrap(),
            };
            let result = self.run_once(workdir, args).await;

            // 唤醒等待者（先发结果，再落 Done；等待者醒来后读到 result 直接返回）。
            let _ = tx.send(Some(result.clone()));
            let finished_version = self
                .state
                .lock()
                .unwrap()
                .versions
                .get(workdir)
                .copied()
                .unwrap_or(0);
            if finished_version == started_version {
                // 世代号未变：正常落缓存。
                self.state
                    .lock()
                    .unwrap()
                    .slots
                    .insert(key, CommandSlot::Done {
                        at: std::time::Instant::now(),
                        version: started_version,
                        result: result.clone(),
                    });
            }
            // 世代号已变（写命令在本读执行期间完成）：不落缓存，避免旧结果覆盖新状态；
            // 守卫 Drop 时槽位仍是 Inflight，会被清理。
            drop(guard);
            return result;
        }
    }

    /// 不走缓存的读命令：每次真实执行一次 git。
    ///
    /// 用于「必须是此刻真实状态」的场景（如发送前分支守卫）。读缓存按调用方传入的
    /// workdir 原样字符串做 key，而写命令失效时用的是 rev-parse 产出的仓库根，
    /// Windows 上两者分隔符不同（配置里是 `E:\repo`，git 返回 `E:/repo`），
    /// 因此同一仓库在两种写法下互不失效。这条路径不读也不写缓存，不受该问题影响。
    pub async fn run_read_uncached(&self, workdir: &str, args: &[&str]) -> Result<String, String> {
        self.run_once(workdir, args).await
    }

    /// 真实执行一次 git，并把非零退出码映射成可读错误。
    async fn run_once(&self, workdir: &str, args: &[&str]) -> Result<String, String> {
        let cmd = args.first().copied().unwrap_or("git");
        self.runner
            .run(workdir, args)
            .await
            .map(|output| {
                if output.exit_code != 0 {
                    let stderr = output.stderr.trim();
                    let stdout = output.stdout.trim();
                    let detail = if !stderr.is_empty() {
                        stderr.to_string()
                    } else if !stdout.is_empty() {
                        stdout.to_string()
                    } else {
                        format!("退出码 {}", output.exit_code)
                    };
                    Err(format!("git {cmd} 失败：{detail}"))
                } else {
                    Ok(output.stdout)
                }
            })
            .and_then(|r| r)
    }

    /// 等待 in-flight 执行者的结果；执行者被取消（tx drop）时返回可读错误。
    async fn wait_inflight(
        rx: &mut tokio::sync::watch::Receiver<Option<Result<String, String>>>,
    ) -> Result<String, String> {
        loop {
            if let Some(result) = rx.borrow_and_update().clone() {
                return result;
            }
            if rx.changed().await.is_err() {
                return Err("git 命令执行被取消".to_string());
            }
        }
    }

    /// 写命令：本地写锁互斥串行，撞外部 Git 进程的锁时指数退避重试；
    /// 成功（退出码 0）后清掉该仓库读缓存。
    pub async fn run_write(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> Result<GitPanelRunOutput, String> {
        let _guard = self.local_write_lock.lock().await;
        let result = self.run_with_lock_retry(workdir, args).await;
        self.invalidate_on_success(workdir, &result);
        result
    }

    /// 纯网络命令（fetch/push）：只经网络锁，不阻塞本地写命令。
    pub async fn run_network(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> Result<GitPanelRunOutput, String> {
        let _guard = self.network_lock.lock().await;
        let wait = self.run_with_lock_retry(workdir, args);
        // 远端无响应时避免前端无限等待；默认 60 秒足够覆盖大仓库传输（测试可调短）。
        match tokio::time::timeout(self.network_timeout, wait).await {
            Ok(result) => {
                self.invalidate_on_success(workdir, &result);
                result
            }
            Err(_) => Err("git 网络操作超时：远端无响应，已终止".to_string()),
        }
    }

    /// pull：既走网络又会改 worktree 与 index，需同时持网络锁与本地写锁。
    /// 锁顺序固定为 network_lock → local_write_lock，避免与其他路径互相等待。
    pub async fn run_pull(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> Result<GitPanelRunOutput, String> {
        let _network_guard = self.network_lock.lock().await;
        let _local_guard = self.local_write_lock.lock().await;
        let wait = self.run_with_lock_retry(workdir, args);
        match tokio::time::timeout(self.network_timeout, wait).await {
            Ok(result) => {
                self.invalidate_on_success(workdir, &result);
                result
            }
            Err(_) => Err("git 网络操作超时：远端无响应，已终止".to_string()),
        }
    }

    /// 执行一条命令，撞锁（外部 Git 进程持锁）时按 2^n 退避重试。
    /// 调用方需已持有相应锁；重试期间仍持锁，避免自身再制造并发。
    async fn run_with_lock_retry(
        &self,
        workdir: &str,
        args: &[&str],
    ) -> Result<GitPanelRunOutput, String> {
        let mut attempt = 0u32;
        loop {
            let result = self.runner.run(workdir, args).await;
            let should_retry = matches!(&result, Ok(output) if git_lock_conflict(output))
                && attempt < GIT_LOCK_MAX_RETRIES;
            if !should_retry {
                return result;
            }
            let delay = GIT_LOCK_BASE_DELAY_MS * 2u64.pow(attempt);
            runtime_log_info(format!(
                "[Git] 命令撞锁，第 {} 次重试（等待 {}ms）：git {}",
                attempt + 1,
                delay,
                args.first().copied().unwrap_or("git")
            ));
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            attempt += 1;
        }
    }

    /// 命令成功（退出码 0）后清掉该仓库读缓存。
    fn invalidate_on_success(&self, workdir: &str, result: &Result<GitPanelRunOutput, String>) {
        if let Ok(output) = result {
            if output.exit_code == 0 {
                self.invalidate(workdir);
            }
        }
    }

    /// 写命令成功后清掉该仓库全部读缓存，并 bump 该仓库世代号：
    /// 世代号变化会让执行中的旧读结果在写回时被拦截，不落缓存。
    pub fn invalidate(&self, repo_root: &str) {
        let mut state = self.state.lock().unwrap();
        state.slots.retain(|key, _| key.workdir != repo_root);
        let version = state.versions.entry(repo_root.to_string()).or_insert(0);
        *version += 1;
    }

    fn ttl_for(&self, result: &Result<String, String>) -> std::time::Duration {
        if result.is_ok() {
            self.ttl
        } else {
            self.fail_ttl
        }
    }
}

// ---------- 全局单例 ----------

static GIT_EXECUTOR: std::sync::OnceLock<GitExecutor> = std::sync::OnceLock::new();

pub fn git_executor() -> &'static GitExecutor {
    GIT_EXECUTOR.get_or_init(|| {
        GitExecutor::new(
            std::sync::Arc::new(RealGitRunner),
            std::time::Duration::from_secs(2),
            std::time::Duration::from_millis(500),
        )
    })
}

// ==================== 测试 ====================

#[cfg(test)]
mod git_executor_tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use futures_util::FutureExt;

    /// 可控 fake runner：记录调用、可设延迟、失败与撞锁次数。
    #[derive(Clone)]
    struct FakeRunner {
        calls: Arc<Mutex<Vec<(String, Vec<String>)>>>,
        delay: Duration,
        /// 仅当 args 完全匹配此列表时才应用 delay；None 表示所有命令都延迟。
        delay_args: Option<Vec<String>>,
        fail: bool,
        /// 前 N 次调用返回 index.lock 冲突，之后成功（模拟外部 Git 进程短暂持锁）。
        lock_conflict_times: usize,
    }

    impl FakeRunner {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                delay: Duration::ZERO,
                delay_args: None,
                fail: false,
                lock_conflict_times: 0,
            }
        }
        fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
        }
        /// 仅指定命令延迟（用于模拟"读慢写快"等时序）。
        fn with_delay_for(mut self, args: &[&str], delay: Duration) -> Self {
            self.delay_args = Some(args.iter().map(|s| s.to_string()).collect());
            self.delay = delay;
            self
        }
        fn with_fail(mut self) -> Self {
            self.fail = true;
            self
        }
        /// 前 N 次调用返回 index.lock 冲突，之后成功。
        fn with_lock_conflict(mut self, times: usize) -> Self {
            self.lock_conflict_times = times;
            self
        }
        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
        /// 统计指定 args 的调用次数。
        fn call_count_for(&self, args: &[&str]) -> usize {
            let target: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, a)| *a == target)
                .count()
        }
    }

    impl GitCommandRunner for FakeRunner {
        fn run(
            &self,
            workdir: &str,
            args: &[&str],
        ) -> Pin<Box<dyn Future<Output = Result<GitPanelRunOutput, String>> + Send + '_>> {
            let workdir = workdir.to_string();
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let calls = self.calls.clone();
            let delay = self.delay;
            let delay_applies = match &self.delay_args {
                Some(target) => *target == args,
                None => !delay.is_zero(),
            };
            let fail = self.fail;
            let lock_conflict_times = self.lock_conflict_times;
            async move {
                // 先记录发起，再模拟耗时；被取消时也留下记录，便于断言重试次数。
                let seq = {
                    let mut guard = calls.lock().unwrap();
                    guard.push((workdir, args.clone()));
                    guard.len()
                };
                if delay_applies {
                    tokio::time::sleep(delay).await;
                }
                if fail {
                    Ok(GitPanelRunOutput {
                        stdout: String::new(),
                        stderr: "fake boom".to_string(),
                        exit_code: 1,
                    })
                } else if seq <= lock_conflict_times {
                    Ok(GitPanelRunOutput {
                        stdout: String::new(),
                        stderr: "fatal: Unable to create '/repo/.git/index.lock': File exists."
                            .to_string(),
                        exit_code: 128,
                    })
                } else {
                    Ok(GitPanelRunOutput {
                        stdout: args.join(" "),
                        stderr: String::new(),
                        exit_code: 0,
                    })
                }
            }
            .boxed()
        }
    }

    fn new_executor(runner: FakeRunner, ttl: Duration, fail_ttl: Duration) -> GitExecutor {
        GitExecutor::new(Arc::new(runner), ttl, fail_ttl)
    }

    #[test]
    fn uncached_read_always_executes_and_leaves_no_cache() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );

            // 先走缓存读一次，缓存里留下该 workdir 的 Done
            executor.run_read("/repo", &["branch", "--show-current"]).await.unwrap();
            assert_eq!(runner.call_count(), 1);

            // 不走缓存的读必须每次都真实执行，不吃上一条缓存
            executor.run_read_uncached("/repo", &["branch", "--show-current"]).await.unwrap();
            executor.run_read_uncached("/repo", &["branch", "--show-current"]).await.unwrap();
            assert_eq!(runner.call_count(), 3, "uncached 读不应命中缓存");
        });
    }

    /// 写命令按仓库根失效、读命令按调用方原样路径做 key 时，两者字符串不同则失效不到。
    /// 这条用例锁住该现象，说明 uncached 读为何是必要的。
    #[test]
    fn invalidate_with_repo_root_does_not_clear_other_path_spelling() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );

            executor.run_read(r"E:\repo", &["branch", "--show-current"]).await.unwrap();
            executor.invalidate("E:/repo");
            executor.run_read(r"E:\repo", &["branch", "--show-current"]).await.unwrap();
            assert_eq!(runner.call_count(), 1, "反斜杠 key 不应被正斜杠仓库根清掉");
        });
    }

    #[test]
    fn concurrent_reads_execute_once() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_delay(Duration::from_millis(50));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            ));

            let mut handles = Vec::new();
            for _ in 0..10 {
                let executor = executor.clone();
                handles.push(tokio::spawn(async move {
                    executor.run_read("/repo", &["status"]).await
                }));
            }
            for h in handles {
                h.await.unwrap().unwrap();
            }
            assert_eq!(runner.call_count(), 1, "同一命令并发 10 请求应只执行 1 次");
        });
    }

    #[test]
    fn ttl_hit_skips_execution() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );

            executor.run_read("/repo", &["status"]).await.unwrap();
            executor.run_read("/repo", &["status"]).await.unwrap();
            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(runner.call_count(), 1, "TTL 内重复请求不应重新执行");
        });
    }

    #[test]
    fn expired_ttl_re_executes() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_millis(30),
                Duration::from_millis(10),
            );

            executor.run_read("/repo", &["status"]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(60)).await;
            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(runner.call_count(), 2, "TTL 过期后应重新执行");
        });
    }

    #[test]
    fn write_invalidates_read_cache() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            );

            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(runner.call_count(), 1);
            executor.run_write("/repo", &["add", "a.txt"]).await.unwrap();
            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(runner.call_count(), 3, "写命令成功后读缓存应失效并重新执行");
        });
    }

    #[test]
    fn different_repos_isolated() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            );

            executor.run_read("/repo-a", &["status"]).await.unwrap();
            executor.run_read("/repo-b", &["status"]).await.unwrap();
            assert_eq!(runner.call_count(), 2, "不同仓库缓存应隔离");
        });
    }

    #[test]
    fn failed_result_uses_short_ttl() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_fail();
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            );

            // 失败结果也缓存（避免风暴），TTL 内不重试
            assert!(executor.run_read("/repo", &["status"]).await.is_err());
            assert!(executor.run_read("/repo", &["status"]).await.is_err());
            assert_eq!(runner.call_count(), 1, "失败结果在 fail_ttl 内不应重试");

            // fail_ttl 过期后重试
            tokio::time::sleep(Duration::from_millis(600)).await;
            assert!(executor.run_read("/repo", &["status"]).await.is_err());
            assert_eq!(runner.call_count(), 2, "fail_ttl 过期后应重试");
        });
    }

    #[test]
    fn write_commands_serialize() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_delay(Duration::from_millis(20));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            ));

            let mut handles = Vec::new();
            for i in 0..5 {
                let executor = executor.clone();
                handles.push(tokio::spawn(async move {
                    executor
                        .run_write("/repo", &["commit", &format!("msg-{i}")])
                        .await
                }));
            }
            for h in handles {
                h.await.unwrap().unwrap();
            }
            // 互斥保证同一时刻只有一个在执行；fake 记录顺序执行（无交错）。
            assert_eq!(runner.call_count(), 5);
            let calls = runner.calls.lock().unwrap().clone();
            assert!(calls.iter().all(|(wd, _)| wd == "/repo"));
        });
    }

    #[test]
    fn network_timeout_returns_readable_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_delay(Duration::from_secs(5));
            let mut executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );
            // 测试环境把网络超时调短，避免等待 60 秒
            executor.network_timeout = Duration::from_millis(100);

            let result = executor.run_network("/repo", &["fetch"]).await;
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("超时"));
        });
    }

    #[test]
    fn write_retries_on_index_lock_conflict() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 前 2 次撞锁，第 3 次成功。
            let runner = FakeRunner::new().with_lock_conflict(2);
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );

            let result = executor.run_write("/repo", &["add", "a.txt"]).await;
            assert!(result.is_ok(), "撞锁后应自动重试成功：{result:?}");
            assert_eq!(
                runner.call_count_for(&["add", "a.txt"]),
                3,
                "撞锁 2 次 + 成功 1 次，共 3 次调用"
            );
        });
    }

    #[test]
    fn write_gives_up_after_max_lock_retries() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 一直撞锁：首次 + 3 次重试 = 4 次后放弃，返回原始失败输出。
            let runner = FakeRunner::new().with_lock_conflict(usize::MAX);
            let executor = new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            );

            let output = executor
                .run_write("/repo", &["add", "a.txt"])
                .await
                .expect("run_write 本身应返回 Ok：git 非零退出码不算执行错误");
            assert_ne!(output.exit_code, 0, "超过重试上限后应返回原始失败输出");
            assert_eq!(
                runner.call_count_for(&["add", "a.txt"]),
                4,
                "首次 + 3 次重试，共 4 次调用"
            );
        });
    }

    #[test]
    fn network_lock_does_not_block_local_write() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // fetch 持网络锁 300ms，期间本地 add 不应被阻塞。
            let runner = FakeRunner::new().with_delay_for(&["fetch"], Duration::from_millis(300));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            ));

            let e = executor.clone();
            let fetch = tokio::spawn(async move { e.run_network("/repo", &["fetch"]).await });
            tokio::time::sleep(Duration::from_millis(30)).await;

            let start = std::time::Instant::now();
            executor.run_write("/repo", &["add", "a.txt"]).await.unwrap();
            let elapsed = start.elapsed();
            fetch.await.unwrap().unwrap();

            assert!(
                elapsed < Duration::from_millis(200),
                "本地写不应等待网络操作，实际耗时 {elapsed:?}"
            );
        });
    }

    #[test]
    fn pull_blocks_local_write_until_done() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // pull 会改 worktree/index，持本地写锁 200ms，期间 add 必须等待。
            let runner = FakeRunner::new()
                .with_delay_for(&["pull"], Duration::from_millis(200));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            ));

            let e = executor.clone();
            let pull =
                tokio::spawn(async move { e.run_pull("/repo", &["pull"]).await });
            tokio::time::sleep(Duration::from_millis(30)).await;

            let start = std::time::Instant::now();
            executor.run_write("/repo", &["add", "a.txt"]).await.unwrap();
            let elapsed = start.elapsed();
            pull.await.unwrap().unwrap();

            assert!(
                elapsed >= Duration::from_millis(100),
                "pull 持本地写锁期间 add 应等待，实际耗时 {elapsed:?}"
            );
        });
    }

    #[test]
    fn stale_read_result_not_cached_after_write() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // status 慢（读旧状态中），add 快（写完成后 bump 世代号）
            let runner = FakeRunner::new().with_delay_for(&["status"], Duration::from_millis(120));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            ));

            // 读先启动（进入执行，读取旧状态）
            let executor_clone = executor.clone();
            let read_handle = tokio::spawn(async move {
                executor_clone.run_read("/repo", &["status"]).await
            });
            // 等读真正开始执行后，写命令完成并 bump 世代号
            tokio::time::sleep(Duration::from_millis(30)).await;
            executor.run_write("/repo", &["add", "a.txt"]).await.unwrap();

            // 读后完成：世代号已变，旧结果不得写回缓存
            read_handle.await.unwrap().unwrap();
            // 再次请求 status：若旧结果被写回缓存会命中（只执行 1 次），
            // 正确行为是缓存未落、重新执行（共 2 次）
            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(
                runner.call_count_for(&["status"]),
                2,
                "写完成后旧读结果不得落缓存，再次请求应重新执行"
            );
        });
    }

    #[test]
    fn cancelled_executor_slot_self_heals() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_delay(Duration::from_millis(5000));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(2),
                Duration::from_millis(500),
            ));

            // 执行者启动后被取消，悬挂 Inflight 槽位
            let executor_clone = executor.clone();
            let handle = tokio::spawn(async move {
                executor_clone.run_read("/repo", &["status"]).await
            });
            tokio::time::sleep(Duration::from_millis(50)).await;
            handle.abort();
            tokio::time::sleep(Duration::from_millis(50)).await;

            // 同 key 下一次请求必须自愈：重新执行并成功返回
            let result = executor.run_read("/repo", &["status"]).await;
            assert!(result.is_ok(), "取消后同 key 请求应自愈重新执行：{result:?}");
            assert_eq!(runner.call_count_for(&["status"]), 2, "取消后应重新执行一次");
        });
    }

    #[test]
    fn old_executor_drop_does_not_remove_new_executor_slot() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // status 慢（400ms）：A 执行中写命令 invalidate 清槽，B 重建槽位，
            // A 完成后守卫 Drop 不得误删 B 的 Inflight 槽位。
            let runner = FakeRunner::new().with_delay_for(&["status"], Duration::from_millis(400));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            ));

            // A：慢读启动，成为执行者（槽位 id=0）
            let e = executor.clone();
            let a = tokio::spawn(async move { e.run_read("/repo", &["status"]).await });
            tokio::time::sleep(Duration::from_millis(30)).await;
            // 写命令完成：invalidate 清掉 A 的槽位并 bump 世代号
            executor.run_write("/repo", &["add", "a.txt"]).await.unwrap();
            // B：A 还在跑，槽位已空，B 成为新执行者（槽位 id=1）
            tokio::time::sleep(Duration::from_millis(20)).await;
            let e = executor.clone();
            let b = tokio::spawn(async move { e.run_read("/repo", &["status"]).await });
            // C：A 完成后（~400ms）、B 完成前（~420ms+）到达。
            // 若 A 的守卫误删 B 槽位，C 会自己执行（第 3 次调用）。
            tokio::time::sleep(Duration::from_millis(390)).await;
            let c = executor.run_read("/repo", &["status"]).await;

            a.await.unwrap().unwrap();
            b.await.unwrap().unwrap();
            c.unwrap();
            assert_eq!(
                runner.call_count_for(&["status"]),
                2,
                "旧执行者 Drop 不得误删新执行者槽位，C 应等待 B 而不是重复执行"
            );
        });
    }

    #[test]
    fn cancelled_executor_concurrent_rebuild_no_duplicate() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runner = FakeRunner::new().with_delay_for(&["status"], Duration::from_millis(300));
            let executor = std::sync::Arc::new(new_executor(
                runner.clone(),
                Duration::from_secs(60),
                Duration::from_millis(500),
            ));

            // A：执行者，将被取消（悬挂槽位）
            let e = executor.clone();
            let a = tokio::spawn(async move { e.run_read("/repo", &["status"]).await });
            tokio::time::sleep(Duration::from_millis(20)).await;
            // W：等待 A
            let e = executor.clone();
            let w = tokio::spawn(async move { e.run_read("/repo", &["status"]).await });
            tokio::time::sleep(Duration::from_millis(20)).await;
            a.abort();
            // B：与 W 并发抢占重建槽位（等待者重试分支可能删槽，不得误删新执行者）
            tokio::time::sleep(Duration::from_millis(20)).await;
            let e = executor.clone();
            let b = tokio::spawn(async move { e.run_read("/repo", &["status"]).await });

            // W/B 都应收敛到同一个新执行者并成功
            w.await.unwrap().unwrap();
            b.await.unwrap().unwrap();

            // 重建完成后请求：应命中缓存不重复执行
            executor.run_read("/repo", &["status"]).await.unwrap();
            assert_eq!(
                runner.call_count_for(&["status"]),
                2,
                "取消后并发重建只能有一个新执行者，后续请求应命中缓存"
            );
        });
    }
}

