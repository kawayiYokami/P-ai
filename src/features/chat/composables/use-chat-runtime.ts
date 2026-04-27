import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";
import { ensureConversationMessageIds } from "../utils/message-id";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

const FOREGROUND_SNAPSHOT_RECENT_LIMIT = 4;

type ForceArchiveResult = {
  archived: boolean;
  archiveId?: string | null;
  activeConversationId?: string | null;
  compactionMessage?: ChatMessage | null;
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
  compactingConversation: Ref<boolean>;
  suppressNextCompactionReload?: Ref<boolean>;
  allMessages: ShallowRef<ChatMessage[]>;
  refreshUnarchivedConversations?: () => Promise<void>;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

type ConversationMaintenanceAction = {
  command: "force_archive_current" | "force_compact_current";
  runningKey: string;
  partialKey: string;
  doneKey: string;
  failedKey: string;
  isDone: (result: ForceArchiveResult) => boolean;
  lockForeground: boolean;
};

export function useChatRuntime(options: UseChatRuntimeOptions) {
  const RECENT_MESSAGE_WINDOW = 10;

  function currentConversationIdOrNull(): string | null {
    const value = String(options.currentConversationId?.value || "").trim();
    return value || null;
  }

  async function runConversationMaintenance(
    action: ConversationMaintenanceAction,
    targetConversationId?: string,
  ) {
    const apiConfigId = String(options.activeChatApiConfigId.value || "").trim();
    const agentId = String(options.assistantDepartmentAgentId.value || "").trim();
    if (!apiConfigId || !agentId) {
      const text = options.t("status.conversationActionNoTarget");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (options.forcingArchive.value || options.compactingConversation.value) {
      const text = options.t("status.conversationActionInProgress");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (options.chatting.value) {
      const text = options.t("status.conversationActionBusy");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }

    const sourceConversationId = currentConversationIdOrNull();
    options.setStatus("");
    options.setChatError("");
    if (action.lockForeground) {
      options.forcingArchive.value = true;
    } else {
      options.compactingConversation.value = true;
    }
    try {
      const normalizedTargetConversationId = String(targetConversationId || "").trim();
      const result = await invokeTauri<ForceArchiveResult>(action.command, {
        input: action.command === "force_archive_current"
          ? {
            session: {
              apiConfigId,
              agentId,
              conversationId: sourceConversationId,
            },
            targetConversationId: normalizedTargetConversationId || null,
          }
          : {
            apiConfigId,
            agentId,
            conversationId: sourceConversationId,
          },
      });
      const activeConversationId = String(result.activeConversationId || "").trim();
      if (action.lockForeground && activeConversationId && options.currentConversationId) {
        const previousConversationId = String(options.currentConversationId.value || "").trim();
        console.info("[CHAT] conversation switched", {
          previousConversationId,
          newConversationId: activeConversationId,
          apiConfigId,
          agentId,
        });
        options.currentConversationId.value = activeConversationId;
      }
      if (result.reasonCode === "background_started") {
        options.setStatus(options.t(action.runningKey));
        options.setChatError("");
      } else if (result.warning) {
        const detail = `${result.warning}${result.elapsedMs ? ` (${result.elapsedMs}ms)` : ""}`;
        const text = options.t(action.partialKey, { reason: detail });
        options.setStatus(text);
        options.setChatError(text);
      } else if (action.isDone(result)) {
        options.setStatus(options.t(action.doneKey, { count: result.mergedMemories }));
        options.setChatError("");
      } else {
        options.setStatus(result.summary || "");
        options.setChatError("");
      }
      if (options.refreshUnarchivedConversations) {
        await options.refreshUnarchivedConversations();
      }
      if (!action.lockForeground && result.compactionMessage) {
        const messageId = String(result.compactionMessage.id || "").trim();
        if (messageId && !options.allMessages.value.some((message) => String(message.id || "").trim() === messageId)) {
          options.allMessages.value = [...options.allMessages.value, result.compactionMessage];
        }
        if (options.suppressNextCompactionReload) {
          options.suppressNextCompactionReload.value = true;
        }
        return;
      }
      await loadAllMessages(action.lockForeground ? undefined : sourceConversationId);
    } catch (e) {
      const errText = String(e ?? "");
      if (errText.includes("活动对话已变化")) {
        const text = options.t("status.conversationActionConflict");
        options.setStatus(text);
        options.setChatError(text);
      } else {
        options.setStatusError(action.failedKey, e);
        options.setChatError(options.t(action.failedKey, { err: String(e) }));
      }
    } finally {
      if (action.lockForeground) {
        options.forcingArchive.value = false;
      } else {
        options.compactingConversation.value = false;
      }
    }
  }

  async function forceArchiveNow(targetConversationId?: string) {
    await runConversationMaintenance({
      command: "force_archive_current",
      runningKey: "status.forceArchiveRunning",
      partialKey: "status.forceArchivePartial",
      doneKey: "status.forceArchiveDone",
      failedKey: "status.forceArchiveFailed",
      isDone: (result) => result.archived,
      lockForeground: true,
    }, targetConversationId);
  }

  async function forceCompactNow() {
    await runConversationMaintenance({
      command: "force_compact_current",
      runningKey: "status.forceCompactRunning",
      partialKey: "status.forceCompactPartial",
      doneKey: "status.forceCompactDone",
      failedKey: "status.forceCompactFailed",
      isDone: (result) => !result.reasonCode,
      lockForeground: false,
    });
  }

  async function loadAllMessages(targetConversationId?: string | null) {
    if (!options.activeChatApiConfigId.value || !options.assistantDepartmentAgentId.value) return;
    const startedAt = options.perfNow();
    try {
      const conversationId = String(targetConversationId || currentConversationIdOrNull() || "").trim() || null;
      const snapshot = await invokeTauri<{ messages: ChatMessage[] }>("get_foreground_conversation_light_snapshot", {
        input: {
          agentId: options.assistantDepartmentAgentId.value,
          conversationId,
          limit: FOREGROUND_SNAPSHOT_RECENT_LIMIT,
        },
      });
      const msgs = ensureConversationMessageIds(Array.isArray(snapshot?.messages) ? snapshot.messages : []);
      if (options.perfDebug) console.log(`[PERF] loadAllMessages count=${msgs.length}`);
      const recent = Array.isArray(msgs) ? msgs.slice(-RECENT_MESSAGE_WINDOW) : [];
      options.allMessages.value = recent;
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    } finally {
      options.perfLog("loadAllMessages", startedAt);
    }
  }

  async function refreshConversationHistory() {
    await loadAllMessages();
  }

  return {
    refreshConversationHistory,
    forceArchiveNow,
    forceCompactNow,
    loadAllMessages,
  };
}
