<template>
  <div class="grid gap-3">
    <!-- ============ 成就卡 ============ -->
    <ConfigCard>
      <div class="stats stats-vertical w-full py-3 sm:stats-horizontal">
        <div class="stat">
          <div class="stat-title text-xs">{{ t("config.welcome.trail.achTotal") }}</div>
          <div class="stat-value text-2xl">{{ formatTokens(achievementTotalTokens) }}</div>
        </div>
        <div class="stat">
          <div class="stat-title text-xs">{{ t("config.welcome.trail.achPeak") }}</div>
          <div class="stat-value text-2xl">{{ formatTokens(achievementPeak.tokens) }}</div>
          <div v-if="achievementPeak.date" class="stat-desc">{{ achievementPeak.date }}</div>
        </div>
        <div class="stat">
          <div class="stat-title text-xs">{{ t("config.welcome.trail.achCurrentStreak") }}</div>
          <div class="stat-value text-2xl">{{ achievementCurrentStreak }}<span class="text-sm opacity-60"> {{ t("config.welcome.trail.dayUnit") }}</span></div>
        </div>
        <div class="stat">
          <div class="stat-title text-xs">{{ t("config.welcome.trail.achBestStreak") }}</div>
          <div class="stat-value text-2xl">{{ achievementBestStreak }}<span class="text-sm opacity-60"> {{ t("config.welcome.trail.dayUnit") }}</span></div>
        </div>
      </div>
    </ConfigCard>

    <!-- ============ 今日卡 ============ -->
    <ConfigCard>
      <!-- 加载态 -->
      <div v-if="loading && !todayData && !historyData" class="flex items-center justify-center gap-2 py-6 text-sm opacity-70">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{{ t("common.loading") }}</span>
      </div>

      <!-- 错误态 -->
      <div v-else-if="errorText" class="rounded-box bg-error/10 px-3 py-2.5 text-sm text-error">
        {{ errorText }}
      </div>

      <template v-else>
        <!-- ============ 今天 ============ -->
        <div v-if="todayData" class="space-y-2.5 py-3">
          <div class="flex items-center gap-2">
            <span class="badge badge-primary badge-sm">{{ t("config.welcome.trail.viewToday") }}</span>
            <span class="text-sm opacity-70">
              {{ t("config.welcome.trail.totalTokens") }}
            </span>
          </div>

          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <div class="text-3xl font-bold tabular-nums">{{ formatTokens(todayData.totals.totalTokens) }}</div>
            </div>
            <div v-if="todayData.hourly.length" class="join join-horizontal shrink-0">
              <button
                type="button"
                class="btn btn-xs join-item"
                :class="chartMode === 'area' ? 'btn-primary' : 'btn-outline'"
                @click="chartMode = 'area'"
              >
                {{ t("config.welcome.trail.chartArea") }}
              </button>
              <button
                type="button"
                class="btn btn-xs join-item"
                :class="chartMode === 'polar' ? 'btn-primary' : 'btn-outline'"
                @click="chartMode = 'polar'"
              >
                {{ t("config.welcome.trail.chartPolar") }}
              </button>
            </div>
          </div>

          <!-- 24 小时堆叠图表（柱状图/面积图切换） -->
          <div v-if="todayData.hourly.length" class="space-y-1">

            <!-- 图表区：Chart.js 渲染 -->
            <ChartView :config="chartConfig" class="h-28 w-full" />

            <!-- 模型图例：流式布局，完整显示 供应商·模型·思考等级 -->
            <div class="flex flex-wrap gap-x-3 gap-y-1 pt-1">
              <span
                v-for="m in todayModelDetails"
                :key="m.model"
                class="flex items-center gap-1.5 text-caption opacity-80"
              >
                <span class="size-2 shrink-0 rounded-field" :style="{ backgroundColor: modelColor(m.model) }"></span>
                <span class="min-w-0">{{ modelDisplayName(m) }}</span>
              </span>
            </div>
          </div>
        </div>
      </template>
    </ConfigCard>

    <!-- ============ 历史卡 ============ -->
    <ConfigCard>
      <template v-if="historyData">
        <div class="space-y-2.5 py-3">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="text-sm opacity-70">{{ t("config.welcome.trail.yearTotal", { year: historyData.year }) }}</div>
            <div v-if="historyData.years.length > 1" class="join join-horizontal">
              <button
                v-for="year in historyData.years"
                :key="year"
                class="btn btn-xs join-item"
                :class="year === historyData.year ? 'btn-primary' : 'btn-ghost'"
                type="button"
                @click="switchYear(year)"
              >
                {{ year }}
              </button>
            </div>
          </div>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <div class="text-3xl font-bold tabular-nums">{{ formatTokens(historyData.totals.totalTokens) }}</div>
            </div>
          </div>

          <!-- 年历热力（GitHub 式） -->
          <div v-if="historyData.calendar.length" class="w-full pr-1">
            <!-- 月份标记行：与该月 1 号所在周列对齐 -->
            <div class="grid gap-px pb-0.5" :style="calendarGridStyle">
              <div
                v-for="(week, weekIndex) in calendarWeeks"
                :key="`m-${weekIndex}`"
                class="min-w-0 overflow-hidden text-left text-caption leading-none opacity-50"
                :style="{ gridColumn: String(weekIndex + 1) }"
              >
                <span v-if="week.monthLabel" class="inline-block align-top [writing-mode:vertical-rl]">{{ cnMonth(week.monthLabel) }}</span>
              </div>
            </div>
            <div class="flex w-full gap-1">
              <!-- 周几标签列 -->
              <div class="grid shrink-0 grid-rows-7 gap-px pr-1">
                <span v-for="label in weekdayLabels" :key="label" class="flex h-3 items-center text-caption opacity-50">{{ label }}</span>
              </div>
              <!-- 年历 grid：列=周序号，行=星期几，列宽严格 1fr 均分 -->
              <div class="grid min-w-0 flex-1 gap-px" :style="calendarGridStyle">
                <div
                  v-for="cell in flatCalendarDays"
                  :key="cell.day.date"
                  class="relative aspect-square rounded-[3px]"
                  :style="{ ...calendarCellStyle(cell.day), gridColumn: String(cell.col), gridRow: String(cell.row) }"
                  @mouseenter="showDayTooltip($event, cell.day)"
                  @mouseleave="hideDayTooltip()"
                ></div>
              </div>
            </div>
            <!-- Less/More 色阶图例：与格子共用当年的分位锚点 -->
            <div class="mt-1 flex w-full items-center justify-end gap-1 text-caption opacity-50">
              <span>{{ t("config.welcome.trail.legendLess") }}</span>
              <div class="flex gap-px">
                <div
                  v-for="pct in calendarLegendPcts"
                  :key="pct"
                  class="h-2 w-3 rounded-[3px]"
                  :style="legendStyle(pct)"
                ></div>
              </div>
              <span>{{ t("config.welcome.trail.legendMore") }}</span>
            </div>
          </div>

          <!-- 叙事行 -->
          <div v-if="historyData.activePeriodLabel" class="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm opacity-80">
            <span v-if="historyData.activePeriodLabel" class="flex items-center gap-1.5">
              <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-3.5 opacity-50">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z" />
              </svg>
              {{ t("config.welcome.trail.activePeriod", { period: t(`config.welcome.trail.periods.${historyData.activePeriodLabel}`) }) }}
            </span>
          </div>
        </div>
      </template>
    </ConfigCard>
  </div>

  <!-- 历史格子 tooltip：Teleport 到 body，避免被卡片 overflow 裁剪 -->
  <Teleport to="body">
    <div
      v-if="dayTooltip"
      class="pointer-events-none fixed z-50 -translate-x-1/2 whitespace-nowrap rounded bg-base-content px-1.5 py-0.5 text-caption text-base-100"
      :style="{ left: `${dayTooltip.left}px`, top: `${dayTooltip.top}px` }"
    >
      {{ dayTooltip.date }} · {{ formatTokens(dayTooltip.totalTokens) }}
      <span v-if="dayTooltip.conversationCount > 0"> · {{ dayTooltip.conversationCount }} {{ t("config.welcome.trail.conversationCount") }}</span>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import { toErrorMessage } from "../../../../utils/error";
