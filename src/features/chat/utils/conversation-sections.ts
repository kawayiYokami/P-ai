import type { ChatConversationOverviewItem } from "../../../types/app";
import { defaultWorkspaceNameFromPath } from "../../../utils/shell-workspaces";

export type ConversationSection = {
  key: string;
  title: string;
  items: ChatConversationOverviewItem[];
  workspaceRootPath?: string;
  /** 人格分组的 agentId，用于取人格头像；非人格分组为空 */
  personaId?: string;
  /** 联系人页的顶层层级：IM 渠道段或 ACP 工作目录段；本地与任务页为空 */
  group?: "remote_im" | "acp";
};

export type ConversationSectionOrderState = {
  local: string[];
  contact: string[];
};

export type ConversationSidebarTab = "local" | "contact" | "task";

/** 侧边栏第三个父节点的分组依据：人格 / 工作目录 / 混合（= 工作目录 + 富卡片渲染） */
export type ConversationSectionGrouping = "persona" | "workspace" | "mixed";

export type ConversationSectionTitles = {
  recent: string;
  pinned: string;
  other: string;
  defaultWorkspace: string;
  currentProject: string;
  unknownPersona: string;
};

export function buildConversationSections(
  items: ChatConversationOverviewItem[],
  options: {
    tab: ConversationSidebarTab;
    titles: ConversationSectionTitles;
    locale?: string | string[];
    currentWorkspaceRootPath?: string;
    activeConversationId?: string;
    /** 第三个父节点的分组依据，默认 mixed（= 工作目录 + 富卡片渲染） */
    grouping?: ConversationSectionGrouping;
    /** 人格名映射（agentId -> 展示名），人格模式用于取父节点标题 */
    personaNameMap?: Record<string, string>;
  },
): ConversationSection[] {
  const { tab, titles, locale } = options;
  const grouping = options.grouping ?? "mixed";
  const isPersonaGrouping = grouping === "persona";
  const normalizedActiveId = String(options.activeConversationId || "").trim();
  const visibleItems = items.filter((item) => {
    const kind = String(item.kind || "local_unarchived").trim();
    // 联系人页承载两类会话：IM 渠道会话与 ACP 会话；本地列表两者都不进。
    const isContactKind = kind === "remote_im_contact" || kind === "acp_session";
    const kindMatched = tab === "contact" ? isContactKind : !isContactKind;
    if (!kindMatched) return false;
    // 会话草稿默认隐藏：仅当它正是当前打开的会话时进入「最近会话」，
    // 且不参与置顶、当前项目与工作区分组。
    if (item.isDraft && String(item.conversationId || "").trim() !== normalizedActiveId) {
      return false;
    }
    return true;
  });
  if (tab === "contact") {
    return buildRemoteConversationSections(visibleItems, {
      fallbackTitle: titles.other,
      locale,
      pinnedFirst: true,
    });
  }

  const draftActiveItems = visibleItems.filter((item) => !!item.isDraft);
  const regularItems = visibleItems.filter((item) => !item.isDraft);
  // 「置顶 / 系统通知」区不参与分组：只把系统通知会话单独列出（不折叠、不带置顶图标），
  // 其余置顶会话留在所属项目 / 人格 / 渠道分组内，组内置顶优先展示。
  const pinned = regularItems.filter((item) => !!item.isSystemNotificationConversation);
  const others = regularItems.filter((item) => !item.isSystemNotificationConversation);

  // 「当前项目」分组：把属于当前工作区路径的会话单独列出，
  // 并从最近会话与其他工作区分组中剔除，避免重复显示。
  // 工作树感知：host 与会话路径若落在 {gitRoot}/.pai/.worktree/{id} 内，先回溯到 gitRoot 再比较。
  const currentWorkspacePath = String(options.currentWorkspaceRootPath || "").trim();
  const normalizedCurrentWorkspacePath = canonicalWorkspaceRootForComparison(currentWorkspacePath);
  const isCurrentProjectItem = (item: ChatConversationOverviewItem) =>
    !!normalizedCurrentWorkspacePath
    && canonicalWorkspaceRootForComparison(String(item.workspaceRootPath || "").trim()) === normalizedCurrentWorkspacePath;
  // 人格模式以人格为父节点，不引入工作区概念，因而没有「当前项目」分组
  const currentProjectItems = isPersonaGrouping ? [] : others.filter(isCurrentProjectItem);
  const restOthers = isPersonaGrouping ? others : others.filter((item) => !isCurrentProjectItem(item));

  // 最近会话始终包含置顶会话（含落在「当前项目」里的置顶会话），保证置顶项永远能从最近区找到
  const recentSection = buildRecentConversationSection(
    [...draftActiveItems, ...restOthers, ...others.filter((item) => !!item.isPinned)],
    titles.recent,
  );
  const sections: ConversationSection[] = [];
  if (pinned.length > 0) {
    sections.push({
      key: "pinned",
      title: titles.pinned,
      items: pinned,
    });
  }
  if (!isPersonaGrouping && !!normalizedCurrentWorkspacePath) {
    sections.push({
      key: CURRENT_PROJECT_SECTION_KEY,
      title: titles.currentProject,
      workspaceRootPath: currentWorkspacePath,
      items: sortConversationItemsPinnedFirst(currentProjectItems),
    });
  }
  if (recentSection && !isPersonaGrouping) {
    sections.push(recentSection);
  }
  // 人格模式下没有「最近会话」，草稿会话并入人格分组，避免新建会话从列表消失
  if (isPersonaGrouping) {
    return [
      ...sections,
      ...buildPersonaConversationSections([...draftActiveItems, ...restOthers], {
        fallbackTitle: titles.unknownPersona,
        personaNameMap: options.personaNameMap,
        locale,
      }),
    ];
  }
  return [
    ...sections,
    ...buildWorkspaceConversationSections(restOthers, {
      defaultWorkspaceTitle: titles.defaultWorkspace,
      locale,
      pinnedFirst: true,
    }),
  ];
}

