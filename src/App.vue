
<template>
  <div class="window-shell text-sm bg-base-200">
    <AppWindowHeader
      :view-mode="viewMode"
      :title-text="titleText"
      :chat-usage-percent="chatUsagePercent"
      :forcing-archive="forcingArchive"
      :chatting="chatting"
      :always-on-top="alwaysOnTop"
      :window-ready="windowReady"
      :force-archive-tip="t('chat.forceArchiveTip')"
      :always-on-top-on-title="t('chat.alwaysOnTopOn')"
      :always-on-top-off-title="t('chat.alwaysOnTopOff')"
      @start-drag="startDrag"
      @force-archive="forceArchiveNow"
      @toggle-always-on-top="toggleAlwaysOnTop"
      @close-window="closeWindow"
    />

    <AppWindowContent
      :t="tr"
      :view-mode="viewMode"
      :config="config"
      :config-tab="configTab"
      :locale-options="localeOptions"
      :current-theme="currentTheme"
      :selected-api-config="selectedApiConfig"
      :tool-api-config="toolApiConfig"
      :base-url-reference="baseUrlReference"
      :refreshing-models="refreshingModels"
      :selected-model-options="selectedModelOptions"
      :model-refresh-ok="selectedModelRefreshOk"
      :model-refresh-error="modelRefreshError"
      :tool-statuses="toolStatuses"
      :personas="personas"
      :assistant-personas="assistantPersonas"
      :user-persona="userPersona"
      :persona-editor-id="personaEditorId"
      :selected-persona-id="selectedPersonaId"
      :selected-persona-editor="selectedPersonaEditor"
      :selected-persona-editor-avatar-url="selectedPersonaEditorAvatarUrl"
      :user-persona-avatar-url="userPersonaAvatarUrl"
      :response-style-options="responseStyleOptions"
      :selected-response-style-id="selectedResponseStyleId"
      :text-capable-api-configs="textCapableApiConfigs"
      :image-capable-api-configs="imageCapableApiConfigs"
      :image-cache-stats="imageCacheStats"
      :image-cache-stats-loading="imageCacheStatsLoading"
      :avatar-saving="avatarSaving"
      :avatar-error="avatarError"
      :config-dirty="configDirty"
      :saving="saving"
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio="hotkeyTestAudio"
      :user-alias="userAlias"
      :selected-persona-name="selectedPersona?.name || t('archives.roleAssistant')"
      :user-avatar-url="userAvatarUrl"
      :selected-persona-avatar-url="selectedPersonaAvatarUrl"
      :latest-user-text="latestUserText"
      :latest-user-images="latestUserImages"
      :latest-assistant-text="latestAssistantText"
      :latest-reasoning-standard-text="latestReasoningStandardText"
      :latest-reasoning-inline-text="latestReasoningInlineText"
      :tool-status-text="toolStatusText"
      :tool-status-state="toolStatusState"
      :chat-error-text="chatErrorText"
      :clipboard-images="clipboardImages"
      :chat-input="chatInput"
      :chat-input-placeholder="chatInputPlaceholder"
      :speech-recognition-supported="speechRecognitionSupported"
      :recording="recording"
      :recording-ms="recordingMs"
      :record-hotkey="config.recordHotkey"
      :chatting="chatting"
      :forcing-archive="forcingArchive"
      :visible-turns="visibleTurns"
      :has-more-turns="hasMoreTurns"
      :archives="archives"
      :selected-archive-id="selectedArchiveId"
      :archive-messages="archiveMessages"
      :render-message="renderMessage"
      :current-history="currentHistory"
      :message-text="messageText"
      :extract-message-images="extractMessageImages"
      :memory-import-input="memoryImportInput"
      :memory-list="memoryList"
      :memory-page="memoryPage"
      :memory-page-count="memoryPageCount"
      :paged-memories="pagedMemories"
      :prompt-preview-mode="promptPreviewMode"
      :prompt-preview-loading="promptPreviewLoading"
      :prompt-preview-text="promptPreviewText"
      :prompt-preview-latest-user-text="promptPreviewLatestUserText"
      :prompt-preview-latest-images="promptPreviewLatestImages"
      :prompt-preview-latest-audios="promptPreviewLatestAudios"
      :set-history-dialog-ref="setHistoryDialogRef"
      :set-memory-dialog-ref="setMemoryDialogRef"
      :set-prompt-preview-dialog-ref="setPromptPreviewDialogRef"
      :update-config-tab="(value) => { configTab = value; }"
      :set-ui-language="setUiLanguage"
      :update-persona-editor-id="(value) => { personaEditorId = value; }"
      :update-selected-persona-id="(value) => { selectedPersonaId = value; }"
      :update-selected-response-style-id="(value) => { selectedResponseStyleId = value; }"
      :toggle-theme="toggleTheme"
      :refresh-models="refreshModels"
      :on-tools-changed="handleToolsChanged"
      :save-config="saveConfig"
      :add-api-config="addApiConfig"
      :remove-selected-api-config="removeSelectedApiConfig"
      :add-persona="addPersona"
      :remove-selected-persona="removeSelectedPersona"
      :open-current-history="openCurrentHistory"
      :open-prompt-preview="openPromptPreview"
      :open-system-prompt-preview="openSystemPromptPreview"
      :open-memory-viewer="openMemoryViewer"
      :refresh-image-cache-stats="refreshImageCacheStats"
      :clear-image-cache="clearImageCache"
      :start-hotkey-record-test="startHotkeyRecordTest"
      :stop-hotkey-record-test="stopHotkeyRecordTest"
      :play-hotkey-record-test="playHotkeyRecordTest"
      :capture-hotkey="captureHotkey"
      :save-agent-avatar="saveAgentAvatar"
      :clear-agent-avatar="clearAgentAvatar"
      :update-chat-input="(value) => { chatInput = value; }"
      :remove-clipboard-image="removeClipboardImage"
      :start-recording="startRecording"
      :stop-recording="() => stopRecording(false)"
      :send-chat="chatFlow.sendChat"
      :stop-chat="chatFlow.stopChat"
      :load-more-turns="loadMoreTurns"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :export-archive="exportArchive"
      :delete-archive="deleteArchive"
      :close-history="closeHistory"
      :close-memory-viewer="closeMemoryViewer"
      :prev-memory-page="() => { memoryPage--; }"
      :next-memory-page="() => { memoryPage++; }"
      :export-memories="exportMemories"
      :trigger-memory-import="triggerMemoryImport"
      :handle-memory-import-file="handleMemoryImportFile"
      :close-prompt-preview="closePromptPreview"
    />
  </div>
