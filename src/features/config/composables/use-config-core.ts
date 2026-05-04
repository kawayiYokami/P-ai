import type { ComputedRef } from "vue";
import type { ApiConfigItem, ApiModelConfigItem, ApiProviderConfigItem, AppConfig, CodexAuthMode, RemoteImChannelConfig, RemoteImPlatform } from "../../../types/app";
import { defaultToolBindings } from "../utils/builtin-tools";
import { normalizeApiRequestFormat } from "../utils/api-request-format";

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

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  const DEFAULT_MAX_OUTPUT_TOKENS = 4096;
  const DEFAULT_CODEX_AUTH_MODE = "read_local";
  const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
  const DEFAULT_REASONING_EFFORT = "medium";

  function toFiniteMaxOutputTokens(value: unknown): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : DEFAULT_MAX_OUTPUT_TOKENS;
  }

  function defaultApiTools() {
    return defaultToolBindings();
  }

  function normalizeCodexAuthMode(value: unknown): CodexAuthMode {
    return String(value || "").trim() === "managed_oauth" ? "managed_oauth" : "read_local";
  }

  function createApiModel(seed = Date.now().toString(), model = "gpt-4o-mini"): ApiModelConfigItem {
    return {
      id: `api-model-${seed}`,
      model,
      enableImage: true,
      enableTools: true,
      reasoningEffort: DEFAULT_REASONING_EFFORT,
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: 128000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
    };
  }

  function createApiProvider(seed = Date.now().toString()): ApiProviderConfigItem {
    return {
      id: `api-provider-${seed}`,
      name: `API Provider ${options.config.apiProviders.length + 1}`,
      requestFormat: "openai",
      allowConcurrentRequests: false,
      enableText: true,
      enableImage: true,
      enableAudio: false,
      enableTools: true,
      tools: defaultApiTools(),
      baseUrl: "https://api.openai.com/v1",
      codexAuthMode: DEFAULT_CODEX_AUTH_MODE,
      codexLocalAuthPath: DEFAULT_CODEX_LOCAL_AUTH_PATH,
      apiKeys: [],
      keyCursor: 0,
      cachedModelOptions: ["gpt-4o-mini"],
      models: [createApiModel(seed, "gpt-4o-mini")],
      failureRetryCount: 0,
    };
  }

  function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
    const provider = createApiProvider(seed);
    const model = provider.models[0];
    return {
      id: `${provider.id}::${model.id}`,
      name: `${provider.name}/${model.model}`,
      requestFormat: normalizeApiRequestFormat(provider.requestFormat),
      allowConcurrentRequests: !!provider.allowConcurrentRequests,
      enableText: provider.enableText,
      enableImage: model.enableImage,
      enableAudio: provider.enableAudio,
      enableTools: model.enableTools,
      tools: defaultApiTools(),
      baseUrl: provider.baseUrl,
      apiKey: "",
      codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
      codexLocalAuthPath: String(provider.codexLocalAuthPath || "").trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
      model: model.model,
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
          requestFormat: normalizeApiRequestFormat(api.requestFormat),
          allowConcurrentRequests: !!api.allowConcurrentRequests,
          enableText: !!api.enableText,
          enableImage: !!api.enableImage,
          enableAudio: !!api.enableAudio,
          enableTools: !!api.enableTools,
          tools: (api.tools || []).map((tool) => ({ ...tool, args: [...(tool.args || [])], values: { ...(tool.values || {}) } })),
          baseUrl: api.baseUrl,
          codexAuthMode: normalizeCodexAuthMode(api.codexAuthMode),
          codexLocalAuthPath: String(api.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
          apiKeys: api.apiKey ? [api.apiKey] : [],
          keyCursor: 0,
          cachedModelOptions: api.model ? [api.model] : [],
          models: [{
            id: `api-model-legacy-${index + 1}`,
            model: api.model,
            enableImage: !!api.enableImage,
            enableTools: !!api.enableTools,
            reasoningEffort: String(api.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
            temperature: Number(api.temperature ?? 1),
            customTemperatureEnabled: !!api.customTemperatureEnabled,
            contextWindowTokens: Math.round(Number(api.contextWindowTokens ?? 128000)),
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
      provider.requestFormat = normalizeApiRequestFormat(provider.requestFormat);
      const models = Array.isArray(provider.models) ? provider.models : [];
      provider.enableImage = models.some((model) => !!model.enableImage);
      provider.enableTools = models.some((model) => model.enableTools !== false);
      if (provider.requestFormat === "codex") {
        provider.enableImage = true;
        provider.enableTools = true;
      }
      for (const model of provider.models || []) {
        const endpointId = `${provider.id}::${model.id}`;
        const draft = endpointDraftById.get(endpointId);
        if (!draft) continue;
        provider.name = String(provider.name || "").trim() || provider.id;
        provider.requestFormat = normalizeApiRequestFormat(draft.requestFormat);
        provider.allowConcurrentRequests = !!draft.allowConcurrentRequests;
        provider.enableText = !!draft.enableText;
        provider.enableAudio = !!draft.enableAudio;
        provider.baseUrl = String(draft.baseUrl || "").trim();
        provider.codexAuthMode = normalizeCodexAuthMode(draft.codexAuthMode || provider.codexAuthMode);
        provider.codexLocalAuthPath = String(draft.codexLocalAuthPath || provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim()
          || DEFAULT_CODEX_LOCAL_AUTH_PATH;
        if (String(draft.apiKey || "").trim()) {
          provider.apiKeys = [String(draft.apiKey || "").trim(), ...(provider.apiKeys || []).slice(1)];
        }
        model.model = String(draft.model || "").trim();
        model.enableImage = !!draft.enableImage;
        model.enableTools = !!draft.enableTools;
        if (provider.requestFormat === "codex") {
          model.enableImage = true;
          model.enableTools = true;
        }
        model.reasoningEffort = String(draft.reasoningEffort || model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT;
        model.temperature = Number(draft.temperature ?? 1);
        model.customTemperatureEnabled = !!draft.customTemperatureEnabled;
        model.contextWindowTokens = Math.round(Number(draft.contextWindowTokens ?? 128000));
        model.customMaxOutputTokensEnabled = !!draft.customMaxOutputTokensEnabled;
        model.maxOutputTokens = toFiniteMaxOutputTokens(draft.maxOutputTokens);
      }
    }

    const nextApiConfigs: ApiConfigItem[] = [];
    for (const provider of options.config.apiProviders) {
      const providerName = String(provider.name || "").trim() || provider.id;
      const apiKey = Array.isArray(provider.apiKeys)
        ? provider.apiKeys.map((value) => String(value || "").trim()).find(Boolean) || ""
        : "";
      const models = Array.isArray(provider.models) ? provider.models : [];
      for (const model of models) {
        const modelValue = String(model.model || "").trim();
        if (!modelValue) continue;
        nextApiConfigs.push({
          id: `${provider.id}::${model.id}`,
          name: `${providerName}/${modelValue}`,
          requestFormat: normalizeApiRequestFormat(provider.requestFormat),
          allowConcurrentRequests: !!provider.allowConcurrentRequests,
          enableText: !!provider.enableText,
          enableImage: !!model.enableImage,
          enableAudio: !!provider.enableAudio,
          enableTools: model.enableTools !== false,
          tools: (provider.tools || []).map((tool) => ({ ...tool, args: [...(tool.args || [])], values: { ...(tool.values || {}) } })),
          baseUrl: provider.baseUrl,
          apiKey,
          codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
          codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
          model: modelValue,
          reasoningEffort: String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
          temperature: Number(model.temperature ?? 1),
          customTemperatureEnabled: !!model.customTemperatureEnabled,
          contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? 128000)),
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
      nextApiConfigs.push({
        id: `${provider.id}::${model.id}`,
        name: `${provider.name}/${model.model}`,
        requestFormat: normalizeApiRequestFormat(provider.requestFormat),
        allowConcurrentRequests: !!provider.allowConcurrentRequests,
        enableText: !!provider.enableText,
        enableImage: !!model.enableImage,
        enableAudio: !!provider.enableAudio,
        enableTools: model.enableTools !== false,
        tools: providerTools,
        baseUrl: provider.baseUrl,
        apiKey: providerApiKey,
        codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
        codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        model: model.model,
        reasoningEffort: String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
        temperature: Number(model.temperature ?? 1),
        customTemperatureEnabled: !!model.customTemperatureEnabled,
        contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? 128000)),
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
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: options.config.uiFont,
      webviewZoomPercent: options.config.webviewZoomPercent,
      githubUpdateMethod: normalizeGithubUpdateMethod(options.config.githubUpdateMethod),
      recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      llmRoundLogCapacity: normalizeLlmRoundLogCapacity(options.config.llmRoundLogCapacity),
      selectedApiConfigId: options.config.selectedApiConfigId,
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      ...(options.config.visionApiConfigId ? { visionApiConfigId: options.config.visionApiConfigId } : {}),
      ...(options.config.toolReviewApiConfigId ? { toolReviewApiConfigId: options.config.toolReviewApiConfigId } : {}),
      ...(options.config.sttApiConfigId ? { sttApiConfigId: options.config.sttApiConfigId } : {}),
      ...(options.config.sttAutoSend ? { sttAutoSend: true } : {}),
      terminalShellKind: String(options.config.terminalShellKind ?? ""),
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      departments: [...(options.config.departments || [])],
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
        activateAssistant: item.activateAssistant !== false,
        receiveFiles: item.receiveFiles !== false,
        streamingSend: !!item.streamingSend,
        showToolCalls: !!item.showToolCalls,
        allowSendFiles: !!item.allowSendFiles,
      })),
      apiProviders: (options.config.apiProviders || []).map((provider) => ({
        id: provider.id,
        name: provider.name,
        requestFormat: normalizeApiRequestFormat(provider.requestFormat),
        allowConcurrentRequests: !!provider.allowConcurrentRequests,
        enableText: !!provider.enableText,
        enableImage: (provider.models || []).some((model) => !!model.enableImage),
        enableAudio: !!provider.enableAudio,
        enableTools: (provider.models || []).some((model) => model.enableTools !== false),
        tools: (provider.tools || []).map((t) => ({
          id: t.id,
          command: t.command,
          args: Array.isArray(t.args) ? t.args : [],
          enabled: typeof t.enabled === "boolean" ? t.enabled : true,
          values: t.values ?? {},
        })),
        baseUrl: provider.baseUrl,
        codexAuthMode: normalizeCodexAuthMode(provider.codexAuthMode),
        codexLocalAuthPath: String(provider.codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim() || DEFAULT_CODEX_LOCAL_AUTH_PATH,
        apiKeys: Array.isArray(provider.apiKeys) ? provider.apiKeys.map((value) => String(value || "").trim()).filter(Boolean) : [],
        keyCursor: Math.max(0, Math.round(Number(provider.keyCursor ?? 0))),
        cachedModelOptions: Array.isArray(provider.cachedModelOptions)
          ? provider.cachedModelOptions.map((value) => String(value || "").trim()).filter(Boolean)
          : [],
        models: (provider.models || []).map((model) => ({
          id: model.id,
          model: model.model,
          enableImage: !!model.enableImage,
          enableTools: model.enableTools !== false,
          reasoningEffort: String(model.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
          temperature: Number(model.temperature ?? 1),
          customTemperatureEnabled: !!model.customTemperatureEnabled,
          contextWindowTokens: Math.round(Number(model.contextWindowTokens ?? 128000)),
          customMaxOutputTokensEnabled: !!model.customMaxOutputTokensEnabled,
          maxOutputTokens: toFiniteMaxOutputTokens(model.maxOutputTokens),
        })),
        failureRetryCount: Math.max(0, Math.round(Number(provider.failureRetryCount ?? 0))),
      })),
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: normalizeApiRequestFormat(a.requestFormat),
        allowConcurrentRequests: !!a.allowConcurrentRequests,
        enableText: !!a.enableText,
        enableImage: !!a.enableImage,
        enableAudio: !!a.enableAudio,
        enableTools: a.enableTools !== false,
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
        model: a.model,
        reasoningEffort: String(a.reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
        temperature: Number(a.temperature ?? 1),
        customTemperatureEnabled: !!a.customTemperatureEnabled,
        contextWindowTokens: Math.round(Number(a.contextWindowTokens ?? 128000)),
        customMaxOutputTokensEnabled: !!a.customMaxOutputTokensEnabled,
        maxOutputTokens: toFiniteMaxOutputTokens(a.maxOutputTokens),
      })),
    };
  }

  function buildConfigSnapshotJson(): string {
    return JSON.stringify({
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: options.config.uiFont,
      webviewZoomPercent: options.config.webviewZoomPercent,
      githubUpdateMethod: normalizeGithubUpdateMethod(options.config.githubUpdateMethod),
      recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      llmRoundLogCapacity: normalizeLlmRoundLogCapacity(options.config.llmRoundLogCapacity),
      selectedApiConfigId: options.config.selectedApiConfigId,
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      toolReviewApiConfigId: options.config.toolReviewApiConfigId,
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: !!options.config.sttAutoSend,
      terminalShellKind: String(options.config.terminalShellKind ?? ""),
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      departments: [...(options.config.departments || [])],
      mcpServers: [...(options.config.mcpServers || [])],
      remoteImChannels: [...(options.config.remoteImChannels || [])],
      apiProviders: [...(options.config.apiProviders || [])],
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: normalizeApiRequestFormat(a.requestFormat),
        enableText: a.enableText,
        enableImage: a.enableImage,
        enableAudio: a.enableAudio,
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
        model: a.model,
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
