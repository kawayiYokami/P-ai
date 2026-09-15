import type { ChatConversationOverviewItem } from "../../../types/app";
import { defaultWorkspaceNameFromPath } from "../../../utils/shell-workspaces";

export type ConversationSection = {
  key: string;
  title: string;
  items: ChatConversationOverviewItem[];
  workspaceRootPath?: string;
};

export type ConversationSectionOrderState = {
  local: string[];
  contact: string[];
};

export type ConversationSidebarTab = "local" | "contact" | "task";

export type ConversationSectionTitles = {
  recent: string;
  pinned: string;
  other: string;
  defaultWorkspace: string;
  currentProject: string;
};

export function buildConversationSections(
  items: ChatConversationOverviewItem[],
  options: {
    tab: ConversationSidebarTab;
    titles: ConversationSectionTitles;
    locale?: string | string[];
    currentWorkspaceRootPath?: string;
    activeConversationId?: string;
    /** 精简模式：忽略置顶 / 当前项目 / 最近，全部会话统一按工作区（本地）或渠道（联系人）分组 */
    compact?: boolean;
  },
): ConversationSection[] {
  const { tab, titles, locale } = options;
  const normalizedActiveId = String(options.activeConversationId || "").trim();
  const visibleItems = items.filter((item) => {
    const kind = String(item.kind || "local_unarchived").trim();
    const kindMatched = tab === "contact"
      ? kind === "remote_im_contact"
      : kind !== "remote_im_contact";
    if (!kindMatched) return false;
    // 会话草稿默认隐藏：仅当它正是当前打开的会话时进入「最近会话」，
    // 且不参与置顶、当前项目与工作区分组。
    if (item.isDraft && String(item.conversationId || "").trim() !== normalizedActiveId) {
      return false;
    }
    return true;
  });
  // 精简模式：忽略置顶 / 当前项目 / 最近，全部会话统一按工作区（本地）或渠道（联系人）分组
  if (options.compact) {
    if (tab === "contact") {
      return buildRemoteConversationSections(visibleItems, {
        fallbackTitle: titles.other,
        locale,
      });
    }
    return buildWorkspaceConversationSections(visibleItems, {
      defaultWorkspaceTitle: titles.defaultWorkspace,
      locale,
    });
  }
  const draftActiveItems = visibleItems.filter((item) => !!item.isDraft);
  const regularItems = visibleItems.filter((item) => !item.isDraft);
  const pinned = regularItems.filter((item) => !!item.isPinned || !!item.isSystemNotificationConversation);
  const others = regularItems.filter((item) => !item.isPinned && !item.isSystemNotificationConversation);

  // 「当前项目」分组：把属于当前工作区路径的会话单独列出，
  // 并从最近会话与其他工作区分组中剔除，避免重复显示。
  // 工作树感知：host 与会话路径若落在 {gitRoot}/.pai/.worktree/{id} 内，先回溯到 gitRoot 再比较。
  const currentWorkspacePath = String(options.currentWorkspaceRootPath || "").trim();
  const normalizedCurrentWorkspacePath = canonicalWorkspaceRootForComparison(currentWorkspacePath);
  const isCurrentProjectItem = (item: ChatConversationOverviewItem) =>
    !!normalizedCurrentWorkspacePath
    && canonicalWorkspaceRootForComparison(String(item.workspaceRootPath || "").trim()) === normalizedCurrentWorkspacePath;
  const currentProjectItems = others.filter(isCurrentProjectItem);
  const restOthers = others.filter((item) => !isCurrentProjectItem(item));

  const recentSection = buildRecentConversationSection([...draftActiveItems, ...restOthers], titles.recent);
  const sections: ConversationSection[] = [];
  if (pinned.length > 0) {
    sections.push({
      key: "pinned",
      title: titles.pinned,
      items: pinned,
    });
  }
  if (!!normalizedCurrentWorkspacePath) {
    sections.push({
      key: CURRENT_PROJECT_SECTION_KEY,
      title: titles.currentProject,
      workspaceRootPath: currentWorkspacePath,
      items: currentProjectItems,
    });
  }
  if (recentSection) {
    sections.push(recentSection);
  }
  if (tab === "contact") {
    return [
      ...sections,
      ...buildRemoteConversationSections(restOthers, {
        fallbackTitle: titles.other,
        locale,
      }),
    ];
  }
  return [
    ...sections,
    ...buildWorkspaceConversationSections(restOthers, {
      defaultWorkspaceTitle: titles.defaultWorkspace,
      locale,
    }),
  ];
}

export const RECENT_CONVERSATION_SECTION_KEY = "recent";
export const CURRENT_PROJECT_SECTION_KEY = "current-project";

