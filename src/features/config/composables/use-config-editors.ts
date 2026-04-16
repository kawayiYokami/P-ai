import type { ComputedRef, Ref } from "vue";
import type { ApiConfigItem, ApiProviderConfigItem, AppConfig, PersonaProfile } from "../../../types/app";
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
  createApiProvider: (seed?: string) => ApiProviderConfigItem;
  normalizeApiBindingsLocal: () => void;
};

export function useConfigEditors(options: UseConfigEditorsOptions) {
  function addApiConfig() {
    const provider = options.createApiProvider();
    options.config.apiProviders.push(provider);
    options.normalizeApiBindingsLocal();
    options.config.selectedApiConfigId = `${provider.id}::${provider.models[0]?.id || ""}`;
  }

  function removeSelectedApiConfig() {
    if (options.config.apiProviders.length <= 1) return;
    const [providerId, modelId] = String(options.config.selectedApiConfigId || "").split("::");
    if (!providerId) return;
    const providerIdx = options.config.apiProviders.findIndex((item) => item.id === providerId);
    if (providerIdx < 0) return;
    const provider = options.config.apiProviders[providerIdx];
    const removedId = String(options.config.selectedApiConfigId || "").trim();
    if (modelId && provider.models.length > 1) {
      provider.models = provider.models.filter((item) => item.id !== modelId);
    } else {
      options.config.apiProviders.splice(providerIdx, 1);
    }
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
    if (options.config.toolReviewApiConfigId === removedId) {
      options.config.toolReviewApiConfigId = undefined;
    }
    options.normalizeApiBindingsLocal();
    if (options.config.apiProviders.length > 0) {
      const provider = options.config.apiProviders[0];
      const modelId = Array.isArray(provider.models) ? String(provider.models[0]?.id || "").trim() : "";
      const providerId = String(provider.id || "").trim();
      options.config.selectedApiConfigId = providerId && modelId ? `${providerId}::${modelId}` : "";
    }
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

