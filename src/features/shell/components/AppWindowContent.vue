<template>
  <div
    class="window-content"
    :class="viewMode === 'chat'
      ? 'flex flex-col min-h-0 overflow-hidden'
      : viewMode === 'config'
        ? 'p-3 min-h-0 overflow-hidden'
        : 'p-3 overflow-auto'"
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
      :selected-persona-id="selectedPersonaId"
      :selected-persona="selectedPersonaEditor"
      :selected-persona-avatar-url="selectedPersonaEditorAvatarUrl"
      :user-persona-avatar-url="userPersonaAvatarUrl"
      :response-style-options="responseStyleOptions"
      :response-style-id="selectedResponseStyleId"
      :text-capable-api-configs="textCapableApiConfigs"
      :image-capable-api-configs="imageCapableApiConfigs"
      :cache-stats="imageCacheStats"
      :cache-stats-loading="imageCacheStatsLoading"
      :avatar-saving="avatarSaving"
      :avatar-error="avatarError"
      :config-dirty="configDirty"
      :saving-config="saving"
      :hotkey-test-recording="hotkeyTestRecording"
      :hotkey-test-recording-ms="hotkeyTestRecordingMs"
      :hotkey-test-audio-ready="!!hotkeyTestAudio"
      @update:config-tab="updateConfigTab"
      @update:ui-language="setUiLanguage"
      @update:persona-editor-id="updatePersonaEditorId"
      @update:selected-persona-id="updateSelectedPersonaId"
      @update:response-style-id="updateSelectedResponseStyleId"
      @toggle-theme="toggleTheme"
      @refresh-models="refreshModels"
      @save-api-config="saveConfig"
      @add-api-config="addApiConfig"
      @remove-selected-api-config="removeSelectedApiConfig"
      @add-persona="addPersona"
      @remove-selected-persona="removeSelectedPersona"
      @open-current-history="openCurrentHistory"
      @open-prompt-preview="openPromptPreview"
      @open-system-prompt-preview="openSystemPromptPreview"
      @open-memory-viewer="openMemoryViewer"
      @refresh-image-cache-stats="refreshImageCacheStats"
      @clear-image-cache="clearImageCache"
      @start-hotkey-record-test="startHotkeyRecordTest"
      @stop-hotkey-record-test="stopHotkeyRecordTest"
      @play-hotkey-record-test="playHotkeyRecordTest"
      @capture-hotkey="captureHotkey"
      @save-agent-avatar="saveAgentAvatar"
      @clear-agent-avatar="clearAgentAvatar"
    />

    <div v-else-if="viewMode === 'chat'" class="relative flex-1 min-h-0">
      <ChatView
        :user-alias="userAlias"
        :persona-name="selectedPersonaName"
        :user-avatar-url="userAvatarUrl"
        :assistant-avatar-url="selectedPersonaAvatarUrl"
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
        :can-record="speechRecognitionSupported"
        :recording="recording"
        :recording-ms="recordingMs"
        :record-hotkey="recordHotkey"
        :chatting="chatting"
        :frozen="forcingArchive"
        :turns="visibleTurns"
        :has-more-turns="hasMoreTurns"
        @update:chat-input="updateChatInput"
        @remove-clipboard-image="removeClipboardImage"
        @start-recording="startRecording"
        @stop-recording="stopRecording"
        @send-chat="sendChat"
        @stop-chat="stopChat"
        @load-more-turns="loadMoreTurns"
      />
      <div
        v-if="forcingArchive"
        class="absolute inset-0 z-20 flex items-center justify-center bg-base-100/60 backdrop-blur-[1px]"
      >
        <div class="rounded-box border border-base-300 bg-base-100 px-4 py-3 shadow-sm flex flex-col items-center gap-1">
          <span class="loading loading-spinner loading-sm"></span>
          <div class="text-sm">{{ t("chat.archiving") }}</div>
          <div class="text-xs opacity-70">{{ t("chat.archivingLock") }}</div>
        </div>
      </div>
    </div>

    <ArchivesView
      v-else
      :archives="archives"
      :selected-archive-id="selectedArchiveId"
      :archive-messages="archiveMessages"
      :render-message="renderMessage"
      @load-archives="loadArchives"
      @select-archive="selectArchive"
      @export-archive="exportArchive"
      @delete-archive="deleteArchive"
    />
    <dialog :ref="historyDialogVNodeRef" class="modal">
      <HistoryDialog
        :title="t('chat.currentHistoryTitle')"
        :close-text="t('common.close')"
        :messages="currentHistory"
        :message-text="messageText"
        :extract-images="extractMessageImages"
        @close="closeHistory"
      />
    </dialog>
    <dialog :ref="memoryDialogVNodeRef" class="modal">
      <MemoryDialog
        :import-input-ref="memoryImportInput"
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
import HistoryDialog from "../../chat/components/dialogs/HistoryDialog.vue";
import MemoryDialog from "../../memory/components/dialogs/MemoryDialog.vue";
import PromptPreviewDialog from "../../chat/components/dialogs/PromptPreviewDialog.vue";
import type { VNodeRef } from "vue";
import type {
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatMessage,
  ChatTurn,
  ImageTextCacheStats,
  PersonaProfile,
  ResponseStyleOption,
  ToolLoadStatus,
} from "../../../types/app";

