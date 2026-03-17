
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
      :assistant-department-agent-id="assistantDepartmentAgentId"
      :selected-persona-editor="selectedPersonaEditor"
      :tool-persona="selectedPersonaEditor"
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
      :persona-saving="personaSaving"
      :persona-dirty="personaDirty"
      :config-dirty="configDirty"
      :saving="saving"
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio="hotkeyTestAudio"
      :user-alias="userAlias"
      :selected-persona-name="assistantDepartmentPersona?.name || t('archives.roleAssistant')"
      :current-chat-workspace-name="chatWorkspaceName"
      :chat-workspace-locked="chatWorkspaceLocked"
      :user-avatar-url="userAvatarUrl"
      :selected-persona-avatar-url="selectedPersonaAvatarUrl"
      :chat-persona-name-map="chatPersonaNameMap"
      :chat-persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :chat-persona-presence-chips="chatPersonaPresenceChips"
      :latest-user-text="latestUserText"
      :latest-user-images="latestUserImages"
      :latest-assistant-text="latestAssistantText"
      :latest-reasoning-standard-text="latestReasoningStandardText"
      :latest-reasoning-inline-text="latestReasoningInlineText"
      :tool-status-text="toolStatusText"
      :tool-status-state="toolStatusState"
      :stream-tool-calls="streamToolCalls"
      :chat-error-text="chatErrorText"
      :clipboard-images="clipboardImages"
      :queued-attachment-notices="queuedAttachmentNotices"
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
      :visible-message-blocks="displayMessageBlocks"
      :has-more-message-blocks="displayHasMoreMessageBlocks"
      :archives="archives"
      :selected-archive-id="selectedArchiveId"
      :archive-messages="archiveMessages"
      :archive-summary-text="archiveSummaryText"
      :unarchived-conversations="unarchivedConversations"
      :selected-unarchived-conversation-id="selectedUnarchivedConversationId"
      :unarchived-messages="unarchivedMessages"
      :delegate-conversations="delegateConversations"
      :selected-delegate-conversation-id="selectedDelegateConversationId"
      :delegate-messages="delegateMessages"
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
      :set-memory-dialog-ref="setMemoryDialogRef"
      :set-prompt-preview-dialog-ref="setPromptPreviewDialogRef"
      :set-status="setStatus"
      :update-config-tab="(value) => { configTab = value; }"
      :set-ui-language="setUiLanguage"
      :update-persona-editor-id="updatePersonaEditorIdWithNotice"
      :update-selected-persona-id="updateAssistantDepartmentAgentId"
      :update-selected-response-style-id="updateSelectedResponseStyleId"
      :set-theme="setTheme"
      :refresh-models="refreshModels"
      :on-tools-changed="handleToolsChanged"
      :save-config="saveConfig"
      :add-api-config="addApiConfig"
      :remove-selected-api-config="removeSelectedApiConfig"
      :add-persona="addPersona"
      :remove-selected-persona="removeSelectedPersona"
      :save-personas="savePersonas"
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
      :remove-queued-attachment-notice="removeQueuedAttachmentNotice"
      :pick-attachments="pickChatAttachments"
      :start-recording="startRecording"
      :stop-recording="() => stopRecording(false)"
      :send-chat="chatFlow.sendChat"
      :stop-chat="chatFlow.stopChat"
      :load-more-message-blocks="loadMoreMessageBlocks"
      :on-recall-turn="handleRecallTurn"
      :on-regenerate-turn="handleRegenerateTurn"
      :on-lock-chat-workspace="lockChatWorkspaceFromPicker"
      :on-unlock-chat-workspace="unlockChatWorkspace"
      :on-open-skill-panel="openSkillPlaceholderDialog"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :select-unarchived-conversation="selectUnarchivedConversation"
      :select-delegate-conversation="selectDelegateConversation"
      :export-archive="exportArchive"
      :import-archive-file="prepareArchiveImport"
      :delete-archive="deleteArchive"
      :delete-unarchived-conversation="deleteUnarchivedConversation"
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
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invokeTauri } from "./services/tauri-api";
import { useAppBootstrap } from "./features/shell/composables/use-app-bootstrap";
import { useAppCore } from "./features/shell/composables/use-app-core";
import { useAppLifecycle } from "./features/shell/composables/use-app-lifecycle";
import { useAppTheme } from "./features/shell/composables/use-app-theme";
import { useGithubUpdate } from "./features/shell/composables/use-github-update";
import { useTerminalApproval, type TerminalApprovalRequestPayload } from "./features/shell/composables/use-terminal-approval";
import { applyUiFont, normalizeUiFont } from "./features/shell/composables/use-ui-font";
import { useViewRefresh } from "./features/shell/composables/use-view-refresh";
import { useWindowShell } from "./features/shell/composables/use-window-shell";
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useConfigPersistence, type ConfigSaveErrorInfo } from "./features/config/composables/use-config-persistence";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useAgentWorkPresence } from "./features/chat/composables/use-agent-work-presence";
import { useArchivesView } from "./features/chat/composables/use-archives-view";
import { useArchiveImport } from "./features/chat/composables/use-archive-import";
import { useAvatarCache } from "./features/chat/composables/use-avatar-cache";
import { useChatDialogActions } from "./features/chat/composables/use-chat-dialog-actions";
import { useChatWorkspace } from "./features/chat/composables/use-chat-workspace";
import { useChatRewindActions } from "./features/chat/composables/use-chat-rewind-actions";
import { useChatRuntime } from "./features/chat/composables/use-chat-runtime";
import { useChatMessageBlocks } from "./features/chat/composables/use-chat-turns";
import { useChatMedia } from "./features/chat/composables/use-chat-media";
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
  AppConfig,
  ChatMessage,
  ChatPersonaPresenceChip,
  ImageTextCacheStats,
  ResponseStyleOption,
  ToolLoadStatus,
} from "./types/app";
import responseStylesJson from "./constants/response-styles.json";
import { normalizeLocale } from "./i18n";

