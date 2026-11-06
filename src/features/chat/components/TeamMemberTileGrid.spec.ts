import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import TeamMemberTileGrid from "./TeamMemberTileGrid.vue";
import { teamMemberWorkRank, type TeamMemberTileItem } from "./team-member-tile";
import { i18n } from "../../../i18n";

/**
 * 磁贴的口径：
 * 有在办议题就写一整句「正在处理 #3 标题」，没有才显示自己的运行时三态。
 * 议题状态不再单独占一只徽章；队员名从会话绑定的 agent 取，不读会话标题。
 */
async function render(items: TeamMemberTileItem[]): Promise<string> {
  const app = createSSRApp({
    render: () => h(TeamMemberTileGrid, { members: items }),
  });
  app.use(i18n);
  return renderToString(app);
}

/**
 * 去掉 HTML 注释后的可见部分。
 * 模板注释里就写着「正在处理」「忙碌」这些词，SSR 会原样输出注释，
 * 拿整段 HTML 做 toContain / not.toContain 等于在断言注释，改一句说明就能让测试红掉。
 */
function visibleHtml(html: string): string {
  return html.replace(/<!--[\s\S]*?-->/g, "");
}

/** 只取徽章元素自己的正文 */
function badgeTexts(html: string): string[] {
  return [...visibleHtml(html).matchAll(/class="[^"]*\bbadge\b[^"]*"[^>]*>([^<]*)</g)]
    .map((match) => match[1].trim())
    .filter((text) => text.length > 0);
}

function member(overrides: Partial<TeamMemberTileItem> = {}): TeamMemberTileItem {
  return {
    conversationId: "conv-member",
    title: "李工",
    teamName: "李工",
    agentName: "李工",
    ...overrides,
  };
}

describe("TeamMemberTileGrid 状态口径", () => {
  it("名下有一条在办议题时，写一整句「正在处理 #编号 标题」", async () => {
    const html = visibleHtml(
      await render([
        member({
          activeIssue: { number: 3, title: "修复登录超时", status: "in_progress" },
          memberState: "busy",
        }),
      ]),
    );
    expect(html).toContain("正在处理 #3 修复登录超时");
    // 议题状态不再单独占一只徽章：解决中 / 待审这些词不该出现在磁贴上
    expect(html).not.toContain("解决中");
    expect(badgeTexts(html)).toEqual([]);
  });

  it("有议题但议题标题为空时，只写编号", async () => {
    const html = visibleHtml(
      await render([member({ activeIssue: { number: 7, title: "  ", status: "blocked" } })]),
    );
    expect(html).toContain("正在处理 #7");
  });

  it("没有议题时不写「正在处理」，只显示运行时状态：忙碌", async () => {
    const html = visibleHtml(await render([member({ memberState: "busy" })]));
    expect(html).not.toContain("正在处理");
    expect(badgeTexts(html)).toEqual(["忙碌"]);
  });

  it("没有议题时显示运行时状态：空闲", async () => {
    const html = await render([member({ memberState: "idle" })]);
    expect(badgeTexts(html)).toEqual(["空闲"]);
  });

  it("没有议题时显示运行时状态：出错", async () => {
    const html = await render([member({ memberState: "error" })]);
    expect(badgeTexts(html)).toEqual(["出错"]);
  });

  it("运行时状态字段缺失时按空闲兜底，不显示空徽章", async () => {
    const html = await render([member()]);
    expect(badgeTexts(html)).toEqual(["空闲"]);
  });

  it("调度进行中时最右边挂转圈，有议题也不被顶掉", async () => {
    const html = await render([
      member({
        activeIssue: { number: 3, title: "修复登录超时", status: "in_progress" },
        runtimeState: "assistant_streaming",
      }),
    ]);
    expect(html).toContain("app-loading");
  });

  it("队员名取会话绑定的 agent，不取会话标题", async () => {
    const html = visibleHtml(
      await render([member({ agentName: "李工", teamName: "李工", title: "修复登录超时" })]),
    );
    expect(html).toContain("李工");
    // 标题是内容标题，拿来当名字会让队员名随话题变
    expect(html).not.toContain("修复登录超时");
  });

  it("agent 名缺失时退回名册角色名", async () => {
    const html = visibleHtml(
      await render([member({ agentName: "", teamName: "王工", title: "补文档" })]),
    );
    expect(html).toContain("王工");
    expect(html).not.toContain("补文档");
  });

  it("teamMemberWorkRank 工作状态权重：有议题或忙碌靠前，空闲靠后", () => {
    const busyAndIssue = member({
      runtimeState: "assistant_streaming",
      activeIssue: { number: 1, title: "任务", status: "in_progress" },
    });
    const busyOnly = member({ runtimeState: "assistant_streaming" });
    const issueOnly = member({ activeIssue: { number: 2, title: "任务", status: "in_progress" } });
    const errorOnly = member({ memberState: "error" });
    const idle = member({ memberState: "idle" });

    expect(teamMemberWorkRank(busyAndIssue)).toBe(0);
    expect(teamMemberWorkRank(busyOnly)).toBe(1);
    expect(teamMemberWorkRank(issueOnly)).toBe(2);
    expect(teamMemberWorkRank(errorOnly)).toBe(3);
    expect(teamMemberWorkRank(idle)).toBe(4);
  });
});
