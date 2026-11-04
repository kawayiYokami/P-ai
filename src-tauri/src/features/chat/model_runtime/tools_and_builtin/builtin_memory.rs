fn normalize_memory_keywords(raw: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for item in raw {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v = trimmed.to_string();
        if !out.iter().any(|x| x == &v) {
            out.push(v);
        }
    }
    out
}

fn memory_contains_sensitive(content: &str, keywords: &[String]) -> bool {
    let mut full = content.to_lowercase();
    if !keywords.is_empty() {
        full.push('\n');
        full.push_str(&keywords.join(" ").to_lowercase());
    }
    let danger_tokens = [
        "password",
        "passwd",
        "api key",
        "apikey",
        "token",
        "secret",
        "private key",
        "sk-",
        "ssh-rsa",
        "验证码",
        "密码",
        "密钥",
        "身份证",
        "银行卡",
        "cvv",
    ];
    danger_tokens.iter().any(|token| full.contains(token))
}

#[derive(Debug, Clone)]
struct MemorySaveDraft {
    memory_type: String,
    judgment: String,
    reasoning: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MemorySaveUpsertItemResult {
    saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn parse_memory_save_draft(
    memory_type_raw: &str,
    judgment: &str,
    reasoning: &str,
    tags_raw: Vec<String>,
) -> Result<MemorySaveDraft, String> {
    let judgment = judgment.trim();
    if judgment.is_empty() {
        return Err("memory_save.judgment is required".to_string());
    }
    let memory_type = memory_store_normalize_memory_type(memory_type_raw)?;
    let tags = normalize_memory_keywords(&tags_raw);
    if tags.is_empty() {
        return Err("memory_save.tags must contain at least one valid tag".to_string());
    }
    if memory_contains_sensitive(judgment, &tags) {
        return Err("memory_save 含敏感内容，已阻止写入".to_string());
    }
    Ok(MemorySaveDraft {
        memory_type,
        judgment: judgment.to_string(),
        reasoning: clean_text(reasoning),
        tags,
    })
}

#[cfg(test)]
mod builtin_memory_tests {
    use super::*;

    #[test]
    fn normalize_memory_keywords_should_preserve_search_anchor_text() {
        let tags = normalize_memory_keywords(&vec![
            " 用户昵称 ".to_string(),
            USER_PERSONA_ID.to_string(),
            "remote-user-Alpha@im.test".to_string(),
            "用户要求".to_string(),
            " ".to_string(),
            "额外标签一".to_string(),
            "额外标签二".to_string(),
            "额外标签三".to_string(),
            "额外标签四".to_string(),
            "额外标签五".to_string(),
            "额外标签六".to_string(),
            "额外标签七".to_string(),
            "额外标签八".to_string(),
            "额外标签九".to_string(),
        ]);
        assert_eq!(
            tags,
            vec![
                "用户昵称".to_string(),
                USER_PERSONA_ID.to_string(),
                "remote-user-Alpha@im.test".to_string(),
                "用户要求".to_string(),
                "额外标签一".to_string(),
                "额外标签二".to_string(),
                "额外标签三".to_string(),
                "额外标签四".to_string(),
                "额外标签五".to_string(),
                "额外标签六".to_string(),
                "额外标签七".to_string(),
                "额外标签八".to_string(),
                "额外标签九".to_string(),
            ]
        );
    }

    #[test]
    fn resolve_memory_source_ids_should_accept_display_id_and_uuid() {
        let memories = vec![
            MemoryEntry {
                id: "memory-uuid-a".to_string(),
                memory_no: Some(12),
                memory_type: "knowledge".to_string(),
                judgment: "测试用户喜欢直接结论".to_string(),
                reasoning: String::new(),
                tags: vec!["测试用户".to_string()],
                owner_agent_id: None,
                created_at: "2026-05-12T00:00:00Z".to_string(),
                updated_at: "2026-05-12T00:00:00Z".to_string(),
            },
            MemoryEntry {
                id: "memory-uuid-b".to_string(),
                memory_no: Some(19),
                memory_type: "skill".to_string(),
                judgment: "测试用户偏好先修 bug".to_string(),
                reasoning: String::new(),
                tags: vec!["测试用户".to_string()],
                owner_agent_id: None,
                created_at: "2026-05-12T00:00:00Z".to_string(),
                updated_at: "2026-05-12T00:00:00Z".to_string(),
            },
        ];

        let resolved = resolve_memory_source_ids_from_memories(
            &memories,
            &["12".to_string(), "memory-uuid-b".to_string(), "12".to_string()],
        )
        .expect("resolve source ids");

        assert_eq!(
            resolved,
            vec!["memory-uuid-a".to_string(), "memory-uuid-b".to_string()]
        );
    }

    #[test]
    fn memory_save_validate_action_sources_should_require_merge_sources() {
        assert!(memory_save_validate_action_sources("create", &[]).is_ok());
        assert!(memory_save_validate_action_sources("create", &["a".to_string()]).is_err());
        assert!(memory_save_validate_action_sources("update", &["a".to_string()]).is_ok());
        assert!(memory_save_validate_action_sources("update", &["a".to_string(), "b".to_string()]).is_err());
        assert!(memory_save_validate_action_sources("merge", &["a".to_string()]).is_err());
        assert!(memory_save_validate_action_sources("merge", &["a".to_string(), "b".to_string()]).is_ok());
    }

    #[test]
    fn normalize_recall_time_filter_should_accept_year_month_day_only() {
        assert_eq!(normalize_recall_time_filter(Some("2026")).unwrap(), Some("2026".to_string()));
        assert_eq!(normalize_recall_time_filter(Some("2026-05")).unwrap(), Some("2026-05".to_string()));
        assert_eq!(normalize_recall_time_filter(Some("2026-05-12")).unwrap(), Some("2026-05-12".to_string()));
        assert!(normalize_recall_time_filter(Some("2026-13")).is_err());
        assert!(normalize_recall_time_filter(Some("2026/05")).is_err());
    }

    #[test]
    fn order_recall_memory_ids_should_preserve_search_order_for_query() {
        let memories = vec![
            MemoryEntry {
                id: "older-more-relevant".to_string(),
                memory_no: Some(1),
                memory_type: "knowledge".to_string(),
                judgment: "更相关但更旧".to_string(),
                reasoning: String::new(),
                tags: vec!["相关".to_string()],
                owner_agent_id: None,
                created_at: "2026-05-01T00:00:00Z".to_string(),
                updated_at: "2026-05-01T00:00:00Z".to_string(),
            },
            MemoryEntry {
                id: "newer-less-relevant".to_string(),
                memory_no: Some(2),
                memory_type: "knowledge".to_string(),
                judgment: "较新但排序靠后".to_string(),
                reasoning: String::new(),
                tags: vec!["相关".to_string()],
                owner_agent_id: None,
                created_at: "2026-05-30T00:00:00Z".to_string(),
                updated_at: "2026-05-30T00:00:00Z".to_string(),
            },
        ];
        let candidate_ids = vec![
            "older-more-relevant".to_string(),
            "newer-less-relevant".to_string(),
        ];

        let ordered = order_recall_memory_ids(&memories, &candidate_ids, None, false);
        assert_eq!(ordered, candidate_ids);
    }

    #[test]
    fn order_recall_memory_ids_should_sort_browse_mode_by_time() {
        let memories = vec![
            MemoryEntry {
                id: "older".to_string(),
                memory_no: Some(1),
                memory_type: "knowledge".to_string(),
                judgment: "旧".to_string(),
                reasoning: String::new(),
                tags: vec![],
                owner_agent_id: None,
                created_at: "2026-05-01T00:00:00Z".to_string(),
                updated_at: "2026-05-01T00:00:00Z".to_string(),
            },
            MemoryEntry {
                id: "newer".to_string(),
                memory_no: Some(2),
                memory_type: "knowledge".to_string(),
                judgment: "新".to_string(),
                reasoning: String::new(),
                tags: vec![],
                owner_agent_id: None,
                created_at: "2026-05-30T00:00:00Z".to_string(),
                updated_at: "2026-05-30T00:00:00Z".to_string(),
            },
        ];

        let ordered = order_recall_memory_ids(&memories, &[], None, true);
        assert_eq!(ordered, vec!["newer".to_string(), "older".to_string()]);
    }
}

#[derive(Debug, Clone)]
struct MemoryAgentContext {
    owner_agent_id: Option<String>,
    effective_agent_id: String,
    private_memory_enabled: bool,
    recall_enabled: bool,
}

fn build_memory_agent_context(
    agent_id: &str,
    private_memory_enabled: bool,
    recall_enabled: bool,
) -> Result<MemoryAgentContext, String> {
    let effective_agent_id = agent_id.trim();
    if effective_agent_id.is_empty() {
        return Err("缺少当前人格 ID，无法读写记忆。".to_string());
    }
    Ok(MemoryAgentContext {
        owner_agent_id: private_memory_enabled.then_some(effective_agent_id.to_string()),
        effective_agent_id: effective_agent_id.to_string(),
        private_memory_enabled,
        recall_enabled,
    })
}

fn memory_agent_context_from_agent(agent: &AgentProfile) -> Result<MemoryAgentContext, String> {
    if agent.is_built_in_user {
        return Err(format!("当前人格不支持读写记忆：agent_id={}", agent.id));
    }
    build_memory_agent_context(
        &agent.id,
        agent.private_memory_enabled,
        agent_memory_recall_enabled(agent),
    )
}

fn upsert_memories(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    drafts: &[MemorySaveDraft],
) -> Result<(Vec<MemorySaveUpsertItemResult>, usize), String> {
    let inputs = drafts
        .iter()
        .map(|d| MemoryDraftInput {
            memory_type: d.memory_type.clone(),
            judgment: d.judgment.clone(),
            reasoning: d.reasoning.clone(),
            tags: d.tags.clone(),
            owner_agent_id: memory_context.owner_agent_id.clone(),
        })
        .collect::<Vec<_>>();
    let (results, total_memories) = memory_store_upsert_drafts(&app_state.data_path, &inputs)?;
    let loop_started_at = std::time::Instant::now();
    for (draft, result) in drafts.iter().zip(results.iter()) {
        let elapsed_ms = loop_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let trigger = if result.id.is_some() {
            "result.id=Some"
        } else {
            "result.id=None"
        };
        let id_text = result.id.as_deref().unwrap_or("none");
        runtime_log_info(format!(
            "[简单记忆回灌] 任务名=简单记忆回灌 状态=完成 触发条件={} id={} memory_type={} tags={} judgment_len={} tags_count={} 耗时毫秒={}",
            trigger,
            id_text,
            draft.memory_type,
            draft.tags.join(","),
            draft.judgment.chars().count(),
            draft.tags.len(),
            elapsed_ms
        ));
    }
    Ok((results, total_memories))
}

fn memory_save_action(args: &Value) -> Result<String, String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "remember.action is required".to_string())?
        .to_ascii_lowercase();
    match action.as_str() {
        "create" | "update" | "merge" => Ok(action),
        _ => Err(format!(
            "remember.action must be create, update, or merge, got `{}`",
            action
        )),
    }
}

