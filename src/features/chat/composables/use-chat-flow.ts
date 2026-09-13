import { onBeforeUnmount, ref } from "vue";
import type { AssistantStreamBlock, ChatMessage } from "../../../types/app";
import { normalizeAssistantStreamBlocks, assistantContentBlocksFromMessage, assistantTextFromStreamBlocks } from "../../../utils/chat-message-semantics";
import { chatStreamNeedsFrontendBind } from "../../../services/tauri-api";
import { useChatFlowChannelBinding } from "./use-chat-flow-channel-binding";
import {
  useChatFlowDrafts,
} from "./use-chat-flow-drafts";
import { useChatFlowExternalEvents } from "./use-chat-flow-external-events";
import { useChatFlowFrontendDispatch } from "./use-chat-flow-frontend-dispatch";
import { useChatFlowSendInput } from "./use-chat-flow-send-input";
import { useChatFlowSendController } from "./use-chat-flow-send-controller";
import {
  readDeltaMessage,
  readHistoryFlushedPayload,
  type AssistantDeltaEvent,
  type HistoryFlushedPayload,
  type RoundCompletedPayload,
  type RoundFailedPayload,
  type RoundStartedPayload,
} from "./use-chat-flow-events";
import { useChatFlowSendPayloads } from "./use-chat-flow-send-payloads";
import {
  useChatFlowStreamCache,
  type ConversationStreamCache,
} from "./use-chat-flow-stream-cache";
import { useChatFlowSendRecovery } from "./use-chat-flow-send-recovery";
import { useChatFlowStop } from "./use-chat-flow-stop";
import { useChatFlowStreamingEvents } from "./use-chat-flow-streaming-events";
import { useChatFlowRoundEvents } from "./use-chat-flow-round-events";
import { useChatFlowForegroundReset } from "./use-chat-flow-foreground-reset";
import { useChatFlowRoundFinalizers } from "./use-chat-flow-round-finalizers";
import { useChatFlowForegroundRounds } from "./use-chat-flow-foreground-rounds";
import { probeChatFlow } from "./chat-flow-probe";
import {
  normalizeConversationId,
} from "./use-chat-flow-utils";
import type {
  DeferredRoundCompletion,
  FrontendRoundPhase,
  PendingTerminalEvent,
  ResumeForegroundRuntimeRoundInput,
  RoundState,
  SendChatOverrides,
  UseChatFlowOptions,
} from "./use-chat-flow-types";

const CHAT_STREAM_DEBUG = typeof window !== "undefined"
  && window.localStorage.getItem("easy-call.debug.chat-stream") === "1";

export type { ConversationRuntimeStreamCacheSnapshot } from "./use-chat-flow-stream-cache";
export type { AssistantDeltaEvent } from "./use-chat-flow-events";
export type { FrontendRoundPhase, ResumeForegroundRuntimeRoundInput } from "./use-chat-flow-types";

