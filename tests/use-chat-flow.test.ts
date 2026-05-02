import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ref, shallowRef } from "vue";
import type { ChatMessage } from "../src/types/app";
import { useChatFlow, type AssistantDeltaEvent } from "../src/features/chat/composables/use-chat-flow";
import { useChatRuntime } from "../src/features/chat/composables/use-chat-runtime";

const hoisted = vi.hoisted(() => {
  class MockChannel<T> {
    onmessage?: (event: T) => void;

    emit(event: T) {
      this.onmessage?.(event);
    }
  }

  return {
    MockChannel,
    invokeTauriMock: vi.fn(),
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: hoisted.MockChannel,
}));

vi.mock("../src/services/tauri-api", () => ({
  invokeTauri: hoisted.invokeTauriMock,
}));

function textMessage(id: string, role: "user" | "assistant", text: string): ChatMessage {
  return {
    id,
    role,
    parts: [{ type: "text", text }],
  };
}

async function flushAsyncSteps(times = 4) {
  // history_flushed 处理链路里包含一个 fire-and-forget async IIFE，
  // 内部还会 await onReloadMessages()，因此这里主动多冲几轮微任务，
  // 让测试在断言前稳定等到“刷新历史 -> 切换 chatting”这条链走完。
  for (let idx = 0; idx < times; idx += 1) {
    await Promise.resolve();
  }
}

