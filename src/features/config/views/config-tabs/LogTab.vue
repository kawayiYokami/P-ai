<template>
  <div class="grid gap-3">
    <ConfigTemplate :model-value="templateValues" :groups="templateGroups">
    <template #row-log-panel>
      <div class="grid min-w-0 gap-4">
        <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="text-sm">{{ t("config.logs.cacheSize") }}</div>
            <div class="mt-1 text-xs text-base-content/60">{{ t("config.logs.capacityHint", { count: props.config.llmRoundLogCapacity }) }}</div>
          </div>
          <div class="join shrink-0">
            <button
              v-for="option in logCapacityOptions"
              :key="option"
              class="btn btn-sm min-h-9 w-16 join-item"
              :class="props.config.llmRoundLogCapacity === option ? 'btn-primary' : 'bg-base-200'"
              type="button"
              :disabled="capacitySaving"
              @click="setLogCapacity(option)"
            >
              {{ t("config.logs.times", { count: option }) }}
            </button>
          </div>
        </div>

        <div class="flex min-w-0 flex-wrap items-center gap-2 border-t border-base-200 pt-4">
          <button :class="actionButtonClass" @click="props.openRuntimeLogs">
            {{ t("config.logs.backendLogs") }}
          </button>
          <button :class="actionButtonClass" :disabled="loading" @click="reload">
            {{ t("config.logs.refreshPipelineLogs") }}
          </button>
          <button
            :class="actionButtonClass"
            :disabled="loading || logs.length === 0"
            @click="clearAll"
          >
            {{ t("config.logs.clearPipelineLogs") }}
          </button>
        </div>

        <div class="grid grid-cols-4 gap-2 border-t border-base-200 pt-4">
          <button class="btn btn-sm w-full bg-base-200" @click="props.openConversationList">
            {{ t("config.chatSettings.openConversationList") }}
          </button>
          <button class="btn btn-sm w-full bg-base-200" @click="props.openPromptPreview">
            {{ t("config.chatSettings.previewRequest") }}
          </button>
          <button class="btn btn-sm w-full bg-base-200" @click="props.openSystemPromptPreview">
            {{ t("config.chatSettings.previewSystemPrompt") }}
          </button>
          <button class="btn btn-sm w-full bg-base-200" @click="openBuiltinToolCatalog">
            {{ t("config.logs.builtinToolCatalog") }}
          </button>
        </div>
      </div>
    </template>
  </ConfigTemplate>

  <div v-if="loading" class="text-sm opacity-70">{{ t("common.loading") }}</div>
    <div v-else-if="logs.length === 0" class="text-sm opacity-50">{{ t("config.logs.noLogs") }}</div>

    <div v-else class="space-y-4">
      <section v-if="pipelineLogs.length" class="space-y-3">
        <article
          v-for="entry in pipelineLogs"
          :key="entry.id"
          class="overflow-hidden rounded-box bg-base-100 p-4"
        >
          <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div class="min-w-0 space-y-2">
              <div class="flex flex-wrap items-center gap-2">
                <div class="badge badge-primary badge-outline">{{ t("config.logs.currentPipeline") }}</div>
                <div class="text-sm font-semibold break-all">
                  {{ entry.provider }} · {{ entry.model }}
                </div>
              </div>
              <div class="text-xs opacity-70 break-all">
                {{ formatLocalTime(entry.createdAt) }} · {{ entry.requestFormat }} · {{ entry.baseUrl || "-" }}
              </div>
              <div v-if="entry.traceId" class="text-xs opacity-50 break-all">
                trace: {{ entry.traceId }}
              </div>
            </div>
            <div class="badge" :class="entry.success ? 'badge-success' : 'badge-error'">
              {{ entry.success ? t("common.success") : t("common.failed") }}
            </div>
          </div>

          <div class="stats stats-vertical sm:stats-horizontal w-full bg-base-200 shadow-sm mt-4">
            <div
              v-for="metric in pipelineMetricCards(entry)"
              :key="metric.key"
              class="stat place-items-center sm:place-items-start px-4 py-3 text-center sm:text-left"
            >
              <div class="stat-title text-xs opacity-70">{{ metric.label }}</div>
              <div class="stat-value text-lg font-semibold leading-tight break-all">{{ metric.value }}</div>
            </div>
          </div>

          <div v-if="usageMetricCards(pipelineUsage(entry)).length" class="stats stats-vertical sm:stats-horizontal w-full bg-base-200 shadow-sm mt-3">
            <div
              v-for="metric in usageMetricCards(pipelineUsage(entry))"
              :key="metric.key"
              class="stat place-items-center sm:place-items-start px-3 py-2 text-center sm:text-left"
            >
              <div class="stat-title text-caption opacity-60">{{ metric.label }}</div>
              <div class="stat-value text-base font-semibold">{{ metric.value }}</div>
            </div>
          </div>
          <div v-else class="mt-3 rounded-box border border-dashed border-base-300 bg-base-100 px-4 py-3 text-sm opacity-60">
            {{ t("config.logs.noUsage") }}
          </div>

          <div v-if="entry.timeline?.length" class="mt-4">
            <details
              class="collapse collapse-arrow rounded-box border border-base-300 bg-base-200/70"
              open
            >
              <summary class="collapse-title min-h-0 py-3 text-sm font-medium">
                {{ t("config.logs.pipelineTimeline", { count: entry.timeline.length }) }}
              </summary>
              <div class="collapse-content">
                <TimelineList :items="timelineItems(entry)" />
              </div>
            </details>
          </div>

          <div class="mt-4 rounded-box border border-base-300 bg-base-200/60 p-3">
            <div class="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
              <div class="text-sm font-medium">{{ t("config.logs.roundsTitle", { count: entry.rounds?.length ?? 0 }) }}</div>
              <div class="text-xs opacity-60">{{ t("config.logs.roundsHint") }}</div>
            </div>
            <div v-if="entry.rounds?.length" class="mt-3 space-y-2">
              <button
                v-for="(round, index) in entry.rounds"
                :key="round.id"
                class="w-full rounded-box border border-base-300 bg-base-100 px-3 py-2 text-left transition hover:border-primary/50"
                @click="openRound(entry, round, index)"
              >
                <div class="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
                  <div class="min-w-0 space-y-1">
                    <div class="text-sm font-medium break-all">
                      {{ t("config.logs.roundTitle", { index: index + 1 }) }} · {{ round.model }}
                    </div>
                    <div class="text-xs opacity-60 break-all">
                      {{ formatLocalTime(round.createdAt) }} · {{ round.baseUrl || "-" }}
                    </div>
                  </div>
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="badge badge-sm badge-outline">{{ round.elapsedMs }}ms</span>
                    <span class="badge badge-sm badge-outline">{{ t("config.logs.toolCount", { count: toolCallCountForEntry(round) }) }}</span>
                    <span class="badge badge-sm" :class="round.success ? 'badge-success' : 'badge-error'">
                      {{ round.success ? t("common.success") : t("common.failed") }}
                    </span>
                  </div>
                </div>
              </button>
            </div>
            <div v-else class="mt-3 text-sm opacity-60">{{ t("config.logs.noRoundDetails") }}</div>
          </div>
        </article>
      </section>

      <section v-if="otherLogs.length" class="space-y-2">
        <div class="text-sm font-medium opacity-80">{{ t("config.logs.otherRequests") }}</div>
        <article
          v-for="entry in otherLogs"
          :key="entry.id"
          class="overflow-hidden rounded-box bg-base-100 p-4"
        >
          <div class="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
            <div class="min-w-0">
              <div class="text-sm font-medium break-all">{{ entry.scene }} · {{ entry.provider }} · {{ entry.model }}</div>
              <div class="mt-1 text-xs opacity-60 break-all">
                {{ formatLocalTime(entry.createdAt) }} · {{ entry.elapsedMs }}ms · {{ entry.baseUrl || "-" }}
              </div>
            </div>
            <div class="badge badge-sm" :class="entry.success ? 'badge-success' : 'badge-error'">
              {{ entry.success ? t("common.success") : t("common.failed") }}
            </div>
          </div>
          <div v-if="usageMetricCards(roundUsage(entry)).length" class="stats stats-vertical sm:stats-horizontal w-full bg-base-200 shadow-sm mt-3">
            <div
              v-for="metric in usageMetricCards(roundUsage(entry))"
              :key="metric.key"
              class="stat place-items-center sm:place-items-start px-3 py-2 text-center sm:text-left"
            >
              <div class="stat-title text-caption opacity-60">{{ metric.label }}</div>
              <div class="stat-value text-base font-semibold">{{ metric.value }}</div>
            </div>
          </div>
          <div v-else class="mt-3 rounded-box border border-dashed border-base-300 bg-base-100 px-4 py-3 text-sm opacity-60">
            {{ t("config.logs.noUsage") }}
          </div>
          <details
            v-if="entry.timeline?.length"
            class="collapse collapse-arrow mt-3 rounded-box border border-base-300 bg-base-200/70"
          >
            <summary class="collapse-title min-h-0 py-3 text-sm font-medium">
              {{ t("config.logs.timeline", { count: entry.timeline.length }) }}
            </summary>
            <div class="collapse-content">
              <TimelineList :items="timelineItems(entry)" />
            </div>
          </details>
          <div v-if="entry.error" class="mt-3 text-sm text-error break-all">{{ entry.error }}</div>
        </article>
      </section>
    </div>
  </div>

  <dialog ref="selectedRoundDialogRef" class="modal" @close="closeRound" @cancel.prevent="closeRound">
      <div class="modal-box max-w-5xl space-y-4">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0 space-y-1">
            <div class="text-lg font-semibold">
              {{ selectedRound ? t("config.logs.roundCallTitle", { index: selectedRound.index + 1 }) : t("config.logs.roundDetails") }}
            </div>
            <div v-if="activeRoundEntry" class="text-sm opacity-70 break-all">
              {{ activeRoundEntry.provider }} · {{ activeRoundEntry.requestFormat }} · {{ activeRoundEntry.model }}
            </div>
            <div v-if="activeRoundEntry" class="text-xs opacity-60">
              {{ formatLocalTime(activeRoundEntry.createdAt) }}
            </div>
            <div v-if="activeRoundEntry?.traceId" class="text-xs opacity-50 break-all">
              trace: {{ activeRoundEntry.traceId }}
            </div>
          </div>
          <button class="btn btn-sm btn-ghost" @click="closeRound">{{ t("common.close") }}</button>
        </div>

        <div v-if="activeRoundEntry" class="grid gap-2 sm:grid-cols-3">
          <MetricTile
            v-for="metric in roundHeaderMetricCards(activeRoundEntry)"
            :key="metric.key"
            :label="metric.label"
            :value="metric.value"
          />
        </div>

        <div class="tabs tabs-boxed inline-flex bg-base-200">
          <button
            v-for="tab in roundDetailTabs"
            :key="tab.id"
            class="tab"
            :class="{ 'tab-active': activeRoundTab === tab.id }"
            @click="setActiveRoundTab(tab.id)"
          >
            {{ tab.label }}
          </button>
        </div>

        <div
          v-if="activeSectionLoading"
          class="text-sm opacity-60"
        >
          {{ t("common.loading") }}
        </div>
        <div
          v-if="activeSectionError"
          class="text-sm text-error break-all"
        >
          {{ activeSectionError }}
        </div>

        <div v-if="activeRoundEntry" class="rounded-box border border-base-300 bg-base-200/60 p-3">
          <div v-if="activeRoundTab === 'answer'" class="space-y-3">
            <div class="grid gap-2 sm:grid-cols-3">
              <MetricTile
                v-for="metric in roundResponseMetricCards(activeRoundEntry)"
                :key="metric.key"
                :label="metric.label"
                :value="metric.value"
              />
            </div>
            <div class="rounded-box border border-base-300 bg-base-100 p-3">
              <div class="mb-2 text-sm font-medium">{{ t("config.logs.answerText") }}</div>
              <pre class="max-h-[36vh] overflow-auto whitespace-pre-wrap break-all text-xs">{{ answerPayload?.assistantText || "-" }}</pre>
            </div>
            <div class="rounded-box border border-base-300 bg-base-100 p-3">
              <div class="mb-2 text-sm font-medium">{{ t("config.logs.reasoningText") }}</div>
              <pre class="max-h-[36vh] overflow-auto whitespace-pre-wrap break-all text-xs">{{ answerPayload?.activityReasoningText || "-" }}</pre>
            </div>
          </div>

          <div v-else-if="activeRoundTab === 'usage'" class="space-y-3">
            <UsageGrid
              :metrics="usageMetricCards(sectionUsage(usagePayload) ?? roundUsage(activeRoundEntry))"
              :empty-text="t('config.logs.noUsage')"
            />
          </div>

          <div v-else-if="activeRoundTab === 'raw'" class="rounded-box border border-base-300 bg-base-100 p-3">
            <div class="mb-2 text-sm font-medium">{{ t("config.logs.rawResponse") }}</div>
            <pre class="max-h-[60vh] overflow-auto whitespace-pre-wrap break-all text-xs">{{ toPretty(rawResponsePayload ?? null) }}</pre>
          </div>

          <div v-else-if="activeRoundTab === 'tools'" class="space-y-4">
            <ToolNameSection
              :title="t('config.logs.availableTools')"
              :empty-text="t('config.logs.noTools')"
              :names="toolSectionAvailableNames(toolPayload, activeRoundEntry)"
            />
            <ToolNameSection
              :title="t('config.logs.calledTools')"
              :empty-text="t('config.logs.noCalledTools')"
              :names="toolSectionCalledNames(toolPayload, activeRoundEntry)"
            />
          </div>

          <div v-else-if="activeRoundTab === 'headers'" class="space-y-2">
            <div
              v-for="header in activeRoundEntry.headers"
              :key="`${header.name}:${header.value}`"
              class="flex flex-col gap-1 rounded-box bg-base-100 px-3 py-2 text-sm sm:flex-row sm:items-center"
            >
              <span class="font-medium">{{ header.name }}</span>
              <span class="break-all opacity-70">{{ header.value }}</span>
            </div>
            <div v-if="activeRoundEntry.headers.length === 0" class="text-sm opacity-60">{{ t("config.logs.noHeaders") }}</div>
          </div>

          <div v-else class="text-sm break-all" :class="activeRoundEntry.error ? 'text-error' : 'opacity-60'">
            {{ activeRoundEntry.error?.trim() || t("config.logs.noError") }}
          </div>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click.prevent="closeRound">close</button>
      </form>
    </dialog>

    <dialog ref="builtinToolCatalogDialog" class="modal" @close="builtinToolCatalogOpen = false">
      <div v-if="builtinToolCatalogOpen" class="modal-box max-w-2xl p-4">
        <h3 class="text-sm font-semibold">{{ t("config.logs.builtinToolCatalogTitle") }}</h3>
        <div v-if="builtinToolDefinitions.length" class="mt-2 divide-y divide-base-300/60">
          <div
            v-for="item in builtinToolDefinitions"
            :key="item.function.name"
            class="py-3"
          >
            <div class="min-w-0">
              <div class="font-medium">{{ item.function.name }}</div>
              <div class="text-xs opacity-60 whitespace-pre-wrap">{{ item.function.description || t("config.mcpToolList.noDescription") }}</div>
              <div v-if="toolParameterSummary(item.function.name).length" class="mt-1 flex flex-wrap gap-1">
                <span
                  v-for="paramText in toolParameterSummary(item.function.name)"
                  :key="`${item.function.name}-param-${paramText}`"
                  class="text-caption px-1.5 py-0.5 rounded bg-base-200 border border-base-300/70 opacity-80"
                >
                  {{ paramText }}
                </span>
              </div>
              <div v-if="toolParameterExamples(item.function.name).length" class="mt-1 grid gap-1">
                <pre
                  v-for="example in toolParameterExamples(item.function.name)"
                  :key="`${item.function.name}-example-${example}`"
                  class="text-caption leading-4 px-2 py-1 rounded bg-base-200 border border-base-300/70 opacity-90 whitespace-pre-wrap overflow-x-auto"
                >{{ example }}</pre>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="py-6 text-center text-sm opacity-50">{{ builtinToolCatalogLoading ? t("common.loading") : t("config.mcpToolList.empty") }}</div>
        <div class="modal-action mt-4">
          <button class="btn btn-sm" type="button" @click="closeBuiltinToolCatalog">{{ t("common.close") }}</button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button @click="closeBuiltinToolCatalog">close</button>
      </form>
    </dialog>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onMounted, ref, shallowRef, watch, type PropType } from "vue";
