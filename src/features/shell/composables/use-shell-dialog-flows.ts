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

const SHORT_CONVERSATION_DELETE_THRESHOLD = 3;
const SHORT_CONVERSATION_COMPACTION_THRESHOLD = 10;

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

  function currentUnarchivedConversationSummary(): UnarchivedConversationSummary | null {
    const conversationId = String(options.currentChatConversationId.value || "").trim();
    if (!conversationId) return null;
    return options.unarchivedConversations.value.find(
      (item) => String(item.conversationId || "").trim() === conversationId,
    ) ?? null;
  }

  function countArchiveCandidateMessages(messages: ChatMessage[]): number {
    return messages.filter((message) => {
      const role = String(message.role || "").trim().toLowerCase();
      return role === "user" || role === "assistant";
    }).length;
  }

  function hasAssistantReply(messages: ChatMessage[]): boolean {
    return messages.some((message) => String(message.role || "").trim().toLowerCase() === "assistant");
  }

  function latestBackendContextUsagePercent(messages: ChatMessage[]): number {
    for (let idx = messages.length - 1; idx >= 0; idx -= 1) {
      const message = messages[idx];
      if (String(message.role || "").trim().toLowerCase() !== "assistant") continue;
      const raw = Number((message.providerMeta || {}).contextUsagePercent);
      if (!Number.isFinite(raw)) continue;
      return Math.min(100, Math.max(0, Math.round(raw)));
    }
    return 0;
  }

  function buildForceArchivePreview(conversationId: string): ForceArchivePreviewResult {
    const messages = options.allMessages.value || [];
    const summary = currentUnarchivedConversationSummary();
    const isMainConversation = summary?.isMainConversation === true;
    const messageCount = countArchiveCandidateMessages(messages);
    const assistantReplyPresent = hasAssistantReply(messages);
    const isEmpty = messages.length === 0;
    const archiveDisabledReason = isMainConversation
      ? "主会话暂不支持归档。"
      : summary?.runtimeState === "organizing_context"
      ? "当前会话正在后台归档或整理上下文，请稍候。"
      : isEmpty
        ? "当前会话为空，不能归档。"
        : !assistantReplyPresent
          ? "当前会话还没有助理回复，不能归档。"
          : messageCount <= SHORT_CONVERSATION_DELETE_THRESHOLD
            ? `当前会话过短（仅 ${messageCount} 条用户/助理消息），暂不建议归档。`
            : null;
    return {
      conversationId,
      canArchive: !archiveDisabledReason,
      canDropConversation: !isMainConversation,
      messageCount,
      hasAssistantReply: assistantReplyPresent,
      isEmpty,
      archiveDisabledReason,
    };
  }

  function buildForceCompactionPreview(conversationId: string): ForceCompactionPreviewResult {
    const messages = options.allMessages.value || [];
    const summary = currentUnarchivedConversationSummary();
    const messageCount = countArchiveCandidateMessages(messages);
    const assistantReplyPresent = hasAssistantReply(messages);
    const isEmpty = messages.length === 0;
    const contextUsagePercent = latestBackendContextUsagePercent(messages);
    const conversationLongEnough = messageCount >= SHORT_CONVERSATION_COMPACTION_THRESHOLD;
    const contextUsageHighEnough = contextUsagePercent >= 10;
    const compactionDisabledReason = summary?.runtimeState === "organizing_context"
      ? "当前会话正在整理上下文或归档处理中，请稍候。"
      : isEmpty
        ? "当前会话为空，无需整理。"
        : !assistantReplyPresent
          ? "当前会话还没有助理回复，暂不建议压缩。"
          : !conversationLongEnough && !contextUsageHighEnough
            ? contextUsagePercent > 0
              ? `当前会话较短（仅 ${messageCount} 条用户/助理消息），且上下文占用仅 ${contextUsagePercent}%，暂不建议压缩。`
              : `当前会话较短（仅 ${messageCount} 条用户/助理消息），暂不建议压缩。`
            : null;
    return {
      conversationId,
      canCompact: !compactionDisabledReason,
      messageCount,
      hasAssistantReply: assistantReplyPresent,
      isEmpty,
      contextUsagePercent,
      compactionDisabledReason,
    };
  }

  async function openForceArchiveActionDialog() {
    const conversationId = String(options.currentChatConversationId.value || "").trim();
    if (!conversationId) {
      options.setStatus("当前没有可处理的会话。");
      return;
    }
    forceArchiveActionDialogOpen.value = false;
    forceArchivePreviewLoading.value = true;
    forceArchivePreview.value = null;
    forceCompactionPreview.value = null;
    try {
      const archivePreview = buildForceArchivePreview(conversationId);
      const compactionPreview = buildForceCompactionPreview(conversationId);
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
    if (currentUnarchivedConversationSummary()?.isMainConversation) {
      closeForceArchiveActionDialog();
      options.setStatus("主会话暂不支持删除。");
      return;
    }
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
