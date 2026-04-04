fn latest_active_conversation_index(
    data: &AppData,
    _api_config_id: &str,
    _agent_id: &str,
) -> Option<usize> {
    data.conversations
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(c)
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

fn latest_main_conversation_index(data: &AppData, _agent_id: &str) -> Option<usize> {
    data.conversations
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.summary.trim().is_empty()
                && conversation_visible_in_foreground_lists(c)
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

fn fallback_foreground_department_id() -> String {
    ASSISTANT_DEPARTMENT_ID.to_string()
}

fn resolved_foreground_department_id_for_conversation(
    config: &AppConfig,
    conversation: &Conversation,
    is_main_conversation: bool,
) -> String {
    let existing = conversation.department_id.trim();
    if !existing.is_empty() && department_by_id(config, existing).is_some() {
        return existing.to_string();
    }
    if is_main_conversation {
        return fallback_foreground_department_id();
    }
    department_for_agent_id(config, &conversation.agent_id)
        .map(|department| department.id.clone())
        .or_else(|| assistant_department(config).map(|department| department.id.clone()))
        .unwrap_or_else(fallback_foreground_department_id)
}

fn normalize_foreground_conversation_departments(
    config: &AppConfig,
    data: &mut AppData,
) -> bool {
    let main_conversation_id = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let mut changed = false;
    for conversation in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(conversation)
            || !conversation.summary.trim().is_empty()
        {
            continue;
        }
        let next_department_id = resolved_foreground_department_id_for_conversation(
            config,
            conversation,
            conversation.id.trim() == main_conversation_id,
        );
        if conversation.department_id.trim() != next_department_id {
            conversation.department_id = next_department_id;
            changed = true;
        }
    }
    changed
}

fn main_conversation_index(data: &AppData, _agent_id: &str) -> Option<usize> {
    let target_id = data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    data.conversations.iter().position(|conversation| {
        conversation.id == target_id
            && conversation.summary.trim().is_empty()
            && conversation_visible_in_foreground_lists(conversation)
    })
}

fn normalize_main_conversation_marker(data: &mut AppData, _agent_id: &str) -> bool {
    if main_conversation_index(data, "").is_some() {
        return false;
    }
    if data.main_conversation_id.is_none() {
        return false;
    }
    data.main_conversation_id = None;
    true
}

fn normalize_single_active_main_conversation(data: &mut AppData) -> bool {
    let Some(keep_idx) = latest_active_conversation_index(data, "", "")
        .or_else(|| latest_main_conversation_index(data, ""))
    else {
        return false;
    };

    let mut changed = false;
    for (_idx, conversation) in data.conversations.iter_mut().enumerate() {
        if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
            continue;
        }
        let target_status = "active";
        if conversation.status.trim() != target_status {
            conversation.status = target_status.to_string();
            changed = true;
        }
    }
    if changed {
        let keep_id = data
            .conversations
            .get(keep_idx)
            .map(|item| item.id.clone())
            .unwrap_or_default();
        eprintln!(
            "[会话] 归一化未归档主会话激活标记: active_conversation_id={}",
            keep_id
        );
    }
    changed
}

fn conversation_is_delegate(conversation: &Conversation) -> bool {
    conversation.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE
}

fn conversation_is_remote_im_contact(conversation: &Conversation) -> bool {
    conversation.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
}

fn conversation_visible_in_foreground_lists(conversation: &Conversation) -> bool {
    !conversation_is_delegate(conversation)
        && !conversation_is_remote_im_contact(conversation)
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
    department_id: &str,
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
        department_id: department_id.trim().to_string(),
        last_read_message_id: String::new(),
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
        user_profile_snapshot: String::new(),
        shell_workspace_path: None,
        archived_at: None,
        messages: Vec::new(),
        current_todos: Vec::new(),
        memory_recall_table: Vec::new(),
    }
}

fn build_foreground_chat_conversation_record(
    data_path: &PathBuf,
    data: &AppData,
    api_config_id: &str,
    agent_id: &str,
    department_id: &str,
    title: &str,
) -> Conversation {
    let mut conversation = build_conversation_record(
        api_config_id,
        agent_id,
        department_id,
        title,
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    let snapshot_agent_id = agent_id
        .trim()
        .is_empty()
        .then(|| data.assistant_department_agent_id.trim().to_string())
        .unwrap_or_else(|| agent_id.trim().to_string());
    let user_profile_snapshot = data
        .agents
        .iter()
        .find(|item| item.id == snapshot_agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_error(format!(
                    "[用户画像] 失败，任务=build_foreground_chat_conversation_record，agent_id={}，error={}",
                    agent.id, err
                ));
                None
            }
        });
    if let Some(snapshot) = user_profile_snapshot.clone() {
        conversation.user_profile_snapshot = snapshot;
    }
    let last_archive_summary = data
        .conversations
        .iter()
        .rev()
        .find(|item| !conversation_is_delegate(item) && !item.summary.trim().is_empty())
        .map(|item| item.summary.as_str());
    let summary_message = build_initial_summary_context_message(
        last_archive_summary,
        user_profile_snapshot.as_deref(),
        Some(&conversation.current_todos),
    );
    conversation.last_user_at = Some(summary_message.created_at.clone());
    conversation.updated_at = summary_message.created_at.clone();
    conversation.messages.push(summary_message);
    conversation
}