import PipelineScheduleTimeline from "../../components/PipelineScheduleTimeline.vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import type { AppConfig, FrontendToolDefinition, LlmRoundLogEntry, LlmRoundLogStage } from "../../../../types/app";
import { toErrorMessage } from "../../../../utils/error";

type Metric = {
  key: string;
  label: string;
  value: string;
};

type TimelineItem = {
  key: string;
  label: string;
  timeDisplay: string;
  delta: string;
  total: string;
};

type RoundDetailTabId = "answer" | "usage" | "raw" | "tools" | "headers" | "error";
type RoundLazySection = "answer" | "usage" | "raw_response" | "tools";

type UnknownRecord = Record<string, unknown>;

const MetricTile = defineComponent({
  name: "MetricTile",
  props: {
    label: { type: String, required: true },
    value: { type: String, required: true },
  },
  setup(props) {
    return () => h("div", { class: "rounded-box border border-base-300 bg-base-200/70 px-3 py-2" }, [
      h("div", { class: "text-xs opacity-60" }, props.label),
      h("div", { class: "mt-1 break-all text-sm font-medium" }, props.value),
    ]);
  },
});

const UsageGrid = defineComponent({
  name: "UsageGrid",
  props: {
    metrics: { type: Array as PropType<Metric[]>, required: true },
    emptyText: { type: String, required: true },
  },
  setup(props, { attrs }) {
    return () => h("div", attrs, [
      props.metrics.length > 0
        ? h("div", { class: "grid gap-2 sm:grid-cols-3 xl:grid-cols-6" }, props.metrics.map((metric) =>
          h("div", { key: metric.key, class: "rounded-box border border-base-300 bg-base-100 px-3 py-2" }, [
            h("div", { class: "text-xs opacity-60" }, metric.label),
            h("div", { class: "mt-1 text-sm font-semibold" }, metric.value),
          ]),
        ))
        : h("div", { class: "rounded-box border border-base-300 bg-base-100 px-3 py-2 text-sm opacity-60" }, props.emptyText),
    ]);
  },
});

