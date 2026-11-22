const ATTACHMENT_TRANSFER_CHUNK_BYTES: usize = 256 * 1024;
const ATTACHMENT_TRANSFER_WEB_MAX_BYTES: u64 = 50 * 1024 * 1024;
const ATTACHMENT_TRANSFER_IDLE_TIMEOUT_SECS: u64 = 10 * 60;
const ATTACHMENT_PREVIEW_MAX_EDGE: u32 = 512;
const ATTACHMENT_PREVIEW_MAX_BYTES: usize = 512 * 1024;
const ATTACHMENT_TRANSFER_TAURI_OWNER: &str = "tauri";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentTransferBeginInput {
    file_name: String,
    #[serde(default)]
    mime: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentTransferBeginOutput {
    transfer_id: String,
    next_offset: u64,
    chunk_size: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentTransferIdInput {
    transfer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentTransferChunkOutput {
    transfer_id: String,
    next_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentIngestLocalPathInput {
    path: String,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    mime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentReceipt {
    id: String,
    file_name: String,
    mime: String,
    size: u64,
    path: String,
    attach_as_media: bool,
    text_notice: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_data_url: Option<String>,
}

#[derive(Debug)]
struct AttachmentTransferSession {
    file_name: String,
    mime: String,
    declared_size: u64,
    received_size: u64,
    staging_path: PathBuf,
    updated_at: std::time::Instant,
    closed: bool,
}

#[derive(Clone)]
struct AttachmentTransferEntry {
    owner: String,
    session: Arc<tokio::sync::Mutex<AttachmentTransferSession>>,
}

#[derive(Default)]
struct AttachmentTransferRuntime {
    sessions: Mutex<std::collections::HashMap<String, AttachmentTransferEntry>>,
}

fn attachment_transfer_runtime() -> &'static AttachmentTransferRuntime {
    static RUNTIME: OnceLock<AttachmentTransferRuntime> = OnceLock::new();
    RUNTIME.get_or_init(AttachmentTransferRuntime::default)
}

fn attachment_transfer_error(code: &str, message: impl AsRef<str>) -> String {
    format!("{}: {}", code.trim(), message.as_ref().trim())
}

fn attachment_transfer_normalized_mime(file_name: &str, mime: &str) -> String {
    let normalized = mime.trim().to_ascii_lowercase();
    if !normalized.is_empty() {
        return normalized;
    }
    media_mime_from_path(std::path::Path::new(file_name))
        .unwrap_or("application/octet-stream")
        .to_string()
}

fn attachment_transfer_staging_dir(state: &AppState) -> PathBuf {
    workspace_downloads_dir(state).join(".attachment-staging")
}

fn attachment_transfer_create_staging_file(
    state: &AppState,
    transfer_id: &str,
) -> Result<PathBuf, String> {
    let staging_dir = attachment_transfer_staging_dir(state);
    std::fs::create_dir_all(&staging_dir).map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("创建附件暂存目录失败：{err}"),
        )
    })?;
    let staging_path = staging_dir.join(format!("{transfer_id}.part"));
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging_path)
        .map_err(|err| {
            attachment_transfer_error(
                "ATTACHMENT_IO_ERROR",
                format!("创建附件暂存文件失败：{err}"),
            )
        })?;
    Ok(staging_path)
}

fn attachment_files_equal(left: &std::path::Path, right: &std::path::Path) -> Result<bool, String> {
    let left_meta =
        std::fs::metadata(left).map_err(|err| format!("读取已有附件元数据失败：{err}"))?;
    let right_meta =
        std::fs::metadata(right).map_err(|err| format!("读取暂存附件元数据失败：{err}"))?;
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }
    let mut left_file =
        std::fs::File::open(left).map_err(|err| format!("打开已有附件失败：{err}"))?;
    let mut right_file =
        std::fs::File::open(right).map_err(|err| format!("打开暂存附件失败：{err}"))?;
    let mut left_buf = [0u8; 64 * 1024];
    let mut right_buf = [0u8; 64 * 1024];
    loop {
        let left_read = std::io::Read::read(&mut left_file, &mut left_buf)
            .map_err(|err| format!("读取已有附件失败：{err}"))?;
        let right_read = std::io::Read::read(&mut right_file, &mut right_buf)
            .map_err(|err| format!("读取暂存附件失败：{err}"))?;
        if left_read != right_read {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
        if left_buf[..left_read] != right_buf[..right_read] {
            return Ok(false);
        }
    }
}

