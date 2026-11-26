import { describe, expect, it } from "vitest";
import { GALLERY_ENTRIES, GALLERY_GROUPS, findGalleryEntry } from "./gallery-registry";
import type { GalleryEntry } from "./types";

/**
 * 组件库的 key 与文件名是对外约定：`?key=xxx` 直达、无头截图任务、文档里写的
 * 「打开 ?key=xxx 看现象」全都照抄它。所以这里把 46 个 key 钉死——
 * 改名或漏搬会在测试里当场变红，而不是等某天截图任务拍到一张空白页才发现。
 */
const BASELINE_KEYS = [
  "app-icons",
  "app-loading",
  "double-deck",
  "theme-cards",
  "settings-shell",
  "panel-tabs",
  "config-cards",
  "quick-model-card",
  "templates",
  "mcp-definition-editor",
  "composer",
  "question",
  "question-tool",
  "search-engines",
  "bubbles",
  "chat-message",
  "thinking-and-tools",
  "thinking-preview",
  "rich-block-embed",
  "diff-flow",
  "markdown-view",
  "chat-view-stream",
  "chat-view-margin",
  "conversation-list",
  "conversation-item",
  "home-cards",
  "session-timeline",
  "session-float-dock",
  "persona-capability",
  "persona-permission",
  "persona-profile",
  "team-member-tiles",
  "delegates",
  "chat-home",
  "skill-managed",
  "kanban-board",
  "kanban-card",
  "overview",
  "catalog-store",
  "catalog-entry-card",
  "file-reader-preview",
  "file-reader-wrap",
  "image-preview-dialog",
  "cron-picker",
  "timer-create-card",
  "timer-list-item",
];

/** 少数项靠 props 固定形态；类型侧由 types.ts 声明，这里只读不写。 */
type EntryWithProps = GalleryEntry & { props?: Record<string, unknown> };

describe("组件库注册表", () => {
  it("key 集合与基线一致", () => {
    expect([...GALLERY_ENTRIES.map((item) => item.key)].sort()).toEqual([...BASELINE_KEYS].sort());
  });

  it("key 不重复", () => {
    const keys = GALLERY_ENTRIES.map((item) => item.key);
    expect(keys.length).toBe(new Set(keys).size);
  });

  it("每项都挂得住加载：有 loader、有非空 file", () => {
    const broken = GALLERY_ENTRIES.filter(
      (item) => typeof item.loader !== "function" || !item.file.trim(),
    ).map((item) => item.key);
    expect(broken).toEqual([]);
  });

  it("只有 persona 三项带 props", () => {
    const withProps = (GALLERY_ENTRIES as EntryWithProps[]).filter(
      (item) => item.props && Object.keys(item.props).length > 0,
    );
    expect(withProps.map((item) => item.key).sort()).toEqual([
      "persona-capability",
      "persona-permission",
      "persona-profile",
    ]);
    expect(withProps.map((item) => item.props?.view).sort()).toEqual([
      "capability",
      "permission",
      "persona",
    ]);
  });

  it("每项都挂了分组，且能按 key 查到", () => {
    const grouped = GALLERY_GROUPS.flatMap((group) => group.entries);
    expect(grouped.length).toBe(GALLERY_ENTRIES.length);
    expect(findGalleryEntry("question")?.file).toBe("demos/QuestionDemo.vue");
    expect(findGalleryEntry("不存在")).toBeUndefined();
  });
});