const TimelineList = defineComponent({
  name: "TimelineList",
  props: {
    items: { type: Array as PropType<LlmRoundLogStage[]>, required: true },
  },
  setup(props) {
    return () => h(PipelineScheduleTimeline, { stages: props.items });
  },
});

const ToolNameSection = defineComponent({
  name: "ToolNameSection",
  props: {
    title: { type: String, required: true },
    emptyText: { type: String, required: true },
    names: { type: Array as PropType<string[]>, required: true },
  },
  setup(props) {
    return () => h("section", [
      h("div", { class: "text-sm font-medium" }, props.title),
      props.names.length > 0
        ? h("div", { class: "mt-2 flex flex-wrap gap-2" }, props.names.map((name) =>
          h("span", { key: name, class: "badge badge-outline badge-md max-w-full break-all" }, name),
        ))
        : h("div", { class: "mt-2 text-sm opacity-60" }, props.emptyText),
    ]);
  },
});

const props = defineProps<{
  config: AppConfig;
  openRuntimeLogs: () => void;
  openConversationList: () => void;
  openPromptPreview: () => void;
  openSystemPromptPreview: () => void;
  saveConfigAction?: () => Promise<boolean> | boolean;
}>();

const { t, locale } = useI18n();
const templateValues = {};
const templateGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "logs",
    title: t("config.logs.title"),
    rows: [{ key: "log-panel", items: [] }],
  },
]);
const loading = ref(false);
const logs = shallowRef<LlmRoundLogEntry[]>([]);
const logCapacityOptions = [1, 3, 10] as const;
const actionButtonClass = "btn btn-sm min-h-9 shrink-0 whitespace-nowrap bg-base-200 px-4";
const capacitySaving = ref(false);
let capacitySaveToken = 0;
const selectedRoundDialogRef = ref<HTMLDialogElement | null>(null);
const selectedRound = shallowRef<{
  pipeline: LlmRoundLogEntry;
  round: LlmRoundLogEntry;
  index: number;
} | null>(null);

