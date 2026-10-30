import { computed, shallowRef } from "vue";
import { describe, expect, it } from "vitest";
import { useChatMessageBlocks } from "./use-chat-turns";
import type { ChatMessage, ToolCallMessage } from "../../../types/app";

function assistantToolEvent(text: string, ids: string[] = []): ToolCallMessage {
  return {
    role: "assistant",
    content: text,
    reasoning_content: "",
    tool_calls: ids.map((id) => ({
      id,
      type: "function",
      function: { name: "read_file", arguments: "{}" },
    })),
  };
}

function toolResultEvent(id: string, content: string): ToolCallMessage {
  return {
    role: "tool",
    content,
    tool_call_id: id,
    tool_calls: [],
  };
}

function assistantMessage(id: string, toolCall: ToolCallMessage[]): ChatMessage {
  return {
    id,
    role: "assistant",
    createdAt: "2026-01-01T00:00:00Z",
    parts: [],
    toolCall,
  } as ChatMessage;
}

function visibleToolItems(messages: ChatMessage[]) {
  const allMessages = shallowRef<ChatMessage[]>(messages);
  const api = useChatMessageBlocks({
    allMessages,
    activeChatApiConfig: computed(() => null),
    perfDebug: false,
    perfNow: () => 0,
  });
  return api.visibleMessageBlocks.value.flatMap((block) =>
    block.activityItems
      .filter((item) => item.kind === "tool")
      .map((item) => ({
        blockId: block.id,
        toolCallId: String(item.toolCallId || ""),
        resultText: String(item.resultText || ""),
      })),
  );
}

describe("工具调用展示层消解重复", () => {
  it("同一消息内先有预写占位、后有真实结果时只展示一次，且展示真实结果", () => {
    const items = visibleToolItems([
      assistantMessage("m1", [
        assistantToolEvent("", ["call_1"]),
        toolResultEvent("call_1", "工具执行尚未返回结果"),
        toolResultEvent("call_1", "真实结果"),
      ]),
    ]);

    expect(items.length).toBe(1);
    expect(items[0].resultText).toContain("真实结果");
  });

  it("压缩后新消息继承同一轮工具事件时，时间线上只展示一次", () => {
    const items = visibleToolItems([
      assistantMessage("m1", [
        assistantToolEvent("", ["call_1"]),
        toolResultEvent("call_1", "真实结果"),
      ]),
      assistantMessage("m2", [
        assistantToolEvent("压缩前的总结", ["call_1"]),
        toolResultEvent("call_1", "真实结果"),
      ]),
    ]);

    expect(items.length).toBe(1);
    expect(items[0].blockId).toBe("m1");
  });
});
