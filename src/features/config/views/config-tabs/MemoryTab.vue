<template>
  <div class="space-y-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3">
        <div class="flex flex-wrap items-center gap-2">
          <button class="btn btn-xs" :disabled="loading" @click="refreshMemories">{{ t("common.refresh") }}</button>
          <button class="btn btn-xs" :disabled="loading" @click="exportMemories">{{ t("memory.export") }}</button>
          <button class="btn btn-xs" :disabled="loading" @click="triggerImport">{{ t("memory.import") }}</button>
          <input ref="importInputRef" type="file" accept=".json,application/json" class="hidden" @change="handleImportFile" />
        </div>
        <div class="text-xs opacity-70 break-all">{{ opMessage }}</div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3 space-y-2">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
          <label class="form-control">
            <div class="label py-0"><span class="label-text text-xs">嵌入 LLM</span></div>
            <select v-model="embeddingApiConfigId" class="select select-bordered select-xs">
              <option value="">无</option>
              <option v-for="api in embeddingApiConfigs" :key="api.id" :value="api.id">
                {{ api.name }} ({{ api.requestFormat }})
              </option>
            </select>
          </label>
          <label class="form-control">
            <div class="label py-0"><span class="label-text text-xs">重排 LLM</span></div>
            <select v-model="rerankApiConfigId" class="select select-bordered select-xs">
              <option value="">无</option>
              <option v-for="api in rerankApiConfigs" :key="api.id" :value="api.id">
                {{ api.name }} ({{ api.requestFormat }})
              </option>
            </select>
          </label>
        </div>
        <div class="flex flex-wrap gap-2">
          <button class="btn btn-xs" :disabled="loading" @click="testEmbeddingProvider">测试嵌入</button>
          <button class="btn btn-xs btn-primary" :disabled="loading || !embeddingApiConfigId || !embeddingReadyToSave" @click="saveEmbeddingBinding">保存嵌入并同步</button>
          <button class="btn btn-xs" :disabled="loading" @click="testRerankProvider">测试重排</button>
          <button class="btn btn-xs btn-primary" :disabled="loading || !rerankApiConfigId || !rerankReadyToSave" @click="saveRerankBinding">保存重排</button>
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

type ApiRequestFormat =
  | "openai"
  | "openai_tts"
  | "openai_stt"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini"
  | "gemini_embedding"
  | "deepseek/kimi"
  | "anthropic";

type ApiConfigLite = {
  id: string;
  name: string;
  requestFormat: ApiRequestFormat;
  enableText?: boolean;
};

type AppConfigLite = {
  apiConfigs: ApiConfigLite[];
};

const { t } = useI18n();
const MEMORY_PAGE_SIZE = 10;
const loading = ref(false);
const opMessage = ref("");
const memoryList = ref<MemoryEntry[]>([]);
const memoryPage = ref(1);
const searchQuery = ref("");
const isSearchMode = ref(false);
const embeddingApiConfigId = ref("");
const rerankApiConfigId = ref("");
const embeddingLastPassedTestKey = ref("");
const rerankLastPassedTestKey = ref("");
const importInputRef = ref<HTMLInputElement | null>(null);
const apiConfigs = ref<ApiConfigLite[]>([]);

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
const embeddingApiConfigs = computed(() =>
  apiConfigs.value.filter((api) =>
    api.requestFormat === "openai_embedding"
    || api.requestFormat === "gemini_embedding",
  ),
);
const rerankApiConfigs = computed(() =>
  apiConfigs.value.filter((api) => api.requestFormat === "openai_rerank"),
);
const selectedEmbeddingApiConfig = computed(() =>
  embeddingApiConfigs.value.find((api) => api.id === embeddingApiConfigId.value) ?? null,
);
const selectedRerankApiConfig = computed(() =>
  rerankApiConfigs.value.find((api) => api.id === rerankApiConfigId.value) ?? null,
);
const embeddingCurrentTestKey = computed(() => {
  const cfg = selectedEmbeddingApiConfig.value;
  if (!cfg) return "";
  return `${cfg.id}|${cfg.model || ""}`;
});
const rerankCurrentTestKey = computed(() => {
  const cfg = selectedRerankApiConfig.value;
  if (!cfg) return "";
  return `${cfg.id}|${cfg.model || ""}`;
});
const embeddingReadyToSave = computed(
  () => !!embeddingCurrentTestKey.value && embeddingCurrentTestKey.value === embeddingLastPassedTestKey.value,
);
const rerankReadyToSave = computed(
  () => !!rerankCurrentTestKey.value && rerankCurrentTestKey.value === rerankLastPassedTestKey.value,
);

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

