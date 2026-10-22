#[cfg(test)]
use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value as TantivyValue, FAST,
    STORED,
};
use tantivy::tokenizer::{SimpleTokenizer, TextAnalyzer};
use tantivy::{doc, Index};

const MEMORY_MATCH_MAX_ITEMS: usize = 7;
const MEMORY_CANDIDATE_MULTIPLIER: usize = 7;
const MEMORY_ROUTE_CANDIDATE_LIMIT: usize = MEMORY_MATCH_MAX_ITEMS * MEMORY_CANDIDATE_MULTIPLIER;
const MEMORY_RRF_K: f64 = 60.0;
const MEMORY_RECALL_TOP_SCORE_RATIO: f64 = 0.5;
// rerank 模式绝对门槛: relevance_score 低于门槛视为无关, 不参与最终排序与召回。
// RAG 自动召回用 0.7 (严格), 工具召回用 0.5 (宽松), 浏览类搜索用 0.0 (不过滤)。
const MEMORY_RERANK_MIN_SCORE_RAG: f64 = 0.7;
const MEMORY_RERANK_MIN_SCORE_TOOL: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMixedRankItem {
    memory_id: String,
    bm25_score: f64,
    bm25_raw_score: f64,
    vector_score: f64,
    rerank_score: f64,
    final_score: f64,
}

#[derive(Debug, Clone)]
struct MemoryBm25Hit {
    memory_id: String,
    raw_score: f64,
    normalized_score: f64,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct CompiledMemoryMatcher {
    signature: String,
    matcher: Option<AhoCorasick>,
    keyword_to_memory_indices: Vec<Vec<usize>>,
}

#[cfg(test)]
fn memory_matcher_cache() -> &'static std::sync::Mutex<Option<CompiledMemoryMatcher>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Option<CompiledMemoryMatcher>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

fn memory_is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

fn memory_normalize_search_text(text: &str) -> String {
    let mut out = String::new();
    for ch in text.trim().chars() {
        let normalized = match ch {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            _ => ch,
        };
        out.push(normalized.to_ascii_lowercase());
    }
    out.trim_matches('"').trim().to_string()
}

fn memory_push_token(
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
    token: String,
    dedup: bool,
) {
    if token.is_empty() {
        return;
    }
    if dedup && !seen.insert(token.clone()) {
        return;
    }
    out.push(token);
}

fn memory_tokenize_terms(text: &str, dedup: bool) -> Vec<String> {
    let normalized = memory_normalize_search_text(text);
    if normalized.trim().is_empty() {
        return Vec::new();
    }

    let mut out = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();
    let mut ascii = String::new();
    let mut cjk_run = Vec::<char>::new();

    let flush_ascii = |ascii: &mut String, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if ascii.is_empty() {
            return;
        }
        memory_push_token(out, seen, ascii.clone(), dedup);
        ascii.clear();
    };
    let flush_cjk = |cjk_run: &mut Vec<char>, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if cjk_run.is_empty() {
            return;
        }
        for ch in cjk_run.iter() {
            memory_push_token(out, seen, ch.to_string(), dedup);
        }
        if cjk_run.len() >= 2 {
            for pair in cjk_run.windows(2) {
                memory_push_token(out, seen, format!("{}{}", pair[0], pair[1]), dedup);
            }
        }
        cjk_run.clear();
    };

    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            flush_cjk(&mut cjk_run, &mut out, &mut seen);
            ascii.push(ch);
            continue;
        }
        flush_ascii(&mut ascii, &mut out, &mut seen);
        if memory_is_cjk_char(ch) {
            cjk_run.push(ch);
        } else {
            flush_cjk(&mut cjk_run, &mut out, &mut seen);
        }
    }
    flush_ascii(&mut ascii, &mut out, &mut seen);
    flush_cjk(&mut cjk_run, &mut out, &mut seen);

    out
}

#[cfg(test)]
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

fn memory_split_query_terms(text: &str) -> Vec<String> {
    let normalized = memory_normalize_search_text(text);
    let mut terms = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();
    for part in normalized.split(|ch: char| ch.is_whitespace() || ch == '|') {
        let term = part.trim();
        if term.is_empty() || !seen.insert(term.to_string()) {
            continue;
        }
        terms.push(term.to_string());
    }
    terms
}

