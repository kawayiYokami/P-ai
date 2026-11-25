<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <ul v-if="hasAnyWork" class="menu bg-base-200 w-full">
      <li v-if="runningDelegates.length > 0">
        <a>{{ t("chat.toolReview.overviewDelegates") }} {{ runningDelegates.length }}</a>
        <ul>
          <li v-for="delegate in runningDelegates" :key="delegate.delegateId">
            <a class="flex flex-col items-start gap-0.5" :title="delegate.title || delegate.delegateId" @click="emit('switchPanelTab', 'delegate')">
              <span class="block min-w-0 truncate">{{ delegate.title || delegate.delegateId }}</span>
              <DelegateProgressLine
                class="min-w-0 truncate"
                :running="true"
                :elapsed-ms="delegate.elapsedMs"
                :request-count="delegate.requestCount"
                :token-count="delegate.tokenCount"
                :last-tool-name="delegate.lastToolName"
              />
            </a>
          </li>
        </ul>
      </li>
      <li v-if="runningTimers.length > 0">
        <a>{{ t("chat.toolReview.overviewTimers") }} {{ runningTimers.length }}</a>
        <ul>
          <li v-for="timer in runningTimers" :key="timer.id">
            <a class="flex flex-col items-start gap-0.5" :title="timerTitle(timer)" @click="emit('switchPanelTab', 'timers')">
              <span class="block min-w-0 truncate">{{ timerTitle(timer) }}</span>
              <span v-if="timerNextRunText(timer)" class="text-xs text-base-content/65">{{ timerNextRunText(timer) }}</span>
            </a>
          </li>
        </ul>
      </li>
      <li v-if="latestBatches.length > 0">
        <a>{{ t("chat.toolReview.overviewLatestChanges") }} {{ latestBatches.length }}</a>
        <ul>
          <li v-for="batch in latestBatches" :key="batch.batchKey">
            <a class="flex flex-col items-start gap-0.5" :title="batch.userMessageText" @click="emit('switchPanelTab', 'tools')">
              <span class="block min-w-0 truncate">{{ batch.userMessageText }}</span>
              <span class="text-xs text-base-content/65">{{ batchDiffText(batch) }}</span>
            </a>
          </li>
        </ul>
      </li>
      <li v-if="runningBackgroundShells.length > 0">
        <a>{{ t("chat.toolReview.overviewBackgroundShells") }} {{ runningBackgroundShells.length }}</a>
        <ul>
          <li v-for="task in runningBackgroundShells" :key="task.id">
            <a class="flex flex-col items-start gap-0.5" :title="task.description">
              <span class="block min-w-0 truncate">{{ task.description }}</span>
              <span class="text-xs text-base-content/65">{{ shellElapsedText(task.startedAt) }}</span>
            </a>
          </li>
        </ul>
      </li>
    </ul>
    <div v-else class="flex flex-1 items-center justify-center p-8 text-center">{{ t("chat.toolReview.overviewEmptyAll") }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { BackgroundShellTaskSummary, ConversationDelegateStatusSummary } from "../../../types/app";
import type { ToolReviewBatchSummary } from "../composables/use-chat-tool-review";
import { formatTimerTime, type TimerEntry } from "../utils/timer-entry";
import type { ChatMonitorPanelMode } from "../composables/chat-ui-layout-storage";
import DelegateProgressLine from "./DelegateProgressLine.vue";

const props = defineProps<{
  delegateStatuses: ConversationDelegateStatusSummary[];
  runningTimers: TimerEntry[];
  backgroundShells: BackgroundShellTaskSummary[];
  latestBatches: ToolReviewBatchSummary[];
}>();

const emit = defineEmits<{
  (e: "switchPanelTab", tab: ChatMonitorPanelMode): void;
}>();

const { t } = useI18n();

const runningDelegates = computed(() =>
  props.delegateStatuses.filter((delegate) => {
    const status = String(delegate.status || "").trim();
    return delegate.active && (status === "running" || status === "delivered");
  }),
);

const runningBackgroundShells = computed(() =>
  props.backgroundShells.filter((task) => String(task.status || "").trim() === "running"),
);

const hasAnyWork = computed(
  () =>
    runningDelegates.value.length > 0 ||
    props.runningTimers.length > 0 ||
    props.latestBatches.length > 0 ||
    runningBackgroundShells.value.length > 0,
);

function batchDiffText(batch: ToolReviewBatchSummary) {
  const files = Number(batch.changedFiles) || 0;
  const added = Number(batch.addedLines) || 0;
  const deleted = Number(batch.deletedLines) || 0;
  if (files <= 0 && added <= 0 && deleted <= 0) return "";
  const parts = [];
  if (files > 0) parts.push(t("chat.toolReview.overviewBatchFiles", { n: files }));
  if (added > 0 || deleted > 0) parts.push(`+${added} -${deleted}`);
  return parts.join(" · ");
}

// 后台终端时长不逐秒推送，用组件内时钟驱动显示；无倒计面条目时不空转
const countdownClockNowMs = ref(Date.now());
let countdownClockTimer: ReturnType<typeof window.setInterval> | null = null;

const hasCountdownWork = computed(() => runningBackgroundShells.value.length > 0);

watch(
  hasCountdownWork,
  (shouldTick) => {
    if (shouldTick && countdownClockTimer == null) {
      countdownClockTimer = window.setInterval(() => {
        countdownClockNowMs.value = Date.now();
      }, 1000);
    } else if (!shouldTick && countdownClockTimer != null) {
      window.clearInterval(countdownClockTimer);
      countdownClockTimer = null;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (countdownClockTimer != null) {
    window.clearInterval(countdownClockTimer);
    countdownClockTimer = null;
  }
});

function shellElapsedText(startedAt: string) {
  const startedMs = Date.parse(String(startedAt || ""));
  if (!Number.isFinite(startedMs) || startedMs <= 0) return "";
  return formatDurationMs(countdownClockNowMs.value - startedMs);
}

function timerNextRunText(timer: TimerEntry) {
  return formatTimerTime(timer.nextRunAt);
}

function formatDurationMs(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0秒";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}时${minutes}分`;
  if (minutes > 0) return `${minutes}分${seconds}秒`;
  return `${seconds}秒`;
}

function timerTitle(timer: TimerEntry) {
  return String(timer.message || "").trim() || t("config.timer.untitled");
}
</script>
