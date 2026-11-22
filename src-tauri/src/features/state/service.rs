// ==================== state service 业务访问层 ====================
// V4：业务代码只通过本层访问 state.sqlite。
// RuntimeStateFile 已退役；旧 JSON 格式只存在于迁移服务（migration.rs）。

// ========== k/v 标量 ==========

fn state_service_get_kv(state: &AppState, key: &str) -> Result<Option<String>, String> {
    state_db_get_kv(&state.data_path, key)
}

fn state_service_set_kv(state: &AppState, key: &str, value: &str) -> Result<(), String> {
    state_db_upsert_kv(&state.data_path, key, value)
}

fn state_service_get_kv_json<T>(state: &AppState, key: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned + Default,
{
    match state_service_get_kv(state, key)? {
        Some(raw) => serde_json::from_str(&raw)
            .map_err(|err| format!("解析 state 键失败，key={key}，error={err}")),
        None => Ok(T::default()),
    }
}

fn state_service_set_kv_json<T>(state: &AppState, key: &str, value: &T) -> Result<(), String>
where
    T: serde::Serialize + ?Sized,
{
    let raw = serde_json::to_string(value)
        .map_err(|err| format!("序列化 state 键失败，key={key}，error={err}"))?;
    state_service_set_kv(state, key, &raw)
}

// ---------- message_store_migration_version ----------

fn state_service_get_message_store_migration_version(state: &AppState) -> Result<u32, String> {
    match state_service_get_kv(state, "message_store_migration_version")? {
        Some(raw) => raw.parse::<u32>().map_err(|err| {
            format!("解析 message_store_migration_version 失败，value={raw}，error={err}")
        }),
        None => Ok(0),
    }
}

fn state_service_set_message_store_migration_version(
    state: &AppState,
    version: u32,
) -> Result<(), String> {
    state_service_set_kv(state, "message_store_migration_version", &version.to_string())
}

// ---------- data_migration_version ----------

fn state_service_get_data_migration_version(state: &AppState) -> Result<u32, String> {
    match state_service_get_kv(state, "data_migration_version")? {
        Some(raw) => raw.parse::<u32>().map_err(|err| {
            format!("解析 data_migration_version 失败，value={raw}，error={err}")
        }),
        None => Ok(0),
    }
}

fn state_service_set_data_migration_version(
    state: &AppState,
    version: u32,
) -> Result<(), String> {
    state_service_set_kv(state, "data_migration_version", &version.to_string())
}

// ---------- pinned_conversation_ids ----------

fn state_service_get_pinned_conversation_ids(state: &AppState) -> Result<Vec<String>, String> {
    state_service_get_kv_json::<Vec<String>>(state, "pinned_conversation_ids")
}

fn state_service_set_pinned_conversation_ids(
    state: &AppState,
    ids: &[String],
) -> Result<(), String> {
    state_service_set_kv_json(state, "pinned_conversation_ids", &ids.to_vec())
}

// ---------- conversation_section_orders ----------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationSectionOrders {
    #[serde(default)]
    local: Vec<String>,
    #[serde(default)]
    contact: Vec<String>,
}

fn state_service_get_conversation_section_orders(
    state: &AppState,
) -> Result<ConversationSectionOrders, String> {
    state_service_get_kv_json::<ConversationSectionOrders>(state, "conversation_section_orders")
}

fn state_service_set_conversation_section_orders(
    state: &AppState,
    orders: &ConversationSectionOrders,
) -> Result<(), String> {
    state_service_set_kv_json(state, "conversation_section_orders", orders)
}

// ---------- main_conversation_id ----------

fn state_service_get_main_conversation_id(state: &AppState) -> Result<Option<String>, String> {
    match state_service_get_kv(state, "main_conversation_id")? {
        Some(raw) if !raw.is_empty() => Ok(Some(raw)),
        _ => Ok(None),
    }
}

fn state_service_set_main_conversation_id(
    state: &AppState,
    id: Option<&str>,
) -> Result<(), String> {
    state_service_set_kv(state, "main_conversation_id", id.unwrap_or(""))
}

// ---------- 标量字符串配置 ----------

/// 助理人格恒为内置的 default-agent。
/// 「当前助理人格」没有用户入口，历史上只被新建人格等副作用写入 kv；
/// 人格一旦删除，kv 里的 id 就会悬空，读到它的入口全部失真。
/// 因此这里不再读 kv，直接返回内置默认人格。
fn state_service_get_assistant_agent_id(_state: &AppState) -> Result<String, String> {
    Ok(default_assistant_agent_id())
}

fn state_service_set_assistant_agent_id(
    state: &AppState,
    value: &str,
) -> Result<(), String> {
    state_service_set_kv(state, "assistant_agent_id", value)
}

