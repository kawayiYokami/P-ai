import { watch, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem, AppConfig, PersonaProfile, ToolLoadStatus } from "../../../types/app";

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
  selectedApiConfig: ComputedRef<ApiConfigItem | null>;
  toolApiConfig: ComputedRef<ApiConfigItem | null>;
  modelRefreshError: Ref<string>;
  toolStatuses: Ref<ToolLoadStatus[]>;
  defaultApiTools: () => ApiConfigItem["tools"];
  t: TrFn;
  normalizeApiBindingsLocal: () => void;
  syncUserAliasFromPersona: () => void;
  syncTrayIcon: (id?: string) => Promise<void>;
  refreshToolsStatus: () => Promise<void>;
  refreshImageCacheStats: () => Promise<void>;
};

export function useAppWatchers(options: UseAppWatchersOptions) {
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
    () => [
      options.configTab.value,
      options.toolApiConfig.value?.id ?? "",
      options.assistantDepartmentAgentId.value,
      options.toolApiConfig.value?.enableTools,
      options.toolApiConfig.value?.enableImage,
      String(options.config.terminalShellKind || ""),
      JSON.stringify(
        (options.config.departments || []).map((item) => ({
          id: item.id,
          apiConfigId: item.apiConfigId,
          agentIds: [...(item.agentIds || [])],
          permissionControl: item.permissionControl ?? null,
        })),
      ),
    ],
    async ([tab, id]) => {
      if (tab !== "tools") return;
      if (!id) return;
      try {
        await options.refreshToolsStatus();
      } catch (error) {
        console.error("[WATCH] refreshToolsStatus failed:", error);
        options.toolStatuses.value = options.defaultApiTools().map((tool) => ({
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

}