import { apiConfigDisplayName } from "../../utils/api-config-display";
import ConfigCard from "../../components/ConfigCard.vue";
import ChartView from "./ChartView.vue";

type TrailView = "today" | "history";

type UsageTrailWallTotals = {
  conversationCount: number;
  weightedTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
};

type UsageTrailWallHour = {
  hour: number;
  totalTokens: number;
  conversationCount: number;
  models: UsageTrailWallModel[];
};

type UsageTrailWallModel = {
  model: string;
  tokens: number;
  providerLabel: string;
  reasoningEffort: string;
};

type UsageTrailWallDay = {
  date: string;
  totalTokens: number;
  conversationCount: number;
};

type UsageTrailWallView = {
  generatedAt: string;
  view: TrailView;
  totals: UsageTrailWallTotals;
  epochTotals: UsageTrailWallTotals | null;
  hourly: UsageTrailWallHour[];
  year: string;
  years: string[];
  calendar: UsageTrailWallDay[];
  topConversationLabel: string | null;
  topConversationPercent: number | null;
  activePeriodLabel: string | null;
};

const { t, tm } = useI18n();
const todayData = ref<UsageTrailWallView | null>(null);
const historyData = ref<UsageTrailWallView | null>(null);
const loading = ref(false);
const errorText = ref("");
const dayTooltip = ref<{ date: string; totalTokens: number; conversationCount: number; left: number; top: number } | null>(null);
let trailUnmounted = false;

