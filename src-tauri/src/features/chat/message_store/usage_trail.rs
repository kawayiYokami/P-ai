// ==================== usage_trail 用量台账 ====================
// 全局 SQLite 台账表：按「小时桶 × 会话」记录每次 LLM 调用的 token 用量，
// 作为足迹墙与用量页的唯一用量数据源（写时记账，不逐会话读消息）。

pub(super) const USAGE_TRAIL_EPOCH_BUCKET: &str = "epoch";

#[derive(Debug, Clone, Default)]
pub(super) struct UsageTrailTokenDelta {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
}

impl UsageTrailTokenDelta {
    pub(super) fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.total_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_write_tokens == 0
            && self.reasoning_tokens == 0
    }

    pub(super) fn saturating_add_assign(&mut self, other: &UsageTrailTokenDelta) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(other.cache_read_tokens);
        self.cache_write_tokens = self.cache_write_tokens.saturating_add(other.cache_write_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }
}

#[derive(Debug, Clone)]
pub(super) struct UsageTrailDelta {
    pub conversation_id: String,
    pub agent_id: String,
    pub conversation_kind: String,
    pub api_config_id: String,
    pub provider_key: String,
    pub provider_label: String,
    pub model_name: String,
    pub tokens: UsageTrailTokenDelta,
}

#[derive(Debug, Clone)]
pub(super) struct UsageTrailRow {
    pub bucket: String,
    pub conversation_id: String,
    pub agent_id: String,
    pub conversation_kind: String,
    pub api_config_id: String,
    pub provider_key: String,
    pub provider_label: String,
    pub model_name: String,
    pub tokens: UsageTrailTokenDelta,
}

fn usage_trail_read_u64(usage: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| {
            let value = usage.get(*key)?;
            value
                .as_u64()
                .or_else(|| value.as_i64().and_then(|item| u64::try_from(item).ok()))
        })
        .unwrap_or(0)
}

/// 从 LLM usage JSON 解析本次调用的 token 增量，key 口径与
/// conversation_cumulative_usage_add_provider_usage 保持一致。
pub(super) fn usage_trail_token_delta_from_usage_value(usage: &Value) -> UsageTrailTokenDelta {
    let input_tokens = usage_trail_read_u64(usage, &["promptTokens", "prompt_tokens"]);
    let output_tokens = usage_trail_read_u64(usage, &["completionTokens", "completion_tokens"]);
    UsageTrailTokenDelta {
        input_tokens,
        output_tokens,
        total_tokens: usage_trail_read_u64(usage, &["totalTokens", "total_tokens"])
            .max(input_tokens.saturating_add(output_tokens)),
        cache_read_tokens: usage_trail_read_u64(usage, &["cachedTokens", "cached_tokens"]),
        cache_write_tokens: usage_trail_read_u64(
            usage,
            &["cacheCreationTokens", "cache_creation_tokens"],
        )
        .saturating_add(usage_trail_read_u64(
            usage,
            &["cacheCreation5mTokens", "cache_creation_5m_tokens"],
        ))
        .saturating_add(usage_trail_read_u64(
            usage,
            &["cacheCreation1hTokens", "cache_creation_1h_tokens"],
        )),
        reasoning_tokens: usage_trail_read_u64(usage, &["reasoningTokens", "reasoning_tokens"]),
    }
}

/// 本地时区小时桶：YYYY-MM-DDTHH:00:00。
/// 按凌晨 4 点分界：0:00-3:59 的使用归属前一个分界日（日期减一天），小时保持实际小时。
pub(super) fn usage_trail_hour_bucket(dt: OffsetDateTime) -> String {
    let local = to_local_datetime(dt);
    let shifted = local - time::Duration::hours(4);
    format!(
        "{:04}-{:02}-{:02}T{:02}:00:00",
        shifted.year(),
        shifted.month() as u8,
        shifted.day(),
        local.hour()
    )
}

/// 按小时桶 UPSERT 累加一次用量增量；同一小时同一会话同一模型多次调用累加同一行。
pub(super) fn chat_metadata_store_usage_trail_upsert_delta(
    data_path: &PathBuf,
    bucket: &str,
    delta: &UsageTrailDelta,
) -> Result<(), String> {
    let normalized_bucket = bucket.trim();
    if normalized_bucket.is_empty() {
        return Ok(());
    }
    if delta.tokens.is_empty() {
        return Ok(());
    }
    let conn = chat_metadata_store_open(data_path)?;
    usage_trail_upsert_on_conn(&conn, normalized_bucket, delta)
}

