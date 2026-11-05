import { describe, expect, it } from "vitest";
import type { SearchEngineConfigItem, SearchMode } from "../../../../types/app";
import {
  SEARCH_ENGINE_META,
  isSearchConfigDirty,
  lockableSearchEngines,
  resolveSavedSearchConfig,
  searchEngineSnapshot,
} from "./search-engine-model";

describe("搜索引擎设置页纯逻辑", () => {
  it("引擎表覆盖五个引擎且与后端内置 ID 一致", () => {
    expect(SEARCH_ENGINE_META.map((engine) => engine.id)).toEqual([
      "exa",
      "parallel",
      "firecrawl",
      "tavily",
      "deepseek",
    ]);
  });

  it("锁定候选只含填了密钥的引擎，免密钥引擎不入候选", () => {
    const lockable = lockableSearchEngines([
      { id: "exa", enabled: true },
      { id: "tavily", enabled: true, apiKey: "   " },
      { id: "deepseek", enabled: true, apiKey: "sk-deepseek" },
      { id: "parallel", enabled: true, apiKey: "pk-parallel" },
    ]);
    expect(lockable).toEqual([
      { id: "parallel", name: "Parallel" },
      { id: "deepseek", name: "DeepSeek" },
    ]);
  });

  it("一个密钥都没填时锁定候选为空", () => {
    expect(lockableSearchEngines(undefined)).toEqual([]);
    expect(lockableSearchEngines([{ id: "exa", enabled: true }])).toEqual([]);
  });

  it("密钥原样带回掩码时视为未改动", () => {
    const saved = {
      engines: [{ id: "exa", enabled: true, apiKey: "sk***ab" }],
      mode: { mode: "auto" as const },
    };
    const current = {
      engines: [{ id: "exa", enabled: true, apiKey: "sk***ab" }],
      mode: { mode: "auto" as const },
    };
    expect(isSearchConfigDirty(current, saved)).toBe(false);
  });

  it("改动启用开关、密钥或模式都算有改动", () => {
    const base = { engines: [{ id: "exa", enabled: true }], mode: { mode: "auto" as const } };

    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", enabled: false }], mode: { mode: "auto" } },
        base,
      ),
    ).toBe(true);
    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", enabled: true, apiKey: "sk-new" }], mode: { mode: "auto" } },
        base,
      ),
    ).toBe(true);
    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", enabled: true }], mode: { mode: "fixed", engineId: "exa" } },
        base,
      ),
    ).toBe(true);
  });

  it("锁定引擎 ID 变化算有改动", () => {
    expect(
      isSearchConfigDirty(
        { engines: [], mode: { mode: "fixed", engineId: "tavily" } },
        { engines: [], mode: { mode: "fixed", engineId: "exa" } },
      ),
    ).toBe(true);
  });

  it("缺字段的旧配置与空列表等价，不会一进页面就显示有改动", () => {
    expect(
      isSearchConfigDirty({ engines: undefined, mode: undefined }, { engines: [], mode: { mode: "auto" } }),
    ).toBe(false);
  });

  it("快照按 ID 排序，不受数组顺序影响", () => {
    const left = searchEngineSnapshot(
      [
        { id: "tavily", enabled: true },
        { id: "exa", enabled: true },
      ],
      { mode: "auto" },
    );
    const right = searchEngineSnapshot(
      [
        { id: "exa", enabled: true },
        { id: "tavily", enabled: true },
      ],
      { mode: "auto" },
    );
    expect(left).toEqual(right);
  });
});

describe("脏判定基准取自保存快照", () => {
  const NORMALIZED: SearchEngineConfigItem[] = [
    { id: "exa", enabled: true, apiKey: "" },
    { id: "parallel", enabled: true, apiKey: "" },
    { id: "tavily", enabled: true, apiKey: "" },
    { id: "firecrawl", enabled: false, apiKey: "" },
    { id: "deepseek", enabled: false, apiKey: "" },
  ];
  const AUTO: SearchMode = { mode: "auto" };
  const current = { engines: NORMALIZED, mode: AUTO };

  function dirtyAfter(savedJson: string, edited = current): boolean {
    return isSearchConfigDirty(edited, resolveSavedSearchConfig(savedJson, current));
  }

  it("快照里没有搜索字段时视为一致，打开页面不进脏态", () => {
    expect(dirtyAfter(JSON.stringify({ hotkey: "Alt+·" }))).toBe(false);
  });

  it("快照为空串、坏 JSON、null、数组时也不进脏态", () => {
    for (const raw of ["", "   ", "not json", "null", "[]"]) {
      expect(dirtyAfter(raw)).toBe(false);
    }
  });

  it("真改过才进脏态，还原回快照后回到干净态", () => {
    const savedJson = JSON.stringify({ searchEngines: NORMALIZED, searchMode: AUTO });
    expect(dirtyAfter(savedJson)).toBe(false);

    const edited = NORMALIZED.map((entry) =>
      entry.id === "deepseek" ? { ...entry, enabled: true, apiKey: "sk-1" } : { ...entry },
    );
    expect(dirtyAfter(savedJson, { engines: edited, mode: AUTO })).toBe(true);

    const restored = NORMALIZED.map((entry) => ({ ...entry }));
    expect(dirtyAfter(savedJson, { engines: restored, mode: AUTO })).toBe(false);
  });

  it("快照里有搜索字段但没 searchMode 时，自动模式仍算一致", () => {
    expect(dirtyAfter(JSON.stringify({ searchEngines: NORMALIZED }))).toBe(false);
  });
});
