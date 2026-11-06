import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import InlineMarkdownText from "./InlineMarkdownText.vue";
import { parseLightInlineText } from "./inline-light-markdown";

async function renderInlineMarkdown(text: string): Promise<string> {
  const app = createSSRApp({
    render: () => h(InlineMarkdownText, { text }),
  });
  return renderToString(app);
}

describe("InlineMarkdownText", () => {
  it("正常长度解析行内 Markdown 格式", async () => {
    const html = await renderInlineMarkdown("这是**加粗**与`代码`");
    expect(html).toContain("ecall-inline-md-strong");
    expect(html).toContain("加粗");
    expect(html).toContain("ecall-inline-md-code");
    expect(html).toContain("代码");
  });

  it("超长文本不降级，标记照常解析", async () => {
    const longText = "正常前缀 **加粗** " + "测试".repeat(7000);
    const html = await renderInlineMarkdown(longText);
    expect(html).toContain("ecall-inline-md-strong");
    expect(html).toContain("加粗");
  });

  it("CRLF 换行不残留回车，避免在 pre-wrap 容器里和 <br> 叠成双倍行距", async () => {
    const html = await renderInlineMarkdown("甲\r\n乙");
    expect(html).not.toContain("\r");
    expect(html).toContain("甲<br>乙");
  });

  it("段落空行切成段间距，不渲染成整行空行", async () => {
    const html = await renderInlineMarkdown("甲\n\n乙\n\n\n丙");
    expect(html).toContain("ecall-inline-md-para-gap");
    expect(html).not.toContain("<br><br>");
    // 空行不再退化成换行：甲与乙之间不该有 <br>
    expect(html).not.toContain("甲<br>乙");
  });

  it("段内单换行仍渲染成换行", async () => {
    const html = await renderInlineMarkdown("甲\n乙");
    expect(html).toContain("甲<br>乙");
    expect(html).not.toContain("ecall-inline-md-para-gap");
  });

  it("链接、图片、公式不进行内渲染，原样当文本", async () => {
    const html = await renderInlineMarkdown("[文字](https://example.com) 与 $x^2$ 与 ![图](a.png)");
    expect(html).not.toContain("<a");
    expect(html).not.toContain("<img");
    expect(html).toContain("[文字](https://example.com)");
  });

  it("乘号不被当成斜体开标记", async () => {
    const html = await renderInlineMarkdown("2 * 3 = 6，另有 *斜体* 正常生效");
    expect(html).toContain("2 * 3 = 6");
    expect(html).toContain("ecall-inline-md-em");
    expect(html).toContain("斜体");
  });

  it("十八万字单趟解析完成，段结构不退化", () => {
    const unit = "用户问：能查到记忆吗？**加粗** 与 `owner_agent_id`\n- 第一项\n\n";
    const longText = unit.repeat(Math.ceil(180_000 / unit.length));
    const segments = parseLightInlineText(longText);
    expect(segments.length).toBeGreaterThan(0);
    expect(segments.some((segment) => segment.type === "strong")).toBe(true);
    expect(segments.some((segment) => segment.type === "code")).toBe(true);
  });
});
