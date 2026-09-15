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
      partialStreamBlocks: [expect.objectContaining({
        reasoning: "先检查配置。",
        tools: [expect.objectContaining({
          toolCallId: "call-1",
          name: "read_file",
          resultText: "配置已读取",
        })],
      })],
    }));
  });
});
