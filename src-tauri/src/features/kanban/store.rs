// ==================== 队伍议题看板：存储层 ====================
// 独立库 kanban/kanban_store.db，对齐 task/ delegate/ 的分域做法。
// 编号单调递增不回收：靠 kanban_next_number 单独维护，不靠 MAX(number)+1
// ——删掉最大编号后 MAX+1 会把那个号重新发出去。

fn kanban_store_db_path(data_path: &PathBuf) -> PathBuf {
    app_root_from_data_path(data_path)
        .join("kanban")
        .join(KANBAN_DB_FILE_NAME)
}

/// 测试观测点：按数据目录统计看板库打开次数。
/// 用于验证「一次批量概览读取只在整批上开一次库」，不参与运行时逻辑。
#[cfg(test)]
fn kanban_store_open_counters() -> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, usize>>
{
    static COUNTERS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, usize>>,
    > = std::sync::OnceLock::new();
    COUNTERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[cfg(test)]
pub(crate) fn kanban_store_reset_open_count(data_path: &PathBuf) {
    if let Ok(mut counters) = kanban_store_open_counters().lock() {
        counters.insert(data_path.clone(), 0);
    }
}

#[cfg(test)]
pub(crate) fn kanban_store_open_count(data_path: &PathBuf) -> usize {    kanban_store_open_counters()
        .lock()
        .ok()
        .and_then(|counters| counters.get(data_path).copied())
        .unwrap_or(0)
}

/// 议题是否存在。
///
/// 只回布尔：给队伍派活那条硬约束用，不把看板内部的议题结构暴露到模块外。
/// 连接口径与看板工具一致，都在这里开一次，调用方不要自己另开连接。
pub(crate) fn kanban_store_issue_exists(
    data_path: &PathBuf,
    leader_conversation_id: &str,
    number: u32,
) -> Result<bool, String> {
    let conn = kanban_store_open(data_path)?;
    Ok(kanban_store_find(&conn, leader_conversation_id, number)?.is_some())
}

fn kanban_store_open(data_path: &PathBuf) -> Result<Connection, String> {
    #[cfg(test)]
    {
        if let Ok(mut counters) = kanban_store_open_counters().lock() {
            *counters.entry(data_path.clone()).or_insert(0) += 1;
        }
    }
    let path = kanban_store_db_path(data_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Kanban db path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|err| format!("创建看板目录失败：{err}"))?;
    let conn = Connection::open(&path)
        .map_err(|err| format!("打开看板库失败（{}）：{err}", path.display()))?;
    kanban_store_init(&conn)?;
    Ok(conn)
}

fn kanban_store_init(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS kanban_issue (
            id TEXT PRIMARY KEY,
            leader_conversation_id TEXT NOT NULL,
            number INTEGER NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            assignee_kind TEXT NOT NULL DEFAULT 'none',
            assignee_member_role TEXT,
            -- 编号时代的归属列：只为一次性迁移保留，新写入一律留 NULL。
            assignee_member_number INTEGER,
            note TEXT NOT NULL DEFAULT '',
            -- 挂在这条议题上的计划文件路径（磁盘绝对路径），空串表示没挂。
            plan_path TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 0
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_kanban_issue_number
            ON kanban_issue(leader_conversation_id, number);
        CREATE INDEX IF NOT EXISTS idx_kanban_issue_status
            ON kanban_issue(leader_conversation_id, status);
        CREATE TABLE IF NOT EXISTS kanban_next_number (
            leader_conversation_id TEXT PRIMARY KEY,
            next_number INTEGER NOT NULL
        );
        COMMIT;",
    )
    .map_err(|err| format!("初始化看板库失败：{err}"))?;
    // 老库补列。看板表最初存的是队员编号（`assignee_member_number`），
    // 后来按角色认人；再后来加计划文件路径。升级只能加列，不能要求用户重建库。
    kanban_store_ensure_column(
        conn,
        "assignee_member_role",
        "ALTER TABLE kanban_issue ADD COLUMN assignee_member_role TEXT",
    )?;
    kanban_store_ensure_column(
        conn,
        "plan_path",
        "ALTER TABLE kanban_issue ADD COLUMN plan_path TEXT NOT NULL DEFAULT ''",
    )?;
    // 「放弃」态已取消，历史 closed 归到 done：留着它既不在任何一格里显示，
    // 也没法通过工具改回来，只能算一条读不出也改不掉的行。
    kanban_store_retire_closed_status(conn)
}

/// 关闭态退场：老库里的 closed 一律按已完成收口，幂等（跑第二遍命中 0 行）。
/// 不动 updated_at：这是状态口径收敛，不是用户改动，不该让老议题的「最近动过」跳到今天。
fn kanban_store_retire_closed_status(conn: &Connection) -> Result<(), String> {
    let affected = conn
        .execute(
            "UPDATE kanban_issue SET status = 'done', version = version + 1 WHERE status = 'closed'",
            [],
        )
        .map_err(|err| format!("迁移看板已关闭状态失败：{err}"))?;
    if affected > 0 {
        runtime_log_info(format!(
            "[看板] 完成，任务=历史已关闭状态归到已完成，migrated={affected}"
        ));
    }
    Ok(())
}

/// 补一列：列已存在就跳过，幂等。
/// 编号列故意留着：一次性归属迁移要读它反查角色，迁移完也不再改写它。
fn kanban_store_ensure_column(conn: &Connection, column: &str, ddl: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(kanban_issue)")
        .map_err(|err| format!("读取看板表结构失败：{err}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>("name"))
        .map_err(|err| format!("读取看板表结构失败：{err}"))?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(|err| format!("读取看板表结构失败：{err}"))?;
    if columns.iter().any(|name| name == column) {
        return Ok(());
    }
    conn.execute(ddl, [])
        .map_err(|err| format!("看板库升级失败：{err}"))?;
    Ok(())
}

fn kanban_issue_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KanbanIssue> {
    Ok(KanbanIssue {
        id: row.get("id")?,
        leader_conversation_id: row.get("leader_conversation_id")?,
        number: row.get::<_, i64>("number")? as u32,
        title: row.get("title")?,
        status: row.get("status")?,
        assignee_kind: row.get("assignee_kind")?,
        assignee_member_role: row.get("assignee_member_role")?,
        note: row.get("note")?,
        plan_path: row.get("plan_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        version: row.get::<_, i64>("version")? as u64,
    })
}

/// 列表按编号升序返回；status / assignee_kind / assignee_role 传 None 表示该维度不过滤。
/// 过滤条件写成 `?x IS NULL OR 列 = ?x`，三种组合共用一套参数绑定。
fn kanban_store_list(
    conn: &Connection,
    leader_conversation_id: &str,
    status: Option<&str>,
    assignee_kind: Option<&str>,
    assignee_role: Option<&str>,
) -> Result<Vec<KanbanIssue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, leader_conversation_id, number, title, status, assignee_kind,
                    assignee_member_role, note, plan_path, created_at, updated_at, version
             FROM kanban_issue
             WHERE leader_conversation_id = ?1
               AND (?2 IS NULL OR status = ?2)
               AND (?3 IS NULL OR assignee_kind = ?3)
               AND (?4 IS NULL OR assignee_member_role = ?4)
             ORDER BY number ASC",
        )
        .map_err(|err| format!("查询看板议题失败：{err}"))?;
    let mut rows = stmt
        .query(params![leader_conversation_id, status, assignee_kind, assignee_role])
        .map_err(|err| format!("查询看板议题失败：{err}"))?;
    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|err| format!("读取看板议题失败：{err}"))?
    {
        out.push(
            kanban_issue_from_row(row).map_err(|err| format!("读取看板议题失败：{err}"))?,
        );
    }
    Ok(out)
}

fn kanban_store_find(
    conn: &Connection,
    leader_conversation_id: &str,
    number: u32,
) -> Result<Option<KanbanIssue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, leader_conversation_id, number, title, status, assignee_kind,
                    assignee_member_role, note, plan_path, created_at, updated_at, version
             FROM kanban_issue WHERE leader_conversation_id = ?1 AND number = ?2",
        )
        .map_err(|err| format!("查询看板议题失败：{err}"))?;
    let mut rows = stmt
        .query(params![leader_conversation_id, number as i64])
        .map_err(|err| format!("查询看板议题失败：{err}"))?;
    match rows
        .next()
        .map_err(|err| format!("读取看板议题失败：{err}"))?
    {
        Some(row) => kanban_issue_from_row(row)
            .map(Some)
            .map_err(|err| format!("读取看板议题失败：{err}")),
        None => Ok(None),
    }
}

