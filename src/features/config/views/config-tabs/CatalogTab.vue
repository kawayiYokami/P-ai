<template>
  <SettingsStickyLayout>
    <template #header>
      <!-- 类型切换 + 刷新 -->
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div role="tablist" class="tabs tabs-box bg-base-200/80 p-1">
          <button
            role="tab"
            class="tab h-7 min-h-0 gap-1.5 rounded-md px-3 text-xs font-medium transition-all"
            :class="kind === 'mcp' ? 'tab-active bg-base-100 text-primary shadow-2xs' : 'text-base-content/60 hover:text-base-content'"
            type="button"
            @click="switchKind('mcp')"
          >
            <Plug class="h-3.5 w-3.5" />
            <span>{{ t("config.tabs.mcp") }}</span>
          </button>
          <button
            role="tab"
            class="tab h-7 min-h-0 gap-1.5 rounded-md px-3 text-xs font-medium transition-all"
            :class="kind === 'skill' ? 'tab-active bg-base-100 text-primary shadow-2xs' : 'text-base-content/60 hover:text-base-content'"
            type="button"
            @click="switchKind('skill')"
          >
            <Code class="h-3.5 w-3.5" />
            <span>{{ t("config.tabs.skill") }}</span>
          </button>
        </div>
        <button
          class="btn btn-ghost btn-sm h-8 min-h-[2rem] gap-1.5 px-2.5 text-base-content/70 hover:text-base-content"
          type="button"
          :disabled="loading"
          :title="t('config.catalog.refresh')"
          @click="reload"
        >
          <RefreshCw class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
          <span>{{ t("config.catalog.refresh") }}</span>
        </button>
      </div>
    </template>

    <!-- 检索与列表 -->
    <div class="grid gap-4 pb-8">
      <!-- 检索条：来源 + 关键词 -->
      <div class="flex flex-wrap items-center gap-3">
        <select
          v-model="sourceId"
          class="select select-bordered select-sm h-9 min-w-[12rem] text-xs"
          :disabled="loading || sources.length === 0"
          @change="onSourceChange"
        >
          <option v-for="source in sources" :key="source.id" :value="source.id">
            {{ source.name }}
          </option>
        </select>
        <div class="relative min-w-[12rem] flex-1 sm:w-72">
          <input
            v-model="query"
            type="text"
            class="input input-bordered input-sm h-9 w-full pl-8 text-xs"
            :placeholder="t('config.catalog.searchPlaceholder')"
            @keydown.enter="runSearch"
          />
          <Search class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
        </div>
        <button class="btn btn-sm min-h-[2.25rem] btn-primary px-4" type="button" :disabled="loading" @click="runSearch">
          {{ t("config.catalog.search") }}
        </button>
      </div>

      <!-- 来源说明与缓存状态 -->
      <div class="flex flex-wrap items-center justify-between gap-2 text-caption opacity-60">
        <span>{{ sourceDescription }}</span>
        <span v-if="updatedAt">
          {{ t("config.catalog.updatedAt", { time: updatedAt }) }}
          <span v-if="fromCache"> · {{ t("config.catalog.fromCache") }}</span>
        </span>
      </div>

      <!-- 状态反馈 -->
      <div v-if="statusText" class="text-xs" :class="statusError ? 'text-error' : 'opacity-70'">
        {{ statusText }}
      </div>

      <!-- 加载 / 空态 -->
      <div v-if="loading && entries.length === 0" class="flex items-center justify-center py-16 text-sm opacity-60">
        <RefreshCw class="mr-2 h-4 w-4 animate-spin" />
        <span>{{ t("config.catalog.loading") }}</span>
      </div>
      <div v-else-if="entries.length === 0" class="rounded-box border border-dashed border-base-300 p-8 text-center">
        <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-base-200 text-base-content/50">
          <Package class="h-6 w-6" />
        </div>
        <div class="mt-3 text-sm font-medium">{{ t("config.catalog.empty") }}</div>
        <div class="mt-1 text-xs opacity-60">{{ t("config.catalog.emptyHint") }}</div>
      </div>

      <!-- 条目卡片 -->
      <div v-else class="config-grid-auto-sm">
        <template v-for="entry in entries" :key="entry.source + ':' + entry.id">
          <!-- MCP 卡：工程面板感，热度归到底部与作者同行 -->
          <div
            v-if="entry.kind === 'mcp'"
            class="rounded-2xl border border-base-200 border-l-4 border-l-info bg-base-100 p-4 shadow-sm flex flex-col justify-between gap-3"
          >
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1 text-sm font-semibold text-base-content truncate" :title="entry.name">
              {{ entry.name }}
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <span v-if="entry.transport" class="badge badge-sm badge-ghost font-mono">
                {{ entry.transport }}
              </span>
              <span
                v-if="entry.requiredEnv.length"
                class="badge badge-sm badge-outline border-warning/40 text-warning"
              >
                {{ t("config.catalog.needEnvBadge") }}
              </span>
            </div>
          </div>

          <p
            :ref="setDescRef(entryKey(entry))"
            class="text-xs line-clamp-2 leading-relaxed min-h-[2.5rem]"
            :class="entry.description ? 'text-base-content/70' : 'text-base-content/40 italic'"
          >
            {{ entry.description || t("config.catalog.noDescription") }}
          </p>

          <!-- 工具清单：部分来源不提供，缺失时整块不渲染 -->
          <div v-if="entry.tools.length" class="rounded-md border border-base-200 bg-base-200/50 px-2.5 py-2">
            <div class="flex flex-wrap gap-1">
              <span
                v-for="tool in entry.tools.slice(0, TOOL_PREVIEW_LIMIT)"
                :key="tool"
                class="rounded border border-base-300/60 bg-base-100 px-1.5 py-0.5 font-mono text-caption truncate max-w-[9rem]"
              >
                {{ tool }}
              </span>
              <span v-if="entry.tools.length > TOOL_PREVIEW_LIMIT" class="px-1 py-0.5 font-mono text-caption opacity-50">
                +{{ entry.tools.length - TOOL_PREVIEW_LIMIT }}
              </span>
            </div>
          </div>

          <div class="flex items-center justify-between border-t border-base-200/80 pt-2.5">
            <div class="flex min-w-0 items-center gap-1.5 text-caption opacity-60">
              <span v-if="entry.author" class="truncate max-w-[9rem]" :title="entry.author">{{ entry.author }}</span>
              <span v-if="entry.author && entry.popularity" class="opacity-50">·</span>
              <span v-if="entry.popularity" class="shrink-0">{{ formatPopularity(entry.popularity) }}</span>
            </div>
            <div class="flex items-center gap-1.5">
              <button
                v-if="detailNeeded(entry)"
                class="btn btn-sm h-8 min-h-[2rem] bg-base-100 px-3 text-caption"
                type="button"
                @click="openDetail(entry)"
              >
                {{ t("config.catalog.detail") }}
              </button>
              <template v-if="entry.installed">
                <span class="text-caption opacity-60">
                  {{ entry.enabled ? t("config.catalog.enabled") : t("config.catalog.disabled") }}
                </span>
                <input
                  type="checkbox"
                  class="toggle toggle-xs toggle-primary"
                  :checked="entry.enabled"
                  :disabled="togglingKey === entryKey(entry)"
                  @change="toggleEntry(entry)"
                />
              </template>
              <button
                v-else
                class="btn btn-sm min-h-[2rem] h-8 px-3.5"
                :class="entry.installReady ? 'btn-primary' : 'bg-base-200'"
                type="button"
                :disabled="!entry.installReady || installingId === entryKey(entry)"
                :title="entry.installReady ? '' : t('config.catalog.notInstallable')"
                @click="requestInstall(entry)"
              >
                <span v-if="installingId === entryKey(entry)" class="loading loading-spinner loading-xs"></span>
                <Download v-else class="h-3.5 w-3.5" />
                <span>{{ t("config.catalog.install") }}</span>
              </button>
            </div>
          </div>
        </div>

          <!-- 技能卡：内容卡感，热度提到顶部与名称同行 -->
          <div
            v-else
            class="rounded-2xl border border-base-200 border-l-4 border-l-accent bg-base-100 p-4 shadow-sm flex flex-col justify-between gap-3"
          >
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0 flex-1 font-mono text-sm font-semibold text-base-content truncate" :title="entry.name">
              {{ entry.name }}
            </div>
            <div class="flex shrink-0 items-center gap-1.5 text-caption opacity-60">
              <span v-if="entry.author" class="truncate max-w-[8rem]" :title="entry.author">{{ entry.author }}</span>
              <span v-if="entry.author && entry.popularity" class="opacity-50">·</span>
              <span v-if="entry.popularity">{{ formatPopularity(entry.popularity) }}</span>
            </div>
          </div>

          <p
            :ref="setDescRef(entryKey(entry))"
            class="text-xs line-clamp-3 leading-relaxed min-h-[3.25rem]"
            :class="entry.description ? 'text-base-content/70' : 'text-base-content/40 italic'"
          >
            {{ entry.description || t("config.catalog.noDescription") }}
          </p>

          <div class="flex items-center justify-end gap-1.5 border-t border-base-200/80 pt-2.5">
            <button
              v-if="detailNeeded(entry)"
              class="btn btn-sm h-8 min-h-[2rem] bg-base-100 px-3 text-caption"
              type="button"
              @click="openDetail(entry)"
            >
              {{ t("config.catalog.detail") }}
            </button>
            <template v-if="entry.installed">
              <span class="text-caption opacity-60">
                {{ entry.enabled ? t("config.catalog.enabled") : t("config.catalog.disabled") }}
              </span>
              <input
                type="checkbox"
                class="toggle toggle-xs toggle-primary"
                :checked="entry.enabled"
                :disabled="togglingKey === entryKey(entry)"
                @change="toggleEntry(entry)"
              />
            </template>
            <button
              v-else
              class="btn btn-sm min-h-[2rem] h-8 px-3.5"
              :class="entry.installReady ? 'btn-primary' : 'bg-base-200'"
              type="button"
              :disabled="!entry.installReady || installingId === entryKey(entry)"
              :title="entry.installReady ? '' : t('config.catalog.notInstallable')"
              @click="requestInstall(entry)"
            >
              <span v-if="installingId === entryKey(entry)" class="loading loading-spinner loading-xs"></span>
              <Download v-else class="h-3.5 w-3.5" />
              <span>{{ t("config.catalog.install") }}</span>
            </button>
          </div>
          </div>
        </template>
      </div>

      <!-- 分页 -->
      <div v-if="entries.length > 0" class="flex items-center justify-center gap-3 pt-1">
        <button class="btn btn-sm bg-base-100" type="button" :disabled="page <= 1 || loading" @click="gotoPage(page - 1)">
          {{ t("config.catalog.prev") }}
        </button>
        <span class="text-xs opacity-60 font-mono">{{ t("config.catalog.page", { page }) }}</span>
        <button class="btn btn-sm bg-base-100" type="button" :disabled="!hasNext || loading" @click="gotoPage(page + 1)">
          {{ t("config.catalog.next") }}
        </button>
      </div>
    </div>

    <!-- 条目详情弹窗 -->
    <dialog ref="detailModalRef" class="modal">
      <div class="modal-box max-w-2xl p-0">
        <div class="flex items-start justify-between gap-3 border-b border-base-300 px-4 py-3">
          <div class="min-w-0">
            <div class="flex flex-wrap items-center gap-2">
              <h3 class="text-sm font-bold text-base-content">{{ detailEntry?.name }}</h3>
              <span
                v-if="detailEntry?.requiredEnv.length"
                class="badge badge-xs badge-outline border-warning/40 text-warning"
              >
                {{ t("config.catalog.needEnvBadge") }}
              </span>
            </div>
            <div class="mt-0.5 flex flex-wrap items-center gap-x-2 text-caption text-base-content/50">
              <span v-if="detailEntry?.author">{{ detailEntry.author }}</span>
              <span v-if="detailEntry?.categories.length">· {{ detailEntry.categories.join(" / ") }}</span>
              <span v-if="detailEntry?.popularity">· {{ formatPopularity(detailEntry.popularity) }}</span>
            </div>
          </div>
          <button class="btn btn-ghost btn-circle h-9 w-9 min-h-[2.25rem] shrink-0" type="button" @click="closeDetail">✕</button>
        </div>

        <OverlayScrollArea scroller-class="max-h-[60vh] p-4">
          <div class="space-y-4">
            <!-- 完整说明：一字不裁剪 -->
            <p
              v-if="detailEntry?.description"
              class="whitespace-pre-wrap break-words text-sm leading-relaxed text-base-content/80"
            >
              {{ detailEntry.description }}
            </p>
            <p v-else class="text-sm italic text-base-content/40">{{ t("config.catalog.noDescription") }}</p>

            <!-- MCP：工具清单 -->
            <div v-if="detailEntry?.kind === 'mcp'" class="space-y-1.5">
              <div class="flex items-center gap-2">
                <span class="text-xs font-semibold text-base-content/70">{{ t("config.catalog.sectionTools") }}</span>
                <span class="badge badge-xs badge-neutral">{{ detailEntry.tools.length }}</span>
              </div>
              <ul v-if="detailEntry.tools.length" class="flex flex-wrap gap-1.5">
                <li
                  v-for="tool in detailEntry.tools"
                  :key="tool"
                  class="rounded-md bg-base-200/70 px-2 py-1 font-mono text-caption text-base-content/70"
                >
                  {{ tool }}
                </li>
              </ul>
              <p v-else class="text-caption text-base-content/45">{{ t("config.catalog.toolsEmpty") }}</p>
            </div>

            <!-- MCP：需要填写的环境变量 -->
            <div v-if="detailEntry?.kind === 'mcp'" class="space-y-1.5">
              <span class="text-xs font-semibold text-base-content/70">{{ t("config.catalog.sectionEnv") }}</span>
              <ul v-if="detailEntry.requiredEnv.length" class="space-y-0.5">
                <li v-for="name in detailEntry.requiredEnv" :key="name" class="font-mono text-caption text-base-content/80">
                  {{ name }}
                </li>
              </ul>
              <p v-else class="text-caption text-base-content/45">{{ t("config.catalog.envEmpty") }}</p>
            </div>

            <!-- MCP：启动配置原文 -->
            <div v-if="detailEntry?.kind === 'mcp'" class="space-y-1.5">
              <span class="text-xs font-semibold text-base-content/70">{{ t("config.catalog.sectionConfig") }}</span>
              <pre
                v-if="detailEntry.definitionJson"
                class="max-h-60 overflow-auto rounded-lg border border-base-300 bg-base-200/40 p-3 font-mono text-caption leading-relaxed select-text"
              >{{ detailEntry.definitionJson }}</pre>
              <p v-else class="text-caption text-base-content/45">{{ t("config.catalog.configEmpty") }}</p>
            </div>

            <!-- 来源地址：可复制 -->
            <div v-if="detailEntry?.homepage" class="space-y-1.5">
              <span class="text-xs font-semibold text-base-content/70">{{ t("config.catalog.sectionSource") }}</span>
              <code class="block break-all rounded-lg border border-base-300 bg-base-200/40 p-3 font-mono text-caption text-base-content/70 select-all">
                {{ detailEntry.homepage }}
              </code>
            </div>
          </div>
        </OverlayScrollArea>

        <div class="flex items-center justify-end gap-2 border-t border-base-300 px-4 py-3">
          <button class="btn btn-sm bg-base-200" type="button" @click="closeDetail">{{ t("common.close") }}</button>
          <button
            v-if="detailEntry"
            class="btn btn-sm btn-primary gap-1.5 px-3.5"
            type="button"
            :disabled="!detailEntry.installReady || installingId === entryKey(detailEntry)"
            :title="detailEntry.installReady ? '' : t('config.catalog.notInstallable')"
            @click="installFromDetail"
          >
            <span v-if="installingId === entryKey(detailEntry)" class="loading loading-spinner loading-xs"></span>
            <Download v-else class="h-3.5 w-3.5" />
            <span>{{ t("config.catalog.install") }}</span>
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop"><button aria-label="close">close</button></form>
    </dialog>

    <!-- 环境变量填写弹窗 -->
    <dialog ref="envModalRef" class="modal">
      <div class="modal-box max-w-lg p-4">
        <div class="flex items-center justify-between border-b border-base-300 pb-3">
          <div class="min-w-0">
            <h3 class="text-sm font-bold truncate">{{ pendingEntry?.name }}</h3>
            <p class="text-caption opacity-60">{{ t("config.catalog.envHint") }}</p>
          </div>
          <button class="btn btn-ghost btn-circle h-9 w-9 min-h-[2.25rem]" type="button" @click="closeEnvModal">✕</button>
        </div>
        <div class="py-4 space-y-3">
          <div v-for="name in pendingEntry?.requiredEnv || []" :key="name" class="flex flex-col gap-1">
            <label class="text-caption font-semibold opacity-70">{{ name }}</label>
            <input
              v-model="envDraft[name]"
              type="text"
              class="input input-bordered input-sm h-9 w-full text-xs font-mono"
              :placeholder="name"
            />
          </div>
          <div v-if="envError" class="text-xs text-error">{{ envError }}</div>
        </div>
        <div class="modal-action mt-0 flex justify-end gap-2">
          <button class="btn btn-sm bg-base-200" type="button" @click="closeEnvModal">{{ t("common.cancel") }}</button>
          <button class="btn btn-sm btn-primary" type="button" :disabled="!!installingId" @click="confirmEnvInstall">
            {{ t("config.catalog.install") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop"><button aria-label="close">close</button></form>
    </dialog>
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, type ComponentPublicInstance } from "vue";
import { useI18n } from "vue-i18n";
import { Download, Package, Plug, RefreshCw, Search, Code } from "@lucide/vue";
import type { CatalogEntry, CatalogSourceInfo } from "../../../../types/app";
import {
  installTransportCatalogEntry,
  invokeTauri,
  listTransportCatalogSources,
  searchTransportCatalog,
  setTransportSkillEnabled,
} from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";

const { t } = useI18n();

const PAGE_SIZE = 30;
// MCP 卡工具清单最多平铺几个，其余折算为 +N。
const TOOL_PREVIEW_LIMIT = 6;

const kind = ref<"mcp" | "skill">("mcp");
const sources = ref<CatalogSourceInfo[]>([]);
const sourceId = ref("");
const query = ref("");
const page = ref(1);
const entries = ref<CatalogEntry[]>([]);
const total = ref(-1);
const fromCache = ref(false);
const updatedAt = ref("");
const loading = ref(false);
const installingId = ref<string | null>(null);
const togglingKey = ref<string | null>(null);
const statusText = ref("");
const statusError = ref(false);

const envModalRef = ref<HTMLDialogElement | null>(null);
const pendingEntry = ref<CatalogEntry | null>(null);
const envDraft = ref<Record<string, string>>({});
const envError = ref("");

const detailModalRef = ref<HTMLDialogElement | null>(null);
const detailEntry = ref<CatalogEntry | null>(null);

// 说明被裁剪的条目键集合：只有这些才需要「详情」入口。
const descEls = new Map<string, HTMLElement>();
const truncatedKeys = ref<Set<string>>(new Set());

const sourceDescription = computed(
  () => sources.value.find((source) => source.id === sourceId.value)?.description || ""
);

const hasNext = computed(() => {
  if (total.value >= 0) return page.value * PAGE_SIZE < total.value;
  return entries.value.length >= PAGE_SIZE;
});

function entryKey(entry: CatalogEntry): string {
  return `${entry.source}:${entry.id}`;
}

// 说明被裁剪时才需要「详情」入口。
function detailNeeded(entry: CatalogEntry): boolean {
  return truncatedKeys.value.has(entryKey(entry));
}

// 卡片说明段落的 ref 收集器：用于判断是否被 line-clamp 裁剪。
function setDescRef(key: string) {
  return (el: Element | ComponentPublicInstance | null) => {
    if (el instanceof HTMLElement) {
      descEls.set(key, el);
    } else {
      descEls.delete(key);
    }
  };
}

// 用滚动高度与可见高度比对，判断哪些说明实际被裁剪。
function measureTruncation() {
  const next = new Set<string>();
  for (const [key, el] of descEls) {
    if (el.scrollHeight > el.clientHeight + 1) next.add(key);
  }
  truncatedKeys.value = next;
}

async function openDetail(entry: CatalogEntry) {
  detailEntry.value = entry;
  await nextTick();
  detailModalRef.value?.showModal();
}

function closeDetail() {
  detailModalRef.value?.close();
  detailEntry.value = null;
}

function installFromDetail() {
  const entry = detailEntry.value;
  if (!entry) return;
  closeDetail();
  requestInstall(entry);
}

function setStatus(text: string, isError = false) {
  statusText.value = text;
  statusError.value = isError;
}

function formatPopularity(value: number): string {
  if (!value || value <= 0) return "0";
  if (value >= 100000000) return `${(value / 100000000).toFixed(1)} 亿`;
  if (value >= 10000) return `${(value / 10000).toFixed(1)} 万`;
  return value.toLocaleString();
}

async function loadSources() {
  try {
    sources.value = await listTransportCatalogSources(kind.value);
    if (!sources.value.some((source) => source.id === sourceId.value)) {
      sourceId.value = sources.value[0]?.id || "";
    }
  } catch (error) {
    sources.value = [];
    sourceId.value = "";
    setStatus(`${t("config.catalog.loadSourcesFailed")}: ${toErrorMessage(error)}`, true);
  }
}

async function reload() {
  if (!sourceId.value) {
    await loadSources();
    if (!sourceId.value) return;
  }
  loading.value = true;
  try {
    const result = await searchTransportCatalog({
      source: sourceId.value,
      query: query.value.trim(),
      page: page.value,
      pageSize: PAGE_SIZE,
    });
    entries.value = result?.entries || [];
    total.value = result?.total ?? -1;
    fromCache.value = !!result?.fromCache;
    updatedAt.value = result?.updatedAt || "";
    if (result?.source) sourceId.value = result.source;
    if ((result?.kind === "mcp" || result?.kind === "skill") && result.kind !== kind.value) {
      kind.value = result.kind;
    }
    setStatus("");
    // 等卡片渲染完成后再判断哪些说明被裁剪。
    await nextTick();
    measureTruncation();
  } catch (error) {
    entries.value = [];
    setStatus(`${t("config.catalog.searchFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function switchKind(next: "mcp" | "skill") {
  if (kind.value === next) return;
  kind.value = next;
  sourceId.value = "";
  query.value = "";
  page.value = 1;
  entries.value = [];
  await loadSources();
  await reload();
}

function onSourceChange() {
  page.value = 1;
  void reload();
}

function runSearch() {
  page.value = 1;
  void reload();
}

function gotoPage(next: number) {
  if (next < 1) return;
  page.value = next;
  void reload();
}

function requestInstall(entry: CatalogEntry) {
  if (!entry.installReady || installingId.value) return;
  if (entry.requiredEnv.length === 0) {
    void doInstall(entry, {});
    return;
  }
  pendingEntry.value = entry;
  envDraft.value = {};
  for (const name of entry.requiredEnv) envDraft.value[name] = "";
  envError.value = "";
  envModalRef.value?.showModal();
}

function closeEnvModal() {
  envModalRef.value?.close();
  pendingEntry.value = null;
}

function confirmEnvInstall() {
  const entry = pendingEntry.value;
  if (!entry) return;
  const missing = entry.requiredEnv.filter((name) => !String(envDraft.value[name] || "").trim());
  if (missing.length > 0) {
    envError.value = t("config.catalog.envRequired", { names: missing.join("、") });
    return;
  }
  envError.value = "";
  const values = { ...envDraft.value };
  closeEnvModal();
  void doInstall(entry, values);
}

async function doInstall(entry: CatalogEntry, envValues: Record<string, string>) {
  installingId.value = entryKey(entry);
  setStatus("");
  try {
    const result = await installTransportCatalogEntry({
      source: entry.source,
      entryId: entry.id,
      envValues,
    });
    setStatus(
      t("config.catalog.installSuccess", {
        name: entry.name,
        id: result?.localId || "",
      })
    );
    // 安装后回读列表，让该条目就地变成启用开关。
    await reload();
  } catch (error) {
    setStatus(`${t("config.catalog.installFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    installingId.value = null;
  }
}

/// 切换已安装条目的启用状态；Skill 走全局启用开关，MCP 走部署/停止。
async function toggleEntry(entry: CatalogEntry) {
  if (!entry.localId || togglingKey.value) return;
  const next = !entry.enabled;
  togglingKey.value = entryKey(entry);
  setStatus("");
  try {
    if (entry.kind === "skill") {
      await setTransportSkillEnabled(entry.localId, next);
    } else if (next) {
      await invokeTauri("mcp_deploy_server", { input: { serverId: entry.localId } });
    } else {
      await invokeTauri("mcp_undeploy_server", { input: { serverId: entry.localId } });
    }
    entry.enabled = next;
  } catch (error) {
    setStatus(`${t("config.catalog.toggleFailed")}: ${toErrorMessage(error)}`, true);
    // 失败时回读真实状态，避免开关停在与后端不一致的位置。
    await reload();
  } finally {
    togglingKey.value = null;
  }
}

function handleResize() {
  measureTruncation();
}

onMounted(async () => {
  window.addEventListener("resize", handleResize);
  await loadSources();
  await reload();
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", handleResize);
});
</script>
