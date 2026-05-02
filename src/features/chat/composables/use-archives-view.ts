import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
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
  const archiveSummaryText = ref("");
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
  const delegateMessages = ref<ChatMessage[]>([]);
  const selectedDelegateConversationId = ref("");
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
      const page = await invokeTauri<ArchiveBlockPage>("get_unarchived_conversation_block_page", {
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
      const page = await invokeTauri<ArchiveBlockPage>("get_unarchived_conversation_block_page", {
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
      unarchivedConversations.value = await invokeTauri<UnarchivedConversationSummary[]>("list_unarchived_conversations");
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
      unarchivedConversations.value = await invokeTauri<UnarchivedConversationSummary[]>("list_unarchived_conversations");
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
    const previousMessages = delegateMessages.value;
    try {
      const messages = await invokeTauri<ChatMessage[]>("get_delegate_conversation_messages", {
        input: { conversationId },
      });
      selectedDelegateConversationId.value = conversationId;
      delegateMessages.value = messages;
    } catch (e) {
      selectedDelegateConversationId.value = previousId;
      delegateMessages.value = previousMessages;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadDelegateConversations() {
    try {
      delegateConversations.value = await invokeTauri<DelegateConversationSummary[]>("list_delegate_conversations");
      if (delegateConversations.value.length === 0) {
        selectedDelegateConversationId.value = "";
        delegateMessages.value = [];
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
      archives.value = await invokeTauri<ArchiveSummary[]>("list_archives");
      if (archives.value.length === 0) {
        selectedArchiveId.value = "";
        selectedArchiveBlockId.value = null;
        archiveBlocks.value = [];
        archiveMessages.value = [];
        archiveSummaryText.value = "";
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
      const page = await invokeTauri<ArchiveBlockPage>("remote_im_get_contact_conversation_block_page", {
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
      const page = await invokeTauri<ArchiveBlockPage>("remote_im_get_contact_conversation_block_page", {
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
        await invokeTauri<RemoteImContactConversationSummary[]>("remote_im_list_contact_conversations");
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

  async function selectArchive(archiveId: string) {
    const previousId = selectedArchiveId.value;
    const previousBlockId = selectedArchiveBlockId.value;
    const previousBlocks = archiveBlocks.value;
    const previousMessages = archiveMessages.value;
    const previousSummary = archiveSummaryText.value;
    const previousHasPrev = archiveHasPrevBlock.value;
    const previousHasNext = archiveHasNextBlock.value;
    try {
      const summary = await invokeTauri<string>("get_archive_summary", { archiveId });
      const page = await invokeTauri<ArchiveBlockPage>("get_archive_block_page", {
        input: { archiveId },
      });
      selectedArchiveId.value = archiveId;
      archiveSummaryText.value = String(summary || "").trim();
      archiveBlocks.value = Array.isArray(page?.blocks) ? page.blocks : [];
      selectedArchiveBlockId.value = Number.isFinite(page?.selectedBlockId) ? page.selectedBlockId : null;
      archiveMessages.value = Array.isArray(page?.messages) ? page.messages : [];
      archiveHasPrevBlock.value = !!page?.hasPrevBlock;
      archiveHasNextBlock.value = !!page?.hasNextBlock;
    } catch (e) {
      selectedArchiveId.value = previousId;
      selectedArchiveBlockId.value = previousBlockId;
      archiveBlocks.value = previousBlocks;
      archiveSummaryText.value = previousSummary;
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
      const page = await invokeTauri<ArchiveBlockPage>("get_archive_block_page", {
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
      await invokeTauri("delete_archive", { archiveId });
      options.setStatus(options.t("status.archiveDeleted"));
      if (selectedArchiveId.value === archiveId) {
        selectedArchiveId.value = "";
        selectedArchiveBlockId.value = null;
        archiveBlocks.value = [];
        archiveSummaryText.value = "";
        archiveMessages.value = [];
        archiveHasPrevBlock.value = false;
        archiveHasNextBlock.value = false;
      }
      await loadArchives();
    } catch (e) {
      options.setStatusError("status.deleteArchiveFailed", e);
    }
  }

  async function deleteUnarchivedConversation(conversationId: string): Promise<DeleteUnarchivedConversationResult | null> {
    if (!conversationId) return null;
    const summary = unarchivedConversations.value.find(
      (item) => String(item.conversationId || "").trim() === conversationId,
    );
    if (summary?.isMainConversation) {
      options.setStatus("主会话暂不支持删除。");
      return null;
    }
    try {
      console.info("[ARCHIVES] delete current unarchived conversation", { conversationId });
      const result = await invokeTauri<DeleteUnarchivedConversationResult>("delete_unarchived_conversation", {
        input: { conversationId },
      });
      options.setStatus(options.t("status.unarchivedConversationDeleted"));
      const nextConversationId = String(result?.activeConversationId || "").trim();
      if (selectedUnarchivedConversationId.value === conversationId) {
        selectedUnarchivedConversationId.value = nextConversationId;
        selectedUnarchivedBlockId.value = null;
        unarchivedBlocks.value = [];
        unarchivedMessages.value = [];
        unarchivedHasPrevBlock.value = false;
        unarchivedHasNextBlock.value = false;
      }
      await loadArchives();
      return result;
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
      return null;
    }
  }

  async function deleteRemoteImContactConversation(contactId: string) {
    if (!contactId) return;
    try {
      await invokeTauri<boolean>("remote_im_clear_contact_conversation", {
        input: { contactId },
      });
      options.setStatus("联系人会话已清空。");
      await loadRemoteImContactConversations();
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
    }
  }

  async function deleteDelegateConversation(conversationId: string): Promise<DeleteDelegateConversationResult | null> {
    if (!conversationId) return null;
    try {
      const result = await invokeTauri<DeleteDelegateConversationResult>("delete_delegate_conversation", {
        input: { conversationId },
      });
      options.setStatus("委托会话已删除。");
      if (selectedDelegateConversationId.value === conversationId) {
        selectedDelegateConversationId.value = "";
        delegateMessages.value = [];
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
      const result = await invokeTauri<ExportArchiveFileResult>("export_archive_to_file", {
        input: {
          archiveId: selectedArchiveId.value,
          format: payload.format,
        },
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
      const result = await invokeTauri<ImportArchivesResult>("import_archives_from_json", {
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

  return {
    archives,
    archiveBlocks,
    archiveMessages,
    archiveSummaryText,
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
    delegateMessages,
    selectedDelegateConversationId,
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
    selectRemoteImContactConversation,
    selectRemoteImContactConversationBlock,
    loadUnarchivedConversations,
    loadUnarchivedConversationListOnly,
    loadDelegateConversations,
    loadRemoteImContactConversations,
    loadArchives,
    selectArchive,
    selectArchiveBlock,
    deleteUnarchivedConversation,
    deleteDelegateConversation,
    deleteRemoteImContactConversation,
    deleteArchive,
    exportArchive,
    buildArchiveImportPreview,
    importArchivePayload,
  };
}
