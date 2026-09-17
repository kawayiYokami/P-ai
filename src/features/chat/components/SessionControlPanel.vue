<template>
  <div class="flex min-w-0 items-center gap-0.5 overflow-hidden">
    <button
      type="button"
      class="btn btn-ghost btn-sm h-8 min-h-8 min-w-0 gap-1.5 overflow-hidden px-2 transition-[max-width,background-color,color] duration-200 ease-out"
      :class="expandedPanel === 'workspace' ? 'w-auto max-w-[min(28rem,56vw)] flex-none justify-start bg-base-200/80' : 'w-8 max-w-8 flex-none justify-center'"
      :disabled="workspaceButtonDisabled"
      :title="workspaceTitle"
      @click="handleWorkspaceClick"
    >
      <component :is="workspacePermissionIcon" class="size-3.5 shrink-0" aria-hidden="true" />
      <Transition name="wdc-content">
        <span v-if="expandedPanel === 'workspace'" class="truncate text-xs">
          {{ workspaceButtonName || workspaceButtonLabel }}
        </span>
      </Transition>
    </button>

    <Transition name="wdc-content">
      <span
        v-if="autoPushActive"
        class="inline-flex h-8 flex-none items-center rounded-full bg-info/15 px-2 text-xs font-medium text-info"
        :title="autoPushTitle"
      >
        {{ autoPushLabel }}
      </span>
    </Transition>

    <div class="ml-auto flex min-w-0 flex-none items-center gap-0.5">
      <button
        type="button"
        class="btn btn-ghost btn-sm h-8 min-h-8 flex-none gap-1.5 overflow-hidden px-2 transition-[max-width,background-color,color] duration-200 ease-out"
        :class="expandedPanel === 'delegate' ? 'w-auto max-w-[min(21rem,52vw)] justify-start bg-base-200/80' : 'w-8 max-w-8 justify-center'"
        :disabled="!hasMonitor"
        :title="delegateTitle"
        @click="handleDelegateClick"
      >
        <span class="indicator shrink-0">
          <span
            v-if="hasMonitor"
            class="indicator-item indicator-top indicator-end h-2.5 w-2.5 rounded-full bg-success"
          ></span>
          <Network class="size-3.5 shrink-0" :class="hasMonitor ? 'text-base-content/70' : 'text-base-content/40'" aria-hidden="true" />
        </span>

        <Transition name="wdc-content">
          <span v-if="expandedPanel === 'delegate'" class="flex min-w-0 items-center gap-1.5 overflow-hidden">
            <template v-if="activeKindCount >= 2">
              <span class="truncate text-xs text-base-content/75">{{ monitorSummaryText }}</span>
            </template>
            <template v-else-if="taskActiveCount > 0">
              <span class="shrink-0 text-xs font-semibold tabular-nums">{{ t("chat.monitorBar.taskCount", { count: taskActiveCount }) }}</span>
              <span class="h-4 w-px shrink-0 bg-base-300"></span>
              <span class="text-xs text-base-content/75">{{ t("chat.monitorBar.runningSuffix") }}</span>
            </template>
            <template v-else-if="shellActiveCount > 0">
              <span class="shrink-0 text-xs font-semibold tabular-nums">{{ t("chat.monitorBar.shellCount", { count: shellActiveCount }) }}</span>
              <span class="h-4 w-px shrink-0 bg-base-300"></span>
              <span class="text-xs text-base-content/75">{{ t("chat.monitorBar.runningSuffix") }}</span>
            </template>
            <template v-else>
              <span class="shrink-0 text-xs font-semibold tabular-nums">{{ t("chat.monitorBar.delegateCount", { count: delegateCount }) }}</span>
              <span class="h-4 w-px shrink-0 bg-base-300"></span>
              <span class="flex min-w-0 items-center gap-2 overflow-hidden text-xs text-base-content/75">
                <span class="inline-flex min-w-0 items-center gap-1" :title="t('chat.monitorBar.elapsedTitle')">
                  <Timer class="size-3.5 shrink-0 text-base-content/45" aria-hidden="true" />
                  <span class="truncate tabular-nums">{{ elapsedText }}</span>
                </span>
                <span class="inline-flex min-w-0 items-center gap-1" :title="t('chat.monitorBar.requestTitle')">
                  <Footprints class="size-3.5 shrink-0 text-base-content/45" aria-hidden="true" />
                  <span class="truncate tabular-nums">{{ t("chat.monitorBar.requestCountLabel", { count: requestCount }) }}</span>
                </span>
                <span class="inline-flex min-w-0 items-center gap-1" :title="t('chat.monitorBar.tokenTitle')">
                  <Coins class="size-3.5 shrink-0 text-base-content/45" aria-hidden="true" />
                  <span class="truncate tabular-nums">{{ t("chat.monitorBar.tokenCountLabel", { value: tokenText }) }}</span>
                </span>
              </span>
            </template>
          </span>
        </Transition>
      </button>

      <Transition name="wdc-content">
        <PanelRightOpen
          v-if="expandedPanel === 'delegate' && delegateCount > 0"
          class="size-3.5 flex-none text-base-content/45"
          aria-hidden="true"
        />
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Coins, Eye, Footprints, Network, PanelRightOpen, ShieldCheck, ShieldOff, ShieldQuestion, Timer } from "@lucide/vue";
import type { ConversationDelegateStatusSummary } from "../../../types/app";

