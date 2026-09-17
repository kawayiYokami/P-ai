<template>
  <div v-if="snapshot" class="relative pointer-events-auto">
    <button
      type="button"
      class="btn btn-sm h-9 min-h-9 gap-2 rounded-full border border-base-300 bg-base-100/95 px-3 shadow-lg backdrop-blur-md hover:bg-base-100"
      :title="dashboardTitle"
      @click="expanded = !expanded"
    >
      <span class="relative flex size-2.5">
        <span class="absolute inline-flex size-full rounded-full" :class="presencePulseClass"></span>
        <span class="relative inline-flex size-2.5 rounded-full" :class="presenceDotClass"></span>
      </span>
      <span class="text-xs font-medium">{{ presenceText }}</span>
      <span class="h-4 w-px bg-base-300"></span>
      <BatteryCharging class="size-4" :class="energyClass" aria-hidden="true" />
      <span class="text-xs font-semibold tabular-nums" :class="energyClass">{{ compactEnergyText }}</span>
    </button>

    <Transition name="remote-energy-card">
      <section
        v-if="expanded"
        class="absolute bottom-full left-1/2 z-30 mb-2 w-64 -translate-x-1/2 rounded-box border border-base-300 bg-base-100/95 p-3 shadow-xl backdrop-blur-md"
      >
        <div class="flex items-center justify-between gap-3">
          <span class="text-xs font-semibold">远程会话能量</span>
          <span class="inline-flex items-center gap-1.5 text-xs" :class="presenceClass">
            <span class="size-1.5 rounded-full" :class="presenceDotClass"></span>
            {{ presenceText }}
          </span>
        </div>
        <div class="mt-3 flex items-end justify-between gap-3">
          <span class="text-2xl font-semibold tracking-tight tabular-nums" :class="energyClass">{{ energyText }}</span>
          <span class="pb-1 text-xs text-base-content/55 tabular-nums">/ {{ maximumEnergyText }}</span>
        </div>
        <div class="mt-2 h-2 overflow-hidden rounded-full bg-base-300/80">
          <div
            class="h-full rounded-full transition-[width] duration-300"
            :class="energyFillClass"
            :style="{ width: `${energyFillPercent}%` }"
          ></div>
        </div>
        <div class="mt-3 flex items-center justify-between gap-3 text-xs text-base-content/60">
          <span>恢复速率</span>
          <span class="tabular-nums">{{ recoveryText }}</span>
        </div>
        <div v-if="lastPresenceText" class="mt-1 flex items-center justify-between gap-3 text-xs text-base-content/60">
          <span>最近在场</span>
          <span class="tabular-nums">{{ lastPresenceText }}</span>
        </div>
      </section>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { BatteryCharging } from "@lucide/vue";
import type { RemoteImContactDashboardSnapshot } from "../../../types/app";

const props = defineProps<{
  snapshot: RemoteImContactDashboardSnapshot | null;
}>();

const expanded = ref(false);
const isPresent = computed(() => props.snapshot?.presence === "present");
const presenceText = computed(() => isPresent.value ? "在场" : "离场");
const presenceClass = computed(() => isPresent.value ? "text-success" : "text-base-content/45");
const presenceDotClass = computed(() => isPresent.value ? "bg-success" : "bg-base-content/45");
const presencePulseClass = computed(() => isPresent.value ? "animate-ping bg-success/35" : "bg-transparent");
const energy = computed(() => finiteNumber(props.snapshot?.energy));
const maximumEnergy = computed(() => Math.max(0, finiteNumber(props.snapshot?.maximumEnergy)));
const energyFillPercent = computed(() => {
  if (maximumEnergy.value <= 0) return 0;
  return Math.max(0, Math.min(100, energy.value / maximumEnergy.value * 100));
});
const energyClass = computed(() => energy.value > 0 ? "text-info" : "text-error");
const energyFillClass = computed(() => energy.value > 0 ? "bg-info" : "bg-error");
const energyText = computed(() => formatNumber(energy.value));
const maximumEnergyText = computed(() => formatNumber(maximumEnergy.value));
const compactEnergyText = computed(() => `${energyText.value}/${maximumEnergyText.value}`);
const recoveryText = computed(() => `+${formatNumber(finiteNumber(props.snapshot?.energyRecoveryPerSecond))} / 秒`);
const lastPresenceText = computed(() => formatDate(props.snapshot?.lastPresenceAt));
const dashboardTitle = computed(() => `${presenceText.value}，能量 ${compactEnergyText.value}`);

function finiteNumber(value: unknown) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function formatNumber(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function formatDate(value: string | undefined) {
  if (!value) return "";
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}
</script>

<style scoped>
.remote-energy-card-enter-active,
.remote-energy-card-leave-active {
  transition: opacity 140ms ease, transform 140ms ease;
}

.remote-energy-card-enter-from,
.remote-energy-card-leave-to {
  opacity: 0;
  transform: translate(-50%, 4px);
}
</style>
