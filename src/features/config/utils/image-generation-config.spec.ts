import { describe, expect, it } from "vitest";
import {
  appendImageGenerationProvider,
  createImageGenerationProvider,
  deriveImageGenerationModelOptions,
  normalizeImageGenerationModelId,
  normalizeImageGenerationProviders,
  resolveSelectedImageProviderId,
} from "./image-generation-config";

describe("image-generation-config", () => {
  it("应创建当前官方生图供应商模板", () => {
    const openai = createImageGenerationProvider("openai", "test");
    const xai = createImageGenerationProvider("xai", "test");
    const seedream = createImageGenerationProvider("seedream", "test");
    const gemini = createImageGenerationProvider("gemini", "test");

    expect(openai.baseUrl).toBe("https://api.openai.com/v1");
    expect(openai.models[0]?.model).toBe("gpt-image-2");
    expect(xai.models[0]?.model).toBe("grok-imagine-image-quality");
    expect(seedream.models[0]?.model).toBe("doubao-seedream-5-0-pro-260628");
    expect(gemini.models[0]?.model).toBe("gemini-3.1-flash-image");
  });

  it("应清理重复项并只投影启用模型", () => {
    const provider = createImageGenerationProvider("openai", "test");
    provider.apiKeys = [" key ", "key", "other"];
    provider.models.push({ ...provider.models[0] });
    const disabled = createImageGenerationProvider("xai", "disabled");
    disabled.enabled = false;

    const normalized = normalizeImageGenerationProviders([provider, provider, disabled]);
    const options = deriveImageGenerationModelOptions(normalized);

    expect(normalized).toHaveLength(2);
    expect(normalized[0]?.apiKeys).toEqual(["key", "other"]);
    expect(normalized[0]?.models).toHaveLength(1);
    expect(options).toHaveLength(1);
    expect(normalizeImageGenerationModelId(options[0]?.id, normalized)).toBe(options[0]?.id);
    expect(normalizeImageGenerationModelId("missing::model", normalized)).toBeUndefined();
  });

  it("选中项应沿用传入的目标而不是回退首项", () => {
    const first = createImageGenerationProvider("openai", "first");
    const second = createImageGenerationProvider("xai", "second");
    const providers = [first, second];

    expect(resolveSelectedImageProviderId(providers, second.id)).toBe(second.id);
    expect(resolveSelectedImageProviderId(providers, "missing")).toBe(first.id);
    expect(resolveSelectedImageProviderId([], "missing")).toBe("");
  });

  it("应支持向空或既有列表中添加新供应商并保持唯一 ID", () => {
    const list: ReturnType<typeof createImageGenerationProvider>[] = [];
    const p1 = createImageGenerationProvider("openai", "1");
    list.push(p1);
    const p2 = createImageGenerationProvider("openai", "2");
    list.push(p2);

    expect(list).toHaveLength(2);
    expect(p1.id).not.toBe(p2.id);
    expect(resolveSelectedImageProviderId(list, p2.id)).toBe(p2.id);
  });

  it("appendImageGenerationProvider 应安全操作配置并自动绑定首个默认端点", () => {
    const fakeConfig: { imageProviders?: ReturnType<typeof createImageGenerationProvider>[]; imageGenerationModelId?: string } = {};
    const created = appendImageGenerationProvider(fakeConfig as any, "openai", "abc");

    expect(fakeConfig.imageProviders).toHaveLength(1);
    expect(fakeConfig.imageProviders![0]?.id).toBe(created.id);
    expect(fakeConfig.imageGenerationModelId).toBe(`${created.id}::${created.models[0].id}`);

    // 第二次添加时已有默认端点，不应覆盖已有默认端点
    const second = appendImageGenerationProvider(fakeConfig as any, "openai", "def");
    expect(fakeConfig.imageProviders).toHaveLength(2);
    expect(fakeConfig.imageGenerationModelId).toBe(`${created.id}::${created.models[0].id}`);
  });
});