fn state_service_get_response_style_id(state: &AppState) -> Result<String, String> {
    Ok(state_service_get_kv(state, "response_style_id")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_response_style_id))
}

fn state_service_set_response_style_id(state: &AppState, value: &str) -> Result<(), String> {
    state_service_set_kv(state, "response_style_id", value)
}

fn state_service_get_pdf_read_mode(state: &AppState) -> Result<String, String> {
    Ok(state_service_get_kv(state, "pdf_read_mode")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_pdf_read_mode))
}

fn state_service_set_pdf_read_mode(state: &AppState, value: &str) -> Result<(), String> {
    state_service_set_kv(state, "pdf_read_mode", value)
}

fn state_service_get_background_voice_screenshot_keywords(
    state: &AppState,
) -> Result<String, String> {
    Ok(state_service_get_kv(state, "background_voice_screenshot_keywords")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_background_voice_screenshot_keywords))
}

fn state_service_set_background_voice_screenshot_keywords(
    state: &AppState,
    value: &str,
) -> Result<(), String> {
    state_service_set_kv(state, "background_voice_screenshot_keywords", value)
}

fn state_service_get_background_voice_screenshot_mode(state: &AppState) -> Result<String, String> {
    Ok(state_service_get_kv(state, "background_voice_screenshot_mode")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_background_voice_screenshot_mode))
}

fn state_service_set_background_voice_screenshot_mode(
    state: &AppState,
    value: &str,
) -> Result<(), String> {
    state_service_set_kv(state, "background_voice_screenshot_mode", value)
}

// ---------- instruction_presets ----------

fn state_service_get_instruction_presets(
    state: &AppState,
) -> Result<Vec<PromptCommandPreset>, String> {
    state_service_get_kv_json::<Vec<PromptCommandPreset>>(state, "instruction_presets")
}

fn state_service_set_instruction_presets(
    state: &AppState,
    presets: &[PromptCommandPreset],
) -> Result<(), String> {
    state_service_set_kv_json(state, "instruction_presets", &presets.to_vec())
}

// ========== 图片/PDF 文本缓存 ==========

