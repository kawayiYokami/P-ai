import { describe, expect, it } from "vitest";
import type { SearchEngineConfigItem, SearchMode } from "../../../../types/app";
import {
  SEARCH_ENGINE_META,
  createSearchBaselineResolver,
  isSearchConfigDirty,
  lockableSearchEngines,
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

  it("锁定候选只含填了密钥的引擎，多 key 只要有非空项就算", () => {
    const lockable = lockableSearchEngines([
      { id: "exa", apiKeys: [] },
      { id: "tavily", apiKeys: ["   "], },
      { id: "deepseek", apiKeys: ["sk-deepseek", "sk-deepseek-2"] },
      { id: "parallel", apiKeys: ["pk-parallel"] },
    ]);
    expect(lockable).toEqual([
      { id: "parallel", name: "Parallel" },
      { id: "deepseek", name: "DeepSeek" },
    ]);
  });

  it("一个密钥都没填时锁定候选为空", () => {
    expect(lockableSearchEngines(undefined)).toEqual([]);
    expect(lockableSearchEngines([{ id: "exa", apiKeys: [] }])).toEqual([]);
    expect(lockableSearchEngines([{ id: "exa" }])).toEqual([]);
  });

  it("密钥原样带回掩码时视为未改动", () => {
    const saved = {
      engines: [{ id: "exa", apiKeys: ["sk***ab", "cd***ef"] }],
      mode: { mode: "auto" as const },
    };
    const current = {
      engines: [{ id: "exa", apiKeys: ["sk***ab", "cd***ef"] }],
      mode: { mode: "auto" as const },
    };
    expect(isSearchConfigDirty(current, saved)).toBe(false);
  });

  it("增删密钥、填入密钥或切换调度都算有改动", () => {
    const base = { engines: [{ id: "exa", apiKeys: [] }], mode: { mode: "auto" as const } };

    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", apiKeys: ["sk-new"] }], mode: { mode: "auto" } },
        base,
      ),
    ).toBe(true);
    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", apiKeys: ["", ""] }], mode: { mode: "auto" } },
        base,
      ),
    ).toBe(true);
    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", apiKeys: [] }], mode: { mode: "fixed", engineId: "exa" } },
        base,
      ),
    ).toBe(true);
  });

  it("密钥顺序参与比对：挪动顺序算改动，因为轮换顺序变了", () => {
    expect(
      isSearchConfigDirty(
        { engines: [{ id: "exa", apiKeys: ["b", "a"] }], mode: { mode: "auto" } },
        { engines: [{ id: "exa", apiKeys: ["a", "b"] }], mode: { mode: "auto" } },
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

  it("快照按 ID 排序，不受引擎数组顺序影响", () => {
    const left = searchEngineSnapshot(
      [
        { id: "tavily", apiKeys: ["tv"] },
        { id: "exa", apiKeys: [] },
      ],
      { mode: "auto" },
    );
    const right = searchEngineSnapshot(
      [
        { id: "exa", apiKeys: [] },
        { id: "tavily", apiKeys: ["tv"] },
      ],
      { mode: "auto" },
    );
    expect(left).toEqual(right);
  });

  it("快照保留每个引擎的多条密钥", () => {
    const snapshot = searchEngineSnapshot(
      [{ id: "exa", apiKeys: [" a ", "b"] }],
      { mode: "auto" },
    );
    expect(snapshot.engines).toEqual([{ id: "exa", apiKeys: ["a", "b"] }]);
  });
});

describe("脏判定基准取自保存快照", () => {
  function engines(): SearchEngineConfigItem[] {
    return [
      { id: "exa", apiKeys: [] },
      { id: "parallel", apiKeys: [] },
      { id: "tavily", apiKeys: [] },
      { id: "firecrawl", apiKeys: [] },
      { id: "deepseek", apiKeys: [] },
    ];
  }
  const AUTO: SearchMode = { mode: "auto" };

  it("快照里没有搜索字段时视为一致，打开页面不进脏态", () => {
    const current = engines();
    const resolve = createSearchBaselineResolver();
    const base = resolve(JSON.stringify({ hotkey: "Alt+·" }), { engines: current, mode: AUTO });
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(false);
  });

  it("快照为空串、坏 JSON、null、数组时也不进脏态", () => {
    for (const raw of ["", "   ", "not json", "null", "[]"]) {
      const current = engines();
      const resolve = createSearchBaselineResolver();
      const base = resolve(raw, { engines: current, mode: AUTO });
      expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(false);
    }
  });

  it("缺字段时原地编辑要进脏态：基准不能和当前是同一份对象", () => {
    // 面板编辑是原地改（entry.apiKeys.push(...)），基准若是同一个引用就永远判不出改动。
    const current = engines();
    const resolve = createSearchBaselineResolver();
    const base = resolve("", { engines: current, mode: AUTO });
    expect(base.engines).not.toBe(current);
    expect(base.engines?.[0]).not.toBe(current[0]);

    current[0].apiKeys?.push("sk-1");
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(true);
  });

  it("基准里的密钥数组也要独立：改当前数组不会改到基准数组本身", () => {
    const current = engines();
    current[0].apiKeys = ["sk-a"];
    const resolve = createSearchBaselineResolver();
    const base = resolve("", { engines: current, mode: AUTO });
    expect(base.engines?.[0].apiKeys).not.toBe(current[0].apiKeys);

    current[0].apiKeys?.push("sk-b");
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(true);
  });

  it("缺字段时基准只抓一次，连续编辑不会把基准带跑", () => {
    const current = engines();
    const resolve = createSearchBaselineResolver();
    const base = resolve("", { engines: current, mode: AUTO });

    current[0].apiKeys = ["sk-1"];
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(true);
    expect(resolve("", { engines: current, mode: AUTO })).toBe(base);

    current[3].apiKeys = ["fc-1"];
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(true);
  });

  it("有真快照时用快照，改回快照值就干净", () => {
    const saved = engines();
    const savedJson = JSON.stringify({ searchEngines: saved, searchMode: AUTO });
    const resolve = createSearchBaselineResolver();
    const current = engines();
    const base = resolve(savedJson, { engines: current, mode: AUTO });
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(false);

    current[4].apiKeys = ["sk-deepseek"];
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(true);

    const restored = engines();
    expect(isSearchConfigDirty({ engines: restored, mode: AUTO }, base)).toBe(false);
  });

  it("快照里有搜索字段但没 searchMode 时，自动模式仍算一致", () => {
    const current = engines();
    const resolve = createSearchBaselineResolver();
    const base = resolve(JSON.stringify({ searchEngines: engines() }), {
      engines: current,
      mode: AUTO,
    });
    expect(isSearchConfigDirty({ engines: current, mode: AUTO }, base)).toBe(false);
  });
});
