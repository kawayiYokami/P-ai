import type { Ref } from "vue";
import type { ChatIngressPart, ChatMentionTarget, ChatMessage } from "../../../types/app";
import type { TransportChannel } from "../../../services/tauri-api";
import type { PreparedChatSendInput } from "./use-chat-flow-send-input";
import type { RoundState, SendChatOverrides } from "./use-chat-flow-types";
import type { AssistantDeltaEvent } from "./use-chat-flow-events";
import { isChatAbortedByUser } from "./use-chat-flow-utils";

type StreamUserImageAttachment = { mime: string; bytesBase64: string; savedPath?: string };

export function buildChatIngressParts(
  text: string,
  images: StreamUserImageAttachment[],
  attachments: Array<{ fileName: string; path: string; mime: string }>,
): ChatIngressPart[] {
  const parts: ChatIngressPart[] = [];
  const normalizedText = String(text || "");
  if (normalizedText) parts.push({ type: "text", text: normalizedText });
  const seenAttachmentPaths = new Set<string>();
  for (const image of images) {
    const path = String(image.savedPath || "").trim().replace(/\\/g, "/");
    const mime = String(image.mime || "").trim();
    if (!mime) continue;
    if (path) {
      seenAttachmentPaths.add(path.toLowerCase());
      parts.push({
        type: "attachment",
        path,
        mime,
        name: path.split("/").pop() || "image",
      });
      continue;
    }
    const bytesBase64 = String(image.bytesBase64 || "").trim();
    if (!bytesBase64) continue;
    parts.push({ type: "attachment", bytesBase64, mime, name: "image" });
  }
  for (const attachment of attachments) {
    const path = String(attachment.path || "").trim().replace(/\\/g, "/");
    if (!path || seenAttachmentPaths.has(path.toLowerCase())) continue;
    seenAttachmentPaths.add(path.toLowerCase());
    parts.push({
      type: "attachment",
      path,
      mime: String(attachment.mime || "").trim(),
      name: String(attachment.fileName || "").trim() || path.split("/").pop() || "attachment",
    });
  }
  return parts;
}

type UseChatFlowSendControllerOptions = {
  chatting: Ref<boolean>;
  submitPending?: Ref<boolean>;
  isConversationBusy?: () => boolean;
  getConversationId?: () => string;
  getSession: () => { apiConfigId: string; agentId: string } | null;
  /** Web 端专用：发起对话前幂等补绑定，确保后端已注册本客户端的会话订阅。桌面端不注入，避免与 sendChat 原生 Channel 双写。 */
  bindActiveConversationStream?: (conversationId: string, force?: boolean) => Promise<void>;
  createSendChatDeltaChannel: (gen: number, conversationId: string) => TransportChannel<AssistantDeltaEvent>;
  invokeSendChatMessage: (input: {
    text: string;
    displayText?: string;
    parts: ChatIngressPart[];
    extraTextBlocks?: string[];
    mentions?: ChatMentionTarget[];
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    traceId: string;
    onDelta: TransportChannel<AssistantDeltaEvent>;
  }) => Promise<{
    accepted: boolean;
    duplicate: boolean;
    eventId: string;
    conversationId: string;
    traceId: string;
    ingress: string;
    userMessageId?: string;
    assistantMessageId?: string;
  }>;
  onOwnUserDraftInserted?: (payload: { conversationId: string; messageId: string }) => void;
  onStreamingAssistantBubbleInserted?: () => void;
  t: (key: string, params?: Record<string, unknown>) => string;
  getRound: () => RoundState;
  setRound: (next: RoundState) => void;
  setBoundDisplayGeneration?: (gen: number) => void;
  nextGeneration: () => number;
  setSendChatActiveGen: (gen: number) => void;
  /** 提交期是否已被停止：为真时跳过本轮助理轮次/气泡的建立（用户消息投影仍保留）。 */
  isSendStopped?: (gen: number) => boolean;
  /** 清掉已消费的停止标记，避免集合随发送次数增长。 */
  clearSendStopped?: (gen: number) => void;
  /** 记下这次发送使用的请求号，供提交期停止指认它。 */
  recordSendRequestId?: (gen: number, requestId: string) => void;
  setActiveActivationId: (value: string) => void;
  setActiveRoundAgentId: (value: string) => void;
  setPendingTerminalEventNull: () => void;
  sendStartedAtMsByGen: Map<number, number>;
  startFrontendDispatchTimer: (gen: number, startedAtMs?: number, elapsedMs?: number) => void;
  clearFrontendDispatchTimer: () => void;
  clearConversationStreamCache: (conversationId?: string | null) => void;
  clearChatErrorText: (conversationId?: string | null) => void;
  applyPreparedSendInput: (input: PreparedChatSendInput) => void;
  prepareSendInput: (overrides?: SendChatOverrides) => PreparedChatSendInput | null;
  insertUserDraft: (
    messageId: string,
    gen: number,
    text: string,
    images: StreamUserImageAttachment[],
    attachments: Array<{ fileName: string; path: string; mime: string }>,
    extraTextBlocks: string[],
    mentions: ChatMentionTarget[],
  ) => string;
  resetDisplayState: () => void;
  removeMessage: (messageId: string) => void;
  updateQueuedAssistantMessageStatus: (messageId: string, statusText: string) => void;
  handleRoundCompleted: (gen: number, result: {
    assistantText: string;
    assistantMessage?: ChatMessage;
  }) => Promise<void>;
  sendRecovery: {
    handleAbortedSend: (gen: number, sendConversationId: string) => void;
    handleFailedSend: (
      gen: number,
      error: unknown,
      sendSession: { apiConfigId: string; agentId: string; conversationId?: string },
      sendConversationId: string,
    ) => Promise<void>;
    finalizeSendChat: (gen: number, suppressInitialReload?: boolean) => Promise<void>;
  };
};

