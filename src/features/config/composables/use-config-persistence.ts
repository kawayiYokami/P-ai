import type { ComputedRef, Ref } from "vue";
import {
  invokeTauri,
  updateTransportRecordBackgroundWake,
  updateTransportRecordHotkey,
} from "../../../services/tauri-api";
import type {
  AppBootstrapSnapshot,
  AppConfig,
  ChatSettings,
  ChatSettingsPatch,
  RecordHotkeyUpdateResult,
  CodexAuthMode,
  ConfigRepairNotice,
  ConversationApiSettings,
  ConversationApiSettingsPatch,
  PdfReadMode,
  PersonaProfile,
  PromptCommandPreset,
  RemoteImChannelConfig,
} from "../../../types/app";import type { SupportedLocale } from "../../../i18n";
import { normalizeApiRequestFormat } from "../utils/api-request-format";
import {
  normalizeImageGenerationModelId,
  normalizeImageGenerationProviders,
} from "../utils/image-generation-config";
import { applyUiSizeScale, normalizeUiSizeScale } from "../../shell/composables/use-ui-size-appearance";

const DEFAULT_CODEX_ORIGINATOR = "codex-tui";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

export type ConfigSaveErrorKind = "hotkey_conflict" | "backend_404" | "unknown";

export type ConfigSaveErrorInfo = {
  kind: ConfigSaveErrorKind;
  errorText: string;
  hotkey: string;
};

/** 把后端上报的自修复记录描述成一句可读提示；没有修复时返回空串。 */
function describeConfigRepairs(
  repairs: ConfigRepairNotice[],
  t: TrFn,
  personas: PersonaProfile[],
): string {
  if (!repairs.length) return "";
  const agentName = (agentId: string) => {
    const target = String(agentId || "").trim();
    const matched = personas.find((persona) => String(persona.id || "").trim() === target);
    return String(matched?.name || target).trim();
  };
  const details = repairs
    .map((repair) => t("status.configRepairAgent", {
      persona: agentName(repair.agentId),
    }))
    .join("；");
  return t("status.configSavedWithRepairs", { details });
}

type UseConfigPersistenceOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  onSaveConfigError?: (info: ConfigSaveErrorInfo) => void;
  config: AppConfig;
  locale: { value: string };
  normalizeLocale: (value: string) => SupportedLocale;
  suppressAutosave: Ref<boolean>;
  loading: Ref<boolean>;
  saving: Ref<boolean>;
  savingPersonas: Ref<boolean>;
  personas: Ref<PersonaProfile[]>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  assistantAgentId: Ref<string>;
  personaEditorId: Ref<string>;
  userAlias: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  selectedPdfReadMode: Ref<PdfReadMode>;
  backgroundVoiceScreenshotKeywords: Ref<string>;
  backgroundVoiceScreenshotMode: Ref<"desktop" | "focused_window">;
  instructionPresets: Ref<PromptCommandPreset[]>;
  responseStyleIds: ComputedRef<string[]>;
  createApiConfig: (name?: string) => AppConfig["apiConfigs"][number];
  normalizeApiBindingsLocal: () => void;
  buildConfigPayload: () => AppConfig;
  buildConfigSnapshotJson: () => string;
  lastSavedConfigJson: Ref<string>;
  buildPersonasSnapshotJson: () => string;
  lastSavedPersonasJson: Ref<string>;
  syncUserAliasFromPersona: () => void;
  preloadPersonaAvatars: () => Promise<void>;
  syncTrayIcon: (agentId?: string) => Promise<void>;
  perfNow?: () => number;
  perfLog?: (label: string, startedAt: number) => void;
};

function normalizeLlmRoundLogCapacity(value: unknown): 1 | 3 | 10 {
  const numeric = Math.round(Number(value));
  if (numeric === 1 || numeric === 3 || numeric === 10) return numeric;
  if (!Number.isFinite(numeric) || numeric <= 0) return 3;
  if (numeric < 3) return 1;
  if (numeric < 10) return 3;
  return 10;
}

function normalizeWebAccessPort(value: unknown): number {
  const parsed = Math.round(Number(value));
  if (Number.isFinite(parsed) && parsed >= 1024 && parsed <= 65535) {
    return parsed;
  }
  return 8429;
}

function normalizeGithubUpdateMethod(value: unknown): AppConfig["githubUpdateMethod"] {
  const text = String(value || "").trim();
  return text === "direct" || text === "proxy" ? text : "auto";
}

function normalizeSkippedGithubUpdateVersion(value: unknown): string {
  return String(value || "").trim();
}

function mapRemoteImChannel(item: unknown): RemoteImChannelConfig {
  const platformRaw = String((item as { platform?: unknown })?.platform || "").trim().toLowerCase();
  const platform =
    platformRaw === "feishu" || platformRaw === "dingtalk" || platformRaw === "onebot_v11" || platformRaw === "weixin_oc"
      ? platformRaw
      : "onebot_v11";
  return {
    id: String((item as { id?: unknown })?.id || "").trim(),
    name: String((item as { name?: unknown })?.name || "").trim(),
    platform,
    enabled: !!(item as { enabled?: unknown })?.enabled,
    credentials:
      (item as { credentials?: unknown })?.credentials
      && typeof (item as { credentials?: unknown }).credentials === "object"
        ? { ...((item as { credentials?: Record<string, unknown> }).credentials || {}) }
        : {},
    receiveFiles: (item as { receiveFiles?: unknown })?.receiveFiles !== false,
    streamingSend: !!(item as { streamingSend?: unknown })?.streamingSend,
    showToolCalls: !!(item as { showToolCalls?: unknown })?.showToolCalls,
    filterMarkdown: !!(item as { filterMarkdown?: unknown })?.filterMarkdown,
    allowSendFiles: !!(item as { allowSendFiles?: unknown })?.allowSendFiles,
    behaviorSettings: (item as { behaviorSettings?: unknown })?.behaviorSettings
      && typeof (item as { behaviorSettings?: unknown }).behaviorSettings === "object"
      ? { ...((item as { behaviorSettings?: Record<string, unknown> }).behaviorSettings || {}) } as RemoteImChannelConfig["behaviorSettings"]
      : undefined,
  };
}

