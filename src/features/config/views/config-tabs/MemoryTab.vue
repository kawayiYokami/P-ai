<template>
  <div class="space-y-3 relative">
    <!-- 同步锁定遮罩 -->
    <div
      v-if="syncLocked"
      class="absolute inset-0 z-20 flex items-start justify-center pt-8 bg-base-100/80 backdrop-blur-sm"
    >
      <div class="rounded-box border border-base-300 bg-base-100 px-6 py-4 shadow-lg flex flex-col items-center gap-3 min-w-72">
        <span class="loading loading-spinner loading-md text-primary"></span>
        <div class="text-sm font-medium">{{ t('config.memory.syncing') }}</div>
        <progress class="progress progress-primary w-full" :value="syncProgressPercent" max="100"></progress>
        <div class="text-sm opacity-70">
          {{ syncProgressText }}
        </div>
      </div>
    </div>

    <!-- 记忆配置区 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-3 space-y-3">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">{{ t('config.memory.vectorization') }}</span>
          <div class="text-sm opacity-60">{{ t('config.memory.vectorizationHint') }}</div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <!-- 嵌入配置 -->
          <div class="flex flex-col gap-2">
            <label class="flex w-full flex-col gap-1">
              <div class="flex items-center justify-between py-0"><span class="text-sm">{{ t('config.memory.embeddingModel') }}</span></div>
              <select v-model="embeddingApiConfigId" class="select select-bordered select-sm">
                <option value="">{{ t('config.memory.notConfigured') }}</option>
                <option v-for="api in embeddingApiConfigs" :key="api.id" :value="api.id">
                  {{ api.name }}
                </option>
              </select>
            </label>
            <div class="flex gap-2">
              <button class="btn btn-sm flex-1" :disabled="loading || !embeddingApiConfigId" @click="testEmbeddingProvider">
                {{ t('config.memory.testEmbedding') }}
              </button>
              <button
                class="btn btn-sm btn-primary flex-1"
                :disabled="loading || syncLocked || (!!embeddingApiConfigId && !embeddingReadyToSave)"
                @click="saveEmbeddingBinding"
              >
                {{ t('config.memory.saveAndSync') }}
              </button>
            </div>
          </div>

          <!-- 重排配置 -->
          <div class="flex flex-col gap-2">
            <label class="flex w-full flex-col gap-1">
              <div class="flex items-center justify-between py-0"><span class="text-sm">{{ t('config.memory.rerankModel') }}</span></div>
              <select v-model="rerankApiConfigId" class="select select-bordered select-sm">
                <option value="">{{ t('config.memory.notConfigured') }}</option>
                <option v-for="api in rerankApiConfigs" :key="api.id" :value="api.id">
                  {{ api.name }}
                </option>
              </select>
            </label>
            <div class="flex gap-2">
              <button class="btn btn-sm flex-1" :disabled="loading || !rerankApiConfigId" @click="testRerankProvider">
                {{ t('config.memory.testRerank') }}
              </button>
              <button
                class="btn btn-sm flex-1"
                :disabled="loading || syncLocked || (!!rerankApiConfigId && !rerankReadyToSave)"
                @click="saveRerankBinding"
              >
                {{ t('config.memory.saveRerank') }}
              </button>
            </div>
          </div>
        </div>

        <div v-if="opMessage" class="text-sm break-all rounded-box bg-base-200/50 px-2 py-1.5">
          {{ opMessage }}
        </div>
      </div>
    </div>

    <!-- 记忆列表区 -->
    <div class="card bg-base-100 min-h-[280px]">
      <div class="card-body p-3 min-h-0 flex flex-col gap-3">
        <!-- 标题 + 操作 -->
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium">{{ t('config.memory.list') }}</span>
          <div class="join">
            <button class="btn btn-sm join-item btn-ghost" :disabled="loading" @click="refreshMemories" title="刷新">
              <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
            </button>
            <button class="btn btn-sm join-item btn-ghost" :disabled="loading" @click="exportMemories" title="导出">
              <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>
            </button>
            <button class="btn btn-sm join-item btn-ghost" :disabled="loading" @click="triggerImport" title="导入">
              <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" x2="12" y1="3" y2="15"/></svg>
            </button>
          </div>
          <input ref="importInputRef" type="file" accept=".json,application/json" class="hidden" @change="handleImportFile" />
        </div>

        <!-- 搜索栏 -->
        <div class="join">
          <div class="flex-auto">
            <input
              v-model.trim="searchQuery"
              class="input input-bordered input-sm join-item w-full"
              :placeholder="t('config.memory.searchPlaceholder')"
              @keyup.enter="searchMemories"
            />
          </div>
          <div class="indicator">
            <span v-if="isSearchMode" class="indicator-item badge badge-secondary badge-sm">结果</span>
            <button
              class="btn btn-sm join-item"
              :class="searchQuery ? 'btn-primary' : 'bg-base-200'"
              :disabled="loading || !searchQuery"
              @click="searchMemories"
            >
              {{ t('config.memory.search') }}
            </button>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <button
            v-if="isSearchMode"
            class="btn btn-sm bg-base-200"
            :disabled="loading"
            @click="clearSearch"
          >
            {{ t('config.memory.clear') }}
          </button>
          <span v-if="loading" class="text-sm opacity-70">
            <span class="loading loading-spinner loading-sm"></span>
            {{ t('config.memory.searching') }}
          </span>
        </div>

        <!-- 搜索结果信息 -->
        <div v-if="isSearchMode && memoryList.length > 0" class="text-sm opacity-70 flex items-center gap-2">
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.searchResults') }}</span>
          <span>{{ t('config.memory.matchesCount', { count: memoryList.length }) }}</span>
        </div>

                <!-- 记忆列表 -->
                <div v-if="memoryList.length === 0" class="flex-1 flex items-center justify-center text-sm opacity-50">
                  {{ t("memory.empty") }}
                </div>
                <div v-else class="min-h-0 flex-1 overflow-auto gap-2 flex flex-col">
                  <div
                    v-for="memory in pagedMemories"
                    :key="memory.id"
                    class="card bg-base-200 card-border border-base-300 card-sm"
                  >
                    <div class="card-body gap-2 p-3">
                      <!-- 内容 -->
                      <div class="whitespace-pre-wrap break-words leading-relaxed text-sm font-bold">{{ memory.judgment }}</div>

                      <!-- 推理 -->
                      <div v-if="memory.reasoning" class="py-2">
                        <div class="pl-2 border-l-2 border-base-300 opacity-70 whitespace-pre-wrap break-words italic text-sm">
                          {{ memory.reasoning }}
                        </div>
                      </div>

                      <!-- 标签行：类型 + 时间 + 标签 + 删除 -->
                      <h2 class="flex items-center justify-between m-0 p-0">
                        <span class="flex flex-wrap items-center gap-2 font-semibold text-sm">
                          <span
                            class="badge badge-sm"
                            :class="memoryTypeBadgeClass(memory.memoryType)"
                          >{{ memoryTypeLabel(memory.memoryType) }}</span>
                          <span class="badge badge-sm badge-outline">
                            {{ memory.ownerAgentId ? t('config.memory.ownerPrivate', { owner: ownerAgentName(memory.ownerAgentId) }) : t('config.memory.globalTag') }}
                          </span>
                          <span class="opacity-50">{{ formatMemoryTime(memory.updatedAt || memory.createdAt) }}</span>
                          <span v-for="(kw, idx) in memory.tags" :key="`${memory.id}-${idx}`" class="badge badge-sm badge-neutral opacity-80">
                            {{ kw }}
                          </span>
                          <span v-if="!memory.tags.length" class="opacity-40 text-[11px]">{{ t('config.memory.noTags') }}</span>
                        </span>
                        <button
                          class="btn btn-sm btn-ghost btn-circle"
                          @click="deleteMemory(memory.id)"
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                        </button>
                      </h2>

                      <!-- 搜索分数 -->
                      <div v-if="isSearchMode" class="pt-2">
                        <div class="flex flex-wrap gap-3 text-[11px] opacity-60">
                          <span>BM25: {{ (memory.bm25Score ?? 0).toFixed(3) }}</span>
                          <span>向量: {{ (memory.vectorScore ?? 0).toFixed(3) }}</span>
                          <span class="text-primary font-medium">综合: {{ (memory.finalScore ?? 0).toFixed(3) }}</span>
                        </div>
                        <div class="mt-1.5 h-1.5 bg-base-300 rounded-full overflow-hidden">
                          <div
                            class="h-full bg-primary rounded-full transition-all"
                            :style="{ width: `${Math.min(100, (memory.finalScore ?? 0) * 100)}%` }"
                          ></div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
        <!-- 分页 -->
        <div v-if="memoryList.length > 0" class="flex justify-center border-t border-base-300 pt-3">
          <div class="join">
            <button class="btn btn-sm join-item bg-base-200" :disabled="memoryPage <= 1" @click="memoryPage--">
              ‹
            </button>
            <button class="btn btn-sm join-item btn-active">{{ memoryPage }} / {{ memoryPageCount }}</button>
            <button class="btn btn-sm join-item bg-base-200" :disabled="memoryPage >= memoryPageCount" @click="memoryPage++">
              ›
            </button>
          </div>
        </div>
          </div>
        </div>
      </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";

