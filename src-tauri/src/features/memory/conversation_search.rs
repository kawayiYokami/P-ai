// ==================== 会话搜索：逐会话并发扫描，凑够即停 ====================
//
// 目标是最快找到相关会话，不做全局相关性排序，也不建索引：
// 候选会话按最近活跃倒序，多个线程抢会话并行解压、解析、渲染、匹配正文，
// 命中片段累计到阈值就不再派新会话，已经开始解压的会话照常读完。
// 不落盘、不缓存、不跨次复用。

const CONVERSATION_SEARCH_DEFAULT_LIMIT: usize = 10;
const CONVERSATION_SEARCH_MAX_LIMIT: usize = 100;
/// 提前停止阈值：命中片段达到这个数就不再派新会话，正在解的照常读完。
const CONVERSATION_SEARCH_EARLY_STOP_LIMIT: usize = 10;
/// 命中片段上限：搜索页只展示定位片段，不搬全文。
const CONVERSATION_SEARCH_SNIPPET_MAX_CHARS: usize = 300;
/// 片段里命中词前面保留的字数。
const CONVERSATION_SEARCH_SNIPPET_LEAD_CHARS: usize = 40;

const CONVERSATION_SOURCE_KIND_LOCAL: &str = "localConversation";
const CONVERSATION_SOURCE_KIND_CONTACT: &str = "contact";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationSearchInput {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
    /// 人格筛选：给值只搜该人格归属的会话，不给表示不限。
    #[serde(default)]
    agent_id: Option<String>,
    /// 来源类型筛选：localConversation / contact，空表示不限。
    #[serde(default)]
    source_kinds: Option<Vec<String>>,
    /// 主工作区路径筛选（只认 shell_workspace_path），空表示不限。
    #[serde(default)]
    workspace_path: Option<String>,
    /// 时间范围筛选：只在最近 N 天内有更新的会话里检索，0 或空表示不限。
    #[serde(default)]
    days: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationSearchHit {
    conversation_id: String,
    conversation_title: String,
    /// 标题为空时的兜底标题：会话的最近摘要标题。
    conversation_summary_title: String,
    created_at: String,
    snippet: String,
    /// 会话最近活动时间：标题为空时用来兜底显示，与侧栏标题口径一致。
    conversation_updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationSearchResult {
    hits: Vec<ConversationSearchHit>,
}

/// 一条命中片段：内部排序用会话序号与消息倒序号，对外再摊平成接口结构。
#[derive(Debug, Clone)]
struct ConversationSearchMatchedHit {
    conv_rank: usize,
    msg_rank: usize,
    hit: ConversationSearchHit,
}

struct ConversationSearchCandidate {
    id: String,
    title: String,
    /// 会话标题为空时用它兜底：这才是点进会话后看到的那行标题。
    summary_title: String,
    updated_at: String,
}

/// 关键词提取：按空白切分并统一小写，不做分词器——搜索要的是够快，
/// 而不是词形归一。中文查询整段就是一个关键词。
fn conversation_search_extract_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::<String>::new();
    for part in query.split_whitespace() {
        let lowered = part.to_lowercase();
        if lowered.is_empty() || terms.contains(&lowered) {
            continue;
        }
        terms.push(lowered);
    }
    terms
}

/// 全部关键词都出现才算命中：多词查询默认按「同时包含」理解，更贴近找人找事。
fn conversation_search_matches(lowered_text: &str, terms: &[String]) -> bool {
    terms.iter().all(|term| lowered_text.contains(term.as_str()))
}

/// 正文匹配：先按原样比，失败再考虑大小写。
/// 关键词没有 ASCII 字母时（中文查询）不必把整条正文降级成小写，省一次大字符串分配。
fn conversation_search_message_matches(text: &str, terms: &[String]) -> bool {
    if conversation_search_matches(text, terms) {
        return true;
    }
    if !terms
        .iter()
        .any(|term| term.bytes().any(|byte| byte.is_ascii_alphabetic()))
    {
        return false;
    }
    let lowered = text.to_lowercase();
    conversation_search_matches(&lowered, terms)
}

/// 并行扫描的阶段累计：各线程本地累加，结束后合并。
#[derive(Debug, Default, Clone, Copy)]
struct ConversationScanTotals {
    read_conversations: usize,
    read_messages: usize,
    skipped_blocks: usize,
    parsed_messages: usize,
    skipped: usize,
    read_ms: u128,
    decompress_ms: u128,
    parse_ms: u128,
    render_ms: u128,
    match_ms: u128,
}

impl ConversationScanTotals {
    fn merge(&mut self, other: Self) {
        self.read_conversations += other.read_conversations;
        self.read_messages += other.read_messages;
        self.skipped_blocks += other.skipped_blocks;
        self.parsed_messages += other.parsed_messages;
        self.skipped += other.skipped;
        self.read_ms += other.read_ms;
        self.decompress_ms += other.decompress_ms;
        self.parse_ms += other.parse_ms;
        self.render_ms += other.render_ms;
        self.match_ms += other.match_ms;
    }
}

/// 工作区路径比较键：去扩展前缀、统一斜杠、去尾斜杠，Windows 下统一小写。
fn conversation_search_workspace_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let without_prefix = trimmed.strip_prefix("\\\\?\\").unwrap_or(trimmed);
    let unified = without_prefix.replace('\\', "/");
    let trimmed_slash = unified.trim_end_matches('/');
    let key = if trimmed_slash.is_empty() {
        unified.clone()
    } else {
        trimmed_slash.to_string()
    };
    #[cfg(target_os = "windows")]
    {
        key.to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        key
    }
}