fn attachment_chunk_matches_staging(
    staging_path: &std::path::Path,
    offset: u64,
    bytes: &[u8],
) -> Result<bool, String> {
    let mut file = std::fs::File::open(staging_path).map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("打开附件暂存文件失败：{err}"),
        )
    })?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(offset)).map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("定位附件重复分块失败：{err}"),
        )
    })?;
    let mut existing = vec![0u8; bytes.len()];
    std::io::Read::read_exact(&mut file, &mut existing).map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("读取附件重复分块失败：{err}"),
        )
    })?;
    Ok(existing == bytes)
}

fn attachment_finalize_staging_file(
    state: &AppState,
    staging_path: &std::path::Path,
    suggested_name: &str,
    mime: &str,
) -> Result<PathBuf, String> {
    let downloads_dir = workspace_downloads_dir(state);
    std::fs::create_dir_all(&downloads_dir).map_err(|err| {
        attachment_transfer_error("ATTACHMENT_IO_ERROR", format!("创建附件目录失败：{err}"))
    })?;
    let file_name = apply_download_extension_policy(suggested_name, mime);
    let target = downloads_dir.join(file_name);
    let final_target = if target.exists() {
        if attachment_files_equal(&target, staging_path)? {
            std::fs::remove_file(staging_path).map_err(|err| {
                attachment_transfer_error(
                    "ATTACHMENT_IO_ERROR",
                    format!("清理重复附件暂存文件失败：{err}"),
                )
            })?;
            return Ok(target);
        }
        let stem = target
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("attachment");
        let ext = target
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin");
        downloads_dir.join(format!("{stem}-{}.{}", Uuid::new_v4(), ext))
    } else {
        target
    };
    std::fs::rename(staging_path, &final_target).map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("提交附件暂存文件失败：{err}"),
        )
    })?;
    Ok(final_target)
}

fn attachment_preview_data_url(path: &std::path::Path, mime: &str) -> Option<String> {
    if !mime.trim().to_ascii_lowercase().starts_with("image/") {
        return None;
    }
    match local_image_read_for_display(path, ATTACHMENT_PREVIEW_MAX_EDGE) {
        Ok(render) if render.bytes.len() <= ATTACHMENT_PREVIEW_MAX_BYTES.saturating_mul(3) / 4 => {
            let data_url = format!("data:{};base64,{}", render.mime, B64.encode(render.bytes));
            if data_url.len() <= ATTACHMENT_PREVIEW_MAX_BYTES {
                Some(data_url)
            } else {
                runtime_log_debug(format!(
                    "[聊天] 跳过过大的图片预览 文件={} 预览字节={} 上限字节={}",
                    path.to_string_lossy(),
                    data_url.len(),
                    ATTACHMENT_PREVIEW_MAX_BYTES
                ));
                None
            }
        }
        Ok(render) => {
            runtime_log_debug(format!(
                "[聊天] 跳过过大的图片预览 文件={} 预览字节={} 上限字节={}",
                path.to_string_lossy(),
                render.bytes.len(),
                ATTACHMENT_PREVIEW_MAX_BYTES
            ));
            None
        }
        Err(err) => {
            runtime_log_debug(format!(
                "[聊天] 跳过无法生成的图片预览 文件={} 错误={}",
                path.to_string_lossy(),
                err
            ));
            None
        }
    }
}

fn attachment_receipt_from_saved_path(
    id: String,
    suggested_name: &str,
    mime: &str,
    size: u64,
    saved_path: PathBuf,
) -> AttachmentReceipt {
    let final_path = message_attachment_display_path(&saved_path.to_string_lossy());
    let final_file_name = saved_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(suggested_name)
        .to_string();
    let normalized_mime = attachment_transfer_normalized_mime(&final_file_name, mime);
    let attach_as_media = matches!(
        normalized_mime.as_str(),
        "application/pdf" | "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp"
    ) && size <= MAX_MULTIMODAL_BYTES as u64;
    let label = if normalized_mime.starts_with("image/") {
        "图片#1"
    } else {
        "附件#1"
    };
    AttachmentReceipt {
        id,
        file_name: final_file_name,
        mime: normalized_mime.clone(),
        size,
        path: final_path.clone(),
        attach_as_media,
        text_notice: message_attachment_notice_text(label, &final_path),
        preview_data_url: attachment_preview_data_url(&saved_path, &normalized_mime),
    }
}

