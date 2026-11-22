// ==================== OneBot v11 事件消费 ====================

#[derive(Debug, Clone)]
enum OnebotInboundMediaKind {
    Image,
    File,
}

impl OnebotInboundMediaKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone)]
struct OnebotInboundMediaRef {
    kind: OnebotInboundMediaKind,
    file_ref: String,
    file_id: Option<String>,
    file_name: Option<String>,
    mime_hint: Option<String>,
}

#[derive(Debug, Clone)]
enum OnebotEmbeddedRefKind {
    Reply,
    Forward,
}

#[derive(Debug, Clone)]
struct OnebotEmbeddedRef {
    kind: OnebotEmbeddedRefKind,
    id: String,
}

#[derive(Debug, Clone)]
struct OnebotMentionRef {
    qq: String,
    placeholder: String,
}

#[derive(Debug, Clone)]
enum OnebotParsedSegment {
    Text(String),
    Media(OnebotInboundMediaRef),
    Embedded(OnebotEmbeddedRef),
}

#[derive(Debug, Clone, Default)]
struct OnebotParsedMessage {
    text: String,
    media_refs: Vec<OnebotInboundMediaRef>,
    embedded_refs: Vec<OnebotEmbeddedRef>,
    mention_refs: Vec<OnebotMentionRef>,
    ordered_segments: Vec<OnebotParsedSegment>,
}

impl OnebotParsedMessage {
    #[cfg(test)]
    fn into_public_parts(self) -> (String, Vec<OnebotInboundMediaRef>, Vec<OnebotEmbeddedRef>) {
        (self.text, self.media_refs, self.embedded_refs)
    }

    fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.text.push_str(text);
            self.ordered_segments
                .push(OnebotParsedSegment::Text(text.to_string()));
        }
    }

    fn push_media(&mut self, media_ref: OnebotInboundMediaRef) {
        self.media_refs.push(media_ref.clone());
        self.ordered_segments
            .push(OnebotParsedSegment::Media(media_ref));
    }

    fn push_embedded(&mut self, embedded_ref: OnebotEmbeddedRef) {
        self.embedded_refs.push(embedded_ref.clone());
        self.ordered_segments
            .push(OnebotParsedSegment::Embedded(embedded_ref));
    }

    fn push_block(&mut self, block: String) {
        let block = block.trim();
        if block.is_empty() {
            return;
        }
        let mut rendered = String::new();
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(block);
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        self.push_text(&rendered);
    }
}

fn onebot_embedded_ref_id(data: Option<&Value>) -> Option<String> {
    data.and_then(|d| onebot_read_id_as_string(d, "id"))
}

fn onebot_scalar_to_trimmed_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn onebot_read_id_as_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(onebot_scalar_to_trimmed_string)
}

fn onebot_data_value_as_string(data: Option<&Value>, key: &str) -> Option<String> {
    data.and_then(|d| d.get(key))
        .and_then(onebot_scalar_to_trimmed_string)
}

fn onebot_read_u64_like(value: &Value, key: &str) -> Option<u64> {
    let raw = value.get(key)?;
    if let Some(id) = raw.as_u64() {
        return Some(id);
    }
    if let Some(id) = raw.as_i64() {
        return u64::try_from(id).ok();
    }
    raw.as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| text.parse::<u64>().ok())
}

fn onebot_truncate_display_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn onebot_value_to_display(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(onebot_truncate_display_text(trimmed, 500))
            }
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value)
            .ok()
            .map(|text| onebot_truncate_display_text(&text, 500)),
    }
}

fn onebot_collect_segment_fields(data: Option<&Value>, keys: &[&str]) -> Vec<(String, String)> {
    let Some(data) = data else {
        return Vec::new();
    };
    keys.iter()
        .filter_map(|key| {
            data.get(*key)
                .and_then(onebot_value_to_display)
                .map(|value| ((*key).to_string(), value))
        })
        .collect()
}

fn onebot_format_segment_quote(title: &str, fields: Vec<(String, String)>) -> String {
    onebot_format_segment_quote_with_body(title, fields, None)
}

fn onebot_format_segment_quote_with_body(
    title: &str,
    fields: Vec<(String, String)>,
    body: Option<&str>,
) -> String {
    let mut lines = Vec::<String>::new();
    lines.push(format!("**{}**", title.trim()));
    for (key, value) in fields {
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() && !value.is_empty() {
            if value.contains('\n') || value.contains("[[PAI_ONEBOT_MENTION_") {
                lines.push(format!("{}:", key));
                lines.extend(value.lines().map(ToOwned::to_owned));
            } else {
                lines.push(format!("{}: {}", key, value));
            }
        }
    }
    if let Some(body) = body.map(str::trim).filter(|value| !value.is_empty()) {
        lines.extend(body.lines().map(ToOwned::to_owned));
    }
    onebot_markdown_quote_block(&lines.join("\n"))
}

fn onebot_mention_placeholder(index: usize) -> String {
    format!("[[PAI_ONEBOT_MENTION_{}]]", index)
}

fn onebot_push_mention(parsed: &mut OnebotParsedMessage, qq: String) {
    let placeholder = onebot_mention_placeholder(parsed.mention_refs.len());
    parsed.mention_refs.push(OnebotMentionRef {
        qq,
        placeholder: placeholder.clone(),
    });
    parsed.push_block(placeholder);
}

