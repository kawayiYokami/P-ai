import { computed, ref } from "vue";
import { emitTransportEvent, isDesktopTauriHost } from "../../../services/tauri-api";
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
  GeneratedThemeTokens,
  PersistedThemePreferences,
  ThemeMode,
  ThemeModeKind,
  ThemeSourceKind,
} from "../theme/theme-types";

const THEME_STORAGE_KEY = "easy-call.theme-state.v1";
const LEGACY_THEME_STORAGE_KEY = "theme";
const GENERATED_THEME_STYLE_ID = "easy-call-generated-theme-style";
const LEGACY_DARK_THEME_NAMES = new Set<string>([
  "dark",
  "halloween",
  "forest",
  "dracula",
  "business",
  "dim",
]);
export const APP_THEMES = [
  "light",
  "dark",
  "cupcake",
  "corporate",
  "halloween",
  "forest",
  "lofi",
  "fantasy",
  "dracula",
  "autumn",
  "business",
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
/** 自定义编辑器的当前编辑侧；显示模式切到明/暗时跟着走，由编辑器里的明暗分段控件改写。 */
const activeGeneratedMode = ref<ThemeMode>(DEFAULT_GENERATED_THEME_CONTROLS.mode);
const generatedThemeControlsByMode = ref<GeneratedThemeControlsByMode>(createGeneratedThemeControlsByMode());
const generatedThemeControls = computed(() => generatedThemeControlsByMode.value[activeGeneratedMode.value]);
// 两个维度各自独立：显示模式（明/暗/自动）+ 外观来源（预设/自定义）
const themeMode = ref<ThemeModeKind>("auto");
const themeSource = ref<ThemeSourceKind>("generated");
const presetLightTheme = ref<string>(GENERATED_THEME_LIGHT_ID);
const presetDarkTheme = ref<string>(GENERATED_THEME_DARK_ID);
const systemPrefersDark = ref(false);
let systemThemeListenerRegistered = false;

export function isVscodeHost(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.getAttribute("data-host") === "vscode";
}

function resolveSystemPrefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** 当前实际生效的明暗：自动档看系统，明/暗档就是自己。 */
function resolveActiveMode(): ThemeMode {
  if (themeMode.value === "auto") {
    // 自动档只属于桌面宿主：Web / VS Code 侧栏没有「跟随系统」这一档，
    // 明暗由各自前端存的那一份决定，不读宿主系统深浅。
    if (!isDesktopTauriHost()) return "light";
    return systemPrefersDark.value ? "dark" : "light";
  }
  return themeMode.value;
}

function registerSystemThemeListener() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
  // 只有桌面宿主跟随系统深浅；Web / VS Code 侧栏的主题各自由前端存档决定。
  if (!isDesktopTauriHost()) return;
  if (systemThemeListenerRegistered) return;
  systemThemeListenerRegistered = true;
  systemPrefersDark.value = resolveSystemPrefersDark();
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (event) => {
    systemPrefersDark.value = event.matches;
    if (themeMode.value === "auto") {
      applyCurrentTheme();
    }
  });
}

function isGeneratedThemeState(value: unknown): value is Extract<AppThemeState, { kind: "generated" }> {
  if (!value || typeof value !== "object") return false;
  const state = value as { kind?: unknown; controls?: unknown };
  return state.kind === "generated" && !!state.controls && typeof state.controls === "object";
}

function isValidTheme(value: unknown): value is AppTheme {
  return typeof value === "string" && THEME_SET.has(value);
}

