import { computed, type ComputedRef, type Ref, type ShallowRef } from "vue";
import type { ApiConfigItem, ChatMessage, ChatMessageBlock } from "../../../types/app";
import {
  readContextUsageRatioFromRecord,
  type ContextUsageUpdatePayload,
} from "./use-chat-flow-events";
import {
  estimateConversationTokens,
} from "../../../utils/chat-message";
import {
  applyMemeAnnotationReplacements,
  assistantTextFromStreamBlocks,
  assistantContentBlocksFromMessage,
  normalizeAssistantStreamBlocks,
  normalizeChatActivityItems,
  projectMessageForDisplay,
  projectStreamingChatActivityForDisplay,
  streamBlocksActivitySignature,
  streamBlocksToActivityItems,
  streamBlocksToToolCalls,
} from "../../../utils/chat-message-semantics";
import { displayLabelFromExtraTextReference } from "../utils/chat-attachment-display";

function baseActivityForMessage(
  projection: ReturnType<typeof projectMessageForDisplay>,
  isStreaming: boolean,
  streamBlocks: ReturnType<typeof normalizeAssistantStreamBlocks>,
) {
  if (isStreaming) {
    return projectStreamingChatActivityForDisplay({
      activityItems: projection.activityItems,
      streamBlocks,
      running: true,
    });
  }
  if (streamBlocks.length > 0) {
    const items = streamBlocksToActivityItems(streamBlocks, false)
      .filter((item) => item.kind === "reasoning" || item.kind === "tool");
    const activityToolCountsByName: Record<string, number> = {};
    let activityReasoningCharCount = 0;
    for (const item of items) {
      if (item.kind === "reasoning") {
        activityReasoningCharCount += String(item.text || "").length;
        continue;
      }
      const name = String(item.name || "").trim() || "unknown";
      activityToolCountsByName[name] = (activityToolCountsByName[name] || 0) + 1;
    }
    return {
      items,
      activityReasoningCharCount,
      activityToolCountsByName,
      activityRunning: false,
      activityStatus: items.length > 0 ? "complete" as const : "idle" as const,
    };
  }
  return {
    items: projection.activityItems,
    activityReasoningCharCount: projection.activityReasoningCharCount,
    activityToolCountsByName: projection.activityToolCountsByName,
    activityRunning: projection.activityRunning,
    activityStatus: projection.activityStatus,
  };
}

function positiveNumberFromProviderMeta(meta: Record<string, unknown>, key: string): number | undefined {
  const value = Number(meta[key]);
  if (!Number.isFinite(value) || value <= 0) return undefined;
  return Math.round(value);
}

function extraTextReferenceLabel(text: string): string {
  return displayLabelFromExtraTextReference(text);
}

function buildExtraTextReferences(message: ChatMessage): Array<{ label: string; text: string }> {
  if (!Array.isArray(message.extraTextBlocks)) return [];
  return message.extraTextBlocks
    .map((raw) => String(raw || "").trim())
    .filter(Boolean)
    .map((text) => ({ label: extraTextReferenceLabel(text), text }));
}

type UseChatMessageBlocksOptions = {
  allMessages: ShallowRef<ChatMessage[]>;
  activeChatApiConfig: ComputedRef<ApiConfigItem | null>;
  currentConversationId?: Ref<string>;
  contextUsagePreview?: Ref<ContextUsageUpdatePayload | null>;
  perfDebug: boolean;
  perfNow: () => number;
  taskTriggerLabels?: { goal: string; todo: string };
};

