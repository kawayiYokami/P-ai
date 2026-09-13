<template>
  <!-- 工作区：权限用文字，工作树模式在右上角挂一颗胶囊标签 -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition duration-200 ease-out"
    leave-to-class="opacity-0 translate-y-1"
  >
    <button
      v-if="showWorkspaceButton"
      type="button"
      :class="[SESSION_FLOAT_FROST_PILL, 'relative max-w-[min(32rem,100%)]']"
      :disabled="workspaceButtonDisabled"
      :title="workspaceTitle"
      @click="emit('lockWorkspace')"
    >
      <span
        v-if="workspaceWorkMode === 'worktree'"
        class="absolute -right-1 -top-1.5 shrink-0 rounded-full border border-primary/30 bg-primary/15 px-1.5 py-0.5 text-caption font-medium leading-none text-primary backdrop-blur-md"
      >
        {{ t("chat.workspaceStatusModeWorktree") }}
      </span>
      <span class="shrink-0 text-base-content/60">{{ workspacePermissionText }}</span>
      <span class="h-4 w-px shrink-0 bg-base-300"></span>
      <span class="truncate">{{ workspaceButtonName || workspaceButtonLabel }}</span>
    </button>
  </Transition>

  <!-- 自动推送：状态标签，不是操作 -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition duration-200 ease-out"
    leave-to-class="opacity-0 translate-y-1"
  >
    <span
      v-if="autoPushActive"
      class="inline-flex h-8 shrink-0 items-center rounded-full border border-info/25 bg-info/15 px-2 text-xs font-medium text-info"
      :title="autoPushTitle"
    >
      {{ autoPushLabel }}
    </span>
  </Transition>

  <!-- 运行监控：纯文字；有运行中才出现，没有就整颗不渲染 -->
  <Transition
    enter-active-class="transition duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1"
    leave-active-class="transition duration-200 ease-out"
    leave-to-class="opacity-0 translate-y-1"
  >
    <button
      v-if="hasMonitor"
      type="button"
      :class="[SESSION_FLOAT_FROST_PILL, 'max-w-full']"
      :title="delegateTitle"
      @click="emit('openDelegateSummary')"
    >
      <span v-if="activeKindCount >= 2" class="truncate">{{ monitorSummaryText }}</span>
      <template v-else>
        <span class="shrink-0 font-semibold tabular-nums">{{ monitorPrimaryText }}</span>
        <template v-if="delegateRunningCount > 0">
          <span class="h-4 w-px shrink-0 bg-base-300"></span>
          <span class="flex min-w-0 items-center gap-2 overflow-hidden text-base-content/75">
            <span class="truncate tabular-nums">{{ elapsedText }}</span>
            <span class="truncate tabular-nums">{{ t("chat.monitorBar.requestCountLabel", { count: requestCount }) }}</span>
            <span class="truncate tabular-nums">{{ t("chat.monitorBar.tokenCountLabel", { value: tokenText }) }}</span>
          </span>
        </template>
      </template>
    </button>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ConversationDelegateStatusSummary, ShellWorkMode } from "../../../types/app";
import { SESSION_FLOAT_FROST_PILL } from "./session-float-styles";

defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  showWorkspaceButton?: boolean;
  workspaceButtonDisabled?: boolean;
  workspaceWorkMode?: ShellWorkMode;
  workspacePermissionKind?: "approval" | "full_access" | "autonomous";
  autoPushActive?: boolean;
  delegates: ConversationDelegateStatusSummary[];
  runningTaskCount?: number;
  runningShellCount?: number;
}>();

const emit = defineEmits<{
  lockWorkspace: [];
  openDelegateSummary: [];
}>();

const { t } = useI18n();

