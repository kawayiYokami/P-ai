import { computed, onScopeDispose, ref, shallowRef, watch, type Ref } from "vue";
import {
  bindTransportConversationStream,
  chatStreamNeedsFrontendBind,
  invokeTauri,
  onTransportNotification,
  PROBE_NOTIFICATION_METHODS,
  probeTransportConversationStream,
  unbindTransportConversationStream,
} from "../../../services/tauri-api";
import type { AssistantStreamBlock, ChatMentionTarget, ChatMessage, ChatRewindCompletedPayload, ChatTodoItem } from "../../../types/app";
import { ensureConversationMessageIds } from "../utils/message-id";
import { registerChatFlowRuntime } from "./chat-flow-runtime-registry";
import { invokeSendChatMessage, invokeStopChatMessage } from "./chat-send-transport";
import type { ExclusiveChatViewSubscriptionSlot } from "./exclusive-chat-view-subscription-slot";
import {
  mergeAuthoritativeConversationMessages,
  replaceConversationHistory,
  type AuthoritativeMessageMergeOptions,
} from "./chat-message-state-machine";
import {
  createLatestTaskRunner,
  createForegroundTailWatermarkCoordinator,
  runForegroundSnapshotBindingTransaction,
  snapshotCanBindAssistantStream,
} from "./chat-foreground-coordinator";
import { reconcileForegroundRuntime } from "./foreground-recovery-state-machine";
import { useChatFlow } from "./use-chat-flow";
import { useChatScrollCoordinator } from "./use-chat-scroll-coordinator";
import { useChatRewindActions } from "./use-chat-rewind-actions";
import { DRAFT_USER_ID_PREFIX } from "./use-chat-flow-drafts";
import type { ConversationRuntimeStreamCacheSnapshot } from "./use-chat-flow-stream-cache";
import {
  extractMessageAttachmentFiles,
  extractMessageImages,
  messageText,
  removeBinaryPlaceholders,
} from "../../../utils/chat-message";
import { CHAT_QUEUE_OUT_OF_SYNC_EVENT } from "./use-chat-queue";
import { probeChatFlow } from "./chat-flow-probe";

type ConversationViewRuntimeOptions = {
  conversationId: Ref<string>;
  apiConfigId: Ref<string>;
  agentId: Ref<string>;
  departmentId: Ref<string>;
  subscriptionSlot?: ExclusiveChatViewSubscriptionSlot;
  t: (key: string, params?: Record<string, unknown>) => string;
  requestRecallMode?: (payload: {
    turnId: string;
    targetUserMessageId: string;
    conversationId?: string;
  }) => Promise<"with_patch" | "message_only" | "cancel">;
};

type ConversationRuntimeState = "idle" | "assistant_streaming" | "organizing_context" | "compacting";

type ConversationLightSnapshot = {
  conversationId?: string;
  messages?: ChatMessage[];
  preferredApiConfigId?: string | null;
  hasMoreHistory?: boolean;
  runtimeState?: ConversationRuntimeState | null;
  shouldBindStream?: boolean;
  streamCache?: ConversationRuntimeStreamCacheSnapshot | null;
  resumeProjectionAuthoritative?: boolean;
  currentTodos?: ChatTodoItem[];
  conversation?: { planModeEnabled?: boolean } | null;
};

type ConversationRuntimeSnapshot = {
  conversationId?: string;
  runtimeState?: ConversationRuntimeState;
  isProcessing?: boolean;
  hasPendingQueue?: boolean;
  pendingQueueCount?: number;
  streamCache?: ConversationRuntimeStreamCacheSnapshot | null;
};

