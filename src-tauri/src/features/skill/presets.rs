#[allow(dead_code)]
pub(crate) struct WorkspacePresetSkill {
    pub dir_name: &'static str,
    pub skill_md: &'static str,
}

const WORKSPACE_PRESET_SKILLS: &[WorkspacePresetSkill] = &[
    WorkspacePresetSkill {
        dir_name: "browser-automation",
        skill_md: include_str!("../../../resources/preset-skills/browser-automation/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "news-analyst",
        skill_md: include_str!("../../../resources/preset-skills/news-analyst/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "agent-office",
        skill_md: include_str!("../../../resources/preset-skills/agent-office/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "agents-md-setup",
        skill_md: include_str!("../../../resources/preset-skills/agents-md-setup/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "assistant-interaction-guide",
        skill_md: include_str!("../../../resources/preset-skills/assistant-interaction-guide/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "p-ai-config-guide",
        skill_md: include_str!("../../../resources/preset-skills/p-ai-config-guide/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "pai-guide",
        skill_md: include_str!("../../../resources/preset-skills/pai-guide/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "code-review",
        skill_md: include_str!("../../../resources/preset-skills/code-review/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "memory-generation",
        skill_md: include_str!("../../../resources/preset-skills/memory-generation/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "reviewer",
        skill_md: include_str!("../../../resources/preset-skills/reviewer/SKILL.md"),
    },
    WorkspacePresetSkill {
        dir_name: "support",
        skill_md: include_str!("../../../resources/preset-skills/support/SKILL.md"),
    },
];

#[allow(dead_code)]
pub(crate) fn workspace_preset_skills() -> &'static [WorkspacePresetSkill] {
    WORKSPACE_PRESET_SKILLS
}