/// 在已打开的连接上执行台账 UPSERT（供写入链路与迁移事务共用）。
fn usage_trail_upsert_on_conn(
    conn: &rusqlite::Connection,
    bucket: &str,
    delta: &UsageTrailDelta,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage_trail (
           bucket, conversation_id, agent_id, conversation_kind,
           api_config_id, provider_key, provider_label, model_name,
           input_tokens, output_tokens, total_tokens, cache_read_tokens, cache_write_tokens,
           reasoning_tokens, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
         ON CONFLICT(bucket, conversation_id, provider_key, model_name) DO UPDATE SET
           agent_id=excluded.agent_id,
           conversation_kind=excluded.conversation_kind,
           api_config_id=excluded.api_config_id,
           provider_label=excluded.provider_label,
           input_tokens=input_tokens+excluded.input_tokens,
           output_tokens=output_tokens+excluded.output_tokens,
           total_tokens=total_tokens+excluded.total_tokens,
           cache_read_tokens=cache_read_tokens+excluded.cache_read_tokens,
           cache_write_tokens=cache_write_tokens+excluded.cache_write_tokens,
           reasoning_tokens=reasoning_tokens+excluded.reasoning_tokens,
           updated_at=excluded.updated_at",
        rusqlite::params![
            bucket,
            delta.conversation_id,
            delta.agent_id,
            delta.conversation_kind,
            delta.api_config_id,
            delta.provider_key,
            delta.provider_label,
            delta.model_name,
            delta.tokens.input_tokens as i64,
            delta.tokens.output_tokens as i64,
            delta.tokens.total_tokens as i64,
            delta.tokens.cache_read_tokens as i64,
            delta.tokens.cache_write_tokens as i64,
            delta.tokens.reasoning_tokens as i64,
            now_iso(),
        ],
    )
    .map_err(|err| format!("写入用量台账失败，conversation_id={}，error={err}", delta.conversation_id))?;
    Ok(())
}

/// 查询台账行；bucket_start 为本地小时桶下界（含），None 表示全部（含 epoch 历史桶）。
pub(super) fn chat_metadata_store_usage_trail_query(
    data_path: &PathBuf,
    bucket_start: Option<&str>,
) -> Result<Vec<UsageTrailRow>, String> {
    let conn = chat_metadata_store_open(data_path)?;
    let base_sql = "SELECT bucket, conversation_id, agent_id, conversation_kind,
           api_config_id, provider_key, provider_label, model_name,
           input_tokens, output_tokens, total_tokens, cache_read_tokens, cache_write_tokens,
           reasoning_tokens
         FROM usage_trail";
    let mut params = Vec::<&dyn rusqlite::ToSql>::new();
    let start_owned: Option<String>;
    let sql;
    match bucket_start {
        Some(start) => {
            start_owned = Some(start.to_string());
            params.push(start_owned.as_ref().expect("start_owned set"));
            sql = format!("{base_sql} WHERE bucket >= ?1 ORDER BY bucket ASC");
        }
        None => {
            sql = format!("{base_sql} ORDER BY bucket ASC");
        }
    }
    let mut statement = conn
        .prepare(&sql)
        .map_err(|err| format!("准备读取用量台账失败: {err}"))?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(params.iter().copied()), |row| {
            Ok(UsageTrailRow {
                bucket: row.get(0)?,
                conversation_id: row.get(1)?,
                agent_id: row.get(2)?,
                conversation_kind: row.get(3)?,
                api_config_id: row.get(4)?,
                provider_key: row.get(5)?,
                provider_label: row.get(6)?,
                model_name: row.get(7)?,
                tokens: UsageTrailTokenDelta {
                    input_tokens: row.get::<_, i64>(8)? as u64,
                    output_tokens: row.get::<_, i64>(9)? as u64,
                    total_tokens: row.get::<_, i64>(10)? as u64,
                    cache_read_tokens: row.get::<_, i64>(11)? as u64,
                    cache_write_tokens: row.get::<_, i64>(12)? as u64,
                    reasoning_tokens: row.get::<_, i64>(13)? as u64,
                },
            })
        })
        .map_err(|err| format!("读取用量台账失败: {err}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| format!("解析用量台账行失败: {err}"))?);
    }
    Ok(out)
}

