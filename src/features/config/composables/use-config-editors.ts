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

export type PersonaChildAgentsUpdateStatus =
  | "applied"
  | "unchanged"
  | "overridden"
  | "failed"
  | "rejected";

export type PersonaChildAgentsUpdateResult = { status: PersonaChildAgentsUpdateStatus };

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
   * 改写某个人格的直属下级人格，立即落盘。
   * 私有人格由私有工作区文件维护，不在这里改。
   *
   * 返回结构化结果：保存会做归一化，用户请求的状态不一定原样落地，
   * 调用方必须能区分，不能一律当成已生效。
   */
  async function setPersonaChildAgents(input: {
    agentId: string;
    childAgentIds: string[];
  }): Promise<PersonaChildAgentsUpdateResult> {
    const agentId = String(input?.agentId || "").trim();
    if (!agentId) return { status: "rejected" };
    const persona = options.personas.value.find(
      (item) => String(item.id || "").trim() === agentId,
    );
    if (!persona) return { status: "rejected" };
    const next = Array.from(
      new Set((input.childAgentIds || []).map((id) => String(id || "").trim()).filter(Boolean)),
    );
    const previous = Array.isArray(persona.childAgentIds) ? [...persona.childAgentIds] : [];
    const sameLength = previous.length === next.length;
    if (sameLength && previous.every((id, index) => id === next[index])) {
      return { status: "unchanged" };
    }
    persona.childAgentIds = next;
    const saved = await options.savePersonas();
    if (!saved) {
      persona.childAgentIds = previous;
      return { status: "failed" };
    }
    const savedPersona = options.personas.value.find(
      (item) => String(item.id || "").trim() === agentId,
    );
    const applied = Array.isArray(savedPersona?.childAgentIds)
      ? savedPersona.childAgentIds.map((id) => String(id || "").trim())
      : [];
    const appliedSame = applied.length === next.length && applied.every((id, index) => id === next[index]);
    return { status: appliedSame ? "applied" : "overridden" };
  }

  function removeSelectedPersona() {
    if (options.assistantPersonas.value.length <= 1) return;
    const target = options.selectedPersonaEditor.value;
    if (!target || target.isBuiltInUser || target.isBuiltInSystem) return;
    const idx = options.personas.value.findIndex((p) => p.id === target.id);
    if (idx >= 0) options.personas.value.splice(idx, 1);
    if (options.assistantAgentId.value === target.id) {
      options.assistantAgentId.value = options.assistantPersonas.value[0]?.id || "default-agent";
    }
    options.personaEditorId.value = options.assistantPersonas.value[0]?.id || "default-agent";
  }

  return {
    addApiConfig,
    removeSelectedApiConfig,
    addPersona,
    setPersonaChildAgents,
    removeSelectedPersona,
  };
}

