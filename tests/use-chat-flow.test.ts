import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, ref, shallowRef } from "vue";
import type { Ref } from "vue";
import type { AssistantStreamBlock, ChatMessage } from "../src/types/app";
import { useChatFlow, type AssistantDeltaEvent } from "../src/features/chat/composables/use-chat-flow";
import { useChatFlowDrafts } from "../src/features/chat/composables/use-chat-flow-drafts";
import { useChatFlowStop } from "../src/features/chat/composables/use-chat-flow-stop";
import { useChatRuntime } from "../src/features/chat/composables/use-chat-runtime";
import { useChatMessageBlocks } from "../src/features/chat/composables/use-chat-turns";
import { assistantTextFromStreamBlocks, projectMessageForDisplay } from "../src/utils/chat-message-semantics";

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
  createTransportChannel: <T,>() => new hoisted.MockChannel<T>(),
  invokeTauri: hoisted.invokeTauriMock,
  isTauriRuntimeAvailable: () => false,
  chatStreamNeedsFrontendBind: () => true,
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

function acceptedSendResult(overrides: Partial<{
  accepted: boolean;
  duplicate: boolean;
  eventId: string;
  conversationId: string;
  traceId: string;
  ingress: string;
  userMessageId: string;
  assistantMessageId: string;
}> = {}) {
  return {
    accepted: true,
    duplicate: false,
    eventId: "event-1",
    conversationId: "conversation-1",
    traceId: "trace-1",
    ingress: "accepted",
    userMessageId: "user-1",
    assistantMessageId: "stream-assistant-1",
    ...overrides,
  };
}

function expectedStreamBlock(input: Partial<AssistantStreamBlock>): AssistantStreamBlock {
  const reasoning = String(input.reasoning || "");
  return {
    reasoning,
    reasoningCharCount: reasoning.length,
    text: String(input.text || ""),
    tools: input.tools || [],
    pendingTextBreak: input.pendingTextBreak === true,
  };
}

function streamedTextOf(allMessages: Ref<ChatMessage[]>, messageIdPrefix: string): string {
  const draft = allMessages.value.find((message) => String(message.id || "").startsWith(messageIdPrefix));
  return assistantTextFromStreamBlocks(draft?.contentBlocks || []);
}

