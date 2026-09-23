import { describe, expect, it } from "vitest";
import { buildConversationSections, canonicalWorkspaceRootForComparison, conversationCountSinceDayStart, applyConversationSectionOrder, type ConversationSection, type ConversationSectionTitles } from "./conversation-sections";
import type { ChatConversationOverviewItem } from "../../../types/app";

const titles: ConversationSectionTitles = {
  recent: "最近",
  pinned: "置顶",
  other: "其他",
  defaultWorkspace: "默认工作区",
  currentProject: "当前项目",
};

function item(overrides: Partial<ChatConversationOverviewItem> & { conversationId: string }): ChatConversationOverviewItem {
  return {
    kind: "local_unarchived",
    title: "",
    lastMessageAt: "",
    updatedAt: "",
    unreadCount: 0,
    isPinned: false,
    ...overrides,
  } as ChatConversationOverviewItem;
}

function ids(sections: ReturnType<typeof buildConversationSections>): string[] {
  return sections.flatMap((section) => section.items.map((entry) => entry.conversationId));
}

describe("buildConversationSections", () => {
  it("置顶会话排在最前，其次最近会话，最后按工作区分组", () => {
    const items = [
      item({ conversationId: "old", lastMessageAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" }),
      item({ conversationId: "new", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
      item({ conversationId: "pinned-old", lastMessageAt: "2025-01-01T00:00:00Z", updatedAt: "2025-01-01T00:00:00Z", isPinned: true }),
      item({ conversationId: "ws", lastMessageAt: "2026-07-01T00:00:00Z", updatedAt: "2026-07-01T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
    ];
    const sections = buildConversationSections(items, { tab: "local", titles, locale: "zh-CN" });
    const order = ids(sections);
    expect(order[0]).toBe("pinned-old");
    expect(order.slice(1)).toContain("new");
    expect(order.slice(1)).toContain("old");
    expect(order).toContain("ws");
    expect(sections.find((section) => section.key === "pinned")?.items.map((entry) => entry.conversationId)).toEqual(["pinned-old"]);
    expect(sections.find((section) => section.key.startsWith("workspace:"))?.items.map((entry) => entry.conversationId)).toEqual(["ws"]);
  });

  it("contact 标签下只显示远程联系人会话", () => {
    const items = [
      item({ conversationId: "local", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
      item({ conversationId: "remote", kind: "remote_im_contact", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", channelName: "频道A" }),
    ];
    const sections = buildConversationSections(items, { tab: "contact", titles, locale: "zh-CN" });
    expect(ids(sections)).not.toContain("local");
    expect(ids(sections)).toContain("remote");
  });

  it("会话草稿非 active 时从所有分组消失", () => {
    const items = [
      item({ conversationId: "normal", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
      item({ conversationId: "draft-hidden", isDraft: true, lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z" }),
      item({ conversationId: "draft-pinned", isDraft: true, isPinned: true, lastMessageAt: "2026-08-03T00:00:00Z", updatedAt: "2026-08-03T00:00:00Z" }),
    ];
    const sections = buildConversationSections(items, { tab: "local", titles, locale: "zh-CN" });
    expect(ids(sections)).not.toContain("draft-hidden");
    expect(ids(sections)).not.toContain("draft-pinned");
  });

  it("会话草稿 active 时只进入最近分组，不进置顶/当前项目/工作区分组", () => {
    const items = [
      item({ conversationId: "normal", lastMessageAt: "2026-07-01T00:00:00Z", updatedAt: "2026-07-01T00:00:00Z" }),
      item({ conversationId: "draft-active", isDraft: true, lastMessageAt: "2026-08-05T00:00:00Z", updatedAt: "2026-08-05T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
      item({ conversationId: "ws", lastMessageAt: "2026-08-04T00:00:00Z", updatedAt: "2026-08-04T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj",
      activeConversationId: "draft-active",
    });
    const recentIds = sections.find((section) => section.key === "recent")?.items.map((entry) => entry.conversationId) || [];
    expect(recentIds).toContain("draft-active");
    expect(sections.find((section) => section.key === "current-project")?.items.map((entry) => entry.conversationId)).toEqual(["ws"]);
    expect(ids(sections).filter((id) => id === "draft-active")).toHaveLength(1);
  });

  it("非 contact 标签下排除远程联系人会话", () => {
    const items = [
      item({ conversationId: "local", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
      item({ conversationId: "remote", kind: "remote_im_contact", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z" }),
    ];
    const sections = buildConversationSections(items, { tab: "local", titles, locale: "zh-CN" });
    expect(ids(sections)).not.toContain("remote");
    expect(ids(sections)).toContain("local");
  });

  it("空列表不产生任何分区", () => {
    const sections = buildConversationSections([], { tab: "local", titles, locale: "zh-CN" });
    expect(sections).toEqual([]);
  });

  it("传入当前工作区时生成「当前项目」分组并剔除重复项", () => {
    const items = [
      item({ conversationId: "project-a", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
      item({ conversationId: "project-b", lastMessageAt: "2026-08-03T00:00:00Z", updatedAt: "2026-08-03T00:00:00Z", workspaceRootPath: "e:\\work\\proj\\" }),
      item({ conversationId: "other-ws", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/other" }),
      item({ conversationId: "no-ws", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj",
    });

    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.title).toBe("当前项目");
    expect(currentProject?.items.map((entry) => entry.conversationId)).toEqual(["project-a", "project-b"]);

    const allOtherIds = new Set(
      sections
        .filter((section) => section.key !== "current-project")
        .flatMap((section) => section.items.map((entry) => entry.conversationId)),
    );
    expect(allOtherIds).not.toContain("project-a");
    expect(allOtherIds).not.toContain("project-b");
    expect(allOtherIds).toContain("other-ws");
    expect(allOtherIds).toContain("no-ws");
  });

  it("当前工作区带 Windows 扩展长度前缀时仍能匹配普通路径的会话", () => {
    const items = [
      item({ conversationId: "project-a", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
      item({ conversationId: "other-ws", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/other" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "\\\\?\\E:\\work\\proj",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.items.map((entry) => entry.conversationId)).toEqual(["project-a"]);
  });

  it("当前工作区无匹配会话时仍生成空的「当前项目」分组", () => {
    const items = [
      item({ conversationId: "other-ws", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/other" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject).toBeDefined();
    expect(currentProject?.items).toEqual([]);
    expect(ids(sections)).toContain("other-ws");
  });

  it("不传当前工作区时保持原有分组行为", () => {
    const items = [
      item({ conversationId: "ws", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
    ];
    const sections = buildConversationSections(items, { tab: "local", titles, locale: "zh-CN" });
    expect(sections.some((section) => section.key === "current-project")).toBe(false);
    expect(sections.some((section) => section.key.startsWith("workspace:"))).toBe(true);
  });

  it("精简模式下忽略置顶/当前项目/最近，全部会话只按工作区分组", () => {
    const items = [
      item({ conversationId: "pinned", isPinned: true, workspaceRootPath: "E:/work/proj", lastMessageAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" }),
      item({ conversationId: "cur", workspaceRootPath: "E:/work/proj", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
      item({ conversationId: "other", workspaceRootPath: "E:/work/other", lastMessageAt: "2026-07-01T00:00:00Z", updatedAt: "2026-07-01T00:00:00Z" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj",
      compact: true,
    });
    expect(sections.every((section) => section.key.startsWith("workspace:"))).toBe(true);
    expect(sections.some((section) => section.key === "pinned")).toBe(false);
    expect(sections.some((section) => section.key === "recent")).toBe(false);
    expect(sections.some((section) => section.key === "current-project")).toBe(false);
    expect(ids(sections).sort()).toEqual(["cur", "other", "pinned"]);
  });

  it("精简模式下 contact 标签按渠道分组且不单列置顶", () => {
    const items = [
      item({ conversationId: "r1", kind: "remote_im_contact", channelName: "频道A", isPinned: true, lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z" }),
      item({ conversationId: "r2", kind: "remote_im_contact", channelName: "频道A", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z" }),
    ];
    const sections = buildConversationSections(items, { tab: "contact", titles, locale: "zh-CN", compact: true });
    expect(sections).toHaveLength(1);
    expect(sections[0].items.map((entry) => entry.conversationId).sort()).toEqual(["r1", "r2"]);
  });

  it("最近会话分组包含全部候选条目，可持续加载更多直到展开完毕", () => {
    const items = Array.from({ length: 12 }, (_, index) =>
      item({
        conversationId: `recent-${index}`,
        lastMessageAt: `2026-08-0${(index % 9) + 1}T0${(index % 9) + 1}:00:00Z`,
        updatedAt: `2026-08-0${(index % 9) + 1}T0${(index % 9) + 1}:00:00Z`,
      }),
    );
    const sections = buildConversationSections(items, { tab: "local", titles, locale: "zh-CN" });
    const recentSection = sections.find((section) => section.key === "recent");
    expect(recentSection?.items.length).toBe(12);
  });

  it("host 为工作树路径时，仓库根与同仓库工作树会话均归入当前项目", () => {
    const items = [
      item({ conversationId: "repo-root", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "E:/work/proj" }),
      item({ conversationId: "worktree-sibling", lastMessageAt: "2026-08-03T00:00:00Z", updatedAt: "2026-08-03T00:00:00Z", workspaceRootPath: "E:/work/proj/.pai/.worktree/def456" }),
      item({ conversationId: "other", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/other" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj/.pai/.worktree/abc123",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.items.map((entry) => entry.conversationId).sort()).toEqual(["repo-root", "worktree-sibling"].sort());
  });

  it("host 为仓库根时，工作树会话亦归入当前项目（双向归一）", () => {
    const items = [
      item({ conversationId: "worktree-a", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "E:/work/proj/.pai/.worktree/abc123" }),
      item({ conversationId: "other", lastMessageAt: "2026-08-01T00:00:00Z", updatedAt: "2026-08-01T00:00:00Z", workspaceRootPath: "E:/work/other" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.items.map((entry) => entry.conversationId)).toEqual(["worktree-a"]);
  });

  it("host 工作树带扩展前缀与大小写差异仍能归一", () => {
    const items = [
      item({ conversationId: "repo-root", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "e:/work/proj" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "\\\\?\\E:\\work\\proj\\.pai\\.worktree\\abc123",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.items.map((entry) => entry.conversationId)).toEqual(["repo-root"]);
  });

  it("不同仓库的工作树不串台", () => {
    const items = [
      item({ conversationId: "other-worktree", lastMessageAt: "2026-08-02T00:00:00Z", updatedAt: "2026-08-02T00:00:00Z", workspaceRootPath: "E:/work/other/.pai/.worktree/xyz" }),
    ];
    const sections = buildConversationSections(items, {
      tab: "local",
      titles,
      locale: "zh-CN",
      currentWorkspaceRootPath: "E:/work/proj/.pai/.worktree/abc123",
    });
    const currentProject = sections.find((section) => section.key === "current-project");
    expect(currentProject?.items).toEqual([]);
  });
});

describe("canonicalWorkspaceRootForComparison", () => {
  it("回溯 .pai/.worktree 段到仓库根，大小写与分隔符归一", () => {
    expect(canonicalWorkspaceRootForComparison("E:\\work\\proj\\.pai\\.worktree\\abc123")).toBe("e:/work/proj");
    expect(canonicalWorkspaceRootForComparison("E:/work/proj/.pai/.worktree/")).toBe("e:/work/proj");
    expect(canonicalWorkspaceRootForComparison("E:/work/proj/.pai/worktree/feature-x")).toBe("e:/work/proj");
    expect(canonicalWorkspaceRootForComparison("\\\\?\\E:\\work\\proj")).toBe("e:/work/proj");
    expect(canonicalWorkspaceRootForComparison("E:/work/proj")).toBe("e:/work/proj");
  });

  it("非工作树路径保持归一化但不截断", () => {
    expect(canonicalWorkspaceRootForComparison("E:/work/proj/sub/dir/")).toBe("e:/work/proj/sub/dir");
  });
});

describe("conversationCountSinceDayStart", () => {
  it("统计当天凌晨 4 点至今活跃的会话数，早于阈值的条目不计数", () => {
    const now = new Date(2026, 7, 17, 17, 0, 0);
    const items = [
      item({ conversationId: "a", lastMessageAt: new Date(2026, 7, 17, 15, 0, 0).toISOString() }),
      item({ conversationId: "b", lastMessageAt: new Date(2026, 7, 17, 5, 0, 0).toISOString() }),
      item({ conversationId: "c", lastMessageAt: new Date(2026, 7, 17, 3, 0, 0).toISOString() }),
    ];
    expect(conversationCountSinceDayStart(items, now.getTime())).toBe(2);
  });

  it("当前时间在凌晨 4 点前时回退到昨天凌晨 4 点", () => {
    const now = new Date(2026, 7, 17, 2, 0, 0);
    const items = [
      item({ conversationId: "a", lastMessageAt: new Date(2026, 7, 17, 1, 30, 0).toISOString() }),
      item({ conversationId: "b", lastMessageAt: new Date(2026, 7, 16, 5, 0, 0).toISOString() }),
      item({ conversationId: "c", lastMessageAt: new Date(2026, 7, 16, 3, 0, 0).toISOString() }),
    ];
    expect(conversationCountSinceDayStart(items, now.getTime())).toBe(2);
  });

  it("凌晨 4 点至今没有活跃会话时返回 0，由调用方以至少 5 条兜底", () => {
    const now = new Date(2026, 7, 17, 17, 0, 0);
    const items = [
      item({ conversationId: "a", lastMessageAt: new Date(2026, 7, 16, 12, 0, 0).toISOString() }),
    ];
    expect(conversationCountSinceDayStart(items, now.getTime())).toBe(0);
  });
});

describe("applyConversationSectionOrder", () => {
  it("应按已保存顺序排列并追加新分区到末尾", () => {
    const sections = [
      { key: "workspace:b", title: "B", items: [] },
      { key: "workspace:c", title: "C", items: [] },
      { key: "workspace:a", title: "A", items: [] },
    ] satisfies ConversationSection[];

    const result = applyConversationSectionOrder(sections, ["workspace:a", "workspace:b"]);

    expect(result.sections.map((section) => section.key)).toEqual([
      "workspace:a",
      "workspace:b",
      "workspace:c",
    ]);
    expect(result.nextOrder).toEqual([
      "workspace:a",
      "workspace:b",
      "workspace:c",
    ]);
    expect(result.changed).toBe(true);
  });

  it("应忽略已保存顺序中不存在的 key 并保持剩余稳定顺序", () => {
    const sections = [
      { key: "workspace:b", title: "B", items: [] },
      { key: "workspace:a", title: "A", items: [] },
    ] satisfies ConversationSection[];

    const result = applyConversationSectionOrder(sections, ["workspace:gone", "workspace:a", "workspace:b"]);

    expect(result.sections.map((section) => section.key)).toEqual([
      "workspace:a",
      "workspace:b",
    ]);
    expect(result.nextOrder).toEqual([
      "workspace:a",
      "workspace:b",
    ]);
    expect(result.changed).toBe(true);
  });

  it("顺序未变化时 changed 为 false", () => {
    const sections = [
      { key: "pinned", title: "置顶", items: [] },
      { key: "workspace:a", title: "A", items: [] },
    ] satisfies ConversationSection[];

    const result = applyConversationSectionOrder(sections, ["pinned", "workspace:a"]);

    expect(result.changed).toBe(false);
  });

  it("当前项目分组固定在置顶与最近之间，不因旧排序被挤到末尾", () => {
    const sections = [
      { key: "workspace:b", title: "B", items: [] },
      { key: "recent", title: "最近", items: [] },
      { key: "current-project", title: "当前项目", items: [] },
      { key: "pinned", title: "置顶", items: [] },
      { key: "workspace:a", title: "A", items: [] },
    ] satisfies ConversationSection[];

    const result = applyConversationSectionOrder(sections, ["pinned", "recent", "workspace:b", "workspace:a"]);

    expect(result.sections.map((section) => section.key)).toEqual([
      "pinned",
      "current-project",
      "recent",
      "workspace:b",
      "workspace:a",
    ]);
    expect(result.nextOrder).toEqual([
      "pinned",
      "current-project",
      "recent",
      "workspace:b",
      "workspace:a",
    ]);
    expect(result.changed).toBe(true);
  });
});
