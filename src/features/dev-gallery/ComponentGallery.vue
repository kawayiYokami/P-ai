<template>
  <div class="flex h-screen w-full overflow-hidden bg-base-100 text-base-content">
    <!-- 左：分组 + 组件 -->
    <aside class="flex w-64 shrink-0 flex-col border-r border-base-content/10">
      <div class="px-5 pb-3 pt-5">
        <h1 class="text-base font-semibold tracking-tight">组件库</h1>
        <p class="mt-1 text-xs text-base-content/45">{{ GALLERY_ENTRIES.length }} 个组件 · 仅开发模式</p>
      </div>
      <label class="mx-3 mb-2 flex items-center gap-1.5 rounded-field bg-base-200 px-2.5 text-base-content/50">
        <AppIcon name="search" class="h-3.5 w-3.5 shrink-0" />
        <input
          v-model="keyword"
          type="search"
          placeholder="筛选组件"
          class="h-8 grow bg-transparent text-[13px] text-base-content outline-none placeholder:text-base-content/40"
        />
      </label>
      <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full">
        <nav class="px-3 pb-10">
          <div v-for="group in filteredGroups" :key="group.id">
            <p class="flex items-center gap-2 px-2.5 pb-1 pt-4 text-xs font-medium text-base-content/40">
              <span class="whitespace-nowrap">{{ group.title }}</span>
              <span class="h-px flex-1 bg-base-content/10" />
            </p>
            <ul>
              <li v-for="item in group.entries" :key="item.key">
                <button
                  :ref="(el) => rememberItemEl(item.key, el)"
                  type="button"
                  :title="`${item.name} · ${item.title}`"
                  class="mb-0.5 flex min-h-9 w-full items-center rounded-field px-2.5 py-1.5 text-left text-[13px] transition-colors"
                  :class="
                    item.key === activeKey
                      ? 'bg-neutral font-medium text-neutral-content'
                      : 'text-base-content/75 hover:bg-base-200'
                  "
                  @click="select(item.key)"
                >
                  <span class="truncate">{{ item.title }}</span>
                </button>
              </li>
            </ul>
          </div>
          <p v-if="!filteredGroups.length" class="px-2.5 py-6 text-xs text-base-content/40">没有匹配的组件。</p>
        </nav>
      </OverlayScrollArea>
    </aside>

    <!-- 右：当前组件 + 效果 -->
    <main class="flex min-w-0 flex-1 flex-col">
      <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full">
        <div class="mx-auto w-full max-w-5xl px-10 py-9">
          <header class="flex items-end justify-between gap-6 border-b border-base-content/10 pb-4">
            <div class="min-w-0">
              <h2 class="truncate text-2xl font-bold tracking-tight">{{ currentEntry?.name }}</h2>
              <p class="mt-1.5 text-sm text-base-content/55">{{ currentEntry?.title }}</p>
            </div>
            <code class="shrink-0 whitespace-nowrap rounded-field bg-base-200 px-2 py-1 font-mono text-xs text-base-content/45">?key={{ activeKey }}</code>
          </header>
          <div class="pt-6">
            <GalleryDetail :entry="currentEntry" />
          </div>
        </div>
      </OverlayScrollArea>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import OverlayScrollArea from "../shared/components/OverlayScrollArea.vue";
import AppIcon from "../shared/components/AppIcon.vue";
import GalleryDetail from "./GalleryDetail.vue";
import { DEFAULT_GALLERY_KEY, GALLERY_ENTRIES, GALLERY_GROUPS, findGalleryEntry } from "./gallery-registry";

const activeKey = ref(findGalleryEntry(readKeyFromUrl())?.key ?? DEFAULT_GALLERY_KEY);

const currentEntry = computed(() => findGalleryEntry(activeKey.value));

/** 40 项挤在一列，靠筛选比靠滚动快。 */
const keyword = ref("");
const filteredGroups = computed(() => {
  const word = keyword.value.trim().toLowerCase();
  if (!word) return GALLERY_GROUPS;
  return GALLERY_GROUPS.map((group) => ({
    ...group,
    entries: group.entries.filter((item) =>
      `${item.name} ${item.title} ${item.key}`.toLowerCase().includes(word),
    ),
  })).filter((group) => group.entries.length > 0);
});

/** 记住每项的 DOM，深链进来时把选中项滚到中间，别让人自己找。 */
const itemEls = new Map<string, HTMLElement>();
function rememberItemEl(key: string, el: unknown) {
  if (el instanceof HTMLElement) itemEls.set(key, el);
  else itemEls.delete(key);
}

function readKeyFromUrl(): string {
  return new URLSearchParams(window.location.search).get("key")?.trim() ?? "";
}

function select(key: string) {
  activeKey.value = key;
  // 直达参数同步进地址栏，截图与分享都能直接落到这个组件。
  const url = new URL(window.location.href);
  url.searchParams.set("key", key);
  window.history.replaceState(null, "", url);
}

onMounted(() => {
  itemEls.get(activeKey.value)?.scrollIntoView({ block: "center" });
});
</script>
