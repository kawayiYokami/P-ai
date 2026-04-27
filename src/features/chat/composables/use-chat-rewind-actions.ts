import { type Ref, type ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMentionTarget, ChatMessage } from "../../../types/app";

type RewindConversationResult = {
  removedCount: number;
  remainingCount: number;
  recalledUserMessage?: ChatMessage;
};

type RecallConfirmMode = "with_patch" | "message_only" | "cancel";

type UseChatRewindActionsOptions = {
  activeApiConfigId: Ref<string>;
  activeAgentId: Ref<string>;
  currentConversationId: Ref<string>;
  allMessages: ShallowRef<ChatMessage[]>;
  maybeUpdateConversationOverviewFromLoadedMessages: (
    conversationId: string,
    messages: ChatMessage[],
    remainingCount: number,
  ) => void;
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  compactingConversation: Ref<boolean>;
  chatInput: Ref<string>;
  selectedMentions: Ref<ChatMentionTarget[]>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>;
  deleteUnarchivedConversationFromArchives: (conversationId: string) => Promise<void>;
  sendChat: () => Promise<void>;
  setStatusError: (key: string, error: unknown) => void;
  setChatErrorText: (text: string) => void;
  removeBinaryPlaceholders: (text: string) => string;
  messageText: (message: ChatMessage) => string;
  extractMessageImages: (message: ChatMessage) => Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  requestRecallMode: (payload: { turnId: string }) => Promise<RecallConfirmMode>;
  stopChat: () => Promise<void> | void;
  clearForegroundRuntimeState: () => void;
};