const normalizedDelegates = computed(() => Array.isArray(props.delegates) ? props.delegates : []);
const runningDelegates = computed(() => normalizedDelegates.value.filter(isDelegateRunning));
const displayedDelegates = computed(() => runningDelegates.value.length > 0 ? runningDelegates.value : normalizedDelegates.value);
const delegateCount = computed(() => displayedDelegates.value.length);
const delegateRunningCount = computed(() => runningDelegates.value.length);
const elapsedMs = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.elapsedMs));
const requestCount = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.requestCount));
const tokenCount = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.tokenCount));
const elapsedText = computed(() => formatElapsedMs(elapsedMs.value));
const tokenText = computed(() => formatTokenK(tokenCount.value));
const workspaceTitle = computed(() => {
  if (!props.workspaceButtonName) return props.workspaceButtonLabel;
  return `${workspaceModeText.value} · ${workspacePermissionText.value} · ${props.workspaceButtonName}`;
});
const workspaceModeText = computed(() =>
  props.workspaceWorkMode === "worktree"
    ? t("chat.workspaceStatusModeWorktree")
    : t("chat.workspaceStatusModeDirectory"),
);
const workspacePermissionText = computed(() => {
  if (props.workspacePermissionKind === "autonomous") return t("chat.workspaceStatusPermissionAutonomous");
  if (props.workspacePermissionKind === "full_access") return t("chat.workspaceStatusPermissionFull");
  return t("chat.workspaceStatusPermissionApproval");
});
const autoPushLabel = computed(() => t("chat.autoPush.activeChip"));
const autoPushTitle = computed(() => t("chat.autoPush.activeHint"));
const taskActiveCount = computed(() => Math.max(0, Number(props.runningTaskCount ?? 0)));
const shellActiveCount = computed(() => Math.max(0, Number(props.runningShellCount ?? 0)));
const hasMonitor = computed(() => delegateRunningCount.value > 0 || taskActiveCount.value > 0 || shellActiveCount.value > 0);
const activeKindCount = computed(() => [delegateRunningCount.value > 0, taskActiveCount.value > 0, shellActiveCount.value > 0].filter(Boolean).length);
const monitorSummaryText = computed(() => {
  const parts: string[] = [];
  if (delegateRunningCount.value > 0) parts.push(t("chat.monitorBar.delegateCount", { count: delegateRunningCount.value }));
  if (taskActiveCount.value > 0) parts.push(t("chat.monitorBar.taskCount", { count: taskActiveCount.value }));
  if (shellActiveCount.value > 0) parts.push(t("chat.monitorBar.shellCount", { count: shellActiveCount.value }));
  return parts.join("、");
});
// 单一类型在跑时出「计数 + 细节」，两种以上只出摘要
const monitorPrimaryText = computed(() => {
  if (delegateRunningCount.value > 0) return t("chat.monitorBar.delegateCount", { count: delegateCount.value });
  if (taskActiveCount.value > 0) return t("chat.monitorBar.taskCount", { count: taskActiveCount.value });
  return t("chat.monitorBar.shellCount", { count: shellActiveCount.value });
});
const delegateTitle = computed(() => t("chat.monitorBar.viewRunningTitle", { summary: monitorSummaryText.value }));

function sumBy(
  delegates: ConversationDelegateStatusSummary[],
  read: (delegate: ConversationDelegateStatusSummary) => number | undefined | null,
) {
  return delegates.reduce((sum, delegate) => {
    const value = Number(read(delegate) ?? 0);
    return sum + (Number.isFinite(value) && value > 0 ? value : 0);
  }, 0);
}

function isDelegateRunning(delegate: ConversationDelegateStatusSummary) {
  const status = String(delegate.status || "").trim();
  return delegate.active && (status === "running" || status === "delivered");
}

function formatTokenK(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0K";
  const k = value / 1000;
  if (k < 10) return `${k.toFixed(1)}K`;
  return `${Math.round(k)}K`;
}

function formatElapsedMs(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0秒";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}时${minutes}分`;
  if (minutes > 0) return `${minutes}分${seconds}秒`;
  return `${seconds}秒`;
}
</script>
