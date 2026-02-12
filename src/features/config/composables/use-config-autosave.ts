import type { Ref } from "vue";

type UseConfigAutosaveOptions = {
  suppressAutosave: Ref<boolean>;
  personasAutosaveReady: Ref<boolean>;
  chatSettingsAutosaveReady: Ref<boolean>;
  savePersonas: () => Promise<void>;
  saveChatPreferences: () => Promise<void>;
};

export function useConfigAutosave(options: UseConfigAutosaveOptions) {
  let personasAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
  let chatSettingsAutosaveTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleConfigAutosave() {
    // API 配置改为手动保存，保留函数占位避免大范围改动。
    return;
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


