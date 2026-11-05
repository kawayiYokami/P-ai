export type ThemeMode = "light" | "dark";

export type GeneratedThemeControls = {
  mode: ThemeMode;
  themeHue: number;
  contrast: number;
  brightness: number;
  tint: number;
  tone: number;
  textStrength: number;
  radius: number;
  depthEnabled: boolean;
  noiseEnabled: boolean;
};

export type GeneratedThemeControlsByMode = Record<ThemeMode, GeneratedThemeControls>;

export type GeneratedThemeTokens = {
  base100: string;
  base200: string;
  base300: string;
  baseContent: string;
  primary: string;
  primaryContent: string;
  secondary: string;
  secondaryContent: string;
  accent: string;
  accentContent: string;
  neutral: string;
  neutralContent: string;
  info: string;
  infoContent: string;
  success: string;
  successContent: string;
  warning: string;
  warningContent: string;
  error: string;
  errorContent: string;
  radiusBox: string;
  radiusField: string;
  radiusSelector: string;
  depth: string;
  noise: string;
  colorScheme: ThemeMode;
};

export type AppThemeState =
  | { kind: "preset"; name: string }
  | { kind: "generated"; controls: GeneratedThemeControls };

/** 显示模式：明 / 暗 / 自动（跟随系统）。 */
export type ThemeModeKind = "light" | "dark" | "auto";

/** 外观来源：预设（内置主题）/ 自定义（参数生成的）。 */
export type ThemeSourceKind = "preset" | "generated";

/**
 * 外观偏好：明暗两侧各自记一张卡。
 *
 * presetLightTheme / presetDarkTheme 就是两侧生效的卡：内置预设名，或
 * generated-light / generated-dark（自定义浅色 / 自定义深色）。
 *
 * source 是派生字段（当前生效那张卡是不是自定义），不再参与决定用哪套主题。
 */
export type PersistedThemePreferences = {
  version: 4;
  mode: ThemeModeKind;
  source: ThemeSourceKind;
  presetLightTheme: string;
  presetDarkTheme: string;
  generatedControlsByMode: GeneratedThemeControlsByMode;
};
