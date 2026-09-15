import { computed, watch, type Ref } from "vue";
import type { ChatMessageBlock, ConversationForwardTarget } from "../../../types/app";
import { stripToolcallMarkers } from "../../../utils/chat-message-semantics";

export interface UseChatSelectionOptions {
  chatRenderItems: Ref<{ renderId: string; block: ChatMessageBlock }[]>;
  messageSelectionModeEnabled: Ref<boolean>;
  selectedMessageRenderIds: Ref<string[]>;
  personaNameMap: Record<string, string>;
  userAlias: string;
  conversationId?: Ref<string> | string;
  t: (key: string, params?: Record<string, unknown>) => string;
  onEmit: {
    selectionActionCopy: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string }) => void;
    selectionActionCopyError: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; error: string }) => void;
    selectionActionBranch: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string }) => void;
    selectionActionForward: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; target: ConversationForwardTarget }) => void;
    selectionActionDelegate: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; agentId: string; presetId: string; why: string; goal: string; todo: string }) => void;
    selectionActionShare: (payload: { count: number; messageIds: string[]; blocks: ChatMessageBlock[]; conversationId?: string; exportFormat?: "html" | "png" | "copyPng" }) => void;
  };
}

type DelegateActionPayload = {
  agentId: string;
  presetId: string;
  why: string;
  goal: string;
  todo: string;
};

