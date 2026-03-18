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
