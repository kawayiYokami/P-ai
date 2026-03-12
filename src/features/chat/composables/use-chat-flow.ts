import { Channel } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { ChatMessage } from "../../../types/app";

export type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
  toolName?: string;
  toolStatus?: string;
  toolArgs?: string;
  message?: string;
};

type HistoryFlushedPayload = {
  conversationId: string;
  messageCount: number;
  messages: ChatMessage[];
  activateAssistant?: boolean;
};

type RoundCompletedPayload = {
  conversationId: string;
  assistantText: string;
  reasoningStandard?: string;
  reasoningInline?: string;
  archivedBeforeSend?: boolean;
  assistantMessage?: ChatMessage;
};

type RoundFailedPayload = {
  error: string;
};

type UseChatFlowOptions = {
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  getSession: () => { apiConfigId: string; agentId: string } | null;
  getConversationId?: () => string;
  chatInput: Ref<string>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  latestAssistantText: Ref<string>;
  latestReasoningStandardText: Ref<string>;
  latestReasoningInlineText: Ref<string>;
  toolStatusText: Ref<string>;
  toolStatusState: Ref<"running" | "done" | "failed" | "">;
  streamToolCalls: Ref<Array<{ name: string; argsText: string }>>;
  chatErrorText: Ref<string>;
  allMessages: Ref<ChatMessage[]>;
  visibleMessageBlockCount: Ref<number>;
  t: (key: string, params?: Record<string, unknown>) => string;
  formatRequestFailed: (error: unknown) => string;
  removeBinaryPlaceholders: (text: string) => string;
  invokeSendChatMessage: (input: {
    text: string;
    images: Array<{ mime: string; bytesBase64: string }>;
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    onDelta: Channel<AssistantDeltaEvent>;
  }) => Promise<{
    assistantText: string;
    latestUserText: string;
    reasoningStandard?: string;
    reasoningInline?: string;
    archivedBeforeSend: boolean;
    assistantMessage?: ChatMessage;
  }>;
  invokeStopChatMessage?: (input: {
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    partialAssistantText: string;
    partialReasoningStandard: string;
    partialReasoningInline: string;
  }) => Promise<void>;
  invokeBindActiveChatViewStream?: (input: {
    conversationId?: string;
    onDelta: Channel<AssistantDeltaEvent>;
  }) => Promise<void>;
  onReloadMessages: () => Promise<void>;
  onHistoryFlushed?: (input: {
    conversationId: string;
    messageCount: number;
    pendingMessages: ChatMessage[];
  }) => Promise<void>;
};

const STREAM_FLUSH_INTERVAL_MS = 33;
const STREAM_DRAIN_TARGET_MS = 1000;

