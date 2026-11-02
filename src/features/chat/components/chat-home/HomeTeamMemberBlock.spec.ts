import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import HomeTeamMemberBlock from "./HomeTeamMemberBlock.vue";
import type { TeamMemberTileItem } from "../team-member-tile";
import { i18n } from "../../../../i18n";

async function renderBlock(members: TeamMemberTileItem[] = []): Promise<string> {
  const app = createSSRApp({
    render: () => h(HomeTeamMemberBlock, { members }),
  });
  app.use(i18n);
  return renderToString(app);
}

describe("HomeTeamMemberBlock", () => {
  it("根元素包含 h-full 与 flex flex-col，确保吃满网格拉伸高度", async () => {
    const html = await renderBlock([]);
    expect(html).toContain("ecall-home-team-block");
    expect(html).toContain("h-full");
    expect(html).toContain("flex");
  });

  it("当有队员时，滚动区带有 flex-1 min-h-0 w-full，避免死高度留白", async () => {
    const html = await renderBlock([
      { conversationId: "m1", title: "欧科塔维亚", agentName: "欧科塔维亚", memberState: "idle" },
      { conversationId: "m2", title: "reviewer", agentName: "reviewer", memberState: "idle" },
    ]);
    expect(html).toContain("ecall-home-team-scroller");
    expect(html).toContain("flex-1 min-h-0 w-full");
    expect(html).toContain("欧科塔维亚");
    expect(html).toContain("reviewer");
  });

  it("0~2 名队员时为标准 1 格高（ecall-home-team-row-1）", async () => {
    const html0 = await renderBlock([]);
    expect(html0).toContain("ecall-home-team-row-1");
    expect(html0).not.toContain("ecall-home-team-row-2");

    const html2 = await renderBlock([
      { conversationId: "m1", title: "队员1", agentName: "队员1", memberState: "idle" },
      { conversationId: "m2", title: "队员2", agentName: "队员2", memberState: "idle" },
    ]);
    expect(html2).toContain("ecall-home-team-row-1");
    expect(html2).not.toContain("ecall-home-team-row-2");
  });

  it("3 名及以上队员时自适应变为 2 格高（ecall-home-team-row-2）", async () => {
    const html3 = await renderBlock([
      { conversationId: "m1", title: "队员1", agentName: "队员1", memberState: "idle" },
      { conversationId: "m2", title: "队员2", agentName: "队员2", memberState: "idle" },
      { conversationId: "m3", title: "队员3", agentName: "队员3", memberState: "idle" },
    ]);
    expect(html3).toContain("ecall-home-team-row-2");
    expect(html3).not.toContain("ecall-home-team-row-1");
  });

  it("工作中的队员（在办议题/忙碌）排在空闲队员前面", async () => {
    const html = await renderBlock([
      { conversationId: "m1", title: "空闲先招募", agentName: "空闲先招募", memberState: "idle" },
      {
        conversationId: "m2",
        title: "工作中后招募",
        agentName: "工作中后招募",
        activeIssue: { number: 1, title: "重要任务", status: "in_progress" },
      },
    ]);
    const posWork = html.indexOf("工作中后招募");
    const posIdle = html.indexOf("空闲先招募");
    expect(posWork).toBeGreaterThan(-1);
    expect(posIdle).toBeGreaterThan(-1);
    expect(posWork).toBeLessThan(posIdle);
  });
});
