import { findNextMarkdownAutoLink } from "./markdown-auto-link";

// ==================== Markdown Block Parser ====================
// Lightweight markdown parser based on PlainMarkdownRenderer's approach,
// extended with math block detection ($$...$$ and $...$) and mermaid awareness.

export type MarkdownListItem = {
  text: string;
  marker: string;
  value?: number;
};

export type MarkdownBlock =
  | { type: "paragraph"; text: string; key: string }
  | { type: "heading"; level: 1 | 2 | 3 | 4; text: string; key: string }
  | { type: "quote"; text: string; key: string }
  | { type: "list"; ordered: boolean; items: MarkdownListItem[]; key: string }
  | { type: "table"; headers: string[]; rows: string[][]; key: string }
  | { type: "code"; lang: string; text: string; key: string }
  | { type: "embed"; src: string; key: string }
  | { type: "math"; text: string; raw: string; key: string }
  | { type: "details"; summary: string; body: string; open: boolean; key: string }
  | { type: "footnotes"; items: { id: string; text: string }[]; key: string }
  | { type: "hr"; key: string };

export type InlineSegment =
  | { type: "text"; text: string }
  | { type: "html_br" }
  | { type: "toolcall_ref"; id: string; label: string }
  | { type: "footnote_ref"; id: string }
  | { type: "code"; text: string }
  | { type: "math"; text: string; raw: string; display: boolean }
  | { type: "link"; text: string; href: string }
  | { type: "image"; alt: string; src: string }
  | { type: "imageLink"; alt: string; src: string; href: string }
  | { type: "html_sub"; children: InlineSegment[] }
  | { type: "html_sup"; children: InlineSegment[] }
  | { type: "html_kbd"; children: InlineSegment[] }
  | { type: "html_mark"; children: InlineSegment[] }
  | { type: "strong"; children: InlineSegment[] }
  | { type: "em"; children: InlineSegment[] }
  | { type: "strongEm"; children: InlineSegment[] }
  | { type: "delete"; children: InlineSegment[] };

// ==================== Block Parser ====================

export function parseCodeFenceLine(line: string): { fence: string; lang: string } | null {
  const match = String(line || "").match(/^\s{0,3}(`{3,})[ \t]*([^`]*)\s*$/);
  if (!match) return null;
  const info = String(match[2] || "").trim();
  return {
    fence: match[1] || "",
    lang: info.split(/\s+/)[0] || "",
  };
}

function pushParagraph(blocks: MarkdownBlock[], lines: string[], keyPrefix: string): MarkdownBlock | null {
  const text = lines.join("\n").trim();
  lines.length = 0;
  if (!text) return null;
  const block = { type: "paragraph" as const, text, key: `${keyPrefix}-p-${blocks.length}` };
  blocks.push(block);
  return block;
}

/** 块在源文本中的字符区间。块身份（渲染 key）取 start，跨 chunk 稳定。 */
export type MarkdownBlockSpan = { start: number; end: number };

function splitTableCells(line: string): string[] {
  // 按未被转义的竖线 | 分割，\| 保留（用于数学范数/绝对值等）
  const cells: string[] = [];
  let current = "";
  let i = 0;
  while (i < line.length) {
    const ch = line[i];
    if (ch === "\\" && line[i + 1] === "|") {
      current += "\\|";
      i += 2;
      continue;
    }
    if (ch === "|") {
      cells.push(current);
      current = "";
      i += 1;
      continue;
    }
    current += ch;
    i += 1;
  }
  cells.push(current);
  return cells;
}

function parseTableRow(line: string | undefined): string[] | null {
  const raw = String(line || "").trim();
  if (!raw.includes("|")) return null;
  const trimmed = raw.replace(/^\|/, "").replace(/\|$/, "");
  const cells = splitTableCells(trimmed).map((cell) => cell.trim());
  if (cells.length < 2) return null;
  return cells;
}

function isTableSeparator(line: string | undefined, expectedCells: number): boolean {
  const cells = parseTableRow(line);
  if (!cells || cells.length < expectedCells) return false;
  return cells.every((cell) => /^:?-{3,}:?$/.test(cell.trim()));
}

type DetailsMatch = {
  summary: string;
  body: string;
  open: boolean;
  endIndex: number;
};

