import { describe, expect, it } from "vitest";
import type { ChatMessageBlock } from "../src/types/app";
import {
  buildTimelineAnchors,
  isTimelineAnchorBlock,
  isSessionPersonaBlock,
  isUserBlock,
} from "../src/features/chat/utils/timeline-anchors";

const SESSION_AGENT_ID = "captain-agent";

function block(patch: Partial<ChatMessageBlock> & { id: string }): ChatMessageBlock {
  return {
    id: patch.id,
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
    ...patch,
  } as ChatMessageBlock;
}

function virtualIndex(blocks: ChatMessageBlock[]): Map<string, number> {
  const map = new Map<string, number>();
  blocks.forEach((item, index) => map.set(item.id, index));
  return map;
}

describe("时间线发言归属判定", () => {
  it("用户发言：是用户、不是当前会话人格", () => {
    const userBlock = block({ id: "u1", role: "user", speakerAgentId: "user-persona", text: "问一句" });
    expect(isUserBlock(userBlock)).toBe(true);
    expect(isSessionPersonaBlock(userBlock, SESSION_AGENT_ID)).toBe(false);
  });

  it("其他队员以 role=user 发言：不是用户、也不是当前会话人格", () => {
    const teammateBlock = block({ id: "t1", role: "user", speakerAgentId: "member-agent", text: "我来补一句" });
    expect(isUserBlock(teammateBlock)).toBe(false);
    expect(isSessionPersonaBlock(teammateBlock, SESSION_AGENT_ID)).toBe(false);
    expect(isTimelineAnchorBlock(teammateBlock, SESSION_AGENT_ID)).toBe(true);
  });

  it("当前会话人格发言：不算锚点", () => {
    const personaBlock = block({ id: "a1", role: "assistant", speakerAgentId: SESSION_AGENT_ID, text: "收到" });
    expect(isSessionPersonaBlock(personaBlock, SESSION_AGENT_ID)).toBe(true);
    expect(isTimelineAnchorBlock(personaBlock, SESSION_AGENT_ID)).toBe(false);
  });

  it("未标注发言人且 role=assistant 的旧数据算当前人格；role=user 的算锚点", () => {
    expect(isSessionPersonaBlock(block({ id: "a2", role: "assistant", text: "旧数据" }), SESSION_AGENT_ID)).toBe(true);
    expect(isSessionPersonaBlock(block({ id: "u2", role: "user", text: "旧用户消息" }), SESSION_AGENT_ID)).toBe(false);
  });

  it("系统人格发言：既不是用户，也不是当前会话人格，且不进锚点", () => {
    const systemBlock = block({ id: "s1", role: "user", speakerAgentId: "system-persona", text: "系统通知" });
    expect(isUserBlock(systemBlock)).toBe(false);
    expect(isSessionPersonaBlock(systemBlock, SESSION_AGENT_ID)).toBe(false);
    expect(isTimelineAnchorBlock(systemBlock, SESSION_AGENT_ID)).toBe(false);
  });
});

describe("锚点集合与按钮显隐同源", () => {
  // 场景：两个队员发言 + 两段人格回复；逐条时间线条目数远多于锚点数
  const blocks: ChatMessageBlock[] = [
    block({ id: "m1", role: "user", speakerAgentId: "member-a", text: "任务一" }),
    block({ id: "p1", role: "assistant", speakerAgentId: SESSION_AGENT_ID, text: "收到任务一" }),
    block({ id: "m2", role: "user", speakerAgentId: "member-b", text: "任务二" }),
    block({ id: "p2", role: "assistant", speakerAgentId: SESSION_AGENT_ID, text: "收到任务二" }),
  ];

  it("按键条件取锚点数，锚点不足两个时按钮不出现", () => {
    const anchors = buildTimelineAnchors(blocks, virtualIndex(blocks), SESSION_AGENT_ID);
    expect(anchors.map((anchor) => anchor.id)).toEqual(["m1", "m2"]);
    expect(anchors.length >= 2).toBe(true);

    const onlyPersonaAnchors = buildTimelineAnchors(
      [
        block({ id: "p1", role: "assistant", speakerAgentId: SESSION_AGENT_ID, text: "自说自话一" }),
        block({ id: "p2", role: "assistant", speakerAgentId: SESSION_AGENT_ID, text: "自说自话二" }),
      ],
      new Map([["p1", 0], ["p2", 1]]),
      SESSION_AGENT_ID,
    );
    expect(onlyPersonaAnchors.length).toBe(0);
    expect(onlyPersonaAnchors.length >= 2).toBe(false);
  });

  it("尾段取紧随其后的人格回复，遇到下一个非人格发言即停", () => {
    const anchors = buildTimelineAnchors(blocks, virtualIndex(blocks), SESSION_AGENT_ID);
    expect(anchors[0]?.assistantTail).toBe("收到任务一");
    expect(anchors[1]?.assistantTail).toBe("收到任务二");
  });

  it("拿不到虚拟下标的块不成为锚点（渲染窗口外无法跳转）", () => {
    const partial = new Map<string, number>([["m1", 0], ["p1", 1]]);
    const anchors = buildTimelineAnchors(blocks, partial, SESSION_AGENT_ID);
    expect(anchors.map((anchor) => anchor.id)).toEqual(["m1"]);
  });

  it("流式、附加文本、远程来源、无内容的块都不成为锚点", () => {
    const cases: ChatMessageBlock[] = [
      block({ id: "x1", role: "user", speakerAgentId: "user-persona", text: "流式中", isStreaming: true }),
      block({ id: "x2", role: "user", speakerAgentId: "user-persona", text: "附加文本", isExtraTextBlock: true }),
      block({
        id: "x3",
        role: "user",
        speakerAgentId: "user-persona",
        text: "远程消息",
        remoteImOrigin: { senderName: "远哥", remoteContactType: "single", channelId: "c", contactId: "i" },
      }),
      block({ id: "x4", role: "user", speakerAgentId: "user-persona", text: "   " }),
    ];
    for (const item of cases) {
      expect(isTimelineAnchorBlock(item, SESSION_AGENT_ID)).toBe(false);
    }
  });
});
