import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import InlineMarkdownText from "./InlineMarkdownText.vue";

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

  it("超长文本（>12000字）自动降级为纯文本，不生成大量标签节点", async () => {
    const longText = "正常前缀 **加粗** " + "测试".repeat(7000);
    const html = await renderInlineMarkdown(longText);
    // 超过阈值后不解析 strong 标签，直接原样纯文本输出
    expect(html).not.toContain("ecall-inline-md-strong");
    expect(html).toContain("正常前缀 **加粗**");
  });

  it("CRLF 换行不残留回车，避免在 pre-wrap 容器里和 <br> 叠成双倍行距", async () => {
    const html = await renderInlineMarkdown("甲\r\n乙");
    expect(html).not.toContain("\r");
    expect(html).toContain("甲<br>乙");
  });

  it("段落空行压成单个换行：行内渲染没有段落概念", async () => {
    const html = await renderInlineMarkdown("甲\n\n乙\n\n\n丙");
    expect(html).toContain("甲<br>乙<br>丙");
    expect(html).not.toContain("<br><br>");
  });
});
