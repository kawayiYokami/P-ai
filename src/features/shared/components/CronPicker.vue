<template>
  <div class="space-y-3">
    <!-- 只给人话：点选的每一步都同步成一两句可读描述，不露出 cron 语法 -->
    <div class="space-y-1">
      <div class="text-sm font-medium text-base-content">{{ description }}</div>
      <div v-if="nextRuns.length" class="flex flex-wrap gap-x-3 gap-y-1 font-mono text-xs text-base-content/60">
        <span v-for="(run, index) in nextRuns" :key="index">{{ formatRun(run) }}</span>
      </div>
    </div>

    <div v-if="invalidText" class="rounded-box border border-error/30 bg-error/10 px-3 py-2 text-xs text-error">
      {{ invalidText }}
    </div>

    <!-- 五个字段是同时生效的并列关系，全部列出来，跟着表单一起滚；
         不用标签页，免得被读成「选分钟还是选小时」 -->
    <div class="space-y-3">
      <div
        v-for="field in CRON_FIELDS"
        :key="field.key"
        class="space-y-2 border-b border-base-300/70 pb-3 last:border-b-0 last:pb-0"
      >
        <div class="flex flex-wrap items-center gap-2">
          <span class="w-8 shrink-0 text-sm font-medium text-base-content/80">{{ field.label }}</span>

          <div class="join shrink-0">
            <button
              v-for="mode in modesFor(field)"
              :key="mode.value"
              type="button"
              class="btn btn-xs join-item"
              :class="state[field.key].kind === mode.value ? 'btn-primary' : 'btn-ghost border-base-300'"
              :disabled="disabled"
              @click="switchMode(field.key, mode.value)"
            >
              {{ mode.label }}
            </button>
          </div>

          <div v-if="state[field.key].kind === 'step'" class="flex items-center gap-1.5">
            <input
              :value="stepValue(field.key)"
              class="input input-bordered input-xs w-16"
              type="number"
              :min="1"
              :max="field.max - field.min"
              :disabled="disabled"
              @input="handleStepInput(field.key, $event)"
            />
            <span class="text-xs text-base-content/60">个{{ field.label }}</span>
          </div>

          <span v-else-if="state[field.key].kind === 'any'" class="text-xs text-base-content/50">
            {{ anyHint(field) }}
          </span>
          <span v-else class="text-xs text-base-content/50">
            已选 {{ selectedCount(field.key) }} 个
          </span>
        </div>

        <!-- 指定值：全部可点；列数随宽度自适应，窄屏只换行不缩到点不到 -->
        <div
          v-if="state[field.key].kind === 'list'"
          class="grid gap-1"
          :style="gridStyle"
        >
          <button
            v-for="candidate in candidates(field)"
            :key="candidate.value"
            type="button"
            class="btn btn-xs h-7 min-h-7 w-full px-0"
            :class="isSelected(field.key, candidate.value) ? 'btn-primary' : 'btn-ghost border-base-300'"
            :disabled="disabled"
            @click="toggleValue(field.key, candidate.value)"
          >
            {{ candidate.label }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  CRON_FIELDS,
  WEEKDAY_LABELS,
  computeNextRuns,
  describeCronState,
  formatCronExpression,
  parseCronExpression,
  type CronFieldKey,
  type CronFieldMeta,
  type CronFieldValue,
  type CronState,
} from "./cron-picker";

const props = withDefaults(defineProps<{
  modelValue: string;
  disabled?: boolean;
}>(), {
  disabled: false,
});

const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();

const state = ref<CronState>(createFallbackState());
const invalidText = ref("");

/** 解析失败时的兜底：每天 09:00，保证界面永远有可点的东西 */
function createFallbackState(): CronState {
  return {
    minute: { kind: "list", values: [0] },
    hour: { kind: "list", values: [9] },
    day: { kind: "any" },
    month: { kind: "any" },
    weekday: { kind: "any" },
  };
}

const expression = computed(() => formatCronExpression(state.value));
/** 值网格的列数跟着容器走：窄了自动少列，按钮不会被压到点不准 */
const gridStyle = {
  gridTemplateColumns: "repeat(auto-fill, minmax(2.1rem, 1fr))",
};
const description = computed(() => describeCronState(state.value));
const nextRuns = computed(() => {
  try {
    return computeNextRuns(state.value, new Date(), 3);
  } catch {
    return [];
  }
});