</template>
<script setup lang="ts">
import { computed, reactive, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "./services/tauri-api";
import { useAppBootstrap } from "./features/shell/composables/use-app-bootstrap";
import { useAppCore } from "./features/shell/composables/use-app-core";
import { useAppLifecycle } from "./features/shell/composables/use-app-lifecycle";
import { useAppTheme } from "./features/shell/composables/use-app-theme";
import { useViewRefresh } from "./features/shell/composables/use-view-refresh";
import { useWindowShell } from "./features/shell/composables/use-window-shell";
import { useConfigAutosave } from "./features/config/composables/use-config-autosave";
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useConfigPersistence } from "./features/config/composables/use-config-persistence";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useArchivesView } from "./features/chat/composables/use-archives-view";
import { useAvatarCache } from "./features/chat/composables/use-avatar-cache";
import { useChatDialogActions } from "./features/chat/composables/use-chat-dialog-actions";
import { useChatRuntime } from "./features/chat/composables/use-chat-runtime";
import { useChatTurns } from "./features/chat/composables/use-chat-turns";
import { useChatMedia } from "./features/chat/composables/use-chat-media";
import { useHistoryViewer } from "./features/chat/composables/use-history-viewer";
import { usePromptPreview } from "./features/chat/composables/use-prompt-preview";
import { useMemoryViewer } from "./features/memory/composables/use-memory-viewer";
import { useAppWatchers } from "./features/shell/composables/use-app-watchers";
import { useRecordHotkey } from "./features/chat/composables/use-record-hotkey";
import { useSpeechRecording } from "./features/chat/composables/use-speech-recording";
import { useChatFlow } from "./features/chat/composables/use-chat-flow";
import {
  extractMessageImages,
  messageText,
  removeBinaryPlaceholders,
  renderMessage,
} from "./utils/chat-message";
import { formatI18nError } from "./utils/error";
import AppWindowContent from "./features/shell/components/AppWindowContent.vue";
import AppWindowHeader from "./features/shell/components/AppWindowHeader.vue";
import type {
  PersonaProfile,
  ApiConfigItem,
  AppConfig,
  ChatMessage,
  ImageTextCacheStats,
  ResponseStyleOption,
  ToolLoadStatus,
} from "./types/app";
import responseStylesJson from "./constants/response-styles.json";
import { normalizeLocale } from "./i18n";

