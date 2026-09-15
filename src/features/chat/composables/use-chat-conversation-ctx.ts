import { computed, type Ref } from "vue";
import type { ChatConversationOverviewItem, ChatMentionTarget, ChatTodoItem } from "../../../types/app";
import type { TerminalApprovalConversationItem } from "../../shell/composables/use-terminal-approval";

export type ChatStatusBannerTone = "default" | "error" | "success" | "info";
export type ChatStatusBanner = { text: string; tone: ChatStatusBannerTone };

export function useChatConversationCtx(
  props: {
    currentTheme: string;
    activeConversationId: string;
    conversationItems?: ChatConversationOverviewItem[];
    unarchivedConversationItems: ChatConversationOverviewItem[];
    currentTodos: ChatTodoItem[];
    compactingConversation: boolean;
    compactingConversationId?: string;
    trimming: boolean;
    trimmingConversationId?: string;
    terminalApprovals?: TerminalApprovalConversationItem[];
    terminalApprovalResolving?: boolean;
    goalActive: boolean;
    goalTitle: string;
    chatErrorText: string;
    selectedMentions: ChatMentionTarget[];
    messageBlocks: Array<{
      isExtraTextBlock?: boolean;
      isStreaming?: boolean;
      planCard?: { action?: string };
      sourceMessageId?: string;
      id?: string;
      providerMeta?: Record<string, unknown>;
      toolCalls?: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
      activityItems?: Array<{ kind: string; name?: string; status?: "doing" | "done" }>;
    }>;
  },
  isDarkAppTheme: (theme: string) => boolean,
  t: (key: string, params?: Record<string, unknown>) => string,
) {
  const markdownIsDark = computed(() => isDarkAppTheme(props.currentTheme));

  function isOrganizeContextToolCall(call: { name: string; status?: string }): boolean {
    const name = String(call.name || "").trim().toLowerCase();
    return name === "archive";
  }

  function isOrganizeContextStatusText(text: string): boolean {
    const value = String(text || "").trim();
    return !!value && (
      value.includes(t("chat.statusCompactingContext"))
      || value.includes("压缩上下文")
      || value.includes("壓縮上下文")
      || value.includes("整理上下文")
      || value.includes("整理後繼續")
      || value.includes("整理后继续")
      || value.toLowerCase().includes("compacting context")
    );
  }

  const VALID_TODO_STATUSES: ReadonlySet<string> = new Set(["pending", "in_progress", "completed"]);

  function isValidTodoStatus(value: string): value is "pending" | "in_progress" | "completed" {
    return VALID_TODO_STATUSES.has(value);
  }

  const normalizedConversationTodos = computed<Array<{ content: string; status: "pending" | "in_progress" | "completed" }>>(() => {
    const todos = Array.isArray(props.currentTodos) ? props.currentTodos : [];
    return todos
      .map((item) => {
        const status = String(item?.status || "").trim();
        return {
          content: String(item?.content || "").trim(),
          status: isValidTodoStatus(status) ? status : ("pending" as const),
        };
      })
      .filter((item) => item.content);
  });

  const activeConversationSummary = computed(() => {
    const id = String(props.activeConversationId || "").trim();
    if (!id) return null;
    return (props.conversationItems || props.unarchivedConversationItems).find(
      (item) => String(item.conversationId || "").trim() === id,
    ) || null;
  });

  const isCurrentConversationCompacting = computed(() =>
    !!String(props.activeConversationId || "").trim() &&
    String(props.activeConversationId || "").trim() === String(props.compactingConversationId || "").trim(),
  );

  const activeConversationTerminalApprovals = computed(() => {
    const conversationId = String(props.activeConversationId || "").trim();
    const allApprovals = Array.isArray(props.terminalApprovals) ? props.terminalApprovals : [];
    if (!conversationId) return allApprovals;
    const conversationApprovals = allApprovals
      .filter((item) => String(item.conversationId || item.sessionId || "").trim() === conversationId);
    // 审批卡绝不能因为会话映射暂时没挂上就直接消失；命中当前会话就显示当前会话，否则回退显示全局待审批队列。
    return conversationApprovals.length > 0 ? conversationApprovals : allApprovals;
  });

  const goalButtonTitle = computed(() => {
    const base = props.goalActive ? t("chat.goal.activeButtonTitle") : t("chat.goal.buttonTitle");
    const detail = String(props.goalTitle || "").trim();
    return detail ? `${base}\n${detail}` : base;
  });

  // 工具状态不再由全局 refs 传入，从流式消息块 providerMeta 派生
  // （状态机写入的 _toolStatusText/_toolStatusState）。
  const streamingToolStatus = computed(() => {
    for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
      const block = props.messageBlocks[idx];
      if (!block.isStreaming) continue;
      const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
      const state = String(providerMeta._toolStatusState || "").trim();
      const text = String(providerMeta._toolStatusText || "").trim();
      if (state === "running" || state === "done" || state === "failed") {
        return { state, text };
      }
    }
    return { state: "", text: "" };
  });

  const activeRunningToolCall = computed(() => {
    if (streamingToolStatus.value.state !== "running") return null;
    const calls = props.messageBlocks.flatMap((block) => [
      ...(Array.isArray(block.toolCalls) ? block.toolCalls : []),
      ...(Array.isArray(block.activityItems)
        ? block.activityItems
          .filter((item) => item.kind === "tool")
          .map((item) => ({ name: String(item.name || ""), argsText: "", status: item.status }))
        : []),
    ]);
    for (let idx = calls.length - 1; idx >= 0; idx -= 1) {
      const call = calls[idx];
      if (String(call?.status || "").trim() === "done") continue;
      if (!String(call?.name || "").trim()) continue;
      return call;
    }
    return null;
  });

  const isOrganizingContextBusy = computed(() => {
    if (props.compactingConversation && isCurrentConversationCompacting.value) return true;
    const runtimeState = String(activeConversationSummary.value?.runtimeState || "").trim();
    if (runtimeState === "organizing_context" || runtimeState === "compacting") return true;
    const runningTool = activeRunningToolCall.value;
    if (runningTool && isOrganizeContextToolCall(runningTool)) return true;
    if (streamingToolStatus.value.state !== "running") return false;
    return isOrganizeContextStatusText(streamingToolStatus.value.text);
  });

  const chatStatusBanner = computed<ChatStatusBanner | null>(() => {
    const errorText = String(props.chatErrorText || "").trim();
    if (errorText) return { text: errorText, tone: "error" };
    if (props.trimming) {
      const currentId = String(props.activeConversationId || "").trim();
      const archivingId = String(props.trimmingConversationId || "").trim();
      if (!currentId || currentId !== archivingId) return null;
      return { text: t("chat.statusArchivingConversation"), tone: "default" };
    }
    if (isOrganizingContextBusy.value) {
      return { text: t("chat.statusCompactingContext"), tone: "info" };
    }
    return null;
  });

  const selectedMentionKeys = computed(() =>
    (Array.isArray(props.selectedMentions) ? props.selectedMentions : [])
      .map((item) => String(item?.agentId || "").trim())
      .filter((value, index, list) => !!value && list.indexOf(value) === index),
  );

  const latestPendingPlanMessageId = computed(() => {
    for (let idx = props.messageBlocks.length - 1; idx >= 0; idx -= 1) {
      const block = props.messageBlocks[idx];
      if (block.isExtraTextBlock) continue;
      const providerMeta = (block.providerMeta || {}) as Record<string, unknown>;
      const messageMeta = ((providerMeta.message_meta || providerMeta.messageMeta || {}) as Record<string, unknown>);
      const messageKind = String(messageMeta.kind || providerMeta.messageKind || "").trim();
      if (messageKind === "plan_complete" || block.planCard?.action === "complete") return "";
      if (messageKind === "plan_present" || block.planCard?.action === "present") {
        return String(block.sourceMessageId || block.id || "").trim();
      }
    }
    return "";
  });

  return {
    markdownIsDark,
    normalizedConversationTodos,
    activeConversationSummary,
    isCurrentConversationCompacting,
    activeConversationTerminalApprovals,
    goalButtonTitle,
    isOrganizingContextBusy,
    chatStatusBanner,
    selectedMentionKeys,
    latestPendingPlanMessageId,
  };
}
