// 会话分支守卫：防止用户在不知情的情况下在错误分支上工作。
//
// 判定发生在「按下发送」这一刻：主动回读会话工作区所在仓库的真实分支，与会话记录的分支比对。
// 不一致就弹一次确认——点「继续对话」才发送，并把会话记录更新为当前分支；
// 点「不发送」则什么都不做，草稿留在输入框里，下次再点发送还会再问。
//
// 不挂在会话切换或 Git 状态回填上：提示要出现在用户即将产出改动的那个动作上，
// 挂在状态回填上会让用户切一次会话就被弹一次，而那时他什么都还没做。
import type { ComputedRef, Ref } from "vue";
import type { ShellWorkMode } from "../../../types/app";

export type BranchGuardDecision =
  | { kind: "none" }
  | { kind: "record"; branch: string }
  | { kind: "prompt"; repoBranch: string; recordedBranch: string };

/**
 * 纯判定：只看「当前仓库分支」与「本会话工作分支」两个值。
 * - 当前分支为空（detached HEAD / 非 git 仓库）→ 不记录也不提示
 * - 记录为空 → 写入当前分支，不提示
 * - 相同 → 不动
 * - 不同 → 提示
 */
export function decideBranchGuard(repoBranch: string, recordedBranch: string): BranchGuardDecision {
  const current = String(repoBranch || "").trim();
  const recorded = String(recordedBranch || "").trim();
  if (!current) return { kind: "none" };
  if (!recorded) return { kind: "record", branch: current };
  if (recorded === current) return { kind: "none" };
  return { kind: "prompt", repoBranch: current, recordedBranch: recorded };
}

/** 路径归一：统一斜杠、去掉 Windows 扩展前缀与结尾斜杠、忽略大小写 */
export function normalizeGuardPath(path: string): string {
  return String(path || "")
    .trim()
    .replace(/^\\\\\?\\/, "")
    .replace(/\\/g, "/")
    .replace(/\/+$/, "")
    .toLowerCase();
}

/** 仓库根是否与给定工作区同源（相等、在其下、或在其上） */
export function isRepoInWorkspace(repoRoot: string, workspacePaths: string[]): boolean {
  const repo = normalizeGuardPath(repoRoot);
  if (!repo) return false;
  return (workspacePaths || []).some((path) => {
    const workspace = normalizeGuardPath(path);
    if (!workspace) return false;
    return repo === workspace || repo.startsWith(`${workspace}/`) || workspace.startsWith(`${repo}/`);
  });
}

/** 发送前的判定结果：ok 直接发送；prompt 需要用户先确认 */
export type BranchGuardCheck =
  | { kind: "ok" }
  | { kind: "prompt"; repoBranch: string; recordedBranch: string };

type UseChatBranchGuardOptions = {
  activeConversationId: ComputedRef<string>;
  /** 会话记录的工作分支 */
  recordedBranch: Ref<string> | ComputedRef<string>;
  /** recordedBranch 所属的会话：与当前会话不一致时说明工作区状态还没刷回来，不判定 */
  workspaceStateConversationId: Ref<string> | ComputedRef<string>;
  /** 会话工作区的根目录：判定就读它所在仓库的分支 */
  workspaceRootPath: Ref<string> | ComputedRef<string>;
  workMode: Ref<ShellWorkMode> | ComputedRef<ShellWorkMode>;
  /** 按工作目录回读真实分支；读不到（detached HEAD / 非 git）返回空串 */
  readBranchAtPath: (workspacePath: string) => Promise<string>;
  /** 按工作目录回读真实分支并复写会话记录；读不到时不写 */
  syncBranchAtPath: (workspacePath: string) => Promise<void>;
};

export function useChatBranchGuard(options: UseChatBranchGuardOptions) {
  function currentConversationId(): string {
    return String(options.activeConversationId.value || "").trim();
  }

  /** 参与判定的工作目录；工作树会话不提醒，返回空串 */
  function guardWorkspacePath(): string {
    if (String(options.workMode.value || "") === "worktree") return "";
    return String(options.workspaceRootPath.value || "").trim();
  }

  /**
   * 发送前主动判定一次。回读是网络调用，期间用户可能已经切走会话或换了工作区，
   * 所以回来后要重新确认会话与工作目录都没变，否则这一次判定作废（放行）。
   */
  async function checkBranchBeforeSend(): Promise<BranchGuardCheck> {
    const conversationId = currentConversationId();
    if (!conversationId) return { kind: "ok" };
    // 工作区状态还没刷回当前会话时，recordedBranch 还是上一个会话的值，不能判定
    if (String(options.workspaceStateConversationId.value || "").trim() !== conversationId) {
      return { kind: "ok" };
    }
    const workspacePath = guardWorkspacePath();
    if (!workspacePath) return { kind: "ok" };
    const branch = String((await options.readBranchAtPath(workspacePath)) || "").trim();
    if (currentConversationId() !== conversationId || guardWorkspacePath() !== workspacePath) {
      return { kind: "ok" };
    }
    if (!branch) return { kind: "ok" };
    const decision = decideBranchGuard(branch, options.recordedBranch.value);
    if (decision.kind === "none") return { kind: "ok" };
    if (decision.kind === "record") {
      // 旧会话没有记录（空记录语义是「未知」）：静默补上，不打扰用户
      void options.syncBranchAtPath(workspacePath);
      return { kind: "ok" };
    }
    return {
      kind: "prompt",
      repoBranch: decision.repoBranch,
      recordedBranch: decision.recordedBranch,
    };
  }

  /** 「继续对话」：把会话记录改成当前仓库分支。只改记录，不动仓库。 */
  function acceptBranchGuardPrompt(): void {
    const workspacePath = guardWorkspacePath();
    if (!workspacePath) return;
    void options.syncBranchAtPath(workspacePath);
  }

  return {
    checkBranchBeforeSend,
    acceptBranchGuardPrompt,
  };
}
