import { ref } from "vue";
import type { KanbanIssue } from "./use-kanban-board";
import { KANBAN_STAGE_CONFIGS, type KanbanStageStatus } from "../components/kanban/kanban-types";

/**
 * 挂进输入框的议题引用。
 *
 * 只存引用要用的四个字段，不复用 `IdeContextReferenceItem`——那个结构是文件/代码位置专用的，
 * 塞议题进去会让「文件路径、行号、语言」这些字段全部变成空壳。
 */
export type KanbanIssueReference = {
  number: number;
  title: string;
  status: string;
  note: string;
};

/** `#` 触发的词元范围：start 是 `#` 的位置，end 是光标位置。 */
export type KanbanIssueTrigger = {
  start: number;
  end: number;
  query: string;
};

/** 状态文案与 `chat.kanban.status.*` 同义：这段是发给模型读的正文，不跟界面语言走。 */
const KANBAN_STATUS_TEXT: Record<string, string> = {
  planning: "规划中",
  discussing: "待议",
  todo: "待办",
  in_progress: "解决中",
  in_review: "待审",
  blocked: "阻塞",
  done: "已完成",
};

export function kanbanIssueStatusText(status: string): string {
  const key = String(status || "").trim();
  return KANBAN_STATUS_TEXT[key] || key;
}

/** 状态文字色：与看板泳道同源取色，未知状态回落中性灰。
 *  类名必须是字面量，拼接出来的 Tailwind 扫不到。 */
export function kanbanIssueStatusClass(status: string): string {
  const key = String(status || "").trim() as KanbanStageStatus;
  return KANBAN_STAGE_CONFIGS[key]?.textClass || "text-base-content/60";
}

export function toKanbanIssueReference(issue: KanbanIssue): KanbanIssueReference {
  return {
    number: Number(issue.number || 0),
    title: String(issue.title || "").trim(),
    status: String(issue.status || "").trim(),
    note: String(issue.note || "").trim(),
  };
}

/** 输入框 chip 与消息引用块共用的标签：`#3 标题`。 */
export function buildKanbanIssueReferenceLabel(reference: KanbanIssueReference): string {
  const number = Number(reference.number || 0);
  const title = String(reference.title || "").trim();
  return title ? `#${number} ${title}` : `#${number}`;
}

/**
 * 发给模型的附加文本：首行必须是标签本身。
 * 消息里的引用块按首行推显示名（`extraTextReferenceDisplayParts`），所以首行不能写别的。
 */
export function buildKanbanIssueReferenceText(reference: KanbanIssueReference): string {
  const lines = [buildKanbanIssueReferenceLabel(reference)];
  const status = kanbanIssueStatusText(reference.status);
  if (status) lines.push(`状态：${status}`);
  const note = String(reference.note || "").trim();
  if (note) lines.push(`备注：${note}`);
  return lines.join("\n");
}

/**
 * 光标处是否正在输入 `#` 词元。
 *
 * 触发条件是前一个字符为 `#` 且它在行首或空白之后——`abc#3` 这类紧贴文字的不算，
 * 免得正常书写里的井号莫名弹出面板。返回的 start 用于选中后把整段词元吃掉。
 */
export function findKanbanIssueTrigger(
  text: string,
  cursor: number,
): KanbanIssueTrigger | null {
  const value = String(text || "");
  const position = Math.max(0, Math.min(value.length, Number(cursor) || 0));
  const beforeCursor = value.slice(0, position);
  const match = beforeCursor.match(/(?:^|\s)#([^\s#]*)$/);
  if (!match) return null;
  const query = match[1];
  return {
    start: position - 1 - query.length,
    end: position,
    query,
  };
}

/** 只列当前会话的议题；纯数字按编号前缀匹配，其余按标题包含匹配。 */
export function filterKanbanIssueOptions(
  entries: KanbanIssue[],
  query: string,
): KanbanIssue[] {
  const list = Array.isArray(entries) ? entries : [];
  const keyword = String(query || "").trim().toLowerCase();
  if (!keyword) return list;
  const numeric = /^\d+$/.test(keyword);
  return list.filter((issue) => {
    const number = String(Number(issue.number || 0));
    const title = String(issue.title || "").toLowerCase();
    if (numeric && number.startsWith(keyword)) return true;
    return title.includes(keyword);
  });
}

/** 议题引用列表的组序：按需要用户看的优先级排——要拍板的在最前，已完成垫底。
    只用于输入面板里 `#` 弹出的那份列表，不参与首页磁贴与看板页的任何排序。 */
export const KANBAN_ISSUE_LIST_ORDER = [
  "in_review",
  "discussing",
  "blocked",
  "todo",
  "in_progress",
  "done",
] as const;

/** 按组序排、组内编号倒序（最新在上）。顺序表外的状态排在最后。 */
export function sortKanbanIssueOptions(entries: KanbanIssue[]): KanbanIssue[] {
  const order = KANBAN_ISSUE_LIST_ORDER as readonly string[];
  const rank = (status: string) => {
    const index = order.indexOf(String(status || "").trim());
    return index < 0 ? order.length : index;
  };
  return [...(Array.isArray(entries) ? entries : [])].sort((a, b) => {
    const byGroup = rank(a.status) - rank(b.status);
    if (byGroup !== 0) return byGroup;
    return Number(b.number || 0) - Number(a.number || 0);
  });
}

/** 渲染用的分组视图：只输出非空组，每项带上它在扁平列表里的下标，键盘导航仍走扁平索引。 */
export function groupKanbanIssueOptions(entries: KanbanIssue[]): {
  status: string;
  label: string;
  items: { issue: KanbanIssue; index: number }[];
}[] {
  const flat = Array.isArray(entries) ? entries : [];
  const groups: { status: string; label: string; items: { issue: KanbanIssue; index: number }[] }[] = [];
  flat.forEach((issue, index) => {
    const status = String(issue.status || "").trim();
    let group = groups.find((item) => item.status === status);
    if (!group) {
      group = { status, label: kanbanIssueStatusText(status), items: [] };
      groups.push(group);
    }
    group.items.push({ issue, index });
  });
  return groups;
}

/** 议题引用列表：一个实例，磁贴点击与 `#` 选中都调这里，避免两条路各写一份。 */
export function useKanbanIssueReferences() {
  const references = ref<KanbanIssueReference[]>([]);

  function attachKanbanIssueReference(issue: KanbanIssue | KanbanIssueReference | null | undefined): void {
    if (!issue) return;
    const next = toKanbanIssueReference(issue as KanbanIssue);
    if (!next.number) return;
    if (references.value.some((item) => item.number === next.number)) return;
    references.value = [...references.value, next];
  }

  function removeKanbanIssueReference(number: number): void {
    const target = Number(number || 0);
    references.value = references.value.filter((item) => item.number !== target);
  }

  function clearKanbanIssueReferences(): void {
    references.value = [];
  }

  /** 发送时转成附加文本块，与 IDE 引用走同一条 `extraTextBlocks` 通道。 */
  function buildKanbanIssueReferenceBlocks(): string[] {
    return references.value
      .map((item) => buildKanbanIssueReferenceText(item))
      .filter(Boolean);
  }

  return {
    references,
    attachKanbanIssueReference,
    removeKanbanIssueReference,
    clearKanbanIssueReferences,
    buildKanbanIssueReferenceBlocks,
  };
}
