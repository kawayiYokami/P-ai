<template>
  <OverlayScrollArea
    class="multi-file-diff-view flex h-full min-h-0 flex-col"
    :class="{ 'multi-file-diff-view--dark': isDark }"
    orientation="both"
    variant="code-dark"
    scroller-class="multi-file-diff-scroller"
  >
    <div class="flex flex-col">
      <div
        v-for="section in sections"
        :key="section.key"
        class="file-section flex flex-col"
      >
        <button
          type="button"
          class="file-section-header sticky top-0 z-10 flex w-full items-center gap-2 bg-base-300 px-3 py-1.5 text-left hover:bg-base-300/80"
          title="左键折叠当前文件，右键折叠全部"
          @click="toggle(section.key)"
          @contextmenu.prevent="collapseAll"
        >
          <AppIcon name="chevron-right" class="h-3.5 w-3.5 shrink-0 opacity-60 transition-transform" :class="{ 'rotate-90': isExpanded(section.key) }" />
          <span class="min-w-0 flex-1 truncate font-mono text-xs font-medium">{{ section.path }}</span>
          <span v-if="section.isBinary" class="badge badge-ghost badge-xs">二进制</span>
          <template v-else>
            <span class="mf-stat mf-add shrink-0 font-mono text-xs font-bold tabular-nums">+{{ section.added }}</span>
            <span class="mf-stat mf-remove shrink-0 font-mono text-xs font-bold tabular-nums">-{{ section.removed }}</span>
          </template>
          <span v-if="section.isRename" class="badge badge-ghost badge-xs shrink-0">重命名</span>
          <span
            class="btn btn-ghost btn-xs h-5 min-h-5 w-5 shrink-0 px-0"
            title="打开源文件"
            @click.stop="emit('openFile', { path: section.path })"
          >
            <AppIcon name="file-text" class="h-3 w-3" />
          </span>
        </button>
        <div v-if="isExpanded(section.key)" class="file-section-body bg-base-100">
          <div v-if="section.isBinary" class="px-3 py-3 text-xs opacity-60">二进制文件，无法显示差异</div>
          <ToolReviewCodePreview
            v-else
            :code="section.diffText"
            mode="patch"
            :is-dark="isDark"
            :show-line-numbers="true"
            :context-lines="sectionContext(section.key)"
            embedded
            @expand-gap="onExpandGap(section)"
          />
        </div>
      </div>
      <div v-if="sections.length === 0" class="px-3 py-6 text-center text-xs opacity-50">无差异内容</div>
    </div>
  </OverlayScrollArea>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import ToolReviewCodePreview from "./ToolReviewCodePreview.vue";
import { parseMultiFileDiff, type FileDiffSection } from "../../file-reader/utils/multiFileDiff";

const props = defineProps<{
  diffText: string;
  isDark: boolean;
  contextLines: number;
}>();

const emit = defineEmits<{
  (e: "expandFileGap", payload: { path: string; nextContext: number }): void;
  (e: "openFile", payload: { path: string }): void;
}>();

const sections = computed(() => parseMultiFileDiff(props.diffText));

const expandedKeys = ref<Set<string>>(new Set());
const sectionContexts = ref<Record<string, number>>({});

const defaultExpandedKeys = computed(() => {
  const secs = sections.value;
  const set = new Set<string>();
  if (secs.length === 0) return set;
  let cum = 0;
  for (let i = 0; i < secs.length; i += 1) {
    const s = secs[i];
    if (i === 0) {
      set.add(s.key);
      cum += s.lineCount;
      continue;
    }
    if (cum >= 100) break;
    set.add(s.key);
    cum += s.lineCount;
  }
  return set;
});

watch(
  sections,
  (newSecs) => {
    if (newSecs.length === 0) {
      expandedKeys.value = new Set();
      return;
    }
    if (expandedKeys.value.size === 0) {
      expandedKeys.value = new Set(defaultExpandedKeys.value);
      return;
    }
    const newKeys = new Set(newSecs.map((s) => s.key));
    const hasOverlap = Array.from(expandedKeys.value).some((k) => newKeys.has(k));
    if (!hasOverlap) {
      expandedKeys.value = new Set(defaultExpandedKeys.value);
    }
  },
  { immediate: true },
);

function isExpanded(key: string) {
  return expandedKeys.value.has(key);
}

function toggle(key: string) {
  const next = new Set(expandedKeys.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expandedKeys.value = next;
}

function collapseAll() {
  if (expandedKeys.value.size === 0) return;
  expandedKeys.value = new Set();
}

function sectionContext(key: string) {
  return sectionContexts.value[key] ?? Math.max(1, props.contextLines || 3);
}

function onExpandGap(section: FileDiffSection) {
  const cur = sectionContexts.value[section.key] ?? Math.max(1, props.contextLines || 3);
  const next = cur + 30;
  sectionContexts.value = { ...sectionContexts.value, [section.key]: next };
  emit("expandFileGap", { path: section.path, nextContext: next });
}
</script>

<style scoped>
.multi-file-diff-view {
  --mf-add-fg: #1a7f37;
  --mf-add-bg: #e6ffec;
  --mf-remove-fg: #cf222e;
  --mf-remove-bg: #ffebe9;
}
.multi-file-diff-view--dark {
  --mf-add-fg: #3fb950;
  --mf-add-bg: rgba(46, 160, 67, 0.15);
  --mf-remove-fg: #f85149;
  --mf-remove-bg: rgba(248, 81, 73, 0.15);
}
.mf-stat {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 2rem;
  padding: 0 0.35rem;
  border-radius: 9999px;
  line-height: 1.4;
}
.mf-add {
  color: var(--mf-add-fg);
  background: var(--mf-add-bg);
}
.mf-remove {
  color: var(--mf-remove-fg);
  background: var(--mf-remove-bg);
}
.file-section-header {
  user-select: none;
  border: none;
}
.file-section-body :deep(.tool-review-code-main) {
  flex: none;
  background: var(--color-base-100);
}
/* embedded 模式由 ToolReviewCodePreview 的 embedded prop 控制为 horizontal，已是高度自适应；此处仅修正背景与 shiki 高度 */
.file-section-body :deep(.tool-review-code-view) {
  background: var(--color-base-100);
}
.file-section-body :deep(.tool-review-raw-pre) {
  background: var(--color-base-100);
}
.file-section-body :deep(.tool-review-code-view .shiki) {
  min-height: auto;
  padding: 0 !important;
  background: var(--color-base-100) !important;
}
.file-section-body :deep(.tool-review-code-view .line:first-child) {
  margin-top: 0;
}
.file-section-body :deep(.tool-review-code-view .line:last-child) {
  margin-bottom: 0;
}
/* 隐藏行折叠条改用 base-200，正文 base-100，文件头 base-300 已在模板层 */
.file-section-body :deep(.tool-review-code-view .line.tool-review-gap-bar) {
  background: var(--color-base-200);
  border-top-color: var(--color-base-300);
  border-bottom-color: var(--color-base-300);
}
.file-section-body :deep(.tool-review-code-view .line.tool-review-gap-bar:hover) {
  background: var(--color-base-200);
  color: var(--color-primary);
}
</style>
