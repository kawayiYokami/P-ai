
<template>
  <div class="window-shell text-sm bg-base-200">
    <AppWindowHeader
      v-if="!hideWindowHeader"
      :view-mode="viewMode"
      :current-theme="currentTheme"
      :title-text="titleText"
      :chat-usage-percent="chatUsagePercent"
      :current-persona-name="String(currentForegroundPersona?.name || '').trim() || t('archives.roleAssistant')"
      :side-conversation-list-visible="sideConversationListVisible"
      :tool-review-panel-open-visible="toolReviewPanelOpenVisible"
      :chat-side-panel-widths="chatSidePanelWidths"
      :active-conversation-id="currentChatConversationId"
      :conversation-items="chatConversationItems"
      :user-alias="userAlias"
      :user-avatar-url="userAvatarUrl"
      :persona-name-map="chatPersonaNameMap"
      :persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :trim-tip="t('chat.trimTip')"
      :maximized="maximized"
      :window-ready="windowReady"
      :open-settings-title="t('window.configTitle')"
      :close-title="t('common.close')"
      :config-search-query="configSearchQuery"
      :config-search-results="configSearchResults"
      :config-search-placeholder="t('config.search.placeholder')"
      :show-update-to-latest-button="showUpdateToLatestButton"
      :has-available-update="hasAvailableUpdate"
      :checking-update="checkingUpdate"
      :update-to-latest-label="updateToLatestLabel"
      :update-to-latest-title="updateToLatestTitle"
      @open-archives="openConversationList"
      @open-settings="openSettingsWindow"
      @minimize-window="minimizeWindowAndClearForeground"
      @toggle-maximize-window="toggleMaximizeWindow"
      @update:config-search-query="updateConfigSearchQuery"
      @select-config-search-result="handleSelectConfigSearchResult"
      @update-to-latest="triggerUpdateToLatest"
      @toggle-side-conversation-list="toggleSideConversationList"
      @toggle-tool-review-panel="toggleToolReviewPanel"
      @switch-conversation="switchChatConversation"
      @rename-conversation="renameCurrentConversation"
      @toggle-pin-conversation="toggleConversationPin"
      @archive-conversation="archiveConversationFromList"
      @delete-conversation="deleteUnarchivedConversationFromArchives"
      @create-conversation="openDraftConversationFromEntry"
      @trim-conversation="openTrimActionDialog"
      @start-drag="startDrag"
      @close-window="handleCloseWindow"
    />

    <AppWindowContent
      :t="tr"
      :view-mode="viewMode"
      :side-conversation-list-visible="sideConversationListVisible"
      :initial-tool-review-panel-open="toolReviewPanelOpenVisible"
      :conversation-list-tab="conversationListTab"
      :chat-left-panel-mode="chatLeftPanelMode"
      :chat-right-panel-mode="chatRightPanelMode"
      :chat-monitor-panel-mode="chatMonitorPanelMode"
      :side-conversations="sideConversations"
      :side-conversation-id="sideConversationId"
      :create-side-chat-conversation="createSideChatConversation"
      :open-side-chat-new-page="openSideChatNewPage"
      :select-side-chat-conversation="selectSideChatConversation"
      :create-side-conversation-branch-from-turn="createSideConversationBranchFromTurn"
      :close-side-chat-conversations="closeSideChatConversations"
      :config="config"
      :config-tab="configTab"
      :locale-options="localeOptions"
      :current-theme="currentTheme"
      :theme-mode="themeMode"
      :auto-light-theme="autoLightTheme"
      :auto-dark-theme="autoDarkTheme"
      :generated-theme-controls="generatedThemeControls"
      :generated-theme-tokens="generatedThemeTokens"
      :generated-light-tokens="generatedLightTokens"
      :generated-dark-tokens="generatedDarkTokens"
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
      :assistant-agent-id="assistantAgentId"
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
      :microphone-permission-state="microphonePermissionState"
      :microphone-permission-requesting="microphonePermissionRequesting"
      :user-alias="userAlias"
      :selected-persona-name="currentForegroundPersona?.name || t('archives.roleAssistant')"
      :current-chat-workspace-name="chatWorkspaceName"
      :current-chat-workspace-display-name="chatWorkspaceDisplayName"
      :current-chat-workspace-root-path="chatWorkspaceRootPath"
      :current-chat-workspace-autonomous-mode="chatWorkspaceAutonomousMode"
      :current-chat-workspaces="chatWorkspaceChoices"
      :current-chat-work-mode="chatWorkspaceWorkMode"
      :save-draft-workspaces="saveChatWorkspaces"
      :draft-workspace-git-root-check="checkChatWorkspaceGitRoot"
      :current-chat-agent-id="currentForegroundAgentId"
      :user-avatar-url="userAvatarUrl"
      :selected-persona-avatar-url="currentForegroundPersonaAvatarUrl"
      :chat-persona-name-map="chatPersonaNameMap"
      :chat-persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :chat-mention-entries="chatMentionEntries"
      :selected-chat-mentions="selectedChatMentions"
      :latest-user-text="latestUserText"
      :latest-user-images="latestUserImages"
      :frontend-round-phase="chatFlow.frontendRoundPhase.value"
      :submit-pending="chatFlow.submitPending.value"
      :chat-error-text="chatErrorText"
      :clipboard-images="clipboardImages"
      :queued-attachment-notices="queuedAttachmentNotices"
      :chat-input="chatInput"
      :instruction-presets="instructionPresets"
      :conversation-call-primary-api-config-id="currentForegroundApiConfigId"
      :preferred-chat-model-id="currentConversationPreferredApiConfigId"
      :tool-review-refresh-tick="toolReviewRefreshTick"
      :terminal-approvals="terminalApprovalConversationItems"
      :terminal-approval-resolving="terminalApprovalResolving"
      :approve-terminal-approval="approveTerminalApproval"
      :deny-terminal-approval="denyTerminalApproval"
      :approve-terminal-approval-for-session="approveTerminalApprovalForSession"
      :approve-terminal-approval-for-workspace="approveTerminalApprovalForWorkspace"
      :plan-mode-enabled="currentConversationPlanModeEnabled"
      :chat-usage-percent="chatUsagePercent"
      :trim-tip="t('chat.trimTip')"
      :media-drag-active="mediaDragActive"
      :chatting="chatting"
      :trimming="trimming"
      :trimming-conversation-id="trimmingConversationId"
      :compacting-conversation="compactingConversation"
      :compacting-conversation-id="compactingConversationId"
      :branching-conversation="branchingConversation"
      :forwarding-conversation-selection="forwardingConversationSelection"
      :visible-message-blocks="displayMessageBlocks"
      :chat-has-more-history="hasMoreBackendHistory"
      :chat-loading-older-history="loadingOlderConversationHistory"
      :latest-own-message-align-request="latestOwnMessageAlignRequest"
      :conversation-scroll-to-bottom-request="conversationScrollToBottomRequest"
      :scroll-to-bottom-behavior="scrollToBottomBehavior"
      :current-chat-conversation-id="currentChatConversationId"
      :current-chat-todos="currentChatTodos"
      :chat-goal-active="chatGoalActive"
      :chat-goal-title="chatGoalTitle"
      :goal-dialog-open="goalDialogOpen"
      :goal-saving="goalSaving"
      :goal-error="goalError"
      :active-goal-task="activeGoalTask"
      :recent-goal-task-history="recentGoalTaskHistory"
      :chat-unarchived-conversation-items="chatUnarchivedConversationItems"
      :chat-conversation-items="chatConversationItems"
      :create-conversation-agent-options="createConversationAgentOptions"
      :recipient-options-ready="startupDataReady"
      :default-create-conversation-agent-id="defaultCreateConversationAgentId"
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
      :message-text="messageText"
      :extract-message-images="extractMessageImages"
      :on-load-older-chat-history="loadOlderConversationHistory"
      :on-load-older-compaction-segment="loadOlderCompactionSegmentHistory"
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
      :prompt-preview-conversation-scope="promptPreviewConversationScope"
      :prompt-preview-conversation-id="promptPreviewConversationId"
      :prompt-preview-conversation-options="promptPreviewConversationOptions"
      :prompt-preview-dialog-open="promptPreviewDialogOpen"
      :mark-prompt-preview-dialog-closed="markPromptPreviewDialogClosed"
      :load-prompt-preview="loadPromptPreview"
      :select-prompt-preview-conversation-scope="selectPromptPreviewConversationScope"
      :select-prompt-preview-conversation="selectPromptPreviewConversation"
      :set-memory-dialog-ref="setMemoryDialogRef"
      :set-prompt-preview-dialog-ref="setPromptPreviewDialogRef"
      :set-status="setStatus"
      :attach-tool-review-report="attachToolReviewReport"
      :update-config-tab="(value) => { configTab = value; }"
      :set-ui-language="setUiLanguage"
      :update-persona-editor-id="updatePersonaEditorIdWithNotice"
      :update-selected-persona-id="updateAssistantAgentId"
      :update-selected-response-style-id="updateSelectedResponseStyleId"
      :update-selected-pdf-read-mode="updateSelectedPdfReadMode"
      :update-background-voice-screenshot-keywords="updateBackgroundVoiceScreenshotKeywords"
      :update-background-voice-screenshot-mode="updateBackgroundVoiceScreenshotMode"
      :update-instruction-presets="updateInstructionPresets"
      :patch-conversation-api-settings="patchConversationApiSettings"
      :patch-chat-settings="patchChatSettings"
      :update-ui-size-scale="updateUiSizeScale"
      :update-github-update-method="updateGithubUpdateMethod"
      :set-theme="setTheme"
      :set-theme-mode="setThemeMode"
      :set-auto-theme="setAutoTheme"
      :activate-generated-theme="activateGeneratedTheme"
      :update-generated-theme-controls="updateGeneratedThemeControls"
      :reset-generated-theme="resetGeneratedTheme"
      :refresh-models="refreshModels"
      :on-tools-changed="handleToolsChanged"
      :save-config="saveConfig"
      :update-record-hotkey="updateRecordHotkey"
      :update-record-background-wake-enabled="updateRecordBackgroundWakeEnabled"
      :restore-config="restoreLastSavedConfigSnapshot"
      :add-api-config="addApiConfig"
      :remove-selected-api-config="removeSelectedApiConfig"
      :add-persona="addPersona"
      :remove-selected-persona="removeSelectedPersona"
      :reset-personas="loadPersonas"
      :save-personas="savePersonas"
      :convert-private-persona-to-public="convertPrivatePersonaToPublic"
      :import-persona-memories="importPersonaMemories"
      :open-conversation-list="openConversationList"
      :open-conversation-summary="openConversationSummary"
      :open-trim-action-dialog="openTrimActionDialog"
      :open-settings-window="openSettingsWindow"
      :open-prompt-preview="openPromptPreview"
      :open-system-prompt-preview="openSystemPromptPreview"
      :open-memory-viewer="openMemoryViewer"
      :open-runtime-logs="openRuntimeLogsDialog"
      :start-hotkey-record-test="startHotkeyRecordTest"
      :stop-hotkey-record-test="stopHotkeyRecordTest"
      :play-hotkey-record-test="playHotkeyRecordTest"
      :request-microphone-permission="requestMicrophonePermission"
      :capture-hotkey="captureHotkey"
      :summon-chat-now="summonChatWindowFromConfig"
      :save-agent-avatar="saveAgentAvatar"
      :clear-agent-avatar="clearAgentAvatar"
      :update-chat-input="handleChatInputUpdate"
      :add-chat-mention="addChatMention"
      :remove-chat-mention="removeChatMention"
      :update-conversation-preferred-api-config-id="updateConversationPreferredApiConfigId"
      :update-plan-mode-enabled="updatePlanModeEnabled"
      :set-side-conversation-list-visible="handleSideConversationListVisibleChange"
      :set-tool-review-panel-open="handleToolReviewPanelOpenChange"
      :open-chat-reader-panel="openChatReaderPanel"
      :set-chat-side-panel-widths="handleChatSidePanelWidthsChange"
      :update-conversation-list-tab="updateConversationListTab"
      :update-chat-left-panel-mode="updateChatLeftPanelMode"
      :update-chat-right-panel-mode="updateChatRightPanelMode"
      :update-chat-monitor-panel-mode="updateChatMonitorPanelMode"
      :remove-clipboard-image="removeClipboardImage"
      :remove-queued-attachment-notice="removeQueuedAttachmentNotice"
      :pick-attachments="pickChatAttachments"
      :send-chat="sendChatFromCurrentWindow"
      :stop-chat="chatFlow.stopChat"
      :clear-chat-error="clearChatError"
      :on-jump-to-conversation-bottom="ensureLatestForegroundTailThenScrollToBottom"
      :on-create-conversation-branch-from-turn="handleCreateConversationBranchFromTurn"
      :open-goal-task-dialog="openGoalTaskDialog"
      :close-goal-task-dialog="closeGoalTaskDialog"
      :save-goal-task="saveGoalTask"
      :stop-goal-task="stopGoalTask"
      :on-reached-chat-bottom="() => undefined"
      :on-recall-turn="handleRecallTurn"
      :on-regenerate-turn="handleRegenerateTurn"
      :request-recall-mode="requestRecallMode"
      :confirm-plan="handleConfirmPlan"
      :on-lock-chat-workspace="openChatWorkspacePicker"
      :on-switch-conversation="switchChatConversation"
      :on-rename-conversation="renameCurrentConversation"
      :on-toggle-conversation-pin="toggleConversationPin"
      :on-archive-conversation="archiveConversationFromList"
      :on-delete-conversation="deleteUnarchivedConversationFromArchives"
      :on-rebind-conversation-recipient="rebindConversationRecipient"
      :on-update-draft-conversation="updateDraftConversation"
      :on-create-conversation="openDraftConversationFromEntry"
      :on-branch-conversation-from-selection="branchConversationFromSelection"
      :on-branch-conversation-from-current="branchConversationFromCurrent"
      :on-forward-conversation-from-selection="forwardConversationFromSelection"
      :on-user-async-delegate-from-selection="userAsyncDelegateFromSelection"
      :on-open-skill-panel="openSkillPlaceholderDialog"
      :load-archives="loadArchives"
      :select-archive="selectArchive"
      :select-archive-block="selectArchiveBlock"
      :select-unarchived-conversation="selectUnarchivedConversation"
      :select-unarchived-conversation-block="selectUnarchivedConversationBlock"
      :select-delegate-conversation="selectDelegateConversation"
      :select-delegate-conversation-block="selectDelegateConversationBlock"
      :select-remote-im-contact-conversation="selectRemoteImContactConversation"
      :select-remote-im-contact-conversation-block="selectRemoteImContactConversationBlock"
      :export-archive="exportArchive"
      :import-archive-file="prepareArchiveImport"
      :unarchive-archive="unarchiveArchive"
      :delete-archive="deleteArchive"
      :delete-unarchived-conversation="deleteUnarchivedConversation"
      :delete-delegate-conversation="deleteDelegateConversation"
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
      @open-code-review="notifySidebarCodeReview"
    />

    <ShellDialogsHost
      :update-dialog-open="updateDialogOpen"
      :update-dialog-title="updateDialogTitle"
      :update-dialog-body="updateDialogBody"
      :update-dialog-kind="updateDialogKind"
      :update-dialog-release-url="updateDialogReleaseUrl"
      :update-dialog-primary-action="updateDialogPrimaryAction"
      :update-progress-percent="updateProgressPercent"
      :update-dialog-skip-version-visible="updateDialogSkipVersionVisible"
      :update-dialog-cancel-update-visible="updateDialogCancelUpdateVisible"
      :update-dialog-cancel-pending="updateCancelPending"
      :portable-pending="portablePending"
      :is-portable-pending="!!portablePending"
      :runtime-logs-dialog-open="runtimeLogsDialogOpen"
      :runtime-logs="runtimeLogs"
      :runtime-logs-loading="runtimeLogsLoading"
      :runtime-logs-error="runtimeLogsError"
      :rewind-confirm-dialog-open="rewindConfirmDialogOpen"
      :rewind-confirm-can-undo-patch="rewindConfirmCanUndoPatch"
      :branch-from-message-confirm-dialog-open="branchFromMessageConfirmDialogOpen"
      :config-save-error-dialog-open="configSaveErrorDialogOpen"
      :config-save-error-dialog-title="configSaveErrorDialogTitle"
      :config-save-error-dialog-body="configSaveErrorDialogBody"
      :config-save-error-dialog-kind="configSaveErrorDialogKind"
      :archive-import-preview-dialog-open="archiveImportPreviewDialogOpen"
      :archive-import-preview="archiveImportPreview"
      :archive-import-running="archiveImportRunning"
      :skill-placeholder-dialog-open="skillPlaceholderDialogOpen"
      :trim-action-dialog-open="trimActionDialogOpen"
      :trim-preview-loading="trimPreviewLoading"
      :trim-preview="trimPreview"
      :trim-compaction-preview="trimCompactionPreview"
      :trimming="trimming"
      @close-update-dialog="closeUpdateDialog"
      @confirm-update-dialog-primary="confirmUpdateDialogPrimary"
      @open-update-release="openUpdateRelease"
      @open-update-repository="openGithubRepository"
      @skip-update-version="skipCurrentUpdateVersion"
      @cancel-update="cancelGithubUpdate"
      @open-portable-pending-dir="openPortablePendingDir"
      @retry-portable-pending="retryPortablePending"
      @dismiss-portable-pending="dismissPortablePending"
      @close-runtime-logs-dialog="closeRuntimeLogsDialog"
      @refresh-runtime-logs="refreshRuntimeLogs"
      @clear-runtime-logs="clearRuntimeLogs"
      @confirm-rewind-with-patch="confirmRewindWithPatch"
      @confirm-rewind-message-only="confirmRewindMessageOnly"
      @cancel-rewind-confirm="cancelRewindConfirm"
      @confirm-branch-from-message="confirmBranchFromMessage"
      @cancel-branch-from-message-confirm="cancelBranchFromMessageConfirm"
      @close-settings-save-error-dialog="closeSettingsSaveErrorDialog"
      @close-archive-import-preview-dialog="closeArchiveImportPreviewDialog"
      @confirm-archive-import="confirmArchiveImport"
      @close-skill-placeholder-dialog="closeSkillPlaceholderDialog"
      @confirm-trim-compaction-action="confirmTrimCompactionAction"
      @confirm-trim-action="handleConfirmTrimAction()"
      @confirm-trim-delete-action="confirmTrimDeleteAction"
      @close-trim-action-dialog="closeTrimActionDialog"
    />
    <Win10ResizeHandles :enabled="resizeHandlesEnabled" />
    <ChatWorkspacePickerDialog
      :open="chatWorkspacePickerOpen"
      :saving="chatWorkspacePickerSaving"
      :workspaces="chatWorkspaceDraftChoices"
      :autonomous-mode="chatWorkspaceDraftAutonomousMode"
      :work-mode="chatWorkspaceDraftWorkMode"
      :selected-branch="chatWorkspaceDraftBranch"
      :worktree-path="chatWorkspaceWorktreePath"
      :worktree-exists="chatWorkspaceWorktreeExists"
      :worktree-available="chatWorkspaceWorktreeAvailable"
      :worktree-check-message="chatWorkspaceWorktreeCheckMessage"
      :validation-message="chatWorkspaceDraftError"
      @close="closeChatWorkspacePicker"
      @add-workspace="addChatWorkspace"
      @add-secondary="addSecondaryPath"
      @remove-secondary="removeSecondaryPath"
      @update-main-path="setMainPath"
      @set-main="setChatWorkspaceAsMain"
      @set-access="setChatWorkspaceAccessLegacy"
      @set-access-unified="setChatWorkspaceAccess"
      @set-branch="setChatWorkspaceBranch"
      @set-autonomous-mode="setChatWorkspaceAutonomousMode"
      @set-work-mode="setChatWorkspaceWorkMode"
      @remove-workspace="removeChatWorkspace"
      @open-dir="openChatWorkspaceDir"
      @save="saveChatWorkspacePicker"
    />
    <StartupOverlay v-if="startupOverlayVisible" />
    <ConfigStatusToast :text="statusToastText" :tone="statusToastTone" />
    <div
      v-if="messageStoreMigration.visible"
      class="fixed inset-0 z-9999 flex items-center justify-center bg-base-300/90 p-6 backdrop-blur"
    >
      <div class="w-full max-w-2xl rounded-box border border-base-content/10 bg-base-100 p-6 shadow-2xl">
        <div class="text-xl font-semibold">{{ t("config.messageStoreMigration.title") }}</div>
        <div class="mt-2 text-sm opacity-70">{{ messageStoreMigration.message }}</div>
        <progress
          v-if="messageStoreMigration.mode === 'migrating' || messageStoreMigration.mode === 'waiting'"
          class="progress progress-primary mt-5 w-full"
          :value="messageStoreMigration.current"
          :max="Math.max(messageStoreMigration.total, 1)"
        />
        <div v-if="messageStoreMigration.mode === 'migrating'" class="mt-2 text-xs opacity-60">
          {{ messageStoreMigration.current }} / {{ messageStoreMigration.total }}
        </div>
        <div v-if="messageStoreMigration.mode === 'completed'" class="mt-2 text-xs opacity-60">
          {{ t("config.messageStoreMigration.migratedCount") }}: {{ messageStoreMigration.migratedCount }}
          · {{ t("config.messageStoreMigration.discardedCount") }}: {{ messageStoreMigration.discardedCount }}
        </div>
        <div v-if="messageStoreMigration.mode === 'completed'" class="mt-5 flex justify-end">
          <button class="btn btn-primary" @click="confirmMessageStoreMigrationSummary">
            {{ t("config.messageStoreMigration.confirm") }}
          </button>
        </div>
        <div v-if="messageStoreMigration.mode === 'error'" class="mt-5 flex justify-end">
          <button class="btn btn-primary" @click="retryMessageStoreMigration">
            {{ t("config.messageStoreMigration.retry") }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent, onBeforeUnmount } from "vue";