export function useChatFlow(options: UseChatFlowOptions) {
  // requestGeneration: 每次 send/stop 都会递增，用于区分不同请求实例。
  // activeDisplayGeneration: 当前“前台可见轮次”的代号。
  //
  // 关键规则：
  // 1. 新消息可以在主助理流式期间继续入队；
  // 2. 入队请求不能抢占当前前台轮次，也不能清空当前流式显示；
  // 3. 只有在收到 history_flushed 之后，才能把这批消息视为“正式进入历史”；
  // 4. 也只有在 history_flushed 之后，前端才允许切换到新的前台轮次。
  //
  // 这套规则保证了：
  // - 队列是入口层
  // - 历史是唯一生效层
  // - 主助理永远只有一个前台流式轮次
  // 从而为未来的跨进程、多来源消息汇流保留稳定的状态边界。
  let requestGeneration = 0;
  let activeDisplayGeneration = 0;
  let streamPendingText = "";
  let streamDrainDeadline = 0;
  let streamFlushTimer: ReturnType<typeof setInterval> | null = null;
  let streamToolCallCount = 0;
  let streamLastToolName = "";
  let activeHistoryMessageCount = 0;
  const reasoningStartedAtMs = ref(0);

  function summarizeToolCallsText(): string {
    if (streamToolCallCount <= 0) return "";
    const extraCount = Math.max(0, streamToolCallCount - 1);
    return extraCount > 0
      ? `调用 ${streamLastToolName || "-"} (+${extraCount})`
      : `调用 ${streamLastToolName || "-"}`;
  }

  function readDeltaMessage(message: unknown): string {
    if (typeof message === "string") return message;
    if (message && typeof message === "object" && "delta" in message) {
      const value = (message as { delta?: unknown }).delta;
      return typeof value === "string" ? value : "";
    }
    return "";
  }

  function readAssistantEvent(message: unknown): AssistantDeltaEvent {
    if (!message || typeof message !== "object") return {};
    const m = message as Record<string, unknown>;
    return {
      delta: typeof m.delta === "string" ? m.delta : undefined,
      kind: typeof m.kind === "string" ? m.kind : undefined,
      toolName: typeof m.toolName === "string" ? m.toolName : undefined,
      toolStatus: typeof m.toolStatus === "string" ? m.toolStatus : undefined,
      toolArgs: typeof m.toolArgs === "string" ? m.toolArgs : undefined,
      message: typeof m.message === "string" ? m.message : undefined,
    };
  }

  function clearStreamBuffer() {
    streamPendingText = "";
    streamDrainDeadline = 0;
    if (streamFlushTimer) {
      clearInterval(streamFlushTimer);
      streamFlushTimer = null;
    }
  }

  function resetDisplayedRoundState() {
    clearStreamBuffer();
    streamToolCallCount = 0;
    streamLastToolName = "";
    options.latestUserText.value = "";
    options.latestUserImages.value = [];
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
    options.streamToolCalls.value = [];
  }

  function hasAssistantVisibleOutput(result: {
    assistantText: string;
    reasoningStandard?: string;
    reasoningInline?: string;
  }): boolean {
    return (
      !!result.assistantText.trim() ||
      !!(result.reasoningStandard || "").trim() ||
      !!(result.reasoningInline || "").trim()
    );
  }

  function readHistoryFlushedPayload(
    raw: string | undefined,
  ): HistoryFlushedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      const conversationId = String(parsed.conversationId || "").trim();
      const messageCount = Math.max(
        0,
        Math.round(Number(parsed.messageCount) || 0),
      );
      return {
        conversationId,
        messageCount,
        messages: Array.isArray(parsed.messages) ? (parsed.messages as ChatMessage[]) : [],
        activateAssistant: !!parsed.activateAssistant,
      };
    } catch {
      return {
        conversationId: text,
        messageCount: 0,
        messages: [],
        activateAssistant: false,
      };
    }
  }

  function readRoundCompletedPayload(
    raw: string | undefined,
  ): RoundCompletedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      return {
        conversationId: String(parsed.conversationId || "").trim(),
        assistantText: String(parsed.assistantText || ""),
        reasoningStandard:
          typeof parsed.reasoningStandard === "string" ? parsed.reasoningStandard : undefined,
        reasoningInline:
          typeof parsed.reasoningInline === "string" ? parsed.reasoningInline : undefined,
        archivedBeforeSend: !!parsed.archivedBeforeSend,
        assistantMessage: (parsed.assistantMessage as ChatMessage | undefined) || undefined,
      };
    } catch {
      return null;
    }
  }

  function readRoundFailedPayload(
    raw: string | undefined,
  ): RoundFailedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      return {
        error: String(parsed.error || ""),
      };
    } catch {
      return {
        error: text,
      };
    }
  }

  function flushStreamBuffer(gen: number) {
    if (gen !== activeDisplayGeneration) {
      clearStreamBuffer();
      return;
    }
    if (!streamPendingText) {
      if (!options.chatting.value) {
        clearStreamBuffer();
      }
      return;
    }
    const now = Date.now();
    const msLeft = Math.max(1, streamDrainDeadline - now);
    const ticksLeft = Math.max(1, Math.ceil(msLeft / STREAM_FLUSH_INTERVAL_MS));
    const step = Math.max(1, Math.ceil(streamPendingText.length / ticksLeft));
    options.latestAssistantText.value += streamPendingText.slice(0, step);
    streamPendingText = streamPendingText.slice(step);
  }

  function enqueueStreamDelta(gen: number, delta: string) {
    if (gen !== activeDisplayGeneration || !delta) return;
    streamPendingText += delta;
    streamDrainDeadline = Date.now() + STREAM_DRAIN_TARGET_MS;
    if (!streamFlushTimer) {
      streamFlushTimer = setInterval(
        () => flushStreamBuffer(gen),
        STREAM_FLUSH_INTERVAL_MS,
      );
    }
  }

  async function handleHistoryFlushedEvent(gen: number, parsed: AssistantDeltaEvent) {
    const flushed = readHistoryFlushedPayload(parsed.message);
    const batchVisibleCount = Math.max(1, flushed?.messageCount || 0);
    activeDisplayGeneration = gen;
    activeHistoryMessageCount = batchVisibleCount;
    options.visibleMessageBlockCount.value = batchVisibleCount;
    resetDisplayedRoundState();
    if (options.onHistoryFlushed) {
      await options.onHistoryFlushed({
        conversationId: String(flushed?.conversationId || "").trim(),
        messageCount: batchVisibleCount,
        pendingMessages: flushed?.messages || [],
      });
    } else {
      await options.onReloadMessages();
    }
    options.chatting.value = !!flushed?.activateAssistant;
  }

  async function applyRoundCompleted(
    gen: number,
    result: {
      assistantText: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      assistantMessage?: ChatMessage;
    },
  ) {
    if (gen !== activeDisplayGeneration) return;
    clearStreamBuffer();
    options.latestAssistantText.value = String(result.assistantText || "");
    if (typeof result.reasoningStandard === "string") {
      options.latestReasoningStandardText.value = result.reasoningStandard;
    }
    if (typeof result.reasoningInline === "string") {
      options.latestReasoningInlineText.value = result.reasoningInline;
    }
    options.chatErrorText.value = "";
    if ((options.toolStatusState.value as string) === "running") {
      options.toolStatusState.value = "done";
      options.toolStatusText.value =
        summarizeToolCallsText() || options.t("status.toolCallDone");
    }
    if (result.assistantMessage) {
      const currentMessages = options.allMessages.value;
      if (!currentMessages.some((item) => item.id === result.assistantMessage?.id)) {
        options.allMessages.value = [...currentMessages, result.assistantMessage];
      }
    } else {
      await options.onReloadMessages();
    }
    options.visibleMessageBlockCount.value =
      activeHistoryMessageCount + (hasAssistantVisibleOutput(result) ? 1 : 0);
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
  }

  async function applyRoundFailed(gen: number, error: unknown) {
    if (gen !== activeDisplayGeneration) return;
    clearStreamBuffer();
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.chatErrorText.value = options.formatRequestFailed(error);
    if (!options.toolStatusText.value) {
      options.toolStatusState.value = "failed";
      options.toolStatusText.value =
        summarizeToolCallsText() || options.t("status.toolCallFailed");
    }
    options.visibleMessageBlockCount.value = activeHistoryMessageCount;
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    await options.onReloadMessages();
  }

  function attachDeltaHandler(
    channel: Channel<AssistantDeltaEvent>,
    getGeneration: () => number,
    nextGenerationOnHistoryFlushed: () => number,
  ) {
    channel.onmessage = (event) => {
      const parsed = readAssistantEvent(event);
      if (parsed.kind === "history_flushed") {
        const gen = nextGenerationOnHistoryFlushed();
        void handleHistoryFlushedEvent(gen, parsed).catch((error) => {
          const err = error as { message?: unknown; stack?: unknown };
          console.error("[聊天] history_flushed 处理失败", {
            action: "history_flushed",
            message: String(err?.message ?? error ?? ""),
            stack: String(err?.stack ?? ""),
            requestGeneration: gen,
          });
          options.chatErrorText.value = options.formatRequestFailed(error);
        });
        return;
      }
      const gen = getGeneration();
      if (!gen || gen !== activeDisplayGeneration) return;
      if (parsed.kind === "round_completed") {
        const payload = readRoundCompletedPayload(parsed.message);
        void applyRoundCompleted(gen, {
          assistantText: String(payload?.assistantText || ""),
          reasoningStandard: payload?.reasoningStandard,
          reasoningInline: payload?.reasoningInline,
          assistantMessage: payload?.assistantMessage,
        }).catch((error) => {
          const err = error as { message?: unknown; stack?: unknown };
          console.error("[聊天] round_completed 处理失败", {
            action: "round_completed",
            message: String(err?.message ?? error ?? ""),
            stack: String(err?.stack ?? ""),
            requestGeneration: gen,
          });
          options.chatErrorText.value = options.formatRequestFailed(error);
        });
        return;
      }
      if (parsed.kind === "round_failed") {
        const payload = readRoundFailedPayload(parsed.message);
        void applyRoundFailed(gen, payload?.error || parsed.message || "round_failed").catch((error) => {
          const err = error as { message?: unknown; stack?: unknown };
          console.error("[聊天] round_failed 处理失败", {
            action: "round_failed",
            message: String(err?.message ?? error ?? ""),
            stack: String(err?.stack ?? ""),
            requestGeneration: gen,
          });
          options.chatErrorText.value = options.formatRequestFailed(error);
        });
        return;
      }
      if (parsed.kind === "tool_status") {
        const toolName = String(parsed.toolName || "").trim();
        if (parsed.toolStatus === "running" && toolName) {
          streamToolCallCount += 1;
          streamLastToolName = toolName;
          options.streamToolCalls.value = [
            ...options.streamToolCalls.value,
            {
              name: toolName,
              argsText: String(parsed.toolArgs || "").trim(),
            },
          ];
        }
        options.toolStatusText.value = parsed.message || "";
        options.toolStatusState.value =
          parsed.toolStatus === "running" ||
          parsed.toolStatus === "done" ||
          parsed.toolStatus === "failed"
            ? parsed.toolStatus
            : "";
        return;
      }
      if (parsed.kind === "reasoning_standard") {
        const deltaText = readDeltaMessage(parsed);
        if (deltaText && reasoningStartedAtMs.value === 0)
          reasoningStartedAtMs.value = Date.now();
        options.latestReasoningStandardText.value += deltaText;
        return;
      }
      if (parsed.kind === "reasoning_inline") {
        const deltaText = readDeltaMessage(parsed);
        if (deltaText && reasoningStartedAtMs.value === 0)
          reasoningStartedAtMs.value = Date.now();
        options.latestReasoningInlineText.value += deltaText;
        return;
      }
      enqueueStreamDelta(gen, readDeltaMessage(parsed));
    };
  }

  let boundConversationId = "";
  let boundDisplayGeneration = 0;
  const boundDeltaChannel = new Channel<AssistantDeltaEvent>();
  attachDeltaHandler(
    boundDeltaChannel,
    () => boundDisplayGeneration,
    () => {
      boundDisplayGeneration = ++requestGeneration;
      return boundDisplayGeneration;
    },
  );

  async function bindActiveConversationStream(conversationId: string) {
    if (!options.invokeBindActiveChatViewStream) return;
    const trimmedConversationId = String(conversationId || "").trim();
    if (trimmedConversationId === boundConversationId) return;
    await options.invokeBindActiveChatViewStream({
      conversationId: trimmedConversationId || undefined,
      onDelta: boundDeltaChannel,
    });
    boundConversationId = trimmedConversationId;
    if (!trimmedConversationId) {
      boundDisplayGeneration = 0;
    }
  }

  async function sendChat() {
    // 注意：不再检查 forcingArchive 和 chatting，因为后端已通过状态机（MainSessionState）和队列处理并发控制
    // 流式期间和归档期间的消息都会入队，由后端串行处理
    const text = options.chatInput.value.trim();
    if (!text && options.clipboardImages.value.length === 0) return;
    const sendSession = options.getSession();
    if (!sendSession || !sendSession.apiConfigId || !sendSession.agentId)
      return;

    const wasChatting = options.chatting.value;
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
    options.streamToolCalls.value = [];
    options.chatErrorText.value = "";

    const sentImages = [...options.clipboardImages.value];
    options.chatInput.value = "";
    options.clipboardImages.value = [];

    const gen = ++requestGeneration;
    if (!wasChatting) {
      // 非对话中发送时，先只完成“入队”。
      // 当前轮次必须等到 history_flushed 之后，才允许进入前台可见状态。
      activeDisplayGeneration = gen;
      resetDisplayedRoundState();
    }
    const deltaChannel = new Channel<AssistantDeltaEvent>();
    attachDeltaHandler(deltaChannel, () => gen, () => gen);

    try {
      const result = await options.invokeSendChatMessage({
        text,
        images: sentImages,
        session: {
          ...sendSession,
          conversationId: options.getConversationId ? options.getConversationId() : "",
        },
        onDelta: deltaChannel,
      });
      const currentSession = options.getSession();
      const sameSession =
        !!currentSession &&
        currentSession.apiConfigId === sendSession.apiConfigId &&
        currentSession.agentId === sendSession.agentId;
      if (!sameSession) return;
      await applyRoundCompleted(gen, {
        assistantText: String(result.assistantText || ""),
        reasoningStandard: result.reasoningStandard,
        reasoningInline: result.reasoningInline,
        assistantMessage: result.assistantMessage,
      });
    } catch (error) {
      const err = error as { message?: unknown; stack?: unknown };
      console.error("[聊天] 聊天流程请求失败", {
        action: "sendChat",
        apiConfigId: sendSession.apiConfigId,
        agentId: sendSession.agentId,
        requestGeneration: gen,
        message: String(err?.message ?? error ?? ""),
        stack: String(err?.stack ?? ""),
      });
      if (gen !== activeDisplayGeneration) {
        options.chatErrorText.value = options.formatRequestFailed(error);
        return;
      }
      clearStreamBuffer();
      options.latestAssistantText.value = "";
      options.latestReasoningStandardText.value = "";
      options.latestReasoningInlineText.value = "";
      options.chatErrorText.value = options.formatRequestFailed(error);
      if (!options.toolStatusText.value) {
        options.toolStatusState.value = "failed";
        options.toolStatusText.value =
          summarizeToolCallsText() || options.t("status.toolCallFailed");
      }
      const currentSession = options.getSession();
      const sameSession =
        !!currentSession &&
        currentSession.apiConfigId === sendSession.apiConfigId &&
        currentSession.agentId === sendSession.agentId;
      if (sameSession) {
        await applyRoundFailed(gen, error);
      }
    } finally {
      if (gen === activeDisplayGeneration) {
        options.chatting.value = false;
        reasoningStartedAtMs.value = 0;
      }
    }
  }

  async function stopChat() {
    if (!options.chatting.value) return;
    const stopSession = options.getSession();
    const gen = ++requestGeneration;
    activeDisplayGeneration = gen;
    if (streamPendingText) {
      options.latestAssistantText.value += streamPendingText;
      streamPendingText = "";
    }
    clearStreamBuffer();
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    if (options.toolStatusState.value === "running") {
      options.toolStatusState.value = "failed";
      options.toolStatusText.value =
        summarizeToolCallsText() || options.t("status.interrupted");
    } else {
      options.toolStatusState.value = "";
      options.toolStatusText.value = "";
    }
    const partialAssistantText = options.latestAssistantText.value;
    const partialReasoningStandard = options.latestReasoningStandardText.value;
    const partialReasoningInline = options.latestReasoningInlineText.value;
    if (stopSession && options.invokeStopChatMessage) {
      try {
        const conversationId = options.getConversationId ? options.getConversationId() : "";
        await options.invokeStopChatMessage({
          session: conversationId
            ? {
                ...stopSession,
                conversationId,
              }
            : stopSession,
          partialAssistantText,
          partialReasoningStandard,
          partialReasoningInline,
        });
      } catch (error) {
        const errorText =
          error instanceof Error
            ? `${error.message}\n${error.stack || ""}`.trim()
            : (() => {
                try {
                  return JSON.stringify(error);
                } catch {
                  return String(error);
                }
              })();
        console.warn(
          `[聊天] 停止消息失败，apiConfigId=${stopSession.apiConfigId}，agentId=${stopSession.agentId}，latestAssistantTextLength=${partialAssistantText.length}，错误=${errorText}`,
        );
      }
    }
    if (gen !== activeDisplayGeneration) return;
    // stop 是纠偏路径，保持从后端整段重载，确保最终一致性。
    await options.onReloadMessages();
  }

  return {
    sendChat,
    stopChat,
    bindActiveConversationStream,
    clearStreamBuffer,
    reasoningStartedAtMs,
  };
}
