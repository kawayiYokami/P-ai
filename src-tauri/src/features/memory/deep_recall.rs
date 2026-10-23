// ==================== deeprecall 深度回忆内存索引 ====================
//
// 与 chat_history_search.rs 的磁盘持久化索引不同：
// deeprecall 的索引在委托会话内按需构建（纯内存 Tantivy），委托结束后释放，
// 不做磁盘持久化、不做签名增量维护、不引入常驻后台开销。

const DEEP_RECALL_FIELD_CONV_IDX: &str = "conv_idx";
const DEEP_RECALL_FIELD_MSG_IDX: &str = "msg_idx";
const DEEP_RECALL_FIELD_CONTENT: &str = "content";
const DEEP_RECALL_FIELD_OWNER_AGENT_ID: &str = "owner_agent_id";

const DEEP_RECALL_SEARCH_DEFAULT_LIMIT: usize = 10;
const DEEP_RECALL_SEARCH_MAX_LIMIT: usize = 50;
const DEEP_RECALL_CONTEXT_MAX_MESSAGES: usize = 100;
/// 命中片段上限：检索只做定位，正文搬运交给 deeprecall_context 展开。
const DEEP_RECALL_SNIPPET_MAX_CHARS: usize = 200;

/// 会话内一条入索引消息。序号从 1 起，按会话内时间顺序编号。
#[derive(Debug, Clone)]
struct DeepRecallMessage {
    index: usize,
    speaker: String,
    time: String,
    text: String,
}

/// 一次深度回忆索引里的一个会话。会话序号从 1 起，按创建时间升序编号。
#[derive(Debug, Clone)]
struct DeepRecallConversation {
    index: usize,
    title: String,
    source_kind: String,
    messages: Vec<DeepRecallMessage>,
    /// 会话归属人格：人格只能回忆起归属自己的会话，会话内所有人的发言都算它的记忆。
    owner_agent_id: String,
}

#[derive(Clone)]
struct DeepRecallIndexFields {
    conv_idx: tantivy::schema::Field,
    msg_idx: tantivy::schema::Field,
    content: tantivy::schema::Field,
    owner_agent_id: tantivy::schema::Field,
}

struct CachedDeepRecallIndex {
    conversations: Vec<DeepRecallConversation>,
    index: Index,
    reader: tantivy::IndexReader,
    fields: DeepRecallIndexFields,
}

fn deep_recall_index_cache() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, std::sync::Arc<CachedDeepRecallIndex>>,
> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<CachedDeepRecallIndex>>,
        >,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 索引归属键：用委托会话 id 标识一份内存索引，多个委托各占一槽、互不驱逐。
fn deep_recall_owner_key(session_id: &str) -> String {
    delegate_session_conversation_id(session_id)
        .unwrap_or_else(|| session_id.trim().to_string())
}

/// 收集阶段的一个候选会话：元数据一次取齐，后面不再重复查库。
struct DeepRecallCandidate {
    created_at: String,
    id: String,
    title: String,
    source_kind: String,
    owner_agent_id: String,
}

/// 并行读一个会话的产物：渲染后的消息与逐条预分词结果。
struct DeepRecallCollected {
    messages: Vec<DeepRecallMessage>,
    tokenized: Vec<String>,
}

/// 一次收集的全部索引素材：会话按创建时间升序排好，分词结果与会话一一对应。
struct DeepRecallCollection {
    conversations: Vec<DeepRecallConversation>,
    tokenized: Vec<Vec<String>>,
}

/// 渲染后的单条消息：正文、说话人、时间，外带已经分好的词。
struct DeepRecallRendered {
    speaker: String,
    time: String,
    text: String,
}

