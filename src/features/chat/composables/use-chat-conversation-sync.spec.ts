import { describe, expect, it, vi } from "vitest";
import { useChatConversationSync } from "./use-chat-conversation-sync";

function createBindings() {
  return {
    ensureConversationMessageIds: vi.fn(),
    unarchivedConversations: { value: [] as any[] },
    lastOverviewSyncAt: { value: "" },
    setConversationChatErrorText: vi.fn(),
  };
}

function overviewItem(conversationId: string, lastMessageAt: string, unreadCount = 0) {
  return {
    conversationId,
    lastMessageAt,
    updatedAt: lastMessageAt,
    unreadCount,
    title: `title-${conversationId}`,
  };
}

describe("applyConversationOverviewItemUpdated", () => {
  it("排序键未变化时原位替换目标项，不重排整个列表", () => {
    const bindings = createBindings();
    // 故意乱序：B 比 A 新，但数组里 B 在前（若发生重排，A 会被排到前面）
    bindings.unarchivedConversations.value = [
      overviewItem("b", "2026-08-09T12:00:00"),
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);

    applyConversationOverviewItemUpdated({
      conversation: overviewItem("b", "2026-08-09T12:00:00", 5),
    });

    // 顺序保持原样（未重排），目标项已被替换为新对象（unreadCount=5）
    expect(bindings.unarchivedConversations.value.map((item: any) => item.conversationId)).toEqual(["b", "a"]);
    expect(bindings.unarchivedConversations.value[0].unreadCount).toBe(5);
  });

  it("排序键变化（最近活动时间更新）时仍按活动时间重排", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      overviewItem("b", "2026-08-09T12:00:00"),
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);

    // a 收到新消息，lastMessageAt 更新为最新
    applyConversationOverviewItemUpdated({
      conversation: overviewItem("a", "2026-08-09T13:00:00"),
    });

    expect(bindings.unarchivedConversations.value.map((item: any) => item.conversationId)).toEqual(["a", "b"]);
  });

  it("列表不存在目标项时追加并按活动时间排到正确位置", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      overviewItem("b", "2026-08-09T12:00:00"),
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);

    applyConversationOverviewItemUpdated({
      conversation: overviewItem("c", "2026-08-09T12:30:00"),
    });

    expect(bindings.unarchivedConversations.value.map((item: any) => item.conversationId)).toEqual(["c", "b", "a"]);
  });

  it("签名完全一致时不替换数组", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      overviewItem("b", "2026-08-09T12:00:00"),
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);
    const before = bindings.unarchivedConversations.value;

    applyConversationOverviewItemUpdated({
      conversation: overviewItem("a", "2026-08-09T11:00:00"),
    });

    expect(bindings.unarchivedConversations.value).toBe(before);
  });

  it("仅 agentId 变化（草稿切换人格）时必须替换列表项", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      { ...overviewItem("a", "2026-08-09T11:00:00"), agentId: "persona-old" },
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);
    const before = bindings.unarchivedConversations.value;

    applyConversationOverviewItemUpdated({
      conversation: {
        ...overviewItem("a", "2026-08-09T11:00:00"),
        agentId: "persona-new",
      },
    });

    expect(bindings.unarchivedConversations.value).not.toBe(before);
    expect(bindings.unarchivedConversations.value[0].agentId).toBe("persona-new");
  });

  it("lastError 变化时同步到会话错误文本，为空时清除", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      overviewItem("b", "2026-08-09T12:00:00"),
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);

    // 带 lastError 的更新：写入错误文本
    applyConversationOverviewItemUpdated({
      conversation: { ...overviewItem("a", "2026-08-09T11:00:00"), lastError: "上游调用失败" },
    });
    expect(bindings.setConversationChatErrorText).toHaveBeenCalledWith("a", "上游调用失败");

    // lastError 清空：清除错误文本
    applyConversationOverviewItemUpdated({
      conversation: { ...overviewItem("a", "2026-08-09T11:00:00"), lastError: "" },
    });
    expect(bindings.setConversationChatErrorText).toHaveBeenCalledWith("a", "");
  });

  it("lastError 变化足以让签名不同，强制替换列表项", () => {
    const bindings = createBindings();
    bindings.unarchivedConversations.value = [
      overviewItem("a", "2026-08-09T11:00:00"),
    ];
    const { applyConversationOverviewItemUpdated } = useChatConversationSync(bindings);
    const before = bindings.unarchivedConversations.value;

    applyConversationOverviewItemUpdated({
      conversation: { ...overviewItem("a", "2026-08-09T11:00:00"), lastError: "失败" },
    });

    expect(bindings.unarchivedConversations.value).not.toBe(before);
    expect(bindings.unarchivedConversations.value[0].lastError).toBe("失败");
  });
});
