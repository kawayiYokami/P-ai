
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
      :update-selected-persona-id="(value) => { assistantDepartmentAgentId.value = value; }"
      :update-selected-response-style-id="(value) => { selectedResponseStyleId = value; }"
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
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useConfigPersistence, type ConfigSaveErrorInfo } from "./features/config/composables/use-config-persistence";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useAgentWorkPresence } from "./features/chat/composables/use-agent-work-presence";
import { useArchivesView, type ArchiveImportPreview } from "./features/chat/composables/use-archives-view";
import { useAvatarCache } from "./features/chat/composables/use-avatar-cache";
import { useChatDialogActions } from "./features/chat/composables/use-chat-dialog-actions";
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
  ApiConfigItem,
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
  departments: [],
  apiConfigs: [],
});
const recordHotkeyProbeLastSeq = ref(0);
const recordHotkeyProbeDown = ref(false);
const chatWindowActiveSynced = ref<boolean | null>(null);
const configTab = ref<"hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "memory" | "task" | "logs" | "appearance" | "about">("hotkey");
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
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);

const allMessages = shallowRef<ChatMessage[]>([]);
const visibleMessageBlockCount = ref(1);

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
  const active = isChatWindowActiveNow();
  if (chatWindowActiveSynced.value === active) return;
  chatWindowActiveSynced.value = active;
  if (active) {
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
  personas.value.filter((p) => !p.isBuiltInUser && !p.isBuiltInSystem && p.id !== "user-persona" && p.id !== "system-persona"),
);
const assistantDepartmentPersona = computed(
  () =>
    assistantPersonas.value.find((p) => p.id === assistantDepartmentAgentId.value)
    ?? assistantPersonas.value[0]
    ?? null,
);
const activeAssistantAgentId = computed(() => assistantDepartmentAgentId.value);
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
  activeChatApiConfig: assistantDepartmentApiConfig,
  perfDebug: PERF_DEBUG,
  perfNow,
});
const displayMessageBlocks = computed(() => visibleMessageBlocks.value);
const displayHasMoreMessageBlocks = computed(() => hasMoreMessageBlocks.value);
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

