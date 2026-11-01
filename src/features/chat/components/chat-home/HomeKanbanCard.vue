<template>
  <CardShell
    tone="warning"
    :icon="LayoutGrid"
    :label="t('chat.kanban.cardTitle')"
    class="ecall-home-kanban"
    :class="kanbanGridClass"
    :style="kanbanGridStyle"
  >
    <template #leading>
      <div class="flex min-w-0 flex-1 items-center">
        <!-- 切换按钮表当前态，点一下换到另一态。展示名是「讨论 / 进展」 -->
        <button
          type="button"
          class="btn btn-ghost btn-xs h-6 min-w-0 gap-1 px-1.5 text-xs font-medium text-base-content/70 transition-colors hover:bg-base-content/10 hover:text-base-content"
          :aria-label="t(`chat.kanban.view.${mode}`)"
          @click.stop="toggleMode"
        >
          <ArrowLeftRight class="size-3.5 shrink-0" aria-hidden="true" />
          <span class="truncate">{{ t(`chat.kanban.view.${mode}`) }}</span>
        </button>
      </div>
    </template>

    <template #trailing>
      <div class="flex shrink-0 items-center gap-1.5">
        <span
          v-if="visibleCount > 0"
          class="badge badge-xs shrink-0 border-0 font-mono font-medium"
          :class="badgeClass"
          :title="t('chat.kanban.issueCount', { count: visibleCount })"
        >
          {{ visibleCount }}
        </span>
        <!-- 看板本身不可点，由右上角独立按钮打开看板大图 -->
        <button
          type="button"
          class="btn btn-ghost btn-xs h-6 gap-1 px-1.5 text-xs text-base-content/50 transition-colors hover:bg-base-content/10 hover:text-base-content"
          :title="t('chat.kanban.cardOpenBoard')"
          @click.stop="emit('open')"
        >
          <span>{{ t("chat.kanban.cardMoreBtn") }}</span>
          <ArrowUpRight class="size-3" aria-hidden="true" />
        </button>
      </div>
    </template>

    <div ref="containerRef" class="flex min-h-0 w-full flex-1 flex-col text-left">
      <!-- 存在可见议题 -->
      <template v-if="visibleCount > 0">
        <!-- 模式 A：横向距离不够时（窄屏/手机端，容器宽度 < 500px）自动收缩为单列，纯手写动态下划线 Tab 切换 -->
        <div v-if="isNarrow" class="flex h-full min-h-0 w-full flex-1 flex-col">
          <!-- 纯手写极简下划线 Tab 栏：纯文本、无数量、空分类不显示 -->
          <div ref="tabBarRef" class="ecall-kanban-tab-bar relative flex shrink-0 items-center gap-2 border-b border-base-content/10 pb-1">
            <div
              class="ecall-kanban-underline pointer-events-none absolute bottom-0 h-0.5 rounded-full bg-warning"
              :style="underlineStyle"
            />
            <button
              v-for="group in activeGroups"
              :key="group.status"
              type="button"
              class="ecall-kanban-tab-btn relative z-10 px-2 py-1 text-xs font-medium transition-colors"
              :class="group.status === activeGroupStatus ? 'font-semibold text-warning' : 'text-base-content/60 hover:text-base-content/90'"
              @click="selectTab(group.status)"
            >
              {{ group.label }}
            </button>
          </div>

          <!-- 单列内容视口：原地平滑呼吸升降淡入 + 独立滚轮隔离 + 底部羽化渐变 -->
          <div class="relative min-h-0 w-full flex-1 pt-2">
            <div
              v-for="group in activeGroups"
              :key="group.status"
              class="ecall-kanban-single-page absolute inset-0"
              :class="{ 'is-active': group.status === activeGroupStatus }"
            >
              <OverlayScrollArea
                orientation="vertical"
                hide-scrollbar
                isolate-scroll
                class="ecall-kanban-fade-mask min-h-0 size-full"
                scroller-class="h-full flex flex-col gap-1.5 pb-8 pr-0.5"
                @wheel.stop
              >
                <KanbanIssueItem
                  v-for="issue in group.items"
                  :key="issue.number"
                  :issue="issue"
                  :assignee-name="resolveAssigneeName(issue.assignee)"
                  :avatar-url="resolveAvatar(issue.assignee)"
                  :relative-time="formatTime(issue.updatedAt || issue.createdAt)"
                />
              </OverlayScrollArea>
            </div>
          </div>
        </div>

        <!-- 模式 B：宽屏充裕模式（容器宽度 >= 500px）多列平铺，仅展示有任务的分类（空分类不显示） -->
        <OverlayScrollArea
          v-else
          orientation="horizontal"
          hide-scrollbar
          class="ecall-kanban-horizontal h-full min-h-0 w-full flex-1"
          scroller-class="flex h-full w-full gap-2.5 pb-1"
        >
          <section
            v-for="group in activeGroups"
            :key="group.status"
            class="ecall-kanban-column flex h-full min-w-[172px] flex-1 flex-col"
            :class="activeGroups.length === 1 ? 'max-w-[320px]' : ''"
          >
            <!-- 泳道头：小圆点 + 状态名 + 数量 -->
            <div class="flex shrink-0 items-center justify-between px-0.5 pb-1.5">
              <div class="flex items-center gap-1.5">
                <span class="size-2 rounded-full" :class="group.dotClass" />
                <span class="text-xs font-semibold text-base-content/85">
                  {{ group.label }}
                </span>
                <span class="font-mono text-xs tabular-nums text-base-content/40">
                  {{ group.total }}
                </span>
              </div>
            </div>

            <!-- 列内纵向滚动区：条目超出时列内竖向滚动，底部渐变淡出，滚动条隐藏，隔离滚轮事件防止多滚动条联动 -->
            <OverlayScrollArea
              orientation="vertical"
              hide-scrollbar
              isolate-scroll
              class="ecall-kanban-fade-mask min-h-0 w-full flex-1"
              scroller-class="h-full flex flex-col gap-1.5 pb-8 pr-0.5"
              @wheel.stop
            >
              <!-- 议题条目卡片 -->
              <KanbanIssueItem
                v-for="issue in group.items"
                :key="issue.number"
                :issue="issue"
                :assignee-name="resolveAssigneeName(issue.assignee)"
                :avatar-url="resolveAvatar(issue.assignee)"
                :relative-time="formatTime(issue.updatedAt || issue.createdAt)"
              />
            </OverlayScrollArea>
          </section>
        </OverlayScrollArea>
      </template>

      <!-- 全局空态 -->
      <div
        v-else
        class="flex min-h-[90px] flex-1 flex-col items-center justify-center gap-1.5 text-center"
      >
        <p class="text-xs leading-relaxed text-base-content/45">
          {{ t(decisionCount === 0 && progressCount === 0 ? "chat.kanban.cardEmptyAttention" : "chat.kanban.cardEmptyColumn") }}
        </p>
      </div>
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeftRight, ArrowUpRight, LayoutGrid } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import KanbanIssueItem from "./KanbanIssueItem.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import { formatRecentRelativeTime } from "../../../shared/utils/relative-time";
import {
  KANBAN_ATTENTION_ORDER,
  KANBAN_PROGRESS_ORDER,
  kanbanIssuesInOrder,
  type KanbanIssue,
} from "../../composables/use-kanban-board";

