// ==================== 全局短 ID 序列 ====================
//
// 每个实体域一行：domain -> next_val。分配走 IMMEDIATE 事务，多协程并发创建时
// 不会拿到同一个号；号只增不减，实体删除后它的号作废、绝不回收复用。
//
// 短 ID 是给人、给 AI 看的外部标识（#0、#42），磁盘目录与既有主键仍然是 UUID，
// 两者通过各实体上的 `short_id` 字段与既有的双向解析入口互相映射。

/// 会话域：本地、委托、队友、追问、系统通知、远程联系人会话共用一个连续空间。
const SHORT_ID_DOMAIN_CONVERSATION: &str = "conversation";
/// 人格域。
const SHORT_ID_DOMAIN_AGENT: &str = "agent";
/// MCP 域。
const SHORT_ID_DOMAIN_MCP: &str = "mcp";
/// 模型配置域。
const SHORT_ID_DOMAIN_API_CONFIG: &str = "api_config";
/// 远程联系人渠道域。
const SHORT_ID_DOMAIN_REMOTE_IM_CHANNEL: &str = "remote_im_channel";

/// 原子取号：返回本次分配到的短 ID。
///
/// 事务用 IMMEDIATE，先把写锁拿到再读游标：并发调用时后到者会在写锁上排队，
/// 不会两个请求读到同一个 next_val。
fn allocate_next_short_id(data_path: &PathBuf, domain: &str) -> Result<u64, String> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Err("短 ID 域名为空".to_string());
    }
    let mut conn = state_db_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| format!("开启短 ID 分配事务失败，domain={domain}，error={err}"))?;
    tx.execute(
        "INSERT OR IGNORE INTO global_short_id_sequences(domain, next_val) VALUES(?1, 0)",
        rusqlite::params![domain],
    )
    .map_err(|err| format!("初始化短 ID 序列失败，domain={domain}，error={err}"))?;
    let current: i64 = tx
        .query_row(
            "SELECT next_val FROM global_short_id_sequences WHERE domain=?1",
            rusqlite::params![domain],
            |row| row.get(0),
        )
        .map_err(|err| format!("读取短 ID 序列失败，domain={domain}，error={err}"))?;
    let next = current.saturating_add(1);
    tx.execute(
        "UPDATE global_short_id_sequences SET next_val=?2 WHERE domain=?1",
        rusqlite::params![domain, next],
    )
    .map_err(|err| format!("推进短 ID 序列失败，domain={domain}，error={err}"))?;
    tx.commit()
        .map_err(|err| format!("提交短 ID 分配失败，domain={domain}，error={err}"))?;
    Ok(current.max(0) as u64)
}

/// 读当前游标但**不分配**（观测与测试用：确认某个域下一个会发几号）。
#[allow(dead_code)]
fn peek_next_short_id(data_path: &PathBuf, domain: &str) -> Result<u64, String> {
    let conn = state_db_open(data_path)?;
    let value: Option<i64> = conn
        .query_row(
            "SELECT next_val FROM global_short_id_sequences WHERE domain=?1",
            rusqlite::params![domain.trim()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| format!("读取短 ID 游标失败，domain={domain}，error={err}"))?;
    Ok(value.unwrap_or(0).max(0) as u64)
}

/// 把游标抬到至少 `next_val`（迁移收尾校准用）。
///
/// 只前进不后退：迁移重跑、或两个域写同一份历史时，都不能把已经发出去的号再发一遍。
fn advance_short_id_sequence(
    data_path: &PathBuf,
    domain: &str,
    next_val: u64,
) -> Result<(), String> {
    let conn = state_db_open(data_path)?;
    conn.execute(
        "INSERT INTO global_short_id_sequences(domain, next_val) VALUES(?1, ?2)
         ON CONFLICT(domain) DO UPDATE SET next_val = max(next_val, excluded.next_val)",
        rusqlite::params![domain.trim(), next_val as i64],
    )
    .map_err(|err| format!("校准短 ID 游标失败，domain={domain}，error={err}"))?;
    Ok(())
}

// ==================== 通用补号 ====================

/// 给一批待补槽位挑号：从 0 开始挑当前未被占用的最小号，逐个向前。
///
/// `used` 里是已经发出去的号（含本次已挑的），挑完即写入，保证同一批内也不重复。
fn pick_short_ids_for_pending(
    used: &mut std::collections::BTreeSet<u64>,
    pending_count: usize,
) -> Vec<u64> {
    let mut next = 0u64;
    let mut picked = Vec::with_capacity(pending_count);
    for _ in 0..pending_count {
        while used.contains(&next) {
            next += 1;
        }
        used.insert(next);
        picked.push(next);
        next += 1;
    }
    picked
}

// ==================== 人格域 ====================

/// 给历史人格补号：内置人格占低位，自定义按创建时间升序（同时间按 id）。
fn backfill_agent_short_ids(state: &AppState) -> Result<usize, String> {
    let mut agents = state_read_agents_cached(state)?;
    if agents.is_empty() {
        return Ok(0);
    }
    let mut used = std::collections::BTreeSet::<u64>::new();
    let mut pending = Vec::<(u8, String, String, usize)>::new();
    for (index, agent) in agents.iter().enumerate() {
        match agent.short_id {
            Some(existing) => {
                used.insert(existing);
            }
            None => {
                let builtin_rank = u8::from(!(agent.is_built_in_user || agent.is_built_in_system));
                pending.push((
                    builtin_rank,
                    agent.created_at.trim().to_string(),
                    agent.id.clone(),
                    index,
                ));
            }
        }
    }
    if pending.is_empty() {
        if let Some(max_used) = used.iter().next_back().copied() {
            advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_AGENT, max_used + 1)?;
        }
        return Ok(0);
    }
    pending.sort();
    let picked = pick_short_ids_for_pending(&mut used, pending.len());
    for ((_, _, _, index), short_id) in pending.iter().zip(picked) {
        agents[*index].short_id = Some(short_id);
    }
    state_write_agents_cached(state, &agents)?;
    if let Some(max_used) = used.iter().next_back().copied() {
        advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_AGENT, max_used + 1)?;
    }
    Ok(pending.len())
}

