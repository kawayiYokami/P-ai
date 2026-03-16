fn latest_active_conversation_index(
    data: &AppData,
    _api_config_id: &str,
    _agent_id: &str,
) -> Option<usize> {
    data.conversations
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.status == "active"
                && c.summary.trim().is_empty()
                && !conversation_is_delegate(c)
        })
        .max_by(|(idx_a, a), (idx_b, b)| {
            let a_updated = a.updated_at.trim();
            let b_updated = b.updated_at.trim();
            let a_created = a.created_at.trim();
            let b_created = b.created_at.trim();
            a_updated
                .cmp(b_updated)
                .then_with(|| a_created.cmp(b_created))
                .then_with(|| idx_a.cmp(idx_b))
        })
        .map(|(idx, _)| idx)
}

fn normalize_single_active_main_conversation(data: &mut AppData) -> bool {
    let Some(keep_idx) = latest_active_conversation_index(data, "", "") else {
        return false;
    };
    let keep_id = data
        .conversations
        .get(keep_idx)
        .map(|item| item.id.clone())
        .unwrap_or_default();
    if keep_id.trim().is_empty() {
        return false;
    }
    let removable_count = data
        .conversations
        .iter()
        .filter(|conversation| {
            !conversation_is_delegate(conversation)
                && conversation.status == "active"
                && conversation.summary.trim().is_empty()
                && conversation.id != keep_id
        })
        .count();
    if removable_count == 0 {
        return false;
    }
    let before = data.conversations.len();
    data.conversations.retain(|conversation| {
        conversation_is_delegate(conversation)
            || conversation.status != "active"
            || !conversation.summary.trim().is_empty()
            || conversation.id == keep_id
    });
    let changed = data.conversations.len() != before;
    if changed {
        eprintln!(
            "[INFO][会话] 归一化未归档主会话: kept_conversation_id={}, removed_count={}",
            keep_id, removable_count
        );
    }
    changed
}

fn conversation_is_delegate(conversation: &Conversation) -> bool {
    conversation.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE
}

fn sanitize_tool_history_events(events: &[Value]) -> Vec<Value> {
    let mut sanitized = Vec::<Value>::new();
    let mut pending_assistant_index: Option<usize> = None;
    for event in events {
        let role = event
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match role.as_str() {
            "assistant" => {
                let has_tool_calls = event
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .map(|items| !items.is_empty())
                    .unwrap_or(false);
                let index = sanitized.len();
                sanitized.push(event.clone());
                pending_assistant_index = if has_tool_calls { Some(index) } else { None };
            }
            "tool" => {
                if pending_assistant_index.is_some() {
                    sanitized.push(event.clone());
                    pending_assistant_index = None;
                }
            }
            _ => {
                pending_assistant_index = None;
                sanitized.push(event.clone());
            }
        }
    }
    if let Some(index) = pending_assistant_index {
        sanitized.truncate(index);
    }
    sanitized
}

fn build_conversation_record(
    _api_config_id: &str,
    agent_id: &str,
    title: &str,
    conversation_kind: &str,
    root_conversation_id: Option<String>,
    delegate_id: Option<String>,
) -> Conversation {
    let now = now_iso();
    Conversation {
        id: Uuid::new_v4().to_string(),
        title: if title.trim().is_empty() {
            format!("Chat {}", &now.chars().take(16).collect::<String>())
        } else {
            title.trim().to_string()
        },
        agent_id: agent_id.to_string(),
        conversation_kind: conversation_kind.trim().to_string(),
        root_conversation_id,
        delegate_id,
        created_at: now.clone(),
        updated_at: now,
        last_user_at: None,
        last_assistant_at: None,
        last_context_usage_ratio: 0.0,
        last_effective_prompt_tokens: 0,
        status: "active".to_string(),
        summary: String::new(),
        archived_at: None,
        messages: Vec::new(),
        memory_recall_table: Vec::new(),
    }
}

fn ensure_active_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    let _ = normalize_single_active_main_conversation(data);
    if let Some(idx) = latest_active_conversation_index(data, api_config_id, agent_id) {
        return idx;
    }

    let conversation = build_conversation_record(
        api_config_id,
        agent_id,
        "",
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );

    data.conversations.push(conversation);
    data.conversations.len() - 1
}

#[derive(Debug, Clone)]
struct ArchiveDecision {
    should_archive: bool,
    forced: bool,
    reason: String,
    usage_ratio: f64,
}

