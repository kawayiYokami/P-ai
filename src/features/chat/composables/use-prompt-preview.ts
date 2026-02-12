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
  const promptPreviewMode = ref<"full" | "system">("full");

  function resetPromptPreviewState(mode: "full" | "system") {
    promptPreviewMode.value = mode;
    promptPreviewLoading.value = true;
    promptPreviewText.value = "";
    promptPreviewLatestUserText.value = "";
    promptPreviewLatestImages.value = 0;
    promptPreviewLatestAudios.value = 0;
    promptPreviewDialog.value?.showModal();
  }

  async function openPromptPreview(apiConfigId: string, agentId: string) {
    if (!apiConfigId || !agentId) return;
    resetPromptPreviewState("full");
    try {
      await options.beforePreview();
      const preview = await invokeTauri<PromptPreviewResult>("get_prompt_preview", {
        input: { apiConfigId, agentId },
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
    resetPromptPreviewState("system");
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
    openPromptPreview,
    openSystemPromptPreview,
    closePromptPreview,
  };
}


