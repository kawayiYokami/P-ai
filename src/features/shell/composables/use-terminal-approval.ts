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
};

type UseTerminalApprovalOptions = {
  queue: Ref<TerminalApprovalRequestPayload[]>;
  resolving: Ref<boolean>;
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

  function enqueueTerminalApprovalRequest(payload: TerminalApprovalRequestPayload) {
    const requestId = String(payload.requestId || "").trim();
    if (!requestId) return;
    options.queue.value.push({
      ...payload,
      requestId,
      title: String(payload.title || "终端审批"),
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
      existingPaths: Array.isArray(payload.existingPaths)
        ? payload.existingPaths.map((item) => String(item || "").trim()).filter(Boolean)
        : [],
      targetPaths: Array.isArray(payload.targetPaths)
        ? payload.targetPaths.map((item) => String(item || "").trim()).filter(Boolean)
        : [],
    });
  }

  async function resolveTerminalApproval(approved: boolean) {
    if (options.resolving.value) return;
    const current = terminalApprovalCurrent.value;
    if (!current) return;
    options.resolving.value = true;
    try {
      await invokeTauri("resolve_terminal_approval", {
        input: {
          requestId: current.requestId,
          approved,
        },
      });
    } catch (error) {
      console.warn("[TERMINAL] resolve_terminal_approval failed:", error);
    } finally {
      options.queue.value.shift();
      options.resolving.value = false;
    }
  }

  function denyTerminalApproval() {
    void resolveTerminalApproval(false);
  }

  function approveTerminalApproval() {
    void resolveTerminalApproval(true);
  }

  return {
    terminalApprovalCurrent,
    terminalApprovalDialogOpen,
    terminalApprovalDialogTitle,
    terminalApprovalDialogBody,
    enqueueTerminalApprovalRequest,
    denyTerminalApproval,
    approveTerminalApproval,
  };
}