const viewMode = ref<"chat" | "archives" | "config">("config");
const { t, locale } = useI18n();
const tr = (key: string, params?: Record<string, unknown>) => (params ? t(key, params) : t(key));
const isMacPlatform = /Mac|iPhone|iPad|iPod/i.test(window.navigator.platform || "");
const { windowReady, alwaysOnTop, initWindow, syncAlwaysOnTop, closeWindow, startDrag, toggleAlwaysOnTop } =
  useWindowShell();
const { currentTheme, applyTheme, setTheme, restoreThemeFromStorage } = useAppTheme();

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  uiLanguage: "zh-CN",
  uiFont: "auto",
  recordHotkey: isMacPlatform ? "Option+Space" : "Alt",
  recordBackgroundWakeEnabled: true,
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  toolMaxIterations: 10,
  selectedApiConfigId: "",
  assistantDepartmentApiConfigId: "",
  visionApiConfigId: undefined,
  sttApiConfigId: undefined,
  sttAutoSend: false,
  terminalShellKind: "auto",
  shellWorkspaces: [],
  mcpServers: [],
  remoteImChannels: [],
  departments: [],
  apiConfigs: [],
});
const recordHotkeyProbeLastSeq = ref(0);
const recordHotkeyProbeDown = ref(false);
const chatWindowActiveSynced = ref<boolean | null>(null);
const tauriWindowLabel = ref("unknown");
const isChatTauriWindow = ref(false);
const CHAT_STREAM_BIND_HEARTBEAT_MS = 10_000;
let chatStreamBindHeartbeatTimer: ReturnType<typeof setInterval> | null = null;
let chatHistoryFlushedUnlisten: UnlistenFn | null = null;
let chatRoundCompletedUnlisten: UnlistenFn | null = null;
let chatRoundFailedUnlisten: UnlistenFn | null = null;
let chatAssistantDeltaUnlisten: UnlistenFn | null = null;
const configTab = ref<"hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about">("hotkey");
const personas = ref<PersonaProfile[]>([]);
const assistantDepartmentAgentId = ref("default-agent");
const personaEditorId = ref("default-agent");
const userAlias = ref(t("archives.roleUser"));
const selectedResponseStyleId = ref("concise");
const chatInput = ref("");
const currentChatConversationId = ref("");
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const latestReasoningStandardText = ref("");
const latestReasoningInlineText = ref("");
const toolStatusText = ref("");
const toolStatusState = ref<"running" | "done" | "failed" | "">("");
const streamToolCalls = ref<Array<{ name: string; argsText: string }>>([]);
const chatErrorText = ref("");
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>([]);

const allMessages = shallowRef<ChatMessage[]>([]);
const visibleMessageBlockCount = ref(1);

