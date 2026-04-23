import { converter } from "culori";
import type {
  AppThemeState,
  GeneratedThemeControls,
  GeneratedThemeTokens,
  GeneratedUiSizePreset,
  ThemeMode,
} from "./theme-types";

type OklchColor = {
  mode: "oklch";
  l: number;
  c: number;
  h: number;
};

type SizePresetTokens = {
  sizeField: string;
  sizeSelector: string;
  border: string;
};

const toRgb = converter("rgb");

const COLOR_ROLES: Array<[keyof GeneratedThemeTokens, string]> = [
  ["base100", "--color-base-100"],
  ["base200", "--color-base-200"],
  ["base300", "--color-base-300"],
  ["baseContent", "--color-base-content"],
  ["primary", "--color-primary"],
  ["primaryContent", "--color-primary-content"],
  ["secondary", "--color-secondary"],
  ["secondaryContent", "--color-secondary-content"],
  ["accent", "--color-accent"],
  ["accentContent", "--color-accent-content"],
  ["neutral", "--color-neutral"],
  ["neutralContent", "--color-neutral-content"],
  ["info", "--color-info"],
  ["infoContent", "--color-info-content"],
  ["success", "--color-success"],
  ["successContent", "--color-success-content"],
  ["warning", "--color-warning"],
  ["warningContent", "--color-warning-content"],
  ["error", "--color-error"],
  ["errorContent", "--color-error-content"],
];

const SHAPE_ROLES: Array<[keyof GeneratedThemeTokens, string]> = [
  ["radiusBox", "--radius-box"],
  ["radiusField", "--radius-field"],
  ["radiusSelector", "--radius-selector"],
  ["sizeField", "--size-field"],
  ["sizeSelector", "--size-selector"],
  ["border", "--border"],
  ["depth", "--depth"],
  ["noise", "--noise"],
];

const UI_SIZE_PRESET_MAP: Record<GeneratedUiSizePreset, SizePresetTokens> = {
  compact: {
    sizeField: "0.22rem",
    sizeSelector: "0.22rem",
    border: "1px",
  },
  default: {
    sizeField: "0.26rem",
    sizeSelector: "0.26rem",
    border: "1px",
  },
  comfortable: {
    sizeField: "0.32rem",
    sizeSelector: "0.32rem",
    border: "1.5px",
  },
};

export const GENERATED_THEME_NAME = "generated";
export const GENERATED_THEME_LIGHT_ID = "generated-light";
export const GENERATED_THEME_DARK_ID = "generated-dark";

export const LIGHT_GENERATED_THEME_CONTROLS: GeneratedThemeControls = {
  mode: "light",
  themeHue: 250,
  contrast: 10,
  brightness: 100,
  tint: 0,
  tone: 90,
  radius: 16,
  uiSizePreset: "default",
  depthEnabled: false,
  noiseEnabled: false,
};

export const DARK_GENERATED_THEME_CONTROLS: GeneratedThemeControls = {
  mode: "dark",
  themeHue: 300,
  contrast: 10,
  brightness: 25,
  tint: 100,
  tone: 90,
  radius: 16,
  uiSizePreset: "default",
  depthEnabled: false,
  noiseEnabled: false,
};

export const DEFAULT_GENERATED_THEME_CONTROLS: GeneratedThemeControls = {
  ...DARK_GENERATED_THEME_CONTROLS,
};

export function getGeneratedThemeDefaultControls(mode: ThemeMode): GeneratedThemeControls {
  return {
    ...(mode === "light" ? LIGHT_GENERATED_THEME_CONTROLS : DARK_GENERATED_THEME_CONTROLS),
  };
}

