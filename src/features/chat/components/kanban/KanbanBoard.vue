<template>
  <div class="flex h-full min-h-0 w-full flex-col bg-base-200 text-base-content select-none">
    <!-- 标签、计数与刷新都由右面板通用标题栏提供，这里不再自绘第二条标题条 -->

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
        scroller-class="flex h-full w-full gap-6 p-4 min-w-full"
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
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import KanbanBoardColumn from "./KanbanBoardColumn.vue";
import type { KanbanIssue, KanbanStageStatus } from "./kanban-types";
import { KANBAN_STAGE_ORDER } from "../../composables/use-kanban-board";

const props = withDefaults(defineProps<{
  issues: KanbanIssue[];
  loading?: boolean;
  errorText?: string;
  approvingNumber?: number | null;
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
  leaderName: "",
  leaderAvatarUrl: "",
  avatarUrlMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  personaNameMap: () => ({}),
  teamMembers: () => [],
});

const emit = defineEmits<{
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "reference", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

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
