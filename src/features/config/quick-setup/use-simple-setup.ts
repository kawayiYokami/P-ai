import { computed, onBeforeUnmount, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { i18n, normalizeLocale } from "../../../i18n";
import { invokeTauri, openTransportExternalUrl } from "../../../services/tauri-api";
import type { ApiConfigItem, ApiModelConfigItem, ApiProviderConfigItem, AppConfig, ChatSettings, ResponseStyleOption, ApiRequestFormat } from "../../../types/app";
import responseStylesJson from "../../../constants/response-styles.json";
import { useAppTheme } from "../../shell/composables/use-app-theme";
import { applyUiSizeScale } from "../../shell/composables/use-ui-size-appearance";
import { defaultToolBindings } from "../utils/builtin-tools";
import { normalizeApiRequestFormat } from "../utils/api-request-format";
import { apiConfigDisplayName } from "../utils/api-config-display";
import { hasUsableTextLlm } from "./usable-text-llm";

export type SimpleProviderId = "deepseek" | "opencode" | "custom";
export type SimpleModelCard = "quick" | "expert" | "vision";
export type SimpleReasoningEffort = "low" | "medium" | "high";

export interface SimpleProviderPreset {
  id: SimpleProviderId;
  label: string;
  requestFormat: ApiRequestFormat;
  baseUrl: string;
  keyUrl: string;
  defaultModel: string;
  /** 该供应商默认的多模态模型；缺省表示不提供多模态卡 */
  visionModel?: string;
}

export interface SimpleSetupDraft {
  providerId: SimpleProviderId;
  apiKey: string;
  customBaseUrl: string;
  /** 自定义供应商的 API 协议（requestFormat），预设供应商固定各自协议 */
  customRequestFormat: ApiRequestFormat;
  models: Record<SimpleModelCard, ApiModelConfigItem>;
  /** 自定义供应商刷新模型列表后拉到的候选模型 */
  customModelOptions: string[];
  responseStyleId: string;
  uiLanguage: AppConfig["uiLanguage"];
  hotkey: string;
  recordHotkey: string;
  siliconFlowKey: string;
}

export const SIMPLE_SETUP_DRAFT_KEY = "pai.simple-setup.draft.v1";
export const SIMPLE_SETUP_PROVIDER_ID = "simple-setup-provider";
export const SIMPLE_SETUP_MODEL_IDS: Record<SimpleModelCard, string> = {
  quick: "simple-setup-model-quick",
  expert: "simple-setup-model-expert",
  vision: "simple-setup-model-vision",
};
export const SIMPLE_SETUP_ENDPOINT_IDS: Record<SimpleModelCard, string> = {
  quick: `${SIMPLE_SETUP_PROVIDER_ID}::${SIMPLE_SETUP_MODEL_IDS.quick}`,
  expert: `${SIMPLE_SETUP_PROVIDER_ID}::${SIMPLE_SETUP_MODEL_IDS.expert}`,
  vision: `${SIMPLE_SETUP_PROVIDER_ID}::${SIMPLE_SETUP_MODEL_IDS.vision}`,
};

// 历史草稿里存在过的多模态模型名，加载时归一到当前预设名
const LEGACY_VISION_MODELS = new Set(["deepseek-v4-flash-vision-exp"]);

export const simpleProviderOptions: SimpleProviderPreset[] = [
  { id: "deepseek", label: "DeepSeek", requestFormat: "deepseek", baseUrl: "https://api.deepseek.com/v1", keyUrl: "https://platform.deepseek.com/api_keys", defaultModel: "deepseek-v4-flash", visionModel: "deepseek-v4-flash" },
  { id: "opencode", label: "OpenCode", requestFormat: "openai", baseUrl: "https://opencode.ai/zen/v1", keyUrl: "https://opencode.ai/zen", defaultModel: "gpt-4o-mini", visionModel: "mimo-v2.5" },
  { id: "custom", label: "自定义", requestFormat: "auto", baseUrl: "https://api.openai.com/v1", keyUrl: "", defaultModel: "gpt-4o-mini" },
];

export const SILICON_FLOW_KEY_URL = "https://cloud.siliconflow.cn/account/ak";
export const SILICON_FLOW_BASE_URL = "https://api.siliconflow.cn/v1";
export const SILICON_FLOW_EMBEDDING_MODEL = "BAAI/bge-m3";
export const SILICON_FLOW_RERANK_MODEL = "BAAI/bge-reranker-v2-m3";
export const SILICON_FLOW_STT_MODEL = "TeleAI/TeleSpeechASR";

export const responseStyleOptions = responseStylesJson as ResponseStyleOption[];

/** 简单页自定义供应商可选的文本协议（与高级配置页 text 能力列表一致） */
export const simpleSetupProtocolOptions: Array<{ value: ApiRequestFormat; label: string }> = [
  { value: "auto", label: "Auto" },
  { value: "openai", label: "OpenAI Compatible" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "codex", label: "OpenAI Codex" },
  { value: "gemini", label: "Google Gemini" },
  { value: "anthropic", label: "Anthropic" },
  { value: "fireworks", label: "Fireworks" },
  { value: "together", label: "Together AI" },
  { value: "groq", label: "Groq" },
  { value: "mimo", label: "Mimo" },
  { value: "minimax", label: "MiniMax" },
  { value: "moonshot", label: "Moonshot/Kimi" },
  { value: "nebius", label: "Nebius" },
  { value: "xai", label: "xAI" },
  { value: "zai", label: "Zai" },
  { value: "bigmodel", label: "BigModel" },
  { value: "aliyun", label: "Aliyun" },
  { value: "baidu", label: "Baidu" },
  { value: "cohere", label: "Cohere" },
  { value: "ollama", label: "Ollama" },
  { value: "ollama_cloud", label: "Ollama Cloud" },
  { value: "vertex", label: "Google Vertex AI" },
  { value: "github_copilot", label: "GitHub Copilot" },
  { value: "opencode_go", label: "OpenCode Go" },
  { value: "bedrock_api", label: "AWS Bedrock API" },
];

export function createDraftModelCard(id: string, model: string, reasoningEffort: SimpleReasoningEffort): ApiModelConfigItem {
  return {
    id,
    model,
    displayName: "",
    enableImage: false,
    enableAudio: false,
    enableVideo: false,
    enableTools: true,
    reasoningEffort,
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 256000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
  };
}

export function defaultSimpleSetupDraft(): SimpleSetupDraft {
  const preset = simpleProviderOptions[0];
  return {
    providerId: preset.id,
    apiKey: "",
    customBaseUrl: preset.baseUrl,
    customRequestFormat: "auto",
    models: {
      quick: createDraftModelCard(SIMPLE_SETUP_MODEL_IDS.quick, preset.defaultModel, "low"),
      expert: createDraftModelCard(SIMPLE_SETUP_MODEL_IDS.expert, preset.defaultModel, "high"),
      vision: createDraftModelCard(SIMPLE_SETUP_MODEL_IDS.vision, preset.visionModel ?? preset.defaultModel, "medium"),
    },
    customModelOptions: [],
    responseStyleId: "none",
    uiLanguage: "zh-CN",
    hotkey: "Alt+·",
    recordHotkey: "CapsLock",
    siliconFlowKey: "",
  };
}

function parseDraft(raw: unknown): SimpleSetupDraft | null {
  if (!raw || typeof raw !== "object") return null;
  const draft = defaultSimpleSetupDraft();
  const obj = raw as Record<string, unknown>;
  const providerId = String(obj.providerId || "");
  if (simpleProviderOptions.some((item) => item.id === providerId)) draft.providerId = providerId as SimpleProviderId;
  draft.apiKey = String(obj.apiKey || "");
  draft.customBaseUrl = String(obj.customBaseUrl || "");
  draft.customRequestFormat = normalizeApiRequestFormat(obj.customRequestFormat);
  const models = (obj.models && typeof obj.models === "object" ? obj.models : {}) as Record<string, Record<string, unknown>>;
  for (const card of Object.keys(SIMPLE_SETUP_MODEL_IDS) as SimpleModelCard[]) {
    const cardRaw = models[card] || {};
    const model = String(cardRaw.model || "");
    if (model) draft.models[card].model = model;
    const displayName = String(cardRaw.displayName || "");
    if (displayName) draft.models[card].displayName = displayName;
    const effort = String(cardRaw.reasoningEffort || "");
    if (effort === "low" || effort === "medium" || effort === "high") draft.models[card].reasoningEffort = effort;
    if (typeof cardRaw.enableImage === "boolean") draft.models[card].enableImage = cardRaw.enableImage;
    if (typeof cardRaw.enableAudio === "boolean") draft.models[card].enableAudio = cardRaw.enableAudio;
    if (typeof cardRaw.enableVideo === "boolean") draft.models[card].enableVideo = cardRaw.enableVideo;
    if (typeof cardRaw.enableTools === "boolean") draft.models[card].enableTools = cardRaw.enableTools;
    if (typeof cardRaw.contextWindowTokens === "number") draft.models[card].contextWindowTokens = cardRaw.contextWindowTokens;
    if (typeof cardRaw.maxOutputTokens === "number") draft.models[card].maxOutputTokens = cardRaw.maxOutputTokens;
    if (typeof cardRaw.temperature === "number") draft.models[card].temperature = cardRaw.temperature;
    if (typeof cardRaw.customTemperatureEnabled === "boolean") draft.models[card].customTemperatureEnabled = cardRaw.customTemperatureEnabled;
    if (typeof cardRaw.customMaxOutputTokensEnabled === "boolean") draft.models[card].customMaxOutputTokensEnabled = cardRaw.customMaxOutputTokensEnabled;
  }
  const style = String(obj.responseStyleId || "");
  if (responseStyleOptions.some((item) => item.id === style) || style === "none") draft.responseStyleId = style;
  const customModels = obj.customModelOptions;
  if (Array.isArray(customModels)) {
    draft.customModelOptions = customModels.map((value) => String(value || "").trim()).filter(Boolean);
  }
  const lang = String(obj.uiLanguage || "");
  if (lang === "zh-CN" || lang === "zh-TW" || lang === "en-US") draft.uiLanguage = lang;
  const hotkey = String(obj.hotkey || "");
  if (hotkey) draft.hotkey = hotkey;
  const recordHotkey = String(obj.recordHotkey || "");
  if (recordHotkey) draft.recordHotkey = recordHotkey;
  draft.siliconFlowKey = String(obj.siliconFlowKey || "");
  const preset = simpleProviderOptions.find((item) => item.id === draft.providerId);
  if (preset?.visionModel) {
    const visionModel = String(draft.models.vision.model || "").trim();
    if (!visionModel || visionModel === preset.defaultModel || LEGACY_VISION_MODELS.has(visionModel)) {
      draft.models.vision.model = preset.visionModel;
    }
  }
  return draft;
}

export function loadSimpleSetupDraft(): SimpleSetupDraft | null {
  try {
    const raw = localStorage.getItem(SIMPLE_SETUP_DRAFT_KEY);
    if (!raw) return null;
    return parseDraft(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function saveSimpleSetupDraft(draft: SimpleSetupDraft) {
  try {
    localStorage.setItem(SIMPLE_SETUP_DRAFT_KEY, JSON.stringify(draft));
  } catch {
    // 忽略草稿写入失败
  }
}

export function clearSimpleSetupDraft() {
  try {
    localStorage.removeItem(SIMPLE_SETUP_DRAFT_KEY);
  } catch {
    // 忽略
  }
}

export function useSimpleSetup() {
  const { t, locale } = useI18n();
  const { restoreThemeFromStorage } = useAppTheme();

  const loading = ref(true);
  const saving = ref(false);
  const errorText = ref("");
  const statusText = ref("");
  const showApiKey = ref(false);
  const showSiliconFlowKey = ref(false);
  const hotkeyCaptureTarget = ref<"summon" | "record" | null>(null);
  const hotkeyCaptureHint = ref(t("quickSetup.hotkeyHints.idle"));
  let hotkeyCaptureHandler: ((event: KeyboardEvent) => void) | null = null;

  const config = reactive<AppConfig>(defaultConfig());
  const chatSettings = reactive<ChatSettings>(defaultChatSettings());

  const draft = reactive<SimpleSetupDraft>(defaultSimpleSetupDraft());
  const hasDraft = ref(false);

  const languageOptions = [
    { value: "zh-CN" as const, label: t("quickSetup.languages.zhCN") },
    { value: "zh-TW" as const, label: t("quickSetup.languages.zhTW") },
    { value: "en-US" as const, label: t("quickSetup.languages.enUS") },
  ];

  const selectedProvider = computed(() => simpleProviderOptions.find((item) => item.id === draft.providerId) || simpleProviderOptions[0]);
  const providerApiKeyUrl = computed(() => selectedProvider.value.keyUrl);

  function defaultConfig(): AppConfig {
    return {
      hotkey: "Alt+·",
      uiLanguage: "zh-CN",
      uiFont: "auto",
      codeFont: "auto",
      uiSizeScale: 100,
      webAccessPort: 8429,
      webAccessEnabled: true,
      webAccessPassword: "",
      recordHotkey: "CapsLock",
      recordBackgroundWakeEnabled: false,
      minRecordSeconds: 1,
      maxRecordSeconds: 60,
      llmRoundLogCapacity: 3,
      messageNotificationEnabled: true,
      messageNotificationSoundEnabled: false,
      desktopOperationNoticeEnabled: true,
      desktopOperateEnabled: true,
      selectedApiConfigId: "",
      expertApiConfigId: "",
      toolReviewApiConfigId: "",
      sttApiConfigId: "",
      sttAutoSend: false,
      shellWorkspaces: [],
      mcpServers: [],
      remoteImChannels: [],
      apiProviders: [],
      imageProviders: [],
      apiConfigs: [],
    };
  }

  function defaultChatSettings(): ChatSettings {
    return {
      assistantAgentId: "default-agent",
      userAlias: t("sidebar.quickSetupUserAlias"),
      responseStyleId: "none",
      pdfReadMode: "text",
      backgroundVoiceScreenshotKeywords: "",
      backgroundVoiceScreenshotMode: "desktop",
      instructionPresets: [],
    };
  }

  function findSimpleSetupConfig() {
    return (config.apiConfigs || []).find((api) => api.id === SIMPLE_SETUP_ENDPOINT_IDS.quick || api.id.startsWith(`${SIMPLE_SETUP_PROVIDER_ID}::`));
  }

  function isUsableTextLlmConfig(api: AppConfig["apiConfigs"][number]): boolean {
    return !!api.enableText
      && !["openai_stt", "mimo_asr", "openai_embedding", "openai_rerank", "gemini_embedding"].includes(api.requestFormat)
      && !!String(api.baseUrl || "").trim()
      && !!String(api.apiKey || "").trim()
      && !!String(api.model || "").trim();
  }

  function providerPresetFromConfig(requestFormat: ApiRequestFormat, baseUrl: string): SimpleProviderPreset {
    const normalizedBaseUrl = String(baseUrl || "").toLowerCase();
    const matchedPreset = simpleProviderOptions
      .filter((preset) => preset.id !== "custom")
      .find((preset) => {
        const presetHost = new URL(preset.baseUrl).host.toLowerCase();
        return preset.requestFormat === requestFormat && normalizedBaseUrl.includes(presetHost);
      });
    if (matchedPreset) return matchedPreset;
    const customPreset = simpleProviderOptions.find((preset) => preset.id === "custom") || simpleProviderOptions[0];
    return {
      ...customPreset,
      requestFormat,
      baseUrl: String(baseUrl || "").trim(),
    };
  }

  function applySnapshot(snapshot: { config: AppConfig; chatSettings: ChatSettings }) {
    Object.assign(config, defaultConfig(), snapshot.config || {});
    config.uiSizeScale = applyUiSizeScale(config.uiSizeScale);
    Object.assign(chatSettings, defaultChatSettings(), snapshot.chatSettings || {});
    if (!responseStyleOptions.some((style) => style.id === chatSettings.responseStyleId) && chatSettings.responseStyleId !== "none") {
      chatSettings.responseStyleId = "none";
    }
    const existing = findSimpleSetupConfig() || (config.apiConfigs || []).find(isUsableTextLlmConfig);
    if (existing) {
      const preset = providerPresetFromConfig(existing.requestFormat, existing.baseUrl);
      draft.providerId = preset.id;
      draft.customBaseUrl = preset.baseUrl;
      draft.customRequestFormat = preset.requestFormat;
      draft.apiKey = existing.apiKey;
      draft.models.quick.model = existing.model || preset.defaultModel;
      draft.models.expert.model = existing.model || preset.defaultModel;
      draft.models.vision.model = existing.model || (preset.visionModel ?? preset.defaultModel);
      draft.models.quick.displayName = String(existing.displayName || "").trim();
      draft.models.expert.displayName = String(existing.displayName || "").trim();
      draft.models.vision.displayName = String(existing.displayName || "").trim();
    }
    draft.hotkey = String(config.hotkey || "Alt+·").trim() || "Alt+·";
    draft.recordHotkey = String(config.recordHotkey || "CapsLock").trim() || "CapsLock";
    draft.uiLanguage = (config.uiLanguage === "zh-CN" || config.uiLanguage === "zh-TW" || config.uiLanguage === "en-US")
      ? config.uiLanguage
      : "zh-CN";
    draft.responseStyleId = chatSettings.responseStyleId || "none";
  }

  async function loadSnapshot() {
    try {
      restoreThemeFromStorage();
      const snapshot = await invokeTauri<{ config: AppConfig; chatSettings: ChatSettings }>("load_app_bootstrap_snapshot");
      applySnapshot(snapshot);
      const savedDraft = loadSimpleSetupDraft();
      if (savedDraft) {
        Object.assign(draft, savedDraft);
        hasDraft.value = true;
      }
      hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.idle");
    } catch (error) {
      errorText.value = t("sidebar.quickSetupLoadFailed", { error: String(error ?? "unknown") });
    } finally {
      loading.value = false;
    }
  }

  function selectProvider(providerId: SimpleProviderId) {
    draft.providerId = providerId;
    connectionTestResult.value = null;
    const preset = simpleProviderOptions.find((item) => item.id === providerId) || simpleProviderOptions[0];
    if (providerId === "custom") {
      draft.customBaseUrl = draft.customBaseUrl || preset.baseUrl;
      for (const card of Object.keys(SIMPLE_SETUP_MODEL_IDS) as SimpleModelCard[]) {
        draft.models[card].model = "";
      }
      draft.customModelOptions = [];
    } else {
      for (const card of Object.keys(SIMPLE_SETUP_MODEL_IDS) as SimpleModelCard[]) {
        draft.models[card].model = card === "vision" ? (preset.visionModel ?? preset.defaultModel) : preset.defaultModel;
      }
    }
    errorText.value = "";
    statusText.value = "";
  }

  const refreshingCustomModels = ref(false);
  const testingConnection = ref(false);
  const connectionTestResult = ref<{
    ok: boolean;
    message: string;
    latencyMs?: number;
  } | null>(null);

  async function testConnection() {
    const isCustom = draft.providerId === "custom";
    const baseUrl = (isCustom ? draft.customBaseUrl : selectedProvider.value.baseUrl).trim();
    const apiKey = draft.apiKey.trim();
    const requestFormat = isCustom ? draft.customRequestFormat : selectedProvider.value.requestFormat;

    if (!apiKey) {
      connectionTestResult.value = {
        ok: false,
        message: t("simpleSetup.needKeyToTest"),
      };
      return;
    }
    if (!baseUrl) {
      connectionTestResult.value = {
        ok: false,
        message: t("simpleSetup.needBaseUrlToTest"),
      };
      return;
    }

    testingConnection.value = true;
    connectionTestResult.value = null;
    const startTime = performance.now();

    try {
      const models = await invokeTauri<string[]>("refresh_models", {
        input: {
          baseUrl,
          apiKey,
          requestFormat,
          providerId: null,
          codexAuthMode: "read_local",
          codexLocalAuthPath: "~/.codex/auth.json",
        },
      });

      const latencyMs = Math.max(1, Math.round(performance.now() - startTime));

      if (isCustom && Array.isArray(models) && models.length > 0) {
        draft.customModelOptions = models.map((v) => String(v || "").trim()).filter(Boolean);
      }

      connectionTestResult.value = {
        ok: true,
        latencyMs,
        message: t("simpleSetup.connectionSuccess", { latency: latencyMs }),
      };
    } catch (error) {
      connectionTestResult.value = {
        ok: false,
        message: t("simpleSetup.connectionFailed", { error: String(error ?? "unknown") }),
      };
    } finally {
      testingConnection.value = false;
    }
  }

  function resetHotkeyDefault(target: "summon" | "record") {
    if (target === "summon") {
      draft.hotkey = "Alt+·";
    } else {
      draft.recordHotkey = "CapsLock";
    }
  }

  async function refreshCustomModels() {
    const baseUrl = draft.customBaseUrl.trim();
    const apiKey = draft.apiKey.trim();
    if (!baseUrl || !apiKey) {
      errorText.value = t("simpleSetup.refreshModelsNeedKey");
      return;
    }
    refreshingCustomModels.value = true;
    errorText.value = "";
    statusText.value = "";
    try {
      const models = await invokeTauri<string[]>("refresh_models", {
        input: {
          baseUrl,
          apiKey,
          requestFormat: draft.customRequestFormat,
          providerId: null,
          codexAuthMode: "read_local",
          codexLocalAuthPath: "~/.codex/auth.json",
        },
      });
      draft.customModelOptions = (models || []).map((value) => String(value || "").trim()).filter(Boolean);
      statusText.value = t("simpleSetup.refreshModelsDone", { count: draft.customModelOptions.length });
    } catch (error) {
      errorText.value = t("simpleSetup.refreshModelsFailed", { error: String(error ?? "unknown") });
    } finally {
      refreshingCustomModels.value = false;
    }
  }

  function openProviderKeyUrl() {
    const url = selectedProvider.value.keyUrl;
    if (url) void openTransportExternalUrl(url);
  }

  function openSiliconFlowKeyUrl() {
    void openTransportExternalUrl(SILICON_FLOW_KEY_URL);
  }

  function setUiLanguage(value: AppConfig["uiLanguage"]) {
    const lang = normalizeLocale(value);
    document.documentElement.lang = lang;
    draft.uiLanguage = lang;
    config.uiLanguage = lang;
    locale.value = lang;
    i18n.global.locale.value = lang;
    if (!hotkeyCaptureTarget.value) {
      hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.idle");
    }
  }

  function currentProviderBaseUrl(): string {
    return draft.providerId === "custom" ? draft.customBaseUrl.trim() : selectedProvider.value.baseUrl;
  }

  function currentProviderRequestFormat(): ApiRequestFormat {
    return draft.providerId === "custom" ? draft.customRequestFormat : selectedProvider.value.requestFormat;
  }

  function buildProviderAndEndpoints(): ApiProviderConfigItem & { endpoints: ApiConfigItem[] } {
    const baseUrl = currentProviderBaseUrl();
    const apiKey = draft.apiKey.trim();
    const requestFormat = normalizeApiRequestFormat(currentProviderRequestFormat());
    const enableText = requestFormat !== "openai_stt" && requestFormat !== "mimo_asr" && requestFormat !== "openai_embedding" && requestFormat !== "openai_rerank" && requestFormat !== "gemini_embedding";
    const includeVision = draft.providerId === "custom" || !!selectedProvider.value.visionModel;
    const configuredModelNames = [
      draft.models.quick.model.trim(),
      draft.models.expert.model.trim(),
      draft.models.vision.model.trim(),
    ].filter(Boolean);
    const cachedModelOptions = draft.providerId === "custom"
      ? Array.from(new Set([...draft.customModelOptions, ...configuredModelNames]))
      : configuredModelNames;
    const models: ApiProviderConfigItem["models"] = [
      {
        id: SIMPLE_SETUP_MODEL_IDS.quick,
        model: draft.models.quick.model.trim(),
        displayName: String(draft.models.quick.displayName || "").trim() || undefined,
        enableImage: draft.models.quick.enableImage,
        enableAudio: draft.models.quick.enableAudio ?? false,
        enableVideo: draft.models.quick.enableVideo ?? false,
        enableTools: enableText,
        reasoningEffort: draft.models.quick.reasoningEffort,
        temperature: 1,
        customTemperatureEnabled: false,
        contextWindowTokens: 256000,
        customMaxOutputTokensEnabled: false,
        maxOutputTokens: 4096,
      },
      {
        id: SIMPLE_SETUP_MODEL_IDS.expert,
        model: draft.models.expert.model.trim(),
        displayName: String(draft.models.expert.displayName || "").trim() || undefined,
        enableImage: draft.models.expert.enableImage,
        enableAudio: draft.models.expert.enableAudio ?? false,
        enableVideo: draft.models.expert.enableVideo ?? false,
        enableTools: enableText,
        reasoningEffort: draft.models.expert.reasoningEffort,
        temperature: 1,
        customTemperatureEnabled: false,
        contextWindowTokens: 256000,
        customMaxOutputTokensEnabled: false,
        maxOutputTokens: 4096,
      },
      ...(includeVision ? [{
        id: SIMPLE_SETUP_MODEL_IDS.vision,
        model: draft.models.vision.model.trim(),
        displayName: String(draft.models.vision.displayName || "").trim() || undefined,
        enableImage: true,
        enableAudio: true,
        enableVideo: true,
        enableTools: enableText,
        reasoningEffort: "medium",
        temperature: 1,
        customTemperatureEnabled: false,
        contextWindowTokens: 256000,
        customMaxOutputTokensEnabled: false,
        maxOutputTokens: 4096,
      }] : []),
    ];
    const provider: ApiProviderConfigItem = {
      id: SIMPLE_SETUP_PROVIDER_ID,
      name: t("simpleSetup.providerName"),
      requestFormat,
      allowConcurrentRequests: true,
      enableText,
      enableImage: includeVision,
      enableAudio: false,
      enableTools: enableText,
      tools: defaultToolBindings(),
      baseUrl,
      codexAuthMode: "read_local",
      codexLocalAuthPath: "~/.codex/auth.json",
      apiKeys: apiKey ? [apiKey] : [],
      keyCursor: 0,
      cachedModelOptions,
      models,
      failureRetryCount: 0,
    };
    const endpoints: ApiConfigItem[] = provider.models.map((model) => ({
      id: `${provider.id}::${model.id}`,
      name: apiConfigDisplayName(provider.name, model.model, model.reasoningEffort || "medium"),
      requestFormat: provider.requestFormat,
      allowConcurrentRequests: true,
      maxConcurrentRequests: null,
      enableText: provider.enableText,
      enableImage: model.enableImage,
      enableAudio: model.enableAudio ?? provider.enableAudio,
      enableVideo: model.enableVideo ?? false,
      enableTools: model.enableTools,
      tools: defaultToolBindings(),
      baseUrl: provider.baseUrl,
      apiKey,
      codexAuthMode: "read_local",
      codexLocalAuthPath: "~/.codex/auth.json",
      model: model.model,
      displayName: String(model.displayName || "").trim() || undefined,
      reasoningEffort: model.reasoningEffort,
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: 256000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
    }));
    return { ...provider, endpoints };
  }

  function applyLlmDraft() {
    const { endpoints } = buildProviderAndEndpoints();
    config.apiProviders = [
      ...(config.apiProviders || []).filter((item) => item.id !== SIMPLE_SETUP_PROVIDER_ID),
      buildProviderAndEndpoints(),
    ];
    config.apiConfigs = [
      ...(config.apiConfigs || []).filter((item) => !item.id.startsWith(`${SIMPLE_SETUP_PROVIDER_ID}::`)),
      ...endpoints,
    ];
    config.selectedApiConfigId = SIMPLE_SETUP_ENDPOINT_IDS.expert;
    config.expertApiConfigId = SIMPLE_SETUP_ENDPOINT_IDS.expert;
    config.toolReviewApiConfigId = SIMPLE_SETUP_ENDPOINT_IDS.quick;
    const includeVision = draft.providerId === "custom" || !!selectedProvider.value.visionModel;
    config.visionApiConfigId = includeVision ? SIMPLE_SETUP_ENDPOINT_IDS.vision : "";
  }

  async function saveChatSettingsOnly() {
    const saved = await invokeTauri<ChatSettings>("patch_chat_settings", {
      input: {
        assistantAgentId: chatSettings.assistantAgentId,
        userAlias: chatSettings.userAlias,
        responseStyleId: draft.responseStyleId,
      },
    });
    Object.assign(chatSettings, saved);
  }

  async function saveConfigOnly() {
    try {
      const result = await invokeTauri<{ config: AppConfig }>("save_config", { config: { ...config } });
      Object.assign(config, result.config);
    } catch (error) {
      throw new Error(t("sidebar.quickSetupSaveFailed", { error: String(error ?? "unknown") }));
    }
  }

  function upsertAdvancedProvider(
    draftModel: { name: string; requestFormat: ApiRequestFormat; baseUrl: string; apiKey: string; model: string },
  ): string {
    const seed = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const providerId = `api-provider-${seed}`;
    const modelId = `api-model-${seed}`;
    const endpointId = `${providerId}::${modelId}`;
    const requestFormat = normalizeApiRequestFormat(draftModel.requestFormat);
    const isStt = requestFormat === "openai_stt" || requestFormat === "mimo_asr";
    const isEmbedding = requestFormat === "openai_embedding" || requestFormat === "gemini_embedding";
    const isRerank = requestFormat === "openai_rerank";
    const provider: ApiProviderConfigItem = {
      id: providerId,
      name: draftModel.name.trim(),
      requestFormat,
      allowConcurrentRequests: true,
      enableText: !isStt && !isEmbedding && !isRerank,
      enableImage: !isStt && !isEmbedding && !isRerank,
      enableAudio: isStt,
      enableTools: !isStt && !isEmbedding && !isRerank,
      tools: defaultToolBindings(),
      baseUrl: draftModel.baseUrl.trim(),
      codexAuthMode: "read_local",
      codexLocalAuthPath: "~/.codex/auth.json",
      apiKeys: draftModel.apiKey.trim() ? [draftModel.apiKey.trim()] : [],
      keyCursor: 0,
      cachedModelOptions: [draftModel.model.trim()],
      models: [{
        id: modelId,
        model: draftModel.model.trim(),
        enableImage: !isStt && !isEmbedding && !isRerank,
        enableTools: !isStt && !isEmbedding && !isRerank,
        reasoningEffort: "medium",
        temperature: 1,
        customTemperatureEnabled: false,
        contextWindowTokens: 256000,
        customMaxOutputTokensEnabled: false,
        maxOutputTokens: 4096,
      }],
      failureRetryCount: 0,
    };
    config.apiProviders = [...(config.apiProviders || []).filter((item) => item.id !== providerId), provider];
    config.apiConfigs = [...(config.apiConfigs || []).filter((item) => !item.id.startsWith(`${providerId}::`)), {
      id: endpointId,
      name: apiConfigDisplayName(provider.name, draftModel.model.trim(), "medium"),
      requestFormat: provider.requestFormat,
      allowConcurrentRequests: true,
      maxConcurrentRequests: null,
      enableText: provider.enableText,
      enableImage: provider.models[0].enableImage,
      enableAudio: provider.enableAudio,
      enableTools: provider.models[0].enableTools,
      tools: defaultToolBindings(),
      baseUrl: provider.baseUrl,
      apiKey: draftModel.apiKey.trim(),
      codexAuthMode: "read_local",
      codexLocalAuthPath: "~/.codex/auth.json",
      model: draftModel.model.trim(),
      reasoningEffort: "medium",
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: 256000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
    }];
    return endpointId;
  }

  async function saveSiliconFlowIfConfigured() {
    const apiKey = draft.siliconFlowKey.trim();
    if (!apiKey) return;
    const embeddingEndpointId = upsertAdvancedProvider({
      name: "SiliconFlow Embedding",
      requestFormat: "openai_embedding",
      baseUrl: SILICON_FLOW_BASE_URL,
      apiKey,
      model: SILICON_FLOW_EMBEDDING_MODEL,
    });
    const rerankEndpointId = upsertAdvancedProvider({
      name: "SiliconFlow Rerank",
      requestFormat: "openai_rerank",
      baseUrl: SILICON_FLOW_BASE_URL,
      apiKey,
      model: SILICON_FLOW_RERANK_MODEL,
    });
    const sttEndpointId = upsertAdvancedProvider({
      name: "SiliconFlow STT",
      requestFormat: "openai_stt",
      baseUrl: SILICON_FLOW_BASE_URL,
      apiKey,
      model: SILICON_FLOW_STT_MODEL,
    });
    await saveConfigOnly();
    await invokeTauri("save_memory_embedding_binding", { input: { apiConfigId: embeddingEndpointId, modelName: SILICON_FLOW_EMBEDDING_MODEL, batchSize: 64 } });
    await invokeTauri("save_memory_rerank_binding", { input: { apiConfigId: rerankEndpointId, modelName: SILICON_FLOW_RERANK_MODEL } });
    config.sttApiConfigId = sttEndpointId;
    await saveConfigOnly();
  }

  async function saveAll() {
    if (saving.value) return;
    const quickModel = draft.models.quick.model.trim();
    const expertModel = draft.models.expert.model.trim();
    const visionModel = draft.models.vision.model.trim();
    const apiKey = draft.apiKey.trim();
    const baseUrl = currentProviderBaseUrl();
    if (!baseUrl || !apiKey || !quickModel || !expertModel || !visionModel) {
      errorText.value = t("sidebar.quickSetupLlmRequired");
      return;
    }
    saving.value = true;
    errorText.value = "";
    statusText.value = "";
    try {
      config.hotkey = draft.hotkey.trim() || "Alt+·";
      config.recordHotkey = draft.recordHotkey.trim() || "CapsLock";
      config.uiLanguage = draft.uiLanguage;
      chatSettings.responseStyleId = draft.responseStyleId;
      applyLlmDraft();
      await saveChatSettingsOnly();
      await saveConfigOnly();
      await saveSiliconFlowIfConfigured();
      if (!hasUsableTextLlm(config)) {
        throw new Error(t("sidebar.quickSetupNoLlmDetected"));
      }
      clearSimpleSetupDraft();
      hasDraft.value = false;
      statusText.value = t("status.saved");
    } catch (error) {
      errorText.value = String(error ?? "unknown");
    } finally {
      saving.value = false;
    }
  }

  function startHotkeyCapture(target: "summon" | "record") {
    stopHotkeyCapture();
    hotkeyCaptureTarget.value = target;
    hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.recording");
    hotkeyCaptureHandler = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        if (target === "record") {
          draft.recordHotkey = "";
          hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.cleared");
          stopHotkeyCapture();
          return;
        }
        hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.cancelled");
        stopHotkeyCapture();
        return;
      }
      const combo = keyboardEventToHotkey(event, target === "summon");
      if (!combo) {
        hotkeyCaptureHint.value = target === "summon"
          ? t("quickSetup.hotkeyHints.summonNeedsModifier")
          : t("quickSetup.hotkeyHints.unrecognized");
        return;
      }
      if (target === "summon") draft.hotkey = combo;
      else draft.recordHotkey = combo;
      hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.recorded", { combo });
      stopHotkeyCapture();
    };
    window.addEventListener("keydown", hotkeyCaptureHandler, true);
  }

  function stopHotkeyCapture() {
    if (hotkeyCaptureHandler) {
      window.removeEventListener("keydown", hotkeyCaptureHandler, true);
      hotkeyCaptureHandler = null;
    }
    hotkeyCaptureTarget.value = null;
  }

  onBeforeUnmount(() => stopHotkeyCapture());

  return {
    loading,
    saving,
    errorText,
    statusText,
    showApiKey,
    showSiliconFlowKey,
    hotkeyCaptureTarget,
    hotkeyCaptureHint,
    config,
    chatSettings,
    draft,
    hasDraft,
    languageOptions,
    selectedProvider,
    providerApiKeyUrl,
    refreshingCustomModels,
    testingConnection,
    connectionTestResult,
    testConnection,
    resetHotkeyDefault,
    loadSnapshot,
    selectProvider,
    refreshCustomModels,
    openProviderKeyUrl,
    openSiliconFlowKeyUrl,
    setUiLanguage,
    startHotkeyCapture,
    stopHotkeyCapture,
    saveAll,
    saveSimpleSetupDraft,
  };
}

function keyboardEventToHotkey(event: KeyboardEvent, requireModifier: boolean): string {
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.metaKey) modifiers.push("Meta");
  const raw = event.key;
  const lower = raw.toLowerCase();
  const modifierOnly: Record<string, string> = { control: "Ctrl", alt: "Alt", shift: "Shift", meta: "Meta" };
  if (modifierOnly[lower]) return requireModifier ? "" : modifierOnly[lower];
  const main = lower === " " ? "Space" : lower === "`" ? "·" : raw.length === 1 ? raw.toUpperCase() : raw;
  if (requireModifier && modifiers.length === 0) return "";
  return [...modifiers, main].join("+");
}
