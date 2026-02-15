import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type ForceArchiveResult = {
  archived: boolean;
  archiveId?: string | null;
  summary: string;
  mergedMemories: number;
};

type UseChatRuntimeOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  activeChatApiConfigId: Ref<string>;
  selectedPersonaId: Ref<string>;
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  allMessages: ShallowRef<ChatMessage[]>;
  visibleTurnCount: Ref<number>;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

export function useChatRuntime(options: UseChatRuntimeOptions) {
  async function forceArchiveNow() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    if (options.chatting.value || options.forcingArchive.value) return;
    options.forcingArchive.value = true;
    try {
      const result = await invokeTauri<ForceArchiveResult>("force_archive_current", {
        input: {
          apiConfigId: options.activeChatApiConfigId.value,
          agentId: options.selectedPersonaId.value,
        },
      });
      options.setStatus(
        result.archived ? options.t("status.forceArchiveDone", { count: result.mergedMemories }) : result.summary,
      );
      await loadAllMessages();
      options.visibleTurnCount.value = 1;
    } catch (e) {
      options.setStatusError("status.forceArchiveFailed", e);
    } finally {
      options.forcingArchive.value = false;
    }
  }

  async function loadAllMessages() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    const startedAt = options.perfNow();
    try {
      const msgs = await invokeTauri<ChatMessage[]>("get_active_conversation_messages", {
        input: {
          apiConfigId: options.activeChatApiConfigId.value,
          agentId: options.selectedPersonaId.value,
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

  function loadMoreTurns() {
    options.visibleTurnCount.value++;
  }

  return {
    refreshConversationHistory,
    forceArchiveNow,
    loadAllMessages,
    loadMoreTurns,
  };
}
