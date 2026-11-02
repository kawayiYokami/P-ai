<template>
  <div
    class="group/card flex flex-col gap-2 rounded-field border border-base-content/10 bg-base-100 p-2.5 shadow-2xs transition-all hover:border-base-content/25 hover:shadow-xs focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary/50"
    :class="[
      issue.status === 'in_review' ? 'border-warning/30 bg-warning/[0.02]' : '',
      issue.status === 'blocked' ? 'border-error/25 bg-error/[0.02]' : '',
    ]"
    role="article"
    tabindex="0"
    @click="emit('select', issue)"
    @keydown.enter.prevent="emit('select', issue)"
  >
    <!-- 顶栏：编号 + 状态药丸（可选） + 计划文件外链 -->
    <div class="flex items-center justify-between gap-1.5 font-mono text-micro">
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="font-mono font-semibold tabular-nums text-base-content/60">
          #{{ issue.number }}
        </span>
        <span
          v-if="showStageBadge"
          class="badge badge-xs shrink-0"
          :class="stageMeta.badgeClass"
        >
          {{ t(stageMeta.labelKey) }}
        </span>
      </div>

      <!-- 挂接计划文件：带图标与悬浮提示，可点击跳转/预览 -->
      <button
        v-if="issue.planPath"
        type="button"
        class="flex max-w-[120px] shrink-0 items-center gap-1 truncate rounded px-1 py-0.5 text-micro text-base-content/50 transition-colors hover:bg-base-200 hover:text-primary"
        :title="issue.planPath"
        @click.stop="emit('openPlan', issue.planPath)"
      >
        <FileText class="size-3 shrink-0" aria-hidden="true" />
        <span class="truncate">{{ planFileName(issue.planPath) }}</span>
      </button>
    </div>

    <!-- 中部：标题（最多 2 行，清晰易读） -->
    <div
      class="line-clamp-2 text-xs font-medium leading-snug text-base-content/90 transition-colors group-hover/card:text-base-content"
      :title="issue.title"
    >
      {{ issue.title }}
    </div>

    <!-- 备注：如果有阻塞原因或当前说明，以浅色背景小气泡呈现 -->
    <div
      v-if="issue.note"
      class="line-clamp-2 rounded bg-base-200/60 px-2 py-1 text-micro leading-relaxed text-base-content/65"
      :title="issue.note"
    >
      {{ issue.note }}
    </div>

    <!-- 底栏：执行人 + 相对时间 / 审核通过按钮 -->
    <div class="flex items-center justify-between gap-1.5 pt-0.5 text-micro">
      <!-- 执行人人格/角色名与头像 -->
      <div class="flex min-w-0 items-center gap-1.5">
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
          class="max-w-[90px] truncate text-micro text-base-content/75"
          :title="assigneeName"
        >
          {{ assigneeName }}
        </span>
      </div>

      <!-- 右侧：待审状态直出通过按钮，其余展示相对时间 -->
      <div class="flex shrink-0 items-center gap-1">
        <button
          v-if="issue.status === 'in_review'"
          type="button"
          class="btn btn-warning btn-xs h-5 min-h-5 gap-1 px-1.5 text-micro font-medium shadow-2xs hover:brightness-105"
          :disabled="approvingNumber === issue.number"
          :title="t('chat.kanban.approveHint')"
          @click.stop="emit('approve', issue.number)"
        >
          <Check class="size-2.5" aria-hidden="true" />
          <span>{{ t("chat.kanban.approve") }}</span>
        </button>
        <span
          v-else-if="timeLabel"
          class="shrink-0 text-micro tabular-nums text-base-content/40"
        >
          {{ timeLabel }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Check, FileText } from "@lucide/vue";
import {
  formatTimeRelative,
  getAvatarColorByName,
  KANBAN_STAGE_CONFIGS,
  type KanbanIssue,
  type KanbanStageStatus,
} from "./kanban-types";

const props = withDefaults(defineProps<{
  issue: KanbanIssue;
  assigneeName: string;
  avatarUrl?: string;
  showStageBadge?: boolean;
  approvingNumber?: number | null;
}>(), {
  avatarUrl: "",
  showStageBadge: false,
  approvingNumber: null,
});

const emit = defineEmits<{
  (e: "select", issue: KanbanIssue): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
}>();

const { t } = useI18n();

const stageMeta = computed(() => {
  const status = String(props.issue.status || "").trim() as KanbanStageStatus;
  return KANBAN_STAGE_CONFIGS[status] || KANBAN_STAGE_CONFIGS.todo;
});

const avatarContainerStyle = computed(() => {
  if (props.avatarUrl) return {};
  return { backgroundColor: getAvatarColorByName(props.assigneeName) };
});

const initialChar = computed(() => {
  const name = props.assigneeName.trim();
  return name ? name.charAt(0).toUpperCase() : "?";
});

const timeLabel = computed(() => {
  return formatTimeRelative(props.issue.updatedAt || props.issue.createdAt);
});

function planFileName(path: string): string {
  const parts = String(path || "").split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || String(path || "");
}
</script>
