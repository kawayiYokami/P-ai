#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncMemoryEmbeddingProviderInput {
    provider_id: String,
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryEmbeddingProviderInput {
    provider_id: Option<String>,
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryRerankProviderInput {
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    documents: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryEmbeddingProviderResult {
    provider_kind: String,
    model_name: String,
    vector_dim: usize,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestMemoryRerankProviderResult {
    provider_kind: String,
    model_name: String,
    elapsed_ms: u128,
    result_count: usize,
    top_index: Option<usize>,
    top_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryEmbeddingBindingInput {
    api_config_id: String,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryRerankBindingInput {
    api_config_id: String,
    #[serde(default)]
    model_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryProviderBindings {
    #[serde(default)]
    embedding_api_config_id: Option<String>,
    #[serde(default)]
    rerank_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveMemoryRerankBindingResult {
    status: String,
    rerank_api_config_id: Option<String>,
    model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEmbeddingSyncProgress {
    status: String,
    done_batches: usize,
    total_batches: usize,
    trace_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryHealthCheckInput {
    #[serde(default)]
    auto_repair: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryBackupInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryRestoreInput {
    path: String,
}

#[tauri::command]
fn sync_memory_embedding_provider(
    input: SyncMemoryEmbeddingProviderInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreProviderSyncReport, String> {
    let provider_id = input.provider_id.trim();
    if provider_id.is_empty() {
        return Err("providerId is required".to_string());
    }
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("");
    let batch_size = input.batch_size.unwrap_or(64).max(1);
    let provider_kind = memory_provider_kind_from_id(provider_id);

    if matches!(provider_kind, MemoryProviderKind::DeterministicLocal) {
        let deterministic_model = if model_name.is_empty() {
            "deterministic-local-embedder"
        } else {
            model_name
        };
        return memory_store_sync_provider_index(
            &state.data_path,
            provider_id,
            deterministic_model,
            batch_size,
            |texts| {
                let mut out = Vec::<Vec<f32>>::new();
                for text in texts {
                    let mut hasher = Sha256::new();
                    hasher.update(provider_id.as_bytes());
                    hasher.update(b"|");
                    hasher.update(text.as_bytes());
                    let digest = hasher.finalize();
                    let mut vec = Vec::<f32>::new();
                    for chunk in digest.chunks(4) {
                        let mut bytes = [0u8; 4];
                        for (idx, b) in chunk.iter().enumerate() {
                            bytes[idx] = *b;
                        }
                        let value = u32::from_le_bytes(bytes) as f32 / u32::MAX as f32;
                        vec.push(value);
                    }
                    out.push(vec);
                }
                Ok(out)
            },
        );
    }

    let app_config = read_config(&state.config_path)?;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        provider_id,
    )
    .ok_or_else(|| {
        format!(
            "No API config matches provider kind '{provider_kind:?}'. Please set apiConfigId."
        )
    })?;
    let embedding_provider = memory_create_embedding_provider(
        provider_kind,
        &provider_cfg,
        if model_name.is_empty() {
            None
        } else {
            Some(model_name)
        },
    )?;
    let model_for_report = if model_name.is_empty() {
        provider_cfg.model.as_str()
    } else {
        model_name
    };

    memory_store_sync_provider_index(
        &state.data_path,
        provider_id,
        model_for_report,
        batch_size,
        |texts| embedding_provider.embed_batch(texts),
    )
}

#[tauri::command]
fn test_memory_embedding_provider(
    input: TestMemoryEmbeddingProviderInput,
    state: State<'_, AppState>,
) -> Result<TestMemoryEmbeddingProviderResult, String> {
    let started = std::time::Instant::now();
    let provider_id = input.provider_id.as_deref().unwrap_or("openai_embedding");
    let provider_kind = memory_provider_kind_from_id(provider_id);
    if matches!(provider_kind, MemoryProviderKind::VllmRerank) {
        return Err("rerank provider cannot be used as embedding provider.".to_string());
    }
    let app_config = read_config(&state.config_path)?;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        provider_id,
    )
    .ok_or_else(|| "No matching API config for embedding test.".to_string())?;
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let provider = memory_create_embedding_provider(provider_kind, &provider_cfg, model_name)?;
    let text = input
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("memory embedding connectivity test")
        .to_string();
    let vectors = provider.embed_batch(&vec![text])?;
    let first = vectors
        .first()
        .ok_or_else(|| "embedding test returned empty vectors".to_string())?;
    let dim = first.len();
    if dim == 0 {
        return Err("embedding test returned zero-dim vector".to_string());
    }
    Ok(TestMemoryEmbeddingProviderResult {
        provider_kind: format!("{provider_kind:?}"),
        model_name: model_name.unwrap_or(provider_cfg.model.trim()).to_string(),
        vector_dim: dim,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[tauri::command]
fn test_memory_rerank_provider(
    input: TestMemoryRerankProviderInput,
    state: State<'_, AppState>,
) -> Result<TestMemoryRerankProviderResult, String> {
    let started = std::time::Instant::now();
    let app_config = read_config(&state.config_path)?;
    let provider_kind = MemoryProviderKind::VllmRerank;
    let provider_cfg = memory_resolve_provider_api_config(
        &app_config,
        provider_kind,
        input.api_config_id.as_deref(),
        "vllm_rerank",
    )
    .ok_or_else(|| "No matching API config for rerank test.".to_string())?;
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let provider = memory_create_rerank_provider(provider_kind, &provider_cfg, model_name)?;
    let query = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("用户偏好什么风格？")
        .to_string();
    let documents = input.documents.unwrap_or_else(|| {
        vec![
            "用户偏好简洁回答，尽量直接结论。".to_string(),
            "用户喜欢复杂铺垫和长篇解释。".to_string(),
            "今天主要讨论了记忆系统检索。".to_string(),
        ]
    });
    let results = provider.rerank(&query, &documents, Some(3))?;
    let top = results.iter().max_by(|a, b| {
        a.relevance_score
            .partial_cmp(&b.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(TestMemoryRerankProviderResult {
        provider_kind: format!("{provider_kind:?}"),
        model_name: model_name.unwrap_or(provider_cfg.model.trim()).to_string(),
        elapsed_ms: started.elapsed().as_millis(),
        result_count: results.len(),
        top_index: top.map(|t| t.index),
        top_score: top.map(|t| t.relevance_score),
    })
}

fn memory_binding_provider_id(api_id: &str, request_format: &str, model: &str) -> String {
    let norm = |raw: &str| -> String {
        let mut out = raw
            .trim()
            .to_ascii_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        while out.contains("__") {
            out = out.replace("__", "_");
        }
        out.trim_matches('_').to_string()
    };
    let id = norm(api_id);
    let fmt = norm(request_format);
    let mdl = norm(model);
    format!("{id}_{fmt}_{mdl}")
}

#[tauri::command]
fn get_memory_provider_bindings(state: State<'_, AppState>) -> Result<MemoryProviderBindings, String> {
    let conn = memory_store_open(&state.data_path)?;
    Ok(MemoryProviderBindings {
        embedding_api_config_id: memory_store_get_runtime_state(&conn, KB_STATE_EMBEDDING_API_CONFIG_ID)?,
        rerank_api_config_id: memory_store_get_runtime_state(&conn, KB_STATE_RERANK_API_CONFIG_ID)?,
    })
}

#[tauri::command]
fn get_memory_embedding_sync_progress(state: State<'_, AppState>) -> Result<MemoryEmbeddingSyncProgress, String> {
    let conn = memory_store_open(&state.data_path)?;
    let status = memory_store_get_runtime_state(&conn, KB_STATE_REBUILD_STATUS)?
        .unwrap_or_else(|| "idle".to_string());
    let done_batches = memory_store_get_runtime_state(&conn, KB_STATE_REBUILD_DONE_BATCHES)?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let total_batches = memory_store_get_runtime_state(&conn, KB_STATE_REBUILD_TOTAL_BATCHES)?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let trace_id = memory_store_get_runtime_state(&conn, KB_STATE_REBUILD_TRACE_ID)?;
    let error = memory_store_get_runtime_state(&conn, KB_STATE_REBUILD_ERROR)?;
    Ok(MemoryEmbeddingSyncProgress {
        status,
        done_batches,
        total_batches,
        trace_id,
        error,
    })
}

#[tauri::command]
fn save_memory_embedding_binding(
    input: SaveMemoryEmbeddingBindingInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreProviderSyncReport, String> {
    let api_id = input.api_config_id.trim();
    if api_id.is_empty() {
        let conn = memory_store_open(&state.data_path)?;
        let old_provider_id = memory_store_get_runtime_state(&conn, KB_STATE_ACTIVE_INDEX_PROVIDER_ID)?;
        memory_store_set_runtime_state(&conn, KB_STATE_EMBEDDING_API_CONFIG_ID, "")?;
        memory_store_set_runtime_state(&conn, KB_STATE_ACTIVE_INDEX_PROVIDER_ID, "")?;
        return Ok(MemoryStoreProviderSyncReport {
            status: "disabled".to_string(),
            old_provider_id,
            new_provider_id: String::new(),
            deleted: 0,
            added: 0,
            batch_count: 0,
        });
    }
    let app_config = read_config(&state.config_path)?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == api_id)
        .cloned()
        .ok_or_else(|| "Selected embedding API config not found.".to_string())?;

    let provider_kind = match api.request_format {
        RequestFormat::OpenAIEmbedding => MemoryProviderKind::OpenAIEmbedding,
        RequestFormat::GeminiEmbedding => MemoryProviderKind::GeminiEmbedding,
        _ => {
            return Err(format!(
                "request_format '{}' is not embedding protocol.",
                api.request_format
            ))
        }
    };
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(api.model.trim());
    if model_name.is_empty() {
        return Err("Embedding model is empty.".to_string());
    }
    let provider_cfg = MemoryProviderApiConfig {
        base_url: api.base_url.clone(),
        api_key: api.api_key.clone(),
        model: api.model.clone(),
    };
    let provider = memory_create_embedding_provider(provider_kind, &provider_cfg, Some(model_name))?;

    let provider_id = memory_binding_provider_id(&api.id, api.request_format.as_str(), model_name);
    let batch_size = input.batch_size.unwrap_or(64).max(1);
    let report = memory_store_sync_provider_index(
        &state.data_path,
        &provider_id,
        model_name,
        batch_size,
        |texts| provider.embed_batch(texts),
    )?;

    let conn = memory_store_open(&state.data_path)?;
    memory_store_set_runtime_state(&conn, KB_STATE_EMBEDDING_API_CONFIG_ID, &api.id)?;
    Ok(report)
}

#[tauri::command]
fn save_memory_rerank_binding(
    input: SaveMemoryRerankBindingInput,
    state: State<'_, AppState>,
) -> Result<SaveMemoryRerankBindingResult, String> {
    let api_id = input.api_config_id.trim();
    if api_id.is_empty() {
        let conn = memory_store_open(&state.data_path)?;
        memory_store_set_runtime_state(&conn, KB_STATE_RERANK_API_CONFIG_ID, "")?;
        return Ok(SaveMemoryRerankBindingResult {
            status: "disabled".to_string(),
            rerank_api_config_id: None,
            model_name: String::new(),
        });
    }
    let app_config = read_config(&state.config_path)?;
    let api = app_config
        .api_configs
        .iter()
        .find(|a| a.id == api_id)
        .cloned()
        .ok_or_else(|| "Selected rerank API config not found.".to_string())?;
    if !matches!(api.request_format, RequestFormat::OpenAIRerank) {
        return Err(format!(
            "request_format '{}' is not rerank protocol.",
            api.request_format
        ));
    }
    let model_name = input
        .model_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(api.model.trim());
    if model_name.is_empty() {
        return Err("Rerank model is empty.".to_string());
    }

    let conn = memory_store_open(&state.data_path)?;
    memory_store_set_runtime_state(&conn, KB_STATE_RERANK_API_CONFIG_ID, &api.id)?;
    Ok(SaveMemoryRerankBindingResult {
        status: "saved".to_string(),
        rerank_api_config_id: Some(api.id),
        model_name: model_name.to_string(),
    })
}

#[tauri::command]
fn memory_rebuild_indexes(state: State<'_, AppState>) -> Result<MemoryStoreRebuildReport, String> {
    memory_store_rebuild_indexes(&state.data_path)
}

#[tauri::command]
fn memory_health_check(
    input: MemoryHealthCheckInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreHealthReport, String> {
    memory_store_health_check(&state.data_path, input.auto_repair)
}

#[tauri::command]
fn memory_backup_db(
    input: MemoryBackupInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreBackupResult, String> {
    let path = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("backup path is empty".to_string());
    }
    memory_store_backup_db(&state.data_path, &path)
}

#[tauri::command]
fn memory_restore_db(
    input: MemoryRestoreInput,
    state: State<'_, AppState>,
) -> Result<MemoryStoreBackupResult, String> {
    let path = PathBuf::from(input.path.trim());
    if input.path.trim().is_empty() {
        return Err("restore path is empty".to_string());
    }
    memory_store_restore_db(&state.data_path, &path)
}
