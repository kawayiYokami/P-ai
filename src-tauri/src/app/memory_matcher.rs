use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const MEMORY_MATCH_MAX_ITEMS: usize = 7;

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

fn memory_match_signature(memories: &[MemoryEntry]) -> String {
    let mut hasher = Sha256::new();
    for memory in memories {
        hasher.update(memory.id.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(memory.updated_at.as_bytes());
        hasher.update(b"\x1f");
        hasher.update(memory.content.as_bytes());
        hasher.update(b"\x1e");
        for kw in &memory.keywords {
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
        for kw in &memory.keywords {
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

fn build_memory_board_xml(
    memories: &[MemoryEntry],
    search_text: &str,
    latest_user_text: &str,
) -> Option<String> {
    if memories.is_empty() {
        return None;
    }

    let mut corpus = String::new();
    corpus.push_str(search_text);
    if !latest_user_text.trim().is_empty() {
        corpus.push('\n');
        corpus.push_str(&latest_user_text.to_lowercase());
    }
    if corpus.trim().is_empty() {
        return None;
    }

    let compiled = get_or_compile_memory_matcher(memories);
    let matcher = compiled.matcher.as_ref()?;

    let mut hit_counts = vec![0usize; memories.len()];
    let mut seen = HashSet::<(usize, usize)>::new();

    for mat in matcher.find_iter(&corpus) {
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
        .filter_map(|(idx, score)| {
            if score >= 1 {
                Some((idx, &memories[idx], score))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if hits.is_empty() {
        return None;
    }

    hits.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    if hits.len() > MEMORY_MATCH_MAX_ITEMS {
        hits.truncate(MEMORY_MATCH_MAX_ITEMS);
    }

    let mut out = String::new();
    out.push_str("<memory_board>\n");
    out.push_str("  <note>这是记忆提示板，请按需参考，不要编造未命中的记忆。</note>\n");
    out.push_str("  <memories>\n");
    for (_idx, memory, _score) in hits {
        out.push_str("    <memory>\n");
        out.push_str(&format!(
            "      <content>{}</content>\n",
            xml_escape(&memory.content)
        ));
        out.push_str("    </memory>\n");
    }
    out.push_str("  </memories>\n");
    out.push_str("</memory_board>");
    Some(out)
}
