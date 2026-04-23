import { describe, expect, it } from "vitest";
import {
  buildGeneratedThemeStyleText,
  DARK_GENERATED_THEME_CONTROLS,
  DEFAULT_GENERATED_THEME_CONTROLS,
  GENERATED_THEME_NAME,
  generateGeneratedThemeTokens,
  getGeneratedThemeDefaultControls,
  generatedThemeTokensToCssVariables,
  LIGHT_GENERATED_THEME_CONTROLS,
  themeStateToThemeId,
} from "../src/features/shell/theme/theme-generator";

function readLightness(token: string): number {
  const match = token.match(/oklch\(([0-9.]+)%/i);
  return Number(match?.[1] || 0);
}

function readChroma(token: string): number {
  const match = token.match(/oklch\([0-9.]+%\s+([0-9.]+)\s+[0-9.]+\)/i);
  return Number(match?.[1] || 0);
}

function readHue(token: string): number {
  const match = token.match(/oklch\([0-9.]+%\s+[0-9.]+\s+([0-9.]+)\)/i);
  return Number(match?.[1] || 0);
}

describe("theme generator", () => {
  it("keeps light mode base layers descending from bright to dim", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
    });

    expect(readLightness(tokens.base100)).toBeGreaterThan(readLightness(tokens.base200));
    expect(readLightness(tokens.base200)).toBeGreaterThan(readLightness(tokens.base300));
    expect(tokens.radiusBox).toBe(tokens.radiusField);
    expect(tokens.radiusField).toBe(tokens.radiusSelector);
  });

  it("keeps dark mode base layers ascending from dim to bright", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      uiSizePreset: "comfortable",
      depthEnabled: true,
      noiseEnabled: true,
    });

    expect(readLightness(tokens.base100)).toBeLessThan(readLightness(tokens.base200));
    expect(readLightness(tokens.base200)).toBeLessThan(readLightness(tokens.base300));
    expect(tokens.sizeField).toBe("0.32rem");
    expect(tokens.sizeSelector).toBe("0.32rem");
    expect(tokens.border).toBe("1.5px");
    expect(tokens.depth).toBe("1");
    expect(tokens.noise).toBe("1");
  });

  it("builds css variables and generated theme identifiers", () => {
    const tokens = generateGeneratedThemeTokens(DEFAULT_GENERATED_THEME_CONTROLS);
    const cssVars = generatedThemeTokensToCssVariables(tokens);
    const styleText = buildGeneratedThemeStyleText(tokens);

    expect(cssVars["--color-primary"]).toBe(tokens.primary);
    expect(cssVars["--radius-box"]).toBe(tokens.radiusBox);
    expect(cssVars.colorScheme).toBe(tokens.colorScheme);
    expect(styleText).toContain(`[data-theme="${GENERATED_THEME_NAME}"]`);
    expect(styleText).toContain("--color-base-100");
    expect(themeStateToThemeId({
      kind: "generated",
      controls: {
        ...DEFAULT_GENERATED_THEME_CONTROLS,
        mode: "dark",
      },
    })).toBe("generated-dark");
  });

  it("returns mode-specific reset defaults", () => {
    expect(getGeneratedThemeDefaultControls("light")).toEqual(LIGHT_GENERATED_THEME_CONTROLS);
    expect(getGeneratedThemeDefaultControls("dark")).toEqual(DARK_GENERATED_THEME_CONTROLS);
  });

  it("uses the exact theme hue for primary and offsets secondary by -50 degrees with hue avoidance", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      themeHue: 60,
      contrast: 50,
      brightness: 80,
      tone: 60,
    });

    expect(readHue(tokens.primary)).toBeCloseTo(60, 0);
    expect(readHue(tokens.secondary)).toBeCloseTo(352, 0);
  });

  it("keeps primary chroma present while respecting the requested hue", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      themeHue: 60,
      contrast: 50,
      brightness: 80,
      tone: 60,
    });

    expect(readHue(tokens.primary)).toBeCloseTo(60, 0);
    expect(readLightness(tokens.primary)).toBeGreaterThanOrEqual(45);
    expect(readChroma(tokens.primary)).toBeGreaterThanOrEqual(0.06);
  });

  it("keeps dark-mode theme colors from getting overly bright", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      themeHue: 60,
      contrast: 56,
      brightness: 52,
      tone: 58,
    });

    expect(readLightness(tokens.primary)).toBeLessThanOrEqual(76);
    expect(readLightness(tokens.secondary)).toBeLessThanOrEqual(70);
  });

  it("keeps generated colors muted at minimum saturation", () => {
    const muted = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      tone: 30,
      themeHue: 60,
    });
    const vivid = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      tone: 100,
      themeHue: 60,
    });

    expect(readChroma(muted.base300)).toBeLessThan(readChroma(vivid.base300));
    expect(readChroma(muted.primary)).toBeLessThan(readChroma(vivid.primary));
    expect(readChroma(muted.secondary)).toBeLessThan(readChroma(vivid.secondary));
    expect(readChroma(muted.accent)).toBeLessThan(readChroma(vivid.accent));
  });

  it("reduces base tint when tint is lowered", () => {
    const neutralBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      tint: 0,
      themeHue: 120,
      tone: 100,
    });
    const tintedBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      tint: 100,
      themeHue: 120,
      tone: 100,
    });

    expect(readChroma(neutralBase.base300)).toBeLessThanOrEqual(0.01);
    expect(readChroma(tintedBase.base300)).toBeGreaterThan(readChroma(neutralBase.base300));
    expect(readHue(tintedBase.base300)).toBeCloseTo(120, 0);
  });

  it("widens base layer spacing when contrast increases", () => {
    const lowContrast = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      contrast: 10,
    });
    const highContrast = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      contrast: 90,
    });

    const lowSpread = readLightness(lowContrast.base100) - readLightness(lowContrast.base300);
    const highSpread = readLightness(highContrast.base100) - readLightness(highContrast.base300);

    expect(highSpread).toBeGreaterThan(lowSpread);
  });

  it("keeps at least a 10% lightness gap in light mode when contrast is enabled", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 80,
      contrast: 1,
    });

    expect(readLightness(tokens.base100) - readLightness(tokens.base300)).toBeGreaterThanOrEqual(10);
  });

  it("keeps at least a 10% lightness gap in dark mode when contrast is enabled", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 80,
      contrast: 1,
    });

    expect(readLightness(tokens.base300) - readLightness(tokens.base100)).toBeGreaterThanOrEqual(10);
  });

  it("pushes light-mode base300 toward 50% lightness at max contrast", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 20,
      contrast: 100,
    });

    expect(readLightness(tokens.base100)).toBeGreaterThan(readLightness(tokens.base200));
    expect(readLightness(tokens.base200)).toBeGreaterThan(readLightness(tokens.base300));
    expect(readLightness(tokens.base300)).toBeLessThanOrEqual(52);
    expect(readLightness(tokens.base300)).toBeGreaterThanOrEqual(48);
  });

  it("pushes dark-mode base300 toward 50% lightness at max contrast", () => {
    const tokens = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 20,
      contrast: 100,
    });

    expect(readLightness(tokens.base100)).toBeLessThan(readLightness(tokens.base200));
    expect(readLightness(tokens.base200)).toBeLessThan(readLightness(tokens.base300));
    expect(readLightness(tokens.base300)).toBeLessThanOrEqual(52);
    expect(readLightness(tokens.base300)).toBeGreaterThanOrEqual(48);
  });

  it("makes light-mode base100 lighter when brightness increases", () => {
    const lowBrightness = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 10,
    });
    const highBrightness = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 90,
    });

    expect(readLightness(highBrightness.base100)).toBeGreaterThan(readLightness(lowBrightness.base100));
  });

  it("makes dark-mode base100 darker when brightness increases", () => {
    const lowBrightness = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 10,
    });
    const highBrightness = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 90,
    });

    expect(readLightness(highBrightness.base100)).toBeLessThan(readLightness(lowBrightness.base100));
  });

  it("pushes light-mode base100 from gray toward white at the brightness extremes", () => {
    const grayBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 0,
    });
    const brightBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "light",
      brightness: 100,
    });

    expect(readLightness(grayBase.base100)).toBeLessThanOrEqual(78);
    expect(readLightness(grayBase.base100)).toBeGreaterThanOrEqual(70);
    expect(readLightness(brightBase.base100)).toBeGreaterThanOrEqual(98);
  });

  it("pushes dark-mode base100 from gray toward black at the brightness extremes", () => {
    const grayBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 0,
    });
    const darkBase = generateGeneratedThemeTokens({
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark",
      brightness: 100,
    });

    expect(readLightness(grayBase.base100)).toBeLessThanOrEqual(30);
    expect(readLightness(grayBase.base100)).toBeGreaterThanOrEqual(22);
    expect(readLightness(darkBase.base100)).toBeLessThanOrEqual(2);
  });
});
