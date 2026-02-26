<template>
  <div class="modal-box w-[92vw] max-w-none flex min-h-0 flex-col overflow-hidden">
    <h3 class="font-semibold text-sm mb-2">{{ title }}</h3>
    <input
      ref="importInputRef"
      type="file"
      accept=".json,application/json"
      class="hidden"
      @change="$emit('importFile', $event)"
    />
    <div v-if="memoryList.length === 0" class="flex-1 min-h-0 text-sm opacity-70">{{ emptyText }}</div>
    <div v-else class="flex flex-1 min-h-0 flex-col space-y-2">
      <div class="min-h-0 flex-1 overflow-auto space-y-2 pr-1">
        <div
          v-for="memory in pagedMemories"
          :key="memory.id"
          class="card card-compact bg-base-100 border border-base-300 shadow-md"
        >
          <div class="card-body text-sm p-3">
            <div class="badge badge-sm">{{ memory.memoryType }}</div>
            <div class="mt-1 whitespace-pre-wrap break-words">{{ memory.judgment }}</div>
            <div v-if="memory.reasoning" class="mt-1 opacity-80 whitespace-pre-wrap break-words">{{ memory.reasoning }}</div>
            <div class="mt-2 flex flex-wrap items-center gap-1">
              <span
                v-for="(kw, kwIdx) in memory.tags"
                :key="`${memory.id}-kw-${kwIdx}`"
                class="badge badge-sm badge-ghost"
              >
                {{ kw }}
              </span>
              <span v-if="memory.tags.length === 0" class="text-[11px] opacity-60">-</span>
            </div>
          </div>
        </div>
      </div>
      <div class="shrink-0 flex items-center justify-between border-t border-base-300 pt-2">
        <span class="text-sm opacity-70">{{ pageText }}</span>
        <div class="join">
          <button class="btn btn-sm join-item" :disabled="memoryPage <= 1" @click="$emit('prevPage')">{{ prevPageText }}</button>
          <button class="btn btn-sm join-item" :disabled="memoryPage >= memoryPageCount" @click="$emit('nextPage')">{{ nextPageText }}</button>
        </div>
      </div>
    </div>
    <div class="modal-action mt-2 shrink-0">
      <button class="btn btn-sm btn-ghost" @click="$emit('exportMemories')">{{ exportText }}</button>
      <button class="btn btn-sm btn-ghost" @click="$emit('triggerImport')">{{ importText }}</button>
      <button class="btn btn-sm" @click="$emit('close')">{{ closeText }}</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";

defineProps<{
  title: string;
  emptyText: string;
  pageText: string;
  prevPageText: string;
  nextPageText: string;
  exportText: string;
  importText: string;
  closeText: string;
  memoryList: Array<{ id: string; memoryType: string; judgment: string; reasoning: string; tags: string[] }>;
  pagedMemories: Array<{ id: string; memoryType: string; judgment: string; reasoning: string; tags: string[] }>;
  memoryPage: number;
  memoryPageCount: number;
}>();

defineEmits<{
  close: [];
  prevPage: [];
  nextPage: [];
  exportMemories: [];
  triggerImport: [];
  importFile: [event: Event];
}>();

const importInputRef = ref<HTMLInputElement | null>(null);

defineExpose({
  importInputRef,
});
</script>
