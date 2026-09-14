import { describe, expect, it } from "vitest";
import { apiConfigDisplayName, formatApiConfigOptionLabel, formatEndpointDisplay } from "./api-config-display";

describe("formatApiConfigOptionLabel", () => {
  it("应以 reasoningEffort 实时补全历史 name 缺失的思维等级，并使用中点分隔", () => {
    expect(formatApiConfigOptionLabel({
      name: "cpa/gpt-5.5",
      model: "gpt-5.5",
      reasoningEffort: "medium",
    })).toBe("cpa · gpt-5.5 · 中");
  });

  it("显示名存在时优先用显示名替换模型名本体", () => {
    expect(formatApiConfigOptionLabel({
      name: "cpa/gpt-5.5 · 中",
      model: "gpt-5.5",
      displayName: "鲸鱼妹",
      reasoningEffort: "medium",
    })).toBe("cpa · 鲸鱼妹 · 中");
  });

  it("应仅在聊天显示时把供应商和模型的斜杠改为中点", () => {
    const name = apiConfigDisplayName("惹", "gpt-5.6-terra", "high");
    expect(name).toBe("惹/gpt-5.6-terra · 高");
    expect(formatApiConfigOptionLabel({
      name,
      model: "gpt-5.6-terra",
      reasoningEffort: "high",
    })).toBe("惹 · gpt-5.6-terra · 高");
  });

  it("紧凑显示时供应商最多保留两个字符", () => {
    expect(formatApiConfigOptionLabel({
      name: "超长供应商/gpt-5.6-terra · 高",
      model: "gpt-5.6-terra",
      reasoningEffort: "high",
    }, undefined, { providerMaxCharacters: 2 })).toBe("超长 · gpt-5.6-terra · 高");
  });
});

describe("formatEndpointDisplay", () => {
  it("去掉默认的 https:// 前缀", () => {
    expect(formatEndpointDisplay("https://api.deepseek.com/v1")).toBe("api.deepseek.com/v1");
  });

  it("保留 http:// 以暴露非加密端点", () => {
    expect(formatEndpointDisplay("http://localhost:11434/v1")).toBe("http://localhost:11434/v1");
  });

  it("仅剥离开头的协议前缀，不误伤路径中的 https", () => {
    expect(formatEndpointDisplay("https://gateway.ai.cloudflare.com/v1/https://x")).toBe("gateway.ai.cloudflare.com/v1/https://x");
  });

  it("空值与空白返回空串", () => {
    expect(formatEndpointDisplay("")).toBe("");
    expect(formatEndpointDisplay(undefined)).toBe("");
    expect(formatEndpointDisplay("   ")).toBe("");
  });
});
