// ==================== 会话自动化脚本托管 ====================
//
// 会话可以给自己写自动化脚本，落在自己的私有目录：
//   chat/conversations/<conversation_id>/automations/
//     automations.json   任务清单（tasks 数组）
//     *.py / *.sh ...    脚本本体，相对路径以 automations/ 为工作目录
//
// 两类任务：
//   - `intervalSeconds` 有值且 > 0：按间隔执行，跑完等退出、再睡够间隔后重跑；
//   - `intervalSeconds` 缺省或 0：常驻脚本，退出即重启，直到会话销毁。
//
// 起停只由两个时刻决定：应用启动时无条件读一遍全部清单；改完清单调用工具的
// `reload` 只重读当前会话的清单。没有巡检、没有文件监听。
//
// 脚本的 stdout / stderr 由本层逐行搬进运行日志，前缀 `[自动化] 输出` / `[自动化] 错误`；
// 每次执行另写一行退出码与耗时。脚本要往会话里说话有两条路：stdout 里打一行
// `{"message": "..."}` 由本层直接投递；或者凭令牌 POST 到激活端点。
//
// 进程树回收是硬约束：Windows 上每个任务进程由独立 Job Object 管辖
// （JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE），Unix 上以独立进程组启动。
// 任一路径的收口——任务取消、会话删除、应用退出——都杀掉整棵进程树，不留孤儿进程。

/// 会话私有自动化目录名。
const AUTOMATION_DIR_NAME: &str = "automations";
/// 任务清单文件名。
const AUTOMATION_CONFIG_FILE_NAME: &str = "automations.json";
/// 常驻脚本退出后的重启等待。
const AUTOMATION_RESTART_BACKOFF_MS: u64 = 5_000;
/// 单次命令结束后的等待上限：进程已取消时用于有界收尾，避免退出路径被卡住。
const AUTOMATION_KILL_WAIT_MS: u64 = 3_000;

fn automation_dir_for_conversation(data_path: &PathBuf, conversation_id: &str) -> PathBuf {
    app_layout_chat_conversations_dir(data_path)
        .join(conversation_id)
        .join(AUTOMATION_DIR_NAME)
}

fn automation_config_for_conversation(data_path: &PathBuf, conversation_id: &str) -> PathBuf {
    automation_dir_for_conversation(data_path, conversation_id).join(AUTOMATION_CONFIG_FILE_NAME)
}

// 清单由会话自己手写，允许带任意额外字段：多余字段静默忽略，不因拼写差异整份拒收。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationAutomationConfig {
    #[serde(default)]
    tasks: Vec<ConversationAutomationTask>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationAutomationTask {
    #[serde(default)]
    id: String,
    #[serde(default)]
    command: String,
    /// 任务用途说明，只用于 describe 回显，不参与调度。
    #[serde(default)]
    description: String,
    /// 有值且 > 0 表示按间隔执行；缺省或 0 表示常驻保活。
    #[serde(default)]
    interval_seconds: Option<u64>,
}

impl ConversationAutomationTask {
    fn interval(&self) -> Option<std::time::Duration> {
        match self.interval_seconds {
            Some(seconds) if seconds > 0 => Some(std::time::Duration::from_secs(seconds)),
            _ => None,
        }
    }
}

/// 把 serde_json 的错误翻成「第几行第几列 + 那一行长什么样」。
///
/// 只报一句 `expected value at line 3 column 5` 的话，写清单的会话还得自己去数行；
/// 把出错那一行的原文一并带出来，改哪一处就是一眼的事。
fn automation_json_error_detail(text: &str, err: &serde_json::Error) -> String {
    let raw = err.to_string();
    let reason = raw.split(" at line ").next().unwrap_or(raw.as_str()).trim();
    let line = err.line();
    let column = err.column();
    let snippet = text
        .lines()
        .nth(line.saturating_sub(1))
        .map(|value| value.trim())
        .unwrap_or_default();
    let snippet = snippet.chars().take(200).collect::<String>();
    if snippet.is_empty() {
        format!("第 {line} 行第 {column} 列：{reason}")
    } else {
        format!("第 {line} 行第 {column} 列：{reason}；该行内容：{snippet}")
    }
}

/// 读取任务清单。`Ok(None)` 表示文件不存在——没有清单和清单写坏是两件事，
/// 前者在启动扫描里是常态，后者必须报给人看。
fn automation_read_config(
    path: &PathBuf,
) -> Result<Option<Vec<ConversationAutomationTask>>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("读取失败，path={}，error={err}", path.display()))?;
    let config = serde_json::from_str::<ConversationAutomationConfig>(&text)
        .map_err(|err| automation_json_error_detail(&text, &err))?;
    Ok(Some(config.tasks))
}

