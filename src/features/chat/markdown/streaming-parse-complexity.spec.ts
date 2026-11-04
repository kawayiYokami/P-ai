import { describe, expect, it } from "vitest";
import { IncrementalMarkdownBlockParser } from "./incremental-markdown";
import { parseMarkdownBlocks, type MarkdownBlock } from "./parse-markdown";

// 守卫：流式渲染的增量必须靠「前缀冻结 + 块身份稳定」成立，而不是每帧全量重解析。
// 这几条钉的是解析行为本身，不钉耗时——谁把增量改回全量，这里当场红。

const CORPUS = [
  "## 这一段是压测语料\n\n",
  "正文里有一句 **加粗** 与 `行内代码`，还有公式 $E=mc^2$ 以及 [链接](https://example.com)。\n\n",
  "- 列表项一\n- 列表项二\n\n",
  "```ts\nconst answer = 42;\n```\n\n",
  "| 列 A | 列 B |\n| --- | --- |\n| 1 | 2 |\n\n",
  "> 引用块里的一行说明。\n\n",
].join("");

function stripKeys(blocks: MarkdownBlock[]): unknown[] {
  return blocks.map((block) => ({ ...block, key: undefined }));
}

describe("IncrementalMarkdownBlockParser 前缀冻结", () => {
  it("冻结块的 key 与对象引用跨轮次稳定", () => {
    const parser = new IncrementalMarkdownBlockParser();
    const text = CORPUS.repeat(6).slice(0, 4000);
    const frozenRefs = new Map<string, MarkdownBlock>();
    let referenceBreaks = 0;
    let firstBroken = "";

    for (let cursor = 10; cursor <= text.length; cursor += 10) {
      const blocks = parser.parse(text.slice(0, cursor));
      // 上一轮认定为冻结的块，这一轮必须还是同一个对象（Vue 才能复用 VNode）
      for (const [key, reference] of frozenRefs) {
        const current = blocks.find((block) => block.key === key);
        if (!current) continue;
        if (current !== reference) {
          referenceBreaks += 1;
          if (!firstBroken) firstBroken = key;
        }
      }
      // 本轮再认定「除尾部两块以外」的全部块为冻结
      for (const block of blocks.slice(0, Math.max(0, blocks.length - 2))) {
        if (!frozenRefs.has(block.key)) frozenRefs.set(block.key, block);
      }
    }

    expect(firstBroken).toBe("");
    expect(referenceBreaks).toBe(0);
    expect(frozenRefs.size).toBeGreaterThan(10);
  });

  it("块 key 唯一，冻结区身份只随文本追加而增多", () => {
    const parser = new IncrementalMarkdownBlockParser();
    const text = CORPUS.repeat(6).slice(0, 4000);
    let previousFrozenKeys: string[] = [];
    for (let cursor = 10; cursor <= text.length; cursor += 10) {
      const blocks = parser.parse(text.slice(0, cursor));
      const keys = blocks.map((block) => block.key);
      // 同一轮里不允许出现重复身份
      expect(new Set(keys).size).toBe(keys.length);
      // 已冻结的块身份不能因为文本变长而改变，否则渲染层会整块重建
      for (const key of previousFrozenKeys) expect(keys).toContain(key);
      // 尾部两块本来就可能被重塑（表格成形、段落续接），不算在内
      previousFrozenKeys = blocks.slice(0, Math.max(0, blocks.length - 2)).map((block) => block.key);
    }
    expect(previousFrozenKeys.length).toBeGreaterThan(5);
    expect(previousFrozenKeys.every((key) => /^#\d+$/.test(key))).toBe(true);
  });

  it("流式收尾的块序列与一次性全量解析一致", () => {
    const parser = new IncrementalMarkdownBlockParser();
    const text = CORPUS.repeat(4);
    let blocks: MarkdownBlock[] = [];
    for (let cursor = 7; cursor <= text.length; cursor += 7) {
      blocks = parser.parse(text.slice(0, cursor));
    }
    blocks = parser.parse(text);
    expect(stripKeys(blocks)).toEqual(stripKeys(parseMarkdownBlocks(text, true)));
  });

  it("逐字符追加数百次，累计解析字符量随文本线性增长而不是平方", () => {
    // 退回全量解析时，每一轮都会重解析整篇，累计量按 n² 增长；前缀冻结下它是常数倍。
    // 所以这里不钉绝对倍数，钉的是「文本翻倍，倍数不变」。
    const measure = (repeat: number) => {
      const parser = new IncrementalMarkdownBlockParser();
      const text = CORPUS.repeat(repeat);
      for (let cursor = 1; cursor <= text.length; cursor += 1) {
        parser.parse(text.slice(0, cursor));
      }
      expect(parser.stats.parses).toBe(text.length);
      return parser.stats.parsedChars / text.length;
    };

    const small = measure(2);
    const large = measure(8);
    // 文本长 4 倍，累计解析量相对文本长度的倍数不应跟着涨
    expect(large).toBeLessThan(small * 1.6);
    expect(large).toBeLessThan(80);
  });

  it("收尾 finalize 不换块身份与对象引用", () => {
    const parser = new IncrementalMarkdownBlockParser();
    const text = CORPUS.repeat(2);
    const streamed = parser.parse(text);
    const keysBefore = streamed.map((block) => block.key);
    const finalized = parser.finalize();
    // 收尾那一帧不能整篇重建：key 与对象引用都必须是流式最后一帧的那一批
    expect(finalized.map((block) => block.key)).toEqual(keysBefore);
    expect(finalized.length).toBe(streamed.length);
    expect(finalized.every((block, index) => block === streamed[index])).toBe(true);
    expect(parser.parse(text).every((block, index) => block === streamed[index])).toBe(true);
  });

  it("收尾之后继续追加，块序列仍与全量解析一致", () => {
    const parser = new IncrementalMarkdownBlockParser();
    const head = CORPUS.repeat(2);
    parser.parse(head);
    parser.finalize();
    const tail = `${head}## 追加的一节\n\n收尾之后又多了两段。\n\n`;
    const blocks = parser.parse(tail);
    expect(stripKeys(blocks)).toEqual(stripKeys(parseMarkdownBlocks(tail, true)));
  });
});