const LEGACY_GENERATED_THEME_CONTROLS: GeneratedThemeControls = {
  mode: "dark",
  themeHue: 230,
  contrast: 56,
  brightness: 52,
  tint: 100,
  tone: 58,
  radius: 16,
  uiSizePreset: "default",
  depthEnabled: false,
  noiseEnabled: false,
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function round(value: number, digits = 0): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function wrapHue(value: number): number {
  const normalized = Number.isFinite(value) ? value % 360 : 0;
  return normalized < 0 ? normalized + 360 : normalized;
}

function normalizePercent(value: number): number {
  return clamp(Number.isFinite(value) ? value : 0, 0, 100);
}

function isDisplayableRgb(color: ReturnType<typeof toRgb>): boolean {
  if (!color) return false;
  return [color.r, color.g, color.b].every((channel) => Number.isFinite(channel) && channel >= 0 && channel <= 1);
}

function toDisplayableOklch(color: OklchColor): OklchColor {
  let next: OklchColor = {
    mode: "oklch",
    l: clamp(color.l, 0, 1),
    c: Math.max(0, color.c),
    h: wrapHue(color.h),
  };
  for (let attempt = 0; attempt < 28; attempt += 1) {
    if (isDisplayableRgb(toRgb(next))) {
      return next;
    }
    next = {
      ...next,
      c: next.c * 0.9,
    };
  }
  return {
    ...next,
    c: 0,
  };
}

function createOklch(l: number, c: number, h: number): OklchColor {
  return toDisplayableOklch({
    mode: "oklch",
    l,
    c,
    h,
  });
}

function oklchToCss(color: OklchColor): string {
  return `oklch(${round(color.l * 100, 1)}% ${round(color.c, 3)} ${round(color.h, 1)})`;
}

function srgbToLinear(channel: number): number {
  if (channel <= 0.04045) {
    return channel / 12.92;
  }
  return ((channel + 0.055) / 1.055) ** 2.4;
}

function relativeLuminance(color: OklchColor): number {
  const rgb = toRgb(color);
  if (!rgb) return 0;
  const r = srgbToLinear(clamp(rgb.r, 0, 1));
  const g = srgbToLinear(clamp(rgb.g, 0, 1));
  const b = srgbToLinear(clamp(rgb.b, 0, 1));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrastRatio(foreground: OklchColor, background: OklchColor): number {
  const fg = relativeLuminance(foreground);
  const bg = relativeLuminance(background);
  const lighter = Math.max(fg, bg);
  const darker = Math.min(fg, bg);
  return (lighter + 0.05) / (darker + 0.05);
}

function adjustForContrast(
  color: OklchColor,
  background: OklchColor,
  minimumRatio: number,
  direction: "lighter" | "darker",
): OklchColor {
  let next = color;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (contrastRatio(next, background) >= minimumRatio) {
      return next;
    }
    next = createOklch(
      next.l + (direction === "lighter" ? 0.03 : -0.03),
      next.c * 0.98,
      next.h,
    );
  }
  return next;
}

function bestReadableContent(background: OklchColor): OklchColor {
  const darkText = createOklch(0.18, 0.01, 260);
  const lightText = createOklch(0.98, 0.005, 260);
  return contrastRatio(darkText, background) >= contrastRatio(lightText, background) ? darkText : lightText;
}

function sizePresetTokens(preset: GeneratedUiSizePreset): SizePresetTokens {
  return UI_SIZE_PRESET_MAP[preset] || UI_SIZE_PRESET_MAP.default;
}

function radiusToToken(radius: number): string {
  const normalizedRadius = clamp(Number.isFinite(radius) ? radius : DARK_GENERATED_THEME_CONTROLS.radius, 0, 28);
  return `${round(normalizedRadius / 16, 3)}rem`;
}

function moveLightnessTowardTarget(
  baseLightness: number,
  targetLightness: number,
  contrastFactor: number,
  minimumDelta = 0.1,
): number {
  const delta = targetLightness - baseLightness;
  const distance = Math.abs(delta);
  if (distance === 0 || contrastFactor <= 0) {
    return baseLightness;
  }
  const minimumFactor = clamp(minimumDelta / distance, 0, 1);
  const effectiveFactor = Math.max(contrastFactor, minimumFactor);
  return baseLightness + delta * effectiveFactor;
}

function nearestHueDistance(left: number, right: number): number {
  const delta = Math.abs(wrapHue(left) - wrapHue(right));
  return Math.min(delta, 360 - delta);
}

function resolveSecondaryHue(primaryHue: number, blockedHues: number[], minimumDistance = 24): number {
  let candidateHue = wrapHue(primaryHue - 50);
  const normalizedBlockedHues = blockedHues.map((hue) => wrapHue(hue));
  for (let attempt = 0; attempt < 12; attempt += 1) {
    const collides = normalizedBlockedHues.some((blockedHue) => nearestHueDistance(candidateHue, blockedHue) < minimumDistance);
    if (!collides) {
      return candidateHue;
    }
    candidateHue = wrapHue(candidateHue - 18);
  }
  return candidateHue;
}

export function normalizeGeneratedThemeControls(
  input?: Partial<GeneratedThemeControls> | null,
): GeneratedThemeControls {
  const fallbackControls = input?.mode === "light" ? LIGHT_GENERATED_THEME_CONTROLS : DARK_GENERATED_THEME_CONTROLS;
  const legacyControls = LEGACY_GENERATED_THEME_CONTROLS;
  const normalizedThemeHue = round(
    wrapHue(Number(input?.themeHue ?? fallbackControls.themeHue ?? legacyControls.themeHue)),
    1,
  );
  return {
    mode: input?.mode === "light" ? "light" : "dark",
    themeHue: normalizedThemeHue === 0 ? 360 : clamp(normalizedThemeHue, 1, 360),
    contrast: clamp(
      Math.round(normalizePercent(Number(input?.contrast ?? fallbackControls.contrast ?? legacyControls.contrast))),
      10,
      100,
    ),
    brightness: Math.round(
      normalizePercent(Number(input?.brightness ?? fallbackControls.brightness ?? legacyControls.brightness)),
    ),
    tint: Math.round(normalizePercent(Number(input?.tint ?? fallbackControls.tint ?? legacyControls.tint))),
    tone: clamp(
      Math.round(normalizePercent(Number(input?.tone ?? fallbackControls.tone ?? legacyControls.tone))),
      30,
      100,
    ),
    radius: clamp(
      Math.round(Number(input?.radius ?? fallbackControls.radius ?? legacyControls.radius)),
      0,
      28,
    ),
    uiSizePreset:
      input?.uiSizePreset === "compact" || input?.uiSizePreset === "comfortable"
        ? input.uiSizePreset
        : "default",
    depthEnabled: !!input?.depthEnabled,
    noiseEnabled: !!input?.noiseEnabled,
  };
}

function buildBaseColors(
  mode: ThemeMode,
  themeHue: number,
  toneFactor: number,
  contrastFactor: number,
  brightnessFactor: number,
  tintFactor: number,
): [OklchColor, OklchColor, OklchColor] {
  const baseHue = wrapHue(255 + (wrapHue(themeHue) - 255) * tintFactor);
  const baseChroma = toneFactor * 0.016 * tintFactor;
  const contrastTargetLightness = 0.5;
  if (mode === "light") {
    const base100Lightness = clamp(0.75 + brightnessFactor * 0.25, 0.75, 1);
    const base300Lightness = moveLightnessTowardTarget(base100Lightness, contrastTargetLightness, contrastFactor);
    const base200Lightness = (base100Lightness + base300Lightness) / 2;
    return [
      createOklch(base100Lightness, baseChroma * 0.28, baseHue),
      createOklch(base200Lightness, baseChroma * 0.48, baseHue),
      createOklch(base300Lightness, baseChroma * 0.9, baseHue),
    ];
  }
  const base100Lightness = clamp(0.25 - brightnessFactor * 0.25, 0, 0.25);
  const base300Lightness = moveLightnessTowardTarget(base100Lightness, contrastTargetLightness, contrastFactor);
  const base200Lightness = (base100Lightness + base300Lightness) / 2;
  return [
    createOklch(base100Lightness, baseChroma * 0.35, baseHue),
    createOklch(base200Lightness, baseChroma * 0.58, baseHue),
    createOklch(base300Lightness, baseChroma * 0.92, baseHue),
  ];
}

function buildRoleColor(
  mode: ThemeMode,
  lightness: number,
  chroma: number,
  hue: number,
  background: OklchColor,
  minimumRatio: number,
): OklchColor {
  const direction = mode === "light" ? "darker" : "lighter";
  const seeded = createOklch(lightness, chroma, hue);
  return adjustForContrast(seeded, background, minimumRatio, direction);
}

export function generateGeneratedThemeTokens(controlsInput: GeneratedThemeControls): GeneratedThemeTokens {
  const controls = normalizeGeneratedThemeControls(controlsInput);
  const toneFactor = controls.tone / 100;
  const contrastFactor = controls.contrast / 100;
  const brightnessFactor = controls.brightness / 100;
  const tintFactor = controls.tint / 100;

  const [base100, base200, base300] = buildBaseColors(
    controls.mode,
    controls.themeHue,
    toneFactor,
    contrastFactor,
    brightnessFactor,
    tintFactor,
  );
  const baseContent = bestReadableContent(base100);
  const secondaryHue = resolveSecondaryHue(controls.themeHue, [
    32,
    255,
    245,
    148,
    82,
    28,
  ]);

  const primary = buildRoleColor(
    controls.mode,
    controls.mode === "light"
      ? 0.5 - contrastFactor * 0.04 + toneFactor * 0.05 + brightnessFactor * 0.015
      : 0.54 + toneFactor * 0.07 - contrastFactor * 0.02,
    toneFactor * (0.14 + contrastFactor * 0.035),
    controls.themeHue,
    base100,
    3.2,
  );

  const secondary = buildRoleColor(
    controls.mode,
    controls.mode === "light"
      ? 0.56 - contrastFactor * 0.02 + toneFactor * 0.04 + brightnessFactor * 0.01
      : 0.6 + toneFactor * 0.05 - contrastFactor * 0.015,
    toneFactor * 0.15,
    secondaryHue,
    base100,
    3,
  );

  const accent = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.62 : 0.72,
    toneFactor * 0.16,
    32,
    base100,
    3,
  );

  const neutral = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.38 - contrastFactor * 0.04 : 0.32 + contrastFactor * 0.06,
    toneFactor * 0.038,
    255,
    base100,
    2.8,
  );

  const info = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.56 : 0.7,
    toneFactor * 0.16,
    245,
    base100,
    3,
  );

  const success = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.58 : 0.74,
    toneFactor * 0.17,
    148,
    base100,
    3,
  );

  const warning = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.66 : 0.76,
    toneFactor * 0.16,
    82,
    base100,
    3,
  );

  const error = buildRoleColor(
    controls.mode,
    controls.mode === "light" ? 0.58 : 0.72,
    toneFactor * 0.18,
    28,
    base100,
    3,
  );

  const { sizeField, sizeSelector, border } = sizePresetTokens(controls.uiSizePreset);
  const radius = radiusToToken(controls.radius);

  return {
    base100: oklchToCss(base100),
    base200: oklchToCss(base200),
    base300: oklchToCss(base300),
    baseContent: oklchToCss(baseContent),
    primary: oklchToCss(primary),
    primaryContent: oklchToCss(bestReadableContent(primary)),
    secondary: oklchToCss(secondary),
    secondaryContent: oklchToCss(bestReadableContent(secondary)),
    accent: oklchToCss(accent),
    accentContent: oklchToCss(bestReadableContent(accent)),
    neutral: oklchToCss(neutral),
    neutralContent: oklchToCss(bestReadableContent(neutral)),
    info: oklchToCss(info),
    infoContent: oklchToCss(bestReadableContent(info)),
    success: oklchToCss(success),
    successContent: oklchToCss(bestReadableContent(success)),
    warning: oklchToCss(warning),
    warningContent: oklchToCss(bestReadableContent(warning)),
    error: oklchToCss(error),
    errorContent: oklchToCss(bestReadableContent(error)),
    radiusBox: radius,
    radiusField: radius,
    radiusSelector: radius,
    sizeField,
    sizeSelector,
    border,
    depth: controls.depthEnabled ? "1" : "0",
    noise: controls.noiseEnabled ? "1" : "0",
    colorScheme: controls.mode,
  };
}

