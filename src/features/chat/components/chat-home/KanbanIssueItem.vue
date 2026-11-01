<template>
  <div
    class="group/item flex flex-col gap-1.5 rounded-field border border-base-content/10 bg-base-100 p-2 shadow-2xs transition-all hover:border-base-content/25 hover:bg-base-200/30"
  >
    <!-- 顶行：编号 + （可选备注） -->
    <div class="flex items-center justify-between font-mono text-caption">
      <span class="font-mono font-medium text-xs tabular-nums text-base-content/50">
        #{{ issue.number }}
      </span>
      <span
        v-if="issue.note"
        class="max-w-[120px] truncate font-sans text-micro text-base-content/40"
        :title="issue.note"
      >
        {{ issue.note }}
      </span>
    </div>

    <!-- 中行：标题（支持 2 行，清晰易读） -->
    <div
      class="line-clamp-2 font-medium text-xs leading-snug text-base-content/90"
      :title="issue.title"
    >
      {{ issue.title }}
    </div>

    <!-- 底行：头像 + 人格名称 + 相对时间 -->
    <div class="flex items-center justify-between gap-1 pt-0.5 text-caption">
      <!-- 人格与头像 -->
      <div class="flex min-w-0 items-center gap-1.5 text-base-content/75">
        <div class="avatar size-4 shrink-0">
          <div
            class="flex size-4 items-center justify-center overflow-hidden rounded-full font-bold text-[9px] text-white"
            :style="avatarContainerStyle"
          >
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              :alt="assigneeName"
              class="size-full object-cover"
            />
            <span v-else>{{ initialChar }}</span>
          </div>
        </div>
        <span
          class="max-w-[90px] truncate font-medium text-xs text-base-content/75"
          :title="assigneeName"
        >
          {{ assigneeName }}
        </span>
      </div>

      <!-- 相对时间 -->
      <span class="shrink-0 text-micro tabular-nums text-base-content/40">
        {{ relativeTime }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { KanbanIssue } from "../../composables/use-kanban-board";

const props = defineProps<{
  issue: KanbanIssue;
  assigneeName: string;
  avatarUrl?: string;
  relativeTime: string;
}>();

const PALETTE = ["#6366f1", "#8b5cf6", "#ec4899", "#f59e0b", "#10b981", "#06b6d4", "#3b82f6"];

const avatarContainerStyle = computed(() => {
  if (props.avatarUrl) return {};
  const name = props.assigneeName;
  let hash = 0;
  for (let i = 0; i < name.length; i += 1) {
    hash = (hash << 5) - hash + name.charCodeAt(i);
  }
  const color = PALETTE[Math.abs(hash) % PALETTE.length];
  return { backgroundColor: color };
});

const initialChar = computed(() => {
  return props.assigneeName.charAt(0).toUpperCase() || "?";
});
</script>
