import type { ComputedRef, Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type {
  AppBootstrapSnapshot,
  AppConfig,
  ChatSettings,
  CodexAuthMode,
  PdfReadMode,
  PersonaProfile,
  PromptCommandPreset,
  RemoteImChannelConfig,
} from "../../../types/app";
import type { SupportedLocale } from "../../../i18n";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

export type ConfigSaveErrorKind = "hotkey_conflict" | "backend_404" | "unknown";

export type ConfigSaveErrorInfo = {
  kind: ConfigSaveErrorKind;
  errorText: string;
  hotkey: string;
};

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
  assistantDepartmentAgentId: Ref<string>;
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
};

function mapDepartmentConfig(item: unknown): AppConfig["departments"][number] {
  const apiConfigIds = Array.isArray((item as { apiConfigIds?: unknown[] })?.apiConfigIds)
    ? ((item as { apiConfigIds?: unknown[] }).apiConfigIds || []).map((v) => String(v || "").trim()).filter(Boolean)
    : [];
  const legacyApiConfigId = String((item as { apiConfigId?: unknown })?.apiConfigId || "").trim();
  const normalizedApiConfigIds = Array.from(new Set((apiConfigIds.length > 0 ? apiConfigIds : [legacyApiConfigId]).filter(Boolean)));
  return {
    id: String((item as { id?: unknown })?.id || "").trim(),
    name: String((item as { name?: unknown })?.name || "").trim(),
    summary: String((item as { summary?: unknown })?.summary || "").trim(),
    guide: String((item as { guide?: unknown })?.guide || "").trim(),
    apiConfigId: normalizedApiConfigIds[0] || "",
    apiConfigIds: normalizedApiConfigIds,
    agentIds: Array.isArray((item as { agentIds?: unknown[] })?.agentIds)
      ? ((item as { agentIds?: unknown[] }).agentIds || []).map((v) => String(v || "").trim()).filter(Boolean)
      : [],
    createdAt: String((item as { createdAt?: unknown })?.createdAt || "").trim(),
    updatedAt: String((item as { updatedAt?: unknown })?.updatedAt || "").trim(),
    orderIndex: Math.max(1, Number((item as { orderIndex?: unknown })?.orderIndex || 1)),
    isBuiltInAssistant: !!(item as { isBuiltInAssistant?: unknown })?.isBuiltInAssistant,
    source: String((item as { source?: unknown })?.source || "").trim() || "main_config",
    scope: String((item as { scope?: unknown })?.scope || "").trim() || "global",
  };
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
    activateAssistant: (item as { activateAssistant?: unknown })?.activateAssistant !== false,
    receiveFiles: (item as { receiveFiles?: unknown })?.receiveFiles !== false,
    streamingSend: !!(item as { streamingSend?: unknown })?.streamingSend,
    showToolCalls: !!(item as { showToolCalls?: unknown })?.showToolCalls,
    allowSendFiles: !!(item as { allowSendFiles?: unknown })?.allowSendFiles,
  };
}

