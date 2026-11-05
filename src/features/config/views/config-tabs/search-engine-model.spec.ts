import { describe, expect, it } from "vitest";
import {
  SEARCH_ENGINE_META,
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
