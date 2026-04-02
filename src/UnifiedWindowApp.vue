
<template>
  <div class="window-shell text-sm bg-base-200">
    <AppWindowHeader
      :view-mode="viewMode"
      :title-text="titleText"
      :chat-usage-percent="chatUsagePercent"
      :forcing-archive="forcingArchive"
      :chatting="chatting"
      :always-on-top="alwaysOnTop"
      :maximized="maximized"
      :window-ready="windowReady"
      :force-archive-tip="t('chat.forceArchiveTip')"
      :always-on-top-on-title="t('chat.alwaysOnTopOn')"
      :always-on-top-off-title="t('chat.alwaysOnTopOff')"
      :open-config-title="t('window.configTitle')"
      :open-logs-title="'运行日志'"
      :close-title="t('common.close')"
      @force-archive="openForceArchiveActionDialog"
      @toggle-always-on-top="toggleAlwaysOnTop"
      @minimize-window="minimizeWindowAndClearForeground"
      @toggle-maximize-window="toggleMaximizeWindow"
      @open-config="openConfigWindow"
      @open-archives="openCurrentHistory"
      @open-runtime-logs="openRuntimeLogsDialog"
      @close-window="closeWindowAndClearForeground"
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
      :selected-pdf-read-mode="selectedPdfReadMode"
      :background-voice-screenshot-keywords="backgroundVoiceScreenshotKeywords"
      :background-voice-screenshot-mode="backgroundVoiceScreenshotMode"
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
      :latest-own-message-align-request="latestOwnMessageAlignRequest"
      :conversation-scroll-to-bottom-request="conversationScrollToBottomRequest"
      :current-chat-conversation-id="currentChatConversationId"
      :chat-unarchived-conversation-items="chatUnarchivedConversationItems"
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
      :remote-im-contact-conversations="remoteImContactConversations"
      :selected-remote-im-contact-id="selectedRemoteImContactId"
      :remote-im-contact-messages="remoteImContactMessages"
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
      :load-prompt-preview="loadPromptPreview"
      :set-memory-dialog-ref="setMemoryDialogRef"
      :set-prompt-preview-dialog-ref="setPromptPreviewDialogRef"
      :set-status="setStatus"
      :update-config-tab="(value) => { configTab = value; }"
      :set-ui-language="setUiLanguage"
      :update-persona-editor-id="updatePersonaEditorIdWithNotice"
      :update-selected-persona-id="updateAssistantDepartmentAgentId"
      :update-selected-response-style-id="updateSelectedResponseStyleId"
      :update-selected-pdf-read-mode="updateSelectedPdfReadMode"
      :update-background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      :update-background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      :save-chat-settings="saveChatSettingsNow"
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
      :open-runtime-logs="openRuntimeLogsDialog"
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
      :on-reached-chat-bottom="trimForegroundMessagesToRecentLimit"
      :on-recall-turn="handleRecallTurn"
      :on-regenerate-turn="handleRegenerateTurn"
      :on-lock-chat-workspace="lockChatWorkspaceFromPicker"
      :on-unlock-chat-workspace="unlockChatWorkspace"
      :on-switch-conversation="switchUnarchivedConversation"
      :on-create-conversation="createUnarchivedConversation"
      :on-open-skill-panel="openSkillPlaceholderDialog"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :select-unarchived-conversation="selectUnarchivedConversation"
      :select-delegate-conversation="selectDelegateConversation"
      :select-remote-im-contact-conversation="selectRemoteImContactConversation"
      :export-archive="exportArchive"
      :import-archive-file="prepareArchiveImport"
      :delete-archive="deleteArchive"
      :delete-unarchived-conversation="deleteUnarchivedConversation"
      :delete-remote-im-contact-conversation="deleteRemoteImContactConversation"
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
    <RuntimeLogsDialog
      :open="runtimeLogsDialogOpen"
      :logs="runtimeLogs"
      :loading="runtimeLogsLoading"
      :error-text="runtimeLogsError"
      @close="closeRuntimeLogsDialog"
      @refresh="refreshRuntimeLogs"
      @clear="clearRuntimeLogs"
    />
    <dialog class="modal" :class="{ 'modal-open': rewindConfirmDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">撤回选项</h3>
        <div class="mt-2 text-sm opacity-80">请选择本次撤回要执行的范围：</div>
        <div class="mt-4 flex flex-col items-center gap-2">
          <button
            v-if="rewindConfirmCanUndoPatch"
            class="btn btn-sm w-full"
            :class="rewindConfirmCanUndoPatch ? 'btn-error' : ''"
            @click="confirmRewindWithPatch"
          >
            撤回消息并撤回修改
          </button>
          <button class="btn btn-sm w-full" @click="confirmRewindMessageOnly">
            仅撤回消息
          </button>
          <button class="btn btn-sm btn-primary w-full" @click="cancelRewindConfirm">取消</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="cancelRewindConfirm">close</button>
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
    <dialog class="modal" :class="{ 'modal-open': forceArchiveActionDialogOpen }">
      <div class="modal-box max-w-md">
        <h3 class="font-semibold text-base">处理当前会话</h3>
        <div v-if="forceArchivePreviewLoading" class="mt-3 text-sm opacity-70">正在判断当前会话是否适合压缩或归档...</div>
        <template v-else>
          <div class="mt-3 rounded-box border border-base-300 bg-base-200/40 px-3 py-3 text-sm">
            <div class="font-medium">压缩</div>
            <div class="mt-1 opacity-80">整理较早历史，保留当前会话继续聊。</div>
            <div class="mt-2 text-xs opacity-70">适合上下文占用偏高，但你还想继续当前话题时使用。</div>
            <div
              v-if="forceCompactionPreview?.compactionDisabledReason"
              class="mt-3 rounded border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-warning-content"
            >
              {{ forceCompactionPreview.compactionDisabledReason }}
            </div>
          </div>
          <div class="mt-3 rounded-box border border-base-300 bg-base-200/40 px-3 py-3 text-sm">
            <div class="font-medium">归档</div>
            <div class="mt-1 opacity-80">生成摘要并提炼记忆，保留为归档记录。</div>
            <div class="mt-2 text-xs opacity-70">适合这段会话已经结束，准备沉淀为历史记录时使用。</div>
            <div
              v-if="forceArchivePreview?.archiveDisabledReason"
              class="mt-3 rounded border border-warning/30 bg-warning/10 px-3 py-2 text-sm text-warning-content"
            >
              {{ forceArchivePreview.archiveDisabledReason }}
            </div>
          </div>
          <div class="mt-3 text-sm opacity-80">
            <div>当前会话消息数：{{ forceArchivePreview?.messageCount ?? 0 }}</div>
            <div>助理是否已回复：{{ forceArchivePreview?.hasAssistantReply ? "是" : "否" }}</div>
            <div>当前上下文占用：{{ forceCompactionPreview?.contextUsagePercent ?? 0 }}%</div>
          </div>
        </template>
        <div class="modal-action">
          <button
            class="btn btn-sm btn-primary"
            :disabled="forceArchivePreviewLoading || !forceCompactionPreview?.canCompact || forcingArchive"
            @click="confirmForceCompactionAction"
          >
            压缩
          </button>
          <button
            class="btn btn-sm btn-secondary"
            :disabled="forceArchivePreviewLoading || !forceArchivePreview?.canArchive || forcingArchive"
            @click="confirmForceArchiveAction"
          >
            归档
          </button>
          <button class="btn btn-sm" :disabled="forceArchivePreviewLoading || forcingArchive" @click="closeForceArchiveActionDialog">
            {{ t("common.cancel") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeForceArchiveActionDialog">close</button>
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
import { inspectUndoablePatchCalls } from "./utils/chat-message-semantics";
import { formatI18nError } from "./utils/error";
import AppWindowContent from "./features/shell/components/AppWindowContent.vue";
import AppWindowHeader from "./features/shell/components/AppWindowHeader.vue";
import RuntimeLogsDialog from "./features/shell/components/RuntimeLogsDialog.vue";
import type {
  PersonaProfile,
  AppConfig,
  ChatMessage,
  ChatPersonaPresenceChip,
  ImageTextCacheStats,
  RuntimeLogEntry,
  ResponseStyleOption,
  ToolLoadStatus,
  UnarchivedConversationSummary,
} from "./types/app";
import responseStylesJson from "./constants/response-styles.json";
import { normalizeLocale } from "./i18n";

const props = withDefaults(defineProps<{ fixedViewMode?: "chat" | "archives" | "config" }>(), {
  fixedViewMode: undefined,
});

const DRAFT_ASSISTANT_ID_PREFIX = "__draft_assistant__:";
const FOREGROUND_RECENT_MESSAGE_LIMIT = 50;
const FOREGROUND_MESSAGE_TRIM_THRESHOLD = 80;
const BACKGROUND_CONVERSATION_CACHE_LIMIT = FOREGROUND_RECENT_MESSAGE_LIMIT;
type BackgroundConversationBadgeState = "completed" | "failed";
type ForegroundPaintTrace = {
  id: number;
  conversationId: string;
  startedAt: number;
};
type ConversationMessagesAfterSyncedPayload = {
  requestId?: string;
  conversationId?: string;
  afterMessageId?: string | null;
  messages?: ChatMessage[];
  fallbackMode?: string | null;
  error?: string | null;
};
type ForceArchivePreviewResult = {
  conversationId: string;
  canArchive: boolean;
  canDiscard: boolean;
  messageCount: number;
  hasAssistantReply: boolean;
  isEmpty: boolean;
  archiveDisabledReason?: string | null;
};
type ForceCompactionPreviewResult = {
  conversationId: string;
  canCompact: boolean;
  messageCount: number;
  hasAssistantReply: boolean;
  isEmpty: boolean;
  contextUsagePercent: number;
  compactionDisabledReason?: string | null;
};

const viewMode = ref<"chat" | "archives" | "config">(props.fixedViewMode ?? "config");
const { t, locale } = useI18n();
const tr = (key: string, params?: Record<string, unknown>) => (params ? t(key, params) : t(key));
const isMacPlatform = /Mac|iPhone|iPad|iPod/i.test(window.navigator.platform || "");
const {
  windowReady,
  alwaysOnTop,
  maximized,
  initWindow,
  syncWindowControlsState,
  closeWindow,
  toggleAlwaysOnTop,
  minimizeWindow,
  toggleMaximizeWindow,
} =
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
let chatHistoryFlushedUnlisten: UnlistenFn | null = null;
let chatRoundCompletedUnlisten: UnlistenFn | null = null;
let chatRoundFailedUnlisten: UnlistenFn | null = null;
let chatAssistantDeltaUnlisten: UnlistenFn | null = null;
let chatConversationMessagesAfterSyncedUnlisten: UnlistenFn | null = null;
let chatConversationOverviewUpdatedUnlisten: UnlistenFn | null = null;
let foregroundPaintTraceSeq = 0;
let chatWindowActiveSyncTimer: ReturnType<typeof setTimeout> | null = null;
let chatMicPrewarmTimer: ReturnType<typeof setTimeout> | null = null;
let foregroundConversationCacheRaf = 0;
const configTab = ref<"hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about">("hotkey");
const personas = ref<PersonaProfile[]>([]);
const assistantDepartmentAgentId = ref("default-agent");
const personaEditorId = ref("default-agent");
const userAlias = ref(t("archives.roleUser"));
const selectedResponseStyleId = ref("concise");
const selectedPdfReadMode = ref<"text" | "image">("image");
const backgroundVoiceScreenshotKeywords = ref("");
const backgroundVoiceScreenshotMode = ref<"desktop" | "focused_window">("focused_window");
const chatInput = ref("");
const currentChatConversationId = ref("");
const conversationForegroundSyncing = ref(false);
const backgroundConversationBadgeMap = ref<Record<string, BackgroundConversationBadgeState>>({});
const conversationMessageCache = ref<Record<string, ChatMessage[]>>({});
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const latestReasoningStandardText = ref("");
const latestReasoningInlineText = ref("");
const latestOwnMessageAlignRequest = ref(0);
const toolStatusText = ref("");
const toolStatusState = ref<"running" | "done" | "failed" | "">("");
const streamToolCalls = ref<Array<{ name: string; argsText: string }>>([]);
const chatErrorText = ref("");
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>([]);

const allMessages = shallowRef<ChatMessage[]>([]);

const status = ref("Ready.");
const runtimeLogsDialogOpen = ref(false);
const runtimeLogs = ref<RuntimeLogEntry[]>([]);
const runtimeLogsLoading = ref(false);
const runtimeLogsError = ref("");
const configSaveErrorDialogOpen = ref(false);
const configSaveErrorDialogTitle = ref("");
const configSaveErrorDialogBody = ref("");
const configSaveErrorDialogKind = ref<"warning" | "error">("error");
const terminalApprovalQueue = ref<TerminalApprovalRequestPayload[]>([]);
const terminalApprovalResolving = ref(false);
const skillPlaceholderDialogOpen = ref(false);
const forceArchiveActionDialogOpen = ref(false);
const forceArchivePreviewLoading = ref(false);
const forceArchivePreview = ref<ForceArchivePreviewResult | null>(null);
const forceCompactionPreview = ref<ForceCompactionPreviewResult | null>(null);
const rewindConfirmDialogOpen = ref(false);
const rewindConfirmCanUndoPatch = ref(false);
const rewindConfirmUndoHint = ref("");
let rewindConfirmResolver: ((mode: "with_patch" | "message_only" | "cancel") => void) | null = null;
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
const CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS = 260;
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
  loadPromptPreview,
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
  remoteImContactConversations,
  remoteImContactMessages,
  selectedRemoteImContactId,
  loadArchives,
  loadDelegateConversations,
  selectArchive,
  selectUnarchivedConversation,
  selectDelegateConversation,
  selectRemoteImContactConversation,
  deleteUnarchivedConversation: deleteUnarchivedConversationFromArchives,
  deleteRemoteImContactConversation,
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

function clearChatWindowActiveSyncTimer() {
  if (!chatWindowActiveSyncTimer) return;
  clearTimeout(chatWindowActiveSyncTimer);
  chatWindowActiveSyncTimer = null;
}

function clearChatMicPrewarmTimer() {
  if (!chatMicPrewarmTimer) return;
  clearTimeout(chatMicPrewarmTimer);
  chatMicPrewarmTimer = null;
}

function clearForegroundConversationCacheRaf() {
  if (!foregroundConversationCacheRaf) return;
  cancelAnimationFrame(foregroundConversationCacheRaf);
  foregroundConversationCacheRaf = 0;
}

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
        || a.requestFormat === "anthropic"
      ),
  ),
);
const imageCapableApiConfigs = computed(() => config.apiConfigs.filter((a) => a.enableImage));
const sttCapableApiConfigs = computed(() =>
  config.apiConfigs.filter((a) => a.requestFormat === "openai_stt"),
);
const MIN_RECORD_SECONDS = 1;
const MAX_MIN_RECORD_SECONDS = 30;
const DEFAULT_MAX_RECORD_SECONDS = 60;
const MAX_RECORD_SECONDS = 600;
const DEFAULT_TOOL_MAX_ITERATIONS = 10;
const MAX_TOOL_MAX_ITERATIONS = 100;