fn onebot_media_ref_from_segment_data(
    seg_type: &str,
    data: Option<&Value>,
) -> Option<OnebotInboundMediaRef> {
    let kind = match seg_type {
        "image" => OnebotInboundMediaKind::Image,
        "file" | "record" | "video" => OnebotInboundMediaKind::File,
        _ => return None,
    };
    let file_ref = onebot_data_value_as_string(data, "url")
        .or_else(|| onebot_data_value_as_string(data, "file"))
        .unwrap_or_default();
    let file_id = onebot_data_value_as_string(data, "file_id")
        .or_else(|| onebot_data_value_as_string(data, "fid"))
        .or_else(|| onebot_data_value_as_string(data, "id"));
    if file_ref.is_empty() && file_id.is_none() {
        return None;
    }
    Some(OnebotInboundMediaRef {
        kind,
        file_ref,
        file_id,
        file_name: onebot_data_value_as_string(data, "name"),
        mime_hint: match seg_type {
            "record" => Some("audio/x-silk".to_string()),
            "video" => Some("video/mp4".to_string()),
            _ => None,
        },
    })
}

fn onebot_push_unresolved_media_block(
    parsed: &mut OnebotParsedMessage,
    title: &str,
    data: Option<&Value>,
) {
    let mut fields = onebot_collect_segment_fields(data, &["file", "url", "file_id", "fid", "id", "name"]);
    if fields.is_empty() {
        fields.push(("说明".to_string(), "无法解析媒体引用".to_string()));
    }
    parsed.push_block(onebot_format_segment_quote(title, fields));
}

fn onebot_merge_nested_message(
    parsed: &mut OnebotParsedMessage,
    nested: OnebotParsedMessage,
) -> String {
    let mut text = nested.text;
    for mention in nested.mention_refs {
        let new_placeholder = onebot_mention_placeholder(parsed.mention_refs.len());
        text = text.replace(&mention.placeholder, &new_placeholder);
        parsed.mention_refs.push(OnebotMentionRef {
            qq: mention.qq,
            placeholder: new_placeholder,
        });
    }
    for segment in nested.ordered_segments {
        match segment {
            OnebotParsedSegment::Media(media_ref) => parsed.push_media(media_ref),
            OnebotParsedSegment::Embedded(embedded_ref) => parsed.push_embedded(embedded_ref),
            OnebotParsedSegment::Text(_) => {}
        }
    }
    text
}

fn onebot_push_node_segment(parsed: &mut OnebotParsedMessage, data: Option<&Value>) {
    let Some(data) = data else {
        parsed.push_block(onebot_format_segment_quote("合并转发节点", Vec::new()));
        return;
    };
    if let Some(content) = data.get("content") {
        let nested = onebot_parse_content_value_detail(content);
        let nested_text = onebot_merge_nested_message(parsed, nested);
        let fields = onebot_collect_segment_fields(Some(data), &["nickname", "user_id"]);
        parsed.push_block(onebot_format_segment_quote_with_body(
            "合并转发节点",
            fields,
            Some(nested_text.trim()),
        ));
        return;
    }
    let fields = onebot_collect_segment_fields(Some(data), &["id", "nickname", "user_id"]);
    parsed.push_block(onebot_format_segment_quote("合并转发节点", fields));
}

fn onebot_push_info_segment(
    parsed: &mut OnebotParsedMessage,
    seg_type: &str,
    data: Option<&Value>,
) {
    match seg_type {
        "at" => {
            let qq = onebot_data_value_as_string(data, "qq");
            if let Some(qq) = qq {
                onebot_push_mention(parsed, qq);
            } else {
                parsed.push_block(onebot_format_segment_quote("提及", Vec::new()));
            }
        }
        "face" => parsed.push_block(onebot_format_segment_quote(
            "QQ 表情",
            onebot_collect_segment_fields(data, &["id"]),
        )),
        "rps" => parsed.push_block(onebot_format_segment_quote("猜拳魔法表情", Vec::new())),
        "dice" => parsed.push_block(onebot_format_segment_quote("掷骰子魔法表情", Vec::new())),
        "shake" => parsed.push_block(onebot_format_segment_quote("窗口抖动", Vec::new())),
        "poke" => parsed.push_block(onebot_format_segment_quote(
            "戳一戳",
            onebot_collect_segment_fields(data, &["type", "id", "name"]),
        )),
        "anonymous" => parsed.push_block(onebot_format_segment_quote(
            "匿名消息",
            onebot_collect_segment_fields(data, &["ignore"]),
        )),
        "share" => parsed.push_block(onebot_format_segment_quote(
            "链接分享",
            onebot_collect_segment_fields(data, &["title", "url", "content", "image"]),
        )),
        "contact" => {
            let title = match onebot_data_value_as_string(data, "type").as_deref() {
                Some("group") => "推荐群",
                Some("qq") => "推荐好友",
                _ => "联系人推荐",
            };
            parsed.push_block(onebot_format_segment_quote(
                title,
                onebot_collect_segment_fields(data, &["type", "id"]),
            ));
        }
        "location" => parsed.push_block(onebot_format_segment_quote(
            "位置",
            onebot_collect_segment_fields(data, &["title", "lat", "lon", "content"]),
        )),
        "music" => parsed.push_block(onebot_format_segment_quote(
            "音乐分享",
            onebot_collect_segment_fields(data, &["type", "id", "title", "url", "audio", "content", "image"]),
        )),
        "xml" => parsed.push_block(onebot_format_segment_quote(
            "XML 消息",
            onebot_collect_segment_fields(data, &["data"]),
        )),
        "json" => parsed.push_block(onebot_format_segment_quote(
            "JSON 消息",
            onebot_collect_segment_fields(data, &["data"]),
        )),
        "node" => onebot_push_node_segment(parsed, data),
        _ => {
            let mut fields = onebot_collect_segment_fields(
                data,
                &["id", "type", "name", "title", "url", "content", "data", "file", "file_id"],
            );
            fields.insert(0, ("type".to_string(), seg_type.to_string()));
            parsed.push_block(onebot_format_segment_quote("OneBot 段", fields));
        }
    }
}