function showDayTooltip(event: MouseEvent, day: UsageTrailWallDay): void {
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  dayTooltip.value = {
    date: day.date,
    totalTokens: day.totalTokens,
    conversationCount: day.conversationCount,
    left: rect.left + rect.width / 2,
    top: rect.top - 6,
  };
}

function hideDayTooltip(): void {
  dayTooltip.value = null;
}

// ============ 今日图表（面积图 / 极区玫瑰图） ============
const chartMode = ref<"area" | "polar">("area");

// 模型颜色板：与参考图一致的 5 色
const MODEL_COLORS = [
  "#22c55e", // 绿
  "#3b82f6", // 蓝
  "#a855f7", // 紫
  "#06b6d4", // 浅蓝
  "#f97316", // 橙
];

// 今日全部模型列表（按首次出现顺序），供图例与堆叠顺序
const todayModels = computed<string[]>(() => {
  const seen: string[] = [];
  const set = new Set<string>();
  for (const cell of todayData.value?.hourly || []) {
    for (const m of cell.models || []) {
      if (!set.has(m.model)) {
        set.add(m.model);
        seen.push(m.model);
      }
    }
  }
  return seen;
});

// 模型完整信息（按首次出现顺序），供流式图例显示 供应商·模型·思考等级
const todayModelDetails = computed<UsageTrailWallModel[]>(() => {
  const seen: UsageTrailWallModel[] = [];
  const set = new Set<string>();
  for (const cell of todayData.value?.hourly || []) {
    for (const m of cell.models || []) {
      if (!set.has(m.model)) {
        set.add(m.model);
        seen.push(m);
      }
    }
  }
  return seen;
});

function modelDisplayName(m: UsageTrailWallModel): string {
  return apiConfigDisplayName(m.providerLabel, modelBaseName(m.model), m.reasoningEffort, t);
}

