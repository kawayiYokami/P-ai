fn memory_store_tag_id_by_name(conn: &Connection, tag: &str) -> Result<String, String> {
    let found = conn
        .query_row(
            "SELECT id FROM global_tag WHERE name=?1",
            params![tag],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("Lookup tag failed: {err}"))?;
    if let Some(id) = found {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO global_tag(id, name) VALUES (?1, ?2)",
        params![id, tag],
    )
    .map_err(|err| format!("Insert tag failed: {err}"))?;
    Ok(id)
}

fn memory_store_sync_tags(conn: &Connection, memory_id: &str, tags: &[String]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_tag_rel WHERE memory_id=?1",
        params![memory_id],
    )
    .map_err(|err| format!("Delete memory_tag_rel failed: {err}"))?;

    for kw in tags {
        let tag_id = memory_store_tag_id_by_name(conn, kw)?;
        conn.execute(
            "INSERT OR IGNORE INTO memory_tag_rel(memory_id, tag_id) VALUES (?1, ?2)",
            params![memory_id, tag_id],
        )
        .map_err(|err| format!("Insert memory_tag_rel failed: {err}"))?;
    }
    Ok(())
}

fn memory_store_sync_memory_fts(conn: &Connection, memory_id: &str) -> Result<(), String> {
    let judgment = conn
        .query_row(
            "SELECT judgment FROM memory_record WHERE id=?1",
            params![memory_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("Load memory judgment failed: {err}"))?
        .unwrap_or_default();

    let mut tag_stmt = conn
        .prepare(
            "SELECT gt.name
             FROM memory_tag_rel rel
             JOIN global_tag gt ON gt.id = rel.tag_id
             WHERE rel.memory_id=?1",
        )
        .map_err(|err| format!("Prepare load memory tags failed: {err}"))?;
    let tag_rows = tag_stmt
        .query_map(params![memory_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("Query memory tags failed: {err}"))?;
    let mut tags = Vec::<String>::new();
    for row in tag_rows {
        tags.push(row.map_err(|err| format!("Read memory tag failed: {err}"))?);
    }
    let tags_text = tags.join(" ");
    let raw_fts_text = format!("{} {}", judgment.trim(), tags_text.trim())
        .trim()
        .to_string();
    // Keep FTS indexing tokenization aligned with query tokenization (jieba).
    let fts_doc = memory_tokenize_terms(&raw_fts_text, false).join(" ");

    conn.execute("DELETE FROM memory_fts WHERE item_id=?1", params![memory_id])
        .map_err(|err| format!("Delete memory_fts row failed: {err}"))?;
    conn.execute(
        "INSERT INTO memory_fts(item_id, judgment) VALUES (?1, ?2)",
        params![memory_id, fts_doc],
    )
    .map_err(|err| format!("Insert memory_fts row failed: {err}"))?;

    Ok(())
}

#[cfg(test)]
fn memory_store_search_fts_bm25(
    data_path: &PathBuf,
    query_text: &str,
    limit: usize,
) -> Result<Vec<(String, f64)>, String> {
    if query_text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let terms = memory_tokenize_query_terms(query_text);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let fts_query = terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ");
    if fts_query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let conn = memory_store_open(data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT item_id, bm25(memory_fts, 1.0) AS score
             FROM memory_fts
             WHERE memory_fts MATCH ?1
             ORDER BY score ASC
             LIMIT ?2",
        )
        .map_err(|err| format!("Prepare memory_fts bm25 query failed: {err}"))?;

    let rows = stmt
        .query_map(params![fts_query, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(|err| format!("Query memory_fts bm25 failed: {err}"))?;

    let mut out = Vec::<(String, f64)>::new();
    for row in rows {
        out.push(row.map_err(|err| format!("Read memory_fts bm25 row failed: {err}"))?);
    }
    Ok(out)
}

fn memory_store_upsert_drafts(
    data_path: &PathBuf,
    drafts: &[MemoryDraftInput],
) -> Result<(Vec<MemorySaveUpsertItemResult>, usize), String> {
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[记忆存储] 开始，任务=memory_store_upsert_drafts，drafts={}",
        drafts.len()
    ));
    if drafts.is_empty() {
        return Ok((Vec::new(), memory_store_count(data_path)?));
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin memory upsert transaction failed: {err}"))?;

    // Inject draft tags into jieba so judgment tokenization keeps these terms intact.
    let draft_tags: Vec<String> = drafts
        .iter()
        .flat_map(|d| d.tags.iter().cloned())
        .collect();
    memory_jieba_add_words(&draft_tags);

    let now = now_iso();
    let mut results = Vec::<MemorySaveUpsertItemResult>::new();
    for draft in drafts {
        let memory_type = memory_store_normalize_memory_type(&draft.memory_type)?;
        if memory_contains_sensitive(&draft.judgment, &draft.tags) {
            runtime_log_info(format!(
                "[记忆存储] 跳过，任务=memory_store_upsert_drafts，reason=sensitive_rejected，judgment_len={}",
                draft.judgment.len()
            ));
            results.push(MemorySaveUpsertItemResult {
                saved: false,
                id: None,
                tags: None,
                updated_at: None,
                reason: Some("sensitive_rejected".to_string()),
            });
            continue;
        }

        let existing_id = tx
            .query_row(
                "SELECT id
                 FROM memory_record
                 WHERE lower(trim(judgment))=lower(trim(?1))
                   AND ((owner_agent_id IS NULL AND ?2 IS NULL) OR owner_agent_id=?2)
                 LIMIT 1",
                params![draft.judgment, draft.owner_agent_id.as_deref()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| format!("Lookup existing memory by judgment failed: {err}"))?;

        let memory_id = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE memory_record
                 SET memory_type=?1, judgment=?2, reasoning=?3, owner_agent_id=?4, updated_at=?5
                 WHERE id=?6",
                params![
                    memory_type,
                    draft.judgment,
                    draft.reasoning,
                    draft.owner_agent_id.as_deref(),
                    now,
                    id
                ],
            )
            .map_err(|err| format!("Update memory_record failed: {err}"))?;
            id
        } else {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO memory_record(
                    id, memory_type, judgment, reasoning, owner_agent_id, strength, is_active, memory_scope, useful_count, useful_score, created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, 'public', 0, 0, ?6, ?7)",
                params![
                    id,
                    memory_type,
                    draft.judgment,
                    draft.reasoning,
                    draft.owner_agent_id.as_deref(),
                    now,
                    now
                ],
            )
            .map_err(|err| format!("Insert memory_record failed: {err}"))?;
            id
        };

        memory_store_sync_tags(&tx, &memory_id, &draft.tags)?;
        memory_store_sync_memory_fts(&tx, &memory_id)?;

        results.push(MemorySaveUpsertItemResult {
            saved: true,
            id: Some(memory_id),
            tags: Some(draft.tags.clone()),
            updated_at: Some(now.clone()),
            reason: None,
        });
    }

    tx.commit()
        .map_err(|err| format!("Commit memory upsert transaction failed: {err}"))?;
    invalidate_memory_matcher_cache();

    let total = memory_store_count(data_path)?;
    let success_count = results.iter().filter(|item| item.saved).count();
    let skipped_count = results.len().saturating_sub(success_count);
    runtime_log_info(format!(
        "[记忆存储] 完成，任务=memory_store_upsert_drafts，success_count={}，skipped_count={}，total={}，elapsed_ms={}",
        success_count,
        skipped_count,
        total,
        started_at.elapsed().as_millis()
    ));
    Ok((results, total))
}

fn memory_store_count(data_path: &PathBuf) -> Result<usize, String> {
    let conn = memory_store_open(data_path)?;
    let count = conn
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?;
    Ok(count.max(0) as usize)
}

fn memory_store_list_memories(data_path: &PathBuf) -> Result<Vec<MemoryEntry>, String> {
    let conn = memory_store_open(data_path)?;

    let mut stmt = conn
        .prepare(
            "SELECT id, memory_type, judgment, reasoning, owner_agent_id, created_at, updated_at
             FROM memory_record
             ORDER BY updated_at DESC",
        )
        .map_err(|err| format!("Prepare list memories failed: {err}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|err| format!("Query list memories failed: {err}"))?;

    let mut out = Vec::<MemoryEntry>::new();
    for row in rows {
        let (id, memory_type, judgment, reasoning, owner_agent_id, created_at, updated_at) =
            row.map_err(|err| format!("Read memory row failed: {err}"))?;
        let mut tag_stmt = conn
            .prepare(
                "SELECT t.name
                 FROM memory_tag_rel r
                 JOIN global_tag t ON t.id=r.tag_id
                 WHERE r.memory_id=?1
                 ORDER BY t.name ASC",
            )
            .map_err(|err| format!("Prepare list tags failed: {err}"))?;
        let tags_iter = tag_stmt
            .query_map(params![id.clone()], |tag_row| tag_row.get::<_, String>(0))
            .map_err(|err| format!("Query list tags failed: {err}"))?;
        let mut tags = Vec::<String>::new();
        for tag in tags_iter {
            tags.push(tag.map_err(|err| format!("Read tag row failed: {err}"))?);
        }

        out.push(MemoryEntry {
            id,
            memory_type,
            judgment,
            reasoning,
            tags,
            owner_agent_id,
            created_at,
            updated_at,
        });
    }

    Ok(out)
}

fn memory_store_delete_memory(data_path: &PathBuf, memory_id: &str) -> Result<(), String> {
    let target_id = memory_id.trim();
    if target_id.is_empty() {
        return Err("memory_id is required".to_string());
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin memory delete transaction failed: {err}"))?;

    tx.execute("DELETE FROM memory_fts WHERE item_id=?1", params![target_id])
        .map_err(|err| format!("Delete memory_fts failed: {err}"))?;
    // memory_tag_rel 使用 memory_id 外键并开启 ON DELETE CASCADE，因此无需额外手动删除。
    let affected = tx
        .execute("DELETE FROM memory_record WHERE id=?1", params![target_id])
        .map_err(|err| format!("Delete memory_record failed: {err}"))?;
    if affected == 0 {
        return Err("Memory not found".to_string());
    }

    tx.commit()
        .map_err(|err| format!("Commit memory delete transaction failed: {err}"))?;
    runtime_log_info(format!(
        "[记忆存储] 完成，任务=memory_store_delete_memory，memory_id={}",
        target_id
    ));
    invalidate_memory_matcher_cache();
    Ok(())
}
