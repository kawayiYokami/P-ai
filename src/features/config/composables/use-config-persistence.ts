import type { ComputedRef, Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { AppConfig, PersonaProfile } from "../../../types/app";
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
  personas: Ref<PersonaProfile[]>;
  assistantPersonas: ComputedRef<PersonaProfile[]>;
  selectedPersonaId: Ref<string>;
  personaEditorId: Ref<string>;
  userAlias: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  responseStyleIds: ComputedRef<string[]>;
  createApiConfig: (name?: string) => AppConfig["apiConfigs"][number];
  normalizeApiBindingsLocal: () => void;
  buildConfigPayload: () => AppConfig;
  buildConfigSnapshotJson: () => string;
  lastSavedConfigJson: Ref<string>;
  syncUserAliasFromPersona: () => void;
  preloadPersonaAvatars: () => Promise<void>;
  syncTrayIcon: (agentId?: string) => Promise<void>;
};

const MIN_RECORD_SECONDS = 1;
const MAX_MIN_RECORD_SECONDS = 30;
const DEFAULT_MAX_RECORD_SECONDS = 60;
const MAX_RECORD_SECONDS = 600;

export function useConfigPersistence(options: UseConfigPersistenceOptions) {
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

  function normalizeRecordSeconds(
    minValue: unknown,
    maxValue: unknown,
  ): { minRecordSeconds: number; maxRecordSeconds: number } {
    const minRecordSeconds = Math.max(
      MIN_RECORD_SECONDS,
      Math.min(MAX_MIN_RECORD_SECONDS, Math.round(Number(minValue) || MIN_RECORD_SECONDS)),
    );
    const maxRecordSeconds = Math.max(
      minRecordSeconds,
      Math.min(MAX_RECORD_SECONDS, Math.round(Number(maxValue) || DEFAULT_MAX_RECORD_SECONDS)),
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

  async function loadConfig() {
    options.suppressAutosave.value = true;
    options.loading.value = true;
    options.setStatus(options.t("status.loadingConfig"));
    try {
      const cfg = await invokeTauri<AppConfig>("load_config");
      options.config.hotkey = cfg.hotkey;
      options.config.uiLanguage = options.normalizeLocale(cfg.uiLanguage);
      options.config.uiFont = String((cfg as { uiFont?: unknown }).uiFont || "auto").trim() || "auto";
      options.locale.value = options.config.uiLanguage;
      options.config.recordHotkey = cfg.recordHotkey || "Alt";
      const normalizedRecord = normalizeRecordSeconds(cfg.minRecordSeconds, cfg.maxRecordSeconds);
      options.config.minRecordSeconds = normalizedRecord.minRecordSeconds;
      options.config.maxRecordSeconds = normalizedRecord.maxRecordSeconds;
      options.config.toolMaxIterations = Math.max(1, Math.min(100, Number(cfg.toolMaxIterations || 10)));
      options.config.selectedApiConfigId = cfg.selectedApiConfigId;
      options.config.chatApiConfigId = cfg.chatApiConfigId;
      options.config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
      options.config.sttApiConfigId = cfg.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!cfg.sttAutoSend;
      options.config.shellWorkspaces = Array.isArray(cfg.shellWorkspaces)
        ? cfg.shellWorkspaces
            .map((v) => ({
              name: String((v as { name?: unknown })?.name || "").trim(),
              path: String((v as { path?: unknown })?.path || "").trim(),
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
      options.config.apiConfigs.splice(
        0,
        options.config.apiConfigs.length,
        ...(cfg.apiConfigs.length ? cfg.apiConfigs : [options.createApiConfig("default")]),
      );
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
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
      options.config.uiFont = String((saved as { uiFont?: unknown }).uiFont || "auto").trim() || "auto";
      options.locale.value = options.config.uiLanguage;
      options.config.recordHotkey = saved.recordHotkey || "Alt";
      const normalizedRecord = normalizeRecordSeconds(saved.minRecordSeconds, saved.maxRecordSeconds);
      options.config.minRecordSeconds = normalizedRecord.minRecordSeconds;
      options.config.maxRecordSeconds = normalizedRecord.maxRecordSeconds;
      options.config.toolMaxIterations = Math.max(1, Math.min(100, Number(saved.toolMaxIterations || 10)));
      options.config.selectedApiConfigId = saved.selectedApiConfigId;
      options.config.chatApiConfigId = saved.chatApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.config.shellWorkspaces = Array.isArray(saved.shellWorkspaces)
        ? saved.shellWorkspaces
            .map((v) => ({
              name: String((v as { name?: unknown })?.name || "").trim(),
              path: String((v as { path?: unknown })?.path || "").trim(),
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
    const saved = await saveConfig();
    if (!saved) return;
    options.setStatus(options.t("status.hotkeyUpdated", { hotkey }));
  }

  async function loadPersonas() {
    options.suppressAutosave.value = true;
    try {
      const list = await invokeTauri<PersonaProfile[]>("load_agents");
      options.personas.value = list;
      if (!options.assistantPersonas.value.some((p) => p.id === options.selectedPersonaId.value)) {
        options.selectedPersonaId.value = options.assistantPersonas.value[0]?.id ?? "default-agent";
      }
      if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
        options.personaEditorId.value = options.selectedPersonaId.value;
      }
      options.syncUserAliasFromPersona();
      await options.preloadPersonaAvatars();
      await options.syncTrayIcon(options.selectedPersonaId.value);
    } catch (e) {
      options.setStatusError("status.loadPersonasFailed", e);
    } finally {
      options.suppressAutosave.value = false;
    }
  }

  async function loadChatSettings() {
    options.suppressAutosave.value = true;
    try {
      const settings = await invokeTauri<{ selectedAgentId: string; userAlias: string; responseStyleId: string }>(
        "load_chat_settings",
      );
      if (options.assistantPersonas.value.some((p) => p.id === settings.selectedAgentId)) {
        options.selectedPersonaId.value = settings.selectedAgentId;
      }
      if (!options.personas.value.some((p) => p.id === options.personaEditorId.value)) {
        options.personaEditorId.value = options.selectedPersonaId.value;
      }
      options.userAlias.value = settings.userAlias?.trim() || options.t("archives.roleUser");
      if (options.responseStyleIds.value.includes(settings.responseStyleId)) {
        options.selectedResponseStyleId.value = settings.responseStyleId;
      } else {
        options.selectedResponseStyleId.value = "concise";
      }
      await options.syncTrayIcon(options.selectedPersonaId.value);
    } catch (e) {
      options.setStatusError("status.loadChatSettingsFailed", e);
    } finally {
      options.suppressAutosave.value = false;
    }
  }

  async function savePersonas() {
    options.suppressAutosave.value = true;
    try {
      options.personas.value = await invokeTauri<PersonaProfile[]>("save_agents", {
        input: { agents: options.personas.value },
      });
      options.syncUserAliasFromPersona();
      options.setStatus(options.t("status.personaSaved"));
    } catch (e) {
      options.setStatusError("status.savePersonasFailed", e);
    } finally {
      options.suppressAutosave.value = false;
    }
  }

  async function saveChatPreferences() {
    options.saving.value = true;
    options.setStatus(options.t("status.savingChatSettings"));
    try {
      const targetAgentId = options.assistantPersonas.value.some((p) => p.id === options.selectedPersonaId.value)
        ? options.selectedPersonaId.value
        : options.assistantPersonas.value[0]?.id || "default-agent";
      await invokeTauri("save_chat_settings", {
        input: {
          selectedAgentId: targetAgentId,
          userAlias: options.userAlias.value,
          responseStyleId: options.selectedResponseStyleId.value,
        },
      });
      options.selectedPersonaId.value = targetAgentId;
      options.setStatus(options.t("status.chatSettingsSaved"));
    } catch (e) {
      options.setStatusError("status.saveChatSettingsFailed", e);
    } finally {
      options.saving.value = false;
    }
  }

  async function saveConversationApiSettings() {
    if (options.suppressAutosave.value) return;
    try {
      console.info("[CONFIG] save_conversation_api_settings invoked");
      const saved = await invokeTauri<{
        chatApiConfigId: string;
        visionApiConfigId?: string;
        sttApiConfigId?: string;
        sttAutoSend?: boolean;
      }>("save_conversation_api_settings", {
        input: {
          chatApiConfigId: options.config.chatApiConfigId,
          visionApiConfigId: options.config.visionApiConfigId || null,
          sttApiConfigId: options.config.sttApiConfigId || null,
          sttAutoSend: !!options.config.sttAutoSend,
        },
      });
      options.config.chatApiConfigId = saved.chatApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.sttApiConfigId = saved.sttApiConfigId ?? undefined;
      options.config.sttAutoSend = !!saved.sttAutoSend;
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      console.info("[CONFIG] save_conversation_api_settings success");
    } catch (e) {
      console.error("[CONFIG] save_conversation_api_settings failed:", e);
      options.setStatusError("status.saveConversationLlmFailed", e);
    }
  }

  return {
    loadConfig,
    saveConfig,
    captureHotkey,
    loadPersonas,
    loadChatSettings,
    savePersonas,
    saveChatPreferences,
    saveConversationApiSettings,
  };
}
