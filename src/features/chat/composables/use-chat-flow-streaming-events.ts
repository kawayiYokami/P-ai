import type { Ref } from "vue";
import { normalizeAssistantStreamBlocks } from "../../../utils/chat-message-semantics";
import {
  assistantEventStartsStreaming,
  readDeltaMessage,
  readContextUsageUpdatePayload,
  readRoundCompletedPayload,
  readRoundFailedPayload,
  type AssistantDeltaEvent,
  type ContextUsageUpdatePayload,
} from "./use-chat-flow-events";
import type { ConversationRuntimeStreamCacheSnapshot } from "./use-chat-flow-stream-cache";
import type { PendingTerminalEvent, RoundState } from "./use-chat-flow-types";

type UseChatFlowStreamingEventsOptions = {
  contextUsagePreview?: Ref<ContextUsageUpdatePayload | null>;
  reasoningStartedAtMs: Ref<number>;
  getRound: () => RoundState;
  promoteQueuedRoundToStreaming: (gen: number) => number;
  setPendingTerminalEvent: (event: PendingTerminalEvent | null) => void;
  clearConversationStreamCache: (conversationId?: string | null) => void;
  getConversationId?: () => string;
  getActiveActivationId: () => string;
  setActiveActivationId: (value: string) => void;
  applyConversationStreamCacheSnapshotToDisplay: (
    conversationId: string,
    snapshot: ConversationRuntimeStreamCacheSnapshot,
  ) => boolean;
  handleRoundCompleted: (
    gen: number,
    result: {
      assistantText: string;
      assistantMessage?: any;
      activationId?: string;
      requestId?: string;
    },
  ) => Promise<void>;
  handleRoundFailed: (
    gen: number,
    error: unknown,
    identity?: { activationId?: string; requestId?: string },
  ) => Promise<void>;
  applyAssistantEventToMessage: (messageId: string, parsed: AssistantDeltaEvent) => void;
  enqueueStreamDelta: (gen: number, delta: string) => void;
};

export function streamingTerminalTargetsRound(
  round: RoundState,
  activeActivationId: string,
  input: { activationId?: string; requestId?: string; assistantMessageId?: string },
): boolean {
  if (round.phase !== "queued" && round.phase !== "streaming") return false;
  const incomingMessageId = String(input.assistantMessageId || "").trim();
  if (incomingMessageId && incomingMessageId !== round.messageId) return false;
  const currentActivationId = String(activeActivationId || "").trim();
  const incomingIds = [String(input.activationId || "").trim(), String(input.requestId || "").trim()]
    .filter(Boolean);
  if (currentActivationId && incomingIds.length > 0 && !incomingIds.includes(currentActivationId)) return false;
  return true;
}

