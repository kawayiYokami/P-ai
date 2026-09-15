import type { Ref } from "vue";
import type { ChatMentionTarget } from "../../../types/app";
import type { SendChatOverrides } from "./use-chat-flow-types";
import { normalizeConversationId } from "./use-chat-flow-utils";

type AttachmentNotice = { id: string; fileName: string; path: string; mime: string };
type ImageAttachment = { mime: string; bytesBase64: string; savedPath?: string };
type Session = { apiConfigId: string; agentId: string; conversationId?: string };

type UseChatFlowSendInputOptions = {
  chatInput: Ref<string>;
  clipboardImages: Ref<ImageAttachment[]>;
  queuedAttachmentNotices?: Ref<AttachmentNotice[]>;
  selectedMentions?: Ref<ChatMentionTarget[]>;
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  getSession: () => Session | null;
  getConversationId?: () => string;
  buildQueuedAttachmentPayload: () => Array<{ fileName: string; path: string; mime: string }>;
  buildImageAttachmentPayload: (
    images: ImageAttachment[],
  ) => Array<{ fileName: string; path: string; mime: string }>;
  mergeAttachmentPayloads: (
    primary: Array<{ fileName: string; path: string; mime: string }>,
    fallback?: Array<{ fileName: string; path: string; mime: string }>,
  ) => Array<{ fileName: string; path: string; mime: string }>;
};

export type PreparedChatSendInput = {
  useOverrideMessage: boolean;
  plainText: string;
  displayText: string;
  selectedMentions: ChatMentionTarget[];
  extraTextBlocks: string[];
  sentImages: ImageAttachment[];
  attachments: Array<{ fileName: string; path: string; mime: string }>;
  sendSession: Session;
  sendConversationId: string;
};

export function useChatFlowSendInput(options: UseChatFlowSendInputOptions) {
  function normalizeSelectedMentions(): ChatMentionTarget[] {
    return Array.isArray(options.selectedMentions?.value)
      ? options.selectedMentions.value
        .map((item) => ({
          agentId: String(item.agentId || "").trim(),
          agentName: String(item.agentName || "").trim(),
          avatarUrl: String(item.avatarUrl || "").trim() || undefined,
        }))
        .filter((item) => !!item.agentId)
      : [];
  }

  function buildDisplayText(
    plainText: string,
    sentImages: ImageAttachment[],
    queuedAttachments: Array<{ fileName: string; path: string; mime: string }>,
  ): string {
    if (plainText) return plainText;
    const imageCount = sentImages.length;
    const attachmentNames = queuedAttachments
      .map((item) => String(item.fileName || item.path || "").trim())
      .filter((item, index, array) => !!item && array.indexOf(item) === index)
      .slice(0, 3);
    const attachmentCount = queuedAttachments.length;
    const parts: string[] = [];
    if (imageCount > 0) {
      parts.push(`用户发送了${imageCount}张图片`);
    }
    if (attachmentCount > 0) {
      const suffix = attachmentNames.length > 0 ? `：${attachmentNames.join("、")}` : "";
      parts.push(`用户发送了${attachmentCount}个附件${suffix}`);
    }
    if (parts.length === 0) return plainText;
    return `${parts.join("，")}。请基于这些内容处理。`;
  }

  function prepareSendInput(overrides?: SendChatOverrides): PreparedChatSendInput | null {
    const useOverrideMessage = !!overrides && typeof overrides.text === "string";
    const plainText = useOverrideMessage
      ? String(overrides.text || "").trim()
      : options.chatInput.value.trim();
    const queuedAttachments = useOverrideMessage ? [] : options.buildQueuedAttachmentPayload();
    const selectedMentions = normalizeSelectedMentions();
    const extraTextBlocks = (Array.isArray(overrides?.extraTextBlocks) ? overrides.extraTextBlocks : [])
      .filter((item) => !!String(item || "").trim());
    const sentImages = useOverrideMessage ? [] : [...options.clipboardImages.value];
    if (!plainText && sentImages.length === 0 && queuedAttachments.length === 0 && extraTextBlocks.length === 0) {
      return null;
    }
    const sendSession = options.getSession();
    if (!sendSession || !sendSession.apiConfigId || !sendSession.agentId) return null;
    const sendConversationId = normalizeConversationId(options.getConversationId ? options.getConversationId() : "");
    const attachments = options.mergeAttachmentPayloads(
      queuedAttachments,
      options.buildImageAttachmentPayload(sentImages),
    );
    const displayText = useOverrideMessage
      ? String(overrides?.displayText || "").trim() || plainText
      : buildDisplayText(plainText, sentImages, queuedAttachments);
    return {
      useOverrideMessage,
      plainText,
      displayText,
      selectedMentions,
      extraTextBlocks,
      sentImages,
      attachments,
      sendSession,
      sendConversationId,
    };
  }

  function applyPreparedSendInput(input: PreparedChatSendInput) {
    options.latestUserText.value = input.plainText;
    options.latestUserImages.value = input.sentImages.map((image) => ({
      mime: String(image.mime || ""),
      bytesBase64: String(image.bytesBase64 || ""),
    }));
    if (input.useOverrideMessage) return;
    options.chatInput.value = "";
    options.clipboardImages.value = [];
    if (options.queuedAttachmentNotices) options.queuedAttachmentNotices.value = [];
    if (options.selectedMentions) options.selectedMentions.value = [];
  }

  return {
    applyPreparedSendInput,
    prepareSendInput,
  };
}
