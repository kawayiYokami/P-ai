import { computed, ref, watch, type ComputedRef, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessageBlock } from "../../../types/app";

export type ToolReviewStoredReview = {
  kind: string;
  allow: boolean;
  reviewOpinion: string;
  modelName: string;
  rawContent?: string;
};

export type ToolReviewItemSummary = {
  callId: string;
  toolName: string;
  orderIndex: number;
  hasReview: boolean;
};

export type ToolReviewReportSummary = {
  batchKey: string;
  reviewedToolCallIds: string[];
  reportText: string;
  sourceToolCount: number;
  generatedAt: string;
};

export type ToolReviewBatchSummary = {
  batchKey: string;
  userMessageId: string;
  itemCount: number;
  unreviewedCount: number;
  items: ToolReviewItemSummary[];
  report?: ToolReviewReportSummary;
};

export type ToolReviewItemDetail = {
  batchKey: string;
  callId: string;
  messageId: string;
  toolName: string;
  orderIndex: number;
  hasReview: boolean;
  previewKind: string;
  previewText: string;
  resultText: string;
  review?: ToolReviewStoredReview;
};

type ToolReviewBatchListOutput = {
  batches: ToolReviewBatchSummary[];
  currentBatchKey?: string;
};

type SubmitToolReviewBatchOutput = {
  batchKey: string;
  report: ToolReviewReportSummary;
  reportMessageId: string;
};

type UseChatToolReviewOptions = {
  activeConversationId: Ref<string>;
  selectedChatModelId: Ref<string>;
  messageBlocks: ComputedRef<ChatMessageBlock[]>;
  refreshTick: Ref<number>;
  t: (key: string, params?: Record<string, unknown>) => string;
  onRefreshMessage?: (input: { conversationId: string; messageId: string }) => void | Promise<void>;
};

