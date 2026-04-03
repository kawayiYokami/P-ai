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

fn memory_store_upsert_profile_memory_links(
    data_path: &PathBuf,
    memory_ids: &[String],
    source: &str,
) -> Result<usize, String> {
    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin profile memory link transaction failed: {err}"))?;
    let now = now_iso();
    let normalized_source = if source.trim().eq_ignore_ascii_case("manual") {
        "manual"
    } else {
        "auto"
    };
    let mut linked_count = 0usize;
    for memory_id in memory_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
    {
        let exists = tx
            .query_row(
                "SELECT 1 FROM memory_record WHERE id=?1 LIMIT 1",
                params![memory_id],
                |_| Ok(1i64),
            )
            .optional()
            .map_err(|err| format!("Check profile memory existence failed: {err}"))?
            .is_some();
        if !exists {
            continue;
        }
        let changed = tx
            .execute(
                "INSERT INTO profile_memory_link(id, memory_id, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(memory_id) DO UPDATE SET source=excluded.source, updated_at=excluded.updated_at",
                params![Uuid::new_v4().to_string(), memory_id, normalized_source, now, now],
            )
            .map_err(|err| format!("Upsert profile_memory_link failed: {err}"))?;
        if changed > 0 {
            linked_count += 1;
        }
    }
    tx.commit()
        .map_err(|err| format!("Commit profile memory link transaction failed: {err}"))?;
    Ok(linked_count)
}

fn memory_store_list_profile_memories_visible_for_agent(
    data_path: &PathBuf,
    agent_id: &str,
    private_memory_enabled: bool,
    limit: usize,
) -> Result<Vec<MemoryEntry>, String> {
    let visible_memories =
        memory_store_list_memories_visible_for_agent(data_path, agent_id, private_memory_enabled)?;
    if visible_memories.is_empty() {
        return Ok(Vec::new());
    }

    let conn = memory_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT memory_id
             FROM profile_memory_link
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|err| format!("Prepare list profile memory links failed: {err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("Query profile memory links failed: {err}"))?;
    let mut ordered_ids = Vec::<String>::new();
    for row in rows {
        ordered_ids.push(row.map_err(|err| format!("Read profile memory link failed: {err}"))?);
    }
    if ordered_ids.is_empty() {
        return Ok(Vec::new());
    }

    let memory_map = visible_memories
        .into_iter()
        .filter(memory_entry_allowed_for_profile)
        .map(|memory| (memory.id.clone(), memory))
        .collect::<HashMap<String, MemoryEntry>>();
    let max_items = if limit == 0 { usize::MAX } else { limit };
    let mut out = Vec::<MemoryEntry>::new();
    for memory_id in ordered_ids {
        if let Some(memory) = memory_map.get(&memory_id) {
            out.push(memory.clone());
        }
        if out.len() >= max_items {
            break;
        }
    }
    Ok(out)
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
