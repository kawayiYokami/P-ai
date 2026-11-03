import type { KanbanIssue, KanbanStageStatus } from "../../composables/use-kanban-board";

export type { KanbanIssue, KanbanStageStatus };

export type KanbanViewMode = "board" | "list";
export type KanbanFilterScope = "all" | "active" | "attention" | "done";

export interface KanbanStageMeta {
  status: KanbanStageStatus;
  labelKey: string;
  tone: "error" | "info" | "neutral" | "primary" | "warning" | "success" | "muted";
  dotClass: string;
  /** 纯文字状态下用色，字面量写成，避免拼接被 Tailwind 裁掉 */
  textClass: string;
  badgeClass: string;
  headerBorderClass: string;
  columnBgClass: string;
}

export const KANBAN_STAGE_CONFIGS: Record<KanbanStageStatus, KanbanStageMeta> = {
  blocked: {
    status: "blocked",
    labelKey: "chat.kanban.status.blocked",
    tone: "error",
    dotClass: "bg-error",
    textClass: "text-error",
    badgeClass: "badge-error badge-outline text-error",
    headerBorderClass: "border-error/40",
    columnBgClass: "bg-error/[0.03]",
  },
  discussing: {
    status: "discussing",
    labelKey: "chat.kanban.status.discussing",
    tone: "info",
    dotClass: "bg-info",
    textClass: "text-info",
    badgeClass: "badge-info badge-outline text-info",
    headerBorderClass: "border-info/40",
    columnBgClass: "bg-info/[0.03]",
  },
  todo: {
    status: "todo",
    labelKey: "chat.kanban.status.todo",
    tone: "neutral",
    dotClass: "bg-base-content/40",
    textClass: "text-base-content/60",
    badgeClass: "border-base-content/20 text-base-content/70",
    headerBorderClass: "border-base-content/20",
    columnBgClass: "bg-base-200/30",
  },
  in_progress: {
    status: "in_progress",
    labelKey: "chat.kanban.status.in_progress",
    tone: "primary",
    dotClass: "bg-primary",
    textClass: "text-primary",
    badgeClass: "badge-primary badge-outline text-primary",
    headerBorderClass: "border-primary/40",
    columnBgClass: "bg-primary/[0.04]",
  },
  in_review: {
    status: "in_review",
    labelKey: "chat.kanban.status.in_review",
    tone: "warning",
    dotClass: "bg-warning",
    textClass: "text-warning",
    badgeClass: "badge-warning badge-outline text-warning",
    headerBorderClass: "border-warning/50",
    columnBgClass: "bg-warning/[0.04]",
  },
  done: {
    status: "done",
    labelKey: "chat.kanban.status.done",
    tone: "success",
    dotClass: "bg-success",
    textClass: "text-success",
    badgeClass: "badge-success badge-outline text-success",
    headerBorderClass: "border-success/40",
    columnBgClass: "bg-success/[0.03]",
  },
};

export const ACTIVE_STAGE_STATUSES: readonly KanbanStageStatus[] = [
  "blocked",
  "discussing",
  "todo",
  "in_progress",
  "in_review",
];

export const ATTENTION_STAGE_STATUSES: readonly KanbanStageStatus[] = [
  "blocked",
  "in_review",
  "discussing",
];

export const DONE_STAGE_STATUSES: readonly KanbanStageStatus[] = [
  "done",
];

export const AVATAR_PALETTE = [
  "#6366f1",
  "#8b5cf6",
  "#ec4899",
  "#f59e0b",
  "#10b981",
  "#06b6d4",
  "#3b82f6",
  "#14b8a6",
];

export function getAvatarColorByName(name: string): string {
  const str = String(name || "").trim();
  if (!str) return AVATAR_PALETTE[0];
  let hash = 0;
  for (let i = 0; i < str.length; i += 1) {
    hash = (hash << 5) - hash + str.charCodeAt(i);
  }
  return AVATAR_PALETTE[Math.abs(hash) % AVATAR_PALETTE.length];
}

export function formatTimeRelative(raw?: string): string {
  const value = String(raw || "").trim();
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const diffSec = Math.floor((Date.now() - date.getTime()) / 1000);
  if (diffSec < 60) return "刚刚";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}分钟前`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `${diffHours}小时前`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}天前`;
  const month = date.getMonth() + 1;
  const day = date.getDate();
  return `${month}/${day}`;
}
