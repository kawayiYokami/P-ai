
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
      :open-config-title="t('window.configTitle')"
      :close-title="t('common.close')"
      @start-drag="startDrag"
      @force-archive="forceArchiveNow"
      @toggle-always-on-top="toggleAlwaysOnTop"
      @open-config="openConfigWindow"
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
      :stt-capable-api-configs="sttCapableApiConfigs"
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
      :current-chat-workspace-name="chatWorkspaceName"
      :chat-workspace-locked="chatWorkspaceLocked"
      :user-avatar-url="userAvatarUrl"
      :selected-persona-avatar-url="selectedPersonaAvatarUrl"
      :chat-persona-name-map="chatPersonaNameMap"
      :chat-persona-avatar-url-map="chatPersonaAvatarUrlMap"
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
      :transcribing="transcribing"
      :record-hotkey="config.recordHotkey"
      :media-drag-active="mediaDragActive"
      :chatting="chatting"
      :forcing-archive="forcingArchive"
      :visible-turns="displayTurns"
      :has-more-turns="displayHasMoreTurns"
      :archives="archives"
      :selected-archive-id="selectedArchiveId"
      :archive-messages="archiveMessages"
      :archive-summary-text="archiveSummaryText"
      :unarchived-conversations="unarchivedConversations"
      :selected-unarchived-conversation-id="selectedUnarchivedConversationId"
      :unarchived-messages="unarchivedMessages"
      :current-history="currentHistory"
      :message-text="messageText"
      :extract-message-images="extractMessageImages"
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
      :set-theme="setTheme"
      :refresh-models="refreshModels"
      :on-tools-changed="handleToolsChanged"
      :save-config="saveConfig"
      :add-api-config="addApiConfig"
      :remove-selected-api-config="removeSelectedApiConfig"
      :add-persona="addPersona"
      :remove-selected-persona="removeSelectedPersona"
      :import-persona-memories="importPersonaMemories"
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
      :summon-chat-now="summonChatWindowFromConfig"
      :save-agent-avatar="saveAgentAvatar"
      :clear-agent-avatar="clearAgentAvatar"
      :update-chat-input="(value) => { chatInput = value; }"
      :remove-clipboard-image="removeClipboardImage"
      :start-recording="startRecording"
      :stop-recording="() => stopRecording(false)"
      :send-chat="chatFlow.sendChat"
      :stop-chat="chatFlow.stopChat"
      :load-more-turns="loadMoreTurns"
      :on-recall-turn="handleRecallTurn"
      :on-regenerate-turn="handleRegenerateTurn"
      :on-lock-chat-workspace="lockChatWorkspaceFromPicker"
      :on-unlock-chat-workspace="unlockChatWorkspace"
      :on-open-skill-panel="openSkillPlaceholderDialog"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :select-unarchived-conversation="selectUnarchivedConversation"
      :export-archive="exportArchive"
      :import-archive-file="prepareArchiveImport"
      :delete-archive="deleteArchive"
      :delete-unarchived-conversation="deleteUnarchivedConversation"
      :close-history="closeHistory"
      :close-memory-viewer="closeMemoryViewer"
      :prev-memory-page="() => { memoryPage--; }"
      :next-memory-page="() => { memoryPage++; }"
      :export-memories="exportMemories"
      :trigger-memory-import="triggerMemoryImport"
      :handle-memory-import-file="handleMemoryImportFile"
      :close-prompt-preview="closePromptPreview"
      :checking-update="checkingUpdate"
      :check-update="manualCheckGithubUpdate"
      :open-github="openGithubRepository"
    />

    <dialog class="modal" :class="{ 'modal-open': updateDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">
          {{ updateDialogTitle }}
        </h3>
        <pre
          class="mt-2 whitespace-pre-wrap text-sm"
          :class="updateDialogKind === 'error' ? 'text-error' : 'text-base-content'"
        >{{ updateDialogBody }}</pre>
        <div class="modal-action">
          <button
            v-if="updateDialogReleaseUrl"
            class="btn btn-sm"
            @click="openUpdateRelease"
          >
            打开 Releases
          </button>
          <button class="btn btn-sm btn-primary" @click="closeUpdateDialog">知道了</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeUpdateDialog">close</button>
      </form>
    </dialog>
    <dialog class="modal" :class="{ 'modal-open': configSaveErrorDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">
          {{ configSaveErrorDialogTitle }}
        </h3>
        <pre
          class="mt-2 whitespace-pre-wrap text-sm"
          :class="configSaveErrorDialogKind === 'warning' ? 'text-warning' : 'text-error'"
        >{{ configSaveErrorDialogBody }}</pre>
        <div class="modal-action">
          <button class="btn btn-sm btn-primary" @click="closeConfigSaveErrorDialog">{{ t("common.close") }}</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeConfigSaveErrorDialog">close</button>
      </form>
    </dialog>
    <dialog class="modal" :class="{ 'modal-open': terminalApprovalDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">
          {{ terminalApprovalDialogTitle }}
        </h3>
        <pre class="mt-2 whitespace-pre-wrap text-sm text-base-content">{{ terminalApprovalDialogBody }}</pre>
        <div v-if="terminalApprovalQueue.length > 1" class="text-sm opacity-70 mt-2">
          {{ t("status.terminalApprovalQueueHint", { count: terminalApprovalQueue.length }) }}
        </div>
        <div class="modal-action">
          <button
            class="btn btn-sm"
            :disabled="terminalApprovalResolving"
            @click="denyTerminalApproval"
          >
            {{ t("common.cancel") }}
          </button>
          <button
            class="btn btn-sm btn-primary"
            :disabled="terminalApprovalResolving"
            @click="approveTerminalApproval"
          >
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="denyTerminalApproval">close</button>
      </form>
    </dialog>
    <dialog class="modal" :class="{ 'modal-open': archiveImportPreviewDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">
          {{ t("archives.importPreviewTitle") }}
        </h3>
        <div v-if="archiveImportPreview" class="mt-3 space-y-1 text-sm">
          <div>{{ t("archives.importPreviewFile", { name: archiveImportPreview.fileName }) }}</div>
          <div>{{ t("archives.importPreviewTotal", { count: archiveImportPreview.total }) }}</div>
          <div>{{ t("archives.importPreviewAdd", { count: archiveImportPreview.imported }) }}</div>
          <div>{{ t("archives.importPreviewReplace", { count: archiveImportPreview.replaced }) }}</div>
          <div class="text-sm opacity-70 mt-2">{{ t("archives.importPreviewHint") }}</div>
        </div>
        <div class="modal-action">
          <button class="btn btn-sm" :disabled="archiveImportRunning" @click="closeArchiveImportPreviewDialog">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-primary" :disabled="archiveImportRunning" @click="confirmArchiveImport">
            {{ archiveImportRunning ? t("common.loading") : t("archives.importConfirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeArchiveImportPreviewDialog">close</button>
      </form>
    </dialog>
    <dialog class="modal" :class="{ 'modal-open': skillPlaceholderDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">Skill 列表</h3>
        <div class="mt-2 text-sm opacity-80">预留功能，暂未实现。</div>
        <div class="modal-action">
          <button class="btn btn-sm btn-primary" @click="closeSkillPlaceholderDialog">{{ t("common.close") }}</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeSkillPlaceholderDialog">close</button>
      </form>
    </dialog>
  </div>
</template>
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeTauri } from "./services/tauri-api";
import { useAppBootstrap, type TerminalApprovalRequestPayload } from "./features/shell/composables/use-app-bootstrap";
import { useAppCore } from "./features/shell/composables/use-app-core";
import { useAppLifecycle } from "./features/shell/composables/use-app-lifecycle";
import { useAppTheme } from "./features/shell/composables/use-app-theme";
import { useViewRefresh } from "./features/shell/composables/use-view-refresh";
import { useWindowShell } from "./features/shell/composables/use-window-shell";
import { useConfigAutosave } from "./features/config/composables/use-config-autosave";
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useConfigPersistence, type ConfigSaveErrorInfo } from "./features/config/composables/use-config-persistence";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useArchivesView, type ArchiveImportPreview } from "./features/chat/composables/use-archives-view";
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
const { currentTheme, applyTheme, setTheme, restoreThemeFromStorage } = useAppTheme();

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  uiLanguage: "zh-CN",
  uiFont: "auto",
  recordHotkey: "Alt",
  recordBackgroundWakeEnabled: true,
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  toolMaxIterations: 10,
  selectedApiConfigId: "",
  chatApiConfigId: "",
  visionApiConfigId: undefined,
  sttApiConfigId: undefined,
  sttAutoSend: false,
  shellWorkspaces: [],
  mcpServers: [],
  apiConfigs: [],
});
const configTab = ref<"hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "chatSettings" | "memory" | "logs" | "appearance" | "about">("hotkey");
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
const checkingUpdate = ref(false);
const updateDialogOpen = ref(false);
const updateDialogTitle = ref("检查更新");
const updateDialogBody = ref("");
const updateDialogKind = ref<"info" | "error">("info");
const updateDialogReleaseUrl = ref("");
const configSaveErrorDialogOpen = ref(false);
const configSaveErrorDialogTitle = ref("");
const configSaveErrorDialogBody = ref("");
const configSaveErrorDialogKind = ref<"warning" | "error">("error");
const terminalApprovalQueue = ref<TerminalApprovalRequestPayload[]>([]);
const terminalApprovalResolving = ref(false);
const archiveImportPreviewDialogOpen = ref(false);
const archiveImportPreview = ref<ArchiveImportPreview | null>(null);
const archiveImportRunning = ref(false);
const skillPlaceholderDialogOpen = ref(false);
const chatWorkspaceName = ref("默认工作空间");
const chatWorkspaceLocked = ref(false);
const chatWorkspacePath = ref("");
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
const PERF_DEBUG = import.meta.env.DEV;
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
  archiveSummaryText,
  selectedArchiveId,
  unarchivedConversations,
  unarchivedMessages,
  selectedUnarchivedConversationId,
  loadArchives,
  selectArchive,
  selectUnarchivedConversation,
  deleteUnarchivedConversation,
  deleteArchive,
  exportArchive,
  buildArchiveImportPreview,
  importArchivePayload,
} = useArchivesView({
  t: tr,
  setStatus,
  setStatusError,
});

