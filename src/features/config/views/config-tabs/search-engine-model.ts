// 搜索引擎设置页的纯逻辑：引擎清单、可锁定候选、快照与脏判定。
// 拆出来是为了能直接单测；保存载荷由 use-config-core 的 buildConfigPayload 负责，这里不重复实现。

import type { AppConfig, SearchEngineConfigItem, SearchMode } from "../../../../types/app";

export type SearchEngineMeta = {
  id: string;
  name: string;
  /** 免密钥可用；firecrawl 的匿名额度已被服务端关闭，DeepSeek 必须填 key */
  keyless: boolean;
  descriptionKey: string;
  registerUrl: string;
};

/** 与后端内置引擎表一一对应；未启用的也列出来，方便随时打开。 */
export const SEARCH_ENGINE_META: SearchEngineMeta[] = [
  {
    id: "exa",
    name: "Exa",
    keyless: true,
    descriptionKey: "config.searchEngines.exaDescription",
    registerUrl: "https://dashboard.exa.ai/",
  },
  {
    id: "parallel",
    name: "Parallel",
    keyless: true,
    descriptionKey: "config.searchEngines.parallelDescription",
    registerUrl: "https://platform.parallel.ai/",
  },
  {
    id: "firecrawl",
    name: "Firecrawl",
    keyless: false,
    descriptionKey: "config.searchEngines.firecrawlDescription",
    registerUrl: "https://www.firecrawl.dev/",
  },
  {
    id: "tavily",
    name: "Tavily",
    keyless: true,
    descriptionKey: "config.searchEngines.tavilyDescription",
    registerUrl: "https://app.tavily.com/",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    keyless: false,
    descriptionKey: "config.searchEngines.deepseekDescription",
    registerUrl: "https://platform.deepseek.com/",
  },
];

function hasKey(entry: SearchEngineConfigItem | undefined): boolean {
  return String(entry?.apiKey ?? "").trim().length > 0;
}

/** 锁定模式下拉框的候选：只列填了密钥的引擎，免密钥引擎不能锁定。 */
export function lockableSearchEngines(
  engines: SearchEngineConfigItem[] | undefined,
): Array<{ id: string; name: string }> {
  return SEARCH_ENGINE_META.filter((meta) =>
    hasKey((engines ?? []).find((entry) => entry.id === meta.id)),
  ).map((meta) => ({ id: meta.id, name: meta.name }));
}

export type SearchConfigSnapshot = {
  engines: Array<{ id: string; enabled: boolean; apiKey: string }>;
  mode: string;
  engineId: string;
};

/** 回显与脏比对用的稳定快照：字段固定、排序固定、缺省值补齐。 */
export function searchEngineSnapshot(
  engines: SearchEngineConfigItem[] | undefined,
  mode: SearchMode | undefined,
): SearchConfigSnapshot {
  return {
    engines: (engines ?? [])
      .map((entry) => ({
        id: String(entry.id || "").trim(),
        enabled: !!entry.enabled,
        apiKey: String(entry.apiKey ?? "").trim(),
      }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    mode: mode?.mode === "fixed" ? "fixed" : "auto",
    engineId: mode?.mode === "fixed" ? String(mode.engineId || "").trim() : "",
  };
}

export function isSearchConfigDirty(
  current: { engines: SearchEngineConfigItem[] | undefined; mode: SearchMode | undefined },
  saved: { engines: SearchEngineConfigItem[] | undefined; mode: SearchMode | undefined },
): boolean {
  return (
    JSON.stringify(searchEngineSnapshot(current.engines, current.mode))
    !== JSON.stringify(searchEngineSnapshot(saved.engines, saved.mode))
  );
}

export type SearchConfigFields = {
  engines: SearchEngineConfigItem[] | undefined;
  mode: SearchMode | undefined;
};

/** 解析保存快照里的搜索字段；快照缺字段或解析失败返回 null。 */
export function parseSavedSearchConfig(lastSavedConfigJson: string): SearchConfigFields | null {
  let parsed: Partial<AppConfig> | null = null;
  try {
    const value = JSON.parse(String(lastSavedConfigJson || "{}")) as Partial<AppConfig>;
    parsed = value && typeof value === "object" ? value : null;
  } catch {
    parsed = null;
  }
  if (!parsed || !Array.isArray(parsed.searchEngines)) {
    return null;
  }
  return { engines: parsed.searchEngines, mode: parsed.searchMode };
}

/** 浅拷贝一份配置字段：基准不能和当前指向同一份对象，否则原地编辑两边一起变。 */
export function cloneSearchFields(fields: SearchConfigFields): SearchConfigFields {
  return {
    engines: (fields.engines ?? []).map((entry) => ({ ...entry })),
    mode: fields.mode ? { ...fields.mode } : undefined,
  };
}

/**
 * 脏判定基准的解析器。
 * 优先用保存快照；快照缺搜索字段时（首次打开、老快照、坏 JSON）取「第一次读到的当前值」
 * 并浅拷贝冻结，之后原地编辑只改当前值、不再改基准。
 */
export function createSearchBaselineResolver() {
  let frozenBaseline: SearchConfigFields | null = null;
  return (lastSavedConfigJson: string, current: SearchConfigFields): SearchConfigFields => {
    const parsed = parseSavedSearchConfig(lastSavedConfigJson);
    if (parsed) {
      frozenBaseline = null;
      return parsed;
    }
    if (!frozenBaseline) {
      frozenBaseline = cloneSearchFields(current);
    }
    return frozenBaseline;
  };
}