fn attachment_transfer_lookup(
    transfer_id: &str,
    owner: &str,
) -> Result<AttachmentTransferEntry, String> {
    let normalized_id = transfer_id.trim();
    if normalized_id.is_empty() {
        return Err(attachment_transfer_error(
            "TRANSFER_NOT_FOUND",
            "缺少 transferId",
        ));
    }
    let sessions = attachment_transfer_runtime()
        .sessions
        .lock()
        .map_err(|_| attachment_transfer_error("ATTACHMENT_IO_ERROR", "附件传输状态锁已损坏"))?;
    let entry = sessions.get(normalized_id).cloned().ok_or_else(|| {
        attachment_transfer_error("TRANSFER_NOT_FOUND", "附件传输会话不存在或已结束")
    })?;
    if entry.owner != owner {
        return Err(attachment_transfer_error(
            "TRANSFER_OWNER_MISMATCH",
            "附件传输会话不属于当前连接",
        ));
    }
    Ok(entry)
}

fn attachment_transfer_remove_entry(transfer_id: &str, expected: &AttachmentTransferEntry) {
    let Ok(mut sessions) = attachment_transfer_runtime().sessions.lock() else {
        return;
    };
    let should_remove = sessions
        .get(transfer_id)
        .is_some_and(|current| Arc::ptr_eq(&current.session, &expected.session));
    if should_remove {
        sessions.remove(transfer_id);
    }
}

async fn attachment_transfer_remove_staging(path: PathBuf) {
    let _ = tokio::task::spawn_blocking(move || {
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    })
    .await;
}

fn attachment_transfer_session_should_expire(session: &AttachmentTransferSession) -> bool {
    if session.closed {
        return false;
    }
    session.updated_at.elapsed()
        >= std::time::Duration::from_secs(ATTACHMENT_TRANSFER_IDLE_TIMEOUT_SECS)
}

fn attachment_transfer_schedule_expiration(transfer_id: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(
                ATTACHMENT_TRANSFER_IDLE_TIMEOUT_SECS,
            ))
            .await;
            let entry = {
                let Ok(sessions) = attachment_transfer_runtime().sessions.lock() else {
                    break;
                };
                sessions.get(&transfer_id).cloned()
            };
            let Some(entry) = entry else {
                break;
            };
            let mut session = entry.session.lock().await;
            if !attachment_transfer_session_should_expire(&session) {
                continue;
            }
            session.closed = true;
            let staging_path = session.staging_path.clone();
            drop(session);
            attachment_transfer_remove_entry(&transfer_id, &entry);
            attachment_transfer_remove_staging(staging_path).await;
            runtime_log_warn(format!(
                "[聊天] 清理超时会话 传输={} 状态=完成",
                transfer_id
            ));
            break;
        }
    });
}

async fn attachment_transfer_begin_inner(
    input: AttachmentTransferBeginInput,
    state: &AppState,
    owner: &str,
    enforce_web_limit: bool,
) -> Result<AttachmentTransferBeginOutput, String> {
    let file_name = sanitize_download_file_name(input.file_name.trim());
    if input.size == 0 {
        return Err(attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            "附件内容为空",
        ));
    }
    if enforce_web_limit && input.size > ATTACHMENT_TRANSFER_WEB_MAX_BYTES {
        return Err(attachment_transfer_error(
            "FILE_TOO_LARGE",
            "文件太大，单个文件不能超过 50 MiB",
        ));
    }
    let mime = attachment_transfer_normalized_mime(&file_name, &input.mime);
    let transfer_id = Uuid::new_v4().to_string();
    let state_for_io = state.clone();
    let transfer_id_for_io = transfer_id.clone();
    let staging_path = tokio::task::spawn_blocking(move || {
        attachment_transfer_create_staging_file(&state_for_io, &transfer_id_for_io)
    })
    .await
    .map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("创建附件暂存任务异常：{err}"),
        )
    })??;
    let staging_path_for_error = staging_path.clone();
    let entry = AttachmentTransferEntry {
        owner: owner.to_string(),
        session: Arc::new(tokio::sync::Mutex::new(AttachmentTransferSession {
            file_name,
            mime,
            declared_size: input.size,
            received_size: 0,
            staging_path,
            updated_at: std::time::Instant::now(),
            closed: false,
        })),
    };
    let insert_result = attachment_transfer_runtime()
        .sessions
        .lock()
        .map_err(|_| attachment_transfer_error("ATTACHMENT_IO_ERROR", "附件传输状态锁已损坏"))
        .map(|mut sessions| {
            sessions.insert(transfer_id.clone(), entry);
        });
    if let Err(err) = insert_result {
        attachment_transfer_remove_staging(staging_path_for_error).await;
        return Err(err);
    }
    attachment_transfer_schedule_expiration(transfer_id.clone());
    Ok(AttachmentTransferBeginOutput {
        transfer_id,
        next_offset: 0,
        chunk_size: ATTACHMENT_TRANSFER_CHUNK_BYTES,
        max_bytes: enforce_web_limit.then_some(ATTACHMENT_TRANSFER_WEB_MAX_BYTES),
    })
}

