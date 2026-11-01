import { describe, expect, it } from "vitest";
import {
  KANBAN_ATTENTION_ORDER,
  KANBAN_PROGRESS_ORDER,
  KANBAN_STAGE_ORDER,
} from "./use-kanban-board";

describe("看板状态集合的分工", () => {
  it("看板时间线与磁贴泳道共用同一份状态集合", () => {
    // 看板页是议题全貌：七态一格不缺，已关闭排在链条末尾。
    expect([...KANBAN_STAGE_ORDER] as string[]).toHaveLength(7);
    expect([...KANBAN_STAGE_ORDER] as string[]).toContain("closed");

    // 磁贴进展栏放解决中、已完成、已关闭三类，顺序固定；已关闭一样是只增不减的收尾态，
    // 没有理由和已完成区别对待。
    expect([...KANBAN_PROGRESS_ORDER] as string[]).toEqual([
      "in_progress",
      "done",
      "closed",
    ]);

    // 两个磁贴泳道都是看板全貌的子集，不是与它并列的另一套状态集合：
    // 谁都不该反过来当对方的类型来源（曾因为进度栏收了 closed 而让看板页少了这个阶段）。
    const stageStatuses = [...KANBAN_STAGE_ORDER] as string[];
    for (const status of [...KANBAN_ATTENTION_ORDER, ...KANBAN_PROGRESS_ORDER]) {
      expect(stageStatuses).toContain(status);
    }
  });
});
