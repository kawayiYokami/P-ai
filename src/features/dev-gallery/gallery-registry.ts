import type { Component } from "vue";
import type { GalleryEntry, GalleryGroup } from "./types";

/**
 * 组件库总表：分组 → 组件 → 内容从哪来。
 *
 * 约定：
 * - 每一项都直接懒加载真实内容：要么是 `src/features/` 下的公用 `*Demo.vue`
 *   （模拟数据 + 控制面板 + 真实组件，自包含），要么是 `demos/` 下少数自带数据、
 *   必须放进组件库才能拼出来的直调文件。不再有「包一层公用组件」的薄包装。
 * - 每个 loader 都是字面量动态 import，Vite 能静态分析：按需加载，生产包不拉整棵子树。
 * - `file` 只给人看，写下内容实际所在，不参与加载。
 * - key 与旧 demo 页保持一致，`?key=xxx` 直达与截图流程不用改。
 */

function entry(
  key: string,
  name: string,
  title: string,
  file: string,
  loader: () => Promise<{ default: Component }>,
  props?: Record<string, unknown>,
): GalleryEntry {
  const item: GalleryEntry = { key, name, title, file, loader };
  // 少数项需要固定一种形态（人格主页 / 权限页共用同一个公用组件）。
  if (props) Object.assign(item, { props });
  return item;
}