const viewMode = ref<"chat" | "archives" | "config">("config");
const { t, locale } = useI18n();
const tr = (key: string, params?: Record<string, unknown>) => (params ? t(key, params) : t(key));
const { windowReady, alwaysOnTop, initWindow, syncAlwaysOnTop, closeWindow, startDrag, toggleAlwaysOnTop } =
  useWindowShell();
const { currentTheme, applyTheme, restoreThemeFromStorage, toggleTheme } = useAppTheme();

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  uiLanguage: "zh-CN",
  recordHotkey: "Alt",
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  toolMaxIterations: 10,
  selectedApiConfigId: "",
  chatApiConfigId: "",
  visionApiConfigId: undefined,
  apiConfigs: [],
});
const configTab = ref<"hotkey" | "api" | "tools" | "persona" | "chatSettings">("hotkey");
const personas = ref<PersonaProfile[]>([]);
const selectedPersonaId = ref("default-agent");
const personaEditorId = ref("default-agent");
const userAlias = ref(t("archives.roleUser"));
const selectedResponseStyleId = ref("concise");
const chatInput = ref("");
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const latestReasoningStandardText = ref("");
const latestReasoningInlineText = ref("");
const toolStatusText = ref("");
const toolStatusState = ref<"running" | "done" | "failed" | "">("");
const chatErrorText = ref("");
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);

const allMessages = shallowRef<ChatMessage[]>([]);
const visibleTurnCount = ref(1);

const status = ref("Ready.");
const loading = ref(false);
const saving = ref(false);
const chatting = ref(false);
const forcingArchive = ref(false);
const refreshingModels = ref(false);
const modelRefreshError = ref("");
const modelRefreshOkFlags = ref<Record<string, boolean>>({});
const checkingToolsStatus = ref(false);
const toolStatuses = ref<ToolLoadStatus[]>([]);
const imageCacheStats = ref<ImageTextCacheStats>({ entries: 0, totalChars: 0 });
const imageCacheStatsLoading = ref(false);
const avatarSaving = ref(false);
const avatarError = ref("");
const apiModelOptions = ref<Record<string, string[]>>({});
const configAutosaveReady = ref(false);
const personasAutosaveReady = ref(false);
const chatSettingsAutosaveReady = ref(false);
const suppressAutosave = ref(false);
const RECORD_HOTKEY_SUPPRESS_AFTER_POPUP_MS = 700;
const lastSavedConfigJson = ref("");
const PERF_DEBUG = true;
const { perfNow, perfLog, setStatus, setStatusError, localeOptions, applyUiLanguage } = useAppCore({
  t: tr,
  config,
  locale,
  status,
  perfDebug: PERF_DEBUG,
});

const {
  historyDialog,
  currentHistory,
  openCurrentHistory: openCurrentHistoryDialog,
  closeHistory,
} = useHistoryViewer({
  setStatusError,
});

const {
  promptPreviewDialog,
  promptPreviewLoading,
  promptPreviewText,
  promptPreviewLatestUserText,
  promptPreviewLatestImages,
  promptPreviewLatestAudios,
  promptPreviewMode,
  openPromptPreview: openPromptPreviewDialog,
  openSystemPromptPreview: openSystemPromptPreviewDialog,
  closePromptPreview,
} = usePromptPreview({
  t: tr,
  beforePreview: async () => {
    await savePersonas();
    await saveChatPreferences();
    await saveConversationApiSettings();
  },
});

const {
  archives,
  archiveMessages,
  selectedArchiveId,
  loadArchives,
  selectArchive,
  deleteArchive,
  exportArchive,
} = useArchivesView({
  t: tr,
  setStatus,
  setStatusError,
});

const {
  memoryDialog,
  memoryImportInput,
  memoryList,
  memoryPage,
  memoryPageCount,
  pagedMemories,
  openMemoryViewer,
  closeMemoryViewer,
  exportMemories,
  triggerMemoryImport,
  handleMemoryImportFile,
} = useMemoryViewer({
  t: tr,
  setStatus,
  setStatusError,
});