import ConfigStatusToast from "./features/config/components/ConfigStatusToast.vue";
import Win10ResizeHandles from "./features/shell/components/Win10ResizeHandles.vue";
import ChatWorkspacePickerDialog from "./features/chat/components/dialogs/ChatWorkspacePickerDialog.vue";
import AppWindowContent from "./features/shell/components/AppWindowContent.vue";
import AppWindowHeader from "./features/shell/components/AppWindowHeader.vue";
import ShellDialogsHost from "./features/shell/components/ShellDialogsHost.vue";
import StartupOverlay from "./features/shell/components/StartupOverlay.vue";
import { useChatWindowApp } from "./features/chat/composables/use-chat-window-app";
import { onTransportRemoteChatCommand, transportDraftHostWorkspaces } from "./services/tauri-api";

/** iframe 嵌入且非 VSCode 宿主时隐藏窗口栏：远程前端模式下由宿主壳层提供 header。 */
function isEmbeddedWebHost(): boolean {
  if (typeof window === "undefined") return false;
  if (window.self === window.top) return false;
  const bridgeWindow = window as Window & { acquireVsCodeApi?: unknown };
  const isVscodeHost =
    typeof bridgeWindow.acquireVsCodeApi === "function"
    || window.location.protocol === "vscode-webview:";
  return !isVscodeHost;
}

