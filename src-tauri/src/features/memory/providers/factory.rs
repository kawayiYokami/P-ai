fn memory_provider_kind_from_id(raw: &str) -> MemoryProviderKind {
    let id = raw.trim().to_ascii_lowercase();
    if id.contains("deterministic") || id.contains("local") {
        return MemoryProviderKind::DeterministicLocal;
    }
    if id.contains("gemini") {
        return MemoryProviderKind::GeminiEmbedding;
    }
    if id.contains("rerank") || id.contains("vllm") {
        return MemoryProviderKind::VllmRerank;
    }
    MemoryProviderKind::OpenAIEmbedding
}

fn memory_provider_matches_kind(kind: MemoryProviderKind, cfg: &ApiConfig) -> bool {
    match kind {
        MemoryProviderKind::OpenAIEmbedding => {
            matches!(
                cfg.request_format,
                RequestFormat::OpenAI | RequestFormat::OpenAIEmbedding
            )
        }
        MemoryProviderKind::GeminiEmbedding => {
            matches!(cfg.request_format, RequestFormat::GeminiEmbedding)
        }
        MemoryProviderKind::VllmRerank => {
            matches!(cfg.request_format, RequestFormat::OpenAIRerank)
        }
        MemoryProviderKind::DeterministicLocal => true,
    }
}

fn memory_resolve_provider_api_config(
    app: &AppConfig,
    kind: MemoryProviderKind,
    explicit_api_config_id: Option<&str>,
    provider_id: &str,
) -> Option<MemoryProviderApiConfig> {
    let explicit_id = explicit_api_config_id
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);

    let selected = if let Some(id) = explicit_id {
        app.api_configs.iter().find(|c| c.id == id)
    } else if let Some(hit) = app.api_configs.iter().find(|c| c.id == provider_id.trim()) {
        Some(hit)
    } else {
        app.api_configs
            .iter()
            .find(|cfg| memory_provider_matches_kind(kind, cfg))
    }?;

    Some(MemoryProviderApiConfig {
        base_url: selected.base_url.clone(),
        api_key: selected.api_key.clone(),
        model: selected.model.clone(),
    })
}

fn memory_create_embedding_provider(
    kind: MemoryProviderKind,
    cfg: &MemoryProviderApiConfig,
    model_name: Option<&str>,
) -> Result<Box<dyn MemoryEmbeddingProvider>, String> {
    let model = model_name
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(cfg.model.trim())
        .to_string();
    if model.trim().is_empty() {
        return Err("Embedding model is empty.".to_string());
    }
    match kind {
        MemoryProviderKind::OpenAIEmbedding => Ok(Box::new(OpenAIEmbeddingProvider {
            base_url: cfg.base_url.clone(),
            api_key: cfg.api_key.clone(),
            model,
        })),
        MemoryProviderKind::GeminiEmbedding => Ok(Box::new(GeminiEmbeddingProvider {
            base_url: cfg.base_url.clone(),
            api_key: cfg.api_key.clone(),
            model,
        })),
        MemoryProviderKind::VllmRerank => Err(
            "Provider is rerank-only and cannot be used for embedding sync.".to_string(),
        ),
        MemoryProviderKind::DeterministicLocal => Err(
            "Deterministic provider is handled by memory command directly.".to_string(),
        ),
    }
}

fn memory_create_rerank_provider(
    kind: MemoryProviderKind,
    cfg: &MemoryProviderApiConfig,
    model_name: Option<&str>,
) -> Result<Box<dyn MemoryRerankProvider>, String> {
    let model = model_name
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(cfg.model.trim())
        .to_string();
    if model.trim().is_empty() {
        return Err("Rerank model is empty.".to_string());
    }
    match kind {
        MemoryProviderKind::VllmRerank => Ok(Box::new(VllmRerankProvider {
            base_url: cfg.base_url.clone(),
            api_key: Some(cfg.api_key.clone()),
            model,
        })),
        _ => Err("Provider is not a rerank provider.".to_string()),
    }
}