fn estimated_tokens_for_text(text: &str) -> f64 {
    let mut zh_chars = 0usize;
    let mut other_chars = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
        {
            zh_chars += 1;
        } else {
            other_chars += 1;
        }
    }
    zh_chars as f64 * 0.6 + other_chars as f64 * 0.3
}

fn estimated_tokens_for_message(message: &ChatMessage) -> f64 {
    let mut tokens = 12.0;
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => {
                tokens += estimated_tokens_for_text(text);
            }
            MessagePart::Image { .. } => {
                tokens += 280.0;
            }
            MessagePart::Audio { .. } => {
                tokens += 320.0;
            }
        }
    }
    tokens
}

fn estimate_conversation_tokens(conversation: &Conversation) -> u32 {
    let mut sum = 0.0f64;
    for msg in &conversation.messages {
        sum += estimated_tokens_for_message(msg);
    }
    sum.ceil().max(0.0) as u32
}

fn compute_context_usage_ratio(conversation: &Conversation, context_window_tokens: u32) -> f64 {
    let max_tokens = context_window_tokens.max(1) as f64;
    (effective_prompt_tokens_for_conversation(conversation) as f64 / max_tokens).max(0.0)
}

fn effective_prompt_tokens_for_conversation(conversation: &Conversation) -> u32 {
    let last_role = conversation
        .messages
        .last()
        .map(|message| message.role.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if last_role == "assistant" && conversation.last_effective_prompt_tokens > 0 {
        return conversation.last_effective_prompt_tokens.min(u64::from(u32::MAX)) as u32;
    }
    estimate_conversation_tokens(conversation)
}

fn decide_archive_before_user_message(
    conversation: &Conversation,
    context_window_tokens: u32,
) -> ArchiveDecision {
    let usage_ratio = compute_context_usage_ratio(conversation, context_window_tokens);
    if usage_ratio >= 0.82 {
        return ArchiveDecision {
            should_archive: true,
            forced: true,
            reason: "force_context_usage_82".to_string(),
            usage_ratio,
        };
    }

    let Some(last_user_at) = conversation.last_user_at.as_deref().and_then(parse_iso) else {
        return ArchiveDecision {
            should_archive: false,
            forced: false,
            reason: "no_last_user_timestamp".to_string(),
            usage_ratio,
        };
    };

    let now = now_utc();
    let idle_seconds = now.unix_timestamp() - last_user_at.unix_timestamp();
    if idle_seconds < ARCHIVE_IDLE_SECONDS {
        return ArchiveDecision {
            should_archive: false,
            forced: false,
            reason: "idle_not_reached_30m".to_string(),
            usage_ratio,
        };
    }

    if usage_ratio >= 0.30 {
        return ArchiveDecision {
            should_archive: true,
            forced: false,
            reason: "idle_30m_and_usage_30pct".to_string(),
            usage_ratio,
        };
    }

    ArchiveDecision {
        should_archive: false,
        forced: false,
        reason: "usage_below_30pct".to_string(),
        usage_ratio,
    }
}

fn archive_conversation_now(
    data: &mut AppData,
    conversation_id: &str,
    reason: &str,
    summary: &str,
) -> Option<String> {
    let idx = data
        .conversations
        .iter()
        .position(|c| c.id == conversation_id && c.status == "active")?;
    let conv = data.conversations.get_mut(idx)?;
    let now = now_iso();
    conv.status = "archived".to_string();
    conv.summary = summary.to_string();
    conv.archived_at = Some(now.clone());
    conv.updated_at = now;
    let archive_id = conv.id.clone();
    eprintln!(
        "[会话] 已归档: conversation_id={}, reason=\"{}\", summary=\"{}\"",
        conv.id, reason, summary
    );
    clear_screenshot_artifact_cache();
    Some(archive_id)
}

fn compress_image_to_webp(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image =
        image::load_from_memory(bytes).map_err(|err| format!("Decode image failed: {err}"))?;
    let mut cursor = Cursor::new(Vec::<u8>::new());
    image
        .write_to(&mut cursor, ImageFormat::WebP)
        .map_err(|err| format!("Encode image to WebP failed: {err}"))?;
    Ok(cursor.into_inner())
}

fn is_supported_image_upload_mime(mime: &str) -> bool {
    matches!(
        mime.trim().to_ascii_lowercase().as_str(),
        "image/jpeg"
            | "image/jpg"
            | "image/png"
            | "image/gif"
            | "image/webp"
            | "image/heic"
            | "image/heif"
            | "image/svg+xml"
            | "application/pdf"
    )
}

fn build_user_parts(
    payload: &ChatInputPayload,
    api_config: &ApiConfig,
) -> Result<Vec<MessagePart>, String> {
    let mut parts = Vec::<MessagePart>::new();
    let mut total_binary = 0usize;

    if let Some(text) = payload
        .text
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !api_config.enable_text {
            return Err("Current API config has text disabled.".to_string());
        }
        parts.push(MessagePart::Text {
            text: text.to_string(),
        });
    }

    if let Some(images) = &payload.images {
        if !images.is_empty() && !api_config.enable_image {
            return Err("Current API config has image disabled.".to_string());
        }

        for image in images {
            let mime = image.mime.trim().to_ascii_lowercase();
            if !is_supported_image_upload_mime(&mime) {
                return Err(format!(
                    "Unsupported attachment mime type: '{}'.",
                    image.mime.trim()
                ));
            }
            let bytes_base64 = image.bytes_base64.trim();
            let raw = B64
                .decode(bytes_base64)
                .map_err(|err| format!("Decode image base64 failed: {err}"))?;
            if mime == "application/pdf" {
                if !api_config.request_format.is_gemini() {
                    return Err(
                        "PDF attachment is only supported when request format is 'gemini'."
                            .to_string(),
                    );
                }
                total_binary += raw.len();
                parts.push(MessagePart::Image {
                    mime: "application/pdf".to_string(),
                    bytes_base64: bytes_base64.to_string(),
                    name: None,
                    compressed: false,
                });
            } else {
                let webp = compress_image_to_webp(&raw)?;
                total_binary += webp.len();
                parts.push(MessagePart::Image {
                    mime: "image/webp".to_string(),
                    bytes_base64: B64.encode(webp),
                    name: None,
                    compressed: true,
                });
            }
        }
    }

    if let Some(audios) = &payload.audios {
        if !audios.is_empty() && !api_config.enable_audio {
            return Err("Current API config has audio disabled.".to_string());
        }

        for audio in audios {
            let bytes_base64 = audio.bytes_base64.trim();
            let raw = B64
                .decode(bytes_base64)
                .map_err(|err| format!("Decode audio base64 failed: {err}"))?;
            total_binary += raw.len();
            parts.push(MessagePart::Audio {
                mime: audio.mime.trim().to_string(),
                bytes_base64: bytes_base64.to_string(),
                name: None,
                compressed: false,
            });
        }
    }

    if total_binary > MAX_MULTIMODAL_BYTES {
        return Err(format!(
            "Multimodal payload exceeds 10MB limit ({} bytes).",
            total_binary
        ));
    }

    if parts.is_empty() {
        return Err("Request payload is empty. Provide text, image, or audio.".to_string());
    }

    Ok(parts)
}

