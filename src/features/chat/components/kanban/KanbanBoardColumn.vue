<template>
  <section
    class="flex h-full min-w-[220px] max-w-[320px] flex-1 flex-col rounded-box border border-base-content/10 bg-base-200/50 p-2.5"
  >
    <!-- 列顶栏：小圆点 + 状态名 + 数量统计 -->
    <header class="flex shrink-0 items-center justify-between border-b border-base-content/10 px-0.5 pb-2">
      <div class="flex items-center gap-1.5">
        <span class="size-2 rounded-full" :class="stageMeta.dotClass" aria-hidden="true" />
        <h3 class="text-xs font-semibold text-base-content/85">
          {{ t(stageMeta.labelKey) }}
        </h3>
        <span class="font-mono text-xs tabular-nums text-base-content/40">
          {{ issues.length }}
        </span>
      </div>
    </header>

    <!-- 卡片滚动区 -->
    <OverlayScrollArea
      class="min-h-0 w-full flex-1 pt-2"
      scroller-class="h-full flex flex-col gap-2 pr-0.5"
    >
      <template v-if="issues.length">
        <KanbanIssueCard
          v-for="issue in issues"
          :key="issue.number"
          :issue="issue"
          :assignee-name="resolveAssigneeName(issue.assignee)"
          :avatar-url="resolveAvatar(issue.assignee)"
          :approving-number="approvingNumber"
          @approve="emit('approve', $event)"
          @open-plan="emit('openPlan', $event)"
          @reference="emit('reference', $event)"
        />
      </template>
      <div
        v-else
        class="flex flex-1 min-h-24 items-center justify-center rounded-field border border-dashed border-base-content/10 p-3 text-center text-caption text-base-content/30 select-none"
      >
        {{ t("chat.kanban.cardEmptyColumn") || t("chat.kanban.groupEmpty") }}
      </div>
    </OverlayScrollArea>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import KanbanIssueCard from "./KanbanIssueCard.vue";
import {
  KANBAN_STAGE_CONFIGS,
  type KanbanIssue,
  type KanbanStageStatus,
} from "./kanban-types";

const props = defineProps<{
  status: KanbanStageStatus;
  issues: KanbanIssue[];
  approvingNumber?: number | null;
  resolveAssigneeName: (raw: string) => string;
  resolveAvatar: (raw: string) => string;
}>();

const emit = defineEmits<{
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "reference", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

const stageMeta = computed(() => {
  return KANBAN_STAGE_CONFIGS[props.status] || KANBAN_STAGE_CONFIGS.todo;
});
</script>
