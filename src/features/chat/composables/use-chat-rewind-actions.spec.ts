import { ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import { useChatRewindActions } from "./use-chat-rewind-actions";

vi.mock("../../../i18n", () => ({
  i18n: {
    global: {
      t: (key: string) => key,
    },
  },
}));

type BusyFlags = {
  chatting?: boolean;
  trimming?: boolean;
  compactingConversation?: boolean;
  trimmingConversationId?: string;
  compactingConversationId?: string;
};

function createHarness(flags: BusyFlags = {}) {
  const chatErrorText = ref("");
  const setChatErrorText = vi.fn((text: string) => {
    chatErrorText.value = text;
  });
  const requestRecallMode = vi.fn(async () => "message_only" as const);

  const actions = useChatRewindActions({
    activeApiConfigId: ref("api"),
    activeAgentId: ref("agent"),
    currentConversationId: ref("conversation-a"),
    allMessages: ref([{ id: "turn-1", role: "user", createdAt: "", parts: [] }]) as never,
    chatting: ref(!!flags.chatting),
    trimming: ref(!!flags.trimming),
    compactingConversation: ref(!!flags.compactingConversation),
    trimmingConversationId: ref(flags.trimmingConversationId || ""),
    compactingConversationId: ref(flags.compactingConversationId || ""),
    chatErrorText,
    chatInput: ref(""),
    selectedMentions: ref([]),
    clipboardImages: ref([]),
    queuedAttachmentNotices: ref([]),
    deleteUnarchivedConversationFromArchives: async () => {},
    sendChat: async () => {},
    setStatusError: () => {},
    setChatErrorText,
    removeBinaryPlaceholders: (text: string) => text,
    messageText: () => "",
    extractMessageImages: () => [],
    extractMessageAttachmentFiles: () => [],
    requestRecallMode,
    requestCreateConversationBranchFromMessageConfirm: async () => false,
    createConversationBranchFromMessage: async () => {},
    branchingConversation: ref(false),
    refreshForegroundConversationAfterRewind: async () => {},
  });

  return { actions, chatErrorText, requestRecallMode };
}

describe("useChatRewindActions 忙碌拦截口径", () => {
  it("压缩别的会话不影响当前会话撤回", async () => {
    // 后台压缩另一个会话时，窗口级 compactingConversation 会置真；
    // 视图层按 id 判定为不忙，撤回也必须同样放行。
    const harness = createHarness({
      compactingConversation: true,
      compactingConversationId: "conversation-b",
    });

    await harness.actions.handleRecallTurn({ turnId: "turn-1" });

    expect(harness.requestRecallMode).toHaveBeenCalled();
  });

  it("修剪别的会话不影响当前会话撤回", async () => {
    const harness = createHarness({
      trimming: true,
      trimmingConversationId: "conversation-b",
    });

    await harness.actions.handleRecallTurn({ turnId: "turn-1" });

    expect(harness.requestRecallMode).toHaveBeenCalled();
  });

  it("压缩当前会话时仍然拦下撤回", async () => {
    const harness = createHarness({
      compactingConversation: true,
      compactingConversationId: "conversation-a",
    });

    await harness.actions.handleRecallTurn({ turnId: "turn-1" });

    expect(harness.requestRecallMode).not.toHaveBeenCalled();
    expect(harness.chatErrorText.value).toContain("rewind.failedBusy");
  });

  it("未指定会话 id 时按占用当前会话处理", async () => {
    const harness = createHarness({ compactingConversation: true });

    await harness.actions.handleRecallTurn({ turnId: "turn-1" });

    expect(harness.requestRecallMode).not.toHaveBeenCalled();
  });

  it("流式态仍然拦下撤回", async () => {
    const harness = createHarness({ chatting: true });

    await harness.actions.handleRecallTurn({ turnId: "turn-1" });

    expect(harness.requestRecallMode).not.toHaveBeenCalled();
  });
});