/// 扫一遍 `chat/conversations/*/automations/automations.json`，拿到每个会话的清单。
///
/// 只有启动加载走这里。单份清单坏掉不影响其他会话：错误收进报告，其余照常读。
fn automation_scan_all_conversations(
    data_path: &PathBuf,
) -> Result<
    (
        std::collections::HashMap<String, Vec<ConversationAutomationTask>>,
        Vec<String>,
    ),
    String,
> {
    let root = app_layout_chat_conversations_dir(data_path);
    let mut out = std::collections::HashMap::new();
    let mut errors = Vec::new();
    // 还没有任何会话时这个目录不存在，等于没有清单。
    if !root.is_dir() {
        return Ok((out, errors));
    }
    let entries = fs::read_dir(&root)
        .map_err(|err| format!("读取会话目录失败，path={}，error={err}", root.display()))?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let conversation_id = name.trim().to_string();
        if conversation_id.is_empty() {
            continue;
        }
        let config_path = automation_config_for_conversation(data_path, &conversation_id);
        match automation_read_config(&config_path) {
            Ok(Some(tasks)) => {
                out.insert(conversation_id, tasks);
            }
            Ok(None) => continue,
            Err(err) => errors.push(format!("{}：{err}（该文件未生效，其他会话不受影响）", config_path.display())),
        }
    }
    Ok((out, errors))
}

// ========== 进程托管登记表 ==========

/// 正在跑的任务：任务号 → 它的定义与取消句柄。
struct RunningAutomationTask {
    command: String,
    /// 归一化后的间隔（缺省与 0 都是常驻），对账比的是它，不是原始字段。
    interval: Option<std::time::Duration>,
    cancel: tokio_util::sync::CancellationToken,
    /// 最近一次执行的收口结果；还没跑完过一轮时为空。
    last_run: Option<AutomationRunRecord>,
}

/// 一次执行的收口记录。只记 PAI 自己确定的事实：
/// 什么时候结束、退出码多少、跑了多久。脚本内部打印了什么，看日志。
#[derive(Clone)]
struct AutomationRunRecord {
    /// 进程退出码；被信号杀掉时取不到，为空。
    exit_code: Option<i32>,
    duration_ms: u64,
    finished_at_ms: u64,
}

static CONVERSATION_AUTOMATIONS: OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::collections::HashMap<String, RunningAutomationTask>>>,
> = OnceLock::new();

/// 正在运行的托管任务进程：会话 → 任务 → pid。
///
/// 退出与停机路径必须**同步**杀掉这些进程——取消 token 只是让协程自己收口，
/// 应用退出的那一刻协程可能还没被调度到，靠它收尸就会留下孤儿进程。
static LIVE_AUTOMATION_PROCESSES: OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, std::collections::HashMap<String, u32>>>,
> = OnceLock::new();

type AutomationTaskTable =
    std::collections::HashMap<String, std::collections::HashMap<String, RunningAutomationTask>>;

fn conversation_automations() -> &'static std::sync::Mutex<AutomationTaskTable> {
    CONVERSATION_AUTOMATIONS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn live_automation_processes() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
> {
    LIVE_AUTOMATION_PROCESSES
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn automation_register_live_process(conversation_id: &str, task_id: &str, pid: u32) {
    if let Ok(mut guard) = live_automation_processes().lock() {
        guard
            .entry(conversation_id.to_string())
            .or_default()
            .insert(task_id.to_string(), pid);
    }
}

fn automation_unregister_live_process(conversation_id: &str, task_id: &str, pid: u32) {
    if let Ok(mut guard) = live_automation_processes().lock() {
        let removed = match guard.get_mut(conversation_id) {
            Some(tasks) => {
                // 只摘自己那一条：同一任务重启用新 pid 时，旧进程的收尾不能把新条目删掉。
                if tasks.get(task_id) == Some(&pid) {
                    tasks.remove(task_id);
                }
                tasks.is_empty()
            }
            None => false,
        };
        if removed {
            guard.remove(conversation_id);
        }
    }
}

/// 同步杀进程树：不依赖 tokio 协程被调度。
///
/// Windows 走 `taskkill /T /F`（连子进程一起杀），Unix 先按独立进程组杀、再补一刀 pid。
fn automation_kill_process_tree_blocking(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        let mut command = std::process::Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x0800_0000);
        let _ = command.status();
    }
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}

#[cfg(target_os = "windows")]
fn automation_process_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // STILL_ACTIVE 是 Windows 定义的「进程仍在运行」哨兵退出码。
    const STILL_ACTIVE: u32 = 259;
    if pid == 0 {
        return false;
    }
    let handle: HANDLE = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    ok != 0 && exit_code == STILL_ACTIVE
}

