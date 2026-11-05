import { describe, expect, it } from "vitest";
import type { ApiModelConfigItem, ApiProviderConfigItem } from "../../../types/app";
import { buildDraftGroups, modelGroupKey, normalizedModelReasoningEffortFor, splitDraftGroups } from "./draft-model-groups";

function createModel(overrides: Partial<ApiModelConfigItem> & { id: string; model: string }): ApiModelConfigItem {
  return {
    displayName: "",
    deprecated: false,
    enableImage: false,
    enableAudio: false,
    enableVideo: false,
    enableTools: true,
    reasoningEffort: "default",
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 128_000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
    ...overrides,
  };
}

function createProvider(models: ApiModelConfigItem[]): ApiProviderConfigItem {
  return {
    id: "provider-1",
    name: "测试供应商",
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: true,
    baseUrl: "https://api.example.com",
    apiKeys: [],
    cachedModelOptions: [],
    models,
  };
}

describe("draft-model-groups 聚合", () => {
  it("同模型同参数不同思考等级聚合为一组，等级与 id 分别记录", () => {
    const provider = createProvider([
      createModel({ id: "m-default", model: "gpt-4o", reasoningEffort: "default" }),
      createModel({ id: "m-low", model: "gpt-4o", reasoningEffort: "low" }),
      createModel({ id: "m-high", model: "gpt-4o", reasoningEffort: "high" }),
    ]);
    const groups = buildDraftGroups(provider);
    expect(groups).toHaveLength(1);
    expect(groups[0].reasoningEfforts).toEqual(["default", "low", "high"]);
    expect(groups[0].variantIdByEffort.get("default")).toBe("m-default");
    expect(groups[0].variantIdByEffort.get("low")).toBe("m-low");
    expect(groups[0].variantIdByEffort.get("high")).toBe("m-high");
  });

  it("同名不同参数拆成多个组，互不合并", () => {
    const provider = createProvider([
      createModel({ id: "m-128", model: "gpt-4o", contextWindowTokens: 128_000 }),
      createModel({ id: "m-256", model: "gpt-4o", contextWindowTokens: 256_000 }),
    ]);
    const groups = buildDraftGroups(provider);
    expect(groups).toHaveLength(2);
    expect(modelGroupKey(groups[0].primary)).not.toBe(modelGroupKey(groups[1].primary));
  });

  it("deprecated 的卡不参与聚合", () => {
    const provider = createProvider([
      createModel({ id: "m-live", model: "gpt-4o", reasoningEffort: "default" }),
      createModel({ id: "m-gone", model: "gpt-4o", reasoningEffort: "high", deprecated: true }),
    ]);
    const groups = buildDraftGroups(provider);
    expect(groups).toHaveLength(1);
    expect(groups[0].reasoningEfforts).toEqual(["default"]);
  });

  it("聚合键覆盖全部可编辑参数，任一字段不同即分到不同组", () => {
    const fields: Array<Partial<ApiModelConfigItem>> = [
      { enableImage: true },
      { enableAudio: true },
      { enableVideo: true },
      { enableTools: false },
      { temperature: 0.5 },
      { customTemperatureEnabled: true },
      { contextWindowTokens: 64_000 },
      { customMaxOutputTokensEnabled: true },
      { maxOutputTokens: 8192 },
    ];
    for (const field of fields) {
      const base = createModel({ id: "m-base", model: "gpt-4o" });
      const variant = createModel({ id: "m-variant", model: "gpt-4o", ...field });
      expect(modelGroupKey(variant)).not.toBe(modelGroupKey(base));
    }
  });
});