fn render_message_for_context(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => chunks.push(text.clone()),
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    chunks.push("[pdf attached]".to_string());
                } else {
                    chunks.push("[image attached]".to_string());
                }
            }
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    format!("{}: {}", message.role.to_uppercase(), chunks.join(" | "))
}

fn render_message_content_for_model(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text } => chunks.push(text.clone()),
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    chunks.push("[pdf attached]".to_string());
                } else {
                    chunks.push("[image attached]".to_string());
                }
            }
            MessagePart::Audio { .. } => chunks.push("[audio attached]".to_string()),
        }
    }
    if let Some(meta) = &message.provider_meta {
        if let Some(hidden_prompt_text) = meta
            .get("hiddenPromptText")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            chunks.push(hidden_prompt_text.to_string());
        }
        if let Some(attachments) = meta.get("attachments").and_then(Value::as_array) {
            for item in attachments {
                let file_name = item
                    .get("fileName")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if file_name.is_empty() || relative_path.is_empty() {
                    continue;
                }
                chunks.push(format!(
                    "用户本次上传了一个附件：{}\n保存到了你工作区的downloads文件夹内（路径：{}）\n如果需要，请使用 shell 工具读取该文件内容。",
                    file_name, relative_path
                ));
            }
        }
    }
    chunks.join(" | ")
}