async function testEmbeddingProvider() {
  const result = await withLoading(() =>
    invokeTauri<{ providerKind: string; modelName: string; vectorDim: number; elapsedMs: number }>(
      "test_memory_embedding_provider",
      {
        input: {
          apiConfigId: embeddingApiConfigId.value || undefined,
          providerId: selectedEmbeddingApiConfig.value?.requestFormat || undefined,
          modelName: selectedEmbeddingApiConfig.value?.model || undefined,
        },
      },
    ),
  );
  if (!result) return;
  embeddingLastPassedTestKey.value = embeddingCurrentTestKey.value;
  opMessage.value = `嵌入测试成功: kind=${result.providerKind}, model=${result.modelName}, dim=${result.vectorDim}, ${result.elapsedMs}ms`;
}

async function testRerankProvider() {
  const result = await withLoading(() =>
    invokeTauri<{ providerKind: string; modelName: string; elapsedMs: number; resultCount: number; topIndex?: number; topScore?: number }>(
      "test_memory_rerank_provider",
      {
        input: {
          apiConfigId: rerankApiConfigId.value || undefined,
          modelName: selectedRerankApiConfig.value?.model || undefined,
        },
      },
    ),
  );
  if (!result) return;
  rerankLastPassedTestKey.value = rerankCurrentTestKey.value;
  opMessage.value = `重排测试成功: kind=${result.providerKind}, model=${result.modelName}, count=${result.resultCount}, top=(${result.topIndex ?? "-"}, ${(result.topScore ?? 0).toFixed(4)}), ${result.elapsedMs}ms`;
}

async function saveEmbeddingBinding() {
  const cfg = selectedEmbeddingApiConfig.value;
  if (!cfg) {
    opMessage.value = "请先选择嵌入 LLM。";
    return;
  }
  const result = await withLoading(() =>
    invokeTauri<{ status: string; oldProviderId?: string; newProviderId: string; deleted: number; added: number; batchCount: number }>(
      "save_memory_embedding_binding",
      {
        input: {
          apiConfigId: cfg.id,
          modelName: cfg.model || undefined,
          batchSize: 64,
        },
      },
    ),
  );
  if (!result) return;
  opMessage.value = `嵌入保存并同步成功: old=${result.oldProviderId || "-"}, new=${result.newProviderId}, add=${result.added}, del=${result.deleted}, batches=${result.batchCount}`;
}

async function saveRerankBinding() {
  const cfg = selectedRerankApiConfig.value;
  if (!cfg) {
    opMessage.value = "请先选择重排 LLM。";
    return;
  }
  const result = await withLoading(() =>
    invokeTauri<{ status: string; rerankApiConfigId: string; modelName: string }>(
      "save_memory_rerank_binding",
      {
        input: {
          apiConfigId: cfg.id,
          modelName: cfg.model || undefined,
        },
      },
    ),
  );
  if (!result) return;
  opMessage.value = `重排保存成功: id=${result.rerankApiConfigId}, model=${result.modelName}`;
}

async function loadApiConfigs() {
  const cfg = await withLoading(() => invokeTauri<AppConfigLite>("load_config"));
  if (!cfg) return;
  apiConfigs.value = Array.isArray(cfg.apiConfigs) ? cfg.apiConfigs : [];
}

async function loadBindings() {
  const result = await withLoading(() =>
    invokeTauri<{ embeddingApiConfigId?: string; rerankApiConfigId?: string }>("get_memory_provider_bindings"),
  );
  if (!result) return;
  embeddingApiConfigId.value = result.embeddingApiConfigId || "";
  rerankApiConfigId.value = result.rerankApiConfigId || "";
}

onMounted(() => {
  void loadApiConfigs();
  void loadBindings();
  void refreshMemories();
});
</script>
