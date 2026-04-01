import type { ComputedRef } from "vue";
import { normalizeLocale } from "../../../i18n";
import type { ApiConfigItem, AppConfig, DepartmentConfig, RemoteImChannelConfig, RemoteImPlatform } from "../../../types/app";
import { defaultToolBindings, normalizeToolBindings } from "../utils/builtin-tools";

const ASSISTANT_DEPARTMENT_ID = "assistant-department";
const DEPUTY_DEPARTMENT_ID = "deputy-department";
const FRONT_DESK_DEPARTMENT_ID = "front-desk-department";

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

function defaultDeputyDepartmentName(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Deputy";
  if (uiLanguage === "zh-TW") return "副手";
  return "副手";
}

function defaultDeputyDepartmentSummary(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Executes explicit tasks from the main department with minimal action and strict scope.";
  if (uiLanguage === "zh-TW") return "負責快速執行上級派發的明確任務，強調最小行動與嚴格邊界。";
  return "负责快速执行上级派发的明确任务，强调最小行动与严格边界。";
}

function defaultDeputyDepartmentGuide(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "You are the deputy department. Stay strictly within scope, do not overreach, and do not expand requirements on your own. Use the fewest tool calls and finish explicit tasks quickly. If information is missing or the task goes beyond the assigned boundary, state that directly and wait for the main department to decide.";
  if (uiLanguage === "zh-TW") return "你是副手部門。核心原則是嚴格不越權、不自行擴展需求、不多想。收到上級派發的任務後，用最少的工具調用、最快的速度完成明確目標；若資訊不足或任務超出指令邊界，就直接說明缺口並等待主部門繼續決策。";
  return "你是副手部门。核心原则是严格不越权、不擅自扩展需求、不多想。收到上级派发的任务后，用最少的工具调用、最快的速度完成明确目标；若信息不足或任务超出指令边界，就直接说明缺口并等待主部门继续决策。";
}

function defaultFrontDeskDepartmentName(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Front Desk";
  if (uiLanguage === "zh-TW") return "前台";
  return "前台";
}

function defaultFrontDeskDepartmentSummary(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "Handles remote IM conversations with short, friendly replies and hands complex work back to the main department.";
  if (uiLanguage === "zh-TW") return "負責承接遠端 IM 訊息，簡短友好應答，並把複雜任務轉交主部門。";
  return "负责承接远程 IM 消息，简短友好应答，并把复杂任务转交主部门。";
}

function defaultFrontDeskDepartmentGuide(uiLanguage: string): string {
  if (uiLanguage === "en-US") return "You are the front desk department for remote IM contacts. Replies must be short, friendly, and patient. Answer simple questions directly. If a request is complex, multi-step, requires deeper analysis, or cannot be handled safely, clearly say it will be handed to the main department instead of expanding on it yourself.";
  if (uiLanguage === "zh-TW") return "你是前台部門，專門負責承接各個遠端 IM 聯絡人的訊息。說話必須簡短、友好、有耐心，優先直接回答簡單問題；遇到複雜任務、涉及多步分析、需要明顯調度或你無法穩妥處理的需求時，應明確告知將轉交主部門處理，不要自己展開複雜推理。";
  return "你是前台部门，专门负责承接各个远程 IM 联系人的消息。说话必须简短、友好、有耐心，优先直接回答简单问题；遇到复杂任务、涉及多步骤分析、需要明显调度或你无法稳妥处理的需求时，应明确告知将转交主部门处理，不要自己展开复杂推理。";
}

function builtInDepartmentRank(id: string): number {
  if (id === ASSISTANT_DEPARTMENT_ID) return 0;
  if (id === DEPUTY_DEPARTMENT_ID) return 1;
  if (id === FRONT_DESK_DEPARTMENT_ID) return 2;
  return 3;
}