export function useChatRewindActions(options: UseChatRewindActionsOptions) {
  function extractRecallableImages(message: ChatMessage): Array<{ mime: string; bytesBase64: string; savedPath?: string }> {
    return options.extractMessageImages(message)
      .filter((image) => !!String(image.bytesBase64 || "").trim())
      .map((image) => ({
        mime: image.mime,
        bytesBase64: String(image.bytesBase64 || "").trim(),
      }));
  }

  function errorText(error: unknown): string {
    if (typeof error === "string") return error;
    if (error && typeof error === "object") {
      const maybeMessage = (error as { message?: unknown }).message;
      if (typeof maybeMessage === "string" && maybeMessage.trim()) return maybeMessage.trim();
      try {
        return JSON.stringify(error);
      } catch {
        return String(error);
      }
    }
    return String(error);
  }

  function extractRecallableMentions(message: ChatMessage): ChatMentionTarget[] {
    const providerMeta = (message.providerMeta || {}) as Record<string, unknown>;
    const messageMeta = ((providerMeta.message_meta || providerMeta.messageMeta || {}) as Record<string, unknown>);
    const raw = Array.isArray(messageMeta.mentions) ? messageMeta.mentions : [];
    const seen = new Set<string>();
    const mentions: ChatMentionTarget[] = [];
    for (const item of raw) {
      if (!item || typeof item !== "object") continue;
      const entry = item as Record<string, unknown>;
      const agentId = String(entry.agentId || "").trim();
      const departmentId = String(entry.departmentId || "").trim();
      if (!agentId || !departmentId) continue;
      const dedupKey = `${agentId}::${departmentId}`;
      if (seen.has(dedupKey)) continue;
      seen.add(dedupKey);
      mentions.push({
        agentId,
        agentName: String(entry.agentName || agentId).trim() || agentId,
        departmentId,
        departmentName: String(entry.departmentName || departmentId).trim() || departmentId,
        avatarUrl: undefined,
      });
    }
    return mentions;
  }

  function resolveRewindTargetUserMessage(currentMessages: ChatMessage[], turnId: string): { targetUserMessageId: string; keepCountFromLocal: number } | null {
    const turnMessageId = String(turnId || "").trim();
    if (!turnMessageId) return null;
    const directIndex = currentMessages.findIndex((item) => item.id === turnMessageId);
    if (directIndex < 0) return null;
    const directRole = String(currentMessages[directIndex]?.role || "").trim();
    if (directRole === "user") {
      return {
        targetUserMessageId: turnMessageId,
        keepCountFromLocal: directIndex,
      };
    }
    for (let i = directIndex - 1; i >= 0; i -= 1) {
      if (String(currentMessages[i]?.role || "").trim() === "user") {
        return {
          targetUserMessageId: String(currentMessages[i]?.id || "").trim(),
          keepCountFromLocal: i,
        };
      }
    }
    return null;
  }

  async function interruptConversationRuntimeForRewind() {
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!agentId || !conversationId) return;
    try {
      await options.stopChat();
    } catch {
      // Continue with backend cleanup even if local stopChat coordination fails.
    }
    await invokeTauri("interrupt_conversation_runtime", {
      session: {
        apiConfigId: String(options.activeApiConfigId.value || "").trim() || null,
        agentId,
        conversationId,
      },
    });
  }

  async function rewindConversationFromTurn(turnId: string, undoApplyPatch: boolean): Promise<ChatMessage | null> {
    const startedAt = Date.now();
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.currentConversationId.value || "").trim();
    console.info("[会话撤回] 开始执行", {
      turnId,
      undoApplyPatch,
      apiConfigId,
      agentId,
      conversationId: conversationId || "(empty)",
    });
    if (!agentId) {
      console.warn("[会话撤回] 失败：缺少 agentId");
      options.setChatErrorText("撤回失败：缺少会话信息（agentId）");
      options.setStatusError("status.rewindConversationFailed", "缺少会话信息（agentId）");
      return null;
    }
    const currentMessages = [...options.allMessages.value];
    const target = resolveRewindTargetUserMessage(currentMessages, turnId);
    if (!target || !target.targetUserMessageId) {
      console.warn("[会话撤回] 失败：未找到可撤回的用户消息", {
        turnId,
        messageCount: currentMessages.length,
      });
      options.setChatErrorText("撤回失败：未找到可撤回的用户消息");
      options.setStatusError("status.rewindConversationFailed", "未找到可撤回的用户消息");
      return null;
    }
    try {
      console.info("[会话撤回] 调用后端命令", {
        command: "rewind_conversation_from_message",
        targetUserMessageId: target.targetUserMessageId,
        undoApplyPatch,
      });
      const result = await invokeTauri<RewindConversationResult>("rewind_conversation_from_message", {
        input: {
          session: {
            apiConfigId,
            agentId,
            conversationId: conversationId || null,
          },
          messageId: target.targetUserMessageId,
          undoApplyPatch,
        },
      });
      const keepCountFromLocal = target.keepCountFromLocal;
      const keepCountFromBackend = Math.max(0, Math.min(currentMessages.length, Number(result.remainingCount) || 0));
      const keepCount = keepCountFromLocal >= 0 ? keepCountFromLocal : keepCountFromBackend;
      const nextMessages = currentMessages.slice(0, keepCount);
      options.allMessages.value = nextMessages;
      options.maybeUpdateConversationOverviewFromLoadedMessages(
        conversationId,
        nextMessages,
        Number(result.remainingCount) || nextMessages.length,
      );
      console.info("[会话撤回] 完成", {
        removedCount: Number(result.removedCount) || 0,
        remainingCount: Number(result.remainingCount) || nextMessages.length,
        localKeepCount: keepCountFromLocal,
        elapsedMs: Date.now() - startedAt,
      });
      return result.recalledUserMessage
        ?? currentMessages[keepCount]
        ?? currentMessages.find((item) => item.id === target.targetUserMessageId && item.role === "user")
        ?? null;
    } catch (error) {
      const detail = errorText(error);
      console.error("[会话撤回] 失败：后端命令异常", {
        targetUserMessageId: target.targetUserMessageId,
        undoApplyPatch,
        elapsedMs: Date.now() - startedAt,
        error: detail,
      });
      options.setStatusError(
        "status.rewindConversationFailed",
        `撤回失败：${detail || "未知错误"}（可查看运行日志）`,
      );
      options.setChatErrorText(`撤回失败：${detail || "未知错误"}`);
      return null;
    }
  }

  async function deleteUnarchivedConversation(conversationId: string) {
    await options.deleteUnarchivedConversationFromArchives(conversationId);
    if (String(options.currentConversationId.value || "").trim() === String(conversationId || "").trim()) {
      options.currentConversationId.value = "";
      options.allMessages.value = [];
    }
  }

  async function handleRecallTurn(payload: { turnId: string }) {
    console.info("[会话撤回] 点击撤回", {
      turnId: payload?.turnId,
      chatting: options.chatting.value,
      forcingArchive: options.forcingArchive.value,
      compactingConversation: options.compactingConversation.value,
    });
    if (options.forcingArchive.value || options.compactingConversation.value) {
      console.info("[会话撤回] 跳过：当前会话处于忙碌状态", {
        turnId: payload?.turnId,
        forcingArchive: options.forcingArchive.value,
        compactingConversation: options.compactingConversation.value,
      });
      return;
    }
    const mode = await options.requestRecallMode({ turnId: payload.turnId });
    console.info("[会话撤回] 弹窗选择结果", { mode, turnId: payload.turnId });
    if (mode === "cancel") return;
    options.clearForegroundRuntimeState();
    options.setChatErrorText("");
    await interruptConversationRuntimeForRewind();
    const recalledUserMessage = await rewindConversationFromTurn(payload.turnId, mode === "with_patch");
    if (!recalledUserMessage) {
      console.warn("[会话撤回] 结束：未拿到可回填消息", { turnId: payload.turnId, mode });
      const message = mode === "with_patch"
        ? "撤回失败：文件状况改变，修改工具不可逆，请选择仅撤回"
        : "撤回失败：未找到可回填的用户消息";
      options.setChatErrorText(message);
      options.setStatusError("status.rewindConversationFailed", `${message}（可查看运行日志）`);
      return;
    }
    options.chatInput.value = options.removeBinaryPlaceholders(options.messageText(recalledUserMessage));
    options.selectedMentions.value = extractRecallableMentions(recalledUserMessage);
    options.clipboardImages.value = extractRecallableImages(recalledUserMessage);
    console.info("[会话撤回] 已回填输入框", {
      textLength: options.chatInput.value.length,
      mentionCount: options.selectedMentions.value.length,
      imageCount: options.clipboardImages.value.length,
      turnId: payload.turnId,
    });
  }

  async function handleRegenerateTurn(payload: { turnId: string }) {
    if (options.forcingArchive.value || options.compactingConversation.value) return;
    options.clearForegroundRuntimeState();
    await interruptConversationRuntimeForRewind();
    const recalledUserMessage = await rewindConversationFromTurn(payload.turnId, false);
    if (!recalledUserMessage) return;
    options.chatInput.value = options.removeBinaryPlaceholders(options.messageText(recalledUserMessage));
    options.selectedMentions.value = extractRecallableMentions(recalledUserMessage);
    options.clipboardImages.value = extractRecallableImages(recalledUserMessage);
    await options.sendChat();
  }

  return {
    handleRecallTurn,
    handleRegenerateTurn,
    deleteUnarchivedConversation,
  };
}
