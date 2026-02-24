import type { ComputedRef } from "vue";
import { normalizeLocale } from "../../../i18n";
import type { ApiConfigItem, AppConfig } from "../../../types/app";

function isTextRequestFormat(format: string): boolean {
  return (
    format === "openai"
    || format === "openai_responses"
    || format === "gemini"
    || format === "deepseek/kimi"
    || format === "anthropic"
  );
}

type UseConfigCoreOptions = {
  config: AppConfig;
  textCapableApiConfigs: ComputedRef<ApiConfigItem[]>;
};

export function useConfigCore(options: UseConfigCoreOptions) {
  const MIN_RECORD_SECONDS = 1;
  const MAX_MIN_RECORD_SECONDS = 30;
  const DEFAULT_MAX_RECORD_SECONDS = 60;
  const MAX_RECORD_SECONDS = 600;

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
    { id: "shell-exec", command: "builtin", args: ["shell-exec"], enabled: false, values: {} },
    {
      id: "shell-switch-workspace",
      command: "builtin",
      args: ["shell-switch-workspace"],
      enabled: false,
      values: {},
    },
    {
      id: "refresh-mcp-skills",
      command: "builtin",
      args: ["refresh-mcp-skills"],
      enabled: true,
      values: {},
    },
  ] as const;
  function normalizeUiFont(value: unknown): string {
    const text = String(value || "").trim();
    if (!text) return "auto";
    return text.length > 128 ? (text.slice(0, 128).trim() || "auto") : text;
  }

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
    options.config.uiFont = normalizeUiFont(options.config.uiFont);
    if (options.config.sttApiConfigId) {
      const sttApi = options.config.apiConfigs.find((a) => a.id === options.config.sttApiConfigId);
      if (sttApi && sttApi.requestFormat === "openai_tts") {
        sttApi.requestFormat = "openai_stt";
      }
    }
    for (const api of options.config.apiConfigs) {
      if (api.requestFormat === "gemini" && !api.enableText) {
        api.requestFormat = "gemini_embedding";
      }
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
      MIN_RECORD_SECONDS,
      Math.min(MAX_MIN_RECORD_SECONDS, Math.round(Number(options.config.minRecordSeconds) || MIN_RECORD_SECONDS)),
    );
    options.config.maxRecordSeconds = Math.max(
      options.config.minRecordSeconds,
      Math.min(MAX_RECORD_SECONDS, Math.round(Number(options.config.maxRecordSeconds) || DEFAULT_MAX_RECORD_SECONDS)),
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
        options.textCapableApiConfigs.value.find((a) => isTextRequestFormat(a.requestFormat))?.id
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
      !options.config.apiConfigs.some((a) => a.id === options.config.sttApiConfigId && a.requestFormat === "openai_stt")
    ) {
      options.config.sttApiConfigId = undefined;
    }
    if (!options.config.sttApiConfigId) {
      options.config.sttAutoSend = false;
    }
    const seenWorkspaceNames = new Set<string>();
    const normalizedWorkspaces = [];
    for (const item of options.config.shellWorkspaces || []) {
      const name = String(item?.name || "").trim();
      const path = String(item?.path || "").trim();
      if (!name || !path) continue;
      const key = name.toLowerCase();
      if (seenWorkspaceNames.has(key)) continue;
      seenWorkspaceNames.add(key);
      normalizedWorkspaces.push({
        name,
        path,
        builtIn: !!item?.builtIn,
      });
    }
    options.config.shellWorkspaces = normalizedWorkspaces;
    const seenMcpServerIds = new Set<string>();
    const normalizedMcpServers = [];
    for (const item of options.config.mcpServers || []) {
      const id = String(item?.id || "").trim();
      const definitionJson = String(item?.definitionJson || "").trim();
      if (!id || !definitionJson) continue;
      const key = id.toLowerCase();
      if (seenMcpServerIds.has(key)) continue;
      seenMcpServerIds.add(key);
      normalizedMcpServers.push({
        id,
        name: String(item?.name || "").trim() || id,
        enabled: !!item?.enabled,
        definitionJson,
        toolPolicies: Array.isArray(item?.toolPolicies)
          ? item.toolPolicies
              .map((p) => ({
                toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                enabled: !!(p as { enabled?: unknown })?.enabled,
              }))
              .filter((p) => !!p.toolName)
          : [],
        cachedTools: Array.isArray((item as { cachedTools?: unknown[] })?.cachedTools)
          ? ((item as { cachedTools?: unknown[] }).cachedTools || [])
              .map((p) => ({
                toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                description: String((p as { description?: unknown })?.description || "").trim(),
              }))
              .filter((p) => !!p.toolName)
          : [],
        lastStatus: String((item as { lastStatus?: unknown })?.lastStatus || "").trim(),
        lastError: String((item as { lastError?: unknown })?.lastError || "").trim(),
        updatedAt: String((item as { updatedAt?: unknown })?.updatedAt || "").trim(),
      });
    }
    options.config.mcpServers = normalizedMcpServers;
  }

  function buildConfigPayload(): AppConfig {
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: normalizeUiFont(options.config.uiFont),
      recordHotkey: options.config.recordHotkey,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      chatApiConfigId: options.config.chatApiConfigId,
      ...(options.config.visionApiConfigId ? { visionApiConfigId: options.config.visionApiConfigId } : {}),
      ...(options.config.sttApiConfigId ? { sttApiConfigId: options.config.sttApiConfigId } : {}),
      ...(options.config.sttAutoSend ? { sttAutoSend: true } : {}),
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
      uiFont: normalizeUiFont(options.config.uiFont),
      recordHotkey: options.config.recordHotkey,
      minRecordSeconds: options.config.minRecordSeconds,
      maxRecordSeconds: options.config.maxRecordSeconds,
      toolMaxIterations: options.config.toolMaxIterations,
      selectedApiConfigId: options.config.selectedApiConfigId,
      chatApiConfigId: options.config.chatApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId,
      sttApiConfigId: options.config.sttApiConfigId,
      sttAutoSend: !!options.config.sttAutoSend,
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      mcpServers: [...(options.config.mcpServers || [])],
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
