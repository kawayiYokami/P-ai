import { describe, expect, it } from "vitest";
import {
  appendTextDeltaToStreamBlocks,
  applyAssistantToolEventToStreamBlocks,
  assistantTextFromStreamBlocks,
  projectMessageForDisplay,
  streamBlocksToActivityItems,
  streamBlocksToActivitySummaryItems,
  TOOL_TEXT_BREAK_PLACEHOLDER,
} from "./chat-message-semantics";
import type { ChatMessage, ToolCallMessage } from "../types/app";

const MARK = TOOL_TEXT_BREAK_PLACEHOLDER;
const CALL_A = "call_a";
const CALL_B = "call_b";

function makeToolCallEvent(
  role: "assistant" | "tool",
  content: string,
  toolCallIds: string[] = [],
  toolCallId?: string,
): ToolCallMessage {
  return {
    role,
    content,
    tool_calls: toolCallIds.map((id) => ({
      id,
      type: "function",
      function: { name: "toolA", arguments: "{}" },
    })),
    tool_call_id: toolCallId,
  };
}

function makeAssistantMessage(
  partsText: string,
  toolCall: ToolCallMessage[],
): ChatMessage {
  return {
    id: "test-message",
    role: "assistant",
    parts: [{ type: "text", text: partsText }],
    toolCall,
  };
}

