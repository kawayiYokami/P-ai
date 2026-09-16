import { describe, expect, it } from "vitest";
import { buildPersonasSnapshotJson } from "../src/features/chat/composables/use-chat-window-message-helpers";
import { ensurePermissionControl } from "../src/features/config/utils/persona-capability";
import type { PersonaProfile } from "../src/types/app";

function createPersona(id = "test-agent", name = "Test"): PersonaProfile {
  return {
    id,
    name,
    systemPrompt: "You are a helpful assistant.",
    tools: [],
    createdAt: "2026-09-17T00:00:00Z",
    updatedAt: "2026-09-17T00:00:00Z",
  };
}

describe("buildPersonasSnapshotJson", () => {
  it("detects changes in childAgentIds (delegates)", () => {
    const persona = createPersona();
    const snapshot1 = buildPersonasSnapshotJson([persona]);

    persona.childAgentIds = ["assistant-1"];
    const snapshot2 = buildPersonasSnapshotJson([persona]);

    expect(snapshot1).not.toBe(snapshot2);

    persona.childAgentIds = [];
    const snapshot3 = buildPersonasSnapshotJson([persona]);
    expect(snapshot1).toBe(snapshot3);
  });

  it("detects changes and ordering in residentSkillNames", () => {
    const persona = createPersona();
    const snapshot1 = buildPersonasSnapshotJson([persona]);

    persona.residentSkillNames = ["skill-a", "skill-b"];
    const snapshot2 = buildPersonasSnapshotJson([persona]);
    expect(snapshot1).not.toBe(snapshot2);

    // Order matters for resident skills
    persona.residentSkillNames = ["skill-b", "skill-a"];
    const snapshot3 = buildPersonasSnapshotJson([persona]);
    expect(snapshot2).not.toBe(snapshot3);
  });

  it("detects changes in permissionControl", () => {
    const persona = createPersona();
    const snapshot1 = buildPersonasSnapshotJson([persona]);

    const control = ensurePermissionControl(persona);
    // ensurePermissionControl sets default values: enabled=false, mode='whitelist', empty arrays
    // This should match the normalized empty state and not trigger a false dirty
    const snapshot2 = buildPersonasSnapshotJson([persona]);
    expect(snapshot1).toBe(snapshot2);

    // Switching mode
    control.mode = "blacklist";
    expect(buildPersonasSnapshotJson([persona])).not.toBe(snapshot1);

    // Enabling permission
    control.mode = "whitelist";
    control.enabled = true;
    expect(buildPersonasSnapshotJson([persona])).not.toBe(snapshot1);

    // Changing tools
    control.builtinToolNames = ["read_file"];
    expect(buildPersonasSnapshotJson([persona])).not.toBe(snapshot1);
  });
});
