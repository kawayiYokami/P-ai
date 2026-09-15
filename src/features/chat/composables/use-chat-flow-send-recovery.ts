import type { Ref } from "vue";
import type { RoundState } from "./use-chat-flow-types";

type SendSession = { apiConfigId: string; agentId: string; conversationId?: string };

type UseChatFlowSendRecoveryOptions = {
  chatting: Ref<boolean>;
  submitPending?: Ref<boolean>;
  reasoningStartedAtMs: Ref<number>;
  getRound: () => RoundState;
  setRound: (next: RoundState) => void;
  getSession: () => SendSession | null;
  getHistoryFlushedReceivedGen: () => number;
  setSendChatActiveGenIfCurrent: (gen: number, value: number) => void;
  setActiveRoundAgentId: (value: string) => void;
  clearFrontendDispatchTimer: () => void;
  clearChatErrorText: (conversationId?: string | null) => void;
  setChatErrorText: (text: string, conversationId?: string | null) => void;
  formatRequestFailed: (error: unknown) => string;
  getPendingUserDraftId: () => string;
  getPendingUserDraftIdForGen: (gen: number) => string;
  removeMessage: (messageId: string) => void;
  deleteSendStartedAtMs: (gen: number) => void;
  failQueuedRoundWithoutMessage: (gen: number, error: unknown) => Promise<void>;
  onReloadMessages: () => Promise<void>;
};

export function useChatFlowSendRecovery(options: UseChatFlowSendRecoveryOptions) {
  function removePendingDraftsForGen(gen: number, assistantMessageId?: string) {
    const pendingUserDraftId = options.getPendingUserDraftIdForGen(gen);
    if (pendingUserDraftId) options.removeMessage(pendingUserDraftId);
    if (assistantMessageId) {
      options.removeMessage(assistantMessageId);
    }
  }

  function handleAbortedSend(gen: number, sendConversationId: string) {
    options.deleteSendStartedAtMs(gen);
    if (options.submitPending) options.submitPending.value = false;
    options.clearChatErrorText(sendConversationId);
    const round = options.getRound();
    if ((round.phase === "streaming" || round.phase === "queued") && round.gen === gen) {
      options.setRound({ phase: "idle" });
      options.setActiveRoundAgentId("");
      options.clearFrontendDispatchTimer();
    }
    options.chatting.value = false;
    options.reasoningStartedAtMs.value = 0;
  }

  async function handleFailedSend(
    gen: number,
    error: unknown,
    sendSession: SendSession,
    sendConversationId: string,
  ) {
    if (options.submitPending) options.submitPending.value = false;
    console.error("[聊天] 聊天流程请求失败", {
      action: "sendChat",
      apiConfigId: sendSession.apiConfigId,
      agentId: sendSession.agentId,
      gen,
      message: String((error as { message?: string })?.message ?? error ?? ""),
    });

    const round = options.getRound();
    if (round.phase === "idle" || round.gen !== gen) {
      removePendingDraftsForGen(gen);
      options.deleteSendStartedAtMs(gen);
      options.clearFrontendDispatchTimer();
      options.setChatErrorText(options.formatRequestFailed(error), sendConversationId);
      return;
    }

    options.setChatErrorText(options.formatRequestFailed(error), sendConversationId);

    const cur = options.getSession();
    if (!cur || cur.apiConfigId !== sendSession.apiConfigId || cur.agentId !== sendSession.agentId) {
      return;
    }

    const latestRound = options.getRound();
    if (latestRound.phase === "streaming" && latestRound.gen === gen) {
      // 有内容保留；空气泡才删。removeMessage 入口也会再拦一层。
      options.removeMessage(latestRound.messageId);
      const pendingUserDraftId = options.getPendingUserDraftIdForGen(gen);
      if (pendingUserDraftId) options.removeMessage(pendingUserDraftId);
      options.deleteSendStartedAtMs(gen);
      options.setRound({ phase: "idle" });
      options.setActiveRoundAgentId("");
      options.clearFrontendDispatchTimer();
      options.chatting.value = false;
      options.reasoningStartedAtMs.value = 0;
    } else if (latestRound.phase === "queued" && latestRound.gen === gen) {
      await options.failQueuedRoundWithoutMessage(gen, error);
    }
  }

  async function finalizeSendChat(gen: number, suppressInitialReload?: boolean) {
    options.setSendChatActiveGenIfCurrent(gen, 0);
    const round = options.getRound();
    if (round.phase === "queued" && round.gen === gen && options.getHistoryFlushedReceivedGen() !== gen) {
      if (options.submitPending) options.submitPending.value = false;
      removePendingDraftsForGen(gen, round.messageId);
      options.deleteSendStartedAtMs(gen);
      options.setRound({ phase: "idle" });
      options.setActiveRoundAgentId("");
      options.clearFrontendDispatchTimer();
      options.chatting.value = false;
      options.reasoningStartedAtMs.value = 0;
      if (!suppressInitialReload) {
        await options.onReloadMessages();
      }
    }
  }

  return {
    finalizeSendChat,
    handleAbortedSend,
    handleFailedSend,
  };
}
