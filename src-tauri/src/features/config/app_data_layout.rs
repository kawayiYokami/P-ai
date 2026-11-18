const LAYOUT_DIR_CONFIG: &str = "config";
const LAYOUT_DIR_STATE: &str = "state";
const LAYOUT_DIR_CHAT: &str = "chat";
const LAYOUT_DIR_CHAT_CONVERSATIONS: &str = "conversations";
const LAYOUT_DIR_BACKUPS: &str = "backups";
const LAYOUT_FILE_AGENTS: &str = "agents.json";
const LAYOUT_FILE_RUNTIME: &str = "runtime_state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    #[serde(default)]
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatIndexConversationItem {
    id: String,
    updated_at: String,
    status: String,
    #[serde(default)]
    archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ChatIndexFile {
    #[serde(default)]
    conversations: Vec<ChatIndexConversationItem>,
}

fn app_layout_config_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_CONFIG)
}

fn app_layout_state_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_STATE)
}

fn app_layout_chat_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_CHAT)
}

fn app_layout_chat_conversations_dir(path: &PathBuf) -> PathBuf {
    app_layout_chat_dir(path).join(LAYOUT_DIR_CHAT_CONVERSATIONS)
}

fn app_layout_backups_dir(path: &PathBuf) -> PathBuf {
    app_root_from_data_path(path).join(LAYOUT_DIR_BACKUPS)
}

fn app_layout_agents_path(path: &PathBuf) -> PathBuf {
    app_layout_config_dir(path).join(LAYOUT_FILE_AGENTS)
}

fn app_layout_runtime_state_path(path: &PathBuf) -> PathBuf {
    app_layout_state_dir(path).join(LAYOUT_FILE_RUNTIME)
}

fn app_layout_chat_conversation_path(path: &PathBuf, conversation_id: &str) -> PathBuf {
    app_layout_chat_conversations_dir(path).join(format!("{conversation_id}.json"))
}

fn build_agents_file(agents: &[AgentProfile]) -> AgentsFile {
    AgentsFile {
        agents: agents.to_vec(),
    }
}

// ==================== 人格文件：内置打底 + 文件覆盖，逐条容错 ====================

/// 内置人格：来自代码，永远是可用基线。
///
/// 内置与自定义人格以前同住 `agents.json` 且文件整体取代内置，于是「用户写坏一个逗号，
/// 连 Pai / 副手 / reviewer / support 一起消失」。现在内置只从代码取，文件只负责
/// 「覆盖同名内置」与「追加自定义」，文件坏了也伤不到内置。
fn built_in_agents() -> Vec<AgentProfile> {
    let mut agents = vec![default_agent(), default_deputy_agent()];
    agents.extend(built_in_organization_agents());
    agents.push(default_user_persona());
    agents.push(default_system_persona());
    agents
}

/// 时间戳缺失补齐：缺 createdAt 用 updatedAt，两个都缺用当前时间；不留空串进排序。
fn normalize_agent_timestamps(agent: &mut AgentProfile) {
    let created = agent.created_at.trim().to_string();
    let updated = agent.updated_at.trim().to_string();
    let updated = if updated.is_empty() {
        if created.is_empty() {
            now_iso()
        } else {
            created.clone()
        }
    } else {
        updated
    };
    let created = if created.is_empty() {
        updated.clone()
    } else {
        created
    };
    agent.created_at = created;
    agent.updated_at = updated;
}

/// 人格文件读取诊断：按数据根分开存。
///
/// 以前是进程级单例、每次直读无条件清空：界面上「先 load_agents 再 load_agents_diagnostics」
/// 两次 IPC 之间只要有任意一次读取（迁移、头像任务、另一个窗口），待展示的提示就没了。
/// 按数据根分开后，各处的读取只影响自己那一份结论。
fn agents_load_diagnostics_store(
) -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, Vec<String>>> {
    static DIAGNOSTICS: OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, Vec<String>>>> =
        OnceLock::new();
    DIAGNOSTICS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 某个数据根上「最近一次读取人格文件」的诊断结论。
fn agents_load_diagnostics(data_path: &PathBuf) -> Vec<String> {
    agents_load_diagnostics_store()
        .lock()
        .map(|items| items.get(data_path).cloned().unwrap_or_default())
        .unwrap_or_default()
}

/// 发布本轮读取的结论：覆盖该数据根的上一次结论，正常的读取会把旧提示清掉。
fn publish_agents_load_diagnostics(data_path: &PathBuf, messages: &[String]) {
    if let Ok(mut items) = agents_load_diagnostics_store().lock() {
        if messages.is_empty() {
            items.remove(data_path);
        } else {
            items.insert(data_path.clone(), messages.to_vec());
        }
    }
}

/// 已被判定「不可信」的人格文件（语法坏且备份失败）：这类路径绝不允许再次覆盖写入。
fn agents_untrusted_files() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    static UNTRUSTED: OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
        OnceLock::new();
    UNTRUSTED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn mark_agents_file_untrusted(path: &PathBuf) {
    if let Ok(mut files) = agents_untrusted_files().lock() {
        files.insert(path.clone());
    }
}

fn agents_file_is_untrusted(path: &PathBuf) -> bool {
    agents_untrusted_files()
        .lock()
        .map(|files| files.contains(path))
        .unwrap_or(false)
}

/// 把坏掉的人格文件原样备份成 `agents.json.corrupt-<时间戳>`，返回备份路径。
///
/// 兜底是「内存里用内置人格」，一旦用户接着改人格就会写回磁盘；不先备份，
/// 用户那份「只少个逗号、手工能修」的数据就被覆盖没了。
fn backup_corrupt_agents_file(path: &PathBuf) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let backup_path = path.with_extension(format!("json.corrupt-{stamp}"));
    let original = fs::read(path).map_err(|err| format!("读取待备份人格文件失败: {err}"))?;
    fs::copy(path, &backup_path).map_err(|err| format!("备份人格文件失败: {err}"))?;
    let copied = fs::read(&backup_path).map_err(|err| format!("校验备份失败: {err}"))?;
    if copied != original {
        return Err(format!(
            "备份校验不一致（原文件 {} 字节，备份 {} 字节）",
            original.len(),
            copied.len()
        ));
    }
    Ok(backup_path)
}

