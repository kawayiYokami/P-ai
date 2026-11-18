import { describe, expect, it, vi } from "vitest";
import { ref, shallowRef } from "vue";
import type { ChatMentionTarget, ChatMessage } from "../src/types/app";
import { useChatRewindActions } from "../src/features/chat/composables/use-chat-rewind-actions";

const hoisted = vi.hoisted(() => ({
  invokeTauriMock: vi.fn(),
}));

vi.mock("../src/services/tauri-api", () => ({
  invokeTauri: hoisted.invokeTauriMock,
}));

function textMessage(id: string, role: ChatMessage["role"], text: string): ChatMessage {
  return {
    id,
    role,
    parts: [{ type: "text", text }],
  };
}

function buildRewindActions(overrides: {
  chatting?: boolean;
  trimming?: boolean;
  compacting?: boolean;
  messages?: ChatMessage[];
  requestRecallMode?: (payload: { turnId: string; targetUserMessageId: string }) => Promise<"message_only" | "with_patch" | "cancel">;
} = {}) {
  const allMessages = shallowRef<ChatMessage[]>(
    overrides.messages ?? [
      textMessage("user-1", "user", "第一句"),
      textMessage("assistant-1", "assistant", "回复"),
      textMessage("user-2", "user", "需要撤回"),
      textMessage("assistant-2", "assistant", "后续回复"),
    ],
  );
  const chatErrorText = ref("");
  const statusErrors: unknown[] = [];
  const actions = useChatRewindActions({
    activeApiConfigId: ref("api-a"),
    activeAgentId: ref("agent-a"),
    currentConversationId: ref("conversation-a"),
    allMessages,
    maybeUpdateConversationOverviewFromLoadedMessages: vi.fn(),
    chatting: ref(Boolean(overrides.chatting)),
    trimming: ref(Boolean(overrides.trimming)),
    compactingConversation: ref(Boolean(overrides.compacting)),
    chatErrorText,
    chatInput: ref(""),
    selectedMentions: ref<ChatMentionTarget[]>([]),
    clipboardImages: ref([]),
    deleteUnarchivedConversationFromArchives: vi.fn(),
    sendChat: vi.fn(),
    setStatusError: (_key, error) => statusErrors.push(error),
    setChatErrorText: (text) => {
      chatErrorText.value = text;
    },
    messageText: (message) => String(message.parts?.[0]?.text || ""),
    extractMessageImages: () => [],
    requestRecallMode: vi.fn(overrides.requestRecallMode ?? (async () => "message_only")),
    refreshForegroundConversationAfterRewind: vi.fn(),
  });
  return { actions, allMessages, chatErrorText, statusErrors };
}