describe("useChatFlow stream isolation", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    hoisted.invokeTauriMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not hydrate streaming bubble from history before first delta", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);

    const oldHistory: ChatMessage[] = [
      textMessage("u-old", "user", "old question"),
      textMessage("a-old", "assistant", "A_old"),
    ];

    hoisted.invokeTauriMock.mockImplementation(async (command: string) => {
      if (command === "get_foreground_conversation_light_snapshot") {
        return { messages: oldHistory };
      }
      throw new Error(`unexpected invoke command: ${command}`);
    });

    const runtime = useChatRuntime({
      t: (key) => key,
      setStatus: () => {},
      setStatusError: () => {},
      setChatError: () => {},
      activeChatApiConfigId: ref("api-1"),
      assistantDepartmentAgentId: ref("agent-1"),
      chatting,
      forcingArchive,
      compactingConversation: ref(false),
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      perfNow: () => Date.now(),
      perfLog: () => {},
      perfDebug: false,
    });

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };

    let capturedChannel: ChannelLike | null = null;
    let resolveRequest:
      | ((value: {
        assistantText: string;
        latestUserText: string;
        reasoningStandard?: string;
        reasoningInline?: string;
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolveRequest = resolve;
        }),
      onReloadMessages: () => runtime.refreshConversationHistory(),
    });

    const sendPromise = flow.sendChat();
    await Promise.resolve();

    expect(chatting.value).toBe(false);
    expect(latestAssistantText.value).toBe("");

    await runtime.refreshConversationHistory();
    expect(allMessages.value).toEqual(oldHistory);
    expect(latestAssistantText.value).toBe("");

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("waiting");
    expect(visibleTurnCount.value).toBe(1);

    capturedChannel!.emit({ delta: "N" });
    await vi.advanceTimersByTimeAsync(34);
    expect(chatting.value).toBe(true);
    expect(latestAssistantText.value).toBe("N");

    expect(resolveRequest).not.toBeNull();
    resolveRequest!({
      assistantText: "A_new",
      latestUserText: "new question",
      reasoningStandard: "",
      reasoningInline: "",
      archivedBeforeSend: false,
    });

    await sendPromise;

    expect(latestAssistantText.value).toBe("A_new");
    expect(chatErrorText.value).toBe("");
    expect(chatting.value).toBe(false);
  });

  it("shows retry status in the pre-streaming assistant draft", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };

    let capturedChannel: ChannelLike | null = null;
    let resolveRequest:
      | ((value: {
        assistantText: string;
        latestUserText: string;
        reasoningStandard?: string;
        reasoningInline?: string;
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolveRequest = resolve;
        }),
      onReloadMessages: async () => {},
    });

    const sendPromise = flow.sendChat();
    await Promise.resolve();

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();

    capturedChannel!.emit({
      kind: "tool_status",
      toolStatus: "running",
      message: "模型请求失败 code 500，正在重试 (1/5)，等待 1 秒...",
    });

    const assistantDraft = allMessages.value.find((message) => String(message.id || "").startsWith("__draft_assistant__:"));
    expect(assistantDraft?.providerMeta?._preStreamingStatusText).toBe("模型请求失败 code 500，正在重试 (1/5)，等待 1 秒...");
    expect(toolStatusText.value).toBe("模型请求失败 code 500，正在重试 (1/5)，等待 1 秒...");

    capturedChannel!.emit({ delta: "N" });
    await vi.advanceTimersByTimeAsync(34);

    const streamingDraft = allMessages.value.find((message) => String(message.id || "").startsWith("__draft_assistant__:"));
    expect(streamingDraft?.providerMeta?._preStreamingStatusText).toBe("");

    expect(resolveRequest).not.toBeNull();
    resolveRequest!({
      assistantText: "A_new",
      latestUserText: "new question",
      reasoningStandard: "",
      reasoningInline: "",
      archivedBeforeSend: false,
    });

    await sendPromise;
  });

  it("clears retry waiting draft after stop succeeds", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});
    const invokeStopChatMessage = vi.fn(async () => ({ aborted: true, persisted: false }));

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };

    let capturedChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise(() => {
          capturedChannel = onDelta as unknown as ChannelLike;
        }),
      invokeStopChatMessage,
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    capturedChannel!.emit({
      kind: "tool_status",
      toolStatus: "running",
      message: "模型请求失败 code 500，正在重试 (1/5)，等待 1 秒...",
    });

    expect(chatting.value).toBe(true);
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("__draft_assistant__:"))).toBe(true);

    await flow.stopChat();

    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("idle");
    expect(toolStatusText.value).toBe("");
    expect(toolStatusState.value).toBe("");
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("__draft_assistant__:"))).toBe(false);
    expect(onReloadMessages).toHaveBeenCalledTimes(2);
  });

  it("stops stream by preserving partial text and syncing stop payload", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});
    const invokeStopChatMessage = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let capturedChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise(() => {
          capturedChannel = onDelta as unknown as ChannelLike;
        }),
      invokeStopChatMessage,
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();
    expect(chatting.value).toBe(false);

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("waiting");
    expect(visibleTurnCount.value).toBe(1);
    capturedChannel!.emit({ delta: "ABC" });
    capturedChannel!.emit({ kind: "reasoning_inline", delta: "R1" });
    expect(chatting.value).toBe(true);

    await flow.stopChat();

    expect(chatting.value).toBe(false);
    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(invokeStopChatMessage).toHaveBeenCalledWith({
      session: { apiConfigId: "api-1", agentId: "agent-1" },
      partialAssistantText: "ABC",
      partialReasoningStandard: "",
      partialReasoningInline: "R1",
    });
    expect(onReloadMessages).toHaveBeenCalledTimes(2);
  });

  it("keeps current streaming round visible until history_flushed switches to next round", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("first question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };

    const capturedChannels: ChannelLike[] = [];
    const resolveRequests: Array<(value: {
      assistantText: string;
      latestUserText: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      archivedBeforeSend: boolean;
    }) => void> = [];

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannels.push(onDelta as unknown as ChannelLike);
          resolveRequests.push(resolve);
        }),
      onReloadMessages,
    });

    const firstSend = flow.sendChat();
    await Promise.resolve();
    expect(chatting.value).toBe(false);
    expect(capturedChannels).toHaveLength(1);

    capturedChannels[0].emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("waiting");
    expect(visibleTurnCount.value).toBe(1);

    capturedChannels[0].emit({ delta: "FIRST" });
    await vi.advanceTimersByTimeAsync(250);
    expect(chatting.value).toBe(true);
    expect(latestAssistantText.value).toBe("FIRST");

    chatInput.value = "second question";
    const secondSend = flow.sendChat();
    await Promise.resolve();
    expect(capturedChannels).toHaveLength(2);

    // 第二次发送只是在排队，不能抢占当前正在显示的第一轮流式。
    expect(chatting.value).toBe(true);
    expect(latestAssistantText.value).toBe("FIRST");

    capturedChannels[1].emit({ delta: "SECOND-BEFORE-FLUSH" });
    await vi.advanceTimersByTimeAsync(250);
    expect(latestAssistantText.value).toBe("FIRST");

    capturedChannels[1].emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":2,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(onReloadMessages).toHaveBeenCalledTimes(2);
    expect(latestAssistantText.value).toBe("");
    expect(chatting.value).toBe(true);
    expect(flow.frontendRoundPhase.value).toBe("waiting");
    expect(visibleTurnCount.value).toBe(1);

    capturedChannels[1].emit({ delta: "SECOND-AFTER-FLUSH" });
    await vi.advanceTimersByTimeAsync(1200);
    expect(latestAssistantText.value).toBe("SECOND-AFTER-FLUSH");

    resolveRequests[0]({
      assistantText: "FIRST-DONE",
      latestUserText: "first question",
      reasoningStandard: "",
      reasoningInline: "",
      archivedBeforeSend: false,
    });
    await firstSend;

    // 第一轮的收尾结果也不应再污染已经接管前台的第二轮。
    expect(latestAssistantText.value).toBe("SECOND-AFTER-FLUSH");
    expect(chatting.value).toBe(true);

    resolveRequests[1]({
      assistantText: "SECOND-DONE",
      latestUserText: "second question",
      reasoningStandard: "",
      reasoningInline: "",
      archivedBeforeSend: false,
    });
    await secondSend;

    expect(latestAssistantText.value).toBe("SECOND-DONE");
    expect(chatting.value).toBe(false);
  });

  it("does not enter streaming view for non-activated batch without history_flushed", async () => {
    const chatting = ref(false);
    const forcingArchive = ref(false);
    const chatInput = ref("queued-only");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const latestReasoningStandardText = ref("");
    const latestReasoningInlineText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    let resolveRequest:
      | ((value: {
        assistantText: string;
        latestUserText: string;
        reasoningStandard?: string;
        reasoningInline?: string;
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      forcingArchive,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      latestReasoningStandardText,
      latestReasoningInlineText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: () =>
        new Promise((resolve) => {
          resolveRequest = resolve;
        }),
      onReloadMessages,
    });

    const sendPromise = flow.sendChat();
    await Promise.resolve();

    // 仅入队、未收到 history_flushed 时，不应出现新的前台流式轮次。
    expect(chatting.value).toBe(false);
    expect(latestAssistantText.value).toBe("");

    resolveRequest!({
      assistantText: "",
      latestUserText: "queued-only",
      reasoningStandard: "",
      reasoningInline: "",
      archivedBeforeSend: false,
    });
    await sendPromise;

    expect(latestAssistantText.value).toBe("");
    expect(onReloadMessages).toHaveBeenCalledTimes(1);
    expect(chatting.value).toBe(false);
  });
});

