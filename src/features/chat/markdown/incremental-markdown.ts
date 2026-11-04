import { parseCodeFenceLine, parseMarkdownBlocks, type MarkdownBlock, type MarkdownBlockSpan } from "./parse-markdown";

function normalizeMarkdownText(input: string): string {
  return String(input || "").replace(/\r\n?/g, "\n");
}

function isCodeFenceLine(line: string): boolean {
  return !!parseCodeFenceLine(line);
}

function isStandaloneMathLine(line: string): boolean {
  const trimmed = line.trim();
  return (trimmed.startsWith("$$") && trimmed.length > 4 && trimmed.endsWith("$$"))
    || (trimmed.startsWith("\\[") && trimmed.length > 4 && trimmed.endsWith("\\]"));
}

function isMathFenceStartLine(line: string): boolean {
  const trimmed = line.trim();
  return trimmed === "$$"
    || trimmed.startsWith("$$")
    || trimmed === "\\["
    || trimmed.startsWith("\\[");
}

function isMathFenceEndLine(line: string): boolean {
  const trimmed = line.trim();
  return trimmed === "$$"
    || (trimmed.endsWith("$$") && !trimmed.startsWith("$$"))
    || trimmed === "\\]"
    || (trimmed.endsWith("\\]") && !trimmed.startsWith("\\["));
}

function isHeadingLine(line: string): boolean {
  return /^\s{0,3}#{1,4}\s+.+?\s*#*\s*$/.test(line);
}

function isHorizontalRuleLine(line: string): boolean {
  return /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line);
}

function isBlockquoteLine(line: string): boolean {
  return /^\s{0,3}>\s?.*$/.test(line);
}

function isListItemLine(line: string): boolean {
  return /^\s{0,3}(?:[-*+]|\d+[.)])\s+.+$/.test(line);
}

function isTableRowLine(line: string): boolean {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) return false;
  const cells = trimmed.replace(/^\|/, "").replace(/\|$/, "").split("|");
  return cells.length >= 2;
}

function isNewBlockStart(line: string): boolean {
  return isHeadingLine(line)
    || isHorizontalRuleLine(line)
    || isCodeFenceLine(line)
    || isMathFenceStartLine(line)
    || isBlockquoteLine(line)
    || isListItemLine(line);
}

function withStableKeys(blocks: MarkdownBlock[], prefix: string): MarkdownBlock[] {
  return blocks.map((block, index) => ({
    ...block,
    key: `${prefix}-${index}-${block.key}`,
  }));
}

function withoutFootnotes(blocks: MarkdownBlock[]): MarkdownBlock[] {
  return blocks.filter((block) => block.type !== "footnotes");
}

function collectFootnoteReferencesFromText(text: string, seen: Set<string>, output: string[]) {
  const pattern = /\[\^([^\]\n]+)\]/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text))) {
    const id = String(match[1] || "").trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    output.push(id);
  }
}

function collectFootnoteReferences(blocks: MarkdownBlock[]): string[] {
  const seen = new Set<string>();
  const output: string[] = [];

  for (const block of blocks) {
    if (block.type === "heading" || block.type === "paragraph" || block.type === "quote") {
      collectFootnoteReferencesFromText(block.text, seen, output);
      continue;
    }
    if (block.type === "list") {
      block.items.forEach((item) => collectFootnoteReferencesFromText(item.text, seen, output));
      continue;
    }
    if (block.type === "table") {
      block.headers.forEach((cell) => collectFootnoteReferencesFromText(cell, seen, output));
      block.rows.forEach((row) => row.forEach((cell) => collectFootnoteReferencesFromText(cell, seen, output)));
    }
  }

  return output;
}

function collectFootnoteDefinitions(text: string): Map<string, string> {
  const lines = normalizeMarkdownText(text).split("\n");
  const definitions = new Map<string, string>();

  for (let index = 0; index < lines.length; index += 1) {
    const firstLineMatch = lines[index].match(/^\s{0,3}\[\^([^\]\n]+)\]:\s*(.*)$/);
    if (!firstLineMatch) continue;

    const id = String(firstLineMatch[1] || "").trim();
    const noteLines = [String(firstLineMatch[2] || "").trimEnd()];

    while (index + 1 < lines.length) {
      const nextLine = lines[index + 1];
      if (!nextLine.trim()) {
        let hasIndentedContinuation = false;
        for (let lookahead = index + 2; lookahead < lines.length; lookahead += 1) {
          const candidate = lines[lookahead];
          if (!candidate.trim()) continue;
          hasIndentedContinuation = /^(?: {4}|\t)/.test(candidate);
          break;
        }
        if (!hasIndentedContinuation) break;
        noteLines.push("");
        index += 1;
        continue;
      }

      const continuationMatch = nextLine.match(/^(?: {4}|\t)(.*)$/);
      if (!continuationMatch) break;
      noteLines.push(String(continuationMatch[1] || "").trimEnd());
      index += 1;
    }

    if (id) definitions.set(id, noteLines.join("\n").trim());
  }

  return definitions;
}

