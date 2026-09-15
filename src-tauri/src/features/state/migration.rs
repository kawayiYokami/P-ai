// ==================== state 迁移服务 ====================
// V4：旧 JSON（runtime_state.json / window_layouts.json / git_panel_repo_history.json）
// 只在本模块被读取。迁移完成后业务代码不再感知旧格式。
//
// 流程：
//   1. 检测 state/state.sqlite 是否存在
//   2. 存在 → 读 SQL 中版本号，无需迁移
//   3. 不存在 → 读旧 JSON → 写入 SQLite → 记录版本号
// 幂等：迁移后 SQL 存在，重复调用直接跳过。

const STATE_MIGRATION_VERSION: u32 = 4;

fn legacy_runtime_state_path(data_path: &PathBuf) -> PathBuf {
    app_layout_state_dir(data_path).join("runtime_state.json")
}

fn legacy_window_layouts_path(data_path: &PathBuf) -> PathBuf {
    app_layout_state_dir(data_path).join("window_layouts.json")
}

fn legacy_git_repo_history_path(data_path: &PathBuf) -> PathBuf {
    app_layout_state_dir(data_path).join("git_panel_repo_history.json")
}

/// 检测是否需要迁移：SQL 不存在（或版本缺失）时需要读旧 JSON。
fn state_migration_needed(data_path: &PathBuf) -> Result<bool, String> {
    if !state_db_path(data_path).exists() {
        return Ok(true);
    }
    Ok(state_db_get_migration_version(data_path)?.is_none())
}

/// 执行迁移（幂等）。返回是否发生了迁移。
fn run_state_migration_if_needed(state: &AppState) -> Result<bool, String> {
    if !state_migration_needed(&state.data_path)? {
        return Ok(false);
    }
    runtime_log_info("[state迁移] 开始，任务=旧JSON迁入state.sqlite".to_string());

    let mut conn = state_db_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启 state 迁移事务失败，error={err}"))?;

    // 1. runtime_state.json → runtime_state k/v + remote_im 表
    let runtime_path = legacy_runtime_state_path(&state.data_path);
    if runtime_path.exists() {
        let legacy: LegacyRuntimeStateFile = read_json_file(&runtime_path, "legacy runtime state")?;
        migrate_legacy_runtime_state(&tx, &legacy)?;
    }

    // 2. window_layouts.json → window_layouts 表
    let layouts_path = legacy_window_layouts_path(&state.data_path);
    if layouts_path.exists() {
        let legacy: LegacyWindowLayouts = read_json_file(&layouts_path, "legacy window layouts")?;
        migrate_legacy_window_layouts(&tx, &legacy)?;
    }

    // 3. git_panel_repo_history.json → git_repo_history 表
    let git_path = legacy_git_repo_history_path(&state.data_path);
    if git_path.exists() {
        let legacy: std::collections::HashMap<String, Vec<String>> =
            read_json_file(&git_path, "legacy git repo history")?;
        migrate_legacy_git_repo_history(&tx, &legacy)?;
    }

    tx.execute(
        "INSERT INTO state_migration(version, migrated_at) VALUES(?1, ?2)",
        rusqlite::params![STATE_MIGRATION_VERSION, now_iso()],
    )
    .map_err(|err| format!("写入 state 迁移版本失败，version={STATE_MIGRATION_VERSION}，error={err}"))?;
    tx.commit()
        .map_err(|err| format!("提交 state 迁移事务失败，error={err}"))?;
    runtime_log_info("[state迁移] 完成，任务=旧JSON迁入state.sqlite".to_string());
    Ok(true)
}