export function useConfigPersistence(options: UseConfigPersistenceOptions) {
  let lastConversationApiSettingsJson = "";
  let conversationApiSettingsSaving = false;
  const MIN_RECORD_SECONDS = 1;
  const MAX_MIN_RECORD_SECONDS = 30;
  const DEFAULT_MAX_RECORD_SECONDS = 60;
  const MAX_RECORD_SECONDS = 600;
  const DEFAULT_CODEX_AUTH_MODE = "read_local";
  const DEFAULT_CODEX_LOCAL_AUTH_PATH = "~/.codex/auth.json";
  const DEFAULT_REASONING_EFFORT = "medium";

  function normalizeCodexAuthMode(value: unknown): CodexAuthMode {
    return String(value || "").trim() === "managed_oauth" ? "managed_oauth" : "read_local";
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
    options.locale.value = options.config.uiLanguage;
    options.config.recordHotkey = String(cfg.recordHotkey ?? "");
    options.config.recordBackgroundWakeEnabled = !!cfg.recordBackgroundWakeEnabled;
    const normalizedConfigNumbers = normalizeConfigNumberFields(
      cfg.minRecordSeconds,
      cfg.maxRecordSeconds,
    );
    options.config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
    options.config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
    options.config.selectedApiConfigId = cfg.selectedApiConfigId;
    options.config.assistantDepartmentApiConfigId = cfg.assistantDepartmentApiConfigId;
    options.config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
    options.config.sttApiConfigId = cfg.sttApiConfigId ?? undefined;
    options.config.sttAutoSend = !!cfg.sttAutoSend;
    options.config.terminalShellKind = String((cfg as AppConfig).terminalShellKind ?? "");
    options.config.departments = Array.isArray((cfg as AppConfig).departments)
      ? (cfg.departments || []).map(mapDepartmentConfig)
      : [];
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
          requestFormat: (String((provider as { requestFormat?: unknown }).requestFormat || "openai").trim() as AppConfig["apiProviders"][number]["requestFormat"]),
          enableText: !!(provider as { enableText?: unknown }).enableText,
          enableImage: !!(provider as { enableImage?: unknown }).enableImage,
          enableAudio: !!(provider as { enableAudio?: unknown }).enableAudio,
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
                enableImage: !!(model as { enableImage?: unknown }).enableImage,
                enableTools: (model as { enableTools?: unknown }).enableTools !== false,
                reasoningEffort: String((model as { reasoningEffort?: unknown }).reasoningEffort || DEFAULT_REASONING_EFFORT).trim() || DEFAULT_REASONING_EFFORT,
                temperature: Number((model as { temperature?: unknown }).temperature ?? 1),
                customTemperatureEnabled: !!(model as { customTemperatureEnabled?: unknown }).customTemperatureEnabled,
                contextWindowTokens: Math.round(Number((model as { contextWindowTokens?: unknown }).contextWindowTokens ?? 128000)),
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
      ...(cfg.apiConfigs.length ? cfg.apiConfigs : [options.createApiConfig("default")]),
    );
    options.normalizeApiBindingsLocal();
    lastConversationApiSettingsJson = JSON.stringify({
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId || null,
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
    if (!options.assistantPersonas.value.some((p) => p.id === options.assistantDepartmentAgentId.value)) {
      options.assistantDepartmentAgentId.value = options.assistantPersonas.value[0]?.id ?? "default-agent";
    }
    if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
      options.personaEditorId.value = options.assistantDepartmentAgentId.value;
    }
    options.syncUserAliasFromPersona();
    options.lastSavedPersonasJson.value = options.buildPersonasSnapshotJson();
  }

  function applyLoadedChatSettings(settings: ChatSettings) {
    options.assistantDepartmentAgentId.value = String(settings.assistantDepartmentAgentId ?? "").trim();
    if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
      options.personaEditorId.value = options.assistantDepartmentAgentId.value;
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
            name: String(item?.name || "").trim(),
            prompt: String(item?.prompt || "").trim(),
          }))
          .filter((item) => !!item.id && !!item.name && !!item.prompt)
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
      console.info("[CONFIG] save_config invoked");
      const saved = await invokeTauri<AppConfig>("save_config", { config: options.buildConfigPayload() });
      options.config.hotkey = saved.hotkey;
      options.config.uiLanguage = options.normalizeLocale(saved.uiLanguage);
      options.config.uiFont = String((saved as { uiFont?: unknown }).uiFont ?? "");
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
      options.config.selectedApiConfigId = saved.selectedApiConfigId;
      options.config.assistantDepartmentApiConfigId = saved.assistantDepartmentApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.config.terminalShellKind = String((saved as AppConfig).terminalShellKind ?? "");
      options.config.departments = Array.isArray(saved.departments)
        ? (saved.departments || []).map(mapDepartmentConfig)
        : [];
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
        ? (saved.apiProviders || []).map((provider) => ({ ...provider }))
        : [];
      options.config.apiConfigs.splice(0, options.config.apiConfigs.length, ...saved.apiConfigs);
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      console.info("[CONFIG] save_config success");
      options.setStatus(options.t("status.configSaved"));
      return true;
    } catch (e) {
      const saveError = classifySaveConfigError(e);
      console.error("[CONFIG] save_config failed:", e);
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

  async function loadPersonas() {
    options.suppressAutosave.value = true;
    try {
      const list = await invokeTauri<PersonaProfile[]>("load_agents");
      applyLoadedPersonas(list);
      await options.preloadPersonaAvatars();
      await options.syncTrayIcon(options.assistantDepartmentAgentId.value);
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
      await options.syncTrayIcon(options.assistantDepartmentAgentId.value);
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
    try {
      const snapshot = await invokeTauri<AppBootstrapSnapshot>("load_app_bootstrap_snapshot");
      applyLoadedConfig(snapshot.config);
      applyLoadedPersonas(snapshot.agents);
      applyLoadedChatSettings(snapshot.chatSettings);
      await options.preloadPersonaAvatars();
      await options.syncTrayIcon(options.assistantDepartmentAgentId.value);
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

  async function saveChatPreferences() {
    options.saving.value = true;
    options.setStatus(options.t("status.savingChatSettings"));
    try {
      const targetAgentId = options.assistantPersonas.value.some((p) => p.id === options.assistantDepartmentAgentId.value)
        ? options.assistantDepartmentAgentId.value
        : options.assistantPersonas.value[0]?.id || "default-agent";
      const normalizedScreenshotKeywords = String(options.backgroundVoiceScreenshotKeywords.value || "").replace(/，/g, ",");
      options.backgroundVoiceScreenshotKeywords.value = normalizedScreenshotKeywords;
      await invokeTauri("save_chat_settings", {
        input: {
          assistantDepartmentAgentId: targetAgentId,
          userAlias: options.userAlias.value,
          responseStyleId: options.selectedResponseStyleId.value,
          pdfReadMode: options.selectedPdfReadMode.value,
          backgroundVoiceScreenshotKeywords: normalizedScreenshotKeywords,
          backgroundVoiceScreenshotMode: options.backgroundVoiceScreenshotMode.value,
          instructionPresets: options.instructionPresets.value,
        },
      });
      options.assistantDepartmentAgentId.value = targetAgentId;
      options.setStatus(options.t("status.chatSettingsSaved"));
    } catch (e) {
      options.setStatusError("status.saveChatSettingsFailed", e);
    } finally {
      options.saving.value = false;
    }
  }

  async function saveConversationApiSettings() {
    if (options.suppressAutosave.value) return;
    const payload = {
      assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
      visionApiConfigId: options.config.visionApiConfigId || null,
      sttApiConfigId: options.config.sttApiConfigId || null,
      sttAutoSend: !!options.config.sttAutoSend,
    };
    const payloadJson = JSON.stringify(payload);
    if (conversationApiSettingsSaving || payloadJson === lastConversationApiSettingsJson) {
      return;
    }
    conversationApiSettingsSaving = true;
    try {
      console.info("[CONFIG] save_conversation_api_settings invoked");
      const saved = await invokeTauri<{
        assistantDepartmentApiConfigId: string;
        visionApiConfigId?: string;
        sttApiConfigId?: string;
        sttAutoSend?: boolean;
      }>("save_conversation_api_settings", {
        input: payload,
      });
      options.config.assistantDepartmentApiConfigId = saved.assistantDepartmentApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      lastConversationApiSettingsJson = JSON.stringify({
        assistantDepartmentApiConfigId: options.config.assistantDepartmentApiConfigId,
        visionApiConfigId: options.config.visionApiConfigId || null,
        sttApiConfigId: options.config.sttApiConfigId || null,
        sttAutoSend: !!options.config.sttAutoSend,
      });
      console.info("[CONFIG] save_conversation_api_settings success");
    } catch (e) {
      console.error("[CONFIG] save_conversation_api_settings failed:", e);
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
      console.error("[CONFIG] restore_last_saved_config_snapshot failed:", e);
      options.setStatusError("status.loadConfigFailed", e);
      return false;
    }
  }

  return {
    loadConfig,
    loadBootstrapSnapshot,
    saveConfig,
    captureHotkey,
    loadPersonas,
    loadChatSettings,
    savePersonas,
    saveChatPreferences,
    saveConversationApiSettings,
    restoreLastSavedConfigSnapshot,
  };
}