function tryParseDetailsBlock(lines: string[], startIndex: number, streaming: boolean): DetailsMatch | null {
  const startLine = String(lines[startIndex] || "");
  const openMatch = startLine.match(/^\s*<details(\s+open)?\s*>\s*$/i);
  if (!openMatch) return null;
  let summary = "";
  const bodyLines: string[] = [];
  let endIndex = startIndex;
  let pendingSummary = false;
  let summaryCaptured = false;

  for (let index = startIndex + 1; index < lines.length; index += 1) {
    const line = String(lines[index] || "");
    const trimmed = line.trim();
    endIndex = index;

    if (!summaryCaptured) {
      const sameLineSummary = trimmed.match(/^<summary>(.*?)<\/summary>\s*$/i);
      if (sameLineSummary) {
        summary = String(sameLineSummary[1] || "").trim();
        summaryCaptured = true;
        continue;
      }
      if (/^<summary>\s*$/i.test(trimmed)) {
        pendingSummary = true;
        summaryCaptured = true;
        continue;
      }
    }

    if (pendingSummary) {
      const closingOnly = trimmed.match(/^(.*?)<\/summary>\s*$/i);
      if (closingOnly) {
        summary = String(closingOnly[1] || "").trim();
        pendingSummary = false;
        continue;
      }
      summary = summary ? `${summary}\n${line}` : line;
      continue;
    }

    if (/^\s*<\/details>\s*$/i.test(trimmed)) {
      return {
        summary: summary.trim(),
        body: bodyLines.join("\n").trim(),
        open: !!openMatch[1],
        endIndex: index,
      };
    }
    bodyLines.push(line);
  }

  if (!streaming) return null;
  return {
    summary: summary.trim(),
    body: bodyLines.join("\n").trim(),
    open: !!openMatch[1],
    endIndex,
  };
}

function isEscapedAt(text: string, index: number): boolean {
  let slashCount = 0;
  for (let cursor = index - 1; cursor >= 0 && text[cursor] === "\\"; cursor -= 1) {
    slashCount += 1;
  }
  return slashCount % 2 === 1;
}

function findUnescapedDelimiter(
  text: string,
  delimiter: string,
  from: number,
  state?: InlineScanState,
): number {
  const missKey = `delim:${delimiter}`;
  if (isScanMiss(state, missKey, from)) return -1;
  let cursor = Math.max(0, from);
  while (cursor < text.length) {
    const index = text.indexOf(delimiter, cursor);
    if (index < 0) break;
    if (!isEscapedAt(text, index)) return index;
    cursor = index + delimiter.length;
  }
  markScanMiss(state, missKey, from);
  return -1;
}

function normalizeMathText(value: string): string {
  return String(value || "").trim();
}

function mathBlock(raw: string, text: string, key: string): MarkdownBlock {
  return {
    type: "math",
    text: normalizeMathText(text),
    raw: String(raw || "").trim(),
    key,
  };
}

function stripBlockquoteMarker(line: string): string | null {
  const match = String(line || "").match(/^\s{0,3}>( ?)(.*)$/);
  if (!match) return null;
  return `${match[2] || ""}`;
}

type DisplayMathDelimiter = {
  open: string;
  close: string;
};

const DISPLAY_MATH_DELIMITERS: DisplayMathDelimiter[] = [
  { open: "$$", close: "$$" },
  { open: "\\[", close: "\\]" },
];

function parseDisplayMathBlockStart(
  line: string,
): { closed: boolean; text: string; raw: string; delimiter: DisplayMathDelimiter } | null {
  const rawLine = String(line || "");
  const trimmed = rawLine.trim();
  for (const delimiter of DISPLAY_MATH_DELIMITERS) {
    if (!trimmed.startsWith(delimiter.open)) continue;
    const closeIndex = findUnescapedDelimiter(trimmed, delimiter.close, delimiter.open.length);
    if (closeIndex >= 0 && !trimmed.slice(closeIndex + delimiter.close.length).trim()) {
      const text = trimmed.slice(delimiter.open.length, closeIndex);
      if (!normalizeMathText(text)) return null;
      return { closed: true, text, raw: trimmed, delimiter };
    }
    return {
      closed: false,
      text: rawLine.slice(rawLine.indexOf(delimiter.open) + delimiter.open.length),
      raw: rawLine,
      delimiter,
    };
  }
  return null;
}

