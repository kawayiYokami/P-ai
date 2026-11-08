<template>
  <div class="flex flex-col gap-2 p-2">
    <div class="flex flex-wrap items-center gap-2 text-xs">
      <label class="flex items-center gap-1">
        <input v-model="draft" type="checkbox" class="checkbox checkbox-xs" />
        草稿会话（带收件人卡片）
      </label>
      <select v-model.number="demoWidth" class="select select-bordered select-xs">
        <option :value="620">620</option>
        <option :value="480">480</option>
        <option :value="360">360（窄）</option>
      </select>
      <button type="button" class="btn btn-xs btn-primary" @click="runAll">跑分组测量</button>
      <span class="text-base-content/60">{{ statusText }}</span>
    </div>

    <div
      ref="stageRef"
      class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100"
      :style="{ width: `min(${demoWidth}px, 100%)`, height: '720px' }"
    >
      <ChatView v-bind="chatViewProps" />
    </div>

    <pre class="demo-measure max-h-40 overflow-auto rounded bg-base-200 p-2 text-[11px] leading-4">{{ reportText }}</pre>
    <p class="text-xs text-base-content/60">
      <span class="font-medium text-base-content/80">验的是：</span>虚拟列表补上 startMargin 后，<code>scrollToIndex</code>
      的落点是否贴合滚动容器顶部（普通会话、草稿会话、宽度变化后各一组）。
      真实 ChatView + {{ turnCount }} 个 mock 回合，数据只从 props 来，不发请求；草稿会话靠 overview 项的 isDraft
      触发收件人卡片。点「跑分组测量」打印每组的像素差。
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import ChatView from "./ChatView.vue";
import type { ChatMessageBlock } from "../../../types/app";

type ChatViewProps = InstanceType<typeof ChatView>["$props"];

const TURN_COUNT = 30;
const demoWidth = ref(620);
const draft = ref(false);
const stageRef = ref<HTMLElement | null>(null);
const statusText = ref("待测量");
const reportText = ref("");

const FILLER = "这一段长度故意写得参差不齐，用来把每条消息撑到不同高度，让虚拟列表的落点测量有真实的几何差异。";

function makeBlock(index: number, role: "user" | "assistant"): ChatMessageBlock {
  const isUser = role === "user";
  return {
    id: `m-${index}-${isUser ? "u" : "a"}`,
    sourceMessageId: `m-${index}-${isUser ? "u" : "a"}`,
    role,
    text: isUser
      ? `第 ${index} 回合的问题：${FILLER.slice(0, 10 + (index % 5) * 8)}`
      : `第 ${index} 回合的回复。\n\n${FILLER.repeat(1 + (index % 3))}\n\n收尾一段，长度随回合浮动。`,
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
  } as ChatMessageBlock;
}

const blocks = computed<ChatMessageBlock[]>(() => {
  const list: ChatMessageBlock[] = [];
  for (let i = 0; i < TURN_COUNT; i += 1) {
    list.push(makeBlock(i, "user"), makeBlock(i, "assistant"));
  }
  return list;
});

const turnCount = computed(() => TURN_COUNT);

const overviewItems = computed(() => [
  {
    conversationId: "demo-conversation",
    title: "demo 会话",
    summaryTitle: "demo 会话",
    updatedAt: "2026-11-08T10:00:00+08:00",
    lastMessageAt: "2026-11-08T10:00:00+08:00",
    isDraft: draft.value,
  },
]);

// 组件契约照 ChatViewStreamDemo 的中性值给，只额外接上 messageBlocks 与 overview（草稿判定）。
const chatViewProps = computed<ChatViewProps>(
  () =>
    ({
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
      frontendRoundPhase: "idle",
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
      chatting: false,
      trimming: false,
      compactingConversation: false,
      conversationBusy: false,
      frozen: false,
      messageBlocks: blocks.value,
      hasMoreHistory: false,
      loadingOlderHistory: false,
      latestOwnMessageAlignRequest: 0,
      conversationScrollToBottomRequest: 0,
      scrollToBottomBehavior: "manual",
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
      unarchivedConversationItems: overviewItems.value,
      conversationItems: overviewItems.value,
      remoteImContactConversations: [],
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
    }) as unknown as ChatViewProps,
);

