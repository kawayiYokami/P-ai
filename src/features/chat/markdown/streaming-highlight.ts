import type { BuiltinTheme } from "shiki";

export type HighlightStreamController = {
  update: (code: string, lang: string, theme: BuiltinTheme, forceReset?: boolean) => Promise<string>;
  reset: () => void;
  dispose: () => void;
};

let shikiModulePromise: Promise<typeof import("shiki") | null> | null = null;
let streamModulePromise: Promise<typeof import("@shikijs/stream") | null> | null = null;

async function loadShiki() {
  if (!shikiModulePromise) {
    shikiModulePromise = import("shiki").catch(() => null);
  }
  return shikiModulePromise;
}

async function loadStream() {
  if (!streamModulePromise) {
    streamModulePromise = import("@shikijs/stream").catch(() => null);
  }
  return streamModulePromise;
}

export function createHighlightStream(): HighlightStreamController {
  let highlighter: Awaited<ReturnType<NonNullable<Awaited<ReturnType<typeof loadShiki>>>["getSingletonHighlighter"]>> | null = null;
  let activeTheme: BuiltinTheme | null = null;
  let activeLang: string | null = null;
  let tokenizer: any | null = null;
  let previousCode = "";
  let previousLang = "";
  let previousTheme: BuiltinTheme | null = null;

  async function ensureHighlighter(theme: BuiltinTheme, lang: string) {
    const shiki = await loadShiki();
    const streamMod = await loadStream();
    if (!shiki || !streamMod) throw new Error("shiki load failed");
    if (!highlighter || activeTheme !== theme) {
      highlighter = await shiki.getSingletonHighlighter({ langs: [], themes: [theme] });
      activeTheme = theme;
    }
    if (activeLang !== lang) {
      try {
        await (highlighter as any).loadLanguage(lang as any);
        activeLang = lang;
      } catch {
        activeLang = "plaintext";
        lang = "plaintext";
      }
    }
    return { shiki, streamMod, lang };
  }

  function createTokenizer(streamMod: any, lang: string, theme: BuiltinTheme) {
    const Cls =
      (streamMod as any).ShikiStreamTokenizer ||
      (streamMod as any).default?.ShikiStreamTokenizer ||
      (streamMod as any).default;
    if (!Cls || !highlighter) return null;
    return new Cls({
      highlighter: highlighter as any,
      lang,
      theme,
    });
  }

  function tokensToHtml(tokenizerInstance: any): string {
    try {
      const tokens = [...(tokenizerInstance.tokensStable || []), ...(tokenizerInstance.tokensUnstable || [])] as any[];
      if (tokens.length === 0) return "";
      // tokens 按行合并为 pre > code 的 innerHTML，由 @shikijs/stream 的 tokenizer 统一着色
      // 复用与 x-markdown 一致的 pre 样式：背景由外层主题控制，这里只产出 token span
      const lines: string[] = [];
      let currentLineSpans: string[] = [];
      let currentBg = "";
      let currentFg = "";
      // 从 highlighter 取主题 bg/fg 用于 preStyle（若无则空）
      try {
        const themeInfo = (highlighter as any)?.getTheme?.(previousTheme || activeTheme);
        currentBg = themeInfo?.bg || "";
        currentFg = themeInfo?.fg || "";
      } catch {}
      for (const t of tokens) {
        const content: string = t.content ?? "";
        const color: string = t.color || "";
        const bgColor: string = t.bgColor || "";
        const fontStyle: number | undefined = (t as any).fontStyle;
        const styleParts: string[] = [];
        if (color) styleParts.push(`color:${color}`);
        if (bgColor) styleParts.push(`background-color:${bgColor}`);
        if (fontStyle !== undefined) {
          if (fontStyle & 1) styleParts.push("font-style:italic");
          if (fontStyle & 2) styleParts.push("font-weight:bold");
          if (fontStyle & 4) styleParts.push("text-decoration:underline");
        }
        const style = styleParts.length ? ` style="${styleParts.join(";")}"` : "";
        const escaped = content
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;");
        if (content === "\n") {
          lines.push(currentLineSpans.join(""));
          currentLineSpans = [];
          continue;
        }
        if (!content.includes("\n")) {
          currentLineSpans.push(`<span${style}>${escaped}</span>`);
          continue;
        }
        const segs = content.split("\n");
        segs.forEach((seg, idx) => {
          if (seg) {
            const segEsc = seg.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
            currentLineSpans.push(`<span${style}>${segEsc}</span>`);
          }
          if (idx < segs.length - 1) {
            lines.push(currentLineSpans.join(""));
            currentLineSpans = [];
          }
        });
      }
      lines.push(currentLineSpans.join(""));
      const inner = lines.map((line) => `<span class="line">${line || " "}</span>`).join("\n");
      const preStyle = currentBg || currentFg ? ` style="${[currentBg ? `background-color:${currentBg}` : "", currentFg ? `color:${currentFg}` : ""].filter(Boolean).join(";")}"` : "";
      return `<pre class="shiki ${activeTheme || previousTheme || ""}"${preStyle} tabindex="0"><code>${inner}</code></pre>`;
    } catch {
      return "";
    }
  }

  async function update(code: string, lang: string, theme: BuiltinTheme, forceReset = false): Promise<string> {
    const normalizedCode = code || "";
    const normalizedLang = lang || "text";
    const needReset = forceReset || normalizedLang !== previousLang || theme !== previousTheme || !normalizedCode.startsWith(previousCode);
    if (needReset && tokenizer) {
      try { tokenizer.clear(); } catch {}
      previousCode = "";
    }
    previousLang = normalizedLang;
    previousTheme = theme;
    if (!normalizedCode) {
      previousCode = "";
      return "";
    }
    const canAppend = !needReset && normalizedCode.startsWith(previousCode);
    let chunk = normalizedCode;
    if (canAppend) chunk = normalizedCode.slice(previousCode.length);
    else if (!needReset && tokenizer) {
      try { tokenizer.clear(); } catch {}
    }
    previousCode = normalizedCode;
    if (!tokenizer || needReset) {
      const { streamMod, lang: resolvedLang } = await ensureHighlighter(theme, normalizedLang);
      tokenizer = createTokenizer(streamMod, resolvedLang, theme);
      if (!tokenizer) {
        // 回退：直接用 shiki 全量
        const shiki = await loadShiki();
        if (!shiki || !highlighter) return "";
        try {
          return await (shiki as any).codeToHtml(normalizedCode, { lang: resolvedLang, theme });
        } catch { return ""; }
      }
      previousCode = "";
      chunk = normalizedCode;
      previousCode = normalizedCode;
    }
    if (!chunk) {
      return tokensToHtml(tokenizer);
    }
    try {
      await tokenizer.enqueue(chunk);
    } catch {
      return "";
    }
    return tokensToHtml(tokenizer);
  }

  function reset() {
    try { tokenizer?.clear(); } catch {}
    tokenizer = null;
    previousCode = "";
    previousLang = "";
    previousTheme = null;
  }

  function dispose() {
    reset();
  }

  return { update, reset, dispose };
}
