import { describe, expect, it } from "vitest";
import type { ChatMessageBlock } from "../src/types/app";
import { blockSizeDependencies } from "../src/features/chat/utils/chat-render";
import { textContentSignature } from "../src/features/chat/utils/text-signature";

function assistantBlock(overrides: Partial<ChatMessageBlock> = {}): ChatMessageBlock {
  return {
    id: "assistant-block",
    role: "assistant",
    text: "",
    images: [],
    audios: [],
    attachmentFiles: [],
    toolCallCount: 0,
    lastToolName: "",
    toolCalls: [],
    activityItems: [],
    activityReasoningCharCount: 0,
    activityToolCountsByName: {},
    activityRunning: false,
    activityStatus: "idle",
    ...overrides,
  };
}

describe("chat render signatures", () => {
  it("changes text signatures for equal-length content corrections", () => {
    expect("result-old").toHaveLength("result-new".length);
    expect(textContentSignature("result-old")).not.toBe(textContentSignature("result-new"));
  });

  it("changes block size dependencies when tool result text changes without growing", () => {
    const first = blockSizeDependencies(assistantBlock({
      activityItems: [{
        kind: "tool",
        id: "tool-1",
        toolCallId: "tool-1",
        name: "operate",
        argsText: "{\"action\":\"wait\"}",
        resultText: "result-old",
        status: "done",
      }],
      activityToolCountsByName: { operate: 1 },
      activityStatus: "complete",
    }));
    const second = blockSizeDependencies(assistantBlock({
      activityItems: [{
        kind: "tool",
        id: "tool-1",
        toolCallId: "tool-1",
        name: "operate",
        argsText: "{\"action\":\"wait\"}",
        resultText: "result-new",
        status: "done",
      }],
      activityToolCountsByName: { operate: 1 },
      activityStatus: "complete",
    }));

    expect(first).not.toEqual(second);
  });

  it("changes block size dependencies when a closed reasoning item is corrected without growing", () => {
    const first = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "reasoning", id: "reason-1", text: "reasoning-old", running: false }],
    }));
    const second = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "reasoning", id: "reason-1", text: "reasoning-new", running: false }],
    }));

    expect(first).not.toEqual(second);
  });

  it("changes block size dependencies when a closed content item is corrected without growing", () => {
    const first = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "content", id: "content-1", text: "content-old", running: false }],
    }));
    const second = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "content", id: "content-1", text: "content-new", running: false }],
    }));

    expect(first).not.toEqual(second);
  });

  it("still tracks a running item that grows by only one character", () => {
    const first = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "reasoning", id: "reason-1", text: "thinking", running: true }],
    }));
    const second = blockSizeDependencies(assistantBlock({
      activityItems: [{ kind: "reasoning", id: "reason-1", text: "thinking.", running: true }],
    }));

    expect(first).not.toEqual(second);
  });

  it("determines right-aligned messages correctly for standard and remote IM messages", async () => {
    const { isRightAlignedMessage } = await import("../src/features/chat/utils/chat-render");

    // standard user
    expect(isRightAlignedMessage(assistantBlock({ role: "user", speakerAgentId: undefined }))).toBe(true);
    // standard assistant
    expect(isRightAlignedMessage(assistantBlock({ role: "assistant", speakerAgentId: "fairy" }))).toBe(false);

    // remote IM private chat user message (e.g. weixin 1-on-1)
    expect(isRightAlignedMessage(assistantBlock({
      role: "user",
      speakerAgentId: undefined,
      remoteImOrigin: {
        senderName: "红豆",
        remoteContactType: "private",
        channelId: "remote-im-1",
        contactId: "c-1",
      },
    }))).toBe(true);

    // remote IM group chat user message
    expect(isRightAlignedMessage(assistantBlock({
      role: "user",
      speakerAgentId: undefined,
      remoteImOrigin: {
        senderName: "红豆",
        remoteContactType: "group",
        channelId: "remote-im-1",
        contactId: "c-1",
      },
    }))).toBe(false);

    // remote IM assistant message
    expect(isRightAlignedMessage(assistantBlock({
      role: "assistant",
      speakerAgentId: "fairy",
      remoteImOrigin: {
        senderName: "红豆",
        remoteContactType: "private",
        channelId: "remote-im-1",
        contactId: "c-1",
      },
    }))).toBe(false);
  });
});

