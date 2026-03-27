import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type {
  ArchiveSummary,
  ChatMessage,
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
  const archiveMessages = ref<ChatMessage[]>([]);
  const archiveSummaryText = ref("");
  const selectedArchiveId = ref("");
  const unarchivedConversations = ref<UnarchivedConversationSummary[]>([]);
  const unarchivedMessages = ref<ChatMessage[]>([]);
  const selectedUnarchivedConversationId = ref("");
  const delegateConversations = ref<DelegateConversationSummary[]>([]);
  const delegateMessages = ref<ChatMessage[]>([]);
  const selectedDelegateConversationId = ref("");
  const remoteImContactConversations = ref<RemoteImContactConversationSummary[]>([]);
  const remoteImContactMessages = ref<ChatMessage[]>([]);
  const selectedRemoteImContactId = ref("");

  async function selectUnarchivedConversation(conversationId: string) {
    const previousId = selectedUnarchivedConversationId.value;
    const previousMessages = unarchivedMessages.value;
    try {
      const messages = await invokeTauri<ChatMessage[]>("get_unarchived_conversation_messages", {
        input: { conversationId },
      });
      selectedUnarchivedConversationId.value = conversationId;
      unarchivedMessages.value = messages;
    } catch (e) {
      selectedUnarchivedConversationId.value = previousId;
      unarchivedMessages.value = previousMessages;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadUnarchivedConversations() {
    try {
      unarchivedConversations.value = await invokeTauri<UnarchivedConversationSummary[]>("list_unarchived_conversations");
      if (unarchivedConversations.value.length === 0) {
        selectedUnarchivedConversationId.value = "";
        unarchivedMessages.value = [];
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
    await loadUnarchivedConversations();
    await loadDelegateConversations();
    await loadRemoteImContactConversations();
    try {
      archives.value = await invokeTauri<ArchiveSummary[]>("list_archives");
      if (archives.value.length === 0) {
        selectedArchiveId.value = "";
        archiveMessages.value = [];
        archiveSummaryText.value = "";
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
    const previousMessages = remoteImContactMessages.value;
    try {
      const messages = await invokeTauri<ChatMessage[]>("remote_im_get_contact_conversation_messages", {
        input: { contactId },
      });
      selectedRemoteImContactId.value = contactId;
      remoteImContactMessages.value = messages;
    } catch (e) {
      selectedRemoteImContactId.value = previousId;
      remoteImContactMessages.value = previousMessages;
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function loadRemoteImContactConversations() {
    try {
      remoteImContactConversations.value =
        await invokeTauri<RemoteImContactConversationSummary[]>("remote_im_list_contact_conversations");
      if (remoteImContactConversations.value.length === 0) {
        selectedRemoteImContactId.value = "";
        remoteImContactMessages.value = [];
        return;
      }
      const targetId = remoteImContactConversations.value.some((item) => item.contactId === selectedRemoteImContactId.value)
        ? selectedRemoteImContactId.value
        : remoteImContactConversations.value[0].contactId;
      await selectRemoteImContactConversation(targetId);
    } catch (e) {
      options.setStatusError("status.loadMessagesFailed", e);
    }
  }

  async function selectArchive(archiveId: string) {
    const previousId = selectedArchiveId.value;
    const previousMessages = archiveMessages.value;
    const previousSummary = archiveSummaryText.value;
    try {
      const summary = await invokeTauri<string>("get_archive_summary", { archiveId });
      const messages = await invokeTauri<ChatMessage[]>("get_archive_messages", { archiveId });
      selectedArchiveId.value = archiveId;
      archiveSummaryText.value = String(summary || "").trim();
      archiveMessages.value = messages;
    } catch (e) {
      selectedArchiveId.value = previousId;
      archiveSummaryText.value = previousSummary;
      archiveMessages.value = previousMessages;
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
        archiveSummaryText.value = "";
        archiveMessages.value = [];
      }
      await loadArchives();
    } catch (e) {
      options.setStatusError("status.deleteArchiveFailed", e);
    }
  }

  async function deleteUnarchivedConversation(conversationId: string) {
    if (!conversationId) return;
    try {
      console.info("[ARCHIVES] delete current unarchived main conversation", { conversationId });
      await invokeTauri("delete_unarchived_conversation", {
        input: { conversationId },
      });
      options.setStatus(options.t("status.unarchivedConversationDeleted"));
      if (selectedUnarchivedConversationId.value === conversationId) {
        selectedUnarchivedConversationId.value = "";
        unarchivedMessages.value = [];
      }
      await loadArchives();
    } catch (e) {
      options.setStatusError("status.deleteUnarchivedConversationFailed", e);
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
    archiveMessages,
    archiveSummaryText,
    selectedArchiveId,
    unarchivedConversations,
    unarchivedMessages,
    selectedUnarchivedConversationId,
    delegateConversations,
    delegateMessages,
    selectedDelegateConversationId,
    remoteImContactConversations,
    remoteImContactMessages,
    selectedRemoteImContactId,
    selectUnarchivedConversation,
    selectDelegateConversation,
    selectRemoteImContactConversation,
    loadUnarchivedConversations,
    loadDelegateConversations,
    loadRemoteImContactConversations,
    loadArchives,
    selectArchive,
    deleteUnarchivedConversation,
    deleteRemoteImContactConversation,
    deleteArchive,
    exportArchive,
    buildArchiveImportPreview,
    importArchivePayload,
  };
}