const titleText = computed(() => {
  if (viewMode.value === "chat") {
    return t("window.chatTitle", { name: selectedPersona.value?.name || t("archives.roleAssistant") });
  }
  if (viewMode.value === "archives") {
    return t("window.archivesTitle");
  }
  return t("window.configTitle");
});
const selectedApiConfig = computed(() => config.apiConfigs.find((a) => a.id === config.selectedApiConfigId) ?? null);
const textCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableText));
const imageCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableImage));
const activeChatApiConfigId = computed(
  () => config.chatApiConfigId || textCapableApiConfigs.value[0]?.id || config.apiConfigs[0]?.id || "",
);
const activeChatApiConfig = computed(
  () => config.apiConfigs.find((a) => a.id === activeChatApiConfigId.value) ?? null,
);
const toolApiConfig = computed(() => activeChatApiConfig.value);
const hasVisionFallback = computed(() =>
  !!config.visionApiConfigId
  && config.apiConfigs.some((a) => a.id === config.visionApiConfigId && a.enableImage),
);
const {
  supported: speechRecognitionSupported,
  recording,
  recordingMs,
  startRecording,
  stopRecording,
  cleanup: cleanupSpeechRecording,
} = useSpeechRecording({
  t: tr,
  canStart: () => !chatting.value && !forcingArchive.value,
  getLanguage: () => normalizeLocale(config.uiLanguage),
  getMaxRecordSeconds: () => config.maxRecordSeconds,
  appendRecognizedText: (text) => {
    chatInput.value = chatInput.value.trim() ? `${chatInput.value.trim()}\n${text}` : text;
  },
  setStatus: (text) => {
    status.value = text;
  },
});
const chatMedia = useChatMedia({
  t: tr,
  setStatus: (text) => {
    status.value = text;
  },
  setStatusError,
  viewMode,
  chatting,
  forcingArchive,
  isRecording: () => recording.value,
  activeChatApiConfig,
  hasVisionFallback,
  chatInput,
  clipboardImages,
});
const hotkeyTestRecording = chatMedia.hotkeyTestRecording;
const hotkeyTestRecordingMs = chatMedia.hotkeyTestRecordingMs;
const hotkeyTestAudio = chatMedia.hotkeyTestAudio;
const onPaste = chatMedia.onPaste;
const removeClipboardImage = chatMedia.removeClipboardImage;
const startHotkeyRecordTest = chatMedia.startHotkeyRecordTest;
const stopHotkeyRecordTest = chatMedia.stopHotkeyRecordTest;
const playHotkeyRecordTest = chatMedia.playHotkeyRecordTest;
const cleanupChatMedia = chatMedia.cleanupChatMedia;
const recordHotkey = useRecordHotkey({
  isActive: () => viewMode.value === "chat",
  getSummonHotkey: () => config.hotkey,
  getRecordHotkey: () => config.recordHotkey,
  onConflict: () => {
    status.value = t("config.hotkey.conflict");
  },
  onStartRecording: () => startRecording(),
  onStopRecording: (discard) => stopRecording(discard),
});
const userPersona = computed(
  () => personas.value.find((p) => p.isBuiltInUser || p.id === "user-persona") ?? null,
);
const assistantPersonas = computed(() =>
  personas.value.filter((p) => !p.isBuiltInUser && p.id !== "user-persona"),
);
const selectedPersona = computed(
  () =>
    assistantPersonas.value.find((p) => p.id === selectedPersonaId.value)
    ?? assistantPersonas.value[0]
    ?? null,
);
const selectedPersonaEditor = computed(
  () => personas.value.find((p) => p.id === personaEditorId.value) ?? null,
);
const userAvatarUrl = computed(
  () => resolveAvatarUrl(userPersona.value?.avatarPath, userPersona.value?.avatarUpdatedAt),
);
const userPersonaAvatarUrl = computed(() => userAvatarUrl.value);
const selectedPersonaAvatarUrl = computed(
  () => resolveAvatarUrl(selectedPersona.value?.avatarPath, selectedPersona.value?.avatarUpdatedAt),
);
const selectedPersonaEditorAvatarUrl = computed(
  () => resolveAvatarUrl(selectedPersonaEditor.value?.avatarPath, selectedPersonaEditor.value?.avatarUpdatedAt),
);
const selectedModelOptions = computed(() => {
  const id = config.selectedApiConfigId;
  if (!id) return [];
  return apiModelOptions.value[id] ?? [];
});
const selectedModelRefreshOk = computed(() => {
  const id = config.selectedApiConfigId;
  if (!id) return false;
  return !!modelRefreshOkFlags.value[id];
});
const responseStyleOptions = responseStylesJson as ResponseStyleOption[];
const baseUrlReference = computed(() => {
  const format = selectedApiConfig.value?.requestFormat ?? "openai";
  if (format === "gemini") return "https://generativelanguage.googleapis.com";
  if (format === "deepseek/kimi") return "https://api.deepseek.com/v1";
  if (format === "anthropic") return "https://api.anthropic.com";
  return "https://api.openai.com/v1";
});
const chatInputPlaceholder = computed(() => {
  const api = activeChatApiConfig.value;
  if (!api) return t("chat.placeholder");
  const hints: string[] = [];
  if (api.enableImage || hasVisionFallback.value) hints.push("Ctrl+V");
  if (hints.length === 0) return t("chat.placeholder");
  return t("chat.placeholder");
});
let toolSwitchAutosaveTimer: ReturnType<typeof setTimeout> | null = null;
const {
  defaultApiTools,
  createApiConfig,
  normalizeApiBindingsLocal,
  buildConfigPayload,
  buildConfigSnapshotJson,
} = useConfigCore({
  config,
  textCapableApiConfigs,
});
const { resolveAvatarUrl, ensureAvatarCached, preloadPersonaAvatars } = useAvatarCache({
  personas,
});
const configDirty = computed(() => buildConfigSnapshotJson() !== lastSavedConfigJson.value);
const responseStyleIds = computed(() => responseStyleOptions.map((item) => item.id));
const { visibleTurns, hasMoreTurns, chatContextUsageRatio, chatUsagePercent } = useChatTurns({
  allMessages,
  visibleTurnCount,
  activeChatApiConfig,
  perfDebug: PERF_DEBUG,
  perfNow,
});

