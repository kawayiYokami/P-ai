<template>
  <div class="flex h-full min-h-0 w-full flex-col bg-base-100 text-base-content select-none">
    <!-- ==================== 顶部控制工具栏 ==================== -->
    <header class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-base-content/10 bg-base-100 px-3 py-2">
      <!-- 左侧：返回按键（可选） + 标题 + 统计胶囊 + 紧急警报胶囊 -->
      <div class="flex items-center gap-2">
        <button
          v-if="showBack"
          type="button"
          class="btn btn-ghost btn-xs btn-square"
          :title="t('chat.kanban.backToTiles')"
          :aria-label="t('chat.kanban.backToTiles')"
          @click="emit('back')"
        >
          <LayoutGrid class="size-3.5" aria-hidden="true" />
        </button>

        <span class="text-sm font-semibold tracking-wide text-base-content/90">
          {{ title || t("chat.kanban.boardTitle") }}
        </span>

        <span class="badge badge-xs border-base-content/10 bg-base-200 font-mono text-micro text-base-content/60">
          {{ t("chat.kanban.issueCount", { count: issues.length }) }}
        </span>

        <!-- 关键态高亮微标（待审、阻塞） -->
        <span
          v-if="attentionCounts.inReview > 0"
          class="badge badge-warning badge-xs text-micro font-medium gap-1"
          :title="`待审 ${attentionCounts.inReview} 项`"
        >
          <span class="size-1.5 rounded-full bg-warning-content animate-pulse" />
          {{ attentionCounts.inReview }} 待审
        </span>
        <span
          v-if="attentionCounts.blocked > 0"
          class="badge badge-error badge-xs text-micro font-medium gap-1"
          :title="`阻塞 ${attentionCounts.blocked} 项`"
        >
          {{ attentionCounts.blocked }} 阻塞
        </span>
      </div>

      <!-- 中间/右侧控制区：范围筛选 + 仅非空切换 + 视图切换 + 搜索 + 刷新 -->
      <div class="flex items-center gap-1.5">
        <!-- 范围筛选标签 -->
        <div class="join border border-base-content/10 bg-base-200/50 p-0.5 rounded-btn">
          <button
            v-for="filter in FILTER_OPTIONS"
            :key="filter.key"
            type="button"
            class="join-item btn btn-xs h-6 min-h-6 border-0 px-2 font-normal transition-colors"
            :class="activeFilter === filter.key ? 'btn-neutral font-medium shadow-2xs' : 'btn-ghost text-base-content/65 hover:text-base-content'"
            @click="activeFilter = filter.key"
          >
            {{ filter.label }}
          </button>
        </div>

        <!-- 仅展示有任务列（空列隐藏）开关 -->
        <button
          type="button"
          class="btn btn-xs h-7 min-h-7 gap-1 border border-base-content/10 px-2 text-micro transition-colors"
          :class="hideEmptyColumns ? 'btn-neutral font-medium' : 'btn-ghost text-base-content/65 hover:text-base-content'"
          title="隐藏没有任务的列"
          @click="hideEmptyColumns = !hideEmptyColumns"
        >
          <Filter class="size-3" aria-hidden="true" />
          <span class="hidden sm:inline">仅有任务</span>
        </button>

        <!-- 视图切换：泳道看板 vs 紧凑列表 -->
        <div class="join border border-base-content/10 bg-base-200/50 p-0.5 rounded-btn">
          <button
            type="button"
            class="join-item btn btn-xs h-6 min-h-6 border-0 px-1.5 transition-colors"
            :class="viewMode === 'board' ? 'btn-neutral shadow-2xs' : 'btn-ghost text-base-content/60'"
            title="泳道看板视图"
            @click="viewMode = 'board'"
          >
            <Columns3 class="size-3" aria-hidden="true" />
          </button>
          <button
            type="button"
            class="join-item btn btn-xs h-6 min-h-6 border-0 px-1.5 transition-colors"
            :class="viewMode === 'list' ? 'btn-neutral shadow-2xs' : 'btn-ghost text-base-content/60'"
            title="清单列表视图"
            @click="viewMode = 'list'"
          >
            <List class="size-3" aria-hidden="true" />
          </button>
        </div>

        <!-- 搜索过滤 -->
        <div class="relative flex items-center">
          <div
            class="flex h-7 items-center rounded-field border border-base-content/10 bg-base-200/40 px-2 transition-all focus-within:border-primary/50 focus-within:bg-base-100"
            :class="searchQuery ? 'w-44' : 'w-24 focus-within:w-44'"
          >
            <Search class="size-3 shrink-0 text-base-content/40" aria-hidden="true" />
            <input
              v-model="searchQuery"
              type="text"
              class="min-w-0 flex-1 bg-transparent px-1.5 text-micro outline-hidden placeholder:text-base-content/35"
              placeholder="搜索..."
            />
            <button
              v-if="searchQuery"
              type="button"
              class="shrink-0 text-base-content/40 hover:text-base-content"
              @click="searchQuery = ''"
            >
              <X class="size-3" aria-hidden="true" />
            </button>
          </div>
        </div>

        <!-- 刷新按钮 -->
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
      </div>
    </header>

    <!-- 错误横条 -->
    <div v-if="errorText" class="shrink-0 bg-error/10 px-3 py-1.5 text-xs text-error">
      {{ errorText }}
    </div>

    <!-- ==================== 内容主区 ==================== -->
    <div class="relative min-h-0 w-full flex-1 overflow-hidden">
      <!-- 状态 A：整块看板完全无任何议题（全局空态） -->
      <div
        v-if="!issues.length && !loading && !errorText"
        class="flex h-full w-full flex-col items-center justify-center gap-3 p-6 text-center"
      >
        <div class="grid size-12 place-items-center rounded-2xl bg-base-200 text-base-content/40">
          <LayoutGrid class="size-6" aria-hidden="true" />
        </div>
        <p class="max-w-xs text-xs leading-relaxed text-base-content/50">
          {{ t("chat.kanban.boardEmpty") }}
        </p>
      </div>

      <!-- 状态 B：搜索或过滤后无匹配项 -->
      <div
        v-else-if="filteredTotalCount === 0 && !loading"
        class="flex h-full w-full flex-col items-center justify-center gap-3 p-6 text-center"
      >
        <div class="grid size-10 place-items-center rounded-xl bg-base-200 text-base-content/40">
          <Search class="size-5" aria-hidden="true" />
        </div>
        <p class="text-xs text-base-content/50">未找到符合条件的议题</p>
        <button
          type="button"
          class="btn btn-ghost btn-xs text-primary"
          @click="resetFilters"
        >
          重置筛选与搜索
        </button>
      </div>

      <!-- 状态 C：泳道看板视图 (Board Mode) -->
      <OverlayScrollArea
        v-else-if="viewMode === 'board'"
        orientation="horizontal"
        class="h-full w-full"
        scroller-class="flex h-full w-full gap-3 p-3 min-w-full"
      >
        <KanbanBoardColumn
          v-for="col in displayColumns"
          :key="col.status"
          :status="col.status"
          :issues="col.issues"
          :approving-number="approvingNumber"
          :resolve-assignee-name="resolveAssigneeName"
          :resolve-avatar="resolveAvatar"
          @select-issue="emit('selectIssue', $event)"
          @approve="emit('approve', $event)"
          @open-plan="emit('openPlan', $event)"
        />
      </OverlayScrollArea>

      <!-- 状态 D：清单列表视图 (List Mode) -->
      <OverlayScrollArea
        v-else
        class="h-full w-full"
        scroller-class="flex h-full flex-col gap-4 p-3"
      >
        <section
          v-for="group in displayColumns"
          :key="group.status"
          class="flex flex-col gap-1.5"
        >
          <!-- 阶段标题栏 -->
          <div class="flex items-center gap-2 px-1">
            <span class="size-2 rounded-full" :class="group.meta.dotClass" aria-hidden="true" />
            <h4 class="text-xs font-semibold text-base-content/85">
              {{ t(group.meta.labelKey) }}
            </h4>
            <span class="font-mono text-micro tabular-nums text-base-content/40">
              ({{ group.issues.length }})
            </span>
          </div>

          <!-- 列表卡片簇 -->
          <div
            v-if="group.issues.length"
            class="flex flex-col divide-y divide-base-content/5 rounded-box border border-base-content/10 bg-base-100 overflow-hidden shadow-2xs"
          >
            <div
              v-for="issue in group.issues"
              :key="issue.number"
              class="group flex flex-wrap items-center justify-between gap-3 p-2.5 transition-colors hover:bg-base-200/50"
            >
              <!-- 左侧：编号 + 标题 + 计划文件 + 备注 -->
              <div class="flex min-w-0 flex-1 items-center gap-2.5">
                <span class="font-mono text-xs font-semibold tabular-nums text-base-content/50">
                  #{{ issue.number }}
                </span>
                <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                  <div class="flex items-center gap-2">
                    <span class="truncate text-xs font-medium text-base-content/90" :title="issue.title">
                      {{ issue.title }}
                    </span>
                    <button
                      v-if="issue.planPath"
                      type="button"
                      class="flex items-center gap-1 text-micro text-base-content/50 hover:text-primary transition-colors shrink-0"
                      :title="issue.planPath"
                      @click.stop="emit('openPlan', issue.planPath)"
                    >
                      <FileText class="size-3" aria-hidden="true" />
                      <span class="max-w-[100px] truncate">{{ planFileName(issue.planPath) }}</span>
                    </button>
                  </div>
                  <p
                    v-if="issue.note"
                    class="truncate text-micro text-base-content/50"
                    :title="issue.note"
                  >
                    {{ issue.note }}
                  </p>
                </div>
              </div>

              <!-- 右侧：执行人 + 时间 + 审核操作 -->
              <div class="flex shrink-0 items-center gap-3">
                <div class="flex items-center gap-1.5">
                  <div class="avatar size-4 shrink-0">
                    <div
                      class="flex size-4 items-center justify-center overflow-hidden rounded-full font-bold text-[9px] text-white"
                      :style="resolveAvatarContainerStyle(resolveAssigneeName(issue.assignee), resolveAvatar(issue.assignee))"
                    >
                      <img
                        v-if="resolveAvatar(issue.assignee)"
                        :src="resolveAvatar(issue.assignee)"
                        :alt="resolveAssigneeName(issue.assignee)"
                        class="size-full object-cover"
                      />
                      <span v-else>{{ getInitial(resolveAssigneeName(issue.assignee)) }}</span>
                    </div>
                  </div>
                  <span class="max-w-[80px] truncate text-micro text-base-content/75">
                    {{ resolveAssigneeName(issue.assignee) }}
                  </span>
                </div>

                <span class="text-micro tabular-nums text-base-content/40">
                  {{ formatTime(issue.updatedAt || issue.createdAt) }}
                </span>

                <button
                  v-if="issue.status === 'in_review'"
                  type="button"
                  class="btn btn-warning btn-xs h-6 min-h-6 gap-1 px-2 text-micro font-medium shadow-2xs"
                  :disabled="approvingNumber === issue.number"
                  :title="t('chat.kanban.approveHint')"
                  @click.stop="emit('approve', issue.number)"
                >
                  <Check class="size-3" aria-hidden="true" />
                  <span>{{ t("chat.kanban.approve") }}</span>
                </button>
              </div>
            </div>
          </div>

          <div
            v-else
            class="rounded-box border border-dashed border-base-content/10 p-3 text-center text-micro text-base-content/30"
          >
            {{ t("chat.kanban.groupEmpty") }}
          </div>
        </section>
      </OverlayScrollArea>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Check,
  Columns3,
  FileText,
  Filter,
  LayoutGrid,
  List,
  RefreshCw,
  Search,
  X,
} from "@lucide/vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import KanbanBoardColumn from "./KanbanBoardColumn.vue";
import {
  ACTIVE_STAGE_STATUSES,
  ATTENTION_STAGE_STATUSES,
  DONE_STAGE_STATUSES,
  formatTimeRelative,
  getAvatarColorByName,
  KANBAN_STAGE_CONFIGS,
  type KanbanFilterScope,
  type KanbanIssue,
  type KanbanStageStatus,
  type KanbanViewMode,
} from "./kanban-types";
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
  initialViewMode?: KanbanViewMode;
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
  initialViewMode: "board",
});

