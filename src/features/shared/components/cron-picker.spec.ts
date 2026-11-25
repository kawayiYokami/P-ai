import { describe, expect, it } from "vitest";
import {
  computeNextRuns,
  createDefaultCronState,
  describeCronState,
  formatCronExpression,
  parseCronExpression,
} from "./cron-picker";

describe("cron 表达式解析", () => {
  it("解析 5 段标准写法并原样还原", () => {
    const cases = [
      "* * * * *",
      "0 9 * * *",
      "*/5 * * * *",
      "30 */2 * * *",
      "0 9 * * 1,3,5",
      "0 9 1,15 * *",
      "0 9 * 6,12 *",
    ];
    for (const expression of cases) {
      const parsed = parseCronExpression(expression);
      expect(parsed, expression).not.toBeNull();
      expect(formatCronExpression(parsed!), expression).toBe(expression);
    }
  });

  it("范围会展开成列表", () => {
    const parsed = parseCronExpression("0 9 * * 1-5");
    expect(parsed?.weekday).toEqual({ kind: "list", values: [1, 2, 3, 4, 5] });
    expect(formatCronExpression(parsed!)).toBe("0 9 * * 1,2,3,4,5");
  });

  it("拒绝段数不对、越界与非法步长", () => {
    expect(parseCronExpression("0 9 * *")).toBeNull();
    expect(parseCronExpression("0 9 * * * *")).toBeNull();
    expect(parseCronExpression("60 9 * * *")).toBeNull();
    expect(parseCronExpression("0 24 * * *")).toBeNull();
    expect(parseCronExpression("0 9 * * 7")).toBeNull();
    expect(parseCronExpression("*/0 * * * *")).toBeNull();
    // 日与周不支持 */N：标准语义是「能被 N 整除」，不是「每隔 N」，容易误读
    expect(parseCronExpression("0 0 */2 * *")).toBeNull();
    expect(parseCronExpression("abc 9 * * *")).toBeNull();
  });
});

describe("下次执行时间", () => {
  it("每天 09:00 从当天早上算出今天与明天", () => {
    const state = createDefaultCronState();
    const from = new Date(2026, 0, 5, 8, 0, 0);
    const runs = computeNextRuns(state, from, 2);
    expect(runs.map((run) => `${run.getMonth() + 1}-${run.getDate()} ${run.getHours()}:${String(run.getMinutes()).padStart(2, "0")}`))
      .toEqual(["1-5 9:00", "1-6 9:00"]);
  });

  it("已经过了今天的时间点时从明天开始", () => {
    const state = createDefaultCronState();
    const from = new Date(2026, 0, 5, 10, 30, 0);
    const runs = computeNextRuns(state, from, 1);
    expect(runs[0].getDate()).toBe(6);
    expect(runs[0].getHours()).toBe(9);
  });

  it("只在指定星期触发", () => {
    const parsed = parseCronExpression("0 9 * * 1")!;
    // 2026-01-05 是周一
    const runs = computeNextRuns(parsed, new Date(2026, 0, 4, 12, 0, 0), 3);
    expect(runs.every((run) => run.getDay() === 1)).toBe(true);
    expect(runs.map((run) => run.getDate())).toEqual([5, 12, 19]);
  });

  it("每隔 5 分钟逐次推进", () => {
    const parsed = parseCronExpression("*/5 * * * *")!;
    const runs = computeNextRuns(parsed, new Date(2026, 0, 5, 8, 1, 0), 3);
    expect(runs.map((run) => run.getMinutes())).toEqual([5, 10, 15]);
  });
});

describe("可读描述", () => {
  it("每天单点时间写成时刻", () => {
    expect(describeCronState(createDefaultCronState())).toBe("每天 09:00");
  });

  it("工作日写成每周加星期", () => {
    const parsed = parseCronExpression("0 9 * * 1,2,3,4,5")!;
    expect(describeCronState(parsed)).toBe("每周一、二、三、四、五 09:00");
  });

  it("固定间隔不该拖上「每天」", () => {
    expect(describeCronState(parseCronExpression("*/5 * * * *")!)).toBe("每 5 分钟");
    expect(describeCronState(parseCronExpression("30 */2 * * *")!)).toBe("每 2 小时的第 30 分");
  });

  it("多个时刻并列写出来", () => {
    const parsed = parseCronExpression("0,30 9,18 * * *")!;
    expect(describeCronState(parsed)).toBe("每天 09:00、09:30、18:00、18:30");
  });

  it("按月按日写成每月几号", () => {
    expect(describeCronState(parseCronExpression("0 0 1,15 * *")!)).toBe("每月 1、15 日 00:00");
  });
});