function buildRecentConversationSection(
  items: ChatConversationOverviewItem[],
  title: string,
): ConversationSection | null {
  const seenIds = new Set<string>();
  const recentItems = [...items]
    .sort((left, right) => conversationRecencyMs(right) - conversationRecencyMs(left))
    .filter((item) => {
      const id = String(item.conversationId || "").trim();
      if (!id || seenIds.has(id)) return false;
      seenIds.add(id);
      return true;
    });
  if (recentItems.length === 0) return null;
  return {
    key: RECENT_CONVERSATION_SECTION_KEY,
    title,
    items: recentItems,
  };
}

type BuildWorkspaceConversationSectionsOptions = {
  defaultWorkspaceTitle: string;
  locale?: string | string[];
};

type BuildRemoteConversationSectionsOptions = {
  fallbackTitle: string;
  locale?: string | string[];
};

function normalizeWorkspaceSectionPath(path: string): string {
  let normalized = String(path || "").trim();
  // 剥掉 Windows 扩展长度前缀（\\?\C:\...、\\?\UNC\server\share）与设备前缀（\\.\C:\...），
  // 它们与普通路径指向同一位置，比较时必须等价对待。
  normalized = normalized
    .replace(/^\\\\\?\\unc\\/i, "//")
    .replace(/^\\\\\?\\/i, "")
    .replace(/^\\\\.\\/i, "");
  return normalized.replace(/\\/g, "/").replace(/\/+$/, "").toLocaleLowerCase();
}

export function canonicalWorkspaceRootForComparison(path: string): string {
  const normalized = normalizeWorkspaceSectionPath(path);
  if (!normalized) return "";
  // 单文件单工作树固定在 {gitRoot}/.pai/.worktree/{id}（兼容 legacy 8 位），
  // 打开工作树目录时回溯到 gitRoot，保证 host 与会话能归到同一「当前项目」。
  const marker = "/.pai/.worktree/";
  const markerIndex = normalized.indexOf(marker);
  if (markerIndex !== -1) return normalized.slice(0, markerIndex);
  const suffix = "/.pai/.worktree";
  if (normalized.endsWith(suffix)) return normalized.slice(0, normalized.length - suffix.length);
  return normalized;
}

function compareWorkspaceSectionText(left: string, right: string, locale?: string | string[]): number {
  return left.localeCompare(right, locale, {
    numeric: true,
    sensitivity: "base",
  });
}

function conversationRecencyMs(item: ChatConversationOverviewItem): number {
  const raw = String(item.lastMessageAt || item.updatedAt || "").trim();
  if (!raw) return 0;
  const time = Date.parse(raw);
  return Number.isFinite(time) ? time : 0;
}

/**
 * 统计「最近一个凌晨 4 点至今」活跃过的会话数。
 * items 需按最近活跃降序；计数从头部开始，遇到早于阈值的条目即停止。
 */
export function conversationCountSinceDayStart(items: ChatConversationOverviewItem[], nowMs: number = Date.now()): number {
  const dayStart = new Date(nowMs);
  dayStart.setHours(4, 0, 0, 0);
  if (dayStart.getTime() > nowMs) {
    dayStart.setDate(dayStart.getDate() - 1);
  }
  const dayStartMs = dayStart.getTime();
  let count = 0;
  for (const item of items) {
    if (conversationRecencyMs(item) < dayStartMs) break;
    count += 1;
  }
  return count;
}

export function workspaceNameFromPath(path: string): string {
  return defaultWorkspaceNameFromPath(path);
}

export function applyConversationSectionOrder(
  sections: ConversationSection[],
  savedOrder: string[],
): { sections: ConversationSection[]; nextOrder: string[]; changed: boolean } {
  const normalizedSavedOrder = Array.isArray(savedOrder)
    ? savedOrder.map((item) => String(item || "").trim()).filter(Boolean)
    : [];
  const sectionByKey = new Map(sections.map((section) => [section.key, section] as const));
  const orderedSections: ConversationSection[] = [];
  const nextOrder: string[] = [];

  // 固定分区（置顶 / 当前项目 / 最近）不参与用户排序，始终按固定顺序排在最前。
  const FIXED_SECTION_KEYS = ["pinned", CURRENT_PROJECT_SECTION_KEY, RECENT_CONVERSATION_SECTION_KEY];
  const fixedKeySet = new Set(FIXED_SECTION_KEYS);

  for (const key of FIXED_SECTION_KEYS) {
    const section = sectionByKey.get(key);
    if (!section) continue;
    orderedSections.push(section);
    nextOrder.push(key);
    sectionByKey.delete(key);
  }

  for (const key of normalizedSavedOrder) {
    if (fixedKeySet.has(key)) continue;
    const section = sectionByKey.get(key);
    if (!section) continue;
    orderedSections.push(section);
    nextOrder.push(key);
    sectionByKey.delete(key);
  }

  for (const section of sections) {
    if (!sectionByKey.has(section.key)) continue;
    orderedSections.push(section);
    nextOrder.push(section.key);
    sectionByKey.delete(section.key);
  }

  const changed = nextOrder.length !== normalizedSavedOrder.length
    || nextOrder.some((key, index) => key !== normalizedSavedOrder[index]);

  return {
    sections: orderedSections,
    nextOrder,
    changed,
  };
}