export function generatedThemeTokensToCssVariables(tokens: GeneratedThemeTokens): Record<string, string> {
  const variables: Record<string, string> = {
    colorScheme: tokens.colorScheme,
  };
  for (const [tokenKey, cssVar] of COLOR_ROLES) {
    variables[cssVar] = String(tokens[tokenKey] || "");
  }
  for (const [tokenKey, cssVar] of SHAPE_ROLES) {
    variables[cssVar] = String(tokens[tokenKey] || "");
  }
  return variables;
}

export function buildGeneratedThemeStyleText(tokens: GeneratedThemeTokens): string {
  const variables = generatedThemeTokensToCssVariables(tokens);
  const lines = Object.entries(variables)
    .map(([key, value]) => {
      if (key === "colorScheme") {
        return `  color-scheme: ${value};`;
      }
      return `  ${key}: ${value};`;
    })
    .join("\n");
  return `[data-theme="${GENERATED_THEME_NAME}"] {\n${lines}\n}`;
}

export function getGeneratedThemeId(mode: ThemeMode): string {
  return mode === "light" ? GENERATED_THEME_LIGHT_ID : GENERATED_THEME_DARK_ID;
}

export function themeStateToThemeId(state: AppThemeState): string {
  return state.kind === "generated" ? getGeneratedThemeId(state.controls.mode) : state.name;
}
