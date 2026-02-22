use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use jieba_rs::Jieba;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value as TantivyValue, FAST, STORED};
use tantivy::tokenizer::{SimpleTokenizer, TextAnalyzer};
use tantivy::{doc, Index};

const MEMORY_MATCH_MAX_ITEMS: usize = 7;
const MEMORY_CANDIDATE_MULTIPLIER: usize = 7;
const MEMORY_ROUTE_CANDIDATE_LIMIT: usize = MEMORY_MATCH_MAX_ITEMS * MEMORY_CANDIDATE_MULTIPLIER;
const MEMORY_WEIGHT_BM25: f64 = 0.3;
const MEMORY_WEIGHT_VECTOR: f64 = 0.7;
const MEMORY_RECALL_MIN_FINAL_SCORE: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMixedRankItem {
    memory_id: String,
    bm25_score: f64,
    bm25_raw_score: f64,
    vector_score: f64,
    final_score: f64,
}

#[derive(Debug, Clone)]
struct CompiledMemoryMatcher {
    signature: String,
    matcher: Option<AhoCorasick>,
    keyword_to_memory_indices: Vec<Vec<usize>>,
}

fn memory_matcher_cache() -> &'static std::sync::Mutex<Option<CompiledMemoryMatcher>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<CompiledMemoryMatcher>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

fn memory_jieba() -> &'static std::sync::Mutex<Jieba> {
    static JIEBA: std::sync::OnceLock<std::sync::Mutex<Jieba>> = std::sync::OnceLock::new();
    JIEBA.get_or_init(|| std::sync::Mutex::new(Jieba::new()))
}

fn memory_jieba_add_words(words: &[String]) {
    if words.is_empty() {
        return;
    }
    if let Ok(mut jieba) = memory_jieba().lock() {
        for word in words {
            let w = word.trim();
            if w.chars().count() >= 2 {
                jieba.add_word(w, None, None);
            }
        }
    }
}

fn memory_tokenize_terms(text: &str, dedup: bool) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let jieba = memory_jieba().lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();
    for term in jieba.cut(text, false) {
        let normalized = term.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if dedup && !seen.insert(normalized.clone()) {
            continue;
        }
        out.push(normalized);
    }
    drop(jieba);

    if out.is_empty() {
        for term in text.split_whitespace() {
            let normalized = term.trim().to_lowercase();
            if normalized.is_empty() {
                continue;
            }
            if dedup && !seen.insert(normalized.clone()) {
                continue;
            }
            out.push(normalized);
        }
    }

    out
}

fn memory_tokenize_query_terms(text: &str) -> Vec<String> {
    let mut terms = memory_tokenize_terms(text, true);

    let compact = text
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.chars().count() >= 2 && !terms.iter().any(|t| t == &compact) {
        terms.push(compact);
    }
    terms
}

