<template>
  <div class="space-y-5">
    <div class="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
      <div>
        <div class="text-sm font-semibold">{{ t("config.usage.pageTitle") }}</div>
      </div>
      <button class="btn btn-sm bg-base-100 shrink-0" :disabled="loading" @click="refreshOverview">
        <AppIcon name="refresh-cw" class="h-4 w-4" :class="{ 'animate-spin': loading }" />
        <span>{{ t("common.refresh") }}</span>
      </button>
    </div>

    <div v-if="loading && !overview" class="rounded-box border border-base-300 bg-base-100 p-4">
      <div class="mb-3 text-sm opacity-70">{{ t("common.loading") }}</div>
      <progress class="progress progress-primary w-full"></progress>
    </div>

    <template v-else-if="overview">
      <ConfigCard>
        <div class="stats stats-vertical w-full md:stats-horizontal py-3">
          <div v-for="item in summaryStats" :key="item.label" class="stat">
            <div class="stat-title text-xs">{{ item.label }}</div>
            <div class="stat-value text-2xl">{{ item.value }}</div>
            <div v-if="item.desc" class="stat-desc">{{ item.desc }}</div>
          </div>
        </div>
      </ConfigCard>

      <div class="grid gap-4 xl:grid-cols-2">
        <ConfigCard :title="t('config.usage.modelTitle')">
          <div class="overflow-x-auto py-3">
            <table class="table table-sm">
                <thead>
                  <tr>
                    <th><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleProviderModelSort('providerLabel')">{{ columnLabel("model") }}{{ sortIndicator(providerModelSort, 'providerLabel') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleProviderModelSort('totalTokens')">{{ columnLabel("totalTokens") }}{{ sortIndicator(providerModelSort, 'totalTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleProviderModelSort('outputTokens')">{{ columnLabel("outputTokens") }}{{ sortIndicator(providerModelSort, 'outputTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleProviderModelSort('reasoningTokens')">{{ columnLabel("reasoningTokens") }}{{ sortIndicator(providerModelSort, 'reasoningTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleProviderModelSort('cacheHitRate')">{{ columnLabel("cacheHitRate") }}{{ sortIndicator(providerModelSort, 'cacheHitRate') }}</button></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in sortedProviderModels" :key="`provider-model-${item.key}`">
                    <td class="min-w-40">{{ providerLabel(item) }} · {{ item.modelName }}</td>
                    <td class="text-right">{{ formatTokens(totalInputTokens(item)) }}</td>
                    <td class="text-right">{{ formatTokens(item.outputTokens) }}</td>
                    <td class="text-right">{{ formatTokens(item.reasoningTokens) }}</td>
                    <td class="text-right">{{ formatPercent(cacheHitRate(item)) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
        </ConfigCard>

        <ConfigCard :title="t('config.usage.agentTitle')">
          <div class="overflow-x-auto py-3">
            <table class="table table-sm">
                <thead>
                  <tr>
                    <th><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleAgentSort('label')">{{ columnLabel("agent") }}{{ sortIndicator(agentSort, 'label') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleAgentSort('totalTokens')">{{ columnLabel("totalTokens") }}{{ sortIndicator(agentSort, 'totalTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleAgentSort('outputTokens')">{{ columnLabel("outputTokens") }}{{ sortIndicator(agentSort, 'outputTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleAgentSort('reasoningTokens')">{{ columnLabel("reasoningTokens") }}{{ sortIndicator(agentSort, 'reasoningTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleAgentSort('cacheHitRate')">{{ columnLabel("cacheHitRate") }}{{ sortIndicator(agentSort, 'cacheHitRate') }}</button></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in sortedAgents" :key="`agent-${item.key}`">
                    <td class="min-w-36">{{ agentLabel(item) }}</td>
                    <td class="text-right">{{ formatTokens(totalInputTokens(item)) }}</td>
                    <td class="text-right">{{ formatTokens(item.outputTokens) }}</td>
                    <td class="text-right">{{ formatTokens(item.reasoningTokens) }}</td>
                    <td class="text-right">{{ formatPercent(cacheHitRate(item)) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
        </ConfigCard>

        <ConfigCard :title="t('config.usage.kindTitle')">
          <div class="overflow-x-auto py-3">
            <table class="table table-sm">
                <thead>
                  <tr>
                    <th><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleKindSort('label')">{{ columnLabel("kind") }}{{ sortIndicator(kindSort, 'label') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleKindSort('totalTokens')">{{ columnLabel("totalTokens") }}{{ sortIndicator(kindSort, 'totalTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleKindSort('outputTokens')">{{ columnLabel("outputTokens") }}{{ sortIndicator(kindSort, 'outputTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleKindSort('reasoningTokens')">{{ columnLabel("reasoningTokens") }}{{ sortIndicator(kindSort, 'reasoningTokens') }}</button></th>
                    <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleKindSort('cacheHitRate')">{{ columnLabel("cacheHitRate") }}{{ sortIndicator(kindSort, 'cacheHitRate') }}</button></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in sortedKinds" :key="`kind-${item.key}`">
                    <td>{{ kindLabel(item) }}</td>
                    <td class="text-right">{{ formatTokens(totalInputTokens(item)) }}</td>
                    <td class="text-right">{{ formatTokens(item.outputTokens) }}</td>
                    <td class="text-right">{{ formatTokens(item.reasoningTokens) }}</td>
                    <td class="text-right">{{ formatPercent(cacheHitRate(item)) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
        </ConfigCard>
      </div>

      <ConfigCard :title="t('config.usage.conversationTitle')">
        <div class="py-3">
          <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div>
            </div>
            <div class="flex flex-col gap-2 md:items-end">
              <div class="join">
                <label
                  v-for="option in conversationFilterOptions"
                  :key="option.value"
                  class="btn btn-sm join-item border-base-300"
                  :class="conversationFilter === option.value ? 'btn-primary' : 'bg-base-200 text-base-content hover:bg-base-300'"
                >
                  <input
                    class="sr-only"
                    type="radio"
                    name="usage-conversation-filter"
                    :checked="conversationFilter === option.value"
                    @change="setConversationFilter(option.value)"
                  />
                  {{ option.label }}
                </label>
              </div>
            </div>
          </div>
          <div class="mt-3 overflow-x-auto">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleConversationSort('activityAt')">{{ columnLabel("conversation") }}{{ sortIndicator(conversationSort, 'activityAt') }}</button></th>
                  <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleConversationSort('totalTokens')">{{ columnLabel("totalTokens") }}{{ sortIndicator(conversationSort, 'totalTokens') }}</button></th>
                  <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleConversationSort('outputTokens')">{{ columnLabel("outputTokens") }}{{ sortIndicator(conversationSort, 'outputTokens') }}</button></th>
                  <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleConversationSort('reasoningTokens')">{{ columnLabel("reasoningTokens") }}{{ sortIndicator(conversationSort, 'reasoningTokens') }}</button></th>
                  <th class="text-right"><button class="btn btn-ghost btn-xs px-1 font-semibold" @click="toggleConversationSort('cacheHitRate')">{{ columnLabel("cacheHitRate") }}{{ sortIndicator(conversationSort, 'cacheHitRate') }}</button></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="item in pagedConversations" :key="item.conversationId">
                  <td class="min-w-64">
                    <div class="flex items-start gap-3">
                      <div class="avatar shrink-0">
                        <div class="flex h-10 w-10 items-center justify-center rounded-full bg-neutral text-neutral-content">
                          <img
                            v-if="conversationAvatarUrl(item)"
                            :src="conversationAvatarUrl(item)"
                            :alt="conversationAvatarLabel(item)"
                            class="h-10 w-10 rounded-full object-cover"
                          />
                          <span v-else class="text-sm font-bold">{{ conversationAvatarInitial(item) }}</span>
                        </div>
                      </div>
                      <div class="min-w-0">
                        <div class="truncate font-medium">{{ displayConversationTitle(item) }}</div>
                        <div class="text-xs opacity-60">{{ item.conversationId }}</div>
                        <div class="text-xs opacity-60">{{ formatConversationMeta(item) }}</div>
                      </div>
                    </div>
                  </td>
                  <td class="text-right font-medium">{{ formatTokens(totalInputTokens(item)) }}</td>
                  <td class="text-right">{{ formatTokens(item.outputTokens) }}</td>
                  <td class="text-right">{{ formatTokens(item.reasoningTokens) }}</td>
                  <td class="text-right">{{ formatPercent(cacheHitRate(item)) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div class="mt-3 flex flex-col gap-2 text-sm md:flex-row md:items-center md:justify-between">
            <div class="opacity-70">
              {{ t("config.usage.pageRange", { page: conversationPage, total: conversationPageCount }) }}
              <span class="mx-1">·</span>
              {{ t("config.usage.showingRange", { start: conversationPageStart, end: conversationPageEnd }) }}
            </div>
            <div class="join">
              <button
                class="btn btn-sm join-item"
                type="button"
                :disabled="conversationPage <= 1"
                @click="conversationPage -= 1"
              >
                {{ t("config.usage.previousPage") }}
              </button>
              <button
                class="btn btn-sm join-item"
                type="button"
                :disabled="conversationPage >= conversationPageCount"
                @click="conversationPage += 1"
              >
                {{ t("config.usage.nextPage") }}
              </button>
            </div>
          </div>
        </div>
      </ConfigCard>
    </template>

    <div v-else class="rounded-box border border-base-300 bg-base-100 p-4 text-sm opacity-70">
      {{ t("config.usage.empty") }}
    </div>

    <div v-if="errorText" class="alert alert-error">
      <span>{{ errorText }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import ConfigCard from "../../components/ConfigCard.vue";
import { invokeTauri } from "../../../../services/tauri-api";
import type {
  PersonaProfile,
  UsageAggregateItem,
  UsageConversationItem,
  UsageOverview,
  UsageProviderModelAggregateItem,
} from "../../../../types/app";
import { useAvatarCache } from "../../../chat/composables/use-avatar-cache";
import { resolveConversationDisplayTitle } from "../../../chat/utils/conversation-title";
import { toErrorMessage } from "../../../../utils/error";

const { t, locale } = useI18n();

const errorText = ref("");
const overview = ref<UsageOverview | null>(null);
const overviewStatus = ref<OverviewStatus | null>(null);
const conversationPage = ref(1);
const conversationPageSize = 20;
const conversationFilter = ref<"all" | "normal" | "delegate" | "contact" | "system" | "archived">("all");
const { resolveAvatarUrl, ensureAvatarCached } = useAvatarCache({ personas: ref<PersonaProfile[]>([]) });
type SortDirection = "asc" | "desc";
type SortState<T extends string> = {
  key: T;
  direction: SortDirection;
};
type OverviewStatus = {
  computeState: "idle" | "running";
  freshness: "never" | "fresh" | "expired";
  generatedAt?: string | null;
  lastError?: string | null;
};
type OverviewSnapshot<T> = {
  status: OverviewStatus;
  data?: T | null;
};
type ProviderModelSortKey =
  | "providerLabel"
  | "modelName"
  | "weightedTokens"
  | "totalTokens"
  | "cacheHitRate"
  | "outputTokens"
  | "reasoningTokens"
  | "cacheReadTokens"
  | "cacheWrite"
  | "conversationCount";
type AggregateSortKey =
  | "label"
  | "weightedTokens"
  | "totalTokens"
  | "cacheHitRate"
  | "outputTokens"
  | "reasoningTokens"
  | "cacheReadTokens"
  | "cacheWrite"
  | "conversationCount";
type ConversationSortKey =
  | "activityAt"
  | "weightedTokens"
  | "totalTokens"
  | "cacheHitRate"
  | "outputTokens"
  | "reasoningTokens"
  | "cacheReadTokens"
  | "cacheWrite"
  | "messageCount";

const providerModelSort = ref<SortState<ProviderModelSortKey>>({ key: "weightedTokens", direction: "desc" });
const agentSort = ref<SortState<AggregateSortKey>>({ key: "weightedTokens", direction: "desc" });
const kindSort = ref<SortState<AggregateSortKey>>({ key: "weightedTokens", direction: "desc" });
const conversationSort = ref<SortState<ConversationSortKey>>({ key: "weightedTokens", direction: "desc" });
const loading = computed(() => overviewStatus.value?.computeState === "running");
let overviewPollTimer: number | null = null;
let usageTabUnmounted = false;

const conversationFilterOptions = computed(() => [
  { value: "all" as const, label: t("config.usage.filters.all") },
  { value: "normal" as const, label: t("config.usage.filters.normal") },
  { value: "delegate" as const, label: t("config.usage.filters.delegate") },
  { value: "contact" as const, label: t("config.usage.filters.contact") },
  { value: "system" as const, label: t("config.usage.filters.system") },
  { value: "archived" as const, label: t("config.usage.filters.archived") },
]);

const summaryStats = computed<Array<{ label: string; value: string; desc?: string }>>(() => {
  if (!overview.value) return [];
  const totals = overview.value.totals;
  return [
    {
      label: t("config.usage.stats.totalTokens"),
      value: formatTokens(totals.totalTokens),
    },
    {
      label: t("config.usage.stats.outputTokens"),
      value: formatTokens(totals.outputTokens),
    },
    {
      label: t("config.usage.stats.reasoningTokens"),
      value: formatTokens(totals.reasoningTokens),
    },
    {
      label: t("config.usage.stats.averageCacheHitRate"),
      value: formatPercent(cacheHitRate(totals)),
    },
  ];
});
const sortedProviderModels = computed(() =>
  sortProviderModelItems(overview.value?.byProviderModel || [], providerModelSort.value).slice(0, 12),
);
const sortedAgents = computed(() =>
  sortAggregateItems(overview.value?.byAgent || [], agentSort.value).slice(0, 12),
);
const sortedKinds = computed(() =>
  sortAggregateItems(overview.value?.byKind || [], kindSort.value),
);
const sortedFilteredConversations = computed(() => {
  const items = overview.value?.conversations || [];
  const filtered = items.filter((item) => conversationMatchesFilter(item, conversationFilter.value));
  return sortConversationItems(filtered, conversationSort.value);
});
const filteredConversationCount = computed(() => sortedFilteredConversations.value.length);
const conversationPageCount = computed(() => Math.max(1, Math.ceil(filteredConversationCount.value / conversationPageSize)));
const pagedConversations = computed(() => {
  const items = sortedFilteredConversations.value;
  const start = (conversationPage.value - 1) * conversationPageSize;
  return items.slice(start, start + conversationPageSize);
});
const conversationPageStart = computed(() => {
  const total = filteredConversationCount.value;
  if (total === 0) return 0;
  return (conversationPage.value - 1) * conversationPageSize + 1;
});
const conversationPageEnd = computed(() => {
  const total = filteredConversationCount.value;
  return Math.min(total, conversationPage.value * conversationPageSize);
});

watchEffect(() => {
  for (const item of pagedConversations.value) {
    if (!item.avatarPath) continue;
    void ensureAvatarCached(item.avatarPath, item.avatarUpdatedAt);
  }
});

function formatTokens(value: number): string {
  const numeric = Number(value || 0);
  if (!Number.isFinite(numeric)) return "0";
  const abs = Math.abs(numeric);
  const units = [
    { threshold: 1_000_000_000_000, suffix: "T" },
    { threshold: 1_000_000_000, suffix: "B" },
    { threshold: 1_000_000, suffix: "M" },
    { threshold: 1_000, suffix: "K" },
  ];
  for (const unit of units) {
    if (abs >= unit.threshold) {
      const scaled = numeric / unit.threshold;
      const digits = Math.abs(scaled) >= 100 ? 0 : Math.abs(scaled) >= 10 ? 1 : 2;
      return `${scaled.toFixed(digits).replace(/\.0+$|(\.\d*[1-9])0+$/, "$1")}${unit.suffix}`;
    }
  }
  return new Intl.NumberFormat(currentLocale()).format(Math.round(numeric));
}

function totalInputTokens(item: { totalTokens: number }): number {
  return Math.max(0, Number(item.totalTokens || 0));
}

function cacheWriteAmount(item: { inputTokens: number; cacheReadTokens: number }): number {
  return Math.max(0, Number(item.inputTokens || 0) - Number(item.cacheReadTokens || 0));
}

function cacheHitRate(item: { totalTokens: number; cacheReadTokens: number }): number {
  const total = totalInputTokens(item);
  if (total <= 0) return 0;
  return Number(item.cacheReadTokens || 0) / total;
}

function formatPercent(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0%";
  return `${(value * 100).toFixed(value >= 0.1 ? 0 : 1).replace(/\.0$/, "")}%`;
}

function formatDateTime(value?: string | null): string {
  const text = String(value || "").trim();
  if (!text) return "-";
  const date = new Date(text);
  if (Number.isNaN(date.getTime())) return text;
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

function timeValue(value?: string | null): number {
  const text = String(value || "").trim();
  if (!text) return 0;
  const timestamp = new Date(text).getTime();
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function compareText(left: string, right: string): number {
  return left.localeCompare(right, currentLocale());
}

function currentLocale(): string {
  return String(locale.value || "zh-CN");
}

function columnLabel(key: string): string {
  return t(`config.usage.columns.${key}`);
}

// 用量页 by_kind 的 label 由后端硬编码中文，这里按 kind key 映射到当前语言
function kindLabel(item: UsageAggregateItem): string {
  const kindKeyMap: Record<string, string> = {
    normal: t("config.usage.kinds.normal"),
    delegate: t("config.usage.kinds.delegate"),
    archived: t("config.usage.kinds.archived"),
    system_notification: t("config.usage.kinds.systemNotification"),
    remote_im_contact: t("config.usage.kinds.remoteImContact"),
  };
  return kindKeyMap[item.key] ?? item.label;
}

// 后端对未识别供应商写入硬编码中文占位，这里映射到当前语言
function providerLabel(item: UsageProviderModelAggregateItem): string {
  if (item.providerLabel === "未识别供应商") {
    return t("config.usage.unknownProvider");
  }
  return item.providerLabel;
}

// 后端对未绑定人格写入硬编码中文占位，这里映射到当前语言
function agentLabel(item: UsageAggregateItem): string {
  if (item.label === "未绑定人格") {
    return t("config.usage.unboundAgent");
  }
  return item.label;
}

function compareNumber(left: number, right: number): number {
  return left - right;
}

function applyDirection(result: number, direction: SortDirection): number {
  return direction === "asc" ? result : -result;
}

function cacheWriteSortValue(item: { inputTokens: number; cacheReadTokens: number }): number {
  return cacheWriteAmount(item);
}

function providerModelSortValue(item: UsageProviderModelAggregateItem, key: ProviderModelSortKey): number | string {
  if (key === "providerLabel") return item.providerLabel || "";
  if (key === "modelName") return item.modelName || "";
  if (key === "weightedTokens") return item.weightedTokens || 0;
  if (key === "totalTokens") return item.totalTokens || 0;
  if (key === "cacheHitRate") return cacheHitRate(item);
  if (key === "outputTokens") return item.outputTokens || 0;
  if (key === "reasoningTokens") return item.reasoningTokens || 0;
  if (key === "cacheReadTokens") return item.cacheReadTokens || 0;
  if (key === "cacheWrite") return cacheWriteSortValue(item);
  return item.conversationCount || 0;
}

function aggregateSortValue(item: UsageAggregateItem, key: AggregateSortKey): number | string {
  if (key === "label") return item.label || "";
  if (key === "weightedTokens") return item.weightedTokens || 0;
  if (key === "totalTokens") return item.totalTokens || 0;
  if (key === "cacheHitRate") return cacheHitRate(item);
  if (key === "outputTokens") return item.outputTokens || 0;
  if (key === "reasoningTokens") return item.reasoningTokens || 0;
  if (key === "cacheReadTokens") return item.cacheReadTokens || 0;
  if (key === "cacheWrite") return cacheWriteSortValue(item);
  return item.conversationCount || 0;
}

function conversationSortValue(item: UsageConversationItem, key: ConversationSortKey): number {
  if (key === "activityAt") {
    return item.archivedAt ? timeValue(item.archivedAt) : timeValue(item.updatedAt);
  }
  if (key === "weightedTokens") return item.weightedTokens || 0;
  if (key === "totalTokens") return item.totalTokens || 0;
  if (key === "cacheHitRate") return cacheHitRate(item);
  if (key === "outputTokens") return item.outputTokens || 0;
  if (key === "reasoningTokens") return item.reasoningTokens || 0;
  if (key === "cacheReadTokens") return item.cacheReadTokens || 0;
  if (key === "cacheWrite") return cacheWriteSortValue(item);
  return item.messageCount || 0;
}

function sortByValue<T>(
  items: T[],
  getter: (item: T) => number | string,
  direction: SortDirection,
  fallback: (left: T, right: T) => number,
): T[] {
  return [...items].sort((left, right) => {
    const leftValue = getter(left);
    const rightValue = getter(right);
    const result = typeof leftValue === "string" && typeof rightValue === "string"
      ? compareText(leftValue, rightValue)
      : compareNumber(Number(leftValue || 0), Number(rightValue || 0));
    if (result !== 0) return applyDirection(result, direction);
    return fallback(left, right);
  });
}

function sortProviderModelItems(
  items: UsageProviderModelAggregateItem[],
  sort: SortState<ProviderModelSortKey>,
): UsageProviderModelAggregateItem[] {
  return sortByValue(
    items,
    (item) => providerModelSortValue(item, sort.key),
    sort.direction,
    (left, right) => compareText(left.key, right.key),
  );
}

function sortAggregateItems(
  items: UsageAggregateItem[],
  sort: SortState<AggregateSortKey>,
): UsageAggregateItem[] {
  return sortByValue(
    items,
    (item) => aggregateSortValue(item, sort.key),
    sort.direction,
    (left, right) => compareText(left.key, right.key),
  );
}

function sortConversationItems(
  items: UsageConversationItem[],
  sort: SortState<ConversationSortKey>,
): UsageConversationItem[] {
  return sortByValue(
    items,
    (item) => conversationSortValue(item, sort.key),
    sort.direction,
    (left, right) => compareText(left.conversationId, right.conversationId),
  );
}

function nextSortState<T extends string>(current: SortState<T>, key: T): SortState<T> {
  if (current.key === key) {
    return {
      key,
      direction: current.direction === "desc" ? "asc" : "desc",
    };
  }
  return { key, direction: "desc" };
}

function toggleProviderModelSort(key: ProviderModelSortKey) {
  providerModelSort.value = nextSortState(providerModelSort.value, key);
}

function toggleAgentSort(key: AggregateSortKey) {
  agentSort.value = nextSortState(agentSort.value, key);
}

function toggleKindSort(key: AggregateSortKey) {
  kindSort.value = nextSortState(kindSort.value, key);
}

function toggleConversationSort(key: ConversationSortKey) {
  conversationSort.value = nextSortState(conversationSort.value, key);
  conversationPage.value = 1;
}

function sortIndicator<T extends string>(sort: { key: T; direction: SortDirection }, key: T): string {
  if (sort.key !== key) return "";
  return sort.direction === "desc" ? " ↓" : " ↑";
}

function conversationMatchesFilter(
  item: UsageConversationItem,
  filter: "all" | "normal" | "delegate" | "contact" | "system" | "archived",
): boolean {
  if (filter === "all") return true;
  if (filter === "system") return item.isSystemNotificationConversation;
  if (filter === "delegate") return item.isDelegate;
  if (filter === "contact") return item.conversationKind === "remote_im_contact";
  if (filter === "archived") return !!item.archivedAt;
  return !item.isSystemNotificationConversation
    && !item.isDelegate
    && item.conversationKind !== "remote_im_contact"
    && !item.archivedAt;
}

function setConversationFilter(value: "all" | "normal" | "delegate" | "contact" | "system" | "archived") {
  conversationFilter.value = value;
  conversationPage.value = 1;
}

function formatConversationMeta(item: UsageConversationItem): string {
  if (item.archivedAt) {
    return t("config.usage.archivedAt", { time: formatDateTime(item.archivedAt) });
  }
  return t("config.usage.updatedAt", { time: formatDateTime(item.updatedAt) });
}

function displayConversationTitle(item: UsageConversationItem): string {
  // 后端对已删除会话写入硬编码中文占位，这里映射到当前语言
  if (item.title === "已删除会话") {
    return t("config.usage.deletedConversation");
  }
  return resolveConversationDisplayTitle(
    {
      conversationId: item.conversationId,
      kind: item.conversationKind === "remote_im_contact" ? "remote_im_contact" : "local_unarchived",
      title: item.title,
      summaryTitle: item.summaryTitle,
      remoteContactDisplayName: "",
      updatedAt: item.updatedAt,
      lastMessageAt: item.updatedAt,
      isSystemNotificationConversation: item.isSystemNotificationConversation,
    },
    {
      locale: currentLocale(),
      untitledLabel: t("chat.untitledConversation"),
      systemNotificationLabel: t("chat.systemPersona"),
    },
  );
}

function conversationAvatarUrl(item: UsageConversationItem): string {
  return resolveAvatarUrl(item.avatarPath, item.avatarUpdatedAt);
}

function conversationAvatarLabel(item: UsageConversationItem): string {
  const fallback = t("config.usage.conversationAvatar");
  const agentName = item.agentName === "未绑定人格"
    ? t("config.usage.unboundAgent")
    : item.agentName;
  return String(agentName || displayConversationTitle(item) || fallback).trim() || fallback;
}

function conversationAvatarInitial(item: UsageConversationItem): string {
  const text = conversationAvatarLabel(item);
  return text.charAt(0).toUpperCase() || "?";
}

function stopOverviewPolling() {
  if (overviewPollTimer != null) {
    window.clearTimeout(overviewPollTimer);
    overviewPollTimer = null;
  }
}

function scheduleOverviewPolling() {
  stopOverviewPolling();
  if (usageTabUnmounted || overviewStatus.value?.computeState !== "running") return;
  overviewPollTimer = window.setTimeout(() => {
    void loadOverview();
  }, 1000);
}

function applyOverviewSnapshot(snapshot: OverviewSnapshot<UsageOverview>) {
  overviewStatus.value = snapshot.status;
  if (snapshot.data) {
    overview.value = snapshot.data;
    conversationPage.value = 1;
  }
  if (snapshot.status.lastError) {
    errorText.value = snapshot.status.lastError;
  }
  scheduleOverviewPolling();
}

async function loadOverview() {
  errorText.value = "";
  try {
    const snapshot = await invokeTauri<OverviewSnapshot<UsageOverview>>("get_usage_overview");
    if (!usageTabUnmounted) {
      applyOverviewSnapshot(snapshot);
    }
  } catch (error) {
    errorText.value = toErrorMessage(error);
  }
}

async function refreshOverview() {
  errorText.value = "";
  try {
    const snapshot = await invokeTauri<OverviewSnapshot<UsageOverview>>("refresh_usage_overview");
    if (!usageTabUnmounted) {
      applyOverviewSnapshot(snapshot);
    }
  } catch (error) {
    errorText.value = toErrorMessage(error);
  }
}

onMounted(() => {
  void loadOverview();
});

onBeforeUnmount(() => {
  usageTabUnmounted = true;
  stopOverviewPolling();
});
</script>
