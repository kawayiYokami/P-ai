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
  it("maps each persona to one option and marks a missing primary model as modelMissing", () => {
    const options = buildAgentPersonaOptions({
      personas,
      apiConfigs,
      expertApiConfigId: "model-gone",
    });
    expect(options).toHaveLength(2);
    const alice = options.find((option) => option.agentId === "alice");
    expect(alice).toBeDefined();
    expect(alice?.modelMissing).toBe(true);
    expect(alice?.modelName).toBeUndefined();
    expect(alice?.providerName).toBeUndefined();
    const bob = options.find((option) => option.agentId === "bob");
    expect(bob?.modelMissing).toBe(false);
    expect(bob?.modelName).toBe("ops-1");
  });

  it("keeps persona options when role:quick / role:expert resolves to a missing model", () => {
    const options = buildAgentPersonaOptions({
      personas,
      apiConfigs,
      toolReviewApiConfigId: "model-gone-too",
    });
    const alice = options.find((option) => option.agentId === "alice");
    expect(alice?.modelMissing).toBe(true);
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
});
