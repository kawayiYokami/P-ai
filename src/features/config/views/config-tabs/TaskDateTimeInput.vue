<template>
  <div ref="rootRef" class="relative flex flex-wrap items-center gap-2">
    <button
      :disabled="disabled"
      :aria-expanded="calendarOpen"
      class="input input-bordered flex min-w-[12rem] flex-1 items-center justify-between gap-2 text-left"
      type="button"
      @click="toggleCalendar"
    >
      <span class="truncate">{{ dateButtonText }}</span>
      <svg class="size-4 shrink-0 opacity-60" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M8 2v4"></path>
        <path d="M16 2v4"></path>
        <path d="M3 10h18"></path>
        <rect x="3" y="4" width="18" height="18" rx="2"></rect>
      </svg>
    </button>

    <div
      v-if="calendarOpen"
      class="rounded-box absolute left-0 z-50 border border-base-300 bg-base-100 p-3 shadow-xl"
      :class="'bottom-full mb-2'"
    >
      <calendar-date
        class="cally"
        :locale="locale"
        :value="datePart"
        @change="onDateChange"
      >
        <svg aria-label="Previous" class="fill-current size-4" slot="previous" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
          <path d="M15.75 19.5 8.25 12l7.5-7.5"></path>
        </svg>
        <svg aria-label="Next" class="fill-current size-4" slot="next" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
          <path d="m8.25 4.5 7.5 7.5-7.5 7.5"></path>
        </svg>
        <calendar-month></calendar-month>
      </calendar-date>
    </div>

    <input
      :disabled="disabled"
      :value="timePart"
      class="input input-bordered w-28"
      step="60"
      type="time"
      @input="onTimeInput"
    />

    <div class="join">
      <button class="btn btn-sm join-item" type="button" :disabled="disabled" @click="nudgeMinutes(-10)">-10m</button>
      <button class="btn btn-sm join-item" type="button" :disabled="disabled" @click="setNow">{{ t("config.task.timeNow") }}</button>
      <button class="btn btn-sm join-item" type="button" :disabled="disabled" @click="nudgeMinutes(10)">+10m</button>
      <button class="btn btn-sm btn-ghost join-item" type="button" :disabled="disabled || !modelValue" :title="t('common.reset')" @click="clearValue">×</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import "cally";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  composeLocalRfc3339,
  extractIsoLocalDatePart,
  extractIsoLocalTimePart,
  nowLocalDatePart,
  nowLocalRfc3339ForInput,
  nudgeLocalRfc3339Minutes,
} from "../../../../utils/time";

const props = defineProps<{
  modelValue: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const { t, locale } = useI18n();
const rootRef = ref<HTMLElement | null>(null);
const calendarOpen = ref(false);

const datePart = computed(() => extractIsoLocalDatePart(props.modelValue));
const timePart = computed(() => extractIsoLocalTimePart(props.modelValue));
const dateButtonText = computed(() => datePart.value || t("config.task.pickDate"));

function emitValue(nextValue: string) {
  emit("update:modelValue", nextValue);
}

function closeCalendar() {
  calendarOpen.value = false;
}

function toggleCalendar() {
  if (props.disabled) return;
  if (calendarOpen.value) {
    closeCalendar();
    return;
  }
  calendarOpen.value = true;
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!calendarOpen.value) return;
  const root = rootRef.value;
  const target = event.target;
  if (!root || !(target instanceof Node)) return;
  if (!root.contains(target)) {
    closeCalendar();
  }
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeCalendar();
  }
}

function onDateChange(event: Event) {
  const nextDate = String((event.target as HTMLInputElement & { value?: string }).value || "").trim();
  if (!nextDate) return;
  emitValue(composeLocalRfc3339(nextDate, timePart.value, props.modelValue));
  closeCalendar();
}

function onTimeInput(event: Event) {
  const nextTime = String((event.target as HTMLInputElement).value || "").trim();
  if (!nextTime) {
    emitValue("");
    return;
  }
  emitValue(composeLocalRfc3339(datePart.value || nowLocalDatePart(), nextTime, props.modelValue));
}

function nudgeMinutes(deltaMinutes: number) {
  emitValue(nudgeLocalRfc3339Minutes(props.modelValue, deltaMinutes));
}

function setNow() {
  emitValue(nowLocalRfc3339ForInput());
}

function clearValue() {
  emitValue("");
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  window.addEventListener("keydown", handleWindowKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>