fn memory_save_payload(args: &Value) -> Result<&Value, String> {
    args.get("memory")
        .filter(|value| value.is_object())
        .ok_or_else(|| "remember.memory is required".to_string())
}

fn memory_save_source_ids(args: &Value) -> Vec<String> {
    args.get("sourceMemoryIds")
        .or_else(|| args.get("source_memory_ids"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .fold(Vec::<String>::new(), |mut acc, item| {
                    if !acc.iter().any(|existing| existing == &item) {
                        acc.push(item);
                    }
                    acc
                })
        })
        .unwrap_or_default()
}

fn resolve_memory_source_ids(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    raw_ids: &[String],
) -> Result<Vec<String>, String> {
    if raw_ids.is_empty() {
        return Ok(Vec::new());
    }
    let memories = memory_store_list_memories_visible_for_agent(
        &app_state.data_path,
        &memory_context.effective_agent_id,
        memory_context.private_memory_enabled,
    )?;
    resolve_memory_source_ids_from_memories(&memories, raw_ids)
}

fn resolve_memory_source_ids_from_memories(
    memories: &[MemoryEntry],
    raw_ids: &[String],
) -> Result<Vec<String>, String> {
    let mut resolved = Vec::<String>::new();
    for raw_id in raw_ids {
        let needle = raw_id.trim();
        if needle.is_empty() {
            continue;
        }
        let Some(memory) = memories
            .iter()
            .find(|memory| memory.id == needle || memory.display_id() == needle)
        else {
            return Err(format!("remember.sourceMemoryIds 包含不可见或不存在的记忆 ID：{}", needle));
        };
        if !resolved.iter().any(|id| id == &memory.id) {
            resolved.push(memory.id.clone());
        }
    }
    Ok(resolved)
}

