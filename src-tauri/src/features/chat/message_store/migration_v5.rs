// ========== 会话元数据 V5：骨肉分离（基石字段搬进 SQL 列，metadata_json 只留业务载荷）==========
//
// 自包含迁移快照：不引用 Conversation / ConversationShardMeta 等业务结构体，也不调业务层
// 任何函数（含路径布局、归一化、可见性判定）。只看磁盘上原样的 metadata_json 文本与
// 弱类型 serde_json::Value，就地清洗后回写。
//
// 版本通道：PRAGMA user_version（0 -> 1）。与 chat_storage_migrations 的 migration_key
// 互不干扰——那份管「消息正文怎么落盘」，这份管「库内元数据怎么分骨肉」。
//
// 铁律（见 .pai/plan/core/20261124_会话元数据骨肉分离与结构化迁移计划.md）：
// - 结构升级、读旧 JSON、提取字段、瘦身、写回、建索引、推进版本，全在同一个
//   IMMEDIATE 事务里完成，不做锁外三段式；
// - 坏条只记 warn、给列赋安全默认值、版本照常推进，绝不抛错阻断启动，也不设重试/deferred；
// - 提取完立刻从 JSON 对象里 remove 掉旧键，避免「同一份数据两个真相」；
// - 不发短号：短号序列住在 state.sqlite（另一个库），跨库拿不到同一把锁，
//   short_id 只搬 JSON 里已有的值，缺号的由启动期 state 侧按列幂等补齐。

/// V5 完成后的 `PRAGMA user_version`。
const CHAT_METADATA_V5_USER_VERSION: i64 = 1;

/// V5 迁移入口：返回是否真的做了迁移。
///
/// 已迁移的库只花一次 `PRAGMA user_version` 查询就返回 `Ok(false)`，重复调用无副作用。
pub(super) fn migrate_chat_metadata_v5(
    conn: &mut rusqlite::Connection,
) -> Result<bool, String> {
    let started = std::time::Instant::now();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|err| format!("读取会话元数据 user_version 失败: {err}"))?;
    if version >= CHAT_METADATA_V5_USER_VERSION {
        return Ok(false);
    }

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|err| format!("开启会话元数据 V5 迁移事务失败: {err}"))?;

    let has_table = migration_v5_has_metadata_table(&tx)?;
    if !has_table {
        // 新库还没建表：没有要搬的东西，照样推进版本，下次不再检查。
        tx.execute_batch(&format!(
            "PRAGMA user_version = {CHAT_METADATA_V5_USER_VERSION};"
        ))
        .map_err(|err| format!("推进会话元数据 V5 版本失败: {err}"))?;
        tx.commit()
            .map_err(|err| format!("提交会话元数据 V5 迁移失败: {err}"))?;
        return Ok(true);
    }

    chat_metadata_ensure_backbone_columns(&tx)?;
    let rows = migration_v5_extract_backbone(&tx)?;
    chat_metadata_ensure_backbone_indexes(&tx)?;

    tx.execute_batch(&format!(
        "PRAGMA user_version = {CHAT_METADATA_V5_USER_VERSION};"
    ))
    .map_err(|err| format!("推进会话元数据 V5 版本失败: {err}"))?;
    tx.commit()
        .map_err(|err| format!("提交会话元数据 V5 迁移失败: {err}"))?;

    runtime_log_info(format!(
        "[会话元数据V5] 完成 任务=骨肉分离 行数={} 耗时={}ms",
        rows,
        started.elapsed().as_millis()
    ));
    Ok(true)
}

fn migration_v5_has_metadata_table(conn: &rusqlite::Connection) -> Result<bool, String> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='conversation_metadata'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("检查会话元数据表是否存在失败: {err}"))?;
    Ok(exists > 0)
}


