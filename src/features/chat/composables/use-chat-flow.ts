import { Channel } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { ChatMessage } from "../../../types/app";

export type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
  toolName?: string;
  toolStatus?: string;
  message?: string;
};

type HistoryFlushedPayload = {
  conversationId: string;
  messageCount: number;
  messages: ChatMessage[];
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
      };
    } catch {
      return {
        conversationId: text,
        messageCount: 0,
        messages: [],
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
    deltaChannel.onmessage = (event) => {
      const parsed = readAssistantEvent(event);
      if (parsed.kind === "history_flushed") {
        void (async () => {
          try {
            const flushed = readHistoryFlushedPayload(parsed.message);
            const batchVisibleCount = Math.max(1, flushed?.messageCount || 0);
            // history_flushed 表示：当前批次的所有消息已经正式写入历史。
            // 只有到这一刻，前端才允许切换“前台可见轮次”。
            //
            // 如果这是排队中的下一轮请求，它会在这里接管前台显示；
            // 如果这是空闲状态下的直接对话，也同样通过这条边界完成
            // “队列/临时态 -> 正式历史 -> 新一轮流式”的切换。
            activeDisplayGeneration = gen;
            activeHistoryMessageCount = batchVisibleCount;
            // 前台窗口的单位不再是“最后 1 条消息”，
            // 而是“当前批次刚刚刷入历史的全部消息块”。
            options.visibleMessageBlockCount.value = batchVisibleCount;
            resetDisplayedRoundState();
            // 优先消费前端已持有的“已确认批次”，减少每轮都回源后端带来的等待。
            // 未提供本地批次消费器时，退化到原有整段历史重载。
            if (options.onHistoryFlushed) {
              await options.onHistoryFlushed({
                conversationId: String(flushed?.conversationId || "").trim(),
                messageCount: batchVisibleCount,
                pendingMessages: flushed?.messages || [],
              });
            } else {
              await options.onReloadMessages();
            }
            // 只有当本批消息已经真正刷新到当前窗口之后，
            // 才允许出现新的助理流式气泡和停止按钮。
            options.chatting.value = true;
          } catch (error) {
            const err = error as { message?: unknown; stack?: unknown };
            console.error("[聊天] 历史已清理 处理失败", {
              action: "history_flushed",
              message: String(err?.message ?? error ?? ""),
              stack: String(err?.stack ?? ""),
              requestGeneration: gen,
            });
            options.chatErrorText.value = options.formatRequestFailed(error);
          }
        })();
        return;
      }
      if (gen !== activeDisplayGeneration) return;
      if (parsed.kind === "tool_status") {
        const toolName = String(parsed.toolName || "").trim();
        if (parsed.toolStatus === "running" && toolName) {
          streamToolCallCount += 1;
          streamLastToolName = toolName;
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
      if (gen !== activeDisplayGeneration) return;
      // Always align to backend final text to avoid stream/snapshot race drift.
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
      const currentSession = options.getSession();
      const sameSession =
        !!currentSession &&
        currentSession.apiConfigId === sendSession.apiConfigId &&
        currentSession.agentId === sendSession.agentId;
      if (sameSession) {
        if (result.assistantMessage) {
          const currentMessages = options.allMessages.value;
          if (!currentMessages.some((item) => item.id === result.assistantMessage?.id)) {
            options.allMessages.value = [...currentMessages, result.assistantMessage];
          }
        } else {
          await options.onReloadMessages();
        }
        options.visibleMessageBlockCount.value =
          activeHistoryMessageCount +
          (hasAssistantVisibleOutput(result) ? 1 : 0);
      }
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
        options.visibleMessageBlockCount.value = activeHistoryMessageCount;
        await options.onReloadMessages();
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
    clearStreamBuffer,
    reasoningStartedAtMs,
  };
}