describe("draft-model-groups 拆分", () => {
  it("每组每个勾选等级拆出一张卡，id 复用 variantIdByEffort", () => {
    const provider = createProvider([createModel({ id: "m-default", model: "gpt-4o" })]);
    const groups = buildDraftGroups(provider);
    groups[0].reasoningEfforts = ["default", "low", "high"];
    const nextModels = splitDraftGroups(provider, groups, () => "new-id");
    expect(nextModels).toHaveLength(3);
    expect(nextModels[0].id).toBe("m-default");
    expect(nextModels[1].id).toBe("new-id");
    expect(nextModels[2].id).toBe("new-id");
    expect(nextModels.map((m) => m.reasoningEffort)).toEqual(["default", "low", "high"]);
    // 新 id 回填到草稿，保证后续重复拆分的稳定性
    expect(groups[0].variantIdByEffort.get("low")).toBe("new-id");
  });

  it("reasoningEfforts 为空时回退 default", () => {
    const provider = createProvider([createModel({ id: "m-default", model: "gpt-4o" })]);
    const groups = buildDraftGroups(provider);
    groups[0].reasoningEfforts = [];
    const nextModels = splitDraftGroups(provider, groups, () => "new-id");
    expect(nextModels).toHaveLength(1);
    expect(nextModels[0].reasoningEffort).toBe("default");
  });

  it("保留原 provider 中 deprecated 的卡", () => {
    const provider = createProvider([
      createModel({ id: "m-live", model: "gpt-4o" }),
      createModel({ id: "m-gone", model: "gpt-4o", reasoningEffort: "high", deprecated: true }),
    ]);
    const groups = buildDraftGroups(provider);
    const nextModels = splitDraftGroups(provider, groups, () => "new-id");
    expect(nextModels).toHaveLength(2);
    expect(nextModels.find((m) => m.id === "m-gone")?.deprecated).toBe(true);
  });

  it("拆分后的卡参数与 primary 一致", () => {
    const provider = createProvider([createModel({ id: "m-default", model: "gpt-4o", contextWindowTokens: 128_000 })]);
    const groups = buildDraftGroups(provider);
    groups[0].primary.contextWindowTokens = 256_000;
    groups[0].reasoningEfforts = ["default", "low"];
    const nextModels = splitDraftGroups(provider, groups, () => "new-id");
    for (const model of nextModels) {
      expect(model.contextWindowTokens).toBe(256_000);
      expect(model.model).toBe("gpt-4o");
    }
  });
});

describe("draft-model-groups 可逆性", () => {
  it("聚合 → 编辑 → 拆分 → 再聚合，等级与参数不丢", () => {
    const provider = createProvider([
      createModel({ id: "m-default", model: "gpt-4o", reasoningEffort: "default" }),
      createModel({ id: "m-low", model: "gpt-4o", reasoningEffort: "low" }),
      createModel({ id: "m-high", model: "gpt-4o", reasoningEffort: "high" }),
    ]);
    const groups = buildDraftGroups(provider);
    // 草稿态编辑：只改 primary，不触发重新聚合
    groups[0].primary.contextWindowTokens = 256_000;
    groups[0].reasoningEfforts = ["default", "low"];
    const savedModels = splitDraftGroups(provider, groups, () => "new-id");
    const reAggregated = buildDraftGroups(createProvider(savedModels));
    expect(reAggregated).toHaveLength(1);
    expect(reAggregated[0].reasoningEfforts).toEqual(["default", "low"]);
    expect(reAggregated[0].primary.contextWindowTokens).toBe(256_000);
    expect(reAggregated[0].variantIdByEffort.get("default")).toBe("m-default");
    expect(reAggregated[0].variantIdByEffort.get("low")).toBe("m-low");
  });
});

describe("draft-model-groups 等级规范化", () => {
  it("reasoningEffort 统一小写，空值归 default", () => {
    expect(normalizedModelReasoningEffortFor(createModel({ id: "m", model: "gpt-4o", reasoningEffort: "HIGH" }))).toBe("high");
    expect(normalizedModelReasoningEffortFor(createModel({ id: "m", model: "gpt-4o", reasoningEffort: "" }))).toBe("default");
    expect(normalizedModelReasoningEffortFor(createModel({ id: "m", model: "gpt-4o", reasoningEffort: undefined }))).toBe("default");
  });
});
