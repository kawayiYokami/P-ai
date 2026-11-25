/**
 * 5 段标准 cron（分 时 日 月 周）的解析与生成。
 *
 * 这一层只管字符串 ↔ 结构化状态，不碰任何 UI，方便直接单测。
 * 支持的写法覆盖标准 crontab 全集：`*`、`*\/N`、单值、列表 `a,b,c`、范围 `a-b`。
 * 生成时统一收敛成 `*` / `*\/N` / `a,b,c` 三种，范围在解析阶段展开成列表，
 * 所以「用户怎么点」和「表达式长什么样」之间没有隐藏状态。
 */

/** 五个字段的键，顺序与表达式一致 */
export const CRON_FIELD_KEYS = ["minute", "hour", "day", "month", "weekday"] as const;
export type CronFieldKey = (typeof CRON_FIELD_KEYS)[number];

/** 单个字段的取值方式 */
export type CronFieldValue =
  /** `*`：不限 */
  | { kind: "any" }
  /** `*\/N`：每隔 N 个单位 */
  | { kind: "step"; step: number }
  /** `a,b,c`：指定多个值，升序且去重 */
  | { kind: "list"; values: number[] };

export type CronState = Record<CronFieldKey, CronFieldValue>;

export type CronFieldMeta = {
  key: CronFieldKey;
  /** 中文短名，界面与报错共用 */
  label: string;
  min: number;
  max: number;
  /** 该字段允许用 `*\/N` 表达；日与周不允许，因为 `*\/N` 在日/周上语义是「能被 N 整除」而不是「每隔 N」 */
  allowStep: boolean;
};

export const CRON_FIELDS: readonly CronFieldMeta[] = [
  { key: "minute", label: "分钟", min: 0, max: 59, allowStep: true },
  { key: "hour", label: "小时", min: 0, max: 23, allowStep: true },
  { key: "day", label: "日", min: 1, max: 31, allowStep: false },
  { key: "month", label: "月", min: 1, max: 12, allowStep: false },
  { key: "weekday", label: "周", min: 0, max: 6, allowStep: false },
];

export const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function isAllAny(state: CronState): boolean {
  return CRON_FIELD_KEYS.every((key) => state[key].kind === "any");
}

/** 五段全 `*` 表示「每分钟」，是合法但几乎没人想要的配置 */
export function isEveryMinute(state: CronState): boolean {
  return isAllAny(state);
}

function fieldMeta(key: CronFieldKey): CronFieldMeta {
  const meta = CRON_FIELDS.find((item) => item.key === key);
  if (!meta) throw new Error(`未知 cron 字段：${key}`);
  return meta;
}

/** 展开 `a-b` 范围，越界或倒序返回 null */
function expandRange(start: number, end: number, min: number, max: number): number[] | null {
  if (start < min || end > max || start > end) return null;
  const values: number[] = [];
  for (let value = start; value <= end; value += 1) values.push(value);
  return values;
}

/** 解析单个字段；无法识别时返回 null，由调用方决定降级策略 */
export function parseCronField(raw: string, key: CronFieldKey): CronFieldValue | null {
  const meta = fieldMeta(key);
  const text = String(raw ?? "").trim();
  if (!text) return null;
  if (text === "*") return { kind: "any" };

  // */N
  const stepMatch = /^\*\/(\d+)$/.exec(text);
  if (stepMatch) {
    const step = Number.parseInt(stepMatch[1], 10);
    if (!Number.isInteger(step) || step <= 0) return null;
    if (!meta.allowStep) return null;
    if (step > meta.max - meta.min) return null;
    return { kind: "step", step };
  }

  // 列表 / 范围 / 单值，逗号分隔
  const collected = new Set<number>();
  for (const segment of text.split(",")) {
    const piece = segment.trim();
    if (!piece) return null;
    const rangeMatch = /^(\d+)-(\d+)$/.exec(piece);
    if (rangeMatch) {
      const expanded = expandRange(
        Number.parseInt(rangeMatch[1], 10),
        Number.parseInt(rangeMatch[2], 10),
        meta.min,
        meta.max,
      );
      if (!expanded) return null;
      for (const value of expanded) collected.add(value);
      continue;
    }
    if (!/^\d+$/.test(piece)) return null;
    const value = Number.parseInt(piece, 10);
    if (value < meta.min || value > meta.max) return null;
    collected.add(value);
  }
  const values = [...collected].sort((left, right) => left - right);
  if (!values.length) return null;
  // 单值也走 list，界面上「只点了一个」和「点了一串」是同一种状态，不做特殊分支
  return { kind: "list", values };
}