/// 保存人格时保证短 ID 不丢：同 id 沿用旧号，只有真正的新人格才分配新号。
///
/// 前端回传的 AgentProfile 可能不带 `shortId`，所以不能按「传进来是 None 就当新人格」处理，
/// 否则每次保存都会把所有人的号重发一遍。
fn inherit_or_assign_agent_short_ids(
    state: &AppState,
    previous: &[AgentProfile],
    agents: &mut [AgentProfile],
) -> Result<usize, String> {
    let mut assigned = 0usize;
    let mut used = std::collections::BTreeSet::<u64>::new();
    for agent in previous.iter().chain(agents.iter()) {
        if let Some(existing) = agent.short_id {
            used.insert(existing);
        }
    }
    for agent in agents.iter_mut() {
        if agent.short_id.is_some() {
            continue;
        }
        if let Some(previous_agent) = previous.iter().find(|item| item.id == agent.id) {
            if let Some(existing) = previous_agent.short_id {
                agent.short_id = Some(existing);
                continue;
            }
        }
        let picked = pick_short_ids_for_pending(&mut used, 1);
        agent.short_id = picked.first().copied();
        assigned += 1;
    }
    if assigned > 0 {
        if let Some(max_used) = used.iter().next_back().copied() {
            advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_AGENT, max_used + 1)?;
        }
    }
    Ok(assigned)
}

// ==================== MCP 域 ====================

/// 给历史 MCP 服务补号：按加载顺序（即配置目录中的接入顺序）依次补齐。
fn backfill_mcp_short_ids(state: &AppState) -> Result<usize, String> {
    let servers = load_workspace_mcp_servers(state)?;
    if servers.is_empty() {
        return Ok(0);
    }
    let mut used = std::collections::BTreeSet::<u64>::new();
    let mut pending = Vec::<usize>::new();
    for (index, server) in servers.iter().enumerate() {
        match server.short_id {
            Some(existing) => {
                used.insert(existing);
            }
            None => pending.push(index),
        }
    }
    if pending.is_empty() {
        if let Some(max_used) = used.iter().next_back().copied() {
            advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_MCP, max_used + 1)?;
        }
        return Ok(0);
    }
    let picked = pick_short_ids_for_pending(&mut used, pending.len());
    let mut assigned = 0usize;
    for (index, short_id) in pending.iter().zip(picked) {
        let mut server = servers[*index].clone();
        server.short_id = Some(short_id);
        save_workspace_mcp_server(state, &server)?;
        assigned += 1;
    }
    if let Some(max_used) = used.iter().next_back().copied() {
        advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_MCP, max_used + 1)?;
    }
    Ok(assigned)
}