// 后端聚合键为 provider_key::model_name，显示时拆出纯模型名
function modelBaseName(key: string): string {
  const idx = key.lastIndexOf("::");
  return idx >= 0 ? key.slice(idx + 2) : key;
}

function modelColor(model: string): string {
  const index = todayModels.value.indexOf(model);
  return MODEL_COLORS[index % MODEL_COLORS.length] ?? "#94a3b8";
}

// Chart.js 配置：平滑堆叠面积图（24 小时坐标轴）/ 极区玫瑰图（按模型聚合今日总量）
const chartConfig = computed<Record<string, unknown>>(() => {
  if (chartMode.value === "polar") {
    // 极区图：每个模型一个扇区，半径 = 该模型今日 total_tokens 之和
    const totalsByModel = new Map<string, number>();
    for (const cell of todayData.value?.hourly || []) {
      for (const m of cell.models || []) {
        totalsByModel.set(m.model, (totalsByModel.get(m.model) ?? 0) + m.tokens);
      }
    }
    const models = todayModels.value.filter((model) => (totalsByModel.get(model) ?? 0) > 0);
    const colors = models.map(
      (model, index) => MODEL_COLORS[index % MODEL_COLORS.length] ?? "#94a3b8",
    );
    const modelDetailByKey = new Map(todayModelDetails.value.map((d) => [d.model, d]));
    return {
      type: "polarArea",
      data: {
        labels: models.map((model) => {
          const detail = modelDetailByKey.get(model);
          return detail ? modelDisplayName(detail) : model;
        }),
        datasets: [
          {
            data: models.map((model) => totalsByModel.get(model) ?? 0),
            backgroundColor: colors.map((color) => `${color}CC`),
            borderColor: colors,
            borderWidth: 1.5,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: {
            displayColors: true,
            callbacks: {
              label: (item: { label?: string; formattedValue?: string }) =>
                ` ${item.label ?? ""}: ${formatTokens(Number(item.formattedValue ?? 0))}`,
            },
          },
        },
        scales: {
          r: {
            ticks: {
              display: false,
              stepSize: 1,
            },
            grid: { color: "rgba(128,128,128,0.12)" },
            angleLines: { color: "rgba(128,128,128,0.12)" },
          },
        },
      },
    };
  }
  // 面积图：固定 24 小时坐标轴，按模型堆叠。
  // 凌晨 4 点分界：x 轴按分界日顺序排列（04→23→00→03），tooltip 同步显示实际小时。
  const cells = todayData.value?.hourly || [];
  const labels = Array.from({ length: 24 }, (_, index) => String((index + 4) % 24).padStart(2, "0"));
  const datasets = todayModelDetails.value.map((m, index) => {
    const color = MODEL_COLORS[index % MODEL_COLORS.length] ?? "#94a3b8";
    const data = labels.map((label) => {
      const cell = cells.find((item) => String(item.hour).padStart(2, "0") === label);
      const item = (cell?.models || []).find((entry) => entry.model === m.model);
      return item?.tokens ?? 0;
    });
    return {
      label: modelDisplayName(m),
      data,
      borderColor: color,
      backgroundColor: color,
      fill: true,
      tension: 0.4,
      pointRadius: 0,
      borderWidth: 1.5,
    };
  });
  return {
    type: "line",
    data: { labels, datasets },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { display: false },
        tooltip: {
          displayColors: true,
          callbacks: {
            title: (items: { label?: string }[]) => {
              const hour = items[0]?.label ?? "";
              return `${hour}:00`;
            },
            label: (item: { dataset?: { label?: string }; formattedValue?: string }) =>
              ` ${item.dataset?.label ?? ""}: ${formatTokens(Number(item.formattedValue ?? 0))}`,
          },
        },
      },
      scales: {
        x: {
          stacked: true,
          grid: { display: false },
          ticks: { font: { size: 10 }, autoSkip: true, maxTicksLimit: 12 },
        },
        y: {
          stacked: true,
          grid: { color: "rgba(128,128,128,0.12)" },
          ticks: {
            callback: (value: number) => formatTokens(value),
            font: { size: 10 },
          },
        },
      },
    },
  };
});