/// 从 data_path 推导 config 并构造当前 active embedding provider 的 embedder。
/// 返回 (provider_id, model_name, embedder)。未绑定 embedding 或构造失败时返回 None。
/// 供启动期增量同步、记忆增删后增量补向量共用,避免每个调用方各自内联三步。
fn memory_build_active_embedder(
    data_path: &PathBuf,
) -> Option<(String, String, Box<dyn MemoryEmbeddingProvider>)> {
    if !memory_has_embedding_binding(data_path) {
        return None;
    }
    let conn = memory_store_open(data_path).ok()?;
    let provider_id = memory_store_active_embedding_provider_id(&conn)
        .ok()
        .flatten()?;
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return None;
    }
    let model_name = memory_store_embedding_provider_model_name(&conn, provider_id)
        .ok()
        .flatten()?;
    let api_config_id = memory_store_embedding_binding_api_id(&conn)
        .ok()
        .flatten()
        .unwrap_or_default();
    let app_root = app_root_from_data_path(data_path);
    let config_path = app_root.join("config").join("app_config.toml");
    let app_cfg = read_config(&config_path).ok()?;
    let kind = memory_provider_kind_from_id(provider_id);
    let provider_cfg = memory_resolve_provider_api_config(
        &app_cfg,
        kind,
        Some(&api_config_id),
        provider_id,
    )?;
    let embedder = memory_create_embedding_provider(kind, &provider_cfg, Some(&model_name)).ok()?;
    Some((provider_id.to_string(), model_name, embedder))
}

/// 启动期向量增量同步: 用当前 active embedding provider 做差集同步。
/// memory_store_sync_provider_index 本身就是增量语义 (only add 缺失 / delete 多余),
/// provider_id 未变时走 no_op 短路; 未绑定 embedding 时直接返回 no_op。
/// 用于 run_deferred_setup, 不应阻塞启动。
fn memory_sync_vectors_on_startup(data_path: &PathBuf) -> Result<MemoryStoreProviderSyncReport, String> {
    let Some((provider_id, model_name, embedder)) = memory_build_active_embedder(data_path)
        else {
        return Ok(MemoryStoreProviderSyncReport {
            status: "no_op".to_string(),
            old_provider_id: None,
            new_provider_id: String::new(),
            deleted: 0,
            added: 0,
            batch_count: 0,
        });
    };
    memory_store_sync_provider_index(
        data_path,
        &provider_id,
        &model_name,
        64,
        true,
        |texts| embedder.embed_batch(texts),
    )
}

/// 记忆删除后删除向量库中对应的 chunk。失败只记日志, 不影响记忆删除本身
/// (下次启动的差集同步会兜底清理孤儿 chunk)。chunk_id 即 memory_id。
fn memory_sync_vectors_after_delete(data_path: &PathBuf, memory_ids: &[String]) -> bool {
    if memory_ids.is_empty() {
        return true;
    }
    let conn = match memory_store_open(data_path) {
        Ok(c) => c,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_delete 原因=open_db_failed 异常={err}",
            ));
            return false;
        }
    };
    let provider_id = match memory_store_active_embedding_provider_id(&conn) {
        Ok(Some(id)) if !id.trim().is_empty() => id,
        _ => {
            runtime_log_info("[记忆] 向量同步 跳过 任务=after_delete 原因=no_active_provider".to_string());
            return true;
        }
    };
    let vector_conn = match memory_store_open_provider_vector_db(data_path, &provider_id) {
        Ok(c) => c,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_delete 原因=open_vector_db_failed 异常={err}",
            ));
            return false;
        }
    };
    let mut deleted = 0usize;
    for id in memory_ids {
        let target = id.trim();
        if target.is_empty() {
            continue;
        }
        match vector_conn.execute(
            "DELETE FROM memory_vector WHERE chunk_id=?1",
            params![target],
        ) {
            Ok(n) => deleted += n,
            Err(err) => {
                runtime_log_warn(format!(
                    "[记忆] 向量同步 失败 任务=after_delete 异常={err}",
                ));
            }
        }
    }
    runtime_log_info(format!(
        "[记忆] 向量同步 完成 任务=after_delete 请求删除条数={} 实际删除条数={deleted}",
        memory_ids.len(),
    ));
    true
}

