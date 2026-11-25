/**
 * 定时器 DTO：与后端 `TimerRecord` 一一对应（后端按 camelCase 序列化）。
 *
 * 定时器只有一句话：什么时候、在哪个会话、把这句话再递进去。
 * 没有标题、进度、待办与结论文档——那些是随着旧 task 体系一起退场的账本字段。
 */
import { describeCronState, parseCronExpression } from "../../shared/components/cron-picker";

export type TimerDispatchMode = "direct" | "delegate";

export type TimerEntry = {
  id: string;
  message: string;
  /** 目标会话；`null` 表示全局定时器（只能在系统通知会话里起委托）。 */
  conversationId: string | null;
  dispatchMode: TimerDispatchMode;
  enabled: boolean;
  /** 起始时间 / 单次触发时间（本地 RFC3339）。 */
  runAt: string;
  /** 标准 5 段 cron；`null` 表示只触发一次。 */
  cron: string | null;
  endAt: string | null;
  lastTriggeredAt: string | null;
  nextRunAt: string | null;
  createdAt: string;
};

export type TimerCreateInput = {
  message: string;
  /** 目标会话；省略或空串＝全局定时器（没有前台会话可激活，后端只允许 delegate）。 */
  conversationId?: string | null;
  dispatchMode?: TimerDispatchMode;
  runAt?: string | null;
  cron?: string | null;
  endAt?: string | null;
};

export type TimerUpdateInput = {
  timerId: string;
  message?: string;
  conversationId?: string | null;
  dispatchMode?: TimerDispatchMode;
  enabled?: boolean;
  runAt?: string;
  cron?: string | null;
  endAt?: string | null;
};

/** 本地 RFC3339 → 界面可读时间；空值返回空串，解析不了就原样返回。 */
export function formatTimerTime(value?: string | null): string {
  const text = (value ?? "").trim();
  if (!text) {
    return "";
  }
  const parsed = new Date(text);
  if (Number.isNaN(parsed.getTime())) {
    return text;
  }
  return parsed.toLocaleString(undefined, { hour12: false });
}

/** 调度频率的人话描述；一次性返回空串，文案由调用方补。不暴露 cron 原文。 */
export function timerScheduleDescription(timer: TimerEntry): string {
  const cron = String(timer.cron ?? "").trim();
  if (!cron) {
    return "";
  }
  const state = parseCronExpression(cron);
  return state ? describeCronState(state) : "";
}

/**
 * 还会再跑＝进行中；停用、到期或单次已过＝已经结束。
 *
 * 侧栏列表与设置页列表都靠它决定状态图标，两边判定必须一致，
 * 否则同一个定时器会在两个地方显示成两种状态。
 */
export function timerIsFinished(timer: TimerEntry): boolean {
  if (!timer.enabled) {
    return true;
  }
  const endAt = String(timer.endAt || "").trim();
  if (endAt) {
    const endedAt = new Date(endAt).getTime();
    if (Number.isFinite(endedAt) && endedAt <= Date.now()) {
      return true;
    }
  }
  if (!String(timer.cron || "").trim()) {
    const runAt = new Date(String(timer.runAt || "").trim()).getTime();
    if (Number.isFinite(runAt) && runAt <= Date.now()) {
      return true;
    }
  }
  return false;
}
