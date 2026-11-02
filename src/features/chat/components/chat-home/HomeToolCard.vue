<template>
  <CardShell
    :col-span="colSpan"
    :row-span="rowSpan"
    tone="info"
    :icon="Wrench"
    :label="t('chat.homePanel.toolLabel')"
    interactive
    @select="emit('open')"
  >
    <template #trailing>
      <span class="ecall-home-num shrink-0 text-xs text-base-content/45">{{ t("chat.homePanel.toolChangedFiles", { n: totalFileCount }) }}</span>
    </template>
    <ul v-if="visibleFiles.length" class="menu menu-xs w-full gap-0.5 p-0">
      <li v-for="file in visibleFiles" :key="file.path">
        <div class="min-w-0 gap-2 font-normal" :title="file.path">
          <span class="min-w-0 flex-1 truncate text-base-content/80">{{ file.label }}</span>
          <span class="ecall-home-num shrink-0 font-mono text-xs">
            <span class="text-success">+{{ file.added }}</span>
            <span class="ml-1 text-error">-{{ file.deleted }}</span>
          </span>
        </div>
      </li>
    </ul>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Wrench } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import type { ToolReviewBatchSummary } from "../../composables/use-chat-tool-review";

const MAX_VISIBLE_FILES = 4;

const props = withDefaults(defineProps<{
  /** 只含出现过工具调用的批次，按会话推进顺序（末尾最新），由 ChatHomePanel 过滤 */
  batches?: ToolReviewBatchSummary[];
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
}>(), {
  batches: () => [],
  colSpan: 2,
  rowSpan: 1,
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();

type FileStat = { path: string; label: string; added: number; deleted: number };

/** 卡片只讲最近一轮工具调用 */
const latestBatch = computed(() => (props.batches.length ? props.batches[props.batches.length - 1] : null));

/** 与后端 changed_files 同口径：只统计成功项涉及的文件，跨条目去重后按文件聚合增删行 */
const fileStats = computed<FileStat[]>(() => {
  const batch = latestBatch.value;
  if (!batch) return [];
  const map = new Map<string, FileStat>();
  for (const item of batch.items || []) {
    if (item.isSuccess === false) continue;
    const paths = (item.affectedPaths || [])
      .map((raw) => String(raw || "").replace(/\\/g, "/").trim())
      .filter(Boolean);
    if (!paths.length) continue;
    const added = Number(item.addedLines) || 0;
    const deleted = Number(item.deletedLines) || 0;
    // 一次调用可能涉及多个文件，按文件数分摊，避免同一批增删被重复计入每个文件
    const perAdded = Math.round(added / paths.length);
    const perDeleted = Math.round(deleted / paths.length);
    for (const path of paths) {
      const stat = map.get(path) || { path, label: baseName(path), added: 0, deleted: 0 };
      stat.added += perAdded;
      stat.deleted += perDeleted;
      map.set(path, stat);
    }
  }
  return [...map.values()];
});

const totalFileCount = computed(() => {
  const declared = Number(latestBatch.value?.changedFiles);
  return Number.isFinite(declared) && declared > 0 ? declared : fileStats.value.length;
});

const visibleFiles = computed(() => fileStats.value.slice(0, MAX_VISIBLE_FILES));

function baseName(path: string): string {
  const parts = path.split("/").filter(Boolean);
  return parts.pop() || path;
}
</script>

<style scoped>
.ecall-home-num {
  font-variant-numeric: tabular-nums;
}
</style>
