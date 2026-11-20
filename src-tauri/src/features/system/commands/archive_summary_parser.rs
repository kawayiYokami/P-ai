#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(untagged)]
enum StringishId {
    Text(String),
    Integer(i64),
    Unsigned(u64),
}

fn stringish_id_to_string(value: StringishId) -> Option<String> {
    match value {
        StringishId::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        StringishId::Integer(value) => Some(value.to_string()),
        StringishId::Unsigned(value) => Some(value.to_string()),
    }
}

fn deserialize_stringish_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<StringishId>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(stringish_id_to_string)
        .collect::<Vec<_>>())
}

fn deserialize_trimmed_strings<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>())
}

#[derive(Debug, Clone, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ArchiveMemoryDraft {
    #[serde(default)]
    memory_type: String,
    #[serde(default)]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, deserialize_with = "deserialize_trimmed_strings")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ArchiveMemoryActionKind {
    Create,
    Update,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ArchiveMemoryActionDraft {
    action: ArchiveMemoryActionKind,
    #[serde(default, deserialize_with = "deserialize_stringish_ids")]
    source_memory_ids: Vec<String>,
    memory: ArchiveMemoryDraft,
}

#[derive(Debug, Clone, Deserialize, rmcp::schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
struct MemoryCurationDraft {
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default, deserialize_with = "deserialize_stringish_ids")]
    useful_memory_ids: Vec<String>,
    #[serde(default)]
    memory_actions: Vec<ArchiveMemoryActionDraft>,
}

fn memory_curation_uses_removed_legacy_fields(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    obj.contains_key("newMemories") || obj.contains_key("profileActions")
}

fn parse_memory_curation_draft_from_value(value: serde_json::Value) -> Option<MemoryCurationDraft> {
    let obj = value.as_object()?;
    let has_any_useful_key = obj.contains_key("usefulMemoryIds")
        || obj.contains_key("title")
        || obj.contains_key("summary")
        || obj.contains_key("memoryActions");
    if !has_any_useful_key {
        return None;
    }
    if memory_curation_uses_removed_legacy_fields(obj) {
        return None;
    }
    let parsed = serde_json::from_value::<MemoryCurationDraft>(value).ok()?;
    validate_memory_curation_draft(parsed)
}

fn validate_memory_curation_draft(draft: MemoryCurationDraft) -> Option<MemoryCurationDraft> {
    let memory_actions_valid = draft.memory_actions.iter().all(|item| match item.action {
        ArchiveMemoryActionKind::Create => item.source_memory_ids.is_empty(),
        ArchiveMemoryActionKind::Update => !item.source_memory_ids.is_empty(),
        ArchiveMemoryActionKind::Merge => !item.source_memory_ids.is_empty(),
    });
    if !memory_actions_valid {
        return None;
    }

    Some(draft)
}

fn parse_memory_curation_draft(raw: &str) -> Option<MemoryCurationDraft> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(parsed) = parse_memory_curation_draft_from_value(parsed_value) {
            return Some(parsed);
        }
    }
    let parsed_value = extract_best_json_object_value(
        trimmed,
        "---JSON---",
        &[],
        &[
            "title",
            "summary",
            "usefulMemoryIds",
            "memoryActions",
        ],
    )?;
    parse_memory_curation_draft_from_value(parsed_value)
}

#[cfg(test)]
mod archive_summary_parser_tests {
    use super::*;

    #[test]
    fn memory_curation_schema_should_expose_current_top_level_fields() {
        let schema = rmcp::schemars::schema_for!(MemoryCurationDraft);
        let schema_value = serde_json::to_value(&schema).expect("serialize schema");
        let properties = schema_value
            .pointer("/properties")
            .and_then(serde_json::Value::as_object)
            .expect("schema properties");
        assert!(properties.contains_key("title"));
        assert!(properties.contains_key("summary"));
        assert!(!properties.contains_key("openLoops"));
        assert!(properties.contains_key("usefulMemoryIds"));
        assert!(properties.contains_key("memoryActions"));
    }

    #[test]
    fn parse_memory_curation_should_support_canonical_actions() {
        let raw = r#"{
          "summary": "归档摘要",
          "openLoops": ["继续改 archive pipeline"],
          "usefulMemoryIds": ["mem-1"],
          "memoryActions": [
            {
              "action": "create",
              "memory": {
                "memoryType": "knowledge",
                "judgment": "用户常驻深圳",
                "reasoning": "本轮明确说明",
                "tags": ["深圳", "居住地"]
              }
            },
            {
              "action": "update",
              "sourceMemoryIds": ["mem-3"],
              "memory": {
                "memoryType": "knowledge",
                "judgment": "用户现在常驻杭州",
                "reasoning": "本轮明确说明已经不在深圳，现居杭州",
                "tags": ["杭州", "居住地"]
              }
            }
          ]
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse canonical actions");
        assert_eq!(parsed.summary, "归档摘要".to_string());
        assert_eq!(parsed.useful_memory_ids, vec!["mem-1".to_string()]);
        assert_eq!(parsed.memory_actions.len(), 2);
        assert_eq!(
            parsed.memory_actions[1].source_memory_ids,
            vec!["mem-3".to_string()]
        );
    }

