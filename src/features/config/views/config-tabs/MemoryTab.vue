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

    <ConfigTemplate :model-value="{}" :groups="memoryTemplateGroups">
      <template #group-actions-vectorization>
        <span class="text-xs opacity-60">{{ t('config.memory.vectorizationHint') }}</span>
      </template>

      <template #row-embedding>
        <div class="grid gap-2">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-sm">{{ t('config.memory.embeddingModel') }}</span>
            <div class="flex flex-wrap justify-end gap-2">
              <button class="btn btn-sm" :disabled="embeddingTesting || !embeddingApiConfigId" @click="testEmbeddingProvider">
                <span v-if="embeddingTesting" class="loading loading-spinner loading-xs"></span>
                {{ embeddingTesting ? t('config.memory.testing') : t('config.memory.testEmbedding') }}
              </button>
              <button
                class="btn btn-sm btn-primary"
                :disabled="loading || syncLocked || (!!embeddingApiConfigId && !embeddingReadyToSave)"
                @click="saveEmbeddingBinding"
              >
                {{ t('config.memory.saveAndSync') }}
              </button>
            </div>
          </div>
          <select v-model="embeddingApiConfigId" class="select select-bordered select-sm w-full" :disabled="syncLocked">
            <option value="">{{ t('config.memory.notConfigured') }}</option>
            <option v-for="api in embeddingApiConfigs" :key="api.id" :value="api.id">
              {{ api.name }}
            </option>
          </select>
        </div>
      </template>

      <template #row-rerank>
        <div class="grid gap-2">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-sm">{{ t('config.memory.rerankModel') }}</span>
            <div class="flex flex-wrap justify-end gap-2">
              <button class="btn btn-sm" :disabled="rerankTesting || !rerankApiConfigId" @click="testRerankProvider">
                <span v-if="rerankTesting" class="loading loading-spinner loading-xs"></span>
                {{ rerankTesting ? t('config.memory.testing') : t('config.memory.testRerank') }}
              </button>
              <button
                class="btn btn-sm"
                :disabled="loading || syncLocked || (!!rerankApiConfigId && !rerankReadyToSave)"
                @click="saveRerankBinding"
              >
                {{ t('config.memory.saveRerank') }}
              </button>
            </div>
          </div>
          <select v-model="rerankApiConfigId" class="select select-bordered select-sm w-full" :disabled="syncLocked">
            <option value="">{{ t('config.memory.notConfigured') }}</option>
            <option v-for="api in rerankApiConfigs" :key="api.id" :value="api.id">
              {{ api.name }}
            </option>
          </select>
        </div>
      </template>

      <template #group-actions-chat-history>
        <div v-if="chatHistoryStats" class="flex flex-wrap items-center justify-end gap-2 text-xs opacity-70">
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeAll') }} {{ chatHistoryStats.totalSlices }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeOrphan') }} {{ chatHistoryStats.skippedOrphanConversations }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeLocal') }} {{ chatHistoryStats.localConversationSlices }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeArchive') }} {{ chatHistoryStats.archiveSlices }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeContact') }} {{ chatHistoryStats.contactSlices }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeIndex') }} {{ formatBytes(chatHistoryStats.indexStorageBytes) }}</span>
          <span class="badge badge-sm badge-ghost">{{ t('config.memory.badgeCache') }} {{ formatBytes(chatHistoryStats.cachedSliceBytes) }}</span>
        </div>
      </template>

      <template #row-chat-history>
        <div class="grid min-w-0 gap-3">
          <div class="grid grid-cols-1 gap-2 md:grid-cols-[180px_minmax(0,1fr)_auto]">
            <select v-model="chatHistoryAgentId" class="select select-bordered select-sm" :disabled="chatHistoryLoading">
              <option value="">{{ t('config.memory.filterPersonaOptional') }}</option>
              <option v-for="agent in personaOptions" :key="agent.id" :value="agent.id">
                {{ agent.name || agent.id }}
              </option>
            </select>
            <input
              v-model.trim="chatHistoryQuery"
              class="input input-bordered input-sm"
              :placeholder="t('config.memory.chatHistorySearchPlaceholder')"
              :disabled="chatHistoryLoading"
              @keyup.enter="searchChatHistory"
            />
            <button
              class="btn btn-sm btn-primary"
              :disabled="chatHistoryLoading || !chatHistoryQuery"
              @click="searchChatHistory"
            >
              <span v-if="chatHistoryLoading" class="loading loading-spinner loading-xs"></span>
              {{ chatHistoryLoading ? t('config.memory.searching') : t('config.memory.searchChatHistory') }}
            </button>
          </div>

          <div v-if="chatHistoryMessage" class="text-xs opacity-70">
            {{ chatHistoryMessage }}
          </div>

          <div v-if="chatHistoryHits.length > 0" class="max-h-96 overflow-auto">
            <div class="flex flex-col gap-2">
              <div
                v-for="hit in chatHistoryHits"
                :key="hit.slice.id"
                class="rounded-box border border-base-300 bg-base-200 p-3"
              >
                <div class="mb-2 flex flex-wrap items-center gap-2 text-xs opacity-70">
                  <span class="badge badge-sm badge-outline">{{ chatHistorySourceLabel(hit.slice.sourceKind) }}</span>
                  <span>{{ hit.slice.sourceTitle || hit.slice.sourceId }}</span>
                  <span>{{ formatMemoryTime(hit.slice.timeStart || hit.slice.timeEnd) }}</span>
                  <span>BM25 {{ hit.bm25NormalizedScore.toFixed(3) }}</span>
                </div>
                <div class="whitespace-pre-wrap wrap-break-word text-sm leading-relaxed">
                  {{ hit.slice.content }}
                </div>
                <div class="mt-2 flex flex-wrap gap-1 text-xs opacity-70">
                  <span v-for="speaker in hit.slice.speakers" :key="`${hit.slice.id}-${speaker}`" class="badge badge-sm badge-ghost">
                    {{ speaker }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #row-recall-diagnosis>
        <div class="grid min-w-0 gap-3">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <span class="text-xs opacity-60">{{ t('config.memory.recallDiagnosisHint') }}</span>
            <div class="join">
              <button
                class="btn btn-sm join-item"
                :class="recallMode === 'tool' ? 'btn-primary' : 'btn-ghost'"
                @click="recallMode = 'tool'"
              >
                {{ t('config.memory.recallModeTool') }}
              </button>
              <button
                class="btn btn-sm join-item"
                :class="recallMode === 'rag' ? 'btn-primary' : 'btn-ghost'"
                @click="recallMode = 'rag'"
              >
                {{ t('config.memory.recallModeRag') }}
              </button>
            </div>
          </div>

          <div class="grid grid-cols-1 gap-2 md:grid-cols-[180px_minmax(0,1fr)_auto]">
            <select v-model="recallAgentId" class="select select-bordered select-sm" :disabled="recallLoading">
              <option value="">{{ t('config.memory.selectPersona') }}</option>
              <option v-for="agent in personaOptions" :key="agent.id" :value="agent.id">
                {{ agent.name || agent.id }}
              </option>
            </select>
            <input
              v-model.trim="recallQuery"
              class="input input-bordered input-sm"
              :placeholder="t('config.memory.recallQueryPlaceholder')"
              :disabled="recallLoading"
              @keyup.enter="searchRecall"
            />
            <button
              class="btn btn-sm btn-primary"
              :disabled="recallLoading || !recallAgentId || !recallQuery"
              @click="searchRecall"
            >
              <span v-if="recallLoading" class="loading loading-spinner loading-xs"></span>
              {{ recallLoading ? t('config.memory.searching') : t('config.memory.recallSearch') }}
            </button>
          </div>

          <div v-if="recallMode === 'tool'" class="grid grid-cols-1 gap-2 md:grid-cols-[180px_minmax(0,1fr)_auto]">
            <div class="text-xs opacity-60 md:text-right md:pt-2">{{ t('config.memory.recallTimeLabel') }}</div>
            <input
              v-model.trim="recallTime"
              class="input input-bordered input-sm"
              :placeholder="t('config.memory.recallTimePlaceholder')"
              :disabled="recallLoading"
            />
            <button class="btn btn-sm bg-base-200" :disabled="recallLoading" @click="clearRecall">
              {{ t('config.memory.clear') }}
            </button>
          </div>

          <div v-if="recallMessage" class="text-xs opacity-70">
            {{ recallMessage }}
          </div>

          <div v-if="recallResults.length > 0" class="max-h-96 overflow-auto">
            <div class="flex flex-col gap-2">
              <div
                v-for="(memory, index) in recallResults"
                :key="memory.id"
                class="rounded-box border border-base-300 bg-base-200 p-3"
              >
                <div class="mb-1 flex flex-wrap items-center gap-2 text-xs opacity-70">
                  <span class="badge badge-sm badge-outline">#{{ index + 1 }}</span>
                  <span v-if="memory.ownerAgentId" class="badge badge-sm badge-ghost">{{ ownerAgentName(memory.ownerAgentId) }}</span>
                  <span class="badge badge-sm badge-ghost">{{ memory.memoryType }}</span>
                  <span>BM25 {{ (memory.bm25Score ?? 0).toFixed(3) }}</span>
                  <span>{{ t('config.memory.vectorScoreLabel') }} {{ (memory.vectorScore ?? 0).toFixed(3) }}</span>
                  <span>{{ t('config.memory.rerankScoreLabel') }} {{ (memory.rerankScore ?? 0).toFixed(3) }}</span>
                  <span class="text-primary font-medium">{{ t('config.memory.finalScoreLabel') }} {{ (memory.finalScore ?? 0).toFixed(3) }}</span>
                </div>
                <div class="whitespace-pre-wrap wrap-break-word text-sm leading-relaxed">
                  {{ memory.judgment }}
                </div>
                <div v-if="memory.reasoning" class="mt-1 text-xs opacity-60 whitespace-pre-wrap wrap-break-word">
                  {{ memory.reasoning }}
                </div>
                <div v-if="memory.tags && memory.tags.length > 0" class="mt-2 flex flex-wrap gap-1">
                  <span v-for="tag in memory.tags" :key="tag" class="badge badge-sm badge-ghost">{{ tag }}</span>
                </div>
                <progress
                  class="progress progress-primary mt-2 h-1"
                  :value="Math.min(100, (memory.finalScore ?? 0) * 100)"
                  max="100"
                ></progress>
              </div>
            </div>
          </div>
        </div>
      </template>

      <template #group-actions-memory-list>
        <div class="join">
          <button class="btn btn-sm join-item btn-ghost" :disabled="loading" @click="refreshMemories" :title="t('sidebar.memoryRefresh')">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
          </button>
          <MemoryExportCard class="join-item" @exported="handleMemoryExportDone" />
          <MemoryImportCard class="join-item" @imported="handleMemoryImportDone" />
        </div>
      </template>

      <template #row-memory-list>
        <div class="min-h-70 min-h-0 flex flex-col gap-3">
          <div class="sticky top-0 z-10 space-y-2 bg-base-100/95 py-2 backdrop-blur">
          <div class="flex items-center gap-3">
            <button
              class="btn btn-sm bg-base-200 hover:bg-base-300 shrink-0"
              :disabled="loading"
              :title="memoryViewMode === 'detailed' ? t('sidebar.memoryToggleView') : t('sidebar.memoryToggleViewDetailed')"
              @click="toggleMemoryViewMode"
            >
              <LayoutList v-if="memoryViewMode === 'detailed'" class="h-3.5 w-3.5" />
              <List v-else class="h-3.5 w-3.5" />
            </button>
            <button
              class="btn btn-sm bg-base-200 hover:bg-base-300 shrink-0"
              :class="{ 'text-primary': filterPanelOpen }"
              :disabled="loading"
              :title="t('sidebar.memoryFilter')"
              @click="toggleFilterPanel"
            >
              <SlidersHorizontal class="h-3.5 w-3.5" />
            </button>
            <div class="join min-w-0 flex-1">
              <div class="flex-auto min-w-0">
                <input
                  v-model.trim="searchQuery"
                  class="input input-bordered input-sm join-item w-full"
                  :placeholder="t('config.memory.searchPlaceholder')"
                  @keyup.enter="searchMemories"
                />
              </div>
              <div class="indicator">
                <span v-if="isSearchMode" class="indicator-item badge badge-secondary badge-sm">{{ t('sidebar.memoryResultBadge') }}</span>
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
            <div v-if="sortedMemories.length > 0" class="flex shrink-0 items-center gap-2">
              <button
                class="btn btn-sm bg-base-200 hover:bg-base-300"
                :disabled="memoryPage <= 1"
                @click="memoryPage--"
              >
                ‹
              </button>
              <div class="min-w-0 text-center text-sm font-medium tabular-nums">
                {{ memoryPage }} / {{ memoryPageCount }}
              </div>
              <button
                class="btn btn-sm bg-base-200 hover:bg-base-300"
                :disabled="memoryPage >= memoryPageCount"
                @click="memoryPage++"
              >
                ›
              </button>
            </div>
          </div>
          <div v-if="filterPanelOpen" class="flex flex-wrap items-center gap-2">
            <select
              v-model="selectedOwnerFilter"
              class="select select-bordered select-sm w-32"
              @change="resetMemoryPage"
            >
              <option value="">{{ t('sidebar.memoryAllPersonas') }}</option>
              <option value="__global__">{{ t('sidebar.memoryGlobal') }}</option>
              <option v-for="option in ownerFilterOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
            <select
              v-model="selectedProfileTagFilter"
              class="select select-bordered select-sm w-32"
              @change="resetMemoryPage"
            >
              <option value="">{{ t('sidebar.memoryProfileAttribute') }}</option>
              <option v-for="tag in PROFILE_ATTRIBUTE_TAGS" :key="tag" :value="tag">
                {{ tag }}
              </option>
            </select>
            <select
              v-model="selectedMemoryTypeFilter"
              class="select select-bordered select-sm w-32"
              @change="resetMemoryPage"
            >
              <option value="">{{ t('sidebar.memoryType') }}</option>
              <option v-for="option in memoryTypeFilterOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
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
            <div v-if="isSearchMode && sortedMemories.length > 0" class="text-sm opacity-70 flex items-center gap-2">
              <span class="badge badge-sm badge-ghost">{{ t('config.memory.searchResults') }}</span>
              <span>{{ t('config.memory.matchesCount', { count: sortedMemories.length }) }}</span>
            </div>
          </div>
        </div>

                <!-- 记忆列表 -->
                <div v-if="sortedMemories.length === 0" class="flex-1 flex items-center justify-center text-sm opacity-50">
                  {{ t("memory.empty") }}
                </div>
                <div v-else class="min-h-0 flex-1 overflow-auto">
                  <div class="flex flex-col gap-2">
                    <div
                      v-for="memory in pagedMemories"
                      :key="memory.id"
                      class="card bg-base-200 card-border border-base-300 card-sm"
                    >
                      <div class="card-body gap-2 p-3">
                        <!-- 内容 -->
                        <div
                          class="whitespace-pre-wrap wrap-break-word leading-relaxed"
                          :class="memoryViewMode === 'compact' ? 'text-xs font-normal' : 'text-sm font-bold'"
                        >
                          {{ memory.judgment }}
                        </div>

                        <!-- 推理 -->
                        <div v-if="showMemoryReasoning && memory.reasoning" class="py-2">
                          <div class="pl-2 border-l-2 border-base-300 opacity-70 whitespace-pre-wrap wrap-break-word italic text-sm">
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
                            <span class="opacity-50">{{ formatMemoryTime(memory.createdAt || memory.updatedAt) }}</span>
                            <span
                              v-for="(kw, idx) in showMemoryTags ? memory.tags : []"
                              :key="`${memory.id}-${idx}`"
                              class="badge badge-sm badge-neutral opacity-80"
                            >
                              {{ kw }}
                            </span>
                            <span v-if="showMemoryTags && !memory.tags.length" class="opacity-40 text-xs">{{ t('config.memory.noTags') }}</span>
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
                          <div class="flex flex-wrap gap-3 text-xs opacity-60">
                            <span>BM25: {{ (memory.bm25Score ?? 0).toFixed(3) }}</span>
                            <span>{{ t('sidebar.memoryVectorScore') }}: {{ (memory.vectorScore ?? 0).toFixed(3) }}</span>
                            <span>{{ t('config.memory.rerankScoreLabel') }}: {{ (memory.rerankScore ?? 0).toFixed(3) }}</span>
                            <span class="text-primary font-medium">{{ t('sidebar.memoryCompositeScore') }}: {{ (memory.finalScore ?? 0).toFixed(3) }}</span>
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
                </div>
        </div>
      </template>
    </ConfigTemplate>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { LayoutList, List, SlidersHorizontal } from "@lucide/vue";
import { invokeTauri } from "../../../../services/tauri-api";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import MemoryExportCard from "../../components/MemoryExportCard.vue";
import MemoryImportCard from "../../components/MemoryImportCard.vue";

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
  rerankScore?: number;
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

type ChatHistorySlice = {
  id: string;
  sourceKind: string;
  sourceId: string;
  sourceTitle: string;
  segmentId: string;
  sliceIndex: number;
  content: string;
  speakers: string[];
  ownerAgentId: string;
  timeStart: string;
  timeEnd: string;
  messageStartId: string;
  messageEndId: string;
};

type ChatHistorySearchHit = {
  slice: ChatHistorySlice;
  bm25Score: number;
  bm25NormalizedScore: number;
};

type ChatHistorySearchStats = {
  totalSlices: number;
  indexStorageBytes: number;
  cachedSliceBytes: number;
  indexedConversations: number;
  skippedDelegateConversations: number;
  skippedLiveBlocks: number;
  skippedOrphanConversations: number;
  localConversationSlices: number;
  archiveSlices: number;
  contactSlices: number;
};

const { t } = useI18n();
const props = withDefaults(defineProps<{
  syncLocked?: boolean;
  setStatusAction?: (text: string, tone?: "default" | "success" | "error") => void;
}>(), {
  syncLocked: false,
});

// 反馈统一走配置窗口的全局 toast（ConfigStatusToast），不在页面里堆状态行
function notify(text: string, tone: "default" | "success" | "error" = "default") {
  props.setStatusAction?.(text, tone);
}
const emit = defineEmits<{
  (e: "sync-lock-change", value: boolean): void;
}>();
const MEMORY_VIEW_MODE_STORAGE_KEY = "easy_call.memory.view_mode.v1";
const SHOW_DEV_MEMORY_DIAGNOSIS = import.meta.env.DEV;
const DETAILED_MEMORY_PAGE_SIZE = 10;
const COMPACT_MEMORY_PAGE_SIZE = 20;
const PROFILE_ATTRIBUTE_TAGS = ["用户别名", "事实属性", "技能树", "关系图谱", "活跃项目", "用户要求"] as const;
const loading = ref(false);
const memoryList = ref<MemoryEntry[]>([]);
const memoryPage = ref(1);
const memoryViewMode = ref<"detailed" | "compact">("detailed");
const filterPanelOpen = ref(false);
const selectedOwnerFilter = ref("");
const selectedProfileTagFilter = ref("");
const selectedMemoryTypeFilter = ref("");
const searchQuery = ref("");
const isSearchMode = ref(false);
const embeddingApiConfigId = ref("");
const rerankApiConfigId = ref("");
const embeddingLastPassedTestKey = ref("");
const rerankLastPassedTestKey = ref("");
// 两个测试各自独立的进行中状态，避免互相干扰、也不占用全局 loading
const embeddingTesting = ref(false);
const rerankTesting = ref(false);
const apiConfigs = ref<ApiConfigLite[]>([]);
const personaOptions = ref<PersonaLite[]>([]);
const personaNameMap = ref<Record<string, string>>({});
const chatHistoryAgentId = ref("");
const chatHistoryQuery = ref("");
const chatHistoryHits = ref<ChatHistorySearchHit[]>([]);
const chatHistoryStats = ref<ChatHistorySearchStats | null>(null);
const chatHistoryMessage = ref("");
const chatHistoryLoading = ref(false);
// 召回诊断（工具召回 / RAG 召换双模式，复刻对话真实召回链路）
type RecallDiagnosisMemory = MemoryEntry & {
  bm25Score: number;
  bm25RawScore: number;
  vectorScore: number;
  rerankScore: number;
  finalScore: number;
};
const recallMode = ref<"rag" | "tool">("rag");
const recallAgentId = ref("");
const recallQuery = ref("");
const recallTime = ref("");
const recallResults = ref<RecallDiagnosisMemory[]>([]);
const recallLoading = ref(false);
const recallMessage = ref("");
const recallMeta = ref<{ total: number; offset: number; limit: number } | null>(null);
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
const memoryPageSize = computed(() => (
  memoryViewMode.value === "compact" ? COMPACT_MEMORY_PAGE_SIZE : DETAILED_MEMORY_PAGE_SIZE
));
const showMemoryReasoning = computed(() => memoryViewMode.value === "detailed");
const showMemoryTags = computed(() => memoryViewMode.value === "detailed");
const ownerFilterOptions = computed(() => {
  const seen = new Set<string>();
  const options: Array<{ value: string; label: string }> = [];
  for (const memory of memoryList.value) {
    const ownerId = String(memory.ownerAgentId || "").trim();
    if (!ownerId || seen.has(ownerId)) continue;
    seen.add(ownerId);
    options.push({
      value: ownerId,
      label: ownerAgentName(ownerId),
    });
  }
  return options.sort((a, b) => a.label.localeCompare(b.label, "zh-CN"));
});
const memoryTypeFilterOptions = computed(() => ([
  { value: "knowledge", label: memoryTypeLabel("knowledge") },
  { value: "skill", label: memoryTypeLabel("skill") },
  { value: "emotion", label: memoryTypeLabel("emotion") },
  { value: "event", label: memoryTypeLabel("event") },
]));
const filteredMemories = computed(() => memoryList.value.filter((memory) => {
  if (selectedOwnerFilter.value === "__global__" && memory.ownerAgentId) return false;
  if (selectedOwnerFilter.value && selectedOwnerFilter.value !== "__global__" && String(memory.ownerAgentId || "").trim() !== selectedOwnerFilter.value) {
    return false;
  }
  if (selectedProfileTagFilter.value && !memory.tags.includes(selectedProfileTagFilter.value)) {
    return false;
  }
  if (selectedMemoryTypeFilter.value && memory.memoryType !== selectedMemoryTypeFilter.value) {
    return false;
  }
  return true;
}));

const sortedMemories = computed(() => {
  const base = filteredMemories.value;
  if (isSearchMode.value) {
    return [...base].sort((a, b) => (b.finalScore ?? 0) - (a.finalScore ?? 0));
  }
  return [...base].sort((a, b) => {
    const ta = Date.parse(a.createdAt || a.updatedAt || "");
    const tb = Date.parse(b.createdAt || b.updatedAt || "");
    if (Number.isFinite(ta) && Number.isFinite(tb)) return tb - ta;
    return (b.createdAt || b.updatedAt || "").localeCompare(a.createdAt || a.updatedAt || "");
  });
});

const memoryPageCount = computed(() => Math.max(1, Math.ceil(sortedMemories.value.length / memoryPageSize.value)));
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
  const start = (page - 1) * memoryPageSize.value;
  return sortedMemories.value.slice(start, start + memoryPageSize.value);
});