type MemoryItem = { id: string; content: string; keywords: string[] };

const props = defineProps<{
  t: (key: string, params?: Record<string, unknown>) => string;
  viewMode: "chat" | "archives" | "config";
  config: AppConfig;
  configTab: "hotkey" | "api" | "tools" | "persona" | "chatSettings";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "ja-JP" | "ko-KR"; label: string }>;
  currentTheme: "light" | "forest";
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
  selectedPersonaId: string;
  selectedPersonaEditor: PersonaProfile | null;
  selectedPersonaEditorAvatarUrl: string;
  userPersonaAvatarUrl: string;
  responseStyleOptions: ResponseStyleOption[];
  selectedResponseStyleId: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  imageCacheStats: ImageTextCacheStats;
  imageCacheStatsLoading: boolean;
  avatarSaving: boolean;
  avatarError: string;
  configDirty: boolean;
  saving: boolean;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudio: unknown;
  userAlias: string;
  selectedPersonaName: string;
  userAvatarUrl: string;
  selectedPersonaAvatarUrl: string;
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  latestAssistantText: string;
  latestReasoningStandardText: string;
  latestReasoningInlineText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  chatInput: string;
  chatInputPlaceholder: string;
  speechRecognitionSupported: boolean;
  recording: boolean;
  recordingMs: number;
  recordHotkey: string;
  chatting: boolean;
  forcingArchive: boolean;
  visibleTurns: ChatTurn[];
  hasMoreTurns: boolean;
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveMessages: ChatMessage[];
  renderMessage: (message: ChatMessage) => string;
  currentHistory: ChatMessage[];
  messageText: (message: ChatMessage) => string;
  extractMessageImages: (message?: ChatMessage) => Array<{ mime: string; bytesBase64: string }>;
  memoryImportInput: HTMLInputElement | null;
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
  setHistoryDialogRef: (el: Element | null) => void;
  setMemoryDialogRef: (el: Element | null) => void;
  setPromptPreviewDialogRef: (el: Element | null) => void;
  updateConfigTab: (value: "hotkey" | "api" | "tools" | "persona" | "chatSettings") => void;
  setUiLanguage: (value: string) => void;
  updatePersonaEditorId: (value: string) => void;
  updateSelectedPersonaId: (value: string) => void;
  updateSelectedResponseStyleId: (value: string) => void;
  toggleTheme: () => void;
  refreshModels: () => void;
  saveConfig: () => void;
  addApiConfig: () => void;
  removeSelectedApiConfig: () => void;
  addPersona: () => void;
  removeSelectedPersona: () => void;
  openCurrentHistory: () => void;
  openPromptPreview: () => void;
  openSystemPromptPreview: () => void;
  openMemoryViewer: () => void;
  refreshImageCacheStats: () => void;
  clearImageCache: () => void;
  startHotkeyRecordTest: () => void;
  stopHotkeyRecordTest: () => void;
  playHotkeyRecordTest: () => void;
  captureHotkey: (value: string) => void;
  saveAgentAvatar: (input: { agentId: string; mime: string; bytesBase64: string }) => void;
  clearAgentAvatar: (input: { agentId: string }) => void;
  updateChatInput: (value: string) => void;
  removeClipboardImage: (index: number) => void;
  startRecording: () => void;
  stopRecording: () => void;
  sendChat: () => void;
  stopChat: () => void;
  loadMoreTurns: () => void;
  loadArchives: () => void;
  selectArchive: (id: string) => void;
  exportArchive: (payload: { format: "markdown" | "json" }) => void;
  deleteArchive: (id: string) => void;
  closeHistory: () => void;
  closeMemoryViewer: () => void;
  prevMemoryPage: () => void;
  nextMemoryPage: () => void;
  exportMemories: () => void;
  triggerMemoryImport: () => void;
  handleMemoryImportFile: (event: Event) => void;
  closePromptPreview: () => void;
}>();

const historyDialogVNodeRef: VNodeRef = (el) => {
  props.setHistoryDialogRef((el as Element | null) ?? null);
};

const memoryDialogVNodeRef: VNodeRef = (el) => {
  props.setMemoryDialogRef((el as Element | null) ?? null);
};

const promptPreviewDialogVNodeRef: VNodeRef = (el) => {
  props.setPromptPreviewDialogRef((el as Element | null) ?? null);
};
</script>
