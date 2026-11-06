import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import PanelTabStrip from "./PanelTabStrip.vue";
// 原文用来钉住 SSR 里构造不出来的口径；走 Vite 的 ?raw，不依赖 @types/node
import SFC_SOURCE_RAW from "./PanelTabStrip.vue?raw";
import type { PanelTabStripItem, PanelTabStripVariant } from "./panel-tab-strip.types";
import { i18n } from "../../../i18n";

/**
 * 标签行的口径（四轮迭代后的最终版）：
 * - 标签固定宽度，不平分；放不下就 flex-wrap 换行，不折进「更多」下拉菜单。
 * - 展开态最多 4 行高，超出在标签区内纵向滚动；尾部胶囊文案「收起」。
 * - 收起态压成一行；尾部胶囊文案「更多」。
 * - 激活项 bg-base-100 加一层极淡描边。
 *
 * 已知限制：仓库没有 jsdom / @vue/test-utils，只能 SSR。
 * SSR 下 onMounted 不执行，可用宽度恒为 0，composable 因此走进
 * 「宽度未就绪 → 全量呈现、不出现尾部胶囊」的分支，
 * 所以「换行时渲染胶囊」「收起态渲染更多」「收起只显示一行」这三个状态
 * 在 SSR 里构造不出来。这部分用下面的样式/文案契约（源码守护）钉住，
 * 真实观感与几何由浏览器实测核对。
 */

const SFC_SOURCE: string = SFC_SOURCE_RAW;

function tab(key: string, overrides: Partial<PanelTabStripItem> = {}): PanelTabStripItem {
  return { key, label: key, ...overrides };
}

async function render(
  tabs: PanelTabStripItem[],
  options: { activeKey?: string; variant?: PanelTabStripVariant; tabWidth?: number } = {},
): Promise<string> {
  const app = createSSRApp({
    render: () => h(PanelTabStrip, {
      tabs,
      activeKey: options.activeKey ?? "",
      variant: options.variant ?? "default",
      tabWidth: options.tabWidth,
    }),
  });
  app.use(i18n);
  return renderToString(app);
}

/** 只取 tablist 那一段，避免把尾部长相无关的部分算进来 */
function tabListStyle(html: string): string {
  const match = html.match(/<div[^>]*role="tablist"[^>]*style="([^"]*)"/);
  return match?.[1] ?? "";
}

function tabButtons(html: string): string[] {
  return [...html.matchAll(/<button\b[^>]*\brole="tab"[^>]*>/g)].map((m) => m[0]);
}

function tabItemWrappers(html: string): string[] {
  // 动态 class 排在静态 class 前面，ecall-tab-item 不一定是 class 的第一个词
  return [...html.matchAll(/<div\b[^>]*class="[^"]*\becall-tab-item\b[^"]*"[^>]*>/g)].map((m) => m[0]);
}

/** 取出某个 CSS 规则块的内容；找不到返回空串 */
function cssBlock(selector: string): string {
  const index = SFC_SOURCE.indexOf(`${selector} {`);
  if (index < 0) return "";
  const end = SFC_SOURCE.indexOf("}", index);
  return end < 0 ? "" : SFC_SOURCE.slice(index, end);
}

describe("PanelTabStrip 标签宽度", () => {
  it("标签等宽：宽度是按一行容量均分出来的固定值，不用 flex-grow 拉满", async () => {
    const html = await render([tab("对话"), tab("设置")]);
    // SSR 下可摆宽为 0，下发的兜底值就是换行基准宽
    expect(tabListStyle(html)).toContain("--ecall-tab-w:104px");
    expect(tabItemWrappers(html)).toHaveLength(2);
    const block = cssBlock(".ecall-tab-item");
    expect(block).toContain("flex: 0 0 auto");
    expect(block).toContain("width: var(--ecall-tab-w, 104px)");
    expect(block).toContain("max-width: 160px");
    // 宽度靠均分算出，不靠 grow：最后一行只剩一个标签时也不会被拉宽
    expect(SFC_SOURCE).toContain("Math.floor((width - (cols - 1) * gap) / cols)");
    expect(SFC_SOURCE).not.toContain("flex-basis");
    // 标签项自己不能靠 grow 铺满（label 内部的 flex 与此无关，只看标签项那条规则）
    expect(cssBlock(".ecall-tab-item")).not.toContain("flex: 1 1");
  });

  it("file 档只把上限放宽到 176，均分基准不变（否则两档一行能排几个会不一致）", async () => {
    const html = await render([tab("a"), tab("b")], { variant: "file" });
    expect(tabListStyle(html)).toContain("--ecall-tab-w:104px");
    expect(cssBlock(".ecall-tab-item--file")).toContain("max-width: 176px");
  });

  it("宿主显式传 tabWidth 时以传入值为准", async () => {
    const html = await render([tab("a")], { tabWidth: 200 });
    expect(tabListStyle(html)).toContain("--ecall-tab-w:200px");
  });
});

