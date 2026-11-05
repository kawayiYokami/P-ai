import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import HomeTeamMemberBlock from "./HomeTeamMemberBlock.vue";
import type { TeamMemberTileItem } from "../team-member-tile";
import { i18n } from "../../../../i18n";

async function renderBlock(
  members: TeamMemberTileItem[] = [],
  span: { colSpan?: number; rowSpan?: number } = {},
): Promise<string> {
  const app = createSSRApp({
    render: () => h(HomeTeamMemberBlock, { members, ...span }),
  });
  app.use(i18n);
  return renderToString(app);
}

/** 忙碌：rank 1 */
function busy(id: string, name: string): TeamMemberTileItem {
  return { conversationId: id, title: name, agentName: name, memberState: "busy" };
}

/** 有在办议题但没在跑：rank 2 */
function withIssue(id: string, name: string): TeamMemberTileItem {
  return {
    conversationId: id,
    title: name,
    agentName: name,
    memberState: "idle",
    activeIssue: { number: 1, title: "在办的那条", status: "in_progress" },
  };
}

/** 空闲：rank 4，排在最后但仍上表 */
function idle(id: string, name: string): TeamMemberTileItem {
  return { conversationId: id, title: name, agentName: name, memberState: "idle" };
}

/** 出错：rank 3，排在空闲之前 */
function errored(id: string, name: string): TeamMemberTileItem {
  return { conversationId: id, title: name, agentName: name, memberState: "error" };
}

describe("HomeTeamMemberBlock", () => {
  it("根元素包含 h-full 与 flex flex-col，确保吃满网格拉伸高度", async () => {
    const html = await renderBlock([]);
    expect(html).toContain("ecall-home-team-block");
    expect(html).toContain("h-full");
    expect(html).toContain("flex");
  });

  it("标题行左边是区块标题，右上是「更多」", async () => {
    const html = await renderBlock([busy("m1", "李工")]);
    expect(html).toContain("队员状态");
    expect(html).toContain("更多");
  });

  it("顶部有标题行时，工作中有队员则渲染滚动区", async () => {
    const html = await renderBlock([busy("m1", "李工"), withIssue("m2", "王工")]);
    expect(html).toContain("ecall-home-team-scroller");
    expect(html).toContain("flex-1 min-h-0 w-full");
    expect(html).toContain("李工");
    expect(html).toContain("王工");
  });

  it("全体队员都上表：空闲与出错也显示，不按工作状态筛人", async () => {
    const html = await renderBlock([
      busy("m1", "忙碌的"),
      withIssue("m2", "有议题的"),
      idle("m3", "空闲的"),
      errored("m4", "出错的"),
    ]);
    expect(html).toContain("忙碌的");
    expect(html).toContain("有议题的");
    expect(html).toContain("空闲的");
    expect(html).toContain("出错的");
  });

  it("队伍为空时才显示空文案", async () => {
    const html = await renderBlock([]);
    expect(html).toContain("ecall-home-team-block");
    expect(html).toContain("队伍里还没有队员");
    expect(html).not.toContain("ecall-home-team-scroller");
  });

  it("滚动区带全局底部渐隐类，长度走 --ecall-fade-size", async () => {
    const html = await renderBlock([busy("m1", "甲")]);
    expect(html).toContain("ecall-fade-bottom");
  });

  it("占几行几列只认宿主下发的 colSpan / rowSpan，不按人数自己涨", async () => {
    // 两个人（宿主此时会给 1 行）与三个人（宿主会给 2 行）用同一组 props 时结果必须一致，
    // 说明组件内部不再按工作人数决定高度
    const html2 = await renderBlock([busy("m1", "甲"), withIssue("m2", "乙")], { colSpan: 2, rowSpan: 1 });
    expect(html2).toContain("grid-column:span 2");
    expect(html2).toContain("grid-row:span 1");

    const html3 = await renderBlock(
      [busy("m1", "甲"), withIssue("m2", "乙"), busy("m3", "丙")],
      { colSpan: 2, rowSpan: 1 },
    );
    expect(html3).toContain("grid-row:span 1");

    const html2Rows = await renderBlock([busy("m1", "甲"), withIssue("m2", "乙")], { colSpan: 2, rowSpan: 2 });
    expect(html2Rows).toContain("grid-row:span 2");
  });

  it("按 teamMemberWorkRank 排序：仅忙碌排在仅有议题之前", async () => {
    const html = await renderBlock([withIssue("m1", "只有议题"), busy("m2", "只在忙")]);
    const posBusy = html.indexOf("只在忙");
    const posIssue = html.indexOf("只有议题");
    expect(posBusy).toBeGreaterThan(-1);
    expect(posIssue).toBeGreaterThan(-1);
    expect(posBusy).toBeLessThan(posIssue);
  });
});
