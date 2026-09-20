import { computed } from "vue";
import { useChatWorkspace } from "./use-chat-workspace";
import { useChatWorkspacePickerFlow } from "./use-chat-workspace-picker-flow";
import { useChatBranchGuard } from "./use-chat-branch-guard";

export function useChatWindowWorkspaceOrchestrator(bindings: Record<string, any>) {
  const workspace = useChatWorkspace({
    activeConversationId: computed(() => bindings.currentChatConversationId.value),
    setStatus: bindings.setStatus,
    setStatusError: bindings.setStatusError,
  });
  // 分支守卫在「按下发送」那一刻主动回读工作目录分支，不挂会话切换或 Git 状态回填
  const branchGuard = useChatBranchGuard({
    activeConversationId: computed(() => bindings.currentChatConversationId.value),
    recordedBranch: workspace.chatWorkspaceRecordedBranch,
    workspaceStateConversationId: workspace.chatWorkspaceStateConversationId,
    workspaceRootPath: workspace.chatWorkspaceRootPath,
    workMode: workspace.chatWorkspaceWorkMode,
    readBranchAtPath: workspace.readChatWorkspaceBranch,
    syncBranchAtPath: workspace.syncChatWorkspaceBranch,
  });
  const picker = useChatWorkspacePickerFlow({
    chatWorkspaceChoices: workspace.chatWorkspaceChoices,
    chatWorkspaceAutonomousMode: workspace.chatWorkspaceAutonomousMode,
    chatWorkspaceWorkMode: workspace.chatWorkspaceWorkMode,
    chatWorkspaceBranch: workspace.chatWorkspaceBranch,
    chatWorkspaceWorktreePath: workspace.chatWorkspaceWorktreePath,
    chatWorkspaceWorktreeExists: workspace.chatWorkspaceWorktreeExists,
    openChatWorkspacePickerBase: workspace.openChatWorkspacePicker,
    closeChatWorkspacePickerBase: workspace.closeChatWorkspacePicker,
    saveChatWorkspaces: workspace.saveChatWorkspaces,
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
