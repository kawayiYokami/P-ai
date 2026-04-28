<template>
  <div
    class="window-content"
    :class="viewMode === 'chat'
      ? 'flex flex-col min-h-0 overflow-hidden'
      : viewMode === 'config'
        ? 'p-0 min-h-0 overflow-hidden'
        : 'p-0 min-h-0 overflow-hidden'"
  >
    <ConfigView
      v-if="viewMode === 'config'"
      :config="config"
      :config-tab="configTab"
      :ui-language="config.uiLanguage"
      :locale-options="localeOptions"
      :current-theme="currentTheme"
      :generated-theme-controls="generatedThemeControls"
      :generated-theme-tokens="generatedThemeTokens"
      :selected-api-config="selectedApiConfig"
      :tool-api-config="toolApiConfig"
      :base-url-reference="baseUrlReference"
      :refreshing-models="refreshingModels"
      :model-options="selectedModelOptions"
      :model-refresh-ok="modelRefreshOk"
      :model-refresh-error="modelRefreshError"
      :tool-statuses="toolStatuses"
      :personas="personas"
      :assistant-personas="assistantPersonas"
      :user-persona="userPersona"
      :persona-editor-id="personaEditorId"
      :assistant-department-agent-id="assistantDepartmentAgentId"
      :selected-persona="selectedPersonaEditor"
      :tool-persona="selectedPersonaEditor"
      :selected-persona-avatar-url="selectedPersonaEditorAvatarUrl"
      :user-persona-avatar-url="userPersonaAvatarUrl"
      :response-style-options="responseStyleOptions"
      :response-style-id="selectedResponseStyleId"
      :pdf-read-mode="selectedPdfReadMode"
      :background-voice-screenshot-keywords="backgroundVoiceScreenshotKeywords"
      :background-voice-screenshot-mode="backgroundVoiceScreenshotMode"
      :instruction-presets="instructionPresets"
      :text-capable-api-configs="textCapableApiConfigs"
      :image-capable-api-configs="imageCapableApiConfigs"
      :stt-capable-api-configs="sttCapableApiConfigs"
      :cache-stats="imageCacheStats"
      :cache-stats-loading="imageCacheStatsLoading"
      :avatar-saving="avatarSaving"
      :avatar-error="avatarError"
      :persona-saving="personaSaving"
      :persona-dirty="personaDirty"
      :config-dirty="configDirty"
      :saving-config="saving"
      :normalize-api-bindings-action="normalizeApiBindingsAction"
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio-ready="!!hotkeyTestAudio"
      :checking-update="checkingUpdate"
      :has-available-update="hasAvailableUpdate"
      :save-config-action="saveConfig"
      :restore-config-action="restoreConfig"
      :last-saved-config-json="lastSavedConfigJson"
      :set-status-action="setStatus"
      @update:config-tab="updateConfigTab"
      @update:ui-language="setUiLanguage"
      @update:persona-editor-id="updatePersonaEditorId"
      @update:assistant-department-agent-id="updateSelectedPersonaId"
      @update:response-style-id="updateSelectedResponseStyleId"
      @update:pdf-read-mode="updateSelectedPdfReadMode"
      @update:background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      @update:background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      @update:instruction-presets="updateInstructionPresets"
      @patch-conversation-api-settings="patchConversationApiSettings"
      @patch-chat-settings="patchChatSettings"
      @set-theme="setTheme"
      @activate-generated-theme="activateGeneratedTheme"
      @update-generated-theme-controls="updateGeneratedThemeControls"
      @reset-generated-theme="resetGeneratedTheme"
      @refresh-models="refreshModels"
      @tool-switch-changed="onToolsChanged"
      @save-api-config="saveConfig"
      @add-api-config="addApiConfig"
      @remove-selected-api-config="removeSelectedApiConfig"
      @add-persona="addPersona"
      @remove-selected-persona="removeSelectedPersona"
      @save-personas="savePersonas"
      @import-persona-memories="importPersonaMemories"
      @open-current-history="openCurrentHistory"
      @open-prompt-preview="openPromptPreview"
      @open-system-prompt-preview="openSystemPromptPreview"
      @open-memory-viewer="openMemoryViewer"
      @refresh-image-cache-stats="refreshImageCacheStats"
      @clear-image-cache="clearImageCache"
      @open-runtime-logs="openRuntimeLogs"
      @start-hotkey-record-test="startHotkeyRecordTest"
      @stop-hotkey-record-test="stopHotkeyRecordTest"
      @play-hotkey-record-test="playHotkeyRecordTest"
      @capture-hotkey="captureHotkey"
      @summon-chat-now="summonChatNow"
      @save-agent-avatar="saveAgentAvatar"
      @clear-agent-avatar="clearAgentAvatar"
      @check-update="checkUpdate"
      @open-github="openGithub"
    />

    <div v-else-if="viewMode === 'chat'" class="relative flex-1 min-h-0">
      <ChatView
        :user-alias="userAlias"
        :persona-name="selectedPersonaName"
        :user-avatar-url="userAvatarUrl"
        :assistant-avatar-url="selectedPersonaAvatarUrl"
        :persona-name-map="chatPersonaNameMap"
        :persona-avatar-url-map="chatPersonaAvatarUrlMap"
        :persona-presence-chips="chatPersonaPresenceChips"
        :mention-options="chatMentionOptions"
        :selected-mentions="selectedChatMentions"
        :latest-user-text="latestUserText"
        :latest-user-images="latestUserImages"
        :latest-assistant-text="latestAssistantText"
        :latest-reasoning-standard-text="latestReasoningStandardText"
        :latest-reasoning-inline-text="latestReasoningInlineText"
        :frontend-round-phase="frontendRoundPhase"
        :tool-status-text="toolStatusText"
        :tool-status-state="toolStatusState"
        :stream-tool-calls="streamToolCalls"
        :chat-error-text="chatErrorText"
        :clipboard-images="clipboardImages"
        :queued-attachment-notices="queuedAttachmentNotices"
        :chat-input="chatInput"
        :instruction-presets="instructionPresets"
        :chat-input-placeholder="chatInputPlaceholder"
        :can-record="speechRecognitionSupported"
        :recording="recording"
        :recording-ms="recordingMs"
        :transcribing="transcribing"
        :record-hotkey="recordHotkey"
        :selected-chat-model-id="selectedChatModelId"
        :tool-review-refresh-tick="toolReviewRefreshTick"
        :terminal-approvals="terminalApprovals"
        :terminal-approval-resolving="terminalApprovalResolving"
        :chat-model-options="textCapableApiConfigs"
        :plan-mode-enabled="planModeEnabled"
        :chat-usage-percent="chatUsagePercent"
        :force-archive-tip="forceArchiveTip"
        :media-drag-active="mediaDragActive"
        :chatting="chatting"
        :forcing-archive="forcingArchive"
        :compacting-conversation="compactingConversation"
        :conversation-busy="forcingArchive || compactingConversation"
        :frozen="branchingConversation || forwardingConversationSelection"
        :message-blocks="visibleMessageBlocks"
        :has-more-history="chatHasMoreHistory"
        :loading-older-history="chatLoadingOlderHistory"
        :latest-own-message-align-request="latestOwnMessageAlignRequest"
        :conversation-scroll-to-bottom-request="conversationScrollToBottomRequest"
        :current-workspace-name="currentChatWorkspaceName"
        :current-workspace-root-path="currentChatWorkspaceRootPath"
        :workspaces="currentChatWorkspaces"
        :current-department-id="currentChatDepartmentId"
        :active-conversation-id="currentChatConversationId"
        :current-todos="currentChatTodos"
        :supervision-active="chatSupervisionActive"
        :supervision-title="chatSupervisionTitle"
        :supervision-dialog-open="supervisionTaskDialogOpen"
        :supervision-task-saving="supervisionTaskSaving"
        :supervision-task-error="supervisionTaskError"
        :active-supervision-task="activeSupervisionTask"
        :recent-supervision-task-history="recentSupervisionTaskHistory"
        :unarchived-conversation-items="chatUnarchivedConversationItems"
        :conversation-items="chatConversationItems || chatUnarchivedConversationItems"
        :create-conversation-department-options="createConversationDepartmentOptions"
        :default-create-conversation-department-id="defaultCreateConversationDepartmentId"
        :current-theme="currentTheme"
        :detached-chat-window="detachedChatWindow"
        @update:chat-input="updateChatInput"
        @update:selected-instruction-prompts="updateSelectedInstructionPrompts"
        @add-mention="addChatMention"
        @remove-mention="removeChatMention"
        @side-conversation-list-visible-change="setSideConversationListVisible"
        @remove-clipboard-image="removeClipboardImage"
        @remove-queued-attachment-notice="removeQueuedAttachmentNotice"
        @pick-attachments="pickAttachments"
        @update:selected-chat-model-id="updateSelectedChatModelId"
        @update:plan-mode-enabled="updatePlanModeEnabled"
        @start-recording="startRecording"
        @stop-recording="stopRecording"
        @send-chat="sendChat"
        @stop-chat="stopChat"
        @load-older-history="onLoadOlderChatHistory"
        @reached-bottom="onReachedChatBottom"
        @jump-to-conversation-bottom="onJumpToConversationBottom"
        @recall-turn="onRecallTurn"
        @regenerate-turn="onRegenerateTurn"
        @confirm-plan="confirmPlan"
        @force-archive="openForceArchiveActionDialog"
        @selection-action-copy="setStatus(`已复制 ${$event.count} 条消息`)"
        @selection-action-copy-error="setStatus(props.t('chat.copyFailed'))"
        @selection-action-branch="onBranchConversationFromSelection($event)"
        @selection-action-forward="onForwardConversationFromSelection($event)"
        @selection-action-share="openSelectionShareDialog($event)"
        @attach-tool-review-report="attachToolReviewReport"
        @lock-workspace="onLockChatWorkspace"
        @open-supervision-task="openSupervisionTaskDialog"
        @detach-conversation="handleDetachConversation"
        @close-supervision-task="closeSupervisionTaskDialog"
        @save-supervision-task="saveSupervisionTask"
        @refresh-tool-review-message="onRefreshToolReviewMessage"
        @switch-conversation="onSwitchConversation"
        @rename-conversation="onRenameConversation"
        @toggle-pin-conversation="onToggleConversationPin"
        @create-conversation="onCreateConversation"
        @approve-terminal-approval="approveTerminalApproval"
        @deny-terminal-approval="denyTerminalApproval"
      />
      <div
        v-if="chatBusyOverlay"
        class="absolute inset-0 z-20 flex items-center justify-center bg-base-100/60 backdrop-blur-[1px]"
      >
        <div class="rounded-box border border-base-300 bg-base-100 px-4 py-3 shadow-sm flex flex-col items-center gap-1">
          <span class="loading loading-spinner loading-sm"></span>
          <div class="text-sm">{{ chatBusyOverlay.title }}</div>
          <div class="text-sm opacity-70">{{ chatBusyOverlay.detail }}</div>
        </div>
      </div>
    </div>

    <ArchivesView
      v-else
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
      :user-alias="userAlias"
      :persona-name-map="chatPersonaNameMap"
      @load-archives="loadArchives"
      @select-archive="selectArchive"
      @select-archive-block="selectArchiveBlock"
      @select-unarchived-conversation="selectUnarchivedConversation"
      @select-unarchived-block="selectUnarchivedConversationBlock"
      @select-delegate-conversation="selectDelegateConversation"
      @select-remote-im-contact-conversation="selectRemoteImContactConversation"
      @select-remote-im-contact-block="selectRemoteImContactConversationBlock"
      @export-archive="exportArchive"
      @import-archive-file="importArchiveFile"
      @delete-archive="deleteArchive"
      @delete-unarchived-conversation="deleteUnarchivedConversation"
      @delete-remote-im-contact-conversation="deleteRemoteImContactConversation"
    />
    <dialog :ref="memoryDialogVNodeRef" class="modal">
      <MemoryDialog
        :title="t('memory.title')"
        :empty-text="t('memory.empty')"
        :page-text="t('memory.page', { page: memoryPage, total: memoryPageCount })"
        :prev-page-text="t('memory.prevPage')"
        :next-page-text="t('memory.nextPage')"
        :export-text="t('memory.export')"
        :import-text="t('memory.import')"
        :close-text="t('common.close')"
        :memory-list="memoryList"
        :paged-memories="pagedMemories"
        :memory-page="memoryPage"
        :memory-page-count="memoryPageCount"
        @close="closeMemoryViewer"
        @prev-page="prevMemoryPage"
        @next-page="nextMemoryPage"
        @export-memories="exportMemories"
        @trigger-import="triggerMemoryImport"
        @import-file="handleMemoryImportFile"
      />
    </dialog>
    <dialog :ref="promptPreviewDialogVNodeRef" class="modal">
      <PromptPreviewDialog
        :mode="promptPreviewMode"
        :loading="promptPreviewLoading"
        :title="promptPreviewMode === 'system' ? t('prompt.systemPreview') : t('prompt.requestPreview')"
        :loading-text="t('common.loading')"
        :empty-hint="'请选择一个预览模式\n对话：正常聊天请求体\n压缩：SummaryContext 压缩请求体\n归档：SummaryContext 归档请求体'"
        :chat-text="'对话'"
        :compaction-text="'压缩'"
        :archive-text="'归档'"
        :latest-input-length-text="t('prompt.latestInputLength')"
        :images-text="t('prompt.images')"
        :audios-text="t('prompt.audios')"
        :close-text="t('common.close')"
        :latest-user-text="promptPreviewLatestUserText"
        :latest-images="promptPreviewLatestImages"
        :latest-audios="promptPreviewLatestAudios"
        :text="promptPreviewText"
        @select-mode="loadPromptPreview"
        @close="closePromptPreview"
      />
    </dialog>
    <SelectionShareDialog
      :open="selectionShareDialogOpen"
      :loading="selectionShareDialogLoading"
      :title-text="t('chat.shareDialogTitle')"
      :message-text="t('chat.shareDialogMessage', { count: selectionSharePayload?.count || 0 })"
      :hint-text="selectionShareDialogLoading ? t('common.loading') : t('chat.shareDialogHint')"
      :image-text="t('chat.shareAsImage')"
      :html-text="t('chat.shareAsHtml')"
      :cancel-text="t('common.cancel')"
      @close="closeSelectionShareDialog"
      @export-image="exportSelectionAsImage"
      @export-html="exportSelectionAsHtml"
    />
  </div>