const STATUS_STYLE: Record<
  string,
  {
    dotClass: string;
  }
> = {
  in_review: {
    dotClass: "bg-warning",
  },
  blocked: {
    dotClass: "bg-error",
  },
  discussing: {
    dotClass: "bg-info",
  },
  in_progress: {
    dotClass: "bg-base-content/60",
  },
  done: {
    dotClass: "bg-success",
  },
};

const props = withDefaults(defineProps<{
  issues?: KanbanIssue[];
  leaderName?: string;
  leaderAvatarUrl?: string;
  personaNameMap?: Record<string, string>;
  personaAvatarUrlMap?: Record<string, string>;
  teamMembers?: Array<{ teamName?: string; agentId?: string; agentName?: string }>;
  avatarUrlMap?: Record<string, string>;
}>(), {
  issues: () => [],
  leaderName: "",
  leaderAvatarUrl: "",
  personaNameMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  teamMembers: () => [],
  avatarUrlMap: () => ({}),
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();

/** 当前时间戳，用于相对时间计算 */
const nowTick = ref(Date.now());

/** 决策态列需要用户参与的四类；进展态列推进中的两类。 */
const KANBAN_MODES = ["decision", "progress"] as const;
type KanbanTileMode = (typeof KANBAN_MODES)[number];

const decisionList = computed(() => kanbanIssuesInOrder(props.issues, KANBAN_ATTENTION_ORDER));
const progressList = computed(() => kanbanIssuesInOrder(props.issues, KANBAN_PROGRESS_ORDER));
const decisionCount = computed(() => decisionList.value.length);
const progressCount = computed(() => progressList.value.length);

/** 默认按内容落位：决策类有东西就显示决策，没有就落到进展；两边都空留在决策显示空态。 */
const autoMode = computed<KanbanTileMode>(() => {
  if (decisionCount.value > 0) return "decision";
  if (progressCount.value > 0) return "progress";
  return "decision";
});

/** 只在本次会话里记用户点过的选择；不落盘，重新打开首页时重新按内容落位。 */
const manualMode = ref<KanbanTileMode | null>(null);
const mode = computed<KanbanTileMode>(() => manualMode.value ?? autoMode.value);

const visibleList = computed(() => (mode.value === "decision" ? decisionList.value : progressList.value));
const visibleCount = computed(() => visibleList.value.length);

function toggleMode(): void {
  manualMode.value = mode.value === "decision" ? "progress" : "decision";
}

/** 数量徽章在决策态是「等你处理」的警示色，进展态只是计数，换中性色免得误读成告警。 */
const badgeClass = computed(() =>
  mode.value === "decision"
    ? "bg-warning/15 text-warning"
    : "bg-base-content/10 text-base-content/60",
);

const groups = computed(() => {
  const order = mode.value === "decision" ? KANBAN_ATTENTION_ORDER : KANBAN_PROGRESS_ORDER;
  const list = visibleList.value;
  return order.map((status) => {
    const items = list.filter((issue) => String(issue.status || "").trim() === status);
    const style = STATUS_STYLE[status] || { dotClass: "bg-base-content/40" };
    return {
      status,
      label: String(t(`chat.kanban.status.${status}`)),
      dotClass: style.dotClass,
      total: items.length,
      items,
    };
  });
});

/** 核心规则：空的不显示！单列 Tab 栏仅保留有实际议题的状态分类 */
const activeGroups = computed(() => groups.value.filter((g) => g.total > 0));

// ==================== 响应式宽度与单列自适应 ====================
const containerRef = ref<HTMLElement | null>(null);
/** 记录外层磁贴流宿主容器的真实宽度，绝不观察卡片自身宽度，杜绝跨列改变自身宽度引发的死循环抖动 */
const flowContainerWidth = ref(typeof window !== "undefined" ? window.innerWidth : 800);
let resizeObserver: ResizeObserver | null = null;

/**
 * 手机/窄屏（单列 Tab 栏）模式触发条件：
 * 1. 外部宿主容器（非卡片自身）宽度 < 500px；
 * 2. 且有至少 2 个任务分类（仅 1 个分类无需 Tab 栏切换，直接单泳道展示）。
 */
const isNarrow = computed(() => flowContainerWidth.value < 500 && activeGroups.value.length >= 2);

onMounted(() => {
  if (typeof ResizeObserver !== "undefined") {
    // 观察外层网格容器 .ecall-home-flow 或其父容器，确保获取的是视口可用宽度
    const hostEl = containerRef.value?.closest(".ecall-home-flow")?.parentElement
      || containerRef.value?.closest(".ecall-home-flow")
      || document.documentElement;

    if (hostEl) {
      flowContainerWidth.value = hostEl.clientWidth || window.innerWidth;
      resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          flowContainerWidth.value = entry.contentRect.width;
          void nextTick(updateUnderline);
        }
      });
      resizeObserver.observe(hostEl);
    }
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

// ==================== 单列 Tab 切换与手写下划线动画 ====================
const activeGroupStatus = ref<string>("");
const tabBarRef = ref<HTMLElement | null>(null);
const underlineStyle = ref<{ width: string; transform: string }>({
  width: "0px",
  transform: "translateX(0px)",
});

watch(
  activeGroups,
  (newGroups) => {
    if (!newGroups.length) {
      activeGroupStatus.value = "";
      return;
    }
    const exists = newGroups.some((g) => g.status === activeGroupStatus.value);
    if (!exists) {
      activeGroupStatus.value = newGroups[0].status;
    }
    void nextTick(updateUnderline);
  },
  { immediate: true },
);

const activeIndex = computed(() => {
  const idx = activeGroups.value.findIndex((g) => g.status === activeGroupStatus.value);
  return idx >= 0 ? idx : 0;
});

function selectTab(status: string) {
  activeGroupStatus.value = status;
  void nextTick(updateUnderline);
}

function updateUnderline() {
  if (!tabBarRef.value) return;
  const btns = tabBarRef.value.querySelectorAll<HTMLElement>(".ecall-kanban-tab-btn");
  const target = btns[activeIndex.value];
  if (target) {
    underlineStyle.value = {
      width: `${target.offsetWidth}px`,
      transform: `translateX(${target.offsetLeft}px)`,
    };
  }
}

watch([mode, isNarrow], () => {
  void nextTick(updateUnderline);
});

// ==================== 自适应尺寸与跨列变动范围 ====================
/** 当前视图下单列或多列中最大任务项数 */
const maxVisibleItems = computed(() => {
  if (visibleCount.value === 0) return 0;
  if (isNarrow.value) {
    const current = activeGroups.value.find((g) => g.status === activeGroupStatus.value);
    return current ? current.items.length : 0;
  }
  if (!activeGroups.value.length) return 0;
  return Math.max(...activeGroups.value.map((g) => g.items.length));
});

/** 磁贴自适应高度（变动范围：6.5rem ~ 21.5rem） */
const cardHeight = computed(() => {
  if (visibleCount.value === 0) {
    return "6.5rem";
  }
  const count = maxVisibleItems.value;
  if (count <= 1) {
    return "11.25rem";
  }
  if (count === 2) {
    return "16.25rem";
  }
  return "21.5rem";
});

/** 磁贴跨列自适应：根据可见分类数决定横向跨格数 */
const kanbanGridClass = computed(() => {
  if (flowContainerWidth.value < 500) {
    return "col-span-full";
  }
  if (visibleCount.value === 0) {
    return "ecall-kanban-span-2 is-empty";
  }
  const categoryCount = activeGroups.value.length;
  if (categoryCount <= 1) {
    // 只有 1 个分类（如只有 1 项议题）：作为普通双格卡（跨 2 格、跨 2 行）与其它磁贴并排
    return "ecall-kanban-span-2";
  }
  if (categoryCount === 2) {
    // 2 个分类：跨 4 格（容纳 2 个泳道）
    return "ecall-kanban-span-4";
  }
  // 3 个及以上分类：占满整行大看板
  return "col-span-full";
});

/** 磁贴自适应高度样式 */
const kanbanGridStyle = computed(() => {
  if (isNarrow.value || activeGroups.value.length >= 3) {
    return { height: cardHeight.value };
  }
  if (visibleCount.value === 0) {
    return { height: "6.5rem" };
  }
  return { height: "14rem" };
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

function formatTime(dateStr?: string): string {
  if (!dateStr) return "";
  return formatRecentRelativeTime(dateStr, nowTick.value, t);
}
</script>

<style scoped>
/* 自适应磁贴：高度与宽度均根据内容弹性变动 */
.ecall-home-kanban {
  min-height: 6.5rem;
  max-height: 21.5rem;
  transition: all 0.24s cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* 0 个任务（空态）：跨 2 格宽、跨 1 行高，与普通小磁贴无异 */
.ecall-home-kanban.ecall-kanban-span-2.is-empty {
  grid-column: span 2;
  grid-row: span 1;
  height: 6.5rem !important;
}

/* 1 个分类（如只有 1 项议题）：跨 2 格宽、跨 2 行高，与其他磁贴并排紧凑混排 */
.ecall-home-kanban.ecall-kanban-span-2 {
  grid-column: span 2;
  grid-row: span 2;
  height: 14rem !important;
}

/* 2 个分类：跨 4 格宽、跨 2 行高（容纳 2 列泳道） */
.ecall-home-kanban.ecall-kanban-span-4 {
  grid-column: span 4;
  grid-row: span 2;
  height: 14rem !important;
}

/* 3 个及以上分类或窄屏：占满整行 */
.ecall-home-kanban.col-span-full {
  grid-column: 1 / -1;
  grid-row: auto;
}

/* 泳道纵向底部渐变淡出：自然羽化截断边缘 */
.ecall-kanban-fade-mask {
  -webkit-mask-image: linear-gradient(to bottom, #000 0%, #000 calc(100% - 2.5rem), transparent 100%);
  mask-image: linear-gradient(to bottom, #000 0%, #000 calc(100% - 2.5rem), transparent 100%);
  touch-action: pan-y;
}

/* 移动端/窄屏横向泳道轻吸附与平滑触摸体验 */
.ecall-kanban-horizontal :deep(.overlay-scroll-area-scroller) {
  scroll-snap-type: x proximity;
  -webkit-overflow-scrolling: touch;
}

.ecall-kanban-column {
  scroll-snap-align: start;
}

/* 纯手写动态下划线位移动画（弹性曲线） */
.ecall-kanban-underline {
  transition: transform 0.28s cubic-bezier(0.2, 0.8, 0.2, 1), width 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
  will-change: transform, width;
}

/* 单列内容页面：原地平滑呼吸升降淡入过渡 */
.ecall-kanban-single-page {
  opacity: 0;
  transform: scale(0.98) translateY(6px);
  pointer-events: none;
  transition: opacity 0.24s cubic-bezier(0.4, 0, 0.2, 1), transform 0.24s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.ecall-kanban-single-page.is-active {
  opacity: 1;
  transform: scale(1) translateY(0);
  pointer-events: auto;
}
</style>
