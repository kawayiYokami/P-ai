import { computed, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { invokeTauri } from "../../../services/tauri-api";

type MemoryEntry = {
  id: string;
  content: string;
  keywords: string[];
  createdAt: string;
  updatedAt: string;
};

type ExportMemoriesFileResult = {
  path: string;
  count: number;
};

type ImportMemoriesResult = {
  importedCount: number;
  createdCount: number;
  mergedCount: number;
  totalCount: number;
};

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseMemoryViewerOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
};

const MEMORY_PAGE_SIZE = 5;

export function useMemoryViewer(options: UseMemoryViewerOptions) {
  const memoryDialog = ref<HTMLDialogElement | null>(null);
  const memoryImportInput = ref<HTMLInputElement | null>(null);
  const memoryList = ref<MemoryEntry[]>([]);
  const memoryPage = ref(1);

  const sortedMemories = computed(() =>
    [...memoryList.value].sort((a, b) => {
      const ta = Date.parse(a.updatedAt || a.createdAt || "");
      const tb = Date.parse(b.updatedAt || b.createdAt || "");
      if (Number.isFinite(ta) && Number.isFinite(tb)) return tb - ta;
      return (b.updatedAt || b.createdAt || "").localeCompare(a.updatedAt || a.createdAt || "");
    }),
  );

  const memoryPageCount = computed(() =>
    Math.max(1, Math.ceil(sortedMemories.value.length / MEMORY_PAGE_SIZE)),
  );

  const pagedMemories = computed(() => {
    const page = Math.max(1, Math.min(memoryPage.value, memoryPageCount.value));
    const start = (page - 1) * MEMORY_PAGE_SIZE;
    return sortedMemories.value.slice(start, start + MEMORY_PAGE_SIZE);
  });

  async function openMemoryViewer() {
    try {
      memoryList.value = await invokeTauri<MemoryEntry[]>("list_memories");
      memoryPage.value = 1;
      memoryDialog.value?.showModal();
    } catch (e) {
      options.setStatusError("status.loadMemoriesFailed", e);
    }
  }

  function closeMemoryViewer() {
    memoryDialog.value?.close();
  }

  async function exportMemories() {
    try {
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `easy-call-ai-memories-${new Date().toISOString().replace(/[:.]/g, "-")}.json`,
      });
      if (!path) {
        return;
      }
      const result = await invokeTauri<ExportMemoriesFileResult>("export_memories_to_path", {
        input: { path },
      });
      options.setStatus(options.t("status.memoriesExported", { count: result.count, path: result.path }));
    } catch (e) {
      options.setStatusError("status.exportMemoriesFailed", e);
    }
  }

  function triggerMemoryImport() {
    if (memoryImportInput.value) {
      memoryImportInput.value.value = "";
      memoryImportInput.value.click();
    }
  }

  async function handleMemoryImportFile(event: Event) {
    const input = event.target as HTMLInputElement | null;
    const file = input?.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const parsed = JSON.parse(text) as unknown;
      const memories = Array.isArray(parsed)
        ? parsed
        : parsed && typeof parsed === "object" && Array.isArray((parsed as { memories?: unknown }).memories)
          ? (parsed as { memories: unknown[] }).memories
          : null;
      if (!Array.isArray(memories)) {
        throw new Error("invalid memories payload");
      }
      const result = await invokeTauri<ImportMemoriesResult>("import_memories", {
        input: { memories },
      });
      memoryList.value = await invokeTauri<MemoryEntry[]>("list_memories");
      memoryPage.value = 1;
      options.setStatus(
        options.t("status.importMemoriesDone", {
          created: result.createdCount,
          merged: result.mergedCount,
          total: result.totalCount,
        }),
      );
    } catch (e) {
      options.setStatusError("status.importMemoriesFailed", e);
    }
  }

  return {
    memoryDialog,
    memoryImportInput,
    memoryList,
    memoryPage,
    memoryPageCount,
    pagedMemories,
    openMemoryViewer,
    closeMemoryViewer,
    exportMemories,
    triggerMemoryImport,
    handleMemoryImportFile,
  };
}