/// 从 OneBot v11 message 数组格式中提取文本、媒体引用和嵌入引用
#[cfg(test)]
fn parse_onebot_message_array(
    segments: &[Value],
) -> (String, Vec<OnebotInboundMediaRef>, Vec<OnebotEmbeddedRef>) {
    parse_onebot_message_array_detail(segments).into_public_parts()
}

fn parse_onebot_message_array_detail(segments: &[Value]) -> OnebotParsedMessage {
    let mut parsed = OnebotParsedMessage::default();
    for seg in segments {
        let seg_type = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let data = seg.get("data");
        match seg_type {
            "text" => {
                if let Some(text) = data.and_then(|d| d.get("text")).and_then(|v| v.as_str()) {
                    parsed.push_text(text);
                }
            }
            "image" | "file" | "record" | "video" => {
                if let Some(media_ref) = onebot_media_ref_from_segment_data(seg_type, data) {
                    parsed.push_media(media_ref);
                } else {
                    let title = match seg_type {
                        "image" => "图片",
                        "record" => "语音",
                        "video" => "短视频",
                        _ => "文件",
                    };
                    onebot_push_unresolved_media_block(&mut parsed, title, data);
                }
            }
            "reply" => {
                if let Some(id) = onebot_embedded_ref_id(data) {
                    parsed.push_embedded(OnebotEmbeddedRef {
                        kind: OnebotEmbeddedRefKind::Reply,
                        id,
                    });
                } else {
                    parsed.push_block(onebot_format_segment_quote(
                        "回复引用",
                        vec![("说明".to_string(), "无法解析引用消息".to_string())],
                    ));
                }
            }
            "forward" => {
                if let Some(id) = onebot_embedded_ref_id(data) {
                    parsed.push_embedded(OnebotEmbeddedRef {
                        kind: OnebotEmbeddedRefKind::Forward,
                        id,
                    });
                } else {
                    parsed.push_block(onebot_format_segment_quote(
                        "合并转发",
                        vec![("说明".to_string(), "无法解析转发消息".to_string())],
                    ));
                }
            }
            "" => {}
            _ => onebot_push_info_segment(&mut parsed, seg_type, data),
        }
    }
    parsed.text = parsed.text.trim_matches('\n').to_string();
    parsed
}

fn onebot_unescape_cq_value(raw: &str) -> String {
    raw.replace("&amp;", "&")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&#44;", ",")
}

fn onebot_cq_param_value(params: &str, key: &str) -> Option<String> {
    let target = key.trim();
    if target.is_empty() {
        return None;
    }
    for pair in params.split(',') {
        let Some((raw_key, raw_value)) = pair.split_once('=') else {
            continue;
        };
        if raw_key.trim() != target {
            continue;
        }
        let value = onebot_unescape_cq_value(raw_value.trim());
        if value.is_empty() {
            continue;
        }
        return Some(value);
    }
    None
}

fn onebot_cq_param_pairs(params: &str) -> Vec<(String, String)> {
    params
        .split(',')
        .filter_map(|pair| {
            let (raw_key, raw_value) = pair.split_once('=')?;
            let key = raw_key.trim();
            if key.is_empty() {
                return None;
            }
            let value = onebot_unescape_cq_value(raw_value.trim());
            if value.trim().is_empty() {
                return None;
            }
            Some((key.to_string(), onebot_truncate_display_text(&value, 500)))
        })
        .collect()
}

fn onebot_cq_fields(params: &str, keys: &[&str]) -> Vec<(String, String)> {
    keys.iter()
        .filter_map(|key| {
            onebot_cq_param_value(params, key)
                .map(|value| ((*key).to_string(), onebot_truncate_display_text(&value, 500)))
        })
        .collect()
}

fn onebot_media_ref_from_cq(cq_type: &str, params: &str) -> Option<OnebotInboundMediaRef> {
    match cq_type.trim() {
        "image" => {
            let file_ref = onebot_cq_param_value(params, "url")
                .or_else(|| onebot_cq_param_value(params, "file"))
                .unwrap_or_default();
            let file_id = onebot_cq_param_value(params, "file_id")
                .or_else(|| onebot_cq_param_value(params, "fid"))
                .or_else(|| onebot_cq_param_value(params, "id"));
            if file_ref.is_empty() && file_id.is_none() {
                return None;
            }
            Some(OnebotInboundMediaRef {
                kind: OnebotInboundMediaKind::Image,
                file_ref,
                file_id,
                file_name: onebot_cq_param_value(params, "name"),
                mime_hint: None,
            })
        }
        "file" | "record" | "video" => {
            let file_ref = onebot_cq_param_value(params, "url")
                .or_else(|| onebot_cq_param_value(params, "file"))
                .unwrap_or_default();
            let file_id = onebot_cq_param_value(params, "file_id")
                .or_else(|| onebot_cq_param_value(params, "fid"))
                .or_else(|| onebot_cq_param_value(params, "id"));
            if file_ref.is_empty() && file_id.is_none() {
                return None;
            }
            Some(OnebotInboundMediaRef {
                kind: OnebotInboundMediaKind::File,
                file_ref,
                file_id,
                file_name: onebot_cq_param_value(params, "name"),
                mime_hint: match cq_type.trim() {
                    "record" => Some("audio/x-silk".to_string()),
                    "video" => Some("video/mp4".to_string()),
                    _ => None,
                },
            })
        }
        _ => None,
    }
}

