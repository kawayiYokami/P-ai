import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentWorkSignalPayload, AppConfig } from "../../../types/app";

type ViewMode = "chat" | "archives" | "config";
type ConversationApiSettingsPayload = {
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
};
type ChatSettingsPayload = {
  assistantDepartmentAgentId: string;
  userAlias: string;
  responseStyleId: string;
  pdfReadMode?: "text" | "image";
  backgroundVoiceScreenshotKeywords?: string;
  backgroundVoiceScreenshotMode?: "desktop" | "focused_window";
  instructionPresets?: Array<{ id: string; name: string; prompt: string }>;
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
  onTerminalApprovalRequested?: (payload: TerminalApprovalRequestPayload) => void;
  onConversationApiUpdated?: (payload: ConversationApiSettingsPayload) => void;
  onChatSettingsUpdated?: (payload: ChatSettingsPayload) => void;
  onConfigUpdated?: (payload: AppConfig) => void;
  onAgentWorkStarted?: (payload: AgentWorkSignalPayload) => void;
  onAgentWorkStopped?: (payload: AgentWorkSignalPayload) => void;
  onRecordHotkeyProbe?: (payload: { state: "pressed" | "released"; seq: number }) => void;
};

export function useAppBootstrap(options: AppBootstrapOptions) {
  const unlisteners: UnlistenFn[] = [];

  async function mount() {
    const mode = options.initWindowMode();
    const isChatWindow = mode === "chat";
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
      if (isChatWindow) {
        unlisteners.push(
          await listen<TerminalApprovalRequestPayload>(
            "easy-call:terminal-approval-request",
            (event) => {
              options.onTerminalApprovalRequested?.(event.payload);
            },
          ),
        );
      } else {
        console.info("[BOOTSTRAP] skipping terminal approval listener: not chat window");
      }
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
      unlisteners.push(
        await listen<AppConfig>("easy-call:config-updated", (event) => {
          options.onConfigUpdated?.(event.payload);
        }),
      );
      unlisteners.push(
        await listen<AgentWorkSignalPayload>("easy-call:agent-work-start", (event) => {
          options.onAgentWorkStarted?.(event.payload);
        }),
      );
      unlisteners.push(
        await listen<AgentWorkSignalPayload>("easy-call:agent-work-stop", (event) => {
          options.onAgentWorkStopped?.(event.payload);
        }),
      );
      unlisteners.push(
        await listen<unknown>("easy-call:record-hotkey-probe", (event) => {
          const payload = event.payload as
            | { state?: unknown; seq?: unknown }
            | string
            | null
            | undefined;
          if (typeof payload === "string") {
            const text = payload.trim().toLowerCase();
            if (text === "pressed" || text === "released") {
              options.onRecordHotkeyProbe?.({ state: text, seq: 0 });
            }
            return;
          }
          const text = String(payload?.state || "").trim().toLowerCase();
          if (text !== "pressed" && text !== "released") return;
          const seqRaw = Number(payload?.seq);
          const seq = Number.isFinite(seqRaw) && seqRaw > 0 ? Math.floor(seqRaw) : 0;
          options.onRecordHotkeyProbe?.({ state: text, seq });
        }),
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

