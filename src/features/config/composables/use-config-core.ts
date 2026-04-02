import type { ComputedRef } from "vue";
import type { ApiConfigItem, AppConfig, RemoteImChannelConfig, RemoteImPlatform } from "../../../types/app";
import { defaultToolBindings } from "../utils/builtin-tools";

function normalizeRemoteImPlatform(value: unknown): RemoteImPlatform {
  const text = String(value || "").trim().toLowerCase();
  if (text === "feishu" || text === "dingtalk" || text === "onebot_v11" || text === "weixin_oc") {
    return text as RemoteImPlatform;
  }
  return "onebot_v11";
}

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  const DEFAULT_MAX_OUTPUT_TOKENS = 4096;

  function toFiniteMaxOutputTokens(value: unknown): number {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : DEFAULT_MAX_OUTPUT_TOKENS;
  }

  function defaultApiTools() {
    return defaultToolBindings();
  }

  function createApiConfig(seed = Date.now().toString()): ApiConfigItem {
    return {
      id: `api-config-${seed}`,
      name: `API Config ${options.config.apiConfigs.length + 1}`,
      requestFormat: "openai",
      enableText: true,
      enableImage: false,
      enableAudio: false,
      enableTools: true,
      tools: defaultApiTools(),
      baseUrl: "https://api.openai.com/v1",
      apiKey: "",
      model: "gpt-4o-mini",
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: 128000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
    };
  }

  function normalizeApiBindingsLocal() {
    if (!options.config.apiConfigs.length) return;
  }

  function buildConfigPayload(): AppConfig {
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: options.config.uiFont,
      recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      ...(options.config.visionApiConfigId ? { visionApiConfigId: options.config.visionApiConfigId } : {}),
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
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: a.requestFormat,
        enableText: !!a.enableText,
        enableImage: !!a.enableImage,
        enableAudio: !!a.enableAudio,
        enableTools: !!a.enableTools,
        tools: (a.tools || []).map((t) => ({
          id: t.id,
          command: t.command,
          args: Array.isArray(t.args) ? t.args : [],
          enabled: typeof t.enabled === "boolean" ? t.enabled : true,
          values: t.values ?? {},
        })),
        baseUrl: a.baseUrl,
        apiKey: a.apiKey,
        model: a.model,
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
      recordHotkey: options.config.recordHotkey,
      recordBackgroundWakeEnabled: !!options.config.recordBackgroundWakeEnabled,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: !!options.config.sttAutoSend,
      terminalShellKind: String(options.config.terminalShellKind ?? ""),
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      departments: [...(options.config.departments || [])],
      mcpServers: [...(options.config.mcpServers || [])],
      remoteImChannels: [...(options.config.remoteImChannels || [])],
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: a.requestFormat,
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
        model: a.model,
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
    createApiConfig,
    normalizeApiBindingsLocal,
    buildConfigPayload,
    buildConfigSnapshotJson,
  };
}
