import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../src/types/app";
import {
  inspectUndoablePatchCalls,
  normalizeMessageToolHistoryEvents,
  projectMessageForDisplay,
} from "../src/utils/chat-message-semantics";

function textMessage(id: string, role: ChatMessage["role"], text: string): ChatMessage {
  return {
    id,
    role,
    createdAt: "2026-03-28T10:00:00Z",
    parts: [{ type: "text", text }],
  };
}

describe("chat-message semantics", () => {
  it("projects display fields from unified message semantics", () => {
    const message: ChatMessage = {
      ...textMessage("a-1", "assistant", "我查好了\n[标准思考]\n先检查上下文"),
      providerMeta: {
        origin: {
          kind: "remote_im",
          sender_name: "张三",
          contact_name: "测试群",
          contact_type: "group",
          channel_id: "remote-im-1",
          contact_id: "group-1",
        },
      },
      toolCall: [
        {
          role: "assistant",
          tool_calls: [{
            id: "fc_1",
            call_id: "call_1",
            type: "function",
            function: {
              name: "remote_im_send",
              arguments: "{\"text\":\"你好\"}",
            },
          }],
        },
      ],
    };

    const projection = projectMessageForDisplay(message);

    expect(projection.text).toBe("我查好了");
    expect(projection.reasoningStandard).toContain("先检查上下文");
    expect(projection.toolCallCount).toBe(1);
    expect(projection.lastToolName).toBe("remote_im_send");
    expect(projection.toolCalls[0]).toEqual({
      name: "remote_im_send",
      argsText: "{\"text\":\"你好\"}",
    });
    expect(projection.remoteImOrigin).toEqual({
      senderName: "张三",
      remoteContactName: "测试群",
      remoteContactType: "group",
      channelId: "remote-im-1",
      contactId: "group-1",
    });
  });

  it("drops orphan assistant tool call only for prompt replay view", () => {
    const message: ChatMessage = {
      ...textMessage("a-2", "assistant", "处理中"),
      toolCall: [
        {
          role: "assistant",
          tool_calls: [{
            id: "fc_2",
            type: "function",
            function: {
              name: "bing_search",
              arguments: "{\"query\":\"rust\"}",
            },
          }],
        },
      ],
    };

    expect(normalizeMessageToolHistoryEvents(message, "display")).toHaveLength(1);
    expect(normalizeMessageToolHistoryEvents(message, "prompt")).toHaveLength(0);
  });

  it("inspects undoable patch calls through normalized tool history", () => {
    const messages: ChatMessage[] = [
      textMessage("u-1", "user", "请改一下文件"),
      {
        ...textMessage("a-3", "assistant", ""),
        toolCall: [
          {
            role: "assistant",
            tool_calls: [{
              id: "patch_1",
              type: "function",
              function: {
                name: "apply_patch",
                arguments: "*** Begin Patch\n*** Update File: C:/demo.txt\n*** End Patch",
              },
            }],
          },
          {
            role: "tool",
            tool_call_id: "patch_1",
            content: "{\"ok\":true}",
          },
        ],
      },
    ];

    const availability = inspectUndoablePatchCalls(messages, "u-1", {
      isApplyPatchArgsUndoable: (rawArgs) => rawArgs.includes("*** Begin Patch"),
    });

    expect(availability).toEqual({ canUndo: true, hint: "" });
  });
});