#[cfg(unix)]
fn automation_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

/// 杀掉登记的托管进程并等它们真的消失。`conversation_id` 与 `task_id` 都可限定范围。
fn automation_kill_live_processes(
    conversation_id: Option<&str>,
    task_id: Option<&str>,
) -> usize {
    let pids = {
        let guard = match live_automation_processes().lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        guard
            .iter()
            .filter(|(conv, _)| conversation_id.map_or(true, |want| **conv == want))
            .flat_map(|(_, tasks)| tasks.iter())
            .filter(|(task, _)| task_id.map_or(true, |want| **task == want))
            .map(|(_, pid)| *pid)
            .collect::<Vec<u32>>()
    };
    if pids.is_empty() {
        return 0;
    }
    for pid in &pids {
        automation_kill_process_tree_blocking(*pid);
    }
    // 有界等待：确认这些 pid 真的没了，孤儿进程不能留到应用退出之后。
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(AUTOMATION_KILL_WAIT_MS);
    while std::time::Instant::now() < deadline {
        if !pids.iter().any(|pid| automation_process_alive(*pid)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // 这些 pid 已经处理过，登记表里不该再留着旧 pid：进程号会被复用，留着会误杀无关进程。
    // 杀不掉的进程只记日志，不留在表里。
    if let Ok(mut guard) = live_automation_processes().lock() {
        guard.retain(|conv, tasks| {
            if conversation_id.is_some_and(|want| want != conv.as_str()) {
                return true;
            }
            tasks.retain(|task, _| task_id.is_some_and(|want| want != task.as_str()));
            !tasks.is_empty()
        });
    }    pids.len()
}

// ========== 单个任务的进程执行 ==========

/// 一次子进程运行的持有物。Drop 即杀树：Windows 关 Job 句柄（kill-on-close），
/// Unix 杀整个进程组，再兜底 kill 直接子进程。
struct AutomationProcess {
    child: Option<tokio::process::Child>,
    pid: Option<u32>,
    conversation_id: String,
    task_id: String,
    /// 自然退出时留下的状态；被取消或没起过时为空。
    exit_status: Option<std::process::ExitStatus>,
    #[cfg(target_os = "windows")]
    job: Option<WindowsJobGuard>,
}

impl AutomationProcess {
    fn new(conversation_id: &str, task_id: &str) -> Self {
        Self {
            child: None,
            pid: None,
            conversation_id: conversation_id.to_string(),
            task_id: task_id.to_string(),
            exit_status: None,
            #[cfg(target_os = "windows")]
            job: None,
        }
    }

    /// 从存活登记表摘掉自己（只摘自己那个 pid）。
    fn unregister_live(&self) {
        if let Some(pid) = self.pid {
            automation_unregister_live_process(&self.conversation_id, &self.task_id, pid);
        }
    }

    /// 杀掉整棵进程树并等待直接子进程退出。
    async fn terminate(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(job) = self.job.as_ref() {
                let _ = job.terminate_job();
            }
        }
        #[cfg(unix)]
        {
            if let Some(pid) = self.pid {
                // 进程以独立进程组启动，负号杀整组，后代一并带走。
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
        self.unregister_live();
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(
                std::time::Duration::from_millis(AUTOMATION_KILL_WAIT_MS),
                child.wait(),
            )
            .await;
        }
        self.child = None;
        self.pid = None;
        #[cfg(target_os = "windows")]
        {
            self.job = None;
        }
    }
}

impl Drop for AutomationProcess {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if let Some(job) = self.job.as_ref() {
                let _ = job.terminate_job();
            }
        }
        #[cfg(unix)]
        {
            if let Some(pid) = self.pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        self.unregister_live();
    }
}

/// 按平台起一条 shell 命令。stdin 接空设备（脚本不该等输入），
/// stdout / stderr 走管道，由 `automation_spawn` 逐行搬进运行日志。
/// `kill_on_drop` 是最后一道保险。
fn automation_build_command(command: &str, cwd: &PathBuf) -> tokio::process::Command {
    let mut builder = if cfg!(target_os = "windows") {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    } else {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };
    builder
        .current_dir(cwd)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW：后台脚本不该弹控制台窗口。
        // CREATE_SUSPENDED：先挂起创建，入 Job 后再恢复，杜绝「先跑出孙进程再入 job」的竞态。
        builder.creation_flags(0x0800_0000 | 0x0000_0004);
    }
    #[cfg(unix)]
    {
        // 独立进程组：收口时按组杀，后代逃不掉。
        builder.process_group(0);
    }
    builder
}

