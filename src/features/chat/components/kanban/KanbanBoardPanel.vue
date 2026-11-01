<template>
  <div class="flex h-full min-h-0 w-full flex-col bg-base-100">
    <header class="flex shrink-0 items-center gap-2 border-b border-base-content/10 px-3 py-1.5">
      <!-- 回磁贴墙的入口：只留图标，尺寸与右侧刷新同档 -->
      <button
        type="button"
        class="btn btn-ghost btn-xs btn-square"
        :title="t('chat.kanban.backToTiles')"
        :aria-label="t('chat.kanban.backToTiles')"
        @click="emit('back')"
      >
        <LayoutGrid class="size-3.5" aria-hidden="true" />
      </button>
      <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ t("chat.kanban.boardTitle") }}</span>
      <span class="shrink-0 text-xs tabular-nums text-base-content/45">
        {{ t("chat.kanban.issueCount", { count: issues.length }) }}
      </span>
      <button
        type="button"
        class="btn btn-ghost btn-xs btn-square"
        :title="t('chat.kanban.refresh')"
        :aria-label="t('chat.kanban.refresh')"
        :disabled="loading"
        @click="emit('refresh')"
      >
        <RefreshCw class="size-3.5" :class="loading ? 'animate-spin' : ''" aria-hidden="true" />
      </button>
    </header>

    <div v-if="errorText" class="shrink-0 px-3 py-2 text-xs text-error">{{ errorText }}</div>

    <OverlayScrollArea v-if="issues.length" class="min-h-0 flex-1" scroller-class="h-full">
      <ul class="timeline timeline-vertical timeline-compact w-full px-3 py-2">
        <li
          v-for="(stage, stageIndex) in stages"
          :key="stage.status"
          :style="{ '--timeline-row-start': '0px' }"
        >
          <hr />
          <div class="timeline-middle">
            <span
              class="grid size-6 shrink-0 place-items-center rounded-full text-micro font-semibold tabular-nums"
              :class="stage.nodeClass"
            >
              <CircleAlert v-if="!stage.chainNumber" class="size-3.5" aria-hidden="true" />
              <template v-else>{{ stage.chainNumber }}</template>
            </span>
          </div>
          <div class="timeline-end w-full min-w-0 pb-2 pl-3">
            <header class="flex min-w-0 items-center gap-1.5">
              <span class="shrink-0 text-xs font-medium" :class="stage.titleClass">
                {{ t(stage.labelKey) }}
              </span>
              <span class="badge badge-xs shrink-0 border-base-content/10 bg-base-content/5 text-micro text-base-content/60">
                {{ stage.issues.length }}
              </span>
            </header>

            <p v-if="!stage.issues.length" class="mt-0.5 text-caption text-base-content/30">
              {{ t("chat.kanban.groupEmpty") }}
            </p>

            <ul v-else class="mt-0.5 flex flex-col">
              <li
                v-for="issue in stage.issues"
                :key="issue.number"
                class="flex min-w-0 items-start gap-2 rounded-field px-1 py-1 transition-colors hover:bg-base-200/70"
              >
                <span class="shrink-0 pt-0.5 text-caption font-medium tabular-nums text-base-content/40">
                  #{{ issue.number }}
                </span>
                <!-- 挂了计划的议题在编号后带一个文件图标：只占一个图标位，文件名走原生悬浮提示 -->
                <FileText
                  v-if="issue.planPath"
                  class="mt-0.5 size-3 shrink-0 text-base-content/40"
                  :title="planFileName(issue.planPath)"
                  :aria-label="planFileName(issue.planPath)"
                />
                <div class="min-w-0 flex-1">
                  <div class="flex min-w-0 items-center gap-2">
                    <span class="min-w-0 flex-1 truncate text-sm text-base-content/85" :title="issue.title">
                      {{ issue.title }}
                    </span>
                    <span
                      class="w-20 shrink-0 truncate text-right text-caption text-base-content/40"
                      :title="assigneeLabel(issue.assignee)"
                    >
                      {{ assigneeLabel(issue.assignee) }}
                    </span>
                    <!-- 通过按钮占固定槽位：没有按钮的行留空，负责人的右端才能落在同一条竖线上。 -->
                    <div class="flex w-16 shrink-0 items-center justify-end">
                      <button
                        v-if="issue.status === 'in_review'"
                        type="button"
                        class="btn btn-primary btn-xs"
                        :disabled="approvingNumber === issue.number"
                        :title="t('chat.kanban.approveHint')"
                        @click="emit('approve', issue.number)"
                      >
                        <Check class="size-3" aria-hidden="true" />
                        {{ t("chat.kanban.approve") }}
                      </button>
                    </div>
                  </div>
                  <p
                    v-if="issue.note && stage.needsAttention"
                    class="mt-0.5 line-clamp-1 text-caption leading-relaxed text-base-content/50"
                    :title="issue.note"
                  >
                    {{ issue.note }}
                  </p>
                </div>
              </li>
            </ul>
          </div>
          <!-- 末段的下半截线不画：时间线终止在最后一个节点，不再往内容底部延伸 -->
          <hr :class="{ hidden: stageIndex === stages.length - 1 }" />
        </li>
      </ul>
    </OverlayScrollArea>

    <div v-else-if="!errorText" class="flex min-h-0 flex-1 items-center justify-center px-6 text-center">
      <p class="text-xs leading-relaxed text-base-content/45">{{ t("chat.kanban.boardEmpty") }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Check, CircleAlert, FileText, LayoutGrid, RefreshCw } from "@lucide/vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import {
  KANBAN_STAGE_ORDER,
  kanbanIssuesByStatus,
  type KanbanIssue,
  type KanbanStageStatus,
} from "../../composables/use-kanban-board";