fn state_service_find_image_text_cache(
    state: &AppState,
    hash: &str,
    model_api_id: &str,
    media_type: &str,
    description: &str,
) -> Result<Option<String>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT text FROM image_text_cache
             WHERE hash=?1 AND model_api_id=?2 AND media_type=?3 AND description=?4",
        )
        .map_err(|err| format!("准备 image_text_cache 查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![hash, model_api_id, media_type, description])
        .map_err(|err| format!("查询 image_text_cache 失败，error={err}"))?;
    if let Some(row) = rows
        .next()
        .map_err(|err| format!("读取 image_text_cache 失败，error={err}"))?
    {
        let text: String = row
            .get(0)
            .map_err(|err| format!("解析 image_text_cache 失败，error={err}"))?;
        Ok(Some(text))
    } else {
        Ok(None)
    }
}

/// 列出全部图片文本缓存条目（text + updated_at）。
fn state_service_list_image_text_cache(
    state: &AppState,
) -> Result<Vec<(String, String)>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT text, updated_at FROM image_text_cache")
        .map_err(|err| format!("准备 image_text_cache 列表查询失败，error={err}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|err| format!("查询 image_text_cache 列表失败，error={err}"))?;
    let mut entries = Vec::new();
    for item in rows {
        entries.push(item.map_err(|err| format!("读取 image_text_cache 列表失败，error={err}"))?);
    }
    Ok(entries)
}

/// 清空图片文本缓存。
fn state_service_clear_image_text_cache(state: &AppState) -> Result<(), String> {
    let conn = state_db_open(&state.data_path)?;
    conn.execute("DELETE FROM image_text_cache", [])
        .map_err(|err| format!("清空 image_text_cache 失败，error={err}"))?;
    Ok(())
}

fn state_service_upsert_image_text_cache(
    state: &AppState,
    hash: &str,
    model_api_id: &str,
    media_type: &str,
    description: &str,
    text: &str,
) -> Result<(), String> {
    let conn = state_db_open(&state.data_path)?;
    conn.execute(
        "INSERT INTO image_text_cache(hash, model_api_id, media_type, description, text, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(hash, model_api_id, media_type, description) DO UPDATE SET
           text=excluded.text,
           updated_at=excluded.updated_at",
        rusqlite::params![hash, model_api_id, media_type, description, text, now_iso()],
    )
    .map_err(|err| format!("写入 image_text_cache 失败，error={err}"))?;
    state_db_trim_image_text_cache(&state.data_path)?;
    Ok(())
}

// ========== remote_im 联系人 ==========

fn state_service_get_remote_im_contact(
    state: &AppState,
    contact_id: &str,
) -> Result<Option<RemoteImContact>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT config_json FROM remote_im_contacts WHERE id=?1")
        .map_err(|err| format!("准备 remote_im_contacts 查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![contact_id])
        .map_err(|err| format!("查询 remote_im_contacts 失败，error={err}"))?;
    if let Some(row) = rows
        .next()
        .map_err(|err| format!("读取 remote_im_contacts 失败，error={err}"))?
    {
        let config_json: String = row
            .get(0)
            .map_err(|err| format!("解析 remote_im_contacts 失败，error={err}"))?;
        let contact = deserialize_remote_im_contact(&conn, &config_json)?;
        Ok(Some(contact))
    } else {
        Ok(None)
    }
}

/// 列出联系人；channel_id 为 None 时列出全部。
fn state_service_list_remote_im_contacts(
    state: &AppState,
    channel_id: Option<&str>,
) -> Result<Vec<RemoteImContact>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT config_json FROM remote_im_contacts WHERE (?1 IS NULL OR channel_id=?1)")
        .map_err(|err| format!("准备 remote_im_contacts 列表查询失败，error={err}"))?;
    let rows = stmt
        .query_map(rusqlite::params![channel_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("查询 remote_im_contacts 列表失败，error={err}"))?;
    let mut contacts = Vec::new();
    for item in rows {
        let config_json = item.map_err(|err| format!("读取 remote_im_contacts 列表失败，error={err}"))?;
        contacts.push(deserialize_remote_im_contact(&conn, &config_json)?);
    }
    Ok(contacts)
}

/// 按渠道身份三元组（channel_id + remote_contact_type + remote_contact_id）查找联系人。
fn state_service_find_remote_im_contact_by_identity(
    state: &AppState,
    channel_id: &str,
    remote_contact_type: &str,
    remote_contact_id: &str,
) -> Result<Option<RemoteImContact>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT config_json FROM remote_im_contacts
             WHERE channel_id=?1 AND remote_contact_type=?2 AND remote_contact_id=?3 LIMIT 1",
        )
        .map_err(|err| format!("准备 remote_im_contacts 身份查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![channel_id, remote_contact_type, remote_contact_id])
        .map_err(|err| format!("查询 remote_im_contacts 身份失败，error={err}"))?;
    if let Some(row) = rows
        .next()
        .map_err(|err| format!("读取 remote_im_contacts 身份失败，error={err}"))?
    {
        let config_json: String = row
            .get(0)
            .map_err(|err| format!("解析 remote_im_contacts 身份失败，error={err}"))?;
        let contact = deserialize_remote_im_contact(&conn, &config_json)?;
        Ok(Some(contact))
    } else {
        Ok(None)
    }
}

fn state_service_upsert_remote_im_contact(
    state: &AppState,
    contact: &RemoteImContact,
) -> Result<(), String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 remote_im_contacts 事务失败，error={err}"))?;
    state_db_write_remote_im_contact_row(&tx, contact)?;
    tx.commit()
        .map_err(|err| format!("提交 remote_im_contacts 事务失败，error={err}"))?;
    Ok(())
}

/// 一批联系人整体写入：单事务，任一联系人写入失败时整体回滚。
fn state_service_upsert_remote_im_contacts(
    state: &AppState,
    contacts: &[RemoteImContact],
) -> Result<(), String> {
    if contacts.is_empty() {
        return Ok(());
    }
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 remote_im_contacts 批量事务失败，error={err}"))?;
    for contact in contacts {
        state_db_write_remote_im_contact_row(&tx, contact)?;
    }
    tx.commit()
        .map_err(|err| format!("提交 remote_im_contacts 批量事务失败，error={err}"))?;
    Ok(())
}

fn state_db_write_remote_im_contact_row(
    tx: &rusqlite::Transaction<'_>,
    contact: &RemoteImContact,
) -> Result<(), String> {
    let mut config = serde_json::to_value(contact)
        .map_err(|err| format!("序列化 remote_im_contacts 失败，error={err}"))?;
    if let Some(obj) = config.as_object_mut() {
        obj.insert("onebotGroupMembers".to_string(), serde_json::Value::Array(vec![]));
    }
    let config_json = serde_json::to_string(&config)
        .map_err(|err| format!("序列化 remote_im_contacts config 失败，error={err}"))?;
    let platform_str = serde_json::to_string(&contact.platform)
        .map(|raw| raw.trim_matches('"').to_string())
        .map_err(|err| format!("序列化 remote_im_contacts 平台类型失败，contact_id={}，error={err}", contact.id))?;
    tx.execute(
        "INSERT INTO remote_im_contacts(id, channel_id, platform, remote_contact_type, remote_contact_id, config_json)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           channel_id=excluded.channel_id,
           platform=excluded.platform,
           remote_contact_type=excluded.remote_contact_type,
           remote_contact_id=excluded.remote_contact_id,
           config_json=excluded.config_json",
        rusqlite::params![
            contact.id,
            contact.channel_id,
            platform_str,
            contact.remote_contact_type,
            contact.remote_contact_id,
            config_json
        ],
    )
    .map_err(|err| format!("写入 remote_im_contacts 失败，contact_id={}，error={err}", contact.id))?;

    // 群成员独立成行：先清后插，保持与 config 一致
    tx.execute(
        "DELETE FROM remote_im_group_members WHERE contact_id=?1",
        rusqlite::params![contact.id],
    )
    .map_err(|err| format!("清理 remote_im_group_members 失败，contact_id={}，error={err}", contact.id))?;
    for member in &contact.onebot_group_members {
        tx.execute(
            "INSERT INTO remote_im_group_members(contact_id, user_id, nickname, card, display_name, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                contact.id,
                member.user_id,
                member.nickname,
                member.card,
                member.display_name,
                member.updated_at
            ],
        )
        .map_err(|err| format!("写入 remote_im_group_members 失败，contact_id={}，user_id={}，error={err}", contact.id, member.user_id))?;
    }
    Ok(())
}

// ==================== 联系人自定义分组 ====================

/// 按渠道列出联系人分组，排序值小的在前。
fn state_service_list_remote_im_contact_groups(
    state: &AppState,
    channel_id: &str,
) -> Result<Vec<RemoteImContactGroup>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, channel_id, name, sort_order, created_at, updated_at
             FROM remote_im_contact_groups WHERE channel_id=?1
             ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|err| format!("准备 remote_im_contact_groups 查询失败，error={err}"))?;
    let rows = stmt
        .query_map(rusqlite::params![channel_id], |row| {
            Ok(RemoteImContactGroup {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                name: row.get(2)?,
                sort_order: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|err| format!("查询 remote_im_contact_groups 失败，error={err}"))?;
    let mut groups = Vec::new();
    for item in rows {
        groups.push(item.map_err(|err| format!("读取 remote_im_contact_groups 失败，error={err}"))?);
    }
    Ok(groups)
}

/// 同渠道内是否已有同名分组；exclude_group_id 用于重命名时排除自身。
fn state_service_remote_im_contact_group_name_exists(
    state: &AppState,
    channel_id: &str,
    name: &str,
    exclude_group_id: Option<&str>,
) -> Result<bool, String> {
    let conn = state_db_open(&state.data_path)?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM remote_im_contact_groups
             WHERE channel_id=?1 AND name=?2 AND (?3 IS NULL OR id<>?3)",
            rusqlite::params![channel_id, name, exclude_group_id],
            |row| row.get(0),
        )
        .map_err(|err| format!("查询 remote_im_contact_groups 重名失败，error={err}"))?;
    Ok(count > 0)
}

/// 新建分组时的排序值：当前渠道最大值加一。
fn state_service_next_remote_im_contact_group_sort_order(
    state: &AppState,
    channel_id: &str,
) -> Result<i64, String> {
    let conn = state_db_open(&state.data_path)?;
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM remote_im_contact_groups WHERE channel_id=?1",
        rusqlite::params![channel_id],
        |row| row.get(0),
    )
    .map_err(|err| format!("查询 remote_im_contact_groups 排序失败，error={err}"))
}

