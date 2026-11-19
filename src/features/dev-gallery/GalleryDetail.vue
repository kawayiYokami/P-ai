<template>
  <div class="min-w-0">
    <component :is="impl" v-if="impl" :key="props.entry?.key" v-bind="props.entry?.props ?? {}" />
    <p v-else class="text-sm text-base-content/50">左侧选一个组件。</p>
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, h, shallowRef, type Component } from "vue";
import type { GalleryEntry } from "./types";

const props = defineProps<{ entry?: GalleryEntry | null }>();

/** 已加载过的组件实例缓存：整页平铺时每项只构建一次，滚动来回不重复跑挂载逻辑。 */
const cache = new Map<string, Component>();

const failed = shallowRef("");

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
      render: () => h("p", { class: "text-sm text-base-content/50" }, "加载中…"),
    },
    errorComponent: {
      render: () =>
        h("p", { class: "text-sm text-error" }, failed.value || `内容加载失败：${entry.name}`),
    },
    onError: (_error, _retry, fail) => {
      failed.value = `内容加载失败：${entry.name}`;
      fail();
    },
  });
  cache.set(entry.key, asyncComponent);
  return asyncComponent;
});
</script>
