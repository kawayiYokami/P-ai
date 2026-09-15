import { computed, reactive } from "vue";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../src/types/app";
import { useConfigCore } from "../src/features/config/composables/use-config-core";

function createBaseConfig(): AppConfig {
  return {
    hotkey: "Alt+·",
    uiLanguage: "zh-CN",
    uiFont: "auto",
    recordHotkey: "Alt",
    recordBackgroundWakeEnabled: true,
    minRecordSeconds: 1,
    maxRecordSeconds: 60,
    selectedApiConfigId: "",
    expertApiConfigId: "",
    visionApiConfigId: undefined,
    sttApiConfigId: undefined,
    sttAutoSend: false,
    terminalShellKind: "auto",
    shellWorkspaces: [],
    mcpServers: [],
    remoteImChannels: [],
    apiProviders: [],
    apiConfigs: [],
  };
}

describe("useConfigCore", () => {
  it("migrates legacy api configs into provider-model structure", () => {
    const config = reactive<AppConfig>({
      ...createBaseConfig(),
      selectedApiConfigId: "legacy-openai",
      apiConfigs: [{
        id: "legacy-openai",
        name: "Legacy OpenAI",
        requestFormat: "openai",
        enableText: true,
        enableImage: false,
        enableAudio: false,
        enableTools: true,
        tools: [],
        baseUrl: "https://api.openai.com/v1",
        apiKey: "legacy-key",
        model: "gpt-4.1",
        temperature: 0.7,
        customTemperatureEnabled: true,
        contextWindowTokens: 256000,
        customMaxOutputTokensEnabled: true,
        maxOutputTokens: 8192,
      }],
    });

    const { normalizeApiBindingsLocal } = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    normalizeApiBindingsLocal();

    expect(config.apiProviders).toHaveLength(1);
    expect(config.apiProviders[0].apiKeys).toEqual(["legacy-key"]);
    expect(config.apiProviders[0].models).toHaveLength(1);
    expect(config.apiProviders[0].models[0].model).toBe("gpt-4.1");
    expect(config.apiConfigs).toHaveLength(1);
    expect(config.apiConfigs[0].id).toBe("api-provider-legacy-1::api-model-legacy-1");
    expect(config.apiConfigs[0].name).toBe("Legacy OpenAI/gpt-4.1 · 中");
  });

  it("expands one provider into independent endpoint configs per model", () => {
    const config = reactive<AppConfig>({
      ...createBaseConfig(),
      apiProviders: [{
        id: "provider-openai",
        name: "OpenAI Main",
        requestFormat: "openai",
        enableText: true,
        enableImage: false,
        enableAudio: false,
        enableTools: true,
        tools: [],
        baseUrl: "https://api.openai.com/v1",
        apiKeys: ["key-1", "key-2"],
        keyCursor: 0,
        cachedModelOptions: ["gpt-4.1", "gpt-4.1-mini"],
        failureRetryCount: 1,
        models: [
          {
            id: "model-main",
            model: "gpt-4.1",
            enableImage: false,
            enableTools: true,
            temperature: 0.7,
            customTemperatureEnabled: true,
            contextWindowTokens: 256000,
            customMaxOutputTokensEnabled: true,
            maxOutputTokens: 8192,
          },
          {
            id: "model-mini",
            model: "gpt-4.1-mini",
            enableImage: false,
            enableTools: true,
            temperature: 1,
            customTemperatureEnabled: false,
            contextWindowTokens: 128000,
            customMaxOutputTokensEnabled: false,
            maxOutputTokens: 4096,
          },
        ],
      }],
    });

    const { normalizeApiBindingsLocal } = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    normalizeApiBindingsLocal();

    expect(config.apiConfigs.map((item) => item.id)).toEqual([
      "provider-openai::model-main",
      "provider-openai::model-mini",
    ]);
    expect(config.apiConfigs.map((item) => item.name)).toEqual([
      "OpenAI Main/gpt-4.1 · 中",
      "OpenAI Main/gpt-4.1-mini · 中",
    ]);
    expect(config.apiConfigs[0].apiKey).toBe("key-1");
    expect(config.selectedApiConfigId).toBe("provider-openai::model-main");
  });
});
