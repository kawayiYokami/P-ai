<template>
  <div class="min-w-0 w-full">
    <div v-if="errorText" class="rounded-lg border border-error/20 bg-error/5 px-3 py-2 text-xs text-error">
      {{ errorText }}
    </div>
    <div v-else-if="!loading && runs.length === 0" class="min-w-0 w-full">
      <div v-if="finalLoading" class="flex items-center justify-center gap-2 py-8 text-xs text-base-content/40">
        <span class="loading loading-spinner loading-xs"></span>加载中
      </div>
      <div v-else-if="finalText" class="min-w-0 w-full">
        <div class="mb-2 text-xs font-medium text-base-content/45">最终正文</div>
        <div class="assistant-markdown text-sm leading-6 text-base-content/80">
          <AppMarkdownRenderer :text="finalText" :is-dark="markdownIsDark" />
        </div>
        <div v-if="finalErrorText" class="mt-2 text-xs text-error/70">{{ finalErrorText }}</div>
      </div>
      <div v-else class="py-8 text-center text-xs text-base-content/40">
        <div>暂无调度记录</div>
        <div v-if="finalErrorText" class="mt-2 text-error/60">{{ finalErrorText }}</div>
        <div v-else class="mt-1 text-base-content/30">暂无正文</div>
      </div>
    </div>
    <div v-else class="space-y-6 w-full min-w-0">
      <section
        v-for="run in runs"
        :key="run.runId"
        class="min-w-0 w-full"
      >
        <div class="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-xs">
          <span class="font-medium tabular-nums text-base-content/80">{{ formatFullTime(run.startedAt) }}</span>
          <span v-if="run.status !== 'running'" class="tabular-nums text-base-content/50">— {{ formatFullTime(run.updatedAt) }}</span>
          <span v-else class="text-base-content/30">— 进行中</span>
          <span class="text-base-content/20">·</span>
          <span class="tabular-nums text-base-content/50">{{ formatElapsed(run.elapsedMs) }}</span>
          <span class="text-base-content/20">·</span>
          <span class="tabular-nums text-base-content/50">{{ run.requestCount }}步</span>
          <span v-if="run.status === 'running'" class="ml-1 inline-flex items-center gap-1 text-warning">
            <span class="size-1.5 animate-pulse rounded-full bg-warning"></span>进行中
          </span>
          <span v-else-if="run.status === 'error'" class="text-error">失败</span>
        </div>

        <ul class="timeline timeline-vertical timeline-compact mt-3 w-full">
          <li v-for="(event, index) in run.events" :key="event.id" class="min-w-0">
            <hr v-if="index !== 0" />
            <div class="timeline-middle">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 20 20"
                fill="currentColor"
                class="h-5 w-5 shrink-0"
                :class="iconColorClass(event.success)"
              >
                <path
                  fill-rule="evenodd"
                  d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 10-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z"
                  clip-rule="evenodd"
                />
              </svg>
            </div>
            <div class="timeline-end timeline-box min-w-0 w-full">
              <button
                type="button"
                class="flex w-full min-w-0 items-center gap-1.5 text-left"
                @click="toggleCollapsed(event.id)"
              >
                <span class="shrink-0 text-xs font-medium text-base-content/80">{{ phaseLabel(event.phase) }}</span>
                <span v-if="titleInlineSummary(event)" class="min-w-0 flex-1 truncate text-xs text-base-content/55">{{ titleInlineSummary(event) }}</span>
                <span v-else class="flex-1"></span>
                <span class="shrink-0 text-caption tabular-nums text-base-content/40">+{{ formatElapsedShort(event.elapsedMs) }}</span>
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5 shrink-0 text-base-content/25 transition-transform duration-200" :class="isCollapsed(event.id) ? '' : 'rotate-90'"><path fill-rule="evenodd" d="M7.21 14.78a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd" /></svg>
              </button>
              <Transition
                :css="false"
                @enter="animateEnter"
                @leave="animateLeave"
                @enter-cancelled="cleanupAnimation"
                @leave-cancelled="cleanupAnimation"
              >
                <div
                  v-if="!isCollapsed(event.id)"
                  class="mt-2 min-w-0 w-full space-y-1.5 border-t border-base-300 pt-2"
                >
                  <div v-if="eventSummaryLine(event)" class="whitespace-normal break-words text-xs leading-5 text-base-content/55">
                    {{ eventSummaryLine(event) }}
                  </div>
                  <div v-if="eventBodyText(event)" class="whitespace-pre-wrap break-words break-all text-xs leading-5 text-base-content/70">
                    {{ eventBodyText(event) }}
                  </div>
                  <div v-if="eventErrorText(event)" class="whitespace-pre-wrap break-words break-all text-caption leading-4 text-error/80">
                    {{ eventErrorText(event) }}
                  </div>
                  <div v-if="!eventSummaryLine(event) && !eventBodyText(event) && !eventErrorText(event)" class="text-xs text-base-content/30">—</div>
                </div>
              </Transition>
            </div>
            <hr v-if="index !== run.events.length - 1" />
          </li>
        </ul>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import { isDarkAppTheme, useAppTheme } from "../../shell/composables/use-app-theme";
