<template>
  <div class="flex h-full min-h-0 w-full flex-col bg-base-100 text-base-content select-none">
    <!-- 顶栏：一条标签条，只有一个「议题」标签，返回与刷新挂在两端 -->
    <PanelTabStrip
      :tabs="boardTabs"
      active-key="kanban-board"
      :aria-label="t('chat.kanban.boardTitle')"
    >
      <template v-if="showBack" #leading>
        <button
          type="button"
          class="btn btn-ghost btn-sm btn-square shrink-0"
          :title="t('chat.kanban.backToTiles')"
          :aria-label="t('chat.kanban.backToTiles')"
          @click="emit('back')"
        >
          <AppIcon name="layout-grid" class="size-4" aria-hidden="true" />
        </button>
      </template>

      <template #actions>
        <span class="shrink-0 px-1 text-xs tabular-nums text-base-content/45">
          {{ t("chat.kanban.issueCount", { count: issues.length }) }}
        </span>
        <button
          type="button"
          class="btn btn-ghost btn-sm btn-square shrink-0"
          :title="t('chat.kanban.refresh')"
          :aria-label="t('chat.kanban.refresh')"
          :disabled="loading"
          @click="emit('refresh')"
        >
          <AppIcon name="refresh-cw" class="size-4" :class="loading ? 'animate-spin' : ''" aria-hidden="true" />
        </button>
      </template>
    </PanelTabStrip>

    <!-- 错误横条 -->
    <div v-if="errorText" class="shrink-0 bg-error/10 px-3 py-1.5 text-xs text-error">
      {{ errorText }}
    </div>

    <!-- 看板主区：空的不显示，仅保留有实际议题的状态分类（遵循项目既有规则） -->
    <div class="relative min-h-0 w-full flex-1 overflow-hidden">
      <!-- 全局无议题空态 -->
      <div
        v-if="!activeColumns.length && !loading && !errorText"
        class="flex h-full w-full flex-col items-center justify-center gap-3 p-6 text-center"
      >
        <p class="max-w-xs text-xs leading-relaxed text-base-content/45">
          {{ t("chat.kanban.boardEmpty") }}
        </p>
      </div>

      <!-- 泳道平铺看板：横向滚动、列内纵向滚动 -->
      <OverlayScrollArea
        v-else
        orientation="horizontal"
        class="h-full w-full"
        scroller-class="flex h-full w-full gap-3 p-3 min-w-full"
      >
        <KanbanBoardColumn
          v-for="col in activeColumns"
          :key="col.status"
          :status="col.status"
          :issues="col.issues"
          :approving-number="approvingNumber"
          :resolve-assignee-name="resolveAssigneeName"
          :resolve-avatar="resolveAvatar"
          :class="activeColumns.length === 1 ? 'max-w-[340px]' : ''"
          @approve="emit('approve', $event)"
          @open-plan="emit('openPlan', $event)"
          @reference="emit('reference', $event)"
        />
      </OverlayScrollArea>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import PanelTabStrip from "../../../shared/components/PanelTabStrip.vue";
import type { PanelTabStripItem } from "../../../shared/components/panel-tab-strip.types";
import KanbanBoardColumn from "./KanbanBoardColumn.vue";
import type { KanbanIssue, KanbanStageStatus } from "./kanban-types";
import { KANBAN_STAGE_ORDER } from "../../composables/use-kanban-board";

const props = withDefaults(defineProps<{
  issues: KanbanIssue[];
  loading?: boolean;
  errorText?: string;
  approvingNumber?: number | null;
  title?: string;
  showBack?: boolean;
  leaderName?: string;
  leaderAvatarUrl?: string;
  avatarUrlMap?: Record<string, string>;
  personaAvatarUrlMap?: Record<string, string>;
  personaNameMap?: Record<string, string>;
  teamMembers?: Array<{ teamName?: string; agentId?: string; agentName?: string }>;
}>(), {
  loading: false,
  errorText: "",
  approvingNumber: null,
  title: "",
  showBack: false,
  leaderName: "",
  leaderAvatarUrl: "",
  avatarUrlMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  personaNameMap: () => ({}),
  teamMembers: () => [],
});

const emit = defineEmits<{
  (e: "back"): void;
  (e: "refresh"): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "reference", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

/**
 * 顶栏只有一个标签：图标 + 议题。看板本身是一个板块，没有第二个页签，
 * 用标签条只是为了让这块和文件阅读器等面板的顶部形态一致。
 */
const boardTabs = computed<PanelTabStripItem[]>(() => [
  {
    key: "kanban-board",
    label: props.title || t("chat.kanban.boardTab"),
    icon: "task-02",
  },
]);

/**
 * 遵循项目既有看板核心规则：空分类不显示，泳道仅保留有实际议题的状态分类。
 * 避免 7 个状态 6 个为空时强行占位造成空旷与无意义的滚动。
 */
const activeColumns = computed(() => {
  return KANBAN_STAGE_ORDER
    .map((status: KanbanStageStatus) => ({
      status,
      issues: props.issues.filter((item) => String(item.status || "").trim() === status),
    }))
    .filter((col) => col.issues.length > 0);
});

// ==================== 人格名称与头像反查 ====================
function resolveAssigneeName(raw: string): string {
  const value = String(raw || "").trim();
  if (!value || value === "none" || value === "unassigned") {
    return String(t("chat.kanban.assigneeNone"));
  }
  if (value === "leader") {
    return props.leaderName || "Claude";
  }
  const key = value.startsWith("member:") ? value.slice(7).trim() : value;
  const matched = props.teamMembers.find(
    (m) => m.teamName === key || m.agentName === key || m.agentId === key,
  );
  if (matched?.agentName) return matched.agentName;
  if (matched?.teamName) return matched.teamName;
  if (props.personaNameMap[key]) return props.personaNameMap[key];
  return key;
}

function resolveAvatar(raw: string): string {
  const value = String(raw || "").trim();
  if (!value) return "";
  if (value === "leader") {
    return props.leaderAvatarUrl || props.avatarUrlMap["leader"] || "";
  }
  const key = value.startsWith("member:") ? value.slice(7).trim() : value;
  if (!key) return "";

  if (props.avatarUrlMap[key]) return props.avatarUrlMap[key];
  if (props.personaAvatarUrlMap[key]) return props.personaAvatarUrlMap[key];

  const matched = props.teamMembers.find(
    (m) => m.teamName === key || m.agentName === key || m.agentId === key,
  );
  if (matched?.agentId) {
    if (props.avatarUrlMap[matched.agentId]) return props.avatarUrlMap[matched.agentId];
    if (props.personaAvatarUrlMap[matched.agentId]) return props.personaAvatarUrlMap[matched.agentId];
  }

  for (const [id, name] of Object.entries(props.personaNameMap)) {
    if (name === key) {
      if (props.avatarUrlMap[id]) return props.avatarUrlMap[id];
      if (props.personaAvatarUrlMap[id]) return props.personaAvatarUrlMap[id];
    }
  }

  return "";
}
</script>
