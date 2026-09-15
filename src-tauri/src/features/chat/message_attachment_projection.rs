const PROMPT_MESSAGE_ABSTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
struct MessageProjectionContext {
    current_agent_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptMessageAbstract {
    schema_version: u32,
    message_id: String,
    role: String,
    parts: Vec<PromptMessageAbstractPart>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
enum PromptMessageAbstractPart {
    Text {
        text: String,
    },
    Attachment {
        kind: String,
        label: String,
        path: String,
        mime: String,
        name: String,
        notice_text: String,
        available: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_description: Option<String>,
    },
}

#[derive(Debug, Clone)]
struct AttachmentProjectionWarning {
    message_id: String,
    part_index: usize,
    detail: String,
}

#[derive(Debug, Clone)]
struct PromptMessageProjectionOutcome {
    message: PromptMessageAbstract,
    warnings: Vec<AttachmentProjectionWarning>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MaterializedPromptMessage {
    message_id: String,
    role: String,
    parts: Vec<MaterializedPromptMessagePart>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum MaterializedPromptMessagePart {
    Text {
        text: String,
    },
    Attachment {
        kind: String,
        label: String,
        path: String,
        mime: String,
        name: String,
        notice_text: String,
        available: bool,
        image_description: Option<String>,
        content_base64: Option<String>,
        materialization_error: Option<String>,
    },
}

fn message_attachment_kind(mime: &str) -> &'static str {
    let normalized = mime.trim().to_ascii_lowercase();
    if normalized.starts_with("image/") {
        "image"
    } else if normalized.starts_with("audio/") {
        "audio"
    } else if normalized == "application/pdf" {
        "pdf"
    } else {
        "file"
    }
}

fn message_attachment_display_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn message_attachment_notice_text(label: &str, path: &str) -> String {
    format!(
        "[{}]\npath: {}",
        label.trim(),
        message_attachment_display_path(path)
    )
}

fn prompt_projection_role(message: &ChatMessage, context: &MessageProjectionContext) -> String {
    let role = message.role.trim().to_ascii_lowercase();
    if !matches!(role.as_str(), "user" | "assistant") {
        return role;
    }
    let speaker_agent_id = message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if speaker_agent_id == Some(context.current_agent_id.trim()) {
        "assistant".to_string()
    } else {
        "user".to_string()
    }
}

fn project_message_attachments(
    message: &ChatMessage,
    context: &MessageProjectionContext,
) -> PromptMessageProjectionOutcome {
    let mut image_index = 0usize;
    let mut attachment_index = 0usize;
    let mut parts = Vec::<PromptMessageAbstractPart>::new();
    let mut warnings = Vec::<AttachmentProjectionWarning>::new();
    for (part_index, part) in message.parts.iter().enumerate() {
        match part {
            MessagePart::Text { text, .. } => {
                parts.push(PromptMessageAbstractPart::Text { text: text.clone() });
            }
            MessagePart::Attachment { path, mime, name } => {
                let kind = message_attachment_kind(mime).to_string();
                let label = if kind == "image" {
                    image_index += 1;
                    format!("图片#{}", image_index)
                } else {
                    attachment_index += 1;
                    format!("附件#{}", attachment_index)
                };
                let normalized_path = message_attachment_display_path(path);
                let is_absolute = std::path::Path::new(path.trim()).is_absolute();
                let available = is_absolute && std::path::Path::new(path.trim()).is_file();
                if !is_absolute {
                    warnings.push(AttachmentProjectionWarning {
                        message_id: message.id.clone(),
                        part_index,
                        detail: format!("附件路径不是绝对路径：{}", normalized_path),
                    });
                } else if !available {
                    warnings.push(AttachmentProjectionWarning {
                        message_id: message.id.clone(),
                        part_index,
                        detail: format!("附件文件不可用：{}", normalized_path),
                    });
                }
                parts.push(PromptMessageAbstractPart::Attachment {
                    kind,
                    label: label.clone(),
                    path: normalized_path.clone(),
                    mime: mime.trim().to_ascii_lowercase(),
                    name: name.trim().to_string(),
                    notice_text: message_attachment_notice_text(&label, &normalized_path),
                    available,
                    image_description: None,
                });
            }
            MessagePart::Image { mime, name, .. } | MessagePart::Audio { mime, name, .. } => {
                let kind = message_attachment_kind(mime).to_string();
                let label = if kind == "image" {
                    image_index += 1;
                    format!("图片#{}", image_index)
                } else {
                    attachment_index += 1;
                    format!("附件#{}", attachment_index)
                };
                warnings.push(AttachmentProjectionWarning {
                    message_id: message.id.clone(),
                    part_index,
                    detail: "旧媒体 part 尚未经过带上下文的兼容解析".to_string(),
                });
                parts.push(PromptMessageAbstractPart::Attachment {
                    kind,
                    label: label.clone(),
                    path: String::new(),
                    mime: mime.trim().to_ascii_lowercase(),
                    name: name.clone().unwrap_or_else(|| "attachment".to_string()),
                    notice_text: message_attachment_notice_text(&label, "[旧附件路径不可用]"),
                    available: false,
                    image_description: None,
                });
            }
        }
    }
    PromptMessageProjectionOutcome {
        message: PromptMessageAbstract {
            schema_version: PROMPT_MESSAGE_ABSTRACT_SCHEMA_VERSION,
            message_id: message.id.clone(),
            role: prompt_projection_role(message, context),
            parts,
        },
        warnings,
    }
}

fn materialize_prompt_message_attachments(
    projected: &PromptMessageAbstract,
) -> MaterializedPromptMessage {
    let mut parts = Vec::<MaterializedPromptMessagePart>::new();
    for part in &projected.parts {
        match part {
            PromptMessageAbstractPart::Text { text } => {
                parts.push(MaterializedPromptMessagePart::Text { text: text.clone() });
            }
            PromptMessageAbstractPart::Attachment {
                kind,
                label,
                path,
                mime,
                name,
                notice_text,
                available,
                image_description,
            } => {
                let read_result = if *available {
                    std::fs::read(path).map_err(|err| format!("读取附件失败：{err}"))
                } else {
                    Err("附件不可用".to_string())
                };
                let (content_base64, materialization_error, materialized_available) =
                    match read_result {
                        Ok(raw) => (Some(B64.encode(raw)), None, true),
                        Err(err) => {
                            runtime_log_warn(format!(
                                "[附件投影] 二进制物化跳过，message_id={}，label={}，path={}，error={}",
                                projected.message_id, label, path, err
                            ));
                            (None, Some(err), false)
                        }
                    };
                parts.push(MaterializedPromptMessagePart::Attachment {
                    kind: kind.clone(),
                    label: label.clone(),
                    path: path.clone(),
                    mime: mime.clone(),
                    name: name.clone(),
                    notice_text: notice_text.clone(),
                    available: materialized_available,
                    image_description: image_description.clone(),
                    content_base64,
                    materialization_error,
                });
            }
        }
    }
    MaterializedPromptMessage {
        message_id: projected.message_id.clone(),
        role: projected.role.clone(),
        parts,
    }
}

fn render_prompt_message_abstract_user_text(projected: &PromptMessageAbstract) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &projected.parts {
        match part {
            PromptMessageAbstractPart::Text { text } => {
                if !text.trim().is_empty() {
                    chunks.push(text.trim().to_string());
                }
            }
            PromptMessageAbstractPart::Attachment {
                path,
                notice_text,
                image_description,
                ..
            } => {
                if !path.trim().is_empty() && !notice_text.trim().is_empty() {
                    chunks.push(notice_text.trim().to_string());
                }
                if let Some(description) = image_description
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    chunks.push(description.to_string());
                }
            }
        }
    }
    chunks.join("\n\n")
}

#[cfg(test)]
fn prompt_message_abstract_set_image_description(
    projected: &mut PromptMessageAbstract,
    label: &str,
    description: &str,
) -> bool {
    let normalized_label = label.trim();
    let normalized_description = description.trim();
    if normalized_label.is_empty() || normalized_description.is_empty() {
        return false;
    }
    for part in &mut projected.parts {
        if let PromptMessageAbstractPart::Attachment {
            kind,
            label,
            image_description,
            ..
        } = part
        {
            if kind == "image" && label.trim() == normalized_label {
                *image_description = Some(format!(
                    "[{} 图片转文]\n{}",
                    normalized_label, normalized_description
                ));
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone)]
struct AttachmentIngressInput {
    path: Option<String>,
    bytes_base64: Option<String>,
    mime: String,
    name: String,
    storage_subdir: Option<String>,
}

#[derive(Debug, Clone)]
struct AttachmentIngressOutcome {
    part: Option<MessagePart>,
    warnings: Vec<String>,
}

fn attachment_path_is_legacy_marker_or_url(path: &str) -> bool {
    let normalized = path.trim().to_ascii_lowercase();
    normalized.starts_with("@media:")
        || normalized.starts_with("@download:")
        || normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("data:")
}

fn attachment_absolute_path_from_input(state: &AppState, path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() || attachment_path_is_legacy_marker_or_url(trimmed) {
        return None;
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return Some(candidate);
    }
    let workspace_root = configured_workspace_root_path(state)
        .unwrap_or_else(|_| state.llm_workspace_path.clone());
    Some(workspace_root.join(candidate))
}

fn attachment_name_from_path_or_input(path: &std::path::Path, name: &str) -> String {
    let normalized_name = name.trim();
    if !normalized_name.is_empty() {
        return normalized_name.to_string();
    }
    path.file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("attachment")
        .to_string()
}

fn attachment_mime_from_input(path: Option<&std::path::Path>, mime: &str, raw: Option<&[u8]>) -> String {
    let normalized = mime.trim().to_ascii_lowercase();
    if !normalized.is_empty() {
        return normalized;
    }
    if let Some(raw) = raw {
        if let Some(kind) = infer::get(raw) {
            return kind.mime_type().to_string();
        }
    }
    path.and_then(media_mime_from_path)
        .unwrap_or("application/octet-stream")
        .to_string()
}

fn normalize_attachment_ingress(
    state: &AppState,
    input: AttachmentIngressInput,
) -> AttachmentIngressOutcome {
    let mut warnings = Vec::<String>::new();
    let path_input = input
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let base64_input = input
        .bytes_base64
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if path_input.is_some() && base64_input.is_some() {
        warnings.push("附件同时携带 path 与 base64，优先复用已落盘 path，忽略重复二进制".to_string());
    }

    if let Some(path_input) = path_input {
        if let Some(path) = attachment_absolute_path_from_input(state, path_input) {
            let mime = attachment_mime_from_input(Some(&path), &input.mime, None);
            let name = attachment_name_from_path_or_input(&path, &input.name);
            if !PathBuf::from(path_input).is_absolute() {
                warnings.push(format!(
                    "兼容相对附件路径并转换为绝对路径：{}",
                    path.to_string_lossy()
                ));
            }
            if !path.is_file() {
                warnings.push(format!("附件路径当前不可用：{}", path.to_string_lossy()));
            }
            return AttachmentIngressOutcome {
                part: Some(MessagePart::Attachment {
                    path: message_attachment_display_path(&path.to_string_lossy()),
                    mime,
                    name,
                }),
                warnings,
            };
        }
        warnings.push(format!("拒绝模糊附件路径表示：{path_input}"));
    }

    let Some(base64_input) = base64_input else {
        warnings.push("附件缺少可用的绝对 path 或原始内容".to_string());
        return AttachmentIngressOutcome {
            part: None,
            warnings,
        };
    };
    let raw = match B64.decode(base64_input) {
        Ok(raw) if !raw.is_empty() => raw,
        Ok(_) => {
            warnings.push("附件原始内容为空".to_string());
            return AttachmentIngressOutcome {
                part: None,
                warnings,
            };
        }
        Err(err) => {
            warnings.push(format!("附件 base64 解码失败：{err}"));
            return AttachmentIngressOutcome {
                part: None,
                warnings,
            };
        }
    };
    let mime = attachment_mime_from_input(None, &input.mime, Some(&raw));
    let suggested_name = if input.name.trim().is_empty() {
        format!("attachment-{}.{}", Uuid::new_v4(), media_extension_from_mime_for_download(&mime))
    } else {
        input.name.trim().to_string()
    };
    match persist_raw_attachment_to_downloads_subdir(
        state,
        input.storage_subdir.as_deref(),
        &suggested_name,
        &mime,
        &raw,
    ) {
        Ok(path) => AttachmentIngressOutcome {
            part: Some(MessagePart::Attachment {
                path: message_attachment_display_path(&path.to_string_lossy()),
                mime,
                name: attachment_name_from_path_or_input(&path, &suggested_name),
            }),
            warnings,
        },
        Err(err) => {
            warnings.push(format!("附件落盘失败，已跳过该附件：{err}"));
            AttachmentIngressOutcome {
                part: None,
                warnings,
            }
        }
    }
}

fn push_normalized_attachment_ingress(
    state: &AppState,
    input: AttachmentIngressInput,
    parts: &mut Vec<MessagePart>,
    warnings: &mut Vec<String>,
) {
    let display_name = input.name.trim().to_string();
    let outcome = normalize_attachment_ingress(state, input);
    let fallback_reason = outcome.warnings.last().cloned();
    warnings.extend(outcome.warnings);
    if let Some(part) = outcome.part {
        parts.push(part);
    } else {
        let name = if display_name.is_empty() {
            "附件"
        } else {
            display_name.as_str()
        };
        let reason = fallback_reason.unwrap_or_else(|| "未知附件错误".to_string());
        parts.push(MessagePart::Text {
            text: format!(
                "[附件不可用：{} 未能完成规范化，已跳过该附件并继续。原因：{}]",
                name, reason
            ),
            reasoning_content: None,
        });
    }
}

fn normalize_chat_input_payload_to_message_parts(
    state: &AppState,
    payload: &ChatInputPayload,
    storage_subdir: Option<&str>,
) -> (Vec<MessagePart>, Vec<String>) {
    let mut parts = Vec::<MessagePart>::new();
    let mut warnings = Vec::<String>::new();
    let storage_subdir = storage_subdir.map(ToOwned::to_owned);
    if let Some(ordered_parts) = payload.parts.as_ref().filter(|items| !items.is_empty()) {
        for part in ordered_parts {
            match part {
                ChatIngressPart::Text { text } => {
                    if !text.trim().is_empty() {
                        parts.push(MessagePart::Text {
                            text: text.trim().to_string(),
                            reasoning_content: None,
                        });
                    }
                }
                ChatIngressPart::Attachment {
                    path,
                    bytes_base64,
                    mime,
                    name,
                } => push_normalized_attachment_ingress(
                    state,
                    AttachmentIngressInput {
                        path: path.clone(),
                        bytes_base64: bytes_base64.clone(),
                        mime: mime.clone(),
                        name: name.clone(),
                        storage_subdir: storage_subdir.clone(),
                    },
                    &mut parts,
                    &mut warnings,
                ),
            }
        }
        return (parts, warnings);
    }

    if let Some(text) = payload
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(MessagePart::Text {
            text: text.to_string(),
            reasoning_content: None,
        });
    }
    for image in payload.images.as_deref().unwrap_or_default() {
        push_normalized_attachment_ingress(
            state,
            AttachmentIngressInput {
                path: image.saved_path.clone(),
                bytes_base64: Some(image.bytes_base64.clone()),
                mime: image.mime.clone(),
                name: image
                    .saved_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).file_name())
                    .and_then(|value| value.to_str())
                    .unwrap_or("image")
                    .to_string(),
                storage_subdir: storage_subdir.clone(),
            },
            &mut parts,
            &mut warnings,
        );
    }
    for audio in payload.audios.as_deref().unwrap_or_default() {
        push_normalized_attachment_ingress(
            state,
            AttachmentIngressInput {
                path: audio.saved_path.clone(),
                bytes_base64: Some(audio.bytes_base64.clone()),
                mime: audio.mime.clone(),
                name: audio
                    .saved_path
                    .as_deref()
                    .and_then(|path| std::path::Path::new(path).file_name())
                    .and_then(|value| value.to_str())
                    .unwrap_or("audio")
                    .to_string(),
                storage_subdir: storage_subdir.clone(),
            },
            &mut parts,
            &mut warnings,
        );
    }
    for attachment in payload.attachments.as_deref().unwrap_or_default() {
        push_normalized_attachment_ingress(
            state,
            AttachmentIngressInput {
                path: Some(attachment.path.clone()),
                bytes_base64: None,
                mime: attachment.mime.clone(),
                name: attachment.file_name.clone(),
                storage_subdir: storage_subdir.clone(),
            },
            &mut parts,
            &mut warnings,
        );
    }
    (parts, warnings)
}

