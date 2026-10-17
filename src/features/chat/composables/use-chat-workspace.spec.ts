import { computed, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeTauriMock = vi.hoisted(() => vi.fn());
const gitPanelHeadStateMock = vi.hoisted(() => vi.fn());

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: invokeTauriMock,
  gitPanelHeadState: gitPanelHeadStateMock,
}));

vi.mock("vue-i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

import { useChatWorkspace } from "./use-chat-workspace";

type Deferred = {
  resolve: (value: unknown) => void;
  reject: (error: unknown) => void;
  promise: Promise<unknown>;
};

function deferred(): Deferred {
  let resolve!: (value: unknown) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<unknown>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { resolve, reject, promise };
}

function workspaceState(rootPath: string) {
  return {
    rootPath,
    workspaces: [],
    autonomousMode: false,
    shellWorkMode: "directory",
    shellWorkBranch: "",
    workspaceName: rootPath,
  };
}

/** 让已排队的 promise 回调跑完 */
async function flush() {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

function createWorkspace(conversationId: string) {
  const activeConversationId = ref(conversationId);
  const workspace = useChatWorkspace({
    activeConversationId: computed(() => activeConversationId.value),
    setStatus: vi.fn(),
    setStatusError: vi.fn(),
  });
  return { activeConversationId, workspace };
}

describe("useChatWorkspace 工作区刷新竞态", () => {
  beforeEach(() => {
    invokeTauriMock.mockReset();
    gitPanelHeadStateMock.mockReset();
  });

  it("读分支只查 head_state，不走会全量扫描工作区的 status", async () => {
    const { workspace } = createWorkspace("conv-a");
    gitPanelHeadStateMock.mockResolvedValueOnce({ branch: "feature/only-branch" });

    await expect(workspace.readChatWorkspaceBranch("E:/repo-a")).resolves.toBe("feature/only-branch");

    expect(gitPanelHeadStateMock).toHaveBeenCalledWith("E:/repo-a");
    expect(invokeTauriMock).not.toHaveBeenCalled();
  });

  it("丢弃过期响应：切会话后晚到的旧会话响应不覆盖新会话工作区", async () => {
    const { activeConversationId, workspace } = createWorkspace("conv-a");
    const pendingA = deferred();
    const pendingB = deferred();
    invokeTauriMock.mockImplementationOnce(() => pendingA.promise as never);
    invokeTauriMock.mockImplementationOnce(() => pendingB.promise as never);

    const first = workspace.refreshChatWorkspaceState();
    activeConversationId.value = "conv-b";
    const second = workspace.refreshChatWorkspaceState();

    // 新会话响应先到
    pendingB.resolve(workspaceState("E:/repo-b"));
    await flush();
    expect(workspace.chatWorkspaceRootPath.value).toBe("E:/repo-b");

    // 旧会话响应晚到，必须被丢弃
    pendingA.resolve(workspaceState("E:/repo-a"));
    await Promise.all([first, second]);
    expect(workspace.chatWorkspaceRootPath.value).toBe("E:/repo-b");
  });

  it("会话切走后，未重发的在途响应也不应用", async () => {
    const { activeConversationId, workspace } = createWorkspace("conv-a");
    const pendingA = deferred();
    invokeTauriMock.mockImplementationOnce(() => pendingA.promise as never);

    const request = workspace.refreshChatWorkspaceState();
    activeConversationId.value = "conv-b";

    pendingA.resolve(workspaceState("E:/repo-a"));
    await request;
    expect(workspace.chatWorkspaceRootPath.value).toBe("");
  });

  it("同一会话内乱序返回时保留最新一次请求的结果", async () => {
    const { workspace } = createWorkspace("conv-a");
    const older = deferred();
    const newer = deferred();
    invokeTauriMock.mockImplementationOnce(() => older.promise as never);
    invokeTauriMock.mockImplementationOnce(() => newer.promise as never);

    const first = workspace.refreshChatWorkspaceState();
    const second = workspace.refreshChatWorkspaceState();

    newer.resolve(workspaceState("E:/repo-new"));
    await flush();
    older.resolve(workspaceState("E:/repo-old"));
    await Promise.all([first, second]);

    expect(workspace.chatWorkspaceRootPath.value).toBe("E:/repo-new");
  });
});

describe("useChatWorkspace 组队模式", () => {
  beforeEach(() => {
    invokeTauriMock.mockReset();
    gitPanelHeadStateMock.mockReset();
  });

  it("切换组队时把预选队员人格一起提交，并用后端回显回填", async () => {
    const { workspace } = createWorkspace("conv-draft");
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conv-draft",
      teamEnabled: true,
      memberAgentIds: ["agent-b", "agent-c"],
    });

    await workspace.saveChatTeamMode(true, ["agent-b", "agent-c"]);

    expect(invokeTauriMock).toHaveBeenCalledWith("conversation.teamMode.set", {
      conversationId: "conv-draft",
      teamEnabled: true,
      memberAgentIds: ["agent-b", "agent-c"],
    });
    expect(workspace.chatWorkspaceTeamEnabled.value).toBe(true);
    expect(workspace.chatWorkspaceTeamAgentIds.value).toEqual(["agent-b", "agent-c"]);
  });

  it("切回单人时清空预选队员，失败则整体回滚", async () => {
    const { workspace } = createWorkspace("conv-draft");
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conv-draft",
      teamEnabled: true,
      memberAgentIds: ["agent-b"],
    });
    await workspace.saveChatTeamMode(true, ["agent-b"]);

    invokeTauriMock.mockRejectedValueOnce(new Error("boom"));
    await expect(workspace.saveChatTeamMode(false, [])).rejects.toThrow("boom");
    expect(workspace.chatWorkspaceTeamEnabled.value).toBe(true);
    expect(workspace.chatWorkspaceTeamAgentIds.value).toEqual(["agent-b"]);
  });
});
