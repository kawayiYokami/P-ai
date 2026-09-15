#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonlSnapshotMessageLine {
    kind: String,
    message: ChatMessage,
}

const JSONL_SNAPSHOT_MESSAGE_KIND: &str = "message";

fn encode_jsonl_snapshot_message(message: &ChatMessage) -> Result<String, String> {
    let line = JsonlSnapshotMessageLine {
        kind: JSONL_SNAPSHOT_MESSAGE_KIND.to_string(),
        message: message.clone(),
    };
    serde_json::to_string(&line)
        .map(|value| format!("{value}\n"))
        .map_err(|err| format!("序列化 JSONL 消息失败: {err}"))
}

fn decode_jsonl_snapshot_message(line: &str) -> Result<ChatMessage, String> {
    let parsed: JsonlSnapshotMessageLine =
        serde_json::from_str(line).map_err(|err| format!("解析 JSONL 消息失败: {err}"))?;
    if parsed.kind.trim() != JSONL_SNAPSHOT_MESSAGE_KIND {
        return Err(format!("不支持的 JSONL 消息类型: {}", parsed.kind));
    }
    Ok(parsed.message)
}

fn encode_jsonl_snapshot_messages(messages: &[ChatMessage]) -> Result<String, String> {
    let mut out = String::new();
    for message in messages {
        out.push_str(&encode_jsonl_snapshot_message(message)?);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsonlSnapshotConversationBlock {
    block_id: u32,
    block_file: String,
    content: String,
    index_items: Vec<MessageStoreIndexItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsonlSnapshotConversationBlocks {
    blocks: Vec<JsonlSnapshotConversationBlock>,
    index: MessageStoreIndexFile,
    message_count: usize,
    last_message_id: String,
    total_bytes: u64,
}

#[derive(Debug, Clone)]
struct ConversationBlockMessageRefs<'a> {
    block_id: u32,
    block_file: String,
    messages: Vec<&'a ChatMessage>,
}

fn build_jsonl_snapshot_conversation_blocks(
    messages: &[ChatMessage],
) -> Result<JsonlSnapshotConversationBlocks, String> {
    build_jsonl_snapshot_conversation_blocks_from_refs(
        false,
        &split_messages_into_conversation_blocks(messages),
    )
}

fn build_jsonl_snapshot_conversation_blocks_for_conversation(
    conversation: &Conversation,
) -> Result<JsonlSnapshotConversationBlocks, String> {
    build_jsonl_snapshot_conversation_blocks_from_refs(
        conversation.status.trim() == "archived",
        &split_conversation_messages_into_blocks(conversation),
    )
}

/// V4 整会话构建（生产 sqlite 写快照用）：消息拆多行组 + 组级 locator + `.jsonl.zstd` 块文件。
/// V1→V2 / V2→V3 迁移保持明文单行（`build_jsonl_snapshot_conversation_blocks_for_conversation`），
/// 生产写快照切这里；`total_bytes` 仍按明文组字节数计（manifest total_bytes 在 v3 后不参与运行期校验）。
fn build_jsonl_snapshot_conversation_blocks_for_conversation_v4(
    conversation: &Conversation,
) -> Result<JsonlSnapshotConversationBlocks, String> {
    let source_blocks = split_conversation_messages_into_blocks(conversation);
    let archived_conversation = conversation.status.trim() == "archived";
    let message_count = source_blocks
        .iter()
        .map(|block| block.messages.len())
        .sum::<usize>();
    let mut blocks = Vec::<JsonlSnapshotConversationBlock>::with_capacity(source_blocks.len());
    let mut all_items = Vec::<MessageStoreIndexItem>::with_capacity(message_count);
    let mut total_bytes = 0_u64;
    let mut last_message_id = String::new();

    for (block_idx, block_messages) in source_blocks.iter().enumerate() {
        let should_slim =
            should_slim_conversation_block(archived_conversation, block_idx, source_blocks.len());
        let block = build_jsonl_snapshot_conversation_block_v4(block_messages, should_slim)?;
        last_message_id = block
            .index_items
            .last()
            .map(|item| item.message_id.clone())
            .unwrap_or(last_message_id);
        total_bytes = total_bytes
            .checked_add(block.content.as_bytes().len() as u64)
            .ok_or_else(|| format!("构建会话块失败：总字节数溢出，block_file={}", block.block_file))?;
        all_items.extend(block.index_items.iter().cloned());
        blocks.push(block);
    }

    Ok(JsonlSnapshotConversationBlocks {
        blocks,
        index: MessageStoreIndexFile::new(MESSAGE_STORE_MANIFEST_VERSION, all_items),
        message_count,
        last_message_id,
        total_bytes,
    })
}

fn build_jsonl_snapshot_conversation_blocks_from_refs(
    archived_conversation: bool,
    source_blocks: &[ConversationBlockMessageRefs<'_>],
) -> Result<JsonlSnapshotConversationBlocks, String> {
    let message_count = source_blocks
        .iter()
        .map(|block| block.messages.len())
        .sum::<usize>();
    let mut blocks = Vec::<JsonlSnapshotConversationBlock>::with_capacity(source_blocks.len());
    let mut all_items = Vec::<MessageStoreIndexItem>::with_capacity(message_count);
    let mut total_bytes = 0_u64;
    let mut last_message_id = String::new();

    for (block_idx, block_messages) in source_blocks.iter().enumerate() {
        let should_slim =
            should_slim_conversation_block(archived_conversation, block_idx, source_blocks.len());
        let block = build_jsonl_snapshot_conversation_block(block_messages, should_slim)?;
        last_message_id = block
            .index_items
            .last()
            .map(|item| item.message_id.clone())
            .unwrap_or(last_message_id);
        total_bytes = total_bytes
            .checked_add(block.content.as_bytes().len() as u64)
            .ok_or_else(|| format!("构建会话块失败：总字节数溢出，block_file={}", block.block_file))?;
        all_items.extend(block.index_items.iter().cloned());
        blocks.push(block);
    }

    Ok(JsonlSnapshotConversationBlocks {
        blocks,
        index: MessageStoreIndexFile::new(MESSAGE_STORE_MANIFEST_VERSION, all_items),
        message_count,
        last_message_id,
        total_bytes,
    })
}

fn split_conversation_messages_into_blocks(
    conversation: &Conversation,
) -> Vec<ConversationBlockMessageRefs<'_>> {
    split_messages_into_conversation_blocks(&conversation.messages)
}

fn split_messages_into_conversation_blocks(
    messages: &[ChatMessage],
) -> Vec<ConversationBlockMessageRefs<'_>> {
    let mut raw_blocks = Vec::<Vec<&ChatMessage>>::new();
    let mut current = Vec::<&ChatMessage>::new();
    for message in messages {
        if message_store_compaction_kind(message).is_some() && !current.is_empty() {
            raw_blocks.push(current);
            current = Vec::new();
        }
        current.push(message);
    }
    if !current.is_empty() {
        raw_blocks.push(current);
    }
    raw_blocks
        .into_iter()
        .enumerate()
        .map(|(idx, messages)| ConversationBlockMessageRefs {
            block_id: idx as u32,
            block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{idx:06}.jsonl"),
            messages,
        })
        .collect()
}

fn should_slim_conversation_block(
    archived_conversation: bool,
    _block_idx: usize,
    _block_count: usize,
) -> bool {
    archived_conversation
}

fn raw_blocks_to_conversation_block_refs(
    raw_blocks: Vec<Vec<&ChatMessage>>,
) -> Vec<ConversationBlockMessageRefs<'_>> {
    raw_blocks
        .into_iter()
        .enumerate()
        .map(|(idx, messages)| ConversationBlockMessageRefs {
            block_id: idx as u32,
            block_file: format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{idx:06}.jsonl"),
            messages,
        })
        .collect()
}