export function buildRecentConversationSections(
  items: ChatConversationOverviewItem[],
  options: {
    titles: ConversationSectionTitles;
    locale?: string | string[];
    currentWorkspaceRootPath?: string;
    activeConversationId?: string;
    grouping?: ConversationSectionGrouping;
    personaNameMap?: Record<string, string>;
    maxCount?: number;
  },
): ConversationSection[] {
  const { titles, locale } = options;
  const isPersonaGrouping = options.grouping === "persona";
  const normalizedActiveId = String(options.activeConversationId || "").trim();

  const candidates = items.filter((item) => {
    const itemKind = String(item.kind || "local_unarchived").trim();
    if (itemKind === "remote_im_contact" || itemKind === "acp_session") return false;
    if (item.isSystemNotificationConversation) return false;
    if (item.isDraft && String(item.conversationId || "").trim() !== normalizedActiveId) return false;
    return true;
  });

  if (candidates.length === 0) return [];

  const seenIds = new Set<string>();
  const sortedCandidates = candidates
    .sort((left, right) => conversationRecencyMs(right) - conversationRecencyMs(left))
    .filter((item) => {
      const id = String(item.conversationId || "").trim();
      if (!id || seenIds.has(id)) return false;
      seenIds.add(id);
      return true;
    });

  const limitedCandidates = (typeof options.maxCount === "number" && options.maxCount > 0)
    ? sortedCandidates.slice(0, options.maxCount)
    : sortedCandidates;

  if (limitedCandidates.length === 0) return [];

  if (isPersonaGrouping) {
    const byPersona = new Map<string, { title: string; personaId?: string; items: ChatConversationOverviewItem[] }>();
    for (const item of limitedCandidates) {
      const agentId = String(item.agentId || "").trim();
      const title = String(options.personaNameMap?.[agentId] || "").trim() || agentId || titles.unknownPersona;
      const key = `recent:persona:${agentId || "__unknown__"}`;
      const existing = byPersona.get(key);
      if (existing) {
        existing.items.push(item);
      } else {
        byPersona.set(key, { title, personaId: agentId || undefined, items: [item] });
      }
    }
    return Array.from(byPersona.entries())
      .map(([key, group]) => ({
        key,
        title: group.title,
        personaId: group.personaId,
        items: sortConversationItemsPinnedFirst(group.items),
        recency: group.items.reduce((max, entry) => Math.max(max, conversationRecencyMs(entry)), 0),
      }))
      .sort((left, right) => right.recency - left.recency)
      .map(({ recency: _recency, ...section }) => section);
  }

  const currentWorkspacePath = String(options.currentWorkspaceRootPath || "").trim();
  const normalizedCurrentWorkspacePath = canonicalWorkspaceRootForComparison(currentWorkspacePath);

  const byWorkspace = new Map<string, { title: string; workspaceRootPath?: string; items: ChatConversationOverviewItem[]; isCurrent: boolean }>();
  for (const item of limitedCandidates) {
    const path = String(item.workspaceRootPath || "").trim();
    const normalizedPath = canonicalWorkspaceRootForComparison(path);
    const isCurrent = !!normalizedCurrentWorkspacePath && normalizedPath === normalizedCurrentWorkspacePath;
    const key = isCurrent
      ? "recent:current-project"
      : `recent:workspace:${normalizedPath || "__default__"}`;
    const title = isCurrent
      ? titles.currentProject
      : (String(item.workspaceLabel || "").trim() || workspaceNameFromPath(path) || titles.defaultWorkspace);

    const existing = byWorkspace.get(key);
    if (existing) {
      existing.items.push(item);
    } else {
      byWorkspace.set(key, {
        title,
        workspaceRootPath: isCurrent ? currentWorkspacePath : (path || undefined),
        items: [item],
        isCurrent,
      });
    }
  }

  const sectionsWithRecency = Array.from(byWorkspace.entries()).map(([key, group]) => ({
    key,
    title: group.title,
    workspaceRootPath: group.workspaceRootPath,
    items: sortConversationItemsPinnedFirst(group.items),
    isCurrent: group.isCurrent,
    recency: group.items.reduce((max, entry) => Math.max(max, conversationRecencyMs(entry)), 0),
  }));

  return sectionsWithRecency
    .sort((left, right) => {
      if (left.isCurrent !== right.isCurrent) return left.isCurrent ? -1 : 1;
      return right.recency - left.recency;
    })
    .map(({ recency: _recency, isCurrent: _isCurrent, ...section }) => section);
}