export function parseMarkdownBlocks(input: string, streaming = false, spans?: MarkdownBlockSpan[]): MarkdownBlock[] {
  const normalized = String(input || "").replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const lineOffsets: number[] = [];
  {
    let offset = 0;
    for (const line of lines) {
      lineOffsets.push(offset);
      offset += line.length + 1;
    }
  }
  const result: MarkdownBlock[] = [];
  // 每块消费的行区间：块身份取它的起始字符偏移，跨 chunk 稳定
  const spanRows: Array<{ start: number; end: number }> = [];
  let blockStartLine = 0;
  const recordSpan = (endLine: number) => {
    spanRows.push({ start: blockStartLine, end: endLine });
    blockStartLine = endLine + 1;
  };
  const paragraphLines: string[] = [];
  const footnotes = new Map<string, string>();
  const referencedFootnoteIds: string[] = [];
  const referencedFootnoteIdSet = new Set<string>();
  let inCode = false;
  let codeLang = "";
  let codeLines: string[] = [];
  let inMathBlock = false;
  let mathLines: string[] = [];
  let mathRawLines: string[] = [];
  let activeMathDelimiter: DisplayMathDelimiter | null = null;
  let activeList: { ordered: boolean; items: MarkdownListItem[] } | null = null;

  const recordFootnoteRefs = (text: string) => {
    const pattern = /\[\^([^\]\n]+)\]/g;
    let match: RegExpExecArray | null;
    while ((match = pattern.exec(text))) {
      const id = String(match[1] || "").trim();
      if (!id || referencedFootnoteIdSet.has(id)) continue;
      referencedFootnoteIdSet.add(id);
      referencedFootnoteIds.push(id);
    }
  };

  const flushList = (endLine: number) => {
    if (!activeList) return;
    activeList.items.forEach((item) => recordFootnoteRefs(item.text));
    result.push({
      type: "list",
      ordered: activeList.ordered,
      items: activeList.items,
      key: `list-${result.length}`,
    });
    recordSpan(endLine);
    activeList = null;
  };

  const flushParagraph = (endLine: number) => {
    flushList(endLine);
    const block = pushParagraph(result, paragraphLines, "root");
    if (!block) return;
    if (block.type === "paragraph") recordFootnoteRefs(block.text);
    recordSpan(endLine);
  };

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex];

    if (inMathBlock) {
      mathRawLines.push(line);
      const closeIndex = activeMathDelimiter
        ? findUnescapedDelimiter(line, activeMathDelimiter.close, 0)
        : -1;
      if (closeIndex >= 0 && !line.slice(closeIndex + (activeMathDelimiter?.close.length || 0)).trim()) {
        const beforeClose = line.slice(0, closeIndex);
        if (beforeClose || mathLines.length > 0) mathLines.push(beforeClose);
        result.push(mathBlock(mathRawLines.join("\n"), mathLines.join("\n"), `math-${result.length}`));
        recordSpan(lineIndex);
        inMathBlock = false;
        mathLines = [];
        mathRawLines = [];
        activeMathDelimiter = null;
        continue;
      }
      mathLines.push(line);
      continue;
    }

    const mathStart = !inCode ? parseDisplayMathBlockStart(line) : null;
    if (mathStart) {
      flushParagraph(lineIndex - 1);
      if (mathStart.closed) {
        result.push(mathBlock(mathStart.raw, mathStart.text, `math-${result.length}`));
        recordSpan(lineIndex);
      } else {
        inMathBlock = true;
        mathLines = mathStart.text ? [mathStart.text] : [];
        mathRawLines = [mathStart.raw];
        activeMathDelimiter = mathStart.delimiter;
      }
      continue;
    }

    // Code fence
    const fenceMatch = parseCodeFenceLine(line);
    if (fenceMatch) {
      if (inCode) {
        result.push({
          type: "code",
          lang: codeLang,
          text: codeLines.join("\n"),
          key: `code-${result.length}`,
        });
        recordSpan(lineIndex);
        inCode = false;
        codeLang = "";
        codeLines = [];
      } else {
        flushParagraph(lineIndex - 1);
        inCode = true;
        codeLang = fenceMatch.lang;
        codeLines = [];
      }
      continue;
    }

    if (inCode) {
      codeLines.push(line);
      continue;
    }

    // Footnote definitions are collected and rendered together at the end.
    const footnoteMatch = line.match(/^\s{0,3}\[\^([^\]\n]+)\]:\s*(.*)$/);
    if (footnoteMatch) {
      flushParagraph(lineIndex - 1);
      const id = String(footnoteMatch[1] || "").trim();
      const noteLines = [String(footnoteMatch[2] || "").trim()];
      while (lineIndex + 1 < lines.length) {
        const continuationMatch = lines[lineIndex + 1].match(/^(?: {4}|\t)(.*)$/);
        if (!continuationMatch) break;
        noteLines.push(String(continuationMatch[1] || "").trimEnd());
        lineIndex += 1;
      }
      if (id) footnotes.set(id, noteLines.join("\n").trim());
      blockStartLine = lineIndex + 1;
      continue;
    }

    if (!line.trim()) {
      flushParagraph(lineIndex - 1);
      continue;
    }

    // 文件嵌入标签：<agent-embed src="..."> —— 指向工作区内的 HTML 沙箱文件
    const embedMatch = line.match(/^\s{0,3}<agent-embed\s+src\s*=\s*"([^"]+)"\s*\/?>\s*(?:<\/agent-embed\s*>)?\s*$/i);
    if (embedMatch) {
      flushParagraph(lineIndex - 1);
      result.push({
        type: "embed",
        src: String(embedMatch[1] || "").trim(),
        key: `embed-${result.length}`,
      });
      recordSpan(lineIndex);
      continue;
    }

    // Horizontal rule
    const hrMatch = line.match(/^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/);
    if (hrMatch) {
      flushParagraph(lineIndex - 1);
      result.push({ type: "hr", key: `hr-${result.length}` });
      recordSpan(lineIndex);
      continue;
    }

    const detailsMatch = tryParseDetailsBlock(lines, lineIndex, streaming);
    if (detailsMatch) {
      flushParagraph(lineIndex - 1);
      recordFootnoteRefs(detailsMatch.summary);
      recordFootnoteRefs(detailsMatch.body);
      result.push({
        type: "details",
        summary: detailsMatch.summary,
        body: detailsMatch.body,
        open: detailsMatch.open,
        key: `details-${result.length}`,
      });
      recordSpan(detailsMatch.endIndex);
      lineIndex = detailsMatch.endIndex;
      continue;
    }

    // Table
    const tableHeader = parseTableRow(line);
    if (tableHeader && isTableSeparator(lines[lineIndex + 1], tableHeader.length)) {
      flushParagraph(lineIndex - 1);
      lineIndex += 2;
      const rows: string[][] = [];
      while (lineIndex < lines.length) {
        const rawLine = lines[lineIndex];
        const row = parseTableRow(rawLine);
        if (row) {
          rows.push(row);
          lineIndex += 1;
          continue;
        }
        if (streaming && lineIndex === lines.length - 1 && rawLine && rawLine.trim().startsWith("|")) {
          const trimmed = rawLine.trim().replace(/^\|/, "").replace(/\|$/, "");
          const pendingCells = splitTableCells(trimmed).map((cell) => cell.trim());
          rows.push(pendingCells.length > 0 ? pendingCells : [""]);
          lineIndex += 1;
          break;
        }
        break;
      }
      lineIndex -= 1;
      tableHeader.forEach(recordFootnoteRefs);
      rows.forEach((row) => row.forEach(recordFootnoteRefs));
      result.push({
        type: "table",
        headers: tableHeader,
        rows,
        key: `table-${result.length}`,
      });
      recordSpan(lineIndex);
      continue;
    }

    // Heading
    const headingMatch = line.match(/^\s{0,3}(#{1,4})\s+(.+?)\s*#*\s*$/);
    if (headingMatch) {
      flushParagraph(lineIndex - 1);
      recordFootnoteRefs(headingMatch[2]);
      result.push({
        type: "heading",
        level: headingMatch[1].length as 1 | 2 | 3 | 4,
        text: headingMatch[2].trim(),
        key: `heading-${result.length}`,
      });
      recordSpan(lineIndex);
      continue;
    }

    // Blockquote
    const quoteLine = stripBlockquoteMarker(line);
    if (quoteLine !== null) {
      flushParagraph(lineIndex - 1);
      const quoteLines = [quoteLine];
      while (lineIndex + 1 < lines.length) {
        const nextQuoteLine = stripBlockquoteMarker(lines[lineIndex + 1]);
        if (nextQuoteLine === null) break;
        quoteLines.push(nextQuoteLine);
        lineIndex += 1;
      }
      const quoteText = quoteLines.join("\n");
      recordFootnoteRefs(quoteText);
      result.push({
        type: "quote",
        text: quoteText,
        key: `quote-${result.length}`,
      });
      recordSpan(lineIndex);
      continue;
    }

    // List item
    const listMatch = line.match(/^\s{0,3}(?:([-*+])|(\d+)[.)])\s+(.+)$/);
    if (listMatch) {
      const block = pushParagraph(result, paragraphLines, "list-before");
      if (block) {
        if (block.type === "paragraph") recordFootnoteRefs(block.text);
        recordSpan(lineIndex - 1);
      }
      const ordered = !!listMatch[2];
      if (!activeList || activeList.ordered !== ordered) {
        flushList(lineIndex - 1);
        activeList = { ordered, items: [] };
      }
      const numericValue = ordered ? Math.max(1, Number.parseInt(listMatch[2] || "1", 10) || 1) : undefined;
      activeList.items.push({
        text: listMatch[3].trim(),
        marker: ordered ? `${listMatch[2]}.` : (listMatch[1] || "-"),
        value: numericValue,
      });
      continue;
    }

    flushList(lineIndex - 1);
    paragraphLines.push(line);
  }

  // Flush remaining
  if (inCode) {
    // 流式和非流式都输出未闭合的代码块（乐观渲染）
    result.push({
      type: "code",
      lang: codeLang,
      text: codeLines.join("\n"),
      key: `code-${result.length}`,
    });
    recordSpan(lines.length - 1);
  }
  if (inMathBlock) {
    // 流式和非流式都输出未闭合的数学公式块（乐观渲染，展示已打出的公式内容）
    result.push(mathBlock(mathRawLines.join("\n"), mathLines.join("\n"), `math-${result.length}`));
    recordSpan(lines.length - 1);
  }
  flushParagraph(lines.length - 1);
  const footnoteItems = referencedFootnoteIds
    .map((id) => ({ id, text: footnotes.get(id) || "" }))
    .filter((item) => item.text.trim());
  if (footnoteItems.length > 0) {
    result.push({
      type: "footnotes",
      items: footnoteItems,
      key: `footnotes-${result.length}`,
    });
    recordSpan(lines.length - 1);
  }
  if (spans) {
    const toOffset = (lineIndex: number) => lineIndex < lineOffsets.length ? lineOffsets[lineIndex] : normalized.length;
    for (const span of spanRows) {
      spans.push({ start: toOffset(span.start), end: toOffset(span.end + 1) });
    }
  }
  return result.length > 0
    ? result
    : footnotes.size > 0
      ? []
    : [{ type: "paragraph", text: normalized, key: "fallback" }];
}

