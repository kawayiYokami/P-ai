fn memory_store_import_memories(
    data_path: &PathBuf,
    incoming: &[MemoryEntry],
) -> Result<MemoryStoreImportStats, String> {
    let mut drafts = Vec::<MemoryDraftInput>::new();
    let mut imported_count = 0usize;
    for item in incoming {
        let judgment = clean_text(item.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let tags = normalize_memory_keywords(&item.tags);
        if tags.is_empty() {
            continue;
        }
        let memory_type = memory_store_normalize_memory_type(&item.memory_type)?;
        let reasoning = clean_text(item.reasoning.trim());
        imported_count += 1;
        drafts.push(MemoryDraftInput {
            memory_type,
            judgment,
            reasoning,
            tags,
            owner_agent_id: item.owner_agent_id.clone(),
        });
    }

    let before = memory_store_count(data_path)?;
    let (results, total_count) = memory_store_upsert_drafts(data_path, &drafts)?;
    let created_count = total_count.saturating_sub(before);
    let merged_count = results.iter().filter(|r| r.saved).count().saturating_sub(created_count);

    Ok(MemoryStoreImportStats {
        imported_count,
        created_count,
        merged_count,
        total_count,
    })
}
