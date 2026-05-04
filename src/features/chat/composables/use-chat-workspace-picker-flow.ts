import { ref, type Ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { ChatWorkspaceChoice } from "./use-chat-workspace";

type UseChatWorkspacePickerFlowOptions = {
  chatWorkspaceChoices: Ref<ChatWorkspaceChoice[]>;
  chatWorkspaceAutonomousMode: Ref<boolean>;
  openChatWorkspacePickerBase: () => void;
  closeChatWorkspacePickerBase: () => void;
  saveChatWorkspaces: (items: ChatWorkspaceChoice[], autonomousMode?: boolean) => Promise<void>;
  setStatus: (message: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  workspaceAlreadyExistsText: string;
};

export function useChatWorkspacePickerFlow(options: UseChatWorkspacePickerFlowOptions) {
  const chatWorkspaceDraftChoices = ref<ChatWorkspaceChoice[]>([]);
  const chatWorkspaceDraftAutonomousMode = ref(false);
  const chatWorkspacePickerSaving = ref(false);

  function cloneChatWorkspaceChoices(items: ChatWorkspaceChoice[]): ChatWorkspaceChoice[] {
    return (items || []).map((item) => ({
      id: String(item.id || "").trim(),
      name: String(item.name || "").trim(),
      path: String(item.path || "").trim(),
      level: item.level,
      access: item.access,
    }));
  }

  function syncChatWorkspaceDraftFromCurrentState() {
    chatWorkspaceDraftChoices.value = cloneChatWorkspaceChoices(options.chatWorkspaceChoices.value);
    chatWorkspaceDraftAutonomousMode.value = Boolean(options.chatWorkspaceAutonomousMode.value);
  }

  function openChatWorkspacePicker() {
    syncChatWorkspaceDraftFromCurrentState();
    options.openChatWorkspacePickerBase();
  }

  function closeChatWorkspacePicker() {
    if (chatWorkspacePickerSaving.value) return;
    options.closeChatWorkspacePickerBase();
    syncChatWorkspaceDraftFromCurrentState();
  }

  async function addChatWorkspace() {
    try {
      const picked = await open({
        directory: true,
        multiple: false,
      });
      if (!picked || Array.isArray(picked)) return;
      const nextPath = String(picked || "").trim();
      if (!nextPath) return;
      const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
      const existed = draft.some((item) => String(item.path || "").trim().toLowerCase() === nextPath.toLowerCase());
      if (existed) {
        options.setStatus(options.workspaceAlreadyExistsText);
        return;
      }
      const hasMain = draft.some((item) => item.level === "main");
      draft.push({
        id: `conversation-workspace-${Math.random().toString(36).slice(2, 8)}`,
        name: nextPath.replace(/\\/g, "/").replace(/\/+$/, "").split("/").pop() || nextPath,
        path: nextPath,
        level: hasMain ? "secondary" : "main",
        access: hasMain ? "read_only" : "approval",
      });
      chatWorkspaceDraftChoices.value = draft;
    } catch (error) {
      options.setStatusError("status.requestFailed", error);
    }
  }

  async function setChatWorkspaceAsMain(workspaceId: string) {
    const draft: ChatWorkspaceChoice[] = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value).map((item): ChatWorkspaceChoice => {
      if (item.level === "system") return item;
      if (item.id === workspaceId) {
        return { ...item, level: "main", access: item.access || "approval" };
      }
      if (item.level === "main") {
        return { ...item, level: "secondary" };
      }
      return item;
    });
    chatWorkspaceDraftChoices.value = draft;
  }

  function setChatWorkspaceAccess(workspaceId: string, access: ChatWorkspaceChoice["access"]) {
    const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const target = draft.find((item) => item.id === workspaceId);
    if (!target) return;
    if (target.level === "system") return;
    target.access = access;
    chatWorkspaceDraftChoices.value = draft;
  }

  function removeChatWorkspace(workspaceId: string) {
    const current = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
    const removing = current.find((item) => item.id === workspaceId);
    const draft = current.filter((item) => item.id !== workspaceId || item.level === "system");
    if (removing?.level === "main") {
      const promoteTarget = draft.find((item) => item.level === "secondary");
      if (promoteTarget) {
        draft.forEach((item) => {
          if (item.level === "system") return;
          if (item.id === promoteTarget.id) {
            item.level = "main";
          } else if (item.level === "main") {
            item.level = "secondary";
          }
        });
      }
    }
    chatWorkspaceDraftChoices.value = draft;
  }

  function setChatWorkspaceAutonomousMode(enabled: boolean) {
    chatWorkspaceDraftAutonomousMode.value = Boolean(enabled);
  }

  async function saveChatWorkspacePicker() {
    if (chatWorkspacePickerSaving.value) return;
    chatWorkspacePickerSaving.value = true;
    try {
      const draft = cloneChatWorkspaceChoices(chatWorkspaceDraftChoices.value);
      await options.saveChatWorkspaces(draft, chatWorkspaceDraftAutonomousMode.value);
      options.closeChatWorkspacePickerBase();
      syncChatWorkspaceDraftFromCurrentState();
    } finally {
      chatWorkspacePickerSaving.value = false;
    }
  }

  return {
    chatWorkspaceDraftChoices,
    chatWorkspaceDraftAutonomousMode,
    chatWorkspacePickerSaving,
    openChatWorkspacePicker,
    closeChatWorkspacePicker,
    addChatWorkspace,
    setChatWorkspaceAsMain,
    setChatWorkspaceAccess,
    setChatWorkspaceAutonomousMode,
    removeChatWorkspace,
    saveChatWorkspacePicker,
  };
}