fn message_store_message_day_key(message: &ChatMessage) -> String {
    message_store_message_business_day_key(&message.created_at).unwrap_or_else(|| {
        message
            .created_at
            .split('T')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
            .to_string()
    })
}

fn message_store_message_business_day_key(created_at: &str) -> Option<String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(created_at.trim()).ok()?;
    let local = parsed.with_timezone(&chrono::Local);
    let day = if local.time() < chrono::NaiveTime::from_hms_opt(4, 0, 0)? {
        local.date_naive().pred_opt()?
    } else {
        local.date_naive()
    };
    Some(day.format("%Y-%m-%d").to_string())
}

fn build_jsonl_snapshot_conversation_block(
    block: &ConversationBlockMessageRefs<'_>,
    should_slim: bool,
) -> Result<JsonlSnapshotConversationBlock, String> {
    let mut content = String::new();
    let mut offset = 0_u64;
    let mut block_items = Vec::<MessageStoreIndexItem>::with_capacity(block.messages.len());

    for message in &block.messages {
        let stored_message = if should_slim {
            slim_older_conversation_block_message(message)
        } else {
            (*message).clone()
        };
        let encoded = encode_jsonl_snapshot_message(&stored_message)?;
        let byte_len = encoded.as_bytes().len() as u64;
        let item = message_store_index_item_for_message_in_block(
            &stored_message,
            Some(block.block_id),
            offset,
            byte_len,
        );
        if item.compaction_kind.is_none() && message_store_compaction_kind(message).is_some() {
            return Err(format!(
                "构建会话块失败：瘦身后丢失压缩边界，message_id={}",
                message.id
            ));
        }
        content.push_str(&encoded);
        offset = offset.checked_add(byte_len).ok_or_else(|| {
            format!(
                "构建会话块失败：block offset 溢出，block_file={}，message_id={}",
                block.block_file, message.id
            )
        })?;
        block_items.push(item);
    }

    Ok(JsonlSnapshotConversationBlock {
        block_id: block.block_id,
        block_file: block.block_file.clone(),
        content,
        index_items: block_items,
    })
}

