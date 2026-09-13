import { describe, expect, it } from "vitest";
import {
  carryStreamingProjection,
  useChatConversationMessageUtils,
} from "./use-chat-conversation-message-utils";

const utils = useChatConversationMessageUtils({
  ensureConversationMessageIds: (messages) => messages,
});

function streamingAssistant(text: string) {
  return {
    id: "assistant-1",
    role: "assistant",
    createdAt: "2026-07-26T08:00:00.000Z",
    parts: [{ type: "text", text }],
    providerMeta: { _streaming: true },
  };
}

function persistedAssistant(text: string) {
  return {
    id: "assistant-1",
    role: "assistant",
    createdAt: "2026-07-26T08:00:00.000Z",
    parts: [{ type: "text", text }],
    providerMeta: {},
  };
}

describe("conversation message merge keeps the streaming projection", () => {
  it("冻结基准会丢弃正在流式消息的 _streaming", () => {
    const frozen = utils.freezeConversationMessages([streamingAssistant("半截正文")]);

    expect(frozen[0].providerMeta?._streaming).toBeUndefined();
  });

  it("未冻结基准合并同 ID 的持久化副本时保留 _streaming", () => {
    const merged = utils.mergeMessagesIntoTimeline(
      [streamingAssistant("半截正文")],
      [persistedAssistant("完整正文")],
    );

    expect(merged[0].providerMeta?._streaming).toBe(true);
  });

  it("冻结基准合并后由其原始消息补回流式投影", () => {
    const original = [streamingAssistant("半截正文")];
    const base = utils.freezeConversationMessages(original);
    const merged = utils.mergeMessagesIntoTimeline(base, [persistedAssistant("完整正文")]);

    // 现状：基准被冻结，合并不带 _streaming
    expect(merged[0].providerMeta?._streaming).toBeUndefined();

    const carried = carryStreamingProjection(original, merged);

    expect(carried[0].providerMeta?._streaming).toBe(true);
  });

  it("原始消息里没有流式投影时不改动合并结果", () => {
    const target = [persistedAssistant("完整正文")];
    const carried = carryStreamingProjection([persistedAssistant("半截正文")], target);

    expect(carried).toBe(target);
  });
});
