import { onBeforeUnmount, onMounted, ref } from "vue";
import { exportTransportArchive, invokeTauri, onTransportNotification } from "../../../services/tauri-api";
import type {
  ArchiveBlockPage,
  ArchiveSummary,
  ChatMessage,
  ConversationBlockSummary,
  DelegateConversationSummary,
  RemoteImContactConversationSummary,
  UnarchivedConversationSummary,
} from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type ExportArchiveFileResult = {
  path: string;
  archiveId: string;
  format: "json" | "markdown";
};

export type ArchiveImportPreview = {
  fileName: string;
  total: number;
  imported: number;
  replaced: number;
  payloadJson: string;
};

type ImportArchivesResult = {
  importedCount: number;
  replacedCount: number;
  skippedCount: number;
  totalCount: number;
  selectedArchiveId?: string | null;
};

type DeleteUnarchivedConversationResult = {
  deletedConversationId: string;
  activeConversationId: string;
};

type DeleteDelegateConversationResult = {
  deletedConversationId: string;
  deleted: boolean;
};

const ARCHIVE_FOCUS_REQUEST_STORAGE_KEY = "easy_call.archives.focus_request.v1";

type UseArchivesViewOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function collectArchiveObjects(payload: unknown): Record<string, unknown>[] {
  if (Array.isArray(payload)) {
    return payload.filter(isRecord);
  }
  if (!isRecord(payload)) {
    return [];
  }
  const wrappedArchive = payload.archive;
  if (isRecord(wrappedArchive)) {
    return [wrappedArchive];
  }
  const archives = payload.archives;
  if (Array.isArray(archives)) {
    return archives.filter(isRecord);
  }
  const archivedConversations = payload.archivedConversations;
  if (Array.isArray(archivedConversations)) {
    return archivedConversations.filter(isRecord);
  }
  if (isRecord(payload.sourceConversation)) {
    return [payload];
  }
  return [];
}

function archiveIdFromPayloadObject(archive: Record<string, unknown>): string {
  const raw = archive.archiveId ?? archive.archive_id;
  return typeof raw === "string" ? raw.trim() : "";
}