fn memory_save_validate_action_sources(action: &str, source_ids: &[String]) -> Result<(), String> {
    match action {
        "create" if source_ids.is_empty() => Ok(()),
        "create" => Err("remember.action=create must not include sourceMemoryIds".to_string()),
        "update" if source_ids.len() == 1 => Ok(()),
        "merge" if source_ids.len() >= 2 => Ok(()),
        "update" => Err("remember.action=update requires exactly one sourceMemoryIds item".to_string()),
        "merge" => Err("remember.action=merge requires at least two sourceMemoryIds items".to_string()),
        _ => Err(format!("remember.action 不支持：{}", action)),
    }
}

fn delete_replaced_memory_sources(
    app_state: &AppState,
    source_ids: &[String],
    retained_ids: &[String],
) -> (Vec<String>, Vec<Value>) {
    let retained = retained_ids.iter().map(|id| id.as_str()).collect::<std::collections::HashSet<_>>();
    let mut deleted = Vec::<String>::new();
    let mut failed = Vec::<Value>::new();
    for source_id in source_ids {
        if retained.contains(source_id.as_str()) {
            continue;
        }
        match memory_store_delete_memory(&app_state.data_path, source_id) {
            Ok(()) => deleted.push(source_id.clone()),
            Err(err) => failed.push(serde_json::json!({
                "id": source_id,
                "error": err,
            })),
        }
    }
    (deleted, failed)
}

