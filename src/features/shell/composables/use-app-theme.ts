import { computed, ref } from "vue";
import { emit } from "@tauri-apps/api/event";
import {
  buildGeneratedThemeStyleText,
  DEFAULT_GENERATED_THEME_CONTROLS,
  GENERATED_THEME_DARK_ID,
  GENERATED_THEME_LIGHT_ID,
  GENERATED_THEME_NAME,
  generateGeneratedThemeTokens,
  getGeneratedThemeDefaultControls,
  normalizeGeneratedThemeControls,
  themeStateToThemeId,
} from "../theme/theme-generator";
import type {
  AppThemeState,
  GeneratedThemeControls,
  GeneratedThemeControlsByMode,
  PersistedThemePreferences,
  ThemeMode,
} from "../theme/theme-types";

const THEME_STORAGE_KEY = "easy-call.theme-state.v1";
const LEGACY_THEME_STORAGE_KEY = "theme";
const GENERATED_THEME_STYLE_ID = "easy-call-generated-theme-style";
const LEGACY_DARK_THEME_NAMES = new Set<string>([
  "dark",
  "halloween",
  "forest",
  "luxury",
  "dracula",
  "business",
  "night",
  "coffee",
  "dim",
]);
export const APP_THEMES = [
  "light",
  "dark",
  "cupcake",
  "emerald",
  "corporate",
  "halloween",
  "garden",
  "forest",
  "lofi",
  "pastel",
  "luxury",
  "dracula",
  "autumn",
  "business",
  "night",
  "coffee",
  "winter",
  "dim",
] as const;
export const DARK_APP_THEMES = new Set<string>(LEGACY_DARK_THEME_NAMES);
type AppTheme = (typeof APP_THEMES)[number];
const THEME_SET = new Set<string>(APP_THEMES);

function cloneGeneratedThemeControls(
  controls: Partial<GeneratedThemeControls> | GeneratedThemeControls,
): GeneratedThemeControls {
  return normalizeGeneratedThemeControls({ ...controls });
}

function createGeneratedThemeControlsByMode(
  input?: Partial<GeneratedThemeControlsByMode> | null,
  legacyControls?: Partial<GeneratedThemeControls> | GeneratedThemeControls | null,
): GeneratedThemeControlsByMode {
  const defaults: GeneratedThemeControlsByMode = {
    light: cloneGeneratedThemeControls({ ...getGeneratedThemeDefaultControls("light"), mode: "light" }),
    dark: cloneGeneratedThemeControls({ ...getGeneratedThemeDefaultControls("dark"), mode: "dark" }),
  };
  const normalizedLegacy = legacyControls ? cloneGeneratedThemeControls(legacyControls) : null;
  const next: GeneratedThemeControlsByMode = {
    light: input?.light
      ? cloneGeneratedThemeControls({ ...input.light, mode: "light" })
      : defaults.light,
    dark: input?.dark
      ? cloneGeneratedThemeControls({ ...input.dark, mode: "dark" })
      : defaults.dark,
  };
  if (normalizedLegacy) {
    next[normalizedLegacy.mode] = cloneGeneratedThemeControls(normalizedLegacy);
  }
  return next;
}

const currentThemeState = ref<AppThemeState>({
  kind: "generated",
  controls: cloneGeneratedThemeControls(DEFAULT_GENERATED_THEME_CONTROLS),
});
const currentTheme = ref<string>(themeStateToThemeId(currentThemeState.value));
const activeGeneratedMode = ref<ThemeMode>(DEFAULT_GENERATED_THEME_CONTROLS.mode);
const generatedThemeControlsByMode = ref<GeneratedThemeControlsByMode>(createGeneratedThemeControlsByMode());
const generatedThemeControls = computed(() => generatedThemeControlsByMode.value[activeGeneratedMode.value]);

function isGeneratedThemeState(value: unknown): value is Extract<AppThemeState, { kind: "generated" }> {
  if (!value || typeof value !== "object") return false;
  const state = value as { kind?: unknown; controls?: unknown };
  return state.kind === "generated" && !!state.controls && typeof state.controls === "object";
}

function cloneThemeState(state: AppThemeState): AppThemeState {
  if (state.kind === "preset") {
    return {
      kind: "preset",
      name: state.name,
    };
  }
  return {
    kind: "generated",
    controls: cloneGeneratedThemeControls(state.controls),
  };
}

function isValidTheme(value: unknown): value is AppTheme {
  return typeof value === "string" && THEME_SET.has(value);
}

