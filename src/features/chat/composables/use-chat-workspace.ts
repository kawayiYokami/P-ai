import { computed, ref, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatShellWorkspaceState, ShellWorkspace } from "../../../types/app";
import {
  defaultWorkspaceNameFromPath,
  inferWorkspaceName,
  isLegacyGenericWorkspaceName,
  normalizeWorkspaceLevel,
  workspaceLevelRank,
} from "../../../utils/shell-workspaces";

export type ChatWorkspaceChoice = {
  id: string;
  name: string;
  path: string;
  level: ShellWorkspace["level"];
  access: ShellWorkspace["access"];
};

type UseChatWorkspaceOptions = {
  activeApiConfigId: ComputedRef<string>;
  activeAgentId: ComputedRef<string>;
  activeConversationId: ComputedRef<string>;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

export function useChatWorkspace(options: UseChatWorkspaceOptions) {
  const { t } = useI18n();
  const DEFAULT_CHAT_WORKSPACE_NAME = t("chat.defaultWorkspace");
  const chatWorkspaceName = ref(DEFAULT_CHAT_WORKSPACE_NAME);
  const chatWorkspacePath = ref("");
  const chatWorkspaceRootPath = ref("");
  const chatWorkspacePickerOpen = ref(false);
  const chatWorkspaceItems = ref<ShellWorkspace[]>([]);
  const chatWorkspaceAutonomousMode = ref(false);

  function normalizeWorkspaceChoice(item: ShellWorkspace, index: number): ChatWorkspaceChoice {
    const path = String(item.path || "").trim();
    const level = normalizeWorkspaceLevel(String(item.level || "").trim().toLowerCase());
    const rawName = String(item.name || "").trim();
    const name = isLegacyGenericWorkspaceName(level, rawName)
      ? inferWorkspaceName(level, path, index)
      : (rawName || defaultWorkspaceNameFromPath(path) || DEFAULT_CHAT_WORKSPACE_NAME);
    return {
      id: String(item.id || "").trim(),
      name,
      path,
      level,
      access: item.access,
    };
  }

  function findWorkspaceChoiceByPath(path: string): ChatWorkspaceChoice | null {
    const target = String(path || "").trim().toLowerCase();
    if (!target) return null;
    return chatWorkspaceChoices.value.find((item) => item.path.toLowerCase() === target) ?? null;
  }

  function resolveWorkspaceDisplayName(path: string, workspaceName: string): string {
    const matched = findWorkspaceChoiceByPath(path);
    if (matched) return matched.name;
    const fallback = defaultWorkspaceNameFromPath(path);
    if (fallback) return fallback;
    return String(workspaceName || "").trim() || DEFAULT_CHAT_WORKSPACE_NAME;
  }

  const chatWorkspaceChoices = computed<ChatWorkspaceChoice[]>(() =>
    (chatWorkspaceItems.value || [])
      .map(normalizeWorkspaceChoice)
      .filter((item) => item.id && item.path)
      .sort((left, right) => {
        return workspaceLevelRank(left.level) - workspaceLevelRank(right.level);
      }),
  );

  function applyChatWorkspaceState(state: ChatShellWorkspaceState) {
    const nextPath = String(state.rootPath || "").trim();
    chatWorkspaceRootPath.value = nextPath;
    chatWorkspaceItems.value = Array.isArray(state.workspaces) ? state.workspaces : [];
    chatWorkspaceAutonomousMode.value = Boolean(state.autonomousMode);
    chatWorkspaceName.value = resolveWorkspaceDisplayName(nextPath, String(state.workspaceName || "").trim());
    chatWorkspacePath.value = nextPath;
  }

  function applyChatWorkspaceDraft(workspaces: ChatWorkspaceChoice[]) {
    chatWorkspaceItems.value = workspaces.map((item) => ({
      id: item.id,
      name: item.name,
      path: item.path,
      level: item.level,
      access: item.access,
      builtIn: item.level === "system",
    }));
    chatWorkspaceName.value = resolveWorkspaceDisplayName(
      chatWorkspacePath.value,
      chatWorkspaceName.value,
    );
  }

  async function refreshChatWorkspaceState() {
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!apiConfigId || !agentId) {
      chatWorkspaceName.value = DEFAULT_CHAT_WORKSPACE_NAME;
      chatWorkspacePath.value = "";
      chatWorkspaceRootPath.value = "";
      chatWorkspaceItems.value = [];
      chatWorkspaceAutonomousMode.value = false;
      return;
    }
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("get_chat_shell_workspace", {
        input: { apiConfigId, agentId, conversationId: conversationId || null },
      });
      applyChatWorkspaceState(state);
    } catch (error) {
      console.warn("[SHELL] refresh chat workspace failed:", error);
    }
  }

  function openChatWorkspacePicker() {
    chatWorkspacePickerOpen.value = true;
  }

  function closeChatWorkspacePicker() {
    chatWorkspacePickerOpen.value = false;
  }

  async function saveChatWorkspaces(workspaces: ChatWorkspaceChoice[], autonomousMode?: boolean) {
    const apiConfigId = String(options.activeApiConfigId.value || "").trim();
    const agentId = String(options.activeAgentId.value || "").trim();
    const conversationId = String(options.activeConversationId.value || "").trim();
    if (!apiConfigId || !agentId) return;
    const previousItems = [...chatWorkspaceItems.value];
    const previousName = chatWorkspaceName.value;
    const previousAutonomousMode = chatWorkspaceAutonomousMode.value;
    applyChatWorkspaceDraft(workspaces);
    chatWorkspaceAutonomousMode.value = Boolean(autonomousMode);
    try {
      const state = await invokeTauri<ChatShellWorkspaceState>("update_chat_shell_workspace_layout", {
        input: {
          apiConfigId,
          agentId,
          conversationId: conversationId || null,
          autonomousMode: Boolean(autonomousMode),
          workspaces: workspaces
            .filter((item) => item.level !== "system")
            .map((item) => ({
              id: item.id,
              name: item.name,
              path: item.path,
              level: item.level,
              access: item.access,
              builtIn: false,
            })),
        },
      });
      applyChatWorkspaceState(state);
    } catch (error) {
      chatWorkspaceItems.value = previousItems;
      chatWorkspaceName.value = previousName;
      chatWorkspaceAutonomousMode.value = previousAutonomousMode;
      options.setStatusError("status.requestFailed", error);
      throw error;
    }
  }

  return {
    chatWorkspaceName,
    chatWorkspacePath,
    chatWorkspaceRootPath,
    chatWorkspacePickerOpen,
    chatWorkspaceChoices,
    chatWorkspaceAutonomousMode,
    refreshChatWorkspaceState,
    openChatWorkspacePicker,
    closeChatWorkspacePicker,
    saveChatWorkspaces,
  };
}
