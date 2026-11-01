//! 内置（预设）技能清单：随应用释放到工作区 skills 目录，用户只能查看、不能编辑。
//!
//! 其中一部分是「系统托管技能」：连开关权与勾选权也归代码所有，
//! 由 [`ManagedSkillInjection`] 说明它在什么会话里被注入。

/// 托管技能的注入条件：由代码按会话角色决定去留。
///
/// 用枚举而不是 `bool`：托管技能之后还会出现别的注入条件（比如某个角色、某类会话），
/// 现在写成一个 [`Option<ManagedSkillInjection>`]，加条件时不用回来改调用方的形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedSkillInjection {
    /// 只注入队长会话：该会话没挂队伍成员身份，且能持有队伍名册。
    ///
    /// 判据复用队伍侧那一份（`conversation_is_team_leader`），不在这里另写一套：
    /// 两处各写一份会在「成员已离队但名册还在」这类边界上分叉。
    TeamLeader,
}

pub(crate) struct WorkspacePresetSkill {
    pub dir_name: &'static str,
    pub skill_md: &'static str,
    /// 托管注入条件。`None` 为普通内置技能：用户可开关、可勾选。
    pub managed: Option<ManagedSkillInjection>,
}

const WORKSPACE_PRESET_SKILLS: &[WorkspacePresetSkill] = &[
    WorkspacePresetSkill {
        dir_name: "user-browser",
        skill_md: include_str!("../../../resources/preset-skills/user-browser/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "browser-automation",
        skill_md: include_str!("../../../resources/preset-skills/browser-automation/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "news-analyst",
        skill_md: include_str!("../../../resources/preset-skills/news-analyst/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "agent-office",
        skill_md: include_str!("../../../resources/preset-skills/agent-office/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "agents-md-setup",
        skill_md: include_str!("../../../resources/preset-skills/agents-md-setup/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "assistant-interaction-guide",
        skill_md: include_str!(
            "../../../resources/preset-skills/assistant-interaction-guide/SKILL.md"
        ),
        managed: Some(ManagedSkillInjection::TeamLeader),
    },
    WorkspacePresetSkill {
        dir_name: "p-ai-config-guide",
        skill_md: include_str!("../../../resources/preset-skills/p-ai-config-guide/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "pai-guide",
        skill_md: include_str!("../../../resources/preset-skills/pai-guide/SKILL.md"),
        managed: Some(ManagedSkillInjection::TeamLeader),
    },
    WorkspacePresetSkill {
        dir_name: "code-review",
        skill_md: include_str!("../../../resources/preset-skills/code-review/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "memory-generation",
        skill_md: include_str!("../../../resources/preset-skills/memory-generation/SKILL.md"),
        managed: None,
    },
    WorkspacePresetSkill {
        dir_name: "persona-craft",
        skill_md: include_str!("../../../resources/preset-skills/persona-craft/SKILL.md"),
        managed: Some(ManagedSkillInjection::TeamLeader),
    },
];

pub(crate) fn workspace_preset_skills() -> &'static [WorkspacePresetSkill] {
    WORKSPACE_PRESET_SKILLS
}

/// 这个目录名对应哪个预设技能。
pub(crate) fn workspace_preset_skill(dir_name: &str) -> Option<&'static WorkspacePresetSkill> {
    let dir_name = dir_name.trim();
    if dir_name.is_empty() {
        return None;
    }
    WORKSPACE_PRESET_SKILLS
        .iter()
        .find(|preset| preset.dir_name == dir_name)
}

/// 这个目录名对应预设技能的托管注入条件；不是托管技能时为 `None`。
pub(crate) fn managed_injection_for_dir(dir_name: &str) -> Option<ManagedSkillInjection> {
    workspace_preset_skill(dir_name).and_then(|preset| preset.managed)
}

/// 按技能名判定是否托管。
///
/// 启停接口与人格候选手上只有技能名，没有目录。技能名取自同一份 SKILL.md 的 frontmatter，
/// 这几个托管技能的名字与目录名一致；两者不一致时以目录名为准——目录名是代码写死的那一份，
/// 名字是文件内容，文件本来就不归用户改。
pub(crate) fn is_managed_skill_name(name: &str) -> bool {
    managed_injection_for_dir(name).is_some()
}
