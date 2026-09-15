import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { emitTransportEvent, invokeTauri } from "../../../services/tauri-api";
import { useAppCore } from "../../shell/composables/use-app-core";
import { useAppTheme } from "../../shell/composables/use-app-theme";
import { normalizeUiSizeScale, useUiSizeAppearance } from "../../shell/composables/use-ui-size-appearance";
import { useGithubUpdateMethod } from "../../shell/composables/use-github-update-method";
import { usePipelineStatus } from "../../shell/composables/use-pipeline-status";
import { applyCodeFont, applyUiFont, normalizeUiFont } from "../../shell/composables/use-ui-font";
import { useMessageStoreMigrationGate } from "../../shell/composables/use-message-store-migration-gate";
import { useWindowActions } from "../../shell/composables/use-window-actions";
import { useWindowShell } from "../../shell/composables/use-window-shell";
import { searchConfigTabs } from "../../config/search/config-search";
import { useChatDialogRefs } from "./use-chat-dialog-refs";
import { useChatScrollCoordinator } from "./use-chat-scroll-coordinator";
import { useChatWindowShellDataOrchestrator } from "./use-chat-window-shell-data-orchestrator";
import { useChatWindowRuntimeOrchestrator } from "./use-chat-window-runtime-orchestrator";
import { useChatWindowContentOrchestrator } from "./use-chat-window-content-orchestrator";
import { useChatWindowWorkspaceOrchestrator } from "./use-chat-window-workspace-orchestrator";
import { useChatWindowLifecycleOrchestrator } from "./use-chat-window-lifecycle-orchestrator";
import { useChatWindowMediaOrchestrator } from "./use-chat-window-media-orchestrator";
import { useChatWindowState } from "./use-chat-window-state";
import { useChatUiStateOrchestrator } from "./use-chat-ui-state-orchestrator";
import { useChatComposerDrafts } from "./use-chat-composer-drafts";
import { extractMessageAttachmentFiles, extractMessageImages, messageText } from "../../../utils/chat-message";
import { formatI18nError } from "../../../utils/error";
import type { AppConfig, ChildConversationSummary } from "../../../types/app";
import { normalizeLocale } from "../../../i18n";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";
import { ensureConversationMessageIds } from "../utils/message-id";
import { useChatWindowConfigOrchestrator } from "./use-chat-window-config-orchestrator";
import { resolveSideChatSelectionAfterClose } from "./side-chat-tabs";
import { useChatWindowPaneExpansion } from "./use-chat-window-pane-expansion";

type ConversationActionsBridge = {
  refreshChatUnarchivedConversations: () => Promise<void>;
  syncUnarchivedConversationOverviewChangedSinceWatermark: (reason?: string) => Promise<void>;
  freezeForegroundConversation: (reason: string) => void;
  restoreForegroundConversationProjection: (conversationId: string, reason: string) => Promise<void>;
  switchUnarchivedConversation: (conversationId: string) => Promise<void>;
  sendChatFromCurrentWindow: (overrides?: { extraTextBlocks?: string[] }) => Promise<void>;
  deleteUnarchivedConversationFromArchives: (conversationId: string) => Promise<void>;
  applyConversationRuntimeStateUpdated: (payload: { conversationId: string; runtimeState: "idle" | "assistant_streaming" | "organizing_context" }) => void;
};

