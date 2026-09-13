<template>
  <div class="relative flex h-full min-h-0 w-full flex-col overflow-hidden bg-base-200">
    <OverlayScrollArea class="relative min-h-0 flex-1" scroller-class="ecall-chat-scroll-container min-h-0 h-full p-4">
      <div v-if="hasAnyCard" class="ecall-home-flow">
        <HomeGitCard
          v-if="workspaceRootPath"
          :workspace-root-path="workspaceRootPath"
          :branch="branch"
          :changes="gitChanges"
          :change-count="changeCount"
          @open-changes="emit('openGitChanges')"
          @open-file="(path) => emit('openFile', path)"
        />
        <HomeGitCommitsCard
          v-if="workspaceRootPath"
          :workspace-root-path="workspaceRootPath"
          :branch="branch"
          :commits="recentCommits"
          @open-commits="emit('openGitCommits')"
        />
        <HomePlanCard
          v-if="latestPlan"
          :plan="latestPlan"
          :conversation-id="conversationId"
          @open="(path) => emit('openFile', path)"
        />
        <HomeFilesCard
          v-if="openFiles.length"
          :files="openFiles"
          :active-path="activePath"
          :item-count="openFileCount"
          @open-panel="emit('selectPanel', 'reader')"
          @open-file="(path) => emit('openFile', path)"
        />
        <HomeWorkspaceCard
          v-if="workspaceRootPath"
          :workspace-root-path="workspaceRootPath"
          @open="emit('openWorkspace')"
        />
        <HomeSideChatCreateCard
          v-if="sideChatEnabled"
          @create="emit('createSideChat')"
        />
        <HomeToolCard
          v-if="activeToolBatches.length"
          :batches="activeToolBatches"
          @open="emit('openMonitorTab', 'tools')"
        />
        <HomeSideChatCard
          v-for="item in sideChats"
          :key="`side-chat-${item.id}`"
          :title="item.title"
          @open="emit('openSideChat', item.id)"
        />
        <HomeShellCard
          v-for="shell in runningShells"
          :key="`shell-${shell.id}`"
          :description="shell.description"
          :command="shell.command"
          :started-at="shell.startedAt"
          :now-ms="nowMs"
        />
        <HomeDelegateCard
          v-for="delegate in runningDelegates"
          :key="`delegate-${delegate.delegateId}`"
          :title="delegate.title || delegate.delegateId"
          :elapsed-ms="delegate.elapsedMs"
          :request-count="delegate.requestCount"
          :token-count="delegate.tokenCount"
          :last-tool-name="delegate.lastToolName"
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
      </div>
      <div v-else class="flex h-full flex-col items-center justify-center p-6 text-center">
        <div class="max-w-xs space-y-3">
          <div class="mx-auto flex size-12 items-center justify-center rounded-2xl bg-base-300/40 text-base-content/50">
            <LayoutGrid class="size-6" />
          </div>
          <div class="space-y-1">
            <div class="text-sm font-medium text-base-content/80">{{ t("chat.homePanel.emptyAll") }}</div>
            <div class="text-xs text-base-content/50 leading-relaxed">
              绑定工作区或在对话中产出代码、任务与工具后，此处将自动聚合展示实时看板。
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
import type { BackgroundShellTaskSummary, ConversationDelegateStatusSummary } from "../../../types/app";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import type { ToolReviewBatchSummary } from "../composables/use-chat-tool-review";
import type { ChatMonitorPanelMode } from "../composables/chat-ui-layout-storage";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import HomeGitCard from "./chat-home/HomeGitCard.vue";
import HomeGitCommitsCard from "./chat-home/HomeGitCommitsCard.vue";
import HomeFilesCard from "./chat-home/HomeFilesCard.vue";
import HomeWorkspaceCard from "./chat-home/HomeWorkspaceCard.vue";
import HomeSideChatCreateCard from "./chat-home/HomeSideChatCreateCard.vue";
import HomeToolCard from "./chat-home/HomeToolCard.vue";
import HomeSideChatCard from "./chat-home/HomeSideChatCard.vue";
import HomeShellCard from "./chat-home/HomeShellCard.vue";
import HomeDelegateCard from "./chat-home/HomeDelegateCard.vue";
import HomeTaskCard from "./chat-home/HomeTaskCard.vue";
import HomePlanCard, { type LatestPlanSummary } from "./chat-home/HomePlanCard.vue";

const props = withDefaults(defineProps<{
  conversationId?: string;
  latestPlan?: LatestPlanSummary | null;
  workspaceRootPath?: string;
  branch?: string;
  gitChanges?: Array<{ path: string; status: string }>;
  changeCount?: number;
  /** 当前仓库最近几条提交；来源与分支、更改列表同一处 */
  recentCommits?: Array<{ hash: string; message: string }>;
  /** 已打开文件，path 为绝对路径，label 仅用于展示 */
  openFiles?: Array<{ path: string; label: string }>;
  activePath?: string;
  openFileCount?: number;
  /** 当前打开的追问会话 */
  sideChats?: Array<{ id: string; title: string }>;
  /** 追问能力是否可用；不可用时首页不出现「新建追问」入口卡 */
  sideChatEnabled?: boolean;
  delegates?: ConversationDelegateStatusSummary[];
  runningTasks?: TaskEntry[];
  shells?: BackgroundShellTaskSummary[];
  /** 工具评审批次；只传有工具调用的批次（由 ChatHomePanel 过滤） */
  toolBatches?: ToolReviewBatchSummary[];
}>(), {
  conversationId: "",
  latestPlan: null,
  workspaceRootPath: "",
  branch: "",
  gitChanges: () => [],
  changeCount: 0,
  recentCommits: () => [],
  openFiles: () => [],
  activePath: "",
  openFileCount: 0,
  sideChats: () => [],
  sideChatEnabled: false,
  delegates: () => [],
  runningTasks: () => [],
  shells: () => [],
  toolBatches: () => [],
});

const emit = defineEmits<{
  (e: "selectPanel", value: "reader" | "monitor" | "sideChat"): void;
  (e: "openFile", path: string): void;
  (e: "openSideChat", conversationId: string): void;
  (e: "createSideChat"): void;
  (e: "openWorkspace"): void;
  (e: "openGitChanges"): void;
  (e: "openGitCommits"): void;
  (e: "openMonitorTab", value: ChatMonitorPanelMode): void;
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
    || props.openFiles.length > 0
    || props.sideChats.length > 0
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
/* 固定网格：列宽固定、宽卡跨两格，同一行的卡片自动等高 */
.ecall-home-flow {
  --ecall-home-tile: 8.75rem;
  /* 卡片间距与圆角（--radius-box: 1rem）对齐 */
  --ecall-home-gap: 1rem;
  container-type: inline-size;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--ecall-home-tile), 1fr));
  grid-auto-flow: dense;
  gap: var(--ecall-home-gap);
  align-content: start;
}

/* 窄容器下（如侧边栏单列时），宽卡自适应退化为单列，防止横向撑裂 */
@container (max-width: 320px) {
  :deep(.ecall-home-card-wide) {
    grid-column: span 1 !important;
  }
}

.ecall-home-flow > * {
  animation: ecall-home-card-enter 220ms cubic-bezier(0.2, 0, 0, 1);
}

@keyframes ecall-home-card-enter {
  from {
    opacity: 0;
    transform: scale(0.94);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ecall-home-flow > * {
    animation: none;
  }
}

</style>
