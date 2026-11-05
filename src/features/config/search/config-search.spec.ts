import { describe, expect, it } from "vitest";
import { searchConfigTabs } from "./config-search";

describe("设置页搜索索引", () => {
  it("中文搜「搜索」要能命中搜索引擎分页", () => {
    const tabs = searchConfigTabs("搜索", "zh-CN").map((item) => item.tab);
    expect(tabs).toContain("searchEngines");
  });

  it("英文搜 Search 要能命中搜索引擎分页", () => {
    const tabs = searchConfigTabs("Search", "en-US").map((item) => item.tab);
    expect(tabs).toContain("searchEngines");
  });

  it("按分页标题「搜索引擎」也能命中", () => {
    const tabs = searchConfigTabs("搜索引擎", "zh-CN").map((item) => item.tab);
    expect(tabs).toContain("searchEngines");
  });
});