fn onebot_push_cq_info_segment(parsed: &mut OnebotParsedMessage, cq_type: &str, params: &str) {
    match cq_type {
        "at" => {
            if let Some(qq) = onebot_cq_param_value(params, "qq") {
                onebot_push_mention(parsed, qq);
            } else {
                parsed.push_block(onebot_format_segment_quote("提及", Vec::new()));
            }
        }
        "face" => parsed.push_block(onebot_format_segment_quote(
            "QQ 表情",
            onebot_cq_fields(params, &["id"]),
        )),
        "rps" => parsed.push_block(onebot_format_segment_quote("猜拳魔法表情", Vec::new())),
        "dice" => parsed.push_block(onebot_format_segment_quote("掷骰子魔法表情", Vec::new())),
        "shake" => parsed.push_block(onebot_format_segment_quote("窗口抖动", Vec::new())),
        "poke" => parsed.push_block(onebot_format_segment_quote(
            "戳一戳",
            onebot_cq_fields(params, &["type", "id", "name"]),
        )),
        "anonymous" => parsed.push_block(onebot_format_segment_quote(
            "匿名消息",
            onebot_cq_fields(params, &["ignore"]),
        )),
        "share" => parsed.push_block(onebot_format_segment_quote(
            "链接分享",
            onebot_cq_fields(params, &["title", "url", "content", "image"]),
        )),
        "contact" => {
            let title = match onebot_cq_param_value(params, "type").as_deref() {
                Some("group") => "推荐群",
                Some("qq") => "推荐好友",
                _ => "联系人推荐",
            };
            parsed.push_block(onebot_format_segment_quote(
                title,
                onebot_cq_fields(params, &["type", "id"]),
            ));
        }
        "location" => parsed.push_block(onebot_format_segment_quote(
            "位置",
            onebot_cq_fields(params, &["title", "lat", "lon", "content"]),
        )),
        "music" => parsed.push_block(onebot_format_segment_quote(
            "音乐分享",
            onebot_cq_fields(params, &["type", "id", "title", "url", "audio", "content", "image"]),
        )),
        "xml" => parsed.push_block(onebot_format_segment_quote(
            "XML 消息",
            onebot_cq_fields(params, &["data"]),
        )),
        "json" => parsed.push_block(onebot_format_segment_quote(
            "JSON 消息",
            onebot_cq_fields(params, &["data"]),
        )),
        "node" => parsed.push_block(onebot_format_segment_quote(
            "合并转发节点",
            onebot_cq_fields(params, &["id", "nickname", "user_id", "content"]),
        )),
        _ => {
            let mut fields = onebot_cq_param_pairs(params);
            fields.insert(0, ("type".to_string(), cq_type.to_string()));
            parsed.push_block(onebot_format_segment_quote("OneBot 段", fields));
        }
    }
}

/// 从 CQ 码字符串中提取文本、媒体引用与嵌入引用
#[cfg(test)]
fn parse_onebot_cq_string(
    raw: &str,
) -> (String, Vec<OnebotInboundMediaRef>, Vec<OnebotEmbeddedRef>) {
    parse_onebot_cq_string_detail(raw).into_public_parts()
}

fn parse_onebot_cq_string_detail(raw: &str) -> OnebotParsedMessage {
    let mut parsed = OnebotParsedMessage::default();
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let rest = &raw[cursor..];
        let Some(start_rel) = rest.find("[CQ:") else {
            parsed.push_text(&onebot_unescape_cq_value(rest));
            break;
        };
        let start = cursor + start_rel;
        if start > cursor {
            parsed.push_text(&onebot_unescape_cq_value(&raw[cursor..start]));
        }
        let after_start = &raw[(start + 4)..];
        let Some(end_rel) = after_start.find(']') else {
            parsed.push_text(&onebot_unescape_cq_value(&raw[start..]));
            break;
        };
        let cq_body = &after_start[..end_rel];
        cursor = start + 4 + end_rel + 1;

        let (cq_type, params) = cq_body
            .split_once(',')
            .map(|(left, right)| (left.trim(), right))
            .unwrap_or((cq_body.trim(), ""));
        if let Some(media_ref) = onebot_media_ref_from_cq(cq_type, params) {
            parsed.push_media(media_ref);
            continue;
        }
        match cq_type {
            "reply" => {
                if let Some(id) = onebot_cq_param_value(params, "id") {
                    parsed.push_embedded(OnebotEmbeddedRef {
                        kind: OnebotEmbeddedRefKind::Reply,
                        id,
                    });
                } else {
                    parsed.push_block(onebot_format_segment_quote(
                        "回复引用",
                        vec![("说明".to_string(), "无法解析引用消息".to_string())],
                    ));
                }
            }
            "forward" => {
                if let Some(id) = onebot_cq_param_value(params, "id") {
                    parsed.push_embedded(OnebotEmbeddedRef {
                        kind: OnebotEmbeddedRefKind::Forward,
                        id,
                    });
                } else {
                    parsed.push_block(onebot_format_segment_quote(
                        "合并转发",
                        vec![("说明".to_string(), "无法解析转发消息".to_string())],
                    ));
                }
            }
            "" => {}
            _ => onebot_push_cq_info_segment(&mut parsed, cq_type, params),
        }
    }
    parsed.text = parsed.text.trim_matches('\n').to_string();
    parsed
}

#[cfg(test)]
fn extract_message_content(
    event: &Value,
) -> (
    String,
    Vec<OnebotInboundMediaRef>,
    Vec<OnebotEmbeddedRef>,
) {
    extract_message_content_detail(event).into_public_parts()
}

