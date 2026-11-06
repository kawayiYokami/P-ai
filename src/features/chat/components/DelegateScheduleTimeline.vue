<template>
  <div class="min-w-0 w-full space-y-4">
    <!-- 错误提示 -->
    <div v-if="errorText" class="rounded-box border border-error/20 bg-error/5 px-3.5 py-2.5 text-xs text-error">
      {{ errorText }}
    </div>

    <!-- 加载中 -->
    <div v-else-if="loading && !currentRun && activityItems.length === 0" class="flex items-center justify-center gap-2 py-12 text-xs text-base-content/40">
      <span class="loading loading-spinner loading-xs text-primary"></span>
      <span>加载中...</span>
    </div>

    <!-- 空状态：无记录且无正文 -->
    <div v-else-if="!currentRun && activityItems.length === 0 && !finalText" class="py-12 text-center text-xs text-base-content/40">
      <div>暂无调度记录与内容</div>
    </div>

    <!-- 内容展示 -->
    <div v-else class="space-y-4 min-w-0 w-full">
      <!-- 1. 顶部概要信息 -->
      <section v-if="currentRun" class="rounded-box border border-base-300/60 bg-base-100 p-3 text-xs">
        <div class="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
          <!-- 状态与时间段 -->
          <div class="flex flex-wrap items-center gap-2 min-w-0">
            <span
              v-if="isRunning"
              class="inline-flex items-center gap-1.5 font-medium text-warning"
            >
              <span class="size-2 animate-pulse rounded-full bg-warning"></span>
              <span>进行中</span>
            </span>
            <span
              v-else-if="isError"
              class="inline-flex items-center gap-1.5 font-medium text-error"
            >
              <AppIcon name="alert-circle" class="h-3.5 w-3.5" />
              <span>失败</span>
            </span>
            <span
              v-else
              class="inline-flex items-center gap-1.5 font-medium text-success"
            >
              <AppIcon name="check-circle" class="h-3.5 w-3.5" />
              <span>已完成</span>
            </span>

            <span class="text-base-content/20">|</span>
            <span class="tabular-nums text-base-content/75">{{ formatRunRange(displayStartedAt, isRunning ? "" : displayUpdatedAt) }}</span>
          </div>

          <!-- 耗时、步数与模型 -->
          <div class="flex flex-wrap items-center gap-2 text-base-content/60">
            <span class="inline-flex items-center gap-1 tabular-nums">
              <AppIcon name="clock" class="h-3.5 w-3.5 opacity-60" />
              <span>{{ formatElapsed(displayElapsedMs) }}</span>
            </span>
            <span class="text-base-content/20">·</span>
            <span class="tabular-nums">{{ displayRequestCount }} 步</span>
            <template v-if="runModelName">
              <span class="text-base-content/20">·</span>
              <span class="inline-flex items-center gap-1 font-mono text-[11px] text-base-content/70 max-w-[200px] truncate" :title="runModelName">
                <AppIcon name="cpu" class="h-3.5 w-3.5 opacity-60 shrink-0" />
                <span class="truncate">{{ runModelName }}</span>
              </span>
            </template>
          </div>
        </div>
      </section>

      <!-- 2. 思考与工具活动流：与消息内共用同一套渲染 -->
      <section v-if="activityItems.length > 0" class="space-y-2 min-w-0 w-full">
        <div class="text-[11px] font-medium text-base-content/50 px-0.5">执行过程</div>
        <ActivityTimeline :items="activityItems" :dark="markdownIsDark" :state="timelineState" />
      </section>

      <!-- 3. 最终答复区域 -->
      <section v-if="finalText" class="rounded-box border border-base-300/60 bg-base-100 p-4 min-w-0 w-full">
        <div class="flex items-center justify-between mb-3 border-b border-base-300/40 pb-2">
          <div class="flex items-center gap-1.5 text-xs font-semibold text-base-content/85">
            <AppIcon name="check-circle" class="h-4 w-4 text-success" />
            <span>最终答复</span>
          </div>
          <button
            type="button"
            class="btn btn-ghost btn-xs gap-1 text-base-content/60 hover:text-base-content"
            @click="copyFinalText"
          >
            <AppIcon name="check" v-if="copied" class="h-3 w-3 text-success" />
            <AppIcon name="copy" v-else class="h-3 w-3" />
            <span>{{ copied ? '已复制' : '复制' }}</span>
          </button>
        </div>
        <div class="assistant-markdown text-sm leading-6 text-base-content/90">
          <AppMarkdownRenderer :text="finalText" :is-dark="markdownIsDark" />
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import { invokeTauri } from "../../../services/tauri-api";
import { isDarkAppTheme, useAppTheme } from "../../shell/composables/use-app-theme";
import type { ArchiveBlockPage, ChatActivityItem, ConversationDelegateStatusSummary, ScheduleEvent, ScheduleRun } from "../../../types/app";
import { AppMarkdownRenderer, initKatex } from "../markdown";
import { projectChatActivityForDisplay } from "../../../utils/chat-message-semantics";
import ActivityTimeline from "./ActivityTimeline.vue";

