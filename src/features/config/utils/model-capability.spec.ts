import { describe, expect, it } from "vitest";
import { capabilityHasContextWindow } from "./model-capability";

describe("capabilityHasContextWindow", () => {
  it("文本能力才有上下文窗口", () => {
    expect(capabilityHasContextWindow("text")).toBe(true);
  });

  it("嵌入/重排/语音/图像能力都没有上下文窗口", () => {
    // 这四种能力卡上不显示该参数，草稿不写它、夹它，比对也不看它
    expect(capabilityHasContextWindow("embedding")).toBe(false);
    expect(capabilityHasContextWindow("rerank")).toBe(false);
    expect(capabilityHasContextWindow("voice")).toBe(false);
    expect(capabilityHasContextWindow("imageGeneration")).toBe(false);
  });

  it("空值与未知能力按没有处理（不会凭空写出默认上下文窗口）", () => {
    expect(capabilityHasContextWindow("")).toBe(false);
    expect(capabilityHasContextWindow("unknown-capability")).toBe(false);
  });
});
