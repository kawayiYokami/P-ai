import { computed } from "vue";
import { formatI18nError } from "../../../utils/error";
import { useChatConversationActionsOrchestrator } from "./use-chat-conversation-actions-orchestrator";
import { useChatConversationDialogGlue } from "./use-chat-conversation-dialog-glue";
import { useChatConversationSync } from "./use-chat-conversation-sync";
import { useChatForegroundOrchestrator } from "./use-chat-foreground-orchestrator";
import { useChatRemoteConversationOrchestrator } from "./use-chat-remote-conversation-orchestrator";

export function useChatWindowConversationOrchestrator(bindings: Record<string, any>) {
  const {
    matchesForegroundConversation,
    formalizeConversationMessages,
    freezeConversationMessages,
    insertMessagesBeforeStreamingAssistantProjection,
    mergeMessagesIntoTimeline,
    currentConversationRuntimeState,
    maybeResumeForegroundStreamingBubble,
    conversationRuntimeSnapshotIsBusy,
    requestConversationRuntimeSnapshot,
    resumeForegroundRuntimeFromBackend,
    areMessagesEquivalent,
    messageContentSignature,
    reuseStableMessageReferences,
    beginForegroundPaintTrace,
    logForegroundPaintTrace,
    cacheConversationMessages,
    inferHasMoreHistoryFromSnapshot,
    clearConversationBadge,
    markConversationReadPersisted,
    setConversationBadge,
    readConversationIdFromPayload,
    readMessagesFromPayload,
    mergeIncomingMessagesIntoCache,
    buildConversationMessagesAfterAnchor,
    requestConversationMessagesAfterAsync,
    requestConversationMessageById,
    replaceConversationMessage,
    reloadForegroundConversationMessages,
    refreshForegroundConversationMessageById,
    loadOlderConversationHistory,
    loadOlderCompactionSegmentHistory,
    mergeConversationMessagesFromSyncPayload,
    applyConversationMessagesAfterSynced,
    applyConversationMessageAppended,
    applyConversationSnapshot,
    applyConversationTodosUpdated,
    applyConversationOverviewUpdated: applyConversationOverviewUpdatedRaw,
    applyConversationOverviewItemUpdated,
    applyConversationPinUpdated,
    applyConversationRuntimeStateUpdated,
    unarchivedConversationActivityAt,
    sortUnarchivedConversationOverviewItems,
  } = useChatConversationSync(bindings.sync);

  const chatForeground = useChatForegroundOrchestrator({
    FOREGROUND_SNAPSHOT_RECENT_LIMIT: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    BACKGROUND_CONVERSATION_CACHE_LIMIT: bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT,
    config: bindings.config,
    currentChatConversationId: bindings.currentChatConversationId,
    currentChatPreferredApiConfigId: bindings.currentChatPreferredApiConfigId,
    currentChatTodos: bindings.currentChatTodos,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    currentForegroundConversationSummary: bindings.currentForegroundConversationSummary,
    unarchivedConversations: bindings.unarchivedConversations,
    remoteImContactConversations: bindings.remoteImContactConversations,
    conversationForegroundSyncing: bindings.conversationForegroundSyncing,
    trimmingConversationId: bindings.trimmingConversationId,
    compactingConversationId: bindings.compactingConversationId,
    trimming: bindings.trimming,
    compactingConversation: bindings.compactingConversation,
    chatting: bindings.chatting,
    sideConversationListVisible: bindings.sideConversationListVisible,
    allMessages: bindings.allMessages,
    hasMoreBackendHistory: bindings.hasMoreBackendHistory,
    foregroundTailLatestReady: bindings.foregroundTailLatestReady,
    chatWorkspaceName: bindings.chatWorkspaceName,
    cacheConversationMessages,
    clearConversationBadge,
    markConversationReadPersisted,
    beginForegroundPaintTrace,
    logForegroundPaintTrace,
    requestConversationRuntimeSnapshot,
    applyConversationSnapshot,
    resumeForegroundRuntimeFromBackend,
    maybeResumeForegroundStreamingBubble,
    buildConversationMessagesAfterAnchor,
    clearPendingManualScrollToBottom: bindings.clearPendingManualScrollToBottom,
    triggerConversationScrollToBottom: bindings.triggerConversationScrollToBottom,
    setPendingManualScrollState: bindings.setPendingManualScrollState,
    waitPendingConversationPreferredModelPersist: bindings.waitPendingConversationPreferredModelPersist,
    setConversationChatErrorText: bindings.setConversationChatErrorText,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    perfNow: bindings.perfNow,
    isChatWindowActiveNow: bindings.isChatWindowActiveNow,
    closeWindow: bindings.closeWindow,
    freezeForegroundConversation: bindings.freezeForegroundConversation,
    getChatFlow: bindings.getChatFlow,
  });

  function applyConversationOverviewUpdated(payload?: Record<string, any> | null) {
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const previousCurrentItem = currentConversationId
      ? bindings.unarchivedConversations.value.find(
        (item: any) => String(item.conversationId || "").trim() === currentConversationId,
      )
      : null;
    applyConversationOverviewUpdatedRaw(payload);
    if (!currentConversationId) return;
    const stillVisible = bindings.unarchivedConversations.value.some(
      (item: any) => String(item.conversationId || "").trim() === currentConversationId,
    );
    if (stillVisible) return;
    if (previousCurrentItem) {
      bindings.unarchivedConversations.value = sortUnarchivedConversationOverviewItems([
        previousCurrentItem,
        ...bindings.unarchivedConversations.value.filter(
          (item: any) => String(item.conversationId || "").trim() !== currentConversationId,
        ),
      ]);
      return;
    }
    const preferredConversationId = String(payload?.preferredConversationId || "").trim() || null;
    void chatForeground.recoverForegroundConversationFromOverview(
      "conversation_overview_updated_missing_current",
      preferredConversationId,
    ).catch((error: unknown) => {
      bindings.setStatusError("status.loadMessagesFailed", error);
    });
  }

  const chatRemoteConversation = useChatRemoteConversationOrchestrator({
    remoteImContactConversations: bindings.remoteImContactConversations,
    currentChatConversationId: bindings.currentChatConversationId,
    currentChatTodos: bindings.currentChatTodos,
    conversationForegroundSyncing: bindings.conversationForegroundSyncing,
    allMessages: bindings.allMessages,
    hasMoreBackendHistory: bindings.hasMoreBackendHistory,
    foregroundTailLatestReady: bindings.foregroundTailLatestReady,
    conversationMessageCache: bindings.conversationMessageCache,
    cacheConversationMessages,
    clearConversationBadge,
    markConversationReadPersisted,
    getChatFlow: bindings.getChatFlow,
    clearPendingManualScrollToBottom: bindings.clearPendingManualScrollToBottom,
    freezeConversationMessages,
    reuseStableMessageReferences,
    refreshUnarchivedConversationOverview: chatForeground.refreshUnarchivedConversationOverview,
    syncUnarchivedConversationOverviewChangedSinceWatermark: chatForeground.syncUnarchivedConversationOverviewChangedSinceWatermark,
    refreshRemoteImConversationOverview: chatForeground.refreshRemoteImConversationOverview,
    switchUnarchivedConversation: chatForeground.switchUnarchivedConversation,
    setStatusError: bindings.setStatusError,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
  });

  const chatConversationActions = useChatConversationActionsOrchestrator({
    t: bindings.t,
    tr: bindings.tr,
    currentChatConversationId: bindings.currentChatConversationId,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    createConversationAgentOptions: bindings.createConversationAgentOptions,
    defaultCreateConversationAgentId: bindings.defaultCreateConversationAgentId,
    unarchivedConversations: bindings.unarchivedConversations,
    branchingConversation: bindings.branchingConversation,
    forwardingConversationSelection: bindings.forwardingConversationSelection,
    trimming: bindings.trimming,
    refreshUnarchivedConversationOverview: chatForeground.refreshUnarchivedConversationOverview,
    syncUnarchivedConversationOverviewChangedSinceWatermark: chatForeground.syncUnarchivedConversationOverviewChangedSinceWatermark,
    switchUnarchivedConversation: chatForeground.switchUnarchivedConversation,
    refreshChatWorkspaceState: bindings.refreshChatWorkspaceState,
    requestConversationLightSnapshot: chatForeground.requestConversationLightSnapshot,
    applyConversationOverviewItemUpdated,
    applyConversationSnapshot,
    applyConversationPinUpdated,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    formatRequestFailed: (error: unknown) => formatI18nError(bindings.tr, "status.requestFailed", error),
  });

  const chatConversationDialogGlue = useChatConversationDialogGlue({
    currentChatConversationId: bindings.currentChatConversationId,
    currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    unarchivedConversations: bindings.unarchivedConversations,
    conversationForegroundSyncing: bindings.conversationForegroundSyncing,
    deleteUnarchivedConversationFromArchivesRaw: bindings.deleteUnarchivedConversationFromArchivesRaw,
    syncUnarchivedConversationOverviewChangedSinceWatermark: chatForeground.syncUnarchivedConversationOverviewChangedSinceWatermark,
    requestConversationLightSnapshot: chatForeground.requestConversationLightSnapshot,
    applyConversationSnapshot,
    pickForegroundConversationId: chatForeground.pickForegroundConversationId,
    clearForegroundConversation: chatForeground.clearForegroundConversation,
    recoverForegroundConversationFromOverview: chatForeground.recoverForegroundConversationFromOverview,
    switchUnarchivedConversation: chatForeground.switchUnarchivedConversation,
    archiveCurrentConversation: bindings.archiveCurrentConversation,
    getOpenTrimActionDialog: () => bindings.openTrimActionDialog,
    getConfirmTrimAction: () => bindings.confirmTrimAction,
    getCloseTrimActionDialog: () => bindings.closeTrimActionDialog,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
  });

  async function refreshChatUnarchivedConversations() {
    await chatForeground.refreshChatUnarchivedConversations();
  }

  async function syncUnarchivedConversationOverviewChangedSinceWatermark(reason?: string) {
    await chatForeground.syncUnarchivedConversationOverviewChangedSinceWatermark(reason);
  }

  async function sendChatFromCurrentWindow(overrides?: { extraTextBlocks?: string[] }) {
    await chatForeground.sendChatFromCurrentWindow(overrides);
  }

  function freezeForegroundConversation(reason: string) {
    chatForeground.freezeForegroundConversation(reason);
  }

  async function restoreForegroundConversationProjection(conversationId: string, reason: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    void reason;
    await chatForeground.switchUnarchivedConversation(cid);
  }

  async function deleteUnarchivedConversationFromArchives(conversationId: string) {
    await chatConversationDialogGlue.deleteUnarchivedConversationFromArchives(conversationId);
  }

  return {
    matchesForegroundConversation,
    formalizeConversationMessages,
    freezeConversationMessages,
    insertMessagesBeforeStreamingAssistantProjection,
    mergeMessagesIntoTimeline,
    currentConversationRuntimeState,
    maybeResumeForegroundStreamingBubble,
    conversationRuntimeSnapshotIsBusy,
    requestConversationRuntimeSnapshot,
    resumeForegroundRuntimeFromBackend,
    areMessagesEquivalent,
    messageContentSignature,
    reuseStableMessageReferences,
    beginForegroundPaintTrace,
    logForegroundPaintTrace,
    cacheConversationMessages,
    inferHasMoreHistoryFromSnapshot,
    clearConversationBadge,
    markConversationReadPersisted,
    setConversationBadge,
    readConversationIdFromPayload,
    readMessagesFromPayload,
    mergeIncomingMessagesIntoCache,
    buildConversationMessagesAfterAnchor,
    requestConversationMessagesAfterAsync,
    requestConversationMessageById,
    replaceConversationMessage,
    reloadForegroundConversationMessages,
    refreshForegroundConversationMessageById,
    loadOlderConversationHistory,
    loadOlderCompactionSegmentHistory,
    mergeConversationMessagesFromSyncPayload,
    applyConversationMessagesAfterSynced,
    applyConversationMessageAppended,
    applyConversationSnapshot,
    applyConversationTodosUpdated,
    applyConversationOverviewUpdated,
    applyConversationOverviewItemUpdated,
    applyConversationPinUpdated,
    applyConversationRuntimeStateUpdated,
    unarchivedConversationActivityAt,
    sortUnarchivedConversationOverviewItems,
    pickForegroundConversationId: chatForeground.pickForegroundConversationId,
    clearForegroundConversation: chatForeground.clearForegroundConversation,
    handleCloseWindow: chatForeground.handleCloseWindow,
    hasActiveForegroundConversation: chatForeground.hasActiveForegroundConversation,
    requestConversationLightSnapshot: chatForeground.requestConversationLightSnapshot,
    refreshRemoteImConversationOverview: chatForeground.refreshRemoteImConversationOverview,
    refreshUnarchivedConversationOverview: chatForeground.refreshUnarchivedConversationOverview,
    syncUnarchivedConversationOverviewChangedSinceWatermark,
    recoverForegroundConversationFromOverview: chatForeground.recoverForegroundConversationFromOverview,
    syncCurrentConversationWorkspaceLabel: chatForeground.syncCurrentConversationWorkspaceLabel,
    switchUnarchivedConversation: chatForeground.switchUnarchivedConversation,
    ensureLatestForegroundTailThenScrollToBottom: chatForeground.ensureLatestForegroundTailThenScrollToBottom,
    refreshChatUnarchivedConversations,
    sendChatFromCurrentWindow,
    freezeForegroundConversation,
    restoreForegroundConversationProjection,
    switchRemoteImContactConversation: chatRemoteConversation.switchRemoteImContactConversation,
    switchChatConversation: chatRemoteConversation.switchChatConversation,
    createUnarchivedConversation: chatConversationActions.createUnarchivedConversation,
    openDraftConversation: chatConversationActions.openDraftConversation,
    updateDraftConversation: chatConversationActions.updateDraftConversation,
    createSideChatConversation: chatConversationActions.createSideChatConversation,
    branchConversationFromSelection: chatConversationActions.branchConversationFromSelection,
    branchConversationFromCurrent: chatConversationActions.branchConversationFromCurrent,
    forwardConversationFromSelection: chatConversationActions.forwardConversationFromSelection,
    userAsyncDelegateFromSelection: chatConversationActions.userAsyncDelegateFromSelection,
    renameCurrentConversation: chatConversationActions.renameCurrentConversation,
    toggleConversationPin: chatConversationActions.toggleConversationPin,
    archiveConversationFromList: chatConversationDialogGlue.archiveConversationFromList,
    handleConfirmTrimAction: chatConversationDialogGlue.handleConfirmTrimAction,
    deleteUnarchivedConversationFromArchives,
  };
}
