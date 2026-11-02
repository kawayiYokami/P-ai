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
      <section v-if="currentRun" class="rounded-box border border-base-300/60 bg-base-200/40 p-3 text-xs">
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
              <AlertCircle class="h-3.5 w-3.5" />
              <span>失败</span>
            </span>
            <span
              v-else
              class="inline-flex items-center gap-1.5 font-medium text-success"
            >
              <CheckCircle2 class="h-3.5 w-3.5" />
              <span>已完成</span>
            </span>

            <span class="text-base-content/20">|</span>
            <span class="tabular-nums text-base-content/75">{{ formatFullTime(currentRun.startedAt) }}</span>
            <span v-if="!isRunning && currentRun.updatedAt" class="text-base-content/45 tabular-nums">— {{ formatFullTime(currentRun.updatedAt) }}</span>
          </div>

          <!-- 耗时、步数与模型 -->
          <div class="flex flex-wrap items-center gap-2 text-base-content/60">
            <span class="inline-flex items-center gap-1 tabular-nums">
              <Clock class="h-3.5 w-3.5 opacity-60" />
              <span>{{ formatElapsed(currentRun.elapsedMs) }}</span>
            </span>
            <span class="text-base-content/20">·</span>
            <span class="tabular-nums">{{ currentRun.requestCount }} 步</span>
            <template v-if="runModelName">
              <span class="text-base-content/20">·</span>
              <span class="inline-flex items-center gap-1 font-mono text-[11px] text-base-content/70 max-w-[200px] truncate" :title="runModelName">
                <Cpu class="h-3.5 w-3.5 opacity-60 shrink-0" />
                <span class="truncate">{{ runModelName }}</span>
              </span>
            </template>
          </div>
        </div>
      </section>

      <!-- 2. 思考与工具活动流 -->
      <section v-if="activityItems.length > 0" class="space-y-2 min-w-0 w-full">
        <div class="text-[11px] font-medium text-base-content/50 px-0.5">执行过程</div>
        <div class="space-y-2">
          <div
            v-for="item in activityItems"
            :key="item.id"
            class="rounded-box border border-base-300/60 bg-base-100 overflow-hidden text-xs transition-colors"
          >
            <!-- 思考过程节点 -->
            <template v-if="item.kind === 'reasoning'">
              <button
                type="button"
                class="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-base-200/50 transition-colors select-none"
                @click="toggleItemExpanded(item.id)"
              >
                <!-- AI 闪耀图标 -->
                <Sparkles class="h-4 w-4 shrink-0 text-primary" />
                <span class="min-w-0 flex-1 truncate font-medium text-base-content/85">
                  <InlineMarkdownText :text="getReasoningSummary(item.text)" />
                </span>
                <span class="shrink-0 tabular-nums text-[11px] text-base-content/40">
                  思考 {{ item.text.length }} 字
                </span>
                <ChevronDown
                  class="h-3.5 w-3.5 shrink-0 text-base-content/40 transition-transform duration-200"
                  :class="isItemExpanded(item.id) ? 'rotate-180' : ''"
                />
              </button>
              <div
                v-if="isItemExpanded(item.id)"
                class="border-t border-base-300/40 bg-base-200/30 px-3.5 py-3"
              >
                <div class="border-l-2 border-primary/30 pl-3 leading-relaxed text-base-content/80 text-xs">
                  <AppMarkdownRenderer :text="item.text" :is-dark="markdownIsDark" />
                </div>
              </div>
            </template>

            <!-- 工具调用节点 -->
            <template v-else-if="item.kind === 'tool'">
              <button
                type="button"
                class="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-base-200/50 transition-colors select-none"
                @click="toggleItemExpanded(item.id)"
              >
                <span v-if="item.status === 'doing'" class="loading loading-spinner loading-xs text-primary shrink-0"></span>
                <AlertCircle v-else-if="isToolFailed(item)" class="h-4 w-4 shrink-0 text-error" />
                <Wrench v-else class="h-4 w-4 shrink-0 text-base-content/60" />

                <!-- 语义化标题 -->
                <div class="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
                  <span class="font-medium text-base-content shrink-0">{{ getToolSemantic(item).action }}</span>
                  <span class="truncate font-mono text-[11px] text-base-content/70">{{ getToolSemantic(item).target }}</span>
                  <span v-if="getToolSemantic(item).lineRange" class="font-mono text-[11px] opacity-60 shrink-0">{{ getToolSemantic(item).lineRange }}</span>
                  <span v-if="getToolSemantic(item).extra" class="text-[11px] opacity-60 shrink-0 truncate">{{ getToolSemantic(item).extra }}</span>
                  <span v-if="getToolSemantic(item).adds" class="font-mono text-[11px] text-success shrink-0">+{{ getToolSemantic(item).adds }}</span>
                  <span v-if="getToolSemantic(item).removes" class="font-mono text-[11px] text-error shrink-0">-{{ getToolSemantic(item).removes }}</span>
                </div>

                <span v-if="isToolFailed(item)" class="shrink-0 text-[11px] text-error font-medium">失败</span>
                <span v-else-if="item.status === 'done'" class="shrink-0 text-[11px] text-success/80">完成</span>

                <ChevronDown
                  class="h-3.5 w-3.5 shrink-0 text-base-content/40 transition-transform duration-200"
                  :class="isItemExpanded(item.id) ? 'rotate-180' : ''"
                />
              </button>

              <div
                v-if="isItemExpanded(item.id)"
                class="border-t border-base-300/40 bg-base-200/30 px-3.5 py-3 space-y-2.5"
              >
                <!-- 参数 -->
                <div v-if="item.argsText?.trim()">
                  <div class="mb-1 text-[11px] font-medium text-base-content/50">参数</div>
                  <pre class="m-0 max-h-60 overflow-y-auto whitespace-pre-wrap break-all rounded bg-base-100 p-2.5 font-mono text-[11px] leading-relaxed text-base-content/80 border border-base-300/40">{{ formatToolArgs(item.argsText) }}</pre>
                </div>

                <!-- 结果 -->
                <div v-if="item.resultText?.trim()">
                  <div class="mb-1 text-[11px] font-medium text-base-content/50">返回结果</div>
                  <pre
                    class="m-0 max-h-60 overflow-y-auto whitespace-pre-wrap break-all rounded p-2.5 font-mono text-[11px] leading-relaxed border"
                    :class="isToolFailed(item) ? 'bg-error/5 text-error border-error/20' : 'bg-base-100 text-base-content/80 border-base-300/40'"
                  >{{ formatToolResult(item.resultText) }}</pre>
                </div>
                <div v-else-if="item.status === 'doing'" class="text-[11px] text-base-content/40 py-1">
                  正在执行中...
                </div>
              </div>
            </template>
          </div>
        </div>
      </section>

      <!-- 3. 最终答复区域 -->
      <section v-if="finalText" class="rounded-box border border-base-300/60 bg-base-100 p-4 min-w-0 w-full">
        <div class="flex items-center justify-between mb-3 border-b border-base-300/40 pb-2">
          <div class="flex items-center gap-1.5 text-xs font-semibold text-base-content/85">
            <CheckCircle2 class="h-4 w-4 text-success" />
            <span>最终答复</span>
          </div>
          <button
            type="button"
            class="btn btn-ghost btn-xs gap-1 text-base-content/60 hover:text-base-content"
            @click="copyFinalText"
          >
            <Check v-if="copied" class="h-3 w-3 text-success" />
            <Copy v-else class="h-3 w-3" />
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
import { computed, ref, watch, onMounted, onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertCircle,
  Check,
  CheckCircle2,
  ChevronDown,
  Clock,
  Copy,
  Cpu,
  Sparkles,
  Wrench,
} from "@lucide/vue";
import { invokeTauri } from "../../../services/tauri-api";
import { isDarkAppTheme, useAppTheme } from "../../shell/composables/use-app-theme";
import type { ArchiveBlockPage, ChatActivityItem, ScheduleEvent, ScheduleRun } from "../../../types/app";
import { AppMarkdownRenderer, initKatex } from "../markdown";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import { createToolCallPresentation } from "../utils/tool-call-presentation";
import { projectChatActivityForDisplay } from "../../../utils/chat-message-semantics";
import { sliceNaturalSentencePrefix } from "../utils/text-slicing";
import { parseToolCallResultStatus } from "../utils/toolcall-preview";