/// 尾部重建：复用目标行之前的原始文件字节，只重新序列化目标行及之后。
///
/// 适用场景：追加（新行在文件尾，前面全部复用）与追加替换（工具事件追加，目标行在块尾）。
/// 前置条件：`prefix_bytes` 必须是块文件 `[0, 目标行 offset)` 的原始字节（含换行符），
/// `prefix_items` 对应这些行的 locator（offset/byte_len 不变）；`tail_messages` 是目标行及
/// 之后的消息（目标行已替换）。
///
/// 注意：本函数不支持 slim（slim 会裁剪所有行，前缀字节无法复用）。调用方必须在
/// `should_slim_conversation_block` 为 true 时走整块重建（`build_jsonl_snapshot_conversation_block`）。
fn build_jsonl_snapshot_conversation_block_tail(
    block: &ConversationBlockMessageRefs<'_>,
    prefix_bytes: &[u8],
    prefix_items: &[MessageStoreIndexItem],
    tail_messages: &[ChatMessage],
) -> Result<JsonlSnapshotConversationBlock, String> {
    let mut content_bytes = Vec::with_capacity(prefix_bytes.len() + 4096);
    content_bytes.extend_from_slice(prefix_bytes);
    let mut offset = prefix_bytes.len() as u64;
    let mut block_items = Vec::<MessageStoreIndexItem>::with_capacity(prefix_items.len() + tail_messages.len());
    block_items.extend_from_slice(prefix_items);

    for message in tail_messages {
        let stored_message = (*message).clone();
        let encoded = encode_jsonl_snapshot_message(&stored_message)?;
        let byte_len = encoded.as_bytes().len() as u64;
        let item = message_store_index_item_for_message_in_block(
            &stored_message,
            Some(block.block_id),
            offset,
            byte_len,
        );
        if item.compaction_kind.is_none() && message_store_compaction_kind(message).is_some() {
            return Err(format!(
                "构建会话块失败：瘦身后丢失压缩边界，message_id={}",
                message.id
            ));
        }
        content_bytes.extend_from_slice(encoded.as_bytes());
        offset = offset.checked_add(byte_len).ok_or_else(|| {
            format!(
                "构建会话块失败：block offset 溢出，block_file={}，message_id={}",
                block.block_file, message.id
            )
        })?;
        block_items.push(item);
    }

    let content = String::from_utf8(content_bytes)
        .map_err(|err| format!("构建会话块失败：块内容不是合法 UTF-8，block_file={}，error={err}", block.block_file))?;
    Ok(JsonlSnapshotConversationBlock {
        block_id: block.block_id,
        block_file: block.block_file.clone(),
        content,
        index_items: block_items,
    })
}

// ==================== V4 多行组构建（生产路径） ====================
//
// V4 块 = zstd 压缩的多行组明文（D12/D15）：
// - 每条消息 = 一组多行（工具行/正文行/普通消息单行），明文坐标 = 组级
//   （byte_offset 指向组首行、byte_len 覆盖整组多行）
// - 块文件后缀 `.jsonl.zstd`；写入 = 整块单帧压缩
// - V1→V2 / V2→V3 迁移保持明文单行格式，不调用这里

