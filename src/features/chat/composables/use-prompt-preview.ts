import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import { formatI18nError } from "../../../utils/error";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type PromptPreviewResult = {
  preamble: string;
  latestUserText: string;
  latestImages: number;
  latestAudios: number;
  requestBodyJson: string;
};

export type RequestPreviewMode = "chat" | "compaction" | "archive";

type SystemPromptPreviewResult = {
  systemPrompt: string;
};

type UsePromptPreviewOptions = {
  t: TrFn;
  beforePreview: () => Promise<void>;
};

export function usePromptPreview(options: UsePromptPreviewOptions) {
  const promptPreviewDialog = ref<HTMLDialogElement | null>(null);
  const promptPreviewLoading = ref(false);
  const promptPreviewText = ref("");
  const promptPreviewLatestUserText = ref("");
  const promptPreviewLatestImages = ref(0);
  const promptPreviewLatestAudios = ref(0);
  const promptPreviewMode = ref<RequestPreviewMode | "system" | null>(null);
  const promptPreviewApiConfigId = ref("");
  const promptPreviewAgentId = ref("");

  function resetPromptPreviewState(mode: RequestPreviewMode | "system" | null) {
    promptPreviewMode.value = mode;
    promptPreviewLoading.value = false;
    promptPreviewText.value = "";
    promptPreviewLatestUserText.value = "";
    promptPreviewLatestImages.value = 0;
    promptPreviewLatestAudios.value = 0;
    promptPreviewDialog.value?.showModal();
  }

  async function openPromptPreview(apiConfigId: string, agentId: string) {
    if (!apiConfigId || !agentId) return;
    promptPreviewApiConfigId.value = apiConfigId;
    promptPreviewAgentId.value = agentId;
    resetPromptPreviewState(null);
  }

  async function loadPromptPreview(mode: RequestPreviewMode) {
    if (!promptPreviewApiConfigId.value || !promptPreviewAgentId.value) return;
    promptPreviewMode.value = mode;
    promptPreviewLoading.value = true;
    promptPreviewText.value = "";
    promptPreviewLatestUserText.value = "";
    promptPreviewLatestImages.value = 0;
    promptPreviewLatestAudios.value = 0;
    try {
      await options.beforePreview();
      const preview = await invokeTauri<PromptPreviewResult>("get_prompt_preview", {
        input: { apiConfigId: promptPreviewApiConfigId.value, agentId: promptPreviewAgentId.value },
        previewMode: mode,
      });
      promptPreviewText.value = preview.requestBodyJson || "";
      promptPreviewLatestUserText.value = preview.latestUserText || "";
      promptPreviewLatestImages.value = Number(preview.latestImages || 0);
      promptPreviewLatestAudios.value = Number(preview.latestAudios || 0);
    } catch (e) {
      promptPreviewText.value = formatI18nError(options.t, "status.loadRequestPreviewFailed", e);
    } finally {
      promptPreviewLoading.value = false;
    }
  }

  async function openSystemPromptPreview(apiConfigId: string, agentId: string) {
    if (!apiConfigId || !agentId) return;
    promptPreviewApiConfigId.value = apiConfigId;
    promptPreviewAgentId.value = agentId;
    resetPromptPreviewState("system");
    promptPreviewLoading.value = true;
    try {
      await options.beforePreview();
      const preview = await invokeTauri<SystemPromptPreviewResult>("get_system_prompt_preview", {
        input: { apiConfigId, agentId },
      });
      promptPreviewText.value = preview.systemPrompt || "";
    } catch (e) {
      promptPreviewText.value = formatI18nError(options.t, "status.loadSystemPromptFailed", e);
    } finally {
      promptPreviewLoading.value = false;
    }
  }

  function closePromptPreview() {
    promptPreviewDialog.value?.close();
  }

  return {
    promptPreviewDialog,
    promptPreviewLoading,
    promptPreviewText,
    promptPreviewLatestUserText,
    promptPreviewLatestImages,
    promptPreviewLatestAudios,
    promptPreviewMode,
    loadPromptPreview,
    openPromptPreview,
    openSystemPromptPreview,
    closePromptPreview,
  };
}