/// 收集某个人格可回忆的会话：本地会话 + 归档 + 联系人会话，跳过委托会话。
/// 只收归属该人格的会话——索引本身就是按人格定制的，非本人会话连读都不读。
/// 会话序号按创建时间升序全局编号，会话内消息按时间顺序编号。
/// 会话之间并行读块、解压、解析、渲染、分词，读完再按候选顺序拼回有序素材，
/// 所以索引顺序与会话创建时间、消息时间顺序完全一致。
fn deep_recall_collect_conversations(
    state: &AppState,
    agent_id: &str,
) -> Result<DeepRecallCollection, String> {
    let started = std::time::Instant::now();
    let agents = state_read_agents_cached(state)?;
    let chat_index = state_read_chat_index_cached(state)?;
    let user_alias = chat_history_user_persona_name(&agents);

    let mut candidates = Vec::<DeepRecallCandidate>::new();
    let mut skipped_orphan_conversations = 0usize;
    let mut skipped_other_agents = 0usize;
    for item in &chat_index.conversations {
        let Ok(conversation_meta) =
            conversation_service_v2().get_conversation_meta(state, &item.id)
        else {
            continue;
        };
        let Some(source_kind) = chat_history_source_kind_from_meta(&conversation_meta) else {
            // 委托会话不是主人的记忆，跳过。
            continue;
        };
        let owner_agent_id = conversation_meta.agent_id.trim().to_string();
        if owner_agent_id.is_empty() {
            // 孤儿会话（归属为空）：不属于任何人格的记忆，需先指定接管人。
            skipped_orphan_conversations += 1;
            continue;
        }
        if owner_agent_id != agent_id {
            // 非本人归属的会话不进本次索引：内存索引只装这个人格能回忆起的会话。
            skipped_other_agents += 1;
            continue;
        }
        candidates.push(DeepRecallCandidate {
            created_at: conversation_meta.created_at.clone(),
            id: conversation_meta.id.clone(),
            title: conversation_meta.title.clone(),
            source_kind: source_kind.to_string(),
            owner_agent_id,
        });
    }
    candidates.sort_by(|left, right| left.created_at.cmp(&right.created_at));

    // 整块读取：不查定位记录、不加会话锁，按块文件顺序读回全部消息。
    let render = |message: &ChatMessage| -> Option<DeepRecallRendered> {
        chat_history_render_message(message, &agents, &user_alias).map(|item| DeepRecallRendered {
            speaker: item.speaker_name,
            time: item.created_at,
            text: item.rendered,
        })
    };

    let data_path = &state.data_path;
    let worker_count = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4)
        .clamp(1, 8)
        .min(candidates.len().max(1));
    let next_task = std::sync::atomic::AtomicUsize::new(0);
    let read_failed = std::sync::atomic::AtomicUsize::new(0);
    let collected = std::sync::Mutex::new(Vec::<(usize, DeepRecallCollected)>::new());
    let collected_ref = &collected;
    let next_task_ref = &next_task;
    let read_failed_ref = &read_failed;

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let task_index = next_task_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if task_index >= candidates.len() {
                    break;
                }
                let candidate = &candidates[task_index];
                let paths = match message_store::message_store_paths(data_path, &candidate.id) {
                    Ok(paths) => paths,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[深度回忆] 跳过无法定位存储的会话，conversation_id={}，error={err}",
                            candidate.id
                        ));
                        read_failed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                };
                let outcome = match message_store::chat_metadata_store_scan_all_blocks(&paths, &render)
                {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[深度回忆] 跳过读取失败的会话，conversation_id={}，error={err}",
                            candidate.id
                        ));
                        read_failed_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                };
                let mut messages = Vec::with_capacity(outcome.messages.len());
                let mut tokenized = Vec::with_capacity(outcome.messages.len());
                for rendered in outcome.messages {
                    // 分词放在读锁之外：它只碰内存里的正文，跟块文件已经无关。
                    tokenized.push(memory_tokenize_terms(&rendered.text, false).join(" "));
                    messages.push(DeepRecallMessage {
                        index: 0,
                        speaker: rendered.speaker,
                        time: rendered.time,
                        text: rendered.text,
                    });
                }
                collected_ref
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push((task_index, DeepRecallCollected { messages, tokenized }));
            });
        }
    });

    let mut by_position = std::collections::HashMap::<usize, DeepRecallCollected>::new();
    for (position, item) in collected
        .into_inner()
        .unwrap_or_else(|poison| poison.into_inner())
    {
        by_position.insert(position, item);
    }

    let mut conversations = Vec::<DeepRecallConversation>::new();
    let mut tokenized_by_conversation = Vec::<Vec<String>>::new();
    let mut skipped_no_messages = 0usize;
    for (position, candidate) in candidates.iter().enumerate() {
        let Some(mut item) = by_position.remove(&position) else {
            continue;
        };
        if item.messages.is_empty() {
            skipped_no_messages += 1;
            continue;
        }
        // 会话内序号在这里统一编：并行读回来的顺序不可靠，索引必须按时间顺序。
        for (offset, message) in item.messages.iter_mut().enumerate() {
            message.index = offset + 1;
        }
        conversations.push(DeepRecallConversation {
            index: conversations.len() + 1,
            title: candidate.title.clone(),
            source_kind: candidate.source_kind.clone(),
            messages: item.messages,
            owner_agent_id: candidate.owner_agent_id.clone(),
        });
        tokenized_by_conversation.push(item.tokenized);
    }

    let total_messages = conversations
        .iter()
        .map(|conversation| conversation.messages.len())
        .sum::<usize>();
    runtime_log_info(format!(
        "[深度回忆] 记忆收集完成 人格={} 会话数={} 消息数={} 跳过空会话={} 跳过孤儿会话={} 跳过非本人会话={} 读取失败={} 线程={} 耗时={}ms",
        agent_id,
        conversations.len(),
        total_messages,
        skipped_no_messages,
        skipped_orphan_conversations,
        skipped_other_agents,
        read_failed.load(std::sync::atomic::Ordering::Relaxed),
        worker_count,
        started.elapsed().as_millis()
    ));
    Ok(DeepRecallCollection {
        conversations,
        tokenized: tokenized_by_conversation,
    })
}