fn kanban_store_max_number(
    conn: &Connection,
    leader_conversation_id: &str,
) -> Result<u32, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(number), 0) FROM kanban_issue WHERE leader_conversation_id = ?1",
        params![leader_conversation_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value as u32)
    .map_err(|err| format!("读取看板最大编号失败：{err}"))
}

/// 分配下一个编号：从 kanban_next_number 取，取不到就以「当前最大 + 1」初始化。
/// 分配即自增，删除不归还，因此编号单调递增、不回收。
fn kanban_store_allocate_number(
    conn: &Connection,
    leader_conversation_id: &str,
) -> Result<u32, String> {
    let stored: Option<u32> = conn
        .query_row(
            "SELECT next_number FROM kanban_next_number WHERE leader_conversation_id = ?1",
            params![leader_conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| format!("读取看板编号计数失败：{err}"))?
        .map(|value| value.max(1) as u32);

    let next = match stored {
        Some(value) => value,
        None => kanban_store_max_number(conn, leader_conversation_id)? + 1,
    };

    conn.execute(
        "INSERT INTO kanban_next_number (leader_conversation_id, next_number)
         VALUES (?1, ?2)
         ON CONFLICT(leader_conversation_id) DO UPDATE SET next_number = ?2",
        params![leader_conversation_id, next as i64 + 1],
    )
    .map_err(|err| format!("更新看板编号计数失败：{err}"))?;
    Ok(next)
}

fn kanban_store_insert(conn: &Connection, issue: &KanbanIssue) -> Result<(), String> {
    conn.execute(
        "INSERT INTO kanban_issue (
            id, leader_conversation_id, number, title, status, assignee_kind,
            assignee_member_role, note, plan_path, created_at, updated_at, version
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            issue.id,
            issue.leader_conversation_id,
            issue.number as i64,
            issue.title,
            issue.status,
            issue.assignee_kind,
            issue.assignee_member_role,
            issue.note,
            issue.plan_path,
            issue.created_at,
            issue.updated_at,
            issue.version as i64,
        ],
    )
    .map_err(|err| format!("新建看板议题失败：{err}"))?;
    Ok(())
}