/** 解析整条表达式；不是 5 段或任一段不合法时返回 null */
export function parseCronExpression(expression: string): CronState | null {
  const parts = String(expression ?? "").trim().split(/\s+/);
  if (parts.length !== CRON_FIELD_KEYS.length) return null;
  const state = {} as CronState;
  for (let index = 0; index < CRON_FIELD_KEYS.length; index += 1) {
    const key = CRON_FIELD_KEYS[index];
    const parsed = parseCronField(parts[index], key);
    if (!parsed) return null;
    state[key] = parsed;
  }
  return state;
}

/** 序列化单个字段 */
export function formatCronField(value: CronFieldValue, key: CronFieldKey): string {
  if (value.kind === "any") return "*";
  if (value.kind === "step") {
    const meta = fieldMeta(key);
    const clampedStep = Math.min(Math.max(1, Math.trunc(value.step) || 1), meta.max - meta.min);
    return `*/${clampedStep}`;
  }
  const values = [...new Set(value.values)].sort((left, right) => left - right);
  if (!values.length) return "*";
  return values.join(",");
}

/** 序列化整条表达式 */
export function formatCronExpression(state: CronState): string {
  return CRON_FIELD_KEYS.map((key) => formatCronField(state[key], key)).join(" ");
}

/** 新建时的默认状态：每天 09:00 */
export function createDefaultCronState(): CronState {
  return {
    minute: { kind: "list", values: [0] },
    hour: { kind: "list", values: [9] },
    day: { kind: "any" },
    month: { kind: "any" },
    weekday: { kind: "any" },
  };
}

/** 判断某段是否命中 */
function fieldMatches(value: CronFieldValue, candidate: number): boolean {
  if (value.kind === "any") return true;
  if (value.kind === "step") return candidate % Math.max(1, value.step) === 0;
  return value.values.includes(candidate);
}

/** 日与周同时限定时，标准 crontab 的语义是「或」，这里照实现 */
function dayMatches(state: CronState, date: Date): boolean {
  const dayAny = state.day.kind === "any";
  const weekdayAny = state.weekday.kind === "any";
  const dayHit = fieldMatches(state.day, date.getDate());
  const weekdayHit = fieldMatches(state.weekday, date.getDay());
  if (dayAny && weekdayAny) return true;
  if (dayAny) return weekdayHit;
  if (weekdayAny) return dayHit;
  return dayHit || weekdayHit;
}

/** 把一个字段展开成可枚举的取值列表 */
function expandField(value: CronFieldValue, min: number, max: number): number[] {
  const all: number[] = [];
  for (let item = min; item <= max; item += 1) all.push(item);
  if (value.kind === "any") return all;
  if (value.kind === "step") {
    const step = Math.max(1, value.step);
    return all.filter((item) => item % step === 0);
  }
  return [...value.values].sort((left, right) => left - right);
}

/**
 * 计算接下来若干次触发时间。
 *
 * 逐分钟推进太慢（按月配置要扫几十万次），这里按天粗筛 + 当天按时/分精算：
 * 先跳过日/月/周都不匹配的整天，再在命中当天找第一个匹配的时:分。
 */