/// 命中片段：以第一个命中词的字符位置为中心截取，找不到命中词就取开头。
fn conversation_search_snippet(text: &str, terms: &[String]) -> String {
    let trimmed = text.trim();
    let total = trimmed.chars().count();
    if total <= CONVERSATION_SEARCH_SNIPPET_MAX_CHARS {
        return trimmed.to_string();
    }
    let lowered = trimmed.to_lowercase();
    let mut hit_char_index: Option<usize> = None;
    for term in terms {
        let term = term.trim();
        if term.is_empty() {
            continue;
        }
        if let Some(byte_pos) = lowered.find(term) {
            let char_pos = lowered[..byte_pos].chars().count();
            hit_char_index = Some(match hit_char_index {
                Some(prev) => prev.min(char_pos),
                None => char_pos,
            });
        }
    }
    let anchor = hit_char_index.unwrap_or(0);
    let start = anchor
        .saturating_sub(CONVERSATION_SEARCH_SNIPPET_LEAD_CHARS)
        .min(total.saturating_sub(CONVERSATION_SEARCH_SNIPPET_MAX_CHARS));
    let end = (start + CONVERSATION_SEARCH_SNIPPET_MAX_CHARS).min(total);
    let mut snippet = trimmed
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<String>();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < total {
        snippet.push('…');
    }
    snippet
}