/// 把子进程的一条输出流逐行搬进运行日志。
///
/// 必须边跑边读：管道缓冲区满了子进程会卡在 write 上，脚本就死在那儿。
/// 不做截断、不限行数——脚本打印多少是脚本自己的事，框架只负责如实搬回来。
/// 脚本投递：stdout 里一行 JSON、带非空 message 字符串，就把它送进本会话。
///
/// 这是脚本跟会话说话的最短路径：不用令牌、不用端点、不用管网络访问开没开。
/// web 激活接口仍然保留，两条路都通。
fn automation_stdout_delivery_text(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let message = parsed.get("message")?.as_str()?.trim();
    if message.is_empty() {
        return None;
    }
    Some(message.to_string())
}

/// 把子进程的一条输出流逐行搬进运行日志。
///
/// 必须边跑边读：管道缓冲区满了子进程会卡在 write 上，脚本就死在那儿。
/// 不做截断、不限行数——脚本打印多少是脚本自己的事，框架只负责如实搬回来。
/// `deliver` 为真时（只对 stdout），带 message 的 JSON 行会被投递进本会话。
fn automation_pipe_to_log<R>(
    stream: R,
    state: &AppState,
    conversation_id: &str,
    task_id: &str,
    tag: &'static str,
    deliver: bool,
)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    use tokio::io::AsyncBufReadExt as _;
    let state = state.clone();
    let conversation_id = conversation_id.to_string();
    let task_id = task_id.to_string();
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let decoded = String::from_utf8_lossy(&buf);
                    let line = decoded.trim_end_matches(['\r', '\n']);
                    if deliver {
                        if let Some(text) = automation_stdout_delivery_text(line) {
                            match activation_deliver_event(&state, &conversation_id, &text) {
                                Ok(event_id) => runtime_log_info(format!(
                                    "[自动化] 投递 任务={task_id} 会话={conversation_id} 事件={event_id} 正文={text}"
                                )),
                                Err(err) => runtime_log_warn(format!(
                                    "[自动化] 失败 任务=投递脚本输出 会话={conversation_id} 自动化={task_id} 原因={err}"
                                )),
                            }
                            continue;
                        }
                    }
                    runtime_log_info(format!(
                        "[自动化] {tag} 任务={task_id} 会话={conversation_id} {line}"
                    ));
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[自动化] 跳过 任务=读取脚本{tag} 会话={conversation_id} 自动化={task_id} 原因={err}"
                    ));
                    break;
                }
            }
        }
    });
}

/// 启动一条命令，并把它的整棵树纳入管辖。
fn automation_spawn(
    state: &AppState,
    command: &str,
    cwd: &PathBuf,
    conversation_id: &str,
    task_id: &str,
) -> Result<AutomationProcess, String> {
    let mut process = AutomationProcess::new(conversation_id, task_id);
    let mut builder = automation_build_command(command, cwd);
    builder
        .env("PAI_CONVERSATION_ID", conversation_id)
        .env("PAI_AUTOMATION_ID", task_id)
        .env("PYTHONIOENCODING", "utf-8");
    let mut child = builder
        .spawn()
        .map_err(|err| format!("启动自动化任务失败，id={task_id}，error={err}"))?;
    // 两条输出流必须当场接管：管道缓冲区满了子进程会卡在 write 上。
    if let Some(stdout) = child.stdout.take() {
        automation_pipe_to_log(stdout, state, conversation_id, task_id, "输出", true);
    }
    if let Some(stderr) = child.stderr.take() {
        automation_pipe_to_log(stderr, state, conversation_id, task_id, "错误", false);
    }
    process.pid = child.id();
    process.child = Some(child);
    // 先登记再入 job：登记表是退出/停机的同步收口依据，晚一步就有一刀砍不到的空窗。
    if let Some(pid) = process.pid {
        automation_register_live_process(conversation_id, task_id, pid);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(pid) = process.pid {
            match WindowsJobGuard::create_kill_on_close().and_then(|job| {
                job.assign_process_id(pid).map(|()| job)
            }) {
                Ok(job) => process.job = Some(job),
                Err(err) => runtime_log_warn(format!(
                    "[自动化] 跳过 任务=进程树管辖 原因={err} 会话={conversation_id} 自动化={task_id}"
                )),
            }
            // 无论入 job 成功与否都必须恢复：进程是以挂起态创建的。
            if let Err(err) = exec_windows_resume_primary_thread(pid) {
                return Err(format!("恢复挂起的自动化进程失败，id={task_id}，error={err}"));
            }
        }
    }
    Ok(process)
}

