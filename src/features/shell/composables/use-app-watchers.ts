import { watch, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem, AppConfig, PersonaProfile, ToolLoadStatus } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseAppWatchersOptions = {
  config: AppConfig;
  configTab: Ref<"hotkey" | "api" | "tools" | "persona" | "chatSettings">;
  viewMode: Ref<"chat" | "archives" | "config">;
  personas: Ref<PersonaProfile[]>;
  userPersona: ComputedRef<PersonaProfile | null>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  selectedPersonaId: Ref<string>;
  personaEditorId: Ref<string>;
  userAlias: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  selectedApiConfig: ComputedRef<ApiConfigItem | null>;
  toolApiConfig: ComputedRef<ApiConfigItem | null>;
  activeChatApiConfigId: ComputedRef<string>;
  suppressChatReloadWatch: Ref<boolean>;
  modelRefreshError: Ref<string>;
  toolStatuses: Ref<ToolLoadStatus[]>;
  defaultApiTools: () => ApiConfigItem["tools"];
  t: TrFn;
  schedulePersonasAutosave: () => void;
  scheduleChatSettingsAutosave: () => void;
  normalizeApiBindingsLocal: () => void;
  syncUserAliasFromPersona: () => void;
  syncTrayIcon: (id?: string) => Promise<void>;
  saveConversationApiSettings: () => Promise<void>;
  refreshToolsStatus: () => Promise<void>;
  refreshImageCacheStats: () => Promise<void>;
  refreshChatSnapshot: () => Promise<void>;
  loadAllMessages: () => Promise<void>;
  resetVisibleTurnCount: () => void;
};

export function useAppWatchers(options: UseAppWatchersOptions) {
  watch(
    () => options.config.apiConfigs.map((a) => ({
      id: a.id,
      requestFormat: a.requestFormat,
      enableText: a.enableText,
      enableImage: a.enableImage,
      enableAudio: a.enableAudio,
      enableTools: a.enableTools,
      temperature: a.temperature,
      contextWindowTokens: a.contextWindowTokens,
    })),
    () => options.normalizeApiBindingsLocal(),
    { deep: true },
  );

  watch(
    () => options.personas.value.map((p) => ({
      id: p.id,
      name: p.name,
      systemPrompt: p.systemPrompt,
      updatedAt: p.updatedAt,
      avatarPath: p.avatarPath,
      avatarUpdatedAt: p.avatarUpdatedAt,
      isBuiltInUser: p.isBuiltInUser,
    })),
    () => options.schedulePersonasAutosave(),
    { deep: true },
  );

  watch(
    () => options.userPersona.value?.name,
    () => {
      options.syncUserAliasFromPersona();
    },
  );

  watch(
    () => options.assistantPersonas.value.map((p) => p.id).join("|"),
    () => {
      if (options.assistantPersonas.value.length === 0) return;
      if (!options.assistantPersonas.value.some((p) => p.id === options.selectedPersonaId.value)) {
        options.selectedPersonaId.value = options.assistantPersonas.value[0].id;
      }
    },
  );

  watch(
    () => options.personas.value.map((p) => p.id).join("|"),
    () => {
      if (options.personas.value.length === 0) return;
      if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
        options.personaEditorId.value = options.selectedPersonaId.value;
      }
    },
  );

  watch(
    () => ({
      selectedPersonaId: options.selectedPersonaId.value,
      userAlias: options.userAlias.value,
      responseStyleId: options.selectedResponseStyleId.value,
    }),
    () => options.scheduleChatSettingsAutosave(),
  );

  watch(
    () => options.selectedPersonaId.value,
    (id) => {
      if (!id) return;
      void options.syncTrayIcon(id);
    },
  );

  watch(
    () => ({
      chatApiConfigId: options.config.chatApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
    }),
    () => {
      void options.saveConversationApiSettings();
    },
  );

  watch(
    () => options.config.selectedApiConfigId,
    () => {
      options.modelRefreshError.value = "";
    },
  );

  watch(
    () => options.selectedApiConfig.value?.enableTools,
    (enabled) => {
      if (!enabled || !options.selectedApiConfig.value) return;
      if (options.selectedApiConfig.value.tools.length === 0) {
        options.selectedApiConfig.value.tools = options.defaultApiTools();
      }
    },
  );

  watch(
    () => [
      options.configTab.value,
      options.activeChatApiConfigId.value,
      options.toolApiConfig.value?.enableTools,
      options.toolApiConfig.value?.enableImage,
      (options.toolApiConfig.value?.tools ?? []).map((tool) => `${tool.id}:${tool.enabled ? 1 : 0}`).join("|"),
    ],
    async ([tab, id, enabled]) => {
      if (tab !== "tools") return;
      if (!id) return;
      if (!enabled) {
        options.toolStatuses.value = (options.toolApiConfig.value?.tools ?? []).map((tool) => ({
          id: tool.id,
          status: "disabled",
          detail: options.t("config.tools.disabledHint"),
        }));
        return;
      }
      await options.refreshToolsStatus();
    },
  );

  watch(
    () => options.configTab.value,
    async (tab) => {
      if (tab !== "chatSettings") return;
      await options.refreshImageCacheStats();
    },
  );

  watch(
    () => options.activeChatApiConfigId.value,
    async (newId, oldId) => {
      if (newId === oldId) return;
      if (options.suppressChatReloadWatch.value) return;
      if (options.viewMode.value !== "chat") return;
      await options.refreshChatSnapshot();
      await options.loadAllMessages();
      options.resetVisibleTurnCount();
    },
  );
}