/// 记录一条本轮读取的诊断：写 error 日志 + 收进本轮结论，绝不让损坏静默发生。
fn record_agents_load_diagnostic(messages: &mut Vec<String>, message: String) {
    runtime_log_error(format!("[配置] {message}"));
    messages.push(message);
}

/// 把 serde 的语法错误翻成「文件 + 行列 + 出错行原文 + 指位」的中文提示。
fn agents_syntax_error_message(path: &PathBuf, content: &str, err: &serde_json::Error) -> String {
    let line = err.line();
    let column = err.column();
    let line_text = content
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or_default();
    let detail = match err.classify() {
        serde_json::error::Category::Syntax if err.to_string().contains("trailing comma") => {
            "该行末尾多了一个逗号（JSON 不允许尾逗号）".to_string()
        }
        serde_json::error::Category::Syntax => format!("JSON 语法错误：{err}"),
        _ => format!("结构不匹配：{err}"),
    };
    let mut caret = String::new();
    if column > 0 {
        caret = format!("\n{}^", " ".repeat(column.saturating_sub(1)));
    }
    format!(
        "人格文件 `{}` 第 {line} 行第 {column} 列解析失败：{detail}\n出错行：{}\
{caret}\n已改用内置人格（Pai / 副手 / reviewer / support / 用户 / 系统人格），自定义人格本次未加载。修好该文件后重启即可恢复。",
        path.display(),
        line_text.trim_end()
    )
}

/// 逐条解析文件里的人格数组；返回可用条目与「被跳过的第几条」说明。
fn parse_agents_items(items: &[Value]) -> (Vec<AgentProfile>, Vec<String>) {
    let mut agents = Vec::<AgentProfile>::new();
    let mut skipped = Vec::<String>::new();
    for (index, item) in items.iter().enumerate() {
        match serde_json::from_value::<AgentProfile>(item.clone()) {
            Ok(mut agent) => {
                normalize_agent_timestamps(&mut agent);
                agents.push(agent);
            }
            Err(err) => {
                let label = item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("id={value}"))
                    .or_else(|| {
                        item.get("name")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(|value| format!("name={value}"))
                    })
                    .unwrap_or_else(|| "无 id/name".to_string());
                skipped.push(format!("第 {} 条（{label}）：{err}", index + 1));
            }
        }
    }
    (agents, skipped)
}

/// 兜底时先把原文件备份成 `*.corrupt-*`，并把备份结果拼进诊断文案。
///
/// 备份失败（比如磁盘只读）时把该路径登记为「不可信」，后续 `write_agents_shard`
/// 一律拒绝覆盖它——宁可这次改动作废，也不能把用户那份还能手工修的数据抹掉。
fn with_agents_corrupt_backup_note(agents_path: &PathBuf, detail: String) -> String {
    match backup_corrupt_agents_file(agents_path) {
        Ok(backup_path) => format!(
            "{detail}\n原文件已备份为 `{}`，修好原文件后重启即可恢复自定义人格。",
            backup_path.display()
        ),
        Err(err) => {
            mark_agents_file_untrusted(agents_path);
            format!(
                "{detail}\n原文件备份失败（{err}）：本次会话不会再写回该文件，避免覆盖你手工修得好的数据。"
            )
        }
    }
}

/// 内置打底 + 文件同 id 覆盖 + 非内置 id 追加。
///
/// 同 id 覆盖时只做字段级兜底：名字与提示词为空的条目保留代码默认值。
/// 否则一条 `{"id":"default-agent","name":"","systemPrompt":""}` 就能把 Pai 的作用清空，
/// 而文件本身语法完全合法、不会触发任何兜底提示。
fn merge_agents_with_built_in(file_agents: Vec<AgentProfile>) -> Vec<AgentProfile> {
    let mut merged = built_in_agents();
    for mut agent in file_agents {
        let id = agent.id.trim().to_string();
        match merged.iter().position(|existing| existing.id.trim() == id) {
            Some(index) => {
                let default = &merged[index];
                if agent.name.trim().is_empty() {
                    agent.name = default.name.clone();
                }
                if agent.system_prompt.trim().is_empty() {
                    agent.system_prompt = default.system_prompt.clone();
                }
                merged[index] = agent;
            }
            None => merged.push(agent),
        }
    }
    merged
}

/// 人格文件的读取结果。
///
/// `fell_back` 为真表示这次拿到的不是文件里的内容，而是「文件坏了，改用内置人格」的兜底值。
/// 任何由 mtime 驱动的重读都必须看这个标志：兜底值不能拿去覆盖正在使用的缓存或磁盘上的文件。
struct AgentsReadOutcome {
    agents: Vec<AgentProfile>,
    fell_back: bool,
}

impl AgentsReadOutcome {
    fn normal(agents: Vec<AgentProfile>) -> Self {
        Self {
            agents,
            fell_back: false,
        }
    }

    fn fell_back(agents: Vec<AgentProfile>) -> Self {
        Self {
            agents,
            fell_back: true,
        }
    }
}

/// 只要人格列表、不关心这次是不是兜底的读取入口（生产代码一律走 `read_agents_shard_outcome`）。
#[cfg(test)]
fn read_agents_shard(path: &PathBuf) -> Result<Vec<AgentProfile>, String> {
    read_agents_shard_outcome(path).map(|outcome| outcome.agents)
}