fn memory_required_query_terms(query: &str) -> Vec<String> {
    let normalized = memory_normalize_search_text(query);
    let mut out = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();
    let mut ascii = String::new();
    let mut cjk_run = Vec::<char>::new();

    let flush_ascii = |ascii: &mut String, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if ascii.is_empty() {
            return;
        }
        memory_push_token(out, seen, ascii.clone(), true);
        ascii.clear();
    };
    let flush_cjk = |cjk_run: &mut Vec<char>, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if cjk_run.is_empty() {
            return;
        }
        if cjk_run.len() <= 2 {
            let term = cjk_run.iter().collect::<String>();
            memory_push_token(out, seen, term, true);
        } else {
            for pair in cjk_run.windows(2) {
                memory_push_token(out, seen, format!("{}{}", pair[0], pair[1]), true);
            }
        }
        cjk_run.clear();
    };

    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            flush_cjk(&mut cjk_run, &mut out, &mut seen);
            ascii.push(ch);
            continue;
        }
        flush_ascii(&mut ascii, &mut out, &mut seen);
        if memory_is_cjk_char(ch) {
            cjk_run.push(ch);
        } else {
            flush_cjk(&mut cjk_run, &mut out, &mut seen);
        }
    }
    flush_ascii(&mut ascii, &mut out, &mut seen);
    flush_cjk(&mut cjk_run, &mut out, &mut seen);

    out
}

