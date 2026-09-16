import { describe, expect, it } from "vitest";
import { parseInlineSegments, type InlineSegment } from "./parse-markdown";
import { clipInlineSegments, inlineSegmentsLength } from "./inline-clip";

function text(value: string): InlineSegment {
  return { type: "text", text: value };
}

function code(value: string): InlineSegment {
  return { type: "code", text: value };
}

function ellipsisCount(segments: InlineSegment[]): number {
  return segments.filter((segment) => segment.type === "text" && segment.text === "…").length;
}

describe("inlineSegmentsLength", () => {
  it("按可见文本累计，嵌套段递归计算", () => {
    const segments: InlineSegment[] = [
      text("ab"),
      { type: "strong", children: [text("cde")] },
      code("fg"),
    ];
    expect(inlineSegmentsLength(segments)).toBe(7);
  });
});

describe("clipInlineSegments", () => {
  it("未超上限时原样返回", () => {
    const segments = [text("短文本")];
    expect(clipInlineSegments(segments, 300, 0.3)).toBe(segments);
  });

  it("上限为 0 或非法值时关闭截断", () => {
    const segments = [text("a".repeat(500))];
    expect(clipInlineSegments(segments, 0, 0.3)).toBe(segments);
    expect(clipInlineSegments(segments, Number.NaN, 0.3)).toBe(segments);
  });

  it("纯文本段按字切开，而不是整段保留", () => {
    const segments = [text("a".repeat(300))];
    const clipped = clipInlineSegments(segments, 100, 0.3);
    expect(clipped).toEqual([text("a".repeat(30)), text("…"), text("a".repeat(70))]);
    expect(inlineSegmentsLength(clipped)).toBe(101);
  });

  it("超出上限时插入省略号，并保留头尾", () => {
    const segments = [text("a".repeat(100)), text("b".repeat(100)), text("c".repeat(100))];
    const clipped = clipInlineSegments(segments, 120, 0.3);
    expect(ellipsisCount(clipped)).toBe(1);
    expect(clipped[0]).toEqual(text("a".repeat(36)));
    expect(clipped[clipped.length - 1]).toEqual(text("c".repeat(84)));
  });

  it("跨过头部预算的行内代码整段丢弃，不切出半截", () => {
    // 头部预算 30 字：第一段 10 字纳入，代码段 25 字跨过预算 → 整段丢弃
    const segments = [text("a".repeat(10)), code("b".repeat(25)), text("c".repeat(300))];
    const clipped = clipInlineSegments(segments, 100, 0.3);
    expect(clipped.filter((segment) => segment.type === "code")).toHaveLength(0);
    expect(clipped).toEqual([text("a".repeat(10)), text("…"), text("c".repeat(70))]);
  });

  it("行内代码能整体装进预算时完整保留", () => {
    const segments = [text("a".repeat(10)), code("b".repeat(15)), text("c".repeat(300))];
    const clipped = clipInlineSegments(segments, 100, 0.3);
    expect(clipped).toContainEqual(code("b".repeat(15)));
  });

  it("尾部切点也落在段边界，不会留下半截段", () => {
    const segments = [text("a".repeat(300)), code("b".repeat(25)), text("c".repeat(10))];
    const clipped = clipInlineSegments(segments, 100, 0.3);
    expect(clipped[0]).toEqual(text("a".repeat(30)));
    expect(clipped[clipped.length - 1]).toEqual(text("c".repeat(10)));
    expect(clipped[clipped.length - 2]).toEqual(code("b".repeat(25)));
  });

  it("整块装不下的单个标记段直接放弃截断", () => {
    const segments = [code("a".repeat(40))];
    const clipped = clipInlineSegments(segments, 20, 0.3);
    expect(clipped).toBe(segments);
  });

  it("头部被标记段挡住时只保留尾部，前面用省略号示意", () => {
    const segments = [code("a".repeat(20)), code("b".repeat(20))];
    const clipped = clipInlineSegments(segments, 30, 0.3);
    expect(clipped).toEqual([text("…"), code("b".repeat(20))]);
  });

  it("同一纯文本段被头尾各切一部分且未盖满时，正常插入省略号", () => {
    const segments = [text("a".repeat(500))];
    const clipped = clipInlineSegments(segments, 100, 0.3);
    expect(ellipsisCount(clipped)).toBe(1);
  });

  it("截断结果里的行内代码始终保持完整", () => {
    const segments = parseInlineSegments("甲".repeat(400) + "`code-block`" + "乙".repeat(20));
    const clipped = clipInlineSegments(segments, 100, 0.3);
    const codeSegments = clipped.filter((segment) => segment.type === "code");
    expect(codeSegments).toHaveLength(1);
    codeSegments.forEach((segment) => {
      if (segment.type === "code") expect(segment.text).toBe("code-block");
    });
  });
});
