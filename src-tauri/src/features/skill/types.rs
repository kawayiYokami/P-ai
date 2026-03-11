use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLoadError {
    pub item: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummaryItem {
    pub name: String,
    pub description: String,
    pub content: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshMcpAndSkillsResult {
    pub mcp_loaded: Vec<String>,
    pub mcp_failed: Vec<WorkspaceLoadError>,
    pub skills_loaded: Vec<String>,
    pub skills_failed: Vec<WorkspaceLoadError>,
    pub skills: Vec<SkillSummaryItem>,
    pub skill_summary: String,
    pub private_agents_loaded: Vec<String>,
    pub private_agents_failed: Vec<WorkspaceLoadError>,
    pub private_departments_loaded: Vec<String>,
    pub private_departments_failed: Vec<WorkspaceLoadError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillListResult {
    pub skills: Vec<SkillSummaryItem>,
    pub errors: Vec<WorkspaceLoadError>,
}
