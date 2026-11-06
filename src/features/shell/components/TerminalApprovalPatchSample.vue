<script setup lang="ts">
import { computed, ref, watch } from "vue";
import AppIcon from "../../shared/components/AppIcon.vue";
import { getSingletonHighlighter, hastToHtml } from "shiki";
import { isDarkAppTheme, useAppTheme } from "../composables/use-app-theme";
import { countTerminalApprovalPatchDelta, getTerminalApprovalPatchKind } from "../utils/terminal-approval-preview";

type ApprovalLineKind = "add" | "remove" | "context" | "warning" | "normal";
type ParsedLine = {
  line: string;
  kind: ApprovalLineKind;
  oldPrefix: string;
  newPrefix: string;
  marker: string;
};

type ShikiHastNode = {
  type: string;
  tagName?: string;
  properties?: Record<string, unknown>;
  children?: ShikiHastNode[];
};

type ShikiHastElement = ShikiHastNode & {
  type: "element";
  tagName: string;
  properties: Record<string, unknown>;
  children: ShikiHastNode[];
};

const props = withDefaults(defineProps<{
  lines: string[];
  diffOnly?: boolean;
  showPrefixes?: boolean;
  title?: string;
  collapsed?: boolean;
  lang?: string;
  hideHeader?: boolean;
  embedded?: boolean;
}>(), {
  diffOnly: true,
  showPrefixes: true,
  title: "",
  collapsed: undefined,
  lang: "",
  hideHeader: false,
  embedded: false,
});

const emit = defineEmits<{
  "update:collapsed": [value: boolean];
  "collapse-all": [];
}>();

const localCollapsed = ref(false);
const isCollapsed = computed({
  get: () => props.collapsed ?? localCollapsed.value,
  set: (value: boolean) => {
    localCollapsed.value = value;
    emit("update:collapsed", value);
  },
});

function toggleCollapsed() {
  isCollapsed.value = !isCollapsed.value;
}

function requestCollapseAll(event: MouseEvent) {
  event.preventDefault();
  emit("collapse-all");
}

const { currentTheme } = useAppTheme();
const isDark = computed(() => isDarkAppTheme(currentTheme.value));

function getPatchLineNumbers(hunkLine: string) {
  const match = hunkLine.match(/^@@\s+-([0-9]+)(?:,[0-9]+)?\s+\+([0-9]+)(?:,[0-9]+)?\s+@@/);
  if (!match) {
    return null;
  }
  return {
    oldLine: Number.parseInt(match[1], 10),
    newLine: Number.parseInt(match[2], 10),
  };
}

const parsedLines = computed<ParsedLine[]>(() => {
  const rawLines = props.lines.map((item) => String(item || "").replace(/\r/g, ""));
  const oldLineRef = { value: 0 };
  const newLineRef = { value: 0 };
  const diffOnly = props.diffOnly !== false;
  return rawLines.flatMap<ParsedLine>((line) => {
    if (line.startsWith("@@")) {
      const range = getPatchLineNumbers(line);
      if (range) {
        oldLineRef.value = range.oldLine;
        newLineRef.value = range.newLine;
      }
      return [];
    }

    if (
      line.startsWith("*** Begin Patch")
      || line.startsWith("*** End Patch")
      || line.startsWith("*** Update File:")
      || line.startsWith("*** Add File:")
      || line.startsWith("*** Delete File:")
    ) {
      return [];
    }

    if (line.trim() === "Error!") {
      return [{ line, kind: "warning" as const, oldPrefix: "", newPrefix: "", marker: "" }];
    }

    if (line.startsWith("+") && line[1] !== "+") {
      const newPrefix = newLineRef.value > 0 ? String(newLineRef.value) : "";
      newLineRef.value += 1;
      return [{ line: line.slice(1), kind: "add" as const, oldPrefix: "", newPrefix, marker: "+" }];
    }

    if (line.startsWith("-") && line[1] !== "-") {
      const oldPrefix = oldLineRef.value > 0 ? String(oldLineRef.value) : "";
      oldLineRef.value += 1;
      return [{ line: line.slice(1), kind: "remove" as const, oldPrefix, newPrefix: "", marker: "-" }];
    }

    const inHunk = oldLineRef.value > 0 || newLineRef.value > 0;
    const oldPrefix = inHunk && oldLineRef.value > 0 ? String(oldLineRef.value) : "";
    const newPrefix = inHunk && newLineRef.value > 0 ? String(newLineRef.value) : "";
    if (inHunk) {
      oldLineRef.value += 1;
      newLineRef.value += 1;
    }

    if (diffOnly) {
      return [];
    }

    return [{ line, kind: "context" as const, oldPrefix, newPrefix, marker: "" }];
  });
});