fn memory_display_id_for_raw_id(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    raw_id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(raw_id) = raw_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let memories = memory_store_list_memories_visible_for_agent(
        &app_state.data_path,
        &memory_context.effective_agent_id,
        memory_context.private_memory_enabled,
    )?;
    Ok(memories
        .iter()
        .find(|memory| memory.id == raw_id)
        .map(MemoryEntry::display_id))
}

fn memory_display_ids_for_raw_ids(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    raw_ids: &[String],
) -> Result<std::collections::HashMap<String, String>, String> {
    if raw_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let memories = memory_store_list_memories_visible_for_agent(
        &app_state.data_path,
        &memory_context.effective_agent_id,
        memory_context.private_memory_enabled,
    )?;
    let raw_set = raw_ids.iter().map(|id| id.as_str()).collect::<std::collections::HashSet<_>>();
    Ok(memories
        .iter()
        .filter(|memory| raw_set.contains(memory.id.as_str()))
        .map(|memory| (memory.id.clone(), memory.display_id()))
        .collect())
}

fn normalize_recall_time_filter(raw: Option<&str>) -> Result<Option<String>, String> {
    let value = raw.map(str::trim).filter(|value| !value.is_empty());
    let Some(value) = value else {
        return Ok(None);
    };
    let is_year = value.len() == 4 && value.chars().all(|ch| ch.is_ascii_digit());
    let is_month = value.len() == 7
        && value.as_bytes().get(4) == Some(&b'-')
        && value[..4].chars().all(|ch| ch.is_ascii_digit())
        && value[5..].chars().all(|ch| ch.is_ascii_digit());
    let is_day = value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value[..4].chars().all(|ch| ch.is_ascii_digit())
        && value[5..7].chars().all(|ch| ch.is_ascii_digit())
        && value[8..].chars().all(|ch| ch.is_ascii_digit());
    if !is_year && !is_month && !is_day {
        return Err("recall.time must be YYYY, YYYY-MM, or YYYY-MM-DD".to_string());
    }
    if is_month || is_day {
        let month = value[5..7].parse::<u32>().unwrap_or(0);
        if !(1..=12).contains(&month) {
            return Err("recall.time month must be 01-12".to_string());
        }
    }
    if is_day {
        let day = value[8..].parse::<u32>().unwrap_or(0);
        if !(1..=31).contains(&day) {
            return Err("recall.time day must be 01-31".to_string());
        }
    }
    Ok(Some(value.to_string()))
}