initKatex();

const props = defineProps<{
  conversationId: string;
  autoRefreshKey?: string;
  initialRuns?: ScheduleRun[];
  initialBlockPage?: ArchiveBlockPage;
}>();

const { t } = useI18n();
const { currentTheme } = useAppTheme();
const markdownIsDark = computed(() => isDarkAppTheme(String(currentTheme.value || "")));

const { toolCallSemanticPresentation } = createToolCallPresentation({
  t: (key, params) => String(t(key, params ?? {})),
  agentName: (agentId) => agentId,
});

const runs = ref<ScheduleRun[]>([]);
const activityItems = ref<ChatActivityItem[]>([]);
const finalText = ref("");
const loading = ref(false);
const errorText = ref("");
const expandedIds = ref<Set<string>>(new Set());
const copied = ref(false);

let fetchToken = 0;
let pollTimer: ReturnType<typeof window.setInterval> | null = null;
let copyResetTimer: ReturnType<typeof window.setTimeout> | null = null;

const currentRun = computed(() => runs.value[0] || null);

const isRunning = computed(() => currentRun.value?.status === "running");
const isError = computed(() => currentRun.value?.status === "error");
const runModelName = computed(() => currentRun.value?.model || currentRun.value?.lastModelName || "");

function isItemExpanded(id: string) {
  return expandedIds.value.has(id);
}

function toggleItemExpanded(id: string) {
  const next = new Set(expandedIds.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  expandedIds.value = next;
}

function getReasoningSummary(text: string): string {
  const parts = sliceNaturalSentencePrefix(text, { targetLength: 40 });
  return parts.summary || "思考过程";
}

function getToolSemantic(item: ChatActivityItem) {
  if (item.kind !== "tool") {
    return { action: "工具", target: "", text: "" };
  }
  return toolCallSemanticPresentation(item);
}

function isToolFailed(item: ChatActivityItem): boolean {
  if (item.kind !== "tool") return false;
  if (!item.resultText) return false;
  const status = parseToolCallResultStatus(item.resultText);
  return status.isFailed || status.isDenied;
}

function formatToolArgs(raw: string): string {
  const trimmed = String(raw || "").trim();
  if (!trimmed) return "";
  try {
    const parsed = JSON.parse(trimmed);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return trimmed;
  }
}

function formatToolResult(raw: string): string {
  const trimmed = String(raw || "").trim();
  if (!trimmed) return "";
  try {
    const parsed = JSON.parse(trimmed);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return trimmed;
  }
}

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

function formatFullTime(value?: string | null) {
  const raw = String(value || "").trim();
  if (!raw) return "--";
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return raw;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
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
