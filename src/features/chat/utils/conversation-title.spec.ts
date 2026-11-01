import { describe, expect, it } from "vitest";
import { resolveConversationDisplayTitle } from "./conversation-title";

const untitledLabel = "未命名会话";

function resolve(item: Parameters<typeof resolveConversationDisplayTitle>[0]) {
  return resolveConversationDisplayTitle(item, { untitledLabel, locale: "zh-CN" });
}

describe("resolveConversationDisplayTitle", () => {
  it("会话标题优先", () => {
    expect(resolve({ conversationId: "c1", title: "重构登录流程", summaryTitle: "摘要", teamName: "李工" }))
      .toBe("重构登录流程");
  });

  it("标题与会话 id 相同时视为还没起标题，落到摘要标题", () => {
    expect(resolve({ conversationId: "c1", title: "c1", summaryTitle: "摘要标题" }))
      .toBe("摘要标题");
  });

  it("标题与摘要都空时用时间兜底，不显示裸 id", () => {
    const title = resolve({ conversationId: "c1", title: "", updatedAt: "2026-03-05T10:20:00+08:00" });
    expect(title).not.toBe("");
    expect(title).not.toBe("c1");
  });

  it("时间兜底之后才落人格名：队员在队时显示角色名", () => {
    expect(resolve({ conversationId: "c1", title: "", teamName: "李工" })).toBe("李工");
  });

  it("人格名兜底顺序是队员角色名优先，其次执行人格名", () => {
    expect(resolve({ conversationId: "c1", title: "", teamName: "李工", agentName: "王工" })).toBe("李工");
    expect(resolve({ conversationId: "c1", title: "", agentName: "王工" })).toBe("王工");
  });

  it("全都拿不到时才用未命名文案", () => {
    expect(resolve({ conversationId: "c1", title: "", teamName: "", agentName: "" })).toBe(untitledLabel);
  });

  it("远程私聊优先显示联系人名字", () => {
    expect(resolve({
      conversationId: "c1",
      kind: "remote_im_contact",
      title: "c1",
      remoteContactDisplayName: "红豆",
    })).toBe("红豆");
  });

  it("子会话摘要的 conversationKind 也参与类型判定", () => {
    expect(resolve({
      conversationId: "c1",
      conversationKind: "remote_im_contact",
      title: "",
      remoteContactDisplayName: "红豆",
    })).toBe("红豆");
  });

  it("系统通知会话显示通知口径，不落成时间或未命名", () => {
    expect(resolve({
      conversationId: "c1",
      title: "",
      isSystemNotificationConversation: true,
    })).toBe("系统通知");
  });
});
