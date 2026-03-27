<template>
  <div
    class="window-content"
    :class="viewMode === 'chat'
      ? 'flex flex-col min-h-0 overflow-hidden'
      : viewMode === 'config'
        ? 'p-0 min-h-0 overflow-hidden'
        : 'p-2 overflow-auto'"
  >
    <ConfigView
      v-if="viewMode === 'config'"
      :config="config"
      :config-tab="configTab"
      :ui-language="config.uiLanguage"
      :locale-options="localeOptions"
      :current-theme="currentTheme"
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
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio-ready="!!hotkeyTestAudio"
      :checking-update="checkingUpdate"
      :save-config-action="saveConfig"
      :set-status-action="setStatus"
      @update:config-tab="updateConfigTab"
      @update:ui-language="setUiLanguage"
      @update:persona-editor-id="updatePersonaEditorId"
      @update:assistant-department-agent-id="updateSelectedPersonaId"
      @update:response-style-id="updateSelectedResponseStyleId"
      @update:pdf-read-mode="updateSelectedPdfReadMode"
      @update:background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      @update:background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      @save-chat-settings="saveChatSettings"
      @set-theme="setTheme"
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
        :can-record="speechRecognitionSupported"
        :recording="recording"
        :recording-ms="recordingMs"
        :transcribing="transcribing"
        :record-hotkey="recordHotkey"
        :media-drag-active="mediaDragActive"
        :chatting="chatting"
        :frozen="forcingArchive"
        :message-blocks="visibleMessageBlocks"
        :has-more-message-blocks="hasMoreMessageBlocks"
        :current-workspace-name="currentChatWorkspaceName"
        :workspace-locked="chatWorkspaceLocked"
        :active-conversation-id="currentChatConversationId"
        :unarchived-conversation-items="chatUnarchivedConversationItems"
        :current-theme="currentTheme"
        @update:chat-input="updateChatInput"
        @remove-clipboard-image="removeClipboardImage"
        @remove-queued-attachment-notice="removeQueuedAttachmentNotice"
        @pick-attachments="pickAttachments"
        @start-recording="startRecording"
        @stop-recording="stopRecording"
        @send-chat="sendChat"
        @stop-chat="stopChat"
        @load-more-message-blocks="loadMoreMessageBlocks"
        @recall-turn="onRecallTurn"
        @regenerate-turn="onRegenerateTurn"
        @lock-workspace="onLockChatWorkspace"
        @unlock-workspace="onUnlockChatWorkspace"
        @switch-conversation="onSwitchConversation"
        @create-conversation="onCreateConversation"
      />
      <div
        v-if="forcingArchive"
        class="absolute inset-0 z-20 flex items-center justify-center bg-base-100/60 backdrop-blur-[1px]"
      >
        <div class="rounded-box border border-base-300 bg-base-100 px-4 py-3 shadow-sm flex flex-col items-center gap-1">
          <span class="loading loading-spinner loading-sm"></span>
          <div class="text-sm">{{ t("chat.archiving") }}</div>
          <div class="text-sm opacity-70">{{ t("chat.archivingLock") }}</div>
        </div>
      </div>
    </div>

    <ArchivesView
      v-else
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
      :persona-name-map="chatPersonaNameMap"
      @load-archives="loadArchives"
      @select-archive="selectArchive"
      @select-unarchived-conversation="selectUnarchivedConversation"
      @select-delegate-conversation="selectDelegateConversation"
      @select-remote-im-contact-conversation="selectRemoteImContactConversation"
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
        :latest-input-length-text="t('prompt.latestInputLength')"
        :images-text="t('prompt.images')"
        :audios-text="t('prompt.audios')"
        :close-text="t('common.close')"
        :latest-user-text="promptPreviewLatestUserText"
        :latest-images="promptPreviewLatestImages"
        :latest-audios="promptPreviewLatestAudios"
        :text="promptPreviewText"
        @close="closePromptPreview"
      />
    </dialog>
  </div>
</template>

