import { formatI18nError } from "../../../utils/error";
import { removeBinaryPlaceholders } from "../../../utils/chat-message";
import { invokeTauri } from "../../../services/tauri-api";
import { useChatRuntimeSetup } from "./use-chat-runtime-setup";
import { useChatServiceAssemblies } from "./use-chat-service-assemblies";
import { useChatWindowConversationOrchestrator } from "./use-chat-window-conversation-orchestrator";

export function useChatWindowRuntimeOrchestrator(bindings: Record<string, any>) {
  let openSettingsSaveErrorDialog: (...args: any[]) => void = (..._args: unknown[]) => {};

  function setConversationRuntimeStateLocal(
    conversationId: string,
    runtimeState: "idle" | "assistant_streaming" | "organizing_context",
  ) {
    bindings.conversationApi.applyConversationRuntimeStateUpdated({
      conversationId,
      runtimeState,
    });
  }

  const serviceAssemblies = useChatServiceAssemblies({
    tr: bindings.tr,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    personas: bindings.personas,
    assistantAgentId: bindings.assistantAgentId,
    avatarSaving: bindings.avatarSaving,
    avatarError: bindings.avatarError,
    selectedApiConfig: bindings.selectedApiConfig,
    selectedApiProvider: bindings.selectedApiProvider,
    refreshingModels: bindings.refreshingModels,
    modelRefreshError: bindings.modelRefreshError,
    apiModelOptions: bindings.apiModelOptions,
    modelRefreshOkFlags: bindings.modelRefreshOkFlags,
    toolApiConfig: bindings.toolApiConfig,
    checkingToolsStatus: bindings.checkingToolsStatus,
    toolStatuses: bindings.toolStatuses,
    ensureAvatarCached: bindings.ensureAvatarCached,
    openSettingsSaveErrorDialog: (...args: unknown[]) => openSettingsSaveErrorDialog(...args),
    config: bindings.config,
    locale: bindings.locale,
    normalizeLocale: bindings.normalizeLocale,
    chatUsagePercent: bindings.chatUsagePercent,
    suppressAutosave: bindings.suppressAutosave,
    loading: bindings.loading,
    saving: bindings.saving,
    personaSaving: bindings.personaSaving,
    assistantPersonas: bindings.assistantPersonas,
    personaEditorId: bindings.personaEditorId,
    userAlias: bindings.userAlias,
    selectedResponseStyleId: bindings.selectedResponseStyleId,
    selectedPdfReadMode: bindings.selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords: bindings.backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode: bindings.backgroundVoiceScreenshotMode,
    instructionPresets: bindings.instructionPresets,
    responseStyleIds: bindings.responseStyleIds,
    createApiConfig: bindings.createApiConfig,
    normalizeApiBindingsLocal: bindings.normalizeApiBindingsLocal,
    buildConfigPayload: bindings.buildConfigPayload,
    buildConfigSnapshotJson: bindings.buildConfigSnapshotJson,
    buildPersonasSnapshotJson: bindings.buildPersonasSnapshotJson,
    lastSavedConfigJson: bindings.lastSavedConfigJson,
    lastSavedPersonasJson: bindings.lastSavedPersonasJson,
    syncUserAliasFromPersona: bindings.syncUserAliasFromPersona,
    preloadPersonaAvatars: bindings.preloadPersonaAvatars,
    selectedPersonaEditor: bindings.selectedPersonaEditor,
    createApiProvider: bindings.createApiProvider,
    chatErrorText: bindings.chatErrorText,
    setConversationRuntimeState: setConversationRuntimeStateLocal,
    currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    currentChatConversationId: bindings.currentChatConversationId,
    trimmingConversationId: bindings.trimmingConversationId,
    compactingConversationId: bindings.compactingConversationId,
    chatting: bindings.chatting,
    trimming: bindings.trimming,
    compactingConversation: bindings.compactingConversation,
    suppressNextCompactionReload: bindings.suppressNextCompactionReload,
    allMessages: bindings.allMessages,
    refreshChatUnarchivedConversations: bindings.refreshChatUnarchivedConversations,
    perfNow: bindings.perfNow,
    perfLog: bindings.perfLog,
    PERF_DEBUG: bindings.PERF_DEBUG,
    configTab: bindings.configTab,
    unarchivedConversations: bindings.unarchivedConversations,
    delegateConversations: bindings.delegateConversations,
    remoteImContactConversations: bindings.remoteImContactConversations,
    deleteUnarchivedConversationFromArchives: bindings.deleteUnarchivedConversationFromArchives,
  });

  const {
    chatRuntime,
    shellDialogFlows,
  } = serviceAssemblies;
  const { loadAllMessages } = chatRuntime;

  // Config persistence reports save errors through shell dialogs, but the shell
  // dialog flow is assembled in the same service graph. Keep this late-bound
  // edge explicit so the initialization order is visible at the call site.
  openSettingsSaveErrorDialog = shellDialogFlows.openSettingsSaveErrorDialog;

  const conversationOrchestrator = useChatWindowConversationOrchestrator({
    FOREGROUND_SNAPSHOT_RECENT_LIMIT: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    BACKGROUND_CONVERSATION_CACHE_LIMIT: bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT,
    OLDER_HISTORY_PAGE_SIZE: bindings.OLDER_HISTORY_PAGE_SIZE,
    sync: {
      BACKGROUND_CONVERSATION_CACHE_LIMIT: bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT,
      OLDER_HISTORY_PAGE_SIZE: bindings.OLDER_HISTORY_PAGE_SIZE,
      currentChatConversationId: bindings.currentChatConversationId,
      currentChatPreferredApiConfigId: bindings.currentChatPreferredApiConfigId,
      currentChatTodos: bindings.currentChatTodos,
      currentForegroundAgentId: bindings.currentForegroundAgentId,
      currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
      unarchivedConversations: bindings.unarchivedConversations,
      lastOverviewSyncAt: bindings.lastOverviewSyncAt,
      remoteImContactConversations: bindings.remoteImContactConversations,
      allMessages: bindings.allMessages,
      hasMoreBackendHistory: bindings.hasMoreBackendHistory,
      loadingOlderConversationHistory: bindings.loadingOlderConversationHistory,
      foregroundTailLatestReady: bindings.foregroundTailLatestReady,
      conversationMessageCache: bindings.conversationMessageCache,
      backgroundConversationBadgeMap: bindings.backgroundConversationBadgeMap,
      ensureConversationMessageIds: bindings.ensureConversationMessageIds,
      clearPendingManualScrollToBottom: bindings.clearPendingManualScrollToBottom,
      triggerConversationScrollToBottom: bindings.triggerConversationScrollToBottom,
      getPendingManualScrollToBottomConversationId: bindings.getPendingManualScrollToBottomConversationId,
      getPendingManualScrollToBottomRequestId: bindings.getPendingManualScrollToBottomRequestId,
      loadAllMessages,
      getChatFlow: () => bindings.getChatFlow(),
      readConversationStreamCache: (conversationId?: string | null) =>
        bindings.getChatFlow()?.readConversationStreamCache(conversationId) || null,
      refreshRemoteImConversationOverview: bindings.refreshRemoteImConversationOverview,
      setStatusError: bindings.setStatusError,
      setConversationChatErrorText: bindings.setConversationChatErrorText,
      perfNow: bindings.perfNow,
      tr: bindings.tr,
    },
    config: bindings.config,
    tr: bindings.tr,
    currentChatConversationId: bindings.currentChatConversationId,
    currentChatPreferredApiConfigId: bindings.currentChatPreferredApiConfigId,
    currentChatTodos: bindings.currentChatTodos,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    currentForegroundConversationSummary: bindings.currentForegroundConversationSummary,
    currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
    unarchivedConversations: bindings.unarchivedConversations,
    lastOverviewSyncAt: bindings.lastOverviewSyncAt,
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
    refreshChatWorkspaceState: bindings.refreshChatWorkspaceState,
    conversationMessageCache: bindings.conversationMessageCache,
    backgroundConversationBadgeMap: bindings.backgroundConversationBadgeMap,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    setConversationChatErrorText: bindings.setConversationChatErrorText,
    perfNow: bindings.perfNow,
    loadAllMessages,
    isChatWindowActiveNow: bindings.isChatWindowActiveNow,
    closeWindow: bindings.closeWindow,
    freezeForegroundConversation: bindings.freezeForegroundConversation,
    clearPendingManualScrollToBottom: bindings.clearPendingManualScrollToBottom,
    triggerConversationScrollToBottom: bindings.triggerConversationScrollToBottom,
    getPendingManualScrollToBottomConversationId: bindings.getPendingManualScrollToBottomConversationId,
    getPendingManualScrollToBottomRequestId: bindings.getPendingManualScrollToBottomRequestId,
    createConversationAgentOptions: bindings.createConversationAgentOptions,
    defaultCreateConversationAgentId: bindings.defaultCreateConversationAgentId,
    branchingConversation: bindings.branchingConversation,
    forwardingConversationSelection: bindings.forwardingConversationSelection,
    deleteUnarchivedConversationFromArchivesRaw: bindings.deleteUnarchivedConversationFromArchivesRaw,
    openTrimActionDialog: shellDialogFlows.openTrimActionDialog,
    confirmTrimAction: shellDialogFlows.confirmTrimAction,
    closeTrimActionDialog: shellDialogFlows.closeTrimActionDialog,
    archiveCurrentConversation: chatRuntime.trimNow,
    getChatFlow: () => bindings.getChatFlow(),
    waitPendingConversationPreferredModelPersist: bindings.waitPendingConversationPreferredModelPersist,
    openPromptPreview: () => serviceAssemblies.chatDialogActions.openPromptPreview,
  });

  bindings.conversationApi.bind({
    refreshChatUnarchivedConversations: conversationOrchestrator.refreshChatUnarchivedConversations,
    syncUnarchivedConversationOverviewChangedSinceWatermark: conversationOrchestrator.syncUnarchivedConversationOverviewChangedSinceWatermark,
    freezeForegroundConversation: conversationOrchestrator.freezeForegroundConversation,
    restoreForegroundConversationProjection: conversationOrchestrator.restoreForegroundConversationProjection,
    switchUnarchivedConversation: conversationOrchestrator.switchUnarchivedConversation,
    sendChatFromCurrentWindow: conversationOrchestrator.sendChatFromCurrentWindow,
    deleteUnarchivedConversationFromArchives: conversationOrchestrator.deleteUnarchivedConversationFromArchives,
    applyConversationRuntimeStateUpdated: conversationOrchestrator.applyConversationRuntimeStateUpdated,
  });

  async function createConversationBranchFromMessage(payload: { turnId: string; targetUserMessageId: string }) {
    const sourceConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const turnMessageId = String(payload?.targetUserMessageId || payload?.turnId || "").trim();
    if (
      !sourceConversationId
      || !turnMessageId
      || bindings.branchingConversation.value
      || bindings.forwardingConversationSelection.value
    ) return;
    bindings.branchingConversation.value = true;
    try {
      const result = await invokeTauri<{
        conversationId: string;
        title: string;
        warning?: string | null;
      }>("conversation.branchFromMessage", {
        input: {
          sourceConversationId,
          turnMessageId,
        },
      });
      const conversationId = String(result?.conversationId || "").trim();
      if (!conversationId) return;
      // 分支创建已由后端单项事件插入，这里仅做差量兜底，不再全量拉取。
      if (typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function") {
        await bindings.syncUnarchivedConversationOverviewChangedSinceWatermark("branch_from_message_shell");
      }
      await conversationOrchestrator.switchUnarchivedConversation(conversationId);
      bindings.setStatus(bindings.tr("status.conversationBranchCreated", { title: String(result?.title || "").trim() || conversationId }));
    } catch (error) {
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.branchingConversation.value = false;
    }
  }

  const chatRuntimeSetup = useChatRuntimeSetup({
    chatRuntime,
    viewMode: bindings.viewMode,
    chatWindowActiveSynced: bindings.chatWindowActiveSynced,
    applyConversationRuntimeStateUpdated: bindings.applyConversationRuntimeStateUpdated,
    syncUnarchivedConversationOverviewChangedSinceWatermark: bindings.syncUnarchivedConversationOverviewChangedSinceWatermark,
    switchUnarchivedConversation: bindings.switchUnarchivedConversation,
    rehydrateTerminalApprovals: bindings.rehydrateTerminalApprovals,
    terminalApproval: bindings.terminalApproval,
    onBackground: bindings.onBackground,
    onVisibilityChange: bindings.onVisibilityChange,
    onCleanup: bindings.onCleanup,
    chatting: bindings.chatting,
    trimming: bindings.trimming,
    compactingConversation: bindings.compactingConversation,
    trimmingConversationId: bindings.trimmingConversationId,
    compactingConversationId: bindings.compactingConversationId,
    currentConversationRuntimeState: conversationOrchestrator.currentConversationRuntimeState,
    currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    currentChatConversationId: bindings.currentChatConversationId,
    chatInput: bindings.chatInput,
    selectedChatMentions: bindings.selectedChatMentions,
    clipboardImages: bindings.clipboardImages,
    queuedAttachmentNotices: bindings.queuedAttachmentNotices,
    latestUserText: bindings.latestUserText,
    latestUserImages: bindings.latestUserImages,
    chatErrorText: bindings.chatErrorText,
    setConversationChatErrorText: bindings.setConversationChatErrorText,
    allMessages: bindings.allMessages,
    bumpOwnUserDraftAlign: bindings.bumpOwnUserDraftAlign,
    tr: bindings.tr,
    formatRequestFailed: (error: unknown) => formatI18nError(bindings.tr, "status.requestFailed", error),
    removeBinaryPlaceholders,
    reloadForegroundConversationMessages: conversationOrchestrator.reloadForegroundConversationMessages,
    refreshForegroundConversationMessageById: conversationOrchestrator.refreshForegroundConversationMessageById,
    isChatWindowActiveNow: bindings.isChatWindowActiveNow,
    consumeOrQueueOwnMessageAlign: bindings.consumeOrQueueOwnMessageAlign,
    applyConversationMessageAppended: conversationOrchestrator.applyConversationMessageAppended,
    cacheConversationMessages: conversationOrchestrator.cacheConversationMessages,
    insertMessagesBeforeStreamingAssistantProjection: conversationOrchestrator.insertMessagesBeforeStreamingAssistantProjection,
    mergeMessagesIntoTimeline: conversationOrchestrator.mergeMessagesIntoTimeline,
    reuseStableMessageReferences: conversationOrchestrator.reuseStableMessageReferences,
    foregroundTailLatestReady: bindings.foregroundTailLatestReady,
    deleteUnarchivedConversationFromArchives: bindings.deleteUnarchivedConversationFromArchives,
    sendChatFromCurrentWindow: bindings.sendChatFromCurrentWindow,
    setStatusError: bindings.setStatusError,
    messageText: bindings.messageText,
    extractMessageImages: bindings.extractMessageImages,
    extractMessageAttachmentFiles: bindings.extractMessageAttachmentFiles,
    requestRecallMode: shellDialogFlows.requestRecallMode,
    requestCreateConversationBranchFromMessageConfirm: shellDialogFlows.requestCreateConversationBranchFromMessageConfirm,
    createConversationBranchFromMessage,
    setConversationPlanMode: bindings.setConversationPlanMode,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    applyConversationSnapshot: conversationOrchestrator.applyConversationSnapshot,
  });

  return {
    ...serviceAssemblies,
    conversationOrchestrator,
    chatRuntimeSetup,
    chatFlow: chatRuntimeSetup.chatFlow,
    handleConfirmPlan: chatRuntimeSetup.handleConfirmPlan,
    deleteUnarchivedConversation: chatRuntimeSetup.deleteUnarchivedConversation,
    handleCreateConversationBranchFromTurn: chatRuntimeSetup.handleCreateConversationBranchFromTurn,
    handleRecallTurn: chatRuntimeSetup.handleRecallTurn,
    handleRegenerateTurn: chatRuntimeSetup.handleRegenerateTurn,
  };
}
