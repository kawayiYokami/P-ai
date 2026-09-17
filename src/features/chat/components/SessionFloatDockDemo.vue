<template>
  <div class="flex flex-col gap-3">
    <!-- 状态开关：直接驱动下排真实组件的 props -->
    <div class="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs">
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showPreviewRow" type="checkbox" class="checkbox checkbox-xs" />
        上排预览条
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showTimelineButton" type="checkbox" class="checkbox checkbox-xs" />
        时间线按钮
      </label>
      <span class="mx-1 h-4 w-px bg-base-300"></span>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showMenu" type="checkbox" class="checkbox checkbox-xs" />
        对话菜单
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showWorkspace" type="checkbox" class="checkbox checkbox-xs" />
        工作区
      </label>
      <label v-if="showWorkspace" class="flex cursor-pointer items-center gap-1">
        <input v-model="longWorkspaceName" type="checkbox" class="checkbox checkbox-xs" />
        长路径名
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showAutoPush" type="checkbox" class="checkbox checkbox-xs" />
        自动推送
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showMonitor" type="checkbox" class="checkbox checkbox-xs" />
        运行监控
      </label>
      <span class="mx-1 h-4 w-px bg-base-300"></span>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="showMentionCandidates" type="checkbox" class="checkbox checkbox-xs" />
        @ 候选
      </label>
      <label v-if="showMentionCandidates" class="flex cursor-pointer items-center gap-1">
        <input v-model="mentionTaskBadge" type="checkbox" class="checkbox checkbox-xs" />
        任务角标
      </label>
      <label class="flex cursor-pointer items-center gap-1">
        <input v-model="mentionBusy" type="checkbox" class="checkbox checkbox-xs" />
        忙碌禁用
      </label>
      <span class="text-base-content/45">（关掉后该元素整颗消失）</span>
      <span class="mx-1 h-4 w-px bg-base-300"></span>
      <label class="flex items-center gap-1">
        模式
        <select v-model="workspaceWorkMode" class="select select-bordered select-xs">
          <option value="directory">目录</option>
          <option value="worktree">工作树</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        权限
        <select v-model="workspacePermission" class="select select-bordered select-xs">
          <option value="approval">审查</option>
          <option value="full_access">全权</option>
          <option value="autonomous">无限制</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        监控
        <select v-model="monitorPreset" class="select select-bordered select-xs">
          <option value="empty">无</option>
          <option value="delegate">仅委派</option>
          <option value="task">仅任务</option>
          <option value="shell">仅 Shell</option>
          <option value="mixed">混合</option>
        </select>
      </label>
      <label v-if="showMentionCandidates" class="flex items-center gap-1">
        人格名
        <select v-model="mentionNamePreset" class="select select-bordered select-xs">
          <option value="normal">常规</option>
          <option value="long">超长</option>
        </select>
      </label>
      <label class="flex items-center gap-1">
        演示宽度
        <select v-model.number="demoWidth" class="select select-bordered select-xs">
          <option :value="618">618 · 聊天窗实际</option>
          <option :value="560">560</option>
          <option :value="480">480</option>
          <option :value="400">400 · 压测</option>
        </select>
      </label>
    </div>

    <!-- 模拟聊天区 + 输入区，悬浮区贴在输入框上沿 -->
    <div
      class="relative flex h-80 flex-col overflow-hidden rounded-box border border-base-300 bg-base-200 transition-[width] duration-200"
      :style="{ width: `min(${demoWidth}px, 100%)` }"
    >
      <div class="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden p-3">
        <div class="h-9 w-2/3 self-start rounded-xl bg-base-100/80"></div>
        <div class="h-14 w-4/5 self-end rounded-xl bg-primary/15"></div>
        <div class="h-9 w-1/2 self-start rounded-xl bg-base-100/80"></div>
      </div>

      <div class="shrink-0 px-2 pb-2">
        <div class="h-12 rounded-box border border-base-300 bg-base-100"></div>
      </div>

      <!-- 会话悬浮操作区 -->
      <div class="absolute inset-x-0" style="bottom: 64px">
        <!-- 上排：预览条（左） + 时间线按钮（右），底边齐平 -->
        <div v-if="showPreviewRow || showTimelineButton" class="flex w-full items-end justify-between gap-2 px-4 pb-2">
          <div class="min-w-0 flex-1">
            <ChatThinkingPreviewBar
              :visible="showPreviewRow"
              :blocks="previewBlocks"
              :streaming="true"
              :idle-text="previewIdleText"
              :lines="2"
            />
          </div>
          <Transition
            enter-active-class="transition duration-200 ease-out"
            enter-from-class="opacity-0 translate-y-1"
            leave-active-class="transition duration-200 ease-out"
            leave-to-class="opacity-0 translate-y-1"
          >
            <button
              v-if="showTimelineButton"
              type="button"
              :class="SESSION_FLOAT_FROST_CIRCLE"
              aria-label="展开时间线"
            >
              <GanttChart class="size-5" />
            </button>
          </Transition>
        </div>

        <!-- 下排：直接挂聊天窗那条工作条真件，开关喂给它不同的 props -->
        <div class="w-full px-4">
          <ChatWorkspaceToolbar
            :chatting="mentionBusy"
            :frozen="false"
            :workspace-button-label="t('chat.allowedWorkspaceButton')"
            :workspace-button-name="workspaceName"
            :workspace-work-mode="workspaceWorkMode"
            :workspace-permission-kind="workspacePermission"
            :auto-push-active="showAutoPush"
            :delegate-statuses="monitorDelegates"
            :running-task-count="monitorCounts.task"
            :running-shell-count="monitorCounts.shell"
            :mention-entries="mentionEntries"
            :selected-mentions="demoSelectedMentions"
            :hide-menu-button="!showMenu"
            :hide-workspace-button="!showWorkspace"
            @add-mention="handleDemoAddMention"
            @remove-mention="handleDemoRemoveMention"
          />
        </div>
      </div>
    </div>

    <div class="text-xs text-base-content/50">
      下排直接挂的就是聊天窗那条工作条真件（<code>ChatWorkspaceToolbar</code>），开关和下拉喂给它不同的 props；上排两个元素是模拟。菜单能点开，项与聊天窗一致，只是这里不接后续动作。
      <br />
      @ 按钮的候选取自输入栏那一份（同一个 <code>mentionEntries</code>）：点条目切换选中，已选中的会带 @ 角标，有后台任务的带转圈角标。聊天窗里它的选中态与输入栏的 @ 面板共享同一份。
      <br />
      每个元素只看自己一个条件：有就出现，没有就不渲染，彼此不互斥、没有展开态。左对齐，一行放不下自动换行。磨砂外观与尺寸定义在 <code>session-float-styles.ts</code>，聊天窗与这里共用同一份。
      <br />
      聊天窗实际尺寸 618×1000，下排去掉左右各 8px 边距后可用宽度约 602px；演示宽度选 618 就是真实情形，往下几档是压测更窄的窗口。
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { GanttChart } from "@lucide/vue";
import type { ChatMentionEntry, ChatMentionTarget, ConversationDelegateStatusSummary, ShellWorkMode } from "../../../types/app";
import ChatThinkingPreviewBar from "./ChatThinkingPreviewBar.vue";
import ChatWorkspaceToolbar from "./ChatWorkspaceToolbar.vue";
import { SESSION_FLOAT_FROST_CIRCLE } from "./session-float-styles";

