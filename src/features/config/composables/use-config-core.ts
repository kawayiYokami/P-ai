import type { ComputedRef } from "vue";
import { normalizeLocale } from "../../../i18n";
import type { ApiConfigItem, AppConfig } from "../../../types/app";

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  const BUILTIN_TOOL_DEFAULTS = [
    {
      id: "fetch",
      command: "npx",
      args: ["-y", "@iflow-mcp/fetch"],
      enabled: true,
      values: {},
    },
    { id: "bing-search", command: "npx", args: ["-y", "bing-cn-mcp"], enabled: true, values: {} },
    { id: "memory-save", command: "builtin", args: ["memory-save"], enabled: true, values: {} },
    { id: "desktop-screenshot", command: "builtin", args: ["desktop-screenshot"], enabled: false, values: {} },
    { id: "desktop-wait", command: "builtin", args: ["desktop-wait"], enabled: false, values: {} },
  ] as const;

  function defaultApiTools() {
    return BUILTIN_TOOL_DEFAULTS.map((tool) => ({
      id: tool.id,
      command: tool.command,
      args: [...tool.args],
      enabled: tool.enabled,
      values: { ...(tool.values as Record<string, unknown>) },
    }));
  }

  function normalizeApiToolBindings(api: ApiConfigItem) {
    const defaults = defaultApiTools();
    const current = Array.isArray(api.tools) ? api.tools : [];
    api.tools = defaults.map((tool) => {
      const found = current.find((item) => item.id === tool.id);
      return {
        id: tool.id,
        command: found?.command || tool.command,
        args: Array.isArray(found?.args) ? found!.args : tool.args,
        enabled: typeof found?.enabled === "boolean" ? found.enabled : tool.enabled,
        values: found?.values ?? tool.values,
      };
    });

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
      normalizeApiToolBindings(api);
    }
    const recordHotkey = String(options.config.recordHotkey || "").trim();
    options.config.recordHotkey = recordHotkey || "Alt";
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
      options.config.chatApiConfigId =
        options.textCapableApiConfigs.value.find((a) => a.requestFormat !== "openai_tts")?.id
        ?? options.textCapableApiConfigs.value[0]?.id
        ?? options.config.apiConfigs[0].id;
    }
    if (
      options.config.visionApiConfigId &&
      !options.config.apiConfigs.some((a) => a.id === options.config.visionApiConfigId && a.enableImage)
    ) {
      options.config.visionApiConfigId = undefined;
    }
    options.config.sttAutoSend = !!options.config.sttAutoSend;
    if (
      options.config.sttApiConfigId &&
      !options.config.apiConfigs.some((a) => a.id === options.config.sttApiConfigId && a.requestFormat === "openai_tts")
    ) {
      options.config.sttApiConfigId = undefined;
    }
    if (!options.config.sttApiConfigId) {
      options.config.sttAutoSend = false;
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
      ...(options.config.sttApiConfigId ? { sttApiConfigId: options.config.sttApiConfigId } : {}),
      ...(options.config.sttAutoSend ? { sttAutoSend: true } : {}),
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
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: !!options.config.sttAutoSend,
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
