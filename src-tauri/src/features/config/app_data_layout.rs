const LAYOUT_DIR_CONFIG: &str = "config";
const LAYOUT_DIR_STATE: &str = "state";
const LAYOUT_DIR_CHAT: &str = "chat";
const LAYOUT_DIR_CHAT_CONVERSATIONS: &str = "conversations";
const LAYOUT_DIR_BACKUPS: &str = "backups";
const LAYOUT_DIR_AGENTS: &str = "agents";
const LAYOUT_FILE_RUNTIME: &str = "runtime_state.json";

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

fn app_layout_runtime_state_path(path: &PathBuf) -> PathBuf {
    app_layout_state_dir(path).join(LAYOUT_FILE_RUNTIME)
}

fn app_layout_chat_conversation_path(path: &PathBuf, conversation_id: &str) -> PathBuf {
    app_layout_chat_conversations_dir(path).join(format!("{conversation_id}.json"))
}

/// 人格目录：`config/agents/`，一人格一文件。
fn app_layout_agents_dir(path: &PathBuf) -> PathBuf {
    app_layout_config_dir(path).join(LAYOUT_DIR_AGENTS)
}

/// 人格 id → 文件名主干：只保留字母数字与 `-` / `_`，其余一律替换成 `_`。
/// 人格 id 来自用户输入，直接当文件名会拼出目录穿越或非法字符。
///
/// 有字符被替换时追加一段 id 哈希：`a:b` 与 `a_b` 清洗后都会变成 `a_b`，
/// 不区分的话两个人格会写到同一个文件，后写的把先写的覆盖掉、静默丢一个。
fn sanitize_agent_file_stem(id: &str) -> String {
    let trimmed = id.trim();
    let stem = trimmed
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if stem.is_empty() {
        return "agent".to_string();
    }
    if stem == trimmed {
        stem
    } else {
        format!("{stem}-{:08x}", short_id_hash(trimmed))
    }
}

/// 人格 id 的稳定短哈希，只用于消解清洗后的文件名冲突。
fn short_id_hash(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish() & 0xffff_ffff
}

/// 某个人格在目录里的文件路径。
fn app_layout_agent_file_path(path: &PathBuf, id: &str) -> PathBuf {
    app_layout_agents_dir(path).join(format!("{}.json", sanitize_agent_file_stem(id)))
}

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

/// 补齐后没有名字时用的占位名：空串在人格列表里会变成一行看不见的东西。
const UNNAMED_AGENT_NAME: &str = "未命名人格";

/// 读入后的结构补齐：id 与时间戳。
///
/// 名字留到 `merge_agents_with_built_in` 之后补——先补名字会盖掉内置人格的默认名，
/// 让「用空串覆盖内置」这种写法又变成合法覆盖。返回内容是否被改动过。
fn normalize_agent(agent: &mut AgentProfile) -> bool {
    let mut changed = false;

    let id = agent.id.trim().to_string();
    if id.is_empty() {
        agent.id = format!("agent-{}", uuid::Uuid::new_v4());
        changed = true;
    } else if id != agent.id {
        agent.id = id;
        changed = true;
    }
    // systemPrompt 空串是合法状态（人格可以一个提示词都没有），不编内容、也不算缺失。
    // 其余字段的 `#[serde(default)]` 已经给出合法默认值（记忆模式、系统准则、
    // 全局 scope 都有专门 default 函数），不需要在这里再补。

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
    if agent.created_at != created {
        agent.created_at = created;
        changed = true;
    }
    if agent.updated_at != updated {
        agent.updated_at = updated;
        changed = true;
    }
    changed
}