fn memory_matches_time_filter(memory: &MemoryEntry, time_prefix: Option<&str>) -> bool {
    let Some(prefix) = time_prefix else {
        return true;
    };
    let updated = memory.updated_at.trim();
    let created = memory.created_at.trim();
    updated.starts_with(prefix) || created.starts_with(prefix)
}

fn order_recall_memory_ids(
    memories: &[MemoryEntry],
    candidate_ids: &[String],
    time_prefix: Option<&str>,
    query_is_empty: bool,
) -> Vec<String> {
    if query_is_empty {
        let mut rows = memories
            .iter()
            .filter(|memory| memory_matches_time_filter(memory, time_prefix))
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            let left_time = if left.updated_at.trim().is_empty() { &left.created_at } else { &left.updated_at };
            let right_time = if right.updated_at.trim().is_empty() { &right.created_at } else { &right.updated_at };
            right_time.cmp(left_time).then_with(|| left.display_id().cmp(&right.display_id()))
        });
        return rows.into_iter().map(|memory| memory.id.clone()).collect();
    }

    let memory_by_id = memories
        .iter()
        .map(|memory| (memory.id.as_str(), memory))
        .collect::<std::collections::HashMap<_, _>>();
    candidate_ids
        .iter()
        .filter_map(|id| memory_by_id.get(id.as_str()).copied())
        .filter(|memory| memory_matches_time_filter(memory, time_prefix))
        .map(|memory| memory.id.clone())
        .collect()
}