function syncSelectedRoundDialog() {
  const d = selectedRoundDialogRef.value;
  if (!d) return;
  if (selectedRound.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(selectedRound, syncSelectedRoundDialog);
watch(selectedRoundDialogRef, syncSelectedRoundDialog);
const roundSectionPayloads = shallowRef<Partial<Record<RoundLazySection, unknown>>>({});
const roundSectionErrors = shallowRef<Partial<Record<RoundLazySection, string>>>({});
const roundSectionLoading = ref<RoundLazySection | null>(null);
const activeRoundTab = ref<RoundDetailTabId>("answer");

const roundDetailTabs = computed<Array<{ id: RoundDetailTabId; label: string }>>(() => [
  { id: "answer", label: t("config.logs.roundTabAnswer") },
  { id: "usage", label: t("config.logs.roundTabUsage") },
  { id: "tools", label: t("config.logs.roundTabTools") },
  { id: "headers", label: t("config.logs.roundTabHeaders") },
  { id: "error", label: t("config.logs.roundTabError") },
  { id: "raw", label: t("config.logs.roundTabRawResponse") },
]);

const pipelineLogs = computed(() =>
  logs.value.filter((entry) => entry.scene === "chat_pipeline"),
);

const otherLogs = computed(() =>
  logs.value.filter((entry) => entry.scene !== "chat_pipeline"),
);

const activeRoundEntry = computed(() => selectedRound.value?.round ?? null);

const answerPayload = computed(() => asRecord(roundSectionPayloads.value.answer));
const usagePayload = computed(() => asRecord(roundSectionPayloads.value.usage));
const rawResponsePayload = computed(() => roundSectionPayloads.value.raw_response);
const toolPayload = computed(() => asRecord(roundSectionPayloads.value.tools));

const activeLazySection = computed(() => lazySectionForTab(activeRoundTab.value));
const activeSectionLoading = computed(() =>
  !!activeLazySection.value && roundSectionLoading.value === activeLazySection.value,
);
const activeSectionError = computed(() =>
  activeLazySection.value ? roundSectionErrors.value[activeLazySection.value] || "" : "",
);

const stageLabelKeys: Record<string, string> = {
  "send_chat_message_inner.start": "config.logs.stages.chatStart",
  "runtime_and_session_ready": "config.logs.stages.runtimeReady",
  "run.begin": "config.logs.stages.runBegin",
  "attachments_processed": "config.logs.stages.attachmentsProcessed",
  "prepare_context.begin": "config.logs.stages.prepareBegin",
  "prepare_context.conversation_lock_wait_done": "config.logs.stages.conversationLockDone",
  "prepare_context.skill_snapshot_ready": "config.logs.stages.skillReady",
  "prepare_context.workspace_agents_ready": "config.logs.stages.workspaceReady",
  "prepare_context.todo_guide_ready": "config.logs.stages.todoGuideReady",
  "prepare_context.im_runtime_ready": "config.logs.stages.imReady",
  "prepare_context.task_board_ready": "config.logs.stages.taskBoardReady",
  "prepare_context.todo_board_ready": "config.logs.stages.todoBoardReady",
  "prepare_context.attachment_hints_ready": "config.logs.stages.attachmentHintsReady",
  "prepare_context.overrides_built": "config.logs.stages.overridesBuilt",
  "prepare_context.terminal_block_ready": "config.logs.stages.terminalBlockReady",
  "prepare_context.prompt_build_begin": "config.logs.stages.promptBuildBegin",
  "prepare_context.prompt_fixed_system_ready": "config.logs.stages.promptFixedSystemReady",
  "prepare_context.prompt_conversation_payload_ready": "config.logs.stages.promptConversationReady",
  "prepare_context.prompt_system_cache_hit": "config.logs.stages.promptCacheHit",
  "prepare_context.prompt_system_cache_rebuilt": "config.logs.stages.promptCacheRebuilt",
  "prepare_context.prompt_system_finalize_ready": "config.logs.stages.promptFinalizeReady",
  "prepare_context.prompt_built": "config.logs.stages.promptBuilt",
  "prepare_context.prompt_tokens_estimated": "config.logs.stages.promptTokensEstimated",
  "prepare_context.done": "config.logs.stages.prepareDone",
  "pre_send_archive_checked": "config.logs.stages.archiveChecked",
  "prompt_ready": "config.logs.stages.promptReady",
  "model_reply_ready": "config.logs.stages.modelReplyReady",
  "assistant_final_append.start": "config.logs.stages.finalAppendStart",
  "assistant_final_append.finish": "config.logs.stages.finalAppendFinish",
  "assistant_message_persist_scheduled": "config.logs.stages.persistScheduled",
  "send_chat_message_inner.finish": "config.logs.stages.chatFinish",
  "model_round_total": "config.logs.stages.modelRoundTotal",
  "dispatch_start": "config.logs.stages.dispatchStart",
  "dispatch_end": "config.logs.stages.dispatchEnd",
  "model_round_start": "config.logs.stages.modelRoundStart",
  "model_round_end": "config.logs.stages.modelRoundEnd",
  "tool_call": "config.logs.stages.toolCall",
  "tool_result": "config.logs.stages.toolResult",
  "compaction_start": "config.logs.stages.compactionStart",
  "compaction_end": "config.logs.stages.compactionEnd",
};

async function setLogCapacity(value: 1 | 3 | 10) {
  if (props.config.llmRoundLogCapacity === value) return;
  if (capacitySaving.value) return;
  const token = ++capacitySaveToken;
  const requested = value;
  const prev = props.config.llmRoundLogCapacity;
  props.config.llmRoundLogCapacity = value;
  capacitySaving.value = true;
  if (!props.saveConfigAction) {
    if (token === capacitySaveToken) capacitySaving.value = false;
    return;
  }
  try {
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      if (token === capacitySaveToken && props.config.llmRoundLogCapacity === requested) {
        props.config.llmRoundLogCapacity = prev;
      }
    }
  } catch {
    if (token === capacitySaveToken && props.config.llmRoundLogCapacity === requested) {
      props.config.llmRoundLogCapacity = prev;
    }
  } finally {
    if (token === capacitySaveToken) capacitySaving.value = false;
  }
}

function asRecord(value: unknown): UnknownRecord | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as UnknownRecord
    : null;
}