const memoryTemplateGroups = computed<ConfigTemplateGroup[]>(() => ([
  {
    key: "vectorization",
    title: t("config.memory.vectorization"),
    rows: [
      { key: "embedding", items: [] },
      { key: "rerank", items: [] },
    ],
  },
  ...(SHOW_DEV_MEMORY_DIAGNOSIS
    ? [
        {
          key: "chat-history",
          title: t("config.memory.chatHistory"),
          rows: [{ key: "chat-history", items: [] }],
        },
        {
          key: "recall-diagnosis",
          title: t("config.memory.recallDiagnosis"),
          rows: [{ key: "recall-diagnosis", items: [] }],
        },
      ]
    : []),
  {
    key: "memory-list",
    title: t("config.memory.list"),
    rows: [{ key: "memory-list", items: [] }],
  },
]));

function setMemoryViewMode(mode: "detailed" | "compact"): void {
  if (memoryViewMode.value === mode) return;
  memoryViewMode.value = mode;
  memoryPage.value = 1;
  persistMemoryViewMode(mode);
}

function toggleMemoryViewMode(): void {
  setMemoryViewMode(memoryViewMode.value === "detailed" ? "compact" : "detailed");
}

function toggleFilterPanel(): void {
  filterPanelOpen.value = !filterPanelOpen.value;
}

