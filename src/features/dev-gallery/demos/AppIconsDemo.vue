<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <label class="input input-bordered input-sm flex items-center gap-2">
        <AppIcon name="search" class="h-3.5 w-3.5 opacity-50" />
        <input v-model="filter" class="grow bg-transparent outline-none" placeholder="按语义名过滤" />
      </label>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-1.5">
        <div
          v-for="name in filteredNames"
          :key="name"
          class="flex min-w-0 flex-col items-center gap-1.5 rounded-lg border border-base-300/70 bg-base-200/40 px-1.5 py-2"
        >
          <AppIcon :name="name" class="h-6 w-6" />
          <span class="w-full truncate text-center font-mono text-[10px] leading-tight text-base-content/60" :title="name">{{ name }}</span>
        </div>
      </div>
      <p v-if="!filteredNames.length" class="py-6 text-center text-xs text-base-content/50">
        没有匹配「{{ filter }}」的语义名
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import { APP_ICON_MAP, type AppIconName } from "../../shared/icons/icon-map";

/** 图标语义名总表：直接取映射表的键，杜绝手抄漏项 */
const iconNames = Object.keys(APP_ICON_MAP).sort() as AppIconName[];
const filter = ref("");
const filteredNames = computed(() => {
  const keyword = filter.value.trim().toLowerCase();
  return keyword ? iconNames.filter((name) => name.includes(keyword)) : iconNames;
});
</script>