function responseRecord(entry: LlmRoundLogEntry | null | undefined): UnknownRecord | null {
  return asRecord(entry?.response);
}

function readNumber(record: UnknownRecord | null | undefined, keys: string[]): number | null {
  if (!record) return null;
  for (const key of keys) {
    const raw = record[key];
    const value = typeof raw === "number" ? raw : Number(raw);
    if (Number.isFinite(value) && value > 0) return Math.round(value);
  }
  return null;
}

function numberText(value: number | null | undefined): string {
  if (!value || value <= 0) return "-";
  return new Intl.NumberFormat(locale.value || undefined).format(value);
}

function toPretty(input: unknown): string {
  try {
    return JSON.stringify(input, null, 2);
  } catch {
    return String(input ?? "");
  }
}

function formatTimeDelta(ms: number): string {
  const rounded = Math.max(0, Math.round(ms));
  if (rounded >= 100) return `${(rounded / 1000).toFixed(1)}秒`;
  return `${rounded}ms`;
}

function msText(value: number | null | undefined): string {
  const ms = Math.max(0, Math.round(Number(value || 0)));
  if (ms >= 100) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

function formatLocalTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value || "-";
  return new Intl.DateTimeFormat(locale.value || undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date).replace(/\//g, "-");
}

function stageLabel(stage: string): string {
  if (stage.startsWith("model_request.start[")) return t("config.logs.stages.modelRequestStart");
  if (stage.startsWith("model_request.finish[")) return t("config.logs.stages.modelRequestFinish");
  const key = stageLabelKeys[stage];
  return key ? t(key) : stage;
}

function timelineItems(entry: LlmRoundLogEntry): LlmRoundLogStage[] {
  return entry.timeline ?? [];
}

function slowestStage(entry: LlmRoundLogEntry): LlmRoundLogStage | null {
  const timeline = entry.timeline ?? [];
  if (timeline.length === 0) return null;
  let best = timeline[0] as LlmRoundLogStage;
  for (let i = 1; i < timeline.length; i += 1) {
    const cur = timeline[i] as LlmRoundLogStage;
    if ((cur.sincePrevMs ?? 0) > (best.sincePrevMs ?? 0)) best = cur;
  }
  return best;
}

function pipelineMetricCards(entry: LlmRoundLogEntry): Metric[] {
  const slow = slowestStage(entry);
  return [
    { key: "elapsed", label: t("config.logs.totalElapsed"), value: msText(entry.elapsedMs) },
    { key: "rounds", label: t("config.logs.modelRounds"), value: numberText(entry.roundCount ?? entry.rounds?.length ?? 0) },
    { key: "tools", label: t("config.logs.toolCalls"), value: numberText(entry.toolCallCount ?? totalToolCallsForRounds(entry.rounds)) },
    { key: "slow", label: t("config.logs.slowestStage"), value: slow ? `${stageLabel(slow.stage)} ${msText(slow.sincePrevMs)}` : "-" },
  ];
}

function pipelineResponseMetricCards(entry: LlmRoundLogEntry): Metric[] {
  const response = responseRecord(entry);
  const usage = asRecord(response?.usage);
  return [
    {
      key: "context",
      label: t("config.logs.contextUsage"),
      value: readNumber(usage, ["contextUsagePercent"]) != null
        ? `${readNumber(usage, ["contextUsagePercent"])}%`
        : "-",
    },
    { key: "effective", label: t("config.logs.effectivePrompt"), value: numberText(readNumber(usage, ["effectivePromptTokens"])) },
    { key: "window", label: t("config.logs.contextWindow"), value: numberText(readNumber(usage, ["contextWindowTokens"])) },
    { key: "assistant", label: t("config.logs.assistantTextLength"), value: numberText(readNumber(response, ["assistantTextLength"])) },
  ];
}

function roundHeaderMetricCards(entry: LlmRoundLogEntry): Metric[] {
  return [
    { key: "elapsed", label: t("config.logs.roundElapsed"), value: msText(entry.elapsedMs) },
    { key: "tools", label: t("config.logs.toolCalls"), value: numberText(toolCallCountForEntry(entry)) },
    { key: "status", label: t("config.logs.status"), value: entry.success ? t("common.success") : t("common.failed") },
  ];
}

function roundResponseMetricCards(entry: LlmRoundLogEntry): Metric[] {
  const response = responseRecord(entry);
  const assistantLength = readNumber(response, ["assistantTextLength"]) ?? textLength(response?.assistantText);
  const reasoningLength = readNumber(response, [
    "reasoningContentLength",
    "activityReasoningTextLength",
    "reasoningTextLength",
  ]) ?? textLength(response?.reasoningContent ?? response?.activityReasoningText);
  return [
    { key: "assistant", label: t("config.logs.assistantTextLength"), value: numberText(assistantLength) },
    { key: "reasoning", label: t("config.logs.reasoningTextLength"), value: numberText(reasoningLength) },
    { key: "toolCalls", label: t("config.logs.toolCalls"), value: numberText(toolCallCountForEntry(entry)) },
  ];
}

function textLength(value: unknown): number {
  return typeof value === "string" ? Array.from(value).length : 0;
}

function roundUsage(entry: LlmRoundLogEntry): UnknownRecord | null {
  return asRecord(responseRecord(entry)?.usage);
}

function pipelineUsage(entry: LlmRoundLogEntry): UnknownRecord | null {
  const response = responseRecord(entry);
  return asRecord(response?.roundUsage) ?? roundUsage(entry);
}

function sectionUsage(payload: UnknownRecord | null): UnknownRecord | null {
  return asRecord(payload?.roundUsage) ?? asRecord(payload?.usage);
}

function usageMetricCards(usage: UnknownRecord | null): Metric[] {
  if (!usage) return [];
  const cacheWrite = (
    readNumber(usage, ["cacheCreationTokens", "cache_creation_tokens"]) ?? 0
  ) + (
    readNumber(usage, ["cacheCreation5mTokens", "cache_creation_5m_tokens"]) ?? 0
  ) + (
    readNumber(usage, ["cacheCreation1hTokens", "cache_creation_1h_tokens"]) ?? 0
  );
  return [
    { key: "prompt", label: t("config.logs.usagePrompt"), value: numberText(readNumber(usage, ["promptTokens", "prompt_tokens"])) },
    { key: "completion", label: t("config.logs.usageCompletion"), value: numberText(readNumber(usage, ["completionTokens", "completion_tokens"])) },
    { key: "total", label: t("config.logs.usageTotal"), value: numberText(readNumber(usage, ["totalTokens", "total_tokens"])) },
    { key: "cached", label: t("config.logs.usageCached"), value: numberText(readNumber(usage, ["cachedTokens", "cached_tokens"])) },
    { key: "write", label: t("config.logs.usageCacheWrite"), value: numberText(cacheWrite) },
    { key: "reasoning", label: t("config.logs.usageReasoning"), value: numberText(readNumber(usage, ["reasoningTokens", "reasoning_tokens"])) },
  ];
}

function toolCallCountForEntry(entry: LlmRoundLogEntry): number {
  const response = responseRecord(entry);
  const compactCount = readNumber(response, ["toolCallCount"]);
  if (compactCount != null) {
    return compactCount;
  }
  if (Array.isArray(response?.toolCalls)) {
    return response.toolCalls.length;
  }
  return (Array.isArray(response?.toolHistoryEvents) ? response.toolHistoryEvents : []).reduce((total, item) => {
    const record = asRecord(item);
    const calls = record?.tool_calls;
    return total + (Array.isArray(calls) ? calls.length : 0);
  }, 0);
}

function totalToolCallsForRounds(rounds?: LlmRoundLogEntry[]): number {
  return (rounds ?? []).reduce((total, round) => total + toolCallCountForEntry(round), 0);
}

function availableToolNames(entry: LlmRoundLogEntry): string[] {
  const tools = Array.isArray(entry.tools) ? entry.tools : [];
  return uniqueNames(tools
    .map((item) => typeof item === "string" ? item : asRecord(item)?.name)
    .filter((name): name is string => typeof name === "string" && !!name.trim())
    .map((name) => name.trim()));
}

function namesFromLogValueList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const names: string[] = [];
  for (const item of value) {
    if (typeof item === "string") {
      names.push(item);
      continue;
    }
    const record = asRecord(item);
    const fn = asRecord(record?.function);
    if (typeof record?.name === "string") {
      names.push(record.name);
    } else if (typeof fn?.name === "string") {
      names.push(fn.name);
    }
  }
  return uniqueNames(names.map((name) => name.trim()).filter(Boolean));
}