fn state_service_upsert_remote_im_contact_group(
    state: &AppState,
    group: &RemoteImContactGroup,
) -> Result<(), String> {
    let conn = state_db_open(&state.data_path)?;
    conn.execute(
        "INSERT INTO remote_im_contact_groups(id, channel_id, name, sort_order, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           name=excluded.name,
           sort_order=excluded.sort_order,
           updated_at=excluded.updated_at",
        rusqlite::params![
            group.id,
            group.channel_id,
            group.name,
            group.sort_order,
            group.created_at,
            group.updated_at
        ],
    )
    .map_err(|err| format!("写入 remote_im_contact_groups 失败，group_id={}，error={err}", group.id))?;
    Ok(())
}

/// 删除分组：先把组内联系人回落到未分组，再删分组，两步同一事务。
fn state_service_delete_remote_im_contact_group(
    state: &AppState,
    group_id: &str,
) -> Result<usize, String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 remote_im_contact_groups 删除事务失败，error={err}"))?;
    let detached = tx
        .execute(
            "UPDATE remote_im_contacts SET config_json = json_set(config_json, '$.groupId', NULL)
             WHERE json_extract(config_json, '$.groupId')=?1",
            rusqlite::params![group_id],
        )
        .map_err(|err| format!("回落分组内联系人失败，group_id={group_id}，error={err}"))?;
    tx.execute(
        "DELETE FROM remote_im_contact_groups WHERE id=?1",
        rusqlite::params![group_id],
    )
    .map_err(|err| format!("删除 remote_im_contact_groups 失败，group_id={group_id}，error={err}"))?;
    tx.commit()
        .map_err(|err| format!("提交 remote_im_contact_groups 删除事务失败，error={err}"))?;
    Ok(detached)
}