export function useConversationViewRuntime(options: ConversationViewRuntimeOptions) {
  const allMessages = shallowRef<ChatMessage[]>([]);
  const chatInput = ref("");
  const selectedMentions = ref<ChatMentionTarget[]>([]);
  const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
  const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; path: string; mime: string }>>([]);
  const latestUserText = ref("");
  const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
  const chatErrorText = ref("");
  const chatting = ref(false);
  const trimming = ref(false);
  const submitPending = ref(false);
  const preferredApiConfigId = ref(String(options.apiConfigId.value || "").trim());
  const hasMoreHistory = ref(false);
  const loadingOlderHistory = ref(false);
  const currentTodos = ref<ChatTodoItem[]>([]);
  const planModeEnabled = ref(false);
  const runtimeState = ref<ConversationRuntimeState>("idle");
  const foregroundSyncing = ref(false);
  // flow 发送保护用：流式/提交/组织上下文都算忙碌（与主窗口 use-chat-runtime-setup 一致）。
  const conversationBusy = computed(() =>
    submitPending.value
    || chatting.value
    || runtimeState.value === "assistant_streaming"
    || runtimeState.value === "organizing_context"
  );
  // 视图层交互忙碌由组件层统一调用 isViewLayerBusy 判定（chat-view-busy.ts），
  // 不再在 runtime 内自建一份拼装，避免与主会话外壳分叉。
  let snapshotRequestSequence = 0;
  let disposed = false;
  const foregroundTailWatermark = createForegroundTailWatermarkCoordinator({
    requestFreshness: async (conversationId) => {
      const snapshot = await invokeTauri<{ lastMessageId?: string | null; updatedAt?: string | null }>("conversation.freshnessSnapshot", {
        input: { conversationId, agentId: null },
      });
      return {
        lastMessageId: String(snapshot?.lastMessageId || "").trim(),
        updatedAt: String(snapshot?.updatedAt || "").trim(),
      };
    },
  });

  function currentConversationId() {
    if (disposed) return "";
    return String(options.conversationId.value || "").trim();
  }

  function mergeAuthoritativeMessages(
    messages: ChatMessage[],
    incomingMessages: ChatMessage[],
    mergeOptions?: AuthoritativeMessageMergeOptions,
  ): ChatMessage[] {
    return mergeAuthoritativeConversationMessages(
      messages,
      ensureConversationMessageIds(incomingMessages),
      mergeOptions,
    );
  }

  function currentFormalTailMessageId(): string {
    const formalMessages = allMessages.value.filter((message) => {
      const messageId = String(message.id || "").trim();
      return !!messageId
        && !messageId.startsWith(DRAFT_USER_ID_PREFIX);
    });
    return String(formalMessages[formalMessages.length - 1]?.id || "").trim();
  }

  function frontendConversationIsStreaming(): boolean {
    const phase = String(flow.frontendRoundPhase.value || "").trim();
    return chatting.value || phase === "queued" || phase === "waiting" || phase === "streaming";
  }

  function applySnapshot(snapshot: ConversationLightSnapshot, preserveExistingHistory: boolean) {
    const incomingMessages = ensureConversationMessageIds(Array.isArray(snapshot?.messages) ? snapshot.messages : []);
    allMessages.value = preserveExistingHistory
      ? mergeAuthoritativeMessages(allMessages.value, incomingMessages)
      : replaceConversationHistory(allMessages.value, incomingMessages);
    const snapshotApiConfigId = String(snapshot?.preferredApiConfigId || "").trim();
    if (snapshotApiConfigId) preferredApiConfigId.value = snapshotApiConfigId;
    hasMoreHistory.value = !!snapshot?.hasMoreHistory;
    currentTodos.value = Array.isArray(snapshot?.currentTodos) ? snapshot.currentTodos : [];
    planModeEnabled.value = !!snapshot?.conversation?.planModeEnabled;
    runtimeState.value = snapshot?.runtimeState || "idle";
    // 打开追问时恢复后端持久化的轮次错误，保证重启/切换后仍可见。
    const snapshotLastError = String((snapshot?.conversation as Record<string, any> | null | undefined)?.lastError || "").trim();
    if (snapshotLastError) {
      chatErrorText.value = snapshotLastError;
    }
  }

  async function requestSnapshot(conversationId: string) {
    const requestSequence = ++snapshotRequestSequence;
    if (!conversationId) {
      allMessages.value = [];
      currentTodos.value = [];
      planModeEnabled.value = false;
      runtimeState.value = "idle";
      return null;
    }
    const snapshot = await invokeTauri<ConversationLightSnapshot>("conversation.foregroundLightSnapshot", {
      input: { conversationId, agentId: null, limit: 50, resumeProjection: true },
    });
    const snapshotConversationId = String(snapshot?.conversationId || conversationId).trim();
    if (
      requestSequence !== snapshotRequestSequence
      || conversationId !== currentConversationId()
      || snapshotConversationId !== conversationId
    ) return null;
    return snapshot;
  }

  let foregroundSyncSequence = 0;
  // 同一 ConversationView 的解绑命令只按 bindingId 生效；恢复事务必须串行，
  // 否则较早的解绑可能晚于新绑定完成，并把刚建立的 Channel 一并移除。
  let foregroundSyncQueue: Promise<void> = Promise.resolve();

  function synchronizeConversation(
    conversationId: string,
    syncOptions: { clearRuntime: boolean; preserveExistingHistory: boolean },
  ): Promise<void> {
    const syncSequence = ++foregroundSyncSequence;
    const task = foregroundSyncQueue.then(async () => {
      if (disposed || !conversationId || conversationId !== currentConversationId()) return;
      foregroundSyncing.value = true;
      try {
        await runForegroundSnapshotBindingTransaction({
          conversationId,
          isCurrent: () => !disposed
            && conversationId === currentConversationId()
            && syncSequence === foregroundSyncSequence,
          clearRuntime: () => {
            if (syncOptions.clearRuntime) flow.clearForegroundRuntimeState();
          },
          unbind: flow.unbindActiveConversationStream,
          requestSnapshot: () => requestSnapshot(conversationId),
          applySnapshot: (snapshot) => applySnapshot(snapshot, syncOptions.preserveExistingHistory),
          bind: () => flow.bindActiveConversationStream(conversationId, true),
          alwaysBind: chatStreamNeedsFrontendBind(),
          resume: (snapshot) => {
            const runtimeState = String(snapshot?.runtimeState || "").trim();
            const streamCache = snapshot?.streamCache as Record<string, unknown> | null | undefined;
            if (runtimeState !== "assistant_streaming" || !snapshotCanBindAssistantStream(snapshot)) {
              return;
            }
            flow.resumeForegroundRuntimeRound({
              conversationId,
              streamCache: snapshot.streamCache || null,
              statusText: options.t("chat.statusWaitingReply"),
              reason: "conversation_view_snapshot_ready",
            });
          },
          onUnbindError: (error) => {
            console.warn("[追问会话] 取消流式通道绑定失败", { conversationId, error });
          },
        });
      } finally {
        if (syncSequence === foregroundSyncSequence) {
          foregroundSyncing.value = false;
        }
      }
    });
    foregroundSyncQueue = task.catch((error) => {
      console.error("[追问会话] 前台同步失败", { conversationId, error });
      if (syncSequence === foregroundSyncSequence) {
        foregroundSyncing.value = false;
        chatErrorText.value = String(error instanceof Error ? error.message : error || "");
      }
    });
    return foregroundSyncQueue;
  }

  async function loadSnapshot() {
    const conversationId = currentConversationId();
    if (!conversationId) {
      ++snapshotRequestSequence;
      allMessages.value = [];
      currentTodos.value = [];
      planModeEnabled.value = false;
      runtimeState.value = "idle";
      return;
    }
    await synchronizeConversation(conversationId, {
      clearRuntime: true,
      preserveExistingHistory: true,
    });
  }

  async function loadOlderHistory() {
    const conversationId = currentConversationId();
    const oldestMessageId = String(allMessages.value[0]?.id || "").trim();
    if (!conversationId || !oldestMessageId || !hasMoreHistory.value || loadingOlderHistory.value) return;
    loadingOlderHistory.value = true;
    try {
      const result = await invokeTauri<{ messages?: ChatMessage[]; hasMore?: boolean }>("conversation.messagesBefore", {
        input: { conversationId, beforeMessageId: oldestMessageId, limit: 50 },
      });
      if (conversationId !== currentConversationId()) return;
      const incoming = ensureConversationMessageIds(Array.isArray(result?.messages) ? result.messages : []);
      allMessages.value = mergeAuthoritativeMessages(allMessages.value, incoming, {
        prependMessages: true,
      });
      hasMoreHistory.value = !!result?.hasMore;
    } finally {
      loadingOlderHistory.value = false;
    }
  }

  // 时间线按「压缩段」补载：一次取锚点之前的一整段（服务端按压缩边界切块，一段即一个物理块文件）。
  // 与 loadOlderHistory 共用 loadingOlderHistory 互斥，锚点同为当前最旧消息，两条路径前插互不错位。
  async function loadOlderCompactionSegment() {
    const conversationId = currentConversationId();
    const oldestMessageId = String(allMessages.value[0]?.id || "").trim();
    if (!conversationId || !oldestMessageId || !hasMoreHistory.value || loadingOlderHistory.value) return;
    loadingOlderHistory.value = true;
    try {
      const result = await invokeTauri<{ messages?: ChatMessage[]; hasMore?: boolean }>("conversation.compactionSegmentBefore", {
        input: { conversationId, anchorMessageId: oldestMessageId },
      });
      if (conversationId !== currentConversationId()) return;
      const incoming = ensureConversationMessageIds(Array.isArray(result?.messages) ? result.messages : []);
      if (incoming.length > 0) {
        allMessages.value = mergeAuthoritativeMessages(allMessages.value, incoming, {
          prependMessages: true,
        });
      }
      hasMoreHistory.value = !!result?.hasMore;
    } finally {
      loadingOlderHistory.value = false;
    }
  }

  async function refreshMessageById(conversationId: string, messageId: string) {
    const message = await invokeTauri<ChatMessage | null>("conversation.messageById", {
      input: { conversationId, messageId },
    });
    if (!message || conversationId !== currentConversationId()) return false;
    allMessages.value = mergeAuthoritativeMessages(allMessages.value, [message], { forceReplace: true });
    return true;
  }

  const {
    conversationScrollToBottomRequest,
    scrollToBottomBehavior,
    triggerConversationScrollToBottom,
  } = useChatScrollCoordinator({
    currentChatConversationId: options.conversationId,
  });

  const flow = useChatFlow({
    chatting,
    submitPending,
    trimming,
    isConversationBusy: () => foregroundSyncing.value || conversationBusy.value,
    getSession: () => {
      const apiConfigId = String(preferredApiConfigId.value || options.apiConfigId.value || "").trim();
      const agentId = String(options.agentId.value || "").trim();
      if (!apiConfigId || !agentId) return null;
      return { apiConfigId, agentId, departmentId: String(options.departmentId.value || "").trim() };
    },
    getConversationId: currentConversationId,
    chatInput,
    selectedMentions,
    clipboardImages,
    queuedAttachmentNotices,
    latestUserText,
    latestUserImages,
    subscribeExternalEvents: (method, handler) => onTransportNotification(method, (payload) => {
      if (method === "chat.roundStarted") runtimeState.value = "assistant_streaming";
      if (PROBE_NOTIFICATION_METHODS.has(method)) {
        const record = payload && typeof payload === "object" ? payload as Record<string, unknown> : null;
        probeChatFlow("事件到达", {
          method,
          payloadConversationId: String(record?.conversationId || ""),
          currentConversationId: currentConversationId(),
        });
      }
      void Promise.resolve().then(() => handler(payload)).catch((error) => {
        probeChatFlow("事件处理异常", {
          method,
          error: String((error as { message?: unknown })?.message || error || ""),
          stack: String((error as { stack?: unknown })?.stack || "").slice(0, 800),
        });
      }).finally(() => {
        if (method === "chat.roundFinished" && !frontendConversationIsStreaming()) {
          runtimeState.value = "idle";
        }
      });
    }),
    chatErrorText,
    allMessages,
    t: options.t,
    formatRequestFailed: (error) => String(error instanceof Error ? error.message : error || ""),
    removeBinaryPlaceholders: (text) => text,
    invokeSendChatMessage,
    invokeStopChatMessage,
    refreshMessageById: ({ conversationId, messageId }) => refreshMessageById(conversationId, messageId),
    invokeBindActiveChatViewStream: bindTransportConversationStream,
    invokeUnbindActiveChatViewStream: unbindTransportConversationStream,
    invokeProbeActiveChatViewStream: probeTransportConversationStream,
    coordinateActiveConversationStreamBind: ({ bindingId, conversationId, bind, unbind }) => {
      if (!options.subscriptionSlot) return bind();
      return options.subscriptionSlot.acquire({
        ownerId: bindingId,
        conversationId,
        bind,
        unbind,
      });
    },
    onReloadMessages: loadSnapshot,
    onOwnUserDraftInserted: ({ conversationId }) => {
      triggerConversationScrollToBottom(conversationId, "draft_inserted", "own_top");
    },
    onStreamingAssistantBubbleInserted: () => {
      const cid = currentConversationId();
      if (cid) triggerConversationScrollToBottom(cid, "assistant_bubble_inserted", "own_top");
    },
    onHistoryFlushed: async ({ conversationId, pendingMessages }) => {
      if (conversationId !== currentConversationId()) return;
      allMessages.value = mergeAuthoritativeMessages(allMessages.value, pendingMessages, {
        replaceOptimisticUserDrafts: true,
        summarySeedsFirst: true,
      });
    },
    onAssistantMessageCompleted: async ({ conversationId, assistantMessage }) => {
      if (conversationId !== currentConversationId()) return;
      allMessages.value = mergeAuthoritativeMessages(allMessages.value, [assistantMessage]);
    },
  });

  async function requestRuntimeSnapshot(conversationId: string) {
    return invokeTauri<ConversationRuntimeSnapshot>("conversation.runtimeSnapshot", {
      conversationId,
    });
  }

  async function requestLatestFormalTailMessageId(conversationId: string) {
    const snapshot = await invokeTauri<{ lastMessageId?: string | null }>("conversation.freshnessSnapshot", {
      input: { conversationId, agentId: null },
    });
    return String(snapshot?.lastMessageId || "").trim();
  }

  async function reconcileForegroundConversation(reason: string) {
    const conversationId = currentConversationId();
    if (!conversationId) return;
    // 切会话快照进行中时不能丢弃 focus；等待同一串行事务结束后，再对其结果做一次
    // 统一尾部对账，避免冻结期间漏掉正式消息。
    if (foregroundSyncing.value) await foregroundSyncQueue;
    if (disposed || conversationId !== currentConversationId()) return;
    try {
      await foregroundTailWatermark.observeCurrentConversation(conversationId);
    } catch (error) {
      console.warn("[追问会话] 前台水位查询失败，回退轻量快照", { conversationId, error });
      await synchronizeConversation(conversationId, {
        clearRuntime: true,
        preserveExistingHistory: true,
      });
      return;
    }
    if (disposed || conversationId !== currentConversationId()) return;
    const runtimeSnapshot = await requestRuntimeSnapshot(conversationId);
    if (disposed || conversationId !== currentConversationId()) return;
    const frontendStreamCache = flow.readConversationStreamCache?.(conversationId);
    const outcome = await reconcileForegroundRuntime({
      conversationId,
      runtimeSnapshot,
      frontendStreaming: frontendConversationIsStreaming(),
      frontendMessageId: frontendStreamCache?.persistedAssistantMessageId,
      frontendActivationId: frontendStreamCache?.activationId,
      frontendRequestId: frontendStreamCache?.requestId,
      frontendRevision: frontendStreamCache?.updatedAt,
    }, {
      probeStream: (targetConversationId) => flow.probeBoundChannel(targetConversationId),
      resumeSubscription: async (targetConversationId) => {
        await flow.bindActiveConversationStream(targetConversationId, true);
        return requestRuntimeSnapshot(targetConversationId);
      },
      applyRuntimeSnapshot: (snapshot) => {
        if (disposed || conversationId !== currentConversationId()) return false;
        runtimeState.value = (snapshot.runtimeState as ConversationRuntimeState) || "idle";
        return flow.resumeForegroundRuntimeRound({
          conversationId,
          streamCache: snapshot.streamCache || null,
          reason: `foreground_${reason}`,
        }) > 0;
      },
      refreshMessageById,
      finalizeMessage: async () => {
        flow.clearForegroundRuntimeState();
        runtimeState.value = "idle";
      },
      applyBackgroundBusy: (snapshot) => {
        runtimeState.value = (snapshot.runtimeState as ConversationRuntimeState) || "organizing_context";
      },
      isCurrent: () => !disposed && conversationId === currentConversationId(),
      currentFormalTailMessageId,
      requestLatestFormalTailMessageId,
      shouldReconcileTail: () => foregroundTailWatermark.shouldReconcileTail(conversationId),
      reloadConversation: () => synchronizeConversation(conversationId, {
        clearRuntime: true,
        preserveExistingHistory: true,
      }),
    });
    if (outcome === "tail_reconciled") foregroundTailWatermark.markTailReconciled(conversationId);
  }

  const foregroundRecoveryRunner = createLatestTaskRunner(async (reason: string) => {
    await reconcileForegroundConversation(reason);
  });

  function scheduleForegroundRecovery(reason = "unknown") {
    return foregroundRecoveryRunner.run(reason).catch((error) => {
        console.error("[追问会话] 前台恢复失败", {
          conversationId: currentConversationId(),
          error,
        });
      });
  }

  const handleExternalRoundStarted = flow.handleExternalRoundStarted.bind(flow);
  const handleExternalRoundCompleted = flow.handleExternalRoundCompleted.bind(flow);
  const handleExternalRoundFailed = flow.handleExternalRoundFailed.bind(flow);
  const runtimeEventHandlers = Object.assign({}, flow, {
    async handleExternalRoundStarted(payload: unknown) {
      runtimeState.value = "assistant_streaming";
      await handleExternalRoundStarted(payload);
    },
    async handleExternalRoundCompleted(payload: unknown) {
      await handleExternalRoundCompleted(payload);
      if (!frontendConversationIsStreaming()) {
        runtimeState.value = "idle";
      }
    },
    async handleExternalRoundFailed(payload: unknown) {
      await handleExternalRoundFailed(payload);
      if (!frontendConversationIsStreaming()) {
        runtimeState.value = "idle";
      }
    },
    handleExternalMessageAppended(payload: unknown) {
      if (!payload || typeof payload !== "object") return;
      const record = payload as { conversationId?: string; message?: ChatMessage };
      if (String(record.conversationId || "").trim() !== currentConversationId() || !record.message) return;
      allMessages.value = mergeAuthoritativeMessages(allMessages.value, [record.message]);
    },
    handleExternalMessagesAfterSynced(payload: unknown) {
      if (!payload || typeof payload !== "object") return;
      const record = payload as { conversationId?: string; messages?: ChatMessage[]; error?: unknown };
      if (String(record.conversationId || "").trim() !== currentConversationId() || record.error) return;
      allMessages.value = mergeAuthoritativeMessages(
        allMessages.value,
        Array.isArray(record.messages) ? record.messages : [],
      );
    },
    handleExternalRuntimeStateUpdated(payload: unknown) {
      if (!payload || typeof payload !== "object") return;
      const record = payload as { conversationId?: string; runtimeState?: ConversationRuntimeState };
      if (String(record.conversationId || "").trim() !== currentConversationId()) return;
      const nextRuntimeState = String(record.runtimeState || "").trim();
      if (
        nextRuntimeState !== "idle"
        && nextRuntimeState !== "assistant_streaming"
        && nextRuntimeState !== "organizing_context"
        && nextRuntimeState !== "compacting"
      ) return;
      runtimeState.value = nextRuntimeState;
      const frontendStreaming = frontendConversationIsStreaming();
      const shouldRecover = nextRuntimeState === "idle"
        ? frontendStreaming
        : !frontendStreaming || !flow.hasActiveBoundDeltaChannel(currentConversationId());
      if (shouldRecover) void scheduleForegroundRecovery();
    },
    handleExternalTodosUpdated(payload: unknown) {
      if (!payload || typeof payload !== "object") return;
      const record = payload as { conversationId?: string; currentTodos?: ChatTodoItem[] };
      if (String(record.conversationId || "").trim() !== currentConversationId()) return;
      currentTodos.value = Array.isArray(record.currentTodos) ? record.currentTodos : [];
    },
  });
  const unregister = registerChatFlowRuntime({
    bindingId: flow.bindingId,
    getConversationId: currentConversationId,
    flow: runtimeEventHandlers,
  });

  watch(options.conversationId, async () => {
    const conversationId = currentConversationId();
    chatErrorText.value = "";
    preferredApiConfigId.value = String(options.apiConfigId.value || "").trim();
    runtimeState.value = "idle";
    if (!conversationId) {
      ++foregroundSyncSequence;
      ++snapshotRequestSequence;
      flow.clearForegroundRuntimeState();
      await flow.unbindActiveConversationStream().catch(() => {});
      return;
    }
    await synchronizeConversation(conversationId, {
      clearRuntime: true,
      preserveExistingHistory: false,
    });
  }, { immediate: true });

  function handleForegroundWake(event: Event) {
    if (document.visibilityState === "hidden") return;
    console.warn("[焦点恢复] focus/visibility 触发", {
      eventType: event.type,
      visibilityState: document.visibilityState,
      conversationId: currentConversationId(),
    });
    void scheduleForegroundRecovery(event.type);
  }

  window.addEventListener("focus", handleForegroundWake);
  document.addEventListener("visibilitychange", handleForegroundWake);

  // 队列撤回/切引导拿到后端准确不在队列反馈时回源同步权威快照；
  // 仅本会话失配才动，空会话与跨会话直接忽略。
  function handleQueueOutOfSync(event: Event) {
    const detail = (event as CustomEvent<{ conversationId?: string } | undefined>)?.detail;
    const staleConversationId = String(detail?.conversationId || "").trim();
    const currentId = currentConversationId();
    if (!staleConversationId || !currentId || staleConversationId !== currentId) return;
    void synchronizeConversation(currentId, {
      clearRuntime: true,
      preserveExistingHistory: false,
    });
  }

  window.addEventListener(CHAT_QUEUE_OUT_OF_SYNC_EVENT, handleQueueOutOfSync);

  const rewindActions = useChatRewindActions({
    activeApiConfigId: preferredApiConfigId,
    activeAgentId: options.agentId,
    currentConversationId: options.conversationId,
    allMessages,
    chatting,
    trimming,
    compactingConversation: ref(false),
    chatErrorText,
    chatInput,
    selectedMentions,
    clipboardImages,
    queuedAttachmentNotices,
    deleteUnarchivedConversationFromArchives: async () => {},
    sendChat: async () => flow.sendChat(),
    setStatusError: () => {},
    setChatErrorText: (text: string) => {
      chatErrorText.value = text;
    },
    removeBinaryPlaceholders,
    messageText,
    extractMessageImages,
    extractMessageAttachmentFiles,
    requestRecallMode: options.requestRecallMode || (async () => "message_only"),
    requestCreateConversationBranchFromMessageConfirm: async () => false,
    createConversationBranchFromMessage: async () => {},
    branchingConversation: ref(false),
    refreshForegroundConversationAfterRewind: async (conversationId: string) => {
      await synchronizeConversation(conversationId, {
        clearRuntime: true,
        preserveExistingHistory: false,
      });
    },
  });
  const stopRewindCompletedEvent = onTransportNotification<ChatRewindCompletedPayload>(
    "chat.rewindCompleted",
    (payload) => {
      const conversationId = String(payload?.conversationId || "").trim();
      const currentId = currentConversationId();
      if (!conversationId || conversationId !== currentId) return;
      const messages = [...allMessages.value];
      if (messages.length === 0) return;
      const remainingLastMessageId = String(payload?.remainingLastMessageId || "").trim();
      const targetMessageId = String(payload?.targetMessageId || "").trim();
      let cutIndex = -1;
      if (remainingLastMessageId) {
        const keepIndex = messages.findIndex((message) => String(message.id || "").trim() === remainingLastMessageId);
        if (keepIndex >= 0) cutIndex = keepIndex + 1;
      }
      if (cutIndex < 0 && targetMessageId) {
        const targetIndex = messages.findIndex((message) => String(message.id || "").trim() === targetMessageId);
        if (targetIndex >= 0) cutIndex = targetIndex;
      }
      // 两个边界 ID 都不在当前已加载切片内时，本地无法安全裁剪（撤回点可能在切片更早处），回源同步权威快照而非静默保留
      if (cutIndex < 0) {
        void synchronizeConversation(conversationId, {
          clearRuntime: true,
          preserveExistingHistory: false,
        });
        return;
      }
      // 边界已找到但落在切片尾部或之后，本地没有可裁的冗余消息
      if (cutIndex >= messages.length) return;
      allMessages.value = messages.slice(0, cutIndex);
      console.info("[会话撤回] 收到撤回广播，已裁剪侧聊消息", {
        conversationId,
        targetMessageId,
        remainingLastMessageId,
        cutIndex,
      });
    },
  );

  const stopChatErrorClearedEvent = onTransportNotification<{ conversationId?: string }>(
    "conversation.chatErrorCleared",
    (payload) => {
      const conversationId = String(payload?.conversationId || "").trim();
      if (!conversationId || conversationId !== currentConversationId()) return;
      chatErrorText.value = "";
    },
  );

  onScopeDispose(() => {
    stopRewindCompletedEvent();
    stopChatErrorClearedEvent();
    disposed = true;
    foregroundRecoveryRunner.cancel();
    ++foregroundSyncSequence;
    ++snapshotRequestSequence;
    window.removeEventListener("focus", handleForegroundWake);
    document.removeEventListener("visibilitychange", handleForegroundWake);
    window.removeEventListener(CHAT_QUEUE_OUT_OF_SYNC_EVENT, handleQueueOutOfSync);
    unregister();
    const unbindPromise = flow.unbindActiveConversationStream().catch(() => {});
    if (options.subscriptionSlot) {
      void options.subscriptionSlot.release(flow.bindingId, unbindPromise).catch(() => {});
    } else {
      void unbindPromise;
    }
  });

  return {
    flow: runtimeEventHandlers,
    allMessages,
    chatInput,
    selectedMentions,
    clipboardImages,
    queuedAttachmentNotices,
    latestUserText,
    latestUserImages,
    chatErrorText,
    chatting,
    submitPending,
    runtimeState,
    conversationBusy,
    foregroundSyncing,
    conversationScrollToBottomRequest,
    scrollToBottomBehavior,
    preferredApiConfigId,
    hasMoreHistory,
    loadingOlderHistory,
    currentTodos,
    planModeEnabled,
    send: () => flow.sendChat(),
    stop: () => flow.stopChat(),
    handleRecallTurn: rewindActions.handleRecallTurn,
    handleRegenerateTurn: rewindActions.handleRegenerateTurn,
    loadSnapshot,
    loadOlderHistory,
    loadOlderCompactionSegment,
  };
}
