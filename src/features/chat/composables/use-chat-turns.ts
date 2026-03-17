import { computed, type ComputedRef, type Ref, type ShallowRef } from "vue";
import type { ApiConfigItem, ChatMessage, ChatMessageBlock, TaskTriggerMessageCard } from "../../../types/app";
import {
  estimateConversationTokens,
  extractMessageAttachmentFiles,
  extractMessageAudios,
  extractMessageImages,
  parseAssistantStoredText,
  removeBinaryPlaceholders,
  renderMessage,
} from "../../../utils/chat-message";

type UseChatMessageBlocksOptions = {
  allMessages: ShallowRef<ChatMessage[]>;
  visibleMessageBlockCount: Ref<number>;
  hasMoreBackendHistory: Ref<boolean>;
  activeChatApiConfig: ComputedRef<ApiConfigItem | null>;
  perfDebug: boolean;
  perfNow: () => number;
};

export function useChatMessageBlocks(options: UseChatMessageBlocksOptions) {
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

  function resolveSpeakerAgentId(message: ChatMessage): string {
    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    const origin = meta.origin as Record<string, unknown> | undefined;
    if (origin && origin.kind === "remote_im") {
      return "";
    }
    const direct = String(message.speakerAgentId || "").trim();
    if (direct) return direct;
    for (const key of [
      "speakerAgentId",
      "speaker_agent_id",
      "targetAgentId",
      "target_agent_id",
      "agentId",
      "agent_id",
      "sourceAgentId",
      "source_agent_id",
    ]) {
      const value = String(meta[key] || "").trim();
      if (value) return value;
    }
    return "";
  }

  function toolArgumentsPreview(raw: unknown): string {
    const text = typeof raw === "string" ? raw.trim() : "";
    if (!text) return "{}";
    const compact = text.replace(/\s+/g, " ");
    return compact.length > 180 ? `${compact.slice(0, 180)}...` : compact;
  }

  function summarizeToolHistory(
    toolHistory: ChatMessage["toolCall"],
  ): { count: number; lastToolName: string; calls: Array<{ name: string; argsText: string }> } {
    if (!Array.isArray(toolHistory) || toolHistory.length === 0) {
      return { count: 0, lastToolName: "", calls: [] };
    }
    const calls = [] as Array<{ name: string; argsText: string }>;
    let count = 0;
    let lastToolName = "";
    for (const event of toolHistory) {
      if (!event || event.role !== "assistant" || !Array.isArray(event.tool_calls)) continue;
      for (const call of event.tool_calls) {
        const name = String(call?.function?.name || "").trim();
        if (!name) continue;
        const argsText = toolArgumentsPreview(call?.function?.arguments);
        calls.push({ name, argsText });
        count += 1;
        lastToolName = name;
      }
    }
    return { count, lastToolName, calls };
  }

  function resolveTaskTrigger(message: ChatMessage): TaskTriggerMessageCard | undefined {
    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    if (String(meta.messageKind || "").trim() !== "task_trigger") return undefined;
    const raw = meta.taskTrigger;
    if (!raw || typeof raw !== "object") return undefined;
    const card = raw as Record<string, unknown>;
    const title = String(card.title || "").trim();
    if (!title) return undefined;
    return {
      taskId: String(card.taskId || "").trim() || undefined,
      title,
      cause: String(card.cause || "").trim() || undefined,
      goal: String(card.goal || "").trim() || undefined,
      flow: String(card.flow || "").trim() || undefined,
      statusSummary: String(card.statusSummary || "").trim() || undefined,
      todos: Array.isArray(card.todos) ? card.todos.map((item) => String(item || "").trim()).filter(Boolean) : [],
      runAt: String(card.runAt || "").trim() || undefined,
      endAt: String(card.endAt || "").trim() || undefined,
      nextRunAt: String(card.nextRunAt || "").trim() || undefined,
      everyMinutes: Number.isFinite(Number(card.everyMinutes)) ? Number(card.everyMinutes) : undefined,
    };
  }

  const allMessageBlocks = computed<ChatMessageBlock[]>(() => {
    const startedAt = options.perfNow();
    const messages = options.allMessages.value;
    const blocks = messages.map((message) => {
      const rendered = removeBinaryPlaceholders(renderMessage(message));
      const parsed = parseAssistantStoredText(rendered);
      const meta = (message.providerMeta || {}) as Record<string, unknown>;
      const toolSummary = summarizeToolHistory(message.toolCall);
      const streamSegments = Array.isArray(meta._streamSegments)
        ? (meta._streamSegments as unknown[])
          .map((item) => String(item ?? ""))
          .filter((item) => item.length > 0)
        : [];
      const streamTail = String(meta._streamTail ?? "");
      return {
        id: message.id,
        role: message.role,
        isStreaming: !!meta._streaming,
        streamSegments,
        streamTail,
        speakerAgentId: resolveSpeakerAgentId(message) || undefined,
        createdAt: String(message.createdAt || "").trim() || undefined,
        text: message.role === "assistant" ? parsed.assistantText : rendered,
        images: extractMessageImages(message),
        audios: extractMessageAudios(message),
        attachmentFiles: extractMessageAttachmentFiles(message),
        taskTrigger: resolveTaskTrigger(message),
        remoteImOrigin: (() => {
          const origin = meta.origin as Record<string, unknown> | undefined;
          if (!origin || origin.kind !== "remote_im") return undefined;
          return {
            senderName: String(origin.senderName || ""),
            remoteContactName: String(origin.remoteContactName || "") || undefined,
            remoteContactType: String(origin.remoteContactType || "private"),
            channelId: String(origin.channelId || ""),
            contactId: String(origin.contactId || ""),
          };
        })(),
        reasoningStandard:
          parsed.reasoningStandard
          || String(meta.reasoningStandard || "").trim(),
        reasoningInline:
          parsed.reasoningInline
          || String(meta.reasoningInline || "").trim(),
        toolCallCount: toolSummary.count,
        lastToolName: toolSummary.lastToolName,
        toolCalls: toolSummary.calls,
      };
    }).filter((block) =>
      block.text
      || !!block.isStreaming
      || block.images.length > 0
      || block.audios.length > 0
      || block.attachmentFiles.length > 0
      || !!block.taskTrigger
      || !!block.reasoningStandard
      || !!block.reasoningInline,
    );

    if (options.perfDebug) {
      const cost = Math.round((options.perfNow() - startedAt) * 10) / 10;
      console.log(`【性能】构建消息块 messages=${messages.length} blocks=${blocks.length} 完成 耗时=${cost}ms`);
    }
    return blocks;
  });

  // 无窗口模式：渲染层直接消费当前持有的全量消息块，不再做本地 slice。
  const visibleMessageBlocks = computed(() => allMessageBlocks.value);

  // 是否还能继续加载更早消息，交由后端游标标志（hasMoreBackendHistory）控制。
  const hasMoreMessageBlocks = computed(() => !!options.hasMoreBackendHistory.value);

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
    hasMoreMessageBlocks,
    chatContextUsageRatio,
    chatUsagePercent,
  };
}
