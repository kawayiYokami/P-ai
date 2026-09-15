import type { ChatMessage } from "../../../types/app";
import { normalizeAssistantStreamBlocks } from "../../../utils/chat-message-semantics";
import { streamCacheHasVisibleProgress } from "./use-chat-flow-stream-cache";
import { applyStreamingHistoryOverlay } from "./use-chat-flow-stream-overlay";
import { formalizeMessages, normalizeConversationId, positiveRoundedNumber } from "./use-chat-flow-utils";
import type { ResumeForegroundRuntimeRoundInput } from "./use-chat-flow-types";
import type { RoundStartedPayload } from "./use-chat-flow-events";

type ResumeForegroundStreamCacheProjectionInput = {
  conversationId?: string | null;
  reason?: string;
};

export function useChatFlowForegroundRounds(bindings: Record<string, any>) {
  function isStreamingAssistantMessage(message?: ChatMessage | null): boolean {
    const messageId = String(message?.id || "").trim();
    const meta = (message?.providerMeta || {}) as Record<string, unknown>;
    return String(message?.role || "").trim() === "assistant" && meta._streaming === true;
  }

  function messageIdOf(message?: ChatMessage | null): string {
    return String(message?.id || "").trim();
  }

  function findStreamingAssistantMessageById(messageId: string): ChatMessage | null {
    const normalizedMessageId = String(messageId || "").trim();
    if (!normalizedMessageId) return null;
    return [...bindings.allMessages.value]
      .reverse()
      .find((message: ChatMessage) =>
        messageIdOf(message) === normalizedMessageId && isStreamingAssistantMessage(message)
      ) || null;
  }

  function streamCacheMatchesMessageId(cache: unknown, messageId: string): boolean {
    const normalizedMessageId = String(messageId || "").trim();
    if (!normalizedMessageId) return false;
    const cacheMessageId = String((cache as { persistedAssistantMessageId?: unknown } | null | undefined)?.persistedAssistantMessageId || "").trim();
    return !cacheMessageId || cacheMessageId === normalizedMessageId;
  }

  function resolveAssistantMessageId(input?: {
    messageId?: string | null;
    assistantMessageId?: string | null;
    persistedAssistantMessageId?: string | null;
    activationId?: string | null;
    requestId?: string | null;
    gen?: number;
  }): string {
    const explicitMessageId = String(input?.messageId || "").trim();
    if (explicitMessageId) return explicitMessageId;
    const assistantMessageId = String(input?.assistantMessageId || input?.persistedAssistantMessageId || "").trim();
    if (assistantMessageId) return assistantMessageId;
    return "";
  }

  function applyStreamingOverlayForConversation(conversationId?: string | null, expectedMessageId = "") {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return;
    const cache = bindings.readConversationStreamCache(cid);
    if (expectedMessageId && !streamCacheMatchesMessageId(cache, expectedMessageId)) return;
    const overlay = applyStreamingHistoryOverlay(
      bindings.allMessages.value,
      cache,
    );
    if (!overlay.removed) return;
    bindings.allMessages.value = overlay.messages;
    // 流式恢复日志已移除
  }

  function applyQueuedStreamingStateIfNeeded(messageId: string) {
    const queuedStreamingState = bindings.getQueuedStreamingState();
    if (!queuedStreamingState) return;
    if (queuedStreamingState.frontendDispatchStartedAtMs || queuedStreamingState.frontendDispatchElapsedMs) {
      const round = bindings.getRound();
      bindings.startFrontendDispatchTimer(
        round.phase === "queued" || round.phase === "streaming" ? round.gen : 0,
        queuedStreamingState.frontendDispatchStartedAtMs,
        queuedStreamingState.frontendDispatchElapsedMs,
      );
    }
    bindings.setQueuedStreamingState(null);
    bindings.updateMessageText(messageId, queuedStreamingState.streamBlocks || [], undefined, {
      toolStatusText: queuedStreamingState.toolStatusText,
      toolStatusState: queuedStreamingState.toolStatusState,
    });
  }

  function beginAssistantActivationFromEvent(payload: RoundStartedPayload): number {
    const payloadConversationId = normalizeConversationId(payload.conversationId);
    const currentConversationId = normalizeConversationId(bindings.getConversationId ? bindings.getConversationId() : "");
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      return 0;
    }
    const nextActivationId = String(payload.activationId || payload.requestId || "").trim();
    const cid = payloadConversationId || currentConversationId;
    const round = bindings.getRound();
    if (bindings.getActiveActivationId() && nextActivationId && bindings.getActiveActivationId() === nextActivationId && round.phase !== "idle") {
      return round.gen;
    }
    if (cid) bindings.clearConversationStreamCache(cid);
    bindings.setActiveActivationId(nextActivationId);
    const canonicalMessageId = String(payload.assistantMessageId || "").trim();
    if (round.phase === "queued" && !round.messageId && canonicalMessageId) {
      bindings.setRound({ phase: "queued", gen: round.gen, messageId: canonicalMessageId }, "waiting");
    }
    if (cid && positiveRoundedNumber(payload.startedAtMs)) {
      bindings.writeConversationStreamCacheSnapshot(cid, {
        activationId: nextActivationId,
        requestId: String(payload.requestId || nextActivationId || "").trim(),
        agentId: String(payload.agentId || "").trim(),
        startedAt: String(payload.startedAt || "").trim(),
        startedAtMs: positiveRoundedNumber(payload.startedAtMs),
        persistedAssistantMessageId: String(payload.assistantMessageId || "").trim(),
      });
    }
    const payloadAgentId = String(payload.agentId || "").trim();
    if (payloadAgentId) bindings.setActiveRoundAgentId?.(payloadAgentId);
    let gen = round.phase === "queued" ? round.gen : bindings.getSendChatActiveGen();
    // 队列出队时上一轮已回到 idle，但 sendChatActiveGen 仍是出队消息的 gen（truthy），
    // 旧条件只在 !gen 时建轮次，导致 round 永远停在 idle，后续 delta 因身份校验被丢弃、气泡刷不出。
    // idle 时即使已有 gen 也必须重建 queued 占位；streaming/同 gen queued 保持不动。
    if (!gen || round.phase === "idle") {
      if (!gen) {
        gen = bindings.nextGeneration();
      }
      bindings.channelBinding.setBoundDisplayGeneration(gen);
      bindings.setPendingTerminalEvent(null);
      bindings.setDeferredRoundCompletion(null);
      bindings.setQueuedStreamingState(null);
      bindings.resetDisplayState();
      bindings.setActiveHistoryMessageCount(formalizeMessages(bindings.allMessages.value).length);
      bindings.setRound({
        phase: "queued",
        gen,
        messageId: resolveAssistantMessageId({
          messageId: round.phase === "queued" ? round.messageId : "",
          assistantMessageId: payload.assistantMessageId,
          activationId: nextActivationId,
          requestId: payload.requestId,
          gen,
        }),
      }, "waiting");
    }
    bindings.startFrontendDispatchTimer(
      gen,
      positiveRoundedNumber(payload.startedAtMs) || bindings.sendStartedAtMsByGen.get(gen),
    );
    bindings.chatting.value = true;
    bindings.updateQueuedAssistantMessageStatus(
      resolveAssistantMessageId({
        messageId: bindings.getRound().phase === "queued" ? bindings.getRound().messageId : "",
        assistantMessageId: payload.assistantMessageId,
        activationId: nextActivationId,
        requestId: payload.requestId,
        gen,
      }),
      bindings.t("chat.statusWaitingReply"),
      payload,
    );
    bindings.setFrontendRoundPhase("waiting");
    return gen;
  }

  function cachedDispatchTimerForConversation(): { startedAtMs: number; elapsedMs: number } {
    const conversationId = normalizeConversationId(bindings.getConversationId ? bindings.getConversationId() : "");
    const cache = conversationId ? bindings.readConversationStreamCache(conversationId) : null;
    return {
      startedAtMs: positiveRoundedNumber(cache?.frontendDispatchStartedAtMs || cache?.startedAtMs),
      elapsedMs: positiveRoundedNumber(cache?.frontendDispatchElapsedMs),
    };
  }

  function ensureForegroundWaitingRound(statusText = bindings.t("chat.statusWaitingReply")) {
    const round = bindings.getRound();
    const cachedTimer = cachedDispatchTimerForConversation();
    const conversationId = normalizeConversationId(bindings.getConversationId ? bindings.getConversationId() : "");
    if (round.phase === "queued") {
      bindings.startFrontendDispatchTimer(
        round.gen,
        bindings.frontendDispatch.getStartedAtMs() || cachedTimer.startedAtMs || undefined,
        bindings.frontendDispatch.getElapsedMs() || cachedTimer.elapsedMs,
      );
      bindings.updateQueuedAssistantMessageStatus(round.messageId, statusText);
      bindings.chatting.value = true;
      bindings.setFrontendRoundPhase("waiting");
      return round.gen;
    }
    if (round.phase === "streaming") {
      bindings.startFrontendDispatchTimer(
        round.gen,
        bindings.frontendDispatch.getStartedAtMs() || cachedTimer.startedAtMs || undefined,
        bindings.frontendDispatch.getElapsedMs() || cachedTimer.elapsedMs,
      );
      if (!bindings.hasStreamingAssistantMessageInMessages()) {
        const messageId = bindings.insertStreamingAssistantMessage(round.messageId, round.gen, statusText);
        bindings.updateMessageText(messageId);
        bindings.setRound({ phase: "streaming", gen: round.gen, messageId });
      }
      bindings.chatting.value = true;
      return round.gen;
    }
    const gen = bindings.nextGeneration();
    bindings.channelBinding.setBoundDisplayGeneration(gen);
    bindings.setPendingTerminalEvent(null);
    bindings.setDeferredRoundCompletion(null);
    bindings.setQueuedStreamingState(null);
    bindings.setActiveHistoryMessageCount(formalizeMessages(bindings.allMessages.value).length);
    const messageId = resolveAssistantMessageId({
      persistedAssistantMessageId: conversationId ? bindings.readConversationStreamCache(conversationId)?.persistedAssistantMessageId : "",
      activationId: cachedTimer.startedAtMs > 0 ? bindings.getActiveActivationId?.() : "",
      requestId: conversationId ? bindings.readConversationStreamCache(conversationId)?.requestId : "",
      gen,
    });
    bindings.setRound({ phase: "queued", gen, messageId }, "waiting");
    bindings.startFrontendDispatchTimer(gen, cachedTimer.startedAtMs || undefined, cachedTimer.elapsedMs);
    bindings.chatting.value = true;
    bindings.updateQueuedAssistantMessageStatus(messageId, statusText);
    return gen;
  }

  function ensureForegroundStreamingRound() {
    const conversationId = bindings.getConversationId ? bindings.getConversationId() : "";
    const round = bindings.getRound();
    const cache = conversationId ? bindings.readConversationStreamCache(conversationId) : null;
    if (round.phase === "streaming") {
      const targetMessageId = round.messageId;
      const existingMessage = findStreamingAssistantMessageById(targetMessageId);
      if (!existingMessage) {
        const restoredFromCache = !!(targetMessageId
          && streamCacheMatchesMessageId(cache, targetMessageId)
          && bindings.applyConversationStreamCacheToDisplay(conversationId));
        const messageId = bindings.insertStreamingAssistantMessage(targetMessageId, round.gen);
        bindings.updateMessageText(messageId);
        bindings.setRound({ phase: "streaming", gen: round.gen, messageId });
      } else if (streamCacheMatchesMessageId(cache, targetMessageId)) {
        // 消息已存在且缓存快照匹配：应用缓存最新内容（正文/推理/工具块），
        // 不传 runtimeStatus，保留消息上已有的 toolStatus 真值。
        bindings.updateMessageText(targetMessageId, normalizeAssistantStreamBlocks(cache?.streamBlocks));
      } else {
        // 缓存与目标消息不匹配：仅同步一次消息自身已有内容，避免串用其他消息快照。
        bindings.updateMessageText(targetMessageId);
      }
      return round.gen;
    }
    const targetMessageId = round.phase === "queued"
      ? round.messageId
      : String(cache?.persistedAssistantMessageId || "").trim();
    if (!targetMessageId) return 0;
    const gen = bindings.nextGeneration();
    const existingMessage = findStreamingAssistantMessageById(targetMessageId);
    const existingMessageId = messageIdOf(existingMessage);
    const existingMessageMeta = ((existingMessage?.providerMeta || {}) as Record<string, unknown>);
    const restoredFromCache = !!(!existingMessageId
      && targetMessageId
      && streamCacheMatchesMessageId(cache, targetMessageId)
      && bindings.applyConversationStreamCacheToDisplay(conversationId));
    applyStreamingOverlayForConversation(conversationId, targetMessageId);
    const existingMessageStartedAtMs = existingMessageId ? positiveRoundedNumber(existingMessageMeta._frontendDispatchStartedAtMs) : 0;
    const existingMessageElapsedMs = existingMessageId ? positiveRoundedNumber(existingMessageMeta._frontendDispatchElapsedMs) : 0;
    bindings.setActiveHistoryMessageCount(formalizeMessages(bindings.allMessages.value).length);
    const messageId = existingMessageId || bindings.insertStreamingAssistantMessage(targetMessageId, gen);
    if (existingMessageId || restoredFromCache) {
      bindings.updateMessageText(messageId);
    }
    bindings.setRound({ phase: "streaming", gen, messageId });
    bindings.startFrontendDispatchTimer(
      gen,
      existingMessageStartedAtMs || bindings.frontendDispatch.getStartedAtMs() || undefined,
      existingMessageElapsedMs || bindings.frontendDispatch.getElapsedMs(),
    );
    bindings.chatting.value = true;
    applyQueuedStreamingStateIfNeeded(messageId);
    return gen;
  }

  function resumeForegroundRuntimeRound(input?: ResumeForegroundRuntimeRoundInput) {
    const conversationId = normalizeConversationId(input?.conversationId || (bindings.getConversationId ? bindings.getConversationId() : ""));
    if (!conversationId) return 0;
    const snapshotBlocks = normalizeAssistantStreamBlocks(input?.streamCache?.streamBlocks || []);
    if (input?.streamCache) {
      bindings.writeConversationStreamCacheSnapshot(conversationId, input.streamCache);
      // 切屏恢复时流式缓存里带的最新用量直接落到预览，圆环无需等待后续事件。
      const cacheUsage = input.streamCache;
      if (
        bindings.contextUsagePreview
        && typeof cacheUsage?.contextUsageRatio === "number"
        && (cacheUsage.contextUsageRatio > 0 || Number(cacheUsage.contextUsagePercent) > 0)
      ) {
        const ratio = Math.max(0, cacheUsage.contextUsageRatio);
        const percent = typeof cacheUsage.contextUsagePercent === "number"
          ? Math.round(cacheUsage.contextUsagePercent)
          : Math.round(ratio * 100);
        bindings.contextUsagePreview.value = {
          conversationId,
          contextUsagePercent: Math.min(100, Math.max(0, percent)),
          contextUsageRatio: ratio,
          effectivePromptTokens: Math.max(0, Math.round(Number(cacheUsage.effectivePromptTokens) || 0)),
          contextWindowTokens: Math.max(0, Math.round(Number(cacheUsage.contextWindowTokens) || 0)),
          source: "stream_cache",
          eventReason: "foreground_resume",
        };
      }
    }
    const cache = bindings.readConversationStreamCache(conversationId);
    const round = bindings.getRound();
    const expectedMessageId = round.phase === "queued" || round.phase === "streaming"
      ? round.messageId
      : String(cache?.persistedAssistantMessageId || "").trim();
    applyStreamingOverlayForConversation(conversationId, expectedMessageId);
    const hasVisibleProgress =
      !!input?.streamCache?.hasVisibleProgress
      || streamCacheHasVisibleProgress(input?.streamCache)
      || streamCacheHasVisibleProgress(cache);
    // 应用后端运行态快照日志已移除
    if (!hasVisibleProgress) {
      return ensureForegroundWaitingRound(input?.statusText || bindings.t("chat.statusWaitingReply"));
    }
    const gen = ensureForegroundStreamingRound();
    const nextRound = bindings.getRound();
    if (nextRound.phase === "streaming") {
      const blocks = snapshotBlocks.length > 0 ? snapshotBlocks : normalizeAssistantStreamBlocks(cache?.streamBlocks || []);
      // toolStatus 优先用已解析缓存（cache 可能已被后续事件更新），且只在有真值时携带，
      // 避免空值覆盖消息上已有的调度阶段状态。
      const toolStatusText = String(cache?.toolStatusText || "").trim();
      const toolStatusState = String(cache?.toolStatusState || "").trim();
      bindings.syncStreamBlocksToMessage(
        nextRound.messageId,
        blocks,
        toolStatusText || toolStatusState
          ? { toolStatusText, toolStatusState }
          : undefined,
      );
      bindings.updateMessageText(nextRound.messageId, blocks);
    }
    return gen;
  }

  function resumeForegroundStreamCacheProjection(input?: ResumeForegroundStreamCacheProjectionInput) {
    const currentConversationId = normalizeConversationId(bindings.getConversationId ? bindings.getConversationId() : "");
    const conversationId = normalizeConversationId(input?.conversationId || currentConversationId);
    if (!conversationId || conversationId !== currentConversationId) return 0;
    const cache = bindings.readConversationStreamCache(conversationId);
    if (!streamCacheHasVisibleProgress(cache)) return 0;
    // 从前端缓存恢复当前会话投影日志已移除
    return ensureForegroundStreamingRound();
  }

  function promoteQueuedRoundToStreaming(gen: number) {
    const round = bindings.getRound();
    if (round.phase === "streaming" && round.gen === gen) {
      return gen;
    }
    if (round.phase !== "queued" || round.gen !== gen) {
      return 0;
    }
    const conversationId = bindings.getConversationId ? bindings.getConversationId() : "";
    const cache = conversationId ? bindings.readConversationStreamCache(conversationId) : null;
    const targetMessageId = round.messageId;
    const existingMessage = findStreamingAssistantMessageById(targetMessageId);
    const existingMessageId = messageIdOf(existingMessage);
    const existingMessageMeta = ((existingMessage?.providerMeta || {}) as Record<string, unknown>);
    const restoredFromCache = !!(!existingMessageId
      && targetMessageId
      && streamCacheMatchesMessageId(cache, targetMessageId)
      && bindings.applyConversationStreamCacheToDisplay(conversationId));
    applyStreamingOverlayForConversation(conversationId, targetMessageId);
    const existingMessageStartedAtMs = existingMessageId ? positiveRoundedNumber(existingMessageMeta._frontendDispatchStartedAtMs) : 0;
    const existingMessageElapsedMs = existingMessageId ? positiveRoundedNumber(existingMessageMeta._frontendDispatchElapsedMs) : 0;
    bindings.setActiveHistoryMessageCount(formalizeMessages(bindings.allMessages.value).length);
    const messageId = existingMessageId || bindings.insertStreamingAssistantMessage(targetMessageId, gen);
    if (existingMessageId || restoredFromCache) {
      bindings.updateMessageText(messageId);
    }
    bindings.setRound({ phase: "streaming", gen, messageId });
    bindings.startFrontendDispatchTimer(
      gen,
      existingMessageStartedAtMs || bindings.frontendDispatch.getStartedAtMs() || undefined,
      existingMessageElapsedMs || bindings.frontendDispatch.getElapsedMs(),
    );
    bindings.chatting.value = true;
    applyQueuedStreamingStateIfNeeded(messageId);
    bindings.applyPendingTerminalEvent(gen);
    return gen;
  }

  return {
    beginAssistantActivationFromEvent,
    ensureForegroundWaitingRound,
    ensureForegroundStreamingRound,
    resumeForegroundRuntimeRound,
    resumeForegroundStreamCacheProjection,
    promoteQueuedRoundToStreaming,
  };
}