fn deep_recall_build_schema() -> (Schema, DeepRecallIndexFields) {
    let mut schema_builder = Schema::builder();
    let conv_idx_field = schema_builder.add_u64_field(DEEP_RECALL_FIELD_CONV_IDX, FAST | STORED);
    let msg_idx_field = schema_builder.add_u64_field(DEEP_RECALL_FIELD_MSG_IDX, FAST | STORED);
    let indexing = TextFieldIndexing::default()
        .set_tokenizer("chat_ws")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let content_field =
        schema_builder.add_text_field(DEEP_RECALL_FIELD_CONTENT, TextOptions::default().set_indexing_options(indexing));
    let owner_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("raw")
            .set_index_option(IndexRecordOption::Basic),
    );
    let owner_field =
        schema_builder.add_text_field(DEEP_RECALL_FIELD_OWNER_AGENT_ID, owner_options);
    let schema = schema_builder.build();
    (
        schema,
        DeepRecallIndexFields {
            conv_idx: conv_idx_field,
            msg_idx: msg_idx_field,
            content: content_field,
            owner_agent_id: owner_field,
        },
    )
}

fn deep_recall_build_index(
    owner_key: &str,
    collection: DeepRecallCollection,
) -> Result<CachedDeepRecallIndex, String> {
    let started = std::time::Instant::now();
    let DeepRecallCollection {
        conversations,
        tokenized,
    } = collection;
    let total_messages = conversations
        .iter()
        .map(|conversation| conversation.messages.len())
        .sum::<usize>();
    let (schema, fields) = deep_recall_build_schema();
    let index = Index::create_in_ram(schema);
    chat_history_register_tokenizers(&index);
    {
        let mut writer = index
            .writer(20_000_000)
            .map_err(|err| format!("创建深度回忆内存索引写入器失败: {err}"))?;
        // 分词已经在并行收集阶段做完，这里只负责入队与提交，不再单线程重复分词。
        for (conversation, tokenized_messages) in conversations.iter().zip(tokenized.iter()) {
            for (message, tokenized_text) in
                conversation.messages.iter().zip(tokenized_messages.iter())
            {
                if tokenized_text.trim().is_empty() {
                    continue;
                }
                let mut document = doc!(
                    fields.conv_idx => conversation.index as u64,
                    fields.msg_idx => message.index as u64,
                    fields.content => tokenized_text.as_str()
                );
                if !conversation.owner_agent_id.trim().is_empty() {
                    document.add_text(fields.owner_agent_id, &conversation.owner_agent_id);
                }
                writer
                    .add_document(document)
                    .map_err(|err| format!("写入深度回忆索引文档失败: {err}"))?;
            }
        }
        writer
            .commit()
            .map_err(|err| format!("提交深度回忆内存索引失败: {err}"))?;
    }
    let reader = index
        .reader()
        .map_err(|err| format!("打开深度回忆索引读取器失败: {err}"))?;
    runtime_log_info(format!(
        "[深度回忆] 内存索引构建完成 会话数={} 消息数={} 耗时={}ms owner_key={}",
        conversations.len(),
        total_messages,
        started.elapsed().as_millis(),
        owner_key
    ));
    Ok(CachedDeepRecallIndex {
        conversations,
        index,
        reader,
        fields,
    })
}

/// 取当前委托会话的内存索引；同槽命中直接复用，否则按人格收集并构建后写入本槽。
fn deep_recall_index_for_key(
    state: &AppState,
    owner_key: &str,
    agent_id: &str,
) -> Result<std::sync::Arc<CachedDeepRecallIndex>, String> {
    let owner_key = deep_recall_owner_key(owner_key);
    {
        let cache = deep_recall_index_cache()
            .lock()
            .map_err(|err| format!("锁定深度回忆索引缓存失败: {err}"))?;
        if let Some(cached) = cache.get(&owner_key) {
            return Ok(cached.clone());
        }
    }
    let collection = deep_recall_collect_conversations(state, agent_id)?;
    let cached = std::sync::Arc::new(deep_recall_build_index(&owner_key, collection)?);
    let mut cache = deep_recall_index_cache()
        .lock()
        .map_err(|err| format!("锁定深度回忆索引缓存失败: {err}"))?;
    cache.insert(owner_key, cached.clone());
    Ok(cached)
}

