import { ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ArchiveSummary, ChatMessage } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type ExportArchiveFileResult = {
  path: string;
  archiveId: string;
  format: "json" | "markdown";
};

type UseArchivesViewOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

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
    selectedArchiveId.value = archiveId;
    archiveMessages.value = await invokeTauri<ChatMessage[]>("get_archive_messages", { archiveId });
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

  return {
    archives,
    archiveMessages,
    selectedArchiveId,
    loadArchives,
    selectArchive,
    deleteArchive,
    exportArchive,
  };
}