/// 还没有名字的人格给一个可读占位名（空串在列表里会变成一行看不见的东西）。
fn apply_default_agent_name(agent: &mut AgentProfile) -> bool {
    if !agent.name.trim().is_empty() {
        return false;
    }
    agent.name = UNNAMED_AGENT_NAME.to_string();
    true
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

/// 记录一条本轮读取的诊断：写 error 日志 + 收进本轮结论，绝不让损坏静默发生。
fn record_agents_load_diagnostic(messages: &mut Vec<String>, message: String) {
    runtime_log_error(format!("[配置] {message}"));
    messages.push(message);
}

/// 人格文件的宽松语法：**JSON + 注释 + 尾逗号**（JSONC）。
///
/// 其余扩展（单引号、裸键名、缺逗号、十六进制）一律不放宽，避免被 JSON5 的额外语法带偏：
/// 放开得越多，真正写错时越难给出「这一行这里错了」的提示。
/// 救援专用：能放多宽放多宽，尽量把用户的东西救回来（救回来之后我们自己重写成标准 JSON）。
/// 把语法错翻成「文件 + 行列 + 出错行原文 + 指位」的中文提示。
/// 值类型的中文说明，用来写清「这条为什么救不回来」。
/// 逐条解析文件里的人格数组。
///
/// 返回：可用条目、被跳过的说明、以及是否有内容被补齐过（补齐过就必须写回）。
/// 只有**类型不对**（`name: 123`）与**结构不对**（条目不是对象）才算跳过；
/// 字段缺失一律由 `normalize_agent` 补齐，不再算坏条。
fn parse_agents_items(items: &[Value]) -> (Vec<AgentProfile>, Vec<String>, bool) {
    let mut agents = Vec::<AgentProfile>::new();
    let mut skipped = Vec::<String>::new();
    let mut normalized = false;
    for (index, item) in items.iter().enumerate() {
        if !item.is_object() {
            skipped.push(format!(
                "第 {} 条是{}而不是对象",
                index + 1,
                json_value_type_label(item)
            ));
            continue;
        }
        match serde_json::from_value::<AgentProfile>(item.clone()) {
            Ok(mut agent) => {
                normalized |= normalize_agent(&mut agent);
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
                skipped.push(format!("第 {} 条（{label}）字段类型不对：{err}", index + 1));
            }
        }
    }
    (agents, skipped, normalized)
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

/// 从解析出来的值里取出人格条目。
///
/// 兼容两种常见手写形态：顶层直接是数组（少写了 `{"agents": ...}` 外壳）、
/// `agents` 写成了单个对象。都不是致命错误，能读就读。
fn agents_items_from_value(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::Array(items) => Some(items.clone()),
        Value::Object(map) => match map.get("agents") {
            Some(Value::Array(items)) => Some(items.clone()),
            Some(other @ Value::Object(_)) => Some(vec![other.clone()]),
            _ => None,
        },
        _ => None,
    }
}

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

/// 读取人格目录：`config/agents/*.json`，一人格一文件。
///
/// 逐文件容错：某个文件读不出来或 JSON 坏掉，只跳过它并记一条诊断，其余人格照常加载。
/// 读是读、写是写：一个坏文件既不该带走其他人格，也不该影响后续写盘。
fn read_agents_dir_outcome(path: &PathBuf) -> Result<Vec<AgentProfile>, String> {
    let agents_dir = app_layout_agents_dir(path);
    let mut diagnostics = Vec::<String>::new();
    let mut file_agents = Vec::<AgentProfile>::new();
    let mut entries = match fs::read_dir(&agents_dir) {
        Ok(entries) => entries
            .filter_map(|item| item.ok())
            .map(|item| item.path())
            .filter(|item| {
                item.is_file()
                    && item
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            })
            .collect::<Vec<_>>(),
        // 目录还不存在＝还没有任何自定义人格，纯内置，属正常状态。
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => {
            return Err(format!(
                "读取人格目录失败（{}）：{err}",
                agents_dir.display()
            ))
        }
    };
    // 目录列举顺序不稳定；按文件名排一遍，让同一条坏数据每次落在同一个位置。
    entries.sort();

    for file_path in entries {
        let content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(err) => {
                record_agents_load_diagnostic(
                    &mut diagnostics,
                    format!(
                        "人格文件读取失败（{}）：{err}。已跳过该文件，其余人格照常加载。",
                        file_path.display()
                    ),
                );
                continue;
            }
        };
        let options = agents_jsonc_options();
        let parsed = match parse_agents_jsonc(&content, &options) {
            Ok(value) => value,
            Err(err) => {
                record_agents_load_diagnostic(
                    &mut diagnostics,
                    format!(
                        "人格文件 JSON 无效（{}）：{}。已跳过该文件，其余人格照常加载。",
                        file_path.display(),
                        err.detail
                    ),
                );
                continue;
            }
        };
        // 一个文件就是一个人格；写成数组或 `{"agents": [...]}` 也照收，不因此报错。
        let items = match agents_items_from_value(&parsed) {
            Some(items) => items,
            // 没有 `agents` 外壳、本身就是人格对象：这就是最常规的形态。
            None if parsed.is_object() => vec![parsed],
            None => {
                record_agents_load_diagnostic(
                    &mut diagnostics,
                    format!(
                        "人格文件结构无法识别（{}）：既不是人格对象也不是数组。已跳过该文件。",
                        file_path.display()
                    ),
                );
                continue;
            }
        };
        let (parsed_agents, item_diagnostics, _) = parse_agents_items(&items);
        for message in item_diagnostics {
            record_agents_load_diagnostic(
                &mut diagnostics,
                format!("{}（{}）", message, file_path.display()),
            );
        }
        file_agents.extend(parsed_agents);
    }

    publish_agents_load_diagnostics(path, &diagnostics);
    Ok(merge_agents_with_built_in(file_agents))
}

