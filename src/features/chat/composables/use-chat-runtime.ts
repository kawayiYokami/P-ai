import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";
import { ensureConversationMessageIds } from "../utils/message-id";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

const FOREGROUND_SNAPSHOT_RECENT_LIMIT = 4;

type ConversationCommandStatus = {
  success: boolean;
};

type UseChatRuntimeOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  setChatError: (text: string) => void;
  setConversationRuntimeState?: (conversationId: string, runtimeState: "idle" | "assistant_streaming" | "organizing_context") => void;
  activeChatApiConfigId: Ref<string>;
  assistantAgentId: Ref<string>;
  currentConversationId?: Ref<string>;
  trimmingConversationId?: Ref<string>;
  compactingConversationId?: Ref<string>;
  chatting: Ref<boolean>;
  trimming: Ref<boolean>;
  compactingConversation: Ref<boolean>;
  suppressNextCompactionReload?: Ref<boolean>;
  allMessages: ShallowRef<ChatMessage[]>;
  refreshUnarchivedConversations?: () => Promise<void>;
  syncUnarchivedConversations?: (reason: string) => Promise<void>;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

type ConversationMaintenanceAction = {
  command: "conversation.archive" | "conversation.compact";
  runningKey: string;
  doneKey: string;
  failedKey: string;
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
    targetConversationId?: string | null,
  ) {
    const currentConversationId = currentConversationIdOrNull();
    const sourceConversationId = String(targetConversationId || currentConversationId || "").trim() || null;
    const targetIsForeground = !currentConversationId || !sourceConversationId || sourceConversationId === currentConversationId;
    const shouldLockForeground = action.lockForeground && targetIsForeground;
    const instantArchiveAction = action.command === "conversation.archive";
    if (!sourceConversationId) {
      const text = options.t("status.conversationActionNoTarget");
      options.setStatus(text);
      options.setChatError(text);
      console.warn("[会话归档] 跳过，缺少 conversationId", {
        command: action.command,
        currentConversationId,
        targetConversationId,
      });
      return;
    }
    if (targetIsForeground && options.compactingConversation.value) {
      const text = options.t("status.conversationActionInProgress");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (targetIsForeground && options.trimming.value) {
      const text = options.t("status.conversationActionInProgress");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }
    if (targetIsForeground && options.chatting.value && action.command !== "conversation.archive") {
      const text = options.t("status.conversationActionBusy");
      options.setStatus(text);
      options.setChatError(text);
      return;
    }

    options.setStatus(instantArchiveAction ? options.t(action.runningKey) : "");
    options.setChatError("");
    console.info("[会话归档] 开始执行会话维护", {
      command: action.command,
      conversationId: sourceConversationId,
      foreground: targetIsForeground,
    });
    if (shouldLockForeground) {
      options.trimming.value = true;
      if (options.trimmingConversationId) {
        options.trimmingConversationId.value = sourceConversationId || "";
      }
    } else if (!action.lockForeground) {
      options.compactingConversation.value = true;
      if (options.compactingConversationId) {
        options.compactingConversationId.value = sourceConversationId || "";
      }
    }
    try {
      const result = await invokeTauri<ConversationCommandStatus>(action.command, {
        input: {
          conversationId: sourceConversationId,
        },
      });
      if (!result.success) {
        const text = options.t(action.failedKey, { err: "command returned unsuccessful status" });
        options.setChatError("");
        options.setStatus(text);
      } else {
        options.setStatus(options.t(action.doneKey, { count: 0 }));
        options.setChatError("");
      }
      if (options.syncUnarchivedConversations) {
        // 归档/压缩后差量同步收敛，避免全量拉取列表。
        await options.syncUnarchivedConversations(
          action.command === "conversation.archive" ? "conversation_archived" : "conversation_compacted",
        );
      } else if (options.refreshUnarchivedConversations) {
        await options.refreshUnarchivedConversations();
      }
      if (action.lockForeground && !targetIsForeground) {
        return;
      }
      if (!action.lockForeground && currentConversationIdOrNull() !== sourceConversationId) {
        return;
      }
      await loadAllMessages(action.lockForeground ? undefined : sourceConversationId);
    } catch (e) {
      const rawErrorText = String(e ?? "");
      console.warn(`[会话归档] 会话维护失败: command=${action.command}, conversationId=${sourceConversationId || ""}, error=${rawErrorText}`, {
        command: action.command,
        conversationId: sourceConversationId,
        error: e,
      });
      const errText = rawErrorText;
      if (errText.includes("活动对话已变化")) {
        const text = options.t("status.conversationActionConflict");
        options.setStatus(text);
        options.setChatError(text);
      } else {
        options.setStatusError(action.failedKey, e);
        options.setChatError(options.t(action.failedKey, { err: rawErrorText }));
      }
    } finally {
      if (shouldLockForeground) {
        options.trimming.value = false;
        if (options.trimmingConversationId) {
          options.trimmingConversationId.value = "";
        }
      } else if (!action.lockForeground) {
        options.compactingConversation.value = false;
        if (options.compactingConversationId) {
          options.compactingConversationId.value = "";
        }
      }
    }
  }

  async function trimNow(targetConversationId?: string | null) {
    await runConversationMaintenance({
      command: "conversation.archive",
      runningKey: "status.trimArchiveRunning",
      doneKey: "status.trimArchiveDone",
      failedKey: "status.trimArchiveFailed",
      lockForeground: true,
    }, targetConversationId);
  }

  async function trimCompactNow() {
    await runConversationMaintenance({
      command: "conversation.compact",
      runningKey: "status.trimCompactRunning",
      doneKey: "status.trimCompactDone",
      failedKey: "status.trimCompactFailed",
      lockForeground: false,
    });
  }

  async function loadAllMessages(targetConversationId?: string | null) {
    if (!options.activeChatApiConfigId.value || !options.assistantAgentId.value) return;
    const startedAt = options.perfNow();
    try {
      const conversationId = String(targetConversationId || currentConversationIdOrNull() || "").trim() || null;
      const snapshot = await invokeTauri<{ messages: ChatMessage[] }>("conversation.foregroundLightSnapshot", {
        input: {
          agentId: conversationId ? null : options.assistantAgentId.value,
          conversationId,
          limit: FOREGROUND_SNAPSHOT_RECENT_LIMIT,
        },
      });
      const msgs = ensureConversationMessageIds(Array.isArray(snapshot?.messages) ? snapshot.messages : []);
      if (options.perfDebug) console.log(`[性能] loadAllMessages count=${msgs.length}`);
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
    trimNow,
    trimCompactNow,
    loadAllMessages,
  };
}
