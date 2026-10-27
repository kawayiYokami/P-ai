<template>
  <EcallDropdown
    v-model="open"
    teleport
    :match-trigger-width="false"
    placement="auto"
    root-class="w-auto min-w-0 max-w-[90vw]"
    panel-class="w-max min-w-52 max-w-[90vw]"
  >
    <template #trigger="{ toggle }">
      <button
        type="button"
        class="select select-bordered bg-none flex w-full min-w-0 items-center justify-between gap-2 pr-3 text-left"
        :class="size === 'sm' ? 'select-sm' : ''"
        :disabled="disabled"
        :title="selectedLabel"
        @click="toggle"
      >
        <span class="min-w-0 flex-1 truncate">{{ selectedLabel }}</span>
        <ChevronDown class="size-4 shrink-0 opacity-50" />
      </button>
    </template>

    <OverlayScrollArea scroller-class="max-h-64 overscroll-contain">
      <ul class="menu menu-sm w-full p-1">
        <li v-if="placeholder">
          <button
            type="button"
            class="text-sm"
            :class="!modelValue ? 'active' : ''"
            @click="select('')"
          >
            <span class="min-w-0 flex-1 truncate">{{ placeholder }}</span>
            <span v-if="!modelValue" class="badge badge-xs badge-primary shrink-0">✓</span>
          </button>
        </li>
        <li v-for="option in options" :key="option.value">
          <button
            type="button"
            class="text-sm"
            :class="option.value === modelValue ? 'active' : ''"
            @click="select(option.value)"
          >
            <span class="min-w-0 flex-1 truncate">{{ option.label }}</span>
            <span v-if="option.value === modelValue" class="badge badge-xs badge-primary shrink-0">✓</span>
          </button>
        </li>
        <li v-if="options.length === 0 && !placeholder">
          <span class="text-sm text-base-content/50">{{ emptyText }}</span>
        </li>
      </ul>
    </OverlayScrollArea>
  </EcallDropdown>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown } from "@lucide/vue";
import EcallDropdown from "../../shared/components/EcallDropdown.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";

const props = withDefaults(defineProps<{
  modelValue?: string;
  options: Array<{ value: string; label: string }>;
  /** 空值选项文案；为空时不显示空值项 */
  placeholder?: string;
  disabled?: boolean;
  size?: "sm" | "md";
  emptyText?: string;
}>(), {
  modelValue: "",
  placeholder: "",
  disabled: false,
  size: "md",
  emptyText: "",
});

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const open = ref(false);

const selectedLabel = computed(() => {
  const found = props.options.find((option) => option.value === props.modelValue);
  if (found) return found.label;
  if (!props.modelValue) return props.placeholder;
  return props.modelValue;
});

function select(value: string) {
  open.value = false;
  if (value === props.modelValue) return;
  emit("update:modelValue", value);
}
</script>