function calledToolNames(entry: LlmRoundLogEntry): string[] {
  const response = responseRecord(entry);
  const compactNames = namesFromLogValueList(response?.toolCallNames);
  if (compactNames.length > 0) return compactNames;
  const names: string[] = [];
  if (Array.isArray(response?.toolCalls)) {
    for (const call of response.toolCalls) {
      const record = asRecord(call);
      const fn = asRecord(record?.function);
      if (typeof fn?.name === "string") names.push(fn.name);
    }
  }
  if (Array.isArray(response?.toolHistoryEvents)) {
    for (const event of response.toolHistoryEvents) {
      const record = asRecord(event);
      const calls = record?.tool_calls;
      if (!Array.isArray(calls)) continue;
      for (const call of calls) {
        const callRecord = asRecord(call);
        const fn = asRecord(callRecord?.function);
        if (typeof fn?.name === "string") names.push(fn.name);
      }
    }
  }
  return uniqueNames(names.map((name) => name.trim()).filter(Boolean));
}

function toolSectionAvailableNames(payload: UnknownRecord | null, fallback: LlmRoundLogEntry): string[] {
  const names = namesFromLogValueList(payload?.availableTools);
  return names.length > 0 ? names : availableToolNames(fallback);
}

function toolSectionCalledNames(payload: UnknownRecord | null, fallback: LlmRoundLogEntry): string[] {
  const compactNames = namesFromLogValueList(payload?.toolCallNames);
  if (compactNames.length > 0) return compactNames;
  const directNames = namesFromLogValueList(payload?.toolCalls);
  const historyNames = summarizeToolHistoryEvents(payload?.toolHistoryEvents).names;
  const names = uniqueNames([...directNames, ...historyNames]);
  return names.length > 0 ? names : calledToolNames(fallback);
}

