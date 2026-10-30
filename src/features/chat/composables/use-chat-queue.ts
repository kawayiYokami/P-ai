import { computed, ref, onMounted, onUnmounted, type Ref } from "vue";
import { invokeTauri, onTransportNotification } from "../../../services/tauri-api";

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
  notInQueue?: boolean;
};

export type ChatQueueMarkGuidedResult = {
  updated: boolean;
  notInQueue?: boolean;
};

export const CHAT_QUEUE_OUT_OF_SYNC_EVENT = "easy-call:chat-queue-out-of-sync";

export type ChatQueueOutOfSyncDetail = {
  eventId: string;
  conversationId: string;
  reason: "not_in_queue";
};

export function broadcastChatQueueOutOfSync(detail: ChatQueueOutOfSyncDetail) {
  if (typeof window === "undefined" || typeof window.dispatchEvent !== "function") return;
  try {
    window.dispatchEvent(new CustomEvent(CHAT_QUEUE_OUT_OF_SYNC_EVENT, { detail }));
  } catch (error) {
    console.error("[聊天队列] Failed to broadcast out-of-sync:", error);
  }
}

export type MainSessionState = "idle" | "assistant_streaming" | "organizing_context";

type ChatQueueSnapshotPush = {
  queueEvents: ChatQueueEvent[];
  sessionState: MainSessionState;
};

type UseChatQueueOptions = {
  enabled?: Ref<boolean> | boolean;
};

function isMainSessionState(value: unknown): value is MainSessionState {
  return value === "idle" || value === "assistant_streaming" || value === "organizing_context";
}

export function useChatQueue(options: UseChatQueueOptions = {}) {
  const queueEvents = ref<ChatQueueEvent[]>([]);
  const sessionState = ref<MainSessionState>("idle");
  const polling = ref(false);
  const unlisteners: Array<() => void> = [];
  const enabled = computed(() => {
    const configured = options.enabled;
    const configuredValue = typeof configured === "object" && configured && "value" in configured
      ? configured.value
      : configured;
    return configuredValue !== false;
  });

  async function refreshQueue() {
    if (!enabled.value) {
      queueEvents.value = [];
      return;
    }
    try {
      const events = await invokeTauri<ChatQueueEvent[]>("chat.queueSnapshot", {}, 10000);
      queueEvents.value = events || [];
    } catch (error) {
      console.error("[聊天队列] Failed to refresh queue:", error);
      queueEvents.value = [];
    }
  }

  async function refreshSessionState() {
    if (!enabled.value) {
      sessionState.value = "idle";
      return;
    }
    try {
      const state = await invokeTauri<MainSessionState>("chat.sessionStateSnapshot", {}, 10000);
      sessionState.value = state || "idle";
    } catch (error) {
      console.error("[聊天队列] Failed to refresh session state:", error);
    }
  }

  function findQueuedConversationId(eventId: string): string {
    const hit = queueEvents.value.find((event) => event.id === eventId);
    return String(hit?.conversationId || "").trim();
  }

  async function recallQueueEvent(eventId: string): Promise<ChatQueueRecallResult> {
    if (!enabled.value) return { removed: false, messageText: "", notInQueue: false };
    try {
      const raw = await invokeTauri<ChatQueueRecallResult>("chat.queueRecall", { eventId }, 10000);
      const result: ChatQueueRecallResult = {
        removed: !!raw?.removed,
        messageText: String(raw?.messageText || ""),
        notInQueue: raw?.notInQueue === true,
      };
      if (result.removed) {
        await refreshQueue();
        return result;
      }
      // 仅当后端准确反馈不在队列时才做失配双刷，其他失败保持静默。
      if (result.notInQueue) {
        const conversationId = findQueuedConversationId(eventId);
        await refreshQueue();
        broadcastChatQueueOutOfSync({ eventId, conversationId, reason: "not_in_queue" });
      }
      return result;
    } catch (error) {
      console.error("[聊天队列] Failed to recall queue event:", error);
      return { removed: false, messageText: "", notInQueue: false };
    }
  }

  async function markGuided(eventId: string): Promise<ChatQueueMarkGuidedResult> {
    if (!enabled.value) return { updated: false, notInQueue: false };
    try {
      const raw = await invokeTauri<ChatQueueMarkGuidedResult | boolean>("chat.queueMarkGuided", { eventId }, 10000);
      const normalized: ChatQueueMarkGuidedResult = typeof raw === "boolean"
        ? { updated: raw, notInQueue: !raw }
        : { updated: !!raw?.updated, notInQueue: raw?.notInQueue === true };
      if (normalized.updated) {
        await refreshQueue();
        return normalized;
      }
      // 仅当后端准确反馈不在队列时才做失配双刷，其他失败保持静默。
      if (normalized.notInQueue) {
        const conversationId = findQueuedConversationId(eventId);
        await refreshQueue();
        broadcastChatQueueOutOfSync({ eventId, conversationId, reason: "not_in_queue" });
      }
      return normalized;
    } catch (error) {
      console.error("[聊天队列] Failed to mark event guided:", error);
      return { updated: false, notInQueue: false };
    }
  }

  async function startPolling() {
    if (polling.value || !enabled.value) return;
    polling.value = true;

    try {
      await refreshQueue();
      await refreshSessionState();
      const applyQueueSnapshot = (payload: ChatQueueSnapshotPush | undefined | null) => {
        queueEvents.value = Array.isArray(payload?.queueEvents) ? payload.queueEvents : [];
        sessionState.value = isMainSessionState(payload?.sessionState) ? payload.sessionState : "idle";
      };
      const refreshRuntimeSnapshot = () => {
        void refreshQueue();
        void refreshSessionState();
      };
      unlisteners.push(onTransportNotification("chat.queueSnapshotUpdated", (payload) => {
        applyQueueSnapshot(payload as ChatQueueSnapshotPush);
      }));
      unlisteners.push(onTransportNotification("chat.roundStarted", refreshRuntimeSnapshot));
      unlisteners.push(onTransportNotification("chat.roundFinished", refreshRuntimeSnapshot));
      // 历史落地意味着队列已被消费：切会话期间错过快照推送时，靠它把队列刷掉，
      // 否则切回会看到历史已刷出、队列还挂着的 ghost。
      unlisteners.push(onTransportNotification("chat.historyFlushed", refreshRuntimeSnapshot));
      // 浏览器切后台/息屏/弱网可能丢推送：回焦、可见、重连时补拉一次。
      if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
        const handleWindowReturn = () => {
          if (typeof document !== "undefined" && document.hidden) return;
          void refreshQueue();
        };
        const handleOnline = () => {
          void refreshQueue();
        };
        window.addEventListener("focus", handleWindowReturn);
        window.addEventListener("online", handleOnline);
        const handleVisibility = () => {
          if (typeof document !== "undefined" && !document.hidden) void refreshQueue();
        };
        if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
          document.addEventListener("visibilitychange", handleVisibility);
          unlisteners.push(() => document.removeEventListener("visibilitychange", handleVisibility));
        }
        unlisteners.push(() => window.removeEventListener("focus", handleWindowReturn));
        unlisteners.push(() => window.removeEventListener("online", handleOnline));
      }
    } catch (error) {
      polling.value = false;
      while (unlisteners.length > 0) {
        const stop = unlisteners.pop();
        stop?.();
      }
      throw error;
    }
  }

  function stopPolling() {
    while (unlisteners.length > 0) {
      const stop = unlisteners.pop();
      stop?.();
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