fn chat_input_payload_has_content(payload: &ChatInputPayload) -> bool {
    payload
        .parts
        .as_ref()
        .map(|parts| {
            parts.iter().any(|part| match part {
                ChatIngressPart::Text { text } => !text.trim().is_empty(),
                ChatIngressPart::Attachment {
                    path,
                    bytes_base64,
                    ..
                } => {
                    path.as_deref().map(str::trim).is_some_and(|value| !value.is_empty())
                        || bytes_base64
                            .as_deref()
                            .map(str::trim)
                            .is_some_and(|value| !value.is_empty())
                }
            })
        })
        .unwrap_or(false)
        || payload
            .text
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        || payload.images.as_ref().is_some_and(|items| !items.is_empty())
        || payload.audios.as_ref().is_some_and(|items| !items.is_empty())
        || payload
            .attachments
            .as_ref()
            .is_some_and(|items| !items.is_empty())
}

fn legacy_attachment_unavailable_candidate_path(
    data_path: &PathBuf,
    mime: &str,
    source: &str,
) -> PathBuf {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let hash = bytes_to_lower_hex(hasher.finalize());
    media_storage_dir_from_data_path(data_path)
        .unwrap_or_else(|_| app_root_from_data_path(data_path).join("media"))
        .join("legacy-unavailable")
        .join(format!(
            "{}.{}",
            hash,
            media_extension_from_mime_for_download(mime)
        ))
}

