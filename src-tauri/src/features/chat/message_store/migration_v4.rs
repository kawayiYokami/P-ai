// ========== V3→V4 迁移（自包含：读 V3 明文 → 拆多行组 → 写 .jsonl.zstd + 更新 sqlite） ==========
//
// V4 存储模型定案（D9/D13/D16，见 .pai/plan/storage/20260822_消息按组多行存储重构计划.md）：
// - 迁移模块完全自包含：旧格式读取、拆分转换、新格式写入全部内部实现，不调用生产环境的读写函数
// - 迁移是历史快照：写死 V4 当时的格式实现，生产 V5 演进（改写入/删旧函数）不牵连迁移
// - 幂等：chat_storage_migrations 表 migration_key，已完成的会话跳过，中断可重入
// - 迁移完成后无明文块残留（旧 .jsonl 删除，只留 .jsonl.zstd）

const MIGRATION_V4_COMPLETED_KEY: &str = "v4_message_group_zstd";

/// 枚举 V3 会话（sqlite conversation_metadata 的 conversation_id）
fn migration_v4_collect_conversation_ids(data_path: &PathBuf) -> Result<Vec<String>, String> {
    let conn = migration_open_v3_database(data_path)?;
    let mut statement = conn
        .prepare("SELECT conversation_id FROM conversation_metadata ORDER BY conversation_id ASC")
        .map_err(|err| format!("V4 迁移准备枚举会话失败: {err}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("V4 迁移枚举会话失败: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("V4 迁移解析会话列表失败: {err}"))
}

/// 读取 V3 全部会话标题（conversation_metadata.metadata_json.title），用于进度回调展示。
fn migration_v4_collect_conversation_titles(
    data_path: &PathBuf,
) -> Result<std::collections::HashMap<String, String>, String> {
    let conn = migration_open_v3_database(data_path)?;
    let mut statement = conn
        .prepare("SELECT conversation_id, metadata_json FROM conversation_metadata")
        .map_err(|err| format!("V4 迁移准备读取会话标题失败: {err}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("V4 迁移读取会话标题失败: {err}"))?;
    let mut titles = std::collections::HashMap::<String, String>::new();
    for row in rows {
        let (conversation_id, metadata_json) =
            row.map_err(|err| format!("V4 迁移解析会话标题失败: {err}"))?;
        let title = serde_json::from_str::<serde_json::Value>(&metadata_json)
            .ok()
            .and_then(|value| {
                value
                    .get("title")
                    .and_then(|title| title.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default();
        titles.insert(conversation_id, title);
    }
    Ok(titles)
}

/// V3→V4 整体入口：逐会话迁移，单会话失败记录并跳过，系统级错误返回 Err。
/// progress 为可选逐会话进度回调：参数依次为（当前序号（从 1 起）、总数、会话 ID、会话标题、阶段名），
/// 在准备处理每个会话前调用一次。
pub(super) fn migration_v3_to_v4(
    data_path: &PathBuf,
    progress: Option<&dyn Fn(usize, usize, &str, &str, &str)>,
) -> Result<(), String> {
    if migration_v3_is_completed(data_path, MIGRATION_V4_COMPLETED_KEY)? {
        return Ok(());
    }
    let conversation_ids = migration_v4_collect_conversation_ids(data_path)?;
    let titles = migration_v4_collect_conversation_titles(data_path)?;
    let total = conversation_ids.len();
    let mut skipped_count = 0usize;
    for (index, conversation_id) in conversation_ids.iter().enumerate() {
        if let Some(callback) = progress {
            let title = titles.get(conversation_id).map(String::as_str).unwrap_or("");
            callback(index + 1, total, conversation_id, title, "v3_to_v4");
        }
        let migration_key = format!(
            "{}:conversation:{}",
            MIGRATION_V4_COMPLETED_KEY, conversation_id
        );
        if migration_v3_is_completed(data_path, &migration_key)? {
            continue;
        }
        match migration_v3_to_v4_conversation(data_path, conversation_id) {
            Ok(()) => migration_v3_mark_completed(data_path, &migration_key)?,
            Err(err) => {
                skipped_count += 1;
                runtime_log_warn(format!(
                    "[聊天存储迁移] 跳过，任务=V3到V4会话迁移，conversation_id={}，异常={}",
                    conversation_id, err
                ));
            }
        }
    }
    if skipped_count > 0 {
        runtime_log_warn(format!(
            "[聊天存储迁移] 完成，任务=V3到V4逐会话迁移，跳过会话数={}，source=保留原始文件供人工处理或显式重试",
            skipped_count
        ));
    }
    migration_v3_mark_completed(data_path, MIGRATION_V4_COMPLETED_KEY)
}

struct MigrationV4LocatorRow {
    sequence: i64,
    message_id: String,
    block_id: i64,
    byte_offset: i64,
    byte_len: i64,
    compaction_kind: Option<String>,
    role: String,
    created_at: String,
}

/// 读会话全部 locator（按 sequence 排序）
fn migration_v4_read_locators(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Vec<MigrationV4LocatorRow>, String> {
    let mut statement = conn
        .prepare(
            "SELECT sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at
             FROM message_locator WHERE conversation_id=?1 ORDER BY sequence ASC",
        )
        .map_err(|err| format!("V4 迁移准备读取 locator 失败: {err}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok(MigrationV4LocatorRow {
                sequence: row.get(0)?,
                message_id: row.get(1)?,
                block_id: row.get(2)?,
                byte_offset: row.get(3)?,
                byte_len: row.get(4)?,
                compaction_kind: row.get(5)?,
                role: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|err| format!("V4 迁移读取 locator 失败: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("V4 迁移解析 locator 失败: {err}"))
}

/// 解析 V3 明文单行聚合（{"kind":"message","message":{...}}）→ ChatMessage。
/// 自包含实现：不调用生产 jsonl_snapshot 的 decode 函数。
fn migration_v4_parse_v3_message_line(line: &[u8]) -> Result<ChatMessage, String> {
    let line = std::str::from_utf8(line)
        .map_err(|err| format!("V4 迁移解析消息行 UTF-8 失败: {err}"))?
        .trim_end_matches('\n');
    let parsed: serde_json::Value = serde_json::from_str(line)
        .map_err(|err| format!("V4 迁移解析消息 JSON 失败: {err}"))?;
    let kind = parsed
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if kind != "message" {
        return Err(format!("V4 迁移遇到非 message 行类型: {kind}"));
    }
    let message = parsed
        .get("message")
        .ok_or_else(|| "V4 迁移消息行缺少 message 字段".to_string())?;
    serde_json::from_value(message.clone())
        .map_err(|err| format!("V4 迁移反序列化 ChatMessage 失败: {err}"))
}

/// 单会话迁移：读 V3 明文块 → 按 locator 拆多行组 → 压缩写 .jsonl.zstd → 事务更新 sqlite
fn migration_v3_to_v4_conversation(
    data_path: &PathBuf,
    conversation_id: &str,
) -> Result<(), String> {
    let shard_dir = app_layout_chat_conversations_dir(data_path).join(conversation_id);
    let conn = migration_open_v3_database(data_path)?;
    let locators = migration_v4_read_locators(&conn, conversation_id)?;

    // 会话全部块（含无 locator 的空块）
    let blocks = conn
        .prepare(
            "SELECT block_id, block_file FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
        )
        .map_err(|err| format!("V4 迁移准备读取块失败: {err}"))?
        .query_map([conversation_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("V4 迁移读取块失败: {err}"))?
        .collect::<Result<Vec<(i64, String)>, _>>()
        .map_err(|err| format!("V4 迁移解析块失败: {err}"))?;

    let mut block_ids = blocks
        .iter()
        .map(|(block_id, _)| *block_id)
        .collect::<std::collections::BTreeSet<_>>();
    for locator in &locators {
        block_ids.insert(locator.block_id);
    }

    let transaction = conn
        .unchecked_transaction()
        .map_err(|err| format!("V4 迁移开启事务失败: {err}"))?;

    let mut migrated_blocks = Vec::<(i64, String)>::new();

    for block_id in &block_ids {
        let block_file = blocks
            .iter()
            .find(|(id, _)| id == block_id)
            .map(|(_, file)| file.clone())
            .unwrap_or_else(|| format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl"));
        if block_file.ends_with(".jsonl.zstd") {
            continue; // 已是 V4 块（断点续迁场景）
        }
        let block_path = shard_dir.join(&block_file);
        let raw = std::fs::read(&block_path).map_err(|err| {
            format!(
                "V4 迁移读取 V3 明文块失败，conversation_id={}，block={}，error={err}",
                conversation_id, block_file
            )
        })?;

        let block_locators = locators
            .iter()
            .filter(|locator| locator.block_id == *block_id)
            .collect::<Vec<_>>();

        let mut v4_plain = String::new();
        let mut rebuilt_locators = Vec::<(i64, String, i64, i64, Option<String>, String, String)>::new();
        let mut cursor = 0usize;
        let mut previous_end = 0usize;
        for locator in &block_locators {
            let start = locator.byte_offset as usize;
            let end = start + locator.byte_len as usize;
            // V3 locator 区间是 V3 明文块内的字节区间，只校验文件边界与区间有序
            if start < previous_end || end > raw.len() || start >= end {
                return Err(format!(
                    "V4 迁移 locator 越界，conversation_id={}，block={}，message_id={}，offset={}，len={}，file_len={}",
                    conversation_id, block_file, locator.message_id, locator.byte_offset, locator.byte_len, raw.len()
                ));
            }
            let message = migration_v4_parse_v3_message_line(&raw[start..end])?;
            let lines = split_message_into_group_lines(&message)?;
            let group_offset = cursor as i64;
            let mut group_len = 0usize;
            for line in &lines {
                v4_plain.push_str(line);
                group_len += line.len();
            }
            rebuilt_locators.push((
                locator.sequence,
                locator.message_id.clone(),
                group_offset,
                group_len as i64,
                locator.compaction_kind.clone(),
                locator.role.clone(),
                locator.created_at.clone(),
            ));
            cursor += group_len;
            previous_end = end;
        }

        // 压缩为整块单帧并写 .jsonl.zstd
        let compressed = zstd_compress_block(v4_plain.as_bytes())?;
        let v4_block_file = format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd");
        let v4_block_path = shard_dir.join(&v4_block_file);
        if let Some(parent) = v4_block_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!("V4 迁移创建块目录失败，path={}，error={err}", parent.display())
            })?;
        }
        std::fs::write(&v4_block_path, &compressed).map_err(|err| {
            format!(
                "V4 迁移写入压缩块失败，conversation_id={}，block={}，error={err}",
                conversation_id, v4_block_file
            )
        })?;

        // 事务内更新 sqlite
        transaction
            .execute(
                "UPDATE conversation_blocks SET block_file=?1, byte_len=?2 WHERE conversation_id=?3 AND block_id=?4",
                rusqlite::params![
                    v4_block_file,
                    compressed.len() as i64,
                    conversation_id,
                    block_id
                ],
            )
            .map_err(|err| format!("V4 迁移更新块记录失败: {err}"))?;
        transaction
            .execute(
                "DELETE FROM message_locator WHERE conversation_id=?1 AND block_id=?2",
                rusqlite::params![conversation_id, block_id],
            )
            .map_err(|err| format!("V4 迁移清理旧 locator 失败: {err}"))?;
        for (sequence, message_id, byte_offset, byte_len, compaction_kind, role, created_at) in
            rebuilt_locators
        {
            transaction
                .execute(
                    "INSERT INTO message_locator(conversation_id, sequence, message_id, block_id, byte_offset, byte_len, compaction_kind, role, created_at)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        conversation_id,
                        sequence,
                        message_id,
                        block_id,
                        byte_offset,
                        byte_len,
                        compaction_kind,
                        role,
                        created_at
                    ],
                )
                .map_err(|err| format!("V4 迁移写入新 locator 失败: {err}"))?;
        }
        migrated_blocks.push((*block_id, block_file));
    }

    transaction
        .commit()
        .map_err(|err| format!("V4 迁移提交事务失败: {err}"))?;

    // 事务提交成功后删除旧明文块（迁移后无明文残留，D13）
    for (_, old_block_file) in &migrated_blocks {
        let old_path = shard_dir.join(old_block_file);
        if old_path.exists() {
            let _ = std::fs::remove_file(&old_path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod migration_v4_tests {
    use super::*;

    fn tool_event(name: &str, tag: &str) -> Value {
        serde_json::json!({
            "type": "tool_call",
            "name": name,
            "tag": tag,
        })
    }

    fn text_message(id: &str, role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-08-22T00:00:0{}Z", id.len() % 10),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: vec!["block".to_string()],
            provider_meta: Some(serde_json::json!({"model": "gpt-4"})),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn tool_message(id: &str, events: Vec<Value>) -> ChatMessage {
        let mut message = text_message(id, "assistant", "final answer");
        message.tool_call = Some(events);
        message
    }

    fn test_conversation(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-a".to_string(),
            title: "会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            is_draft: false,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
        }
    }

    /// 自包含读取 V4 会话（不依赖生产读写路径）：sqlite locator + 解压 zstd 块 + 按组组装
    fn read_v4_conversation_messages(
        data_path: &PathBuf,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let conn = migration_open_v3_database(data_path)?;
        let locators = migration_v4_read_locators(&conn, conversation_id)?;
        let shard_dir = app_layout_chat_conversations_dir(data_path).join(conversation_id);
        let blocks = conn
            .prepare(
                "SELECT block_id, block_file FROM conversation_blocks WHERE conversation_id=?1 ORDER BY block_id ASC",
            )
            .map_err(|err| format!("测试读取块记录失败: {err}"))?
            .query_map([conversation_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| format!("测试读取块失败: {err}"))?
            .collect::<Result<Vec<(i64, String)>, _>>()
            .map_err(|err| format!("测试解析块失败: {err}"))?;
        let mut plain_by_block = std::collections::BTreeMap::<i64, Vec<u8>>::new();
        for (block_id, block_file) in blocks {
            let raw = std::fs::read(shard_dir.join(&block_file))
                .map_err(|err| format!("测试读取 V4 块失败: {err}"))?;
            let plain = zstd_decompress_block(&raw)?;
            plain_by_block.insert(block_id, plain);
        }
        let mut messages = Vec::<ChatMessage>::new();
        for locator in locators {
            let plain = plain_by_block
                .get(&locator.block_id)
                .ok_or_else(|| format!("测试缺少块 {}", locator.block_id))?;
            let start = locator.byte_offset as usize;
            let end = start + locator.byte_len as usize;
            let slice = plain
                .get(start..end)
                .ok_or_else(|| {
                    format!(
                        "测试 locator 越界，block_id={}，start={start}，end={end}，plain_len={}",
                        locator.block_id,
                        plain.len()
                    )
                })?;
            let text = std::str::from_utf8(slice)
                .map_err(|err| format!("测试 V4 明文 UTF-8 失败: {err}"))?;
            let lines = text
                .lines()
                .map(|line| format!("{line}\n"))
                .collect::<Vec<_>>();
            messages.push(assemble_group_message(&lines)?);
        }
        Ok(messages)
    }

    fn messages_json(messages: &[ChatMessage]) -> Value {
        serde_json::to_value(messages).expect("序列化消息")
    }

    #[test]
    fn migration_v3_to_v4_should_preserve_all_message_fields() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-migration-v3-v4-equivalence-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            text_message("m1", "user", "hello"),
            tool_message(
                "m2",
                vec![
                    tool_event("weather", "call-1"),
                    tool_event("weather", "result-1"),
                    tool_event("flight", "call-2"),
                    tool_event("flight", "result-2"),
                ],
            ),
            text_message("m3", "assistant", "plain answer"),
        ]);
        migration_v1_to_v2_conversation(&paths, &conversation, false).expect("seed v2");
        migration_v2_to_v3(&data_path, None).expect("v2 to v3");
        migration_v3_to_v4(&data_path, None).expect("v3 to v4");

        let stored = read_v4_conversation_messages(&data_path, "conversation-a")
            .expect("read v4 messages");
        assert_eq!(
            messages_json(&stored),
            messages_json(&conversation.messages),
            "迁移后消息逐字段等价"
        );
        // 明文块已删除，压缩块保留
        assert!(!paths.blocks_dir.join("000000.jsonl").exists());
        assert!(paths.blocks_dir.join("000000.jsonl.zstd").exists());
        // 全局迁移 key 已标记
        assert!(migration_v3_is_completed(&data_path, MIGRATION_V4_COMPLETED_KEY)
            .expect("global key"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_v3_to_v4_should_rebuild_locators_across_group_lines() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-migration-v3-v4-locator-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![
            tool_message(
                "m1",
                vec![
                    tool_event("weather", "call-1"),
                    tool_event("weather", "result-1"),
                ],
            ),
            text_message("m2", "user", "follow up"),
        ]);
        migration_v1_to_v2_conversation(&paths, &conversation, false).expect("seed v2");
        migration_v2_to_v3(&data_path, None).expect("v2 to v3");
        migration_v3_to_v4(&data_path, None).expect("v3 to v4");

        let conn = migration_open_v3_database(&data_path).expect("open db");
        let locators = migration_v4_read_locators(&conn, "conversation-a").expect("locators");
        assert_eq!(locators.len(), 2);
        // locator 字节区间连续且互不重叠，覆盖整个明文
        let mut cursor = 0i64;
        for locator in &locators {
            assert_eq!(locator.byte_offset, cursor, "offset 连续");
            assert!(locator.byte_len > 0, "byte_len 非零");
            cursor = locator.byte_offset + locator.byte_len;
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_v3_to_v4_should_be_idempotent_on_retry() {
        let root = std::env::temp_dir().join(format!(
            "easy-call-migration-v3-v4-idempotent-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("config_mark");
        let paths = message_store_paths(&data_path, "conversation-a").expect("paths");
        let conversation = test_conversation(vec![text_message("m1", "user", "hello")]);
        migration_v1_to_v2_conversation(&paths, &conversation, false).expect("seed v2");
        migration_v2_to_v3(&data_path, None).expect("v2 to v3");
        migration_v3_to_v4(&data_path, None).expect("first v3 to v4");

        let block_before =
            fs::read(paths.blocks_dir.join("000000.jsonl.zstd")).expect("v4 block");
        migration_v3_to_v4(&data_path, None).expect("second v3 to v4");
        let block_after =
            fs::read(paths.blocks_dir.join("000000.jsonl.zstd")).expect("v4 block after");
        assert_eq!(block_before, block_after, "重跑不重写块");

        let stored = read_v4_conversation_messages(&data_path, "conversation-a")
            .expect("read v4 messages");
        assert_eq!(messages_json(&stored), messages_json(&conversation.messages));
        let _ = fs::remove_dir_all(root);
    }
}