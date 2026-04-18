import { ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatMessage, RuntimeLogEntry, UnarchivedConversationSummary } from "../../../types/app";
import type { ConfigSaveErrorInfo } from "../../config/composables/use-config-persistence";
import { inspectUndoablePatchCalls } from "../../../utils/chat-message-semantics";

export type ForceArchivePreviewResult = {
  conversationId: string;
  canArchive: boolean;
  canDropConversation: boolean;
  messageCount: number;
  hasAssistantReply: boolean;
  isEmpty: boolean;
  archiveDisabledReason?: string | null;
};

export type ForceCompactionPreviewResult = {
  conversationId: string;
  canCompact: boolean;
  messageCount: number;
  hasAssistantReply: boolean;
  isEmpty: boolean;
  contextUsagePercent: number;
  compactionDisabledReason?: string | null;
};

type RecallMode = "with_patch" | "message_only" | "cancel";

type UseShellDialogFlowsOptions = {
  t: (key: string, params?: Record<string, unknown>) => string;
  configTab: Ref<string>;
  allMessages: Ref<ChatMessage[]>;
  tauriWindowLabel: Ref<string>;
  currentForegroundApiConfigId: Ref<string>;
  currentForegroundAgentId: Ref<string>;
  currentForegroundDepartmentId: Ref<string>;
  currentChatConversationId: Ref<string>;
  unarchivedConversations: Ref<UnarchivedConversationSummary[]>;
  setStatus: (message: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  forceCompactNow: () => Promise<void>;
  forceArchiveNow: (targetConversationId?: string) => Promise<void>;
  deleteUnarchivedConversationFromArchives: (conversationId: string) => Promise<void>;
};

export function useShellDialogFlows(options: UseShellDialogFlowsOptions) {
  const runtimeLogsDialogOpen = ref(false);
  const runtimeLogs = ref<RuntimeLogEntry[]>([]);
  const runtimeLogsLoading = ref(false);
  const runtimeLogsError = ref("");
  const configSaveErrorDialogOpen = ref(false);
  const configSaveErrorDialogTitle = ref("");
  const configSaveErrorDialogBody = ref("");
  const configSaveErrorDialogKind = ref<"warning" | "error">("error");
  const skillPlaceholderDialogOpen = ref(false);
  const forceArchiveActionDialogOpen = ref(false);
  const forceArchivePreviewLoading = ref(false);
  const forceArchivePreview = ref<ForceArchivePreviewResult | null>(null);
  const forceCompactionPreview = ref<ForceCompactionPreviewResult | null>(null);
  const rewindConfirmDialogOpen = ref(false);
  const rewindConfirmCanUndoPatch = ref(false);
  const rewindConfirmUndoHint = ref("");
  let rewindConfirmResolver: ((mode: RecallMode) => void) | null = null;

  function closeForceArchiveActionDialog() {
    forceArchiveActionDialogOpen.value = false;
    forceArchivePreviewLoading.value = false;
    forceArchivePreview.value = null;
    forceCompactionPreview.value = null;
  }

  async function openForceArchiveActionDialog() {
    const apiConfigId = String(options.currentForegroundApiConfigId.value || "").trim();
    const agentId = String(options.currentForegroundAgentId.value || "").trim();
    const departmentId = String(options.currentForegroundDepartmentId.value || "").trim();
    const conversationId = String(options.currentChatConversationId.value || "").trim();
    if (!conversationId || !apiConfigId || !agentId) {
      options.setStatus("当前没有可处理的会话。");
      return;
    }
    forceArchiveActionDialogOpen.value = false;
    forceArchivePreviewLoading.value = true;
    forceArchivePreview.value = null;
    forceCompactionPreview.value = null;
    try {
      const [archivePreview, compactionPreview] = await Promise.all([
        invokeTauri<ForceArchivePreviewResult>("preview_force_archive_current", {
          input: {
            apiConfigId,
            agentId,
            departmentId: departmentId || null,
            conversationId,
          },
        }),
        invokeTauri<ForceCompactionPreviewResult>("preview_force_compact_current", {
          input: {
            apiConfigId,
            agentId,
            departmentId: departmentId || null,
            conversationId,
          },
        }),
      ]);
      forceArchivePreview.value = archivePreview;
      forceCompactionPreview.value = compactionPreview;
      forceArchiveActionDialogOpen.value = true;
    } catch (error) {
      closeForceArchiveActionDialog();
      options.setStatusError("status.loadConversationActionPreviewFailed", error);
    } finally {
      forceArchivePreviewLoading.value = false;
    }
  }

  async function confirmForceCompactionAction() {
    if (!forceCompactionPreview.value?.canCompact) return;
    closeForceArchiveActionDialog();
    await options.forceCompactNow();
  }

  async function confirmForceArchiveAction(targetConversationId?: string) {
    if (!forceArchivePreview.value?.canArchive) return;
    const normalizedTargetConversationId = String(targetConversationId || "").trim();
    closeForceArchiveActionDialog();
    await options.forceArchiveNow(normalizedTargetConversationId);
  }

  async function confirmDeleteConversationFromArchiveDialog() {
    const conversationId = String(options.currentChatConversationId.value || "").trim();
    if (!conversationId) return;
    closeForceArchiveActionDialog();
    await options.deleteUnarchivedConversationFromArchives(conversationId);
  }

  function openSkillPlaceholderDialog() {
    skillPlaceholderDialogOpen.value = true;
  }

  function closeSkillPlaceholderDialog() {
    skillPlaceholderDialogOpen.value = false;
  }

  function isApplyPatchArgsUndoable(rawArgs: string): boolean {
    const text = String(rawArgs || "").trim();
    if (!text) return false;
    if (text.startsWith("*** Begin Patch")) return true;
    if (!text.startsWith("{")) return false;
    try {
      const parsed = JSON.parse(text) as { input?: unknown };
      return typeof parsed.input === "string" && parsed.input.trim().startsWith("*** Begin Patch");
    } catch {
      return false;
    }
  }

  function getUndoAvailabilityForTurn(turnId: string): { canUndo: boolean; hint: string } {
    return inspectUndoablePatchCalls(options.allMessages.value || [], turnId, {
      isApplyPatchArgsUndoable,
    });
  }

  function requestRecallMode(payload: { turnId: string }): Promise<RecallMode> {
    const availability = getUndoAvailabilityForTurn(payload.turnId);
    console.info("[会话撤回] 打开撤回弹窗", {
      turnId: payload.turnId,
      canUndoPatch: availability.canUndo,
      hint: availability.hint || "",
    });
    rewindConfirmCanUndoPatch.value = availability.canUndo;
    rewindConfirmUndoHint.value = availability.hint;
    rewindConfirmDialogOpen.value = true;
    return new Promise((resolve) => {
      rewindConfirmResolver = resolve;
    });
  }

  function resolveRewindConfirm(mode: RecallMode) {
    console.info("[会话撤回] 弹窗确认", {
      mode,
      canUndoPatch: rewindConfirmCanUndoPatch.value,
      dialogOpen: rewindConfirmDialogOpen.value,
    });
    const resolver = rewindConfirmResolver;
    rewindConfirmResolver = null;
    rewindConfirmDialogOpen.value = false;
    rewindConfirmUndoHint.value = "";
    if (resolver) {
      resolver(mode);
    }
  }

  function confirmRewindWithPatch() {
    console.info("[会话撤回] 点击：撤回消息并撤回修改");
    resolveRewindConfirm("with_patch");
  }

  function confirmRewindMessageOnly() {
    console.info("[会话撤回] 点击：仅撤回消息");
    resolveRewindConfirm("message_only");
  }

  function cancelRewindConfirm() {
    console.info("[会话撤回] 点击：取消撤回");
    resolveRewindConfirm("cancel");
  }

  function cancelPendingRewindConfirm() {
    if (!rewindConfirmResolver) {
      rewindConfirmDialogOpen.value = false;
      rewindConfirmUndoHint.value = "";
      return;
    }
    const resolver = rewindConfirmResolver;
    rewindConfirmResolver = null;
    rewindConfirmDialogOpen.value = false;
    rewindConfirmUndoHint.value = "";
    resolver("cancel");
  }

  async function refreshRuntimeLogs() {
    runtimeLogsLoading.value = true;
    runtimeLogsError.value = "";
    try {
      const items = await invokeTauri<RuntimeLogEntry[]>("list_recent_runtime_logs");
      runtimeLogs.value = items;
    } catch (error) {
      runtimeLogsError.value = `加载运行日志失败：${String(error)}`;
    } finally {
      runtimeLogsLoading.value = false;
    }
  }

  function openRuntimeLogsDialog() {
    runtimeLogsDialogOpen.value = true;
    void (async () => {
      try {
        await invokeTauri("append_runtime_log_probe", {
          message: `日志窗口打开，window=${options.tauriWindowLabel.value}`,
        });
      } catch {
        // ignore probe write failure, do not block log list refresh
      }
      await refreshRuntimeLogs();
    })();
  }

  function closeRuntimeLogsDialog() {
    runtimeLogsDialogOpen.value = false;
  }

  async function clearRuntimeLogs() {
    runtimeLogsLoading.value = true;
    runtimeLogsError.value = "";
    try {
      await invokeTauri("clear_recent_runtime_logs");
      runtimeLogs.value = [];
    } catch (error) {
      runtimeLogsError.value = `清空运行日志失败：${String(error)}`;
    } finally {
      runtimeLogsLoading.value = false;
    }
  }

  function closeConfigSaveErrorDialog() {
    configSaveErrorDialogOpen.value = false;
  }

  function openConfigSaveErrorDialog(info: ConfigSaveErrorInfo) {
    configSaveErrorDialogTitle.value = options.t("status.saveConfigDialogTitle");
    if (info.kind === "hotkey_conflict") {
      configSaveErrorDialogKind.value = "warning";
      configSaveErrorDialogBody.value = `${options.t("status.saveConfigHotkeyOccupied", { hotkey: info.hotkey })}\n${options.t("status.saveConfigDialogHint")}`;
      options.configTab.value = "hotkey";
    } else if (info.kind === "backend_404") {
      configSaveErrorDialogKind.value = "error";
      configSaveErrorDialogBody.value = options.t("status.saveConfigBackend404");
    } else {
      configSaveErrorDialogKind.value = "error";
      configSaveErrorDialogBody.value = options.t("status.saveConfigFailed", { err: info.errorText });
    }
    configSaveErrorDialogOpen.value = true;
  }

  return {
    runtimeLogsDialogOpen,
    runtimeLogs,
    runtimeLogsLoading,
    runtimeLogsError,
    configSaveErrorDialogOpen,
    configSaveErrorDialogTitle,
    configSaveErrorDialogBody,
    configSaveErrorDialogKind,
    skillPlaceholderDialogOpen,
    forceArchiveActionDialogOpen,
    forceArchivePreviewLoading,
    forceArchivePreview,
    forceCompactionPreview,
    rewindConfirmDialogOpen,
    rewindConfirmCanUndoPatch,
    rewindConfirmUndoHint,
    openForceArchiveActionDialog,
    closeForceArchiveActionDialog,
    confirmForceCompactionAction,
    confirmForceArchiveAction,
    confirmDeleteConversationFromArchiveDialog,
    openSkillPlaceholderDialog,
    closeSkillPlaceholderDialog,
    requestRecallMode,
    confirmRewindWithPatch,
    confirmRewindMessageOnly,
    cancelRewindConfirm,
    cancelPendingRewindConfirm,
    refreshRuntimeLogs,
    openRuntimeLogsDialog,
    closeRuntimeLogsDialog,
    clearRuntimeLogs,
    closeConfigSaveErrorDialog,
    openConfigSaveErrorDialog,
  };
}
