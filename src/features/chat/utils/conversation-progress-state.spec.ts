import { describe, expect, it } from "vitest";
import {
  patchConversationProgressStateList,
  readConversationProgressStatePayload,
} from "./conversation-progress-state";

function list(): Array<Record<string, any>> {
  return [
    {
      conversationId: "parent",
      title: "父会话",
      progressState: "",
      evidence: "",
      childConversations: [
        { conversationId: "side-1", title: "追问一", progressState: "", evidence: "" },
        { conversationId: "side-2", title: "追问二", progressState: "todo", evidence: "" },
      ],
    },
    { conversationId: "other", title: "别的会话", progressState: "blocked", evidence: "等接口" },
  ];
}

describe("readConversationProgressStatePayload", () => {
  it("读 camelCase 契约字段", () => {
    expect(readConversationProgressStatePayload({
      conversationId: "c1",
      progressState: "in_progress",
      evidence: "在跑测试",
      evidenceUpdatedAt: "2026-10-31T23:40:00+08:00",
    })).toEqual({
      conversationId: "c1",
      progressState: "in_progress",
      evidence: "在跑测试",
      evidenceUpdatedAt: "2026-10-31T23:40:00+08:00",
    });
  });

  it("空串状态读出空串（未设置），不是 null", () => {
    const payload = readConversationProgressStatePayload({ conversationId: "c1", progressState: "", evidence: "" });
    expect(payload?.progressState).toBe("");
  });

  it("没有 conversationId 的载荷直接丢弃", () => {
    expect(readConversationProgressStatePayload({ progressState: "done" })).toBeNull();
    expect(readConversationProgressStatePayload(null)).toBeNull();
  });
});

describe("patchConversationProgressStateList 事件消费", () => {
  it("主会话列表项收到事件后状态更新", () => {
    const before = list();
    const result = patchConversationProgressStateList(before, {
      conversationId: "other",
      progressState: "done",
      evidence: "用户说可以了",
      evidenceUpdatedAt: "2026-10-31T23:41:00+08:00",
    });
    expect(result.changed).toBe(true);
    expect(result.items[1].progressState).toBe("done");
    expect(result.items[1].evidence).toBe("用户说可以了");
  });

  // 追问会话单独测：它不走 overview 广播，只靠独立事件通道
  it("追问会话（childConversations 内）收到事件后状态更新，父项其余字段不变", () => {
    const result = patchConversationProgressStateList(list(), {
      conversationId: "side-1",
      progressState: "in_review",
      evidence: "方案已提交等你确认",
      evidenceUpdatedAt: "2026-10-31T23:42:00+08:00",
    });
    expect(result.changed).toBe(true);
    const parent = result.items[0];
    expect(parent.title).toBe("父会话");
    expect(parent.conversationId).toBe("parent");
    expect(parent.childConversations[0].progressState).toBe("in_review");
    expect(parent.childConversations[0].evidence).toBe("方案已提交等你确认");
    // 兄弟追问不受影响
    expect(parent.childConversations[1].progressState).toBe("todo");
  });

  it("追问会话状态置空即回到未设置", () => {
    const result = patchConversationProgressStateList(list(), {
      conversationId: "side-2",
      progressState: "",
      evidence: "",
      evidenceUpdatedAt: "",
    });
    expect(result.items[0].childConversations[1].progressState).toBe("");
  });

  it("状态没变但证据变了也要落库", () => {
    const result = patchConversationProgressStateList(list(), {
      conversationId: "other",
      progressState: "blocked",
      evidence: "改成等设计稿",
      evidenceUpdatedAt: "2026-10-31T23:43:00+08:00",
    });
    expect(result.changed).toBe(true);
    expect(result.items[1].evidence).toBe("改成等设计稿");
  });

  it("完全相同的载荷不产生新数组引用", () => {
    const before = list();
    const result = patchConversationProgressStateList(before, {
      conversationId: "other",
      progressState: "blocked",
      evidence: "等接口",
      evidenceUpdatedAt: "",
    });
    expect(result.changed).toBe(false);
    expect(result.items).toBe(before);
  });

  it("未命中的会话 id 不改动任何项", () => {
    const before = list();
    const result = patchConversationProgressStateList(before, {
      conversationId: "nobody",
      progressState: "done",
      evidence: "x",
      evidenceUpdatedAt: "",
    });
    expect(result.changed).toBe(false);
    expect(result.items).toBe(before);
  });

  it("不改动入参数组的原对象（不可变更新）", () => {
    const before = list();
    const snapshot = JSON.parse(JSON.stringify(before));
    patchConversationProgressStateList(before, {
      conversationId: "side-1",
      progressState: "done",
      evidence: "已收口",
      evidenceUpdatedAt: "",
    });
    expect(JSON.parse(JSON.stringify(before))).toEqual(snapshot);
  });
});
