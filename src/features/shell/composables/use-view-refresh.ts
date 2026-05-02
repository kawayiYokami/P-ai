import { ref, type Ref } from "vue";

type ViewMode = "chat" | "archives" | "config";

type UseViewRefreshOptions = {
  viewMode: Ref<ViewMode>;
  loadConfig: () => Promise<void>;
  loadBootstrapSnapshot?: () => Promise<boolean>;
  loadPersonas: () => Promise<void>;
  loadChatSettings: () => Promise<void>;
  refreshImageCacheStats: () => Promise<void>;
  refreshConversationHistory: () => Promise<void>;
  loadDelegateConversations: () => Promise<void>;
  loadArchives: () => Promise<void>;
  resetVisibleTurnCount: () => void;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  onRefreshStepSlow?: (label: string, error: unknown) => void;
  onRefreshStepFailed?: (label: string, error: unknown) => void;
};

const VIEW_REFRESH_STEP_TIMEOUT_MS = 10_000;

function viewRefreshTimeoutError(label: string): Error {
  return new Error(`启动数据加载较慢：${label} 超过 ${VIEW_REFRESH_STEP_TIMEOUT_MS / 1000} 秒未完成，仍在等待。`);
}

async function runRefreshStep<T>(
  label: string,
  task: () => Promise<T>,
  onSlow?: (label: string, error: unknown) => void,
  onFailed?: (label: string, error: unknown) => void,
): Promise<T> {
  let timeoutReported = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    timer = setTimeout(() => {
      timeoutReported = true;
      const error = viewRefreshTimeoutError(label);
      console.warn(`[VIEW] refresh step slow: ${label}`, error);
      onSlow?.(label, error);
    }, VIEW_REFRESH_STEP_TIMEOUT_MS);
    return await task();
  } catch (error) {
    console.error(`[VIEW] refresh step failed: ${label}`, error);
    if (!timeoutReported) {
      onFailed?.(label, error);
    }
    throw error;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

export function useViewRefresh(options: UseViewRefreshOptions) {
  const suppressChatReloadWatch = ref(false);
  const windowBootstrapped = ref(false);

  async function refreshAllViewData() {
    suppressChatReloadWatch.value = true;
    const startedAt = options.perfNow();
    try {
      if (options.loadBootstrapSnapshot) {
        const tLoadBootstrap = options.perfNow();
        const bootstrapped = await runRefreshStep(
          "loadBootstrapSnapshot",
          () => options.loadBootstrapSnapshot?.() ?? Promise.resolve(false),
          options.onRefreshStepSlow,
          options.onRefreshStepFailed,
        );
        options.perfLog("refreshAll/loadBootstrapSnapshot", tLoadBootstrap);
        if (!bootstrapped) {
          throw new Error("loadBootstrapSnapshot failed");
        }
      } else {
      const tLoadConfig = options.perfNow();
      await runRefreshStep("loadConfig", options.loadConfig, options.onRefreshStepSlow, options.onRefreshStepFailed);
      options.perfLog("refreshAll/loadConfig", tLoadConfig);
      const tLoadPersonas = options.perfNow();
      await runRefreshStep("loadPersonas", options.loadPersonas, options.onRefreshStepSlow, options.onRefreshStepFailed);
      options.perfLog("refreshAll/loadPersonas", tLoadPersonas);
      const tLoadChatSettings = options.perfNow();
      await runRefreshStep("loadChatSettings", options.loadChatSettings, options.onRefreshStepSlow, options.onRefreshStepFailed);
      options.perfLog("refreshAll/loadChatSettings", tLoadChatSettings);
      }
      if (options.viewMode.value === "config") {
        const tRefreshCache = options.perfNow();
        await runRefreshStep("refreshImageCacheStats", options.refreshImageCacheStats, options.onRefreshStepSlow, options.onRefreshStepFailed);
        options.perfLog("refreshAll/refreshImageCacheStats", tRefreshCache);
      }
      if (options.viewMode.value === "chat") {
        const tMessages = options.perfNow();
        await runRefreshStep("refreshConversationHistory", options.refreshConversationHistory, options.onRefreshStepSlow, options.onRefreshStepFailed);
        options.perfLog("refreshAll/refreshConversationHistory", tMessages);
        const tDelegates = options.perfNow();
        await runRefreshStep("loadDelegateConversations", options.loadDelegateConversations, options.onRefreshStepSlow, options.onRefreshStepFailed);
        options.perfLog("refreshAll/loadDelegateConversations", tDelegates);
        options.resetVisibleTurnCount();
      } else if (options.viewMode.value === "archives") {
        const tMessages = options.perfNow();
        await runRefreshStep("refreshConversationHistory", options.refreshConversationHistory, options.onRefreshStepSlow, options.onRefreshStepFailed);
        options.perfLog("refreshAll/refreshConversationHistory", tMessages);
        const tArchives = options.perfNow();
        await runRefreshStep("loadArchives", options.loadArchives, options.onRefreshStepSlow, options.onRefreshStepFailed);
        options.perfLog("refreshAll/loadArchives", tArchives);
      }
    } finally {
      suppressChatReloadWatch.value = false;
      options.perfLog("refreshAll/total", startedAt);
    }
  }

  async function handleWindowRefreshSignal() {
    if (!windowBootstrapped.value) {
      try {
        await refreshAllViewData();
        windowBootstrapped.value = true;
      } catch (error) {
        console.error("[VIEW] window bootstrap refresh failed:", error);
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