const weekdayLabels = computed(() => {
  const weekdays = ["一", "二", "三", "四", "五", "六", "日"];
  return weekdays.map((day) => (day === "一" || day === "三" || day === "五" ? day : ""));
});

// 平滑色阶：0 无数据为灰底，有数据从 25% 起缓升到 100% 主题色
// 锚点取自当前年份非零用量的 25/50/75 分位（含最小与最大值），token 取对数后
// 在锚点间线性插值；分位点随数据分布移动，日常用量不会被压在同一片极浅区域
const CALENDAR_PCTS = [25, 44, 63, 81, 100];

type CalendarScale = { logs: number[]; pcts: number[] };

const calendarScale = computed<CalendarScale | null>(() => {
  const tokens = (historyData.value?.calendar ?? [])
    .map((day) => day.totalTokens)
    .filter((value) => value > 0)
    .sort((a, b) => a - b);
  if (!tokens.length) return null;
  const at = (p: number) => Math.log10(tokens[Math.min(tokens.length - 1, Math.floor(p * tokens.length))]);
  const logs = [at(0), at(0.25), at(0.5), at(0.75), at(1)];
  // 非零天数太少或取值过于集中时，分位点会重合到同一个值上，插值会把整年
  // 压成一片同色。此时改用去重后的真实取值当锚点，让每个实际档位都落在
  // 一个可辨的色深上，而不是让振幅更大的那几天去点亮整条色阶
  if (logs.some((log, i) => i > 0 && log <= logs[i - 1])) {
    const distinct = [...new Set(tokens)].sort((a, b) => a - b).map((value) => Math.log10(value));
    // 全年每天用量完全相同，没有高低可分，统一取最深一档
    if (distinct.length < 2) return { logs: distinct, pcts: [100] };
    return {
      logs: distinct,
      pcts: distinct.map((_, i) => CALENDAR_PCTS[0] + ((100 - CALENDAR_PCTS[0]) * i) / (distinct.length - 1)),
    };
  }
  return { logs, pcts: CALENDAR_PCTS };
});

// 图例色块与格子走同一套锚点，落地色与该年实际分位一致
// 锚点可能远多于图例能放的格数，按色深区间等距取样，最多五格
const calendarLegendPcts = computed<number[]>(() => {
  const pcts = calendarScale.value?.pcts ?? CALENDAR_PCTS;
  if (pcts.length <= 5) return pcts;
  return Array.from({ length: 5 }, (_, i) => pcts[Math.round((i * (pcts.length - 1)) / 4)]);
});

function calendarPct(tokens: number): number | null {
  if (tokens <= 0) return null;
  const scale = calendarScale.value;
  if (!scale) return null;
  const { logs, pcts } = scale;
  const log = Math.log10(tokens);
  const last = logs.length - 1;
  if (log <= logs[0]) return pcts[0];
  if (log >= logs[last]) return pcts[last];
  for (let i = 1; i <= last; i += 1) {
    if (log <= logs[i]) {
      const ratio = (log - logs[i - 1]) / (logs[i] - logs[i - 1]);
      return pcts[i - 1] + (pcts[i] - pcts[i - 1]) * ratio;
    }
  }
  return pcts[last];
}

function cnMonth(datePrefix: string): string {
  const months = tm("config.welcome.trail.months") as unknown as string[];
  return months[Number(datePrefix.slice(5))] || "";
}

type TrailWeek = { days: UsageTrailWallDay[]; monthLabel: string | null };

