<template>
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-sm font-medium">画布宽度</span>
      <div class="join">
        <button
          v-for="preset in widthPresets"
          :key="preset.key"
          type="button"
          class="join-item btn btn-sm"
          :class="activeWidth === preset.key ? 'btn-active' : ''"
          @click="activeWidth = preset.key"
        >
          {{ preset.label }}
        </button>
      </div>
      <span class="text-xs text-base-content/50">{{ currentPreset.width }}px</span>
      <span class="mx-1 h-4 w-px bg-base-300"></span>
      <label class="flex cursor-pointer items-center gap-1 text-xs">
        <input v-model="showEmptyState" type="checkbox" class="checkbox checkbox-xs" />
        空态看板
      </label>
    </div>
    <div class="ecall-gallery-wall" :style="{ width: currentPreset.width + 'px' }">
      <ChatHomePanel v-bind="activeWallCards" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import type { BackgroundShellTaskSummary, ConversationDelegateStatusSummary } from "../../../types/app";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import type { ToolReviewBatchSummary } from "../composables/use-chat-tool-review";
import ChatHomePanel from "./ChatHomePanel.vue";

const MOCK_WORKSPACE = "E:/github/easy_call_ai";

const MOCK_GIT = {
  workspaceRootPath: MOCK_WORKSPACE,
  branch: "main",
  gitChanges: [
    { path: "src/features/chat/components/ChatHomePanel.vue", status: "M" },
    { path: "src/features/chat/components/chat-home/CardShell.vue", status: "M" },
    { path: "src/features/chat/components/chat-home/HomeWorkspaceCard.vue", status: "A" },
    { path: "src/locales/zh-TW.json", status: "M" },
    { path: "src/features/config/views/config-tabs/OldDemoTab.vue", status: "D" },
    { path: "src/features/chat/views/ChatView.vue", status: "R" },
  ],
  changeCount: 9,
};

const MOCK_FILES = {
  openFiles: [
    { path: `${MOCK_WORKSPACE}/src/features/chat/views/ChatView.vue`, label: "ChatView.vue" },
    { path: `${MOCK_WORKSPACE}/src/features/chat/components/ChatHomePanel.vue`, label: "ChatHomePanel.vue" },
    { path: `${MOCK_WORKSPACE}/src/features/chat/components/chat-home/CardShell.vue`, label: "CardShell.vue" },
    { path: `${MOCK_WORKSPACE}/src/locales/zh-CN.json`, label: "zh-CN.json" },
    { path: `${MOCK_WORKSPACE}/src/features/chat/composables/chat-ui-layout-storage.ts`, label: "chat-ui-layout-storage.ts" },
  ],
  activePath: `${MOCK_WORKSPACE}/src/features/chat/components/ChatHomePanel.vue`,
  openFileCount: 6,
};

const MOCK_PLAN = {
  latestPlan: {
    path: `${MOCK_WORKSPACE}/.pai/plan/chat/20260911_预览幕墙计划卡.md`,
    markdownContent: `# 侧边预览幕墙计划卡开发计划

## 核心任务
- [x] 解析计划 Markdown 结构
- [x] 设计并构建 HomePlanCard 组件
- [ ] 接入 ChatView 与 ChatHomePanel
- [ ] 单元测试与端到端验证
- [ ] 完善国际化与暗黑模式适配`,
  },
};

const MOCK_SHELLS = {
  shells: [makeShell(0, "ready in 1243 ms\nLocal: http://localhost:1420/")],
};

const MOCK_SIDE_CHAT_ENABLED = { sideChatEnabled: true };

// 第 2 条 itemCount 为 0：用来确认「没有工具调用的轮次」不会进卡片。
// 卡片只看最后一条（最新一轮）——它改了 6 个文件，所以头部是「修改了 6 个文件」，卡内 4 行文件 + 「还有 2 个文件被修改」。
const MOCK_TOOL_BATCHES = {
  toolBatches: [
    makeBatch(1, 6, 4, 96, 128, [
      `${MOCK_WORKSPACE}/src/features/chat/components/ChatHomePanel.vue`,
      `${MOCK_WORKSPACE}/src/features/chat/components/chat-home/CardShell.vue`,
      `${MOCK_WORKSPACE}/src/features/chat/composables/use-chat-tool-review.ts`,
      `${MOCK_WORKSPACE}/src/locales/zh-CN.json`,
    ]),
    makeBatch(2, 0, 0, 0, 0, []),
    makeBatch(3, 3, 2, 41, 6, [
      `${MOCK_WORKSPACE}/src/features/config/views/config-tabs/DemoTab.vue`,
      `${MOCK_WORKSPACE}/src/features/chat/components/HomeCardGalleryDemo.vue`,
    ]),
    makeBatch(4, 9, 1, 18, 3, [`${MOCK_WORKSPACE}/src/features/shell/components/AppWindowContent.vue`]),
    makeBatch(5, 2, 6, 96, 12, [
      `${MOCK_WORKSPACE}/src/features/chat/components/chat-home/HomeToolCard.vue`,
      `${MOCK_WORKSPACE}/src/features/chat/components/ChatHomePanel.vue`,
      `${MOCK_WORKSPACE}/src/features/chat/views/ChatView.vue`,
      `${MOCK_WORKSPACE}/src/locales/zh-CN.json`,
      `${MOCK_WORKSPACE}/src/locales/zh-TW.json`,
      `${MOCK_WORKSPACE}/src/locales/en-US.json`,
    ]),
  ],
};

