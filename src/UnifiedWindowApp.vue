
<template>
  <div class="window-shell text-sm bg-base-200">
    <AppWindowHeader
      :view-mode="viewMode"
      :detached-chat-window="detachedChatWindow"
      :current-theme="currentTheme"
      :title-text="titleText"
      :chat-usage-percent="chatUsagePercent"
      :forcing-archive="forcingArchive"
      :chatting="chatting"
      :current-persona-name="String(currentForegroundPersona?.name || '').trim() || t('archives.roleAssistant')"
      :side-conversation-list-visible="sideConversationListVisible"
      :active-conversation-id="currentChatConversationId"
      :conversation-items="chatConversationItems"
      :user-alias="userAlias"
      :user-avatar-url="userAvatarUrl"
      :persona-name-map="chatPersonaNameMap"
      :persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :create-conversation-department-options="createConversationDepartmentOptions"
      :default-create-conversation-department-id="defaultCreateConversationDepartmentId"
      :force-archive-tip="t('chat.forceArchiveTip')"
      :maximized="maximized"
      :window-ready="windowReady"
      :open-config-title="t('window.configTitle')"
      :close-title="t('common.close')"
      :config-search-query="configSearchQuery"
      :config-search-results="configSearchResults"
      :config-search-placeholder="t('config.search.placeholder')"
      :show-update-to-latest-button="showUpdateToLatestButton"
      :has-available-update="hasAvailableUpdate"
      :checking-update="checkingUpdate"
      :update-to-latest-label="updateToLatestLabel"
      :update-to-latest-title="updateToLatestTitle"
      @open-archives="openCurrentHistory"
      @open-config="openConfigWindow"
      @minimize-window="minimizeWindowAndClearForeground"
      @toggle-maximize-window="toggleMaximizeWindow"
      @update:config-search-query="updateConfigSearchQuery"
      @select-config-search-result="handleSelectConfigSearchResult"
      @update-to-latest="triggerUpdateToLatest"
      @switch-conversation="switchChatConversation"
      @rename-conversation="renameCurrentConversation"
      @toggle-pin-conversation="toggleConversationPin"
      @create-conversation="createUnarchivedConversation"
      @force-archive="openForceArchiveActionDialog"
      @start-drag="startDrag"
      @close-window="handleCloseWindow"
    />

    <AppWindowContent
      :t="tr"
      :view-mode="viewMode"
      :detached-chat-window="detachedChatWindow"
      :config="config"
      :config-tab="configTab"
      :locale-options="localeOptions"
      :current-theme="currentTheme"
      :generated-theme-controls="generatedThemeControls"
      :generated-theme-tokens="generatedThemeTokens"
      :on-refresh-tool-review-message="refreshForegroundConversationMessageById"
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
      :normalize-api-bindings-action="normalizeApiBindingsLocal"
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio="hotkeyTestAudio"
      :user-alias="userAlias"
      :selected-persona-name="currentForegroundPersona?.name || t('archives.roleAssistant')"
      :current-chat-workspace-name="chatWorkspaceName"
      :current-chat-workspace-root-path="chatWorkspaceRootPath"
      :current-chat-workspaces="chatWorkspaceChoices"
      :current-chat-department-id="currentForegroundDepartmentId"
      :user-avatar-url="userAvatarUrl"
      :selected-persona-avatar-url="currentForegroundPersonaAvatarUrl"
      :chat-persona-name-map="chatPersonaNameMap"
      :chat-persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :chat-persona-presence-chips="chatPersonaPresenceChips"
      :chat-mention-options="chatMentionOptions"
      :selected-chat-mentions="selectedChatMentions"
      :latest-user-text="latestUserText"
      :latest-user-images="latestUserImages"
      :latest-assistant-text="latestAssistantText"
      :latest-reasoning-standard-text="latestReasoningStandardText"
      :latest-reasoning-inline-text="latestReasoningInlineText"
      :frontend-round-phase="chatFlow.frontendRoundPhase.value"
      :tool-status-text="toolStatusText"
      :tool-status-state="toolStatusState"
      :stream-tool-calls="streamToolCalls"
      :chat-error-text="chatErrorText"
      :clipboard-images="clipboardImages"
      :queued-attachment-notices="queuedAttachmentNotices"
      :chat-input="chatInput"
      :instruction-presets="instructionPresets"
      :chat-input-placeholder="chatInputPlaceholder"
      :speech-recognition-supported="speechRecognitionSupported"
      :recording="recording"
      :recording-ms="recordingMs"
      :transcribing="transcribing"
      :record-hotkey="config.recordHotkey"
      :selected-chat-model-id="currentForegroundApiConfigId"
      :tool-review-refresh-tick="toolReviewRefreshTick"
      :terminal-approvals="terminalApprovalConversationItems"
      :terminal-approval-resolving="terminalApprovalResolving"
      :approve-terminal-approval="approveTerminalApproval"
      :deny-terminal-approval="denyTerminalApproval"
      :plan-mode-enabled="currentConversationPlanModeEnabled"
      :chat-usage-percent="chatUsagePercent"
      :force-archive-tip="t('chat.forceArchiveTip')"
      :media-drag-active="mediaDragActive"
      :chatting="chatting"
      :forcing-archive="forcingArchive"
      :compacting-conversation="compactingConversation"
      :branching-conversation="branchingConversation"
      :forwarding-conversation-selection="forwardingConversationSelection"
      :visible-message-blocks="displayMessageBlocks"
      :chat-has-more-history="hasMoreBackendHistory"
      :chat-loading-older-history="loadingOlderConversationHistory"
      :latest-own-message-align-request="latestOwnMessageAlignRequest"
      :conversation-scroll-to-bottom-request="conversationScrollToBottomRequest"
      :current-chat-conversation-id="currentChatConversationId"
      :current-chat-todos="currentChatTodos"
      :chat-supervision-active="chatSupervisionActive"
      :chat-supervision-title="chatSupervisionTitle"
      :supervision-task-dialog-open="supervisionTaskDialogOpen"
      :supervision-task-saving="supervisionTaskSaving"
      :supervision-task-error="supervisionTaskError"
      :active-supervision-task="activeSupervisionTask"
      :recent-supervision-task-history="recentSupervisionTaskHistory"
      :chat-unarchived-conversation-items="chatUnarchivedConversationItems"
      :chat-conversation-items="chatConversationItems"
      :create-conversation-department-options="createConversationDepartmentOptions"
      :default-create-conversation-department-id="defaultCreateConversationDepartmentId"
      :archives="archives"
      :selected-archive-id="selectedArchiveId"
      :archive-blocks="archiveBlocks"
      :selected-archive-block-id="selectedArchiveBlockId"
      :archive-has-prev-block="archiveHasPrevBlock"
      :archive-has-next-block="archiveHasNextBlock"
      :archive-messages="archiveMessages"
      :archive-summary-text="archiveSummaryText"
      :unarchived-conversations="unarchivedConversations"
      :unarchived-blocks="unarchivedBlocks"
      :selected-unarchived-conversation-id="selectedUnarchivedConversationId"
      :selected-unarchived-block-id="selectedUnarchivedBlockId"
      :unarchived-has-prev-block="unarchivedHasPrevBlock"
      :unarchived-has-next-block="unarchivedHasNextBlock"
      :unarchived-messages="unarchivedMessages"
      :delegate-conversations="delegateConversations"
      :selected-delegate-conversation-id="selectedDelegateConversationId"
      :delegate-messages="delegateMessages"
      :remote-im-contact-conversations="remoteImContactConversations"
      :remote-im-contact-blocks="remoteImContactBlocks"
      :selected-remote-im-contact-id="selectedRemoteImContactId"
      :selected-remote-im-contact-block-id="selectedRemoteImContactBlockId"
      :remote-im-has-prev-block="remoteImHasPrevBlock"
      :remote-im-has-next-block="remoteImHasNextBlock"
      :remote-im-contact-messages="remoteImContactMessages"
      :message-text="messageText"
      :extract-message-images="extractMessageImages"
      :on-load-older-chat-history="loadOlderConversationHistory"
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
      :attach-tool-review-report="attachToolReviewReport"
      :update-config-tab="(value) => { configTab = value; }"
      :set-ui-language="setUiLanguage"
      :update-persona-editor-id="updatePersonaEditorIdWithNotice"
      :update-selected-persona-id="updateAssistantDepartmentAgentId"
      :update-selected-response-style-id="updateSelectedResponseStyleId"
      :update-selected-pdf-read-mode="updateSelectedPdfReadMode"
      :update-background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      :update-background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      :update-instruction-presets="updateInstructionPresets"
      :patch-conversation-api-settings="patchConversationApiSettings"
      :patch-chat-settings="patchChatSettings"
      :set-theme="setTheme"
      :activate-generated-theme="activateGeneratedTheme"
      :update-generated-theme-controls="updateGeneratedThemeControls"
      :reset-generated-theme="resetGeneratedTheme"
      :refresh-models="refreshModels"
      :on-tools-changed="handleToolsChanged"
      :save-config="saveConfig"
      :restore-config="restoreLastSavedConfigSnapshot"
      :add-api-config="addApiConfig"
      :remove-selected-api-config="removeSelectedApiConfig"
      :add-persona="addPersona"
      :remove-selected-persona="removeSelectedPersona"
      :save-personas="savePersonas"
      :import-persona-memories="importPersonaMemories"
      :open-current-history="openCurrentHistory"
      :open-conversation-summary="openConversationSummary"
      :open-force-archive-action-dialog="openForceArchiveActionDialog"
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
      :update-chat-input="handleChatInputUpdate"
      :update-selected-instruction-prompts="updateSelectedInstructionPrompts"
      :add-chat-mention="addChatMention"
      :remove-chat-mention="removeChatMention"
      :update-selected-chat-model-id="updateAssistantDepartmentApiConfigId"
      :update-plan-mode-enabled="updatePlanModeEnabled"
      :set-side-conversation-list-visible="handleSideConversationListVisibleChange"
      :remove-clipboard-image="removeClipboardImage"
      :remove-queued-attachment-notice="removeQueuedAttachmentNotice"
      :pick-attachments="pickChatAttachments"
      :start-recording="startRecording"
      :stop-recording="() => stopRecording(false)"
      :send-chat="sendChatFromCurrentWindow"
      :stop-chat="chatFlow.stopChat"
      :on-jump-to-conversation-bottom="ensureLatestForegroundTailThenScrollToBottom"
      :open-supervision-task-dialog="openSupervisionTaskDialog"
      :on-detach-conversation="detachCurrentConversationToWindow"
      :close-supervision-task-dialog="closeSupervisionTaskDialog"
      :save-supervision-task="saveSupervisionTask"
      :on-reached-chat-bottom="() => undefined"
      :on-recall-turn="handleRecallTurn"
      :on-regenerate-turn="handleRegenerateTurn"
      :confirm-plan="handleConfirmPlan"
      :on-lock-chat-workspace="openChatWorkspacePicker"
      :on-switch-conversation="switchChatConversation"
      :on-rename-conversation="renameCurrentConversation"
      :on-toggle-conversation-pin="toggleConversationPin"
      :on-create-conversation="createUnarchivedConversation"
      :on-branch-conversation-from-selection="branchConversationFromSelection"
      :on-forward-conversation-from-selection="forwardConversationFromSelection"
      :on-open-skill-panel="openSkillPlaceholderDialog"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :select-archive-block="selectArchiveBlock"
      :select-unarchived-conversation="selectUnarchivedConversation"
      :select-unarchived-conversation-block="selectUnarchivedConversationBlock"
      :select-delegate-conversation="selectDelegateConversation"
      :select-remote-im-contact-conversation="selectRemoteImContactConversation"
      :select-remote-im-contact-conversation-block="selectRemoteImContactConversationBlock"
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
      :has-available-update="hasAvailableUpdate"
      :check-update="manualCheckGithubUpdate"
      :open-github="openGithubRepository"
      :last-saved-config-json="lastSavedConfigJson"
    />

    <ShellDialogsHost
      :update-dialog-open="updateDialogOpen"
      :update-dialog-title="updateDialogTitle"
      :update-dialog-body="updateDialogBody"
      :update-dialog-kind="updateDialogKind"
      :update-dialog-release-url="updateDialogReleaseUrl"
      :update-dialog-primary-action="updateDialogPrimaryAction"
      :update-progress-percent="updateProgressPercent"
      :runtime-logs-dialog-open="runtimeLogsDialogOpen"
      :runtime-logs="runtimeLogs"
      :runtime-logs-loading="runtimeLogsLoading"
      :runtime-logs-error="runtimeLogsError"
      :rewind-confirm-dialog-open="rewindConfirmDialogOpen"
      :rewind-confirm-can-undo-patch="rewindConfirmCanUndoPatch"
      :config-save-error-dialog-open="configSaveErrorDialogOpen"
      :config-save-error-dialog-title="configSaveErrorDialogTitle"
      :config-save-error-dialog-body="configSaveErrorDialogBody"
      :config-save-error-dialog-kind="configSaveErrorDialogKind"
      :archive-import-preview-dialog-open="archiveImportPreviewDialogOpen"
      :archive-import-preview="archiveImportPreview"
      :archive-import-running="archiveImportRunning"
      :skill-placeholder-dialog-open="skillPlaceholderDialogOpen"
      :force-archive-action-dialog-open="forceArchiveActionDialogOpen"
      :force-archive-preview-loading="forceArchivePreviewLoading"
      :force-archive-preview="forceArchivePreview"
      :force-compaction-preview="forceCompactionPreview"
      :forcing-archive="forcingArchive"
      @close-update-dialog="closeUpdateDialog"
      @confirm-update-dialog-primary="confirmUpdateDialogPrimary"
      @open-update-release="openUpdateRelease"
      @close-runtime-logs-dialog="closeRuntimeLogsDialog"
      @refresh-runtime-logs="refreshRuntimeLogs"
      @clear-runtime-logs="clearRuntimeLogs"
      @confirm-rewind-with-patch="confirmRewindWithPatch"
      @confirm-rewind-message-only="confirmRewindMessageOnly"
      @cancel-rewind-confirm="cancelRewindConfirm"
      @close-config-save-error-dialog="closeConfigSaveErrorDialog"
      @close-archive-import-preview-dialog="closeArchiveImportPreviewDialog"
      @confirm-archive-import="confirmArchiveImport"
      @close-skill-placeholder-dialog="closeSkillPlaceholderDialog"
      @confirm-delete-conversation-from-archive-dialog="confirmDeleteConversationFromArchiveDialog"
      @confirm-force-compaction-action="confirmForceCompactionAction"
      @confirm-force-archive-action="handleConfirmForceArchiveAction()"
      @close-force-archive-action-dialog="closeForceArchiveActionDialog"
    />
    <ChatWorkspacePickerDialog
      :open="chatWorkspacePickerOpen"
      :saving="chatWorkspacePickerSaving"
      :workspaces="chatWorkspaceDraftChoices"
      @close="closeChatWorkspacePicker"
      @add-workspace="addChatWorkspace"
      @set-main="setChatWorkspaceAsMain"
      @set-access="setChatWorkspaceAccess"
      @remove-workspace="removeChatWorkspace"
      @save="saveChatWorkspacePicker"
    />
    <div
      v-if="messageStoreMigration.visible"
      class="fixed inset-0 z-9999 flex items-center justify-center bg-base-300/90 p-6 backdrop-blur"
    >
      <div class="w-full max-w-2xl rounded-box border border-base-content/10 bg-base-100 p-6 shadow-2xl">
        <div class="text-xl font-semibold">会话消息仓库迁移</div>
        <div class="mt-2 text-sm opacity-70">{{ messageStoreMigration.message }}</div>
        <progress
          v-if="messageStoreMigration.mode === 'migrating'"
          class="progress progress-primary mt-5 w-full"
          :value="messageStoreMigration.current"
          :max="Math.max(messageStoreMigration.total, 1)"
        />
        <div v-if="messageStoreMigration.mode === 'migrating'" class="mt-2 text-xs opacity-60">
          {{ messageStoreMigration.current }} / {{ messageStoreMigration.total }}
        </div>
        <div
          v-if="messageStoreMigration.blockedItems.length > 0"
          class="mt-5 max-h-64 space-y-2 overflow-auto rounded-box bg-base-200 p-3"
        >
          <div
            v-for="item in messageStoreMigration.blockedItems"
            :key="item.conversationId"
            class="rounded-box bg-base-100 p-3 text-sm"
          >
            <div class="font-medium">{{ item.title || item.conversationId }}</div>
            <div class="mt-1 text-xs opacity-60">{{ item.conversationId }}</div>
            <div class="mt-2 text-error">{{ item.reason || "未知错误" }}</div>
          </div>
        </div>
        <div v-if="messageStoreMigration.mode === 'blocked'" class="mt-5 flex justify-end gap-3">
          <button class="btn btn-ghost" @click="cancelMessageStoreMigration">取消启动</button>
          <button class="btn btn-error" @click="continueMessageStoreMigrationWithDiscard">
            抛弃异常会话并继续迁移
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invokeTauri } from "./services/tauri-api";
import { useAppBootstrap } from "./features/shell/composables/use-app-bootstrap";
import { useAppCore } from "./features/shell/composables/use-app-core";
import { useAppLifecycle } from "./features/shell/composables/use-app-lifecycle";
import { useAppTheme } from "./features/shell/composables/use-app-theme";
import { useGithubUpdate } from "./features/shell/composables/use-github-update";
import {
  useShellDialogFlows,
} from "./features/shell/composables/use-shell-dialog-flows";
import { useTerminalApproval, type TerminalApprovalRequestPayload } from "./features/shell/composables/use-terminal-approval";
import { applyUiFont, normalizeUiFont } from "./features/shell/composables/use-ui-font";
import { useWindowActions } from "./features/shell/composables/use-window-actions";
import { useViewRefresh } from "./features/shell/composables/use-view-refresh";
import { useWindowShell } from "./features/shell/composables/use-window-shell";
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useConfigPersistence } from "./features/config/composables/use-config-persistence";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useAgentWorkPresence } from "./features/chat/composables/use-agent-work-presence";
import { useArchivesView } from "./features/chat/composables/use-archives-view";
import { useArchiveImport } from "./features/chat/composables/use-archive-import";
import { useAvatarCache } from "./features/chat/composables/use-avatar-cache";
import { useChatDialogActions } from "./features/chat/composables/use-chat-dialog-actions";
import { useChatAttachmentPickerFlow } from "./features/chat/composables/use-chat-attachment-picker-flow";
import { useChatWorkspace } from "./features/chat/composables/use-chat-workspace";
import { useChatWorkspacePickerFlow } from "./features/chat/composables/use-chat-workspace-picker-flow";
import { useChatRewindActions } from "./features/chat/composables/use-chat-rewind-actions";
import { useConfirmPlan } from "./features/chat/composables/use-confirm-plan";
import { useConversationPlanMode } from "./features/chat/composables/use-conversation-plan-mode";
import { useSupervisionTask } from "./features/chat/composables/use-supervision-task";
import { useChatRuntime } from "./features/chat/composables/use-chat-runtime";
import { useChatMessageBlocks } from "./features/chat/composables/use-chat-turns";
import { useChatMedia } from "./features/chat/composables/use-chat-media";
import { usePromptPreview } from "./features/chat/composables/use-prompt-preview";
import ChatWorkspacePickerDialog from "./features/chat/components/dialogs/ChatWorkspacePickerDialog.vue";
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
import ShellDialogsHost from "./features/shell/components/ShellDialogsHost.vue";
import type { ChatWorkspaceChoice } from "./features/chat/composables/use-chat-workspace";
import type {
  PersonaProfile,
  AppConfig,
  ChatMentionTarget,
  ChatMessage,
  ChatConversationOverviewItem,
  PromptCommandPreset,
  ChatTodoItem,
  ChatPersonaPresenceChip,
  ConversationPreviewMessage,
  ImageTextCacheStats,
  RemoteImContactConversationSummary,
  ResponseStyleOption,
  ToolLoadStatus,
  UnarchivedConversationSummary,
} from "./types/app";
import responseStylesJson from "./constants/response-styles.json";
import { normalizeLocale } from "./i18n";
import { searchConfigTabs, type ConfigSearchResult, type ConfigSearchTab } from "./features/config/search/config-search";
import { ensureConversationMessageIds } from "./features/chat/utils/message-id";

