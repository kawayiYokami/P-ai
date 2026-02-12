import type { ComputedRef, Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { AppConfig, PersonaProfile } from "../../../types/app";
import type { SupportedLocale } from "../../../i18n";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseConfigPersistenceOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
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

export function useConfigPersistence(options: UseConfigPersistenceOptions) {
  async function loadConfig() {
    options.suppressAutosave.value = true;
    options.loading.value = true;
    options.setStatus(options.t("status.loadingConfig"));
    try {
      const cfg = await invokeTauri<AppConfig>("load_config");
      options.config.hotkey = cfg.hotkey;
      options.config.uiLanguage = options.normalizeLocale(cfg.uiLanguage);
      options.locale.value = options.config.uiLanguage;
      options.config.recordHotkey = cfg.recordHotkey || "Alt";
      options.config.minRecordSeconds = Math.max(1, Math.min(30, Number(cfg.minRecordSeconds || 1)));
      options.config.maxRecordSeconds = Math.max(options.config.minRecordSeconds, Number(cfg.maxRecordSeconds || 60));
      options.config.toolMaxIterations = Math.max(1, Math.min(100, Number(cfg.toolMaxIterations || 10)));
      options.config.selectedApiConfigId = cfg.selectedApiConfigId;
      options.config.chatApiConfigId = cfg.chatApiConfigId;
      options.config.visionApiConfigId = cfg.visionApiConfigId ?? undefined;
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
      options.locale.value = options.config.uiLanguage;
      options.config.recordHotkey = saved.recordHotkey || "Alt";
      options.config.minRecordSeconds = Math.max(1, Math.min(30, Number(saved.minRecordSeconds || 1)));
      options.config.maxRecordSeconds = Math.max(options.config.minRecordSeconds, Number(saved.maxRecordSeconds || 60));
      options.config.toolMaxIterations = Math.max(1, Math.min(100, Number(saved.toolMaxIterations || 10)));
      options.config.selectedApiConfigId = saved.selectedApiConfigId;
      options.config.chatApiConfigId = saved.chatApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.config.apiConfigs.splice(0, options.config.apiConfigs.length, ...saved.apiConfigs);
      options.normalizeApiBindingsLocal();
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      console.info("[CONFIG] save_config success");
      options.setStatus(options.t("status.configSaved"));
    } catch (e) {
      const err = String(e);
      console.error("[CONFIG] save_config failed:", e);
      if (err.includes("404")) {
        options.setStatus(options.t("status.saveConfigBackend404"));
      } else {
        options.setStatus(options.t("status.saveConfigFailed", { err }));
      }
    } finally {
      options.suppressAutosave.value = false;
      options.saving.value = false;
    }
  }

  async function captureHotkey(value: string) {
    const hotkey = String(value || "").trim();
    if (!hotkey) return;
    options.config.hotkey = hotkey;
    await saveConfig();
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
      }>("save_conversation_api_settings", {
        input: {
          chatApiConfigId: options.config.chatApiConfigId,
          visionApiConfigId: options.config.visionApiConfigId || null,
        },
      });
      options.config.chatApiConfigId = saved.chatApiConfigId;
      options.config.visionApiConfigId = saved.visionApiConfigId ?? undefined;
      options.lastSavedConfigJson.value = options.buildConfigSnapshotJson();
      console.info("[CONFIG] save_conversation_api_settings success");
    } catch (e) {
      console.error("[CONFIG] save_conversation_api_settings failed:", e);
      options.setStatusError("status.saveConversationApiFailed", e);
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