const calendarWeeks = computed<TrailWeek[]>(() => {
  if (!historyData.value || !historyData.value.calendar.length) return [];
  const weeks: TrailWeek[] = [];
  let currentWeek: UsageTrailWallDay[] = [];
  for (const day of historyData.value.calendar) {
    const date = new Date(`${day.date}T00:00:00`);
    // 周一起始：getDay() 0=周日，转成周一=0
    const weekday = (date.getDay() + 6) % 7;
    if (weekday === 0 && currentWeek.length > 0) {
      weeks.push({ days: currentWeek, monthLabel: null });
      currentWeek = [];
    }
    currentWeek.push(day);
  }
  if (currentWeek.length > 0) {
    weeks.push({ days: currentWeek, monthLabel: null });
  }
  // 月份标记：某月 1 号落在哪一周，该周顶部显示月份
  return weeks.map((week) => {
    const firstOfMonth = week.days.find((d) => d.date.endsWith("-01"));
    return { days: week.days, monthLabel: firstOfMonth ? firstOfMonth.date.slice(0, 7) : null };
  });
});

// 年历 grid 列模板：周数均分，minmax(0,1fr) 保证列宽不超容器
const calendarGridStyle = computed(() => {
  const count = Math.max(calendarWeeks.value.length, 1);
  return { gridTemplateColumns: `repeat(${count}, minmax(0, 1fr))` };
});

// 展平为 grid 定位单元：col=周序号(1 起)，row=星期几(1=周一..7=周日)
const flatCalendarDays = computed(() => {
  const cells: { day: UsageTrailWallDay; col: number; row: number }[] = [];
  calendarWeeks.value.forEach((week, colIndex) => {
    for (const day of week.days) {
      const date = new Date(`${day.date}T00:00:00`);
      const row = (date.getDay() + 6) % 7 + 1;
      cells.push({ day, col: colIndex + 1, row });
    }
  });
  return cells;
});

async function loadTrail() {
  loading.value = true;
  errorText.value = "";
  try {
    const [today, history] = await Promise.all([
      invokeTauri<UsageTrailWallView>("get_usage_trail", {
        input: { view: "today" as TrailView },
      }),
      invokeTauri<UsageTrailWallView>("get_usage_trail", {
        input: {
          view: "history" as TrailView,
          year: historyData.value?.year,
        },
      }),
    ]);
    if (!trailUnmounted) {
      todayData.value = today;
      historyData.value = history;
      // 拉齐所有年份的历史日历，供成就卡计算（累计/峰值/连续日）
      void loadAllYears(today.years);
    }
  } catch (error) {
    if (!trailUnmounted) {
      errorText.value = t("config.welcome.trail.loadFailed", { err: toErrorMessage(error) });
    }
  } finally {
    if (!trailUnmounted) {
      loading.value = false;
    }
  }
}

// 全量历史年份数据缓存（成就卡数据源）
const allYearsData = ref<UsageTrailWallView[]>([]);

async function loadAllYears(years: string[]) {
  if (years.length === 0) return;
  try {
    const views = await Promise.all(
      years.map((year) =>
        invokeTauri<UsageTrailWallView>("get_usage_trail", {
          input: { view: "history" as TrailView, year },
        }),
      ),
    );
    if (!trailUnmounted) {
      allYearsData.value = views;
    }
  } catch {
    // 成就数据降级：至少保留已加载的默认年份
  }
}

