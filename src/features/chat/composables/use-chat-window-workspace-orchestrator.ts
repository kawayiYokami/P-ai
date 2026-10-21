import { computed } from "vue";
import { useChatWorkspace } from "./use-chat-workspace";
import { useChatWorkspacePickerFlow } from "./use-chat-workspace-picker-flow";
import { isRepoInWorkspace, useChatBranchGuard } from "./use-chat-branch-guard";
import { useWorkspaceGitStatus } from "../../file-reader/composables/use-workspace-git-status";

export function useChatWindowWorkspaceOrchestrator(bindings: Record<string, any>) {
  const workspace = useChatWorkspace({
    activeConversationId: computed(() => bindings.currentChatConversationId.value),
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
  });
  // 首页 Git 卡与侧边栏共用的实时状态：当前分支已经在这里，发送时直接读它，不再单独跑 git
  const gitStatus = useWorkspaceGitStatus();
  // 分支守卫在「按下发送」那一刻比对：会话记录的分支 vs 已经知道的当前分支
  const branchGuard = useChatBranchGuard({
    activeConversationId: computed(() => bindings.currentChatConversationId.value),
    recordedBranch: workspace.chatWorkspaceRecordedBranch,
    workspaceStateConversationId: workspace.chatWorkspaceStateConversationId,
    workspaceRootPath: workspace.chatWorkspaceRootPath,
    workMode: workspace.chatWorkspaceWorkMode,
    knownBranchAtPath: (path: string) => {
      if (!gitStatus.statusLoaded.value) return "";
      const knownRoot = String(gitStatus.repoRoot.value || "");
      if (!isRepoInWorkspace(knownRoot, [path])) return "";
      return String(gitStatus.currentBranch.value || "");
    },
    recordBranch: workspace.recordChatWorkspaceBranch,
    syncBranchAtPath: workspace.syncChatWorkspaceBranch,
  });
  const picker = useChatWorkspacePickerFlow({
    chatWorkspaceChoices: workspace.chatWorkspaceChoices,
    chatWorkspaceAutonomousMode: workspace.chatWorkspaceAutonomousMode,
    chatWorkspaceTeamEnabled: workspace.chatWorkspaceTeamEnabled,
    chatWorkspaceWorkMode: workspace.chatWorkspaceWorkMode,
    chatWorkspaceBranch: workspace.chatWorkspaceBranch,
    chatWorkspaceWorktreePath: workspace.chatWorkspaceWorktreePath,
    chatWorkspaceWorktreeExists: workspace.chatWorkspaceWorktreeExists,
    openChatWorkspacePickerBase: workspace.openChatWorkspacePicker,
    closeChatWorkspacePickerBase: workspace.closeChatWorkspacePicker,
    saveChatWorkspaces: workspace.saveChatWorkspaces,
    saveChatTeamMode: workspace.saveChatTeamMode,
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
    workspaceAlreadyExistsText: bindings.tr("config.tools.workspaceAlreadyExists"),
    worktreeRequiresApprovalText: bindings.tr("chat.workspaceWorktreeRequiresApproval"),
    worktreeUnavailableText: bindings.tr("chat.workspaceWorktreeUnavailable"),
    checkChatWorkspaceGitRoot: workspace.checkChatWorkspaceGitRoot,
  });

  return {
    ...workspace,
    ...picker,
    ...branchGuard,
  };
}