function ensureGeneratedThemeStyleElement(): HTMLStyleElement | null {
  if (typeof document === "undefined") return null;
  const existing = document.getElementById(GENERATED_THEME_STYLE_ID);
  if (existing instanceof HTMLStyleElement) {
    return existing;
  }
  const element = document.createElement("style");
  element.id = GENERATED_THEME_STYLE_ID;
  document.head.appendChild(element);
  return element;
}

function clearGeneratedThemeStyle() {
  if (typeof document === "undefined") return;
  const element = document.getElementById(GENERATED_THEME_STYLE_ID);
  if (element) {
    element.remove();
  }
}

function persistThemePreferences() {
  if (typeof window === "undefined") return;
  const activeState = cloneThemeState(currentThemeState.value);
  const payload: PersistedThemePreferences = {
    version: 2,
    activeState,
    generatedControls: cloneGeneratedThemeControls(generatedThemeControls.value),
    generatedControlsByMode: createGeneratedThemeControlsByMode(generatedThemeControlsByMode.value),
  };
  window.localStorage.setItem(THEME_STORAGE_KEY, JSON.stringify(payload));
  window.localStorage.setItem(LEGACY_THEME_STORAGE_KEY, currentTheme.value);
}

function applyGeneratedTheme(controlsInput: Partial<GeneratedThemeControls> | GeneratedThemeControls): boolean {
  if (typeof document === "undefined") return false;
  const controls = cloneGeneratedThemeControls(controlsInput);
  const tokens = generateGeneratedThemeTokens(controls);
  const styleElement = ensureGeneratedThemeStyleElement();
  if (!styleElement) return false;
  generatedThemeControlsByMode.value = {
    ...generatedThemeControlsByMode.value,
    [controls.mode]: controls,
  };
  activeGeneratedMode.value = controls.mode;
  currentThemeState.value = { kind: "generated", controls };
  currentTheme.value = themeStateToThemeId(currentThemeState.value);
  styleElement.textContent = buildGeneratedThemeStyleText(tokens);
  document.documentElement.setAttribute("data-theme", GENERATED_THEME_NAME);
  persistThemePreferences();
  return true;
}

function applyPresetTheme(theme: AppTheme): boolean {
  if (typeof document === "undefined") return false;
  currentTheme.value = theme;
  currentThemeState.value = { kind: "preset", name: theme };
  clearGeneratedThemeStyle();
  document.documentElement.setAttribute("data-theme", theme);
  persistThemePreferences();
  return true;
}

function resolveLegacyThemeMode(value: unknown): ThemeMode {
  const themeName = String(value || "").trim().toLowerCase();
  return LEGACY_DARK_THEME_NAMES.has(themeName) ? "dark" : "light";
}

function applyThemeState(nextState: AppThemeState | string | null | undefined): boolean {
  if (typeof nextState === "string") {
    if (isValidTheme(nextState)) {
      return applyPresetTheme(nextState);
    }
    if (nextState === GENERATED_THEME_LIGHT_ID || nextState === GENERATED_THEME_DARK_ID) {
      const targetMode = nextState === GENERATED_THEME_LIGHT_ID ? "light" : "dark";
      return applyGeneratedTheme(generatedThemeControlsByMode.value[targetMode]);
    }
    return applyGeneratedTheme(generatedThemeControlsByMode.value[resolveLegacyThemeMode(nextState)]);
  }
  if (!nextState) return false;
  if (nextState.kind === "preset") {
    return isValidTheme(nextState.name) ? applyPresetTheme(nextState.name) : false;
  }
  return applyGeneratedTheme(nextState.controls);
}

function readStoredThemePreferences(): PersistedThemePreferences | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedThemePreferences> | null;
    if (!parsed || typeof parsed !== "object") return null;
    const storedControls = cloneGeneratedThemeControls(parsed.generatedControls || DEFAULT_GENERATED_THEME_CONTROLS);
    const storedControlsByMode = createGeneratedThemeControlsByMode(
      parsed.generatedControlsByMode && typeof parsed.generatedControlsByMode === "object"
        ? parsed.generatedControlsByMode
        : null,
      storedControls,
    );
    const activeState = parsed.activeState && typeof parsed.activeState === "object" ? parsed.activeState : null;
    if (isGeneratedThemeState(activeState)) {
      const activeControls = cloneGeneratedThemeControls(activeState.controls);
      storedControlsByMode[activeControls.mode] = activeControls;
      return {
        version: 2,
        activeState: {
          kind: "generated",
          controls: activeControls,
        },
        generatedControls: storedControls,
        generatedControlsByMode: storedControlsByMode,
      };
    }
    const legacyThemeName = (activeState as { name?: unknown } | null)?.name;
    if (isValidTheme(legacyThemeName)) {
      return {
        version: 2,
        activeState: {
          kind: "preset",
          name: legacyThemeName,
        },
        generatedControls: storedControls,
        generatedControlsByMode: storedControlsByMode,
      };
    }
    const migratedMode =
      legacyThemeName === undefined || legacyThemeName === null || String(legacyThemeName).trim() === ""
        ? storedControls.mode
        : resolveLegacyThemeMode(legacyThemeName);
    const migratedControls = cloneGeneratedThemeControls({
      ...storedControlsByMode[migratedMode],
      ...storedControls,
      mode: migratedMode,
    });
    storedControlsByMode[migratedMode] = migratedControls;
    return {
      version: 2,
      activeState: {
        kind: "generated",
        controls: migratedControls,
      },
      generatedControls: storedControls,
      generatedControlsByMode: storedControlsByMode,
    };
  } catch {
    return null;
  }
}

