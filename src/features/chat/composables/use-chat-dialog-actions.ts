import type { Ref } from "vue";

type UseChatDialogActionsOptions = {
  activeChatApiConfigId: Ref<string>;
  selectedPersonaId: Ref<string>;
  openCurrentHistoryDialog: (apiConfigId: string, agentId: string) => Promise<void>;
  openPromptPreviewDialog: (apiConfigId: string, agentId: string) => Promise<void>;
  openSystemPromptPreviewDialog: (apiConfigId: string, agentId: string) => Promise<void>;
};

export function useChatDialogActions(options: UseChatDialogActionsOptions) {
  async function openCurrentHistory() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    await options.openCurrentHistoryDialog(options.activeChatApiConfigId.value, options.selectedPersonaId.value);
  }

  async function openPromptPreview() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    await options.openPromptPreviewDialog(options.activeChatApiConfigId.value, options.selectedPersonaId.value);
  }

  async function openSystemPromptPreview() {
    if (!options.activeChatApiConfigId.value || !options.selectedPersonaId.value) return;
    await options.openSystemPromptPreviewDialog(options.activeChatApiConfigId.value, options.selectedPersonaId.value);
  }

  return {
    openCurrentHistory,
    openPromptPreview,
    openSystemPromptPreview,
  };
}

