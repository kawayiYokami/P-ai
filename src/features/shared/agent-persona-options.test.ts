import { describe, expect, it } from "vitest";
import type { ApiConfigItem, PersonaProfile } from "../../types/app";
import { buildAgentPersonaOptions } from "./agent-persona-options";

const personas = [
  { id: "alice", name: "爱丽丝", isBuiltInUser: false, apiConfigIds: ["role:expert"] },
  { id: "bob", name: "鲍勃", isBuiltInUser: false, apiConfigIds: ["model-ops"] },
] as PersonaProfile[];

const apiConfigs = [
  { id: "model-ops", name: "运维模型", displayName: "ops-1", model: "ops-1", enableText: true },
] as ApiConfigItem[];

describe("buildAgentPersonaOptions", () => {
  it("maps each persona to one option", () => {
    const options = buildAgentPersonaOptions({
      personas,
      apiConfigs,
      expertApiConfigId: "model-gone",
    });
    expect(options).toHaveLength(2);
    const alice = options.find((option) => option.agentId === "alice");
    expect(alice).toBeDefined();
    expect(alice?.modelName).toBeUndefined();
    const bob = options.find((option) => option.agentId === "bob");
    expect(bob?.modelName).toBe("ops-1");
  });

  it("exposes the persona direct child agents as childAgentIds", () => {
    const withChildren = [
      { id: "lead", name: "组长", isBuiltInUser: false, apiConfigIds: ["model-ops"], childAgentIds: ["alice", ""] },
    ] as PersonaProfile[];
    const options = buildAgentPersonaOptions({ personas: withChildren, apiConfigs });
    expect(options).toHaveLength(1);
    expect(options[0].childAgentIds).toEqual(["alice"]);
  });

  it("skips the built-in user persona", () => {
    const withUser = [
      { id: "user-persona", name: "用户", isBuiltInUser: true },
      ...personas,
    ] as PersonaProfile[];
    const options = buildAgentPersonaOptions({ personas: withUser, apiConfigs });
    expect(options.map((option) => option.agentId)).toEqual(["alice", "bob"]);
  });

  it("skips the built-in system personas", () => {
    const withSystem = [
      { id: "internal-system", name: "内部系统人格", isBuiltInSystem: true },
      { id: "system-persona", name: "pai system" },
      ...personas,
    ] as PersonaProfile[];
    const options = buildAgentPersonaOptions({ personas: withSystem, apiConfigs });
    expect(options.map((option) => option.agentId)).toEqual(["alice", "bob"]);
  });
});