/// 重命名分组：只改名称与更新时间，保留排序与创建时间。
fn state_service_rename_remote_im_contact_group(
    state: &AppState,
    group_id: &str,
    name: &str,
    updated_at: &str,
) -> Result<bool, String> {
    let conn = state_db_open(&state.data_path)?;
    let updated = conn
        .execute(
            "UPDATE remote_im_contact_groups SET name=?2, updated_at=?3 WHERE id=?1",
            rusqlite::params![group_id, name, updated_at],
        )
        .map_err(|err| format!("重命名 remote_im_contact_groups 失败，group_id={group_id}，error={err}"))?;
    Ok(updated > 0)
}

/// 把一组联系人移入分组（group_id 为 None 表示移入未分组）；只接受该渠道下的联系人，单事务。
fn state_service_set_remote_im_contacts_group(
    state: &AppState,
    channel_id: &str,
    contact_ids: &[String],
    group_id: Option<&str>,
) -> Result<usize, String> {
    if contact_ids.is_empty() {
        return Ok(0);
    }
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 remote_im_contacts 分组移动事务失败，error={err}"))?;
    let mut moved = 0usize;
    for contact_id in contact_ids {
        let updated = tx
            .execute(
                "UPDATE remote_im_contacts SET config_json = json_set(config_json, '$.groupId', ?1)
                 WHERE id=?2 AND channel_id=?3",
                rusqlite::params![group_id, contact_id, channel_id],
            )
            .map_err(|err| format!("移动 remote_im_contacts 分组失败，contact_id={contact_id}，error={err}"))?;
        if updated == 0 {
            return Err(format!("联系人不存在或不属于该渠道：{contact_id}"));
        }
        moved = moved.saturating_add(1);
    }
    tx.commit()
        .map_err(|err| format!("提交 remote_im_contacts 分组移动事务失败，error={err}"))?;
    Ok(moved)
}

/// 分组所属渠道；用于重命名与删除前的存在性校验。
fn state_service_remote_im_contact_group_channel_id(
    state: &AppState,
    group_id: &str,
) -> Result<Option<String>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT channel_id FROM remote_im_contact_groups WHERE id=?1")
        .map_err(|err| format!("准备 remote_im_contact_groups 渠道查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![group_id])
        .map_err(|err| format!("查询 remote_im_contact_groups 渠道失败，error={err}"))?;
    match rows
        .next()
        .map_err(|err| format!("读取 remote_im_contact_groups 渠道失败，error={err}"))?
    {
        Some(row) => Ok(Some(
            row.get(0)
                .map_err(|err| format!("解析 remote_im_contact_groups 渠道失败，group_id={group_id}，error={err}"))?,
        )),
        None => Ok(None),
    }
}

/// 仅当联系人当前绑定仍为 expected_conversation_id 时，才清除会话绑定（原子条件更新）。
/// 返回是否执行了清除；绑定已被其他请求修改时返回 Ok(false)，调用方应停止后续删除流程。
fn state_service_clear_remote_im_contact_binding_if_matches(
    state: &AppState,
    contact_id: &str,
    expected_conversation_id: &str,
) -> Result<bool, String> {
    let conn = state_db_open(&state.data_path)?;
    let cleared = conn
        .execute(
            "UPDATE remote_im_contacts
             SET config_json = json_set(config_json, '$.boundConversationId', NULL)
             WHERE id=?1 AND json_extract(config_json, '$.boundConversationId')=?2",
            rusqlite::params![contact_id, expected_conversation_id],
        )
        .map_err(|err| format!("条件清除 remote_im_contacts 绑定失败，contact_id={contact_id}，error={err}"))?;
    Ok(cleared > 0)
}

/// 只 upsert 联系人群成员行（不动 contacts 主表），避免整条回写覆盖并发修改。
fn state_service_upsert_remote_im_group_members(
    state: &AppState,
    contact_id: &str,
    members: &[RemoteImGroupMemberInfo],
) -> Result<(), String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 remote_im_group_members 事务失败，error={err}"))?;
    for member in members {
        tx.execute(
            "INSERT INTO remote_im_group_members(contact_id, user_id, nickname, card, display_name, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(contact_id, user_id) DO UPDATE SET
               nickname=excluded.nickname,
               card=excluded.card,
               display_name=excluded.display_name,
               updated_at=excluded.updated_at",
            rusqlite::params![
                contact_id,
                member.user_id,
                member.nickname,
                member.card,
                member.display_name,
                member.updated_at
            ],
        )
        .map_err(|err| format!("写入 remote_im_group_members 失败，contact_id={contact_id}，user_id={}，error={err}", member.user_id))?;
    }
    tx.commit()
        .map_err(|err| format!("提交 remote_im_group_members 事务失败，error={err}"))?;
    Ok(())
}