/// 逐行无模式清洗：提取底座字段 -> 从 JSON 里删掉旧键 -> 回写列与瘦身 JSON。
///
/// 读、改、写全程在调用方给的同一事务内完成。
fn migration_v5_extract_backbone(conn: &rusqlite::Connection) -> Result<usize, String> {
    let rows = {
        let mut statement = conn
            .prepare("SELECT conversation_id, metadata_json FROM conversation_metadata")
            .map_err(|err| format!("V5 迁移准备读取会话元数据失败: {err}"))?;
        let mapped = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| format!("V5 迁移读取会话元数据失败: {err}"))?;
        mapped
            .collect::<Result<Vec<(String, String)>, _>>()
            .map_err(|err| format!("V5 迁移解析会话元数据行失败: {err}"))?
    };

    for (conversation_id, raw) in &rows {
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(mut value) => {
                let backbone = migration_v5_take_backbone(&mut value);
                let slim_json = serde_json::to_string(&value).unwrap_or_else(|err| {
                    runtime_log_warn(format!(
                        "[会话元数据V5] 降级 任务=序列化瘦身JSON 会话={conversation_id} 原因={err}"
                    ));
                    raw.clone()
                });
                conn.execute(
                    "UPDATE conversation_metadata
                        SET short_id=CASE WHEN short_id IS NULL THEN ?1 ELSE short_id END,
                            created_at=?2, status=?3, conversation_kind=?4,
                            agent_id=?5, parent_conversation_id=?6,
                            root_conversation_id=?7, delegate_id=?8, metadata_json=?9
                      WHERE conversation_id=?10",
                    rusqlite::params![
                        backbone.short_id,
                        backbone.created_at,
                        backbone.status,
                        backbone.conversation_kind,
                        backbone.agent_id,
                        backbone.parent_conversation_id,
                        backbone.root_conversation_id,
                        backbone.delegate_id,
                        slim_json,
                        conversation_id,
                    ],
                )
                .map_err(|err| {
                    format!("V5 迁移写回会话元数据失败，会话={conversation_id}，error={err}")
                })?;
            }
            Err(err) => {
                // 坏条：迁移对它无能为力，记一条 warn，给底座列安全默认值，JSON 原样不动。
                runtime_log_warn(format!(
                    "[会话元数据V5] 跳过 任务=解析metadata_json 会话={conversation_id} 原因={err}"
                ));
                conn.execute(
                    "UPDATE conversation_metadata
                        SET created_at=CASE WHEN created_at='' THEN ?1 ELSE created_at END,
                            status=CASE WHEN status='' THEN 'active' ELSE status END,
                            conversation_kind=CASE WHEN conversation_kind='' THEN 'regular' ELSE conversation_kind END,
                            agent_id=COALESCE(agent_id, '')
                      WHERE conversation_id=?2",
                    rusqlite::params![migration_v5_now_iso(), conversation_id],
                )
                .map_err(|err| {
                    format!("V5 迁移兜底写回会话元数据失败，会话={conversation_id}，error={err}")
                })?;
            }
        }
    }
    Ok(rows.len())
}

/// 底座字段的一次性搬运行李，仅在迁移模块内活着。
struct MigrationV5Backbone {
    short_id: Option<i64>,
    created_at: String,
    status: String,
    conversation_kind: String,
    agent_id: String,
    parent_conversation_id: Option<String>,
    root_conversation_id: Option<String>,
    delegate_id: Option<String>,
}

/// 从 JSON 对象里取出底座字段，并把这些键从对象里彻底删掉（物理瘦身）。
fn migration_v5_take_backbone(value: &mut serde_json::Value) -> MigrationV5Backbone {
    let short_id = value
        .get("shortId")
        .and_then(serde_json::Value::as_u64)
        .and_then(|item| i64::try_from(item).ok());
    let created_at = migration_v5_take_text(value, "createdAt").unwrap_or_default();
    let status = migration_v5_take_text(value, "status")
        .filter(|item| !item.trim().is_empty())
        .unwrap_or_else(|| "active".to_string());
    let conversation_kind = migration_v5_take_text(value, "conversationKind")
        .filter(|item| !item.trim().is_empty())
        .unwrap_or_else(|| "regular".to_string());
    let agent_id = migration_v5_take_text(value, "agentId").unwrap_or_default();
    let parent_conversation_id = migration_v5_take_text(value, "parentConversationId");
    let root_conversation_id = migration_v5_take_text(value, "rootConversationId");
    let delegate_id = migration_v5_take_text(value, "delegateId");
    if let Some(object) = value.as_object_mut() {
        object.remove("shortId");
    }
    MigrationV5Backbone {
        short_id,
        created_at,
        status,
        conversation_kind,
        agent_id,
        parent_conversation_id,
        root_conversation_id,
        delegate_id,
    }
}

/// 取一个字符串键并从对象里删掉它。
fn migration_v5_take_text(value: &mut serde_json::Value, key: &str) -> Option<String> {
    let object = value.as_object_mut()?;
    match object.remove(key) {
        Some(serde_json::Value::String(text)) => Some(text),
        Some(serde_json::Value::Null) => None,
        Some(other) => other.as_str().map(str::to_string),
        None => None,
    }
}

/// 迁移自带的 RFC3339 时间快照（不调业务时间函数，格式与项目 now_iso 一致）。
fn migration_v5_now_iso() -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

// ========== 业务侧接触点 ==========
//
// 唯一的调用点在 sqlite.rs 的 `chat_metadata_store_open` 里（一行）。业务读写路径不含
// 任何旧 JSON 键名、不含旧结构体，所以「注掉本文件 + 那一行调用」后全部业务代码照常
// 编译通过——这是计划里那条机械验收的实际执行方式。