/// 读取人格文件，并告诉调用方这次是不是「内置兜底」。
fn read_agents_shard_outcome(path: &PathBuf) -> Result<AgentsReadOutcome, String> {
    let agents_path = app_layout_agents_path(path);
    // 结论先攒在本轮里，读完一次性发布：读取中途不能被别的读取刷掉。
    let mut diagnostics = Vec::<String>::new();
    let outcome = read_agents_file_outcome(&agents_path, &mut diagnostics);
    publish_agents_load_diagnostics(path, &diagnostics);
    Ok(outcome)
}

fn read_agents_file_outcome(
    agents_path: &PathBuf,
    diagnostics: &mut Vec<String>,
) -> AgentsReadOutcome {
    if !agents_path.exists() {
        // 文件不存在：纯内置，与历史行为一致（这是正常状态，不是兜底）。
        return AgentsReadOutcome::normal(built_in_agents());
    }
    let content = match fs::read_to_string(agents_path) {
        Ok(content) => content,
        Err(err) => {
            // 读不出来（ANSI 另存导致的非 UTF-8、被占用、无权限）：同样按兜底处理，
            // 但绝不写回——原文件内容我们连看都没看到过，写回等于凭空覆盖。
            mark_agents_file_untrusted(agents_path);
            record_agents_load_diagnostic(
                diagnostics,
                format!(
                    "读取人格文件失败（{}）：{err}。已改用内置人格，自定义人格本次未加载。\n为避免覆盖尚未读通的原文件，本次会话不会再写回该文件。",
                    agents_path.display()
                ),
            );
            return AgentsReadOutcome::fell_back(built_in_agents());
        }
    };

    let raw = match serde_json::from_str::<Value>(&content) {
        Ok(raw) => raw,
        Err(err) => {
            // 整体语法错（尾逗号 / 括号不闭合）：先备份原文件，再用内置兜底。
            let detail = agents_syntax_error_message(agents_path, &content, &err);
            record_agents_load_diagnostic(
                diagnostics,
                with_agents_corrupt_backup_note(agents_path, detail),
            );
            return AgentsReadOutcome::fell_back(built_in_agents());
        }
    };
    let Some(items) = raw.get("agents").and_then(Value::as_array).cloned() else {
        let detail = format!(
            "人格文件 `{}` 缺少 agents 数组（期望 {{\"agents\": [ ... ]}}），已改用内置人格，自定义人格本次未加载。",
            agents_path.display()
        );
        record_agents_load_diagnostic(
            diagnostics,
            with_agents_corrupt_backup_note(agents_path, detail),
        );
        return AgentsReadOutcome::fell_back(built_in_agents());
    };

    let (file_agents, skipped) = parse_agents_items(&items);
    if !skipped.is_empty() {
        // 坏条被跳过时结果「残缺但合法」，最容易被当成正常读取：缓存被这份残缺内容覆盖，
        // 用户随后任何一次保存都会把跳过的条目永久写没。所以这里同样先备份、同样标兜底，
        // 让活缓存继续保持旧值，等用户把文件改对、mtime 变了再自动恢复。
        let detail = format!(
            "人格文件 `{}` 有 {} 条记录无法解析，本次未加载（其余条目与内置人格照常可用）：{}",
            agents_path.display(),
            skipped.len(),
            skipped.join("；")
        );
        record_agents_load_diagnostic(
            diagnostics,
            with_agents_corrupt_backup_note(agents_path, detail),
        );
        return AgentsReadOutcome::fell_back(merge_agents_with_built_in(file_agents));
    }
    AgentsReadOutcome::normal(merge_agents_with_built_in(file_agents))
}

/// 比较用视图：忽略时间戳，避免「只差 created_at」被当成用户改动而反复写回内置。
fn agent_comparable_value(agent: &AgentProfile) -> Value {
    let mut value = serde_json::to_value(agent).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("createdAt");
        object.remove("updatedAt");
    }
    value
}

/// 只落盘「需要保留的部分」：自定义人格 + 被用户改过的内置人格。
///
/// 纯默认内置人格不再写进文件，避免文件成为内置人格的单一真相来源。
fn agents_to_persist(agents: &[AgentProfile]) -> Vec<AgentProfile> {
    let defaults = built_in_agents();
    let default_ids = defaults
        .iter()
        .map(|agent| agent.id.trim().to_string())
        .collect::<std::collections::HashSet<_>>();
    agents
        .iter()
        .filter(|agent| {
            let id = agent.id.trim();
            if !default_ids.contains(id) {
                // 自定义人格：必须落盘
                return true;
            }
            match defaults.iter().find(|default| default.id.trim() == id) {
                // 与代码默认值不同（用户改过）：落盘
                Some(default) => agent_comparable_value(default) != agent_comparable_value(agent),
                None => true,
            }
        })
        .cloned()
        .collect()
}

fn write_agents_shard(path: &PathBuf, agents: &[AgentProfile]) -> Result<bool, String> {
    let agents_path = app_layout_agents_path(path);
    if agents_file_is_untrusted(&agents_path) {
        // 原文件语法坏且备份失败：宁可这次不改盘，也不覆盖用户还能手工修的数据。
        runtime_log_warn(format!(
            "[配置] 人格文件 {} 本次会话不可信（坏文件备份失败），跳过写回",
            agents_path.display()
        ));
        return Ok(false);
    }
    fs::create_dir_all(app_layout_config_dir(path))
        .map_err(|err| format!("Create config layout dir failed: {err}"))?;
    write_json_file_atomic_if_changed(
        &agents_path,
        &build_agents_file(&agents_to_persist(agents)),
        "agents file",
    )
}

