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
      if (command === "get_active_conversation_messages") {
        return oldHistory;
      }
      throw new Error(`unexpected invoke command: ${command}`);
    });

    const runtime = useChatRuntime({
      t: (key) => key,
      setStatus: () => {},
      setStatusError: () => {},
      activeChatApiConfigId: ref("api-1"),
      selectedPersonaId: ref("agent-1"),
      chatting,
      forcingArchive,
      allMessages,
      visibleTurnCount,
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
      visibleTurnCount,
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

    expect(chatting.value).toBe(true);
    expect(latestAssistantText.value).toBe("");

    await runtime.refreshConversationHistory();
    expect(allMessages.value).toEqual(oldHistory);
    expect(latestAssistantText.value).toBe("");

    expect(capturedChannel).not.toBeNull();
    capturedChannel!.emit({ delta: "N" });
    await vi.advanceTimersByTimeAsync(34);
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
});
