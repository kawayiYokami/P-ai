import { ref } from "vue";
import type { ChatMessage } from "../../../types/app";
import { invokeTauri } from "../../../services/tauri-api";

type UseHistoryViewerOptions = {
  setStatusError: (key: string, error: unknown) => void;
};

export function useHistoryViewer(options: UseHistoryViewerOptions) {
  const historyDialog = ref<HTMLDialogElement | null>(null);
  const currentHistory = ref<ChatMessage[]>([]);

  async function openCurrentHistory(apiConfigId: string, agentId: string) {
    try {
      currentHistory.value = await invokeTauri<ChatMessage[]>("get_active_conversation_messages", {
        input: { apiConfigId, agentId },
      });
      historyDialog.value?.showModal();
    } catch (e) {
      options.setStatusError("status.loadHistoryFailed", e);
    }
  }

  function closeHistory() {
    historyDialog.value?.close();
  }

  return {
    historyDialog,
    currentHistory,
    openCurrentHistory,
    closeHistory,
  };
}