/// 保存 MCP 服务时保证短 ID 不丢：已有记录沿用旧号，新接入的服务才分配。
fn resolve_mcp_server_short_id(
    state: &AppState,
    server_id: &str,
    incoming: Option<u64>,
) -> Result<Option<u64>, String> {
    if incoming.is_some() {
        return Ok(incoming);
    }
    if let Ok(existing) = load_server_by_id(state, server_id) {
        if let Some(existing_short_id) = existing.short_id {
            return Ok(Some(existing_short_id));
        }
    }
    Ok(Some(allocate_next_short_id(
        &state.data_path,
        SHORT_ID_DOMAIN_MCP,
    )?))
}

// ==================== 模型配置与远程渠道 ====================

/// 给历史模型配置与远程渠道补号：按配置里的现有顺序依次补齐。
fn backfill_app_config_short_ids(state: &AppState) -> Result<usize, String> {
    let mut config = state_read_config_cached(state)?;
    let mut api_used = std::collections::BTreeSet::<u64>::new();
    let mut api_pending = Vec::<usize>::new();
    for (index, api) in config.api_configs.iter().enumerate() {
        match api.short_id {
            Some(existing) => {
                api_used.insert(existing);
            }
            None => api_pending.push(index),
        }
    }
    let mut channel_used = std::collections::BTreeSet::<u64>::new();
    let mut channel_pending = Vec::<usize>::new();
    for (index, channel) in config.remote_im_channels.iter().enumerate() {
        match channel.short_id {
            Some(existing) => {
                channel_used.insert(existing);
            }
            None => channel_pending.push(index),
        }
    }
    let mut assigned = 0usize;
    if !api_pending.is_empty() {
        let picked = pick_short_ids_for_pending(&mut api_used, api_pending.len());
        for (index, short_id) in api_pending.iter().zip(picked) {
            config.api_configs[*index].short_id = Some(short_id);
            assigned += 1;
        }
    }
    if !channel_pending.is_empty() {
        let picked = pick_short_ids_for_pending(&mut channel_used, channel_pending.len());
        for (index, short_id) in channel_pending.iter().zip(picked) {
            config.remote_im_channels[*index].short_id = Some(short_id);
            assigned += 1;
        }
    }
    if assigned > 0 {
        state_write_config_cached(state, &config)?;
    }
    if let Some(max_used) = api_used.iter().next_back().copied() {
        advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_API_CONFIG, max_used + 1)?;
    }
    if let Some(max_used) = channel_used.iter().next_back().copied() {
        advance_short_id_sequence(
            &state.data_path,
            SHORT_ID_DOMAIN_REMOTE_IM_CHANNEL,
            max_used + 1,
        )?;
    }
    Ok(assigned)
}

/// 保存配置时保证短 ID 不丢：同 id 沿用旧号，新条目才分配。
///
/// 前端回传的配置同样可能不带 `shortId`，所以新号只发给「库里也没有同 id」的条目。
fn inherit_or_assign_config_short_ids(
    state: &AppState,
    base: &AppConfig,
    config: &mut AppConfig,
) -> Result<usize, String> {
    let mut api_used = std::collections::BTreeSet::<u64>::new();
    for api in base.api_configs.iter().chain(config.api_configs.iter()) {
        if let Some(existing) = api.short_id {
            api_used.insert(existing);
        }
    }
    let mut channel_used = std::collections::BTreeSet::<u64>::new();
    for channel in base
        .remote_im_channels
        .iter()
        .chain(config.remote_im_channels.iter())
    {
        if let Some(existing) = channel.short_id {
            channel_used.insert(existing);
        }
    }
    let mut assigned = 0usize;
    for api in config.api_configs.iter_mut() {
        if api.short_id.is_some() {
            continue;
        }
        if let Some(previous) = base.api_configs.iter().find(|item| item.id == api.id) {
            if let Some(existing) = previous.short_id {
                api.short_id = Some(existing);
                continue;
            }
        }
        api.short_id = pick_short_ids_for_pending(&mut api_used, 1).first().copied();
        assigned += 1;
    }
    for channel in config.remote_im_channels.iter_mut() {
        if channel.short_id.is_some() {
            continue;
        }
        if let Some(previous) = base
            .remote_im_channels
            .iter()
            .find(|item| item.id == channel.id)
        {
            if let Some(existing) = previous.short_id {
                channel.short_id = Some(existing);
                continue;
            }
        }
        channel.short_id = pick_short_ids_for_pending(&mut channel_used, 1)
            .first()
            .copied();
        assigned += 1;
    }
    if assigned > 0 {
        if let Some(max_used) = api_used.iter().next_back().copied() {
            advance_short_id_sequence(&state.data_path, SHORT_ID_DOMAIN_API_CONFIG, max_used + 1)?;
        }
        if let Some(max_used) = channel_used.iter().next_back().copied() {
            advance_short_id_sequence(
                &state.data_path,
                SHORT_ID_DOMAIN_REMOTE_IM_CHANNEL,
                max_used + 1,
            )?;
        }
    }
    Ok(assigned)
}