/// V4 块文件名：统一带 `.jsonl.zstd` 后缀（构建与写入共用，避免后缀漂移）
fn jsonl_snapshot_block_file_v4(block_id: u32) -> String {
    format!("{MESSAGE_STORE_BLOCKS_DIR_NAME}/{block_id:06}.jsonl.zstd")
}

/// V4 整块重建（replace/归档/compaction）：消息拆多行组 → 明文拼接 → 组级 locator
fn build_jsonl_snapshot_conversation_block_v4(
    block: &ConversationBlockMessageRefs<'_>,
    should_slim: bool,
) -> Result<JsonlSnapshotConversationBlock, String> {
    let mut content = String::new();
    let mut offset = 0_u64;
    let mut block_items = Vec::<MessageStoreIndexItem>::with_capacity(block.messages.len());

    for message in &block.messages {
        let stored_message = if should_slim {
            slim_older_conversation_block_message(message)
        } else {
            (*message).clone()
        };
        let lines = split_message_into_group_lines(&stored_message)?;
        let group_len = lines.iter().map(|line| line.as_bytes().len() as u64).sum::<u64>();
        let item = message_store_index_item_for_message_in_block(
            &stored_message,
            Some(block.block_id),
            offset,
            group_len,
        );
        if item.compaction_kind.is_none() && message_store_compaction_kind(message).is_some() {
            return Err(format!(
                "构建会话块失败：瘦身后丢失压缩边界，message_id={}",
                message.id
            ));
        }
        for line in &lines {
            content.push_str(&line);
        }
        offset = offset.checked_add(group_len).ok_or_else(|| {
            format!(
                "构建会话块失败：block offset 溢出，block_file={}，message_id={}",
                block.block_file, message.id
            )
        })?;
        block_items.push(item);
    }

    Ok(JsonlSnapshotConversationBlock {
        block_id: block.block_id,
        block_file: jsonl_snapshot_block_file_v4(block.block_id),
        content,
        index_items: block_items,
    })
}

/// V4 整块压缩写入（原子）：明文多行组 → 单帧 zstd → 原子替换；返回压缩字节数
fn write_jsonl_snapshot_block_v4_atomic(path: &PathBuf, plain_content: &str) -> Result<usize, String> {
    let compressed = zstd_compress_block(plain_content.as_bytes())?;
    write_message_store_bytes_atomic(path, "jsonl.zstd.tmp", &compressed, "V4 压缩块")?;
    Ok(compressed.len())
}

/// V4 尾部重建：复用目标行之前的明文前缀字节，只把新追加的消息拆成组行拼接。
///
/// 适用场景：延续尾块 + 多组新消息（物理追加快速路径之外的尾块重建）。
/// 前置条件：`prefix_bytes` 是块文件解压后的明文 `[0, 目标行 offset)` 原始字节（含换行符），
/// `prefix_items` 对应这些组的 locator（offset/byte_len 不变，组级）；`tail_messages` 是
/// 要追加的消息（拆成多行组）。
///
/// 注意：不支持 slim（slim 会裁剪所有行，前缀字节无法复用）。调用方必须在
/// `should_slim_conversation_block` 为 true 时走整块重建（`build_jsonl_snapshot_conversation_block_v4`）。
fn build_jsonl_snapshot_conversation_block_tail_v4(
    block: &ConversationBlockMessageRefs<'_>,
    prefix_bytes: &[u8],
    prefix_items: &[MessageStoreIndexItem],
    tail_messages: &[ChatMessage],
) -> Result<JsonlSnapshotConversationBlock, String> {
    let mut content_bytes = Vec::with_capacity(prefix_bytes.len() + 4096);
    content_bytes.extend_from_slice(prefix_bytes);
    let mut offset = prefix_bytes.len() as u64;
    let mut block_items = Vec::<MessageStoreIndexItem>::with_capacity(prefix_items.len() + tail_messages.len());
    block_items.extend_from_slice(prefix_items);

    for message in tail_messages {
        let stored_message = (*message).clone();
        let lines = split_message_into_group_lines(&stored_message)?;
        let group_len = lines.iter().map(|line| line.as_bytes().len() as u64).sum::<u64>();
        let item = message_store_index_item_for_message_in_block(
            &stored_message,
            Some(block.block_id),
            offset,
            group_len,
        );
        if item.compaction_kind.is_none() && message_store_compaction_kind(message).is_some() {
            return Err(format!(
                "构建会话块失败：瘦身后丢失压缩边界，message_id={}",
                message.id
            ));
        }
        for line in &lines {
            content_bytes.extend_from_slice(line.as_bytes());
        }
        offset = offset.checked_add(group_len).ok_or_else(|| {
            format!(
                "构建会话块失败：block offset 溢出，block_file={}，message_id={}",
                block.block_file, message.id
            )
        })?;
        block_items.push(item);
    }

    let content = String::from_utf8(content_bytes)
        .map_err(|err| format!("构建会话块失败：块内容不是合法 UTF-8，block_file={}，error={err}", block.block_file))?;
    Ok(JsonlSnapshotConversationBlock {
        block_id: block.block_id,
        block_file: jsonl_snapshot_block_file_v4(block.block_id),
        content,
        index_items: block_items,
    })
}