fn extract_message_content_detail(event: &Value) -> OnebotParsedMessage {
    let message_field = event.get("message");
    if let Some(arr) = message_field.and_then(|v| v.as_array()) {
        let result = parse_onebot_message_array_detail(arr);
        runtime_log_info(format!(
            "[远程IM] 解析数组格式消息 文本长度={} 媒体数={} 内嵌引用={}",
            result.text.len(),
            result.media_refs.len(),
            result.embedded_refs.len()
        ));
        return result;
    }
    if let Some(msg_str) = message_field.and_then(|v| v.as_str()) {
        let parsed = parse_onebot_cq_string_detail(msg_str);
        runtime_log_info(format!(
            "[远程IM] 解析字符串格式消息 文本长度={} 媒体数={} 内嵌引用={}",
            parsed.text.len(),
            parsed.media_refs.len(),
            parsed.embedded_refs.len()
        ));
        return parsed;
    }
    if let Some(raw) = event.get("raw_message").and_then(|v| v.as_str()) {
        let parsed = parse_onebot_cq_string_detail(raw);
        runtime_log_info(format!(
            "[远程IM] 解析原始消息 文本长度={} 媒体数={} 内嵌引用={}",
            parsed.text.len(),
            parsed.media_refs.len(),
            parsed.embedded_refs.len()
        ));
        return parsed;
    }
    runtime_log_info(format!(
        "[远程IM] 消息字段类型未识别 类型={}",
        message_field.map(|v| v.to_string()).unwrap_or_else(|| "无".to_string())
    ));
    OnebotParsedMessage::default()
}

fn onebot_extract_nested_segments(value: &Value) -> Option<&[Value]> {
    value
        .get("message")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .or_else(|| value.get("messages").and_then(Value::as_array).map(Vec::as_slice))
        .or_else(|| value.get("nodes").and_then(Value::as_array).map(Vec::as_slice))
        .or_else(|| value.get("data").and_then(|v| v.get("message")).and_then(Value::as_array).map(Vec::as_slice))
        .or_else(|| value.get("data").and_then(|v| v.get("messages")).and_then(Value::as_array).map(Vec::as_slice))
}

fn onebot_parse_content_value_detail(value: &Value) -> OnebotParsedMessage {
    if let Some(segments) = value.as_array() {
        return parse_onebot_message_array_detail(segments);
    }
    if let Some(text) = value.as_str() {
        return parse_onebot_cq_string_detail(text);
    }
    if let Some(segments) = onebot_extract_nested_segments(value) {
        return parse_onebot_message_array_detail(segments);
    }
    if let Some(content) = value.get("content") {
        return onebot_parse_content_value_detail(content);
    }
    if let Some(content) = value.get("data").and_then(|v| v.get("content")) {
        return onebot_parse_content_value_detail(content);
    }
    if let Some(text) = value
        .get("raw_message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return parse_onebot_cq_string_detail(text);
    }
    OnebotParsedMessage::default()
}

#[cfg(test)]
fn onebot_parse_forward_payload(
    value: &Value,
) -> (
    String,
    Vec<OnebotInboundMediaRef>,
) {
    let parsed = onebot_parse_forward_payload_detail(value);
    (parsed.text, parsed.media_refs)
}

fn onebot_parse_forward_payload_detail(value: &Value) -> OnebotParsedMessage {
    let nodes = value
        .get("messages")
        .or_else(|| value.get("message"))
        .or_else(|| value.get("nodes"))
        .or_else(|| value.get("nodeList"))
        .or_else(|| value.get("data").and_then(|v| v.get("messages")))
        .or_else(|| value.get("data").and_then(|v| v.get("message")))
        .or_else(|| value.get("data").and_then(|v| v.get("nodes")))
        .or_else(|| value.get("data").and_then(|v| v.get("nodeList")));
    let Some(nodes) = nodes.and_then(Value::as_array) else {
        return onebot_parse_content_value_detail(value);
    };

    let mut parsed = OnebotParsedMessage::default();
    let mut text_parts = Vec::<String>::new();
    for node in nodes {
        let sender_name = onebot_resolve_forward_node_sender_name(node);
        let nested = node
            .get("data")
            .and_then(|v| v.get("content"))
            .map(onebot_parse_content_value_detail)
            .or_else(|| node.get("content").map(onebot_parse_content_value_detail))
            .or_else(|| node.get("message").map(onebot_parse_content_value_detail))
            .unwrap_or_else(|| onebot_parse_content_value_detail(node));
        let text = onebot_merge_nested_message(&mut parsed, nested);
        if !text.trim().is_empty() {
            if text.contains('\n') || text.contains("[[PAI_ONEBOT_MENTION_") {
                text_parts.push(format!("{}：\n{}", sender_name, text.trim()));
            } else {
                text_parts.push(format!("{}：{}", sender_name, text.trim()));
            }
        }
    }
    parsed.text = text_parts.join("\n").trim().to_string();
    parsed
}

fn onebot_read_sender_name(sender: &Value, prefer_card: bool) -> Option<String> {
    let primary_key = if prefer_card { "card" } else { "nickname" };
    let secondary_key = if prefer_card { "nickname" } else { "card" };

    sender
        .get(primary_key)
        .or_else(|| sender.get(secondary_key))
        .or_else(|| sender.get("user_id"))
        .and_then(onebot_scalar_to_trimmed_string)
}

fn onebot_resolve_forward_node_sender_name(node: &Value) -> String {
    node.get("sender")
        .and_then(|sender| onebot_read_sender_name(sender, false))
        .or_else(|| {
            node.get("data")
                .and_then(|data| data.get("sender"))
                .and_then(|sender| onebot_read_sender_name(sender, false))
        })
        .or_else(|| {
            node.get("data")
                .and_then(|data| data.get("name"))
                .or_else(|| node.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "未知发送者".to_string())
}

fn onebot_read_sender_display_name(sender: &Value, prefer_card: bool) -> Option<String> {
    let primary_key = if prefer_card { "card" } else { "nickname" };
    let secondary_key = if prefer_card { "nickname" } else { "card" };

    sender
        .get(primary_key)
        .or_else(|| sender.get(secondary_key))
        .and_then(onebot_scalar_to_trimmed_string)
}

fn onebot_read_reply_sender_name(payload: &Value) -> Option<String> {
    payload
        .get("sender")
        .and_then(|sender| onebot_read_sender_display_name(sender, true))
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| data.get("sender"))
                .and_then(|sender| onebot_read_sender_display_name(sender, true))
        })
}

