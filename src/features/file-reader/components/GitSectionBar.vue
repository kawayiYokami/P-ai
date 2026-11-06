<template>
  <div
    class="relative flex shrink-0 cursor-pointer select-none items-center gap-1 border-b border-base-300 bg-base-200/35 px-2 py-1"
    @click="$emit('update:modelValue', !modelValue)"
  >
    <button
      type="button"
      class="btn btn-ghost btn-xs h-6 min-h-6 w-6 px-0"
      :title="modelValue ? t('gitPanel.expand') : t('gitPanel.collapse')"
      @click.stop="$emit('update:modelValue', !modelValue)"
    >
      <AppIcon name="chevron-down" v-if="!modelValue" class="h-3.5 w-3.5 opacity-70" />
      <AppIcon name="chevron-right" v-else class="h-3.5 w-3.5 opacity-70" />
    </button>
    <slot />
    <span class="flex-1"></span>
    <template v-if="!modelValue">
      <div class="flex min-w-0 flex-wrap items-center gap-1" @click.stop>
        <slot name="actions" />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";
import { useI18n } from "vue-i18n";

defineProps<{ modelValue: boolean }>();
defineEmits<{ (e: "update:modelValue", value: boolean): void }>();

const { t } = useI18n();
</script>
