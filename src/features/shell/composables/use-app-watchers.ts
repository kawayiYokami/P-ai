import { watch, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem, AppConfig, PersonaProfile, ToolLoadStatus } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseAppWatchersOptions = {
  config: AppConfig;
  configTab: Ref<"welcome" | "hotkey" | "api" | "imageGeneration" | "tools" | "mcp" | "skill" | "catalog" | "persona" | "demo" | "chatSettings" | "notification" | "networkAccess" | "remoteIm" | "usage" | "memory" | "task" | "logs" | "appearance" | "migration" | "about">;
  viewMode: Ref<"chat" | "archives" | "config">;
  personas: Ref<PersonaProfile[]>;
  userPersona: ComputedRef<PersonaProfile | null>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  assistantAgentId: Ref<string>;
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
      if (!options.assistantPersonas.value.some((p) => p.id === options.assistantAgentId.value)) {
        options.assistantAgentId.value = options.assistantPersonas.value[0].id;
      }
    },
  );

  watch(
    () => options.personas.value.map((p) => p.id).join("|"),
    () => {
      if (options.personas.value.length === 0) return;
      if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
        options.personaEditorId.value = options.assistantAgentId.value;
      }
    },
  );

  // 当前助理人格只由运行时状态（chat settings）决定，不再从组织成员列表的首位反推，
  // 避免调整组织关系时把当前助理人格一并改掉。
  watch(
    () => options.assistantAgentId.value,
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
      options.toolApiConfig.value?.id ?? "",
      options.assistantAgentId.value,
      options.toolApiConfig.value?.enableTools,
      options.toolApiConfig.value?.enableImage,
      String(options.config.terminalShellKind || ""),
      JSON.stringify(
        options.personas.value.map((item) => ({
          id: item.id,
          apiConfigIds: [...(item.apiConfigIds || [])],
          childAgentIds: [...(item.childAgentIds || [])],
          permissionControl: item.permissionControl ?? null,
        })),
      ),
    ],
    async ([id]) => {
      if (options.configTab.value !== "tools") return;
      if (!id) return;
      try {
        await options.refreshToolsStatus();
      } catch (error) {
        console.error("[监听] refreshToolsStatus failed:", error);
        options.toolStatuses.value = options.defaultApiTools().map((tool) => ({
          id: tool.id,
          status: "failed",
          detail: String(error),
        }));
      }
    },
  );
}
