<template>
  <div class="flex h-full min-h-0 flex-col bg-base-200">
    <div class="flex items-center gap-2 border-b border-base-300 bg-base-100 px-3 py-2 text-xs">
      <button class="btn btn-xs btn-primary" :disabled="phase === 'streaming'" @click="pump">开始流式</button>
      <button class="btn btn-xs" @click="reset">重置</button>
      <div class="join">
        <button
          class="btn btn-xs join-item"
          :class="alignMode === 'own_top' ? 'btn-active btn-neutral' : ''"
          @click="alignMode = 'own_top'"
        >
          对齐顶部 (own_top)
        </button>
        <button
          class="btn btn-xs join-item"
          :class="alignMode === 'follow' ? 'btn-active btn-neutral' : ''"
          @click="alignMode = 'follow'"
        >
          贴底跟随 (follow)
        </button>
      </div>
      <span class="text-base-content/60">{{ statusText }}</span>
    </div>
    <div class="min-h-0 flex-1">
      <ChatView v-bind="demoProps" />
    </div>
  </div>
</template>

<script setup lang="ts">
// 独立 ChatView 流式复现台：真实组件 + 真实 markdown 解析 + 真实滚动区，
// 只有数据源是假的。用来复现「流式时内容跳动」，不碰任何真实会话。
import { computed, onBeforeUnmount, ref } from "vue";
import ChatView from "./ChatView.vue";
import type { ChatMessageBlock } from "../../../types/app";

type ChatViewProps = InstanceType<typeof ChatView>["$props"];

const SOURCE = [
  "| 改动项 | 有没有变 | 原因 |",
  "|---|---|---|",
  "| 内边距 | 没变 | 容器尺寸与卡片内边距沿用原值 |",
  "| 圆角 | 变了 | 由 8px 提到 12px，对齐新的控件尺度 |",
  "| 主色 | 没变 | 主题变量仍是同一套 primary |",
  "| 字号 | 变了 | 正文基准从 14px 降到 13px |",
  "| 换新 id: 7096 → 7097 之后旧消息重新挂载 | 变了 | 键值变化让 diff 认不出是同一条，整块 VNode 被销毁重建 |",
  "| 滚动容器 | 没变 | 仍走同一个 OverlayScrollArea |",
  "| 虚拟列表阈值 | 变了 | 触发线从 200 条下调到 120 条 |",
  "",
  "**一、改动范围**",
  "",
  "这次动的是视觉尺度与渲染路径两类，前者只影响观感，后者影响每帧的调用次数。两类混在同一批里交付，回归时要分开看。",
  "",
  "**二、最容易被误判的一条**",
  "",
  "换新 id 那一条看起来只是数据层的小动作，实际是整份清单里代价最大的一处。键值一变，diff 就无法把它认成同一条。",
  "",
  "**三、没有变的部分同样要核**",
  "",
  "没变的项不是不用管，而是回归时它们必须保持没变。",
  "",
  "**四、怎么验**",
  "",
  "视觉项逐页截图比对，渲染项给子组件渲染闭包加计数器。",
].join("\n");

function makeUserBlock(id: string, text: string): ChatMessageBlock {
  return {
    id,
    role: "user",
    text,
    images: [],
    audios: [],
    attachmentFiles: [],
    toolCallCount: 0,
    lastToolName: "",
    toolCalls: [],
    activityItems: [],
    activityReasoningCharCount: 0,
    activityToolCountsByName: {},
    activityRunning: false,
    activityStatus: "complete",
  };
}

function makeAssistantBlock(id: string, text: string, streaming: boolean): ChatMessageBlock {
  return {
    id,
    role: "assistant",
    text,
    isStreaming: streaming,
    images: [],
    audios: [],
    attachmentFiles: [],
    toolCallCount: 0,
    lastToolName: "",
    toolCalls: [],
    activityItems: [],
    activityReasoningCharCount: 0,
    activityToolCountsByName: {},
    activityRunning: streaming,
    activityStatus: streaming ? "running_tool" : "complete",
  };
}

const blocks = ref<ChatMessageBlock[]>([]);
const phase = ref<"idle" | "streaming">("idle");
const statusText = computed(() => (phase.value === "streaming" ? `流式中 ${cursor.value}/${SOURCE.length}` : "空闲"));
const cursor = ref(0);
const historyCount = ref(6);
const alignMode = ref<"own_top" | "follow">("own_top");
const scrollToBottomRequest = ref(0);
const alignRequest = ref(0);
let timer: ReturnType<typeof setInterval> | null = null;

function makeHistoryBlock(index: number): ChatMessageBlock {
  const isUser = index % 2 === 0;
  const filler = "这一段的长度故意写得参差不齐，用来把每条消息撑到不同的高度，让虚拟列表与尾部留白产生真实的结算压力。";
  return isUser
    ? makeUserBlock(`h-u-${index}`, `第 ${index + 1} 轮的问题：${filler.slice(0, 12 + (index % 7) * 6)}`)
    : makeAssistantBlock(
        `h-a-${index}`,
        `第 ${index + 1} 轮的回答。\n\n${filler.repeat(1 + (index % 4))}\n\n${"收尾一段，长度随轮次浮动。".repeat(1 + (index % 3))}`,
        false,
      );
}

