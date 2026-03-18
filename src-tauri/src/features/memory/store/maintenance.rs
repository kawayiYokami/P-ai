fn memory_store_rebuild_indexes(data_path: &PathBuf) -> Result<MemoryStoreRebuildReport, String> {
    // Inject all tags into jieba before rebuilding FTS so tokenization keeps known terms intact.
    {
        let conn = memory_store_open(data_path)?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT name FROM global_tag")
            .map_err(|err| format!("Prepare list all tags for jieba failed: {err}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Query all tags for jieba failed: {err}"))?;
        let mut all_tags = Vec::<String>::new();
        for row in rows {
            all_tags.push(row.map_err(|err| format!("Read tag for jieba failed: {err}"))?);
        }
        memory_jieba_add_words(&all_tags);
    }

    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin rebuild indexes tx failed: {err}"))?;

    tx.execute("DELETE FROM memory_fts", [])
        .map_err(|err| format!("Clear memory_fts failed: {err}"))?;
    tx.execute("DELETE FROM note_fts", [])
        .map_err(|err| format!("Clear note_fts failed: {err}"))?;

    {
        let mut memory_stmt = tx
            .prepare("SELECT id FROM memory_record")
            .map_err(|err| format!("Prepare list memory ids failed: {err}"))?;
        let memory_ids = memory_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| format!("Query list memory ids failed: {err}"))?;
        for id in memory_ids {
            let memory_id = id.map_err(|err| format!("Read memory id failed: {err}"))?;
            memory_store_sync_memory_fts(&tx, &memory_id)?;
        }
    }

    tx.execute(
        "INSERT INTO note_fts(item_id, tags)
         SELECT n.source_id, COALESCE(GROUP_CONCAT(t.name, ' '), '')
         FROM note_index_record n
         LEFT JOIN note_tag_rel r ON r.source_id = n.source_id
         LEFT JOIN global_tag t ON t.id = r.tag_id
         GROUP BY n.source_id",
        [],
    )
    .map_err(|err| format!("Rebuild note_fts failed: {err}"))?;

    tx.commit()
        .map_err(|err| format!("Commit rebuild indexes tx failed: {err}"))?;

    let conn = memory_store_open(data_path)?;
    let memory_rows = conn
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?
        .max(0) as usize;
    let memory_fts_rows = conn
        .query_row("SELECT COUNT(1) FROM memory_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_fts failed: {err}"))?
        .max(0) as usize;
    let note_rows = conn
        .query_row("SELECT COUNT(1) FROM note_index_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_index_record failed: {err}"))?
        .max(0) as usize;
    let note_fts_rows = conn
        .query_row("SELECT COUNT(1) FROM note_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_fts failed: {err}"))?
        .max(0) as usize;

    Ok(MemoryStoreRebuildReport {
        memory_rows,
        memory_fts_rows,
        note_rows,
        note_fts_rows,
    })
}