const emit = defineEmits<{
  (e: "back"): void;
  (e: "refresh"): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  (e: "selectIssue", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

// ==================== 状态机 ====================
const viewMode = ref<KanbanViewMode>(props.initialViewMode);
const activeFilter = ref<KanbanFilterScope>("all");
const hideEmptyColumns = ref<boolean>(false);
const searchQuery = ref<string>("");

// 默认智能控制：如果总议题数少（例如总共只有 1~2 个状态有任务），自动开启「仅有任务」
watch(
  () => props.issues,
  (list) => {
    if (list.length > 0 && list.length <= 4) {
      hideEmptyColumns.value = true;
    }
  },
  { immediate: true },
);

const FILTER_OPTIONS: Array<{ key: KanbanFilterScope; label: string }> = [
  { key: "all", label: "全部" },
  { key: "active", label: "进行中" },
  { key: "attention", label: "需关注" },
  { key: "done", label: "已完成" },
];

// ==================== 统计计数 ====================
const attentionCounts = computed(() => {
  let inReview = 0;
  let blocked = 0;
  for (const issue of props.issues) {
    if (issue.status === "in_review") inReview += 1;
    if (issue.status === "blocked") blocked += 1;
  }
  return { inReview, blocked };
});

// ==================== 筛选与列切分 ====================
const filteredIssues = computed(() => {
  let list = props.issues;

  // 1. 范围筛选
  if (activeFilter.value === "active") {
    list = list.filter((item) => ACTIVE_STAGE_STATUSES.includes(item.status as KanbanStageStatus));
  } else if (activeFilter.value === "attention") {
    list = list.filter((item) => ATTENTION_STAGE_STATUSES.includes(item.status as KanbanStageStatus));
  } else if (activeFilter.value === "done") {
    list = list.filter((item) => DONE_STAGE_STATUSES.includes(item.status as KanbanStageStatus));
  }

  // 2. 搜索过滤
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((item) => {
      const matchNum = String(item.number).includes(q);
      const matchTitle = String(item.title || "").toLowerCase().includes(q);
      const matchNote = String(item.note || "").toLowerCase().includes(q);
      const matchAssignee = resolveAssigneeName(item.assignee).toLowerCase().includes(q);
      return matchNum || matchTitle || matchNote || matchAssignee;
    });
  }

  return list;
});

const filteredTotalCount = computed(() => filteredIssues.value.length);

/** 最终要在看板上呈现的列配置 */
const displayColumns = computed(() => {
  const allowedStatuses = new Set<string>();
  if (activeFilter.value === "all") {
    for (const s of KANBAN_STAGE_ORDER) allowedStatuses.add(s);
  } else if (activeFilter.value === "active") {
    for (const s of ACTIVE_STAGE_STATUSES) allowedStatuses.add(s);
  } else if (activeFilter.value === "attention") {
    for (const s of ATTENTION_STAGE_STATUSES) allowedStatuses.add(s);
  } else if (activeFilter.value === "done") {
    for (const s of DONE_STAGE_STATUSES) allowedStatuses.add(s);
  }

  const columns = KANBAN_STAGE_ORDER
    .filter((status) => allowedStatuses.has(status))
    .map((status) => {
      const issues = filteredIssues.value.filter((i) => i.status === status);
      return {
        status,
        meta: KANBAN_STAGE_CONFIGS[status] || KANBAN_STAGE_CONFIGS.todo,
        issues,
      };
    });

  // 如果启用了「仅有任务的列」，过滤掉 0 项的列；若全部都是 0 项则保留空态
  if (hideEmptyColumns.value) {
    const nonEmpty = columns.filter((col) => col.issues.length > 0);
    return nonEmpty.length > 0 ? nonEmpty : columns;
  }

  return columns;
});

function resetFilters() {
  activeFilter.value = "all";
  searchQuery.value = "";
  hideEmptyColumns.value = false;
}

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

function resolveAvatarContainerStyle(name: string, url: string) {
  if (url) return {};
  return { backgroundColor: getAvatarColorByName(name) };
}

function getInitial(name: string) {
  const str = String(name || "").trim();
  return str ? str.charAt(0).toUpperCase() : "?";
}

function formatTime(time?: string) {
  return formatTimeRelative(time);
}

function planFileName(path: string): string {
  const parts = String(path || "").split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || String(path || "");
}
</script>