fn ensure_active_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    let _ = normalize_main_conversation_marker(data, agent_id);
    let _ = normalize_single_active_main_conversation(data);
    if let Some(idx) = latest_active_conversation_index(data, api_config_id, agent_id) {
        return idx;
    }

    if let Some(idx) = latest_main_conversation_index(data, agent_id) {
        for (_i, conversation) in data.conversations.iter_mut().enumerate() {
            if !conversation_visible_in_foreground_lists(conversation) || !conversation.summary.trim().is_empty() {
                continue;
            }
            conversation.status = "active".to_string();
        }
        return idx;
    }

    let conversation = build_conversation_record(
        api_config_id,
        "",
        "",
        "",
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );

    for item in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(item) || !item.summary.trim().is_empty() {
            continue;
        }
        item.status = "active".to_string();
    }
    data.conversations.push(conversation);
    if data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        data.main_conversation_id = data.conversations.last().map(|item| item.id.clone());
    }
    data.conversations.len() - 1
}

fn ensure_main_conversation_index(
    data: &mut AppData,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    let _ = normalize_main_conversation_marker(data, agent_id);
    if let Some(idx) = main_conversation_index(data, agent_id) {
        return idx;
    }
    let conversation = build_conversation_record(
        api_config_id,
        agent_id,
        ASSISTANT_DEPARTMENT_ID,
        "",
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    let conversation_id = conversation.id.clone();
    for item in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(item) || !item.summary.trim().is_empty() {
            continue;
        }
        item.status = "active".to_string();
    }
    data.conversations.push(conversation);
    data.main_conversation_id = Some(conversation_id);
    data.conversations.len() - 1
}

fn ensure_active_foreground_conversation_index_atomic(
    data: &mut AppData,
    data_path: &PathBuf,
    api_config_id: &str,
    agent_id: &str,
) -> usize {
    let _ = normalize_main_conversation_marker(data, agent_id);
    let _ = normalize_single_active_main_conversation(data);
    if let Some(idx) = main_conversation_index(data, agent_id) {
        for conversation in &mut data.conversations {
            if !conversation_visible_in_foreground_lists(conversation)
                || !conversation.summary.trim().is_empty()
            {
                continue;
            }
            conversation.status = "active".to_string();
        }
        return idx;
    }

    let conversation =
        build_foreground_chat_conversation_record(
            data_path,
            data,
            api_config_id,
            agent_id,
            ASSISTANT_DEPARTMENT_ID,
            "",
        );
    for item in &mut data.conversations {
        if !conversation_visible_in_foreground_lists(item) || !item.summary.trim().is_empty() {
            continue;
        }
        item.status = "active".to_string();
    }
    data.conversations.push(conversation);
    if data
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        data.main_conversation_id = data.conversations.last().map(|item| item.id.clone());
    }
    data.conversations.len() - 1
}

fn active_foreground_conversation_index_read_only(
    data: &AppData,
    agent_id: &str,
) -> Option<usize> {
    main_conversation_index(data, agent_id)
        .or_else(|| latest_active_conversation_index(data, "", agent_id))
        .or_else(|| latest_main_conversation_index(data, agent_id))
}

#[derive(Debug, Clone)]
struct ArchiveDecision {
    should_archive: bool,
    forced: bool,
    reason: String,
    usage_ratio: f64,
}

