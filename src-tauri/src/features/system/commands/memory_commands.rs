#[tauri::command]
fn list_memories(state: State<'_, AppState>) -> Result<Vec<MemoryEntry>, String> {
    memory_store_list_memories(&state.data_path)
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
struct MemoryExportPayload {
    version: u32,
    exported_at: String,
    memories: Vec<MemoryEntry>,
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
}

#[tauri::command]
fn export_memories(state: State<'_, AppState>) -> Result<MemoryExportPayload, String> {
    let memories = memory_store_list_memories(&state.data_path)?;

    Ok(MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    })
}

#[tauri::command]
fn export_memories_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExportMemoriesFileResult, String> {
    let memories = memory_store_list_memories(&state.data_path)?;
    let payload = MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    };
    let selected = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_save_file();
    let file_path = selected
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "Export cancelled".to_string())?;
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize export payload failed: {err}"))?;
    fs::write(&file_path, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: file_path.to_string_lossy().to_string(),
        count: payload.memories.len(),
    })
}

#[tauri::command]
fn export_memories_to_path(
    input: ExportMemoriesToPathInput,
    state: State<'_, AppState>,
) -> Result<ExportMemoriesFileResult, String> {
    let target = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("Export path is empty".to_string());
    }
    let parent = target
        .parent()
        .ok_or_else(|| "Export path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("Create export dir failed: {err}"))?;

    let memories = memory_store_list_memories(&state.data_path)?;
    let payload = MemoryExportPayload {
        version: 1,
        exported_at: now_iso(),
        memories,
    };
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize export payload failed: {err}"))?;
    fs::write(&target, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportMemoriesFileResult {
        path: target.to_string_lossy().to_string(),
        count: payload.memories.len(),
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