export function useArchivesView(options: UseArchivesViewOptions) {
  const archives = ref<ArchiveSummary[]>([]);
  const archiveBlocks = ref<ConversationBlockSummary[]>([]);
  const archiveMessages = ref<ChatMessage[]>([]);
  const selectedArchiveId = ref("");
  const selectedArchiveBlockId = ref<number | null>(null);
  const archiveHasPrevBlock = ref(false);
  const archiveHasNextBlock = ref(false);
  const unarchivedConversations = ref<UnarchivedConversationSummary[]>([]);
  const unarchivedBlocks = ref<ConversationBlockSummary[]>([]);
  const unarchivedMessages = ref<ChatMessage[]>([]);
  const selectedUnarchivedConversationId = ref("");
  const selectedUnarchivedBlockId = ref<number | null>(null);
  const unarchivedHasPrevBlock = ref(false);
  const unarchivedHasNextBlock = ref(false);
  const delegateConversations = ref<DelegateConversationSummary[]>([]);
  const delegateBlocks = ref<ConversationBlockSummary[]>([]);
  const delegateMessages = ref<ChatMessage[]>([]);
  const selectedDelegateConversationId = ref("");
  const selectedDelegateBlockId = ref<number | null>(null);
  const delegateHasPrevBlock = ref(false);
  const delegateHasNextBlock = ref(false);
  const remoteImContactConversations = ref<RemoteImContactConversationSummary[]>([]);
  const remoteImContactBlocks = ref<ConversationBlockSummary[]>([]);
  const remoteImContactMessages = ref<ChatMessage[]>([]);
  const selectedRemoteImContactId = ref("");
  const selectedRemoteImContactBlockId = ref<number | null>(null);
  const remoteImHasPrevBlock = ref(false);
  const remoteImHasNextBlock = ref(false);

  async function selectUnarchivedConversation(conversationId: string) {
    const previousId = selectedUnarchivedConversationId.value;
    const previousBlocks = unarchivedBlocks.value;
    const previousBlockId = selectedUnarchivedBlockId.value;
    const previousMessages = unarchivedMessages.value;
    const previousHasPrev = unarchivedHasPrevBlock.value;
    const previousHasNext = unarchivedHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("conversation.blockPage", {
        input: { conversationId },
      });
      selectedUnarchivedConversationId.value = conversationId;
      unarchivedBlocks.value = Array.isArray(page?.blocks) ? page.blocks : [];
      selectedUnarchivedBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      unarchivedMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      unarchivedHasPrevBlock.value = !!page?.hasPrevBlock;
      unarchivedHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedUnarchivedConversationId.value = previousId;
      unarchivedBlocks.value = previousBlocks;
      selectedUnarchivedBlockId.value = previousBlockId;
      unarchivedMessages.value = previousMessages;
      unarchivedHasPrevBlock.value = previousHasPrev;
      unarchivedHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function selectUnarchivedConversationBlock(blockId?: number | null) {
    const conversationId = String(selectedUnarchivedConversationId.value || "").trim();
    if (!conversationId) return;
    const previousBlocks = unarchivedBlocks.value;
    const previousBlockId = selectedUnarchivedBlockId.value;
    const previousMessages = unarchivedMessages.value;
    const previousHasPrev = unarchivedHasPrevBlock.value;
    const previousHasNext = unarchivedHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("conversation.blockPage", {
        input: {
          conversationId,
          blockId: typeof blockId === "number" ? blockId : undefined,
        },
      });
      unarchivedBlocks.value = Array.isArray(page?.blocks) ? page.blocks : unarchivedBlocks.value;
      selectedUnarchivedBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      unarchivedMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      unarchivedHasPrevBlock.value = !!page?.hasPrevBlock;
      unarchivedHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      unarchivedBlocks.value = previousBlocks;
      selectedUnarchivedBlockId.value = previousBlockId;
      unarchivedMessages.value = previousMessages;
      unarchivedHasPrevBlock.value = previousHasPrev;
      unarchivedHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadUnarchivedConversations() {
    try {
      unarchivedConversations.value = await invokeTauri<UnarchivedConversationSummary[]>("conversation.overview.list");
      if (unarchivedConversations.value.length === 0) {
        selectedUnarchivedConversationId.value = "";
        selectedUnarchivedBlockId.value = null;
        unarchivedBlocks.value = [];
        unarchivedMessages.value = [];
        unarchivedHasPrevBlock.value = false;
        unarchivedHasNextBlock.value = false;
        return;
      }
      const targetId = unarchivedConversations.value.some((item) => item.conversationId === selectedUnarchivedConversationId.value)
        ? selectedUnarchivedConversationId.value
        : unarchivedConversations.value[0].conversationId;
      await selectUnarchivedConversation(targetId);
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadUnarchivedConversationListOnly() {
    try {
      unarchivedConversations.value = await invokeTauri<UnarchivedConversationSummary[]>("conversation.overview.list");
      const selectedId = String(selectedUnarchivedConversationId.value || "").trim();
      if (!unarchivedConversations.value.some((item) => String(item.conversationId || "").trim() === selectedId)) {
        selectedUnarchivedConversationId.value = "";
        selectedUnarchivedBlockId.value = null;
        unarchivedBlocks.value = [];
        unarchivedMessages.value = [];
        unarchivedHasPrevBlock.value = false;
        unarchivedHasNextBlock.value = false;
      }
      if (unarchivedConversations.value.length === 0) {
        selectedUnarchivedBlockId.value = null;
        unarchivedBlocks.value = [];
        unarchivedMessages.value = [];
        unarchivedHasPrevBlock.value = false;
        unarchivedHasNextBlock.value = false;
      }
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function selectDelegateConversation(conversationId: string) {
    const previousId = selectedDelegateConversationId.value;
    const previousBlocks = delegateBlocks.value;
    const previousBlockId = selectedDelegateBlockId.value;
    const previousMessages = delegateMessages.value;
    const previousHasPrev = delegateHasPrevBlock.value;
    const previousHasNext = delegateHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("delegate.blockPage", {
        input: { conversationId },
      });
      selectedDelegateConversationId.value = conversationId;
      delegateBlocks.value = Array.isArray(page?.blocks) ? page.blocks : [];
      selectedDelegateBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      delegateMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      delegateHasPrevBlock.value = !!page?.hasPrevBlock;
      delegateHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedDelegateConversationId.value = previousId;
      delegateBlocks.value = previousBlocks;
      selectedDelegateBlockId.value = previousBlockId;
      delegateMessages.value = previousMessages;
      delegateHasPrevBlock.value = previousHasPrev;
      delegateHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function selectDelegateConversationBlock(blockId?: number | null) {
    const conversationId = String(selectedDelegateConversationId.value || "").trim();
    if (!conversationId) return;
    const previousBlocks = delegateBlocks.value;
    const previousBlockId = selectedDelegateBlockId.value;
    const previousMessages = delegateMessages.value;
    const previousHasPrev = delegateHasPrevBlock.value;
    const previousHasNext = delegateHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("delegate.blockPage", {
        input: {
          conversationId,
          blockId: typeof blockId === "number" ? blockId : undefined,
        },
      });
      delegateBlocks.value = Array.isArray(page?.blocks) ? page.blocks : delegateBlocks.value;
      selectedDelegateBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      delegateMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      delegateHasPrevBlock.value = !!page?.hasPrevBlock;
      delegateHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      delegateBlocks.value = previousBlocks;
      selectedDelegateBlockId.value = previousBlockId;
      delegateMessages.value = previousMessages;
      delegateHasPrevBlock.value = previousHasPrev;
      delegateHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadDelegateConversations() {
    try {
      delegateConversations.value = await invokeTauri<DelegateConversationSummary[]>("delegate.conversations.list");
      if (delegateConversations.value.length === 0) {
        selectedDelegateConversationId.value = "";
        selectedDelegateBlockId.value = null;
        delegateBlocks.value = [];
        delegateMessages.value = [];
        delegateHasPrevBlock.value = false;
        delegateHasNextBlock.value = false;
        return;
      }
      const targetId = delegateConversations.value.some((item) => item.conversationId === selectedDelegateConversationId.value)
        ? selectedDelegateConversationId.value
        : delegateConversations.value[0].conversationId;
      await selectDelegateConversation(targetId);
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadArchives() {
    await Promise.all([
      loadUnarchivedConversations(),
      loadDelegateConversations(),
      loadRemoteImContactConversations(),
    ]);
    try {
      archives.value = await invokeTauri<ArchiveSummary[]>("archives.list");
      if (archives.value.length === 0) {
        selectedArchiveId.value = "";
        selectedArchiveBlockId.value = null;
        archiveBlocks.value = [];
        archiveMessages.value = [];
        archiveHasPrevBlock.value = false;
        archiveHasNextBlock.value = false;
        return;
      }
      const targetId = archives.value.some((a) => a.archiveId === selectedArchiveId.value)
        ? selectedArchiveId.value
        : archives.value[0].archiveId;
      await selectArchive(targetId);
    } catch (e) {
      options.setStatusError("status.loadArchivesFailed", e);
    }
  }

  async function selectRemoteImContactConversation(contactId: string) {
    const previousId = selectedRemoteImContactId.value;
    const previousBlocks = remoteImContactBlocks.value;
    const previousBlockId = selectedRemoteImContactBlockId.value;
    const previousMessages = remoteImContactMessages.value;
    const previousHasPrev = remoteImHasPrevBlock.value;
    const previousHasNext = remoteImHasNextBlock.value;
    // 先更新高亮，避免等待消息加载导致左侧选中反馈卡顿。
    selectedRemoteImContactId.value = contactId;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("remoteIm.conversation.blockPage", {
        input: { contactId },
      });
      remoteImContactBlocks.value = Array.isArray(page?.blocks) ? page.blocks : [];
      selectedRemoteImContactBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      remoteImContactMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      remoteImHasPrevBlock.value = !!page?.hasPrevBlock;
      remoteImHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedRemoteImContactId.value = previousId;
      remoteImContactBlocks.value = previousBlocks;
      selectedRemoteImContactBlockId.value = previousBlockId;
      remoteImContactMessages.value = previousMessages;
      remoteImHasPrevBlock.value = previousHasPrev;
      remoteImHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function selectRemoteImContactConversationBlock(blockId?: number | null) {
    const contactId = String(selectedRemoteImContactId.value || "").trim();
    if (!contactId) return;
    const previousBlocks = remoteImContactBlocks.value;
    const previousBlockId = selectedRemoteImContactBlockId.value;
    const previousMessages = remoteImContactMessages.value;
    const previousHasPrev = remoteImHasPrevBlock.value;
    const previousHasNext = remoteImHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("remoteIm.conversation.blockPage", {
        input: {
          contactId,
          blockId: typeof blockId === "number" ? blockId : undefined,
        },
      });
      remoteImContactBlocks.value = Array.isArray(page?.blocks) ? page.blocks : remoteImContactBlocks.value;
      selectedRemoteImContactBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      remoteImContactMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      remoteImHasPrevBlock.value = !!page?.hasPrevBlock;
      remoteImHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      remoteImContactBlocks.value = previousBlocks;
      selectedRemoteImContactBlockId.value = previousBlockId;
      remoteImContactMessages.value = previousMessages;
      remoteImHasPrevBlock.value = previousHasPrev;
      remoteImHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadRemoteImContactConversations() {
    try {
      const previousSelectedId = selectedRemoteImContactId.value;
      remoteImContactConversations.value =
        await invokeTauri<RemoteImContactConversationSummary[]>("remoteIm.conversations.list");
      if (remoteImContactConversations.value.length === 0) {
        selectedRemoteImContactId.value = "";
        selectedRemoteImContactBlockId.value = null;
        remoteImContactBlocks.value = [];
        remoteImContactMessages.value = [];
        remoteImHasPrevBlock.value = false;
        remoteImHasNextBlock.value = false;
        return;
      }
      const targetId = remoteImContactConversations.value.some((item) => item.contactId === selectedRemoteImContactId.value)
        ? selectedRemoteImContactId.value
        : remoteImContactConversations.value[0].contactId;
      selectedRemoteImContactId.value = targetId;
      if (targetId !== previousSelectedId) {
        remoteImContactMessages.value = [];
      }
      // 列表优先响应，消息异步加载，避免大会话阻塞左侧选中反馈。
      void selectRemoteImContactConversation(targetId);
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  function sortUnarchivedConversationItems(items: UnarchivedConversationSummary[]) {
    return [...items].sort((a, b) => {
      if (!!a.isSystemNotificationConversation !== !!b.isSystemNotificationConversation) {
        return Number(!!b.isSystemNotificationConversation) - Number(!!a.isSystemNotificationConversation);
      }
      if (!!a.isPinned !== !!b.isPinned) {
        return Number(!!b.isPinned) - Number(!!a.isPinned);
      }
      if (a.isPinned && b.isPinned) {
        const aIndex = Number.isFinite(Number(a.pinIndex)) ? Number(a.pinIndex) : Number.MAX_SAFE_INTEGER;
        const bIndex = Number.isFinite(Number(b.pinIndex)) ? Number(b.pinIndex) : Number.MAX_SAFE_INTEGER;
        return aIndex - bIndex || String(a.conversationId || "").localeCompare(String(b.conversationId || ""));
      }
      const aActivity = String(a.lastMessageAt || a.updatedAt || "").trim();
      const bActivity = String(b.lastMessageAt || b.updatedAt || "").trim();
      return bActivity.localeCompare(aActivity) || String(a.conversationId || "").localeCompare(String(b.conversationId || ""));
    });
  }

  function applyUnarchivedConversationOverviewItemUpdated(payload?: Record<string, any> | null) {
    const conversation = payload?.conversation as UnarchivedConversationSummary | undefined;
    const conversationId = String(conversation?.conversationId || "").trim();
    if (!conversationId || !conversation) return;
    let replaced = false;
    const nextItems = unarchivedConversations.value.map((item) => {
      if (String(item.conversationId || "").trim() !== conversationId) return item;
      replaced = true;
      return { ...item, ...conversation };
    });
    if (!replaced) {
      nextItems.push(conversation);
    }
    unarchivedConversations.value = sortUnarchivedConversationItems(nextItems);
  }

  async function selectArchive(archiveId: string) {
    const previousId = selectedArchiveId.value;
    const previousBlockId = selectedArchiveBlockId.value;
    const previousBlocks = archiveBlocks.value;
    const previousMessages = archiveMessages.value;
    const previousHasPrev = archiveHasPrevBlock.value;
    const previousHasNext = archiveHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("archives.blockPage", {
        input: { archiveId },
      });
      selectedArchiveId.value = archiveId;
      archiveBlocks.value = Array.isArray(page?.blocks) ? page.blocks : [];
      selectedArchiveBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      archiveMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      archiveHasPrevBlock.value = !!page?.hasPrevBlock;
      archiveHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedArchiveId.value = previousId;
      selectedArchiveBlockId.value = previousBlockId;
      archiveBlocks.value = previousBlocks;
      archiveMessages.value = previousMessages;
      archiveHasPrevBlock.value = previousHasPrev;
      archiveHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadArchivesFailed", e);
    }
  }

  async function selectArchiveBlock(blockId?: number | null) {
    const archiveId = String(selectedArchiveId.value || "").trim();
    if (!archiveId) return;
    const previousBlockId = selectedArchiveBlockId.value;
    const previousMessages = archiveMessages.value;
    const previousHasPrev = archiveHasPrevBlock.value;
    const previousHasNext = archiveHasNextBlock.value;
    try {
      const page = await invokeTauri<ArchiveBlockPage>("archives.blockPage", {
        input: {
          archiveId,
          blockId: typeof blockId === "number" ? blockId : undefined,
        },
      });
      archiveBlocks.value = Array.isArray(page?.blocks) ? page.blocks : archiveBlocks.value;
      selectedArchiveBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      archiveMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      archiveHasPrevBlock.value = !!page?.hasPrevBlock;
      archiveHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedArchiveBlockId.value = previousBlockId;
      archiveMessages.value = previousMessages;
      archiveHasPrevBlock.value = previousHasPrev;
      archiveHasNextBlock.value = previousHasNext;
      options.setStatusError("status.loadArchivesFailed", e);
    }
  }

  async function deleteArchive(archiveId: string) {
    if (!archiveId) return;
    try {
      await invokeTauri("archives.delete", { archiveId });
      options.setStatus(options.t("status.archiveDeleted"));
      if (selectedArchiveId.value === archiveId) {
        selectedArchiveId.value = "";
        selectedArchiveBlockId.value = null;
        archiveBlocks.value = [];
        archiveMessages.value = [];
        archiveHasPrevBlock.value = false;
        archiveHasNextBlock.value = false;
      }
      await loadArchives();
    } catch (e) {
      options.setStatusError("status.deleteArchiveFailed", e);
    }
  }

  async function unarchiveArchive(archiveId: string) {
    const conversationId = String(archiveId || "").trim();
    if (!conversationId) return;
    try {
      await invokeTauri("archives.unarchive", { archiveId: conversationId });
      selectedArchiveId.value = "";
      selectedArchiveBlockId.value = null;
      archiveBlocks.value = [];
      archiveMessages.value = [];
      archiveHasPrevBlock.value = false;
      archiveHasNextBlock.value = false;
      selectedUnarchivedConversationId.value = conversationId;
      if (typeof window !== "undefined") {
        window.localStorage.setItem(ARCHIVE_FOCUS_REQUEST_STORAGE_KEY, JSON.stringify({
          conversationId,
          viewMode: "current",
          createdAt: Date.now(),
        }));
      }
      await loadArchives();
      options.setStatus(options.t("status.archiveUnarchived"));
    } catch (e) {
      options.setStatusError("status.unarchiveArchiveFailed", e);
    }
  }

  async function deleteUnarchivedConversation(conversationId: string): Promise<DeleteUnarchivedConversationResult | null> {
    if (!conversationId) return null;
    const summary = unarchivedConversations.value.find(
      (item) => String(item.conversationId || "").trim() === conversationId,
    );
    if (summary?.isSystemNotificationConversation) {
      options.setStatus("系统通知会话暂不支持删除。");
      return null;
    }
    try {
      console.info("[归档] delete current unarchived conversation", { conversationId });
      const result = await invokeTauri<DeleteUnarchivedConversationResult>("conversation.delete", {
        input: { conversationId },
      });
      options.setStatus(options.t("status.unarchivedConversationDeleted"));
      // 后端删除后不再返回全量列表：本地过滤被删项，避免残留陈旧项。
      unarchivedConversations.value = unarchivedConversations.value
        .filter((item) => String(item.conversationId || "").trim() !== conversationId);
      const nextConversationId = String(result?.activeConversationId || "").trim();
      if (selectedUnarchivedConversationId.value === conversationId) {
        selectedUnarchivedConversationId.value = nextConversationId;
        selectedUnarchivedBlockId.value = null;
        unarchivedBlocks.value = [];
        unarchivedMessages.value = [];
        unarchivedHasPrevBlock.value = false;
        unarchivedHasNextBlock.value = false;
      }
      return result;
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
      return null;
    }
  }

  async function deleteRemoteImContactConversation(contactId: string) {
    if (!contactId) return;
    try {
      await invokeTauri<boolean>("remoteIm.conversation.clear", {
        input: { contactId },
      });
      options.setStatus("联系人会话已删除。");
      await loadRemoteImContactConversations();
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
    }
  }

  async function deleteDelegateConversation(conversationId: string): Promise<DeleteDelegateConversationResult | null> {
    if (!conversationId) return null;
    try {
      const result = await invokeTauri<DeleteDelegateConversationResult>("delegate.delete", {
        input: { conversationId },
      });
      options.setStatus("委托会话已删除。");
      if (selectedDelegateConversationId.value === conversationId) {
        selectedDelegateConversationId.value = "";
        selectedDelegateBlockId.value = null;
        delegateBlocks.value = [];
        delegateMessages.value = [];
        delegateHasPrevBlock.value = false;
        delegateHasNextBlock.value = false;
      }
      await loadDelegateConversations();
      return result;
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
      return null;
    }
  }

  async function exportArchive(payload: { format: "markdown" | "json" }) {
    if (!selectedArchiveId.value) {
      options.setStatus(options.t("status.selectArchiveFirst"));
      return;
    }
    try {
      const result = await exportTransportArchive<ExportArchiveFileResult>({
        archiveId: selectedArchiveId.value,
        format: payload.format,
      });
      options.setStatus(options.t("status.archiveExported", { format: result.format, path: result.path }));
    } catch (e) {
      options.setStatusError("status.exportArchiveFailed", e);
    }
  }

  async function buildArchiveImportPreview(file: File): Promise<ArchiveImportPreview> {
    const payloadJson = await file.text();
    let parsed: unknown;
    try {
      parsed = JSON.parse(payloadJson);
    } catch {
      throw new Error("Invalid JSON file.");
    }
    const archivesInPayload = collectArchiveObjects(parsed);
    if (archivesInPayload.length === 0) {
      throw new Error("No archive records found.");
    }
    const existingIds = new Set(archives.value.map((item) => item.archiveId));
    let replaced = 0;
    for (const archive of archivesInPayload) {
      const archiveId = archiveIdFromPayloadObject(archive);
      if (archiveId && existingIds.has(archiveId)) {
        replaced += 1;
      }
    }
    const total = archivesInPayload.length;
    const imported = Math.max(0, total - replaced);
    return {
      fileName: (file.name || "archive.json").trim() || "archive.json",
      total,
      imported,
      replaced,
      payloadJson,
    };
  }

  async function importArchivePayload(payloadJson: string) {
    try {
      const result = await invokeTauri<ImportArchivesResult>("conversation.importArchives", {
        input: { payloadJson },
      });
      if (result.selectedArchiveId) {
        selectedArchiveId.value = result.selectedArchiveId;
      }
      await loadArchives();
      options.setStatus(
        options.t("status.importArchiveDone", {
          imported: result.importedCount,
          replaced: result.replacedCount,
          total: result.totalCount,
        }),
      );
    } catch (err) {
      options.setStatusError("status.importArchiveFailed", err);
    }
  }

  // ==================== 待答问题计数 ====================
  //
  // 问题增删或答题后，后端只推「这个会话现在有哪些待答问题」，不像概览项那样整项下发。
  // 会话行的计数就跟着这份列表走，不必为了一个数字重新拉一遍列表。
  // 列表项在 ref 里是深层响应式对象，改字段即可触发会话行重算，不需要换数组引用。
  function applyQuestionCount(conversationId: unknown, count: number) {
    const id = String(conversationId || "").trim();
    if (!id) return;
    const target = unarchivedConversations.value.find((item) => item.conversationId === id);
    if (!target) return;
    target.questionCount = count;
  }

  let unlistenQuestionItemsUpdated: (() => void) | undefined;
  onMounted(() => {
    unlistenQuestionItemsUpdated = onTransportNotification<{
      conversationId?: string;
      items?: unknown[];
    }>("question.itemsUpdated", (payload) => {
      const items = payload?.items;
      applyQuestionCount(payload?.conversationId, Array.isArray(items) ? items.length : 0);
    });
  });
  onBeforeUnmount(() => {
    unlistenQuestionItemsUpdated?.();
    unlistenQuestionItemsUpdated = undefined;
  });

  return {
    archives,
    archiveBlocks,
    archiveMessages,
    selectedArchiveId,
    selectedArchiveBlockId,
    archiveHasPrevBlock,
    archiveHasNextBlock,
    unarchivedConversations,
    unarchivedBlocks,
    unarchivedMessages,
    selectedUnarchivedConversationId,
    selectedUnarchivedBlockId,
    unarchivedHasPrevBlock,
    unarchivedHasNextBlock,
    delegateConversations,
    delegateBlocks,
    delegateMessages,
    selectedDelegateConversationId,
    selectedDelegateBlockId,
    delegateHasPrevBlock,
    delegateHasNextBlock,
    remoteImContactConversations,
    remoteImContactBlocks,
    remoteImContactMessages,
    selectedRemoteImContactId,
    selectedRemoteImContactBlockId,
    remoteImHasPrevBlock,
    remoteImHasNextBlock,
    selectUnarchivedConversation,
    selectUnarchivedConversationBlock,
    selectDelegateConversation,
    selectDelegateConversationBlock,
    selectRemoteImContactConversation,
    selectRemoteImContactConversationBlock,
    loadUnarchivedConversations,
    loadUnarchivedConversationListOnly,
    loadDelegateConversations,
    loadRemoteImContactConversations,
    loadArchives,
    applyUnarchivedConversationOverviewItemUpdated,
    selectArchive,
    selectArchiveBlock,
    deleteUnarchivedConversation,
    deleteDelegateConversation,
    deleteRemoteImContactConversation,
    deleteArchive,
    unarchiveArchive,
    exportArchive,
    buildArchiveImportPreview,
    importArchivePayload,
  };
}
