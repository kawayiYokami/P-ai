import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ArchiveSummary, ChatMessage } from "../../../types/app";

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
  const selectedArchiveId = ref("");

  async function loadArchives() {
    try {
      archives.value = await invokeTauri<ArchiveSummary[]>("list_archives");
      if (archives.value.length === 0) {
        selectedArchiveId.value = "";
        archiveMessages.value = [];
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

  async function selectArchive(archiveId: string) {
    const previousId = selectedArchiveId.value;
    const previousMessages = archiveMessages.value;
    try {
      const messages = await invokeTauri<ChatMessage[]>("get_archive_messages", { archiveId });
      selectedArchiveId.value = archiveId;
      archiveMessages.value = messages;
    } catch (e) {
      selectedArchiveId.value = previousId;
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
        archiveMessages.value = [];
      }
      await loadArchives();
    } catch (e) {
      options.setStatusError("status.deleteArchiveFailed", e);
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
    selectedArchiveId,
    loadArchives,
    selectArchive,
    deleteArchive,
    exportArchive,
    buildArchiveImportPreview,
    importArchivePayload,
  };
}