function resetMemoryPage(): void {
  memoryPage.value = 1;
}

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

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = size >= 10 || unitIndex === 0 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

async function withLoading<T>(fn: () => Promise<T>): Promise<T | null> {
  loading.value = true;
  try {
    return await fn();
  } catch (err) {
    notify(t('sidebar.memoryOperationFailed', { error: String(err) }), "error");
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
  memoryList.value = memoryList.value.filter((memory) => memory.id !== memoryId);
  memoryPage.value = Math.max(1, Math.min(memoryPage.value, memoryPageCount.value));
  notify(t('config.memory.memoryDeleted'), "success");
}

async function refreshMemories() {
  const result = await withLoading(() => invokeTauri<MemoryEntry[]>("list_memories"));
  if (!result) return;
  memoryList.value = result;
  memoryPage.value = 1;
  isSearchMode.value = false;
  notify(t('config.memory.loadedCount', { count: result.length }), "success");
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
        rerankScore: number;
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
    rerankScore: hit.rerankScore,
    finalScore: hit.finalScore,
  }));
  memoryPage.value = 1;
  isSearchMode.value = true;
  notify(t('config.memory.searchCompleted', { count: result.memories.length }), "success");
}

function chatHistorySourceLabel(sourceKind: string): string {
  const labels: Record<string, string> = {
    localConversation: t('config.memory.sourceLocal'),
    archive: t('config.memory.sourceArchive'),
    contact: t('config.memory.sourceContact'),
  };
  return labels[sourceKind] || sourceKind;
}