fn onebot_read_reply_sender_id(payload: &Value) -> Option<String> {
    onebot_read_id_as_string(payload, "user_id")
        .or_else(|| {
            payload
                .get("sender")
                .and_then(|sender| onebot_read_id_as_string(sender, "user_id"))
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| onebot_read_id_as_string(data, "user_id"))
        })
        .or_else(|| {
            payload
                .get("data")
                .and_then(|data| data.get("sender"))
                .and_then(|sender| onebot_read_id_as_string(sender, "user_id"))
        })
}

fn onebot_markdown_quote_block(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                ">".to_string()
            } else {
                format!("> {}", trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn onebot_format_reply_quote_text(sender_name: Option<&str>, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let quote_text = if let Some(sender_name) = sender_name.map(str::trim).filter(|value| !value.is_empty()) {
        if trimmed.contains('\n') {
            format!("{}：\n{}", sender_name, trimmed)
        } else {
            format!("{}：{}", sender_name, trimmed)
        }
    } else {
        trimmed.to_string()
    };
    onebot_format_segment_quote_with_body("回复引用", Vec::new(), Some(&quote_text))
}

async fn onebot_call_action_try_params(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    action: &str,
    params_list: &[Value],
) -> Result<Value, String> {
    let mut last_err = String::new();
    for params in params_list {
        match manager.call_api(channel_id, action, params.clone(), 5000).await {
            Ok(v) => return Ok(v),
            Err(err) => last_err = err,
        }
    }
    Err(format!(
        "all attempts failed for action={}, last_err={}",
        action, last_err
    ))
}

fn onebot_member_data_object(value: &Value) -> &Value {
    value.get("data").unwrap_or(value)
}

fn onebot_member_field(value: &Value, key: &str) -> String {
    onebot_read_id_as_string(onebot_member_data_object(value), key).unwrap_or_default()
}

fn onebot_first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("")
        .to_string()
}

fn onebot_group_member_display_name_from_parts(
    user_id: &str,
    card: &str,
    nickname: &str,
    fallback: Option<&str>,
) -> String {
    onebot_first_non_empty(&[card, nickname, fallback.unwrap_or(""), user_id])
}

fn onebot_group_member_info_from_event_sender(
    event: &Value,
    fallback_sender_name: &str,
    updated_at: &str,
) -> Option<RemoteImGroupMemberInfo> {
    let user_id = onebot_read_id_as_string(event, "user_id")?;
    let sender = event.get("sender").unwrap_or(&Value::Null);
    let fallback = fallback_sender_name.trim();
    let raw_nickname = onebot_member_field(sender, "nickname");
    let nickname = onebot_first_non_empty(&[
        raw_nickname.as_str(),
        if fallback == user_id { "" } else { fallback },
    ]);
    let card = onebot_member_field(sender, "card");
    let display_name = onebot_group_member_display_name_from_parts(
        &user_id,
        &card,
        &nickname,
        Some(fallback_sender_name),
    );
    Some(RemoteImGroupMemberInfo {
        user_id,
        nickname,
        card,
        display_name,
        updated_at: Some(updated_at.to_string()),
        raw: event.get("sender").cloned(),
    })
}

fn onebot_group_member_info_from_payload(
    requested_user_id: &str,
    payload: &Value,
    updated_at: &str,
) -> Option<RemoteImGroupMemberInfo> {
    let data = onebot_member_data_object(payload);
    let user_id = onebot_read_id_as_string(data, "user_id")
        .or_else(|| onebot_read_id_as_string(data, "userId"))
        .unwrap_or_else(|| requested_user_id.trim().to_string());
    if user_id.trim().is_empty() {
        return None;
    }
    let nickname = onebot_member_field(payload, "nickname");
    let card = onebot_member_field(payload, "card");
    let display_name =
        onebot_group_member_display_name_from_parts(&user_id, &card, &nickname, None);
    Some(RemoteImGroupMemberInfo {
        user_id,
        nickname,
        card,
        display_name,
        updated_at: Some(updated_at.to_string()),
        raw: Some(payload.clone()),
    })
}

fn onebot_group_member_display_name(info: &RemoteImGroupMemberInfo) -> String {
    onebot_group_member_display_name_from_parts(
        &info.user_id,
        &info.card,
        &info.nickname,
        Some(&info.display_name),
    )
}

fn onebot_display_name_if_not_id(user_id: &str, display_name: String) -> Option<String> {
    let user_id = user_id.trim();
    let display_name = display_name.trim();
    if display_name.is_empty() || display_name == user_id {
        None
    } else {
        Some(display_name.to_string())
    }
}

fn onebot_group_member_cache_for_contact(
    state: &AppState,
    channel_id: &str,
    group_id: Option<u64>,
) -> std::collections::HashMap<String, RemoteImGroupMemberInfo> {
    let Some(group_id) = group_id else {
        return std::collections::HashMap::new();
    };
    let group_id = group_id.to_string();
    let Ok(contact) = state_service_find_remote_im_contact_by_identity(
        state,
        channel_id,
        "group",
        &group_id,
    ) else {
        return std::collections::HashMap::new();
    };
    let Some(contact) = contact else {
        return std::collections::HashMap::new();
    };
    contact
        .onebot_group_members
        .iter()
        .filter(|item| !item.user_id.trim().is_empty())
        .map(|item| (item.user_id.trim().to_string(), item.clone()))
        .collect()
}

fn onebot_merge_group_member_cache_entry(
    cache: &mut std::collections::HashMap<String, RemoteImGroupMemberInfo>,
    info: RemoteImGroupMemberInfo,
) {
    let user_id = info.user_id.trim();
    if !user_id.is_empty() {
        cache.insert(user_id.to_string(), info);
    }
}

/// 写群成员缓存。
///
/// 按群身份自己查联系人：入站走缓冲后入队不再返回联系人 ID，调用方也拿不到，
/// 不能再依赖入队返回值。联系人尚未建立（群第一条消息）时跳过，属正常情况。
fn onebot_persist_group_member_cache(
    state: &AppState,
    channel_id: &str,
    group_id: &str,
    members: Vec<RemoteImGroupMemberInfo>,
) -> Result<(), String> {
    if members.is_empty() {
        return Ok(());
    }
    let contact =
        state_service_find_remote_im_contact_by_identity(state, channel_id, "group", group_id)?;
    let Some(contact) = contact else {
        runtime_log_debug(format!(
            "[远程IM] 群成员缓存跳过 任务=写群成员 渠道={channel_id} 群={group_id} 原因=联系人尚未建立"
        ));
        return Ok(());
    };
    let members = members
        .into_iter()
        .filter(|member| !member.user_id.trim().is_empty())
        .collect::<Vec<_>>();
    if members.is_empty() {
        return Ok(());
    }
    state_service_upsert_remote_im_group_members(state, &contact.id, &members)
}

fn onebot_format_mention_quote(qq: &str, display_name: Option<&str>) -> String {
    let qq = qq.trim();
    if qq.eq_ignore_ascii_case("all") {
        return onebot_format_segment_quote(
            "提及",
            vec![("对象".to_string(), "全体成员".to_string())],
        );
    }
    let target = display_name
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != qq)
        .map(|name| format!("{} (QQ: {})", name, qq))
        .unwrap_or_else(|| format!("QQ: {}", qq));
    onebot_format_segment_quote("提及", vec![("对象".to_string(), target)])
}

