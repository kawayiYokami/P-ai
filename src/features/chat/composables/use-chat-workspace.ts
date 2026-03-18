import { open } from "@tauri-apps/plugin-dialog";
import { ref, type ComputedRef } from "vue";
import { invokeTauri } from "../../../services/tauri-api";

type ChatShellWorkspaceState = {
  sessionId: string;
  workspaceName: string;
  rootPath: string;
  locked: boolean;
};

type UseChatWorkspaceOptions = {
  activeApiConfigId: ComputedRef<string>;
  activeAgentId: ComputedRef<string>;
  activeConversationId: ComputedRef<string>;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

export function useChatWorkspace(options: UseChatWorkspaceOptions) {
  const chatWorkspaceName = ref("默认工作空间");
  const chatWorkspaceLocked = ref(false);
  const chatWorkspacePath = ref("");

  async function refreshChatWorkspaceState() {
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!apiConfigId || !agentId) {
      chatWorkspaceName.value = "默认工作空间";
      chatWorkspaceLocked.value = false;
      chatWorkspacePath.value = "";
      return;
    }
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("get_chat_shell_workspace", {
        input: { apiConfigId, agentId, conversationId: conversationId || null },
      });
      chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
      chatWorkspaceLocked.value = !!state.locked;
      chatWorkspacePath.value = String(state.rootPath || "").trim();
    } catch (error) {
      console.warn("[SHELL] refresh chat workspace failed:", error);
    }
  }

  async function lockChatWorkspaceFromPicker() {
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!apiConfigId || !agentId) return;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        defaultPath: chatWorkspacePath.value || undefined,
      });
      if (!picked || Array.isArray(picked)) return;
      const state = await invokeTauri<ChatShellWorkspaceState>("lock_chat_shell_workspace", {
        input: {
          apiConfigId,
          agentId,
          conversationId: conversationId || null,
          workspacePath: String(picked),
        },
      });
      chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
      chatWorkspaceLocked.value = !!state.locked;
      chatWorkspacePath.value = String(state.rootPath || "").trim();
      options.setStatus(`工作空间已锁定: ${chatWorkspaceName.value}`);
    } catch (error) {
      options.setStatusError("status.requestFailed", error);
    }
  }

  async function unlockChatWorkspace() {
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!apiConfigId || !agentId) return;
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("unlock_chat_shell_workspace", {
        input: {
          apiConfigId,
          agentId,
          conversationId: conversationId || null,
        },
      });
      chatWorkspaceName.value = String(state.workspaceName || "").trim() || "默认工作空间";
      chatWorkspaceLocked.value = !!state.locked;
      chatWorkspacePath.value = String(state.rootPath || "").trim();
      options.setStatus(`工作空间已解锁: ${chatWorkspaceName.value}`);
    } catch (error) {
      options.setStatusError("status.requestFailed", error);
    }
  }

  return {
    chatWorkspaceName,
    chatWorkspaceLocked,
    chatWorkspacePath,
    refreshChatWorkspaceState,
    lockChatWorkspaceFromPicker,
    unlockChatWorkspace,
  };
}