function normalizeRuntimeConfigNumbers(
  minValue: unknown,
  maxValue: unknown,
  toolValue: unknown,
  fallback?: {
    minRecordSeconds?: number;
    maxRecordSeconds?: number;
    toolMaxIterations?: number;
  },
): { minRecordSeconds: number; maxRecordSeconds: number; toolMaxIterations: number } {
  const fallbackMin = Number(fallback?.minRecordSeconds);
  const fallbackMax = Number(fallback?.maxRecordSeconds);
  const fallbackTool = Number(fallback?.toolMaxIterations);
  const nextMin = Number(minValue);
  const nextMax = Number(maxValue);
  const nextTool = Number(toolValue);
  const resolvedMin = Number.isFinite(nextMin)
    ? nextMin
    : (Number.isFinite(fallbackMin) ? fallbackMin : MIN_RECORD_SECONDS);
  const minRecordSeconds = Math.max(
    MIN_RECORD_SECONDS,
    Math.min(MAX_MIN_RECORD_SECONDS, Math.round(resolvedMin)),
  );
  const resolvedMax = Number.isFinite(nextMax)
    ? nextMax
    : (Number.isFinite(fallbackMax) ? fallbackMax : DEFAULT_MAX_RECORD_SECONDS);
  const maxRecordSeconds = Math.max(
    minRecordSeconds,
    Math.min(MAX_RECORD_SECONDS, Math.round(resolvedMax)),
  );
  const resolvedTool = Number.isFinite(nextTool)
    ? nextTool
    : (Number.isFinite(fallbackTool) ? fallbackTool : DEFAULT_TOOL_MAX_ITERATIONS);
  const toolMaxIterations = Math.max(
    1,
    Math.min(MAX_TOOL_MAX_ITERATIONS, Math.round(resolvedTool)),
  );
  return { minRecordSeconds, maxRecordSeconds, toolMaxIterations };
}