fn migrate_legacy_runtime_state(
    tx: &rusqlite::Transaction,
    legacy: &LegacyRuntimeStateFile,
) -> Result<(), String> {
    // runtime_state k/v：配置与全局状态
    let kv_pairs: Vec<(&str, String)> = vec![
        ("assistant_agent_id", legacy.assistant_department_agent_id.clone()),
        ("response_style_id", legacy.response_style_id.clone()),
        ("pdf_read_mode", legacy.pdf_read_mode.clone()),
        ("background_voice_screenshot_keywords", legacy.background_voice_screenshot_keywords.clone()),
        ("background_voice_screenshot_mode", legacy.background_voice_screenshot_mode.clone()),
        ("instruction_presets", serde_json::to_string(&legacy.instruction_presets).map_err(|e| format!("序列化 instruction_presets 失败：{e}"))?),
        ("main_conversation_id", legacy.main_conversation_id.clone().or_else(|| legacy.system_notification_conversation_id.clone()).unwrap_or_default()),
        ("pinned_conversation_ids", serde_json::to_string(&legacy.pinned_conversation_ids).map_err(|e| format!("序列化 pinned_conversation_ids 失败：{e}"))?),
        ("data_migration_version", legacy.data_migration_version.to_string()),
        ("message_store_migration_version", legacy.message_store_migration_version.to_string()),
    ];
    for (key, value) in kv_pairs {
        tx.execute(
            "INSERT INTO runtime_state(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![key, value],
        )
        .map_err(|err| format!("迁移 runtime_state k/v 失败，key={key}，error={err}"))?;
    }

    // remote_im_contacts + group_members
    for contact in &legacy.remote_im_contacts {
        let (config_json, member_rows) = build_remote_im_contact_rows(contact)?;
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
                contact.platform,
                contact.remote_contact_type,
                contact.remote_contact_id,
                config_json
            ],
        )
        .map_err(|err| format!("迁移 remote_im_contacts 失败，contact_id={}，error={err}", contact.id))?;
        for (user_id, nickname, card, display_name, updated_at) in member_rows {
            tx.execute(
                "INSERT INTO remote_im_group_members(contact_id, user_id, nickname, card, display_name, updated_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(contact_id, user_id) DO UPDATE SET
                   nickname=excluded.nickname,
                   card=excluded.card,
                   display_name=excluded.display_name,
                   updated_at=excluded.updated_at",
                rusqlite::params![contact.id, user_id, nickname, card, display_name, updated_at],
            )
            .map_err(|err| format!("迁移 remote_im_group_members 失败，contact_id={}，user_id={user_id}，error={err}", contact.id))?;
        }
    }

    // remote_im_contact_checkpoints
    for checkpoint in &legacy.remote_im_contact_checkpoints {
        let checkpoint_json = serde_json::to_string(checkpoint)
            .map_err(|err| format!("序列化 checkpoint 失败：{err}"))?;
        tx.execute(
            "INSERT INTO remote_im_contact_checkpoints(contact_id, checkpoint_json)
             VALUES(?1, ?2)
             ON CONFLICT(contact_id) DO UPDATE SET checkpoint_json=excluded.checkpoint_json",
            rusqlite::params![checkpoint.contact_id, checkpoint_json],
        )
        .map_err(|err| format!("迁移 remote_im_contact_checkpoints 失败，contact_id={}，error={err}", checkpoint.contact_id))?;
    }

    // 缓存表
    migrate_legacy_image_text_cache(tx, &legacy.image_text_cache)?;
    migrate_legacy_pdf_text_cache(tx, &legacy.pdf_text_cache)?;
    migrate_legacy_pdf_image_cache(tx, &legacy.pdf_image_cache)?;

    Ok(())
}

/// 把旧联系人拆成 config_json（不含群成员）+ 群成员行。
fn build_remote_im_contact_rows(
    contact: &LegacyRemoteImContact,
) -> Result<(String, Vec<(String, String, String, String, Option<String>)>), String> {
    // config_json 保留完整联系人（群成员单独成表，config 中置空避免重复）
    let mut config = serde_json::to_value(contact)
        .map_err(|err| format!("序列化旧联系人失败，contact_id={}，error={err}", contact.id))?;
    if let Some(obj) = config.as_object_mut() {
        obj.insert("onebotGroupMembers".to_string(), serde_json::Value::Array(vec![]));
    }
    let config_json = serde_json::to_string(&config)
        .map_err(|err| format!("序列化旧联系人 config 失败，contact_id={}，error={err}", contact.id))?;
    let members = contact
        .onebot_group_members
        .iter()
        .map(|member| {
            (
                member.user_id.clone(),
                member.nickname.clone(),
                member.card.clone(),
                member.display_name.clone(),
                member.updated_at.clone(),
            )
        })
        .collect::<Vec<_>>();
    Ok((config_json, members))
}

fn migrate_legacy_image_text_cache(
    conn: &rusqlite::Connection,
    entries: &[serde_json::Value],
) -> Result<(), String> {
    for entry in entries {
        let hash = entry.get("hash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if hash.is_empty() {
            continue;
        }
        let model_api_id = entry.get("modelApiId").and_then(|v| v.as_str())
            .or_else(|| entry.get("visionApiId").and_then(|v| v.as_str()))
            .unwrap_or_default().to_string();
        let media_type = entry.get("mediaType").and_then(|v| v.as_str()).unwrap_or("image").to_string();
        let description = entry.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let text = entry.get("text").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let updated_at = entry.get("updatedAt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO image_text_cache(hash, model_api_id, media_type, description, text, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(hash, model_api_id, media_type, description) DO UPDATE SET
               text=excluded.text,
               updated_at=excluded.updated_at",
            rusqlite::params![hash, model_api_id, media_type, description, text, updated_at],
        )
        .map_err(|err| format!("迁移 image_text_cache 失败，error={err}"))?;
    }
    Ok(())
}

