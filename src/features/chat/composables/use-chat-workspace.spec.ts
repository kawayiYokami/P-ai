import { computed, ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeTauriMock = vi.hoisted(() => vi.fn());

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: invokeTauriMock,
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