/// 写人格目录：一人格一文件，只落盘「自定义人格 + 被用户改过的内置人格」。
///
/// 每次只动真正变了的那一个文件（内容相同就不写），不整目录重写：
/// 改一个人格不该连带刷新其他所有人格的 mtime。
///
/// 同时删掉目录里不该再存在的人格文件——分文件写没有整文件覆盖那种天然删除，
/// 不补这一步的话，删掉的人格会在下次读取时复活，改回默认的内置人格也改不回去。
fn write_agents_dir(path: &PathBuf, agents: &[AgentProfile]) -> Result<bool, String> {
    let agents_dir = app_layout_agents_dir(path);
    fs::create_dir_all(&agents_dir)
        .map_err(|err| format!("Create agents layout dir failed: {err}"))?;
    let to_persist = agents_to_persist(agents);
    let keep = to_persist
        .iter()
        .map(|agent| format!("{}.json", sanitize_agent_file_stem(&agent.id)))
        .collect::<std::collections::HashSet<_>>();
    let mut changed = false;
    if let Ok(entries) = fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }
            let is_json = file_path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
            if !is_json {
                continue;
            }
            let file_name = file_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            if keep.contains(&file_name) {
                continue;
            }
            // 目录里这一份已经不在人格列表里：删掉它，否则下次读取会把它带回来。
            //
            // 删除失败只记日志、不中断保存——文件被别的程序短暂占用时，
            // 「这次没删掉」不该让用户整次保存失败。下次保存会再试一次。
            if let Err(err) = fs::remove_file(&file_path) {
                runtime_log_warn(format!(
                    "[配置] 删除过期人格文件失败，路径={}，error={err}",
                    file_path.display()
                ));
                continue;
            }
            changed = true;
        }
    }
    for agent in to_persist {
        let file_path = app_layout_agent_file_path(path, &agent.id);
        changed |= write_json_file_atomic_if_changed(&file_path, &agent, "agent file")?;
    }
    Ok(changed)
}

/// 人格目录的内容时间戳：目录自身与其中每个文件的 mtime 里取最新那个。
///
/// 只看目录 mtime 不够——覆盖写一个已存在的文件不改目录 mtime，缓存会永远不失效。
fn app_layout_agents_dir_modified_time(path: &PathBuf) -> Option<std::time::SystemTime> {
    let agents_dir = app_layout_agents_dir(path);
    let mut newest = fs::metadata(&agents_dir)
        .ok()
        .and_then(|meta| meta.modified().ok());
    if let Ok(entries) = fs::read_dir(&agents_dir) {
        for entry in entries.flatten() {
            let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
                continue;
            };
            newest = Some(match newest {
                Some(current) if current >= modified => current,
                _ => modified,
            });
        }
    }
    newest
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
    let runtime_path = app_layout_runtime_state_path(path);
    // 人格是目录：目录自身与其中每个文件的 mtime 取最新那个，覆盖写单个文件也能感知。
    // 条数不参与签名——目录没有长度，只看 mtime 就够，新增/删除/改写都会刷新它。
    let agents_len = 0u64;
    let agents_modified = app_layout_agents_dir_modified_time(path);
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
    let agents = read_agents_dir_outcome(path)?;

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

fn agents_jsonc_options() -> jsonc_parser::ParseOptions {
    jsonc_parser::ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

fn agents_jsonc_salvage_options() -> jsonc_parser::ParseOptions {
    jsonc_parser::ParseOptions::default()
}

struct AgentsJsoncError {
    line: usize,
    column: usize,
    detail: String,
}

fn parse_agents_jsonc(
    content: &str,
    options: &jsonc_parser::ParseOptions,
) -> Result<Value, AgentsJsoncError> {
    jsonc_parser::parse_to_serde_value::<Value>(content, options).map_err(|err| {
        use jsonc_parser::errors::ParseErrorKind;
        let detail = match err.kind() {
            ParseErrorKind::UnterminatedObject => "对象没有闭合（少了一个 `}`）".to_string(),
            ParseErrorKind::UnterminatedArray => "数组没有闭合（少了一个 `]`）".to_string(),
            ParseErrorKind::UnterminatedCommentBlock => "注释块 `/*` 没有闭合".to_string(),
            ParseErrorKind::ExpectedColonAfterObjectKey => "对象属性后面缺少冒号".to_string(),
            ParseErrorKind::MultipleRootJsonValues => "文件里有多个顶层 JSON 值".to_string(),
            other => format!("JSON 语法错误：{other}"),
        };
        AgentsJsoncError {
            line: err.line_display(),
            column: err.column_display(),
            detail,
        }
    })
}


fn json_value_type_label(value: &Value) -> &'static str {
    match value {
        Value::Null => "空值",
        Value::Bool(_) => "布尔值",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}

fn conversation_shell_workspace_path_key(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        String::new()
    } else {
        normalize_terminal_path_for_compare(&PathBuf::from(path))
    }
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
