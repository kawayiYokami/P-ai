<template>
  <div class="flex h-screen w-full overflow-hidden bg-base-200/40 text-base-content">
    <!-- 左：分组 + 组件 -->
    <aside class="flex w-64 shrink-0 flex-col border-r border-base-300 bg-base-100">
      <div class="flex items-baseline gap-2 px-4 pb-3 pt-4">
        <h1 class="text-sm font-semibold tracking-wide">组件库</h1>
        <span class="text-xs text-base-content/40">{{ GALLERY_ENTRIES.length }}</span>
      </div>
      <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full">
        <nav class="px-2 pb-8">
          <div v-for="group in GALLERY_GROUPS" :key="group.id" class="mb-4">
            <p class="px-3 pb-1.5 text-xs font-medium tracking-wide text-base-content/40">{{ group.title }}</p>
            <ul class="space-y-0.5">
              <li v-for="item in group.entries" :key="item.key">
                <button
                  :ref="(el) => rememberItemEl(item.key, el)"
                  type="button"
                  :title="`${item.name} · ${item.title}`"
                  class="flex w-full items-center rounded-field px-3 py-1.5 text-left text-sm transition-colors"
                  :class="
                    item.key === activeKey
                      ? 'bg-primary/10 font-medium text-primary'
                      : 'text-base-content/70 hover:bg-base-200'
                  "
                  @click="select(item.key)"
                >
                  <span class="truncate">{{ item.title }}</span>
                </button>
              </li>
            </ul>
          </div>
        </nav>
      </OverlayScrollArea>
    </aside>

    <!-- 右：当前组件名 + 效果 -->
    <main class="flex min-w-0 flex-1 flex-col">
      <header class="flex shrink-0 items-baseline gap-2 border-b border-base-300 bg-base-100/60 px-6 py-3">
        <span class="truncate text-sm font-semibold">{{ currentEntry?.name }}</span>
        <span class="truncate text-sm text-base-content/60">{{ currentEntry?.title }}</span>
        <code class="ml-auto shrink-0 select-all whitespace-nowrap text-xs text-base-content/30">?key={{ activeKey }}</code>
      </header>
      <OverlayScrollArea class="min-h-0 flex-1" scroller-class="h-full">
        <div class="mx-auto w-full max-w-[1536px] p-6">
          <GalleryDetail :entry="currentEntry" />
        </div>
      </OverlayScrollArea>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import OverlayScrollArea from "../shared/components/OverlayScrollArea.vue";
import GalleryDetail from "./GalleryDetail.vue";
import { DEFAULT_GALLERY_KEY, GALLERY_ENTRIES, GALLERY_GROUPS, findGalleryEntry } from "./gallery-registry";

const activeKey = ref(findGalleryEntry(readKeyFromUrl())?.key ?? DEFAULT_GALLERY_KEY);

const currentEntry = computed(() => findGalleryEntry(activeKey.value));

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
