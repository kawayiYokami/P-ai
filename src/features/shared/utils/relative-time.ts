/**
 * 相对时间文案：7 天内显示「刚刚 / N 分钟前 / N 小时前 / N 天前」，
 * 超过 7 天退化为具体日期（同年 `MM-DD HH:mm`，跨年 `YYYY-MM-DD`）。
 *
 * 文案取自 config.memory 命名空间下的通用相对时间词条，聊天与 Git 面板共用。
 */

/** 与 vue-i18n 的 t 兼容的最小签名 */
export type RelativeTimeTranslate = (key: string, named?: Record<string, unknown>) => string;

function padTimePart(value: number): string {
  return String(value).padStart(2, "0");
}

export function formatRecentRelativeTime(
  value: string | undefined,
  nowMs: number,
  t: RelativeTimeTranslate,
): string {
  const raw = String(value || "").trim();
  if (!raw) return "";
  const date = new Date(raw);
  const timestamp = date.getTime();
  if (!Number.isFinite(timestamp)) return raw;

  const diffMs = Math.max(0, nowMs - timestamp);
  const seconds = Math.floor(diffMs / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) return t("config.memory.justNow");
  if (minutes < 60) return t("config.memory.minutesAgo", { count: minutes });
  if (hours < 24) return t("config.memory.hoursAgo", { count: hours });
  if (days < 7) return t("config.memory.daysAgo", { count: days });

  const now = new Date(nowMs);
  const year = date.getFullYear();
  const monthDay = `${padTimePart(date.getMonth() + 1)}-${padTimePart(date.getDate())}`;
  const clock = `${padTimePart(date.getHours())}:${padTimePart(date.getMinutes())}`;
  if (year === now.getFullYear()) return `${monthDay} ${clock}`;
  return `${year}-${monthDay}`;
}
