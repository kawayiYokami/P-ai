import type { Ref, ShallowRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage, ChatSnapshot } from "../../../types/app";
import { extractMessageImages, removeBinaryPlaceholders, renderMessage } from "../../../utils/chat-message";

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
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  latestAssistantText: Ref<string>;
  allMessages: ShallowRef<ChatMessage[]>;
  visibleTurnCount: Ref<number>;
  perfNow: () => number;
  perfLog: (label: string, startedAt: number) => void;
  perfDebug: boolean;
};

export function useChatRuntime(options: UseChatRuntimeOptions) {
  async function refreshChatSnapshot() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    const startedAt = options.perfNow();
    try {
      const snap = await invokeTauri<ChatSnapshot>("get_chat_snapshot", {
        input: {
          apiConfigId: options.activeChatApiConfigId.value,
          agentId: options.selectedPersonaId.value,
        },
      });
      options.latestUserText.value = snap.latestUser ? removeBinaryPlaceholders(renderMessage(snap.latestUser)) : "";
      options.latestUserImages.value = extractMessageImages(snap.latestUser);
      options.latestAssistantText.value = snap.latestAssistant ? renderMessage(snap.latestAssistant) : "";
    } catch (e) {
      options.setStatusError("status.loadChatSnapshotFailed", e);
    } finally {
      options.perfLog("refreshChatSnapshot", startedAt);
    }
  }

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
      await refreshChatSnapshot();
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

  function loadMoreTurns() {
    options.visibleTurnCount.value++;
  }

  return {
    refreshChatSnapshot,
    forceArchiveNow,
    loadAllMessages,
    loadMoreTurns,
  };
}
