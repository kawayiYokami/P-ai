import { describe, expect, it } from "vitest";
import { buildPersonasSnapshotJson } from "./use-chat-window-message-helpers";
import type { PersonaProfile } from "../../../types/app";

function makePersona(overrides: Partial<PersonaProfile> = {}): PersonaProfile {
  return {
    id: "agent-1",
    name: "测试人格",
    systemPrompt: "你是测试助手",
    residentSkillNames: [],
    apiConfigIds: [],
    childAgentIds: [],
    tools: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("buildPersonasSnapshotJson", () => {
  it("系统准则开关变化必须让脏检测指纹变化", () => {
    const enabled = buildPersonasSnapshotJson([makePersona({ includeSystemRules: true })]);
    const disabled = buildPersonasSnapshotJson([makePersona({ includeSystemRules: false })]);

    expect(enabled).not.toBe(disabled);
  });

  it("缺省 includeSystemRules 与显式 true 视为同一状态", () => {
    const omitted = buildPersonasSnapshotJson([makePersona()]);
    const explicitTrue = buildPersonasSnapshotJson([makePersona({ includeSystemRules: true })]);

    expect(omitted).toBe(explicitTrue);
  });
});
