import { useAvatarCache } from "./use-avatar-cache";
import { useChatConversationItemsDerivedState } from "./use-chat-conversation-items-derived-state";
import { useChatMessageBlocks } from "./use-chat-turns";
import { useChatPersonaConversationDerivedState } from "./use-chat-persona-conversation-derived-state";
import { useChatWindowBasicDerivedState } from "./use-chat-window-basic-derived-state";
import { useConversationPreferredModel } from "./use-conversation-preferred-model";
import { useChatWindowLocalTools } from "./use-chat-window-local-tools";
import { useChatWindowMessageHelpers } from "./use-chat-window-message-helpers";
import { useTerminalApproval } from "../../shell/composables/use-terminal-approval";
import type { Ref } from "vue";

type ChatWindowContentOrchestratorBindings = Record<string, any> & {
  currentChatConversationId: Ref<string>;
  currentChatPreferredApiConfigId: Ref<string>;
  personaDirty: Ref<boolean>;
};

export function useChatWindowContentOrchestrator(bindings: ChatWindowContentOrchestratorBindings) {
  const configDerived = bindings.configDerived;
  const avatarCache = useAvatarCache({ personas: bindings.personas });
  const conversationItems = useChatConversationItemsDerivedState({
    config: bindings.config,
    unarchivedConversations: bindings.unarchivedConversations,
    remoteImContactConversations: bindings.remoteImContactConversations,
    backgroundConversationBadgeMap: bindings.backgroundConversationBadgeMap,
  });
  const personaConversation = useChatPersonaConversationDerivedState({
    t: bindings.t,
    config: bindings.config,
    personas: bindings.personas,
    assistantAgentId: bindings.assistantAgentId,
    personaEditorId: bindings.personaEditorId,
    currentChatConversationId: bindings.currentChatConversationId,
    currentChatPreferredApiConfigId: bindings.currentChatPreferredApiConfigId,
    chatConversationItems: conversationItems.chatConversationItems,
    unarchivedConversations: bindings.unarchivedConversations,
    agentConversationApiConfigId: configDerived.agentConversationApiConfigId,
    agentOrderedApiConfigIds: configDerived.agentOrderedApiConfigIds,
    isTextRequestFormat: configDerived.isTextRequestFormat,
    resolveAvatarUrl: avatarCache.resolveAvatarUrl,
    resolveBrandAvatarUrl: avatarCache.resolveBrandAvatarUrl,
    agentWorkPresence: bindings.agentWorkPresence,
  });
  const messageHelpers = useChatWindowMessageHelpers({
    t: bindings.t,
    userPersona: personaConversation.userPersona,
    userAlias: bindings.userAlias,
    allMessages: bindings.allMessages,
    foregroundTailLatestReady: bindings.foregroundTailLatestReady,
  });
  const localTools = useChatWindowLocalTools({
    t: bindings.t,
    status: bindings.status,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    personaEditorId: bindings.personaEditorId,
    personaDirty: bindings.personaDirty,
    selectedPersonaEditor: personaConversation.selectedPersonaEditor,
    assistantAgentId: bindings.assistantAgentId,
    currentForegroundAgentId: personaConversation.currentForegroundAgentId,
    currentForegroundApiConfig: personaConversation.currentForegroundApiConfig,
    selectedResponseStyleId: bindings.selectedResponseStyleId,
    selectedPdfReadMode: bindings.selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords: bindings.backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode: bindings.backgroundVoiceScreenshotMode,
    instructionPresets: bindings.instructionPresets,
    clipboardImages: bindings.clipboardImages,
    queuedAttachmentNotices: bindings.queuedAttachmentNotices,
    hasVisionFallback: configDerived.hasVisionFallback,
    config: bindings.config,
    applyAgentPrimaryApiConfigLocally: configDerived.applyAgentPrimaryApiConfigLocally,
  });
  const conversationPreferredModel = useConversationPreferredModel({
    config: bindings.config,
    currentChatConversationId: bindings.currentChatConversationId,
    currentChatPreferredApiConfigId: bindings.currentChatPreferredApiConfigId,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    isTextRequestFormat: configDerived.isTextRequestFormat,
  });
  const messageBlocks = useChatMessageBlocks({
    allMessages: bindings.allMessages,
    activeChatApiConfig: personaConversation.currentForegroundApiConfig,
    currentConversationId: bindings.currentChatConversationId,
    contextUsagePreview: bindings.latestContextUsagePreview,
    perfDebug: bindings.PERF_DEBUG,
    perfNow: bindings.perfNow,
    taskTriggerLabels: {
      goal: bindings.tr("config.task.fields.goal"),
      todo: bindings.tr("config.task.fields.todo"),
    },
  });
  const terminalApproval = useTerminalApproval({
    queue: bindings.terminalApprovalQueue,
    resolving: bindings.terminalApprovalResolving,
  });
  // 启动与焦点恢复：后台仍有 pending 但前端队列已空（刷新/失焦）时拉回
  // 去重：contentOrchestrator 可能被 sideChat 等多次实例化，单窗口只注册一次焦点恢复
  if (typeof window !== "undefined" && !(window as unknown as Record<string, unknown>).__ecallTerminalApprovalRehydrateBound) {
    (window as unknown as Record<string, unknown>).__ecallTerminalApprovalRehydrateBound = true;
    const rehydrate = () => { void terminalApproval.rehydrateTerminalApprovals().catch(() => {}); };
    void rehydrate();
    window.addEventListener("focus", rehydrate);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") rehydrate();
    });
  } else {
    void terminalApproval.rehydrateTerminalApprovals().catch(() => {});
  }
  const basicState = useChatWindowBasicDerivedState({
    t: bindings.tr,
    viewMode: bindings.viewMode,
    maximized: bindings.maximized,
    trimming: bindings.trimming,
    compactingConversation: bindings.compactingConversation,
    currentForegroundPersona: personaConversation.currentForegroundPersona,
    currentChatConversationId: bindings.currentChatConversationId,
    visibleMessageBlocks: messageBlocks.visibleMessageBlocks,
    listConversationTerminalApprovals: terminalApproval.listConversationTerminalApprovals,
  });

  return {
    configDerived,
    avatarCache,
    conversationItems,
    personaConversation,
    messageHelpers,
    localTools: {
      ...localTools,
      ...conversationPreferredModel,
    },
    messageBlocks,
    terminalApproval,
    basicState,
  };
}
