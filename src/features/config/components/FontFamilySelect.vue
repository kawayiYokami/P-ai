<template>
  <div class="min-w-0">
    <!-- 触发按钮：popovertarget 原生切换 + anchor-name 锚定 -->
    <button
      type="button"
      tabindex="0"
      class="select select-bordered select-sm flex w-52 max-w-full shrink-0 items-center justify-between gap-2 pr-3 text-left"
      :disabled="disabled"
      v-bind="triggerAttrs"
      @click="toggle"
    >
      <span class="min-w-0 flex-1 truncate" :class="isAuto ? 'text-base-content/50' : ''">
        {{ displayLabel }}
      </span>
      <AppIcon name="chevron-down" class="h-4 w-4 shrink-0 opacity-70 transition-transform" :class="open ? 'rotate-180' : ''" />
    </button>

    <!-- 下拉面板：popover 顶层渲染（无 overflow 裁剪），anchor 定位到按钮 -->
    <ul
      :id="panelId"
      ref="panelRef"
      popover="auto"
      tabindex="0"
      class="dropdown dropdown-start menu z-50 max-h-[60vh] w-64 overflow-y-auto overflow-x-hidden rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
      :style="panelStyle"
      @toggle="onPanelToggle"
    >
      <li>
        <button
          type="button"
          class="truncate"
          :class="isAuto ? 'font-medium text-primary' : 'text-base-content/70'"
          @click="selectValue('auto')"
        >
          {{ autoLabel }}
        </button>
      </li>
      <li v-if="options.length" class="pointer-events-none my-1 h-px bg-base-200"></li>
      <li v-for="font in options" :key="font">
        <button
          type="button"
          class="truncate"
          :class="!isAuto && modelValue === font ? 'font-medium text-primary' : ''"
          @click="selectValue(font)"
        >
          {{ font }}
        </button>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

defineOptions({ inheritAttrs: false });

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

const anchorName = `--font-anchor-${Math.random().toString(36).slice(2, 10)}`;
const panelId = `font-family-select-${Math.random().toString(36).slice(2, 10)}`;
const panelRef = ref<HTMLElement | null>(null);
const open = ref(false);

const isAuto = computed(() => !props.modelValue || props.modelValue === "auto");
const displayLabel = computed(() => (isAuto.value ? props.autoLabel : props.modelValue));
// popovertarget 与 anchor-name 用 v-bind 注入，避免 vue-tsc 对原生属性的严格校验
const triggerAttrs = computed(() => (props.disabled
  ? {}
  : {
      popovertarget: panelId,
      style: { anchorName: anchorName } as Record<string, string>,
    }));
const panelStyle = computed(() => ({ positionAnchor: anchorName } as Record<string, string>));

function toggle() {
  // popover 原生管理显隐；箭头状态跟随 toggle 事件
}

function selectValue(value: string) {
  emit("update:modelValue", value);
  panelRef.value?.hidePopover();
}

function onPanelToggle(event: Event) {
  open.value = (event.target as HTMLElement | null)?.matches?.(":popover-open") ?? false;
}

function closeOnEscape(event: KeyboardEvent) {
  if (event.key === "Escape") {
    panelRef.value?.hidePopover();
  }
}

onMounted(() => {
  document.addEventListener("keydown", closeOnEscape);
});
onBeforeUnmount(() => {
  document.removeEventListener("keydown", closeOnEscape);
});
</script>