function resolveWorkspaceSectionTitle(
  currentTitle: string,
  nextTitle: string,
  workspaceRootPath: string,
): string {
  const current = String(currentTitle || "").trim();
  const next = String(nextTitle || "").trim();
  if (!current) return next;
  if (!next) return current;
  if (current === next) return current;

  const fallback = workspaceNameFromPath(workspaceRootPath);
  const currentIsFallback = !!fallback && current.localeCompare(fallback, undefined, { sensitivity: "accent" }) === 0;
  const nextIsFallback = !!fallback && next.localeCompare(fallback, undefined, { sensitivity: "accent" }) === 0;

  if (currentIsFallback && !nextIsFallback) return next;
  if (nextIsFallback && !currentIsFallback) return current;

  return current.length >= next.length ? current : next;
}

function resolveRemoteConversationSectionMeta(
  item: ChatConversationOverviewItem,
  fallbackTitle: string,
): { channelId: string; channelName: string; hasChannel: boolean; title: string; key: string } {
  const channelId = String(item.channelId || "").trim();
  const channelName = String(item.channelName || "").trim();
  const hasChannel = !!(channelId || channelName);
  const title = channelName || channelId || fallbackTitle;
  return {
    channelId,
    channelName,
    hasChannel,
    title,
    key: `channel:${channelId || channelName || "__fallback__"}`,
  };
}

export function buildWorkspaceConversationSections(
  items: ChatConversationOverviewItem[],
  options: BuildWorkspaceConversationSectionsOptions,
): ConversationSection[] {
  const sections: ConversationSection[] = [];
  const byWorkspace = new Map<string, ConversationSection>();
  for (const item of items) {
    const path = String(item.workspaceRootPath || "").trim();
    const normalizedPath = normalizeWorkspaceSectionPath(path);
    const title = String(item.workspaceLabel || "").trim()
      || workspaceNameFromPath(path)
      || options.defaultWorkspaceTitle;
    const key = `workspace:${normalizedPath || "__default__"}`;
    const existing = byWorkspace.get(key);
    if (existing) {
      existing.title = resolveWorkspaceSectionTitle(existing.title, title, path || existing.workspaceRootPath || "");
      existing.items.push(item);
      continue;
    }
    const section = {
      key,
      title,
      workspaceRootPath: path || undefined,
      items: [item],
    };
    byWorkspace.set(key, section);
    sections.push(section);
  }
  return sections.sort((left, right) => {
    const leftPath = normalizeWorkspaceSectionPath(left.workspaceRootPath || "");
    const rightPath = normalizeWorkspaceSectionPath(right.workspaceRootPath || "");
    if (!!leftPath !== !!rightPath) {
      return leftPath ? -1 : 1;
    }
    return compareWorkspaceSectionText(leftPath || left.title, rightPath || right.title, options.locale)
      || compareWorkspaceSectionText(left.title, right.title, options.locale)
      || compareWorkspaceSectionText(left.key, right.key, options.locale);
  });
}

export function buildRemoteConversationSections(
  items: ChatConversationOverviewItem[],
  options: BuildRemoteConversationSectionsOptions,
): ConversationSection[] {
  const byChannel = new Map<string, {
    section: ConversationSection;
    hasChannel: boolean;
    sortTitle: string;
    sortKey: string;
  }>();
  for (const item of items) {
    const { channelId, channelName, hasChannel, title, key } = resolveRemoteConversationSectionMeta(
      item,
      options.fallbackTitle,
    );
    const existing = byChannel.get(key);
    if (existing) {
      existing.section.items.push(item);
      continue;
    }
    byChannel.set(key, {
      section: {
        key,
        title,
        items: [item],
      },
      hasChannel,
      sortTitle: title,
      sortKey: channelId || channelName || title,
    });
  }
  return Array.from(byChannel.values())
    .sort((left, right) => {
      if (left.hasChannel !== right.hasChannel) {
        return left.hasChannel ? -1 : 1;
      }
      return compareWorkspaceSectionText(left.sortTitle, right.sortTitle, options.locale)
        || compareWorkspaceSectionText(left.sortKey, right.sortKey, options.locale);
    })
    .map((entry) => entry.section);
}
