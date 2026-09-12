<template>
  <div v-if="visible" class="pointer-events-none flex justify-start px-2">
    <button
      type="button"
      class="ecall-thinking-preview-bar pointer-events-auto w-fit max-w-[min(720px,100%)] cursor-pointer rounded-2xl border border-base-300/50 bg-base-100/55 px-3 py-2 text-left text-xs shadow-sm backdrop-blur-md backdrop-saturate-150 transition-colors hover:bg-base-100/75"
      @click="emit('jumpToBottom')"
    >
      <div v-if="streaming && reasoningWindowLines.length > 0" class="block max-h-20 max-w-[min(720px,100%)] overflow-hidden">
        <TransitionGroup
          :name="lineTransitionName"
          tag="div"
          class="relative flex flex-col"
        >
          <span
            v-for="line in reasoningWindowLines"
            :key="line.key"
            class="block truncate leading-5 text-base-content/55"
          >{{ line.text }}</span>
        </TransitionGroup>
      </div>
      <div v-if="answerLine" class="block max-w-[min(720px,100%)] overflow-hidden" :class="reasoningWindowLines.length > 0 ? 'mt-1' : ''">
        <Transition :name="lineTransitionName" mode="out-in">
          <span :key="answerLine" class="flex min-w-0 items-center gap-1.5">
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              alt=""
              class="h-4 w-4 shrink-0 rounded-full object-cover"
            />
            <span class="truncate leading-5 text-base-content/85">{{ answerLine }}</span>
            <ArrowDownToLine class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
          </span>
        </Transition>
      </div>
      <span
        v-if="!hasVisibleContent"
        class="flex items-center gap-1 leading-5 text-base-content/70"
      >
        <ArrowDownToLine class="h-3.5 w-3.5" />
        {{ t("chat.jumpToBottom") }}
      </span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDownToLine } from "@lucide/vue";
import { stripToolcallMarkers } from "../../../utils/chat-message-semantics";

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    /** 当前回合的内容块序列，按输出顺序排列 */
    blocks: Array<{ reasoning?: string; text?: string }>;
    /** 思维链预览行数 */
    lines?: number;
    /** 单行最大字符数，超出只保留末尾 */
    maxChars?: number;
    /** 整条是否显示：由父组件按「是否离开底部」决定 */
    visible?: boolean;
    /** 当前回合是否正在流式输出；非流式时收起思维链，只显示最新一行正文 */
    streaming?: boolean;
    /** 非流式预览正文：取最新一条助理消息的正文，由父组件给定 */
    idleText?: string;
    /** 正文行前的头像（最新一条助理消息的人格头像） */
    avatarUrl?: string;
  }>(),
  {
    lines: 4,
    maxChars: 96,
    visible: true,
    streaming: true,
    idleText: "",
    avatarUrl: "",
  },
);

const emit = defineEmits<{ jumpToBottom: [] }>();

// 阅读速度：中文每秒 12 字，英文每秒 5 个词（按空格切分）
const CJK_CHARS_PER_SECOND = 12;
const ENGLISH_WORDS_PER_SECOND = 5;
// 推进粒度：每 100ms 结算一次阅读进度
const TICK_MS = 100;
// 单行最短停留，避免短行一闪而过
const MIN_LINE_MS = 300;

const lineTransitionName = "ecall-thinking-line";

