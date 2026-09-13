import { computed, ref, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatShellWorkspaceState, ShellWorkspace, ShellWorkMode } from "../../../types/app";
import {
  defaultWorkspaceNameFromPath,
  inferWorkspaceName,
  isLegacyGenericWorkspaceName,
  normalizeWorkspaceLevel,
  workspaceLevelRank,
} from "../../../utils/shell-workspaces";
import { normalizeShellWorkMode, normalizeWorkspaceAccess } from "../../../utils/shell-workspaces";

export type ChatWorkspaceChoice = {
  id: string;
  name: string;
  path: string;
  level: ShellWorkspace["level"];
  access: ShellWorkspace["access"];
};

type UseChatWorkspaceOptions = {
  activeConversationId: ComputedRef<string>;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

export function useChatWorkspace(options: UseChatWorkspaceOptions) {
  const { t } = useI18n();
  const DEFAULT_CHAT_WORKSPACE_NAME = t("chat.defaultWorkspace");
  const chatWorkspaceName = ref(DEFAULT_CHAT_WORKSPACE_NAME);
  const chatWorkspacePath = ref("");
  const chatWorkspaceRootPath = ref("");
  const chatWorkspacePickerOpen = ref(false);
  const chatWorkspaceItems = ref<ShellWorkspace[]>([]);
  const chatWorkspaceAutonomousMode = ref(false);
  const chatWorkspaceWorkMode = ref<ShellWorkMode>("directory");
  const chatWorkspaceBranch = ref("");
  const chatWorkspaceWorktreePath = ref("");
  const chatWorkspaceWorktreeExists = ref(false);
  const chatWorkspaceWorktreeAvailable = ref(false);
  const chatWorkspaceWorktreeCheckMessage = ref("");
  let gitCheckSequence = 0;
  // 工作区状态刷新序号：会话切换会并发发起 workspace.list，晚到的旧响应必须丢弃，
  // 否则会把另一个会话的工作区根路径写进来（卡片墙 Git 卡等按它解析仓库）
  let workspaceRefreshSequence = 0;

  function normalizeWorkspaceChoice(item: ShellWorkspace, index: number): ChatWorkspaceChoice {
    const path = String(item.path || "").trim();
    const level = normalizeWorkspaceLevel(String(item.level || "").trim().toLowerCase());
    const rawName = String(item.name || "").trim();
    const name = isLegacyGenericWorkspaceName(level, rawName)
      ? inferWorkspaceName(level, path, index)
      : (rawName || defaultWorkspaceNameFromPath(path) || DEFAULT_CHAT_WORKSPACE_NAME);
    return {
      id: String(item.id || "").trim(),
      name,
      path,
      level,
      access: normalizeWorkspaceAccess(String(item.access || "")),
    };
  }

  function findWorkspaceChoiceByPath(path: string): ChatWorkspaceChoice | null {
    const target = String(path || "").trim().toLowerCase();
    if (!target) return null;
    return chatWorkspaceChoices.value.find((item) => item.path.toLowerCase() === target) ?? null;
  }

  function resolveWorkspaceDisplayName(path: string, workspaceName: string): string {
    const matched = findWorkspaceChoiceByPath(path);
    if (matched) return matched.name;
    const fallback = defaultWorkspaceNameFromPath(path);
    if (fallback) return fallback;
    return String(workspaceName || "").trim() || DEFAULT_CHAT_WORKSPACE_NAME;
  }

  const chatWorkspaceChoices = computed<ChatWorkspaceChoice[]>(() =>
    (chatWorkspaceItems.value || [])
      .map(normalizeWorkspaceChoice)
      .filter((item) => item.id && item.path)
      .sort((left, right) => {
        return workspaceLevelRank(left.level) - workspaceLevelRank(right.level);
      }),
  );
  const chatWorkspaceEffectiveAccess = computed<ShellWorkspace["access"]>(() => {
    const matched = findWorkspaceChoiceByPath(chatWorkspaceRootPath.value);
    if (matched) return matched.access;
    const mainWorkspace = chatWorkspaceChoices.value.find((item) => item.level === "main");
    if (mainWorkspace) return mainWorkspace.access;
    return "approval";
  });
  const chatWorkspacePermissionLabel = computed(() => {
    if (chatWorkspaceAutonomousMode.value) return t("chat.workspaceStatusPermissionAutonomous");
    if (chatWorkspaceEffectiveAccess.value === "full_access") return t("chat.workspaceStatusPermissionFull");
    return t("chat.workspaceStatusPermissionApproval");
  });
  const chatWorkspaceWorkModeLabel = computed(() => {
    if (chatWorkspaceWorkMode.value === "worktree") return t("chat.workspaceStatusModeWorktree");
    return t("chat.workspaceStatusModeDirectory");
  });
  const chatWorkspaceDisplayName = computed(() => {
    const workspaceName = String(chatWorkspaceName.value || "").trim() || DEFAULT_CHAT_WORKSPACE_NAME;
    return `${chatWorkspaceWorkModeLabel.value} · ${chatWorkspacePermissionLabel.value} · ${workspaceName}`;
  });

  function applyChatWorkspaceState(state: ChatShellWorkspaceState) {
    const nextPath = String(state.rootPath || "").trim();
    chatWorkspaceItems.value = Array.isArray(state.workspaces) ? state.workspaces : [];
    chatWorkspaceAutonomousMode.value = Boolean(state.autonomousMode);
    chatWorkspaceWorkMode.value = normalizeShellWorkMode(String(state.shellWorkMode || ""));
    chatWorkspaceBranch.value = String(state.shellWorkBranch || "").trim();
    chatWorkspaceWorktreePath.value = String((state as any).worktreePath || (state as any).worktree_path || "").trim();
    chatWorkspaceWorktreeExists.value = Boolean((state as any).worktreeExists ?? (state as any).worktree_exists);
    // worktree 已创建时 worktreePath 兜底
    if (chatWorkspaceWorkMode.value === "worktree" && !chatWorkspaceWorktreePath.value && nextPath) {
      chatWorkspaceWorktreePath.value = `${nextPath.replace(/\\/g, "/").replace(/\/+$/, "")}/.pai/.worktree/${String(options.activeConversationId.value || "").trim()}`;
    }
    chatWorkspaceName.value = resolveWorkspaceDisplayName(nextPath, String(state.workspaceName || "").trim());
    chatWorkspacePath.value = nextPath;
    chatWorkspaceRootPath.value = nextPath;
  }

  async function checkChatWorkspaceGitRoot(path: string): Promise<boolean> {
    const checkSequence = ++gitCheckSequence;
    const normalizedPath = String(path || "").trim();
    if (!normalizedPath) {
      chatWorkspaceWorktreeAvailable.value = false;
      chatWorkspaceWorktreeCheckMessage.value = "";
      return false;
    }
    chatWorkspaceWorktreeAvailable.value = false;
    chatWorkspaceWorktreeCheckMessage.value = t("chat.workspaceWorktreeChecking");
    try {
      const result = await invokeTauri<{ isGitRoot?: boolean; checked?: boolean; error?: string }>("workspace.gitRootCheck", {
        workspacePath: normalizedPath,
      });
      if (checkSequence !== gitCheckSequence) return Boolean(result.isGitRoot);
      chatWorkspaceWorktreeAvailable.value = Boolean(result.isGitRoot);
      chatWorkspaceWorktreeCheckMessage.value = result.error
        ? String(result.error)
        : (result.checked ? "" : "无法确认 Git 仓库");
      return chatWorkspaceWorktreeAvailable.value;
    } catch (error) {
      if (checkSequence !== gitCheckSequence) return false;
      chatWorkspaceWorktreeAvailable.value = false;
      chatWorkspaceWorktreeCheckMessage.value = error instanceof Error ? error.message : String(error);
      return false;
    }
  }

  function applyChatWorkspaceDraft(workspaces: ChatWorkspaceChoice[]) {
    chatWorkspaceItems.value = workspaces.map((item) => ({
      id: item.id,
      name: item.name,
      path: item.path,
      level: item.level,
      access: item.access,
      builtIn: item.level === "system",
    }));
    chatWorkspaceName.value = resolveWorkspaceDisplayName(
      chatWorkspacePath.value,
      chatWorkspaceName.value,
    );
  }

  async function refreshChatWorkspaceState() {
    const conversationId = String(options.activeConversationId.value || "").trim();
    const sequence = ++workspaceRefreshSequence;
    if (!conversationId) {
      chatWorkspaceName.value = DEFAULT_CHAT_WORKSPACE_NAME;
      chatWorkspacePath.value = "";
      chatWorkspaceRootPath.value = "";
      chatWorkspaceItems.value = [];
      chatWorkspaceAutonomousMode.value = false;
      chatWorkspaceWorkMode.value = "directory";
      chatWorkspaceBranch.value = "";
      chatWorkspaceWorktreePath.value = "";
      chatWorkspaceWorktreeExists.value = false;
      chatWorkspaceWorktreeAvailable.value = false;
      chatWorkspaceWorktreeCheckMessage.value = "";
      return;
    }
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("workspace.list", {
        conversationId,
      });
      // 过期响应丢弃：期间又发起过刷新，或当前会话已经切走
      if (sequence !== workspaceRefreshSequence) return;
      if (String(options.activeConversationId.value || "").trim() !== conversationId) return;
      applyChatWorkspaceState(state);
    } catch (error) {
      if (sequence !== workspaceRefreshSequence) return;
      console.warn("[工作区] refresh chat workspace failed:", error);
    }
  }

  function openChatWorkspacePicker() {
    chatWorkspacePickerOpen.value = true;
    // Git 探测只允许由用户显式打开工作目录面板触发，不能跟随前端刷新或会话切换。
    void checkChatWorkspaceGitRoot(chatWorkspaceRootPath.value);
  }

  function closeChatWorkspacePicker() {
    chatWorkspacePickerOpen.value = false;
  }

  async function saveChatWorkspaces(workspaces: ChatWorkspaceChoice[], autonomousMode?: boolean, workMode: ShellWorkMode = chatWorkspaceWorkMode.value, shellWorkBranch?: string) {
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!conversationId) {
      options.setStatus("当前会话未就绪，暂时不能设置工作目录");
      return;
    }
    const previousItems = [...chatWorkspaceItems.value];
    const previousName = chatWorkspaceName.value;
    const previousAutonomousMode = chatWorkspaceAutonomousMode.value;
    const previousWorkMode = chatWorkspaceWorkMode.value;
    const previousBranch = chatWorkspaceBranch.value;
    // 统一权限：全部目录共享同一 access（取主目录或首个），避免旧按目录分离
    const unifiedAccess = normalizeWorkspaceAccess(String(workspaces.find((w) => w.level === "main")?.access || workspaces[0]?.access || "approval"));
    const normalizedWorkspaces = workspaces.map((w) => ({ ...w, access: unifiedAccess as ChatWorkspaceChoice["access"] }));
    applyChatWorkspaceDraft(normalizedWorkspaces);
    chatWorkspaceAutonomousMode.value = Boolean(autonomousMode);
    chatWorkspaceWorkMode.value = normalizeShellWorkMode(String(workMode || ""));
    const nextBranch = shellWorkBranch !== undefined ? String(shellWorkBranch || "").trim() : previousBranch;
    chatWorkspaceBranch.value = nextBranch;
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("workspace.layout.save", {
        conversationId,
        autonomousMode: Boolean(autonomousMode),
        shellWorkMode: chatWorkspaceWorkMode.value,
        shellWorkBranch: chatWorkspaceBranch.value || null,
        workspaces: normalizedWorkspaces
          .filter((item) => item.level !== "system")
          .map((item) => ({
            id: item.id,
            name: item.name,
            path: item.path,
            level: item.level,
            access: item.access,
            builtIn: false,
          })),
      });
      applyChatWorkspaceState(state);
    } catch (error) {
      chatWorkspaceItems.value = previousItems;
      chatWorkspaceName.value = previousName;
      chatWorkspaceAutonomousMode.value = previousAutonomousMode;
      chatWorkspaceWorkMode.value = previousWorkMode;
      chatWorkspaceBranch.value = previousBranch;
      options.setStatusError("status.requestFailed", error);
      throw error;
    }
  }

  return {
    chatWorkspaceName,
    chatWorkspacePath,
    chatWorkspaceRootPath,
    chatWorkspacePickerOpen,
    chatWorkspaceChoices,
    chatWorkspaceAutonomousMode,
    chatWorkspaceWorkMode,
    chatWorkspaceBranch,
    chatWorkspaceWorktreePath,
    chatWorkspaceWorktreeExists,
    chatWorkspaceWorktreeAvailable,
    chatWorkspaceWorktreeCheckMessage,
    chatWorkspacePermissionLabel,
    chatWorkspaceDisplayName,
    refreshChatWorkspaceState,
    checkChatWorkspaceGitRoot,
    openChatWorkspacePicker,
    closeChatWorkspacePicker,
    saveChatWorkspaces,
  };
}
