fn memory_store_list_memories_visible_for_agent(
    data_path: &PathBuf,
    agent_id: &str,
    private_memory_enabled: bool,
) -> Result<Vec<MemoryEntry>, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return memory_store_list_memories(data_path);
    }
    let all = memory_store_list_memories(data_path)?;
    let filtered = all
        .into_iter()
        .filter(|m| match m.owner_agent_id.as_deref() {
            None => true,
            Some(owner) => private_memory_enabled && owner == target,
        })
        .collect::<Vec<_>>();
    Ok(filtered)
}

fn memory_store_list_memories_by_ids_visible_for_agent(
    data_path: &PathBuf,
    memory_ids: &[String],
    agent_id: &str,
    private_memory_enabled: bool,
) -> Result<Vec<MemoryEntry>, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return memory_store_list_memories_by_ids(data_path, memory_ids);
    }
    let filtered = memory_store_list_memories_by_ids(data_path, memory_ids)?
        .into_iter()
        .filter(|m| match m.owner_agent_id.as_deref() {
            None => true,
            Some(owner) => private_memory_enabled && owner == target,
        })
        .collect::<Vec<_>>();
    Ok(filtered)
}

fn memory_store_list_private_memories_by_agent(
    data_path: &PathBuf,
    agent_id: &str,
) -> Result<Vec<MemoryEntry>, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return Ok(Vec::new());
    }
    let all = memory_store_list_memories(data_path)?;
    Ok(all
        .into_iter()
        .filter(|m| m.owner_agent_id.as_deref() == Some(target))
        .collect::<Vec<_>>())
}

fn memory_store_count_private_memories_by_agent(
    data_path: &PathBuf,
    agent_id: &str,
) -> Result<usize, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return Ok(0);
    }
    let conn = memory_store_open(data_path)?;
    let count = conn
        .query_row(
            "SELECT COUNT(1) FROM memory_record WHERE owner_agent_id=?1",
            params![target],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("Count private memories by agent failed: {err}"))?;
    Ok(count.max(0) as usize)
}

fn memory_store_delete_memories_by_owner_agent_id(
    data_path: &PathBuf,
    agent_id: &str,
) -> Result<usize, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return Ok(0);
    }
    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin owner memory delete transaction failed: {err}"))?;

    tx.execute(
        "DELETE FROM memory_fts WHERE item_id IN (SELECT id FROM memory_record WHERE owner_agent_id=?1)",
        params![target],
    )
    .map_err(|err| format!("Delete owner memory_fts failed: {err}"))?;
    let deleted = tx
        .execute("DELETE FROM memory_record WHERE owner_agent_id=?1", params![target])
        .map_err(|err| format!("Delete owner memory_record failed: {err}"))?;

    tx.commit()
        .map_err(|err| format!("Commit owner memory delete transaction failed: {err}"))?;
    if deleted > 0 {
        invalidate_memory_matcher_cache();
    }
    Ok(deleted)
}

fn memory_entry_allowed_for_profile(memory: &MemoryEntry) -> bool {
    matches!(
        memory.memory_type.trim().to_ascii_lowercase().as_str(),
        "knowledge" | "skill" | "event"
    )
}

const USER_PROFILE_ATTR_TAGS: [&str; 6] = [
    "用户别名",
    "事实属性",
    "技能树",
    "关系图谱",
    "活跃项目",
    "用户要求",
];

fn memory_tag_is_user_profile_category_tag(tag: &str) -> bool {
    USER_PROFILE_ATTR_TAGS.iter().any(|item| *item == tag.trim())
}

