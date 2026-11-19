<template>
  <div class="drawer mx-auto max-w-[100rem] lg:drawer-open">
    <input id="gallery-drawer" type="checkbox" class="drawer-toggle" />

    <!-- 右：当前组件 + 效果 -->
    <div class="drawer-content">
      <div class="flex items-center gap-2 border-b border-base-content/10 px-6 py-2 lg:hidden">
        <label for="gallery-drawer" class="btn btn-ghost btn-sm drawer-button">选组件</label>
        <span class="truncate text-sm font-medium">{{ currentEntry?.title }}</span>
      </div>
      <div class="px-6 pb-16 pt-6">
        <div class="max-w-4xl">
          <header class="border-b border-base-content/10 pb-4">
            <div class="min-w-0">
              <h2 class="truncate text-2xl font-bold tracking-tight">{{ currentEntry?.title }}</h2>
              <p v-if="currentEntry?.name" class="mt-1.5 font-mono text-xs text-base-content/45">{{ currentEntry.name }}</p>
            </div>
          </header>
          <div class="pt-6">
            <GalleryDetail :entry="currentEntry" />
          </div>
        </div>
      </div>
    </div>

    <!-- 左：分组 + 组件 -->
    <div class="drawer-side">
      <label for="gallery-drawer" aria-label="关闭组件列表" class="drawer-overlay"></label>
      <aside class="flex h-full w-80 flex-col bg-base-100">
        <div class="px-5 pb-3 pt-5">
          <h1 class="text-base font-semibold tracking-tight">组件库</h1>
          <p class="mt-1 text-xs text-base-content/45">共 {{ GALLERY_ENTRIES.length }} 个，只在开发模式可见</p>
        </div>
        <label class="mx-3 mb-2 flex items-center gap-1.5 rounded-field bg-base-200 px-2.5 text-base-content/50">
          <AppIcon name="search" class="h-3.5 w-3.5 shrink-0" />
          <input
            v-model="keyword"
            type="search"
            aria-label="筛选组件"
            placeholder="筛选组件"
            class="h-8 grow bg-transparent text-sm text-base-content outline-none placeholder:text-base-content/40"
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
                  :title="item.title"
                  class="mb-0.5 flex min-h-9 w-full items-center rounded-field px-2.5 py-1.5 text-left text-sm transition-colors"
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
          <p v-if="!filteredGroups.length" class="px-2.5 py-6 text-xs text-base-content/40">没有匹配的组件，换个词试试。</p>
          </nav>
        </OverlayScrollArea>
      </aside>
    </div>
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
  // 换组件回顶，不然在长列表里点下一个，还停在上一个的滚动位置。
  window.scrollTo({ top: 0 });
  // 直达参数同步进地址栏，截图与分享都能直接落到这个组件。
  const url = new URL(window.location.href);
  url.searchParams.set("key", key);
  window.history.replaceState(null, "", url);
}

onMounted(() => {
  itemEls.get(activeKey.value)?.scrollIntoView({ block: "center" });
});
</script>
