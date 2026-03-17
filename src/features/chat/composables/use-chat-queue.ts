import { ref, onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeTauri } from "../../../services/tauri-api";

export type ChatQueueEvent = {
  id: string;
  source: "user" | "task" | "delegate" | "system" | "remote_im";
  createdAt: string;
  messagePreview: string;
  conversationId: string;
};

export type MainSessionState = "idle" | "assistant_streaming" | "organizing_context";

type ChatQueueSnapshotPush = {
  queueEvents: ChatQueueEvent[];
  sessionState: MainSessionState;
};

function isMainSessionState(value: unknown): value is MainSessionState {
  return value === "idle" || value === "assistant_streaming" || value === "organizing_context";
}

export function useChatQueue() {
  const queueEvents = ref<ChatQueueEvent[]>([]);
  const sessionState = ref<MainSessionState>("idle");
  const polling = ref(false);
  let unlisten: UnlistenFn | null = null;

  async function refreshQueue() {
    try {
      const events = await invokeTauri<ChatQueueEvent[]>("get_chat_queue_snapshot");
      queueEvents.value = events || [];
    } catch (error) {
      console.error("[CHAT-QUEUE] Failed to refresh queue:", error);
      queueEvents.value = [];
    }
  }

  async function refreshSessionState() {
    try {
      const state = await invokeTauri<MainSessionState>("get_main_session_state_snapshot");
      sessionState.value = state || "idle";
    } catch (error) {
      console.error("[CHAT-QUEUE] Failed to refresh session state:", error);
    }
  }

  async function removeFromQueue(eventId: string): Promise<boolean> {
    try {
      const removed = await invokeTauri<boolean>("remove_chat_queue_event", { eventId });
      if (removed) {
        await refreshQueue();
      }
      return removed;
    } catch (error) {
      console.error("[CHAT-QUEUE] Failed to remove event:", error);
      return false;
    }
  }

  async function startPolling() {
    if (polling.value) return;
    polling.value = true;

    try {
      await refreshQueue();
      await refreshSessionState();
      unlisten = await listen<ChatQueueSnapshotPush>("easy-call:chat-queue-snapshot", (event) => {
        const payload = event.payload;
        queueEvents.value = Array.isArray(payload?.queueEvents) ? payload.queueEvents : [];
        sessionState.value = isMainSessionState(payload?.sessionState) ? payload.sessionState : "idle";
      });
    } catch (error) {
      polling.value = false;
      unlisten = null;
      throw error;
    }
  }

  function stopPolling() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    polling.value = false;
  }

  onMounted(() => {
    void startPolling();
  });

  onUnmounted(() => {
    stopPolling();
  });

  return {
    queueEvents,
    sessionState,
    polling,
    refreshQueue,
    refreshSessionState,
    removeFromQueue,
    startPolling,
    stopPolling,
  };
}
