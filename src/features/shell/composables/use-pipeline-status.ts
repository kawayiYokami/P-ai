import { computed, onMounted, onUnmounted, ref, type ComputedRef } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ConversationPipelineStatus = "busy" | "error";

export interface ConversationWorkStatusEvent {
  conversationId?: string;
  conversation_id?: string;
  status: "working" | "completed" | "error";
  requestId?: string;
  request_id?: string;
}

export interface PipelineState {
  conversationStatusById: ComputedRef<Record<string, ConversationPipelineStatus>>;
  markConversationRead: (conversationId: string) => void;
}

export function usePipelineStatus(): PipelineState {
  const conversationStatusByIdRef = ref<Record<string, ConversationPipelineStatus>>({});
  const latestRequestIdByConversation = new Map<string, string>();
  let unlisten: UnlistenFn | null = null;

  function setConversationStatus(conversationId: string, status?: ConversationPipelineStatus) {
    const next = { ...conversationStatusByIdRef.value };
    if (status) {
      next[conversationId] = status;
    } else {
      delete next[conversationId];
    }
    conversationStatusByIdRef.value = next;
  }

  onMounted(async () => {
    unlisten = await listen<ConversationWorkStatusEvent>("conversation_work_status", (event) => {
      const payload = event.payload;
      const conversationId = String(payload.conversationId || payload.conversation_id || "").trim();
      if (!conversationId) return;

      const requestId = String(payload.requestId || payload.request_id || "").trim();
      const latestRequestId = latestRequestIdByConversation.get(conversationId) || "";
      if (requestId) {
        if (latestRequestId && requestId !== latestRequestId && payload.status !== "working") {
          return;
        }
        latestRequestIdByConversation.set(conversationId, requestId);
      }

      if (payload.status === "working") {
        setConversationStatus(conversationId, "busy");
        return;
      }
      if (payload.status === "error") {
        setConversationStatus(conversationId, "error");
        return;
      }
      setConversationStatus(conversationId);
    });
  });

  onUnmounted(() => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  });

  return {
    conversationStatusById: computed(() => conversationStatusByIdRef.value),
    markConversationRead: (conversationId: string) => {
      const normalizedConversationId = String(conversationId || "").trim();
      if (!normalizedConversationId) return;
      setConversationStatus(normalizedConversationId);
    },
  };
}
