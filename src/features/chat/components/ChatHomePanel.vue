<template>
  <div class="relative flex h-full min-h-0 w-full flex-col overflow-hidden bg-base-200">
    <OverlayScrollArea class="relative min-h-0 flex-1" scroller-class="ecall-chat-scroll-container min-h-0 h-full p-4">
      <HomeTileFlow v-if="hasAnyCard">
        <HomeKanbanCard
          v-if="kanbanVisible"
          :issues="kanbanIssues"
          :leader-name="leaderName"
          :leader-avatar-url="leaderAvatarUrl"
          :persona-name-map="personaNameMap"
          :persona-avatar-url-map="personaAvatarUrlMap || teamMemberAvatarUrlMap"
          :team-members="teamMembers"
          @open="emit('openKanban')"
        />
        <HomeEnvCard
          v-if="workspaceRootPath"
          :repo-root="gitRepoRoot || workspaceRootPath"
          :branch="branch"
          :change-count="changeCount"
          :commits="recentCommits"
          @open-changes="emit('openGitChanges')"
          @open-commits="emit('openGitCommits')"
          @branch-switched="emit('branchSwitched', gitRepoRoot || workspaceRootPath)"
          @error="(message) => emit('gitError', message)"
        />
        <HomePlanCard
          v-if="latestPlan"
          :plan="latestPlan"
          :conversation-id="conversationId"
          @open="(path) => emit('openFile', path)"
        />
        <HomeWorkspaceCard
          v-if="workspaceRootPath"
          :workspace-root-path="workspaceRootPath"
          :worktree-path="worktreePath"
          :worktree-branch="worktreeBranch"
          :work-mode="workMode"
          @open="(path) => emit('openWorkspace', path)"
          @error="(message) => emit('gitError', message)"
        />
        <HomeSideChatCreateCard
          v-if="sideChatEnabled"
          @open="emit('openSideChatPanel')"
        />
        <!-- 队员状态表：整块收在 HomeTeamMemberBlock 里，占格、行高、滚动高度都由它自己定
             （demo 页渲染同一个组件，不再各抄一份样式）。点某一格打开该队员会话，走队员面板 -->
        <HomeTeamMemberBlock
          v-if="teamVisible"
          :members="teamMembers"
          :avatar-url-map="teamMemberAvatarUrlMap"
          @select-member="emit('openTeamMember', $event)"
        />
        <HomeToolCard
          v-if="activeToolBatches.length"
          :batches="activeToolBatches"
          @open="emit('openMonitorTab', 'tools')"
        />
        <HomeShellCard
          v-for="shell in runningShells"
          :key="`shell-${shell.id}`"
          :description="shell.description"
          :command="shell.command"
          :started-at="shell.startedAt"
          :now-ms="nowMs"
          interruptible
          @interrupt="emit('interruptShell', shell.id)"
        />
        <HomeDelegateCard
          v-for="delegate in runningDelegates"
          :key="`delegate-${delegate.delegateId}`"
          :title="delegate.title || delegate.delegateId"
          :elapsed-ms="delegate.elapsedMs"
          :request-count="delegate.requestCount"
          :token-count="delegate.tokenCount"
          :last-tool-name="delegate.lastToolName"
          pulsing
          @open="emit('openMonitorTab', 'delegate')"
        />
        <HomeDelegateCard
          v-if="latestFinishedDelegate"
          :key="`finished-delegate-${latestFinishedDelegate.delegateId}`"
          :title="latestFinishedDelegate.title || latestFinishedDelegate.delegateId"
          :meta-label="finishedDelegateMeta"
          @open="emit('openMonitorTab', 'delegate')"
        />
        <HomeTaskCard
          v-for="task in runningTasks"
          :key="`task-${task.taskId}`"
          :goal="task.goal"
          :next-run-at="task.trigger?.next_run_at"
          :now-ms="nowMs"
          @open="emit('openMonitorTab', 'tasks')"
        />
        <HomeFileCard
          v-for="file in openFiles"
          :key="`open-file-${file.path}`"
          :label="file.label"
          :path="file.path"
          :active="file.path === activePath"
          @open="emit('openFile', file.path)"
          @close="emit('closeFile', file.path)"
        />
      </HomeTileFlow>
      <div v-else class="flex h-full flex-col items-center justify-center p-6 text-center">
        <div class="max-w-xs space-y-3">
          <div class="mx-auto flex size-12 items-center justify-center rounded-2xl bg-base-300/40 text-base-content/50">
            <LayoutGrid class="size-6" />
          </div>
          <div class="space-y-1">
            <div class="text-sm font-medium text-base-content/80">{{ t("chat.homePanel.emptyAll") }}</div>
            <div class="text-xs text-base-content/50 leading-relaxed">
              {{ t("chat.homePanel.emptyAllHint") }}
            </div>
          </div>
          <div class="pt-2">
            <button
              type="button"
              class="btn btn-sm btn-outline border-base-300 gap-1.5"
              @click="emit('openWorkspace')"
            >
              <FolderTree class="size-4" />
              {{ t("chat.homePanel.workspace") }}
            </button>
          </div>
        </div>
      </div>
    </OverlayScrollArea>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { FolderTree, LayoutGrid } from "@lucide/vue";