export function useChatFlowStreamingEvents(options: UseChatFlowStreamingEventsOptions) {
  function applyStreamCacheContextUsageToPreview(parsed: AssistantDeltaEvent) {
    const cache = parsed.streamCache;
    if (!cache || typeof cache.contextUsageRatio !== "number") return;
    if (!options.contextUsagePreview) return;
    const conversationId = options.getConversationId ? options.getConversationId() : "";
    if (!conversationId) return;
    const ratio = Math.max(0, cache.contextUsageRatio);
    if (!(ratio > 0) && !(Number(cache.contextUsagePercent) > 0)) return;
    const percent = typeof cache.contextUsagePercent === "number"
      ? Math.round(cache.contextUsagePercent)
      : Math.round(ratio * 100);
    options.contextUsagePreview.value = {
      conversationId,
      contextUsagePercent: Math.min(100, Math.max(0, percent)),
      contextUsageRatio: ratio,
      effectivePromptTokens: Math.max(0, Math.round(Number(cache.effectivePromptTokens) || 0)),
      contextWindowTokens: Math.max(0, Math.round(Number(cache.contextWindowTokens) || 0)),
      source: "stream_cache",
      eventReason: "provider_tool_round",
    };
  }

  // ==================== 流式平滑追赶缓冲 ====================
  // 自适应追赶算法：按目标在 1 秒内清空剩余缓冲的速度平滑输出。
  // 缓冲少时按打字机节奏逐字流出；突发激增时自动平滑提速，保证不累积延迟。
  const STREAM_SMOOTH_CATCHUP_MS = 1000;
  type PendingOrderedItem = { kind: "text" | "reasoning"; delta: string };
  let pendingOrdered: PendingOrderedItem[] = [];
  let pendingStreamGen = 0;
  let pendingStreamMessageId = "";
  let streamFlushFrameId: number | null = null;
  let streamFlushLastAt = 0;
  let streamFlushCarryChars = 0;

  function stopStreamFlushLoop() {
    if (streamFlushFrameId !== null) {
      if (typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(streamFlushFrameId);
      } else {
        clearTimeout(streamFlushFrameId);
      }
      streamFlushFrameId = null;
    }
    streamFlushLastAt = 0;
    streamFlushCarryChars = 0;
  }

  function flushAllPendingOrdered(targetGen?: number, targetMessageId?: string) {
    const round = options.getRound();
    const gen = targetGen || pendingStreamGen || (round.phase === "streaming" || round.phase === "queued" ? round.gen : 0);
    const messageId = targetMessageId || pendingStreamMessageId || (round.phase === "streaming" || round.phase === "queued" ? round.messageId : "");
    const ordered = pendingOrdered;
    pendingOrdered = [];
    pendingStreamGen = 0;
    pendingStreamMessageId = "";
    if (ordered.length === 0) return;

    let combinedReasoning = "";
    let combinedText = "";
    for (const item of ordered) {
      if (!item.delta) continue;
      if (item.kind === "reasoning") {
        combinedReasoning += item.delta;
      } else if (item.kind === "text") {
        combinedText += item.delta;
      }
    }

    if (combinedReasoning && messageId) {
      options.applyAssistantEventToMessage(messageId, { kind: "activity_reasoning_delta", delta: combinedReasoning });
    }
    if (combinedText) {
      if (gen && round.phase === "streaming" && round.gen === gen) {
        options.enqueueStreamDelta(gen, combinedText);
      } else if (messageId) {
        options.applyAssistantEventToMessage(messageId, { delta: combinedText });
      }
    }
  }

  function flushStreamTextBuffer(targetGen?: number, targetMessageId?: string) {
    stopStreamFlushLoop();
    flushAllPendingOrdered(targetGen, targetMessageId);
  }

  function safeSliceCount(str: string, count: number): number {
    if (count >= str.length) return str.length;
    const code = str.charCodeAt(count - 1);
    if (code >= 0xD800 && code <= 0xDBFF) {
      return Math.min(str.length, count + 1);
    }
    return count;
  }

  function emitOrderedChars(count: number) {
    let remaining = count;
    const gen = pendingStreamGen;
    const messageId = pendingStreamMessageId;
    while (remaining > 0 && pendingOrdered.length > 0) {
      const first = pendingOrdered[0];
      if (!first.delta) {
        pendingOrdered.shift();
        continue;
      }
      const sliceLen = safeSliceCount(first.delta, remaining);
      if (sliceLen >= first.delta.length) {
        const chunk = first.delta;
        remaining = Math.max(0, remaining - chunk.length);
        pendingOrdered.shift();
        if (first.kind === "reasoning" && messageId) {
          options.applyAssistantEventToMessage(messageId, { kind: "activity_reasoning_delta", delta: chunk });
        } else if (first.kind === "text" && gen) {
          options.enqueueStreamDelta(gen, chunk);
        }
      } else {
        const chunk = first.delta.slice(0, sliceLen);
        first.delta = first.delta.slice(sliceLen);
        remaining = 0;
        if (first.kind === "reasoning" && messageId) {
          options.applyAssistantEventToMessage(messageId, { kind: "activity_reasoning_delta", delta: chunk });
        } else if (first.kind === "text" && gen) {
          options.enqueueStreamDelta(gen, chunk);
        }
      }
    }
    if (pendingOrdered.length === 0) {
      pendingStreamGen = 0;
      pendingStreamMessageId = "";
    }
  }

  function scheduleNextFrame() {
    if (typeof requestAnimationFrame === "function") {
      streamFlushFrameId = requestAnimationFrame(() => stepStreamSmoothFlush()) as unknown as number;
    } else {
      streamFlushFrameId = setTimeout(() => stepStreamSmoothFlush(), 16) as unknown as number;
    }
  }

  function stepStreamSmoothFlush() {
    streamFlushFrameId = null;
    if (pendingOrdered.length === 0) {
      stopStreamFlushLoop();
      return;
    }

    const now = typeof performance !== "undefined" ? performance.now() : Date.now();
    if (streamFlushLastAt <= 0) {
      streamFlushLastAt = now;
    }
    const elapsed = Math.min(100, Math.max(0, now - streamFlushLastAt));
    streamFlushLastAt = now;

    let totalBufferedChars = 0;
    for (let i = 0; i < pendingOrdered.length; i++) {
      totalBufferedChars += pendingOrdered[i].delta.length;
    }

    if (totalBufferedChars === 0) {
      pendingOrdered = [];
      stopStreamFlushLoop();
      return;
    }

    // 基础速度：缓冲内字符在 1 秒内清空
    // 当字数较少时提供保底打字速度（25 字/秒，即每字约 40ms 自然打字节奏），避免短句拖沓
    const calculatedChars = (totalBufferedChars * elapsed) / STREAM_SMOOTH_CATCHUP_MS;
    const floorChars = (25 * elapsed) / 1000;
    streamFlushCarryChars += Math.max(calculatedChars, floorChars);
    const emitCount = Math.floor(streamFlushCarryChars);

    if (emitCount > 0) {
      streamFlushCarryChars = Math.max(0, streamFlushCarryChars - emitCount);
      emitOrderedChars(emitCount);
    }

    if (pendingOrdered.length === 0) {
      stopStreamFlushLoop();
      return;
    }

    scheduleNextFrame();
  }

  function ensureStreamFlushLoop() {
    if (streamFlushFrameId !== null || pendingOrdered.length === 0) return;
    streamFlushLastAt = 0;
    // 首字立即响应：当队列从空转有内容时，赋予初始 1.0 carry，首字零延迟上屏
    streamFlushCarryChars = 1;
    scheduleNextFrame();
  }

  function bufferStreamText(input: { gen: number; messageId: string; text?: string; reasoning?: string }) {
    const genMismatched = !!pendingStreamGen && pendingStreamGen !== input.gen;
    const idMismatched = !!pendingStreamMessageId && pendingStreamMessageId !== input.messageId;
    if ((genMismatched || idMismatched) && pendingOrdered.length > 0) {
      flushStreamTextBuffer();
    }
    if (input.reasoning) {
      const last = pendingOrdered[pendingOrdered.length - 1];
      if (last?.kind === "reasoning") last.delta += input.reasoning;
      else pendingOrdered.push({ kind: "reasoning", delta: input.reasoning });
    }
    if (input.text) {
      const last = pendingOrdered[pendingOrdered.length - 1];
      if (last?.kind === "text") last.delta += input.text;
      else pendingOrdered.push({ kind: "text", delta: input.text });
    }
    if (!pendingStreamGen) pendingStreamGen = input.gen;
    if (!pendingStreamMessageId) pendingStreamMessageId = input.messageId;
    if (pendingOrdered.length > 0) ensureStreamFlushLoop();
  }

  function handleStreamingEvent(currentGen: number, parsed: AssistantDeltaEvent) {
    if (parsed.kind === "round_completed" || parsed.kind === "round_failed") {
      // 终态事件到达时先冲刷文本缓冲，避免最后一段正文/思维链丢失。
      const currentRound = options.getRound();
      const messageId = "messageId" in currentRound ? currentRound.messageId : undefined;
      flushStreamTextBuffer(currentGen, messageId);
    }
    if (parsed.kind === "context_usage_update") {
      const p = readContextUsageUpdatePayload(parsed.message);
      const activeConversationId = options.getConversationId ? options.getConversationId() : "";
      if (p && (!activeConversationId || p.conversationId === activeConversationId)) {
        if (options.contextUsagePreview) {
          options.contextUsagePreview.value = p;
        }
      }
      return;
    }
    // 工具执行期间用量随流式缓存下发：直接更新预览，无需旁路广播。
    applyStreamCacheContextUsageToPreview(parsed);
    const round = options.getRound();
    if (
      !currentGen
      || (round.phase !== "queued" && round.phase !== "streaming")
      || round.gen !== currentGen
    ) {
      return;
    }
    if (parsed.kind === "tool_status") {
      // tool_status 是调度信号（准备上下文/等待回应/执行工具），只原地刷新
      // 等待气泡的 _toolStatusText/_schedulingState，不触发 queued -> streaming。
      // 只有正文/思维链/工具事件才算流式开始。
    }
    if (round.phase === "queued" && round.gen === currentGen && assistantEventStartsStreaming(parsed)) {
      options.promoteQueuedRoundToStreaming(currentGen);
    }
    const currentRound = options.getRound();
    if (currentRound.phase !== "streaming" && currentRound.phase !== "queued") {
      return;
    }
    if (currentRound.gen !== currentGen) {
      return;
    }
    if (parsed.kind === "round_completed") {
      const p = readRoundCompletedPayload(parsed.message);
      const identity = {
        activationId: p?.activationId || parsed.activationId,
        requestId: p?.requestId || parsed.requestId,
        assistantMessageId: p?.assistantMessage?.id,
      };
      if (!streamingTerminalTargetsRound(
        currentRound,
        options.getActiveActivationId(),
        identity,
      )) return;
      const result = {
        assistantText: String(p?.assistantText || ""),
        assistantMessage: p?.assistantMessage,
        activationId: identity.activationId,
        requestId: identity.requestId,
        ...(currentRound.phase === "queued"
          && parsed.reason === "context_compaction_boundary"
          && !String(p?.assistantText || "").trim()
          && !p?.assistantMessage
          ? { skipCanonicalReadback: true }
          : {}),
      };
      if (currentRound.phase === "queued" && parsed.reason === "context_compaction_boundary") {
        void options.handleRoundCompleted(currentGen, result);
        return;
      }
      if (currentRound.phase === "queued") {
        options.setPendingTerminalEvent({
          kind: "completed",
          gen: currentGen,
          result,
        });
        options.clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
        options.setActiveActivationId("");
        return;
      }
      void options.handleRoundCompleted(currentGen, result);
      return;
    }

    if (parsed.kind === "round_failed") {
      const p = readRoundFailedPayload(parsed.message);
      const identity = {
        activationId: p?.activationId || parsed.activationId,
        requestId: p?.requestId || parsed.requestId,
      };
      if (!streamingTerminalTargetsRound(
        currentRound,
        options.getActiveActivationId(),
        identity,
      )) return;
      if (options.contextUsagePreview) {
        options.contextUsagePreview.value = null;
      }
      const error = p?.error || parsed.message || JSON.stringify(parsed);
      if (currentRound.phase === "queued") {
        options.setPendingTerminalEvent({
          kind: "failed",
          gen: currentGen,
          error,
          activationId: identity.activationId,
          requestId: identity.requestId,
        });
        options.clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
        options.setActiveActivationId("");
        return;
      }
      void options.handleRoundFailed(currentGen, error, {
        activationId: identity.activationId,
        requestId: identity.requestId,
      });
      return;
    }

    const conversationId = options.getConversationId ? options.getConversationId() : "";
    const delta = readDeltaMessage(parsed);
    const isActivityProjectionEvent =
      parsed.kind === "activity_reasoning_delta"
      || parsed.kind === "assistant_tool_event"
      || parsed.kind === "assistant_tool_result";
    let receivedCanonicalSnapshot = false;
    if (conversationId && parsed.streamCache) {
      const streamCacheMessageId = String(parsed.streamCache.persistedAssistantMessageId || "").trim();
      if (streamCacheMessageId && currentRound.messageId && streamCacheMessageId !== currentRound.messageId) {
        return;
      }
      const snapshotBlocks = normalizeAssistantStreamBlocks(parsed.streamCache.streamBlocks);
      const snapshotHasVisibleProgress = !!(
        String(parsed.streamCache.assistantText || "").trim()
        || String(parsed.streamCache.toolStatusText || "").trim()
        || String(parsed.streamCache.toolStatusState || "").trim()
        || snapshotBlocks.length > 0
      );
      if (currentRound.phase === "streaming" && snapshotHasVisibleProgress) {
        // 权威快照到达前先冲刷缓冲，避免旧增量文本追加到快照状态之后造成错乱。
        flushStreamTextBuffer();
        options.applyConversationStreamCacheSnapshotToDisplay(conversationId, parsed.streamCache);
        options.applyAssistantEventToMessage(currentRound.messageId, parsed);
        receivedCanonicalSnapshot = true;
      }
    }

    if (parsed.kind === "tool_status") {
      // 工具状态即时处理（先冲刷缓冲保持事件顺序），不参与正文节流。
      // queued 下也要原地应用：等待回应文案靠它刷新 schedulingState，
      // 应用本身不提升阶段（状态机 tool_status 分支保 phase）。
      flushStreamTextBuffer();
      if ((currentRound.phase === "streaming" || currentRound.phase === "queued") && !receivedCanonicalSnapshot) {
        options.applyAssistantEventToMessage(currentRound.messageId, parsed);
      }
    }

    if (isActivityProjectionEvent) {
      if (delta && options.reasoningStartedAtMs.value === 0) options.reasoningStartedAtMs.value = Date.now();
      if (parsed.kind === "activity_reasoning_delta" && delta) {
        // 思维链文本与正文一起平滑追赶输出。
        if (currentRound.phase === "streaming" && !receivedCanonicalSnapshot) {
          bufferStreamText({ gen: currentGen, messageId: currentRound.messageId, reasoning: delta });
        }
      } else {
        // 工具事件即时处理（先冲刷缓冲保持顺序）。
        flushStreamTextBuffer();
        if (currentRound.phase === "streaming" && !receivedCanonicalSnapshot) {
          options.applyAssistantEventToMessage(currentRound.messageId, parsed);
        }
      }
    }

    if (parsed.kind === "tool_status" || isActivityProjectionEvent || receivedCanonicalSnapshot) {
      return;
    }

    if (currentRound.phase === "streaming") {
      bufferStreamText({ gen: currentGen, messageId: currentRound.messageId, text: delta });
    }
  }

  return {
    handleStreamingEvent,
    flushStreamTextBuffer,
  };
}
