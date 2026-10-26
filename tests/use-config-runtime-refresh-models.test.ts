import { computed, ref } from "vue";
import { describe, expect, it, vi } from "vitest";
import type { ApiConfigItem, ApiProviderConfigItem, AppConfig } from "../src/types/app";

const invokeTauriMock = vi.fn();

vi.mock("../src/services/tauri-api", () => ({
  invokeTauri: (...args: unknown[]) => invokeTauriMock(...args),
  syncTransportTrayIcon: vi.fn(() => Promise.resolve()),
}));

import { useConfigRuntime } from "../src/features/config/composables/use-config-runtime";

function createProvider(overrides: Partial<ApiProviderConfigItem> = {}): ApiProviderConfigItem {
  return {
    id: "provider-empty-models",
    name: "删光模型的供应商",
    deprecated: false,
    requestFormat: "openai",
    enableText: true,
    enableImage: false,
    enableAudio: false,
    enableTools: true,
    tools: [],
    baseUrl: "https://api.example.com/v1",
    apiKeys: ["sk-test-key"],
    cachedModelOptions: [],
    models: [],
    failureRetryCount: 5,
    ...overrides,
  };
}

function setup(options: {
  providers: ApiProviderConfigItem[];
  selectedApiConfigId: string;
}) {
  const providers = ref<ApiProviderConfigItem[]>(options.providers);
  const config = {
    apiProviders: providers.value,
    selectedApiConfigId: options.selectedApiConfigId,
  } as unknown as AppConfig;

  const selectedApiConfig = computed<ApiConfigItem | null>(() => null);
  // 与生产一致：selectedApiProvider 只由 selectedApiConfigId 拆出 providerId 再匹配，
  // 供应商删光模型后 selectedApiConfigId 为空，这里必然解析为 null。
  const selectedApiProvider = computed<ApiProviderConfigItem | null>(() => {
    const [providerId] = String(config.selectedApiConfigId || "").split("::");
    return providers.value.find((provider) => provider.id === providerId) ?? null;
  });

  const statuses: Array<{ text: string; tone?: string }> = [];
  const runtime = useConfigRuntime({
    t: (key: string) => key,
    setStatus: (text: string, tone?: "default" | "error" | "success") => {
      statuses.push({ text, tone });
    },
    setStatusError: () => undefined,
    personas: ref([]),
    assistantAgentId: ref("default-agent"),
    toolAgentId: ref("default-agent"),
    avatarSaving: ref(false),
    avatarError: ref(""),
    selectedApiConfig,
    selectedApiProvider,
    refreshingModels: ref(false),
    modelRefreshError: ref(""),
    apiModelOptions: ref({}),
    modelRefreshOkFlags: ref({}),
    toolApiConfig: computed<ApiConfigItem | null>(() => null),
    checkingToolsStatus: ref(false),
    toolStatuses: ref([]),
    ensureAvatarCached: () => Promise.resolve(),
  });

  return { runtime, providers, statuses };
}

describe("refreshModels 的目标供应商", () => {
  it("供应商删光模型、selectedApiConfigId 为空时，仍能按显式传入的供应商刷新", async () => {
    const provider = createProvider();
    const { runtime, statuses } = setup({
      providers: [provider],
      selectedApiConfigId: "",
    });
    invokeTauriMock.mockReset();
    invokeTauriMock.mockResolvedValue(["gpt-4.1", "gpt-4.1-mini"]);

    await runtime.refreshModels(provider);

    expect(invokeTauriMock).toHaveBeenCalledTimes(1);
    expect(invokeTauriMock).toHaveBeenCalledWith("refresh_models", {
      input: expect.objectContaining({
        providerId: "provider-empty-models",
        baseUrl: "https://api.example.com/v1",
        apiKey: "sk-test-key",
        requestFormat: "openai",
      }),
    });
    expect(provider.cachedModelOptions).toEqual(["gpt-4.1", "gpt-4.1-mini"]);
    expect(statuses.at(-1)?.tone).toBe("success");
  });

  it("不传供应商时退回当前选中端点所属供应商", async () => {
    const provider = createProvider({ id: "provider-a", models: [] });
    const { runtime, statuses } = setup({
      providers: [provider],
      selectedApiConfigId: "provider-a::model-1",
    });
    invokeTauriMock.mockReset();
    invokeTauriMock.mockResolvedValue(["m-1"]);

    await runtime.refreshModels();

    expect(invokeTauriMock).toHaveBeenCalledTimes(1);
    expect(provider.cachedModelOptions).toEqual(["m-1"]);
    expect(statuses.at(-1)?.tone).toBe("success");
  });

  it("既没有显式供应商、也没有当前选中端点时提示未选中并跳过请求", async () => {
    const { runtime, statuses } = setup({
      providers: [createProvider()],
      selectedApiConfigId: "",
    });
    invokeTauriMock.mockReset();

    await runtime.refreshModels();

    expect(invokeTauriMock).not.toHaveBeenCalled();
    expect(statuses.at(-1)?.text).toContain("当前未选中 API 供应商");
  });
});