/// 等待一次运行自然结束，并把退出状态留在 `process.exit_status` 上；
/// 已被取消时立刻收口返回 `None`。
async fn automation_wait_exit(
    process: &mut AutomationProcess,
    cancel: &tokio_util::sync::CancellationToken,
) -> Option<()> {
    let Some(child) = process.child.as_mut() else {
        return Some(());
    };
    let status = tokio::select! {
        _ = cancel.cancelled() => return None,
        status = child.wait() => status,
    };
    match status {
        Ok(status) => process.exit_status = Some(status),
        Err(err) => runtime_log_warn(format!(
            "[自动化] 跳过 任务=等待脚本退出 自动化={} 原因={err}",
            process.task_id
        )),
    }
    Some(())
}

/// 一次执行收口：写一行日志，并把结果记到登记表上供 `get` 回执带出。
///
/// 日志是完整历史，给人看；登记表只留最近一次，给会话自己看——
/// 它想判断「脚本到底跑没跑、成没成」，不该去翻日志。
fn automation_record_run_end(
    conversation_id: &str,
    task_id: &str,
    status: Option<&std::process::ExitStatus>,
    elapsed: std::time::Duration,
) {
    let exit_code = status.and_then(|status| status.code());
    let finished_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0);
    let code_text = exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "无".to_string());
    let line = format!(
        "[自动化] 执行 任务={task_id} 会话={conversation_id} 退出码={code_text} 耗时={}ms",
        elapsed.as_millis()
    );
    // 非 0 退出按失败记 warn 级别，翻日志时一眼看得见。
    if exit_code == Some(0) {
        runtime_log_info(line);
    } else {
        runtime_log_warn(line);
    }
    let Ok(mut guard) = conversation_automations().lock() else {
        return;
    };
    let Some(tasks) = guard.get_mut(conversation_id) else {
        return;
    };
    let Some(task) = tasks.get_mut(task_id) else {
        return;
    };
    task.last_run = Some(AutomationRunRecord {
        exit_code,
        duration_ms: elapsed.as_millis() as u64,
        finished_at_ms,
    });
}

/// 常驻脚本：跑、退出、等待、重跑，直到取消。
async fn automation_run_resident(
    state: AppState,
    conversation_id: String,
    task_id: String,
    command: String,
    cancel: tokio_util::sync::CancellationToken,
) {
    let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let started = std::time::Instant::now();
        match automation_spawn(&state, &command, &cwd, &conversation_id, &task_id) {
            Ok(mut process) => {
                let cancelled = automation_wait_exit(&mut process, &cancel).await.is_none();
                process.terminate().await;
                if !cancelled {
                    automation_record_run_end(
                        &conversation_id,
                        &task_id,
                        process.exit_status.as_ref(),
                        started.elapsed(),
                    );
                }
                if cancelled {
                    return;
                }
            }
            Err(err) => runtime_log_error(format!(
                "[自动化] 失败 任务=常驻脚本启动 会话={conversation_id} 自动化={task_id} 原因={err}"
            )),
        }
        if automation_sleep_or_cancel(
            &cancel,
            std::time::Duration::from_millis(AUTOMATION_RESTART_BACKOFF_MS),
        )
        .await
        .is_err()
        {
            return;
        }
    }
}

/// 定时脚本：跑完等退出，再睡满间隔，循环到取消。
async fn automation_run_interval(
    state: AppState,
    conversation_id: String,
    task_id: String,
    command: String,
    interval: std::time::Duration,
    cancel: tokio_util::sync::CancellationToken,
) {
    let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let started = std::time::Instant::now();
        match automation_spawn(&state, &command, &cwd, &conversation_id, &task_id) {
            Ok(mut process) => {
                let cancelled = automation_wait_exit(&mut process, &cancel).await.is_none();
                process.terminate().await;
                if !cancelled {
                    automation_record_run_end(
                        &conversation_id,
                        &task_id,
                        process.exit_status.as_ref(),
                        started.elapsed(),
                    );
                }
                if cancelled {
                    return;
                }
            }
            Err(err) => runtime_log_error(format!(
                "[自动化] 失败 任务=定时脚本启动 会话={conversation_id} 自动化={task_id} 原因={err}"
            )),
        }
        if automation_sleep_or_cancel(&cancel, interval).await.is_err() {
            return;
        }
    }
}

/// 可被取消打断的睡眠；被打断时返回 `Err(())`。
async fn automation_sleep_or_cancel(
    cancel: &tokio_util::sync::CancellationToken,
    duration: std::time::Duration,
) -> Result<(), ()> {
    tokio::select! {
        _ = cancel.cancelled() => Err(()),
        _ = tokio::time::sleep(duration) => Ok(()),
    }
}