describe("useChatFlow stream isolation", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    hoisted.invokeTauriMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function createDraftHarness(initialMessages: ChatMessage[]) {
    const allMessages = shallowRef<ChatMessage[]>(initialMessages);
    const drafts = useChatFlowDrafts({
      allMessages,
      latestUserText: ref(""),
      latestAssistantText: ref(""),
      toolStatusText: ref(""),
      toolStatusState: ref(""),
      buildImageAttachmentPayload: () => [],
      getSendStartedAtMs: () => 0,
      getActiveHistoryMessageCount: () => allMessages.value.length,
      getFrontendDispatchStartedAtMs: () => 0,
      currentFrontendDispatchElapsedMs: () => 0,
    });
    return { allMessages, drafts };
  }

  it("does not replace a flushed user message with a local real user message for the same id", () => {
    const { allMessages, drafts } = createDraftHarness([
      {
        id: "user-1",
        role: "user",
        createdAt: "2026-01-01T00:00:00.000Z",
        speakerAgentId: "user-persona",
        parts: [{ type: "text", text: "flushed user" }],
        providerMeta: {},
      },
    ]);

    drafts.insertUserDraft("user-1", 1, "local user", [], [], [], []);

    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].parts).toEqual([{ type: "text", text: "flushed user" }]);
    expect(allMessages.value[0].providerMeta?._optimistic).toBeUndefined();
    expect(drafts.getPendingUserDraftId()).toBe("");
  });

  it("keeps a local real user message out of pending draft cleanup", () => {
    const { allMessages, drafts } = createDraftHarness([]);

    drafts.insertUserDraft("user-real-1", 1, "real user", [], [], [], []);

    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].id).toBe("user-real-1");
    expect(allMessages.value[0].providerMeta?._optimistic).toBeUndefined();
    expect(drafts.getPendingUserDraftId()).toBe("");
  });

  it("does not project or cache a user message when a busy conversation queues it", async () => {
    const chatting = ref(true);
    const onOwnUserDraftInserted = vi.fn();
    const allMessages = shallowRef<ChatMessage[]>([textMessage("assistant-1", "assistant", "正在回复")]);
    const flow = useChatFlow({
      chatting,
      trimming: ref(false),
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput: ref("排队消息"),
      clipboardImages: ref([]),
      latestUserText: ref(""),
      latestUserImages: ref([]),
      latestAssistantText: ref("正在回复"),
      toolStatusText: ref(""),
      toolStatusState: ref<"running" | "done" | "failed" | "">(""),
      chatErrorText: ref(""),
      allMessages,
      visibleMessageBlockCount: ref(1),
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult({ ingress: "queued" })),
      onOwnUserDraftInserted,
      onReloadMessages: vi.fn(async () => {}),
    });

    await flow.sendChat();

    expect(allMessages.value).toEqual([textMessage("assistant-1", "assistant", "正在回复")]);
    expect(onOwnUserDraftInserted).not.toHaveBeenCalled();
  });

  it("does not downgrade an active assistant stream to a queued waiting bubble for the same id", () => {
    const { allMessages, drafts } = createDraftHarness([
      {
        id: "assistant-1",
        role: "assistant",
        createdAt: "2026-01-01T00:00:00.000Z",
        speakerAgentId: "agent-1",
        parts: [{ type: "text", text: "streaming text" }],
        providerMeta: {
          _streaming: true,
          _streamSegments: ["streaming text"],
        },
      },
    ]);

    drafts.updateQueuedAssistantMessageStatus("assistant-1", "chat.statusPreparingMessage");

    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].parts).toEqual([{ type: "text", text: "streaming text" }]);
    expect(allMessages.value[0].providerMeta?._preStreamingStatusText).toBeUndefined();
  });

  it("cleans the old queued projection immediately when compaction restarts dispatch", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };

    let capturedChannel: ChannelLike | null = null;
    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolve(acceptedSendResult());
        }),
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    capturedChannel!.emit({
      kind: "round_completed",
      reason: "context_compaction_boundary",
      message: "{\"conversationId\":\"conversation-1\",\"assistantText\":\"\",\"archivedBeforeSend\":false}",
    });
    await flushAsyncSteps();

    expect(flow.frontendRoundPhase.value).toBe("idle");
    expect(chatting.value).toBe(false);
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("stream-assistant-"))).toBe(false);
    expect(onReloadMessages).toHaveBeenCalledTimes(0);
  });

  it("does not hydrate streaming bubble from history before first delta", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);

    const oldHistory: ChatMessage[] = [
      textMessage("u-old", "user", "old question"),
      textMessage("a-old", "assistant", "A_old"),
    ];

    hoisted.invokeTauriMock.mockImplementation(async (command: string) => {
      if (command === "conversation.foregroundLightSnapshot") {
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
      assistantAgentId: ref("agent-1"),
      chatting,
      trimming,
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
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolveRequest = (() => {}) as typeof resolveRequest;
          resolve(acceptedSendResult());
        }),
      onReloadMessages: () => runtime.refreshConversationHistory(),
    });

    const sendPromise = flow.sendChat();
    await Promise.resolve();

    expect(chatting.value).toBe(false);

    await runtime.refreshConversationHistory();
    expect(allMessages.value).toEqual(oldHistory);

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(chatting.value).toBe(true);
    expect(visibleTurnCount.value).toBe(1);

    capturedChannel!.emit({ delta: "N" });
    await vi.advanceTimersByTimeAsync(110);
    expect(chatting.value).toBe(true);
    const streamingDraft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(streamingDraft?.contentBlocks).toMatchObject([{ reasoning: "", text: "N" }]);

    expect(resolveRequest).not.toBeNull();
    resolveRequest!({
      assistantText: "A_new",
      latestUserText: "new question",
      archivedBeforeSend: false,
    });

    await sendPromise;

    const finalDraft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(finalDraft?.contentBlocks).toMatchObject([{ reasoning: "", text: "N" }]);
    expect(chatErrorText.value).toBe("");
    expect(chatting.value).toBe(true);
  });

  it("shows retry status in the pre-streaming assistant draft", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
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
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
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
          resolveRequest = (() => {}) as typeof resolveRequest;
          resolve(acceptedSendResult());
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

    const assistantDraft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(assistantDraft?.providerMeta?._toolStatusText).toBe("模型请求失败 code 500，正在重试 (1/5)，等待 1 秒...");
    expect(assistantDraft?.providerMeta?._toolStatusState).toBe("running");

    capturedChannel!.emit({ delta: "N" });
    await vi.advanceTimersByTimeAsync(110);

    const streamingDraft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(streamingDraft?.providerMeta?._preStreamingStatusText).toBe("");

    expect(resolveRequest).not.toBeNull();
    resolveRequest!({
      assistantText: "A_new",
      latestUserText: "new question",
      archivedBeforeSend: false,
    });

    await sendPromise;
  });

  it("removes retry waiting draft immediately after stop succeeds", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
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
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
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
          resolve(acceptedSendResult());
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
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("stream-assistant-"))).toBe(true);
    allMessages.value = [...allMessages.value, {
      id: "stale-assistant",
      role: "assistant",
      createdAt: "2026-01-01T00:00:00.000Z",
      speakerAgentId: "agent-1",
      parts: [{ type: "text", text: "已收到的片段" }],
      providerMeta: { _streaming: true },
    }];

    await flow.stopChat();

    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("idle");
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("stream-assistant-"))).toBe(false);
    expect(allMessages.value.find((message) => message.id === "stale-assistant")?.providerMeta?._streaming).toBeUndefined();
    expect(onReloadMessages).toHaveBeenCalledTimes(0);

    await flow.handleExternalRoundStarted({
      conversationId: "conversation-1",
      assistantMessageId: "stream-assistant-1",
      activationId: "late-activation",
    });
    capturedChannel!.emit({
      kind: "history_flushed",
      message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}",
    });
    await flushAsyncSteps();
    await flow.handleExternalRoundCompleted({ conversationId: "conversation-1", status: "completed" });

    expect(chatting.value).toBe(false);
    expect(flow.frontendRoundPhase.value).toBe("idle");
    expect(allMessages.value.some((message) => String(message.id || "").startsWith("stream-assistant-"))).toBe(false);
    expect(onReloadMessages).toHaveBeenCalledTimes(1);
  });

  it("keeps the local real user message after stopping a queued first-turn dispatch", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("first question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const currentConversationId = ref("");
    const cachedMessagesByConversation: Record<string, ChatMessage[]> = {};
    const invokeStopChatMessage = vi.fn(async () => ({ aborted: true, persisted: false }));

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => currentConversationId.value,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => ({
        accepted: true,
        duplicate: false,
        eventId: "event-1",
        conversationId: "conversation-1",
        traceId: "trace-1",
        ingress: "accepted",
        userMessageId: "user-1",
        assistantMessageId: "assistant-1",
      })),
      invokeStopChatMessage,
      onOwnUserDraftInserted: ({ conversationId }) => {
        currentConversationId.value = conversationId;
        cachedMessagesByConversation[conversationId] = [...allMessages.value];
      },
      onReloadMessages: async () => {
        allMessages.value = [...(cachedMessagesByConversation[currentConversationId.value] || [])];
      },
    });

    await flow.sendChat();

    expect(currentConversationId.value).toBe("conversation-1");
    expect(allMessages.value.map((message) => String(message.id || ""))).toEqual(["user-1", "assistant-1"]);
    expect(flow.frontendRoundPhase.value).toBe("queued");

    await flow.stopChat();

    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].id).toBe("user-1");
    expect(allMessages.value[0].parts).toEqual([{ type: "text", text: "first question" }]);
    expect(flow.frontendRoundPhase.value).toBe("idle");
  });

  it("freezes the visible stream immediately when stop returns no formal message", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
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
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolve(acceptedSendResult());
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
    expect(chatting.value).toBe(true);
    expect(visibleTurnCount.value).toBe(1);
    capturedChannel!.emit({
      delta: "ABC",
      streamCache: {
        assistantText: "ABC",
        streamBlocks: [{ text: "ABC" }],
      },
    });
    capturedChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "R1",
      streamCache: {
        assistantText: "ABC",
        streamBlocks: [{ reasoning: "R1", text: "ABC" }],
      },
    });
    expect(chatting.value).toBe(true);

    await flow.stopChat();

    expect(chatting.value).toBe(false);
    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(invokeStopChatMessage).toHaveBeenCalledWith({
      session: { apiConfigId: "api-1", agentId: "agent-1", conversationId: "conversation-1" },
      partialAssistantText: "ABC",
      partialStreamBlocks: [expectedStreamBlock({ reasoning: "R1", text: "ABC" })],
    });
    expect(onReloadMessages).toHaveBeenCalledTimes(0);
    expect(allMessages.value).toHaveLength(2);
    const stoppedAssistant = allMessages.value[1];
    expect(stoppedAssistant.role).toBe("assistant");
    expect(stoppedAssistant.parts).toEqual([{ type: "text", text: "" }]);
    expect(stoppedAssistant.contentBlocks).toEqual([expectedStreamBlock({ reasoning: "R1", text: "ABC" })]);
    expect(stoppedAssistant.providerMeta?._streaming).toBeUndefined();
    expect(projectMessageForDisplay(stoppedAssistant).activityItems.map((item) => item.kind === "tool" ? item.name : item.text)).toEqual([
      "R1",
      "ABC",
    ]);
  });

  it("keeps streamed reasoning on final assistant messages without tools", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let capturedChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolve(acceptedSendResult());
        }),
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();
    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    capturedChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "先判断用户提到的工具指代。",
      streamCache: {
        assistantText: "",
        streamBlocks: [{ reasoning: "先判断用户提到的工具指代。" }],
      },
    });
    capturedChannel!.emit({
      delta: "不太确定，展开说说？",
      streamCache: {
        assistantText: "不太确定，展开说说？",
        streamBlocks: [{ reasoning: "先判断用户提到的工具指代。", text: "不太确定，展开说说？" }],
      },
    });
    capturedChannel!.emit({
      kind: "round_completed",
      message: JSON.stringify({
        conversationId: "conversation-1",
        assistantText: "不太确定，展开说说？",
        archivedBeforeSend: false,
        assistantMessage: {
          ...textMessage("stream-assistant-1", "assistant", "不太确定，展开说说？"),
          parts: [{
            type: "text",
            text: "不太确定，展开说说？",
            reasoningContent: "先判断用户提到的工具指代。",
          }],
        },
      }),
    });
    await flushAsyncSteps();

    const finalMessage = allMessages.value.find((message) => message.role === "assistant");
    const projection = projectMessageForDisplay(finalMessage as ChatMessage);
    expect(projection.activityItems).toHaveLength(2);
    expect(projection.activityItems[0]).toMatchObject({
      kind: "reasoning",
      text: "先判断用户提到的工具指代。",
    });
    expect(projection.activityItems[1]).toMatchObject({
      kind: "content",
      text: "不太确定，展开说说？",
    });
    expect(chatting.value).toBe(false);
  });

  it("does not synthesize reasoning or tools from empty stream block snapshots", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let capturedChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolve(acceptedSendResult());
        }),
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();
    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();

    capturedChannel!.emit({
      kind: "assistant_tool_event",
      message: JSON.stringify({
        role: "assistant",
        content: null,
        reasoning_content: "打算用 operate 工具等待 3 秒。",
        tool_calls: [{
          id: "tool-1",
          call_id: "tool-1",
          type: "function",
          function: {
            name: "operate",
            arguments: "{\"action\":\"wait\",\"seconds\":3}",
          },
        }],
      }),
      streamCache: {
        assistantText: "",
        streamBlocks: [],
      },
    });
    capturedChannel!.emit({
      kind: "tool_status",
      toolName: "operate",
      toolCallId: "tool-1",
      toolStatus: "running",
      toolArgs: "{\"action\":\"wait\",\"seconds\":3}",
      message: "正在执行 operate",
      streamCache: {
        assistantText: "",
        streamBlocks: [],
      },
    });

    const draft = allMessages.value.find((message) => message.role === "assistant" && message.id.startsWith("stream-assistant-"));
    expect(draft?.providerMeta?._toolStatusText).toBe("正在执行 operate");
    expect(draft?.providerMeta?._toolStatusState).toBe("running");
    expect(draft?.contentBlocks).toEqual([]);
    const projection = projectMessageForDisplay(draft as ChatMessage);
    expect(projection.activityItems).toEqual([]);

    const { visibleMessageBlocks } = useChatMessageBlocks({
      allMessages,
      activeChatApiConfig: computed(() => null),
      perfDebug: false,
      perfNow: () => 0,
    });
    const draftBlock = visibleMessageBlocks.value.find((block) => String(block.id || "").startsWith("stream-assistant-"));
    expect(draftBlock?.activityItems).toEqual([]);
    expect(draftBlock?.activityStatus).toBe("requesting");
  });

  it("shows external reasoning deltas on the active streaming draft", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let capturedChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) =>
        new Promise((resolve) => {
          capturedChannel = onDelta as unknown as ChannelLike;
          resolve(acceptedSendResult());
        }),
      onReloadMessages,
    });

    void flow.sendChat();
    await Promise.resolve();
    capturedChannel!.emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    capturedChannel!.emit({
      delta: "A",
      streamCache: {
        assistantText: "A",
        streamBlocks: [{ text: "A" }],
      },
    });
    await flushAsyncSteps();

    await flow.handleExternalAssistantDelta({
      conversationId: "conversation-1",
      event: {
        kind: "activity_reasoning_delta",
        delta: "外部流式思考",
        streamCache: {
          assistantText: "A",
          streamBlocks: [{ reasoning: "外部流式思考", text: "A" }],
        },
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({ reasoning: "", text: "A" })]);
  });

  it("keeps current streaming round visible until history_flushed switches to next round", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("first question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
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
      archivedBeforeSend: boolean;
    }) => void> = [];

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
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
          resolveRequests.push((() => {}) as (typeof resolveRequests)[number]);
          resolve(acceptedSendResult({
            userMessageId: `user-${capturedChannels.length}`,
            assistantMessageId: `stream-assistant-${capturedChannels.length}`,
          }));
        }),
      onReloadMessages,
    });

    const firstSend = flow.sendChat();
    await Promise.resolve();
    expect(chatting.value).toBe(false);
    expect(capturedChannels).toHaveLength(1);

    capturedChannels[0].emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":1,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(chatting.value).toBe(true);
    expect(visibleTurnCount.value).toBe(1);

    capturedChannels[0].emit({ delta: "FIRST" });
    await vi.advanceTimersByTimeAsync(250);
    expect(chatting.value).toBe(true);
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");

    chatInput.value = "second question";
    const secondSend = flow.sendChat();
    await Promise.resolve();
    expect(capturedChannels).toHaveLength(2);

    // 第二次发送只是在排队，不能抢占当前正在显示的第一轮流式。
    expect(chatting.value).toBe(true);
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");

    capturedChannels[1].emit({ delta: "SECOND-BEFORE-FLUSH" });
    await vi.advanceTimersByTimeAsync(250);
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");

    capturedChannels[1].emit({ kind: "history_flushed", message: "{\"conversationId\":\"conversation-1\",\"messageCount\":2,\"activateAssistant\":true}" });
    await flushAsyncSteps();
    expect(onReloadMessages).toHaveBeenCalledTimes(0);
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");
    expect(chatting.value).toBe(true);
    expect(flow.frontendRoundPhase.value).toBe("streaming");
    expect(visibleTurnCount.value).toBe(1);

    capturedChannels[1].emit({ delta: "SECOND-AFTER-FLUSH" });
    await vi.advanceTimersByTimeAsync(1200);
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");

    resolveRequests[0]({
      assistantText: "FIRST-DONE",
      latestUserText: "first question",
      archivedBeforeSend: false,
    });
    await firstSend;

    // 当前流式轮次保持可见，排队中的后续轮次不能覆盖当前显示。
    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");
    expect(chatting.value).toBe(true);

    resolveRequests[1]({
      assistantText: "SECOND-DONE",
      latestUserText: "second question",
      archivedBeforeSend: false,
    });
    await secondSend;

    expect(streamedTextOf(allMessages, "stream-assistant-1")).toBe("FIRST");
    expect(chatting.value).toBe(true);
  });

  it("does not enter streaming view for non-activated batch without history_flushed", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("queued-only");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
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
        archivedBeforeSend: boolean;
      }) => void)
      | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
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
          resolveRequest = (() => {}) as typeof resolveRequest;
          resolve(acceptedSendResult());
        }),
      onReloadMessages,
    });

    const sendPromise = flow.sendChat();
    await Promise.resolve();

    // 仅入队、未收到 history_flushed 时，不应出现新的前台流式轮次。
    expect(chatting.value).toBe(false);

    resolveRequest!({
      assistantText: "",
      latestUserText: "queued-only",
      archivedBeforeSend: false,
    });
    await sendPromise;

    expect(onReloadMessages).toHaveBeenCalledTimes(0);
    expect(chatting.value).toBe(false);
  });

  it("projects bound channel stream blocks while the send round is still queued", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    expect(flow.frontendRoundPhase.value).toBe("queued");
    expect(boundChannel).not.toBeNull();

    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "正在分析流式块。",
      streamCache: {
        assistantText: "你好",
        streamBlocks: [{ reasoning: "正在分析流式块。", text: "你好" }],
      },
    });
    await flushAsyncSteps();

    expect(flow.frontendRoundPhase.value).toBe("streaming");
    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({
      reasoning: "正在分析流式块。",
      text: "你好",
    })]);
    expect(projectMessageForDisplay(draft as ChatMessage).activityItems.map((item) => item.text)).toEqual([
      "正在分析流式块。",
      "你好",
    ]);
  });

  it("projects active stream snapshots even when runtime activation ids differ", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      activationId: "backend-activation",
      requestId: "backend-activation",
      delta: "正在分析不同 activation 的当前通道事件。",
      streamCache: {
        activationId: "backend-activation",
        requestId: "backend-activation",
        assistantText: "",
        streamBlocks: [{ reasoning: "正在分析不同 activation 的当前通道事件。" }],
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({
      reasoning: "正在分析不同 activation 的当前通道事件。",
      text: "",
    })]);
    expect(projectMessageForDisplay(draft as ChatMessage).activityItems.map((item) => item.text)).toEqual([
      "正在分析不同 activation 的当前通道事件。",
    ]);
  });

  it("projects stream snapshots into the draft without relying on the streamBlocks ref", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "直接从快照写入草稿。",
      streamCache: {
        assistantText: "",
        streamBlocks: [{ reasoning: "直接从快照写入草稿。" }],
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({
      reasoning: "直接从快照写入草稿。",
      text: "",
    })]);
    expect(projectMessageForDisplay(draft as ChatMessage).activityItems.map((item) => item.text)).toEqual([
      "直接从快照写入草稿。",
    ]);
  });

  it("corrects stale tool activity from a later stream snapshot after a missed result event", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    boundChannel!.emit({
      kind: "assistant_tool_event",
      message: JSON.stringify({
        role: "assistant",
        content: null,
        reasoning_content: "先等一下。",
        tool_calls: [{
          id: "tool-1",
          call_id: "tool-1",
          type: "function",
          function: {
            name: "operate",
            arguments: "{\"action\":\"wait\"}",
          },
        }],
      }),
      streamCache: {
        assistantText: "",
        streamBlocks: [{
          reasoning: "先等一下。",
          tools: [{
            toolCallId: "tool-1",
            name: "operate",
            argsText: "{\"action\":\"wait\"}",
            status: "doing",
          }],
        }],
      },
    });
    await flushAsyncSteps();

    boundChannel!.emit({
      delta: "等待完成，现在汇报。",
      streamCache: {
        assistantText: "等待完成，现在汇报。",
        streamBlocks: [{
          reasoning: "先等一下。",
          text: "等待完成，现在汇报。",
          tools: [{
            toolCallId: "tool-1",
            name: "operate",
            argsText: "{\"action\":\"wait\"}",
            resultText: "等待完成",
            status: "done",
          }],
        }],
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.parts).toEqual([{ type: "text", text: "" }]);
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({
      reasoning: "先等一下。",
      text: "等待完成，现在汇报。",
      tools: [{
        toolCallId: "tool-1",
        name: "operate",
        argsText: "{\"action\":\"wait\"}",
        resultText: "等待完成",
        status: "done",
      }],
    })]);

    const projection = projectMessageForDisplay(draft as ChatMessage);
    expect(projection.activityItems).toMatchObject([
      { kind: "reasoning", text: "先等一下。" },
      { kind: "content", text: "等待完成，现在汇报。 [toolcall:tool-1]" },
      { kind: "tool", name: "operate", status: "done", resultText: "等待完成" },
    ]);

    const { visibleMessageBlocks } = useChatMessageBlocks({
      allMessages,
      activeChatApiConfig: computed(() => null),
      perfDebug: false,
      perfNow: () => 0,
    });
    const draftBlock = visibleMessageBlocks.value.find((block) => String(block.id || "").startsWith("stream-assistant-"));
    expect(draftBlock?.toolCalls).toEqual([{
      toolCallId: "tool-1",
      name: "operate",
      argsText: "{\"action\":\"wait\"}",
      status: "done",
    }]);
    expect(draftBlock?.activityItems).toMatchObject([
      { kind: "reasoning", text: "" },
      { kind: "content", text: "" },
      { kind: "tool", name: "operate", status: "done" },
    ]);
  });

  it("keeps prior reasoning when a tool snapshot and later reasoning arrive", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "思维链1",
      streamCache: {
        assistantText: "",
        streamBlocks: [{ reasoning: "思维链1" }],
      },
    });
    await flushAsyncSteps();
    boundChannel!.emit({
      kind: "assistant_tool_event",
      message: JSON.stringify({
        role: "assistant",
        content: null,
        tool_calls: [{
          id: "tool-1",
          type: "function",
          function: {
            name: "operate",
            arguments: "{\"action\":\"wait\"}",
          },
        }],
      }),
      streamCache: {
        assistantText: "",
        streamBlocks: [{
          reasoning: "思维链1",
          tools: [{
            toolCallId: "tool-1",
            name: "operate",
            argsText: "{\"action\":\"wait\"}",
            status: "doing",
          }],
        }],
      },
    });
    await flushAsyncSteps();
    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "思维链2",
      streamCache: {
        assistantText: "",
        streamBlocks: [{
          reasoning: "思维链1",
          tools: [{
            toolCallId: "tool-1",
            name: "operate",
            argsText: "{\"action\":\"wait\"}",
            status: "doing",
          }],
        }, { reasoning: "思维链2" }],
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    const blocks = draft?.contentBlocks;
    expect(blocks).toEqual([expectedStreamBlock({
      reasoning: "思维链1",
      text: "",
      tools: [{
        toolCallId: "tool-1",
        name: "operate",
        argsText: "{\"action\":\"wait\"}",
        resultText: undefined,
        status: "doing",
      }],
    }), expectedStreamBlock({
      reasoning: "思维链2",
      text: "",
    })]);
    const projection = projectMessageForDisplay(draft as ChatMessage);
    expect(projection.activityItems.map((item) => item.kind === "tool" ? item.name : item.text)).toEqual([
      "思维链1",
      "operate",
      "思维链2",
    ]);
  });

  it("does not let duplicated app tool events clear a bound-channel draft", async () => {
    const chatting = ref(false);
    const trimming = ref(false);
    const chatInput = ref("new question");
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestUserText = ref("");
    const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
    const latestAssistantText = ref("");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([]);
    const chatErrorText = ref("");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);
    const onReloadMessages = vi.fn(async () => {});

    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let boundChannel: ChannelLike | null = null;

    const flow = useChatFlow({
      chatting,
      trimming,
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput,
      clipboardImages,
      latestUserText,
      latestUserImages,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      chatErrorText,
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      invokeBindActiveChatViewStream: vi.fn(async ({ onDelta }) => {
        boundChannel = onDelta as unknown as ChannelLike;
      }),
      onReloadMessages,
    });

    await flow.bindActiveConversationStream("conversation-1");
    await flow.sendChat();
    boundChannel!.emit({
      kind: "activity_reasoning_delta",
      delta: "思维链1",
      streamCache: {
        assistantText: "",
        streamBlocks: [{ reasoning: "思维链1" }],
      },
    });
    await flushAsyncSteps();

    await flow.handleExternalAssistantDelta({
      conversationId: "conversation-1",
      event: {
        kind: "tool_status",
        toolName: "operate",
        toolCallId: "tool-1",
        toolStatus: "running",
        toolArgs: "{\"action\":\"wait\"}",
        message: "正在执行 operate",
        streamCache: {
          assistantText: "",
          streamBlocks: [],
        },
      },
    });
    await flushAsyncSteps();

    const draft = allMessages.value.find((message) => String(message.id || "").startsWith("stream-assistant-"));
    expect(draft?.providerMeta?._toolStatusText).toBe("正在执行 operate");
    expect(draft?.providerMeta?._toolStatusState).toBe("running");
    expect(draft?.contentBlocks).toEqual([expectedStreamBlock({
      reasoning: "思维链1",
      text: "",
    })]);
    expect(projectMessageForDisplay(draft as ChatMessage).activityItems.map((item) => item.text)).toEqual([
      "思维链1",
    ]);
  });

  it("accepts context usage broadcasts after the foreground round is idle", async () => {
    const contextUsagePreview = ref(null);
    const flow = useChatFlow({
      chatting: ref(false),
      trimming: ref(false),
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput: ref(""),
      clipboardImages: ref([]),
      latestUserText: ref(""),
      latestUserImages: ref([]),
      latestAssistantText: ref(""),
      toolStatusText: ref(""),
      toolStatusState: ref(""),
      contextUsagePreview,
      chatErrorText: ref(""),
      allMessages: shallowRef<ChatMessage[]>([]),
      visibleMessageBlockCount: ref(0),
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: vi.fn(async () => acceptedSendResult()),
      onReloadMessages: vi.fn(async () => {}),
    });

    await flow.handleExternalAssistantDelta({
      conversationId: "conversation-1",
      event: {
        kind: "context_usage_update",
        message: JSON.stringify({
          conversationId: "conversation-1",
          contextUsageRatio: 0.42,
          contextUsagePercent: 42,
          effectivePromptTokens: 420,
          contextWindowTokens: 1000,
        }),
      },
    });

    expect(contextUsagePreview.value).toMatchObject({
      conversationId: "conversation-1",
      contextUsagePercent: 42,
      effectivePromptTokens: 420,
    });
  });

  it("keeps the latest context usage preview after the round completes", async () => {
    const contextUsagePreview = ref(null);
    type ChannelLike = {
      emit: (event: AssistantDeltaEvent) => void;
    };
    let capturedChannel: ChannelLike | null = null;
    const flow = useChatFlow({
      chatting: ref(false),
      trimming: ref(false),
      getConversationId: () => "conversation-1",
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      chatInput: ref("new question"),
      clipboardImages: ref([]),
      latestUserText: ref(""),
      latestUserImages: ref([]),
      latestAssistantText: ref(""),
      toolStatusText: ref(""),
      toolStatusState: ref(""),
      contextUsagePreview,
      chatErrorText: ref(""),
      allMessages: shallowRef<ChatMessage[]>([]),
      visibleMessageBlockCount: ref(0),
      t: (key) => key,
      formatRequestFailed: (error) => String(error),
      removeBinaryPlaceholders: (text) => text,
      invokeSendChatMessage: ({ onDelta }) => {
        capturedChannel = onDelta as unknown as ChannelLike;
        return Promise.resolve(acceptedSendResult());
      },
      onReloadMessages: vi.fn(async () => {}),
    });

    void flow.sendChat();
    await Promise.resolve();
    capturedChannel!.emit({
      kind: "history_flushed",
      message: JSON.stringify({
        conversationId: "conversation-1",
        messageCount: 1,
        activateAssistant: true,
      }),
    });
    await flushAsyncSteps();
    capturedChannel!.emit({
      kind: "context_usage_update",
      message: JSON.stringify({
        conversationId: "conversation-1",
        contextUsageRatio: 0.42,
        contextUsagePercent: 42,
        effectivePromptTokens: 420,
        contextWindowTokens: 1000,
      }),
    });
    capturedChannel!.emit({
      kind: "round_completed",
      message: JSON.stringify({
        conversationId: "conversation-1",
        assistantText: "answer",
        assistantMessage: textMessage("assistant-1", "assistant", "answer"),
      }),
    });
    await flushAsyncSteps();

    expect(contextUsagePreview.value).toMatchObject({
      conversationId: "conversation-1",
      contextUsagePercent: 42,
    });
  });
});

