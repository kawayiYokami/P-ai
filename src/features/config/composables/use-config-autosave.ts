import type { Ref } from "vue";

type UseConfigAutosaveOptions = {
  suppressAutosave: Ref<boolean>;
  configAutosaveReady: Ref<boolean>;
  personasAutosaveReady: Ref<boolean>;
  chatSettingsAutosaveReady: Ref<boolean>;
  saveConfig: () => Promise<boolean>;
  savePersonas: () => Promise<void>;
  saveChatPreferences: () => Promise<void>;
};

export function useConfigAutosave(options: UseConfigAutosaveOptions) {
  let configAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let personasAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let chatSettingsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleConfigAutosave() {
    if (options.suppressAutosave.value) return;
    if (!options.configAutosaveReady.value) return;
    if (configAutosaveTimer) clearTimeout(configAutosaveTimer);
    configAutosaveTimer = setTimeout(() => {
      void options.saveConfig();
    }, 350);
  }

  function schedulePersonasAutosave() {
    if (options.suppressAutosave.value) return;
    if (!options.personasAutosaveReady.value) return;
    if (personasAutosaveTimer) clearTimeout(personasAutosaveTimer);
    personasAutosaveTimer = setTimeout(() => {
      void options.savePersonas();
    }, 350);
  }

  function scheduleChatSettingsAutosave() {
    if (options.suppressAutosave.value) return;
    if (!options.chatSettingsAutosaveReady.value) return;
    if (chatSettingsAutosaveTimer) clearTimeout(chatSettingsAutosaveTimer);
    chatSettingsAutosaveTimer = setTimeout(() => {
      void options.saveChatPreferences();
    }, 350);
  }

  function disposeAutosaveTimers() {
    if (configAutosaveTimer) {
      clearTimeout(configAutosaveTimer);
      configAutosaveTimer = null;
    }
    if (personasAutosaveTimer) {
      clearTimeout(personasAutosaveTimer);
      personasAutosaveTimer = null;
    }
    if (chatSettingsAutosaveTimer) {
      clearTimeout(chatSettingsAutosaveTimer);
      chatSettingsAutosaveTimer = null;
    }
  }

  return {
    scheduleConfigAutosave,
    schedulePersonasAutosave,
    scheduleChatSettingsAutosave,
    disposeAutosaveTimers,
  };
}

