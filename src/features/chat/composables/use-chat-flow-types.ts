import type { Ref } from "vue";
import type { AssistantStreamBlock, ChatIngressPart, ChatMentionTarget, ChatMessage } from "../../../types/app";
import type { TransportChannel } from "../../../services/tauri-api";
import type { AssistantDeltaEvent, ContextUsageUpdatePayload } from "./use-chat-flow-events";
import type { ConversationRuntimeStreamCacheSnapshot } from "./use-chat-flow-stream-cache";

export type FrontendRoundPhase = "idle" | "queued" | "waiting" | "streaming";

export type UseChatFlowOptions = {
  /** 由统一传输适配器提供外部事件订阅；聊天状态机本身负责注册和销毁。 */
  subscribeExternalEvents?: (method: string, handler: (payload: unknown) => void) => () => void;
  chatting: Ref<boolean>;
  submitPending?: Ref<boolean>;
  trimming: Ref<boolean>;
  isConversationBusy?: () => boolean;
  getSession: () => { apiConfigId: string; agentId: string } | null;
  getConversationId?: () => string;
  chatInput: Ref<string>;
  selectedMentions?: Ref<ChatMentionTarget[]>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>;
  queuedAttachmentNotices?: Ref<Array<{ id: string; fileName: string; path: string; mime: string; pending?: boolean }>>;
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  contextUsagePreview?: Ref<ContextUsageUpdatePayload | null>;
  chatErrorText: Ref<string>;
  setConversationChatError?: (conversationId: string, text: string) => void;
  allMessages: Ref<ChatMessage[]>;
  onOwnUserDraftInserted?: (payload: { conversationId: string; messageId: string }) => void;
  onStreamingAssistantBubbleInserted?: () => void;
  t: (key: string, params?: Record<string, unknown>) => string;
  formatRequestFailed: (error: unknown) => string;
  removeBinaryPlaceholders: (text: string) => string;
  invokeSendChatMessage: (input: {
    text: string;
    displayText?: string;
    parts: ChatIngressPart[];
    extraTextBlocks?: string[];
    mentions?: ChatMentionTarget[];
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    traceId: string;
    onDelta: TransportChannel<AssistantDeltaEvent>;
  }) => Promise<{
    accepted: boolean;
    duplicate: boolean;
    eventId: string;
    conversationId: string;
    traceId: string;
    ingress: string;
    userMessageId?: string;
    assistantMessageId?: string;
  }>;
  invokeStopChatMessage?: (input: {
    session: { apiConfigId: string; agentId: string; conversationId?: string };
    partialAssistantText: string;
    partialStreamBlocks: AssistantStreamBlock[];
  }) => Promise<{
    aborted: boolean;
    persisted: boolean;
    conversationId?: string | null;
    assistantText?: string;
    assistantMessage?: ChatMessage;
  }>;
  refreshMessageById?: (input: {
    conversationId: string;
    messageId: string;
  }) => Promise<boolean | void>;
  invokeBindActiveChatViewStream?: (input: {
    bindingId: string;
    conversationId?: string;
    onDelta: TransportChannel<AssistantDeltaEvent>;
  }) => Promise<void>;
  invokeUnbindActiveChatViewStream?: (input: { bindingId: string }) => Promise<void>;
  invokeProbeActiveChatViewStream?: (input: {
    bindingId: string;
    conversationId?: string;
    probeId: string;
  }) => Promise<boolean>;
  coordinateActiveConversationStreamBind?: (input: {
    bindingId: string;
    conversationId: string;
    force: boolean;
    bind: () => Promise<void>;
    unbind: () => Promise<void>;
  }) => Promise<void>;
  onReloadMessages: () => Promise<void>;
  onHistoryFlushed?: (input: {
    conversationId: string;
    messageCount: number;
    pendingMessages: ChatMessage[];
    activateAssistant: boolean;
  }) => Promise<void>;
  onAssistantMessageCompleted?: (input: {
    conversationId: string;
    assistantMessage: ChatMessage;
  }) => Promise<void> | void;
};

export type RoundState =
  | { phase: "idle" }
  | { phase: "queued"; gen: number; messageId: string }
  | { phase: "streaming"; gen: number; messageId: string };

export type PendingTerminalEvent =
  | {
      kind: "completed";
      gen: number;
      activationId?: string;
      requestId?: string;
      result: {
        assistantText: string;
        assistantMessage?: ChatMessage;
        activationId?: string;
        requestId?: string;
      };
    }
  | {
      kind: "failed";
      gen: number;
      error: unknown;
      activationId?: string;
      requestId?: string;
    };

export type DeferredRoundCompletion = {
  gen: number;
  result: {
    assistantText: string;
    assistantMessage?: ChatMessage;
    activationId?: string;
    requestId?: string;
  };
};

export type SendChatOverrides = {
  text?: string;
  displayText?: string;
  extraTextBlocks?: string[];
  suppressInitialReload?: boolean;
};

export type ResumeForegroundRuntimeRoundInput = {
  conversationId?: string | null;
  streamCache?: ConversationRuntimeStreamCacheSnapshot | null;
  statusText?: string;
  reason?: string;
};
