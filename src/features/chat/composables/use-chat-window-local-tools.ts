import { i18n } from "../../../i18n";
import {
  captureTransportDesktop,
  captureTransportFocusedWindow,
  invokeTauri,
} from "../../../services/tauri-api";
import {
  attachmentPreviewBase64,
  base64AttachmentFile,
  ingestAttachment,
} from "../../../services/attachment-transfer";
import type { AppConfig } from "../../../types/app";
import type { Ref } from "vue";
import { isAbsoluteLocalPath } from "../utils/local-link";

const t = i18n.global.t;

type ChatWindowLocalToolsBindings = Record<string, any> & {
  t: (key: string, params?: Record<string, unknown>) => string;
  status: Ref<string>;
  setStatus: (value: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  personaEditorId: Ref<string>;
  personaDirty: Ref<boolean>;
  selectedPersonaEditor: Ref<{ name?: string } | null>;
  assistantAgentId: Ref<string>;
  currentForegroundAgentId: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  selectedPdfReadMode: Ref<"text" | "image">;
  backgroundVoiceScreenshotKeywords: Ref<string>;
  backgroundVoiceScreenshotMode: Ref<"desktop" | "focused_window">;
  instructionPresets: Ref<any[]>;
  currentForegroundApiConfig: Ref<any>;
  config: AppConfig;
  applyAgentPrimaryApiConfigLocally: (agent: any, apiConfigId: string) => boolean;
};

export function useChatWindowLocalTools(bindings: ChatWindowLocalToolsBindings) {
  function updatePersonaEditorIdWithNotice(value: string) {
    const nextId = String(value || "").trim();
    if (!nextId || nextId === bindings.personaEditorId.value) return;
    if (bindings.personaDirty.value) {
      const currentName = String(bindings.selectedPersonaEditor.value?.name || bindings.personaEditorId.value || "").trim()
        || bindings.t("config.persona.title");
      bindings.status.value = bindings.t("status.personaUnsavedSwitchHint", { name: currentName });
    }
    bindings.personaEditorId.value = nextId;
  }

  function updateAssistantAgentId(value: string) {
    bindings.assistantAgentId.value = value;
  }

  async function updateForegroundAgentPrimaryApiConfig(value: string) {
    const nextId = String(value || "").trim();
    if (!nextId) return;
    if (!bindings.config.apiConfigs.some((item: any) => String(item.id || "").trim() === nextId)) {
      console.warn("[聊天模型] 选择的模型不存在，忽略更新", { nextId });
      return;
    }
    const currentAgentId = String(bindings.currentForegroundAgentId.value || "").trim();
    const currentAgent = bindings.personas.value.find(
      (item: any) => String(item.id || "").trim() === currentAgentId,
    );
    if (!currentAgent) {
      console.warn("[聊天模型] 当前前台人格不存在，忽略更新", { currentAgentId, nextId });
      return;
    }
    const previousAgent = {
      apiConfigId: String(currentAgent.apiConfigId || "").trim(),
      apiConfigIds: [...(currentAgent.apiConfigIds || [])],
      updatedAt: String(currentAgent.updatedAt || ""),
    };
    const previousSelectedApiConfigId = String(bindings.config.selectedApiConfigId || "").trim();
    const changed = bindings.applyAgentPrimaryApiConfigLocally(currentAgent, nextId);
    if (!changed) return;
    try {
      await invokeTauri("set_agent_primary_api_config", {
        input: {
          agentId: currentAgentId,
          apiConfigId: nextId,
        },
      });
    } catch (error) {
      currentAgent.apiConfigId = previousAgent.apiConfigId;
      currentAgent.apiConfigIds = previousAgent.apiConfigIds;
      currentAgent.updatedAt = previousAgent.updatedAt;
      bindings.config.selectedApiConfigId = previousSelectedApiConfigId;
      bindings.setStatusError("status.saveConfigFailed", error);
    }
  }

  function updateSelectedResponseStyleId(value: string) {
    bindings.selectedResponseStyleId.value = value;
  }

  function updateSelectedPdfReadMode(value: "text" | "image") {
    bindings.selectedPdfReadMode.value = value;
  }

  function updateBackgroundVoiceScreenshotKeywords(value: string) {
    bindings.backgroundVoiceScreenshotKeywords.value = String(value || "").replace(/，/g, ",");
  }

  function updateBackgroundVoiceScreenshotMode(value: "desktop" | "focused_window") {
    bindings.backgroundVoiceScreenshotMode.value = value;
  }

  function updateInstructionPresets(value: any[]) {
    bindings.instructionPresets.value = Array.isArray(value)
      ? value
          .map((item) => ({
            id: String(item?.id || "").trim(),
            name: String(item?.prompt || item?.name || "").trim(),
            prompt: String(item?.prompt || item?.name || "").trim(),
          }))
          .filter((item) => !!item.id && !!item.prompt)
      : [];
  }

  function parseBackgroundVoiceScreenshotKeywords(raw: string): string[] {
    return Array.from(
      new Set(
        String(raw || "")
          .split(/[,\n;，；]+/)
          .map((item) => item.trim())
          .filter(Boolean),
      ),
    );
  }

  function matchBackgroundVoiceScreenshotKeyword(text: string, keywords: string[]): string | null {
    const normalize = (value: string) => String(value || "").replace(/\s+/g, "").toLocaleLowerCase();
    const target = normalize(text);
    if (!target || keywords.length === 0) return null;
    for (const keyword of keywords) {
      const normalized = normalize(keyword);
      if (!normalized) continue;
      if (target.includes(normalized)) {
        return keyword;
      }
    }
    return null;
  }

  async function queueAutoScreenshotFromVoice(input: {
    source: "local" | "remote";
    keyword: string;
    mode: "desktop" | "focused_window";
    startedAt: number;
  }) {
    const apiConfig = bindings.currentForegroundApiConfig.value;
    if (!apiConfig) {
      console.warn("[后台语音截图] 跳过：当前无可用对话模型配置");
      return;
    }
    const screenshotModeLabel = input.mode === "focused_window" ? t('chat.localTools.screenshotModeFocused') : t('chat.localTools.screenshotModeFullscreen');
    try {
      let imageMime = "";
      let imageBase64 = "";
      if (input.mode === "focused_window") {
        const output = await captureTransportFocusedWindow<{
          data?: { imageMime?: string; imageBase64?: string };
        }>();
        imageMime = String(output?.data?.imageMime || "").trim();
        imageBase64 = String(output?.data?.imageBase64 || "").trim();
      } else {
        const output = await captureTransportDesktop<{ imageMime?: string; imageBase64?: string }>();
        imageMime = String(output?.imageMime || "").trim();
        imageBase64 = String(output?.imageBase64 || "").trim();
      }
      if (!imageBase64) {
        throw new Error(t('chat.localTools.screenshotResultEmpty'));
      }
      const queued = await ingestAttachment({
        kind: "browser-file",
        file: base64AttachmentFile(
          `voice-auto-${Date.now()}.webp`,
          imageBase64,
          imageMime || "image/webp",
        ),
      });
      const mime = String(queued.mime || "").trim().toLowerCase();
      const imageSupported = !!apiConfig.enableImage || bindings.hasVisionFallback.value;
      const canSendAsImage =
        !!queued.attachAsMedia
        && mime.startsWith("image/")
        && imageSupported;
      if (canSendAsImage) {
        const previewImage = {
          mime,
          bytesBase64: attachmentPreviewBase64(queued),
          savedPath: String(queued.path || "").trim() || undefined,
          previewDataUrl: String(queued.previewDataUrl || "").trim() || undefined,
        };
        bindings.clipboardImages.value.push(previewImage);
      } else {
        const path = String(queued.path || "").trim().replace(/\\/g, "/");
        if (!isAbsoluteLocalPath(path)) {
          throw new Error("截图附件保存未返回绝对路径，已跳过本次截图附件。");
        }
        const fileName = String(queued.fileName || "").trim() || path.split("/").pop() || "attachment";
        const id = `${path}::${mime}`;
        if (!bindings.queuedAttachmentNotices.value.some((item: any) => item.id === id)) {
          bindings.queuedAttachmentNotices.value.push({
            id,
            fileName,
            path,
            mime,
          });
        }
      }
      const elapsedMs = Date.now() - input.startedAt;
      console.info(
        "[后台语音截图] 完成：命中关键词=%s，模式=%s，来源=%s，耗时=%dms",
        input.keyword,
        screenshotModeLabel,
        input.source,
        elapsedMs,
      );
    } catch (error) {
      const elapsedMs = Date.now() - input.startedAt;
      console.error(
        "[后台语音截图] 失败：命中关键词=%s，模式=%s，来源=%s，耗时=%dms，原因=%s",
        input.keyword,
        screenshotModeLabel,
        input.source,
        elapsedMs,
        String(error),
      );
      bindings.setStatus(t('chat.localTools.screenshotFailed', { error: String(error) }));
    }
  }

  return {
    updatePersonaEditorIdWithNotice,
    updateAssistantAgentId,
    updateForegroundAgentPrimaryApiConfig,
    updateSelectedResponseStyleId,
    updateSelectedPdfReadMode,
    updateBackgroundVoiceScreenshotKeywords,
    updateBackgroundVoiceScreenshotMode,
    updateInstructionPresets,
    parseBackgroundVoiceScreenshotKeywords,
    matchBackgroundVoiceScreenshotKeyword,
    queueAutoScreenshotFromVoice,
  };
}