</template>

<script setup lang="ts">
import ConfigView from "../../config/views/ConfigView.vue";
import ChatView from "../../chat/views/ChatView.vue";
import type { TerminalApprovalConversationItem } from "../composables/use-terminal-approval";
import ArchivesView from "../../archive/views/ArchivesView.vue";
import MemoryDialog from "../../memory/components/dialogs/MemoryDialog.vue";
import PromptPreviewDialog from "../../chat/components/dialogs/PromptPreviewDialog.vue";
import SelectionShareDialog from "../../chat/components/dialogs/SelectionShareDialog.vue";
import { computed, ref, type VNodeRef } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatConversationOverviewItem,
  ChatMentionTarget,
  ChatMessage,
  ChatMessageBlock,
  ChatTodoItem,
  ChatPersonaPresenceChip,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  ImageTextCacheStats,
  PersonaProfile,
  PromptCommandPreset,
  ResponseStyleOption,
  ShellWorkspace,
  ToolLoadStatus,
  UnarchivedConversationSummary,
} from "../../../types/app";
import type { GeneratedThemeControls, GeneratedThemeTokens } from "../../shell/theme/theme-types";
import {
  buildShareExportFileName,
  buildShareHtmlDocument,
  prepareShareEntries,
  renderShareDocumentToPngDataUrl,
} from "../../chat/utils/share-export";
import { invokeTauri } from "../../../services/tauri-api";