function makeBatch(
  index: number,
  itemCount: number,
  changedFiles: number,
  addedLines: number,
  deletedLines: number,
  affectedPaths: string[],
): ToolReviewBatchSummary {
  return {
    batchKey: `demo-batch-${index}`,
    userMessageId: `demo-message-${index}`,
    userMessageText: `演示用第 ${index} 轮用户消息`,
    itemCount,
    unreviewedCount: itemCount,
    changedFiles,
    addedLines,
    deletedLines,
    items: affectedPaths.map((path, pathIndex) => ({
      callId: `demo-call-${index}-${pathIndex}`,
      toolName: "apply_patch",
      orderIndex: pathIndex,
      hasReview: false,
      affectedPaths: [path],
      patchOperation: "update",
      isSuccess: true,
      addedLines: 6 + pathIndex * 11,
      deletedLines: 2 + pathIndex * 5,
    })),
  };
}

function makeShell(index: number, outputTail: string): BackgroundShellTaskSummary {
  return {
    id: `demo-shell-${index}`,
    kind: "shell",
    status: "running",
    exitCode: null,
    description: "后台启动 vite dev server 用于本地视觉自检",
    command: "pnpm dev",
    cwd: MOCK_WORKSPACE,
    startedAt: new Date(Date.now() - 154000).toISOString(),
    timeoutMs: null,
    log: "C:/temp/bg-shell-demo.log",
    outputTail,
  };
}

function makeDelegate(index: number, status: string, active = true): ConversationDelegateStatusSummary {
  return {
    delegateId: `demo-delegate-${index}`,
    kind: "delegate",
    conversationId: "demo",
    rootConversationId: "demo",
    title: "排查后台任务页加载失败并补齐回归测试",
    status,
    active,
    startedAt: new Date(Date.now() - 62000).toISOString(),
    updatedAt: new Date(Date.now() - index * 60000).toISOString(),
    elapsedMs: 62000,
    requestCount: 12,
    toolCallCount: 34,
    lastToolName: "exec",
    tokenCount: 29000,
  };
}

function makeTask(index: number): TaskEntry {
  return {
    taskId: `demo-task-${index}`,
    orderIndex: index,
    goal: "每周整理一次 changelog 未发布条目",
    why: "保持发布流程可追溯",
    todo: "检查清单并逐项归类",
    completionState: "active",
    completionConclusion: "",
    progressNotes: [],
    trigger: { next_run_at: new Date(Date.now() + 125 * 60000).toISOString() },
    createdAtLocal: new Date().toISOString(),
    updatedAtLocal: new Date().toISOString(),
  };
}

const widthPresets = [
  { key: "sidebar", label: "侧栏窄屏", width: 300 },
  { key: "phone", label: "手机", width: 390 },
  { key: "tablet", label: "平板", width: 768 },
  { key: "pc", label: "PC", width: 1200 },
];
const activeWidth = ref("pc");
const showEmptyState = ref(false);
const currentPreset = computed(
  () => widthPresets.find((preset) => preset.key === activeWidth.value) || widthPresets[0],
);

const wallCards = {
  ...MOCK_GIT,
  ...MOCK_PLAN,
  ...MOCK_FILES,
  ...MOCK_SIDE_CHAT_ENABLED,
  sideChats: [
    { id: "demo-side-1", title: "压缩重开后收尾标识为什么会对不上？" },
    { id: "demo-side-2", title: "右侧主页网格的列宽是怎么算出来的" },
    { id: "demo-side-3", title: "日志窗滚动条为什么改成常驻" },
  ],
  shells: MOCK_SHELLS.shells,
  delegates: [makeDelegate(0, "running"), makeDelegate(1, "delivered"), makeDelegate(2, "completed", false)],
  runningTasks: [makeTask(0), makeTask(1)],
  toolBatches: MOCK_TOOL_BATCHES.toolBatches,
};

const activeWallCards = computed(() => (showEmptyState.value ? {} : wallCards));
</script>

<style scoped>
.ecall-gallery-wall {
  max-width: 100%;
  height: 1000px;
  overflow: hidden;
  border-radius: 0.75rem;
  border: 1px solid var(--color-base-300);
  background-color: var(--color-base-200);
}
</style>