fn canonical_attachment_path_for_persistence(
    data_path: &PathBuf,
    path: &str,
    mime: &str,
) -> (PathBuf, Option<String>) {
    let trimmed = path.trim();
    if let Some((kind, stored_id)) = stored_binary_ref_from_marker(trimmed) {
        let root = match kind {
            StoredBinaryRefKind::Media => media_storage_dir_from_data_path(data_path)
                .unwrap_or_else(|_| app_root_from_data_path(data_path).join("media")),
            StoredBinaryRefKind::Download => downloads_storage_dir_from_data_path(data_path)
                .unwrap_or_else(|_| app_root_from_data_path(data_path).join("llm-workspace/downloads")),
        };
        return (
            root.join(stored_id.trim()),
            Some("旧附件 marker 已词法转换为 canonical 绝对路径".to_string()),
        );
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        return (candidate, None);
    }
    if !trimmed.is_empty() && !attachment_path_is_legacy_marker_or_url(trimmed) {
        return (
            app_root_from_data_path(data_path)
                .join("llm-workspace")
                .join(trimmed.replace('\\', "/")),
            Some("旧附件相对路径已按 workspace 根目录词法绝对化".to_string()),
        );
    }
    (
        legacy_attachment_unavailable_candidate_path(data_path, mime, trimmed),
        Some("附件路径为空或为模糊表示，已保留不可用绝对候选路径".to_string()),
    )
}

