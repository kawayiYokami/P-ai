import { onScopeDispose } from "vue";
import {
  bindTransportConversationStream,
  invokeTauri,
  onTransportNotification,
  probeTransportConversationStream,
  unbindTransportConversationStream,
} from "../../../services/tauri-api";
import { registerChatFlowRuntime } from "./chat-flow-runtime-registry";
import { invokeSendChatMessage, invokeStopChatMessage } from "./chat-send-transport";
import type { ChatRewindCompletedPayload } from "../../../types/app";
import { useChatFlow } from "./use-chat-flow";
import { useChatForegroundRuntime } from "./use-chat-foreground-runtime";
import { useChatRewindActions } from "./use-chat-rewind-actions";
import { useConfirmPlan } from "./use-confirm-plan";

export function useChatRuntimeSetup(bindings: Record<string, any>) {
  let chatFlowRef: any = null;

  const chatFlow = useChatFlow({
      subscribeExternalEvents: (method, handler) => onTransportNotification(method, handler),
      chatting: bindings.chatting,
      trimming: bindings.trimming,
      isConversationBusy: () => {
        const conversationId = String(bindings.currentChatConversationId.value || "").trim();
        if (!conversationId) return false;
        const runtimeState = String(
          typeof bindings.currentConversationRuntimeState === "function"
            ? bindings.currentConversationRuntimeState(conversationId)
            : "",
        ).trim();
        if (runtimeState === "assistant_streaming" || runtimeState === "organizing_context" || runtimeState === "compacting") {
          return true;
        }
        const trimmingId = String(bindings.trimmingConversationId?.value || "").trim();
        if (bindings.trimming?.value && (!trimmingId || trimmingId === conversationId)) {
          return true;
        }
        const compactingId = String(bindings.compactingConversationId?.value || "").trim();
        return !!bindings.compactingConversation?.value && (!compactingId || compactingId === conversationId);
      },
      getSession: () => {
        const apiConfigId = String(bindings.currentForegroundApiConfigId.value || "").trim();
        const agentId = String(bindings.currentForegroundAgentId.value || "").trim();
        const departmentId = String(bindings.currentForegroundDepartmentId.value || "").trim();
        if (!apiConfigId || !agentId) return null;
        return { apiConfigId, agentId, departmentId };
      },
      getConversationId: () => String(bindings.currentChatConversationId.value || "").trim(),
      chatInput: bindings.chatInput,
      selectedMentions: bindings.selectedChatMentions,
      clipboardImages: bindings.clipboardImages,
      queuedAttachmentNotices: bindings.queuedAttachmentNotices,
      latestUserText: bindings.latestUserText,
      latestUserImages: bindings.latestUserImages,
      contextUsagePreview: bindings.latestContextUsagePreview,
      chatErrorText: bindings.chatErrorText,
      setConversationChatError: bindings.setConversationChatErrorText,
      allMessages: bindings.allMessages,
      onOwnUserDraftInserted: ({ conversationId }) => {
        const insertedConversationId = String(conversationId || "").trim();
        if (
          insertedConversationId
          && bindings.isChatWindowActiveNow()
          && !String(bindings.currentChatConversationId.value || "").trim()
        ) {
          bindings.currentChatConversationId.value = insertedConversationId;
        }
        bindings.bumpOwnUserDraftAlign();
        bindings.cacheConversationMessages(
          insertedConversationId || String(bindings.currentChatConversationId.value || "").trim(),
          bindings.allMessages.value,
        );
      },
      onStreamingAssistantBubbleInserted: () => {
        bindings.bumpOwnUserDraftAlign();
      },
      t: bindings.tr,
      formatRequestFailed: (error: unknown) => bindings.formatRequestFailed(error),
      removeBinaryPlaceholders: bindings.removeBinaryPlaceholders,
      invokeSendChatMessage,
      invokeStopChatMessage,
      refreshMessageById: async ({ conversationId, messageId }) => {
        const normalizedMessageId = String(messageId || "").trim();
        const beforeMessage = bindings.allMessages.value.find((message: any) => String(message?.id || "").trim() === normalizedMessageId);
        await bindings.refreshForegroundConversationMessageById({
          conversationId,
          messageId,
        });
        const afterMessage = bindings.allMessages.value.find((message: any) => String(message?.id || "").trim() === normalizedMessageId);
        return !!afterMessage && afterMessage !== beforeMessage;
      },
      invokeBindActiveChatViewStream: bindTransportConversationStream,
      invokeUnbindActiveChatViewStream: unbindTransportConversationStream,
      invokeProbeActiveChatViewStream: probeTransportConversationStream,
      onReloadMessages: () => bindings.reloadForegroundConversationMessages("chat_flow_reload"),
      onAssistantMessageCompleted: async ({ conversationId, assistantMessage }) => {
        bindings.applyConversationMessageAppended({
          conversationId,
          message: assistantMessage,
        });
      },
      onHistoryFlushed: async ({ conversationId, pendingMessages }) => {
        const flushedConversationId = String(conversationId || "").trim();
        if (flushedConversationId && bindings.isChatWindowActiveNow()) {
          bindings.currentChatConversationId.value = flushedConversationId;
        }
        const queueMessages = Array.isArray(pendingMessages) ? pendingMessages : [];
        if (queueMessages.length > 0) {
          bindings.allMessages.value = bindings.mergeMessagesIntoTimeline(
            bindings.allMessages.value,
            queueMessages,
            {
              replaceOptimisticUserDrafts: true,
              summarySeedsFirst: true,
            },
          );
          bindings.foregroundTailLatestReady.value = true;
        }
        bindings.cacheConversationMessages(
          flushedConversationId || String(bindings.currentChatConversationId.value || "").trim(),
          bindings.allMessages.value,
        );
      },
  });
  const confirmPlan = useConfirmPlan({
      currentApiConfigId: bindings.currentForegroundApiConfigId,
      currentAgentId: bindings.currentForegroundAgentId,
      currentDepartmentId: bindings.currentForegroundDepartmentId,
      currentConversationId: bindings.currentChatConversationId,
      chatting: bindings.chatting,
      trimming: bindings.trimming,
      compactingConversation: bindings.compactingConversation,
      setConversationPlanMode: bindings.setConversationPlanMode,
      clearForegroundRuntimeState: () => {
        chatFlowRef?.clearForegroundRuntimeState();
      },
      confirmPlanAndContinue: ({ conversationId, planMessageId, departmentId, agentId }) => invokeTauri<void>("conversation.plan.confirm", {
        input: {
          conversationId,
          planMessageId,
          departmentId: departmentId || null,
          agentId: agentId || null,
        },
      }),
  });
  const rewindActions = useChatRewindActions({
      activeApiConfigId: bindings.currentForegroundApiConfigId,
      activeAgentId: bindings.currentForegroundAgentId,
      currentConversationId: bindings.currentChatConversationId,
      allMessages: bindings.allMessages,
      chatting: bindings.chatting,
      trimming: bindings.trimming,
      compactingConversation: bindings.compactingConversation,
      chatErrorText: bindings.chatErrorText,
      chatInput: bindings.chatInput,
      selectedMentions: bindings.selectedChatMentions,
      clipboardImages: bindings.clipboardImages,
      queuedAttachmentNotices: bindings.queuedAttachmentNotices,
      deleteUnarchivedConversationFromArchives: bindings.deleteUnarchivedConversationFromArchives,
      sendChat: bindings.sendChatFromCurrentWindow,
      setStatusError: bindings.setStatusError,
      setChatErrorText: (text: string) => {
        bindings.chatErrorText.value = text;
      },
      removeBinaryPlaceholders: bindings.removeBinaryPlaceholders,
      messageText: bindings.messageText,
      extractMessageImages: bindings.extractMessageImages,
      extractMessageAttachmentFiles: bindings.extractMessageAttachmentFiles,
      requestRecallMode: bindings.requestRecallMode,
      requestCreateConversationBranchFromMessageConfirm: bindings.requestCreateConversationBranchFromMessageConfirm,
      createConversationBranchFromMessage: bindings.createConversationBranchFromMessage,
      branchingConversation: bindings.branchingConversation,
      refreshForegroundConversationAfterRewind: async (conversationId: string) => {
        const normalizedConversationId = String(conversationId || "").trim();
        if (!normalizedConversationId) return;
        chatFlowRef?.clearForegroundRuntimeState();
        const snapshot = await invokeTauri<any>("conversation.foregroundLightSnapshot", {
          input: {
            conversationId: normalizedConversationId,
            agentId: null,
            limit: bindings.FOREGROUND_SNAPSHOT_RECENT_LIMIT,
          },
        });
        bindings.applyConversationSnapshot(snapshot);
      },
  });

  chatFlowRef = chatFlow;
  const foregroundRuntime = useChatForegroundRuntime({
    ...bindings,
    getChatFlow: () => chatFlow,
    rehydrateTerminalApprovals: (() => {
      const direct = (bindings as unknown as { rehydrateTerminalApprovals?: () => Promise<unknown> }).rehydrateTerminalApprovals;
      if (typeof direct === "function") return direct;
      const fromApproval = (bindings as unknown as { terminalApproval?: { rehydrateTerminalApprovals?: () => Promise<unknown> } }).terminalApproval?.rehydrateTerminalApprovals;
      if (typeof fromApproval === "function") return fromApproval;
      return () => Promise.resolve(0);
    })(),
  });
  const stopRewindCompletedEvent = onTransportNotification<ChatRewindCompletedPayload>(
    "chat.rewindCompleted",
    (payload) => {
      const conversationId = String(payload?.conversationId || "").trim();
      const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
      if (!conversationId || conversationId !== currentConversationId) return;
      const messages = Array.isArray(bindings.allMessages.value) ? [...bindings.allMessages.value] : [];
      if (messages.length === 0) return;
      const remainingLastMessageId = String(payload?.remainingLastMessageId || "").trim();
      const targetMessageId = String(payload?.targetMessageId || "").trim();
      let cutIndex = -1;
      if (remainingLastMessageId) {
        const keepIndex = messages.findIndex((message: any) => String(message?.id || "").trim() === remainingLastMessageId);
        if (keepIndex >= 0) cutIndex = keepIndex + 1;
      }
      if (cutIndex < 0 && targetMessageId) {
        const targetIndex = messages.findIndex((message: any) => String(message?.id || "").trim() === targetMessageId);
        if (targetIndex >= 0) cutIndex = targetIndex;
      }
      // 两个边界 ID 都不在当前已加载切片内时，本地无法安全裁剪（撤回点可能在切片更早处），回源重载权威快照而非静默保留
      if (cutIndex < 0) {
        void bindings.reloadForegroundConversationMessages("chat_rewind_completed_boundary_miss");
        return;
      }
      // 边界已找到但落在切片尾部或之后，本地没有可裁的冗余消息
      if (cutIndex >= messages.length) return;
      bindings.allMessages.value = messages.slice(0, cutIndex);
      bindings.cacheConversationMessages(conversationId, bindings.allMessages.value);
      console.info("[会话撤回] 收到撤回广播，已裁剪本地消息", {
        conversationId,
        targetMessageId,
        remainingLastMessageId,
        cutIndex,
      });
    },
  );
  const unregisterChatFlowRuntime = registerChatFlowRuntime({
    bindingId: chatFlow.bindingId,
    getConversationId: () => String(bindings.currentChatConversationId.value || "").trim(),
    flow: chatFlow,
  });
  onScopeDispose(() => {
    stopRewindCompletedEvent();
    unregisterChatFlowRuntime();
    void chatFlow.unbindActiveConversationStream?.().catch(() => {});
  });

  return {
    chatFlow,
    ...foregroundRuntime,
    handleConfirmPlan: confirmPlan.handleConfirmPlan,
    deleteUnarchivedConversation: rewindActions.deleteUnarchivedConversation,
    handleCreateConversationBranchFromTurn: rewindActions.handleCreateConversationBranchFromTurn,
    handleRecallTurn: rewindActions.handleRecallTurn,
    handleRegenerateTurn: rewindActions.handleRegenerateTurn,
  };
}
