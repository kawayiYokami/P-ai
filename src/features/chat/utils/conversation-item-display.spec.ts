import { describe, expect, it } from "vitest";
import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";
import {
  conversationPendingQuestionBadge,
  conversationRuntimeBusy,
  conversationUnreadBadge,
  hasUnreadOrRecentActivity,
  simpleConversationItemLevel,
} from "./conversation-item-display";

const NOW = Date.parse("2026-08-12T15:00:00+08:00");

function item(partial: Partial<ChatConversationOverviewItem>): ChatConversationOverviewItem {
  return {
    conversationId: "c1",
    title: "会话",
    messageCount: 0,
    ...partial,
  } as ChatConversationOverviewItem;
}

function preview(partial: Partial<ConversationPreviewMessage>): ConversationPreviewMessage {
  return { role: "assistant", textPreview: "hello", ...partial } as ConversationPreviewMessage;
}

describe("simpleConversationItemLevel / hasUnreadOrRecentActivity", () => {
  it("未读 → sim（摘要常显）", () => {
    expect(simpleConversationItemLevel(item({ unreadCount: 3 }), NOW)).toBe("sim");
    expect(hasUnreadOrRecentActivity(item({ unreadCount: 3 }), NOW)).toBe(true);
  });

  it("无未读但今天（凌晨 4 点起）有更新 → sim", () => {
    const recent = item({ updatedAt: "2026-08-12T10:00:00+08:00", lastMessageAt: "2026-08-12T10:00:00+08:00" });
    expect(simpleConversationItemLevel(recent, NOW)).toBe("sim");
  });

  it("天起点边界：当天 4 点整 → sim；4 点前一毫秒 → mini", () => {
    const atBoundary = item({ updatedAt: "2026-08-12T04:00:00+08:00" });
    expect(simpleConversationItemLevel(atBoundary, NOW)).toBe("sim");
    const beforeBoundary = item({ updatedAt: "2026-08-12T03:59:59.999+08:00" });
    expect(simpleConversationItemLevel(beforeBoundary, NOW)).toBe("mini");
  });

  it("当前时刻未到凌晨 4 点时，天起点回退到前一天 4 点", () => {
    const beforeDawn = Date.parse("2026-08-12T02:00:00+08:00");
    const yesterdayEvening = item({ updatedAt: "2026-08-11T22:00:00+08:00" });
    expect(simpleConversationItemLevel(yesterdayEvening, beforeDawn)).toBe("sim");
    const yesterdayMorning = item({ updatedAt: "2026-08-11T03:00:00+08:00" });
    expect(simpleConversationItemLevel(yesterdayMorning, beforeDawn)).toBe("mini");
  });

  it("无未读且今天之前无更新 → mini（折叠一行）", () => {
    const old = item({ updatedAt: "2026-07-01T10:00:00+08:00", lastMessageAt: "2026-07-01T10:00:00+08:00" });
    expect(simpleConversationItemLevel(old, NOW)).toBe("mini");
    expect(hasUnreadOrRecentActivity(old, NOW)).toBe(false);
  });

  it("无任何时间信息 → 非近期（mini）", () => {
    const empty = item({ updatedAt: "", lastMessageAt: "" });
    expect(simpleConversationItemLevel(empty, NOW)).toBe("mini");
  });
});

describe("conversationUnreadBadge", () => {
  it("当前会话不显示未读角标", () => {
    expect(conversationUnreadBadge(item({ unreadCount: 5 }), "c1")).toBe("");
  });

  it("0 或负数不显示", () => {
    expect(conversationUnreadBadge(item({ unreadCount: 0 }), "c2")).toBe("");
    expect(conversationUnreadBadge(item({ unreadCount: -1 }), "c2")).toBe("");
  });

  it("1~99 显示数字", () => {
    expect(conversationUnreadBadge(item({ unreadCount: 1 }), "c2")).toBe("1");
    expect(conversationUnreadBadge(item({ unreadCount: 99 }), "c2")).toBe("99");
  });

  it("超过 99 显示 99+", () => {
    expect(conversationUnreadBadge(item({ unreadCount: 100 }), "c2")).toBe("99+");
    expect(conversationUnreadBadge(item({ unreadCount: 1000 }), "c2")).toBe("99+");
  });
});

describe("conversationPendingQuestionBadge", () => {
  it("字段缺失（后端查询尚未接上）不显示", () => {
    expect(conversationPendingQuestionBadge(item({}))).toBe("");
  });

  it("0 或负数不显示", () => {
    expect(conversationPendingQuestionBadge(item({ questionCount: 0 }))).toBe("");
    expect(conversationPendingQuestionBadge(item({ questionCount: -2 }))).toBe("");
  });

  it("非法值不显示，不渲染 NaN", () => {
    expect(conversationPendingQuestionBadge(item({ questionCount: Number.NaN }))).toBe("");
  });

  it("1~99 显示数字", () => {
    expect(conversationPendingQuestionBadge(item({ questionCount: 1 }))).toBe("1");
    expect(conversationPendingQuestionBadge(item({ questionCount: 99 }))).toBe("99");
  });

  it("超过 99 显示 99+", () => {
    expect(conversationPendingQuestionBadge(item({ questionCount: 100 }))).toBe("99+");
  });

  it("当前会话照样显示：正看着它不代表问题已经回答过", () => {
    expect(conversationPendingQuestionBadge(item({ questionCount: 2 }))).toBe("2");
  });
});

describe("conversationRuntimeBusy", () => {
  it("流式/整理上下文/归档/压缩均为忙碌", () => {
    expect(conversationRuntimeBusy("assistant_streaming")).toBe(true);
    expect(conversationRuntimeBusy("organizing_context")).toBe(true);
    expect(conversationRuntimeBusy("archiving")).toBe(true);
    expect(conversationRuntimeBusy("compacting")).toBe(true);
  });

  it("idle 与空值不忙碌", () => {
    expect(conversationRuntimeBusy("idle")).toBe(false);
    expect(conversationRuntimeBusy(undefined)).toBe(false);
  });
});