async fn attachment_transfer_append_chunk_inner(
    transfer_id: &str,
    owner: &str,
    offset: u64,
    bytes: Vec<u8>,
) -> Result<AttachmentTransferChunkOutput, String> {
    if bytes.is_empty() || bytes.len() > ATTACHMENT_TRANSFER_CHUNK_BYTES {
        return Err(attachment_transfer_error(
            "INVALID_BINARY_FRAME",
            format!("附件分块大小无效：{}", bytes.len()),
        ));
    }
    let entry = attachment_transfer_lookup(transfer_id, owner)?;
    let mut session = entry.session.lock().await;
    if session.closed {
        return Err(attachment_transfer_error(
            "TRANSFER_NOT_FOUND",
            "附件传输会话已结束",
        ));
    }
    if offset < session.received_size {
        let duplicate_end = offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| attachment_transfer_error("TRANSFER_SIZE_MISMATCH", "附件大小溢出"))?;
        if duplicate_end != session.received_size {
            return Err(attachment_transfer_error(
                "TRANSFER_OFFSET_MISMATCH",
                format!(
                    "附件重复分块范围无效：received={}，offset={offset}，chunk_bytes={}",
                    session.received_size,
                    bytes.len()
                ),
            ));
        }
        let staging_path = session.staging_path.clone();
        let duplicate_bytes = bytes;
        let duplicate_matches = tokio::task::spawn_blocking(move || {
            attachment_chunk_matches_staging(&staging_path, offset, &duplicate_bytes)
        })
        .await
        .map_err(|err| {
            attachment_transfer_error(
                "ATTACHMENT_IO_ERROR",
                format!("校验附件重复分块任务异常：{err}"),
            )
        })??;
        if !duplicate_matches {
            return Err(attachment_transfer_error(
                "TRANSFER_OFFSET_MISMATCH",
                "附件重复分块内容与已落盘内容不一致",
            ));
        }
        session.updated_at = std::time::Instant::now();
        return Ok(AttachmentTransferChunkOutput {
            transfer_id: transfer_id.to_string(),
            next_offset: session.received_size,
        });
    }
    if offset != session.received_size {
        return Err(attachment_transfer_error(
            "TRANSFER_OFFSET_MISMATCH",
            format!(
                "附件分块偏移错误：expected={}，actual={offset}",
                session.received_size
            ),
        ));
    }
    let next_offset = offset
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| attachment_transfer_error("TRANSFER_SIZE_MISMATCH", "附件大小溢出"))?;
    if next_offset > session.declared_size {
        return Err(attachment_transfer_error(
            "TRANSFER_SIZE_MISMATCH",
            format!(
                "附件分块超过声明大小：declared={}，received={next_offset}",
                session.declared_size
            ),
        ));
    }
    let staging_path = session.staging_path.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&staging_path)
            .map_err(|err| {
                attachment_transfer_error(
                    "ATTACHMENT_IO_ERROR",
                    format!("打开附件暂存文件失败：{err}"),
                )
            })?;
        let result = (|| -> Result<(), String> {
            let actual_len = file
                .metadata()
                .map_err(|err| {
                    attachment_transfer_error(
                        "ATTACHMENT_IO_ERROR",
                        format!("读取附件暂存大小失败：{err}"),
                    )
                })?
                .len();
            if actual_len != offset {
                return Err(attachment_transfer_error(
                    "TRANSFER_SIZE_MISMATCH",
                    format!("附件暂存大小异常：expected={offset}，actual={actual_len}"),
                ));
            }
            std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(offset)).map_err(|err| {
                attachment_transfer_error("ATTACHMENT_IO_ERROR", format!("定位附件分块失败：{err}"))
            })?;
            std::io::Write::write_all(&mut file, &bytes).map_err(|err| {
                attachment_transfer_error("ATTACHMENT_IO_ERROR", format!("写入附件分块失败：{err}"))
            })?;
            std::io::Write::flush(&mut file).map_err(|err| {
                attachment_transfer_error("ATTACHMENT_IO_ERROR", format!("刷新附件分块失败：{err}"))
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = file.set_len(offset);
        }
        result
    })
    .await
    .map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("写入附件分块任务异常：{err}"),
        )
    })??;
    session.received_size = next_offset;
    session.updated_at = std::time::Instant::now();
    Ok(AttachmentTransferChunkOutput {
        transfer_id: transfer_id.to_string(),
        next_offset,
    })
}