function normalizePresetTheme(value: unknown): AppTheme | null {
  const themeName = String(value || "").trim().toLowerCase();
  if (themeName === "pastel") return "autumn";
  return isValidTheme(themeName) ? themeName : null;
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

function buildPersistedPreferences(): PersistedThemePreferences {
  return {
    version: 4,
    mode: themeMode.value,
    source: themeSource.value,
    presetLightTheme: presetLightTheme.value,
    presetDarkTheme: presetDarkTheme.value,
    generatedControlsByMode: createGeneratedThemeControlsByMode(generatedThemeControlsByMode.value),
  };
}

function persistThemePreferences() {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(THEME_STORAGE_KEY, JSON.stringify(buildPersistedPreferences()));
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

/** 明/暗两侧各自记一张卡：要么某个内置预设，要么「自定义浅色 / 自定义深色」。 */
function isGeneratedCard(cardId: string): boolean {
  return cardId === GENERATED_THEME_LIGHT_ID || cardId === GENERATED_THEME_DARK_ID;
}

function sideCardId(side: ThemeMode): string {
  return side === "dark" ? presetDarkTheme.value : presetLightTheme.value;
}

/** 按「来源 × 当前生效明暗」落一次主题：这是唯一的施加入口。 */
function applyCurrentTheme(): boolean {
  const mode = resolveActiveMode();
  const cardId = sideCardId(mode);
  // 来源是派生值：只看这一侧的卡是不是自定义。切「预设/自定义」tab 不参与。
  themeSource.value = isGeneratedCard(cardId) ? "generated" : "preset";
  if (isGeneratedCard(cardId)) {
    return applyGeneratedTheme(generatedThemeControlsByMode.value[mode]);
  }
  const preset = normalizePresetTheme(cardId);
  return preset ? applyPresetTheme(preset) : false;
}

function resolveLegacyThemeMode(value: unknown): ThemeMode {
  const themeName = String(value || "").trim().toLowerCase();
  return LEGACY_DARK_THEME_NAMES.has(themeName) ? "dark" : "light";
}

/**
 * 把存档归一成 v4。
 *
 * 迁移只做单向升级，不写降级分支。四条老状态各自的落点：
 * - manual + preset(浅) → mode=light、source=preset，主题名填 presetLightTheme
 * - manual + preset(深) → mode=dark、source=preset，填 presetDarkTheme
 * - manual + generated → source=generated，mode 按当时生效那一侧
 * - auto + 任意 → mode=auto
 */
export function migratePersistedThemePreferences(raw: unknown): PersistedThemePreferences | null {
  if (!raw || typeof raw !== "object") return null;
  const parsed = raw as Record<string, unknown>;

  // 已是 v4：只做兜底归一，不动用户选择
  if ("source" in parsed && "presetLightTheme" in parsed) {
    const storedMode = parsed.mode;
    const storedSource = parsed.source;
    return {
      version: 4,
      mode: storedMode === "light" || storedMode === "dark" || storedMode === "auto" ? storedMode : "auto",
      source: storedSource === "generated" ? "generated" : "preset",
      presetLightTheme:
        normalizePresetTheme(parsed.presetLightTheme) || GENERATED_THEME_LIGHT_ID,
      presetDarkTheme:
        normalizePresetTheme(parsed.presetDarkTheme) || GENERATED_THEME_DARK_ID,
      generatedControlsByMode: createGeneratedThemeControlsByMode(
        parsed.generatedControlsByMode && typeof parsed.generatedControlsByMode === "object"
          ? (parsed.generatedControlsByMode as Partial<GeneratedThemeControlsByMode>)
          : null,
      ),
    };
  }

  const storedControls = cloneGeneratedThemeControls(
    (parsed.generatedControls as Partial<GeneratedThemeControls> | undefined) || DEFAULT_GENERATED_THEME_CONTROLS,
  );
  const storedControlsByMode = createGeneratedThemeControlsByMode(
    parsed.generatedControlsByMode && typeof parsed.generatedControlsByMode === "object"
      ? (parsed.generatedControlsByMode as Partial<GeneratedThemeControlsByMode>)
      : null,
    storedControls,
  );
  // 预设槽位：v3 的 autoLightTheme / autoDarkTheme 直接搬过来
  let presetLight =
    normalizePresetTheme(parsed.autoLightTheme) || (GENERATED_THEME_LIGHT_ID as string);
  let presetDark = normalizePresetTheme(parsed.autoDarkTheme) || (GENERATED_THEME_DARK_ID as string);
  const legacyAuto = parsed.mode === "auto";
  const activeState = parsed.activeState && typeof parsed.activeState === "object" ? parsed.activeState : null;

  if (isGeneratedThemeState(activeState)) {
    const activeControls = cloneGeneratedThemeControls(activeState.controls);
    storedControlsByMode[activeControls.mode] = activeControls;
    return {
      version: 4,
      mode: legacyAuto ? "auto" : activeControls.mode,
      source: "generated",
      presetLightTheme: presetLight,
      presetDarkTheme: presetDark,
      generatedControlsByMode: storedControlsByMode,
    };
  }

  const legacyThemeName = (activeState as { name?: unknown } | null)?.name;
  const presetTheme = normalizePresetTheme(legacyThemeName);
  if (presetTheme) {
    // 老的手动预设只有一个名字，按它自己的明暗属性落进对应槽位，另一槽保留原值
    const presetMode: ThemeMode = isDarkAppTheme(presetTheme) ? "dark" : "light";
    if (presetMode === "dark") {
      presetDark = presetTheme;
    } else {
      presetLight = presetTheme;
    }
    return {
      version: 4,
      mode: legacyAuto ? "auto" : presetMode,
      source: "preset",
      presetLightTheme: presetLight,
      presetDarkTheme: presetDark,
      generatedControlsByMode: storedControlsByMode,
    };
  }

  // 更老的存档：没有 activeState，或只有一个认不出的主题名
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
    version: 4,
    mode: legacyAuto ? "auto" : migratedMode,
    source: "generated",
    presetLightTheme: presetLight,
    presetDarkTheme: presetDark,
    generatedControlsByMode: storedControlsByMode,
  };
}

function readStoredThemePreferences(): PersistedThemePreferences | null {
  if (typeof window === "undefined") return null;
  const raw = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (!raw) return null;
  try {
    return migratePersistedThemePreferences(JSON.parse(raw));
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
  const generatedThemeTokensByMode = computed<Record<ThemeMode, GeneratedThemeTokens>>(() => ({
    light: generateGeneratedThemeTokens(generatedThemeControlsByMode.value.light),
    dark: generateGeneratedThemeTokens(generatedThemeControlsByMode.value.dark),
  }));

  function isPersistedPreferences(value: unknown): value is PersistedThemePreferences {
    if (!value || typeof value !== "object") return false;
    // v3 的 payload 也认：跨窗口同步时偶尔会收到旧结构，交给同一个迁移函数归一
    return "source" in value || "activeState" in value;
  }

  function adoptPreferences(preferences: PersistedThemePreferences) {
    generatedThemeControlsByMode.value = createGeneratedThemeControlsByMode(
      preferences.generatedControlsByMode,
    );
    presetLightTheme.value = preferences.presetLightTheme || GENERATED_THEME_LIGHT_ID;
    presetDarkTheme.value = preferences.presetDarkTheme || GENERATED_THEME_DARK_ID;
    themeMode.value = preferences.mode;
    themeSource.value = preferences.source;
  }

  function applyThemeState(nextState: AppThemeState | string | null | undefined): boolean {
    if (typeof nextState === "string") {
      const presetTheme = normalizePresetTheme(nextState);
      if (presetTheme) {
        return applyPresetTheme(presetTheme);
      }
      if (nextState === GENERATED_THEME_LIGHT_ID || nextState === GENERATED_THEME_DARK_ID) {
        const targetMode: ThemeMode = nextState === GENERATED_THEME_LIGHT_ID ? "light" : "dark";
        return applyGeneratedTheme(generatedThemeControlsByMode.value[targetMode]);
      }
      return applyGeneratedTheme(generatedThemeControlsByMode.value[resolveLegacyThemeMode(nextState)]);
    }
    if (!nextState) return false;
    if (nextState.kind === "preset") {
      const presetTheme = normalizePresetTheme(nextState.name);
      return presetTheme ? applyPresetTheme(presetTheme) : false;
    }
    return applyGeneratedTheme(nextState.controls);
  }

  function applyTheme(theme: PersistedThemePreferences | AppThemeState | string): boolean {
    // 跨窗口同步：配置窗口广播完整状态，各窗口按「来源 × 模式」自行落主题
    if (isPersistedPreferences(theme)) {
      const preferences = migratePersistedThemePreferences(theme);
      if (!preferences) return false;
      adoptPreferences(preferences);
      return applyCurrentTheme();
    }
    return applyThemeState(theme);
  }

  function restoreThemeFromStorage() {
    registerSystemThemeListener();
    const storedPreferences = readStoredThemePreferences();
    if (storedPreferences) {
      adoptPreferences(storedPreferences);
      applyCurrentTheme();
      return;
    }

    if (typeof window === "undefined") return;
    const savedTheme = window.localStorage.getItem(LEGACY_THEME_STORAGE_KEY);
    if (isValidTheme(savedTheme)) {
      // 旧存档只记了一个主题名：把它落到对应那一侧的卡上，再走统一施加入口
      themeMode.value = DARK_APP_THEMES.has(savedTheme) ? "dark" : "light";
      if (themeMode.value === "dark") {
        presetDarkTheme.value = savedTheme;
      } else {
        presetLightTheme.value = savedTheme;
      }
      applyCurrentTheme();
      return;
    }
    // 全新安装无任何主题记录：默认自动档 + 自定义来源，浅色/深色都用自定义主题。
    themeMode.value = "auto";
    themeSource.value = "generated";
    presetLightTheme.value = GENERATED_THEME_LIGHT_ID;
    presetDarkTheme.value = GENERATED_THEME_DARK_ID;
    applyCurrentTheme();
  }

  function emitThemeChanged() {
    const payload = buildPersistedPreferences();
    return emitTransportEvent("theme.changed", payload).catch((error) => {
      console.warn("[主题] 同步主题变化失败", error);
    });
  }

  /** 切显示模式：明 / 暗 / 自动。不影响来源。 */
  function setThemeMode(mode: ThemeModeKind) {
    const next: ThemeModeKind = mode === "light" || mode === "dark" ? mode : "auto";
    themeMode.value = next;
    applyCurrentTheme();
    void emitThemeChanged();
  }

  /** 把当前生效侧的卡片切成「预设」或「自定义」。切 tab 不再调它，只由勾卡路径改来源。 */
  function setThemeSource(source: ThemeSourceKind) {
    const side = resolveActiveMode();
    if (source === "generated") {
      if (side === "light") {
        presetLightTheme.value = GENERATED_THEME_LIGHT_ID;
      } else {
        presetDarkTheme.value = GENERATED_THEME_DARK_ID;
      }
    } else {
      const preset = normalizePresetTheme(sideCardId(side)) ?? (side === "dark" ? "dark" : "light");
      if (side === "light") {
        presetLightTheme.value = preset;
      } else {
        presetDarkTheme.value = preset;
      }
    }
    applyCurrentTheme();
    void emitThemeChanged();
  }

  /**
   * 勾某一侧的主题卡片：这张卡就是这一侧（明 / 暗）生效的主题。
   * 明暗两侧各自独立——勾了深色侧的自定义卡，不会把浅色侧也从预设改成自定义。
   */
  function setPresetTheme(side: ThemeMode, themeId: string) {
    const rawId = String(themeId || "").trim().toLowerCase();
    const cardId = isGeneratedCard(rawId) ? rawId : normalizePresetTheme(rawId);
    if (!cardId) return;
    if (side === "light") {
      presetLightTheme.value = cardId;
    } else {
      presetDarkTheme.value = cardId;
    }
    applyCurrentTheme();
    void emitThemeChanged();
  }

  function updateGeneratedThemeControls(patch: Partial<GeneratedThemeControls>) {
    const targetMode = patch.mode ?? activeGeneratedMode.value;
    const baseControls = generatedThemeControlsByMode.value[targetMode];
    const nextControls = cloneGeneratedThemeControls({
      ...baseControls,
      ...patch,
      mode: targetMode,
    });
    generatedThemeControlsByMode.value = {
      ...generatedThemeControlsByMode.value,
      [targetMode]: nextControls,
    };
    activeGeneratedMode.value = targetMode;
    // 只在「这一侧用的是自定义卡，且这一侧正在生效」时重落主题；
    // 编辑另一侧、或这一侧在预设状态下的参数，都不该把页面跳走。
    if (isGeneratedCard(sideCardId(targetMode)) && resolveActiveMode() === targetMode) {
      if (!applyGeneratedTheme(nextControls)) return;
    } else {
      persistThemePreferences();
    }
    void emitThemeChanged();
  }

  function resetGeneratedTheme() {
    updateGeneratedThemeControls(getGeneratedThemeDefaultControls(activeGeneratedMode.value));
  }

  function toggleTheme() {
    if (themeMode.value === "auto") {
      // 自动档跟随系统，忽略手动明暗切换
      return;
    }
    themeMode.value = resolveActiveMode() === "dark" ? "light" : "dark";
    applyCurrentTheme();
    void emitThemeChanged();
  }

  return {
    currentTheme,
    generatedThemeControls,
    generatedThemeControlsByMode,
    generatedThemeTokens,
    generatedThemeTokensByMode,
    themeMode,
    themeSource,
    presetLightTheme,
    presetDarkTheme,
    systemPrefersDark,
    applyTheme,
    setThemeMode,
    setThemeSource,
    setPresetTheme,
    updateGeneratedThemeControls,
    resetGeneratedTheme,
    restoreThemeFromStorage,
    toggleTheme,
  };
}
