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

type UseChatToolReviewOptions = {
  activeConversationId: Ref<string>;
  selectedChatModelId: Ref<string>;
  messageBlocks: ComputedRef<ChatMessageBlock[]>;
  t: (key: string, params?: Record<string, unknown>) => string;
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
  const toolReviewButtonLabel = computed(() =>
    options.t("chat.toolReview.button", { count: toolReviewButtonCount.value })
  );
  const toolReviewButtonEnabled = computed(() => !!String(options.activeConversationId.value || "").trim());

  async function refreshToolReviewBatches() {
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!conversationId) {
      toolReviewBatches.value = [];
      toolReviewCurrentBatchKey.value = "";
      toolReviewDetailMap.value = {};
      return;
    }
    try {
      const result = await invokeTauri<ToolReviewBatchListOutput>("list_tool_review_batches", {
        input: {
          conversationId,
        },
      });
      toolReviewBatches.value = Array.isArray(result?.batches) ? result.batches : [];
      const currentKey = String(toolReviewCurrentBatchKey.value || "").trim();
      if (currentKey && toolReviewBatches.value.some((batch) => batch.batchKey === currentKey)) {
        toolReviewCurrentBatchKey.value = currentKey;
      } else {
        toolReviewCurrentBatchKey.value = String(result?.currentBatchKey || toolReviewBatches.value[toolReviewBatches.value.length - 1]?.batchKey || "").trim();
      }
      const validCallIds = new Set(toolReviewBatches.value.flatMap((batch) => batch.items.map((item) => item.callId)));
      toolReviewDetailMap.value = Object.fromEntries(
        Object.entries(toolReviewDetailMap.value).filter(([callId]) => validCallIds.has(callId))
      );
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    }
  }

  async function loadToolReviewItemDetail(callId: string) {
    const normalizedCallId = String(callId || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedCallId || !conversationId || toolReviewDetailMap.value[normalizedCallId]) return;
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
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
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
      for (const callId of reviewedCallIds) {
        await loadToolReviewItemDetail(callId);
      }
      await refreshToolReviewBatches();
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
      await invokeTauri("submit_tool_review_batch", {
        input: {
          conversationId,
          batchKey: normalizedBatchKey,
          apiConfigId: String(options.selectedChatModelId.value || "").trim() || undefined,
        },
      });
      await refreshToolReviewBatches();
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
    toolReviewPanelOpen.value = !toolReviewPanelOpen.value;
  }

  function setToolReviewCurrentBatchKey(batchKey: string) {
    toolReviewCurrentBatchKey.value = String(batchKey || "").trim();
  }

  watch(
    () => [
      String(options.activeConversationId.value || "").trim(),
      options.messageBlocks.value.map((block) => `${block.id}:${block.toolCallCount}:${block.text.length}`).join("|"),
    ],
    () => {
      void refreshToolReviewBatches();
    },
    { immediate: true },
  );

  watch(
    () => String(options.activeConversationId.value || "").trim(),
    () => {
      toolReviewPanelOpen.value = false;
      toolReviewDetailMap.value = {};
      toolReviewCurrentBatchKey.value = "";
      toolReviewErrorText.value = "";
      toolReviewReportErrorText.value = "";
    },
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
    setToolReviewCurrentBatchKey,
    loadToolReviewItemDetail,
    runToolReviewForCall,
    runToolReviewForBatch,
    submitToolReviewBatch,
  };
}
