import { watch } from "vue";

export function useChatRuntimeWatchers(bindings: Record<string, any>) {
  watch(
    () => ({
      mode: bindings.viewMode.value,
      agentId: String(bindings.currentForegroundAgentId.value || "").trim(),
    }),
    ({ mode }) => {
      if (mode !== "chat" || !bindings.startupDataReady.value) return;
      const syncOverview = typeof bindings.syncUnarchivedConversationOverviewChangedSinceWatermark === "function"
        ? bindings.syncUnarchivedConversationOverviewChangedSinceWatermark
        : bindings.refreshChatUnarchivedConversations;
      void syncOverview("runtime_watcher").catch((error: unknown) => {
        bindings.setStatusError("status.loadMessagesFailed", error);
      });
    },
    { immediate: true },
  );

}
