import { effectScope, nextTick, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeTauriMock = vi.hoisted(() => vi.fn());
const onTransportNotificationMock = vi.hoisted(() => vi.fn());
const rewindCompletedHandlers = vi.hoisted(() => new Set<Function>());
const useChatFlowOptionsHolder = vi.hoisted(() => ({ value: null as any }));
const flowMock = vi.hoisted(() => ({
  bindingId: "side-view-test",
  frontendRoundPhase: { value: "idle" as "idle" | "queued" | "waiting" | "streaming" },
  unbindActiveConversationStream: vi.fn(async () => {}),
  bindActiveConversationStream: vi.fn(async () => {}),
  clearForegroundRuntimeState: vi.fn(),
  resumeForegroundRuntimeRound: vi.fn(),
  probeBoundChannel: vi.fn(async () => true),
  hasActiveBoundDeltaChannel: vi.fn(() => false),
  readConversationStreamCache: vi.fn<(conversationId?: string) => any>(() => null),
  handleExternalAssistantDelta: vi.fn(),
  handleExternalHistoryFlushed: vi.fn(),
  handleExternalRoundStarted: vi.fn(async () => {}),
  handleExternalRoundCompleted: vi.fn(async () => {}),
  handleExternalRoundFailed: vi.fn(async () => {}),
  handleExternalStreamRebindRequired: vi.fn(async () => {}),
  sendChat: vi.fn(),
  stopChat: vi.fn(),
}));

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: invokeTauriMock,
  isTauriRuntimeAvailable: vi.fn(() => true),
  chatStreamNeedsFrontendBind: vi.fn(() => false),
  bindTransportConversationStream: vi.fn(async () => {}),
  unbindTransportConversationStream: vi.fn(async () => {}),
  probeTransportConversationStream: vi.fn(async () => true),
  onTransportNotification: (method: string, handler: unknown) => {
    if (method === "chat.rewindCompleted") rewindCompletedHandlers.add(handler as Function);
    return () => {};
  },
}));

vi.mock("./use-chat-flow", () => ({
  useChatFlow: (options: unknown) => {
    useChatFlowOptionsHolder.value = options;
    return flowMock;
  },
}));

import { useConversationViewRuntime } from "./use-conversation-view-runtime";

function message(id: string, text: string, role: "user" | "assistant" = "assistant") {
  return {
    id,
    role,
    createdAt: `2026-07-17T00:00:0${id.length}Z`,
    parts: [{ type: "text", text }],
  } as any;
}

function createEventTarget() {
  const target = new EventTarget();
  return {
    addEventListener: target.addEventListener.bind(target),
    removeEventListener: target.removeEventListener.bind(target),
    dispatchEvent: target.dispatchEvent.bind(target),
  };
}

async function createRuntime(conversationId = "conversation-a", subscriptionSlot?: any) {
  const windowTarget = createEventTarget();
  const documentTarget = Object.assign(createEventTarget(), { visibilityState: "visible" });
  vi.stubGlobal("window", windowTarget);
  vi.stubGlobal("document", documentTarget);
  const scope = effectScope();
  const id = ref(conversationId);
  const runtime = scope.run(() => useConversationViewRuntime({
    conversationId: id,
    apiConfigId: ref("api-a"),
    agentId: ref("agent-a"),
    subscriptionSlot,
    t: (key) => key,
  }));
  await nextTick();
  return { runtime: runtime!, scope, id, windowTarget, documentTarget };
}

