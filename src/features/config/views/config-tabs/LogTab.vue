<template>
  <div class="grid gap-3">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <div class="flex flex-col gap-3">
          <div class="flex items-center justify-between">
            <div class="text-sm font-medium">调用日志（内存）</div>
            <div class="join">
              <button class="btn btn-sm bg-base-200 join-item" @click="props.openRuntimeLogs">
                后台日志
              </button>
              <button class="btn btn-sm bg-base-200 join-item" :disabled="loading" @click="reload">
                刷新
              </button>
              <button
                class="btn btn-sm bg-base-200 join-item"
                :disabled="loading || logs.length === 0"
                @click="clearAll"
              >
                清空
              </button>
            </div>
          </div>
          <div class="text-sm opacity-60">
            仅保留最近 10 次调度日志；单次调度内的多轮 chat 会聚合到所属 pipeline 中，进程退出后自动清空。
          </div>
        </div>
      </div>
    </div>

    <div v-if="loading" class="text-sm opacity-70">加载中...</div>
    <div v-else-if="logs.length === 0" class="text-sm opacity-50">暂无日志</div>

    <div v-else class="space-y-4">
      <div v-if="pipelineLogs.length" class="space-y-3">
        <div class="text-sm font-medium opacity-80">调度日志</div>
        <div
          v-for="entry in pipelineLogs"
          :key="entry.id"
          class="card bg-base-100 border-2 border-primary/20 shadow-sm"
        >
          <div class="card-body p-4 space-y-4">
            <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div class="space-y-2">
                <div class="flex items-center gap-2 flex-wrap">
                  <div class="badge badge-primary badge-outline">本次调度</div>
                  <div class="text-sm font-medium break-all">
                    {{ entry.createdAt }} | {{ entry.provider }} | {{ entry.requestFormat }} | {{ entry.model }}
                  </div>
                </div>
                <div class="text-xs opacity-60 break-all">{{ entry.baseUrl || "-" }}</div>
                <div v-if="entry.traceId" class="text-xs opacity-60 break-all">
                  trace: {{ entry.traceId }}
                </div>
              </div>
              <div class="badge" :class="entry.success ? 'badge-success' : 'badge-error'">
                {{ entry.success ? "成功" : "失败" }}
              </div>
            </div>

            <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
              <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
                <div class="text-xs opacity-60">总耗时</div>
                <div class="text-sm font-medium">{{ entry.elapsedMs }}ms</div>
              </div>
              <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
                <div class="text-xs opacity-60">模型轮次</div>
                <div class="text-sm font-medium">{{ entry.roundCount ?? (entry.rounds?.length ?? 0) }}</div>
              </div>
              <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
                <div class="text-xs opacity-60">工具调用</div>
                <div class="text-sm font-medium">{{ entry.toolCallCount ?? totalToolCallsForRounds(entry.rounds) }}</div>
              </div>
              <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
                <div class="text-xs opacity-60">失败摘要</div>
                <div class="text-sm font-medium break-all">{{ entry.error?.trim() || "-" }}</div>
              </div>
            </div>

            <details
              v-if="entry.timeline?.length"
              class="collapse collapse-arrow bg-base-200 border border-base-300"
            >
              <summary class="collapse-title text-sm py-2 min-h-0">
                调度 Timeline（{{ entry.timeline.length }} 阶段）
              </summary>
              <div class="collapse-content text-sm space-y-2">
                <div class="opacity-70 break-all">
                  慢阶段：
                  {{ topSlowStages(entry).map((item) => `${item.stage} +${item.sincePrevMs}ms`).join(" | ") || "-" }}
                </div>
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.timeline) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">调度请求</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.request) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">调度响应</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.response ?? null) }}</pre>
              </div>
            </details>

            <div class="rounded-box border border-base-300 bg-base-200/60 p-3 space-y-3">
              <div class="flex items-center justify-between gap-2">
                <div class="text-sm font-medium">轮次（{{ entry.rounds?.length ?? 0 }}）</div>
                <div class="text-xs opacity-60">
                  点击某一轮查看 Request / Response / Tools / Headers / Error
                </div>
              </div>
              <div v-if="entry.rounds?.length" class="space-y-2">
                <button
                  v-for="(round, index) in entry.rounds"
                  :key="round.id"
                  class="w-full rounded-box border border-base-300 bg-base-100 px-3 py-2 text-left transition hover:border-primary/40"
                  @click="openRound(entry, round, index)"
                >
                  <div class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
                    <div class="space-y-1">
                      <div class="text-sm font-medium">
                        第 {{ index + 1 }} 轮 | {{ round.provider }} | {{ round.model }}
                      </div>
                      <div class="text-xs opacity-60 break-all">{{ round.baseUrl || "-" }}</div>
                    </div>
                    <div class="flex items-center gap-2 flex-wrap">
                      <div class="badge badge-sm badge-outline">{{ round.elapsedMs }}ms</div>
                      <div class="badge badge-sm badge-outline">
                        工具 {{ toolCallCountForEntry(round) }}
                      </div>
                      <div class="badge badge-sm" :class="round.success ? 'badge-success' : 'badge-error'">
                        {{ round.success ? "成功" : "失败" }}
                      </div>
                    </div>
                  </div>
                </button>
              </div>
              <div v-else class="text-sm opacity-60">当前调度没有可展开的轮次明细。</div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="otherLogs.length" class="space-y-2">
        <div class="text-sm font-medium opacity-80">其他请求</div>
        <div
          v-for="entry in otherLogs"
          :key="entry.id"
          class="card bg-base-100 border border-base-300"
        >
          <div class="card-body p-3 space-y-2">
            <div class="flex items-center justify-between gap-2">
              <div class="text-sm opacity-70 break-all">
                {{ entry.createdAt }} | {{ entry.scene }} | {{ entry.provider }} | {{ entry.requestFormat }} | {{ entry.model }}
              </div>
              <div class="badge badge-sm" :class="entry.success ? 'badge-success' : 'badge-error'">
                {{ entry.success ? "成功" : "失败" }}
              </div>
            </div>

            <div class="text-sm opacity-70">耗时 {{ entry.elapsedMs }}ms | {{ entry.baseUrl || "-" }}</div>
            <div v-if="entry.traceId" class="text-xs opacity-60 break-all">trace: {{ entry.traceId }}</div>

            <details
              v-if="entry.timeline?.length"
              class="collapse collapse-arrow bg-base-200 border border-base-300"
            >
              <summary class="collapse-title text-sm py-2 min-h-0">
                Timeline（{{ entry.timeline.length }} 阶段）
              </summary>
              <div class="collapse-content text-sm space-y-2">
                <div class="opacity-70 break-all">
                  慢阶段：
                  {{ topSlowStages(entry).map((item) => `${item.stage} +${item.sincePrevMs}ms`).join(" | ") || "-" }}
                </div>
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.timeline) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">Headers</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.headers) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">Tools</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.tools ?? null) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">Request</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.request) }}</pre>
              </div>
            </details>

            <details class="collapse collapse-arrow bg-base-200 border border-base-300">
              <summary class="collapse-title text-sm py-2 min-h-0">Response</summary>
              <div class="collapse-content text-sm">
                <pre class="whitespace-pre-wrap break-all">{{ toPretty(entry.response ?? null) }}</pre>
              </div>
            </details>

            <div v-if="entry.error" class="text-sm text-error break-all">{{ entry.error }}</div>
          </div>
        </div>
      </div>
    </div>

    <dialog class="modal" :class="{ 'modal-open': !!selectedRound }">
      <div class="modal-box max-w-5xl space-y-4">
        <div class="flex items-start justify-between gap-3">
          <div class="space-y-1">
            <div class="text-lg font-semibold">
              {{ selectedRound ? `第 ${selectedRound.index + 1} 轮模型调用` : "轮次详情" }}
            </div>
            <div v-if="selectedRound" class="text-sm opacity-70 break-all">
              {{ selectedRound.round.provider }} | {{ selectedRound.round.requestFormat }} | {{ selectedRound.round.model }}
            </div>
            <div v-if="selectedRound?.round.traceId" class="text-xs opacity-60 break-all">
              trace: {{ selectedRound.round.traceId }}
            </div>
          </div>
          <button class="btn btn-sm btn-ghost" @click="closeRound">关闭</button>
        </div>

        <div v-if="selectedRound" class="grid gap-2 sm:grid-cols-3">
          <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
            <div class="text-xs opacity-60">本轮耗时</div>
            <div class="text-sm font-medium">{{ selectedRound.round.elapsedMs }}ms</div>
          </div>
          <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
            <div class="text-xs opacity-60">工具调用</div>
            <div class="text-sm font-medium">{{ toolCallCountForEntry(selectedRound.round) }}</div>
          </div>
          <div class="rounded-box border border-base-300 bg-base-200/70 px-3 py-2">
            <div class="text-xs opacity-60">状态</div>
            <div class="text-sm font-medium">{{ selectedRound.round.success ? "成功" : "失败" }}</div>
          </div>
        </div>

        <div class="tabs tabs-boxed bg-base-200 inline-flex">
          <button
            v-for="tab in roundDetailTabs"
            :key="tab.id"
            class="tab"
            :class="{ 'tab-active': activeRoundTab === tab.id }"
            @click="activeRoundTab = tab.id"
          >
            {{ tab.label }}
          </button>
        </div>

        <div v-if="selectedRound" class="rounded-box border border-base-300 bg-base-200/60 p-3">
          <pre
            v-if="activeRoundTab !== 'error'"
            class="whitespace-pre-wrap break-all text-sm"
          >{{ roundTabContent(selectedRound.round, activeRoundTab) }}</pre>
          <div v-else class="text-sm break-all" :class="selectedRound.round.error ? 'text-error' : 'opacity-60'">
            {{ selectedRound.round.error?.trim() || "无错误" }}
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop" @submit.prevent="closeRound">
        <button @click="closeRound">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { invokeTauri } from "../../../../services/tauri-api";
