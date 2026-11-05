import { ref } from "vue";
import { emitTransportEvent, onTransportNotification } from "../../../services/tauri-api";

const UI_SIZE_STORAGE_KEY = "easy-call.ui-size.v1";
const UI_SIZE_HINT_HIDE_MS = 900;
const UI_SIZE_HINT_ELEMENT_ID = "easy-call-ui-size-hint";

export const UI_SIZE_MIN_SCALE = 75;
export const UI_SIZE_MAX_SCALE = 150;
export const UI_SIZE_DEFAULT_SCALE = 100;
export type UiSizeScale = number;

type UiSizePayload = {
  scale?: unknown;
  preset?: unknown;
};

type UiSizeTokens = {
  textMicro: string;
  textCaption: string;
  textXs: string;
  textSm: string;
  textBase: string;
  textLg: string;
  textXl: string;
  text2Xl: string;
  markdownHeading1: string;
  markdownHeading2: string;
  markdownHeading3: string;
  markdownHeading4: string;
  markdownDocumentHeading1: string;
  markdownDocumentHeading2: string;
  markdownDocumentHeading3: string;
  markdownDocumentHeading4: string;
  sizeField: string;
  sizeSelector: string;
  border: string;
};

const LEGACY_UI_SIZE_SCALE_MAP: Record<string, UiSizeScale> = {
  small: 75,
  default: 100,
  large: 125,
  extraLarge: 150,
};

const uiSizeScale = ref<UiSizeScale>(UI_SIZE_DEFAULT_SCALE);
let initialized = false;
let eventUnlisten: (() => void) | null = null;
let hintHideTimer: ReturnType<typeof window.setTimeout> | null = null;

/**
 * 界面尺寸是四档字号，不再按百分比缩放。
 *
 * 以前的做法是滑杆给一个「占屏面积」百分比（75-150），线性尺寸取它的平方根，
 * 于是 150% 档下 14px 会变成 17.15px、边框变成 1.22px。字号一旦不是整数，
 * 行高、图标与间距就同时落在半像素上，看起来哪里都差一点；换行宽度的抖动也来自这里。
 *
 * 现在每档一张整数 px 表，档位之间只换表、不做乘法。对外仍用 uiSizeScale 这个字段，
 * 取值收敛到四档编号（75/100/125/150），这样配置字段、三个窗口、持久化都不用改。
 */
type UiSizePreset = "compact" | "default" | "large" | "xlarge";

const UI_SIZE_PRESET_SCALES: Record<UiSizePreset, UiSizeScale> = {
  compact: 75,
  default: 100,
  large: 125,
  xlarge: 150,
};

const UI_SIZE_PRESET_ORDER: UiSizePreset[] = ["compact", "default", "large", "xlarge"];

/** 每档的整数 px 表。新增字号只在这四张表里加整数，不要回头引入乘倍数。 */
const UI_SIZE_TOKEN_TABLE: Record<UiSizePreset, UiSizeTokens> = {
  compact: {
    textMicro: "10px",
    textCaption: "11px",
    textXs: "12px",
    textSm: "13px",
    textBase: "14px",
    textLg: "16px",
    textXl: "18px",
    text2Xl: "22px",
    markdownHeading1: "15px",
    markdownHeading2: "15px",
    markdownHeading3: "14px",
    markdownHeading4: "14px",
    markdownDocumentHeading1: "20px",
    markdownDocumentHeading2: "18px",
    markdownDocumentHeading3: "16px",
    markdownDocumentHeading4: "15px",
    sizeField: "4px",
    sizeSelector: "4px",
    border: "1px",
  },
  default: {
    textMicro: "11px",
    textCaption: "12px",
    textXs: "13px",
    textSm: "14px",
    textBase: "16px",
    textLg: "18px",
    textXl: "20px",
    text2Xl: "24px",
    markdownHeading1: "16px",
    markdownHeading2: "16px",
    markdownHeading3: "15px",
    markdownHeading4: "14px",
    markdownDocumentHeading1: "24px",
    markdownDocumentHeading2: "20px",
    markdownDocumentHeading3: "18px",
    markdownDocumentHeading4: "16px",
    sizeField: "4px",
    sizeSelector: "4px",
    border: "1px",
  },
  large: {
    textMicro: "12px",
    textCaption: "13px",
    textXs: "14px",
    textSm: "16px",
    textBase: "18px",
    textLg: "20px",
    textXl: "24px",
    text2Xl: "28px",
    markdownHeading1: "18px",
    markdownHeading2: "17px",
    markdownHeading3: "16px",
    markdownHeading4: "15px",
    markdownDocumentHeading1: "28px",
    markdownDocumentHeading2: "24px",
    markdownDocumentHeading3: "20px",
    markdownDocumentHeading4: "18px",
    sizeField: "5px",
    sizeSelector: "5px",
    border: "1px",
  },
  xlarge: {
    textMicro: "13px",
    textCaption: "15px",
    textXs: "16px",
    textSm: "18px",
    textBase: "20px",
    textLg: "24px",
    textXl: "28px",
    text2Xl: "32px",
    markdownHeading1: "20px",
    markdownHeading2: "19px",
    markdownHeading3: "18px",
    markdownHeading4: "16px",
    markdownDocumentHeading1: "32px",
    markdownDocumentHeading2: "28px",
    markdownDocumentHeading3: "24px",
    markdownDocumentHeading4: "20px",
    sizeField: "5px",
    sizeSelector: "5px",
    border: "1px",
  },
};

