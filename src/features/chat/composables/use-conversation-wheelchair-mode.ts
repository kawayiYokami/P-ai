import { computed, onScopeDispose, ref, type Ref } from "vue";
import { invokeTauri, onTransportNotification } from "../../../services/tauri-api";

type ConversationWheelchairModeSummary = {
  conversationId: string;
  wheelchairModeEnabled?: boolean;
};

export type WheelchairSuggestionPayload = {
  conversationId?: string;
  type?: "suggest" | "options";
  text?: string;
  options?: string[];
};

export type WheelchairSuggestion = {
  type: "suggest" | "options";
  text: string;
  options: string[];
};

type UseConversationWheelchairModeOptions<T extends ConversationWheelchairModeSummary> = {
  currentConversationId: Ref<string>;
  unarchivedConversations: Ref<T[]>;
  /**
   * 宿主侧可选注入的 suggestion ref。
   * 追问视图（ConversationView）没有 unarchivedConversations 总表，
   * 它的开关来自 use-conversation-view-runtime 的 snapshot ref，
   * 建议气泡也由 runtime 持有；主窗口则走本 composable 内部的默认 ref。
   */
  wheelchairSuggestion?: Ref<WheelchairSuggestion | null>;
};

export function useConversationWheelchairMode<T extends ConversationWheelchairModeSummary>(
  options: UseConversationWheelchairModeOptions<T>,
) {
  const internalSuggestion = ref<WheelchairSuggestion | null>(null);
  const suggestionRef = options.wheelchairSuggestion ?? internalSuggestion;

  function getConversationWheelchairModeEnabledById(conversationId: string): boolean {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return false;
    return !!options.unarchivedConversations.value.find((item) =>
      String(item.conversationId || "").trim() === normalizedConversationId
    )?.wheelchairModeEnabled;
  }

  function patchConversationWheelchairModeInOverview(conversationId: string, enabled: boolean) {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return;
    let changed = false;
    const next = options.unarchivedConversations.value.map((item) => {
      if (String(item.conversationId || "").trim() !== normalizedConversationId) {
        return item;
      }
      if (!!item.wheelchairModeEnabled === !!enabled) {
        return item;
      }
      changed = true;
      return {
        ...item,
        wheelchairModeEnabled: !!enabled,
      };
    });
    if (changed) {
      options.unarchivedConversations.value = next;
    }
  }

  async function setConversationWheelchairMode(conversationId: string, value: boolean): Promise<boolean> {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return false;
    const nextValue = !!value;
    const previousValue = getConversationWheelchairModeEnabledById(normalizedConversationId);
    if (previousValue === nextValue) return true;
    patchConversationWheelchairModeInOverview(normalizedConversationId, nextValue);
    try {
      await invokeTauri<{ conversationId: string; wheelchairModeEnabled: boolean }>(
        "conversation.wheelchairMode.set",
        {
          input: {
            conversationId: normalizedConversationId,
            wheelchairModeEnabled: nextValue,
          },
        },
      );
      return true;
    } catch (error) {
      if (getConversationWheelchairModeEnabledById(normalizedConversationId) === nextValue) {
        patchConversationWheelchairModeInOverview(normalizedConversationId, previousValue);
      }
      console.warn("[轮椅模式] 切换会话轮椅模式失败", {
        conversationId: normalizedConversationId,
        nextValue,
        error,
      });
      return false;
    }
  }

  async function setCurrentConversationWheelchairMode(value: boolean): Promise<boolean> {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) return false;
    return setConversationWheelchairMode(conversationId, value);
  }

  function dismissWheelchairSuggestion() {
    suggestionRef.value = null;
  }

  const stopWheelchairSuggestionEvent = onTransportNotification<WheelchairSuggestionPayload>(
    "conversation.wheelchairSuggestion",
    (payload) => {
      const conversationId = String(payload?.conversationId || "").trim();
      const currentConversationId = String(options.currentConversationId.value || "").trim();
      if (!conversationId || conversationId !== currentConversationId) return;
      const type = payload?.type === "options" ? "options" : "suggest";
      const text = String(payload?.text || "");
      const items = Array.isArray(payload?.options) ? payload!.options!.map((item) => String(item || "")) : [];
      suggestionRef.value = { type, text, options: items };
    },
  );

  onScopeDispose(() => {
    stopWheelchairSuggestionEvent();
  });

  const currentConversationWheelchairModeEnabled = computed(() => {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) return false;
    return getConversationWheelchairModeEnabledById(conversationId);
  });

  return {
    currentConversationWheelchairModeEnabled,
    wheelchairSuggestion: suggestionRef,
    setConversationWheelchairMode,
    setCurrentConversationWheelchairMode,
    updateWheelchairModeEnabled: setCurrentConversationWheelchairMode,
    dismissWheelchairSuggestion,
  };
}