fn sanitize_memory_block_xml(raw: &str) -> String {
    if !raw.contains("<memory_board") && !raw.contains("[MemoryBoard]") {
        return raw.to_string();
    }
    raw.lines()
        .filter(|line| {
            let t = line.trim();
            !(t.contains("<keywords>")
                || t.contains("</keywords>")
                || t.contains("<reason>")
                || t.contains("</reason>"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn xml_escape_prompt(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn prompt_role_for_message(message: &ChatMessage, current_agent_id: &str) -> Option<String> {
    let raw_role = message.role.trim().to_lowercase();
    if raw_role != "user" && raw_role != "assistant" {
        return None;
    }
    let speaker_id = message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if !speaker_id.is_empty() && speaker_id == current_agent_id {
        return Some("assistant".to_string());
    }
    Some("user".to_string())
}

fn prompt_speaker_label(
    message: &ChatMessage,
    agents: &[AgentProfile],
    user_name: &str,
) -> String {
    // 优先检查远程 IM 来源
    if let Some(meta) = &message.provider_meta {
        if let Some(origin) = meta.get("origin") {
            if origin.get("kind").and_then(|v| v.as_str()) == Some("remote_im") {
                let sender = origin
                    .get("senderName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let contact = origin
                    .get("remoteContactName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let contact_type = origin
                    .get("remoteContactType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if contact_type == "group" && !contact.is_empty() && !sender.is_empty() {
                    return format!("{} ({})", sender, contact);
                }
                if !sender.is_empty() {
                    return sender.to_string();
                }
                if !contact.is_empty() {
                    return contact.to_string();
                }
            }
        }
    }

    let speaker_id = message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if speaker_id.is_empty() {
        return user_name.trim().to_string();
    }
    if speaker_id == USER_PERSONA_ID {
        let label = user_name.trim();
        if !label.is_empty() {
            return label.to_string();
        }
    }
    agents
        .iter()
        .find(|profile| profile.id == speaker_id)
        .map(|profile| profile.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| speaker_id.to_string())
}

fn build_prompt_speaker_block(
    message: &ChatMessage,
    agents: &[AgentProfile],
    user_name: &str,
    _ui_language: &str,
) -> String {
    let speaker_name = prompt_speaker_label(message, agents, user_name);
    if speaker_name.trim().is_empty() {
        return String::new();
    }
    format!("[{}]", speaker_name)
}

fn build_prompt_user_meta_text(
    message: &ChatMessage,
    agents: &[AgentProfile],
    user_name: &str,
    ui_language: &str,
) -> Option<String> {
    let speaker_block = build_prompt_speaker_block(message, agents, user_name, ui_language);
    let time_text = format_message_time_rfc3339_local(&message.created_at);
    let has_speaker = !speaker_block.trim().is_empty();
    let has_time = !time_text.trim().is_empty();
    match (has_speaker, has_time) {
        (true, true) => Some(format!("{} {}", speaker_block, time_text)),
        (true, false) => Some(speaker_block),
        (false, true) => Some(format!("[{}]", time_text)),
        (false, false) => None,
    }
}
fn render_prompt_message_text(message: &ChatMessage) -> String {
    render_message_content_for_model(message)
}

fn render_prompt_user_text_only(message: &ChatMessage) -> String {
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        if let MessagePart::Text { text } = part {
            if !text.trim().is_empty() {
                chunks.push(text.clone());
            }
        }
    }
    chunks.join("\n")
}

fn resolve_media_from_parts(
    parts: &[MessagePart],
    data_path: Option<&PathBuf>,
    log_prefix: &str,
) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let mut images = Vec::<(String, String)>::new();
    let mut audios = Vec::<(String, String)>::new();
    for part in parts {
        match part {
            MessagePart::Image {
                mime, bytes_base64, ..
            } => {
                let resolved = if let Some(path) = data_path {
                    match resolve_stored_binary_base64(path, bytes_base64) {
                        Ok(value) => value,
                        Err(err) => {
                            eprintln!(
                                "{} 解析图片附件失败，mime={}，data_path={}，bytes_base64_len={}，error={}",
                                log_prefix,
                                mime,
                                path.to_string_lossy(),
                                bytes_base64.len(),
                                err
                            );
                            bytes_base64.clone()
                        }
                    }
                } else {
                    bytes_base64.clone()
                };
                if !resolved.trim().is_empty() {
                    images.push((mime.clone(), resolved));
                }
            }
            MessagePart::Audio {
                mime, bytes_base64, ..
            } => {
                let resolved = if let Some(path) = data_path {
                    match resolve_stored_binary_base64(path, bytes_base64) {
                        Ok(value) => value,
                        Err(err) => {
                            eprintln!(
                                "{} 解析音频附件失败，mime={}，data_path={}，bytes_base64_len={}，error={}",
                                log_prefix,
                                mime,
                                path.to_string_lossy(),
                                bytes_base64.len(),
                                err
                            );
                            bytes_base64.clone()
                        }
                    }
                } else {
                    bytes_base64.clone()
                };
                if !resolved.trim().is_empty() {
                    audios.push((mime.clone(), resolved));
                }
            }
            MessagePart::Text { .. } => {}
        }
    }
    (images, audios)
}

fn collect_prompt_media_parts(
    message: &ChatMessage,
    data_path: Option<&PathBuf>,
) -> (Vec<(String, String)>, Vec<(String, String)>) {
    resolve_media_from_parts(&message.parts, data_path, "[提示词] 历史消息")
}

#[derive(Debug, Clone)]
struct PromptDepartmentCard {
    id: String,
    name: String,
    summary: String,
}

#[derive(Debug, Clone)]
struct PromptDepartmentContext {
    current: PromptDepartmentCard,
    available: Vec<PromptDepartmentCard>,
}

struct DepartmentPromptLabels {
    current_title: &'static str,
    current_name_label: &'static str,
    current_summary_label: &'static str,
    current_guide_label: &'static str,
    available_title: &'static str,
    available_empty: &'static str,
    available_id_label: &'static str,
    available_summary_label: &'static str,
    empty_summary: &'static str,
    empty_guide: &'static str,
}

fn department_prompt_labels(ui_language: &str) -> DepartmentPromptLabels {
    match ui_language.trim() {
        "en-US" => DepartmentPromptLabels {
            current_title: "Current Department",
            current_name_label: "Department",
            current_summary_label: "Summary",
            current_guide_label: "Guide",
            available_title: "Available Departments",
            available_empty: "No available department right now.",
            available_id_label: "Department ID",
            available_summary_label: "Summary",
            empty_summary: "Not provided",
            empty_guide: "No guide configured.",
        },
        "zh-TW" => DepartmentPromptLabels {
            current_title: "你當前所屬部門",
            current_name_label: "部門",
            current_summary_label: "部門概述",
            current_guide_label: "部門辦事指南",
            available_title: "你的可用部門",
            available_empty: "當前沒有可用部門。",
            available_id_label: "部門 ID",
            available_summary_label: "概述",
            empty_summary: "未提供",
            empty_guide: "尚未配置辦事指南。",
        },
        _ => DepartmentPromptLabels {
            current_title: "你当前所属部门",
            current_name_label: "部门",
            current_summary_label: "部门概述",
            current_guide_label: "部门办事指南",
            available_title: "你的可用部门",
            available_empty: "当前没有可用部门。",
            available_id_label: "部门 ID",
            available_summary_label: "概述",
            empty_summary: "未提供",
            empty_guide: "尚未配置办事指南。",
        },
    }
}

fn prompt_department_card_from_config(
    department: &DepartmentConfig,
    empty_summary: &str,
) -> PromptDepartmentCard {
    PromptDepartmentCard {
        id: department.id.clone(),
        name: department.name.trim().to_string(),
        summary: if department.summary.trim().is_empty() {
            empty_summary.to_string()
        } else {
            department.summary.trim().to_string()
        },
    }
}

fn departments_only_config(departments: &[DepartmentConfig]) -> AppConfig {
    AppConfig {
        hotkey: String::new(),
        ui_language: String::new(),
        ui_font: String::new(),
        record_hotkey: String::new(),
        record_background_wake_enabled: false,
        min_record_seconds: 0,
        max_record_seconds: 0,
        tool_max_iterations: 0,
        selected_api_config_id: String::new(),
        assistant_department_api_config_id: String::new(),
        vision_api_config_id: None,
        stt_api_config_id: None,
        stt_auto_send: false,
        terminal_shell_kind: default_terminal_shell_kind(),
        shell_workspaces: Vec::new(),
        mcp_servers: Vec::new(),
        remote_im_channels: Vec::new(),
        departments: departments.to_vec(),
        api_configs: Vec::new(),
    }
}

fn prompt_department_context_from_provider_meta(
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    empty_summary: &str,
) -> Option<PromptDepartmentContext> {
    let temp_config = departments_only_config(departments);
    let current_department = department_for_agent_id(&temp_config, &agent.id)?;
    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|message| prompt_role_for_message(message, &agent.id).as_deref() == Some("user"))?;
    let meta = latest_user.provider_meta.as_ref()?.as_object()?;
    let target_department_id = meta.get("targetDepartmentId").and_then(Value::as_str)?.trim();
    if target_department_id != current_department.id {
        return None;
    }
    let call_stack = meta
        .get("callStack")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<HashSet<String>>()
        })
        .unwrap_or_default();
    let mut available = departments
        .iter()
        .filter(|department| department.id != current_department.id)
        .filter(|department| !call_stack.contains(department.id.as_str()))
        .map(|department| prompt_department_card_from_config(department, empty_summary))
        .collect::<Vec<_>>();
    available.sort_by(|a, b| a.name.cmp(&b.name));
    Some(PromptDepartmentContext {
        current: prompt_department_card_from_config(current_department, empty_summary),
        available,
    })
}

fn build_departments_prompt_block(
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    ui_language: &str,
) -> String {
    if departments.is_empty() {
        return String::new();
    }
    let labels = department_prompt_labels(ui_language);
    let config = departments_only_config(departments);
    let current_department = department_for_agent_id(&config, &agent.id);
    let prompt_context = prompt_department_context_from_provider_meta(
        conversation,
        agent,
        departments,
        labels.empty_summary,
    )
    .or_else(|| {
        current_department.map(|department| PromptDepartmentContext {
            current: prompt_department_card_from_config(department, labels.empty_summary),
            available: departments
                .iter()
                .filter(|item| item.id != department.id)
                .map(|item| prompt_department_card_from_config(item, labels.empty_summary))
                .collect::<Vec<_>>(),
        })
    });
    let Some(prompt_context) = prompt_context else {
        return String::new();
    };
    let guide = current_department
        .map(|department| department.guide.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| labels.empty_guide.to_string());
    let mut lines = vec![
        format!("## {}", labels.current_title),
        format!("- {}：{}", labels.current_name_label, prompt_context.current.name),
        format!(
            "- {}：{}",
            labels.current_summary_label, prompt_context.current.summary
        ),
        format!("- {}：{}", labels.current_guide_label, guide),
        String::new(),
        format!("## {}", labels.available_title),
    ];
    if prompt_context.available.is_empty() {
        lines.push(format!("- {}", labels.available_empty));
    } else {
        for department in prompt_context.available {
            lines.push(format!(
                "- {}：{} | {}：{} | {}：{}",
                labels.current_name_label,
                department.name,
                labels.available_id_label,
                department.id,
                labels.available_summary_label,
                department.summary
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn build_prompt(
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    user_name: &str,
    user_intro: &str,
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
) -> PreparedPrompt {
    build_prompt_with_mode(
        conversation,
        agent,
        agents,
        departments,
        Some((user_name, user_intro)),
        response_style_id,
        ui_language,
        data_path,
    )
}

fn build_delegate_prompt(
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
) -> PreparedPrompt {
    build_prompt_with_mode(
        conversation,
        agent,
        agents,
        departments,
        None,
        response_style_id,
        ui_language,
        data_path,
    )
}

fn build_prompt_with_mode(
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    user_profile: Option<(&str, &str)>,
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
) -> PreparedPrompt {
    let prompt_user_name = user_profile.map(|(user_name, _)| user_name).unwrap_or("");
    let latest_user_index = conversation
        .messages
        .iter()
        .rposition(|message| prompt_role_for_message(message, &agent.id).as_deref() == Some("user"));
    let mut history_messages = Vec::<PreparedHistoryMessage>::new();
    for (idx, message) in conversation.messages.iter().enumerate() {
        let Some(role) = prompt_role_for_message(message, &agent.id) else {
            continue;
        };
        let is_self_message = role == "assistant";
        if Some(idx) == latest_user_index {
            continue;
        }
        if is_self_message {
            if let Some(events) = &message.tool_call {
                for event in sanitize_tool_history_events(events) {
                    let event_role = event
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_lowercase();
                    if event_role == "assistant" {
                        let text = event
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let tool_calls = event
                            .get("tool_calls")
                            .and_then(Value::as_array)
                            .cloned();
                        let reasoning_content = event
                            .get("reasoning_content")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        history_messages.push(PreparedHistoryMessage {
                            role: "assistant".to_string(),
                            text,
                            user_time_text: None,
                            images: Vec::new(),
                            audios: Vec::new(),
                            tool_calls,
                            tool_call_id: None,
                            reasoning_content,
                        });
                    } else if event_role == "tool" {
                        let text = event
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let tool_call_id = event
                            .get("tool_call_id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        if !text.trim().is_empty() || tool_call_id.is_some() {
                            history_messages.push(PreparedHistoryMessage {
                                role: "tool".to_string(),
                                text,
                                user_time_text: None,
                                images: Vec::new(),
                                audios: Vec::new(),
                                tool_calls: None,
                                tool_call_id,
                                reasoning_content: None,
                            });
                        }
                    }
                }
            }
        }
        let is_user = role == "user";
        let text = if is_user {
            render_prompt_user_text_only(message)
        } else {
            render_prompt_message_text(message)
        };
        let (images, audios) = if is_user {
            collect_prompt_media_parts(message, data_path)
        } else {
            (Vec::new(), Vec::new())
        };
        if text.trim().is_empty() && images.is_empty() && audios.is_empty() {
            continue;
        }
        history_messages.push(PreparedHistoryMessage {
            role: role.clone(),
            text,
            user_time_text: if role == "user" {
                build_prompt_user_meta_text(message, agents, prompt_user_name, ui_language)
            } else {
                None
            },
            images,
            audios,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    }
    let response_style = response_style_preset(response_style_id);
    let highest_instruction_md = highest_instruction_markdown();
    let (
        not_provided_label,
        assistant_settings_label,
        user_settings_label,
        role_constraints_label,
        conversation_style_label,
        language_settings_label,
        user_nickname_label,
        user_intro_label,
        role_identity_line,
        role_confusion_line,
        language_follow_user_line,
        language_instruction,
    ) = match ui_language.trim() {
        "en-US" => (
            "Not provided",
            "Assistant settings",
            "User settings",
            "Role constraints",
            "Conversation style",
            "Language settings",
            "User nickname",
            "User self-introduction",
            "- You are \"{}\", and the user is \"{}\".",
            "- Do not treat yourself as the user, and do not confuse the two roles.",
            "- If the user explicitly requests a reply language, follow the user's request.",
            "Please respond in English by default.",
        ),
        "zh-TW" => (
            "未提供",
            "助理設定",
            "使用者設定",
            "角色約束",
            "對話風格",
            "語言設定",
            "使用者暱稱",
            "使用者自我介紹",
            "- 你是「{}」，使用者是「{}」。",
            "- 不要把自己當作使用者，不要混淆雙方身分。",
            "- 若使用者明確指定回答語言，以使用者指定為準。",
            "預設使用繁體中文回答。",
        ),
        _ => (
            "未提供",
            "助理设定",
            "用户设定",
            "角色约束",
            "对话风格",
            "语言设定",
            "用户昵称",
            "用户自我介绍",
            "- 你是“{}”，用户是“{}”。",
            "- 不要把自己当作用户，不要混淆双方身份。",
            "- 若用户明确指定回答语言，以用户指定为准。",
            "默认使用中文回答。",
        ),
    };
    let departments_block = build_departments_prompt_block(conversation, agent, departments, ui_language);
    let preamble = if let Some((user_name, user_intro)) = user_profile {
        let user_intro_display = if user_intro.trim().is_empty() {
            not_provided_label.to_string()
        } else {
            user_intro.trim().to_string()
        };
        let role_identity_text = role_identity_line
            .replacen("{}", &xml_escape_prompt(&agent.name), 1)
            .replacen("{}", &xml_escape_prompt(user_name), 1);
        format!(
            "{}\n\
{}\n\
## {}\n\
{}\n\
\n\
## {}\n\
- {}：{}\n\
- {}：{}\n\
\n\
## {}\n\
{}\n\
{}\n\
\n\
## {}\n\
- 当前风格：{}\n\
{}\n\
\n\
## {}\n\
- {}\n\
- {}\n\
\n",
            highest_instruction_md,
            departments_block,
            assistant_settings_label,
            agent.system_prompt,
            user_settings_label,
            user_nickname_label,
            xml_escape_prompt(user_name),
            user_intro_label,
            xml_escape_prompt(&user_intro_display),
            role_constraints_label,
            role_identity_text,
            role_confusion_line,
            conversation_style_label,
            response_style.name,
            response_style.prompt,
            language_settings_label,
            language_instruction,
            language_follow_user_line
        )
    } else {
        let delegate_role_line = match ui_language.trim() {
            "en-US" => "- This is a delegate thread. There is no default user persona in this thread.",
            "zh-TW" => "- 這是一條委託執行緒。此執行緒不存在預設使用者人格。",
            _ => "- 这是一条委托线程。此线程不存在默认用户人格。",
        };
        let delegate_scope_line = match ui_language.trim() {
            "en-US" => "- Only use the current delegate task block and this thread's own history. Do not invent user profile, nickname, or main-thread background.",
            "zh-TW" => "- 只依據本輪委託任務塊與本執行緒歷史處理工作，不要自行補充使用者設定、暱稱或主會話背景。",
            _ => "- 只依据本轮委托任务块与本线程历史处理工作，不要自行补充用户设定、昵称或主会话背景。",
        };
        format!(
            "{}\n\
{}\n\
## {}\n\
{}\n\
\n\
## {}\n\
{}\n\
{}\n\
\n\
## {}\n\
- 当前风格：{}\n\
{}\n\
\n\
## {}\n\
- {}\n\
\n",
            highest_instruction_md,
            departments_block,
            assistant_settings_label,
            agent.system_prompt,
            role_constraints_label,
            delegate_role_line,
            delegate_scope_line,
            conversation_style_label,
            response_style.name,
            response_style.prompt,
            language_settings_label,
            language_instruction,
        )
    };

    let latest_user = conversation
        .messages
        .iter()
        .rev()
        .find(|message| prompt_role_for_message(message, &agent.id).as_deref() == Some("user"))
        .cloned();

    let mut latest_user_text = String::new();
    let mut latest_user_meta_text = String::new();
    let mut latest_user_extra_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        let user_meta_text =
            build_prompt_user_meta_text(&msg, agents, prompt_user_name, ui_language);
        let latest_user_text_rendered = render_prompt_user_text_only(&msg);
        let (resolved_images, resolved_audios) =
            resolve_media_from_parts(&msg.parts, data_path, "[提示词] 最新消息");
        let ChatMessage {
            extra_text_blocks, ..
        } = msg;
        latest_user_meta_text = user_meta_text.unwrap_or_default();
        latest_user_text = latest_user_text_rendered;
        latest_images = resolved_images;
        latest_audios = resolved_audios;
        for extra in extra_text_blocks {
            if extra.trim().is_empty() {
                continue;
            }
            let extra = sanitize_memory_block_xml(&extra);
            if extra.trim().is_empty() {
                continue;
            }
            if !latest_user_extra_text.is_empty() {
                latest_user_extra_text.push('\n');
            }
            latest_user_extra_text.push_str(&extra);
        }
        if let Some(meta) = msg.provider_meta.as_ref() {
            if let Some(attachments) = meta.get("attachments").and_then(Value::as_array) {
                for item in attachments {
                    let file_name = item
                        .get("fileName")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or("");
                    let relative_path = item
                        .get("relativePath")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or("");
                    if file_name.is_empty() || relative_path.is_empty() {
                        continue;
                    }
                    if !latest_user_extra_text.is_empty() {
                        latest_user_extra_text.push('\n');
                    }
                    latest_user_extra_text.push_str(&format!(
                        "用户本次上传了一个附件：{}\n保存到了你工作区的downloads文件夹内（路径：{}）\n如果需要，请使用 shell 工具读取该文件内容。",
                        file_name, relative_path
                    ));
                }
            }
        }
    }

    PreparedPrompt {
        preamble,
        history_messages,
        latest_user_text,
        latest_user_meta_text,
        latest_user_extra_text,
        latest_images,
        latest_audios,
    }
}

fn image_media_type_from_mime(mime: &str) -> Option<ImageMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Some(ImageMediaType::JPEG),
        "image/png" => Some(ImageMediaType::PNG),
        "image/gif" => Some(ImageMediaType::GIF),
        "image/webp" => Some(ImageMediaType::WEBP),
        "image/heic" => Some(ImageMediaType::HEIC),
        "image/heif" => Some(ImageMediaType::HEIF),
        "image/svg+xml" => Some(ImageMediaType::SVG),
        _ => None,
    }
}

fn audio_media_type_from_mime(mime: &str) -> Option<AudioMediaType> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "audio/wav" | "audio/wave" => Some(AudioMediaType::WAV),
        "audio/mp3" | "audio/mpeg" => Some(AudioMediaType::MP3),
        "audio/aiff" => Some(AudioMediaType::AIFF),
        "audio/aac" => Some(AudioMediaType::AAC),
        "audio/ogg" => Some(AudioMediaType::OGG),
        "audio/flac" => Some(AudioMediaType::FLAC),
        _ => None,
    }
}
