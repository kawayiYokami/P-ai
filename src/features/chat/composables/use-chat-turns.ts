import { computed, type ComputedRef, type Ref, type ShallowRef } from "vue";
import type { ApiConfigItem, ChatMessage, ChatMessageBlock } from "../../../types/app";
import {
  estimateConversationTokens,
} from "../../../utils/chat-message";
import {
  projectMessageForDisplay,
} from "../../../utils/chat-message-semantics";

type UseChatMessageBlocksOptions = {
  allMessages: ShallowRef<ChatMessage[]>;
  activeChatApiConfig: ComputedRef<ApiConfigItem | null>;
  perfDebug: boolean;
  perfNow: () => number;
};

export function useChatMessageBlocks(options: UseChatMessageBlocksOptions) {
  let lastMessageBlockSignature = "";
  let lastMessageBlocks: ChatMessageBlock[] = [];
  const messageSignatureCache = new WeakMap<ChatMessage, string>();
  const messageBlockCache = new WeakMap<ChatMessage, { signature: string; block: ChatMessageBlock | null }>();

  function messageSignature(message: ChatMessage): string {
    const cached = messageSignatureCache.get(message);
    if (cached) return cached;
    const signature = [
      String(message.id || "").trim(),
      String(message.createdAt || "").trim(),
      JSON.stringify(message.parts || []),
      JSON.stringify(message.providerMeta || {}),
      JSON.stringify(message.toolCall || []),
    ].join("|");
    messageSignatureCache.set(message, signature);
    return signature;
  }

  function latestBackendContextUsagePercent(messages: ChatMessage[]): number | null {
    for (let idx = messages.length - 1; idx >= 0; idx -= 1) {
      const message = messages[idx];
      if (message.role !== "assistant") continue;
      const raw = Number((message.providerMeta || {}).contextUsagePercent);
      if (!Number.isFinite(raw)) continue;
      return Math.min(100, Math.max(0, Math.round(raw)));
    }
    return null;
  }

  function buildMessageBlock(message: ChatMessage): ChatMessageBlock | null {
    const signature = messageSignature(message);
    const cached = messageBlockCache.get(message);
    if (cached && cached.signature === signature) {
      return cached.block;
    }

    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    const projection = projectMessageForDisplay(message);
    const streamSegments = Array.isArray(meta._streamSegments)
      ? (meta._streamSegments as unknown[])
        .map((item) => String(item ?? ""))
        .filter((item) => item.length > 0)
      : [];
    const streamTail = String(meta._streamTail ?? "");
    const block = {
      id: message.id,
      role: message.role,
      isStreaming: !!meta._streaming,
      streamSegments,
      streamTail,
      speakerAgentId: projection.speakerAgentId,
      createdAt: String(message.createdAt || "").trim() || undefined,
      providerMeta: message.providerMeta,
      text: projection.text,
      images: projection.images,
      audios: projection.audios,
      attachmentFiles: projection.attachmentFiles,
      taskTrigger: projection.taskTrigger,
      remoteImOrigin: projection.remoteImOrigin,
      reasoningStandard: projection.reasoningStandard,
      reasoningInline: projection.reasoningInline,
      toolCallCount: projection.toolCallCount,
      lastToolName: projection.lastToolName,
      toolCalls: projection.toolCalls,
    } satisfies ChatMessageBlock;

    const normalized =
      block.text
      || !!block.isStreaming
      || block.images.length > 0
      || block.audios.length > 0
      || block.attachmentFiles.length > 0
      || !!block.taskTrigger
      || !!block.reasoningStandard
      || !!block.reasoningInline
        ? block
        : null;

    messageBlockCache.set(message, { signature, block: normalized });
    return normalized;
  }

  const allMessageBlocks = computed<ChatMessageBlock[]>(() => {
    const messages = options.allMessages.value;
    const signature = messages.map((message) => messageSignature(message)).join("||");
    if (signature === lastMessageBlockSignature) {
      return lastMessageBlocks;
    }
    const blocks = messages
      .map((message) => buildMessageBlock(message))
      .filter((block): block is ChatMessageBlock => !!block);
    lastMessageBlockSignature = signature;
    lastMessageBlocks = blocks;
    return blocks;
  });

  const visibleMessageBlocks = computed(() => allMessageBlocks.value);

  const chatContextUsageRatio = computed(() => {
    const backendPercent = latestBackendContextUsagePercent(options.allMessages.value);
    if (backendPercent !== null) {
      return backendPercent / 100;
    }
    const api = options.activeChatApiConfig.value;
    if (!api) return 0;
    const maxTokens = Math.max(16000, Math.round(Number(api.contextWindowTokens ?? 128000)));
    const used = estimateConversationTokens(options.allMessages.value);
    return used / Math.max(1, maxTokens);
  });

  const chatUsagePercent = computed(() => Math.min(100, Math.max(0, Math.round(chatContextUsageRatio.value * 100))));

  return {
    allMessageBlocks,
    visibleMessageBlocks,
    chatContextUsageRatio,
    chatUsagePercent,
  };
}
