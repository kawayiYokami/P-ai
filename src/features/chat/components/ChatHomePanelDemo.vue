<template>
  <div class="flex flex-col gap-2 p-2">
    <div class="flex items-center gap-3">
      <select v-model.number="demoWidth" class="select select-bordered select-xs">
        <option :value="586">586（两列贴墙）</option>
        <option :value="420">420</option>
        <option :value="320">320（真机右栏默认宽度）</option>
        <option :value="300">300</option>
      </select>
      <label class="flex items-center gap-1 text-xs">
        <input v-model="withTeam" type="checkbox" class="checkbox checkbox-xs" />
        有名册
      </label>
      <span class="text-xs text-base-content/60">{{ members.length }} 名队员</span>
    </div>
    <div
      class="relative flex h-[640px] flex-col overflow-hidden rounded-box border border-base-300 bg-base-100"
      :style="{ width: `min(${demoWidth}px, 100%)` }"
    >
      <ChatHomePanel
        conversation-id="demo-conversation"
        workspace-root-path="E:/demo/workspace"
        side-chat-enabled
        :team-visible="withTeam"
        :team-members="members"
        @open-side-chat-panel="noop('openSideChatPanel')"
        @open-team-member="(id: string) => noop(`openTeamMember:${id}`)"
        @open-workspace="() => noop('openWorkspace')"
      />
    </div>
    <p class="text-xs text-base-content/60">
      demo：首页磁贴区只喂「追问入口 + 队员状态表」，队员数据是 mock，点击原地丢弃。
    </p>
    <p class="demo-measure text-xs text-base-content/50">{{ measureText }}</p>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import ChatHomePanel from "./ChatHomePanel.vue";

const widthParam = Number(new URLSearchParams(window.location.search).get("w") || 0);
const demoWidth = ref(widthParam > 0 ? widthParam : 586);
const withTeam = ref(true);

/** 队员磁贴的实际高度与列数：用来核对「两行固定高度」是否算对，别靠猜 */
const measureText = ref("量测中…");

const baseMembers = [
  // 第一条专门验证标题降级：没有标题时应当落到人格名，而不是显示裸 id 或空行
  { conversationId: "m0", title: "", teamName: "郑工", agentId: "a0", agentName: "郑工" },
  { conversationId: "m1", title: "重构登录流程", teamName: "李工", agentId: "a1", agentName: "李工" },
  { conversationId: "m2", title: "补齐结算页用例", teamName: "王工", agentId: "a2", agentName: "王工" },
  { conversationId: "m3", title: "一个特别特别长的会话标题用来验证单行截断是否生效", teamName: "赵工", agentId: "a3", agentName: "赵工" },
  { conversationId: "m4", title: "远程队长名册", teamName: "钱工", agentId: "", agentName: "钱工" },
  { conversationId: "m5", title: "第五个人验证滚动", teamName: "孙工", agentId: "a5", agentName: "孙工" },
  { conversationId: "m6", title: "第六个人验证滚动", teamName: "周工", agentId: "a6", agentName: "周工" },
];

const countParam = Number(new URLSearchParams(window.location.search).get("n") || 0);
const members = countParam > 0 ? baseMembers.slice(0, countParam) : baseMembers;

async function measure() {
  try {
    await nextTick();
    const grid = document.querySelector(".team-member-tile-grid") as HTMLElement | null;
    const tile = grid?.firstElementChild as HTMLElement | null;
    if (!grid || !tile) {
      measureText.value = `没有渲染出队员磁贴（grid=${!!grid} tile=${!!tile}）`;
      return;
    }
    const tileHeight = Math.round(tile.getBoundingClientRect().height * 10) / 10;
    const gridStyle = window.getComputedStyle(grid);
    const columns = gridStyle.gridTemplateColumns.split(" ").filter(Boolean).length;
    const scrollHost = grid.closest(".overlay-scroll-area-scroller") as HTMLElement | null;
    measureText.value = [
      `磁贴 ${tileHeight}px`,
      `${columns} 列`,
      `内层宽 ${Math.round(grid.getBoundingClientRect().width)}px`,
      scrollHost
        ? `滚动区 可视 ${Math.round(scrollHost.clientHeight)}px / 内容 ${Math.round(scrollHost.scrollHeight)}px`
        : "滚动区未挂载",
    ].join(" · ");
  } catch (error) {
    measureText.value = `量测失败：${String(error)}`;
  }
}

onMounted(() => {
  void measure();
  // 磁贴墙有 220ms 入场动画（transform scale），动画期间 getBoundingClientRect 会偏小，
  // 量测必须等动画结束，否则会把 56px 量成 53.3px。
  window.setTimeout(() => void measure(), 900);
});

watch(demoWidth, () => {
  window.setTimeout(() => void measure(), 500);
});

function noop(label: string) {
  console.log("[demo]", label);
}
</script>