/// 释放指定委托会话的内存索引（该委托结束时调用）。
fn deep_recall_index_release(owner_key: &str) -> bool {
    let owner_key = owner_key.trim();
    if owner_key.is_empty() {
        return false;
    }
    let Ok(mut cache) = deep_recall_index_cache().lock() else {
        runtime_log_warn("[深度回忆] 释放内存索引失败 原因=锁定缓存失败".to_string());
        return false;
    };
    let released = cache.remove(owner_key).is_some();
    if released {
        runtime_log_info(format!(
            "[深度回忆] 内存索引已释放 owner_key={owner_key} 剩余={}",
            cache.len()
        ));
    }
    released
}

/// 兜底释放全部内存索引：仅在无法确认本次委托归属时使用。
fn deep_recall_index_clear_all() -> bool {
    let Ok(mut cache) = deep_recall_index_cache().lock() else {
        runtime_log_warn("[深度回忆] 释放内存索引失败 原因=锁定缓存失败".to_string());
        return false;
    };
    let released = !cache.is_empty();
    let count = cache.len();
    cache.clear();
    if released {
        runtime_log_info(format!("[深度回忆] 内存索引已全部释放 数量={count}"));
    }
    released
}

fn deep_recall_tantivy_search(
    cached: &CachedDeepRecallIndex,
    agent_id: &str,
    query_text: &str,
    limit: usize,
) -> Result<Vec<(usize, usize, f64, f64)>, String> {
    if cached.conversations.is_empty() || query_text.trim().is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let terms = memory_tokenize_terms(query_text, true);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let searcher = cached.reader.searcher();
    let query_parser = QueryParser::for_index(&cached.index, vec![cached.fields.content]);
    let query_text = memory_build_any_terms_query("content", &terms);
    let content_query = query_parser
        .parse_query(&query_text)
        .map_err(|err| format!("解析深度回忆检索式失败: {err}"))?;
    // 索引构建时已按归属裁剪，这里再挂一次归属条件：万一缓存槽与人格式身份错配，
    // 越权在检索这一层就被挡住，不依赖「同槽必然同人格」这一假设。
    let owner_term = tantivy::Term::from_field_text(cached.fields.owner_agent_id, agent_id);
    let owner_query: Box<dyn tantivy::query::Query> = Box::new(tantivy::query::TermQuery::new(
        owner_term,
        IndexRecordOption::Basic,
    ));
    let parsed = tantivy::query::BooleanQuery::new(vec![
        (tantivy::query::Occur::Must, owner_query),
        (tantivy::query::Occur::Must, content_query),
    ]);
    let hits = searcher
        .search(&parsed, &TopDocs::with_limit(limit).order_by_score())
        .map_err(|err| format!("深度回忆 BM25 检索失败: {err}"))?;
    let max_score = hits
        .iter()
        .map(|(score, _)| *score as f64)
        .fold(0.0f64, f64::max);
    let mut out = Vec::<(usize, usize, f64, f64)>::new();
    for (score, addr) in hits {
        let document: tantivy::schema::TantivyDocument = searcher
            .doc(addr)
            .map_err(|err| format!("读取深度回忆命中文档失败: {err}"))?;
        let conv_idx = document
            .get_first(cached.fields.conv_idx)
            .and_then(|value| value.as_u64())
            .ok_or_else(|| "读取深度回忆会话序号失败".to_string())? as usize;
        let msg_idx = document
            .get_first(cached.fields.msg_idx)
            .and_then(|value| value.as_u64())
            .ok_or_else(|| "读取深度回忆消息序号失败".to_string())? as usize;
        let raw = score as f64;
        let normalized = if max_score > 0.0 {
            (raw / max_score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push((conv_idx, msg_idx, raw, normalized));
    }
    Ok(out)
}

/// 会话对某人格是否可见：判据是会话归属人格，而不是谁在会话里说过话。
/// 归属人格是该会话记忆的归属者，会话内所有人的发言都算它的记忆。
fn deep_recall_conversation_visible_to(
    conversation: &DeepRecallConversation,
    agent_id: &str,
) -> bool {
    conversation.owner_agent_id == agent_id
}

/// 命中片段截断：检索结果只用于定位，超出上限的正文交给 deeprecall_context。
fn deep_recall_snippet(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= DEEP_RECALL_SNIPPET_MAX_CHARS {
        return trimmed.to_string();
    }
    let mut snippet = trimmed
        .chars()
        .take(DEEP_RECALL_SNIPPET_MAX_CHARS)
        .collect::<String>();
    snippet.push('…');
    snippet
}

/// 深度回忆检索：返回命中消息的会话序号 / 消息序号与正文片段（只做定位，不搬上下文）。
fn deep_recall_search(
    state: &AppState,
    owner_key: &str,
    agent_id: &str,
    query: &str,
    limit: Option<usize>,
) -> Result<Value, String> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return Err("deeprecall_search 缺少当前人格标识".to_string());
    }
    let limit = limit
        .unwrap_or(DEEP_RECALL_SEARCH_DEFAULT_LIMIT)
        .clamp(1, DEEP_RECALL_SEARCH_MAX_LIMIT);
    let started = std::time::Instant::now();
    let cached = deep_recall_index_for_key(state, owner_key, agent_id)?;

    let mut hits = Vec::<Value>::new();
    if query.trim().is_empty() {
        return Ok(serde_json::json!({
            "hits": hits,
            "totalConversations": cached.conversations.len(),
            "elapsedMs": started.elapsed().as_millis(),
        }));
    }
    let hit_limit = limit.saturating_mul(4).max(limit);
    for (conv_idx, msg_idx, _raw, normalized) in
        deep_recall_tantivy_search(&cached, agent_id, query, hit_limit)?
    {
        let Some(conversation) = cached
            .conversations
            .iter()
            .find(|conversation| conversation.index == conv_idx)
        else {
            continue;
        };
        let Some(message) = conversation
            .messages
            .iter()
            .find(|message| message.index == msg_idx)
        else {
            continue;
        };
        hits.push(serde_json::json!({
            "conversationIndex": conversation.index,
            "messageIndex": message.index,
            "conversationTitle": conversation.title,
            "sourceKind": conversation.source_kind,
            "time": message.time,
            "speaker": message.speaker,
            "snippet": deep_recall_snippet(&message.text),
            "score": normalized,
        }));
        if hits.len() >= limit {
            break;
        }
    }
    Ok(serde_json::json!({
        "hits": hits,
        "totalConversations": cached.conversations.len(),
        "elapsedMs": started.elapsed().as_millis(),
    }))
}