describe("useChatRuntime force archive conversation sync", () => {
  beforeEach(() => {
    hoisted.invokeTauriMock.mockReset();
  });

  it("updates current conversation id from trim_current_conversation before reload messages", async () => {
    const statusList: string[] = [];
    const errorList: string[] = [];
    const currentConversationId = ref("conv-old");
    const allMessages = shallowRef<ChatMessage[]>([]);
    const visibleTurnCount = ref(1);

    hoisted.invokeTauriMock.mockImplementation(async (command: string, payload?: unknown) => {
      if (command === "conversation.archive") {
        return {
          success: true,
        };
      }
      if (command === "conversation.foregroundLightSnapshot") {
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
      assistantAgentId: ref("agent-1"),
      currentConversationId,
      chatting: ref(false),
      trimming: ref(false),
      compactingConversation: ref(false),
      allMessages,
      visibleMessageBlockCount: visibleTurnCount,
      perfNow: () => Date.now(),
      perfLog: () => {},
      perfDebug: false,
    });

    await runtime.trimNow();

    expect(currentConversationId.value).toBe("conv-old");
    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].parts?.[0]).toEqual({
      type: "text",
      text: "conversation:conv-old",
    });
    expect(errorList).toEqual([]);
    expect(statusList.length).toBeGreaterThan(0);
  });
});

