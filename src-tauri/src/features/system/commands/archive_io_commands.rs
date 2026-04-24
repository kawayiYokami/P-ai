#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArchiveToFileInput {
    archive_id: String,
    format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportArchiveFileResult {
    path: String,
    archive_id: String,
    format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveExportPayload {
    version: u32,
    exported_at: String,
    archive: ConversationArchive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportArchivesFromJsonInput {
    payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportArchivesResult {
    imported_count: usize,
    replaced_count: usize,
    skipped_count: usize,
    total_count: usize,
    selected_archive_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportBatchPayload {
    archives: Vec<ConversationArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportAppDataPayload {
    archived_conversations: Vec<ConversationArchive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveImportConversationsPayload {
    conversations: Vec<Conversation>,
}

fn parse_archives_for_import(raw: &str) -> Result<Vec<ConversationArchive>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Archive payload is empty".to_string());
    }
    if let Ok(payload) = serde_json::from_str::<ArchiveExportPayload>(trimmed) {
        return Ok(vec![payload.archive]);
    }
    if let Ok(archive) = serde_json::from_str::<ConversationArchive>(trimmed) {
        return Ok(vec![archive]);
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportBatchPayload>(trimmed) {
        if !batch.archives.is_empty() {
            return Ok(batch.archives);
        }
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportAppDataPayload>(trimmed) {
        if !batch.archived_conversations.is_empty() {
            return Ok(batch.archived_conversations);
        }
    }
    if let Ok(batch) = serde_json::from_str::<ArchiveImportConversationsPayload>(trimmed) {
        let out = batch
            .conversations
            .into_iter()
            .filter(|c| !c.summary.trim().is_empty())
            .map(|c| conversation_to_archive(&c))
            .collect::<Vec<_>>();
        if !out.is_empty() {
            return Ok(out);
        }
    }
    if let Ok(list) = serde_json::from_str::<Vec<ConversationArchive>>(trimmed) {
        if !list.is_empty() {
            return Ok(list);
        }
    }
    Err("Invalid archive payload. Expected exported archive JSON.".to_string())
}

fn normalize_media_for_import(data_path: &PathBuf, mime: &str, stored: &str) -> String {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if stored_binary_ref_from_marker(trimmed).is_some() {
        let decoded = match resolve_stored_binary_base64(data_path, trimmed) {
            Ok(v) => v,
            Err(err) => {
                eprintln!(
                    "[ARCHIVE-IMPORT] resolve stored ref failed: marker={}, err={}",
                    trimmed, err
                );
                return trimmed.to_string();
            }
        };
        return match externalize_stored_binary_base64(data_path, mime, &decoded) {
            Ok(v) => v,
            Err(err) => {
                eprintln!(
                    "[ARCHIVE-IMPORT] externalize resolved media failed: marker={}, err={}",
                    trimmed, err
                );
                trimmed.to_string()
            }
        };
    }
    match externalize_stored_binary_base64(data_path, mime, trimmed) {
        Ok(v) => v,
        Err(err) => {
            eprintln!(
                "[ARCHIVE-IMPORT] externalize media base64 failed: value_prefix={}, err={}",
                trimmed.chars().take(32).collect::<String>(),
                err
            );
            trimmed.to_string()
        }
    }
}

fn normalize_archive_for_import(archive: &mut ConversationArchive, data_path: &PathBuf) {
    if archive.archive_id.trim().is_empty() {
        archive.archive_id = Uuid::new_v4().to_string();
    }
    if archive.archived_at.trim().is_empty() {
        archive.archived_at = now_iso();
    }
    archive.reason = clean_text(archive.reason.trim());
    if archive.reason.is_empty() {
        archive.reason = "import_archive".to_string();
    }
    let conversation = &mut archive.source_conversation;
    if conversation.id.trim().is_empty() {
        conversation.id = Uuid::new_v4().to_string();
    }
    conversation.title = clean_text(conversation.title.trim());
    if conversation.title.is_empty() {
        conversation.title = format!("Imported {}", archive_time_label(&archive.archived_at));
    }
    if conversation.created_at.trim().is_empty() {
        conversation.created_at = archive.archived_at.clone();
    }
    if conversation.updated_at.trim().is_empty() {
        conversation.updated_at = conversation.created_at.clone();
    }
    conversation.status = "archived".to_string();
    if conversation.last_user_at.as_ref().map(|v| v.trim().is_empty()).unwrap_or(false) {
        conversation.last_user_at = None;
    }
    if conversation
        .last_assistant_at
        .as_ref()
        .map(|v| v.trim().is_empty())
        .unwrap_or(false)
    {
        conversation.last_assistant_at = None;
    }
    for message in &mut conversation.messages {
        if message.id.trim().is_empty() {
            message.id = Uuid::new_v4().to_string();
        }
        if message.created_at.trim().is_empty() {
            message.created_at = conversation.updated_at.clone();
        }
        message.role = clean_text(message.role.trim());
        if message.role.is_empty() {
            message.role = "user".to_string();
        }
        for part in &mut message.parts {
            match part {
                MessagePart::Text { text } => {
                    *text = clean_text(text.trim());
                }
                MessagePart::Image {
                    mime,
                    bytes_base64,
                    name,
                    ..
                } => {
                    *mime = clean_text(mime.trim());
                    if mime.is_empty() {
                        *mime = "image/webp".to_string();
                    }
                    *bytes_base64 = normalize_media_for_import(data_path, mime, bytes_base64);
                    *name = name
                        .as_ref()
                        .map(|v| clean_text(v.trim()))
                        .filter(|v| !v.is_empty());
                }
                MessagePart::Audio {
                    mime,
                    bytes_base64,
                    name,
                    ..
                } => {
                    *mime = clean_text(mime.trim());
                    if mime.is_empty() {
                        *mime = "audio/webm".to_string();
                    }
                    *bytes_base64 = normalize_media_for_import(data_path, mime, bytes_base64);
                    *name = name
                        .as_ref()
                        .map(|v| clean_text(v.trim()))
                        .filter(|v| !v.is_empty());
                }
            }
        }
        message
            .extra_text_blocks
            .iter_mut()
            .for_each(|text| *text = clean_text(text.trim()));
        message.extra_text_blocks.retain(|text| !text.is_empty());
    }
}

fn archive_message_plain_text(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.trim().to_string()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn archive_message_image_count(message: &ChatMessage) -> usize {
    message
        .parts
        .iter()
        .filter(|part| matches!(part, MessagePart::Image { .. }))
        .count()
}

fn archive_message_audio_count(message: &ChatMessage) -> usize {
    message
        .parts
        .iter()
        .filter(|part| matches!(part, MessagePart::Audio { .. }))
        .count()
}

fn tool_call_markdown_lines(message: &ChatMessage) -> Vec<String> {
    tool_history_markdown_lines_from_message(message)
}

fn archive_message_markdown_block(message: &ChatMessage) -> String {
    let role = match message.role.as_str() {
        "user" => "用户",
        "assistant" => "助手",
        "tool" => "工具",
        other => other,
    };
    let mut lines = Vec::<String>::new();
    lines.push(format!("### {}  {}", role, message.created_at));

    let text = archive_message_plain_text(message);
    if !text.is_empty() {
        lines.push(text);
    }

    let image_count = archive_message_image_count(message);
    if image_count > 0 {
        lines.push(format!("- 图片 x{image_count}"));
    }
    let audio_count = archive_message_audio_count(message);
    if audio_count > 0 {
        lines.push(format!("- 音频 x{audio_count}"));
    }

    for line in tool_call_markdown_lines(message) {
        lines.push(line);
    }

    if lines.len() == 1 {
        lines.push("- (空消息)".to_string());
    }
    lines.join("\n")
}

fn build_archive_markdown(archive: &ConversationArchive) -> String {
    let mut blocks = Vec::<String>::new();
    blocks.push("# 对话归档".to_string());
    blocks.push(format!("- 标题: {}", archive.source_conversation.title));
    blocks.push(format!("- 归档时间: {}", archive.archived_at));
    if !archive.source_conversation.summary.trim().is_empty() {
        blocks.push(String::new());
        blocks.push("## 摘要".to_string());
        blocks.push(archive.source_conversation.summary.trim().to_string());
    }
    blocks.push(String::new());
    blocks.push("## 消息时间线".to_string());
    for message in &archive.source_conversation.messages {
        let role = message.role.as_str();
        if role != "user" && role != "assistant" && role != "tool" {
            continue;
        }
        blocks.push(String::new());
        blocks.push(archive_message_markdown_block(message));
    }
    blocks.join("\n")
}

#[tauri::command]
fn export_archive_to_file(
    input: ExportArchiveToFileInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ExportArchiveFileResult, String> {
    if input.archive_id.trim().is_empty() {
        return Err("archiveId is required".to_string());
    }
    let export_format = match input.format.trim().to_ascii_lowercase().as_str() {
        "json" => "json",
        "markdown" | "md" => "markdown",
        _ => return Err("Unsupported export format. Use 'json' or 'markdown'.".to_string()),
    };

    let data = state_read_app_data_cached(&state)?;

    let archive = data
        .conversations
        .iter()
        .find(|c| c.id == input.archive_id && !c.summary.trim().is_empty())
        .cloned()
        .ok_or_else(|| "Archive not found".to_string())?;
    let mut archive = conversation_to_archive(&archive);
    if export_format == "json" {
        materialize_chat_message_parts_from_media_refs(
            &mut archive.source_conversation.messages,
            &state.data_path,
        );
    }

    let selected = if export_format == "json" {
        app.dialog()
            .file()
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    } else {
        app.dialog()
            .file()
            .add_filter("Markdown", &["md", "markdown"])
            .blocking_save_file()
    };

    let file_path = selected
        .and_then(|fp| fp.as_path().map(ToOwned::to_owned))
        .ok_or_else(|| "Export cancelled".to_string())?;

    let body = if export_format == "json" {
        let payload = ArchiveExportPayload {
            version: 1,
            exported_at: now_iso(),
            archive: archive.clone(),
        };
        serde_json::to_string_pretty(&payload)
            .map_err(|err| format!("Serialize archive export failed: {err}"))?
    } else {
        build_archive_markdown(&archive)
    };

    fs::write(&file_path, body).map_err(|err| format!("Write export file failed: {err}"))?;

    Ok(ExportArchiveFileResult {
        path: file_path.to_string_lossy().to_string(),
        archive_id: archive.archive_id,
        format: export_format.to_string(),
    })
}

#[tauri::command]
fn import_archives_from_json(
    input: ImportArchivesFromJsonInput,
    state: State<'_, AppState>,
) -> Result<ImportArchivesResult, String> {
    let mut incoming_archives = parse_archives_for_import(&input.payload_json)?;
    if incoming_archives.is_empty() {
        return Err("No archives found in payload.".to_string());
    }

    let result = conversation_service().import_archives(state.inner(), &mut incoming_archives)?;
    Ok(ImportArchivesResult {
        imported_count: result.imported_count,
        replaced_count: result.replaced_count,
        skipped_count: result.skipped_count,
        total_count: result.total_count,
        selected_archive_id: result.selected_archive_id,
    })
}


