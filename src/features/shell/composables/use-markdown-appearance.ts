import { ref } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

const MARKDOWN_APPEARANCE_STORAGE_KEY = "easy-call.markdown-appearance.v1";
const MARKDOWN_APPEARANCE_CHANGED_EVENT = "easy-call:markdown-appearance-changed";

export const MARKDOWN_FONT_SCALE_MIN = 0;
export const MARKDOWN_FONT_SCALE_MAX = 1;
export const MARKDOWN_FONT_SCALE_DEFAULT = 0;

type MarkdownAppearancePayload = {
  fontScale?: number;
};

const markdownFontScale = ref(MARKDOWN_FONT_SCALE_DEFAULT);
let initialized = false;
let eventUnlisten: UnlistenFn | null = null;

function clampFontScale(value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return MARKDOWN_FONT_SCALE_DEFAULT;
  return Math.min(MARKDOWN_FONT_SCALE_MAX, Math.max(MARKDOWN_FONT_SCALE_MIN, Math.round(numeric)));
}

function markdownWeightsForScale(scale: number) {
  const normalized = clampFontScale(scale);
  const bold = normalized >= 1;
  return {
    body: bold ? 850 : 400,
    heading: bold ? 900 : 600,
    strong: bold ? 900 : 700,
    code: bold ? 850 : 500,
    tableHeading: bold ? 900 : 600,
  };
}

function readStoredMarkdownFontScale(): number {
  if (typeof window === "undefined") return MARKDOWN_FONT_SCALE_DEFAULT;
  const raw = window.localStorage.getItem(MARKDOWN_APPEARANCE_STORAGE_KEY);
  if (!raw) return MARKDOWN_FONT_SCALE_DEFAULT;
  try {
    const parsed = JSON.parse(raw) as MarkdownAppearancePayload | null;
    return clampFontScale(parsed?.fontScale);
  } catch {
    return MARKDOWN_FONT_SCALE_DEFAULT;
  }
}

function persistMarkdownFontScale(scale: number) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(
    MARKDOWN_APPEARANCE_STORAGE_KEY,
    JSON.stringify({ version: 1, fontScale: clampFontScale(scale) }),
  );
}

function applyMarkdownFontScale(scale: unknown) {
  const normalized = clampFontScale(scale);
  markdownFontScale.value = normalized;
  if (typeof document === "undefined") return;
  const weights = markdownWeightsForScale(normalized);
  const root = document.documentElement.style;
  root.setProperty("--ecall-md-font-scale", String(normalized));
  root.setProperty("--ecall-md-body-weight-setting", String(weights.body));
  root.setProperty("--ecall-md-heading-weight-setting", String(weights.heading));
  root.setProperty("--ecall-md-strong-weight-setting", String(weights.strong));
  root.setProperty("--ecall-md-code-weight-setting", String(weights.code));
  root.setProperty("--ecall-md-table-heading-weight-setting", String(weights.tableHeading));
}

function restoreMarkdownAppearanceFromStorage() {
  applyMarkdownFontScale(readStoredMarkdownFontScale());
}

function handleStorageEvent(event: StorageEvent) {
  if (event.key !== MARKDOWN_APPEARANCE_STORAGE_KEY) return;
  restoreMarkdownAppearanceFromStorage();
}

export function initMarkdownAppearance() {
  if (initialized) return;
  initialized = true;
  restoreMarkdownAppearanceFromStorage();
  if (typeof window !== "undefined") {
    window.addEventListener("storage", handleStorageEvent);
  }
  void listen<MarkdownAppearancePayload>(MARKDOWN_APPEARANCE_CHANGED_EVENT, (event) => {
    applyMarkdownFontScale(event.payload?.fontScale);
  }).then((unlisten) => {
    eventUnlisten = unlisten;
  });
}

export function disposeMarkdownAppearance() {
  if (!initialized) return;
  initialized = false;
  if (typeof window !== "undefined") {
    window.removeEventListener("storage", handleStorageEvent);
  }
  eventUnlisten?.();
  eventUnlisten = null;
}

export function useMarkdownAppearance() {
  initMarkdownAppearance();

  function setMarkdownFontScale(value: number) {
    const normalized = clampFontScale(value);
    applyMarkdownFontScale(normalized);
    persistMarkdownFontScale(normalized);
    void emit(MARKDOWN_APPEARANCE_CHANGED_EVENT, { fontScale: normalized }).catch((error) => {
      console.warn("[Markdown外观] 同步字体强度失败", error);
    });
  }

  return {
    markdownFontScale,
    setMarkdownFontScale,
  };
}
