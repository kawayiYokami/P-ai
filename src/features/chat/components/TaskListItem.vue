<template>
  <div class="group relative mx-1">
    <button
      type="button"
      class="block w-full rounded-lg px-2 py-2 text-left transition-colors hover:bg-base-100/70 disabled:cursor-default disabled:hover:bg-transparent"
      :disabled="disabled"
      :title="title"
      @click="$emit('click')"
    >
      <div class="flex min-w-0 items-start gap-2">
        <div class="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-base-100 text-base-content/65">
          <AppIcon name="list-todo" class="h-4 w-4" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex min-w-0 items-start justify-between gap-2">
            <div class="min-w-0 flex-1 truncate whitespace-nowrap text-sm font-medium">
              {{ label }}
            </div>
            <div v-if="timeDateLabel" class="shrink-0 text-right text-xs leading-4 text-base-content/55">
              {{ timeDateLabel }}
            </div>
          </div>
          <div class="mt-1 flex min-w-0 items-start justify-between gap-2 text-xs text-base-content/55">
            <div class="min-w-0 flex-1 truncate">
              {{ description }}
            </div>
            <div v-if="timeMinuteLabel" class="shrink-0 text-right leading-4">
              {{ timeMinuteLabel }}
            </div>
          </div>
        </div>
      </div>
    </button>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";

defineProps<{
  label: string;
  description: string;
  title?: string;
  timeDateLabel?: string;
  timeMinuteLabel?: string;
  disabled?: boolean;
}>();

defineEmits<{
  click: [];
}>();
</script>
