import { describe, expect, it } from "vitest";
import { parseInlineSegments, type InlineSegment } from "./parse-markdown";

// 行内解析的等价性基准：这些期望值是「旧实现」的实测产出，逐条固化在这里。
// 改 findNextInlineMarker 的 protected ranges 传递方式时，任何一条对不上都说明边界被切坏。
// 压缩成单行字符串是为了让失败输出能直接看出是哪一段变了。
function compact(segments: InlineSegment[]): string {
  return segments
    .map((segment) => {
      if ("children" in segment) return `${segment.type}[${compact(segment.children)}]`;
      switch (segment.type) {
        case "text":
          return `t:${segment.text}`;
        case "code":
          return `c:${segment.text}`;
        case "math":
          return `m:${segment.text}|${segment.raw}|${segment.display ? 1 : 0}`;
        case "link":
          return `l:${segment.text}>${segment.href}`;
        case "image":
          return `i:${segment.alt}>${segment.src}`;
        case "imageLink":
          return `il:${segment.alt}>${segment.src}>${segment.href}`;
        case "toolcall_ref":
          return `tc:${segment.id}|${segment.label}`;
        case "footnote_ref":
          return `fn:${segment.id}`;
        default:
          return String(segment.type);
      }
    })
    .join(" ¶ ");
}

const CASES: Array<{ input: string; expected: string }> = [
  { input: "普通文字没有任何标记", expected: "t:普通文字没有任何标记" },
  { input: "**加粗**", expected: "strong[t:加粗]" },
  { input: "前 `code` 后", expected: "t:前  ¶ c:code ¶ t: 后" },
  { input: "公式 $E=mc^2$ 结尾", expected: "t:公式  ¶ m:E=mc^2|$E=mc^2$|0 ¶ t: 结尾" },
  { input: "[链接](https://a.com) 文字", expected: "l:链接>https://a.com ¶ t: 文字" },
  { input: "~~删除线~~", expected: "delete[t:删除线]" },
  { input: "***粗斜*** 混排", expected: "strongEm[t:粗斜] ¶ t: 混排" },
  { input: "**Issue 1: `heading_h1` 一直为空**", expected: "strong[t:Issue 1:  ¶ c:heading_h1 ¶ t: 一直为空]" },
  { input: "`**not strong**`", expected: "c:**not strong**" },
  { input: "**a $x$ b**", expected: "strong[t:a  ¶ m:x|$x$|0 ¶ t: b]" },
  { input: "$a * b$ 与 *emphasis*", expected: "m:a * b|$a * b$|0 ¶ t: 与  ¶ em[t:emphasis]" },
  { input: "***a** b*", expected: "strong[t:*a] ¶ t: b*" },
  { input: "~~a `b` c~~", expected: "delete[t:a  ¶ c:b ¶ t: c]" },
  {
    input: "按 <kbd>Ctrl</kbd>+<kbd>K</kbd><br>H<sub>2</sub>O 与 x<sup>2</sup>，<mark>重点</mark>",
    expected:
      "t:按  ¶ html_kbd[t:Ctrl] ¶ t:+ ¶ html_kbd[t:K] ¶ html_br ¶ t:H ¶ html_sub[t:2] ¶ t:O 与 x ¶ html_sup[t:2] ¶ t:， ¶ html_mark[t:重点]",
  },
  { input: "**外层 `code` 里 `*` 和 `**`**", expected: "strong[t:外层  ¶ c:code ¶ t: 里  ¶ c:* ¶ t: 和  ¶ c:**]" },
  {
    input: "`a` 与 **b** 与 $c$ 与 ~~d~~ 与 [e](f)",
    expected: "c:a ¶ t: 与  ¶ strong[t:b] ¶ t: 与  ¶ m:c|$c$|0 ¶ t: 与  ¶ delete[t:d] ¶ t: 与  ¶ l:e>f",
  },
  { input: "** 未闭合", expected: "t:** 未闭合" },
  { input: "a **b** c *d* e ~~f~~ g", expected: "t:a  ¶ strong[t:b] ¶ t: c  ¶ em[t:d] ¶ t: e  ¶ delete[t:f] ¶ t: g" },
  { input: "**[链接](https://u.com)**", expected: "strong[l:链接>https://u.com]" },
  { input: "$x$**y**$z$", expected: "m:x|$x$|0 ¶ strong[t:y] ¶ m:z|$z$|0" },
  { input: "**a ~~b `c` d~~ e**", expected: "strong[t:a  ¶ delete[t:b  ¶ c:c ¶ t: d] ¶ t: e]" },
  {
    input: "纯文本 $x$ 纯文本 $y$ 纯文本 $z$ 纯文本",
    expected: "t:纯文本  ¶ m:x|$x$|0 ¶ t: 纯文本  ¶ m:y|$y$|0 ¶ t: 纯文本  ¶ m:z|$z$|0 ¶ t: 纯文本",
  },
  { input: "1. `a` **b** 2. `c` **d**", expected: "t:1.  ¶ c:a ¶ t:  ¶ strong[t:b] ¶ t: 2.  ¶ c:c ¶ t:  ¶ strong[t:d]" },
];

describe("parseInlineSegments", () => {
  it.each(CASES)("等价性：$input", ({ input, expected }) => {
    expect(compact(parseInlineSegments(input))).toBe(expected);
  });

  it("长混排逐段与短样本一致：拼接后不因长度改变切分", () => {
    const unit = "`a` 与 **b** 与 $c$ 与 ~~d~~ 与 [e](f) ";
    const single = compact(parseInlineSegments(unit));
    const long = compact(parseInlineSegments(unit.repeat(20)));
    expect(long.split(" ¶ ").length).toBe(single.split(" ¶ ").length * 20);
  });
});