function uniqueNames(names: string[]): string[] {
  return [...new Set(names)].sort((a, b) => a.localeCompare(b));
}

function sanitizeToolsForUi(tools: unknown): unknown {
  const names = namesFromLogValueList(tools);
  return names.length > 0 ? names.map((name) => ({ name })) : null;
}

function summarizeToolHistoryEvents(value: unknown): { count: number; names: string[] } {
  if (!Array.isArray(value)) return { count: 0, names: [] };
  const names: string[] = [];
  let count = 0;
  for (const event of value) {
    const record = asRecord(event);
    const calls = record?.tool_calls;
    if (!Array.isArray(calls)) continue;
    count += calls.length;
    names.push(...namesFromLogValueList(calls));
  }
  return { count, names: uniqueNames(names) };
}

function sanitizeResponseForUi(response: unknown): unknown {
  const record = asRecord(response);
  if (!record) return response;
  const compact: UnknownRecord = {};
  for (const key of [
    "conversationId",
    "assistantTextLength",
    "activityReasoningTextLength",
    "reasoningContentLength",
    "reasoningTextLength",
    "toolCallCount",
    "toolCallNames",
    "usage",
    "roundUsage",
  ]) {
    if (record[key] !== undefined) {
      compact[key] = record[key];
    }
  }

  const assistantLength = readNumber(compact, ["assistantTextLength"]) ?? textLength(record.assistantText);
  if (assistantLength > 0) {
    compact.assistantTextLength = assistantLength;
  }
  const reasoningLength = readNumber(compact, [
    "reasoningContentLength",
    "activityReasoningTextLength",
    "reasoningTextLength",
  ]) ?? textLength(record.reasoningContent ?? record.activityReasoningText);
  if (reasoningLength > 0) {
    compact.reasoningContentLength = reasoningLength;
  }

  const directToolNames = namesFromLogValueList(record.toolCalls);
  const historyToolSummary = summarizeToolHistoryEvents(record.toolHistoryEvents);
  const compactToolNames = namesFromLogValueList(compact.toolCallNames);
  const toolNames = compactToolNames.length > 0
    ? compactToolNames
    : uniqueNames([...directToolNames, ...historyToolSummary.names]);
  if (toolNames.length > 0) {
    compact.toolCallNames = toolNames;
  }
  if (readNumber(compact, ["toolCallCount"]) == null) {
    const directCount = Array.isArray(record.toolCalls) ? record.toolCalls.length : 0;
    compact.toolCallCount = directCount + historyToolSummary.count;
  }
  return compact;
}

function sanitizeLogEntryForUi(entry: LlmRoundLogEntry): LlmRoundLogEntry {
  return {
    ...entry,
    tools: sanitizeToolsForUi(entry.tools),
    response: sanitizeResponseForUi(entry.response),
    rounds: entry.rounds?.map(sanitizeLogEntryForUi),
  };
}

function lazySectionForTab(tab: RoundDetailTabId): RoundLazySection | null {
  if (tab === "answer") return "answer";
  if (tab === "usage") return "usage";
  if (tab === "raw") return "raw_response";
  if (tab === "tools") return "tools";
  return null;
}

async function ensureRoundSection(tab: RoundDetailTabId = activeRoundTab.value) {
  const section = lazySectionForTab(tab);
  const id = selectedRound.value?.round.id;
  if (!section || !id || roundSectionPayloads.value[section] !== undefined) {
    return;
  }
  roundSectionLoading.value = section;
  roundSectionErrors.value = { ...roundSectionErrors.value, [section]: "" };
  try {
    const payload = await invokeTauri<unknown | null>("get_recent_llm_round_log_section", { id, section });
    if (selectedRound.value?.round.id !== id) {
      return;
    }
    roundSectionPayloads.value = { ...roundSectionPayloads.value, [section]: payload };
  } catch (error) {
    if (selectedRound.value?.round.id === id) {
      roundSectionErrors.value = { ...roundSectionErrors.value, [section]: toErrorMessage(error) };
    }
  } finally {
    if (selectedRound.value?.round.id === id && roundSectionLoading.value === section) {
      roundSectionLoading.value = null;
    }
  }
}

function setActiveRoundTab(tab: RoundDetailTabId) {
  activeRoundTab.value = tab;
  void ensureRoundSection(tab);
}

function openRound(pipeline: LlmRoundLogEntry, round: LlmRoundLogEntry, index: number) {
  selectedRound.value = { pipeline, round, index };
  roundSectionPayloads.value = {};
  roundSectionErrors.value = {};
  roundSectionLoading.value = null;
  activeRoundTab.value = "answer";
  void ensureRoundSection("answer");
}

function closeRound() {
  selectedRound.value = null;
  roundSectionPayloads.value = {};
  roundSectionErrors.value = {};
  roundSectionLoading.value = null;
}

async function reload() {
  loading.value = true;
  try {
    const list = await invokeTauri<LlmRoundLogEntry[]>("list_recent_llm_round_logs");
    logs.value = [...list].reverse().map(sanitizeLogEntryForUi);
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
    roundSectionPayloads.value = {};
    roundSectionErrors.value = {};
    roundSectionLoading.value = null;
  } finally {
    loading.value = false;
  }
}

// ========== 内置工具清单（list_tool_catalog 只读展示） ==========

const builtinToolCatalogDialog = ref<HTMLDialogElement | null>(null);
const builtinToolCatalogOpen = ref(false);
const builtinToolCatalogLoading = ref(false);
const builtinToolDefinitions = ref<FrontendToolDefinition[]>([]);