/// 深度回忆上下文展开：按会话序号 + 起止消息序号截取闭区间消息。
fn deep_recall_context(
    state: &AppState,
    owner_key: &str,
    agent_id: &str,
    conversation_index: usize,
    start_index: usize,
    end_index: usize,
) -> Result<Value, String> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return Err("deeprecall_context 缺少当前人格标识".to_string());
    }
    if start_index == 0 || end_index == 0 {
        return Err("deeprecall_context 消息序号从 1 开始，不能为 0".to_string());
    }
    if start_index > end_index {
        return Err(format!(
            "deeprecall_context 参数错误：start_index={start_index} 不能大于 end_index={end_index}"
        ));
    }
    let cached = deep_recall_index_for_key(state, owner_key, agent_id)?;
    let conversation = cached
        .conversations
        .iter()
        .find(|conversation| conversation.index == conversation_index)
        .ok_or_else(|| {
            format!(
                "deeprecall_context 会话序号不存在：conversation_index={conversation_index}，当前索引共 {} 个会话",
                cached.conversations.len()
            )
        })?;
    // 与检索同源的可见性断言：命中列表已按人格过滤，展开也不允许越过会话可见范围。
    if !deep_recall_conversation_visible_to(conversation, agent_id) {
        return Err(format!(
            "deeprecall_context 无权访问会话 {conversation_index}：该会话不在当前人格的可见范围内"
        ));
    }
    let total = conversation.messages.len();
    if start_index > total {
        return Err(format!(
            "deeprecall_context 消息序号超出范围：会话 {} 共 {} 条消息，start_index={start_index}",
            conversation_index, total
        ));
    }

    let (sliced, truncated) =
        deep_recall_context_slice(&conversation.messages, start_index, end_index);
    let messages = sliced
        .iter()
        .map(|message| {
            serde_json::json!({
                "index": message.index,
                "speaker": message.speaker,
                "time": message.time,
                "text": message.text,
            })
        })
        .collect::<Vec<Value>>();
    Ok(serde_json::json!({
        "conversationIndex": conversation.index,
        "conversationTitle": conversation.title,
        "startIndex": start_index,
        "endIndex": end_index.min(total),
        "totalMessages": total,
        "truncated": truncated,
        "messages": messages,
    }))
}

