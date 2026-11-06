/**
 * 行内 Markdown 的轻量解析：只认行内代码、粗体、斜体、删除线。
 *
 * 为什么不复用 parse-markdown 的 parseInlineSegments：
 * 那个解析器服务完整渲染器，要同时处理公式、链接、图片、HTML 标签、脚注、
 * 工具调用引用。它会先把整段扫一遍收集「受保护区间」，再让四路匹配器逐个
 * 找位置；对思维链这种以纯文本为主、动辄十几万字的输入，绝大部分算力花在
 * 永不出现的语法上。
 *
 * 这里单趟扫描：遇到标记才切段，段内的标记直接当文本，不回头重扫。
 * 不处理的语法（链接、图片、公式、HTML 标签）原样当文本输出，符合
 * 「紧凑预览」的定位——这类内容本来就不该在行内预览里展开成结构。
 */

export type LightInlineSegment =
  | { type: "text"; text: string }
  | { type: "html_br" }
  | { type: "code"; text: string }
  | { type: "strong"; children: LightInlineSegment[] }
  | { type: "em"; children: LightInlineSegment[] }
  | { type: "strongEm"; children: LightInlineSegment[] }
  | { type: "delete"; children: LightInlineSegment[] };

/** 标记按长度从长到短排列，保证 `***` 先于 `**`、`**` 先于 `*` 被匹配到 */
const INLINE_MARKER_PATTERN = /`|\*\*\*|\*\*|~~|\*/g;
/** 嵌套强调的上限：防止畸形输入把递归压爆栈，超过就当纯文本 */
const MAX_NESTING_DEPTH = 6;

function pushText(segments: LightInlineSegment[], text: string): void {
  if (!text) return;
  const prev = segments[segments.length - 1];
  if (prev && prev.type === "text") {
    prev.text += text;
    return;
  }
  segments.push({ type: "text", text });
}

/**
 * 找配对标记。闭标记前一个字符不能是空白，否则 `2 * 3 = 6` 里的乘号
 * 会被当成斜体开标记，把后面的内容整段吃掉。
 */
function findClosingMarker(text: string, marker: string, from: number): number {
  let index = text.indexOf(marker, from);
  while (index >= 0) {
    const before = text[index - 1];
    if (before !== undefined && !/\s/.test(before)) return index;
    index = text.indexOf(marker, index + 1);
  }
  return -1;
}

function buildSegment(marker: string, inner: string, depth: number): LightInlineSegment {
  if (marker === "`") return { type: "code", text: inner };
  const children = parseLightInline(inner, depth + 1);
  if (marker === "***") return { type: "strongEm", children };
  if (marker === "**") return { type: "strong", children };
  if (marker === "~~") return { type: "delete", children };
  return { type: "em", children };
}

export function parseLightInline(input: string, depth = 0): LightInlineSegment[] {
  const text = String(input || "");
  const segments: LightInlineSegment[] = [];
  if (!text) return segments;
  if (depth >= MAX_NESTING_DEPTH) {
    pushText(segments, text);
    return segments;
  }

  let cursor = 0;
  while (cursor < text.length) {
    INLINE_MARKER_PATTERN.lastIndex = cursor;
    const match = INLINE_MARKER_PATTERN.exec(text);
    if (!match) {
      pushText(segments, text.slice(cursor));
      break;
    }

    const marker = match[0];
    const openIndex = match.index;
    const innerStart = openIndex + marker.length;

    // 行内代码内部可以是任意字符；强调标记后面紧跟空白说明它不是标记
    if (marker !== "`" && (innerStart >= text.length || /\s/.test(text[innerStart]))) {
      pushText(segments, text.slice(cursor, innerStart));
      cursor = innerStart;
      continue;
    }

    const closeIndex = findClosingMarker(text, marker, innerStart);
    // 没配对，或者配对出来是空内容（`**` 连写的情况）：当前标记当文本，只前进一个标记长度
    if (closeIndex <= innerStart) {
      pushText(segments, text.slice(cursor, innerStart));
      cursor = innerStart;
      continue;
    }

    pushText(segments, text.slice(cursor, openIndex));
    segments.push(buildSegment(marker, text.slice(innerStart, closeIndex), depth));
    cursor = closeIndex + marker.length;
  }

  return segments;
}

const HEADING_LINE_PATTERN = /^\s{0,3}#{1,6}\s+(.*)$/;

/**
 * 把整段文本切成行内段，顺带处理两件行内渲染特有的事：
 *
 * - 标题行剥掉 `#` 前缀当粗体，行内渲染不生成块级标题；
 * - 连续换行压成一个。行内渲染没有段落概念，`\n\n` 只会渲染成一整行空白。
 */
export function parseLightInlineText(input: string): LightInlineSegment[] {
  const segments: LightInlineSegment[] = [];
  const lines = String(input || "")
    .replace(/\r\n?/g, "\n")
    .replace(/\n{2,}/g, "\n")
    .split("\n");

  lines.forEach((line, index) => {
    if (index > 0) segments.push({ type: "html_br" });
    const heading = line.match(HEADING_LINE_PATTERN);
    if (heading) {
      const children = parseLightInline(heading[1]);
      if (children.length > 0) {
        segments.push({ type: "strong", children });
        return;
      }
    }
    segments.push(...parseLightInline(line));
  });

  // 相邻的 strong 段之间如果没有文本/空白分隔，插一个换行让它们各占一行——
  // 思维链流式输出常是 "**Step1****Step2****Step3**" 这种无间隔格式，
  // 直接渲染会连成一片不可读。
  const spaced: LightInlineSegment[] = [];
  for (const seg of segments) {
    const prev = spaced[spaced.length - 1];
    if (seg.type === "strong" && prev?.type === "strong") {
      spaced.push({ type: "html_br" });
    }
    spaced.push(seg);
  }
  return spaced;
}
