<template>
  <div class="absolute inset-0 z-40 flex min-h-0 flex-col bg-base-200">
    <div class="flex min-h-0 flex-1 flex-col" :class="showIdle ? 'justify-center pb-16' : ''">
      <div class="shrink-0">
        <div
          class="mx-auto flex w-full max-w-xl flex-col gap-2 px-5 pb-2"
          :class="showIdle ? 'pt-4' : 'pt-10'"
        >
          <!-- 搜索行 -->
          <div class="join w-full">
            <label class="input input-sm join-item grow bg-base-100">
              <svg class="h-[1em] opacity-50" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                <g stroke-linejoin="round" stroke-linecap="round" stroke-width="2.5" fill="none" stroke="currentColor">
                  <circle cx="11" cy="11" r="8"></circle>
                  <path d="m21 21-4.3-4.3"></path>
                </g>
              </svg>
              <input
                ref="inputRef"
                v-model="query"
                type="search"
                required
                :placeholder="t('chat.conversationSearchPanel.placeholder')"
                @keydown.enter.prevent="triggerSearch"
                @keydown.esc.prevent="emit('close')"
              />
            </label>
            <button
              type="button"
              class="btn btn-sm join-item shrink-0"
              :class="query.trim() ? 'btn-primary' : ''"
              :disabled="loading || !query.trim()"
              @click="triggerSearch"
            >
              {{ loading ? t("chat.conversationSearchPanel.searching") : t("chat.conversationSearchPanel.searchAction") }}
            </button>
          </div>

          <!-- 筛选行 -->
          <div class="flex flex-wrap items-center gap-1.5">
            <div class="w-28 shrink-0">
              <ConfigValueSelect v-model="sourceMode" size="sm" :options="sourceOptions" />
            </div>
            <div class="w-28 shrink-0">
              <ConfigValueSelect
                v-model="agentId"
                size="sm"
                :options="agentFilterOptions"
                :placeholder="t('chat.conversationSearchPanel.agentAny')"
              />
            </div>
            <div class="w-32 shrink-0">
              <ConfigValueSelect
                v-model="workspacePath"
                size="sm"
                :options="workspaceFilterOptions"
                :placeholder="t('chat.conversationSearchPanel.workspaceAny')"
              />
            </div>
            <div class="w-28 shrink-0">
              <ConfigValueSelect v-model="days" size="sm" :options="timeOptions" />
            </div>
          </div>

          <div v-if="notice" class="text-xs text-warning">{{ notice }}</div>
        </div>
      </div>

      <OverlayScrollArea v-if="!showIdle" class="min-h-0 flex-1" scroller-class="h-full">
        <div class="mx-auto w-full max-w-xl px-4 pb-10 pt-2">
          <div class="flex flex-col">
            <template v-if="!loading">
              <div v-for="group in hitGroups" :key="group.conversationId" class="flex flex-col gap-1.5 pb-4">
                <button
                  type="button"
                  class="flex items-baseline gap-3 rounded-lg px-2 pb-0.5 text-left"
                  @click="emit('select', group.conversationId)"
                >
                  <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ group.title }}</span>
                  <span class="shrink-0 text-xs text-base-content/40">{{ formatRecentTime(group.time) }}</span>
                </button>
                <button
                  v-for="(hit, index) in group.hits"
                  :key="`${group.conversationId}-${index}`"
                  type="button"
                  class="block w-full rounded-lg bg-base-100 px-3 py-2 text-left text-xs leading-relaxed text-base-content/70 transition-colors hover:text-base-content"
                  @click="emit('select', group.conversationId)"
                >
                  <span class="line-clamp-2"><InlineMarkdownText :text="hit.snippet" /></span>
                </button>
              </div>
              <div
                v-if="hits.length === 0"
                class="px-2 py-10 text-center text-sm text-base-content/40"
              >
                {{ error || t("chat.conversationSearchPanel.empty") }}
              </div>
            </template>
            <div
              v-else
              class="flex items-center justify-center py-10"
              :aria-label="t('chat.conversationSearchPanel.searching')"
            >
              <span class="loading loading-spinner loading-md text-base-content/50"></span>
            </div>
          </div>
        </div>
      </OverlayScrollArea>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Search } from "@lucide/vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import ConfigValueSelect from "../../config/components/ConfigValueSelect.vue";
