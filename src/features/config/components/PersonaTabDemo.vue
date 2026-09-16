<template>
  <div class="h-screen w-full bg-base-200">
    <PersonaTab
      ref="tabRef"
      :personas="personas"
      :assistant-personas="personas"
      persona-editor-id="default-agent"
      :selected-persona="personas[0]"
      selected-persona-avatar-url=""
      :avatar-saving="false"
      avatar-error=""
      :persona-saving="false"
      :persona-dirty="false"
      :config-saving="false"
      :save-relations="saveRelations"
      :set-status-action="setStatus"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { PermissionCatalog, PersonaProfile, SkillSummaryItem } from "../../../types/app";
import PersonaTab from "../views/config-tabs/PersonaTab.vue";

const props = withDefaults(defineProps<{ view?: "profile" | "permission" | "delegate" | "injection" }>(), {
  view: "profile",
});

const tabRef = ref<InstanceType<typeof PersonaTab> | null>(null);
const statusText = ref("");

function makePersona(input: Partial<PersonaProfile> & { id: string; name: string }): PersonaProfile {
  return {
    systemPrompt: "",
    tools: [],
    createdAt: "2026-09-16T00:00:00Z",
    updatedAt: "2026-09-16T00:00:00Z",
    ...input,
  };
}

const personas = ref<PersonaProfile[]>([
  makePersona({
    id: "default-agent",
    name: "Pai",
    systemPrompt: "你是 PAI 的主助理，负责理解用户的意图并分派任务。",
    childAgentIds: ["reviewer", "saddler"],
    residentSkillNames: ["pai-guide", "assistant-space-guide", "code-review"],
    permissionControl: {
      enabled: true,
      mode: "whitelist",
      builtinToolNames: ["exec", "read", "fetch", "delegate"],
      skillNames: [],
      mcpToolNames: ["send_message", "get_group_member_list"],
    },
  }),
  makePersona({ id: "reviewer", name: "审阅者", childAgentIds: [] }),
  makePersona({ id: "saddler", name: "马鞍匠", childAgentIds: [] }),
  makePersona({ id: "explorer", name: "探索者", childAgentIds: [] }),
  makePersona({ id: "support", name: "客服", childAgentIds: [] }),
]);

const skills: SkillSummaryItem[] = [
  { name: "pai-guide", description: "解释 pai 应用的能力边界与关键概念", content: "", path: "preset/pai-guide", isBuiltin: true },
  { name: "assistant-space-guide", description: "管理助理空间中的共享资产", content: "", path: "preset/assistant-space-guide", isBuiltin: true },
  { name: "code-review", description: "审查当前工作区代码改动", content: "", path: "preset/code-review", isBuiltin: true },
  { name: "memory-generation", description: "判断并生成合格记忆", content: "", path: "preset/memory-generation", isBuiltin: true },
  { name: "mcp-setup", description: "安装、配置、排查 MCP", content: "", path: "preset/mcp-setup", isBuiltin: true },
  { name: "zhihu-kit", description: "知乎搜索、直答、热榜与 PPT 生成", content: "", path: "preset/zhihu-kit", isBuiltin: true },
  { name: "每周复盘", description: "汇总本周会话与委托，产出复盘草稿", content: "", path: "project/weekly-review", isBuiltin: false },
  { name: "竞品跟踪", description: "定期抓取竞品更新并对比", content: "", path: "project/competitor-watch", isBuiltin: false },
];

const catalog: PermissionCatalog = {
  builtinTools: [
    { name: "read", description: "读取本地文档内容" },
    { name: "write", description: "新增或整写一个完整文件" },
    { name: "update", description: "修改文件局部内容" },
    { name: "delete", description: "删除整个文件" },
    { name: "move", description: "移动或重命名文件" },
    { name: "exec", description: "执行一次性命令" },
    { name: "fetch", description: "抓取网页正文" },
    { name: "websearch", description: "搜索互联网内容" },
    { name: "operate", description: "自动化操作桌面应用" },
    { name: "windows", description: "窗口管理" },
    { name: "delegate", description: "向下级发起委托" },
    { name: "image_generate", description: "生成图片" },
    { name: "meme", description: "收编贴纸" },
  ],
  skills: [
    { name: "pai-guide", description: "解释 pai 应用的能力边界与关键概念" },
    { name: "code-review", description: "审查当前工作区代码改动" },
    { name: "memory-generation", description: "判断并生成合格记忆" },
    { name: "mcp-setup", description: "安装、配置、排查 MCP" },
    { name: "zhihu-kit", description: "知乎搜索、直答、热榜与 PPT 生成" },
    { name: "每周复盘", description: "汇总本周会话与委托，产出复盘草稿" },
  ],
  mcpTools: [
    { name: "send_message", description: "向指定会话发送消息", group: "钉钉" },
    { name: "list_departments", description: "列出组织部门", group: "钉钉" },
    { name: "get_group_member_list", description: "获取群成员列表", group: "OneBot" },
    { name: "send_group_msg", description: "发送群消息", group: "OneBot" },
  ],
};

function setStatus(message: string) {
  statusText.value = message;
}

async function saveRelations(): Promise<boolean> {
  return true;
}

onMounted(() => {
  const tab = tabRef.value;
  if (!tab) return;
  tab.previewSetCapabilityData(skills, catalog);
  tab.previewOpenPersona("default-agent");
  if (props.view !== "profile") tab.previewSetDetailView(props.view);
});
</script>
