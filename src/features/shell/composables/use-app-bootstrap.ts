import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type ViewMode = "chat" | "archives" | "config";
type ConversationApiSettingsPayload = {
  chatApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
};
type ChatSettingsPayload = {
  selectedAgentId: string;
  userAlias: string;
  responseStyleId: string;
};
type RecordHotkeyPayload = {
  state: "pressed" | "released";
  background?: boolean;
};

export type TerminalApprovalRequestPayload = {
  requestId: string;
  title: string;
  message: string;
  approvalKind: string;
  sessionId: string;
  cwd?: string;
  command?: string;
  requestedPath?: string;
  reason?: string;
  existingPaths?: string[];
  timeoutMs?: number;
};

type AppBootstrapOptions = {
  setViewMode: (mode: ViewMode) => void;
  initWindowMode: () => ViewMode;
  onThemeChanged: (theme: string) => void;
  onLocaleChanged: (locale: string) => void;
  onRefreshSignal: () => Promise<void>;
  onTerminalApprovalRequested?: (payload: TerminalApprovalRequestPayload) => void;
  onConversationApiUpdated?: (payload: ConversationApiSettingsPayload) => void;
  onChatSettingsUpdated?: (payload: ChatSettingsPayload) => void;
  onRecordHotkey?: (payload: RecordHotkeyPayload) => void;
};

export function useAppBootstrap(options: AppBootstrapOptions) {
  const unlisteners: UnlistenFn[] = [];

  async function mount() {
    const mode = options.initWindowMode();
    options.setViewMode(mode);
    try {
      unlisteners.push(
        await listen<string>("easy-call:theme-changed", (event) => {
          options.onThemeChanged(event.payload);
        }),
      );
      unlisteners.push(
        await listen<string>("easy-call:locale-changed", (event) => {
          options.onLocaleChanged(event.payload);
        }),
      );
      unlisteners.push(
        await listen("easy-call:refresh", async () => {
          await options.onRefreshSignal();
        }),
      );
      unlisteners.push(
        await listen<TerminalApprovalRequestPayload>(
          "easy-call:terminal-approval-request",
          (event) => {
            options.onTerminalApprovalRequested?.(event.payload);
          },
        ),
      );
      unlisteners.push(
        await listen<RecordHotkeyPayload>("easy-call:record-hotkey", (event) => {
          options.onRecordHotkey?.(event.payload);
        }),
      );
      unlisteners.push(
        await listen<ConversationApiSettingsPayload>(
          "easy-call:conversation-api-updated",
          (event) => {
            options.onConversationApiUpdated?.(event.payload);
          },
        ),
      );
      unlisteners.push(
        await listen<ChatSettingsPayload>(
          "easy-call:chat-settings-updated",
          (event) => {
            options.onChatSettingsUpdated?.(event.payload);
          },
        ),
      );
    } catch (error) {
      unmount();
      throw error;
    }
  }

  function unmount() {
    while (unlisteners.length > 0) {
      const fn = unlisteners.pop();
      if (fn) fn();
    }
  }

  return {
    mount,
    unmount,
  };
}
