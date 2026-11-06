import { ref, shallowRef } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { ChatMessage } from "../../../types/app";
import { useChatFlowStop } from "./use-chat-flow-stop";

describe("useChatFlowStop", () => {
  it("freezes the current formal assistant message before settling stale streaming projections", async () => {
    const allMessages = shallowRef<ChatMessage[]>([{
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "" }],
      contentBlocks: [{
        reasoning: "先检查配置。",
        tools: [{
          toolCallId: "call-1",
          name: "read_file",
          argsText: "{\"path\":\"app.toml\"}",
          resultText: "配置已读取",
          status: "done",
        }],
      }],
      providerMeta: { _streaming: true },
    }]);
    const finalizeMessage = vi.fn();
    const settleStreamingAssistantMessages = vi.fn(() => []);
    const invokeStopChatMessage = vi.fn(async () => ({
      aborted: true,
      persisted: false,
      assistantText: "",
    }));

    const { stopChat } = useChatFlowStop({
      chatting: ref(true),
      allMessages,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      invokeStopChatMessage,
      getRound: () => ({ phase: "streaming", gen: 1, messageId: "assistant-1" }),
      setRound: vi.fn(),
      advanceGeneration: vi.fn(),
      setSendChatActiveGen: vi.fn(),
      clearDeferredRoundCompletion: vi.fn(),
      clearPendingTerminalEvent: vi.fn(),
      setActiveActivationId: vi.fn(),
      getActiveActivationId: () => "activation-1",
      setActiveRoundAgentId: vi.fn(),
      clearFrontendDispatchTimer: vi.fn(),
      getPendingUserDraftId: () => "",
      removeMessage: vi.fn(),
      settleStreamingAssistantMessages,
      finalizeMessage,
      updateMessageText: vi.fn(),
      deleteSendStartedAtMs: vi.fn(),
      clearConversationStreamCache: vi.fn(),
      reasoningStartedAtMs: ref(0),
      flushStreamTextBuffer: vi.fn(),
    });

    await stopChat();

    expect(finalizeMessage).toHaveBeenCalledWith("assistant-1");
    expect(settleStreamingAssistantMessages).toHaveBeenCalledOnce();
    expect(finalizeMessage.mock.invocationCallOrder[0]).toBeLessThan(
      settleStreamingAssistantMessages.mock.invocationCallOrder[0],
    );
    expect(invokeStopChatMessage).toHaveBeenCalledWith(expect.objectContaining({
      // 停止必须指认正在收流的那一个调度，不能只给会话。
      activationId: "activation-1",
      assistantMessageId: "assistant-1",
    }));
  });

  it("accepts a stop during the submit window and marks the send as stopped", async () => {
    const allMessages = shallowRef<ChatMessage[]>([]);
    const invokeStopChatMessage = vi.fn(async () => ({ aborted: true, persisted: false }));
    const markSendStopped = vi.fn();

    const { stopChat } = useChatFlowStop({
      chatting: ref(false),
      submitPending: ref(true),
      allMessages,
      getSession: () => ({ apiConfigId: "api-1", agentId: "agent-1" }),
      getConversationId: () => "conversation-1",
      invokeStopChatMessage,
      getRound: () => ({ phase: "idle" }),
      setRound: vi.fn(),
      advanceGeneration: vi.fn(),
      setSendChatActiveGen: vi.fn(),
      clearDeferredRoundCompletion: vi.fn(),
      clearPendingTerminalEvent: vi.fn(),
      setActiveActivationId: vi.fn(),
      getActiveActivationId: () => "",
      setActiveRoundAgentId: vi.fn(),
      clearFrontendDispatchTimer: vi.fn(),
      getPendingUserDraftId: () => "",
      removeMessage: vi.fn(),
      settleStreamingAssistantMessages: vi.fn(() => []),
      finalizeMessage: vi.fn(),
      updateMessageText: vi.fn(),
      deleteSendStartedAtMs: vi.fn(),
      clearConversationStreamCache: vi.fn(),
      reasoningStartedAtMs: ref(0),
      flushStreamTextBuffer: vi.fn(),
      getSendChatActiveGen: () => 7,
      markSendStopped,
      getSendRequestId: (gen: number) => (gen === 7 ? "req-7" : ""),
    });

    await stopChat();

    expect(markSendStopped).toHaveBeenCalledWith(7);
    expect(invokeStopChatMessage).toHaveBeenCalledOnce();
    // 提交期没有激活号，只能用手上这次提交的请求号指认；不能只给会话让后端去猜。
    expect(invokeStopChatMessage).toHaveBeenCalledWith(expect.objectContaining({
      requestId: "req-7",
      activationId: undefined,
      assistantMessageId: undefined,
    }));
  });
});
