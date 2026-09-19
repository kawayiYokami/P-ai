import { describe, it, expect } from "vitest";
import { formatRecentRelativeTime, type RelativeTimeTranslate } from "./relative-time";

/** 用稳定的替身翻译，避免依赖 i18n 实例 */
const t: RelativeTimeTranslate = (key, named) => {
  const count = named?.count ?? "";
  switch (key) {
    case "config.memory.justNow":
      return "刚刚";
    case "config.memory.minutesAgo":
      return `${count} 分钟前`;
    case "config.memory.hoursAgo":
      return `${count} 小时前`;
    case "config.memory.daysAgo":
      return `${count} 天前`;
    default:
      return key;
  }
};

/** 固定「现在」：2026-09-19 20:00:00 本地时间 */
function nowMs(): number {
  return new Date(2026, 8, 19, 20, 0, 0).getTime();
}

describe("formatRecentRelativeTime", () => {
  it("空值返回空串", () => {
    expect(formatRecentRelativeTime(undefined, nowMs(), t)).toBe("");
    expect(formatRecentRelativeTime("", nowMs(), t)).toBe("");
  });

  it("非法日期原样返回", () => {
    expect(formatRecentRelativeTime("not-a-date", nowMs(), t)).toBe("not-a-date");
  });

  it("60 秒内显示刚刚", () => {
    const value = new Date(nowMs() - 30_000).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("刚刚");
  });

  it("60 分钟内显示分钟前", () => {
    const value = new Date(nowMs() - 5 * 60_000).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("5 分钟前");
  });

  it("24 小时内显示小时前", () => {
    const value = new Date(nowMs() - 3 * 3_600_000).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("3 小时前");
  });

  it("7 天内显示天前", () => {
    const value = new Date(nowMs() - 2 * 86_400_000).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("2 天前");
  });

  it("超过 7 天且同年显示 MM-DD HH:mm", () => {
    const value = new Date(2026, 3, 29, 14, 30, 0).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("04-29 14:30");
  });

  it("跨年显示 YYYY-MM-DD", () => {
    const value = new Date(2025, 11, 31, 9, 5, 0).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("2025-12-31");
  });

  it("未来时间按刚刚处理（不出现负值）", () => {
    const value = new Date(nowMs() + 60_000).toISOString();
    expect(formatRecentRelativeTime(value, nowMs(), t)).toBe("刚刚");
  });
});
