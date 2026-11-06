/**
 * MCP 握手 instructions 的展示裁剪。
 *
 * instructions 常常是服务端写的一大段契约说明（授权范围、资源约定、用法禁忌），
 * 卡片副标题只有一行，只取第一句并限长；要看全文去服务端文档，不在卡片里铺开。
 */

/** 卡片副标题的字符上限：中文一行大约 60~80 字，超出会挤掉名称与状态。 */
export const MCP_INSTRUCTIONS_SUMMARY_MAX_CHARS = 72;

/** 取第一句：中文句号/问号/叹号、换行、或英文句点后接空白，先到者为准。 */
export function firstSentenceOfInstructions(text: string): string {
  const normalized = text.replace(/\r\n/g, "\n").trim();
  if (!normalized) return "";

  let end = normalized.length;
  for (const marker of ["。", "！", "？", "\n"]) {
    const index = normalized.indexOf(marker);
    if (index >= 0 && index < end) end = index;
  }
  // 英文句点只在后面跟空白或位于结尾时才算句子边界，避免把 URL、版本号切断。
  const englishStop = normalized.search(/\.(\s|$)/);
  if (englishStop >= 0 && englishStop < end) end = englishStop;

  return normalized.slice(0, end).trim();
}

/** 卡片副标题用的摘要：第一句 + 限长，空输入返回空串（调用方据此回落地址/命令）。 */
export function summarizeMcpInstructions(
  text: string,
  maxChars: number = MCP_INSTRUCTIONS_SUMMARY_MAX_CHARS,
): string {
  const collapsed = text.trim();
  if (!collapsed) return "";

  const firstSentence = firstSentenceOfInstructions(collapsed);
  const base = firstSentence || collapsed.replace(/\s+/g, " ").trim();
  if (base.length <= maxChars) return base;
  return `${base.slice(0, maxChars).trimEnd()}...`;
}
