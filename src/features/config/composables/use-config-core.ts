import type { ComputedRef } from "vue";
import type { ApiConfigItem, ApiModelConfigItem, ApiProviderConfigItem, AppConfig, CodexAuthMode, RemoteImChannelConfig, RemoteImPlatform } from "../../../types/app";
import { defaultToolBindings } from "../utils/builtin-tools";
import { normalizeApiRequestFormat } from "../utils/api-request-format";
import { apiConfigDisplayName as buildApiConfigDisplayName } from "../utils/api-config-display";
import {
  normalizeImageGenerationModelId,
  normalizeImageGenerationProviders,
} from "../utils/image-generation-config";

function normalizeRemoteImPlatform(value: unknown): RemoteImPlatform {
  const text = String(value || "").trim().toLowerCase();
  if (text === "feishu" || text === "dingtalk" || text === "onebot_v11" || text === "weixin_oc") {
    return text as RemoteImPlatform;
  }
  return "onebot_v11";
}

function normalizeGithubUpdateMethod(value: unknown): AppConfig["githubUpdateMethod"] {
  const text = String(value || "").trim();
  return text === "direct" || text === "proxy" ? text : "auto";
}

function normalizeSkippedGithubUpdateVersion(value: unknown): string {
  return String(value || "").trim();
}