fn legacy_binary_message_part_to_attachment(
    data_path: &PathBuf,
    mime: &str,
    stored: &str,
    name: Option<&str>,
) -> (MessagePart, Option<String>) {
    let normalized_mime = if mime.trim().is_empty() {
        "application/octet-stream".to_string()
    } else {
        mime.trim().to_ascii_lowercase()
    };
    let trimmed = stored.trim();
    let (path, warning) = if let Some((kind, stored_id)) = stored_binary_ref_from_marker(trimmed) {
        let base = match kind {
            StoredBinaryRefKind::Media => media_storage_dir_from_data_path(data_path)
                .unwrap_or_else(|_| app_root_from_data_path(data_path).join("media")),
            StoredBinaryRefKind::Download => downloads_storage_dir_from_data_path(data_path)
                .unwrap_or_else(|_| app_root_from_data_path(data_path).join("llm-workspace/downloads")),
        };
        (base.join(stored_id.trim()), None)
    } else if PathBuf::from(trimmed).is_absolute() {
        (PathBuf::from(trimmed), None)
    } else if !trimmed.is_empty()
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
        && (trimmed.contains('/') || trimmed.contains('\\'))
    {
        let path = downloads_storage_dir_from_data_path(data_path)
            .unwrap_or_else(|_| app_root_from_data_path(data_path).join("llm-workspace/downloads"))
            .join(trimmed.replace('\\', "/"));
        (
            path,
            Some("旧附件相对路径已按 downloads 根目录词法绝对化".to_string()),
        )
    } else {
        match B64.decode(trimmed) {
            Ok(raw) if !raw.is_empty() => match persist_media_bytes(data_path, &normalized_mime, &raw) {
                Ok(media_id) => (
                    media_storage_dir_from_data_path(data_path)
                        .unwrap_or_else(|_| app_root_from_data_path(data_path).join("media"))
                        .join(media_id),
                    Some("旧内联媒体已迁移为 canonical 绝对路径".to_string()),
                ),
                Err(err) => (
                    legacy_attachment_unavailable_candidate_path(
                        data_path,
                        &normalized_mime,
                        trimmed,
                    ),
                    Some(format!("旧媒体落盘失败，保留不可用绝对候选路径：{err}")),
                ),
            },
            Ok(_) => (
                legacy_attachment_unavailable_candidate_path(
                    data_path,
                    &normalized_mime,
                    trimmed,
                ),
                Some("旧媒体内容为空，保留不可用绝对候选路径".to_string()),
            ),
            Err(err) => (
                legacy_attachment_unavailable_candidate_path(
                    data_path,
                    &normalized_mime,
                    trimmed,
                ),
                Some(format!("旧媒体内容无法解析，保留不可用绝对候选路径：{err}")),
            ),
        }
    };
    let normalized_name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "attachment".to_string());
    (
        MessagePart::Attachment {
            path: message_attachment_display_path(&path.to_string_lossy()),
            mime: normalized_mime,
            name: normalized_name,
        },
        warning,
    )
}