function seed(): void {
  reset();
  const history: ChatMessageBlock[] = [];
  for (let i = 0; i < historyCount.value; i += 1) history.push(makeHistoryBlock(i));
  blocks.value = [...history,
    makeUserBlock("u-1", "先看一下弹性布局的收缩比怎么算。"),
    makeAssistantBlock("a-1", "收缩比按 flex-shrink 与基准尺寸的乘积分摊：每个项目被削掉的量，等于总溢出量乘以「本项目 shrink×basis 占全部项目该乘积之和的比例」。所以基准尺寸越大的项目被削得越多；把某项 shrink 设为 0，它就拒绝被压缩。", false),
    makeUserBlock("u-2", "那最小宽度兜底怎么写？"),
    makeAssistantBlock("a-2", "给项目加 min-width，或者用 flex-basis 加 min-inline-size。注意 min-width 的优先级高于 flex-shrink：一旦缩到 min-width 就不再继续缩，溢出部分会转嫁给其它项目。", false),
  ];
}
seed();

function reset(): void {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  phase.value = "idle";
  cursor.value = 0;
  blocks.value = [];
}

function pump(): void {
  if (timer) return;
  const id = `a-${Date.now()}`;
  const userId = `u-${Date.now()}`;
  const prior = blocks.value.slice();
  blocks.value = [...prior, makeUserBlock(userId, "给我一份 12 行的对比清单。"), makeAssistantBlock(id, "", true)];
  phase.value = "streaming";
  cursor.value = 0;
  if (alignMode.value === "own_top") {
    alignRequest.value += 1;
    scrollToBottomRequest.value += 1;
  } else {
    scrollToBottomRequest.value += 1;
  }

  timer = setInterval(() => {
    const target = blocks.value.find((b) => b.id === id);
    if (!target) return;
    const next = Math.min(SOURCE.length, cursor.value + 24);
    cursor.value = next;
    target.text = SOURCE.slice(0, next);
    if (next >= SOURCE.length) {
      target.isStreaming = false;
      target.activityRunning = false;
      target.activityStatus = "complete";
      phase.value = "idle";
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
    }
  }, 24);
}

onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

// 只为撑起组件契约：真实运行时由宿主注入，这里全部给中性值。
// 必须是 computed，否则 messageBlocks 变化不会传给子组件。
const demoProps = computed<ChatViewProps>(() => ({
  composerScope: "main",
  userAlias: "我",
  personaName: "卡维",
  userAvatarUrl: "",
  assistantAvatarUrl: "",
  personaNameMap: {},
  personaAvatarUrlMap: {},
  mentionEntries: [],
  selectedMentions: [],
  latestUserText: "",
  latestUserImages: [],
  frontendRoundPhase: phase.value,
  submitPending: false,
  chatErrorText: "",
  clipboardImages: [],
  queuedAttachmentNotices: [],
  chatInput: "",
  instructionPresets: [],
  conversationCallPrimaryApiConfigId: "",
  preferredChatModelId: "",
  toolReviewApiConfigId: "",
  toolReviewRefreshTick: 0,
  chatModelOptions: [],
  planModeEnabled: false,
  chatUsagePercent: 0,
  wheelchairModeEnabled: false,
  wheelchairSuggestion: null,
  mediaDragActive: false,
  chatting: phase.value === "streaming",
  trimming: false,
  compactingConversation: false,
  conversationBusy: false,
  frozen: false,
  messageBlocks: blocks.value,
  hasMoreHistory: false,
  loadingOlderHistory: false,
  latestOwnMessageAlignRequest: alignRequest.value,
  conversationScrollToBottomRequest: scrollToBottomRequest.value,
  scrollToBottomBehavior: alignMode.value,
  currentWorkspaceName: "demo",
  currentWorkspaceDisplayName: "demo",
  currentWorkspaceRootPath: "",
  workspaces: [],
  activeAgentId: "",
  activeConversationId: "demo-conversation",
  currentTodos: [],
  goalActive: false,
  goalTitle: "",
  goalDialogOpen: false,
  goalSaving: false,
  goalError: "",
  activeGoalTask: null,
  recentGoalTaskHistory: [],
  currentTheme: "light",
  unarchivedConversationItems: [],
  remoteImContactConversations: [],
  conversationItems: [],
  sideConversationListVisible: false,
  initialToolReviewPanelOpen: false,
  conversationListTab: "local",
  chatLeftPanelMode: "local",
  chatRightPanelMode: "monitor",
  chatMonitorPanelMode: "delegate",
  sideChatPanelEnabled: false,
  teamVisible: false,
  teamMembers: [],
  teamMemberAvatarUrlMap: {},
  createConversationAgentOptions: [],
  recipientOptionsReady: false,
  defaultCreateConversationAgentId: "",
  ideContextGroups: [],
  terminalApprovals: [],
  terminalApprovalResolving: false,
  hideConversationControlPanel: true,
  showOpenInBrowserButton: false,
  systemNotificationMode: false,
  hideWorkspaceButton: true,
  workspaceAccess: "approval",
}) as unknown as ChatViewProps);
</script>
