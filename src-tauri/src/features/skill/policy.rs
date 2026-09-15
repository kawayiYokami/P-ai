// Skill 全局启用状态。
// 与 skills/ 目录解耦：目录只表达「有哪些 Skill」，本模块表达「哪些 Skill 参与运行时」。
// 缺省视为启用，保证既有 Skill 行为不变；由商店或内置清单写入的条目会显式落盘为关闭。

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillPolicyFile {
    #[serde(default)]
    pub(crate) skill_name: String,
    #[serde(default = "default_skill_policy_enabled")]
    pub(crate) enabled: bool,
}

fn default_skill_policy_enabled() -> bool {
    true
}

pub(crate) fn llm_workspace_skill_policies_dir(state: &AppState) -> Result<PathBuf, String> {
    Ok(configured_workspace_root_path(state)?.join("skill-policies"))
}

fn skill_policy_file_path(state: &AppState, skill_name: &str) -> Result<PathBuf, String> {
    let file = format!("{}.json", sanitize_skill_name_for_filename(skill_name));
    Ok(llm_workspace_skill_policies_dir(state)?.join(file))
}

fn sanitize_skill_name_for_filename(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').trim().to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

/// 写入单个 Skill 的启用状态。
pub(crate) fn write_skill_enabled(
    state: &AppState,
    skill_name: &str,
    enabled: bool,
) -> Result<(), String> {
    let path = skill_policy_file_path(state, skill_name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建 Skill 状态目录失败（{}）：{err}", parent.display()))?;
    }
    let policy = SkillPolicyFile {
        skill_name: skill_name.to_string(),
        enabled,
    };
    let raw = serde_json::to_vec_pretty(&policy)
        .map_err(|err| format!("序列化 Skill 启用状态失败：{err}"))?;
    fs::write(&path, raw)
        .map_err(|err| format!("写入 Skill 启用状态失败（{}）：{err}", path.display()))
}

/// 移除某个 Skill 的启用状态记录；记录不存在时视为已清理。
pub(crate) fn remove_skill_policy(state: &AppState, skill_name: &str) -> Result<(), String> {
    let path = skill_policy_file_path(state, skill_name)?;
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|err| format!("删除 Skill 启用状态失败（{}）：{err}", path.display()))
}

/// 载入全部已显式落盘的启用状态，键为 Skill 名。
pub(crate) fn load_skill_enabled_map(
    state: &AppState,
) -> Result<std::collections::HashMap<String, bool>, String> {
    let dir = llm_workspace_skill_policies_dir(state)?;
    let mut map = std::collections::HashMap::new();
    if !dir.exists() {
        return Ok(map);
    }
    let entries = fs::read_dir(&dir)
        .map_err(|err| format!("读取 Skill 状态目录失败（{}）：{err}", dir.display()))?;
    for entry in entries.filter_map(|item| item.ok()) {
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read(&path) else {
            continue;
        };
        let Ok(policy) = serde_json::from_slice::<SkillPolicyFile>(&raw) else {
            runtime_log_warn(format!(
                "[技能启用] 跳过无法解析的状态文件：{}",
                path.display()
            ));
            continue;
        };
        if policy.skill_name.trim().is_empty() {
            continue;
        }
        map.insert(policy.skill_name.trim().to_string(), policy.enabled);
    }
    Ok(map)
}
