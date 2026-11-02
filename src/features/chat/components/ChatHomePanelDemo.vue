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
      <select v-model="statePreset" class="select select-bordered select-xs">
        <option value="working">有队员在工作</option>
        <option value="idle">全部空闲</option>
      </select>
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
        :kanban-visible="true"
        :kanban-issues="visibleKanbanIssues"
        :open-files="demoOpenFiles"
        @open-side-chat-panel="noop('openSideChatPanel')"
        @open-team-member="(id: string) => noop(`openTeamMember:${id}`)"
        @open-team-management="noop('openTeamManagement')"
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
import { computed, nextTick, onMounted, ref, watch } from "vue";
import ChatHomePanel from "./ChatHomePanel.vue";
import type { KanbanIssue } from "../composables/use-kanban-board";
import type { TeamMemberTileItem } from "./team-member-tile";

/**
 * 议题看板磁贴的 mock：分类数与「最长分类条数」都超过上限，用来验证磁贴高度封顶在 2 行、
 * 多出来的议题在卡内滚动，而不是把网格撑高。
 */
const demoKanbanIssues: KanbanIssue[] = [
  { number: 3, title: "发布流程要不要加一步回归检查", status: "in_review", assignee: "小吉祥草王", note: "三条回归都跑过了", updatedAt: "2026-11-01T11:20:00+08:00" },
  { number: 5, title: "看板磁贴尺寸定稿", status: "in_review", assignee: "莫娜", updatedAt: "2026-11-01T11:35:00+08:00", note: "" },
  { number: 11, title: "提问工具的回答要不要带时间戳", status: "in_review", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:46:00+08:00", note: "" },
  { number: 12, title: "队伍名册改角色后的文案", status: "in_review", assignee: "leader", updatedAt: "2026-11-01T11:47:00+08:00", note: "" },
  { number: 7, title: "队员 3 号的 worktree 有未解决冲突", status: "blocked", assignee: "桑多涅", note: "需要你定谁先合", updatedAt: "2026-11-01T10:05:00+08:00" },
  { number: 13, title: "摘要工具的回滚逻辑要重做", status: "blocked", assignee: "钟离", note: "等确认快照失败时回滚到哪一版", updatedAt: "2026-11-01T10:30:00+08:00" },
  { number: 8, title: "提问工具要不要支持多选", status: "discussing", assignee: "欧科塔维亚", updatedAt: "2026-11-01T11:40:00+08:00", note: "" },
  { number: 1, title: "模型继承链改成父会话优先", status: "in_progress", assignee: "小黑塔", updatedAt: "2026-11-01T11:10:00+08:00", note: "" },
  { number: 2, title: "专家模型改成被动跟随", status: "todo", assignee: "小黑塔", note: "" },
  { number: 6, title: "队伍名册改用角色标识", status: "done", assignee: "桑多涅", note: "已确认由界面通过" },
  { number: 14, title: "看板页要不要支持拖拽改状态", status: "done", assignee: "莫娜", updatedAt: "2026-11-01T12:10:00+08:00", note: "" },
];

/** 文件磁贴 mock：覆盖按扩展名取图标的几类（markdown / vue / json / rust）加一条未知扩展名，验证兜底回落到通用文件图标。 */
const demoOpenFiles = [
  { path: "E:/demo/workspace/docs/team-leader-rule.md", label: "team-leader-rule.md" },
  { path: "E:/demo/workspace/src/views/ChatView.vue", label: "ChatView.vue" },
  { path: "E:/demo/workspace/src/locales/zh-CN.json", label: "zh-CN.json" },
  { path: "E:/demo/workspace/src-tauri/src/main.rs", label: "main.rs" },
  { path: "E:/demo/workspace/data/unknown.foo", label: "unknown.foo" },
];

const widthParam = Number(new URLSearchParams(window.location.search).get("w") || 0);
const demoWidth = ref(widthParam > 0 ? widthParam : 586);
const withTeam = ref(true);

/** `?k=progress` 只喂进展态议题（决策态一条没有），用来验证「另一侧空时切换按钮不可点」。 */
const kanbanPreset = new URLSearchParams(window.location.search).get("k") || "both";
const PROGRESS_STATUSES = ["in_progress", "done"];

const visibleKanbanIssues = computed(() =>
  kanbanPreset === "progress"
    ? demoKanbanIssues.filter((issue) => PROGRESS_STATUSES.includes(issue.status))
    : demoKanbanIssues,
);

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

/**
 * 两种状态都备好，用来截「有队员在工作」与「全部空闲」：
 * 工作中那一档三种情形各放一个（忙碌且有议题、仅忙碌、仅有议题），
 * 空闲与出错的两条故意留在数据里——它们正是要被筛掉的那两类。
 */
const workingPreset: TeamMemberTileItem[] = [
  { ...baseMembers[0], memberState: "busy", activeIssue: { number: 3, title: "重构登录流程", status: "in_progress" } },
  { ...baseMembers[1], memberState: "busy", runtimeState: "streaming" },
  { ...baseMembers[2], memberState: "idle", activeIssue: { number: 8, title: "补齐结算页用例", status: "in_progress" } },
  { ...baseMembers[3], memberState: "idle" },
  { ...baseMembers[4], memberState: "error" },
];

const idlePreset: TeamMemberTileItem[] = (
  countParam > 0 ? baseMembers.slice(0, countParam) : baseMembers
).map((member) => ({ ...member, memberState: "idle" }));

const statePreset = ref<"working" | "idle">("working");
const members = computed(() => (statePreset.value === "working" ? workingPreset : idlePreset));

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