export default defineComponent({
  name: "ChatWindowApp",
  components: {
    Win10ResizeHandles,
    ChatWorkspacePickerDialog,
    AppWindowContent,
    AppWindowHeader,
    ShellDialogsHost,
    StartupOverlay,
    ConfigStatusToast,
  },
  setup() {
    const app = useChatWindowApp();
    const embedded = isEmbeddedWebHost();

    // 打开草稿会话：从文件夹分节新建时带该文件夹；标题栏/composer 新建时带当前会话工作区
    function openDraftConversationFromEntry(payload?: { workspaceRootPath?: string } | Record<string, unknown>) {
      const workspaceRootPath = String((payload as { workspaceRootPath?: string } | undefined)?.workspaceRootPath || "").trim();
      if (workspaceRootPath) {
        const name = workspaceRootPath.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || workspaceRootPath;
        void app.openDraftConversation({
          shellWorkspaces: [{
            id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
            name,
            path: workspaceRootPath,
            level: "main",
            access: "approval",
            builtIn: false,
          }],
          shellWorkMode: "directory",
          shellAutonomousMode: false,
        });
        return;
      }
      // VS Code 侧边栏等 Web 宿主：顶栏新建默认落在宿主打开的工作区，
      // 而非当前会话工作区；桌面端或宿主未注入工作区时维持原语义。
      const hostWorkspaces = transportDraftHostWorkspaces();
      if (hostWorkspaces.length > 0) {
        void app.openDraftConversation({
          shellWorkspaces: hostWorkspaces,
          shellWorkMode: "directory",
          shellAutonomousMode: false,
        });
        return;
      }
      void app.openDraftConversation({
        shellWorkspaces: (app.chatWorkspaceChoices?.value || []).map((item) => ({
          id: item.id,
          name: item.name,
          path: item.path,
          level: item.level,
          access: item.access,
          builtIn: false,
        })),
        shellWorkMode: app.chatWorkspaceWorkMode?.value,
        shellAutonomousMode: app.chatWorkspaceAutonomousMode?.value,
      });
    }

    // 远程前端模式：手机壳层 header 的会话操作（切换对话列表/新建对话）通过
    // postMessage 转发到这里执行，由电脑 PAI 页面在自己的会话状态上完成操作。
    // 监听与安全校验统一收敛在 tauri-api 的 onTransportRemoteChatCommand。
    if (embedded) {
      const stopRemoteCommands = onTransportRemoteChatCommand((method) => {
        if (method === "toggle-conversation-list") {
          void app.toggleSideConversationList();
        } else if (method === "create-conversation") {
          openDraftConversationFromEntry();
        }
      });
      onBeforeUnmount(stopRemoteCommands);
    }

    return {
      ...app,
      hideWindowHeader: embedded,
      openDraftConversationFromEntry,
    };
  },
});
</script>
