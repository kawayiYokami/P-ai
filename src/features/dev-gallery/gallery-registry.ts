import type { Component } from "vue";
import type { GalleryEntry, GalleryGroup } from "./types";

/**
 * 组件库总表：分组 → 组件 → 内容文件名。
 *
 * 约定：
 * - 每项内容都是 `demos/` 下的一个自包含 SFC，自己带容器样式与模拟数据。
 * - 文件名用字符串登记、由 import.meta.glob 统一解析：表里可以先列、文件后补，
 *   缺文件只影响那一项（点开时提示缺哪个文件），不会让整页编译失败。
 * - glob 默认懒加载，生产构建不会把整棵子树拉进依赖图。
 * - key 与旧 demo 页保持一致，`?key=xxx` 直达与截图流程不用改。
 */

const DEMO_MODULES = import.meta.glob("./demos/*.vue") as Record<
  string,
  () => Promise<{ default: Component }>
>;

function entry(key: string, name: string, title: string, file: string): GalleryEntry {
  const path = `./demos/${file}.vue`;
  return {
    key,
    name,
    title,
    file,
    loader: () => {
      const load = DEMO_MODULES[path];
      if (!load) return Promise.reject(new Error(`缺少组件文件 ${path}`));
      return load();
    },
  };
}

export const GALLERY_GROUPS: GalleryGroup[] = [
  {
    id: "basic",
    title: "基础",
    entries: [
      entry("app-icons", "AppIcon", "图标语义名总表", "AppIconsDemo"),
      entry("app-loading", "AppLoading", "加载指示器", "AppLoadingDemo"),
      entry("double-deck", "DoubleDeck", "双层卡", "DoubleDeckDemo"),
      entry("theme-cards", "AutoThemeGrid", "主题预设卡片", "ThemeCardsDemo"),
    ],
  },
  {
    id: "layout",
    title: "布局与外壳",
    entries: [
      entry("settings-shell", "SettingsShell", "设置页头部（面包屑 + 标签页）", "SettingsShellDemo"),
      entry("panel-tabs", "PanelTabStrip", "标签栏组件", "PanelTabsDemo"),
      entry("config-cards", "ConfigCards", "配置卡片画廊", "ConfigCardsDemo"),
      entry("quick-model-card", "QuickModelCard", "快速模型不可用卡片", "QuickModelCardDemo"),
      entry("templates", "ConfigTemplate", "配置模板", "ConfigTemplateDemo"),
    ],
  },
  {
    id: "form",
    title: "表单与输入",
    entries: [
      entry("composer", "ChatComposer", "输入面板新结构", "ComposerDemo"),
      entry("question", "ChatQuestionPanel", "提问卡", "QuestionDemo"),
      entry("question-tool", "ConversationQuestion", "会话提问（question 工具）", "QuestionToolDemo"),
      entry("search-engines", "SearchEngines", "搜索设置", "SearchEnginesDemo"),
    ],
  },
  {
    id: "chat",
    title: "聊天",
    entries: [
      entry("bubbles", "ChatBubbleShell", "自研气泡", "BubblesDemo"),
      entry("chat-message", "ChatMessageItem", "消息项（队伍消息气泡）", "ChatMessageDemo"),
      entry("thinking-and-tools", "ChatThinkingAndTools", "思考与工具（折叠块 + 流式压测）", "ThinkingAndToolsDemo"),
      entry("thinking-preview", "ChatThinkingPreviewBar", "思考预览 bar", "ThinkingPreviewDemo"),
      entry("rich-block-embed", "ChatRichBlockEmbed", "代码块 / mermaid 内嵌气泡", "RichBlockEmbedDemo"),
      entry("markdown-view", "AppMarkdownRenderer", "Markdown 全量渲染", "MarkdownViewDemo"),
      entry("chat-view-stream", "ChatViewStream", "对话视图流式复现（全组件）", "ChatViewStreamDemo"),
      entry("chat-view-margin", "ChatViewMargin", "对话视图滚动落点（startMargin 实测）", "ChatViewMarginDemo"),
      entry("conversation-list", "ChatConversationList", "列表卡画廊", "ConversationListDemo"),
      entry("conversation-item", "ChatConversationItem", "会话条目（状态 × 未读）", "ConversationItemDemo"),
      entry("home-cards", "HomeCardGallery", "预览卡画廊", "HomeCardsDemo"),
      entry("session-timeline", "SessionTimeline", "会话时间线（垂直）", "SessionTimelineDemo"),
      entry("session-float-dock", "SessionFloatDock", "会话悬浮操作区", "SessionFloatDockDemo"),
    ],
  },
  {
    id: "persona",
    title: "人格与队伍",
    entries: [
      entry("persona-capability", "PersonaTab", "人格主页", "PersonaCapabilityDemo"),
      entry("persona-permission", "PersonaPermission", "人格权限页（黑白名单树）", "PersonaPermissionDemo"),
      entry("team-member-tiles", "TeamMemberTiles", "队员磁贴（自适应列数）", "TeamMemberTilesDemo"),
      entry("delegates", "DelegateProgressLine", "委派进度条", "DelegatesDemo"),
      entry("chat-home", "ChatHomePanel", "首页磁贴区", "ChatHomeDemo"),
      entry("skill-managed", "SkillPanel", "技能面板（托管技能禁用态）", "SkillManagedDemo"),
    ],
  },
  {
    id: "kanban",
    title: "看板与任务",
    entries: [
      entry("kanban-board", "KanbanBoardPanel", "议题看板页", "KanbanBoardDemo"),
      entry("kanban-card", "HomeKanbanCard", "议题看板磁贴（3×3）", "KanbanCardDemo"),
      entry("overview", "MonitorOverview", "监控概览", "OverviewDemo"),
    ],
  },
  {
    id: "catalog",
    title: "能力商店",
    entries: [
      entry("catalog-store", "CatalogStore", "能力商店", "CatalogStoreDemo"),
      entry("catalog-entry-card", "CatalogEntryCard", "能力商店条目卡", "CatalogEntryCardDemo"),
    ],
  },
  {
    id: "files",
    title: "文件与媒体",
    entries: [
      entry("file-reader-preview", "FileReaderPreview", "右栏内容视图（HTML 沙箱 / 源码 / 图片）", "FileReaderPreviewDemo"),
      entry("file-reader-wrap", "FileReaderWrap", "文件阅读器换行模式", "FileReaderWrapDemo"),
      entry("image-preview-dialog", "ChatImagePreviewDialog", "图片预览百分比读数", "ImagePreviewDialogDemo"),
    ],
  },
];

export const GALLERY_ENTRIES: GalleryEntry[] = GALLERY_GROUPS.flatMap((group) => group.entries);

export const DEFAULT_GALLERY_KEY = GALLERY_ENTRIES[0]?.key ?? "";

export function findGalleryEntry(key: string | null | undefined): GalleryEntry | undefined {
  if (!key) return undefined;
  return GALLERY_ENTRIES.find((item) => item.key === key);
}