fn state_service_remove_remote_im_contact(
    state: &AppState,
    contact_id: &str,
) -> Result<bool, String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启删除 remote_im_contacts 事务失败，error={err}"))?;
    let removed = tx
        .execute(
            "DELETE FROM remote_im_contacts WHERE id=?1",
            rusqlite::params![contact_id],
        )
        .map_err(|err| format!("删除 remote_im_contacts 失败，contact_id={contact_id}，error={err}"))?;
    tx.execute(
        "DELETE FROM remote_im_group_members WHERE contact_id=?1",
        rusqlite::params![contact_id],
    )
    .map_err(|err| format!("删除 remote_im_group_members 失败，contact_id={contact_id}，error={err}"))?;
    tx.execute(
        "DELETE FROM remote_im_contact_checkpoints WHERE contact_id=?1",
        rusqlite::params![contact_id],
    )
    .map_err(|err| format!("删除 remote_im_contact_checkpoints 失败，contact_id={contact_id}，error={err}"))?;
    tx.commit()
        .map_err(|err| format!("提交删除 remote_im_contacts 事务失败，error={err}"))?;
    Ok(removed > 0)
}

/// 旧版迁移会把 `Option` 字段以 `null` 原样写入 config_json（如 groupReplyPacing），
/// 而当前结构里这些字段可能已改为非 Option + `#[serde(default)]`；`serde(default)`
/// 只兜底「字段缺失」不兜底「字段为 null」。先把顶层 null 键剔除再交给 serde，
/// 让 default 生效；等下一次 upsert 写回干净 JSON 完成自愈。
fn deserialize_state_json_tolerating_null_fields<T>(
    raw_json: &str,
    context: &str,
) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let mut raw = serde_json::from_str::<serde_json::Value>(raw_json)
        .map_err(|err| format!("解析 {context} JSON 失败，error={err}"))?;
    if let Some(obj) = raw.as_object_mut() {
        obj.retain(|_, value| !value.is_null());
    }
    serde_json::from_value(raw).map_err(|err| format!("反序列化 {context} 失败，error={err}"))
}

/// config_json → RemoteImContact，并回填成员表数据。
fn deserialize_remote_im_contact(
    conn: &rusqlite::Connection,
    config_json: &str,
) -> Result<RemoteImContact, String> {
    let mut contact: RemoteImContact =
        deserialize_state_json_tolerating_null_fields(config_json, "remote_im_contacts config")?;
    let mut stmt = conn
        .prepare(
            "SELECT user_id, nickname, card, display_name, updated_at
             FROM remote_im_group_members WHERE contact_id=?1",
        )
        .map_err(|err| format!("准备 remote_im_group_members 查询失败，error={err}"))?;
    let rows = stmt
        .query_map(rusqlite::params![contact.id], |row| {
            Ok(RemoteImGroupMemberInfo {
                user_id: row.get(0)?,
                nickname: row.get(1)?,
                card: row.get(2)?,
                display_name: row.get(3)?,
                updated_at: row.get(4)?,
                raw: None,
            })
        })
        .map_err(|err| format!("查询 remote_im_group_members 失败，error={err}"))?;
    let mut members = Vec::new();
    for item in rows {
        members.push(item.map_err(|err| format!("读取 remote_im_group_members 失败，error={err}"))?);
    }
    contact.onebot_group_members = members;
    Ok(contact)
}

// ========== remote_im checkpoints ==========

fn state_service_get_remote_im_contact_checkpoint(
    state: &AppState,
    contact_id: &str,
) -> Result<Option<RemoteImContactCheckpoint>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT checkpoint_json FROM remote_im_contact_checkpoints WHERE contact_id=?1")
        .map_err(|err| format!("准备 remote_im_contact_checkpoints 查询失败，error={err}"))?;
    let mut rows = stmt
        .query(rusqlite::params![contact_id])
        .map_err(|err| format!("查询 remote_im_contact_checkpoints 失败，error={err}"))?;
    if let Some(row) = rows
        .next()
        .map_err(|err| format!("读取 remote_im_contact_checkpoints 失败，error={err}"))?
    {
        let checkpoint_json: String = row
            .get(0)
            .map_err(|err| format!("解析 remote_im_contact_checkpoints 失败，error={err}"))?;
        let checkpoint: RemoteImContactCheckpoint = deserialize_state_json_tolerating_null_fields(
            &checkpoint_json,
            "remote_im_contact_checkpoints",
        )?;
        Ok(Some(checkpoint))
    } else {
        Ok(None)
    }
}

