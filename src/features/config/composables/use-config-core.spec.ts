import { computed } from "vue";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../types/app";
import { createImageGenerationProvider, imageGenerationEndpointId } from "../utils/image-generation-config";
import { useConfigCore } from "./use-config-core";

function createConfig(): AppConfig {
  const imageProvider = createImageGenerationProvider("openai", "test");
  // 生图与生视频各自引用一个端点，用来验证两个字段都会被带出去。
  imageProvider.models.push({
    ...imageProvider.models[0],
    id: "video-model",
    name: "video",
    model: "grok-imagine-video",
  });
  return {
    hotkey: "Alt+·",
    uiLanguage: "zh-CN",
    uiFont: "auto",
    codeFont: "auto",
    uiSizeScale: 100,
    webAccessPort: 8429,
    webAccessEnabled: true,
    webAccessPassword: "",
    githubUpdateMethod: "auto",
    skippedGithubUpdateVersion: "",
    recordHotkey: "CapsLock",
    recordBackgroundWakeEnabled: false,
    minRecordSeconds: 1,
    maxRecordSeconds: 60,
    llmRoundLogCapacity: 3,
    messageNotificationEnabled: true,
    messageNotificationSoundEnabled: false,
    desktopOperationNoticeEnabled: true,
    desktopOperateEnabled: true,
    selectedApiConfigId: "",
    expertApiConfigId: "",
    visionApiConfigId: undefined,
    imageGenerationModelId: imageGenerationEndpointId(imageProvider.id, imageProvider.models[0]?.id || ""),
    imageToVideoModelId: imageGenerationEndpointId(imageProvider.id, "video-model"),
    toolReviewApiConfigId: undefined,
    sttApiConfigId: undefined,
    sttAutoSend: false,
    terminalShellKind: "auto",
    shellWorkspaces: [],
    mcpServers: [],
    remoteImChannels: [],
    apiProviders: [],
    imageProviders: [imageProvider],
    apiConfigs: [],
  };
}

describe("useConfigCore image generation", () => {
  it("应在保存载荷和脏检查快照中保留独立生图与生视频配置", () => {
    const config = createConfig();
    const core = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    const payload = core.buildConfigPayload();
    const snapshot = JSON.parse(core.buildConfigSnapshotJson()) as AppConfig;

    expect(payload.imageProviders).toHaveLength(1);
    expect(payload.imageGenerationModelId).toBe(config.imageGenerationModelId);
    expect(payload.imageToVideoModelId).toBe(config.imageToVideoModelId);
    expect(snapshot.imageProviders[0]?.models[0]?.model).toBe("gpt-image-2");
    expect(snapshot.imageGenerationModelId).toBe(config.imageGenerationModelId);
    expect(snapshot.imageToVideoModelId).toBe(config.imageToVideoModelId);
  });
});

describe("useConfigCore search engines", () => {
  it("密钥留空时保存载荷不下发 apiKey，掩码原样带回", () => {
    const config = createConfig();
    config.searchEngines = [
      { id: "exa", enabled: true, apiKey: "sk***ab" },
      { id: "tavily", enabled: true },
    ];
    config.searchMode = { mode: "fixed", engineId: "exa" };
    const core = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    const payload = core.buildConfigPayload() as AppConfig & {
      searchEngines: Array<{ id: string; enabled: boolean; apiKey?: string }>;
      searchMode: { mode: string; engineId?: string };
    };

    expect(payload.searchEngines).toEqual([
      { id: "exa", enabled: true, apiKey: "sk***ab" },
      { id: "tavily", enabled: true },
    ]);
    expect(payload.searchMode).toEqual({ mode: "fixed", engineId: "exa" });
  });

  it("模式缺省时保存载荷写出自动模式", () => {
    const config = createConfig();
    const core = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    const payload = core.buildConfigPayload() as unknown as { searchMode: { mode: string } };
    expect(payload.searchMode).toEqual({ mode: "auto" });
  });
});

describe("useConfigCore api provider displayName", () => {
  it("应在保存载荷与脏检查快照中保留模型显示名", () => {
    const config = createConfig();
    const core = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    const provider = core.createApiProvider("display-name-seed", "gpt-4o-mini");
    provider.models[0].displayName = "我的显示名";
    config.apiProviders = [provider];

    const payload = core.buildConfigPayload();
    const snapshot = JSON.parse(core.buildConfigSnapshotJson()) as AppConfig;

    expect(payload.apiProviders[0]?.models[0]?.displayName).toBe("我的显示名");
    expect(snapshot.apiProviders[0]?.models[0]?.displayName).toBe("我的显示名");
  });

  it("selectedApiConfigId 缺失时保存载荷仍写出空字符串", () => {
    const config = createConfig();
    (config as { selectedApiConfigId?: string }).selectedApiConfigId = undefined;
    const core = useConfigCore({
      config,
      textCapableApiConfigs: computed(() => []),
    });

    expect(core.buildConfigPayload().selectedApiConfigId).toBe("");
  });
});
