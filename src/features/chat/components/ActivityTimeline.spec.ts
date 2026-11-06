import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import ActivityTimeline from "./ActivityTimeline.vue";
import { i18n } from "../../../i18n";
import type { ChatActivityItem } from "../../../types/app";

type TimelineState = {
  expandedOverrides: Record<string, boolean>;
  toolResultOverrides: Record<string, string>;
  toolResultLoading: Record<string, boolean>;
  toolResultErrors: Record<string, string>;
};

function emptyState(): TimelineState {
  return {
    expandedOverrides: {},
    toolResultOverrides: {},
    toolResultLoading: {},
    toolResultErrors: {},
  };
}

function renderTimeline(items: ChatActivityItem[], state = emptyState()): Promise<string> {
  const app = createSSRApp({
    render: () => h(ActivityTimeline, { items, dark: false, state }),
  });
  app.use(i18n);
  return renderToString(app);
}

const reasoningItem: ChatActivityItem = {
  kind: "reasoning",
  id: "r1",
  text: "第一行\n第二行\n第三行\n第四行\n第五行",
};

const toolItem: ChatActivityItem = {
  kind: "tool",
  id: "t1",
  name: "read",
  argsText: JSON.stringify({ path: "/tmp/a.ts" }),
  resultText: "done",
  status: "done",
};

describe("ActivityTimeline 两行网格", () => {
  it("左列跨两行、右列标题行与内容行各自成格", async () => {
    const html = await renderTimeline([reasoningItem]);
    // 表格结构：一条 li 就是一张两行两列的表，左列跨两行
    expect(html).toContain("grid-cols-[1rem_minmax(0,1fr)]");
    expect(html).toContain("ecall-activity-rail-col");
    expect(html).toContain("ecall-activity-rail");
    expect(html).toContain("ecall-activity-title");
    expect(html).toContain("ecall-activity-body");
  });

  it("条目外层不带任何颜色类，只用 data-kind 给数据", async () => {
    const html = await renderTimeline([reasoningItem, toolItem]);
    const lis = html.match(/<li[^>]*>/g) ?? [];
    expect(lis.length).toBe(2);
    expect(lis[0]).toContain('data-kind="reasoning"');
    expect(lis[1]).toContain('data-kind="tool"');
    for (const li of lis) {
      expect(li).not.toContain("text-base-content");
      expect(li).not.toContain("text-error");
      expect(li).not.toContain("text-warning");
      expect(li).not.toContain("ecall-activity-reasoning-dark");
    }
  });

  it("标题行不写颜色类，配色交给 data-kind 那张表", async () => {
    const html = await renderTimeline([reasoningItem, toolItem]);
    const head = html.match(/<div[^>]*ecall-reasoning-head[^>]*>/)?.[0] ?? "";
    expect(head).not.toBe("");
    expect(head).not.toContain("text-base-content");
    // 标题行高度统一到 24px，图标列同高才对得齐
    expect(head).toContain("min-h-6");
    // 工具标题行同样不许自带档位色，否则又会出现一行深一行浅
    const titles = (html.match(/<div[^>]*ecall-activity-title[^>]*>/g) ?? []).filter(
      (tag) => !tag.includes("ecall-reasoning-head"),
    );
    expect(titles.length).toBeGreaterThan(0);
    for (const tag of titles) expect(tag).not.toContain("text-base-content");
  });

  it("思考内容行默认裁四行，且与标题行隔 8px", async () => {
    const html = await renderTimeline([reasoningItem]);
    const body = html.match(/<div[^>]*ecall-reasoning-text[^>]*>/)?.[0] ?? "";
    expect(body).toContain("ecall-reasoning-clamped");
    expect(body).toContain("pt-2");
  });

  it("工具条目不展开时不渲染内容行", async () => {
    const html = await renderTimeline([toolItem]);
    expect(html).toContain("ecall-activity-title");
    expect(html).not.toContain("ecall-activity-body");
  });

  it("工具条目展开后内容行落在第二行", async () => {
    const state = emptyState();
    state.expandedOverrides = { "tool:t1": true };
    const html = await renderTimeline([toolItem], state);
    const body = html.match(/<div[^>]*ecall-activity-body[^>]*>/)?.[0] ?? "";
    expect(body).not.toBe("");
    expect(body).toContain("pt-2");
  });

  it("展开态折叠节点带 is-expanded，下折箭头不靠 hover 才出现", async () => {
    const state = emptyState();
    state.expandedOverrides = { "tool:t1": true };
    const html = await renderTimeline([toolItem], state);
    const node = html.match(/<div[^>]*ecall-fold-node[^>]*>/)?.[0] ?? "";
    expect(node).toContain("is-collapsible");
    expect(node).toContain("is-expanded");
    // 三个图标同时存在，靠 is-expanded 的 CSS 只显示下折那个
    expect(html).toContain("ecall-fold-open");
    // 注：真正的病根是 v-memo 依赖里漏了展开态（点开后子树停在旧缓存，箭头不刷新）。
    // SSR 不走缓存，这条测不到那一层，只能钉住「状态类有没有正确挂上」。
  });

  it("正文条目只有标题行，没有内容行", async () => {
    const html = await renderTimeline([{ kind: "content", id: "c1", text: "工具之间的一段正文" }]);
    expect(html).toContain("ecall-activity-title");
    expect(html).not.toContain("ecall-activity-body");
  });
});