import type { BackgroundShellTaskSummary, ConversationDelegateStatusSummary, ShellWorkMode } from "../../../types/app";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import type { ToolReviewBatchSummary } from "../composables/use-chat-tool-review";
import type { ChatMonitorPanelMode } from "../composables/chat-ui-layout-storage";
import type { KanbanIssue } from "../composables/use-kanban-board";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import HomeEnvCard from "./chat-home/HomeEnvCard.vue";
import HomeKanbanCard from "./chat-home/HomeKanbanCard.vue";
import HomeFileCard from "./chat-home/HomeFileCard.vue";
import HomeWorkspaceCard from "./chat-home/HomeWorkspaceCard.vue";
import HomeSideChatCreateCard from "./chat-home/HomeSideChatCreateCard.vue";
import HomeTileFlow from "./chat-home/HomeTileFlow.vue";
import HomeTeamMemberBlock from "./chat-home/HomeTeamMemberBlock.vue";
import HomeToolCard from "./chat-home/HomeToolCard.vue";
import HomeShellCard from "./chat-home/HomeShellCard.vue";
import HomeDelegateCard from "./chat-home/HomeDelegateCard.vue";
import HomeTaskCard from "./chat-home/HomeTaskCard.vue";
import HomePlanCard, { type LatestPlanSummary } from "./chat-home/HomePlanCard.vue";

const props = withDefaults(defineProps<{
  conversationId?: string;
  latestPlan?: LatestPlanSummary | null;
  workspaceRootPath?: string;
  worktreePath?: string;
  worktreeBranch?: string;
  workMode?: ShellWorkMode;
  /** Git 仓库根；与工作目录可能不同（仓库在上级目录），为空时回落到工作目录 */
  gitRepoRoot?: string;
  branch?: string;
  changeCount?: number;
  /** 当前仓库最近几条提交；来源与分支、更改列表同一处 */
  recentCommits?: Array<{ hash: string; message: string }>;
  /** 已打开文件，path 为绝对路径，label 仅用于展示 */
  openFiles?: Array<{ path: string; label: string }>;
  activePath?: string;
  /** 追问能力是否可用；不可用时首页不出现「追问」入口磁贴 */
  sideChatEnabled?: boolean;
  /** 队伍状态表是否可见；只有能拥有名册的会话（chat / 远程私聊）才显示 */
  teamVisible?: boolean;
  /** 队员状态表数据：字段形状对齐队员磁贴 */
  teamMembers?: Array<{
    conversationId: string;
    title: string;
    teamName?: string;
    agentId?: string;
    agentName?: string;
    progressState?: string;
    /** 他手上在办的那条议题；没有就退回 memberState */
    activeIssue?: { number: number; title: string; status: string };
    /** 运行时三态：busy / idle / error */
    memberState?: string;
  }>;
  /** 队员磁贴的头像来源：按 agentId 查 */
  teamMemberAvatarUrlMap?: Record<string, string>;
  leaderName?: string;
  leaderAvatarUrl?: string;
  personaNameMap?: Record<string, string>;
  personaAvatarUrlMap?: Record<string, string>;
  delegates?: ConversationDelegateStatusSummary[];
  runningTasks?: TaskEntry[];
  shells?: BackgroundShellTaskSummary[];
  /** 工具评审批次；只传有工具调用的批次（由 ChatHomePanel 过滤） */
  toolBatches?: ToolReviewBatchSummary[];
  /** 议题看板磁贴是否可见；只有本地主会话才有看板 */
  kanbanVisible?: boolean;
  /** 看板数据与看板页同源，磁贴只截取需要用户参与的四类 */
  kanbanIssues?: KanbanIssue[];
}>(), {
  conversationId: "",
  latestPlan: null,
  workspaceRootPath: "",
  worktreePath: "",
  worktreeBranch: "",
  workMode: "directory",
  gitRepoRoot: "",
  branch: "",
  changeCount: 0,
  recentCommits: () => [],
  openFiles: () => [],
  activePath: "",
  sideChatEnabled: false,
  teamVisible: false,
  teamMembers: () => [],
  teamMemberAvatarUrlMap: () => ({}),
  leaderName: "",
  leaderAvatarUrl: "",
  personaNameMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  delegates: () => [],
  runningTasks: () => [],
  shells: () => [],
  toolBatches: () => [],
  kanbanVisible: false,
  kanbanIssues: () => [],
});

