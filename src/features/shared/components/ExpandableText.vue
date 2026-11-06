<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "./AppIcon.vue";

const props = withDefaults(
  defineProps<{
    text: string;
    /** 折叠时显示的最小行数，默认 4 行 */
    previewLines?: number;
    /** 直接指定预览高度（像素），优先于 previewLines */
    previewHeight?: number;
    follow?: boolean;
    textClass?: string;
    /** 受控展开态；不传时由组件内部维护 */
    expanded?: boolean;
    /** 是否在右上角展示折叠箭头 */
    showChevron?: boolean;
    /** 允许点击首行区域折叠/展开 */
    clickableHeader?: boolean;
  }>(),
  {
    previewLines: 4,
    previewHeight: 0,
    follow: false,
    textClass: "",
    expanded: undefined,
    showChevron: true,
    clickableHeader: true,
  },
);

const emit = defineEmits<{ (e: "update:expanded", value: boolean): void }>();

const { t } = useI18n();

const shellRef = ref<HTMLElement | null>(null);
const bodyRef = ref<HTMLElement | null>(null);
const overflowing = ref(false);
const innerExpanded = ref(false);
const contentHeight = ref(0);

// 折叠只看展开态；follow 只表示「展开后不设高度上限、随内容自然生长」，不再隐含强制展开
const expanded = computed(() => (props.expanded === undefined ? innerExpanded.value : props.expanded));
const clamped = computed(() => !expanded.value);

const effectivePreviewHeight = computed(() => {
  if (props.previewHeight && props.previewHeight > 0) {
    return props.previewHeight;
  }
  const lines = Math.max(1, props.previewLines ?? 4);
  // text-xs leading-relaxed: 单行约 19.5px，4 行约 78px，微调至 84px 保证第 4 行能完整呈现
  return Math.round(lines * 21);
});

const shellStyle = computed(() => {
  if (props.follow && expanded.value) return undefined;
  const hPx = `${effectivePreviewHeight.value}px`;
  const vars = { "--ecall-preview-height": hPx } as Record<string, string>;
  if (clamped.value) {
    return { ...vars, maxHeight: hPx } as any;
  }
  const h = contentHeight.value > 0 ? `${contentHeight.value}px` : "none";
  return { ...vars, maxHeight: h } as any;
});

function setExpanded(next: boolean): void {
  if (props.expanded === undefined) {
    innerExpanded.value = next;
    return;
  }
  emit("update:expanded", next);
}

function toggleExpanded(): void {
  setExpanded(!expanded.value);
}

function onHeaderAreaClick(event: MouseEvent): void {
  if (!props.clickableHeader || !overflowing.value) return;
  if (window.getSelection()?.toString()) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest('button, a, input, textarea, select, [data-selection-ignore="true"]')) {
    return;
  }
  // 折叠态：点击文本任意位置展开
  if (!expanded.value) {
    setExpanded(true);
    return;
  }
  // 展开态：点击首行区域（距离容器顶部 26px 以内）触发收起
  const shellEl = shellRef.value;
  if (shellEl) {
    const rect = shellEl.getBoundingClientRect();
    const offsetY = event.clientY - rect.top;
    if (offsetY <= 26) {
      setExpanded(false);
    }
  }
}

let resizeObserver: ResizeObserver | null = null;

function measure(): void {
  const el = bodyRef.value;
  if (!el) return;
  const sh = el.scrollHeight;
  contentHeight.value = sh;
  overflowing.value = sh > effectivePreviewHeight.value + 4;
}

watch(
  () => [props.text, effectivePreviewHeight.value, props.follow] as const,
  () => {
    nextTick(measure);
  },
);

watch(expanded, () => {
  void nextTick(measure);
});

onMounted(() => {
  measure();
  if (bodyRef.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => measure());
    resizeObserver.observe(bodyRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});
</script>

<template>
  <div
    class="ecall-expandable relative flex min-w-0 flex-col"
    :class="{
      'ecall-expandable--expanded': expanded,
      'ecall-expandable--clamped': clamped,
      'ecall-expandable--following': props.follow && expanded,
    }"
  >
    <!-- 右上角折叠箭头：展开收起双向联动 -->
    <button
      v-if="props.showChevron && overflowing"
      type="button"
      class="absolute right-1 top-0.5 z-10 flex h-4 w-4 items-center justify-center rounded text-base-content/45 hover:bg-base-200 hover:text-base-content/80 transition-colors"
      :title="expanded ? t('common.collapse') : t('common.expand')"
      data-selection-ignore="true"
      @click.stop="toggleExpanded"
    >
      <AppIcon name="chevron-down"
        class="h-3.5 w-3.5 transition-transform duration-150"
        :class="{ 'rotate-180': expanded }"
      />
    </button>

    <!-- 可折叠文本外壳 -->
    <div
      ref="shellRef"
      class="ecall-expandable__shell"
      :class="{
        'pr-5': props.showChevron && overflowing,
        'cursor-pointer': clamped && overflowing && props.clickableHeader,
      }"
      :style="shellStyle"
      @click="onHeaderAreaClick"
    >
      <div class="ecall-expandable__content">
        <div
          ref="bodyRef"
          class="whitespace-pre-wrap wrap-break-word text-xs leading-relaxed"
          :class="[props.textClass, { 'ecall-expandable-text-clamped': clamped && overflowing }]"
        >
          <slot :text="text">{{ text }}</slot>
        </div>
      </div>
    </div>

    <!-- 底部操作栏：上下都有收起 -->
    <div v-if="overflowing" class="mt-1 flex items-center">
      <button
        v-if="!expanded"
        type="button"
        class="inline-flex items-center gap-1 text-xs text-base-content/45 hover:text-base-content/80 transition-colors"
        data-selection-ignore="true"
        @pointerdown.stop.prevent="setExpanded(true)"
        @click.stop
      >
        <span>{{ t("common.expand") }}</span>
        <AppIcon name="chevron-down" class="h-3 w-3" />
      </button>
      <button
        v-else
        type="button"
        class="inline-flex items-center gap-1 text-xs text-base-content/45 hover:text-base-content/80 transition-colors"
        data-selection-ignore="true"
        @pointerdown.stop.prevent="setExpanded(false)"
        @click.stop
      >
        <span>{{ t("common.collapse") }}</span>
        <AppIcon name="chevron-up" class="h-3 w-3" />
      </button>
    </div>
  </div>
</template>
