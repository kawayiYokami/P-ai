<template>
  <div class="flex flex-col gap-3">
    <div class="demo-controls flex flex-wrap items-center gap-3">
      <label class="flex items-center gap-2 text-xs text-base-content/70">
        磁贴墙宽度
        <select v-model.number="demoWidth" class="select select-bordered select-xs">
          <option :value="586">586（首页右栏默认，磁贴墙出 3 列）</option>
          <option :value="440">440（磁贴墙出 2 列，跨列块吃满整行）</option>
          <option :value="320">320（单列极限，块占满整行）</option>
        </select>
      </label>
      <label class="flex items-center gap-2 text-xs text-base-content/70">
        数据档位
        <select v-model="demoPreset" class="select select-bordered select-xs">
          <option value="full">完整状态（六态 + 三种头像兜底）</option>
          <option value="remote">远程路径（无 agentId、无状态）</option>
          <option value="with-issue">有在办议题（写「正在处理 #编号 标题」）</option>
          <option value="no-issue">没有议题（徽章显忙碌 / 空闲 / 出错）</option>
        </select>
      </label>
    </div>

    <!-- 直接复用首页的磁贴墙与队员状态表组件：跨列、行高、滚动高度都来自真实实现，
         手抄一份样式只会在改参数时两边不一致 -->
    <div class="rounded-box border border-base-300 bg-base-100 p-3" :style="{ width: `min(${demoWidth}px, 100%)` }">
      <HomeTileFlow>
        <div class="demo-home-card">追问入口（占位）</div>
        <HomeTeamMemberBlock :members="members" :avatar-url-map="avatarUrlMap" />
      </HomeTileFlow>
    </div>

    <div class="demo-notes rounded-box border border-base-300 bg-base-200/40 p-3 text-xs leading-relaxed text-base-content/60">
      <div>数据是 mock，交互一律不写库。当前档位：{{ presetLabel }}。</div>
      <div>「追问入口（占位）」是磁贴墙里相邻的另一块，用来看跨列块和它的对齐关系，不是本次组件。</div>
      <div>这里渲染的就是首页那两个组件：磁贴墙用 HomeTileFlow，队员状态表用 HomeTeamMemberBlock，不另写样式。</div>
      <div>头像三级兜底：有缓存头像用图 → 无人格头像用名字首字 → 连名字都没有才退回图标。</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { TeamMemberTileItem } from "./team-member-tile";
import HomeTileFlow from "./chat-home/HomeTileFlow.vue";
import HomeTeamMemberBlock from "./chat-home/HomeTeamMemberBlock.vue";

const demoWidth = ref(586);
const demoPreset = ref<"full" | "remote" | "with-issue" | "no-issue">("full");

const PRESET_LABEL: Record<string, string> = {
  full: "完整状态",
  remote: "远程路径",
  "with-issue": "有在办议题",
  "no-issue": "没有议题",
};
const presetLabel = computed(() => PRESET_LABEL[demoPreset.value] || demoPreset.value);

// 截图脚本改不走 Runtime.evaluate（上次带 js 的任务卡死），档位从 URL 读，一个档位一条 URL。
const demoParams = new URLSearchParams(window.location.search);
const initialWidth = Number(demoParams.get("w"));
if (Number.isFinite(initialWidth) && initialWidth > 0) {
  demoWidth.value = initialWidth;
}
if (demoParams.get("preset") === "remote") {
  demoPreset.value = "remote";
}
if (demoParams.get("preset") === "with-issue" || demoParams.get("preset") === "no-issue") {
  demoPreset.value = demoParams.get("preset") as "with-issue" | "no-issue";
}

