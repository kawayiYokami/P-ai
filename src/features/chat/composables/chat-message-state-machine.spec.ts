import { describe, expect, it } from "vitest";
import type { ChatMessage } from "../../../types/app";
import {
  createChatMessageState,
  mergeAuthoritativeConversationMessages,
  reduceChatMessageState,
} from "./chat-message-state-machine";

function assistantMessage(
  id: string,
  text: string,
  providerMeta?: Record<string, unknown>,
): ChatMessage {
  return {
    id,
    role: "assistant",
    createdAt: "2026-07-26T08:00:00.000Z",
    parts: [{ type: "text", text }],
    providerMeta,
  };
}

describe("chat message state machine", () => {
  it("creates one placeholder and applies repeated deltas to the same message", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
      startedAt: "2026-07-26T08:00:00.000Z",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-1", delta: "第一段" },
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-1", delta: "第二段" },
    });

    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].id).toBe("assistant-1");
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("第一段第二段");
    expect(state.round.phase).toBe("streaming");
  });

  it("keeps duplicate round_started events from downgrading an active stream", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-1", delta: "正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
      phase: "waiting",
    });

    expect(state.round.phase).toBe("streaming");
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("正文");
  });

  it("ignores context usage signals without promoting the message round", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
      phase: "waiting",
    });
    const waitingMessage = state.messages[0];

    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: {
        activationId: "activation-1",
        kind: "context_usage_update",
        message: JSON.stringify({ contextUsagePercent: 42 }),
      },
    });

    expect(state.round.phase).toBe("waiting");
    expect(state.messages[0]).toBe(waitingMessage);
  });

  it("replaces the streaming projection with the formal message and keeps planCard", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
    });
    const stableRenderId = state.messages[0].providerMeta?._stableRenderId;
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { delta: "流式正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessage: assistantMessage("assistant-1", "正式正文", {
        planCard: { action: "present", path: ".pai/plan/example.md" },
      }),
    });

    expect(state.messages).toHaveLength(1);
    expect(state.messages[0].providerMeta?._streaming).toBeUndefined();
    expect(state.messages[0].providerMeta?._stableRenderId).toBe(stableRenderId);
    expect(state.messages[0].providerMeta?.planCard).toEqual({
      action: "present",
      path: ".pai/plan/example.md",
    });
    expect(state.round.phase).toBe("idle");
  });

  it("keeps visible streamed content when completion has no formal message", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { delta: "已显示正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
    });

    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("已显示正文");
    expect(state.messages[0].providerMeta?._streaming).toBeUndefined();
    expect(state.round.phase).toBe("idle");
  });

  it("keeps streamed blocks and accepts planCard when a same-id formal message arrives first", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { delta: "流式正文" },
    });
    state = reduceChatMessageState(state, {
      type: "authoritative_messages_merged",
      conversationId: "conversation-1",
      messages: [assistantMessage("assistant-1", "正式正文")],
    });
    // 同 ID 正式消息先到不覆盖流式投影，回合仍继续接收流式快照（不再被隐式收尾）。
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("流式正文");
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      snapshot: {
        updatedAt: "2026-07-26T08:00:00Z",
        streamBlocks: [{ text: "迟到快照", tools: [] }],
      },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      assistantMessage: assistantMessage("assistant-1", "迟到正文", {
        planCard: { action: "present", path: ".pai/plan/example.md" },
      }),
    });

    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("迟到快照");
    expect(state.messages[0].providerMeta?._streaming).toBeUndefined();
    expect(state.messages[0].providerMeta?.planCard).toEqual({
      action: "present",
      path: ".pai/plan/example.md",
    });
  });

  it("keeps the streaming projection when a same-id persisted message merges mid-round", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { delta: "流式正文" },
    });
    state = reduceChatMessageState(state, {
      type: "authoritative_messages_merged",
      conversationId: "conversation-1",
      messages: [assistantMessage("assistant-1", "半截持久化正文")],
    });

    expect(state.round.phase).toBe("streaming");
    expect(state.messages[0].providerMeta?._streaming).toBe(true);
  });

  it("clears the streaming projection when a settling round receives its final content", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      statusText: "等待回复",
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
    });
    expect(state.round.phase).toBe("settling");

    state = reduceChatMessageState(state, {
      type: "authoritative_messages_merged",
      conversationId: "conversation-1",
      messages: [assistantMessage("assistant-1", "回读正文")],
    });

    expect(state.round.phase).toBe("idle");
    expect(state.messages[0].providerMeta?._streaming).toBeUndefined();
  });

  it("clears a stale streaming projection when a terminal arrives with the machine round idle", () => {
    let state = createChatMessageState("conversation-1", [
      { ...assistantMessage("assistant-1", "半截正文"), providerMeta: { _streaming: true } },
    ]);
    expect(state.round.phase).toBe("idle");

    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      assistantMessage: assistantMessage("assistant-1", "正式正文"),
    });

    expect(state.round.phase).toBe("idle");
    expect(state.messages[0].providerMeta?._streaming).toBeUndefined();
  });

  it("enters settling when completion has neither formal nor visible content", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      statusText: "等待回复",
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
    });

    expect(state.round.phase).toBe("settling");
    state = reduceChatMessageState(state, {
      type: "authoritative_messages_merged",
      conversationId: "conversation-1",
      messages: [{
        ...assistantMessage("assistant-1", ""),
        providerMeta: { _streaming: true },
      }],
      options: { forceReplace: true },
    });
    expect(state.round.phase).toBe("settling");
    state = reduceChatMessageState(state, {
      type: "authoritative_messages_merged",
      conversationId: "conversation-1",
      messages: [assistantMessage("assistant-1", "回读正文")],
      options: { forceReplace: true },
    });
    expect(state.round.phase).toBe("idle");
    expect(state.messages[0].parts[0]).toEqual({ type: "text", text: "回读正文" });
  });

  it("does not let an older stream snapshot overwrite a newer revision", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      snapshot: {
        persistedAssistantMessageId: "assistant-1",
        activationId: "activation-1",
        updatedAt: "2026-07-26T08:00:02.000Z",
        streamBlocks: [{ text: "新内容", tools: [] }],
      },
    });
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      snapshot: {
        persistedAssistantMessageId: "assistant-1",
        activationId: "activation-1",
        updatedAt: "2026-07-26T08:00:01.000Z",
        streamBlocks: [{ text: "旧内容", tools: [] }],
      },
    });

    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("新内容");
    expect(state.round.revision).toBe("2026-07-26T08:00:02.000Z");
  });

  it("applies a newer full snapshot even when its second-level revision is unchanged", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      snapshot: {
        persistedAssistantMessageId: "assistant-1",
        activationId: "activation-1",
        updatedAt: "2026-07-26T08:00:02Z",
        streamBlocks: [{ text: "第一段", tools: [] }],
      },
    });
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      snapshot: {
        persistedAssistantMessageId: "assistant-1",
        activationId: "activation-1",
        updatedAt: "2026-07-26T08:00:02Z",
        streamBlocks: [{ text: "第一段第二段", tools: [] }],
      },
    });

    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("第一段第二段");
    expect(state.round.revision).toBe("2026-07-26T08:00:02Z");
  });

  it("ignores late events from another activation or conversation", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-new",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-old", delta: "旧内容" },
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-2",
      event: { activationId: "activation-new", delta: "其他会话" },
    });

    expect(state.messages[0].contentBlocks || []).toHaveLength(0);
    expect(state.round.activationId).toBe("activation-new");
  });

  it("does not let a stale completion without message id end the active round", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-new",
      activationId: "activation-new",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-new", delta: "当前正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      activationId: "activation-old",
    });

    expect(state.round.phase).toBe("streaming");
    expect(state.round.assistantMessageId).toBe("assistant-new");
    expect(state.messages[0].providerMeta?._streaming).toBe(true);
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("当前正文");
  });

  it("does not merge a stale formal completion into the active message", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-new",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-new", delta: "当前正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-old",
      assistantMessage: assistantMessage("assistant-1", "旧正式正文", {
        planCard: { action: "present", path: ".pai/plan/stale.md" },
      }),
    });

    expect(state.round.phase).toBe("streaming");
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("当前正文");
    expect(state.messages[0].providerMeta?.planCard).toBeUndefined();
    expect(state.messages[0].providerMeta?._streaming).toBe(true);
  });

  it("ignores a stale formal completion whose message id belongs to another round", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-new",
      activationId: "activation-new",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-new", delta: "当前正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-old",
      activationId: "activation-old",
      assistantMessage: assistantMessage("assistant-old", "旧轮次正文"),
    });

    expect(state.round.assistantMessageId).toBe("assistant-new");
    expect(state.messages.map((message) => message.id)).toEqual(["assistant-new"]);
  });

  it("rejects a different formal message id even when the terminal wrapper claims the active id", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-new",
      activationId: "activation-new",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-new", delta: "当前正文" },
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-new",
      activationId: "activation-new",
      assistantMessage: assistantMessage("assistant-old", "旧轮次正文"),
    });

    expect(state.round.phase).toBe("streaming");
    expect(state.messages.map((message) => message.id)).toEqual(["assistant-new"]);
    expect(state.messages[0].contentBlocks?.[0]?.text).toBe("当前正文");
  });

  it("inserts a completion that was missed locally at its authoritative timeline position", () => {
    let state = createChatMessageState("conversation-1", [{
      ...assistantMessage("assistant-later", "后续正文"),
      createdAt: "2026-07-26T09:00:00.000Z",
    }]);
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-missed",
      assistantMessage: assistantMessage("assistant-missed", "漏接正文"),
    });

    expect(state.messages.map((message) => message.id)).toEqual([
      "assistant-missed",
      "assistant-later",
    ]);
  });

  it("lets a new round replace an unresolved settling bubble", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-old",
      activationId: "activation-old",
      statusText: "等待旧回复",
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-old",
      activationId: "activation-old",
    });
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-new",
      activationId: "activation-new",
    });

    expect(state.round.phase).toBe("waiting");
    expect(state.round.assistantMessageId).toBe("assistant-new");
    expect(state.messages.map((message) => message.id)).toEqual(["assistant-new"]);
  });

  it("does not revive a settling round when round_started is replayed", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
      statusText: "等待回复",
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
    });
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
    });

    expect(state.round.phase).toBe("settling");
    expect(state.messages).toHaveLength(1);
  });

  it("does not revive a settling round with late delta or snapshot events", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
      statusText: "等待回复",
    });
    state = reduceChatMessageState(state, {
      type: "round_finished",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      activationId: "activation-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { activationId: "activation-1", delta: "迟到正文" },
    });
    state = reduceChatMessageState(state, {
      type: "assistant_stream_snapshot",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
      snapshot: {
        activationId: "activation-1",
        updatedAt: "2026-07-26T08:00:00Z",
        streamBlocks: [{ text: "迟到快照", tools: [] }],
      },
    });

    expect(state.round.phase).toBe("settling");
    expect(state.messages[0].contentBlocks || []).toHaveLength(0);
  });

  it("protects frozen text while accepting authoritative usage metadata", () => {
    const existing = assistantMessage("assistant-1", "停止时正文", { contextUsagePercent: 10 });
    const merged = mergeAuthoritativeConversationMessages(
      [existing],
      [assistantMessage("assistant-1", "迟到正文", { contextUsagePercent: 25 })],
    );

    expect(merged[0].parts[0]).toEqual({ type: "text", text: "停止时正文" });
    expect(merged[0].providerMeta?.contextUsagePercent).toBe(25);
  });

  it("lets an explicit force replacement discard stale partial stream blocks", () => {
    let state = createChatMessageState("conversation-1");
    state = reduceChatMessageState(state, {
      type: "round_started",
      conversationId: "conversation-1",
      assistantMessageId: "assistant-1",
    });
    state = reduceChatMessageState(state, {
      type: "assistant_delta",
      conversationId: "conversation-1",
      event: { delta: "截断正文" },
    });

    const messages = mergeAuthoritativeConversationMessages(
      state.messages,
      [assistantMessage("assistant-1", "完整正式正文")],
      { forceReplace: true },
    );

    expect(messages[0].parts[0]).toEqual({ type: "text", text: "完整正式正文" });
    expect(messages[0].contentBlocks).toBeUndefined();
    expect(messages[0].providerMeta?._streaming).toBeUndefined();
  });

  it("replaces an optimistic user draft and keeps summary seeds at the front", () => {
    const draft: ChatMessage = {
      id: "__draft_user__:1",
      role: "user",
      createdAt: "2026-07-26T08:00:02.000Z",
      parts: [{ type: "text", text: "草稿" }],
      providerMeta: { _optimistic: true, _stableRenderId: "draft-stable" },
    };
    const committed: ChatMessage = {
      id: "user-1",
      role: "user",
      createdAt: "2026-07-26T08:00:02.000Z",
      parts: [{ type: "text", text: "正式用户消息" }],
    };
    const seed: ChatMessage = {
      id: "seed-1",
      role: "system",
      createdAt: "2026-07-26T08:00:03.000Z",
      parts: [{ type: "text", text: "摘要" }],
      providerMeta: { message_meta: { kind: "summary_context_seed" } },
    };
    const merged = mergeAuthoritativeConversationMessages(
      [draft],
      [committed, seed],
      { replaceOptimisticUserDrafts: true, summarySeedsFirst: true },
    );

    expect(merged.map((message) => message.id)).toEqual(["seed-1", "user-1"]);
    expect(merged[1].providerMeta?._stableRenderId).toBe("draft-stable");
  });

  it("places new summary seeds before existing seeds while preserving incoming order", () => {
    const seed = (id: string, text: string): ChatMessage => ({
      id,
      role: "system",
      parts: [{ type: "text", text }],
      providerMeta: { message_meta: { kind: "summary_context_seed" } },
    });
    const merged = mergeAuthoritativeConversationMessages(
      [seed("seed-old", "旧摘要"), assistantMessage("assistant-1", "正文")],
      [seed("seed-new-1", "新摘要一"), seed("seed-new-2", "新摘要二")],
      { summarySeedsFirst: true },
    );

    expect(merged.map((message) => message.id)).toEqual([
      "seed-new-1",
      "seed-new-2",
      "seed-old",
      "assistant-1",
    ]);
  });

  it("prepends an older page without relying on message timestamps", () => {
    const message = (id: string, text: string): ChatMessage => ({
      id,
      role: "user",
      parts: [{ type: "text", text }],
    });
    const merged = mergeAuthoritativeConversationMessages(
      [message("current-1", "当前一"), message("current-2", "当前二")],
      [message("older-1", "旧一"), message("older-2", "旧二")],
      { prependMessages: true },
    );

    expect(merged.map((item) => item.id)).toEqual([
      "older-1",
      "older-2",
      "current-1",
      "current-2",
    ]);
  });

  it("keeps an explicit optimistic draft target until an own user message arrives", () => {
    const draft: ChatMessage = {
      id: "draft-explicit",
      role: "user",
      parts: [{ type: "text", text: "草稿" }],
      providerMeta: { _stableRenderId: "stable-explicit" },
    };
    const seed: ChatMessage = {
      id: "seed-1",
      role: "system",
      parts: [{ type: "text", text: "摘要" }],
      providerMeta: { message_meta: { kind: "summary_context_seed" } },
    };
    const committed: ChatMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "正式消息" }],
      speakerAgentId: "user-persona",
    };
    const merged = mergeAuthoritativeConversationMessages(
      [draft],
      [seed, committed],
      { optimisticUserDraftId: "draft-explicit", summarySeedsFirst: true },
    );

    expect(merged.map((message) => message.id)).toEqual(["seed-1", "user-1"]);
    expect(merged[1].providerMeta?._stableRenderId).toBe("stable-explicit");
  });
});
