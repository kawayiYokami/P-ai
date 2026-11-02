import { describe, expect, it } from "vitest";
import { createSSRApp, h, provide } from "vue";
import { renderToString } from "vue/server-renderer";
import AppMarkdownRenderer from "./AppMarkdownRenderer.vue";
import { LOCALE_MESSAGES, i18n } from "../../../i18n";
import { CHAT_HTML_READER_KEY, type ChatHtmlReader } from "../utils/chat-html-reader";

async function renderMarkdown(text: string): Promise<string> {
  const app = createSSRApp({
    render: () => h(AppMarkdownRenderer, { text, streaming: false }),
  });
  app.use(i18n);
  return renderToString(app);
}

describe("AppMarkdownRenderer", () => {
  it("renders display math blocks nested inside blockquotes", async () => {
    const html = await renderMarkdown([
      "> The force of interest, \\( \\delta(t) \\), is a function of time ...",
      ">",
      "> \\[",
      "> \\delta(t)=",
      "> \\begin{cases}",
      "> 0.07-0.005t, & t\\le 8,\\\\",
      "> 0.06, & t>8.",
      "> \\end{cases}",
      "> \\]",
      ">",
      "> Calculate the present value ...",
    ].join("\n"));

    expect(html).toContain("ecall-md-quote");
    expect(html).toContain("ecall-md-math-block-shell");
    expect(html).toContain("\\delta(t)=");
    expect(html).toContain("Calculate the present value");
  });

  it("groups adjacent toolcall badges into one wrench with count", async () => {
    const html = await renderMarkdown("前文[toolcall:a][toolcall:b] [toolcall:c] 后文");
    expect((html.match(/data-toolcall-pill="true"/g) || []).length).toBe(1);
    expect(html).toContain("ecall-md-toolcall-ref-count");
    expect(html).toContain("+3");
    expect(html).toContain("data-toolcall-id=\"a\"");
    expect(html).toContain("toolcall:a\ntoolcall:b\ntoolcall:c");
  });

  it("groups consecutive marker-only paragraphs into one wrench", async () => {
    const html = await renderMarkdown("[toolcall:a][toolcall:b]\n\n[toolcall:c]\n\n后面正文");
    expect((html.match(/data-toolcall-pill="true"/g) || []).length).toBe(1);
    expect(html).toContain("+3");
    expect(html).toContain("后面正文");
  });

  it("groups tools across blank lines between paragraphs", async () => {
    const html = await renderMarkdown("前文 [toolcall:a][toolcall:b]\n\n[toolcall:c]\n\n[toolcall:d] 后文");
    expect((html.match(/data-toolcall-pill="true"/g) || []).length).toBe(1);
    expect(html).toContain("+4");
    expect(html).toContain("前文");
    expect(html).toContain("后文");
  });

  it("shows the target line for local file links", async () => {
    const html = await renderMarkdown("[README.md](E:/github/easy_call_ai/README.md:57)");
    expect(html).toContain(">README.md:57</a>");
    expect(html).toContain("data-href=\"E:/github/easy_call_ai/README.md:57\"");
  });

  it("does not attach animation words to quote block during streaming", async () => {
    const app = createSSRApp({
      render: () => h(AppMarkdownRenderer, {
        text: "> 已经渲染好的引用文本\n\n后续正文内容正在流式输出",
        streaming: true,
      }),
    });
    app.use(i18n);
    const html = await renderToString(app);
    expect(html).toContain("ecall-md-quote");
    expect(html).toContain("已经渲染好的引用文本");
    const quoteHtml = html.match(/<blockquote[\s\S]*?<\/blockquote>/)?.[0] || "";
    expect(quoteHtml).not.toContain("ecall-md-animate-word");
  });

  it("renders html:interactive blocks as sandboxed iframe instead of code block", async () => {
    const html = await renderMarkdown([
      "```html:interactive",
      "<!DOCTYPE html><html><body><button>点我</button></body></html>",
      "```",
    ].join("\n"));
    expect(html).toContain("ecall-sandbox-card");
    expect(html).toContain("iframe");
    expect(html).toContain("sandbox=");
    expect(html).not.toContain("ecall-md-code-block");
  });

  it("renders plain html blocks as normal code blocks", async () => {
    const html = await renderMarkdown("```html\n<div>x</div>\n```");
    expect(html).toContain("ecall-md-code-block");
  });

  it("renders agent-embed tag as embed block", async () => {
    const html = await renderMarkdown('<agent-embed src="demo.html"></agent-embed>');
    expect(html).toContain("ecall-agent-embed");
  });

  it("renders unfinished math blocks during streaming without discarding content", async () => {
    const app = createSSRApp({
      render: () => h(AppMarkdownRenderer, {
        text: "前文说明\n\n$$\nE = mc^2",
        streaming: true,
      }),
    });
    app.use(i18n);
    const html = await renderToString(app);
    expect(html).toContain("前文说明");
    expect(html).toContain("E = mc^2");
  });
});

// ==================== 交互组件「在右栏打开」 ====================

const INTERACTIVE_HTML = [
  "```html:interactive",
  "<!DOCTYPE html><html><body><button id=\"count\">点我</button></body></html>",
  "```",
].join("\n");

const READER_LABEL = "在右栏打开";

async function renderMarkdownWithHtmlReader(
  text: string,
  options: { streaming?: boolean; reader?: ChatHtmlReader | null } = {},
): Promise<string> {
  const app = createSSRApp({
    setup() {
      provide(CHAT_HTML_READER_KEY, options.reader ?? null);
      return () => h(AppMarkdownRenderer, { text, streaming: !!options.streaming });
    },
  });
  app.use(i18n);
  return renderToString(app);
}

function readerButtonCount(html: string): number {
  return (html.match(/aria-label="在右栏打开"/g) || []).length;
}

describe("交互组件卡片「在右栏打开」入口", () => {
  it("聊天侧提供能力时，工具栏在原有按钮之外多出一个入口", async () => {
    const html = await renderMarkdownWithHtmlReader(INTERACTIVE_HTML, {
      reader: { label: READER_LABEL, open: () => {} },
    });
    // 原有按钮照旧
    expect(html).toContain("title=\"导出为 HTML\"");
    expect(html).toContain("title=\"复制代码\"");
    // 新入口是追加，不是替换
    expect(readerButtonCount(html)).toBe(1);
  });

  it("右栏自己渲染时拿不到能力，卡片上不出现这个按钮", async () => {
    const html = await renderMarkdownWithHtmlReader(INTERACTIVE_HTML, { reader: null });
    expect(html).toContain("ecall-sandbox-card");
    expect(html).toContain("title=\"导出为 HTML\"");
    expect(readerButtonCount(html)).toBe(0);
  });

  it("流式期间还是骨架卡，不给入口", async () => {
    const html = await renderMarkdownWithHtmlReader(INTERACTIVE_HTML, {
      streaming: true,
      reader: { label: READER_LABEL, open: () => {} },
    });
    expect(html).toContain("正在编写交互组件");
    expect(html).not.toContain("ecall-sandbox-card");
    expect(readerButtonCount(html)).toBe(0);
  });

  it("三份 locale 都补了按钮文案与面板标签标题", () => {
    for (const messages of Object.values(LOCALE_MESSAGES)) {
      const chat = (messages as { chat: Record<string, unknown> }).chat;
      expect(String(chat.openInRightPanel ?? "").trim()).not.toBe("");
      expect(String(chat.interactiveHtmlTitle ?? "").trim()).not.toBe("");
    }
  });
});