describe("useConversationViewRuntime", () => {
  beforeEach(() => {
    invokeTauriMock.mockReset();
    rewindCompletedHandlers.clear();
    onTransportNotificationMock.mockReset();
    flowMock.frontendRoundPhase.value = "idle";
    flowMock.unbindActiveConversationStream.mockReset().mockResolvedValue(undefined);
    flowMock.bindActiveConversationStream.mockReset().mockResolvedValue(undefined);
    flowMock.clearForegroundRuntimeState.mockReset();
    flowMock.resumeForegroundRuntimeRound.mockReset();
    flowMock.probeBoundChannel.mockReset().mockResolvedValue(true);
    flowMock.hasActiveBoundDeltaChannel.mockReset().mockReturnValue(false);
    flowMock.readConversationStreamCache.mockReset().mockReturnValue(null);
    flowMock.handleExternalRoundStarted.mockReset().mockResolvedValue(undefined);
    flowMock.handleExternalRoundCompleted.mockReset().mockResolvedValue(undefined);
    flowMock.handleExternalRoundFailed.mockReset().mockResolvedValue(undefined);
    useChatFlowOptionsHolder.value = null;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("先应用权威快照，等待旧解绑完成后强制绑定并恢复轮次", async () => {
    let finishUnbind: (() => void) | undefined;
    flowMock.unbindActiveConversationStream.mockImplementationOnce(() => new Promise<void>((resolve) => {
      finishUnbind = resolve;
    }));
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("assistant-1", "partial")],
      runtimeState: "assistant_streaming",
      shouldBindStream: true,
      streamCache: { persistedAssistantMessageId: "assistant-1", assistantText: "partial", hasVisibleProgress: true },
    });

    const { runtime, scope } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));
    expect(flowMock.bindActiveConversationStream).not.toHaveBeenCalled();

    finishUnbind?.();
    await vi.waitFor(() => expect(flowMock.bindActiveConversationStream).toHaveBeenCalledWith("conversation-a", true));
    expect(flowMock.resumeForegroundRuntimeRound).toHaveBeenCalledWith(expect.objectContaining({
      conversationId: "conversation-a",
    }));
    scope.stop();
  });

  it("没有正式 assistant 消息时即使旧快照声称需要绑定也保持解绑", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("assistant-1", "历史正文")],
      shouldBindStream: true,
      streamCache: null,
    });

    const { runtime, scope } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));

    expect(runtime.runtimeState.value).toBe("idle");
    expect(runtime.conversationBusy.value).toBe(false);
    expect(flowMock.bindActiveConversationStream).not.toHaveBeenCalled();
    scope.stop();
  });

  it("按会话事件更新权威忙碌态并合并后台追加消息", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("user-1", "question", "user")],
      runtimeState: "idle",
      shouldBindStream: false,
    });
    flowMock.frontendRoundPhase.value = "streaming";
    flowMock.hasActiveBoundDeltaChannel.mockReturnValue(true);

    const { runtime, scope } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));
    const handlers = runtime.flow as any;
    handlers.handleExternalRuntimeStateUpdated({
      conversationId: "conversation-a",
      runtimeState: "assistant_streaming",
    });
    handlers.handleExternalMessageAppended({
      conversationId: "conversation-a",
      message: message("assistant-1", "answer"),
    });

    expect(runtime.runtimeState.value).toBe("assistant_streaming");
    expect(runtime.conversationBusy.value).toBe(true);
    expect(runtime.allMessages.value.map((item) => item.id)).toEqual(["user-1", "assistant-1"]);
    scope.stop();
  });

  it("流式期间 conversationBusy 为 true（flow 发送保护保留），视图层忙碌由 isViewLayerBusy 判定（chat-view-busy.spec.ts 钉死）", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("user-1", "question", "user")],
      runtimeState: "idle",
      shouldBindStream: false,
    });
    flowMock.frontendRoundPhase.value = "streaming";
    flowMock.hasActiveBoundDeltaChannel.mockReturnValue(true);

    const { runtime, scope } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));
    const handlers = runtime.flow as any;
    handlers.handleExternalRuntimeStateUpdated({
      conversationId: "conversation-a",
      runtimeState: "assistant_streaming",
    });

    expect(runtime.runtimeState.value).toBe("assistant_streaming");
    expect(runtime.conversationBusy.value).toBe(true);
    scope.stop();
  });

  it("串行化同一视图的重复快照恢复，后发恢复不会被旧请求反向覆盖", async () => {
    let resolveFirst: ((value: unknown) => void) | undefined;
    let activeRequests = 0;
    let maxActiveRequests = 0;
    invokeTauriMock.mockImplementation((command: string) => {
      if (command !== "conversation.foregroundLightSnapshot") {
        return Promise.resolve({});
      }
      activeRequests += 1;
      maxActiveRequests = Math.max(maxActiveRequests, activeRequests);
      if (!resolveFirst) {
        return new Promise((resolve) => {
          resolveFirst = (value) => {
            activeRequests -= 1;
            resolve(value);
          };
        });
      }
      activeRequests -= 1;
      return Promise.resolve({
        conversationId: "conversation-a",
        messages: [message("assistant-new", "new")],
        runtimeState: "idle",
        shouldBindStream: false,
      });
    });

    const { runtime, scope } = await createRuntime();
    const reload = runtime.loadSnapshot();
    resolveFirst?.({
      conversationId: "conversation-a",
      messages: [message("assistant-old", "old")],
      runtimeState: "idle",
      shouldBindStream: false,
    });
    await reload;

    expect(maxActiveRequests).toBe(1);
    expect(runtime.allMessages.value.map((item) => item.id)).toEqual(["assistant-new"]);
    scope.stop();
  });

  it("切回前台且 Channel 探针健康时保留当前流，不重新加载消息", async () => {
    const streamCache = {
      activationId: "activation-a",
      requestId: "request-a",
      updatedAt: "2026-07-17T00:00:10Z",
      persistedAssistantMessageId: "assistant-1",
      assistantText: "partial",
      hasVisibleProgress: true,
    };
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          messages: [message("assistant-1", "partial")],
          runtimeState: "assistant_streaming",
          shouldBindStream: true,
          streamCache,
        });
      }
      if (command === "conversation.runtimeSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          runtimeState: "assistant_streaming",
          streamCache,
        });
      }
      return Promise.resolve({});
    });
    flowMock.readConversationStreamCache.mockReturnValue(streamCache);

    const { scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(flowMock.bindActiveConversationStream).toHaveBeenCalledTimes(1));
    flowMock.frontendRoundPhase.value = "streaming";
    windowTarget.dispatchEvent(new Event("focus"));

    await vi.waitFor(() => expect(flowMock.probeBoundChannel).toHaveBeenCalledWith("conversation-a"));
    expect(flowMock.bindActiveConversationStream).toHaveBeenCalledTimes(1);
    expect(invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot")).toHaveLength(1);
    scope.stop();
  });

  it("恢复进行中再次收到前台触发时，完成后会再执行一次最新对账", async () => {
    const streamCache = {
      activationId: "activation-a",
      requestId: "request-a",
      updatedAt: "2026-07-17T00:00:10Z",
      persistedAssistantMessageId: "assistant-1",
      assistantText: "partial",
      hasVisibleProgress: true,
    };
    let runtimeSnapshotCalls = 0;
    let resolveFirstRuntimeSnapshot: ((value: unknown) => void) | undefined;
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          messages: [message("assistant-1", "partial")],
          runtimeState: "assistant_streaming",
          shouldBindStream: true,
          streamCache,
        });
      }
      if (command === "conversation.runtimeSnapshot") {
        runtimeSnapshotCalls += 1;
        if (runtimeSnapshotCalls === 1) {
          return new Promise((resolve) => {
            resolveFirstRuntimeSnapshot = resolve;
          });
        }
        return Promise.resolve({
          conversationId: "conversation-a",
          runtimeState: "assistant_streaming",
          streamCache,
        });
      }
      return Promise.resolve({});
    });
    flowMock.readConversationStreamCache.mockReturnValue(streamCache);

    const { scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(flowMock.bindActiveConversationStream).toHaveBeenCalledTimes(1));
    flowMock.frontendRoundPhase.value = "streaming";
    windowTarget.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect(resolveFirstRuntimeSnapshot).toBeTypeOf("function"));
    windowTarget.dispatchEvent(new Event("focus"));
    resolveFirstRuntimeSnapshot?.({
      conversationId: "conversation-a",
      runtimeState: "assistant_streaming",
      streamCache,
    });

    await vi.waitFor(() => expect(runtimeSnapshotCalls).toBe(2));
    scope.stop();
  });

  it("后端已完成但追问仍在流式时只刷新目标消息并收口", async () => {
    const streamCache = {
      activationId: "activation-a",
      requestId: "request-a",
      updatedAt: "2026-07-17T00:00:10Z",
      persistedAssistantMessageId: "assistant-1",
      assistantText: "partial",
      hasVisibleProgress: true,
    };
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          messages: [message("assistant-1", "partial")],
          runtimeState: "assistant_streaming",
          shouldBindStream: true,
          streamCache,
        });
      }
      if (command === "conversation.runtimeSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          runtimeState: "idle",
          streamCache,
        });
      }
      if (command === "conversation.messageById") {
        return Promise.resolve(message("assistant-1", "final"));
      }
      return Promise.resolve({});
    });
    flowMock.readConversationStreamCache.mockReturnValue(streamCache);

    const { runtime, scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(flowMock.bindActiveConversationStream).toHaveBeenCalledTimes(1));
    flowMock.frontendRoundPhase.value = "streaming";
    windowTarget.dispatchEvent(new Event("focus"));

    await vi.waitFor(() => {
      expect((runtime.allMessages.value[0]?.parts?.[0] as any)?.text).toBe("final");
    });
    expect(invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot")).toHaveLength(1);
    expect(runtime.runtimeState.value).toBe("idle");
    scope.stop();
  });

  it("焦点对账会在压缩消息之后补上后端新增的正式 assistant 消息", async () => {
    const compaction = message("compaction-a", "上下文已压缩", "assistant");
    const assistantReply = message("assistant-b", "压缩后的正式回复", "assistant");
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          messages: [compaction],
          runtimeState: "idle",
          shouldBindStream: false,
        });
      }
      if (command === "conversation.runtimeSnapshot") {
        return Promise.resolve({ conversationId: "conversation-a", runtimeState: "idle" });
      }
      if (command === "conversation.freshnessSnapshot") {
        return Promise.resolve({ conversationId: "conversation-a", lastMessageId: "assistant-b", updatedAt: "2026-08-07T00:00:01Z" });
      }
      if (command === "conversation.messageById") {
        return Promise.resolve(assistantReply);
      }
      return Promise.resolve({});
    });

    const { runtime, scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value.map((item) => item.id)).toEqual(["compaction-a"]));
    windowTarget.dispatchEvent(new Event("focus"));

    await vi.waitFor(() => expect(runtime.allMessages.value.map((item) => item.id)).toEqual([
      "compaction-a",
      "assistant-b",
    ]));
    expect(invokeTauriMock).toHaveBeenCalledWith("conversation.messageById", {
      input: { conversationId: "conversation-a", messageId: "assistant-b" },
    });
    scope.stop();
  });

  it("会话 freshness 变化时即使尾消息 ID 相同也以正式消息覆盖半截内容", async () => {
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({
          conversationId: "conversation-a",
          messages: [message("assistant-b", "半截")],
          runtimeState: "idle",
          shouldBindStream: false,
        });
      }
      if (command === "conversation.runtimeSnapshot") return Promise.resolve({ runtimeState: "idle" });
      if (command === "conversation.freshnessSnapshot") {
        return Promise.resolve({ lastMessageId: "assistant-b", updatedAt: "2026-08-07T00:00:01Z" });
      }
      if (command === "conversation.messageById") return Promise.resolve(message("assistant-b", "完成态"));
      return Promise.resolve({});
    });

    const { runtime, scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));
    windowTarget.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect((runtime.allMessages.value[0].parts[0] as any).text).toBe("完成态"));
    expect(invokeTauriMock).toHaveBeenCalledWith("conversation.messageById", {
      input: { conversationId: "conversation-a", messageId: "assistant-b" },
    });
    scope.stop();
  });

  it("会话 freshness 未变化时空闲 focus 不读取单条消息", async () => {
    invokeTauriMock.mockImplementation((command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
        return Promise.resolve({ conversationId: "conversation-a", messages: [message("assistant-a", "正文")], runtimeState: "idle" });
      }
      if (command === "conversation.runtimeSnapshot") return Promise.resolve({ runtimeState: "idle" });
      if (command === "conversation.freshnessSnapshot") {
        return Promise.resolve({ lastMessageId: "assistant-a", updatedAt: "2026-08-07T00:00:00Z" });
      }
      if (command === "conversation.messageById") return Promise.resolve(message("assistant-a", "正文"));
      return Promise.resolve({});
    });
    const { scope, windowTarget } = await createRuntime();
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledWith("conversation.foregroundLightSnapshot", expect.anything()));
    windowTarget.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledWith("conversation.freshnessSnapshot", {
      input: { conversationId: "conversation-a", agentId: null },
    }));
    // 首次 focus 建立基线后会按 lastMessageId 对账一次
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledWith("conversation.messageById", {
      input: { conversationId: "conversation-a", messageId: "assistant-a" },
    }));
    invokeTauriMock.mockClear();
    windowTarget.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect(invokeTauriMock).toHaveBeenCalledWith("conversation.freshnessSnapshot", {
      input: { conversationId: "conversation-a", agentId: null },
    }));
    expect(invokeTauriMock.mock.calls.some(([command]) => command === "conversation.messageById")).toBe(false);
    scope.stop();
  });

  it("视图因主会话切换被卸载后，旧快照不得重新绑定僵尸 Channel", async () => {
    let resolveSnapshot: ((value: unknown) => void) | undefined;
    invokeTauriMock.mockImplementation((command: string) => {
      if (command !== "conversation.foregroundLightSnapshot") {
        return Promise.resolve({});
      }
      return new Promise((resolve) => {
        resolveSnapshot = resolve;
      });
    });

    const { scope } = await createRuntime();
    await vi.waitFor(() => expect(resolveSnapshot).toBeTypeOf("function"));
    scope.stop();
    resolveSnapshot?.({
      conversationId: "conversation-a",
      messages: [message("assistant-1", "partial")],
      runtimeState: "assistant_streaming",
      shouldBindStream: true,
      streamCache: { persistedAssistantMessageId: "assistant-1", assistantText: "partial" },
    });
    await Promise.resolve();
    await Promise.resolve();

    expect(flowMock.bindActiveConversationStream).not.toHaveBeenCalled();
  });

  it("追问的所有绑定入口都交给独占订阅槽协调", async () => {
    const subscriptionSlot = {
      acquire: vi.fn(async (lease: any) => lease.bind()),
      release: vi.fn(async () => {}),
    };
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [],
      runtimeState: "idle",
      shouldBindStream: false,
    });

    const { scope } = await createRuntime("conversation-a", subscriptionSlot);
    await vi.waitFor(() => expect(useChatFlowOptionsHolder.value).toBeTruthy());
    const bind = vi.fn(async () => {});
    const unbind = vi.fn(async () => {});
    await useChatFlowOptionsHolder.value.coordinateActiveConversationStreamBind({
      bindingId: "side-view-test",
      conversationId: "conversation-a",
      force: true,
      bind,
      unbind,
    });

    expect(subscriptionSlot.acquire).toHaveBeenCalledWith({
      ownerId: "side-view-test",
      conversationId: "conversation-a",
      bind,
      unbind,
    });
    scope.stop();
    await vi.waitFor(() => expect(subscriptionSlot.release).toHaveBeenCalled());
  });

  it("收到撤回广播且会话匹配时裁剪目标消息之后的消息", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [
        message("user-1", "q1", "user"),
        message("assistant-1", "a1"),
        message("user-2", "q2", "user"),
        message("assistant-2", "a2"),
      ],
      runtimeState: "idle",
      shouldBindStream: false,
    });

    const { runtime, scope } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(4));
    rewindCompletedHandlers.forEach((handler) => handler({
      conversationId: "conversation-a",
      targetMessageId: "user-2",
      remainingLastMessageId: "assistant-1",
      removedCount: 2,
      remainingCount: 2,
    }));
    await nextTick();
    expect(runtime.allMessages.value.map((item: any) => item.id)).toEqual(["user-1", "assistant-1"]);
    scope.stop();
  });

  it("撤回广播的会话不匹配时不裁剪本地消息", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("user-1", "q1", "user"), message("assistant-1", "a1")],
      runtimeState: "idle",
      shouldBindStream: false,
    });

    const { runtime, scope } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(2));
    rewindCompletedHandlers.forEach((handler) => handler({
      conversationId: "conversation-other",
      targetMessageId: "user-1",
      remainingLastMessageId: "",
      removedCount: 1,
      remainingCount: 1,
    }));
    await nextTick();
    expect(runtime.allMessages.value).toHaveLength(2);
    scope.stop();
  });

  it("保留消息 ID 不在本地时按撤回目标消息裁剪", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("user-1", "q1", "user"), message("assistant-1", "a1")],
      runtimeState: "idle",
      shouldBindStream: false,
    });

    const { runtime, scope } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(2));
    rewindCompletedHandlers.forEach((handler) => handler({
      conversationId: "conversation-a",
      targetMessageId: "assistant-1",
      remainingLastMessageId: "missing-id",
      removedCount: 1,
      remainingCount: 1,
    }));
    await nextTick();
    expect(runtime.allMessages.value.map((item: any) => item.id)).toEqual(["user-1"]);
    scope.stop();
  });

  it("撤回广播的两个边界 ID 都不在本地时，重新同步权威快照而非静默保留旧切片", async () => {
    invokeTauriMock
      .mockResolvedValueOnce({
        conversationId: "conversation-a",
        messages: [message("user-2", "q2", "user"), message("assistant-2", "a2")],
        runtimeState: "idle",
        shouldBindStream: false,
      })
      .mockResolvedValueOnce({
        conversationId: "conversation-a",
        messages: [message("user-1", "q1", "user")],
        runtimeState: "idle",
        shouldBindStream: false,
      });

    const { runtime, scope } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(2));
    // 本地只加载了尾部切片 [user-2, assistant-2]，撤回点更早（user-1），两个边界 ID 都不在切片内
    rewindCompletedHandlers.forEach((handler) => handler({
      conversationId: "conversation-a",
      targetMessageId: "user-1",
      remainingLastMessageId: "missing-keep",
      removedCount: 2,
      remainingCount: 1,
    }));
    // 不再静默保留本地旧切片，而是发起权威同步并最终以服务端快照为准
    await vi.waitFor(() => expect(runtime.allMessages.value.map((item: any) => item.id)).toEqual(["user-1"]));
    expect(
      invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot"),
    ).toHaveLength(2);
    scope.stop();
  });

  it("队列失配事件与本会话一致时重新同步权威快照", async () => {
    invokeTauriMock
      .mockResolvedValueOnce({
        conversationId: "conversation-a",
        messages: [message("user-1", "q1", "user")],
        runtimeState: "idle",
        shouldBindStream: false,
      })
      .mockResolvedValueOnce({
        conversationId: "conversation-a",
        messages: [message("user-1", "q1", "user"), message("assistant-1", "a1")],
        runtimeState: "idle",
        shouldBindStream: false,
      });

    const { runtime, scope, windowTarget } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(runtime.allMessages.value).toHaveLength(1));
    windowTarget.dispatchEvent(new CustomEvent("easy-call:chat-queue-out-of-sync", {
      detail: { eventId: "event-1", conversationId: "conversation-a", reason: "not_in_queue" },
    }));
    await vi.waitFor(() => expect(runtime.allMessages.value.map((item: any) => item.id)).toEqual(["user-1", "assistant-1"]));
    expect(
      invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot"),
    ).toHaveLength(2);
    scope.stop();
  });

  it("队列失配事件属于其他会话时不触动本会话", async () => {
    invokeTauriMock.mockResolvedValueOnce({
      conversationId: "conversation-a",
      messages: [message("user-1", "q1", "user")],
      runtimeState: "idle",
      shouldBindStream: false,
    });

    const { scope, windowTarget } = await createRuntime("conversation-a");
    await vi.waitFor(() => expect(
      invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot"),
    ).toHaveLength(1));
    windowTarget.dispatchEvent(new CustomEvent("easy-call:chat-queue-out-of-sync", {
      detail: { eventId: "event-9", conversationId: "conversation-b", reason: "not_in_queue" },
    }));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(
      invokeTauriMock.mock.calls.filter(([command]) => command === "conversation.foregroundLightSnapshot"),
    ).toHaveLength(1);
    scope.stop();
  });
});