/// 起一个任务的协程。
fn conversation_automation_spawn_task(
    state: &AppState,
    conversation_id: &str,
    task: &ConversationAutomationTask,
    cancel: tokio_util::sync::CancellationToken,
) {
    let state = state.clone();
    let conversation_id = conversation_id.to_string();
    let task_id = task.id.clone();
    let command = task.command.clone();
    let interval = task.interval();
    tauri::async_runtime::spawn(async move {
        match interval {
            Some(interval) => {
                automation_run_interval(
                    state,
                    conversation_id,
                    task_id,
                    command,
                    interval,
                    cancel,
                )
                .await
            }
            None => {
                automation_run_resident(state, conversation_id, task_id, command, cancel).await
            }
        }
    });
}

/// 停机一个会话的托管：摘掉它的全部任务，同步杀掉进程树。
fn conversation_automation_stop(conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    let removed = {
        let mut guard = match conversation_automations().lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        guard.remove(conversation_id)
    };
    let Some(running) = removed else {
        return;
    };
    // 先同步杀进程，再取消：反过来的话协程收口是异步的，调用方拿不到「已经停干净」的保证。
    let killed = automation_kill_live_processes(Some(conversation_id), None);
    for (_, task) in running {
        task.cancel.cancel();
    }
    runtime_log_info(format!(
        "[自动化] 完成 任务=会话自动化停机 会话={conversation_id} 杀进程数={killed}"
    ));
}

/// 应用退出路径：撤销全部会话托管，并同步杀干净所有托管进程。
///
/// 退出时不能只 `cancel()` 就返回——`app.exit` 之后运行时会被拆掉，
/// 那些还在等调度的协程根本跑不到自己的收尸逻辑。
fn conversation_automation_stop_all() {
    let handles = {
        let mut guard = match conversation_automations().lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        std::mem::take(&mut *guard)
    };
    let task_count = handles.values().map(|tasks| tasks.len()).sum::<usize>();
    for (_, tasks) in handles {
        for (_, task) in tasks {
            task.cancel.cancel();
        }
    }
    let killed = automation_kill_live_processes(None, None);
    if task_count == 0 && killed == 0 {
        return;
    }
    runtime_log_info(format!(
        "[自动化] 完成 任务=退出前停机全部会话自动化 任务数={task_count} 杀进程数={killed}"
    ));
}

/// 一次 reload 的结果：被接受的条目与没被接受的原因。
///
/// 调用方只关心「我写的清单对不对」，所以只给这两样；
/// 扫描范围、扫了几个会话、几个文件一概不报，那是噪音。
#[derive(Debug, Default)]
struct AutomationReloadOutcome {
    /// 被接受、正在托管的条目。
    tasks: Vec<Value>,
    /// 没被接受的原因，一条一句。
    errors: Vec<String>,
    /// 这轮新起 / 停掉的任务数，只进日志，不回执。
    started_count: usize,
    stopped_count: usize,
}

/// 对账内核：拿「会话 → 清单」去和登记表对账，起该起的、停该停的。
///
/// 只碰 `desired` 里出现的会话。不在 `desired` 里的会话一个都不动——
/// 一个会话改自己的清单，不该牵连别人的任务。
fn automation_reconcile(
    state: &AppState,
    desired: &std::collections::HashMap<String, Vec<ConversationAutomationTask>>,
    outcome: &mut AutomationReloadOutcome,
) -> Result<(), String> {
    // 第一步：算出要停的任务，摘出句柄。杀进程树必须放在锁外。
    let mut to_stop: Vec<(String, String, tokio_util::sync::CancellationToken)> = Vec::new();
    {
        let mut guard = match conversation_automations().lock() {
            Ok(guard) => guard,
            Err(_) => return Err("自动化登记表不可用".to_string()),
        };
        for (conversation_id, tasks) in desired {
            let Some(running) = guard.get_mut(conversation_id) else {
                continue;
            };
            let mut stale = Vec::new();
            for (task_id, task) in running.iter() {
                let keep = tasks.iter().any(|want| {
                    &want.id == task_id
                        && want.command.trim() == task.command
                        && want.interval() == task.interval
                });
                if !keep {
                    stale.push(task_id.clone());
                }
            }
            for task_id in stale {
                if let Some(task) = running.remove(&task_id) {
                    to_stop.push((conversation_id.clone(), task_id, task.cancel));
                }
            }
            if running.is_empty() {
                guard.remove(conversation_id);
            }
        }
    }
    for (conversation_id, task_id, cancel) in &to_stop {
        cancel.cancel();
        let killed = automation_kill_live_processes(Some(conversation_id), Some(task_id));
        outcome.stopped_count += 1;
        runtime_log_info(format!(
            "[自动化] 完成 任务=停掉已移除的自动化 会话={conversation_id} 自动化={task_id} 杀进程数={killed}"
        ));
    }

    // 第二步：清单里合格的条目逐条校验并起协程。
    let mut to_start = Vec::new();
    {
        let mut guard = match conversation_automations().lock() {
            Ok(guard) => guard,
            Err(_) => return Err("自动化登记表不可用".to_string()),
        };
        for (conversation_id, tasks) in desired {
            for (index, task) in tasks.iter().enumerate() {
                // 条目本身不合法就不托管，并且要说清是第几条、哪里不对。
                let mut problems = Vec::new();
                if task.id.trim().is_empty() {
                    problems.push("id 为空");
                }
                if task.command.trim().is_empty() {
                    problems.push("command 为空");
                }
                if !problems.is_empty() {
                    outcome.errors.push(format!(
                        "第 {} 条（id={:?}）：{}，这一条不会被托管",
                        index + 1,
                        task.id,
                        problems.join("、")
                    ));
                    continue;
                }
                let running = guard.entry(conversation_id.clone()).or_default();
                let already = running.contains_key(&task.id);
                outcome.tasks.push(serde_json::json!({
                    "id": task.id,
                    "command": task.command.trim(),
                    "intervalSeconds": task.interval_seconds,
                    "state": if already { "running" } else { "started" },
                }));
                if already {
                    continue;
                }
                let cancel = tokio_util::sync::CancellationToken::new();
                running.insert(
                    task.id.clone(),
                    RunningAutomationTask {
                        command: task.command.trim().to_string(),
                        interval: task.interval(),
                        cancel: cancel.clone(),
                        last_run: None,
                    },
                );
                outcome.started_count += 1;
                to_start.push((conversation_id.clone(), task.clone(), cancel));
            }
        }
    }
    for (conversation_id, task, cancel) in to_start {
        conversation_automation_spawn_task(state, &conversation_id, &task, cancel);
    }
    Ok(())
}