const props = withDefaults(defineProps<{ fixedViewMode?: "chat" | "archives" | "config" }>(), {
  fixedViewMode: undefined,
});

const DRAFT_ASSISTANT_ID_PREFIX = "__draft_assistant__:";
const BACKGROUND_CONVERSATION_CACHE_LIMIT = 10;
const FOREGROUND_SNAPSHOT_RECENT_LIMIT = 4;
const OLDER_HISTORY_PAGE_SIZE = 2;
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

type ConversationMessageAppendedPayload = {
  conversationId?: string;
  message?: ChatMessage | null;
};
type DetachedChatWindowInfo = {
  detached: boolean;
  conversationId?: string | null;
  windowLabel?: string | null;
};
type MessageStoreMigrationPreflightItem = {
  conversationId: string;
  title: string;
  status: string;
  messageCount: number;
  reason?: string | null;
};
type MessageStoreMigrationPreflightReport = {
  totalConversations: number;
  readyCount: number;
  legacyCount: number;
  blockedCount: number;
  canAutoMigrate: boolean;
  items: MessageStoreMigrationPreflightItem[];
};
type MessageStoreMigrationProgressPayload = {
  current: number;
  total: number;
  conversationId: string;
  title: string;
  status: string;
  detail?: string | null;
};
type MessageStoreMigrationGateMode = "idle" | "checking" | "migrating" | "blocked" | "error";

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
  startDrag,
  toggleAlwaysOnTop,
  minimizeWindow,
  toggleMaximizeWindow,
} =
  useWindowShell();
const {
  currentTheme,
  generatedThemeControls,
  generatedThemeTokens,
  applyTheme,
  setTheme,
  activateGeneratedTheme,
  updateGeneratedThemeControls,
  resetGeneratedTheme,
  restoreThemeFromStorage,
} = useAppTheme();

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  uiLanguage: "zh-CN",
  uiFont: "auto",
  recordHotkey: isMacPlatform ? "Option+Space" : "Alt",
  recordBackgroundWakeEnabled: true,
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  llmRoundLogCapacity: 3,
  selectedApiConfigId: "",
  assistantDepartmentApiConfigId: "",
  visionApiConfigId: undefined,
  toolReviewApiConfigId: undefined,
  sttApiConfigId: undefined,
  sttAutoSend: false,
  terminalShellKind: "auto",
  shellWorkspaces: [],
  mcpServers: [],
  remoteImChannels: [],
  departments: [],
  apiProviders: [],
  apiConfigs: [],
});
const recordHotkeyProbeLastSeq = ref(0);
const recordHotkeyProbeDown = ref(false);
const chatWindowActiveSynced = ref<boolean | null>(null);
const tauriWindowLabel = ref("unknown");
const isChatTauriWindow = ref(false);
const detachedChatWindow = ref(false);
const detachedChatConversationId = ref("");
const detachedTemporaryApiConfigId = ref("");
const webviewZoomFactor = ref(1);
const WEBVIEW_ZOOM_MIN = 0.8;
const WEBVIEW_ZOOM_MAX = 2.0;
const WEBVIEW_ZOOM_STEP = 0.1;
let chatHistoryFlushedUnlisten: UnlistenFn | null = null;
let chatRoundStartedUnlisten: UnlistenFn | null = null;
let chatRoundCompletedUnlisten: UnlistenFn | null = null;
let chatRoundFailedUnlisten: UnlistenFn | null = null;
let chatAssistantDeltaUnlisten: UnlistenFn | null = null;
let chatStreamRebindRequiredUnlisten: UnlistenFn | null = null;
let chatConversationMessagesAfterSyncedUnlisten: UnlistenFn | null = null;
let chatConversationMessageAppendedUnlisten: UnlistenFn | null = null;
let chatConversationTodosUpdatedUnlisten: UnlistenFn | null = null;
let chatConversationPinUpdatedUnlisten: UnlistenFn | null = null;
let chatConversationOverviewUpdatedUnlisten: UnlistenFn | null = null;
let foregroundPaintTraceSeq = 0;
let chatWindowActiveSyncTimer: ReturnType<typeof setTimeout> | null = null;
let chatMicPrewarmTimer: ReturnType<typeof setTimeout> | null = null;
let foregroundConversationCacheRaf = 0;
const configTab = ref<ConfigSearchTab>("hotkey");
const configSearchQuery = ref("");
const configSearchResults = computed<ConfigSearchResult[]>(() => {
  if (viewMode.value !== "config") return [];
  return searchConfigTabs(configSearchQuery.value, normalizeLocale(config.uiLanguage));
});
const personas = ref<PersonaProfile[]>([]);
const assistantDepartmentAgentId = ref("default-agent");
const personaEditorId = ref("default-agent");
const userAlias = ref(t("archives.roleUser"));
const selectedResponseStyleId = ref("concise");
const selectedPdfReadMode = ref<"text" | "image">("image");
const backgroundVoiceScreenshotKeywords = ref("");
const backgroundVoiceScreenshotMode = ref<"desktop" | "focused_window">("focused_window");
const instructionPresets = ref<PromptCommandPreset[]>([]);
const selectedInstructionPrompts = ref<PromptCommandPreset[]>([]);
const selectedChatMentions = ref<ChatMentionTarget[]>([]);
const chatInput = ref("");
const currentChatConversationId = ref("");
const sideConversationListVisible = ref(false);
const conversationForegroundSyncing = ref(false);
const backgroundConversationBadgeMap = ref<Record<string, BackgroundConversationBadgeState>>({});
const conversationMessageCache = ref<Record<string, ChatMessage[]>>({});
const latestUserText = ref("");
const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
const latestAssistantText = ref("");
const latestReasoningStandardText = ref("");
const latestReasoningInlineText = ref("");
const latestOwnMessageAlignRequest = ref(0);
let suppressNextOwnMessageAlignFromHistoryFlushed = 0;
const toolStatusText = ref("");
const toolStatusState = ref<"running" | "done" | "failed" | "">("");
const streamToolCalls = ref<Array<{ name: string; argsText: string }>>([]);
const chatErrorText = ref("");
const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>([]);

function handleChatInputUpdate(value: string) {
  chatInput.value = value;
}

function updateConfigSearchQuery(value: string) {
  configSearchQuery.value = String(value || "");
}

function handleSelectConfigSearchResult(tab: ConfigSearchTab) {
  configTab.value = tab;
  configSearchQuery.value = "";
}

function updateSelectedInstructionPrompts(value: PromptCommandPreset[]) {
  selectedInstructionPrompts.value = Array.isArray(value)
    ? value
        .map((item) => ({
          id: String(item?.id || "").trim(),
          name: String(item?.prompt || item?.name || "").trim(),
          prompt: String(item?.prompt || item?.name || "").trim(),
        }))
        .filter((item) => !!item.id && !!item.prompt)
    : [];
}

function addChatMention(value: ChatMentionTarget) {
  const agentId = String(value?.agentId || "").trim();
  const departmentId = String(value?.departmentId || "").trim();
  const agentName = String(value?.agentName || "").trim();
  if (!agentId || !departmentId || !agentName) return;
  if (selectedChatMentions.value.some((item) => item.agentId === agentId)) return;
  selectedChatMentions.value = [
    ...selectedChatMentions.value,
    {
      agentId,
      agentName,
      departmentId,
      departmentName: String(value?.departmentName || "").trim(),
      avatarUrl: String(value?.avatarUrl || "").trim() || undefined,
    },
  ];
}

function removeChatMention(agentId: string) {
  const normalizedAgentId = String(agentId || "").trim();
  selectedChatMentions.value = selectedChatMentions.value.filter((item) => item.agentId !== normalizedAgentId);
}

async function updatePlanModeEnabled(value: boolean) {
  await setCurrentConversationPlanMode(value);
}

function handleSideConversationListVisibleChange(value: boolean) {
  sideConversationListVisible.value = value;
}

const allMessages = shallowRef<ChatMessage[]>([]);

const status = ref("Ready.");
const terminalApprovalQueue = ref<TerminalApprovalRequestPayload[]>([]);
const terminalApprovalResolving = ref(false);
const loading = ref(false);
const saving = ref(false);
const startupDataReady = ref(false);
const messageStoreMigration = reactive<{
  visible: boolean;
  mode: MessageStoreMigrationGateMode;
  message: string;
  current: number;
  total: number;
  blockedItems: MessageStoreMigrationPreflightItem[];
}>({
  visible: false,
  mode: "idle",
  message: "",
  current: 0,
  total: 0,
  blockedItems: [],
});
let messageStoreMigrationResolve: (() => void) | null = null;
let messageStoreMigrationReject: ((error: Error) => void) | null = null;
let messageStoreMigrationProgressUnlisten: UnlistenFn | null = null;
const chatting = ref(false);
const forcingArchive = ref(false);
const compactingConversation = ref(false);
const suppressNextCompactionReload = ref(false);
const conversationBusy = computed(() => forcingArchive.value || compactingConversation.value);
const branchingConversation = ref(false);
const forwardingConversationSelection = ref(false);
const hasMoreBackendHistory = ref(false);
const loadingOlderConversationHistory = ref(false);
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
const CHAT_STREAM_DEBUG = false;
const { perfNow, perfLog, setStatus, setStatusError, localeOptions, applyUiLanguage } = useAppCore({
  t: tr,
  config,
  locale,
  status,
  perfDebug: PERF_DEBUG,
});
const {
  checkingUpdate,
  hasAvailableUpdate,
  updateReadyToRestart,
  latestCheckResult,
  updateDialogOpen,
  updateDialogTitle,
  updateDialogBody,
  updateDialogKind,
  updateDialogReleaseUrl,
  updateDialogPrimaryAction,
  updateProgressPercent,
  closeUpdateDialog,
  openUpdateRelease,
  confirmUpdateDialogPrimary,
  autoCheckGithubUpdate,
  manualCheckGithubUpdate,
  triggerUpdateToLatest,
} = useGithubUpdate({
  viewMode,
  status,
});
const showUpdateToLatestButton = computed(() => hasAvailableUpdate.value);
const updateToLatestLabel = computed(() =>
  updateReadyToRestart.value
    ? t("about.updateAndRestart")
    : checkingUpdate.value
      ? t("about.updating")
      : t("about.updateNow"),
);
const updateToLatestTitle = computed(() => {
  const latestVersion = String(latestCheckResult.value?.latestVersion || "").trim();
  if (updateReadyToRestart.value && latestVersion) {
    return t("about.updateReadyAction", { version: latestVersion });
  }
  if (latestVersion) {
    return t("about.updateAvailableAction", { version: latestVersion });
  }
  return t("about.updateNow");
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
});

