<template>
  <div
    ref="rootRef"
    role="tablist"
    class="relative flex min-w-0 items-center"
    :class="[bordered ? 'border-b border-base-content/10' : '', size === 'sm' ? 'gap-1.5' : '']"
  >
    <button
      v-for="(tab, index) in tabs"
      :key="tab.key"
      :ref="(el) => setTabRef(el, index)"
      type="button"
      role="tab"
      class="tab min-w-0 gap-1.5"
      :class="[
        size === 'sm' ? 'h-7 whitespace-nowrap px-1 text-xs' : 'h-10 px-3 text-base',
        tab.key === activeKey
          ? ['font-semibold', tab.accentClass || 'text-base-content']
          : 'text-base-content/60 hover:text-base-content',
        tab.disabled ? 'pointer-events-none opacity-45' : '',
      ]"
      :aria-selected="tab.key === activeKey"
      :disabled="tab.disabled"
      @click="selectTab(tab)"
    >
      <span class="truncate">{{ tab.label }}</span>
      <span
        v-if="tab.badge !== undefined && tab.badge !== null && tab.badge !== ''"
        class="badge badge-xs shrink-0 font-mono"
        :class="tab.key === activeKey ? 'badge-neutral' : 'badge-ghost opacity-70'"
      >
        {{ tab.badge }}
      </span>
    </button>

    <span
      v-if="indicatorReady"
      class="ecall-tab-indicator pointer-events-none absolute bottom-0 h-0.5 rounded-full"
      :class="activeIndicatorClass"
      :style="indicatorStyle"
      aria-hidden="true"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

export type UnderlineTabItem = {
  key: string;
  label: string;
  badge?: string | number;
  disabled?: boolean;
  /** 选中时的文字色，默认 text-base-content */
  accentClass?: string;
  /** 选中时下划线的色，默认 bg-base-content */
  indicatorClass?: string;
};

const props = withDefaults(defineProps<{
  tabs: UnderlineTabItem[];
  activeKey: string;
  /** 尺寸：md=40px 高度常规字号，sm=28px 小字号 */
  size?: "md" | "sm";
  /** 是否在底部画一条基线 */
  bordered?: boolean;
}>(), {
  activeKey: "",
  size: "md",
  bordered: false,
});

const emit = defineEmits<{
  (e: "update:activeKey", key: string): void;
}>();

const rootRef = ref<HTMLElement | null>(null);
const tabRefs = ref<Array<HTMLElement | null>>([]);
const indicatorReady = ref(false);
const indicatorStyle = ref<Record<string, string>>({ width: "0px", transform: "translateX(0px)" });
let resizeObserver: ResizeObserver | null = null;

const activeIndex = computed(() => props.tabs.findIndex((tab) => tab.key === props.activeKey));

/** 下划线颜色跟随当前选中项；没给就退回前景色 */
const activeIndicatorClass = computed(() => {
  const active = props.tabs.find((tab) => tab.key === props.activeKey);
  return active?.indicatorClass || "bg-base-content";
});

// 测量选中项的位置与宽度，驱动下划线指示条滑动；等宽与不等宽两种排版都适用
function updateIndicator() {
  const el = activeIndex.value >= 0 ? tabRefs.value[activeIndex.value] : null;
  if (!el || el.offsetWidth === 0) {
    if (!el) indicatorReady.value = false;
    return;
  }
  indicatorStyle.value = {
    width: `${el.offsetWidth}px`,
    transform: `translateX(${el.offsetLeft}px)`,
  };
  indicatorReady.value = true;
}

function setTabRef(el: unknown, index: number) {
  tabRefs.value[index] = (el as HTMLElement) || null;
}

function selectTab(tab: UnderlineTabItem) {
  if (tab.disabled || tab.key === props.activeKey) return;
  emit("update:activeKey", tab.key);
}

onMounted(() => {
  updateIndicator();
  if (rootRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => updateIndicator());
    resizeObserver.observe(rootRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

watch(
  () => [props.tabs, props.activeKey],
  () => updateIndicator(),
  { deep: true, flush: "post" },
);
</script>