fn system_notification_conversation_shard_has_artifacts(path: &PathBuf) -> Result<bool, String> {
    let store_paths = message_store::message_store_paths(path, SYSTEM_NOTIFICATION_CONVERSATION_ID)?;
    message_store::chat_store_read_status(&store_paths).map(|status| status.is_some())
}

fn ensure_system_notification_conversation_shard(path: &PathBuf) -> Result<bool, String> {
    match read_conversation_shard(path, SYSTEM_NOTIFICATION_CONVERSATION_ID) {
        Ok(mut conversation) => {
            if normalize_system_notification_conversation(&mut conversation) {
                return write_conversation_shard(path, &conversation);
            }
            Ok(false)
        }
        Err(err) => {
            if system_notification_conversation_shard_has_artifacts(path)? {
                runtime_log_warn(format!(
                    "[系统通知会话] 跳过，任务=确保固定会话分片，原因=固定会话分片已存在但暂不可读，conversation_id={}，error={}",
                    SYSTEM_NOTIFICATION_CONVERSATION_ID,
                    err
                ));
                return Ok(false);
            }
            let conversation = build_system_notification_conversation_record();
            write_conversation_shard(path, &conversation)
        }
    }
}

fn build_chat_index_item(conversation: &Conversation) -> ChatIndexConversationItem {
    ChatIndexConversationItem {
        id: conversation.id.clone(),
        updated_at: conversation.updated_at.clone(),
        status: conversation.status.clone(),
        archived_at: conversation.archived_at.clone(),
    }
}

fn chat_index_item_is_archived(item: &ChatIndexConversationItem) -> bool {
    if item.status.trim() == "archived" {
        return true;
    }
    item.archived_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
}

#[cfg(test)]
fn build_chat_index_file(conversations: &[Conversation]) -> ChatIndexFile {
    ChatIndexFile {
        conversations: conversations
            .iter()
            .map(build_chat_index_item)
            .collect::<Vec<_>>(),
    }
}

fn upsert_chat_index_conversation(index: &mut ChatIndexFile, conversation: &Conversation) {
    let next = build_chat_index_item(conversation);
    if let Some(existing) = index
        .conversations
        .iter_mut()
        .find(|item| item.id == conversation.id)
    {
        *existing = next;
    } else {
        index.conversations.push(next);
    }
}

#[allow(dead_code)]
fn remove_chat_index_conversation(index: &mut ChatIndexFile, conversation_id: &str) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    index.conversations.retain(|item| item.id != conversation_id);
}

fn read_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<Conversation, String> {
    let mut conversation = read_conversation_shard_raw(path, conversation_id)?;
    normalize_conversation_runtime_volatile_fields(&mut conversation);
    Ok(conversation)
}

fn read_conversation_meta_shard(
    path: &PathBuf,
    conversation_id: &str,
) -> Result<message_store::ConversationShardMeta, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    match message_store::chat_store_read_meta(&store_paths) {
        Ok(Some(meta))
            if meta.schema_version() >= message_store::CONVERSATION_META_SCHEMA_VERSION =>
        {
            if meta.cumulative_usage().needs_legacy_total_tokens_backfill() {
                let repaired = meta.clone().normalized_legacy_usage_totals();
                write_conversation_meta_shard_from_meta(path, &repaired)?;
                return Ok(repaired);
            }
            return Ok(meta);
        }
        Ok(Some(_)) | Ok(None) | Err(_) => {}
    }
    Err(format!("Conversation '{conversation_id}' not found."))
}

fn refresh_conversation_meta_shard_if_needed(
    path: &PathBuf,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    if let Ok(Some(meta)) = message_store::chat_store_read_meta(&store_paths) {
        if meta.schema_version() >= message_store::CONVERSATION_META_SCHEMA_VERSION {
            return Ok(false);
        }
    }
    if message_store::chat_store_read_status(&store_paths)?.is_none() {
        return Ok(false);
    }
    let _ = read_conversation_meta_shard(path, conversation_id)?;
    Ok(true)
}

fn read_conversation_shard_raw(path: &PathBuf, conversation_id: &str) -> Result<Conversation, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("Conversation id is empty".to_string());
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    if let Some(conversation) =
        message_store::chat_store_read_conversation(&store_paths)?
    {
        if conversation
            .cumulative_usage
            .needs_legacy_total_tokens_backfill()
        {
            let mut repaired = conversation;
            repaired.cumulative_usage = repaired.cumulative_usage.clone().normalized_legacy_totals();
            let _ = write_conversation_shard(path, &repaired)?;
            return Ok(repaired);
        }
        return Ok(conversation);
    }
    Err(format!("Conversation '{conversation_id}' not found."))
}

fn write_conversation_shard(path: &PathBuf, conversation: &Conversation) -> Result<bool, String> {
    fs::create_dir_all(app_layout_chat_conversations_dir(path))
        .map_err(|err| format!("Create chat conversations dir failed: {err}"))?;
    let store_paths = message_store::message_store_paths(path, &conversation.id)?;
    if message_store::chat_store_read_meta(&store_paths)?.is_some() {
        // v3 的正文只能由 append/replace/truncate/splice 原子接口发布。
        // 后台 metadata 刷新不得整读或重建 locator 与 JSONL block。
        write_conversation_meta_shard_from_meta(
            path,
            &message_store::ConversationShardMeta::from_conversation(conversation),
        )?;
        return Ok(true);
    }
    message_store::chat_store_write_snapshot(&store_paths, conversation)?;
    Ok(true)
}

fn write_conversation_meta_shard_from_meta(
    path: &PathBuf,
    meta: &message_store::ConversationShardMeta,
) -> Result<(), String> {
    let paths = message_store::message_store_paths(path, meta.id())?;
    let mut meta_to_persist = meta.clone();
    if let Some(ready_meta) = message_store::chat_store_read_meta(&paths)? {
        meta_to_persist.preserve_message_derived_fields_from(&ready_meta);
    }
    let persist_meta = meta_to_persist.to_persist_meta();
    message_store::chat_store_write_meta(&paths, &persist_meta)
}