fn profile_user_id_tag(user_id: &str) -> Option<String> {
    let trimmed = user_id.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn memory_has_profile_shape_tags(memory: &MemoryEntry) -> bool {
    memory.tags.iter().any(|tag| memory_tag_is_user_profile_category_tag(tag))
}

fn memory_entry_is_profile_memory(memory: &MemoryEntry) -> bool {
    memory_entry_allowed_for_profile(memory) && memory_has_profile_shape_tags(memory)
}

fn memory_store_list_profile_memories_by_user_id_visible_for_agent(
    data_path: &PathBuf,
    user_id: &str,
    agent_id: &str,
    private_memory_enabled: bool,
    limit: usize,
) -> Result<Vec<MemoryEntry>, String> {
    let started_at = std::time::Instant::now();
    let Some(user_id_tag) = profile_user_id_tag(user_id) else {
        runtime_log_debug(format!(
            "[记忆] 用户画像 跳过 任务=快速画像查询 用户={} 私有记忆={} 请求上限={} 原因=用户为空 耗时={}",
            user_id.trim(),
            private_memory_enabled,
            limit,
            started_at.elapsed().as_millis()
        ));
        return Ok(Vec::new());
    };
    let conn = memory_store_open(data_path)?;
    let max_items = if limit == 0 { i64::MAX } else { limit as i64 };
    let target = agent_id.trim();
    let use_private_scope = if target.is_empty() || !private_memory_enabled {
        0i64
    } else {
        1i64
    };
    let sql = if target.is_empty() {
        "SELECT mr.id
         FROM memory_record mr
         WHERE EXISTS (
             SELECT 1
             FROM memory_tag_rel rel
             JOIN global_tag tag ON tag.id = rel.tag_id
             WHERE rel.memory_id = mr.id AND tag.name = ?1
         )
         AND EXISTS (
             SELECT 1
             FROM memory_tag_rel rel
             JOIN global_tag tag ON tag.id = rel.tag_id
             WHERE rel.memory_id = mr.id AND tag.name IN ('用户别名', '事实属性', '技能树', '关系图谱', '活跃项目', '用户要求')
         )
         ORDER BY mr.updated_at DESC
         LIMIT ?2"
    } else {
        "SELECT mr.id
         FROM memory_record mr
         WHERE EXISTS (
             SELECT 1
             FROM memory_tag_rel rel
             JOIN global_tag tag ON tag.id = rel.tag_id
             WHERE rel.memory_id = mr.id AND tag.name = ?1
         )
         AND EXISTS (
             SELECT 1
             FROM memory_tag_rel rel
             JOIN global_tag tag ON tag.id = rel.tag_id
             WHERE rel.memory_id = mr.id AND tag.name IN ('用户别名', '事实属性', '技能树', '关系图谱', '活跃项目', '用户要求')
         )
         AND (
             mr.owner_agent_id IS NULL
             OR (?2 = 1 AND mr.owner_agent_id = ?3)
         )
         ORDER BY mr.updated_at DESC
         LIMIT ?4"
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| format!("Prepare list profile memories by user_id failed: {err}"))?;
    let mut ids = Vec::<String>::new();
    if target.is_empty() {
        let rows = stmt
            .query_map(params![user_id_tag, max_items], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Query list profile memories by user_id failed: {err}"))?;
        for row in rows {
            ids.push(
                row.map_err(|err| {
                    format!("Read profile memory by user_id row failed: {err}")
                })?,
            );
        }
    } else {
        let rows = stmt
            .query_map(params![user_id_tag, use_private_scope, target, max_items], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|err| format!("Query list profile memories by user_id failed: {err}"))?;
        for row in rows {
            ids.push(
                row.map_err(|err| {
                    format!("Read profile memory by user_id row failed: {err}")
                })?,
            );
        }
    }
    if ids.is_empty() {
        runtime_log_info(format!(
            "[记忆] 用户画像 完成 任务=快速画像查询 用户={} 私有记忆={} 请求上限={} 结果数=0 耗时={}",
            user_id.trim(),
            private_memory_enabled,
            limit,
            started_at.elapsed().as_millis()
        ));
        return Ok(Vec::new());
    }
    let mut memories =
        memory_store_list_memories_by_ids_visible_for_agent(data_path, &ids, agent_id, private_memory_enabled)?;
    memories.retain(memory_entry_is_profile_memory);
    let order = ids
        .into_iter()
        .enumerate()
        .map(|(idx, id)| (id, idx))
        .collect::<HashMap<String, usize>>();
    memories.sort_by_key(|memory| {
        order
            .get(memory.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    if limit != 0 && memories.len() > limit {
        memories.truncate(limit);
    }
    runtime_log_info(format!(
        "[记忆] 用户画像 完成 任务=快速画像查询 用户={} 私有记忆={} 请求上限={} 结果数={} 耗时={}",
        user_id.trim(),
        private_memory_enabled,
        limit,
        memories.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(memories)
}

#[allow(dead_code)]
fn memory_store_list_profile_memories_visible_for_agent(
    data_path: &PathBuf,
    agent_id: &str,
    private_memory_enabled: bool,
    limit: usize,
) -> Result<Vec<MemoryEntry>, String> {
    memory_store_list_profile_memories_by_user_id_visible_for_agent(
        data_path,
        USER_PERSONA_ID,
        agent_id,
        private_memory_enabled,
        limit,
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPrivateMemoryExportResult {
    path: String,
    count: usize,
}

fn memory_store_export_agent_private_memories(
    data_path: &PathBuf,
    agent_id: &str,
) -> Result<AgentPrivateMemoryExportResult, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return Err("agent_id is required".to_string());
    }
    let memories = memory_store_list_private_memories_by_agent(data_path, target)?;
    let mut exported_memories = Vec::with_capacity(memories.len());
    for mut memory in memories {
        // Persona-private export is intentionally rewritten as global memories,
        // so users can import into global scope directly.
        memory.owner_agent_id = None;
        exported_memories.push(memory);
    }

    let ts = OffsetDateTime::now_utc().unix_timestamp().to_string();
    let export_dir = app_root_from_data_path(data_path).join("backups").join(ts);
    fs::create_dir_all(&export_dir)
        .map_err(|err| format!("Create private memory backup dir failed: {err}"))?;
    let safe_agent = target.replace('\\', "_").replace('/', "_").replace(':', "_");
    let file_name = format!("agent-{safe_agent}-private-memories.json");
    let path = export_dir.join(file_name);
    let payload = serde_json::json!({
        "version": 1,
        "exportedAt": now_iso(),
        "agentId": target,
        "memories": exported_memories,
    });
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize private memory export payload failed: {err}"))?;
    fs::write(&path, body)
        .map_err(|err| format!("Write private memory export file failed: {err}"))?;

    Ok(AgentPrivateMemoryExportResult {
        path: path.to_string_lossy().to_string(),
        count: payload
            .get("memories")
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or(0),
    })
}

fn memory_store_import_memories_for_agent(
    data_path: &PathBuf,
    incoming: &[MemoryEntry],
    agent_id: &str,
) -> Result<MemoryStoreImportStats, String> {
    let target = agent_id.trim();
    if target.is_empty() {
        return Err("agent_id is required".to_string());
    }
    let mut rewritten = Vec::<MemoryEntry>::with_capacity(incoming.len());
    for item in incoming {
        let mut next = item.clone();
        next.owner_agent_id = Some(target.to_string());
        rewritten.push(next);
    }
    memory_store_import_memories(data_path, &rewritten)
}
