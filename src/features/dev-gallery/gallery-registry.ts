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
 * - `title` 是侧栏与标题栏唯一露出的文字：只写用户认得的短名，
 *   折叠块、startMargin、禁用态这类实现术语不进导航。
 * - 分组按**功能域**划分（设置页归设置、磁贴归队伍），不按组件形态混装：
 *   同一组里的东西是「用户在同一处会一起遇到」的，不是「长得都像卡片」。
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
      entry("app-icons", "AppIcon", "图标", "demos/AppIconsDemo.vue", () => import("./demos/AppIconsDemo.vue")),
      entry("app-loading", "AppLoading", "加载指示器", "shared/components/AppLoadingDemo.vue", () =>
        import("../shared/components/AppLoadingDemo.vue"),
      ),
      entry("double-deck", "DoubleDeck", "双层卡", "chat/components/DoubleDeckCardDemo.vue", () =>
        import("../chat/components/DoubleDeckCardDemo.vue"),
      ),
      entry("theme-cards", "AutoThemeGrid", "主题卡片", "demos/ThemeCardsDemo.vue", () =>
        import("./demos/ThemeCardsDemo.vue"),
      ),
      entry("panel-tabs", "PanelTabStrip", "标签栏", "demos/PanelTabsDemo.vue", () =>
        import("./demos/PanelTabsDemo.vue"),
      ),
    ],
  },
  {
    id: "settings",
    title: "设置页",
    entries: [
      entry("settings-shell", "SettingsShell", "设置页头部", "config/components/SettingsShellGalleryDemo.vue", () =>
        import("../config/components/SettingsShellGalleryDemo.vue"),
      ),
      entry("config-cards", "ConfigCards", "配置卡片", "config/components/ConfigCardsGalleryDemo.vue", () =>
        import("../config/components/ConfigCardsGalleryDemo.vue"),
      ),
      entry("quick-model-card", "QuickModelCard", "模型不可用卡", "config/components/QuickModelUnavailableCardDemo.vue", () =>
        import("../config/components/QuickModelUnavailableCardDemo.vue"),
      ),
      entry("templates", "ConfigTemplate", "配置模板", "demos/ConfigTemplateDemo.vue", () =>
        import("./demos/ConfigTemplateDemo.vue"),
      ),
      entry("mcp-definition-editor", "McpServerDefinitionEditor", "MCP 定义编辑器", "demos/McpDefinitionEditorDemo.vue", () =>
        import("./demos/McpDefinitionEditorDemo.vue"),
      ),
      entry("search-engines", "SearchEngines", "搜索供应商", "config/views/config-tabs/SearchEnginesDemo.vue", () =>
        import("../config/views/config-tabs/SearchEnginesDemo.vue"),
      ),
      entry("skill-managed", "SkillPanel", "技能面板", "config/components/SkillTabDemo.vue", () =>
        import("../config/components/SkillTabDemo.vue"),
      ),
      entry("persona-capability", "PersonaTab", "人格主页", "config/components/PersonaTabDemo.vue", () =>
        import("../config/components/PersonaTabDemo.vue"), { view: "persona" }),
      entry("persona-permission", "PersonaPermission", "人格权限页", "config/components/PersonaTabDemo.vue", () =>
        import("../config/components/PersonaTabDemo.vue"), { view: "permission" }),
    ],
  },
  {
    id: "chat",
    title: "聊天",
    entries: [
      entry("bubbles", "ChatBubbleShell", "气泡", "demos/BubblesDemo.vue", () =>
        import("./demos/BubblesDemo.vue"),
      ),
      entry("chat-message", "ChatMessageItem", "消息项", "chat/components/ChatMessageItemDemo.vue", () =>
        import("../chat/components/ChatMessageItemDemo.vue"),
      ),
      entry("thinking-and-tools", "ChatThinkingAndTools", "思考与工具", "chat/components/ChatThinkingAndToolsDemo.vue", () =>
        import("../chat/components/ChatThinkingAndToolsDemo.vue"),
      ),
      entry("thinking-preview", "ChatThinkingPreviewBar", "思考预览", "chat/components/ChatThinkingPreviewBarDemo.vue", () =>
        import("../chat/components/ChatThinkingPreviewBarDemo.vue"),
      ),
      entry("rich-block-embed", "ChatRichBlockEmbed", "代码块与图表", "chat/components/ChatRichBlockEmbedDemo.vue", () =>
        import("../chat/components/ChatRichBlockEmbedDemo.vue"),
      ),
      entry("diff-flow", "DiffFlowView", "改动平铺", "chat/components/DiffFlowViewDemo.vue", () =>
        import("../chat/components/DiffFlowViewDemo.vue"),
      ),
      entry("markdown-view", "AppMarkdownRenderer", "Markdown 渲染", "chat/markdown/AppMarkdownRendererDemo.vue", () =>
        import("../chat/markdown/AppMarkdownRendererDemo.vue"),
      ),
      entry("chat-view-stream", "ChatViewStream", "对话视图", "chat/views/ChatViewStreamDemo.vue", () =>
        import("../chat/views/ChatViewStreamDemo.vue"),
      ),
      entry("chat-view-margin", "ChatViewMargin", "滚动落点", "chat/views/ChatViewDemo.vue", () =>
        import("../chat/views/ChatViewDemo.vue"),
      ),
      entry("conversation-list", "ChatConversationList", "会话列表", "chat/components/ChatConversationListGalleryDemo.vue", () =>
        import("../chat/components/ChatConversationListGalleryDemo.vue"),
      ),
      entry("conversation-item", "ChatConversationItem", "会话条目", "chat/components/ChatConversationItemDemo.vue", () =>
        import("../chat/components/ChatConversationItemDemo.vue"),
      ),
      entry("home-cards", "HomeCardGallery", "预览卡", "chat/components/HomeCardGalleryDemo.vue", () =>
        import("../chat/components/HomeCardGalleryDemo.vue"),
      ),
      entry("session-timeline", "SessionTimeline", "会话时间线", "chat/components/SessionTimelineDemo.vue", () =>
        import("../chat/components/SessionTimelineDemo.vue"),
      ),
      entry("session-float-dock", "SessionFloatDock", "悬浮操作区", "chat/components/SessionFloatDockDemo.vue", () =>
        import("../chat/components/SessionFloatDockDemo.vue"),
      ),
      entry("question", "ChatQuestionPanel", "提问卡", "demos/QuestionDemo.vue", () =>
        import("./demos/QuestionDemo.vue"),
      ),
      entry("question-tool", "ConversationQuestion", "会话提问", "demos/QuestionToolDemo.vue", () =>
        import("./demos/QuestionToolDemo.vue"),
      ),
      entry("composer", "ChatComposer", "输入面板", "chat/components/ChatComposerStructureDemo.vue", () =>
        import("../chat/components/ChatComposerStructureDemo.vue"),
      ),
      entry("cron-picker", "CronPicker", "定时表达式", "shared/components/CronPickerDemo.vue", () =>
        import("../shared/components/CronPickerDemo.vue"),
      ),
      entry("timer-create-card", "TimerCreateCard", "定时器弹窗", "chat/components/dialogs/TimerCreateCardDemo.vue", () =>
        import("../chat/components/dialogs/TimerCreateCardDemo.vue"),
      ),
      entry("timer-list-item", "TimerListItem", "定时器列表项", "chat/components/TimerListItemDemo.vue", () =>
        import("../chat/components/TimerListItemDemo.vue"),
      ),
    ],
  },
  {
    id: "tile",
    title: "磁贴",
    entries: [
      entry("team-member-tiles", "TeamMemberTiles", "队员磁贴", "chat/components/TeamMemberTileGridDemo.vue", () =>
        import("../chat/components/TeamMemberTileGridDemo.vue"),
      ),
      entry("chat-home", "ChatHomePanel", "首页磁贴", "chat/components/ChatHomePanelDemo.vue", () =>
        import("../chat/components/ChatHomePanelDemo.vue"),
      ),
      entry("kanban-card", "HomeKanbanCard", "看板磁贴", "demos/KanbanCardDemo.vue", () =>
        import("./demos/KanbanCardDemo.vue"),
      ),
    ],
  },
  {
    id: "kanban",
    title: "看板与任务",
    entries: [
      entry("kanban-board", "KanbanBoardPanel", "议题看板", "demos/KanbanBoardDemo.vue", () =>
        import("./demos/KanbanBoardDemo.vue"),
      ),
      entry("delegates", "DelegateProgressLine", "委派进度条", "demos/DelegatesDemo.vue", () =>
        import("./demos/DelegatesDemo.vue"),
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
      entry("catalog-entry-card", "CatalogEntryCard", "商店条目卡", "config/components/CatalogEntryCardDemo.vue", () =>
        import("../config/components/CatalogEntryCardDemo.vue"),
      ),
    ],
  },
  {
    id: "files",
    title: "文件与媒体",
    entries: [
      entry("file-reader-preview", "FileReaderPreview", "右栏内容视图", "file-reader/components/FileReaderPreviewDemo.vue", () =>
        import("../file-reader/components/FileReaderPreviewDemo.vue"),
      ),
      entry("file-reader-wrap", "FileReaderWrap", "文件阅读器", "file-reader/components/FileReaderPanelDemo.vue", () =>
        import("../file-reader/components/FileReaderPanelDemo.vue"),
      ),
      entry("image-preview-dialog", "ChatImagePreviewDialog", "图片预览", "chat/components/dialogs/ChatImagePreviewDialogDemo.vue", () =>
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