fn delete_conversation_shard(path: &PathBuf, conversation_id: &str) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let store_paths = message_store::message_store_paths(path, conversation_id)?;
    message_store::delete_message_store_shard_artifacts(&store_paths)
}

fn read_json_file<T>(path: &PathBuf, label: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|err| format!("Read app_data failed: {err}"))?;
    serde_json::from_str::<T>(&content).map_err(|err| {
        runtime_log_error(format!("[配置] 解析{label}失败 ({}): {err}", path.display()));
        format!("Parse {label} failed ({}): {err}", path.display())
    })
}

fn file_metadata_signature(path: &PathBuf) -> (u64, Option<std::time::SystemTime>) {
    match fs::metadata(path) {
        Ok(metadata) => (metadata.len(), metadata.modified().ok()),
        Err(_) => (0, None),
    }
}

fn app_data_cache_signature(path: &PathBuf) -> AppDataCacheSignature {
    let agents_path = app_layout_agents_path(path);
    let runtime_path = app_layout_runtime_state_path(path);
    let (agents_len, agents_modified) = file_metadata_signature(&agents_path);
    let (runtime_len, runtime_modified) = file_metadata_signature(&runtime_path);
    // 普通运行时缓存只随 V3 current 数据库变化；不能用 V1 JSON 或 V2
    // manifest/meta/index/blocks 的文件状态驱动缓存，从而间接恢复旧格式。
    let chat_metadata_path = message_store::chat_metadata_store_db_path(path);
    let (chat_metadata_len, chat_metadata_modified) =
        file_metadata_signature(&chat_metadata_path);
    let conversations = ConversationDirCacheSignature {
        file_count: u64::from(chat_metadata_modified.is_some()),
        total_size: chat_metadata_len,
        latest_file_name: "chat_metadata.sqlite".to_string(),
        latest_modified: chat_metadata_modified,
    };

    AppDataCacheSignature {
        agents_len,
        agents_modified,
        runtime_len,
        runtime_modified,
        conversations,
    }
}

#[cfg(test)]
fn write_json_file_atomic<T>(path: &PathBuf, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize,
{
    ensure_parent_dir(path)?;
    let body = serde_json::to_vec_pretty(value).map_err(|err| format!("Serialize {label} failed: {err}"))?;
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid {label} file path"))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, body).map_err(|err| format!("Write temp {label} failed: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp, path) {
        fs::copy(&tmp, path).map_err(|copy_err| {
            format!(
                "Finalize {label} failed (rename: {rename_err}; copy: {copy_err})"
            )
        })?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

fn write_json_file_atomic_if_changed<T>(
    path: &PathBuf,
    value: &T,
    label: &str,
) -> Result<bool, String>
where
    T: Serialize,
{
    ensure_parent_dir(path)?;
    let body = serde_json::to_vec_pretty(value).map_err(|err| format!("Serialize {label} failed: {err}"))?;
    if let Ok(existing) = fs::read(path) {
        if existing == body {
            return Ok(false);
        }
    }
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| format!("Invalid {label} file path"))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, body).map_err(|err| format!("Write temp {label} failed: {err}"))?;
    if let Err(rename_err) = fs::rename(&tmp, path) {
        fs::copy(&tmp, path).map_err(|copy_err| {
            format!(
                "Finalize {label} failed (rename: {rename_err}; copy: {copy_err})"
            )
        })?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(true)
}

#[cfg(test)]
fn read_layout_app_data(path: &PathBuf) -> Result<AppData, String> {
    let agents = if app_layout_agents_path(path).exists() {
        read_json_file::<AgentsFile>(&app_layout_agents_path(path), "agents file")?.agents
    } else {
        AppData::default().agents
    };

    let data_migration_version =
        state_db_get_kv(path, "data_migration_version")?
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(0);

    Ok(AppData {
        version: APP_DATA_SCHEMA_VERSION,
        data_migration_version,
        agents,
        user_alias: default_user_alias(),
        conversations: Vec::new(),
    })
}

// ========== 数据迁移 registry ==========
//
// v2+ 需要显式上下文，避免在只有 app_data 路径时猜测助理空间位置或名称。
struct DataMigrationContext<'a> {
    state: &'a AppState,
    config: &'a AppConfig,
}

#[derive(Debug, Default, Clone, Copy)]
struct DataMigrationStepStats {
    data_changed: bool,
    conversation_writes: usize,
    /// 这一步没能真正执行（前置数据不可读，比如人格文件损坏），版本必须停在原处等下次重跑。
    /// 不推迟就会出现「版本推进了、迁移没做」的永久空洞：用户修好文件重启后也不会再跑。
    deferred: bool,
}

struct DataMigrationStep {
    version: u32,
    name: &'static str,
    run: for<'a> fn(&DataMigrationContext<'a>) -> Result<DataMigrationStepStats, String>,
}

fn data_migration_steps() -> Vec<DataMigrationStep> {
    vec![
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES,
            name: "v2_assistant_workspace_for_empty_shell_workspaces",
            run: migrate_empty_shell_workspaces_to_assistant_workspace,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V5_DEPARTMENTS_TO_AGENT_ORGANIZATION,
            name: "v5_departments_to_agent_organization",
            run: migrate_departments_into_agent_organization,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V6_AVATAR_PATH_RELATIVE,
            name: "v6_avatar_path_relative",
            run: migrate_avatar_paths_to_relative,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V7_REMOVE_HR_PERSONA,
            name: "v7_remove_hr_persona",
            run: migrate_remove_hr_persona,
        },
        DataMigrationStep {
            version: DATA_MIGRATION_VERSION_V8_DEPUTY_NOT_SYSTEM,
            name: "v8_deputy_not_system",
            run: migrate_deputy_not_system,
        },
    ]
}