/// 按筛选条件挑出候选会话：排除归档与委托会话，只认主工作区。
fn conversation_search_collect_candidates(
    state: &AppState,
    agent_filter: Option<&str>,
    source_kinds: &[String],
    workspace_key: Option<&str>,
    cutoff: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<ConversationSearchCandidate>, String> {
    // 一次取出全部会话元数据，在内存里筛选：逐会话查库会让上千次连接与建表语句白耗掉。
    let all_conversations =
        message_store::chat_metadata_store_list_search_conversations(&state.data_path)?;
    let mut candidates = Vec::<ConversationSearchCandidate>::new();
    for conversation_meta in &all_conversations {
        if conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE {
            // 委托会话不是可搜索的会话。
            continue;
        }
        let archived = conversation_meta.status.trim() == "archived"
            || conversation_meta
                .archived_at
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
        if archived {
            // 归档会话不在搜索范围内。
            continue;
        }
        let source_kind = if conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT {
            "contact"
        } else {
            "localConversation"
        };
        let owner_agent_id = conversation_meta.agent_id.trim().to_string();
        if owner_agent_id.is_empty() {
            // 孤儿会话没有归属人格，不进范围。
            continue;
        }
        if let Some(agent) = agent_filter {
            if owner_agent_id != agent {
                continue;
            }
        }
        if !source_kinds.is_empty() && !source_kinds.iter().any(|kind| kind == source_kind) {
            continue;
        }
        if let Some(cutoff) = cutoff {
            // 时间范围按最近活动时间算：更早没有新消息的会话不可能命中最新检索意图。
            // 时间串解析不出来时保守保留，宁可多扫也不能悄悄漏掉会话。
            let last_active = conversation_meta.updated_at.trim();
            let in_range = chrono::DateTime::parse_from_rfc3339(last_active)
                .map(|value| value.with_timezone(&chrono::Utc) >= cutoff)
                .unwrap_or(true);
            if !in_range {
                continue;
            }
        }
        let workspace_path = conversation_meta
            .shell_workspace_path
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if let Some(expected) = workspace_key {
            if conversation_search_workspace_key(&workspace_path) != expected {
                continue;
            }
        }
        candidates.push(ConversationSearchCandidate {
            id: conversation_meta.conversation_id.clone(),
            title: conversation_meta.title.clone(),
            summary_title: conversation_meta
                .summary_title
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
            updated_at: conversation_meta.updated_at.clone(),
        });
    }
    // 最近活动的会话先扫：凑够条数就停，越靠前的会话越可能命中。
    candidates.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(candidates)
}

fn conversation_search_messages(
    state: &AppState,
    input: &ConversationSearchInput,
) -> Result<ConversationSearchResult, String> {
    let started = std::time::Instant::now();
    let query = input.query.trim().to_string();
    // 提前停止阈值：凑够这么多命中片段就不再派新会话，正在读的会话照常读完，
    // 所以最终结果通常比阈值略多。
    let stop_threshold = input
        .limit
        .unwrap_or(CONVERSATION_SEARCH_DEFAULT_LIMIT)
        .clamp(CONVERSATION_SEARCH_EARLY_STOP_LIMIT, CONVERSATION_SEARCH_MAX_LIMIT);
    let agent_filter = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let source_kinds = input
        .source_kinds
        .as_ref()
        .map(|kinds| {
            kinds
                .iter()
                .map(|kind| kind.trim())
                .filter(|kind| {
                    !kind.is_empty()
                        && (*kind == CONVERSATION_SOURCE_KIND_LOCAL
                            || *kind == CONVERSATION_SOURCE_KIND_CONTACT)
                })
                .map(str::to_string)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    let workspace_key = input
        .workspace_path
        .as_deref()
        .map(conversation_search_workspace_key)
        .filter(|key| !key.is_empty());
    let days = input.days.unwrap_or(0);
    let cutoff = if days > 0 {
        Some(chrono::Utc::now() - chrono::Duration::days(days as i64))
    } else {
        None
    };

    runtime_log_info(format!(
        "[会话搜索] 开始 query={} 阈值={} 来源={} 人格={} 目录={} 天数={}",
        query,
        stop_threshold,
        if source_kinds.is_empty() {
            "不限".to_string()
        } else {
            source_kinds.join(",")
        },
        agent_filter.unwrap_or("不限"),
        input.workspace_path.as_deref().unwrap_or("不限"),
        days
    ));

    let candidate_started = std::time::Instant::now();
    let candidates = conversation_search_collect_candidates(
        state,
        agent_filter,
        &source_kinds,
        workspace_key.as_deref(),
        cutoff,
    )?;
    runtime_log_info(format!(
        "[会话搜索] 候选完成 会话={} 耗时={}ms",
        candidates.len(),
        candidate_started.elapsed().as_millis()
    ));

    if query.is_empty() {
        runtime_log_info("[会话搜索] 跳过 空关键词".to_string());
        return Ok(ConversationSearchResult { hits: Vec::new() });
    }

    let terms = conversation_search_extract_terms(&query);
    if terms.is_empty() {
        runtime_log_info("[会话搜索] 跳过 关键词为空".to_string());
        return Ok(ConversationSearchResult { hits: Vec::new() });
    }

    let agents = state_read_agents_cached(state)?;
    let user_alias = chat_history_user_persona_name(&agents);

    // 定位记录一次性取回，扫描阶段只读块文件，不再碰 SQLite。
    let locate_started = std::time::Instant::now();
    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.id.clone())
        .collect::<std::collections::HashSet<String>>();
    let locators_by_conversation = message_store::chat_metadata_store_list_locators_by_conversation(
        &state.data_path,
        &candidate_ids,
    )?;
    let locate_ms = locate_started.elapsed().as_millis();

    let scan_started = std::time::Instant::now();
    let render = |message: &ChatMessage| {
        chat_history_render_message(message, &agents, &user_alias).map(|item| {
            message_store::MessageStoreRenderedMessage {
                rendered: item.rendered,
                created_at: item.created_at,
            }
        })
    };
    let data_path = &state.data_path;
    let terms_ref = &terms;
    let worker_count = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4)
        .clamp(1, 8)
        .min(candidates.len().max(1));
    // 抢任务而不是静态分片：命中够了就不再派新会话，已在解压的会话照常读完。
    let next_task = std::sync::atomic::AtomicUsize::new(0);
    let hits_total = std::sync::atomic::AtomicUsize::new(0);
    let collected_hits = std::sync::Mutex::new(Vec::<ConversationSearchMatchedHit>::new());
    let collected_ref = &collected_hits;
    let next_task_ref = &next_task;
    let hits_total_ref = &hits_total;
    let mut stage_totals = ConversationScanTotals::default();

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| {
                let mut totals = ConversationScanTotals::default();
                let mut local_hits = Vec::<ConversationSearchMatchedHit>::new();
                loop {
                    if hits_total_ref.load(std::sync::atomic::Ordering::Relaxed) >= stop_threshold {
                        break;
                    }
                    let task_index =
                        next_task_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if task_index >= candidates.len() {
                        break;
                    }
                    let candidate = &candidates[task_index];
                    let Some(items) = locators_by_conversation.get(&candidate.id) else {
                        // 没有定位记录说明这个会话本来就没有消息，不算失败。
                        continue;
                    };
                    let paths = match message_store::message_store_paths(data_path, &candidate.id) {
                        Ok(paths) => paths,
                        Err(err) => {
                            runtime_log_debug(format!(
                                "[会话搜索] 跳过无法定位存储的会话，conversation_id={}，error={err}",
                                candidate.id
                            ));
                            totals.skipped += 1;
                            continue;
                        }
                    };
                    let read = match message_store::chat_metadata_store_scan_items_for_terms(
                        &paths,
                        items,
                        terms_ref,
                        &render,
                    ) {
                        Ok(read) => read,
                        Err(err) => {
                            runtime_log_debug(format!(
                                "[会话搜索] 跳过读取失败的会话，conversation_id={}，error={err}",
                                candidate.id
                            ));
                            totals.skipped += 1;
                            continue;
                        }
                    };
                    totals.skipped_blocks += read.skipped_blocks;
                    totals.parsed_messages += read.parsed_messages;
                    totals.read_ms += read.stages.read_ms;
                    totals.decompress_ms += read.stages.decompress_ms;
                    totals.parse_ms += read.stages.parse_ms;
                    totals.render_ms += read.stages.render_ms;
                    if read.messages.is_empty() {
                        continue;
                    }
                    totals.read_conversations += 1;
                    totals.read_messages += read.messages.len();

                    // 会话内从新到旧扫：命中片段按时间倒序更符合「最近说过」的直觉。
                    for (msg_rank, message) in read.messages.iter().rev().enumerate() {
                        let match_started = std::time::Instant::now();
                        let matched =
                            conversation_search_message_matches(&message.rendered, terms_ref);
                        totals.match_ms += match_started.elapsed().as_millis();
                        if !matched {
                            continue;
                        }
                        local_hits.push(ConversationSearchMatchedHit {
                            conv_rank: task_index,
                            msg_rank,
                            hit: ConversationSearchHit {
                                conversation_id: candidate.id.clone(),
                                conversation_title: candidate.title.clone(),
                                conversation_summary_title: candidate.summary_title.clone(),
                                created_at: message.created_at.clone(),
                                snippet: conversation_search_snippet(&message.rendered, terms_ref),
                                conversation_updated_at: candidate.updated_at.clone(),
                            },
                        });
                        if local_hits.len() >= stop_threshold {
                            break;
                        }
                    }
                    if !local_hits.is_empty() {
                        let count = local_hits.len();
                        {
                            let mut guard = collected_ref
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner());
                            guard.append(&mut local_hits);
                        }
                        hits_total_ref.fetch_add(count, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                totals
            }));
        }
        for handle in handles {
            match handle.join() {
                Ok(totals) => stage_totals.merge(totals),
                Err(_) => runtime_log_error("[会话搜索] 扫描线程异常退出".to_string()),
            }
        }
    });
    let scan_elapsed = scan_started.elapsed();

    let mut matched = std::mem::take(
        &mut *collected_hits
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()),
    );
    matched.sort_by(|left, right| {
        left.conv_rank
            .cmp(&right.conv_rank)
            .then(left.msg_rank.cmp(&right.msg_rank))
    });
    let hits = matched.into_iter().map(|item| item.hit).collect::<Vec<_>>();

    let early_stopped =
        hits.len() >= stop_threshold && stage_totals.read_conversations < candidates.len();

    runtime_log_info(format!(
        "[会话搜索] 完成 命中={} 跳过={} 候选会话={} 读取会话={} 读取消息={} 跳过块={} 解析消息={} 线程={} 提前停止={} 总耗时={}ms",
        hits.len(),
        stage_totals.skipped,
        candidates.len(),
        stage_totals.read_conversations,
        stage_totals.read_messages,
        stage_totals.skipped_blocks,
        stage_totals.parsed_messages,
        worker_count,
        if early_stopped { "是" } else { "否" },
        started.elapsed().as_millis()
    ));
    runtime_log_info(format!(
        "[会话搜索] 阶段耗时 定位={}ms 并行扫描={}ms（累计：读文件={} 解压={} 解析={} 渲染={} 匹配={}）",
        locate_ms,
        scan_elapsed.as_millis(),
        stage_totals.read_ms,
        stage_totals.decompress_ms,
        stage_totals.parse_ms,
        stage_totals.render_ms,
        stage_totals.match_ms
    ));

    Ok(ConversationSearchResult { hits })
}