import type { ArchiveBlockPage, ChatMessage, ScheduleRun } from "../../../types/app";
import { useCollapseTransition } from "../composables/use-collapse-transition";
import { AppMarkdownRenderer, initKatex } from "../markdown";

initKatex();

const props = defineProps<{
  conversationId: string;
  autoRefreshKey?: string;
}>();

const runs = ref<ScheduleRun[]>([]);
const loading = ref(false);
const errorText = ref("");
const finalText = ref("");
const finalLoading = ref(false);
const finalErrorText = ref("");
let finalFetchToken = 0;
let fetchRunsToken = 0;
const collapsedIds = ref<Set<string>>(new Set());
let previousAllIds = new Set<string>();
let previousLastIds = new Set<string>();
let pollTimer: ReturnType<typeof window.setInterval> | null = null;

const { currentTheme } = useAppTheme();
const { animateEnter, animateLeave, cleanupAnimation } = useCollapseTransition();
const markdownIsDark = computed(() => isDarkAppTheme(String(currentTheme.value || "")));

function isCollapsed(id: string) {
  return collapsedIds.value.has(id);
}

function toggleCollapsed(id: string) {
  const next = new Set(collapsedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  collapsedIds.value = next;
}

function syncCollapsedState() {
  const currentAll = new Set<string>();
  const currentLast = new Set<string>();
  for (const run of runs.value) {
    for (const event of run.events) currentAll.add(event.id);
    const lastId = run.events[run.events.length - 1]?.id;
    if (lastId) currentLast.add(lastId);
  }
  if (currentAll.size === 0) {
    collapsedIds.value = new Set();
    previousAllIds = currentAll;
    previousLastIds = currentLast;
    return;
  }
  const next = new Set(collapsedIds.value);
  const isFirstLoad = previousAllIds.size === 0 && collapsedIds.value.size === 0;
  if (isFirstLoad) {
    for (const id of currentAll) next.add(id);
    collapsedIds.value = next;
    previousAllIds = currentAll;
    previousLastIds = currentLast;
    return;
  }
  for (const id of currentAll) {
    const isNew = !previousAllIds.has(id);
    if (isNew) next.add(id);
  }
  for (const id of Array.from(next)) {
    if (!currentAll.has(id)) next.delete(id);
  }
  collapsedIds.value = next;
  previousAllIds = currentAll;
  previousLastIds = currentLast;
}

async function fetchRuns(silent = false) {
  const conversationId = props.conversationId.trim();
  const token = ++fetchRunsToken;
  // Invalidate any in-flight final text fetch when a newer run fetch starts
  finalFetchToken++;
  if (!conversationId) {
    runs.value = [];
    previousAllIds = new Set();
    previousLastIds = new Set();
    collapsedIds.value = new Set();
    finalText.value = "";
    finalErrorText.value = "";
    finalLoading.value = false;
    if (token === fetchRunsToken) {
      loading.value = false;
      errorText.value = "";
    }
    return;
  }
  if (!silent) loading.value = true;
  if (token === fetchRunsToken) errorText.value = "";
  let data: ScheduleRun[] | null = null;
  let fetchError: unknown = null;
  try {
    const result = await invokeTauri<ScheduleRun[]>("list_schedule_runs", { conversationId });
    data = Array.isArray(result) ? result : [];
  } catch (error) {
    fetchError = error;
  }
  if (token !== fetchRunsToken || conversationId !== props.conversationId.trim()) return;
  if (fetchError) {
    if (!silent) errorText.value = String(fetchError);
    if (!silent) loading.value = false;
    finalText.value = "";
    finalErrorText.value = "";
    finalLoading.value = false;
    return;
  }
  runs.value = data ?? [];
  syncCollapsedState();
  if (!silent) loading.value = false;
  if (!loading.value && runs.value.length === 0 && !errorText.value) {
    void fetchFinalText(conversationId);
  } else {
    finalText.value = "";
    finalErrorText.value = "";
    finalLoading.value = false;
  }
}

async function fetchFinalText(conversationId: string) {
  const capturedConversationId = conversationId.trim();
  const token = ++finalFetchToken;
  finalLoading.value = true;
  finalErrorText.value = "";
  finalText.value = "";
  try {
    const page = await invokeTauri<ArchiveBlockPage>("delegate.blockPage", { conversationId: capturedConversationId }, 10000);
    if (token !== finalFetchToken || capturedConversationId !== props.conversationId.trim()) return;
    const messages = Array.isArray(page?.messages) ? page.messages : [];
    const raw = findLastAssistantText(messages);
    finalText.value = formatDelegateResultText(raw);
    if (!finalText.value) finalErrorText.value = "";
  } catch (error) {
    if (token !== finalFetchToken || capturedConversationId !== props.conversationId.trim()) return;
    finalErrorText.value = String(error);
  } finally {
    if (token === finalFetchToken && capturedConversationId === props.conversationId.trim()) finalLoading.value = false;
  }
}

function findLastAssistantText(messages: ChatMessage[]) {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message?.role !== "assistant") continue;
    const text = message.parts
      ?.filter((part) => part.type === "text")
      .map((part) => part.text)
      .join("\n")
      .trim();
    if (text) return text;
  }
  return "";
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