fn memory_store_health_check(
    data_path: &PathBuf,
    auto_repair: bool,
) -> Result<MemoryStoreHealthReport, String> {
    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin health check tx failed: {err}"))?;

    let memory_rows = tx
        .query_row("SELECT COUNT(1) FROM memory_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_record failed: {err}"))?
        .max(0) as usize;
    let memory_fts_rows = tx
        .query_row("SELECT COUNT(1) FROM memory_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count memory_fts failed: {err}"))?
        .max(0) as usize;
    let note_rows = tx
        .query_row("SELECT COUNT(1) FROM note_index_record", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_index_record failed: {err}"))?
        .max(0) as usize;
    let note_fts_rows = tx
        .query_row("SELECT COUNT(1) FROM note_fts", [], |row| row.get::<_, i64>(0))
        .map_err(|err| format!("Count note_fts failed: {err}"))?
        .max(0) as usize;

    let orphan_memory_tag_rows = tx
        .query_row(
            "SELECT COUNT(1)
             FROM memory_tag_rel r
             LEFT JOIN memory_record m ON m.id=r.memory_id
             LEFT JOIN global_tag t ON t.id=r.tag_id
             WHERE m.id IS NULL OR t.id IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("Count orphan memory_tag_rel failed: {err}"))?
        .max(0) as usize;
    let orphan_note_tag_rows = tx
        .query_row(
            "SELECT COUNT(1)
             FROM note_tag_rel r
             LEFT JOIN note_index_record n ON n.source_id=r.source_id
             LEFT JOIN global_tag t ON t.id=r.tag_id
             WHERE n.source_id IS NULL OR t.id IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("Count orphan note_tag_rel failed: {err}"))?
        .max(0) as usize;

    let mut repaired = false;
    if auto_repair {
        if orphan_memory_tag_rows > 0 {
            tx.execute(
                "DELETE FROM memory_tag_rel
                 WHERE memory_id NOT IN (SELECT id FROM memory_record)
                    OR tag_id NOT IN (SELECT id FROM global_tag)",
                [],
            )
            .map_err(|err| format!("Repair memory_tag_rel failed: {err}"))?;
            repaired = true;
        }
        if orphan_note_tag_rows > 0 {
            tx.execute(
                "DELETE FROM note_tag_rel
                 WHERE source_id NOT IN (SELECT source_id FROM note_index_record)
                    OR tag_id NOT IN (SELECT id FROM global_tag)",
                [],
            )
            .map_err(|err| format!("Repair note_tag_rel failed: {err}"))?;
            repaired = true;
        }
    }

    tx.commit()
        .map_err(|err| format!("Commit health check tx failed: {err}"))?;

    let needs_rebuild =
        memory_rows != memory_fts_rows || note_rows != note_fts_rows || orphan_memory_tag_rows > 0 || orphan_note_tag_rows > 0;
    let refreshed_report = if auto_repair && needs_rebuild {
        let report = memory_store_rebuild_indexes(data_path)?;
        repaired = true;
        Some(report)
    } else {
        None
    };

    let status = if needs_rebuild {
        if auto_repair {
            "repaired"
        } else {
            "warn"
        }
    } else {
        "ok"
    };

    let (memory_fts_rows_final, note_fts_rows_final) = if let Some(report) = refreshed_report {
        (report.memory_fts_rows, report.note_fts_rows)
    } else {
        (memory_fts_rows, note_fts_rows)
    };

    Ok(MemoryStoreHealthReport {
        status: status.to_string(),
        memory_rows,
        memory_fts_rows: memory_fts_rows_final,
        note_rows,
        note_fts_rows: note_fts_rows_final,
        orphan_memory_tag_rows,
        orphan_note_tag_rows,
        repaired,
    })
}

fn memory_store_backup_db(data_path: &PathBuf, target_path: &PathBuf) -> Result<MemoryStoreBackupResult, String> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create backup dir failed ({}): {err}", parent.display()))?;
    }
    let conn = memory_store_open(data_path)?;
    let target_sql = target_path.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{target_sql}';"))
        .map_err(|err| format!("Backup memory db via VACUUM INTO failed: {err}"))?;
    let bytes = fs::metadata(target_path)
        .map_err(|err| format!("Read backup file metadata failed: {err}"))?
        .len();
    Ok(MemoryStoreBackupResult {
        path: target_path.to_string_lossy().to_string(),
        bytes,
    })
}

fn memory_store_restore_db(data_path: &PathBuf, source_path: &PathBuf) -> Result<MemoryStoreBackupResult, String> {
    if !source_path.exists() {
        return Err(format!("Restore source not found: {}", source_path.display()));
    }
    let target = memory_store_db_path(data_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Create memory db dir failed ({}): {err}", parent.display()))?;
    }
    let wal = PathBuf::from(format!("{}-wal", target.to_string_lossy()));
    let shm = PathBuf::from(format!("{}-shm", target.to_string_lossy()));
    if wal.exists() {
        let _ = fs::remove_file(&wal);
    }
    if shm.exists() {
        let _ = fs::remove_file(&shm);
    }
    fs::copy(source_path, &target)
        .map_err(|err| format!("Copy restore db failed: {err}"))?;
    let _ = memory_store_open(data_path)?;
    let _ = memory_store_rebuild_indexes(data_path)?;
    let bytes = fs::metadata(&target)
        .map_err(|err| format!("Read restored db metadata failed: {err}"))?
        .len();
    Ok(MemoryStoreBackupResult {
        path: target.to_string_lossy().to_string(),
        bytes,
    })
}

