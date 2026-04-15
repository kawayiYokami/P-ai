import { LOCALE_MESSAGES, type SupportedLocale } from "../../../i18n";

export type ConfigSearchTab =
  | "hotkey"
  | "api"
  | "tools"
  | "mcp"
  | "skill"
  | "persona"
  | "department"
  | "chatSettings"
  | "remoteIm"
  | "memory"
  | "task"
  | "logs"
  | "appearance"
  | "about";

export type ConfigSearchResult = {
  tab: ConfigSearchTab;
  title: string;
  matchedTexts: string[];
};

type ConfigSearchIndexEntry = {
  tab: ConfigSearchTab;
  title: string;
  texts: string[];
};

type ConfigSearchSource = {
  tab: ConfigSearchTab;
  titleKey?: string;
  titleFallback: string;
  prefixes: string[];
};

const CONFIG_SEARCH_SOURCES: ConfigSearchSource[] = [
  { tab: "hotkey", titleKey: "config.tabs.hotkey", titleFallback: "Hotkey", prefixes: ["config.hotkey"] },
  { tab: "api", titleKey: "config.tabs.api", titleFallback: "API", prefixes: ["config.api"] },
  { tab: "tools", titleKey: "config.tabs.tools", titleFallback: "Tools", prefixes: ["config.tools"] },
  { tab: "mcp", titleFallback: "MCP", prefixes: ["config.mcp"] },
  { tab: "skill", titleKey: "config.tabs.skill", titleFallback: "Skill", prefixes: [] },
  { tab: "persona", titleKey: "config.tabs.persona", titleFallback: "Persona", prefixes: ["config.persona"] },
  { tab: "department", titleKey: "config.tabs.department", titleFallback: "Department", prefixes: ["config.department"] },
  { tab: "chatSettings", titleKey: "config.tabs.chatSettings", titleFallback: "Chat", prefixes: ["config.chatSettings"] },
  { tab: "remoteIm", titleKey: "config.tabs.remoteIm", titleFallback: "Contacts", prefixes: ["config.remoteIm"] },
  { tab: "memory", titleKey: "config.tabs.memory", titleFallback: "Memory", prefixes: ["config.memory", "memory"] },
  { tab: "task", titleKey: "config.tabs.task", titleFallback: "Task", prefixes: ["config.task"] },
  { tab: "logs", titleKey: "config.tabs.logs", titleFallback: "Logs", prefixes: ["config.logs"] },
  { tab: "appearance", titleKey: "config.tabs.appearance", titleFallback: "Appearance", prefixes: ["appearance"] },
  { tab: "about", titleKey: "config.tabs.about", titleFallback: "About", prefixes: ["about"] },
];

const indexCache = new Map<SupportedLocale, ConfigSearchIndexEntry[]>();

function getNodeByPath(root: unknown, path: string): unknown {
  const segments = String(path || "").split(".").filter(Boolean);
  let current: unknown = root;
  for (const segment of segments) {
    if (!current || typeof current !== "object" || !(segment in (current as Record<string, unknown>))) {
      return undefined;
    }
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

function flattenStrings(node: unknown, bucket: string[]) {
  if (typeof node === "string") {
    const text = node.trim();
    if (text) bucket.push(text);
    return;
  }
  if (Array.isArray(node)) {
    for (const item of node) flattenStrings(item, bucket);
    return;
  }
  if (!node || typeof node !== "object") return;
  for (const value of Object.values(node as Record<string, unknown>)) {
    flattenStrings(value, bucket);
  }
}

function uniqueStrings(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    const normalized = value.replace(/\s+/g, " ").trim();
    if (!normalized || seen.has(normalized)) continue;
    seen.add(normalized);
    result.push(normalized);
  }
  return result;
}

function resolveTextByKey(messages: unknown, key?: string, fallback = ""): string {
  if (!key) return fallback;
  const value = getNodeByPath(messages, key);
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function buildConfigSearchIndex(locale: SupportedLocale): ConfigSearchIndexEntry[] {
  const cached = indexCache.get(locale);
  if (cached) return cached;
  const messages = LOCALE_MESSAGES[locale];
  const next = CONFIG_SEARCH_SOURCES.map((source) => {
    const title = resolveTextByKey(messages, source.titleKey, source.titleFallback);
    const texts = uniqueStrings([
      title,
      ...source.prefixes.flatMap((prefix) => {
        const bucket: string[] = [];
        flattenStrings(getNodeByPath(messages, prefix), bucket);
        return bucket;
      }),
    ]);
    return {
      tab: source.tab,
      title,
      texts,
    };
  });
  indexCache.set(locale, next);
  return next;
}

function normalizeSearchText(value: string): string {
  return String(value || "").toLocaleLowerCase().replace(/\s+/g, " ").trim();
}

export function searchConfigTabs(query: string, locale: SupportedLocale): ConfigSearchResult[] {
  const normalizedQuery = normalizeSearchText(query);
  if (!normalizedQuery) return [];
  const tokens = normalizedQuery.split(" ").filter(Boolean);
  if (tokens.length === 0) return [];

  return buildConfigSearchIndex(locale)
    .map((entry) => {
      const normalizedTexts = entry.texts.map((text) => ({
        raw: text,
        normalized: normalizeSearchText(text),
      }));
      const combinedText = normalizedTexts.map((item) => item.normalized).join("\n");
      if (!tokens.every((token) => combinedText.includes(token))) {
        return null;
      }
      const matchedTexts = uniqueStrings(
        normalizedTexts
          .filter((item) => tokens.some((token) => item.normalized.includes(token)))
          .map((item) => item.raw),
      );
      const titleNormalized = normalizeSearchText(entry.title);
      const titleHitCount = tokens.filter((token) => titleNormalized.includes(token)).length;
      const exactTitle = titleNormalized === normalizedQuery ? 1000 : 0;
      const prefixTitle = titleNormalized.startsWith(normalizedQuery) ? 200 : 0;
      const score = exactTitle + prefixTitle + titleHitCount * 10 + matchedTexts.length;
      return {
        tab: entry.tab,
        title: entry.title,
        matchedTexts: matchedTexts.slice(0, 2),
        score,
      };
    })
    .filter((item): item is ConfigSearchResult & { score: number } => !!item)
    .sort((left, right) => {
      if (right.score !== left.score) return right.score - left.score;
      return left.title.localeCompare(right.title, locale === "en-US" ? "en" : "zh-CN");
    })
    .map(({ score: _score, ...result }) => result);
}