export function useChatFlow(options: UseChatFlowOptions) {
  // ── 状态 ──
  let round: RoundState = { phase: "idle" };
  const frontendRoundPhase = ref<FrontendRoundPhase>("idle");
  const submitPending = options.submitPending ?? ref(false);
  let generation = 0;
  let sendChatActiveGen = 0; // 防止 bound channel 抢占 sendChat 轮次
  let historyFlushedReceivedGen = 0; // 记录 sendChat 轮次是否已收到 history_flushed，避免 finally 误回收
  let pendingTerminalEvent: PendingTerminalEvent | null = null;
  let deferredRoundCompletion: DeferredRoundCompletion | null = null;
  let foregroundRounds: ReturnType<typeof useChatFlowForegroundRounds> | null = null;
  let activeActivationId = "";
  let recentlyCompletedActivationId = "";
  let recentlyCompletedRequestId = "";
  let activeRoundAgentId = "";
  let queuedStreamingState: {
    assistantText: string;
    toolStatusText: string;
    toolStatusState: "running" | "done" | "failed" | "";
    streamBlocks: AssistantStreamBlock[];
    frontendDispatchStartedAtMs: number;
    frontendDispatchElapsedMs: number;
  } | null = null;
  const sendStartedAtMsByGen = new Map<number, number>();
  let activeHistoryMessageCount = 0;
  const {
    buildQueuedAttachmentPayload,
    buildImageAttachmentPayload,
    mergeAttachmentPayloads,
  } = useChatFlowSendPayloads({
    queuedAttachmentNotices: options.queuedAttachmentNotices,
  });
  const sendInput = useChatFlowSendInput({
    chatInput: options.chatInput,
    clipboardImages: options.clipboardImages,
    queuedAttachmentNotices: options.queuedAttachmentNotices,
    selectedMentions: options.selectedMentions,
    latestUserText: options.latestUserText,
    latestUserImages: options.latestUserImages,
    getSession: options.getSession,
    getConversationId: options.getConversationId,
    buildQueuedAttachmentPayload,
    buildImageAttachmentPayload,
    mergeAttachmentPayloads,
  });
  const frontendDispatch = useChatFlowFrontendDispatch({
    getMessageIdForGen: (gen) => {
      if ((round.phase === "queued" || round.phase === "streaming") && round.gen === gen) {
        return round.messageId;
      }
      return "";
    },
    isRoundActiveForGen: (gen) => (
      (round.phase === "queued" || round.phase === "streaming")
      && round.gen === gen
    ),
    syncCurrentDisplayStateToConversationStreamCache: () => {
      syncCurrentDisplayStateToConversationStreamCache();
    },
  });
  const {
    applyAssistantDeltaToMessage,
    applyAssistantEventToMessage,
    failMessage,
    finalizeMessage,
    getMessageStreamBlocks,
    getPendingUserDraftId,
    getPendingUserDraftIdForGen,
    hasStreamingAssistantMessageInMessages,
    insertStreamingAssistantMessage,
    insertUserDraft,
    removeMessage,
    settleStreamingAssistantMessages,
    syncStreamBlocksToMessage,
    updateMessageText,
    updateQueuedAssistantMessageStatus,
  } = useChatFlowDrafts({
    allMessages: options.allMessages,
    latestUserText: options.latestUserText,
    getActiveRoundAgentId: () => activeRoundAgentId,
    getConversationId: options.getConversationId,
    getSendStartedAtMs: (gen) => sendStartedAtMsByGen.get(gen) || 0,
    getActiveHistoryMessageCount: () => activeHistoryMessageCount,
    getFrontendDispatchStartedAtMs: frontendDispatch.getStartedAtMs,
    currentFrontendDispatchElapsedMs: frontendDispatch.currentElapsedMs,
  });
  const {
    applyAssistantEventToConversationStreamCache,
    applyConversationStreamCacheSnapshotToDisplay,
    applyConversationStreamCacheToDisplay,
    clearConversationStreamCache,
    readConversationStreamCache,
    syncCurrentDisplayStateToConversationStreamCache,
    writeConversationStreamCacheSnapshot,
  } = useChatFlowStreamCache({
    getConversationId: options.getConversationId,
    getCurrentDisplayState: () => {
      const currentRound = round;
      if (currentRound.phase !== "queued" && currentRound.phase !== "streaming") return null;
      if (!currentRound.messageId) return null;
      const message = options.allMessages.value.find((item) => item.id === currentRound.messageId);
      const blocks = assistantContentBlocksFromMessage(message);
      const meta = (message?.providerMeta || {}) as Record<string, unknown>;
      const rawStatus = String(meta._toolStatusState || "").trim();
      const toolStatusState = rawStatus === "running" || rawStatus === "done" || rawStatus === "failed"
        ? rawStatus
        : "";
      return {
        assistantText: assistantTextFromStreamBlocks(blocks),
        toolStatusText: String(meta._toolStatusText || meta._preStreamingStatusText || ""),
        toolStatusState,
        streamBlocks: blocks,
      };
    },
    getActiveActivationId: () => activeActivationId,
    getFrontendDispatchStartedAtMs: frontendDispatch.getStartedAtMs,
    getFrontendDispatchElapsedMs: frontendDispatch.getElapsedMs,
    currentFrontendDispatchElapsedMs: frontendDispatch.currentElapsedMs,
    restoreFrontendDispatchTimerFromCache,
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
  });
  const reasoningStartedAtMs = ref(0);
  const roundFinalizers = useChatFlowRoundFinalizers({
    getRound: () => round,
    setRound,
    getConversationId: options.getConversationId,
    allMessages: options.allMessages,
    refreshMessageById: options.refreshMessageById,
    getDeferredRoundCompletion: () => deferredRoundCompletion,
    setDeferredRoundCompletion: (value: DeferredRoundCompletion | null) => {
      deferredRoundCompletion = value;
    },
    chatting: options.chatting,
    reasoningStartedAtMs,
    t: options.t,
    clearChatErrorText,
    updateMessageText,
    finalizeMessage,
    failMessage,
    clearConversationStreamCache,
    clearFrontendDispatchTimer,
    setActiveActivationId: (value: string) => {
      activeActivationId = value;
    },
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
    onReloadMessages: options.onReloadMessages,
    removeMessage,
    setPendingTerminalEvent: (event: PendingTerminalEvent | null) => {
      pendingTerminalEvent = event;
    },
    setQueuedStreamingState: (value: typeof queuedStreamingState) => {
      queuedStreamingState = value
        ? { ...value, streamBlocks: normalizeAssistantStreamBlocks(value.streamBlocks) }
        : null;
    },
    sendStartedAtMsByGen,
    getPendingUserDraftId,
    formatRequestFailed: options.formatRequestFailed,
    setChatErrorText,
    applyAssistantDeltaToMessage,
    submitPending,
    flushStreamTextBuffer: () => streamingEvents.flushStreamTextBuffer(),
  });
  const streamingEvents = useChatFlowStreamingEvents({
    contextUsagePreview: options.contextUsagePreview,
    reasoningStartedAtMs,
    getRound: () => round,
    promoteQueuedRoundToStreaming,
    setPendingTerminalEvent: (event) => {
      pendingTerminalEvent = event;
    },
    clearConversationStreamCache,
    applyConversationStreamCacheSnapshotToDisplay,
    getConversationId: options.getConversationId,
    getActiveActivationId: () => activeActivationId,
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    handleRoundCompleted,
    handleRoundFailed,
    enqueueStreamDelta: roundFinalizers.enqueueStreamDelta,
    applyAssistantEventToMessage,
  });
  const channelBinding = useChatFlowChannelBinding({
    debug: CHAT_STREAM_DEBUG,
    getConversationId: options.getConversationId,
    invokeBindActiveChatViewStream: options.invokeBindActiveChatViewStream,
    invokeUnbindActiveChatViewStream: options.invokeUnbindActiveChatViewStream,
    invokeProbeActiveChatViewStream: options.invokeProbeActiveChatViewStream,
    getRoundActiveGen: () => (
      round.phase === "queued" || round.phase === "streaming" ? round.gen : 0
    ),
    getCurrentGeneration: () => generation,
    markHistoryFlushedReceived: (gen) => {
      historyFlushedReceivedGen = Math.max(historyFlushedReceivedGen, gen);
    },
    handleHistoryFlushed,
    handleStreamingEvent,
    formatRequestFailed: options.formatRequestFailed,
    setChatErrorText,
  });
  const bindActiveConversationStream = (conversationId: string, force = false) => {
    const bind = () => channelBinding.bindActiveConversationStream(conversationId, force);
    if (!options.coordinateActiveConversationStreamBind) return bind();
    return options.coordinateActiveConversationStreamBind({
      bindingId: channelBinding.bindingId,
      conversationId,
      force,
      bind,
      unbind: channelBinding.unbindActiveConversationStream,
    });
  };
  const coordinatedChannelBinding = {
    ...channelBinding,
    bindActiveConversationStream,
  };
  foregroundRounds = useChatFlowForegroundRounds({
    getRound: () => round,
    setRound,
    frontendDispatch,
    setFrontendRoundPhase: (value: FrontendRoundPhase) => {
      frontendRoundPhase.value = value;
    },
    nextGeneration: () => ++generation,
    getSendChatActiveGen: () => sendChatActiveGen,
    getActiveActivationId: () => activeActivationId,
    setActiveActivationId: (value: string) => {
      activeActivationId = value;
    },
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
    setPendingTerminalEvent: (event: PendingTerminalEvent | null) => {
      pendingTerminalEvent = event;
    },
    setDeferredRoundCompletion: (event: DeferredRoundCompletion | null) => {
      deferredRoundCompletion = event;
    },
    getQueuedStreamingState: () => queuedStreamingState,
    setQueuedStreamingState: (value: typeof queuedStreamingState) => {
      queuedStreamingState = value
        ? { ...value, streamBlocks: normalizeAssistantStreamBlocks(value.streamBlocks) }
        : null;
    },
    setActiveHistoryMessageCount: (value: number) => {
      activeHistoryMessageCount = value;
    },
    getConversationId: options.getConversationId,
    allMessages: options.allMessages,
    chatting: options.chatting,
    t: options.t,
    sendStartedAtMsByGen,
    channelBinding: coordinatedChannelBinding,
    clearConversationStreamCache,
    resetDisplayState,
    startFrontendDispatchTimer,
    updateQueuedAssistantMessageStatus,
    hasStreamingAssistantMessageInMessages,
    insertStreamingAssistantMessage,
    syncStreamBlocksToMessage,
    updateMessageText,
    applyConversationStreamCacheToDisplay,
    readConversationStreamCache,
    writeConversationStreamCacheSnapshot,
    applyPendingTerminalEvent,
  });
  const externalEvents = useChatFlowExternalEvents({
    debug: CHAT_STREAM_DEBUG,
    getCurrentConversationId: () => String(options.getConversationId ? options.getConversationId() : "").trim(),
    getActiveActivationId: () => activeActivationId,
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    clearRecentlyCompletedRoundIds,
    hasRecentlyCompletedRoundIds,
    markRecentlyCompletedRoundIds,
    matchesRecentlyCompletedRoundIds,
    getRound: () => round,
    setRound,
    getSendChatActiveGen: () => sendChatActiveGen,
    nextGeneration: () => ++generation,
    channelBinding: coordinatedChannelBinding,
    handleHistoryFlushed,
    beginAssistantActivationFromEvent,
    markRoundStarted,
    handleRoundCompleted,
    handleRoundFailed,
    clearConversationStreamCache,
    clearFrontendDispatchTimer,
    onReloadMessages: options.onReloadMessages,
    onAssistantMessageCompleted: options.onAssistantMessageCompleted,
    setChatErrorText,
    formatRequestFailed: options.formatRequestFailed,
    chatting: options.chatting,
    reasoningStartedAtMs,
    applyAssistantEventToConversationStreamCache,
    writeConversationStreamCacheSnapshot,
    applyConversationStreamCacheToDisplay,
    hasStreamingAssistantMessageInMessages,
    ensureForegroundStreamingRound,
    handleStreamingEvent,
    syncStreamBlocksToMessage,
    updateMessageText,
    flushStreamTextBuffer: (gen, messageId) => streamingEvents.flushStreamTextBuffer(gen, messageId),
  });
  function handleExternalRoundFinished(payload: unknown) {
    const value = payload && typeof payload === "object" ? payload as Record<string, unknown> : null;
    const status = String(value?.status || "").trim();
    if (status === "failed" || String(value?.error || "").trim()) {
      return externalEvents.handleExternalRoundFailed(payload);
    }
    return externalEvents.handleExternalRoundCompleted(payload);
  }
  const externalEventUnsubscribers = options.subscribeExternalEvents
    ? [
        ["chat.historyFlushed", externalEvents.handleExternalHistoryFlushed],
        ["chat.roundStarted", externalEvents.handleExternalRoundStarted],
        ["chat.assistantDelta", externalEvents.handleExternalAssistantDelta],
        ["chat.roundFinished", handleExternalRoundFinished],
        ["chat.streamRebindRequired", externalEvents.handleExternalStreamRebindRequired],
      ].map(([method, handler]) => options.subscribeExternalEvents!(method as string, handler as (payload: unknown) => void))
    : [];
  if (externalEventUnsubscribers.length > 0) {
    onBeforeUnmount(() => {
      for (const unsubscribe of externalEventUnsubscribers) unsubscribe();
    });
  }
  probeChatFlow("订阅注册", {
    hasSubscribe: !!options.subscribeExternalEvents,
    count: externalEventUnsubscribers.length,
    conversationId: String(options.getConversationId ? options.getConversationId() : ""),
  });
  const stopController = useChatFlowStop({
    chatting: options.chatting,
    allMessages: options.allMessages,
    getSession: options.getSession,
    getConversationId: options.getConversationId,
    invokeStopChatMessage: options.invokeStopChatMessage,
    getRound: () => round,
    setRound,
    advanceGeneration: () => {
      generation += 1;
    },
    setSendChatActiveGen: (gen) => {
      sendChatActiveGen = gen;
    },
    clearDeferredRoundCompletion: () => {
      deferredRoundCompletion = null;
    },
    clearPendingTerminalEvent: () => {
      pendingTerminalEvent = null;
    },
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    getActiveActivationId: () => activeActivationId,
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
    clearFrontendDispatchTimer,
    getPendingUserDraftId,
    removeMessage,
    settleStreamingAssistantMessages,
    finalizeMessage,
    updateMessageText,
    deleteSendStartedAtMs: (gen) => {
      sendStartedAtMsByGen.delete(gen);
    },
    clearConversationStreamCache,
    reasoningStartedAtMs,
    flushStreamTextBuffer: () => {
      streamingEvents.flushStreamTextBuffer();
    },
  });
  const sendRecovery = useChatFlowSendRecovery({
    chatting: options.chatting,
    submitPending,
    reasoningStartedAtMs,
    getRound: () => round,
    setRound,
    getSession: options.getSession,
    getHistoryFlushedReceivedGen: () => historyFlushedReceivedGen,
    setSendChatActiveGenIfCurrent: (gen, value) => {
      if (sendChatActiveGen === gen) sendChatActiveGen = value;
    },
    clearFrontendDispatchTimer,
    clearChatErrorText,
    setChatErrorText,
    formatRequestFailed: options.formatRequestFailed,
    getPendingUserDraftId,
    getPendingUserDraftIdForGen,
    removeMessage,
    deleteSendStartedAtMs: (gen) => {
      sendStartedAtMsByGen.delete(gen);
    },
    failQueuedRoundWithoutMessage: roundFinalizers.failQueuedRoundWithoutMessage,
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
    onReloadMessages: options.onReloadMessages,
  });
  const roundEvents = useChatFlowRoundEvents({
    chatting: options.chatting,
    allMessages: options.allMessages,
    reasoningStartedAtMs,
    getRound: () => round,
    setRound,
    getGeneration: () => generation,
    setPendingTerminalEvent: (event) => {
      pendingTerminalEvent = event;
    },
    getPendingTerminalEvent: () => pendingTerminalEvent,
    setDeferredRoundCompletion: (event) => {
      deferredRoundCompletion = event;
    },
    clearConversationStreamCache,
    clearFrontendDispatchTimer,
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    setSendChatActiveGen: (value) => {
      sendChatActiveGen = value;
    },
    sendStartedAtMsByGen,
    hasStreamingAssistantMessageInMessages,
    applyConversationStreamCacheToDisplay,
    updateQueuedAssistantMessageStatus,
    insertStreamingAssistantMessage,
    updateMessageText,
    finalizeMessage,
    failMessage,
    syncStreamBlocksToMessage,
    applyPendingTerminalEvent,
    promoteQueuedRoundToStreaming,
    finalizeDeferredRoundCompletion: roundFinalizers.finalizeDeferredRoundCompletion,
    finalizeQueuedRoundWithoutMessage: roundFinalizers.finalizeQueuedRoundWithoutMessage,
    failQueuedRoundWithoutMessage: roundFinalizers.failQueuedRoundWithoutMessage,
    enqueueStreamDelta: roundFinalizers.enqueueStreamDelta,
    setChatErrorText,
    formatRequestFailed: options.formatRequestFailed,
    onReloadMessages: options.onReloadMessages,
    optionsT: options.t,
    flushStreamTextBuffer: (gen, messageId) => streamingEvents.flushStreamTextBuffer(gen, messageId),
  });
  const sendController = useChatFlowSendController({
    chatting: options.chatting,
    submitPending,
    isConversationBusy: options.isConversationBusy,
    getConversationId: options.getConversationId,
    getSession: options.getSession,
    createSendChatDeltaChannel: channelBinding.createSendChatDeltaChannel,
    // 仅 Web 端注入：桌面端 sendChat 原生 Tauri Channel 已覆盖流式，再 bind 会双通道双写。
    // 使用 coordinated 包装版，保证与订阅槽位（subscriptionSlot）协调一致。
    bindActiveConversationStream: chatStreamNeedsFrontendBind()
      ? bindActiveConversationStream
      : undefined,
    invokeSendChatMessage: options.invokeSendChatMessage,
    onOwnUserDraftInserted: options.onOwnUserDraftInserted,
    onStreamingAssistantBubbleInserted: options.onStreamingAssistantBubbleInserted,
    t: options.t,
    getRound: () => round,
    setRound,
    setBoundDisplayGeneration: channelBinding.setBoundDisplayGeneration,
    nextGeneration: () => ++generation,
    setSendChatActiveGen: (gen) => {
      sendChatActiveGen = gen;
    },
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    setActiveRoundAgentId: (value) => {
      activeRoundAgentId = value;
    },
    setPendingTerminalEventNull: () => {
      pendingTerminalEvent = null;
    },
    sendStartedAtMsByGen,
    startFrontendDispatchTimer,
    clearFrontendDispatchTimer,
    clearConversationStreamCache,
    clearChatErrorText,
    applyPreparedSendInput: sendInput.applyPreparedSendInput,
    prepareSendInput: sendInput.prepareSendInput,
    insertUserDraft,
    resetDisplayState,
    removeMessage,
    updateQueuedAssistantMessageStatus,
    handleRoundCompleted,
    sendRecovery,
  });
  const foregroundReset = useChatFlowForegroundReset({
    latestUserText: options.latestUserText,
    latestUserImages: options.latestUserImages,
    chatting: options.chatting,
    submitPending,
    getConversationId: options.getConversationId,
    getRound: () => round,
    setRound,
    tickGeneration: () => {
      generation += 1;
    },
    setSendChatActiveGen: (value) => {
      sendChatActiveGen = value;
    },
    setActiveActivationId: (value) => {
      activeActivationId = value;
    },
    setActiveRoundAgentId: (value: string) => {
      activeRoundAgentId = String(value || "").trim();
    },
    setDeferredRoundCompletionNull: () => {
      deferredRoundCompletion = null;
    },
    setPendingTerminalEventNull: () => {
      pendingTerminalEvent = null;
    },
    resetQueuedStreamingState: () => {
      queuedStreamingState = null;
    },
    clearFrontendDispatchTimer,
    getPendingUserDraftId,
    removeMessage,
    finalizeMessage,
    clearConversationStreamCache,
    setActiveHistoryMessageCount: (value) => {
      activeHistoryMessageCount = value;
    },
    reasoningStartedAtMs,
  });

  function setRound(next: RoundState, frontendPhase?: FrontendRoundPhase) {
    round = next;
    frontendRoundPhase.value = frontendPhase ?? next.phase;
  }

  // =========================================================================
  // 工具函数（纯逻辑，无副作用）
  // =========================================================================

  function setChatErrorText(text: string, conversationId?: string | null) {
    const cid = normalizeConversationId(conversationId || (options.getConversationId ? options.getConversationId() : ""));
    if (cid && options.setConversationChatError) {
      options.setConversationChatError(cid, text);
      return;
    }
    options.chatErrorText.value = text;
  }

  function clearChatErrorText(conversationId?: string | null) {
    setChatErrorText("", conversationId);
  }

  function clearFrontendDispatchTimer() {
    frontendDispatch.clear();
  }

  function clearContextUsagePreview() {
    if (options.contextUsagePreview) {
      options.contextUsagePreview.value = null;
    }
  }

  function startFrontendDispatchTimer(gen: number, startedAtMs?: number, elapsedMs?: number) {
    frontendDispatch.start(gen, startedAtMs, elapsedMs);
  }

  function restoreFrontendDispatchTimerFromCache(cache: ConversationStreamCache) {
    const gen = round.phase === "queued" || round.phase === "streaming" ? round.gen : 0;
    frontendDispatch.restoreFromCache(cache, gen);
  }

  function clearRecentlyCompletedRoundIds() {
    recentlyCompletedActivationId = "";
    recentlyCompletedRequestId = "";
  }

  function hasRecentlyCompletedRoundIds() {
    return !!(recentlyCompletedActivationId || recentlyCompletedRequestId);
  }

  function markRecentlyCompletedRoundIds(payload: { activationId?: string; requestId?: string } | null | undefined) {
    recentlyCompletedActivationId = String(payload?.activationId || payload?.requestId || "").trim();
    recentlyCompletedRequestId = String(payload?.requestId || payload?.activationId || "").trim();
  }

  function matchesRecentlyCompletedRoundIds(payload: { activationId?: string; requestId?: string } | null | undefined): boolean {
    const payloadActivationId = String(payload?.activationId || "").trim();
    const payloadRequestId = String(payload?.requestId || "").trim();
    return !!(
      (recentlyCompletedActivationId && payloadActivationId && payloadActivationId === recentlyCompletedActivationId)
      || (recentlyCompletedActivationId && payloadRequestId && payloadRequestId === recentlyCompletedActivationId)
      || (recentlyCompletedRequestId && payloadRequestId && payloadRequestId === recentlyCompletedRequestId)
      || (recentlyCompletedRequestId && payloadActivationId && payloadActivationId === recentlyCompletedRequestId)
    );
  }

  // =========================================================================
  // 显示状态重置（只在 history_flushed 清屏时调用）
  // =========================================================================

  function resetDisplayState() {
    clearContextUsagePreview();
    foregroundReset.resetDisplayState();
  }

  function clearForegroundRoundState() {
    clearContextUsagePreview();
    foregroundReset.clearForegroundRoundState();
  }

  function clearForegroundRuntimeState() {
    clearContextUsagePreview();
    // 前台运行态被强制清除（切会话 / 对账收尾）时只清回合、不动消息列表，
    // 残留的 `_streaming` 会被「流式状态归回合所有」的合并规则保留而挂死。
    // 先冲刷平滑追赶缓冲（与停轮路径一致），避免结算后残留分片又把流式投影写回；
    // 再同语义结算一次剩余流式投影。
    streamingEvents.flushStreamTextBuffer();
    settleStreamingAssistantMessages();
    foregroundReset.clearForegroundRuntimeState();
  }

  function freezeForegroundRoundState() {
    clearContextUsagePreview();
    foregroundReset.freezeForegroundRoundState();
  }

  function beginAssistantActivationFromEvent(payload: RoundStartedPayload): number {
    const gen = foregroundRounds?.beginAssistantActivationFromEvent(payload) ?? 0;
    probeChatFlow("激活轮次返回", {
      gen,
      hasForegroundRounds: !!foregroundRounds,
      payloadConversationId: String(payload?.conversationId || ""),
      assistantMessageId: String(payload?.assistantMessageId || ""),
      currentConversationId: String(options.getConversationId ? options.getConversationId() : ""),
      roundPhase: round.phase,
    });
    return gen;
  }

  function ensureForegroundWaitingRound(statusText = options.t("chat.statusWaitingReply")) {
    return foregroundRounds?.ensureForegroundWaitingRound(statusText) ?? 0;
  }

  function ensureForegroundStreamingRound() {
    return foregroundRounds?.ensureForegroundStreamingRound() ?? 0;
  }

  function resumeForegroundRuntimeRound(input?: ResumeForegroundRuntimeRoundInput) {
    return foregroundRounds?.resumeForegroundRuntimeRound(input) ?? 0;
  }

  function resumeForegroundStreamCacheProjection(input?: { conversationId?: string | null; reason?: string }) {
    return foregroundRounds?.resumeForegroundStreamCacheProjection(input) ?? 0;
  }

  function promoteQueuedRoundToStreaming(gen: number) {
    return foregroundRounds?.promoteQueuedRoundToStreaming(gen) ?? 0;
  }

  // =========================================================================
  // 事件处理
  // =========================================================================

  /**
 * history_flushed：唯一做 allMessages 大规模合并的地方。
 * 只表达“消息已落入正式历史”，不再推进助理轮次状态。
 * 助理是否已启动由 round_started 表达。
   */
  async function handleHistoryFlushed(
    gen: number,
    parsed: AssistantDeltaEvent,
    source: "sendChat" | "bound",
  ) {
    if (source === "sendChat") {
      submitPending.value = false;
    }
    const flushed = readHistoryFlushedPayload(parsed.message);
    if (flushed && options.onHistoryFlushed) {
      await options.onHistoryFlushed({
        conversationId: flushed.conversationId,
        messageCount: flushed.messageCount,
        pendingMessages: flushed.messages,
        activateAssistant: !!flushed.activateAssistant,
      });
    }
    await roundEvents.handleHistoryFlushed(gen, parsed, source);
  }

  async function markRoundStarted(gen: number) {
    await roundEvents.markRoundStarted(gen);
  }

  /**
   * round_completed：终结当前轮次。
   * 只做文字收尾 + 状态转换，不碰 allMessages（除了 updateMessageText）。
   */
  async function handleRoundCompleted(
    gen: number,
    result: {
      assistantText: string;
      assistantMessage?: ChatMessage;
      activationId?: string;
      requestId?: string;
    },
  ) {
    await roundEvents.handleRoundCompleted(gen, result);
  }

  async function handleRoundFailed(
    gen: number,
    error: unknown,
    identity?: { activationId?: string; requestId?: string },
  ) {
    await roundEvents.handleRoundFailed(gen, error, identity);
  }

  function applyPendingTerminalEvent(gen: number) {
    return roundEvents.applyPendingTerminalEvent(gen);
  }

  // =========================================================================
  // Delta 分发
  // =========================================================================
  function handleStreamingEvent(currentGen: number, parsed: AssistantDeltaEvent) {
    streamingEvents.handleStreamingEvent(currentGen, parsed);
  }

  // =========================================================================
  // 公共方法
  // =========================================================================

  async function sendChat(overrides?: SendChatOverrides) {
    clearContextUsagePreview();
    await sendController.sendChat(overrides);
  }

  async function stopChat() {
    clearContextUsagePreview();
    await stopController.stopChat();
  }

  return {
    bindingId: channelBinding.bindingId,
    sendChat,
    stopChat,
    clearForegroundRoundState,
    clearForegroundRuntimeState,
    freezeForegroundRoundState,
    readConversationStreamCache,
    resumeForegroundStreamingRound: ensureForegroundStreamingRound,
    resumeForegroundRuntimeRound,
    resumeForegroundStreamCacheProjection,
    bindActiveConversationStream,
    unbindActiveConversationStream: channelBinding.unbindActiveConversationStream,
    hasActiveBoundDeltaChannel: channelBinding.hasActiveBoundDeltaChannel,
    probeBoundChannel: channelBinding.probeBoundChannel,
    handleExternalStreamRebindRequired: externalEvents.handleExternalStreamRebindRequired,
    handleExternalHistoryFlushed: externalEvents.handleExternalHistoryFlushed,
    handleExternalRoundStarted: externalEvents.handleExternalRoundStarted,
    handleExternalRoundCompleted: externalEvents.handleExternalRoundCompleted,
    handleExternalRoundFailed: externalEvents.handleExternalRoundFailed,
    handleExternalAssistantDelta: externalEvents.handleExternalAssistantDelta,
    handleExternalRoundFinished,
    frontendRoundPhase,
    submitPending,
    reasoningStartedAtMs,
  };
}
