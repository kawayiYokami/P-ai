import { describe, expect, it } from "vitest";
import {
  normalizeChatLeftPanelMode,
  normalizeChatMonitorPanelMode,
  normalizeChatRightPanelMode,
} from "./chat-ui-layout-storage";

describe("normalizeChatLeftPanelMode", () => {
  it("keeps left panel modes and converts the retired task tab", () => {
    expect(normalizeChatLeftPanelMode("local")).toBe("local");
    expect(normalizeChatLeftPanelMode("contact")).toBe("contact");
    expect(normalizeChatLeftPanelMode("timer")).toBe("timer");
    expect(normalizeChatLeftPanelMode("task")).toBe("timer");
    expect(normalizeChatLeftPanelMode("unknown")).toBe("local");
  });
});

describe("normalizeChatRightPanelMode", () => {
  it("preserves top-level panel modes", () => {
    expect(normalizeChatRightPanelMode("home")).toBe("home");
    expect(normalizeChatRightPanelMode("sideChat")).toBe("sideChat");
    expect(normalizeChatRightPanelMode("monitor")).toBe("monitor");
  });

  it("migrates legacy monitor tabs to the monitor panel", () => {
    expect(normalizeChatRightPanelMode("delegate")).toBe("monitor");
    expect(normalizeChatRightPanelMode("review")).toBe("monitor");
  });
});

describe("normalizeChatMonitorPanelMode", () => {
  it("keeps monitor tabs independent from the top-level panel", () => {
    expect(normalizeChatMonitorPanelMode("timers")).toBe("timers");
    expect(normalizeChatMonitorPanelMode("review")).toBe("tools");
  });

  it("migrates removed tabs to delegate and falls back to delegate", () => {
    expect(normalizeChatMonitorPanelMode("overview")).toBe("delegate");
    expect(normalizeChatMonitorPanelMode("backgroundShells")).toBe("delegate");
    expect(normalizeChatMonitorPanelMode("unknown")).toBe("delegate");
  });
});
