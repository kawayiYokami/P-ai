<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs text-base-content/60">预设：</span>
      <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'normal' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'normal'">仅委托</button>
      <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'tasksOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'tasksOnly'">仅任务</button>
      <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'shellsOnly' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'shellsOnly'">仅终端</button>
      <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'all' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'all'">全部运行</button>
      <button type="button" class="btn btn-xs" :class="demoMonitorPreset === 'empty' ? 'btn-primary' : 'btn-ghost'" @click="demoMonitorPreset = 'empty'">空闲</button>
    </div>
    <div class="flex w-full max-w-2xl flex-wrap items-center gap-2">
      <SessionControlItems
        workspace-button-label="工作空间"
        workspace-button-name="easy_call_ai"
        workspace-work-mode="worktree"
        :delegates="demoDelegateStatuses"
        :running-task-count="demoRunningTaskCount"
        :running-shell-count="demoRunningShellCount"
      />
    </div>
    <div class="flex flex-col gap-1 py-2">
      <DelegateCard
        title="示例：代码审查（pending）"
        :running="true"
        :elapsed-ms="demoDelegateStatuses[0]?.elapsedMs"
        :request-count="demoDelegateStatuses[0]?.requestCount"
        :token-count="demoDelegateStatuses[0]?.tokenCount"
        last-tool-name="apply_patch"
      />
      <DelegateCard
        title="示例：委托任务（运行中）"
        :running="true"
        :elapsed-ms="demoDelegateStatuses[1]?.elapsedMs"
        :request-count="demoDelegateStatuses[1]?.requestCount"
        :token-count="demoDelegateStatuses[1]?.tokenCount"
        last-tool-name="shell_exec"
      />
      <DelegateCard
        title="示例：审查报告（完成）"
        text="整体判定：正确，置信度 0.92"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import SessionControlItems from "../../chat/components/SessionControlItems.vue";
import DelegateCard from "../../chat/components/DelegateCard.vue";
import type { ConversationDelegateStatusSummary } from "../../../types/app";

const demoDelegateStatuses = ref<ConversationDelegateStatusSummary[]>([
  createDemoDelegateStatus("demo-code-review", "示例：代码审查（pending）", 45000, 12, 15600, "apply_patch"),
  createDemoDelegateStatus("demo-research", "示例：委托任务（运行中）", 120000, 34, 52800, "shell_exec"),
  {
    ...createDemoDelegateStatus("demo-report", "示例：审查报告（完成）", 347000, 18, 23600, ""),
    status: "completed",
    active: false,
    completedAt: new Date().toISOString(),
  },
]);
let delegateDemoTimer = 0;
const demoMonitorPreset = ref<"normal" | "tasksOnly" | "shellsOnly" | "all" | "empty">("normal");
const demoHasRunningDelegates = computed(() => demoMonitorPreset.value === "normal" || demoMonitorPreset.value === "all");
const demoRunningTaskCount = computed(() => (demoMonitorPreset.value === "tasksOnly" || demoMonitorPreset.value === "all") ? 2 : 0);
const demoRunningShellCount = computed(() => (demoMonitorPreset.value === "shellsOnly" || demoMonitorPreset.value === "all") ? 3 : 0);

function createDemoDelegateStatus(
  delegateId: string,
  title: string,
  elapsedMs: number,
  requestCount: number,
  tokenCount: number,
  lastToolName: string,
): ConversationDelegateStatusSummary {
  const now = new Date().toISOString();
  return {
    delegateId,
    kind: "normal",
    conversationId: `${delegateId}-conversation`,
    rootConversationId: "demo-root-conversation",
    title,
    status: "running",
    active: true,
    startedAt: now,
    updatedAt: now,
    elapsedMs,
    requestCount,
    toolCallCount: requestCount,
    lastToolName,
    tokenCount,
    targetAgentId: "demo-agent",
  };
}

function advanceDemoDelegateStatus() {
  demoDelegateStatuses.value = demoDelegateStatuses.value.map((delegate, index) => {
    if (!delegate.active) return delegate;
    const nextStep = index === 0 ? 1 : 2;
    const nextToken = index === 0 ? 680 : 1340;
    return {
      ...delegate,
      elapsedMs: delegate.elapsedMs + 1000,
      requestCount: delegate.requestCount + nextStep,
      toolCallCount: delegate.toolCallCount + nextStep,
      tokenCount: delegate.tokenCount + nextToken,
      updatedAt: new Date().toISOString(),
    };
  });
}

watch(demoHasRunningDelegates, (nextActive) => {
  demoDelegateStatuses.value = demoDelegateStatuses.value.map((delegate, index) => {
    if (index > 1) return delegate;
    return {
      ...delegate,
      status: nextActive ? "running" : "completed",
      active: nextActive,
      completedAt: nextActive ? undefined : new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
  });
}, { immediate: true });

onMounted(() => {
  delegateDemoTimer = window.setInterval(advanceDemoDelegateStatus, 1000);
});

onBeforeUnmount(() => {
  if (!delegateDemoTimer) return;
  window.clearInterval(delegateDemoTimer);
  delegateDemoTimer = 0;
});
</script>
