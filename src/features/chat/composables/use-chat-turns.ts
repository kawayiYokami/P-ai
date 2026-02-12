import { computed, type ComputedRef, type Ref, type ShallowRef } from "vue";
import type { ApiConfigItem, ChatMessage, ChatTurn } from "../../../types/app";
import {
  estimateConversationTokens,
  extractMessageAudios,
  extractMessageImages,
  parseAssistantStoredText,
  removeBinaryPlaceholders,
  renderMessage,
} from "../../../utils/chat-message";

type UseChatTurnsOptions = {
  allMessages: ShallowRef<ChatMessage[]>;
  visibleTurnCount: Ref<number>;
  activeChatApiConfig: ComputedRef<ApiConfigItem | null>;
  perfDebug: boolean;
  perfNow: () => number;
};

export function useChatTurns(options: UseChatTurnsOptions) {
  const allTurns = computed<ChatTurn[]>(() => {
    const startedAt = options.perfNow();
    const msgs = options.allMessages.value;
    const turns: ChatTurn[] = [];
    for (let i = 0; i < msgs.length; i++) {
      const msg = msgs[i];
      if (msg.role === "user") {
        const userText = removeBinaryPlaceholders(renderMessage(msg));
        const userImages = extractMessageImages(msg);
        const userAudios = extractMessageAudios(msg);
        let assistantText = "";
        let assistantReasoningStandard = "";
        let assistantReasoningInline = "";
        if (i + 1 < msgs.length && msgs[i + 1].role === "assistant") {
          const assistantMsg = msgs[i + 1];
          const parsed = parseAssistantStoredText(renderMessage(assistantMsg));
          const providerMeta = assistantMsg.providerMeta || {};
          assistantText = parsed.assistantText;
          assistantReasoningStandard = parsed.reasoningStandard || String(providerMeta.reasoningStandard || "");
          assistantReasoningInline = parsed.reasoningInline || String(providerMeta.reasoningInline || "");
          i++;
        }
        if (
          userText
          || userImages.length > 0
          || userAudios.length > 0
          || assistantText.trim()
          || assistantReasoningStandard.trim()
          || assistantReasoningInline.trim()
        ) {
          turns.push({
            id: msg.id,
            userText,
            userImages,
            userAudios,
            assistantText,
            assistantReasoningStandard,
            assistantReasoningInline,
          });
        }
      }
    }
    if (options.perfDebug) {
      const cost = Math.round((options.perfNow() - startedAt) * 10) / 10;
      console.log(`[PERF] buildAllTurns messages=${msgs.length} turns=${turns.length} cost=${cost}ms`);
    }
    return turns;
  });

  const visibleTurns = computed(() =>
    allTurns.value.slice(Math.max(0, allTurns.value.length - options.visibleTurnCount.value))
  );

  const hasMoreTurns = computed(() => options.visibleTurnCount.value < allTurns.value.length);

  const chatContextUsageRatio = computed(() => {
    const api = options.activeChatApiConfig.value;
    if (!api) return 0;
    const maxTokens = Math.max(16000, Math.min(200000, Number(api.contextWindowTokens ?? 128000)));
    const used = estimateConversationTokens(options.allMessages.value);
    return used / Math.max(1, maxTokens);
  });

  const chatUsagePercent = computed(() => Math.min(100, Math.max(0, Math.round(chatContextUsageRatio.value * 100))));

  return {
    allTurns,
    visibleTurns,
    hasMoreTurns,
    chatContextUsageRatio,
    chatUsagePercent,
  };
}
