import { watch, type Ref } from "vue";
import type { ChatMentionTarget } from "../../../types/app";

type ClipboardImageDraft = { mime: string; bytesBase64: string; savedPath?: string };
type QueuedAttachmentDraft = { id: string; fileName: string; path: string; mime: string };

type ComposerDraft = {
  chatInput: string;
  selectedMentions: ChatMentionTarget[];
  clipboardImages: ClipboardImageDraft[];
  queuedAttachmentNotices: QueuedAttachmentDraft[];
};

type UseChatComposerDraftsOptions = {
  activeConversationId: Ref<string>;
  chatInput: Ref<string>;
  selectedMentions: Ref<ChatMentionTarget[]>;
  clipboardImages: Ref<ClipboardImageDraft[]>;
  queuedAttachmentNotices: Ref<QueuedAttachmentDraft[]>;
};

const COMPOSER_DRAFT_CACHE_LIMIT = 50;

function normalizeConversationId(value: unknown): string {
  return String(value || "").trim();
}

function cloneMention(item: ChatMentionTarget): ChatMentionTarget {
  return {
    agentId: String(item.agentId || "").trim(),
    agentName: String(item.agentName || "").trim(),
    avatarUrl: String(item.avatarUrl || "").trim() || undefined,
  };
}

function cloneClipboardImage(item: ClipboardImageDraft): ClipboardImageDraft {
  return {
    mime: String(item.mime || ""),
    bytesBase64: String(item.bytesBase64 || ""),
    savedPath: String(item.savedPath || "").trim() || undefined,
  };
}

function cloneQueuedAttachment(item: QueuedAttachmentDraft): QueuedAttachmentDraft {
  return {
    id: String(item.id || "").trim(),
    fileName: String(item.fileName || "").trim(),
    path: String(item.path || "").trim(),
    mime: String(item.mime || "").trim(),
  };
}

function cloneDraft(draft: ComposerDraft): ComposerDraft {
  return {
    chatInput: String(draft.chatInput || ""),
    selectedMentions: draft.selectedMentions.map(cloneMention),
    clipboardImages: draft.clipboardImages.map(cloneClipboardImage),
    queuedAttachmentNotices: draft.queuedAttachmentNotices.map(cloneQueuedAttachment),
  };
}

function draftHasContent(draft: ComposerDraft): boolean {
  return !!(
    draft.chatInput
    || draft.selectedMentions.length > 0
    || draft.clipboardImages.length > 0
    || draft.queuedAttachmentNotices.length > 0
  );
}

export function useChatComposerDrafts(options: UseChatComposerDraftsOptions) {
  const drafts = new Map<string, ComposerDraft>();
  let activeConversationId = normalizeConversationId(options.activeConversationId.value);

  function readCurrentDraft(): ComposerDraft {
    return {
      chatInput: String(options.chatInput.value || ""),
      selectedMentions: (Array.isArray(options.selectedMentions.value) ? options.selectedMentions.value : [])
        .map(cloneMention)
        .filter((item) => !!item.agentId),
      clipboardImages: (Array.isArray(options.clipboardImages.value) ? options.clipboardImages.value : [])
        .map(cloneClipboardImage)
        .filter((item) => !!item.mime && (!!item.bytesBase64 || !!item.savedPath)),
      queuedAttachmentNotices: (Array.isArray(options.queuedAttachmentNotices.value) ? options.queuedAttachmentNotices.value : [])
        .map(cloneQueuedAttachment)
        .filter((item) => !!item.id && !!item.path),
    };
  }

  function pruneDrafts() {
    while (drafts.size > COMPOSER_DRAFT_CACHE_LIMIT) {
      const oldestConversationId = drafts.keys().next().value;
      if (!oldestConversationId) return;
      drafts.delete(oldestConversationId);
    }
  }

  function storeCurrentDraft(conversationId: string) {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return;
    const draft = readCurrentDraft();
    drafts.delete(cid);
    if (draftHasContent(draft)) {
      drafts.set(cid, draft);
      pruneDrafts();
    }
  }

  function applyDraft(conversationId: string) {
    const cid = normalizeConversationId(conversationId);
    const draft = cid ? drafts.get(cid) : null;
    const next = draft
      ? cloneDraft(draft)
      : {
          chatInput: "",
          selectedMentions: [],
          clipboardImages: [],
          queuedAttachmentNotices: [],
        };
    options.chatInput.value = next.chatInput;
    options.selectedMentions.value = next.selectedMentions;
    options.clipboardImages.value = next.clipboardImages;
    options.queuedAttachmentNotices.value = next.queuedAttachmentNotices;
  }

  watch(
    () => normalizeConversationId(options.activeConversationId.value),
    (nextConversationId) => {
      if (activeConversationId && activeConversationId !== nextConversationId) {
        storeCurrentDraft(activeConversationId);
      }
      activeConversationId = nextConversationId;
      applyDraft(nextConversationId);
    },
    { immediate: true, flush: "sync" },
  );

  watch(
    () => [
      options.chatInput.value,
      options.selectedMentions.value,
      options.clipboardImages.value,
      options.queuedAttachmentNotices.value,
    ],
    () => {
      storeCurrentDraft(activeConversationId);
    },
    { deep: true },
  );

  return {
    storeCurrentDraft: () => storeCurrentDraft(activeConversationId),
  };
}
