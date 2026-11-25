import { describe, expect, it } from "vitest";
import { indexConversations } from "./conversation-picker";
import type { UnarchivedConversationSummary } from "../../../types/app";

function conversation(conversationId: string, title: string, updatedAt: string): UnarchivedConversationSummary {
  return {
    conversationId,
    title,
    updatedAt,
    messageCount: 0,
    unreadCount: 0,
    agentId: "agent-1",
  } as UnarchivedConversationSummary;
}

describe("indexConversations", () => {
  it("按更新时间倒序编号，最近的会话拿 1 号", () => {
    const indexed = indexConversations([
      conversation("conv-old", "旧会话", "2026-11-20T10:00:00+08:00"),
      conversation("conv-new", "新会话", "2026-11-25T10:00:00+08:00"),
      conversation("conv-mid", "中间会话", "2026-11-23T10:00:00+08:00"),
    ]);

    expect(indexed.map((item) => [item.index, item.id])).toEqual([
      [1, "conv-new"],
      [2, "conv-mid"],
      [3, "conv-old"],
    ]);
  });

  it("没有标题时走统一降级链，不退回会话 ID；缺 ID 的条目被剔除", () => {
    const indexed = indexConversations([
      conversation("conv-a", "", "2026-11-25T10:00:00+08:00"),
      conversation("  ", "空 ID", "2026-11-24T10:00:00+08:00"),
    ], { untitledLabel: "未命名会话", locale: "zh-CN" });

    expect(indexed).toHaveLength(1);
    expect(indexed[0].id).toBe("conv-a");
    expect(indexed[0].title).not.toBe("conv-a");
    expect(indexed[0].title).toContain("10:00");
  });

  it("标题等于会话 ID 时视为还没起标题，同样走时间兜底", () => {
    const indexed = indexConversations([
      conversation("conv-b", "conv-b", "2026-11-25T09:30:00+08:00"),
    ], { untitledLabel: "未命名会话", locale: "zh-CN" });

    expect(indexed[0].title).not.toBe("conv-b");
    expect(indexed[0].title).toContain("09:30");
  });

  it("入参为空时返回空数组", () => {
    expect(indexConversations(null)).toEqual([]);
    expect(indexConversations(undefined)).toEqual([]);
  });
});
