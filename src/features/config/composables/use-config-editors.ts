import type { ComputedRef, Ref } from "vue";
import type { ApiConfigItem, ApiProviderConfigItem, AppConfig, PersonaProfile } from "../../../types/app";

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

  async function addPersona() {
    const previousPersonas = options.personas.value.map((persona) => ({ ...persona }));
    const previousPersonaEditorId = options.personaEditorId.value;
    const id = `persona-${Date.now()}`;
    const now = new Date().toISOString();
    options.personas.value.push({
      id,
      name: `${options.t("config.persona.title")} ${options.assistantPersonas.value.length + 1}`,
      systemPrompt: options.t("config.persona.assistantPlaceholder"),
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
    addPersona,
    removeSelectedPersona,
  };
}