function syncUserAliasFromPersona() {
  const next = (userPersona.value?.name || "").trim() || t("archives.roleUser");
  if (userAlias.value !== next) {
    userAlias.value = next;
  }
}

const {
  syncTrayIcon,
  saveAgentAvatar,
  clearAgentAvatar,
  refreshModels,
  refreshToolsStatus,
  refreshImageCacheStats,
  clearImageCache,
} = useConfigRuntime({
  t: tr,
  setStatus,
  setStatusError,
  personas,
  selectedPersonaId,
  avatarSaving,
  avatarError,
  selectedApiConfig,
  refreshingModels,
  modelRefreshError,
  apiModelOptions,
  modelRefreshOkFlags,
  toolApiConfig,
  checkingToolsStatus,
  toolStatuses,
  imageCacheStats,
  imageCacheStatsLoading,
  ensureAvatarCached,
});
const configPersistence = useConfigPersistence({
  t: tr,
  setStatus,
  setStatusError,
  config,
  locale,
  normalizeLocale,
  suppressAutosave,
  loading,
  saving,
  personas,
  assistantPersonas,
  selectedPersonaId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  responseStyleIds,
  createApiConfig,
  normalizeApiBindingsLocal,
  buildConfigPayload,
  buildConfigSnapshotJson,
  lastSavedConfigJson,
  syncUserAliasFromPersona,
  preloadPersonaAvatars,
  syncTrayIcon,
});
const {
  loadConfig,
  saveConfig,
  captureHotkey,
  loadPersonas,
  loadChatSettings,
  savePersonas,
  saveChatPreferences,
  saveConversationApiSettings,
} = configPersistence;
const chatRuntime = useChatRuntime({
  t: tr,
  setStatus,
  setStatusError,
  activeChatApiConfigId,
  selectedPersonaId,
  chatting,
  forcingArchive,
  latestUserText,
  latestUserImages,
  latestAssistantText,
  allMessages,
  visibleTurnCount,
  perfNow,
  perfLog,
  perfDebug: PERF_DEBUG,
});
const {
  refreshChatSnapshot,
  forceArchiveNow,
  loadAllMessages,
  loadMoreTurns,
} = chatRuntime;

const {
  scheduleConfigAutosave,
  schedulePersonasAutosave,
  scheduleChatSettingsAutosave,
  disposeAutosaveTimers,
} = useConfigAutosave({
  suppressAutosave,
  personasAutosaveReady,
  chatSettingsAutosaveReady,
  savePersonas,
  saveChatPreferences,
});

const {
  addApiConfig,
  removeSelectedApiConfig,
  addPersona,
  removeSelectedPersona,
} = useConfigEditors({
  t: tr,
  config,
  personas,
  assistantPersonas,
  selectedPersonaId,
  personaEditorId,
  selectedPersonaEditor,
  createApiConfig,
  normalizeApiBindingsLocal,
});