describe("useChatRewindActions", () => {
  it("does not call backend or mutate messages when conversation is busy", async () => {
    hoisted.invokeTauriMock.mockReset();
    const { actions, allMessages, chatErrorText } = buildRewindActions({ chatting: true });
    const before = allMessages.value;

    await actions.handleRecallTurn({ turnId: "assistant-2" });

    expect(hoisted.invokeTauriMock).not.toHaveBeenCalled();
    expect(allMessages.value).toBe(before);
    expect(chatErrorText.value).toContain("当前会话正在运行或整理上下文");
  });

  it("keeps local messages unchanged when backend rejects rewind", async () => {
    hoisted.invokeTauriMock.mockReset();
    hoisted.invokeTauriMock.mockRejectedValueOnce("当前会话正在运行或整理上下文，完成后再撤回。");
    const { actions, allMessages, chatErrorText } = buildRewindActions();
    const before = allMessages.value;

    await actions.handleRecallTurn({ turnId: "assistant-2" });

    expect(hoisted.invokeTauriMock).toHaveBeenCalledWith(
      "conversation.rewind",
      expect.any(Object),
    );
    expect(allMessages.value).toBe(before);
    expect(chatErrorText.value).toContain("当前会话正在运行或整理上下文");
  });

  it("ignores duplicate rewind clicks while confirmation is pending", async () => {
    hoisted.invokeTauriMock.mockReset();
    let resolveMode: ((mode: "message_only") => void) | null = null;
    const requestRecallMode = vi.fn((_payload: { turnId: string; targetUserMessageId: string }) => new Promise<"message_only">((resolve) => {
      resolveMode = resolve;
    }));
    const { actions } = buildRewindActions({ requestRecallMode });

    const first = actions.handleRecallTurn({ turnId: "assistant-2" });
    const second = actions.handleRecallTurn({ turnId: "assistant-2" });
    resolveMode?.("message_only");
    await Promise.all([first, second]);

    expect(requestRecallMode).toHaveBeenCalledTimes(1);
    expect(requestRecallMode).toHaveBeenCalledWith({
      turnId: "assistant-2",
      targetUserMessageId: "assistant-2",
      conversationId: "conversation-a",
    });
    expect(hoisted.invokeTauriMock).toHaveBeenCalledTimes(1);
  });

  it("助理消息撤回：后端收到助理消息 ID，不回填输入框", async () => {
    hoisted.invokeTauriMock.mockReset();
    hoisted.invokeTauriMock.mockResolvedValueOnce({
      removedCount: 2,
      remainingCount: 2,
      recalledUserMessage: null,
    });
    const chatInput = ref("");
    const selectedMentions = ref<ChatMentionTarget[]>([]);
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
    const { actions } = buildRewindActions({});
    // 需要替换 chatInput 等引用
    const realActions = useChatRewindActions({
      activeApiConfigId: ref("api-a"),
      activeAgentId: ref("agent-a"),
      currentConversationId: ref("conversation-a"),
      allMessages: shallowRef<ChatMessage[]>([
        textMessage("user-1", "user", "第一句"),
        textMessage("assistant-1", "assistant", "回复第一句"),
        textMessage("user-2", "user", "需要撤回"),
        textMessage("assistant-2", "assistant", "后续回复"),
      ]),
      maybeUpdateConversationOverviewFromLoadedMessages: vi.fn(),
      chatting: ref(false),
      trimming: ref(false),
      compactingConversation: ref(false),
      chatErrorText: ref(""),
      chatInput,
      selectedMentions,
      clipboardImages,
      deleteUnarchivedConversationFromArchives: vi.fn(),
      sendChat: vi.fn(),
      setStatusError: vi.fn(),
      setChatErrorText: vi.fn(),
      messageText: (message) => String(message.parts?.[0]?.text || ""),
      extractMessageImages: () => [],
      requestRecallMode: vi.fn(async () => "message_only"),
      refreshForegroundConversationAfterRewind: vi.fn(),
      requestCreateConversationBranchFromMessageConfirm: vi.fn(),
      createConversationBranchFromMessage: vi.fn(),
      branchingConversation: ref(false),
    });

    await realActions.handleRecallTurn({ turnId: "assistant-2" });

    // 后端应收到 assistant-2（不是 user-2）
    expect(hoisted.invokeTauriMock).toHaveBeenCalledWith(
      "conversation.rewind",
      expect.objectContaining({
        session: expect.objectContaining({
          conversationId: "conversation-a",
        }),
        messageId: "assistant-2",
        undoApplyPatch: false,
      }),
    );
    // 输入框不应被回填
    expect(chatInput.value).toBe("");
    expect(selectedMentions.value).toEqual([]);
    expect(clipboardImages.value).toEqual([]);
  });

  it("用户消息撤回：后端收到用户消息 ID，回填输入框", async () => {
    hoisted.invokeTauriMock.mockReset();
    hoisted.invokeTauriMock.mockResolvedValueOnce({
      removedCount: 2,
      remainingCount: 2,
      recalledUserMessage: textMessage("user-2", "user", "需要撤回"),
    });
    const chatInput = ref("");
    const selectedMentions = ref<ChatMentionTarget[]>([]);
    const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
    const realActions = useChatRewindActions({
      activeApiConfigId: ref("api-a"),
      activeAgentId: ref("agent-a"),
      currentConversationId: ref("conversation-a"),
      allMessages: shallowRef<ChatMessage[]>([
        textMessage("user-1", "user", "第一句"),
        textMessage("assistant-1", "assistant", "回复第一句"),
        textMessage("user-2", "user", "需要撤回"),
        textMessage("assistant-2", "assistant", "后续回复"),
      ]),
      maybeUpdateConversationOverviewFromLoadedMessages: vi.fn(),
      chatting: ref(false),
      trimming: ref(false),
      compactingConversation: ref(false),
      chatErrorText: ref(""),
      chatInput,
      selectedMentions,
      clipboardImages,
      deleteUnarchivedConversationFromArchives: vi.fn(),
      sendChat: vi.fn(),
      setStatusError: vi.fn(),
      setChatErrorText: vi.fn(),
      messageText: (message) => String(message.parts?.[0]?.text || ""),
      extractMessageImages: () => [],
      requestRecallMode: vi.fn(async () => "message_only"),
      refreshForegroundConversationAfterRewind: vi.fn(),
      requestCreateConversationBranchFromMessageConfirm: vi.fn(),
      createConversationBranchFromMessage: vi.fn(),
      branchingConversation: ref(false),
    });

    await realActions.handleRecallTurn({ turnId: "user-2" });

    // 后端应收到 user-2
    expect(hoisted.invokeTauriMock).toHaveBeenCalledWith(
      "conversation.rewind",
      expect.objectContaining({
        session: expect.objectContaining({
          conversationId: "conversation-a",
        }),
        messageId: "user-2",
        undoApplyPatch: false,
      }),
    );
    // 输入框应被回填
    expect(chatInput.value).toBe("需要撤回");
  });
});
