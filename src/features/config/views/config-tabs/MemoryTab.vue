<template>
  <div class="space-y-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3">
        <div class="flex flex-wrap items-center gap-2">
          <button class="btn btn-xs" :disabled="loading" @click="refreshMemories">{{ t("common.refresh") }}</button>
          <button class="btn btn-xs" :disabled="loading" @click="exportMemories">{{ t("memory.export") }}</button>
          <button class="btn btn-xs" :disabled="loading" @click="triggerImport">{{ t("memory.import") }}</button>
          <button class="btn btn-xs" :disabled="loading" @click="rebuildIndexes">重建索引</button>
          <button class="btn btn-xs" :disabled="loading" @click="healthCheck(false)">巡检</button>
          <button class="btn btn-xs btn-warning" :disabled="loading" @click="healthCheck(true)">巡检并修复</button>
          <input ref="importInputRef" type="file" accept=".json,application/json" class="hidden" @change="handleImportFile" />
        </div>
        <div class="text-xs opacity-70 break-all">{{ opMessage }}</div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3 space-y-2">
        <div class="text-xs font-semibold">Embedding Provider 切换</div>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-2">
          <input v-model.trim="providerId" class="input input-bordered input-xs" placeholder="provider id" />
          <input v-model.trim="providerModelName" class="input input-bordered input-xs" placeholder="model name" />
          <input v-model.number="providerBatchSize" class="input input-bordered input-xs" type="number" min="1" max="512" step="1" placeholder="batch size" />
        </div>
        <div>
          <button class="btn btn-xs btn-primary" :disabled="loading || !providerId" @click="syncProvider">同步 Provider 索引</button>
        </div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3 space-y-2">
        <div class="text-xs font-semibold">数据库备份/恢复</div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
          <input v-model.trim="backupPath" class="input input-bordered input-xs" placeholder="backup path, e.g. E:\\backup\\memory_store.db" />
          <button class="btn btn-xs" :disabled="loading || !backupPath" @click="backupDb">备份数据库</button>
          <input v-model.trim="restorePath" class="input input-bordered input-xs" placeholder="restore source path" />
          <button class="btn btn-xs btn-warning" :disabled="loading || !restorePath" @click="restoreDb">恢复数据库</button>
        </div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300 min-h-[260px]">
      <div class="card-body p-3 min-h-0 flex flex-col gap-2">
        <div class="flex flex-wrap items-center gap-2 border-b border-base-300 pb-2">
          <input
            v-model.trim="searchQuery"
            class="input input-bordered input-xs w-64"
            placeholder="搜索记忆（混合检索）"
            @keyup.enter="searchMemories"
          />
          <button class="btn btn-xs btn-primary" :disabled="loading || !searchQuery" @click="searchMemories">搜索</button>
          <button class="btn btn-xs" :disabled="loading || !isSearchMode" @click="clearSearch">清空</button>
          <span v-if="loading" class="text-[11px] opacity-70">搜索中...</span>
        </div>
        <div v-if="memoryList.length === 0" class="text-xs opacity-70">{{ t("memory.empty") }}</div>
        <div v-else class="min-h-0 flex-1 overflow-auto space-y-2 pr-1">
          <div v-for="memory in pagedMemories" :key="memory.id" class="border border-base-300 rounded p-2 text-xs">
            <div class="badge badge-sm">{{ memory.memoryType }}</div>
            <div class="mt-1 whitespace-pre-wrap break-words">{{ memory.judgment }}</div>
            <div v-if="memory.reasoning" class="mt-1 opacity-80 whitespace-pre-wrap break-words">{{ memory.reasoning }}</div>
            <div class="mt-2 flex flex-wrap gap-1">
              <span v-for="(kw, idx) in memory.tags" :key="`${memory.id}-${idx}`" class="badge badge-sm badge-ghost">{{ kw }}</span>
              <span v-if="!memory.tags.length" class="opacity-60">-</span>
            </div>
            <div v-if="isSearchMode" class="mt-1 opacity-70 text-[11px]">
              bm25={{ (memory.bm25Score ?? 0).toFixed(3) }},
              vector={{ (memory.vectorScore ?? 0).toFixed(3) }},
              final={{ (memory.finalScore ?? 0).toFixed(3) }}
            </div>
            <div class="mt-1 opacity-60 text-[11px]">{{ memory.updatedAt || memory.createdAt }}</div>
          </div>
        </div>
        <div class="flex items-center justify-between border-t border-base-300 pt-2">
          <span class="text-xs opacity-70">{{ t("memory.page", { page: memoryPage, total: memoryPageCount }) }}</span>
          <div class="join">
            <button class="btn btn-xs join-item" :disabled="memoryPage <= 1" @click="memoryPage--">{{ t("memory.prevPage") }}</button>
            <button class="btn btn-xs join-item" :disabled="memoryPage >= memoryPageCount" @click="memoryPage++">{{ t("memory.nextPage") }}</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";

type MemoryEntry = {
  id: string;
  memoryType: "knowledge" | "skill" | "emotion" | "event";
  judgment: string;
  reasoning: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  bm25Score?: number;
  vectorScore?: number;
  finalScore?: number;
};

const { t } = useI18n();
const MEMORY_PAGE_SIZE = 10;
const loading = ref(false);
const opMessage = ref("");
const memoryList = ref<MemoryEntry[]>([]);
const memoryPage = ref(1);
const searchQuery = ref("");
const isSearchMode = ref(false);
const providerId = ref("default_provider");
const providerModelName = ref("embedding-model");
const providerBatchSize = ref(64);
const backupPath = ref("");
const restorePath = ref("");
const importInputRef = ref<HTMLInputElement | null>(null);