function todayDateString(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

function shiftDate(date: string, offset: number): string {
  const parsed = new Date(`${date}T00:00:00`);
  parsed.setDate(parsed.getDate() + offset);
  const month = String(parsed.getMonth() + 1).padStart(2, "0");
  const day = String(parsed.getDate()).padStart(2, "0");
  return `${parsed.getFullYear()}-${month}-${day}`;
}

// ============ 成就卡 ============

// 全量日序列：所有年份日历合并（同日期取最大值，今天取 today 视图实时值）
const allDayTotals = computed<Map<string, number>>(() => {
  const map = new Map<string, number>();
  for (const view of allYearsData.value) {
    for (const day of view.calendar || []) {
      map.set(day.date, Math.max(map.get(day.date) ?? 0, day.totalTokens));
    }
  }
  return map;
});

// 累计 token：迁移前旧账本（epoch）+ 所有年份总量
const achievementTotalTokens = computed(() => {
  const epoch = todayData.value?.epochTotals?.totalTokens || 0;
  let sum = epoch;
  for (const view of allYearsData.value) {
    sum += view.totals.totalTokens;
  }
  return sum;
});

// 单日峰值：全量日历中最大的单日 token（含日期）
const achievementPeak = computed(() => {
  let peakTokens = 0;
  let peakDate = "";
  for (const [date, tokens] of allDayTotals.value) {
    if (tokens > peakTokens) {
      peakTokens = tokens;
      peakDate = date;
    }
  }
  return { tokens: peakTokens, date: peakDate || "" };
});

// 当前连续日：以今天为锚，今天 0 不算断（从昨天起算），往前数连续有 token 的天数
const achievementCurrentStreak = computed(() => {
  const map = allDayTotals.value;
  const today = todayDateString();
  let cursor = map.get(today) && (map.get(today) ?? 0) > 0 ? today : shiftDate(today, -1);
  let streak = 0;
  while (cursor) {
    const tokens = map.get(cursor) ?? 0;
    if (tokens <= 0) break;
    streak += 1;
    cursor = shiftDate(cursor, -1);
  }
  return streak;
});

// 最高连续日：全量历史中最长的连续有 token 天数
const achievementBestStreak = computed(() => {
  const dates = [...allDayTotals.value.keys()].sort();
  let best = 0;
  let current = 0;
  let prev: string | null = null;
  for (const date of dates) {
    const tokens = allDayTotals.value.get(date) ?? 0;
    if (tokens > 0) {
      current = prev && shiftDate(prev, 1) === date ? current + 1 : 1;
      best = Math.max(best, current);
    } else {
      current = 0;
    }
    prev = date;
  }
  return best;
});

async function switchYear(year: string) {
  if (!historyData.value || historyData.value.year === year) return;
  historyData.value = { ...historyData.value, year };
  await loadTrail();
}

function formatTokens(value: number): string {
  const numeric = Number(value || 0);
  if (!Number.isFinite(numeric)) return "0";
  const abs = Math.abs(numeric);
  const units = [
    { threshold: 1_000_000_000_000, suffix: "T" },
    { threshold: 1_000_000_000, suffix: "B" },
    { threshold: 1_000_000, suffix: "M" },
    { threshold: 1_000, suffix: "K" },
  ];
  for (const unit of units) {
    if (abs >= unit.threshold) {
      const scaled = numeric / unit.threshold;
      const digits = Math.abs(scaled) >= 100 ? 0 : Math.abs(scaled) >= 10 ? 1 : 2;
      return `${scaled.toFixed(digits).replace(/\.0+$|(\.\d*[1-9])0+$/, "$1")}${unit.suffix}`;
    }
  }
  return new Intl.NumberFormat("zh-CN").format(Math.round(numeric));
}

function calendarCellStyle(day: UsageTrailWallDay): Record<string, string> {
  // 平滑对数色阶：0 无数据为灰底，>0 起从 25% 连续缓升，1B 达 100% 纯主题色
  const pct = calendarPct(day.totalTokens);
  if (pct === null) {
    return { backgroundColor: "var(--color-base-200)" };
  }
  return { backgroundColor: `color-mix(in oklab, var(--color-primary) ${pct}%, var(--color-base-100))` };
}

function legendStyle(pct: number): Record<string, string> {
  // 图例色块与格子同公式渲染，避免 CSS 渐变偏色
  return { backgroundColor: `color-mix(in oklab, var(--color-primary) ${pct}%, var(--color-base-100))` };
}

onMounted(() => {
  void loadTrail();
});

onBeforeUnmount(() => {
  trailUnmounted = true;
});
</script>