type MemoryItem = {
  id: string;
  memoryType: "knowledge" | "skill" | "emotion" | "event";
  judgment: string;
  reasoning: string;
  tags: string[];
  ownerAgentId?: string;
};

type SelectionSharePayload = {
  count: number;
  messageIds: string[];
  blocks: ChatMessageBlock[];
};

const props = defineProps<{
  t: (key: string, params?: Record<string, unknown>) => string;
  viewMode: "chat" | "archives" | "config";
  detachedChatWindow?: boolean;
  config: AppConfig;
  configTab: "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
  generatedThemeControls: GeneratedThemeControls;
  generatedThemeTokens: GeneratedThemeTokens;
  selectedApiConfig: ApiConfigItem | null;
  toolApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  selectedModelOptions: string[];
  modelRefreshOk: boolean;
  modelRefreshError: string;
  toolStatuses: ToolLoadStatus[];
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  userPersona: PersonaProfile | null;
  personaEditorId: string;
  assistantDepartmentAgentId: string;
  selectedPersonaEditor: PersonaProfile | null;
  toolPersona: PersonaProfile | null;
  selectedPersonaEditorAvatarUrl: string;
  userPersonaAvatarUrl: string;
  responseStyleOptions: ResponseStyleOption[];
  selectedResponseStyleId: string;
  selectedPdfReadMode: "text" | "image";
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  instructionPresets: PromptCommandPreset[];
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  imageCacheStats: ImageTextCacheStats;
  imageCacheStatsLoading: boolean;
  avatarSaving: boolean;
  avatarError: string;
  personaSaving: boolean;
  personaDirty: boolean;
  configDirty: boolean;
  saving: boolean;
  normalizeApiBindingsAction: () => void;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudio: unknown;
  checkingUpdate: boolean;
  hasAvailableUpdate: boolean;
  setStatus: (text: string) => void;
  attachToolReviewReport: (reportText: string) => void;
  userAlias: string;
  selectedPersonaName: string;
  userAvatarUrl: string;
  selectedPersonaAvatarUrl: string;
  chatPersonaNameMap: Record<string, string>;
  chatPersonaAvatarUrlMap: Record<string, string>;
  chatPersonaPresenceChips: ChatPersonaPresenceChip[];
  chatMentionOptions: ChatMentionTarget[];
  selectedChatMentions: ChatMentionTarget[];
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  frontendRoundPhase: "idle" | "queued" | "waiting" | "streaming";
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  queuedAttachmentNotices: Array<{ id: string; fileName: string; relativePath: string; mime: string }>;
  chatInput: string;
  chatInputPlaceholder: string;
  speechRecognitionSupported: boolean;
  recording: boolean;
  recordingMs: number;
  transcribing: boolean;
  recordHotkey: string;
  selectedChatModelId: string;
  toolReviewRefreshTick: number;
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
  approveTerminalApproval: (requestId?: string) => void;
  denyTerminalApproval: (requestId?: string) => void;
  planModeEnabled: boolean;
  chatUsagePercent: number;
  forceArchiveTip: string;
  mediaDragActive: boolean;
  chatting: boolean;
  forcingArchive: boolean;
  compactingConversation: boolean;
  branchingConversation: boolean;
  forwardingConversationSelection: boolean;
  visibleMessageBlocks: ChatMessageBlock[];
  chatHasMoreHistory: boolean;
  chatLoadingOlderHistory: boolean;
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  currentChatWorkspaceName: string;
  currentChatWorkspaceRootPath: string;
  currentChatWorkspaces: ShellWorkspace[];
  currentChatDepartmentId: string;
  currentChatConversationId: string;
  currentChatTodos: ChatTodoItem[];
  chatSupervisionActive: boolean;
  chatSupervisionTitle: string;
  supervisionTaskDialogOpen: boolean;
  supervisionTaskSaving: boolean;
  supervisionTaskError: string;
  activeSupervisionTask: {
    taskId: string;
    goal: string;
    why: string;
    todo: string;
    endAtLocal: string;
    remainingHours: number;
  } | null;
  recentSupervisionTaskHistory: Array<{
    goal: string;
    why: string;
    todo: string;
    durationHours: number;
  }>;
  chatUnarchivedConversationItems: ChatConversationOverviewItem[];
  chatConversationItems?: ChatConversationOverviewItem[];
  createConversationDepartmentOptions: Array<{ id: string; name: string; ownerName: string; providerName?: string; modelName?: string }>;
  defaultCreateConversationDepartmentId: string;
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedArchiveBlockId?: number | null;
  archiveHasPrevBlock?: boolean;
  archiveHasNextBlock?: boolean;
  archiveMessages: ChatMessage[];
  archiveSummaryText: string;
  unarchivedConversations: UnarchivedConversationSummary[];
  unarchivedBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedUnarchivedConversationId: string;
  selectedUnarchivedBlockId?: number | null;
  unarchivedHasPrevBlock?: boolean;
  unarchivedHasNextBlock?: boolean;
  unarchivedMessages: ChatMessage[];
  delegateConversations: DelegateConversationSummary[];
  selectedDelegateConversationId: string;
  delegateMessages: ChatMessage[];
  remoteImContactConversations: RemoteImContactConversationSummary[];
  remoteImContactBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedRemoteImContactId: string;
  selectedRemoteImContactBlockId?: number | null;
  remoteImHasPrevBlock?: boolean;
  remoteImHasNextBlock?: boolean;
  remoteImContactMessages: ChatMessage[];
  messageText: (message: ChatMessage) => string;
  extractMessageImages: (message?: ChatMessage) => Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  memoryList: MemoryItem[];
  memoryPage: number;
  memoryPageCount: number;
  pagedMemories: MemoryItem[];
  promptPreviewMode: "chat" | "compaction" | "archive" | "system" | null;
  promptPreviewLoading: boolean;
  promptPreviewText: string;
  promptPreviewLatestUserText: string;
  promptPreviewLatestImages: number;
  promptPreviewLatestAudios: number;
  loadPromptPreview: (mode: "chat" | "compaction" | "archive") => void;
  setMemoryDialogRef: (el: Element | null) => void;
  setPromptPreviewDialogRef: (el: Element | null) => void;
  updateConfigTab: (value: "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about") => void;
  setUiLanguage: (value: string) => void;
  updatePersonaEditorId: (value: string) => void;
  updateSelectedPersonaId: (value: string) => void;
  updateSelectedResponseStyleId: (value: string) => void;
  updateSelectedPdfReadMode: (value: "text" | "image") => void;
  updateBackgroundVoiceScreenshotKeywords: (value: string) => void;
  updateBackgroundVoiceScreenshotMode: (value: "desktop" | "focused_window") => void;
  updateInstructionPresets: (value: PromptCommandPreset[]) => void;
  patchConversationApiSettings: (value: import("../../../types/app").ConversationApiSettingsPatch) => void;
  patchChatSettings: (value: import("../../../types/app").ChatSettingsPatch) => void;
  setTheme: (value: string) => void;
  activateGeneratedTheme: () => void;
  updateGeneratedThemeControls: (patch: Partial<GeneratedThemeControls>) => void;
  resetGeneratedTheme: () => void;
  refreshModels: () => void;
  saveConfig: () => Promise<boolean> | boolean;
  restoreConfig: () => boolean;
  lastSavedConfigJson: string;
  onToolsChanged: () => void;
  addApiConfig: () => void;
  removeSelectedApiConfig: () => void;
  addPersona: () => void;
  removeSelectedPersona: () => void;
  savePersonas: () => Promise<boolean> | boolean;
  importPersonaMemories: (payload: { agentId: string; file: File }) => void;
  openCurrentHistory: () => void;
  openConversationSummary: (conversationId: string) => void;
  openForceArchiveActionDialog: () => void;
  openPromptPreview: () => void;
  openSystemPromptPreview: () => void;
  openMemoryViewer: () => void;
  refreshImageCacheStats: () => void;
  clearImageCache: () => void;
  openRuntimeLogs: () => void;
  startHotkeyRecordTest: () => void;
  stopHotkeyRecordTest: () => void;
  playHotkeyRecordTest: () => void;
  captureHotkey: (value: string) => void;
  summonChatNow: () => void;
  saveAgentAvatar: (input: { agentId: string; mime: string; bytesBase64: string }) => void;
  clearAgentAvatar: (input: { agentId: string }) => void;
  updateChatInput: (value: string) => void;
  updateSelectedInstructionPrompts: (value: PromptCommandPreset[]) => void;
  addChatMention: (value: ChatMentionTarget) => void;
  removeChatMention: (agentId: string) => void;
  updateSelectedChatModelId: (value: string) => void;
  updatePlanModeEnabled: (value: boolean) => void;
  setSideConversationListVisible: (value: boolean) => void;
  removeClipboardImage: (index: number) => void;
  removeQueuedAttachmentNotice: (index: number) => void;
  pickAttachments: () => void;
  startRecording: () => void;
  stopRecording: () => void;
  sendChat: () => void;
  stopChat: () => void;
  onLoadOlderChatHistory: () => void;
  onReachedChatBottom: () => void;
  onJumpToConversationBottom: () => void;
  onRecallTurn: (payload: { turnId: string }) => void;
  onRegenerateTurn: (payload: { turnId: string }) => void;
  confirmPlan: (payload: { messageId: string }) => void;
  onLockChatWorkspace: () => void;
  openSupervisionTaskDialog: () => void;
  onDetachConversation: () => void;
  closeSupervisionTaskDialog: () => void;
  saveSupervisionTask: (payload: { durationHours: number; goal: string; why: string; todo: string }) => void;
  onRefreshToolReviewMessage: (payload: { conversationId: string; messageId: string }) => void;
  onSwitchConversation: (payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }) => void;
  onRenameConversation: (payload: { conversationId: string; title: string }) => void;
  onToggleConversationPin: (conversationId: string) => void;
  onCreateConversation: (input?: { title?: string; departmentId?: string }) => void;
  onBranchConversationFromSelection: (payload: { count: number; messageIds: string[] }) => void;
  onForwardConversationFromSelection: (payload: { count: number; messageIds: string[]; targetConversationId: string }) => void;
  loadArchives: () => void;
  selectArchive: (id: string) => void;
  selectArchiveBlock: (blockId?: number | null) => void;
  selectUnarchivedConversation: (id: string) => void;
  selectUnarchivedConversationBlock: (blockId?: number | null) => void;
  selectDelegateConversation: (id: string) => void;
  selectRemoteImContactConversation: (id: string) => void;
  selectRemoteImContactConversationBlock: (blockId?: number | null) => void;
  exportArchive: (payload: { format: "markdown" | "json" }) => void;
  importArchiveFile: (file: File) => void;
  deleteArchive: (id: string) => void;
  deleteUnarchivedConversation: (id: string) => void;
  deleteRemoteImContactConversation: (id: string) => void;
  closeMemoryViewer: () => void;
  prevMemoryPage: () => void;
  nextMemoryPage: () => void;
  exportMemories: () => void;
  triggerMemoryImport: () => void;
  handleMemoryImportFile: (event: Event) => void;
  closePromptPreview: () => void;
  checkUpdate: () => void;
  openGithub: () => void;
}>();

