import { computed, nextTick, ref, watch, type Ref } from "vue";
import type { ConfigSearchTab, ConfigSearchResult } from "../../config/search/config-search";
import type { ChatMentionTarget } from "../../../types/app";
import type { ConversationPipelineStatus } from "../../shell/composables/use-pipeline-status";
import type { searchConfigTabs } from "../../config/search/config-search";
import { invokeTauri } from "../../../services/tauri-api";
import {
  loadStoredChatLeftPanelMode,
  loadStoredChatMonitorPanelMode,
  loadStoredChatRightPanelMode,
  loadStoredChatSidePanelVisibility,
  loadStoredChatSidePanelWidths,
  loadStoredConversationListTab,
  normalizeChatLeftPanelMode,
  normalizeChatMonitorPanelMode,
  normalizeChatRightPanelMode,
  normalizeChatSidePanelWidths,
  storeChatLeftPanelMode,
  storeChatMonitorPanelMode,
  storeChatRightPanelMode,
  storeChatSidePanelVisibility,
  storeChatSidePanelWidths,
  storeConversationListTab,
  type ChatLeftPanelMode,
  type ChatMonitorPanelMode,
  type ChatRightPanelMode,
} from "./chat-ui-layout-storage";
import type { ChatWindowPaneSide } from "./use-chat-window-pane-expansion";

type ChatWindowPaneExpansionBindings = {
  beforeOpen: (side: ChatWindowPaneSide, width: number) => Promise<void>;
  afterOpen: () => Promise<void>;
  beforeClose: () => Promise<void>;
  afterClose: (side: ChatWindowPaneSide) => Promise<void>;
};

export type ChatUiStateBindings = {
  viewMode: Ref<"chat" | "archives" | "config">;
  currentChatConversationId: Ref<string>;
  clearConversationStatus: (conversationId: string, status?: ConversationPipelineStatus) => void;
  searchConfigTabs: typeof searchConfigTabs;
  resolveConfigLocale: () => Parameters<typeof searchConfigTabs>[1];
  windowPaneExpansion?: ChatWindowPaneExpansionBindings;
};