/// 记忆 upsert 后增量补向量。对新写入/更新的 memory_id 重新生成 embedding 并 upsert 进向量库。
/// chunk_id 即 memory_id。失败只记日志, 不影响记忆写入 (下次启动差集同步兜底)。
fn memory_sync_vectors_after_upsert(data_path: &PathBuf, memory_ids: &[String]) -> bool {
    let ids: Vec<String> = memory_ids
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        return true;
    }
    let Some((provider_id, _model_name, embedder)) = memory_build_active_embedder(data_path)
    else {
        runtime_log_info("[记忆] 向量同步 跳过 任务=after_upsert 原因=no_active_embedder".to_string());
        return true;
    };
    let started = std::time::Instant::now();
    // 读 memory_record 的 judgment 作为待 embed 文本
    let conn = match memory_store_open(data_path) {
        Ok(c) => c,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_upsert 原因=open_db_failed 异常={err}",
            ));
            return false;
        }
    };
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut stmt = match conn.prepare(&format!(
        "SELECT id, judgment FROM memory_record WHERE id IN ({placeholders})"
    )) {
        Ok(s) => s,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_upsert 原因=prepare_failed 异常={err}",
            ));
            return false;
        }
    };
    let id_params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let pairs = match stmt
        .query_map(id_params.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect::<Vec<(String, String)>>(),
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_upsert 原因=query_failed 异常={err}",
            ));
            return false;
        }
    };
    if pairs.is_empty() {
        return true;
    }
    let texts = pairs.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
    let vectors = match embedder.embed_batch(&texts) {
        Ok(v) => v,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 失败 任务=after_upsert 原因=embed_failed 异常={} 条数={} 耗时毫秒={}",
                err,
                texts.len(),
                started.elapsed().as_millis(),
            ));
            return false;
        }
    };
    if vectors.len() != pairs.len() {
        runtime_log_warn(format!(
            "[记忆] 向量同步 失败 任务=after_upsert 原因=length_mismatch 期望={} 实际={}",
            pairs.len(),
            vectors.len(),
        ));
        return false;
    }
    let vector_conn = match memory_store_open_provider_vector_db(data_path, &provider_id) {
        Ok(c) => c,
        Err(err) => {
            runtime_log_warn(format!(
                "[记忆] 向量同步 跳过 任务=after_upsert 原因=open_vector_db_failed 异常={err}",
            ));
            return false;
        }
    };
    let mut upserted = 0usize;
    for ((chunk_id, _), vector) in pairs.iter().zip(vectors.iter()) {
        let embedding_json = match serde_json::to_string(vector) {
            Ok(s) => s,
            Err(err) => {
                runtime_log_warn(format!(
                    "[记忆] 向量同步 失败 任务=after_upsert 异常=serialize_failed 详情={err}",
                ));
                continue;
            }
        };
        match vector_conn.execute(
            "INSERT INTO memory_vector(chunk_id, embedding_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(chunk_id) DO UPDATE SET embedding_json=excluded.embedding_json, updated_at=excluded.updated_at",
            params![chunk_id, embedding_json, now_iso()],
        ) {
            Ok(_) => upserted += 1,
            Err(err) => {
                runtime_log_warn(format!(
                    "[记忆] 向量同步 失败 任务=after_upsert 异常=upsert_failed 详情={err}",
                ));
            }
        }
    }
    runtime_log_info(format!(
        "[记忆] 向量同步 完成 任务=after_upsert 请求条数={} 成功条数={upserted} 耗时毫秒={}",
        pairs.len(),
        started.elapsed().as_millis(),
    ));
    true
}