const {
  archives,
  archiveBlocks,
  archiveMessages,
  archiveSummaryText,
  selectedArchiveId,
  selectedArchiveBlockId,
  archiveHasPrevBlock,
  archiveHasNextBlock,
  unarchivedConversations,
  unarchivedBlocks,
  unarchivedMessages,
  selectedUnarchivedConversationId,
  selectedUnarchivedBlockId,
  unarchivedHasPrevBlock,
  unarchivedHasNextBlock,
  delegateConversations,
  delegateMessages,
  selectedDelegateConversationId,
  remoteImContactConversations,
  remoteImContactBlocks,
  remoteImContactMessages,
  selectedRemoteImContactId,
  selectedRemoteImContactBlockId,
  remoteImHasPrevBlock,
  remoteImHasNextBlock,
  loadArchives,
  loadDelegateConversations,
  selectArchive,
  selectArchiveBlock,
  selectUnarchivedConversation,
  selectUnarchivedConversationBlock,
  selectDelegateConversation,
  selectRemoteImContactConversation,
  selectRemoteImContactConversationBlock,
  deleteUnarchivedConversation: deleteUnarchivedConversationFromArchivesRaw,
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

const {
  currentConversationPlanModeEnabled,
  setConversationPlanMode,
  setCurrentConversationPlanMode,
} = useConversationPlanMode({
  currentConversationId: currentChatConversationId,
  unarchivedConversations,
});
const {
  supervisionTaskDialogOpen,
  supervisionTaskSaving,
  supervisionTaskError,
  activeSupervisionTask,
  recentSupervisionTaskHistory,
  chatSupervisionActive,
  chatSupervisionTitle,
  openSupervisionTaskDialog,
  closeSupervisionTaskDialog,
  saveSupervisionTask,
  refreshActiveSupervisionTask,
  startSupervisionTaskPolling,
  clearSupervisionTaskPollTimer,
  handleConversationChanged: handleSupervisionConversationChanged,
} = useSupervisionTask({
  t: tr,
  currentConversationId: currentChatConversationId,
  setStatus,
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
    return t("window.chatTitle", { name: currentForegroundPersona.value?.name || t("archives.roleAssistant") });
  }
  if (viewMode.value === "archives") {
    return t("window.archivesTitle");
  }
  return t("window.configTitle");
});
const selectedApiConfig = computed(() => config.apiConfigs.find((a) => a.id === config.selectedApiConfigId) ?? null);
const selectedApiProvider = computed(() => {
  const [providerId] = String(config.selectedApiConfigId || "").split("::");
  return config.apiProviders.find((provider) => provider.id === providerId) ?? null;
});
const textCapableApiConfigs = computed(() =>
  config.apiConfigs.filter(
    (a) =>
      a.enableText
      && (
        a.requestFormat === "openai"
        || a.requestFormat === "deepseek/kimi"
        || a.requestFormat === "codex"
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
function departmentPrimaryApiConfigId(
  department?: { apiConfigId?: string; apiConfigIds?: string[] } | null,
): string {
  const ids = Array.isArray(department?.apiConfigIds)
    ? department.apiConfigIds.map((id) => String(id || "").trim()).filter(Boolean)
    : [];
  if (ids.length > 0) return ids[0];
  return String(department?.apiConfigId || "").trim();
}

function departmentOrderedApiConfigIds(
  department?: { apiConfigId?: string; apiConfigIds?: string[] } | null,
): string[] {
  return Array.from(new Set([
    ...((Array.isArray(department?.apiConfigIds) ? department.apiConfigIds : []).map((id) => String(id || "").trim()).filter(Boolean)),
    String(department?.apiConfigId || "").trim(),
  ].filter(Boolean)));
}

function applyDepartmentPrimaryApiConfigLocally(
  department: { id?: string; isBuiltInAssistant?: boolean; apiConfigId?: string; apiConfigIds?: string[]; updatedAt?: string } | null | undefined,
  apiConfigId: string,
): boolean {
  if (!department) return false;
  const nextId = String(apiConfigId || "").trim();
  if (!nextId) return false;
  const next = departmentOrderedApiConfigIds(department);
  if ((next[0] || "") === nextId) {
    config.selectedApiConfigId = nextId;
    if (department.id === "assistant-department" || department.isBuiltInAssistant) {
      config.assistantDepartmentApiConfigId = nextId;
    }
    return false;
  }
  const filtered = next.filter((item) => item.toLowerCase() !== nextId.toLowerCase());
  if (filtered.length === 0) {
    filtered.push(nextId);
  } else {
    filtered[0] = nextId;
  }
  const deduped = Array.from(new Set(filtered.filter(Boolean)));
  department.apiConfigIds = deduped;
  department.apiConfigId = deduped[0] || "";
  department.updatedAt = new Date().toISOString();
  if (department.id === "assistant-department" || department.isBuiltInAssistant) {
    config.assistantDepartmentApiConfigId = department.apiConfigId;
  }
  config.selectedApiConfigId = nextId;
  return true;
}
const MIN_RECORD_SECONDS = 1;
const MAX_MIN_RECORD_SECONDS = 30;
const DEFAULT_MAX_RECORD_SECONDS = 60;
const MAX_RECORD_SECONDS = 600;

function normalizeRuntimeConfigNumbers(
  minValue: unknown,
  maxValue: unknown,
  fallback?: {
    minRecordSeconds?: number;
    maxRecordSeconds?: number;
  },
): { minRecordSeconds: number; maxRecordSeconds: number } {
  const fallbackMin = Number(fallback?.minRecordSeconds);
  const fallbackMax = Number(fallback?.maxRecordSeconds);
  const nextMin = Number(minValue);
  const nextMax = Number(maxValue);
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
  return { minRecordSeconds, maxRecordSeconds };
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
      sendChatFromCurrentWindow();
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

function isPrimaryChatWindow(): boolean {
  return tauriWindowLabel.value === "chat" && !detachedChatWindow.value;
}

function clearRecordHotkeyProbeState() {
  recordHotkeyProbeDown.value = false;
  recordHotkeyProbeLastSeq.value = 0;
}

function scheduleChatWindowActiveStateSync(reason: string, delayMs = 0) {
  if (!isPrimaryChatWindow()) return;
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
  if (!isPrimaryChatWindow()) return;
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
  activeChatApiConfig: computed(() => currentForegroundApiConfig.value),
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
const queueTextAttachment = chatMedia.queueTextAttachment;
const removeClipboardImage = chatMedia.removeClipboardImage;
const startHotkeyRecordTest = chatMedia.startHotkeyRecordTest;
const stopHotkeyRecordTest = chatMedia.stopHotkeyRecordTest;
const playHotkeyRecordTest = chatMedia.playHotkeyRecordTest;
const cleanupChatMedia = chatMedia.cleanupChatMedia;
const { removeQueuedAttachmentNotice, pickChatAttachments } = useChatAttachmentPickerFlow({
  chatting,
  forcingArchive,
  queuedAttachmentNotices,
  onNativeFileDrop,
  setStatusError,
});

async function attachToolReviewReport(reportText: string) {
  try {
    await queueTextAttachment("tool-review-report.md", reportText, "text/markdown");
    status.value = "已附加审查报告";
  } catch (error) {
    setStatusError("status.pasteImageReadFailed", error);
  }
}

async function switchRemoteImContactConversation(contactId: string) {
  const normalizedContactId = String(contactId || "").trim();
  if (!normalizedContactId) return;
  const targetOverview = remoteImContactConversations.value.find((item) => String(item.contactId || "").trim() === normalizedContactId);
  const conversationId = String(targetOverview?.conversationId || "").trim();
  if (!conversationId) return;
  const previousConversationId = String(currentChatConversationId.value || "").trim();
  try {
    conversationForegroundSyncing.value = true;
    if (previousConversationId) {
      cacheConversationMessages(previousConversationId, allMessages.value);
    }
    chatFlow.freezeForegroundRoundState();
    currentChatConversationId.value = conversationId;
    currentChatTodos.value = [];
    clearPendingManualScrollToBottom();
    const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[conversationId] || []);
    allMessages.value = cachedDisplay;
    hasMoreBackendHistory.value = cachedDisplay.length >= FOREGROUND_SNAPSHOT_RECENT_LIMIT;
    foregroundTailLatestReady.value = true;
    const messages = await requestRemoteImConversationMessages(normalizedContactId);
    const nextMessages = reuseStableMessageReferences(
      freezeConversationMessages(Array.isArray(messages) ? messages : []),
      allMessages.value,
    );
    allMessages.value = nextMessages;
    cacheConversationMessages(conversationId, nextMessages);
    hasMoreBackendHistory.value = nextMessages.length >= FOREGROUND_SNAPSHOT_RECENT_LIMIT;
    clearConversationBadge(conversationId);
    scheduleConversationScrollToBottomFallback(conversationId);
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    conversationForegroundSyncing.value = false;
  }
}

async function openConversationInDetachedWindowById(conversationId: string) {
  // 打开或聚焦指定会话的独立聊天窗口，并在窗口操作后刷新普通会话与远程联系人概览。
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  try {
    const focused = await invokeTauri<boolean>("focus_detached_chat_window_by_conversation", {
      input: { conversationId: cid },
    });
    if (focused) return;
  } catch (error) {
    console.warn("[独立聊天窗口] 聚焦已占用会话失败", {
      conversationId: cid,
      error,
    });
  }
  try {
    await invokeTauri<{ conversationId: string; windowLabel: string; mainConversationId?: string | null }>("detach_current_conversation_to_window", {
      input: { conversationId: cid },
    });
  } catch (error) {
    console.warn("[独立聊天窗口] 打开独立会话窗口失败", {
      conversationId: cid,
      error,
    });
  }
  await refreshUnarchivedConversationOverview();
  await refreshRemoteImConversationOverview();
}

async function switchChatConversation(payload: { kind?: ChatConversationKind; conversationId: string; remoteContactId?: string }) {
  const kind = payload.kind === "remote_im_contact" ? "remote_im_contact" : "local_unarchived";
  if (kind === "remote_im_contact") {
    await openConversationInDetachedWindowById(payload.conversationId);
    return;
  }
  await switchUnarchivedConversation(payload.conversationId);
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
const currentForegroundConversationSummary = computed(() => {
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (currentConversationId) {
    const matched = chatConversationItems.value.find(
      (item) => String(item.conversationId || "").trim() === currentConversationId,
    );
    if (matched) return matched;
  }
  return (
    unarchivedConversations.value.find((item) => !!item.isMainConversation)
    || unarchivedConversations.value[0]
    || null
  );
});
const currentForegroundDepartmentId = computed(
  () => String(currentForegroundConversationSummary.value?.departmentId || "").trim() || "assistant-department",
);
const currentForegroundDepartment = computed(
  () =>
    config.departments.find((item) => String(item.id || "").trim() === currentForegroundDepartmentId.value)
    || config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant)
    || null,
);
const currentForegroundAgentId = computed(
  () =>
    String(currentForegroundConversationSummary.value?.agentId || "").trim()
    || String(currentForegroundDepartment.value?.agentIds?.[0] || "").trim()
    || String(assistantDepartmentAgentId.value || "").trim(),
);
const currentForegroundApiConfigId = computed(
  () => {
    if (detachedChatWindow.value) {
      const temporaryApiConfigId = String(detachedTemporaryApiConfigId.value || "").trim();
      if (temporaryApiConfigId && config.apiConfigs.some((item) => item.id === temporaryApiConfigId && item.enableText)) {
        return temporaryApiConfigId;
      }
    }
    return departmentPrimaryApiConfigId(currentForegroundDepartment.value)
      || String(config.assistantDepartmentApiConfigId || "").trim();
  },
);
const currentForegroundApiConfig = computed(
  () => config.apiConfigs.find((a) => a.id === currentForegroundApiConfigId.value) ?? null,
);
const currentForegroundPersona = computed(
  () =>
    assistantPersonas.value.find((p) => p.id === currentForegroundAgentId.value)
    ?? assistantDepartmentPersona.value
    ?? assistantPersonas.value[0]
    ?? null,
);
let {
  runtimeLogsDialogOpen,
  runtimeLogs,
  runtimeLogsLoading,
  runtimeLogsError,
  configSaveErrorDialogOpen,
  configSaveErrorDialogTitle,
  configSaveErrorDialogBody,
  configSaveErrorDialogKind,
  skillPlaceholderDialogOpen,
  forceArchiveActionDialogOpen,
  forceArchivePreviewLoading,
  forceArchivePreview,
  forceCompactionPreview,
  rewindConfirmDialogOpen,
  rewindConfirmCanUndoPatch,
  openForceArchiveActionDialog,
  closeForceArchiveActionDialog,
  confirmForceCompactionAction,
  confirmForceArchiveAction,
  confirmDeleteConversationFromArchiveDialog,
  openSkillPlaceholderDialog,
  closeSkillPlaceholderDialog,
  requestRecallMode,
  confirmRewindWithPatch,
  confirmRewindMessageOnly,
  cancelRewindConfirm,
  cancelPendingRewindConfirm,
  refreshRuntimeLogs,
  openRuntimeLogsDialog,
  closeRuntimeLogsDialog,
  clearRuntimeLogs,
  closeConfigSaveErrorDialog,
  openConfigSaveErrorDialog,
} = {} as ReturnType<typeof useShellDialogFlows>;
const {
  openConfigWindow,
  summonChatWindowFromConfig,
  closeWindowAndClearForeground,
  minimizeWindowAndClearForeground,
  openGithubRepository,
} = useWindowActions({
  isChatTauriWindow,
  closeWindow,
  minimizeWindow,
  freezeForegroundConversation,
});
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
const toolReviewRefreshTick = ref(0);
const currentChatTodos = ref<ChatTodoItem[]>([]);
const foregroundTailLatestReady = ref(true);
let pendingConversationScrollToBottomConversationId = "";
let pendingConversationScrollToBottomTimer = 0;
let pendingManualScrollToBottomConversationId = "";
let pendingManualScrollToBottomRequestId = "";

type SwitchConversationSnapshot = {
  conversationId: string;
  messages: ChatMessage[];
  hasMoreHistory: boolean;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context";
  currentTodo?: string;
  currentTodos?: ChatTodoItem[];
  unarchivedConversations?: UnarchivedConversationSummary[];
};

type ChatConversationKind = "local_unarchived" | "remote_im_contact";

type ConversationTodosUpdatedPayload = {
  conversationId?: string;
  currentTodo?: string;
  currentTodos?: ChatTodoItem[];
};

type ConversationPinUpdatedPayload = {
  conversationId?: string;
  isPinned?: boolean;
  pinIndex?: number | null;
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

function clearPendingManualScrollToBottom() {
  pendingManualScrollToBottomConversationId = "";
  pendingManualScrollToBottomRequestId = "";
}

function triggerConversationScrollToBottom(conversationId: string, reason: string) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  if (cid !== String(currentChatConversationId.value || "").trim()) return;
  conversationScrollToBottomRequest.value += 1;
  pendingConversationScrollToBottomConversationId = "";
  clearPendingConversationScrollToBottomFallback();
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
      kind: "local_unarchived" as const,
      messageCount: Number(item.messageCount || 0),
      unreadCount: Number(item.unreadCount || 0),
      agentId: String(item.agentId || "").trim(),
      departmentId: String(item.departmentId || "").trim(),
      departmentName: String(item.departmentName || "").trim(),
      parentConversationId: String(item.parentConversationId || "").trim() || undefined,
      forkMessageCursor: String(item.forkMessageCursor || "").trim() || undefined,
      workspaceLabel: String(item.workspaceLabel || "").trim() || "默认工作空间",
      isActive: !!item.isActive,
      isMainConversation: !!item.isMainConversation,
      isPinned: !!item.isPinned,
      pinIndex: Number.isFinite(Number(item.pinIndex)) ? Number(item.pinIndex) : undefined,
      runtimeState: item.runtimeState,
      currentTodo: String(item.currentTodo || "").trim(),
      currentTodos: Array.isArray(item.currentTodos) ? item.currentTodos : [],
      detachedWindowOpen: !!item.detachedWindowOpen,
      detachedWindowLabel: String(item.detachedWindowLabel || "").trim() || undefined,
      updatedAt: item.lastMessageAt || item.updatedAt || "",
      lastMessageAt: item.lastMessageAt || item.updatedAt || "",
      previewMessages: Array.isArray(item.previewMessages) ? item.previewMessages : [],
      backgroundStatus:
        backgroundConversationBadgeMap.value[String(item.conversationId || "").trim()] || undefined,
    }));

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

function resolveRemoteConversationDepartmentName(boundDepartmentId?: string): string {
  const normalizedDepartmentId = String(boundDepartmentId || "").trim();
  if (!normalizedDepartmentId) return "主部门";
  return (
    config.departments.find((item) => String(item.id || "").trim() === normalizedDepartmentId)?.name
    || normalizedDepartmentId
  );
}

const chatRemoteImConversationItems = computed<ChatConversationOverviewItem[]>(() =>
  remoteImContactConversations.value.map((item) => ({
    conversationId: String(item.conversationId || "").trim(),
    title: String(item.title || "").trim() || String(item.contactDisplayName || "").trim(),
    kind: "remote_im_contact",
    remoteContactId: String(item.contactId || "").trim(),
    remoteContactDisplayName: String(item.contactDisplayName || "").trim(),
    messageCount: Number(item.messageCount || 0),
    departmentId: String(item.boundDepartmentId || "").trim() || undefined,
    departmentName: [
      String(item.channelName || "").trim(),
      resolveRemoteConversationDepartmentName(item.boundDepartmentId),
    ].filter(Boolean).join(" · "),
    updatedAt: item.lastMessageAt || item.updatedAt || "",
    lastMessageAt: item.lastMessageAt || item.updatedAt || "",
    previewMessages: Array.isArray(item.previewMessages) ? item.previewMessages : [],
  })),
);

const chatConversationItems = computed<ChatConversationOverviewItem[]>(() => ([
  ...chatUnarchivedConversationItems.value,
  ...chatRemoteImConversationItems.value,
]));
const {
  chatWorkspaceName,
  chatWorkspaceRootPath,
  chatWorkspacePickerOpen,
  chatWorkspaceChoices,
  refreshChatWorkspaceState,
  openChatWorkspacePicker: openChatWorkspacePickerBase,
  closeChatWorkspacePicker: closeChatWorkspacePickerBase,
  saveChatWorkspaces,
} = useChatWorkspace({
  activeApiConfigId: currentForegroundApiConfigId,
  activeAgentId: currentForegroundAgentId,
  activeConversationId: computed(() => currentChatConversationId.value),
  setStatus,
  setStatusError,
});
const {
  chatWorkspaceDraftChoices,
  chatWorkspacePickerSaving,
  openChatWorkspacePicker,
  closeChatWorkspacePicker,
  addChatWorkspace,
  setChatWorkspaceAsMain,
  setChatWorkspaceAccess,
  removeChatWorkspace,
  saveChatWorkspacePicker,
} = useChatWorkspacePickerFlow({
  chatWorkspaceChoices,
  openChatWorkspacePickerBase,
  closeChatWorkspacePickerBase,
  saveChatWorkspaces,
  setStatus,
  setStatusError,
  workspaceAlreadyExistsText: tr("config.tools.workspaceAlreadyExists"),
});
const selectedPersonaEditor = computed(
  () => personas.value.find((p) => p.id === personaEditorId.value) ?? null,
);
const toolDepartment = computed(() =>
  config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant)
  ?? config.departments.find((item) => (item.agentIds || []).includes(assistantDepartmentAgentId.value))
  ?? null,
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
const currentForegroundPersonaAvatarUrl = computed(
  () => resolveAvatarUrl(currentForegroundPersona.value?.avatarPath, currentForegroundPersona.value?.avatarUpdatedAt),
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
      isFrontSpeaking: id === currentForegroundAgentId.value,
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
const chatMentionOptions = computed<ChatMentionTarget[]>(() => {
  const seen = new Set<string>();
  const next: ChatMentionTarget[] = [];
  for (const department of config.departments) {
    const departmentId = String(department.id || "").trim();
    if (!departmentId) continue;
    const departmentName = String(department.name || "").trim() || departmentId;
    const firstAgentId = Array.isArray(department.agentIds)
      ? department.agentIds.map((item) => String(item || "").trim()).find((item) => !!item)
      : "";
    if (!firstAgentId || seen.has(firstAgentId)) continue;
    if (firstAgentId === currentForegroundAgentId.value) continue;
    const persona = personas.value.find((item) => String(item.id || "").trim() === firstAgentId);
    if (!persona || persona.isBuiltInUser || persona.isBuiltInSystem) continue;
    seen.add(firstAgentId);
    next.push({
      agentId: firstAgentId,
      agentName: String(persona.name || "").trim() || firstAgentId,
      departmentId,
      departmentName,
      avatarUrl: String(chatPersonaAvatarUrlMap.value[firstAgentId] || "").trim() || undefined,
    });
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
const defaultCreateConversationDepartmentId = computed(() => "assistant-department");
const {
  defaultApiTools,
  createApiProvider,
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
const createConversationDepartmentOptions = computed(() =>
  (config.departments || [])
    .filter((department) => {
      const departmentId = String(department.id || "").trim();
      if (!departmentId) return false;
      const apiConfigId = departmentPrimaryApiConfigId(department);
      if (!apiConfigId) return false;
      return config.apiConfigs.some((api) => api.id === apiConfigId && api.enableText);
    })
    .map((department) => {
      const ownerId = String((department.agentIds || [])[0] || "").trim();
      const owner = personas.value.find((persona) => String(persona.id || "").trim() === ownerId) ?? null;
      const apiConfigId = departmentPrimaryApiConfigId(department);
      const apiConfig = config.apiConfigs.find((api) => api.id === apiConfigId) ?? null;
      return {
        id: String(department.id || "").trim(),
        name: String(department.name || "").trim() || String(department.id || "").trim(),
        ownerName: String(owner?.name || "").trim() || ownerId || "未设置负责人",
        providerName: String(apiConfig?.name || apiConfig?.id || "").trim(),
        modelName: String(apiConfig?.model || "").trim(),
      };
    }),
);
const configDirty = computed(() => buildConfigSnapshotJson() !== lastSavedConfigJson.value);
const personaDirty = computed(() => buildPersonasSnapshotJson() !== lastSavedPersonasJson.value);
const responseStyleIds = computed(() => responseStyleOptions.map((item) => item.id));
const { visibleMessageBlocks, chatContextUsageRatio, chatUsagePercent } = useChatMessageBlocks({
  allMessages,
  activeChatApiConfig: currentForegroundApiConfig,
  perfDebug: PERF_DEBUG,
  perfNow,
});
const displayMessageBlocks = computed(() => visibleMessageBlocks.value);
const {
  terminalApprovalCurrent,
  listConversationTerminalApprovals,
  enqueueTerminalApprovalRequest,
  denyTerminalApproval,
  approveTerminalApproval,
} = useTerminalApproval({
  queue: terminalApprovalQueue,
  resolving: terminalApprovalResolving,
});

const terminalApprovalConversationItems = computed(() =>
  listConversationTerminalApprovals(currentChatConversationId.value)
);

function syncUserAliasFromPersona() {
  const next = (userPersona.value?.name || "").trim() || t("archives.roleUser");
  if (userAlias.value !== next) {
    userAlias.value = next;
  }
}

function isLocalOwnUserMessage(message?: ChatMessage | null): boolean {
  if (!message || message.role !== "user") return false;
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const origin = meta.origin as Record<string, unknown> | undefined;
  if (origin && origin.kind === "remote_im") return false;
  const speakerAgentId = String(message.speakerAgentId || meta.speakerAgentId || meta.speaker_agent_id || "").trim();
  return !speakerAgentId || speakerAgentId === "user-persona";
}

function isOptimisticOwnUserDraft(message?: ChatMessage | null): boolean {
  if (!message || message.role !== "user") return false;
  const messageId = String(message.id || "").trim();
  if (messageId.startsWith("__draft_user__:")) return true;
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  return meta._optimistic === true && isLocalOwnUserMessage(message);
}

function historyFlushedMessageKind(message?: ChatMessage | null): string {
  const meta = (message?.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
  return String(messageMeta.kind || meta.messageKind || "").trim();
}

type SingleOwnUserHistoryFlushFastPathResult = {
  messageId: string;
};

function applySingleOwnUserHistoryFlushFastPath(messages: ChatMessage[]): SingleOwnUserHistoryFlushFastPathResult | null {
  if (messages.length !== 1) return null;
  const committedMessage = messages[0];
  if (!isLocalOwnUserMessage(committedMessage)) return null;
  if (historyFlushedMessageKind(committedMessage) === "summary_context_seed") return null;

  const draftIndex = allMessages.value.findIndex((message) => isOptimisticOwnUserDraft(message));
  if (draftIndex < 0) return null;

  const committedId = String(committedMessage.id || "").trim();
  if (committedId) {
    const existingIndex = allMessages.value.findIndex(
      (message, index) => index !== draftIndex && String(message.id || "").trim() === committedId,
    );
    if (existingIndex >= 0) {
      allMessages.value.splice(draftIndex, 1);
      foregroundTailLatestReady.value = true;
      return { messageId: committedId };
    }
  }

  allMessages.value.splice(draftIndex, 1, committedMessage);
  foregroundTailLatestReady.value = true;
  return { messageId: committedId };
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

async function updateForegroundDepartmentPrimaryApiConfig(value: string) {
  const nextId = String(value || "").trim();
  if (!nextId) return;
  if (!config.apiConfigs.some((item) => String(item.id || "").trim() === nextId)) {
    console.warn("[聊天模型] 选择的模型不存在，忽略更新", { nextId });
    return;
  }
  const currentDepartmentId = String(currentForegroundDepartmentId.value || "").trim();
  const currentDepartment = config.departments.find(
    (item) => String(item.id || "").trim() === currentDepartmentId,
  );
  if (!currentDepartment) {
    console.warn("[聊天模型] 当前前台部门不存在，忽略更新", { currentDepartmentId, nextId });
    return;
  }
  const previousDepartment = {
    apiConfigId: String(currentDepartment.apiConfigId || "").trim(),
    apiConfigIds: [...(currentDepartment.apiConfigIds || [])],
    updatedAt: String(currentDepartment.updatedAt || ""),
  };
  const previousAssistantDepartmentApiConfigId = String(config.assistantDepartmentApiConfigId || "").trim();
  const previousSelectedApiConfigId = String(config.selectedApiConfigId || "").trim();
  const changed = applyDepartmentPrimaryApiConfigLocally(currentDepartment, nextId);
  if (!changed) return;
  try {
    await invokeTauri<AppConfig>("set_department_primary_api_config", {
      input: {
        departmentId: currentDepartmentId,
        apiConfigId: nextId,
      },
    });
  } catch (error) {
    currentDepartment.apiConfigId = previousDepartment.apiConfigId;
    currentDepartment.apiConfigIds = previousDepartment.apiConfigIds;
    currentDepartment.updatedAt = previousDepartment.updatedAt;
    config.assistantDepartmentApiConfigId = previousAssistantDepartmentApiConfigId;
    config.selectedApiConfigId = previousSelectedApiConfigId;
    setStatusError("status.saveConfigFailed", error);
  }
}

function updateAssistantDepartmentApiConfigId(value: string) {
  if (detachedChatWindow.value) {
    const nextId = String(value || "").trim();
    if (nextId && !config.apiConfigs.some((item) => item.id === nextId && item.enableText)) {
      setStatus("当前模型不可用，请重新选择。");
      return;
    }
    detachedTemporaryApiConfigId.value = nextId;
    return;
  }
  void updateForegroundDepartmentPrimaryApiConfig(value);
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

function updateInstructionPresets(value: PromptCommandPreset[]) {
  instructionPresets.value = Array.isArray(value)
    ? value
        .map((item) => ({
          id: String(item?.id || "").trim(),
          name: String(item?.prompt || item?.name || "").trim(),
          prompt: String(item?.prompt || item?.name || "").trim(),
        }))
        .filter((item) => !!item.id && !!item.prompt)
    : [];
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
  const apiConfig = currentForegroundApiConfig.value;
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
  avatarSaving,
  avatarError,
  toolAgentId: assistantDepartmentAgentId,
  selectedApiConfig,
  selectedApiProvider,
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
  instructionPresets,
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
  loadBootstrapSnapshot,
  saveConfig,
  captureHotkey,
  loadPersonas,
  loadChatSettings,
  savePersonas,
  patchChatSettings,
  saveChatPreferences,
  patchConversationApiSettings,
  saveConversationApiSettings,
  restoreLastSavedConfigSnapshot,
} = configPersistence;
const chatRuntime = useChatRuntime({
  t: tr,
  setStatus,
  setStatusError,
  setChatError: (text) => {
    chatErrorText.value = text;
  },
  activeChatApiConfigId: currentForegroundApiConfigId,
  assistantDepartmentAgentId: currentForegroundAgentId,
  currentConversationId: currentChatConversationId,
  chatting,
  forcingArchive,
  compactingConversation,
  suppressNextCompactionReload,
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
({
  runtimeLogsDialogOpen,
  runtimeLogs,
  runtimeLogsLoading,
  runtimeLogsError,
  configSaveErrorDialogOpen,
  configSaveErrorDialogTitle,
  configSaveErrorDialogBody,
  configSaveErrorDialogKind,
  skillPlaceholderDialogOpen,
  forceArchiveActionDialogOpen,
  forceArchivePreviewLoading,
  forceArchivePreview,
  forceCompactionPreview,
  rewindConfirmDialogOpen,
  rewindConfirmCanUndoPatch,
  openForceArchiveActionDialog,
  closeForceArchiveActionDialog,
  confirmForceCompactionAction,
  confirmForceArchiveAction,
  confirmDeleteConversationFromArchiveDialog,
  openSkillPlaceholderDialog,
  closeSkillPlaceholderDialog,
  requestRecallMode,
  confirmRewindWithPatch,
  confirmRewindMessageOnly,
  cancelRewindConfirm,
  cancelPendingRewindConfirm,
  refreshRuntimeLogs,
  openRuntimeLogsDialog,
  closeRuntimeLogsDialog,
  clearRuntimeLogs,
  closeConfigSaveErrorDialog,
  openConfigSaveErrorDialog,
} = useShellDialogFlows({
  t: tr,
  configTab,
  allMessages,
  tauriWindowLabel,
  currentForegroundApiConfigId,
  currentForegroundAgentId,
  currentForegroundDepartmentId,
  currentChatConversationId,
  unarchivedConversations,
  setStatus,
  setStatusError,
  forceCompactNow,
  forceArchiveNow,
  deleteUnarchivedConversationFromArchives,
}));

async function deleteUnarchivedConversationFromArchives(conversationId: string) {
  const normalizedConversationId = String(conversationId || "").trim();
  if (!normalizedConversationId) return;
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  const deletingCurrentConversation = currentConversationId === normalizedConversationId;
  if (detachedChatWindow.value && deletingCurrentConversation) {
    void deleteUnarchivedConversationFromArchivesRaw(normalizedConversationId).catch((error) => {
      console.error("[独立聊天窗口] 后台删除会话失败", error);
    });
    await getCurrentWindow().close();
    return;
  }
  if (deletingCurrentConversation) {
    const optimisticNextConversationId = pickForegroundConversationId(
      unarchivedConversations.value.filter((item) => String(item.conversationId || "").trim() !== normalizedConversationId),
    );
    if (optimisticNextConversationId) {
      try {
        conversationForegroundSyncing.value = true;
        const snapshot = await requestConversationLightSnapshot(optimisticNextConversationId);
        applyConversationSnapshot({
          ...snapshot,
          unarchivedConversations: unarchivedConversations.value,
        });
      } finally {
        conversationForegroundSyncing.value = false;
      }
    } else {
      clearForegroundConversation("delete_unarchived_conversation_optimistic_empty");
    }
  }
  const result = await deleteUnarchivedConversationFromArchivesRaw(normalizedConversationId);
  if (!deletingCurrentConversation) return;
  if (String(currentChatConversationId.value || "").trim()) return;
  await recoverForegroundConversationFromOverview("delete_unarchived_conversation", String(result?.activeConversationId || "").trim() || null);
}

async function handleConfirmForceArchiveAction() {
  if (!detachedChatWindow.value) {
    await confirmForceArchiveAction();
    return;
  }
  const conversationId = String(currentChatConversationId.value || "").trim();
  const apiConfigId = String(currentForegroundApiConfigId.value || "").trim();
  const agentId = String(currentForegroundAgentId.value || "").trim();
  if (!conversationId || !apiConfigId || !agentId) {
    setStatus("当前没有可归档的会话。");
    closeForceArchiveActionDialog();
    return;
  }
  closeForceArchiveActionDialog();
  void invokeTauri("force_archive_current", {
      input: {
        session: {
          apiConfigId,
          agentId,
          conversationId,
        },
        targetConversationId: null,
      },
    }).catch((error) => {
      console.error("[独立聊天窗口] 后台归档会话失败", error);
    });
  await getCurrentWindow().close();
}

async function refreshChatUnarchivedConversations() {
  if (conversationForegroundSyncing.value) return;
  if (detachedChatWindow.value) {
    await refreshRemoteImConversationOverview();
    return;
  }
  try {
    conversationForegroundSyncing.value = true;
    await refreshUnarchivedConversationOverview();
    await refreshRemoteImConversationOverview();
    const currentConversationId = String(currentChatConversationId.value || "").trim();
    const nextConversationId = currentConversationId && unarchivedConversations.value.some((item) =>
      String(item.conversationId || "").trim() === currentConversationId
    )
      ? currentConversationId
      : pickForegroundConversationId(unarchivedConversations.value);
    if (!nextConversationId) {
      clearForegroundConversation("refresh_unarchived_conversations_empty");
      return;
    }
    const snapshot = await requestConversationLightSnapshot(nextConversationId);
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
  currentChatTodos.value = [];
  allMessages.value = [];
  hasMoreBackendHistory.value = false;
  foregroundTailLatestReady.value = true;
  clearPendingManualScrollToBottom();
  chatFlow.freezeForegroundRoundState();
  console.warn("[聊天追踪][前台会话] 已清空", {
    windowLabel: tauriWindowLabel.value,
    reason,
    previousConversationId,
  });
}

async function initializeDetachedChatWindow() {
  if (!detachedChatWindow.value) return;
  try {
    const info = await invokeTauri<DetachedChatWindowInfo>("get_detached_chat_window_info");
    const conversationId = String(info?.conversationId || "").trim();
    if (!info?.detached || !conversationId) {
      setStatus("独立聊天窗口缺少绑定会话，窗口即将关闭。");
      try {
        await getCurrentWindow().close();
      } catch (closeError) {
        console.error("[独立聊天窗口] 缺少绑定会话时关闭窗口失败", closeError);
        setStatusError("status.requestFailed", closeError);
      }
      return;
    }
    detachedChatConversationId.value = conversationId;
    currentChatConversationId.value = conversationId;
    sideConversationListVisible.value = false;
    await refreshRemoteImConversationOverview();
    const snapshot = await requestConversationLightSnapshot(conversationId);
    applyConversationSnapshot(snapshot);
    await nextTick();
    maybeResumeForegroundStreamingDraft(conversationId, "detached_window_init");
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  }
}

async function handleCloseWindow() {
  if (detachedChatWindow.value) {
    await getCurrentWindow().close();
    return;
  }
  await closeWindowAndClearForeground();
}

async function detachCurrentConversationToWindow() {
  console.info("[独立聊天窗口][前端链路] UnifiedWindowApp 进入 detachCurrentConversationToWindow", {
    windowLabel: tauriWindowLabel.value,
    detachedChatWindow: detachedChatWindow.value,
    currentConversationId: String(currentChatConversationId.value || "").trim(),
    chatting: chatting.value,
    forcingArchive: forcingArchive.value,
    compactingConversation: compactingConversation.value,
    isMainConversation: !!currentForegroundConversationSummary.value?.isMainConversation,
  });
  setStatus("正在打开独立聊天窗口...");
  if (detachedChatWindow.value) {
    console.warn("[独立聊天窗口][前端链路] 当前已经是独立窗口，忽略独立窗口请求");
    return;
  }
  const conversationId = String(currentChatConversationId.value || "").trim();
  if (!conversationId || chatting.value || forcingArchive.value || compactingConversation.value) {
    console.warn("[独立聊天窗口][前端链路] 当前状态不允许独立窗口", {
      conversationId,
      chatting: chatting.value,
      forcingArchive: forcingArchive.value,
      compactingConversation: compactingConversation.value,
    });
    return;
  }
  if (currentForegroundConversationSummary.value?.isMainConversation) {
    console.warn("[独立聊天窗口][前端链路] 主会话不允许独立窗口", { conversationId });
    setStatus("主会话不能独立打开，请选择一个子会话。");
    return;
  }
  try {
    console.info("[独立聊天窗口][前端链路] 准备 invoke detach_current_conversation_to_window", {
      conversationId,
    });
    void invokeTauri<{ conversationId: string; windowLabel: string; mainConversationId?: string | null }>("detach_current_conversation_to_window", {
      input: { conversationId },
    }).then((result) => {
      console.info("[独立聊天窗口][前端链路] invoke detach_current_conversation_to_window 已返回", result);
      void refreshUnarchivedConversationOverview();
    }).catch((error) => {
      console.error("[独立聊天窗口][前端链路] 打开独立窗口失败", error);
      setStatusError("status.loadMessagesFailed", error);
      void refreshUnarchivedConversationOverview();
    });
    clearForegroundConversation("detach_current_conversation");
    const mainConversationId = String(unarchivedConversations.value.find((item) => !!item.isMainConversation)?.conversationId || "").trim();
    if (mainConversationId) {
      await switchUnarchivedConversation(mainConversationId);
    } else {
      await refreshChatUnarchivedConversations();
    }
    setStatus("已发送独立聊天窗口请求");
  } catch (error) {
    console.error("[独立聊天窗口][前端链路] 打开独立窗口失败", error);
    setStatusError("status.loadMessagesFailed", error);
  }
}

async function sendChatFromCurrentWindow() {
  if (detachedChatWindow.value) {
    const temporaryApiConfigId = String(detachedTemporaryApiConfigId.value || "").trim();
    if (temporaryApiConfigId && !config.apiConfigs.some((item) => item.id === temporaryApiConfigId && item.enableText)) {
      setStatus("临时模型已不可用，请重新选择模型。");
      return;
    }
  }
  await chatFlow.sendChat();
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

function matchesForegroundConversation(conversationId?: string | null): boolean {
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (!currentConversationId) return false;
  const targetConversationId = String(conversationId || "").trim();
  return !targetConversationId || targetConversationId === currentConversationId;
}

function formalizeConversationMessages(messages: ChatMessage[]): ChatMessage[] {
  return ensureConversationMessageIds(messages)
    .filter((item) => !String(item?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX));
}

function freezeConversationMessages(messages: ChatMessage[]): ChatMessage[] {
  return ensureConversationMessageIds(messages).map((message) => {
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

function isAssistantDraftMessage(message?: ChatMessage | null): boolean {
  return String(message?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX);
}

function insertMessagesBeforeAssistantDraft(
  messages: ChatMessage[],
  incoming: ChatMessage[],
): ChatMessage[] {
  if (!Array.isArray(incoming) || incoming.length <= 0) return messages;
  const draftIdx = messages.findIndex((message) => isAssistantDraftMessage(message));
  if (draftIdx < 0) {
    return [...messages, ...incoming];
  }
  return [
    ...messages.slice(0, draftIdx),
    ...incoming,
    ...messages.slice(draftIdx),
  ];
}

function currentConversationRuntimeState(conversationId?: string | null) {
  const cid = String(conversationId || "").trim();
  if (!cid) return "";
  return String(
    unarchivedConversations.value.find((item) => String(item.conversationId || "").trim() === cid)?.runtimeState || "",
  ).trim();
}

function maybeResumeForegroundStreamingDraft(conversationId?: string | null, reason = "unknown") {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  if (cid !== String(currentChatConversationId.value || "").trim()) return;
  if (currentConversationRuntimeState(cid) !== "assistant_streaming") return;
  console.info("[聊天追踪][草稿恢复] 尝试恢复", {
    conversationId: cid,
    reason,
    currentMessageCount: allMessages.value.length,
  });
  chatFlow.resumeForegroundStreamingRound();
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

function messageContentSignature(message?: ChatMessage | null): string {
  return [
    String(message?.id || "").trim(),
    String(message?.createdAt || "").trim(),
    String(message?.role || "").trim(),
    String(message?.speakerAgentId || "").trim(),
    JSON.stringify(message?.providerMeta || {}),
    JSON.stringify(message?.parts || []),
    JSON.stringify(message?.extraTextBlocks || []),
    JSON.stringify(message?.toolCall || []),
  ].join("|");
}

function reuseStableMessageReferences(nextMessages: ChatMessage[], previousMessages: ChatMessage[]): ChatMessage[] {
  if (!Array.isArray(nextMessages) || nextMessages.length <= 0) {
    return [];
  }
  const previousById = new Map<string, ChatMessage>();
  for (const message of Array.isArray(previousMessages) ? previousMessages : []) {
    const messageId = String(message?.id || "").trim();
    if (!messageId) continue;
    previousById.set(messageId, message);
  }
  return nextMessages.map((message) => {
    const messageId = String(message?.id || "").trim();
    if (!messageId) return message;
    const previous = previousById.get(messageId);
    if (!previous) return message;
    return messageContentSignature(previous) === messageContentSignature(message)
      ? previous
      : message;
  });
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
  if (!cid) return;
  const hasBackgroundBadge = !!backgroundConversationBadgeMap.value[cid];
  if (hasBackgroundBadge) {
    const next = { ...backgroundConversationBadgeMap.value };
    delete next[cid];
    backgroundConversationBadgeMap.value = next;
  }
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== cid) return item;
    if (Math.max(0, Number(item.unreadCount || 0)) <= 0) return item;
    changed = true;
    return {
      ...item,
      unreadCount: 0,
    };
  });
  if (changed) {
    unarchivedConversations.value = nextItems;
  }
}

function applyConversationOverviewAppendedMessage(
  conversationId: string,
  message: ChatMessage,
) {
  const cid = String(conversationId || "").trim();
  const messageId = String(message?.id || "").trim();
  if (!cid || !message || !messageId || isOverviewDraftMessage(message)) return;
  const preview = previewMessageFromChatMessage(message);
  const messageAt = String(message.createdAt || "").trim();
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== cid) {
      return item;
    }
    const existingPreviewMessages = Array.isArray(item.previewMessages) ? item.previewMessages : [];
    if (existingPreviewMessages.some((previewItem) => String(previewItem.messageId || "").trim() === messageId)) {
      return item;
    }
    changed = true;
    return {
      ...item,
      messageCount: Math.max(0, Number(item.messageCount || 0)) + 1,
      unreadCount: Number(item.unreadCount || 0),
      updatedAt: messageAt || item.updatedAt,
      lastMessageAt: messageAt || item.lastMessageAt,
      previewMessages: [...existingPreviewMessages, preview].slice(-2),
    };
  });
  if (changed) {
    unarchivedConversations.value = sortUnarchivedConversationOverviewItems(nextItems);
  }
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

async function requestConversationMessageById(
  conversationId: string,
  messageId: string,
): Promise<ChatMessage> {
  return invokeTauri<ChatMessage>("get_unarchived_conversation_message_by_id", {
    input: {
      conversationId,
      messageId,
    },
  });
}

function replaceConversationMessage(messages: ChatMessage[], nextMessage: ChatMessage): ChatMessage[] {
  const targetMessageId = String(nextMessage?.id || "").trim();
  if (!targetMessageId || !Array.isArray(messages) || messages.length <= 0) {
    return messages;
  }
  let changed = false;
  const nextMessages = messages.map((message) => {
    if (String(message?.id || "").trim() !== targetMessageId) {
      return message;
    }
    changed = true;
    return nextMessage;
  });
  return changed ? reuseStableMessageReferences(nextMessages, messages) : messages;
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

async function refreshForegroundConversationMessageById(payload: { conversationId: string; messageId: string }) {
  const conversationId = String(payload?.conversationId || "").trim();
  const messageId = String(payload?.messageId || "").trim();
  if (!conversationId || !messageId) return;
  try {
    const refreshedMessage = freezeConversationMessages([
      await requestConversationMessageById(conversationId, messageId),
    ])[0];
    if (!refreshedMessage) return;

    const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[conversationId] || []);
    const nextCached = replaceConversationMessage(cachedDisplay, refreshedMessage);
    if (nextCached !== cachedDisplay) {
      cacheConversationMessages(conversationId, nextCached);
    }

    if (String(currentChatConversationId.value || "").trim() !== conversationId) {
      return;
    }
    const nextMessages = replaceConversationMessage(allMessages.value, refreshedMessage);
    if (nextMessages === allMessages.value) {
      return;
    }
    allMessages.value = nextMessages;
    cacheConversationMessages(conversationId, nextMessages);
  } catch (error) {
    console.warn("[会话缓存] 单条消息刷新失败", {
      conversationId,
      messageId,
      error,
    });
  }
}

async function loadOlderConversationHistory() {
  const conversationId = String(currentChatConversationId.value || "").trim();
  if (!conversationId || loadingOlderConversationHistory.value || !hasMoreBackendHistory.value) {
    return;
  }
  const apiConfigId = String(currentForegroundApiConfigId.value || "").trim();
  const agentId = String(currentForegroundAgentId.value || "").trim();
  if (!apiConfigId || !agentId) return;

  const formalMessages = formalizeConversationMessages(allMessages.value);
  const oldestMessageId = String(formalMessages[0]?.id || "").trim();
  if (!oldestMessageId) {
    hasMoreBackendHistory.value = false;
    return;
  }

  loadingOlderConversationHistory.value = true;
  try {
    const result = await invokeTauri<{ messages: ChatMessage[]; hasMore: boolean }>("get_active_conversation_messages_before", {
      input: {
        session: {
          apiConfigId,
          agentId,
          conversationId,
        },
        beforeMessageId: oldestMessageId,
        limit: OLDER_HISTORY_PAGE_SIZE,
      },
    });
    if (
      String(currentChatConversationId.value || "").trim() !== conversationId
      || String(currentForegroundApiConfigId.value || "").trim() !== apiConfigId
      || String(currentForegroundAgentId.value || "").trim() !== agentId
    ) {
      return;
    }
    const previousMessages = Array.isArray(allMessages.value) ? allMessages.value : [];
    const incomingMessages = freezeConversationMessages(Array.isArray(result?.messages) ? result.messages : []);
    const existingIds = new Set(previousMessages.map((item) => String(item?.id || "").trim()).filter(Boolean));
    const uniqueIncoming = incomingMessages.filter((item) => {
      const messageId = String(item?.id || "").trim();
      return !!messageId && !existingIds.has(messageId);
    });
    const nextMessages = reuseStableMessageReferences([...uniqueIncoming, ...previousMessages], previousMessages);
    allMessages.value = nextMessages;
    cacheConversationMessages(conversationId, nextMessages);
    hasMoreBackendHistory.value = !!result?.hasMore;
  } catch (error) {
    console.warn("[会话缓存] 向上补历史失败", {
      conversationId,
      error,
    });
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    loadingOlderConversationHistory.value = false;
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
  const nextMerged = merged.length > 0 ? merged : cachedDisplay;
  return reuseStableMessageReferences(nextMerged, cachedDisplay);
}

async function applyConversationMessagesAfterSynced(payload: ConversationMessagesAfterSyncedPayload) {
  const conversationId = String(payload?.conversationId || "").trim();
  const requestId = String(payload?.requestId || "").trim();
  if (!conversationId) return;
  if (payload?.error) {
    console.warn("[会话缓存] 异步补消息失败", {
      conversationId,
      requestId,
      error: payload.error,
    });
    if (
      requestId
      && requestId === pendingManualScrollToBottomRequestId
      && conversationId === pendingManualScrollToBottomConversationId
    ) {
      clearPendingManualScrollToBottom();
    }
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
    foregroundTailLatestReady.value = true;
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    if (
      requestId
      && requestId === pendingManualScrollToBottomRequestId
      && conversationId === pendingManualScrollToBottomConversationId
    ) {
      clearPendingManualScrollToBottom();
      triggerConversationScrollToBottom(conversationId, "manual_after_synced");
      return;
    }
    if (pendingConversationScrollToBottomConversationId === conversationId) {
      triggerConversationScrollToBottom(conversationId, "after_synced");
    }
  }
}

function applyConversationMessageAppended(payload?: ConversationMessageAppendedPayload | null) {
  const conversationId = String(payload?.conversationId || "").trim();
  const message = payload?.message || null;
  const messageId = String(message?.id || "").trim();
  if (!conversationId || !message || !messageId) return;

  const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[conversationId] || []);
  const cachedFormal = formalizeConversationMessages(cachedDisplay);
  const messageAlreadyCached = cachedFormal.some((item) => String(item?.id || "").trim() === messageId);
  const nextCached = messageAlreadyCached
    ? cachedFormal
    : [...cachedFormal, message];
  cacheConversationMessages(conversationId, nextCached);

  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (conversationId !== currentConversationId) {
    if (!messageAlreadyCached) {
      applyConversationOverviewAppendedMessage(conversationId, message);
    }
    setConversationBadge(conversationId, "completed");
    return;
  }

  const existing = allMessages.value.filter((item) => String(item?.id || "").trim() !== messageId);
  const stableMessage = reuseStableMessageReferences([message], allMessages.value)[0] || message;
  allMessages.value = [...existing, stableMessage];
  foregroundTailLatestReady.value = true;
  clearConversationBadge(conversationId);
  updateForegroundConversationOverviewFromMessages(conversationId, message);
}

function applyConversationSnapshot(snapshot: SwitchConversationSnapshot) {
  const nextConversationId = String(snapshot.conversationId || "").trim();
  const detachedConversationId = String(detachedChatConversationId.value || "").trim();
  if (
    detachedChatWindow.value
    && detachedConversationId
    && nextConversationId
    && nextConversationId !== detachedConversationId
  ) {
    console.warn("[独立窗口] 跳过非绑定会话快照", {
      windowLabel: tauriWindowLabel.value,
      detachedConversationId,
      snapshotConversationId: nextConversationId,
    });
    return;
  }
  const previousMessages = Array.isArray(allMessages.value) ? allMessages.value : [];
  const rawNextMessages = freezeConversationMessages(Array.isArray(snapshot.messages) ? snapshot.messages : []);
  const nextRuntimeState = String(snapshot.runtimeState || "").trim();
  const hasAssistantDraftInSnapshot = rawNextMessages.some((message) => isAssistantDraftMessage(message));
  if (!hasAssistantDraftInSnapshot && nextRuntimeState === "assistant_streaming") {
    const preservedDraft = [...previousMessages].reverse().find((message) => isAssistantDraftMessage(message));
    if (preservedDraft) {
      rawNextMessages.push(preservedDraft);
    }
  }
  const nextMessages = reuseStableMessageReferences(rawNextMessages, allMessages.value);
  currentChatConversationId.value = nextConversationId;
  currentChatTodos.value = Array.isArray(snapshot.currentTodos)
    ? snapshot.currentTodos
      .map((item) => ({
        content: String(item?.content || "").trim(),
        status: String(item?.status || "").trim() as ChatTodoItem["status"],
      }))
      .filter((item) => item.content && (item.status === "pending" || item.status === "in_progress" || item.status === "completed"))
    : [];
  allMessages.value = nextMessages;
  hasMoreBackendHistory.value = !!snapshot.hasMoreHistory;
  foregroundTailLatestReady.value = true;
  cacheConversationMessages(nextConversationId, nextMessages);
  clearConversationBadge(nextConversationId);
  if (Array.isArray(snapshot.unarchivedConversations)) {
    unarchivedConversations.value = snapshot.unarchivedConversations;
  }
  if (nextRuntimeState === "assistant_streaming") {
    maybeResumeForegroundStreamingDraft(nextConversationId, "apply_snapshot");
  }
  scheduleConversationScrollToBottomFallback(nextConversationId);
}

function applyConversationTodosUpdated(payload?: ConversationTodosUpdatedPayload | null) {
  const conversationId = String(payload?.conversationId || "").trim();
  if (!conversationId) return;
  const nextTodos = Array.isArray(payload?.currentTodos)
    ? payload.currentTodos
      .map((item) => ({
        content: String(item?.content || "").trim(),
        status: String(item?.status || "").trim() as ChatTodoItem["status"],
      }))
      .filter((item) => item.content && (item.status === "pending" || item.status === "in_progress" || item.status === "completed"))
    : [];
  if (conversationId === String(currentChatConversationId.value || "").trim()) {
    currentChatTodos.value = nextTodos;
  }
  const nextCurrentTodo = String(payload?.currentTodo || "").trim();
  unarchivedConversations.value = unarchivedConversations.value.map((item) =>
    String(item.conversationId || "").trim() === conversationId
      ? {
        ...item,
        currentTodo: nextCurrentTodo,
        currentTodos: nextTodos,
      }
      : item
  );
}

function applyConversationOverviewUpdated(payload?: ConversationOverviewUpdatedPayload | null) {
  if (!Array.isArray(payload?.unarchivedConversations)) return;
  unarchivedConversations.value = payload.unarchivedConversations;
}

function applyConversationPinUpdated(payload?: ConversationPinUpdatedPayload | null) {
  const conversationId = String(payload?.conversationId || "").trim();
  if (!conversationId) return;
  const isPinned = !!payload?.isPinned;
  const pinIndex = Number.isFinite(Number(payload?.pinIndex)) ? Number(payload?.pinIndex) : undefined;
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== conversationId) {
      return item;
    }
    changed = true;
    return {
      ...item,
      isPinned,
      pinIndex,
    };
  });
  if (!changed) return;
  unarchivedConversations.value = sortUnarchivedConversationOverviewItems(nextItems);
}

function isOverviewDraftMessage(message?: ChatMessage): boolean {
  const messageId = String(message?.id || "").trim();
  return messageId.startsWith(DRAFT_ASSISTANT_ID_PREFIX) || messageId.startsWith("__draft_user__:");
}

function previewMessageFromChatMessage(message: ChatMessage): ConversationPreviewMessage {
  const parts = Array.isArray(message.parts) ? message.parts : [];
  const textPreview = parts
    .filter((part) => part && typeof part === "object" && (part as { type?: unknown }).type === "text")
    .map((part) => String((part as { text?: unknown }).text || "").trim())
    .filter(Boolean)
    .join(" | ")
    .slice(0, 160);
  const providerMeta = (message.providerMeta || {}) as Record<string, unknown>;
  const attachmentEntries = Array.isArray(providerMeta.attachments) ? providerMeta.attachments : [];
  const hasPdfAttachment = attachmentEntries.some((entry) => {
    const item = entry as Record<string, unknown>;
    return String(item?.mime || "").toLowerCase().includes("pdf");
  });
  return {
    messageId: String(message.id || "").trim(),
    role: (String(message.role || "").trim() || "assistant") as ConversationPreviewMessage["role"],
    speakerAgentId: String(message.speakerAgentId || "").trim() || undefined,
    createdAt: String(message.createdAt || "").trim() || undefined,
    textPreview: textPreview || undefined,
    hasImage: parts.some((part) => part && typeof part === "object" && (part as { type?: unknown }).type === "image"),
    hasPdf: hasPdfAttachment,
    hasAudio: parts.some((part) => part && typeof part === "object" && (part as { type?: unknown }).type === "audio"),
    hasAttachment: attachmentEntries.length > 0,
  };
}

function unarchivedConversationActivityAt(item: UnarchivedConversationSummary): string {
  return String(item.lastMessageAt || item.updatedAt || "").trim();
}

function sortUnarchivedConversationOverviewItems(
  items: UnarchivedConversationSummary[],
): UnarchivedConversationSummary[] {
  return [...items].sort((a, b) => {
    if (!!a.isMainConversation !== !!b.isMainConversation) {
      return Number(!!b.isMainConversation) - Number(!!a.isMainConversation);
    }
    if (!!a.isPinned !== !!b.isPinned) {
      return Number(!!b.isPinned) - Number(!!a.isPinned);
    }
    if (a.isPinned && b.isPinned) {
      const aIndex = Number.isFinite(Number(a.pinIndex)) ? Number(a.pinIndex) : Number.MAX_SAFE_INTEGER;
      const bIndex = Number.isFinite(Number(b.pinIndex)) ? Number(b.pinIndex) : Number.MAX_SAFE_INTEGER;
      return aIndex - bIndex || String(a.conversationId || "").localeCompare(String(b.conversationId || ""));
    }
    const aActivity = unarchivedConversationActivityAt(a);
    const bActivity = unarchivedConversationActivityAt(b);
    return bActivity.localeCompare(aActivity) || String(a.conversationId || "").localeCompare(String(b.conversationId || ""));
  });
}

function updateForegroundConversationOverviewFromMessages(
  conversationId: string,
  assistantMessage?: ChatMessage | null,
) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (cid !== currentConversationId) return;
  const formalMessages = allMessages.value
    .filter((message) => !isOverviewDraftMessage(message));
  const nextMessages = assistantMessage
    ? [...formalMessages, assistantMessage].filter((message, index, items) => {
      const messageId = String(message?.id || "").trim();
      if (!messageId) return true;
      return items.findIndex((item) => String(item?.id || "").trim() === messageId) === index;
    })
    : formalMessages;
  const previewMessages = nextMessages
    .slice(-2)
    .map(previewMessageFromChatMessage);
  const lastMessage = nextMessages[nextMessages.length - 1];
  const lastMessageAt = String(lastMessage?.createdAt || "").trim();
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== cid) {
      return item;
    }
    changed = true;
    return {
      ...item,
      messageCount: nextMessages.length,
      updatedAt: lastMessageAt || item.updatedAt,
      lastMessageAt: lastMessageAt || item.lastMessageAt,
      previewMessages,
    };
  });
  if (changed) {
    unarchivedConversations.value = sortUnarchivedConversationOverviewItems(nextItems);
  }
}

function maybeUpdateForegroundConversationOverviewFromLoadedMessages(
  conversationId: string,
  messages: ChatMessage[],
  remainingCount: number,
) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  const currentConversationId = String(currentChatConversationId.value || "").trim();
  if (cid !== currentConversationId) return;
  const formalMessages = (Array.isArray(messages) ? messages : [])
    .filter((message) => !isOverviewDraftMessage(message));
  const requiredPreviewCount = Math.min(2, Math.max(0, Number(remainingCount) || 0));
  if (requiredPreviewCount <= 0) return;
  if (formalMessages.length < requiredPreviewCount) return;
  const previewMessages = formalMessages
    .slice(-requiredPreviewCount)
    .map(previewMessageFromChatMessage);
  const lastMessage = formalMessages[formalMessages.length - 1];
  const lastMessageAt = String(lastMessage?.createdAt || "").trim();
  let changed = false;
  const nextItems = unarchivedConversations.value.map((item) => {
    if (String(item.conversationId || "").trim() !== cid) {
      return item;
    }
    changed = true;
    return {
      ...item,
      messageCount: Math.max(0, Number(remainingCount) || formalMessages.length),
      updatedAt: lastMessageAt || item.updatedAt,
      lastMessageAt: lastMessageAt || item.lastMessageAt,
      previewMessages,
    };
  });
  if (changed) {
    unarchivedConversations.value = sortUnarchivedConversationOverviewItems(nextItems);
  }
}

async function requestConversationLightSnapshot(conversationId?: string | null): Promise<SwitchConversationSnapshot> {
  return invokeTauri<SwitchConversationSnapshot>("get_foreground_conversation_light_snapshot", {
    input: {
      conversationId: String(conversationId || "").trim() || null,
      agentId: String(currentForegroundAgentId.value || "").trim() || null,
      limit: FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    },
  });
}

async function requestUnarchivedConversationOverview(): Promise<UnarchivedConversationSummary[]> {
  return invokeTauri<UnarchivedConversationSummary[]>("list_unarchived_conversations");
}

async function requestRemoteImConversationMessages(contactId: string): Promise<ChatMessage[]> {
  return invokeTauri<ChatMessage[]>("remote_im_get_contact_conversation_messages", {
    input: { contactId },
  });
}

async function refreshRemoteImConversationOverview() {
  remoteImContactConversations.value = await invokeTauri<RemoteImContactConversationSummary[]>("remote_im_list_contact_conversations");
}

async function refreshUnarchivedConversationOverview() {
  const items = await requestUnarchivedConversationOverview();
  unarchivedConversations.value = Array.isArray(items) ? items : [];
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
    const snapshot = await requestConversationLightSnapshot(nextConversationId);
    applyConversationSnapshot(snapshot);
  } finally {
    conversationForegroundSyncing.value = false;
  }
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
  const targetOverview = unarchivedConversations.value.find((item) => String(item.conversationId || "").trim() === cid);
  if (targetOverview?.detachedWindowOpen) {
    try {
      await invokeTauri<boolean>("focus_detached_chat_window_by_conversation", {
        input: { conversationId: cid },
      });
    } catch (error) {
      console.warn("[独立聊天窗口] 聚焦已占用会话失败", error);
    }
    return;
  }
  const previousConversationId = String(currentChatConversationId.value || "").trim();
  const startedAt = perfNow();
  try {
    conversationForegroundSyncing.value = true;
    if (previousConversationId) {
      cacheConversationMessages(previousConversationId, allMessages.value);
    }
    chatFlow.freezeForegroundRoundState();
    currentChatConversationId.value = cid;
    currentChatTodos.value = [];
    clearPendingManualScrollToBottom();
    const cachedDisplay = freezeConversationMessages(conversationMessageCache.value[cid] || []);
    allMessages.value = cachedDisplay;
    hasMoreBackendHistory.value = inferHasMoreHistoryFromSnapshot(cachedDisplay);
    foregroundTailLatestReady.value = false;
    maybeResumeForegroundStreamingDraft(cid, "switch_cached_display");
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
    const snapshot = await requestConversationLightSnapshot(cid);
    applyConversationSnapshot(snapshot);
    await nextTick();
    logForegroundPaintTrace(trace, "前台轻量快照已接管最新消息", {
      conversationId: cid,
      snapshotCount: Array.isArray(snapshot?.messages) ? snapshot.messages.length : 0,
      hasMoreHistory: !!snapshot?.hasMoreHistory,
    });
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    conversationForegroundSyncing.value = false;
  }
}

async function ensureLatestForegroundTailThenScrollToBottom() {
  const conversationId = String(currentChatConversationId.value || "").trim();
  if (!conversationId) return;
  if (foregroundTailLatestReady.value) {
    triggerConversationScrollToBottom(conversationId, "manual_ready");
    return;
  }
  try {
    const result = await invokeTauri<{ accepted: boolean; requestId: string }>("request_conversation_messages_after_async", {
      input: {
        conversationId,
        afterMessageId: buildConversationMessagesAfterAnchor(conversationId),
        fallbackLimit: BACKGROUND_CONVERSATION_CACHE_LIMIT,
      },
    });
    if (!result?.accepted) {
      triggerConversationScrollToBottom(conversationId, "manual_request_rejected");
      return;
    }
    pendingManualScrollToBottomConversationId = conversationId;
    pendingManualScrollToBottomRequestId = String(result.requestId || "").trim();
    if (!pendingManualScrollToBottomRequestId) {
      triggerConversationScrollToBottom(conversationId, "manual_request_missing_id");
    }
  } catch (error) {
    console.warn("[会话切换] 手动滚到底前请求尾部增量失败", {
      conversationId,
      error,
    });
    triggerConversationScrollToBottom(conversationId, "manual_request_failed");
  }
}

async function createUnarchivedConversation(input?: { title?: string; departmentId?: string; copyCurrent?: boolean }) {
  const departmentId =
    String(input?.departmentId || "").trim()
    || defaultCreateConversationDepartmentId.value;
  if (!departmentId) return;
  try {
    const copySourceConversationId = input?.copyCurrent
      ? String(currentChatConversationId.value || "").trim()
      : "";
    const result = await invokeTauri<{
      conversationId: string;
      unarchivedConversations?: UnarchivedConversationSummary[];
    }>("create_unarchived_conversation", {
      input: {
        departmentId,
        title: String(input?.title || "").trim() || null,
        copySourceConversationId: copySourceConversationId || null,
      },
    });
    const conversationId = String(result?.conversationId || "").trim();
    if (!conversationId) return;
    if (Array.isArray(result.unarchivedConversations)) {
      unarchivedConversations.value = result.unarchivedConversations;
    } else {
      await refreshUnarchivedConversationOverview();
    }
    const snapshot = await requestConversationLightSnapshot(conversationId);
    applyConversationSnapshot(snapshot);
  } catch (error) {
    setStatus(`转发到会话失败：${formatI18nError(tr, "status.requestFailed", error)}`);
  }
}

async function branchConversationFromSelection(payload: { count: number; messageIds: string[] }) {
  const sourceConversationId = String(currentChatConversationId.value || "").trim();
  const selectedMessageIds = Array.isArray(payload?.messageIds)
    ? payload.messageIds
        .map((item) => String(item || "").trim())
        .filter((item, index, array) => !!item && array.indexOf(item) === index)
    : [];
  if (
    !sourceConversationId
    || selectedMessageIds.length === 0
    || branchingConversation.value
    || forwardingConversationSelection.value
  ) return;
  branchingConversation.value = true;
  try {
    const result = await invokeTauri<{
      conversationId: string;
      title: string;
      warning?: string | null;
    }>("branch_unarchived_conversation_from_selection", {
      input: {
        sourceConversationId,
        selectedMessageIds,
      },
    });
    const conversationId = String(result?.conversationId || "").trim();
    if (!conversationId) return;
    await refreshUnarchivedConversationOverview();
    const warning = String(result?.warning || "").trim();
    if (detachedChatWindow.value) {
      try {
        await invokeTauri<{ conversationId: string; windowLabel: string }>("detach_current_conversation_to_window", {
          input: { conversationId },
        });
        if (warning) {
          setStatus(`会话分支已在新独立窗口打开（降级整理）：${warning}`);
        } else {
          setStatus(`已在新独立窗口打开会话分支：${String(result?.title || "").trim() || conversationId}`);
        }
      } catch (detachError) {
        console.error("[独立聊天窗口] 会话分支创建成功，但打开新独立窗口失败", detachError);
        setStatus(`会话分支已创建，但打开新独立窗口失败：${formatI18nError(tr, "status.requestFailed", detachError)}`);
      }
      return;
    }
    const snapshot = await requestConversationLightSnapshot(conversationId);
    applyConversationSnapshot(snapshot);
    if (warning) {
      setStatus(`会话分支创建完成（降级整理）：${warning}`);
    } else {
      setStatus(`已创建会话分支：${String(result?.title || "").trim() || conversationId}`);
    }
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    branchingConversation.value = false;
  }
}

async function forwardConversationFromSelection(payload: {
  count: number;
  messageIds: string[];
  targetConversationId: string;
}) {
  const sourceConversationId = String(currentChatConversationId.value || "").trim();
  const targetConversationId = String(payload?.targetConversationId || "").trim();
  const selectedMessageIds = Array.isArray(payload?.messageIds)
    ? payload.messageIds
        .map((item) => String(item || "").trim())
        .filter((item, index, array) => !!item && array.indexOf(item) === index)
    : [];
  if (
    !sourceConversationId
    || !targetConversationId
    || selectedMessageIds.length === 0
    || forcingArchive.value
    || branchingConversation.value
    || forwardingConversationSelection.value
  ) return;
  forwardingConversationSelection.value = true;
  try {
    const result = await invokeTauri<{
      targetConversationId: string;
      forwardedCount: number;
    }>("forward_unarchived_conversation_selection", {
      input: {
        sourceConversationId,
        targetConversationId,
        selectedMessageIds,
      },
    });
    const effectiveTargetConversationId = String(result?.targetConversationId || targetConversationId).trim();
    if (!effectiveTargetConversationId) return;
    await refreshUnarchivedConversationOverview();
    const snapshot = await requestConversationLightSnapshot(effectiveTargetConversationId);
    applyConversationSnapshot(snapshot);
    setStatus(`已转发到会话 ${Number(result?.forwardedCount || selectedMessageIds.length)} 条消息`);
  } catch (error) {
    setStatusError("status.loadMessagesFailed", error);
  } finally {
    forwardingConversationSelection.value = false;
  }
}

async function renameCurrentConversation(payload: { conversationId: string; title: string }) {
  const conversationId = String(payload?.conversationId || "").trim();
  const title = String(payload?.title || "").trim();
  if (!conversationId || !title) return;
  if (conversationId !== String(currentChatConversationId.value || "").trim()) return;
  try {
    const result = await invokeTauri<{ conversationId: string; title: string }>("rename_unarchived_conversation", {
      input: {
        conversationId,
        title,
      },
    });
    const nextTitle = String(result?.title || "").trim() || title;
    unarchivedConversations.value = unarchivedConversations.value.map((item) =>
      String(item.conversationId || "").trim() === conversationId
        ? {
          ...item,
          title: nextTitle,
        }
        : item
    );
    setStatus(t("status.conversationRenamed"));
  } catch (error) {
    setStatusError("status.renameConversationFailed", error);
  }
}

async function toggleConversationPin(conversationId: string) {
  const cid = String(conversationId || "").trim();
  if (!cid) return;
  try {
    const result = await invokeTauri<{ conversationId: string; isPinned: boolean; pinIndex?: number | null }>("toggle_unarchived_conversation_pin", {
      input: {
        conversationId: cid,
      },
    });
    applyConversationPinUpdated({
      conversationId: String(result?.conversationId || cid).trim(),
      isPinned: !!result?.isPinned,
      pinIndex: Number.isFinite(Number(result?.pinIndex)) ? Number(result?.pinIndex) : undefined,
    });
  } catch (error) {
    setStatusError("status.requestFailed", error);
  }
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
  createApiProvider,
  normalizeApiBindingsLocal,
});

const { suppressChatReloadWatch, refreshAllViewData } = useViewRefresh({
  viewMode,
  loadConfig,
  loadBootstrapSnapshot,
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
    config.toolReviewApiConfigId = payload.toolReviewApiConfigId ?? undefined;
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
    if (Array.isArray(payload.instructionPresets)) {
      instructionPresets.value = payload.instructionPresets
        .map((item) => ({
          id: String(item?.id || "").trim(),
          name: String(item?.prompt || item?.name || "").trim(),
          prompt: String(item?.prompt || item?.name || "").trim(),
        }))
        .filter((item) => !!item.id && !!item.prompt);
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
    if ("minRecordSeconds" in payload || "maxRecordSeconds" in payload) {
      const normalizedConfigNumbers = normalizeRuntimeConfigNumbers(
        "minRecordSeconds" in payload ? payload.minRecordSeconds : config.minRecordSeconds,
        "maxRecordSeconds" in payload ? payload.maxRecordSeconds : config.maxRecordSeconds,
        {
          minRecordSeconds: config.minRecordSeconds,
          maxRecordSeconds: config.maxRecordSeconds,
        },
      );
      config.minRecordSeconds = normalizedConfigNumbers.minRecordSeconds;
      config.maxRecordSeconds = normalizedConfigNumbers.maxRecordSeconds;
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
    if ("toolReviewApiConfigId" in payload) {
      config.toolReviewApiConfigId = payload.toolReviewApiConfigId ?? undefined;
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
    if ("apiProviders" in payload) {
      config.apiProviders = Array.isArray(payload.apiProviders)
        ? payload.apiProviders.map((provider) => ({
            ...provider,
            apiKeys: Array.isArray(provider.apiKeys) ? [...provider.apiKeys] : [],
            cachedModelOptions: Array.isArray(provider.cachedModelOptions) ? [...provider.cachedModelOptions] : [],
            models: Array.isArray(provider.models)
              ? provider.models.map((model) => ({ ...model }))
              : [],
            tools: Array.isArray(provider.tools)
              ? provider.tools.map((tool) => ({
                  ...tool,
                  args: Array.isArray(tool.args) ? [...tool.args] : [],
                  values: { ...((tool.values || {}) as Record<string, unknown>) },
                }))
              : [],
          }))
        : [];
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
  },
  onToolReviewReportsUpdated: (payload) => {
    const payloadConversationId = String(payload?.conversationId || "").trim();
    const currentConversationId = String(currentChatConversationId.value || "").trim();
    if (!payloadConversationId || payloadConversationId !== currentConversationId) return;
    toolReviewRefreshTick.value += 1;
  },
  onRecordHotkeyProbe: ({ state, seq }) => {
    if (seq > 0) {
      if (seq <= recordHotkeyProbeLastSeq.value) return;
      recordHotkeyProbeLastSeq.value = seq;
    }
    if (state === "released") {
      recordHotkeyProbeDown.value = false;
    }
    if (viewMode.value !== "chat" || !isPrimaryChatWindow()) return;
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

function clampWebviewZoom(value: number) {
  return Math.min(WEBVIEW_ZOOM_MAX, Math.max(WEBVIEW_ZOOM_MIN, value));
}

async function applyWebviewZoom(nextZoom: number) {
  const normalized = Number(nextZoom);
  if (!Number.isFinite(normalized)) return;
  const clamped = clampWebviewZoom(normalized);
  if (Math.abs(clamped - webviewZoomFactor.value) < 0.001) return;
  await getCurrentWebview().setZoom(clamped);
  webviewZoomFactor.value = clamped;
}

function hasZoomModifier(event: WheelEvent | KeyboardEvent) {
  return !!event.ctrlKey || !!event.metaKey;
}

function handleGlobalZoomWheel(event: WheelEvent) {
  if (!hasZoomModifier(event)) return;
  event.preventDefault();
  const direction = event.deltaY < 0 ? 1 : -1;
  void applyWebviewZoom(webviewZoomFactor.value + direction * WEBVIEW_ZOOM_STEP).catch((error) => {
    console.error("[外观] WebView 缩放失败", error);
  });
}

function handleGlobalZoomKeydown(event: KeyboardEvent) {
  if (!hasZoomModifier(event)) return;
  const key = String(event.key || "").trim();
  if (key === "+" || key === "=") {
    event.preventDefault();
    void applyWebviewZoom(webviewZoomFactor.value + WEBVIEW_ZOOM_STEP).catch((error) => {
      console.error("[外观] WebView 缩放失败", error);
    });
    return;
  }
  if (key === "-" || key === "_") {
    event.preventDefault();
    void applyWebviewZoom(webviewZoomFactor.value - WEBVIEW_ZOOM_STEP).catch((error) => {
      console.error("[外观] WebView 缩放失败", error);
    });
    return;
  }
  if (key === "0") {
    event.preventDefault();
    void applyWebviewZoom(1).catch((error) => {
      console.error("[外观] WebView 缩放失败", error);
    });
  }
}

onMounted(() => {
  try {
    const label = String(getCurrentWindow().label || "").trim();
    tauriWindowLabel.value = label || "unknown";
    detachedChatWindow.value = tauriWindowLabel.value.startsWith("chat-detached-");
    isChatTauriWindow.value = tauriWindowLabel.value === "chat" || detachedChatWindow.value;
  } catch {
    tauriWindowLabel.value = "unknown";
    detachedChatWindow.value = false;
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
      if (matchesForegroundConversation(payloadConversationId)) {
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
      const payloadObject = event.payload && typeof event.payload === "object"
        ? event.payload as Record<string, unknown>
        : null;
      const assistantMessage = (payloadObject?.assistantMessage || null) as ChatMessage | null;
      if (payloadConversationId && payloadConversationId !== currentConversationId) {
      const assistantMessageId = String(assistantMessage?.id || "").trim();
      const cachedMessages = formalizeConversationMessages(conversationMessageCache.value[payloadConversationId] || []);
      const messageAlreadyCached = !!assistantMessageId && cachedMessages.some((message) => String(message?.id || "").trim() === assistantMessageId);
      if (assistantMessage && assistantMessageId && !messageAlreadyCached) {
        cacheConversationMessages(payloadConversationId, [...cachedMessages, assistantMessage]);
      }
      if (!assistantMessageId || !messageAlreadyCached) {
          if (assistantMessage && assistantMessageId) {
            applyConversationOverviewAppendedMessage(payloadConversationId, assistantMessage);
          }
      }
        setConversationBadge(payloadConversationId, "completed");
        void chatFlow.handleExternalRoundCompleted(event.payload);
        return;
      }
      if (!matchesForegroundConversation(payloadConversationId)) return;
      clearConversationBadge(payloadConversationId);
      toolReviewRefreshTick.value += 1;
      updateForegroundConversationOverviewFromMessages(payloadConversationId || currentConversationId, assistantMessage);
      void chatFlow.handleExternalRoundCompleted(event.payload);
    })
      .then((unlisten) => {
        chatRoundCompletedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][轮次完成] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:round-started", (event) => {
      const payloadConversationId = readConversationIdFromPayload(event.payload);
      if (!matchesForegroundConversation(payloadConversationId)) return;
      void chatFlow.handleExternalRoundStarted(event.payload);
    })
      .then((unlisten) => {
        chatRoundStartedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][轮次开始] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:round-failed", (event) => {
      const payloadConversationId = readConversationIdFromPayload(event.payload);
      const currentConversationId = String(currentChatConversationId.value || "").trim();
      if (payloadConversationId && payloadConversationId !== currentConversationId) {
        setConversationBadge(payloadConversationId, "failed");
        void chatFlow.handleExternalRoundFailed(event.payload);
        return;
      }
      if (!matchesForegroundConversation(payloadConversationId)) return;
      clearConversationBadge(payloadConversationId);
      void chatFlow.handleExternalRoundFailed(event.payload);
    })
      .then((unlisten) => {
        chatRoundFailedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][轮次失败] 监听器注册失败", error);
      });
    void listen<ConversationTodosUpdatedPayload>("easy-call:conversation-todos-updated", (event) => {
      applyConversationTodosUpdated(event.payload);
    })
      .then((unlisten) => {
        chatConversationTodosUpdatedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[Todo] 监听器注册失败", error);
      });
    void listen<ConversationPinUpdatedPayload>("easy-call:conversation-pin-updated", (event) => {
      applyConversationPinUpdated(event.payload);
    })
      .then((unlisten) => {
        chatConversationPinUpdatedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[会话置顶] 监听器注册失败", error);
      });
    void listen<ConversationOverviewUpdatedPayload>("easy-call:conversation-overview-updated", (event) => {
      applyConversationOverviewUpdated(event.payload);
    })
      .then((unlisten) => {
        chatConversationOverviewUpdatedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[会话概览] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:assistant-delta", (event) => {
      const conversationId = readConversationIdFromPayload(event.payload);
      if (CHAT_STREAM_DEBUG) {
        console.debug("[聊天流式重绑][前端] 收到助手增量普通事件", {
          conversationId,
          currentConversationId: String(currentChatConversationId.value || "").trim(),
        });
      }
      void chatFlow.handleExternalAssistantDelta(event.payload);
    })
      .then((unlisten) => {
        chatAssistantDeltaUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][助手增量] 监听器注册失败", error);
      });
    void listen<unknown>("easy-call:stream-rebind-required", (event) => {
      const conversationId = readConversationIdFromPayload(event.payload);
      if (CHAT_STREAM_DEBUG) {
        console.debug("[聊天流式重绑][前端] 收到重绑普通事件", {
          conversationId,
          currentConversationId: String(currentChatConversationId.value || "").trim(),
          payload: event.payload,
        });
      }
      void chatFlow.handleExternalStreamRebindRequired(event.payload).catch((error) => {
        console.error("[聊天追踪][流重绑] 处理失败", {
          conversationId,
          error,
        });
      });
    })
      .then((unlisten) => {
        chatStreamRebindRequiredUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][流重绑] 监听器注册失败", error);
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
    void listen<ConversationMessageAppendedPayload>("easy-call:conversation-message-appended", (event) => {
      applyConversationMessageAppended(event.payload);
    })
      .then((unlisten) => {
        chatConversationMessageAppendedUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("[聊天追踪][追加消息] 监听器注册失败", error);
      });

  }
  scheduleChatWindowActiveStateSync("mounted");
  startSupervisionTaskPolling();
  void refreshActiveSupervisionTask({ silent: true });
  window.addEventListener("focus", handleWindowFocusForStateSync);
  window.addEventListener("blur", handleWindowBlurForStateSync);
  document.addEventListener("visibilitychange", handleVisibilityForStateSync);
  window.addEventListener("focus", handleWindowFocusForMicPrewarm);
  document.addEventListener("visibilitychange", handleVisibilityForMicPrewarm);
  window.addEventListener("wheel", handleGlobalZoomWheel, { passive: false });
  window.addEventListener("keydown", handleGlobalZoomKeydown);
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
  if (chatRoundStartedUnlisten) {
    chatRoundStartedUnlisten();
    chatRoundStartedUnlisten = null;
  }
  if (chatRoundFailedUnlisten) {
    chatRoundFailedUnlisten();
    chatRoundFailedUnlisten = null;
  }
  if (chatAssistantDeltaUnlisten) {
    chatAssistantDeltaUnlisten();
    chatAssistantDeltaUnlisten = null;
  }
  if (chatStreamRebindRequiredUnlisten) {
    chatStreamRebindRequiredUnlisten();
    chatStreamRebindRequiredUnlisten = null;
  }
  if (chatConversationMessagesAfterSyncedUnlisten) {
    chatConversationMessagesAfterSyncedUnlisten();
    chatConversationMessagesAfterSyncedUnlisten = null;
  }
  if (chatConversationMessageAppendedUnlisten) {
    chatConversationMessageAppendedUnlisten();
    chatConversationMessageAppendedUnlisten = null;
  }
  if (chatConversationTodosUpdatedUnlisten) {
    chatConversationTodosUpdatedUnlisten();
    chatConversationTodosUpdatedUnlisten = null;
  }
  if (messageStoreMigrationProgressUnlisten) {
    messageStoreMigrationProgressUnlisten();
    messageStoreMigrationProgressUnlisten = null;
  }
  if (chatConversationPinUpdatedUnlisten) {
    chatConversationPinUpdatedUnlisten();
    chatConversationPinUpdatedUnlisten = null;
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
  clearSupervisionTaskPollTimer();
  clearForegroundConversationCacheRaf();
  clearRecordHotkeyProbeState();
  agentWorkPresence.cleanup();
  chatWindowActiveSynced.value = null;
  if (isPrimaryChatWindow()) {
    void invokeTauri("set_chat_window_active", { active: false }).catch(() => {});
  }
  window.removeEventListener("focus", handleWindowFocusForMicPrewarm);
  document.removeEventListener("visibilitychange", handleVisibilityForMicPrewarm);
  window.removeEventListener("wheel", handleGlobalZoomWheel);
  window.removeEventListener("keydown", handleGlobalZoomKeydown);
  cancelPendingRewindConfirm();
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
  () => currentChatConversationId.value,
  () => {
    handleSupervisionConversationChanged();
  },
  { immediate: true },
);

watch(
  () => ({
    apiId: currentForegroundApiConfigId.value,
    imageEnabled: !!currentForegroundApiConfig.value?.enableImage,
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
    apiId: currentForegroundApiConfigId.value,
    agentId: currentForegroundAgentId.value,
    conversationId: currentChatConversationId.value,
    conversationIds: unarchivedConversations.value
      .map((item) => String(item.conversationId || "").trim())
      .join("|"),
  }),
  ({ mode }) => {
    if (mode !== "chat" || !startupDataReady.value) return;
    void refreshChatWorkspaceState();
  },
  { immediate: true },
);

watch(
  () => ({
    mode: viewMode.value,
    workspaceName: chatWorkspaceName.value,
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
    const apiConfigId = String(currentForegroundApiConfigId.value || "").trim();
    const agentId = String(currentForegroundAgentId.value || "").trim();
    const departmentId = String(currentForegroundDepartmentId.value || "").trim();
    if (!apiConfigId || !agentId) return null;
    return { apiConfigId, agentId, departmentId };
  },
  getConversationId: () => String(currentChatConversationId.value || "").trim(),
  chatInput,
  selectedInstructionPrompts,
  selectedMentions: selectedChatMentions,
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
  onOwnUserDraftInserted: () => {
    suppressNextOwnMessageAlignFromHistoryFlushed += 1;
    latestOwnMessageAlignRequest.value += 1;
  },
  t: tr,
  formatRequestFailed: (error) => formatI18nError(tr, "status.requestFailed", error),
  removeBinaryPlaceholders,
  invokeSendChatMessage: ({ text, displayText, images, attachments, extraTextBlocks, mentions, session, onDelta }) =>
    invokeTauri(
      Array.isArray(mentions) && mentions.length > 0
        ? "send_user_mention_message"
        : "send_chat_message",
      {
      input: {
        payload: {
          text,
          displayText,
          images,
          attachments: attachments && attachments.length > 0 ? attachments : undefined,
          extraTextBlocks: extraTextBlocks && extraTextBlocks.length > 0 ? extraTextBlocks : undefined,
          mentions: Array.isArray(mentions) && mentions.length > 0
            ? mentions.map((item) => ({
                agentId: item.agentId,
                agentName: item.agentName,
                departmentId: item.departmentId,
                departmentName: item.departmentName,
              }))
            : undefined,
        },
        session: {
          apiConfigId: session.apiConfigId,
          agentId: session.agentId,
          departmentId: session.departmentId || null,
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
          departmentId: session.departmentId || null,
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
      const fastPathResult = applySingleOwnUserHistoryFlushFastPath(queueMessages);
      if (fastPathResult) {
        if (suppressNextOwnMessageAlignFromHistoryFlushed > 0) {
          suppressNextOwnMessageAlignFromHistoryFlushed -= 1;
        } else {
          latestOwnMessageAlignRequest.value += 1;
        }
        console.warn("[聊天追踪][历史刷写处理] 单条用户消息快路径完成", {
          windowLabel: tauriWindowLabel.value,
          activateAssistant,
          messageId: fastPathResult.messageId,
          finalMessageCount: allMessages.value.length,
        });
        cacheConversationMessages(
          flushedConversationId || String(currentChatConversationId.value || "").trim(),
          allMessages.value,
        );
        console.warn("[聊天追踪][历史刷写处理] 完成", {
          windowLabel: tauriWindowLabel.value,
          flushedConversationId: String(currentChatConversationId.value || "").trim(),
          finalMessageCount: allMessages.value.length,
        });
        return;
      }

      const currentMessages = [...allMessages.value];
      const dedup = new Set(
        currentMessages
          .filter((message) => !isOptimisticOwnUserDraft(message))
          .map((message) => String(message.id || "").trim())
          .filter((id) => !!id),
      );
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
      const appendedOwnUser = appended.filter((message) => isLocalOwnUserMessage(message));
      const appendedOthers = appended.filter((message) => !isLocalOwnUserMessage(message));

      let nextMessages = [...currentMessages];
      if (prepended.length > 0) {
        nextMessages = [...prepended, ...nextMessages];
      }

      if (appendedOwnUser.length > 0) {
        let replacedOwnDraft = false;
        const remainingOwnIncoming = [...appendedOwnUser];
        nextMessages = nextMessages.flatMap((message) => {
          if (!replacedOwnDraft && isOptimisticOwnUserDraft(message)) {
            replacedOwnDraft = true;
            return [remainingOwnIncoming.shift()!];
          }
          return [message];
        });
        if (remainingOwnIncoming.length > 0) {
          nextMessages = insertMessagesBeforeAssistantDraft(nextMessages, remainingOwnIncoming);
        }
      }

      if (appendedOthers.length > 0) {
        nextMessages = insertMessagesBeforeAssistantDraft(nextMessages, appendedOthers);
      }

      nextMessages = reuseStableMessageReferences(nextMessages, allMessages.value);
      allMessages.value = nextMessages;
      foregroundTailLatestReady.value = true;
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
      const appendedOwnUserMessage = appended.some((message) => isLocalOwnUserMessage(message));
      if (appendedOwnUserMessage) {
        if (suppressNextOwnMessageAlignFromHistoryFlushed > 0) {
          suppressNextOwnMessageAlignFromHistoryFlushed -= 1;
        } else {
          latestOwnMessageAlignRequest.value += 1;
        }
      }
      console.warn("[聊天追踪][历史刷写处理] 合并完成", {
        windowLabel: tauriWindowLabel.value,
        activateAssistant,
        beforeDedupCount,
        prependedCount: prepended.length,
        appendedCount: appended.length,
        droppedAsDuplicate: beforeDedupCount - uniqueIncoming.length,
        previousMessageCount: currentMessages.length,
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

const { handleConfirmPlan } = useConfirmPlan({
  currentApiConfigId: currentForegroundApiConfigId,
  currentAgentId: currentForegroundAgentId,
  currentDepartmentId: currentForegroundDepartmentId,
  currentConversationId: currentChatConversationId,
  chatting,
  forcingArchive,
  compactingConversation,
  setConversationPlanMode,
  clearForegroundRuntimeState: chatFlow.clearForegroundRuntimeState,
  confirmPlanAndContinue: ({ conversationId, planMessageId, departmentId, agentId }) => invokeTauri<void>("confirm_plan_and_continue", {
    input: {
      conversationId,
      planMessageId,
      departmentId: departmentId || null,
      agentId: agentId || null,
    },
  }),
});

watch(
  () => ({
    mode: viewMode.value,
    departmentId: String(currentForegroundDepartmentId.value || "").trim(),
    agentId: String(currentForegroundAgentId.value || "").trim(),
  }),
  ({ mode }) => {
    if (mode !== "chat" || !startupDataReady.value) return;
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
    if (mode !== "chat" || !startupDataReady.value) return;
    console.warn("[聊天追踪][流绑定] 准备绑定", {
      windowLabel: tauriWindowLabel.value,
      conversationId,
    });
    void (async () => {
      try {
        await chatFlow.bindActiveConversationStream(conversationId);
        maybeResumeForegroundStreamingDraft(conversationId, "bind_active_stream");
      } catch (error) {
        console.warn("[聊天推送] 绑定前台流失败", {
          conversationId,
          error,
        });
      }
    })();
  },
  { immediate: true },
);

watch(
  () => String(currentChatConversationId.value || "").trim(),
  () => {
    selectedChatMentions.value = [];
  },
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
  activeApiConfigId: currentForegroundApiConfigId,
  activeAgentId: currentForegroundAgentId,
  currentConversationId: currentChatConversationId,
  allMessages,
  maybeUpdateConversationOverviewFromLoadedMessages: maybeUpdateForegroundConversationOverviewFromLoadedMessages,
  chatting,
  forcingArchive,
  compactingConversation,
  chatInput,
  selectedMentions: selectedChatMentions,
  clipboardImages,
  deleteUnarchivedConversationFromArchives,
  sendChat: sendChatFromCurrentWindow,
  stopChat: chatFlow.stopChat,
  clearForegroundRuntimeState: chatFlow.clearForegroundRuntimeState,
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

const { openCurrentHistory, openConversationSummary, openPromptPreview, openSystemPromptPreview } = useChatDialogActions({
  activeChatApiConfigId: currentForegroundApiConfigId,
  assistantDepartmentAgentId: currentForegroundAgentId,
  openPromptPreviewDialog,
  openSystemPromptPreviewDialog,
});

function setMemoryDialogRef(el: Element | null) {
  memoryDialog.value = (el as HTMLDialogElement | null) ?? null;
}

function setPromptPreviewDialogRef(el: Element | null) {
  promptPreviewDialog.value = (el as HTMLDialogElement | null) ?? null;
}

function resetMessageStoreMigrationGate() {
  messageStoreMigration.visible = false;
  messageStoreMigration.mode = "idle";
  messageStoreMigration.message = "";
  messageStoreMigration.current = 0;
  messageStoreMigration.total = 0;
  messageStoreMigration.blockedItems = [];
}

async function ensureMessageStoreMigrationProgressListener() {
  if (messageStoreMigrationProgressUnlisten) return;
  messageStoreMigrationProgressUnlisten = await listen<MessageStoreMigrationProgressPayload>(
    "easy-call:message-store-migration-progress",
    (event) => {
      const payload = event.payload;
      messageStoreMigration.visible = true;
      messageStoreMigration.mode = payload.status === "failed" ? "error" : "migrating";
      messageStoreMigration.current = Number(payload.current || 0);
      messageStoreMigration.total = Number(payload.total || 0);
      const title = String(payload.title || payload.conversationId || "").trim();
      const detail = String(payload.detail || "").trim();
      messageStoreMigration.message = detail || `正在迁移：${title || "会话"}`;
    },
  );
}

async function runMessageStoreMigrationFromGate(discardInvalid: boolean) {
  await ensureMessageStoreMigrationProgressListener();
  messageStoreMigration.visible = true;
  messageStoreMigration.mode = "migrating";
  messageStoreMigration.message = discardInvalid
    ? "正在备份异常会话并继续迁移..."
    : "正在迁移会话消息仓库...";
  await invokeTauri("run_message_store_migration", {
    input: { discardInvalid },
  });
  resetMessageStoreMigrationGate();
}

async function ensureMessageStoreMigrationGate() {
  await ensureMessageStoreMigrationProgressListener();
  messageStoreMigration.visible = true;
  messageStoreMigration.mode = "checking";
  messageStoreMigration.message = "正在检查会话消息仓库...";
  const report = await invokeTauri<MessageStoreMigrationPreflightReport>(
    "check_message_store_migration",
  );
  if (report.blockedCount > 0) {
    messageStoreMigration.mode = "blocked";
    messageStoreMigration.blockedItems = report.items.filter((item) => item.status === "blocked");
    messageStoreMigration.message = `发现 ${report.blockedCount} 个异常会话。需要确认是否抛弃异常会话并继续迁移。`;
    return await new Promise<void>((resolve, reject) => {
      messageStoreMigrationResolve = resolve;
      messageStoreMigrationReject = reject;
    });
  }
  if (report.legacyCount > 0) {
    messageStoreMigration.message = `发现 ${report.legacyCount} 个旧会话，正在迁移...`;
    await runMessageStoreMigrationFromGate(false);
    return;
  }
  resetMessageStoreMigrationGate();
}

function cancelMessageStoreMigration() {
  const error = new Error("用户取消会话消息仓库迁移，启动已暂停。");
  messageStoreMigration.mode = "error";
  messageStoreMigration.message = error.message;
  messageStoreMigrationReject?.(error);
  messageStoreMigrationResolve = null;
  messageStoreMigrationReject = null;
}

async function continueMessageStoreMigrationWithDiscard() {
  try {
    await runMessageStoreMigrationFromGate(true);
    messageStoreMigrationResolve?.();
  } catch (error) {
    messageStoreMigration.mode = "error";
    messageStoreMigration.message = formatI18nError(tr, "status.requestFailed", error);
    messageStoreMigrationReject?.(error instanceof Error ? error : new Error(String(error)));
  } finally {
    messageStoreMigrationResolve = null;
    messageStoreMigrationReject = null;
  }
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
  beforeRefreshData: ensureMessageStoreMigrationGate,
  refreshAllViewData,
  afterRefreshData: () => {
    startupDataReady.value = true;
  },
  viewMode,
  syncWindowControlsState,
  stopRecording,
  cleanupSpeechRecording,
  cleanupChatMedia,
  afterMountedReady: async () => {
    await initializeDetachedChatWindow();
    void invokeTauri<boolean>("frontend_ready_start_remote_im_services")
      .then((started) => {
        console.info("[启动] 前端 mounted ready 已通知后端启动远程 IM 服务", { started });
      })
      .catch((error) => {
        console.warn("[启动] 通知后端启动远程 IM 服务失败", error);
      });
    await autoCheckGithubUpdate();
  },
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
  selectedApiConfig,
  toolApiConfig,
  modelRefreshError,
  toolStatuses,
  defaultApiTools,
  t: tr,
  normalizeApiBindingsLocal,
  syncUserAliasFromPersona,
  syncTrayIcon,
  refreshToolsStatus,
  refreshImageCacheStats,
});
</script>