const sortedMemories = computed(() => {
  if (isSearchMode.value) {
    return [...memoryList.value].sort((a, b) => (b.finalScore ?? 0) - (a.finalScore ?? 0));
  }
  return [...memoryList.value].sort((a, b) => {
    const ta = Date.parse(a.updatedAt || a.createdAt || "");
    const tb = Date.parse(b.updatedAt || b.createdAt || "");
    if (Number.isFinite(ta) && Number.isFinite(tb)) return tb - ta;
    return (b.updatedAt || b.createdAt || "").localeCompare(a.updatedAt || a.createdAt || "");
  });
});

const memoryPageCount = computed(() => Math.max(1, Math.ceil(sortedMemories.value.length / MEMORY_PAGE_SIZE)));

const pagedMemories = computed(() => {
  const page = Math.max(1, Math.min(memoryPage.value, memoryPageCount.value));
  const start = (page - 1) * MEMORY_PAGE_SIZE;
  return sortedMemories.value.slice(start, start + MEMORY_PAGE_SIZE);
});

async function withLoading<T>(fn: () => Promise<T>): Promise<T | null> {
  loading.value = true;
  try {
    return await fn();
  } catch (err) {
    opMessage.value = `Error: ${String(err)}`;
    return null;
  } finally {
    loading.value = false;
  }
}

async function refreshMemories() {
  const result = await withLoading(() => invokeTauri<MemoryEntry[]>("list_memories"));
  if (!result) return;
  memoryList.value = result;
  memoryPage.value = 1;
  isSearchMode.value = false;
  opMessage.value = `Loaded ${result.length} memories.`;
}

async function searchMemories() {
  const query = searchQuery.value.trim();
  if (!query) {
    await clearSearch();
    return;
  }
  const result = await withLoading(() =>
    invokeTauri<{
      memories: Array<{
        memory: MemoryEntry;
        bm25Score: number;
        vectorScore: number;
        finalScore: number;
      }>;
      elapsedMs: number;
    }>("search_memories_mixed", { input: { query } }),
  );
  if (!result) return;
  memoryList.value = result.memories.map((hit) => ({
    ...hit.memory,
    bm25Score: hit.bm25Score,
    vectorScore: hit.vectorScore,
    finalScore: hit.finalScore,
  }));
  memoryPage.value = 1;
  isSearchMode.value = true;
  opMessage.value = `Search done: ${result.memories.length} hit(s), ${result.elapsedMs} ms.`;
}

async function clearSearch() {
  searchQuery.value = "";
  await refreshMemories();
}

async function exportMemories() {
  const result = await withLoading(() => invokeTauri<{ path: string; count: number }>("export_memories_to_file"));
  if (!result) return;
  opMessage.value = `Exported ${result.count} memories to ${result.path}`;
}

function triggerImport() {
  if (!importInputRef.value) return;
  importInputRef.value.value = "";
  importInputRef.value.click();
}

async function handleImportFile(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  await withLoading(async () => {
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
    const result = await invokeTauri<{ importedCount: number; createdCount: number; mergedCount: number; totalCount: number }>(
      "import_memories",
      { input: { memories } },
    );
    await refreshMemories();
    opMessage.value = `Import done: created=${result.createdCount}, merged=${result.mergedCount}, total=${result.totalCount}`;
  });
}

async function rebuildIndexes() {
  const result = await withLoading(() => invokeTauri<{ memoryRows: number; memoryFtsRows: number; noteRows: number; noteFtsRows: number }>("memory_rebuild_indexes"));
  if (!result) return;
  opMessage.value = `Rebuild done: memory ${result.memoryRows}/${result.memoryFtsRows}, note ${result.noteRows}/${result.noteFtsRows}`;
}

async function healthCheck(autoRepair: boolean) {
  const result = await withLoading(() =>
    invokeTauri<{ status: string; repaired: boolean; memoryRows: number; memoryFtsRows: number; noteRows: number; noteFtsRows: number }>(
      "memory_health_check",
      { input: { autoRepair } },
    ),
  );
  if (!result) return;
  opMessage.value = `Health: status=${result.status}, repaired=${result.repaired}, memory ${result.memoryRows}/${result.memoryFtsRows}, note ${result.noteRows}/${result.noteFtsRows}`;
}

async function syncProvider() {
  const result = await withLoading(() =>
    invokeTauri<{ status: string; oldProviderId?: string; newProviderId: string; deleted: number; added: number; batchCount: number }>(
      "sync_memory_embedding_provider",
      {
        input: {
          providerId: providerId.value,
          modelName: providerModelName.value || undefined,
          batchSize: Math.max(1, Math.min(512, Number(providerBatchSize.value || 64))),
        },
      },
    ),
  );
  if (!result) return;
  opMessage.value = `Provider sync ${result.status}: old=${result.oldProviderId || "-"}, new=${result.newProviderId}, add=${result.added}, del=${result.deleted}, batches=${result.batchCount}`;
}

async function backupDb() {
  const result = await withLoading(() => invokeTauri<{ path: string; bytes: number }>("memory_backup_db", { input: { path: backupPath.value } }));
  if (!result) return;
  opMessage.value = `Backup done: ${result.path} (${result.bytes} bytes)`;
}

async function restoreDb() {
  const result = await withLoading(() => invokeTauri<{ path: string; bytes: number }>("memory_restore_db", { input: { path: restorePath.value } }));
  if (!result) return;
  await refreshMemories();
  opMessage.value = `Restore done: ${result.path} (${result.bytes} bytes)`;
}

onMounted(() => {
  void refreshMemories();
});
</script>
