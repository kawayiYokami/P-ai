import { describe, expect, it } from "vitest";
import { shouldApplyAvatarGenerated } from "./use-config-bootstrap-sync";

describe("shouldApplyAvatarGenerated", () => {
  it("lets both windows apply a successful event", () => {
    const payload = { agentId: "p1", ok: true, path: "/a.png" };
    expect(shouldApplyAvatarGenerated(payload, "chat")).toBe(true);
    expect(shouldApplyAvatarGenerated(payload, "config")).toBe(true);
  });

  it("only lets the window that triggered the failure show the card", () => {
    const payload = { agentId: "p1", ok: false, error: "502", origin: "chat" };
    expect(shouldApplyAvatarGenerated(payload, "chat")).toBe(true);
    expect(shouldApplyAvatarGenerated(payload, "config")).toBe(false);

    const saved = { agentId: "p1", ok: false, error: "502", origin: "config" };
    expect(shouldApplyAvatarGenerated(saved, "config")).toBe(true);
    expect(shouldApplyAvatarGenerated(saved, "chat")).toBe(false);
  });

  it("falls back to handling events that carry no origin", () => {
    expect(shouldApplyAvatarGenerated({ agentId: "p1", ok: false }, "chat")).toBe(true);
    expect(shouldApplyAvatarGenerated({ agentId: "p1", ok: false, origin: " " }, "config")).toBe(true);
    expect(shouldApplyAvatarGenerated(null, "chat")).toBe(true);
  });
});
