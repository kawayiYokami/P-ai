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
    #[serde(default)]
    source_ids: Vec<String>,
    target: ArchiveMemoryDraft,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveSummaryDraft {
    summary: String,
    #[serde(default)]
    useful_memory_ids: Vec<String>,
    #[serde(default, alias = "memories")]
    new_memories: Vec<ArchiveMemoryDraft>,
    #[serde(default)]
    merge_groups: Vec<ArchiveMergeGroupDraft>,
}

const SUMMARY_SECTIONS: [&str; 8] = [
    "Current Progress",
    "Current State",
    "User Decisions",
    "Open Issue (Root Cause)",
    "What Changed",
    "What Remains",
    "Constraints / Preferences",
    "Quick References",
];

fn summary_from_section_object(obj: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let mut blocks = Vec::<String>::new();
    for key in SUMMARY_SECTIONS {
        let value = obj
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or("无")
            .trim()
            .to_string();
        blocks.push(format!("## {}\n{}", key, if value.is_empty() { "无" } else { &value }));
    }
    let out = blocks.join("\n\n");
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_archive_summary_draft_from_value(value: serde_json::Value) -> Option<ArchiveSummaryDraft> {
    let obj = value.as_object()?;
    let has_any_useful_key = obj.contains_key("summary")
        || obj.contains_key("usefulMemoryIds")
        || obj.contains_key("useful_memory_ids")
        || obj.contains_key("newMemories")
        || obj.contains_key("new_memories")
        || obj.contains_key("memories")
        || obj.contains_key("mergeGroups")
        || obj.contains_key("merge_groups");
    if !has_any_useful_key {
        return None;
    }

    let summary = match obj.get("summary") {
        Some(serde_json::Value::String(s)) => s.trim().to_string(),
        Some(serde_json::Value::Object(map)) => summary_from_section_object(map).unwrap_or_default(),
        Some(other) => other.to_string(),
        None => String::new(),
    };

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

    Some(ArchiveSummaryDraft {
        summary,
        useful_memory_ids,
        new_memories,
        merge_groups,
    })
}

fn parse_archive_summary_draft(raw: &str) -> Option<ArchiveSummaryDraft> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<ArchiveSummaryDraft>(trimmed) {
        return Some(parsed);
    }
    if let Ok(parsed_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(parsed) = parse_archive_summary_draft_from_value(parsed_value) {
            return Some(parsed);
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    let snippet = &trimmed[start..=end];
    if let Ok(parsed) = serde_json::from_str::<ArchiveSummaryDraft>(snippet) {
        return Some(parsed);
    }
    let parsed_value = serde_json::from_str::<serde_json::Value>(snippet).ok()?;
    parse_archive_summary_draft_from_value(parsed_value)
}
