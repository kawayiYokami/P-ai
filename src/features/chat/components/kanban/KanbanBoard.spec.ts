import { describe, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import KanbanBoard from "./KanbanBoard.vue";
import type { KanbanIssue } from "./kanban-types";
import { i18n } from "../../../../i18n";

const MOCK_ISSUES: KanbanIssue[] = [
  {
    number: 1,
    title: "演练：议题承载一次完整协作",
    status: "done",
    assignee: "leader",
    note: "",
    createdAt: "2026-11-02T10:00:00Z",
  },
  {
    number: 2,
    title: "实现多端统一看板组件",
    status: "in_progress",
    assignee: "member:工程师",
    note: "前端多列与单列自适应",
    planPath: "docs/plan.md",
    createdAt: "2026-11-02T11:00:00Z",
  },
  {
    number: 3,
    title: "代码审查与验收",
    status: "in_review",
    assignee: "member:reviewer",
    note: "等待确认",
    createdAt: "2026-11-02T12:00:00Z",
  },
  {
    number: 4,
    title: "网络依赖阻塞",
    status: "blocked",
    assignee: "none",
    note: "代理未就绪",
    createdAt: "2026-11-02T13:00:00Z",
  },
];

async function renderBoard(props: Partial<InstanceType<typeof KanbanBoard>["$props"]> = {}): Promise<string> {
  const app = createSSRApp({
    render: () => h(KanbanBoard, { issues: MOCK_ISSUES, ...props }),
  });
  app.use(i18n);
  return renderToString(app);
}

describe("KanbanBoard 独立组件", () => {
  it("默认渲染看板标题、统计数与泳道列", async () => {
    const html = await renderBoard({ showBack: true, leaderName: "纳西妲" });
    expect(html).toContain("议题看板");
    expect(html).toContain("共 4 条");
    expect(html).toContain("演练：议题承载一次完整协作");
    expect(html).toContain("实现多端统一看板组件");
  });

  it("正确识别待审与阻塞的紧急指标胶囊", async () => {
    const html = await renderBoard({ leaderName: "纳西妲" });
    expect(html).toContain("1 待审");
    expect(html).toContain("1 阻塞");
  });

  it("负责人映射：队长映射为队长人格名，未分配正常展示", async () => {
    const html = await renderBoard({ leaderName: "纳西妲" });
    expect(html).toContain("纳西妲");
    expect(html).toContain("未分配");
    expect(html).toContain("工程师");
    expect(html).toContain("reviewer");
  });

  it("挂接计划文件的议题渲染计划文件名", async () => {
    const html = await renderBoard();
    expect(html).toContain("plan.md");
  });

  it("待审核议题渲染通过按钮", async () => {
    const html = await renderBoard();
    expect(html).toContain("通过");
  });

  it("支持渲染清单列表视图 (initialViewMode: 'list')", async () => {
    const html = await renderBoard({ initialViewMode: "list" });
    expect(html).toContain("演练：议题承载一次完整协作");
    expect(html).toContain("实现多端统一看板组件");
    expect(html).toContain("代码审查与验收");
  });

  it("空列表时展示全局友好空态", async () => {
    const app = createSSRApp({
      render: () => h(KanbanBoard, { issues: [] }),
    });
    app.use(i18n);
    const html = await renderToString(app);
    expect(html).toContain("这块看板还没有议题");
  });
});