describe("useChatRuntime force archive conversation sync", () => {
  beforeEach(() => {
    hoisted.invokeTauriMock.mockReset();
  });

  it("updates current conversation id from force_archive_current before reload messages", async () => {
    const statusList: string[] = [];
    const errorList: string[] = [];
    const currentConversationId = ref("conv-old");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);

    hoisted.invokeTauriMock.mockImplementation(async (command: string, payload?: unknown) => {
      if (command === "force_archive_current") {
        return {
          archived: true,
          archiveId: "archive-1",
          activeConversationId: "conv-new",
          summary: "ok",
          mergedMemories: 2,
        };
      }
      if (command === "get_foreground_conversation_light_snapshot") {
        const input = (payload as { input?: { conversationId?: string | null } } | undefined)?.input;
        return {
          messages: [
            textMessage(
              "a1",
              "assistant",
              `conversation:${String(input?.conversationId || "")}`,
            ),
          ],
        };
      }
      throw new Error(`unexpected invoke command: ${command}`);
    });

    const runtime = useChatRuntime({
      t: (key) => key,
      setStatus: (text) => statusList.push(text),
      setStatusError: (key, error) => errorList.push(`${key}:${String(error)}`),
      setChatError: () => {},
      activeChatApiConfigId: ref("api-1"),
      assistantDepartmentAgentId: ref("agent-1"),
      currentConversationId,
      chatting: ref(false),
      forcingArchive: ref(false),
      compactingConversation: ref(false),
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      perfNow: () => Date.now(),
      perfLog: () => {},
      perfDebug: false,
    });

    await runtime.forceArchiveNow();

    expect(currentConversationId.value).toBe("conv-new");
    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].parts?.[0]).toEqual({
      type: "text",
      text: "conversation:conv-new",
    });
    expect(errorList).toEqual([]);
    expect(statusList.length).toBeGreaterThan(0);
  });
});