export function useChatToolReview(options: UseChatToolReviewOptions) {
  const toolReviewPanelOpen = ref(false);
  const toolReviewBatches = ref<ToolReviewBatchSummary[]>([]);
  const toolReviewCurrentBatchKey = ref("");
  const toolReviewDetailMap = ref<Record<string, ToolReviewItemDetail>>({});
  const toolReviewDetailLoadingCallId = ref("");
  const toolReviewReviewingCallId = ref("");
  const toolReviewBatchReviewingKey = ref("");
  const toolReviewSubmittingBatchKey = ref("");
  const toolReviewErrorText = ref("");
  const toolReviewReportErrorText = ref("");
  const loadedConversationId = ref("");

  function formatToolReviewError(error: unknown): string {
    const message = error instanceof Error ? String(error.message || "").trim() : String(error);
    const stack = error instanceof Error ? String(error.stack || "").trim() : "";
    if (stack && stack !== message) {
      return message ? `${message}\n${stack}` : stack;
    }
    return message || "Unknown error";
  }

  const toolReviewButtonCount = computed(() => {
    const currentKey = String(toolReviewCurrentBatchKey.value || "").trim();
    if (!currentKey) return 0;
    return toolReviewBatches.value.find((batch) => batch.batchKey === currentKey)?.itemCount || 0;
  });
  const toolReviewHasReviewableContent = computed(() =>
    toolReviewBatches.value.some((batch) => Number(batch.itemCount || 0) > 0)
  );
  const toolReviewButtonLabel = computed(() =>
    options.t("chat.toolReview.button", { count: toolReviewButtonCount.value })
  );
  const toolReviewLoadedForCurrentConversation = computed(
    () => String(loadedConversationId.value || "").trim() === String(options.activeConversationId.value || "").trim()
  );
  const toolReviewButtonEnabled = computed(
    () =>
      !!String(options.activeConversationId.value || "").trim()
      && (!toolReviewLoadedForCurrentConversation.value || toolReviewHasReviewableContent.value)
  );

  async function refreshMessagesAfterReviewMutation(
    conversationId: string,
    messageIds: string[],
  ) {
    if (!options.onRefreshMessage) return;
    const normalizedMessageIds = messageIds
      .map((item) => String(item || "").trim())
      .filter((item, index, list) => !!item && list.indexOf(item) === index);
    for (const messageId of normalizedMessageIds) {
      await options.onRefreshMessage({
        conversationId,
        messageId,
      });
    }
  }

  function resolveValidBatchKey(
    batches: ToolReviewBatchSummary[],
    preferredKey?: string | null,
  ): string {
    const normalizedPreferredKey = String(preferredKey || "").trim();
    if (normalizedPreferredKey && batches.some((batch) => batch.batchKey === normalizedPreferredKey)) {
      return normalizedPreferredKey;
    }
    return String(batches[batches.length - 1]?.batchKey || "").trim();
  }

  async function refreshToolReviewBatches() {
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!conversationId) {
      toolReviewBatches.value = [];
      toolReviewCurrentBatchKey.value = "";
      toolReviewDetailMap.value = {};
      loadedConversationId.value = "";
      return;
    }
    try {
      const result = await invokeTauri<ToolReviewBatchListOutput>("list_tool_review_batches", {
        input: {
          conversationId,
        },
      });
      toolReviewBatches.value = Array.isArray(result?.batches) ? result.batches : [];
      if (!toolReviewBatches.value.some((batch) => Number(batch.itemCount || 0) > 0)) {
        toolReviewPanelOpen.value = false;
      }
      const currentKey = String(toolReviewCurrentBatchKey.value || "").trim();
      toolReviewCurrentBatchKey.value = currentKey
        ? resolveValidBatchKey(toolReviewBatches.value, currentKey)
        : resolveValidBatchKey(toolReviewBatches.value, result?.currentBatchKey);
      const validCallIds = new Set(toolReviewBatches.value.flatMap((batch) => batch.items.map((item) => item.callId)));
      toolReviewDetailMap.value = Object.fromEntries(
        Object.entries(toolReviewDetailMap.value).filter(([callId]) => validCallIds.has(callId))
      );
      loadedConversationId.value = conversationId;
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    }
  }

  async function loadToolReviewItemDetail(callId: string, force = false) {
    const normalizedCallId = String(callId || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedCallId || !conversationId) return null;
    const cachedDetail = toolReviewDetailMap.value[normalizedCallId];
    if (!force && cachedDetail) return cachedDetail;
    toolReviewDetailLoadingCallId.value = normalizedCallId;
    try {
      const detail = await invokeTauri<ToolReviewItemDetail>("get_tool_review_item_detail", {
        input: {
          conversationId,
          callId: normalizedCallId,
        },
      });
      toolReviewDetailMap.value = {
        ...toolReviewDetailMap.value,
        [normalizedCallId]: detail,
      };
      toolReviewErrorText.value = "";
      return detail;
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
      return null;
    } finally {
      if (toolReviewDetailLoadingCallId.value === normalizedCallId) {
        toolReviewDetailLoadingCallId.value = "";
      }
    }
  }

  async function runToolReviewForCall(callId: string) {
    const normalizedCallId = String(callId || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedCallId || !conversationId) return;
    toolReviewReviewingCallId.value = normalizedCallId;
    try {
      const detail = await invokeTauri<ToolReviewItemDetail>("run_tool_review_for_call", {
        input: {
          conversationId,
          callId: normalizedCallId,
        },
      });
      toolReviewDetailMap.value = {
        ...toolReviewDetailMap.value,
        [normalizedCallId]: detail,
      };
      await refreshToolReviewBatches();
      await refreshMessagesAfterReviewMutation(conversationId, [detail.messageId]);
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    } finally {
      if (toolReviewReviewingCallId.value === normalizedCallId) {
        toolReviewReviewingCallId.value = "";
      }
    }
  }

  async function runToolReviewForBatch(batchKey?: string) {
    const normalizedBatchKey = String(batchKey || toolReviewCurrentBatchKey.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedBatchKey || !conversationId) return;
    toolReviewBatchReviewingKey.value = normalizedBatchKey;
    try {
      const result = await invokeTauri<{ reviewedCallIds: string[] }>("run_tool_review_for_batch", {
        input: {
          conversationId,
          batchKey: normalizedBatchKey,
        },
      });
      const reviewedCallIds = Array.isArray(result?.reviewedCallIds) ? result.reviewedCallIds : [];
      const refreshedDetails = await Promise.all(reviewedCallIds.map((callId) => loadToolReviewItemDetail(callId, true)));
      await refreshToolReviewBatches();
      await refreshMessagesAfterReviewMutation(
        conversationId,
        refreshedDetails
          .map((detail) => String(detail?.messageId || "").trim())
          .filter(Boolean),
      );
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    } finally {
      if (toolReviewBatchReviewingKey.value === normalizedBatchKey) {
        toolReviewBatchReviewingKey.value = "";
      }
    }
  }

  async function submitToolReviewBatch(batchKey?: string) {
    const normalizedBatchKey = String(batchKey || toolReviewCurrentBatchKey.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedBatchKey || !conversationId) return;
    toolReviewSubmittingBatchKey.value = normalizedBatchKey;
    toolReviewReportErrorText.value = "";
    try {
      const result = await invokeTauri<SubmitToolReviewBatchOutput>("submit_tool_review_batch", {
        input: {
          conversationId,
          batchKey: normalizedBatchKey,
          apiConfigId: String(options.selectedChatModelId.value || "").trim() || undefined,
        },
      });
      await refreshToolReviewBatches();
      await refreshMessagesAfterReviewMutation(conversationId, [String(result?.reportMessageId || "").trim()]);
      toolReviewReportErrorText.value = "";
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewReportErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    } finally {
      if (toolReviewSubmittingBatchKey.value === normalizedBatchKey) {
        toolReviewSubmittingBatchKey.value = "";
      }
    }
  }

  function toggleToolReviewPanel() {
    if (!toolReviewButtonEnabled.value) {
      toolReviewPanelOpen.value = false;
      return;
    }
    toolReviewPanelOpen.value = !toolReviewPanelOpen.value;
  }

  function setToolReviewCurrentBatchKey(batchKey: string) {
    toolReviewCurrentBatchKey.value = String(batchKey || "").trim();
  }

  watch(
    () => options.refreshTick.value,
    () => {
      if (!String(options.activeConversationId.value || "").trim()) return;
      void refreshToolReviewBatches();
    },
  );

  watch(
    () => String(options.activeConversationId.value || "").trim(),
    (conversationId) => {
      toolReviewPanelOpen.value = false;
      toolReviewBatches.value = [];
      toolReviewDetailMap.value = {};
      toolReviewDetailLoadingCallId.value = "";
      toolReviewReviewingCallId.value = "";
      toolReviewBatchReviewingKey.value = "";
      toolReviewSubmittingBatchKey.value = "";
      toolReviewCurrentBatchKey.value = "";
      loadedConversationId.value = "";
      toolReviewErrorText.value = "";
      toolReviewReportErrorText.value = "";
      if (conversationId) {
        void refreshToolReviewBatches();
      }
    },
    { immediate: true },
  );

  return {
    toolReviewPanelOpen,
    toolReviewBatches,
    toolReviewCurrentBatchKey,
    toolReviewButtonLabel,
    toolReviewButtonEnabled,
    toolReviewDetailMap,
    toolReviewDetailLoadingCallId,
    toolReviewReviewingCallId,
    toolReviewBatchReviewingKey,
    toolReviewSubmittingBatchKey,
    toolReviewErrorText,
    toolReviewReportErrorText,
    toggleToolReviewPanel,
    refreshToolReviewBatches,
    setToolReviewCurrentBatchKey,
    loadToolReviewItemDetail,
    runToolReviewForCall,
    runToolReviewForBatch,
    submitToolReviewBatch,
  };
}
