<template>
  <div ref="wrapperRef" class="ecall-shiki-codeblock markstream-vue ecall-shiki-codeblock-dark">
    <MarkdownCodeBlockNode v-bind="resolvedProps" />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { MarkdownCodeBlockNode } from "markstream-vue";
import type { CodeBlockNodeProps } from "markstream-vue";

const props = defineProps<CodeBlockNodeProps>();
const wrapperRef = ref<HTMLElement | null>(null);
const canExpand = ref(false);
let refreshTimer: ReturnType<typeof setTimeout> | null = null;
let resizeObserver: ResizeObserver | null = null;
let pendingResizeFrame = 0;

const resolvedNode = computed(() => {
  if (!props.node?.diff) return props.node;
  return {
    ...props.node,
    language: "diff",
    code: String(props.node.raw ?? ""),
    diff: false,
    originalCode: undefined,
    updatedCode: undefined,
  };
});

function resolveCollapsedMaxHeightPx(contentEl: HTMLElement): number | null {
  if (typeof window === "undefined") return null;
  const computedStyle = window.getComputedStyle(contentEl);
  const rawMaxHeight = String(computedStyle.maxHeight || "").trim().toLowerCase();
  if (!rawMaxHeight || rawMaxHeight === "none") return null;
  const parsed = Number.parseFloat(rawMaxHeight);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

function refreshExpandAvailability() {
  const wrapperEl = wrapperRef.value;
  if (!wrapperEl) return;
  const contentEl = wrapperEl.querySelector<HTMLElement>(".code-block-content");
  if (!contentEl) return;
  const collapsedMaxHeightPx = resolveCollapsedMaxHeightPx(contentEl);
  if (!collapsedMaxHeightPx) {
    canExpand.value = false;
    return;
  }
  canExpand.value = contentEl.scrollHeight > collapsedMaxHeightPx + 1;
}

function scheduleResizeDrivenRefresh() {
  if (typeof window === "undefined") {
    refreshExpandAvailability();
    return;
  }
  if (pendingResizeFrame) return;
  pendingResizeFrame = window.requestAnimationFrame(() => {
    pendingResizeFrame = 0;
    refreshExpandAvailability();
  });
}

function scheduleExpandAvailabilityRefresh() {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
  void nextTick(() => {
    refreshExpandAvailability();
    if (typeof window !== "undefined") {
      window.requestAnimationFrame(() => {
        refreshExpandAvailability();
      });
    }
    refreshTimer = setTimeout(() => {
      refreshExpandAvailability();
      refreshTimer = null;
    }, 120);
  });
}

const resolvedProps = computed(() => {
  const { themes: _themes, ...rest } = props as CodeBlockNodeProps & { themes?: unknown };
  return {
    ...rest,
    node: resolvedNode.value,
    showExpandButton: Boolean(rest.showExpandButton && canExpand.value),
    darkTheme: "github-dark",
    lightTheme: "github-dark",
    isDark: true,
    stream: false,
    loading: false,
  };
});

onMounted(() => {
  if (typeof ResizeObserver !== "undefined" && wrapperRef.value) {
    resizeObserver = new ResizeObserver(() => {
      scheduleResizeDrivenRefresh();
    });
    resizeObserver.observe(wrapperRef.value);
  }
});

watch(
  () => [props.node?.code, props.node?.raw, props.node?.language],
  () => {
    scheduleExpandAvailabilityRefresh();
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
  if (pendingResizeFrame && typeof window !== "undefined") {
    window.cancelAnimationFrame(pendingResizeFrame);
    pendingResizeFrame = 0;
  }
  resizeObserver?.disconnect();
  resizeObserver = null;
});
</script>

<style scoped>
.ecall-shiki-codeblock {
  min-width: 0;
}

.ecall-shiki-codeblock-dark {
  --code-fg: #e5e7eb;
  --code-action-fg: #475569;
  --code-action-hover-fg: #0f172a;
  --code-line-number: #64748b;
  --vscode-editor-foreground: #e5e7eb;
  --vscode-foreground: #e5e7eb;
  color: #e5e7eb;
}

.ecall-shiki-codeblock-dark :deep(.code-block-header),
.ecall-shiki-codeblock-dark :deep(.code-header-main),
.ecall-shiki-codeblock-dark :deep(.code-header-title),
.ecall-shiki-codeblock-dark :deep(.code-header-caption),
.ecall-shiki-codeblock-dark :deep(.code-header-actions),
.ecall-shiki-codeblock-dark :deep(.code-block-header .icon-slot) {
  color: #475569 !important;
}

.ecall-shiki-codeblock-dark :deep(.code-block-header button:hover),
.ecall-shiki-codeblock-dark :deep(.code-block-header [role="button"]:hover) {
  color: #0f172a !important;
}

.ecall-shiki-codeblock-dark :deep(.code-block-container),
.ecall-shiki-codeblock-dark :deep(.code-block-content),
.ecall-shiki-codeblock-dark :deep(.code-block-render),
.ecall-shiki-codeblock-dark :deep(.code-fallback-plain),
.ecall-shiki-codeblock-dark :deep(pre.code-pre-fallback) {
  color: #e5e7eb !important;
}

.ecall-shiki-codeblock-dark :deep(.code-block-content),
.ecall-shiki-codeblock-dark :deep(.code-block-render),
.ecall-shiki-codeblock-dark :deep(.code-fallback-plain),
.ecall-shiki-codeblock-dark :deep(pre.code-pre-fallback) {
  background: #101828 !important;
}

.ecall-shiki-codeblock-dark :deep(pre),
.ecall-shiki-codeblock-dark :deep(.shiki) {
  color: #e5e7eb !important;
  background: #101828 !important;
}

.ecall-shiki-codeblock-dark :deep(.code-block-content) {
  border-top: 0 !important;
  padding-top: 0 !important;
}

.ecall-shiki-codeblock-dark :deep(.code-block-content > :first-child),
.ecall-shiki-codeblock-dark :deep(.code-block-render > :first-child) {
  margin-top: 0 !important;
}

.ecall-shiki-codeblock-dark :deep(.shiki span),
.ecall-shiki-codeblock-dark :deep(.line),
.ecall-shiki-codeblock-dark :deep(.line span) {
  color: inherit;
}

</style>
