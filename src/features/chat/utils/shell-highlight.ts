/**
 * shell 命令的 shiki 高亮。
 *
 * 只取 token span，不要 shiki 那层 pre 包裹与背景色：参数区的底色由卡片与毛玻璃决定，
 * 套上 shiki 的白底会把它顶掉。高亮失败或尚未就绪时调用方退回纯文本，不影响可读性。
 */
import type { BundledTheme } from "shiki";

const THEME_LIGHT: BundledTheme = "github-light";
const THEME_DARK: BundledTheme = "github-dark";

/** 命令短且重复率高，按「主题 + 原文」缓存，滚动来回不会重复 parse。 */
const CACHE_MAX = 64;
const htmlCache = new Map<string, string>();

let shikiPromise: Promise<typeof import("shiki") | null> | null = null;

function loadShiki() {
  if (!shikiPromise) shikiPromise = import("shiki").catch(() => null);
  return shikiPromise;
}

/** 把 shiki 的 `pre > code > span.line…` 压成一段用 `<br>` 分行的 inline 片段。 */
function toInlineHtml(html: string): string {
  const match = html.match(/<code[^>]*>([\s\S]*)<\/code>/);
  if (!match?.[1]) return "";
  return match[1]
    .replace(/<\/span>\n<span class="line">/g, "<br>")
    .replace(/^<span class="line">/, "")
    .replace(/<\/span>\s*$/, "");
}

/**
 * 高亮一段命令，返回可直接 v-html 的片段（内容已由 shiki 转义）。
 * 空命令、shiki 加载失败、语言不支持时一律返回空串，由调用方回退纯文本。
 */
export async function highlightShellCommand(code: string, dark: boolean): Promise<string> {
  const source = String(code ?? "");
  if (!source.trim()) return "";
  const key = `${dark ? "d" : "l"}\u0000${source}`;
  const cached = htmlCache.get(key);
  if (cached !== undefined) return cached;
  const shiki = await loadShiki();
  if (!shiki) return "";
  let html = "";
  try {
    html = await shiki.codeToHtml(source, {
      lang: "bash",
      theme: dark ? THEME_DARK : THEME_LIGHT,
    });
  } catch {
    return "";
  }
  const inline = toInlineHtml(html);
  if (!inline) return "";
  if (htmlCache.size >= CACHE_MAX) {
    const oldest = htmlCache.keys().next();
    if (!oldest.done) htmlCache.delete(oldest.value);
  }
  htmlCache.set(key, inline);
  return inline;
}
