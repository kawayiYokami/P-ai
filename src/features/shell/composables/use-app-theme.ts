import { ref } from "vue";
import { emit } from "@tauri-apps/api/event";

export type AppTheme = "light" | "forest";

export function useAppTheme() {
  const currentTheme = ref<AppTheme>("light");

  function applyTheme(theme: AppTheme) {
    currentTheme.value = theme;
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  }

  function restoreThemeFromStorage() {
    const savedTheme = localStorage.getItem("theme") as AppTheme | null;
    if (savedTheme) {
      applyTheme(savedTheme);
    }
  }

  function toggleTheme() {
    const next = currentTheme.value === "light" ? "forest" : "light";
    applyTheme(next);
    emit("easy-call:theme-changed", next);
  }

  return {
    currentTheme,
    applyTheme,
    restoreThemeFromStorage,
    toggleTheme,
  };
}