fn slim_older_conversation_block_message(message: &ChatMessage) -> ChatMessage {
    let mut next = message.clone();
    next.parts = message
        .parts
        .iter()
        .filter_map(slim_older_conversation_block_part)
        .collect();
    next.extra_text_blocks.clear();
    next.provider_meta = slim_older_conversation_block_provider_meta(message);
    next.tool_call = None;
    next.mcp_call = None;
    next
}

fn slim_older_conversation_block_part(part: &MessagePart) -> Option<MessagePart> {
    match part {
        MessagePart::Text {
            text,
            reasoning_content,
        } => Some(MessagePart::Text {
            text: text.clone(),
            reasoning_content: reasoning_content.clone(),
        }),
        MessagePart::Image {
            mime,
            bytes_base64,
            name,
            compressed,
        } => {
            let trimmed = bytes_base64.trim();
            if !(trimmed.starts_with("@media:")
                || trimmed.starts_with("@download:")
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://"))
            {
                return None;
            }
            Some(MessagePart::Image {
                mime: mime.clone(),
                bytes_base64: bytes_base64.clone(),
                name: name.clone(),
                compressed: *compressed,
            })
        }
        MessagePart::Audio {
            mime,
            bytes_base64,
            name,
            compressed,
        } => {
            let trimmed = bytes_base64.trim();
            if !(trimmed.starts_with("@media:")
                || trimmed.starts_with("@download:")
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://"))
            {
                return None;
            }
            Some(MessagePart::Audio {
                mime: mime.clone(),
                bytes_base64: bytes_base64.clone(),
                name: name.clone(),
                compressed: *compressed,
            })
        }
        MessagePart::Attachment { path, mime, name } => Some(MessagePart::Attachment {
            path: path.clone(),
            mime: mime.clone(),
            name: name.clone(),
        }),
    }
}

fn slim_older_conversation_block_provider_meta(message: &ChatMessage) -> Option<Value> {
    let mut meta = serde_json::Map::new();
    if let Some(kind) = message_store_compaction_kind(message) {
        meta.insert(
            "message_meta".to_string(),
            serde_json::json!({
                "kind": kind,
            }),
        );
    }
    if let Some(origin) = remote_im_origin_from_message(message) {
        meta.insert("origin".to_string(), origin.clone());
    }
    if meta.is_empty() {
        None
    } else {
        Some(Value::Object(meta))
    }
}

fn write_jsonl_snapshot_atomic(path: &PathBuf, content: &str) -> Result<(), String> {
    write_message_store_text_atomic(path, "jsonl.tmp", content, "JSONL 快照")
}

#[cfg(test)]
mod jsonl_snapshot_conversation_block_tests {
    use super::*;

    fn text_message(id: &str, role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: "2026-04-25T00:00:00Z".to_string(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }
    }

    fn summary_seed_message(id: &str) -> ChatMessage {
        let mut message = text_message(id, "assistant", "summary");
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "summary_context_seed",
            },
            "runtime": {
                "shouldBeDroppedInOldBlocks": true,
            },
        }));
        message
    }

    fn legacy_media_message_json(id: &str, part_type: &str, mime: &str, stored: &str) -> String {
        serde_json::json!({
            "kind": "message",
            "message": {
                "id": id,
                "role": "user",
                "createdAt": "2026-04-25T00:00:00Z",
                "speakerAgentId": null,
                "parts": [{
                    "type": part_type,
                    "mime": mime,
                    "bytesBase64": stored,
                    "name": "legacy.bin",
                    "compressed": false
                }],
                "extraTextBlocks": [],
                "providerMeta": null,
                "toolCall": null,
                "mcpCall": null,
                "memeAnnotations": null
            }
        })
        .to_string()
    }

    #[test]
    fn decode_jsonl_snapshot_message_should_accept_legacy_image_and_audio_wire() {
        let image = decode_jsonl_snapshot_message(&legacy_media_message_json(
            "legacy-image",
            "image",
            "image/png",
            "@download:conversation-a/image.png",
        ))
        .expect("decode legacy image");
        let audio = decode_jsonl_snapshot_message(&legacy_media_message_json(
            "legacy-audio",
            "audio",
            "audio/webm",
            "@media:audio.webm",
        ))
        .expect("decode legacy audio");

        assert!(matches!(image.parts.first(), Some(MessagePart::Image { .. })));
        assert!(matches!(audio.parts.first(), Some(MessagePart::Audio { .. })));
    }

    #[test]
    fn canonical_attachment_json_should_not_contain_legacy_binary_fields() {
        let mut message = text_message("canonical-attachment", "user", "look");
        message.parts.push(MessagePart::Attachment {
            path: "C:/attachments/a.png".to_string(),
            mime: "image/png".to_string(),
            name: "a.png".to_string(),
        });

        let json = encode_jsonl_snapshot_message(&message).expect("encode canonical attachment");

        assert!(json.contains("\"type\":\"attachment\""));
        assert!(json.contains("C:/attachments/a.png"));
        assert!(!json.contains("bytesBase64"));
        assert!(!json.contains("@download:"));
        assert!(!json.contains("@media:"));
    }

    #[test]
    fn slim_older_conversation_block_part_should_keep_text_reasoning_content() {
        let part = MessagePart::Text {
            text: "最终回答".to_string(),
            reasoning_content: Some("最终思考".to_string()),
        };

        let slimmed = slim_older_conversation_block_part(&part).expect("slimmed text part");

        match slimmed {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, "最终回答");
                assert_eq!(reasoning_content.as_deref(), Some("最终思考"));
            }
            other => panic!("unexpected part: {:?}", other),
        }
    }

    #[test]
    fn slim_older_conversation_block_message_should_keep_remote_origin_only() {
        let mut message = text_message("remote-user", "user", "旧消息");
        message.parts.push(MessagePart::Image {
            mime: "image/png".to_string(),
            bytes_base64: "iVBORw0KGgoAAA".to_string(),
            name: Some("inline.png".to_string()),
            compressed: false,
        });
        message.provider_meta = Some(serde_json::json!({
            "origin": {
                "kind": "remote_im",
                "channel_id": "channel-a",
                "contact_type": "group",
                "contact_id": "group-1",
                "contact_name": "测试群",
                "sender_id": "member-001",
                "sender_name": "群友甲"
            },
            "runtime": {
                "temporary": true
            }
        }));
        message.tool_call = Some(vec![serde_json::json!({"name": "heavy_tool"})]);
        message.mcp_call = Some(vec![serde_json::json!({"name": "heavy_mcp"})]);

        let slimmed = slim_older_conversation_block_message(&message);

        assert_eq!(slimmed.parts.len(), 1);
        assert!(slimmed.tool_call.is_none());
        assert!(slimmed.mcp_call.is_none());
        let meta = slimmed.provider_meta.expect("remote origin meta");
        assert_eq!(
            meta.pointer("/origin/sender_name").and_then(Value::as_str),
            Some("群友甲")
        );
        assert_eq!(
            meta.pointer("/origin/channel_id").and_then(Value::as_str),
            Some("channel-a")
        );
        assert!(meta.get("runtime").is_none());
    }

    #[test]
    fn conversation_blocks_should_split_at_compaction_seed_and_slim_only_when_archived() {
        let mut first = text_message("m1", "assistant", "first");
        first.extra_text_blocks.push("memory widget".to_string());
        first.tool_call = Some(vec![serde_json::json!({"name": "tool"})]);
        let mut messages = vec![first, summary_seed_message("s1")];
        messages.push(text_message("m2", "user", "second"));
        messages.push(summary_seed_message("s2"));
        messages.push(text_message("m3", "user", "third"));
        messages.push(summary_seed_message("s3"));
        messages.push(text_message("m4", "user", "latest"));

        let mut conversation = test_conversation_for_blocks(messages);
        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build active blocks");

        assert_eq!(blocks.blocks.len(), 4);
        assert_eq!(blocks.blocks[0].block_file, "blocks/000000.jsonl");
        assert_eq!(blocks.index.items[1].block_id, Some(1));
        // 活跃会话不瘦身：块 0 保留 extraTextBlocks 与 toolCall
        assert!(blocks.blocks[0].content.contains("memory widget"));
        assert!(blocks.blocks[0].content.contains("\"toolCall\""));
        assert!(blocks.blocks[2].content.contains("summary_context_seed"));
        assert!(blocks.blocks[3].content.contains("latest"));

        // 归档会话全瘦身：extraTextBlocks 清空、toolCall 置 null
        conversation.status = "archived".to_string();
        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build archived blocks");
        assert_eq!(blocks.blocks.len(), 4);
        assert!(blocks.blocks[0].content.contains("\"extraTextBlocks\":[]"));
        assert!(blocks.blocks[0].content.contains("\"toolCall\":null"));
        assert!(blocks.blocks[2].content.contains("summary_context_seed"));
        assert!(blocks.blocks[3].content.contains("latest"));
    }

    #[test]
    fn remote_im_conversation_without_compaction_should_keep_one_block_across_days() {
        let mut conversation = test_conversation_for_blocks(vec![
            text_message_at("m1", "user", "day 1", "2026-04-20T10:00:00Z"),
            text_message_at("m2", "assistant", "day 1 reply", "2026-04-20T10:01:00Z"),
            text_message_at("m3", "user", "day 2", "2026-04-21T10:00:00Z"),
        ]);
        conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();

        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build remote im blocks");

        assert_eq!(blocks.blocks.len(), 1);
        assert_eq!(blocks.index.items[0].block_id, Some(0));
        assert_eq!(blocks.index.items[1].block_id, Some(0));
        assert_eq!(blocks.index.items[2].block_id, Some(0));
    }

    #[test]
    fn remote_im_conversation_should_not_split_at_four_am_boundary() {
        let mut conversation = test_conversation_for_blocks(vec![
            text_message_at("m1", "user", "late night", "2026-04-20T19:00:00Z"),
            text_message_at("m2", "assistant", "before 4am local", "2026-04-20T19:30:00Z"),
            text_message_at("m3", "user", "after 4am local", "2026-04-20T20:10:00Z"),
        ]);
        conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();

        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build remote im day boundary blocks");

        assert_eq!(blocks.blocks.len(), 1);
        assert_eq!(blocks.index.items[0].block_id, Some(0));
        assert_eq!(blocks.index.items[1].block_id, Some(0));
        assert_eq!(blocks.index.items[2].block_id, Some(0));
    }

    #[test]
    fn remote_im_conversation_should_only_split_at_compaction_boundary() {
        let mut messages = vec![
            text_message_at("m1", "user", "day 1", "2026-04-20T10:00:00Z"),
            text_message_at("m2", "assistant", "day 2 before compaction", "2026-04-21T10:00:00Z"),
            summary_seed_message("s1"),
            text_message_at("m3", "user", "day 2 after compaction", "2026-04-21T10:01:00Z"),
        ];
        messages[2].created_at = "2026-04-21T10:00:30Z".to_string();
        let mut conversation = test_conversation_for_blocks(messages);
        conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();

        let blocks = build_jsonl_snapshot_conversation_blocks_for_conversation(&conversation)
            .expect("build remote im compaction blocks");

        assert_eq!(blocks.blocks.len(), 2);
        assert_eq!(blocks.index.items[0].block_id, Some(0));
        assert_eq!(blocks.index.items[1].block_id, Some(0));
        assert_eq!(blocks.index.items[2].block_id, Some(1));
        assert_eq!(blocks.index.items[3].block_id, Some(1));
    }

    fn text_message_at(id: &str, role: &str, text: &str, created_at: &str) -> ChatMessage {
        let mut message = text_message(id, role, text);
        message.created_at = created_at.to_string();
        message
    }

    #[test]
    fn tail_rebuild_should_equal_full_rebuild_for_replace_last_message() {
        let block_id = 0u32;
        let block_file = format!("blocks/{block_id:06}.jsonl");
        let original = vec![
            text_message("m1", "user", "first"),
            text_message("m2", "assistant", "second"),
            text_message("m3", "user", "third"),
            text_message("m4", "assistant", "fourth"),
        ];
        let full_refs = ConversationBlockMessageRefs {
            block_id,
            block_file: block_file.clone(),
            messages: original.iter().collect(),
        };
        let full_block = build_jsonl_snapshot_conversation_block(&full_refs, false)
            .expect("full rebuild");

        // 追加替换：目标行在块尾，前缀字节原样复用，只重建最后一行
        let mut replaced = original.clone();
        replaced[3] = text_message("m4", "assistant", "fourth-updated");
        let target_offset = full_block.index_items[3].offset as usize;
        let prefix_bytes = &full_block.content.as_bytes()[0..target_offset];
        let prefix_items = full_block.index_items[..3].to_vec();
        let tail_refs = ConversationBlockMessageRefs {
            block_id,
            block_file: block_file.clone(),
            messages: replaced[3..].iter().collect(),
        };
        let tail_block = build_jsonl_snapshot_conversation_block_tail(
            &tail_refs,
            prefix_bytes,
            &prefix_items,
            &replaced[3..],
        )
        .expect("tail rebuild");

        let expected_refs = ConversationBlockMessageRefs {
            block_id,
            block_file,
            messages: replaced.iter().collect(),
        };
        let expected_block = build_jsonl_snapshot_conversation_block(&expected_refs, false)
            .expect("expected full rebuild");

        assert_eq!(tail_block.content, expected_block.content);
        assert_eq!(tail_block.index_items, expected_block.index_items);
        // 前缀字节确实是原样复用，没有重新序列化
        assert!(tail_block.content.starts_with(&full_block.content[..target_offset]));
        // 前缀行的 offset/byte_len 保持不变
        assert_eq!(&tail_block.index_items[..3], &full_block.index_items[..3]);
    }

    #[test]
    fn tail_rebuild_should_equal_full_rebuild_for_append_at_end() {
        let block_id = 0u32;
        let block_file = format!("blocks/{block_id:06}.jsonl");
        let existing = vec![
            text_message("m1", "user", "first"),
            text_message("m2", "assistant", "second"),
            text_message("m3", "user", "third"),
        ];
        let full_refs = ConversationBlockMessageRefs {
            block_id,
            block_file: block_file.clone(),
            messages: existing.iter().collect(),
        };
        let full_block = build_jsonl_snapshot_conversation_block(&full_refs, false)
            .expect("full rebuild");

        // 普通追加：新行在文件尾，整个原文件字节都是前缀
        let appended = vec![text_message("m4", "assistant", "fourth")];
        let prefix_items = full_block.index_items.clone();
        let tail_refs = ConversationBlockMessageRefs {
            block_id,
            block_file: block_file.clone(),
            messages: appended.iter().collect(),
        };
        let tail_block = build_jsonl_snapshot_conversation_block_tail(
            &tail_refs,
            full_block.content.as_bytes(),
            &prefix_items,
            &appended,
        )
        .expect("tail append rebuild");

        let mut expected = existing.clone();
        expected.extend(appended.clone());
        let expected_refs = ConversationBlockMessageRefs {
            block_id,
            block_file,
            messages: expected.iter().collect(),
        };
        let expected_block = build_jsonl_snapshot_conversation_block(&expected_refs, false)
            .expect("expected full rebuild");

        assert_eq!(tail_block.content, expected_block.content);
        assert_eq!(tail_block.index_items, expected_block.index_items);
        // 新消息 offset 从原文件字节长度开始
        let appended_offset = tail_block.index_items[3].offset as usize;
        assert_eq!(appended_offset, full_block.content.len());
    }

    fn test_conversation_for_blocks(messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "conversation-block-test".to_string(),
            title: "test".to_string(),
            agent_id: "agent".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-20T00:00:00Z".to_string(),
            updated_at: "2026-04-20T00:00:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
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
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }
}