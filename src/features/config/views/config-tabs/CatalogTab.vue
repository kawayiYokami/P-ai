<template>
  <SettingsPageShell :tab="kind" :tabs="catalogTabs" @update:tab="switchKind">
    <template #actions>
      <button
        class="btn btn-ghost btn-sm h-8 min-h-[2rem] gap-1.5 px-2.5 text-base-content/70 hover:text-base-content"
        type="button"
        :disabled="loading"
        :title="t('config.catalog.refresh')"
        @click="reload"
      >
        <AppIcon name="refresh-cw" class="h-3.5 w-3.5" :class="{ 'animate-spin': loading }" />
        <span>{{ t("config.catalog.refresh") }}</span>
      </button>
    </template>

    <!-- 检索与列表 -->
    <div class="grid grid-cols-1 gap-4 pb-8">
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
          <AppIcon name="search" class="absolute left-2.5 top-2.5 h-4 w-4 opacity-50 pointer-events-none" />
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
        <AppIcon name="refresh-cw" class="mr-2 h-4 w-4 animate-spin" />
        <span>{{ t("config.catalog.loading") }}</span>
      </div>
      <div v-else-if="entries.length === 0" class="rounded-box border border-dashed border-base-300 p-8 text-center">
        <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-base-200 text-base-content/50">
          <AppIcon name="package" class="h-6 w-6" />
        </div>
        <div class="mt-3 text-sm font-medium">{{ t("config.catalog.empty") }}</div>
        <div class="mt-1 text-xs opacity-60">{{ t("config.catalog.emptyHint") }}</div>
      </div>

      <!-- 条目卡片 -->
      <div v-else class="config-grid-auto-sm">
        <template v-for="entry in entries" :key="entry.source + ':' + entry.id">
          <CatalogEntryCard
            :entry="entry"
            :icon-url="iconUrlOf(entry)"
            :install-busy="installingId === entryKey(entry)"
            :toggle-busy="togglingKey === entryKey(entry)"
            :icon-ref="setIconRef(entryKey(entry))"
            @open="openDetail(entry)"
            @install="requestInstall(entry)"
            @toggle="toggleEntry(entry)"
            @icon-error="dropIcon(entry)"
          />
        </template>
      </div>

      <!-- 分页；来源不提供总数时整块隐藏 -->
      <div v-if="entries.length > 0 && !hidePager" class="flex items-center justify-center gap-3 pt-1">
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
                  class="rounded-selector bg-base-200/70 px-2 py-1 font-mono text-caption text-base-content/70"
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
                class="max-h-60 overflow-auto rounded-field border border-base-300 bg-base-200/40 p-3 font-mono text-caption leading-relaxed select-text"
              >{{ detailEntry.definitionJson }}</pre>
              <p v-else class="text-caption text-base-content/45">{{ t("config.catalog.configEmpty") }}</p>
            </div>

            <!-- 来源地址：外链按钮 -->
            <div v-if="detailEntry?.homepage" class="space-y-1">
              <span class="text-xs font-semibold text-base-content/70">{{ t("config.catalog.sectionSource") }}</span>
              <button
                type="button"
                class="link link-primary block break-all text-left font-mono text-caption"
                @click="openSourceUrl(detailEntry.homepage)"
              >
                {{ detailEntry.homepage }}
              </button>
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
            <AppIcon name="download" v-else class="h-3.5 w-3.5" />
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
  </SettingsPageShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, type ComponentPublicInstance } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import type { CatalogEntry, CatalogSourceInfo } from "../../../../types/app";
import {
  fetchCatalogIconDataUrl,
  fetchCatalogSkillDescriptions,
  installTransportCatalogEntry,
  invokeTauri,
  listTransportCatalogSources,
  openTransportExternalUrl,
  searchTransportCatalog,
  setTransportSkillEnabled,
} from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import SettingsPageShell from "../../components/SettingsPageShell.vue";
import CatalogEntryCard from "./CatalogEntryCard.vue";
import type { UnderlineTabItem } from "../../../shared/components/UnderlineTabs.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";

const { t } = useI18n();

const PAGE_SIZE = 30;

const kind = ref<"mcp" | "skill">("mcp");

const catalogTabs = computed<UnderlineTabItem[]>(() => [
  { key: "mcp", label: t("config.tabs.mcp") },
  { key: "skill", label: t("config.tabs.skill") },
]);

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

// 图标地址 → data URL。同址只取一次，跨页复用，重渲染不重复拉。
const iconDataUrl = ref(new Map<string, string>());
// 已发起过请求的地址：元素重建会导致重新 observe，靠集合挡住重复请求。
const iconRequested = new Set<string>();
const iconEls = new Map<string, HTMLElement>();
let iconObserver: IntersectionObserver | null = null;

// 没有图标时退化成首字母方块，不留空框也不显示碎图。
function iconInitial(entry: CatalogEntry): string {
  const text = String(entry.author || entry.name || "").trim();
  return (text[0] || "?").toUpperCase();
}

