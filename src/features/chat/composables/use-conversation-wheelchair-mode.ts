import { computed, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

type ConversationWheelchairSuggestionPayload = {
  type?: string;
  text?: string;
  options?: string[];
};

type ConversationWheelchairModeSummary = {
  conversationId: string;
  wheelchairModeEnabled?: boolean;
  wheelchairSuggestion?: ConversationWheelchairSuggestionPayload | null;
};

export type WheelchairSuggestion = {
  type: "suggest" | "options";
  text: string;
  options: string[];
};

type UseConversationWheelchairModeOptions<T extends ConversationWheelchairModeSummary> = {
  currentConversationId: Ref<string>;
  unarchivedConversations: Ref<T[]>;
};

/**
 * 护航的开关和建议条都挂在会话运行时内存账本上，由概览项带下来。
 * 建议只在用户点选项或直接发言时消失；切换会话、刷新界面都不会清掉。
 */
export function useConversationWheelchairMode<T extends ConversationWheelchairModeSummary>(
  options: UseConversationWheelchairModeOptions<T>,
) {
  function findConversationById(conversationId: string): T | undefined {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return undefined;
    return options.unarchivedConversations.value.find(
      (item) => String(item.conversationId || "").trim() === normalizedConversationId,
    );
  }

  function getConversationWheelchairModeEnabledById(conversationId: string): boolean {
    return !!findConversationById(conversationId)?.wheelchairModeEnabled;
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

  function patchConversationWheelchairSuggestionInOverview(
    conversationId: string,
    suggestion: ConversationWheelchairSuggestionPayload | null,
  ) {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return;
    let changed = false;
    const next = options.unarchivedConversations.value.map((item) => {
      if (String(item.conversationId || "").trim() !== normalizedConversationId) {
        return item;
      }
      if (!item.wheelchairSuggestion === !suggestion) {
        return item;
      }
      changed = true;
      return {
        ...item,
        wheelchairSuggestion: suggestion,
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
      console.warn("[护航] 切换会话护航失败", {
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

  async function dismissWheelchairSuggestion(): Promise<void> {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) return;
    // 先本地清掉让界面立刻响应；后端账本同步清空，切走再回来不会复活。
    patchConversationWheelchairSuggestionInOverview(conversationId, null);
    try {
      await invokeTauri<{ conversationId: string; dismissed: boolean }>(
        "conversation.wheelchairSuggestion.dismiss",
        {
          input: { conversationId },
        },
      );
    } catch (error) {
      console.warn("[护航] 清掉建议条失败", {
        conversationId,
        error,
      });
    }
  }

  const currentConversationWheelchairModeEnabled = computed(() => {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) return false;
    return getConversationWheelchairModeEnabledById(conversationId);
  });

  const wheelchairSuggestion = computed<WheelchairSuggestion | null>(() => {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) return null;
    const raw = findConversationById(conversationId)?.wheelchairSuggestion;
    if (!raw) return null;
    const type = raw.type === "options" ? "options" : "suggest";
    const text = String(raw.text || "");
    const items = Array.isArray(raw.options) ? raw.options.map((item) => String(item || "")) : [];
    if (!text.trim() && items.length === 0) return null;
    return { type, text, options: items };
  });

  return {
    currentConversationWheelchairModeEnabled,
    wheelchairSuggestion,
    setConversationWheelchairMode,
    setCurrentConversationWheelchairMode,
    updateWheelchairModeEnabled: setCurrentConversationWheelchairMode,
    dismissWheelchairSuggestion,
  };
}
