import { ref } from "vue";
import { emitTransportEvent, onTransportNotification } from "../../../services/tauri-api";

const UI_SIZE_STORAGE_KEY = "easy-call.ui-size.v1";
const UI_SIZE_HINT_HIDE_MS = 900;
const UI_SIZE_HINT_ELEMENT_ID = "easy-call-ui-size-hint";

export const UI_SIZE_MIN_SCALE = 75;
export const UI_SIZE_MAX_SCALE = 150;
export const UI_SIZE_DEFAULT_SCALE = 100;
export const UI_SIZE_STEP_SCALE = 10;
export const UI_SIZE_SCALE_MARKS = [75, 100, 125, 150] as const;
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
 * 界面尺寸映射：滑杆上的百分比表示「占屏面积」，线性尺寸取它的平方根。
 * 150% 对应线性 √1.5 ≈ 1.2247 倍、面积 1.5 倍。
 * 字号基准、--size-field、--size-selector、--border 以及根字号全挂在这一个比例上，
 * 在这里开方，全链路一起变成同一个倍数，不会出现字缩了而间距还按旧比例放的错位。
 */
function scaledLinearFactor(scale: UiSizeScale): number {
  return Math.sqrt(scale / 100);
}

function scaledPx(value: number, scale: UiSizeScale): string {
  return `${Math.round(value * scaledLinearFactor(scale) * 100) / 100}px`;
}

/** 边框不吃亚像素：低于 1px 在部分显示器上直接消失，所以取 1px 作下限。 */
function scaledBorderPx(scale: UiSizeScale): string {
  return `${Math.max(scaledLinearFactor(scale), 1).toFixed(2)}px`;
}

export function normalizeUiSizeScale(value: unknown): UiSizeScale {
  if (value == null || (typeof value === "string" && !value.trim())) {
    return UI_SIZE_DEFAULT_SCALE;
  }
  if (typeof value === "string" && value.trim() in LEGACY_UI_SIZE_SCALE_MAP) {
    return LEGACY_UI_SIZE_SCALE_MAP[value.trim()];
  }
  const numeric = Math.round(Number(value));
  if (!Number.isFinite(numeric)) return UI_SIZE_DEFAULT_SCALE;
  return Math.min(UI_SIZE_MAX_SCALE, Math.max(UI_SIZE_MIN_SCALE, numeric));
}

export function uiSizeTokensFor(value: unknown): UiSizeTokens {
  const scale = normalizeUiSizeScale(value);
  return {
    textMicro: scaledPx(9, scale),
    textCaption: scaledPx(11, scale),
    textXs: scaledPx(12, scale),
    textSm: scaledPx(14, scale),
    textBase: scaledPx(16, scale),
    textLg: scaledPx(18, scale),
    textXl: scaledPx(20, scale),
    text2Xl: scaledPx(24, scale),
    markdownHeading1: scaledPx(16.32, scale),
    markdownHeading2: scaledPx(15.68, scale),
    markdownHeading3: scaledPx(15.04, scale),
    markdownHeading4: scaledPx(14.4, scale),
    markdownDocumentHeading1: scaledPx(24, scale),
    markdownDocumentHeading2: scaledPx(20.48, scale),
    markdownDocumentHeading3: scaledPx(17.92, scale),
    markdownDocumentHeading4: scaledPx(16.32, scale),
    sizeField: scaledPx(4.16, scale),
    sizeSelector: scaledPx(4.16, scale),
    border: scaledBorderPx(scale),
  };
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

export function showUiSizeScaleHint(scale: UiSizeScale) {
  if (typeof document === "undefined" || typeof window === "undefined") return;
  const el = ensureUiSizeHintElement();
  if (!el) return;
  el.textContent = `${normalizeUiSizeScale(scale)}%`;
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
  const delta = direction > 0 ? UI_SIZE_STEP_SCALE : -UI_SIZE_STEP_SCALE;
  const next = setUiSizeScale(uiSizeScale.value + delta);
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