/// 启动自检：给还没有短 ID 的历史实体补号，并把各域游标校准到 max+1。
///
/// 只处理没有号的：已分配的号一律保留，所以中途失败后重启可以从断点继续，
/// 不会重排用户已经看到的编号。某个域失败只记日志，不影响其它域。
fn run_global_short_id_bootstrap(state: &AppState) -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut total = 0usize;
    match backfill_conversation_short_ids(state) {
        Ok(count) => total += count,
        Err(err) => runtime_log_warn(format!("[短ID] 失败 任务=历史会话补号 原因={err}")),
    }
    match backfill_agent_short_ids(state) {
        Ok(count) => total += count,
        Err(err) => runtime_log_warn(format!("[短ID] 失败 任务=历史人格补号 原因={err}")),
    }
    match backfill_mcp_short_ids(state) {
        Ok(count) => total += count,
        Err(err) => runtime_log_warn(format!("[短ID] 失败 任务=历史 MCP 补号 原因={err}")),
    }
    match backfill_app_config_short_ids(state) {
        Ok(count) => total += count,
        Err(err) => runtime_log_warn(format!("[短ID] 失败 任务=历史配置补号 原因={err}")),
    }
    runtime_log_info(format!(
        "[短ID] 完成 任务=历史实体补号 新增={} 耗时={}ms",
        total,
        started.elapsed().as_millis()
    ));
    Ok(())
}

// ==================== 会话域 ====================

/// 为新建会话分配短 ID（已有则原样返回）。
///
/// 分配成功后写回会话本身，并尽力同步会话库里的索引列；会话此刻可能还没落库
/// （调度持久化是异步的），0 行更新的情况由迁移与读取回退兜底。
fn assign_conversation_short_id(
    state: &AppState,
    conversation: &mut Conversation,
) -> Result<u64, String> {
    if let Some(existing) = conversation.short_id {
        return Ok(existing);
    }
    let short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_CONVERSATION)?;
    conversation.short_id = Some(short_id);
    if let Err(err) =
        chat_metadata_set_conversation_short_id(&state.data_path, &conversation.id, short_id)
    {
        runtime_log_debug(format!(
            "[短ID] 跳过 任务=同步会话索引列 会话={} 短ID={} 原因={}",
            conversation.id, short_id, err
        ));
    }
    Ok(short_id)
}

/// 把短 ID 写进会话库的索引列。返回更新的行数（0 表示这一行还没落库）。
fn chat_metadata_set_conversation_short_id(
    data_path: &PathBuf,
    conversation_id: &str,
    short_id: u64,
) -> Result<usize, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(0);
    }
    let conn = crate::message_store::chat_metadata_store_open(data_path)?;
    conn.execute(
        "UPDATE conversation_metadata SET short_id=?1 WHERE conversation_id=?2",
        rusqlite::params![short_id as i64, conversation_id],
    )
    .map_err(|err| format!("写入会话短 ID 索引失败，会话={conversation_id}，error={err}"))
}

/// 按短 ID 查会话主键（走索引列）。
fn chat_metadata_conversation_id_by_short_id(
    data_path: &PathBuf,
    short_id: u64,
) -> Result<Option<String>, String> {
    use rusqlite::OptionalExtension as _;
    let conn = crate::message_store::chat_metadata_store_open(data_path)?;
    conn.query_row(
        "SELECT conversation_id FROM conversation_metadata WHERE short_id=?1",
        rusqlite::params![short_id as i64],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|err| format!("按短 ID 查会话失败，短ID={short_id}，error={err}"))
}

/// 按短 ID 扫元数据兜底查会话主键。
///
/// 索引列是加速用的，老库、迁移中断、或落库早于列同步时都可能没有值；
/// 这时候回到元数据里找，保证「短 ID 一定能寻址」。
fn conversation_id_by_short_id_scan(
    state: &AppState,
    short_id: u64,
) -> Result<Option<String>, String> {
    let conversations = read_all_conversation_metadata_rows(&state.data_path)?;
    for (conversation_id, metadata) in conversations {
        let value = metadata.get("shortId").and_then(serde_json::Value::as_u64);
        if value == Some(short_id) {
            return Ok(Some(conversation_id));
        }
    }
    Ok(None)
}