function hasRunning() {
  return runs.value.some((run) => run.status === "running");
}

function syncPolling() {
  if (pollTimer != null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
  if (hasRunning()) {
    pollTimer = window.setInterval(() => { void fetchRuns(true); }, 1500);
  }
}

watch(
  () => [props.conversationId, props.autoRefreshKey] as const,
  () => { void fetchRuns(false); },
);

watch(
  () => runs.value.map((r) => `${r.runId}:${r.status}:${r.events.length}`).join("|"),
  () => syncPolling(),
);

onMounted(() => { void fetchRuns(false); });
onBeforeUnmount(() => {
  if (pollTimer != null) window.clearInterval(pollTimer);
});

function phaseLabel(phase: string) {
  if (phase === "dispatch_start") return "开始";
  if (phase === "dispatch_end") return "结束";
  if (phase === "model_round_start") return "请求";
  if (phase === "model_round_end") return "响应";
  if (phase === "tool_call") return "调用";
  if (phase === "tool_result") return "结果";
  if (phase === "compaction_start") return "整理";
  if (phase === "compaction_end") return "整理完成";
  return phase;
}

function iconColorClass(success?: boolean | null) {
  if (success === false) return "text-error";
  if (success === true) return "text-success";
  return "text-primary";
}

function eventErrorText(event: { detail: any }) {
  const raw = (event.detail as any)?.error;
  if (!raw) return "";
  return String(raw).trim();
}

function normalizeDetail(detail: any) {
  return (detail ?? {}) as Record<string, any>;
}

function titleInlineSummary(event: { phase: string; detail: any }) {
  const d = normalizeDetail(event.detail);
  if (event.phase === "model_round_start") {
    if (d.modelName) return String(d.modelName);
    if (d.attempt) return `第${d.attempt}次`;
    return "";
  }
  if (event.phase === "model_round_end") {
    if (d.modelName) return String(d.modelName);
    return "";
  }
  if (event.phase === "tool_call" || event.phase === "tool_result") {
    if (d.toolName) return String(d.toolName);
    return "";
  }
  if (event.phase === "dispatch_start") {
    return d.traceId ? String(d.traceId).slice(0, 8) : "";
  }
  if (event.phase === "dispatch_end") {
    if (d.assistantTextLength != null) return `正文${d.assistantTextLength}字`;
    return "";
  }
  if (event.phase === "compaction_start" || event.phase === "compaction_end") {
    if (d.reason) return String(d.reason);
    return "";
  }
  return "";
}

function eventSummaryLine(event: { phase: string; detail: any }) {
  const d = normalizeDetail(event.detail);
  if (event.phase === "model_round_start") {
    const parts: string[] = [];
    if (d.modelName) parts.push(String(d.modelName));
    if (d.attempt) parts.push(`第${d.attempt}次`);
    if (d.providerName) parts.push(String(d.providerName));
    return parts.join(" · ");
  }
  if (event.phase === "model_round_end") {
    const parts: string[] = [];
    if (d.modelName) parts.push(String(d.modelName));
    if (d.elapsedMs != null) parts.push(`${d.elapsedMs}ms`);
    if (d.assistantTextLength != null) parts.push(`正文${d.assistantTextLength}字`);
    if (d.reasoningLength != null && Number(d.reasoningLength) > 0) parts.push(`思考${d.reasoningLength}字`);
    if (d.toolCallCount != null) parts.push(d.toolCallCount > 0 ? `${d.toolCallCount}工具` : "无工具");
    if (d.hasError) parts.push("异常");
    return parts.join(" · ");
  }
  if (event.phase === "tool_call") {
    const parts: string[] = [];
    if (d.toolName) parts.push(String(d.toolName));
    if (d.argLength != null) parts.push(`${d.argLength}字`);
    return parts.join(" · ");
  }
  if (event.phase === "tool_result") {
    const parts: string[] = [];
    if (d.toolName) parts.push(String(d.toolName));
    if (d.isError) parts.push("失败");
    else if (d.isError === false) parts.push("成功");
    if (d.textLength != null) parts.push(`${d.textLength}字`);
    return parts.join(" · ");
  }
  if (event.phase === "compaction_start" || event.phase === "compaction_end") {
    const parts: string[] = [];
    if (d.reason) parts.push(String(d.reason));
    if (d.elapsedMs != null) parts.push(`${d.elapsedMs}ms`);
    return parts.join(" · ");
  }
  if (event.phase === "dispatch_start") return d.traceId ? String(d.traceId) : "";
  if (event.phase === "dispatch_end") {
    if (d.assistantTextLength != null) return `正文${d.assistantTextLength}字`;
    return "";
  }
  return "";
}

function eventBodyText(event: { phase: string; detail: any }) {
  const d = normalizeDetail(event.detail);
  if (event.phase === "dispatch_start") {
    if (d.textPreview) return String(d.textPreview);
    return "";
  }
  if (event.phase === "model_round_end") {
    const chunks: string[] = [];
    if (d.reasoningPreview) chunks.push(String(d.reasoningPreview).trim());
    if (d.textPreview) chunks.push(String(d.textPreview).trim());
    return chunks.join("\n\n");
  }
  if (event.phase === "tool_call") {
    if (d.argPreview) return String(d.argPreview);
    return "";
  }
  if (event.phase === "tool_result") {
    if (d.textPreview) return String(d.textPreview);
    return "";
  }
  if (event.phase === "dispatch_end") {
    if (d.textPreview) return String(d.textPreview);
    return "";
  }
  return "";
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

function formatElapsedShort(value: number) {
  if (!Number.isFinite(value) || value < 0) return "0s";
  if (value < 1000) return `${value}ms`;
  return `${(value / 1000).toFixed(1)}s`;
}

function formatFullTime(value: string) {
  const raw = String(value || "").trim();
  if (!raw) return "--";
  const d = new Date(raw);
  if (Number.isNaN(d.getTime())) return raw;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

defineExpose({ refresh: () => fetchRuns(false) });
</script>