export function buildCategoryConversationSections(
  items: ChatConversationOverviewItem[],
  options: {
    titles: ConversationSectionTitles;
    locale?: string | string[];
    grouping?: ConversationSectionGrouping;
    personaNameMap?: Record<string, string>;
  },
): ConversationSection[] {
  const { titles, locale } = options;
  const isPersonaGrouping = options.grouping === "persona";

  const regularItems = items.filter((item) => {
    const itemKind = String(item.kind || "local_unarchived").trim();
    if (itemKind === "remote_im_contact" || itemKind === "acp_session") return false;
    if (item.isSystemNotificationConversation || item.isDraft) return false;
    return true;
  });

  if (isPersonaGrouping) {
    return buildPersonaConversationSections(regularItems, {
      fallbackTitle: titles.unknownPersona,
      personaNameMap: options.personaNameMap,
      locale,
    });
  }

  return buildWorkspaceConversationSections(regularItems, {
    defaultWorkspaceTitle: titles.defaultWorkspace,
    locale,
    pinnedFirst: true,
  });
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

/** 组内置顶优先，其余按最近活跃时间倒序 */
function sortConversationItemsPinnedFirst(items: ChatConversationOverviewItem[]): ChatConversationOverviewItem[] {
  return [...items].sort((left, right) => {
    const leftPinned = left.isPinned ? 1 : 0;
    const rightPinned = right.isPinned ? 1 : 0;
    if (leftPinned !== rightPinned) return rightPinned - leftPinned;
    return conversationRecencyMs(right) - conversationRecencyMs(left);
  });
}

type BuildWorkspaceConversationSectionsOptions = {
  defaultWorkspaceTitle: string;
  locale?: string | string[];
  /** 项目模式：置顶会话留在组内，需置顶优先排序 */
  pinnedFirst?: boolean;
};

type BuildRemoteConversationSectionsOptions = {
  fallbackTitle: string;
  locale?: string | string[];
  pinnedFirst?: boolean;
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
  // 单文件单工作树固定在 {gitRoot}/.pai/.worktree/{id}（兼容 legacy 8 位与无前导点变体），
  // 打开工作树目录时回溯到 gitRoot，保证 host 与会话能归到同一「当前项目」。
  for (const marker of ["/.pai/.worktree/", "/.pai/worktree/"]) {
    const markerIndex = normalized.indexOf(marker);
    if (markerIndex !== -1) return normalized.slice(0, markerIndex);
  }
  for (const suffix of ["/.pai/.worktree", "/.pai/worktree"]) {
    if (normalized.endsWith(suffix)) return normalized.slice(0, normalized.length - suffix.length);
  }
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
 * 统计最近 activeHours 小时内活跃过的会话数，用于决定「最近」大区的初始展示量。
 * items 需按最近活跃降序；计数从头部开始，遇到早于窗口的条目即停止。
 */
export function conversationCountWithinHours(
  items: ChatConversationOverviewItem[],
  activeHours: number,
  nowMs: number = Date.now(),
): number {
  if (!(activeHours > 0)) return items.length;
  const cutoffMs = nowMs - activeHours * 3600 * 1000;
  let count = 0;
  for (const item of items) {
    if (conversationRecencyMs(item) < cutoffMs) break;
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
  if (options.pinnedFirst) {
    for (const section of sections) {
      section.items = sortConversationItemsPinnedFirst(section.items);
    }
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
        group: "remote_im",
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
    .map((entry) => entry.section)
    .map((section) => (options.pinnedFirst ? { ...section, items: sortConversationItemsPinnedFirst(section.items) } : section));
}

type BuildAcpConversationSectionsOptions = {
  fallbackTitle: string;
  locale?: string | string[];
  pinnedFirst?: boolean;
};

/** ACP 分组：以会话 cwd（workspaceRootPath）为父节点，标题取目录名；cwd 缺失时统一回退到 fallbackTitle。 */
export function buildAcpConversationSections(
  items: ChatConversationOverviewItem[],
  options: BuildAcpConversationSectionsOptions,
): ConversationSection[] {
  const byWorkspace = new Map<string, { section: ConversationSection; sortTitle: string }>();
  for (const item of items) {
    const rawPath = String(item.workspaceRootPath || "").trim();
    const normalizedPath = normalizeWorkspaceSectionPath(rawPath);
    const title = (rawPath ? defaultWorkspaceNameFromPath(rawPath) : "") || options.fallbackTitle;
    const key = `acp:${normalizedPath || "__unknown__"}`;
    const existing = byWorkspace.get(key);
    if (existing) {
      existing.section.items.push(item);
      continue;
    }
    byWorkspace.set(key, {
      section: {
        key,
        title,
        items: [item],
        workspaceRootPath: rawPath || undefined,
        group: "acp",
      },
      sortTitle: title,
    });
  }
  return Array.from(byWorkspace.values())
    .sort((left, right) => compareWorkspaceSectionText(left.sortTitle, right.sortTitle, options.locale))
    .map((entry) => entry.section)
    .map((section) => (options.pinnedFirst ? { ...section, items: sortConversationItemsPinnedFirst(section.items) } : section));
}

type BuildPersonaConversationSectionsOptions = {
  fallbackTitle: string;
  personaNameMap?: Record<string, string>;
  locale?: string | string[];
};

/** 人格分组：以会话的 agentId 作为父节点，标题取人格名（缺名时回退 agentId，再无则回退 fallbackTitle） */
export function buildPersonaConversationSections(
  items: ChatConversationOverviewItem[],
  options: BuildPersonaConversationSectionsOptions,
): ConversationSection[] {
  const byPersona = new Map<string, ConversationSection>();
  for (const item of items) {
    const agentId = String(item.agentId || "").trim();
    const title = String(options.personaNameMap?.[agentId] || "").trim() || agentId || options.fallbackTitle;
    const key = `persona:${agentId || "__unknown__"}`;
    const existing = byPersona.get(key);
    if (existing) {
      existing.items.push(item);
      continue;
    }
    byPersona.set(key, { key, title, items: [item], personaId: agentId || undefined });
  }
  // 组内置顶优先，其余按最近活跃时间倒序
  for (const section of byPersona.values()) {
    section.items = sortConversationItemsPinnedFirst(section.items);
  }
  return Array.from(byPersona.values())
    .sort((left, right) => compareWorkspaceSectionText(left.title, right.title, options.locale)
      || compareWorkspaceSectionText(left.key, right.key, options.locale));
}