const assistantDepartmentApiConfigId = computed(
  () => String(config.assistantDepartmentApiConfigId || "").trim(),
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
  onTranscribed: async ({ source, text }) => {
    const wasBackgroundWake = !isChatWindowActiveNow();
    if (wasBackgroundWake) {
      const startedAt = Date.now();
      const keywords = parseBackgroundVoiceScreenshotKeywords(backgroundVoiceScreenshotKeywords.value);
      const recognizedText = String(text || "").trim();
      const matched = matchBackgroundVoiceScreenshotKeyword(recognizedText, keywords);
      if (!matched) {
        console.info(
          "[后台语音截图] 跳过：未命中关键词，关键词数=%d，转写长度=%d",
          keywords.length,
          recognizedText.length,
        );
      } else {
        await queueAutoScreenshotFromVoice({
          source,
          keyword: matched,
          mode: backgroundVoiceScreenshotMode.value,
          startedAt,
        });
      }
      void invokeTauri("show_chat_window").catch((error) => {
        console.warn("[音频] 打开聊天窗口失败:", error);
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

async function tryPrewarmChatMic(reason: string) {
  if (viewMode.value !== "chat") return;
  if (document.visibilityState === "hidden") return;
  if (!document.hasFocus()) return;
  void reason;
  await prewarmMicrophone();
}

function isChatWindowActiveNow(): boolean {
  return viewMode.value === "chat" && document.visibilityState === "visible" && document.hasFocus();
}

function clearRecordHotkeyProbeState() {
  recordHotkeyProbeDown.value = false;
  recordHotkeyProbeLastSeq.value = 0;
}

function scheduleChatWindowActiveStateSync(reason: string, delayMs = 0) {
  if (!isChatTauriWindow.value) return;
  clearChatWindowActiveSyncTimer();
  if (delayMs <= 0) {
    syncChatWindowActiveState(reason);
    return;
  }
  chatWindowActiveSyncTimer = setTimeout(() => {
    chatWindowActiveSyncTimer = null;
    syncChatWindowActiveState(reason);
  }, delayMs);
}

function scheduleChatMicPrewarm(reason: string, delayMs = 0) {
  clearChatMicPrewarmTimer();
  if (delayMs <= 0) {
    void tryPrewarmChatMic(reason);
    return;
  }
  chatMicPrewarmTimer = setTimeout(() => {
    chatMicPrewarmTimer = null;
    void tryPrewarmChatMic(reason);
  }, delayMs);
}

function syncChatWindowActiveState(reason = "unknown") {
  if (!isChatTauriWindow.value) return;
  const active = isChatWindowActiveNow();
  if (chatWindowActiveSynced.value === active) return;
  chatWindowActiveSynced.value = active;
  if (active) {
    void stopRecording(false);
    const activeConversationId = String(currentChatConversationId.value || "").trim();
    if (!activeConversationId) {
      void refreshChatUnarchivedConversations()
        .catch((error) => {
          console.warn("[聊天追踪][前台会话] 激活恢复失败", error);
        });
    }
  }
  clearRecordHotkeyProbeState();
  void invokeTauri("set_chat_window_active", { active }).catch((error) => {
    console.warn("[热键] 设置聊天窗口激活状态失败:", error);
  });
}

function handleWindowFocusForStateSync() {
  scheduleChatWindowActiveStateSync("focus");
}

function handleWindowBlurForStateSync() {
  scheduleChatWindowActiveStateSync("blur");
}

function handleVisibilityForStateSync() {
  clearChatWindowActiveSyncTimer();
  clearChatMicPrewarmTimer();
  if (isChatTauriWindow.value && document.visibilityState !== "visible") {
    freezeForegroundConversation("window_hidden");
  }
  syncChatWindowActiveState("visibilitychange");
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
// 对话颜色（跳跃分配，最大化对比度）
const CONVERSATION_COLORS = [
  'primary',   // 0: 紫
  'warning',   // 1: 黄
  'secondary', // 2: 粉
  'error',     // 3: 红
  'accent',    // 4: 青
  'info',      // 5: 蓝
  'success',   // 6: 绿
  'neutral',   // 7: 黑
] as const;

const conversationScrollToBottomRequest = ref(0);
let pendingConversationScrollToBottomConversationId = "";
let pendingConversationScrollToBottomTimer = 0;

type SwitchConversationSnapshot = {
  conversationId: string;
  messages: ChatMessage[];
  hasMoreHistory: boolean;
  unarchivedConversations: UnarchivedConversationSummary[];
};

type ConversationOverviewUpdatedPayload = {
  unarchivedConversations?: UnarchivedConversationSummary[];
  preferredConversationId?: string | null;
};

function clearPendingConversationScrollToBottomFallback() {
  if (pendingConversationScrollToBottomTimer) {
    window.clearTimeout(pendingConversationScrollToBottomTimer);
    pendingConversationScrollToBottomTimer = 0;
  }
}

function triggerConversationScrollToBottom(conversationId: string, reason: string) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  if (cid !== String(currentChatConversationId.value || "").trim()) return;
  conversationScrollToBottomRequest.value += 1;
  pendingConversationScrollToBottomConversationId = "";
  clearPendingConversationScrollToBottomFallback();
  console.info("[会话切换] 触发滚到底", {
    conversationId: cid,
    reason,
    request: conversationScrollToBottomRequest.value,
  });
}

function scheduleConversationScrollToBottomFallback(conversationId: string) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  pendingConversationScrollToBottomConversationId = cid;
  clearPendingConversationScrollToBottomFallback();
  pendingConversationScrollToBottomTimer = window.setTimeout(() => {
    pendingConversationScrollToBottomTimer = 0;
    if (pendingConversationScrollToBottomConversationId !== cid) return;
    triggerConversationScrollToBottom(cid, "fallback_timeout");
  }, 240);
}

const chatUnarchivedConversationItems = computed(() => {
  const items = unarchivedConversations.value
    .map((item) => ({
      conversationId: item.conversationId,
      title: item.title,
      messageCount: Number(item.messageCount || 0),
      workspaceLabel: String(item.workspaceLabel || "").trim() || "默认工作空间",
      isActive: !!item.isActive,
      isMainConversation: !!item.isMainConversation,
      runtimeState: item.runtimeState,
      updatedAt: item.lastMessageAt || item.updatedAt || "",
      previewMessages: Array.isArray(item.previewMessages) ? item.previewMessages : [],
      backgroundStatus:
        backgroundConversationBadgeMap.value[String(item.conversationId || "").trim()] || undefined,
    }))
    .sort((a, b) => String(b.updatedAt).localeCompare(String(a.updatedAt)));

  // 分配颜色（按顺序取可用颜色）
  const usedIndices = new Set<number>();
  return items.map((item, index) => {
    // 找到第一个未使用的颜色索引
    let colorIdx = 0;
    for (let i = 0; i < 8; i++) {
      if (!usedIndices.has(i)) {
        colorIdx = i;
        usedIndices.add(i);
        break;
      }
    }
    return {
      ...item,
      color: CONVERSATION_COLORS[colorIdx],
      canCreateNew: items.length < 8,
    };
  });
});
const {
  chatWorkspaceName,
  chatWorkspaceLocked,
  refreshChatWorkspaceState,
  lockChatWorkspaceFromPicker,
  unlockChatWorkspace,
} = useChatWorkspace({
  activeApiConfigId: assistantDepartmentApiConfigId,
  activeAgentId: activeAssistantAgentId,
  activeConversationId: computed(() => currentChatConversationId.value),
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
  if (format === "anthropic") return "https://api.anthropic.com";
  if (format === "openai_tts") return "https://api.openai.com/v1/audio/speech";
  if (format === "openai_stt") return "https://api.openai.com/v1";
  if (format === "openai_embedding") return "https://api.openai.com/v1";
  if (format === "openai_rerank") return "https://api.openai.com/v1";
  return "https://api.openai.com/v1";
});
const chatInputPlaceholder = computed(() => {
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
const { visibleMessageBlocks, chatContextUsageRatio, chatUsagePercent } = useChatMessageBlocks({
  allMessages,
  activeChatApiConfig: assistantDepartmentApiConfig,
  perfDebug: PERF_DEBUG,
  perfNow,
});
const displayMessageBlocks = computed(() => visibleMessageBlocks.value);
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

function trimForegroundMessagesToRecentLimit() {
  if (chatting.value) return;
  if (allMessages.value.length <= FOREGROUND_MESSAGE_TRIM_THRESHOLD) return;
  allMessages.value = allMessages.value.slice(-FOREGROUND_RECENT_MESSAGE_LIMIT);
  hasMoreBackendHistory.value = false;
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (currentConversationId) {
    cacheConversationMessages(currentConversationId, allMessages.value);
  }
  console.warn("[聊天追踪][前台会话] 已裁剪到最近消息", {
    windowLabel: tauriWindowLabel.value,
    currentConversationId,
    trimThreshold: FOREGROUND_MESSAGE_TRIM_THRESHOLD,
    recentLimit: FOREGROUND_RECENT_MESSAGE_LIMIT,
    finalMessageCount: allMessages.value.length,
  });
}

function isLocalOwnUserMessage(message?: ChatMessage | null): boolean {
  if (!message || message.role !== "user") return false;
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const origin = meta.origin as Record<string, unknown> | undefined;
  if (origin && origin.kind === "remote_im") return false;
  const speakerAgentId = String(message.speakerAgentId || meta.speakerAgentId || meta.speaker_agent_id || "").trim();
  return !speakerAgentId || speakerAgentId === "user-persona";
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

function updateSelectedPdfReadMode(value: "text" | "image") {
  selectedPdfReadMode.value = value;
}

function updateBackgroundVoiceScreenshotKeywords(value: string) {
  backgroundVoiceScreenshotKeywords.value = String(value || "").replace(/，/g, ",");
}

function updateBackgroundVoiceScreenshotMode(value: "desktop" | "focused_window") {
  backgroundVoiceScreenshotMode.value = value;
}

function parseBackgroundVoiceScreenshotKeywords(raw: string): string[] {
  return Array.from(
    new Set(
      String(raw || "")
        .split(/[,\n;，；]+/)
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

function matchBackgroundVoiceScreenshotKeyword(text: string, keywords: string[]): string | null {
  const normalize = (value: string) => String(value || "").replace(/\s+/g, "").toLocaleLowerCase();
  const target = normalize(text);
  if (!target || keywords.length === 0) return null;
  for (const keyword of keywords) {
    const normalized = normalize(keyword);
    if (!normalized) continue;
    if (target.includes(normalized)) {
      return keyword;
    }
  }
  return null;
}

async function queueAutoScreenshotFromVoice(input: {
  source: "local" | "remote";
  keyword: string;
  mode: "desktop" | "focused_window";
  startedAt: number;
}) {
  const apiConfig = assistantDepartmentApiConfig.value;
  if (!apiConfig) {
    console.warn("[后台语音截图] 跳过：当前无可用对话模型配置");
    return;
  }
  const screenshotModeLabel = input.mode === "focused_window" ? "前台窗口" : "全屏";
  try {
    let imageMime = "";
    let imageBase64 = "";
    if (input.mode === "focused_window") {
      const output = await invokeTauri<{ data?: { imageMime?: string; imageBase64?: string } }>("xcap", {
        input: {
          method: "capture_focused_window",
          args: {},
        },
      });
      imageMime = String(output?.data?.imageMime || "").trim();
      imageBase64 = String(output?.data?.imageBase64 || "").trim();
    } else {
      const output = await invokeTauri<{ imageMime?: string; imageBase64?: string }>("desktop_screenshot", {
        input: {
          mode: "desktop",
        },
      });
      imageMime = String(output?.imageMime || "").trim();
      imageBase64 = String(output?.imageBase64 || "").trim();
    }
    if (!imageBase64) {
      throw new Error("截图结果为空");
    }
    const queued = await invokeTauri<{
      mime: string;
      fileName: string;
      savedPath: string;
      attachAsMedia: boolean;
      bytesBase64?: string | null;
    }>("queue_inline_file_attachment", {
      input: {
        fileName: `voice-auto-${Date.now()}.webp`,
        mime: imageMime || "image/webp",
        bytesBase64: imageBase64,
      },
    });
    const mime = String(queued.mime || "").trim().toLowerCase();
    const imageSupported = !!apiConfig.enableImage || hasVisionFallback.value;
    const canSendAsImage =
      !!queued.attachAsMedia
      && !!String(queued.bytesBase64 || "").trim()
      && mime.startsWith("image/")
      && imageSupported;
    if (canSendAsImage) {
      clipboardImages.value.push({
        mime,
        bytesBase64: String(queued.bytesBase64 || "").trim(),
      });
    } else {
      const savedPath = String(queued.savedPath || "").trim();
      const relativePath = savedPath.replace(/\\/g, "/").replace(/^.*\/downloads\//, "downloads/");
      const fileName = String(queued.fileName || "").trim() || relativePath.split("/").pop() || "attachment";
      const id = `${relativePath || fileName}::${mime}`;
      if (!queuedAttachmentNotices.value.some((item) => item.id === id)) {
        queuedAttachmentNotices.value.push({
          id,
          fileName,
          relativePath: relativePath || savedPath || fileName,
          mime,
        });
      }
    }
    const elapsedMs = Date.now() - input.startedAt;
    console.info(
      "[后台语音截图] 完成：命中关键词=%s，模式=%s，来源=%s，耗时=%dms",
      input.keyword,
      screenshotModeLabel,
      input.source,
      elapsedMs,
    );
  } catch (error) {
    const elapsedMs = Date.now() - input.startedAt;
    console.error(
      "[后台语音截图] 失败：命中关键词=%s，模式=%s，来源=%s，耗时=%dms，原因=%s",
      input.keyword,
      screenshotModeLabel,
      input.source,
      elapsedMs,
      String(error),
    );
    setStatus(`后台语音截图失败：${String(error)}`);
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
  selectedPdfReadMode,
  backgroundVoiceScreenshotKeywords,
  backgroundVoiceScreenshotMode,
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
  refreshUnarchivedConversations: refreshChatUnarchivedConversations,
  perfNow,
  perfLog,
  perfDebug: PERF_DEBUG,
});
const {
  refreshConversationHistory,
  forceArchiveNow,
  forceCompactNow,
  loadAllMessages,
} = chatRuntime;

async function refreshChatUnarchivedConversations() {
  if (conversationForegroundSyncing.value) return;
  try {
    conversationForegroundSyncing.value = true;
    const snapshot = await requestConversationSnapshot(String(currentChatConversationId.value || "").trim() || null);
    applyConversationSnapshot(snapshot);
  } finally {
    conversationForegroundSyncing.value = false;
  }
}

function pickForegroundConversationId(candidates: UnarchivedConversationSummary[]): string {
  const target =
    candidates.find((item) => !!item.isActive)
    || candidates.find((item) => !!item.isMainConversation)
    || candidates[0];
  return String(target?.conversationId || "").trim();
}

function clearForegroundConversation(reason: string) {
  const previousConversationId = String(currentChatConversationId.value || "").trim();
  if (!previousConversationId) return;
  cacheConversationMessages(previousConversationId, allMessages.value);
  currentChatConversationId.value = "";
  allMessages.value = [];
  hasMoreBackendHistory.value = false;
  chatFlow.freezeForegroundRoundState();
  console.warn("[聊天追踪][前台会话] 已清空", {
    windowLabel: tauriWindowLabel.value,
    reason,
    previousConversationId,
  });
}

function freezeForegroundConversation(reason: string) {
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (currentConversationId) {
    cacheConversationMessages(currentConversationId, allMessages.value);
  }
  chatFlow.freezeForegroundRoundState();
  console.warn("[聊天追踪][前台会话] 已冻结", {
    windowLabel: tauriWindowLabel.value,
    reason,
    currentConversationId,
    messageCount: allMessages.value.length,
  });
}

function hasActiveForegroundConversation(conversationId?: string | null): boolean {
  if (!isChatWindowActiveNow()) return false;
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (!currentConversationId) return false;
  const targetConversationId = String(conversationId || "").trim();
  return !targetConversationId || targetConversationId === currentConversationId;
}

function formalizeConversationMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.filter((item) => !String(item?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX));
}

function freezeConversationMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.map((message) => {
    const messageId = String(message?.id || "").trim();
    if (!messageId.startsWith(DRAFT_ASSISTANT_ID_PREFIX)) {
      return message;
    }
    const providerMeta = { ...((message.providerMeta || {}) as Record<string, unknown>) };
    delete providerMeta._streaming;
    delete providerMeta._streamSegments;
    delete providerMeta._streamTail;
    return {
      ...message,
      providerMeta,
    };
  });
}

function areMessagesEquivalent(left: ChatMessage[], right: ChatMessage[]): boolean {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    const leftMessage = left[index];
    const rightMessage = right[index];
    const leftId = String(leftMessage?.id || "").trim();
    const rightId = String(rightMessage?.id || "").trim();
    if (leftId !== rightId) return false;
    const leftCreatedAt = String(leftMessage?.createdAt || "").trim();
    const rightCreatedAt = String(rightMessage?.createdAt || "").trim();
    if (leftCreatedAt !== rightCreatedAt) return false;
    const leftMeta = JSON.stringify(leftMessage?.providerMeta || {});
    const rightMeta = JSON.stringify(rightMessage?.providerMeta || {});
    if (leftMeta !== rightMeta) return false;
    const leftParts = JSON.stringify(leftMessage?.parts || []);
    const rightParts = JSON.stringify(rightMessage?.parts || []);
    if (leftParts !== rightParts) return false;
  }
  return true;
}

function beginForegroundPaintTrace(conversationId: string): ForegroundPaintTrace {
  return {
    id: ++foregroundPaintTraceSeq,
    conversationId: String(conversationId || "").trim(),
    startedAt: perfNow(),
  };
}

function logForegroundPaintTrace(
  trace: ForegroundPaintTrace,
  label: string,
  detail?: Record<string, unknown>,
) {
  const totalMs = Math.round((perfNow() - trace.startedAt) * 10) / 10;
  console.warn("[会话切换计时]", {
    traceId: trace.id,
    conversationId: trace.conversationId,
    label,
    totalMs,
    ...detail,
  });
}

function cacheConversationMessages(conversationId: string, messages: ChatMessage[]) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  const cachedMessages = freezeConversationMessages(Array.isArray(messages) ? messages : []);
  conversationMessageCache.value = {
    ...conversationMessageCache.value,
    [cid]: cachedMessages.slice(-BACKGROUND_CONVERSATION_CACHE_LIMIT),
  };
}

function inferHasMoreHistoryFromSnapshot(messages: ChatMessage[]): boolean {
  return Array.isArray(messages) && messages.length >= BACKGROUND_CONVERSATION_CACHE_LIMIT;
}

function clearConversationBadge(conversationId: string) {
  const cid = String(conversationId || "").trim();
  if (!cid || !backgroundConversationBadgeMap.value[cid]) return;
  const next = { ...backgroundConversationBadgeMap.value };
  delete next[cid];
  backgroundConversationBadgeMap.value = next;
}

function setConversationBadge(conversationId: string, status: BackgroundConversationBadgeState) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  backgroundConversationBadgeMap.value = {
    ...backgroundConversationBadgeMap.value,
    [cid]: status,
  };
}

function readConversationIdFromPayload(payload: unknown): string {
  if (!payload || typeof payload !== "object") return "";
  return String((payload as { conversationId?: unknown }).conversationId || "").trim();
}

function buildConversationMessagesAfterAnchor(conversationId: string): string | null {
  const cid = String(conversationId || "").trim();
  if (!cid) return null;
  const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[cid] || []);
  const cachedFormal = formalizeConversationMessages(cachedDisplay);
  const lastFormalMessageId = String(cachedFormal[cachedFormal.length - 1]?.id || "").trim();
  return lastFormalMessageId || null;
}

async function requestConversationMessagesAfterAsync(conversationId: string, trace?: ForegroundPaintTrace) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  const afterMessageId = buildConversationMessagesAfterAnchor(cid);
  if (trace) {
    logForegroundPaintTrace(trace, "开始请求后台异步补消息", {
      afterMessageId: afterMessageId || "",
    });
  }
  await invokeTauri<{ accepted: boolean; requestId: string }>("request_conversation_messages_after_async", {
    input: {
      conversationId: cid,
      afterMessageId,
      fallbackLimit: BACKGROUND_CONVERSATION_CACHE_LIMIT,
    },
  });
}

async function reloadForegroundConversationMessages(reason = "unknown") {
  const conversationId = String(currentChatConversationId.value || "").trim();
  if (!conversationId) {
    await loadAllMessages();
    return;
  }
  try {
    await requestConversationMessagesAfterAsync(conversationId);
  } catch (error) {
    console.warn("[会话缓存] 增量补消息失败，回退全量加载", {
      reason,
      conversationId,
      error,
    });
    await loadAllMessages();
  }
}

function mergeConversationMessagesFromSyncPayload(
  conversationId: string,
  payloadMessages: ChatMessage[],
  fallbackMode?: string | null,
) {
  const cid = String(conversationId || "").trim();
  const nextPayloadMessages = freezeConversationMessages(Array.isArray(payloadMessages) ? payloadMessages : []);
  const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[cid] || []);
  const cachedFormal = formalizeConversationMessages(cachedDisplay);
  const fallback = String(fallbackMode || "").trim();
  if (fallback === "recent_limit") {
    return nextPayloadMessages;
  }
  const merged = [...cachedFormal];
  const existingIds = new Set(merged.map((item) => String(item?.id || "").trim()).filter(Boolean));
  for (const message of nextPayloadMessages) {
    const messageId = String(message?.id || "").trim();
    if (!messageId || existingIds.has(messageId)) continue;
    existingIds.add(messageId);
    merged.push(message);
  }
  return merged.length > 0 ? merged : cachedDisplay;
}

async function applyConversationMessagesAfterSynced(payload: ConversationMessagesAfterSyncedPayload) {
  const conversationId = String(payload?.conversationId || "").trim();
  if (!conversationId) return;
  if (payload?.error) {
    console.warn("[会话缓存] 异步补消息失败", {
      conversationId,
      requestId: payload.requestId,
      error: payload.error,
    });
    return;
  }
  const nextMessages = mergeConversationMessagesFromSyncPayload(
    conversationId,
    Array.isArray(payload?.messages) ? payload.messages : [],
    payload?.fallbackMode ?? null,
  );
  cacheConversationMessages(conversationId, nextMessages);
  if (String(currentChatConversationId.value || "").trim() === conversationId) {
    if (!areMessagesEquivalent(allMessages.value, nextMessages)) {
      allMessages.value = nextMessages;
    }
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    if (pendingConversationScrollToBottomConversationId === conversationId) {
      triggerConversationScrollToBottom(conversationId, "after_synced");
    }
  }
}

function applyConversationSnapshot(snapshot: SwitchConversationSnapshot) {
  const nextConversationId = String(snapshot.conversationId || "").trim();
  const nextMessages = freezeConversationMessages(Array.isArray(snapshot.messages) ? snapshot.messages : []);
  currentChatConversationId.value = nextConversationId;
  allMessages.value = nextMessages;
  hasMoreBackendHistory.value = !!snapshot.hasMoreHistory;
  cacheConversationMessages(nextConversationId, nextMessages);
  clearConversationBadge(nextConversationId);
  unarchivedConversations.value = Array.isArray(snapshot.unarchivedConversations)
    ? snapshot.unarchivedConversations
    : [];
  scheduleConversationScrollToBottomFallback(nextConversationId);
}

function applyConversationOverviewPayload(payload?: ConversationOverviewUpdatedPayload | null) {
  unarchivedConversations.value = Array.isArray(payload?.unarchivedConversations)
    ? payload.unarchivedConversations
    : [];
}

async function requestConversationSnapshot(conversationId?: string | null): Promise<SwitchConversationSnapshot> {
  return invokeTauri<SwitchConversationSnapshot>("switch_active_conversation_snapshot", {
    input: {
      conversationId: String(conversationId || "").trim() || null,
      agentId: String(activeAssistantAgentId.value || "").trim() || null,
    },
  });
}

async function recoverForegroundConversationFromOverview(reason: string, preferredConversationId?: string | null) {
  if (conversationForegroundSyncing.value) return;
  try {
    conversationForegroundSyncing.value = true;
    const currentConversationId = String(currentChatConversationId.value || "").trim();
    if (currentConversationId && unarchivedConversations.value.some((item) => String(item.conversationId || "").trim() === currentConversationId)) {
      return;
    }
    const nextConversationId = String(preferredConversationId || "").trim() || pickForegroundConversationId(unarchivedConversations.value);
    if (!nextConversationId) {
      clearForegroundConversation(reason);
      return;
    }
    const snapshot = await requestConversationSnapshot(nextConversationId);
    applyConversationSnapshot(snapshot);
  } finally {
    conversationForegroundSyncing.value = false;
  }
}

async function handleConversationOverviewUpdated(payload?: ConversationOverviewUpdatedPayload | null) {
  applyConversationOverviewPayload(payload);
  await recoverForegroundConversationFromOverview(
    "overview_updated",
    String(payload?.preferredConversationId || "").trim() || null,
  );
}

function syncCurrentConversationWorkspaceLabel() {
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (!currentConversationId) return;
  const nextLabel = String(chatWorkspaceName.value || "").trim() || "默认工作空间";
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== currentConversationId) {
      return item;
    }
    if (String(item.workspaceLabel || "").trim() === nextLabel) {
      return item;
    }
    changed = true;
    return {
      ...item,
      workspaceLabel: nextLabel,
    };
  });
  if (changed) {
    unarchivedConversations.value = nextItems;
  }
}