const emit = defineEmits<{
  (e: "selectPanel", value: "reader" | "monitor" | "sideChat"): void;
  (e: "openFile", path: string): void;
  (e: "closeFile", path: string): void;
  /** 追问入口磁贴：只切面板，新建留在追问面板里 */
  (e: "openSideChatPanel"): void;
  /** 队员状态表点某一格：打开该队员会话（切队员面板 + 激活会话） */
  (e: "openTeamMember", conversationId: string): void;
  (e: "openWorkspace", path?: string): void;
  (e: "openGitChanges"): void;
  (e: "openGitCommits"): void;
  (e: "branchSwitched", workspacePath: string): void;
  (e: "gitError", message: string): void;
  (e: "openMonitorTab", value: ChatMonitorPanelMode): void;
  (e: "interruptShell", taskId: string): void;
  (e: "openKanban"): void;
}>();

const { t } = useI18n();

const runningShells = computed(() =>
  props.shells.filter((task) => String(task.status || "").trim() === "running"),
);

const runningDelegates = computed(() =>
  props.delegates.filter((delegate) => {
    const status = String(delegate.status || "").trim();
    return delegate.active && (status === "running" || status === "delivered");
  }),
);

/** 最近一条已结束的委托：跑完不消失，像「最近工具」那样留一张卡 */
const latestFinishedDelegate = computed<ConversationDelegateStatusSummary | null>(() => {
  const finished = props.delegates.filter((delegate) => !delegate.active);
  if (!finished.length) return null;
  return finished
    .slice()
    .sort((left, right) => timestampMs(right.updatedAt) - timestampMs(left.updatedAt))[0] || null;
});

const finishedDelegateMeta = computed(() => {
  const status = String(latestFinishedDelegate.value?.status || "").trim();
  if (status === "failed") return "已失败";
  if (status === "completed") return "已完成";
  return "被中断";
});

function timestampMs(value: unknown) {
  const ms = Date.parse(String(value || ""));
  return Number.isFinite(ms) ? ms : 0;
}

/** 没有工具调用的轮次不构成卡片：红豆口径是「没工具自然就不显示」 */
const activeToolBatches = computed(() =>
  props.toolBatches.filter((batch) => Number(batch.itemCount || 0) > 0),
);

const hasAnyCard = computed(
  () =>
    Boolean(String(props.workspaceRootPath || "").trim())
    || Boolean(props.latestPlan)
    || props.sideChatEnabled
    || Boolean(props.teamVisible)
    || Boolean(props.kanbanVisible)
    || props.openFiles.length > 0
    || runningShells.value.length > 0
    || runningDelegates.value.length > 0
    || props.runningTasks.length > 0
    || activeToolBatches.value.length > 0,
);

// 后台终端时长与任务倒计时不逐秒推送，由容器统一提供一个时钟；无需要计时的卡片时不空转
const nowMs = ref(Date.now());
let clockTimer: ReturnType<typeof window.setInterval> | null = null;

const needsClock = computed(
  () => runningShells.value.length > 0 || props.runningTasks.some((task) => Boolean(String(task.trigger?.next_run_at || "").trim())),
);

watch(
  needsClock,
  (shouldTick) => {
    if (shouldTick && clockTimer == null) {
      clockTimer = window.setInterval(() => {
        nowMs.value = Date.now();
      }, 1000);
      return;
    }
    if (!shouldTick && clockTimer != null) {
      window.clearInterval(clockTimer);
      clockTimer = null;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (clockTimer != null) {
    window.clearInterval(clockTimer);
    clockTimer = null;
  }
});
</script>

<style scoped>
/* 磁贴墙的网格参数与队员状态表的占格/行高都收在 chat-home/HomeTileFlow.vue 与
   chat-home/HomeTeamMemberBlock.vue 里：宿主只决定放哪些卡，不再持有这些布局常量，
   否则 demo 页要复现同一套样式只能抄一份，一改就两边不一致。 */
</style>

