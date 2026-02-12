import type { ComputedRef } from "vue";
import { normalizeLocale } from "../../../i18n";
import type { ApiConfigItem, AppConfig } from "../../../types/app";

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  function defaultApiTools() {
    return [
      {
        id: "fetch",
        command: "npx",
        args: ["-y", "@iflow-mcp/fetch"],
        values: {},
      },
      { id: "bing-search", command: "npx", args: ["-y", "bing-cn-mcp"], values: {} },
      { id: "memory-save", command: "builtin", args: ["memory-save"], values: {} },
    ];
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
      contextWindowTokens: 128000,
    };
  }

  function normalizeApiBindingsLocal() {
    if (!options.config.apiConfigs.length) return;
    options.config.uiLanguage = normalizeLocale(options.config.uiLanguage);
    for (const api of options.config.apiConfigs) {
      api.enableAudio = false;
      api.temperature = Math.max(0, Math.min(2, Number(api.temperature ?? 1)));
      api.contextWindowTokens = Math.max(
        16000,
        Math.min(200000, Math.round(Number(api.contextWindowTokens ?? 128000))),
      );
    }
    if (!["Alt", "Ctrl", "Shift"].includes(options.config.recordHotkey)) {
      options.config.recordHotkey = "Alt";
    }
    options.config.minRecordSeconds = Math.max(
      1,
      Math.min(30, Math.round(Number(options.config.minRecordSeconds) || 1)),
    );
    options.config.maxRecordSeconds = Math.max(
      options.config.minRecordSeconds,
      Math.round(Number(options.config.maxRecordSeconds) || 60),
    );
    options.config.toolMaxIterations = Math.max(
      1,
      Math.min(100, Math.round(Number(options.config.toolMaxIterations) || 10)),
    );
    if (!options.config.apiConfigs.some((a) => a.id === options.config.selectedApiConfigId)) {
      options.config.selectedApiConfigId = options.config.apiConfigs[0].id;
    }
    if (!options.config.apiConfigs.some((a) => a.id === options.config.chatApiConfigId && a.enableText)) {
      options.config.chatApiConfigId = options.textCapableApiConfigs.value[0]?.id ?? options.config.apiConfigs[0].id;
    }
    if (
      options.config.visionApiConfigId &&
      !options.config.apiConfigs.some((a) => a.id === options.config.visionApiConfigId && a.enableImage)
    ) {
      options.config.visionApiConfigId = undefined;
    }
  }

  function buildConfigPayload(): AppConfig {
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      recordHotkey: options.config.recordHotkey,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      chatApiConfigId: options.config.chatApiConfigId,
      ...(options.config.visionApiConfigId ? { visionApiConfigId: options.config.visionApiConfigId } : {}),
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
          values: t.values ?? {},
        })),
        baseUrl: a.baseUrl,
        apiKey: a.apiKey,
        model: a.model,
        temperature: Number(a.temperature ?? 1),
        contextWindowTokens: Math.round(Number(a.contextWindowTokens ?? 128000)),
      })),
    };
  }

  function buildConfigSnapshotJson(): string {
    return JSON.stringify({
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      recordHotkey: options.config.recordHotkey,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      chatApiConfigId: options.config.chatApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      apiConfigs: options.config.apiConfigs.map((a) => ({
        id: a.id,
        name: a.name,
        requestFormat: a.requestFormat,
        enableText: a.enableText,
        enableImage: a.enableImage,
        enableAudio: a.enableAudio,
        enableTools: a.enableTools,
        tools: a.tools,
        baseUrl: a.baseUrl,
        apiKey: a.apiKey,
        model: a.model,
        temperature: a.temperature,
        contextWindowTokens: a.contextWindowTokens,
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