const {
  memoryDialog,
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

type ChatShellWorkspaceState = {
  sessionId: string;
  workspaceName: string;
  rootPath: string;
  locked: boolean;
};

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
const textCapableApiConfigs = computed(() =>
  config.apiConfigs.filter(
    (a) =>
      a.enableText
      && (
        a.requestFormat === "openai"
        || a.requestFormat === "openai_responses"
        || a.requestFormat === "gemini"
        || a.requestFormat === "deepseek/kimi"
        || a.requestFormat === "anthropic"
      ),
  ),
);
const imageCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableImage));
const sttCapableApiConfigs = computed(() =>
  config.apiConfigs.filter((a) => a.requestFormat === "openai_stt"),
);
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
const activeSttApiConfig = computed(
  () => sttCapableApiConfigs.value.find((a) => a.id === config.sttApiConfigId) ?? null,
);
const shouldUseRemoteStt = computed(() => {
  const cfg = activeSttApiConfig.value;
  if (!cfg) return false;
  return !!cfg.model.trim() && !!cfg.baseUrl.trim() && !!cfg.apiKey.trim();
});
const {
  supported: speechRecognitionSupported,
  recording,
  recordingMs,
  transcribing,
  startRecording,
  stopRecording,
  prewarmMicrophone,
  cleanup: cleanupSpeechRecording,
} = useSpeechRecording({
  t: tr,
  canStart: () => !chatting.value && !forcingArchive.value,
  getLanguage: () => normalizeLocale(config.uiLanguage),
  getMinRecordSeconds: () => config.minRecordSeconds,
  getMaxRecordSeconds: () => config.maxRecordSeconds,
  shouldUseRemoteStt: () => shouldUseRemoteStt.value,
  transcribeRemoteStt: async (audio) => {
    const sttApiConfigId = activeSttApiConfig.value?.id;
    if (!sttApiConfigId) throw new Error("No STT API selected.");
    const out = await invokeTauri<{ text: string }>("stt_transcribe", {
      input: {
        sttApiConfigId,
        mime: audio.mime,
        bytesBase64: audio.bytesBase64,
      },
    });
    return String(out.text || "").trim();
  },
  appendRecognizedText: (text) => {
    chatInput.value = chatInput.value.trim() ? `${chatInput.value.trim()}\n${text}` : text;
  },
  onTranscribed: ({ source }) => {
    void invokeTauri("show_chat_window").catch((error) => {
      console.warn("[AUDIO] show_chat_window failed:", error);
    });
    if (source !== "remote") return;
    if (!config.sttAutoSend) return;
    if (chatting.value || forcingArchive.value) return;
    setTimeout(() => {
      void chatFlow.sendChat();
    }, 0);
  },
  setStatus: (text) => {
    status.value = text;
  },
});

