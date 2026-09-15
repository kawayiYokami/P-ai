import { computed, type Ref } from "vue";
import type { AppConfig, PersonaProfile, ResponseStyleOption } from "../../../types/app";
import responseStylesJson from "../../../constants/response-styles.json";
import { buildPersonasSnapshotJson as buildPersonasSnapshotJsonValue } from "./use-chat-window-message-helpers";

type UseChatConfigUiDerivedStateOptions = {
  config: AppConfig;
  apiModelOptions: Ref<Record<string, string[]>>;
  modelRefreshOkFlags: Ref<Record<string, boolean>>;
  selectedApiConfig: Ref<{ requestFormat?: string } | null>;
  personas: Ref<PersonaProfile[]>;
  lastSavedConfigJson: Ref<string>;
  lastSavedPersonasJson: Ref<string>;
  buildConfigSnapshotJson: () => string;
  t: (key: string) => string;
};

export function useChatConfigUiDerivedState(options: UseChatConfigUiDerivedStateOptions) {
  const selectedModelOptions = computed(() => {
    const id = options.config.selectedApiConfigId;
    if (!id) return [];
    const fromApiOptions = options.apiModelOptions.value[id] ?? [];
    const [providerId] = id.split("::");
    const provider = options.config.apiProviders.find((p) => p.id === providerId);
    const fromCache = Array.isArray(provider?.cachedModelOptions) ? provider.cachedModelOptions : [];
    return Array.from(new Set([...fromApiOptions, ...fromCache].map((item) => String(item || "").trim()).filter(Boolean)));
  });
  const selectedModelRefreshOk = computed(() => {
    const id = options.config.selectedApiConfigId;
    if (!id) return false;
    const fromCache = selectedModelOptions.value.length > 0;
    return fromCache;
  });
  const responseStyleOptions = responseStylesJson as ResponseStyleOption[];
  const baseUrlReference = computed(() => {
    const format = options.selectedApiConfig.value?.requestFormat ?? "openai";
    if (format === "gemini") return "https://generativelanguage.googleapis.com";
    if (format === "gemini_embedding") return "https://generativelanguage.googleapis.com";
    if (format === "anthropic") return "https://api.anthropic.com";
    if (format === "openai_tts") return "https://api.openai.com/v1/audio/speech";
    if (format === "openai_stt") return "https://api.openai.com/v1";
    if (format === "mimo_asr") return "https://api.xiaomimimo.com/v1";
    if (format === "openai_embedding") return "https://api.openai.com/v1";
    if (format === "openai_rerank") return "https://api.openai.com/v1";
    return "https://api.openai.com/v1";
  });
  const defaultCreateConversationAgentId = computed(() => "default-agent");
  const configDirty = computed(() => options.buildConfigSnapshotJson() !== options.lastSavedConfigJson.value);
  const personaDirty = computed(() => buildPersonasSnapshotJsonValue(options.personas.value) !== options.lastSavedPersonasJson.value);
  const responseStyleIds = computed(() => responseStyleOptions.map((item) => item.id));

  return {
    selectedModelOptions,
    selectedModelRefreshOk,
    responseStyleOptions,
    baseUrlReference,
    defaultCreateConversationAgentId,
    configDirty,
    personaDirty,
    responseStyleIds,
  };
}
