<template>
  <CardShell
    tone="warning"
    :icon="LayoutGrid"
    :label="t('chat.kanban.cardTitle')"
    class="ecall-home-kanban"
    :style="tileSpanStyle"
  >
    <template #leading>
      <div class="flex min-w-0 flex-1 items-center">
        <!-- 切换按钮表当前态，点一下换到另一态。展示名是「讨论 / 进展」；
             另一侧一条议题都没有时保留按钮但不可点，避免切到一屏空白 -->
        <button
          type="button"
          class="btn btn-ghost btn-xs h-6 min-w-0 gap-1 px-1.5 text-xs font-medium text-base-content/70 transition-colors hover:bg-base-content/10 hover:text-base-content"
          :class="canToggleMode ? '' : 'cursor-default opacity-60 hover:bg-transparent hover:text-base-content/70'"
          :disabled="!canToggleMode"
          :aria-label="t(`chat.kanban.view.${mode}`)"
          :aria-disabled="!canToggleMode"
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
          <!-- 下划线 Tab 栏：纯文本、无数量、空分类不显示 -->
          <UnderlineTabs
            class="shrink-0"
            size="sm"
            bordered
            :tabs="tabItems"
            :active-key="activeGroupStatus"
            @update:active-key="selectTab"
          />

          <!-- 单列内容视口：原地平滑呼吸升降淡入 + 独立滚轮隔离 + 底部羽化渐变。
               顶部留白给在页面上而不是父级 padding：页面是 absolute 定位，父级 padding 对它不生效 -->
          <div class="relative min-h-0 w-full flex-1">
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
                class="ecall-fade-bottom touch-pan-y min-h-0 size-full"
                scroller-class="h-full flex flex-col gap-1.5 pt-3 pb-8 pr-0.5"
                @wheel.stop
              >
                <KanbanIssueCard
                  v-for="issue in group.items"
                  :key="issue.number"
                  :issue="issue"
                  :assignee-name="resolveAssigneeName(issue.assignee)"
                  :avatar-url="resolveAvatar(issue.assignee)"
                  :approving-number="approvingNumber"
                  density="tile"
                  referenceable
                  @select="emit('open')"
                  @approve="emit('approve', $event)"
                  @open-plan="emit('openPlan', $event)"
                  @reference="emit('reference', $event)"
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
              class="ecall-fade-bottom touch-pan-y min-h-0 w-full flex-1"
              scroller-class="h-full flex flex-col gap-1.5 pb-8 pr-0.5"
              @wheel.stop
            >
              <!-- 议题条目卡片 -->
              <KanbanIssueCard
                v-for="issue in group.items"
                :key="issue.number"
                :issue="issue"
                :assignee-name="resolveAssigneeName(issue.assignee)"
                :avatar-url="resolveAvatar(issue.assignee)"
                :approving-number="approvingNumber"
                density="tile"
                referenceable
                @select="emit('open')"
                @approve="emit('approve', $event)"
                @open-plan="emit('openPlan', $event)"
                @reference="emit('reference', $event)"
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
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeftRight, ArrowUpRight, LayoutGrid } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import KanbanIssueCard from "../kanban/KanbanIssueCard.vue";
import OverlayScrollArea from "../../../shared/components/OverlayScrollArea.vue";
import UnderlineTabs from "../../../shared/components/UnderlineTabs.vue";
import {
  KANBAN_ATTENTION_ORDER,
  KANBAN_PROGRESS_ORDER,
  kanbanDefaultGroupStatus,
  kanbanIssuesInOrder,
  type KanbanIssue,
} from "../../composables/use-kanban-board";

const STATUS_STYLE: Record<
  string,
  {
    /** 色点与选中下划线用同一个背景色 */
    dotClass: string;
    /** 选中时文字色；必须写成字面量，Tailwind 才扫得到 */
    textClass: string;
  }
> = {
  in_review: {
    dotClass: "bg-warning",
    textClass: "text-warning",
  },
  blocked: {
    dotClass: "bg-error",
    textClass: "text-error",
  },
  discussing: {
    dotClass: "bg-info",
    textClass: "text-info",
  },
  in_progress: {
    dotClass: "bg-base-content/60",
    textClass: "text-base-content/60",
  },
  done: {
    dotClass: "bg-success",
    textClass: "text-success",
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
  approvingNumber?: number | null;
  /** 占几列；0 表示占满整行。由宿主决定，磁贴自己不算。 */
  colSpan?: number;
  /** 占几行。由宿主决定，磁贴自己不算。 */
  rowSpan?: number;
}>(), {
  issues: () => [],
  leaderName: "",
  leaderAvatarUrl: "",
  personaNameMap: () => ({}),
  personaAvatarUrlMap: () => ({}),
  teamMembers: () => [],
  avatarUrlMap: () => ({}),
  approvingNumber: null,
  colSpan: 2,
  rowSpan: 1,
});

const emit = defineEmits<{
  (e: "open"): void;
  (e: "approve", number: number): void;
  (e: "openPlan", path: string): void;
  /** 点议题编号：把这条议题挂到输入框当引用，不打开看板 */
  (e: "reference", issue: KanbanIssue): void;
}>();

const { t } = useI18n();