const memoryDialogVNodeRef: VNodeRef = (el) => {
  props.setMemoryDialogRef((el as Element | null) ?? null);
};

const promptPreviewDialogVNodeRef: VNodeRef = (el) => {
  props.setPromptPreviewDialogRef((el as Element | null) ?? null);
};

function handleDetachConversation() {
  console.info("[独立聊天窗口][前端链路] AppWindowContent 收到 detachConversation，调用顶层处理函数", {
    viewMode: props.viewMode,
    detachedChatWindow: !!props.detachedChatWindow,
  });
  props.onDetachConversation();
}

const selectionShareDialogOpen = ref(false);
const selectionShareDialogLoading = ref(false);
const selectionSharePayload = ref<SelectionSharePayload | null>(null);

const chatBusyOverlay = computed(() => {
  if (props.branchingConversation) {
    return {
      title: props.t("chat.branchingConversation"),
      detail: props.t("chat.branchingConversationDetail"),
    };
  }
  if (props.forwardingConversationSelection) {
    return {
      title: props.t("chat.forwardingConversationSelection"),
      detail: props.t("chat.forwardingConversationSelectionDetail"),
    };
  }
  return null;
});

function openSelectionShareDialog(payload: SelectionSharePayload) {
  if (!payload || payload.count <= 0 || !Array.isArray(payload.blocks) || payload.blocks.length === 0) {
    return;
  }
  selectionSharePayload.value = payload;
  selectionShareDialogOpen.value = true;
}

