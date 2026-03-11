import { computed, type ComputedRef, type Ref, type ShallowRef } from "vue";
import type { ApiConfigItem, ChatMessage, ChatMessageBlock, TaskTriggerMessageCard } from "../../../types/app";
import {
  estimateConversationTokens,
  extractMessageAudios,
  extractMessageImages,
  parseAssistantStoredText,
  removeBinaryPlaceholders,
  renderMessage,
} from "../../../utils/chat-message";

type UseChatMessageBlocksOptions = {
  allMessages: ShallowRef<ChatMessage[]>;
  visibleMessageBlockCount: Ref<number>;
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
    const direct = String(message.speakerAgentId || "").trim();
    if (direct) return direct;
    const meta = (message.providerMeta || {}) as Record<string, unknown>;
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

  function summarizeToolHistory(
    toolHistory: ChatMessage["toolCall"],
  ): { count: number; lastToolName: string } {
    if (!Array.isArray(toolHistory) || toolHistory.length === 0) {
      return { count: 0, lastToolName: "" };
    }
    let count = 0;
    let lastToolName = "";
    for (const event of toolHistory) {
      if (!event || event.role !== "assistant" || !Array.isArray(event.tool_calls)) continue;
      for (const call of event.tool_calls) {
        const name = String(call?.function?.name || "").trim();
        if (!name) continue;
        count += 1;
        lastToolName = name;
      }
    }
    return { count, lastToolName };
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

  function latestUserAnchorIndex(messages: ChatMessage[]): number {
    for (let idx = messages.length - 1; idx >= 0; idx -= 1) {
      const item = messages[idx];
      if (item.role !== "user") continue;
      const speaker = resolveSpeakerAgentId(item);
      if (!speaker || speaker === "user-persona") return idx;
    }
    return 0;
  }

  const allMessageBlocks = computed<ChatMessageBlock[]>(() => {
    const startedAt = options.perfNow();
    const messages = options.allMessages.value;
    const startIndex = latestUserAnchorIndex(messages);
    const blocks = messages.slice(startIndex).map((message) => {
      const rendered = removeBinaryPlaceholders(renderMessage(message));
      const parsed = parseAssistantStoredText(rendered);
      const meta = (message.providerMeta || {}) as Record<string, unknown>;
      const toolSummary = summarizeToolHistory(message.toolCall);
      return {
        id: message.id,
        role: message.role,
        speakerAgentId: resolveSpeakerAgentId(message) || undefined,
        createdAt: String(message.createdAt || "").trim() || undefined,
        text: message.role === "assistant" ? parsed.assistantText : rendered,
        images: extractMessageImages(message),
        audios: extractMessageAudios(message),
        taskTrigger: resolveTaskTrigger(message),
        reasoningStandard:
          parsed.reasoningStandard
          || String(meta.reasoningStandard || "").trim(),
        reasoningInline:
          parsed.reasoningInline
          || String(meta.reasoningInline || "").trim(),
        toolCallCount: toolSummary.count,
        lastToolName: toolSummary.lastToolName,
      };
    }).filter((block) =>
      block.text
      || block.images.length > 0
      || block.audios.length > 0
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

  const visibleMessageBlocks = computed(() =>
    allMessageBlocks.value.slice(Math.max(0, allMessageBlocks.value.length - options.visibleMessageBlockCount.value)),
  );

  const hasMoreMessageBlocks = computed(() => options.visibleMessageBlockCount.value < allMessageBlocks.value.length);

  const chatContextUsageRatio = computed(() => {
    const backendPercent = latestBackendContextUsagePercent(options.allMessages.value);
    if (backendPercent !== null) {
      return backendPercent / 100;
    }
    const api = options.activeChatApiConfig.value;
    if (!api) return 0;
    const maxTokens = Math.max(16000, Math.min(200000, Number(api.contextWindowTokens ?? 128000)));
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