#[cfg(test)]
fn onebot_resolve_reply_sender_display_name(payload: &Value) -> Option<String> {
    if let Some(name) = onebot_read_reply_sender_name(payload) {
        return Some(name);
    }
    onebot_read_reply_sender_id(payload)
}

async fn onebot_fetch_group_member_info(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: u64,
    user_id: &str,
) -> Option<RemoteImGroupMemberInfo> {
    let numeric_user_id = user_id.trim().parse::<u64>().ok()?;
    let payload = onebot_call_action_try_params(
        manager,
        channel_id,
        "get_group_member_info",
        &[
            serde_json::json!({ "group_id": group_id, "user_id": numeric_user_id, "no_cache": false }),
            serde_json::json!({ "group_id": group_id, "user_id": numeric_user_id }),
            serde_json::json!({ "group_id": group_id.to_string(), "user_id": user_id.trim(), "no_cache": false }),
            serde_json::json!({ "group_id": group_id.to_string(), "user_id": user_id.trim() }),
        ],
    )
    .await
    .ok()?;
    onebot_group_member_info_from_payload(user_id, &payload, &now_iso())
}

async fn onebot_fetch_stranger_display_name(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    user_id: &str,
) -> Option<String> {
    let numeric_user_id = user_id.trim().parse::<u64>().ok()?;
    let payload = onebot_call_action_try_params(
        manager,
        channel_id,
        "get_stranger_info",
        &[
            serde_json::json!({ "user_id": numeric_user_id, "no_cache": false }),
            serde_json::json!({ "user_id": numeric_user_id }),
            serde_json::json!({ "user_id": user_id.trim(), "no_cache": false }),
            serde_json::json!({ "user_id": user_id.trim() }),
        ],
    )
    .await
    .ok()?;
    onebot_display_name_if_not_id(user_id, onebot_member_field(&payload, "nickname"))
}

async fn onebot_resolve_native_user_display_name(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: Option<u64>,
    user_id: &str,
    group_member_cache: &mut std::collections::HashMap<String, RemoteImGroupMemberInfo>,
) -> Option<String> {
    let user_id = user_id.trim();
    if user_id.is_empty() || user_id.eq_ignore_ascii_case("all") {
        return None;
    }
    if group_id.is_some() {
        if let Some(cached) = group_member_cache.get(user_id) {
            if let Some(name) =
                onebot_display_name_if_not_id(user_id, onebot_group_member_display_name(cached))
            {
                return Some(name);
            }
        }
    }
    if let Some(group_id) = group_id {
        if let Some(info) =
            onebot_fetch_group_member_info(manager, channel_id, group_id, user_id).await
        {
            let display_name = onebot_group_member_display_name(&info);
            onebot_merge_group_member_cache_entry(group_member_cache, info);
            if let Some(name) = onebot_display_name_if_not_id(user_id, display_name) {
                return Some(name);
            }
        }
    }
    onebot_fetch_stranger_display_name(manager, channel_id, user_id).await
}

async fn onebot_resolve_reply_sender_display_name_with_api(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: Option<u64>,
    payload: &Value,
    group_member_cache: &mut std::collections::HashMap<String, RemoteImGroupMemberInfo>,
) -> Option<String> {
    if let Some(name) = onebot_read_reply_sender_name(payload) {
        return Some(name);
    }
    let sender_id = onebot_read_reply_sender_id(payload)?;
    onebot_resolve_native_user_display_name(
        manager,
        channel_id,
        group_id,
        &sender_id,
        group_member_cache,
    )
    .await
    .or(Some(sender_id))
}

