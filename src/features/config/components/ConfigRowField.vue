<template>
  <div class="flex min-w-0 flex-wrap items-center justify-between gap-x-4 gap-y-2" :class="rowClass">
    <div class="flex min-w-0 items-center gap-1.5 whitespace-nowrap">
      <span class="text-sm">{{ label }}</span>
      <EcallDropdown
        v-if="hint"
        v-model="hintOpen"
        teleport
        :match-trigger-width="false"
        placement="auto"
        panel-class="w-64 p-3 text-xs leading-relaxed text-base-content/70"
      >
        <template #trigger="{ toggle }">
          <button
            type="button"
            class="btn btn-ghost btn-xs btn-circle size-5 min-h-0 shrink-0 p-0 opacity-50 hover:opacity-100"
            :title="label"
            @click="toggle"
          >
            <AppIcon name="help-circle" class="size-3.5" />
          </button>
        </template>
        {{ hint }}
      </EcallDropdown>
    </div>
    <div class="flex min-w-0 shrink-0 items-center gap-2">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import EcallDropdown from "../../shared/components/EcallDropdown.vue";

defineProps<{
  label: string;
  hint?: string;
  rowClass?: string;
}>();

const hintOpen = ref(false);
</script>
