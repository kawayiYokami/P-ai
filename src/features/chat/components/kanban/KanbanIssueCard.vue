<template>
  <div
    class="flex flex-col rounded-field border border-base-content/10 transition-colors hover:border-base-content/25 dark:bg-[color-mix(in_oklab,var(--color-base-100)_90%,white)] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary/50"
    :class="[
      densityClass,
      issue.status === 'in_review' ? 'border-warning/40' : '',
      issue.status === 'blocked' ? 'border-error/40' : '',
      selectable ? 'cursor-pointer' : '',
    ]"
    role="article"
    :tabindex="selectable ? 0 : undefined"
    @click="handleSelect"
    @keydown.enter.prevent="handleSelect"
  >
    <!-- 顶行：编号 + 计划文件外链 -->
    <div class="flex items-center justify-between gap-1.5 text-caption">
      <!-- 编号在磁贴里可点：一点就把这条议题挂到输入框当引用，卡片其余部分仍是打开看板 -->
      <button
        v-if="referenceable"
        type="button"
        class="cursor-pointer font-mono text-xs font-medium tabular-nums text-base-content/50 underline decoration-dotted underline-offset-2 transition-colors hover:text-primary"
        :title="t('chat.kanban.referenceHint')"
        @click.stop="handleReference"
      >
        #{{ issue.number }}
      </button>
      <span v-else class="font-mono text-xs font-medium tabular-nums text-base-content/50">
        #{{ issue.number }}
      </span>

      <button
        v-if="showPlanLink && issue.planPath"
        type="button"
        class="flex max-w-[140px] cursor-pointer items-center gap-1 truncate text-caption text-base-content/50 transition-colors hover:text-primary"
        :title="issue.planPath"
        @click.stop="emit('openPlan', issue.planPath)"
      >
        <FileText class="size-3 shrink-0" aria-hidden="true" />
        <span class="truncate">{{ planFileName(issue.planPath) }}</span>
      </button>
    </div>

    <!-- 中行：标题（最多 2 行） -->
    <div
      class="line-clamp-2 text-xs font-medium leading-snug text-base-content/90"
      :title="issue.title"
    >
      {{ issue.title }}
    </div>

    <!-- 备注是写给人格/工具看的信息（例如「用户在看板上确认通过」），不进界面 -->

    <!-- 底行：执行人 + 相对时间 / 审核通过按钮 -->
    <div class="flex items-center justify-between gap-1.5 pt-0.5 text-caption">
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

      <!-- 待审状态直出通过按钮：这张卡的动作，任何形态都有 -->
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

/**
 * 议题卡：全屏看板与首页磁贴共用同一份实现。
 *
 * 形态差异只走 prop —— `density` 管内边距与底色，`selectable` 管能否点选，
 * `showPlanLink` 管计划文件外链。通过按钮不设开关：它就是这张卡的动作。
 */
const props = withDefaults(defineProps<{
  issue: KanbanIssue;
  assigneeName: string;
  avatarUrl?: string;
  approvingNumber?: number | null;
  /** board = 全屏看板（白底、宽松）；tile = 首页磁贴（更紧、底色下沉一档）。 */
  density?: "board" | "tile";
  /** 能否点选：磁贴里点卡片等同打开看板。 */
  selectable?: boolean;
  showPlanLink?: boolean;
  /** 编号能否作为引用挂进输入框：只有首页磁贴打开，看板页保持纯展示。 */
  referenceable?: boolean;
}>(), {
  avatarUrl: "",
  approvingNumber: null,
  density: "board",
  selectable: true,
  showPlanLink: true,
  referenceable: false,
});

const emit = defineEmits<{
  (e: "select", issue: KanbanIssue): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "reference", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

const densityClass = computed(() =>
  props.density === "tile"
    ? "gap-1.5 p-2 bg-base-200/50 hover:bg-base-200/80"
    : "gap-1.5 p-2.5 bg-base-100",
);

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

function handleSelect(): void {
  if (!props.selectable) return;
  emit("select", props.issue);
}

function handleReference(): void {
  emit("reference", props.issue);
}

function planFileName(path: string): string {
  const parts = String(path || "").split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || String(path || "");
}
</script>