import type { LlmRoundLogEntry } from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";

const props = defineProps<{
  openRuntimeLogs: () => void;
}>();

const loading = ref(false);
const logs = ref<LlmRoundLogEntry[]>([]);
const selectedRound = ref<{
  pipeline: LlmRoundLogEntry;
  round: LlmRoundLogEntry;
  index: number;
} | null>(null);
const roundDetailTabs = [
  { id: "request", label: "Request" },
  { id: "response", label: "Response" },
  { id: "tools", label: "Tools" },
  { id: "headers", label: "Headers" },
  { id: "error", label: "Error" },
] as const;
const activeRoundTab = ref<(typeof roundDetailTabs)[number]["id"]>("request");

const pipelineLogs = computed(() =>
  logs.value.filter((entry) => entry.scene === "chat_pipeline"),
);

const otherLogs = computed(() =>
  logs.value.filter((entry) => entry.scene !== "chat_pipeline"),
);

function toPretty(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input ?? "");
  }
}

function topSlowStages(entry: LlmRoundLogEntry) {
  return [...(entry.timeline ?? [])]
    .sort((a, b) => b.sincePrevMs - a.sincePrevMs)
    .slice(0, 3);
}

function toolCallCountForEntry(entry: LlmRoundLogEntry): number {
  const response = entry.response as { toolHistoryEvents?: Array<{ tool_calls?: unknown[] }> } | null | undefined;
  return (response?.toolHistoryEvents ?? []).reduce((total, item) => {
    return total + (Array.isArray(item?.tool_calls) ? item.tool_calls.length : 0);
  }, 0);
}

