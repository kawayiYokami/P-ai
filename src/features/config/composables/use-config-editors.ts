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
  function firstActiveApiConfigId(): string {
    for (const provider of options.config.apiProviders || []) {
      if (provider.deprecated) continue;
      for (const model of provider.models || []) {
        if (model.deprecated) continue;
        const providerId = String(provider.id || "").trim();
        const modelId = String(model.id || "").trim();
        if (providerId && modelId) return `${providerId}::${modelId}`;
      }
    }
    return "";
  }

  function addApiConfig() {
    const provider = options.createApiProvider();
    options.config.apiProviders.push(provider);
    options.normalizeApiBindingsLocal();
    options.config.selectedApiConfigId = `${provider.id}::${provider.models[0]?.id || ""}`;
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
    options.config.selectedApiConfigId = firstActiveApiConfigId();
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
    const previousAssistantAgentId = options.assistantAgentId.value;
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
    options.assistantAgentId.value = id;
    options.personaEditorId.value = id;
    const saved = await options.savePersonas();
    if (!saved) {
      options.personas.value = previousPersonas;
      options.assistantAgentId.value = previousAssistantAgentId;
      options.personaEditorId.value = previousPersonaEditorId;
      return;
    }
    await options.saveChatPreferences();
  }

  /**
   * 批量改写多个人格的直属下级，一次落盘。
   * 组织页是草稿式编辑（勾选/连线先落本地草稿），保存时把有变更的人格一次性写回，
   * 避免逐个人格各触发一次 personas 落盘。失败时整体回滚。
   */
  async function setPersonaChildAgentsBatch(input: {
    updates: { agentId: string; childAgentIds: string[] }[];
  }): Promise<boolean> {
    const normalized = (input?.updates || [])
      .map((item) => ({
        agentId: String(item?.agentId || "").trim(),
        childAgentIds: Array.from(
          new Set((item?.childAgentIds || []).map((id) => String(id || "").trim()).filter(Boolean)),
        ),
      }))
      .filter((item) => !!item.agentId);
    if (normalized.length === 0) return true;

    const snapshot = new Map<string, string[]>();
    let applied = 0;
    for (const update of normalized) {
      const persona = options.personas.value.find(
        (item) => String(item.id || "").trim() === update.agentId,
      );
      if (!persona) continue;
      snapshot.set(
        update.agentId,
        Array.isArray(persona.childAgentIds) ? [...persona.childAgentIds] : [],
      );
      persona.childAgentIds = update.childAgentIds;
      applied += 1;
    }
    if (applied === 0) return true;

    const saved = await options.savePersonas();
    if (!saved) {
      for (const [agentId, previous] of snapshot) {
        const persona = options.personas.value.find(
          (item) => String(item.id || "").trim() === agentId,
        );
        if (persona) persona.childAgentIds = previous;
      }
      return false;
    }
    return true;
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
    setPersonaChildAgentsBatch,
    removeSelectedPersona,
  };
}

