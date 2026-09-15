import { ref, watch, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

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
  reviewOpinion?: string;
  affectedPaths?: string[];
  patchOperation?: "add" | "update" | "delete" | "mixed" | string;
  command?: string;
  finishedAt?: string;
  addedLines?: number;
  deletedLines?: number;
  isSuccess?: boolean;
  isDenied?: boolean;
  blockedReason?: string;
};

export type ToolReviewReportRecord = {
  id: string;
  conversationId: string;
  title: string;
  status: "pending" | "failed" | "success" | string;
  scope: string;
  target: string;
  agentId?: string;
  delegateId?: string;
  workspacePath: string;
  createdAt: string;
  updatedAt: string;
  reportText: string;
  errorText?: string;
};

export type ToolReviewCodeReviewScope = "uncommitted" | "main" | "commit" | "custom";

export type ToolReviewCommitOption = {
  hash: string;
  shortHash: string;
  subject: string;
  authorTime: string;
};

export type ToolReviewCommitPage = {
  total: number;
  page: number;
  pageSize: number;
  commits: ToolReviewCommitOption[];
};

export type ToolReviewBatchSummary = {
  batchKey: string;
  userMessageId: string;
  userMessageText: string;
  itemCount: number;
  unreviewedCount: number;
  changedFiles?: number;
  addedLines?: number;
  deletedLines?: number;
  items: ToolReviewItemSummary[];
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

export type ToolReviewSegment = {
  path: string;
  action: "add" | "update" | "delete" | "move" | string;
  diffLines: string[];
};

type ToolReviewBatchDetailsOutput = {
  batchKey: string;
  segments: ToolReviewSegment[];
};

type ToolReviewBatchListOutput = {
  batches: ToolReviewBatchSummary[];
  currentBatchKey?: string;
};

type SubmitToolReviewTaskOutput = {
  report: ToolReviewReportRecord;
};

type ListToolReviewReportsOutput = {
  reports: ToolReviewReportRecord[];
};

type SubmitToolReviewCodeInput = {
  conversationId: string;
  scope: ToolReviewCodeReviewScope;
  target?: string;
  agentId?: string;
};

type DeleteToolReviewReportInput = {
  conversationId: string;
  reportId: string;
};

type ToolReviewItemDecisionInput = {
  conversationId: string;
  callId: string;
  allow: boolean;
  opinion?: string;
};

type ListToolReviewCommitOptionsOutput = {
  total: number;
  page: number;
  pageSize: number;
  commits: ToolReviewCommitOption[];
};

type UseChatToolReviewOptions = {
  activeConversationId: Ref<string>;
  refreshTick: Ref<number>;
  initialPanelOpen?: Ref<boolean>;
  activeTab?: Ref<string>;
  /** 右侧主页可见时为真：主页的「最近工具」卡片复用同一份批次列表，需要在这里补一次加载 */
  homePreviewActive?: Ref<boolean>;
  t: (key: string, params?: Record<string, unknown>) => string;
  onRefreshMessage?: (input: { conversationId: string; messageId: string }) => void | Promise<void>;
};

export function useChatToolReview(options: UseChatToolReviewOptions) {
  const toolReviewPanelOpen = ref(!!options.initialPanelOpen?.value);
  const toolReviewBatches = ref<ToolReviewBatchSummary[]>([]);
  const toolReviewCurrentBatchKey = ref("");
  const toolReviewDetailMap = ref<Record<string, ToolReviewItemDetail>>({});
  const toolReviewSegmentMap = ref<Record<string, ToolReviewSegment[]>>({});
  const toolReviewDetailLoadingCallId = ref("");
  const toolReviewReviewingCallId = ref("");
  const toolReviewDecisionCallId = ref("");
  const toolReviewBatchReviewingKey = ref("");
  const toolReviewSubmittingBatchKey = ref("");
  const toolReviewErrorText = ref("");
  const toolReviewReportErrorText = ref("");
  const toolReviewReports = ref<ToolReviewReportRecord[]>([]);
  const toolReviewCurrentReportId = ref("");

  function formatToolReviewError(error: unknown): string {
    const message = error instanceof Error ? String(error.message || "").trim() : String(error);
    const stack = error instanceof Error ? String(error.stack || "").trim() : "";
    if (stack && stack !== message) {
      return message ? `${message}\n${stack}` : stack;
    }
    return message || "Unknown error";
  }

  async function requestToolReview<T>(command: string, args?: { input?: Record<string, unknown> }): Promise<T> {
    return await invokeTauri<T>(command, args?.input || {});
  }

  async function refreshToolReviewReports() {
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!conversationId) {
      toolReviewReports.value = [];
      toolReviewCurrentReportId.value = "";
      return;
    }
    try {
      const result = await requestToolReview<ListToolReviewReportsOutput>("list_tool_review_reports", {
        input: { conversationId },
      });
      toolReviewReports.value = Array.isArray(result?.reports) ? result.reports : [];
      const currentId = String(toolReviewCurrentReportId.value || "").trim();
      if (currentId && !toolReviewReports.value.some((item) => item.id === currentId)) {
        toolReviewCurrentReportId.value = "";
      }
      toolReviewReportErrorText.value = "";
    } catch (error) {
      console.error("[工具审查][前端] 刷新审查报告失败", {
        conversationId,
        error,
      });
      toolReviewReports.value = [];
      toolReviewCurrentReportId.value = "";
      toolReviewReportErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    }
  }

  async function deleteToolReviewReport(input: DeleteToolReviewReportInput) {
    const conversationId = String(input.conversationId || options.activeConversationId.value || "").trim();
    const reportId = String(input.reportId || "").trim();
    if (!conversationId || !reportId) return;
    toolReviewReportErrorText.value = "";
    try {
      await requestToolReview("delete_tool_review_report", {
        input: {
          conversationId,
          reportId,
        },
      });
      if (toolReviewCurrentReportId.value === reportId) {
        toolReviewCurrentReportId.value = "";
      }
      toolReviewReportErrorText.value = "";
    } catch (error) {
      toolReviewReportErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
      throw error;
    }
  }

  async function listToolReviewCommitOptions(conversationId?: string, page = 1, pageSize = 30) {
    const normalizedConversationId = String(conversationId || options.activeConversationId.value || "").trim();
    if (!normalizedConversationId) {
      return { total: 0, page, pageSize, commits: [] } as ToolReviewCommitPage;
    }
    const result = await requestToolReview<ListToolReviewCommitOptionsOutput>("list_tool_review_commit_options", {
      input: {
        conversationId: normalizedConversationId,
        page,
        pageSize,
      },
    });
    return {
      total: Number(result?.total || 0),
      page: Number(result?.page || page),
      pageSize: Number(result?.pageSize || pageSize),
      commits: Array.isArray(result?.commits) ? result.commits : [],
    } as ToolReviewCommitPage;
  }

  async function submitToolReviewCode(input: SubmitToolReviewCodeInput): Promise<ToolReviewReportRecord | null> {
    const conversationId = String(input.conversationId || options.activeConversationId.value || "").trim();
    const scope = String(input.scope || "").trim() as ToolReviewCodeReviewScope;
    if (!conversationId || !scope) return null;
    toolReviewSubmittingBatchKey.value = `scope:${scope}`;
    toolReviewReportErrorText.value = "";
    try {
      console.info("[工具审查][前端] 调用 submit_tool_review_code", {
        conversationId,
        scope,
        target: String(input.target || "").trim(),
        agentId: String(input.agentId || "").trim(),
      });
      const result = await requestToolReview<SubmitToolReviewTaskOutput>("submit_tool_review_code", {
        input: {
          conversationId,
          scope,
          target: String(input.target || "").trim() || undefined,
          agentId: String(input.agentId || "").trim() || undefined,
        },
      });
      toolReviewCurrentReportId.value = String(result?.report?.id || "").trim();
      toolReviewReportErrorText.value = "";
      toolReviewErrorText.value = "";
      return result?.report || null;
    } catch (error) {
      toolReviewReportErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
      return null;
    } finally {
      if (toolReviewSubmittingBatchKey.value === `scope:${scope}`) {
        toolReviewSubmittingBatchKey.value = "";
      }
    }
  }

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
      toolReviewSegmentMap.value = {};
      return;
    }
    try {
      const result = await requestToolReview<ToolReviewBatchListOutput>("list_tool_review_batches", {
        input: {
          conversationId,
        },
      });
      toolReviewBatches.value = Array.isArray(result?.batches) ? result.batches : [];
      const currentKey = String(toolReviewCurrentBatchKey.value || "").trim();
      toolReviewCurrentBatchKey.value = currentKey
        ? resolveValidBatchKey(toolReviewBatches.value, currentKey)
        : resolveValidBatchKey(toolReviewBatches.value, result?.currentBatchKey);
      const validCallIds = new Set(toolReviewBatches.value.flatMap((batch) => batch.items.map((item) => item.callId)));
      toolReviewDetailMap.value = Object.fromEntries(
        Object.entries(toolReviewDetailMap.value).filter(([callId]) => validCallIds.has(callId))
      );
      const validBatchKeys = new Set(toolReviewBatches.value.map((batch) => batch.batchKey));
      toolReviewSegmentMap.value = Object.fromEntries(
        Object.entries(toolReviewSegmentMap.value).filter(([batchKey]) => validBatchKeys.has(batchKey))
      );
      toolReviewErrorText.value = "";
      await refreshToolReviewReports();
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
      const detail = await requestToolReview<ToolReviewItemDetail>("get_tool_review_item_detail", {
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

  async function loadToolReviewBatchSegments(batchKey: string, force = false) {
    const normalizedBatchKey = String(batchKey || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedBatchKey || !conversationId) return;
    const cachedSegments = toolReviewSegmentMap.value[normalizedBatchKey];
    if (!force && cachedSegments) return;
    const batchIndex = toolReviewBatches.value.findIndex(
      (batch) => String(batch.batchKey || "").trim() === normalizedBatchKey,
    );
    if (batchIndex < 0) return;
    try {
      const result = await requestToolReview<ToolReviewBatchDetailsOutput>("get_tool_review_batch_details", {
        input: {
          conversationId,
          batchIndex,
        },
      });
      toolReviewSegmentMap.value = {
        ...toolReviewSegmentMap.value,
        [normalizedBatchKey]: Array.isArray(result?.segments) ? result.segments : [],
      };
      toolReviewErrorText.value = "";
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
    }
  }

  async function runToolReviewForCall(callId: string) {
    const normalizedCallId = String(callId || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!normalizedCallId || !conversationId) return;
    toolReviewReviewingCallId.value = normalizedCallId;
    try {
      const detail = await requestToolReview<ToolReviewItemDetail>("run_tool_review_for_call", {
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
    const batchIndex = toolReviewBatches.value.findIndex((batch) => String(batch.batchKey || "").trim() === normalizedBatchKey);
    if (batchIndex < 0) return;
    toolReviewBatchReviewingKey.value = normalizedBatchKey;
    try {
      const result = await requestToolReview<{ reviewedCallIds: string[] }>("run_tool_review_for_batch", {
        input: {
          conversationId,
          batchIndex,
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

  async function setToolReviewItemUserDecision(input: ToolReviewItemDecisionInput): Promise<ToolReviewItemDetail | null> {
    const conversationId = String(input.conversationId || options.activeConversationId.value || "").trim();
    const callId = String(input.callId || "").trim();
    if (!conversationId || !callId) return null;
    toolReviewDecisionCallId.value = callId;
    try {
      const detail = await requestToolReview<ToolReviewItemDetail>("set_tool_review_item_user_decision", {
        input: {
          conversationId,
          callId,
          allow: !!input.allow,
          opinion: String(input.opinion || "").trim(),
        },
      });
      toolReviewDetailMap.value = { ...toolReviewDetailMap.value, [callId]: detail };
      toolReviewErrorText.value = "";
      await refreshToolReviewBatches();
      await refreshMessagesAfterReviewMutation(
        conversationId,
        [String(detail?.messageId || "").trim()].filter(Boolean),
      );
      return detail;
    } catch (error) {
      toolReviewErrorText.value = options.t("chat.toolReview.loadFailed", { err: formatToolReviewError(error) });
      return null;
    } finally {
      if (toolReviewDecisionCallId.value === callId) {
        toolReviewDecisionCallId.value = "";
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
    () => toolReviewCurrentBatchKey.value,
    (batchKey) => {
      if (!String(batchKey || "").trim()) return;
      void loadToolReviewBatchSegments(batchKey);
    },
  );

  watch(
    () => options.refreshTick.value,
    () => {
      if (!String(options.activeConversationId.value || "").trim()) return;
      if (!shouldLoadToolReviewBatches()) return;
      void refreshToolReviewBatches();
    },
  );

  if (options.initialPanelOpen) {
    watch(
      () => options.initialPanelOpen?.value,
      (open) => {
        toolReviewPanelOpen.value = !!open;
      },
    );
  }

  watch(
    () => String(options.activeConversationId.value || "").trim(),
    (conversationId) => {
      toolReviewBatches.value = [];
      toolReviewDetailMap.value = {};
      toolReviewSegmentMap.value = {};
      toolReviewDetailLoadingCallId.value = "";
      toolReviewReviewingCallId.value = "";
      toolReviewBatchReviewingKey.value = "";
      toolReviewSubmittingBatchKey.value = "";
      toolReviewCurrentBatchKey.value = "";
      toolReviewReports.value = [];
      toolReviewCurrentReportId.value = "";
      toolReviewErrorText.value = "";
      toolReviewReportErrorText.value = "";
      if (conversationId && shouldLoadToolReviewBatches()) {
        void refreshToolReviewBatches();
      }
    },
    { immediate: true },
  );

  // 工具评审批列表在监控面板打开且 tools 标签激活时加载（懒加载），避免切换会话时白读；
  // 右侧主页也展示批次摘要，因此主页可见时一并放行，否则首页那张卡永远没有数据。
  function shouldLoadToolReviewBatches(): boolean {
    if (options.homePreviewActive?.value) return true;
    if (!toolReviewPanelOpen.value) return false;
    if (options.activeTab && options.activeTab.value !== "tools") return false;
    return true;
  }

  if (options.activeTab) {
    watch(
      () => [toolReviewPanelOpen.value, String(options.activeTab?.value || "").trim()] as const,
      () => {
        const conversationId = String(options.activeConversationId.value || "").trim();
        if (conversationId && shouldLoadToolReviewBatches()) {
          void refreshToolReviewBatches();
        }
      },
    );
  }

  // 主页每次显示时刷新一次批次列表：批次随对话推进变化，缓存旧值会显示过期摘要
  if (options.homePreviewActive) {
    watch(
      () => Boolean(options.homePreviewActive?.value),
      (visible) => {
        if (!visible) return;
        const conversationId = String(options.activeConversationId.value || "").trim();
        if (!conversationId) return;
        void refreshToolReviewBatches();
      },
    );
  }

  return {
    toolReviewPanelOpen,
    toolReviewBatches,
    toolReviewCurrentBatchKey,
    toolReviewDetailMap,
    toolReviewSegmentMap,
    toolReviewDetailLoadingCallId,
    toolReviewReviewingCallId,
    toolReviewBatchReviewingKey,
    toolReviewSubmittingBatchKey,
    toolReviewDecisionCallId,
    toolReviewErrorText,
    toolReviewReportErrorText,
    toolReviewReports,
    toolReviewCurrentReportId,
    toggleToolReviewPanel,
    refreshToolReviewBatches,
    refreshToolReviewReports,
    setToolReviewCurrentBatchKey,
    loadToolReviewItemDetail,
    loadToolReviewBatchSegments,
    runToolReviewForCall,
    runToolReviewForBatch,
    submitToolReviewCode,
    deleteToolReviewReport,
    listToolReviewCommitOptions,
    setToolReviewItemUserDecision,
  };
}