/// 按闭区间截取消息，返回（消息列表，是否发生截断）。
/// end_index 超出会话末尾时截到末尾，超出条数上限时截到上限。
fn deep_recall_context_slice(
    messages: &[DeepRecallMessage],
    start_index: usize,
    end_index: usize,
) -> (Vec<&DeepRecallMessage>, bool) {
    let total = messages.len();
    let clamped_end = end_index.min(total);
    let mut truncated = end_index > total;
    let mut out = Vec::<&DeepRecallMessage>::new();
    for message in messages
        .iter()
        .filter(|message| message.index >= start_index && message.index <= clamped_end)
    {
        if out.len() >= DEEP_RECALL_CONTEXT_MAX_MESSAGES {
            truncated = true;
            break;
        }
        out.push(message);
    }
    (out, truncated)
}

// ==================== deeprecall 委托发起 ====================

const DEEP_RECALL_WORKFLOW_WHY: &str = "这是深度回忆任务：发起者要从自己的全部历史聊天记录中找回指定话题的来龙去脉。重建的记忆索引已就绪，检索范围仅限发起者本人可见的会话，并已过滤掉工具调用等非正文内容。";

const DEEP_RECALL_WORKFLOW_TODO: &str = "1. 你拥有两个记忆检索工具：deeprecall_search（按关键词检索全部历史聊天正文，命中返回会话序号与消息序号）、deeprecall_context（按会话序号与起止消息序号展开那段真实对话）。\n2. 把回忆目标拆成多个检索角度，用 deeprecall_search 多轮检索：换关键词、换说法、换时间线索，命中不足就换角度继续，不要只搜一次。\n3. 对可信命中用 deeprecall_context 展开前后文，核对来龙去脉、时间顺序与结论演变，把散落的片段串成一条线。\n4. 整合成一份回忆报告：时间线、关键结论、分歧点与演变过程，并为每条结论标注来源坐标（会话 00XX / 消息 00XX）。\n5. 检索不到就如实说明搜过什么、为什么没找到，禁止虚构任何未检索到的内容。";

fn deep_recall_delegate_args(source_agent_id: &str, query: &str) -> DelegateToolArgs {
    DelegateToolArgs {
        agent_id: source_agent_id.trim().to_string(),
        mode: Some("wait".to_string()),
        why: Some(DEEP_RECALL_WORKFLOW_WHY.to_string()),
        goal: Some(format!("深度回忆：{}", query.trim())),
        todo: Some(DEEP_RECALL_WORKFLOW_TODO.to_string()),
        background: None,
        question: None,
        focus: None,
    }
}

/// deeprecall 工具主体：委托发起者自己去回忆，同步等待结束再回填报告。
async fn builtin_deep_recall(
    app_state: &AppState,
    session_id: &str,
    source_agent_id: &str,
    args: DeepRecallToolArgs,
) -> Result<Value, String> {
    let query = args.query.trim();
    if query.is_empty() {
        return Ok(serde_json::json!({
            "ok": false,
            "status": "深度回忆无法发起",
            "reason": "deeprecall.query is required",
            "message": "深度回忆工具执行失败"
        }));
    }
    if source_agent_id.trim().is_empty() {
        return Ok(serde_json::json!({
            "ok": false,
            "status": "深度回忆无法发起",
            "reason": "缺少发起人格，无法确定记忆归属",
            "message": "深度回忆工具执行失败"
        }));
    }

    runtime_log_info(format!(
        "[深度回忆] 发起回忆委托 query_len={} agent_id={}",
        query.chars().count(),
        source_agent_id.trim()
    ));
    let result = delegate_execute_sync(
        app_state,
        session_id,
        Some(source_agent_id),
        DELEGATE_TOOL_KIND_DEEP_RECALL,
        deep_recall_delegate_args(source_agent_id, query),
        None,
    )
    .await;
    // 委托已结束，按本次委托会话释放它占用的内存索引；归属不明时兜底清理。
    match deep_recall_result_delegate_id(&result) {
        Some(delegate_id) => {
            deep_recall_index_release(&delegate_id);
        }
        None => {
            deep_recall_index_clear_all();
        }
    }
    result
}

/// 从同步委托返回值里取本次委托 id（等于委托会话 id），用于精确释放内存索引。
fn deep_recall_result_delegate_id(result: &Result<Value, String>) -> Option<String> {
    let delegate_id = result
        .as_ref()
        .ok()?
        .get("delegate")?
        .get("delegateId")?
        .as_str()?
        .trim();
    if delegate_id.is_empty() {
        return None;
    }
    Some(delegate_id.to_string())
}

#[cfg(test)]
mod deep_recall_tests {
    use super::*;

