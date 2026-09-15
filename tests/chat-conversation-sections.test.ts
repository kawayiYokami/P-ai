import { describe, expect, it } from "vitest";
import type { ChatConversationOverviewItem } from "../src/types/app";
import {
  buildRemoteConversationSections,
  buildWorkspaceConversationSections,
} from "../src/features/chat/utils/conversation-sections";

function conversationItem(
  conversationId: string,
  workspaceRootPath: string,
  workspaceLabel?: string,
): ChatConversationOverviewItem {
  return {
    conversationId,
    title: conversationId,
    messageCount: 1,
    workspaceRootPath,
    workspaceLabel,
  };
}

describe("buildWorkspaceConversationSections", () => {
  it("sorts workspace groups by normalized path instead of input order", () => {
    const sections = buildWorkspaceConversationSections([
      conversationItem("story-1", "E:\\work\\b-story"),
      conversationItem("island-1", "E:\\work\\c-island"),
      conversationItem("easy-1", "E:\\work\\a-easy_call_ai"),
    ], {
      defaultWorkspaceTitle: "默认会话目录",
      locale: "zh-CN",
    });

    expect(sections.map((section) => section.workspaceRootPath)).toEqual([
      "E:\\work\\a-easy_call_ai",
      "E:\\work\\b-story",
      "E:\\work\\c-island",
    ]);
  });

  it("keeps conversations in the same workspace in their original order", () => {
    const sections = buildWorkspaceConversationSections([
      conversationItem("conversation-2", "E:\\work\\same"),
      conversationItem("conversation-1", "E:\\work\\same"),
    ], {
      defaultWorkspaceTitle: "默认会话目录",
      locale: "zh-CN",
    });

    expect(sections).toHaveLength(1);
    expect(sections[0]?.items.map((item) => item.conversationId)).toEqual([
      "conversation-2",
      "conversation-1",
    ]);
  });
});

describe("buildRemoteConversationSections", () => {
  it("groups remote conversations by channel and sorts channels by display name", () => {
    const sections = buildRemoteConversationSections([
      {
        conversationId: "contact-2",
        title: "Bob",
        kind: "remote_im_contact",
        channelId: "wechat",
        channelName: "微信",
        messageCount: 1,
      },
      {
        conversationId: "contact-1",
        title: "Alice",
        kind: "remote_im_contact",
        channelId: "dingtalk",
        channelName: "钉钉",
        messageCount: 1,
      },
      {
        conversationId: "contact-3",
        title: "Carol",
        kind: "remote_im_contact",
        channelId: "wechat",
        channelName: "微信",
        messageCount: 1,
      },
    ], {
      fallbackTitle: "其他会话",
      locale: "zh-CN",
    });

    expect(sections.map((section) => section.title)).toEqual([
      "钉钉",
      "微信",
    ]);
    expect(sections[1]?.items.map((item) => item.conversationId)).toEqual([
      "contact-2",
      "contact-3",
    ]);
  });

  it("keeps missing-channel conversations in a fallback group at the end", () => {
    const sections = buildRemoteConversationSections([
      {
        conversationId: "contact-1",
        title: "Alice",
        kind: "remote_im_contact",
        channelId: "wechat",
        channelName: "微信",
        messageCount: 1,
      },
      {
        conversationId: "contact-2",
        title: "Bob",
        kind: "remote_im_contact",
        messageCount: 1,
      },
    ], {
      fallbackTitle: "其他会话",
      locale: "zh-CN",
    });

    expect(sections.map((section) => section.title)).toEqual([
      "微信",
      "其他会话",
    ]);
    expect(sections[1]?.items.map((item) => item.conversationId)).toEqual([
      "contact-2",
    ]);
  });
});