const { suppressChatReloadWatch, refreshAllViewData, handleWindowRefreshSignal } = useViewRefresh({
  viewMode,
  recordHotkeySuppressAfterPopup: recordHotkey.suppressAfterPopup,
  recordHotkeySuppressMs: RECORD_HOTKEY_SUPPRESS_AFTER_POPUP_MS,
  configAutosaveReady,
  personasAutosaveReady,
  chatSettingsAutosaveReady,
  loadConfig,
  loadPersonas,
  loadChatSettings,
  refreshImageCacheStats,
  refreshChatSnapshot,
  loadAllMessages,
  loadArchives,
  resetVisibleTurnCount: () => {
    visibleTurnCount.value = 1;
  },
  perfNow,
  perfLog,
});

const appBootstrap = useAppBootstrap({
  setViewMode: (mode) => {
    viewMode.value = mode;
  },
  initWindowMode: () => initWindow(),
  onThemeChanged: (theme) => {
    if (theme === "light" || theme === "forest") {
      applyTheme(theme);
    }
  },
  onLocaleChanged: (payload) => {
    const lang = normalizeLocale(payload);
    config.uiLanguage = lang;
    locale.value = lang;
  },
  onRefreshSignal: handleWindowRefreshSignal,
});

function setUiLanguage(value: string) {
  if (!applyUiLanguage(value)) return;
  void saveConfig();
}

const chatFlow = useChatFlow({
  chatting,
  forcingArchive,
  chatInput,
  clipboardImages,
  latestUserText,
  latestUserImages,
  latestAssistantText,
  latestReasoningStandardText,
  latestReasoningInlineText,
  toolStatusText,
  toolStatusState,
  chatErrorText,
  allMessages,
  visibleTurnCount,
  t: tr,
  formatRequestFailed: (error) => formatI18nError(tr, "status.requestFailed", error),
  removeBinaryPlaceholders,
  invokeSendChatMessage: ({ text, images, onDelta }) =>
    invokeTauri("send_chat_message", {
      input: {
        payload: { text, images },
      },
      onDelta,
    }),
  onReloadMessages: () => loadAllMessages(),
});

function clearStreamBuffer() {
  chatFlow.clearStreamBuffer();
}

function handleToolsChanged() {
  if (toolSwitchAutosaveTimer) {
    clearTimeout(toolSwitchAutosaveTimer);
  }
  toolSwitchAutosaveTimer = setTimeout(async () => {
    await saveConfig();
    if (configTab.value === "tools") {
      await refreshToolsStatus();
    }
  }, 250);
}
const { openCurrentHistory, openPromptPreview, openSystemPromptPreview } = useChatDialogActions({
  activeChatApiConfigId,
  selectedPersonaId,
  openCurrentHistoryDialog,
  openPromptPreviewDialog,
  openSystemPromptPreviewDialog,
});

function setHistoryDialogRef(el: Element | null) {
  historyDialog.value = (el as HTMLDialogElement | null) ?? null;
}

function setMemoryDialogRef(el: Element | null) {
  memoryDialog.value = (el as HTMLDialogElement | null) ?? null;
}

function setPromptPreviewDialogRef(el: Element | null) {
  promptPreviewDialog.value = (el as HTMLDialogElement | null) ?? null;
}

useAppLifecycle({
  appBootstrapMount: appBootstrap.mount,
  appBootstrapUnmount: appBootstrap.unmount,
  restoreThemeFromStorage,
  onPaste,
  recordHotkeyMount: recordHotkey.mount,
  recordHotkeyUnmount: recordHotkey.unmount,
  refreshAllViewData,
  configAutosaveReady,
  personasAutosaveReady,
  chatSettingsAutosaveReady,
  viewMode,
  syncAlwaysOnTop,
  disposeAutosaveTimers,
  clearStreamBuffer,
  stopRecording,
  cleanupSpeechRecording,
  cleanupChatMedia,
});

useAppWatchers({
  config,
  configTab,
  viewMode,
  personas,
  userPersona,
  assistantPersonas,
  selectedPersonaId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  selectedApiConfig,
  toolApiConfig,
  activeChatApiConfigId,
  suppressChatReloadWatch,
  modelRefreshError,
  toolStatuses,
  defaultApiTools,
  t: tr,
  schedulePersonasAutosave,
  scheduleChatSettingsAutosave,
  normalizeApiBindingsLocal,
  syncUserAliasFromPersona,
  syncTrayIcon,
  saveConversationApiSettings,
  refreshToolsStatus,
  refreshImageCacheStats,
  refreshChatSnapshot,
  loadAllMessages,
  resetVisibleTurnCount: () => {
    visibleTurnCount.value = 1;
  },
});
</script>
