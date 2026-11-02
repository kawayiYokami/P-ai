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

  it("当无队员时，展示空提示", async () => {
    const html = await renderBlock([]);
    expect(html).toContain("队伍里还没有队员");
  });
});
