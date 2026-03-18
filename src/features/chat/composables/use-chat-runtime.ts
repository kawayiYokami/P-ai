import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type ForceArchiveResult = {
  archived: boolean;
  archiveId?: string | null;
  activeConversationId?: string | null;
  summary: string;
  mergedMemories: number;
  warning?: string | null;
  reasonCode?: string | null;
  elapsedMs?: number | null;
};

type UseChatRuntimeOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  setChatError: (text: string) => void;
  activeChatApiConfigId: Ref<string>;
  assistantDepartmentAgentId: Ref<string>;
  currentConversationId?: Ref<string>;
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  allMessages: ShallowRef<ChatMessage[]>;
  visibleMessageBlockCount: Ref<number>;
  // 可选：单测场景常只关心 forceArchive / loadAllMessages，不一定传该 ref。
  // 保持可选可以避免测试构造参数缺失时出现 undefined.value 错误。
  hasMoreBackendHistory?: Ref<boolean>;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

export function useChatRuntime(options: UseChatRuntimeOptions) {
  const MESSAGE_BLOCK_LOAD_STEP = 5;

  function setHasMoreBackendHistory(value: boolean) {
    // 统一做空值保护，避免各处直接写 options.hasMoreBackendHistory.value。
    if (options.hasMoreBackendHistory) options.hasMoreBackendHistory.value = value;
  }

  type MessagesBeforeResult = {
    messages: ChatMessage[];
    hasMore: boolean;
  };

  function currentConversationIdOrNull(): string | null {
    const value = String(options.currentConversationId?.value || "").trim();
    return value || null;
  }

  function initialVisibleCount(total: number): number {
    const n = Math.max(0, Math.round(Number(total) || 0));
    if (n <= 0) return 1;
    return Math.min(MESSAGE_BLOCK_LOAD_STEP, n);
  }

  async function forceArchiveNow() {
    const apiConfigId = String(options.activeChatApiConfigId.value || "").trim();
    const agentId = String(options.assistantDepartmentAgentId.value || "").trim();
    if (!apiConfigId || !agentId) {
      const text = options.t("status.forceArchiveNoTarget");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (options.forcingArchive.value) {
      const text = options.t("status.forceArchiveInProgress");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (options.chatting.value) {
      const text = options.t("status.forceArchiveBusy");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }

    options.setStatus(options.t("status.forceArchiveRunning"));
    options.setChatError("");
    options.forcingArchive.value = true;
    try {
      const result = await invokeTauri<ForceArchiveResult>("force_archive_current", {
        input: {
          apiConfigId,
          agentId,
          conversationId: currentConversationIdOrNull(),
        },
      });
      const activeConversationId = String(result.activeConversationId || "").trim();
      if (activeConversationId && options.currentConversationId) {
        const previousConversationId = String(options.currentConversationId.value || "").trim();
        console.info("[CHAT] conversation switched", {
          previousConversationId,
          newConversationId: activeConversationId,
          apiConfigId,
          agentId,
        });
        options.currentConversationId.value = activeConversationId;
      }
      if (result.warning) {
        const detail = `${result.warning}${result.elapsedMs ? ` (${result.elapsedMs}ms)` : ""}`;
        const text = options.t("status.forceArchivePartial", { reason: detail });
        options.setStatus(text);
        options.setChatError(text);
      } else if (result.archived) {
        options.setStatus(options.t("status.forceArchiveDone", { count: result.mergedMemories }));
        options.setChatError("");
      } else {
        options.setStatus(result.summary);
        options.setChatError(result.summary);
      }
      await loadAllMessages();
      options.visibleMessageBlockCount.value = initialVisibleCount(options.allMessages.value.length);
    } catch (e) {
      const errText = String(e ?? "");
      if (errText.includes("活动对话已变化")) {
        const text = options.t("status.forceArchiveConflict");
        options.setStatus(text);
        options.setChatError(text);
      } else {
        options.setStatusError("status.forceArchiveFailed", e);
        options.setChatError(options.t("status.forceArchiveFailed", { err: String(e) }));
      }
    } finally {
      options.forcingArchive.value = false;
    }
  }

  async function loadAllMessages() {
    if (!options.activeChatApiConfigId.value || !options.assistantDepartmentAgentId.value) return;
    const startedAt = options.perfNow();
    try {
      const msgs = await invokeTauri<ChatMessage[]>("get_active_conversation_messages", {
        input: {
          apiConfigId: options.activeChatApiConfigId.value,
          agentId: options.assistantDepartmentAgentId.value,
          conversationId: currentConversationIdOrNull(),
        },
      });
      if (options.perfDebug) console.log(`[PERF] loadAllMessages count=${msgs.length}`);
      options.allMessages.value = msgs;
      options.visibleMessageBlockCount.value = initialVisibleCount(msgs.length);
      setHasMoreBackendHistory(false);
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    } finally {
      options.perfLog("loadAllMessages", startedAt);
    }
  }

  async function refreshConversationHistory() {
    await loadAllMessages();
  }

  function loadMoreMessageBlocks() {
    const conversationId = currentConversationIdOrNull();
    const apiConfigId = String(options.activeChatApiConfigId.value || "").trim();
    const agentId = String(options.assistantDepartmentAgentId.value || "").trim();
    if (!conversationId || !apiConfigId || !agentId) return;

    const blocks = options.allMessages.value;
    if (!Array.isArray(blocks) || blocks.length === 0) {
      void (async () => {
        try {
          const msgs = await invokeTauri<ChatMessage[]>("get_active_conversation_messages", {
            input: {
              apiConfigId,
              agentId,
              conversationId,
            },
          });
          const recent = Array.isArray(msgs) ? msgs.slice(-MESSAGE_BLOCK_LOAD_STEP) : [];
          options.allMessages.value = recent;
          options.visibleMessageBlockCount.value = initialVisibleCount(recent.length);
          setHasMoreBackendHistory(Array.isArray(msgs) && msgs.length > recent.length);
        } catch (e) {
          setHasMoreBackendHistory(false);
          options.setStatusError("status.loadMessagesFailed", e);
        }
      })();
      return;
    }
    const oldest = blocks[0];
    const beforeMessageId = String(oldest?.id || "").trim();
    if (!beforeMessageId || beforeMessageId.startsWith("__draft_assistant__:")) {
      setHasMoreBackendHistory(false);
      return;
    }

    void (async () => {
      try {
        const result = await invokeTauri<MessagesBeforeResult>("get_active_conversation_messages_before", {
          input: {
            session: {
              apiConfigId,
              agentId,
              conversationId,
            },
            beforeMessageId,
            limit: MESSAGE_BLOCK_LOAD_STEP,
          },
        });
        const older = Array.isArray(result?.messages) ? result.messages : [];
        if (older.length === 0) {
          setHasMoreBackendHistory(!!result?.hasMore);
          return;
        }
        const existingIds = new Set(options.allMessages.value.map((item) => String(item?.id || "").trim()).filter(Boolean));
        const prepend = older.filter((item) => {
          const id = String(item?.id || "").trim();
          return !!id && !existingIds.has(id);
        });
        if (prepend.length > 0) {
          options.allMessages.value = [...prepend, ...options.allMessages.value];
        }
        setHasMoreBackendHistory(!!result?.hasMore);
      } catch (e) {
        setHasMoreBackendHistory(false);
        options.setStatusError("status.loadMessagesFailed", e);
      }
    })();
  }

  return {
    refreshConversationHistory,
    forceArchiveNow,
    loadAllMessages,
    loadMoreMessageBlocks,
  };
}
