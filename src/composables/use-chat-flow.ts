import { Channel } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { ChatMessage } from "../types/app";

export type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
  toolName?: string;
  toolStatus?: string;
  message?: string;
};

type UseChatFlowOptions = {
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
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
  visibleTurnCount: Ref<number>;
  activeChatApiConfigId: Ref<string>;
  selectedPersonaId: Ref<string>;
  activeChatModel: () => string | undefined;
  t: (key: string, params?: Record<string, unknown>) => string;
  formatRequestFailed: (error: unknown) => string;
  removeBinaryPlaceholders: (text: string) => string;
  invokeSendChatMessage: (input: {
    apiConfigId: string;
    agentId: string;
    text: string;
    images: Array<{ mime: string; bytesBase64: string }>;
    model?: string;
    onDelta: Channel<AssistantDeltaEvent>;
  }) => Promise<{ assistantText: string; latestUserText: string; archivedBeforeSend: boolean }>;
  onReloadMessages: () => Promise<void>;
};

const STREAM_FLUSH_INTERVAL_MS = 33;
const STREAM_DRAIN_TARGET_MS = 1000;

export function useChatFlow(options: UseChatFlowOptions) {
  let chatGeneration = 0;
  let streamPendingText = "";
  let streamDrainDeadline = 0;
  let streamFlushTimer: ReturnType<typeof setInterval> | null = null;
  const reasoningStartedAtMs = ref(0);

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

  function flushStreamBuffer(gen: number) {
    if (gen !== chatGeneration) {
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
    if (gen !== chatGeneration || !delta) return;
    streamPendingText += delta;
    streamDrainDeadline = Date.now() + STREAM_DRAIN_TARGET_MS;
    if (!streamFlushTimer) {
      streamFlushTimer = setInterval(() => flushStreamBuffer(gen), STREAM_FLUSH_INTERVAL_MS);
    }
  }

  function enqueueFinalAssistantText(gen: number, finalText: string) {
    if (gen !== chatGeneration) return;
    const text = finalText.trim();
    if (!text) return;
    const combined = `${options.latestAssistantText.value}${streamPendingText}`;
    if (!combined) {
      enqueueStreamDelta(gen, finalText);
      return;
    }
    if (text.startsWith(combined)) {
      const missing = text.slice(combined.length);
      if (missing) enqueueStreamDelta(gen, missing);
    }
  }

  async function sendChat() {
    if (options.chatting.value || options.forcingArchive.value) return;
    const text = options.chatInput.value.trim();
    if (!text && options.clipboardImages.value.length === 0) return;

    options.latestUserText.value = text;
    options.latestUserImages.value = [...options.clipboardImages.value];
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
    options.chatErrorText.value = "";

    const sentImages = [...options.clipboardImages.value];
    const sentModel = options.activeChatModel();
    options.chatInput.value = "";
    options.clipboardImages.value = [];

    const optimisticUserMessage: ChatMessage = {
      id: `optimistic-user-${Date.now()}`,
      role: "user",
      parts: [
        ...(text ? [{ type: "text" as const, text }] : []),
        ...sentImages.map((img) => ({
          type: "image" as const,
          mime: img.mime,
          bytesBase64: img.bytesBase64,
        })),
      ],
    };
    options.allMessages.value = [...options.allMessages.value, optimisticUserMessage];
    options.visibleTurnCount.value = 1;

    const gen = ++chatGeneration;
    clearStreamBuffer();
    const deltaChannel = new Channel<AssistantDeltaEvent>();
    deltaChannel.onmessage = (event) => {
      const parsed = readAssistantEvent(event);
      if (parsed.kind === "tool_status") {
        options.toolStatusText.value = parsed.message || "";
        options.toolStatusState.value = parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
          ? parsed.toolStatus
          : "";
        return;
      }
      if (parsed.kind === "reasoning_standard") {
        const deltaText = readDeltaMessage(parsed);
        if (deltaText && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
        options.latestReasoningStandardText.value += deltaText;
        return;
      }
      if (parsed.kind === "reasoning_inline") {
        const deltaText = readDeltaMessage(parsed);
        if (deltaText && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
        options.latestReasoningInlineText.value += deltaText;
        return;
      }
      enqueueStreamDelta(gen, readDeltaMessage(parsed));
    };

    options.chatting.value = true;
    try {
      const result = await options.invokeSendChatMessage({
        apiConfigId: options.activeChatApiConfigId.value,
        agentId: options.selectedPersonaId.value,
        text,
        images: sentImages,
        model: sentModel,
        onDelta: deltaChannel,
      });
      if (gen !== chatGeneration) return;
      options.latestUserText.value = options.removeBinaryPlaceholders(result.latestUserText);
      options.latestUserImages.value = sentImages;
      enqueueFinalAssistantText(gen, result.assistantText);
      options.chatErrorText.value = "";
      if ((options.toolStatusState.value as string) === "running") {
        options.toolStatusState.value = "done";
        options.toolStatusText.value = options.t("status.toolCallDone");
      }
      await options.onReloadMessages();
    } catch (error) {
      if (gen !== chatGeneration) return;
      clearStreamBuffer();
      options.latestAssistantText.value = "";
      options.latestReasoningStandardText.value = "";
      options.latestReasoningInlineText.value = "";
      options.chatErrorText.value = options.formatRequestFailed(error);
      if (!options.toolStatusText.value) {
        options.toolStatusState.value = "failed";
        options.toolStatusText.value = options.t("status.toolCallFailed");
      }
      await options.onReloadMessages();
    } finally {
      if (gen === chatGeneration) {
        options.chatting.value = false;
        reasoningStartedAtMs.value = 0;
      }
    }
  }

  function stopChat() {
    chatGeneration += 1;
    clearStreamBuffer();
    options.chatting.value = false;
    options.latestAssistantText.value = options.t("status.interrupted");
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    reasoningStartedAtMs.value = 0;
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
  }

  return {
    sendChat,
    stopChat,
    clearStreamBuffer,
    reasoningStartedAtMs,
  };
}