/// `get` 回执里带出的最近日志条数。
const AUTOMATION_RECENT_LOG_LINES: usize = 20;
/// 取日志时从文件末尾读多少字节：够放几千行，不必整读 5MB。
const AUTOMATION_LOG_TAIL_BYTES: u64 = 512 * 1024;

/// 会话在日志里的样子：日志层会把 36 位 UUID 截成前 8 位，非 UUID 的 id 原样保留。
fn automation_log_conversation_tag(conversation_id: &str) -> String {
    let trimmed = conversation_id.trim();
    let bytes = trimmed.as_bytes();
    let looks_like_uuid = bytes.len() >= 36
        && bytes.get(8) == Some(&b'-')
        && bytes.get(13) == Some(&b'-')
        && bytes.get(18) == Some(&b'-')
        && bytes.get(23) == Some(&b'-');
    if looks_like_uuid {
        trimmed[..8].to_string()
    } else {
        trimmed.to_string()
    }
}

/// 从运行日志里取本会话最近的自动化记录。
///
/// 日志本身就是完整历史，缺的只是一个按会话取最近几条的入口：
/// 会话想知道「脚本刚打印了什么」，不该自己去猜日志路径、拼 grep 命令。
/// 先读当前文件；刚轮转时当前文件里可能一条都没有，再补读最新的一份归档。
fn automation_recent_log_lines(conversation_id: &str, max_lines: usize) -> Vec<String> {
    let Some(dir) = resolve_runtime_log_dir() else {
        return Vec::new();
    };
    let needle = format!("会话={}", automation_log_conversation_tag(conversation_id));
    let current = dir.join(format!("{RUNTIME_LOG_FILE_NAME}.log"));
    let mut lines = automation_tail_matching_lines(&current, &needle, max_lines);
    if lines.is_empty() {
        if let Some(latest) = automation_latest_archived_log(&dir) {
            lines = automation_tail_matching_lines(&latest, &needle, max_lines);
        }
    }
    lines
}

