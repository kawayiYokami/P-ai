<template>
  <CardShell
    :col-span="colSpan"
    :row-span="rowSpan"
    tone="info"
    icon="refresh-cw"
    :label="t('chat.homePanel.automationLabel')"
  >
    <template #trailing>
      <span class="ecall-home-num shrink-0 text-xs text-base-content/45">{{ tasks.length }}</span>
    </template>
    <ul class="flex w-full flex-col overflow-y-auto text-xs">
      <li
        v-for="(task, index) in tasks"
        :key="task.id"
        class="flex min-w-0 items-center gap-2 p-1"
        :title="task.id"
      >
        <span class="ecall-home-num shrink-0 text-base-content/45">{{ index + 1 }}</span>
        <span class="min-w-0 flex-1 truncate text-base-content/80">{{ task.description || task.id }}</span>
      </li>
    </ul>
  </CardShell>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import CardShell from "./CardShell.vue";

withDefaults(defineProps<{
  /** 当前会话的自动化任务：一个任务一行，只作提示，不可关闭 */
  tasks?: Array<{ id: string; description: string }>;
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
}>(), {
  tasks: () => [],
  colSpan: 2,
  rowSpan: 1,
});

const { t } = useI18n();
</script>

<style scoped>
.ecall-home-num {
  font-variant-numeric: tabular-nums;
}
</style>