type VirtuaHandle = {
  scrollToIndex?: (index: number, options?: { align?: "start" | "center" | "end" | "nearest"; smooth?: boolean }) => void;
  findItemIndex?: (offset: number) => number;
};

function chatScrollContainer(): HTMLElement | null {
  return stageRef.value?.querySelector<HTMLElement>(".ecall-chat-scroll-container") || null;
}

/** virtua 句柄从 DOM 上挂的组件实例取：Virtualizer 根元素是 history-flow 的第一个子元素。 */
function virtuaHandle(): VirtuaHandle | null {
  const flow = stageRef.value?.querySelector(".ecall-chat-history-flow");
  const root = flow?.firstElementChild as
    | (HTMLElement & { __vueParentComponent?: { exposed?: VirtuaHandle } })
    | null
    | undefined;
  return root?.__vueParentComponent?.exposed || null;
}

const nextFrame = () =>
  new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));

function readStartMargin(): number | null {
  const scroller = chatScrollContainer();
  const flow = stageRef.value?.querySelector<HTMLElement>(".ecall-chat-history-flow");
  if (!scroller || !flow) return null;
  const raw = flow.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
  return Math.round(raw * 100) / 100;
}

async function measureAt(index: number) {
  const handle = virtuaHandle();
  const scroller = chatScrollContainer();
  if (!handle?.scrollToIndex || !scroller) return { index, top: null as number | null, note: "拿不到 virtua 句柄" };
  handle.scrollToIndex(index, { align: "start" });
  await nextFrame();
  const target = scroller.querySelector<HTMLElement>(`[data-turn-id="turn-m-${index}-u"]`);
  if (!target) return { index, top: null as number | null, note: "目标回合未渲染" };
  const top = Math.round((target.getBoundingClientRect().top - scroller.getBoundingClientRect().top) * 100) / 100;
  return { index, top, note: "", scrollTop: Math.round(scroller.scrollTop) };
}

async function runAll() {
  statusText.value = "测量中…";
  const lines: string[] = [];
  const push = (label: string, value: unknown) => lines.push(`${label}: ${JSON.stringify(value)}`);

  await nextTick();
  await nextFrame();
  const scroller = chatScrollContainer();
  push("草稿会话", draft.value);
  push("容器宽/高", scroller ? `${scroller.clientWidth}x${scroller.clientHeight}` : null);
  push("virtua 句柄", !!virtuaHandle());
  push("startMargin 实测", readStartMargin());
  push("有收件人卡片", !!stageRef.value?.querySelector(".ecall-chat-scroll-container .absolute.inset-0"));

  for (const k of [0, 5, 20]) push(`scrollToIndex(${k}) 顶部差`, await measureAt(k));

  const before = readStartMargin();
  demoWidth.value = demoWidth.value === 360 ? 620 : 360;
  await nextTick();
  await nextFrame();
  await nextFrame();
  push("换宽后 startMargin", { before, after: readStartMargin() });
  push("换宽后 scrollToIndex(20) 顶部差", await measureAt(20));

  const diffs: number[] = [];
  for (const k of [18, 3, 22, 7, 27, 11, 25, 1, 15, 9]) {
    const result = await measureAt(k);
    if (typeof result.top === "number") diffs.push(result.top);
  }
  push("连续 10 跳差值", diffs);
  push("差值最大绝对值", diffs.length ? Math.max(...diffs.map((d) => Math.abs(d))) : null);

  reportText.value = lines.join("\n");
  statusText.value = "测量完成";
}

onMounted(() => {
  (window as unknown as { __chatDemo?: unknown }).__chatDemo = {
    runAll,
    measureAt,
    readStartMargin,
    setDraft: (value: boolean) => {
      draft.value = value;
    },
    setWidth: (value: number) => {
      demoWidth.value = value;
    },
    report: () => reportText.value,
  };
  statusText.value = "已挂载";
});
</script>