export const GALLERY_GROUPS: GalleryGroup[] = [
  {
    id: "basic",
    title: "基础",
    entries: [
      entry("app-icons", "AppIcon", "图标语义名总表", "demos/AppIconsDemo.vue", () => import("./demos/AppIconsDemo.vue")),
      entry("app-loading", "AppLoading", "加载指示器", "shared/components/AppLoadingDemo.vue", () =>
        import("../shared/components/AppLoadingDemo.vue"),
      ),
      entry("double-deck", "DoubleDeck", "双层卡", "chat/components/DoubleDeckCardDemo.vue", () =>
        import("../chat/components/DoubleDeckCardDemo.vue"),
      ),
      entry("theme-cards", "AutoThemeGrid", "主题预设卡片", "demos/ThemeCardsDemo.vue", () =>
        import("./demos/ThemeCardsDemo.vue"),
      ),
    ],
  },
  {
    id: "layout",
    title: "布局与外壳",
    entries: [
      entry("settings-shell", "SettingsShell", "设置页头部（面包屑 + 标签页）", "config/components/SettingsShellGalleryDemo.vue", () =>
        import("../config/components/SettingsShellGalleryDemo.vue"),
      ),
      entry("panel-tabs", "PanelTabStrip", "标签栏组件", "demos/PanelTabsDemo.vue", () =>
        import("./demos/PanelTabsDemo.vue"),
      ),
      entry("config-cards", "ConfigCards", "配置卡片画廊", "config/components/ConfigCardsGalleryDemo.vue", () =>
        import("../config/components/ConfigCardsGalleryDemo.vue"),
      ),
      entry("quick-model-card", "QuickModelCard", "快速模型不可用卡片", "config/components/QuickModelUnavailableCardDemo.vue", () =>
        import("../config/components/QuickModelUnavailableCardDemo.vue"),
      ),
      entry("templates", "ConfigTemplate", "配置模板", "demos/ConfigTemplateDemo.vue", () =>
        import("./demos/ConfigTemplateDemo.vue"),
      ),
    ],
  },
  {
    id: "form",
    title: "表单与输入",
    entries: [
      entry("composer", "ChatComposer", "输入面板新结构", "chat/components/ChatComposerStructureDemo.vue", () =>
        import("../chat/components/ChatComposerStructureDemo.vue"),
      ),
      entry("question", "ChatQuestionPanel", "提问卡", "demos/QuestionDemo.vue", () =>
        import("./demos/QuestionDemo.vue"),
      ),
      entry("question-tool", "ConversationQuestion", "会话提问（question 工具）", "demos/QuestionToolDemo.vue", () =>
        import("./demos/QuestionToolDemo.vue"),
      ),
      entry("search-engines", "SearchEngines", "搜索设置", "config/views/config-tabs/SearchEnginesDemo.vue", () =>
        import("../config/views/config-tabs/SearchEnginesDemo.vue"),
      ),
    ],
  },
  {
    id: "chat",
    title: "聊天",
    entries: [
      entry("bubbles", "ChatBubbleShell", "自研气泡", "demos/BubblesDemo.vue", () =>
        import("./demos/BubblesDemo.vue"),
      ),
      entry("chat-message", "ChatMessageItem", "消息项（队伍消息气泡）", "chat/components/ChatMessageItemDemo.vue", () =>
        import("../chat/components/ChatMessageItemDemo.vue"),
      ),
      entry("thinking-and-tools", "ChatThinkingAndTools", "思考与工具（折叠块 + 流式压测）", "chat/components/ChatThinkingAndToolsDemo.vue", () =>
        import("../chat/components/ChatThinkingAndToolsDemo.vue"),
      ),
      entry("thinking-preview", "ChatThinkingPreviewBar", "思考预览 bar", "chat/components/ChatThinkingPreviewBarDemo.vue", () =>
        import("../chat/components/ChatThinkingPreviewBarDemo.vue"),
      ),
      entry("rich-block-embed", "ChatRichBlockEmbed", "代码块 / mermaid 内嵌气泡", "chat/components/ChatRichBlockEmbedDemo.vue", () =>
        import("../chat/components/ChatRichBlockEmbedDemo.vue"),
      ),
      entry("markdown-view", "AppMarkdownRenderer", "Markdown 全量渲染", "chat/markdown/AppMarkdownRendererDemo.vue", () =>
        import("../chat/markdown/AppMarkdownRendererDemo.vue"),
      ),
      entry("chat-view-stream", "ChatViewStream", "对话视图流式复现（全组件）", "chat/views/ChatViewStreamDemo.vue", () =>
        import("../chat/views/ChatViewStreamDemo.vue"),
      ),
      entry("chat-view-margin", "ChatViewMargin", "对话视图滚动落点（startMargin 实测）", "chat/views/ChatViewDemo.vue", () =>
        import("../chat/views/ChatViewDemo.vue"),
      ),
      entry("conversation-list", "ChatConversationList", "列表卡画廊", "chat/components/ChatConversationListGalleryDemo.vue", () =>
        import("../chat/components/ChatConversationListGalleryDemo.vue"),
      ),
      entry("conversation-item", "ChatConversationItem", "会话条目（状态 × 未读）", "chat/components/ChatConversationItemDemo.vue", () =>
        import("../chat/components/ChatConversationItemDemo.vue"),
      ),
      entry("home-cards", "HomeCardGallery", "预览卡画廊", "chat/components/HomeCardGalleryDemo.vue", () =>
        import("../chat/components/HomeCardGalleryDemo.vue"),
      ),
      entry("session-timeline", "SessionTimeline", "会话时间线（垂直）", "chat/components/SessionTimelineDemo.vue", () =>
        import("../chat/components/SessionTimelineDemo.vue"),
      ),
      entry("session-float-dock", "SessionFloatDock", "会话悬浮操作区", "chat/components/SessionFloatDockDemo.vue", () =>
        import("../chat/components/SessionFloatDockDemo.vue"),
      ),
    ],
  },
  {
    id: "persona",
    title: "人格与队伍",
    entries: [
      entry("persona-capability", "PersonaTab", "人格主页", "config/components/PersonaTabDemo.vue", () =>
        import("../config/components/PersonaTabDemo.vue"), { view: "profile" }),
      entry("persona-permission", "PersonaPermission", "人格权限页（黑白名单树）", "config/components/PersonaTabDemo.vue", () =>
        import("../config/components/PersonaTabDemo.vue"), { view: "permission" }),
      entry("team-member-tiles", "TeamMemberTiles", "队员磁贴（自适应列数）", "chat/components/TeamMemberTileGridDemo.vue", () =>
        import("../chat/components/TeamMemberTileGridDemo.vue"),
      ),
      entry("delegates", "DelegateProgressLine", "委派进度条", "demos/DelegatesDemo.vue", () =>
        import("./demos/DelegatesDemo.vue"),
      ),
      entry("chat-home", "ChatHomePanel", "首页磁贴区", "chat/components/ChatHomePanelDemo.vue", () =>
        import("../chat/components/ChatHomePanelDemo.vue"),
      ),
      entry("skill-managed", "SkillPanel", "技能面板（托管技能禁用态）", "config/components/SkillTabDemo.vue", () =>
        import("../config/components/SkillTabDemo.vue"),
      ),
    ],
  },
  {
    id: "kanban",
    title: "看板与任务",
    entries: [
      entry("kanban-board", "KanbanBoardPanel", "议题看板页", "demos/KanbanBoardDemo.vue", () =>
        import("./demos/KanbanBoardDemo.vue"),
      ),
      entry("kanban-card", "HomeKanbanCard", "议题看板磁贴（3×3）", "demos/KanbanCardDemo.vue", () =>
        import("./demos/KanbanCardDemo.vue"),
      ),
      entry("overview", "MonitorOverview", "监控概览", "demos/OverviewDemo.vue", () =>
        import("./demos/OverviewDemo.vue"),
      ),
    ],
  },
  {
    id: "catalog",
    title: "能力商店",
    entries: [
      entry("catalog-store", "CatalogStore", "能力商店", "config/components/CatalogStoreDemo.vue", () =>
        import("../config/components/CatalogStoreDemo.vue"),
      ),
      entry("catalog-entry-card", "CatalogEntryCard", "能力商店条目卡", "config/components/CatalogEntryCardDemo.vue", () =>
        import("../config/components/CatalogEntryCardDemo.vue"),
      ),
    ],
  },
  {
    id: "files",
    title: "文件与媒体",
    entries: [
      entry("file-reader-preview", "FileReaderPreview", "右栏内容视图（HTML 沙箱 / 源码 / 图片）", "file-reader/components/FileReaderPreviewDemo.vue", () =>
        import("../file-reader/components/FileReaderPreviewDemo.vue"),
      ),
      entry("file-reader-wrap", "FileReaderWrap", "文件阅读器换行模式", "file-reader/components/FileReaderPanelDemo.vue", () =>
        import("../file-reader/components/FileReaderPanelDemo.vue"),
      ),
      entry("image-preview-dialog", "ChatImagePreviewDialog", "图片预览百分比读数", "chat/components/dialogs/ChatImagePreviewDialogDemo.vue", () =>
        import("../chat/components/dialogs/ChatImagePreviewDialogDemo.vue"),
      ),
    ],
  },
];

export const GALLERY_ENTRIES: GalleryEntry[] = GALLERY_GROUPS.flatMap((group) => group.entries);

export const DEFAULT_GALLERY_KEY = GALLERY_ENTRIES[0]?.key ?? "";

export function findGalleryEntry(key: string | null | undefined): GalleryEntry | undefined {
  if (!key) return undefined;
  return GALLERY_ENTRIES.find((item) => item.key === key);
}
