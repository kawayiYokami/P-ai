import { ref, type Ref } from "vue";
import { openTransportWorkspaceDirectory } from "../../../services/tauri-api";
import type { ChatWorkspaceChoice } from "./use-chat-workspace";
import type { ShellWorkMode } from "../../../types/app";
import { normalizeShellWorkMode, normalizeWorkspaceAccess } from "../../../utils/shell-workspaces";

type UseChatWorkspacePickerFlowOptions = {
  chatWorkspaceChoices: Ref<ChatWorkspaceChoice[]>;
  chatWorkspaceAutonomousMode: Ref<boolean>;
  chatWorkspaceWorkMode: Ref<ShellWorkMode>;
  chatWorkspaceBranch: Ref<string>;
  chatWorkspaceWorktreePath: Ref<string>;
  chatWorkspaceWorktreeExists: Ref<boolean>;
  openChatWorkspacePickerBase: () => void;
  closeChatWorkspacePickerBase: () => void;
  saveChatWorkspaces: (items: ChatWorkspaceChoice[], autonomousMode?: boolean, workMode?: ShellWorkMode, shellWorkBranch?: string, shellWorktreePath?: string) => Promise<void>;
  setStatus: (message: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  workspaceAlreadyExistsText: string;
  worktreeRequiresApprovalText: string;
  worktreeUnavailableText: string;
  checkChatWorkspaceGitRoot: (path: string) => Promise<boolean>;
};

export function useChatWorkspacePickerFlow(options: UseChatWorkspacePickerFlowOptions) {
  const chatWorkspaceDraftChoices = ref<ChatWorkspaceChoice[]>([]);
  const chatWorkspaceDraftAutonomousMode = ref(false);
  const chatWorkspaceDraftWorkMode = ref<ShellWorkMode>("directory");
  const chatWorkspaceDraftBranch = ref("");
  /** 草稿绑定的已有工作树路径；非空表示这次保存要直接把会话绑到该工作树 */
  const chatWorkspaceDraftWorktreePath = ref("");
  const chatWorkspaceDraftError = ref("");
  const chatWorkspacePickerSaving = ref(false);

  function cloneChatWorkspaceChoices(items: ChatWorkspaceChoice[]): ChatWorkspaceChoice[] {
    return (items || []).map((item) => ({
      id: String(item.id || "").trim(),
      name: String(item.name || "").trim(),
      path: String(item.path || "").trim(),
      level: item.level,
      access: item.access,
    }));
  }

  function syncChatWorkspaceDraftFromCurrentState() {
    chatWorkspaceDraftChoices.value = cloneChatWorkspaceChoices(options.chatWorkspaceChoices.value);
    chatWorkspaceDraftAutonomousMode.value = Boolean(options.chatWorkspaceAutonomousMode.value);
    chatWorkspaceDraftWorkMode.value = normalizeShellWorkMode(String(options.chatWorkspaceWorkMode.value || ""));
    const persistedBranch = String(options.chatWorkspaceBranch.value || "").trim();
    chatWorkspaceDraftWorktreePath.value = String(options.chatWorkspaceWorktreePath.value || "").trim();
    chatWorkspaceDraftError.value = "";
    if (chatWorkspaceDraftWorkMode.value === "worktree") {
      // worktree：显示持久化的意图分支；已创建时会被对话框的 git 真值覆盖
      chatWorkspaceDraftBranch.value = persistedBranch;
    } else {
      // directory：分支显示由对话框按 git 真值回填，草稿本身不持有分支
      chatWorkspaceDraftBranch.value = "";
    }
  }

  function openChatWorkspacePicker() {
    syncChatWorkspaceDraftFromCurrentState();
    options.openChatWorkspacePickerBase();
  }

  function closeChatWorkspacePicker() {
    if (chatWorkspacePickerSaving.value) return;
    options.closeChatWorkspacePickerBase();
    syncChatWorkspaceDraftFromCurrentState();
  }

  async function addChatWorkspace() {
    // 已由 WorkspaceDirectoryPickerDialog 统一处理目录选择，此处保留为空实现以兼容旧的 @add-workspace 事件
    // 实际新增通过 addSecondaryPath(path) 完成
    return;
  }

  // 兼容旧调用：若外部传入路径则直接添加
  async function addChatWorkspaceWithPath(nextPath: string) {
    const normalized = String(nextPath || "").trim();
    if (!normalized) return;
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const existed = draft.some((item) => String(item.path || "").trim().toLowerCase() === normalized.toLowerCase());
    if (existed) {
      options.setStatus(options.workspaceAlreadyExistsText);
      return;
    }
    const hasMain = draft.some((item) => item.level === "main");
    const unifiedAccess = normalizeWorkspaceAccess(String(draft.find((w) => w.level === "main")?.access || draft[0]?.access || "approval"));
    draft.push({
      id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
      name: normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized,
      path: normalized,
      level: hasMain ? "secondary" : "main",
      access: unifiedAccess,
    });
    chatWorkspaceDraftChoices.value = draft;
  }

  async function setChatWorkspaceAsMain(workspaceId: string) {
    chatWorkspaceDraftError.value = "";
    const draft: ChatWorkspaceChoice[] = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value).map((item): ChatWorkspaceChoice => {
      if (item.level === "system") return item;
      if (item.id === workspaceId) {
        return { ...item, level: "main", access: item.access || "approval" };
      }
      if (item.level === "main") {
        return { ...item, level: "secondary" };
      }
      return item;
    });
    chatWorkspaceDraftChoices.value = draft;
  }

  function setChatWorkspaceAccess(access: ChatWorkspaceChoice["access"]) {
    chatWorkspaceDraftError.value = "";
    const normalized = normalizeWorkspaceAccess(String(access || ""));
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value).map((item) => {
      if (item.level === "system") return item;
      return { ...item, access: normalized as ChatWorkspaceChoice["access"] };
    });
    chatWorkspaceDraftChoices.value = draft;
  }

  // 兼容旧按目录调用：统一权限，忽略 workspaceId
  function setChatWorkspaceAccessLegacy(_workspaceId: string, access: ChatWorkspaceChoice["access"]) {
    setChatWorkspaceAccess(access);
  }

  async function removeChatWorkspace(workspaceId: string) {
    chatWorkspaceDraftError.value = "";
    const current = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const removing = current.find((item) => item.id === workspaceId);
    const draft = current.filter((item) => item.id !== workspaceId || item.level === "system");
    if (removing?.level === "main") {
      const promoteTarget = draft.find((item) => item.level === "secondary");
      if (promoteTarget) {
        draft.forEach((item) => {
          if (item.level === "system") return;
          if (item.id === promoteTarget.id) {
            item.level = "main";
          } else if (item.level === "main") {
            item.level = "secondary";
          }
        });
      }
    }
    chatWorkspaceDraftChoices.value = draft;
    // 主目录被删后，分支在 WorkspaceConfigCard 隐藏，保存时自动清空
    if (removing?.level === "main") {
      chatWorkspaceDraftBranch.value = "";
      chatWorkspaceDraftWorktreePath.value = "";
    }
  }

  function setChatWorkspaceAutonomousMode(enabled: boolean) {
    chatWorkspaceDraftAutonomousMode.value = Boolean(enabled);
  }

  function setChatWorkspaceWorkMode(mode: ShellWorkMode) {
    chatWorkspaceDraftError.value = "";
    const next = normalizeShellWorkMode(String(mode || ""));
    chatWorkspaceDraftWorkMode.value = next;
  }

  /** 第三级：对选中工作树执行 checkout（选中树不变，只换该树的分支） */
  function setChatWorkspaceBranch(branch: string) {
    const normalized = String(branch || "").trim();
    chatWorkspaceDraftBranch.value = normalized;
  }

  /** 第二级：选中某个已有工作树（空串=主工作树）；workMode 由 dialog 同步 emit */
  function setChatWorkspaceWorktreePath(worktreePath: string) {
    chatWorkspaceDraftWorktreePath.value = String(worktreePath || "").trim();
  }

  async function openChatWorkspaceDir(workspaceId: string) {
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const target = draft.find((item) => item.id === workspaceId);
    if (!target?.path) return;
    try {
      const opened = await openTransportWorkspaceDirectory(target.path);
      if (opened) options.setStatus(`已打开目录: ${opened}`);
    } catch (error) {
      options.setStatusError("config.tools.openDirFailed", error);
    }
  }

  function setMainPath(path: string) {
    const normalized = String(path || "").trim();
    if (!normalized) return;
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const existing = draft.find((item) => String(item.path || "").trim().toLowerCase() === normalized.toLowerCase());
    if (existing) {
      void setChatWorkspaceAsMain(existing.id);
      return;
    }
    const hasMain = draft.some((item) => item.level === "main");
    if (hasMain) {
      const newDraft = draft.map((item) => item.level === "main" ? { ...item, level: "secondary" as const } : item);
      const unifiedAccess = normalizeWorkspaceAccess(String(newDraft.find((w) => w.level === "secondary")?.access || newDraft[0]?.access || "approval"));
      newDraft.push({
        id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
        name: normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized,
        path: normalized,
        level: "main",
        access: unifiedAccess,
      });
      chatWorkspaceDraftChoices.value = newDraft;
    } else {
      const unifiedAccess = normalizeWorkspaceAccess(String(draft[0]?.access || "approval"));
      draft.push({
        id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
        name: normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized,
        path: normalized,
        level: "main",
        access: unifiedAccess,
      });
      chatWorkspaceDraftChoices.value = draft;
    }
    chatWorkspaceDraftBranch.value = "";
    chatWorkspaceDraftWorktreePath.value = "";
  }

  function addSecondaryPath(path: string) {
    const normalized = String(path || "").trim();
    if (!normalized) return;
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    if (draft.some((item) => String(item.path || "").trim().toLowerCase() === normalized.toLowerCase())) {
      options.setStatus(options.workspaceAlreadyExistsText);
      return;
    }
    const unifiedAccess = normalizeWorkspaceAccess(String(draft.find((w) => w.level === "main")?.access || draft[0]?.access || "approval"));
    draft.push({
      id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
      name: normalized.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || normalized,
      path: normalized,
      level: draft.some((item) => item.level === "main") ? "secondary" : "main",
      access: unifiedAccess,
    });
    chatWorkspaceDraftChoices.value = draft;
  }

  function removeSecondaryPath(path: string) {
    const key = String(path || "").trim().toLowerCase();
    if (!key) return;
    const matched = chatWorkspaceDraftChoices.value.find((item) => String(item.path || "").trim().toLowerCase() === key);
    if (matched) void removeChatWorkspace(matched.id);
  }

  async function saveChatWorkspacePicker() {
    if (chatWorkspacePickerSaving.value) return;
    chatWorkspacePickerSaving.value = true;
    try {
      const normalizedMode = normalizeShellWorkMode(String(chatWorkspaceDraftWorkMode.value || ""));
      const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
      // 旧 read_only 已迁移为 approval，此处仅保留 Git 根校验提示，worktree 不再阻断
      if (normalizedMode === "worktree") {
        const mainWorkspace = draft.find((item) => item.level === "main") || draft[0];
        if (mainWorkspace) {
          const gitOk = await options.checkChatWorkspaceGitRoot(String(mainWorkspace.path || ""));
          if (!gitOk) {
            chatWorkspaceDraftError.value = options.worktreeUnavailableText || options.worktreeRequiresApprovalText;
            options.setStatus(chatWorkspaceDraftError.value);
            return;
          }
        }
      }
      // 选中树是链接树时把路径与意图分支一并落库；主树则清空（=跟随主目录语义）
      const branchToSave = normalizedMode === "worktree" ? String(chatWorkspaceDraftBranch.value || "").trim() : "";
      const worktreeToSave = normalizedMode === "worktree" ? String(chatWorkspaceDraftWorktreePath.value || "").trim() : "";
      await options.saveChatWorkspaces(draft, chatWorkspaceDraftAutonomousMode.value, normalizedMode, branchToSave, worktreeToSave);
      options.closeChatWorkspacePickerBase();
      syncChatWorkspaceDraftFromCurrentState();
    } finally {
      chatWorkspacePickerSaving.value = false;
    }
  }

  return {
    chatWorkspaceDraftChoices,
    chatWorkspaceDraftAutonomousMode,
    chatWorkspaceDraftWorkMode,
    chatWorkspaceDraftBranch,
    chatWorkspaceDraftWorktreePath,
    chatWorkspaceDraftError,
    chatWorkspacePickerSaving,
    openChatWorkspacePicker,
    closeChatWorkspacePicker,
    addChatWorkspace,
    setChatWorkspaceAsMain,
    setMainPath,
    setChatWorkspaceAccess,
    setChatWorkspaceAccessLegacy,
    setChatWorkspaceBranch,
    setChatWorkspaceWorktreePath,
    setChatWorkspaceAutonomousMode,
    setChatWorkspaceWorkMode,
    removeChatWorkspace,
    addSecondaryPath,
    removeSecondaryPath,
    openChatWorkspaceDir,
    saveChatWorkspacePicker,
  };
}