// ==================== Inline Parser ====================

const MARKDOWN_IMAGE_LINK_PATTERN = /\[!\[([^\]\n]*)\]\(([^)\n]+)\)\]\(([^)\n]+)\)/g;
const MARKDOWN_LINK_PATTERN = /!?\[([^\]\n]*)\]\(([^)\n]+)\)/g;
const TOOLCALL_REF_PATTERN = /\[toolcall:([^\]\n]+)\]/g;
const FOOTNOTE_REF_PATTERN = /\[\^([^\]\n]+)\]/g;
// 行内扫描是逐游标调用，正则提到模块级，避免每轮重新构造
const INLINE_CODE_PATTERN = /`([^`]+)`/g;
const HTML_BR_PATTERN = /<br\s*\/?>/ig;
const HTML_INLINE_TAG_PATTERN = /<(sub|sup|kbd|mark)>([\s\S]*?)<\/\1>/ig;

function trimTrailingUrlPunctuation(value: string): { href: string; trailing: string } {
  let href = value;
  let trailing = "";
  while (/[.,;:!?，。！？；：、]$/.test(href)) {
    trailing = `${href.slice(-1)}${trailing}`;
    href = href.slice(0, -1);
  }
  return { href, trailing };
}

function pushTextSegment(segments: InlineSegment[], text: string) {
  if (!text) return;
  const previous = segments[segments.length - 1];
  if (previous?.type === "text") {
    previous.text += text;
    return;
  }
  segments.push({ type: "text", text });
}

type LinkMatch =
  | { kind: "image_link"; start: number; end: number; raw: string; alt: string; src: string; href: string }
  | { kind: "markdown"; start: number; end: number; raw: string; text: string; href: string; image: boolean }
  | { kind: "auto"; start: number; end: number; href: string }
  | { kind: "toolcall_ref"; start: number; end: number; raw: string; id: string }
  | { kind: "footnote_ref"; start: number; end: number; raw: string; id: string };

function pickEarlierLink(left: LinkMatch | null, right: LinkMatch | null): LinkMatch | null {
  if (!left) return right;
  if (!right) return left;
  return left.start <= right.start ? left : right;
}

function nextMarkdownImageLink(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  if (isScanMiss(state, "mdImageLink", from)) return null;
  MARKDOWN_IMAGE_LINK_PATTERN.lastIndex = from;
  const match = MARKDOWN_IMAGE_LINK_PATTERN.exec(input);
  if (!match) {
    markScanMiss(state, "mdImageLink", from);
    return null;
  }
  return {
    kind: "image_link",
    start: match.index,
    end: match.index + match[0].length,
    raw: match[0],
    alt: match[1],
    src: match[2],
    href: match[3],
  };
}

function nextMarkdownLink(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  if (isScanMiss(state, "mdLink", from)) return null;
  MARKDOWN_LINK_PATTERN.lastIndex = from;
  const match = MARKDOWN_LINK_PATTERN.exec(input);
  if (!match) {
    markScanMiss(state, "mdLink", from);
    return null;
  }
  return {
    kind: "markdown",
    start: match.index,
    end: match.index + match[0].length,
    raw: match[0],
    text: match[1],
    href: match[2],
    image: match[0].startsWith("!"),
  };
}

function nextAutoLink(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  if (isScanMiss(state, "autoLink", from)) return null;
  const match = findNextMarkdownAutoLink(input, from);
  if (!match) {
    markScanMiss(state, "autoLink", from);
    return null;
  }
  return {
    kind: "auto",
    ...match,
  };
}

function nextToolcallRef(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  if (isScanMiss(state, "toolcallRef", from)) return null;
  TOOLCALL_REF_PATTERN.lastIndex = from;
  const match = TOOLCALL_REF_PATTERN.exec(input);
  if (!match) {
    markScanMiss(state, "toolcallRef", from);
    return null;
  }
  return {
    kind: "toolcall_ref",
    start: match.index,
    end: match.index + match[0].length,
    raw: match[0],
    id: String(match[1] || "").trim(),
  };
}

function nextFootnoteRef(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  if (isScanMiss(state, "footnoteRef", from)) return null;
  FOOTNOTE_REF_PATTERN.lastIndex = from;
  const match = FOOTNOTE_REF_PATTERN.exec(input);
  if (!match) {
    markScanMiss(state, "footnoteRef", from);
    return null;
  }
  return {
    kind: "footnote_ref",
    start: match.index,
    end: match.index + match[0].length,
    raw: match[0],
    id: String(match[1] || "").trim(),
  };
}

function toolcallRefLabel(id: string): string {
  const normalized = String(id || "").trim();
  if (!normalized) return "?";
  const numericSuffix = normalized.match(/(\d+)(?!.*\d)/);
  return numericSuffix?.[1] || normalized;
}

function normalizeMarkdownHref(rawHref: string): string {
  let href = String(rawHref || "").trim();
  if (!href) return "";
  const titleMatch = href.match(/^(.+?)\s+["'][^"']*["']$/);
  if (titleMatch) href = titleMatch[1].trim();
  if (href.startsWith("<") && href.endsWith(">")) {
    href = href.slice(1, -1).trim();
  }
  return href;
}

type EmphasisMarkerType = "strong" | "em" | "strongEm" | "delete";
type InlineSyntaxMatch =
  | { kind: "html_br"; start: number; end: number; raw: string }
  | { kind: "html_tag"; start: number; end: number; raw: string; tag: "sub" | "sup" | "kbd" | "mark"; inner: string }
  | { kind: "code"; start: number; end: number; raw: string; text: string }
  | { kind: "math"; start: number; end: number; raw: string; text: string; display: boolean }
  | { kind: "emphasis"; start: number; end: number; inner: string; emphasisType: EmphasisMarkerType }
  | LinkMatch;

type ProtectedInlineRange = { start: number; end: number };

// 行内扫描的共享状态。主循环游标只增不减，所以某类标记一旦在 from 之后确认找不到，
// 更大的 from 也不必再扫全尾——早先每轮重扫整串，整轮退化成 O(n²)。
type InlineScanState = {
  protectedRanges: ProtectedInlineRange[];
  missFrom: Map<string, number>;
};

function createInlineScanState(protectedRanges: ProtectedInlineRange[] = []): InlineScanState {
  return { protectedRanges, missFrom: new Map() };
}

function isScanMiss(state: InlineScanState | undefined, key: string, from: number): boolean {
  if (!state) return false;
  const missFrom = state.missFrom.get(key);
  return missFrom !== undefined && from >= missFrom;
}

function markScanMiss(state: InlineScanState | undefined, key: string, from: number): void {
  if (!state) return;
  const previous = state.missFrom.get(key);
  if (previous === undefined || from < previous) state.missFrom.set(key, from);
}

function pickEarlierInline(left: InlineSyntaxMatch | null, right: InlineSyntaxMatch | null): InlineSyntaxMatch | null {
  if (!left) return right;
  if (!right) return left;
  return left.start <= right.start ? left : right;
}

// 区间由 collectProtectedInlineRanges 按游标递增产出，天然有序且互不重叠，可以二分
function isIndexInProtectedRange(index: number, ranges: ProtectedInlineRange[]): boolean {
  let low = 0;
  let high = ranges.length - 1;
  while (low <= high) {
    const mid = (low + high) >> 1;
    const range = ranges[mid];
    if (index < range.start) high = mid - 1;
    else if (index >= range.end) low = mid + 1;
    else return true;
  }
  return false;
}

function collectProtectedInlineRanges(text: string): ProtectedInlineRange[] {
  const ranges: ProtectedInlineRange[] = [];
  const state = createInlineScanState(ranges);
  let cursor = 0;
  while (cursor < text.length) {
    const match = nextCodeOrMath(text, cursor, state);
    if (!match) break;
    ranges.push({ start: match.start, end: match.end });
    cursor = Math.max(match.end, match.start + 1);
  }
  return ranges;
}

function findMarkerOutsideProtectedRanges(
  text: string,
  marker: string,
  from: number,
  state: InlineScanState,
): number {
  const missKey = `em:${marker}`;
  if (isScanMiss(state, missKey, from)) return -1;
  const protectedRanges = state.protectedRanges;
  let cursor = from;
  while (cursor < text.length) {
    const index = text.indexOf(marker, cursor);
    if (index < 0) break;
    if (!isIndexInProtectedRange(index, protectedRanges)) return index;
    cursor = index + marker.length;
  }
  markScanMiss(state, missKey, from);
  return -1;
}

// protectedRanges 由调用方在入口算一次传进来：它只与整串文本有关，和 from 无关。
function findNextInlineMarker(
  text: string,
  from: number,
  state: InlineScanState,
): { type: EmphasisMarkerType; start: number; end: number; inner: string } | null {
  const patterns: Array<{ type: EmphasisMarkerType; marker: string }> = [
    { type: "strongEm", marker: "***" },
    { type: "delete", marker: "~~" },
    { type: "strong", marker: "**" },
    { type: "em", marker: "*" },
  ];
  let best: { type: EmphasisMarkerType; start: number; end: number; inner: string } | null = null;
  for (const pattern of patterns) {
    const start = findMarkerOutsideProtectedRanges(text, pattern.marker, from, state);
    if (start < 0) continue;
    if (pattern.marker === "*" && text[start + 1] === "*") continue;
    const contentStart = start + pattern.marker.length;
    const endMarker = findMarkerOutsideProtectedRanges(text, pattern.marker, contentStart, state);
    if (endMarker < 0 || endMarker === contentStart) continue;
    const candidate = {
      type: pattern.type,
      start,
      end: endMarker + pattern.marker.length,
      inner: text.slice(contentStart, endMarker),
    };
    if (!best || candidate.start < best.start || (candidate.start === best.start && candidate.end > best.end)) {
      best = candidate;
    }
  }
  return best;
}

function nextInlineCode(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  if (isScanMiss(state, "code", from)) return null;
  const codePattern = INLINE_CODE_PATTERN;
  codePattern.lastIndex = from;
  const match = codePattern.exec(input);
  if (!match) {
    markScanMiss(state, "code", from);
    return null;
  }
  return {
    kind: "code",
    start: match.index,
    end: match.index + match[0].length,
    raw: match[0],
    text: match[1],
  };
}

function inlineMathCanRender(text: string): boolean {
  const content = normalizeMathText(text);
  if (!content) return false;
  return !/[\r\n]/.test(content);
}

function nextInlineParenMath(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  if (isScanMiss(state, "parenMath", from)) return null;
  let cursor = Math.max(0, from);
  while (cursor < input.length) {
    const start = findUnescapedDelimiter(input, "\\(", cursor, state);
    if (start < 0) break;
    const contentStart = start + 2;
    const end = findUnescapedDelimiter(input, "\\)", contentStart, state);
    if (end < 0) break;
    const raw = input.slice(start, end + 2);
    const text = input.slice(contentStart, end);
    if (!inlineMathCanRender(text)) {
      cursor = start + 2;
      continue;
    }
    return {
      kind: "math",
      start,
      end: end + 2,
      raw,
      text,
      display: false,
    };
  }
  markScanMiss(state, "parenMath", from);
  return null;
}

function nextInlineMath(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  const dollarMath = nextInlineDollarMath(input, from, state);
  const parenMath = nextInlineParenMath(input, from, state);
  return pickEarlierInline(dollarMath, parenMath);
}

function nextInlineDollarMath(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  if (isScanMiss(state, "dollarMath", from)) return null;
  let cursor = Math.max(0, from);
  while (cursor < input.length) {
    const start = findUnescapedDelimiter(input, "$", cursor, state);
    if (start < 0) break;
    if (input[start + 1] === "$" || input[start - 1] === "$") {
      cursor = start + 1;
      continue;
    }
    const contentStart = start + 1;
    const end = findUnescapedDelimiter(input, "$", contentStart, state);
    if (end < 0) break;
    if (input[end + 1] === "$") {
      cursor = start + 1;
      continue;
    }
    const raw = input.slice(start, end + 1);
    const text = input.slice(contentStart, end);
    if (!inlineMathCanRender(text)) {
      cursor = start + 1;
      continue;
    }
    return {
      kind: "math",
      start,
      end: end + 1,
      raw,
      text,
      display: false,
    };
  }
  markScanMiss(state, "dollarMath", from);
  return null;
}

function nextCodeOrMath(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  return pickEarlierInline(nextInlineCode(input, from, state), nextInlineMath(input, from, state));
}

function nextAllowedHtml(input: string, from: number, state?: InlineScanState): InlineSyntaxMatch | null {
  const brPattern = HTML_BR_PATTERN;
  brPattern.lastIndex = from;
  const brMatch = isScanMiss(state, "htmlBr", from) ? null : brPattern.exec(input);
  if (!brMatch) markScanMiss(state, "htmlBr", from);

  const tagPattern = HTML_INLINE_TAG_PATTERN;
  tagPattern.lastIndex = from;
  const tagMatch = isScanMiss(state, "htmlTag", from) ? null : tagPattern.exec(input);
  if (!tagMatch) markScanMiss(state, "htmlTag", from);

  const brCandidate = brMatch
    ? {
      kind: "html_br" as const,
      start: brMatch.index,
      end: brMatch.index + brMatch[0].length,
      raw: brMatch[0],
    }
    : null;
  const tagCandidate = tagMatch
    ? {
      kind: "html_tag" as const,
      start: tagMatch.index,
      end: tagMatch.index + tagMatch[0].length,
      raw: tagMatch[0],
      tag: tagMatch[1].toLowerCase() as "sub" | "sup" | "kbd" | "mark",
      inner: tagMatch[2],
    }
    : null;

  return pickEarlierInline(brCandidate, tagCandidate);
}

function nextLinkLike(input: string, from: number, state?: InlineScanState): LinkMatch | null {
  const imageLink = nextMarkdownImageLink(input, from, state);
  const markdownLink = nextMarkdownLink(input, from, state);
  const autoLink = nextAutoLink(input, from, state);
  const toolcallRef = nextToolcallRef(input, from, state);
  const footnoteRef = nextFootnoteRef(input, from, state);
  return pickEarlierLink(
    pickEarlierLink(pickEarlierLink(pickEarlierLink(imageLink, markdownLink), autoLink), toolcallRef),
    footnoteRef,
  );
}

function nextEmphasis(input: string, from: number, state: InlineScanState): InlineSyntaxMatch | null {
  const matched = findNextInlineMarker(input, from, state);
  if (!matched) return null;
  return {
    kind: "emphasis",
    start: matched.start,
    end: matched.end,
    inner: matched.inner,
    emphasisType: matched.type,
  };
}

function emitInlineSyntaxMatch(match: InlineSyntaxMatch, segments: InlineSegment[]) {
  if (match.kind === "html_br") {
    segments.push({ type: "html_br" });
    return;
  }
  if (match.kind === "html_tag") {
    const children = parseInlineSegments(match.inner);
    if (match.tag === "sub") {
      segments.push({ type: "html_sub", children });
      return;
    }
    if (match.tag === "sup") {
      segments.push({ type: "html_sup", children });
      return;
    }
    if (match.tag === "kbd") {
      segments.push({ type: "html_kbd", children });
      return;
    }
    segments.push({ type: "html_mark", children });
    return;
  }
  if (match.kind === "code") {
    segments.push({ type: "code", text: match.text });
    return;
  }
  if (match.kind === "math") {
    segments.push({ type: "math", text: normalizeMathText(match.text), raw: match.raw, display: match.display });
    return;
  }
  if (match.kind === "emphasis") {
    segments.push({
      type: match.emphasisType,
      children: parseInlineSegments(match.inner),
    } as InlineSegment);
    return;
  }
  if (match.kind === "image_link") {
    const src = normalizeMarkdownHref(match.src);
    const href = normalizeMarkdownHref(match.href);
    if (src && href) {
      segments.push({ type: "imageLink", src, href, alt: match.alt });
    } else {
      pushTextSegment(segments, match.raw);
    }
    return;
  }
  if (match.kind === "markdown") {
    const href = normalizeMarkdownHref(match.href);
    if (href) {
      if (match.image) {
        segments.push({ type: "image", src: href, alt: match.text });
      } else {
        segments.push({ type: "link", href, text: match.text });
      }
    } else {
      pushTextSegment(segments, match.raw);
    }
    return;
  }
  if (match.kind === "auto") {
    const { href, trailing } = trimTrailingUrlPunctuation(match.href);
    if (href) segments.push({ type: "link", href, text: href });
    pushTextSegment(segments, trailing);
    return;
  }
  if (match.kind === "toolcall_ref") {
    segments.push({
      type: "toolcall_ref",
      id: match.id,
      label: toolcallRefLabel(match.id),
    });
    return;
  }
  if (match.id) {
    segments.push({ type: "footnote_ref", id: match.id });
  } else {
    pushTextSegment(segments, match.raw);
  }
}

/**
 * Parse inline text into segments, handling: inline code, inline math ($...$),
 * markdown links, auto-links, bold, italic, bold-italic, strikethrough.
 */
export function parseInlineSegments(input: string): InlineSegment[] {
  const segments: InlineSegment[] = [];
  const text = String(input || "");
  // 受保护区间（行内代码 / 行内公式）扫一遍即可；游标只增不减，扫描状态随之共享，
  // 各类标记「已确认不存在」之后不再重扫全尾。
  const state = createInlineScanState(collectProtectedInlineRanges(text));
  let cursor = 0;

  while (cursor < text.length) {
    const next = pickEarlierInline(
      pickEarlierInline(
        pickEarlierInline(nextCodeOrMath(text, cursor, state), nextAllowedHtml(text, cursor, state)),
        nextLinkLike(text, cursor, state),
      ),
      nextEmphasis(text, cursor, state),
    );
    if (!next) break;
    pushTextSegment(segments, text.slice(cursor, next.start));
    emitInlineSyntaxMatch(next, segments);
    cursor = next.end;
  }
  pushTextSegment(segments, text.slice(cursor));
  return segments;
}

export function normalizedTableRow(row: string[], size: number): string[] {
  return Array.from({ length: Math.max(1, size) }, (_item, index) => row[index] || "");
}
