import type { ComputedRef, Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type {
  ApiConfigItem,
  ImageTextCacheStats,
  PersonaProfile,
  ToolLoadStatus,
} from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseConfigRuntimeOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  personas: Ref<PersonaProfile[]>;
  selectedPersonaId: Ref<string>;
  avatarSaving: Ref<boolean>;
  avatarError: Ref<string>;
  selectedApiConfig: ComputedRef<ApiConfigItem | null>;
  refreshingModels: Ref<boolean>;
  modelRefreshError: Ref<string>;
  apiModelOptions: Ref<Record<string, string[]>>;
  modelRefreshOkFlags: Ref<Record<string, boolean>>;
  toolApiConfig: ComputedRef<ApiConfigItem | null>;
  checkingToolsStatus: Ref<boolean>;
  toolStatuses: Ref<ToolLoadStatus[]>;
  imageCacheStats: Ref<ImageTextCacheStats>;
  imageCacheStatsLoading: Ref<boolean>;
  ensureAvatarCached: (path?: string, updatedAt?: string) => Promise<void>;
};

export function useConfigRuntime(options: UseConfigRuntimeOptions) {
  async function syncTrayIcon(agentId?: string) {
    try {
      await invokeTauri("sync_tray_icon", {
        input: {
          agentId: agentId ?? null,
        },
      });
    } catch (e) {
      console.warn("[TRAY] sync icon failed:", e);
    }
  }

  async function saveAgentAvatar(input: { agentId: string; mime: string; bytesBase64: string }) {
    options.avatarSaving.value = true;
    options.avatarError.value = "";
    try {
      const result = await invokeTauri<{ path: string; updatedAt: string }>("save_agent_avatar", {
        input: {
          agentId: input.agentId,
          mime: input.mime,
          bytesBase64: input.bytesBase64,
        },
      });
      const idx = options.personas.value.findIndex((p) => p.id === input.agentId);
      if (idx >= 0) {
        options.personas.value[idx].avatarPath = result.path;
        options.personas.value[idx].avatarUpdatedAt = result.updatedAt;
        options.personas.value[idx].updatedAt = new Date().toISOString();
      }
      await options.ensureAvatarCached(result.path, result.updatedAt);
      if (input.agentId === options.selectedPersonaId.value) {
        await syncTrayIcon(input.agentId);
      }
      options.setStatus(options.t("status.avatarSaved"));
    } catch (e) {
      const err = String(e);
      options.avatarError.value = err;
      options.setStatus(options.t("status.avatarSaveFailed", { err }));
    } finally {
      options.avatarSaving.value = false;
    }
  }

  async function clearAgentAvatar(input: { agentId: string }) {
    options.avatarError.value = "";
    try {
      await invokeTauri("clear_agent_avatar", { input: { agentId: input.agentId } });
      const idx = options.personas.value.findIndex((p) => p.id === input.agentId);
      if (idx >= 0) {
        options.personas.value[idx].avatarPath = undefined;
        options.personas.value[idx].avatarUpdatedAt = undefined;
        options.personas.value[idx].updatedAt = new Date().toISOString();
      }
      if (input.agentId === options.selectedPersonaId.value) {
        await syncTrayIcon(input.agentId);
      }
      options.setStatus(options.t("status.avatarCleared"));
    } catch (e) {
      const err = String(e);
      options.avatarError.value = err;
      options.setStatus(options.t("status.avatarClearFailed", { err }));
    }
  }

  async function refreshModels() {
    if (!options.selectedApiConfig.value) return;
    const apiId = options.selectedApiConfig.value.id;
    options.refreshingModels.value = true;
    options.modelRefreshError.value = "";
    try {
      const models = await invokeTauri<string[]>("refresh_models", {
        input: {
          baseUrl: options.selectedApiConfig.value.baseUrl,
          apiKey: options.selectedApiConfig.value.apiKey,
          requestFormat: options.selectedApiConfig.value.requestFormat,
        },
      });
      const normalizedModels = models.map((m) => m.trim()).filter(Boolean);
      options.apiModelOptions.value[apiId] = normalizedModels;
      options.modelRefreshOkFlags.value[apiId] = true;
      options.setStatus(options.t("status.modelListRefreshed", { count: normalizedModels.length }));
    } catch (e) {
      const err = String(e);
      options.modelRefreshError.value = err;
      options.modelRefreshOkFlags.value[apiId] = false;
      options.setStatus(options.t("status.refreshModelsFailed", { err }));
    } finally {
      options.refreshingModels.value = false;
    }
  }

  async function refreshToolsStatus() {
    if (!options.toolApiConfig.value) return;
    options.checkingToolsStatus.value = true;
    try {
      options.toolStatuses.value = await invokeTauri<ToolLoadStatus[]>("check_tools_status", {
        input: { apiConfigId: options.toolApiConfig.value.id },
      });
    } catch (e) {
      options.toolStatuses.value = [
        {
          id: "tools",
          status: "failed",
          detail: String(e),
        },
      ];
    } finally {
      options.checkingToolsStatus.value = false;
    }
  }

  async function refreshImageCacheStats() {
    options.imageCacheStatsLoading.value = true;
    try {
      options.imageCacheStats.value = await invokeTauri<ImageTextCacheStats>("get_image_text_cache_stats");
    } catch (e) {
      options.setStatusError("status.loadImageCacheStatsFailed", e);
    } finally {
      options.imageCacheStatsLoading.value = false;
    }
  }

  async function clearImageCache() {
    options.imageCacheStatsLoading.value = true;
    try {
      options.imageCacheStats.value = await invokeTauri<ImageTextCacheStats>("clear_image_text_cache");
      options.setStatus(options.t("status.imageCacheCleared"));
    } catch (e) {
      options.setStatusError("status.clearImageCacheFailed", e);
    } finally {
      options.imageCacheStatsLoading.value = false;
    }
  }

  return {
    syncTrayIcon,
    saveAgentAvatar,
    clearAgentAvatar,
    refreshModels,
    refreshToolsStatus,
    refreshImageCacheStats,
    clearImageCache,
  };
}
