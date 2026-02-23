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

fn parse_archive_summary_draft(raw: &str) -> Option<ArchiveSummaryDraft> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<ArchiveSummaryDraft>(trimmed) {
        return Some(parsed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<ArchiveSummaryDraft>(&trimmed[start..=end]).ok()
}
