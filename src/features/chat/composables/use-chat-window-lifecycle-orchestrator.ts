import { onMounted, onUnmounted } from "vue";
import { useViewRefresh } from "../../shell/composables/use-view-refresh";
import { formatI18nError } from "../../../utils/error";
import { CHAT_QUEUE_OUT_OF_SYNC_EVENT } from "./use-chat-queue";
import { useChatRuntimeWatchers } from "./use-chat-runtime-watchers";
import { useChatWindowBootstrap } from "./use-chat-window-bootstrap";
import { useChatWindowEvents } from "./use-chat-window-events";
import { useChatWindowLifecycleSetup } from "./use-chat-window-lifecycle-setup";
import { useChatWindowWatchersGlue } from "./use-chat-window-watchers-glue";

export function useChatWindowLifecycleOrchestrator(bindings: Record<string, any>) {
  const { refreshAllViewData } = useViewRefresh({
    viewMode: bindings.viewMode,
    loadConfig: bindings.loadConfig,
    loadBootstrapSnapshot: bindings.loadBootstrapSnapshot,
    loadPersonas: bindings.loadPersonas,
    loadChatSettings: bindings.loadChatSettings,
    refreshConversationHistory: bindings.refreshConversationHistory,
    loadDelegateConversations: bindings.loadDelegateConversations,
    loadArchives: bindings.loadArchives,
    resetVisibleTurnCount: () => {},
    perfNow: bindings.perfNow,
    perfLog: bindings.perfLog,
    onRefreshStepSlow: (label, error) => {
      bindings.setStatus(`启动数据加载较慢：${label}：${formatI18nError(bindings.tr, "status.requestFailed", error)}`);
    },
    onRefreshStepFailed: (label, error) => {
      bindings.setStatus(`启动数据加载失败：${label}：${formatI18nError(bindings.tr, "status.requestFailed", error)}`);
    },
  });

  const appBootstrap = useChatWindowBootstrap({
    viewMode: bindings.viewMode,
    initWindow: bindings.initWindow,
    applyTheme: bindings.applyTheme,
    normalizeLocale: bindings.normalizeLocale,
    config: bindings.config,
    locale: bindings.locale,
    agentWorkPresence: bindings.agentWorkPresence,
    enqueueTerminalApprovalRequest: bindings.enqueueTerminalApprovalRequest,
    clearMatchingConversationChatErrors: bindings.clearMatchingConversationChatErrors,
    refreshConversationHistory: bindings.refreshConversationHistory,
    assistantAgentId: bindings.assistantAgentId,
    personaEditorId: bindings.personaEditorId,
    userAlias: bindings.userAlias,
    selectedResponseStyleId: bindings.selectedResponseStyleId,
    selectedPdfReadMode: bindings.selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords: bindings.backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode: bindings.backgroundVoiceScreenshotMode,
    instructionPresets: bindings.instructionPresets,
    normalizeUiSizeScale: bindings.normalizeUiSizeScale,
    updateGithubUpdateMethod: bindings.updateGithubUpdateMethod,
    normalizeRuntimeConfigNumbers: bindings.normalizeRuntimeConfigNumbers,
    createApiConfig: bindings.createApiConfig,
    buildConfigSnapshotJson: bindings.buildConfigSnapshotJson,
    lastSavedConfigJson: bindings.lastSavedConfigJson,
    toolReviewRefreshTick: bindings.toolReviewRefreshTick,
    currentChatConversationId: bindings.currentChatConversationId,
    recordHotkeyProbeLastSeq: bindings.recordHotkeyProbeLastSeq,
    recordHotkeyProbeDown: bindings.recordHotkeyProbeDown,
    recordHotkey: bindings.recordHotkey,
    startRecording: bindings.startRecording,
    stopRecording: bindings.stopRecording,
  });

  useChatWindowEvents({
    unlisteners: bindings.chatWindowEventUnlisteners,
    sideConversationListVisible: bindings.sideConversationListVisible,
    toolReviewPanelOpenVisible: bindings.toolReviewPanelOpenVisible,
    readConversationIdFromPayload: bindings.readConversationIdFromPayload,
    currentChatConversationId: bindings.currentChatConversationId,
    matchesForegroundConversation: bindings.matchesForegroundConversation,
    getChatFlow: bindings.getChatFlow,
    mergeIncomingMessagesIntoCache: bindings.mergeIncomingMessagesIntoCache,
    readMessagesFromPayload: bindings.readMessagesFromPayload,
    setConversationBadge: bindings.setConversationBadge,
    formalizeConversationMessages: bindings.formalizeConversationMessages,
    conversationMessageCache: bindings.conversationMessageCache,
    cacheConversationMessages: bindings.cacheConversationMessages,
    clearConversationBadge: bindings.clearConversationBadge,
    toolReviewRefreshTick: bindings.toolReviewRefreshTick,
    applyConversationTodosUpdated: bindings.applyConversationTodosUpdated,
    applyConversationPinUpdated: bindings.applyConversationPinUpdated,
    applyConversationRuntimeStateUpdated: bindings.applyConversationRuntimeStateUpdated,
    applyConversationOverviewUpdated: bindings.applyConversationOverviewUpdated,
    applyConversationOverviewItemUpdated: bindings.applyConversationOverviewItemUpdated,
    CHAT_STREAM_DEBUG: bindings.CHAT_STREAM_DEBUG,
    applyConversationMessagesAfterSynced: bindings.applyConversationMessagesAfterSynced,
    applyConversationMessageAppended: bindings.applyConversationMessageAppended,
    switchUnarchivedConversation: bindings.switchUnarchivedConversation,
    scheduleChatWindowActiveStateSync: bindings.scheduleChatWindowActiveStateSync,
    startGoalTaskPolling: bindings.startGoalTaskPolling,
    refreshActiveGoalTask: bindings.refreshActiveGoalTask,
    handleWindowFocusForStateSync: bindings.handleWindowFocusForStateSync,
    handleWindowBlurForStateSync: bindings.handleWindowBlurForStateSync,
    handleVisibilityForStateSync: bindings.handleVisibilityForStateSync,
    handlePageShowForStateSync: bindings.handlePageShowForStateSync,
    handleResumeForStateSync: bindings.handleResumeForStateSync,
    handleFreezeForStateSync: bindings.handleFreezeForStateSync,
    handleOnlineForStateSync: bindings.handleOnlineForStateSync,
    handleColdStartForStateSync: bindings.handleColdStartForStateSync,
    handleWindowFocusForMicPrewarm: bindings.handleWindowFocusForMicPrewarm,
    handleVisibilityForMicPrewarm: bindings.handleVisibilityForMicPrewarm,
    clearChatWindowActiveSyncTimer: bindings.clearChatWindowActiveSyncTimer,
    clearChatMicPrewarmTimer: bindings.clearChatMicPrewarmTimer,
    clearGoalTaskPollTimer: bindings.clearGoalTaskPollTimer,
    cleanupChatForegroundActivity: bindings.cleanupChatForegroundActivity,
    agentWorkPresence: bindings.agentWorkPresence,
    cancelPendingRewindConfirm: bindings.cancelPendingRewindConfirm,
  });

  useChatWindowWatchersGlue({
    viewMode: bindings.viewMode,
    config: bindings.config,
    currentChatConversationId: bindings.currentChatConversationId,
    currentForegroundApiConfigId: bindings.currentForegroundApiConfigId,
    currentForegroundApiConfig: bindings.currentForegroundApiConfig,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    hasVisionFallback: bindings.hasVisionFallback,
    unarchivedConversations: bindings.unarchivedConversations,
    startupDataReady: bindings.startupDataReady,
    chatWorkspaceName: bindings.chatWorkspaceName,
    scheduleChatWindowActiveStateSync: bindings.scheduleChatWindowActiveStateSync,
    scheduleChatWindowActiveStateRecheck: bindings.scheduleChatWindowActiveStateRecheck,
    handleGoalConversationChanged: bindings.handleGoalConversationChanged,
    clearMatchingConversationChatErrors: bindings.clearMatchingConversationChatErrors,
    refreshChatWorkspaceState: bindings.refreshChatWorkspaceState,
    syncCurrentConversationWorkspaceLabel: bindings.syncCurrentConversationWorkspaceLabel,
    handleWindowFocusForMicPrewarm: bindings.handleWindowFocusForMicPrewarm,
    applyUiFont: bindings.applyUiFont,
    applyCodeFont: bindings.applyCodeFont,
  });

  useChatRuntimeWatchers({
    viewMode: bindings.viewMode,
    currentForegroundAgentId: bindings.currentForegroundAgentId,
    currentChatConversationId: bindings.currentChatConversationId,
    startupDataReady: bindings.startupDataReady,
    allMessages: bindings.allMessages,
    setStatusError: bindings.setStatusError,
    refreshChatUnarchivedConversations: bindings.refreshChatUnarchivedConversations,
    syncUnarchivedConversationOverviewChangedSinceWatermark: bindings.syncUnarchivedConversationOverviewChangedSinceWatermark,
    getChatFlow: bindings.getChatFlow,
    maybeResumeForegroundStreamingBubble: bindings.maybeResumeForegroundStreamingBubble,
    resumeForegroundRuntimeFromBackend: bindings.resumeForegroundRuntimeFromBackend,
  });

  // 队列撤回/切引导拿到后端准确不在队列反馈时，前端队列已在 useChatQueue 内刷新，
  // 这里只补刷当前会话历史；非准确反馈不触发，跨会话失配直接忽略。
  onMounted(() => {
    const handleQueueOutOfSync = (event: Event) => {
      if (String(bindings.viewMode?.value || "").trim() !== "chat") return;
      const detail = (event as CustomEvent<{ conversationId?: string } | undefined>)?.detail;
      const staleConversationId = String(detail?.conversationId || "").trim();
      const currentConversationId = String(bindings.currentChatConversationId?.value || "").trim();
      if (!currentConversationId) return;
      if (staleConversationId && staleConversationId !== currentConversationId) return;
      void bindings.refreshConversationHistory?.();
    };
    window.addEventListener(CHAT_QUEUE_OUT_OF_SYNC_EVENT, handleQueueOutOfSync);
    onUnmounted(() => {
      window.removeEventListener(CHAT_QUEUE_OUT_OF_SYNC_EVENT, handleQueueOutOfSync);
    });
  });

  useChatWindowLifecycleSetup({
    appBootstrap,
    restoreThemeFromStorage: bindings.restoreThemeFromStorage,
    onPaste: bindings.onPaste,
    onDragOver: bindings.onDragOver,
    onDrop: bindings.onDrop,
    onTransportFileDrop: bindings.onTransportFileDrop,
    mediaDragActive: bindings.mediaDragActive,
    recordHotkey: bindings.recordHotkey,
    ensureMessageStoreMigrationGate: async () => {
      await bindings.ensureMessageStoreMigrationGate();
    },
    refreshAllViewData,
    startupDataReady: bindings.startupDataReady,
    currentChatConversationId: bindings.currentChatConversationId,
    refreshChatUnarchivedConversations: bindings.refreshChatUnarchivedConversations,
    viewMode: bindings.viewMode,
    syncWindowControlsState: bindings.syncWindowControlsState,
    stopRecording: bindings.stopRecording,
    cleanupSpeechRecording: bindings.cleanupSpeechRecording,
    cleanupChatMedia: bindings.cleanupChatMedia,
    startupOverlayVisible: bindings.startupOverlayVisible,
    setStatus: bindings.setStatus,
    formatRequestFailed: (error: unknown) => formatI18nError(bindings.tr, "status.requestFailed", error),
    refreshGithubUpdateState: bindings.refreshGithubUpdateState,
    config: bindings.config,
    configTab: bindings.configTab,
    personas: bindings.personas,
    userPersona: bindings.userPersona,
    assistantPersonas: bindings.assistantPersonas,
    assistantAgentId: bindings.assistantAgentId,
    personaEditorId: bindings.personaEditorId,
    selectedApiConfig: bindings.selectedApiConfig,
    toolApiConfig: bindings.toolApiConfig,
    modelRefreshError: bindings.modelRefreshError,
    toolStatuses: bindings.toolStatuses,
    defaultApiTools: bindings.defaultApiTools,
    tr: bindings.tr,
    normalizeApiBindingsLocal: bindings.normalizeApiBindingsLocal,
    syncUserAliasFromPersona: bindings.syncUserAliasFromPersona,
    syncTrayIcon: bindings.syncTrayIcon,
    refreshToolsStatus: bindings.refreshToolsStatus,
  });
}
