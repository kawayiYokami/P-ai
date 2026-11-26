import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("../src/services/tauri-api", () => ({
  emitTransportEvent: vi.fn(() => Promise.resolve()),
  onTransportNotification: vi.fn(() => () => {}),
}));

import {
  normalizeUiSizeScale,
  stepUiSizeScale,
  uiSizeTokensFor,
  useUiSizeAppearance,
  UI_SIZE_DEFAULT_SCALE,
  UI_SIZE_MAX_SCALE,
  UI_SIZE_MIN_SCALE,
} from "../src/features/shell/composables/use-ui-size-appearance";

describe("ui size appearance", () => {
  it("normalizes legacy presets and continuous values within the supported range", () => {
    expect(normalizeUiSizeScale("small")).toBe(75);
    expect(normalizeUiSizeScale("default")).toBe(100);
    expect(normalizeUiSizeScale("large")).toBe(125);
    expect(normalizeUiSizeScale("extraLarge")).toBe(150);
    expect(normalizeUiSizeScale(74)).toBe(75);
    expect(normalizeUiSizeScale(151)).toBe(150);
    expect(normalizeUiSizeScale(112.6)).toBe(125);
    expect(normalizeUiSizeScale(112)).toBe(100);
    expect(normalizeUiSizeScale(null)).toBe(100);
    expect(normalizeUiSizeScale(" ")).toBe(100);
    expect(normalizeUiSizeScale("invalid")).toBe(100);
  });

  it("derives every typography and control token from the selected scale", () => {
    expect(uiSizeTokensFor(75)).toMatchObject({
      textMicro: "10px",
      textCaption: "11px",
      textXs: "12px",
      textSm: "13px",
      textBase: "14px",
      text2Xl: "22px",
      sizeField: "4px",
      border: "1px",
    });
    expect(uiSizeTokensFor(125)).toMatchObject({
      textMicro: "12px",
      textCaption: "13px",
      textXs: "14px",
      textSm: "16px",
      textBase: "18px",
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
      border: "1px",
    });
  });

  it("steps ui size preset by preset and clamps within the supported range", () => {
    const { setUiSizeScale, uiSizeScale } = useUiSizeAppearance();
    setUiSizeScale(UI_SIZE_DEFAULT_SCALE);
    expect(stepUiSizeScale(1)).toBe(125);
    expect(uiSizeScale.value).toBe(125);
    expect(stepUiSizeScale(-1)).toBe(100);
    setUiSizeScale(UI_SIZE_MIN_SCALE);
    expect(stepUiSizeScale(-1)).toBe(UI_SIZE_MIN_SCALE);
    setUiSizeScale(UI_SIZE_MAX_SCALE);
    expect(stepUiSizeScale(1)).toBe(UI_SIZE_MAX_SCALE);
    setUiSizeScale(145);
    expect(stepUiSizeScale(1)).toBe(UI_SIZE_MAX_SCALE);
  });

  it("shows the current preset label when stepping", () => {
    const elements = new Map<string, {
      id: string;
      textContent: string;
      style: { cssText: string; opacity?: string; transform?: string; setProperty: (name: string, value: string) => void };
      getAttribute: (name: string) => string | null;
      setAttribute: (name: string, value: string) => void;
      attributes: Record<string, string>;
    }>();
    const createElement = () => {
      const attributes: Record<string, string> = {};
      return {
        id: "",
        textContent: "",
        style: {
          cssText: "",
          setProperty: () => undefined,
        },
        attributes,
        getAttribute: (name: string) => attributes[name] ?? null,
        setAttribute: (name: string, value: string) => {
          attributes[name] = value;
        },
      };
    };
    const body = {
      appendChild: (el: { id: string }) => {
        if (el.id) elements.set(el.id, el as never);
        return el;
      },
    };
    const previousDocument = (globalThis as { document?: unknown }).document;
    const previousWindow = (globalThis as { window?: unknown }).window;
    const previousLocalStorage = (globalThis as { localStorage?: unknown }).localStorage;
    const store = new Map<string, string>();
    (globalThis as { localStorage?: unknown }).localStorage = {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
    };
    (globalThis as { document?: unknown }).document = {
      getElementById: (id: string) => elements.get(id) ?? null,
      createElement,
      body,
      documentElement: {
        style: {
          setProperty: () => undefined,
        },
      },
    };
    (globalThis as { window?: unknown }).window = {
      localStorage: (globalThis as { localStorage?: unknown }).localStorage,
      setTimeout: (handler: () => void) => {
        // 测试只验证提示立即显示，不模拟自动隐藏。
        void handler;
        return 1;
      },
      clearTimeout: () => undefined,
    };
    try {
      const { setUiSizeScale } = useUiSizeAppearance();
      setUiSizeScale(100);
      stepUiSizeScale(1);
      const hint = elements.get("easy-call-ui-size-hint");
      expect(hint?.textContent).toBe("大");
      expect(hint?.getAttribute("role")).toBe("status");
    } finally {
      if (previousDocument === undefined) {
        delete (globalThis as { document?: unknown }).document;
      } else {
        (globalThis as { document?: unknown }).document = previousDocument;
      }
      if (previousWindow === undefined) {
        delete (globalThis as { window?: unknown }).window;
      } else {
        (globalThis as { window?: unknown }).window = previousWindow;
      }
      if (previousLocalStorage === undefined) {
        delete (globalThis as { localStorage?: unknown }).localStorage;
      } else {
        (globalThis as { localStorage?: unknown }).localStorage = previousLocalStorage;
      }
    }
  });
});