import { formatConversationListTime } from "../utils/conversation-time";
import { formatConversationFallbackTitle } from "../utils/conversation-title";
import {
  useConversationSearch,
  type ConversationSearchAgentOption,
  type ConversationSearchHit,
  type ConversationSearchWorkspaceOption,
} from "../composables/use-conversation-search";

const props = withDefaults(defineProps<{
  open: boolean;
  agentOptions?: ConversationSearchAgentOption[];
  workspaceOptions?: ConversationSearchWorkspaceOption[];
  notice?: string;
}>(), {
  agentOptions: () => [],
  workspaceOptions: () => [],
  notice: "",
});

const emit = defineEmits<{
  (e: "close"): void;
  (e: "select", conversationId: string): void;
}>();

const { t, locale } = useI18n();
const inputRef = ref<HTMLInputElement | null>(null);
const { query, sourceMode, agentId, workspacePath, days, hits, loading, error, searched, search } = useConversationSearch();

const sourceOptions = computed<Array<{ value: string; label: string }>>(() => [
  { value: "localConversation", label: t("chat.conversationSearchPanel.sourceLocal") },
  { value: "contact", label: t("chat.conversationSearchPanel.sourceContact") },
]);

const timeOptions = computed<Array<{ value: string; label: string }>>(() => [
  { value: "0", label: t("chat.conversationSearchPanel.timeAny") },
  { value: "7", label: t("chat.conversationSearchPanel.time7d") },
  { value: "30", label: t("chat.conversationSearchPanel.time30d") },
  { value: "90", label: t("chat.conversationSearchPanel.time90d") },
  { value: "365", label: t("chat.conversationSearchPanel.time365d") },
]);

const agentFilterOptions = computed<Array<{ value: string; label: string }>>(() => props.agentOptions
  .filter((item) => String(item.name || "").trim() && item.name !== item.id)
  .map((item) => ({ value: item.id, label: item.name })));

const workspaceFilterOptions = computed<Array<{ value: string; label: string }>>(() => props.workspaceOptions
  .map((item) => ({ value: item.path, label: item.label })));

const showIdle = computed(() => !searched.value);

type HitGroup = { conversationId: string; title: string; time: string; hits: ConversationSearchHit[] };

const HITS_PER_GROUP = 3;

const hitGroups = computed<HitGroup[]>(() => {
  const groups = new Map<string, HitGroup>();
  for (const hit of hits.value) {
    const id = String(hit.conversationId || "").trim();
    if (!id) continue;
    const existing = groups.get(id);
    if (existing) {
      if (existing.hits.length < HITS_PER_GROUP) existing.hits.push(hit);
      continue;
    }
    groups.set(id, { conversationId: id, title: hitTitle(hit), time: hit.createdAt, hits: [hit] });
  }
  return Array.from(groups.values());
});

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await nextTick();
    inputRef.value?.focus();
  },
);

function triggerSearch() {
  void search();
}

function hitTitle(hit: ConversationSearchHit): string {
  const conversationId = String(hit.conversationId || "").trim();
  const candidates = [hit.conversationTitle, hit.conversationSummaryTitle];
  for (const value of candidates) {
    const raw = String(value || "").trim();
    if (raw && raw !== conversationId) return raw;
  }
  // 会话本身没有标题时，跟侧栏一样退到最近活动时间，而不是把一堆会话都显示成「未命名会话」。
  const fallback = formatConversationFallbackTitle(hit.conversationUpdatedAt, locale.value);
  return fallback || t("chat.untitledConversation");
}

function formatRecentTime(raw: string): string {
  const value = String(raw || "").trim();
  if (!value) return "";
  return formatConversationListTime(value, locale.value);
}
</script>
