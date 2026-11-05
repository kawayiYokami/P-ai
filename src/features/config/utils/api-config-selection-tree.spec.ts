import { describe, expect, it } from "vitest";
import type { ApiConfigItem } from "../../../types/app";
import { buildApiConfigSelectionTree } from "./api-config-selection-tree";

function apiConfig(overrides: Partial<ApiConfigItem>): ApiConfigItem {
  return {
    id: "provider-a::model-default",
    name: "Provider A/gpt-5.6-terra · 默认",
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableVideo: false,
    enableTools: true,
    baseUrl: "https://example.com/v1",
    apiKey: "",
    model: "gpt-5.6-terra",
    reasoningEffort: "default",
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 128_000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4_096,
    ...overrides,
  };
}

describe("buildApiConfigSelectionTree", () => {
  it("按渠道、同公共配置模型和思维等级分组，并保留原 apiConfigId", () => {
    const tree = buildApiConfigSelectionTree([
      apiConfig({ id: "provider-a::model-default", reasoningEffort: "default" }),
      apiConfig({ id: "provider-a::model-medium", reasoningEffort: "medium" }),
      apiConfig({
        id: "provider-a::model-max",
        reasoningEffort: "max",
        contextWindowTokens: 256_000,
      }),
    ]);

    expect(tree).toHaveLength(1);
    expect(tree[0].name).toBe("Provider A");
    expect(tree[0].models).toHaveLength(2);
    expect(tree[0].models[0].leaves.map((leaf) => leaf.id)).toEqual([
      "provider-a::model-default",
      "provider-a::model-medium",
    ]);
    expect(tree[0].models[0].summaryFields).toContain("contextWindowTokens");
    expect(tree[0].models[1].leaves[0].id).toBe("provider-a::model-max");
  });

  it("模型组显示名优先取 displayName，缺失时回退模型名", () => {
    const tree = buildApiConfigSelectionTree([
      apiConfig({
        id: "provider-a::model-named",
        displayName: "鲸鱼妹",
        model: "deepseek-v4-flash",
        name: "Provider A/deepseek-v4-flash · 中",
        reasoningEffort: "medium",
      }),
      apiConfig({
        id: "provider-a::model-plain",
        model: "gpt-5.6-terra",
        name: "Provider A/gpt-5.6-terra · 高",
        reasoningEffort: "high",
      }),
    ]);

    expect(tree[0].models).toHaveLength(2);
    expect(tree[0].models[0].name).toBe("鲸鱼妹");
    expect(tree[0].models[1].name).toBe("gpt-5.6-terra");
  });

  it("未知思维等级仍作为可选叶子保留", () => {
    const tree = buildApiConfigSelectionTree([
      apiConfig({ id: "provider-a::model-legacy", reasoningEffort: "legacy" }),
    ]);

    expect(tree[0].models[0].leaves).toEqual([
      expect.objectContaining({ id: "provider-a::model-legacy", label: "legacy" }),
    ]);
  });

  it("思维等级叶子按固定档位序排列，不因录入顺序靠前", () => {
    const tree = buildApiConfigSelectionTree([
      apiConfig({ id: "provider-a::model-high", reasoningEffort: "high" }),
      apiConfig({ id: "provider-a::model-default", reasoningEffort: "default" }),
      apiConfig({ id: "provider-a::model-low", reasoningEffort: "low" }),
      apiConfig({ id: "provider-a::model-max", reasoningEffort: "max" }),
    ]);

    expect(tree[0].models[0].leaves.map((leaf) => leaf.item.reasoningEffort)).toEqual([
      "default",
      "low",
      "high",
      "max",
    ]);
  });
});

