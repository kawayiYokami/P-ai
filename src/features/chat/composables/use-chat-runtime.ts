import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type ForceArchiveResult = {
  archived: boolean;
  archiveId?: string | null;
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
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

export function useChatRuntime(options: UseChatRuntimeOptions) {
  function currentConversationIdOrNull(): string | null {
    const value = String(options.currentConversationId?.value || "").trim();
    return value || null;
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
      options.visibleMessageBlockCount.value = 1;
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
    options.visibleMessageBlockCount.value++;
  }

  return {
    refreshConversationHistory,
    forceArchiveNow,
    loadAllMessages,
    loadMoreMessageBlocks,
  };
}

