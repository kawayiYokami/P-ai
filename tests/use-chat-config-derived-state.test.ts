import { reactive } from "vue";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../src/types/app";
import { useChatConfigDerivedState } from "../src/features/chat/composables/use-chat-config-derived-state";

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

describe("useChatConfigDerivedState", () => {
  it("uses explicit model capabilities instead of model name inference for multimodal configs", () => {
    const config = reactive<AppConfig>({
      ...createBaseConfig(),
      visionApiConfigId: "claude-vision",
      apiConfigs: [
        {
          id: "claude-vision",
          name: "Claude Vision",
          requestFormat: "anthropic",
          enableText: true,
          enableImage: true,
          enableAudio: false,
          enableVideo: false,
          enableTools: true,
          tools: [],
          baseUrl: "https://api.anthropic.com",
          apiKey: "test-key",
          model: "claude-3-5-sonnet-latest",
          temperature: 1,
          contextWindowTokens: 200000,
          maxOutputTokens: 8192,
        },
        {
          id: "text-only",
          name: "Text Only",
          requestFormat: "openai",
          enableText: true,
          enableImage: false,
          enableAudio: false,
          enableVideo: false,
          enableTools: true,
          tools: [],
          baseUrl: "https://api.example.com/v1",
          apiKey: "test-key",
          model: "plain-text-model",
          temperature: 1,
          contextWindowTokens: 128000,
          maxOutputTokens: 4096,
        },
      ],
    });

    const { imageCapableApiConfigs, hasVisionFallback } = useChatConfigDerivedState(config);

    expect(imageCapableApiConfigs.value.map((item) => item.id)).toEqual(["claude-vision"]);
    expect(hasVisionFallback.value).toBe(true);
  });
});
