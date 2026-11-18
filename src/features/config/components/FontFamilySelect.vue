<template>
  <!-- 原生 <select> + daisyUI 的 `.select`：箭头、边框、内边距、聚焦态全部由组件库提供，
       不需要自己的箭头、不需要 bg-none 关掉库的背景图，也不需要 popover 自己搭面板。
       之前是 `<button class="select">` 伪装下拉 + 自画 chevron，库的箭头还在，于是两个箭头叠在一起。 -->
  <select
    class="select select-bordered select-sm w-52 max-w-full shrink-0"
    :class="isAuto ? 'text-base-content/50' : ''"
    :disabled="disabled"
    :value="props.modelValue || 'auto'"
    @change="onChange"
  >
    <option value="auto">{{ autoLabel }}</option>
    <option v-for="font in options" :key="font" :value="font">{{ font }}</option>
  </select>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(defineProps<{
  modelValue?: string;
  options?: string[];
  autoLabel?: string;
  disabled?: boolean;
}>(), {
  modelValue: "auto",
  options: () => [],
  autoLabel: "自动",
  disabled: false,
});

const emit = defineEmits<{
  (event: "update:modelValue", value: string): void;
}>();

const isAuto = computed(() => !props.modelValue || props.modelValue === "auto");

function onChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  emit("update:modelValue", value);
}
</script>
