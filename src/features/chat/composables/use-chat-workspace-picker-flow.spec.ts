import { ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { ShellWorkMode } from "../../../types/app";
import { useChatWorkspacePickerFlow } from "./use-chat-workspace-picker-flow";

function createFlow(access: "approval" | "full_access" = "approval", mode: ShellWorkMode = "worktree", gitRootOk = true) {
  const saveChatWorkspaces = vi.fn(async () => undefined);
  const setStatus = vi.fn();
  const flow = useChatWorkspacePickerFlow({
    chatWorkspaceChoices: ref([{
      id: "main-workspace",
      name: "项目",
      path: "E:/project",
      level: "main",
      access,
    }]),
    chatWorkspaceAutonomousMode: ref(false),
    chatWorkspaceWorkMode: ref<ShellWorkMode>(mode),
    chatWorkspaceBranch: ref(""),
    chatWorkspaceWorktreePath: ref(""),
    chatWorkspaceWorktreeExists: ref(false),
    openChatWorkspacePickerBase: vi.fn(),
    closeChatWorkspacePickerBase: vi.fn(),
    saveChatWorkspaces,
    setStatus,
    setStatusError: vi.fn(),
    workspaceAlreadyExistsText: "目录已存在",
    worktreeRequiresApprovalText: "工作树模式至少需要审批权限。",
    worktreeUnavailableText: "目录不是 Git 根目录",
    checkChatWorkspaceGitRoot: vi.fn(async () => gitRootOk),
  });
  flow.openChatWorkspacePicker();
  return { flow, saveChatWorkspaces, setStatus };
}

describe("useChatWorkspacePickerFlow", () => {
  it("blocks saving worktree mode when git root check fails", async () => {
    const { flow, saveChatWorkspaces, setStatus } = createFlow("approval", "worktree", false);
    await flow.saveChatWorkspacePicker();
    expect(saveChatWorkspaces).not.toHaveBeenCalled();
    expect(setStatus).toHaveBeenCalledWith("目录不是 Git 根目录");
    expect(flow.chatWorkspaceDraftError.value).toBe("目录不是 Git 根目录");
  });
  it("persists worktree mode when git root check passes", async () => {
    const { flow, saveChatWorkspaces } = createFlow("approval", "worktree", true);
    await flow.saveChatWorkspacePicker();
    expect(saveChatWorkspaces).toHaveBeenCalledWith(expect.any(Array), false, "worktree", expect.any(String), expect.any(String));
  });
  it("persists directory mode regardless of git check", async () => {
    const { flow, saveChatWorkspaces } = createFlow("approval", "directory", false);
    await flow.saveChatWorkspacePicker();
    expect(saveChatWorkspaces).toHaveBeenCalledWith(expect.any(Array), false, "directory", "", "");
  });
  it("binds the picked worktree path when saving", async () => {
    const { flow, saveChatWorkspaces } = createFlow("approval", "worktree", true);
    flow.setChatWorkspaceWorktreePath("E:/repo/.pai/.worktree/20260921-a1b2c3d4");
    flow.setChatWorkspaceBranch("feat/relay");
    await flow.saveChatWorkspacePicker();
    expect(saveChatWorkspaces).toHaveBeenCalledWith(
      expect.any(Array),
      false,
      "worktree",
      "feat/relay",
      "E:/repo/.pai/.worktree/20260921-a1b2c3d4",
    );
  });
  it("keeps the worktree binding when the branch is checked out in-place", async () => {
    const { flow, saveChatWorkspaces } = createFlow("approval", "worktree", true);
    flow.setChatWorkspaceWorktreePath("E:/repo/.pai/.worktree/20260921-a1b2c3d4");
    // 第三级是选中树的真实 checkout：切分支不动工作树绑定
    flow.setChatWorkspaceBranch("main");
    await flow.saveChatWorkspacePicker();
    expect(saveChatWorkspaces).toHaveBeenCalledWith(
      expect.any(Array),
      false,
      "worktree",
      "main",
      "E:/repo/.pai/.worktree/20260921-a1b2c3d4",
    );
  });
});
