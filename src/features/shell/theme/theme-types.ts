export type ThemeMode = "light" | "dark";

export type GeneratedUiSizePreset = "compact" | "default" | "comfortable";

export type GeneratedThemeControls = {
  mode: ThemeMode;
  themeHue: number;
  contrast: number;
  brightness: number;
  tint: number;
  tone: number;
  radius: number;
  uiSizePreset: GeneratedUiSizePreset;
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
  sizeField: string;
  sizeSelector: string;
  border: string;
  depth: string;
  noise: string;
  colorScheme: ThemeMode;
};

export type AppThemeState =
  | { kind: "preset"; name: string }
  | { kind: "generated"; controls: GeneratedThemeControls };

export type PersistedThemePreferences = {
  version: 2;
  activeState: AppThemeState;
  generatedControls: GeneratedThemeControls;
  generatedControlsByMode: GeneratedThemeControlsByMode;
};