const kindLabel = computed(() => {
  const explicit = String(props.title || "").trim();
  if (explicit) return explicit;
  const kind = getTerminalApprovalPatchKind(props.lines);
  if (kind === "add") return "新增";
  if (kind === "delete") return "删除";
  if (kind === "update") return "修改";
  return "改动";
});

const delta = computed(() => countTerminalApprovalPatchDelta(props.lines));

const showTitleBar = computed(() => !props.hideHeader && props.showPrefixes !== false);

// ==================== shiki 行级高亮 ====================
// 保留自定义解析（行号/标记/底色），仅把 code 文字换成语法高亮 HTML。
// 逐行 codeToHast 会丢失跨行语法状态，但 diff 卡按行展示本就无连续状态，可接受。

const highlightedLineHtml = ref<Record<number, string>>({});
let highlightRequestId = 0;
let highlighterPromise: Promise<Awaited<ReturnType<typeof getSingletonHighlighter>>> | null = null;

function escapeHtml(value: string) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function hastLineHtml(root: ShikiHastNode): string {
  const pre = root.children?.find((node): node is ShikiHastElement => node.type === "element" && node.tagName === "pre");
  const code = pre?.children?.find((node): node is ShikiHastElement => node.type === "element" && node.tagName === "code");
  const line = code?.children?.find((node): node is ShikiHastElement => (
    node.type === "element" && node.tagName === "span" && String(node.properties?.class || "").split(/\s+/).includes("line")
  ));
  if (!line) return "";
  return hastToHtml({ type: "root", children: line.children || [] } as Parameters<typeof hastToHtml>[0]) || "";
}

async function highlightParsedLines() {
  const requestId = ++highlightRequestId;
  const language = String(props.lang || "").trim().toLowerCase();
  const target = parsedLines.value;
  if (!language || target.length === 0 || isCollapsed.value) {
    if (requestId === highlightRequestId) highlightedLineHtml.value = {};
    return;
  }
  const theme = isDark.value ? "github-dark" : "github-light";
  if (!highlighterPromise) {
    highlighterPromise = getSingletonHighlighter({ langs: [language], themes: [theme] });
  }
  try {
    const highlighter = await highlighterPromise;
    if (requestId !== highlightRequestId) return;
    const next: Record<number, string> = {};
    await Promise.all(target.map(async (item, index) => {
      if (item.kind === "warning") {
        next[index] = escapeHtml(item.line);
        return;
      }
      try {
        const root = await highlighter.codeToHast(item.line || " ", { lang: language, theme });
        const html = hastLineHtml(root);
        next[index] = html || escapeHtml(item.line);
      } catch {
        next[index] = escapeHtml(item.line);
      }
    }));
    if (requestId !== highlightRequestId) return;
    highlightedLineHtml.value = next;
  } catch {
    if (requestId === highlightRequestId) highlightedLineHtml.value = {};
  }
}

watch(
  () => [parsedLines.value, props.lang, isDark.value, isCollapsed.value] as const,
  () => {
    void highlightParsedLines();
  },
  { immediate: true },
);

function lineHtml(index: number, fallback: string) {
  return highlightedLineHtml.value[index] ?? escapeHtml(fallback);
}
</script>

<template>
  <div
    class="approval-patch-sample flex w-full flex-col"
    :class="{ 'approval-patch-sample--dark': isDark, 'approval-patch-sample--embedded': embedded }"
  >
    <div
      v-if="showTitleBar"
      class="approval-patch-sample__title"
      role="button"
      tabindex="0"
      :title="isCollapsed ? '展开' : '折叠（右键折叠同组）'"
      @click="toggleCollapsed"
      @contextmenu="requestCollapseAll"
      @keydown.enter.prevent="toggleCollapsed"
      @keydown.space.prevent="toggleCollapsed"
    >
      <AppIcon name="chevron-right"
        class="approval-patch-sample__title-chevron"
        :class="{ 'approval-patch-sample__title-chevron--open': !isCollapsed }"
      />
      <span class="approval-patch-sample__title-kind">{{ kindLabel }}</span>
      <span class="approval-patch-sample__title-delta">
        <span class="approval-patch-sample__title-delta-add">+{{ delta.adds }}</span>
        <span class="approval-patch-sample__title-delta-remove">-{{ delta.removes }}</span>
      </span>
    </div>
    <div v-if="!isCollapsed" class="approval-patch-sample__body">
      <div
        v-for="(item, idx) in parsedLines"
        :key="idx"
        class="approval-patch-sample__row"
        :class="`approval-patch-sample__row--${item.kind}`"
      >
        <span v-if="showPrefixes" class="approval-patch-sample__old">{{ item.oldPrefix }}</span>
        <span v-if="showPrefixes" class="approval-patch-sample__new">{{ item.newPrefix }}</span>
        <span v-if="showPrefixes" class="approval-patch-sample__marker">{{ item.marker }}</span>
        <code class="approval-patch-sample__code" v-html="lineHtml(idx, item.line)"></code>
      </div>
      <div v-if="parsedLines.length === 0" class="approval-patch-sample__row approval-patch-sample__row--normal">
        <span v-if="showPrefixes" class="approval-patch-sample__old" />
        <span v-if="showPrefixes" class="approval-patch-sample__new" />
        <span v-if="showPrefixes" class="approval-patch-sample__marker" />
        <code class="approval-patch-sample__code">（无内容）</code>
      </div>
    </div>
  </div>