async function searchChatHistory() {
  // 人格为可选筛选：不选即全库检索（主人手动翻库不受归属限制）。
  const agentId = chatHistoryAgentId.value.trim();
  const query = chatHistoryQuery.value.trim();
  if (!query || chatHistoryLoading.value) return;
  chatHistoryLoading.value = true;
  chatHistoryMessage.value = t('sidebar.memorySearching');
  try {
    const result = await invokeTauri<{
      hits: ChatHistorySearchHit[];
      stats: ChatHistorySearchStats;
      elapsedMs: number;
    }>("search_chat_history_slices", {
      input: {
        agentId: agentId || null,
        query,
        limit: 30,
      },
    });
    chatHistoryHits.value = result.hits;
    chatHistoryStats.value = result.stats;
    chatHistoryMessage.value = t('sidebar.memorySearchResult', { count: result.hits.length, ms: result.elapsedMs });
  } catch (err) {
    chatHistoryMessage.value = `${t('config.memory.chatHistorySearchFailed')}: ${String(err)}`;
  } finally {
    chatHistoryLoading.value = false;
  }
}

async function clearSearch() {
  searchQuery.value = "";
  await refreshMemories();
}

// 召回诊断：工具召回 / RAG 召换双模式
// 后端 search_memories_recall 复用对话真实召回链路(visible_for_agent + limit=7 + top*0.5 阈值),
// 给定 (agentId, query) 输出与对话实际召回逐字一致的结果。
async function searchRecall() {
  const agentId = recallAgentId.value.trim();
  const query = recallQuery.value.trim();
  if (!agentId || !query || recallLoading.value) return;
  recallLoading.value = true;
  recallMessage.value = t("config.memory.recallSearching");
  try {
    const result = await invokeTauri<{
      memories: Array<{
        memory: MemoryEntry;
        bm25Score: number;
        bm25RawScore: number;
        vectorScore: number;
        rerankScore: number;
        finalScore: number;
      }>;
      elapsedMs: number;
      mode: string;
      total: number;
      offset: number;
      limit: number;
    }>("search_memories_recall", {
      input: {
        agentId,
        query,
        mode: recallMode.value,
        time: recallMode.value === "tool" ? recallTime.value.trim() || null : null,
        offset: 0,
        limit: 7,
      },
    });
    recallResults.value = result.memories.map((hit) => ({
      ...hit.memory,
      bm25Score: hit.bm25Score,
      bm25RawScore: hit.bm25RawScore,
      vectorScore: hit.vectorScore,
      rerankScore: hit.rerankScore,
      finalScore: hit.finalScore,
    }));
    recallMeta.value = { total: result.total, offset: result.offset, limit: result.limit };
    recallMessage.value = t("config.memory.recallResult", {
      count: result.memories.length,
      total: result.total,
      ms: result.elapsedMs,
    });
  } catch (err) {
    recallResults.value = [];
    recallMeta.value = null;
    recallMessage.value = `${t("config.memory.recallFailed")}: ${String(err)}`;
  } finally {
    recallLoading.value = false;
  }
}

