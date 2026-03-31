#[derive(Debug, Clone, Deserialize)]
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

fn deserialize_optional_stringish_id<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<StringishId>::deserialize(deserializer)?;
    Ok(raw.and_then(stringish_id_to_string))
}

#[derive(Debug, Clone, Deserialize)]
struct ArchiveMemoryDraft {
    #[serde(default, alias = "memoryType")]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, alias = "keywords")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveMergeGroupDraft {
    #[serde(default, deserialize_with = "deserialize_stringish_ids")]
    source_ids: Vec<String>,
    target: ArchiveMemoryDraft,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveProfileMemoryDraft {
    #[serde(default, deserialize_with = "deserialize_optional_stringish_id")]
    memory_id: Option<String>,
    #[serde(default)]
    memory: Option<ArchiveMemoryDraft>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryCurationDraft {
    #[serde(default)]
    summary: String,
    #[serde(default, deserialize_with = "deserialize_stringish_ids")]
    useful_memory_ids: Vec<String>,
    #[serde(default, alias = "memories")]
    new_memories: Vec<ArchiveMemoryDraft>,
    #[serde(default)]
    merge_groups: Vec<ArchiveMergeGroupDraft>,
    #[serde(default)]
    profile_memories: Vec<ArchiveProfileMemoryDraft>,
}

fn parse_memory_curation_draft_from_value(value: serde_json::Value) -> Option<MemoryCurationDraft> {
    let obj = value.as_object()?;
    let has_any_useful_key = obj.contains_key("usefulMemoryIds")
        || obj.contains_key("summary")
        || obj.contains_key("useful_memory_ids")
        || obj.contains_key("newMemories")
        || obj.contains_key("new_memories")
        || obj.contains_key("memories")
        || obj.contains_key("mergeGroups")
        || obj.contains_key("merge_groups")
        || obj.contains_key("profileMemories")
        || obj.contains_key("profile_memories");
    if !has_any_useful_key {
        return None;
    }

    let summary = obj
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();

    let useful_memory_ids = obj
        .get("usefulMemoryIds")
        .or_else(|| obj.get("useful_memory_ids"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let new_memories = obj
        .get("newMemories")
        .or_else(|| obj.get("new_memories"))
        .or_else(|| obj.get("memories"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<ArchiveMemoryDraft>(item.clone()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let merge_groups = obj
        .get("mergeGroups")
        .or_else(|| obj.get("merge_groups"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    serde_json::from_value::<ArchiveMergeGroupDraft>(item.clone()).ok()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let profile_memories = obj
        .get("profileMemories")
        .or_else(|| obj.get("profile_memories"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    serde_json::from_value::<ArchiveProfileMemoryDraft>(item.clone()).ok()
                })
                .filter(|item| {
                    item.memory_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_some()
                        || item.memory.is_some()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(MemoryCurationDraft {
        summary,
        useful_memory_ids,
        new_memories,
        merge_groups,
        profile_memories,
    })
}

fn parse_memory_curation_draft(raw: &str) -> Option<MemoryCurationDraft> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<MemoryCurationDraft>(trimmed) {
        return Some(parsed);
    }
    if let Ok(parsed_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(parsed) = parse_memory_curation_draft_from_value(parsed_value) {
            return Some(parsed);
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    let snippet = &trimmed[start..=end];
    if let Ok(parsed) = serde_json::from_str::<MemoryCurationDraft>(snippet) {
        return Some(parsed);
    }
    let parsed_value = serde_json::from_str::<serde_json::Value>(snippet).ok()?;
    parse_memory_curation_draft_from_value(parsed_value)
}

#[cfg(test)]
mod archive_summary_parser_tests {
    use super::*;

    #[test]
    fn parse_memory_curation_should_support_profile_memories() {
        let raw = r#"{
          "summary": "归档摘要",
          "usefulMemoryIds": ["mem-1"],
          "profileMemories": [
            { "memoryId": "mem-2" },
            {
              "memory": {
                "memoryType": "knowledge",
                "judgment": "用户常驻深圳",
                "reasoning": "本轮明确说明",
                "tags": ["深圳", "居住地"]
              }
            }
          ]
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse profile memories");
        assert_eq!(parsed.summary, "归档摘要".to_string());
        assert_eq!(parsed.useful_memory_ids, vec!["mem-1".to_string()]);
        assert_eq!(parsed.profile_memories.len(), 2);
        assert_eq!(
            parsed.profile_memories[0].memory_id.as_deref(),
            Some("mem-2")
        );
        assert_eq!(
            parsed.profile_memories[1]
                .memory
                .as_ref()
                .map(|item| item.memory_type.as_str()),
            Some("knowledge")
        );
    }

    #[test]
    fn parse_memory_curation_should_accept_numeric_short_ids() {
        let raw = r#"{
          "summary": "归档摘要",
          "usefulMemoryIds": [12, "19"],
          "mergeGroups": [
            {
              "sourceIds": [12, 19],
              "target": {
                "memoryType": "knowledge",
                "judgment": "合并后的记忆",
                "reasoning": "同义重复",
                "tags": ["合并"]
              }
            }
          ],
          "profileMemories": [
            { "memoryId": 12 }
          ]
        }"#;
        let parsed = parse_memory_curation_draft(raw).expect("parse numeric ids");
        assert_eq!(parsed.useful_memory_ids, vec!["12".to_string(), "19".to_string()]);
        assert_eq!(parsed.merge_groups[0].source_ids, vec!["12".to_string(), "19".to_string()]);
        assert_eq!(parsed.profile_memories[0].memory_id.as_deref(), Some("12"));
    }
}