fn memory_escape_query_term(term: &str) -> String {
    let mut out = String::new();
    for ch in term.chars() {
        if matches!(
            ch,
            '\\' | '"' | '+' | '-' | '!' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | '~'
                | '*' | '?' | ':' | '/'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn memory_build_all_terms_query(field_name: &str, terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("+{}:\"{}\"", field_name, memory_escape_query_term(term)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn memory_build_any_terms_query(field_name: &str, terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("{}:\"{}\"", field_name, memory_escape_query_term(term)))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn memory_resolve_hit_limit(query: &str, base_limit: usize) -> usize {
    let compact = memory_normalize_search_text(query)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let has_cjk = compact.chars().any(memory_is_cjk_char);
    let len = compact.chars().count();
    if has_cjk && len <= 1 {
        base_limit.max(800)
    } else if has_cjk && len <= 3 {
        base_limit.max(240)
    } else {
        base_limit
    }
}

fn memory_rrf_score_for_id(
    memory_id: &str,
    bm25_rank_map: &HashMap<String, usize>,
    vector_rank_map: Option<&HashMap<String, usize>>,
) -> f64 {
    let bm25_rrf = bm25_rank_map
        .get(memory_id)
        .map(|rank| 1.0 / (MEMORY_RRF_K + *rank as f64))
        .unwrap_or(0.0);
    let vector_rrf = vector_rank_map
        .and_then(|rank_map| rank_map.get(memory_id))
        .map(|rank| 1.0 / (MEMORY_RRF_K + *rank as f64))
        .unwrap_or(0.0);
    bm25_rrf + vector_rrf
}

#[cfg(test)]
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
    bytes_to_lower_hex(hasher.finalize())
}

#[cfg(test)]
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

#[cfg(test)]
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
    #[cfg(test)]
    {
        if let Ok(mut guard) = memory_matcher_cache().lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
fn conversation_search_text(conversation: &Conversation) -> String {
    let mut lines = Vec::<String>::new();
    for msg in &conversation.messages {
        if msg.role != "user" {
            continue;
        }
        for part in &msg.parts {
            if let MessagePart::Text { text, .. } = part {
                if !text.trim().is_empty() {
                    lines.push(text.to_lowercase());
                }
            }
        }
    }
    lines.join("\n")
}

fn memory_extract_query_tags_from_text(memories: &[MemoryEntry], latest_user_text: &str) -> Vec<String> {
    let lowered = latest_user_text.to_lowercase();
    if lowered.trim().is_empty() {
        return Vec::new();
    }

    let mut seen_lower = HashSet::<String>::new();
    let mut tags = memories
        .iter()
        .flat_map(|memory| memory.tags.iter())
        .map(|tag| tag.trim())
        .filter(|tag| lowered.contains(&tag.to_lowercase()))
        .filter(|tag| seen_lower.insert(tag.to_lowercase()))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    tags.sort_by(|a, b| {
        b.chars()
            .count()
            .cmp(&a.chars().count())
            .then_with(|| a.cmp(b))
    });
    tags.truncate(24);
    tags
}

fn memory_search_query_text(memories: &[MemoryEntry], query_text: &str) -> String {
    let trimmed = query_text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() > 100 {
        let matched_tags = memory_extract_query_tags_from_text(memories, trimmed);
        if !matched_tags.is_empty() {
            return matched_tags.join("\n");
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
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
    rerank_min_score: f64,
) -> Vec<String> {
    let ranked = memory_mixed_ranked_items(
        data_path,
        memories,
        query_text,
        MEMORY_MATCH_MAX_ITEMS,
        rerank_min_score,
    );
    memory_recall_ids_from_ranked_items(ranked)
}

fn memory_recall_ids_from_ranked_items(ranked: Vec<MemoryMixedRankItem>) -> Vec<String> {
    let top_score = ranked
        .first()
        .map(|item| item.final_score)
        .filter(|score| score.is_finite() && *score > 0.0)
        .unwrap_or(0.0);
    if top_score <= 0.0 {
        return Vec::new();
    }
    let threshold = top_score * MEMORY_RECALL_TOP_SCORE_RATIO;
    ranked
        .into_iter()
        .filter(|item| item.final_score.is_finite() && item.final_score >= threshold)
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
) -> Result<Vec<MemoryBm25Hit>, String> {
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
    let qp = QueryParser::for_index(&index, vec![content_field]);

    fn search_tantivy_stage(
        qp: &QueryParser,
        searcher: &tantivy::Searcher,
        memory_idx_field: Field,
        query_text: &str,
        fetch_limit: usize,
    ) -> Result<Vec<(usize, f64)>, String> {
        if query_text.trim().is_empty() || fetch_limit == 0 {
            return Ok(Vec::new());
        }
        let parsed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| qp.parse_query(query_text)));
        let query = match parsed {
            Ok(Ok(query)) => query,
            Ok(Err(err)) => return Err(format!("Parse tantivy query failed: {err}")),
            Err(_) => {
                return Err(
                    "Parse tantivy query panicked (invalid grammar input); query was rejected safely."
                        .to_string(),
                )
            }
        };
        let collector = TopDocs::with_limit(fetch_limit).order_by_score();
        let searched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            searcher.search(&query, &collector)
        }));
        let hits: Vec<(f32, tantivy::DocAddress)> = match searched {
            Ok(Ok(hits)) => hits,
            Ok(Err(err)) => return Err(format!("Search tantivy bm25 failed: {err}")),
            Err(_) => {
                return Err(
                    "Search tantivy bm25 panicked unexpectedly; search was aborted safely."
                        .to_string(),
                )
            }
        };

        let mut out = Vec::<(usize, f64)>::new();
        for (score, addr) in hits {
            let doc: tantivy::schema::TantivyDocument = searcher
                .doc(addr)
                .map_err(|err| format!("Read tantivy hit document failed: {err}"))?;
            let idx = doc
                .get_first(memory_idx_field)
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .ok_or_else(|| "Read tantivy memory_idx failed".to_string())?;
            out.push((idx, score as f64));
        }
        Ok(out)
    }

    fn merge_single_term_hits(
        memories: &[MemoryEntry],
        searcher: &tantivy::Searcher,
        qp: &QueryParser,
        memory_idx_field: Field,
        content_field_name: &str,
        term: &str,
        limit: usize,
    ) -> Result<Vec<(usize, f64, bool)>, String> {
        let required_terms = memory_required_query_terms(term);
        let all_terms = memory_tokenize_terms(term, true);
        if required_terms.is_empty() && all_terms.is_empty() {
            return Ok(Vec::new());
        }

        let base_hit_limit = limit.saturating_mul(8).max(50);
        let hit_limit = memory_resolve_hit_limit(term, base_hit_limit);
        let strict_query = memory_build_all_terms_query(content_field_name, &required_terms);
        let loose_query = memory_build_any_terms_query(content_field_name, &all_terms);

        let mut seen = HashSet::<usize>::new();
        let mut merged = Vec::<(usize, f64, usize)>::new();

        for (stage_rank, query_text) in [(0usize, strict_query), (1usize, loose_query)] {
            if query_text.trim().is_empty() {
                continue;
            }
            if stage_rank == 1 && merged.len() >= limit {
                break;
            }
            let stage_hits = search_tantivy_stage(
                qp,
                searcher,
                memory_idx_field,
                &query_text,
                hit_limit.min(memories.len().max(1)),
            )?;
            for (idx, score) in stage_hits {
                if seen.insert(idx) {
                    merged.push((idx, score, stage_rank));
                }
            }
        }

        merged.sort_by(|a, b| {
            a.2.cmp(&b.2)
                .then_with(|| b.1.total_cmp(&a.1))
                .then_with(|| a.0.cmp(&b.0))
        });
        Ok(merged
            .into_iter()
            .take(limit)
            .map(|(idx, score, stage_rank)| (idx, score, stage_rank == 0))
            .collect())
    }

    let query_terms = memory_split_query_terms(query_text);
    if query_terms.is_empty() {
        return Ok(Vec::new());
    }
    let mut scored = if query_terms.len() == 1 {
        merge_single_term_hits(
            memories,
            &searcher,
            &qp,
            memory_idx_field,
            "content",
            &query_terms[0],
            limit,
        )?
        .into_iter()
        .map(|(idx, score, _)| (idx, score))
        .collect::<Vec<_>>()
    } else {
        let per_term_limit = memories.len().max(limit);
        let mut by_memory = HashMap::<usize, (f64, usize)>::new();
        for term in &query_terms {
            let term_hits = merge_single_term_hits(
                memories,
                &searcher,
                &qp,
                memory_idx_field,
                "content",
                term,
                per_term_limit,
            )?;
            for (idx, score, strict_matched) in term_hits {
                let entry = by_memory.entry(idx).or_insert((score, 0));
                entry.0 = entry.0.max(score);
                if strict_matched {
                    entry.1 += 1;
                }
            }
        }
        let mut merged = by_memory
            .into_iter()
            .map(|(idx, (score, matched_terms))| (idx, score, matched_terms))
            .collect::<Vec<_>>();
        merged.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| b.1.total_cmp(&a.1))
                .then_with(|| a.0.cmp(&b.0))
        });
        merged
            .into_iter()
            .take(limit)
            .map(|(idx, score, _)| (idx, score))
            .collect::<Vec<_>>()
    };

    let mut out = Vec::<MemoryBm25Hit>::new();
    for (idx, raw_score) in scored.drain(..) {
        let memory_id = memories
            .get(idx)
            .map(|m| m.id.clone())
            .ok_or_else(|| format!("Invalid tantivy memory_idx: {idx}"))?;
        // 不归一化, 直接用原始 BM25 分参与融合与展示。
        out.push(MemoryBm25Hit {
            memory_id,
            raw_score,
            normalized_score: raw_score,
        });
    }
    Ok(out)
}

