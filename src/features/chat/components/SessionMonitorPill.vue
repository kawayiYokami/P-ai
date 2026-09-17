<template>
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
      :class="[SESSION_GHOST_PILL, 'max-w-full gap-1.5']"
      :title="delegateTitle"
      @click="emit('openRunSummary')"
    >
      <!-- 活动脉冲指示灯 -->
      <span class="relative flex h-2 w-2 shrink-0 items-center justify-center">
        <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary/60 opacity-75"></span>
        <span class="relative inline-flex h-1.5 w-1.5 rounded-full bg-primary"></span>
      </span>

      <span v-if="activeKindCount >= 2" class="truncate font-medium">{{ monitorSummaryText }}</span>
      <template v-else>
        <span class="shrink-0 font-semibold tabular-nums">{{ monitorPrimaryText }}</span>
        <template v-if="delegateRunningCount > 0">
          <span class="h-4 w-px shrink-0 bg-base-300"></span>
          <span class="flex min-w-0 items-center gap-2 overflow-hidden text-base-content/75">
            <span class="truncate tabular-nums">{{ elapsedText }}</span>
            <span class="truncate tabular-nums">{{ t("chat.monitorBar.requestCountLabel", { count: requestCount }) }}</span>
          </span>
        </template>
      </template>
    </button>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ConversationDelegateStatusSummary } from "../../../types/app";
import { SESSION_GHOST_PILL } from "./session-float-styles";

const props = defineProps<{
  delegates: ConversationDelegateStatusSummary[];
  runningTaskCount?: number;
  runningShellCount?: number;
}>();

const emit = defineEmits<{
  openRunSummary: [];
}>();

const { t } = useI18n();

const normalizedDelegates = computed(() => Array.isArray(props.delegates) ? props.delegates : []);
const runningDelegates = computed(() => normalizedDelegates.value.filter(isDelegateRunning));
const displayedDelegates = computed(() => runningDelegates.value.length > 0 ? runningDelegates.value : normalizedDelegates.value);
const delegateCount = computed(() => displayedDelegates.value.length);
const delegateRunningCount = computed(() => runningDelegates.value.length);
const elapsedMs = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.elapsedMs));
const requestCount = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.requestCount));
const elapsedText = computed(() => formatElapsedMs(elapsedMs.value));
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