/// 列出全部 checkpoint。
fn state_service_list_remote_im_contact_checkpoints(
    state: &AppState,
) -> Result<Vec<RemoteImContactCheckpoint>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT checkpoint_json FROM remote_im_contact_checkpoints")
        .map_err(|err| format!("准备 remote_im_contact_checkpoints 列表查询失败，error={err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("查询 remote_im_contact_checkpoints 列表失败，error={err}"))?;
    let mut checkpoints = Vec::new();
    for item in rows {
        let checkpoint_json =
            item.map_err(|err| format!("读取 remote_im_contact_checkpoints 列表失败，error={err}"))?;
        let checkpoint: RemoteImContactCheckpoint = deserialize_state_json_tolerating_null_fields(
            &checkpoint_json,
            "remote_im_contact_checkpoints",
        )?;
        checkpoints.push(checkpoint);
    }
    Ok(checkpoints)
}

fn state_service_set_remote_im_contact_checkpoint(
    state: &AppState,
    checkpoint: &RemoteImContactCheckpoint,
) -> Result<(), String> {
    let conn = state_db_open(&state.data_path)?;
    let checkpoint_json = serde_json::to_string(checkpoint)
        .map_err(|err| format!("序列化 remote_im_contact_checkpoints 失败，error={err}"))?;
    conn.execute(
        "INSERT INTO remote_im_contact_checkpoints(contact_id, checkpoint_json)
         VALUES(?1, ?2)
         ON CONFLICT(contact_id) DO UPDATE SET checkpoint_json=excluded.checkpoint_json",
        rusqlite::params![checkpoint.contact_id, checkpoint_json],
    )
    .map_err(|err| format!("写入 remote_im_contact_checkpoints 失败，contact_id={}，error={err}", checkpoint.contact_id))?;
    Ok(())
}

// ========== 窗口布局 ==========

fn state_service_get_window_layouts(state: &AppState) -> Result<PersistedWindowLayouts, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT window_label, width, height, x, y, maximized FROM window_layouts")
        .map_err(|err| format!("准备 window_layouts 查询失败，error={err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PersistedWindowLayout {
                    width: row.get(1)?,
                    height: row.get(2)?,
                    x: row.get(3)?,
                    y: row.get(4)?,
                    maximized: row.get::<_, i64>(5)? != 0,
                },
            ))
        })
        .map_err(|err| format!("查询 window_layouts 失败，error={err}"))?;
    let mut layouts = PersistedWindowLayouts::default();
    for item in rows {
        let (label, layout) = item.map_err(|err| format!("读取 window_layouts 失败，error={err}"))?;
        layouts.windows.insert(label, layout);
    }
    Ok(layouts)
}

fn state_service_save_window_layouts(
    state: &AppState,
    layouts: &PersistedWindowLayouts,
) -> Result<(), String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 window_layouts 事务失败，error={err}"))?;
    tx.execute("DELETE FROM window_layouts", [])
        .map_err(|err| format!("清理 window_layouts 失败，error={err}"))?;
    for (label, layout) in &layouts.windows {
        tx.execute(
            "INSERT INTO window_layouts(window_label, width, height, x, y, maximized)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                label,
                layout.width,
                layout.height,
                layout.x,
                layout.y,
                layout.maximized as i64
            ],
        )
        .map_err(|err| format!("写入 window_layouts 失败，label={label}，error={err}"))?;
    }
    tx.commit()
        .map_err(|err| format!("提交 window_layouts 事务失败，error={err}"))?;
    Ok(())
}

// ========== git 仓库历史 ==========

