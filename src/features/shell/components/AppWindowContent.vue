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
      :theme-mode="themeMode"
      :auto-light-theme="autoLightTheme"
      :auto-dark-theme="autoDarkTheme"
      :generated-theme-controls="generatedThemeControls"
      :generated-theme-tokens="generatedThemeTokens"
      :generated-light-tokens="generatedLightTokens"
      :generated-dark-tokens="generatedDarkTokens"
      :ui-size-scale="config.uiSizeScale ?? 100"
      :selected-api-config="selectedApiConfig"
      :base-url-reference="baseUrlReference"
      :refreshing-models="refreshingModels"
      :model-options="selectedModelOptions"
      :model-refresh-ok="modelRefreshOk"
      :model-refresh-error="modelRefreshError"
      :tool-statuses="toolStatuses"
      :personas="personas"
      :persona-avatar-url-map="chatPersonaAvatarUrlMap"
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
      :microphone-permission-state="microphonePermissionState"
      :microphone-permission-requesting="microphonePermissionRequesting"
      :checking-update="checkingUpdate"
      :has-available-update="hasAvailableUpdate"
      :save-config-action="saveConfig"
      :update-record-hotkey-action="updateRecordHotkey"
      :update-record-background-wake-enabled-action="updateRecordBackgroundWakeEnabled"
      :restore-config-action="restoreConfig"
      :last-saved-config-json="lastSavedConfigJson"
      :set-status-action="setStatus"
      @update:config-tab="updateConfigTab"
      @update:ui-language="setUiLanguage"
      @update:persona-editor-id="updatePersonaEditorId"
      @update:response-style-id="updateSelectedResponseStyleId"
      @update:pdf-read-mode="updateSelectedPdfReadMode"
      @update:background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      @update:background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      @update:instruction-presets="updateInstructionPresets"
      @patch-conversation-api-settings="patchConversationApiSettings"
      @patch-chat-settings="patchChatSettings"
      @update:ui-size-scale="updateUiSizeScale"
      @update:github-update-method="updateGithubUpdateMethod"
      @set-theme="setTheme"
      @set-theme-mode="setThemeMode"
      @set-auto-theme="setAutoTheme"
      @activate-generated-theme="activateGeneratedTheme"
      @update-generated-theme-controls="updateGeneratedThemeControls"
      @reset-generated-theme="resetGeneratedTheme"
      @refresh-models="refreshModels"
      @add-api-config="addApiConfig"
      @remove-selected-api-config="removeSelectedApiConfig"
      @add-persona="addPersona"
      @remove-selected-persona="removeSelectedPersona"
      @reset-personas="resetPersonas"
      @save-personas="savePersonas"
      @convert-private-persona-to-public="convertPrivatePersonaToPublic"
      @import-persona-memories="importPersonaMemories"
      @open-conversation-list="openConversationList"
      @open-prompt-preview="openPromptPreview"
      @open-system-prompt-preview="openSystemPromptPreview"
      @open-memory-viewer="openMemoryViewer"
      @open-runtime-logs="openRuntimeLogs"
      @start-hotkey-record-test="startHotkeyRecordTest"
      @stop-hotkey-record-test="stopHotkeyRecordTest"
      @play-hotkey-record-test="playHotkeyRecordTest"
      @request-microphone-permission="requestMicrophonePermission"
      @capture-hotkey="captureHotkey"
      @summon-chat-now="summonChatNow"
      @save-agent-avatar="saveAgentAvatar"
      @clear-agent-avatar="clearAgentAvatar"
      @check-update="checkUpdate"
      @open-github="openGithub"
    />

    <div v-else-if="viewMode === 'chat'" class="relative flex-1 min-h-0">
      <ChatView
        ref="chatViewRef"
        composer-scope="main"
        :user-alias="userAlias"
        :persona-name="selectedPersonaName"
        :user-avatar-url="userAvatarUrl"
        :assistant-avatar-url="selectedPersonaAvatarUrl"
        :persona-name-map="chatPersonaNameMap"
        :persona-avatar-url-map="chatPersonaAvatarUrlMap"
        :mention-entries="chatMentionEntries"
        :selected-mentions="selectedChatMentions"
        :latest-user-text="latestUserText"
        :latest-user-images="latestUserImages"
        :frontend-round-phase="frontendRoundPhase"
        :submit-pending="submitPending"
        :chat-error-text="chatErrorText"
        :clipboard-images="clipboardImages"
        :queued-attachment-notices="queuedAttachmentNotices"
        :chat-input="chatInput"
        :instruction-presets="instructionPresets"
        :recording="recording"
        :recording-ms="recordingMs"
        :transcribing="transcribing"
        :conversation-call-primary-api-config-id="conversationCallPrimaryApiConfigId"
        :preferred-chat-model-id="preferredChatModelId"
        :tool-review-api-config-id="config.toolReviewApiConfigId || ''"
        :tool-review-refresh-tick="toolReviewRefreshTick"
        :terminal-approvals="terminalApprovals"
        :terminal-approval-resolving="terminalApprovalResolving"
        :chat-model-options="textCapableApiConfigs"
        :plan-mode-enabled="planModeEnabled"
        :chat-usage-percent="chatUsagePercent"
        :trim-tip="trimTip"
        :media-drag-active="mediaDragActive"
        :chatting="chatting"
        :trimming="trimming"
        :trimming-conversation-id="trimmingConversationId"
        :compacting-conversation="compactingConversation"
        :compacting-conversation-id="compactingConversationId"
        :conversation-busy="
          isViewLayerBusy({
            trimming,
            trimmingConversationId,
            compactingConversation,
            compactingConversationId,
            activeConversationId: currentChatConversationId,
            organizingContext: false,
          })
        "
        :frozen="branchingConversation || forwardingConversationSelection"
        :message-blocks="visibleMessageBlocks"
        :has-more-history="chatHasMoreHistory"
        :loading-older-history="chatLoadingOlderHistory"
        :latest-own-message-align-request="latestOwnMessageAlignRequest"
        :conversation-scroll-to-bottom-request="conversationScrollToBottomRequest"
        :scroll-to-bottom-behavior="scrollToBottomBehavior"
        :current-workspace-name="currentChatWorkspaceName"
        :current-workspace-display-name="currentChatWorkspaceDisplayName"
        :current-workspace-root-path="currentChatWorkspaceRootPath"
        :current-workspace-autonomous-mode="currentChatWorkspaceAutonomousMode"
        :current-workspace-work-mode="currentChatWorkMode"
        :current-workspace-branch="currentChatWorkBranch || ''"
        :workspaces="currentChatWorkspaces"
        :config-shell-workspaces="config.shellWorkspaces || []"
        :save-draft-workspaces="saveDraftWorkspaces"
        :draft-workspace-git-root-check="draftWorkspaceGitRootCheck"
        :current-department-id="currentChatDepartmentId"
        :active-agent-id="currentChatAgentId"
        :active-conversation-id="currentChatConversationId"
        :current-todos="currentChatTodos"
        :goal-active="chatGoalActive"
        :goal-title="chatGoalTitle"
        :goal-dialog-open="goalDialogOpen"
        :goal-saving="goalSaving"
        :goal-error="goalError"
        :active-goal-task="activeGoalTask"
        :recent-goal-task-history="recentGoalTaskHistory"
        :unarchived-conversation-items="chatUnarchivedConversationItems"
        :remote-im-contact-conversations="remoteImContactConversations"
        :conversation-items="chatConversationItems || chatUnarchivedConversationItems"
        :create-conversation-department-options="createConversationDepartmentOptions"
        :recipient-options-ready="recipientOptionsReady"
        :default-create-conversation-department-id="defaultCreateConversationDepartmentId"
        :ide-context-groups="[]"
        :current-theme="currentTheme"
        :side-conversation-list-visible="sideConversationListVisible"
        :initial-tool-review-panel-open="initialToolReviewPanelOpen"
        :conversation-list-tab="conversationListTab"
        :chat-left-panel-mode="chatLeftPanelMode"
        :chat-right-panel-mode="chatRightPanelMode"
        :chat-monitor-panel-mode="chatMonitorPanelMode"
        :side-chat-panel-enabled="true"
        :side-chat-items="sideChatItems"
        @open-side-chat-conversation="openSideChatFromHome"
        @open-side-chat-new-page="openSideChatNewPage?.()"
        @update:chat-input="updateChatInput"
        @add-mention="addChatMention"
        @remove-mention="removeChatMention"
        @side-conversation-list-visible-change="setSideConversationListVisible"
        @tool-review-panel-open-change="setToolReviewPanelOpen"
        @open-chat-reader-file="openChatReaderFile"
        @open-chat-reader-directory="openChatReaderDirectory"
        @side-panel-widths-change="setChatSidePanelWidths"
        @side-panel-widths-commit="commitChatSidePanelWidths"
        @update:conversation-list-tab="updateConversationListTab"
        @update:chat-left-panel-mode="updateChatLeftPanelMode"
        @update:chat-right-panel-mode="updateChatRightPanelMode"
        @update:chat-monitor-panel-mode="updateChatMonitorPanelMode"
        @remove-clipboard-image="removeClipboardImage"
        @remove-queued-attachment-notice="removeQueuedAttachmentNotice"
        @pick-attachments="pickAttachments"
        @update:conversation-preferred-api-config-id="updateConversationPreferredApiConfigId"
        @update:plan-mode-enabled="updatePlanModeEnabled"
        @send-chat="sendChat"
        @stop-chat="stopChat"
        @clear-chat-error="clearChatError"
        @load-older-history="onLoadOlderChatHistory"
        @load-older-compaction-segment="onLoadOlderCompactionSegment"
        @reached-bottom="onReachedChatBottom"
        @jump-to-conversation-bottom="onJumpToConversationBottom"
        @create-conversation-branch-from-turn="onCreateConversationBranchFromTurn"
        @recall-turn="onRecallTurn"
        @regenerate-turn="onRegenerateTurn"
        @confirm-plan="confirmPlan"
        @trim-conversation="openTrimActionDialog"
        @open-conversation-list="openConversationList"
        @open-settings="openSettingsWindow"
        @selection-action-copy="() => {}"
        @selection-action-copy-error="() => {}"
        @selection-action-branch="onBranchConversationFromSelection($event)"
        @branch-conversation-from-current="onBranchConversationFromCurrent()"
        @selection-action-forward="onForwardConversationFromSelection($event)"
        @selection-action-delegate="handleUserAsyncDelegateFromSelection"
        @selection-action-share="handleSelectionShareAction($event)"
        @attach-tool-review-report="attachToolReviewReport"
        @lock-workspace="onLockChatWorkspace"
        @open-code-review="$emit('open-code-review')"
        @open-goal-task="openGoalTaskDialog"
        @close-goal-task="closeGoalTaskDialog"
        @save-goal-task="saveGoalTask"
        @stop-goal-task="stopGoalTask"
        @task-created="setStatus(props.t('config.task.created'))"
        @task-updated="setStatus(props.t('config.task.updated'))"
        @refresh-tool-review-message="onRefreshToolReviewMessage"
        @switch-conversation="onSwitchConversation"
        @rename-conversation="onRenameConversation"
        @toggle-pin-conversation="onToggleConversationPin"
        @archive-conversation="onArchiveConversation"
        @export-conversation="exportConversationShare"
        @delete-conversation="onDeleteConversation"
        @rebind-conversation-recipient="onRebindConversationRecipient"
        @update-draft-conversation="onUpdateDraftConversation"
        @create-conversation="onCreateConversation"
        @approve-terminal-approval="approveTerminalApproval"
        @deny-terminal-approval="denyTerminalApproval"
        @approve-terminal-approval-for-session="approveTerminalApprovalForSession"
        @approve-terminal-approval-for-workspace="approveTerminalApprovalForWorkspace"
      >
        <template #side-chat-panel>
          <div class="flex h-full min-h-0 w-full flex-col">
            <PanelTabStrip
              :tabs="sideChatTabs"
              :active-key="sideConversationId"
              :aria-label="t('chat.sideChat.title')"
              :close-title="t('chat.sideChat.close')"
              :close-left-title="t('fileReader.closeLeft')"
              :close-right-title="t('fileReader.closeRight')"
              :close-others-title="t('fileReader.closeOthers')"
              @select-tab="selectSideChatConversation?.($event)"
              @close-tab="closeSideChatTab"
              @close-tabs-to-left="closeSideChatTabsToLeft"
              @close-tabs-to-right="closeSideChatTabsToRight"
              @close-other-tabs="closeOtherSideChatTabs"
            >
              <template #leading>
                <ChatRightPanelSwitcher @select-home="updateChatRightPanelMode('home')" />
              </template>
              <template #tabTrailing>
                <button
                  type="button"
                  class="btn btn-ghost btn-sm btn-circle"
                  :title="t('chat.sideChat.create')"
                  @click="openSideChatNewPage?.()"
                >
                  <Plus class="size-4" />
                </button>
              </template>
            </PanelTabStrip>
            <ConversationView
              v-if="sideConversationId"
              :key="sideConversationId"
              class="min-h-0 flex-1"
              :subscription-slot="sideChatSubscriptionSlot"
              :conversation-id="sideConversationId"
              :api-config-id="conversationCallPrimaryApiConfigId"
              :agent-id="currentChatAgentId"
              :department-id="currentChatDepartmentId"
              :persona-name="selectedPersonaName"
              :user-alias="userAlias"
              :user-avatar-url="userAvatarUrl"
              :assistant-avatar-url="selectedPersonaAvatarUrl"
              :persona-name-map="chatPersonaNameMap"
              :persona-avatar-url-map="chatPersonaAvatarUrlMap"
              :chat-model-options="textCapableApiConfigs"
              :instruction-presets="instructionPresets"
              :config="config"
              :workspace-name="currentChatWorkspaceDisplayName"
              :workspace-root-path="currentChatWorkspaceRootPath"
              :workspaces="currentChatWorkspaces"
              :workspace-access="currentChatWorkspaces.find((item) => item.level === 'main')?.access || 'approval'"
              :current-theme="currentTheme"
              :terminal-approvals="terminalApprovals"
              :terminal-approval-resolving="terminalApprovalResolving"
              :approve-terminal-approval="(requestId: string, reason?: string) => approveTerminalApproval(requestId, reason)"
              :deny-terminal-approval="(requestId: string, reason?: string) => denyTerminalApproval(requestId, reason)"
              :approve-terminal-approval-for-session="(requestId) => approveTerminalApprovalForSession(requestId)"
              :approve-terminal-approval-for-workspace="(requestId) => approveTerminalApprovalForWorkspace(requestId)"
              :request-recall-mode="requestRecallMode"
              :create-conversation-branch-from-turn="(payload) => createSideConversationBranchFromTurn?.({ ...payload, sourceConversationId: sideConversationId })"
            />
            <div v-else class="relative flex min-h-0 flex-1 flex-col items-center justify-center gap-5 overflow-hidden px-6">
              <div class="relative flex w-full max-w-80 flex-col items-center gap-5">
                <!-- 按钮身后的主题色辉光：替代原先三团散在四角的写死光斑 -->
                <div
                  class="pointer-events-none absolute left-1/2 top-1/2 h-56 w-[420px] max-w-[85vw] -translate-x-1/2 -translate-y-1/2 rounded-[100%] bg-primary/10 blur-3xl"
                  aria-hidden="true"
                ></div>
                <button
                  type="button"
                  class="btn btn-lg btn-primary z-10 h-16 w-full max-w-80 rounded-2xl text-base shadow-lg"
                  @click="createSideChatConversation?.(true)"
                >
                  {{ t('chat.sideChat.newPageInContext', { persona: selectedPersonaName }) }}
                </button>
                <button
                  type="button"
                  class="btn btn-lg z-10 h-16 w-full max-w-80 rounded-2xl border-base-300 bg-base-100 text-base shadow-lg hover:bg-base-200"
                  @click="createSideChatConversation?.(false)"
                >
                  {{ t('chat.sideChat.newPageBlank', { persona: selectedPersonaName }) }}
                </button>
              </div>
            </div>
          </div>
        </template>
      </ChatView>
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
      :unarchived-conversations="unarchivedConversations"
      :unarchived-blocks="unarchivedBlocks"
      :selected-unarchived-conversation-id="selectedUnarchivedConversationId"
      :selected-unarchived-block-id="selectedUnarchivedBlockId"
      :unarchived-has-prev-block="unarchivedHasPrevBlock"
      :unarchived-has-next-block="unarchivedHasNextBlock"
      :unarchived-messages="unarchivedMessages"
      :delegate-conversations="delegateConversations"
      :delegate-blocks="delegateBlocks"
      :selected-delegate-conversation-id="selectedDelegateConversationId"
      :selected-delegate-block-id="selectedDelegateBlockId"
      :delegate-has-prev-block="delegateHasPrevBlock"
      :delegate-has-next-block="delegateHasNextBlock"
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
      :current-theme="currentTheme"
      @load-archives="loadArchives"
      @select-archive="selectArchive"
      @select-archive-block="selectArchiveBlock"
      @select-unarchived-conversation="selectUnarchivedConversation"
      @select-unarchived-block="selectUnarchivedConversationBlock"
      @select-delegate-conversation="selectDelegateConversation"
      @select-delegate-block="selectDelegateConversationBlock"
      @select-remote-im-contact-conversation="selectRemoteImContactConversation"
      @select-remote-im-contact-block="selectRemoteImContactConversationBlock"
      @export-archive="exportArchive"
      @import-archive-file="importArchiveFile"
      @unarchive-archive="unarchiveArchive"
      @delete-archive="deleteArchive"
      @delete-unarchived-conversation="deleteUnarchivedConversation"
      @delete-delegate-conversation="deleteDelegateConversation"
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
    <dialog :ref="promptPreviewDialogVNodeRef" class="modal" @close="markPromptPreviewDialogClosed">
      <PromptPreviewDialog
        v-if="promptPreviewDialogOpen"
        :mode="promptPreviewMode"
        :conversation-scope="promptPreviewConversationScope"
        :loading="promptPreviewLoading"
        :title="promptPreviewMode === 'system' ? t('prompt.systemPreview') : t('prompt.requestPreview')"
        :loading-text="t('common.loading')"
        :empty-hint="t('prompt.emptyHint')"
        :chat-text="t('prompt.chat')"
        :compaction-text="t('prompt.compaction')"
        :archive-text="t('prompt.archive')"
        :local-scope-text="t('prompt.local')"
        :remote-scope-text="t('prompt.remote')"
        :delegate-scope-text="t('prompt.delegate')"
        :conversation-text="t('prompt.conversation')"
        :selected-conversation-id="promptPreviewConversationId"
        :conversation-options="promptPreviewConversationOptions"
        :latest-input-length-text="t('prompt.latestInputLength')"
        :images-text="t('prompt.images')"
        :audios-text="t('prompt.audios')"
        :close-text="t('common.close')"
        :latest-user-text="promptPreviewLatestUserText"
        :latest-images="promptPreviewLatestImages"
        :latest-audios="promptPreviewLatestAudios"
        :text="promptPreviewText"
        @select-mode="loadPromptPreview"
        @select-scope="selectPromptPreviewConversationScope"
        @select-conversation="selectPromptPreviewConversation"
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
import ConversationView from "../../chat/views/ConversationView.vue";
import ChatRightPanelSwitcher from "../../chat/components/ChatRightPanelSwitcher.vue";
import PanelTabStrip from "../../shared/components/PanelTabStrip.vue";
import { MessageSquareMore, Plus } from "@lucide/vue";
import type { TerminalApprovalConversationItem } from "../composables/use-terminal-approval";
import ArchivesView from "../../archive/views/ArchivesView.vue";
import MemoryDialog from "../../memory/components/dialogs/MemoryDialog.vue";
import PromptPreviewDialog from "../../chat/components/dialogs/PromptPreviewDialog.vue";
import SelectionShareDialog from "../../chat/components/dialogs/SelectionShareDialog.vue";
import { computed, ref, type VNodeRef } from "vue";
import type {
  ApiConfigItem,
  AppConfig,
  ArchiveSummary,
  ChatConversationOverviewItem,
  ChatMentionEntry,
  ChatMentionTarget,
  ChatMessage,
  ChatMessageBlock,
  ChatTodoItem,
  ChildConversationSummary,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  PersonaProfile,
  PromptCommandPreset,
  ResponseStyleOption,
  ShellWorkMode,
  ShellWorkspace,
  ToolLoadStatus,
  UnarchivedConversationSummary,
} from "../../../types/app";
import type { GeneratedThemeControls, GeneratedThemeTokens, ThemeMode, ThemeModeKind } from "../../shell/theme/theme-types";
import type { DepartmentPersonaOption } from "../../shared/department-persona-options";
import type { ChatMonitorPanelMode, ChatRightPanelMode } from "../../chat/composables/chat-ui-layout-storage";
import { createExclusiveChatViewSubscriptionSlot } from "../../chat/composables/exclusive-chat-view-subscription-slot";
import { isViewLayerBusy } from "../../chat/composables/chat-view-busy";
import {
  buildShareExportFileName,
  generateShareFromMessageIds,
} from "../../chat/utils/share-generator";
import {
  invokeTauri,
  saveTransportFileDialog,
  writeTransportBase64File,
  writeTransportUtf8TextFile,
} from "../../../services/tauri-api";

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
  conversationId?: string;
  exportFormat?: "html" | "png" | "copyPng";
};