fn memory_has_embedding_binding(data_path: &PathBuf) -> bool {
    let Ok(conn) = memory_store_open(data_path) else {
        return false;
    };
    let active = memory_store_get_runtime_state(&conn, KB_STATE_ACTIVE_INDEX_PROVIDER_ID)
        .ok()
        .flatten()
        .unwrap_or_default();
    let embedding_api = memory_store_get_runtime_state(&conn, KB_STATE_EMBEDDING_API_CONFIG_ID)
        .ok()
        .flatten()
        .unwrap_or_default();
    !active.trim().is_empty() && !embedding_api.trim().is_empty()
}

fn memory_rerank_provider_from_binding(
    data_path: &PathBuf,
) -> Result<Option<Box<dyn MemoryRerankProvider>>, String> {
    let conn = memory_store_open(data_path)?;
    let rerank_api_id = memory_store_get_runtime_state(&conn, KB_STATE_RERANK_API_CONFIG_ID)?
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
    let mut out = HashMap::<String, f64>::new();
    for row in rows {
        if row.index >= candidate_memories.len() || !row.relevance_score.is_finite() {
            continue;
        }
        let memory_id = candidate_memories[row.index].id.clone();
        // 不归一化, 直接用原始 rerank relevance_score 参与融合与展示。
        out.insert(memory_id, row.relevance_score);
    }
    Ok(out)
}