describe("PanelTabStrip 激活项底色", () => {
  it("默认档激活按钮是 bg-base-100 并带极淡描边", async () => {
    const html = await render([tab("对话"), tab("设置")], { activeKey: "设置" });
    const active = tabButtons(html).filter((button) => button.includes('aria-selected="true"'));
    expect(active).toHaveLength(1);
    expect(active[0]).toContain("bg-base-100");
    expect(active[0]).toContain("ring-1 ring-base-content/10");

    const inactive = tabButtons(html).filter((button) => button.includes('aria-selected="false"'));
    expect(inactive).toHaveLength(1);
    expect(inactive[0]).not.toContain("bg-base-100");
  });

  it("file 档激活块铺在标签外壳上，同样 bg-base-100 加描边", async () => {
    const html = await render([tab("a"), tab("b")], { variant: "file", activeKey: "b" });
    const active = tabItemWrappers(html).filter((wrapper) => wrapper.includes("bg-base-100"));
    expect(active).toHaveLength(1);
    expect(active[0]).toContain("ring-1 ring-base-content/10");
  });
});

describe("PanelTabStrip 溢出行为", () => {
  it("放不下时全部标签都渲染，不折进下拉菜单", async () => {
    const tabs = Array.from({ length: 9 }, (_, index) => tab(`标签${index}`));
    const html = await render(tabs, { activeKey: "标签8" });
    // 9 个标签全部在 DOM 里，顺序不变
    expect(tabItemWrappers(html)).toHaveLength(9);
    const labels = [...html.matchAll(/<span class="[^"]*whitespace-nowrap font-medium[^"]*"[^>]*>([^<]*)</g)]
      .map((m) => m[1]);
    expect(labels).toEqual(tabs.map((item) => item.label));
  });

  it("单行放得下时不渲染尾部胶囊（SSR 下可用宽度为 0，等价于不换行）", async () => {
    const html = await render([tab("对话"), tab("设置")]);
    expect(html).not.toContain("ecall-tab-pill");
  });
});

/**
 * SSR 构造不出「换行 / 收起」这两个状态，改用源码契约钉住口径：
 * 改动口径时这几条会当场变红，不会静默漂掉。
 */
describe("PanelTabStrip 展开/收起契约", () => {
  it("展开态高度写死为 4 行（行高 × 4 + 行间距 × 3），并在标签区内滚动", () => {
    const block = cssBlock(".ecall-tab-list");
    expect(block).toContain("max-height: calc(32px * 4 + 2px * 3)");
    expect(block).toContain("overflow-y: auto");
  });

  it("file 档也是 32px 行高（label 24 + padding 8），只把行间距放宽", () => {
    const block = cssBlock(".ecall-tab-list--file");
    expect(block).toContain("max-height: calc(32px * 4 + 4px * 3)");
    // 28px 这个数在 file 档从来没成立过：按它算 4 行装不下，第 4 行会被裁一半
    expect(SFC_SOURCE).not.toContain("28px * 4");
  });

  it("收起态只显示一行：标签按整行容量切片，不做 max-height 裁切", () => {
    // 裁切会把被藏的标签留在 DOM 里，Tab 键还能走到看不见的标签上
    expect(SFC_SOURCE).toContain("Math.max(1, perRowCapacity.value)");
    expect(SFC_SOURCE).toContain("props.tabs.slice(0, collapsedCapacity.value)");
    expect(SFC_SOURCE).toContain("v-for=\"tab in renderedTabs\"");
    expect(SFC_SOURCE).not.toContain("ecall-tab-list--collapsed");
  });

  it("折叠开关是纯图标 ghost 圆按钮：无文字、无底无框", () => {
    expect(SFC_SOURCE).toContain("ecall-tab-pill btn btn-ghost btn-sm btn-circle");
    // 之前是自定义实心胶囊（base-100 底 + base-300 描边），红豆要求改 ghost
    expect(cssBlock(".ecall-tab-pill")).toBe("");
    // 不带文字：文字会把第一行挤窄
    expect(SFC_SOURCE).not.toContain("whitespace-nowrap\">{{ collapsed");
  });

  it("图标区分折叠方向：展开态是向下，收起态是向右", () => {
    expect(SFC_SOURCE).toContain("collapsed ? 'chevron-right' : 'chevron-down'");
  });

  it("折叠开关和 tabTrailing 都钉在第一行右侧，且不放进滚动区里", () => {
    // 放进 tablist 会被 4 行滚动区滚走，标签多到 5 行时入口就点不到了
    expect(SFC_SOURCE).not.toContain("self-end");
    expect(SFC_SOURCE.indexOf("$slots.tabTrailing")).toBeLessThan(SFC_SOURCE.indexOf("ecall-tab-pill"));
    expect(SFC_SOURCE.indexOf("ecall-tab-pill")).toBeLessThan(SFC_SOURCE.indexOf('$slots.actions'));
  });

  it("文案仍用于 title / aria-label，所以键不能删", () => {
    expect(SFC_SOURCE).toContain(':title="collapsed ? t(\'chat.moreTabsShort\') : t(\'chat.collapseTabs\')"');
    expect(SFC_SOURCE).toContain(":aria-expanded=\"!collapsed\"");
    expect(i18n.global.t("chat.collapseTabs")).toBe("收起");
    expect(i18n.global.t("chat.moreTabsShort")).toBe("更多");
  });

  it("默认展开", () => {
    expect(SFC_SOURCE).toContain("const collapsed = ref(false)");
  });

  it("「更多」下拉菜单已随口径变更整块删除", () => {
    expect(SFC_SOURCE).not.toContain("overflowTabs");
    expect(SFC_SOURCE).not.toContain("EcallDropdown");
  });
});