async function tryPrewarmChatMic() {
  if (viewMode.value !== "chat") return;
  if (document.visibilityState === "hidden") return;
  if (!document.hasFocus()) return;
  await prewarmMicrophone();
}
const chatMedia = useChatMedia({
  t: tr,
  setStatus: (text) => {
    status.value = text;
  },
  setChatError: (text) => {
    chatErrorText.value = text;
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
const mediaDragActive = chatMedia.mediaDragActive;
const onPaste = chatMedia.onPaste;
const onDragOver = chatMedia.onDragOver;
const onDrop = chatMedia.onDrop;
const onNativeFileDrop = chatMedia.onNativeFileDrop;
const removeClipboardImage = chatMedia.removeClipboardImage;
const startHotkeyRecordTest = chatMedia.startHotkeyRecordTest;
const stopHotkeyRecordTest = chatMedia.stopHotkeyRecordTest;
const playHotkeyRecordTest = chatMedia.playHotkeyRecordTest;
const cleanupChatMedia = chatMedia.cleanupChatMedia;
const recordHotkey = useRecordHotkey({
  isActive: () => viewMode.value === "chat",
  getRecordHotkey: () => config.recordHotkey,
  onStartRecording: () => startRecording(),
  onStopRecording: (discard) => stopRecording(discard),
  startDelayMs: 0,
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
const chatPersonaNameMap = computed<Record<string, string>>(() => {
  const next: Record<string, string> = {};
  for (const persona of personas.value) {
    const id = String(persona.id || "").trim();
    if (!id) continue;
    const name = String(persona.name || "").trim();
    next[id] = name || id;
  }
  return next;
});
const chatPersonaAvatarUrlMap = computed<Record<string, string>>(() => {
  const next: Record<string, string> = {};
  for (const persona of personas.value) {
    const id = String(persona.id || "").trim();
    if (!id) continue;
    const url = resolveAvatarUrl(persona.avatarPath, persona.avatarUpdatedAt);
    if (url) next[id] = url;
  }
  return next;
});
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
  if (format === "gemini_embedding") return "https://generativelanguage.googleapis.com";
  if (format === "deepseek/kimi") return "https://api.deepseek.com/v1";
  if (format === "anthropic") return "https://api.anthropic.com";
  if (format === "openai_tts") return "https://api.openai.com/v1/audio/speech";
  if (format === "openai_stt") return "https://api.openai.com/v1";
  if (format === "openai_embedding") return "https://api.openai.com/v1";
  if (format === "openai_rerank") return "https://api.openai.com/v1";
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
const displayTurns = computed(() => chatting.value ? [] : visibleTurns.value);
const displayHasMoreTurns = computed(() => hasMoreTurns.value);
const terminalApprovalCurrent = computed(() => terminalApprovalQueue.value[0] ?? null);
const terminalApprovalDialogOpen = computed(() => !!terminalApprovalCurrent.value);
const terminalApprovalDialogTitle = computed(
  () => terminalApprovalCurrent.value?.title || "终端审批",
);
const terminalApprovalDialogBody = computed(
  () => terminalApprovalCurrent.value?.message || "",
);

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
  onSaveConfigError: openConfigSaveErrorDialog,
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
  setChatError: (text) => {
    chatErrorText.value = text;
  },
  activeChatApiConfigId,
  selectedPersonaId,
  chatting,
  forcingArchive,
  allMessages,
  visibleTurnCount,
  perfNow,
  perfLog,
  perfDebug: PERF_DEBUG,
});
const {
  refreshConversationHistory,
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
  refreshConversationHistory,
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
    applyTheme(theme);
  },
  onLocaleChanged: (payload) => {
    const lang = normalizeLocale(payload);
    config.uiLanguage = lang;
    locale.value = lang;
  },
  onRefreshSignal: async () => {
    await handleWindowRefreshSignal();
    void tryPrewarmChatMic();
  },
  onTerminalApprovalRequested: (payload) => {
    enqueueTerminalApprovalRequest(payload);
  },
  onConversationApiUpdated: async (payload) => {
    config.chatApiConfigId = String(payload.chatApiConfigId || "");
    config.visionApiConfigId = payload.visionApiConfigId || undefined;
    config.sttApiConfigId = payload.sttApiConfigId || undefined;
    config.sttAutoSend = !!payload.sttAutoSend;
    if (chatErrorText.value.includes("不支持图片附件") || chatErrorText.value.includes("PDF 附件")) {
      chatErrorText.value = "";
    }
    if (viewMode.value === "chat") {
      await refreshConversationHistory();
      visibleTurnCount.value = 1;
    }
  },
  onChatSettingsUpdated: async (payload) => {
    const nextAgentId = String(payload.selectedAgentId || "").trim();
    if (nextAgentId) {
      selectedPersonaId.value = nextAgentId;
      if (personaEditorId.value !== nextAgentId) {
        personaEditorId.value = nextAgentId;
      }
    }
    userAlias.value = String(payload.userAlias || "").trim() || t("archives.roleUser");
    const nextStyleId = String(payload.responseStyleId || "").trim();
    if (nextStyleId) {
      selectedResponseStyleId.value = nextStyleId;
    }
    if (viewMode.value === "chat") {
      await refreshConversationHistory();
      visibleTurnCount.value = 1;
    }
  },
  onConfigUpdated: (payload) => {
    if (!payload || typeof payload !== "object") return;
    config.hotkey = String(payload.hotkey || config.hotkey || "").trim() || config.hotkey;
    config.uiFont = normalizeUiFont(String(payload.uiFont || config.uiFont || "").trim() || config.uiFont);
    config.recordHotkey = String(payload.recordHotkey || config.recordHotkey || "").trim() || config.recordHotkey;
    config.recordBackgroundWakeEnabled = !!payload.recordBackgroundWakeEnabled;
    config.minRecordSeconds = Math.max(1, Math.min(30, Math.round(Number(payload.minRecordSeconds) || config.minRecordSeconds)));
    config.maxRecordSeconds = Math.max(
      config.minRecordSeconds,
      Math.min(600, Math.round(Number(payload.maxRecordSeconds) || config.maxRecordSeconds)),
    );
  },
  onRecordHotkeyProbe: (state) => {
    if (viewMode.value !== "chat") return;
    if (!config.recordBackgroundWakeEnabled) return;
    if (state === "pressed") {
      void startRecording();
      return;
    }
    if (state === "released") {
      void stopRecording(false);
    }
  },
});

function handleWindowFocusForMicPrewarm() {
  void tryPrewarmChatMic();
}

function handleVisibilityForMicPrewarm() {
  if (document.visibilityState !== "visible") return;
  void tryPrewarmChatMic();
}

onMounted(() => {
  window.addEventListener("focus", handleWindowFocusForMicPrewarm);
  document.addEventListener("visibilitychange", handleVisibilityForMicPrewarm);
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", handleWindowFocusForMicPrewarm);
  document.removeEventListener("visibilitychange", handleVisibilityForMicPrewarm);
});

watch(
  () => ({
    apiId: activeChatApiConfigId.value,
    imageEnabled: !!activeChatApiConfig.value?.enableImage,
    visionEnabled: hasVisionFallback.value,
  }),
  () => {
    if (chatErrorText.value.includes("不支持图片附件") || chatErrorText.value.includes("PDF 附件")) {
      chatErrorText.value = "";
    }
  },
);

watch(
  () => ({
    mode: viewMode.value,
    apiId: activeChatApiConfigId.value,
    agentId: selectedPersonaId.value,
  }),
  ({ mode }) => {
    if (mode !== "chat") return;
    void refreshChatWorkspaceState();
  },
  { immediate: true },
);

watch(
  () => viewMode.value,
  (mode) => {
    if (mode !== "chat") return;
    void tryPrewarmChatMic();
  },
);

watch(
  () => ({ uiFont: config.uiFont, uiLanguage: config.uiLanguage }),
  ({ uiFont, uiLanguage }) => {
    applyUiFont(uiFont, uiLanguage);
  },
  { immediate: true },
);

function setUiLanguage(value: string) {
  if (!applyUiLanguage(value)) return;
  void saveConfig();
}

function normalizeUiFont(value: string): string {
  const text = String(value || "").trim();
  if (!text) return "auto";
  if (text.length > 128) return text.slice(0, 128).trim() || "auto";
  return text;
}

function resolveUiFontFamily(uiFont: string, uiLanguage: string): string {
  const normalized = normalizeUiFont(uiFont);
  if (normalized === "auto") {
    if (uiLanguage === "zh-CN") {
      return "\"Microsoft YaHei\", \"PingFang SC\", \"Noto Sans CJK SC\", \"Segoe UI\", system-ui, sans-serif";
    }
    if (uiLanguage === "zh-TW") {
      return "\"PingFang TC\", \"Microsoft JhengHei\", \"Noto Sans CJK TC\", \"Segoe UI\", system-ui, sans-serif";
    }
    return "\"Segoe UI\", \"SF Pro Text\", system-ui, -apple-system, Roboto, \"Helvetica Neue\", Arial, sans-serif";
  }
  const escaped = normalized.replace(/"/g, '\\"');
  return `"${escaped}", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif`;
}

function applyUiFont(uiFont: string, uiLanguage: string) {
  const family = resolveUiFontFamily(uiFont, uiLanguage);
  document.documentElement.style.setProperty("--app-font-family", family);
}

function setUiFont(value: string) {
  const next = normalizeUiFont(value);
  if (config.uiFont === next) return;
  config.uiFont = next;
  void saveConfig();
}

async function refreshChatWorkspaceState() {
  const apiConfigId = String(activeChatApiConfigId.value || "").trim();
  const agentId = String(selectedPersonaId.value || "").trim();
  if (!apiConfigId || !agentId) {
    chatWorkspaceName.value = "默认工作空间";
    chatWorkspaceLocked.value = false;
    chatWorkspacePath.value = "";
    return;
  }
  try {
    const state = await invokeTauri<ChatShellWorkspaceState>("get_chat_shell_workspace", {
      input: { apiConfigId, agentId },
    });
    chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
    chatWorkspaceLocked.value = !!state.locked;
    chatWorkspacePath.value = String(state.rootPath || "").trim();
  } catch (error) {
    console.warn("[SHELL] refresh chat workspace failed:", error);
  }
}

async function lockChatWorkspaceFromPicker() {
  const apiConfigId = String(activeChatApiConfigId.value || "").trim();
  const agentId = String(selectedPersonaId.value || "").trim();
  if (!apiConfigId || !agentId) return;
  try {
    const picked = await open({
      directory: true,
      multiple: false,
      defaultPath: chatWorkspacePath.value || undefined,
    });
    if (!picked || Array.isArray(picked)) return;
    const state = await invokeTauri<ChatShellWorkspaceState>("lock_chat_shell_workspace", {
      input: {
        apiConfigId,
        agentId,
        workspacePath: String(picked),
      },
    });
    chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
    chatWorkspaceLocked.value = !!state.locked;
    chatWorkspacePath.value = String(state.rootPath || "").trim();
    setStatus(`工作空间已锁定: ${chatWorkspaceName.value}`);
  } catch (error) {
    setStatusError("status.requestFailed", error);
  }
}

async function unlockChatWorkspace() {
  const apiConfigId = String(activeChatApiConfigId.value || "").trim();
  const agentId = String(selectedPersonaId.value || "").trim();
  if (!apiConfigId || !agentId) return;
  try {
    const state = await invokeTauri<ChatShellWorkspaceState>("unlock_chat_shell_workspace", {
      input: {
        apiConfigId,
        agentId,
      },
    });
    chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
    chatWorkspaceLocked.value = !!state.locked;
    chatWorkspacePath.value = String(state.rootPath || "").trim();
    setStatus(`工作空间已解锁: ${chatWorkspaceName.value}`);
  } catch (error) {
    setStatusError("status.requestFailed", error);
  }
}

function openSkillPlaceholderDialog() {
  skillPlaceholderDialogOpen.value = true;
}

function closeSkillPlaceholderDialog() {
  skillPlaceholderDialogOpen.value = false;
}

function openConfigWindow() {
  void invokeTauri("show_main_window");
}

function summonChatWindowFromConfig() {
  void invokeTauri("show_chat_window");
}

function openGithubRepository() {
  void invokeTauri("open_external_url", { url: "https://github.com/kawayiYokami/Easy-call-ai" });
}

function closeUpdateDialog() {
  updateDialogOpen.value = false;
}

function closeConfigSaveErrorDialog() {
  configSaveErrorDialogOpen.value = false;
}

function enqueueTerminalApprovalRequest(payload: TerminalApprovalRequestPayload) {
  const requestId = String(payload.requestId || "").trim();
  if (!requestId) return;
  terminalApprovalQueue.value.push({
    ...payload,
    requestId,
    title: String(payload.title || "终端审批"),
    message: String(payload.message || ""),
    approvalKind: String(payload.approvalKind || "unknown"),
    sessionId: String(payload.sessionId || ""),
  });
}

async function resolveTerminalApproval(approved: boolean) {
  if (terminalApprovalResolving.value) return;
  const current = terminalApprovalCurrent.value;
  if (!current) return;
  terminalApprovalResolving.value = true;
  try {
    await invokeTauri("resolve_terminal_approval", {
      input: {
        requestId: current.requestId,
        approved,
      },
    });
  } catch (error) {
    console.warn("[TERMINAL] resolve_terminal_approval failed:", error);
  } finally {
    terminalApprovalQueue.value.shift();
    terminalApprovalResolving.value = false;
  }
}

function denyTerminalApproval() {
  void resolveTerminalApproval(false);
}

function approveTerminalApproval() {
  void resolveTerminalApproval(true);
}

function closeArchiveImportPreviewDialog() {
  if (archiveImportRunning.value) return;
  archiveImportPreviewDialogOpen.value = false;
  archiveImportPreview.value = null;
}

function openUpdateDialog(text: string, kind: "info" | "error", releaseUrl?: string) {
  updateDialogTitle.value = "检查更新";
  updateDialogBody.value = text;
  updateDialogKind.value = kind;
  updateDialogReleaseUrl.value = releaseUrl || "";
  updateDialogOpen.value = true;
}

function openUpdateRelease() {
  const url = updateDialogReleaseUrl.value.trim();
  if (!url) return;
  void invokeTauri("open_external_url", { url });
}

function openConfigSaveErrorDialog(info: ConfigSaveErrorInfo) {
  configSaveErrorDialogTitle.value = t("status.saveConfigDialogTitle");
  if (info.kind === "hotkey_conflict") {
    configSaveErrorDialogKind.value = "warning";
    configSaveErrorDialogBody.value = `${t("status.saveConfigHotkeyOccupied", { hotkey: info.hotkey })}\n${t("status.saveConfigDialogHint")}`;
    configTab.value = "hotkey";
  } else if (info.kind === "backend_404") {
    configSaveErrorDialogKind.value = "error";
    configSaveErrorDialogBody.value = t("status.saveConfigBackend404");
  } else {
    configSaveErrorDialogKind.value = "error";
    configSaveErrorDialogBody.value = t("status.saveConfigFailed", { err: info.errorText });
  }
  configSaveErrorDialogOpen.value = true;
}

async function prepareArchiveImport(file: File) {
  try {
    const preview = await buildArchiveImportPreview(file);
    archiveImportPreview.value = preview;
    archiveImportPreviewDialogOpen.value = true;
  } catch (e) {
    setStatusError("status.importArchiveFailed", e);
  }
}

async function importPersonaMemories(payload: { agentId: string; file: File }) {
  const agentId = String(payload.agentId || "").trim();
  if (!agentId) return;
  try {
    const text = await payload.file.text();
    const parsed = JSON.parse(text) as unknown;
    const memories = Array.isArray(parsed)
      ? parsed
      : parsed && typeof parsed === "object" && Array.isArray((parsed as { memories?: unknown }).memories)
        ? (parsed as { memories: unknown[] }).memories
        : null;
    if (!Array.isArray(memories)) {
      throw new Error("无效的记忆文件格式");
    }
    const result = await invokeTauri<{ importedCount: number; createdCount: number; mergedCount: number; totalCount: number }>(
      "import_agent_memories",
      {
        input: { agentId, memories },
      },
    );
    setStatus(`人格记忆导入完成: 新增 ${result.createdCount} 条, 合并 ${result.mergedCount} 条, 总计 ${result.totalCount} 条`);
  } catch (e) {
    setStatusError("status.importMemoriesFailed", e);
  }
}

async function confirmArchiveImport() {
  if (!archiveImportPreview.value || archiveImportRunning.value) return;
  archiveImportRunning.value = true;
  try {
    await importArchivePayload(archiveImportPreview.value.payloadJson);
    archiveImportPreviewDialogOpen.value = false;
    archiveImportPreview.value = null;
  } catch (e) {
    setStatusError("status.importArchiveFailed", e);
  } finally {
    archiveImportRunning.value = false;
  }
}

async function autoCheckGithubUpdate() {
  await checkGithubUpdate(true);
}

async function manualCheckGithubUpdate() {
  await checkGithubUpdate(false);
}

async function checkGithubUpdate(silent: boolean) {
  if (viewMode.value !== "config") return;
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    status.value = "检查更新中...";
    const result = await invokeTauri<{
      currentVersion: string;
      latestVersion: string;
      hasUpdate: boolean;
      releaseUrl: string;
    }>("check_github_update");
    if (!result?.hasUpdate) {
      if (!silent) {
        status.value = `当前已是最新版本 ${result.currentVersion}`;
        openUpdateDialog(`当前已是最新版本 ${result.currentVersion}`, "info");
      }
      return;
    }
    status.value = `发现新版本 ${result.latestVersion}（当前 ${result.currentVersion}）`;
    if (!silent) {
      openUpdateDialog(
        `发现新版本 ${result.latestVersion}\n当前版本 ${result.currentVersion}\n\n可前往 GitHub Releases 下载更新。`,
        "info",
        result.releaseUrl,
      );
    }
  } catch (error) {
    if (!silent) {
      status.value = `检查更新失败: ${String(error)}`;
      openUpdateDialog(`检查更新失败：${String(error)}`, "error");
    }
    console.warn("[UPDATE] check_github_update failed:", error);
  } finally {
    checkingUpdate.value = false;
  }
}

const chatFlow = useChatFlow({
  chatting,
  forcingArchive,
  getSession: () => {
    const apiConfigId = String(activeChatApiConfigId.value || "").trim();
    const agentId = String(selectedPersonaId.value || "").trim();
    if (!apiConfigId || !agentId) return null;
    return { apiConfigId, agentId };
  },
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
  invokeSendChatMessage: ({ text, images, session, onDelta }) =>
    invokeTauri("send_chat_message", {
      input: {
        payload: { text, images },
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
        },
      },
      onDelta,
    }),
  invokeStopChatMessage: ({ session, partialAssistantText, partialReasoningStandard, partialReasoningInline }) =>
    invokeTauri("stop_chat_message", {
      input: {
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
        },
        partialAssistantText,
        partialReasoningStandard,
        partialReasoningInline,
      },
    }),
  onReloadMessages: () => loadAllMessages(),
});