/** 纯色 SVG 头像，避免 demo 依赖外网图片导致截图时首帧空白 */
function mockAvatar(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="${color}"/></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

const avatarUrlMap: Record<string, string> = {
  keqing: mockAvatar("#6d5bd0"),
  hutao: mockAvatar("#b45309"),
  ganyu: mockAvatar("#0f766e"),
};

const fullMembers: TeamMemberTileItem[] = [
  // 有头像 + 六个状态各来一个，验证徽章取色与文案
  // 刻晴这条带运行态：徽章后面应当挂一个转圈，用来看调度中的样子
  { conversationId: "c1", title: "刻晴的会话", teamName: "刻晴", agentId: "keqing", agentName: "刻晴", progressState: "in_progress", runtimeState: "assistant_streaming" },
  { conversationId: "c2", title: "胡桃的会话", teamName: "胡桃", agentId: "hutao", agentName: "胡桃", progressState: "in_review" },
  { conversationId: "c3", title: "甘雨的会话", teamName: "甘雨", agentId: "ganyu", agentName: "甘雨", progressState: "done" },
  // 无头像但有 agentId：验证首字兜底（中文取首字）
  { conversationId: "c4", title: "钟离的会话", teamName: "钟离", agentId: "zhongli", agentName: "钟离", progressState: "blocked" },
  // 无 agentId：验证首字兜底（走 teamName）
  { conversationId: "c5", title: "温迪的会话", teamName: "温迪", progressState: "todo" },
  { conversationId: "c6", title: "可莉的会话", teamName: "可莉", agentId: "klee", agentName: "可莉", progressState: "planning" },
  // 有 agentId 但头像未缓存、状态也未设置：验证「未设置」有徽章而不是空白
  { conversationId: "c7", title: "阿贝多的会话", teamName: "阿贝多", agentId: "albedo", agentName: "阿贝多", progressState: "" },
  // 超长名字：验证单行截断不会把同排磁贴撑成不同高度
  { conversationId: "c8", title: "一斗的会话", teamName: "荒泷一斗和他的帮派全名", agentId: "itto", agentName: "荒泷一斗" },
];

const remoteMembers: TeamMemberTileItem[] = [
  { conversationId: "r1", title: "纳西妲", teamName: "纳西妲" },
  { conversationId: "r2", title: "雷电将军", teamName: "雷电将军" },
  { conversationId: "r3", title: "八重神子", teamName: "八重神子" },
  { conversationId: "r4", title: "Remote Contact Long Name Here", teamName: "Remote Contact Long Name Here" },
];

/**
 * 有在办议题：徽章显示议题状态，名字后面跟议题标题。
 * 第二条故意不带议题，用来对照「同一个墙里两种口径并存」——
 * 带议题的看议题，不带议题的退回自己的运行时状态。
 * 只放两条：队员块内部一次只看得到两行，多了会被滚动裁掉，截图里反而像丢人。
 */
const withIssueMembers: TeamMemberTileItem[] = [
  {
    conversationId: "i1",
    title: "刻晴的会话",
    teamName: "刻晴",
    agentId: "keqing",
    agentName: "刻晴",
    progressState: "in_progress",
    memberState: "busy",
    runtimeState: "assistant_streaming",
    activeIssue: { number: 3, title: "磁贴徽章口径重做", status: "in_progress" },
  },
  {
    conversationId: "i2",
    title: "钟离的会话",
    teamName: "钟离",
    agentId: "zhongli",
    agentName: "钟离",
    progressState: "todo",
    memberState: "idle",
  },
];

/** 没有议题：徽章退回运行时三态，忙碌与出错各一条（空闲那条在有议题档位里） */
const noIssueMembers: TeamMemberTileItem[] = [
  { conversationId: "n1", title: "刻晴的会话", teamName: "刻晴", agentId: "keqing", agentName: "刻晴", progressState: "in_progress", memberState: "busy" },
  { conversationId: "n2", title: "钟离的会话", teamName: "钟离", agentId: "zhongli", agentName: "钟离", progressState: "blocked", memberState: "error" },
];

const members = computed(() => {
  if (demoPreset.value === "remote") return remoteMembers;
  if (demoPreset.value === "with-issue") return withIssueMembers;
  if (demoPreset.value === "no-issue") return noIssueMembers;
  return fullMembers;
});
</script>

<style scoped>
/* 占位块只提供参照，样式故意做轻，别让人误认成产品界面 */
.demo-home-card {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 6.5rem;
  border: 1px dashed var(--color-base-content, #888);
  border-radius: 1rem;
  opacity: 0.35;
  font-size: 0.75rem;
  text-align: center;
  padding: 0.5rem;
}
</style>