fn canonicalize_message_parts_for_persistence(
    parts: &mut Vec<MessagePart>,
    data_path: &PathBuf,
) -> bool {
    let mut changed = false;
    let mut next = Vec::<MessagePart>::with_capacity(parts.len());
    for part in std::mem::take(parts) {
        match part {
            MessagePart::Image {
                mime,
                bytes_base64,
                name,
                ..
            }
            | MessagePart::Audio {
                mime,
                bytes_base64,
                name,
                ..
            } => {
                let (attachment, warning) = legacy_binary_message_part_to_attachment(
                    data_path,
                    &mime,
                    &bytes_base64,
                    name.as_deref(),
                );
                if let Some(warning) = warning {
                    runtime_log_warn(format!("[附件迁移] 降级继续：{warning}"));
                }
                next.push(attachment);
                changed = true;
            }
            MessagePart::Attachment { path, mime, name } => {
                let normalized_mime = if mime.trim().is_empty() {
                    "application/octet-stream".to_string()
                } else {
                    mime.trim().to_ascii_lowercase()
                };
                let (absolute_path, warning) = canonical_attachment_path_for_persistence(
                    data_path,
                    &path,
                    &normalized_mime,
                );
                if let Some(warning) = warning {
                    runtime_log_warn(format!("[附件迁移] 降级继续：{warning}，path={path}"));
                }
                let normalized_path = message_attachment_display_path(&absolute_path.to_string_lossy());
                let normalized_name = attachment_name_from_path_or_input(&absolute_path, &name);
                changed |= normalized_path != path
                    || normalized_mime != mime
                    || normalized_name != name;
                next.push(MessagePart::Attachment {
                    path: normalized_path,
                    mime: normalized_mime,
                    name: normalized_name,
                });
            }
            MessagePart::Text { text, reasoning_content } => next.push(MessagePart::Text {
                text,
                reasoning_content,
            }),
        }
    }
    *parts = next;
    changed
}