/// 覆盖更新。命中返回 true；版本不符或已不存在返回 false。
fn kanban_store_update(
    conn: &Connection,
    issue: &KanbanIssue,
    expected_version: Option<u64>,
) -> Result<bool, String> {
    let affected = match expected_version {
        Some(version) => conn.execute(
            "UPDATE kanban_issue
             SET title = ?3, status = ?4, assignee_kind = ?5, assignee_member_role = ?6,
                 note = ?7, plan_path = ?8, updated_at = ?9, version = version + 1
             WHERE leader_conversation_id = ?1 AND number = ?2 AND version = ?10",
            params![
                issue.leader_conversation_id,
                issue.number as i64,
                issue.title,
                issue.status,
                issue.assignee_kind,
                issue.assignee_member_role,
                issue.note,
                issue.plan_path,
                issue.updated_at,
                version as i64,
            ],
        ),
        None => conn.execute(
            "UPDATE kanban_issue
             SET title = ?3, status = ?4, assignee_kind = ?5, assignee_member_role = ?6,
                 note = ?7, plan_path = ?8, updated_at = ?9, version = version + 1
             WHERE leader_conversation_id = ?1 AND number = ?2",
            params![
                issue.leader_conversation_id,
                issue.number as i64,
                issue.title,
                issue.status,
                issue.assignee_kind,
                issue.assignee_member_role,
                issue.note,
                issue.plan_path,
                issue.updated_at,
            ],
        ),
    }
    .map_err(|err| format!("更新看板议题失败：{err}"))?;
    Ok(affected > 0)
}

/// 条件更新：只在当前状态等于 from_status 时推进到 to_status。
/// 用户点通过与队长工具调用撞车时，靠它保证「先到先得」，后到的一侧拿到 0 行。
/// 条件更新状态。`clear_assignee` 为真时把负责人一并清成 none：
/// 已完成不归任何人办，把这件事放在存储层做，调用方就没机会漏掉。
fn kanban_store_transition(
    conn: &Connection,
    leader_conversation_id: &str,
    number: u32,
    from_status: &str,
    to_status: &str,
    note: &str,
    clear_assignee: bool,
    updated_at: &str,
) -> Result<bool, String> {
    let affected = conn
        .execute(
            "UPDATE kanban_issue
             SET status = ?4, note = ?5, updated_at = ?6, version = version + 1,
                 assignee_kind = CASE WHEN ?7 THEN ?8 ELSE assignee_kind END,
                 assignee_member_role = CASE WHEN ?7 THEN NULL ELSE assignee_member_role END
             WHERE leader_conversation_id = ?1 AND number = ?2 AND status = ?3",
            params![
                leader_conversation_id,
                number as i64,
                from_status,
                to_status,
                note,
                updated_at,
                clear_assignee,
                KANBAN_ASSIGNEE_NONE,
            ],
        )
        .map_err(|err| format!("推进看板议题状态失败：{err}"))?;
    Ok(affected > 0)
}