function iconUrlOf(entry: CatalogEntry): string {
  return entry.icon ? iconDataUrl.value.get(entry.icon) || "" : "";
}

function dropIcon(entry: CatalogEntry) {
  if (entry.icon) iconDataUrl.value.delete(entry.icon);
}

// 取图并发闸：一页 30 条进可视区会同时触发（rootMargin 160px 一次覆盖整屏），
// 全发出去容易撞第三方限流；而失败不重试，那批图标会永久退成首字母方块。
// 上限 6，兼顾首屏出图速度与对端压力。
const ICON_CONCURRENCY = 6;
let iconInFlight = 0;
const iconQueue: string[] = [];

async function runIconRequest(address: string) {
  try {
    const dataUrl = await fetchCatalogIconDataUrl(address);
    if (dataUrl) iconDataUrl.value.set(address, dataUrl);
  } catch {
    // 取不到就退首字母，不重试也不打断列表。
  } finally {
    iconInFlight -= 1;
    pumpIconQueue();
  }
}

function pumpIconQueue() {
  while (iconInFlight < ICON_CONCURRENCY && iconQueue.length > 0) {
    const address = iconQueue.shift();
    if (!address) return;
    iconInFlight += 1;
    void runIconRequest(address);
  }
}

function loadIcon(address: string) {
  if (!address || iconRequested.has(address)) return;
  iconRequested.add(address);
  iconQueue.push(address);
  pumpIconQueue();
}

// 图标进可视区才取图：没翻到的条目不发请求，发出去的也按 ICON_CONCURRENCY 排队。
function setIconRef(key: string) {
  return (el: Element | ComponentPublicInstance | null) => {
    if (el instanceof HTMLElement) {
      iconEls.set(key, el);
      if (iconObserver) {
        iconObserver.observe(el);
      } else {
        loadIcon(el.dataset.icon || "");
      }
      return;
    }
    const current = iconEls.get(key);
    if (current) iconObserver?.unobserve(current);
    iconEls.delete(key);
  };
}

function handleIconIntersect(records: IntersectionObserverEntry[]) {
  for (const record of records) {
    if (!record.isIntersecting) continue;
    iconObserver?.unobserve(record.target);
    loadIcon((record.target as HTMLElement).dataset.icon || "");
  }
}

const sourceDescription = computed(
  () => sources.value.find((source) => source.id === sourceId.value)?.description || ""
);

const hasNext = computed(() => {
  if (total.value >= 0) return page.value * PAGE_SIZE < total.value;
  return entries.value.length >= PAGE_SIZE;
});

// 来源不提供总数（total = -1）就没有分页可翻，翻页控件整体隐藏。ClawHub 检索只给相关性 top-N，属于这种。
const hidePager = computed(() => total.value < 0);

function entryKey(entry: CatalogEntry): string {
  return `${entry.source}:${entry.id}`;
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

function openSourceUrl(url?: string) {
  const target = String(url || "").trim();
  if (!target) return;
  void openTransportExternalUrl(target);
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

/** 简介回填的世代号：列表一换，上一批迟到的结果就作废，不许覆盖新内容。 */
const descriptionGeneration = ref(0);

/**
 * 列表已经渲染出来了，再回头把缺的简介补上。
 * skills.sh 的检索与热门榜都不带简介，只能按 id 去详情页取；这一步绝不能挡在列表前面，
 * 否则一页 30 条就要等 30 个页面抓完。
 */
async function fillMissingDescriptions(list: CatalogEntry[]) {
  const ids = list.filter((entry) => !entry.description.trim()).map((entry) => entry.id);
  if (ids.length === 0) return;
  const generation = ++descriptionGeneration.value;
  try {
    const filled = await fetchCatalogSkillDescriptions(ids);
    if (generation !== descriptionGeneration.value) return;
    for (const entry of entries.value) {
      const description = filled[entry.id];
      if (description && !entry.description.trim()) entry.description = description;
    }
  } catch {
    // 简介是补充信息，取不到就留着占位，不影响列表与安装。
  }
}

async function reload() {
  if (!sourceId.value) {
    await loadSources();
    if (!sourceId.value) return;
  }
  const trimmedQuery = query.value.trim();
  loading.value = true;
  try {
    const result = await searchTransportCatalog({
      source: sourceId.value,
      query: trimmedQuery,
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
    // 列表已经回到界面上了，简介在后面自己补，不占这条等待。
    void fillMissingDescriptions(entries.value);
  } catch (error) {
    entries.value = [];
    setStatus(`${t("config.catalog.searchFailed")}: ${toErrorMessage(error)}`, true);
  } finally {
    loading.value = false;
  }
}

async function switchKind(next: string) {
  if (next !== "mcp" && next !== "skill") return;
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

onMounted(async () => {
  if (typeof IntersectionObserver !== "undefined") {
    iconObserver = new IntersectionObserver(handleIconIntersect, { rootMargin: "160px" });
  }
  await loadSources();
  await reload();
});

onBeforeUnmount(() => {
  iconObserver?.disconnect();
  iconObserver = null;
  iconEls.clear();
});
</script>
