import type { ComputedRef } from "vue";
import { normalizeLocale } from "../../../i18n";
import type { ApiConfigItem, AppConfig, DepartmentConfig } from "../../../types/app";
import { defaultToolBindings, normalizeToolBindings } from "../utils/builtin-tools";

function defaultAssistantDepartmentName(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Assistant Department";
  if (uiLanguage === "zh-TW") return "助理部門";
  return "助理部门";
}

function defaultAssistantDepartmentSummary(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Responsible for talking to the user directly, owning the main conversation, and coordinating delegation.";
  if (uiLanguage === "zh-TW") return "負責直接與使用者對話，承接主會話與統籌調度。";
  return "负责直接与用户对话，承接主会话与统筹调度。";
}

function defaultAssistantDepartmentGuide(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "You are the assistant department. Your job is to understand user needs, decide whether delegation is needed, summarize results, and continue the main conversation.";
  if (uiLanguage === "zh-TW") return "你是助理部門，負責作為主負責人理解使用者需求、決定是否需要委派、彙總結果並繼續推進主對話。";
  return "你是助理部门，负责作为主负责人理解用户需求、决定是否需要委派、汇总结果并继续推进主对话。";
}

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
  const MAX_EMPTY_REPLY_RETRY_COUNT = 20;
  function normalizeFailureRetryCount(value: unknown): number {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return 0;
    return Math.max(0, Math.min(MAX_EMPTY_REPLY_RETRY_COUNT, Math.round(parsed)));
  }

  function normalizeUiFont(value: unknown): string {
    const text = String(value || "").trim();
    if (!text) return "auto";
    return text.length > 128 ? (text.slice(0, 128).trim() || "auto") : text;
  }

  function defaultApiTools() {
    return defaultToolBindings();
  }

  function normalizeApiToolBindings(api: ApiConfigItem) {
    api.tools = normalizeToolBindings(api.tools);
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
      failureRetryCount: 0,
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
      api.failureRetryCount = Math.max(
        0,
        normalizeFailureRetryCount(api.failureRetryCount),
      );
      normalizeApiToolBindings(api);
    }
    const recordHotkey = String(options.config.recordHotkey || "").trim();
    options.config.recordHotkey = recordHotkey || "Alt";
    options.config.recordBackgroundWakeEnabled = !!options.config.recordBackgroundWakeEnabled;
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
    if (!options.config.apiConfigs.some((a) => a.id === options.config.assistantDepartmentApiConfigId && a.enableText)) {
      options.config.assistantDepartmentApiConfigId =
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
    const validTextChatApiIds = new Set(
      options.config.apiConfigs
        .filter((a) => !!a.enableText && isTextRequestFormat(a.requestFormat))
        .map((a) => a.id),
    );
    const normalizedDepartments: DepartmentConfig[] = [];
    const seenDepartmentIds = new Set<string>();
    const defaultAssistantDepartmentApiId =
      options.config.assistantDepartmentApiConfigId
      || options.textCapableApiConfigs.value.find((a) => isTextRequestFormat(a.requestFormat))?.id
      || options.textCapableApiConfigs.value[0]?.id
      || options.config.apiConfigs[0]?.id
      || "";
    const assistantName = defaultAssistantDepartmentName(options.config.uiLanguage);
    const assistantSummary = defaultAssistantDepartmentSummary(options.config.uiLanguage);
    const assistantGuide = defaultAssistantDepartmentGuide(options.config.uiLanguage);
    const defaultAssistantDepartment: DepartmentConfig = {
      id: "assistant-department",
      name: assistantName,
      summary: assistantSummary,
      guide: assistantGuide,
      apiConfigId: defaultAssistantDepartmentApiId,
      agentIds: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      orderIndex: 1,
      isBuiltInAssistant: true,
      source: "main_config",
      scope: "global",
    };
    for (const item of options.config.departments || []) {
      const id = String(item?.id || "").trim();
      if (!id) continue;
      const key = id.toLowerCase();
      if (seenDepartmentIds.has(key)) continue;
      seenDepartmentIds.add(key);
      const agentIds = Array.isArray(item?.agentIds)
        ? Array.from(new Set(item.agentIds.map((v) => String(v || "").trim()).filter(Boolean)))
        : [];
      normalizedDepartments.push({
        id,
        name: String(item?.name || "").trim() || `部门 ${normalizedDepartments.length + 1}`,
        summary: String(item?.summary || "").trim(),
        guide: String(item?.guide || "").trim(),
        apiConfigId: validTextChatApiIds.has(String(item?.apiConfigId || "").trim())
          ? String(item?.apiConfigId || "").trim()
          : defaultAssistantDepartmentApiId,
        agentIds,
        createdAt: String(item?.createdAt || "").trim() || new Date().toISOString(),
        updatedAt: String(item?.updatedAt || "").trim() || new Date().toISOString(),
        orderIndex: Math.max(1, Number(item?.orderIndex || normalizedDepartments.length + 1)),
        isBuiltInAssistant: !!item?.isBuiltInAssistant || id === "assistant-department",
        source: String(item?.source || "").trim() || "main_config",
        scope: String(item?.scope || "").trim() || "global",
      });
    }
    if (!normalizedDepartments.some((item) => item.id === "assistant-department" || item.isBuiltInAssistant)) {
      normalizedDepartments.unshift(defaultAssistantDepartment);
    }
    normalizedDepartments.sort((a, b) => {
      const aRank = a.isBuiltInAssistant || a.id === "assistant-department" ? 0 : 1;
      const bRank = b.isBuiltInAssistant || b.id === "assistant-department" ? 0 : 1;
      return aRank - bRank || a.orderIndex - b.orderIndex;
    });
    options.config.departments = normalizedDepartments.map((item, idx) => ({
      ...item,
      id: item.isBuiltInAssistant || item.id === "assistant-department" ? "assistant-department" : item.id,
      name: String(item.name || "").trim() || (item.isBuiltInAssistant || item.id === "assistant-department" ? assistantName : `部门 ${idx + 1}`),
      apiConfigId: validTextChatApiIds.has(item.apiConfigId) ? item.apiConfigId : defaultAssistantDepartmentApiId,
      orderIndex: idx + 1,
      isBuiltInAssistant: item.isBuiltInAssistant || item.id === "assistant-department",
      source: item.source || "main_config",
      scope: item.scope || "global",
    }));
    const assistantDept = options.config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
    if (assistantDept) {
      options.config.assistantDepartmentApiConfigId = assistantDept.apiConfigId || defaultAssistantDepartmentApiId;
    }
  }

  function buildConfigPayload(): AppConfig {
    return {
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: normalizeUiFont(options.config.uiFont),
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
        failureRetryCount: normalizeFailureRetryCount(a.failureRetryCount),
      })),
    };
  }

  function buildConfigSnapshotJson(): string {
    return JSON.stringify({
      hotkey: options.config.hotkey,
      uiLanguage: options.config.uiLanguage,
      uiFont: normalizeUiFont(options.config.uiFont),
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
      shellWorkspaces: [...(options.config.shellWorkspaces || [])],
      departments: [...(options.config.departments || [])],
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
        failureRetryCount: normalizeFailureRetryCount(a.failureRetryCount),
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

