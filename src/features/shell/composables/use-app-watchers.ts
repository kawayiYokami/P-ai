import { watch, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem, AppConfig, PdfReadMode, PersonaProfile, ToolLoadStatus } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseAppWatchersOptions = {
  config: AppConfig;
  configTab: Ref<"welcome" | "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about">;
  viewMode: Ref<"chat" | "archives" | "config">;
  personas: Ref<PersonaProfile[]>;
  userPersona: ComputedRef<PersonaProfile | null>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  assistantDepartmentAgentId: Ref<string>;
  personaEditorId: Ref<string>;
  userAlias: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  selectedPdfReadMode: Ref<PdfReadMode>;
  backgroundVoiceScreenshotKeywords: Ref<string>;
  backgroundVoiceScreenshotMode: Ref<"desktop" | "focused_window">;
  selectedApiConfig: ComputedRef<ApiConfigItem | null>;
  toolApiConfig: ComputedRef<ApiConfigItem | null>;
  activeChatApiConfigId: ComputedRef<string>;
  suppressChatReloadWatch: Ref<boolean>;
  suppressAutosave: Ref<boolean>;
  modelRefreshError: Ref<string>;
  toolStatuses: Ref<ToolLoadStatus[]>;
  defaultApiTools: () => ApiConfigItem["tools"];
  t: TrFn;
  normalizeApiBindingsLocal: () => void;
  syncUserAliasFromPersona: () => void;
  syncTrayIcon: (id?: string) => Promise<void>;
  saveChatPreferences: () => Promise<void>;
  saveConversationApiSettings: () => Promise<void>;
  refreshToolsStatus: () => Promise<void>;
  refreshImageCacheStats: () => Promise<void>;
  refreshConversationHistory: () => Promise<void>;
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
      customTemperatureEnabled: !!a.customTemperatureEnabled,
      contextWindowTokens: a.contextWindowTokens,
      customMaxOutputTokensEnabled: !!a.customMaxOutputTokensEnabled,
      maxOutputTokens: a.maxOutputTokens,
    })),
    () => options.normalizeApiBindingsLocal(),
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
      if (!options.assistantPersonas.value.some((p) => p.id === options.assistantDepartmentAgentId.value)) {
        options.assistantDepartmentAgentId.value = options.assistantPersonas.value[0].id;
      }
    },
  );

  watch(
    () => options.personas.value.map((p) => p.id).join("|"),
    () => {
      if (options.personas.value.length === 0) return;
      if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
        options.personaEditorId.value = options.assistantDepartmentAgentId.value;
      }
    },
  );

  watch(
    () => {
      const assistantDepartment = options.config.departments.find(
        (item) => item.id === "assistant-department" || item.isBuiltInAssistant,
      );
      return {
        agentId: assistantDepartment?.agentIds?.[0] || "",
        apiConfigId: assistantDepartment?.apiConfigId || "",
      };
    },
    ({ agentId, apiConfigId }) => {
      if (agentId && options.assistantDepartmentAgentId.value !== agentId) {
        options.assistantDepartmentAgentId.value = agentId;
      }
      if (options.config.assistantDepartmentApiConfigId !== apiConfigId) {
        options.config.assistantDepartmentApiConfigId = apiConfigId;
      }
    },
    { deep: true },
  );

  watch(
    () => options.assistantDepartmentAgentId.value,
    (id) => {
      if (!id) return;
      void options.syncTrayIcon(id);
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
      options.toolApiConfig.value?.id ?? "",
      options.personaEditorId.value,
      options.toolApiConfig.value?.enableTools,
      options.toolApiConfig.value?.enableImage,
      (options.personas.value.find((item) => item.id === options.personaEditorId.value)?.tools ?? [])
        .map((tool) => `${tool.id}:${tool.enabled ? 1 : 0}`)
        .join("|"),
    ],
    async ([tab, id, enabled]) => {
      if (tab !== "tools") return;
      if (id && !enabled) {
        options.toolStatuses.value = (
          options.personas.value.find((item) => item.id === options.personaEditorId.value)?.tools ?? []
        ).map((tool) => ({
          id: tool.id,
          status: "disabled",
          detail: options.t("config.tools.disabledHint"),
        }));
        return;
      }
      try {
        await options.refreshToolsStatus();
      } catch (error) {
        console.error("[WATCH] refreshToolsStatus failed:", error);
        options.toolStatuses.value = (
          options.personas.value.find((item) => item.id === options.personaEditorId.value)?.tools ?? []
        ).map((tool) => ({
          id: tool.id,
          status: "failed",
          detail: String(error),
        }));
      }
    },
  );

  watch(
    () => options.configTab.value,
    async (tab) => {
      if (tab !== "chatSettings") return;
      try {
        await options.refreshImageCacheStats();
      } catch (error) {
        console.error("[WATCH] refreshImageCacheStats failed:", error);
      }
    },
  );

  watch(
    () => options.activeChatApiConfigId.value,
    async () => {
      if (options.suppressChatReloadWatch.value) return;
      if (options.viewMode.value !== "chat") return;
      try {
        await options.refreshConversationHistory();
        options.resetVisibleTurnCount();
      } catch (error) {
        console.error("[WATCH] refreshConversationHistory failed:", error);
      }
    },
  );
  watch(
    () => ({
      userAlias: options.userAlias.value,
      responseStyleId: options.selectedResponseStyleId.value,
      pdfReadMode: options.selectedPdfReadMode.value,
      backgroundVoiceScreenshotKeywords: options.backgroundVoiceScreenshotKeywords.value,
      backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
    }),
    () => {
      if (options.suppressAutosave.value) return;
      void options.saveChatPreferences();
    },
  );

  watch(
    () => ({
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: options.config.sttAutoSend,
    }),
    () => {
      if (options.suppressAutosave.value) return;
      void options.saveConversationApiSettings();
    },
  );
}
