import { describe, expect, it } from "vitest";
import { normalizeMermaidCodeForRender, patchSvgToUsePaiFontSystem } from "./MermaidBlock";

describe("normalizeMermaidCodeForRender", () => {
  it("将 LLM 输出的字面换行标记转为 Mermaid 可渲染换行", () => {
    expect(normalizeMermaidCodeForRender("A[alpha\\nBeta]"))
      .toBe("A[alpha<br/>Beta]");
    expect(normalizeMermaidCodeForRender("A[alpha\\NBeta]"))
      .toBe("A[alpha<br/>Beta]");
  });

  it("不改写 Mermaid 源码中的真实换行", () => {
    expect(normalizeMermaidCodeForRender("flowchart TD\nA --> B"))
      .toBe("flowchart TD\nA --> B");
  });
});

describe("patchSvgToUsePaiFontSystem", () => {
  it("移除外网 Google Fonts @import 并注入 PAI 自身字体体系", () => {
    const rawSvg = `<svg><style>
  @import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
  @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&display=swap');
  text { font-family: 'Inter', system-ui, sans-serif; }
  .mono { font-family: 'JetBrains Mono', 'SF Mono', 'Fira Code', ui-monospace, monospace; }
</style></svg>`;

    const patched = patchSvgToUsePaiFontSystem(rawSvg);
    expect(patched).not.toContain("@import");
    expect(patched).toContain("text { font-family: var(--app-font-family, system-ui, sans-serif);");
    expect(patched).toContain(".mono { font-family: var(--app-code-font-family); }");
  });
});