export function useChatUiStateOrchestrator(bindings: ChatUiStateBindings) {
  const configTab = ref<ConfigSearchTab>("hotkey");
  const configSearchQuery = ref("");
  const selectedChatMentions = ref<ChatMentionTarget[]>([]);
  const chatInput = ref("");

  const conversationListTab = ref<ChatLeftPanelMode>(loadStoredConversationListTab());
  const chatLeftPanelMode = ref<ChatLeftPanelMode>(loadStoredChatLeftPanelMode());
  const chatRightPanelMode = ref<ChatRightPanelMode>("home");
  const chatMonitorPanelMode = ref<ChatMonitorPanelMode>("delegate");
  const sideConversationListVisible = ref(loadStoredChatSidePanelVisibility("left"));
  const toolReviewPanelOpenVisible = ref(loadStoredChatSidePanelVisibility("right"));
  const chatSidePanelWidths = ref(loadStoredChatSidePanelWidths());
  let openingChatReaderPanel: Promise<void> | null = null;

  const conversationChatErrorTextMap = ref<Record<string, string>>({});
  const fallbackChatErrorText = ref("");

  async function setSidePanelVisibility(side: ChatWindowPaneSide, visible: boolean) {
    const expansion = bindings.windowPaneExpansion;
    const width = side === "left"
      ? chatSidePanelWidths.value.leftWidth
      : chatSidePanelWidths.value.rightWidth;
    if (visible) {
      await expansion?.beforeOpen(side, width);
      if (side === "left") {
        sideConversationListVisible.value = true;
        storeChatSidePanelVisibility("left", true);
      } else {
        toolReviewPanelOpenVisible.value = true;
        storeChatSidePanelVisibility("right", true);
      }
      await nextTick();
      await expansion?.afterOpen();
      return;
    }
    await expansion?.beforeClose();
    const shouldStoreRightVisibility = side === "left"
      || visible
      || String(bindings.currentChatConversationId.value || "").trim();
    if (side === "left") {
      sideConversationListVisible.value = false;
      storeChatSidePanelVisibility("left", false);
    } else {
      toolReviewPanelOpenVisible.value = false;
      if (shouldStoreRightVisibility) storeChatSidePanelVisibility("right", false);
    }
    await nextTick();
    await expansion?.afterClose(side);
  }

  function getConversationChatErrorText(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return fallbackChatErrorText.value;
    return conversationChatErrorTextMap.value[cid] || "";
  }

  function setConversationChatErrorText(conversationId: string, text: string) {
    const cid = String(conversationId || "").trim();
    const normalizedText = String(text || "");
    if (!cid) {
      fallbackChatErrorText.value = normalizedText;
      return;
    }
    const next = { ...conversationChatErrorTextMap.value };
    if (normalizedText.trim()) {
      next[cid] = normalizedText;
    } else {
      delete next[cid];
    }
    conversationChatErrorTextMap.value = next;
  }

  function clearMatchingConversationChatErrors(predicate: (text: string) => boolean) {
    let changed = false;
    const next: Record<string, string> = {};
    for (const [conversationId, text] of Object.entries(conversationChatErrorTextMap.value)) {
      if (predicate(text)) {
        changed = true;
        continue;
      }
      next[conversationId] = text;
    }
    if (changed) {
      conversationChatErrorTextMap.value = next;
    }
    if (predicate(fallbackChatErrorText.value)) {
      fallbackChatErrorText.value = "";
    }
  }

  const chatErrorText = computed({
    get: () => getConversationChatErrorText(bindings.currentChatConversationId.value),
    set: (text: string) => {
      setConversationChatErrorText(bindings.currentChatConversationId.value, text);
    },
  });

  function clearChatError() {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    // 本地立即清除，不等后端广播回环；后端清除后所有端同步消失。
    setConversationChatErrorText(conversationId, "");
    bindings.clearConversationStatus(conversationId, "error");
    if (!conversationId) return;
    void invokeTauri("conversation.clearChatError", { input: { conversationId } }).catch((error) => {
      console.warn("[会话错误] 清除失败", { conversationId, error });
    });
  }

  function handleChatInputUpdate(value: string) {
    chatInput.value = value;
  }

  function updateConfigSearchQuery(value: string) {
    configSearchQuery.value = String(value || "");
  }

  function handleSelectConfigSearchResult(tab: ConfigSearchTab) {
    configTab.value = tab;
    configSearchQuery.value = "";
  }

  function addChatMention(value: ChatMentionTarget) {
    const agentId = String(value?.agentId || "").trim();
    const agentName = String(value?.agentName || "").trim();
    if (!agentId || !agentName) return;
    if (selectedChatMentions.value.some((item) => item.agentId === agentId)) return;
    selectedChatMentions.value = [
      ...selectedChatMentions.value,
      {
        agentId,
        agentName,
        avatarUrl: String(value?.avatarUrl || "").trim() || undefined,
      },
    ];
  }

  function removeChatMention(value: string | { agentId?: string }) {
    const normalizedAgentId =
      typeof value === "string"
        ? String(value || "").trim()
        : String(value?.agentId || "").trim();
    selectedChatMentions.value = selectedChatMentions.value.filter(
      (item) => item.agentId !== normalizedAgentId,
    );
  }

  function handleSideConversationListVisibleChange(value: boolean) {
    void setSidePanelVisibility("left", value);
  }

  function handleToolReviewPanelOpenChange(value: boolean) {
    void setSidePanelVisibility("right", value);
  }

  function updateConversationListTab(value: ChatLeftPanelMode) {
    conversationListTab.value = normalizeChatLeftPanelMode(value);
    chatLeftPanelMode.value = conversationListTab.value;
    storeConversationListTab(conversationListTab.value);
    storeChatLeftPanelMode(chatLeftPanelMode.value);
  }

  function updateChatLeftPanelMode(value: ChatLeftPanelMode) {
    const nextMode = normalizeChatLeftPanelMode(value);
    chatLeftPanelMode.value = nextMode;
    conversationListTab.value = nextMode;
    storeChatLeftPanelMode(nextMode);
    storeConversationListTab(nextMode);
    if (!sideConversationListVisible.value && bindings.viewMode.value === "chat") {
      void setSidePanelVisibility("left", true);
    }
  }

  function updateChatRightPanelMode(value: ChatRightPanelMode) {
    const nextMode = normalizeChatRightPanelMode(value, "home");
    chatRightPanelMode.value = nextMode;
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (conversationId) {
      storeChatRightPanelMode(nextMode, conversationId);
    }
    if (!toolReviewPanelOpenVisible.value && bindings.viewMode.value === "chat") {
      void setSidePanelVisibility("right", true);
    }
  }

  function openChatReaderPanel() {
    if (openingChatReaderPanel) return openingChatReaderPanel;
    openingChatReaderPanel = (async () => {
      const conversationId = String(bindings.currentChatConversationId.value || "").trim();
      chatRightPanelMode.value = "reader";
      if (conversationId) {
        storeChatRightPanelMode("reader", conversationId);
      }
      if (!toolReviewPanelOpenVisible.value) {
        await setSidePanelVisibility("right", true);
      }
      await nextTick();
    })().finally(() => {
      openingChatReaderPanel = null;
    });
    return openingChatReaderPanel;
  }

  function updateChatMonitorPanelMode(value: ChatMonitorPanelMode) {
    const nextMode = normalizeChatMonitorPanelMode(value, "delegate");
    chatMonitorPanelMode.value = nextMode;
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (conversationId) {
      storeChatMonitorPanelMode(nextMode, conversationId);
    }
  }

  function handleChatSidePanelWidthsChange(value: { leftWidth: number; rightWidth: number }, options?: { commit?: boolean; syncWindow?: boolean }) {
    chatSidePanelWidths.value = normalizeChatSidePanelWidths(value);
    if (options?.commit) {
      storeChatSidePanelWidths(chatSidePanelWidths.value);
    }
  }

  async function toggleSideConversationList() {
    const nextVisible = !sideConversationListVisible.value;
    await setSidePanelVisibility("left", nextVisible);
  }

  async function toggleToolReviewPanel() {
    const nextVisible = !toolReviewPanelOpenVisible.value;
    await setSidePanelVisibility("right", nextVisible);
  }

  const configSearchResults = computed<ConfigSearchResult[]>(() => {
    if (bindings.viewMode.value !== "config") return [];
    return bindings.searchConfigTabs(configSearchQuery.value, bindings.resolveConfigLocale());
  });

  watch(
    () => String(bindings.currentChatConversationId.value || "").trim(),
    (conversationId) => {
      selectedChatMentions.value = [];
      chatRightPanelMode.value = conversationId
        ? loadStoredChatRightPanelMode("home", conversationId)
        : "home";
      const storedMonitorPanelMode = conversationId
        ? loadStoredChatMonitorPanelMode("delegate", conversationId)
        : "delegate";
      chatMonitorPanelMode.value = storedMonitorPanelMode;
      if (conversationId) storeChatMonitorPanelMode(storedMonitorPanelMode, conversationId);
    },
    { immediate: true },
  );

  return {
    configTab,
    configSearchQuery,
    configSearchResults,
    selectedChatMentions,
    chatInput,
    conversationListTab,
    chatLeftPanelMode,
    chatRightPanelMode,
    chatMonitorPanelMode,
    sideConversationListVisible,
    toolReviewPanelOpenVisible,
    chatSidePanelWidths,
    chatErrorText,
    handleChatInputUpdate,
    updateConfigSearchQuery,
    handleSelectConfigSearchResult,
    addChatMention,
    removeChatMention,
    handleSideConversationListVisibleChange,
    handleToolReviewPanelOpenChange,
    updateConversationListTab,
    updateChatLeftPanelMode,
    updateChatRightPanelMode,
    openChatReaderPanel,
    updateChatMonitorPanelMode,
    handleChatSidePanelWidthsChange,
    toggleSideConversationList,
    toggleToolReviewPanel,
    setConversationChatErrorText,
    clearMatchingConversationChatErrors,
    clearChatError,
  };
}