</template>

<style scoped>
.approval-patch-sample {
  --ap-warning-bg: rgba(187, 128, 9, 0.18);
  --ap-muted: #94a3b8;
  background-color: var(--color-base-100);
  border: 1px solid var(--color-base-300);
  border-radius: 0.5rem;
  overflow: hidden;
  font-family: var(--app-code-font-family, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
  font-size: 0.75rem;
  line-height: 1.6;
}

/* 浅色主题（GitHub light diff 配色） */
.approval-patch-sample {
  --ap-add-bg: #e6ffec;
  --ap-add-fg: #1a7f37;
  --ap-remove-bg: #ffebe9;
  --ap-remove-fg: #cf222e;
}

/* 深色主题（GitHub dark diff 配色） */
.approval-patch-sample--dark {
  --ap-add-bg: rgba(46, 160, 67, 0.15);
  --ap-add-fg: #3fb950;
  --ap-remove-bg: rgba(248, 81, 73, 0.15);
  --ap-remove-fg: #f85149;
}

.approval-patch-sample--embedded {
  background: transparent;
  border: 0;
  border-radius: 0;
  overflow: visible;
}

.approval-patch-sample__title {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  padding: 0.4rem 0.75rem;
  color: var(--color-base-content);
  font-size: 0.6875rem;
  cursor: pointer;
  user-select: none;
}

.approval-patch-sample__title:hover {
  background-color: color-mix(in srgb, var(--color-base-300) 40%, transparent);
}

.approval-patch-sample__title-chevron {
  width: 0.85rem;
  height: 0.85rem;
  flex-shrink: 0;
  align-self: center;
  color: var(--ap-muted);
  transition: transform 140ms ease-out;
}

.approval-patch-sample__title-chevron--open {
  transform: rotate(90deg);
}

.approval-patch-sample__title-kind {
  font-weight: 600;
  color: var(--color-base-content);
}

.approval-patch-sample__title-delta {
  display: inline-flex;
  align-items: baseline;
  gap: 0.3rem;
  font-variant-numeric: tabular-nums;
}

.approval-patch-sample__title-delta-add {
  color: var(--ap-add-fg);
}

.approval-patch-sample__title-delta-remove {
  color: var(--ap-remove-fg);
}

.approval-patch-sample__row {
  display: grid;
  grid-template-columns: minmax(1.5rem, auto) minmax(1.5rem, auto) 1.25rem minmax(0, 1fr);
  align-items: baseline;
  padding: 0 0.5rem;
}

.approval-patch-sample__old,
.approval-patch-sample__new {
  text-align: right;
  color: var(--ap-muted);
  user-select: none;
  white-space: nowrap;
  padding-right: 0.35rem;
}

.approval-patch-sample__marker {
  text-align: center;
  user-select: none;
  white-space: nowrap;
  padding-right: 0.5rem;
}

.approval-patch-sample__code {
  font-family: inherit;
  color: var(--color-base-content);
  white-space: pre-wrap;
  word-break: break-all;
  overflow-wrap: anywhere;
}

.approval-patch-sample__row--add {
  background-color: var(--ap-add-bg);
}

.approval-patch-sample__row--add .approval-patch-sample__code {
  color: var(--ap-add-fg);
}

.approval-patch-sample__row--add .approval-patch-sample__marker {
  color: var(--ap-add-fg);
  font-weight: 600;
}

.approval-patch-sample__row--remove {
  background-color: var(--ap-remove-bg);
}

.approval-patch-sample__row--remove .approval-patch-sample__code {
  color: var(--ap-remove-fg);
}

.approval-patch-sample__row--remove .approval-patch-sample__marker {
  color: var(--ap-remove-fg);
  font-weight: 600;
}

.approval-patch-sample__row--warning {
  background-color: var(--ap-warning-bg);
}

.approval-patch-sample__row--normal {
  color: var(--ap-muted);
}
</style>
