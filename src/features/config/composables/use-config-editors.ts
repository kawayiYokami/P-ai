import type { ComputedRef, Ref } from "vue";
import type { ApiConfigItem, AppConfig, PersonaProfile } from "../../../types/app";
import { defaultToolBindings } from "../utils/builtin-tools";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseConfigEditorsOptions = {
  t: TrFn;
  config: AppConfig;
  personas: Ref<PersonaProfile[]>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  assistantDepartmentAgentId: Ref<string>;
  personaEditorId: Ref<string>;
  selectedPersonaEditor: ComputedRef<PersonaProfile | null>;
  createApiConfig: (seed?: string) => ApiConfigItem;
  normalizeApiBindingsLocal: () => void;
};

export function useConfigEditors(options: UseConfigEditorsOptions) {
  function addApiConfig() {
    const c = options.createApiConfig();
    options.config.apiConfigs.push(c);
    options.config.selectedApiConfigId = c.id;
    options.normalizeApiBindingsLocal();
  }

  function removeSelectedApiConfig() {
    if (options.config.apiConfigs.length <= 1) return;
    const idx = options.config.apiConfigs.findIndex(
      (a) => a.id === options.config.selectedApiConfigId,
    );
    if (idx < 0) return;
    const removedId = options.config.apiConfigs[idx].id;
    options.config.apiConfigs.splice(idx, 1);
    for (const department of options.config.departments || []) {
      const nextIds = (Array.isArray(department.apiConfigIds) ? department.apiConfigIds : [])
        .map((id) => String(id || "").trim())
        .filter((id) => !!id && id !== removedId);
      department.apiConfigIds = nextIds;
      if (String(department.apiConfigId || "").trim() === removedId) {
        department.apiConfigId = nextIds[0] || "";
      }
    }
    if (options.config.assistantDepartmentApiConfigId === removedId) {
      options.config.assistantDepartmentApiConfigId = "";
    }
    if (options.config.sttApiConfigId === removedId) {
      options.config.sttApiConfigId = undefined;
      options.config.sttAutoSend = false;
    }
    if (options.config.visionApiConfigId === removedId) {
      options.config.visionApiConfigId = undefined;
    }
    if (options.config.apiConfigs.length > 0) {
      options.config.selectedApiConfigId = options.config.apiConfigs[0].id;
    }
    options.normalizeApiBindingsLocal();
  }

  function addPersona() {
    const id = `persona-${Date.now()}`;
    const now = new Date().toISOString();
    options.personas.value.push({
      id,
      name: `${options.t("config.persona.title")} ${options.assistantPersonas.value.length + 1}`,
      systemPrompt: options.t("config.persona.assistantPlaceholder"),
      tools: defaultToolBindings(),
      privateMemoryEnabled: false,
      createdAt: now,
      updatedAt: now,
      avatarPath: undefined,
      avatarUpdatedAt: undefined,
      isBuiltInUser: false,
      isBuiltInSystem: false,
      source: "main_config",
      scope: "global",
    });
    options.assistantDepartmentAgentId.value = id;
    options.personaEditorId.value = id;
  }

  function removeSelectedPersona() {
    if (options.assistantPersonas.value.length <= 1) return;
    const target = options.selectedPersonaEditor.value;
    if (!target || target.isBuiltInUser || target.isBuiltInSystem) return;
    const idx = options.personas.value.findIndex((p) => p.id === target.id);
    if (idx >= 0) options.personas.value.splice(idx, 1);
    if (options.assistantDepartmentAgentId.value === target.id) {
      options.assistantDepartmentAgentId.value = options.assistantPersonas.value[0]?.id || "default-agent";
    }
    options.personaEditorId.value = options.assistantPersonas.value[0]?.id || "default-agent";
  }

  return {
    addApiConfig,
    removeSelectedApiConfig,
    addPersona,
    removeSelectedPersona,
  };
}