function normalizeWebAccessPort(value: unknown): number {
  const parsed = Math.round(Number(value));
  if (Number.isFinite(parsed) && parsed >= 1024 && parsed <= 65535) {
    return parsed;
  }
  return 8429;
}

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
  t?: (key: string, params?: Record<string, unknown>) => string;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  const DEFAULT_MAX_OUTPUT_TOKENS = 4096;
  const DEFAULT_CONTEXT_WINDOW_TOKENS = 256000;
  const DEFAULT_CODEX_AUTH_MODE = "read_local";
  const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
  const DEFAULT_CODEX_ORIGINATOR = "codex-tui";
  const DEFAULT_REASONING_EFFORT = "medium";

  function apiConfigDisplayName(providerName: string, modelValue: string, reasoningEffort: string): string {
    return buildApiConfigDisplayName(providerName, modelValue, reasoningEffort, options.t);
  }

  function toFiniteMaxOutputTokens(value: unknown): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : DEFAULT_MAX_OUTPUT_TOKENS;
  }

  function defaultApiTools() {
    return defaultToolBindings();
  }

  function normalizeCodexAuthMode(value: unknown): CodexAuthMode {
    const normalized = String(value || "").trim();
    if (normalized === "managed_oauth") return "managed_oauth";
    if (normalized === "custom_url") return "custom_url";
    return "read_local";
  }

  function effectiveProviderBaseUrl(provider: ApiProviderConfigItem): string {
    const authMode = normalizeCodexAuthMode(provider.codexAuthMode);
    if (normalizeApiRequestFormat(provider.requestFormat) === "codex" && authMode === "custom_url") {
      return String(provider.codexCustomUrl || "").trim() || provider.baseUrl;
    }
    return provider.baseUrl;
  }

  function createApiModel(seed = Date.now().toString(), model = ""): ApiModelConfigItem {
    return {
      id: `api-model-${seed}`,
      model,
      deprecated: false,
      enableImage: true,
      enableAudio: false,
      enableTools: true,
      reasoningEffort: DEFAULT_REASONING_EFFORT,
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: DEFAULT_MAX_OUTPUT_TOKENS,
    };
  }

  function createApiProvider(seed = Date.now().toString(), modelName = ""): ApiProviderConfigItem {
    return {
      id: `api-provider-${seed}`,
      name: `API Provider ${options.config.apiProviders.length + 1}`,
      deprecated: false,
      requestFormat: "openai",
      allowConcurrentRequests: true,
      maxConcurrentRequests: null,
      enableText: true,
      enableImage: true,
      enableAudio: false,
      enableVideo: false,
      enableTools: true,
      tools: defaultApiTools(),
      baseUrl: "https://api.openai.com/v1",
      codexAuthMode: DEFAULT_CODEX_AUTH_MODE,
      codexLocalAuthPath: DEFAULT_CODEX_LOCAL_AUTH_PATH,
      codexCustomUrl: "",
      codexCustomApiKey: "",
      codexOriginator: DEFAULT_CODEX_ORIGINATOR,
      codexResidencyRequirement: undefined,
      apiKeys: [],
      keyCursor: 0,
      cachedModelOptions: modelName ? [modelName] : [],
      models: [createApiModel(seed, modelName)],
      failureRetryCount: 0,
    };
  }

  function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
    const provider = createApiProvider(seed, "gpt-4o-mini");
    const model = provider.models[0];
    const reasoningEffort = String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT;
    return {
      id: `${provider.id}::${model.id}`,
      name: apiConfigDisplayName(provider.name, model.model, reasoningEffort),
      requestFormat: normalizeApiRequestFormat(provider.requestFormat),
      allowConcurrentRequests: !!provider.allowConcurrentRequests,
      maxConcurrentRequests: provider.maxConcurrentRequests ?? null,
      enableText: provider.enableText,
      enableImage: model.enableImage,
      enableAudio: provider.enableAudio,
      enableVideo: model.enableVideo,
      enableTools: model.enableTools,
      tools: defaultApiTools(),
      baseUrl: provider.baseUrl,
      apiKey: "",
      codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
      codexLocalAuthPath: String(provider.codexLocalAuthPath || "").trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
      model: model.model,
      displayName: String(model.displayName || "").trim() || undefined,
      reasoningEffort: String(model.reasoningEffort || "").trim() || DEFAULT_REASONING_EFFORT,
      temperature: model.temperature,
      customTemperatureEnabled: false,
      contextWindowTokens: model.contextWindowTokens,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: model.maxOutputTokens,
    };
  }

  function normalizeApiBindingsLocal() {
    if (options.config.apiProviders.length === 0) {
      if (options.config.apiConfigs.length > 0) {
        console.info("[配置迁移] 开始", {
          taskName: "legacy_api_configs_to_api_providers",
          configCount: options.config.apiConfigs.length,
        });
        options.config.apiProviders = options.config.apiConfigs.map((api, index) => ({
          id: `api-provider-legacy-${index + 1}`,
          name: api.name,
          deprecated: false,
          requestFormat: normalizeApiRequestFormat(api.requestFormat),
          allowConcurrentRequests: !!api.allowConcurrentRequests,
          maxConcurrentRequests: api.maxConcurrentRequests ?? null,
          enableText: !!api.enableText,
          enableImage: !!api.enableImage,
          enableAudio: !!api.enableAudio,
          enableVideo: !!api.enableVideo,
          enableTools: !!api.enableTools,
          tools: (api.tools || []).map((tool) => ({ ...tool, args: [...(tool.args || [])], values: { ...(tool.values || {}) } })),
          baseUrl: api.baseUrl,
          codexAuthMode: normalizeCodexAuthMode(api.codexAuthMode),
          codexLocalAuthPath: String(api.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
          codexCustomUrl: String(api.codexCustomUrl || "").trim(),
          codexCustomApiKey: String(api.codexCustomApiKey || "").trim(),
          codexOriginator: String(api.codexOriginator || DEFAULT_CODEX_ORIGINATOR).trim() || DEFAULT_CODEX_ORIGINATOR,
          codexResidencyRequirement: String(api.codexResidencyRequirement || "").trim() || undefined,
          apiKeys: api.apiKey ? [api.apiKey] : [],
          keyCursor: 0,
          cachedModelOptions: api.model ? [api.model] : [],
          models: [{
            id: `api-model-legacy-${index + 1}`,
            model: api.model,
            deprecated: false,
            enableImage: !!api.enableImage,
            enableAudio: !!api.enableAudio,
            enableVideo: !!api.enableVideo,
            enableTools: !!api.enableTools,
            reasoningEffort: String(api.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
            temperature: Number(api.temperature ?? 1),
            customTemperatureEnabled: !!api.customTemperatureEnabled,
            contextWindowTokens: Math.round(Number(api.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS)),
            customMaxOutputTokensEnabled: !!api.customMaxOutputTokensEnabled,
            maxOutputTokens: toFiniteMaxOutputTokens(api.maxOutputTokens),
          }],
          failureRetryCount: 0,
        }));
      } else {
        options.config.apiProviders = [createApiProvider()];
      }
    }

    const endpointDraftById = new Map(
      (options.config.apiConfigs || []).map((api) => [String(api.id || "").trim(), api] as const),
    );
    for (const provider of options.config.apiProviders) {
      provider.deprecated = !!provider.deprecated;
      provider.requestFormat = normalizeApiRequestFormat(provider.requestFormat);
      const models = Array.isArray(provider.models) ? provider.models : [];
      provider.enableImage = models.some((model) => !!model.enableImage);
      provider.enableVideo = models.some((model) => !!model.enableVideo);
      provider.enableAudio = models.some((model) => !!model.enableAudio);
      provider.enableTools = models.some((model) => model.enableTools !== false);
      if (provider.requestFormat === "codex") {
        provider.enableImage = true;
        provider.enableVideo = false;
        provider.enableAudio = false;
        provider.enableTools = true;
      }
      for (const model of provider.models || []) {
        model.deprecated = !!model.deprecated;
        const endpointId = `${provider.id}::${model.id}`;
        const draft = endpointDraftById.get(endpointId);
        if (!draft) continue;
        provider.name = String(provider.name || "").trim() || provider.id;
        provider.requestFormat = normalizeApiRequestFormat(draft.requestFormat);
        provider.allowConcurrentRequests = !!draft.allowConcurrentRequests;
        provider.maxConcurrentRequests = draft.maxConcurrentRequests ?? null;
        provider.enableText = !!draft.enableText;
        provider.enableAudio = !!draft.enableAudio;
        provider.enableVideo = !!draft.enableVideo;
        provider.baseUrl = String(draft.baseUrl || "").trim();
        provider.codexAuthMode = normalizeCodexAuthMode(draft.codexAuthMode || provider.codexAuthMode);
        provider.codexLocalAuthPath = String(draft.codexLocalAuthPath || provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim()
          || DEFAULT_CODEX_LOCAL_AUTH_PATH;
        if (String(draft.apiKey || "").trim()) {
          provider.apiKeys = [String(draft.apiKey || "").trim(), ...(provider.apiKeys || []).slice(1)];
        }
        model.model = String(draft.model || "").trim();
        model.enableImage = !!draft.enableImage;
        model.enableAudio = !!draft.enableAudio;
        model.enableVideo = !!draft.enableVideo;
        model.enableTools = !!draft.enableTools;
        if (provider.requestFormat === "codex") {
          model.enableImage = true;
          model.enableAudio = false;
          model.enableVideo = false;
          model.enableTools = true;
        }
        model.reasoningEffort = String(draft.reasoningEffort || model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT;
        model.temperature = Number(draft.temperature ?? 1);
        model.customTemperatureEnabled = !!draft.customTemperatureEnabled;
        model.contextWindowTokens = Math.round(Number(draft.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS));
        model.customMaxOutputTokensEnabled = !!draft.customMaxOutputTokensEnabled;
        model.maxOutputTokens = toFiniteMaxOutputTokens(draft.maxOutputTokens);
      }
    }

    const nextApiConfigs: ApiConfigItem[] = [];
    for (const provider of options.config.apiProviders) {
      if (provider.deprecated) continue;
      const providerName = String(provider.name || "").trim() || provider.id;
      const apiKey = Array.isArray(provider.apiKeys)
        ? provider.apiKeys.map((value) => String(value || "").trim()).find(Boolean) || ""
        : "";
      const models = Array.isArray(provider.models) ? provider.models : [];
      for (const model of models) {
        if (model.deprecated) continue;
        const modelValue = String(model.model || "").trim();
        if (!modelValue) continue;
        const reasoningEffort = String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT;
        nextApiConfigs.push({
          id: `${provider.id}::${model.id}`,
          name: apiConfigDisplayName(providerName, modelValue, reasoningEffort),
          requestFormat: normalizeApiRequestFormat(provider.requestFormat),
          allowConcurrentRequests: !!provider.allowConcurrentRequests,
          maxConcurrentRequests: provider.maxConcurrentRequests ?? null,
          enableText: !!provider.enableText,
          enableImage: !!model.enableImage,
          enableAudio: !!model.enableAudio,
          enableVideo: !!model.enableVideo,
          enableTools: model.enableTools !== false,
          tools: (provider.tools || []).map((tool) => ({ ...tool, args: [...(tool.args || [])], values: { ...(tool.values || {}) } })),
          baseUrl: provider.baseUrl,
          apiKey,
          codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
          codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
          model: modelValue,
          displayName: String(model.displayName || "").trim() || undefined,
          reasoningEffort,
          temperature: Number(model.temperature ?? 1),
          customTemperatureEnabled: !!model.customTemperatureEnabled,
          contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS)),
          customMaxOutputTokensEnabled: !!model.customMaxOutputTokensEnabled,
          maxOutputTokens: toFiniteMaxOutputTokens(model.maxOutputTokens),
        });
      }
    }
    if (nextApiConfigs.length === 0) {
      const provider = options.config.apiProviders[0] ?? createApiProvider();
      const model = Array.isArray(provider.models) ? (provider.models[0] ?? createApiModel()) : createApiModel();
      const providerTools = Array.isArray(provider.tools)
        ? provider.tools.map((tool) => ({ ...tool, args: [...(tool.args || [])], values: { ...(tool.values || {}) } }))
        : [];
      const providerApiKey = Array.isArray(provider.apiKeys) ? (provider.apiKeys[0] || "") : "";
      const providerName = String(provider.name || "").trim() || provider.id;
      const modelValue = String(model.model || "").trim();
      const reasoningEffort = String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT;
      nextApiConfigs.push({
        id: `${provider.id}::${model.id}`,
        name: apiConfigDisplayName(providerName, modelValue, reasoningEffort),
        requestFormat: normalizeApiRequestFormat(provider.requestFormat),
        allowConcurrentRequests: !!provider.allowConcurrentRequests,
        maxConcurrentRequests: provider.maxConcurrentRequests ?? null,
        enableText: !!provider.enableText,
        enableImage: !!model.enableImage,
        enableAudio: !!model.enableAudio,
        enableVideo: !!model.enableVideo,
        enableTools: model.enableTools !== false,
        tools: providerTools,
        baseUrl: effectiveProviderBaseUrl(provider),
        apiKey: providerApiKey,
        codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
        codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        codexCustomUrl: String(provider.codexCustomUrl || "").trim() || undefined,
        codexCustomApiKey: String(provider.codexCustomApiKey || "").trim() || undefined,
        codexOriginator: String(provider.codexOriginator || "").trim() || undefined,
        codexResidencyRequirement: String(provider.codexResidencyRequirement || "").trim() || undefined,
        model: modelValue,
        displayName: String(model.displayName || "").trim() || undefined,
        reasoningEffort,
        temperature: Number(model.temperature ?? 1),
        customTemperatureEnabled: !!model.customTemperatureEnabled,
        contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS)),
        customMaxOutputTokensEnabled: !!model.customMaxOutputTokensEnabled,
        maxOutputTokens: toFiniteMaxOutputTokens(model.maxOutputTokens),
      });
    }
    options.config.apiConfigs.splice(0, options.config.apiConfigs.length, ...nextApiConfigs);
    if (!nextApiConfigs.some((item) => item.id === options.config.selectedApiConfigId)) {
      options.config.selectedApiConfigId = nextApiConfigs[0]?.id ?? "";
    }
  }

  function buildConfigPayload(): AppConfig {
    const imageProviders = normalizeImageGenerationProviders(options.config.imageProviders);
    const imageGenerationModelId = normalizeImageGenerationModelId(
      options.config.imageGenerationModelId,
      imageProviders,
    );
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: options.config.uiFont,
      codeFont: options.config.codeFont,
      uiSizeScale: options.config.uiSizeScale,
      webAccessPort: normalizeWebAccessPort(options.config.webAccessPort),
        webAccessEnabled: options.config.webAccessEnabled !== false,
        webAccessPassword: String(options.config.webAccessPassword || "").trim(),
        githubUpdateMethod: normalizeGithubUpdateMethod(options.config.githubUpdateMethod),
        skippedGithubUpdateVersion: normalizeSkippedGithubUpdateVersion(options.config.skippedGithubUpdateVersion),
        recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      llmRoundLogCapacity: normalizeLlmRoundLogCapacity(options.config.llmRoundLogCapacity),
      messageNotificationEnabled: !!options.config.messageNotificationEnabled,
      messageNotificationSoundEnabled: !!options.config.messageNotificationSoundEnabled,
      desktopOperationNoticeEnabled: !!options.config.desktopOperationNoticeEnabled,
      desktopOperateEnabled: !!options.config.desktopOperateEnabled,
      selectedApiConfigId: options.config.selectedApiConfigId,
      expertApiConfigId: options.config.expertApiConfigId,
      ...(options.config.visionApiConfigId ? { visionApiConfigId: options.config.visionApiConfigId } : {}),
      ...(imageGenerationModelId ? { imageGenerationModelId } : {}),
      ...(options.config.toolReviewApiConfigId ? { toolReviewApiConfigId: options.config.toolReviewApiConfigId } : {}),
      ...(options.config.sttApiConfigId ? { sttApiConfigId: options.config.sttApiConfigId } : {}),
      ...(options.config.sttAutoSend ? { sttAutoSend: true } : {}),
      terminalShellKind: String(options.config.terminalShellKind ?? ""),
      simpleSetupMode: false,
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      // `cachedTools` is runtime-derived and should not be client-controlled on save.
      mcpServers: (options.config.mcpServers || []).map((item) => ({
        id: item.id,
        name: item.name,
        enabled: !!item.enabled,
        definitionJson: item.definitionJson,
        toolPolicies: [...(item.toolPolicies || [])],
        lastStatus: item.lastStatus || "",
        lastError: item.lastError || "",
        updatedAt: item.updatedAt || "",
      })),
      remoteImChannels: (options.config.remoteImChannels || []).map((item): RemoteImChannelConfig => ({
        id: String(item.id || "").trim(),
        name: String(item.name || "").trim(),
        platform: normalizeRemoteImPlatform(item.platform),
        enabled: !!item.enabled,
        credentials: item.credentials && typeof item.credentials === "object" ? { ...item.credentials } : {},
        receiveFiles: item.receiveFiles !== false,
        streamingSend: !!item.streamingSend,
        showToolCalls: !!item.showToolCalls,
        filterMarkdown: !!item.filterMarkdown,
        allowSendFiles: !!item.allowSendFiles,
        behaviorSettings: item.behaviorSettings ? { ...item.behaviorSettings } : undefined,
      })),
      apiProviders: (options.config.apiProviders || []).map((provider) => ({
        id: provider.id,
        name: provider.name,
        deprecated: !!provider.deprecated,
        requestFormat: normalizeApiRequestFormat(provider.requestFormat),
        allowConcurrentRequests: !!provider.allowConcurrentRequests,
        maxConcurrentRequests: provider.maxConcurrentRequests ?? null,
        enableText: !!provider.enableText,
        enableImage: (provider.models || []).some((model) => !!model.enableImage),
        enableVideo: (provider.models || []).some((model) => !!model.enableVideo),
        enableAudio: (provider.models || []).some((model) => !!model.enableAudio),
        enableTools: (provider.models || []).some((model) => model.enableTools !== false),
        tools: (provider.tools || []).map((t) => ({
          id: t.id,
          command: t.command,
          args: Array.isArray(t.args) ? t.args : [],
          enabled: typeof t.enabled === "boolean" ? t.enabled : true,
          values: t.values ?? {},
        })),
        baseUrl: effectiveProviderBaseUrl(provider),
        codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
        codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        codexCustomUrl: String(provider.codexCustomUrl || "").trim() || undefined,
        codexCustomApiKey: String(provider.codexCustomApiKey || "").trim() || undefined,
        codexOriginator: String(provider.codexOriginator || "").trim() || undefined,
        codexResidencyRequirement: String(provider.codexResidencyRequirement || "").trim() || undefined,
        apiKeys: Array.isArray(provider.apiKeys) ? provider.apiKeys.map((value) => String(value || "").trim()).filter(Boolean) : [],
        keyCursor: Math.max(0, Math.round(Number(provider.keyCursor ?? 0))),
        cachedModelOptions: Array.isArray(provider.cachedModelOptions)
          ? provider.cachedModelOptions.map((value) => String(value || "").trim()).filter(Boolean)
          : [],
        models: (provider.models || []).map((model) => ({
          id: model.id,
          model: model.model,
          displayName: String(model.displayName || "").trim(),
          deprecated: !!model.deprecated,
          enableImage: !!model.enableImage,
          enableAudio: !!model.enableAudio,
          enableVideo: !!model.enableVideo,
          enableTools: true,
          reasoningEffort: String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
          temperature: Number(model.temperature ?? 1),
          customTemperatureEnabled: !!model.customTemperatureEnabled,
          contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS)),
          customMaxOutputTokensEnabled: !!model.customMaxOutputTokensEnabled,
          maxOutputTokens: toFiniteMaxOutputTokens(model.maxOutputTokens),
        })),
        failureRetryCount: Math.max(0, Math.round(Number(provider.failureRetryCount ?? 0))),
      })),
      imageProviders,
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: normalizeApiRequestFormat(a.requestFormat),
        allowConcurrentRequests: !!a.allowConcurrentRequests,
        maxConcurrentRequests: a.maxConcurrentRequests ?? null,
        enableText: !!a.enableText,
        enableImage: !!a.enableImage,
        enableAudio: !!a.enableAudio,
        enableVideo: !!a.enableVideo,
        enableTools: true,
        tools: (a.tools || []).map((t) => ({
          id: t.id,
          command: t.command,
          args: Array.isArray(t.args) ? t.args : [],
          enabled: typeof t.enabled === "boolean" ? t.enabled : true,
          values: t.values ?? {},
        })),
        baseUrl: a.baseUrl,
        apiKey: a.apiKey,
        codexAuthMode: normalizeCodexAuthMode(a.codexAuthMode),
        codexLocalAuthPath: String(a.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        codexCustomUrl: String(a.codexCustomUrl || "").trim() || undefined,
        codexCustomApiKey: String(a.codexCustomApiKey || "").trim() || undefined,
        codexOriginator: String(a.codexOriginator || "").trim() || undefined,
        codexResidencyRequirement: String(a.codexResidencyRequirement || "").trim() || undefined,
        model: a.model,
        displayName: String(a.displayName || "").trim() || undefined,
        reasoningEffort: String(a.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
        temperature: Number(a.temperature ?? 1),
        customTemperatureEnabled: !!a.customTemperatureEnabled,
        contextWindowTokens: Math.round(Number(a.contextWindowTokens ?? DEFAULT_CONTEXT_WINDOW_TOKENS)),
        customMaxOutputTokensEnabled: !!a.customMaxOutputTokensEnabled,
        maxOutputTokens: toFiniteMaxOutputTokens(a.maxOutputTokens),
      })),
    };
  }

  function buildConfigSnapshotJson(): string {
    const imageProviders = normalizeImageGenerationProviders(options.config.imageProviders);
    return JSON.stringify({
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: options.config.uiFont,
      codeFont: options.config.codeFont,
      uiSizeScale: options.config.uiSizeScale,
      webAccessPort: normalizeWebAccessPort(options.config.webAccessPort),
      webAccessEnabled: options.config.webAccessEnabled !== false,
        webAccessPassword: String(options.config.webAccessPassword || "").trim(),
        githubUpdateMethod: normalizeGithubUpdateMethod(options.config.githubUpdateMethod),
        skippedGithubUpdateVersion: normalizeSkippedGithubUpdateVersion(options.config.skippedGithubUpdateVersion),
        recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      llmRoundLogCapacity: normalizeLlmRoundLogCapacity(options.config.llmRoundLogCapacity),
      messageNotificationEnabled: !!options.config.messageNotificationEnabled,
      messageNotificationSoundEnabled: !!options.config.messageNotificationSoundEnabled,
      desktopOperationNoticeEnabled: !!options.config.desktopOperationNoticeEnabled,
      desktopOperateEnabled: !!options.config.desktopOperateEnabled,
      // selectedApiConfigId 只是「上次使用的端点」行为记录，没有运行时消费方；
      // 它会在 load/normalize/人格切换时被静默改写，不该算 dirty。
      // 保留字段持久化（万一以后有用），但排除在 dirty 检测之外。
      expertApiConfigId: options.config.expertApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      imageGenerationModelId: normalizeImageGenerationModelId(
        options.config.imageGenerationModelId,
        imageProviders,
      ),
      toolReviewApiConfigId: options.config.toolReviewApiConfigId,
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: !!options.config.sttAutoSend,
      terminalShellKind: String(options.config.terminalShellKind ?? ""),
      simpleSetupMode: false,
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      mcpServers: [...(options.config.mcpServers || [])],
      remoteImChannels: [...(options.config.remoteImChannels || [])],
      apiProviders: [...(options.config.apiProviders || [])],
      imageProviders,
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: normalizeApiRequestFormat(a.requestFormat),
        enableText: a.enableText,
        enableImage: a.enableImage,
        enableAudio: a.enableAudio,
        enableVideo: a.enableVideo,
        enableTools: a.enableTools,
        tools: (a.tools || []).map((t) => ({
          id: t.id,
          command: t.command,
          args: Array.isArray(t.args) ? t.args : [],
          enabled: typeof t.enabled === "boolean" ? t.enabled : true,
          values: t.values ?? {},
        })),
        baseUrl: a.baseUrl,
        apiKey: a.apiKey,
        codexAuthMode: normalizeCodexAuthMode(a.codexAuthMode),
        codexLocalAuthPath: String(a.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        codexCustomUrl: String(a.codexCustomUrl || "").trim() || undefined,
        codexCustomApiKey: String(a.codexCustomApiKey || "").trim() || undefined,
        codexOriginator: String(a.codexOriginator || "").trim() || undefined,
        codexResidencyRequirement: String(a.codexResidencyRequirement || "").trim() || undefined,
        model: a.model,
        displayName: String(a.displayName || "").trim() || undefined,
        reasoningEffort: String(a.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
        temperature: a.temperature,
        customTemperatureEnabled: !!a.customTemperatureEnabled,
        contextWindowTokens: a.contextWindowTokens,
        customMaxOutputTokensEnabled: !!a.customMaxOutputTokensEnabled,
        maxOutputTokens: toFiniteMaxOutputTokens(a.maxOutputTokens),
      })),
    });
  }

  return {
    defaultApiTools,
    createApiProvider,
    createApiModel,
    createApiConfig,
    normalizeApiBindingsLocal,
    buildConfigPayload,
    buildConfigSnapshotJson,
  };
}

function normalizeLlmRoundLogCapacity(value: unknown): 1 | 3 | 10 {
  const numeric = Math.round(Number(value));
  if (numeric === 1 || numeric === 3 || numeric === 10) return numeric;
  if (!Number.isFinite(numeric) || numeric <= 0) return 3;
  if (numeric < 3) return 1;
  if (numeric < 10) return 3;
  return 10;
}