fn memory_mixed_ranked_items(
    data_path: &PathBuf,
    memories: &[MemoryEntry],
    query_text: &str,
    limit: usize,
    rerank_min_score: f64,
) -> Vec<MemoryMixedRankItem> {
    if limit == 0 {
        return Vec::new();
    }
    if memories.is_empty() || query_text.trim().is_empty() {
        return Vec::new();
    }
    let effective_query_text = memory_search_query_text(memories, query_text);
    if effective_query_text.trim().is_empty() {
        return Vec::new();
    }

    let memory_index = memories
        .iter()
        .enumerate()
        .map(|(idx, memory)| (memory.id.clone(), idx))
        .collect::<HashMap<_, _>>();

    let bm25_hits = memory_tantivy_bm25_scores(memories, &effective_query_text, MEMORY_ROUTE_CANDIDATE_LIMIT)
        .unwrap_or_default();
    let mut bm25_map = HashMap::<String, f64>::new();
    let mut bm25_raw_map = HashMap::<String, f64>::new();
    let mut bm25_top_ids = Vec::<String>::new();
    for hit in bm25_hits {
        bm25_raw_map.insert(hit.memory_id.clone(), hit.raw_score);
        bm25_map.insert(hit.memory_id.clone(), hit.normalized_score);
        bm25_top_ids.push(hit.memory_id);
    }

    let has_embedding = memory_has_embedding_binding(data_path);
    let mut vector_map = HashMap::<String, f64>::new();
    let mut vector_available = false;
    if has_embedding {
        match memory_store_search_vector_scores(
            data_path,
            &effective_query_text,
            MEMORY_ROUTE_CANDIDATE_LIMIT,
        ) {
            Ok(rows) => {
                vector_available = true;
                for (memory_id, vector_score) in rows {
                    if vector_score.is_finite() {
                        vector_map.insert(memory_id, vector_score);
                    }
                }
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[记忆] 向量搜索失败，降级为仅 BM25 检索。 err={}",
                    err
                ));
            }
        }
    }

    // Retrieval modes:
    // 1) no embedding + no rerank: BM25 rank via RRF score.
    // 2) embedding + no rerank: vector rank + BM25 rank via RRF.
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
        candidate_ids.extend(bm25_top_ids.iter().take(limit).cloned());
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

    let bm25_rank_map = bm25_top_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.clone(), idx + 1))
        .collect::<HashMap<_, _>>();
    let mut vector_rank_pairs = vector_map
        .iter()
        .map(|(id, score)| (id.clone(), *score))
        .collect::<Vec<_>>();
    vector_rank_pairs.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let vector_rank_map = vector_rank_pairs
        .iter()
        .enumerate()
        .map(|(idx, (id, _))| (id.clone(), idx + 1))
        .collect::<HashMap<_, _>>();

    let mut rerank_map = HashMap::<String, f64>::new();
    let mut rerank_available = false;
    if has_rerank {
        let candidate_memories = candidate_ids
            .iter()
            .filter_map(|id| memory_index.get(id).and_then(|idx| memories.get(*idx)))
            .collect::<Vec<_>>();
        if let Some(provider) = rerank_provider.as_ref() {
            match memory_rerank_scores(provider.as_ref(), &effective_query_text, &candidate_memories) {
                Ok(map) => {
                    rerank_available = true;
                    rerank_map = map;
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[记忆] 重排失败，降级为不重排评分。 err={}",
                        err
                    ));
                }
            }
        }
    }

    let mut ranked = candidate_ids
        .into_iter()
        .filter_map(|memory_id| {
            let idx = *memory_index.get(&memory_id)?;
            let bm25_score = bm25_map.get(&memory_id).copied().unwrap_or(0.0);
            let rerank_score = if rerank_available {
                rerank_map.get(&memory_id).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            // rerank 模式绝对门槛仅由调用方决定; 不影响 RRF / 纯 BM25 模式。
            if rerank_available && rerank_score < rerank_min_score {
                return None;
            }
            let final_score = if rerank_available {
                rerank_score
            } else if effective_has_embedding {
                memory_rrf_score_for_id(&memory_id, &bm25_rank_map, Some(&vector_rank_map))
            } else {
                bm25_score
            };
            Some((memory_id, idx, final_score, rerank_score))
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
        .map(|(memory_id, _, final_score, rerank_score)| MemoryMixedRankItem {
            bm25_score: bm25_map.get(&memory_id).copied().unwrap_or(0.0),
            bm25_raw_score: bm25_raw_map.get(&memory_id).copied().unwrap_or(0.0),
            vector_score: vector_map.get(&memory_id).copied().unwrap_or(0.0),
            rerank_score,
            final_score,
            memory_id,
        })
        .collect::<Vec<_>>()
}

fn memory_store_active_embedding_provider_id(conn: &Connection) -> Result<Option<String>, String> {
    memory_store_get_runtime_state(conn, KB_STATE_ACTIVE_INDEX_PROVIDER_ID)
}

fn memory_store_embedding_binding_api_id(conn: &Connection) -> Result<Option<String>, String> {
    memory_store_get_runtime_state(conn, KB_STATE_EMBEDDING_API_CONFIG_ID)
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
        // 不归一化, 直接用原始余弦相似度参与融合与展示。
        let score = cos as f64;
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
    include_reasoning: bool,
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

    // 外层是 XML 看板，内部是纯粹的记忆，一行一行：[id:xxx] 正文（id 后不换行）。
    Some(format!(
        "<memory_context>\n{}\n</memory_context>",
        ordered_memories
            .into_iter()
            .map(|memory| {
                if include_reasoning {
                    let reasoning = memory.reasoning.trim();
                    let display_reasoning = if reasoning.is_empty() { "无" } else { reasoning };
                    format!(
                        "[id:{}] {}\n> {}",
                        memory.display_id(),
                        memory.judgment.trim(),
                        display_reasoning
                    )
                } else {
                    format!(
                        "[id:{}] {}",
                        memory.display_id(),
                        memory.judgment.trim()
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

#[cfg(test)]
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
    build_memory_board_xml_from_recall_ids(memories, &recall_ids, true)
}