async fn attachment_transfer_complete_inner(
    transfer_id: &str,
    state: &AppState,
    owner: &str,
) -> Result<AttachmentReceipt, String> {
    let entry = attachment_transfer_lookup(transfer_id, owner)?;
    let mut session = entry.session.lock().await;
    if session.closed {
        return Err(attachment_transfer_error(
            "TRANSFER_NOT_FOUND",
            "附件传输会话已结束",
        ));
    }
    if session.received_size != session.declared_size {
        return Err(attachment_transfer_error(
            "TRANSFER_SIZE_MISMATCH",
            format!(
                "附件大小不完整：declared={}，received={}",
                session.declared_size, session.received_size
            ),
        ));
    }
    session.closed = true;
    let file_name = session.file_name.clone();
    let mime = session.mime.clone();
    let size = session.received_size;
    let staging_path = session.staging_path.clone();
    drop(session);
    attachment_transfer_remove_entry(transfer_id, &entry);
    let state_for_io = state.clone();
    let transfer_id_for_io = transfer_id.to_string();
    let staging_path_for_error = staging_path.clone();
    let finalize_result = tokio::task::spawn_blocking(move || {
        let result = (|| -> Result<AttachmentReceipt, String> {
            let actual_size = std::fs::metadata(&staging_path)
                .map_err(|err| {
                    attachment_transfer_error(
                        "ATTACHMENT_IO_ERROR",
                        format!("读取附件暂存元数据失败：{err}"),
                    )
                })?
                .len();
            if actual_size != size {
                return Err(attachment_transfer_error(
                    "TRANSFER_SIZE_MISMATCH",
                    format!("附件落盘大小不一致：expected={size}，actual={actual_size}"),
                ));
            }
            let saved_path =
                attachment_finalize_staging_file(&state_for_io, &staging_path, &file_name, &mime)?;
            Ok(attachment_receipt_from_saved_path(
                transfer_id_for_io,
                &file_name,
                &mime,
                size,
                saved_path,
            ))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&staging_path);
        }
        result
    })
    .await;
    match finalize_result {
        Ok(result) => result,
        Err(err) => {
            attachment_transfer_remove_staging(staging_path_for_error).await;
            Err(attachment_transfer_error(
                "ATTACHMENT_IO_ERROR",
                format!("提交附件任务异常：{err}"),
            ))
        }
    }
}

async fn attachment_transfer_abort_inner(transfer_id: &str, owner: &str) -> Result<Value, String> {
    let entry = attachment_transfer_lookup(transfer_id, owner)?;
    let mut session = entry.session.lock().await;
    session.closed = true;
    let staging_path = session.staging_path.clone();
    drop(session);
    attachment_transfer_remove_entry(transfer_id, &entry);
    attachment_transfer_remove_staging(staging_path).await;
    Ok(serde_json::json!({ "ok": true }))
}

async fn attachment_transfer_abort_owner(owner: &str) {
    let entries = {
        let Ok(mut sessions) = attachment_transfer_runtime().sessions.lock() else {
            return;
        };
        let ids = sessions
            .iter()
            .filter_map(|(transfer_id, entry)| {
                (entry.owner == owner).then_some(transfer_id.clone())
            })
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|transfer_id| {
                sessions
                    .remove(&transfer_id)
                    .map(|entry| (transfer_id, entry))
            })
            .collect::<Vec<_>>()
    };
    for (transfer_id, entry) in entries {
        let mut session = entry.session.lock().await;
        session.closed = true;
        let staging_path = session.staging_path.clone();
        drop(session);
        attachment_transfer_remove_staging(staging_path).await;
        runtime_log_warn(format!(
            "[聊天] 连接断开后清理会话 传输={} 状态=完成",
            transfer_id
        ));
    }
}