/// V6 迁移步骤：把 `agents.json` 里的绝对头像路径归一为相对数据根的相对路径。
/// - 已是相对路径：跳过（幂等）。
/// - 绝对路径且当前 avatars 目录存在同名文件：改写为 `avatars/<文件名>`。
/// - 找不到同名文件：保留原值并告警，不做无声抹除。
fn migrate_avatar_paths_to_relative(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let mut stats = DataMigrationStepStats::default();
    let read = read_agents_shard_outcome(&context.state.data_path)?;
    if read.fell_back {
        // 文件坏了、这次拿到的是内置兜底值：拿它写回会把用户的自定义人格从原文件抹掉。
        // 这一版迁移没做成，版本必须停住等下次重跑，不能记成「已完成」。
        stats.deferred = true;
        runtime_log_warn(
            "[应用数据迁移] 头像路径迁移推迟，任务=v6头像相对路径，原因=人格文件损坏已用内置兜底，跳过写回以保留原文件".to_string(),
        );
        return Ok(stats);
    }
    let mut agents = read.agents;
    let avatars_dir = avatar_storage_dir(context.state)?;
    let mut changed = false;
    let mut rewritten = 0usize;
    let mut kept = 0usize;
    for agent in agents.iter_mut() {
        let Some(raw) = agent
            .avatar_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let candidate = PathBuf::from(raw);
        if !candidate.is_absolute() {
            continue;
        }
        let file_name = candidate
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !file_name.is_empty() && avatars_dir.join(file_name).is_file() {
            agent.avatar_path = Some(avatar_relative_path(file_name));
            changed = true;
            rewritten += 1;
        } else {
            kept += 1;
            runtime_log_warn(format!(
                "[应用数据迁移] 头像路径无法归一，任务=v6头像相对路径，人格={}，原值={}，原因=当前头像目录未找到同名文件",
                agent.id, raw
            ));
        }
    }
    if changed {
        if !write_agents_shard(&context.state.data_path, &agents)? {
            // 写盘被拒（原文件不可信）：同样算这一步没做成，版本必须停住。
            stats.deferred = true;
            runtime_log_warn(
                "[应用数据迁移] 推迟，原因=人格文件不可信（坏文件备份失败），本次不写回".to_string(),
            );
            return Ok(stats);
        }
        stats.data_changed = true;
    }
    runtime_log_info(format!(
        "[应用数据迁移] 头像路径迁移完成，任务=v6头像相对路径，改写={rewritten}，保留={kept}"
    ));
    Ok(stats)
}

/// V7 迁移步骤：移除已废弃的 HR 人格，并从各人格的下级列表里摘除对它的引用。
/// HR 机制已整体下线（人格、常驻 skill、招募入口一并删除），但历史数据里可能已经落下
/// 一个 `hr` 人格节点，且主助理的下级列表里含 `hr`，需要一并清理，
/// 否则组织图会残留一个已失效的成员、根人格的下级也指向空节点。
/// 幂等：既无 `hr` 节点、也无对它的引用时不写盘。
fn migrate_remove_hr_persona(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    /// 已废弃的 HR 人格 id；仅本迁移使用，业务代码不再感知。
    const LEGACY_HR_AGENT_ID: &str = "hr";
    let mut stats = DataMigrationStepStats::default();
    let read = read_agents_shard_outcome(&context.state.data_path)?;
    if read.fell_back {
        stats.deferred = true;
        runtime_log_warn(
            "[应用数据迁移] HR 人格清理推迟，任务=v7移除HR人格，原因=人格文件损坏已用内置兜底，跳过写回以保留原文件".to_string(),
        );
        return Ok(stats);
    }
    let mut agents = read.agents;
    let mut pruned_refs = 0usize;
    for agent in agents.iter_mut() {
        let before = agent.child_agent_ids.len();
        agent
            .child_agent_ids
            .retain(|child| child.trim() != LEGACY_HR_AGENT_ID);
        pruned_refs += before - agent.child_agent_ids.len();
    }
    let removed_nodes = agents
        .iter()
        .filter(|agent| agent.id == LEGACY_HR_AGENT_ID)
        .count();
    agents.retain(|agent| agent.id != LEGACY_HR_AGENT_ID);
    if removed_nodes > 0 || pruned_refs > 0 {
        if !write_agents_shard(&context.state.data_path, &agents)? {
            // 写盘被拒（原文件不可信）：同样算这一步没做成，版本必须停住。
            stats.deferred = true;
            runtime_log_warn(
                "[应用数据迁移] 推迟，原因=人格文件不可信（坏文件备份失败），本次不写回".to_string(),
            );
            return Ok(stats);
        }
        stats.data_changed = true;
        runtime_log_info(format!(
            "[应用数据迁移] HR 人格清理完成，任务=v7移除HR人格，删除节点={removed_nodes}，摘除下级引用={pruned_refs}"
        ));
    }
    Ok(stats)
}