export function useChatWindowApp() {
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
    toggleMaximizeWindow: toggleMaximizeWindowBase,
  } = useWindowShell();
  const chatWindowPaneExpansion = useChatWindowPaneExpansion();
  const {
    currentTheme,
    generatedThemeControls,
    generatedThemeTokens,
    generatedThemeTokensByMode,
    themeMode,
    autoLightTheme,
    autoDarkTheme,
    applyTheme,
    setTheme,
    setThemeMode,
    setAutoTheme,
    activateGeneratedTheme,
    updateGeneratedThemeControls,
    resetGeneratedTheme,
    restoreThemeFromStorage,
  } = useAppTheme();
  const generatedLightTokens = computed(() => generatedThemeTokensByMode.value.light);
  const generatedDarkTokens = computed(() => generatedThemeTokensByMode.value.dark);
  
  const {
    BACKGROUND_CONVERSATION_CACHE_LIMIT,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    OLDER_HISTORY_PAGE_SIZE,
    viewMode,
    config,
    recordHotkeyProbeLastSeq,
    recordHotkeyProbeDown,
    chatWindowActiveSynced,
    chatWindowEventUnlisteners,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    personas,
    assistantAgentId,
    personaEditorId,
    userAlias,
    selectedResponseStyleId,
    selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    instructionPresets,
    conversationForegroundSyncing,
    lastOverviewSyncAt,
    backgroundConversationBadgeMap,
    conversationMessageCache,
    latestUserText,
    latestUserImages,
    latestOwnMessageAlignRequest,
    latestContextUsagePreview,
    clipboardImages,
    queuedAttachmentNotices,
    allMessages,
    status,
    terminalApprovalQueue,
    terminalApprovalResolving,
    loading,
    saving,
    startupDataReady,
    startupOverlayVisible,
    chatting,
    trimming,
    compactingConversation,
    trimmingConversationId,
    compactingConversationId,
    suppressNextCompactionReload,
    branchingConversation,
    forwardingConversationSelection,
    hasMoreBackendHistory,
    loadingOlderConversationHistory,
    refreshingModels,
    modelRefreshError,
    modelRefreshOkFlags,
    checkingToolsStatus,
    toolStatuses,
    avatarSaving,
    avatarError,
    personaSaving,
    apiModelOptions,
    suppressAutosave,
    lastSavedConfigJson,
    lastSavedPersonasJson,
    PERF_DEBUG,
    CHAT_STREAM_DEBUG,
    toolReviewRefreshTick,
    currentChatTodos,
    foregroundTailLatestReady,
  } = useChatWindowState({
    isMacPlatform,
    t,
  });
  let conversationActions: ConversationActionsBridge = {
    refreshChatUnarchivedConversations: async () => {},
    syncUnarchivedConversationOverviewChangedSinceWatermark: async () => {},
    freezeForegroundConversation: () => {},
    restoreForegroundConversationProjection: async () => {},
    switchUnarchivedConversation: async () => {},
    sendChatFromCurrentWindow: async () => {},
    deleteUnarchivedConversationFromArchives: async () => {},
    applyConversationRuntimeStateUpdated: () => {},
  };
  const refreshChatUnarchivedConversations = () => conversationActions.refreshChatUnarchivedConversations();
  const syncUnarchivedConversationOverviewChangedSinceWatermark = (reason?: string) =>
    conversationActions.syncUnarchivedConversationOverviewChangedSinceWatermark(reason);
  const freezeForegroundConversation = (reason: string) => conversationActions.freezeForegroundConversation(reason);
  const restoreForegroundConversationProjection = (conversationId: string, reason: string) =>
    conversationActions.restoreForegroundConversationProjection(conversationId, reason);
  const switchUnarchivedConversation = (conversationId: string) =>
    conversationActions.switchUnarchivedConversation(conversationId);
  const sendChatFromCurrentWindow = (overrides?: { extraTextBlocks?: string[] }) => conversationActions.sendChatFromCurrentWindow(overrides);
  const deleteUnarchivedConversationFromArchives = (conversationId: string) => conversationActions.deleteUnarchivedConversationFromArchives(conversationId);
  const applyConversationRuntimeStateUpdated: ConversationActionsBridge["applyConversationRuntimeStateUpdated"] =
    (payload) => conversationActions.applyConversationRuntimeStateUpdated(payload);
  const conversationApi = {
    bind: (actions: ConversationActionsBridge) => { conversationActions = actions; },
    applyConversationRuntimeStateUpdated,
  };
  let chatFlow: any = null;
  let pendingOwnMessageAlignConversationId = "";
  let pendingOwnMessageAlignToken = 0;
  let pendingOwnMessageAlignTimer = 0;

  function clearPendingOwnMessageAlignTimer() {
    if (pendingOwnMessageAlignTimer) {
      window.clearTimeout(pendingOwnMessageAlignTimer);
      pendingOwnMessageAlignTimer = 0;
    }
  }

  function bumpOwnUserDraftAlign() {
    const conversationId = String(currentChatConversationId.value || "").trim();
    pendingOwnMessageAlignConversationId = conversationId;
    pendingOwnMessageAlignToken += 1;
    latestOwnMessageAlignRequest.value = pendingOwnMessageAlignToken;
    clearPendingOwnMessageAlignTimer();
    if (!conversationId) return;
    triggerConversationScrollToBottom(conversationId, "draft_inserted", "own_top");
  }

  function consumeOrQueueOwnMessageAlign() {
    const conversationId = pendingOwnMessageAlignConversationId || String(currentChatConversationId.value || "").trim();
    const token = pendingOwnMessageAlignToken || (latestOwnMessageAlignRequest.value + 1);
    if (!conversationId) return;
    pendingOwnMessageAlignConversationId = conversationId;
    pendingOwnMessageAlignToken = token;
    latestOwnMessageAlignRequest.value = token;
    clearPendingOwnMessageAlignTimer();
    triggerConversationScrollToBottom(conversationId, "own_message_aligned", "own_top");
  }
  const {
    messageStoreMigration,
    ensureMessageStoreMigrationGate,
    confirmMessageStoreMigrationSummary,
    retryMessageStoreMigration,
  } = useMessageStoreMigrationGate({
    formatRequestFailed: (error) => formatI18nError(tr, "status.requestFailed", error),
    t: tr,
  });
  const { perfNow, perfLog, setStatus, setStatusError, statusTone, localeOptions, applyUiLanguage } = useAppCore({
    t: tr,
    config,
    locale,
    status,
    perfDebug: PERF_DEBUG,
  });
  const { setUiSizeScale, uiSizeScale } = useUiSizeAppearance();
  const { updateGithubUpdateMethod } = useGithubUpdateMethod(config, setStatusError);
  const { clearConversationStatus } = usePipelineStatus({
    activeConversationId: computed(() => String(currentChatConversationId.value || "").trim()),
  });
  const chatUiState = useChatUiStateOrchestrator({
    viewMode,
    currentChatConversationId,
    clearConversationStatus: (conversationId, statusKind) => {
      clearConversationStatus(conversationId, statusKind);
    },
    searchConfigTabs,
    resolveConfigLocale: () => normalizeLocale(config.uiLanguage),
    windowPaneExpansion: {
      beforeOpen: chatWindowPaneExpansion.beforeOpen,
      afterOpen: chatWindowPaneExpansion.afterOpen,
      beforeClose: chatWindowPaneExpansion.beforeClose,
      afterClose: chatWindowPaneExpansion.afterClose,
    },
  });
  const {
    configTab,
    configSearchQuery,
    configSearchResults,
    selectedChatMentions,
    chatInput,
    conversationListTab,
    chatLeftPanelMode,
    chatRightPanelMode,
    chatMonitorPanelMode,
    sideConversationListVisible,
    toolReviewPanelOpenVisible,
    chatSidePanelWidths,
    chatErrorText,
    handleChatInputUpdate,
    updateConfigSearchQuery,
    handleSelectConfigSearchResult,
    addChatMention,
    removeChatMention,
    handleSideConversationListVisibleChange,
    handleToolReviewPanelOpenChange,
    updateConversationListTab,
    updateChatLeftPanelMode,
    updateChatRightPanelMode,
    openChatReaderPanel,
    updateChatMonitorPanelMode,
    handleChatSidePanelWidthsChange,
    toggleSideConversationList,
    toggleToolReviewPanel,
    setConversationChatErrorText,
    clearMatchingConversationChatErrors,
    clearChatError,
  } = chatUiState;
  function currentPaneVisibility() {
    return {
      leftVisible: sideConversationListVisible.value,
      rightVisible: toolReviewPanelOpenVisible.value,
      leftWidth: chatSidePanelWidths.value.leftWidth,
      rightWidth: chatSidePanelWidths.value.rightWidth,
    };
  }

  async function toggleMaximizeWindow() {
    await chatWindowPaneExpansion.collapseVisiblePanes(currentPaneVisibility());
    await toggleMaximizeWindowBase();
    if (!maximized.value) {
      await chatWindowPaneExpansion.syncVisiblePanes(currentPaneVisibility());
    }
  }

  watch(
    windowReady,
    (ready) => {
      if (!ready || viewMode.value !== "chat") return;
      void chatWindowPaneExpansion.syncVisiblePanes(currentPaneVisibility());
    },
    { immediate: true },
  );
  useChatComposerDrafts({
    activeConversationId: currentChatConversationId,
    chatInput,
    selectedMentions: selectedChatMentions,
    clipboardImages,
    queuedAttachmentNotices,
  });
  const shellData = useChatWindowShellDataOrchestrator({
    tr,
    viewMode,
    status,
    config,
    setStatus,
    setStatusError,
    currentChatConversationId,
  });
  const {
    unarchivedConversations,
    delegateConversations,
    remoteImContactConversations,
    loadArchives,
    loadDelegateConversations,
    deleteUnarchivedConversation: deleteUnarchivedConversationFromArchivesRaw,
  } = shellData.archivesView;
  const { setConversationPlanMode } = shellData.conversationPlanMode;
  const {
    refreshActiveGoalTask,
    startGoalTaskPolling,
    clearGoalTaskPollTimer,
    handleConversationChanged: handleGoalConversationChanged,
    applyConversationGoalUpdated,
  } = shellData.goalTask;
  const agentWorkPresence = shellData.agentWorkPresence;
  let refreshToolsStatus: () => void | Promise<void> = () => {};
  const configOrchestrator = useChatWindowConfigOrchestrator({
    t,
    config,
    locale,
    personas,
    configTab,
    apiModelOptions,
    modelRefreshOkFlags,
    lastSavedConfigJson,
    lastSavedPersonasJson,
    normalizeLocale,
    applyUiLanguage,
    refreshToolsStatus: () => refreshToolsStatus(),
    setStatus,
    setStatusError,
  });
  const {
    configDerived,
    configCore,
    configUi,
    configActions,
  } = configOrchestrator;
  const contentOrchestrator = useChatWindowContentOrchestrator({
    t,
    tr,
    viewMode,
    maximized,
    config,
    configDerived,
    locale,
    personas,
    assistantAgentId,
    personaEditorId,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    personaDirty: configUi.personaDirty,
    unarchivedConversations,
    remoteImContactConversations,
    backgroundConversationBadgeMap,
    allMessages,
    foregroundTailLatestReady,
    status,
    setStatus,
    setStatusError,
    selectedResponseStyleId,
    selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    instructionPresets,
    clipboardImages,
    queuedAttachmentNotices,
    normalizeLocale,
    PERF_DEBUG,
    perfNow,
    userAlias,
    agentWorkPresence,
    terminalApprovalQueue,
    terminalApprovalResolving,
  });
  const {
    selectedApiConfig,
    selectedApiProvider,
    normalizeRuntimeConfigNumbers,
    hasVisionFallback,
    activeSttApiConfig,
    shouldUseRemoteStt,
  } = configDerived;
  const chatMedia = useChatWindowMediaOrchestrator({
    tr,
    normalizeLocale,
    viewMode,
    config,
    status,
    setStatusError,
    chatting,
    trimming,
    chatInput,
    chatErrorText,
    clipboardImages,
    queuedAttachmentNotices,
    activeSttApiConfig,
    shouldUseRemoteStt,
    currentChatConversationId,
    currentForegroundAgentId: contentOrchestrator.personaConversation.currentForegroundAgentId,
    startupDataReady,
    recordHotkeyProbeLastSeq,
    recordHotkeyProbeDown,
    chatWindowActiveSynced,
    allMessages,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    BACKGROUND_CONVERSATION_CACHE_LIMIT,
    getChatFlow: () => chatFlow,
    applyConversationRuntimeStateUpdated,
    refreshChatUnarchivedConversations,
    syncUnarchivedConversationOverviewChangedSinceWatermark,
    freezeForegroundConversation,
    restoreForegroundConversationProjection,
    switchUnarchivedConversation,
    parseBackgroundVoiceScreenshotKeywords: (text: string) => parseBackgroundVoiceScreenshotKeywords(text),
    matchBackgroundVoiceScreenshotKeyword: (text: string, keywords: string[]) => matchBackgroundVoiceScreenshotKeyword(text, keywords),
    queueAutoScreenshotFromVoice: (input: {
      source: "local" | "remote";
      keyword: string;
      mode: "desktop" | "focused_window";
      startedAt: number;
    }) => queueAutoScreenshotFromVoice(input),
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    sendChatFromCurrentWindow,
    getCurrentForegroundApiConfig: () => currentForegroundApiConfig.value,
    hasVisionFallback,
  });
  const { isChatWindowActiveNow } = chatMedia;
  const {
    conversationScrollToBottomRequest,
    scrollToBottomBehavior,
    clearPendingConversationScrollToBottomFallback,
    clearPendingManualScrollToBottom,
    triggerConversationScrollToBottom,
    scheduleConversationScrollToBottomFallback,
    setPendingManualScrollState,
    getPendingManualScrollToBottomConversationId,
    getPendingManualScrollToBottomRequestId,
  } = useChatScrollCoordinator({
    currentChatConversationId,
  });
  
  const {
    resolveAvatarUrl,
    ensureAvatarCached,
    preloadPersonaAvatars,
  } = contentOrchestrator.avatarCache;
  const {
    userPersona,
    assistantPersonas,
    currentForegroundConversationSummary,
    currentForegroundAgentId,
    currentForegroundApiConfigId,
    currentForegroundApiConfig,
    currentForegroundPersona,
    selectedPersonaEditor,
    toolApiConfig,
    userPersonaAvatarUrl,
    currentForegroundPersonaAvatarUrl,
    selectedPersonaEditorAvatarUrl,
    chatPersonaAvatarUrlMap,
    createConversationAgentOptions,
  } = contentOrchestrator.personaConversation;
  const {
    openSettingsWindow,
    summonChatWindowFromConfig,
    closeWindowAndClearForeground,
    minimizeWindowAndClearForeground,
    openGithubRepository,
  } = useWindowActions({
    closeWindow,
    minimizeWindow,
    freezeForegroundConversation,
  });
  const workspaceOrchestrator = useChatWindowWorkspaceOrchestrator({
    currentForegroundApiConfigId,
    currentForegroundAgentId,
    currentChatConversationId,
    setStatus,
    setStatusError,
    tr,
  });
  const {
    chatWorkspaceName,
    chatWorkspaceDisplayName,
    refreshChatWorkspaceState,
  } = workspaceOrchestrator;
  const {
    createApiProvider,
    createApiConfig,
    normalizeApiBindingsLocal,
    buildConfigPayload,
    buildConfigSnapshotJson,
  } = configCore;
  const {
    defaultCreateConversationAgentId,
    responseStyleIds,
  } = configUi;

  const {
    syncUserAliasFromPersona,
  } = contentOrchestrator.messageHelpers;
  
  const {
    updatePersonaEditorIdWithNotice,
    updateAssistantAgentId,
    updateForegroundAgentPrimaryApiConfig,
    updateConversationPreferredApiConfigId,
    updateSelectedResponseStyleId,
    updateSelectedPdfReadMode,
    updateBackgroundVoiceScreenshotKeywords,
    updateBackgroundVoiceScreenshotMode,
    updateInstructionPresets,
    waitPendingConversationPreferredModelPersist,
    parseBackgroundVoiceScreenshotKeywords,
    matchBackgroundVoiceScreenshotKeyword,
    queueAutoScreenshotFromVoice,
  } = contentOrchestrator.localTools;
  const {
    buildPersonasSnapshotJson,
    setUiLanguage,
    importPersonaMemories,
    handleToolsChanged,
  } = configActions;

  const runtimeOrchestrator = useChatWindowRuntimeOrchestrator({
    BACKGROUND_CONVERSATION_CACHE_LIMIT,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    OLDER_HISTORY_PAGE_SIZE,
    conversationApi,
    tr,
    setStatus,
    setStatusError,
    personas,
    assistantAgentId,
    avatarSaving,
    avatarError,
    selectedApiConfig,
    selectedApiProvider,
    refreshingModels,
    modelRefreshError,
    apiModelOptions,
    modelRefreshOkFlags,
    toolApiConfig,
    checkingToolsStatus,
    toolStatuses,
    ensureAvatarCached,
    config,
    locale,
    normalizeLocale,
    suppressAutosave,
    loading,
    saving,
    personaSaving,
    assistantPersonas,
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
    selectedPersonaEditor,
    createApiProvider,
    chatErrorText,
    currentForegroundApiConfigId,
    chatUsagePercent: contentOrchestrator.messageBlocks.chatUsagePercent,
    currentForegroundAgentId,
    currentForegroundConversationSummary,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    currentChatTodos,
    trimmingConversationId,
    compactingConversationId,
    chatting,
    trimming,
    compactingConversation,
    suppressNextCompactionReload,
    allMessages,
    refreshChatUnarchivedConversations,
    refreshChatWorkspaceState,
    perfNow,
    perfLog,
    PERF_DEBUG,
    configTab,
    unarchivedConversations,
    delegateConversations,
    remoteImContactConversations,
    conversationForegroundSyncing,
    sideConversationListVisible,
    hasMoreBackendHistory,
    loadingOlderConversationHistory,
    foregroundTailLatestReady,
    chatWorkspaceName,
    chatWorkspaceDisplayName,
    viewMode,
    chatWindowActiveSynced,
    applyConversationRuntimeStateUpdated,
    syncUnarchivedConversationOverviewChangedSinceWatermark,
    switchUnarchivedConversation,
    onBackground: chatMedia.cancelForegroundRecordingOnBackground,
    onVisibilityChange: chatMedia.clearChatMicPrewarmTimer,
    onCleanup: chatMedia.cleanupChatForegroundRecording,
    conversationMessageCache,
    backgroundConversationBadgeMap,
    ensureConversationMessageIds,
    isChatWindowActiveNow,
    closeWindow,
    freezeForegroundConversation,
    clearPendingManualScrollToBottom,
    triggerConversationScrollToBottom,
    getPendingManualScrollToBottomConversationId,
    getPendingManualScrollToBottomRequestId,
    createConversationAgentOptions,
    defaultCreateConversationAgentId,
    branchingConversation,
    forwardingConversationSelection,
    deleteUnarchivedConversationFromArchivesRaw,
    getChatFlow: () => chatFlow,
    waitPendingConversationPreferredModelPersist,
    chatInput,
    selectedChatMentions,
    clipboardImages,
    queuedAttachmentNotices,
    latestUserText,
    latestUserImages,
    setConversationChatErrorText,
    bumpOwnUserDraftAlign,
    consumeOrQueueOwnMessageAlign,
    deleteUnarchivedConversationFromArchives,
    sendChatFromCurrentWindow,
    setConversationPlanMode,
    messageText,
    extractMessageImages,
    extractMessageAttachmentFiles,
    rehydrateTerminalApprovals: () => contentOrchestrator.terminalApproval.rehydrateTerminalApprovals(),
    terminalApproval: contentOrchestrator.terminalApproval,
  });
  const {
    configRuntime,
    configPersistence,
    configEditors,
    chatRuntime,
    promptPreviewActions,
    memoryViewerActions,
    shellDialogFlows,
    chatDialogActions,
    conversationOrchestrator,
    handleConfirmPlan,
    deleteUnarchivedConversation,
    handleCreateConversationBranchFromTurn,
    handleRecallTurn,
    handleRegenerateTurn,
  } = runtimeOrchestrator;
  chatFlow = runtimeOrchestrator.chatFlow;
  const sideConversationId = ref("");
  const closingSideConversationIds = ref<string[]>([]);
  // 新建页锁定：点 + 后停留在追问新建页，不自动选中任一追问，直到用户选择或创建
  const sideChatNewPageRequested = ref(false);
  // 只记住每个父会话当前选中的标签；真实标签集合始终以后端父摘要为准。
  const activeSideConversationByParent = new Map<string, string>();
  const sideConversations = computed<ChildConversationSummary[]>(() => {
    const parentId = String(currentChatConversationId.value || "").trim();
    const parent = unarchivedConversations.value
      .find((item) => String(item?.conversationId || "").trim() === parentId);
    const closingIds = new Set(closingSideConversationIds.value);
    return (Array.isArray(parent?.childConversations) ? parent.childConversations : [])
      .filter((item) => {
        const conversationId = String(item?.conversationId || "").trim();
        return conversationId && !closingIds.has(conversationId);
      });
  });
  let observedSideConversationParentId = "";
  watch(
    [currentChatConversationId, sideConversations],
    ([parentConversationId, children]) => {
      const parentId = String(parentConversationId || "").trim();
      const parentChanged = parentId !== observedSideConversationParentId;
      observedSideConversationParentId = parentId;
      // 用户显式停留在新建页时，不随父会话变化自动选中追问
      if (sideChatNewPageRequested.value) return;
      if (!parentId) {
        sideConversationId.value = "";
        return;
      }
      const childIds = children.map((item) => String(item?.conversationId || "").trim()).filter(Boolean);
      const currentId = String(sideConversationId.value || "").trim();
      if (!parentChanged && childIds.includes(currentId)) return;
      const rememberedId = String(activeSideConversationByParent.get(parentId) || "").trim();
      const nextId = childIds.includes(rememberedId) ? rememberedId : childIds[0] || "";
      sideConversationId.value = nextId;
      if (nextId) activeSideConversationByParent.set(parentId, nextId);
    },
    { immediate: true },
  );
  const selectSideChatConversation = (conversationId: string) => {
    const normalizedId = String(conversationId || "").trim();
    if (!sideConversations.value.some((item) => String(item.conversationId || "").trim() === normalizedId)) return;
    sideChatNewPageRequested.value = false;
    sideConversationId.value = normalizedId;
    const parentId = String(currentChatConversationId.value || "").trim();
    if (parentId) activeSideConversationByParent.set(parentId, normalizedId);
  };
  // 打开追问新建页（Chrome 新标签页式）：不创建会话，停留在选择页直到用户点击
  const openSideChatNewPage = () => {
    sideChatNewPageRequested.value = true;
    sideConversationId.value = "";
    chatUiState.updateChatRightPanelMode("sideChat");
  };
  const createSideChatConversation = async (withContext = true) => {
    const conversationId = await conversationOrchestrator.createSideChatConversation(undefined, withContext);
    if (conversationId) {
      sideChatNewPageRequested.value = false;
      sideConversationId.value = conversationId;
      const parentId = String(currentChatConversationId.value || "").trim();
      if (parentId) activeSideConversationByParent.set(parentId, conversationId);
      chatUiState.updateChatRightPanelMode("sideChat");
      toolReviewPanelOpenVisible.value = true;
      // 追问创建不重建主列表：父会话单项事件已推送，这里仅差量兜底。
      await syncUnarchivedConversationOverviewChangedSinceWatermark("side_chat_created").catch((error) => {
        setStatusError("status.requestFailed", error);
      });
    }
    return conversationId;
  };
  const createSideConversationBranchFromTurn = async (payload: { turnId: string; sourceConversationId?: string }) => {
    const sourceConversationId = String(payload?.sourceConversationId || "").trim();
    const turnMessageId = String(payload?.turnId || "").trim();
    if (!sourceConversationId || !turnMessageId) return;
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
      // 分支创建已由后端单项事件插入，这里仅差量兜底。
      await syncUnarchivedConversationOverviewChangedSinceWatermark("side_chat_branch_created").catch((error) => {
        setStatusError("status.requestFailed", error);
      });
      selectSideChatConversation(conversationId);
    } catch (error) {
      setStatusError("status.createBranchFailed", error);
    }
  };
  const closeSideChatConversations = async (conversationIds: string[]) => {
    const orderedIds = sideConversations.value.map((item) => String(item.conversationId || "").trim()).filter(Boolean);
    const requestedIds = new Set((conversationIds || []).map((item) => String(item || "").trim()).filter(Boolean));
    const idsToClose = orderedIds.filter((conversationId) => requestedIds.has(conversationId));
    if (idsToClose.length === 0) return;
    const closingSet = new Set(idsToClose);
    const activeId = String(sideConversationId.value || "").trim();
    if (closingSet.has(activeId)) {
      const nextId = resolveSideChatSelectionAfterClose(orderedIds, activeId, closingSet);
      sideConversationId.value = nextId;
      const parentId = String(currentChatConversationId.value || "").trim();
      if (parentId) {
        if (nextId) activeSideConversationByParent.set(parentId, nextId);
        else activeSideConversationByParent.delete(parentId);
      }
    }
    closingSideConversationIds.value = Array.from(new Set([
      ...closingSideConversationIds.value,
      ...idsToClose,
    ]));
    await nextTick();
    try {
      for (const conversationId of idsToClose) {
        await invokeTauri("chat.stop", {
          input: {
            session: {
              apiConfigId: String(currentForegroundApiConfigId.value || "").trim(),
              agentId: String(currentForegroundAgentId.value || "").trim(),
              conversationId,
            },
            partialAssistantText: "",
            partialStreamBlocks: [],
          },
        }).catch(() => {});
        await invokeTauri("conversation.delete", {
          input: { conversationId },
        }).catch((error) => {
          setStatusError("status.requestFailed", error);
        });
      }
      // 删除追问：后端已注册 watermark 删除语义，前端差量同步收敛。
      await syncUnarchivedConversationOverviewChangedSinceWatermark("side_chat_closed").catch((error) => {
        setStatusError("status.requestFailed", error);
      });
    } finally {
      closingSideConversationIds.value = closingSideConversationIds.value
        .filter((conversationId) => !closingSet.has(conversationId));
    }
  };
  refreshToolsStatus = configRuntime.refreshToolsStatus;
  const {
    setMemoryDialogRef,
    setPromptPreviewDialogRef,
  } = useChatDialogRefs({
    memoryDialog: memoryViewerActions.memoryDialog,
    promptPreviewDialog: promptPreviewActions.promptPreviewDialog,
  });

  useChatWindowLifecycleOrchestrator({
    viewMode,
    config,
    locale,
    tr,
    perfNow,
    perfLog,
    setStatus,
    setStatusError,
    initWindow,
    applyTheme,
    restoreThemeFromStorage,
    normalizeLocale,
    normalizeUiSizeScale,
    updateGithubUpdateMethod,
    applyUiFont,
    applyCodeFont,
    chatWindowEventUnlisteners,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    chatWindowActiveSynced,
    currentChatTodos,
    startupDataReady,
    startupOverlayVisible,
    toolReviewRefreshTick,
    recordHotkeyProbeLastSeq,
    recordHotkeyProbeDown,
    userAlias,
    selectedResponseStyleId,
    selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    instructionPresets,
    allMessages,
    conversationMessageCache,
    ensureConversationMessageIds,
    CHAT_STREAM_DEBUG,
    getChatFlow: () => chatFlow,
    ensureMessageStoreMigrationGate,
    syncWindowControlsState,
    refreshGithubUpdateState: shellData.githubUpdate.refreshGithubUpdateState,
    loadDelegateConversations,
    loadArchives,
    unarchivedConversations,
    agentWorkPresence,
    cancelPendingRewindConfirm: shellDialogFlows.cancelPendingRewindConfirm,
    handleGoalConversationChanged,
    clearGoalTaskPollTimer,
    startGoalTaskPolling,
    refreshActiveGoalTask,
    applyConversationGoalUpdated,
    ...configDerived,
    ...configCore,
    ...configRuntime,
    ...configPersistence,
    ...chatRuntime,
    ...contentOrchestrator.terminalApproval,
    ...contentOrchestrator.personaConversation,
    ...contentOrchestrator.messageHelpers,
    ...chatUiState,
    ...chatMedia,
    ...runtimeOrchestrator.chatRuntimeSetup,
    ...workspaceOrchestrator,
    ...conversationOrchestrator,
    personas,
    userPersona: contentOrchestrator.personaConversation.userPersona,
    assistantPersonas: contentOrchestrator.personaConversation.assistantPersonas,
    assistantAgentId,
    personaEditorId,
    selectedApiConfig: configDerived.selectedApiConfig,
    toolApiConfig: contentOrchestrator.personaConversation.toolApiConfig,
    modelRefreshError,
    toolStatuses,
  });

  function notifySidebarCodeReview() {
    void emitTransportEvent("codeReview.requested");
  }

  async function rebindConversationRecipient(payload: { conversationId: string; agentId: string }) {
    const conversationId = String(payload?.conversationId || "").trim();
    const agentId = String(payload?.agentId || "").trim();
    if (!conversationId || !agentId) return;
    try {
      const result = await invokeTauri<{
        conversationId: string;
        agentId: string;
        preferredApiConfigId?: string | null;
      }>("conversation.rebindRecipient", {
        input: { conversationId, agentId },
      });
      if (String(currentChatConversationId.value || "").trim() === conversationId) {
        currentChatPreferredApiConfigId.value = String(result.preferredApiConfigId || "").trim();
      }
      await syncUnarchivedConversationOverviewChangedSinceWatermark("rebind_conversation_recipient");
      setStatus(t("status.conversationRecipientRebound"));
    } catch (error) {
      setStatusError("status.rebindConversationRecipientFailed", error);
    }
  }

  function updateUiSizeScale(value: unknown) {
    config.uiSizeScale = setUiSizeScale(value);
  }

  watch(uiSizeScale, (scale) => {
    if (config.uiSizeScale !== scale) {
      config.uiSizeScale = scale;
    }
  });

  // 录音/转写状态只走窗口级 toast（原 ChatView 输入区 infobar 已移除）
  const statusToastText = computed(() => {
    if (chatMedia.transcribing.value) return tr("chat.transcribing");
    if (chatMedia.recording.value) {
      return tr("chat.recording", { seconds: Math.max(1, Math.round(chatMedia.recordingMs.value / 1000)) });
    }
    return status.value;
  });
  const statusToastTone = computed(() =>
    chatMedia.recording.value || chatMedia.transcribing.value ? "default" : statusTone.value,
  );

  return {
    messageText,
    extractMessageImages,
    statusToastText,
    statusToastTone,
    extractMessageAttachmentFiles,
    viewMode,
    t,
    locale,
    tr,
    windowReady,
    maximized,
    startDrag,
    toggleMaximizeWindow,
    currentTheme,
    generatedThemeControls,
    generatedThemeTokens,
    generatedLightTokens,
    generatedDarkTokens,
    themeMode,
    autoLightTheme,
    autoDarkTheme,
    setTheme,
    setThemeMode,
    setAutoTheme,
    activateGeneratedTheme,
    updateGeneratedThemeControls,
    resetGeneratedTheme,
    config,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    personas,
    assistantAgentId,
    personaEditorId,
    userAlias,
    selectedResponseStyleId,
    selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    instructionPresets,
    latestUserText,
    latestUserImages,
    latestOwnMessageAlignRequest,
    latestContextUsagePreview,
    clipboardImages,
    queuedAttachmentNotices,
    status,
    statusTone,
    terminalApprovalResolving,
    loading,
    saving,
    startupDataReady,
    startupOverlayVisible,
    messageStoreMigration,
    chatting,
    trimming,
    compactingConversation,
    trimmingConversationId,
    compactingConversationId,
    branchingConversation,
    forwardingConversationSelection,
    hasMoreBackendHistory,
    loadingOlderConversationHistory,
    refreshingModels,
    modelRefreshError,
    toolStatuses,
    avatarSaving,
    avatarError,
    personaSaving,
    lastSavedConfigJson,
    setStatus,
    localeOptions,
    updateUiSizeScale,
    updateGithubUpdateMethod,
    toolReviewRefreshTick,
    currentChatTodos,
    ...runtimeOrchestrator.chatRuntimeSetup,
    chatFlow,
    ...chatUiState,
    ...shellData.githubUpdate,
    ...shellData.archivesView,
    ...shellData.conversationPlanMode,
    ...shellData.goalTask,
    ...shellData.archiveImport,
    ...configDerived,
    ...configCore,
    ...configUi,
    ...configActions,
    ...contentOrchestrator.conversationItems,
    ...contentOrchestrator.personaConversation,
    ...contentOrchestrator.messageBlocks,
    ...contentOrchestrator.terminalApproval,
    ...contentOrchestrator.basicState,
    ...contentOrchestrator.localTools,
    ...chatMedia,
    ...workspaceOrchestrator,
    ...configRuntime,
    ...configPersistence,
    ...configEditors,
    ...promptPreviewActions,
    ...memoryViewerActions,
    ...shellDialogFlows,
    ...chatDialogActions,
    ...conversationOrchestrator,
    sideConversations,
    sideConversationId,
    selectSideChatConversation,
    openSideChatNewPage,
    createSideChatConversation,
    createSideConversationBranchFromTurn,
    closeSideChatConversations,
    openSettingsWindow,
    summonChatWindowFromConfig,
    minimizeWindowAndClearForeground,
    openGithubRepository,
    conversationScrollToBottomRequest,
    scrollToBottomBehavior,
    handleConfirmPlan,
    deleteUnarchivedConversation,
    rebindConversationRecipient,
    handleCreateConversationBranchFromTurn,
    handleRecallTurn,
    handleRegenerateTurn,
    setMemoryDialogRef,
    setPromptPreviewDialogRef,
    confirmMessageStoreMigrationSummary,
    retryMessageStoreMigration,
    notifySidebarCodeReview,
  };
}