fn estimated_tokens_for_text(text: &str) -> f64 {
    static TOKEN_BPE: std::sync::OnceLock<Option<tiktoken_rs::CoreBPE>> = std::sync::OnceLock::new();
    if let Some(bpe) = TOKEN_BPE
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
    {
        return bpe.encode_with_special_tokens(text).len() as f64;
    }

    // 极端情况下 tokenizer 初始化失败，回退到旧启发式，避免中断主流程。
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

fn build_archive_decision_from_usage_ratio(
    usage_ratio: f64,
    last_user_at: Option<&str>,
    has_assistant_reply: bool,
) -> ArchiveDecision {
    if !has_assistant_reply {
        return ArchiveDecision {
            should_archive: false,
            forced: false,
            reason: "no_assistant_reply".to_string(),
            usage_ratio,
        };
    }
    if usage_ratio >= 0.82 {
        return ArchiveDecision {
            should_archive: true,
            forced: true,
            reason: "force_context_usage_82".to_string(),
            usage_ratio,
        };
    }

    let Some(last_user_at) = last_user_at.and_then(parse_iso) else {
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

fn decide_archive_before_model_request(
    estimated_prompt_tokens: u64,
    context_window_tokens: u32,
    last_user_at: Option<&str>,
    has_assistant_reply: bool,
) -> ArchiveDecision {
    let max_tokens = context_window_tokens.max(1) as f64;
    let usage_ratio = (estimated_prompt_tokens as f64 / max_tokens).max(0.0);
    build_archive_decision_from_usage_ratio(usage_ratio, last_user_at, has_assistant_reply)
}

fn decide_archive_before_send_with_fallback(
    cached_effective_prompt_tokens: u64,
    cached_usage_ratio: f64,
    estimated_prompt_tokens: Option<u64>,
    context_window_tokens: u32,
    last_user_at: Option<&str>,
    has_assistant_reply: bool,
) -> (ArchiveDecision, &'static str) {
    if cached_effective_prompt_tokens > 0 {
        return (
            decide_archive_before_model_request(
                cached_effective_prompt_tokens,
                context_window_tokens,
                last_user_at,
                has_assistant_reply,
            ),
            "cached_effective_prompt_tokens",
        );
    }
    if cached_usage_ratio.is_finite() && cached_usage_ratio > 0.0 {
        return (
            build_archive_decision_from_usage_ratio(
                cached_usage_ratio.max(0.0),
                last_user_at,
                has_assistant_reply,
            ),
            "cached_usage_ratio",
        );
    }
    (
        decide_archive_before_model_request(
            estimated_prompt_tokens.unwrap_or(0),
            context_window_tokens,
            last_user_at,
            has_assistant_reply,
        ),
        "estimated_prompt_tokens",
    )
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
        .position(|c| c.id == conversation_id && c.summary.trim().is_empty())?;
    let conv = data.conversations.get_mut(idx)?;
    let previous_status = conv.status.clone();
    let now = now_iso();
    conv.status = "archived".to_string();
    conv.summary = summary.to_string();
    conv.archived_at = Some(now.clone());
    conv.updated_at = now;
    let archive_id = conv.id.clone();
    eprintln!(
        "[会话] 已归档: conversation_id={}, previous_status={}, reason=\"{}\", summary=\"{}\"",
        conv.id,
        previous_status,
        reason,
        summary
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
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if relative_path.is_empty() {
                    continue;
                }
                chunks.push(format!(
                    "用户上传了附件，文件位于你工作区的 downloads 目录（路径：{}）。\n你可以先用 shell 工具定位或查看基础文件信息；具体解析方式应按文件类型选择合适 skill 或在线检索正确方法。\n仅当用户明确要求处理该附件时再处理；若用户未明确要求，请先询问用户想如何处理。",
                    relative_path
                ));
            }
        }
    }
    chunks.join(" | ")
}

fn sanitize_memory_block_xml(raw: &str) -> String {
    if !raw.contains("<memory_board")
        && !raw.contains("[MemoryBoard]")
        && !raw.contains("<memory_context>")
    {
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
                let sender = remote_im_origin_string(origin, "sender_name").unwrap_or("");
                let contact = remote_im_origin_string(origin, "contact_name").unwrap_or("");
                let contact_type = remote_im_origin_string(origin, "contact_type").unwrap_or("");
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
    include_remote_identity: bool,
) -> Option<String> {
    if is_context_compaction_message(message, "user") {
        return None;
    }
    let speaker_block = build_prompt_speaker_block(message, agents, user_name, ui_language);
    let time_text = format_message_time_rfc3339_local_to_minute(&message.created_at);
    let has_speaker = !speaker_block.trim().is_empty();
    let has_time = !time_text.trim().is_empty();
    let mut base = match (has_speaker, has_time) {
        (true, true) => format!("{} {}", speaker_block, time_text),
        (true, false) => speaker_block,
        (false, true) => format!("[{}]", time_text),
        (false, false) => String::new(),
    };
    let mut tags = Vec::<String>::new();
    if include_remote_identity {
        if let Some(origin) = remote_im_origin_from_message(message) {
            let channel_id = remote_im_origin_string(origin, "channel_id").unwrap_or("");
            let contact_id = remote_im_origin_string(origin, "contact_id").unwrap_or("");
            if !channel_id.is_empty() {
                tags.push(format!("channel_id={}", channel_id));
            }
            if !contact_id.is_empty() {
                tags.push(format!("contact_id={}", contact_id));
            }
        }
    }
    if let Some(memory_ids) = message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("memoryIds"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
    {
        tags.push(format!("memory={}", memory_ids.join(",")));
    }
    if !tags.is_empty() {
        if !base.trim().is_empty() {
            base.push_str(" | ");
        }
        base.push_str(&tags.join(" | "));
    }
    if base.trim().is_empty() {
        None
    } else {
        Some(base)
    }
}

fn format_message_time_rfc3339_local_to_minute(raw: &str) -> String {
    let full = format_message_time_rfc3339_local(raw);
    let trimmed = full.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let Some(t_idx) = trimmed.find('T') else {
        return format_message_time_text(raw)
            .chars()
            .take(16)
            .collect::<String>();
    };
    let date = &trimmed[..t_idx];
    let rest = &trimmed[t_idx + 1..];
    let tz_idx = rest
        .find(|ch: char| ch == '+' || ch == '-' || ch == 'Z')
        .unwrap_or(rest.len());
    let time = &rest[..tz_idx];
    let mut segs = time.split(':');
    let hh = segs.next().unwrap_or("");
    let mm = segs.next().unwrap_or("");
    if hh.len() == 2 && mm.len() == 2 {
        return format!("{date}T{hh}:{mm}");
    }
    trimmed.to_string()
}

fn prompt_current_date_timezone_line(_ui_language: &str) -> String {
    let tz = local_utc_offset()
        .map(|offset| {
            let seconds = offset.whole_seconds();
            let sign = if seconds < 0 { '-' } else { '+' };
            let abs = seconds.abs();
            let hours = abs / 3600;
            let minutes = (abs % 3600) / 60;
            format!("{sign}{hours:02}:{minutes:02}")
        })
        .unwrap_or_else(|| "local".to_string());
    format!("- 时区：{}", tz)
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

fn remote_im_origin_from_message(message: &ChatMessage) -> Option<&Value> {
    let meta = message.provider_meta.as_ref()?;
    let origin = meta.get("origin")?;
    if origin.get("kind").and_then(Value::as_str) != Some("remote_im") {
        return None;
    }
    Some(origin)
}

fn remote_im_origin_string<'a>(origin: &'a Value, key: &str) -> Option<&'a str> {
    origin
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn remote_im_contact_key_from_message(message: &ChatMessage) -> Option<String> {
    let origin = remote_im_origin_from_message(message)?;
    let channel_id = remote_im_origin_string(origin, "channel_id").unwrap_or("");
    let contact_id = remote_im_origin_string(origin, "contact_id").unwrap_or("");
    if channel_id.is_empty() || contact_id.is_empty() {
        return None;
    }
    Some(format!("{}::{}", channel_id, contact_id))
}

fn build_prompt_message_context_block(message: &ChatMessage) -> Option<String> {
    let mut lines = Vec::<String>::new();
    if let Some(meta) = message.provider_meta.as_ref() {
        if let Some(target_department_id) = meta
            .get("targetDepartmentId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("targetDepartmentId: {}", target_department_id));
        }
        if let Some(target_agent_id) = meta
            .get("targetAgentId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("targetAgentId: {}", target_agent_id));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!("[消息上下文]\n{}", lines.join("\n")))
}

fn prompt_retrieved_memory_ids_from_message(message: &ChatMessage) -> Vec<String> {
    let Some(meta) = message.provider_meta.as_ref() else {
        return Vec::new();
    };
    let Some(ids) = meta
        .get("retrieved_memory_ids")
        .or_else(|| meta.get("recallMemoryIds"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut seen = HashSet::<String>::new();
    ids.iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let owned = value.to_string();
            if seen.insert(owned.clone()) {
                Some(owned)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

fn collect_prompt_retrieved_memory_ids(messages: &[ChatMessage]) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut collected = Vec::<String>::new();
    for message in messages {
        for memory_id in prompt_retrieved_memory_ids_from_message(message) {
            if seen.insert(memory_id.clone()) {
                collected.push(memory_id);
            }
        }
    }
    collected
}

fn prompt_recall_memory_block_for_message(
    message: &ChatMessage,
    recall_memories: Option<&[MemoryEntry]>,
    seen_memory_ids: &mut HashSet<String>,
) -> Option<String> {
    let Some(memories) = recall_memories else {
        return None;
    };
    let retrieved_ids = prompt_retrieved_memory_ids_from_message(message);
    if retrieved_ids.is_empty() {
        return None;
    }
    let inject_ids = retrieved_ids
        .into_iter()
        .filter(|memory_id| seen_memory_ids.insert(memory_id.clone()))
        .collect::<Vec<_>>();
    build_memory_board_xml_from_recall_ids(memories, &inject_ids)
}

fn prompt_user_extra_blocks_for_message(
    message: &ChatMessage,
    recall_memories: Option<&[MemoryEntry]>,
    seen_memory_ids: &mut HashSet<String>,
) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    if let Some(context_block) = build_prompt_message_context_block(message) {
        blocks.push(context_block);
    }
    if let Some(recall_block) =
        prompt_recall_memory_block_for_message(message, recall_memories, seen_memory_ids)
    {
        blocks.push(recall_block);
    }
    for extra in &message.extra_text_blocks {
        if extra.trim().is_empty() {
            continue;
        }
        let trimmed = extra.trim();
        if trimmed.starts_with("[远程IM] 发送者:")
            || trimmed.starts_with("[RemoteIM] sender:")
        {
            continue;
        }
        let extra = sanitize_memory_block_xml(extra);
        if extra.trim().is_empty() {
            continue;
        }
        blocks.push(extra);
    }
    if let Some(meta) = message.provider_meta.as_ref() {
        if let Some(attachments) = meta.get("attachments").and_then(Value::as_array) {
            for item in attachments {
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if relative_path.is_empty() {
                    continue;
                }
                blocks.push(format!(
                    "用户上传了附件，文件位于你工作区的 downloads 目录（路径：{}）。\n你可以先用 shell 工具定位或查看基础文件信息；具体解析方式应按文件类型选择合适 skill 或在线检索正确方法。\n仅当用户明确要求处理该附件时再处理；若用户未明确要求，请先询问用户想如何处理。",
                    relative_path
                ));
            }
        }
    }
    blocks
}

fn provider_meta_message_kind(message: &ChatMessage) -> Option<String> {
    message
        .provider_meta
        .as_ref()?
        .get("message_meta")
        .and_then(Value::as_object)?
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_context_compaction_message(message: &ChatMessage, role: &str) -> bool {
    if role != "user" {
        return false;
    }
    matches!(
        provider_meta_message_kind(message).as_deref(),
        Some("context_compaction") | Some("summary_context_seed")
    )
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

fn department_prompt_labels(_ui_language: &str) -> DepartmentPromptLabels {
    DepartmentPromptLabels {
        current_name_label: "部门",
        current_summary_label: "部门概述",
        current_guide_label: "部门办事指南",
        available_title: "你的可用部门",
        available_empty: "当前没有可用部门。",
        available_id_label: "部门 ID",
        available_summary_label: "概述",
        empty_summary: "未提供",
        empty_guide: "尚未配置办事指南。",
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
        provider_non_stream_base_urls: Vec::new(),
        api_providers: Vec::new(),
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
        format!("{}：{}", labels.current_name_label, prompt_context.current.name),
        format!(
            "{}：{}",
            labels.current_summary_label, prompt_context.current.summary
        ),
        format!("{}：{}", labels.current_guide_label, guide),
        String::new(),
        format!("{}：", labels.available_title),
    ];
    if prompt_context.available.is_empty() {
        lines.push(labels.available_empty.to_string());
    } else {
        for department in prompt_context.available {
            lines.push(format!(
                "{}：{} | {}：{} | {}：{}",
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
    prompt_xml_block("department context", lines.join("\n"))
}

fn build_memory_rag_rule_block() -> String {
    prompt_xml_block(
        "memory rag rule",
        "当用户消息中出现 `<memory_context>` 块时，应将其视为系统检索出的历史记忆背景，而不是用户当前这条消息本身。\n\
         1. 只有在确有帮助时才参考这些记忆。\n\
         2. 应自然融入理解与回复，不要主动暴露检索、注入或档案读取等机制。\n\
         3. 不要把这些记忆误当成用户此刻正在明确表达的立场、需求或情绪。\n\
         4. 若当前消息与记忆冲突，一律以当前消息为准。\n\
         5. 若用户明确追问你为什么记得，可以坦诚说明这来自历史对话记忆。",
    )
}

fn build_system_tools_rule_block(
    _conversation: &Conversation,
    _agent: &AgentProfile,
    _departments: &[DepartmentConfig],
    _ui_language: &str,
) -> Option<String> {
    let mut sections = vec![
        "仅在系统工具能真正帮助完成用户任务时才使用，不要为了显得主动而滥用工具。"
            .to_string(),
    ];

    sections.push(
        "1. todo\n\
         何时必须用：当任务预计需要多个阶段、存在依赖关系、需要跨文件修改、需要验证，或可能持续超过一次工具调用时，必须使用 todo。\n\
         何时不要用：单步即可完成的简单问题、纯解释性回答、无需实际操作的闲聊，不要使用 todo。\n\
         如何使用：todo 必须拆成 3~7 步；每一步都必须是可验证、可完成的结果；开始执行后要及时更新状态；任一时刻只允许一个 in_progress；计划变化时同步修正。\n\
         为什么：todo 是当前会话内的执行步骤板，不是长期任务系统。"
            .to_string(),
    );
    sections.push(
        "2. delegate\n\
         何时必须用：当子任务过于模糊，需要先探索再收敛结论时，可以使用 delegate。模糊探索既可以是本地探索，也可以是网络探索。\n\
         何时不要用：如果主线程立刻需要这个结果来继续下一步，通常不要委托；边界明确、可直接动手的任务，也不要滥用 delegate。\n\
         如何使用：先快速扫描少量关键文件或关键信息形成初步理解；先写骨架计划并尽早和用户完成第一轮对齐；不要在和用户建立共识前做穷尽式探索。质量优先于数量，最多只允许有限数量的 explore 代理，一般应尽量少，通常一个就够。\n\
         为什么：delegate 负责高不确定性的探索任务，不是把核心决策责任直接甩出去。"
            .to_string(),
    );
    sections.push(
        "3. task\n\
         何时必须用：task 只用于非即时、长期、跨会话追踪的任务。到点后系统会自动提示你要执行某个任务，并在任务追踪中持续提供执行该任务所需的上下文。\n\
         何时不要用：如果事情只在当前会话内推进和完成，不要使用 task，而应使用 todo。\n\
         如何使用：只有在确实需要未来触发、长期提醒、跨会话延续时才创建或更新 task，并明确长期目标、为什么做、当前下一步以及触发条件。\n\
         为什么：task 是长期任务机制，不是当前会话的临时步骤板。"
            .to_string(),
    );
    sections.push(
        "4. exec\n\
         何时必须用：当必须通过命令、程序或脚本来搜索文件、读取信息、检查环境、运行验证或执行脚本时，使用 exec。很多 skill 会要求执行脚本。\n\
         何时不要用：一般情况下禁止使用 exec 修改文件；文件修改应优先使用正常编辑能力，而不是把 exec 当成文件编辑器。\n\
         如何使用：优先把 exec 用于搜索、读取、检查、运行和验证；执行前先判断是否存在更低风险替代，并尽量缩小命令影响范围。\n\
         为什么：exec 负责命令执行与脚本运行，但副作用风险更高，因此默认不承担常规文件编辑职责。"
            .to_string(),
    );

    Some(prompt_xml_block("system tools rule", sections.join("\n\n")))
}

fn build_question_and_planning_rule_block() -> String {
    prompt_xml_block(
        "question and planning rule",
        "提问之法\n\
         价值锚定：唯当缺失信息重创方向、风险、成本或产出时，方可求询。\n\
         前置分析：提问前必先检索上下文，形成初步逻辑模型。\n\
         高频克制：首轮提问须精准且低通量，严禁堆砌问题清单。\n\
         自主检索：凡代码、配置或既有文档可自证者，莫扰用户。\n\
         默认对齐：若存在高概率、低风险之默认假设，应带假推进并明确告知。\n\
         拒绝外包：凡属自身职能之分析与设计工作，断不可转嫁用户。\n\n\
         规划之道\n\
         骨架建模：遇非平凡任务，先扫描核心文件，构建含主阶段与要点之骨架计划。\n\
         敏捷探索：计划未定，严禁穷尽式本地或网络搜寻。\n\
         交付导向：计划旨在驱动迭代，而非沦为冗长之形式文档。\n\
         风险对齐：遇非显性分叉或成本差异，必先与用户同步，后行重度开发。\n\
         动态修正：新信息既入，路径即更，切莫硬性执行旧版路线图。\n\n\
         二者逻辑\n\
         提问乃为破除当前计划之核心不确定性。\n\
         计划乃是将当前认知转化为可执行路径。\n\
         严禁在无初步计划时盲目索取，亦不可在关键因子未明时伪称架构稳定。",
    )
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
    state: Option<&AppState>,
    resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: bool,
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
        state,
        resolved_api,
        enable_pdf_images,
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
    state: Option<&AppState>,
    resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: bool,
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
        state,
        resolved_api,
        enable_pdf_images,
    )
}

fn find_last_context_compaction_index(
    messages: &[ChatMessage],
    agent_id: &str,
) -> Option<usize> {
    messages
        .iter()
        .enumerate()
        .filter_map(|(idx, message)| {
            let role = prompt_role_for_message(message, agent_id)?;
            if is_context_compaction_message(message, role.as_str()) {
                Some(idx)
            } else {
                None
            }
        })
        .last()
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
    state: Option<&AppState>,
    _resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: bool,
) -> PreparedPrompt {
    let source_messages = match find_last_context_compaction_index(&conversation.messages, &agent.id)
    {
        Some(boundary) => &conversation.messages[boundary..],
        None => conversation.messages.as_slice(),
    };

    // 仅对压缩边界后的有效消息做附件 enrich，避免旧历史重复处理。
    let mut enriched_messages = source_messages.to_vec();

    if let Some(state) = state {
        for message in &mut enriched_messages {
            if let Some(meta) = message.provider_meta.as_ref() {
                if let Some(attachments) = meta.get("attachments").and_then(Value::as_array) {
                    for item in attachments {
                        let mime = item.get("mime").and_then(Value::as_str).unwrap_or("");
                        if mime != "application/pdf" {
                            continue;
                        }
                        let relative_path = item.get("relativePath").and_then(Value::as_str).unwrap_or("");
                        if relative_path.is_empty() {
                            continue;
                        }
                        let file_name = item.get("fileName").and_then(Value::as_str).unwrap_or("");
                        let include_images = enable_pdf_images;
                        let workspace_root = configured_workspace_root_path(state)
                            .unwrap_or_else(|_| state.llm_workspace_path.clone());
                        let file_path = workspace_root.join(relative_path);
                        let Some(file_path_str) = file_path.to_str() else {
                            eprintln!(
                                "[PDF提取] 跳过 路径包含非UTF-8字符, conversation_id={}, relative_path={}",
                                conversation.id, relative_path
                            );
                            continue;
                        };
                        let conversation_id = conversation.id.clone();
                        match get_or_extract_pdf_structured(
                            state,
                            &conversation_id,
                            file_path_str,
                            include_images,
                        ) {
                            Ok(result) => {
                                if enable_pdf_images {
                                    for page in result.pages {
                                        for (img_idx, img) in page.images.iter().enumerate() {
                                            message.parts.push(MessagePart::Image {
                                                mime: img.mime.clone(),
                                                bytes_base64: img.bytes_base64.clone(),
                                                name: Some(format!(
                                                    "{}_p{}_img{}.webp",
                                                    result.file_name,
                                                    page.page_index + 1,
                                                    img_idx + 1,
                                                )),
                                                compressed: false,
                                            });
                                        }
                                    }
                                } else {
                                    for page in result.pages {
                                        let page_text = format!(
                                            "[PDF文档分页]\n文件名：{}\n页码：{}/{}\n\n{}",
                                            result.file_name,
                                            page.page_index + 1,
                                            result.total_pages,
                                            page.text
                                        );
                                        message.extra_text_blocks.push(page_text);
                                    }
                                }
                                message.extra_text_blocks.push(format!(
                                    "提示：如需阅读完整内容，请使用 shell 工具读取 {}",
                                    relative_path
                                ));
                            }
                            Err(e) => {
                                if is_pdf_page_limit_exceeded_error(&e) {
                                    message.extra_text_blocks.push(format!(
                                        "提示：PDF 页数超过 {} 页，已按普通文件处理，不进行自动提取。",
                                        100
                                    ));
                                    message.extra_text_blocks.push(format!(
                                        "提示：如需阅读完整内容，请使用 shell 工具读取 {}",
                                        relative_path
                                    ));
                                    continue;
                                }
                                eprintln!(
                                    "[PDF提取] 失败 conversation_id={}, file_name={}, error={:?}",
                                    conversation_id, file_name, e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // 使用enriched_messages替代conversation.messages
    let enriched_conversation = Conversation {
        id: conversation.id.clone(),
        title: conversation.title.clone(),
        agent_id: conversation.agent_id.clone(),
        department_id: conversation.department_id.clone(),
        last_read_message_id: conversation.last_read_message_id.clone(),
        conversation_kind: conversation.conversation_kind.clone(),
        root_conversation_id: conversation.root_conversation_id.clone(),
        delegate_id: conversation.delegate_id.clone(),
        created_at: conversation.created_at.clone(),
        updated_at: conversation.updated_at.clone(),
        last_user_at: conversation.last_user_at.clone(),
        last_assistant_at: conversation.last_assistant_at.clone(),
        last_context_usage_ratio: conversation.last_context_usage_ratio,
        last_effective_prompt_tokens: conversation.last_effective_prompt_tokens,
        status: conversation.status.clone(),
        summary: conversation.summary.clone(),
        user_profile_snapshot: conversation.user_profile_snapshot.clone(),
        shell_workspace_path: conversation.shell_workspace_path.clone(),
        archived_at: conversation.archived_at.clone(),
        messages: enriched_messages,
        current_todos: conversation.current_todos.clone(),
        memory_recall_table: conversation.memory_recall_table.clone(),
    };
    let recall_memory_ids = collect_prompt_retrieved_memory_ids(&enriched_conversation.messages);
    let recall_memories = if recall_memory_ids.is_empty() {
        None
    } else {
        data_path.and_then(|path| match memory_store_list_memories_by_ids_visible_for_agent(
            path,
            &recall_memory_ids,
            &agent.id,
            agent.private_memory_enabled,
        ) {
            Ok(memories) => Some(memories),
            Err(err) => {
                runtime_log_error(format!(
                    "[提示词] 读取召回记忆失败: agent_id={}, recall_ids={:?}, error={:?}",
                    agent.id, recall_memory_ids, err
                ));
                None
            }
        })
    };

    let prompt_user_name = user_profile.map(|(user_name, _)| user_name).unwrap_or("");
    let mut seen_remote_contacts = std::collections::HashSet::<String>::new();
    let mut seen_prompt_memory_ids = HashSet::<String>::new();
    let last_compaction_index =
        find_last_context_compaction_index(&enriched_conversation.messages, &agent.id);
    let mut latest_user_index = None;
    for (idx, message) in enriched_conversation.messages.iter().enumerate().rev() {
        if let Some(boundary) = last_compaction_index {
            if idx < boundary {
                break;
            }
        }
        let Some(role) = prompt_role_for_message(message, &agent.id) else {
            continue;
        };
        if is_context_compaction_message(message, role.as_str()) {
            continue;
        }
        if role == "user" {
            latest_user_index = Some(idx);
        }
        break;
    }
    let mut history_messages = Vec::<PreparedHistoryMessage>::new();
    for (idx, message) in enriched_conversation.messages.iter().enumerate() {
        let Some(role) = prompt_role_for_message(message, &agent.id) else {
            continue;
        };
        if let Some(boundary) = last_compaction_index {
            if idx < boundary {
                continue;
            }
        }
        let is_self_message = role == "assistant";
        if Some(idx) == latest_user_index {
            continue;
        }
        if is_self_message {
            history_messages.extend(build_prepared_history_messages_from_tool_history(
                message,
                MessageToolHistoryView::PromptReplay,
            ));
        }
        let is_user = role == "user";
        let history_extra_blocks = if is_user {
            prompt_user_extra_blocks_for_message(
                message,
                recall_memories.as_deref(),
                &mut seen_prompt_memory_ids,
            )
        } else {
            Vec::new()
        };
        let mut text = if is_user {
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
            // Keep message shape stable for providers that reject empty messages.
            text = " ".to_string();
        }
        history_messages.push(PreparedHistoryMessage {
            role: role.clone(),
            text,
            extra_text_blocks: history_extra_blocks,
            user_time_text: if role == "user" {
                let include_remote_identity = remote_im_contact_key_from_message(message)
                    .map(|key| seen_remote_contacts.insert(key))
                    .unwrap_or(false);
                build_prompt_user_meta_text(
                    message,
                    agents,
                    prompt_user_name,
                    ui_language,
                    include_remote_identity,
                )
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
    let date_timezone_line = prompt_current_date_timezone_line(ui_language);
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
    ) = (
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
    );
    let remote_im_rules_block = prompt_xml_block(
        "remote im contact rules",
        "联系人是特殊用户，不是当前聊天窗口中的直接用户。\n他们的消息来自远程接口接入，应视为独立的外部用户。\n不要把联系人和当前用户混为一谈，也不要混淆回复目标。\n如果需要回复远程联系人，必须调用 `remote_im_send`。",
    );
    let departments_block = build_departments_prompt_block(conversation, agent, departments, ui_language);
    let memory_rag_rule_block = build_memory_rag_rule_block();
    let system_tools_rule_block = build_system_tools_rule_block(
        conversation,
        agent,
        departments,
        ui_language,
    );
    let question_and_planning_rule_block = build_question_and_planning_rule_block();
    let mut preamble = if let Some((user_name, user_intro)) = user_profile {
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
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n",
            highest_instruction_md,
            memory_rag_rule_block,
            system_tools_rule_block.as_deref().unwrap_or(""),
            question_and_planning_rule_block,
            departments_block,
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(
                user_settings_label,
                format!(
                    "{}：{}\n{}：{}",
                    user_nickname_label,
                    xml_escape_prompt(user_name),
                    user_intro_label,
                    xml_escape_prompt(&user_intro_display)
                )
            ),
            prompt_xml_block(
                role_constraints_label,
                format!("{}\n{}", role_identity_text, role_confusion_line)
            ),
            prompt_xml_block(
                conversation_style_label,
                format!("当前风格：{}\n{}", response_style.name, response_style.prompt)
            ),
            prompt_xml_block(
                language_settings_label,
                format!(
                    "{}\n{}\n{}",
                    language_instruction, language_follow_user_line, date_timezone_line
                )
            )
        )
    } else {
        let delegate_role_line = "- 这是一条委托线程。此线程不存在默认用户人格。";
        let delegate_scope_line =
            "- 只依据本轮委托任务块与本线程历史处理工作，不要自行补充用户设定、昵称或主会话背景。";
        format!(
            "{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n\
{}\n",
            highest_instruction_md,
            memory_rag_rule_block,
            system_tools_rule_block.as_deref().unwrap_or(""),
            question_and_planning_rule_block,
            departments_block,
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(
                role_constraints_label,
                format!("{}\n{}", delegate_role_line, delegate_scope_line)
            ),
            prompt_xml_block(
                conversation_style_label,
                format!("当前风格：{}\n{}", response_style.name, response_style.prompt)
            ),
            prompt_xml_block(
                language_settings_label,
                format!("{}\n{}", language_instruction, date_timezone_line)
            ),
        )
    };
    if !remote_im_rules_block.trim().is_empty() {
        if !preamble.ends_with('\n') {
            preamble.push('\n');
        }
        preamble.push('\n');
        preamble.push_str(&remote_im_rules_block);
        preamble.push('\n');
    }

    let latest_user = latest_user_index
        .and_then(|idx| enriched_conversation.messages.get(idx).cloned());

    let mut latest_user_text = String::new();
    let mut latest_user_meta_text = String::new();
    let mut latest_user_extra_text = String::new();
    let mut latest_images = Vec::<(String, String)>::new();
    let mut latest_audios = Vec::<(String, String)>::new();

    if let Some(msg) = latest_user {
        let latest_user_text_rendered = render_prompt_user_text_only(&msg);
        let (resolved_images, resolved_audios) =
            resolve_media_from_parts(&msg.parts, data_path, "[提示词] 最新消息");
        let latest_extra_blocks = prompt_user_extra_blocks_for_message(
            &msg,
            recall_memories.as_deref(),
            &mut seen_prompt_memory_ids,
        );
        let include_remote_identity = remote_im_contact_key_from_message(&msg)
            .map(|key| seen_remote_contacts.insert(key))
            .unwrap_or(false);
        latest_user_meta_text = build_prompt_user_meta_text(
            &msg,
            agents,
            prompt_user_name,
            ui_language,
            include_remote_identity,
        )
        .unwrap_or_default();
        latest_user_text = latest_user_text_rendered;
        latest_images = resolved_images;
        latest_audios = resolved_audios;
        for extra in latest_extra_blocks {
            if extra.trim().is_empty() {
                continue;
            }
            if !latest_user_extra_text.is_empty() {
                latest_user_extra_text.push('\n');
            }
            latest_user_extra_text.push_str(&extra);
        }
        if latest_user_text.trim().is_empty()
            && latest_user_meta_text.trim().is_empty()
            && latest_user_extra_text.trim().is_empty()
            && latest_images.is_empty()
            && latest_audios.is_empty()
        {
            latest_user_text = " ".to_string();
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