function totalToolCallsForRounds(rounds?: LlmRoundLogEntry[]): number {
  return (rounds ?? []).reduce((total, round) => total + toolCallCountForEntry(round), 0);
}

function openRound(pipeline: LlmRoundLogEntry, round: LlmRoundLogEntry, index: number) {
  selectedRound.value = { pipeline, round, index };
  activeRoundTab.value = "request";
}

function closeRound() {
  selectedRound.value = null;
}

function roundTabContent(
  entry: LlmRoundLogEntry,
  tab: (typeof roundDetailTabs)[number]["id"],
): string {
  if (tab === "request") {
    return toPretty(entry.request);
  }
  if (tab === "response") {
    return toPretty(entry.response ?? null);
  }
  if (tab === "tools") {
    return toPretty(entry.tools ?? null);
  }
  return toPretty(entry.headers);
}

async function reload() {
  loading.value = true;
  try {
    const list = await invokeTauri<LlmRoundLogEntry[]>("list_recent_llm_round_logs");
    logs.value = [...list].reverse();
  } catch (error) {
    logs.value = [
      {
        id: "error",
        createdAt: new Date().toISOString(),
        scene: "ui",
        requestFormat: "-",
        provider: "-",
        model: "-",
        baseUrl: "",
        headers: [],
        request: null,
        tools: null,
        response: null,
        error: toErrorMessage(error),
        elapsedMs: 0,
        success: false,
      },
    ];
  } finally {
    loading.value = false;
  }
}

async function clearAll() {
  loading.value = true;
  try {
    await invokeTauri<boolean>("clear_recent_llm_round_logs");
    logs.value = [];
    selectedRound.value = null;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  void reload();
});
</script>
