/**
 * 工具参数的结构化渲染。
 *
 * 输入是模型的原始参数文本：
 * - 能解析成 JSON：摊成字段行，键名红、值工具绿，嵌套靠缩进，
 *   去掉全部 JSON 语法字符（大括号、方括号、键名引号、逗号）。
 * - 不是 JSON（exec 的裸命令、流式半截 JSON）：退回纯文本，原样整块输出。
 *
 * 配色在这一层定：参数区的绿是它自己的语义，不跟随条目层的 context 灰。
 */
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { breakCommandLines, isCommandArgLabel, parseToolArgs, type ToolArgLine } from "../utils/tool-args-tree";
import { highlightShellCommand } from "../utils/shell-highlight";

const props = defineProps<{
  text: string;
  /** 深色主题下工具绿要提亮，否则压在深底上看不清 */
  dark?: boolean;
}>();

/** 每一级缩进。用 ch 而不是 px：破折号占两格，缩进也对齐两格，子字段才能和它首行标签落在同一条竖线上。 */
const INDENT_UNIT = "2ch";

/** 解析结果按文本缓存：流式期间同一段参数只解析一次，不同条目内容相同也能复用。 */
const CACHE_LIMIT = 24;
/** 超长参数不进缓存，省内存；它们本来也只渲染一次。 */
const CACHE_MAX_TEXT = 8000;
const lineCache = new Map<string, ToolArgLine[] | null>();

function readLines(text: string): ToolArgLine[] | null {
  if (text.length > CACHE_MAX_TEXT) return parseToolArgs(text);
  if (lineCache.has(text)) {
    const hit = lineCache.get(text) ?? null;
    lineCache.delete(text);
    lineCache.set(text, hit);
    return hit;
  }
  const lines = parseToolArgs(text);
  lineCache.set(text, lines);
  if (lineCache.size > CACHE_LIMIT) {
    const oldest = lineCache.keys().next();
    if (!oldest.done) lineCache.delete(oldest.value);
  }
  return lines;
}

const source = computed(() => String(props.text ?? ""));
const lines = computed(() => readLines(source.value));

/** 命令行按操作符折行显示；其他字段原样。 */
function valueText(line: ToolArgLine): string {
  if (!line.value) return "";
  if (!isCommandArgLabel(line.label)) return line.value;
  return breakCommandLines(line.value);
}

/** 命令行的高亮结果，按行号存；shiki 是异步的，没就绪前这段为空，模板回退纯文本。 */
const highlightedCommands = ref<Record<number, string>>({});

watch(
  [lines, () => props.dark],
  async () => {
    const snapshot = lines.value;
    if (!snapshot) {
      highlightedCommands.value = {};
      return;
    }
    const next: Record<number, string> = {};
    for (let index = 0; index < snapshot.length; index += 1) {
      const line = snapshot[index];
      if (!line?.value || !isCommandArgLabel(line.label)) continue;
      const html = await highlightShellCommand(breakCommandLines(line.value), !!props.dark);
      if (html) next[index] = html;
    }
    // 异步返回时参数可能已被新的流式内容替换，只有快照仍然有效才提交
    if (lines.value === snapshot) highlightedCommands.value = next;
  },
  { immediate: true },
);

function indentStyle(line: ToolArgLine): Record<string, string> {
  return { paddingLeft: `calc(${INDENT_UNIT} * ${line.depth})` };
}
</script>

<template>
  <div class="ecall-tool-args-body" :class="{ 'ecall-tool-args-body-dark': props.dark }">
    <div v-if="lines" class="flex flex-col gap-0.5">
      <div
        v-for="(line, index) in lines"
        :key="index"
        class="ecall-tool-args-line flex items-baseline"
        :class="{ 'ecall-tool-args-hint': line.hint }"
        :style="indentStyle(line)"
      >
        <span v-if="line.prefix" class="ecall-tool-args-prefix shrink-0">{{ line.prefix }}</span>
        <span v-if="line.label" class="ecall-tool-arg-key shrink-0">{{ line.label }}<span class="ecall-tool-arg-colon">:</span></span>
        <span v-if="line.value" class="min-w-0 whitespace-pre-wrap break-all">
          <span v-if="highlightedCommands[index]" v-html="highlightedCommands[index]" />
          <template v-else>{{ valueText(line) }}</template>
        </span>
      </div>
    </div>
    <span v-else class="whitespace-pre-wrap break-all">{{ breakCommandLines(source) }}</span>
  </div>
</template>

<style scoped>
/* 参数区整体：工具绿。条目那一层是 context 60% 灰，两层不同色是故意的 */
.ecall-tool-args-body {
  color: #15803d;
}

.ecall-tool-args-body-dark {
  color: #4ade80;
}

/* 破折号总占位固定两格（扣掉自己那 6px 间距），下一级缩进正好两格，子字段标签才能和它首行对齐 */
.ecall-tool-args-prefix {
  width: calc(2ch - 6px);
  margin-right: 6px;
  opacity: 0.7;
}

/* 键名沿用行内代码那套红，无背景 */
.ecall-tool-arg-key {
  margin-right: 6px;
  color: var(--ecall-md-inline-code-color);
}

.ecall-tool-arg-colon {
  color: var(--ecall-md-inline-code-color);
}

/* 截断标注：弱化，单独一行，避免把长值挤断 */
.ecall-tool-args-hint {
  opacity: 0.6;
  font-size: 0.9em;
}
</style>