export function useConfigPersistence(options: UseConfigPersistenceOptions) {
  let lastConversationApiSettingsJson = "";
  let conversationApiSettingsSaving = false;
  let lastChatSettingsJson = "";
  let chatSettingsSaving = false;
  const MIN_RECORD_SECONDS = 1;
  const MAX_MIN_RECORD_SECONDS = 30;
  const DEFAULT_MAX_RECORD_SECONDS = 60;
  const MAX_RECORD_SECONDS = 600;
  const DEFAULT_CODEX_AUTH_MODE = "read_local";
  const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
  const DEFAULT_REASONING_EFFORT = "medium";

  function normalizeCodexAuthMode(value: unknown): CodexAuthMode {
    const normalized = String(value || "").trim();
    if (normalized === "managed_oauth") return "managed_oauth";
    if (normalized === "custom_url") return "custom_url";
    return "read_local";
  }

  function extractHttpStatus(error: unknown): number | null {
    if (!error || typeof error !== "object") return null;
    const err = error as Record<string, unknown>;
    const directStatus = err.status;
    if (typeof directStatus === "number") return directStatus;
    if (typeof directStatus === "string") {
      const parsed = Number(directStatus);
      if (Number.isFinite(parsed)) return parsed;
    }
    const response = err.response;
    if (!response || typeof response !== "object") return null;
    const responseStatus = (response as Record<string, unknown>).status;
    if (typeof responseStatus === "number") return responseStatus;
    if (typeof responseStatus === "string") {
      const parsed = Number(responseStatus);
      if (Number.isFinite(parsed)) return parsed;
    }
    return null;
  }

  function normalizeConfigNumberFields(
    minValue: unknown,
    maxValue: unknown,
    fallback?: {
      minRecordSeconds?: number;
      maxRecordSeconds?: number;
    },
  ): { minRecordSeconds: number; maxRecordSeconds: number } {
    const fallbackMin = Number(fallback?.minRecordSeconds);
    const fallbackMax = Number(fallback?.maxRecordSeconds);
    const minSeed = Number(minValue);
    const maxSeed = Number(maxValue);
    const resolvedMin = Number.isFinite(minSeed)
      ? minSeed
      : (Number.isFinite(fallbackMin) ? fallbackMin : MIN_RECORD_SECONDS);
    const minRecordSeconds = Math.max(
      MIN_RECORD_SECONDS,
      Math.min(MAX_MIN_RECORD_SECONDS, Math.round(resolvedMin)),
    );
    const resolvedMax = Number.isFinite(maxSeed)
      ? maxSeed
      : (Number.isFinite(fallbackMax) ? fallbackMax : DEFAULT_MAX_RECORD_SECONDS);
    const maxRecordSeconds = Math.max(
      minRecordSeconds,
      Math.min(MAX_RECORD_SECONDS, Math.round(resolvedMax)),
    );
    return { minRecordSeconds, maxRecordSeconds };
  }

  function classifySaveConfigError(error: unknown): ConfigSaveErrorInfo {
    const errorText = String(error ?? "unknown");
    const normalized = errorText.toLowerCase();
    const status = extractHttpStatus(error);
    const isBackend404 = status === 404 || normalized.includes("404");
    const isHotkeyConflict =
      (normalized.includes("register hotkey failed")
        && (normalized.includes("already registered") || normalized.includes("already in use")))
      || normalized.includes("hotkey already registered");

    if (isBackend404) {
      return {
        kind: "backend_404",
        errorText,
        hotkey: options.config.hotkey,
      };
    }
    if (isHotkeyConflict) {
      return {
        kind: "hotkey_conflict",
        errorText,
        hotkey: options.config.hotkey,
      };
    }
    return {
      kind: "unknown",
      errorText,
      hotkey: options.config.hotkey,
    };
  }

  function applyLoadedConfig(cfg: AppConfig) {
    options.config.hotkey = cfg.hotkey;
    options.config.uiLanguage = options.normalizeLocale(cfg.uiLanguage);
    options.config.uiFont = String((cfg as { uiFont?: unknown }).uiFont ?? "");
    options.config.codeFont = String((cfg as { codeFont?: unknown }).codeFont ?? "");
    options.config.uiSizeScale = normalizeUiSizeScale((cfg as { uiSizeScale?: unknown }).uiSizeScale);
    applyUiSizeScale(options.config.uiSizeScale);
    options.config.webAccessPort = normalizeWebAccessPort((cfg as { webAccessPort?: unknown }).webAccessPort);
    options.config.webAccessEnabled = (cfg as { webAccessEnabled?: unknown }).webAccessEnabled !== false;
    options.config.webAccessPassword = String((cfg as { webAccessPassword?: unknown }).webAccessPassword || "").trim();
    options.config.githubUpdateMethod = normalizeGithubUpdateMethod((cfg as { githubUpdateMethod?: unknown }).githubUpdateMethod);
    options.config.skippedGithubUpdateVersion = normalizeSkippedGithubUpdateVersion(
      (cfg as { skippedGithubUpdateVersion?: unknown }).skippedGithubUpdateVersion,
    );
    options.locale.value = options.config.uiLanguage;
    options.config.recordHotkey = String(cfg.recordHotkey ?? "");
    options.config.recordBackgroundWakeEnabled = !!cfg.recordBackgroundWakeEnabled;
    const normalizedConfigNumbers = normalizeConfigNumberFields(
      cfg.minRecordSeconds,
      cfg.maxRecordSeconds,
    );
    options.config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
    options.config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
    options.config.llmRoundLogCapacity = normalizeLlmRoundLogCapacity((cfg as AppConfig).llmRoundLogCapacity);
    options.config.messageNotificationEnabled = (cfg as { messageNotificationEnabled?: unknown }).messageNotificationEnabled !== false;
    options.config.messageNotificationSoundEnabled = (cfg as { messageNotificationSoundEnabled?: unknown }).messageNotificationSoundEnabled === true;
    options.config.desktopOperationNoticeEnabled = (cfg as { desktopOperationNoticeEnabled?: unknown }).desktopOperationNoticeEnabled !== false;
    options.config.desktopOperateEnabled = (cfg as { desktopOperateEnabled?: unknown }).desktopOperateEnabled !== false;
    options.config.selectedApiConfigId = cfg.selectedApiConfigId;
    options.config.expertApiConfigId = cfg.expertApiConfigId;
    options.config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
    options.config.imageProviders = normalizeImageGenerationProviders(
      (cfg as Partial<AppConfig>).imageProviders,
    );
    options.config.imageGenerationModelId = normalizeImageGenerationModelId(
      (cfg as Partial<AppConfig>).imageGenerationModelId,
      options.config.imageProviders,
    );
    options.config.toolReviewApiConfigId = cfg.toolReviewApiConfigId ?? undefined;
    options.config.sttApiConfigId = cfg.sttApiConfigId ?? undefined;
    options.config.sttAutoSend = !!cfg.sttAutoSend;
    options.config.terminalShellKind = String((cfg as AppConfig).terminalShellKind ?? "");
    options.config.simpleSetupMode = (cfg as { simpleSetupMode?: unknown }).simpleSetupMode !== false;
    options.config.shellWorkspaces = Array.isArray(cfg.shellWorkspaces)
      ? cfg.shellWorkspaces
          .map((v) => ({
            id: String((v as { id?: unknown })?.id || "").trim() || "system-workspace",
            name: String((v as { name?: unknown })?.name || "").trim(),
            path: String((v as { path?: unknown })?.path || "").trim(),
            level: "system" as const,
            access: "full_access" as const,
            builtIn: !!(v as { builtIn?: unknown })?.builtIn,
          }))
          .filter((v) => v.name && v.path)
      : [];
    options.config.mcpServers = Array.isArray(cfg.mcpServers)
      ? cfg.mcpServers.map((v) => ({
          id: String((v as { id?: unknown })?.id || "").trim(),
          name: String((v as { name?: unknown })?.name || "").trim(),
          enabled: !!(v as { enabled?: unknown })?.enabled,
          definitionJson: String((v as { definitionJson?: unknown })?.definitionJson || "").trim(),
          toolPolicies: Array.isArray((v as { toolPolicies?: unknown[] })?.toolPolicies)
            ? ((v as { toolPolicies?: unknown[] }).toolPolicies || []).map((p) => ({
                toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                enabled: !!(p as { enabled?: unknown })?.enabled,
              }))
            : [],
          cachedTools: Array.isArray((v as { cachedTools?: unknown[] })?.cachedTools)
            ? ((v as { cachedTools?: unknown[] }).cachedTools || []).map((p) => ({
                toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                description: String((p as { description?: unknown })?.description || "").trim(),
              }))
            : [],
          lastStatus: String((v as { lastStatus?: unknown })?.lastStatus || "").trim(),
          lastError: String((v as { lastError?: unknown })?.lastError || "").trim(),
          updatedAt: String((v as { updatedAt?: unknown })?.updatedAt || "").trim(),
        }))
      : [];
    options.config.remoteImChannels = Array.isArray((cfg as AppConfig).remoteImChannels)
      ? (cfg.remoteImChannels || []).map(mapRemoteImChannel).filter((item) => !!item.id)
      : [];
    options.config.apiProviders = Array.isArray((cfg as AppConfig).apiProviders)
      ? (cfg.apiProviders || []).map((provider) => ({
          id: String((provider as { id?: unknown }).id || "").trim(),
          name: String((provider as { name?: unknown }).name || "").trim(),
          deprecated: !!(provider as { deprecated?: unknown }).deprecated,
          requestFormat: normalizeApiRequestFormat((provider as { requestFormat?: unknown }).requestFormat),
          allowConcurrentRequests: !!(provider as { allowConcurrentRequests?: unknown }).allowConcurrentRequests,
          maxConcurrentRequests: (() => {
            const raw = (provider as { maxConcurrentRequests?: unknown }).maxConcurrentRequests;
            if (raw == null) return null;
            const parsed = Math.round(Number(raw));
            return Number.isFinite(parsed) && parsed >= 1 ? parsed : null;
          })(),
          enableText: !!(provider as { enableText?: unknown }).enableText,
          enableImage: !!(provider as { enableImage?: unknown }).enableImage,
          enableAudio: !!(provider as { enableAudio?: unknown }).enableAudio,
          enableVideo: !!(provider as { enableVideo?: unknown }).enableVideo,
          enableTools: (provider as { enableTools?: unknown }).enableTools !== false,
          tools: Array.isArray((provider as { tools?: unknown[] }).tools)
            ? ((provider as { tools?: unknown[] }).tools || []).map((tool) => ({
                id: String((tool as { id?: unknown }).id || "").trim(),
                command: String((tool as { command?: unknown }).command || ""),
                args: Array.isArray((tool as { args?: unknown[] }).args) ? ((tool as { args?: unknown[] }).args || []).map((arg) => String(arg || "")) : [],
                enabled: (tool as { enabled?: unknown }).enabled !== false,
                values: ((tool as { values?: Record<string, unknown> }).values || {}),
              }))
            : [],
          baseUrl: String((provider as { baseUrl?: unknown }).baseUrl || "").trim(),
          codexAuthMode: normalizeCodexAuthMode((provider as { codexAuthMode?: unknown }).codexAuthMode),
          codexLocalAuthPath: String((provider as { codexLocalAuthPath?: unknown }).codexLocalAuthPath || DEFAULT_CODEX_LOCAL_AUTH_PATH).trim()
            || DEFAULT_CODEX_LOCAL_AUTH_PATH,
          codexCustomUrl: String((provider as { codexCustomUrl?: unknown }).codexCustomUrl || "").trim(),
          codexCustomApiKey: String((provider as { codexCustomApiKey?: unknown }).codexCustomApiKey || "").trim(),
          codexOriginator: String((provider as { codexOriginator?: unknown }).codexOriginator || DEFAULT_CODEX_ORIGINATOR).trim() || DEFAULT_CODEX_ORIGINATOR,
          codexResidencyRequirement: String((provider as { codexResidencyRequirement?: unknown }).codexResidencyRequirement || "").trim() || undefined,
          apiKeys: Array.isArray((provider as { apiKeys?: unknown[] }).apiKeys)
            ? ((provider as { apiKeys?: unknown[] }).apiKeys || []).map((value) => String(value || "").trim()).filter(Boolean)
            : [],
          keyCursor: Math.max(0, Number((provider as { keyCursor?: unknown }).keyCursor || 0)),
          cachedModelOptions: Array.isArray((provider as { cachedModelOptions?: unknown[] }).cachedModelOptions)
            ? ((provider as { cachedModelOptions?: unknown[] }).cachedModelOptions || []).map((value) => String(value || "").trim()).filter(Boolean)
            : [],
          models: Array.isArray((provider as { models?: unknown[] }).models)
            ? ((provider as { models?: unknown[] }).models || []).map((model) => ({
                id: String((model as { id?: unknown }).id || "").trim(),
                model: String((model as { model?: unknown }).model || "").trim(),
                displayName: String((model as { displayName?: unknown }).displayName || "").trim(),
                deprecated: !!(model as { deprecated?: unknown }).deprecated,
                enableImage: !!(model as { enableImage?: unknown }).enableImage,
                enableAudio: !!(model as { enableAudio?: unknown }).enableAudio,
                enableVideo: !!(model as { enableVideo?: unknown }).enableVideo,
                enableTools: (model as { enableTools?: unknown }).enableTools !== false,
                reasoningEffort: String((model as { reasoningEffort?: unknown }).reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
                temperature: Number((model as { temperature?: unknown }).temperature ?? 1),
                customTemperatureEnabled: !!(model as { customTemperatureEnabled?: unknown }).customTemperatureEnabled,
                contextWindowTokens: Math.round(Number((model as { contextWindowTokens?: unknown }).contextWindowTokens ?? 256000)),
                customMaxOutputTokensEnabled: !!(model as { customMaxOutputTokensEnabled?: unknown }).customMaxOutputTokensEnabled,
                maxOutputTokens: Math.round(Number((model as { maxOutputTokens?: unknown }).maxOutputTokens ?? 4096)),
              }))
            : [],
          failureRetryCount: Math.max(0, Number((provider as { failureRetryCount?: unknown }).failureRetryCount || 0)),
        }))
      : [];
    options.config.apiConfigs.splice(
      0,
      options.config.apiConfigs.length,
      ...(cfg.apiConfigs.length
        ? cfg.apiConfigs.map((api) => ({
            ...api,
            requestFormat: normalizeApiRequestFormat((api as { requestFormat?: unknown }).requestFormat),
          }))
        : [options.createApiConfig("default")]),
    );
    options.normalizeApiBindingsLocal();
    lastConversationApiSettingsJson = JSON.stringify({
      expertApiConfigId: options.config.expertApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId || null,
      toolReviewApiConfigId: options.config.toolReviewApiConfigId || null,
      sttApiConfigId: options.config.sttApiConfigId || null,
      sttAutoSend: !!options.config.sttAutoSend,
    });
    options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
  }

  function applyLoadedPersonas(list: PersonaProfile[]) {
    options.personas.value = list.map((item) => ({
      ...item,
      tools: Array.isArray(item.tools)
        ? item.tools.map((tool) => ({
            ...tool,
            args: Array.isArray(tool.args) ? [...tool.args] : [],
            values: { ...((tool.values || {}) as Record<string, unknown>) },
          }))
        : [],
    }));
    if (!options.assistantPersonas.value.some((p) => p.id === options.assistantAgentId.value)) {
      options.assistantAgentId.value = options.assistantPersonas.value[0]?.id ?? "default-agent";
    }
    if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
      options.personaEditorId.value = options.assistantAgentId.value;
    }
    options.syncUserAliasFromPersona();
    options.lastSavedPersonasJson.value = options.buildPersonasSnapshotJson();
  }

  function applyLoadedChatSettings(settings: ChatSettings) {
    options.assistantAgentId.value = String(settings.assistantAgentId ?? "").trim();
    if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
      options.personaEditorId.value = options.assistantAgentId.value;
    }
    options.userAlias.value = String(settings.userAlias ?? "");
    if (typeof settings.responseStyleId === "string") {
      options.selectedResponseStyleId.value = settings.responseStyleId;
    }
    if (settings.pdfReadMode === "text" || settings.pdfReadMode === "image") {
      options.selectedPdfReadMode.value = settings.pdfReadMode;
    }
    options.backgroundVoiceScreenshotKeywords.value = String(settings.backgroundVoiceScreenshotKeywords ?? "");
    if (
      settings.backgroundVoiceScreenshotMode === "desktop"
      || settings.backgroundVoiceScreenshotMode === "focused_window"
    ) {
      options.backgroundVoiceScreenshotMode.value = settings.backgroundVoiceScreenshotMode;
    }
    options.instructionPresets.value = Array.isArray(settings.instructionPresets)
      ? settings.instructionPresets
          .map((item) => ({
            id: String(item?.id || "").trim(),
            name: String(item?.prompt || item?.name || "").trim(),
            prompt: String(item?.prompt || item?.name || "").trim(),
          }))
          .filter((item) => !!item.id && !!item.prompt)
      : [];
  }

  async function loadConfig() {
    options.suppressAutosave.value = true;
    options.loading.value = true;
    options.setStatus(options.t("status.loadingConfig"));
    try {
      const cfg = await invokeTauri<AppConfig>("load_config");
      applyLoadedConfig(cfg);
      options.setStatus(options.t("status.configLoaded"));
    } catch (e) {
      options.setStatusError("status.loadConfigFailed", e);
    } finally {
      options.suppressAutosave.value = false;
      options.loading.value = false;
    }
  }

  async function saveConfig() {
    options.suppressAutosave.value = true;
    options.saving.value = true;
    options.setStatus(options.t("status.savingConfig"));
    try {
      console.info("[配置] save_config invoked");
      const saveResult = await invokeTauri<{ config: AppConfig; repairs?: ConfigRepairNotice[] }>("save_config", { config: options.buildConfigPayload() });
      const saved = saveResult.config;
      const repairs = Array.isArray(saveResult.repairs) ? saveResult.repairs : [];
      options.config.hotkey = saved.hotkey;
      options.config.uiLanguage = options.normalizeLocale(saved.uiLanguage);
      options.config.uiFont = String((saved as { uiFont?: unknown }).uiFont ?? "");
      options.config.codeFont = String((saved as { codeFont?: unknown }).codeFont ?? "");
      options.config.uiSizeScale = normalizeUiSizeScale((saved as { uiSizeScale?: unknown }).uiSizeScale);
      applyUiSizeScale(options.config.uiSizeScale);
      options.config.webAccessPort = normalizeWebAccessPort((saved as { webAccessPort?: unknown }).webAccessPort);
      options.config.webAccessEnabled = (saved as { webAccessEnabled?: unknown }).webAccessEnabled !== false;
      options.config.webAccessPassword = String((saved as { webAccessPassword?: unknown }).webAccessPassword || "").trim();
      options.config.githubUpdateMethod = normalizeGithubUpdateMethod((saved as { githubUpdateMethod?: unknown }).githubUpdateMethod);
      options.locale.value = options.config.uiLanguage;
      options.config.recordHotkey = String(saved.recordHotkey ?? "");
      options.config.recordBackgroundWakeEnabled = !!saved.recordBackgroundWakeEnabled;
      const normalizedConfigNumbers = normalizeConfigNumberFields(
        saved.minRecordSeconds,
        saved.maxRecordSeconds,
        {
          minRecordSeconds: options.config.minRecordSeconds,
          maxRecordSeconds: options.config.maxRecordSeconds,
        },
      );
      options.config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
      options.config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
      options.config.llmRoundLogCapacity = normalizeLlmRoundLogCapacity((saved as AppConfig).llmRoundLogCapacity);
      options.config.messageNotificationEnabled = (saved as { messageNotificationEnabled?: unknown }).messageNotificationEnabled !== false;
      options.config.messageNotificationSoundEnabled = (saved as { messageNotificationSoundEnabled?: unknown }).messageNotificationSoundEnabled === true;
      options.config.desktopOperationNoticeEnabled = (saved as { desktopOperationNoticeEnabled?: unknown }).desktopOperationNoticeEnabled !== false;
      options.config.desktopOperateEnabled = (saved as { desktopOperateEnabled?: unknown }).desktopOperateEnabled !== false;
      options.config.selectedApiConfigId = saved.selectedApiConfigId;
      options.config.expertApiConfigId = saved.expertApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.imageProviders = normalizeImageGenerationProviders(
        (saved as Partial<AppConfig>).imageProviders,
      );
      options.config.imageGenerationModelId = normalizeImageGenerationModelId(
        (saved as Partial<AppConfig>).imageGenerationModelId,
        options.config.imageProviders,
      );
      options.config.toolReviewApiConfigId = saved.toolReviewApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.config.terminalShellKind = String((saved as AppConfig).terminalShellKind ?? "");
      options.config.shellWorkspaces = Array.isArray(saved.shellWorkspaces)
        ? saved.shellWorkspaces
            .map((v) => ({
              id: String((v as { id?: unknown })?.id || "").trim() || "system-workspace",
              name: String((v as { name?: unknown })?.name || "").trim(),
              path: String((v as { path?: unknown })?.path || "").trim(),
              level: "system" as const,
              access: "full_access" as const,
              builtIn: !!(v as { builtIn?: unknown })?.builtIn,
            }))
            .filter((v) => v.name && v.path)
        : [];
      options.config.mcpServers = Array.isArray(saved.mcpServers)
        ? saved.mcpServers.map((v) => ({
            id: String((v as { id?: unknown })?.id || "").trim(),
            name: String((v as { name?: unknown })?.name || "").trim(),
            enabled: !!(v as { enabled?: unknown })?.enabled,
            definitionJson: String((v as { definitionJson?: unknown })?.definitionJson || "").trim(),
            toolPolicies: Array.isArray((v as { toolPolicies?: unknown[] })?.toolPolicies)
              ? ((v as { toolPolicies?: unknown[] }).toolPolicies || []).map((p) => ({
                  toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                  enabled: !!(p as { enabled?: unknown })?.enabled,
                }))
              : [],
            cachedTools: Array.isArray((v as { cachedTools?: unknown[] })?.cachedTools)
              ? ((v as { cachedTools?: unknown[] }).cachedTools || []).map((p) => ({
                  toolName: String((p as { toolName?: unknown })?.toolName || "").trim(),
                  description: String((p as { description?: unknown })?.description || "").trim(),
                }))
              : [],
            lastStatus: String((v as { lastStatus?: unknown })?.lastStatus || "").trim(),
            lastError: String((v as { lastError?: unknown })?.lastError || "").trim(),
            updatedAt: String((v as { updatedAt?: unknown })?.updatedAt || "").trim(),
          }))
        : [];
      options.config.remoteImChannels = Array.isArray((saved as AppConfig).remoteImChannels)
        ? (saved.remoteImChannels || []).map(mapRemoteImChannel).filter((item) => !!item.id)
        : [];
      options.config.apiProviders = Array.isArray((saved as AppConfig).apiProviders)
        ? (saved.apiProviders || []).map((provider) => ({
            ...provider,
            deprecated: !!provider.deprecated,
            apiKeys: Array.isArray(provider.apiKeys) ? [...provider.apiKeys] : [],
            cachedModelOptions: Array.isArray(provider.cachedModelOptions) ? [...provider.cachedModelOptions] : [],
            models: Array.isArray(provider.models)
              ? provider.models.map((model) => ({ ...model, deprecated: !!model.deprecated }))
              : [],
            tools: Array.isArray(provider.tools)
              ? provider.tools.map((tool) => ({
                  ...tool,
                  args: Array.isArray(tool.args) ? [...tool.args] : [],
                  values: { ...((tool.values || {}) as Record<string, unknown>) },
                }))
              : [],
          }))
        : [];
      options.config.apiConfigs.splice(0, options.config.apiConfigs.length, ...saved.apiConfigs);
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      console.info("[配置] save_config success");
      const repairMessage = describeConfigRepairs(repairs, (key, params) => options.t(key, params ?? {}), options.personas.value);
      options.setStatus(repairMessage || options.t("status.configSaved"));
      return true;
    } catch (e) {
      const saveError = classifySaveConfigError(e);
      console.error("[配置] save_config failed:", e);
      if (saveError.kind === "backend_404") {
        options.setStatus(options.t("status.saveConfigBackend404"));
      } else if (saveError.kind === "hotkey_conflict") {
        options.setStatus(options.t("status.saveConfigHotkeyOccupied", { hotkey: saveError.hotkey }));
      } else {
        options.setStatus(options.t("status.saveConfigFailed", { err: saveError.errorText }));
      }
      options.onSaveConfigError?.(saveError);
      return false;
    } finally {
      options.suppressAutosave.value = false;
      options.saving.value = false;
    }
  }

  async function captureHotkey(value: string) {
    const hotkey = String(value || "").trim();
    if (!hotkey) return;
    options.config.hotkey = hotkey;
    options.setStatus(options.t("status.hotkeyUpdated", { hotkey }));
  }

  async function updateRecordHotkey(value: string) {
    const next = String(value || "").trim();
    const current = String(options.config.recordHotkey || "").trim();
    if (next === current) return true;
    options.saving.value = true;
    try {
      const saved = await updateTransportRecordHotkey<RecordHotkeyUpdateResult>(next);
      options.config.recordHotkey = String(saved.recordHotkey || "");
      options.config.recordBackgroundWakeEnabled = !!saved.recordBackgroundWakeEnabled;
      const normalizedConfigNumbers = normalizeConfigNumberFields(
        saved.minRecordSeconds,
        saved.maxRecordSeconds,
        {
          minRecordSeconds: options.config.minRecordSeconds,
          maxRecordSeconds: options.config.maxRecordSeconds,
        },
      );
      options.config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
      options.config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      options.setStatus(options.t("status.configSaved"));
      return true;
    } catch (error) {
      options.setStatus(options.t("status.saveConfigFailed", { err: String(error ?? "unknown") }));
      return false;
    } finally {
      options.saving.value = false;
    }
  }

  async function updateRecordBackgroundWakeEnabled(value: boolean) {
    const next = !!value;
    const previous = !!options.config.recordBackgroundWakeEnabled;
    if (next === previous) return true;
    options.saving.value = true;
    try {
      const saved = await updateTransportRecordBackgroundWake<RecordHotkeyUpdateResult>(next);
      options.config.recordHotkey = String(saved.recordHotkey || "");
      options.config.recordBackgroundWakeEnabled = !!saved.recordBackgroundWakeEnabled;
      const normalizedConfigNumbers = normalizeConfigNumberFields(
        saved.minRecordSeconds,
        saved.maxRecordSeconds,
        {
          minRecordSeconds: options.config.minRecordSeconds,
          maxRecordSeconds: options.config.maxRecordSeconds,
        },
      );
      options.config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
      options.config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      options.setStatus(options.t("status.configSaved"));
      return true;
    } catch (error) {
      options.config.recordBackgroundWakeEnabled = previous;
      options.setStatus(options.t("status.saveConfigFailed", { err: String(error ?? "unknown") }));
      return false;
    } finally {
      options.saving.value = false;
    }
  }

  async function loadPersonas() {
    options.suppressAutosave.value = true;
    try {
      const list = await invokeTauri<PersonaProfile[]>("load_agents");
      applyLoadedPersonas(list);
      await options.preloadPersonaAvatars();
      await options.syncTrayIcon(options.assistantAgentId.value);
    } catch (e) {
      options.setStatusError("status.loadPersonasFailed", e);
    } finally {
      options.suppressAutosave.value = false;
    }
  }

  async function loadChatSettings() {
    options.suppressAutosave.value = true;
    try {
      const settings = await invokeTauri<ChatSettings>("load_chat_settings");
      applyLoadedChatSettings(settings);
      lastChatSettingsJson = JSON.stringify({
        assistantAgentId: options.assistantAgentId.value,
        userAlias: options.userAlias.value,
        responseStyleId: options.selectedResponseStyleId.value,
        pdfReadMode: options.selectedPdfReadMode.value,
        backgroundVoiceScreenshotKeywords: options.backgroundVoiceScreenshotKeywords.value,
        backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
        instructionPresets: options.instructionPresets.value,
      });
      await options.syncTrayIcon(options.assistantAgentId.value);
    } catch (e) {
      options.setStatusError("status.loadChatSettingsFailed", e);
    } finally {
      options.suppressAutosave.value = false;
    }
  }

  async function loadBootstrapSnapshot() {
    options.suppressAutosave.value = true;
    options.loading.value = true;
    options.setStatus(options.t("status.loadingConfig"));
    const perfNow = options.perfNow;
    const perfLog = options.perfLog;
    try {
      const tInvoke = perfNow?.();
      const snapshot = await invokeTauri<AppBootstrapSnapshot>("load_app_bootstrap_snapshot");
      if (tInvoke !== undefined) perfLog?.("loadBootstrapSnapshot/invoke", tInvoke);

      const tApplyConfig = perfNow?.();
      applyLoadedConfig(snapshot.config);
      if (tApplyConfig !== undefined) perfLog?.("loadBootstrapSnapshot/applyConfig", tApplyConfig);

      const tApplyPersonas = perfNow?.();
      applyLoadedPersonas(snapshot.agents);
      if (tApplyPersonas !== undefined) perfLog?.("loadBootstrapSnapshot/applyPersonas", tApplyPersonas);

      const tApplyChatSettings = perfNow?.();
      applyLoadedChatSettings(snapshot.chatSettings);
      if (tApplyChatSettings !== undefined) {
        perfLog?.("loadBootstrapSnapshot/applyChatSettings", tApplyChatSettings);
      }

      const tSnapshotJson = perfNow?.();
      lastChatSettingsJson = JSON.stringify({
        assistantAgentId: options.assistantAgentId.value,
        userAlias: options.userAlias.value,
        responseStyleId: options.selectedResponseStyleId.value,
        pdfReadMode: options.selectedPdfReadMode.value,
        backgroundVoiceScreenshotKeywords: options.backgroundVoiceScreenshotKeywords.value,
        backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
        instructionPresets: options.instructionPresets.value,
      });
      lastConversationApiSettingsJson = JSON.stringify({
        expertApiConfigId: options.config.expertApiConfigId,
        visionApiConfigId: options.config.visionApiConfigId || null,
        toolReviewApiConfigId: options.config.toolReviewApiConfigId || null,
        sttApiConfigId: options.config.sttApiConfigId || null,
        sttAutoSend: !!options.config.sttAutoSend,
      });
      if (tSnapshotJson !== undefined) {
        perfLog?.("loadBootstrapSnapshot/snapshotJson", tSnapshotJson);
      }

      const tPreloadAvatars = perfNow?.();
      await options.preloadPersonaAvatars();
      if (tPreloadAvatars !== undefined) {
        perfLog?.("loadBootstrapSnapshot/preloadPersonaAvatars", tPreloadAvatars);
      }

      const tSyncTray = perfNow?.();
      await options.syncTrayIcon(options.assistantAgentId.value);
      if (tSyncTray !== undefined) perfLog?.("loadBootstrapSnapshot/syncTrayIcon", tSyncTray);

      options.setStatus(options.t("status.configLoaded"));
      return true;
    } catch (e) {
      options.setStatusError("status.loadConfigFailed", e);
      return false;
    } finally {
      options.suppressAutosave.value = false;
      options.loading.value = false;
    }
  }

  async function savePersonas() {
    options.suppressAutosave.value = true;
    options.savingPersonas.value = true;
    try {
      options.personas.value = await invokeTauri<PersonaProfile[]>("save_agents", {
        input: { agents: options.personas.value },
      });
      options.personas.value = options.personas.value.map((item) => ({
        ...item,
        tools: Array.isArray(item.tools)
          ? item.tools.map((tool) => ({
              ...tool,
              args: Array.isArray(tool.args) ? [...tool.args] : [],
              values: { ...((tool.values || {}) as Record<string, unknown>) },
            }))
          : [],
      }));
      options.syncUserAliasFromPersona();
      options.lastSavedPersonasJson.value = options.buildPersonasSnapshotJson();
      options.setStatus(options.t("status.personaSaved"));
      return true;
    } catch (e) {
      options.setStatusError("status.savePersonasFailed", e);
      return false;
    } finally {
      options.savingPersonas.value = false;
      options.suppressAutosave.value = false;
    }
  }

  async function convertPrivatePersonaToPublic(agentId: string) {
    const normalizedAgentId = String(agentId || "").trim();
    if (!normalizedAgentId) return false;
    options.suppressAutosave.value = true;
    options.savingPersonas.value = true;
    try {
      options.personas.value = await invokeTauri<PersonaProfile[]>("convert_private_agent_to_main", {
        input: { agentId: normalizedAgentId },
      });
      options.personas.value = options.personas.value.map((item) => ({
        ...item,
        tools: Array.isArray(item.tools)
          ? item.tools.map((tool) => ({
              ...tool,
              args: Array.isArray(tool.args) ? [...tool.args] : [],
              values: { ...((tool.values || {}) as Record<string, unknown>) },
            }))
          : [],
      }));
      options.syncUserAliasFromPersona();
      options.lastSavedPersonasJson.value = options.buildPersonasSnapshotJson();
      options.setStatus(options.t("config.persona.convertToPublicSuccess"));
      return true;
    } catch (e) {
      options.setStatusError("config.persona.convertToPublicFailed", e);
      return false;
    } finally {
      options.savingPersonas.value = false;
      options.suppressAutosave.value = false;
    }
  }

  async function saveChatPreferences() {
    await patchChatSettings({
      assistantAgentId: options.assistantAgentId.value,
      userAlias: options.userAlias.value,
      responseStyleId: options.selectedResponseStyleId.value,
      pdfReadMode: options.selectedPdfReadMode.value,
      backgroundVoiceScreenshotKeywords: options.backgroundVoiceScreenshotKeywords.value,
      backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
      instructionPresets: options.instructionPresets.value,
    });
  }

  async function saveConversationApiSettings() {
    await patchConversationApiSettings({
      expertApiConfigId: options.config.expertApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId || null,
      toolReviewApiConfigId: options.config.toolReviewApiConfigId || null,
      sttApiConfigId: options.config.sttApiConfigId || null,
      sttAutoSend: !!options.config.sttAutoSend,
    });
  }

  async function patchChatSettings(patch: ChatSettingsPatch) {
    if (options.suppressAutosave.value) return;
    const normalizedPatch: ChatSettingsPatch = {};
    if (Object.prototype.hasOwnProperty.call(patch, "assistantAgentId")) {
      const targetAgentId = options.assistantPersonas.value.some((p) => p.id === patch.assistantAgentId)
        ? patch.assistantAgentId
        : options.assistantPersonas.value[0]?.id || "default-agent";
      normalizedPatch.assistantAgentId = targetAgentId;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "userAlias")) {
      normalizedPatch.userAlias = String(patch.userAlias || "");
    }
    if (Object.prototype.hasOwnProperty.call(patch, "responseStyleId")) {
      normalizedPatch.responseStyleId = String(patch.responseStyleId || "");
    }
    if (Object.prototype.hasOwnProperty.call(patch, "pdfReadMode")) {
      normalizedPatch.pdfReadMode = patch.pdfReadMode;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "backgroundVoiceScreenshotKeywords")) {
      const normalizedScreenshotKeywords = String(patch.backgroundVoiceScreenshotKeywords || "").replace(/，/g, ",");
      options.backgroundVoiceScreenshotKeywords.value = normalizedScreenshotKeywords;
      normalizedPatch.backgroundVoiceScreenshotKeywords = normalizedScreenshotKeywords;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "backgroundVoiceScreenshotMode")) {
      normalizedPatch.backgroundVoiceScreenshotMode = patch.backgroundVoiceScreenshotMode;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "instructionPresets")) {
      normalizedPatch.instructionPresets = Array.isArray(patch.instructionPresets)
        ? patch.instructionPresets.map((item) => ({
            id: String(item?.id || "").trim(),
            name: String(item?.prompt || item?.name || "").trim(),
            prompt: String(item?.prompt || item?.name || "").trim(),
          })).filter((item) => !!item.id && !!item.prompt)
        : [];
    }
    if (Object.keys(normalizedPatch).length === 0) return;
    const nextChatSettingsJson = JSON.stringify({
      assistantAgentId: Object.prototype.hasOwnProperty.call(normalizedPatch, "assistantAgentId")
        ? normalizedPatch.assistantAgentId
        : options.assistantAgentId.value,
      userAlias: Object.prototype.hasOwnProperty.call(normalizedPatch, "userAlias")
        ? normalizedPatch.userAlias
        : options.userAlias.value,
      responseStyleId: Object.prototype.hasOwnProperty.call(normalizedPatch, "responseStyleId")
        ? normalizedPatch.responseStyleId
        : options.selectedResponseStyleId.value,
      pdfReadMode: Object.prototype.hasOwnProperty.call(normalizedPatch, "pdfReadMode")
        ? normalizedPatch.pdfReadMode
        : options.selectedPdfReadMode.value,
      backgroundVoiceScreenshotKeywords: Object.prototype.hasOwnProperty.call(normalizedPatch, "backgroundVoiceScreenshotKeywords")
        ? normalizedPatch.backgroundVoiceScreenshotKeywords
        : options.backgroundVoiceScreenshotKeywords.value,
      backgroundVoiceScreenshotMode: Object.prototype.hasOwnProperty.call(normalizedPatch, "backgroundVoiceScreenshotMode")
        ? normalizedPatch.backgroundVoiceScreenshotMode
        : options.backgroundVoiceScreenshotMode.value,
      instructionPresets: Object.prototype.hasOwnProperty.call(normalizedPatch, "instructionPresets")
        ? normalizedPatch.instructionPresets
        : options.instructionPresets.value,
    });
    if (chatSettingsSaving || nextChatSettingsJson === lastChatSettingsJson) return;
    chatSettingsSaving = true;
    options.saving.value = true;
    options.setStatus(options.t("status.savingChatSettings"));
    try {
      const saved = await invokeTauri<ChatSettings>("patch_chat_settings", { input: normalizedPatch });
      applyLoadedChatSettings(saved);
      lastChatSettingsJson = JSON.stringify({
        assistantAgentId: options.assistantAgentId.value,
        userAlias: options.userAlias.value,
        responseStyleId: options.selectedResponseStyleId.value,
        pdfReadMode: options.selectedPdfReadMode.value,
        backgroundVoiceScreenshotKeywords: options.backgroundVoiceScreenshotKeywords.value,
        backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
        instructionPresets: options.instructionPresets.value,
      });
      options.setStatus(options.t("status.chatSettingsSaved"));
      await options.syncTrayIcon(options.assistantAgentId.value);
    } catch (e) {
      options.setStatusError("status.saveChatSettingsFailed", e);
    } finally {
      options.saving.value = false;
      chatSettingsSaving = false;
    }
  }

  async function patchConversationApiSettings(patch: ConversationApiSettingsPatch) {
    if (options.suppressAutosave.value) return;
    const normalizedPatch: ConversationApiSettingsPatch = {};
    if (Object.prototype.hasOwnProperty.call(patch, "expertApiConfigId")) {
      normalizedPatch.expertApiConfigId = String(patch.expertApiConfigId || "");
    }
    if (Object.prototype.hasOwnProperty.call(patch, "visionApiConfigId")) {
      normalizedPatch.visionApiConfigId = patch.visionApiConfigId ?? null;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "toolReviewApiConfigId")) {
      normalizedPatch.toolReviewApiConfigId = patch.toolReviewApiConfigId ?? null;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "sttApiConfigId")) {
      normalizedPatch.sttApiConfigId = patch.sttApiConfigId ?? null;
    }
    if (Object.prototype.hasOwnProperty.call(patch, "sttAutoSend")) {
      normalizedPatch.sttAutoSend = !!patch.sttAutoSend;
    }
    if (Object.keys(normalizedPatch).length === 0) return;
    const nextPayloadJson = JSON.stringify({
      expertApiConfigId: Object.prototype.hasOwnProperty.call(normalizedPatch, "expertApiConfigId")
        ? normalizedPatch.expertApiConfigId
        : options.config.expertApiConfigId,
      visionApiConfigId: Object.prototype.hasOwnProperty.call(normalizedPatch, "visionApiConfigId")
        ? normalizedPatch.visionApiConfigId
        : options.config.visionApiConfigId || null,
      toolReviewApiConfigId: Object.prototype.hasOwnProperty.call(normalizedPatch, "toolReviewApiConfigId")
        ? normalizedPatch.toolReviewApiConfigId
        : options.config.toolReviewApiConfigId || null,
      sttApiConfigId: Object.prototype.hasOwnProperty.call(normalizedPatch, "sttApiConfigId")
        ? normalizedPatch.sttApiConfigId
        : options.config.sttApiConfigId || null,
      sttAutoSend: Object.prototype.hasOwnProperty.call(normalizedPatch, "sttAutoSend")
        ? normalizedPatch.sttAutoSend
        : !!options.config.sttAutoSend,
    });
    if (conversationApiSettingsSaving || nextPayloadJson === lastConversationApiSettingsJson) return;
    conversationApiSettingsSaving = true;
    try {
      console.info("[配置] patch_conversation_api_settings invoked");
      const saved = await invokeTauri<ConversationApiSettings>("patch_conversation_api_settings", {
        input: normalizedPatch,
      });
      options.config.expertApiConfigId = saved.expertApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.toolReviewApiConfigId = saved.toolReviewApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      lastConversationApiSettingsJson = JSON.stringify({
        expertApiConfigId: options.config.expertApiConfigId,
        visionApiConfigId: options.config.visionApiConfigId || null,
        toolReviewApiConfigId: options.config.toolReviewApiConfigId || null,
        sttApiConfigId: options.config.sttApiConfigId || null,
        sttAutoSend: !!options.config.sttAutoSend,
      });
      console.info("[配置] patch_conversation_api_settings success");
    } catch (e) {
      console.error("[配置] patch_conversation_api_settings failed:", e);
      options.setStatusError("status.saveConversationLlmFailed", e);
    } finally {
      conversationApiSettingsSaving = false;
    }
  }

  function restoreLastSavedConfigSnapshot(): boolean {
    const raw = String(options.lastSavedConfigJson.value || "").trim();
    if (!raw) return false;
    try {
      const snapshot = JSON.parse(raw) as AppConfig;
      applyLoadedConfig(snapshot);
      options.setStatus("已还原未保存配置");
      return true;
    } catch (e) {
      console.error("[配置] restore_last_saved_config_snapshot failed:", e);
      options.setStatusError("status.loadConfigFailed", e);
      return false;
    }
  }

  return {
    loadConfig,
    loadBootstrapSnapshot,
    saveConfig,
    captureHotkey,
    updateRecordHotkey,
    updateRecordBackgroundWakeEnabled,
    loadPersonas,
    loadChatSettings,
    savePersonas,
    convertPrivatePersonaToPublic,
    patchChatSettings,
    saveChatPreferences,
    patchConversationApiSettings,
    saveConversationApiSettings,
    restoreLastSavedConfigSnapshot,
  };
}
