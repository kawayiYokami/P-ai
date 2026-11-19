<template>
  <div class="min-w-0">
    <component :is="impl" v-if="impl" :key="props.entry?.key" />
    <p v-else class="text-sm text-base-content/50">左侧选一个组件。</p>
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, h, shallowRef, watch, type Component } from "vue";
import type { GalleryEntry } from "./types";

const props = defineProps<{ entry?: GalleryEntry | null }>();

/** 已加载过的组件实例缓存：来回切换不重复构建，也不重复跑挂载逻辑。 */
const cache = new Map<string, Component>();

const failed = shallowRef("");
watch(
  () => props.entry?.key,
  () => {
    failed.value = "";
  },
);

const impl = computed<Component | null>(() => {
  const entry = props.entry;
  if (!entry) return null;
  const cached = cache.get(entry.key);
  if (cached) return cached;
  const asyncComponent = defineAsyncComponent({
    loader: entry.loader as () => Promise<{ default: Component }>,
    delay: 0,
    timeout: 30000,
    loadingComponent: {
      render: () =>
        h("p", { class: "text-sm text-base-content/50" }, "加载中…"),
    },
    errorComponent: {
      render: () =>
        h(
          "p",
          { class: "text-sm text-error" },
          failed.value || `内容加载失败：demos/${entry.key}`,
        ),
    },
    onError: (_error, _retry, fail) => {
      failed.value = `内容加载失败：demos/${entry.key}`;
      fail();
    },
  });
  cache.set(entry.key, asyncComponent);
  return asyncComponent;
});
</script>
