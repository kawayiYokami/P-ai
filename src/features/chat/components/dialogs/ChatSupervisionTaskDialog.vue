<template>
  <dialog class="modal" :class="{ 'modal-open': open }">
    <div class="modal-box w-11/12 max-w-lg p-0">
      <div class="border-b border-base-300/70 px-5 py-4">
        <div class="text-base font-semibold">
          {{ activeTask ? t("chat.supervision.updateTitle") : t("chat.supervision.createTitle") }}
        </div>
      </div>

      <div class="space-y-4 px-5 py-4">
        <div
          v-if="activeTask"
          class="rounded-box border border-primary/20 bg-primary/5 px-3 py-2 text-sm text-base-content/80"
        >
          {{ t("chat.supervision.activeHint", { endAt: activeTask.endAtLocal }) }}
        </div>

        <div
          v-if="errorText"
          class="rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error whitespace-pre-wrap break-all"
        >
          {{ errorText }}
        </div>

        <label class="block space-y-2">
          <span class="block text-sm font-medium">{{ t("chat.supervision.durationLabel") }}</span>
          <select v-model="durationHours" class="select select-bordered w-full" :disabled="saving">
            <option v-for="hour in durationOptions" :key="hour" :value="hour">
              {{ t("chat.supervision.durationOption", { hours: hour }) }}
            </option>
          </select>
        </label>

        <label class="block space-y-2">
          <span class="block text-sm font-medium">{{ t("chat.supervision.goalLabel") }}</span>
          <input
            v-model="goal"
            class="input input-bordered w-full"
            type="text"
            :placeholder="t('chat.supervision.goalPlaceholder')"
            :disabled="saving"
          />
        </label>

        <label class="block space-y-2">
          <span class="block text-sm font-medium">{{ t("chat.supervision.whyLabel") }}</span>
          <input
            v-model="why"
            class="input input-bordered w-full"
            type="text"
            :placeholder="t('chat.supervision.whyPlaceholder')"
            :disabled="saving"
          />
        </label>

        <label class="block space-y-2">
          <span class="block text-sm font-medium">{{ t("chat.supervision.todoLabel") }}</span>
          <input
            v-model="todo"
            class="input input-bordered w-full"
            type="text"
            :placeholder="t('chat.supervision.todoPlaceholder')"
            :disabled="saving"
          />
        </label>
      </div>

      <div class="border-t border-base-300/70 bg-base-100 px-5 py-4">
        <div class="flex items-end gap-4">
          <div v-if="recentHistory.length" class="min-w-0 flex-1">
            <div class="mb-2 text-xs font-medium uppercase tracking-[0.08em] text-base-content/50">
              {{ t("chat.supervision.recentTitle") }}
            </div>
            <div class="flex flex-wrap gap-2">
              <button
                v-for="(entry, index) in recentHistory"
                :key="`${entry.goal}-${entry.todo}-${index}`"
                type="button"
                class="min-w-0 max-w-full rounded-box border border-base-300 bg-base-200/70 px-3 py-2 text-left transition hover:border-primary/40 hover:bg-base-200"
                :disabled="saving"
                @click="applyRecentHistory(entry)"
              >
                <div class="truncate text-sm font-medium text-base-content">
                  {{ entry.goal }}
                </div>
              </button>
            </div>
          </div>
          <div class="ml-auto flex shrink-0 items-center justify-end gap-2">
            <button class="btn btn-ghost" :disabled="saving" @click="emit('close')">
              {{ t("common.cancel") }}
            </button>
            <button class="btn btn-primary" :disabled="saving || !canSubmit" @click="handleSave">
              {{ saving ? t("common.loading") : (activeTask ? t("chat.supervision.updateAction") : t("chat.supervision.createAction")) }}
            </button>
          </div>
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="emit('close')">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

type ActiveSupervisionTask = {
  taskId: string;
  goal: string;
  why: string;
  todo: string;
  endAtLocal: string;
  remainingHours: number;
};

type SupervisionHistoryEntry = {
  goal: string;
  why: string;
  todo: string;
  durationHours: number;
};

const props = defineProps<{
  open: boolean;
  saving: boolean;
  errorText: string;
  activeTask: ActiveSupervisionTask | null;
  recentHistory: SupervisionHistoryEntry[];
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "save", payload: { durationHours: number; goal: string; why: string; todo: string }): void;
}>();

const { t } = useI18n();

const durationOptions = Array.from({ length: 24 }, (_, index) => index + 1);
const goal = ref("");
const why = ref("");
const todo = ref("");
const durationHours = ref(1);

const canSubmit = computed(() => {
  return !!goal.value.trim() && !!todo.value.trim();
});

function resetForm() {
  goal.value = String(props.activeTask?.goal || t("chat.supervision.defaultGoal")).trim();
  why.value = String(props.activeTask?.why || t("chat.supervision.defaultWhy")).trim();
  todo.value = String(props.activeTask?.todo || t("chat.supervision.defaultTodo")).trim();
  durationHours.value = Number.isFinite(props.activeTask?.remainingHours)
    ? Math.min(24, Math.max(1, Number(props.activeTask?.remainingHours || 1)))
    : 1;
}

function handleSave() {
  if (!canSubmit.value) return;
  emit("save", {
    durationHours: durationHours.value,
    goal: goal.value.trim(),
    why: why.value.trim(),
    todo: todo.value.trim(),
  });
}

function applyRecentHistory(entry: SupervisionHistoryEntry) {
  goal.value = String(entry.goal || "").trim();
  why.value = String(entry.why || "").trim();
  todo.value = String(entry.todo || "").trim();
  durationHours.value = Math.min(24, Math.max(1, Number(entry.durationHours || 1)));
}

watch(
  () => [props.open, props.activeTask?.taskId, props.activeTask?.endAtLocal] as const,
  ([open]) => {
    if (!open) return;
    resetForm();
  },
  { immediate: true },
);
</script>