#[cfg(test)]
mod message_attachment_projection_tests {
    use super::*;

    fn test_message(parts: Vec<MessagePart>) -> ChatMessage {
        ChatMessage {
            id: "message-projection".to_string(),
            role: "user".to_string(),
            created_at: "2026-07-18T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts,
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    #[test]
    fn projector_should_keep_part_order_labels_and_absolute_paths() {
        let root = std::env::temp_dir().join(format!("eca-projector-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let image_a = root.join("a.png");
        let pdf = root.join("report.pdf");
        let image_b = root.join("b.png");
        std::fs::write(&image_a, b"image-a").expect("write image a");
        std::fs::write(&pdf, b"pdf").expect("write pdf");
        std::fs::write(&image_b, b"image-b").expect("write image b");
        let message = test_message(vec![
            MessagePart::Text {
                text: "帮我看看".to_string(),
                reasoning_content: None,
            },
            MessagePart::Attachment {
                path: image_a.to_string_lossy().to_string(),
                mime: "image/png".to_string(),
                name: "a.png".to_string(),
            },
            MessagePart::Attachment {
                path: pdf.to_string_lossy().to_string(),
                mime: "application/pdf".to_string(),
                name: "report.pdf".to_string(),
            },
            MessagePart::Attachment {
                path: image_b.to_string_lossy().to_string(),
                mime: "image/png".to_string(),
                name: "b.png".to_string(),
            },
        ]);

        let outcome = project_message_attachments(
            &message,
            &MessageProjectionContext {
                current_agent_id: "agent-a".to_string(),
            },
        );

        assert!(outcome.warnings.is_empty());
        let json = serde_json::to_value(&outcome.message).expect("serialize projection");
        let parts = json["parts"].as_array().expect("parts");
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["label"], "图片#1");
        assert_eq!(parts[2]["label"], "附件#1");
        assert_eq!(parts[3]["label"], "图片#2");
        assert_eq!(
            parts[1]["noticeText"],
            format!("[图片#1]\npath: {}", message_attachment_display_path(&image_a.to_string_lossy()))
        );
        assert!(json.to_string().contains("schemaVersion"));
        assert!(!json.to_string().contains("contentBase64"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn projector_should_keep_missing_absolute_path_as_unavailable() {
        let path = std::env::temp_dir()
            .join(format!("missing-{}.png", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let outcome = project_message_attachments(
            &test_message(vec![MessagePart::Attachment {
                path: path.clone(),
                mime: "image/png".to_string(),
                name: "missing.png".to_string(),
            }]),
            &MessageProjectionContext {
                current_agent_id: "agent-a".to_string(),
            },
        );

        assert_eq!(outcome.warnings.len(), 1);
        let json = serde_json::to_value(&outcome.message).expect("serialize projection");
        assert_eq!(json["parts"][0]["available"], false);
        assert_eq!(json["parts"][0]["path"], message_attachment_display_path(&path));
    }

    #[test]
    fn projector_should_use_explicit_current_agent_for_role() {
        let mut message = test_message(vec![MessagePart::Text {
            text: "协作消息".to_string(),
            reasoning_content: None,
        }]);
        message.role = "assistant".to_string();
        message.speaker_agent_id = Some("agent-b".to_string());

        let other = project_message_attachments(
            &message,
            &MessageProjectionContext {
                current_agent_id: "agent-a".to_string(),
            },
        );
        let own = project_message_attachments(
            &message,
            &MessageProjectionContext {
                current_agent_id: "agent-b".to_string(),
            },
        );

        assert_eq!(other.message.role, "user");
        assert_eq!(own.message.role, "assistant");
    }

    #[test]
    fn image_description_should_share_label_and_never_repeat_path() {
        let path = "C:/attachments/a.png";
        let mut projected = project_message_attachments(
            &test_message(vec![MessagePart::Attachment {
                path: path.to_string(),
                mime: "image/png".to_string(),
                name: "a.png".to_string(),
            }]),
            &MessageProjectionContext {
                current_agent_id: "agent-a".to_string(),
            },
        )
        .message;

        assert!(prompt_message_abstract_set_image_description(
            &mut projected,
            "图片#1",
            "一只猫"
        ));
        let rendered = render_prompt_message_abstract_user_text(&projected);

        assert!(rendered.contains("[图片#1]\npath: C:/attachments/a.png"));
        assert!(rendered.contains("[图片#1 图片转文]\n一只猫"));
        assert_eq!(rendered.matches("path:").count(), 1);
    }

    #[test]
    fn persistence_canonicalizer_should_never_keep_relative_or_url_attachment_paths() {
        let root = std::env::temp_dir().join(format!("attachment-canonical-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let data_path = root.join("config_mark");
        let mut parts = vec![
            MessagePart::Attachment {
                path: "downloads/relative.png".to_string(),
                mime: "IMAGE/PNG".to_string(),
                name: String::new(),
            },
            MessagePart::Attachment {
                path: "https://example.com/remote.png".to_string(),
                mime: "image/png".to_string(),
                name: "remote.png".to_string(),
            },
        ];

        assert!(canonicalize_message_parts_for_persistence(&mut parts, &data_path));
        for part in &parts {
            let MessagePart::Attachment { path, .. } = part else {
                panic!("expected attachment");
            };
            assert!(PathBuf::from(path).is_absolute());
            assert!(!path.starts_with("http://"));
            assert!(!path.starts_with("https://"));
        }
        let MessagePart::Attachment { path, mime, name } = &parts[0] else {
            panic!("expected attachment");
        };
        assert!(path.replace('\\', "/").ends_with("llm-workspace/downloads/relative.png"));
        assert_eq!(mime, "image/png");
        assert_eq!(name, "relative.png");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn materializer_should_fail_soft_when_file_disappears() {
        let path = std::env::temp_dir().join(format!("gone-{}.png", Uuid::new_v4()));
        std::fs::write(&path, b"image").expect("write image");
        let projected = project_message_attachments(
            &test_message(vec![MessagePart::Attachment {
                path: path.to_string_lossy().to_string(),
                mime: "image/png".to_string(),
                name: "gone.png".to_string(),
            }]),
            &MessageProjectionContext {
                current_agent_id: "agent-a".to_string(),
            },
        )
        .message;
        std::fs::remove_file(&path).expect("remove image");

        let materialized = materialize_prompt_message_attachments(&projected);

        assert_eq!(materialized.message_id, "message-projection");
        assert_eq!(materialized.role, "user");
        match materialized.parts.first().expect("attachment") {
            MaterializedPromptMessagePart::Attachment {
                available,
                content_base64,
                materialization_error,
                ..
            } => {
                assert!(!available);
                assert!(content_base64.is_none());
                assert!(materialization_error.is_some());
            }
            _ => panic!("expected attachment"),
        }
    }
}
