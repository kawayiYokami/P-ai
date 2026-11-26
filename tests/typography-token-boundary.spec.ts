import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const srcRoot = resolve(process.cwd(), "src");

// 标准令牌：text-micro(9px) text-caption(11px) text-xs(12px) text-sm(14px) text-base(16px) ...
// 任意 text-[*px] 都会绕过 use-ui-size-appearance.ts 的缩放，必须禁止。
const TEXT_ARBITRARY_PX = /text-\[\s*\d+px\s*\]/;
// 行内硬写 font-size: *px 同样绕过令牌，导出与样式都必须走 var(--app-text-*) 或 uiSizeTokensFor
const FONT_SIZE_HARD_PX = /font-size\s*:\s*\d+(\.\d+)?px/;

// 这些文件里带着「供沙箱 iframe 演示的内嵌页源码」字符串：那是被演示的独立文档，
// 拿不到宿主的 --app-text-* 变量（srcdoc 是隔离文档，没有根变量），px 必须写死。
// 只豁免 font-size 这一项，text-[*px] 仍照常禁止——两处都在同一份内嵌 HTML 的 <style> 里。
const INLINE_SANDBOX_HTML_PATHS = new Set([
  "src/features/file-reader/components/FileReaderPreviewDemo.vue",
]);

function collectFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      if (entry === "node_modules" || entry === "dist" || entry === ".pai") continue;
      out.push(...collectFiles(full));
      continue;
    }
    if (/\.(vue|ts|tsx|css)$/.test(entry)) out.push(full);
  }
  return out;
}

function findViolations(
  pattern: RegExp,
  exemptPaths: Set<string> = new Set(),
): Array<{ file: string; line: number; text: string }> {
  const files = collectFiles(srcRoot);
  const violations: Array<{ file: string; line: number; text: string }> = [];
  for (const file of files) {
    const relativeFile = relative(process.cwd(), file).replace(/\\/g, "/");
    if (exemptPaths.has(relativeFile)) continue;
    const content = readFileSync(file, "utf8");
    const lines = content.split("\n");
    lines.forEach((raw, idx) => {
      // 忽略注释中的示例： markdown-content.css 的注释会提到 16px/28px 基准，仅为文档
      if (raw.trim().startsWith("//") || raw.trim().startsWith("*")) return;
      if (pattern.test(raw)) {
        // 排除 CSS 变量回退中的 px： var(--app-text-*, 16px) 是令牌定义侧的合法回退，不算硬写
        // 但 font-size: var(--app-text-base-size, 16px) 的 16px 夹在 var() 内，pattern 本身不会命中
        // 此处仅排除 style.css 中 --app-text-* 的定义行里的 scaledPx 数字干扰，pattern 不会误判，无需额外过滤
        violations.push({
          file: relativeFile,
          line: idx + 1,
          text: raw.trim().slice(0, 200),
        });
      }
    });
  }
  return violations;
}

describe("排版令牌边界", () => {
  it("禁止 text-[*px] 硬写，必须走 text-micro/text-caption/text-xs/text-sm 等标准", () => {
    const violations = findViolations(TEXT_ARBITRARY_PX);
    expect(
      violations,
      `发现 ${violations.length} 处 text-[*px] 硬写：\n` +
        violations.map((v) => `  ${v.file}:${v.line}  ${v.text}`).join("\n") +
        `\n请改为 text-micro(9px) / text-caption(11px) / text-xs(12px) / text-sm(14px) / text-base(16px) 等标准，定义见 src/features/shell/composables/use-ui-size-appearance.ts`,
    ).toEqual([]);
  });

  it("禁止 font-size: *px 硬写，必须走 var(--app-text-*) 或 uiSizeTokensFor", () => {
    const violations = findViolations(FONT_SIZE_HARD_PX, INLINE_SANDBOX_HTML_PATHS);
    // 过滤：style.css 中 font-size: var(...) 且回退含 px 的情况已被上面的 var() 排除，剩余即为硬写
    // 若未来需要在 CSS 中新增硬写，需先在 use-ui-size-appearance.ts 增加令牌
    expect(
      violations,
      `发现 ${violations.length} 处 font-size: *px 硬写：\n` +
        violations.map((v) => `  ${v.file}:${v.line}  ${v.text}`).join("\n") +
        `\n请改为 var(--app-text-*) 或 uiSizeTokensFor(...).text*，导出场景见 src/features/chat/utils/share-export.ts 的 readShareUiSizeTokens`,
    ).toEqual([]);
  });

  it("share-export 必须通过 uiSizeTokensFor 跟随缩放，不得回退硬写", () => {
    const content = readFileSync(resolve(srcRoot, "features/chat/utils/share-export.ts"), "utf8");
    expect(content).toContain("uiSizeTokensFor");
    expect(content).toContain("readShareUiSizeTokens");
    // 确保不再出现硬写 11px/12px/14px
    expect(content).not.toMatch(/font-size:\s*11px/);
    expect(content).not.toMatch(/font-size:\s*12px/);
    expect(content).not.toMatch(/font-size:\s*14px/);
  });
});
