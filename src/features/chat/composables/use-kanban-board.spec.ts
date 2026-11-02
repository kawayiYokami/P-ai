import { describe, expect, it } from "vitest";
import {
  KANBAN_ATTENTION_ORDER,
  KANBAN_PROGRESS_ORDER,
  KANBAN_STAGE_ORDER,
  kanbanDefaultGroupStatus,
} from "./use-kanban-board";

describe("看板状态集合的分工", () => {
  it("看板时间线与磁贴泳道共用同一份状态集合", () => {
    // 看板页是议题全貌：六态一格不缺，已完成排在链条末尾。
    expect([...KANBAN_STAGE_ORDER] as string[]).toHaveLength(6);
    expect([...KANBAN_STAGE_ORDER] as string[]).toContain("done");
    // 放弃态已取消：阻塞就是「不做了」，不再单列一格。
    expect([...KANBAN_STAGE_ORDER] as string[]).not.toContain("closed");

    // 磁贴进展栏放阻塞、解决中、已完成三类，顺序固定；阻塞排最前，
    // 它是推进里唯一卡住的那类。
    expect([...KANBAN_PROGRESS_ORDER] as string[]).toEqual([
      "blocked",
      "in_progress",
      "done",
    ]);

    // 两个磁贴泳道都是看板全貌的子集，不是与它并列的另一套状态集合：
    // 谁都不该反过来当对方的类型来源。
    const stageStatuses = [...KANBAN_STAGE_ORDER] as string[];
    for (const status of [...KANBAN_ATTENTION_ORDER, ...KANBAN_PROGRESS_ORDER]) {
      expect(stageStatuses).toContain(status);
    }
  });
});

describe("磁贴默认停哪个分类", () => {
  it("有待审就停待审，与待审在分类顺序里排第几无关", () => {
    // 顺序被打乱（阻塞排最前）也照样落到待审：磁贴打开先看到的是「等你通过」，
    // 不是「已经卡住」或「已经做完」。
    expect(kanbanDefaultGroupStatus([{ status: "blocked" }, { status: "in_review" }])).toBe("in_review");
    expect(kanbanDefaultGroupStatus([{ status: "todo" }, { status: "in_review" }])).toBe("in_review");
  });

  it("没有待审就退回第一个非空分类", () => {
    expect(kanbanDefaultGroupStatus([{ status: "in_progress" }, { status: "done" }])).toBe("in_progress");
    expect(kanbanDefaultGroupStatus([{ status: "done" }])).toBe("done");
  });

  it("空分类列表返回空串，不会给出一个没有内容可显示的默认", () => {
    expect(kanbanDefaultGroupStatus([])).toBe("");
  });
});