describe("projectMessageForDisplay 分段占位符一致性", () => {
  it("分支4：工具标记后紧邻正文时插入占位符（完成态与流式一致分段）", () => {
    const msg = makeAssistantMessage(
      `正文1 [toolcall:${CALL_A}]正文2`,
      [
        makeToolCallEvent("assistant", "正文1", [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
        makeToolCallEvent("assistant", "正文2"),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    const segments = projection.text.split(MARK);
    expect(segments.length).toBe(2);
    expect(segments[0]).toBe(`正文1 [toolcall:${CALL_A}]`);
    expect(segments[1]).toBe("正文2");
  });

  it("分支4：纯标记事件后的正文边界也插入占位符（连续标记合并为前段）", () => {
    const msg = makeAssistantMessage(
      `正文1 [toolcall:${CALL_A}][toolcall:${CALL_B}]正文2`,
      [
        makeToolCallEvent("assistant", "正文1", [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
        makeToolCallEvent("assistant", "", [CALL_B]),
        makeToolCallEvent("tool", "结果2", [], CALL_B),
        makeToolCallEvent("assistant", "正文2"),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    const segments = projection.text.split(MARK);
    expect(segments.length).toBe(2);
    expect(segments[0]).toBe(`正文1 [toolcall:${CALL_A}][toolcall:${CALL_B}]`);
    expect(segments[1]).toBe("正文2");
  });

  it("分支4：工具标记后无正文时不插入占位符", () => {
    const msg = makeAssistantMessage(
      `正文1 [toolcall:${CALL_A}]`,
      [
        makeToolCallEvent("assistant", "正文1", [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    expect(projection.text).not.toContain(MARK);
  });

  it("分支4：已含占位符（流式缓存文本）时不重复插入", () => {
    const msg = makeAssistantMessage(
      `正文1 [toolcall:${CALL_A}]${MARK}正文2`,
      [
        makeToolCallEvent("assistant", "正文1", [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
        makeToolCallEvent("assistant", "正文2"),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    const segments = projection.text.split(MARK);
    expect(segments.length).toBe(2);
    expect(projection.text).toBe(`正文1 [toolcall:${CALL_A}]${MARK}正文2`);
  });

  it("无工具调用消息不产生占位符（现状回归）", () => {
    const msg = makeAssistantMessage("纯正文文本", []);
    const projection = projectMessageForDisplay(msg);
    expect(projection.text).toBe("纯正文文本");
    expect(projection.text).not.toContain(MARK);
  });

  it("分支4：事件文本已含同名工具标记时，占位符插在追加标记（最后位置）后", () => {
    const msg = makeAssistantMessage(
      `开场 [toolcall:${CALL_A}] 过渡正文2`,
      [
        makeToolCallEvent("assistant", `开场 [toolcall:${CALL_A}] 过渡`, [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
        makeToolCallEvent("assistant", "正文2"),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    const segments = projection.text.split(MARK);
    expect(segments.length).toBe(2);
    // 事件文本自带的 [toolcall:call_a] 保持原样，占位符只出现在事件追加的标记（最后一个）之后
    expect(segments[0]).toBe(`开场 [toolcall:${CALL_A}] 过渡 [toolcall:${CALL_A}]`);
    expect(segments[1]).toBe("正文2");
  });

  it("分支5（join 路径）行为不变：事件正文不在 parts 时仍按旧规则分段", () => {
    const msg = makeAssistantMessage(
      "最终答案正文",
      [
        makeToolCallEvent("assistant", "过渡语", [CALL_A]),
        makeToolCallEvent("tool", "结果", [], CALL_A),
        makeToolCallEvent("assistant", "最终答案正文"),
      ],
    );
    const projection = projectMessageForDisplay(msg);
    const segments = projection.text.split(MARK);
    expect(segments.length).toBe(2);
    expect(segments[0]).toContain(`[toolcall:${CALL_A}]`);
  });
});
describe("streamBlocksToActivityItems 按事件切块后的块粒度出条目", () => {
  const blocks = [
    {
      reasoning: "思考1",
      text: `正文1 [toolcall:${CALL_A}]`,
      tools: [
        { toolCallId: CALL_A, name: "toolA", argsText: "{}", status: "done" },
      ],
    },
    {
      reasoning: "",
      text: `正文2 [toolcall:${CALL_B}]`,
      tools: [
        { toolCallId: CALL_B, name: "toolB", argsText: "{}", status: "done" },
      ],
    },
  ];

  it("展开明细：每块一个 content item，工具紧随其后，块间天然交错", () => {
    const items = streamBlocksToActivityItems(blocks);
    const kinds = items.map((item) => item.kind);
    expect(kinds).toEqual(["reasoning", "content", "tool", "content", "tool"]);
    const contents = items.filter((item) => item.kind === "content");
    // 每段保留各自的工具标记，渲染侧 activityItemText 再剥离
    expect(contents.map((item) => item.text)).toEqual([
      `正文1 [toolcall:${CALL_A}]`,
      `正文2 [toolcall:${CALL_B}]`,
    ]);
    const tools = items.filter((item) => item.kind === "tool");
    expect(tools.map((item) => item.name)).toEqual(["toolA", "toolB"]);
  });

  it("折叠 summary：与展开明细相同的交错顺序", () => {
    const items = streamBlocksToActivitySummaryItems(blocks);
    const kinds = items.map((item) => item.kind);
    expect(kinds).toEqual(["reasoning", "content", "tool", "content", "tool"]);
    const tools = items.filter((item) => item.kind === "tool");
    expect(tools.map((item) => item.name)).toEqual(["toolA", "toolB"]);
  });

  it("单正文块（无工具）仍只生成一个 content item（现状回归）", () => {
    const items = streamBlocksToActivityItems([{ reasoning: "", text: "纯正文", tools: [] }]);
    expect(items.filter((item) => item.kind === "content")).toHaveLength(1);
  });

  it("纯工具块（无正文）工具保留在块尾", () => {
    const items = streamBlocksToActivityItems([
      { reasoning: "", text: "", tools: [{ toolCallId: CALL_A, name: "toolA", argsText: "{}", status: "done" }] },
    ]);
    expect(items.map((item) => item.kind)).toEqual(["tool"]);
    const tools = items.filter((item) => item.kind === "tool");
    expect(tools[0].name).toBe("toolA");
  });
});

describe("appendTextDeltaToStreamBlocks 工具标记后切新块", () => {
  it("pendingTextBreak 时正文 delta 开新块，不再拼占位符进原块", () => {
    const blocks = appendTextDeltaToStreamBlocks([], "正文1。");
    const withTool = applyAssistantToolEventToStreamBlocks(blocks, JSON.stringify({
      role: "assistant",
      content: null,
      tool_calls: [{
        id: CALL_A,
        type: "function",
        function: { name: "toolA", arguments: "{}" },
      }],
    }));
    const next = appendTextDeltaToStreamBlocks(withTool, "正文2。");

    expect(next).toEqual([
      {
        reasoning: "",
        reasoningCharCount: 0,
        text: `正文1。 [toolcall:${CALL_A}]`,
        tools: [{
          toolCallId: CALL_A,
          name: "toolA",
          argsText: "",
          resultText: undefined,
          status: "doing",
        }],
        pendingTextBreak: true,
      },
      {
        reasoning: "",
        reasoningCharCount: 0,
        text: "正文2。",
        tools: [],
        pendingTextBreak: false,
      },
    ]);
  });

  it("切块后气泡渲染文本仍注入占位符（joinAssistantHistoryTexts）", () => {
    const blocks = appendTextDeltaToStreamBlocks([], "正文1。");
    const withTool = applyAssistantToolEventToStreamBlocks(blocks, JSON.stringify({
      role: "assistant",
      content: null,
      tool_calls: [{
        id: CALL_A,
        type: "function",
        function: { name: "toolA", arguments: "{}" },
      }],
    }));
    const next = appendTextDeltaToStreamBlocks(withTool, "正文2。");
    const text = assistantTextFromStreamBlocks(next);
    expect(text).toBe(`正文1。 [toolcall:${CALL_A}]${MARK}正文2。`);
  });

  it("块文本尾带换行时拼接工具标记先清理尾随空白", () => {
    const blocks = appendTextDeltaToStreamBlocks([], "正文1。\n");
    const withTool = applyAssistantToolEventToStreamBlocks(blocks, JSON.stringify({
      role: "assistant",
      content: null,
      tool_calls: [{
        id: CALL_A,
        type: "function",
        function: { name: "toolA", arguments: "{}" },
      }],
    }));

    expect(withTool[0]?.text).toBe(`正文1。 [toolcall:${CALL_A}]`);
    expect(assistantTextFromStreamBlocks(withTool)).toBe(`正文1。 [toolcall:${CALL_A}]`);
  });

  it("流式块文本尾带换行时补插 done 标记先清理尾随空白", () => {
    const text = assistantTextFromStreamBlocks([{
      reasoning: "",
      reasoningCharCount: 0,
      text: "正文1。\n",
      tools: [{
        toolCallId: CALL_A,
        name: "toolA",
        argsText: "{}",
        resultText: "结果",
        status: "done",
      }],
      pendingTextBreak: false,
    }]);
    expect(text).toBe(`正文1。 [toolcall:${CALL_A}]`);
  });
});

describe("工具调用空参数不落成裸的 {}", () => {
  const emptyArgShapes: Array<[string, string | Record<string, unknown> | unknown[]]> = [
    ["字符串 \"{}\"", "{}"],
    ["空对象", {}],
    ["空数组", []],
  ];

  for (const [label, argsValue] of emptyArgShapes) {
    it(`arguments 为${label}时 argsText 归一为空串`, () => {
      const blocks = applyAssistantToolEventToStreamBlocks([], JSON.stringify({
        role: "assistant",
        content: null,
        tool_calls: [{
          id: CALL_A,
          type: "function",
          function: { name: "get_session", arguments: argsValue },
        }],
      }));
      expect(blocks[0]?.tools?.[0]?.argsText).toBe("");
    });
  }

  it("arguments 缺失时 argsText 归一为空串", () => {
    const blocks = applyAssistantToolEventToStreamBlocks([], JSON.stringify({
      role: "assistant",
      content: null,
      tool_calls: [{
        id: CALL_A,
        type: "function",
        function: { name: "get_session" },
      }],
    }));
    expect(blocks[0]?.tools?.[0]?.argsText).toBe("");
  });

  it("有参数的工具 argsText 原样保留", () => {
    const argsText = JSON.stringify({ path: "src/a.ts" });
    const blocks = applyAssistantToolEventToStreamBlocks([], JSON.stringify({
      role: "assistant",
      content: null,
      tool_calls: [{
        id: CALL_A,
        type: "function",
        function: { name: "read", arguments: argsText },
      }],
    }));
    expect(blocks[0]?.tools?.[0]?.argsText).toBe(argsText);
  });

  it("空参数工具活动项不带参数文本，仍保留工具名", () => {
    const items = streamBlocksToActivityItems([{
      reasoning: "",
      reasoningCharCount: 0,
      text: "",
      tools: [{
        toolCallId: CALL_A,
        name: "get_session",
        argsText: "",
        resultText: "会话列表",
        status: "done",
      }],
      pendingTextBreak: false,
    }]);
    const tool = items.find((item) => item.kind === "tool");
    expect(tool?.name).toBe("get_session");
    expect(tool?.argsText).toBe("");
  });
});