fn memory_match_signature(memories: &[MemoryEntry]) -> String {
    let mut hasher = Sha256::new();
    for memory in memories {
        hasher.update(memory.id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(memory.updated_at.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(memory.judgment.as_bytes());
        hasher.update(b"\x1e");
        for kw in &memory.tags {
            hasher.update(kw.as_bytes());
            hasher.update(b"\x1d");
        }
        hasher.update(b"\x1c");
    }
    format!("{:x}", hasher.finalize())
}

fn compile_memory_matcher(memories: &[MemoryEntry]) -> CompiledMemoryMatcher {
    let signature = memory_match_signature(memories);
    let mut patterns = Vec::<String>::new();
    let mut keyword_index = HashMap::<String, usize>::new();
    let mut keyword_to_memory_indices = Vec::<Vec<usize>>::new();

    for (memory_idx, memory) in memories.iter().enumerate() {
        let mut local_seen = HashSet::<String>::new();
        for kw in &memory.tags {
            let normalized = kw.trim().to_lowercase();
            if normalized.len() < 2 || !local_seen.insert(normalized.clone()) {
                continue;
            }
            let idx = if let Some(existing) = keyword_index.get(&normalized).copied() {
                existing
            } else {
                let id = patterns.len();
                patterns.push(normalized.clone());
                keyword_index.insert(normalized, id);
                keyword_to_memory_indices.push(Vec::new());
                id
            };
            keyword_to_memory_indices[idx].push(memory_idx);
        }
    }

    let matcher = if patterns.is_empty() {
        None
    } else {
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(false)
            .build(patterns)
            .ok()
    };

    CompiledMemoryMatcher {
        signature,
        matcher,
        keyword_to_memory_indices,
    }
}

fn get_or_compile_memory_matcher(memories: &[MemoryEntry]) -> CompiledMemoryMatcher {
    let signature = memory_match_signature(memories);
    let cache = memory_matcher_cache();
    if let Ok(guard) = cache.lock() {
        if let Some(compiled) = guard.as_ref() {
            if compiled.signature == signature {
                return compiled.clone();
            }
        }
    }

    let compiled = compile_memory_matcher(memories);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(compiled.clone());
    }
    compiled
}

fn invalidate_memory_matcher_cache() {
    if let Ok(mut guard) = memory_matcher_cache().lock() {
        *guard = None;
    }
}

fn conversation_search_text(conversation: &Conversation) -> String {
    let mut lines = Vec::<String>::new();
    for msg in &conversation.messages {
        if msg.role != "user" {
            continue;
        }
        for part in &msg.parts {
            if let MessagePart::Text { text } = part {
                if !text.trim().is_empty() {
                    lines.push(text.to_lowercase());
                }
            }
        }
    }
    lines.join("\n")
}

fn chat_message_text_lowercase(message: &ChatMessage) -> String {
    let mut parts = Vec::<String>::new();
    for part in &message.parts {
        if let MessagePart::Text { text } = part {
            if !text.trim().is_empty() {
                parts.push(text.to_lowercase());
            }
        }
    }
    parts.join("\n")
}

fn memory_recall_query_text(conversation: &Conversation, latest_user_text: &str) -> String {
    let latest_assistant = conversation
        .messages
        .iter()
        .rev()
        .find(|msg| msg.role == "assistant")
        .map(chat_message_text_lowercase)
        .unwrap_or_default();

    let latest_user = if latest_user_text.trim().is_empty() {
        conversation
            .messages
            .iter()
            .rev()
            .find(|msg| msg.role == "user")
            .map(chat_message_text_lowercase)
            .unwrap_or_default()
    } else {
        latest_user_text.to_lowercase()
    };

    [latest_assistant, latest_user]
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn memory_match_hit_indices(memories: &[MemoryEntry], corpus: &str) -> Vec<(usize, usize)> {
    if memories.is_empty() || corpus.trim().is_empty() {
        return Vec::new();
    }

    let compiled = get_or_compile_memory_matcher(memories);
    let Some(matcher) = compiled.matcher.as_ref() else {
        return Vec::new();
    };

    let mut hit_counts = vec![0usize; memories.len()];
    let mut seen = HashSet::<(usize, usize)>::new();

    for mat in matcher.find_iter(corpus) {
        let keyword_idx = mat.pattern().as_usize();
        if let Some(memory_indices) = compiled.keyword_to_memory_indices.get(keyword_idx) {
            for &memory_idx in memory_indices {
                if seen.insert((memory_idx, keyword_idx)) {
                    hit_counts[memory_idx] += 1;
                }
            }
        }
    }

    let mut hits = hit_counts
        .into_iter()
        .enumerate()
        .filter_map(|(idx, score)| if score >= 1 { Some((idx, score)) } else { None })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    hits
}

fn memory_recall_hit_ids(
    data_path: &PathBuf,
    memories: &[MemoryEntry],
    query_text: &str,
) -> Vec<String> {
    memory_mixed_ranked_items(data_path, memories, query_text, MEMORY_MATCH_MAX_ITEMS)
        .into_iter()
        .filter(|item| item.final_score >= MEMORY_RECALL_MIN_FINAL_SCORE)
        .map(|item| item.memory_id)
        .collect::<Vec<_>>()
}

fn memory_board_ids_from_current_hits(recall_ids: &[String], max_items: usize) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::<String>::new();
    for memory_id in recall_ids {
        if out.len() >= max_items {
            break;
        }
        if seen.insert(memory_id.clone()) {
            out.push(memory_id.clone());
        }
    }
    out
}

fn memory_tantivy_bm25_scores(
    memories: &[MemoryEntry],
    query_text: &str,
    limit: usize,
) -> Result<Vec<(String, f64, f64)>, String> {
    if memories.is_empty() || query_text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let mut schema_builder = Schema::builder();
    let memory_idx_field = schema_builder.add_u64_field("memory_idx", FAST | STORED);
    let indexing = TextFieldIndexing::default()
        .set_tokenizer("zh_ws")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(indexing)
        .set_stored();
    let content_field = schema_builder.add_text_field("content", text_options);
    let schema = schema_builder.build();

    let index = Index::create_in_ram(schema);
    index
        .tokenizers()
        .register("zh_ws", TextAnalyzer::from(SimpleTokenizer::default()));

    let mut writer = index
        .writer(20_000_000)
        .map_err(|err| format!("Create tantivy writer failed: {err}"))?;

    for (idx, memory) in memories.iter().enumerate() {
        let raw = format!("{} {}", memory.judgment.trim(), memory.tags.join(" ").trim())
            .trim()
            .to_string();
        let tokenized = memory_tokenize_terms(&raw, false).join(" ");
        writer
            .add_document(doc!(
                memory_idx_field => idx as u64,
                content_field => tokenized
            ))
            .map_err(|err| format!("Add tantivy document failed: {err}"))?;
    }
    writer
        .commit()
        .map_err(|err| format!("Commit tantivy index failed: {err}"))?;

    let reader = index
        .reader()
        .map_err(|err| format!("Open tantivy reader failed: {err}"))?;
    let searcher = reader.searcher();
    let query_tokens = memory_tokenize_query_terms(query_text).join(" ");
    if query_tokens.trim().is_empty() {
        return Ok(Vec::new());
    }
    let qp = QueryParser::for_index(&index, vec![content_field]);
    let query = qp
        .parse_query(&query_tokens)
        .map_err(|err| format!("Parse tantivy query failed: {err}"))?;
    let hits = searcher
        .search(&query, &TopDocs::with_limit(limit))
        .map_err(|err| format!("Search tantivy bm25 failed: {err}"))?;

    let max_score = hits
        .iter()
        .map(|(score, _)| *score as f64)
        .fold(0.0f64, f64::max);
    let mut out = Vec::<(String, f64, f64)>::new();
    for (score, addr) in hits {
        let doc: tantivy::schema::TantivyDocument = searcher
            .doc(addr)
            .map_err(|err| format!("Read tantivy hit document failed: {err}"))?;
        let idx = doc
            .get_first(memory_idx_field)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .ok_or_else(|| "Read tantivy memory_idx failed".to_string())?;
        let memory_id = memories
            .get(idx)
            .map(|m| m.id.clone())
            .ok_or_else(|| format!("Invalid tantivy memory_idx: {idx}"))?;
        let raw_score = score as f64;
        let normalized = if max_score > 0.0 {
            (raw_score / max_score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push((memory_id, raw_score, normalized));
    }
    Ok(out)
}

fn memory_has_embedding_binding(data_path: &PathBuf) -> bool {
    let Ok(conn) = memory_store_open(data_path) else {
        return false;
    };
    let active = memory_store_get_runtime_state(&conn, "active_index_provider_id")
        .ok()
        .flatten()
        .unwrap_or_default();
    let embedding_api = memory_store_get_runtime_state(&conn, "embedding_api_config_id")
        .ok()
        .flatten()
        .unwrap_or_default();
    !active.trim().is_empty() && !embedding_api.trim().is_empty()
}

fn memory_rerank_provider_from_binding(
    data_path: &PathBuf,
) -> Result<Option<Box<dyn MemoryRerankProvider>>, String> {
    let conn = memory_store_open(data_path)?;
    let rerank_api_id = memory_store_get_runtime_state(&conn, "rerank_api_config_id")?
        .unwrap_or_default();
    if rerank_api_id.trim().is_empty() {
        return Ok(None);
    }

    let app_root = app_root_from_data_path(data_path);
    let config_path = app_root.join("config").join("app_config.toml");
    let app_cfg = read_config(&config_path)?;
    let api = app_cfg
        .api_configs
        .iter()
        .find(|cfg| cfg.id == rerank_api_id)
        .ok_or_else(|| format!("rerank api config '{}' not found", rerank_api_id))?;
    if !matches!(api.request_format, RequestFormat::OpenAIRerank) {
        return Err(format!(
            "request_format '{}' is not rerank protocol",
            api.request_format
        ));
    }
    let provider_cfg = MemoryProviderApiConfig {
        base_url: api.base_url.clone(),
        api_key: api.api_key.clone(),
        model: api.model.clone(),
    };
    let provider = memory_create_rerank_provider(
        MemoryProviderKind::VllmRerank,
        &provider_cfg,
        Some(api.model.trim()),
    )?;
    Ok(Some(provider))
}

fn memory_rerank_scores(
    provider: &dyn MemoryRerankProvider,
    query_text: &str,
    candidate_memories: &[&MemoryEntry],
) -> Result<HashMap<String, f64>, String> {
    if candidate_memories.len() <= 1 {
        return Ok(HashMap::new());
    }
    let docs = candidate_memories
        .iter()
        .map(|m| format!("{} {}", m.judgment.trim(), m.tags.join(" ").trim()).trim().to_string())
        .collect::<Vec<_>>();
    let rows = provider.rerank(query_text, &docs, Some(candidate_memories.len()))?;
    let max_score = rows
        .iter()
        .map(|r| r.relevance_score)
        .filter(|v| v.is_finite())
        .fold(0.0f64, f64::max);
    let mut out = HashMap::<String, f64>::new();
    for row in rows {
        if row.index >= candidate_memories.len() || !row.relevance_score.is_finite() {
            continue;
        }
        let memory_id = candidate_memories[row.index].id.clone();
        let norm = if max_score > 0.0 {
            (row.relevance_score / max_score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.insert(memory_id, norm);
    }
    Ok(out)
}

fn memory_mixed_ranked_items(
    data_path: &PathBuf,
    memories: &[MemoryEntry],
    query_text: &str,
    limit: usize,
) -> Vec<MemoryMixedRankItem> {
    if limit == 0 {
        return Vec::new();
    }
    if memories.is_empty() || query_text.trim().is_empty() {
        return Vec::new();
    }

    memory_store_ensure_jieba_tags(data_path);

    let memory_index = memories
        .iter()
        .enumerate()
        .map(|(idx, memory)| (memory.id.clone(), idx))
        .collect::<HashMap<_, _>>();

    let bm25_hits = memory_tantivy_bm25_scores(memories, query_text, MEMORY_ROUTE_CANDIDATE_LIMIT)
        .unwrap_or_default();
    let mut bm25_map = HashMap::<String, f64>::new();
    let mut bm25_raw_map = HashMap::<String, f64>::new();
    let mut bm25_top_ids = Vec::<String>::new();
    for (memory_id, raw_score, norm_score) in bm25_hits {
        bm25_raw_map.insert(memory_id.clone(), raw_score);
        bm25_map.insert(memory_id.clone(), norm_score);
        bm25_top_ids.push(memory_id);
    }

    let has_embedding = memory_has_embedding_binding(data_path);
    let mut vector_map = HashMap::<String, f64>::new();
    let mut vector_available = false;
    if has_embedding {
        match memory_store_search_vector_scores(
            data_path,
            query_text,
            MEMORY_ROUTE_CANDIDATE_LIMIT,
        ) {
            Ok(rows) => {
                vector_available = true;
                for (memory_id, vector_score) in rows {
                    if vector_score.is_finite() {
                        vector_map.insert(memory_id, vector_score.clamp(0.0, 1.0));
                    }
                }
            }
            Err(err) => {
                eprintln!(
                    "[MEMORY] vector search failed, fallback to bm25-only path. err={}",
                    err
                );
            }
        }
    }

    // Retrieval modes:
    // 1) no embedding + no rerank: BM25 direct output.
    // 2) embedding + no rerank: weighted fusion (vector + BM25).
    // 3) no embedding + rerank: BM25 candidates reranked.
    // 4) embedding + rerank: BM25 + vector candidates union, then reranked.
    let rerank_provider = memory_rerank_provider_from_binding(data_path).ok().flatten();

    let has_rerank = rerank_provider.is_some();
    let effective_has_embedding = has_embedding && vector_available;
    let mut candidate_ids = Vec::<String>::new();
    if has_rerank {
        let mut all = HashSet::<String>::new();
        if effective_has_embedding {
            for id in bm25_top_ids.iter().take(limit) {
                if all.insert(id.clone()) {
                    candidate_ids.push(id.clone());
                }
            }
            let mut vector_pairs = vector_map
                .iter()
                .map(|(id, score)| (id.clone(), *score))
                .collect::<Vec<_>>();
            vector_pairs.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            for (id, _) in vector_pairs.into_iter().take(limit) {
                if all.insert(id.clone()) {
                    candidate_ids.push(id);
                }
            }
        } else {
            for id in bm25_top_ids.iter().take(limit) {
                if all.insert(id.clone()) {
                    candidate_ids.push(id.clone());
                }
            }
        }
    } else if !effective_has_embedding {
        candidate_ids.extend(bm25_top_ids.into_iter().take(limit));
    } else {
        let mut all = HashSet::<String>::new();
        for id in &bm25_top_ids {
            if all.insert(id.clone()) {
                candidate_ids.push(id.clone());
            }
        }
        let mut vector_pairs = vector_map
            .iter()
            .map(|(id, score)| (id.clone(), *score))
            .collect::<Vec<_>>();
        vector_pairs.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (id, _) in vector_pairs {
            if all.insert(id.clone()) {
                candidate_ids.push(id);
            }
        }
    }

    if candidate_ids.is_empty() {
        return Vec::new();
    }

    let mut rerank_map = HashMap::<String, f64>::new();
    let mut rerank_available = false;
    if has_rerank {
        let candidate_memories = candidate_ids
            .iter()
            .filter_map(|id| memory_index.get(id).and_then(|idx| memories.get(*idx)))
            .collect::<Vec<_>>();
        if let Some(provider) = rerank_provider.as_ref() {
            match memory_rerank_scores(provider.as_ref(), query_text, &candidate_memories) {
                Ok(map) => {
                    rerank_available = true;
                    rerank_map = map;
                }
                Err(err) => {
                    eprintln!(
                        "[MEMORY] rerank failed, fallback to non-rerank scoring. err={}",
                        err
                    );
                }
            }
        }
    }

    let mut ranked = candidate_ids
        .into_iter()
        .filter_map(|memory_id| {
            let idx = *memory_index.get(&memory_id)?;
            let bm25_score = bm25_map.get(&memory_id).copied().unwrap_or(0.0);
            let vector_score = vector_map.get(&memory_id).copied().unwrap_or(0.0);
            let final_score = if rerank_available {
                rerank_map.get(&memory_id).copied().unwrap_or(0.0)
            } else if effective_has_embedding {
                MEMORY_WEIGHT_VECTOR * vector_score + MEMORY_WEIGHT_BM25 * bm25_score
            } else {
                bm25_score
            };
            Some((memory_id, idx, final_score))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|a, b| {
        b.2.total_cmp(&a.2)
            .then_with(|| memories[b.1].updated_at.cmp(&memories[a.1].updated_at))
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked
        .into_iter()
        .take(limit)
        .map(|(memory_id, _, final_score)| MemoryMixedRankItem {
            bm25_score: bm25_map.get(&memory_id).copied().unwrap_or(0.0),
            bm25_raw_score: bm25_raw_map.get(&memory_id).copied().unwrap_or(0.0),
            vector_score: vector_map.get(&memory_id).copied().unwrap_or(0.0),
            final_score,
            memory_id,
        })
        .collect::<Vec<_>>()
}

fn memory_store_active_embedding_provider_id(conn: &Connection) -> Result<Option<String>, String> {
    memory_store_get_runtime_state(conn, "active_index_provider_id")
}

fn memory_store_embedding_binding_api_id(conn: &Connection) -> Result<Option<String>, String> {
    memory_store_get_runtime_state(conn, "embedding_api_config_id")
}

fn memory_store_embedding_provider_model_name(
    conn: &Connection,
    provider_id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT model_name FROM embedding_provider WHERE provider_id=?1",
        params![provider_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|err| format!("Query embedding provider model_name failed: {err}"))
}

fn memory_query_embedding_vector(data_path: &PathBuf, query_text: &str) -> Result<Vec<f32>, String> {
    let conn = memory_store_open(data_path)?;
    let provider_id = memory_store_active_embedding_provider_id(&conn)?
        .ok_or_else(|| "active_index_provider_id is empty".to_string())?;
    let api_config_id = memory_store_embedding_binding_api_id(&conn)?
        .ok_or_else(|| "embedding_api_config_id is empty".to_string())?;
    let model_name = memory_store_embedding_provider_model_name(&conn, &provider_id)?;

    let app_root = app_root_from_data_path(data_path);
    let config_path = app_root.join("config").join("app_config.toml");
    let app_cfg = read_config(&config_path)?;
    let api = app_cfg
        .api_configs
        .iter()
        .find(|cfg| cfg.id == api_config_id)
        .ok_or_else(|| {
            format!(
                "embedding api config '{}' not found in app_config.toml",
                api_config_id
            )
        })?;

    let provider_kind = memory_provider_kind_from_id(&provider_id);
    let provider_cfg = MemoryProviderApiConfig {
        base_url: api.base_url.clone(),
        api_key: api.api_key.clone(),
        model: api.model.clone(),
    };
    let provider = memory_create_embedding_provider(
        provider_kind,
        &provider_cfg,
        model_name.as_deref(),
    )?;
    let vectors = provider.embed_batch(&[query_text.to_string()])?;
    let first = vectors
        .into_iter()
        .next()
        .ok_or_else(|| "embedding query returned empty vector list".to_string())?;
    if first.is_empty() {
        return Err("embedding query returned zero-dimension vector".to_string());
    }
    Ok(first)
}

fn memory_cosine_similarity(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return None;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for i in 0..a.len() {
        let av = a[i] as f64;
        let bv = b[i] as f64;
        dot += av * bv;
        na += av * av;
        nb += bv * bv;
    }
    if na <= 0.0 || nb <= 0.0 {
        return None;
    }
    Some(dot / (na.sqrt() * nb.sqrt()))
}

fn memory_store_search_vector_scores(
    data_path: &PathBuf,
    query_text: &str,
    limit: usize,
) -> Result<Vec<(String, f64)>, String> {
    if limit == 0 || query_text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let query_vector = memory_query_embedding_vector(data_path, query_text)?;
    let conn = memory_store_open(data_path)?;
    let provider_id = memory_store_active_embedding_provider_id(&conn)?
        .ok_or_else(|| "active_index_provider_id is empty".to_string())?;
    let vector_conn = memory_store_open_provider_vector_db(data_path, &provider_id)?;
    let table = memory_store_provider_table(&provider_id)?;

    let mut stmt = vector_conn
        .prepare(&format!(
            "SELECT chunk_id, embedding_json FROM {table}"
        ))
        .map_err(|err| format!("Prepare vector table scan failed: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("Query vector table scan failed: {err}"))?;

    let mut scored = Vec::<(String, f64)>::new();
    for row in rows {
        let (chunk_id, embedding_json) =
            row.map_err(|err| format!("Read vector row failed: {err}"))?;
        let vector = serde_json::from_str::<Vec<f32>>(&embedding_json)
            .map_err(|err| format!("Parse vector json failed: {err}"))?;
        let Some(cos) = memory_cosine_similarity(&query_vector, &vector) else {
            continue;
        };
        // Normalize cosine [-1,1] to [0,1] so it can be fused with bm25 relevance.
        let score = ((cos + 1.0) * 0.5).clamp(0.0, 1.0);
        scored.push((chunk_id, score));
    }

    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.truncate(limit);
    Ok(scored)
}

#[cfg(test)]
fn latest_recall_memory_ids(recall_table: &[String], max_items: usize) -> Vec<String> {
    recall_table
        .iter()
        .rev()
        .take(max_items)
        .cloned()
        .collect::<Vec<_>>()
}

fn build_memory_board_xml_from_recall_ids(
    memories: &[MemoryEntry],
    recall_ids: &[String],
) -> Option<String> {
    if memories.is_empty() || recall_ids.is_empty() {
        return None;
    }

    let memory_map = memories
        .iter()
        .map(|memory| (memory.id.as_str(), memory))
        .collect::<HashMap<_, _>>();

    let mut ordered_memories = Vec::<&MemoryEntry>::new();
    for memory_id in recall_ids.iter().take(MEMORY_MATCH_MAX_ITEMS) {
        if let Some(memory) = memory_map.get(memory_id.as_str()) {
            ordered_memories.push(*memory);
        }
    }

    if ordered_memories.is_empty() {
        return None;
    }

    let mut out = String::new();
    out.push_str("<memory_board>\n");
    out.push_str("  <note>这是最新回忆表（最多 7 条），请按需参考，不要编造未命中的记忆。</note>\n");
    out.push_str("  <memories>\n");
    for memory in ordered_memories {
        out.push_str("    <memory>\n");
        out.push_str(&format!(
            "      <content>{}</content>\n",
            xml_escape(&memory.judgment)
        ));
        let reasoning = memory.reasoning.trim();
        let display_reasoning = if reasoning.is_empty() { "无" } else { reasoning };
        out.push_str(&format!(
            "      <reasoning>{}</reasoning>\n",
            xml_escape(display_reasoning)
        ));
        out.push_str("    </memory>\n");
    }
    out.push_str("  </memories>\n");
    out.push_str("</memory_board>");
    Some(out)
}

fn build_memory_board_xml(
    memories: &[MemoryEntry],
    search_text: &str,
    latest_user_text: &str,
) -> Option<String> {
    let mut corpus = String::new();
    corpus.push_str(search_text);
    if !latest_user_text.trim().is_empty() {
        corpus.push('\n');
        corpus.push_str(&latest_user_text.to_lowercase());
    }
    let recall_ids = memory_match_hit_indices(memories, &corpus)
        .into_iter()
        .take(MEMORY_MATCH_MAX_ITEMS)
        .map(|(idx, _)| memories[idx].id.clone())
        .collect::<Vec<_>>();
    build_memory_board_xml_from_recall_ids(memories, &recall_ids)
}