function closeSelectionShareDialog() {
  if (selectionShareDialogLoading.value) return;
  selectionShareDialogOpen.value = false;
}

async function exportSelectionAsHtml() {
  const payload = selectionSharePayload.value;
  if (!payload || payload.count <= 0 || payload.blocks.length === 0) return;
  selectionShareDialogLoading.value = true;
  try {
    const path = await save({
      filters: [{ name: "HTML", extensions: ["html"] }],
      defaultPath: buildShareExportFileName("html"),
    });
    if (!path) return;
    const entries = await prepareShareEntries({
      blocks: payload.blocks,
      userAlias: props.userAlias,
      userAvatarUrl: props.userAvatarUrl,
      personaNameMap: props.chatPersonaNameMap,
      personaAvatarUrlMap: props.chatPersonaAvatarUrlMap,
      trigger: "selection_share_html",
    });
    const html = buildShareHtmlDocument({
      title: props.t("chat.shareDocumentTitle"),
      subtitle: props.t("chat.shareDocumentSubtitle", { count: payload.count }),
      entries,
    });
    await invokeTauri("write_utf8_text_file_to_path", {
      input: {
        path,
        text: html,
      },
    });
    props.setStatus(props.t("chat.shareHtmlExported", { path }));
    selectionShareDialogOpen.value = false;
  } catch (error) {
    props.setStatus(props.t("chat.shareExportFailed", { err: String(error) }));
  } finally {
    selectionShareDialogLoading.value = false;
  }
}