function nonEmptyLines(text: string): string[] {
  return String(text || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function lastNonEmptyLine(text: string): string {
  const lines = nonEmptyLines(text);
  return lines.length > 0 ? lines[lines.length - 1] : "";
}

// 「工具后新正文」分段占位符，与 chat-message-semantics 的时间线预览口径一致
const TOOL_TEXT_BREAK = "\uE000TOOLBREAK\uE000";

// 一条消息的正文可能由多段「工具后新正文」拼接（段间是占位符、不是换行），
// 照抄时间线预览：按占位符切段取最后一段，再剥工具标记、取末行，才是真正的最终正文。
function finalBodyLine(text: string): string {
  const segments = String(text || "")
    .split(TOOL_TEXT_BREAK)
    .map((segment) => segment.trim())
    .filter(Boolean);
  const tail = segments.length > 0 ? segments[segments.length - 1] : "";
  return lastNonEmptyLine(stripToolcallMarkers(tail));
}

// 单行过长只保留末尾，保证看到的永远是"最新"
function windowLine(line: string): string {
  return line.length <= props.maxChars ? line : `…${line.slice(line.length - props.maxChars)}`;
}

// ==================== 节点队列 ====================
// 一个节点 = 一段思维链 + 一段正文。节点内先把思维链按阅读速度逐行走完，
// 等下一个节点出现才认定本节点输出完毕，此时才把正文显示出来。

const activeBlockIndex = ref(0);
const consumedLineCount = ref(0);
const remainingMs = ref(0);
const answerLine = ref("");
let alignedToLatest = false;
let tickTimer: ReturnType<typeof setInterval> | null = null;

const hasVisibleContent = computed(
  () => (props.streaming && reasoningWindowLines.value.length > 0) || !!answerLine.value,
);

const activeReasoningLines = computed(() =>
  nonEmptyLines(props.blocks[activeBlockIndex.value]?.reasoning || ""),
);

// 一行要读多久：汉字数 / 12 + 英文词数 / 5，单位秒
function lineReadDurationMs(line: string): number {
  const cjkCount = (line.match(/[\u3040-\u30ff\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\uac00-\ud7af]/g) || []).length;
  const englishWordCount = line.split(/\s+/).filter((token) => /[a-z]/i.test(token)).length;
  const seconds = cjkCount / CJK_CHARS_PER_SECOND + englishWordCount / ENGLISH_WORDS_PER_SECOND;
  return Math.max(MIN_LINE_MS, seconds * 1000);
}

const windowRange = computed(() => {
  const total = activeReasoningLines.value.length;
  const end = Math.min(consumedLineCount.value, total);
  const start = Math.max(0, end - props.lines);
  return { start, end };
});

const reasoningWindowLines = computed(() =>
  activeReasoningLines.value
    .slice(windowRange.value.start, windowRange.value.end)
    .map((text, index) => ({
      // 行在源文本中的绝对位置作为 key，只有整行进出才产生增删；当前行增长不会触发动画
      key: `${activeBlockIndex.value}:${windowRange.value.start + index}`,
      text: windowLine(text),
    })),
);

function pendingWork(): boolean {
  const total = props.blocks.length;
  if (total === 0) return false;
  if (activeBlockIndex.value < total - 1) return true;
  return consumedLineCount.value < activeReasoningLines.value.length;
}

function stopTicking() {
  if (!tickTimer) return;
  clearInterval(tickTimer);
  tickTimer = null;
}

function commitFinishedAnswer() {
  const text = finalBodyLine(props.blocks[activeBlockIndex.value]?.text || "");
  if (text) answerLine.value = windowLine(text);
}

function advanceTick() {
  const total = props.blocks.length;
  if (activeBlockIndex.value >= total) return;

  const lines = activeReasoningLines.value;
  // 先把本节点的思维链按阅读速度走完
  if (consumedLineCount.value < lines.length) {
    if (remainingMs.value <= 0) {
      remainingMs.value = lineReadDurationMs(lines[consumedLineCount.value] || "");
      consumedLineCount.value += 1;
    }
    remainingMs.value -= TICK_MS;
    return;
  }

  // 思维链读完；只有后面已经出现新节点，才认定本节点输出完毕
  if (activeBlockIndex.value >= total - 1) return;

  commitFinishedAnswer();
  activeBlockIndex.value += 1;
  consumedLineCount.value = 0;
  remainingMs.value = 0;
}

function tick() {
  advanceTick();
  if (!pendingWork()) stopTicking();
}

function ensureTicking() {
  if (tickTimer) return;
  if (!pendingWork()) return;
  tickTimer = setInterval(tick, TICK_MS);
}

// 只在内容增长时触发，避免深度遍历每帧扫全量文本
const blocksFingerprint = computed(() =>
  props.blocks.map((block) => `${block.reasoning?.length ?? 0}:${block.text?.length ?? 0}`).join("|"),
);

// 对齐到最新块：挂载时（含切回历史会话）不重放已有内容
function alignToLatest() {
  const total = props.blocks.length;
  if (total === 0) {
    activeBlockIndex.value = 0;
    consumedLineCount.value = 0;
    return;
  }
  activeBlockIndex.value = total - 1;
  consumedLineCount.value = activeReasoningLines.value.length;
}

// 新一轮开始：从内容开头重新按节奏读
function restartFromBeginning() {
  stopTicking();
  activeBlockIndex.value = 0;
  consumedLineCount.value = 0;
  remainingMs.value = 0;
}

// 非流式：收起思维链，只显示最新一条助理消息的正文末行
function applyIdleAnswer() {
  const source = String(props.idleText || "");
  const text = finalBodyLine(source);
  if (text) {
    answerLine.value = windowLine(text);
    return;
  }
  for (let i = props.blocks.length - 1; i >= 0; i -= 1) {
    const fallback = finalBodyLine(props.blocks[i]?.text || "");
    if (fallback) {
      answerLine.value = windowLine(fallback);
      return;
    }
  }
  answerLine.value = "";
}

// 挂载对齐时补一段已完成节点的正文
function applyFinishedAnswer() {
  for (let i = props.blocks.length - 2; i >= 0; i -= 1) {
    const text = finalBodyLine(props.blocks[i]?.text || "");
    if (text) {
      answerLine.value = windowLine(text);
      return;
    }
  }
}

watch(
  [blocksFingerprint, () => props.streaming],
  ([, streaming], [, prevStreaming]) => {
    if (!alignedToLatest) {
      alignedToLatest = true;
      if (streaming) {
        alignToLatest();
        applyFinishedAnswer();
      } else {
        applyIdleAnswer();
      }
      return;
    }
    if (!streaming) {
      stopTicking();
      applyIdleAnswer();
      return;
    }
    if (prevStreaming === false) {
      restartFromBeginning();
    } else if (activeBlockIndex.value >= props.blocks.length) {
      alignToLatest();
    }
    ensureTicking();
  },
  { immediate: true },
);

watch(
  () => props.idleText,
  () => {
    if (!props.streaming) applyIdleAnswer();
  },
);

onBeforeUnmount(stopTicking);
</script>

<style scoped>
/* 逐行滚动：新行从底部推入、走过的行向上顶出，其余行平滑位移 */
.ecall-thinking-line-enter-active,
.ecall-thinking-line-leave-active,
.ecall-thinking-line-move {
  transition: transform 0.24s ease, opacity 0.24s ease;
}

.ecall-thinking-line-enter-from {
  transform: translateY(1.25rem);
  opacity: 0;
}

.ecall-thinking-line-leave-to {
  transform: translateY(-1.25rem);
  opacity: 0;
}

/* 离场行脱离文档流，避免它占位挡住其余行的位移动画 */
.ecall-thinking-line-leave-active {
  position: absolute;
  right: 0;
  left: 0;
}
</style>