/// 编号是否存在。批量写入前统一预检，避免「改了一半才发现某个号不存在」。
fn kanban_store_missing_numbers(
    conn: &Connection,
    leader_conversation_id: &str,
    numbers: &[u32],
) -> Result<Vec<u32>, String> {
    let mut missing = Vec::new();
    for number in numbers {
        if kanban_store_find(conn, leader_conversation_id, *number)?.is_none() {
            missing.push(*number);
        }
    }
    Ok(missing)
}

// ==================== 归属从编号迁移到角色 ====================

/// 一条还停在编号时代的归属。角色列空、编号列有值，就是没迁过。
struct KanbanLegacyMemberAssignee {
    leader_conversation_id: String,
    number: u32,
    member_number: u32,
}

fn kanban_store_legacy_member_assignees(
    conn: &Connection,
) -> Result<Vec<KanbanLegacyMemberAssignee>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT leader_conversation_id, number, assignee_member_number
             FROM kanban_issue
             WHERE assignee_kind = ?1
               AND assignee_member_role IS NULL
               AND assignee_member_number IS NOT NULL",
        )
        .map_err(|err| format!("查询待迁移归属失败：{err}"))?;
    let rows = stmt
        .query_map(params![KANBAN_ASSIGNEE_MEMBER], |row| {
            Ok(KanbanLegacyMemberAssignee {
                leader_conversation_id: row.get::<_, String>(0)?,
                number: row.get::<_, i64>(1)? as u32,
                member_number: row.get::<_, i64>(2)? as u32,
            })
        })
        .map_err(|err| format!("查询待迁移归属失败：{err}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| format!("查询待迁移归属失败：{err}"))?;
    Ok(rows)
}

/// 反查到了角色：只补角色列，编号列留原值当审计痕迹。
fn kanban_store_set_assignee_member_role(
    conn: &Connection,
    leader_conversation_id: &str,
    number: u32,
    role: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE kanban_issue SET assignee_member_role = ?3
         WHERE leader_conversation_id = ?1 AND number = ?2",
        params![leader_conversation_id, number as i64, role],
    )
    .map_err(|err| format!("迁移看板归属失败：{err}"))?;
    Ok(())
}

/// 反查不到角色：那个编号当时也指不到人，把无效引用显式化成 none。
fn kanban_store_clear_legacy_member_assignee(
    conn: &Connection,
    leader_conversation_id: &str,
    number: u32,
) -> Result<(), String> {
    conn.execute(
        "UPDATE kanban_issue SET assignee_kind = ?3
         WHERE leader_conversation_id = ?1 AND number = ?2",
        params![leader_conversation_id, number as i64, KANBAN_ASSIGNEE_NONE],
    )
    .map_err(|err| format!("迁移看板归属失败：{err}"))?;
    Ok(())
}

/// 测试专用：按编号时代的样子写一条归属（kind=member、角色列空、编号有值），
/// 用来验证一次性迁移。生产代码不会再写出这种行。
#[cfg(test)]
fn kanban_store_insert_legacy_member_assignee(
    conn: &Connection,
    issue: &KanbanIssue,
    member_number: u32,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO kanban_issue (
            id, leader_conversation_id, number, title, status, assignee_kind,
            assignee_member_role, assignee_member_number, note, created_at, updated_at, version
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10, ?11)",
        params![
            issue.id,
            issue.leader_conversation_id,
            issue.number as i64,
            issue.title,
            issue.status,
            KANBAN_ASSIGNEE_MEMBER,
            member_number as i64,
            issue.note,
            issue.created_at,
            issue.updated_at,
            issue.version as i64,
        ],
    )
    .map_err(|err| format!("写入历史归属失败：{err}"))?;
    Ok(())
}