function updatePersonaEditorIdWithNotice(value: string) {
  const nextId = String(value || "").trim();
  if (!nextId || nextId === personaEditorId.value) return;
  if (personaDirty.value) {
    const currentName = String(selectedPersonaEditor.value?.name || personaEditorId.value || "").trim() || t("config.persona.title");
    status.value = t("status.personaUnsavedSwitchHint", { name: currentName });
  }
  personaEditorId.value = nextId;
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

const { suppressChatReloadWatch, refreshAllViewData, handleWindowRefreshSignal } = useViewRefresh({
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
    visibleMessageBlockCount.value = 1;
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
      visibleMessageBlockCount.value = 1;
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
      visibleMessageBlockCount.value = 1;
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
  syncChatWindowActiveState();
  window.addEventListener("focus", handleWindowFocusForStateSync);
  window.addEventListener("blur", handleWindowBlurForStateSync);
  document.addEventListener("visibilitychange", handleVisibilityForStateSync);
  window.addEventListener("focus", handleWindowFocusForMicPrewarm);
  document.addEventListener("visibilitychange", handleVisibilityForMicPrewarm);
});

onBeforeUnmount(() => {
  void chatFlow.bindActiveConversationStream("");
  window.removeEventListener("focus", handleWindowFocusForStateSync);
  window.removeEventListener("blur", handleWindowBlurForStateSync);
  document.removeEventListener("visibilitychange", handleVisibilityForStateSync);
  clearRecordHotkeyProbeState();
  agentWorkPresence.cleanup();
  chatWindowActiveSynced.value = null;
  void invokeTauri("set_chat_window_active", { active: false }).catch(() => {});
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
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  const agentId = String(activeAssistantAgentId.value || "").trim();
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
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  const agentId = String(activeAssistantAgentId.value || "").trim();
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
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  const agentId = String(activeAssistantAgentId.value || "").trim();
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
    const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
    const agentId = String(activeAssistantAgentId.value || "").trim();
    if (!apiConfigId || !agentId) return null;
    return { apiConfigId, agentId };
  },
  getConversationId: () => String(currentChatConversationId.value || "").trim(),
  chatInput,
  clipboardImages,
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
  invokeSendChatMessage: ({ text, images, session, onDelta }) =>
    invokeTauri("send_chat_message", {
      input: {
        payload: { text, images },
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
          conversationId: session.conversationId || null,
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
  onHistoryFlushed: async ({ conversationId, pendingMessages }) => {
    const flushedConversationId = String(conversationId || "").trim();
    if (flushedConversationId) {
      currentChatConversationId.value = flushedConversationId;
    }
    // 优先合并前端本地批次：history_flushed 到达后先把前端已确认批次并入当前视图，
    // 避免流式开始前还阻塞等待后端全量历史。
    {
      const current = allMessages.value;
      const mergedMap = new Map<string, ChatMessage>();
      for (const message of current) mergedMap.set(message.id, message);
      for (const message of pendingMessages || []) mergedMap.set(message.id, message);
      const merged = [...current];
      for (const message of pendingMessages || []) {
        if (!merged.some((item) => item.id === message.id)) merged.push(message);
      }
      allMessages.value = merged.map((item) => mergedMap.get(item.id) as ChatMessage);
    }
    await loadUnarchivedConversations();
  },
});

watch(
  () => String(currentChatConversationId.value || "").trim(),
  (conversationId) => {
    void chatFlow.bindActiveConversationStream(conversationId);
  },
  { immediate: true },
);

function clearStreamBuffer() {
  chatFlow.clearStreamBuffer();
}

type RewindConversationResult = {
  removedCount: number;
  remainingCount: number;
  recalledUserMessage?: ChatMessage;
};

function resetVisibleBlocksAfterRewind(nextMessages: ChatMessage[]) {
  const assistantIndices: number[] = [];
  for (let i = 0; i < nextMessages.length; i += 1) {
    if (nextMessages[i]?.role === "assistant") assistantIndices.push(i);
  }
  if (assistantIndices.length < 2) {
    visibleMessageBlockCount.value = Math.max(1, nextMessages.length || 1);
    return;
  }
  const startIndex = assistantIndices[assistantIndices.length - 2];
  visibleMessageBlockCount.value = Math.max(1, nextMessages.length - startIndex);
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

async function rewindConversationFromTurn(turnId: string): Promise<ChatMessage | null> {
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  const agentId = String(activeAssistantAgentId.value || "").trim();
  const conversationId = String(currentChatConversationId.value || "").trim();
  const messageId = String(turnId || "").trim();
  if (!apiConfigId || !agentId || !messageId) return null;
  const currentMessages = [...allMessages.value];
  try {
    const result = await invokeTauri<RewindConversationResult>("rewind_conversation_from_message", {
      input: {
        session: {
          apiConfigId,
          agentId,
          conversationId: conversationId || null,
        },
        messageId,
      },
    });
    const keepCount = Math.max(0, Math.min(currentMessages.length, Number(result.remainingCount) || 0));
    const nextMessages = currentMessages.slice(0, keepCount);
    allMessages.value = nextMessages;
    resetVisibleBlocksAfterRewind(nextMessages);
    return result.recalledUserMessage
      ?? currentMessages[keepCount]
      ?? currentMessages.find((item) => item.id === messageId && item.role === "user")
      ?? null;
  } catch (error) {
    setStatusError("status.rewindConversationFailed", error);
    return null;
  }
}

async function deleteUnarchivedConversation(conversationId: string) {
  await deleteUnarchivedConversationFromArchives(conversationId);
  if (String(currentChatConversationId.value || "").trim() === String(conversationId || "").trim()) {
    currentChatConversationId.value = "";
    allMessages.value = [];
    visibleMessageBlockCount.value = 1;
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
    visibleMessageBlockCount.value = 1;
  },
});
</script>