async function exportSelectionAsImage() {
  const payload = selectionSharePayload.value;
  if (!payload || payload.count <= 0 || payload.blocks.length === 0) return;
  selectionShareDialogLoading.value = true;
  try {
    const path = await save({
      filters: [{ name: "PNG", extensions: ["png"] }],
      defaultPath: buildShareExportFileName("png"),
    });
    if (!path) return;
    const entries = await prepareShareEntries({
      blocks: payload.blocks,
      userAlias: props.userAlias,
      userAvatarUrl: props.userAvatarUrl,
      personaNameMap: props.chatPersonaNameMap,
      personaAvatarUrlMap: props.chatPersonaAvatarUrlMap,
      trigger: "selection_share_image",
    });
    const dataUrl = await renderShareDocumentToPngDataUrl({
      title: props.t("chat.shareDocumentTitle"),
      subtitle: props.t("chat.shareDocumentSubtitle", { count: payload.count }),
      entries,
    });
    const bytesBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
    if (!bytesBase64) {
      throw new Error(props.t("chat.shareImageGenerationFailed"));
    }
    await invokeTauri("write_base64_file_to_path", {
      input: {
        path,
        bytesBase64,
      },
    });
    props.setStatus(props.t("chat.shareImageExported", { path }));
    selectionShareDialogOpen.value = false;
  } catch (error) {
    props.setStatus(props.t("chat.shareExportFailed", { err: String(error) }));
  } finally {
    selectionShareDialogLoading.value = false;
  }
}
</script>
