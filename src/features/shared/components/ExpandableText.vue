<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = withDefaults(
  defineProps<{
    text: string;
    previewHeight?: number;
    follow?: boolean;
    textClass?: string;
  }>(),
  { previewHeight: 160, follow: false, textClass: "" },
);

const { t } = useI18n();

const bodyRef = ref<HTMLElement | null>(null);
const overflowing = ref(false);
const expanded = ref(false);
const contentHeight = ref(0);

const clamped = computed(() => !props.follow && !expanded.value);

const shellStyle = computed(() => {
  if (props.follow) return undefined;
  const vars = { "--ecall-preview-height": `${props.previewHeight}px` } as Record<string, string>;
  if (clamped.value) {
    return { ...vars, maxHeight: `${props.previewHeight}px` } as any;
  }
  if (expanded.value) {
    const h = contentHeight.value > 0 ? `${contentHeight.value}px` : "none";
    return { ...vars, maxHeight: h } as any;
  }
  return vars as any;
});

let resizeObserver: ResizeObserver | null = null;

function measure(): void {
  const el = bodyRef.value;
  if (!el) return;
  const sh = el.scrollHeight;
  contentHeight.value = sh;
  overflowing.value = sh > props.previewHeight + 1;
}

watch(
  () => [props.text, props.previewHeight, props.follow] as const,
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
  <div class="ecall-expandable flex min-w-0 flex-col" :class="{ 'ecall-expandable--expanded': expanded, 'ecall-expandable--clamped': clamped }">
    <div
      class="ecall-expandable__shell"
      :style="shellStyle"
    >
      <div class="ecall-expandable__content">
        <div
          ref="bodyRef"
          class="whitespace-pre-wrap wrap-break-word text-xs leading-relaxed"
          :class="[props.textClass, { 'ecall-expandable-text-clamped': clamped && overflowing }]"
        >{{ text }}</div>
      </div>
    </div>
    <div v-if="overflowing" class="mt-0.5">
      <button
        v-if="!expanded"
        type="button"
        class="inline-flex items-center text-xs text-base-content/45 hover:text-base-content/80"
        data-selection-ignore="true"
        @click.stop="expanded = true"
      >
        {{ t("common.expand") }}
      </button>
      <button
        v-else
        type="button"
        class="inline-flex items-center text-xs text-base-content/45 hover:text-base-content/80"
        data-selection-ignore="true"
        @click.stop="expanded = false"
      >
        {{ t("common.collapse") }}
      </button>
    </div>
  </div>
</template>
