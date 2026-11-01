<template>
  <!-- demo 说明：喂的是「两个托管技能 + 一个普通内置技能 + 一个自定义技能」的清单，
       只渲染真实 SkillTab，开关点击被 mock 掉，不写任何库。 -->
  <div class="flex h-[880px] w-full justify-center overflow-hidden bg-base-200 px-4 py-3">
    <div class="w-full max-w-[960px] min-w-0">
      <SkillTab />
    </div>
  </div>
</template>

<script setup lang="ts">
import SkillTab from "../views/config-tabs/SkillTab.vue";

const SKILLS_ROOT = "/demo/llm-workspace/skills";

function demoSkill(input: {
  name: string;
  description: string;
  content: string;
  isBuiltin: boolean;
  managed?: boolean;
  enabled?: boolean;
}) {
  return {
    additionalFiles: [],
    enabled: true,
    ...input,
    managed: input.managed ?? false,
    path: `${SKILLS_ROOT}/${input.name}/SKILL.md`,
  };
}

const MOCK_SKILLS = [
  demoSkill({
    name: "weekly-review",
    description: "把一周的提交与议题整理成一份周报，按项目分组。",
    content: "# 周报\n\n把结果按项目分组输出。",
    isBuiltin: false,
  }),
  demoSkill({
    name: "persona-craft",
    description:
      "为接下来的工作造一个合适的队员人格。写身份履历而不是办事要求，让专业能力通过暗示自然长出来。招募队员之前使用。",
    content: "# 给这项工作造一个人\n\n造人不是写一份要求清单，是给这件事配一个有相关经历的人。",
    isBuiltin: true,
    managed: true,
  }),
  demoSkill({
    name: "pai-guide",
    description: "解释 PAI 的能力边界、窗口分工、人格、组织、工具、Skill、MCP、委托与记忆之间的关系。",
    content: "# PAI 使用指南\n\n先讲能力边界，再讲概念关系。",
    isBuiltin: true,
    managed: true,
  }),
  demoSkill({
    name: "code-review",
    description: "审查当前工作区的代码改动，按风险排序给出结论。",
    content: "# 代码审查\n\n先定位改动范围，再逐条核对。",
    isBuiltin: true,
  }),
];

// demo 专用：把 Tauri IPC 顶成内存桩，让真实 SkillTab 以为自己在真机上跑，
// 从而拿到上面这份清单。只在本 demo 的 setup 里安装，别的 demo 与真机都不受影响。
function installFakeInvoke() {
  const target = window as unknown as {
    __TAURI_INTERNALS__?: { invoke?: (...args: unknown[]) => unknown };
  };
  if (typeof target.__TAURI_INTERNALS__?.invoke === "function") return;
  target.__TAURI_INTERNALS__ = {
    invoke: async (command: unknown) => {
      if (String(command).includes("skill")) {
        return { skills: MOCK_SKILLS, errors: [] };
      }
      return null;
    },
  };
}

installFakeInvoke();
</script>
