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
  function resolveAssistantAgentId(message: ChatMessage): string {
    const direct = String(message.speakerAgentId || "").trim();
    if (direct) return direct;
    const meta = (message.providerMeta || {}) as Record<string, unknown>;
    for (const key of ["speakerAgentId", "speaker_agent_id", "agentId", "agent_id"]) {
      const value = String(meta[key] || "").trim();
      if (value) return value;
    }
    return "";
  }

  function summarizeAssistantToolHistory(
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


  function resolvePrimaryTaskTrigger(message: ChatMessage): TaskTriggerMessageCard | undefined {
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
    const msgs = options.allMessages.value;
    const turns: ChatMessageBlock[] = [];
    for (let i = 0; i < msgs.length; i++) {
      const msg = msgs[i];
      if (msg.role === "user") {
        const primaryText = removeBinaryPlaceholders(renderMessage(msg));
        const primaryAgentId = String(msg.speakerAgentId || "").trim();
        const primaryImages = extractMessageImages(msg);
        const primaryAudios = extractMessageAudios(msg);
        const primaryTaskTrigger = resolvePrimaryTaskTrigger(msg);
        let replyText = "";
        let replyCreatedAt = "";
        let replyReasoningStandard = "";
        let replyReasoningInline = "";
        let replyToolCallCount = 0;
        let replyLastToolName = "";
        let replyAgentId = "";
        if (i + 1 < msgs.length && msgs[i + 1].role === "assistant") {
          const assistantMsg = msgs[i + 1];
          const parsed = parseAssistantStoredText(renderMessage(assistantMsg));
          const providerMeta = assistantMsg.providerMeta || {};
          replyCreatedAt = String(assistantMsg.createdAt || "").trim();
          const toolSummary = summarizeAssistantToolHistory(assistantMsg.toolCall);
          replyText = parsed.assistantText;
          replyReasoningStandard = parsed.reasoningStandard || String(providerMeta.reasoningStandard || "");
          replyReasoningInline = parsed.reasoningInline || String(providerMeta.reasoningInline || "");
          replyToolCallCount = toolSummary.count;
          replyLastToolName = toolSummary.lastToolName;
          replyAgentId = resolveAssistantAgentId(assistantMsg);
          i++;
        }
        if (
          primaryText
          || primaryImages.length > 0
          || primaryAudios.length > 0
          || !!primaryTaskTrigger
          || replyText.trim()
          || replyReasoningStandard.trim()
          || replyReasoningInline.trim()
        ) {
          turns.push({
            id: msg.id,
            primaryAgentId,
            replyAgentId,
            primaryCreatedAt: String(msg.createdAt || "").trim(),
            replyCreatedAt,
            primaryText,
            primaryImages,
            primaryAudios,
            primaryTaskTrigger,
            replyText,
            replyReasoningStandard,
            replyReasoningInline,
            replyToolCallCount,
            replyLastToolName,
          });
        }
      }
    }
    if (options.perfDebug) {
      const cost = Math.round((options.perfNow() - startedAt) * 10) / 10;
      console.log(`【性能】构建消息块 messages=${msgs.length} turns=${turns.length} 状态=完成 耗时=${cost}ms`);
    }
    return turns;
  });

  const visibleMessageBlocks = computed(() =>
    allMessageBlocks.value.slice(Math.max(0, allMessageBlocks.value.length - options.visibleMessageBlockCount.value))
  );

  const hasMoreMessageBlocks = computed(() => options.visibleMessageBlockCount.value < allMessageBlocks.value.length);

  const chatContextUsageRatio = computed(() => {
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




