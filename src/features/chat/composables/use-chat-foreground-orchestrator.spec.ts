import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeTauriMock = vi.hoisted(() => vi.fn());

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: invokeTauriMock,
  isTauriRuntimeAvailable: () => true,
  chatStreamNeedsFrontendBind: () => false,
}));

import { useChatForegroundOrchestrator } from "./use-chat-foreground-orchestrator";

function createBindings(shouldBindStream: boolean, order: string[]) {
  let finishUnbind: (() => void) | null = null;
  const flow = {
    clearForegroundRuntimeState: vi.fn(),
    unbindActiveConversationStream: vi.fn(() => {
      order.push("unbind-start");
      return new Promise<void>((resolve) => {
        finishUnbind = () => {
          order.push("unbind-finish");
          resolve();
        };
      });
    }),
    bindActiveConversationStream: vi.fn(async () => {
      order.push("bind");
    }),
    resumeForegroundRuntimeRound: vi.fn(),
  };
  const currentChatConversationId = ref("conversation-a");
  const bindings: Record<string, any> = {
    currentChatConversationId,
    currentChatPreferredApiConfigId: ref(""),
    currentChatTodos: ref([]),
    currentForegroundAgentId: ref(""),
    conversationForegroundSyncing: ref(false),
    allMessages: ref([]),
    foregroundTailLatestReady: ref(true),
    unarchivedConversations: ref([]),
    trimming: ref(false),
    trimmingConversationId: ref(""),
    compactingConversation: ref(false),
    compactingConversationId: ref(""),
    hasMoreBackendHistory: ref(false),
    FOREGROUND_SNAPSHOT_RECENT_LIMIT: 80,
    perfNow: () => 1,
    cacheConversationMessages: vi.fn(),
    clearConversationBadge: vi.fn(),
    markConversationReadPersisted: vi.fn(),
    clearPendingManualScrollToBottom: vi.fn(),
    beginForegroundPaintTrace: vi.fn(() => ({})),
    applyConversationSnapshot: vi.fn((snapshot) => {
      order.push("snapshot-apply");
      currentChatConversationId.value = snapshot.conversationId;
    }),
    triggerConversationScrollToBottom: vi.fn(),
    logForegroundPaintTrace: vi.fn(),
    getChatFlow: () => flow,
  };
  invokeTauriMock.mockImplementation(async (command: string) => {
    if (command !== "conversation.foregroundLightSnapshot") {
      throw new Error(`unexpected command: ${command}`);
    }
    order.push("snapshot-request");
    return {
      conversationId: "conversation-b",
      messages: [],
      shouldBindStream,
      runtimeState: shouldBindStream ? "assistant_streaming" : "idle",
      streamCache: shouldBindStream ? { persistedAssistantMessageId: "assistant-1" } : null,
    };
  });
  return {
    bindings,
    flow,
    finishUnbind: () => finishUnbind?.(),
  };
}

describe("useChatForegroundOrchestrator", () => {
  beforeEach(() => {
    invokeTauriMock.mockReset();
  });

  it("立即发起解绑和 snapshot，并在旧解绑完成后才绑定目标会话", async () => {
    const order: string[] = [];
    const { bindings, finishUnbind } = createBindings(true, order);
    const orchestrator = useChatForegroundOrchestrator(bindings);

    const switching = orchestrator.switchUnarchivedConversation("conversation-b");
    await vi.waitFor(() => {
      expect(order).toContain("snapshot-apply");
    });

    expect(order).toContain("unbind-start");
    expect(order).toContain("snapshot-request");
    expect(order).not.toContain("bind");

    finishUnbind();
    await switching;

    expect(order.indexOf("snapshot-request")).toBeLessThan(order.indexOf("unbind-finish"));
    expect(order.indexOf("unbind-finish")).toBeLessThan(order.indexOf("bind"));
  });

  it("snapshot 不需要流式绑定时保持解绑且不补绑", async () => {
    const order: string[] = [];
    const { bindings, flow, finishUnbind } = createBindings(false, order);
    const orchestrator = useChatForegroundOrchestrator(bindings);

    const switching = orchestrator.switchUnarchivedConversation("conversation-b");
    await Promise.resolve();
    finishUnbind();
    await switching;

    expect(flow.bindActiveConversationStream).not.toHaveBeenCalled();
  });

  it("快照应用后立即释放切换任务，角标/已读/滚动在后台收尾", async () => {
    const order: string[] = [];
    const { bindings, finishUnbind } = createBindings(false, order);
    const orchestrator = useChatForegroundOrchestrator(bindings);

    const switching = orchestrator.switchUnarchivedConversation("conversation-b");
    await vi.waitFor(() => {
      expect(order).toContain("snapshot-apply");
    });
    finishUnbind();
    // 切换任务完成，不等待渲染/滚动收尾
    await switching;
    // 收尾操作仍会执行（后台异步补跑，滚动按会话 id 校验）
    await vi.waitFor(() => {
      expect(bindings.clearConversationBadge).toHaveBeenCalledWith("conversation-b");
      expect(bindings.markConversationReadPersisted).toHaveBeenCalledWith("conversation-b");
      expect(bindings.triggerConversationScrollToBottom).toHaveBeenCalledWith(
        "conversation-b",
        "switch_snapshot_ready",
      );
    });
  });

  it("切到正在流式的会话时立即贴底并解锁跟随，不等落库", async () => {
    const order: string[] = [];
    const { bindings, finishUnbind } = createBindings(true, order);
    const orchestrator = useChatForegroundOrchestrator(bindings);

    const switching = orchestrator.switchUnarchivedConversation("conversation-b");
    await vi.waitFor(() => {
      expect(order).toContain("snapshot-apply");
    });
    finishUnbind();
    await switching;

    await vi.waitFor(() => {
      expect(bindings.triggerConversationScrollToBottom).toHaveBeenCalledWith(
        "conversation-b",
        "switch_streaming_ready",
        "follow",
      );
    });
  });
});