async function switchUnarchivedConversation(conversationId: string) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  const previousConversationId = String(currentChatConversationId.value || "").trim();
  const startedAt = perfNow();
  try {
    conversationForegroundSyncing.value = true;
    if (previousConversationId) {
      cacheConversationMessages(previousConversationId, allMessages.value);
    }
    chatFlow.freezeForegroundRoundState();
    currentChatConversationId.value = cid;
    const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[cid] || []);
    allMessages.value = cachedDisplay;
    hasMoreBackendHistory.value = inferHasMoreHistoryFromSnapshot(cachedDisplay);
    clearConversationBadge(cid);
    const trace = beginForegroundPaintTrace(cid);
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    logForegroundPaintTrace(trace, "前台切换首帧完成", {
      fromConversationId: previousConversationId,
      cachedCount: cachedDisplay.length,
      displayBlockCount: displayMessageBlocks.value.length,
      syncCostMs: Math.round((perfNow() - startedAt) * 10) / 10,
    });
    const snapshot = await requestConversationSnapshot(cid);
    applyConversationSnapshot(snapshot);
    await nextTick();
    void requestConversationMessagesAfterAsync(cid, trace).catch((error) => {
      setStatusError("status.loadMessagesFailed", error);
    });
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    conversationForegroundSyncing.value = false;
  }
}

