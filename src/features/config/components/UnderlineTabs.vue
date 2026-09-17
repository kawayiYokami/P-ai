<template>
  <div ref="rootRef" role="tablist" class="relative flex min-w-0 items-center">
    <button
      v-for="(tab, index) in tabs"
      :key="tab.key"
      :ref="(el) => setTabRef(el, index)"
      type="button"
      role="tab"
      class="tab h-10 min-w-0 gap-1.5 px-3 text-base"
      :class="[
        tab.key === activeKey
          ? 'font-medium text-base-content'
          : 'text-base-content/60 hover:text-base-content',
        tab.disabled ? 'pointer-events-none opacity-45' : '',
      ]"
      :aria-selected="tab.key === activeKey"
      :disabled="tab.disabled"
      @click="selectTab(tab)"
    >
      <component :is="tab.icon" v-if="tab.icon" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
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
      class="ecall-tab-indicator pointer-events-none absolute bottom-0 h-0.5 rounded-full bg-base-content"
      :style="indicatorStyle"
      aria-hidden="true"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";

export type UnderlineTabItem = {
  key: string;
  label: string;
  icon?: Component;
  badge?: string | number;
  disabled?: boolean;
};

const props = withDefaults(defineProps<{
  tabs: UnderlineTabItem[];
  activeKey: string;
}>(), {
  activeKey: "",
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