export function useChatSelection(options: UseChatSelectionOptions) {
  const {
    chatRenderItems,
    messageSelectionModeEnabled,
    selectedMessageRenderIds,
    personaNameMap,
    userAlias,
    conversationId,
    t,
    onEmit,
  } = options;

  const selectedMessageRenderIdSet = computed(() => new Set(selectedMessageRenderIds.value));

  const renderedMessageItems = computed(() =>
    chatRenderItems.value.flatMap((item) => {
      if (item.renderId && item.block) return [{ renderId: item.renderId, block: item.block }];
      return [];
    }),
  );

  const selectedMessageBlocks = computed(() =>
    renderedMessageItems.value.filter((item) => selectedMessageRenderIdSet.value.has(item.renderId)),
  );

  function enterMessageSelectionMode(selectionKey: string) {
    const normalizedSelectionKey = String(selectionKey || "").trim();
    if (!normalizedSelectionKey) return;
    messageSelectionModeEnabled.value = true;
    if (!selectedMessageRenderIds.value.includes(normalizedSelectionKey)) {
      selectedMessageRenderIds.value = [...selectedMessageRenderIds.value, normalizedSelectionKey];
    }
  }

  function toggleMessageSelected(selectionKey: string) {
    const normalizedSelectionKey = String(selectionKey || "").trim();
    if (!normalizedSelectionKey) return;
    if (!messageSelectionModeEnabled.value) {
      enterMessageSelectionMode(normalizedSelectionKey);
      return;
    }
    if (selectedMessageRenderIds.value.includes(normalizedSelectionKey)) {
      selectedMessageRenderIds.value = selectedMessageRenderIds.value.filter((item) => item !== normalizedSelectionKey);
      return;
    }
    selectedMessageRenderIds.value = [...selectedMessageRenderIds.value, normalizedSelectionKey];
  }

  function exitMessageSelectionMode() {
    messageSelectionModeEnabled.value = false;
    selectedMessageRenderIds.value = [];
  }

  function selectionPayload() {
    const blocks = selectedMessageBlocks.value.map((item) => item.block);
    const cid = typeof conversationId === "string"
      ? conversationId
      : String(conversationId?.value || "").trim();
    return {
      count: blocks.length,
      messageIds: blocks.map((block) => String(block.sourceMessageId || block.id || "").trim()).filter(Boolean),
      blocks,
      conversationId: cid || undefined,
    };
  }

  function selectionDisplayName(block: ChatMessageBlock): string {
    if (block.remoteImOrigin) {
      return block.remoteImOrigin.senderName || block.remoteImOrigin.remoteContactName || "IM";
    }
    const speakerAgentId = String(block.speakerAgentId || "").trim();
    if (speakerAgentId && personaNameMap[speakerAgentId]) return personaNameMap[speakerAgentId];
    if (!speakerAgentId || speakerAgentId === "user-persona" || block.role === "user") {
      return userAlias || t("archives.roleUser");
    }
    return speakerAgentId || block.role;
  }

  function selectionBlockSummary(block: ChatMessageBlock): string {
    const parts: string[] = [];
    const text = stripToolcallMarkers(block.text || "");
    if (text) parts.push(text);
    if (block.images.length > 0) parts.push(t("chat.imageCount", { count: block.images.length }));
    if (block.audios.length > 0) parts.push(t("chat.audioCount", { count: block.audios.length }));
    if (block.attachmentFiles.length > 0) parts.push(t("chat.attachmentList", { names: block.attachmentFiles.map((item) => item.fileName).join("、") }));
    return parts.join("\n").trim();
  }

  async function copySelectedMessages() {
    const payload = selectionPayload();
    if (payload.count === 0) return;
    const text = payload.blocks
      .map((block) => `[${selectionDisplayName(block)}]: ${selectionBlockSummary(block) || "[\u7A7A\u6D88\u606F]"}`)
      .join("\n\n");
    if (!text.trim()) return;
    try {
      await navigator.clipboard.writeText(text);
      onEmit.selectionActionCopy(payload);
    } catch (error) {
      onEmit.selectionActionCopyError({ ...payload, error: error instanceof Error ? error.message : String(error) });
    }
  }

  function emitSelectionAction(
    kind: "branch" | "share" | "forward" | "delegate",
    actionPayload: ConversationForwardTarget | DelegateActionPayload | "" | "html" | "png" | "copyPng" = "",
  ) {
    const payload = selectionPayload();
    if (kind === "branch") {
      if (payload.count === 0) return;
      onEmit.selectionActionBranch(payload);
      return;
    }
    if (kind === "forward") {
      if (payload.count === 0) return;
      if (!actionPayload || typeof actionPayload === "string" || !("conversationId" in actionPayload)) return;
      const target = {
        kind: actionPayload.kind === "remote_im_contact" ? "remote_im_contact" : "local_unarchived",
        conversationId: String(actionPayload.conversationId || "").trim(),
        remoteContactId: String(actionPayload.remoteContactId || "").trim() || undefined,
      } satisfies ConversationForwardTarget;
      if (!target.conversationId) return;
      if (target.kind === "remote_im_contact" && !target.remoteContactId) return;
      onEmit.selectionActionForward({ ...payload, target });
      return;
    }
    if (kind === "delegate") {
      if (!actionPayload || typeof actionPayload === "string" || !("agentId" in actionPayload)) return;
      onEmit.selectionActionDelegate({
        ...payload,
        agentId: String(actionPayload.agentId || "").trim(),
        presetId: String(actionPayload.presetId || "review").trim() || "review",
        why: String(actionPayload.why || "").trim(),
        goal: String(actionPayload.goal || "").trim(),
        todo: String(actionPayload.todo || "").trim(),
      });
      return;
    }
    if (payload.count === 0) return;
    const exportFormat = actionPayload === "html" || actionPayload === "png" || actionPayload === "copyPng" ? actionPayload : undefined;
    onEmit.selectionActionShare({ ...payload, exportFormat });
  }

  watch(
    () => ({
      selectionModeEnabled: messageSelectionModeEnabled.value,
      selectedRenderIdCount: selectedMessageRenderIds.value.length,
      selectedBlockCount: selectedMessageBlocks.value.length,
    }),
    ({ selectionModeEnabled, selectedRenderIdCount, selectedBlockCount }) => {
      if (!selectionModeEnabled) return;
      if (selectedRenderIdCount === 0) return;
      if (selectedBlockCount === 0) exitMessageSelectionMode();
    },
  );

  return {
    selectedMessageRenderIdSet,
    selectedMessageBlocks,
    enterMessageSelectionMode,
    toggleMessageSelected,
    exitMessageSelectionMode,
    copySelectedMessages,
    emitSelectionAction,
  };
}