fn state_service_get_git_repo_history(
    state: &AppState,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let conn = state_db_open(&state.data_path)?;
    let mut stmt = conn
        .prepare("SELECT repo_key, history_json FROM git_repo_history")
        .map_err(|err| format!("准备 git_repo_history 查询失败，error={err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("查询 git_repo_history 失败，error={err}"))?;
    let mut history = std::collections::HashMap::new();
    for item in rows {
        let (repo_key, history_json) =
            item.map_err(|err| format!("读取 git_repo_history 失败，error={err}"))?;
        let group: Vec<String> = serde_json::from_str(&history_json)
            .map_err(|err| format!("解析 git_repo_history JSON 失败，repo_key={repo_key}，error={err}"))?;
        history.insert(repo_key, group);
    }
    Ok(history)
}

fn state_service_save_git_repo_history(
    state: &AppState,
    history: &std::collections::HashMap<String, Vec<String>>,
) -> Result<(), String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 git_repo_history 事务失败，error={err}"))?;
    tx.execute("DELETE FROM git_repo_history", [])
        .map_err(|err| format!("清理 git_repo_history 失败，error={err}"))?;
    for (repo_key, group) in history {
        let history_json = serde_json::to_string(group)
            .map_err(|err| format!("序列化 git_repo_history 失败，repo_key={repo_key}，error={err}"))?;
        tx.execute(
            "INSERT INTO git_repo_history(repo_key, history_json) VALUES(?1, ?2)",
            rusqlite::params![repo_key, history_json],
        )
        .map_err(|err| format!("写入 git_repo_history 失败，repo_key={repo_key}，error={err}"))?;
    }
    tx.commit()
        .map_err(|err| format!("提交 git_repo_history 事务失败，error={err}"))?;
    Ok(())
}

// ========== PDF 缓存清理与统计 ==========

/// 从 pdf 文本/图片缓存中移除指定会话关联；条目失去全部关联后删除。
/// 返回 (移除的文本条目数, 移除的图片条目数)。
fn state_service_remove_conversation_from_pdf_caches(
    state: &AppState,
    conversation_id: &str,
) -> Result<(usize, usize), String> {
    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 PDF 缓存清理事务失败，error={err}"))?;
    let removed_text = remove_conversation_from_pdf_cache_table(
        &tx,
        "pdf_text_cache",
        conversation_id,
    )?;
    let removed_image = remove_conversation_from_pdf_cache_table(
        &tx,
        "pdf_image_cache",
        conversation_id,
    )?;
    tx.commit()
        .map_err(|err| format!("提交 PDF 缓存清理事务失败，error={err}"))?;
    Ok((removed_text, removed_image))
}

fn remove_conversation_from_pdf_cache_table(
    tx: &rusqlite::Transaction,
    table: &str,
    conversation_id: &str,
) -> Result<usize, String> {
    let mut stmt = tx
        .prepare(&format!(
            "SELECT file_hash, file_name, conversation_ids FROM {table}"
        ))
        .map_err(|err| format!("准备 {table} 清理查询失败，error={err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .map_err(|err| format!("查询 {table} 清理失败，error={err}"))?;
    let mut removals: Vec<(String, String)> = Vec::new();
    for item in rows {
        let (file_hash, file_name, conversation_ids_json) =
            item.map_err(|err| format!("读取 {table} 清理失败，error={err}"))?;
        let mut conversation_ids: Vec<String> = serde_json::from_str(&conversation_ids_json)
            .map_err(|err| format!("解析 {table} conversation_ids 失败，error={err}"))?;
        let before = conversation_ids.len();
        conversation_ids.retain(|id| id != conversation_id);
        if conversation_ids.is_empty() {
            removals.push((file_hash, file_name));
        } else if conversation_ids.len() != before {
            let updated_json = serde_json::to_string(&conversation_ids)
                .map_err(|err| format!("序列化 {table} conversation_ids 失败，error={err}"))?;
            tx.execute(
                &format!(
                    "UPDATE {table} SET conversation_ids=?1, updated_at=?2 WHERE file_hash=?3"
                ),
                rusqlite::params![updated_json, now_iso(), file_hash],
            )
            .map_err(|err| format!("更新 {table} conversation_ids 失败，error={err}"))?;
        }
    }
    let mut removed = 0usize;
    for (file_hash, file_name) in removals {
        runtime_log_info(format!(
            "[存储] PDF缓存清理 删除缓存条目 表={table} 文件={file_name} 校验={file_hash}"
        ));
        tx.execute(
            &format!("DELETE FROM {table} WHERE file_hash=?1"),
            rusqlite::params![file_hash],
        )
        .map_err(|err| format!("删除 {table} 失败，error={err}"))?;
        removed += 1;
    }
    Ok(removed)
}

fn state_service_count_image_text_cache(state: &AppState) -> Result<usize, String> {
    state_db_count(&state.data_path, "image_text_cache")
}

fn state_service_count_pdf_text_cache(state: &AppState) -> Result<usize, String> {
    state_db_count(&state.data_path, "pdf_text_cache")
}

fn state_service_count_pdf_image_cache(state: &AppState) -> Result<usize, String> {
    state_db_count(&state.data_path, "pdf_image_cache")
}

fn state_db_count(data_path: &PathBuf, table: &str) -> Result<usize, String> {
    let conn = state_db_open(data_path)?;
    let count: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM {table}"),
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("统计 {table} 条目数失败，error={err}"))?;
    Ok(count as usize)
}