const { t } = useI18n();

const showPreviewRow = ref(true);
const showTimelineButton = ref(true);
const showMenu = ref(true);
const showWorkspace = ref(true);
const showAutoPush = ref(true);
const showMonitor = ref(true);
const demoWidth = ref(618);
const workspaceWorkMode = ref<ShellWorkMode>("worktree");
const workspacePermission = ref<"approval" | "full_access" | "autonomous">("approval");
const monitorPreset = ref<"empty" | "delegate" | "task" | "shell" | "mixed">("mixed");

const longWorkspaceName = ref(false);
const workspaceName = computed(() =>
  longWorkspaceName.value
    ? "easy_call_ai/src/features/chat/super-long-module-directory"
    : "easy_call_ai",
);

// 上排预览条内容：一段思维链 + 一段正文，够长到能看出宽度表现
const previewBlocks = ref<Array<{ reasoning?: string; text?: string }>>([
  {
    reasoning: "先看第二排现在由哪些控件组成，再决定哪些要拆成独立按钮。\n工作区按钮和监控按钮现在会互相抢展开权。",
    text: "把工作区 bar 拆成一排独立的磨砂元素，左对齐、放不下换行。",
  },
]);
const previewIdleText = "把工作区 bar 拆成一排独立的磨砂元素，左对齐、放不下换行。";

