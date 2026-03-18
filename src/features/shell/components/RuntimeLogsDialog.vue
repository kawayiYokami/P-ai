<template>
  <dialog class="modal" :class="{ 'modal-open': open }">
    <div class="modal-box max-w-4xl h-[70vh] flex flex-col">
      <div class="flex items-center justify-between gap-2">
        <h3 class="font-semibold text-base">运行日志（内存）</h3>
        <div class="join">
          <button class="btn btn-sm join-item" :disabled="loading" @click="$emit('refresh')">刷新</button>
          <button class="btn btn-sm join-item" :disabled="loading || logs.length === 0" @click="$emit('clear')">清空</button>
          <button class="btn btn-sm btn-primary join-item" @click="$emit('close')">关闭</button>
        </div>
      </div>
      <div class="text-xs opacity-70 mt-1">仅保留内存日志，容量上限 10MB，进程退出即清空。</div>
      <div v-if="errorText" class="text-error text-sm mt-2">{{ errorText }}</div>
      <div
        ref="viewportRef"
        class="mt-3 flex-1 overflow-auto rounded-box border border-base-300 bg-base-100 p-2 font-mono text-xs"
        @scroll="onScroll"
      >
        <div :style="{ height: `${totalHeight}px`, position: 'relative' }">
          <div :style="{ transform: `translateY(${offsetTop}px)` }">
            <div
              v-for="item in visibleLogs"
              :key="item.id"
              class="leading-6 whitespace-pre-wrap break-all border-b border-base-300/40"
            >
              <span class="opacity-60">[{{ item.createdAt }}]</span>
              <span class="ml-2" :class="item.level === 'error' ? 'text-error' : 'text-base-content'">
                {{ item.level.toUpperCase() }}
              </span>
              <span class="ml-2">{{ item.message }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="$emit('close')">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { RuntimeLogEntry } from "../../../types/app";

const props = defineProps<{
  open: boolean;
  logs: RuntimeLogEntry[];
  loading: boolean;
  errorText: string;
}>();

defineEmits<{
  (e: "close"): void;
  (e: "refresh"): void;
  (e: "clear"): void;
}>();

const ITEM_HEIGHT = 24;
const OVERSCAN = 12;
const viewportRef = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(400);

function onScroll() {
  const el = viewportRef.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  viewportHeight.value = el.clientHeight;
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    requestAnimationFrame(() => {
      const el = viewportRef.value;
      if (!el) return;
      viewportHeight.value = el.clientHeight;
    });
  },
);

const totalHeight = computed(() => props.logs.length * ITEM_HEIGHT);
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / ITEM_HEIGHT) - OVERSCAN));
const endIndex = computed(() =>
  Math.min(
    props.logs.length,
    Math.ceil((scrollTop.value + viewportHeight.value) / ITEM_HEIGHT) + OVERSCAN,
  ),
);
const offsetTop = computed(() => startIndex.value * ITEM_HEIGHT);
const visibleLogs = computed(() => props.logs.slice(startIndex.value, endIndex.value));
</script>