#[tauri::command]
async fn search_conversation_messages(
    input: ConversationSearchInput,
    state: State<'_, AppState>,
) -> Result<ConversationSearchResult, String> {
    // 全库扫描是秒级重活，必须移出主线程，否则窗口消息循环被占住会显示未响应。
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || conversation_search_messages(&app_state, &input))
        .await
        .map_err(|err| format!("会话搜索任务异常：{err}"))?
}

#[cfg(test)]
mod conversation_search_tests {
    use super::*;

    #[test]
    fn conversation_search_workspace_key_should_normalize_slashes_and_prefix() {
        let plain = conversation_search_workspace_key("E:/github/demo");
        assert_eq!(conversation_search_workspace_key("E:\\github\\demo"), plain);
        assert_eq!(
            conversation_search_workspace_key("\\\\?\\E:\\github\\demo\\"),
            plain
        );
        assert_eq!(conversation_search_workspace_key("  "), "");
    }

    #[test]
    fn conversation_search_snippet_should_center_on_hit_term() {
        let long_text = format!(
            "{}{}{}",
            "开头一段无关的话。".repeat(30),
            "命中关键词在这里出现",
            "后面还有很多内容。".repeat(30)
        );
        let snippet = conversation_search_snippet(&long_text, &["命中关键词".to_string()]);
        assert!(snippet.contains("命中关键词"));
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert!(snippet.chars().count() <= CONVERSATION_SEARCH_SNIPPET_MAX_CHARS + 2);
    }

    #[test]
    fn conversation_search_snippet_should_keep_short_text_as_is() {
        let text = "很短的一句话";
        assert_eq!(
            conversation_search_snippet(text, &["短".to_string()]),
            text.to_string()
        );
    }

    #[test]
    fn conversation_search_snippet_should_fall_back_to_head_without_term() {
        let long_text = "没有任何关键词的长文本。".repeat(60);
        let snippet = conversation_search_snippet(&long_text, &["不存在的词".to_string()]);
        assert!(snippet.starts_with("没有任何关键词"));
        assert!(snippet.ends_with('…'));
    }

}