const status = ref("Ready.");
const configSaveErrorDialogOpen = ref(false);
const configSaveErrorDialogTitle = ref("");
const configSaveErrorDialogBody = ref("");
const configSaveErrorDialogKind = ref<"warning" | "error">("error");
const terminalApprovalQueue = ref<TerminalApprovalRequestPayload[]>([]);
const terminalApprovalResolving = ref(false);
const skillPlaceholderDialogOpen = ref(false);
const loading = ref(false);
const saving = ref(false);
const chatting = ref(false);
const forcingArchive = ref(false);
const hasMoreBackendHistory = ref(false);
const refreshingModels = ref(false);
const modelRefreshError = ref("");
const modelRefreshOkFlags = ref<Record<string, boolean>>({});
const checkingToolsStatus = ref(false);
const toolStatuses = ref<ToolLoadStatus[]>([]);
const imageCacheStats = ref<ImageTextCacheStats>({ entries: 0, totalChars: 0 });
const imageCacheStatsLoading = ref(false);
const avatarSaving = ref(false);
const avatarError = ref("");
const personaSaving = ref(false);
const apiModelOptions = ref<Record<string, string[]>>({});
const suppressAutosave = ref(false);
const RECORD_HOTKEY_SUPPRESS_AFTER_POPUP_MS = 700;
const lastSavedConfigJson = ref("");
const lastSavedPersonasJson = ref("");
const PERF_DEBUG = import.meta.env.DEV;
const { perfNow, perfLog, setStatus, setStatusError, localeOptions, applyUiLanguage } = useAppCore({
  t: tr,
  config,
  locale,
  status,
  perfDebug: PERF_DEBUG,
});
const {
  checkingUpdate,
  updateDialogOpen,
  updateDialogTitle,
  updateDialogBody,
  updateDialogKind,
  updateDialogReleaseUrl,
  closeUpdateDialog,
  openUpdateRelease,
  autoCheckGithubUpdate,
  manualCheckGithubUpdate,
} = useGithubUpdate({
  viewMode,
  status,
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
  delegateConversations,
  delegateMessages,
  selectedDelegateConversationId,
  loadArchives,
  loadDelegateConversations,
  loadUnarchivedConversations,
  selectArchive,
  selectUnarchivedConversation,
  selectDelegateConversation,
  deleteUnarchivedConversation: deleteUnarchivedConversationFromArchives,
  deleteArchive,
  exportArchive,
  buildArchiveImportPreview,
  importArchivePayload,
} = useArchivesView({
  t: tr,
  setStatus,
  setStatusError,
});
const agentWorkPresence = useAgentWorkPresence();
const {
  archiveImportPreviewDialogOpen,
  archiveImportPreview,
  archiveImportRunning,
  closeArchiveImportPreviewDialog,
  prepareArchiveImport,
  confirmArchiveImport,
} = useArchiveImport({
  buildArchiveImportPreview,
  importArchivePayload,
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

const titleText = computed(() => {
  if (viewMode.value === "chat") {
    return t("window.chatTitle", { name: assistantDepartmentPersona.value?.name || t("archives.roleAssistant") });
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
const assistantDepartmentApiConfigId = computed(
  () => config.assistantDepartmentApiConfigId || textCapableApiConfigs.value[0]?.id || config.apiConfigs[0]?.id || "",
);
const assistantDepartmentApiConfig = computed(
  () => config.apiConfigs.find((a) => a.id === assistantDepartmentApiConfigId.value) ?? null,
);
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
    if (!isChatWindowActiveNow()) {
      void invokeTauri("show_chat_window").catch((error) => {
        console.warn("[AUDIO] show_chat_window failed:", error);
      });
    }
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

function isChatWindowActiveNow(): boolean {
  return viewMode.value === "chat" && document.visibilityState === "visible" && document.hasFocus();
}

function clearRecordHotkeyProbeState() {
  recordHotkeyProbeDown.value = false;
  recordHotkeyProbeLastSeq.value = 0;
}

function syncChatWindowActiveState() {
  if (!isChatTauriWindow.value) return;
  const active = isChatWindowActiveNow();
  if (chatWindowActiveSynced.value === active) return;
  chatWindowActiveSynced.value = active;
  if (active) {
    // 每次窗口激活都强制重绑一次聊天流通道，确保后端绑定表不丢失。
    void chatFlow.bindActiveConversationStream(String(currentChatConversationId.value || "").trim(), true);
    void stopRecording(false);
    recordHotkey.suppressAfterPopup(RECORD_HOTKEY_SUPPRESS_AFTER_POPUP_MS);
  }
  clearRecordHotkeyProbeState();
  void invokeTauri("set_chat_window_active", { active }).catch((error) => {
    console.warn("[HOTKEY] set_chat_window_active failed:", error);
  });
}

function handleWindowFocusForStateSync() {
  syncChatWindowActiveState();
}

function handleWindowBlurForStateSync() {
  syncChatWindowActiveState();
}

function handleVisibilityForStateSync() {
  syncChatWindowActiveState();
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
  activeChatApiConfig: assistantDepartmentApiConfig,
  hasVisionFallback,
  chatInput,
  clipboardImages,
  queuedAttachmentNotices,
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
function removeQueuedAttachmentNotice(index: number) {
  if (index < 0 || index >= queuedAttachmentNotices.value.length) return;
  queuedAttachmentNotices.value.splice(index, 1);
}
const startHotkeyRecordTest = chatMedia.startHotkeyRecordTest;
const stopHotkeyRecordTest = chatMedia.stopHotkeyRecordTest;
const playHotkeyRecordTest = chatMedia.playHotkeyRecordTest;
const cleanupChatMedia = chatMedia.cleanupChatMedia;

async function pickChatAttachments() {
  if (chatting.value || forcingArchive.value) return;
  try {
    const picked = await open({
      multiple: true,
      directory: false,
      title: "选择附件",
    });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    const normalized = paths
      .map((v) => String(v || "").trim())
      .filter(Boolean);
    if (normalized.length === 0) return;
    await onNativeFileDrop(normalized);
  } catch (error) {
    setStatusError("status.pasteImageReadFailed", error);
  }
}
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
  personas.value.filter((p) => !p.isBuiltInUser && !p.isBuiltInSystem && p.id !== "user-persona" && p.id !== "system-persona"),
);
const assistantDepartmentPersona = computed(
  () =>
    assistantPersonas.value.find((p) => p.id === assistantDepartmentAgentId.value)
    ?? assistantPersonas.value[0]
    ?? null,
);
const activeAssistantAgentId = computed(() => assistantDepartmentAgentId.value);
const {
  chatWorkspaceName,
  chatWorkspaceLocked,
  refreshChatWorkspaceState,
  lockChatWorkspaceFromPicker,
  unlockChatWorkspace,
} = useChatWorkspace({
  activeApiConfigId: assistantDepartmentApiConfigId,
  activeAgentId: activeAssistantAgentId,
  setStatus,
  setStatusError,
});
const selectedPersonaEditor = computed(
  () => personas.value.find((p) => p.id === personaEditorId.value) ?? null,
);
const toolDepartment = computed(() =>
  config.departments.find((item) => (item.agentIds || []).includes(personaEditorId.value)) ?? null,
);
const toolApiConfig = computed(() =>
  config.apiConfigs.find((a) => a.id === (toolDepartment.value?.apiConfigId || "")) ?? null,
);
const userAvatarUrl = computed(
  () => resolveAvatarUrl(userPersona.value?.avatarPath, userPersona.value?.avatarUpdatedAt),
);
const userPersonaAvatarUrl = computed(() => userAvatarUrl.value);
const selectedPersonaAvatarUrl = computed(
  () => resolveAvatarUrl(assistantDepartmentPersona.value?.avatarPath, assistantDepartmentPersona.value?.avatarUpdatedAt),
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
const chatPersonaDepartmentNameMap = computed<Record<string, string>>(() => {
  const next: Record<string, string> = {};
  for (const department of config.departments) {
    const departmentName = String(department.name || "").trim();
    if (!departmentName) continue;
    for (const agentId of department.agentIds || []) {
      const trimmedId = String(agentId || "").trim();
      if (!trimmedId) continue;
      next[trimmedId] = departmentName;
    }
  }
  return next;
});
const chatPersonaPresenceChips = computed<ChatPersonaPresenceChip[]>(() => {
  const items: ChatPersonaPresenceChip[] = [];
  for (const persona of personas.value) {
    if (persona.isBuiltInSystem || persona.id === "system-persona") continue;
    const id = String(persona.id || "").trim();
    if (!id) continue;
    const backgroundTaskCount = agentWorkPresence.activeWorkCountForAgent(id);
    items.push({
      id,
      name: String(persona.name || "").trim() || id,
      avatarUrl: String(chatPersonaAvatarUrlMap.value[id] || "").trim(),
      departmentName:
        String(chatPersonaDepartmentNameMap.value[id] || "").trim()
        || (id === "user-persona" ? "用户" : "未归属部门"),
      isFrontSpeaking: chatting.value && id === activeAssistantAgentId.value,
      hasBackgroundTask: backgroundTaskCount > 0,
    });
  }
  return items.sort((left, right) => {
    if (left.isFrontSpeaking !== right.isFrontSpeaking) return left.isFrontSpeaking ? -1 : 1;
    if (left.hasBackgroundTask !== right.hasBackgroundTask) return left.hasBackgroundTask ? -1 : 1;
    if (left.id === "user-persona" && right.id !== "user-persona") return -1;
    if (right.id === "user-persona" && left.id !== "user-persona") return 1;
    return left.name.localeCompare(right.name, config.uiLanguage === "en-US" ? "en" : "zh-CN");
  });
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
  const api = assistantDepartmentApiConfig.value;
  if (!api) return t("chat.placeholder");
  const hints: string[] = [];
  if (api.enableImage || hasVisionFallback.value) hints.push("Ctrl+V");
  if (hints.length === 0) return t("chat.placeholder");
  return t("chat.placeholder");
});
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
const personaDirty = computed(() => buildPersonasSnapshotJson() !== lastSavedPersonasJson.value);
const responseStyleIds = computed(() => responseStyleOptions.map((item) => item.id));
const { visibleMessageBlocks, hasMoreMessageBlocks, chatContextUsageRatio, chatUsagePercent } = useChatMessageBlocks({
  allMessages,
  visibleMessageBlockCount,
  hasMoreBackendHistory,
  activeChatApiConfig: assistantDepartmentApiConfig,
  perfDebug: PERF_DEBUG,
  perfNow,
});
const DEFAULT_CHAT_VISIBLE_COUNT = 5;
const displayMessageBlocks = computed(() => visibleMessageBlocks.value);
const displayHasMoreMessageBlocks = computed(() => hasMoreMessageBlocks.value || hasMoreBackendHistory.value);
const {
  terminalApprovalDialogOpen,
  terminalApprovalDialogTitle,
  terminalApprovalDialogBody,
  enqueueTerminalApprovalRequest,
  denyTerminalApproval,
  approveTerminalApproval,
} = useTerminalApproval({
  queue: terminalApprovalQueue,
  resolving: terminalApprovalResolving,
});

function syncUserAliasFromPersona() {
  const next = (userPersona.value?.name || "").trim() || t("archives.roleUser");
  if (userAlias.value !== next) {
    userAlias.value = next;
  }
}

function resetVisibleMessageBlocksByCurrentMessages() {
  const total = allMessages.value.length;
  if (total <= 0) {
    visibleMessageBlockCount.value = 1;
    return;
  }
  visibleMessageBlockCount.value = Math.min(DEFAULT_CHAT_VISIBLE_COUNT, total);
}

function updatePersonaEditorIdWithNotice(value: string) {
  const nextId = String(value || "").trim();
  if (!nextId || nextId === personaEditorId.value) return;
  if (personaDirty.value) {
    const currentName = String(selectedPersonaEditor.value?.name || personaEditorId.value || "").trim() || t("config.persona.title");
    status.value = t("status.personaUnsavedSwitchHint", { name: currentName });
  }
  personaEditorId.value = nextId;
}

function updateAssistantDepartmentAgentId(value: string) {
  assistantDepartmentAgentId.value = value;
}

function updateSelectedResponseStyleId(value: string) {
  selectedResponseStyleId.value = value;
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
  assistantDepartmentAgentId,
  personaEditorId,
  avatarSaving,
  avatarError,
  toolPersona: selectedPersonaEditor,
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
  savingPersonas: personaSaving,
  personas,
  assistantPersonas,
  assistantDepartmentAgentId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  responseStyleIds,
  createApiConfig,
  normalizeApiBindingsLocal,
  buildConfigPayload,
  buildConfigSnapshotJson,
  buildPersonasSnapshotJson,
  lastSavedConfigJson,
  lastSavedPersonasJson,
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
  activeChatApiConfigId: assistantDepartmentApiConfigId,
  assistantDepartmentAgentId: activeAssistantAgentId,
  currentConversationId: currentChatConversationId,
  chatting,
  forcingArchive,
  allMessages,
  visibleMessageBlockCount,
  hasMoreBackendHistory,
  perfNow,
  perfLog,
  perfDebug: PERF_DEBUG,
});
const {
  refreshConversationHistory,
  forceArchiveNow,
  loadAllMessages,
  loadMoreMessageBlocks,
} = chatRuntime;

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
  assistantDepartmentAgentId,
  personaEditorId,
  selectedPersonaEditor,
  createApiConfig,
  normalizeApiBindingsLocal,
});

const { suppressChatReloadWatch, refreshAllViewData } = useViewRefresh({
  viewMode,
  recordHotkeySuppressAfterPopup: recordHotkey.suppressAfterPopup,
  recordHotkeySuppressMs: RECORD_HOTKEY_SUPPRESS_AFTER_POPUP_MS,
  loadConfig,
  loadPersonas,
  loadChatSettings,
  refreshImageCacheStats,
  refreshConversationHistory,
  loadDelegateConversations,
  loadArchives,
  resetVisibleTurnCount: () => {
    resetVisibleMessageBlocksByCurrentMessages();
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
  onAgentWorkStarted: (payload) => {
    agentWorkPresence.markAgentWorkStarted(payload);
  },
  onAgentWorkStopped: (payload) => {
    agentWorkPresence.markAgentWorkStopped(payload);
  },
  onTerminalApprovalRequested: (payload) => {
    enqueueTerminalApprovalRequest(payload);
  },
  onConversationApiUpdated: async (payload) => {
    config.assistantDepartmentApiConfigId = String(payload.assistantDepartmentApiConfigId || "");
    config.visionApiConfigId = payload.visionApiConfigId || undefined;
    config.sttApiConfigId = payload.sttApiConfigId || undefined;
    config.sttAutoSend = !!payload.sttAutoSend;
    if (chatErrorText.value.includes("不支持图片附件") || chatErrorText.value.includes("PDF 附件")) {
      chatErrorText.value = "";
    }
    if (viewMode.value === "chat") {
      await refreshConversationHistory();
      resetVisibleMessageBlocksByCurrentMessages();
    }
  },
  onChatSettingsUpdated: async (payload) => {
    const nextAgentId = String(payload.assistantDepartmentAgentId || "").trim();
    if (nextAgentId) {
      assistantDepartmentAgentId.value = nextAgentId;
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
      resetVisibleMessageBlocksByCurrentMessages();
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
    config.toolMaxIterations = Math.max(1, Math.min(100, Number(payload.toolMaxIterations || config.toolMaxIterations)));
    config.selectedApiConfigId = String(payload.selectedApiConfigId || config.selectedApiConfigId || "").trim() || config.selectedApiConfigId;
    config.assistantDepartmentApiConfigId =
      String(payload.assistantDepartmentApiConfigId || config.assistantDepartmentApiConfigId || "").trim()
      || config.assistantDepartmentApiConfigId;
    config.visionApiConfigId = payload.visionApiConfigId ?? undefined;
    config.sttApiConfigId = payload.sttApiConfigId ?? undefined;
    config.sttAutoSend = !!payload.sttAutoSend;
    config.terminalShellKind =
      String(payload.terminalShellKind || config.terminalShellKind || "auto").trim() || "auto";
    config.apiConfigs.splice(
      0,
      config.apiConfigs.length,
      ...((Array.isArray(payload.apiConfigs) && payload.apiConfigs.length > 0)
        ? payload.apiConfigs.map((item) => ({
            ...item,
            tools: Array.isArray(item.tools) ? item.tools.map((tool) => ({
              ...tool,
              args: Array.isArray(tool.args) ? [...tool.args] : [],
              values: { ...((tool.values || {}) as Record<string, unknown>) },
            })) : [],
          }))
        : [createApiConfig("default")]),
    );
    config.departments = Array.isArray(payload.departments)
      ? payload.departments.map((item) => ({
          ...item,
          agentIds: Array.isArray(item.agentIds) ? [...item.agentIds] : [],
        }))
      : [];
    normalizeApiBindingsLocal();
  },
  onRecordHotkeyProbe: ({ state, seq }) => {
    if (seq > 0) {
      if (seq <= recordHotkeyProbeLastSeq.value) return;
      recordHotkeyProbeLastSeq.value = seq;
    }
    if (state === "released") {
      recordHotkeyProbeDown.value = false;
    }
    if (viewMode.value !== "chat") return;
    if (!config.recordBackgroundWakeEnabled) return;
    if (state === "pressed") {
      recordHotkeyProbeDown.value = true;
      void startRecording().then(() => {
        if (!recordHotkeyProbeDown.value) {
          void stopRecording(false);
        }
      });
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
  try {
    const label = String(getCurrentWindow().label || "").trim();
    tauriWindowLabel.value = label || "unknown";
    isChatTauriWindow.value = tauriWindowLabel.value === "chat";
  } catch {
    tauriWindowLabel.value = "unknown";
    isChatTauriWindow.value = false;
  }
  console.warn("[CHAT_TRACE][window] init", {
    label: tauriWindowLabel.value,
    isChatWindow: isChatTauriWindow.value,
  });
  if (isChatTauriWindow.value) {
    void listen<unknown>("easy-call:history-flushed", (event) => {
      console.warn("[CHAT_TRACE][emit_history_flushed] received", {
        windowLabel: tauriWindowLabel.value,
        hasPayload: event.payload !== undefined,
      });
      void chatFlow.handleExternalHistoryFlushed(event.payload);
    })
      .then((unlisten) => {
        chatHistoryFlushedUnlisten = unlisten;
        console.warn("[CHAT_TRACE][emit_history_flushed] listener_ready", {
          windowLabel: tauriWindowLabel.value,
        });
      })
      .catch((error) => {
        console.error("[CHAT_TRACE][emit_history_flushed] listener_failed", error);
      });
    void listen<unknown>("easy-call:round-completed", (event) => {
      void chatFlow.handleExternalRoundCompleted(event.payload);
    })
      .then((unlisten) => {
        chatRoundCompletedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[CHAT_TRACE][emit_round_completed] listener_failed", error);
      });
    void listen<unknown>("easy-call:round-failed", (event) => {
      void chatFlow.handleExternalRoundFailed(event.payload);
    })
      .then((unlisten) => {
        chatRoundFailedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[CHAT_TRACE][emit_round_failed] listener_failed", error);
      });
    void listen<unknown>("easy-call:assistant-delta", (event) => {
      void chatFlow.handleExternalAssistantDelta(event.payload);
    })
      .then((unlisten) => {
        chatAssistantDeltaUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[CHAT_TRACE][emit_assistant_delta] listener_failed", error);
      });

    chatStreamBindHeartbeatTimer = setInterval(() => {
      if (!isChatWindowActiveNow()) return;
      const conversationId = String(currentChatConversationId.value || "").trim();
      console.warn("[CHAT_TRACE][bind_heartbeat] force_rebind", {
        windowLabel: tauriWindowLabel.value,
        conversationId,
      });
      void chatFlow.bindActiveConversationStream(conversationId, true);
    }, CHAT_STREAM_BIND_HEARTBEAT_MS);
  }
  syncChatWindowActiveState();
  window.addEventListener("focus", handleWindowFocusForStateSync);
  window.addEventListener("blur", handleWindowBlurForStateSync);
  document.addEventListener("visibilitychange", handleVisibilityForStateSync);
  window.addEventListener("focus", handleWindowFocusForMicPrewarm);
  document.addEventListener("visibilitychange", handleVisibilityForMicPrewarm);
});

onBeforeUnmount(() => {
  if (chatHistoryFlushedUnlisten) {
    chatHistoryFlushedUnlisten();
    chatHistoryFlushedUnlisten = null;
  }
  if (chatRoundCompletedUnlisten) {
    chatRoundCompletedUnlisten();
    chatRoundCompletedUnlisten = null;
  }
  if (chatRoundFailedUnlisten) {
    chatRoundFailedUnlisten();
    chatRoundFailedUnlisten = null;
  }
  if (chatAssistantDeltaUnlisten) {
    chatAssistantDeltaUnlisten();
    chatAssistantDeltaUnlisten = null;
  }
  if (chatStreamBindHeartbeatTimer) {
    clearInterval(chatStreamBindHeartbeatTimer);
    chatStreamBindHeartbeatTimer = null;
  }
  if (isChatTauriWindow.value) {
    void chatFlow.bindActiveConversationStream("");
  }
  window.removeEventListener("focus", handleWindowFocusForStateSync);
  window.removeEventListener("blur", handleWindowBlurForStateSync);
  document.removeEventListener("visibilitychange", handleVisibilityForStateSync);
  clearRecordHotkeyProbeState();
  agentWorkPresence.cleanup();
  chatWindowActiveSynced.value = null;
  if (isChatTauriWindow.value) {
    void invokeTauri("set_chat_window_active", { active: false }).catch(() => {});
  }
  window.removeEventListener("focus", handleWindowFocusForMicPrewarm);
  document.removeEventListener("visibilitychange", handleVisibilityForMicPrewarm);
});

watch(
  () => delegateConversations.value,
  (items) => {
    agentWorkPresence.seedFromDelegateConversations(items);
  },
  { deep: true, immediate: true },
);

watch(
  () => viewMode.value,
  () => {
    syncChatWindowActiveState();
  },
);

watch(
  () => ({
    apiId: assistantDepartmentApiConfigId.value,
    imageEnabled: !!assistantDepartmentApiConfig.value?.enableImage,
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
    apiId: assistantDepartmentApiConfigId.value,
    agentId: activeAssistantAgentId.value,
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

async function openGithubRepository() {
  try {
    const url = await invokeTauri<string>("get_project_repository_url");
    void invokeTauri("open_external_url", { url });
  } catch (error) {
    console.warn("[ABOUT] resolve project repository failed:", error);
  }
}

function closeConfigSaveErrorDialog() {
  configSaveErrorDialogOpen.value = false;
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


const chatFlow = useChatFlow({
  chatting,
  forcingArchive,
  getSession: () => {
    const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
    const agentId = String(activeAssistantAgentId.value || "").trim();
    if (!apiConfigId || !agentId) return null;
    return { apiConfigId, agentId };
  },
  getConversationId: () => String(currentChatConversationId.value || "").trim(),
  chatInput,
  clipboardImages,
  queuedAttachmentNotices,
  latestUserText,
  latestUserImages,
  latestAssistantText,
  latestReasoningStandardText,
  latestReasoningInlineText,
  toolStatusText,
  toolStatusState,
  streamToolCalls,
  chatErrorText,
  allMessages,
  visibleMessageBlockCount,
  t: tr,
  formatRequestFailed: (error) => formatI18nError(tr, "status.requestFailed", error),
  removeBinaryPlaceholders,
  invokeSendChatMessage: ({ text, displayText, images, attachments, extraTextBlocks, session, onDelta }) =>
    invokeTauri("send_chat_message", {
      input: {
        payload: {
          text,
          displayText,
          images,
          attachments: attachments && attachments.length > 0 ? attachments : undefined,
          extraTextBlocks: extraTextBlocks && extraTextBlocks.length > 0 ? extraTextBlocks : undefined,
        },
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
          conversationId: session.conversationId || null,
        },
      },
      onDelta,
    }),
  invokeStopChatMessage: ({ session, partialAssistantText, partialReasoningStandard, partialReasoningInline }) =>
    invokeTauri<{
      aborted: boolean;
      persisted: boolean;
      conversationId?: string | null;
      assistantText?: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      assistantMessage?: ChatMessage;
    }>("stop_chat_message", {
      input: {
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
          conversationId: session.conversationId || null,
        },
        partialAssistantText,
        partialReasoningStandard,
        partialReasoningInline,
      },
    }),
  invokeBindActiveChatViewStream: ({ conversationId, onDelta }) =>
    invokeTauri("bind_active_chat_view_stream", {
      input: {
        conversationId: conversationId || null,
      },
      onDelta,
    }),
  onReloadMessages: () => loadAllMessages(),
  onHistoryFlushed: async ({ conversationId, pendingMessages, activateAssistant }) => {
    const flushedConversationId = String(conversationId || "").trim();
    console.warn("[CHAT_TRACE][onHistoryFlushed] start", {
      windowLabel: tauriWindowLabel.value,
      flushedConversationId,
      activateAssistant,
      pendingCount: Array.isArray(pendingMessages) ? pendingMessages.length : 0,
      currentConversationId: String(currentChatConversationId.value || "").trim(),
      currentMessageCount: allMessages.value.length,
    });
    if (flushedConversationId) {
      currentChatConversationId.value = flushedConversationId;
    }
    // 激活助理的批次：清屏并原子重放；非激活批次：仅顺序追加，不清屏。
    const queueMessages = Array.isArray(pendingMessages) ? pendingMessages : [];
    if (activateAssistant) {
      allMessages.value = [];
      await nextTick();
      allMessages.value = [...queueMessages];
      hasMoreBackendHistory.value = queueMessages.length > 0;
      console.warn("[CHAT_TRACE][onHistoryFlushed] activate_replace_done", {
        windowLabel: tauriWindowLabel.value,
        replacedCount: queueMessages.length,
        finalMessageCount: allMessages.value.length,
      });
    } else if (queueMessages.length > 0) {
      const existing = allMessages.value;
      const dedup = new Set(existing.map((m) => String(m.id || "").trim()).filter((id) => !!id));
      const beforeDedupCount = queueMessages.length;
      const appended = queueMessages.filter((m) => {
        const id = String(m.id || "").trim();
        if (!id) return true;
        if (dedup.has(id)) return false;
        dedup.add(id);
        return true;
      });
      allMessages.value = [...existing, ...appended];
      hasMoreBackendHistory.value = existing.length > 0 || appended.length > 0;
      console.warn("[CHAT_TRACE][onHistoryFlushed] append_done", {
        windowLabel: tauriWindowLabel.value,
        beforeDedupCount,
        appendedCount: appended.length,
        droppedAsDuplicate: beforeDedupCount - appended.length,
        previousMessageCount: existing.length,
        finalMessageCount: allMessages.value.length,
        firstAppendedId: String(appended[0]?.id || ""),
        lastAppendedId: String(appended[appended.length - 1]?.id || ""),
      });
    } else {
      console.warn("[CHAT_TRACE][onHistoryFlushed] no_pending_messages", {
        windowLabel: tauriWindowLabel.value,
        activateAssistant,
        finalMessageCount: allMessages.value.length,
      });
    }
    await loadUnarchivedConversations();
    console.warn("[CHAT_TRACE][onHistoryFlushed] done", {
      windowLabel: tauriWindowLabel.value,
      flushedConversationId: String(currentChatConversationId.value || "").trim(),
      finalMessageCount: allMessages.value.length,
    });
  },
});

watch(
  () => String(currentChatConversationId.value || "").trim(),
  (conversationId) => {
    if (!isChatTauriWindow.value) return;
    void chatFlow.bindActiveConversationStream(conversationId);
  },
  { immediate: true },
);

function clearStreamBuffer() {
  chatFlow.clearStreamBuffer();
}

function buildPersonasSnapshotJson() {
  return JSON.stringify(
    personas.value.map((item) => ({
      id: item.id,
      name: item.name,
      systemPrompt: item.systemPrompt,
      privateMemoryEnabled: !!item.privateMemoryEnabled,
      avatarPath: item.avatarPath || "",
      avatarUpdatedAt: item.avatarUpdatedAt || "",
      isBuiltInUser: !!item.isBuiltInUser,
      isBuiltInSystem: !!item.isBuiltInSystem,
      source: item.source || "",
      scope: item.scope || "",
      tools: (item.tools || []).map((tool) => ({
        id: tool.id,
        enabled: !!tool.enabled,
        command: tool.command || "",
        args: Array.isArray(tool.args) ? [...tool.args] : [],
        values: tool.values ?? null,
      })),
    })),
  );
}
const {
  deleteUnarchivedConversation,
  handleRecallTurn,
  handleRegenerateTurn,
} = useChatRewindActions({
  activeApiConfigId: assistantDepartmentApiConfigId,
  activeAgentId: activeAssistantAgentId,
  currentConversationId: currentChatConversationId,
  allMessages,
  visibleMessageBlockCount,
  chatting,
  forcingArchive,
  chatInput,
  clipboardImages,
  deleteUnarchivedConversationFromArchives,
  sendChat: chatFlow.sendChat,
  setStatusError,
  removeBinaryPlaceholders,
  messageText,
  extractMessageImages,
});

function handleToolsChanged() {
  if (selectedPersonaEditor.value?.source === "private_workspace") {
    setStatus(t("config.tools.privateWorkspaceReadonly"));
    return;
  }
  if (configTab.value === "tools") {
    void refreshToolsStatus();
  }
}

async function saveChatSettingsNow() {
  await saveConversationApiSettings();
  await saveChatPreferences();
}
const { openCurrentHistory, openPromptPreview, openSystemPromptPreview } = useChatDialogActions({
  activeChatApiConfigId: assistantDepartmentApiConfigId,
  assistantDepartmentAgentId: activeAssistantAgentId,
  openPromptPreviewDialog,
  openSystemPromptPreviewDialog,
});

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
  viewMode,
  syncAlwaysOnTop,
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
  assistantDepartmentAgentId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  selectedApiConfig,
  toolApiConfig,
  activeChatApiConfigId: assistantDepartmentApiConfigId,
  suppressChatReloadWatch,
  modelRefreshError,
  toolStatuses,
  defaultApiTools,
  t: tr,
  normalizeApiBindingsLocal,
  syncUserAliasFromPersona,
  syncTrayIcon,
  saveChatSettings: saveChatSettingsNow,
  refreshToolsStatus,
  refreshImageCacheStats,
  refreshConversationHistory,
  resetVisibleTurnCount: () => {
    resetVisibleMessageBlocksByCurrentMessages();
  },
});
</script>
