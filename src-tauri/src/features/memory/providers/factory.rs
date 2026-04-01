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