type MemoryEntry = {
  id: string;
  memoryNo?: number;
  memoryType: "knowledge" | "skill" | "emotion" | "event";
  judgment: string;
  reasoning: string;
  tags: string[];
  ownerAgentId?: string;
  createdAt: string;
  updatedAt: string;
  bm25Score?: number;
  bm25RawScore?: number;
  vectorScore?: number;
  finalScore?: number;
};

type ApiRequestFormat =
  | "openai"
  | "openai_responses"
  | "openai_tts"
  | "openai_stt"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini"
  | "gemini_embedding"
  | "anthropic";

type ApiConfigLite = {
  id: string;
  name: string;
  model?: string;
  requestFormat: ApiRequestFormat;
  enableText?: boolean;
};

type AppConfigLite = {
  apiConfigs: ApiConfigLite[];
};

type PersonaLite = {
  id: string;
  name: string;
};

const { t } = useI18n();
const props = withDefaults(defineProps<{ syncLocked?: boolean }>(), {
  syncLocked: false,
});
const emit = defineEmits<{
  (e: "sync-lock-change", value: boolean): void;
}>();
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
const personaNameMap = ref<Record<string, string>>({});
const syncProgressDone = ref(0);
const syncProgressTotal = ref(0);
const syncProgressStatus = ref("idle");
let syncTimer: ReturnType<typeof setInterval> | null = null;
const syncLocked = computed(() => !!props.syncLocked);
const syncProgressPercent = computed(() => {
  if (syncProgressTotal.value <= 0) return 5;
  return Math.max(0, Math.min(100, Math.round((syncProgressDone.value / syncProgressTotal.value) * 100)));
});
const syncProgressText = computed(() => {
  if (syncProgressTotal.value <= 0) return t('config.memory.progressStatus', { status: syncProgressStatus.value });
  return t('config.memory.progressBatch', { done: syncProgressDone.value, total: syncProgressTotal.value, percent: syncProgressPercent.value });
});

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

