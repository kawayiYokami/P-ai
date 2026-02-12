import type { ComputedRef, Ref } from "vue";
import type { ApiConfigItem, AppConfig, PersonaProfile } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseConfigEditorsOptions = {
  t: TrFn;
  config: AppConfig;
  personas: Ref<PersonaProfile[]>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  selectedPersonaId: Ref<string>;
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
    if (idx >= 0) options.config.apiConfigs.splice(idx, 1);
    options.config.selectedApiConfigId = options.config.apiConfigs[0].id;
    options.normalizeApiBindingsLocal();
  }

  function addPersona() {
    const id = `persona-${Date.now()}`;
    const now = new Date().toISOString();
    options.personas.value.push({
      id,
      name: `${options.t("config.persona.title")} ${options.assistantPersonas.value.length + 1}`,
      systemPrompt: options.t("config.persona.assistantPlaceholder"),
      createdAt: now,
      updatedAt: now,
      avatarPath: undefined,
      avatarUpdatedAt: undefined,
      isBuiltInUser: false,
    });
    options.selectedPersonaId.value = id;
    options.personaEditorId.value = id;
  }

  function removeSelectedPersona() {
    if (options.assistantPersonas.value.length <= 1) return;
    const target = options.selectedPersonaEditor.value;
    if (!target || target.isBuiltInUser) return;
    const idx = options.personas.value.findIndex((p) => p.id === target.id);
    if (idx >= 0) options.personas.value.splice(idx, 1);
    if (options.selectedPersonaId.value === target.id) {
      options.selectedPersonaId.value = options.assistantPersonas.value[0]?.id || "default-agent";
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