async fn attachment_ingest_local_path_inner(
    input: AttachmentIngestLocalPathInput,
    state: &AppState,
) -> Result<AttachmentReceipt, String> {
    let path_text = input.path.trim();
    if path_text.is_empty() {
        return Err(attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            "文件路径为空",
        ));
    }
    let source_path = PathBuf::from(path_text);
    let state_for_io = state.clone();
    tokio::task::spawn_blocking(move || {
        let metadata = std::fs::metadata(&source_path).map_err(|err| {
            attachment_transfer_error("ATTACHMENT_IO_ERROR", format!("读取文件元数据失败：{err}"))
        })?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(attachment_transfer_error(
                "ATTACHMENT_IO_ERROR",
                "附件不存在、不是文件或内容为空",
            ));
        }
        let suggested_name = input
            .file_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                source_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "attachment".to_string());
        let mime = attachment_transfer_normalized_mime(
            &suggested_name,
            input.mime.as_deref().unwrap_or(""),
        );
        let transfer_id = Uuid::new_v4().to_string();
        let staging_path = attachment_transfer_create_staging_file(&state_for_io, &transfer_id)?;
        let result = (|| -> Result<AttachmentReceipt, String> {
            let copied_size = std::fs::copy(&source_path, &staging_path).map_err(|err| {
                attachment_transfer_error(
                    "ATTACHMENT_IO_ERROR",
                    format!("复制附件到暂存目录失败：{err}"),
                )
            })?;
            let saved_path = attachment_finalize_staging_file(
                &state_for_io,
                &staging_path,
                &suggested_name,
                &mime,
            )?;
            Ok(attachment_receipt_from_saved_path(
                transfer_id,
                &suggested_name,
                &mime,
                copied_size,
                saved_path,
            ))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&staging_path);
        }
        result
    })
    .await
    .map_err(|err| {
        attachment_transfer_error(
            "ATTACHMENT_IO_ERROR",
            format!("摄取本地附件任务异常：{err}"),
        )
    })?
}

fn attachment_transfer_parse_header(
    request: &tauri::ipc::Request<'_>,
    name: &str,
) -> Result<String, String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            attachment_transfer_error("INVALID_BINARY_FRAME", format!("缺少请求头 {name}"))
        })
}

#[tauri::command]
async fn attachment_transfer_begin(
    input: AttachmentTransferBeginInput,
    state: State<'_, AppState>,
) -> Result<AttachmentTransferBeginOutput, String> {
    attachment_transfer_begin_inner(input, state.inner(), ATTACHMENT_TRANSFER_TAURI_OWNER, false)
        .await
}

#[tauri::command]
async fn attachment_transfer_chunk(
    request: tauri::ipc::Request<'_>,
) -> Result<AttachmentTransferChunkOutput, String> {
    let transfer_id = attachment_transfer_parse_header(&request, "x-pai-transfer-id")?;
    let offset = attachment_transfer_parse_header(&request, "x-pai-transfer-offset")?
        .parse::<u64>()
        .map_err(|err| {
            attachment_transfer_error(
                "INVALID_BINARY_FRAME",
                format!("附件分块 offset 无效：{err}"),
            )
        })?;
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes.clone(),
        tauri::ipc::InvokeBody::Json(_) => {
            return Err(attachment_transfer_error(
                "INVALID_BINARY_FRAME",
                "附件分块必须使用原始二进制 IPC",
            ));
        }
    };
    attachment_transfer_append_chunk_inner(
        &transfer_id,
        ATTACHMENT_TRANSFER_TAURI_OWNER,
        offset,
        bytes,
    )
    .await
}

#[tauri::command]
async fn attachment_transfer_complete(
    input: AttachmentTransferIdInput,
    state: State<'_, AppState>,
) -> Result<AttachmentReceipt, String> {
    attachment_transfer_complete_inner(
        &input.transfer_id,
        state.inner(),
        ATTACHMENT_TRANSFER_TAURI_OWNER,
    )
    .await
}

#[tauri::command]
async fn attachment_transfer_abort(input: AttachmentTransferIdInput) -> Result<Value, String> {
    attachment_transfer_abort_inner(&input.transfer_id, ATTACHMENT_TRANSFER_TAURI_OWNER).await
}

#[tauri::command]
async fn attachment_ingest_local_path(
    input: AttachmentIngestLocalPathInput,
    state: State<'_, AppState>,
) -> Result<AttachmentReceipt, String> {
    attachment_ingest_local_path_inner(input, state.inner()).await
}
