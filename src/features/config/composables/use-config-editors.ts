import type { ComputedRef, Ref } from "vue";
import type { ApiConfigItem, ApiProviderConfigItem, AppConfig, PersonaProfile } from "../../../types/app";
import { defaultToolBindings } from "../utils/builtin-tools";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseConfigEditorsOptions = {
  t: TrFn;
  config: AppConfig;
  personas: Ref<PersonaProfile[]>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  assistantAgentId: Ref<string>;
  personaEditorId: Ref<string>;
  selectedPersonaEditor: ComputedRef<PersonaProfile | null>;
  createApiConfig: (seed?: string) => ApiConfigItem;
  createApiProvider: (seed?: string) => ApiProviderConfigItem;
  normalizeApiBindingsLocal: () => void;
  savePersonas: () => Promise<boolean>;
  saveChatPreferences: () => Promise<void>;
  saveConfig: () => Promise<boolean>;
};

export function useConfigEditors(options: UseConfigEditorsOptions) {
  function addApiConfig() {
    const provider = options.createApiProvider();
    options.config.apiProviders.push(provider);
    options.normalizeApiBindingsLocal();
  }

  function removeSelectedApiConfig() {
    const [providerId, modelId] = String(options.config.selectedApiConfigId || "").split("::");
    if (!providerId) return;
    const providerIdx = options.config.apiProviders.findIndex((item) => item.id === providerId);
    if (providerIdx < 0) return;
    const provider = options.config.apiProviders[providerIdx];
    const removedId = String(options.config.selectedApiConfigId || "").trim();
    const activeProviders = (options.config.apiProviders || []).filter((item) => !item.deprecated);
    const activeModels = (provider.models || []).filter((item) => !item.deprecated);
    if (!provider.deprecated && activeProviders.length <= 1 && activeModels.length <= 1) return;
    if (modelId) {
      const model = (provider.models || []).find((item) => item.id === modelId);
      if (!model) return;
      if (!provider.deprecated && activeModels.length <= 1) {
        provider.deprecated = true;
        provider.models = (provider.models || []).map((item) => ({ ...item, deprecated: true }));
      } else {
        model.deprecated = true;
      }
    } else {
      provider.deprecated = true;
      provider.models = (provider.models || []).map((item) => ({ ...item, deprecated: true }));
    }
    for (const persona of options.personas.value || []) {
      if (!Array.isArray(persona.apiConfigIds)) continue;
      persona.apiConfigIds = persona.apiConfigIds
        .map((id) => String(id || "").trim())
        .filter((id) => !!id && id !== removedId);
    }
    if (options.config.expertApiConfigId === removedId) {
      options.config.expertApiConfigId = "";
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
  }

  async function addPersona() {
    const previousPersonas = options.personas.value.map((persona) => ({
      ...persona,
      tools: Array.isArray(persona.tools)
        ? persona.tools.map((tool) => ({
            ...tool,
            args: Array.isArray(tool.args) ? [...tool.args] : [],
            values: { ...((tool.values || {}) as Record<string, unknown>) },
          }))
        : [],
    }));
    const previousPersonaEditorId = options.personaEditorId.value;
    const id = `persona-${Date.now()}`;
    const now = new Date().toISOString();
    options.personas.value.push({
      id,
      name: `${options.t("config.persona.title")} ${options.assistantPersonas.value.length + 1}`,
      systemPrompt: options.t("config.persona.assistantPlaceholder"),
      tools: defaultToolBindings(),
      privateMemoryEnabled: false,
      memoryRecallMode: "auto",
      createdAt: now,
      updatedAt: now,
      avatarPath: undefined,
      avatarUpdatedAt: undefined,
      isBuiltInUser: false,
      isBuiltInSystem: false,
      source: "main_config",
      scope: "global",
      childAgentIds: [],
      apiConfigIds: [],
    });
    options.personaEditorId.value = id;
    const saved = await options.savePersonas();
    if (!saved) {
      options.personas.value = previousPersonas;
      options.personaEditorId.value = previousPersonaEditorId;
      return;
    }
    await options.saveChatPreferences();
  }

  async function removeSelectedPersona() {
    if (options.assistantPersonas.value.length <= 1) return false;
    const target = options.selectedPersonaEditor.value;
    if (!target || target.isBuiltInUser || target.isBuiltInSystem) return false;
    const idx = options.personas.value.findIndex((p) => p.id === target.id);
    if (idx < 0) return false;
    const nextEditorId = options.assistantPersonas.value
      .filter((p) => p.id !== target.id)
      .map((p) => p.id)[0] || "default-agent";
    options.personas.value.splice(idx, 1);
    if (options.assistantAgentId.value === target.id) {
      options.assistantAgentId.value = nextEditorId;
    }
    options.personaEditorId.value = nextEditorId;
    // 删除是用户在确认框里明确同意过的动作，直接落盘；留在内存里等保存会在切页或刷新时被悄悄丢掉。
    const saved = await options.savePersonas();
    if (!saved) {
      options.personas.value.splice(idx, 0, target);
    }
    return saved;
  }

  return {
    addApiConfig,
    removeSelectedApiConfig,
    addPersona,
    removeSelectedPersona,
  };
}

