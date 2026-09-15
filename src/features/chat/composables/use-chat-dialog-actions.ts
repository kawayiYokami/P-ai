import { invokeTauri, openTransportWindow } from "../../../services/tauri-api";
import type { Ref } from "vue";

type UseChatDialogActionsOptions = {
  activeChatApiConfigId: Ref<string>;
  assistantAgentId: Ref<string>;
  openPromptPreviewDialog: (apiConfigId: string, agentId: string) => Promise<void>;
  openSystemPromptPreviewDialog: (apiConfigId: string, agentId: string) => Promise<void>;
};

const ARCHIVE_FOCUS_REQUEST_STORAGE_KEY = "easy_call.archives.focus_request.v1";

export function useChatDialogActions(options: UseChatDialogActionsOptions) {
  async function openConversationList() {
    console.info("[CHAT] openConversationList 开始: 打开归档窗口");
    try {
      await openTransportWindow("archives");
    } catch (error) {
      const err = error as { message?: unknown; stack?: unknown };
      console.error("[CHAT] openConversationList 失败: 打开归档窗口", {
        message: String(err?.message ?? error ?? ""),
        stack: String(err?.stack ?? ""),
        action: "openTransportWindow:archives",
      });
    }
  }

  async function openConversationSummary(conversationId: string) {
    const normalizedConversationId = String(conversationId || "").trim();
    if (!normalizedConversationId) {
      await openConversationList();
      return;
    }
    try {
      if (typeof window !== "undefined") {
        window.localStorage.setItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY, JSON.stringify({
          conversationId: normalizedConversationId,
          viewMode: "current",
          createdAt: Date.now(),
        }));
      }
      await invokeTauri("conversation.setActive", {
        input: {
          conversationId: normalizedConversationId,
        },
      });
    } catch (error) {
      console.warn("[CHAT] openConversationSummary 预设目标会话失败", {
        conversationId: normalizedConversationId,
        error,
      });
    }
    await openConversationList();
  }

  async function openPromptPreview() {
    if (!options.activeChatApiConfigId.value || !options.assistantAgentId.value) return;
    await options.openPromptPreviewDialog(options.activeChatApiConfigId.value, options.assistantAgentId.value);
  }

  async function openSystemPromptPreview() {
    if (!options.activeChatApiConfigId.value || !options.assistantAgentId.value) return;
    await options.openSystemPromptPreviewDialog(options.activeChatApiConfigId.value, options.assistantAgentId.value);
  }

  return {
    openConversationList,
    openConversationSummary,
    openPromptPreview,
    openSystemPromptPreview,
  };
}

