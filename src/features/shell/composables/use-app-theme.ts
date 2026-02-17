import { ref } from "vue";
import { emit } from "@tauri-apps/api/event";

export type AppTheme = "light" | "forest";
const THEME_SET: AppTheme[] = ["light", "forest"];
const currentTheme = ref<AppTheme>("light");

function isValidTheme(value: unknown): value is AppTheme {
  return typeof value === "string" && THEME_SET.includes(value as AppTheme);
}

export function useAppTheme() {
  function applyTheme(theme: AppTheme) {
    currentTheme.value = theme;
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }

  function restoreThemeFromStorage() {
    const savedTheme = localStorage.getItem("theme");
    if (isValidTheme(savedTheme)) {
      applyTheme(savedTheme);
    }
  }

  function toggleTheme() {
    const next = currentTheme.value === "light" ? "forest" : "light";
    applyTheme(next);
    void emit("easy-call:theme-changed", next).catch((error) => {
      console.warn("[THEME] emit easy-call:theme-changed failed:", error);
    });
  }

  return {
    currentTheme,
    applyTheme,
    restoreThemeFromStorage,
    toggleTheme,
  };
}