async fn onebot_resolve_mentions_in_text(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: Option<u64>,
    text: String,
    mention_refs: &[OnebotMentionRef],
    group_member_cache: &mut std::collections::HashMap<String, RemoteImGroupMemberInfo>,
) -> String {
    if mention_refs.is_empty() {
        return text;
    }
    let mut resolved_names = std::collections::HashMap::<String, Option<String>>::new();
    let mut out = text;
    for item in mention_refs {
        let qq = item.qq.trim();
        if !resolved_names.contains_key(qq) {
            let name = onebot_resolve_native_user_display_name(
                manager,
                channel_id,
                group_id,
                qq,
                group_member_cache,
            )
            .await;
            resolved_names.insert(qq.to_string(), name);
        }
        let display_name = resolved_names.get(qq).and_then(|value| value.as_deref());
        out = out.replace(&item.placeholder, &onebot_format_mention_quote(qq, display_name));
    }
    out
}

async fn onebot_expand_embedded_content(
    manager: &OnebotV11WsManager,
    channel_id: &str,
    group_id: Option<u64>,
    group_member_cache: &mut std::collections::HashMap<String, RemoteImGroupMemberInfo>,
    refs: &[OnebotEmbeddedRef],
) -> (String, Vec<OnebotInboundMediaRef>) {
    let mut text_parts = Vec::<String>::new();
    let mut media_refs = Vec::<OnebotInboundMediaRef>::new();
    for item in refs {
        let payload_result = match item.kind {
            OnebotEmbeddedRefKind::Reply => {
                onebot_call_action_try_params(
                    manager,
                    channel_id,
                    "get_msg",
                    &[serde_json::json!({ "message_id": item.id })],
                )
                .await
            }
            OnebotEmbeddedRefKind::Forward => {
                onebot_call_action_try_params(
                    manager,
                    channel_id,
                    "get_forward_msg",
                    &[
                        serde_json::json!({ "id": item.id }),
                        serde_json::json!({ "message_id": item.id }),
                    ],
                )
                .await
            }
        };

        let Ok(payload) = payload_result else {
            match item.kind {
                OnebotEmbeddedRefKind::Reply => text_parts.push(onebot_format_segment_quote(
                    "回复引用",
                    vec![("说明".to_string(), "无法获取引用消息".to_string())],
                )),
                OnebotEmbeddedRefKind::Forward => text_parts.push(onebot_format_segment_quote(
                    "合并转发",
                    vec![("说明".to_string(), "无法获取转发消息".to_string())],
                )),
            }
            continue;
        };

        let (text, nested_media_refs) = match item.kind {
            OnebotEmbeddedRefKind::Reply => {
                let parsed = onebot_parse_content_value_detail(&payload);
                let text = onebot_resolve_mentions_in_text(
                    manager,
                    channel_id,
                    group_id,
                    parsed.text,
                    &parsed.mention_refs,
                    group_member_cache,
                )
                .await;
                (text, parsed.media_refs)
            }
            OnebotEmbeddedRefKind::Forward => {
                let parsed = onebot_parse_forward_payload_detail(&payload);
                let text = onebot_resolve_mentions_in_text(
                    manager,
                    channel_id,
                    group_id,
                    parsed.text,
                    &parsed.mention_refs,
                    group_member_cache,
                )
                .await;
                (text, parsed.media_refs)
            }
        };

        if !text.trim().is_empty() {
            let rendered = match item.kind {
                OnebotEmbeddedRefKind::Reply => {
                    let sender_name = onebot_resolve_reply_sender_display_name_with_api(
                        manager,
                        channel_id,
                        group_id,
                        &payload,
                        group_member_cache,
                    )
                    .await;
                    onebot_format_reply_quote_text(sender_name.as_deref(), &text)
                }
                OnebotEmbeddedRefKind::Forward => onebot_format_segment_quote_with_body(
                    "合并转发",
                    Vec::new(),
                    Some(text.trim()),
                ),
            };
            if !rendered.trim().is_empty() {
                text_parts.push(rendered);
            }
        }
        media_refs.extend(nested_media_refs);
    }

    (text_parts.join("\n").trim().to_string(), media_refs)
}

async fn resolve_contact_info(
    event: &Value,
    manager: &OnebotV11WsManager,
    channel_id: &str,
) -> Result<(String, String, Option<String>), String> {
    let message_type = event
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("private");
    let user_id = onebot_read_id_as_string(event, "user_id").unwrap_or_default();
    let group_id = onebot_read_id_as_string(event, "group_id");
    if message_type == "group" {
        let gid = group_id.clone().ok_or("群消息缺少 group_id")?;
        let group_name = match manager
            .call_api(
                channel_id,
                "get_group_info",
                serde_json::json!({ "group_id": gid }),
                5000,
            )
            .await
        {
            Ok(info) => info
                .get("group_name")
                .and_then(|n| n.as_str())
                .map(String::from),
            Err(_) => None,
        };
        Ok(("group".to_string(), gid, group_name))
    } else {
        if user_id.is_empty() {
            return Err("私聊消息缺少 user_id".to_string());
        }
        Ok(("private".to_string(), user_id, None))
    }
}

fn read_channel_config(
    state: &AppState,
    channel_id: &str,
) -> Result<Option<RemoteImChannelConfig>, String> {
    let config = state_read_config_cached(state)?;
    let channel_config = remote_im_channel_by_id(&config, channel_id).cloned();
    Ok(channel_config)
}

fn resolve_sender_name(event: &Value) -> String {
    event
        .get("sender")
        .and_then(|sender| onebot_read_sender_name(sender, false))
        .or_else(|| onebot_read_id_as_string(event, "user_id"))
        .unwrap_or_else(|| "Unknown".to_string())
}

fn message_field_kind(message_field: Option<&Value>) -> &'static str {
    message_field
        .map(|v| match v {
            Value::Array(_) => "array",
            Value::String(_) => "string",
            Value::Null => "null",
            Value::Object(_) => "object",
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
        })
        .unwrap_or("missing")
}