function clearStreamBuffer() {
  chatFlow.clearStreamBuffer();
}

type RewindConversationResult = {
  removedCount: number;
  remainingCount: number;
  recalledUserMessage?: ChatMessage;
};

async function rewindConversationFromTurn(turnId: string): Promise<ChatMessage | null> {
  const apiConfigId = String(activeChatApiConfigId.value || "").trim();
  const agentId = String(selectedPersonaId.value || "").trim();
  const messageId = String(turnId || "").trim();
  if (!apiConfigId || !agentId || !messageId) return null;
  try {
    const result = await invokeTauri<RewindConversationResult>("rewind_conversation_from_message", {
      input: {
        session: {
          apiConfigId,
          agentId,
        },
        messageId,
      },
    });
    await loadAllMessages();
    visibleTurnCount.value = 1;
    return result.recalledUserMessage ?? null;
  } catch (error) {
    setStatusError("status.rewindConversationFailed", error);
    return null;
  }
}

async function handleRecallTurn(payload: { turnId: string }) {
  if (chatting.value || forcingArchive.value) return;
  const recalledUserMessage = await rewindConversationFromTurn(payload.turnId);
  if (!recalledUserMessage) return;
  chatInput.value = removeBinaryPlaceholders(messageText(recalledUserMessage));
  clipboardImages.value = extractMessageImages(recalledUserMessage);
}

async function handleRegenerateTurn(payload: { turnId: string }) {
  if (chatting.value || forcingArchive.value) return;
  const recalledUserMessage = await rewindConversationFromTurn(payload.turnId);
  if (!recalledUserMessage) return;
  chatInput.value = removeBinaryPlaceholders(messageText(recalledUserMessage));
  clipboardImages.value = extractMessageImages(recalledUserMessage);
  await chatFlow.sendChat();
}

function handleToolsChanged() {
  if (toolSwitchAutosaveTimer) {
    clearTimeout(toolSwitchAutosaveTimer);
  }
  toolSwitchAutosaveTimer = setTimeout(async () => {
    const saved = await saveConfig();
    if (saved && configTab.value === "tools") {
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
  onDragOver,
  onDrop,
  onNativeFileDrop,
  onNativeDragState: (active) => {
    mediaDragActive.value = active;
  },
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
  afterMountedReady: autoCheckGithubUpdate,
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
  refreshConversationHistory,
  resetVisibleTurnCount: () => {
    visibleTurnCount.value = 1;
  },
});
</script>