/** 旧值（任意 75-150 之间的百分比、或 small/default/large/extraLarge）就近归到四档之一。 */
function uiSizePresetOf(scale: UiSizeScale): UiSizePreset {
  let best: UiSizePreset = "default";
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const preset of UI_SIZE_PRESET_ORDER) {
    const distance = Math.abs(UI_SIZE_PRESET_SCALES[preset] - scale);
    if (distance < bestDistance) {
      bestDistance = distance;
      best = preset;
    }
  }
  return best;
}

export function uiSizePresetLabel(value: unknown): UiSizePreset {
  return uiSizePresetOf(normalizeUiSizeScale(value));
}

export function uiSizeScaleOfPreset(preset: UiSizePreset): UiSizeScale {
  return UI_SIZE_PRESET_SCALES[preset];
}

export function normalizeUiSizeScale(value: unknown): UiSizeScale {
  if (value == null || (typeof value === "string" && !value.trim())) {
    return UI_SIZE_DEFAULT_SCALE;
  }
  const raw = typeof value === "string" ? value.trim() : value;
  for (const preset of UI_SIZE_PRESET_ORDER) {
    if (raw === preset) return UI_SIZE_PRESET_SCALES[preset];
  }
  if (typeof raw === "string" && raw in LEGACY_UI_SIZE_SCALE_MAP) {
    return UI_SIZE_PRESET_SCALES[uiSizePresetOf(LEGACY_UI_SIZE_SCALE_MAP[raw])];
  }
  const numeric = Math.round(Number(raw));
  if (!Number.isFinite(numeric)) return UI_SIZE_DEFAULT_SCALE;
  const clamped = Math.min(UI_SIZE_MAX_SCALE, Math.max(UI_SIZE_MIN_SCALE, numeric));
  return UI_SIZE_PRESET_SCALES[uiSizePresetOf(clamped)];
}

export function uiSizeTokensFor(value: unknown): UiSizeTokens {
  return UI_SIZE_TOKEN_TABLE[uiSizePresetOf(normalizeUiSizeScale(value))];
}

function readStoredUiSizeScale(): UiSizeScale {
  if (typeof window === "undefined") return UI_SIZE_DEFAULT_SCALE;
  return normalizeUiSizeScale(window.localStorage.getItem(UI_SIZE_STORAGE_KEY));
}

function persistUiSizeScale(scale: UiSizeScale) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(UI_SIZE_STORAGE_KEY, String(scale));
}

export function applyUiSizeScale(value: unknown): UiSizeScale {
  const scale = normalizeUiSizeScale(value);
  uiSizeScale.value = scale;
  if (typeof document === "undefined") return scale;
  const tokens = uiSizeTokensFor(scale);
  const root = document.documentElement.style;
  root.setProperty("--app-text-micro-size", tokens.textMicro);
  root.setProperty("--app-text-caption-size", tokens.textCaption);
  root.setProperty("--app-text-xs-size", tokens.textXs);
  root.setProperty("--app-text-sm-size", tokens.textSm);
  root.setProperty("--app-text-base-size", tokens.textBase);
  root.setProperty("--app-text-lg-size", tokens.textLg);
  root.setProperty("--app-text-xl-size", tokens.textXl);
  root.setProperty("--app-text-2xl-size", tokens.text2Xl);
  root.setProperty("--app-text-markdown-heading-1-size", tokens.markdownHeading1);
  root.setProperty("--app-text-markdown-heading-2-size", tokens.markdownHeading2);
  root.setProperty("--app-text-markdown-heading-3-size", tokens.markdownHeading3);
  root.setProperty("--app-text-markdown-heading-4-size", tokens.markdownHeading4);
  root.setProperty("--app-text-markdown-document-heading-1-size", tokens.markdownDocumentHeading1);
  root.setProperty("--app-text-markdown-document-heading-2-size", tokens.markdownDocumentHeading2);
  root.setProperty("--app-text-markdown-document-heading-3-size", tokens.markdownDocumentHeading3);
  root.setProperty("--app-text-markdown-document-heading-4-size", tokens.markdownDocumentHeading4);
  root.setProperty("--size-field", tokens.sizeField);
  root.setProperty("--size-selector", tokens.sizeSelector);
  root.setProperty("--border", tokens.border);
  return scale;
}