async function createUnarchivedConversation(title?: string) {
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  if (!apiConfigId) return;
  try {
    const result = await invokeTauri<{ conversationId: string }>("create_unarchived_conversation", {
      input: {
        apiConfigId,
        title: String(title || "").trim() || null,
      },
    });
    const conversationId = String(result?.conversationId || "").trim();
    if (!conversationId) return;
    const snapshot = await requestConversationSnapshot(conversationId);
    applyConversationSnapshot(snapshot);
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  }
}

function closeForceArchiveActionDialog() {
  forceArchiveActionDialogOpen.value = false;
  forceArchivePreviewLoading.value = false;
  forceArchivePreview.value = null;
  forceCompactionPreview.value = null;
}

async function openForceArchiveActionDialog() {
  const apiConfigId = String(assistantDepartmentApiConfigId.value || "").trim();
  const agentId = String(activeAssistantAgentId.value || "").trim();
  const conversationId = String(currentChatConversationId.value || "").trim();
  if (!conversationId) {
    setStatus("当前没有可处理的会话。");
    return;
  }
  forceArchiveActionDialogOpen.value = true;
  forceArchivePreviewLoading.value = true;
  forceArchivePreview.value = null;
  forceCompactionPreview.value = null;
  try {
    const [archivePreview, compactionPreview] = await Promise.all([
      invokeTauri<ForceArchivePreviewResult>("preview_force_archive_current", {
        input: {
          apiConfigId,
          agentId,
          conversationId,
        },
      }),
      invokeTauri<ForceCompactionPreviewResult>("preview_force_compact_current", {
        input: {
          apiConfigId,
          agentId,
          conversationId,
        },
      }),
    ]);
    forceArchivePreview.value = archivePreview;
    forceCompactionPreview.value = compactionPreview;
  } catch (error) {
    closeForceArchiveActionDialog();
    setStatusError("status.loadConversationActionPreviewFailed", error);
  } finally {
    forceArchivePreviewLoading.value = false;
  }
}