export function isDarkAppTheme(theme: string): boolean {
  const normalizedTheme = String(theme || "").trim();
  return normalizedTheme === GENERATED_THEME_DARK_ID || DARK_APP_THEMES.has(normalizedTheme);
}

export function useAppTheme() {
  const generatedThemeTokens = computed(() => generateGeneratedThemeTokens(generatedThemeControls.value));

  function applyTheme(theme: AppThemeState | string): boolean {
    return applyThemeState(theme);
  }

  function restoreThemeFromStorage() {
    const storedPreferences = readStoredThemePreferences();
    if (storedPreferences) {
      generatedThemeControlsByMode.value = createGeneratedThemeControlsByMode(
        storedPreferences.generatedControlsByMode,
        storedPreferences.generatedControls,
      );
      if (storedPreferences.activeState.kind === "preset") {
        if (isValidTheme(storedPreferences.activeState.name)) {
          applyPresetTheme(storedPreferences.activeState.name);
        }
      } else {
        applyGeneratedTheme(storedPreferences.activeState.controls);
      }
      return;
    }

    if (typeof window === "undefined") return;
    const savedTheme = window.localStorage.getItem(LEGACY_THEME_STORAGE_KEY);
    if (isValidTheme(savedTheme)) {
      applyPresetTheme(savedTheme);
      return;
    }
    const fallbackMode = savedTheme ? resolveLegacyThemeMode(savedTheme) : activeGeneratedMode.value;
    applyGeneratedTheme(generatedThemeControlsByMode.value[fallbackMode]);
  }

  function emitThemeChanged(state: AppThemeState) {
    const payload = cloneThemeState(state);
    return emit("easy-call:theme-changed", payload).catch((error) => {
      console.warn("[THEME] emit easy-call:theme-changed failed:", error);
    });
  }

  function setTheme(theme: string) {
    if (!isValidTheme(theme)) return;
    if (!applyPresetTheme(theme)) return;
    void emitThemeChanged({ kind: "preset", name: theme });
  }

  function activateGeneratedTheme() {
    const targetMode =
      currentThemeState.value.kind === "preset" ? resolveLegacyThemeMode(currentTheme.value) : activeGeneratedMode.value;
    const nextState: AppThemeState = {
      kind: "generated",
      controls: cloneGeneratedThemeControls(generatedThemeControlsByMode.value[targetMode]),
    };
    if (!applyGeneratedTheme(nextState.controls)) return;
    void emitThemeChanged(nextState);
  }

  function updateGeneratedThemeControls(patch: Partial<GeneratedThemeControls>) {
    const targetMode = patch.mode ?? activeGeneratedMode.value;
    const baseControls = generatedThemeControlsByMode.value[targetMode];
    const nextControls = cloneGeneratedThemeControls({
      ...baseControls,
      ...patch,
      mode: targetMode,
    });
    if (!applyGeneratedTheme(nextControls)) return;
    void emitThemeChanged({
      kind: "generated",
      controls: nextControls,
    });
  }

  function resetGeneratedTheme() {
    updateGeneratedThemeControls(getGeneratedThemeDefaultControls(activeGeneratedMode.value));
  }

  function toggleTheme() {
    if (currentThemeState.value.kind === "preset") {
      setTheme(currentTheme.value === "light" ? "dark" : "light");
      return;
    }
    const nextMode = currentThemeState.value.controls.mode === "light" ? "dark" : "light";
    const nextControls = cloneGeneratedThemeControls(generatedThemeControlsByMode.value[nextMode]);
    if (!applyGeneratedTheme(nextControls)) return;
    void emitThemeChanged({
      kind: "generated",
      controls: nextControls,
    });
  }

  return {
    currentTheme,
    generatedThemeControls,
    generatedThemeTokens,
    applyTheme,
    setTheme,
    activateGeneratedTheme,
    updateGeneratedThemeControls,
    resetGeneratedTheme,
    restoreThemeFromStorage,
    toggleTheme,
  };
}