/// V8 迁移步骤：把内置副手人格（`deputy-agent`）的 `is_built_in_system` 从 true 纠正为 false。
/// 副手是内置组织成员，不是「只做系统播报、不在选择器出现」的系统人格；早期默认构造把它错标为系统，
/// 导致委托模型配置卡片等处被 `!is_built_in_system` 误拦。只纠正这一个已知的错标节点。
/// 幂等：副手已是 false 时不写盘。
fn migrate_deputy_not_system(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let mut stats = DataMigrationStepStats::default();
    let read = read_agents_shard_outcome(&context.state.data_path)?;
    if read.fell_back {
        stats.deferred = true;
        runtime_log_warn(
            "[应用数据迁移] 副手人格标记纠正推迟，任务=v8副手非系统，原因=人格文件损坏已用内置兜底，跳过写回以保留原文件".to_string(),
        );
        return Ok(stats);
    }
    let mut agents = read.agents;
    let mut corrected = 0usize;
    for agent in agents.iter_mut() {
        if agent.id.trim() == DEPUTY_AGENT_ID && agent.is_built_in_system {
            agent.is_built_in_system = false;
            corrected += 1;
        }
    }
    if corrected > 0 {
        if !write_agents_shard(&context.state.data_path, &agents)? {
            // 写盘被拒（原文件不可信）：同样算这一步没做成，版本必须停住。
            stats.deferred = true;
            runtime_log_warn(
                "[应用数据迁移] 推迟，原因=人格文件不可信（坏文件备份失败），本次不写回".to_string(),
            );
            return Ok(stats);
        }
        stats.data_changed = true;
        runtime_log_info(format!(
            "[应用数据迁移] 副手人格标记纠正完成，任务=v8副手非系统，agent_id={DEPUTY_AGENT_ID}，is_built_in_system=true→false，纠正节点={corrected}"
        ));
    }
    Ok(stats)
}

fn conversation_shell_workspace_path_key(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        String::new()
    } else {
        normalize_terminal_path_for_compare(&PathBuf::from(path))
    }
}

fn legacy_shell_workspace_path_as_main_workspace(
    state: &AppState,
    path: &str,
) -> Option<ShellWorkspaceConfig> {
    let raw = ShellWorkspaceConfig {
        id: "legacy-main-workspace".to_string(),
        name: String::new(),
        path: path.trim().to_string(),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
        built_in: false,
    };
    let candidate = shell_workspace_resolve_path_candidate(state, &raw)?;
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    normalize_conversation_shell_workspaces(
        state,
        &[ShellWorkspaceConfig {
            path: terminal_path_for_user(&canonical),
            ..raw
        }],
    )
    .into_iter()
    .next()
}

fn state_write_conversation_shell_workspace_metadata_direct(
    state: &AppState,
    conversation_id: &str,
    shell_workspaces: Vec<ShellWorkspaceConfig>,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let mutation_gate = conversation_mutation_gate(&state.data_path, conversation_id)?;
    let _guard = mutation_gate.lock().map_err(|err| {
        named_lock_error("conversation_mutation_gate", file!(), line!(), module_path!(), &err)
    })?;
    let conversation_meta = state_read_conversation_metadata_cached(state, conversation_id)?;
    let mut conversation =
        conversation_service_v2().build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
    let original_path = conversation.shell_workspace_path.clone();
    let original_workspaces = conversation.shell_workspaces.clone();
    conversation.shell_workspace_path = None;
    conversation.shell_workspaces = shell_workspaces;
    if conversation.shell_workspace_path == original_path
        && conversation.shell_workspaces == original_workspaces
    {
        return Ok(false);
    }
    let mut updated_meta = message_store::ConversationShardMeta::from_conversation(&conversation);
    updated_meta.preserve_message_derived_fields_from(&conversation_meta);
    write_conversation_meta_shard_from_meta(&state.data_path, &updated_meta)?;
    let _ = state_mark_conversation_metadata_direct_persisted(state, conversation_id)?;
    Ok(true)
}

fn shell_workspaces_for_empty_conversation_workspace_migration(
    state: &AppState,
    config: &AppConfig,
    conversation: &Conversation,
) -> Option<Vec<ShellWorkspaceConfig>> {
    let normalized = normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
    if !normalized.is_empty() {
        return None;
    }
    if let Some(legacy_workspace) = conversation
        .shell_workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|path| legacy_shell_workspace_path_as_main_workspace(state, path))
    {
        return Some(vec![legacy_workspace]);
    }
    Some(vec![assistant_workspace_as_conversation_main_workspace(
        state, config,
    )])
}

fn migrate_empty_shell_workspaces_to_assistant_workspace(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let chat_index = collect_chat_index_items_from_storage(&context.state.data_path)?;
    let mut stats = DataMigrationStepStats::default();
    for item in chat_index {
        let conversation_id = item.id.trim();
        if conversation_id.is_empty() {
            continue;
        }
        let conversation_meta =
            match state_read_conversation_metadata_cached(context.state, conversation_id) {
                Ok(meta) => meta,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[应用数据迁移] 跳过，任务=v2补齐会话工作区，conversation_id={}，error={}",
                        conversation_id, err
                    ));
                    continue;
                }
            };
        let conversation = conversation_service_v2()
            .build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
        if !conversation_visible_in_foreground_lists(&conversation)
            || !conversation_is_local_normal_chat(&conversation)
            || !conversation_is_unarchived(&conversation)
        {
            continue;
        }
        let Some(shell_workspaces) = shell_workspaces_for_empty_conversation_workspace_migration(
            context.state,
            context.config,
            &conversation,
        ) else {
            continue;
        };
        if state_write_conversation_shell_workspace_metadata_direct(
            context.state,
            conversation_id,
            shell_workspaces,
        )? {
            stats.data_changed = true;
            stats.conversation_writes += 1;
        }
    }
    Ok(stats)
}