initKatex();

const props = defineProps<{
  conversationId: string;
  autoRefreshKey?: string;
  initialRuns?: ScheduleRun[];
  initialBlockPage?: ArchiveBlockPage;
  /**
   * 委托列表那行的状态汇总。重启后内存事件已清，后端按持久化消息时间戳倒推出来的
   * Run 起止时间会相同、耗时为 0；这份汇总是准的，用来兜住那几项展示值。
   */
  statusSnapshot?: ConversationDelegateStatusSummary | null;
}>();

const { t } = useI18n();
const { currentTheme } = useAppTheme();
const markdownIsDark = computed(() => isDarkAppTheme(String(currentTheme.value || "")));

const runs = ref<ScheduleRun[]>([]);
const activityItems = ref<ChatActivityItem[]>([]);
// 时间线的条目级状态由这一层持有，刷新数据重建组件时不会丢展开态
const timelineState = reactive({
  expandedOverrides: {} as Record<string, boolean>,
  toolResultOverrides: {} as Record<string, string>,
  toolResultLoading: {} as Record<string, boolean>,
  toolResultErrors: {} as Record<string, string>,
});
const finalText = ref("");
const loading = ref(false);
const errorText = ref("");
const copied = ref(false);

let fetchToken = 0;
let pollTimer: ReturnType<typeof window.setInterval> | null = null;
let copyResetTimer: ReturnType<typeof window.setTimeout> | null = null;

const currentRun = computed(() => runs.value[0] || null);

/**
 * 后端冷路径合成的 Run（应用重启、内存事件已清）起止时间取自同一批持久化消息，
 * 两条相同就把耗时算成 0。列表那行的状态汇总里是准的，这种情况下改用它。
 */
const runIsPersistedSynthesis = computed(() => String(currentRun.value?.runId || "").startsWith("persisted-"));
const displaySnapshot = computed(() => (runIsPersistedSynthesis.value ? props.statusSnapshot || null : null));
const displayStartedAt = computed(() => displaySnapshot.value?.startedAt || currentRun.value?.startedAt || "");
const displayUpdatedAt = computed(() => displaySnapshot.value?.updatedAt || currentRun.value?.updatedAt || "");
const displayElapsedMs = computed(() => {
  const snapshotElapsed = displaySnapshot.value?.elapsedMs;
  if (Number.isFinite(snapshotElapsed) && (snapshotElapsed as number) > 0) return snapshotElapsed as number;
  return currentRun.value?.elapsedMs ?? 0;
});
const displayRequestCount = computed(() => {
  const snapshotCount = displaySnapshot.value?.requestCount;
  if (Number.isFinite(snapshotCount) && (snapshotCount as number) > 0) return snapshotCount as number;
  return currentRun.value?.requestCount ?? 0;
});

