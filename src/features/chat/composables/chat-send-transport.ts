import { invokeTauri, type TransportChannel } from "../../../services/tauri-api";
import type {
  AssistantStreamBlock,
  ChatIngressPart,
  ChatMentionTarget,
  ChatMessage,
} from "../../../types/app";
import type { AssistantDeltaEvent } from "./use-chat-flow-events";

export type ChatSendSession = {
  apiConfigId: string;
  agentId: string;
  conversationId?: string;
};

export type InvokeSendChatMessageInput = {
  text: string;
  displayText?: string;
  parts: ChatIngressPart[];
  extraTextBlocks?: string[];
  mentions?: ChatMentionTarget[];
  session: ChatSendSession;
  traceId: string;
  onDelta: TransportChannel<AssistantDeltaEvent>;
};

export type SendChatMessageResult = {
  accepted: boolean;
  duplicate: boolean;
  eventId: string;
  conversationId: string;
  traceId: string;
  ingress: string;
  userMessageId?: string;
  assistantMessageId?: string;
};

export type InvokeStopChatMessageInput = {
  session: ChatSendSession;
  partialAssistantText: string;
  partialStreamBlocks: AssistantStreamBlock[];
};

export type StopChatMessageResult = {
  aborted: boolean;
  persisted: boolean;
  conversationId?: string | null;
  assistantText?: string;
  assistantMessage?: ChatMessage;
};

/**
 * 聊天发送唯一入口：主聊天与侧边追问共用。
 * 以超集语义为准：支持 extraTextBlocks，mentions 按后端 UserMentionTargetInput 归一化（去 avatarUrl 杂字段）。
 */
export function invokeSendChatMessage({
  text,
  displayText,
  parts,
  extraTextBlocks,
  mentions,
  session,
  traceId,
  onDelta,
}: InvokeSendChatMessageInput): Promise<SendChatMessageResult> {
  return invokeTauri<SendChatMessageResult>("chat.send", {
    input: {
      payload: {
        text,
        displayText,
        parts,
        extraTextBlocks: extraTextBlocks && extraTextBlocks.length > 0 ? extraTextBlocks : undefined,
        mentions: Array.isArray(mentions) && mentions.length > 0
          ? mentions.map((item) => ({
              agentId: item.agentId,
              agentName: item.agentName,
            }))
          : undefined,
      },
      session: {
        apiConfigId: session.apiConfigId,
        agentId: session.agentId,
        conversationId: session.conversationId || null,
      },
      traceId,
    },
    onDelta,
  });
}

/** 聊天停止唯一入口：主聊天与侧边追问共用。 */
export function invokeStopChatMessage({
  session,
  partialAssistantText,
  partialStreamBlocks,
}: InvokeStopChatMessageInput): Promise<StopChatMessageResult> {
  return invokeTauri<StopChatMessageResult>("chat.stop", {
    input: {
      session: {
        apiConfigId: session.apiConfigId,
        agentId: session.agentId,
        conversationId: session.conversationId || null,
      },
      partialAssistantText,
      partialStreamBlocks,
    },
  });
}