function handleStorageEvent(event: StorageEvent) {
  if (event.key === UI_SIZE_STORAGE_KEY) applyUiSizeScale(event.newValue);
}

function hasUiSizeZoomModifier(event: WheelEvent | KeyboardEvent) {
  return !!event.ctrlKey || !!event.metaKey;
}

function ensureUiSizeHintElement(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  const existing = document.getElementById(UI_SIZE_HINT_ELEMENT_ID);
  if (existing) return existing;
  const el = document.createElement("div");
  el.id = UI_SIZE_HINT_ELEMENT_ID;
  el.setAttribute("role", "status");
  el.setAttribute("aria-live", "polite");
  el.style.cssText = [
    "position:fixed",
    "left:50%",
    "top:50%",
    "transform:translate(-50%,-50%) scale(0.96)",
    "z-index:2147483000",
    "min-width:4.5rem",
    "padding:0.7rem 1.1rem",
    "border-radius:9999px",
    "border:1px solid color-mix(in oklab, var(--color-base-content) 18%, transparent)",
    "background:color-mix(in oklab, var(--color-base-100) 88%, transparent)",
    "color:var(--color-base-content)",
    "box-shadow:0 10px 30px color-mix(in oklab, var(--color-base-content) 18%, transparent)",
    "backdrop-filter:blur(8px)",
    "font-size:1.25rem",
    "font-weight:700",
    "font-variant-numeric:tabular-nums",
    "letter-spacing:0.01em",
    "line-height:1",
    "text-align:center",
    "pointer-events:none",
    "opacity:0",
    "transition:opacity 120ms ease, transform 120ms ease",
  ].join(";");
  document.body.appendChild(el);
  return el;
}

const UI_SIZE_PRESET_TEXT: Record<UiSizePreset, string> = {
  compact: "紧凑",
  default: "默认",
  large: "大",
  xlarge: "特大",
};

export function showUiSizeScaleHint(scale: UiSizeScale) {
  if (typeof document === "undefined" || typeof window === "undefined") return;
  const el = ensureUiSizeHintElement();
  if (!el) return;
  el.textContent = UI_SIZE_PRESET_TEXT[uiSizePresetOf(normalizeUiSizeScale(scale))];
  el.style.opacity = "1";
  el.style.transform = "translate(-50%, -50%) scale(1)";
  if (hintHideTimer) {
    window.clearTimeout(hintHideTimer);
    hintHideTimer = null;
  }
  hintHideTimer = window.setTimeout(() => {
    el.style.opacity = "0";
    el.style.transform = "translate(-50%, -50%) scale(0.96)";
    hintHideTimer = null;
  }, UI_SIZE_HINT_HIDE_MS);
}

export function stepUiSizeScale(direction: number): UiSizeScale {
  // 四档之间逐档切换，不再按 ±10% 连续加减：中间档位已经不存在了。
  const currentPreset = uiSizePresetOf(normalizeUiSizeScale(uiSizeScale.value));
  const currentIndex = UI_SIZE_PRESET_ORDER.indexOf(currentPreset);
  const nextIndex = Math.min(
    UI_SIZE_PRESET_ORDER.length - 1,
    Math.max(0, currentIndex + (direction > 0 ? 1 : -1)),
  );
  const next = setUiSizeScale(UI_SIZE_PRESET_SCALES[UI_SIZE_PRESET_ORDER[nextIndex]]);
  showUiSizeScaleHint(next);
  return next;
}

function handleGlobalUiSizeWheel(event: WheelEvent) {
  if (!hasUiSizeZoomModifier(event)) return;
  event.preventDefault();
  event.stopPropagation();
  // 向上滚放大，向下滚缩小，每步 10%。
  stepUiSizeScale(event.deltaY < 0 ? 1 : -1);
}

function setUiSizeScale(value: unknown): UiSizeScale {
  const scale = normalizeUiSizeScale(value);
  const changed = uiSizeScale.value !== scale;
  applyUiSizeScale(scale);
  persistUiSizeScale(scale);
  if (changed) {
    void emitTransportEvent("uiSize.changed", { scale }).catch((error) => {
      console.warn("[界面尺寸] 同步尺寸变化失败", error);
    });
  }
  return scale;
}

export function initUiSizeAppearance() {
  if (initialized) return;
  initialized = true;
  applyUiSizeScale(readStoredUiSizeScale());
  if (typeof window === "undefined") return;
  window.addEventListener("storage", handleStorageEvent);
  window.addEventListener("wheel", handleGlobalUiSizeWheel, { passive: false, capture: true });
  eventUnlisten = onTransportNotification<UiSizePayload>("uiSize.changed", (payload) => {
    applyUiSizeScale(payload?.scale ?? payload?.preset);
  });
}

export function useUiSizeAppearance() {
  initUiSizeAppearance();
  return { uiSizeScale, setUiSizeScale, stepUiSizeScale };
}