/** 外部值变化 → 重新解析；解析不了就保留当前状态并提示，不静默改写外来表达式 */
watch(() => props.modelValue, (value) => {
  const raw = String(value ?? "").trim();
  if (!raw) {
    invalidText.value = "";
    return;
  }
  const parsed = parseCronExpression(raw);
  if (!parsed) {
    invalidText.value = `无法识别「${raw}」，请重新点选。`;
    return;
  }
  if (formatCronExpression(parsed) === expression.value) {
    invalidText.value = "";
    return;
  }
  state.value = parsed;
  invalidText.value = "";
}, { immediate: true });

function modesFor(field: CronFieldMeta) {
  const modes: { value: CronFieldValue["kind"]; label: string }[] = [
    { value: "any", label: "不限" },
  ];
  if (field.allowStep) {
    modes.push({ value: "step", label: "每隔" });
  }
  modes.push({ value: "list", label: "指定" });
  return modes;
}

function candidates(field: CronFieldMeta): { value: number; label: string }[] {
  const result: { value: number; label: string }[] = [];
  for (let value = field.min; value <= field.max; value += 1) {
    result.push({
      value,
      label: field.key === "weekday" ? WEEKDAY_LABELS[value] : String(value),
    });
  }
  return result;
}

function anyHint(field: CronFieldMeta): string {
  if (field.key === "weekday") return "每周的每一天";
  if (field.key === "day") return "每个月的每一天";
  if (field.key === "month") return "每个月";
  return `每${field.label}`;
}

function isSelected(key: CronFieldKey, value: number): boolean {
  const current = state.value[key];
  return current.kind === "list" && current.values.includes(value);
}

function selectedCount(key: CronFieldKey): number {
  const current = state.value[key];
  return current.kind === "list" ? current.values.length : 0;
}

function stepValue(key: CronFieldKey): number | string {
  const current = state.value[key];
  return current.kind === "step" ? current.step : 1;
}

/** 切换字段的取值方式；切到「指定」时先放进当前位置，避免空选择 */
function switchMode(key: CronFieldKey, kind: CronFieldValue["kind"]) {
  if (props.disabled) return;
  const field = CRON_FIELDS.find((item) => item.key === key)!;
  const current = state.value[key];
  if (current.kind === kind) return;

  if (kind === "any") {
    state.value = { ...state.value, [key]: { kind: "any" } };
  } else if (kind === "step") {
    const step = current.kind === "step" ? current.step : 2;
    state.value = { ...state.value, [key]: { kind: "step", step } };
  } else {
    const seed = current.kind === "list" && current.values.length
      ? current.values
      : [field.min];
    state.value = { ...state.value, [key]: { kind: "list", values: seed } };
  }
  commit();
}

/** 点选一个值：已选则去掉，未选则加上；最后一个被去掉时退回「不限」 */
function toggleValue(key: CronFieldKey, value: number) {
  if (props.disabled) return;
  const current = state.value[key];
  if (current.kind !== "list") return;
  const next = current.values.includes(value)
    ? current.values.filter((item) => item !== value)
    : [...current.values, value].sort((left, right) => left - right);

  if (!next.length) {
    state.value = { ...state.value, [key]: { kind: "any" } };
  } else {
    state.value = { ...state.value, [key]: { kind: "list", values: next } };
  }
  commit();
}

function handleStepInput(key: CronFieldKey, event: Event) {
  const field = CRON_FIELDS.find((item) => item.key === key)!;
  const target = event.target as HTMLInputElement;
  const raw = Number.parseInt(target.value, 10);
  const step = Number.isFinite(raw) ? Math.min(Math.max(1, raw), field.max - field.min) : 2;
  state.value = { ...state.value, [key]: { kind: "step", step } };
  commit();
}

function commit() {
  emit("update:modelValue", expression.value);
}

function formatRun(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getMonth() + 1}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/**
 * 外部传进来的表达式与内部状态不一致时（比如宿主重置表单），
 * 只有点了控件才回写，避免 watch 里 emit 造成回环。
 */
defineExpose({ expression });
</script>