/** 决策态列需要用户参与的四类；进展态列推进中的两类。 */
const KANBAN_MODES = ["decision", "progress"] as const;
type KanbanTileMode = (typeof KANBAN_MODES)[number];

const decisionList = computed(() => kanbanIssuesInOrder(props.issues, KANBAN_ATTENTION_ORDER));
const progressList = computed(() => kanbanIssuesInOrder(props.issues, KANBAN_PROGRESS_ORDER));
const decisionCount = computed(() => decisionList.value.length);
const progressCount = computed(() => progressList.value.length);

/** 默认按内容落位：待审属于决策态，所以有待审就一定落在决策态；退一步看其它决策类，
    都空才落到进展态。 */
const autoMode = computed<KanbanTileMode>(() => {
  if (decisionCount.value > 0) return "decision";
  if (progressCount.value > 0) return "progress";
  return "decision";
});

/** 只在本次会话里记用户点过的选择；不落盘，重新打开首页时重新按内容落位。 */
const manualMode = ref<KanbanTileMode | null>(null);

/** 两侧各自有没有内容：这是「能不能切过去」的唯一依据。 */
const hasDecision = computed(() => decisionCount.value > 0);
const hasProgress = computed(() => progressCount.value > 0);
/** 两侧都有内容才允许切换；只有一侧有内容时按钮保留（它同时是当前态的名字），但不可点。 */
const canToggleMode = computed(() => hasDecision.value && hasProgress.value);

const mode = computed<KanbanTileMode>(() => {
  const manual = manualMode.value;
  if (!manual) return autoMode.value;
  // 手动选的那一侧被清空、另一侧还有内容时自动退回，绝不把用户停在一屏空白上
  const manualEmpty = manual === "decision" ? !hasDecision.value : !hasProgress.value;
  const other = manual === "decision" ? "progress" : "decision";
  const otherHas = other === "decision" ? hasDecision.value : hasProgress.value;
  return manualEmpty && otherHas ? other : manual;
});

const visibleList = computed(() => (mode.value === "decision" ? decisionList.value : progressList.value));
const visibleCount = computed(() => visibleList.value.length);

function toggleMode(): void {
  if (!canToggleMode.value) return;
  manualMode.value = mode.value === "decision" ? "progress" : "decision";
}

/** 数量徽章在决策态是「等你处理」的警示色，进展态只是计数，换中性色免得误读成告警。 */
const badgeClass = computed(() =>
  mode.value === "decision"
    ? "bg-warning/15 text-warning"
    : "bg-base-content/10 text-base-content/60",
);

/** 同一分类内按编号排：讨论态（要你处理）升序，先来的先处理；进展态（已在推进或做完）倒序，最近的在上。 */
const groups = computed(() => {
  const order = mode.value === "decision" ? KANBAN_ATTENTION_ORDER : KANBAN_PROGRESS_ORDER;
  const list = visibleList.value;
  const direction = mode.value === "decision" ? 1 : -1;
  return order.map((status) => {
    const items = list
      .filter((issue) => String(issue.status || "").trim() === status)
      .sort((a, b) => (a.number - b.number) * direction);
    const style = STATUS_STYLE[status] || { dotClass: "bg-base-content/40", textClass: "text-base-content/40" };
    return {
      status,
      label: String(t(`chat.kanban.status.${status}`)),
      dotClass: style.dotClass,
      textClass: style.textClass,
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
/** 默认停在哪一列：有待审就停在待审，否则退回该模式第一个非空分类。
    空分类不参与候选，所以待审被全部通过后不会留下一列空白，而是落到下一个有内容的分类。 */
const defaultGroupStatus = computed(() => kanbanDefaultGroupStatus(activeGroups.value));

/** 用户点过的 Tab 高于默认判定；他选的那个分类消失后（比如待审已清空）自动退回默认。 */
const manualGroupStatus = ref<string>("");
const activeGroupStatus = computed(() => {
  const manual = manualGroupStatus.value;
  if (manual && activeGroups.value.some((group) => group.status === manual)) return manual;
  return defaultGroupStatus.value;
});

const tabItems = computed(() => activeGroups.value.map((group) => ({
  key: group.status,
  label: group.label,
  accentClass: group.textClass,
  indicatorClass: group.dotClass,
})));

/** 选中即写入手动选择；下划线的位移与宽度由 UnderlineTabs 自己量。 */
function selectTab(status: string) {
  manualGroupStatus.value = status;
}

// ==================== 尺寸（由宿主给定） ====================
/** 占几列几行由宿主算好传进来；磁贴只把它套到网格上，不参与尺寸决策，也不靠内容把格子撑大。 */
const tileSpanStyle = computed(() => ({
  gridColumn: props.colSpan > 0 ? `span ${props.colSpan}` : "1 / -1",
  gridRow: `span ${props.rowSpan}`,
}));

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

<style scoped>
/* 高度 100% 贴住宿主给的格子；内容超出在卡片内部滚动 */
.ecall-home-kanban {
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  transition: all 0.24s cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* 移动端/窄屏横向泳道轻吸附与平滑触摸体验 */
.ecall-kanban-horizontal :deep(.overlay-scroll-area-scroller) {
  scroll-snap-type: x proximity;
  -webkit-overflow-scrolling: touch;
}

.ecall-kanban-column {
  scroll-snap-align: start;
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