async function confirmForceCompactionAction() {
  if (!forceCompactionPreview.value?.canCompact) return;
  closeForceArchiveActionDialog();
  await forceCompactNow();
}

async function confirmForceArchiveAction() {
  if (!forceArchivePreview.value?.canArchive) return;
  closeForceArchiveActionDialog();
  await forceArchiveNow();
}

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
  loadConfig,
  loadPersonas,
  loadChatSettings,
  refreshImageCacheStats,
  refreshConversationHistory,
  loadDelegateConversations,
  loadArchives,
  resetVisibleTurnCount: () => {},
  perfNow,
  perfLog,
});

const appBootstrap = useAppBootstrap({
  setViewMode: (mode) => {
    if (props.fixedViewMode) return;
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
    config.assistantDepartmentApiConfigId = String(payload.assistantDepartmentApiConfigId ?? "").trim();
    config.visionApiConfigId = payload.visionApiConfigId ?? undefined;
    config.sttApiConfigId = payload.sttApiConfigId ?? undefined;
    if ("sttAutoSend" in payload) {
      config.sttAutoSend = !!payload.sttAutoSend;
    }
    if (chatErrorText.value.includes("不支持图片附件") || chatErrorText.value.includes("PDF 附件")) {
      chatErrorText.value = "";
    }
    if (viewMode.value === "chat") {
      await refreshConversationHistory();
    }
  },
  onChatSettingsUpdated: async (payload) => {
    if ("assistantDepartmentAgentId" in payload) {
      const nextAgentId = String(payload.assistantDepartmentAgentId ?? "").trim();
      assistantDepartmentAgentId.value = nextAgentId;
      if (personaEditorId.value !== nextAgentId) {
        personaEditorId.value = nextAgentId;
      }
    }
    if ("userAlias" in payload) {
      userAlias.value = String(payload.userAlias ?? "");
    }
    if ("responseStyleId" in payload) {
      const nextStyleId = String(payload.responseStyleId ?? "").trim();
      selectedResponseStyleId.value = nextStyleId;
    }
    if (payload.pdfReadMode === "text" || payload.pdfReadMode === "image") {
      selectedPdfReadMode.value = payload.pdfReadMode;
    }
    if ("backgroundVoiceScreenshotKeywords" in payload) {
      backgroundVoiceScreenshotKeywords.value = String(payload.backgroundVoiceScreenshotKeywords ?? "");
    }
    if (payload.backgroundVoiceScreenshotMode === "desktop" || payload.backgroundVoiceScreenshotMode === "focused_window") {
      backgroundVoiceScreenshotMode.value = payload.backgroundVoiceScreenshotMode;
    }
    if (viewMode.value === "chat") {
      await refreshConversationHistory();
    }
  },
  onConfigUpdated: (payload) => {
    if (!payload || typeof payload !== "object") return;
    if ("hotkey" in payload) {
      config.hotkey = String(payload.hotkey ?? "").trim();
    }
    if ("uiFont" in payload) {
      config.uiFont = String(payload.uiFont ?? "");
    }
    if ("recordHotkey" in payload) {
      config.recordHotkey = String(payload.recordHotkey ?? "");
    }
    if ("recordBackgroundWakeEnabled" in payload) {
      config.recordBackgroundWakeEnabled = !!payload.recordBackgroundWakeEnabled;
    }
    if ("minRecordSeconds" in payload || "maxRecordSeconds" in payload || "toolMaxIterations" in payload) {
      const normalizedConfigNumbers = normalizeRuntimeConfigNumbers(
        "minRecordSeconds" in payload ? payload.minRecordSeconds : config.minRecordSeconds,
        "maxRecordSeconds" in payload ? payload.maxRecordSeconds : config.maxRecordSeconds,
        "toolMaxIterations" in payload ? payload.toolMaxIterations : config.toolMaxIterations,
        {
          minRecordSeconds: config.minRecordSeconds,
          maxRecordSeconds: config.maxRecordSeconds,
          toolMaxIterations: config.toolMaxIterations,
        },
      );
      config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
      config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
      config.toolMaxIterations = normalizedConfigNumbers.toolMaxIterations;
    }
    if ("selectedApiConfigId" in payload) {
      config.selectedApiConfigId = String(payload.selectedApiConfigId ?? "").trim();
    }
    if ("assistantDepartmentApiConfigId" in payload) {
      config.assistantDepartmentApiConfigId = String(payload.assistantDepartmentApiConfigId ?? "").trim();
    }
    if ("visionApiConfigId" in payload) {
      config.visionApiConfigId = payload.visionApiConfigId ?? undefined;
    }
    if ("sttApiConfigId" in payload) {
      config.sttApiConfigId = payload.sttApiConfigId ?? undefined;
    }
    if ("sttAutoSend" in payload) {
      config.sttAutoSend = !!payload.sttAutoSend;
    }
    if ("terminalShellKind" in payload) {
      config.terminalShellKind = String(payload.terminalShellKind ?? "");
    }
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
  scheduleChatMicPrewarm("focus", CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS);
}

function handleVisibilityForMicPrewarm() {
  if (document.visibilityState !== "visible") return;
  scheduleChatMicPrewarm("visibility_visible", CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS);
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
  console.warn("[聊天追踪][窗口] 初始化", {
    label: tauriWindowLabel.value,
    isChatWindow: isChatTauriWindow.value,
  });
  if (isChatTauriWindow.value) {
    void listen<unknown>("easy-call:history-flushed", (event) => {
      const payloadConversationId = readConversationIdFromPayload(event.payload);
      const currentConversationId = String(currentChatConversationId.value || "").trim();
      console.warn("[聊天追踪][历史刷写] 收到事件", {
        windowLabel: tauriWindowLabel.value,
        hasPayload: event.payload !== undefined,
        payloadConversationId,
        currentConversationId,
      });
      if (hasActiveForegroundConversation(payloadConversationId)) {
        void chatFlow.handleExternalHistoryFlushed(event.payload);
      }
    })
      .then((unlisten) => {
        chatHistoryFlushedUnlisten = unlisten;
        console.warn("[聊天追踪][历史刷写] 监听器已就绪", {
          windowLabel: tauriWindowLabel.value,
        });
      })
      .catch((error) => {
        console.error("[聊天追踪][历史刷写] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:round-completed", (event) => {
      const payloadConversationId = readConversationIdFromPayload(event.payload);
      const currentConversationId = String(currentChatConversationId.value || "").trim();
      if (payloadConversationId && payloadConversationId !== currentConversationId) {
        setConversationBadge(payloadConversationId, "completed");
        return;
      }
      if (!hasActiveForegroundConversation(payloadConversationId)) return;
      clearConversationBadge(payloadConversationId);
      void chatFlow.handleExternalRoundCompleted(event.payload);
    })
      .then((unlisten) => {
        chatRoundCompletedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][轮次完成] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:round-failed", (event) => {
      const payloadConversationId = readConversationIdFromPayload(event.payload);
      const currentConversationId = String(currentChatConversationId.value || "").trim();
      if (payloadConversationId && payloadConversationId !== currentConversationId) {
        setConversationBadge(payloadConversationId, "failed");
        return;
      }
      if (!hasActiveForegroundConversation(payloadConversationId)) return;
      clearConversationBadge(payloadConversationId);
      void chatFlow.handleExternalRoundFailed(event.payload);
    })
      .then((unlisten) => {
        chatRoundFailedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][轮次失败] 监听器注册失败", error);
      });
    void listen<ConversationOverviewUpdatedPayload>("easy-call:conversation-overview-updated", (event) => {
      void handleConversationOverviewUpdated(event.payload).catch((error) => {
        console.warn("[会话概览] 推送更新处理失败", error);
      });
    })
      .then((unlisten) => {
        chatConversationOverviewUpdatedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[会话概览] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:assistant-delta", (event) => {
      const conversationId = readConversationIdFromPayload(event.payload);
      if (hasActiveForegroundConversation(conversationId)) {
        void chatFlow.handleExternalAssistantDelta(event.payload);
      }
    })
      .then((unlisten) => {
        chatAssistantDeltaUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][助手增量] 监听器注册失败", error);
      });
    void listen<ConversationMessagesAfterSyncedPayload>("easy-call:conversation-messages-after-synced", (event) => {
      void applyConversationMessagesAfterSynced(event.payload);
    })
      .then((unlisten) => {
        chatConversationMessagesAfterSyncedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][异步补消息] 监听器注册失败", error);
      });

  }
  scheduleChatWindowActiveStateSync("mounted");
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
  if (chatConversationMessagesAfterSyncedUnlisten) {
    chatConversationMessagesAfterSyncedUnlisten();
    chatConversationMessagesAfterSyncedUnlisten = null;
  }
  if (chatConversationOverviewUpdatedUnlisten) {
    chatConversationOverviewUpdatedUnlisten();
    chatConversationOverviewUpdatedUnlisten = null;
  }
  window.removeEventListener("focus", handleWindowFocusForStateSync);
  window.removeEventListener("blur", handleWindowBlurForStateSync);
  document.removeEventListener("visibilitychange", handleVisibilityForStateSync);
  clearChatWindowActiveSyncTimer();
  clearChatMicPrewarmTimer();
  clearForegroundConversationCacheRaf();
  clearRecordHotkeyProbeState();
  agentWorkPresence.cleanup();
  chatWindowActiveSynced.value = null;
  if (isChatTauriWindow.value) {
    void invokeTauri("set_chat_window_active", { active: false }).catch(() => {});
  }
  window.removeEventListener("focus", handleWindowFocusForMicPrewarm);
  document.removeEventListener("visibilitychange", handleVisibilityForMicPrewarm);
  if (rewindConfirmResolver) {
    const resolver = rewindConfirmResolver;
    rewindConfirmResolver = null;
    resolver("cancel");
  }
  rewindConfirmDialogOpen.value = false;
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
    scheduleChatWindowActiveStateSync("viewmode_changed");
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
    conversationId: currentChatConversationId.value,
    conversationIds: unarchivedConversations.value
      .map((item) => String(item.conversationId || "").trim())
      .join("|"),
  }),
  ({ mode }) => {
    if (mode !== "chat") return;
    void refreshChatWorkspaceState();
  },
  { immediate: true },
);

