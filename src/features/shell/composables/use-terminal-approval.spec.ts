import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeTauriMock = vi.hoisted(() => vi.fn());

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: invokeTauriMock,
}));

import {
  useTerminalApproval,
  type TerminalApprovalRequestPayload,
} from "./use-terminal-approval";

function approval(requestId: string): TerminalApprovalRequestPayload {
  return {
    requestId,
    title: "终端审批",
    message: "确认执行",
    approvalKind: "terminal",
    sessionId: "conversation-a",
  };
}

function createApprovalApi(requestIds: string[]) {
  const queue = ref<TerminalApprovalRequestPayload[]>(requestIds.map(approval));
  const resolving = ref(false);
  const api = useTerminalApproval({ queue, resolving });
  return { queue, resolving, api };
}

function queueIds(queue: { value: TerminalApprovalRequestPayload[] }) {
  return queue.value.map((item) => item.requestId);
}

beforeEach(() => {
  invokeTauriMock.mockReset();
});

describe("useTerminalApproval", () => {
  it("处理审批期间本窗口已收到 resolved 事件时，不误删队列里的下一条审批", async () => {
    const { queue, api } = createApprovalApi(["a", "b"]);
    const holder: { release?: () => void } = {};
    invokeTauriMock.mockImplementationOnce(
      () => new Promise<void>((resolve) => {
        holder.release = resolve;
      }),
    );

    api.approveTerminalApproval("a");
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledTimes(1));

    // 后端在命令返回之前就广播了 resolved，本窗口先收到并摘掉 a，队列左移
    api.removeTerminalApprovalRequest("a");
    holder.release?.();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(queueIds(queue)).toEqual(["b"]);
  });

  it("重新拉取期间新到达的审批不会被旧快照清掉", async () => {
    const { queue, api } = createApprovalApi(["a"]);
    const holder: { release?: (items: TerminalApprovalRequestPayload[]) => void } = {};
    invokeTauriMock.mockImplementationOnce(
      () => new Promise<TerminalApprovalRequestPayload[]>((resolve) => {
        holder.release = resolve;
      }),
    );

    const rehydrate = api.rehydrateTerminalApprovals();
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledTimes(1));

    // 快照在途时后端新开了一条审批，它不在快照里但确实有效
    api.enqueueTerminalApprovalRequest(approval("c"));
    holder.release?.([approval("a")]);
    await rehydrate;

    expect(queueIds(queue)).toEqual(["a", "c"]);
  });

  it("重新拉取时撤下后端已经不存在的审批", async () => {
    const { queue, api } = createApprovalApi(["a", "b"]);
    invokeTauriMock.mockResolvedValueOnce([approval("b")]);

    const added = await api.rehydrateTerminalApprovals();

    expect(queueIds(queue)).toEqual(["b"]);
    expect(added).toBe(0);
  });
});
