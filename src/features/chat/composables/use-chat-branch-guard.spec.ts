import { computed, ref } from "vue";
import { describe, expect, it } from "vitest";
import {
  decideBranchGuard,
  isRepoInWorkspace,
  normalizeGuardPath,
  useChatBranchGuard,
} from "./use-chat-branch-guard";
import type { ShellWorkMode } from "../../../types/app";

describe("decideBranchGuard", () => {
  it("当前分支为空（detached HEAD / 非 git 仓库）时不记录也不提示", () => {
    expect(decideBranchGuard("", "main")).toEqual({ kind: "none" });
    expect(decideBranchGuard("   ", "main")).toEqual({ kind: "none" });
    expect(decideBranchGuard("", "")).toEqual({ kind: "none" });
  });

  it("记录为空时写入当前分支，不提示", () => {
    expect(decideBranchGuard("main", "")).toEqual({ kind: "record", branch: "main" });
    expect(decideBranchGuard("  main  ", "   ")).toEqual({ kind: "record", branch: "main" });
  });

  it("记录与当前分支相同时不动", () => {
    expect(decideBranchGuard("main", "main")).toEqual({ kind: "none" });
    expect(decideBranchGuard("  main ", " main  ")).toEqual({ kind: "none" });
  });

  it("记录与当前分支不同时提示，两侧都做首尾空白归一", () => {
    expect(decideBranchGuard("main", "dev")).toEqual({
      kind: "prompt",
      repoBranch: "main",
      recordedBranch: "dev",
    });
    expect(decideBranchGuard(" main ", " dev ")).toEqual({
      kind: "prompt",
      repoBranch: "main",
      recordedBranch: "dev",
    });
  });
});

describe("normalizeGuardPath", () => {
  it("统一斜杠、去 Windows 扩展前缀与结尾斜杠、忽略大小写", () => {
    expect(normalizeGuardPath("E:\\GitHub\\Easy_Call_AI\\")).toBe("e:/github/easy_call_ai");
    expect(normalizeGuardPath("\\\\?\\E:\\repo")).toBe("e:/repo");
    expect(normalizeGuardPath("  /home/User/repo/  ")).toBe("/home/user/repo");
    expect(normalizeGuardPath("")).toBe("");
  });
});

describe("isRepoInWorkspace", () => {
  it("仓库根与会话工作区相等时判定成立", () => {
    expect(isRepoInWorkspace("E:\\repo", ["E:\\repo"])).toBe(true);
  });

  it("仓库根在工作区下一层时判定成立（会话选的是仓库的子目录）", () => {
    expect(isRepoInWorkspace("E:\\repo", ["E:\\repo\\packages\\app"])).toBe(true);
  });

  it("仓库根在工作区上一层时判定成立（会话选的是仓库根）", () => {
    expect(isRepoInWorkspace("E:\\repo\\packages\\app", ["E:\\repo"])).toBe(true);
  });

  it("无关路径不成立", () => {
    expect(isRepoInWorkspace("E:\\repo-a", ["E:\\repo-b"])).toBe(false);
    expect(isRepoInWorkspace("E:\\repo-abc", ["E:\\repo"])).toBe(false);
    expect(isRepoInWorkspace("E:\\other", ["E:\\repo", "E:\\another"])).toBe(false);
  });

  it("仓库根或工作区路径为空时不成立", () => {
    expect(isRepoInWorkspace("", ["E:\\repo"])).toBe(false);
    expect(isRepoInWorkspace("E:\\repo", [])).toBe(false);
    expect(isRepoInWorkspace("E:\\repo", ["", "   "])).toBe(false);
  });

  it("大小写与斜杠方向不同的同一路径仍判定成立", () => {
    expect(isRepoInWorkspace("e:/GITHUB/repo/", ["E:\\github\\REPO"])).toBe(true);
  });
});