watch(
  () => ({
    mode: viewMode.value,
    workspaceName: chatWorkspaceName.value,
    workspaceLocked: chatWorkspaceLocked.value,
  }),
  ({ mode }) => {
    if (mode !== "chat") return;
    syncCurrentConversationWorkspaceLabel();
  },
);

watch(
  () => viewMode.value,
  (mode) => {
    if (mode !== "chat") return;
    scheduleChatMicPrewarm("viewmode_chat", CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS);
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

function isApplyPatchArgsUndoable(rawArgs: string): boolean {
  const text = String(rawArgs || "").trim();
  if (!text) return false;
  if (text.startsWith("*** Begin Patch")) return true;
  if (!text.startsWith("{")) return false;
  try {
    const parsed = JSON.parse(text) as { input?: unknown };
    return typeof parsed.input === "string" && parsed.input.trim().startsWith("*** Begin Patch");
  } catch {
    return false;
  }
}

function getUndoAvailabilityForTurn(turnId: string): { canUndo: boolean; hint: string } {
  return inspectUndoablePatchCalls(allMessages.value || [], turnId, {
    isApplyPatchArgsUndoable,
  });
}

function requestRecallMode(payload: { turnId: string }): Promise<"with_patch" | "message_only" | "cancel"> {
  const availability = getUndoAvailabilityForTurn(payload.turnId);
  console.info("[会话撤回] 打开撤回弹窗", {
    turnId: payload.turnId,
    canUndoPatch: availability.canUndo,
    hint: availability.hint || "",
  });
  rewindConfirmCanUndoPatch.value = availability.canUndo;
  rewindConfirmUndoHint.value = availability.hint;
  rewindConfirmDialogOpen.value = true;
  return new Promise((resolve) => {
    rewindConfirmResolver = resolve;
  });
}

function resolveRewindConfirm(mode: "with_patch" | "message_only" | "cancel") {
  console.info("[会话撤回] 弹窗确认", {
    mode,
    canUndoPatch: rewindConfirmCanUndoPatch.value,
    dialogOpen: rewindConfirmDialogOpen.value,
  });
  const resolver = rewindConfirmResolver;
  rewindConfirmResolver = null;
  rewindConfirmDialogOpen.value = false;
  rewindConfirmUndoHint.value = "";
  if (resolver) {
    resolver(mode);
  }
}

function confirmRewindWithPatch() {
  console.info("[会话撤回] 点击：撤回消息并撤回修改");
  resolveRewindConfirm("with_patch");
}

function confirmRewindMessageOnly() {
  console.info("[会话撤回] 点击：仅撤回消息");
  resolveRewindConfirm("message_only");
}

function cancelRewindConfirm() {
  console.info("[会话撤回] 点击：取消撤回");
  resolveRewindConfirm("cancel");
}

function openConfigWindow() {
  void invokeTauri("show_main_window");
}

async function refreshRuntimeLogs() {
  runtimeLogsLoading.value = true;
  runtimeLogsError.value = "";
  try {
    const items = await invokeTauri<RuntimeLogEntry[]>("list_recent_runtime_logs");
    runtimeLogs.value = items;
  } catch (error) {
    runtimeLogsError.value = `加载运行日志失败：${String(error)}`;
  } finally {
    runtimeLogsLoading.value = false;
  }
}

function openRuntimeLogsDialog() {
  runtimeLogsDialogOpen.value = true;
  void (async () => {
    try {
      await invokeTauri("append_runtime_log_probe", {
        message: `日志窗口打开，window=${tauriWindowLabel.value}`,
      });
    } catch {
      // ignore probe write failure, do not block log list refresh
    }
    await refreshRuntimeLogs();
  })();
}

function closeRuntimeLogsDialog() {
  runtimeLogsDialogOpen.value = false;
}

async function clearRuntimeLogs() {
  runtimeLogsLoading.value = true;
  runtimeLogsError.value = "";
  try {
    await invokeTauri("clear_recent_runtime_logs");
    runtimeLogs.value = [];
  } catch (error) {
    runtimeLogsError.value = `清空运行日志失败：${String(error)}`;
  } finally {
    runtimeLogsLoading.value = false;
  }
}

function summonChatWindowFromConfig() {
  if (isChatTauriWindow.value) {
    freezeForegroundConversation("before_manual_summon");
  }
  void invokeTauri("show_chat_window");
}

async function closeWindowAndClearForeground() {
  if (isChatTauriWindow.value) {
    freezeForegroundConversation("close_window");
  }
  await closeWindow();
}

async function minimizeWindowAndClearForeground() {
  if (isChatTauriWindow.value) {
    freezeForegroundConversation("minimize_window");
  }
  await minimizeWindow();
}

async function openGithubRepository() {
  try {
    const url = await invokeTauri<string>("get_project_repository_url");
    void invokeTauri("open_external_url", { url });
  } catch (error) {
    console.warn("[关于] 获取项目仓库地址失败:", error);
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
  onReloadMessages: () => reloadForegroundConversationMessages("chat_flow_reload"),
  onHistoryFlushed: async ({ conversationId, pendingMessages, activateAssistant }) => {
    const flushedConversationId = String(conversationId || "").trim();
    console.warn("[聊天追踪][历史刷写处理] 开始", {
      windowLabel: tauriWindowLabel.value,
      flushedConversationId,
      activateAssistant,
      pendingCount: Array.isArray(pendingMessages) ? pendingMessages.length : 0,
      currentConversationId: String(currentChatConversationId.value || "").trim(),
      currentMessageCount: allMessages.value.length,
    });
    if (flushedConversationId && isChatWindowActiveNow()) {
      currentChatConversationId.value = flushedConversationId;
    }
    // 激活助理的批次也只做去重合并，避免清空重建打断滚动与分页状态。
    const queueMessages = Array.isArray(pendingMessages) ? pendingMessages : [];
    if (queueMessages.length > 0) {
      const existing = allMessages.value;
      const dedup = new Set(existing.map((m) => String(m.id || "").trim()).filter((id) => !!id));
      const beforeDedupCount = queueMessages.length;
      const uniqueIncoming = queueMessages.filter((m) => {
        const id = String(m.id || "").trim();
        if (!id) return true;
        if (dedup.has(id)) return false;
        dedup.add(id);
        return true;
      });
      const prepended = uniqueIncoming.filter((message) => {
        const meta = ((message.providerMeta || {}) as Record<string, unknown>);
        const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
        return String(messageMeta.kind || "").trim() === "summary_context_seed";
      });
      const appended = uniqueIncoming.filter((message) => {
        const meta = ((message.providerMeta || {}) as Record<string, unknown>);
        const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
        return String(messageMeta.kind || "").trim() !== "summary_context_seed";
      });
      allMessages.value = [...prepended, ...existing, ...appended];
      const appendedSummary = uniqueIncoming.map((message) => {
        const meta = (message.providerMeta || {}) as Record<string, unknown>;
        const origin = meta.origin as Record<string, unknown> | undefined;
        const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
        return {
          id: String(message.id || "").trim(),
          role: String(message.role || "").trim(),
          speakerAgentId: String(message.speakerAgentId || meta.speakerAgentId || meta.speaker_agent_id || "").trim(),
          originKind: String(origin?.kind || "").trim(),
          messageKind: String(messageMeta.kind || meta.messageKind || "").trim(),
          textPreview: Array.isArray(message.parts)
            ? message.parts
              .filter((part) => part?.type === "text")
              .map((part) => String((part as { text?: string }).text || "").trim())
              .filter(Boolean)
              .join(" | ")
              .slice(0, 80)
            : "",
        };
      });
      console.warn(`[聊天追踪][前台追加消息] 明细 ${JSON.stringify({
        windowLabel: tauriWindowLabel.value,
        appended: appendedSummary,
      })}`);
      if (appended.some((message) => isLocalOwnUserMessage(message))) {
        latestOwnMessageAlignRequest.value += 1;
      }
      console.warn("[聊天追踪][历史刷写处理] 合并完成", {
        windowLabel: tauriWindowLabel.value,
        activateAssistant,
        beforeDedupCount,
        prependedCount: prepended.length,
        appendedCount: appended.length,
        droppedAsDuplicate: beforeDedupCount - uniqueIncoming.length,
        previousMessageCount: existing.length,
        finalMessageCount: allMessages.value.length,
        firstPrependedId: String(prepended[0]?.id || ""),
        firstAppendedId: String(appended[0]?.id || ""),
        lastAppendedId: String(appended[appended.length - 1]?.id || ""),
      });
    } else {
      console.warn("[聊天追踪][历史刷写处理] 无待写入消息", {
        windowLabel: tauriWindowLabel.value,
        activateAssistant,
        finalMessageCount: allMessages.value.length,
      });
    }
    cacheConversationMessages(flushedConversationId || String(currentChatConversationId.value || "").trim(), allMessages.value);
    console.warn("[聊天追踪][历史刷写处理] 完成", {
      windowLabel: tauriWindowLabel.value,
      flushedConversationId: String(currentChatConversationId.value || "").trim(),
      finalMessageCount: allMessages.value.length,
    });
  },
});

watch(
  () => ({
    mode: viewMode.value,
    agentId: String(activeAssistantAgentId.value || "").trim(),
  }),
  ({ mode }) => {
    if (mode !== "chat") return;
    void refreshChatUnarchivedConversations().catch((error) => {
      setStatusError("status.loadMessagesFailed", error);
    });
  },
  { immediate: true },
);

watch(
  () => ({
    mode: viewMode.value,
    conversationId: String(currentChatConversationId.value || "").trim(),
  }),
  ({ mode, conversationId }) => {
    if (mode !== "chat") return;
    console.warn("[聊天追踪][流绑定] 准备绑定", {
      windowLabel: tauriWindowLabel.value,
      conversationId,
    });
    void chatFlow.bindActiveConversationStream(conversationId).catch((error) => {
      console.warn("[聊天推送] 绑定前台流失败", {
        conversationId,
        error,
      });
    });
  },
  { immediate: true },
);

watch(
  [() => String(currentChatConversationId.value || "").trim(), () => allMessages.value],
  ([conversationId, messages]) => {
    if (!conversationId) return;
    const nextMessages = Array.isArray(messages) ? messages : [];
    const hasStreamingDraft = nextMessages.some((item) => {
      const meta = (item?.providerMeta || {}) as Record<string, unknown>;
      return !!meta._streaming;
    });
    if (hasStreamingDraft) return;
    clearForegroundConversationCacheRaf();
    foregroundConversationCacheRaf = requestAnimationFrame(() => {
      foregroundConversationCacheRaf = 0;
      cacheConversationMessages(conversationId, nextMessages);
    });
  },
);

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
  chatting,
  forcingArchive,
  chatInput,
  clipboardImages,
  deleteUnarchivedConversationFromArchives,
  sendChat: chatFlow.sendChat,
  stopChat: chatFlow.stopChat,
  setStatusError,
  setChatErrorText: (text: string) => {
    chatErrorText.value = text;
  },
  removeBinaryPlaceholders,
  messageText,
  extractMessageImages,
  requestRecallMode,
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
  syncWindowControlsState,
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
  selectedPdfReadMode,
  backgroundVoiceScreenshotKeywords,
  backgroundVoiceScreenshotMode,
  selectedApiConfig,
  toolApiConfig,
  activeChatApiConfigId: assistantDepartmentApiConfigId,
  suppressChatReloadWatch,
  suppressAutosave,
  modelRefreshError,
  toolStatuses,
  defaultApiTools,
  t: tr,
  normalizeApiBindingsLocal,
  syncUserAliasFromPersona,
  syncTrayIcon,
  saveChatPreferences,
  saveConversationApiSettings,
  refreshToolsStatus,
  refreshImageCacheStats,
  refreshConversationHistory,
  resetVisibleTurnCount: () => {},
});
</script>