/// 会话引用双解析：纯数字按短 ID 寻址，其它一律当作既有主键原样返回。
fn resolve_conversation_id_reference(
    state: &AppState,
    reference: &str,
) -> Result<Option<String>, String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Ok(None);
    }
    if let Ok(short_id) = reference.parse::<u64>() {
        if let Some(conversation_id) =
            chat_metadata_conversation_id_by_short_id(&state.data_path, short_id)?
        {
            return Ok(Some(conversation_id));
        }
        return conversation_id_by_short_id_scan(state, short_id);
    }
    Ok(Some(reference.to_string()))
}

/// 读出会话库里全部 (conversation_id, metadata_json)。
fn read_all_conversation_metadata_rows(
    data_path: &PathBuf,
) -> Result<Vec<(String, serde_json::Value)>, String> {
    let conn = crate::message_store::chat_metadata_store_open(data_path)?;
    let mut stmt = conn
        .prepare("SELECT conversation_id, metadata_json FROM conversation_metadata")
        .map_err(|err| format!("准备读取会话元数据失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("查询会话元数据失败: {err}"))?
        .collect::<Result<Vec<(String, String)>, _>>()
        .map_err(|err| format!("读取会话元数据行失败: {err}"))?;
    let mut result = Vec::with_capacity(rows.len());
    for (conversation_id, raw) in rows {
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(value) => result.push((conversation_id, value)),
            Err(err) => runtime_log_warn(format!(
                "[短ID] 跳过 任务=解析会话元数据 会话={} 原因={}",
                conversation_id, err
            )),
        }
    }
    Ok(result)
}

/// 历史会话迁移：按创建时间升序（同时间按主键字典序）给未分配 short_id 的会话补号。
///
/// 从 0 起挑未被占用的号依次分配，已分配的号原样保留——迁移中断后重跑不会重算，
/// 也不会打乱用户已经看到的号。返回本次新分配的条数。
fn backfill_conversation_short_ids(state: &AppState) -> Result<usize, String> {
    let rows = read_all_conversation_metadata_rows(&state.data_path)?;
    let mut used = std::collections::BTreeSet::<u64>::new();
    let mut pending = Vec::<(String, String)>::new();
    for (conversation_id, metadata) in rows {
        match metadata.get("shortId").and_then(serde_json::Value::as_u64) {
            Some(existing) => {
                used.insert(existing);
            }
            None => {
                let created_at = metadata
                    .get("createdAt")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string();
                pending.push((created_at, conversation_id));
            }
        }
    }
    if pending.is_empty() {
        // 没有待分配的，只把游标校准到 max+1。
        if let Some(max_used) = used.iter().next_back().copied() {
            advance_short_id_sequence(
                &state.data_path,
                SHORT_ID_DOMAIN_CONVERSATION,
                max_used + 1,
            )?;
        }
        return Ok(0);
    }
    pending.sort();
    let conn = crate::message_store::chat_metadata_store_open(&state.data_path)?;
    let mut next_candidate = 0u64;
    let mut assigned = 0usize;
    for (_, conversation_id) in pending {
        while used.contains(&next_candidate) {
            next_candidate += 1;
        }
        let short_id = next_candidate;
        used.insert(short_id);
        next_candidate += 1;
        // 元数据里的值是权威，先解析出来插字段再写回；索引列同一次事务里一起更新。
        let raw: String = conn
            .query_row(
                "SELECT metadata_json FROM conversation_metadata WHERE conversation_id=?1",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|err| format!("读取会话元数据失败，会话={conversation_id}，error={err}"))?;
        let mut metadata: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|err| format!("解析会话元数据失败，会话={conversation_id}，error={err}"))?;
        if let Some(object) = metadata.as_object_mut() {
            object.insert(
                "shortId".to_string(),
                serde_json::Value::from(short_id),
            );
        }
        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|err| format!("序列化会话元数据失败，会话={conversation_id}，error={err}"))?;
        conn.execute(
            "UPDATE conversation_metadata SET metadata_json=?1, short_id=?2 WHERE conversation_id=?3",
            rusqlite::params![metadata_json, short_id as i64, conversation_id],
        )
        .map_err(|err| format!("写回会话短 ID 失败，会话={conversation_id}，error={err}"))?;
        assigned += 1;
    }
    if let Some(max_used) = used.iter().next_back().copied() {
        advance_short_id_sequence(
            &state.data_path,
            SHORT_ID_DOMAIN_CONVERSATION,
            max_used + 1,
        )?;
    }
    Ok(assigned)
}