describe("useChatFlowStop", () => {
  it("freezes the visible frontend stream when stop returns no formal message", async () => {
    const chatting = ref(true);
    const latestAssistantText = ref("ABC");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    // 停止只能读取正式消息本身；不得依赖已废弃的 streamBlocks 镜像状态。
    const renderedStreamBlocks = [expectedStreamBlock({
      reasoning: "R1",
      text: "ABC",
      tools: [{
        toolCallId: "tool-1",
        name: "operate",
        argsText: "{\"action\":\"wait\"}",
        status: "running",
      }],
    })];
    const reasoningStartedAtMs = ref(123);
    const allMessages = shallowRef<ChatMessage[]>([{
      id: "assistant-1",
      role: "assistant",
      createdAt: "2026-01-01T00:00:00.000Z",
      speakerAgentId: "agent-1",
      parts: [{ type: "text", text: "" }],
      contentBlocks: renderedStreamBlocks,
      providerMeta: {
        _streaming: true,
      },
    }]);
    let round: { phase: "streaming"; gen: number; messageId: string } | { phase: "idle" } = {
      phase: "streaming",
      gen: 1,
      messageId: "assistant-1",
    };
    const invokeStopChatMessage = vi.fn(async () => ({ aborted: true, persisted: false }));

    const stop = useChatFlowStop({
      chatting,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      allMessages,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      invokeStopChatMessage,
      getRound: () => round,
      setRound: (next) => {
        round = next;
      },
      advanceGeneration: () => {},
      setSendChatActiveGen: () => {},
      clearDeferredRoundCompletion: () => {},
      clearPendingTerminalEvent: () => {},
      setActiveActivationId: () => {},
      getActiveActivationId: () => "",
      setActiveRoundAgentId: () => {},
      markStoppedRound: () => {},
      clearFrontendDispatchTimer: () => {},
      getPendingUserDraftId: () => "",
      removeMessage: (messageId) => {
        allMessages.value = allMessages.value.filter((message) => String(message.id || "") !== messageId);
      },
      settleStreamingAssistantMessages: () => {
        allMessages.value = allMessages.value.map((message) => {
          const nextMeta = { ...(message.providerMeta || {}) } as Record<string, unknown>;
          delete nextMeta._streaming;
          return { ...message, providerMeta: nextMeta };
        });
        return ["assistant-1"];
      },
      finalizeMessage: (messageId) => {
        allMessages.value = allMessages.value.map((message) => {
          if (String(message.id || "") !== messageId) return message;
          const nextMeta = { ...(message.providerMeta || {}) } as Record<string, unknown>;
          delete nextMeta._streaming;
          return {
            ...message,
            providerMeta: nextMeta,
          };
        });
      },
      updateMessageText: (messageId, rawBlocks) => {
        allMessages.value = allMessages.value.map((message) => {
          if (String(message.id || "") !== messageId) return message;
          return {
            ...message,
            parts: [{ type: "text", text: assistantTextFromStreamBlocks(rawBlocks || []) }],
            contentBlocks: rawBlocks,
          };
        });
      },
      deleteSendStartedAtMs: () => {},
      clearConversationStreamCache: () => {},
      reasoningStartedAtMs,
      flushStreamTextBuffer: () => {},
    });

    await stop.stopChat();

    expect(invokeStopChatMessage).toHaveBeenCalledTimes(1);
    expect(chatting.value).toBe(false);
    expect(round.phase).toBe("idle");
    expect(allMessages.value).toHaveLength(1);
    expect(allMessages.value[0].contentBlocks).toMatchObject([{
      reasoning: "R1",
      text: "ABC",
      tools: [{ toolCallId: "tool-1", name: "operate" }],
    }]);
    expect(allMessages.value[0].providerMeta?._streaming).toBeUndefined();
  });

  it("merges a same-id formal assistant message returned by stop", async () => {
    const chatting = ref(true);
    const latestAssistantText = ref("ABC");
    const toolStatusText = ref("");
    const toolStatusState = ref<"running" | "done" | "failed" | "">("");
    const streamBlocks = ref<AssistantStreamBlock[]>([{ reasoning: "R1", text: "ABC" }]);
    const reasoningStartedAtMs = ref(123);
    const allMessages = shallowRef<ChatMessage[]>([{
      id: "assistant-1",
      role: "assistant",
      createdAt: "2026-01-01T00:00:00.000Z",
      speakerAgentId: "agent-1",
      parts: [{ type: "text", text: "" }],
      contentBlocks: streamBlocks.value,
      providerMeta: {
        _streaming: true,
      },
    }]);
    let round: { phase: "streaming"; gen: number; messageId: string } | { phase: "idle" } = {
      phase: "streaming",
      gen: 1,
      messageId: "assistant-1",
    };
    const persistedAssistantMessage: ChatMessage = {
      id: "assistant-1",
      role: "assistant",
      createdAt: "2026-01-01T00:00:01.000Z",
      speakerAgentId: "agent-1",
      parts: [{ type: "text", text: "正式文本" }],
      providerMeta: {},
    };
    const invokeStopChatMessage = vi.fn(async () => ({
      aborted: true,
      persisted: true,
      assistantMessage: persistedAssistantMessage,
    }));

    const stop = useChatFlowStop({
      chatting,
      latestAssistantText,
      toolStatusText,
      toolStatusState,
      streamBlocks,
      allMessages,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      invokeStopChatMessage,
      getRound: () => round,
      setRound: (next) => {
        round = next;
      },
      advanceGeneration: () => {},
      setSendChatActiveGen: () => {},
      clearDeferredRoundCompletion: () => {},
      clearPendingTerminalEvent: () => {},
      setActiveActivationId: () => {},
      getActiveActivationId: () => "",
      setActiveRoundAgentId: () => {},
      markStoppedRound: () => {},
      clearFrontendDispatchTimer: () => {},
      getPendingUserDraftId: () => "",
      removeMessage: (messageId) => {
        allMessages.value = allMessages.value.filter((message) => String(message.id || "") !== messageId);
      },
      settleStreamingAssistantMessages: () => {
        allMessages.value = allMessages.value.map((message) => {
          const nextMeta = { ...(message.providerMeta || {}) } as Record<string, unknown>;
          delete nextMeta._streaming;
          return { ...message, providerMeta: nextMeta };
        });
        return ["assistant-1"];
      },
      finalizeMessage: (messageId, finalMessage) => {
        allMessages.value = allMessages.value.map((message) => {
          if (String(message.id || "") !== messageId) return message;
          if (finalMessage) return finalMessage;
          const nextMeta = { ...(message.providerMeta || {}) } as Record<string, unknown>;
          delete nextMeta._streaming;
          return {
            ...message,
            providerMeta: nextMeta,
          };
        });
      },
      updateMessageText: (messageId, rawBlocks) => {
        allMessages.value = allMessages.value.map((message) => {
          if (String(message.id || "") !== messageId) return message;
          return {
            ...message,
            parts: [{ type: "text", text: assistantTextFromStreamBlocks(rawBlocks || []) }],
            contentBlocks: rawBlocks,
          };
        });
      },
      deleteSendStartedAtMs: () => {},
      clearConversationStreamCache: () => {},
      reasoningStartedAtMs,
      flushStreamTextBuffer: () => {},
    });

    await stop.stopChat();

    expect(allMessages.value[0]).toEqual(persistedAssistantMessage);
  });
});