function clearRecall() {
  recallQuery.value = "";
  recallTime.value = "";
  recallResults.value = [];
  recallMessage.value = "";
  recallMeta.value = null;
}

async function handleMemoryImportDone(result: {
  createdCount: number;
  mergedCount: number;
  totalCount: number;
}) {
  await refreshMemories();
  notify(t("config.memory.importDone", {
    created: result.createdCount,
    merged: result.mergedCount,
    total: result.totalCount,
  }), "success");
}

function handleMemoryExportDone(result: { count: number }) {
  notify(t("config.memory.exportedCount", { count: result.count }), "success");
}

function loadMemoryViewModePreference(): void {
  if (typeof window === "undefined") return;
  try {
    const stored = window.localStorage.getItem(MEMORY_VIEW_MODE_STORAGE_KEY);
    if (stored === "detailed" || stored === "compact") {
      memoryViewMode.value = stored;
    }
  } catch {
    // Ignore storage errors and keep default mode.
  }
}

function persistMemoryViewMode(mode: "detailed" | "compact"): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(MEMORY_VIEW_MODE_STORAGE_KEY, mode);
  } catch {
    // Ignore storage errors and keep UI functional.
  }
}

async function testEmbeddingProvider() {
  if (embeddingTesting.value) return;
  embeddingTesting.value = true;
  try {
    const result = await invokeTauri<{ providerKind: string; modelName: string; vectorDim: number; elapsedMs: number }>(
      "test_memory_embedding_provider",
      {
        input: {
          apiConfigId: embeddingApiConfigId.value || undefined,
          providerId: selectedEmbeddingApiConfig.value?.requestFormat || undefined,
          modelName: selectedEmbeddingApiConfig.value?.model || undefined,
        },
      },
    );
    embeddingLastPassedTestKey.value = embeddingCurrentTestKey.value;
    notify(t('config.memory.embeddingTestSuccess', { provider: result.providerKind, dim: result.vectorDim, ms: result.elapsedMs }), "success");
  } catch (err) {
    notify(t('config.memory.embeddingTestFailed', { error: String(err) }), "error");
  } finally {
    embeddingTesting.value = false;
  }
}