#[cfg(test)]
mod migration_v5_tests {
    use super::*;

    /// 沙盒：在系统临时目录手工造一个「V5 之前」的库（无底座列、user_version=0）。
    fn migration_v5_test_db(tag: &str) -> (std::path::PathBuf, rusqlite::Connection) {
        let root = std::env::temp_dir().join(format!("eca-meta-v5-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let db_path = root.join("chat_metadata.sqlite");
        let conn = rusqlite::Connection::open(&db_path).expect("open legacy db");
        conn.execute_batch(
            "CREATE TABLE conversation_metadata (
               conversation_id TEXT PRIMARY KEY,
               metadata_json TEXT NOT NULL,
               storage_revision INTEGER NOT NULL DEFAULT 0,
               updated_at TEXT NOT NULL
             );",
        )
        .expect("create legacy table");
        (root, conn)
    }

    fn insert_legacy(conn: &rusqlite::Connection, conversation_id: &str, metadata: &serde_json::Value) {
        conn.execute(
            "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at)
             VALUES(?1, ?2, 0, ?3)",
            rusqlite::params![
                conversation_id,
                serde_json::to_string(metadata).expect("serialize legacy meta"),
                "2024-01-02T03:04:05Z"
            ],
        )
        .expect("insert legacy meta");
    }

    fn metadata_json_of(conn: &rusqlite::Connection, conversation_id: &str) -> String {
        conn.query_row(
            "SELECT metadata_json FROM conversation_metadata WHERE conversation_id=?1",
            rusqlite::params![conversation_id],
            |row| row.get(0),
        )
        .expect("read metadata json")
    }

    fn user_version(conn: &rusqlite::Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user_version")
    }

    /// 全字段提取 + JSON 物理瘦身 + 业务载荷原样保留。
    #[test]
    fn migration_v5_should_lift_backbone_columns_and_slim_json() {
        let (_root, mut conn) = migration_v5_test_db("lift");
        let legacy = serde_json::json!({
            "id": "conv-a",
            "title": "旧标题",
            "agentId": "agent-1",
            "status": "archived",
            "conversationKind": "delegate",
            "createdAt": "2024-01-02T03:04:05Z",
            "parentConversationId": "conv-parent",
            "rootConversationId": "conv-root",
            "delegateId": "delegate-9",
            "shortId": 42,
            "unreadCount": 3,
            "currentTodos": [{"id": "t1", "text": "待办", "done": false}]
        });
        insert_legacy(&conn, "conv-a", &legacy);

        assert!(migrate_chat_metadata_v5(&mut conn).expect("migrate v5"));

        let row: (
            Option<i64>,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT short_id, created_at, status, conversation_kind, agent_id,
                        parent_conversation_id, root_conversation_id, delegate_id
                   FROM conversation_metadata WHERE conversation_id='conv-a'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .expect("read backbone columns");
        assert_eq!(row.0, Some(42), "shortId 应落到 short_id 列");
        assert_eq!(row.1, "2024-01-02T03:04:05Z");
        assert_eq!(row.2, "archived");
        assert_eq!(row.3, "delegate");
        assert_eq!(row.4, "agent-1");
        assert_eq!(row.5.as_deref(), Some("conv-parent"));
        assert_eq!(row.6.as_deref(), Some("conv-root"));
        assert_eq!(row.7.as_deref(), Some("delegate-9"));

        let slim = metadata_json_of(&conn, "conv-a");
        let value: serde_json::Value = serde_json::from_str(&slim).expect("parse slim json");
        let object = value.as_object().expect("slim json object");
        for key in [
            "shortId",
            "createdAt",
            "status",
            "conversationKind",
            "agentId",
            "parentConversationId",
            "rootConversationId",
            "delegateId",
        ] {
            assert!(!object.contains_key(key), "瘦身后 JSON 不该再有 {key}：{slim}");
        }
        assert_eq!(value["title"], "旧标题", "标题留在 JSON，不建列也不搬走");
        assert_eq!(value["unreadCount"], 3, "业务载荷必须原样保留");
        assert_eq!(value["currentTodos"][0]["text"], "待办");
        assert_eq!(user_version(&conn), CHAT_METADATA_V5_USER_VERSION);
    }

    /// 坏条：记 warn、列给安全默认值、JSON 原样不动、版本照常推进、不返回 Err。
    #[test]
    fn migration_v5_should_tolerate_broken_json_and_still_advance_version() {
        let (_root, mut conn) = migration_v5_test_db("broken");
        conn.execute(
            "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at)
             VALUES('conv-broken', '{not json', 0, '2024-01-02T03:04:05Z')",
            [],
        )
        .expect("insert broken meta");

        assert!(
            migrate_chat_metadata_v5(&mut conn).expect("坏条不得让迁移失败"),
            "坏条也要照常推进版本"
        );

        assert_eq!(
            metadata_json_of(&conn, "conv-broken"),
            "{not json",
            "坏条 JSON 原样保留，迁移不猜内容"
        );
        let (status, conversation_kind, created_at, agent_id): (String, String, String, String) =
            conn.query_row(
                "SELECT status, conversation_kind, created_at, agent_id
                   FROM conversation_metadata WHERE conversation_id='conv-broken'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read default columns");
        assert_eq!(status, "active");
        assert_eq!(conversation_kind, "regular");
        assert!(created_at.ends_with('Z'), "坏条 created_at 应填当前时间：{created_at}");
        assert_eq!(agent_id, "");
        assert_eq!(user_version(&conn), CHAT_METADATA_V5_USER_VERSION);
    }

    /// 幂等：已迁移的库再跑一次直接跳过，数据一个字节都不动。
    #[test]
    fn migration_v5_should_be_idempotent() {
        let (_root, mut conn) = migration_v5_test_db("idempotent");
        let legacy = serde_json::json!({
            "id": "conv-b",
            "title": "标题",
            "status": "active",
            "agentId": "agent-2",
            "createdAt": "2024-05-06T07:08:09Z"
        });
        insert_legacy(&conn, "conv-b", &legacy);
        assert!(migrate_chat_metadata_v5(&mut conn).expect("first run"));

        let slim_first = metadata_json_of(&conn, "conv-b");

        assert!(
            !migrate_chat_metadata_v5(&mut conn).expect("second run"),
            "第二次必须是 0ms 跳过"
        );
        assert_eq!(metadata_json_of(&conn, "conv-b"), slim_first);
        let value: serde_json::Value = serde_json::from_str(&slim_first).expect("parse slim json");
        assert_eq!(value["title"], "标题", "标题留在 JSON，两次迁移后原样不动");
        assert_eq!(user_version(&conn), CHAT_METADATA_V5_USER_VERSION);
    }

    /// P0 回归：列里已经写好短号、JSON 里没有 `shortId` 时，迁移不得把列抹成 NULL。
    #[test]
    fn migration_v5_should_keep_existing_short_id_column() {
        let (_root, mut conn) = migration_v5_test_db("keep-short-id");
        conn.execute_batch("ALTER TABLE conversation_metadata ADD COLUMN short_id INTEGER;")
            .expect("add short_id column");
        let legacy = serde_json::json!({
            "id": "conv-keep",
            "title": "列里已有号",
            "status": "active",
            "createdAt": "2024-02-03T04:05:06Z"
        });
        insert_legacy(&conn, "conv-keep", &legacy);
        conn.execute(
            "UPDATE conversation_metadata SET short_id=7 WHERE conversation_id='conv-keep'",
            [],
        )
        .expect("seed short_id column");

        assert!(migrate_chat_metadata_v5(&mut conn).expect("migrate v5"));

        let short_id: Option<i64> = conn
            .query_row(
                "SELECT short_id FROM conversation_metadata WHERE conversation_id='conv-keep'",
                [],
                |row| row.get(0),
            )
            .expect("read short_id column");
        assert_eq!(
            short_id,
            Some(7),
            "列里已有的短号是权威，JSON 缺号也不许覆写成 NULL"
        );
        let created_at: String = conn
            .query_row(
                "SELECT created_at FROM conversation_metadata WHERE conversation_id='conv-keep'",
                [],
                |row| row.get(0),
            )
            .expect("read created_at column");
        assert_eq!(created_at, "2024-02-03T04:05:06Z");
    }

    /// 老库已经有部分底座列时不报错，缺的补上，多余的不动。
    #[test]
    fn migration_v5_should_tolerate_partially_upgraded_schema() {
        let (_root, mut conn) = migration_v5_test_db("partial");
        conn.execute_batch("ALTER TABLE conversation_metadata ADD COLUMN short_id INTEGER;")
            .expect("add short_id column");
        let legacy = serde_json::json!({
            "id": "conv-c",
            "title": "部分升级",
            "status": "active"
        });
        insert_legacy(&conn, "conv-c", &legacy);

        assert!(migrate_chat_metadata_v5(&mut conn).expect("migrate v5"));
        let short_id: Option<i64> = conn
            .query_row(
                "SELECT short_id FROM conversation_metadata WHERE conversation_id='conv-c'",
                [],
                |row| row.get(0),
            )
            .expect("read short_id column");
        assert_eq!(short_id, None, "老库没写短号就留空，由 state 侧补号");
        let raw = metadata_json_of(&conn, "conv-c");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("parse json");
        assert_eq!(value["title"], "部分升级", "标题不从 JSON 搬走");
    }
}