const props = defineProps<{
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  workspaceButtonDisabled?: boolean;
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
const expandedPanel = ref<"workspace" | "delegate">("workspace");
const normalizedDelegates = computed(() => Array.isArray(props.delegates) ? props.delegates : []);
const runningDelegates = computed(() => normalizedDelegates.value.filter(isDelegateRunning));
const displayedDelegates = computed(() => runningDelegates.value.length > 0 ? runningDelegates.value : normalizedDelegates.value);
const delegateCount = computed(() => displayedDelegates.value.length);
const delegateRunningCount = computed(() => runningDelegates.value.length);
const runningCount = computed(() => runningDelegates.value.length);
const elapsedMs = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.elapsedMs));
const requestCount = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.requestCount));
const tokenCount = computed(() => sumBy(displayedDelegates.value, (delegate) => delegate.tokenCount));
const elapsedText = computed(() => formatElapsedMs(elapsedMs.value));
const tokenText = computed(() => formatTokenK(tokenCount.value));
const workspaceTitle = computed(() => props.workspaceButtonName || props.workspaceButtonLabel);
const workspacePermissionIcon = computed(() => {
  if (props.workspacePermissionKind === "autonomous") return ShieldOff;
  if (props.workspacePermissionKind === "full_access") return ShieldCheck;
  if (props.workspacePermissionKind === "approval") return ShieldQuestion;
  return Eye;
});
const autoPushLabel = computed(() => t("chat.autoPush.activeChip"));
const autoPushTitle = computed(() => t("chat.autoPush.activeHint"));
const delegateTitle = computed(() => {
  if (!hasMonitor.value) return t("chat.monitorBar.emptyTitle");
  return t("chat.monitorBar.viewRunningTitle", { summary: monitorSummaryText.value });
});
const taskActiveCount = computed(() => Math.max(0, Number(props.runningTaskCount ?? 0)));
const shellActiveCount = computed(() => Math.max(0, Number(props.runningShellCount ?? 0)));
const hasMonitor = computed(() => delegateRunningCount.value > 0 || taskActiveCount.value > 0 || shellActiveCount.value > 0);
const activeKindCount = computed(() => [delegateRunningCount.value > 0, taskActiveCount.value > 0, shellActiveCount.value > 0].filter(Boolean).length);
const monitorSummaryText = computed(() => {
  const parts: string[] = [];
  if (delegateRunningCount.value > 0) parts.push(t("chat.monitorBar.delegateCount", { count: delegateRunningCount.value }));
  if (taskActiveCount.value > 0) parts.push(t("chat.monitorBar.taskCount", { count: taskActiveCount.value }));
  if (shellActiveCount.value > 0) parts.push(t("chat.monitorBar.shellCount", { count: shellActiveCount.value }));
  return `${parts.join("、")} ${t("chat.monitorBar.runningSuffix")}`;
});

watch(
  [runningCount, taskActiveCount, shellActiveCount],
  ([delegateRunning, tasks, shells], [prevDelegate, prevTasks, prevShells]) => {
    const total = delegateRunning + tasks + shells;
    const previousTotal = (prevDelegate ?? 0) + (prevTasks ?? 0) + (prevShells ?? 0);
    if (total > 0 && previousTotal <= 0) {
      expandedPanel.value = "delegate";
      return;
    }
    if (total <= 0) {
      expandedPanel.value = "workspace";
    }
  },
  { immediate: true },
);

function handleWorkspaceClick() {
  if (expandedPanel.value !== "workspace") {
    expandedPanel.value = "workspace";
    return;
  }
  emit("lockWorkspace");
}

function handleDelegateClick() {
  if (!hasMonitor.value) return;
  if (expandedPanel.value !== "delegate") {
    expandedPanel.value = "delegate";
    return;
  }
  emit("openDelegateSummary");
}

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

<style scoped>
.wdc-content-enter-active,
.wdc-content-leave-active {
  transition: opacity 120ms ease, transform 120ms ease;
}

.wdc-content-enter-from,
.wdc-content-leave-to {
  opacity: 0;
  transform: translateX(-4px);
}
</style>
