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
  const messageBlockCache = new WeakMap<ChatMessage, { signature: string; blocks: ChatMessageBlock[] }>();

  function messageSignature(message: ChatMessage): string {
    const cached = messageSignatureCache.get(message);
    if (cached) return cached;
    const signature = [
      String(message.id || "").trim(),
      String(message.createdAt || "").trim(),
      JSON.stringify(message.parts || []),
      JSON.stringify(message.extraTextBlocks || []),
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

  function buildMessageBlocks(message: ChatMessage): ChatMessageBlock[] {
    const signature = messageSignature(message);
    const cached = messageBlockCache.get(message);
    if (cached && cached.signature === signature) {
      return cached.blocks;
    }

    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    const projection = projectMessageForDisplay(message);
    const streamSegments = Array.isArray(meta._streamSegments)
      ? (meta._streamSegments as unknown[])
        .map((item) => String(item ?? ""))
        .filter((item) => item.length > 0)
      : [];
    const streamTail = String(meta._streamTail ?? "");
    const streamAnimatedDelta = String(meta._streamAnimatedDelta ?? "");
    const baseBlock = {
      id: message.id,
      sourceMessageId: message.id,
      isExtraTextBlock: false,
      role: message.role,
      isStreaming: !!meta._streaming,
      streamSegments,
      streamTail,
      streamAnimatedDelta,
      speakerAgentId: projection.speakerAgentId,
      createdAt: String(message.createdAt || "").trim() || undefined,
      providerMeta: message.providerMeta,
      text: projection.text,
      images: projection.images,
      audios: projection.audios,
      attachmentFiles: projection.attachmentFiles,
      taskTrigger: projection.taskTrigger,
      planCard: projection.planCard,
      remoteImOrigin: projection.remoteImOrigin,
      reasoningStandard: projection.reasoningStandard,
      reasoningInline: projection.reasoningInline,
      toolCallCount: projection.toolCallCount,
      lastToolName: projection.lastToolName,
      toolCalls: projection.toolCalls,
    } satisfies ChatMessageBlock;

    const blocks: ChatMessageBlock[] = [];
    if (
      baseBlock.text
      || !!baseBlock.isStreaming
      || baseBlock.images.length > 0
      || baseBlock.audios.length > 0
      || baseBlock.attachmentFiles.length > 0
      || !!baseBlock.taskTrigger
      || !!baseBlock.planCard
      || !!baseBlock.reasoningStandard
      || !!baseBlock.reasoningInline
      || baseBlock.toolCallCount > 0
    ) {
      blocks.push(baseBlock);
    }

    if (Array.isArray(message.extraTextBlocks)) {
      message.extraTextBlocks.forEach((raw, index) => {
        const text = String(raw || "").trim();
        if (!text) return;
        blocks.push({
          id: `${message.id}::extra:${index}`,
          sourceMessageId: message.id,
          isExtraTextBlock: true,
          role: message.role,
          isStreaming: false,
          streamSegments: [],
          streamTail: "",
          streamAnimatedDelta: "",
          speakerAgentId: projection.speakerAgentId,
          createdAt: String(message.createdAt || "").trim() || undefined,
          providerMeta: message.providerMeta,
          text,
          images: [],
          audios: [],
          attachmentFiles: [],
          taskTrigger: undefined,
          planCard: undefined,
          remoteImOrigin: projection.remoteImOrigin,
          reasoningStandard: "",
          reasoningInline: "",
          toolCallCount: 0,
          lastToolName: "",
          toolCalls: [],
        });
      });
    }

    messageBlockCache.set(message, { signature, blocks });
    return blocks;
  }

  const allMessageBlocks = computed<ChatMessageBlock[]>(() => {
    const messages = options.allMessages.value;
    const signature = messages.map((message) => messageSignature(message)).join("||");
    if (signature === lastMessageBlockSignature) {
      return lastMessageBlocks;
    }
    const blocks = messages
      .flatMap((message) => buildMessageBlocks(message));
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