const isRunning = computed(() => currentRun.value?.status === "running");
const isError = computed(() => currentRun.value?.status === "error");
const runModelName = computed(() => currentRun.value?.model || currentRun.value?.lastModelName || "");

function formatDelegateResultText(text: string) {
  const trimmed = text.trim();
  if (!trimmed) return "";
  try {
    const parsed = JSON.parse(trimmed);
    return `\`\`\`json\n${JSON.stringify(parsed, null, 2)}\n\`\`\``;
  } catch {
    return text;
  }
}

function formatElapsed(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0s";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

function pad2(n: number) {
  return String(n).padStart(2, "0");
}

function parseTimeValue(value?: string | null): Date | null {
  const raw = String(value || "").trim();
  if (!raw) return null;
  const parsed = new Date(raw);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatMonthDayTime(d: Date, withYear: boolean) {
  const md = `${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  return `${withYear ? `${d.getFullYear()}-` : ""}${md} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

/**
 * 起止时间合成一段：同一天结束只给时分，跨天才重复日期；年份只在非当年或跨年时出现。
 * 右边的耗时已经说明跨度，这里只负责回答「什么时候跑的」。
 */
function formatRunRange(startValue?: string | null, endValue?: string | null) {
  const start = parseTimeValue(startValue);
  if (!start) return "--";
  const end = parseTimeValue(endValue);
  const currentYear = new Date().getFullYear();
  const crossedYear = !!end && start.getFullYear() !== end.getFullYear();
  const head = formatMonthDayTime(start, crossedYear || start.getFullYear() !== currentYear);
  if (!end) return head;
  const sameDay = start.getFullYear() === end.getFullYear()
    && start.getMonth() === end.getMonth()
    && start.getDate() === end.getDate();
  if (sameDay) return `${head} → ${pad2(end.getHours())}:${pad2(end.getMinutes())}`;
  return `${head} → ${formatMonthDayTime(end, crossedYear || end.getFullYear() !== currentYear)}`;
}

async function copyFinalText() {
  if (!finalText.value) return;
  try {
    await navigator.clipboard.writeText(finalText.value);
    copied.value = true;
    if (copyResetTimer) clearTimeout(copyResetTimer);
    copyResetTimer = setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch {
    // 忽略写入剪贴板异常
  }
}

/**
 * 当委托会话尚在流式或刚派发、数据库消息未落盘时，从 ScheduleEvent[] 提取 fallback 活动项
 */
function buildActivityFromRunEvents(events: ScheduleEvent[]): ChatActivityItem[] {
  const items: ChatActivityItem[] = [];
  const toolCallPending = new Map<string, { id: string; name: string; argsText: string }>();

  for (let idx = 0; idx < events.length; idx++) {
    const ev = events[idx];
    const detail = (ev.detail ?? {}) as Record<string, any>;

    if (ev.phase === "model_round_end") {
      const reasoning = String(detail.reasoningPreview || "").trim();
      if (reasoning) {
        items.push({
          kind: "reasoning",
          id: `run-reasoning-${ev.id || idx}`,
          text: reasoning,
        });
      }
    } else if (ev.phase === "tool_call") {
      const name = String(detail.toolName || "tool");
      const argsText = String(detail.argPreview || "");
      toolCallPending.set(name, {
        id: `run-tool-${ev.id || idx}`,
        name,
        argsText,
      });
    } else if (ev.phase === "tool_result") {
      const name = String(detail.toolName || "tool");
      const pending = toolCallPending.get(name);
      const isError = detail.isError === true;
      const rawText = String(detail.textPreview || "");
      const resultText = isError ? (rawText.startsWith("失败") || rawText.startsWith("错误") ? rawText : `[失败] ${rawText}`) : rawText;
      if (pending) {
        items.push({
          kind: "tool",
          id: pending.id,
          name: pending.name,
          argsText: pending.argsText,
          resultText,
          status: "done",
        });
        toolCallPending.delete(name);
      } else {
        items.push({
          kind: "tool",
          id: `run-tool-res-${ev.id || idx}`,
          name,
          argsText: "",
          resultText,
          status: "done",
        });
      }
    }
  }

  for (const pending of toolCallPending.values()) {
    items.push({
      kind: "tool",
      id: pending.id,
      name: pending.name,
      argsText: pending.argsText,
      status: "doing",
    });
  }

  return items;
}

function applyData(runsResult: ScheduleRun[], blockPageResult: ArchiveBlockPage | null) {
  runs.value = Array.isArray(runsResult) ? runsResult : [];

  const messages = Array.isArray(blockPageResult?.messages) ? blockPageResult.messages : [];
  const extractedActivities: ChatActivityItem[] = [];
  let lastAssistantText = "";

  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    if (msg.role !== "assistant") continue;

    const text = (msg.parts || [])
      .filter((part) => part.type === "text")
      .map((part) => part.text)
      .join("\n")
      .trim();
    if (text) {
      lastAssistantText = text;
    }

    const proj = projectChatActivityForDisplay(msg);
    for (let j = 0; j < proj.items.length; j++) {
      const it = proj.items[j];
      if (it.kind === "content" && it.text.trim() === lastAssistantText) {
        continue;
      }
      extractedActivities.push({
        ...it,
        id: it.id || `msg-${i}-act-${j}`,
      });
    }
  }

  if (extractedActivities.length === 0 && runs.value.length > 0 && runs.value[0].events.length > 0) {
    activityItems.value = buildActivityFromRunEvents(runs.value[0].events);
  } else {
    activityItems.value = extractedActivities;
  }

  finalText.value = formatDelegateResultText(lastAssistantText);
}

if (props.initialRuns || props.initialBlockPage) {
  applyData(props.initialRuns || [], props.initialBlockPage || null);
}

async function fetchData(silent = false) {
  const conversationId = props.conversationId.trim();
  const token = ++fetchToken;
  if (!conversationId) {
    runs.value = [];
    activityItems.value = [];
    finalText.value = "";
    errorText.value = "";
    loading.value = false;
    return;
  }

  if (!silent) loading.value = true;
  if (token === fetchToken) errorText.value = "";

  try {
    const [runsResult, blockPageResult] = await Promise.all([
      invokeTauri<ScheduleRun[]>("list_schedule_runs", { conversationId }).catch((err) => {
        console.warn("[DelegateScheduleTimeline] list_schedule_runs 失败:", err);
        return [] as ScheduleRun[];
      }),
      invokeTauri<ArchiveBlockPage>("delegate.blockPage", { input: { conversationId } }, 10000).catch((err) => {
        console.warn("[DelegateScheduleTimeline] delegate.blockPage 失败:", err);
        return null;
      }),
    ]);

    if (token !== fetchToken || conversationId !== props.conversationId.trim()) return;

    applyData(runsResult, blockPageResult);
  } catch (err) {
    if (token !== fetchToken || conversationId !== props.conversationId.trim()) return;
    if (!silent) errorText.value = String(err);
  } finally {
    if (token === fetchToken && conversationId === props.conversationId.trim()) {
      loading.value = false;
    }
  }
}

function syncPolling() {
  if (pollTimer != null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
  if (isRunning.value) {
    pollTimer = window.setInterval(() => {
      void fetchData(true);
    }, 1500);
  }
}

watch(
  () => [props.conversationId, props.autoRefreshKey] as const,
  () => { void fetchData(false); },
);

watch(
  () => isRunning.value,
  () => syncPolling(),
);

onMounted(() => {
  void fetchData(false);
});

onBeforeUnmount(() => {
  if (pollTimer != null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
  if (copyResetTimer != null) {
    window.clearTimeout(copyResetTimer);
    copyResetTimer = null;
  }
});

defineExpose({ refresh: () => fetchData(false) });
</script>