function memoryTypeBadgeClass(type: string): string {
  const classes: Record<string, string> = {
    knowledge: "badge-primary",
    skill: "badge-secondary",
    emotion: "badge-accent",
    event: "badge-info",
  };
  return classes[type] || "badge-ghost";
}

function memoryTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    knowledge: t('config.memory.typeKnowledge'),
    skill: t('config.memory.typeSkill'),
    emotion: t('config.memory.typeEmotion'),
    event: t('config.memory.typeEvent'),
  };
  return labels[type] || type;
}

function ownerAgentName(agentId: string): string {
  const id = String(agentId || "").trim();
  if (!id) return t('config.memory.deletedPersona');
  return personaNameMap.value[id] || t('config.memory.deletedPersona');
}

function formatMemoryTime(iso: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (isNaN(date.getTime())) return iso;

  const now = new Date();
  const diff = now.getTime() - date.getTime();
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (seconds < 60) return t('config.memory.justNow');
  if (minutes < 60) return t('config.memory.minutesAgo', { count: minutes });
  if (hours < 24) return t('config.memory.hoursAgo', { count: hours });
  if (days < 7) return t('config.memory.daysAgo', { count: days });

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");

  if (year === now.getFullYear()) {
    return `${month}-${day} ${hour}:${minute}`;
  }
  return `${year}-${month}-${day}`;
}

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

async function deleteMemory(memoryId: string) {
  const result = await withLoading(() =>
    invokeTauri<{ status: string }>("delete_memory", { input: { memoryId } }),
  );
  if (!result) return;
  await refreshMemories();
  opMessage.value = t('config.memory.memoryDeleted');
}

async function refreshMemories() {
  const result = await withLoading(() => invokeTauri<MemoryEntry[]>("list_memories"));
  if (!result) return;
  memoryList.value = result;
  memoryPage.value = 1;
  isSearchMode.value = false;
  opMessage.value = t('config.memory.loadedCount', { count: result.length });
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
        bm25RawScore: number;
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
    bm25RawScore: hit.bm25RawScore,
    vectorScore: hit.vectorScore,
    finalScore: hit.finalScore,
  }));
  memoryPage.value = 1;
  isSearchMode.value = true;
  opMessage.value = t('config.memory.searchCompleted', { count: result.memories.length });
}

