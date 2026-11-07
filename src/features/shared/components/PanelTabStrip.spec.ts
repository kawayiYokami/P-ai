import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import PanelTabStrip from "./PanelTabStrip.vue";
// 原文用来钉住 SSR 里构造不出来的口径；走 Vite 的 ?raw，不依赖 @types/node
import SFC_SOURCE_RAW from "./PanelTabStrip.vue?raw";
import type { PanelTabStripItem, PanelTabStripVariant } from "./panel-tab-strip.types";
import { i18n } from "../../../i18n";

/**
 * 标签行的口径（A+B 组合）：
 * - 单行恒定高度，绝不换行。标签按 flex-1 basis-0 自适应压宽，最宽 max-w-40（文件档 44）。
 * - 放不下就收进「更多」下拉菜单，不再靠换行堆行数。
 * - 「更多」按钮默认悬停即弹出（openMoreOnHover）。
 * - 下拉菜单项与标签一样带忙碌转圈。
 * - 激活项只有底色，没有描边与阴影。
 *
 * 已知限制：仓库没有 jsdom / @vue/test-utils，只能 SSR。
 * SSR 下 onMounted 不执行，可用宽度恒为 0，usePanelTabOverflow 因此走进
 * 「宽度未就绪 → 全量呈现、不出现更多按钮」的分支，
 * 所以「出现更多按钮」「菜单里某项在转圈」这两个状态在 SSR 里构造不出来，
 * 改用源码契约钉住。真实观感与几何由浏览器实测核对。
 */

const SFC_SOURCE: string = SFC_SOURCE_RAW;

function tab(key: string, overrides: Partial<PanelTabStripItem> = {}): PanelTabStripItem {
  return { key, label: key, ...overrides };
}

async function render(
  tabs: PanelTabStripItem[],
  options: { activeKey?: string; variant?: PanelTabStripVariant } = {},
): Promise<string> {
  const app = createSSRApp({
    render: () => h(PanelTabStrip, {
      tabs,
      activeKey: options.activeKey ?? "",
      variant: options.variant ?? "default",
    }),
  });
  app.use(i18n);
  return renderToString(app);
}

function tabButtons(html: string): string[] {
  return [...html.matchAll(/<button\b[^>]*\brole="tab"[^>]*>/g)].map((m) => m[0]);
}

function tabItemWrappers(html: string): string[] {
  return [...html.matchAll(/<div\b[^>]*class="[^"]*\becall-tab-item\b[^"]*"[^>]*>/g)].map((m) => m[0]);
}

function cssBlock(selector: string): string {
  const index = SFC_SOURCE.indexOf(`${selector} {`);
  if (index < 0) return "";
  const end = SFC_SOURCE.indexOf("}", index);
  return end < 0 ? "" : SFC_SOURCE.slice(index, end);
}

describe("PanelTabStrip 布局", () => {
  it("标签自适应压宽而不是固定宽度：flex-1 basis-0，各自带上限", async () => {
    const html = await render([tab("对话"), tab("设置")]);
    expect(html).toContain("flex min-w-0 flex-1 basis-0");
    expect(html).toContain("max-w-40");
  });

  it("文件档把上限放宽到 max-w-44，并换成圆角档", async () => {
    const html = await render([tab("a")], { variant: "file" });
    expect(html).toContain("max-w-44");
    expect(html).toContain("rounded-lg");
  });

  it("单行恒定高度：tablist 不换行，也不设多行高度", () => {
    expect(SFC_SOURCE).toContain('class="flex w-full min-w-0 items-center"');
    expect(SFC_SOURCE).not.toContain("flex-wrap");
    expect(SFC_SOURCE).not.toContain("max-height: calc(");
  });
});

describe("PanelTabStrip 激活项底色", () => {
  it("默认档激活按钮只加底色，不加描边", async () => {
    const html = await render([tab("对话"), tab("设置")], { activeKey: "设置" });
    const active = tabButtons(html).filter((button) => button.includes('aria-selected="true"'));
    expect(active).toHaveLength(1);
    expect(active[0]).toContain("bg-base-100/60");
    expect(active[0]).not.toContain("ring-1");
    const inactive = tabButtons(html).filter((button) => button.includes('aria-selected="false"'));
    expect(inactive[0]).not.toContain("bg-base-100");
  });

  it("文件档激活项只加底色，不加描边与阴影", async () => {
    const html = await render([tab("a"), tab("b")], { activeKey: "b", variant: "file" });
    // 文件档的底色铺在标签外壳上，按钮本身保持透明
    expect(html).toContain("cursor-pointer bg-base-100 group relative");
    expect(html).not.toContain("shadow-sm");
  });
});

describe("PanelTabStrip 溢出进下拉菜单", () => {
  it("溢出项走 EcallDropdown，不再靠换行铺开", () => {
    expect(SFC_SOURCE).toContain("<EcallDropdown");
    expect(SFC_SOURCE).toContain("v-for=\"tab in overflowTabs\"");
    expect(SFC_SOURCE).toContain("v-if=\"hasOverflow\"");
  });

  it("「更多」按钮默认悬停即弹出", () => {
    // prop 默认值必须是 true，否则用户得先点一下才知道后面还有标签
    expect(SFC_SOURCE).toContain('openMoreOnHover: true');
    expect(SFC_SOURCE).toContain(':open-on-hover="openMoreOnHover"');
  });

  it("激活项掉进溢出菜单时，更多按钮有高亮", () => {
    expect(SFC_SOURCE).toContain("activeInOverflow");
    expect(SFC_SOURCE).toContain("ring-base-content/15");
  });
});

describe("PanelTabStrip 忙碌态", () => {
  it("标签上的转圈靠右固定一格，标题再长也不挤它", async () => {
    const html = await render([tab("a", { busy: true })], { activeKey: "a" });
    expect(html).toContain("ml-auto shrink-0");
  });

  it("下拉菜单项同样显示转圈", () => {
    const menuStart = SFC_SOURCE.indexOf('v-for="tab in overflowTabs"');
    const menuEnd = SFC_SOURCE.indexOf("</EcallDropdown>");
    expect(menuStart).toBeGreaterThan(0);
    const menuSource = SFC_SOURCE.slice(menuStart, menuEnd);
    expect(menuSource).toContain("tab.busy");
    expect(menuSource).toContain("<AppLoading");
  });
});

describe("PanelTabStrip 样式契约", () => {
  it("图标有轻微描边，浅底深底都能看清", () => {
    const block = cssBlock(".panel-tab-strip-icon");
    expect(block).toContain("drop-shadow");
  });

  it("文件档标题超长走右侧淡出，不是硬截断", () => {
    expect(SFC_SOURCE).toContain("file-tab-label-fade");
    expect(cssBlock(".file-tab-label-fade")).toContain("overflow: hidden");
  });

  it("i18n 文案键仍在用", () => {
    expect(SFC_SOURCE).toContain('t("chat.moreTabs")');
    expect(i18n.global.t("chat.moreTabs")).toBeTruthy();
  });
});
