<template>
  <div class="grid gap-2">
    <button class="btn btn-sm" @click="$emit('loadArchives')">刷新归档</button>
    <div class="grid grid-cols-1 gap-1 max-h-56 overflow-auto">
      <button v-for="a in archives" :key="a.archiveId" class="btn btn-sm justify-start" @click="$emit('selectArchive', a.archiveId)">
        {{ a.archivedAt }} · {{ a.title }}
      </button>
    </div>
    <div class="divider my-1">归档内容</div>
    <div class="max-h-80 overflow-auto space-y-2">
      <div v-for="m in archiveMessages" :key="m.id" class="text-xs border border-base-300 rounded p-2">
        <div class="font-semibold uppercase text-[11px]">{{ m.role }}</div>
        <div class="whitespace-pre-wrap">{{ renderMessage(m) }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ArchiveSummary, ChatMessage } from "../types/app";

defineProps<{
  archives: ArchiveSummary[];
  archiveMessages: ChatMessage[];
  renderMessage: (msg: ChatMessage) => string;
}>();

defineEmits<{
  (e: "loadArchives"): void;
  (e: "selectArchive", archiveId: string): void;
}>();
</script>