<script setup lang="ts">
import ConfigView from "../../config/views/ConfigView.vue";
import ChatView from "../../chat/views/ChatView.vue";
import ArchivesView from "../../archive/views/ArchivesView.vue";
import MemoryDialog from "../../memory/components/dialogs/MemoryDialog.vue";
import PromptPreviewDialog from "../../chat/components/dialogs/PromptPreviewDialog.vue";
import type { VNodeRef } from "vue";
import type {
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatMessage,
  ChatMessageBlock,
  ChatPersonaPresenceChip,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  ImageTextCacheStats,
  PersonaProfile,
  ResponseStyleOption,
  ToolLoadStatus,
  UnarchivedConversationSummary,
} from "../../../types/app";

type MemoryItem = {
  id: string;
  memoryType: "knowledge" | "skill" | "emotion" | "event";
  judgment: string;
  reasoning: string;
  tags: string[];
  ownerAgentId?: string;
};

const props = defineProps<{
  t: (key: string, params?: Record<string, unknown>) => string;
  viewMode: "chat" | "archives" | "config";
  config: AppConfig;
  configTab: "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
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
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudio: unknown;
  checkingUpdate: boolean;
  setStatus: (text: string) => void;
  userAlias: string;
  selectedPersonaName: string;
  userAvatarUrl: string;
  selectedPersonaAvatarUrl: string;
  chatPersonaNameMap: Record<string, string>;
  chatPersonaAvatarUrlMap: Record<string, string>;
  chatPersonaPresenceChips: ChatPersonaPresenceChip[];
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string }>;
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
  mediaDragActive: boolean;
  chatting: boolean;
  forcingArchive: boolean;
  visibleMessageBlocks: ChatMessageBlock[];
  hasMoreMessageBlocks: boolean;
  currentChatWorkspaceName: string;
  chatWorkspaceLocked: boolean;
  currentChatConversationId: string;
  chatUnarchivedConversationItems: Array<{
    conversationId: string;
    messageCount: number;
    updatedAt?: string;
    workspaceLabel?: string;
    isActive?: boolean;
    isMainConversation?: boolean;
    color?: string;
    canCreateNew?: boolean;
    backgroundStatus?: "completed" | "failed";
  }>;
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
  archiveSummaryText: string;
  unarchivedConversations: UnarchivedConversationSummary[];
  selectedUnarchivedConversationId: string;
  unarchivedMessages: ChatMessage[];
  delegateConversations: DelegateConversationSummary[];
  selectedDelegateConversationId: string;
  delegateMessages: ChatMessage[];
  remoteImContactConversations: RemoteImContactConversationSummary[];
  selectedRemoteImContactId: string;
  remoteImContactMessages: ChatMessage[];
  messageText: (message: ChatMessage) => string;
  extractMessageImages: (message?: ChatMessage) => Array<{ mime: string; bytesBase64: string }>;
  memoryList: MemoryItem[];
  memoryPage: number;
  memoryPageCount: number;
  pagedMemories: MemoryItem[];
  promptPreviewMode: "full" | "system";
  promptPreviewLoading: boolean;
  promptPreviewText: string;
  promptPreviewLatestUserText: string;
  promptPreviewLatestImages: number;
  promptPreviewLatestAudios: number;
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
  saveChatSettings: () => void;
  setTheme: (value: string) => void;
  refreshModels: () => void;
  saveConfig: () => Promise<boolean> | boolean;
  onToolsChanged: () => void;
  addApiConfig: () => void;
  removeSelectedApiConfig: () => void;
  addPersona: () => void;
  removeSelectedPersona: () => void;
  savePersonas: () => Promise<boolean> | boolean;
  importPersonaMemories: (payload: { agentId: string; file: File }) => void;
  openCurrentHistory: () => void;
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
  removeClipboardImage: (index: number) => void;
  removeQueuedAttachmentNotice: (index: number) => void;
  pickAttachments: () => void;
  startRecording: () => void;
  stopRecording: () => void;
  sendChat: () => void;
  stopChat: () => void;
  loadMoreMessageBlocks: () => void;
  onRecallTurn: (payload: { turnId: string }) => void;
  onRegenerateTurn: (payload: { turnId: string }) => void;
  onLockChatWorkspace: () => void;
  onUnlockChatWorkspace: () => void;
  onSwitchConversation: (conversationId: string) => void;
  onCreateConversation: () => void;
  loadArchives: () => void;
  selectArchive: (id: string) => void;
  selectUnarchivedConversation: (id: string) => void;
  selectDelegateConversation: (id: string) => void;
  selectRemoteImContactConversation: (id: string) => void;
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
</script>