function isTextRequestFormat(format: string): boolean {
  return (
    format === "openai"
    || format === "openai_responses"
    || format === "gemini"
    || format === "anthropic"
  );
}

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
  const CONTEXT_WINDOW_HARD_MAX = 2_000_000;
  const MIN_RECORD_SECONDS = 1;
  const MAX_MIN_RECORD_SECONDS = 30;
  const DEFAULT_MAX_RECORD_SECONDS = 60;
  const MAX_RECORD_SECONDS = 600;
  function normalizeMaxOutputTokens(value: unknown): number {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) return 4096;
    return Math.max(256, Math.min(32768, Math.round(parsed)));
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
      customTemperatureEnabled: false,
      contextWindowTokens: 128000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
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
      api.customTemperatureEnabled = !!api.customTemperatureEnabled;
      api.contextWindowTokens = Math.max(
        16000,
        Math.min(CONTEXT_WINDOW_HARD_MAX, Math.round(Number(api.contextWindowTokens ?? 128000))),
      );
      api.customMaxOutputTokensEnabled = api.requestFormat === "anthropic"
        ? true
        : !!api.customMaxOutputTokensEnabled;
      api.maxOutputTokens = normalizeMaxOutputTokens(api.maxOutputTokens);
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
    // 移除自动清空 sttApiConfigId 和 sttAutoSend 的逻辑
    // 用户的选择应该被尊重，即使 API 格式不匹配也不应该自动修改
    // if (
    //   options.config.sttApiConfigId &&
    //   !options.config.apiConfigs.some((a) => a.id === options.config.sttApiConfigId && a.requestFormat === "openai_stt")
    // ) {
    //   options.config.sttApiConfigId = undefined;
    // }
    // if (!options.config.sttApiConfigId) {
    //   options.config.sttAutoSend = false;
    // }
    const seenWorkspaceNames = new Set<string>();
    const terminalShellKind = String(options.config.terminalShellKind || "").trim().toLowerCase();
    options.config.terminalShellKind =
      terminalShellKind === "auto"
      || terminalShellKind === "powershell7"
      || terminalShellKind === "powershell5"
      || terminalShellKind === "git-bash"
      || terminalShellKind === "zsh"
      || terminalShellKind === "bash"
      || terminalShellKind === "sh"
        ? terminalShellKind
        : "auto";
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
    const seenRemoteChannelIds = new Set<string>();
    const normalizedRemoteChannels = [];
    for (const item of options.config.remoteImChannels || []) {
      const id = String(item?.id || "").trim();
      if (!id) continue;
      const key = id.toLowerCase();
      if (seenRemoteChannelIds.has(key)) continue;
      seenRemoteChannelIds.add(key);
      normalizedRemoteChannels.push({
        id,
        name: String(item?.name || "").trim() || id,
        platform: normalizeRemoteImPlatform(item?.platform),
        enabled: !!item?.enabled,
        credentials: item?.credentials && typeof item.credentials === "object" ? { ...item.credentials } : {},
        activateAssistant: item?.activateAssistant !== false,
        receiveFiles: item?.receiveFiles !== false,
        streamingSend: !!item?.streamingSend,
        showToolCalls: !!item?.showToolCalls,
        allowSendFiles: !!item?.allowSendFiles,
      });
    }
    options.config.remoteImChannels = normalizedRemoteChannels;

    // Only normalize departments if they don't exist yet (initial load)
    if (!options.config.departments || options.config.departments.length === 0) {
      normalizeDepartments();
    }
  }

  function normalizeDepartments() {
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
      id: ASSISTANT_DEPARTMENT_ID,
      name: assistantName,
      summary: assistantSummary,
      guide: assistantGuide,
      apiConfigId: defaultAssistantDepartmentApiId,
      apiConfigIds: defaultAssistantDepartmentApiId ? [defaultAssistantDepartmentApiId] : [],
      agentIds: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      orderIndex: 1,
      isBuiltInAssistant: true,
      source: "main_config",
      scope: "global",
    };
    const defaultDeputyDepartment: DepartmentConfig = {
      id: DEPUTY_DEPARTMENT_ID,
      name: defaultDeputyDepartmentName(options.config.uiLanguage),
      summary: defaultDeputyDepartmentSummary(options.config.uiLanguage),
      guide: defaultDeputyDepartmentGuide(options.config.uiLanguage),
      apiConfigId: defaultAssistantDepartmentApiId,
      apiConfigIds: defaultAssistantDepartmentApiId ? [defaultAssistantDepartmentApiId] : [],
      agentIds: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      orderIndex: 2,
      isBuiltInAssistant: false,
      source: "main_config",
      scope: "global",
    };
    const defaultFrontDeskDepartment: DepartmentConfig = {
      id: FRONT_DESK_DEPARTMENT_ID,
      name: defaultFrontDeskDepartmentName(options.config.uiLanguage),
      summary: defaultFrontDeskDepartmentSummary(options.config.uiLanguage),
      guide: defaultFrontDeskDepartmentGuide(options.config.uiLanguage),
      apiConfigId: defaultAssistantDepartmentApiId,
      apiConfigIds: defaultAssistantDepartmentApiId ? [defaultAssistantDepartmentApiId] : [],
      agentIds: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      orderIndex: 3,
      isBuiltInAssistant: false,
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
      const rawApiConfigIds = Array.isArray(item?.apiConfigIds)
        ? item.apiConfigIds.map((v) => String(v || "").trim()).filter(Boolean)
        : [];
      const legacyApiConfigId = String(item?.apiConfigId || "").trim();
      const apiConfigIds = Array.from(new Set(
        (rawApiConfigIds.length > 0 ? rawApiConfigIds : [legacyApiConfigId])
          .filter((id) => validTextChatApiIds.has(id)),
      ));
      normalizedDepartments.push({
        id,
        name: String(item?.name || "").trim() || `部门 ${normalizedDepartments.length + 1}`,
        summary: String(item?.summary || "").trim(),
        guide: String(item?.guide || "").trim(),
        apiConfigId: apiConfigIds[0] || defaultAssistantDepartmentApiId,
        apiConfigIds: apiConfigIds.length > 0 ? apiConfigIds : (defaultAssistantDepartmentApiId ? [defaultAssistantDepartmentApiId] : []),
        agentIds,
        createdAt: String(item?.createdAt || "").trim() || new Date().toISOString(),
        updatedAt: String(item?.updatedAt || "").trim() || new Date().toISOString(),
        orderIndex: Math.max(1, Number(item?.orderIndex || normalizedDepartments.length + 1)),
        isBuiltInAssistant: !!item?.isBuiltInAssistant || id === ASSISTANT_DEPARTMENT_ID,
        source: String(item?.source || "").trim() || "main_config",
        scope: String(item?.scope || "").trim() || "global",
      });
    }
    if (!normalizedDepartments.some((item) => item.id === ASSISTANT_DEPARTMENT_ID || item.isBuiltInAssistant)) {
      normalizedDepartments.push(defaultAssistantDepartment);
    }
    if (!normalizedDepartments.some((item) => item.id === DEPUTY_DEPARTMENT_ID)) {
      normalizedDepartments.push(defaultDeputyDepartment);
    }
    if (!normalizedDepartments.some((item) => item.id === FRONT_DESK_DEPARTMENT_ID)) {
      normalizedDepartments.push(defaultFrontDeskDepartment);
    }
    normalizedDepartments.sort((a, b) => {
      return builtInDepartmentRank(a.id) - builtInDepartmentRank(b.id) || a.orderIndex - b.orderIndex;
    });
    const finalDepartments = normalizedDepartments.map((item, idx) => ({
      ...item,
      id: item.isBuiltInAssistant || item.id === ASSISTANT_DEPARTMENT_ID ? ASSISTANT_DEPARTMENT_ID : item.id,
      name: String(item.name || "").trim()
        || (item.id === ASSISTANT_DEPARTMENT_ID || item.isBuiltInAssistant
          ? assistantName
          : item.id === DEPUTY_DEPARTMENT_ID
            ? defaultDeputyDepartmentName(options.config.uiLanguage)
            : item.id === FRONT_DESK_DEPARTMENT_ID
              ? defaultFrontDeskDepartmentName(options.config.uiLanguage)
              : `部门 ${idx + 1}`),
      apiConfigIds: Array.from(new Set(
        (Array.isArray(item.apiConfigIds) ? item.apiConfigIds : [])
          .map((id) => String(id || "").trim())
          .filter((id) => validTextChatApiIds.has(id)),
      )),
      apiConfigId: "",
      orderIndex: idx + 1,
      isBuiltInAssistant: item.isBuiltInAssistant || item.id === ASSISTANT_DEPARTMENT_ID,
      source: item.source || "main_config",
      scope: item.scope || "global",
    })).map((item) => ({
      ...item,
      apiConfigIds: item.apiConfigIds.length > 0 ? item.apiConfigIds : (defaultAssistantDepartmentApiId ? [defaultAssistantDepartmentApiId] : []),
      apiConfigId: item.apiConfigIds[0] || defaultAssistantDepartmentApiId,
    }));

    options.config.departments = finalDepartments;
    const assistantDept = options.config.departments.find((item) => item.id === ASSISTANT_DEPARTMENT_ID || item.isBuiltInAssistant);
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
      terminalShellKind: String(options.config.terminalShellKind || "auto"),
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
        customMaxOutputTokensEnabled: a.requestFormat === "anthropic"
          ? true
          : !!a.customMaxOutputTokensEnabled,
        maxOutputTokens: normalizeMaxOutputTokens(a.maxOutputTokens),
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
      terminalShellKind: String(options.config.terminalShellKind || "auto"),
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
        customMaxOutputTokensEnabled: a.requestFormat === "anthropic"
          ? true
          : !!a.customMaxOutputTokensEnabled,
        maxOutputTokens: normalizeMaxOutputTokens(a.maxOutputTokens),
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
