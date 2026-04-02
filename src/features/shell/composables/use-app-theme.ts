import { ref } from "vue";
import { emit } from "@tauri-apps/api/event";

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

export const DARK_APP_THEMES = new Set<string>([
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

export type AppTheme = (typeof APP_THEMES)[number];
const THEME_SET = new Set<string>(APP_THEMES);
const currentTheme = ref<AppTheme>("business");

function isValidTheme(value: unknown): value is AppTheme {
  return typeof value === "string" && THEME_SET.has(value);
}

export function isDarkAppTheme(theme: string): boolean {
  return DARK_APP_THEMES.has(String(theme || "").trim());
}

export function useAppTheme() {
  function applyTheme(theme: string): boolean {
    if (!isValidTheme(theme)) return false;
    currentTheme.value = theme;
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
    return true;
  }

  function restoreThemeFromStorage() {
    const savedTheme = localStorage.getItem("theme");
    if (isValidTheme(savedTheme)) {
      applyTheme(savedTheme);
    }
  }

  function setTheme(theme: string) {
    if (!applyTheme(theme)) return;
    void emit("easy-call:theme-changed", theme).catch((error) => {
      console.warn("[THEME] emit easy-call:theme-changed failed:", error);
    });
  }

  function toggleTheme() {
    const next = currentTheme.value === "light" ? "dark" : "light";
    setTheme(next);
  }

  return {
    currentTheme,
    applyTheme,
    setTheme,
    restoreThemeFromStorage,
    toggleTheme,
  };
}
