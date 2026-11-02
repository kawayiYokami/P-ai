import { ref } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAvatarCache } from "./use-avatar-cache";
import type { PersonaProfile } from "../../../types/app";

vi.mock("../../../services/tauri-api", () => ({
  invokeTauri: vi.fn(),
}));

import { invokeTauri } from "../../../services/tauri-api";

const mockInvoke = vi.mocked(invokeTauri);

const BRAND_DATA_URL = "data:image/png;base64,BRAND";
const OWN_DATA_URL = "data:image/webp;base64,OWN";

function makePersona(overrides: Partial<PersonaProfile> & { id: string }): PersonaProfile {
  return {
    name: overrides.id,
    systemPrompt: "",
    tools: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

type InvokeHandler = (command: string) => Promise<unknown>;

function stubInvoke(handler: InvokeHandler) {
  mockInvoke.mockImplementation(((command: string) => handler(command)) as typeof invokeTauri);
}

function countCalls(command: string) {
  return mockInvoke.mock.calls.filter(([name]) => name === command).length;
}

describe("useAvatarCache 人格头像预加载", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("预加载只读头像文件与品牌图标，不发起任何生成", async () => {
    stubInvoke(async (command) => {
      if (command === "read_avatar_data_url") return { dataUrl: OWN_DATA_URL };
      return { dataUrl: "data:image/svg+xml;base64,GENERATED", path: "avatars/agent-x.svg", updatedAt: "2026-05-01T00:00:00Z" };
    });
    const personas = ref<PersonaProfile[]>([
      makePersona({ id: "with-avatar", avatarPath: "avatars/agent-with.webp", avatarUpdatedAt: "2026-04-01T00:00:00Z" }),
      makePersona({ id: "without-avatar" }),
      makePersona({ id: "user-persona", isBuiltInUser: true }),
    ]);
    const cache = useAvatarCache({ personas });

    await cache.preloadPersonaAvatars();

    // 头像生成只在后端「新建 / 更新 / 保存人格」时发起，界面预加载一律不催。
    expect(countCalls("generate_agent_avatar")).toBe(0);
    expect(cache.resolveAvatarUrl("avatars/agent-with.webp", "2026-04-01T00:00:00Z")).toBe(OWN_DATA_URL);
    expect(cache.resolveBrandAvatarUrl()).toBe(OWN_DATA_URL);
    expect(personas.value[1].avatarPath).toBeUndefined();
  });

  it("无人像人格只回落到品牌图标", async () => {
    stubInvoke(async (command) => {
      if (command === "read_avatar_data_url") return { dataUrl: BRAND_DATA_URL };
      return { dataUrl: "data:image/svg+xml;base64,GENERATED", path: "avatars/agent-y.svg", updatedAt: "2026-05-01T00:00:00Z" };
    });
    const personas = ref<PersonaProfile[]>([makePersona({ id: "lonely" })]);
    const cache = useAvatarCache({ personas });

    await cache.preloadPersonaAvatars();

    expect(cache.resolveAvatarUrl(personas.value[0].avatarPath, personas.value[0].avatarUpdatedAt)).toBe("");
    expect(cache.resolveBrandAvatarUrl()).toBe(BRAND_DATA_URL);
    expect(countCalls("generate_agent_avatar")).toBe(0);
  });
});