export function useChatMessageBlocks(options: UseChatMessageBlocksOptions) {
  let lastMessageBlockSignature = "";
  let lastMessageBlocks: ChatMessageBlock[] = [];
  const messageSignatureCache = new WeakMap<ChatMessage, string>();
  const messageBlockCache = new WeakMap<ChatMessage, { signature: string; blocks: ChatMessageBlock[] }>();

  function messagePartsSignature(message: ChatMessage): string {
    return (Array.isArray(message.parts) ? message.parts : [])
      .map((part, index) => {
        if (!part || typeof part !== "object") return `unknown:${index}`;
        if (part.type === "text") {
          return [
            "text",
            String(part.text || "").length,
            String(part.reasoningContent || part.reasoning_content || "").length,
          ].join(":");
        }
        if (part.type === "image") {
          return ["image", String(part.mime || ""), String(part.bytesBase64 || "").length].join(":");
        }
        if (part.type === "audio") {
          return ["audio", String(part.mime || ""), String(part.bytesBase64 || "").length].join(":");
        }
        return `${String((part as { type?: string }).type || "unknown")}:${index}`;
      })
      .join("|");
  }

  function providerMetaSignature(message: ChatMessage): string {
    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    const streamBlocksSignature = streamBlocksActivitySignature(assistantContentBlocksFromMessage(message));
    const attachments = Array.isArray(meta.attachments) ? meta.attachments.length : 0;
    const memeAnnotations = Array.isArray(message.memeAnnotations)
      ? message.memeAnnotations.map((item) => `${String(item.meme || "").trim()}::${String(item.path || "").trim()}`).join("|")
      : "";
    return [
      String(meta.messageKind || ""),
      String(meta.hiddenPromptText || "").length,
      String(meta._streaming ? "1" : "0"),
      String(meta._toolStatusText || "").length,
      String(meta._toolStatusState || ""),
      // 耗时数字（dispatchElapsedMs/_frontendDispatchElapsedMs）不进签名：
      // 它由显示层独立状态驱动（frontendDispatchElapsedByMessageId），
      // 若计入签名，计时器每秒更新会破坏消息块缓存、带动虚拟列表空转重算。
      attachments,
      String(meta.planCard ? "1" : "0"),
      String(meta.taskTrigger ? "1" : "0"),
      streamBlocksSignature,
      memeAnnotations,
    ].join("|");
  }

  function toolCallSignature(message: ChatMessage): string {
    return (Array.isArray(message.toolCall) ? message.toolCall : [])
      .map((event, index) => {
        const calls = Array.isArray(event?.tool_calls) ? event.tool_calls : [];
        return [
          String(event?.role || ""),
          String(event?.content || "").length,
          String(event?.reasoning_content || "").length,
          String(event?.tool_call_id || ""),
          calls.length,
          ...calls.map((call) => [
            String(call?.id || call?.call_id || ""),
            String(call?.function?.name || ""),
            typeof call?.function?.arguments === "string"
              ? call.function.arguments.length
              : JSON.stringify(call?.function?.arguments || {}).length,
          ].join(":")),
          index,
        ].join("|");
      })
      .join("||");
  }

  function activityItemsSignature(message: ChatMessage): string {
    return normalizeChatActivityItems(message.activityItems)
      .map((item) => {
        if (item.kind === "tool") {
          return [
            "tool",
            String(item.id || ""),
            String(item.toolCallId || ""),
            String(item.name || ""),
            String(item.status || ""),
            String(item.argsText || "").length,
            String(item.resultText || "").length,
          ].join(":");
        }
        return [
          item.kind,
          String(item.id || ""),
          String(item.text || "").length,
          item.running ? "1" : "0",
        ].join(":");
      })
      .join("|");
  }

  function messageSignature(message: ChatMessage): string {
    const cached = messageSignatureCache.get(message);
    if (cached) return cached;
    const signature = [
      String(message.id || "").trim(),
      String(message.createdAt || "").trim(),
      String(message.speakerAgentId || "").trim(),
      messagePartsSignature(message),
      (Array.isArray(message.extraTextBlocks) ? message.extraTextBlocks : [])
        .map((text) => String(text || "").length)
        .join(","),
      providerMetaSignature(message),
      toolCallSignature(message),
      activityItemsSignature(message),
    ].join("|");
    messageSignatureCache.set(message, signature);
    return signature;
  }

  // 压缩消息本身不携带 usage；压缩完成后上下文按默认 20k tokens 折算占用率，
  // 避免从尾部回退到压缩前旧消息的高占用率，导致标题栏上下文圆环不归零。
  const COMPACTION_FALLBACK_CONTEXT_TOKENS = 20000;

  function isCompactionMessage(message: ChatMessage | undefined): boolean {
    const providerMeta = (message?.providerMeta || {}) as Record<string, unknown>;
    const messageMeta = (
      providerMeta.message_meta
      || providerMeta.messageMeta
      || {}
    ) as Record<string, unknown>;
    const kind = String(messageMeta.kind || providerMeta.messageKind || "").trim();
    return kind === "context_compaction" || kind === "summary_context_seed";
  }

  function latestBackendContextUsageRatio(
    messages: ChatMessage[],
    fallbackContextWindowTokens: number,
  ): number | null {
    for (let idx = messages.length - 1; idx >= 0; idx -= 1) {
      const message = messages[idx];
      if (isCompactionMessage(message)) {
        return Math.max(0, COMPACTION_FALLBACK_CONTEXT_TOKENS / Math.max(1, fallbackContextWindowTokens));
      }
      if (message.role !== "assistant") continue;
      const ratio = readContextUsageRatioFromRecord(
        (message.providerMeta || {}) as Record<string, unknown>,
        fallbackContextWindowTokens,
      );
      if (ratio === null) continue;
      return Math.max(0, ratio);
    }
    return null;
  }

  function previewContextUsageRatio(): number | null {
    const preview = options.contextUsagePreview?.value;
    if (!preview) return null;
    const activeConversationId = String(options.currentConversationId?.value || "").trim();
    if (activeConversationId && preview.conversationId !== activeConversationId) return null;
    const ratio = Number(preview.contextUsageRatio);
    if (Number.isFinite(ratio) && ratio >= 0) return ratio;
    const percent = Number(preview.contextUsagePercent);
    if (!Number.isFinite(percent)) return null;
    return Math.max(0, percent) / 100;
  }

  function buildMessageBlocks(message: ChatMessage): ChatMessageBlock[] {
    const providerMeta = (message.providerMeta || {}) as Record<string, unknown>;
    const messageMeta = ((providerMeta.message_meta || providerMeta.messageMeta || {}) as Record<string, unknown>);
    const messageKind = String(messageMeta.kind || providerMeta.messageKind || "").trim();
    if (messageKind === "plan_confirm_continue") {
      return [{
        id: message.id,
        rawMessage: message,
        sourceMessageId: message.id,
        isExtraTextBlock: false,
        role: "system",
        dividerKind: "plan_started",
        isStreaming: false,
        streamSegments: [],
        streamTail: "",
        streamAnimatedDelta: "",
        speakerAgentId: undefined,
        createdAt: String(message.createdAt || "").trim() || undefined,
        providerMeta: message.providerMeta,
        mentions: [],
        text: "",
        images: [],
        audios: [],
        attachmentFiles: [],
        extraTextReferences: [],
        taskTrigger: undefined,
        planCard: undefined,
        remoteImOrigin: undefined,
        dispatchElapsedMs: undefined,
        toolCallCount: 0,
        lastToolName: "",
        toolCalls: [],
        activityItems: [],
        activityReasoningCharCount: 0,
        activityToolCountsByName: {},
        activityRunning: false,
        activityStatus: "idle",
      }];
    }
    const signature = messageSignature(message);
    const cached = messageBlockCache.get(message);
    if (cached && cached.signature === signature) {
      return cached.blocks;
    }

    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    const projection = projectMessageForDisplay(message, options.taskTriggerLabels);
    // 分段键已收敛：_streamSegments/_streamTail/_streamAnimatedDelta 不再写入，
    // 分段由渲染层从 contentBlocks 全量重算（乐观渲染 chunks=[全量文本]）。
    const streamSegments: string[] = [];
    const streamTail = "";
    const streamAnimatedDelta = "";
    const streamBlocks = assistantContentBlocksFromMessage(message);
    const streamingDisplayTextRaw = assistantTextFromStreamBlocks(streamBlocks);
    const streamingDisplayText = applyMemeAnnotationReplacements(
      streamingDisplayTextRaw,
      message.memeAnnotations,
    );
    const streamBlockToolCalls = streamBlocksToToolCalls(streamBlocks);
    const streamBlockActivityItems = streamBlocksToActivityItems(streamBlocks, false);
    if (
      !meta._streaming
      && streamBlocks.length > 0
      && (streamBlockToolCalls.length > 0 || streamBlockActivityItems.length > 0 || !!streamingDisplayText.trim())
      && projection.toolCalls.length === 0
      && projection.activityItems.length === 0
    ) {
      console.warn("[聊天] 检测到停止后消息投影缺失，streamBlocks 有内容但投影为空", {
        conversationId: String(options.currentConversationId?.value || "").trim(),
        messageId: String(message.id || "").trim(),
        streamBlockCount: streamBlocks.length,
        streamToolCallCount: streamBlockToolCalls.length,
        streamActivityCount: streamBlockActivityItems.length,
        streamTextLength: streamingDisplayText.length,
        projectionTextLength: String(projection.text || "").length,
      });
    }
    const displayToolCalls = !!meta._streaming && streamBlockToolCalls.length > 0
      ? streamBlockToolCalls
      : (projection.toolCalls.length > 0 ? projection.toolCalls : streamBlockToolCalls);
    const lastDisplayToolName = displayToolCalls[displayToolCalls.length - 1]?.name || "";
    const activity = baseActivityForMessage(
      projection,
      !!meta._streaming,
      streamBlocks,
    );
    const dispatchElapsedMs = positiveNumberFromProviderMeta(meta, "dispatchElapsedMs");
    const frontendDispatchElapsedMs = positiveNumberFromProviderMeta(meta, "_frontendDispatchElapsedMs");
    const extraTextReferences = buildExtraTextReferences(message);
    const baseBlock = {
      id: message.id,
      rawMessage: message,
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
      contentBlocks: message.contentBlocks,
      mentions: projection.mentions,
      text: streamBlocks.length > 0
        ? streamingDisplayText
        : projection.text,
      images: projection.images,
      audios: projection.audios,
      attachmentFiles: projection.attachmentFiles,
      extraTextReferences: message.role === "user" ? extraTextReferences : [],
      taskTrigger: projection.taskTrigger,
      planCard: projection.planCard,
      remoteImOrigin: projection.remoteImOrigin,
      dispatchElapsedMs,
      frontendDispatchElapsedMs,
      toolCallCount: displayToolCalls.length,
      lastToolName: lastDisplayToolName,
      toolCalls: displayToolCalls,
      activityItems: activity.items,
      activityReasoningCharCount: activity.activityReasoningCharCount,
      activityToolCountsByName: activity.activityToolCountsByName,
      activityRunning: activity.activityRunning,
      activityStatus: activity.activityStatus,
    } satisfies ChatMessageBlock;
    const blocks: ChatMessageBlock[] = [];
    if (
      baseBlock.text
      || !!baseBlock.isStreaming
      || baseBlock.images.length > 0
      || baseBlock.audios.length > 0
      || baseBlock.attachmentFiles.length > 0
      || (baseBlock.extraTextReferences || []).length > 0
      || !!baseBlock.taskTrigger
      || !!baseBlock.planCard
      || baseBlock.activityItems.length > 0
      || baseBlock.activityRunning
    ) {
      blocks.push(baseBlock);
    }

    if (message.role !== "user" && Array.isArray(message.extraTextBlocks)) {
      message.extraTextBlocks.forEach((raw, index) => {
        const text = String(raw || "").trim();
        if (!text) return;
        blocks.push({
          id: `${message.id}::extra:${index}`,
          rawMessage: message,
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
          mentions: projection.mentions,
          text,
          images: [],
          audios: [],
          attachmentFiles: [],
          extraTextReferences: [],
          taskTrigger: undefined,
          planCard: undefined,
          remoteImOrigin: projection.remoteImOrigin,
          dispatchElapsedMs,
          toolCallCount: 0,
          lastToolName: "",
          toolCalls: [],
          activityItems: [],
          activityReasoningCharCount: 0,
          activityToolCountsByName: {},
          activityRunning: false,
          activityStatus: "idle",
        });
      });
    }

    messageBlockCache.set(message, { signature, blocks });
    return blocks;
  }

  /** 同一消息内经手多次的工具记录（预写占位 → 真实结果）只保留最后一条。 */
  function keepLastToolEntry<T>(
    items: T[],
    callIdOf: (item: T) => string,
  ): T[] {
    const lastIndexById = new Map<string, number>();
    items.forEach((item, index) => {
      const callId = String(callIdOf(item) || "").trim();
      if (!callId) return;
      lastIndexById.set(callId, index);
    });
    if (lastIndexById.size === 0) return items;
    return items.filter((item, index) => {
      const callId = String(callIdOf(item) || "").trim();
      if (!callId) return true;
      return lastIndexById.get(callId) === index;
    });
  }

  /**
   * 跨消息消解同一次工具调用。
   * 压缩后新建的 assistant 消息会继承上一轮的工具事件，同一条调用因此在时间线上出现两次：
   * 一份在压缩前的旧消息里，一份在新消息里。事件本身就带同一个 tool_call_id，
   * 这里按 id 只保留先出现的那份，后到的整条移除。
   * 分段补载时先出现的可能是新消息那份，规则不变，仍是先到先留。
   */
  function dedupeInheritedToolCalls(blocks: ChatMessageBlock[]): ChatMessageBlock[] {
    const seenToolCalls = new Set<string>();
    const seenActivityTools = new Set<string>();
    const result: ChatMessageBlock[] = [];
    for (const block of blocks) {
      const toolCalls = keepLastToolEntry(
        Array.isArray(block.toolCalls) ? block.toolCalls : [],
        (call) => call.toolCallId || "",
      );
      const activityItems = keepLastToolEntry(
        Array.isArray(block.activityItems) ? block.activityItems : [],
        (item) => (item.kind === "tool" ? item.toolCallId || "" : ""),
      );
      const nextToolCalls = toolCalls.filter((call) => {
        const callId = String(call.toolCallId || "").trim();
        if (!callId) return true;
        if (seenToolCalls.has(callId)) return false;
        seenToolCalls.add(callId);
        return true;
      });
      const nextActivityItems = activityItems.filter((item) => {
        if (item.kind !== "tool") return true;
        const callId = String(item.toolCallId || "").trim();
        if (!callId) return true;
        if (seenActivityTools.has(callId)) return false;
        seenActivityTools.add(callId);
        return true;
      });
      const untouched = nextToolCalls.length === (Array.isArray(block.toolCalls) ? block.toolCalls.length : 0)
        && nextActivityItems.length === (Array.isArray(block.activityItems) ? block.activityItems.length : 0);
      if (untouched) {
        result.push(block);
        continue;
      }
      const stillHasBody = !!String(block.text || "").trim()
        || !!block.isStreaming
        || (block.images || []).length > 0
        || (block.audios || []).length > 0
        || (block.attachmentFiles || []).length > 0
        || (block.extraTextReferences || []).length > 0
        || !!block.taskTrigger
        || !!block.planCard
        || nextActivityItems.length > 0;
      if (!stillHasBody) continue;
      const activityToolCountsByName: Record<string, number> = {};
      let activityReasoningCharCount = 0;
      for (const item of nextActivityItems) {
        if (item.kind === "reasoning") {
          activityReasoningCharCount += String(item.text || "").length;
          continue;
        }
        if (item.kind !== "tool") continue;
        const name = String(item.name || "").trim();
        if (!name) continue;
        activityToolCountsByName[name] = (activityToolCountsByName[name] || 0) + 1;
      }
      result.push({
        ...block,
        toolCalls: nextToolCalls,
        toolCallCount: nextToolCalls.length,
        lastToolName: nextToolCalls[nextToolCalls.length - 1]?.name || "",
        activityItems: nextActivityItems,
        activityReasoningCharCount,
        activityToolCountsByName,
      });
    }
    return result;
  }

  const allMessageBlocks = computed<ChatMessageBlock[]>(() => {
    const messages = options.allMessages.value;
    const signature = messages.map((message) => messageSignature(message)).join("||");
    if (signature === lastMessageBlockSignature) {
      return lastMessageBlocks;
    }
    const blocks = messages
      .flatMap((message) => buildMessageBlocks(message));
    const dedupedBlocks = dedupeInheritedToolCalls(blocks);
    lastMessageBlockSignature = signature;
    lastMessageBlocks = dedupedBlocks;
    return dedupedBlocks;
  });

  const visibleMessageBlocks = computed(() => allMessageBlocks.value);

  const chatContextUsageRatio = computed(() => {
    const previewRatio = previewContextUsageRatio();
    if (previewRatio !== null) {
      return previewRatio;
    }
    const api = options.activeChatApiConfig.value;
    const maxTokens = api
      ? Math.max(16000, Math.round(Number(api.contextWindowTokens ?? 256000)))
      : 0;
    const backendRatio = latestBackendContextUsageRatio(options.allMessages.value, maxTokens);
    if (backendRatio !== null) {
      return backendRatio;
    }
    if (!api) return 0;
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
