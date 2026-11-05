import { describe, expect, it } from "vitest";
import { normalizeUiSizeScale, uiSizeTokensFor } from "./use-ui-size-appearance";

const INTEGER_PX = /^\d+px$/;

describe("界面字号四档", () => {
  it("每档每个 token 都是整数 px", () => {
    for (const scale of [75, 100, 125, 150]) {
      const tokens = uiSizeTokensFor(scale);
      for (const [name, value] of Object.entries(tokens)) {
        expect(value, `${scale} 档的 ${name} 不是整数 px：${value}`).toMatch(INTEGER_PX);
      }
    }
  });

  it("旧百分比就近归入四档，不产生中间档", () => {
    expect(normalizeUiSizeScale(130)).toBe(125);
    expect(normalizeUiSizeScale(112)).toBe(100);
    expect(normalizeUiSizeScale(140)).toBe(150);
    expect(normalizeUiSizeScale(80)).toBe(75);
    // 归入同一档后拿到的必须是同一张表，不是按原值再算一遍
    expect(uiSizeTokensFor(130)).toEqual(uiSizeTokensFor(125));
  });

  it("旧预设字符串映射到四档", () => {
    expect(normalizeUiSizeScale("small")).toBe(75);
    expect(normalizeUiSizeScale("default")).toBe(100);
    expect(normalizeUiSizeScale("large")).toBe(125);
    expect(normalizeUiSizeScale("extraLarge")).toBe(150);
  });

  it("新档位名可直接当值用", () => {
    expect(normalizeUiSizeScale("compact")).toBe(75);
    expect(normalizeUiSizeScale("xlarge")).toBe(150);
  });

  it("空值与未知值回落到默认档", () => {
    expect(normalizeUiSizeScale(null)).toBe(100);
    expect(normalizeUiSizeScale("")).toBe(100);
    expect(normalizeUiSizeScale("不认识")).toBe(100);
  });

  it("四档正文字号逐档递增且不重复", () => {
    const sizes = [75, 100, 125, 150].map((scale) =>
      Number.parseInt(uiSizeTokensFor(scale).textSm, 10),
    );
    expect(sizes).toEqual([13, 14, 16, 18]);
  });
});