export function computeNextRuns(state: CronState, from: Date, count = 3): Date[] {
  const runs: Date[] = [];
  if (count <= 0) return runs;

  const hours = expandField(state.hour, 0, 23);
  const minutes = expandField(state.minute, 0, 59);

  const cursor = new Date(from.getTime());
  cursor.setSeconds(0, 0);
  cursor.setMinutes(cursor.getMinutes() + 1);

  // 五段 cron 的最长周期是 4 年（2 月 29 日 + 闰年）；多留一年余量，扫不到就承认无解
  const limit = new Date(cursor.getTime());
  limit.setFullYear(limit.getFullYear() + 5);

  while (runs.length < count && cursor <= limit) {
    if (!fieldMatches(state.month, cursor.getMonth() + 1) || !dayMatches(state, cursor)) {
      cursor.setDate(cursor.getDate() + 1);
      cursor.setHours(0, 0, 0, 0);
      continue;
    }
    const nextHour = hours.find((hour) => hour >= cursor.getHours());
    if (nextHour === undefined) {
      cursor.setDate(cursor.getDate() + 1);
      cursor.setHours(0, 0, 0, 0);
      continue;
    }
    if (nextHour !== cursor.getHours()) {
      cursor.setHours(nextHour, 0, 0, 0);
    }
    const nextMinute = minutes.find((minute) => minute >= cursor.getMinutes());
    if (nextMinute === undefined) {
      cursor.setHours(cursor.getHours() + 1, 0, 0, 0);
      continue;
    }
    cursor.setMinutes(nextMinute, 0, 0);
    runs.push(new Date(cursor.getTime()));
    cursor.setMinutes(cursor.getMinutes() + 1);
  }

  return runs;
}

/** 用中文描述当前配置，给界面做一句可读确认 */
/**
 * 用一句人话说清当前配置。
 * 这是界面里唯一对外露出的「结果」——cron 语法本身不给用户看，
 * 所以这里必须写得像人说话，而不是把五段拼成一行。
 */
export function describeCronState(state: CronState): string {
  const clock = (hour: number, minute: number) =>
    `${String(hour).padStart(2, "0")}:${String(minute).padStart(2, "0")}`;
  const sorted = (value: CronFieldValue) => (value.kind === "list" ? [...value.values].sort((left, right) => left - right) : []);

  // 日期部分
  let dateText = "";
  if (state.weekday.kind === "list") {
    const names = sorted(state.weekday).map((value) => WEEKDAY_LABELS[value].replace(/^周/, ""));
    dateText = `每周${names.join("、")}`;
  } else if (state.day.kind === "list") {
    const days = sorted(state.day).join("、");
    dateText = state.month.kind === "list"
      ? `${sorted(state.month).join("、")} 月 ${days} 日`
      : `每月 ${days} 日`;
  } else if (state.month.kind === "list") {
    dateText = `${sorted(state.month).join("、")} 月`;
  }

  // 时间部分
  let timeText = "";
  if (state.hour.kind === "any") {
    if (state.minute.kind === "any") {
      timeText = "每分钟";
    } else if (state.minute.kind === "step") {
      timeText = state.minute.step === 1 ? "每分钟" : `每 ${state.minute.step} 分钟`;
    } else {
      timeText = `每小时的第 ${sorted(state.minute).join("、")} 分`;
    }
  } else if (state.hour.kind === "step") {
    const hourText = state.hour.step === 1 ? "每小时" : `每 ${state.hour.step} 小时`;
    if (state.minute.kind === "any") {
      timeText = hourText;
    } else if (state.minute.kind === "step") {
      timeText = `${hourText}的每 ${state.minute.step} 分钟`;
    } else {
      timeText = `${hourText}的第 ${sorted(state.minute).join("、")} 分`;
    }
  } else {
    const hours = sorted(state.hour);
    if (state.minute.kind === "list") {
      const minutes = sorted(state.minute);
      const times: string[] = [];
      for (const hour of hours) {
        for (const minute of minutes) {
          times.push(clock(hour, minute));
        }
      }
      timeText = times.length > 6
        ? `${times.slice(0, 6).join("、")} 等 ${times.length} 个时刻`
        : times.join("、");
    } else if (state.minute.kind === "step") {
      timeText = `${hours.map((hour) => String(hour).padStart(2, "0")).join("、")} 点起每 ${state.minute.step} 分钟`;
    } else {
      timeText = `${hours.map((hour) => String(hour).padStart(2, "0")).join("、")} 点的每分钟`;
    }
  }

  if (!dateText) {
    // 没有日期限定时，整点时刻才需要「每天」；否则「每 5 分钟」自己已经说清了
    return state.hour.kind === "list" ? `每天 ${timeText}` : timeText;
  }
  return `${dateText} ${timeText}`;
}