// 委派假数据：口径与 DemoTab 的委派样本一致（运行中 + 一组固定细节数字，不跑计时器）
const demoRunningDelegate: ConversationDelegateStatusSummary = {
  delegateId: "demo-running-delegate",
  kind: "normal",
  conversationId: "demo-conversation",
  rootConversationId: "demo-root-conversation",
  title: "示例：代码审查",
  status: "running",
  active: true,
  startedAt: new Date().toISOString(),
  updatedAt: new Date().toISOString(),
  elapsedMs: 192000,
  requestCount: 18,
  toolCallCount: 24,
  lastToolName: "apply_patch",
  tokenCount: 24600,
};

// 监控计数：监控开关关掉时全部归零，真实组件据此自行决定显不显示
const monitorCounts = computed(() => {
  if (!showMonitor.value) return { delegate: 0, task: 0, shell: 0 };
  switch (monitorPreset.value) {
    case "delegate": return { delegate: 1, task: 0, shell: 0 };
    case "task": return { delegate: 0, task: 2, shell: 0 };
    case "shell": return { delegate: 0, task: 0, shell: 2 };
    case "mixed": return { delegate: 1, task: 1, shell: 1 };
    default: return { delegate: 0, task: 0, shell: 0 };
  }
});
const monitorDelegates = computed(() => (monitorCounts.value.delegate > 0 ? [demoRunningDelegate] : []));

// ========== @ 人格按钮：候选与选中态由这里的开关喂给真实组件 ==========

const showMentionCandidates = ref(true);
const mentionTaskBadge = ref(false);
const mentionBusy = ref(false);
const mentionNamePreset = ref<"normal" | "long">("normal");
const demoSelectedMentions = ref<ChatMentionTarget[]>([]);

const mentionEntries = computed<ChatMentionEntry[]>(() => {
  if (!showMentionCandidates.value) return [];
  const longName = mentionNamePreset.value === "long";
  return [
    {
      agentId: "demo-persona-1",
      agentName: longName ? "超长人格名用来压测面板宽度表现" : "纳西妲",
      mentionable: true,
      isFrontSpeaking: false,
      hasBackgroundTask: false,
    },
    {
      agentId: "demo-persona-2",
      agentName: longName ? "第二个人格名也一样很长用来压测" : "书记官",
      mentionable: true,
      isFrontSpeaking: false,
      hasBackgroundTask: mentionTaskBadge.value,
    },
    {
      agentId: "demo-persona-3",
      agentName: longName ? "第三个人格名同样很长继续压测用" : "reviewer",
      mentionable: true,
      isFrontSpeaking: false,
      hasBackgroundTask: false,
    },
  ];
});

function handleDemoAddMention(entry: ChatMentionTarget) {
  if (demoSelectedMentions.value.some((item) => item.agentId === entry.agentId)) return;
  demoSelectedMentions.value = [...demoSelectedMentions.value, entry];
}

function handleDemoRemoveMention(entry: { agentId: string }) {
  demoSelectedMentions.value = demoSelectedMentions.value.filter((item) => item.agentId !== entry.agentId);
}
</script>