async function testRerankProvider() {
  if (rerankTesting.value) return;
  rerankTesting.value = true;
  try {
    const result = await invokeTauri<{ providerKind: string; modelName: string; elapsedMs: number; resultCount: number; topIndex?: number; topScore?: number }>(
      "test_memory_rerank_provider",
      {
        input: {
          apiConfigId: rerankApiConfigId.value || undefined,
          modelName: selectedRerankApiConfig.value?.model || undefined,
        },
      },
    );
    rerankLastPassedTestKey.value = rerankCurrentTestKey.value;
    notify(t('config.memory.rerankTestSuccess', { provider: result.providerKind, count: result.resultCount, ms: result.elapsedMs }), "success");
  } catch (err) {
    notify(t('config.memory.rerankTestFailed', { error: String(err) }), "error");
  } finally {
    rerankTesting.value = false;
  }
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
    notify(t('config.memory.embeddingClosed'), "default");
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
    notify(t('config.memory.embeddingSyncSuccess', { added: result.added, deleted: result.deleted, batchCount: result.batchCount }), "success");
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
    notify(t('config.memory.rerankClosed'), "default");
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
  notify(t('config.memory.rerankSaved'), "success");
}

async function loadApiConfigs() {
  const cfg = await withLoading(() => invokeTauri<AppConfigLite>("load_config"));
  if (!cfg) return;
  apiConfigs.value = Array.isArray(cfg.apiConfigs) ? cfg.apiConfigs : [];
}

async function loadPersonaNames() {
  const agents = await withLoading(() => invokeTauri<PersonaLite[]>("load_agents"));
  if (!agents) return;
  personaOptions.value = agents.filter((agent) => String(agent.id || "").trim());
  const next: Record<string, string> = {};
  for (const agent of agents) {
    const id = String(agent.id || "").trim();
    if (!id) continue;
    const name = String(agent.name || "").trim();
    next[id] = name || id;
  }
  personaNameMap.value = next;
  if (!chatHistoryAgentId.value && personaOptions.value.length > 0) {
    chatHistoryAgentId.value = personaOptions.value[0].id;
  }
  if (!recallAgentId.value && personaOptions.value.length > 0) {
    recallAgentId.value = personaOptions.value[0].id;
  }
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
  loadMemoryViewModePreference();
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