export function useChatFlowSendController(options: UseChatFlowSendControllerOptions) {
  async function sendChat(overrides?: SendChatOverrides) {
    const prepared = options.prepareSendInput(overrides);
    if (!prepared) return;
    const {
      plainText,
      displayText,
      selectedMentions,
      extraTextBlocks,
      sentImages,
      attachments,
      sendSession,
      sendConversationId,
    } = prepared;

    const hasForegroundRoundInFlight =
      options.chatting.value
      || options.getRound().phase !== "idle"
      || !!options.isConversationBusy?.();
    if (!hasForegroundRoundInFlight) {
      options.clearConversationStreamCache(sendConversationId);
      options.setActiveActivationId("");
      options.clearChatErrorText(sendConversationId);
    }

    options.applyPreparedSendInput(prepared);

    const gen = options.nextGeneration();
    options.setBoundDisplayGeneration?.(gen);
    options.setSendChatActiveGen(gen);
    options.setActiveRoundAgentId(sendSession.agentId);
    options.sendStartedAtMsByGen.set(gen, Date.now());
    if (!hasForegroundRoundInFlight) {
      options.startFrontendDispatchTimer(gen, options.sendStartedAtMsByGen.get(gen));
    }
    options.setPendingTerminalEventNull();

    const traceId = typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    // 提交期停止要用这个请求号指认「这一次提交」，先记下来。
    options.recordSendRequestId?.(gen, traceId);
    let queuedAssistantMessageId = "";

    if (!hasForegroundRoundInFlight) {
      options.resetDisplayState();
      const round = options.getRound();
      if (round.phase === "streaming") options.removeMessage(round.messageId);
    }

    const shouldBlockStopUntilHistoryFlushed = !hasForegroundRoundInFlight;
    let submitSucceeded = false;

    try {
      const onDelta = options.createSendChatDeltaChannel(gen, sendConversationId);
      // Web 端：sendChat 的 onDelta 会被 WebSocket 序列化丢弃（无法携带 Tauri Channel），
      // 正文 delta 只能走 bound channel。这里幂等补一次绑定（已绑定则跳过），
      // 确保后端已注册本客户端 → conversation 映射，否则 delta 无处投递。
      if (options.bindActiveConversationStream) {
        try {
          await options.bindActiveConversationStream(sendConversationId);
        } catch (bindError) {
          console.error("[聊天] sendChat 前补绑定失败", {
            conversationId: sendConversationId,
            message: String((bindError as { message?: string })?.message ?? bindError ?? ""),
          });
        }
      }
      if (shouldBlockStopUntilHistoryFlushed && options.submitPending) {
        options.submitPending.value = true;
      }
      const ingressParts = buildChatIngressParts(plainText, sentImages, attachments);
      const submitResult = await options.invokeSendChatMessage({
        text: plainText,
        displayText,
        parts: ingressParts,
        extraTextBlocks: extraTextBlocks.length > 0 ? extraTextBlocks : undefined,
        mentions: selectedMentions.length > 0 ? selectedMentions : undefined,
        session: {
          ...sendSession,
          conversationId: sendConversationId,
        },
        traceId,
        onDelta,
      });
      submitSucceeded = true;
      const ingress = String((submitResult as { ingress?: string } | null)?.ingress || "").trim();
      const accepted = typeof (submitResult as { accepted?: unknown } | null)?.accepted === "boolean"
        ? !!(submitResult as { accepted?: boolean }).accepted
        : true;
      const userMessageId = String((submitResult as { userMessageId?: string } | null)?.userMessageId || "").trim();
      const assistantMessageId = String((submitResult as { assistantMessageId?: string } | null)?.assistantMessageId || "").trim();
      const shouldProjectUserMessage = accepted && ingress !== "queued" && !!userMessageId;
      if (shouldProjectUserMessage) {
        options.insertUserDraft(userMessageId, gen, plainText, sentImages, attachments, extraTextBlocks, selectedMentions);
      }
      // 提交窗口内已被停止：后端会跳过这一轮、不再广播任何终态。
      // 这里不再建立助理轮次/空气泡，用户消息投影保留。
      const stoppedDuringSubmit = !!options.isSendStopped?.(gen);
      if (stoppedDuringSubmit) {
        options.sendStartedAtMsByGen.delete(gen);
        if (options.submitPending) options.submitPending.value = false;
      } else if (!hasForegroundRoundInFlight && accepted && ingress !== "queued") {
        if (selectedMentions.length === 0 && assistantMessageId) {
          queuedAssistantMessageId = assistantMessageId;
          options.setRound({ phase: "queued", gen, messageId: queuedAssistantMessageId });
          options.updateQueuedAssistantMessageStatus(queuedAssistantMessageId, options.t("chat.statusPreparingMessage"));
          options.onStreamingAssistantBubbleInserted?.();
        }
      }
      if (shouldProjectUserMessage) {
        options.onOwnUserDraftInserted?.({
          conversationId: String(submitResult.conversationId || sendConversationId || "").trim(),
          messageId: userMessageId,
        });
      }
      if (!stoppedDuringSubmit && !hasForegroundRoundInFlight && (ingress === "queued" || !accepted)) {
        options.removeMessage(queuedAssistantMessageId);
        if (options.getRound().phase !== "idle") {
          options.setRound({ phase: "idle" });
        }
        options.chatting.value = false;
        options.clearFrontendDispatchTimer();
      }
      if ((!accepted || ingress === "queued") && options.submitPending) {
        options.submitPending.value = false;
      }
    } catch (error) {
      if (options.submitPending) options.submitPending.value = false;
      if (isChatAbortedByUser(error)) {
        options.sendRecovery.handleAbortedSend(gen, sendConversationId);
        return;
      }
      await options.sendRecovery.handleFailedSend(gen, error, sendSession, sendConversationId);
    } finally {
      if (!submitSucceeded && options.submitPending) options.submitPending.value = false;
      options.clearSendStopped?.(gen);
      // chat.send 是短提交命令；成功后的轮次收束只由统一 history/round 事件驱动。
    }
  }

  return {
    sendChat,
  };
}
