import { ref, type Ref } from "vue";
import { i18n } from "../../../i18n";
import { invokeTauri, openTransportWindow } from "../../../services/tauri-api";
import type { ChatMessage, RuntimeLogEntry, UnarchivedConversationSummary } from "../../../types/app";
import { useConfigSaveErrorDialog } from "./use-config-save-error-dialog";
import { useConversationMaintenanceDialog } from "../../chat/composables/use-conversation-maintenance-dialog";
import { useRewindConfirmation } from "../../chat/composables/use-rewind-confirmation";

export type { TrimCompactionPreviewResult, TrimPreviewResult } from "../../chat/composables/use-conversation-maintenance-dialog";

const t = i18n.global.t;

type UseShellDialogFlowsOptions = {
  t: (key: string, params?: Record<string, unknown>) => string;
  configTab: Ref<string>;
  allMessages: Ref<ChatMessage[]>;
  chatUsagePercent: Readonly<Ref<number>>;
  currentForegroundApiConfigId: Ref<string>;
  currentForegroundAgentId: Ref<string>;
  currentChatConversationId: Ref<string>;
  unarchivedConversations: Ref<UnarchivedConversationSummary[]>;
  setStatus: (message: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  trimCompactNow: () => Promise<void>;
  trimNow: (conversationId?: string | null) => Promise<void>;
  deleteConversation: (conversationId: string) => Promise<void> | void;
};

export function useShellDialogFlows(options: UseShellDialogFlowsOptions) {
  const runtimeLogsDialogOpen = ref(false);
  const runtimeLogs = ref<RuntimeLogEntry[]>([]);
  const runtimeLogsLoading = ref(false);
  const runtimeLogsError = ref("");
  const configSaveErrorDialog = useConfigSaveErrorDialog({
    t: options.t,
    configTab: options.configTab,
  });
  const skillPlaceholderDialogOpen = ref(false);
  const conversationMaintenanceDialog = useConversationMaintenanceDialog({
    t: options.t,
    currentConversationId: options.currentChatConversationId,
    chatUsagePercent: options.chatUsagePercent,
    conversationSummaries: options.unarchivedConversations,
    trimCompactNow: options.trimCompactNow,
    trimNow: options.trimNow,
    deleteConversation: options.deleteConversation,
    setStatus: options.setStatus,
    setStatusError: options.setStatusError,
  });
  const rewindConfirmation = useRewindConfirmation({
    currentConversationId: options.currentChatConversationId,
  });
  const {
    rewindConfirmDialogOpen,
    rewindConfirmCanUndoPatch,
    rewindConfirmUndoHint,
    branchFromMessageConfirmDialogOpen,
    requestRecallMode,
    confirmRewindWithPatch,
    confirmRewindMessageOnly,
    cancelRewindConfirm,
    cancelPendingRewindConfirm,
    requestCreateConversationBranchFromMessageConfirm,
    confirmBranchFromMessage,
    cancelBranchFromMessageConfirm,
    cancelPendingBranchFromMessageConfirm,
  } = rewindConfirmation;

  function openSkillPlaceholderDialog() {
    skillPlaceholderDialogOpen.value = true;
  }

  function closeSkillPlaceholderDialog() {
    skillPlaceholderDialogOpen.value = false;
  }

  async function refreshRuntimeLogs() {
    runtimeLogsLoading.value = true;
    runtimeLogsError.value = "";
    try {
      const items = await invokeTauri<RuntimeLogEntry[]>("list_recent_runtime_logs");
      runtimeLogs.value = items;
    } catch (error) {
      runtimeLogsError.value = t('sidebar.loadRuntimeLogsFailed', { error: String(error) });
    } finally {
      runtimeLogsLoading.value = false;
    }
  }

  function openRuntimeLogsDialog() {
    void openTransportWindow("runtimeLogs").catch((err) => {
      console.warn("[运行日志] 打开日志窗口失败", err);
    });
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
      runtimeLogsError.value = t('sidebar.clearRuntimeLogsFailed', { error: String(error) });
    } finally {
      runtimeLogsLoading.value = false;
    }
  }

  return {
    runtimeLogsDialogOpen,
    runtimeLogs,
    runtimeLogsLoading,
    runtimeLogsError,
    ...configSaveErrorDialog,
    skillPlaceholderDialogOpen,
    ...conversationMaintenanceDialog,
    rewindConfirmDialogOpen,
    rewindConfirmCanUndoPatch,
    rewindConfirmUndoHint,
    branchFromMessageConfirmDialogOpen,
    openSkillPlaceholderDialog,
    closeSkillPlaceholderDialog,
    requestRecallMode,
    requestCreateConversationBranchFromMessageConfirm,
    confirmRewindWithPatch,
    confirmRewindMessageOnly,
    cancelRewindConfirm,
    cancelPendingRewindConfirm,
    confirmBranchFromMessage,
    cancelBranchFromMessageConfirm,
    refreshRuntimeLogs,
    openRuntimeLogsDialog,
    closeRuntimeLogsDialog,
    clearRuntimeLogs,
  };
}