/** 尾部保留为不稳定区的块数：追加文本最多重塑最后一个块，倒数第二个作安全余量。 */
const UNSTABLE_TAIL_BLOCKS = 2;

/** 给块打上「全文起始偏移」身份。同一个块从尾部转入冻结区时 key 不变，渲染层靠它复用而不是重建。 */
function withOffsetKey(block: MarkdownBlock, offset: number): MarkdownBlock {
  const key = `#${offset}`;
  return block.key === key ? block : { ...block, key };
}

/**
 * 追加式流式块解析器。
 *
 * 每来一条增量就把整篇重新解析一遍，在最终长度上是二次的。CommonMark 的块解析按行进行，
 * 追加文本只可能重塑解析前沿，所以除末尾 {@link UNSTABLE_TAIL_BLOCKS} 个块之外全部冻结，
 * 每轮只重解析冻结切点之后的那一小段源文本。切点取最后一个冻结块的结束偏移，
 * 块间空行留在尾部，切片是逐字原文。
 *
 * 冻结块连对象引用一起复用（不只是 key 稳定），下游的 VNode 比对才能整棵跳过。
 * 已知偏差：引用式链接与脚注定义是全文解析的，跨冻结边界会短暂显示原文，最终全量解析自愈。
 */
export class IncrementalMarkdownBlockParser {
  private text = "";
  /** 已冻结的块，对象引用跨轮次复用 */
  private frozen: MarkdownBlock[] = [];
  /** 冻结区在全文中的结束偏移，下一轮从这里开始解析 */
  private frozenEnd = 0;
  private cachedBlocks: MarkdownBlock[] = [];
  /** 已收尾：尾部块全部转为冻结，后续追加需要整篇重来 */
  private finalized = false;
  /** 解析量统计，供守卫测试钉住「每个源字符被解析的次数上界」 */
  readonly stats = { parses: 0, parsedChars: 0 };

  parse(input: string): MarkdownBlock[] {
    const text = normalizeMarkdownText(input);
    if (!text) {
      this.reset();
      return [];
    }

    if (text === this.text) return this.cachedBlocks;

    // 非追加（回退、重放、切换消息）整篇重来
    if (!text.startsWith(this.text)) this.reset();
    else if (this.finalized) {
      // 收尾之后又追加（续接、重试）：尾部已是冻结块，不能就地续接，整篇重来一次
      this.reset();
    }
    this.text = text;

    const base = this.frozenEnd;
    const slice = text.slice(base);
    const spans: MarkdownBlockSpan[] = [];
    this.stats.parses += 1;
    this.stats.parsedChars += slice.length;
    const blocks = parseMarkdownBlocks(slice, true, spans);

    // 位置缺失（fallback 单块等）时不做冻结，整段留在尾部
    const freezeCount = spans.length === blocks.length
      ? Math.max(0, blocks.length - UNSTABLE_TAIL_BLOCKS)
      : 0;

    if (freezeCount > 0) {
      const cutEnd = spans[freezeCount - 1].end;
      for (let index = 0; index < freezeCount; index += 1) {
        const block = withOffsetKey(blocks[index], base + spans[index].start);
        // 脚注块由全文重新收集后统一追加，不参与冻结
        if (block.type === "footnotes") continue;
        this.frozen.push(block);
      }
      this.frozenEnd = base + cutEnd;
    }

    const tail = blocks
      .slice(freezeCount)
      .map((block, index) => withOffsetKey(block, base + spans[freezeCount + index].start))
      .filter((block) => block.type !== "footnotes");

    this.cachedBlocks = this.withFootnotes([...this.frozen, ...tail]);
    return this.cachedBlocks;
  }

  reset(): void {
    this.text = "";
    this.frozen = [];
    this.frozenEnd = 0;
    this.cachedBlocks = [];
    this.finalized = false;
  }

  /**
   * 流式收尾：尾部未稳定块原地转为冻结，块身份（#offset）与对象引用都不换。
   * 不能退化成「全量重解析 + reset」——那一帧块身份整体换新，整篇会重建一次。
   */
  finalize(): MarkdownBlock[] {
    if (!this.text) return this.cachedBlocks;
    this.frozen = this.cachedBlocks.filter((block) => block.type !== "footnotes");
    this.frozenEnd = this.text.length;
    this.finalized = true;
    return this.cachedBlocks;
  }

  private withFootnotes(blocks: MarkdownBlock[]): MarkdownBlock[] {
    if (!this.text.includes("[^")) return blocks;
    const definitions = collectFootnoteDefinitions(this.text);
    if (definitions.size === 0) return blocks;

    const items = collectFootnoteReferences(blocks)
      .map((id) => ({ id, text: definitions.get(id) || "" }))
      .filter((item) => item.text.trim());
    if (items.length === 0) return blocks;

    return [
      ...blocks,
      {
        type: "footnotes",
        items,
        key: `footnotes-${items.map((item) => item.id).join("-")}`,
      },
    ];
  }
}
