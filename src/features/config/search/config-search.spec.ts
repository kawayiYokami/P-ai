import { describe, expect, it } from "vitest";
import { searchConfigTabs } from "./config-search";

// 搜索引擎设置并入了供应商页的 header tab，不再有独立分页：命中要落在 api。
describe("设置页搜索索引", () => {
  it("中文搜「搜索」要能落到供应商页", () => {
    const tabs = searchConfigTabs("搜索", "zh-CN").map((item) => item.tab);
    expect(tabs).toContain("api");
  });

  it("英文搜 Search 要能落到供应商页", () => {
    const tabs = searchConfigTabs("Search", "en-US").map((item) => item.tab);
    expect(tabs).toContain("api");
  });

  it("按分页标题「搜索引擎」也能落到供应商页", () => {
    const tabs = searchConfigTabs("搜索引擎", "zh-CN").map((item) => item.tab);
    expect(tabs).toContain("api");
  });

  it("不再有独立的 searchEngines 分页", () => {
    const tabs = searchConfigTabs("搜索", "zh-CN").map((item) => item.tab);
    expect(tabs).not.toContain("searchEngines");
  });
});
