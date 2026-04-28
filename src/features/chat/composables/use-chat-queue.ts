import { ref, onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeTauri } from "../../../services/tauri-api";

export type ChatQueueEvent = {
  id: string;
  source: "user" | "task" | "delegate" | "system" | "remote_im";
  queueMode: "normal" | "guided";
  createdAt: string;
  messagePreview: string;
  messageText?: string;
  conversationId: string;
};

export type ChatQueueRecallResult = {
  removed: boolean;
  messageText: string;
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

  async function recallQueueEvent(eventId: string): Promise<ChatQueueRecallResult> {
    try {
      const result = await invokeTauri<ChatQueueRecallResult>("recall_chat_queue_event", { eventId });
      if (result?.removed) {
        await refreshQueue();
      }
      return result || { removed: false, messageText: "" };
    } catch (error) {
      console.error("[CHAT-QUEUE] Failed to recall queue event:", error);
      return { removed: false, messageText: "" };
    }
  }

  async function markGuided(eventId: string): Promise<boolean> {
    try {
      const updated = await invokeTauri<boolean>("mark_chat_queue_event_guided", { eventId });
      if (updated) {
        await refreshQueue();
      }
      return updated;
    } catch (error) {
      console.error("[CHAT-QUEUE] Failed to mark event guided:", error);
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
    recallQueueEvent,
    markGuided,
    startPolling,
    stopPolling,
  };
}
