<template>
  <div
    ref="rootRef"
    role="radiogroup"
    class="relative inline-flex items-stretch rounded-field p-0.5"
    :class="[
      surfaceClass,
      fullWidth ? 'flex w-full min-w-0' : '',
      disabled ? 'pointer-events-none opacity-60' : '',
    ]"
  >
    <span
      v-if="sliderReady && !disabled"
      class="pointer-events-none absolute bottom-0.5 top-0.5 z-0 rounded-[calc(var(--radius-field)-2px)] bg-base-100 shadow-sm transition-[transform,width] duration-200 ease-out"
      :style="sliderStyle"
      aria-hidden="true"
    />
    <button
      v-for="(option, index) in options"
      :key="String(option.value)"
      :ref="(el) => setButtonRef(el, index)"
      type="button"
      role="radio"
      class="relative z-10 flex items-center justify-center whitespace-nowrap rounded-[calc(var(--radius-field)-2px)] transition-colors"
      :class="[
        fullWidth ? 'min-w-0 flex-1' : '',
        sizeClass,
        isSelected(option.value)
          ? 'font-medium text-base-content'
          : 'text-base-content/60 hover:text-base-content',
      ]"
      :disabled="disabled || !!option.disabled"
      :aria-checked="isSelected(option.value)"
      @click="selectValue(option.value)"
    >
      <slot name="option" :option="option" :selected="isSelected(option.value)" :index="index">
        <span class="truncate">{{ option.label }}</span>
        <span
          v-if="option.badge !== undefined && option.badge !== null && option.badge !== ''"
          class="badge badge-xs ml-1.5 shrink-0 transition-colors font-mono"
          :class="isSelected(option.value) ? 'badge-neutral' : 'badge-ghost opacity-70'"
        >
          {{ option.badge }}
        </span>
      </slot>
    </button>
  </div>
</template>

<script setup lang="ts" generic="T extends string | number | boolean">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

export type SegmentedControlOption<T extends string | number | boolean> = {
  value: T;
  label: string;
  badge?: string | number;
  disabled?: boolean;
};

const props = withDefaults(defineProps<{
  modelValue: T;
  options: Array<SegmentedControlOption<T>>;
  disabled?: boolean;
  fullWidth?: boolean;
  size?: "xs" | "sm" | "md";
  surfaceClass?: string;
}>(), {
  disabled: false,
  fullWidth: true,
  size: "md",
  surfaceClass: "bg-base-200",
});

const emit = defineEmits<{
  (e: "update:modelValue", value: T): void;
  (e: "change", value: T): void;
}>();

// DaisyUI 5 的 tab 尺寸类（tab-sm 等）已不存在，尺寸全部用 utility 自控
const sizeClass = computed(() => {
  if (props.size === "xs") return "h-6 px-2 text-xs leading-none gap-1";
  if (props.size === "sm") return "h-7 px-2.5 text-xs leading-none gap-1.5";
  return "h-8 px-3.5 text-xs font-medium leading-none gap-1.5";
});

const rootRef = ref<HTMLElement | null>(null);
const buttonRefs = ref<Array<HTMLElement | null>>([]);
const sliderReady = ref(false);
const sliderStyle = ref<Record<string, string>>({ width: "0px", transform: "translateX(0px)" });
let resizeObserver: ResizeObserver | null = null;

const activeIndex = computed(() =>
  props.options.findIndex((option) => option.value === props.modelValue),
);

// 测量选中按钮的位置与宽度，驱动滑动指示块；等宽与不等宽两种模式都适用
function updateSlider() {
  const index = activeIndex.value;
  const el = index >= 0 ? buttonRefs.value[index] : null;
  if (!el) return;
  sliderStyle.value = {
    width: `${el.offsetWidth}px`,
    transform: `translateX(${el.offsetLeft}px)`,
  };
  sliderReady.value = true;
}

function setButtonRef(el: unknown, index: number) {
  buttonRefs.value[index] = (el as HTMLElement) || null;
}

onMounted(() => {
  updateSlider();
  if (rootRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => updateSlider());
    resizeObserver.observe(rootRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

watch(
  () => [props.options, props.modelValue],
  () => updateSlider(),
  { deep: true, flush: "post" },
);

function isSelected(value: T): boolean {
  return props.modelValue === value;
}

function selectValue(value: T) {
  if (props.disabled || props.modelValue === value) return;
  emit("update:modelValue", value);
  emit("change", value);
}
</script>