    fn test_message(index: usize, speaker: Option<&str>, text: &str) -> DeepRecallMessage {
        let speaker = speaker.unwrap_or("用户").to_string();
        DeepRecallMessage {
            index,
            speaker: speaker.clone(),
            time: "2026-01-01T00:00:00Z".to_string(),
            text: format!("[{speaker}]: {text}"),
        }
    }

    fn test_conversation(
        index: usize,
        owner_agent_id: &str,
        messages: Vec<DeepRecallMessage>,
    ) -> DeepRecallConversation {
        DeepRecallConversation {
            index,
            title: format!("会话{index}"),
            source_kind: "localConversation".to_string(),
            messages,
            owner_agent_id: owner_agent_id.to_string(),
        }
    }

    #[test]
    fn deep_recall_index_should_filter_by_conversation_owner() {
        let conversations = vec![
            // 归属 agent-a，但会话里只有 agent-b 在说：agent-a 仍应搜到自己的这段记忆。
            test_conversation(
                1,
                "agent-a",
                vec![
                    test_message(1, Some("agent-b"), "我们讨论一下部署方案，用容器还是直装"),
                    test_message(2, Some("agent-b"), "今天天气不错"),
                ],
            ),
            // 归属 agent-b，agent-a 在里面对过话：agent-a 不该搜到别人的会话。
            test_conversation(
                2,
                "agent-b",
                vec![test_message(1, Some("agent-a"), "部署方案已经改成容器化了")],
            ),
        ];
        let tokenized = conversations
            .iter()
            .map(|conversation| {
                conversation
                    .messages
                    .iter()
                    .map(|message| memory_tokenize_terms(&message.text, false).join(" "))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cached = deep_recall_build_index(
            "test-owner",
            DeepRecallCollection {
                conversations,
                tokenized,
            },
        )
        .expect("build index");

        let hits_a =
            deep_recall_tantivy_search(&cached, "agent-a", "部署方案", 10).expect("search agent a");
        assert_eq!(hits_a.len(), 1);
        assert_eq!(hits_a[0].0, 1);
        assert_eq!(hits_a[0].1, 1);
        assert!(hits_a[0].3 > 0.0 && hits_a[0].3 <= 1.0);

        let hits_b =
            deep_recall_tantivy_search(&cached, "agent-b", "部署方案", 10).expect("search agent b");
        assert_eq!(hits_b.len(), 1);
        assert_eq!(hits_b[0].0, 2);
        assert_eq!(hits_b[0].1, 1);

        let empty =
            deep_recall_tantivy_search(&cached, "agent-c", "部署方案", 10).expect("search agent c");
        assert!(empty.is_empty());
    }

    #[test]
    fn deep_recall_context_slice_should_use_closed_range_and_clamp_end() {
        let messages = (1..=5)
            .map(|index| test_message(index, Some("agent-a"), "内容"))
            .collect::<Vec<_>>();
        let (out, truncated) = deep_recall_context_slice(&messages, 2, 4);
        assert_eq!(
            out.iter().map(|message| message.index).collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
        assert!(!truncated);

        let (out, truncated) = deep_recall_context_slice(&messages, 3, 99);
        assert_eq!(
            out.iter().map(|message| message.index).collect::<Vec<_>>(),
            vec![3, 4, 5]
        );
        assert!(truncated);
    }

    #[test]
    fn deep_recall_context_slice_should_cap_message_count() {
        let count = DEEP_RECALL_CONTEXT_MAX_MESSAGES + 20;
        let messages = (1..=count)
            .map(|index| test_message(index, Some("agent-a"), "内容"))
            .collect::<Vec<_>>();
        let (out, truncated) = deep_recall_context_slice(&messages, 1, count);
        assert_eq!(out.len(), DEEP_RECALL_CONTEXT_MAX_MESSAGES);
        assert!(truncated);
    }

    #[test]
    fn deep_recall_policy_should_limit_search_tools_to_deep_recall_delegate() {
        // 深度回忆委托会话：检索工具可用
        assert_eq!(
            builtin_tool_runtime_unavailable_reason(
                "deeprecall_search",
                RuntimeToolOriginScope::Local,
                true,
                false,
                true,
                false,
                false,
                true
            ),
            None
        );
        assert_eq!(
            builtin_tool_runtime_unavailable_reason(
                "deeprecall_context",
                RuntimeToolOriginScope::Local,
                true,
                false,
                true,
                false,
                false,
                true
            ),
            None
        );
        // 普通委托会话：检索工具不可用
        assert!(builtin_tool_runtime_unavailable_reason(
            "deeprecall_search",
            RuntimeToolOriginScope::Local,
            true,
            false,
            true,
            false,
            false,
            false
        )
        .is_some());
        // 本地会话：检索工具不可用
        assert!(builtin_tool_runtime_unavailable_reason(
            "deeprecall_search",
            RuntimeToolOriginScope::Local,
            true,
            true,
            false,
            false,
            false,
            false
        )
        .is_some());
    }

    #[test]
    fn deep_recall_render_should_only_keep_text_parts_and_extra_blocks() {
        let message = ChatMessage {
            id: "m1".to_string(),
            role: "assistant".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![
                MessagePart::Text {
                    text: "部署方案定为容器化".to_string(),
                    reasoning_content: None,
                },
                MessagePart::Image {
                    mime: "image/png".to_string(),
                    bytes_base64: "AAAA".to_string(),
                    name: None,
                    compressed: false,
                },
            ],
            extra_text_blocks: vec!["补充：回滚脚本在 docs 里".to_string()],
            provider_meta: None,
            tool_call: Some(vec![serde_json::json!({
                "name": "exec",
                "arguments": "{\"command\":\"rm -rf /tmp/demo\"}"
            })]),
            mcp_call: None,
            meme_annotations: None,
        };
        let agents = Vec::<AgentProfile>::new();
        let rendered =
            chat_history_render_message(&message, &agents, "红豆").expect("render message");

        assert!(rendered.rendered.contains("部署方案定为容器化"));
        assert!(rendered.rendered.contains("补充：回滚脚本在 docs 里"));
        // 工具调用与图片附件不进入回忆正文。
        assert!(!rendered.rendered.contains("rm -rf"));
        assert!(!rendered.rendered.contains("AAAA"));
        assert_eq!(rendered.speaker_name, "agent-a");
    }

    #[test]
    fn deep_recall_delegate_args_should_target_self_in_wait_mode() {
        let args = deep_recall_delegate_args("agent-a", "上个月我们讨论过的部署方案");
        assert_eq!(args.agent_id, "agent-a");
        assert_eq!(args.mode.as_deref(), Some("wait"));
        assert_eq!(
            args.goal.as_deref(),
            Some("深度回忆：上个月我们讨论过的部署方案")
        );
        assert!(args.background.is_none());
        assert!(args
            .todo
            .as_deref()
            .is_some_and(|todo| todo.contains("deeprecall_search")
                && todo.contains("deeprecall_context")));
    }

    #[test]
    fn deep_recall_policy_should_limit_main_tool_to_local_conversation() {        // 本地会话：deeprecall 可用
        assert_eq!(
            builtin_tool_runtime_unavailable_reason(
                "deeprecall",
                RuntimeToolOriginScope::Local,
                true,
                true,
                false,
                false,
                false,
                false
            ),
            None
        );
        // 委托会话：deeprecall 不可用，避免递归委托
        assert!(builtin_tool_runtime_unavailable_reason(
            "deeprecall",
            RuntimeToolOriginScope::Local,
            true,
            false,
            true,
            false,
            false,
            false
        )
        .is_some());
    }

    /// 手动基准：并行收集与建索引各自的耗时。
    /// 环境变量指向数据根下的锚点路径（例如 `<data>/config_mark`），不是数据根目录本身：
    /// 数据根由该路径的父目录反推，直接传数据根会读不到 chat 目录。
    /// `PA_DEEP_RECALL_DATA=<data>/config_mark cargo test bench_deep_recall_collect -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_deep_recall_collect() {
        let Ok(data_path) = std::env::var("PA_DEEP_RECALL_DATA") else {
            return;
        };
        let state = AppState {
            data_path: std::path::PathBuf::from(data_path),
            ..AppState::new().expect("create test app state")
        };
        let chat_index = state_read_chat_index_cached(&state).expect("read chat index");
        let mut agent_id = String::new();
        for item in &chat_index.conversations {
            let Ok(meta) = conversation_service_v2().get_conversation_meta(&state, &item.id)
            else {
                continue;
            };
            if chat_history_source_kind_from_meta(&meta).is_none() {
                continue;
            }
            let owner = meta.agent_id.trim().to_string();
            if !owner.is_empty() {
                agent_id = owner;
                break;
            }
        }
        assert!(!agent_id.is_empty(), "样本里没有可用人格");

        let started = std::time::Instant::now();
        let collection = deep_recall_collect_conversations(&state, &agent_id).expect("collect");
        println!(
            "收集完成 会话数={} 耗时={}ms",
            collection.conversations.len(),
            started.elapsed().as_millis()
        );
        let build_started = std::time::Instant::now();
        let cached = deep_recall_build_index("bench-owner", collection).expect("build index");
        println!(
            "建索引完成 耗时={}ms 会话数={}",
            build_started.elapsed().as_millis(),
            cached.conversations.len()
        );
    }
}