fn migrate_legacy_pdf_text_cache(
    conn: &rusqlite::Connection,
    entries: &[serde_json::Value],
) -> Result<(), String> {
    for entry in entries {
        let file_hash = entry.get("fileHash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if file_hash.is_empty() {
            continue;
        }
        let file_path = entry.get("filePath").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let file_name = entry.get("fileName").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let extracted_text = entry.get("extractedText").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let total_pages = entry.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let extracted_pages = entry.get("extractedPages").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let is_truncated = entry.get("isTruncated").and_then(|v| v.as_bool()).unwrap_or(false);
        let conversation_ids = entry.get("conversationIds").cloned().unwrap_or(serde_json::Value::Array(vec![]));
        let conversation_ids_json = serde_json::to_string(&conversation_ids).map_err(|e| format!("序列化 conversation_ids 失败：{e}"))?;
        let created_at = entry.get("createdAt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let updated_at = entry.get("updatedAt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO pdf_text_cache(file_hash, file_path, file_name, extracted_text, total_pages, extracted_pages, is_truncated, conversation_ids, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(file_hash) DO UPDATE SET
               file_path=excluded.file_path,
               file_name=excluded.file_name,
               extracted_text=excluded.extracted_text,
               total_pages=excluded.total_pages,
               extracted_pages=excluded.extracted_pages,
               is_truncated=excluded.is_truncated,
               conversation_ids=excluded.conversation_ids,
               updated_at=excluded.updated_at",
            rusqlite::params![file_hash, file_path, file_name, extracted_text, total_pages, extracted_pages, is_truncated as i64, conversation_ids_json, created_at, updated_at],
        )
        .map_err(|err| format!("迁移 pdf_text_cache 失败，error={err}"))?;
    }
    Ok(())
}

fn migrate_legacy_pdf_image_cache(
    conn: &rusqlite::Connection,
    entries: &[serde_json::Value],
) -> Result<(), String> {
    for entry in entries {
        let file_hash = entry.get("fileHash").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        if file_hash.is_empty() {
            continue;
        }
        let file_path = entry.get("filePath").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let file_name = entry.get("fileName").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let total_pages = entry.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let rendered_pages = entry.get("renderedPages").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let dpi = entry.get("dpi").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let images = entry.get("images").cloned().unwrap_or(serde_json::Value::Array(vec![]));
        let images_json = serde_json::to_string(&images).map_err(|e| format!("序列化 images 失败：{e}"))?;
        let conversation_ids = entry.get("conversationIds").cloned().unwrap_or(serde_json::Value::Array(vec![]));
        let conversation_ids_json = serde_json::to_string(&conversation_ids).map_err(|e| format!("序列化 conversation_ids 失败：{e}"))?;
        let created_at = entry.get("createdAt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let updated_at = entry.get("updatedAt").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        conn.execute(
            "INSERT INTO pdf_image_cache(file_hash, file_path, file_name, total_pages, rendered_pages, dpi, images_json, conversation_ids, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(file_hash) DO UPDATE SET
               file_path=excluded.file_path,
               file_name=excluded.file_name,
               total_pages=excluded.total_pages,
               rendered_pages=excluded.rendered_pages,
               dpi=excluded.dpi,
               images_json=excluded.images_json,
               conversation_ids=excluded.conversation_ids,
               updated_at=excluded.updated_at",
            rusqlite::params![file_hash, file_path, file_name, total_pages, rendered_pages, dpi, images_json, conversation_ids_json, created_at, updated_at],
        )
        .map_err(|err| format!("迁移 pdf_image_cache 失败，error={err}"))?;
    }
    Ok(())
}

fn migrate_legacy_window_layouts(
    tx: &rusqlite::Transaction,
    legacy: &LegacyWindowLayouts,
) -> Result<(), String> {
    for (label, layout) in &legacy.windows {
        tx.execute(
            "INSERT INTO window_layouts(window_label, width, height, x, y, maximized)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(window_label) DO UPDATE SET
               width=excluded.width,
               height=excluded.height,
               x=excluded.x,
               y=excluded.y,
               maximized=excluded.maximized",
            rusqlite::params![
                label,
                layout.width,
                layout.height,
                layout.x,
                layout.y,
                layout.maximized as i64
            ],
        )
        .map_err(|err| format!("迁移 window_layouts 失败，label={label}，error={err}"))?;
    }
    Ok(())
}

fn migrate_legacy_git_repo_history(
    tx: &rusqlite::Transaction,
    legacy: &std::collections::HashMap<String, Vec<String>>,
) -> Result<(), String> {
    for (repo_key, history) in legacy {
        let history_json = serde_json::to_string(history)
            .map_err(|err| format!("序列化 git repo history 失败：{err}"))?;
        tx.execute(
            "INSERT INTO git_repo_history(repo_key, history_json) VALUES(?1, ?2)
             ON CONFLICT(repo_key) DO UPDATE SET history_json=excluded.history_json",
            rusqlite::params![repo_key, history_json],
        )
        .map_err(|err| format!("迁移 git_repo_history 失败，repo_key={repo_key}，error={err}"))?;
    }
    Ok(())
}