async function clearSearch() {
  searchQuery.value = "";
  await refreshMemories();
}

async function exportMemories() {
  const result = await withLoading(() => invokeTauri<{ path: string; count: number }>("export_memories_to_file"));
  if (!result) return;
  opMessage.value = t('config.memory.exportedCount', { count: result.count });
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
      throw new Error("无效的记忆文件格式");
    }
    const result = await invokeTauri<{ importedCount: number; createdCount: number; mergedCount: number; totalCount: number }>(
      "import_memories",
      { input: { memories } },
    );
    await refreshMemories();
    opMessage.value = t('config.memory.importCompleted');
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
  opMessage.value = t('config.memory.embeddingTestSuccess', { provider: result.providerKind, dim: result.vectorDim, ms: result.elapsedMs });
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
  opMessage.value = t('config.memory.rerankTestSuccess', { provider: result.providerKind, count: result.resultCount, ms: result.elapsedMs });
}

async function saveEmbeddingBinding() {
  const cfg = selectedEmbeddingApiConfig.value;
  if (!cfg) {
    const result = await withLoading(() =>
      invokeTauri<{ status: string }>("save_memory_embedding_binding", {
        input: {
          apiConfigId: "",
        },
      }),
    );
    if (!result) return;
    embeddingLastPassedTestKey.value = "";
    opMessage.value = t('config.memory.embeddingClosed');
    return;
  }
  emit("sync-lock-change", true);
  syncProgressDone.value = 0;
  syncProgressTotal.value = 0;
  syncProgressStatus.value = "running";
  startSyncProgressPolling();
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
  if (result) {
    opMessage.value = t('config.memory.embeddingSyncSuccess', { added: result.added, deleted: result.deleted, batchCount: result.batchCount });
  }
  await refreshSyncProgress();
  stopSyncProgressPolling();
  emit("sync-lock-change", false);
}

async function saveRerankBinding() {
  const cfg = selectedRerankApiConfig.value;
  if (!cfg) {
    const result = await withLoading(() =>
      invokeTauri<{ status: string; rerankApiConfigId: string; modelName: string }>(
        "save_memory_rerank_binding",
        {
          input: {
            apiConfigId: "",
          },
        },
      ),
    );
    if (!result) return;
    rerankLastPassedTestKey.value = "";
    opMessage.value = t('config.memory.rerankClosed');
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
  opMessage.value = t('config.memory.rerankSaved');
}

async function loadApiConfigs() {
  const cfg = await withLoading(() => invokeTauri<AppConfigLite>("load_config"));
  if (!cfg) return;
  apiConfigs.value = Array.isArray(cfg.apiConfigs) ? cfg.apiConfigs : [];
}

async function loadPersonaNames() {
  const agents = await withLoading(() => invokeTauri<PersonaLite[]>("load_agents"));
  if (!agents) return;
  const next: Record<string, string> = {};
  for (const agent of agents) {
    const id = String(agent.id || "").trim();
    if (!id) continue;
    const name = String(agent.name || "").trim();
    next[id] = name || id;
  }
  personaNameMap.value = next;
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
  void loadPersonaNames();
  void loadApiConfigs();
  void loadBindings();
  void refreshMemories();
});

onBeforeUnmount(() => {
  stopSyncProgressPolling();
  emit("sync-lock-change", false);
});

function startSyncProgressPolling() {
  stopSyncProgressPolling();
  syncTimer = setInterval(() => {
    void refreshSyncProgress();
  }, 350);
}

function stopSyncProgressPolling() {
  if (syncTimer) {
    clearInterval(syncTimer);
    syncTimer = null;
  }
}

async function refreshSyncProgress() {
  try {
    const progress = await invokeTauri<{
      status: string;
      doneBatches: number;
      totalBatches: number;
      traceId?: string;
      error?: string;
    }>("get_memory_embedding_sync_progress");
    syncProgressStatus.value = progress.status || "idle";
    syncProgressDone.value = Math.max(0, Number(progress.doneBatches || 0));
    syncProgressTotal.value = Math.max(0, Number(progress.totalBatches || 0));
  } catch {
    // Keep UI responsive if progress endpoint is temporarily unavailable.
  }
}
</script>
