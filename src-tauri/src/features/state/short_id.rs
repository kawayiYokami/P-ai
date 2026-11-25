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
/// 定时器域：给「几号定时器」一个可读、可复述的号。
const SHORT_ID_DOMAIN_TIMER: &str = "timer";

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
    let mut taken = std::collections::BTreeSet::<u64>::new();
    for agent in previous.iter().chain(agents.iter()) {
        if let Some(existing) = agent.short_id {
            taken.insert(existing);
        }
    }
    for agent in agents.iter_mut() {
        if agent.short_id.is_some() {
            continue;
        }
        if let Some(existing) = previous
            .iter()
            .find(|item| item.id == agent.id)
            .and_then(|item| item.short_id)
        {
            agent.short_id = Some(existing);
            continue;
        }
        // 真正的新人格：走原子序列取号，绝不复用旧号。
        let mut short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_AGENT)?;
        while taken.contains(&short_id) {
            // 游标落后于已发出的号（手工导入、旧库直改）时跳过，避免撞号。
            short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_AGENT)?;
        }
        taken.insert(short_id);
        agent.short_id = Some(short_id);
        assigned += 1;
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
        if let Some(existing) = base
            .api_configs
            .iter()
            .find(|item| item.id == api.id)
            .and_then(|item| item.short_id)
        {
            api.short_id = Some(existing);
            continue;
        }
        // 新条目走原子序列，不复用旧号。
        let mut short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_API_CONFIG)?;
        while api_used.contains(&short_id) {
            short_id = allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_API_CONFIG)?;
        }
        api_used.insert(short_id);
        api.short_id = Some(short_id);
        assigned += 1;
    }
    for channel in config.remote_im_channels.iter_mut() {
        if channel.short_id.is_some() {
            continue;
        }
        if let Some(existing) = base
            .remote_im_channels
            .iter()
            .find(|item| item.id == channel.id)
            .and_then(|item| item.short_id)
        {
            channel.short_id = Some(existing);
            continue;
        }
        let mut short_id =
            allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_REMOTE_IM_CHANNEL)?;
        while channel_used.contains(&short_id) {
            short_id =
                allocate_next_short_id(&state.data_path, SHORT_ID_DOMAIN_REMOTE_IM_CHANNEL)?;
        }
        channel_used.insert(short_id);
        channel.short_id = Some(short_id);
        assigned += 1;
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

/// 会话引用双解析：纯数字按短 ID 寻址，其它一律当作既有主键原样返回。
///
/// 短 ID 的唯一权威是 `conversation_metadata.short_id` 列，查不到就是查不到，
/// 没有 JSON 兜底扫描（metadata_json 里根本不存短 ID）。
fn resolve_conversation_id_reference(
    state: &AppState,
    reference: &str,
) -> Result<Option<String>, String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Ok(None);
    }
    let candidate = reference.strip_prefix('#').unwrap_or(reference).trim();
    if let Ok(short_id) = candidate.parse::<u64>() {
        return chat_metadata_conversation_id_by_short_id(&state.data_path, short_id);
    }
    Ok(Some(reference.to_string()))
}

/// 历史会话迁移：只认 SQLite 的 `short_id` 列。
///
/// 读写在同一个 IMMEDIATE 事务里完成，全程不反序列化、不写回 `metadata_json`：
/// 排序键直接取 `created_at` 物理列（V5 之后 JSON 里已经没有 `createdAt`），
/// 写入只动 `short_id` 列。列上已有值的行视为永久锁定，绝不重算、绝不覆盖。
/// 返回本次新分配的条数。
fn backfill_conversation_short_ids(state: &AppState) -> Result<usize, String> {
    let mut conn = crate::message_store::chat_metadata_store_open(&state.data_path)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| format!("开启会话短 ID 补号事务失败: {err}"))?;

    let mut used = std::collections::BTreeSet::<u64>::new();
    let mut pending = Vec::<(String, String)>::new();
    {
        let mut stmt = tx
            .prepare(
                "SELECT conversation_id, short_id, COALESCE(created_at, '')
                   FROM conversation_metadata",
            )
            .map_err(|err| format!("准备读取会话短 ID 列失败: {err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|err| format!("查询会话短 ID 列失败: {err}"))?
            .collect::<Result<Vec<(String, Option<i64>, Option<String>)>, _>>()
            .map_err(|err| format!("读取会话短 ID 列失败: {err}"))?;
        for (conversation_id, short_id, created_at) in rows {
            match short_id.and_then(|value| u64::try_from(value).ok()) {
                Some(existing) => {
                    used.insert(existing);
                }
                None => pending.push((created_at.unwrap_or_default(), conversation_id)),
            }
        }
    }

    // 按创建时间升序补号，同时间按主键字典序，保证重跑顺序稳定。
    pending.sort();
    let mut next_candidate = 0u64;
    let mut assigned = 0usize;
    for (_, conversation_id) in pending {
        while used.contains(&next_candidate) {
            next_candidate += 1;
        }
        let short_id = next_candidate;
        used.insert(short_id);
        next_candidate += 1;
        let updated = tx
            .execute(
                "UPDATE conversation_metadata SET short_id=?1 \
                 WHERE conversation_id=?2 AND short_id IS NULL",
                rusqlite::params![short_id as i64, conversation_id],
            )
            .map_err(|err| format!("写会话短 ID 列失败，会话={conversation_id}，error={err}"))?;
        if updated == 0 {
            // 列已被别处写好了（不可变），把预占的号还回去，不留空洞。
            used.remove(&short_id);
            next_candidate = short_id;
            continue;
        }
        assigned += 1;
    }

    let max_used = used.iter().next_back().copied();
    tx.commit()
        .map_err(|err| format!("提交会话短 ID 补号失败: {err}"))?;
    if let Some(max_used) = max_used {
        // 游标在 state.sqlite（另一个库），只能在提交后单独推进；
        // 即使这一步中断，重跑补号也是幂等的：列已有值 -> 只做游标校准。
        advance_short_id_sequence(
            &state.data_path,
            SHORT_ID_DOMAIN_CONVERSATION,
            max_used + 1,
        )?;
    }
    Ok(assigned)
}
