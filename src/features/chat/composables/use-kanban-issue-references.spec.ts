import { describe, expect, it } from "vitest";
import type { KanbanIssue } from "./use-kanban-board";
import {
  buildKanbanIssueReferenceLabel,
  buildKanbanIssueReferenceText,
  filterKanbanIssueOptions,
  findKanbanIssueTrigger,
  kanbanIssueStatusText,
  useKanbanIssueReferences,
} from "./use-kanban-issue-references";

function makeIssue(overrides: Partial<KanbanIssue> = {}): KanbanIssue {
  return {
    number: 3,
    title: "看板磁贴尺寸定稿",
    status: "in_review",
    assignee: "纳西妲",
    note: "红豆要看两种宽度",
    ...overrides,
  };
}

describe("kanban issue reference trigger", () => {
  it("行首与空白之后都算触发", () => {
    expect(findKanbanIssueTrigger("#", 1)).toEqual({ start: 0, end: 1, query: "" });
    expect(findKanbanIssueTrigger("#3", 2)).toEqual({ start: 0, end: 2, query: "3" });
    expect(findKanbanIssueTrigger("请看一下 #3", 7)).toEqual({ start: 5, end: 7, query: "3" });
    expect(findKanbanIssueTrigger("第一行\n#12", 7)).toEqual({ start: 4, end: 7, query: "12" });
  });

  it("紧贴文字的井号不触发，避免正常书写时弹面板", () => {
    expect(findKanbanIssueTrigger("abc#3", 5)).toBeNull();
    expect(findKanbanIssueTrigger("C#", 2)).toBeNull();
  });

  it("井号后面带空格或第二个井号时不再是一个词元", () => {
    expect(findKanbanIssueTrigger("#3 已结案", 7)).toBeNull();
    expect(findKanbanIssueTrigger("##3", 3)).toBeNull();
  });

  it("光标不在末尾时只看光标之前的内容", () => {
    const text = "#3 后续内容";
    expect(findKanbanIssueTrigger(text, 2)).toEqual({ start: 0, end: 2, query: "3" });
  });
});

describe("kanban issue reference text", () => {
  it("标签是 #编号 加标题", () => {
    expect(buildKanbanIssueReferenceLabel(makeIssue())).toBe("#3 看板磁贴尺寸定稿");
    expect(buildKanbanIssueReferenceLabel(makeIssue({ title: "  " }))).toBe("#3");
  });

  it("附加文本首行是标签，其后是状态与备注", () => {
    expect(buildKanbanIssueReferenceText(makeIssue())).toBe(
      "#3 看板磁贴尺寸定稿\n状态：待审\n备注：红豆要看两种宽度",
    );
  });

  it("没有备注时只留编号、标题与状态", () => {
    expect(buildKanbanIssueReferenceText(makeIssue({ note: "" }))).toBe(
      "#3 看板磁贴尺寸定稿\n状态：待审",
    );
  });

  it("状态按界面同一套中文说明，未知状态原样保留", () => {
    expect(kanbanIssueStatusText("in_progress")).toBe("解决中");
    expect(kanbanIssueStatusText("archived")).toBe("archived");
  });
});

describe("kanban issue option filter", () => {
  const issues = [
    makeIssue({ number: 3, title: "看板磁贴尺寸定稿" }),
    makeIssue({ number: 12, title: "输入框引用" }),
    makeIssue({ number: 21, title: "看板排序" }),
  ];

  it("空查询返回全部议题", () => {
    expect(filterKanbanIssueOptions(issues, "")).toHaveLength(3);
    expect(filterKanbanIssueOptions(issues, "   ")).toHaveLength(3);
  });

  it("当前会话没有议题时不给任何候选，调用方据此不弹面板", () => {
    expect(filterKanbanIssueOptions([], "")).toEqual([]);
    expect(filterKanbanIssueOptions([], "3")).toEqual([]);
  });

  it("纯数字按编号前缀匹配", () => {
    expect(filterKanbanIssueOptions(issues, "1").map((item) => item.number)).toEqual([12]);
    expect(filterKanbanIssueOptions(issues, "2").map((item) => item.number)).toEqual([21]);
  });

  it("非数字按标题包含匹配，大小写无关", () => {
    expect(filterKanbanIssueOptions(issues, "看板").map((item) => item.number)).toEqual([3, 21]);
    expect(filterKanbanIssueOptions(issues, "引用").map((item) => item.number)).toEqual([12]);
  });
});

describe("kanban issue reference store", () => {
  it("同一编号只挂一次，移除后不再进发送文本", () => {
    const store = useKanbanIssueReferences();
    store.attachKanbanIssueReference(makeIssue());
    store.attachKanbanIssueReference(makeIssue({ title: "重复挂同一条" }));
    expect(store.references.value).toHaveLength(1);
    expect(store.buildKanbanIssueReferenceBlocks()).toEqual([
      "#3 看板磁贴尺寸定稿\n状态：待审\n备注：红豆要看两种宽度",
    ]);

    store.removeKanbanIssueReference(3);
    expect(store.references.value).toHaveLength(0);
    expect(store.buildKanbanIssueReferenceBlocks()).toEqual([]);
  });

  it("发送后清空，下一条消息不再带旧引用", () => {
    const store = useKanbanIssueReferences();
    store.attachKanbanIssueReference(makeIssue());
    store.clearKanbanIssueReferences();
    expect(store.buildKanbanIssueReferenceBlocks()).toEqual([]);
  });

  it("没有编号的议题不挂", () => {
    const store = useKanbanIssueReferences();
    store.attachKanbanIssueReference(makeIssue({ number: 0 }));
    expect(store.references.value).toHaveLength(0);
  });
});