fn builtin_memory_save(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    args: Value,
) -> Result<Value, String> {
    let action = memory_save_action(&args)?;
    let raw_source_ids = memory_save_source_ids(&args);
    let source_ids = if action == "create" {
        memory_save_validate_action_sources(&action, &raw_source_ids)?;
        Vec::new()
    } else {
        let resolved = resolve_memory_source_ids(app_state, memory_context, &raw_source_ids)?;
        memory_save_validate_action_sources(&action, &resolved)?;
        resolved
    };
    let payload = memory_save_payload(&args)?;
    let memory_type = payload
        .get("memoryType")
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_save.memoryType is required".to_string())?;
    let judgment = payload
        .get("judgment")
        .and_then(Value::as_str)
        .ok_or_else(|| "memory_save.judgment is required".to_string())?;
    let reasoning = payload
        .get("reasoning")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tags_raw = payload
        .get("tags")
        .and_then(Value::as_array)
        .ok_or_else(|| "memory_save.tags is required".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let draft = parse_memory_save_draft(memory_type, judgment, reasoning, tags_raw)?;
    let source_display_map = memory_display_ids_for_raw_ids(app_state, memory_context, &source_ids)?;
    let display_source_ids = source_ids
        .iter()
        .filter_map(|source_id| source_display_map.get(source_id).cloned())
        .collect::<Vec<_>>();
    if memory_contains_sensitive(&draft.judgment, &draft.tags) {
        runtime_log_warn(format!(
            "[简单记忆回灌] 任务名=简单记忆回灌 状态=跳过 触发条件=敏感内容检测 id=none memory_type={} tags={} judgment_len={} tags_count={} 耗时毫秒=0",
            draft.memory_type,
            draft.tags.join(","),
            draft.judgment.chars().count(),
            draft.tags.len()
        ));
        return Ok(serde_json::json!({
          "ok": false,
          "action": action,
          "saved": false,
          "id": Value::Null,
          "tags": draft.tags,
          "updatedAt": Value::Null,
          "reason": "memory_save 含敏感内容，已阻止写入",
          "sourceMemoryIds": display_source_ids,
          "deletedSourceIds": [],
          "failedDeleteIds": [],
          "totalMemories": Value::Null
        }));
    }
    let (results, total_memories) = upsert_memories(app_state, memory_context, &[draft])?;
    let first = results
        .into_iter()
        .next()
        .ok_or_else(|| "memory_save failed to produce result".to_string())?;
    let raw_saved_id = first.id.clone();
    let display_saved_id = memory_display_id_for_raw_id(
        app_state,
        memory_context,
        raw_saved_id.as_deref(),
    )?;
    let retained_ids = raw_saved_id.iter().cloned().collect::<Vec<_>>();
    let (deleted_source_ids, failed_delete_ids) = if action == "create" || !first.saved {
        (Vec::new(), Vec::new())
    } else {
        delete_replaced_memory_sources(app_state, &source_ids, &retained_ids)
    };
    let display_deleted_source_ids = deleted_source_ids
        .iter()
        .filter_map(|source_id| source_display_map.get(source_id).cloned())
        .collect::<Vec<_>>();
    let display_failed_delete_ids = failed_delete_ids
        .into_iter()
        .map(|item| {
            let raw_id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            serde_json::json!({
                "id": source_display_map.get(raw_id).cloned().unwrap_or_else(|| "unknown".to_string()),
                "error": item.get("error").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    // 回执只留调用方推不出来的事实：存成哪条、成没成、合并替换掉了哪些、库里总共多少。
    // action / tags / reason / updatedAt 都是调用方自己刚提交的原文或内部时间戳，不再抄回。
    let mut output = serde_json::json!({
      "saved": first.saved,
      "id": display_saved_id,
      "totalMemories": total_memories
    });
    if !display_deleted_source_ids.is_empty() {
        output["deletedSourceIds"] = serde_json::json!(display_deleted_source_ids);
    }
    if !display_failed_delete_ids.is_empty() {
        output["failedDeleteIds"] = serde_json::json!(display_failed_delete_ids);
    }
    Ok(output)
}

fn builtin_recall(
    app_state: &AppState,
    memory_context: &MemoryAgentContext,
    query: &str,
    time: Option<&str>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Value, String> {
    let trimmed_query = query.trim();
    let time_prefix = normalize_recall_time_filter(time)?;
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(7).clamp(1, 50);
    if !memory_context.recall_enabled {
        return Ok(serde_json::json!({
          "memories": "",
          "count": 0,
          "total": 0,
          "offset": offset,
          "limit": limit,
          "reason": "memory_recall_disabled"
        }));
    }
    let memories = memory_store_list_memories_visible_for_agent(
        &app_state.data_path,
        &memory_context.effective_agent_id,
        memory_context.private_memory_enabled,
    )?;

    let candidate_ids = if trimmed_query.is_empty() {
        memories.iter().map(|memory| memory.id.clone()).collect::<Vec<_>>()
    } else {
        memory_recall_hit_ids(
            &app_state.data_path,
            &memories,
            trimmed_query,
            MEMORY_RERANK_MIN_SCORE_TOOL,
        )
    };
    let ordered_ids = order_recall_memory_ids(
        &memories,
        &candidate_ids,
        time_prefix.as_deref(),
        trimmed_query.is_empty(),
    );
    let total = ordered_ids.len();
    let page_ids = ordered_ids
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    // 回执正文自己拼，不走系统提示词那条 <memory_context> 拼装：
    // recall 的调用方是模型自己，不需要 XML 外壳来标边界，标签只会变成噪音。
    let memory_board = recall_receipt_board_text(&memories, &page_ids);

    Ok(serde_json::json!({
      "memories": memory_board,
      "count": page_ids.len(),
      "total": total,
      "offset": offset,
      "limit": limit
    }))
}

/// recall 回执里的记忆正文：一行一条 `[id:xxx] 判断句`，有理由时缩进一行。
///
/// 与 `build_memory_board_xml_from_recall_ids` 的区别只有外层：那边给系统提示词用，
/// 需要 `<memory_context>` 标出注入边界；这里是工具回执，边界由工具调用本身划出。
fn recall_receipt_board_text(memories: &[MemoryEntry], recall_ids: &[String]) -> String {
    if memories.is_empty() || recall_ids.is_empty() {
        return String::new();
    }
    let memory_map = memories
        .iter()
        .map(|memory| (memory.id.as_str(), memory))
        .collect::<std::collections::HashMap<_, _>>();
    recall_ids
        .iter()
        .filter_map(|memory_id| memory_map.get(memory_id.as_str()).copied())
        .map(|memory| {
            let reasoning = memory.reasoning.trim();
            if reasoning.is_empty() {
                format!("[id:{}] {}", memory.display_id(), memory.judgment.trim())
            } else {
                format!(
                    "[id:{}] {}\n> {}",
                    memory.display_id(),
                    memory.judgment.trim(),
                    reasoning
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
