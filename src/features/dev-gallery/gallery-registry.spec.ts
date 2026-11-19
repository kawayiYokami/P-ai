import { describe, expect, it } from "vitest";
import { GALLERY_ENTRIES, GALLERY_GROUPS, findGalleryEntry } from "./gallery-registry";

/**
 * 组件库的 key 与文件名是对外约定：`?key=xxx` 直达、无头截图任务、文档里写的
 * 「打开 ?key=xxx 看现象」全都照抄它。所以这里把 40 个 key 钉死——
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
  "composer",
  "question",
  "question-tool",
  "search-engines",
  "bubbles",
  "chat-message",
  "thinking-and-tools",
  "thinking-preview",
  "rich-block-embed",
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
];

const DEMO_FILE_KEYS = Object.keys(import.meta.glob("./demos/*.vue")).map((path) =>
  path.replace("./demos/", "").replace(/\.vue$/, ""),
);

describe("组件库注册表", () => {
  it("key 集合与基线一致", () => {
    expect([...GALLERY_ENTRIES.map((item) => item.key)].sort()).toEqual([...BASELINE_KEYS].sort());
  });

  it("key 不重复", () => {
    const keys = GALLERY_ENTRIES.map((item) => item.key);
    expect(keys.length).toBe(new Set(keys).size);
  });

  it("每个登记录入的文件都真实存在", () => {
    const existing = new Set(DEMO_FILE_KEYS);
    const missing = GALLERY_ENTRIES.filter((item) => !existing.has(item.file)).map((item) => item.key);
    expect(missing).toEqual([]);
  });

  it("没有登记表之外的多余 demo 文件", () => {
    const registered = new Set(GALLERY_ENTRIES.map((item) => item.file));
    expect(DEMO_FILE_KEYS.filter((file) => !registered.has(file))).toEqual([]);
  });

  it("每项都挂了分组，且能按 key 查到", () => {
    const grouped = GALLERY_GROUPS.flatMap((group) => group.entries);
    expect(grouped.length).toBe(GALLERY_ENTRIES.length);
    expect(findGalleryEntry("question")?.file).toBe("QuestionDemo");
    expect(findGalleryEntry("不存在")).toBeUndefined();
  });
});
