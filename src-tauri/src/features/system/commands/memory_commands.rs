#[tauri::command]
fn list_memories(state: State<'_, AppState>) -> Result<Vec<MemoryEntry>, String> {
    memory_store_list_memories(&state.data_path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryExportPayload {
    schema_version: u32,
    exported_at: i64,
    records: Vec<AngelMemoryExportRecord>,
    global_tags: Vec<AngelMemoryExportTag>,
    memory_tag_rel: Vec<AngelMemoryExportRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryExportRecord {
    id: String,
    memory_type: String,
    judgment: String,
    reasoning: String,
    strength: i64,
    is_active: i64,
    useful_count: i64,
    useful_score: f64,
    last_recalled_at: f64,
    last_decay_at: f64,
    memory_scope: String,
    created_at: f64,
    updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryExportTag {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryExportRelation {
    memory_id: String,
    tag_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AngelMemoryTagId {
    Integer(i64),
    String(String),
}

impl AngelMemoryTagId {
    fn lookup_key(&self) -> String {
        match self {
            Self::Integer(value) => value.to_string(),
            Self::String(value) => value.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryImportPayload {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    exported_at: i64,
    #[serde(default)]
    records: Vec<AngelMemoryImportRecord>,
    #[serde(default)]
    global_tags: Vec<AngelMemoryImportTag>,
    #[serde(default)]
    memory_tag_rel: Vec<AngelMemoryImportRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryImportRecord {
    id: String,
    #[serde(default)]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    strength: i64,
    #[serde(default)]
    is_active: i64,
    #[serde(default)]
    useful_count: i64,
    #[serde(default)]
    useful_score: f64,
    #[serde(default)]
    last_recalled_at: f64,
    #[serde(default)]
    last_decay_at: f64,
    #[serde(default)]
    memory_scope: String,
    #[serde(default)]
    created_at: f64,
    #[serde(default)]
    updated_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryImportTag {
    id: AngelMemoryTagId,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AngelMemoryImportRelation {
    memory_id: String,
    tag_id: AngelMemoryTagId,
}

#[derive(Debug, Clone)]
struct AngelNormalizedMemoryRecord {
    id: String,
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
    strength: i64,
    is_active: i64,
    useful_count: i64,
    useful_score: f64,
    last_recalled_at: Option<String>,
    last_decay_at: Option<String>,
    memory_scope: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportAngelMemoriesInput {
    payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportAngelMemoriesSample {
    id: String,
    memory_scope: String,
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportAngelMemoriesScopeItem {
    scope: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewImportAngelMemoriesResult {
    total_count: usize,
    scopes: Vec<PreviewImportAngelMemoriesScopeItem>,
    samples: Vec<PreviewImportAngelMemoriesSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewExportMemoriesScopeItem {
    scope: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewExportMemoriesResult {
    total_count: usize,
    scopes: Vec<PreviewExportMemoriesScopeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAngelMemoriesScopeMapping {
    scope: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAngelMemoriesInput {
    payload: String,
    scope_agent_mappings: Vec<ImportAngelMemoriesScopeMapping>,
}

fn memory_internal_type_to_angel(raw: &str) -> Result<String, String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "knowledge" | "" => Ok("知识记忆".to_string()),
        "skill" => Ok("技能记忆".to_string()),
        "emotion" => Ok("情感记忆".to_string()),
        "event" => Ok("事件记忆".to_string()),
        other => Err(format!("不支持导出记忆类型: {other}")),
    }
}

fn memory_angel_type_to_internal(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let normalized = trimmed.to_ascii_lowercase();
    match normalized.as_str() {
        "knowledge" | "" => Ok("knowledge".to_string()),
        "skill" => Ok("skill".to_string()),
        "emotion" | "emotional" => Ok("emotion".to_string()),
        "event" => Ok("event".to_string()),
        "task" => Err("当前版本暂不支持导入 task 记忆".to_string()),
        _ => match trimmed {
            "知识记忆" => Ok("knowledge".to_string()),
            "技能记忆" => Ok("skill".to_string()),
            "情感记忆" => Ok("emotion".to_string()),
            "事件记忆" => Ok("event".to_string()),
            "任务记忆" => Err("当前版本暂不支持导入任务记忆".to_string()),
            _ => Err(format!("未知记忆类型: {raw}")),
        },
    }
}

fn memory_iso_to_unix_seconds(value: &str) -> Result<f64, String> {
    let text = value.trim();
    if text.is_empty() {
        return Ok(0.0);
    }
    let parsed = parse_iso(text).ok_or_else(|| format!("解析记忆时间失败: {text}"))?;
    Ok((parsed.unix_timestamp_nanos() as f64) / 1_000_000_000.0)
}

fn memory_timestamp_to_iso_required(value: f64) -> Result<String, String> {
    if !value.is_finite() || value <= 0.0 {
        return Ok(now_iso());
    }
    OffsetDateTime::from_unix_timestamp_nanos((value * 1_000_000_000.0).round() as i128)
        .map_err(|err| format!("转换记忆时间失败: {err}"))?
        .format(&Rfc3339)
        .map_err(|err| format!("格式化记忆时间失败: {err}"))
}

fn memory_timestamp_to_iso_optional(value: f64) -> Result<Option<String>, String> {
    if !value.is_finite() || value <= 0.0 {
        return Ok(None);
    }
    Ok(Some(memory_timestamp_to_iso_required(value)?))
}

fn agent_memory_scope_label(agent: &AgentProfile) -> String {
    let name = agent.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let scope = agent.scope.trim();
    if !scope.is_empty() && scope != "global" {
        return scope.to_string();
    }
    agent.id.trim().to_string()
}

fn load_importable_agent_scope_labels(
    state: &AppState,
) -> Result<std::collections::HashMap<String, String>, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| format!("获取会话状态锁失败 {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let data = state_read_app_data_cached(state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    let mut out = std::collections::HashMap::<String, String>::new();
    for agent in &data.agents {
        if agent.is_built_in_user || agent.is_built_in_system {
            continue;
        }
        if private_agent_ids.contains(agent.id.trim()) {
            continue;
        }
        let scope = agent_memory_scope_label(agent);
        if scope.trim().is_empty() {
            continue;
        }
        out.insert(agent.id.clone(), scope);
    }
    drop(guard);
    Ok(out)
}

fn effective_memory_export_scope(
    owner_agent_id: Option<&str>,
    memory_scope: &str,
    owner_scope_by_agent: &std::collections::HashMap<String, String>,
) -> String {
    owner_agent_id
        .and_then(|owner| owner_scope_by_agent.get(owner.trim()))
        .cloned()
        .unwrap_or_else(|| {
            let trimmed = memory_scope.trim();
            if trimmed.is_empty() {
                "public".to_string()
            } else {
                trimmed.to_string()
            }
        })
}

fn build_memory_exchange_payload(
    data_path: &PathBuf,
    owner_scope_by_agent: &std::collections::HashMap<String, String>,
    selected_scopes: Option<&std::collections::HashSet<String>>,
) -> Result<AngelMemoryExportPayload, String> {
    let conn = memory_store_open(data_path)?;
    let mut memory_rows = conn
        .prepare(
            "SELECT id, memory_type, judgment, reasoning, owner_agent_id, strength, is_active, useful_count,
                    useful_score, last_recalled_at, last_decay_at, memory_scope, created_at, updated_at
             FROM memory_record
             ORDER BY datetime(updated_at) DESC, updated_at DESC, id ASC",
        )
        .map_err(|err| format!("准备读取记忆导出记录失败: {err}"))?;

    let records_iter = memory_rows
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, f64>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
            ))
        })
        .map_err(|err| format!("读取记忆导出记录失败: {err}"))?;

    let mut raw_records = Vec::<(String, String, String, String, Option<String>, i64, i64, i64, f64, Option<String>, Option<String>, String, String, String)>::new();
    for row in records_iter {
        raw_records.push(row.map_err(|err| format!("解析记忆导出行失败: {err}"))?);
    }

    let mut tag_stmt = conn
        .prepare(
            "SELECT rel.memory_id, gt.name
             FROM memory_tag_rel rel
             JOIN global_tag gt ON gt.id=rel.tag_id
             ORDER BY rel.memory_id ASC, gt.name ASC",
        )
        .map_err(|err| format!("准备读取记忆标签关系失败: {err}"))?;
    let tag_rows = tag_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|err| format!("读取记忆标签关系失败: {err}"))?;

    let mut tags_by_memory = std::collections::HashMap::<String, Vec<String>>::new();
    for row in tag_rows {
        let (memory_id, tag_name) = row.map_err(|err| format!("解析记忆标签关系行失败: {err}"))?;
        tags_by_memory.entry(memory_id).or_default().push(tag_name);
    }

    let mut records = Vec::<AngelMemoryExportRecord>::new();
    let mut used_tag_names = std::collections::BTreeSet::<String>::new();
    let mut pending_relations = Vec::<(String, String)>::new();
    for (
        id,
        memory_type,
        judgment,
        reasoning,
        owner_agent_id,
        strength,
        is_active,
        useful_count,
        useful_score,
        last_recalled_at,
        last_decay_at,
        memory_scope,
        created_at,
        updated_at,
    ) in raw_records
    {
        let export_memory_type = memory_internal_type_to_angel(&memory_type)?;
        let created_at_ts = memory_iso_to_unix_seconds(&created_at)?;
        let updated_at_ts = memory_iso_to_unix_seconds(&updated_at)?;
        let last_recalled_at_ts = last_recalled_at
            .as_deref()
            .map(memory_iso_to_unix_seconds)
            .transpose()?
            .unwrap_or(0.0);
        let last_decay_at_ts = last_decay_at
            .as_deref()
            .map(memory_iso_to_unix_seconds)
            .transpose()?
            .unwrap_or(0.0);
        let export_scope =
            effective_memory_export_scope(owner_agent_id.as_deref(), &memory_scope, owner_scope_by_agent);
        if let Some(scopes) = selected_scopes {
            if !scopes.contains(export_scope.trim()) {
                continue;
            }
        }

        if let Some(tags) = tags_by_memory.get(&id) {
            for tag_name in tags {
                used_tag_names.insert(tag_name.clone());
                pending_relations.push((id.clone(), tag_name.clone()));
            }
        }

        records.push(AngelMemoryExportRecord {
            id,
            memory_type: export_memory_type,
            judgment,
            reasoning,
            strength,
            is_active,
            useful_count,
            useful_score,
            last_recalled_at: last_recalled_at_ts,
            last_decay_at: last_decay_at_ts,
            memory_scope: export_scope,
            created_at: created_at_ts,
            updated_at: updated_at_ts,
        });
    }

    let mut tag_id_map = std::collections::HashMap::<String, i64>::new();
    let mut global_tags = Vec::<AngelMemoryExportTag>::new();
    for (idx, tag_name) in used_tag_names.into_iter().enumerate() {
        let tag_id = (idx + 1) as i64;
        tag_id_map.insert(tag_name.clone(), tag_id);
        global_tags.push(AngelMemoryExportTag {
            id: tag_id,
            name: tag_name,
        });
    }
    let mut relations = Vec::<AngelMemoryExportRelation>::new();
    for (memory_id, tag_name) in pending_relations {
        if let Some(tag_id) = tag_id_map.get(tag_name.trim()) {
            relations.push(AngelMemoryExportRelation {
                memory_id,
                tag_id: *tag_id,
            });
        }
    }

    Ok(AngelMemoryExportPayload {
        schema_version: 1,
        exported_at: OffsetDateTime::now_utc().unix_timestamp(),
        records,
        global_tags,
        memory_tag_rel: relations,
    })
}

fn parse_angel_memory_payload(payload: &str) -> Result<Vec<AngelNormalizedMemoryRecord>, String> {
    let parsed: AngelMemoryImportPayload =
        serde_json::from_str(payload).map_err(|err| format!("解析记忆备份 JSON 失败: {err}"))?;
    if parsed.records.is_empty() {
        return Ok(Vec::new());
    }

    let mut tag_name_by_id = std::collections::HashMap::<String, String>::new();
    for tag in parsed.global_tags {
        let key = tag.id.lookup_key();
        if key.is_empty() {
            continue;
        }
        let name = tag.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        tag_name_by_id.insert(key, name);
    }

    let mut tags_by_memory = std::collections::HashMap::<String, Vec<String>>::new();
    for rel in parsed.memory_tag_rel {
        let memory_id = rel.memory_id.trim().to_string();
        if memory_id.is_empty() {
            continue;
        }
        let key = rel.tag_id.lookup_key();
        if key.is_empty() {
            continue;
        }
        if let Some(tag_name) = tag_name_by_id.get(&key) {
            tags_by_memory
                .entry(memory_id)
                .or_default()
                .push(tag_name.clone());
        }
    }

    let mut out = Vec::<AngelNormalizedMemoryRecord>::new();
    for item in parsed.records {
        let judgment = clean_text(item.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let internal_type = memory_angel_type_to_internal(&item.memory_type)?;
        let mut tags = tags_by_memory.remove(item.id.trim()).unwrap_or_default();
        tags.sort();
        tags.dedup();
        let normalized_tags = normalize_memory_keywords(&tags);
        if normalized_tags.is_empty() {
            runtime_log_info(format!(
                "[记忆导入] 跳过记录：标签归一化后为空 id={} judgment={} original_tags={:?}",
                item.id.trim(),
                item.judgment.trim(),
                tags
            ));
            continue;
        }

        out.push(AngelNormalizedMemoryRecord {
            id: item.id.trim().to_string(),
            memory_type: internal_type,
            judgment,
            reasoning: clean_text(item.reasoning.trim()),
            tags: normalized_tags,
            strength: item.strength.max(0),
            is_active: if item.is_active > 0 { 1 } else { 0 },
            useful_count: item.useful_count.max(0),
            useful_score: item.useful_score.max(0.0),
            last_recalled_at: memory_timestamp_to_iso_optional(item.last_recalled_at)?,
            last_decay_at: memory_timestamp_to_iso_optional(item.last_decay_at)?,
            memory_scope: {
                let trimmed = item.memory_scope.trim();
                if trimmed.is_empty() {
                    "public".to_string()
                } else {
                    trimmed.to_string()
                }
            },
            created_at: memory_timestamp_to_iso_required(item.created_at)?,
            updated_at: memory_timestamp_to_iso_required(item.updated_at)?,
        });
    }
    Ok(out)
}

fn sampled_angel_memory_preview_items(
    items: &[AngelNormalizedMemoryRecord],
    limit: usize,
) -> Vec<PreviewImportAngelMemoriesSample> {
    if items.len() <= limit {
        return items
            .iter()
            .map(|item| PreviewImportAngelMemoriesSample {
                id: item.id.clone(),
                memory_scope: item.memory_scope.clone(),
                memory_type: item.memory_type.clone(),
                judgment: item.judgment.clone(),
                reasoning: item.reasoning.clone(),
                tags: item.tags.clone(),
                created_at: item.created_at.clone(),
                updated_at: item.updated_at.clone(),
            })
            .collect();
    }

    let mut selected_indices = Vec::<usize>::new();
    let fixed_count = limit.min(3);
    for idx in 0..fixed_count {
        selected_indices.push(idx);
    }
    let mut used = selected_indices
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut seed = (items.len() as u64).saturating_mul(1_103_515_245).saturating_add(12_345);
    while selected_indices.len() < limit {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let idx = fixed_count + ((seed as usize) % (items.len() - fixed_count));
        if used.insert(idx) {
            selected_indices.push(idx);
        }
    }

    selected_indices.sort_unstable();
    selected_indices
        .into_iter()
        .filter_map(|idx| items.get(idx))
        .map(|item| PreviewImportAngelMemoriesSample {
            id: item.id.clone(),
            memory_scope: item.memory_scope.clone(),
            memory_type: item.memory_type.clone(),
            judgment: item.judgment.clone(),
            reasoning: item.reasoning.clone(),
            tags: item.tags.clone(),
            created_at: item.created_at.clone(),
            updated_at: item.updated_at.clone(),
        })
        .collect()
}

fn build_preview_scope_items(
    items: &[AngelNormalizedMemoryRecord],
) -> Vec<PreviewImportAngelMemoriesScopeItem> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for item in items {
        *counts.entry(item.memory_scope.clone()).or_insert(0) += 1;
    }
    let mut out = counts
        .into_iter()
        .map(|(scope, count)| PreviewImportAngelMemoriesScopeItem { scope, count })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.scope.cmp(&b.scope));
    out
}

fn build_export_scope_items(
    data_path: &PathBuf,
    owner_scope_by_agent: &std::collections::HashMap<String, String>,
) -> Result<Vec<PreviewExportMemoriesScopeItem>, String> {
    let conn = memory_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT owner_agent_id, memory_scope
             FROM memory_record
             ORDER BY id ASC",
        )
        .map_err(|err| format!("准备读取导出记忆域失败: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, String>(1)?,
            ))
        })
        .map_err(|err| format!("读取导出记忆域失败: {err}"))?;
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for row in rows {
        let (owner_agent_id, memory_scope) =
            row.map_err(|err| format!("解析导出记忆域失败: {err}"))?;
        let scope = effective_memory_export_scope(
            owner_agent_id.as_deref(),
            &memory_scope,
            owner_scope_by_agent,
        );
        *counts.entry(scope).or_insert(0) += 1;
    }
    let mut out = counts
        .into_iter()
        .map(|(scope, count)| PreviewExportMemoriesScopeItem { scope, count })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| a.scope.cmp(&b.scope));
    Ok(out)
}

fn normalize_selected_export_scopes(
    scopes: &[String],
) -> Result<std::collections::HashSet<String>, String> {
    let selected = scopes
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<std::collections::HashSet<_>>();
    if selected.is_empty() {
        return Err("请至少选择一个记忆域后再导出".to_string());
    }
    Ok(selected)
}

fn resolve_import_scope_targets(
    state: &AppState,
    items: &[AngelNormalizedMemoryRecord],
    mappings: &[ImportAngelMemoriesScopeMapping],
) -> Result<std::collections::HashMap<String, (String, String)>, String> {
    let importable_agents = load_importable_agent_scope_labels(state)?;
    let mut mapping_by_scope = std::collections::HashMap::<String, String>::new();
    for item in mappings {
        let scope = item.scope.trim();
        let agent_id = item.agent_id.trim();
        if scope.is_empty() || agent_id.is_empty() {
            continue;
        }
        mapping_by_scope.insert(scope.to_string(), agent_id.to_string());
    }

    let mut source_scopes = items
        .iter()
        .map(|item| item.memory_scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .collect::<std::collections::BTreeSet<_>>();
    if source_scopes.is_empty() {
        source_scopes.insert("public".to_string());
    }

    let mut out = std::collections::HashMap::<String, (String, String)>::new();
    for scope in source_scopes {
        let target_agent_id = mapping_by_scope
            .get(&scope)
            .ok_or_else(|| format!("作用域 '{}' 尚未选择目标人格", scope))?;
        let target_scope = importable_agents
            .get(target_agent_id)
            .ok_or_else(|| format!("目标人格 '{}' 不存在或不可导入", target_agent_id))?;
        out.insert(scope, (target_agent_id.clone(), target_scope.clone()));
    }
    Ok(out)
}

fn import_angel_memories_by_scope(
    data_path: &PathBuf,
    items: &[AngelNormalizedMemoryRecord],
    scope_targets: &std::collections::HashMap<String, (String, String)>,
) -> Result<MemoryStoreImportStats, String> {
    if items.is_empty() {
        return Ok(MemoryStoreImportStats {
            imported_count: 0,
            created_count: 0,
            merged_count: 0,
            total_count: memory_store_count(data_path)?,
        });
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("开始记忆备份导入事务失败: {err}"))?;

    let draft_tags: Vec<String> = items.iter().flat_map(|d| d.tags.iter().cloned()).collect();
    memory_jieba_add_words(&draft_tags);

    let mut next_memory_no = tx
        .query_row(
            "SELECT COALESCE(MAX(memory_no), 0) + 1 FROM memory_record",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("查询下一个记忆编号失败: {err}"))?
        .max(1) as u64;

    let mut imported_count = 0usize;
    let mut created_count = 0usize;
    let mut merged_count = 0usize;

    for item in items {
        if memory_contains_sensitive(&item.judgment, &item.tags) {
            runtime_log_info(format!(
                "[记忆导入] 跳过记录：命中敏感内容 id={} judgment={} tags={:?}",
                item.id,
                item.judgment,
                item.tags
            ));
            continue;
        }
        let memory_type = memory_store_normalize_memory_type(&item.memory_type)?;
        imported_count += 1;
        let (target_agent_id, target_scope) = scope_targets
            .get(item.memory_scope.trim())
            .cloned()
            .ok_or_else(|| format!("作用域 '{}' 未配置目标人格", item.memory_scope))?;

        let existing_id = tx
            .query_row(
                "SELECT id
                 FROM memory_record
                 WHERE lower(trim(judgment))=lower(trim(?1))
                   AND owner_agent_id=?2
                 LIMIT 1",
                params![item.judgment, target_agent_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| format!("按 judgment 查找已有导入记忆失败: {err}"))?;

        let memory_id = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE memory_record
                 SET memory_type=?1, judgment=?2, reasoning=?3, owner_agent_id=?4,
                     strength=?5, is_active=?6, memory_scope=?7, useful_count=?8,
                     useful_score=?9, last_recalled_at=?10, last_decay_at=?11,
                     created_at=?12, updated_at=?13
                 WHERE id=?14",
                params![
                    memory_type,
                    item.judgment,
                    item.reasoning,
                    target_agent_id,
                    item.strength,
                    item.is_active,
                    target_scope,
                    item.useful_count,
                    item.useful_score,
                    item.last_recalled_at.as_deref(),
                    item.last_decay_at.as_deref(),
                    item.created_at,
                    item.updated_at,
                    id,
                ],
            )
            .map_err(|err| format!("更新已导入记忆失败: {err}"))?;
            tx.execute(
                "UPDATE memory_record
                 SET memory_no=COALESCE(memory_no, ?1)
                 WHERE id=?2",
                params![next_memory_no as i64, id],
            )
            .map_err(|err| format!("回填已导入记忆编号失败: {err}"))?;
            let assigned_no = tx
                .query_row(
                    "SELECT memory_no FROM memory_record WHERE id=?1",
                    params![id.clone()],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .map_err(|err| format!("读取已导入记忆编号失败: {err}"))?
                .unwrap_or(next_memory_no as i64)
                .max(1) as u64;
            next_memory_no = assigned_no.saturating_add(1).max(next_memory_no);
            merged_count += 1;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO memory_record(
                    id, memory_no, memory_type, judgment, reasoning, owner_agent_id,
                    strength, is_active, memory_scope, useful_count, useful_score,
                    last_recalled_at, last_decay_at, created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    id,
                    next_memory_no as i64,
                    memory_type,
                    item.judgment,
                    item.reasoning,
                    target_agent_id,
                    item.strength,
                    item.is_active,
                    target_scope,
                    item.useful_count,
                    item.useful_score,
                    item.last_recalled_at.as_deref(),
                    item.last_decay_at.as_deref(),
                    item.created_at,
                    item.updated_at,
                ],
            )
            .map_err(|err| format!("插入导入记忆失败: {err}"))?;
            next_memory_no += 1;
            created_count += 1;
            id
        };

        memory_store_sync_tags(&tx, &memory_id, &item.tags)?;
        memory_store_sync_memory_fts(&tx, &memory_id)?;
    }

    tx.commit()
        .map_err(|err| format!("提交记忆备份导入事务失败: {err}"))?;
    invalidate_memory_matcher_cache();

    Ok(MemoryStoreImportStats {
        imported_count,
        created_count,
        merged_count,
        total_count: memory_store_count(data_path)?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteMemoryInput {
    memory_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteMemoryResult {
    status: String,
}

#[tauri::command]
fn delete_memory(
    input: DeleteMemoryInput,
    state: State<'_, AppState>,
) -> Result<DeleteMemoryResult, String> {
    memory_store_delete_memory(&state.data_path, &input.memory_id)?;
    Ok(DeleteMemoryResult {
        status: "deleted".to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportMemoriesInput {
    memories: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportMemoriesResult {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedInput {
    query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedResult {
    memories: Vec<SearchMemoriesMixedHit>,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMemoriesMixedHit {
    memory: MemoryEntry,
    bm25_score: f64,
    bm25_raw_score: f64,
    vector_score: f64,
    final_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMemoriesFileResult {
    path: String,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMemoriesToPathInput {
    path: String,
    #[serde(default)]
    scopes: Vec<String>,
}

#[tauri::command]
fn export_memories(state: State<'_, AppState>) -> Result<AngelMemoryExportPayload, String> {
    let owner_scope_by_agent = load_importable_agent_scope_labels(state.inner())?;
    build_memory_exchange_payload(&state.data_path, &owner_scope_by_agent, None)
}

#[tauri::command]
fn preview_export_memories(state: State<'_, AppState>) -> Result<PreviewExportMemoriesResult, String> {
    let owner_scope_by_agent = load_importable_agent_scope_labels(state.inner())?;
    let scopes = build_export_scope_items(&state.data_path, &owner_scope_by_agent)?;
    Ok(PreviewExportMemoriesResult {
        total_count: scopes.iter().map(|item| item.count).sum(),
        scopes,
    })
}

#[tauri::command]
fn export_memories_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ExportMemoriesToPathInput,
) -> Result<ExportMemoriesFileResult, String> {
    let selected_scopes = normalize_selected_export_scopes(&input.scopes)?;
    let owner_scope_by_agent = load_importable_agent_scope_labels(state.inner())?;
    let payload = build_memory_exchange_payload(
        &state.data_path,
        &owner_scope_by_agent,
        Some(&selected_scopes),
    )?;
    let selected = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_save_file();
    let file_path = selected
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "已取消导出".to_string())?;
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("序列化导出记忆备份失败: {err}"))?;
    fs::write(&file_path, body).map_err(|err| format!("写入导出记忆备份失败: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: file_path.to_string_lossy().to_string(),
        count: payload.records.len(),
    })
}

#[tauri::command]
fn export_memories_to_path(
    input: ExportMemoriesToPathInput,
    state: State<'_, AppState>,
) -> Result<ExportMemoriesFileResult, String> {
    let target = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("导出路径不能为空".to_string());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "导出路径缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("创建导出目录失败: {err}"))?;

    let selected_scopes = normalize_selected_export_scopes(&input.scopes)?;
    let owner_scope_by_agent = load_importable_agent_scope_labels(state.inner())?;
    let payload = build_memory_exchange_payload(
        &state.data_path,
        &owner_scope_by_agent,
        Some(&selected_scopes),
    )?;
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("序列化导出记忆备份失败: {err}"))?;
    fs::write(&target, body).map_err(|err| format!("写入导出记忆备份失败: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: target.to_string_lossy().to_string(),
        count: payload.records.len(),
    })
}

#[tauri::command]
fn import_memories(
    input: ImportMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ImportMemoriesResult, String> {
    let stats = memory_store_import_memories(&state.data_path, &input.memories)?;
    Ok(ImportMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn preview_import_angel_memories(
    input: PreviewImportAngelMemoriesInput,
) -> Result<PreviewImportAngelMemoriesResult, String> {
    let parsed = parse_angel_memory_payload(&input.payload)?;
    Ok(PreviewImportAngelMemoriesResult {
        total_count: parsed.len(),
        scopes: build_preview_scope_items(&parsed),
        samples: sampled_angel_memory_preview_items(&parsed, 10),
    })
}

#[tauri::command]
fn import_angel_memories(
    input: ImportAngelMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ImportMemoriesResult, String> {
    let parsed = parse_angel_memory_payload(&input.payload)?;
    let scope_targets =
        resolve_import_scope_targets(state.inner(), &parsed, &input.scope_agent_mappings)?;
    let stats = import_angel_memories_by_scope(&state.data_path, &parsed, &scope_targets)?;
    Ok(ImportMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn search_memories_mixed(
    input: SearchMemoriesMixedInput,
    state: State<'_, AppState>,
) -> Result<SearchMemoriesMixedResult, String> {
    let started = std::time::Instant::now();
    let query = input.query.trim();
    if query.is_empty() {
        // Empty query is intentionally used by the frontend as "browse all memories" mode.
        // Real mixed retrieval always provides non-empty query text.
        return Ok(SearchMemoriesMixedResult {
            memories: memory_store_list_memories(&state.data_path)?
                .into_iter()
                .map(|memory| SearchMemoriesMixedHit {
                    memory,
                    bm25_score: 0.0,
                    bm25_raw_score: 0.0,
                    vector_score: 0.0,
                    final_score: 0.0,
                })
                .collect::<Vec<_>>(),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }

    let memories = memory_store_list_memories(&state.data_path)?;
    let ranked = memory_mixed_ranked_items(
        &state.data_path,
        &memories,
        query,
        MEMORY_MATCH_MAX_ITEMS * MEMORY_CANDIDATE_MULTIPLIER,
    );
    if ranked.is_empty() {
        return Ok(SearchMemoriesMixedResult {
            memories: Vec::new(),
            elapsed_ms: started.elapsed().as_millis(),
        });
    }

    let memory_map = memories
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect::<std::collections::HashMap<_, _>>();
    let mut out = Vec::<SearchMemoriesMixedHit>::new();
    for item in ranked {
        if let Some(memory) = memory_map.get(&item.memory_id) {
            out.push(SearchMemoriesMixedHit {
                memory: memory.clone(),
                bm25_score: item.bm25_score,
                bm25_raw_score: item.bm25_raw_score,
                vector_score: item.vector_score,
                final_score: item.final_score,
            });
        }
    }
    Ok(SearchMemoriesMixedResult {
        memories: out,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("Only http/https URLs are allowed.".to_string());
    }
    webbrowser::open(trimmed).map_err(|err| format!("Open browser failed: {err}"))?;
    Ok(())
}

#[tauri::command]
fn open_workspace_file(relative_path: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = relative_path.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let workspace_root = configured_workspace_root_path(&state)?;
    let target = workspace_root.join(&trimmed);
    let canonical = target
        .canonicalize()
        .map_err(|err| format!("解析文件路径失败: {err}"))?;
    let workspace = workspace_root
        .canonicalize()
        .map_err(|err| format!("解析工作区路径失败: {err}"))?;
    if !canonical.starts_with(&workspace) {
        return Err("仅允许打开工作区内的文件".to_string());
    }
    if !canonical.is_file() {
        return Err("目标文件不存在".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(canonical.as_os_str())
            .spawn()
            .map_err(|err| format!("打开文件失败: {err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(canonical.as_os_str())
            .spawn()
            .map_err(|err| format!("打开文件失败: {err}"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(canonical.as_os_str())
            .spawn()
            .map_err(|err| format!("打开文件失败: {err}"))?;
        return Ok(());
    }
}

#[cfg(test)]
mod memory_exchange_tests {
    use super::*;

    fn temp_data_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("{}_{}", name, Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp dir");
        root.join("app_data.json")
    }

    #[test]
    fn parse_angel_payload_should_restore_tags_and_types() {
        let payload = serde_json::json!({
            "schema_version": 1,
            "exported_at": 1776616786i64,
            "records": [
                {
                    "id": "m1",
                    "memory_type": "知识记忆",
                    "judgment": "用户喜欢简洁回答",
                    "reasoning": "多次提到",
                    "strength": 10,
                    "is_active": 0,
                    "useful_count": 1,
                    "useful_score": 2.5,
                    "last_recalled_at": 1776616786.0,
                    "last_decay_at": 0.0,
                    "memory_scope": "public",
                    "created_at": 1776616000.0,
                    "updated_at": 1776616786.0
                }
            ],
            "global_tags": [
                { "id": 1, "name": "偏好" },
                { "id": 2, "name": "简洁" }
            ],
            "memory_tag_rel": [
                { "memory_id": "m1", "tag_id": 1 },
                { "memory_id": "m1", "tag_id": 2 }
            ]
        });

        let parsed = parse_angel_memory_payload(&serde_json::to_string(&payload).unwrap())
            .expect("parse angel payload");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].memory_type, "knowledge");
        assert_eq!(parsed[0].tags, vec!["偏好".to_string(), "简洁".to_string()]);
        assert_eq!(parsed[0].memory_scope, "public");
        assert!(!parsed[0].created_at.is_empty());
    }

    #[test]
    fn preview_scope_items_should_count_each_scope() {
        let items = vec![
            AngelNormalizedMemoryRecord {
                id: "a".to_string(),
                memory_type: "knowledge".to_string(),
                judgment: "a".to_string(),
                reasoning: String::new(),
                tags: vec!["x".to_string()],
                strength: 0,
                is_active: 1,
                useful_count: 0,
                useful_score: 0.0,
                last_recalled_at: None,
                last_decay_at: None,
                memory_scope: "派蒙".to_string(),
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            AngelNormalizedMemoryRecord {
                id: "b".to_string(),
                memory_type: "knowledge".to_string(),
                judgment: "b".to_string(),
                reasoning: String::new(),
                tags: vec!["y".to_string()],
                strength: 0,
                is_active: 1,
                useful_count: 0,
                useful_score: 0.0,
                last_recalled_at: None,
                last_decay_at: None,
                memory_scope: "派蒙".to_string(),
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            AngelNormalizedMemoryRecord {
                id: "c".to_string(),
                memory_type: "event".to_string(),
                judgment: "c".to_string(),
                reasoning: String::new(),
                tags: vec!["z".to_string()],
                strength: 0,
                is_active: 1,
                useful_count: 0,
                useful_score: 0.0,
                last_recalled_at: None,
                last_decay_at: None,
                memory_scope: "助理".to_string(),
                created_at: now_iso(),
                updated_at: now_iso(),
            },
        ];
        let scopes = build_preview_scope_items(&items);
        assert_eq!(scopes.len(), 2);
        assert_eq!(scopes[0].scope, "助理");
        assert_eq!(scopes[0].count, 1);
        assert_eq!(scopes[1].scope, "派蒙");
        assert_eq!(scopes[1].count, 2);
    }

    #[test]
    fn export_payload_should_use_angel_shape() {
        let data_path = temp_data_path("memory_exchange_export");
        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "Alice likes rust".to_string(),
            reasoning: "test".to_string(),
            tags: vec!["alice".to_string(), "rust".to_string()],
            owner_agent_id: Some("agent-a".to_string()),
        }];
        memory_store_upsert_drafts(&data_path, &drafts).expect("seed memories");

        let payload = build_memory_exchange_payload(
            &data_path,
            &std::collections::HashMap::from([("agent-a".to_string(), "派蒙".to_string())]),
            None,
        ).expect("build exchange payload");
        assert_eq!(payload.schema_version, 1);
        assert_eq!(payload.records.len(), 1);
        assert_eq!(payload.records[0].memory_type, "知识记忆");
        assert_eq!(payload.records[0].memory_scope, "派蒙");
        assert_eq!(payload.global_tags.len(), 2);
        assert_eq!(payload.memory_tag_rel.len(), 2);
    }

    #[test]
    fn export_payload_should_filter_selected_scopes() {
        let data_path = temp_data_path("memory_exchange_export_scopes");
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "Alice likes rust".to_string(),
                reasoning: "test".to_string(),
                tags: vec!["alice".to_string()],
                owner_agent_id: Some("agent-a".to_string()),
            },
            MemoryDraftInput {
                memory_type: "event".to_string(),
                judgment: "global event".to_string(),
                reasoning: "test".to_string(),
                tags: vec!["global".to_string()],
                owner_agent_id: None,
            },
        ];
        memory_store_upsert_drafts(&data_path, &drafts).expect("seed memories");

        let payload = build_memory_exchange_payload(
            &data_path,
            &std::collections::HashMap::from([("agent-a".to_string(), "派蒙".to_string())]),
            Some(&std::collections::HashSet::from(["派蒙".to_string()])),
        ).expect("build filtered exchange payload");
        assert_eq!(payload.records.len(), 1);
        assert_eq!(payload.records[0].memory_scope, "派蒙");
        assert_eq!(payload.global_tags.len(), 1);
        assert_eq!(payload.memory_tag_rel.len(), 1);
    }

    #[test]
    fn import_angel_memories_by_scope_should_preserve_core_fields() {
        let data_path = temp_data_path("memory_exchange_import");
        let payload = serde_json::json!({
            "schema_version": 1,
            "exported_at": 1776616786i64,
            "records": [
                {
                    "id": "m1",
                    "memory_type": "知识记忆",
                    "judgment": "用户喜欢简洁回答",
                    "reasoning": "多次提到",
                    "strength": 9,
                    "is_active": 1,
                    "useful_count": 3,
                    "useful_score": 4.5,
                    "last_recalled_at": 1776616786.0,
                    "last_decay_at": 0.0,
                    "memory_scope": "派蒙",
                    "created_at": 1776616000.0,
                    "updated_at": 1776616786.0
                }
            ],
            "global_tags": [
                { "id": 1, "name": "偏好" }
            ],
            "memory_tag_rel": [
                { "memory_id": "m1", "tag_id": 1 }
            ]
        });
        let parsed = parse_angel_memory_payload(&serde_json::to_string(&payload).unwrap())
            .expect("parse angel payload");
        let scope_targets = std::collections::HashMap::from([(
            "派蒙".to_string(),
            ("agent-a".to_string(), "派蒙".to_string()),
        )]);
        let stats = import_angel_memories_by_scope(&data_path, &parsed, &scope_targets)
            .expect("import angel memories");
        assert_eq!(stats.created_count, 1);

        let conn = memory_store_open(&data_path).expect("open memory db");
        let row = conn
            .query_row(
                "SELECT strength, is_active, useful_count, useful_score, owner_agent_id, memory_scope
                 FROM memory_record
                 WHERE judgment=?1",
                params!["用户喜欢简洁回答"],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, f64>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .expect("query imported memory");
        assert_eq!(row.0, 9);
        assert_eq!(row.1, 1);
        assert_eq!(row.2, 3);
        assert_eq!(row.3, 4.5);
        assert_eq!(row.4.as_deref(), Some("agent-a"));
        assert_eq!(row.5, "派蒙");
    }
}
