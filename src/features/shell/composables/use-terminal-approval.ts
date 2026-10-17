import { computed, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

export type TerminalApprovalRequestPayload = {
  requestId: string;
  title?: string;
  message?: string;
  approvalKind?: string;
  sessionId?: string;
  toolName?: string;
  summary?: string;
  callPreview?: string;
  cwd?: string;
  command?: string;
  requestedPath?: string;
  reason?: string;
  existingPaths?: string[];
  targetPaths?: string[];
  reviewOpinion?: string;
  reviewModelName?: string;
  canRememberWorkspace?: boolean;
  workspaceName?: string;
  workspacePath?: string;
  description?: string;
};

export type TerminalApprovalConversationItem = TerminalApprovalRequestPayload & {
  conversationId: string;
};

type UseTerminalApprovalOptions = {
  queue: Ref<TerminalApprovalRequestPayload[]>;
  resolving: Ref<boolean>;
  defaultTitle?: string | (() => string);
  onError?: (input: {
    action: "resolve" | "approve_for_session" | "approve_for_workspace";
    request: TerminalApprovalRequestPayload;
    error: unknown;
  }) => void;
};

export function useTerminalApproval(options: UseTerminalApprovalOptions) {
  const terminalApprovalCurrent = computed(() => options.queue.value[0] ?? null);
  const terminalApprovalDialogOpen = computed(() => !!terminalApprovalCurrent.value);
  const terminalApprovalDialogTitle = computed(
    () => terminalApprovalCurrent.value?.title || "终端审批",
  );
  const terminalApprovalDialogBody = computed(
    () => terminalApprovalCurrent.value?.message || "",
  );

  function defaultTitle(): string {
    const value = typeof options.defaultTitle === "function"
      ? options.defaultTitle()
      : options.defaultTitle;
    return String(value || "终端审批").trim() || "终端审批";
  }

  function normalizeTerminalApprovalConversationId(payload: Pick<TerminalApprovalRequestPayload, "sessionId"> | null | undefined): string {
    const sessionId = String(payload?.sessionId || "").trim();
    if (!sessionId) return "";
    const parts = sessionId.split("::");
    if (parts.length >= 2) {
      return String(parts[parts.length - 1] || "").trim();
    }
    return sessionId;
  }

  function listConversationTerminalApprovals(conversationId: string): TerminalApprovalConversationItem[] {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) return [];
    return options.queue.value
      .filter((item) => normalizeTerminalApprovalConversationId(item) === normalizedConversationId)
      .map((item) => ({
        ...item,
        conversationId: normalizedConversationId,
      }));
  }

  function getConversationTerminalApprovalCurrent(conversationId: string): TerminalApprovalConversationItem | null {
    return listConversationTerminalApprovals(conversationId)[0] ?? null;
  }

  function hasConversationTerminalApprovals(conversationId: string): boolean {
    return !!getConversationTerminalApprovalCurrent(conversationId);
  }

  function enqueueTerminalApprovalRequest(payload: TerminalApprovalRequestPayload) {
    const requestId = String(payload.requestId || "").trim();
    if (!requestId) return;
    if (options.queue.value.some((item) => item.requestId === requestId)) return;
    options.queue.value.push({
      ...payload,
      requestId,
      title: String(payload.title || defaultTitle()),
      message: String(payload.message || ""),
      approvalKind: String(payload.approvalKind || "unknown"),
      sessionId: String(payload.sessionId || ""),
      toolName: String(payload.toolName || ""),
      summary: String(payload.summary || ""),
      callPreview: String(payload.callPreview || ""),
      cwd: String(payload.cwd || ""),
      command: String(payload.command || ""),
      requestedPath: String(payload.requestedPath || ""),
      reason: String(payload.reason || ""),
      reviewOpinion: String(payload.reviewOpinion || ""),
      reviewModelName: String(payload.reviewModelName || ""),
      canRememberWorkspace: !!payload.canRememberWorkspace,
      workspaceName: String(payload.workspaceName || ""),
      workspacePath: String(payload.workspacePath || ""),
      existingPaths: Array.isArray(payload.existingPaths)
        ? payload.existingPaths.map((item) => String(item || "").trim()).filter(Boolean)
        : [],
      targetPaths: Array.isArray(payload.targetPaths)
        ? payload.targetPaths.map((item) => String(item || "").trim()).filter(Boolean)
        : [],
    });
  }

  function removeTerminalApprovalRequest(requestId: string) {
    const normalized = String(requestId || "").trim();
    if (!normalized) return;
    const index = options.queue.value.findIndex((item) => item.requestId === normalized);
    if (index < 0) return;
    options.queue.value.splice(index, 1);
  }

  async function resolveTerminalApproval(approved: boolean, requestId?: string, reason?: string) {
    if (options.resolving.value) return;
    const normalizedRequestId = String(requestId || "").trim();
    const targetIndex = normalizedRequestId
      ? options.queue.value.findIndex((item) => item.requestId === normalizedRequestId)
      : 0;
    if (targetIndex < 0) return;
    const current = options.queue.value[targetIndex] ?? null;
    if (!current) return;
    const normalizedReason = String(reason ?? "").trim().slice(0, 500) || undefined;
    options.resolving.value = true;
    try {
      await invokeTauri("terminalApproval.resolve", {
        requestId: current.requestId,
        approved,
        ...(normalizedReason ? { reason: normalizedReason } : {}),
      });
      // 后端在命令返回之前就会广播 resolved，本窗口可能已收到并摘掉这一项，
      // 队列此时已经左移；按 id 重新定位，不能用旧索引删掉顺位补上来的另一条审批。
      removeTerminalApprovalRequest(current.requestId);
    } catch (error) {
      options.onError?.({ action: "resolve", request: current, error });
      if (!options.onError) console.warn("[终端审批] 失败，操作=处理审批", error);
    } finally {
      options.resolving.value = false;
    }
  }

  function denyTerminalApproval(requestId?: string, reason?: string) {
    void resolveTerminalApproval(false, requestId, reason);
  }

  function approveTerminalApproval(requestId?: string, reason?: string) {
    void resolveTerminalApproval(true, requestId, reason);
  }

  async function invokeTerminalApprovalAction(
    command: "terminalApproval.approveForSession" | "terminalApproval.approveForWorkspace",
    action: "approve_for_session" | "approve_for_workspace",
    requestId?: string,
  ) {
    if (options.resolving.value) return;
    const normalizedRequestId = String(requestId || "").trim();
    const targetIndex = normalizedRequestId
      ? options.queue.value.findIndex((item) => item.requestId === normalizedRequestId)
      : 0;
    if (targetIndex < 0) return;
    const current = options.queue.value[targetIndex] ?? null;
    if (!current) return;
    options.resolving.value = true;
    try {
      await invokeTauri(command, {
        requestId: current.requestId,
      });
      removeTerminalApprovalRequest(current.requestId);
    } catch (error) {
      options.onError?.({ action, request: current, error });
      if (!options.onError) console.warn(`[终端审批] 失败，操作=${action}`, error);
    } finally {
      options.resolving.value = false;
    }
  }

  function approveTerminalApprovalForSession(requestId?: string) {
    void invokeTerminalApprovalAction("terminalApproval.approveForSession", "approve_for_session", requestId);
  }

  function approveTerminalApprovalForWorkspace(requestId?: string) {
    void invokeTerminalApprovalAction("terminalApproval.approveForWorkspace", "approve_for_workspace", requestId);
  }

  async function rehydrateTerminalApprovals(): Promise<number> {
    try {
      // 只对「请求发出时本地已挂着」的审批做撤下判定：请求往返期间新到达的审批
      // 不在快照里，但它们仍然有效，不能拿旧快照把它们一起清掉。
      const knownRequestIds = options.queue.value.map((item) => item.requestId);
      const items = await invokeTauri<TerminalApprovalRequestPayload[]>("terminalApproval.list");
      if (!Array.isArray(items)) return 0;
      // 后端 pending 表是唯一权威：本地还挂着、后端已没有的，说明这条审批
      // 已在别的窗口处理或已失效，本地必须同步撤下，否则卡片会一直留在界面上。
      const pendingIds = new Set(
        items.map((item) => String(item?.requestId || "").trim()).filter(Boolean),
      );
      for (const requestId of knownRequestIds) {
        if (!pendingIds.has(requestId)) removeTerminalApprovalRequest(requestId);
      }
      let added = 0;
      for (const item of items) {
        const before = options.queue.value.length;
        enqueueTerminalApprovalRequest(item as TerminalApprovalRequestPayload);
        if (options.queue.value.length !== before) added += 1;
      }
      return added;
    } catch {
      return 0;
    }
  }

  return {
    terminalApprovalCurrent,
    terminalApprovalDialogOpen,
    terminalApprovalDialogTitle,
    terminalApprovalDialogBody,
    listConversationTerminalApprovals,
    getConversationTerminalApprovalCurrent,
    hasConversationTerminalApprovals,
    enqueueTerminalApprovalRequest,
    removeTerminalApprovalRequest,
    denyTerminalApproval,
    approveTerminalApproval,
    approveTerminalApprovalForSession,
    approveTerminalApprovalForWorkspace,
    rehydrateTerminalApprovals,
  };
}