const props = withDefaults(defineProps<{
  issues: KanbanIssue[];
  loading?: boolean;
  errorText?: string;
  approvingNumber?: number | null;
  /** 队长的人格名，由会话侧 personaName 传入；不传时与磁贴一样回落到 Claude。 */
  leaderName?: string;
}>(), {
  loading: false,
  errorText: "",
  approvingNumber: null,
  leaderName: "",
});

const emit = defineEmits<{
  (e: "back"): void;
  (e: "refresh"): void;
  (e: "approve", number: number): void;
}>();

const { t } = useI18n();

/** 顺序与类型来自看板公共模块：看板页展示全部七态，磁贴两个泳道只是它的子集。 */
const STAGE_ORDER = KANBAN_STAGE_ORDER;

type StageStatus = KanbanStageStatus;
type StageStyle = { titleClass: string; nodeClass: string; needsAttention: boolean };

/** 主链 = 除 blocked 之外的阶段；序号只给主链，blocked 用图标区分。 */
const MAIN_CHAIN_ORDER: readonly StageStatus[] = STAGE_ORDER.filter((status) => status !== "blocked");

const STAGE_STYLE: Record<StageStatus, StageStyle> = {
  blocked: {
    titleClass: "text-error",
    nodeClass: "bg-error text-error-content",
    needsAttention: true,
  },
  discussing: {
    titleClass: "text-info",
    nodeClass: "border border-base-content/15 bg-base-100 text-base-content/55",
    needsAttention: true,
  },
  todo: {
    titleClass: "text-base-content/70",
    nodeClass: "border border-base-content/15 bg-base-100 text-base-content/55",
    needsAttention: true,
  },
  in_progress: {
    titleClass: "text-base-content/70",
    nodeClass: "border border-base-content/15 bg-base-100 text-base-content/55",
    needsAttention: false,
  },
  in_review: {
    titleClass: "text-warning",
    nodeClass: "border border-warning/30 bg-warning/15 text-warning",
    needsAttention: true,
  },
  done: {
    titleClass: "text-success",
    nodeClass: "border border-success/30 bg-success/15 text-success",
    needsAttention: false,
  },
  closed: {
    titleClass: "text-base-content/45",
    nodeClass: "border border-base-content/15 bg-base-content/5 text-base-content/45",
    needsAttention: false,
  },
};

const stages = computed(() =>
  STAGE_ORDER.map((status) => {
    const style = STAGE_STYLE[status];
    return {
      status,
      labelKey: `chat.kanban.status.${status}`,
      titleClass: style.titleClass,
      nodeClass: style.nodeClass,
      needsAttention: style.needsAttention,
      chainNumber: MAIN_CHAIN_ORDER.indexOf(status) + 1,
      issues: kanbanIssuesByStatus(props.issues, status),
    };
  }),
);

/** 负责人列直接写人格名：队长取会话人格名，队员取角色名（角色本身就是人格名），不再加「队员 · 」前缀。 */
function assigneeLabel(raw: string): string {
  const value = String(raw || "").trim();
  if (value === "leader") return props.leaderName || "Claude";
  if (value.startsWith("member:")) {
    return value.slice("member:".length).trim() || String(t("chat.kanban.assigneeNone"));
  }
  if (value === "none" || !value) return String(t("chat.kanban.assigneeNone"));
  return value;
}

/** 只取文件名做悬浮提示：完整路径太长，会把行挤变形。 */
function planFileName(path: string): string {
  const parts = String(path || "").split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || String(path || "");
}
</script>
