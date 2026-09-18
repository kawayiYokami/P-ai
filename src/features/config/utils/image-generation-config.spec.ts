import { describe, expect, it } from "vitest";
import {
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
});
