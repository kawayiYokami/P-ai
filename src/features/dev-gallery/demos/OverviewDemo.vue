<template>
  <div class="space-y-3">
    <GalleryControls>
      <span class="text-xs text-base-content/60">预设：</span>
      <button type="button" class="btn btn-xs" :class="preset === 'normal' ? 'btn-primary' : 'btn-ghost'" @click="preset = 'normal'">常规</button>
      <button type="button" class="btn btn-xs" :class="preset === 'empty' ? 'btn-primary' : 'btn-ghost'" @click="preset = 'empty'">空闲</button>
      <button type="button" class="btn btn-xs" :class="preset === 'busy' ? 'btn-primary' : 'btn-ghost'" @click="preset = 'busy'">高负载</button>
      <button type="button" class="btn btn-xs" :class="preset === 'backgroundOnly' ? 'btn-primary' : 'btn-ghost'" @click="preset = 'backgroundOnly'">仅后台任务</button>
    </GalleryControls>
    <div class="flex justify-center">
      <div class="h-[440px] w-[400px] overflow-hidden rounded-lg border border-base-300 bg-base-100 shadow-sm">
        <MonitorOverview
          class="h-full w-full"
          :delegate-statuses="delegates"
          :running-timers="timers"
          :background-shells="backgroundShells"
          :latest-batches="batches"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import GalleryControls from "../GalleryControls.vue";
import MonitorOverview from "../../chat/components/MonitorOverview.vue";
import type {
  BackgroundShellTaskSummary,
  ConversationDelegateStatusSummary,
} from "../../../types/app";
import type { ToolReviewBatchSummary } from "../../chat/composables/use-chat-tool-review";
import type { TimerEntry } from "../../chat/utils/timer-entry";

const preset = ref<"empty" | "normal" | "busy" | "backgroundOnly">("normal");

function makeDelegate(index: number, status: string): ConversationDelegateStatusSummary {
  const titles = ["整理会话记录并归档", "排查后台任务页加载失败", "梳理委托状态轮询调用链"];
  const tools = ["exec", "read", "rg"];
  return {
    delegateId: `demo-delegate-${index}`,
    kind: "delegate",
    conversationId: "demo",
    rootConversationId: "demo",
    title: titles[index % titles.length],
    status,
    active: true,
    startedAt: new Date(Date.now() - 62000 * (index + 1)).toISOString(),
    updatedAt: new Date().toISOString(),
    elapsedMs: 62000 * (index + 1),
    requestCount: 4 + index * 8,
    toolCallCount: 9 + index * 25,
    lastToolName: tools[index % tools.length],
    tokenCount: 12800 + index * 16200,
  };
}

function makeTimer(index: number): TimerEntry {
  const messages = [
    "每周整理一次 changelog 未发布条目",
    "补齐后台 shell 终止的回归测试",
    "梳理监控面板 tab 的状态链路",
    "清理未使用的组件导入",
    "核对三语言包缺失的 key",
  ];
  return {
    id: `demo-timer-${index}`,
    message: messages[index % messages.length],
    conversationId: "demo",
    dispatchMode: index % 2 === 0 ? "direct" : "delegate",
    enabled: true,
    runAt: new Date(Date.now() - 60000).toISOString(),
    cron: index % 2 === 0 ? "0 9 * * *" : null,
    endAt: null,
    lastTriggeredAt: null,
    nextRunAt: index < 3
      ? new Date(Date.now() + (index === 0 ? 5 : index === 1 ? 125 : 1490) * 60000).toISOString()
      : null,
    createdAt: new Date().toISOString(),
  };
}

function makeShell(index: number, outputTail: string): BackgroundShellTaskSummary {
  const descriptions = ["前端 dev server 预热", "批量截图回归任务", "Rust 增量编译检查"];
  const commands = ["pnpm dev", "pnpm smoke", "cargo check"];
  return {
    id: `demo-shell-${index}`,
    kind: "shell",
    status: "running",
    exitCode: null,
    description: descriptions[index % descriptions.length],
    command: commands[index % commands.length],
    cwd: "E:/demo/workspace",
    startedAt: new Date(Date.now() - 124000 * (index + 1)).toISOString(),
    timeoutMs: null,
    log: `C:/demo/bg-shell-demo-${index}.log`,
    outputTail,
  };
}

const delegates = computed<ConversationDelegateStatusSummary[]>(() => {
  if (preset.value === "empty" || preset.value === "backgroundOnly") return [];
  if (preset.value === "busy") return [0, 1, 2].map((index) => makeDelegate(index, index === 2 ? "delivered" : "running"));
  return [makeDelegate(0, "running"), makeDelegate(1, "delivered")];
});

const timers = computed<TimerEntry[]>(() => {
  if (preset.value === "empty" || preset.value === "backgroundOnly") return [];
  if (preset.value === "busy") return [0, 1, 2, 3, 4].map((index) => makeTimer(index));
  return [makeTimer(0)];
});

const backgroundShells = computed<BackgroundShellTaskSummary[]>(() => {
  if (preset.value === "empty") return [];
  if (preset.value === "busy") {
    return [
      makeShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/"),
      makeShell(1, "[smoke] 12/24 用例通过，正在执行终端联动用例…"),
      makeShell(2, "Checking the core runtime... 412 crates checked.\nwarning: unused import: `serde::Serialize`\n= note: `#[warn(unused_imports)]` on part of module `commands`"),
    ];
  }
  if (preset.value === "backgroundOnly") {
    return [
      makeShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/"),
      makeShell(1, "3 分钟计时任务剩余 01:42…"),
    ];
  }
  return [makeShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/")];
});

const batches = computed<ToolReviewBatchSummary[]>(() => {
  if (preset.value === "empty" || preset.value === "backgroundOnly") return [];
  if (preset.value === "busy") {
    return [
      {
        batchKey: "demo-batch-busy-1",
        userMessageId: "demo-message-1",
        userMessageText: "重构监控面板并提取概览组件，同步更新三语言包文案",
        itemCount: 14,
        unreviewedCount: 9,
        changedFiles: 6,
        addedLines: 218,
        deletedLines: 34,
        items: [],
      },
      {
        batchKey: "demo-batch-busy-2",
        userMessageId: "demo-message-2",
        userMessageText: "后台任务页接通 monitor.changed 脏标记事件",
        itemCount: 7,
        unreviewedCount: 2,
        changedFiles: 3,
        addedLines: 64,
        deletedLines: 41,
        items: [],
      },
      {
        batchKey: "demo-batch-busy-3",
        userMessageId: "demo-message-3",
        userMessageText: "修复 DeepSeek 协议下思考开关不生效的问题",
        itemCount: 4,
        unreviewedCount: 0,
        changedFiles: 1,
        addedLines: 12,
        deletedLines: 9,
        items: [],
      },
    ];
  }
  return [
    {
      batchKey: "demo-batch",
      userMessageId: "demo-message",
      userMessageText: "修复后台任务页打不开的问题并补充回归测试",
      itemCount: 8,
      unreviewedCount: 3,
      changedFiles: 4,
      addedLines: 96,
      deletedLines: 12,
      items: [],
    },
  ];
});
</script>
