use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use jieba_rs::Jieba;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const MEMORY_MATCH_MAX_ITEMS: usize = 7;
const MEMORY_CANDIDATE_MULTIPLIER: usize = 7;
const MEMORY_ROUTE_CANDIDATE_LIMIT: usize = MEMORY_MATCH_MAX_ITEMS * MEMORY_CANDIDATE_MULTIPLIER;
const MEMORY_WEIGHT_FTS: f64 = 0.7;
const MEMORY_WEIGHT_VECTOR: f64 = 0.3;
// TODO: replace with real vector retrieval score after embedding index/search is wired.
const MEMORY_VECTOR_SCORE_DEFAULT: f64 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMixedRankItem {
    memory_id: String,
    bm25_score: f64,
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
        .map(|item| item.memory_id)
        .collect::<Vec<_>>()
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

    let fts_hits = memory_store_search_fts_bm25(data_path, query_text, MEMORY_ROUTE_CANDIDATE_LIMIT)
        .unwrap_or_default();
    let mut fts_map = HashMap::<String, f64>::new();
    for (memory_id, bm25_score) in fts_hits {
        if !bm25_score.is_finite() {
            continue;
        }
        // FTS5 bm25 is lower-is-better (negative). Use log-based normalization:
        // ln(1+|x|) / (1+ln(1+|x|)) gives continuous (0, 1) with good spread,
        // unlike sigmoid which saturates to ~1.0 for typical bm25 magnitudes.
        let abs_score = bm25_score.abs();
        let log_val = (1.0 + abs_score).ln();
        let relevance = log_val / (1.0 + log_val);
        fts_map.insert(memory_id, relevance.clamp(0.0, 1.0));
    }

    let mut candidates = HashSet::<String>::new();
    candidates.extend(fts_map.keys().cloned());
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut ranked = candidates
        .into_iter()
        .filter_map(|memory_id| {
            let idx = *memory_index.get(&memory_id)?;
            let fts_score = fts_map.get(&memory_id).copied().unwrap_or(0.0);
            let vector_score = MEMORY_VECTOR_SCORE_DEFAULT;
            let final_score = MEMORY_WEIGHT_FTS * fts_score + MEMORY_WEIGHT_VECTOR * vector_score;
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
            bm25_score: fts_map.get(&memory_id).copied().unwrap_or(0.0),
            vector_score: MEMORY_VECTOR_SCORE_DEFAULT,
            final_score,
            memory_id,
        })
        .collect::<Vec<_>>()
}

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