// 发送前判定：主动回读会话工作区所在仓库的分支，再与会话记录比对。
// 这里只覆盖「什么时候放行、什么时候拦住、什么时候写记录」，不涉及弹窗与发送本身。
describe("useChatBranchGuard.checkBranchBeforeSend", () => {
  function createGuard(overrides: {
    conversationId?: string;
    recordedBranch?: string;
    stateConversationId?: string;
    workspaceRootPath?: string;
    workMode?: ShellWorkMode;
    branch?: string;
    onRead?: (path: string) => void;
  } = {}) {
    const conversationId = ref(overrides.conversationId ?? "conv-1");
    const recordedBranch = ref(overrides.recordedBranch ?? "main");
    const stateConversationId = ref(overrides.stateConversationId ?? "conv-1");
    const workspaceRootPath = ref(overrides.workspaceRootPath ?? "E:/repo");
    const workMode = ref<ShellWorkMode>(overrides.workMode ?? "directory");
    const readCalls: string[] = [];
    const syncCalls: string[] = [];
    const guard = useChatBranchGuard({
      activeConversationId: computed(() => conversationId.value),
      recordedBranch,
      workspaceStateConversationId: stateConversationId,
      workspaceRootPath,
      workMode,
      readBranchAtPath: async (path: string) => {
        readCalls.push(path);
        overrides.onRead?.(path);
        return overrides.branch ?? "main";
      },
      syncBranchAtPath: async (path: string) => {
        syncCalls.push(path);
      },
    });
    return {
      guard,
      conversationId,
      recordedBranch,
      stateConversationId,
      workspaceRootPath,
      readCalls,
      syncCalls,
    };
  }

  it("分支一致：放行，不写记录", async () => {
    const ctx = createGuard({ branch: "main", recordedBranch: "main" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.readCalls).toEqual(["E:/repo"]);
    expect(ctx.syncCalls).toEqual([]);
  });

  it("记录为空（旧会话）：静默补上当前分支并放行，不打扰用户", async () => {
    const ctx = createGuard({ branch: "dev", recordedBranch: "" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.syncCalls).toEqual(["E:/repo"]);
  });

  it("分支不一致：拦住这次发送，把两个分支都带出来", async () => {
    const ctx = createGuard({ branch: "main", recordedBranch: "dev" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({
      kind: "prompt",
      repoBranch: "main",
      recordedBranch: "dev",
    });
    expect(ctx.syncCalls).toEqual([]);
  });

  it("读不到分支（detached HEAD / 非 git 仓库）：放行，不记录", async () => {
    const ctx = createGuard({ branch: "", recordedBranch: "dev" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.syncCalls).toEqual([]);
  });

  it("工作树会话：不判定也不回读", async () => {
    const ctx = createGuard({ workMode: "worktree", branch: "main", recordedBranch: "dev" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.readCalls).toEqual([]);
  });

  it("工作区状态还没刷回当前会话：不判定，避免拿上一个会话的记录误报", async () => {
    const ctx = createGuard({ stateConversationId: "conv-0", branch: "main", recordedBranch: "dev" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.readCalls).toEqual([]);
  });

  it("回读期间切走会话：这次判定作废，放行且不写记录", async () => {
    const ctx = createGuard({
      branch: "dev",
      recordedBranch: "",
      onRead: () => {
        ctx.conversationId.value = "conv-2";
      },
    });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.syncCalls).toEqual([]);
  });

  it("没有工作区根：不判定", async () => {
    const ctx = createGuard({ workspaceRootPath: "", branch: "main", recordedBranch: "dev" });
    await expect(ctx.guard.checkBranchBeforeSend()).resolves.toEqual({ kind: "ok" });
    expect(ctx.readCalls).toEqual([]);
  });

  it("「继续对话」按会话工作区根复写记录", async () => {
    const ctx = createGuard({ workspaceRootPath: "E:/repo" });
    ctx.guard.acceptBranchGuardPrompt();
    expect(ctx.syncCalls).toEqual(["E:/repo"]);
  });
});