/// 从文件尾部读一段，挑出同时含 `[自动化]` 与会话标记的行，保留最后 `max_lines` 条。
fn automation_tail_matching_lines(
    path: &PathBuf,
    needle: &str,
    max_lines: usize,
) -> Vec<String> {
    use std::io::{Read, Seek, SeekFrom};
    if max_lines == 0 {
        return Vec::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(size) = file.metadata().map(|meta| meta.len()) else {
        return Vec::new();
    };
    let start = size.saturating_sub(AUTOMATION_LOG_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    // 日志里可能混着脚本吐出的非 UTF-8 字节，按损失转换读，不因此整段丢掉。
    let buffer = String::from_utf8_lossy(&bytes);
    let mut text = buffer.as_ref();
    if start > 0 {
        // 从中间切进来的第一行是残行，丢掉。
        match text.find('\n') {
            Some(pos) => text = &text[pos + 1..],
            None => return Vec::new(),
        }
    }
    let mut collected: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for line in text.lines() {
        if line.contains("[自动化]") && line.contains(needle) {
            if collected.len() == max_lines {
                collected.pop_front();
            }
            collected.push_back(line.trim_end().to_string());
        }
    }
    collected.into_iter().collect()
}

/// 目录里最新的一份轮转日志。
fn automation_latest_archived_log(dir: &PathBuf) -> Option<PathBuf> {
    let prefix = format!("{RUNTIME_LOG_FILE_NAME}_");
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(".log") {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        if best.as_ref().map(|(time, _)| modified > *time).unwrap_or(true) {
            best = Some((modified, entry.path()));
        }
    }
    best.map(|(_, path)| path)
}

/// 通知前端：这份清单刚被重读过，任务列表需要刷新。
fn conversation_automation_broadcast_updated(state: &AppState, conversation_id: &str) {
    let payload = serde_json::json!({ "conversationId": conversation_id });
    ide_chat_broadcast_notification("automation.updated", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!("[自动化] 获取窗口句柄失败：锁已损坏 错误={err}"));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:automation-updated", payload) {
        runtime_log_error(format!("[自动化] 推送失败 错误={err}"));
    }
}

/// 只重读当前会话的清单：其他会话的任务一个都不碰。
///
/// 清单文件不存在时按「这个会话没有任务」处理：把它自己的任务停掉，并在回执里说明期望路径。
/// 清单写坏时不清空、不停任务——读不到新清单就当这次没改，报错让人去修。
fn conversation_automation_reload_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<AutomationReloadOutcome, String> {
    let conversation_id = conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("缺少会话 id，无法重读自动化清单".to_string());
    }
    let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);
    let mut desired = std::collections::HashMap::new();
    let mut outcome = AutomationReloadOutcome::default();
    match automation_read_config(&config_path) {
        Ok(Some(tasks)) => {
            desired.insert(conversation_id.clone(), tasks);
        }
        Ok(None) => {
            desired.insert(conversation_id.clone(), Vec::new());
            outcome.errors.push(format!(
                "{} 不存在：当前会话还没有自动化清单，要托管任务就写这个文件再调一次 reload。",
                config_path.display()
            ));
        }
        Err(err) => {
            outcome.errors.push(format!("{}：{err}", config_path.display()));
            return Ok(outcome);
        }
    }
    automation_reconcile(state, &desired, &mut outcome)?;
    conversation_automation_broadcast_updated(state, &conversation_id);
    runtime_log_info(format!(
        "[自动化] 完成 任务=重读会话自动化清单 会话={conversation_id} 任务数={} 起任务数={} 停任务数={} 错误数={}",
        outcome.tasks.len(),
        outcome.started_count,
        outcome.stopped_count,
        outcome.errors.len()
    ));
    Ok(outcome)
}

/// 启动时读一遍全部会话的清单。进程刚起、登记表为空，只起不停。
fn conversation_automation_reload_all(state: &AppState) -> Result<AutomationReloadOutcome, String> {
    let (desired, scan_errors) = automation_scan_all_conversations(&state.data_path)?;
    let mut outcome = AutomationReloadOutcome {
        errors: scan_errors,
        ..Default::default()
    };
    automation_reconcile(state, &desired, &mut outcome)?;
    runtime_log_info(format!(
        "[自动化] 完成 任务=启动加载全部自动化清单 任务数={} 起任务数={} 错误数={}",
        outcome.tasks.len(),
        outcome.started_count,
        outcome.errors.len()
    ));
    Ok(outcome)
}

/// 清单里现在有什么、各自跑没跑。
fn conversation_automation_describe(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<Value>, String> {
    let config_path = automation_config_for_conversation(&state.data_path, conversation_id);
    let tasks = automation_read_config(&config_path)?.unwrap_or_default();
    let running = {
        let guard = match conversation_automations().lock() {
            Ok(guard) => guard,
            Err(_) => return Err("自动化登记表不可用".to_string()),
        };
        guard
            .get(conversation_id)
            .map(|tasks| {
                tasks
                    .iter()
                    .map(|(id, task)| (id.clone(), task.last_run.clone()))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default()
    };
    Ok(tasks
        .into_iter()
        .map(|task| {
            let record = running.get(&task.id);
            serde_json::json!({
                "id": task.id,
                "command": task.command,
                "description": task.description,
                "intervalSeconds": task.interval_seconds,
                "running": record.is_some(),
                "lastRun": record.and_then(|record| record.clone()).map(|record| {
                    serde_json::json!({
                        "exitCode": record.exit_code,
                        "durationMs": record.duration_ms,
                        "finishedAtMs": record.finished_at_ms,
                    })
                }),
            })
        })
        .collect())
}

/// 前端读取当前会话的自动化任务列表：就是 describe 的那份，不做任何加工。
#[tauri::command]
fn automation_list_tasks(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    conversation_automation_describe(state.inner(), &conversation_id)
}