/// 把会话的历史 cumulative_usage（含 by_provider_model 拆分与 legacy remainder）
/// 写入 usage_trail 的 epoch 桶。幂等：chat_storage_migrations 标记 completed。
/// 仅在 v3 消息仓库迁移完成后执行，避免迁移空表后漏掉历史。
///
/// 事务 + 进程内互斥：全程在一个事务内 UPSERT 并写入 completed 标记，
/// 中途失败整体回滚，避免重跑时对已写入的 epoch 行再次累加（翻倍）。
pub(super) fn chat_metadata_store_run_usage_trail_migration(
    data_path: &PathBuf,
    config: &AppConfig,
    agents: &[AgentProfile],
) -> Result<(), String> {
    static MIGRATION_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let _guard = MIGRATION_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .map_err(|_| "用量台账迁移互斥锁不可用".to_string())?;
    if chat_metadata_store_migration_is_completed(data_path, USAGE_TRAIL_MIGRATION_KEY)? {
        return Ok(());
    }
    let mut conn = chat_metadata_store_open(data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启用量台账迁移事务失败: {err}"))?;
    let mut statement = tx
        .prepare("SELECT conversation_id, metadata_json FROM conversation_metadata")
        .map_err(|err| format!("准备读取会话元数据失败: {err}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("读取会话元数据失败: {err}"))?;
    let mut pending = Vec::<(String, String)>::new();
    for row in rows {
        pending.push(row.map_err(|err| format!("解析会话元数据行失败: {err}"))?);
    }
    drop(statement);
    let mut migrated_row_count = 0_usize;
    for (conversation_id, metadata_json) in pending {
        let meta = match serde_json::from_str::<ConversationShardMeta>(&metadata_json) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let cumulative = meta.cumulative_usage.clone().normalized_legacy_totals();
        if cumulative.is_empty() {
            continue;
        }
        let api_config_id = usage_trail_resolve_api_config_id_from_meta(&meta, config, agents);
        let provider_key = usage_trail_provider_key_from_api_config_id(&api_config_id, config);
        let provider_label = usage_trail_provider_label_from_provider_key(&provider_key, config);
        let model_name = usage_trail_resolve_model_name(&api_config_id, config);
        let mut written = false;
        for (trail_provider_key, models) in &cumulative.by_provider_model {
            for (trail_model_name, bucket) in models {
                if bucket.is_empty() {
                    continue;
                }
                let delta = UsageTrailDelta {
                    conversation_id: conversation_id.clone(),
                    agent_id: meta.agent_id.clone(),
                    conversation_kind: usage_trail_kind_key_from_meta(&meta),
                    api_config_id: api_config_id.clone(),
                    provider_key: trail_provider_key.clone(),
                    provider_label: usage_trail_provider_label_from_provider_key(
                        trail_provider_key,
                        config,
                    ),
                    model_name: trail_model_name.clone(),
                    tokens: UsageTrailTokenDelta {
                        input_tokens: bucket.input_tokens,
                        output_tokens: bucket.output_tokens,
                        total_tokens: bucket.total_tokens,
                        cache_read_tokens: bucket.cache_read_tokens,
                        cache_write_tokens: bucket.cache_write_tokens,
                        reasoning_tokens: bucket.reasoning_tokens,
                    },
                };
                usage_trail_upsert_on_conn(&tx, USAGE_TRAIL_EPOCH_BUCKET, &delta)?;
                written = true;
            }
        }
        let remainder = cumulative.legacy_remainder();
        if !remainder.is_empty() {
            let delta = UsageTrailDelta {
                conversation_id: conversation_id.clone(),
                agent_id: meta.agent_id.clone(),
                conversation_kind: usage_trail_kind_key_from_meta(&meta),
                api_config_id: api_config_id.clone(),
                provider_key: provider_key.clone(),
                provider_label: provider_label.clone(),
                model_name: if model_name.is_empty() {
                    "unknown".to_string()
                } else {
                    model_name.clone()
                },
                tokens: UsageTrailTokenDelta {
                    input_tokens: remainder.input_tokens,
                    output_tokens: remainder.output_tokens,
                    total_tokens: remainder.total_tokens,
                    cache_read_tokens: remainder.cache_read_tokens,
                    cache_write_tokens: remainder.cache_write_tokens,
                    reasoning_tokens: remainder.reasoning_tokens,
                },
            };
            usage_trail_upsert_on_conn(&tx, USAGE_TRAIL_EPOCH_BUCKET, &delta)?;
            written = true;
        }
        if written {
            migrated_row_count = migrated_row_count.saturating_add(1);
        }
    }
    tx.execute(
        "INSERT INTO chat_storage_migrations(migration_key, state, updated_at) VALUES(?1, 'completed', ?2)
         ON CONFLICT(migration_key) DO UPDATE SET state='completed', updated_at=excluded.updated_at",
        rusqlite::params![USAGE_TRAIL_MIGRATION_KEY, now_iso()],
    )
    .map_err(|err| {
        format!(
            "写入用量台账迁移状态失败，migration_key={USAGE_TRAIL_MIGRATION_KEY}，error={err}"
        )
    })?;
    tx.commit()
        .map_err(|err| format!("提交用量台账迁移事务失败: {err}"))?;
    runtime_log_info(format!(
        "[用量台账] 完成历史迁移，写入会话数={}，data_path={}",
        migrated_row_count,
        data_path.display()
    ));
    Ok(())
}

fn usage_trail_kind_key_from_meta(meta: &ConversationShardMeta) -> String {
    if meta.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID
        || meta.conversation_kind.trim() == CONVERSATION_KIND_SYSTEM_NOTIFICATION
    {
        return "system_notification".to_string();
    }
    if meta
        .delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return "delegate".to_string();
    }
    if meta.conversation_kind.trim() == "remote_im_contact" {
        return "remote_im_contact".to_string();
    }
    if meta.archived_at.is_some() {
        return "archived".to_string();
    }
    "normal".to_string()
}

fn usage_trail_resolve_api_config_id_from_meta(
    meta: &ConversationShardMeta,
    config: &AppConfig,
    agents: &[AgentProfile],
) -> String {
    let preferred = meta
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(value) = preferred {
        return value;
    }
    // 会话未固化模型时回退到负责人格的主模型（旧口径是所属部门的主模型）。
    let agent_id = meta.agent_id.trim();
    if agent_id.is_empty() {
        return String::new();
    }
    agents
        .iter()
        .find(|agent| agent.id.trim() == agent_id)
        .map(|agent| {
            resolve_chat_api_config_id(config, &agent_primary_api_config_id(agent)).unwrap_or_default()
        })
        .unwrap_or_default()
}

fn usage_trail_provider_key_from_api_config_id(api_config_id: &str, config: &AppConfig) -> String {
    let normalized_api_config_id = api_config_id.trim();
    if normalized_api_config_id.is_empty() {
        return "unknown_provider".to_string();
    }
    parse_api_endpoint_id(normalized_api_config_id)
        .map(|(provider_id, _)| provider_id)
        .or_else(|| {
            config
                .api_configs
                .iter()
                .find(|item| item.id.trim() == normalized_api_config_id)
                .map(|item| item.request_format.as_str().to_string())
        })
        .unwrap_or_else(|| "unknown_provider".to_string())
}

fn usage_trail_provider_label_from_provider_key(provider_key: &str, config: &AppConfig) -> String {
    let normalized_provider_key = provider_key.trim();
    if normalized_provider_key.is_empty() {
        return "未识别供应商".to_string();
    }
    config
        .api_providers
        .iter()
        .find(|item| item.id.trim() == normalized_provider_key)
        .map(|item| item.name.clone())
        .unwrap_or_else(|| normalized_provider_key.to_string())
}

fn usage_trail_resolve_model_name(api_config_id: &str, config: &AppConfig) -> String {
    let normalized_api_config_id = api_config_id.trim();
    if normalized_api_config_id.is_empty() {
        return String::new();
    }
    if let Some(model_name) = config
        .api_configs
        .iter()
        .find(|item| item.id.trim() == normalized_api_config_id)
        .map(|item| item.model.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
    {
        return model_name;
    }
    let (provider_id, model_id) = match parse_api_endpoint_id(normalized_api_config_id) {
        Some(value) => value,
        None => return String::new(),
    };
    config
        .api_providers
        .iter()
        .find(|provider| provider.id.trim() == provider_id.trim())
        .and_then(|provider| {
            provider
                .models
                .iter()
                .find(|model| model.id.trim() == model_id.trim())
        })
        .map(|model| model.model.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_default()
}
