use serde::{Deserialize, Serialize};

/// Skill 全局启用状态的缺省值：启用。
fn default_skill_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLoadError {
    pub item: String,
    pub error: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    #[serde(default)]
    pub skipped: bool,
}

impl WorkspaceLoadError {
    pub(crate) fn with_hint(
        item: impl Into<String>,
        error: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            item: item.into(),
            error: error.into(),
            hint: hint.into(),
            skipped: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLoadedGroup {
    pub kind: String,
    pub label: String,
    pub count: usize,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFailedGroup {
    pub kind: String,
    pub label: String,
    pub count: usize,
    pub items: Vec<WorkspaceLoadError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileItem {
    pub name: String,
    pub relative_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummaryItem {
    pub name: String,
    pub description: String,
    pub content: String,
    pub path: String,
    #[serde(default)]
    pub additional_files: Vec<SkillFileItem>,
    #[serde(default)]
    pub is_builtin: bool,
    /// 全局启用状态；缺省为启用。
    #[serde(default = "default_skill_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshMcpAndSkillsResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub status: String,
    pub mcp_loaded: Vec<String>,
    pub mcp_failed: Vec<WorkspaceLoadError>,
    pub skills_loaded: Vec<String>,
    pub skills_failed: Vec<WorkspaceLoadError>,
    pub skills: Vec<SkillSummaryItem>,
    pub skill_summary: String,
    pub private_agents_loaded: Vec<String>,
    pub private_agents_failed: Vec<WorkspaceLoadError>,
    pub loaded_groups: Vec<WorkspaceLoadedGroup>,
    pub failed_groups: Vec<WorkspaceFailedGroup>,
    pub total_loaded: usize,
    pub total_failed: usize,
    pub loaded_summary: String,
    pub failed_summary: String,
    #[serde(default)]
    pub repair_summary: String,
    #[serde(default)]
    pub repair_items: Vec<WorkspaceLoadError>,
    pub needs_repair: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillListResult {
    pub skills: Vec<SkillSummaryItem>,
    pub errors: Vec<WorkspaceLoadError>,
}
