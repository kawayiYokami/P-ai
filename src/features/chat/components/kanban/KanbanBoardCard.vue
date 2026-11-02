<template>
  <div
    class="flex flex-col gap-1.5 rounded-field border border-base-content/10 bg-base-100 p-2.5 shadow-sm transition-colors hover:border-base-content/25 dark:bg-[color-mix(in_oklab,var(--color-base-100)_90%,white)] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary/50"
    :class="[
      issue.status === 'in_review' ? 'border-warning/40' : '',
      issue.status === 'blocked' ? 'border-error/40' : '',
    ]"
    role="article"
    tabindex="0"
    @click="emit('select', issue)"
    @keydown.enter.prevent="emit('select', issue)"
  >
    <!-- 顶行：编号 + 计划文件外链 -->
    <div class="flex items-center justify-between gap-1.5 text-caption">
      <span class="font-mono font-medium text-xs tabular-nums text-base-content/50">
        #{{ issue.number }}
      </span>

      <button
        v-if="issue.planPath"
        type="button"
        class="flex max-w-[140px] items-center gap-1 truncate text-caption text-base-content/50 hover:text-primary transition-colors cursor-pointer"
        :title="issue.planPath"
        @click.stop="emit('openPlan', issue.planPath)"
      >
        <FileText class="size-3 shrink-0" aria-hidden="true" />
        <span class="truncate">{{ planFileName(issue.planPath) }}</span>
      </button>
    </div>

    <!-- 中行：标题（最多 2 行，清晰易读） -->
    <div
      class="line-clamp-2 text-xs font-medium leading-snug text-base-content/90"
      :title="issue.title"
    >
      {{ issue.title }}
    </div>

    <!-- 备注说明：微透明底色小条 -->
    <div
      v-if="issue.note"
      class="line-clamp-2 rounded bg-base-200/70 px-2 py-1 text-micro leading-relaxed text-base-content/65"
      :title="issue.note"
    >
      {{ issue.note }}
    </div>

    <!-- 底行：执行人 + 相对时间 / 审核通过按钮 -->
    <div class="flex items-center justify-between gap-1.5 pt-0.5 text-caption">
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
          class="max-w-[100px] truncate text-caption text-base-content/75"
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
          class="btn btn-warning btn-xs h-5 min-h-5 gap-1 px-1.5 text-caption font-medium"
          :disabled="approvingNumber === issue.number"
          :title="t('chat.kanban.approveHint')"
          @click.stop="emit('approve', issue.number)"
        >
          <Check class="size-3" aria-hidden="true" />
          <span>{{ t("chat.kanban.approve") }}</span>
        </button>
        <span
          v-else-if="timeLabel"
          class="shrink-0 text-caption tabular-nums text-base-content/40"
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
  type KanbanIssue,
} from "./kanban-types";

const props = withDefaults(defineProps<{
  issue: KanbanIssue;
  assigneeName: string;
  avatarUrl?: string;
  approvingNumber?: number | null;
}>(), {
  avatarUrl: "",
  approvingNumber: null,
});

const emit = defineEmits<{
  (e: "select", issue: KanbanIssue): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
}>();

const { t } = useI18n();

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