    #[test]
    fn parse_memory_curation_should_support_archive_reflection_shape() {
        let raw = r#"{
          "usefulMemoryIds": ["mem-1"],
          "memoryActions": []
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse archive reflection");
        assert_eq!(parsed.title, "");
        assert_eq!(parsed.summary, "");
        assert_eq!(parsed.useful_memory_ids, vec!["mem-1".to_string()]);
    }

    #[test]
    fn parse_memory_curation_should_keep_compat_extra_archive_fields() {
        let raw = r#"{
          "title": "旧格式标题",
          "summary": "旧格式摘要",
          "openLoops": ["旧格式待办"],
          "usefulMemoryIds": ["mem-1"],
          "memoryActions": []
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse compatible archive reflection");
        assert_eq!(parsed.title, "旧格式标题");
        assert_eq!(parsed.summary, "旧格式摘要");
        assert_eq!(parsed.useful_memory_ids, vec!["mem-1".to_string()]);
    }

    #[test]
    fn parse_memory_curation_should_accept_numeric_short_ids() {
        let raw = r#"{
          "summary": "归档摘要",
          "usefulMemoryIds": [12, "19"],
          "memoryActions": [
            {
              "action": "merge",
              "sourceMemoryIds": [12, 19],
              "memory": {
                "memoryType": "knowledge",
                "judgment": "合并后的记忆",
                "reasoning": "同义重复",
                "tags": ["合并"]
              }
            }
          ]
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse numeric ids");
        assert_eq!(parsed.useful_memory_ids, vec!["12".to_string(), "19".to_string()]);
        assert_eq!(
            parsed.memory_actions[0].source_memory_ids,
            vec!["12".to_string(), "19".to_string()]
        );
    }

    #[test]
    fn parse_memory_curation_should_reject_legacy_shape() {
        let raw = r#"{
          "summary": "归档摘要",
          "usefulMemoryIds": ["12"],
          "newMemories": [
            {
              "memoryType": "knowledge",
              "judgment": "旧结构",
              "reasoning": "",
              "tags": ["旧"]
            }
          ]
        }"#;
        assert!(parse_memory_curation_draft(raw).is_none());
    }

    #[test]
    fn parse_memory_curation_should_reject_removed_profile_actions_field() {
        let raw = r#"{
          "summary": "归档摘要",
          "memoryActions": [],
          "profileActions": [
            {
              "action": "create",
                "memory": {
                "memoryType": "knowledge",
                "judgment": "旧画像结构",
                "reasoning": "",
                "tags": ["旧"]
              }
            }
          ]
        }"#;
        assert!(parse_memory_curation_draft(raw).is_none());
    }

    #[test]
    fn parse_memory_curation_should_ignore_unknown_fields() {
        let raw = r#"{
          "title": "归档标题",
          "summary": "归档摘要",
          "extraField": "ignore-me",
          "memoryActions": [
            {
              "action": "create",
              "debugNote": "ignore-me-too",
              "memory": {
                "memoryType": "knowledge",
                "judgment": "用户常驻杭州",
                "reasoning": "本轮明确提到",
                "tags": ["杭州", "居住地"],
                "extraNestedField": true
              }
            }
          ]
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse with unknown fields");
        assert_eq!(parsed.title, "归档标题".to_string());
        assert_eq!(parsed.memory_actions.len(), 1);
        assert_eq!(parsed.memory_actions[0].memory.judgment, "用户常驻杭州");
    }

    #[test]
    fn parse_memory_curation_should_extract_json_from_wrapped_text() {
        let fence = "```";
        let raw = format!(r#"下面是整理结果：

{}json
{{
  "summary": "归档摘要",
  "openLoops": ["补 archive 测试"],
  "usefulMemoryIds": ["12"],
  "memoryActions": []
}}
{}

请查收。"#, fence, fence);
        let parsed = parse_memory_curation_draft(&raw).expect("extract wrapped json");
        assert_eq!(parsed.summary, "归档摘要".to_string());
        assert_eq!(parsed.useful_memory_ids, vec!["12".to_string()]);
    }

    #[test]
    fn parse_memory_curation_should_extract_last_json_object() {
        let raw = r#"EXAMPLE JSON OUTPUT:
{
  "summary": "示例摘要",
  "memoryActions": []
}

最终输出：
{
  "summary": "真实摘要",
  "openLoops": ["继续修解析"],
  "memoryActions": []
}"#;
        let parsed = parse_memory_curation_draft(raw).expect("extract last json");
        assert_eq!(parsed.summary, "真实摘要".to_string());
    }

    #[test]
    fn parse_memory_curation_should_trim_empty_items_in_salvaged_json() {
        let raw = r#"结果如下：
{
  "summary": "归档摘要",
  "openLoops": [" 继续推进  ", " ", ""],
  "usefulMemoryIds": [" 12 ", ""],
  "memoryActions": []
}
尾注"#;
        let parsed = parse_memory_curation_draft(raw).expect("trim salvaged json");
        assert_eq!(parsed.useful_memory_ids, vec!["12".to_string()]);
    }

    #[test]
    fn parse_memory_curation_should_reject_update_without_source_ids_even_with_unknown_fields() {
        let raw = r#"{
          "summary": "归档摘要",
          "memoryActions": [
            {
              "action": "update",
              "extraField": "ignore-me",
              "memory": {
                "memoryType": "knowledge",
                "judgment": "新结论",
                "reasoning": "有依据",
                "tags": ["标签"]
              }
            }
          ]
        }"#;
        assert!(parse_memory_curation_draft(raw).is_none());
    }

}