async function openBuiltinToolCatalog() {
  const dialog = builtinToolCatalogDialog.value;
  if (!dialog) return;
  builtinToolCatalogOpen.value = true;
  dialog.showModal();
  if (builtinToolDefinitions.value.length > 0) return;
  builtinToolCatalogLoading.value = true;
  try {
    const list = await invokeTauri<FrontendToolDefinition[]>("list_tool_catalog");
    builtinToolDefinitions.value = Array.isArray(list) ? list : [];
  } catch {
    builtinToolDefinitions.value = [];
  } finally {
    builtinToolCatalogLoading.value = false;
  }
}

function closeBuiltinToolCatalog() {
  builtinToolCatalogOpen.value = false;
  builtinToolCatalogDialog.value?.close();
}

type ToolSchemaShape = Record<string, unknown>;

function asToolSchemaShape(value: unknown): ToolSchemaShape {
  return value && typeof value === "object" ? (value as ToolSchemaShape) : {};
}

function toolSchemaTypeText(shape: ToolSchemaShape): string {
  if (shape.const !== undefined && shape.const !== null) {
    return String(shape.const);
  }
  const typeValue = shape.type;
  if (Array.isArray(shape.type)) {
    return shape.type.map(String).join(" | ");
  }
  return String(typeValue || "any");
}

function toolSchemaSummaryLine(name: string, shape: ToolSchemaShape, required: boolean): string {
  const requiredText = required ? "*" : "";
  const enumValues = Array.isArray(shape.enum) ? ` [${shape.enum.map(String).join(", ")}]` : "";
  const minText = shape.minimum !== undefined ? ` >= ${shape.minimum}` : "";
  const maxText = shape.maximum !== undefined ? ` <= ${shape.maximum}` : "";
  const rangeText = `${enumValues}${minText}${maxText}`.trim();
  const descriptionText = typeof shape.description === "string" ? shape.description.trim() : "";
  return `${requiredText}${name}: ${toolSchemaTypeText(shape)}${rangeText ? ` ${rangeText}` : ""}${descriptionText ? ` - ${descriptionText}` : ""}`;
}

function collectToolSchemaSummaryLines(
  properties: ToolSchemaShape,
  requiredRaw: string[],
  prefix = "",
): string[] {
  const lines = new Set<string>();
  for (const [name, schema] of Object.entries(properties)) {
    const shape = asToolSchemaShape(schema);
    const path = prefix ? `${prefix}.${name}` : name;
    lines.add(toolSchemaSummaryLine(path, shape, requiredRaw.includes(name)));

    const nestedPropertiesRaw = shape.properties;
    if (nestedPropertiesRaw && typeof nestedPropertiesRaw === "object") {
      const nestedRequired = Array.isArray(shape.required) ? shape.required : [];
      collectToolSchemaSummaryLines(
        nestedPropertiesRaw as ToolSchemaShape,
        nestedRequired.map(String),
        path,
      ).forEach((line) => lines.add(line));
    }

    const itemsShape = asToolSchemaShape(shape.items);
    if (itemsShape.properties && typeof itemsShape.properties === "object") {
      collectToolSchemaSummaryLines(
        itemsShape.properties as ToolSchemaShape,
        Array.isArray(itemsShape.required) ? itemsShape.required.map(String) : [],
        `${path}[]`,
      ).forEach((line) => lines.add(line));
    }
  }
  return Array.from(lines);
}

function formatSchemaExample(value: unknown): string {
  if (typeof value === "string") {
    return value.trim();
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function collectToolSchemaExamples(
  properties: ToolSchemaShape,
  prefix = "",
): string[] {
  const examples: string[] = [];
  for (const [name, schema] of Object.entries(properties)) {
    const shape = asToolSchemaShape(schema);
    const path = prefix ? `${prefix}.${name}` : name;
    if (shape.examples !== undefined) {
      examples.push(`${path}: ${formatSchemaExample(shape.examples)}`);
    } else if (shape.default !== undefined) {
      examples.push(`${path}: ${formatSchemaExample(shape.default)}`);
    }

    const nestedPropertiesRaw = shape.properties;
    if (nestedPropertiesRaw && typeof nestedPropertiesRaw === "object") {
      examples.push(...collectToolSchemaExamples(nestedPropertiesRaw as ToolSchemaShape, path));
    }

    const itemsShape = asToolSchemaShape(shape.items);
    if (itemsShape.properties && typeof itemsShape.properties === "object") {
      examples.push(...collectToolSchemaExamples(itemsShape.properties as ToolSchemaShape, `${path}[]`));
    }
  }
  return examples;
}

function toolParameterSummary(id: string): string[] {
  const definition = builtinToolDefinitions.value.find((item) => item.function?.name === id);
  const parameters = definition?.function?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const branches = Array.isArray(root.oneOf) ? root.oneOf : [];
  if (branches.length) {
    return Array.from(new Set(
      branches.flatMap((branch) => {
        const shape = asToolSchemaShape(branch);
        const propertiesRaw = shape.properties;
        const requiredRaw = Array.isArray(shape.required) ? shape.required.map(String) : [];
        if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
        return collectToolSchemaSummaryLines(propertiesRaw as ToolSchemaShape, requiredRaw);
      }),
    ));
  }
  const propertiesRaw = root.properties;
  const requiredRaw = Array.isArray(root.required) ? root.required.map(String) : [];
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return collectToolSchemaSummaryLines(propertiesRaw as ToolSchemaShape, requiredRaw);
}

function toolParameterExamples(id: string): string[] {
  const definition = builtinToolDefinitions.value.find((item) => item.function?.name === id);
  const parameters = definition?.function?.parameters;
  if (!parameters || typeof parameters !== "object") return [];
  const root = parameters as Record<string, unknown>;
  const branches = Array.isArray(root.oneOf) ? root.oneOf : [];
  if (branches.length) {
    return Array.from(new Set(
      branches.flatMap((branch) => {
        const shape = asToolSchemaShape(branch);
        const propertiesRaw = shape.properties;
        if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
        return collectToolSchemaExamples(propertiesRaw as ToolSchemaShape);
      }),
    ));
  }
  const propertiesRaw = root.properties;
  if (!propertiesRaw || typeof propertiesRaw !== "object") return [];
  return Array.from(new Set(collectToolSchemaExamples(propertiesRaw as ToolSchemaShape)));
}

onMounted(() => {
  void reload();
});
</script>
