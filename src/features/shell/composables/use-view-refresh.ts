import { ref, type Ref } from "vue";

type ViewMode = "chat" | "archives" | "config";

type UseViewRefreshOptions = {
  viewMode: Ref<ViewMode>;
  recordHotkeySuppressAfterPopup: (ms: number) => void;
  recordHotkeySuppressMs: number;
  configAutosaveReady: Ref<boolean>;
  personasAutosaveReady: Ref<boolean>;
  chatSettingsAutosaveReady: Ref<boolean>;
  loadConfig: () => Promise<void>;
  loadPersonas: () => Promise<void>;
  loadChatSettings: () => Promise<void>;
  refreshImageCacheStats: () => Promise<void>;
  refreshConversationHistory: () => Promise<void>;
  loadDelegateConversations: () => Promise<void>;
  loadArchives: () => Promise<void>;
  resetVisibleTurnCount: () => void;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
};

export function useViewRefresh(options: UseViewRefreshOptions) {
  const suppressChatReloadWatch = ref(false);
  const windowBootstrapped = ref(false);

  async function refreshAllViewData() {
    suppressChatReloadWatch.value = true;
    const startedAt = options.perfNow();
    try {
      const tLoadConfig = options.perfNow();
      await options.loadConfig();
      options.perfLog("refreshAll/loadConfig", tLoadConfig);
      const tLoadPersonas = options.perfNow();
      await options.loadPersonas();
      options.perfLog("refreshAll/loadPersonas", tLoadPersonas);
      const tLoadChatSettings = options.perfNow();
      await options.loadChatSettings();
      options.perfLog("refreshAll/loadChatSettings", tLoadChatSettings);
      if (options.viewMode.value === "config") {
        const tRefreshCache = options.perfNow();
        await options.refreshImageCacheStats();
        options.perfLog("refreshAll/refreshImageCacheStats", tRefreshCache);
      }
      if (options.viewMode.value === "chat") {
        const tMessages = options.perfNow();
        await options.refreshConversationHistory();
        options.perfLog("refreshAll/refreshConversationHistory", tMessages);
        const tDelegates = options.perfNow();
        await options.loadDelegateConversations();
        options.perfLog("refreshAll/loadDelegateConversations", tDelegates);
        options.resetVisibleTurnCount();
      } else if (options.viewMode.value === "archives") {
        const tMessages = options.perfNow();
        await options.refreshConversationHistory();
        options.perfLog("refreshAll/refreshConversationHistory", tMessages);
        const tArchives = options.perfNow();
        await options.loadArchives();
        options.perfLog("refreshAll/loadArchives", tArchives);
      }
    } finally {
      suppressChatReloadWatch.value = false;
      options.perfLog("refreshAll/total", startedAt);
    }
  }

  async function handleWindowRefreshSignal() {
    options.recordHotkeySuppressAfterPopup(options.recordHotkeySuppressMs);
    if (!windowBootstrapped.value) {
      options.configAutosaveReady.value = false;
      options.personasAutosaveReady.value = false;
      options.chatSettingsAutosaveReady.value = false;
      try {
        await refreshAllViewData();
        windowBootstrapped.value = true;
        options.configAutosaveReady.value = true;
        options.personasAutosaveReady.value = true;
        options.chatSettingsAutosaveReady.value = true;
      } catch (error) {
        console.error("[VIEW] window bootstrap refresh failed:", error);
        options.configAutosaveReady.value = false;
        options.personasAutosaveReady.value = false;
        options.chatSettingsAutosaveReady.value = false;
      }
      return;
    }
    if (options.viewMode.value === "chat") {
      await options.refreshConversationHistory();
      await options.loadDelegateConversations();
    } else if (options.viewMode.value === "config") {
      await refreshAllViewData();
    } else if (options.viewMode.value === "archives") {
      await options.refreshConversationHistory();
      await options.loadArchives();
    }
  }

  return {
    suppressChatReloadWatch,
    refreshAllViewData,
    handleWindowRefreshSignal,
  };
}