fn run_app_data_migrations_with_state(
    state: &AppState,
    config: &AppConfig,
) -> Result<bool, String> {
    require_message_store_migration_completed_for_runtime(state, "执行应用数据迁移")?;
    let mut migration_version = state_service_get_data_migration_version(state)?;
    if migration_version >= DATA_MIGRATION_CURRENT_VERSION {
        return Ok(false);
    }
    let migration_version_before = migration_version;
    let mut any_data_changed = false;
    let mut deferred_step: Option<&'static str> = None;
    for step in data_migration_steps() {
        if migration_version >= step.version {
            continue;
        }
        let started = std::time::Instant::now();
        let stats = (step.run)(&DataMigrationContext { state, config })?;
        if stats.deferred {
            // 这一步没做成：版本停在上一版，后面的步骤也一起推迟。
            // 继续往后跑会留下「半迁移」状态（比如 V5 的配置键指向不存在的人格）。
            runtime_log_warn(format!(
                "[应用数据迁移] 推迟，任务={}，migration_version 保持 {}，原因=这一步的前置数据当前不可用，修好后重启会重跑",
                step.name, migration_version
            ));
            deferred_step = Some(step.name);
            break;
        }
        migration_version = step.version;
        any_data_changed |= stats.data_changed;
        runtime_log_info(format!(
            "[应用数据迁移] 完成，任务={}，migration_version_before={}，migration_version_after={}，data_changed={}，conversation_writes={}，duration_ms={}",
            step.name,
            migration_version_before,
            migration_version,
            stats.data_changed,
            stats.conversation_writes,
            started.elapsed().as_millis()
        ));
    }
    if deferred_step.is_none() && migration_version < DATA_MIGRATION_CURRENT_VERSION {
        migration_version = DATA_MIGRATION_CURRENT_VERSION;
    }
    state_service_set_data_migration_version(state, migration_version)?;
    Ok(any_data_changed || migration_version_before != migration_version)
}

fn assistant_workspace_label_sync_target_keys(
    state: &AppState,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> std::collections::HashSet<String> {
    let mut previous = previous_config.clone();
    let mut next = next_config.clone();
    let _ = ensure_default_shell_workspace_in_config(&mut previous, state);
    let _ = ensure_default_shell_workspace_in_config(&mut next, state);
    [
        assistant_workspace_as_conversation_main_workspace(state, &previous),
        assistant_workspace_as_conversation_main_workspace(state, &next),
    ]
    .into_iter()
    .map(|workspace| conversation_shell_workspace_path_key(&workspace.path))
    .filter(|key| !key.is_empty())
    .collect()
}

fn sync_assistant_workspace_label_for_unarchived_conversations(
    state: &AppState,
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> Result<usize, String> {
    let target_keys =
        assistant_workspace_label_sync_target_keys(state, previous_config, next_config);
    if target_keys.is_empty() {
        return Ok(0);
    }
    let assistant_workspace =
        assistant_workspace_as_conversation_main_workspace(state, next_config);
    let chat_index = collect_chat_index_items_from_storage(&state.data_path)?;
    let mut changed = 0usize;
    for item in chat_index {
        let conversation_id = item.id.trim();
        if conversation_id.is_empty() {
            continue;
        }
        let conversation_meta = match state_read_conversation_metadata_cached(state, conversation_id)
        {
            Ok(meta) => meta,
            Err(err) => {
                runtime_log_warn(format!(
                    "[终端工作空间] 跳过，任务=同步助理空间会话标签，conversation_id={}，error={}",
                    conversation_id, err
                ));
                continue;
            }
        };
        let conversation = conversation_service_v2()
            .build_conversation_snapshot_from_meta(&conversation_meta, Vec::new());
        if !conversation_visible_in_foreground_lists(&conversation)
            || !conversation_is_local_normal_chat(&conversation)
            || !conversation_is_unarchived(&conversation)
        {
            continue;
        }
        let mut workspaces =
            normalize_conversation_shell_workspaces(state, &conversation.shell_workspaces);
        if workspaces.is_empty() {
            workspaces = vec![assistant_workspace.clone()];
        } else {
            let main_index = workspaces
                .iter()
                .position(|workspace| {
                    normalize_shell_workspace_level_text(&workspace.level)
                        == SHELL_WORKSPACE_LEVEL_MAIN
                })
                .unwrap_or(0);
            let key = conversation_shell_workspace_path_key(&workspaces[main_index].path);
            if !target_keys.contains(&key) {
                continue;
            }
            let mut synced = workspaces[main_index].clone();
            synced.name = assistant_workspace.name.clone();
            synced.path = assistant_workspace.path.clone();
            synced.level = SHELL_WORKSPACE_LEVEL_MAIN.to_string();
            synced.built_in = false;
            if normalize_shell_workspace_access_text(&synced.access).is_empty() {
                synced.access = assistant_workspace.access.clone();
            }
            workspaces[main_index] = synced;
        }
        if state_write_conversation_shell_workspace_metadata_direct(
            state,
            conversation_id,
            workspaces,
        )? {
            changed += 1;
        }
    }
    if changed > 0 {
        runtime_log_info(format!(
            "[终端工作空间] 完成，任务=同步助理空间会话标签，conversation_count={}",
            changed
        ));
    }
    Ok(changed)
}

fn normalize_conversation_runtime_volatile_fields(conversation: &mut Conversation) {
    let _ = fill_missing_conversation_message_speaker_agent_ids(conversation);
    let _ = cleanup_legacy_summary_context_messages(conversation);
}

/// 测试专用：从聚合 AppData 结构 seed 分片布局（agents.json + conversation shards + 系统通知会话）。
/// 替代已删除的兼容层全量写入器，供测试构造使用。
#[cfg(test)]
fn seed_app_data_shards(path: &PathBuf, data: &AppData) -> Result<(), String> {
    write_agents_shard(path, &data.agents)?;
    if data.data_migration_version > 0 {
        state_db_upsert_kv(
            path,
            "data_migration_version",
            &data.data_migration_version.to_string(),
        )?;
    }
    for conv in &data.conversations {
        write_conversation_shard(path, conv)?;
    }
    ensure_system_notification_conversation_shard(path)?;
    Ok(())
}