const props = defineProps<{
  t: (key: string, params?: Record<string, unknown>) => string;
  viewMode: "chat" | "archives" | "config";
  sideConversationListVisible: boolean;
  initialToolReviewPanelOpen: boolean;
  conversationListTab: "local" | "contact" | "task";
  chatLeftPanelMode: "local" | "contact" | "task";
  chatRightPanelMode: ChatRightPanelMode;
  chatMonitorPanelMode: ChatMonitorPanelMode;
  config: AppConfig;
  configTab: "welcome" | "hotkey" | "api" | "mcp" | "skill" | "catalog" | "persona" | "department" | "departmentTree" | "demo" | "chatSettings" | "notification" | "networkAccess" | "remoteIm" | "usage" | "memory" | "task" | "logs" | "appearance" | "migration" | "about";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
  themeMode: ThemeModeKind;
  autoLightTheme: string;
  autoDarkTheme: string;
  generatedThemeControls: GeneratedThemeControls;
  generatedThemeTokens: GeneratedThemeTokens;
  generatedLightTokens: GeneratedThemeTokens;
  generatedDarkTokens: GeneratedThemeTokens;
  selectedApiConfig: ApiConfigItem | null;
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
  microphonePermissionState: "granted" | "denied" | "prompt" | "unsupported" | "unknown";
  microphonePermissionRequesting: boolean;
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
  chatMentionEntries: ChatMentionEntry[];
  selectedChatMentions: ChatMentionTarget[];
  latestUserText: string;
  latestUserImages: Array<{ mime: string; bytesBase64: string }>;
  frontendRoundPhase: "idle" | "queued" | "waiting" | "streaming";
  submitPending?: boolean;
  chatErrorText: string;
  clipboardImages: Array<{ mime: string; bytesBase64: string }>;
  queuedAttachmentNotices: Array<{ id: string; fileName: string; path: string; mime: string; pending?: boolean }>;
  chatInput: string;
  recording: boolean;
  recordingMs: number;
  transcribing: boolean;
  conversationCallPrimaryApiConfigId: string;
  preferredChatModelId?: string;
  toolReviewRefreshTick: number;
  terminalApprovals?: TerminalApprovalConversationItem[];
  terminalApprovalResolving?: boolean;
  approveTerminalApproval: (requestId?: string, reason?: string) => void;
  denyTerminalApproval: (requestId?: string, reason?: string) => void;
  approveTerminalApprovalForSession: (requestId?: string) => void;
  approveTerminalApprovalForWorkspace: (requestId?: string) => void;
  planModeEnabled: boolean;
  chatUsagePercent: number;
  trimTip: string;
  mediaDragActive: boolean;
  chatting: boolean;
  trimming: boolean;
  trimmingConversationId?: string;
  compactingConversation: boolean;
  compactingConversationId?: string;
  branchingConversation: boolean;
  forwardingConversationSelection: boolean;
  visibleMessageBlocks: ChatMessageBlock[];
  chatHasMoreHistory: boolean;
  chatLoadingOlderHistory: boolean;
  latestOwnMessageAlignRequest: number;
  conversationScrollToBottomRequest: number;
  scrollToBottomBehavior: "auto" | "smooth" | "own_top" | "manual" | "follow";
  currentChatWorkspaceName: string;
  currentChatWorkspaceDisplayName: string;
  currentChatWorkspaceRootPath: string;
  currentChatWorkspaceAutonomousMode: boolean;
  currentChatWorkspaces: ShellWorkspace[];
  currentChatWorkMode?: ShellWorkMode;
  currentChatWorkBranch?: string;
  saveDraftWorkspaces?: (items: ShellWorkspace[], autonomousMode: boolean, workMode: ShellWorkMode, shellWorkBranch?: string) => Promise<void>;
  draftWorkspaceGitRootCheck?: (path: string) => Promise<boolean>;
  currentChatDepartmentId: string;
  currentChatAgentId: string;
  currentChatConversationId: string;
  sideConversations?: ChildConversationSummary[];
  sideConversationId?: string;
  createSideChatConversation?: (withContext?: boolean) => Promise<string> | string;
  openSideChatNewPage?: () => void;
  selectSideChatConversation?: (conversationId: string) => void;
  createSideConversationBranchFromTurn?: (payload: { turnId: string; sourceConversationId?: string }) => Promise<void> | void;
  closeSideChatConversations?: (conversationIds: string[]) => Promise<void> | void;
  currentChatTodos: ChatTodoItem[];
  chatGoalActive: boolean;
  chatGoalTitle: string;
  goalDialogOpen: boolean;
  goalSaving: boolean;
  goalError: string;
  activeGoalTask: {
    taskId: string;
    goal: string;
    why: string;
    todo: string;
    endAtLocal: string;
    remainingHours: number;
  } | null;
  recentGoalTaskHistory: Array<{
    goal: string;
    why: string;
    todo: string;
    durationHours: number;
  }>;
  chatUnarchivedConversationItems: ChatConversationOverviewItem[];
  chatConversationItems?: ChatConversationOverviewItem[];
  createConversationDepartmentOptions: DepartmentPersonaOption[];
  recipientOptionsReady?: boolean;
  defaultCreateConversationDepartmentId: string;
  archives: ArchiveSummary[];
  selectedArchiveId: string;
  archiveBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedArchiveBlockId?: number | null;
  archiveHasPrevBlock?: boolean;
  archiveHasNextBlock?: boolean;
  archiveMessages: ChatMessage[];
  unarchivedConversations: UnarchivedConversationSummary[];
  unarchivedBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedUnarchivedConversationId: string;
  selectedUnarchivedBlockId?: number | null;
  unarchivedHasPrevBlock?: boolean;
  unarchivedHasNextBlock?: boolean;
  unarchivedMessages: ChatMessage[];
  delegateConversations: DelegateConversationSummary[];
  delegateBlocks: import("../../../types/app").ConversationBlockSummary[];
  selectedDelegateConversationId: string;
  selectedDelegateBlockId?: number | null;
  delegateHasPrevBlock?: boolean;
  delegateHasNextBlock?: boolean;
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
  promptPreviewConversationScope: "local" | "remote" | "delegate";
  promptPreviewConversationId: string;
  promptPreviewConversationOptions: Array<{ conversationId: string; title: string }>;
  loadPromptPreview: (mode: "chat" | "compaction" | "archive") => void;
  selectPromptPreviewConversationScope: (scope: "local" | "remote" | "delegate") => void;
  selectPromptPreviewConversation: (conversationId: string) => void;
  setMemoryDialogRef: (el: Element | null) => void;
  setPromptPreviewDialogRef: (el: Element | null) => void;
  promptPreviewDialogOpen: boolean;
  markPromptPreviewDialogClosed: () => void;
  updateConfigTab: (value: "hotkey" | "api" | "mcp" | "skill" | "catalog" | "persona" | "department" | "departmentTree" | "demo" | "chatSettings" | "notification" | "networkAccess" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about") => void;
  setUiLanguage: (value: string) => void;
  updatePersonaEditorId: (value: string) => void;
  updateSelectedResponseStyleId: (value: string) => void;
  updateSelectedPdfReadMode: (value: "text" | "image") => void;
  updateBackgroundVoiceScreenshotKeywords: (value: string) => void;
  updateBackgroundVoiceScreenshotMode: (value: "desktop" | "focused_window") => void;
  updateInstructionPresets: (value: PromptCommandPreset[]) => void;
  patchConversationApiSettings: (value: import("../../../types/app").ConversationApiSettingsPatch) => void;
  patchChatSettings: (value: import("../../../types/app").ChatSettingsPatch) => void;
  updateUiSizeScale: (value: number) => void;
  updateGithubUpdateMethod: (value: import("../../../types/app").GithubUpdateMethod) => void;
  setTheme: (value: string) => void;
  setThemeMode: (value: ThemeModeKind) => void;
  setAutoTheme: (side: ThemeMode, value: string) => void;
  activateGeneratedTheme: () => void;
  updateGeneratedThemeControls: (patch: Partial<GeneratedThemeControls>) => void;
  resetGeneratedTheme: () => void;
  refreshModels: () => void;
  saveConfig: () => Promise<boolean> | boolean;
  updateRecordHotkey: (value: string) => Promise<boolean> | boolean;
  updateRecordBackgroundWakeEnabled: (value: boolean) => Promise<boolean> | boolean;
  restoreConfig: () => boolean;
  lastSavedConfigJson: string;
  onToolsChanged: () => void;
  addApiConfig: () => void;
  removeSelectedApiConfig: () => void;
  addPersona: () => void;
  removeSelectedPersona: () => void;
  resetPersonas: () => Promise<unknown> | unknown;
  savePersonas: () => Promise<boolean> | boolean;
  convertPrivatePersonaToPublic: (agentId: string) => Promise<boolean> | boolean;
  importPersonaMemories: (payload: { agentId: string; file: File }) => void;
  openConversationList: () => void;
  openConversationSummary: (conversationId: string) => void;
  openTrimActionDialog: () => void;
  openSettingsWindow: () => void;
  openPromptPreview: () => void;
  openSystemPromptPreview: () => void;
  openMemoryViewer: () => void;
  openRuntimeLogs: () => void;
  startHotkeyRecordTest: () => void;
  stopHotkeyRecordTest: () => void;
  playHotkeyRecordTest: () => void;
  requestMicrophonePermission: () => Promise<boolean> | boolean;
  captureHotkey: (value: string) => void;
  summonChatNow: () => void;
  saveAgentAvatar: (input: { agentId: string; mime: string; bytesBase64: string }) => void;
  clearAgentAvatar: (input: { agentId: string }) => void;
  updateChatInput: (value: string) => void;
  addChatMention: (value: ChatMentionTarget) => void;
  removeChatMention: (value: string | { agentId?: string; departmentId?: string }) => void;
  updateConversationPreferredApiConfigId: (value: string) => void;
  updatePlanModeEnabled: (value: boolean) => void;
  setSideConversationListVisible: (value: boolean) => void;
  setToolReviewPanelOpen: (value: boolean) => void;
  openChatReaderPanel: () => Promise<void>;
  setChatSidePanelWidths: (value: { leftWidth: number; rightWidth: number }, options?: { syncWindow?: boolean; commit?: boolean }) => void;
  updateConversationListTab: (value: "local" | "contact" | "task") => void;
  updateChatLeftPanelMode: (value: "local" | "contact" | "task") => void;
  updateChatRightPanelMode: (value: ChatRightPanelMode) => void;
  updateChatMonitorPanelMode: (value: ChatMonitorPanelMode) => void;
  removeClipboardImage: (index: number) => void;
  removeQueuedAttachmentNotice: (index: number) => void;
  pickAttachments: () => void;
  sendChat: () => void;
  stopChat: () => void;
  clearChatError: () => void;
  onLoadOlderChatHistory: () => void;
  onLoadOlderCompactionSegment: () => void;
  onReachedChatBottom: () => void;
  onJumpToConversationBottom: () => void;
  onCreateConversationBranchFromTurn: (payload: { turnId: string }) => void;
  onRecallTurn: (payload: { turnId: string }) => void;
  onRegenerateTurn: (payload: { turnId: string }) => void;
  requestRecallMode: (payload: {
    turnId: string;
    targetUserMessageId: string;
    conversationId?: string;
  }) => Promise<"with_patch" | "message_only" | "cancel">;
  confirmPlan: (payload: { messageId: string }) => void;
  onLockChatWorkspace: () => void;
  openGoalTaskDialog: () => void;
  closeGoalTaskDialog: () => void;
  saveGoalTask: (payload: { durationHours: number; goal: string; why: string; todo: string }) => void;
  stopGoalTask: () => void;
  onRefreshToolReviewMessage: (payload: { conversationId: string; messageId: string }) => void;
  onSwitchConversation: (payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact"; remoteContactId?: string }) => void;
  onRenameConversation: (payload: { conversationId: string; title: string }) => void;
  onToggleConversationPin: (conversationId: string) => void;
  onArchiveConversation: (conversationId: string) => void;
  onDeleteConversation: (conversationId: string) => void;
  onRebindConversationRecipient: (payload: { conversationId: string; departmentId: string; agentId: string }) => void;
  onUpdateDraftConversation: (payload: { conversationId: string; departmentId?: string; agentId?: string; preferredApiConfigId?: string | null; title?: string | null }) => void;
  onCreateConversation: (input?: { title?: string; departmentId?: string; agentId?: string; copyCurrent?: boolean; importPath?: string; shellWorkspaces?: ShellWorkspace[]; shellAutonomousMode?: boolean }) => void;
  onBranchConversationFromSelection: (payload: { count: number; messageIds: string[] }) => void;
  onBranchConversationFromCurrent: () => void;
  onForwardConversationFromSelection: (payload: { count: number; messageIds: string[]; target: { kind: "local_unarchived" | "remote_im_contact"; conversationId: string; remoteContactId?: string } }) => void;
  onUserAsyncDelegateFromSelection: (payload: { count: number; messageIds: string[]; departmentId: string; agentId: string; presetId: string; why: string; goal: string; todo: string }) => Promise<boolean> | boolean;
  loadArchives: () => void;
  selectArchive: (id: string) => void;
  selectArchiveBlock: (blockId?: number | null) => void;
  selectUnarchivedConversation: (id: string) => void;
  selectUnarchivedConversationBlock: (blockId?: number | null) => void;
  selectDelegateConversation: (id: string) => void;
  selectDelegateConversationBlock: (blockId?: number | null) => void;
  selectRemoteImContactConversation: (id: string) => void;
  selectRemoteImContactConversationBlock: (blockId?: number | null) => void;
  exportArchive: (payload: { format: "markdown" | "json" }) => void;
  importArchiveFile: (file: File) => void;
  unarchiveArchive: (id: string) => void;
  deleteArchive: (id: string) => void;
  deleteUnarchivedConversation: (id: string) => void;
  deleteDelegateConversation: (id: string) => void;
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
const sideChatSubscriptionSlot = createExclusiveChatViewSubscriptionSlot();
const memoryDialogVNodeRef: VNodeRef = (el) => {
  props.setMemoryDialogRef((el as Element | null) ?? null);
};

const promptPreviewDialogVNodeRef: VNodeRef = (el) => {
  props.setPromptPreviewDialogRef((el as Element | null) ?? null);
};

const chatViewRef = ref<{
  exitMessageSelectionMode: () => void;
  showTransientNotice: (text: string, tone?: "default" | "error" | "info") => void;
  openFileInReader: (path: string, line?: number) => Promise<void>;
  openDirectoryInReader: (path: string) => Promise<boolean>;
} | null>(null);

const sideChatTabs = computed(() => (props.sideConversations || []).map((conversation) => ({
  key: String(conversation.conversationId || "").trim(),
  label: String(conversation.title || "").trim() || props.t("chat.sideChat.title"),
  icon: MessageSquareMore,
  closeable: true,
})));

/** 右侧主页「追问」卡片数据：当前打开的追问会话 */
const sideChatItems = computed(() =>
  sideChatTabs.value.map((tab) => ({ id: String(tab.key || "").trim(), title: String(tab.label || "").trim() })),
);

/** 主页点某张追问小卡：切到追问面板并激活对应会话 */
function openSideChatFromHome(conversationId: string) {
  const id = String(conversationId || "").trim();
  if (!id) return;
  props.updateChatRightPanelMode("sideChat");
  props.selectSideChatConversation?.(id);
}

function closeSideChatTab(conversationId: string) {
  return props.closeSideChatConversations?.([conversationId]);
}

function closeSideChatTabsToLeft(conversationId: string) {
  const index = sideChatTabs.value.findIndex((tab) => tab.key === conversationId);
  if (index <= 0) return;
  return props.closeSideChatConversations?.(sideChatTabs.value.slice(0, index).map((tab) => tab.key));
}

function closeSideChatTabsToRight(conversationId: string) {
  const index = sideChatTabs.value.findIndex((tab) => tab.key === conversationId);
  if (index < 0 || index >= sideChatTabs.value.length - 1) return;
  return props.closeSideChatConversations?.(sideChatTabs.value.slice(index + 1).map((tab) => tab.key));
}

function closeOtherSideChatTabs(conversationId: string) {
  return props.closeSideChatConversations?.(
    sideChatTabs.value.filter((tab) => tab.key !== conversationId).map((tab) => tab.key),
  );
}

function showChatNotice(text: string, tone: "default" | "error" | "info" = "info") {
  chatViewRef.value?.showTransientNotice?.(text, tone);
}

async function openChatReaderFile(path: string, line?: number) {
  try {
    await props.openChatReaderPanel();
    const chatView = chatViewRef.value;
    if (!chatView) throw new Error("文件阅读面板尚未就绪");
    await chatView.openFileInReader(path, line);
  } catch (error) {
    showChatNotice(props.t("status.openLinkFailed", { err: String(error) }), "error");
  }
}

async function openChatReaderDirectory(path: string, line?: number) {
  try {
    await props.openChatReaderPanel();
    const chatView = chatViewRef.value;
    if (!chatView) throw new Error("文件阅读面板尚未就绪");
    const openedAsDirectory = await chatView.openDirectoryInReader(path);
    if (!openedAsDirectory) {
      // 无扩展名但不是目录（如 Makefile）：回退按文件打开
      await chatView.openFileInReader(path, line);
    }
  } catch (error) {
    showChatNotice(props.t("status.openLinkFailed", { err: String(error) }), "error");
  }
}

function commitChatSidePanelWidths(value: { leftWidth: number; rightWidth: number }) {
  props.setChatSidePanelWidths(value, { commit: true });
}

const selectionShareDialogOpen = ref(false);
const selectionShareDialogLoading = ref(false);
const selectionSharePayload = ref<SelectionSharePayload | null>(null);

async function exportConversationShare(conversationId: string) {
  const id = String(conversationId || "").trim();
  if (!id) return;
  try {
    const result = await invokeTauri<{ fileName: string; payloadJson: string }>("conversation.exportShare", {
      input: { conversationId: id },
    });
    const path = await saveTransportFileDialog({
      filters: [{ name: "JSON", extensions: ["json"] }],
      defaultPath: String(result?.fileName || "conversation.json").trim() || "conversation.json",
    });
    if (!path) return;
    await writeTransportUtf8TextFile(path, String(result?.payloadJson || ""));
    showChatNotice(props.t("chat.conversationShareExported", { path }), "info");
  } catch (error) {
    showChatNotice(props.t("chat.conversationShareExportFailed", { err: String(error) }), "error");
  }
}

async function handleUserAsyncDelegateFromSelection(payload: {
  count: number;
  messageIds: string[];
  departmentId: string;
  agentId: string;
  presetId: string;
  why: string;
  goal: string;
  todo: string;
}) {
  const ok = await props.onUserAsyncDelegateFromSelection(payload);
  if (ok) chatViewRef.value?.exitMessageSelectionMode();
}

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

function handleSelectionShareAction(payload: SelectionSharePayload) {
  if (!payload || payload.count <= 0 || !Array.isArray(payload.blocks) || payload.blocks.length === 0) {
    return;
  }
  selectionSharePayload.value = payload;
  if (payload.exportFormat === "html") {
    void exportSelectionAsHtml();
    return;
  }
  if (payload.exportFormat === "png") {
    void exportSelectionAsImage();
    return;
  }
  if (payload.exportFormat === "copyPng") {
    void copySelectionAsImage();
    return;
  }
  selectionShareDialogOpen.value = true;
}

function closeSelectionShareDialog() {
  if (selectionShareDialogLoading.value) return;
  selectionShareDialogOpen.value = false;
}

function resolveShareConversationId(payload: SelectionSharePayload): string {
  return String(
    payload.conversationId
    || props.currentChatConversationId
    || "",
  ).trim();
}

function resolveShareMessageIds(payload: SelectionSharePayload): string[] {
  const fromPayload = (payload.messageIds || []).map((id) => String(id || "").trim()).filter(Boolean);
  if (fromPayload.length > 0) return fromPayload;
  return (payload.blocks || [])
    .map((block) => String(block.sourceMessageId || block.id || "").trim())
    .filter(Boolean);
}

async function exportSelectionAsHtml() {
  const payload = selectionSharePayload.value;
  if (!payload || payload.count <= 0) return;
  const conversationId = resolveShareConversationId(payload);
  const messageIds = resolveShareMessageIds(payload);
  if (!conversationId || messageIds.length === 0) return;
  selectionShareDialogLoading.value = true;
  try {
    const path = await saveTransportFileDialog({
      filters: [{ name: "HTML", extensions: ["html"] }],
      defaultPath: buildShareExportFileName("html"),
    });
    if (!path) return;
    const generated = await generateShareFromMessageIds({
      conversationId,
      messageIds,
      formats: ["html"],
      title: props.t("chat.shareDocumentTitle"),
      subtitle: props.t("chat.shareDocumentSubtitle", { count: messageIds.length }),
      userAlias: props.userAlias,
      userAvatarUrl: props.userAvatarUrl,
      personaNameMap: props.chatPersonaNameMap,
      personaAvatarUrlMap: props.chatPersonaAvatarUrlMap,
      trigger: "selection_share_html",
    });
    if (!generated.html) {
      throw new Error(props.t("chat.shareExportFailed", { err: "empty html" }));
    }
    await writeTransportUtf8TextFile(path, generated.html);
    showChatNotice(props.t("chat.shareHtmlExported", { path }), "info");
    selectionShareDialogOpen.value = false;
  } catch (error) {
    showChatNotice(props.t("chat.shareExportFailed", { err: String(error) }), "error");
  } finally {
    selectionShareDialogLoading.value = false;
  }
}

async function generateSelectionSharePng(payload: SelectionSharePayload, trigger: string) {
  const conversationId = resolveShareConversationId(payload);
  const messageIds = resolveShareMessageIds(payload);
  if (!conversationId || messageIds.length === 0) {
    console.warn("[分享导出] 图片生成跳过：会话或消息为空", {
      conversationId,
      messageIdCount: messageIds.length,
    });
    throw new Error("conversationId/messageIds empty");
  }
  const generated = await generateShareFromMessageIds({
    conversationId,
    messageIds,
    formats: ["png"],
    title: props.t("chat.shareDocumentTitle"),
    subtitle: props.t("chat.shareDocumentSubtitle", { count: messageIds.length }),
    userAlias: props.userAlias,
    userAvatarUrl: props.userAvatarUrl,
    personaNameMap: props.chatPersonaNameMap,
    personaAvatarUrlMap: props.chatPersonaAvatarUrlMap,
    trigger,
  });
  const dataUrl = String(generated.pngDataUrl || "");
  const bytesBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
  console.info("[分享导出] 图片数据已生成", {
    trigger,
    dataUrlLength: dataUrl.length,
    base64Length: bytesBase64.length,
    usedCount: generated.usedMessageIds.length,
    skippedCount: generated.skippedMessageIds.length,
  });
  if (!dataUrl || !bytesBase64) {
    throw new Error(props.t("chat.shareImageGenerationFailed"));
  }
  return { dataUrl, bytesBase64 };
}

async function copySelectionAsImage() {
  const payload = selectionSharePayload.value;
  if (!payload || payload.count <= 0) {
    console.warn("[分享导出] 图片复制跳过：未找到选择载荷");
    return;
  }
  selectionShareDialogLoading.value = true;
  console.info("[分享导出] 图片复制开始", {
    messageIdCount: resolveShareMessageIds(payload).length,
  });
  try {
    if (!navigator.clipboard?.write || typeof ClipboardItem === "undefined") {
      throw new Error(props.t("chat.copyFailed"));
    }
    showChatNotice(props.t("chat.shareImageGenerating"), "info");
    const { dataUrl } = await generateSelectionSharePng(payload, "selection_share_copy_image");
    const response = await fetch(dataUrl);
    const blob = await response.blob();
    if (blob.type !== "image/png") {
      throw new Error(`unsupported clipboard image type: ${blob.type || "unknown"}`);
    }
    await navigator.clipboard.write([
      new ClipboardItem({ "image/png": blob }),
    ]);
    showChatNotice(props.t("chat.shareImageCopied"), "info");
    selectionShareDialogOpen.value = false;
  } catch (error) {
    console.error("[分享导出] 图片复制失败", error);
    showChatNotice(props.t("chat.shareExportFailed", { err: String(error) }), "error");
  } finally {
    selectionShareDialogLoading.value = false;
  }
}

async function exportSelectionAsImage() {
  const payload = selectionSharePayload.value;
  if (!payload || payload.count <= 0) {
    console.warn("[分享导出] 图片导出跳过：未找到选择载荷");
    return;
  }
  const conversationId = resolveShareConversationId(payload);
  const messageIds = resolveShareMessageIds(payload);
  if (!conversationId || messageIds.length === 0) {
    console.warn("[分享导出] 图片导出跳过：会话或消息为空", {
      conversationId,
      messageIdCount: messageIds.length,
    });
    showChatNotice(props.t("chat.shareExportFailed", { err: "conversationId/messageIds empty" }), "error");
    return;
  }
  selectionShareDialogLoading.value = true;
  console.info("[分享导出] 图片导出开始", {
    conversationId,
    messageIdCount: messageIds.length,
  });
  try {
    const path = await saveTransportFileDialog({
      filters: [{ name: "PNG", extensions: ["png"] }],
      defaultPath: buildShareExportFileName("png"),
    });
    if (!path) {
      console.info("[分享导出] 图片导出取消：未选择保存路径");
      return;
    }
    showChatNotice(props.t("chat.shareImageGenerating"), "info");
    console.info("[分享导出] 图片保存路径已选择", { path });
    const { bytesBase64 } = await generateSelectionSharePng(payload, "selection_share_image");
    showChatNotice(props.t("chat.shareImageWriting"), "info");
    await writeTransportBase64File(path, bytesBase64);
    showChatNotice(props.t("chat.shareImageExported", { path }), "info");
    selectionShareDialogOpen.value = false;
  } catch (error) {
    console.error("[分享导出] 图片导出失败", error);
    showChatNotice(props.t("chat.shareExportFailed", { err: String(error) }), "error");
  } finally {
    selectionShareDialogLoading.value = false;
  }
}
</script>
